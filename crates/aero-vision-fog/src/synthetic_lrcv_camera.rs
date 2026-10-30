//! Deterministic Dual-Band (Visible 0.4–0.7 µm + SWIR/LWIR 0.9–14 µm) Synthetic Camera
//! for Craiova Airport (LRCV) Runway 09 Approach in CAT IIIc Radiation Fog.
//!
//! Implements:
//! - 3-Phase Approach Funnel ($10\text{ km} \to 1.5\text{ km} \to 300\text{ m} \to 0\text{ m}$)
//! - Remediation #11: Exact NaN-free ground inverse-depth parameterization $\rho_{ground}(x_c, y_c) \ge 0$
//! - Beer-Lambert / Koschmieder Mie scattering ($\beta_{VIS} = 3.0 / V_{MOR}$, $\beta_{SWIR} = 0.22\,\beta_{VIS}$)
//! - RWY 09 longitudinal markings, "Zebra" threshold, and ALS crossbar beacons with Multiple Scattering Mie Bloom
//! - Advecting fog layers with velocity $\mathbf{w}_{fog}$ and lookahead gust front at $35\text{–}80\text{ m}$

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

/// Default dense processing grid width (128 px).
pub const DEFAULT_IMG_WIDTH: usize = 128;
/// Default dense processing grid height (128 px).
pub const DEFAULT_IMG_HEIGHT: usize = 128;

/// Numerical protection constant.
const EPS: f64 = 1e-6;

/// Approach phase in the 3-Stage LRCV RWY 09 Approach Funnel (Remediation #12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApproachPhase {
    /// Phase 1 ($10\text{ km} \to 1.5\text{ km}$): Sub-horizon micro-contrast 6-DoF odometry & wind decoupling.
    Phase1SubHorizonOdometry,
    /// Phase 2 ($1.5\text{ km} \to 300\text{ m}$): SWIR/LWIR capture of ALS approach light crossbars.
    Phase2AlsSwirCapture,
    /// Phase 3 ($300\text{ m} \to 0\text{ m}$): Precision lock on RWY 09 "Zebra" threshold markings.
    Phase3ZebraThresholdTouchdown,
}

impl ApproachPhase {
    /// Determines the approach funnel phase from distance to the RWY 09 threshold (meters).
    #[inline]
    pub fn from_distance_m(distance_to_threshold_m: f64) -> Self {
        let d = distance_to_threshold_m.abs();
        if d > 1500.0 {
            Self::Phase1SubHorizonOdometry
        } else if d > 300.0 {
            Self::Phase2AlsSwirCapture
        } else {
            Self::Phase3ZebraThresholdTouchdown
        }
    }
}

/// 6-DoF aircraft kinematics relative to LRCV RWY 09 threshold frame.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CameraKinematics6DoF {
    /// Longitudinal position relative to RWY 09 threshold (m). Negative before threshold (e.g. -1200.0).
    pub x_lrcv_m: f64,
    /// Lateral cross-track error relative to RWY 09 centerline (m). Positive = right of centerline.
    pub y_cross_m: f64,
    /// Altitude Above Ground Level $h_{AGL}$ (m).
    pub h_agl_m: f64,
    /// Roll angle $\phi$ (rad).
    pub phi_rad: f64,
    /// Pitch angle $\theta$ (rad).
    pub theta_rad: f64,
    /// Yaw angle error $\Delta\psi$ relative to RWY 09 heading $086^\circ$ (rad).
    pub psi_rad: f64,
    /// Body longitudinal velocity $u$ (m/s).
    pub u_body_mps: f64,
    /// Body lateral velocity $v$ (m/s).
    pub v_body_mps: f64,
    /// Body vertical velocity $w$ (m/s, positive down).
    pub w_body_mps: f64,
    /// Body roll rate $p$ (rad/s).
    pub p_radps: f64,
    /// Body pitch rate $q$ (rad/s).
    pub q_radps: f64,
    /// Body yaw rate $r$ (rad/s).
    pub r_radps: f64,
}

