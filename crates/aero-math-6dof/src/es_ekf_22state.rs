//! 22-State Error-State Extended Kalman Filter (`EsEkf22State`) on $SO(3)$ Manifold
//! with Joseph Stabilized Covariance Update, Cholesky/SVD Zero-Panic Solver,
//! Anisotropic Fisher Optical Flow Information, and ALS SWIR Runway Anchoring.

use crate::frames::{
    sanitize_f64, skew_symmetric, BodyFrame, FramedVec3, NedFrameLrcv,
};
use crate::inverse_depth::{
    compute_coupled_optical_flow, rotation_flow_matrix_b, translation_flow_matrix_a,
};
use crate::kinematics_6dof::{AircraftState6DoF, ControlInput6DoF, FogField3D};
use nalgebra::{DMatrix, SMatrix, SVector, Vector3};
use serde::{Deserialize, Serialize};

/// Index `0..3`: Eroarea de poziție 3D în reperul NED $\delta\mathbf{p}^n \in \mathbb{R}^3$.
pub const IDX_POS_NED: usize = 0;
/// Index `3..6`: Eroarea de viteză liniară în reperul corpului $\delta\mathbf{v}^b \in \mathbb{R}^3$.
pub const IDX_VEL_BODY: usize = 3;
/// Index `6..9`: Eroarea de atitudine 3D în algebra Lie $\delta\boldsymbol{\theta} \in \mathfrak{so}(3)$.
pub const IDX_ATT_THETA: usize = 6;
/// Index `9..12`: Eroarea bias-ului accelerometrului $\delta\mathbf{b}_a \in \mathbb{R}^3$.
pub const IDX_ACCEL_BIAS: usize = 9;
/// Index `12..15`: Eroarea bias-ului giroscopului $\delta\mathbf{b}_g \in \mathbb{R}^3$.
pub const IDX_GYRO_BIAS: usize = 12;
/// Index `15..18`: Eroarea vectorului vitezei ceții în NED $\delta\mathbf{w}_{fog}^n \in \mathbb{R}^3$.
pub const IDX_FOG_WIND_NED: usize = 15;
/// Index `18`: Eroarea inversei adâncimii efective a ceții $\delta\rho_{fog} \in \mathbb{R}^1$.
pub const IDX_RHO_FOG: usize = 18;
/// Index `19`: Eroarea decalajului temporal IMU–cameră $\delta\Delta t_{sync} \in \mathbb{R}^1$.
pub const IDX_DT_SYNC: usize = 19;
/// Index `20..22`: Offsetul de aliniere laterală și longitudinală pe balizajul ALS RWY 09 $\delta\mathbf{p}_{ALS} \in \mathbb{R}^2$.
pub const IDX_ALS_OFFSET: usize = 20;

/// Actualizează starea de eroare $\delta\mathbf{x} \in \mathbb{R}^{22}$ și matricea de covarianță $\mathbf{P} \in \mathbb{R}^{22 \times 22}$
/// folosind **Forma Stabilizată Joseph**:
/// $$\mathbf{P}^+ = (\mathbf{I} - \mathbf{K}\mathbf{H})\mathbf{P}(\mathbf{I} - \mathbf{K}\mathbf{H})^T + \mathbf{K}\mathbf{R}\mathbf{K}^T$$
/// cu regularizare Tikhonov ($10^{-6}\mathbf{I}$) și rezolvare rapidă Cholesky + fallback SVD sigur (Zero Panics!).
pub fn kalman_joseph_update<const M: usize>(
    x_err: &mut SVector<f64, 22>,
    p_cov: &mut SMatrix<f64, 22, 22>,
    innovation: &SVector<f64, M>,
    h_jac: &SMatrix<f64, M, 22>,
    r_meas: &SMatrix<f64, M, M>,
) {
    let hp_t: SMatrix<f64, 22, M> = (*p_cov) * h_jac.transpose();
    let s_raw: SMatrix<f64, M, M> = (*h_jac) * hp_t + (*r_meas);
    // Simetrizare + Regularizare Tikhonov (garantează inversabilitatea chiar și în Flat Whiteout 100%)
    let s_reg = (&s_raw + s_raw.transpose()) * 0.5 + SMatrix::<f64, M, M>::identity() * 1e-6;

    // Rezolvare rapidă Cholesky cu fallback SVD sigur (Zero Panics!)
    let k_gain: SMatrix<f64, 22, M> = if let Some(chol) = s_reg.cholesky() {
        chol.solve(&hp_t.transpose()).transpose()
    } else {
        let s_dyn = DMatrix::from_fn(M, M, |r, c| s_reg[(r, c)]);
        let rhs_dyn = DMatrix::from_fn(M, 22, |r, c| hp_t[(c, r)]);
        let svd = s_dyn.svd(true, true);
        let sol_dyn = svd
            .solve(&rhs_dyn, 1e-9)
            .unwrap_or_else(|_| DMatrix::zeros(M, 22));
        SMatrix::<f64, 22, M>::from_fn(|r, c| sol_dyn[(c, r)])
    };

    *x_err += k_gain * (*innovation);

    // Forma Stabilizată Joseph: P+ = (I - K*H)*P*(I - K*H)^T + K*R*K^T
    let i_kh = SMatrix::<f64, 22, 22>::identity() - k_gain * (*h_jac);
    let p_updated = i_kh * (*p_cov) * i_kh.transpose() + k_gain * (*r_meas) * k_gain.transpose();
    *p_cov = (&p_updated + p_updated.transpose()) * 0.5;
}

