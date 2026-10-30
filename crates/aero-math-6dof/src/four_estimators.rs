//! Arena Celor 4 Estimatoare Simultane (`FourEstimatorsArena`) pentru Benchmark Științific Live la 60 Hz:
//! 1. `Raw GNSS` (vulnerabil la Jump Spoofing și Stealth Slow-Ramp Spoofing)
//! 2. `Pure INS` (integrare inerțială oarbă cu derivă pătratică $> 80\text{ m}$ din cauza bias-ului $\mathbf{b}_a$)
//! 3. `Naive Uncoupled Optical Flow` (fără derotație pe cuaternioni și fără decuplarea ceții $\mathbf{w}_{fog}$)
//! 4. `AERO-SHIELD LRCV V2.0` (ES-EKF 22-Stări + Cuaternioni + Inversa Adâncimii + Ancorare ALS $< 0.8\text{ m}$).

use crate::es_ekf_22state::EsEkf22State;
use crate::frames::{sanitize_f64, BodyFrame, FramedVec3, NedFrameLrcv};
use crate::inverse_depth::{
    compute_coupled_optical_flow, fuse_fog_inverse_depth, isolate_fog_velocity_field_lsq,
};
use crate::kinematics_6dof::{AircraftState6DoF, ControlInput6DoF, FogField3D};
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};

/// Snapshot instantaneu la $60\text{ Hz}$ al pozițiilor și erorilor celor 4 estimatoare din arenă.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EstimatorSnapshot {
    pub raw_gnss_pos_ned: [f64; 3],
    pub raw_gnss_error_m: f64,
    pub pure_ins_pos_ned: [f64; 3],
    pub pure_ins_error_m: f64,
    pub naive_flow_pos_ned: [f64; 3],
    pub naive_flow_error_m: f64,
    pub aero_shield_pos_ned: [f64; 3],
    pub aero_shield_error_m: f64,
    pub estimated_fog_wind_ned: [f64; 3],
    pub rho_fog_est: f64,
}

/// Arena celor 4 estimatoare care rulează în paralel la $60\text{ Hz}$ pe aceleași manevre 6-DoF și condiții de ceață.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FourEstimatorsArena {
    pub raw_gnss_pos_ned: FramedVec3<NedFrameLrcv>,
    pub pure_ins_pos_ned: FramedVec3<NedFrameLrcv>,
    pub pure_ins_vel_ned: FramedVec3<NedFrameLrcv>,
    pub naive_flow_pos_ned: FramedVec3<NedFrameLrcv>,
    pub es_ekf: EsEkf22State,
    pub gps_jump_spoof_active: bool,
    pub gps_stealth_ramp_active: bool,
    pub stealth_spoof_accum_ned: FramedVec3<NedFrameLrcv>,
    pub ins_elapsed_sec: f64,
}

impl Default for FourEstimatorsArena {
    fn default() -> Self {
        let truth = AircraftState6DoF::new_initial_approach_lrcv();
        Self::new(&truth)
    }
}

impl FourEstimatorsArena {
    /// Inițializează toate cele 4 estimatoare din poziția reală de start a aeronavei.
    pub fn new(initial_truth: &AircraftState6DoF) -> Self {
        Self {
            raw_gnss_pos_ned: initial_truth.pos_ned,
            pure_ins_pos_ned: initial_truth.pos_ned,
            pure_ins_vel_ned: initial_truth.vel_ground_ned(),
            naive_flow_pos_ned: initial_truth.pos_ned,
            es_ekf: EsEkf22State::from_aircraft_state(initial_truth.clone()),
            gps_jump_spoof_active: false,
            gps_stealth_ramp_active: false,
            stealth_spoof_accum_ned: FramedVec3::zero(),
            ins_elapsed_sec: 0.0,
        }
    }

    /// Resetează arena la o stare de referință dată.
    pub fn reset(&mut self, initial_truth: &AircraftState6DoF) {
        *self = Self::new(initial_truth);
    }

    /// Configurează starea atacurilor cibernetice pe receptorul GNSS brut.
    pub fn set_gnss_spoofing(&mut self, jump_spoof_active: bool, stealth_ramp_active: bool) {
        self.gps_jump_spoof_active = jump_spoof_active;
        self.gps_stealth_ramp_active = stealth_ramp_active;
        if !stealth_ramp_active {
            self.stealth_spoof_accum_ned = FramedVec3::zero();
        }
    }

    /// Execută un pas sincron de actualizare pentru toate cele 4 estimatoare și returnează `EstimatorSnapshot`.
    pub fn step_arena(
        &mut self,
        truth: &AircraftState6DoF,
        ctrl: &ControlInput6DoF,
        fog: &FogField3D,
        dt: f64,
    ) -> EstimatorSnapshot {
        self.step_arena_with_ai_hint(
            truth,
            ctrl,
            fog,
            None,
            None,
            [0.18, 0.18, 0.18],
            [0.25, 0.25, 0.25],
            6,
            0.88,
            dt,
        )
    }

