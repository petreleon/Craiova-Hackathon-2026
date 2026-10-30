//! Singularity-Free 6-DoF Quaternion Kinematics, IMU Pre-Integration ($\Delta t_{sync}$),
//! Stratified 3D Fog/Wind Field, and LRCV RWY 09 Guidance Metrics.

use crate::frames::{
    sanitize_f64, wrap_angle_deg, wrap_angle_rad, AttitudeQuaternion, BodyFrame, FramedVec3,
    NedFrameLrcv, ILS_GLIDESLOPE_DEG, RWY09_HEADING_DEG,
};
use nalgebra::{UnitQuaternion, Vector3};
use serde::{Deserialize, Serialize};

/// Pre-integrator IMU pe durata expunerii cadrului camerei $[t_{k-1} + \hat{\Delta t}_{sync}, t_k + \hat{\Delta t}_{sync}]$.
/// Elimină Gimbal Lock și reziduurile de derotație la manevre rapide de roll ($p$), pitch ($q$) și yaw ($r$).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ImuPreintegrator {
    pub delta_q: UnitQuaternion<f64>,
    pub mean_omega_corr: FramedVec3<BodyFrame>,
    pub angular_accel: FramedVec3<BodyFrame>,
}

impl Default for ImuPreintegrator {
    #[inline(always)]
    fn default() -> Self {
        Self {
            delta_q: UnitQuaternion::identity(),
            mean_omega_corr: FramedVec3::zero(),
            angular_accel: FramedVec3::zero(),
        }
    }
}

impl ImuPreintegrator {
    /// Integrează exact sub-pașii IMU ($400\text{ Hz}$) pe durata cadrului ($60\text{ Hz}$)
    /// și aplică corecția de desincronizare temporală `dt_sync_est`.
    pub fn integrate_frame_window(
        imu_samples: &[(f64, FramedVec3<BodyFrame>)],
        gyro_bias: FramedVec3<BodyFrame>,
        dt_sync_est: f64,
        prev_omega: FramedVec3<BodyFrame>,
    ) -> Self {
        let mut q_accum = UnitQuaternion::identity();
        let mut omega_sum = Vector3::zeros();
        let mut total_dt = 0.0_f64;

        let safe_bias = Vector3::new(
            sanitize_f64(gyro_bias.raw.x, -5.0, 5.0, 0.0),
            sanitize_f64(gyro_bias.raw.y, -5.0, 5.0, 0.0),
            sanitize_f64(gyro_bias.raw.z, -5.0, 5.0, 0.0),
        );

        for &(dt_raw, omega_raw) in imu_samples {
            let dt = sanitize_f64(dt_raw, 0.0, 0.5, 0.0);
            if dt <= 0.0 {
                continue;
            }
            let w_raw = Vector3::new(
                sanitize_f64(omega_raw.raw.x, -30.0, 30.0, 0.0),
                sanitize_f64(omega_raw.raw.y, -30.0, 30.0, 0.0),
                sanitize_f64(omega_raw.raw.z, -30.0, 30.0, 0.0),
            );
            let omega_unbiased = w_raw - safe_bias;
            q_accum = q_accum * UnitQuaternion::from_scaled_axis(omega_unbiased * dt);
            omega_sum += omega_unbiased * dt;
            total_dt += dt;
        }
        q_accum.renormalize();

        let prev_w = Vector3::new(
            sanitize_f64(prev_omega.raw.x, -30.0, 30.0, 0.0),
            sanitize_f64(prev_omega.raw.y, -30.0, 30.0, 0.0),
            sanitize_f64(prev_omega.raw.z, -30.0, 30.0, 0.0),
        );

        let mean_w = if total_dt > 1e-6 {
            omega_sum / total_dt
        } else {
            Vector3::zeros()
        };
        let alpha_w = if total_dt > 1e-6 {
            (mean_w - prev_w) / total_dt
        } else {
            Vector3::zeros()
        };
        let safe_dt_sync = sanitize_f64(dt_sync_est, -0.025, 0.025, 0.0);
        let sync_corrected_w = mean_w + alpha_w * safe_dt_sync;

        Self {
            delta_q: q_accum,
            mean_omega_corr: FramedVec3::new(
                sync_corrected_w.x,
                sync_corrected_w.y,
                sync_corrected_w.z,
            ),
            angular_accel: FramedVec3::new(alpha_w.x, alpha_w.y, alpha_w.z),
        }
    }