/// Filtrul Kalman Extins în Starea de Eroare cu 22 de Stări (`EsEkf22State`) pentru `AERO-SHIELD LRCV V2.0`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EsEkf22State {
    pub nominal_state: AircraftState6DoF,
    pub accel_bias: FramedVec3<BodyFrame>,
    pub gyro_bias: FramedVec3<BodyFrame>,
    pub fog_wind_est_ned: FramedVec3<NedFrameLrcv>,
    pub rho_fog_est: f64,
    pub dt_sync_est: f64,
    pub als_offset_est: SVector<f64, 2>,
    pub x_err: SVector<f64, 22>,
    pub p_cov: SMatrix<f64, 22, 22>,
}

impl Default for EsEkf22State {
    fn default() -> Self {
        Self::new_initial_lrcv()
    }
}

impl EsEkf22State {
    /// Inițializează ES-EKF 22-Stări pe panta de apropiere RWY 09 Craiova (`LRCV`).
    pub fn new_initial_lrcv() -> Self {
        Self::from_aircraft_state(AircraftState6DoF::new_initial_approach_lrcv())
    }

    /// Inițializează ES-EKF 22-Stări plecând de la o stare nominală 6-DoF dată.
    pub fn from_aircraft_state(nominal_state: AircraftState6DoF) -> Self {
        let mut p_cov = SMatrix::<f64, 22, 22>::zeros();
        for i in 0..3 {
            p_cov[(IDX_POS_NED + i, IDX_POS_NED + i)] = 2.5;
            p_cov[(IDX_VEL_BODY + i, IDX_VEL_BODY + i)] = 0.5;
            p_cov[(IDX_ATT_THETA + i, IDX_ATT_THETA + i)] = 0.002;
            p_cov[(IDX_ACCEL_BIAS + i, IDX_ACCEL_BIAS + i)] = 0.01;
            p_cov[(IDX_GYRO_BIAS + i, IDX_GYRO_BIAS + i)] = 1e-4;
            p_cov[(IDX_FOG_WIND_NED + i, IDX_FOG_WIND_NED + i)] = 4.0;
        }
        p_cov[(IDX_RHO_FOG, IDX_RHO_FOG)] = 0.0025;
        p_cov[(IDX_DT_SYNC, IDX_DT_SYNC)] = 1e-4;
        p_cov[(IDX_ALS_OFFSET, IDX_ALS_OFFSET)] = 0.5;
        p_cov[(IDX_ALS_OFFSET + 1, IDX_ALS_OFFSET + 1)] = 0.5;

        Self {
            nominal_state,
            accel_bias: FramedVec3::zero(),
            gyro_bias: FramedVec3::zero(),
            fog_wind_est_ned: FramedVec3::new(-2.8, 5.5, 0.2),
            rho_fog_est: 0.06,
            dt_sync_est: 0.0,
            als_offset_est: SVector::<f64, 2>::zeros(),
            x_err: SVector::<f64, 22>::zeros(),
            p_cov,
        }
    }