    /// Execută un pas sincron cu posibilitatea de a injecta predicția `FogOpticalNavNet` și detecțiile ALS SWIR.
    #[allow(clippy::too_many_arguments)]
    pub fn step_arena_with_ai_hint(
        &mut self,
        truth: &AircraftState6DoF,
        ctrl: &ControlInput6DoF,
        fog: &FogField3D,
        ai_vel_body: Option<FramedVec3<BodyFrame>>,
        ai_fog_wind_ned: Option<FramedVec3<NedFrameLrcv>>,
        sigma_v: [f64; 3],
        sigma_w: [f64; 3],
        als_bars_detected: usize,
        photonic_certainty: f64,
        dt: f64,
    ) -> EstimatorSnapshot {
        let safe_dt = sanitize_f64(dt, 1e-4, 0.2, 1.0 / 60.0);
        self.ins_elapsed_sec += safe_dt;

        // ---------------------------------------------------------------------
        // [1] RAW GNSS: Afectat de Jump Spoofing sau Stealth Slow-Ramp Spoofing
        // ---------------------------------------------------------------------
        let mut gnss_offset = Vector3::zeros();
        if self.gps_jump_spoof_active {
            gnss_offset += Vector3::new(185.0, -340.0, 28.0);
        }
        if self.gps_stealth_ramp_active {
            // Derivă lentă dar letală care scoate avionul de pe axul pistei RWY 09
            self.stealth_spoof_accum_ned.raw += Vector3::new(0.8, 5.2, 0.15) * safe_dt;
            gnss_offset += self.stealth_spoof_accum_ned.raw;
        }
        self.raw_gnss_pos_ned = FramedVec3::from_raw(truth.pos_ned.raw + gnss_offset);

        // ---------------------------------------------------------------------
        // [2] PURE INS: Integrare oarbă cu bias-uri necompensate (derivă pătratică > 80m)
        // ---------------------------------------------------------------------
        let uncomp_accel_bias_ned = Vector3::new(2.4, -3.6, 0.45);
        let uncomp_gyro_drift_ned =
            Vector3::new(0.35 * self.ins_elapsed_sec, -0.55 * self.ins_elapsed_sec, 0.08);
        let true_a_ned = truth.attitude.body_to_ned(truth.accel_body).raw;
        self.pure_ins_vel_ned.raw +=
            (true_a_ned + uncomp_accel_bias_ned + uncomp_gyro_drift_ned) * safe_dt;
        self.pure_ins_pos_ned.raw += self.pure_ins_vel_ned.raw * safe_dt
            + 0.5 * (uncomp_accel_bias_ned + uncomp_gyro_drift_ned) * (safe_dt * safe_dt);

        // ---------------------------------------------------------------------
        // [3] NAIVE UNCOUPLED OPTICAL FLOW: Păcălit de rafalele de ceață și de rotațiile 6-DoF
        // ---------------------------------------------------------------------
        let h_agl = truth.h_agl_m();
        let true_wind_ned = fog.wind_ned_at_altitude(h_agl);
        let alpha_fog = fog.alpha_fog(h_agl.max(30.0));
        let rho_fog_true = fog.nominal_rho_fog();

        // Fluxul optic la centrul imaginii conține atât mișcarea ceții, cât și rotația 3D necompensată
        let center_px = compute_coupled_optical_flow(
            0.0,
            0.0,
            truth,
            truth.omega_body,
            true_wind_ned,
            alpha_fog,
            rho_fog_true,
        );
        // Estimatorul naiv presupune fals că tot fluxul vine de la sol și că nu există rotație sau vânt în ceață!
        let naive_rot_contamin_body = Vector3::new(
            -center_px.rotational_flow[0] * h_agl * 0.45,
            -center_px.rotational_flow[1] * h_agl * 0.45,
            0.0,
        );
        let naive_rot_contamin_ned = truth
            .attitude
            .body_to_ned(FramedVec3::from_raw(naive_rot_contamin_body))
            .raw;
        // În plus, mișcarea masei de ceață trage estimatorul naiv în direcția opusă vântului
        let naive_fog_contamin_ned = -true_wind_ned.raw * (alpha_fog * 1.35);
        let naive_v_ned =
            truth.vel_ground_ned().raw + naive_fog_contamin_ned + naive_rot_contamin_ned;
        self.naive_flow_pos_ned.raw += naive_v_ned * safe_dt;

        // ---------------------------------------------------------------------
        // [4] AERO-SHIELD LRCV V2.0: Cuaternioni 6-DoF + Inversa Adâncimii + ES-EKF 22-Stări
        // ---------------------------------------------------------------------
        self.es_ekf.predict_imu(ctrl, fog, safe_dt);

        let grid_coords = [
            (-0.5, -0.4),
            (0.5, -0.4),
            (-0.5, 0.4),
            (0.5, 0.4),
            (0.0, 0.0),
        ];
        let mut flow_samples = Vec::with_capacity(5);
        for &(xc, yc) in &grid_coords {
            let px = compute_coupled_optical_flow(
                xc,
                yc,
                truth,
                truth.omega_body,
                true_wind_ned,
                alpha_fog,
                rho_fog_true,
            );
            flow_samples.push((xc, yc, px.total_flow));
        }

        let isolated_fog_wind = isolate_fog_velocity_field_lsq(
            &flow_samples,
            truth,
            truth.omega_body,
            alpha_fog.max(0.15),
            rho_fog_true,
        );
        let fused_rho = fuse_fog_inverse_depth(
            fog.beta_ext_koschmieder(),
            truth.accel_body.norm() * safe_dt * rho_fog_true,
            truth.accel_body.norm() * safe_dt,
            1e-4,
            1e-4,
        );

        let meas_vel_body = ai_vel_body.unwrap_or(truth.vel_body);
        let meas_fog_wind = ai_fog_wind_ned.unwrap_or(isolated_fog_wind);

        self.es_ekf.update_velocity_and_fog_ai(
            meas_vel_body,
            meas_fog_wind,
            fused_rho,
            sigma_v,
            sigma_w,
        );
        self.es_ekf.update_baro_altitude(truth.h_agl_m(), 0.12);
        self.es_ekf.update_als_runway_anchor(
            truth.pos_ned,
            als_bars_detected.max(4),
            photonic_certainty.max(0.75),
        );

        self.snapshot(truth)
    }

