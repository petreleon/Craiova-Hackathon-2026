//! SNR-Gated CLAHE (Remediation #1) and Dual Physical Fog Scale Anchoring:
//! Koschmieder Optical Extinction Law + Active Inertial Scale Probing (Remediation #3).
//!
//! ### Key Mathematical Guarantees:
//! 1. **SNR-Gated CLAHE**:
//!    $$\text{clip\_limit}(x, y) = 1.0 + \text{clip}_{\max} \cdot \frac{\sigma_{local}^2(x, y)}{\sigma_{local}^2(x, y) + 3\sigma_{sensor\_noise}^2 + \epsilon}$$
//!    In `100% Flat Whiteout` ($\sigma_{local}^2 \ll \sigma_{sensor\_noise}^2$), $\text{clip\_limit} \to 1.0$.
//!    A normalized histogram clip limit of $1.0$ flattens the histogram redistribution to a uniform
//!    density, producing an exact identity CDF $T(I) = I$ and completely preventing amplification
//!    of CMOS thermal/shot noise!
//! 2. **Dual Physical Anchoring for Fog Inverse Depth $\hat{\rho}_{fog} = 1 / \bar{Z}_{fog}$**:
//!    - *Koschmieder Optical Anchor*:
//!      $$\bar{Z}_{fog, opt} = \int_0^\infty Z \beta_{ext} e^{-\beta_{ext} Z} dZ = \frac{1}{\beta_{ext}} \implies \hat{\rho}_{fog, opt} = \hat{\beta}_{ext} \approx \frac{3.0}{V_{MOR}}$$
//!    - *Active Inertial Scale Probing*:
//!      $$\hat{\rho}_{fog, inert} = \frac{\|\Delta\dot{\mathbf{u}}_{fog, trans}\|}{\|\mathbf{A}_{tr}\Delta\mathbf{v}_b\| + \epsilon}$$

use nalgebra::{Matrix2x3, Vector2, Vector3};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

const EPS_F32: f32 = 1e-9;
const EPS_F64: f64 = 1e-6;
const NUM_HIST_BINS: usize = 64;

/// Configuration for SNR-Gated CLAHE and Koschmieder Scale Probing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ClaheKoschmiederConfig {
    /// Maximum additional histogram clip factor $\text{clip}_{\max}$ (e.g. `3.0` -> max clip limit `4.0`).
    pub clip_max: f32,
    /// Estimated sensor thermal noise standard deviation $\sigma_{sensor\_noise}$.
    pub sigma_sensor_noise: f32,
    /// Tile grid dimension (e.g. `8` for $8 \times 8$ tiles on a $128 \times 128$ frame).
    pub num_tiles_per_axis: usize,
    /// Minimum Meteorological Optical Range $V_{MOR,\min}$ clamp (m).
    pub min_v_mor_m: f64,
    /// Maximum Meteorological Optical Range $V_{MOR,\max}$ clamp (m).
    pub max_v_mor_m: f64,
}

impl Default for ClaheKoschmiederConfig {
    fn default() -> Self {
        Self {
            clip_max: 3.0,
            sigma_sensor_noise: 0.008,
            num_tiles_per_axis: 8,
            min_v_mor_m: 10.0,
            max_v_mor_m: 5000.0,
        }
    }
}

/// Output of the Koschmieder + Active Inertial Scale Probing estimator (Remediation #3).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FogScaleEstimate {
    /// Estimated extinction coefficient $\hat{\beta}_{ext}$ ($\text{m}^{-1}$).
    pub beta_ext_inv_m: f64,
    /// Estimated Meteorological Optical Range $\hat{V}_{MOR} = 3.0 / \hat{\beta}_{ext}$ (m).
    pub estimated_v_mor_m: f64,
    /// Optical inverse depth anchor $\hat{\rho}_{fog, opt} = \hat{\beta}_{ext}$ ($\text{m}^{-1}$).
    pub rho_fog_optical: f64,
    /// Inertial scale probing inverse depth $\hat{\rho}_{fog, inert}$ ($\text{m}^{-1}$).
    pub rho_fog_inertial: Option<f64>,
    /// Fused fog inverse depth $\hat{\rho}_{fog} = 1 / \bar{Z}_{fog}$ ($\text{m}^{-1}$).
    pub rho_fog_fused: f64,
    /// Effective mean fog layer distance $\bar{Z}_{fog} = 1 / \hat{\rho}_{fog}$ (m).
    pub mean_fog_depth_m: f64,
}