    /// Construiește o fereastră de pre-integrare din rata curentă și cea anterioară prin 8 sub-eșantioane.
    pub fn from_single_frame_rate(
        current_omega_raw: FramedVec3<BodyFrame>,
        prev_omega_unbiased: FramedVec3<BodyFrame>,
        gyro_bias: FramedVec3<BodyFrame>,
        frame_dt: f64,
        dt_sync_est: f64,
    ) -> Self {
        let safe_dt = sanitize_f64(frame_dt, 1e-4, 0.2, 1.0 / 60.0);
        let sub_dt = safe_dt / 8.0;
        let mut samples = [(0.0_f64, FramedVec3::<BodyFrame>::zero()); 8];
        for (i, item) in samples.iter_mut().enumerate() {
            let frac = (i as f64 + 0.5) / 8.0;
            let interp = prev_omega_unbiased.raw * (1.0 - frac) + current_omega_raw.raw * frac;
            *item = (sub_dt, FramedVec3::from_raw(interp));
        }
        Self::integrate_frame_window(&samples, gyro_bias, dt_sync_est, prev_omega_unbiased)
    }
}

/// Comenzi de control 6-DoF pentru aeronavă:
/// viraj stânga/dreapta (yaw), urcare/coborâre (pitch), rotire în jurul axei longitudinale (roll)
/// și accelerare/decelerare (throttle $a_x$).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ControlInput6DoF {
    pub roll_rate_cmd_rad_s: f64,
    pub pitch_rate_cmd_rad_s: f64,
    pub yaw_rate_cmd_rad_s: f64,
    pub accel_x_cmd_mps2: f64,
    pub accel_y_cmd_mps2: f64,
    pub accel_z_cmd_mps2: f64,
}

impl Default for ControlInput6DoF {
    #[inline(always)]
    fn default() -> Self {
        Self::zero()
    }
}

impl ControlInput6DoF {
    /// Comandă nulă (zbor stabilizat la viteză și atitudine constantă).
    #[inline(always)]
    pub const fn zero() -> Self {
        Self {
            roll_rate_cmd_rad_s: 0.0,
            pitch_rate_cmd_rad_s: 0.0,
            yaw_rate_cmd_rad_s: 0.0,
            accel_x_cmd_mps2: 0.0,
            accel_y_cmd_mps2: 0.0,
            accel_z_cmd_mps2: 0.0,
        }
    }

    /// Creează un `ControlInput6DoF` dintr-un pas discret de manevră exprimat în grade și $\Delta u$ [m/s].
    pub fn from_maneuver_step_deg(
        d_yaw_deg: f64,
        d_pitch_deg: f64,
        d_roll_deg: f64,
        d_throttle_mps: f64,
        dt: f64,
    ) -> Self {
        let safe_dt = sanitize_f64(dt, 1e-3, 1.0, 1.0 / 60.0);
        let d_yaw_rad = sanitize_f64(d_yaw_deg, -90.0, 90.0, 0.0).to_radians();
        let d_pitch_rad = sanitize_f64(d_pitch_deg, -60.0, 60.0, 0.0).to_radians();
        let d_roll_rad = sanitize_f64(d_roll_deg, -90.0, 90.0, 0.0).to_radians();
        let d_u = sanitize_f64(d_throttle_mps, -50.0, 50.0, 0.0);

        let pitch_rate = (d_pitch_rad / safe_dt).clamp(-6.0, 6.0);
        Self {
            roll_rate_cmd_rad_s: (d_roll_rad / safe_dt).clamp(-8.0, 8.0),
            pitch_rate_cmd_rad_s: pitch_rate,
            yaw_rate_cmd_rad_s: (d_yaw_rad / safe_dt).clamp(-6.0, 6.0),
            accel_x_cmd_mps2: (d_u / safe_dt).clamp(-35.0, 35.0),
            accel_y_cmd_mps2: 0.0,
            // La cabraj (+pitch), portanța crescută imprimă o componentă verticală negativă în BodyFrame (w < 0 => urcare)
            accel_z_cmd_mps2: (-12.0 * pitch_rate).clamp(-40.0, 40.0),
        }
    }

    /// Creează un `ControlInput6DoF` din rate unghiulare continue [deg/s] și accelerație longitudinală [m/s²].
    pub fn from_continuous_rates_deg(
        roll_dps: f64,
        pitch_dps: f64,
        yaw_dps: f64,
        accel_x_mps2: f64,
    ) -> Self {
        let r_rad = sanitize_f64(roll_dps, -180.0, 180.0, 0.0).to_radians();
        let p_rad = sanitize_f64(pitch_dps, -120.0, 120.0, 0.0).to_radians();
        let y_rad = sanitize_f64(yaw_dps, -120.0, 120.0, 0.0).to_radians();
        let ax = sanitize_f64(accel_x_mps2, -30.0, 30.0, 0.0);

        Self {
            roll_rate_cmd_rad_s: r_rad,
            pitch_rate_cmd_rad_s: p_rad,
            yaw_rate_cmd_rad_s: y_rad,
            accel_x_cmd_mps2: ax,
            accel_y_cmd_mps2: 0.0,
            accel_z_cmd_mps2: (-10.0 * p_rad).clamp(-30.0, 30.0),
        }
    }
}