    /// Propagă starea nominală 6-DoF și matricea de covarianță $\mathbf{P} \in \mathbb{R}^{22 \times 22}$ pe pasul `dt`.
    pub fn predict_imu(&mut self, ctrl: &ControlInput6DoF, fog_hint: &FogField3D, dt: f64) {
        let safe_dt = sanitize_f64(dt, 1e-4, 0.2, 1.0 / 60.0);

        // Propagăm starea nominală 6-DoF folosind vântul estimat intern în ES-EKF
        let mut local_fog = fog_hint.clone();
        let base_w = local_fog.base_wind_ned_at_altitude(self.nominal_state.h_agl_m());
        local_fog.active_gust_delta_ned = self.fog_wind_est_ned - base_w;
        self.nominal_state.step_6dof(ctrl, &local_fog, safe_dt);

        // Construim matricea de tranziție a erorii F = I_22 + A * dt
        let mut f_mat = SMatrix::<f64, 22, 22>::identity();
        let r_b_to_n = self.nominal_state.attitude.rotation_matrix_b_to_n();
        let v_b = self.nominal_state.vel_body.raw;
        let omega_b = self.nominal_state.omega_body.raw;

        // dp_dot = R_b^n * dv_b - R_b^n * [v_b]_x * dtheta
        let v_skew = skew_symmetric(&v_b);
        let dp_dtheta = -r_b_to_n * v_skew;
        for r in 0..3 {
            for c in 0..3 {
                f_mat[(IDX_POS_NED + r, IDX_VEL_BODY + c)] = r_b_to_n[(r, c)] * safe_dt;
                f_mat[(IDX_POS_NED + r, IDX_ATT_THETA + c)] = dp_dtheta[(r, c)] * safe_dt;
                // dv_dot = -db_a
                if r == c {
                    f_mat[(IDX_VEL_BODY + r, IDX_ACCEL_BIAS + c)] = -safe_dt;
                    f_mat[(IDX_ATT_THETA + r, IDX_GYRO_BIAS + c)] = -safe_dt;
                }
            }
        }
        let w_skew = skew_symmetric(&omega_b);
        for r in 0..3 {
            for c in 0..3 {
                f_mat[(IDX_ATT_THETA + r, IDX_ATT_THETA + c)] -= w_skew[(r, c)] * safe_dt;
            }
        }

        // Zgomotul de proces Q * dt
        let mut q_proc = SMatrix::<f64, 22, 22>::zeros();
        for i in 0..3 {
            q_proc[(IDX_POS_NED + i, IDX_POS_NED + i)] = 0.02 * safe_dt;
            q_proc[(IDX_VEL_BODY + i, IDX_VEL_BODY + i)] = 0.08 * safe_dt;
            q_proc[(IDX_ATT_THETA + i, IDX_ATT_THETA + i)] = 1e-4 * safe_dt;
            q_proc[(IDX_ACCEL_BIAS + i, IDX_ACCEL_BIAS + i)] = 1e-5 * safe_dt;
            q_proc[(IDX_GYRO_BIAS + i, IDX_GYRO_BIAS + i)] = 1e-6 * safe_dt;
            q_proc[(IDX_FOG_WIND_NED + i, IDX_FOG_WIND_NED + i)] = 0.85 * safe_dt;
        }
        q_proc[(IDX_RHO_FOG, IDX_RHO_FOG)] = 1e-4 * safe_dt;
        q_proc[(IDX_DT_SYNC, IDX_DT_SYNC)] = 1e-7 * safe_dt;
        q_proc[(IDX_ALS_OFFSET, IDX_ALS_OFFSET)] = 1e-4 * safe_dt;
        q_proc[(IDX_ALS_OFFSET + 1, IDX_ALS_OFFSET + 1)] = 1e-4 * safe_dt;

        let p_pred = f_mat * self.p_cov * f_mat.transpose() + q_proc;
        self.p_cov = (&p_pred + p_pred.transpose()) * 0.5;
    }