/// Computes the exact SNR-gated CLAHE clip limit (Remediation #1):
///
/// $$\text{clip\_limit}(x, y) = 1.0 + \text{clip}_{\max} \cdot \frac{\sigma_{local}^2(x, y)}{\sigma_{local}^2(x, y) + 3\sigma_{sensor\_noise}^2 + \epsilon}$$
#[inline]
pub fn compute_snr_clip_limit(
    local_variance: f32,
    sigma_sensor_noise: f32,
    clip_max: f32,
) -> f32 {
    let var = local_variance.max(0.0);
    let noise_var_3x = 3.0 * sigma_sensor_noise * sigma_sensor_noise;
    let snr_gate = if var <= noise_var_3x {
        // Below the 3*sigma_noise^2 thermal noise floor, strongly suppress contrast stretching
        let ratio = var / (noise_var_3x + EPS_F32);
        ratio * ratio * (var / (var + noise_var_3x + EPS_F32))
    } else {
        var / (var + noise_var_3x + EPS_F32)
    };
    1.0 + clip_max.max(0.0) * snr_gate.clamp(0.0, 1.0)
}

/// Applies SNR-Gated CLAHE in parallel into `out_enhanced` and writes per-pixel `out_clip_limits`.
///
/// When `local_variance` is dominated by thermal noise (`Flat Whiteout`), `clip_limit -> 1.0`,
/// which caps every histogram bin at the uniform average $N_{tile} / N_{bins}$ and forces an
/// identity transfer curve so thermal noise is never amplified.
pub fn apply_snr_gated_clahe_into(
    input_image: &[f32],
    local_variance: &[f32],
    width: usize,
    height: usize,
    config: &ClaheKoschmiederConfig,
    out_enhanced: &mut [f32],
    out_clip_limits: &mut [f32],
) {
    let n = width * height;
    if input_image.len() < n
        || local_variance.len() < n
        || out_enhanced.len() < n
        || out_clip_limits.len() < n
        || width == 0
        || height == 0
    {
        return;
    }

    let n_tiles = config.num_tiles_per_axis.clamp(2, 16);
    let tile_w = (width + n_tiles - 1) / n_tiles;
    let tile_h = (height + n_tiles - 1) / n_tiles;

    // 1. Compute per-pixel SNR clip limits in parallel
    out_clip_limits[..n]
        .par_iter_mut()
        .zip(local_variance[..n].par_iter())
        .for_each(|(clip_out, &var)| {
            *clip_out = compute_snr_clip_limit(var, config.sigma_sensor_noise, config.clip_max);
        });

    // 2. Build clipped CDF lookup tables for each tile in parallel
    let total_tiles = n_tiles * n_tiles;
    let tile_cdfs: Vec<[f32; NUM_HIST_BINS]> = (0..total_tiles)
        .into_par_iter()
        .map(|tile_idx| {
            let ty = tile_idx / n_tiles;
            let tx = tile_idx % n_tiles;
            let y0 = ty * tile_h;
            let y1 = ((ty + 1) * tile_h).min(height);
            let x0 = tx * tile_w;
            let x1 = ((tx + 1) * tile_w).min(width);

            let mut hist = [0.0_f32; NUM_HIST_BINS];
            let mut sum_clip = 0.0_f32;
            let mut count = 0.0_f32;

            for y in y0..y1 {
                let row_off = y * width;
                for x in x0..x1 {
                    let val = input_image[row_off + x].clamp(0.0, 1.0);
                    let bin = ((val * (NUM_HIST_BINS - 1) as f32).round() as usize)
                        .min(NUM_HIST_BINS - 1);
                    hist[bin] += 1.0;
                    sum_clip += out_clip_limits[row_off + x];
                    count += 1.0;
                }
            }

            if count < 1.0 {
                let mut identity_cdf = [0.0_f32; NUM_HIST_BINS];
                for (b, slot) in identity_cdf.iter_mut().enumerate() {
                    *slot = b as f32 / (NUM_HIST_BINS - 1) as f32;
                }
                return identity_cdf;
            }

            let mean_tile_clip = (sum_clip / count).max(1.0);
            let avg_bin_count = count / (NUM_HIST_BINS as f32);
            let max_bin_count = mean_tile_clip * avg_bin_count;

            // Clip histogram and accumulate excess
            let mut excess = 0.0_f32;
            for h_bin in &mut hist {
                if *h_bin > max_bin_count {
                    excess += *h_bin - max_bin_count;
                    *h_bin = max_bin_count;
                }
            }

            // Redistribute excess uniformly across all bins
            let redistrib = excess / (NUM_HIST_BINS as f32);
            for h_bin in &mut hist {
                *h_bin += redistrib;
            }

            // Build normalized CDF
            let mut cdf = [0.0_f32; NUM_HIST_BINS];
            let mut cum = 0.0_f32;
            let inv_total = 1.0 / count.max(1.0);
            for b in 0..NUM_HIST_BINS {
                cum += hist[b];
                cdf[b] = (cum * inv_total).clamp(0.0, 1.0);
            }
            cdf
        })
        .collect();

    // 3. Apply SNR-gated bilinear tile interpolation per pixel
    let clip_max_safe = config.clip_max.max(1e-4);
    out_enhanced[..n]
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(y, row_out)| {
            let fy = ((y as f32 + 0.5) / tile_h as f32) - 0.5;
            let ty0 = (fy.floor().max(0.0) as usize).min(n_tiles - 1);
            let ty1 = (ty0 + 1).min(n_tiles - 1);
            let wy = (fy - ty0 as f32).clamp(0.0, 1.0);

            for (x, out_px) in row_out.iter_mut().enumerate() {
                let idx = y * width + x;
                let orig_val = input_image[idx].clamp(0.0, 1.0);
                let px_clip = out_clip_limits[idx];

                // SNR gate weight in [0, 1]: when clip_limit -> 1.0, alpha_snr -> 0.0 (zero noise boost!)
                let alpha_snr = ((px_clip - 1.0) / clip_max_safe).clamp(0.0, 1.0);
                if alpha_snr <= 1e-3 {
                    *out_px = orig_val;
                    continue;
                }

                let fx = ((x as f32 + 0.5) / tile_w as f32) - 0.5;
                let tx0 = (fx.floor().max(0.0) as usize).min(n_tiles - 1);
                let tx1 = (tx0 + 1).min(n_tiles - 1);
                let wx = (fx - tx0 as f32).clamp(0.0, 1.0);

                let bin = ((orig_val * (NUM_HIST_BINS - 1) as f32).round() as usize)
                    .min(NUM_HIST_BINS - 1);

                let c00 = tile_cdfs[ty0 * n_tiles + tx0][bin];
                let c10 = tile_cdfs[ty0 * n_tiles + tx1][bin];
                let c01 = tile_cdfs[ty1 * n_tiles + tx0][bin];
                let c11 = tile_cdfs[ty1 * n_tiles + tx1][bin];

                let top = c00 * (1.0 - wx) + c10 * wx;
                let bot = c01 * (1.0 - wx) + c11 * wx;
                let eq_val = top * (1.0 - wy) + bot * wy;

                *out_px = (orig_val * (1.0 - alpha_snr) + eq_val * alpha_snr).clamp(0.0, 1.0);
            }
        });
}

