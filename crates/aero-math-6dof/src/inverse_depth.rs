//! Singularity-Free Inverse-Depth Optical Flow ($\rho_{ground} = 1 / Z_{ground} \ge 0$),
//! Dual-Anchored Fog Scale Estimation ($\hat{\rho}_{fog}$), and 6-DoF Rotational Decoupling.

use crate::frames::{sanitize_f64, BodyFrame, FramedVec3, NedFrameLrcv};
use crate::kinematics_6dof::AircraftState6DoF;
use nalgebra::{SMatrix, SVector, Vector3};
use serde::{Deserialize, Serialize};

/// Calculează inversa adâncimii planului solului $\rho_{ground}(x_c, y_c) = 1 / Z_{ground} \ge 0$
/// pentru coordonatele optice normalizate $(x_c, y_c) \in [-1, 1]^2$.
///
/// **Garanție Numerică Absolută (Remedierea #11)**:
/// Deasupra liniei orizontului (cer/nori), proiecția pe versorul Down este $\le 0$,
/// iar funcția returnează exact `0.0` — nu apare niciodată împărțire la zero (`NaN` / `Inf`).
#[inline(always)]
pub fn compute_inverse_depth_ground(
    xc: f64,
    yc: f64,
    roll_phi: f64,
    pitch_theta: f64,
    h_agl_meters: f64,
) -> f64 {
    let xc_s = sanitize_f64(xc, -4.0, 4.0, 0.0);
    let yc_s = sanitize_f64(yc, -4.0, 4.0, 0.0);
    let phi = sanitize_f64(roll_phi, -10.0, 10.0, 0.0);
    let theta = sanitize_f64(pitch_theta, -10.0, 10.0, 0.0);

    let (s_phi, c_phi) = phi.sin_cos();
    let (s_theta, c_theta) = theta.sin_cos();

    // Proiecția razei vizuale pe versorul Down în reperul aeronavei
    let ray_dot_down = c_phi * c_theta - xc_s * s_theta + yc_s * s_phi * c_theta;
    let safe_h = sanitize_f64(h_agl_meters, 0.5, 20_000.0, 0.5).max(0.5);

    (ray_dot_down.max(0.0)) / safe_h
}

/// Fuzionează bayesian cele două ancore fizice pentru inversa adâncimii efective a ceții $\hat{\rho}_{fog} = 1 / \hat{Z}_{fog}$:
/// 1. Ancora Optică Koschmieder: $\hat{\rho}_{fog, opt} = \hat{\beta}_{ext} \approx 3.0 / V_{MOR}$
/// 2. Ancora Inerțială Activă (*Active Inertial Scale Probing*): $\hat{\rho}_{fog, inert} = \|\Delta\dot{\mathbf{u}}_{fog, trans}\| / \|\mathbf{A}_{tr}\Delta\mathbf{v}_b\|$.
#[inline(always)]
pub fn fuse_fog_inverse_depth(
    beta_ext_koschmieder: f64,
    delta_flow_fog_norm: f64,
    delta_v_body_proj_norm: f64,
    var_optical: f64,
    var_inertial: f64,
) -> f64 {
    let rho_opt = sanitize_f64(beta_ext_koschmieder, 0.002, 0.25, 0.06).clamp(0.002, 0.25);
    let df_norm = sanitize_f64(delta_flow_fog_norm, 0.0, 1.0e4, 0.0);
    let dv_norm = sanitize_f64(delta_v_body_proj_norm, 0.0, 1.0e4, 0.0);
    let v_opt = sanitize_f64(var_optical, 1e-9, 1.0e4, 1e-3);
    let v_in = sanitize_f64(var_inertial, 1e-9, 1.0e4, 1e-3);

    if dv_norm > 0.35 {
        let rho_inert = (df_norm / dv_norm).clamp(0.002, 0.25);
        let w_opt = v_in / (v_opt + v_in + 1e-9);
        (w_opt * rho_opt + (1.0 - w_opt) * rho_inert).clamp(0.002, 0.25)
    } else {
        rho_opt
    }
}

/// Matricea cinematică de proiecție perspectivă a translației $\mathbf{A}_{tr}(x_c, y_c) \in \mathbb{R}^{2 \times 3}$:
/// $$\mathbf{A}_{tr}(x_c, y_c) = \begin{bmatrix} -1 & 0 & x_c \\ 0 & -1 & y_c \end{bmatrix}$$
#[inline(always)]
pub fn translation_flow_matrix_a(xc: f64, yc: f64) -> SMatrix<f64, 2, 3> {
    let x = sanitize_f64(xc, -4.0, 4.0, 0.0);
    let y = sanitize_f64(yc, -4.0, 4.0, 0.0);
    SMatrix::<f64, 2, 3>::new(
        -1.0, 0.0, x,
        0.0, -1.0, y,
    )
}

