//! Forward-Backward Optical Flow with Cycle Consistency, IMU Exposure Pre-Integration with
//! $\hat{\Delta t}_{sync}$ Compensation (Remediation #6), Exact 3D Rotational Derotation $\mathbf{B}_{rot}(x_c, y_c)$,
//! Anisotropic Fisher Weighted Least Squares Ground/Fog Velocity Separation, and
//! Optical Gust Lookahead ($35\text{–}80\text{ m}$ Ahead) with Autopilot Crab Angle $\psi_{crab}$ (Remediation #5).

use crate::structure_tensor_3f::{PatchStructureTensor, RingBuffer3Frames};
use nalgebra::{Matrix2, Matrix2x3, Matrix3, Vector2, Vector3};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

const EPS_F64: f64 = 1e-9;

/// Single IMU gyroscope measurement sample $\boldsymbol{\omega}_{IMU}(\tau)$ for exposure pre-integration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ImuAngularSample {
    /// Sample timestamp $\tau$ (s).
    pub timestamp_s: f64,
    /// Raw angular rate vector $[p, q, r]^T$ in camera/body axes (rad/s).
    pub omega_radps: Vector3<f64>,
}

/// Configuration for Optical Flow, 3D Derotation, and Optical Gust Lookahead.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OpticalFlowDerotConfig {
    /// Patch size in pixels (matches `StructureTensorConfig::patch_size_px`, default `8`).
    pub patch_size_px: usize,
    /// Frame exposure interval $\Delta t_k = t_k - t_{k-1}$ (s, e.g. `1.0 / 60.0`).
    pub dt_frame_s: f64,
    /// Estimated IMU-to-Camera time synchronization offset $\hat{\Delta t}_{sync}$ (s, e.g. `0.005` = 5ms).
    pub dt_sync_s: f64,
    /// Lookahead gust detection distance ahead of the aircraft $L_{lookahead} \in [35, 80]\text{ m}$ (default `55.0`).
    pub lookahead_distance_m: f64,
    /// Wind shear magnitude threshold $\|\Delta\mathbf{w}_{gust}\|$ (m/s) that triggers a Gust Lookahead alert (default `4.5`).
    pub gust_alert_threshold_mps: f64,
    /// Tikhonov regularization parameter for WLS velocity inversion.
    pub wls_regularization: f64,
}

impl Default for OpticalFlowDerotConfig {
    fn default() -> Self {
        Self {
            patch_size_px: 8,
            dt_frame_s: 1.0 / 60.0,
            dt_sync_s: 0.0,
            lookahead_distance_m: 55.0,
            gust_alert_threshold_mps: 4.5,
            wls_regularization: 1e-6,
        }
    }
}

/// Per-patch raw and derotated optical flow measurement.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PatchOpticalFlow {
    /// Patch index.
    pub patch_index: usize,
    /// Normalized camera coordinates $(x_c, y_c)$.
    pub center_norm: (f64, f64),
    /// Forward optical flow rate $\dot{\mathbf{u}}_{fwd}(x_c, y_c)$ (normalized rad/s).
    pub u_dot_fwd: Vector2<f64>,
    /// Backward optical flow rate $\dot{\mathbf{u}}_{bwd}(x_c, y_c)$ (normalized rad/s).
    pub u_dot_bwd: Vector2<f64>,
    /// Forward-backward cycle consistency error $e_{cycle} = \|\dot{\mathbf{u}}_{fwd} + \dot{\mathbf{u}}_{bwd}\|_2$.
    pub e_cycle: f64,
    /// Derotated translational optical flow $\dot{\mathbf{u}}_{trans} = \dot{\mathbf{u}}_{fwd} - \mathbf{B}_{rot}\bar{\boldsymbol{\omega}}_k$.
    pub u_dot_trans: Vector2<f64>,
    /// Representative ground inverse depth $\rho_{ground}$ at this patch ($\text{m}^{-1}$).
    pub rho_ground: f64,
    /// Representative ALS bloom mask value $M_{bloom} \in [0, 1]$ at this patch.
    pub bloom_mask: f64,
    /// True if this patch belongs to the sub-horizon ground-anchored layer; false if fog/lookahead layer.
    pub is_ground_anchored: bool,
}

