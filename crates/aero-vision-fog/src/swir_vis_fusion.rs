//! Dual-Band Visible ($0.4\text{–}0.7\,\mu\text{m}$) + SWIR/LWIR ($0.9\text{–}14\,\mu\text{m}$)
//! Adaptive Contrast-Weighted Sensor Fusion and Local Variance Estimation.
//!
//! In radiation fog characteristic of the Jiu River valley and Craiova Airport (LRCV),
//! water droplet radii $r \in [5, 15]\,\mu\text{m}$ cause severe Mie scattering in the
//! Visible band ($\beta_{VIS} = 3.0 / V_{MOR}$), whereas the SWIR/LWIR band experiences
//! $\beta_{SWIR} \approx 0.22\,\beta_{VIS}$ ($4.5\times$ deeper optical range).

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

const EPS_F32: f32 = 1e-6;

/// Configuration for Dual-Band Visible + SWIR/LWIR adaptive fusion.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SwirVisFusionConfig {
    /// Half-window radius for local variance estimation (e.g., `radius = 2` -> $5 \times 5$ window).
    pub window_radius: usize,
    /// Minimum baseline weight assigned to SWIR/LWIR in dense fog ($w_{SWIR,\min} \in [0.5, 0.95]$).
    pub min_swir_weight: f32,
    /// Ratio $\beta_{SWIR} / \beta_{VIS}$ according to Mie scattering theory ($\approx 0.22$).
    pub mie_swir_to_vis_ratio: f32,
}

impl Default for SwirVisFusionConfig {
    fn default() -> Self {
        Self {
            window_radius: 2,
            min_swir_weight: 0.65,
            mie_swir_to_vis_ratio: 0.22,
        }
    }
}

/// Output summary of Dual-Band fusion.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct FusionMetrics {
    /// Mean local variance in the Visible band $\bar{\sigma}_{VIS}^2$.
    pub mean_var_vis: f32,
    /// Mean local variance in the SWIR/LWIR band $\bar{\sigma}_{SWIR}^2$.
    pub mean_var_swir: f32,
    /// Mean local variance of the fused output image $\bar{\sigma}_{fused}^2$.
    pub mean_var_fused: f32,
    /// Average adaptive weight assigned to the SWIR/LWIR channel $\bar{w}_{SWIR} \in [0, 1]$.
    pub mean_swir_weight: f32,
}

/// Computes the local intensity variance $\sigma_{local}^2(x, y)$ over a $(2R+1) \times (2R+1)$ window
/// in parallel using `rayon`, writing directly into `out_variance` (zero heap allocations in the inner loop).
pub fn compute_local_variance_into(
    image: &[f32],
    width: usize,
    height: usize,
    radius: usize,
    out_variance: &mut [f32],
) {
    let n = width * height;
    if image.len() < n || out_variance.len() < n || width == 0 || height == 0 {
        return;
    }
    let r = radius.clamp(1, 8);

    out_variance[..n]
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(y, row_out)| {
            let y0 = y.saturating_sub(r);
            let y1 = (y + r).min(height - 1);

            for (x, out_val) in row_out.iter_mut().enumerate() {
                let x0 = x.saturating_sub(r);
                let x1 = (x + r).min(width - 1);

                let mut sum = 0.0_f32;
                let mut sum_sq = 0.0_f32;
                let mut count = 0.0_f32;

                for wy in y0..=y1 {
                    let row_offset = wy * width;
                    for wx in x0..=x1 {
                        let v = image[row_offset + wx];
                        sum += v;
                        sum_sq += v * v;
                        count += 1.0;
                    }
                }

                let inv_n = 1.0 / count.max(1.0);
                let mean = sum * inv_n;
                let var = (sum_sq * inv_n - mean * mean).max(0.0);
                *out_val = var;
            }
        });
}