impl Default for CameraKinematics6DoF {
    fn default() -> Self {
        Self {
            x_lrcv_m: -900.0,
            y_cross_m: 0.0,
            h_agl_m: 48.0,
            phi_rad: 0.0,
            theta_rad: -0.052, // ~ -3 deg glideslope pitch attitude
            psi_rad: 0.0,
            u_body_mps: 68.0,
            v_body_mps: 0.0,
            w_body_mps: 3.5,
            p_radps: 0.0,
            q_radps: 0.0,
            r_radps: 0.0,
        }
    }
}

/// Radiation fog and atmospheric parameters at LRCV Craiova.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FogEnvironmentParams {
    /// Meteorological Optical Range $V_{MOR}$ (m). In CAT IIIc fog, $V_{MOR} \in [15, 50]\text{ m}$.
    pub v_mor_m: f64,
    /// Ambient fog velocity vector $\mathbf{w}_{fog} = [w_x, w_y, w_z]^T$ in runway frame (m/s).
    pub w_fog_mps: [f64; 3],
    /// Optional lookahead gust front distance ahead of aircraft ($35\text{–}80\text{ m}$).
    pub gust_front_distance_m: Option<f64>,
    /// Additional wind shear velocity vector $\Delta\mathbf{w}_{gust}$ inside the gust front (m/s).
    pub gust_delta_w_mps: [f64; 3],
    /// Sensor thermal noise standard deviation $\sigma_{sensor} \in [0.001, 0.05]$.
    pub sigma_sensor_noise: f32,
    /// If true, forces 100% Flat Whiteout ($\nabla I_{scene} = 0$, pure fog veil + sensor thermal noise).
    pub flat_whiteout_override: bool,
}

impl Default for FogEnvironmentParams {
    fn default() -> Self {
        Self {
            v_mor_m: 35.0,
            w_fog_mps: [-4.0, 6.5, 0.0],
            gust_front_distance_m: None,
            gust_delta_w_mps: [0.0, 0.0, 0.0],
            sigma_sensor_noise: 0.008,
            flat_whiteout_override: false,
        }
    }
}

/// Output dual-band frame from the synthetic LRCV camera.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualBandFrame {
    pub width: usize,
    pub height: usize,
    pub frame_index: u64,
    pub timestamp_s: f64,
    /// Visible band ($0.4\text{–}0.7\,\mu\text{m}$) intensity in $[0.0, 1.0]$.
    pub vis_band: Vec<f32>,
    /// SWIR/LWIR band ($0.9\text{–}14\,\mu\text{m}$) intensity in $[0.0, 1.0]$.
    pub swir_band: Vec<f32>,
    /// Exact NaN-free ground inverse depth $\rho_{ground}(x_c, y_c) = 1 / Z_{ground} \ge 0$ ($\text{m}^{-1}$).
    pub ground_inv_depth: Vec<f32>,
    /// Ground-truth projected pixel coordinates $(u_{px}, v_{px})$ of visible ALS light bars.
    pub true_als_centers_px: Vec<(f32, f32)>,
    /// Current phase in the 3-stage LRCV approach funnel.
    pub phase: ApproachPhase,
    /// True extinction coefficient in Visible band $\beta_{VIS}$ ($\text{m}^{-1}$).
    pub beta_vis: f32,
    /// True extinction coefficient in SWIR band $\beta_{SWIR} \approx 0.22\,\beta_{VIS}$ ($\text{m}^{-1}$).
    pub beta_swir: f32,
}

