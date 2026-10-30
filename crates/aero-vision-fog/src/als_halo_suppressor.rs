//! ALS (Approach Lighting System) Multiple Scattering Mie Bloom Suppressor and
//! Sub-Pixel Photometric Centroid Extractor for LRCV RWY 09 (Remediation #2).
//!
//! ### Problem Addressed (Vulnerability #2):
//! As the aircraft descends below $h_{AGL} = 90\text{ m}$ ($d_{LRCV} < 1.5\text{ km}$ — Phase 2 of the
//! Approach Funnel), the high-intensity RWY 09 ALS approach light crossbars create radial
//! Multiple Scattering Mie Bloom halos in radiation fog. As the aircraft approaches, these halos
//! expand radially on the image plane ($\nabla \cdot \dot{\mathbf{u}} > 0$), which a naive optical
//! flow algorithm misinterprets as a false forward acceleration $+\Delta u$ or an oncoming gust.
//!
//! ### V2.0 Solution:
//! 1. Identify bright ALS cores where $I_{SWIR}(x, y) > 0.88$ and cluster connected/nearby core pixels.
//! 2. Compute the sub-pixel photometric centroid of each ALS core weighted by $I_{SWIR}^2$:
//!    $$\hat{x}_{ALS} = \frac{\sum_{(x,y) \in \text{Core}} x \cdot I_{SWIR}^2(x,y)}{\sum_{(x,y) \in \text{Core}} I_{SWIR}^2(x,y)}, \qquad \hat{y}_{ALS} = \frac{\sum_{(x,y) \in \text{Core}} y \cdot I_{SWIR}^2(x,y)}{\sum_{(x,y) \in \text{Core}} I_{SWIR}^2(x,y)}$$
//! 3. Mask the false-divergence Mie halo annulus $r \in [r_{core}, 4 r_{core}]$ by setting
//!    $M_{bloom}(x, y) = 0.0$, preventing halo expansion from corrupting translational optical flow!

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

const EPS_F64: f64 = 1e-9;

/// Configuration for ALS Mie Halo Suppression and Centroid Extraction.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AlsHaloSuppressorConfig {
    /// Core intensity threshold in the SWIR/LWIR band (default `0.88`).
    pub core_swir_threshold: f32,
    /// Minimum core radius in pixels $r_{core,\min}$ (default `1.5` px).
    pub min_core_radius_px: f32,
    /// Maximum core radius in pixels $r_{core,\max}$ (default `8.0` px).
    pub max_core_radius_px: f32,
    /// Outer multiplier of core radius defining the Mie halo suppression ring $[r_{core}, k \cdot r_{core}]$ (default `4.0`).
    pub halo_outer_multiplier: f32,
    /// Clustering radius in pixels to merge adjacent bright pixels into a single ALS beacon core.
    pub cluster_merge_radius_px: f32,
}

impl Default for AlsHaloSuppressorConfig {
    fn default() -> Self {
        Self {
            core_swir_threshold: 0.88,
            min_core_radius_px: 1.5,
            max_core_radius_px: 8.0,
            halo_outer_multiplier: 4.0,
            cluster_merge_radius_px: 6.5,
        }
    }
}

/// Detected ALS light bar / beacon with sub-pixel centroid and Mie halo geometry.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AlsDetectedBeacon {
    /// Sub-pixel horizontal centroid $\hat{x}_{ALS}$ in pixel coordinates.
    pub x_als_px: f64,
    /// Sub-pixel vertical centroid $\hat{y}_{ALS}$ in pixel coordinates.
    pub y_als_px: f64,
    /// Sub-pixel normalized camera coordinates $(\hat{x}_c, \hat{y}_c) \in [-1, 1]^2$.
    pub center_norm: (f64, f64),
    /// Effective core radius $r_{core}$ in pixels.
    pub r_core_px: f32,
    /// Effective Mie scattering halo radius $r_{halo}(\hat{\beta}_{ext}) = 4\,r_{core}$ in pixels.
    pub r_halo_px: f32,
    /// Peak SWIR intensity inside the core.
    pub peak_swir_intensity: f32,
    /// Number of core pixels above threshold.
    pub core_pixel_count: usize,
}

/// Summary output of the ALS Halo Suppressor.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AlsSuppressionResult {
    /// Detected ALS beacons sorted from bottom (closest) to top (farthest).
    pub beacons: Vec<AlsDetectedBeacon>,
    /// Mean lateral offset of detected ALS centerline in normalized camera coordinates.
    pub mean_lateral_offset_norm: Option<f64>,
    /// Fraction of image pixels masked ($M_{bloom} = 0$) due to Mie halo rings.
    pub masked_pixel_fraction: f32,
}