/// Performs adaptive Dual-Band (Visible + SWIR/LWIR) fusion into pre-allocated buffers.
///
/// For each pixel $(x, y)$, the SWIR weight increases both with local contrast ratio
/// $\frac{\sigma_{SWIR}^2(x,y)}{\sigma_{VIS}^2(x,y) + \sigma_{SWIR}^2(x,y) + \epsilon}$
/// and with the Mie transmission advantage $T_{SWIR}(Z) = T_{VIS}(Z)^{0.22}$.
#[allow(clippy::too_many_arguments)]
pub fn fuse_dual_band_into(
    vis_band: &[f32],
    swir_band: &[f32],
    width: usize,
    height: usize,
    config: &SwirVisFusionConfig,
    var_vis_buf: &mut [f32],
    var_swir_buf: &mut [f32],
    out_fused: &mut [f32],
    out_fused_var: &mut [f32],
) -> FusionMetrics {
    let n = width * height;
    if vis_band.len() < n
        || swir_band.len() < n
        || var_vis_buf.len() < n
        || var_swir_buf.len() < n
        || out_fused.len() < n
        || out_fused_var.len() < n
        || n == 0
    {
        return FusionMetrics::default();
    }

    compute_local_variance_into(vis_band, width, height, config.window_radius, var_vis_buf);
    compute_local_variance_into(swir_band, width, height, config.window_radius, var_swir_buf);

    // Transmission advantage prior from Mie scattering (beta_SWIR = 0.22 * beta_VIS => 4.54x penetration)
    let mie_prior_swir = (1.0 / (1.0 + config.mie_swir_to_vis_ratio.max(0.05)))
        .clamp(config.min_swir_weight, 0.95);

    let (sum_w_swir, sum_v_vis, sum_v_swir) = out_fused[..n]
        .par_iter_mut()
        .enumerate()
        .map(|(idx, fused_px)| {
            let v_vis = var_vis_buf[idx].max(0.0);
            let v_swir = var_swir_buf[idx].max(0.0);

            // Local contrast-driven weight
            let contrast_w_swir = (v_swir + EPS_F32 * mie_prior_swir)
                / (v_vis + v_swir + EPS_F32);

            // Combine Mie physical prior with local SNR variance ratio
            let w_swir = (0.5 * mie_prior_swir + 0.5 * contrast_w_swir)
                .clamp(config.min_swir_weight, 0.98);
            let w_vis = 1.0 - w_swir;

            *fused_px = (w_vis * vis_band[idx] + w_swir * swir_band[idx]).clamp(0.0, 1.0);
            (w_swir as f64, v_vis as f64, v_swir as f64)
        })
        .reduce(
            || (0.0_f64, 0.0_f64, 0.0_f64),
            |(a0, a1, a2), (b0, b1, b2)| (a0 + b0, a1 + b1, a2 + b2),
        );

    // Compute local variance of the resulting fused image for downstream SNR-Gated CLAHE
    compute_local_variance_into(out_fused, width, height, config.window_radius, out_fused_var);

    let inv_n = 1.0 / (n as f64).max(1.0);
    let mean_var_fused =
        (out_fused_var[..n].par_iter().map(|&v| v as f64).sum::<f64>() * inv_n) as f32;

    FusionMetrics {
        mean_var_vis: (sum_v_vis * inv_n) as f32,
        mean_var_swir: (sum_v_swir * inv_n) as f32,
        mean_var_fused,
        mean_swir_weight: (sum_w_swir * inv_n) as f32,
    }
}

/// Convenience wrapper that allocates output buffers and runs Dual-Band fusion.
pub fn fuse_dual_band(
    vis_band: &[f32],
    swir_band: &[f32],
    width: usize,
    height: usize,
    config: &SwirVisFusionConfig,
) -> (Vec<f32>, Vec<f32>, FusionMetrics) {
    let n = width * height;
    let mut var_vis = vec![0.0_f32; n];
    let mut var_swir = vec![0.0_f32; n];
    let mut fused = vec![0.0_f32; n];
    let mut fused_var = vec![0.0_f32; n];

    let metrics = fuse_dual_band_into(
        vis_band,
        swir_band,
        width,
        height,
        config,
        &mut var_vis,
        &mut var_swir,
        &mut fused,
        &mut fused_var,
    );
    (fused, fused_var, metrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_variance_flat_vs_textured() {
        let width = 32;
        let height = 32;
        let flat = vec![0.75_f32; width * height];
        let mut var_out = vec![0.0_f32; width * height];

        compute_local_variance_into(&flat, width, height, 2, &mut var_out);
        for &v in &var_out {
            assert!(v.abs() < 1e-6, "Flat image must have zero local variance, got {v}");
        }

        // Checkerboard pattern with amplitude +/- 0.2 around 0.5 -> variance ~ 0.04
        let mut checker = vec![0.0_f32; width * height];
        for y in 0..height {
            for x in 0..width {
                checker[y * width + x] = if (x + y) % 2 == 0 { 0.7 } else { 0.3 };
            }
        }
        compute_local_variance_into(&checker, width, height, 2, &mut var_out);
        let center_var = var_out[(height / 2) * width + (width / 2)];
        assert!(
            (center_var - 0.04).abs() < 0.005,
            "Checkerboard variance should be ~0.04, got {center_var}"
        );
    }

    #[test]
    fn test_adaptive_swir_vis_fusion_prioritizes_swir_in_fog() {
        let width = 32;
        let height = 32;
        let n = width * height;
        // Visible band washed out by fog (almost flat 0.85)
        let vis = vec![0.85_f32; n];
        // SWIR band sees strong runway structure
        let mut swir = vec![0.4_f32; n];
        for i in 0..n {
            if (i % width) > 12 && (i % width) < 20 {
                swir[i] = 0.92;
            }
        }

        let cfg = SwirVisFusionConfig::default();
        let (fused, fused_var, metrics) = fuse_dual_band(&vis, &swir, width, height, &cfg);

        assert!(metrics.mean_var_vis < 1e-6);
        assert!(metrics.mean_var_swir > 0.01);
        assert!(
            metrics.mean_swir_weight > 0.75,
            "SWIR weight should dominate when VIS is flat fog, got {}",
            metrics.mean_swir_weight
        );
        assert!(metrics.mean_var_fused > 0.005);
        assert_eq!(fused.len(), n);
        assert_eq!(fused_var.len(), n);
    }
}