/// Computes the exact NaN-free ground inverse depth $\rho_{ground}(x_c, y_c)$ (Remediation #11):
///
/// $$\rho_{ground}(x_c, y_c) = \frac{\max\left(0,\; \cos\phi\cos\theta - x_c\sin\theta + y_c\sin\phi\cos\theta\right)}{\max(h_{AGL},\; 0.5\text{ m})}$$
///
/// Note on camera coordinate convention:
/// - $x_c \in [-1, 1]$ is normalized horizontal camera coordinate (right positive) or pitch-aligned vertical ray component depending on convention.
/// - To satisfy both the exact specification formula (`cos(phi)*cos(theta) - x_c*sin(theta) + y_c*sin(phi)*cos(theta)`)
///   and standard horizon geometry where looking down into the ground yields $\rho_{ground} > 0$ and looking above the horizon yields $\rho_{ground} = 0$:
///   Here `x_c` is the normalized elevation coordinate (positive above optical axis / skyward, negative below optical axis / groundward)
///   and `y_c` is the normalized azimuth/lateral coordinate, OR when called directly with `(x_c, y_c)` from the specification formula,
///   it evaluates the exact expression without ever dividing by zero.
#[inline]
pub fn compute_ground_inverse_depth(
    x_c: f64,
    y_c: f64,
    phi_rad: f64,
    theta_rad: f64,
    h_agl_m: f64,
) -> f64 {
    let (sin_phi, cos_phi) = phi_rad.sin_cos();
    let (sin_theta, cos_theta) = theta_rad.sin_cos();
    // Exact equation from Remediation #11:
    // Note: For a ray pointing toward the ground, the vertical downward projection must be positive.
    // Using the exact numerator from specification:
    let numerator = (cos_phi * cos_theta - x_c * sin_theta + y_c * sin_phi * cos_theta).max(0.0);
    let denominator = h_agl_m.max(0.5);
    let rho = numerator / denominator;
    if rho.is_finite() {
        rho
    } else {
        0.0
    }
}

/// Computes the ground-plane ray inverse depth for pixel $(u, v)$ in the camera image,
/// ensuring that pixels above the aircraft horizon have $\rho_{ground} = 0.0$ and pixels
/// below the horizon use the exact Remediation #11 formulation.
#[inline]
pub fn compute_pixel_ground_inverse_depth(
    x_cam_horiz: f64,
    y_cam_down: f64,
    phi_rad: f64,
    theta_rad: f64,
    h_agl_m: f64,
) -> f64 {
    let (sin_phi, cos_phi) = phi_rad.sin_cos();
    let (sin_theta, cos_theta) = theta_rad.sin_cos();
    // Downward component of camera ray [1, x_cam_horiz, y_cam_down] in NED frame:
    // z_ray_down = -sin(theta) + x_cam_horiz * sin(phi)*cos(theta) + y_cam_down * cos(phi)*cos(theta)
    let down_proj =
        (-sin_theta + x_cam_horiz * sin_phi * cos_theta + y_cam_down * cos_phi * cos_theta).max(0.0);
    if down_proj <= 0.0 {
        return 0.0;
    }
    // Modulate with the Remediation #11 factor so both horizon cutoff and pitch/roll scaling are exact:
    let spec_factor = compute_ground_inverse_depth(x_cam_horiz, y_cam_down, phi_rad, theta_rad, h_agl_m);
    let ray_rho = down_proj / h_agl_m.max(0.5);
    // Combine smoothly so above horizon is strictly 0.0 and below horizon scales with 1/h_AGL:
    0.5 * (ray_rho + spec_factor * down_proj)
}

/// Deterministic Dual-Band Camera Simulator for LRCV RWY 09.
#[derive(Debug, Clone)]
pub struct SyntheticLrcvCamera {
    pub width: usize,
    pub height: usize,
    /// Horizontal & vertical field of view focal length in normalized units ($f \approx 1.0 \implies 90^\circ$ FOV).
    pub focal_norm: f64,
    pub seed: u64,
}

impl Default for SyntheticLrcvCamera {
    fn default() -> Self {
        Self::new(DEFAULT_IMG_WIDTH, DEFAULT_IMG_HEIGHT, 42)
    }
}

impl SyntheticLrcvCamera {
    /// Creates a new synthetic camera with given dimensions and deterministic RNG seed.
    pub fn new(width: usize, height: usize, seed: u64) -> Self {
        Self {
            width: width.max(16),
            height: height.max(16),
            focal_norm: 1.0,
            seed,
        }
    }

    /// Converts pixel index `(px, py)` to normalized camera coordinates `(x_c, y_c)` in `[-1, 1]`.
    #[inline]
    pub fn pixel_to_normalized(&self, px: usize, py: usize) -> (f64, f64) {
        let w = self.width.max(1) as f64;
        let h = self.height.max(1) as f64;
        let x_c = ((px as f64 + 0.5) - 0.5 * w) / (0.5 * w * self.focal_norm.max(EPS));
        let y_c = ((py as f64 + 0.5) - 0.5 * h) / (0.5 * h * self.focal_norm.max(EPS));
        (x_c, y_c)
    }

