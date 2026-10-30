//! 3-Frame Spatio-Temporal Structure Tensor $(t-2, t-1, t)$, Photonic Certainty Weight $w_{photon, i}$,
//! and Anisotropic Fisher Information Matrix $\mathbf{R}_{opt, i}^{-1}$ (Remediations #1 & #4).
//!
//! Simultaneously solves:
//! - **Vulnerability #1 (`Flat Whiteout` Sensor Noise Hallucination)**: When $\lambda_{1,i} < 3\sigma_{sensor}^2$,
//!   $w_{photon, i} = 0.0$ exactly, cutting off CMOS thermal noise before it enters the ES-EKF.
//! - **Vulnerability #4 (*Aperture Problem* on Runway Longitudinal Markings)**:
//!   $$\mathbf{R}_{opt, i}^{-1} = w_{photon, i} \left( \lambda_{1,i}\,\mathbf{e}_{1,i}\mathbf{e}_{1,i}^T + \lambda_{2,i}\,\mathbf{e}_{2,i}\mathbf{e}_{2,i}^T \right)$$
//!   Longitudinal lines ($\lambda_1 \gg \lambda_2 \approx 0$) constrain only lateral drift $v$ and yaw $\psi$,
//!   while transverse ALS crossbars and RWY 09 "Zebra" piano keys ($\lambda_1 \approx \lambda_2 \gg 0$)
//!   provide full rank-2 observability to lock longitudinal velocity $u$ and threshold distance.

use nalgebra::{Matrix2, Vector2};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

const EPS_F64: f64 = 1e-12;

/// Pre-allocated circular buffer holding the last 3 frames $(t-2, t-1, t)$ and their spatial gradients
/// $(I_x, I_y)$ with zero dynamic memory allocations during flight.
#[derive(Debug, Clone)]
pub struct RingBuffer3Frames {
    pub width: usize,
    pub height: usize,
    frames: [Vec<f32>; 3],
    grad_x: [Vec<f32>; 3],
    grad_y: [Vec<f32>; 3],
    head: usize,
    count: usize,
}

impl RingBuffer3Frames {
    /// Allocates a new 3-frame ring buffer for `width x height` images.
    pub fn new(width: usize, height: usize) -> Self {
        let n = width * height;
        Self {
            width,
            height,
            frames: [vec![0.0; n], vec![0.0; n], vec![0.0; n]],
            grad_x: [vec![0.0; n], vec![0.0; n], vec![0.0; n]],
            grad_y: [vec![0.0; n], vec![0.0; n], vec![0.0; n]],
            head: 0,
            count: 0,
        }
    }

    /// Number of valid frames currently stored ($0 \dots 3$).
    #[inline]
    pub fn len(&self) -> usize {
        self.count
    }

    /// Returns true if no frames have been pushed yet.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Resets the ring buffer state without deallocating memory.
    pub fn clear(&mut self) {
        self.head = 0;
        self.count = 0;
    }

    /// Pushes a new frame $I(u, v, t)$ into the circular buffer and computes its central spatial
    /// gradients $(I_x, I_y)$ in parallel using `rayon`.
    pub fn push_frame(&mut self, image: &[f32]) {
        let n = self.width * self.height;
        if image.len() < n || n == 0 {
            return;
        }

        let slot = self.head;
        self.frames[slot][..n].copy_from_slice(&image[..n]);

        let w = self.width;
        let h = self.height;
        let img_ref = &self.frames[slot];
        let gx_slot = &mut self.grad_x[slot];
        let gy_slot = &mut self.grad_y[slot];

        gx_slot[..n]
            .par_chunks_mut(w)
            .zip(gy_slot[..n].par_chunks_mut(w))
            .enumerate()
            .for_each(|(y, (row_gx, row_gy))| {
                let y_prev = y.saturating_sub(1);
                let y_next = (y + 1).min(h - 1);

                for x in 0..w {
                    let x_prev = x.saturating_sub(1);
                    let x_next = (x + 1).min(w - 1);

                    // Scharr / Sobel-like smoothed central difference to suppress high-frequency shot noise
                    let ix_center = 0.5 * (img_ref[y * w + x_next] - img_ref[y * w + x_prev]);
                    let ix_up = 0.5 * (img_ref[y_prev * w + x_next] - img_ref[y_prev * w + x_prev]);
                    let ix_dn = 0.5 * (img_ref[y_next * w + x_next] - img_ref[y_next * w + x_prev]);
                    row_gx[x] = 0.25 * ix_up + 0.50 * ix_center + 0.25 * ix_dn;

                    let iy_center = 0.5 * (img_ref[y_next * w + x] - img_ref[y_prev * w + x]);
                    let iy_left =
                        0.5 * (img_ref[y_next * w + x_prev] - img_ref[y_prev * w + x_prev]);
                    let iy_right =
                        0.5 * (img_ref[y_next * w + x_next] - img_ref[y_prev * w + x_next]);
                    row_gy[x] = 0.25 * iy_left + 0.50 * iy_center + 0.25 * iy_right;
                }
            });

        self.head = (self.head + 1) % 3;
        if self.count < 3 {
            self.count += 1;
        }
    }