    /// Injectează starea de eroare $\delta\mathbf{x}$ în starea nominală 6-DoF (inclusiv pe varietatea $SO(3)$)
    /// și resetează $\delta\mathbf{x} \leftarrow \mathbf{0}$.
    pub fn inject_error_and_reset(&mut self) {
        for i in 0..22 {
            if !self.x_err[i].is_finite() {
                self.x_err[i] = 0.0;
            }
        }

        let dp = Vector3::new(
            sanitize_f64(self.x_err[IDX_POS_NED], -200.0, 200.0, 0.0),
            sanitize_f64(self.x_err[IDX_POS_NED + 1], -200.0, 200.0, 0.0),
            sanitize_f64(self.x_err[IDX_POS_NED + 2], -200.0, 200.0, 0.0),
        );
        let mut next_pos = self.nominal_state.pos_ned.raw + dp;
        if next_pos.z > -0.5 {
            next_pos.z = -0.5;
        }
        self.nominal_state.pos_ned = FramedVec3::from_raw(next_pos);

        let dv = Vector3::new(
            sanitize_f64(self.x_err[IDX_VEL_BODY], -30.0, 30.0, 0.0),
            sanitize_f64(self.x_err[IDX_VEL_BODY + 1], -30.0, 30.0, 0.0),
            sanitize_f64(self.x_err[IDX_VEL_BODY + 2], -30.0, 30.0, 0.0),
        );
        let next_vel = self.nominal_state.vel_body.raw + dv;
        self.nominal_state.vel_body = FramedVec3::new(
            next_vel.x.clamp(20.0, 250.0),
            next_vel.y.clamp(-40.0, 40.0),
            next_vel.z.clamp(-40.0, 40.0),
        );

        let dtheta = SVector::<f64, 3>::new(
            self.x_err[IDX_ATT_THETA],
            self.x_err[IDX_ATT_THETA + 1],
            self.x_err[IDX_ATT_THETA + 2],
        );
        self.nominal_state.attitude.inject_error_angle(&dtheta);

        self.accel_bias.raw += Vector3::new(
            sanitize_f64(self.x_err[IDX_ACCEL_BIAS], -2.0, 2.0, 0.0),
            sanitize_f64(self.x_err[IDX_ACCEL_BIAS + 1], -2.0, 2.0, 0.0),
            sanitize_f64(self.x_err[IDX_ACCEL_BIAS + 2], -2.0, 2.0, 0.0),
        );
        self.gyro_bias.raw += Vector3::new(
            sanitize_f64(self.x_err[IDX_GYRO_BIAS], -0.5, 0.5, 0.0),
            sanitize_f64(self.x_err[IDX_GYRO_BIAS + 1], -0.5, 0.5, 0.0),
            sanitize_f64(self.x_err[IDX_GYRO_BIAS + 2], -0.5, 0.5, 0.0),
        );

        let dw_fog = Vector3::new(
            sanitize_f64(self.x_err[IDX_FOG_WIND_NED], -40.0, 40.0, 0.0),
            sanitize_f64(self.x_err[IDX_FOG_WIND_NED + 1], -40.0, 40.0, 0.0),
            sanitize_f64(self.x_err[IDX_FOG_WIND_NED + 2], -40.0, 40.0, 0.0),
        );
        self.fog_wind_est_ned = FramedVec3::from_raw(self.fog_wind_est_ned.raw + dw_fog);

        self.rho_fog_est = sanitize_f64(
            self.rho_fog_est + self.x_err[IDX_RHO_FOG],
            0.002,
            0.25,
            0.06,
        );
        self.dt_sync_est = sanitize_f64(
            self.dt_sync_est + self.x_err[IDX_DT_SYNC],
            -0.025,
            0.025,
            0.0,
        );
        self.als_offset_est[0] = sanitize_f64(
            self.als_offset_est[0] + self.x_err[IDX_ALS_OFFSET],
            -25.0,
            25.0,
            0.0,
        );
        self.als_offset_est[1] = sanitize_f64(
            self.als_offset_est[1] + self.x_err[IDX_ALS_OFFSET + 1],
            -25.0,
            25.0,
            0.0,
        );

        self.x_err = SVector::<f64, 22>::zeros();
    }