/// Matricea cinematică de proiecție perspectivă a rotației 3D $\mathbf{B}_{rot}(x_c, y_c) \in \mathbb{R}^{2 \times 3}$:
/// $$\mathbf{B}_{rot}(x_c, y_c) = \begin{bmatrix} x_c y_c & -(1 + x_c^2) & y_c \\ 1 + y_c^2 & -x_c y_c & -x_c \end{bmatrix}$$
#[inline(always)]
pub fn rotation_flow_matrix_b(xc: f64, yc: f64) -> SMatrix<f64, 2, 3> {
    let x = sanitize_f64(xc, -4.0, 4.0, 0.0);
    let y = sanitize_f64(yc, -4.0, 4.0, 0.0);
    SMatrix::<f64, 2, 3>::new(
        x * y, -(1.0 + x * x), y,
        1.0 + y * y, -x * y, -x,
    )
}

/// Pseudo-inversa analitică Moore-Penrose $\mathbf{A}_{tr}^\dagger(x_c, y_c) = \mathbf{A}_{tr}^T (\mathbf{A}_{tr}\mathbf{A}_{tr}^T)^{-1} \in \mathbb{R}^{3 \times 2}$.
/// Numitorul este $1 + x_c^2 + y_c^2 \ge 1.0$, deci nu are niciodată singularități!
#[inline(always)]
pub fn pseudo_inverse_a_tr(xc: f64, yc: f64) -> SMatrix<f64, 3, 2> {
    let x = sanitize_f64(xc, -4.0, 4.0, 0.0);
    let y = sanitize_f64(yc, -4.0, 4.0, 0.0);
    let inv_denom = 1.0 / (1.0 + x * x + y * y);
    SMatrix::<f64, 3, 2>::new(
        -(1.0 + y * y) * inv_denom, (x * y) * inv_denom,
        (x * y) * inv_denom, -(1.0 + x * x) * inv_denom,
        x * inv_denom, y * inv_denom,
    )
}

/// Descompunerea completă a fluxului optic observat la un pixel $(x_c, y_c)$ conform ecuației (3.2).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CoupledOpticalFlowPixel {
    pub xc: f64,
    pub yc: f64,
    pub rho_ground: f64,
    pub rho_fog: f64,
    pub alpha_fog: f64,
    pub ground_trans_flow: SVector<f64, 2>,
    pub fog_trans_flow: SVector<f64, 2>,
    pub rotational_flow: SVector<f64, 2>,
    pub total_flow: SVector<f64, 2>,
    pub derotated_flow: SVector<f64, 2>,
}

/// Calculează fluxul optic cuplat 6-DoF (Sol + Ceață Dinamică + Rotație 3D) la pixelul $(x_c, y_c)$
/// exact conform ecuației (3.2) din `model_matematic_6dof_ceata.md`:
/// $$\dot{\mathbf{u}}(x_c, y_c) = (1 - \alpha_{fog})\rho_{ground}\mathbf{A}_{tr}\mathbf{v}_b + \alpha_{fog}\hat{\rho}_{fog}\mathbf{A}_{tr}[\mathbf{v}_b - (\mathbf{R}_b^n)^T\mathbf{w}_{fog}] + \mathbf{B}_{rot}\bar{\boldsymbol{\omega}}_k$$
pub fn compute_coupled_optical_flow(
    xc: f64,
    yc: f64,
    state: &AircraftState6DoF,
    omega_preintegrated: FramedVec3<BodyFrame>,
    fog_wind_ned: FramedVec3<NedFrameLrcv>,
    alpha_fog: f64,
    rho_fog: f64,
) -> CoupledOpticalFlowPixel {
    let xc_s = sanitize_f64(xc, -2.0, 2.0, 0.0);
    let yc_s = sanitize_f64(yc, -2.0, 2.0, 0.0);
    let alpha = sanitize_f64(alpha_fog, 0.0, 1.0, 0.5);
    let rho_f = sanitize_f64(rho_fog, 0.002, 0.25, 0.06);

    let (roll_phi, pitch_theta, _) = state.attitude.to_euler_rad();
    let rho_g = compute_inverse_depth_ground(xc_s, yc_s, roll_phi, pitch_theta, state.h_agl_m());

    let a_tr = translation_flow_matrix_a(xc_s, yc_s);
    let b_rot = rotation_flow_matrix_b(xc_s, yc_s);

    let v_b = state.vel_body.raw;
    let w_fog_body = state.attitude.ned_to_body(fog_wind_ned).raw;
    let v_rel_fog_body = v_b - w_fog_body;

    let ground_trans_flow = a_tr * v_b * ((1.0 - alpha) * rho_g);
    let fog_trans_flow = a_tr * v_rel_fog_body * (alpha * rho_f);
    let rotational_flow = b_rot * omega_preintegrated.raw;

    let derotated_flow = ground_trans_flow + fog_trans_flow;
    let total_flow = derotated_flow + rotational_flow;

    CoupledOpticalFlowPixel {
        xc: xc_s,
        yc: yc_s,
        rho_ground: rho_g,
        rho_fog: rho_f,
        alpha_fog: alpha,
        ground_trans_flow,
        fog_trans_flow,
        rotational_flow,
        total_flow,
        derotated_flow,
    }
}