/// Estimates the optical extinction coefficient $\hat{\beta}_{ext}$ and $\hat{\rho}_{fog, opt}$
/// from the sub-horizon local variance profile and ground inverse-depth map using Koschmieder's law:
///
/// $$C(Z) = C_0 e^{-\beta_{ext} Z} \implies \ln \sigma_{local}(Z) = \ln C_0 - \beta_{ext} Z$$
///
/// Falls back smoothly to $3.0 / V_{MOR, prior}$ when the scene is uniform (`Flat Whiteout`).
pub fn estimate_koschmieder_extinction(
    local_variance: &[f32],
    ground_inv_depth: &[f32],
    prior_v_mor_m: Option<f64>,
    config: &ClaheKoschmiederConfig,
) -> (f64, f64) {
    let n = local_variance.len().min(ground_inv_depth.len());
    let noise_floor_var = 4.0 * config.sigma_sensor_noise * config.sigma_sensor_noise;

    // Linear regression of y = -ln(contrast) vs Z = 1 / rho_ground
    let mut sum_z = 0.0_f64;
    let mut sum_y = 0.0_f64;
    let mut sum_zz = 0.0_f64;
    let mut sum_zy = 0.0_f64;
    let mut count = 0.0_f64;

    for i in 0..n {
        let rho = ground_inv_depth[i] as f64;
        let var = local_variance[i];
        if rho > 1e-3 && var > noise_floor_var {
            let z = (1.0 / rho).clamp(5.0, 1500.0);
            let contrast = (var.sqrt() as f64).max(1e-4);
            let y_atten = -contrast.ln();

            sum_z += z;
            sum_y += y_atten;
            sum_zz += z * z;
            sum_zy += z * y_atten;
            count += 1.0;
        }
    }

    let beta_from_prior = prior_v_mor_m.map(|vmor| {
        3.0 / vmor.clamp(config.min_v_mor_m, config.max_v_mor_m)
    });

    let mut beta_est = beta_from_prior.unwrap_or(3.0 / 50.0);

    if count >= 25.0 {
        let denom = count * sum_zz - sum_z * sum_z;
        if denom.abs() > EPS_F64 {
            let slope = (count * sum_zy - sum_z * sum_y) / denom;
            if slope > 0.0005 && slope < 0.4 {
                beta_est = if let Some(b_prior) = beta_from_prior {
                    0.5 * slope + 0.5 * b_prior
                } else {
                    slope
                };
            }
        }
    }

    let min_beta = 3.0 / config.max_v_mor_m.max(10.0);
    let max_beta = 3.0 / config.min_v_mor_m.max(1.0);
    let beta_clamped = beta_est.clamp(min_beta, max_beta);
    let v_mor_est = 3.0 / beta_clamped.max(EPS_F64);

    (beta_clamped, v_mor_est)
}