    /// Actualizare cu flux optic anizotrop folosind Matricea de Informație Fisher (Remedierea #1 & #4):
    /// $$\mathbf{R}_{opt, i}^{-1} = w_{photon, i}\left(\lambda_1 \mathbf{e}_1\mathbf{e}_1^T + \lambda_2 \mathbf{e}_2\mathbf{e}_2^T\right)$$
    /// În condiții de `Flat Whiteout` ($w_{photon} \to 0$), inovația este ponderată cu zero fără a destabiliza covarianța.
    #[allow(clippy::too_many_arguments)]
    pub fn update_optical_flow_anisotropic(
        &mut self,
        xc: f64,
        yc: f64,
        observed_flow: &SVector<f64, 2>,
        e1_dir: &SVector<f64, 2>,
        lambda1: f64,
        lambda2: f64,
        photon_weight: f64,
        alpha_fog: f64,
    ) {
        let w_p = sanitize_f64(photon_weight, 0.0, 1.0, 0.0);
        let l1 = sanitize_f64(lambda1, 0.0, 1.0e6, 0.0);
        let l2 = sanitize_f64(lambda2, 0.0, 1.0e6, 0.0);

        let pred_px = compute_coupled_optical_flow(
            xc,
            yc,
            &self.nominal_state,
            self.nominal_state.omega_body,
            self.fog_wind_est_ned,
            alpha_fog,
            self.rho_fog_est,
        );

        let raw_innov = (*observed_flow) - pred_px.total_flow;
        let innovation = if w_p <= 1e-5 || l1 <= 1e-6 {
            SVector::<f64, 2>::zeros()
        } else {
            SVector::<f64, 2>::new(
                sanitize_f64(raw_innov[0], -10.0, 10.0, 0.0),
                sanitize_f64(raw_innov[1], -10.0, 10.0, 0.0),
            )
        };

        // Construim direcțiile ortonormale e1, e2
        let norm_e1 = e1_dir.norm();
        let e1 = if norm_e1 > 1e-6 {
            (*e1_dir) / norm_e1
        } else {
            SVector::<f64, 2>::new(1.0, 0.0)
        };
        let e2 = SVector::<f64, 2>::new(-e1[1], e1[0]);

        let info_fisher =
            w_p * (l1 * (e1 * e1.transpose()) + l2 * (e2 * e2.transpose()))
                + SMatrix::<f64, 2, 2>::identity() * 1e-4;
        let r_meas = info_fisher
            .try_inverse()
            .unwrap_or_else(|| SMatrix::<f64, 2, 2>::identity() * 1.0e4);

        let a_tr = translation_flow_matrix_a(xc, yc);
        let b_rot = rotation_flow_matrix_b(xc, yc);
        let r_n_to_b = self.nominal_state.attitude.rotation_matrix_n_to_b();

        let scale_v = (1.0 - pred_px.alpha_fog) * pred_px.rho_ground
            + pred_px.alpha_fog * pred_px.rho_fog;
        let h_v = a_tr * scale_v;
        let h_w_fog = -a_tr * r_n_to_b * (pred_px.alpha_fog * pred_px.rho_fog);
        let h_bg = -b_rot;

        let mut h_jac = SMatrix::<f64, 2, 22>::zeros();
        for r in 0..2 {
            for c in 0..3 {
                h_jac[(r, IDX_VEL_BODY + c)] = h_v[(r, c)];
                h_jac[(r, IDX_FOG_WIND_NED + c)] = h_w_fog[(r, c)];
                h_jac[(r, IDX_GYRO_BIAS + c)] = h_bg[(r, c)];
            }
        }
        let v_rel_fog = self.nominal_state.vel_body.raw
            - self
                .nominal_state
                .attitude
                .ned_to_body(self.fog_wind_est_ned)
                .raw;
        let h_rho = a_tr * v_rel_fog * pred_px.alpha_fog;
        h_jac[(0, IDX_RHO_FOG)] = h_rho[0];
        h_jac[(1, IDX_RHO_FOG)] = h_rho[1];

        kalman_joseph_update(&mut self.x_err, &mut self.p_cov, &innovation, &h_jac, &r_meas);
        self.inject_error_and_reset();
    }