/// Câmpul 3D stratificat de ceață și vânt (`10m / 80m / 120m / 180m`) în zona Aeroportului Craiova (`LRCV`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FogField3D {
    /// Vizibilitatea meteorologică (Meteorological Optical Range $V_{MOR}$) în metri.
    pub visibility_mor_m: f64,
    /// Activarea canalului SWIR/LWIR pentru detecția timpurie a balizajului ALS.
    pub enable_swir_als: bool,
    /// Vectorul vitezei ceții/vântului la plafonul de 10m AGL în NED [m/s].
    pub layer_10m_wind_ned: FramedVec3<NedFrameLrcv>,
    /// Vectorul vitezei ceții/vântului la plafonul de 80m AGL în NED [m/s].
    pub layer_80m_wind_ned: FramedVec3<NedFrameLrcv>,
    /// Vectorul vitezei ceții/vântului la plafonul de 120m AGL în NED [m/s].
    pub layer_120m_wind_ned: FramedVec3<NedFrameLrcv>,
    /// Vectorul vitezei ceții/vântului la plafonul de 180m AGL în NED [m/s].
    pub layer_180m_wind_ned: FramedVec3<NedFrameLrcv>,
    /// Perturbația de rafală bruscă activă ($\Delta\mathbf{w}_{fog}$) în NED [m/s].
    pub active_gust_delta_ned: FramedVec3<NedFrameLrcv>,
    /// Frontul de rafală în fața aeronavei vizibil optic prin Lookahead în NED [m/s].
    pub pending_lookahead_gust_ned: FramedVec3<NedFrameLrcv>,
    /// Intensitatea turbulenței locale Von Kármán ($\in [0.0, 1.0]$).
    pub turbulence_intensity: f64,
}

impl Default for FogField3D {
    fn default() -> Self {
        Self::default_lrcv_cat3c()
    }
}

impl FogField3D {
    /// Configurație implicită CAT IIIc pe LRCV RWY 09 (vizibilitate 50m, forfecare moderată pe plafoane).
    pub fn default_lrcv_cat3c() -> Self {
        Self {
            visibility_mor_m: 50.0,
            enable_swir_als: true,
            layer_10m_wind_ned: FramedVec3::new(-1.5, 3.2, 0.1),
            layer_80m_wind_ned: FramedVec3::new(-2.8, 5.5, 0.2),
            layer_120m_wind_ned: FramedVec3::new(-4.0, 7.8, 0.3),
            layer_180m_wind_ned: FramedVec3::new(-5.5, 9.5, 0.4),
            active_gust_delta_ned: FramedVec3::zero(),
            pending_lookahead_gust_ned: FramedVec3::zero(),
            turbulence_intensity: 0.15,
        }
    }

    /// Setează vizibilitatea meteorologică $V_{MOR}$ și starea senzorului SWIR.
    pub fn set_visibility_mor(&mut self, visibility_mor_m: f64, enable_swir_als: bool) {
        self.visibility_mor_m = sanitize_f64(visibility_mor_m, 15.0, 10_000.0, 50.0);
        self.enable_swir_als = enable_swir_als;
    }

    /// Coeficientul de extincție optică în spectrul vizibil conform legii lui Koschmieder:
    /// $\beta_{ext} = \frac{3.0}{V_{MOR}}$ [$\text{m}^{-1}$].
    #[inline(always)]
    pub fn beta_ext_koschmieder(&self) -> f64 {
        let safe_vis = sanitize_f64(self.visibility_mor_m, 12.0, 10_000.0, 50.0);
        3.0 / safe_vis
    }

    /// Coeficientul de extincție în banda SWIR/LWIR ($\beta_{SWIR} \approx 0.25 \cdot \beta_{VIS}$).
    #[inline(always)]
    pub fn beta_ext_swir(&self) -> f64 {
        0.25 * self.beta_ext_koschmieder()
    }

    /// Inversa adâncimii efective nominale a stratului de ceață $\hat{\rho}_{fog, opt} \in [0.002, 0.25]\text{ m}^{-1}$.
    #[inline(always)]
    pub fn nominal_rho_fog(&self) -> f64 {
        self.beta_ext_koschmieder().clamp(0.002, 0.25)
    }