/// Output of the Decoupled Ground/Fog Velocity Estimator and Optical Gust Lookahead (Remediation #5).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DecoupledMotionEstimate {
    /// Pre-integrated and sync-compensated angular velocity $\bar{\boldsymbol{\omega}}_k$ (rad/s).
    pub preintegrated_omega_radps: Vector3<f64>,
    /// Estimated aircraft velocity relative to the ground in camera/body frame $\hat{\mathbf{v}}_{b, ground}$ (m/s).
    pub v_ground_body_mps: Vector3<f64>,
    /// Posterior Fisher Information matrix (inverse covariance $\mathbf{P}_{v, opt}^{-1}$) of $\hat{\mathbf{v}}_{b, ground}$.
    pub v_ground_fisher_info: Matrix3<f64>,
    /// Estimated ambient fog velocity in camera/body frame $\hat{\mathbf{w}}_{fog, body}$ (m/s).
    pub w_fog_body_mps: Vector3<f64>,
    /// Estimated lookahead fog/gust velocity at $L_{lookahead} = 35\text{–}80\text{ m}$ ahead $\hat{\mathbf{w}}_{gust, lookahead}$ (m/s).
    pub w_gust_lookahead_mps: Vector3<f64>,
    /// Detected wind-shear gust delta $\Delta\hat{\mathbf{w}}_{gust} = \hat{\mathbf{w}}_{gust, lookahead} - \hat{\mathbf{w}}_{fog, body}$ (m/s).
    pub gust_shear_delta_mps: Vector3<f64>,
    /// True if $\|\Delta\hat{\mathbf{w}}_{gust}\|_2 \ge \text{gust\_alert\_threshold\_mps}$.
    pub gust_warning_active: bool,
    /// Feed-forward gust lead time $\Delta t_{lead} = \frac{L_{lookahead}}{\max(u + \|\mathbf{w}_{fog}\|, 1.0)}$ (s).
    pub gust_lead_time_s: f64,
    /// Recommended Autopilot Crab Angle $\psi_{crab}(t) = \psi_{LRCV}(t) - \arcsin(\hat{w}_{fog, cross} / \|\hat{\mathbf{v}}_{air}\|)$ (rad).
    pub crab_angle_rad: f64,
    /// Number of valid ground-anchored patches used in WLS.
    pub active_ground_patches: usize,
    /// Number of valid fog-layer patches used in WLS.
    pub active_fog_patches: usize,
}

/// Exact 3D rotational optical flow Jacobian $\mathbf{B}_{rot}(x_c, y_c)$ (Remediation #6):
///
/// $$\mathbf{B}_{rot}(x_c, y_c) = \begin{bmatrix} x_c y_c & -(1 + x_c^2) & y_c \\ 1 + y_c^2 & -x_c y_c & -x_c \end{bmatrix}$$
#[inline]
pub fn compute_rotation_jacobian_b_rot(x_c: f64, y_c: f64) -> Matrix2x3<f64> {
    Matrix2x3::new(
        x_c * y_c,
        -(1.0 + x_c * x_c),
        y_c,
        1.0 + y_c * y_c,
        -x_c * y_c,
        -x_c,
    )
}

/// Exact perspective translational optical flow Jacobian $\mathbf{A}_{tr}(x_c, y_c)$:
///
/// $$\mathbf{A}_{tr}(x_c, y_c) = \begin{bmatrix} -1 & 0 & x_c \\ 0 & -1 & y_c \end{bmatrix}$$
#[inline]
pub fn compute_translation_jacobian_a_tr(x_c: f64, y_c: f64) -> Matrix2x3<f64> {
    Matrix2x3::new(-1.0, 0.0, x_c, 0.0, -1.0, y_c)
}