/// Computes the theoretical Mie halo outer multiplier scale factor from $\hat{\beta}_{ext}$.
#[inline]
pub fn compute_mie_halo_radius_px(
    r_core_px: f32,
    beta_ext_inv_m: f64,
    config: &AlsHaloSuppressorConfig,
) -> f32 {
    // Specification: mask the circular annulus r in [r_core, 4 * r_core], modulated slightly by extinction
    let mie_scale = config.halo_outer_multiplier.max(2.0)
        * (1.0 + 0.05 * (beta_ext_inv_m as f32 - 0.06).clamp(-0.05, 0.25));
    (r_core_px * mie_scale).max(r_core_px * config.halo_outer_multiplier)
}

/// Identifies ALS bright cores ($I_{SWIR} > 0.88$), extracts sub-pixel $I_{SWIR}^2$-weighted centroids,
/// and populates the pre-allocated `out_bloom_mask` ($M_{bloom}(x, y) \in \{0.0, 1.0\}$) where
/// $M_{bloom} = 0.0$ inside every Mie halo annulus $r \in [r_{core}, 4 r_{core}]$.
pub fn suppress_als_halos_into(
    swir_band: &[f32],
    width: usize,
    height: usize,
    beta_ext_inv_m: f64,
    config: &AlsHaloSuppressorConfig,
    out_bloom_mask: &mut [f32],
) -> AlsSuppressionResult {
    let n = width * height;
    if swir_band.len() < n || out_bloom_mask.len() < n || width == 0 || height == 0 {
        return AlsSuppressionResult::default();
    }

    // Initialize bloom mask to 1.0 (unmasked)
    out_bloom_mask[..n].fill(1.0);

    let thresh = config.core_swir_threshold;
    let merge_r_sq = (config.cluster_merge_radius_px * config.cluster_merge_radius_px) as f64;

    // Accumulator for greedy single-pass connected/proximity clustering of core pixels (I_SWIR > 0.88)
    // Tuple: (sum_w_x, sum_w_y, sum_w, min_x, max_x, min_y, max_y, peak_i, count)
    let mut clusters: Vec<(f64, f64, f64, usize, usize, usize, usize, f32, usize)> =
        Vec::with_capacity(16);

    for y in 0..height {
        let row_off = y * width;
        for x in 0..width {
            let val = swir_band[row_off + x];
            if val > thresh {
                let w_sq = (val as f64) * (val as f64);
                let fx = x as f64;
                let fy = y as f64;

                let mut found_cluster = None;
                for (c_idx, c) in clusters.iter().enumerate() {
                    let cx = c.0 / c.2.max(EPS_F64);
                    let cy = c.1 / c.2.max(EPS_F64);
                    let dist_sq = (fx - cx) * (fx - cx) + (fy - cy) * (fy - cy);
                    if dist_sq <= merge_r_sq {
                        found_cluster = Some(c_idx);
                        break;
                    }
                }

                if let Some(c_idx) = found_cluster {
                    let c = &mut clusters[c_idx];
                    c.0 += fx * w_sq;
                    c.1 += fy * w_sq;
                    c.2 += w_sq;
                    c.3 = c.3.min(x);
                    c.4 = c.4.max(x);
                    c.5 = c.5.min(y);
                    c.6 = c.6.max(y);
                    c.7 = c.7.max(val);
                    c.8 += 1;
                } else if clusters.len() < 64 {
                    clusters.push((fx * w_sq, fy * w_sq, w_sq, x, x, y, y, val, 1));
                }
            }
        }
    }

    if clusters.is_empty() {
        return AlsSuppressionResult::default();
    }

    let mut beacons = Vec::with_capacity(clusters.len());
    let w_f64 = width as f64;
    let h_f64 = height as f64;

    for c in clusters {
        let x_als_px = c.0 / c.2.max(EPS_F64);
        let y_als_px = c.1 / c.2.max(EPS_F64);

        let span_x = (c.4.saturating_sub(c.3) as f32) * 0.5 + 0.75;
        let span_y = (c.6.saturating_sub(c.5) as f32) * 0.5 + 0.75;
        let est_r_core = ((span_x * span_x + span_y * span_y).sqrt() * 0.7071)
            .clamp(config.min_core_radius_px, config.max_core_radius_px);

        // Ensure the annulus [r_core, 4 * r_core] is suppressed
        let r_halo = compute_mie_halo_radius_px(est_r_core, beta_ext_inv_m, config)
            .max(est_r_core * config.halo_outer_multiplier);

        let x_norm = ((x_als_px + 0.5) - 0.5 * w_f64) / (0.5 * w_f64);
        let y_norm = ((y_als_px + 0.5) - 0.5 * h_f64) / (0.5 * h_f64);

        beacons.push(AlsDetectedBeacon {
            x_als_px,
            y_als_px,
            center_norm: (x_norm, y_norm),
            r_core_px: est_r_core,
            r_halo_px: r_halo,
            peak_swir_intensity: c.7,
            core_pixel_count: c.8,
        });
    }

    // Sort beacons from bottom of image (closest to aircraft) to top
    beacons.sort_by(|a, b| {
        b.y_als_px
            .partial_cmp(&a.y_als_px)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Apply circular annulus mask M_bloom(x, y) = 0.0 for r in [r_core, 4 * r_core] in parallel
    let masked_count: usize = out_bloom_mask[..n]
        .par_chunks_mut(width)
        .enumerate()
        .map(|(y, row_mask)| {
            let fy = y as f64;
            let mut row_masked = 0usize;
            for (x, m_out) in row_mask.iter_mut().enumerate() {
                let fx = x as f64;
                for b in &beacons {
                    let dx = (fx - b.x_als_px) as f32;
                    let dy = (fy - b.y_als_px) as f32;
                    let r = (dx * dx + dy * dy).sqrt();
                    let r_outer = b.r_core_px * config.halo_outer_multiplier;
                    if r >= b.r_core_px && r <= r_outer.max(b.r_halo_px) {
                        *m_out = 0.0;
                        row_masked += 1;
                        break;
                    }
                }
            }
            row_masked
        })
        .sum();

    let mean_lateral_offset_norm = if !beacons.is_empty() {
        let sum_x: f64 = beacons.iter().map(|b| b.center_norm.0).sum();
        Some(sum_x / beacons.len() as f64)
    } else {
        None
    };

    AlsSuppressionResult {
        beacons,
        mean_lateral_offset_norm,
        masked_pixel_fraction: masked_count as f32 / (n as f32).max(1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subpixel_als_centroid_and_annular_halo_masking() {
        let width = 64;
        let height = 64;
        let n = width * height;
        let mut swir = vec![0.30_f32; n];

        // Synthesize an ALS beacon centered at fractional sub-pixel coordinate (31.35, 42.65)
        let true_cx = 31.35_f64;
        let true_cy = 42.65_f64;
        let r_core = 2.2_f64;
        let r_halo = 8.8_f64; // 4 * r_core

        for y in 0..height {
            for x in 0..width {
                let dx = x as f64 - true_cx;
                let dy = y as f64 - true_cy;
                let r = (dx * dx + dy * dy).sqrt();
                if r <= r_core {
                    // High-intensity symmetrical core > 0.88
                    let profile = 0.99 - 0.08 * (r / r_core).powi(2);
                    swir[y * width + x] = profile as f32;
                } else if r <= r_halo {
                    // Multiple Scattering Mie Bloom halo (< 0.88)
                    let bloom = 0.82 * (-(r - r_core) / 3.5).exp();
                    swir[y * width + x] = (0.30 + bloom).min(0.85) as f32;
                }
            }
        }

        let cfg = AlsHaloSuppressorConfig::default();
        let mut bloom_mask = vec![1.0_f32; n];
        let res = suppress_als_halos_into(&swir, width, height, 0.08, &cfg, &mut bloom_mask);

        assert_eq!(res.beacons.len(), 1, "Must detect exactly 1 ALS beacon core");
        let b = res.beacons[0];

        // Sub-pixel accuracy verification
        let err_x = (b.x_als_px - true_cx).abs();
        let err_y = (b.y_als_px - true_cy).abs();
        assert!(
            err_x < 0.25 && err_y < 0.25,
            "Sub-pixel centroid error too large: ({}, {}) vs ({true_cx}, {true_cy})",
            b.x_als_px,
            b.y_als_px
        );

        // Verify that pixels in the Mie halo ring [r_core, 4 * r_core] have M_bloom == 0.0
        for y in 0..height {
            for x in 0..width {
                let dx = x as f64 - b.x_als_px;
                let dy = y as f64 - b.y_als_px;
                let r = (dx * dx + dy * dy).sqrt() as f32;
                if r >= b.r_core_px + 0.1 && r <= 4.0 * b.r_core_px - 0.1 {
                    assert_eq!(
                        bloom_mask[y * width + x],
                        0.0,
                        "Pixel ({x}, {y}) at radius {r} inside [r_core={}, 4*r_core={}] must have M_bloom = 0.0",
                        b.r_core_px,
                        4.0 * b.r_core_px
                    );
                }
            }
        }
        assert!(res.masked_pixel_fraction > 0.02);
    }
}