    /// Opacitatea / ponderea stratului de ceață $\alpha_{fog}(Z) = 1 - e^{-\beta_{ext} Z} \in [0.0, 1.0]$.
    #[inline(always)]
    pub fn alpha_fog(&self, slant_range_m: f64) -> f64 {
        let z = sanitize_f64(slant_range_m, 0.5, 10_000.0, 50.0);
        let beta = if self.enable_swir_als {
            0.55 * self.beta_ext_koschmieder()
        } else {
            self.beta_ext_koschmieder()
        };
        (1.0 - (-beta * z).exp()).clamp(0.0, 1.0)
    }

    /// Vântul de bază (fără rafala bruscă) interpolat liniar între plafoanele `10m / 80m / 120m / 180m`.
    pub fn base_wind_ned_at_altitude(&self, h_agl: f64) -> FramedVec3<NedFrameLrcv> {
        let h = sanitize_f64(h_agl, 0.0, 2000.0, 50.0);
        let raw = if h <= 10.0 {
            self.layer_10m_wind_ned.raw
        } else if h <= 80.0 {
            let t = (h - 10.0) / 70.0;
            self.layer_10m_wind_ned.raw * (1.0 - t) + self.layer_80m_wind_ned.raw * t
        } else if h <= 120.0 {
            let t = (h - 80.0) / 40.0;
            self.layer_80m_wind_ned.raw * (1.0 - t) + self.layer_120m_wind_ned.raw * t
        } else if h <= 180.0 {
            let t = (h - 120.0) / 60.0;
            self.layer_120m_wind_ned.raw * (1.0 - t) + self.layer_180m_wind_ned.raw * t
        } else {
            self.layer_180m_wind_ned.raw
        };
        FramedVec3::from_raw(raw)
    }

    /// Vectorul curent complet al vitezei ceții/vântului $\mathbf{w}_{fog}(z, t)$ la altitudinea `h_agl` în NED.
    pub fn wind_ned_at_altitude(&self, h_agl: f64) -> FramedVec3<NedFrameLrcv> {
        self.base_wind_ned_at_altitude(h_agl) + self.active_gust_delta_ned
    }

    /// Injectează o rafală bruscă de ceață/vânt $\Delta\mathbf{w}_{fog}$ în NED.
    pub fn inject_sudden_gust(
        &mut self,
        dw_ned: FramedVec3<NedFrameLrcv>,
        turbulence_intensity: f64,
    ) {
        let safe_dw = FramedVec3::sanitized(dw_ned.x(), dw_ned.y(), dw_ned.z(), -60.0, 60.0);
        self.active_gust_delta_ned = safe_dw;
        self.pending_lookahead_gust_ned = safe_dw;
        self.turbulence_intensity = sanitize_f64(turbulence_intensity, 0.0, 1.0, 0.35);
    }

    /// Returnează viteza ceții în volumul din fața aeronavei la distanța `lookahead_dist_m`
    /// pentru detecția anticipativă `Optical Gust Lookahead`.
    pub fn lookahead_wind_ned(
        &self,
        pos_ned: FramedVec3<NedFrameLrcv>,
        lookahead_dist_m: f64,
    ) -> FramedVec3<NedFrameLrcv> {
        let h_agl = (-pos_ned.z()).max(0.5);
        let _dist = sanitize_f64(lookahead_dist_m, 10.0, 200.0, 50.0);
        self.base_wind_ned_at_altitude(h_agl) + self.pending_lookahead_gust_ned
    }
}

/// Parametrii de ghidaj către pragul pistei RWY 09 a Aeroportului Internațional Craiova (`LRCV`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LrcvGuidanceMetrics {
    pub distance_to_rwy09_m: f64,
    pub horiz_distance_m: f64,
    pub bearing_to_rwy09_deg: f64,
    pub heading_correction_deg: f64,
    pub glideslope_error_deg: f64,
    pub eta_touchdown_sec: f64,
}

impl LrcvGuidanceMetrics {
    /// Eticheta fazei curente din Pâlnia de Apropiere în 3 Faze către RWY 09 Craiova.
    #[inline(always)]
    pub fn approach_phase_label(&self) -> &'static str {
        if self.horiz_distance_m > 1500.0 {
            "PHASE 1: OUTER (10->1.5km)"
        } else if self.horiz_distance_m > 300.0 {
            "PHASE 2: ALS SWIR (1.5km->300m)"
        } else {
            "PHASE 3: THRESHOLD RWY 09"
        }
    }
}