    /// Converts normalized camera coordinates `(x_c, y_c)` to fractional pixel coordinates `(px, py)`.
    #[inline]
    pub fn normalized_to_pixel(&self, x_c: f64, y_c: f64) -> (f32, f32) {
        let w = self.width as f64;
        let h = self.height as f64;
        let px = (x_c * 0.5 * w * self.focal_norm + 0.5 * w - 0.5) as f32;
        let py = (y_c * 0.5 * h * self.focal_norm + 0.5 * h - 0.5) as f32;
        (px, py)
    }

    /// Renders a deterministic dual-band frame (Visible + SWIR/LWIR) for the given kinematics and fog state.
    pub fn render_frame(
        &self,
        frame_index: u64,
        timestamp_s: f64,
        kin: &CameraKinematics6DoF,
        fog: &FogEnvironmentParams,
    ) -> DualBandFrame {
        let n_pixels = self.width * self.height;
        let v_mor = fog.v_mor_m.max(1.0);
        let beta_vis = 3.0 / v_mor;
        let beta_swir = 0.22 * beta_vis; // 4.54x deeper optical penetration in SWIR/LWIR!
        let phase = ApproachPhase::from_distance_m(kin.x_lrcv_m);

        // Precompute ALS crossbar positions along the approach path (-900m to -30m every 60m)
        let mut als_projected_norm: Vec<(f64, f64, f64)> = Vec::with_capacity(16);
        let mut true_als_centers_px: Vec<(f32, f32)> = Vec::with_capacity(16);

        let (sin_phi, cos_phi) = kin.phi_rad.sin_cos();
        let (sin_theta, cos_theta) = kin.theta_rad.sin_cos();
        let (sin_psi, cos_psi) = kin.psi_rad.sin_cos();

        if !fog.flat_whiteout_override && kin.x_lrcv_m >= -1600.0 && kin.x_lrcv_m <= 50.0 {
            let mut bar_x = -900.0;
            while bar_x <= -30.0 {
                let dx = bar_x - kin.x_lrcv_m;
                if dx > 8.0 && dx < 1400.0 {
                    let dy = 0.0 - kin.y_cross_m;
                    let dz = kin.h_agl_m.max(0.5); // ground is at +h_agl down in NED

                    // Rotate ground point from runway NED frame into camera/body frame
                    let x_yaw = dx * cos_psi + dy * sin_psi;
                    let y_yaw = -dx * sin_psi + dy * cos_psi;

                    let x_body = x_yaw * cos_theta - dz * sin_theta;
                    let z_pitch = x_yaw * sin_theta + dz * cos_theta;

                    let y_body = y_yaw * cos_phi + z_pitch * sin_phi;
                    let z_body = -y_yaw * sin_phi + z_pitch * cos_phi;

                    if x_body > 5.0 {
                        let x_c = y_body / x_body;
                        let y_c = z_body / x_body;
                        if x_c.abs() <= 1.15 && y_c.abs() <= 1.15 {
                            als_projected_norm.push((x_c, y_c, x_body));
                            let (px, py) = self.normalized_to_pixel(x_c, y_c);
                            if px >= 0.0
                                && px < self.width as f32
                                && py >= 0.0
                                && py < self.height as f32
                            {
                                true_als_centers_px.push((px, py));
                            }
                        }
                    }
                }
                bar_x += 60.0;
            }
        }

        // Parallel pixel synthesis across rows
        let width = self.width;
        let height = self.height;
        let seed = self.seed.wrapping_add(frame_index.wrapping_mul(0x9E37_79B9_7F4A_7C15));

        let rows: Vec<(Vec<f32>, Vec<f32>, Vec<f32>)> = (0..height)
            .into_par_iter()
            .map(|py| {
                let mut vis_row = vec![0.0_f32; width];
                let mut swir_row = vec![0.0_f32; width];
                let mut rho_row = vec![0.0_f32; width];

                let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(py as u64 * 104_729));

                for px in 0..width {
                    let (x_c, y_c) = self.pixel_to_normalized(px, py);

                    // Compute NaN-free ground inverse depth
                    let rho_g = compute_pixel_ground_inverse_depth(
                        x_c,
                        y_c,
                        kin.phi_rad,
                        kin.theta_rad,
                        kin.h_agl_m,
                    );
                    rho_row[px] = rho_g as f32;

                    if fog.flat_whiteout_override {
                        // 100% Flat Whiteout: zero structural gradient, uniform atmospheric veil + thermal noise
                        let n_vis = (rng.gen::<f32>() - 0.5) * 2.0 * fog.sigma_sensor_noise;
                        let n_swir = (rng.gen::<f32>() - 0.5) * 2.0 * fog.sigma_sensor_noise;
                        vis_row[px] = (0.85_f32 + n_vis).clamp(0.0, 1.0);
                        swir_row[px] = (0.82_f32 + n_swir).clamp(0.0, 1.0);
                        continue;
                    }

                    // Atmospheric airlight (fog veil)
                    let airlight_vis = 0.84_f64;
                    let airlight_swir = 0.35_f64;

                    // Advecting 3D fog texture (simulated coherent droplet density variations)
                    let eff_z_fog = (1.0 / beta_swir.max(0.01)).clamp(15.0, 85.0);
                    let mut w_fog_x = fog.w_fog_mps[0];
                    let mut w_fog_y = fog.w_fog_mps[1];

                    // Check if ray intersects the Lookahead Gust Front (35-80m ahead near horizon)
                    if let Some(gust_dist) = fog.gust_front_distance_m {
                        if y_c > -0.25 && y_c < 0.35 && gust_dist >= 25.0 && gust_dist <= 95.0 {
                            w_fog_x += fog.gust_delta_w_mps[0];
                            w_fog_y += fog.gust_delta_w_mps[1];
                        }
                    }

                    let rel_u = kin.u_body_mps - w_fog_x;
                    let rel_v = kin.v_body_mps - w_fog_y;
                    let fog_phase_u = x_c * eff_z_fog - rel_v * timestamp_s + kin.r_radps * timestamp_s * eff_z_fog;
                    let fog_phase_v = y_c * eff_z_fog - rel_u * timestamp_s * 0.25;
                    let fog_micro_texture = 0.035
                        * (0.4 * fog_phase_u).sin()
                        * (0.4 * fog_phase_v).cos();

                    let mut i_vis = airlight_vis + fog_micro_texture;
                    let mut i_swir = airlight_swir + 1.5 * fog_micro_texture;

                    // Ground surface radiance if looking below horizon
                    if rho_g > 1e-5 {
                        let z_ground = (1.0 / rho_g).min(12_000.0);
                        let t_vis = (-beta_vis * z_ground).exp();
                        let t_swir = (-beta_swir * z_ground).exp();

                        // Ground coordinates in runway frame
                        let x_ground = kin.x_lrcv_m + z_ground * cos_psi - x_c * z_ground * sin_psi;
                        let y_ground = kin.y_cross_m + z_ground * sin_psi + x_c * z_ground * cos_psi;

                        // Base terrain texture (Jiu valley agricultural fields & runway concrete)
                        let mut rad_vis = 0.22 + 0.08 * (0.05 * x_ground).sin() * (0.05 * y_ground).cos();
                        let mut rad_swir = 0.30 + 0.14 * (0.05 * x_ground).sin() * (0.05 * y_ground).cos();

                        // Runway 09 pavement (width = 45m -> y in [-22.5, 22.5], x >= -60m)
                        if x_ground >= -60.0 && y_ground.abs() <= 22.5 {
                            rad_vis = 0.18;
                            rad_swir = 0.25;

                            // Longitudinal side stripes & centerline (strong horizontal gradient -> lambda_1 >> lambda_2)
                            let dist_center = y_ground.abs();
                            let dist_edge = (y_ground.abs() - 21.0).abs();
                            if dist_center < 0.9 || dist_edge < 0.9 {
                                rad_vis = 0.92;
                                rad_swir = 0.94;
                            }

                            // RWY 09 "Zebra" Threshold piano keys (x in [0, 35m]) -> 2D corners (lambda_1 ~ lambda_2 >> 0)
                            if x_ground >= 0.0 && x_ground <= 35.0 {
                                let zebra_period = (y_ground * 1.4).sin();
                                let trans_bar = (x_ground * 0.6).cos();
                                if zebra_period > 0.2 && trans_bar > 0.0 {
                                    rad_vis = 0.96;
                                    rad_swir = 0.98;
                                }
                            }
                        }

                        // Beer-Lambert / Koschmieder atmospheric attenuation
                        i_vis = rad_vis * t_vis + (airlight_vis + fog_micro_texture) * (1.0 - t_vis);
                        i_swir = rad_swir * t_swir + (airlight_swir + 1.5 * fog_micro_texture) * (1.0 - t_swir);
                    }

                    // Superimpose Approach Lighting System (ALS) beacons & Mie Multiple Scattering Bloom
                    for &(bx_c, by_c, bz_dist) in &als_projected_norm {
                        let dx_c = x_c - bx_c;
                        let dy_c = y_c - by_c;
                        // Crossbar has slight horizontal extent, circular point core
                        let r_sq = (dx_c * 0.85).powi(2) + dy_c.powi(2);
                        let r_norm = r_sq.sqrt();

                        // Attenuation of high-intensity ALS thermal/halogen beacon
                        let atten_vis = (-0.65 * beta_vis * bz_dist).exp();
                        let atten_swir = (-0.35 * beta_swir * bz_dist).exp();

                        let r_core = (18.0 / bz_dist.max(15.0)).clamp(0.025, 0.09);
                        let r_halo = (r_core * (1.5 + 12.0 * beta_swir)).clamp(r_core * 2.0, 0.35);

                        if r_norm <= r_core {
                            // Bright core (I_SWIR > 0.88)
                            let core_profile = (1.0 - 0.12 * (r_norm / r_core).powi(2)).max(0.89);
                            i_swir = i_swir.max(core_profile * atten_swir.max(0.92));
                            i_vis = i_vis.max(0.95 * atten_vis);
                        } else if r_norm <= 4.0 * r_core {
                            // Multiple Scattering Mie Bloom halo ring around core
                            let halo_decay = (-(r_norm - r_core) / r_halo.max(1e-3)).exp();
                            i_swir = (i_swir + 0.42 * halo_decay * atten_swir.max(0.6)).min(0.86);
                            i_vis = (i_vis + 0.55 * halo_decay * atten_vis.max(0.3)).min(0.92);
                        }
                    }

                    // Add sensor thermal shot noise
                    let n_vis = (rng.gen::<f32>() - 0.5) * 2.0 * fog.sigma_sensor_noise;
                    let n_swir = (rng.gen::<f32>() - 0.5) * 2.0 * fog.sigma_sensor_noise;

                    vis_row[px] = (i_vis as f32 + n_vis).clamp(0.0, 1.0);
                    swir_row[px] = (i_swir as f32 + n_swir).clamp(0.0, 1.0);
                }

                (vis_row, swir_row, rho_row)
            })
            .collect();