/// Pre-integrates IMU angular velocity over the exposure window $[t_{k-1}, t_k]$ and applies
/// first-order time-synchronization compensation $\hat{\Delta t}_{sync}$ (Remediation #6):
///
/// $$\bar{\boldsymbol{\omega}}_k = \frac{1}{\Delta t_k}\int_{t_{k-1}+\hat{\Delta t}_{sync}}^{t_k+\hat{\Delta t}_{sync}}(\boldsymbol{\omega}_{IMU}(\tau)-\hat{\mathbf{b}}_g)d\tau + \dot{\boldsymbol{\omega}}_k\hat{\Delta t}_{sync}$$
pub fn preintegrate_imu_angular_rate(
    imu_samples: &[ImuAngularSample],
    gyro_bias_radps: &Vector3<f64>,
    dt_sync_s: f64,
) -> Vector3<f64> {
    if imu_samples.is_empty() {
        return Vector3::zeros();
    }
    if imu_samples.len() == 1 {
        return imu_samples[0].omega_radps - gyro_bias_radps;
    }

    let mut integral = Vector3::zeros();
    let mut total_dt = 0.0_f64;

    for pair in imu_samples.windows(2) {
        let s0 = &pair[0];
        let s1 = &pair[1];
        let dt = (s1.timestamp_s - s0.timestamp_s).max(0.0);
        if dt > 0.0 {
            let w0 = s0.omega_radps - gyro_bias_radps;
            let w1 = s1.omega_radps - gyro_bias_radps;
            integral += 0.5 * (w0 + w1) * dt;
            total_dt += dt;
        }
    }

    let mean_omega = if total_dt > EPS_F64 {
        integral / total_dt
    } else {
        imu_samples[0].omega_radps - gyro_bias_radps
    };

    // Estimate angular acceleration \dot{\omega}_k across the frame exposure
    let first = imu_samples[0];
    let last = imu_samples[imu_samples.len() - 1];
    let span_dt = (last.timestamp_s - first.timestamp_s).max(EPS_F64);
    let omega_dot = if span_dt > 1e-5 {
        (last.omega_radps - first.omega_radps) / span_dt
    } else {
        Vector3::zeros()
    };

    mean_omega + omega_dot * dt_sync_s
}

/// Derotates a raw optical flow vector $\dot{\mathbf{u}}_{raw}(x_c, y_c)$ using the pre-integrated
/// angular velocity $\bar{\boldsymbol{\omega}}_k$:
///
/// $$\dot{\mathbf{u}}_{trans}(x_c, y_c) = \dot{\mathbf{u}}_{raw}(x_c, y_c) - \mathbf{B}_{rot}(x_c, y_c)\bar{\boldsymbol{\omega}}_k$$
#[inline]
pub fn derotate_optical_flow(
    u_dot_raw: &Vector2<f64>,
    x_c: f64,
    y_c: f64,
    omega_bar: &Vector3<f64>,
) -> Vector2<f64> {
    let b_rot = compute_rotation_jacobian_b_rot(x_c, y_c);
    u_dot_raw - b_rot * omega_bar
}