/// Starea cinematică și dinamică completă 6-DoF a aeronavei în reperul `LRCV` ($\mathcal{F}_n$) și reperul corpului ($\mathcal{F}_b$).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AircraftState6DoF {
    /// Poziția 3D în NED față de pragul RWY 09 Craiova ($\mathbf{p}_{LRCV} = [0, 0, 0]^T$).
    pub pos_ned: FramedVec3<NedFrameLrcv>,
    /// Viteza liniară în reperul corpului $\mathbf{v}_b = [u, v, w]^T$ [m/s].
    pub vel_body: FramedVec3<BodyFrame>,
    /// Orientarea 3D fără singularități prin cuaternion unitar $\mathbf{q}_b^n$.
    pub attitude: AttitudeQuaternion,
    /// Viteza unghiulară în reperul corpului $\boldsymbol{\omega}_b = [p, q, r]^T$ [rad/s].
    pub omega_body: FramedVec3<BodyFrame>,
    /// Accelerația liniară specifică în reperul corpului $\mathbf{a}_b = [a_x, a_y, a_z]^T$ [m/s²].
    pub accel_body: FramedVec3<BodyFrame>,
    /// Unghiul de atac aerodinamic $\alpha_{AOA}$ [rad].
    pub alpha_aoa_rad: f64,
    /// Unghiul de derapaj lateral $\beta_{sideslip}$ [rad].
    pub beta_sideslip_rad: f64,
    /// Unghiul de corecție în vânt $\psi_{crab}$ comandat de autopilot [rad].
    pub crab_angle_cmd_rad: f64,
    /// Timpul de simulare acumulat [s].
    pub sim_time_sec: f64,
}

impl Default for AircraftState6DoF {
    fn default() -> Self {
        Self::new_initial_approach_lrcv()
    }
}

impl AircraftState6DoF {
    /// Inițializează aeronava pe panta nominală ILS ($3.0^\circ$, cap $087^\circ$) către RWY 09 Craiova (`LRCV`),
    /// la distanța orizontală de $1800\text{ m}$ de pragul pistei și viteza de apropiere $u = 70\text{ m/s}$.
    pub fn new_initial_approach_lrcv() -> Self {
        let d_horiz = 1800.0_f64;
        let psi_rwy_rad = RWY09_HEADING_DEG.to_radians();
        let glide_rad = ILS_GLIDESLOPE_DEG.to_radians();

        let x_ned = -d_horiz * psi_rwy_rad.cos();
        let y_ned = -d_horiz * psi_rwy_rad.sin();
        let h_agl = d_horiz * glide_rad.tan();
        let z_ned = -h_agl;

        Self {
            pos_ned: FramedVec3::new(x_ned, y_ned, z_ned),
            vel_body: FramedVec3::new(70.0, 0.0, 0.0),
            attitude: AttitudeQuaternion::from_euler_deg(0.0, -ILS_GLIDESLOPE_DEG, RWY09_HEADING_DEG),
            omega_body: FramedVec3::zero(),
            accel_body: FramedVec3::zero(),
            alpha_aoa_rad: glide_rad,
            beta_sideslip_rad: 0.0,
            crab_angle_cmd_rad: psi_rwy_rad,
            sim_time_sec: 0.0,
        }
    }

    /// Altitudinea deasupra solului $h_{AGL}(t) = -z(t) \ge 0.5\text{ m}$.
    #[inline(always)]
    pub fn h_agl_m(&self) -> f64 {
        (-self.pos_ned.z()).max(0.5)
    }

    /// Viteza reală față de sol în reperul NED: $\mathbf{v}_{ground}(t) = \mathbf{R}_b^n(\mathbf{q})\mathbf{v}_b(t)$.
    #[inline(always)]
    pub fn vel_ground_ned(&self) -> FramedVec3<NedFrameLrcv> {
        self.attitude.body_to_ned(self.vel_body)
    }