/// Izolează variația vitezei ceții $\widehat{\Delta\mathbf{w}}_{fog}$ în NED dintr-o variație observată a fluxului optic
/// la pixelul $(x_c, y_c)$ după eliminarea rotației 3D $\mathbf{B}_{rot}\bar{\boldsymbol{\omega}}_k$ și a contribuției solului/translației aeronavei.
pub fn isolate_fog_velocity_from_flow(
    xc: f64,
    yc: f64,
    observed_total_flow: &SVector<f64, 2>,
    state: &AircraftState6DoF,
    omega_preintegrated: FramedVec3<BodyFrame>,
    alpha_fog: f64,
    rho_fog: f64,
) -> FramedVec3<NedFrameLrcv> {
    let xc_s = sanitize_f64(xc, -2.0, 2.0, 0.0);
    let yc_s = sanitize_f64(yc, -2.0, 2.0, 0.0);
    let alpha = sanitize_f64(alpha_fog, 0.05, 1.0, 0.5);
    let rho_f = sanitize_f64(rho_fog, 0.002, 0.25, 0.06);

    let (roll_phi, pitch_theta, _) = state.attitude.to_euler_rad();
    let rho_g = compute_inverse_depth_ground(xc_s, yc_s, roll_phi, pitch_theta, state.h_agl_m());

    let a_tr = translation_flow_matrix_a(xc_s, yc_s);
    let b_rot = rotation_flow_matrix_b(xc_s, yc_s);
    let a_dag = pseudo_inverse_a_tr(xc_s, yc_s);

    // 1. Derotare exactă pe cuaternioni / rate pre-integrate
    let u_derot = (*observed_total_flow) - b_rot * omega_preintegrated.raw;

    // 2. Scăderea fluxului solului și al translației proprii a aeronavei prin ceață
    let u_ground = a_tr * state.vel_body.raw * ((1.0 - alpha) * rho_g);
    let u_aircraft_in_fog = a_tr * state.vel_body.raw * (alpha * rho_f);

    // Fluxul indus strict de mișcarea masei de ceață: -alpha * rho_fog * A_tr * (R_b^n)^T * w_fog
    let u_fog_wind_only = u_derot - u_ground - u_aircraft_in_fog;

    let scale_denom = (alpha * rho_f).max(1e-4);
    let w_fog_body_est: Vector3<f64> = -(a_dag * (u_fog_wind_only / scale_denom));

    state.attitude.body_to_ned(FramedVec3::from_raw(w_fog_body_est))
}