    /// Actualizează ES-EKF cu măsurătoarea cuplată de viteză la sol, viteza ceții și scara $\hat{\rho}_{fog}$
    /// ponderată cu incertitudinea heteroscedastică $\boldsymbol{\sigma}_v, \boldsymbol{\sigma}_w$ (Remedierea #3 & #9).
    pub fn update_velocity_and_fog_ai(
        &mut self,
        vel_body_meas: FramedVec3<BodyFrame>,
        fog_wind_ned_meas: FramedVec3<NedFrameLrcv>,
        rho_fog_meas: f64,
        sigma_v: [f64; 3],
        sigma_w: [f64; 3],
    ) {
        let mut innovation = SVector::<f64, 7>::zeros();
        let dv = vel_body_meas.raw - self.nominal_state.vel_body.raw;
        let dw = fog_wind_ned_meas.raw - self.fog_wind_est_ned.raw;
        let drho = sanitize_f64(rho_fog_meas, 0.002, 0.25, self.rho_fog_est) - self.rho_fog_est;

        for i in 0..3 {
            innovation[i] = sanitize_f64(dv[i], -30.0, 30.0, 0.0);
            innovation[3 + i] = sanitize_f64(dw[i], -40.0, 40.0, 0.0);
        }
        innovation[6] = sanitize_f64(drho, -0.2, 0.2, 0.0);

        let mut h_jac = SMatrix::<f64, 7, 22>::zeros();
        let mut r_meas = SMatrix::<f64, 7, 7>::identity() * 1e-3;
        for i in 0..3 {
            h_jac[(i, IDX_VEL_BODY + i)] = 1.0;
            h_jac[(3 + i, IDX_FOG_WIND_NED + i)] = 1.0;
            let sv = sanitize_f64(sigma_v[i], 0.05, 50.0, 0.35);
            let sw = sanitize_f64(sigma_w[i], 0.05, 50.0, 0.45);
            r_meas[(i, i)] = sv * sv;
            r_meas[(3 + i, 3 + i)] = sw * sw;
        }
        h_jac[(6, IDX_RHO_FOG)] = 1.0;
        r_meas[(6, 6)] = 1e-4;

        kalman_joseph_update(&mut self.x_err, &mut self.p_cov, &innovation, &h_jac, &r_meas);
        self.inject_error_and_reset();
    }

    /// Actualizare cu altimetrul barometric / radio-altimetrul pentru constrângerea axei verticale Down ($z_{ned} = -h_{AGL}$).
    pub fn update_baro_altitude(&mut self, h_agl_baro_m: f64, var_baro: f64) {
        let safe_h = sanitize_f64(h_agl_baro_m, 0.5, 10_000.0, self.nominal_state.h_agl_m());
        let z_meas = -safe_h;
        let innov = SVector::<f64, 1>::new(sanitize_f64(
            z_meas - self.nominal_state.pos_ned.z(),
            -100.0,
            100.0,
            0.0,
        ));
        let mut h_jac = SMatrix::<f64, 1, 22>::zeros();
        h_jac[(0, IDX_POS_NED + 2)] = 1.0;
        let r_meas = SMatrix::<f64, 1, 1>::new(sanitize_f64(var_baro, 1e-3, 100.0, 0.25));

        kalman_joseph_update(&mut self.x_err, &mut self.p_cov, &innov, &h_jac, &r_meas);
        self.inject_error_and_reset();
    }