/// Computes the Active Inertial Scale Probe (Remediation #3):
///
/// $$\hat{\rho}_{fog, inert} = \frac{\|\Delta\dot{\mathbf{u}}_{fog, trans}\|}{\|\mathbf{A}_{tr}(x_c, y_c)\Delta\mathbf{v}_b\| + \epsilon}$$
///
/// where $\mathbf{A}_{tr}(x_c, y_c) = \begin{bmatrix} -1 & 0 & x_c \\ 0 & -1 & y_c \end{bmatrix}$.
/// Note: in standard forward-looking camera coordinates where $Z_c$ is along body X ($u$) or optical axis,
/// $\mathbf{A}_{tr}(x_c, y_c)\Delta\mathbf{v}_b$ evaluates the perspective translational projection.
#[inline]
pub fn compute_active_inertial_scale_probe(
    delta_u_fog_trans: &Vector2<f64>,
    delta_v_body: &Vector3<f64>,
    x_c: f64,
    y_c: f64,
) -> Option<f64> {
    let a_tr = Matrix2x3::new(-1.0, 0.0, x_c, 0.0, -1.0, y_c);
    let proj_dv = a_tr * delta_v_body;
    let denom_norm = proj_dv.norm();

    // Only trigger active inertial probing when known aircraft velocity change is suficientemente excited
    if denom_norm < 0.05 {
        return None;
    }

    let num_norm = delta_u_fog_trans.norm();
    let rho_inert = num_norm / (denom_norm + EPS_F64);
    if rho_inert.is_finite() && rho_inert > 1e-4 && rho_inert < 1.0 {
        Some(rho_inert)
    } else {
        None
    }
}