/// Computes Lucas-Kanade Forward-Backward patch optical flow $(\dot{\mathbf{u}}_{fwd}, \dot{\mathbf{u}}_{bwd})$
/// and cycle consistency error $e_{cycle} = \|\dot{\mathbf{u}}_{fwd} + \dot{\mathbf{u}}_{bwd}\|_2$
/// between frame $t-1$ and frame $t$ in the `RingBuffer3Frames`.
pub fn compute_forward_backward_flow_patches(
    ring: &RingBuffer3Frames,
    omega_bar: &Vector3<f64>,
    ground_inv_depth: &[f32],
    bloom_mask: &[f32],
    config: &OpticalFlowDerotConfig,
    out_flows: &mut Vec<PatchOpticalFlow>,
    out_cycle_errors: &mut Vec<f64>,
) {
    let w = ring.width;
    let h = ring.height;
    let patch_sz = config.patch_size_px.clamp(2, 32);
    let grid_w = w / patch_sz;
    let grid_h = h / patch_sz;
    let total_patches = grid_w * grid_h;

    let (curr_img, curr_gx, curr_gy) = match ring.latest_frame_and_gradients() {
        Some(t) => t,
        None => {
            out_flows.clear();
            out_cycle_errors.clear();
            return;
        }
    };
    let (prev_img, prev_gx, prev_gy) = match ring.previous_frame_and_gradients() {
        Some(t) => t,
        None => {
            out_flows.clear();
            out_cycle_errors.clear();
            return;
        }
    };

    if out_flows.len() != total_patches {
        out_flows.resize(
            total_patches,
            PatchOpticalFlow {
                patch_index: 0,
                center_norm: (0.0, 0.0),
                u_dot_fwd: Vector2::zeros(),
                u_dot_bwd: Vector2::zeros(),
                e_cycle: 0.0,
                u_dot_trans: Vector2::zeros(),
                rho_ground: 0.0,
                bloom_mask: 1.0,
                is_ground_anchored: false,
            },
        );
    }
    if out_cycle_errors.len() != total_patches {
        out_cycle_errors.resize(total_patches, 0.0);
    }

    let dt = config.dt_frame_s.max(1e-4);
    // Conversion from pixel displacement to normalized camera coordinate rate:
    let px_to_norm_x = 2.0 / (w.max(1) as f64 * dt);
    let px_to_norm_y = 2.0 / (h.max(1) as f64 * dt);

    out_flows
        .par_iter_mut()
        .enumerate()
        .for_each(|(patch_idx, flow_out)| {
            let gy = patch_idx / grid_w;
            let gx = patch_idx % grid_w;
            let u0 = gx * patch_sz;
            let v0 = gy * patch_sz;
            let u1 = (u0 + patch_sz).min(w);
            let v1 = (v0 + patch_sz).min(h);

            // Forward LK system (prev -> curr) and Backward LK system (curr -> prev)
            let mut f_xx = 0.0_f64;
            let mut f_xy = 0.0_f64;
            let mut f_yy = 0.0_f64;
            let mut f_xt = 0.0_f64;
            let mut f_yt = 0.0_f64;

            let mut b_xx = 0.0_f64;
            let mut b_xy = 0.0_f64;
            let mut b_yy = 0.0_f64;
            let mut b_xt = 0.0_f64;
            let mut b_yt = 0.0_f64;

            let mut sum_rho = 0.0_f64;
            let mut sum_mask = 0.0_f64;
            let mut count = 0.0_f64;

            for v in v0..v1 {
                let row_off = v * w;
                for u in u0..u1 {
                    let idx = row_off + u;
                    let it_fwd = (curr_img[idx] - prev_img[idx]) as f64;
                    let it_bwd = -it_fwd;

                    let ix_f = 0.5 * (prev_gx[idx] + curr_gx[idx]) as f64;
                    let iy_f = 0.5 * (prev_gy[idx] + curr_gy[idx]) as f64;

                    f_xx += ix_f * ix_f;
                    f_xy += ix_f * iy_f;
                    f_yy += iy_f * iy_f;
                    f_xt += ix_f * it_fwd;
                    f_yt += iy_f * it_fwd;

                    let ix_b = curr_gx[idx] as f64;
                    let iy_b = curr_gy[idx] as f64;
                    b_xx += ix_b * ix_b;
                    b_xy += ix_b * iy_b;
                    b_yy += iy_b * iy_b;
                    b_xt += ix_b * it_bwd;
                    b_yt += iy_b * it_bwd;

                    if idx < ground_inv_depth.len() {
                        sum_rho += ground_inv_depth[idx] as f64;
                    }
                    if idx < bloom_mask.len() {
                        sum_mask += bloom_mask[idx] as f64;
                    }
                    count += 1.0;
                }
            }

            let reg = 1e-4;
            let det_f = (f_xx + reg) * (f_yy + reg) - f_xy * f_xy;
            let u_fwd_px = if det_f > EPS_F64 {
                Vector2::new(
                    (-(f_yy + reg) * f_xt + f_xy * f_yt) / det_f,
                    (f_xy * f_xt - (f_xx + reg) * f_yt) / det_f,
                )
            } else {
                Vector2::zeros()
            };

            let det_b = (b_xx + reg) * (b_yy + reg) - b_xy * b_xy;
            let u_bwd_px = if det_b > EPS_F64 {
                Vector2::new(
                    (-(b_yy + reg) * b_xt + b_xy * b_yt) / det_b,
                    (b_xy * b_xt - (b_xx + reg) * b_yt) / det_b,
                )
            } else {
                Vector2::zeros()
            };

            let u_dot_fwd = Vector2::new(u_fwd_px.x * px_to_norm_x, u_fwd_px.y * px_to_norm_y);
            let u_dot_bwd = Vector2::new(u_bwd_px.x * px_to_norm_x, u_bwd_px.y * px_to_norm_y);
            let e_cycle = (u_dot_fwd + u_dot_bwd).norm();

            let cx_px = (u0 + u1) as f64 * 0.5 - 0.5;
            let cy_px = (v0 + v1) as f64 * 0.5 - 0.5;
            let x_c = ((cx_px + 0.5) - 0.5 * w as f64) / (0.5 * w as f64);
            let y_c = ((cy_px + 0.5) - 0.5 * h as f64) / (0.5 * h as f64);

            let u_dot_trans = derotate_optical_flow(&u_dot_fwd, x_c, y_c, omega_bar);
            let inv_c = 1.0 / count.max(1.0);
            let mean_rho = sum_rho * inv_c;
            let mean_mask = (sum_mask * inv_c).clamp(0.0, 1.0);

            *flow_out = PatchOpticalFlow {
                patch_index: patch_idx,
                center_norm: (x_c, y_c),
                u_dot_fwd,
                u_dot_bwd,
                e_cycle,
                u_dot_trans,
                rho_ground: mean_rho,
                bloom_mask: mean_mask,
                is_ground_anchored: mean_rho > 0.004 && y_c > 0.05,
            };
        });

    for (i, f) in out_flows.iter().enumerate() {
        out_cycle_errors[i] = f.e_cycle;
    }
}