    /// Actualizare cu ancora fotometrică a balizajului luminos ALS RWY 09 în banda SWIR/LWIR (Remedierea #2 & #12).
    /// Reduce eroarea de poziție la $< 0.8\text{ m}$ pe pragul pistei RWY 09 Craiova.
    pub fn update_als_runway_anchor(
        &mut self,
        pos_ned_als_anchor: FramedVec3<NedFrameLrcv>,
        als_bars_count: usize,
        photonic_certainty: f64,
    ) {
        if als_bars_count == 0 || photonic_certainty <= 0.02 {
            return;
        }
        let cert = sanitize_f64(photonic_certainty, 0.02, 1.0, 0.5);
        let bars_factor = (als_bars_count as f64).clamp(1.0, 12.0);
        let meas_var = (0.35 / (cert * bars_factor)).clamp(0.015, 4.0);

        let dp = pos_ned_als_anchor.raw - self.nominal_state.pos_ned.raw;
        let innov = SVector::<f64, 3>::new(
            sanitize_f64(dp.x, -80.0, 80.0, 0.0),
            sanitize_f64(dp.y, -80.0, 80.0, 0.0),
            sanitize_f64(dp.z, -40.0, 40.0, 0.0),
        );

        let mut h_jac = SMatrix::<f64, 3, 22>::zeros();
        h_jac[(0, IDX_POS_NED)] = 1.0;
        h_jac[(1, IDX_POS_NED + 1)] = 1.0;
        h_jac[(2, IDX_POS_NED + 2)] = 1.0;
        h_jac[(0, IDX_ALS_OFFSET)] = 0.05;
        h_jac[(1, IDX_ALS_OFFSET + 1)] = 0.05;

        let r_meas = SMatrix::<f64, 3, 3>::identity() * meas_var;
        kalman_joseph_update(&mut self.x_err, &mut self.p_cov, &innov, &h_jac, &r_meas);
        self.inject_error_and_reset();
    }

    /// Verifică faptul că matricea de covarianță $\mathbf{P} \in \mathbb{R}^{22 \times 22}$ este finită,
    /// simetrică și pozitiv-definită (toate elementele diagonale $> 0$).
    pub fn is_covariance_spd_and_finite(&self) -> bool {
        for r in 0..22 {
            if !self.p_cov[(r, r)].is_finite() || self.p_cov[(r, r)] <= 0.0 {
                return false;
            }
            for c in 0..22 {
                if !self.p_cov[(r, c)].is_finite() {
                    return false;
                }
                if (self.p_cov[(r, c)] - self.p_cov[(c, r)]).abs() > 1e-7 {
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_joseph_covariance_symmetry_and_spd_under_100_flat_whiteout_steps() {
        let mut ekf = EsEkf22State::new_initial_lrcv();
        let fog = FogField3D::default_lrcv_cat3c();
        let ctrl = ControlInput6DoF::from_continuous_rates_deg(5.0, -1.0, 2.0, 0.5);

        for step in 0..100 {
            ekf.predict_imu(&ctrl, &fog, 1.0 / 60.0);
            // Condiție extremă Flat Whiteout 100% ceață: lambda1 = 0.0, photon_weight = 0.0
            let e1 = SVector::<f64, 2>::new(1.0, 0.0);
            let noisy_flow = SVector::<f64, 2>::new(0.1 * (step as f64).sin(), -0.08);
            ekf.update_optical_flow_anisotropic(0.2, -0.1, &noisy_flow, &e1, 0.0, 0.0, 0.0, 1.0);
            ekf.update_baro_altitude(ekf.nominal_state.h_agl_m(), 0.2);
        }

        assert!(ekf.is_covariance_spd_and_finite());
    }

    #[test]
    fn test_convergence_below_0_8m_at_lrcv_rwy09_threshold() {
        let mut truth = AircraftState6DoF::new_initial_approach_lrcv();
        let mut ekf = EsEkf22State::from_aircraft_state(truth.clone());
        // Perturbăm inițial estimatorul cu o eroare de 4.5 metri
        ekf.nominal_state.pos_ned += FramedVec3::new(3.0, -3.2, 1.0);

        let fog = FogField3D::default_lrcv_cat3c();
        let ctrl = ControlInput6DoF::zero();

        for _ in 0..60 {
            truth.step_6dof(&ctrl, &fog, 1.0 / 60.0);
            ekf.predict_imu(&ctrl, &fog, 1.0 / 60.0);
            let true_wind = fog.wind_ned_at_altitude(truth.h_agl_m());
            ekf.update_velocity_and_fog_ai(
                truth.vel_body,
                true_wind,
                fog.nominal_rho_fog(),
                [0.2, 0.2, 0.2],
                [0.3, 0.3, 0.3],
            );
            ekf.update_als_runway_anchor(truth.pos_ned, 6, 0.85);
            ekf.update_baro_altitude(truth.h_agl_m(), 0.15);
        }

        let pos_err = (ekf.nominal_state.pos_ned.raw - truth.pos_ned.raw).norm();
        assert!(
            pos_err < 0.8,
            "Expected position error < 0.8m at LRCV RWY 09, got {}m",
            pos_err
        );
    }
}