    /// Execută un pas de integrare cinematică și aerodinamică 6-DoF pe durata `dt` [secunde].
    /// Actualizează cuaternionul $\mathbf{q}_b^n$, viteza $\mathbf{v}_b$, cuplajul cu ceața/vântul și poziția NED.
    pub fn step_6dof(&mut self, ctrl: &ControlInput6DoF, fog: &FogField3D, dt: f64) {
        let safe_dt = sanitize_f64(dt, 1e-4, 0.25, 1.0 / 60.0);

        // 1. Calculul vitezei relative față de masa de aer/ceață în BodyFrame
        let h_agl = self.h_agl_m();
        let wind_ned = fog.wind_ned_at_altitude(h_agl);
        let wind_body = self.attitude.ned_to_body(wind_ned);
        let v_air_body = self.vel_body - wind_body;

        let u_air = v_air_body.x().max(10.0);
        let v_air = v_air_body.y();
        let w_air = v_air_body.z();
        let v_air_norm = v_air_body.norm().max(15.0);

        self.alpha_aoa_rad = w_air.atan2(u_air);
        self.beta_sideslip_rad = (v_air / v_air_norm).clamp(-0.85, 0.85).asin();

        // 2. Cuplajul aerodinamic Dutch Roll (C_l_beta < 0, C_n_beta > 0) indus de derapajul în rafală
        let p_aero_gust = -0.35 * self.beta_sideslip_rad;
        let r_aero_gust = 0.28 * self.beta_sideslip_rad;

        let p_cmd = sanitize_f64(ctrl.roll_rate_cmd_rad_s, -8.0, 8.0, 0.0);
        let q_cmd = sanitize_f64(ctrl.pitch_rate_cmd_rad_s, -6.0, 6.0, 0.0);
        let r_cmd = sanitize_f64(ctrl.yaw_rate_cmd_rad_s, -6.0, 6.0, 0.0);

        self.omega_body = FramedVec3::new(
            p_cmd + p_aero_gust,
            q_cmd,
            r_cmd + r_aero_gust,
        );

        // 3. Integrarea exactă a cuaternionului: q_{k+1} = q_k * exp(0.5 * omega * dt)
        self.attitude.integrate_body_rate(self.omega_body, safe_dt);

        // 4. Actualizarea accelerației și vitezei liniare în BodyFrame
        let ax_cmd = sanitize_f64(ctrl.accel_x_cmd_mps2, -35.0, 35.0, 0.0);
        let ay_cmd = sanitize_f64(ctrl.accel_y_cmd_mps2, -25.0, 25.0, 0.0)
            + 0.45 * wind_body.y()
            - 1.2 * self.vel_body.y();
        let az_cmd = if ctrl.accel_z_cmd_mps2.abs() > 1e-6 {
            sanitize_f64(ctrl.accel_z_cmd_mps2, -40.0, 40.0, 0.0)
        } else {
            (-12.0 * q_cmd - 1.5 * self.vel_body.z()).clamp(-40.0, 40.0)
        };

        self.accel_body = FramedVec3::new(ax_cmd, ay_cmd, az_cmd);

        let new_u = (self.vel_body.x() + self.accel_body.x() * safe_dt).clamp(22.0, 240.0);
        let new_v = (self.vel_body.y() + self.accel_body.y() * safe_dt).clamp(-35.0, 35.0);
        let new_w = (self.vel_body.z() + self.accel_body.z() * safe_dt).clamp(-35.0, 35.0);
        self.vel_body = FramedVec3::new(new_u, new_v, new_w);

        // 5. Integrarea poziției 3D în NED: p(t+dt) = p(t) + v_ground*dt + 0.5*R_b^n*a_b*dt^2
        let v_ground = self.vel_ground_ned();
        let a_ned = self.attitude.body_to_ned(self.accel_body);
        let mut next_pos =
            self.pos_ned.raw + v_ground.raw * safe_dt + 0.5 * a_ned.raw * (safe_dt * safe_dt);
        // Plafon minim de siguranță la sol (z_ned <= -0.5m => h_AGL >= 0.5m)
        if next_pos.z > -0.5 {
            next_pos.z = -0.5;
        }
        self.pos_ned = FramedVec3::from_raw(next_pos);

        // 6. Calculul unghiului de corecție în vânt (Crab Angle psi_crab) către RWY 09
        let psi_lrcv = (-self.pos_ned.y()).atan2(-self.pos_ned.x());
        let rwy_dir_ned = Vector3::new(psi_lrcv.cos(), psi_lrcv.sin(), 0.0);
        let cross_dir_ned = Vector3::new(-rwy_dir_ned.y, rwy_dir_ned.x, 0.0);
        let w_crosswind = wind_ned.raw.dot(&cross_dir_ned);
        let crab_offset = (w_crosswind / v_air_norm.max(25.0)).clamp(-0.45, 0.45).asin();
        self.crab_angle_cmd_rad = wrap_angle_rad(psi_lrcv - crab_offset);

        self.sim_time_sec += safe_dt;
    }