/// Computes the Autopilot Crab Angle $\psi_{crab}$ (Remediation #5):
///
/// $$\psi_{crab}(t) = \psi_{LRCV}(t) - \arcsin\left(\text{clamp}\left(\frac{\hat{w}_{fog, crosswind}}{\max(\|\hat{\mathbf{v}}_{air}\|, 1.0)}, -0.95, 0.95\right)\right)$$
#[inline]
pub fn compute_autopilot_crab_angle(
    psi_lrcv_rad: f64,
    w_fog_crosswind_mps: f64,
    v_air_norm_mps: f64,
) -> f64 {
    let ratio = (w_fog_crosswind_mps / v_air_norm_mps.max(1.0)).clamp(-0.95, 0.95);
    psi_lrcv_rad - ratio.asin()
}

/// Computes the Optical Gust Lookahead lead time $\Delta t_{lead}$ (Remediation #5):
///
/// $$\Delta t_{lead} = \frac{L_{lookahead}}{\max(u + \|\mathbf{w}_{fog}\|, 1.0)}$$
#[inline]
pub fn compute_gust_lookahead_lead_time(
    lookahead_distance_m: f64,
    u_aircraft_mps: f64,
    w_fog_mps: &Vector3<f64>,
) -> f64 {
    let closing_speed = (u_aircraft_mps + w_fog_mps.norm()).max(1.0);
    lookahead_distance_m.max(1.0) / closing_speed
}