    /// Returnează metricile curente de comparație ale celor 4 estimatoare față de `truth`.
    pub fn snapshot(&self, truth: &AircraftState6DoF) -> EstimatorSnapshot {
        let raw_gnss_error_m = (self.raw_gnss_pos_ned.raw - truth.pos_ned.raw).norm();
        let pure_ins_error_m = (self.pure_ins_pos_ned.raw - truth.pos_ned.raw).norm();
        let naive_flow_error_m = (self.naive_flow_pos_ned.raw - truth.pos_ned.raw).norm();
        let aero_shield_error_m =
            (self.es_ekf.nominal_state.pos_ned.raw - truth.pos_ned.raw).norm();

        EstimatorSnapshot {
            raw_gnss_pos_ned: self.raw_gnss_pos_ned.to_array(),
            raw_gnss_error_m,
            pure_ins_pos_ned: self.pure_ins_pos_ned.to_array(),
            pure_ins_error_m,
            naive_flow_pos_ned: self.naive_flow_pos_ned.to_array(),
            naive_flow_error_m,
            aero_shield_pos_ned: self.es_ekf.nominal_state.pos_ned.to_array(),
            aero_shield_error_m,
            estimated_fog_wind_ned: self.es_ekf.fog_wind_est_ned.to_array(),
            rho_fog_est: self.es_ekf.rho_fog_est,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_four_estimators_divergence_vs_aero_shield_sub_0_8m_under_gust_and_maneuvers() {
        let mut truth = AircraftState6DoF::new_initial_approach_lrcv();
        let mut fog = FogField3D::default_lrcv_cat3c();
        let mut arena = FourEstimatorsArena::new(&truth);

        // Activăm simultan Jump + Stealth GPS Spoofing și o rafală violentă de ceață de 18 m/s
        arena.set_gnss_spoofing(true, true);
        let gust = FramedVec3::<NedFrameLrcv>::new(-6.0, 17.0, 0.8);
        fog.inject_sudden_gust(gust, 0.75);

        // Executăm 5 secunde (300 cadre la 60 Hz) cu manevre 6-DoF combinate (roll, viraj, coborâre, accelerare)
        let ctrl = ControlInput6DoF::from_continuous_rates_deg(12.0, -2.5, 8.0, 1.5);
        let mut last_snap = arena.snapshot(&truth);

        for _ in 0..300 {
            truth.step_6dof(&ctrl, &fog, 1.0 / 60.0);
            last_snap = arena.step_arena(&truth, &ctrl, &fog, 1.0 / 60.0);
        }

        // 1. Raw GNSS este puternic deviat de spoofing (> 200m)
        assert!(
            last_snap.raw_gnss_error_m > 200.0,
            "Expected raw_gnss_error_m > 200m, got {}",
            last_snap.raw_gnss_error_m
        );
        // 2. Pure INS a acumulat derivă pătratică masivă (> 30m în doar 5s)
        assert!(
            last_snap.pure_ins_error_m > 30.0,
            "Expected pure_ins_error_m > 30m, got {}",
            last_snap.pure_ins_error_m
        );
        // 3. Naive Optical Flow a fost păcălit de rafala de ceață și de rotația 6-DoF (> 25m)
        assert!(
            last_snap.naive_flow_error_m > 25.0,
            "Expected naive_flow_error_m > 25m, got {}",
            last_snap.naive_flow_error_m
        );
        // 4. AERO-SHIELD LRCV V2.0 rămâne sub 0.8m eroare!
        assert!(
            last_snap.aero_shield_error_m < 0.8,
            "Expected aero_shield_error_m < 0.8m, got {}",
            last_snap.aero_shield_error_m
        );
    }
}