    /// Aplică instantaneu o manevră discretă 6-DoF comandată de pilot/juriu:
    /// `d_yaw_deg` (+dreapta / -stânga), `d_pitch_deg` (+urcare / -coborâre),
    /// `d_roll_deg` (+înclinare dreapta / -stânga), `d_throttle_mps` (+accelerare / -decelerare).
    pub fn apply_maneuver_step_deg(
        &mut self,
        d_yaw_deg: f64,
        d_pitch_deg: f64,
        d_roll_deg: f64,
        d_throttle_mps: f64,
    ) {
        let d_yaw = sanitize_f64(d_yaw_deg, -90.0, 90.0, 0.0).to_radians();
        let d_pitch = sanitize_f64(d_pitch_deg, -60.0, 60.0, 0.0).to_radians();
        let d_roll = sanitize_f64(d_roll_deg, -90.0, 90.0, 0.0).to_radians();
        let d_u = sanitize_f64(d_throttle_mps, -60.0, 60.0, 0.0);

        let (cur_roll, cur_pitch, cur_yaw) = self.attitude.to_euler_rad();
        let next_roll = wrap_angle_rad(cur_roll + d_roll);
        let next_pitch = (cur_pitch + d_pitch).clamp(-1.48, 1.48);
        let next_yaw = wrap_angle_rad(cur_yaw + d_yaw);
        self.attitude = AttitudeQuaternion::from_euler_rad(next_roll, next_pitch, next_yaw);

        let dt_nom = 0.1_f64;
        self.omega_body = FramedVec3::new(d_roll / dt_nom, d_pitch / dt_nom, d_yaw / dt_nom);

        let new_u = (self.vel_body.x() + d_u).clamp(22.0, 240.0);
        // La urcare (+pitch), w devine negativ (portanță ascendentă în BodyFrame); la coborâre (-pitch), w > 0.
        let new_w = if d_pitch.abs() > 1e-6 {
            (-new_u * d_pitch.sin() * 0.35).clamp(-35.0, 35.0)
        } else {
            self.vel_body.z()
        };
        let new_v = if d_yaw.abs() > 1e-6 {
            (new_u * d_yaw.sin() * 0.12).clamp(-25.0, 25.0)
        } else {
            self.vel_body.y()
        };
        self.accel_body = FramedVec3::new(
            d_u / dt_nom,
            (new_v - self.vel_body.y()) / dt_nom,
            (new_w - self.vel_body.z()) / dt_nom,
        );
        self.vel_body = FramedVec3::new(new_u, new_v, new_w);

        let v_ground = self.vel_ground_ned();
        let mut next_pos = self.pos_ned.raw + v_ground.raw * dt_nom;
        if next_pos.z > -0.5 {
            next_pos.z = -0.5;
        }
        self.pos_ned = FramedVec3::from_raw(next_pos);
        self.sim_time_sec += dt_nom;
    }