/// Separates Aircraft Ground Velocity $\hat{\mathbf{v}}_{b, ground}$ from Ambient Fog Velocity
/// $\hat{\mathbf{w}}_{fog}$ and Lookahead Gust Front $\hat{\mathbf{w}}_{gust, lookahead}$
/// using Anisotropic Fisher Weighted Least Squares (Remediations #4, #5, #6).
///
/// For each patch $i$, the translational optical flow equation is:
/// - **Ground-anchored patches**:
///   $$\dot{\mathbf{u}}_{trans, i} = \rho_{ground, i} \mathbf{A}_{tr}(x_{c,i}, y_{c,i}) \mathbf{v}_{b, ground}$$
/// - **Fog-layer patches**:
///   $$\dot{\mathbf{u}}_{trans, i} = \hat{\rho}_{fog} \mathbf{A}_{tr}(x_{c,i}, y_{c,i}) (\mathbf{v}_{b, ground} - \mathbf{w}_{fog, body})$$
#[allow(clippy::too_many_arguments)]
pub fn solve_decoupled_ground_and_fog_velocity(
    flows: &[PatchOpticalFlow],
    tensors: &[PatchStructureTensor],
    rho_fog_est: f64,
    preintegrated_omega: &Vector3<f64>,
    psi_lrcv_rad: f64,
    prior_v_ground_body: Option<&Vector3<f64>>,
    config: &OpticalFlowDerotConfig,
) -> DecoupledMotionEstimate {
    let n = flows.len().min(tensors.len());
    let rho_fog = rho_fog_est.max(1e-4);

    // 1. Solve for Aircraft Ground Velocity \hat{v}_{b, ground} from ground-anchored patches
    let mut h_t_w_h_ground = Matrix3::<f64>::zeros();
    let mut h_t_w_z_ground = Vector3::<f64>::zeros();
    let mut active_ground_patches = 0usize;

    for i in 0..n {
        let f = &flows[i];
        let t = &tensors[i];

        if f.is_ground_anchored && f.bloom_mask > 0.1 && t.w_photon > 0.0 && f.rho_ground > 1e-4 {
            let (x_c, y_c) = f.center_norm;
            let h_i = f.rho_ground * compute_translation_jacobian_a_tr(x_c, y_c);
            let w_mat: Matrix2<f64> = f.bloom_mask * t.fisher_info;

            h_t_w_h_ground += h_i.transpose() * w_mat * h_i;
            h_t_w_z_ground += h_i.transpose() * (w_mat * f.u_dot_trans);
            active_ground_patches += 1;
        }
    }

    // Add gentle prior regularization so singular configurations (e.g. 100% whiteout) fall back gracefully
    let ground_reg = if active_ground_patches >= 3 && prior_v_ground_body.is_none() {
        1e-12
    } else {
        config.wls_regularization.max(1e-9)
    };
    let reg_mat = Matrix3::identity() * ground_reg;
    let info_ground = h_t_w_h_ground + reg_mat;
    if let Some(v_prior) = prior_v_ground_body {
        h_t_w_z_ground += reg_mat * v_prior;
    }

    let v_ground_body_mps = info_ground
        .try_inverse()
        .map(|inv| inv * h_t_w_z_ground)
        .unwrap_or_else(|| prior_v_ground_body.copied().unwrap_or_else(Vector3::zeros));

    // 2. Solve for Relative Fog Velocity v_rel = v_ground - w_fog in ambient fog patches vs lookahead zone
    let mut info_fog_amb = Matrix3::<f64>::identity() * 1e-12;
    let mut rhs_fog_amb = Vector3::<f64>::zeros();
    let mut active_fog_patches = 0usize;

    let mut info_fog_gust = Matrix3::<f64>::identity() * 1e-12;
    let mut rhs_fog_gust = Vector3::<f64>::zeros();
    let mut active_gust_patches = 0usize;

    for i in 0..n {
        let f = &flows[i];
        let t = &tensors[i];

        if !f.is_ground_anchored && f.bloom_mask > 0.1 && t.w_photon > 0.0 {
            let (x_c, y_c) = f.center_norm;
            let h_fog = rho_fog * compute_translation_jacobian_a_tr(x_c, y_c);
            let w_mat: Matrix2<f64> = f.bloom_mask * t.fisher_info;

            // Upper-horizon lookahead zone (y_c in [-0.30, 0.05]) samples the 35-80m lookahead gust front
            if y_c >= -0.35 && y_c <= 0.05 {
                info_fog_gust += h_fog.transpose() * w_mat * h_fog;
                rhs_fog_gust += h_fog.transpose() * (w_mat * f.u_dot_trans);
                active_gust_patches += 1;
            } else {
                info_fog_amb += h_fog.transpose() * w_mat * h_fog;
                rhs_fog_amb += h_fog.transpose() * (w_mat * f.u_dot_trans);
                active_fog_patches += 1;
            }
        }
    }

    let w_fog_body_mps = if active_fog_patches >= 3 {
        let v_rel_amb = info_fog_amb
            .try_inverse()
            .map(|inv| inv * rhs_fog_amb)
            .unwrap_or(v_ground_body_mps);
        v_ground_body_mps - v_rel_amb
    } else {
        Vector3::zeros()
    };

    let w_gust_lookahead_mps = if active_gust_patches >= 3 {
        let v_rel_gust = info_fog_gust
            .try_inverse()
            .map(|inv| inv * rhs_fog_gust)
            .unwrap_or_else(|| v_ground_body_mps - w_fog_body_mps);
        v_ground_body_mps - v_rel_gust
    } else {
        w_fog_body_mps
    };

    let gust_shear_delta_mps = w_gust_lookahead_mps - w_fog_body_mps;
    let gust_warning_active = gust_shear_delta_mps.norm() >= config.gust_alert_threshold_mps;

    // In camera coordinates (x_c right, y_c down, z_c forward), forward speed u is along z (or norm of v_ground)
    let u_forward = v_ground_body_mps.z.abs().max(v_ground_body_mps.norm());
    let gust_lead_time_s = compute_gust_lookahead_lead_time(
        config.lookahead_distance_m,
        u_forward,
        &w_gust_lookahead_mps,
    );

    let v_air_vec = v_ground_body_mps - w_gust_lookahead_mps;
    let crab_angle_rad = compute_autopilot_crab_angle(
        psi_lrcv_rad,
        w_gust_lookahead_mps.x,
        v_air_vec.norm().max(u_forward),
    );

    DecoupledMotionEstimate {
        preintegrated_omega_radps: *preintegrated_omega,
        v_ground_body_mps,
        v_ground_fisher_info: h_t_w_h_ground,
        w_fog_body_mps,
        w_gust_lookahead_mps,
        gust_shear_delta_mps,
        gust_warning_active,
        gust_lead_time_s,
        crab_angle_rad,
        active_ground_patches,
        active_fog_patches: active_fog_patches + active_gust_patches,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::Matrix2;

    #[test]
    fn test_pure_3d_rotation_derotation_is_exact_machine_zero() {
        // Arbitrary aggressive 6-DoF roll, pitch, yaw angular velocity (e.g., Dutch Roll + banking)
        let omega_true = Vector3::new(0.65, -0.42, 0.28); // rad/s
        let gyro_bias = Vector3::new(0.01, -0.005, 0.002);

        // Simulate IMU samples with constant rate
        let samples = vec![
            ImuAngularSample {
                timestamp_s: 0.0,
                omega_radps: omega_true + gyro_bias,
            },
            ImuAngularSample {
                timestamp_s: 0.008,
                omega_radps: omega_true + gyro_bias,
            },
            ImuAngularSample {
                timestamp_s: 0.016,
                omega_radps: omega_true + gyro_bias,
            },
        ];

        let omega_bar = preintegrate_imu_angular_rate(&samples, &gyro_bias, 0.0);
        assert!((omega_bar - omega_true).norm() < 1e-14);

        // Test across a grid of normalized camera coordinates (x_c, y_c) with ZERO translation:
        // Raw optical flow is purely rotational: u_dot_raw = B_rot(x_c, y_c) * omega_true
        for ix in -5..=5 {
            for iy in -5..=5 {
                let x_c = ix as f64 * 0.18;
                let y_c = iy as f64 * 0.18;
                let u_dot_raw = compute_rotation_jacobian_b_rot(x_c, y_c) * omega_true;
                let u_dot_trans = derotate_optical_flow(&u_dot_raw, x_c, y_c, &omega_bar);

                assert!(
                    u_dot_trans.norm() < 1e-14,
                    "Pure rotation must derotate to exact machine zero (< 1e-14), got {} at ({x_c}, {y_c})",
                    u_dot_trans.norm()
                );
            }
        }
    }

    #[test]
    fn test_decoupled_ground_velocity_and_optical_gust_lookahead() {
        // True aircraft velocity in camera frame (vx_lat = 0.0, vy_vert = 3.2, vz_fwd = 70.0 m/s)
        let true_v_ground = Vector3::new(0.0, 3.2, 70.0);
        // Ambient fog velocity (crosswind 4.0 m/s)
        let true_w_fog_amb = Vector3::new(4.0, 0.0, -2.0);
        // Sudden lookahead gust front at 55m ahead (+14 m/s lateral wind shear!)
        let true_w_fog_gust = Vector3::new(18.0, 0.0, -5.0);
        let rho_fog = 0.05_f64; // 20m effective fog scale

        let mut flows = Vec::new();
        let mut tensors = Vec::new();
        let mut idx = 0;

        for gy in 0..8 {
            for gx in 0..8 {
                let x_c = -0.8 + gx as f64 * 0.22;
                let y_c = -0.7 + gy as f64 * 0.22;
                let a_tr = compute_translation_jacobian_a_tr(x_c, y_c);

                let is_ground = y_c > 0.10;
                let rho_g = if is_ground { 0.03 * y_c } else { 0.0 };

                let u_dot_trans = if is_ground {
                    rho_g * (a_tr * true_v_ground)
                } else if y_c >= -0.35 && y_c <= 0.05 {
                    // Lookahead gust zone
                    rho_fog * (a_tr * (true_v_ground - true_w_fog_gust))
                } else {
                    // Ambient fog zone
                    rho_fog * (a_tr * (true_v_ground - true_w_fog_amb))
                };

                flows.push(PatchOpticalFlow {
                    patch_index: idx,
                    center_norm: (x_c, y_c),
                    u_dot_fwd: u_dot_trans,
                    u_dot_bwd: -u_dot_trans,
                    e_cycle: 0.0,
                    u_dot_trans,
                    rho_ground: rho_g,
                    bloom_mask: 1.0,
                    is_ground_anchored: is_ground,
                });

                tensors.push(PatchStructureTensor {
                    grid_x: gx,
                    grid_y: gy,
                    center_px: (0.0, 0.0),
                    center_norm: (x_c, y_c),
                    j_xx: 0.05,
                    j_xy: 0.0,
                    j_yy: 0.05,
                    lambda_1: 0.05,
                    lambda_2: 0.05,
                    e_1: Vector2::new(1.0, 0.0),
                    e_2: Vector2::new(0.0, 1.0),
                    e_cycle: 0.0,
                    w_photon: 0.95,
                    fisher_info: Matrix2::identity() * 0.0475,
                });

                idx += 1;
            }
        }

        let cfg = OpticalFlowDerotConfig::default();
        let est = solve_decoupled_ground_and_fog_velocity(
            &flows,
            &tensors,
            rho_fog,
            &Vector3::zeros(),
            0.0,
            None,
            &cfg,
        );

        // 1. Aircraft ground velocity must be recovered accurately without being corrupted by the fog gust!
        let err_v_ground = (est.v_ground_body_mps - true_v_ground).norm();
        assert!(
            err_v_ground < 1e-3,
            "Ground velocity must be uncorrupted by fog gust, err = {err_v_ground}"
        );

        // 2. Ambient fog velocity and lookahead gust velocity must be extracted accurately
        let err_w_amb = (est.w_fog_body_mps - true_w_fog_amb).norm();
        let err_w_gust = (est.w_gust_lookahead_mps - true_w_fog_gust).norm();
        assert!(err_w_amb < 1e-2, "Ambient fog error too large: {err_w_amb}");
        assert!(err_w_gust < 1e-2, "Lookahead gust error too large: {err_w_gust}");

        // 3. Optical Gust Lookahead warning must trigger with lead time ~0.6-0.8s and Crab Angle
        assert!(est.gust_warning_active, "14 m/s wind shear must trigger Gust Lookahead alert");
        assert!(
            est.gust_lead_time_s > 0.4 && est.gust_lead_time_s < 1.2,
            "Unexpected lead time: {}",
            est.gust_lead_time_s
        );
        assert!(est.crab_angle_rad.abs() > 0.15, "Crab angle must compensate for crosswind gust");
    }
}