/// Reconstruiește exact în 3D vectorul vitezei ceții $\hat{\mathbf{w}}_{fog} \in \mathbb{R}^3$ (în NED)
/// prin metoda celor mai mici pătrate regularizată pe un câmp de pixeli $(x_c^{(i)}, y_c^{(i)})$.
pub fn isolate_fog_velocity_field_lsq(
    samples: &[(f64, f64, SVector<f64, 2>)],
    state: &AircraftState6DoF,
    omega_preintegrated: FramedVec3<BodyFrame>,
    alpha_fog: f64,
    rho_fog: f64,
) -> FramedVec3<NedFrameLrcv> {
    if samples.is_empty() {
        return FramedVec3::zero();
    }
    let alpha = sanitize_f64(alpha_fog, 0.05, 1.0, 0.5);
    let rho_f = sanitize_f64(rho_fog, 0.002, 0.25, 0.06);
    let scale = (alpha * rho_f).max(1e-4);
    let (roll_phi, pitch_theta, _) = state.attitude.to_euler_rad();

    let mut ata = SMatrix::<f64, 3, 3>::identity() * 1e-6;
    let mut atb = Vector3::<f64>::zeros();

    for &(xc, yc, ref flow_obs) in samples {
        let xc_s = sanitize_f64(xc, -2.0, 2.0, 0.0);
        let yc_s = sanitize_f64(yc, -2.0, 2.0, 0.0);
        let rho_g = compute_inverse_depth_ground(xc_s, yc_s, roll_phi, pitch_theta, state.h_agl_m());
        let a_tr = translation_flow_matrix_a(xc_s, yc_s);
        let b_rot = rotation_flow_matrix_b(xc_s, yc_s);

        let u_derot = (*flow_obs) - b_rot * omega_preintegrated.raw;
        let u_ground = a_tr * state.vel_body.raw * ((1.0 - alpha) * rho_g);
        let u_aircraft_in_fog = a_tr * state.vel_body.raw * scale;
        let rhs = -(u_derot - u_ground - u_aircraft_in_fog) / scale;

        ata += a_tr.transpose() * a_tr;
        atb += a_tr.transpose() * rhs;
    }

    let w_fog_body = if let Some(chol) = ata.cholesky() {
        chol.solve(&atb)
    } else {
        ata.svd(true, true)
            .solve(&atb, 1e-9)
            .unwrap_or_else(|_| Vector3::zeros())
    };

    state.attitude.body_to_ned(FramedVec3::from_raw(w_fog_body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    #[test]
    fn test_inverse_depth_zero_nan_inf_at_extreme_attitudes() {
        let angles = [0.0, FRAC_PI_2, -FRAC_PI_2, 1.57079632679, -1.57079632679];
        let heights = [0.0, -10.0, 0.5, 50.0, f64::NAN, f64::INFINITY];
        for &phi in &angles {
            for &theta in &angles {
                for &h in &heights {
                    for &xc in &[-1.0, 0.0, 1.0] {
                        for &yc in &[-1.0, 0.0, 1.0] {
                            let rho = compute_inverse_depth_ground(xc, yc, phi, theta, h);
                            assert!(rho.is_finite(), "rho must be finite, got {}", rho);
                            assert!(rho >= 0.0, "rho must be non-negative, got {}", rho);
                        }
                    }
                }
            }
        }
        // La pitch = +90 deg (privire verticală în sus spre cer), rho_ground == 0.0 exact!
        let rho_sky = compute_inverse_depth_ground(0.2, -0.3, 0.0, FRAC_PI_2, 80.0);
        assert_eq!(rho_sky, 0.0);
    }

    #[test]
    fn test_dual_anchor_koschmieder_and_inertial_probing() {
        let beta_ext = 3.0 / 50.0; // 0.06 m^-1
        // Fără manevră de accelerare (dv <= 0.35), folosește ancora Koschmieder
        let rho_static = fuse_fog_inverse_depth(beta_ext, 0.01, 0.1, 1e-4, 1e-4);
        assert!((rho_static - 0.06).abs() < 1e-9);

        // Cu manevră activă de accelerare (dv = 4.0 m/s, df = 0.32 rad/s => rho_inert = 0.08 m^-1)
        let rho_probed = fuse_fog_inverse_depth(beta_ext, 0.32, 4.0, 1e-3, 1e-3);
        assert!((rho_probed - 0.07).abs() < 1e-6);
    }

    #[test]
    fn test_optical_flow_derotation_and_3d_fog_isolation_under_roll_and_yaw() {
        let mut state = AircraftState6DoF::new_initial_approach_lrcv();
        state.apply_maneuver_step_deg(18.0, -4.0, 28.0, 5.0);

        let omega_pre = FramedVec3::<BodyFrame>::new(0.65, -0.22, 0.48);
        let true_fog_wind = FramedVec3::<NedFrameLrcv>::new(-6.5, 14.2, 0.8);
        let alpha_fog = 0.85;
        let rho_fog = 0.06;

        let grid_coords = [
            (-0.6, -0.5),
            (0.6, -0.5),
            (-0.6, 0.5),
            (0.6, 0.5),
            (0.0, 0.0),
        ];
        let mut samples = Vec::new();
        for &(xc, yc) in &grid_coords {
            let px = compute_coupled_optical_flow(
                xc,
                yc,
                &state,
                omega_pre,
                true_fog_wind,
                alpha_fog,
                rho_fog,
            );
            // Verificăm că derotația elimină exact rotational_flow
            let diff_derot = (px.total_flow - px.rotational_flow - px.derotated_flow).norm();
            assert!(diff_derot < 1e-12);
            samples.push((xc, yc, px.total_flow));
        }

        let est_fog_wind =
            isolate_fog_velocity_field_lsq(&samples, &state, omega_pre, alpha_fog, rho_fog);
        assert!(
            (est_fog_wind.raw - true_fog_wind.raw).norm() < 1e-4,
            "Expected {:?}, got {:?}",
            true_fog_wind,
            est_fog_wind
        );
    }
}