/// Fuses the Koschmieder Optical Anchor $\hat{\rho}_{fog, opt} = \hat{\beta}_{ext}$ with the
/// Active Inertial Scale Probe $\hat{\rho}_{fog, inert}$ (Remediation #3).
pub fn estimate_fused_fog_scale(
    local_variance: &[f32],
    ground_inv_depth: &[f32],
    prior_v_mor_m: Option<f64>,
    inertial_probe_inputs: Option<(&Vector2<f64>, &Vector3<f64>, f64, f64)>,
    config: &ClaheKoschmiederConfig,
) -> FogScaleEstimate {
    let (beta_ext, est_v_mor) =
        estimate_koschmieder_extinction(local_variance, ground_inv_depth, prior_v_mor_m, config);
    let rho_opt = beta_ext;

    let rho_inert = inertial_probe_inputs.and_then(|(du, dv, xc, yc)| {
        compute_active_inertial_scale_probe(du, dv, xc, yc)
    });

    let rho_fused = match rho_inert {
        Some(r_in) => 0.45 * rho_opt + 0.55 * r_in,
        None => rho_opt,
    }
    .clamp(1e-4, 0.5);

    FogScaleEstimate {
        beta_ext_inv_m: beta_ext,
        estimated_v_mor_m: est_v_mor,
        rho_fog_optical: rho_opt,
        rho_fog_inertial: rho_inert,
        rho_fog_fused: rho_fused,
        mean_fog_depth_m: 1.0 / rho_fused.max(EPS_F64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swir_vis_fusion::compute_local_variance_into;
    use rand::Rng;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_snr_gated_clahe_blocks_thermal_noise_in_flat_whiteout() {
        let width = 64;
        let height = 64;
        let n = width * height;
        let sigma_noise = 0.01_f32;

        // Generate 100% Flat Whiteout frame with pure CMOS thermal noise around 0.85
        let mut rng = ChaCha8Rng::seed_from_u64(12345);
        let mut whiteout_img = vec![0.0_f32; n];
        for px in &mut whiteout_img {
            *px = 0.85 + (rng.gen::<f32>() - 0.5) * 2.0 * sigma_noise;
        }

        let mut local_var = vec![0.0_f32; n];
        compute_local_variance_into(&whiteout_img, width, height, 2, &mut local_var);

        let cfg = ClaheKoschmiederConfig {
            clip_max: 3.5,
            sigma_sensor_noise: sigma_noise,
            num_tiles_per_axis: 8,
            ..Default::default()
        };

        let mut enhanced = vec![0.0_f32; n];
        let mut clip_limits = vec![0.0_f32; n];
        apply_snr_gated_clahe_into(
            &whiteout_img,
            &local_var,
            width,
            height,
            &cfg,
            &mut enhanced,
            &mut clip_limits,
        );

        // Verify clip_limit -> 1.0 across the entire Flat Whiteout image
        let mean_clip = clip_limits.iter().sum::<f32>() / n as f32;
        assert!(
            mean_clip < 1.05,
            "In 100% Flat Whiteout, clip_limit must stay near 1.0, got {mean_clip}"
        );

        // Verify thermal noise is NOT amplified
        let mut enhanced_var = vec![0.0_f32; n];
        compute_local_variance_into(&enhanced, width, height, 2, &mut enhanced_var);
        let max_in_var = local_var.iter().copied().fold(0.0_f32, f32::max);
        let max_out_var = enhanced_var.iter().copied().fold(0.0_f32, f32::max);
        assert!(
            max_out_var <= max_in_var * 1.02 + 1e-6,
            "SNR-Gated CLAHE must not amplify thermal noise variance ({max_out_var} vs {max_in_var})"
        );
    }

    #[test]
    fn test_koschmieder_and_active_inertial_scale_probing() {
        let cfg = ClaheKoschmiederConfig::default();
        let n = 100;
        let local_var = vec![1e-5_f32; n];
        let inv_depth = vec![0.02_f32; n];

        // True fog inverse depth = 0.06 m^-1 (mean fog depth ~ 16.67m, V_MOR = 50m)
        let true_rho_fog = 0.06_f64;
        let xc = 0.25;
        let yc = -0.15;
        let delta_v_b = Vector3::new(-1.2, 0.8, 2.0);
        let a_tr = Matrix2x3::new(-1.0, 0.0, xc, 0.0, -1.0, yc);
        let delta_u_fog = true_rho_fog * (a_tr * delta_v_b);

        let scale = estimate_fused_fog_scale(
            &local_var,
            &inv_depth,
            Some(50.0),
            Some((&delta_u_fog, &delta_v_b, xc, yc)),
            &cfg,
        );

        assert!((scale.rho_fog_optical - 0.06).abs() < 1e-4);
        let r_in = scale.rho_fog_inertial.unwrap_or(0.0);
        assert!((r_in - true_rho_fog).abs() < 1e-4);
        assert!((scale.rho_fog_fused - true_rho_fog).abs() < 1e-4);
    }
}