    /// Returns a reference to the latest frame $t$ and its spatial gradients $(I_x, I_y)$.
    #[inline]
    pub fn latest_frame_and_gradients(&self) -> Option<(&[f32], &[f32], &[f32])> {
        if self.count == 0 {
            return None;
        }
        let idx = (self.head + 2) % 3;
        Some((&self.frames[idx], &self.grad_x[idx], &self.grad_y[idx]))
    }

    /// Returns a reference to the previous frame $t-1$ and its spatial gradients $(I_x, I_y)$.
    #[inline]
    pub fn previous_frame_and_gradients(&self) -> Option<(&[f32], &[f32], &[f32])> {
        if self.count < 2 {
            return None;
        }
        let idx = (self.head + 1) % 3;
        Some((&self.frames[idx], &self.grad_x[idx], &self.grad_y[idx]))
    }

    /// Returns the internal slices of active slots up to `self.count`.
    #[inline]
    pub fn active_slots(&self) -> impl Iterator<Item = (&[f32], &[f32])> {
        let c = self.count;
        let h = self.head;
        (0..c).map(move |k| {
            let idx = (h + 3 - c + k) % 3;
            (self.grad_x[idx].as_slice(), self.grad_y[idx].as_slice())
        })
    }
}

/// Configuration for the 3-Frame Structure Tensor and Photonic Certainty Gate.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StructureTensorConfig {
    /// Patch size in pixels along each axis (e.g. `8` -> $16 \times 16$ patches on a $128 \times 128$ image,
    /// or `2` -> $64 \times 64$ patches on a $128 \times 128$ image).
    pub patch_size_px: usize,
    /// Sensor thermal noise standard deviation $\sigma_{sensor}$.
    pub sigma_sensor_noise: f64,
    /// Forward-backward cycle consistency standard deviation $\sigma_{cycle}$ (in pixels or rad/s).
    pub sigma_cycle: f64,
}

impl Default for StructureTensorConfig {
    fn default() -> Self {
        Self {
            patch_size_px: 8,
            sigma_sensor_noise: 0.008,
            sigma_cycle: 0.35,
        }
    }
}

/// Spatio-temporal structure tensor $\mathbf{M}_i$, eigenvalues $(\lambda_1, \lambda_2)$,
/// orthonormal eigenvectors $(\mathbf{e}_1, \mathbf{e}_2)$, photonic certainty $w_{photon, i}$,
/// and Anisotropic Fisher Information Matrix $\mathbf{R}_{opt, i}^{-1}$ for a single patch $i$.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PatchStructureTensor {
    /// Patch grid column index.
    pub grid_x: usize,
    /// Patch grid row index.
    pub grid_y: usize,
    /// Patch center pixel coordinates $(u_{px}, v_{px})$.
    pub center_px: (f32, f32),
    /// Normalized camera coordinates $(x_c, y_c) \in [-1, 1]^2$.
    pub center_norm: (f64, f64),
    /// Structure tensor entry $J_{xx}$.
    pub j_xx: f64,
    /// Structure tensor entry $J_{xy}$.
    pub j_xy: f64,
    /// Structure tensor entry $J_{yy}$.
    pub j_yy: f64,
    /// Principal eigenvalue $\lambda_{1,i} \ge \lambda_{2,i} \ge 0$.
    pub lambda_1: f64,
    /// Secondary eigenvalue $\lambda_{2,i} \ge 0$.
    pub lambda_2: f64,
    /// Principal unit eigenvector $\mathbf{e}_{1,i}$ (direction of maximum intensity gradient).
    pub e_1: Vector2<f64>,
    /// Secondary unit eigenvector $\mathbf{e}_{2,i}$ (orthogonal to $\mathbf{e}_{1,i}$).
    pub e_2: Vector2<f64>,
    /// Forward-backward optical flow cycle error $e_{cycle} = \|\dot{\mathbf{u}}_{fwd} + \dot{\mathbf{u}}_{bwd}\|_2$.
    pub e_cycle: f64,
    /// Photonic certainty weight $w_{photon, i} \in [0.0, 1.0]$.
    pub w_photon: f64,
    /// Anisotropic Fisher Information Matrix $\mathbf{R}_{opt, i}^{-1} = w_{photon, i}(\lambda_1 \mathbf{e}_1 \mathbf{e}_1^T + \lambda_2 \mathbf{e}_2 \mathbf{e}_2^T)$.
    pub fisher_info: Matrix2<f64>,
}