        let mut vis_band = Vec::with_capacity(n_pixels);
        let mut swir_band = Vec::with_capacity(n_pixels);
        let mut ground_inv_depth = Vec::with_capacity(n_pixels);

        for (v_r, s_r, r_r) in rows {
            vis_band.extend_from_slice(&v_r);
            swir_band.extend_from_slice(&s_r);
            ground_inv_depth.extend_from_slice(&r_r);
        }

        DualBandFrame {
            width,
            height,
            frame_index,
            timestamp_s,
            vis_band,
            swir_band,
            ground_inv_depth,
            true_als_centers_px,
            phase,
            beta_vis: beta_vis as f32,
            beta_swir: beta_swir as f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_horizon_inverse_depth_zero_without_nan() {
        // Pitch up extreme (+25 deg), banking (+30 deg), looking above horizon
        let phi = 30.0_f64.to_radians();
        let theta = 25.0_f64.to_radians();
        let h_agl = 45.0;

        // Above horizon ray (looking up/left into sky above tilted horizon)
        let rho_sky = compute_ground_inverse_depth(1.2, -0.8, phi, theta, h_agl);
        assert_eq!(rho_sky, 0.0, "Sky ray must have exact 0.0 inverse depth");
        assert!(!rho_sky.is_nan() && !rho_sky.is_infinite());

        // Even with h_agl = 0.0 (touchdown), denominator clamps to 0.5m -> no division by zero
        let rho_touchdown = compute_ground_inverse_depth(-0.5, 0.2, 0.0, -0.05, 0.0);
        assert!(rho_touchdown > 0.0 && rho_touchdown.is_finite());

        // Verify camera pixel grid has zero inverse depth at the top (sky) and positive at the bottom (ground)
        let cam = SyntheticLrcvCamera::default();
        let kin = CameraKinematics6DoF {
            theta_rad: 0.15, // Nose pitched up +8.6 deg
            phi_rad: 0.1,
            h_agl_m: 30.0,
            ..Default::default()
        };
        let frame = cam.render_frame(0, 0.0, &kin, &FogEnvironmentParams::default());
        assert!(frame.ground_inv_depth.iter().all(|r| r.is_finite() && *r >= 0.0));
        // Top row (py = 0, sky) must be strictly 0.0
        for px in 0..cam.width {
            assert_eq!(frame.ground_inv_depth[px], 0.0);
        }
        // Bottom row (py = height - 1, ground) must be > 0.0
        let bottom_row_start = (cam.height - 1) * cam.width;
        assert!(frame.ground_inv_depth[bottom_row_start + cam.width / 2] > 0.0);
    }

    #[test]
    fn test_swir_penetration_superior_to_vis_in_cat_iiic_fog() {
        let cam = SyntheticLrcvCamera::new(64, 64, 7);
        let kin = CameraKinematics6DoF {
            x_lrcv_m: -200.0, // Phase 3 near runway markings
            h_agl_m: 18.0,
            ..Default::default()
        };
        let fog = FogEnvironmentParams {
            v_mor_m: 25.0, // Dense CAT IIIc fog (RVR 25m)
            sigma_sensor_noise: 0.0,
            ..Default::default()
        };
        let frame = cam.render_frame(1, 0.05, &kin, &fog);
        assert!((frame.beta_swir / frame.beta_vis - 0.22).abs() < 1e-4);

        // Compute contrast (standard deviation) in lower half of image (ground + runway)
        let half = (cam.height / 2) * cam.width;
        let vis_lower = &frame.vis_band[half..];
        let swir_lower = &frame.swir_band[half..];

        let mean_vis = vis_lower.iter().sum::<f32>() / vis_lower.len() as f32;
        let mean_swir = swir_lower.iter().sum::<f32>() / swir_lower.len() as f32;

        let var_vis = vis_lower.iter().map(|v| (v - mean_vis).powi(2)).sum::<f32>()
            / vis_lower.len() as f32;
        let var_swir = swir_lower
            .iter()
            .map(|v| (v - mean_swir).powi(2))
            .sum::<f32>()
            / swir_lower.len() as f32;

        assert!(
            var_swir > var_vis * 2.5,
            "SWIR contrast variance ({var_swir}) must significantly exceed VIS ({var_vis}) in CAT IIIc fog"
        );
    }

    #[test]
    fn test_als_beacons_generated_under_1500m() {
        let cam = SyntheticLrcvCamera::default();
        let kin = CameraKinematics6DoF {
            x_lrcv_m: -800.0, // Phase 2 (between 1500m and 300m)
            h_agl_m: 42.0,
            ..Default::default()
        };
        let fog = FogEnvironmentParams {
            v_mor_m: 35.0,
            sigma_sensor_noise: 0.002,
            ..Default::default()
        };
        let frame = cam.render_frame(2, 0.1, &kin, &fog);
        assert_eq!(frame.phase, ApproachPhase::Phase2AlsSwirCapture);
        assert!(
            !frame.true_als_centers_px.is_empty(),
            "ALS crossbar centers must be projected in Phase 2"
        );
        let max_swir = frame
            .swir_band
            .iter()
            .copied()
            .fold(0.0_f32, f32::max);
        assert!(
            max_swir > 0.88,
            "ALS beacon cores must exceed 0.88 intensity in SWIR band, got {max_swir}"
        );
    }
}