    /// Calculează distanța 3D, azimutul, eroarea de cap/pantă ILS și timpul estimat până la touchdown (ETA)
    /// pe pista RWY 09 Craiova (`LRCV`).
    pub fn compute_lrcv_guidance(&self) -> LrcvGuidanceMetrics {
        let x = self.pos_ned.x();
        let y = self.pos_ned.y();
        let h_agl = self.h_agl_m();

        let horiz_dist = (x * x + y * y).sqrt();
        let dist_3d = (x * x + y * y + h_agl * h_agl).sqrt();

        let psi_lrcv_rad = (-y).atan2(-x);
        let mut bearing_deg = psi_lrcv_rad.to_degrees();
        if bearing_deg < 0.0 {
            bearing_deg += 360.0;
        }

        let (_, _, yaw_rad) = self.attitude.to_euler_rad();
        let target_heading_rad = if self.crab_angle_cmd_rad.abs() > 1e-6 {
            self.crab_angle_cmd_rad
        } else {
            psi_lrcv_rad
        };
        let heading_corr_deg = wrap_angle_deg((target_heading_rad - yaw_rad).to_degrees());

        let current_glide_deg = h_agl.atan2(horiz_dist.max(1.0)).to_degrees();
        let glideslope_error_deg = current_glide_deg - ILS_GLIDESLOPE_DEG;

        let ground_speed = self.vel_ground_ned().norm().max(5.0);
        let eta_touchdown_sec = dist_3d / ground_speed;

        LrcvGuidanceMetrics {
            distance_to_rwy09_m: dist_3d,
            horiz_distance_m: horiz_dist,
            bearing_to_rwy09_deg: bearing_deg,
            heading_correction_deg: heading_corr_deg,
            glideslope_error_deg,
            eta_touchdown_sec,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maneuver_yaw_right_and_left() {
        let fog = FogField3D::default_lrcv_cat3c();
        let mut state_right = AircraftState6DoF::new_initial_approach_lrcv();
        let (_, _, yaw0) = state_right.attitude.to_euler_rad();

        state_right.apply_maneuver_step_deg(15.0, 0.0, 0.0, 0.0);
        let (_, _, yaw_right) = state_right.attitude.to_euler_rad();
        assert!(wrap_angle_rad(yaw_right - yaw0) > 0.20);

        let mut state_left = AircraftState6DoF::new_initial_approach_lrcv();
        let ctrl_left = ControlInput6DoF::from_continuous_rates_deg(0.0, 0.0, -30.0, 0.0);
        for _ in 0..30 {
            state_left.step_6dof(&ctrl_left, &fog, 1.0 / 60.0);
        }
        let (_, _, yaw_left) = state_left.attitude.to_euler_rad();
        assert!(wrap_angle_rad(yaw_left - yaw0) < -0.15);
    }

    #[test]
    fn test_maneuver_pitch_climb_and_descent() {
        let fog = FogField3D::default_lrcv_cat3c();
        let mut state_climb = AircraftState6DoF::new_initial_approach_lrcv();
        let h0 = state_climb.h_agl_m();

        // Urcare (+pitch): w < 0 și h_AGL crește
        state_climb.apply_maneuver_step_deg(0.0, 12.0, 0.0, 0.0);
        assert!(state_climb.vel_body.z() < 0.0);
        let ctrl_climb = ControlInput6DoF::from_continuous_rates_deg(0.0, 8.0, 0.0, 0.0);
        for _ in 0..30 {
            state_climb.step_6dof(&ctrl_climb, &fog, 1.0 / 60.0);
        }
        assert!(state_climb.h_agl_m() > h0);

        // Coborâre (-pitch): h_AGL scade
        let mut state_desc = AircraftState6DoF::new_initial_approach_lrcv();
        let h_start = state_desc.h_agl_m();
        state_desc.apply_maneuver_step_deg(0.0, -8.0, 0.0, 0.0);
        assert!(state_desc.vel_body.z() > 0.0);
        let ctrl_desc = ControlInput6DoF::from_continuous_rates_deg(0.0, -5.0, 0.0, 0.0);
        for _ in 0..30 {
            state_desc.step_6dof(&ctrl_desc, &fog, 1.0 / 60.0);
        }
        assert!(state_desc.h_agl_m() < h_start);
    }

    #[test]
    fn test_maneuver_roll_right_and_left() {
        let mut state = AircraftState6DoF::new_initial_approach_lrcv();
        state.apply_maneuver_step_deg(0.0, 0.0, 25.0, 0.0);
        let euler_right = state.attitude.to_euler_deg();
        assert!((euler_right[0] - 25.0).abs() < 1e-3);

        state.apply_maneuver_step_deg(0.0, 0.0, -50.0, 0.0);
        let euler_left = state.attitude.to_euler_deg();
        assert!((euler_left[0] + 25.0).abs() < 1e-3);
    }

    #[test]
    fn test_maneuver_throttle_accelerate_and_decelerate() {
        let mut state = AircraftState6DoF::new_initial_approach_lrcv();
        let u0 = state.vel_body.x();
        let eta0 = state.compute_lrcv_guidance().eta_touchdown_sec;

        // Accelerare (+throttle)
        state.apply_maneuver_step_deg(0.0, 0.0, 0.0, 20.0);
        let u_acc = state.vel_body.x();
        let eta_acc = state.compute_lrcv_guidance().eta_touchdown_sec;
        assert!(u_acc > u0);
        assert!(eta_acc < eta0);

        // Decelerare (-throttle)
        state.apply_maneuver_step_deg(0.0, 0.0, 0.0, -35.0);
        let u_dec = state.vel_body.x();
        let eta_dec = state.compute_lrcv_guidance().eta_touchdown_sec;
        assert!(u_dec < u0);
        assert!(eta_dec > eta_acc);
    }

    #[test]
    fn test_imu_preintegrator_with_dt_sync() {
        let bias = FramedVec3::<BodyFrame>::new(0.01, -0.005, 0.002);
        let prev_w = FramedVec3::<BodyFrame>::new(0.2, 0.1, -0.1);
        let current_w_raw = FramedVec3::<BodyFrame>::new(0.61, 0.295, -0.098);
        let pre = ImuPreintegrator::from_single_frame_rate(
            current_w_raw,
            prev_w,
            bias,
            1.0 / 60.0,
            0.008, // +8ms shutter sync offset
        );
        assert!((pre.delta_q.norm() - 1.0).abs() < 1e-12);
        assert!(pre.angular_accel.x() > 10.0);
        assert!(pre.mean_omega_corr.x() > 0.4);
    }
}