/// Analytical eigendecomposition of a symmetric $2 \times 2$ positive semi-definite matrix
/// $\begin{bmatrix} J_{xx} & J_{xy} \\ J_{xy} & J_{yy} \end{bmatrix}$:
///
/// $$\lambda_{1,2} = \frac{1}{2}\left( (J_{xx} + J_{yy}) \pm \sqrt{(J_{xx} - J_{yy})^2 + 4 J_{xy}^2} \right)$$
#[inline]
pub fn decompose_structure_tensor_2x2(
    j_xx: f64,
    j_xy: f64,
    j_yy: f64,
) -> (f64, f64, Vector2<f64>, Vector2<f64>) {
    let trace = (j_xx + j_yy).max(0.0);
    let diff = j_xx - j_yy;
    let disc = (diff * diff + 4.0 * j_xy * j_xy).max(0.0).sqrt();

    let lambda_1 = (0.5 * (trace + disc)).max(0.0);
    let lambda_2 = (0.5 * (trace - disc)).max(0.0);

    // Compute unit eigenvector e_1 corresponding to lambda_1
    let e_1 = if j_xy.abs() > EPS_F64 {
        let vx = lambda_1 - j_yy;
        let vy = j_xy;
        let norm = (vx * vx + vy * vy).sqrt().max(EPS_F64);
        Vector2::new(vx / norm, vy / norm)
    } else if j_xx >= j_yy {
        Vector2::new(1.0, 0.0)
    } else {
        Vector2::new(0.0, 1.0)
    };

    // Orthonormal eigenvector e_2 perpendicular to e_1
    let e_2 = Vector2::new(-e_1.y, e_1.x);

    (lambda_1, lambda_2, e_1, e_2)
}

/// Computes the Photonic Certainty Weight $w_{photon, i} \in [0, 1]$ (Section 3.1 / Remediation #1):
///
/// $$w_{photon, i} = \begin{cases} 0.0, & \text{if } \lambda_{1,i} < 3\sigma_{sensor}^2 \\ \frac{\lambda_{1,i} - 3\sigma_{sensor}^2}{\lambda_{1,i} + 10\sigma_{sensor}^2} \cdot \exp\left(-\frac{e_{cycle}^2}{2\sigma_{cycle}^2}\right), & \text{otherwise} \end{cases}$$
#[inline]
pub fn compute_photonic_weight(
    lambda_1: f64,
    sigma_sensor: f64,
    e_cycle: f64,
    sigma_cycle: f64,
) -> f64 {
    let sigma_sq = sigma_sensor * sigma_sensor;
    let threshold = 3.0 * sigma_sq;
    if lambda_1 < threshold || !lambda_1.is_finite() {
        return 0.0;
    }

    let denom = (lambda_1 + 10.0 * sigma_sq).max(EPS_F64);
    let snr_factor = ((lambda_1 - threshold) / denom).clamp(0.0, 1.0);

    let sig_c = sigma_cycle.max(1e-6);
    let cycle_exp = (-0.5 * (e_cycle * e_cycle) / (sig_c * sig_c)).exp();

    (snr_factor * cycle_exp).clamp(0.0, 1.0)
}

/// Computes the Anisotropic Fisher Information Matrix $\mathbf{R}_{opt, i}^{-1}$ (Section 3.2 / Remediation #4):
///
/// $$\mathbf{R}_{opt, i}^{-1} = w_{photon, i} \left( \lambda_{1,i}\,\mathbf{e}_{1,i}\mathbf{e}_{1,i}^T + \lambda_{2,i}\,\mathbf{e}_{2,i}\mathbf{e}_{2,i}^T \right)$$
#[inline]
pub fn compute_anisotropic_fisher_matrix(
    w_photon: f64,
    lambda_1: f64,
    lambda_2: f64,
    e_1: &Vector2<f64>,
    e_2: &Vector2<f64>,
) -> Matrix2<f64> {
    if w_photon <= 0.0 {
        return Matrix2::zeros();
    }
    let outer_1 = e_1 * e_1.transpose();
    let outer_2 = e_2 * e_2.transpose();
    w_photon * (lambda_1 * outer_1 + lambda_2 * outer_2)
}

/// Evaluates the 3-frame spatio-temporal structure tensor across all patches in parallel using `rayon`,
/// writing results into `out_patches`.
pub fn compute_structure_tensors_3f_into(
    ring: &RingBuffer3Frames,
    cycle_errors_per_patch: Option<&[f64]>,
    config: &StructureTensorConfig,
    out_patches: &mut Vec<PatchStructureTensor>,
) {
    let w = ring.width;
    let h = ring.height;
    let patch_sz = config.patch_size_px.clamp(2, 32);
    let grid_w = w / patch_sz;
    let grid_h = h / patch_sz;
    let total_patches = grid_w * grid_h;

    if ring.is_empty() || total_patches == 0 {
        out_patches.clear();
        return;
    }

    let active_frames: Vec<(&[f32], &[f32])> = ring.active_slots().collect();
    let n_active = active_frames.len().max(1) as f64;
    let norm_scale = 1.0 / (n_active * (patch_sz * patch_sz) as f64);

    if out_patches.len() != total_patches {
        out_patches.resize(
            total_patches,
            PatchStructureTensor {
                grid_x: 0,
                grid_y: 0,
                center_px: (0.0, 0.0),
                center_norm: (0.0, 0.0),
                j_xx: 0.0,
                j_xy: 0.0,
                j_yy: 0.0,
                lambda_1: 0.0,
                lambda_2: 0.0,
                e_1: Vector2::new(1.0, 0.0),
                e_2: Vector2::new(0.0, 1.0),
                e_cycle: 0.0,
                w_photon: 0.0,
                fisher_info: Matrix2::zeros(),
            },
        );
    }

    out_patches
        .par_iter_mut()
        .enumerate()
        .for_each(|(patch_idx, patch_out)| {
            let gy = patch_idx / grid_w;
            let gx = patch_idx % grid_w;
            let u0 = gx * patch_sz;
            let v0 = gy * patch_sz;
            let u1 = (u0 + patch_sz).min(w);
            let v1 = (v0 + patch_sz).min(h);

            let mut j_xx = 0.0_f64;
            let mut j_xy = 0.0_f64;
            let mut j_yy = 0.0_f64;

            for &(gx_slice, gy_slice) in &active_frames {
                for v in v0..v1 {
                    let row_off = v * w;
                    for u in u0..u1 {
                        let ix = gx_slice[row_off + u] as f64;
                        let iy = gy_slice[row_off + u] as f64;
                        j_xx += ix * ix;
                        j_xy += ix * iy;
                        j_yy += iy * iy;
                    }
                }
            }

            // Normalize by spatio-temporal patch support so eigenvalues directly compare to sigma_sensor^2
            j_xx *= norm_scale;
            j_xy *= norm_scale;
            j_yy *= norm_scale;

            let (lambda_1, lambda_2, e_1, e_2) = decompose_structure_tensor_2x2(j_xx, j_xy, j_yy);
            let e_cycle = cycle_errors_per_patch
                .and_then(|ce| ce.get(patch_idx).copied())
                .unwrap_or(0.0);

            let w_photon = compute_photonic_weight(
                lambda_1,
                config.sigma_sensor_noise,
                e_cycle,
                config.sigma_cycle,
            );
            let fisher_info =
                compute_anisotropic_fisher_matrix(w_photon, lambda_1, lambda_2, &e_1, &e_2);

            let cx_px = (u0 + u1) as f32 * 0.5 - 0.5;
            let cy_px = (v0 + v1) as f32 * 0.5 - 0.5;
            let cx_norm = ((cx_px as f64 + 0.5) - 0.5 * w as f64) / (0.5 * w as f64);
            let cy_norm = ((cy_px as f64 + 0.5) - 0.5 * h as f64) / (0.5 * h as f64);

            *patch_out = PatchStructureTensor {
                grid_x: gx,
                grid_y: gy,
                center_px: (cx_px, cy_px),
                center_norm: (cx_norm, cy_norm),
                j_xx,
                j_xy,
                j_yy,
                lambda_1,
                lambda_2,
                e_1,
                e_2,
                e_cycle,
                w_photon,
                fisher_info,
            };
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_flat_whiteout_yields_zero_photonic_weight_and_zero_fisher_info() {
        let w = 64;
        let h = 64;
        let sigma_sensor = 0.01_f64;
        let mut ring = RingBuffer3Frames::new(w, h);
        let mut rng = ChaCha8Rng::seed_from_u64(99);

        // Push 3 frames of pure sensor thermal noise in 100% Flat Whiteout
        for _ in 0..3 {
            let mut img = vec![0.85_f32; w * h];
            for px in &mut img {
                *px += (rng.gen::<f32>() - 0.5) * 2.0 * (sigma_sensor as f32);
            }
            ring.push_frame(&img);
        }

        let cfg = StructureTensorConfig {
            patch_size_px: 8,
            sigma_sensor_noise: sigma_sensor,
            sigma_cycle: 0.3,
        };
        let mut patches = Vec::new();
        compute_structure_tensors_3f_into(&ring, None, &cfg, &mut patches);

        assert!(!patches.is_empty());
        for p in &patches {
            assert_eq!(
                p.w_photon, 0.0,
                "Flat whiteout thermal noise must have w_photon == 0.0, got {} (lambda_1={})",
                p.w_photon, p.lambda_1
            );
            assert_eq!(p.fisher_info.norm(), 0.0);
        }
    }

    #[test]
    fn test_longitudinal_runway_line_anisotropic_fisher_solves_aperture_problem() {
        let w = 64;
        let h = 64;
        let mut ring = RingBuffer3Frames::new(w, h);

        // Vertical stripe in image (simulating longitudinal runway marking: I depends only on x, not y)
        let mut img = vec![0.2_f32; w * h];
        for y in 0..h {
            for x in 0..w {
                if (x >= 28 && x <= 35) || (x >= 12 && x <= 15) {
                    img[y * w + x] = 0.95;
                }
            }
        }
        for _ in 0..3 {
            ring.push_frame(&img);
        }

        let cfg = StructureTensorConfig {
            patch_size_px: 8,
            sigma_sensor_noise: 0.005,
            sigma_cycle: 0.3,
        };
        let mut patches = Vec::new();
        compute_structure_tensors_3f_into(&ring, None, &cfg, &mut patches);

        // Find patch straddling the vertical edge around x = 28 (grid_x = 3)
        let edge_patch = patches
            .iter()
            .find(|p| p.grid_x == 3 && p.grid_y == 4)
            .copied()
            .unwrap_or(patches[0]);

        assert!(edge_patch.lambda_1 > 0.005);
        assert!(
            edge_patch.lambda_2 < edge_patch.lambda_1 * 1e-4,
            "1D longitudinal line must have lambda_2 << lambda_1 ({} vs {})",
            edge_patch.lambda_2,
            edge_patch.lambda_1
        );
        assert!(edge_patch.w_photon > 0.7);
        // Fisher info must be strongly rank-1 along x (perpendicular to the vertical line)
        // and zero along y (parallel to the line -> Aperture Problem handled!)
        assert!(edge_patch.fisher_info[(0, 0)] > 0.003);
        assert!(edge_patch.fisher_info[(1, 1)].abs() < 1e-7);
    }

    #[test]
    fn test_transverse_als_and_zebra_corner_full_rank_fisher_info() {
        let w = 64;
        let h = 64;
        let mut ring = RingBuffer3Frames::new(w, h);

        // 2D checkerboard / Zebra & ALS crossbar pattern with both horizontal and vertical gradients
        let mut img = vec![0.2_f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let gx = ((x as f32 * 0.45).sin() > 0.0) as i32;
                let gy = ((y as f32 * 0.45).cos() > 0.0) as i32;
                if gx ^ gy == 1 {
                    img[y * w + x] = 0.95;
                }
            }
        }
        for _ in 0..3 {
            ring.push_frame(&img);
        }

        let cfg = StructureTensorConfig {
            patch_size_px: 8,
            sigma_sensor_noise: 0.005,
            sigma_cycle: 0.3,
        };
        let mut patches = Vec::new();
        compute_structure_tensors_3f_into(&ring, None, &cfg, &mut patches);

        let center_patch = patches[patches.len() / 2];
        assert!(center_patch.lambda_1 > 0.01);
        assert!(
            center_patch.lambda_2 > center_patch.lambda_1 * 0.4,
            "2D corner/Zebra pattern must have strong lambda_2 ({}) and lambda_1 ({})",
            center_patch.lambda_2,
            center_patch.lambda_1
        );
        assert!(center_patch.w_photon > 0.8);
        // Full rank-2 Fisher matrix: determinant > 0
        let det = center_patch.fisher_info.determinant();
        assert!(det > 1e-5, "2D feature Fisher matrix must be full rank 2, det={det}");
    }
}
