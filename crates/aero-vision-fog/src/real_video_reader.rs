//! Real Aviation Video Reader, Physical Dual-Band (VIS + SWIR/Near-IR) Converter,
//! and Real-World CAT III Fog / Crosswind Cockpit Video Benchmark.
//!
//! Strictly adheres to the **Zero-Panic Philosophy** (zero `.unwrap()`, `.expect()`, or `panic!()`
//! in production code). Uses `ffmpeg` subprocess piping to decode real `.mp4` / `.webm` / `.mkv`
//! cockpit landing videos into contiguous RGB24 buffers and processes them through the complete
//! [`VisionFogPipeline`].

use crate::optical_flow_derot::ImuAngularSample;
use crate::synthetic_lrcv_camera::{
    compute_pixel_ground_inverse_depth, ApproachPhase, CameraKinematics6DoF, DualBandFrame,
    DEFAULT_IMG_HEIGHT, DEFAULT_IMG_WIDTH,
};
use crate::{VisionFogPipeline, VisionFrameReport, VisionPipelineConfig};
use nalgebra::Vector3;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use thiserror::Error;

/// Alias for compatibility with dual-band pipeline consumers.
pub type SyntheticDualBandFrame = DualBandFrame;

/// Errors that can occur during real video decoding, dual-band conversion, or benchmark export.
#[derive(Debug, Error)]
pub enum RealVideoError {
    #[error("Video file does not exist or is not readable: {0}")]
    VideoFileNotFound(String),

    #[error("Invalid frame dimensions ({width}x{height}) or RGB24 buffer length ({actual} < {expected})")]
    InvalidBufferLength {
        width: usize,
        height: usize,
        expected: usize,
        actual: usize,
    },

    #[error("Failed to spawn or execute ffmpeg subprocess: {0}")]
    FfmpegExecutionError(#[from] std::io::Error),

    #[error("ffmpeg returned non-zero exit status ({status}): {stderr}")]
    FfmpegNonZeroExit { status: i32, stderr: String },

    #[error("Zero valid video frames decoded from file: {0}")]
    EmptyVideoStream(String),

    #[error("Failed to serialize benchmark report to JSON: {0}")]
    JsonSerializationError(#[from] serde_json::Error),
}

/// Complete telemetry & performance benchmark report for a single real aviation video.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealVideoBenchmarkReport {
    /// Path or filename of the real video file.
    pub video_path: String,
    /// Human-readable description of the real cockpit approach scene.
    pub source_description: String,
    /// Frame width processed by `VisionFogPipeline`.
    pub frame_width: usize,
    /// Frame height processed by `VisionFogPipeline`.
    pub frame_height: usize,
    /// Total number of real consecutive video frames processed.
    pub frames_processed: usize,
    /// Mean execution time per frame (ms).
    pub mean_frame_time_ms: f64,
    /// 99th-percentile execution time per frame (ms).
    pub p99_frame_time_ms: f64,
    /// Real achieved throughput in frames per second (FPS).
    pub real_fps_achieved: f64,
    /// Mean Koschmieder Meteorological Optical Range visibility $\hat{V}_{MOR} = 3.0 / \hat{\beta}_{ext}$ (m).
    pub estimated_mor_visibility_m: f64,
    /// Mean estimated optical extinction coefficient $\hat{\beta}_{ext}$ ($\text{m}^{-1}$).
    pub mean_extinction_beta: f64,
    /// Mean number of valid sub-horizon ground-anchored patches per frame.
    pub mean_valid_ground_patches: f64,
    /// Mean number of valid fog/lookahead patches per frame.
    pub mean_valid_fog_patches: f64,
    /// Mean photonic certainty weight $\bar{w}_{photon} \in [0, 1]$ across all frames.
    pub mean_photonic_weight: f64,
    /// Mean estimated aircraft velocity relative to ground in body frame $[\hat{u}, \hat{v}, \hat{w}]$ (m/s).
    pub mean_ground_velocity_body_mps: [f64; 3],
    /// Mean estimated fog/wind velocity in body frame $[\hat{w}_x, \hat{w}_y, \hat{w}_z]$ (m/s).
    pub mean_fog_wind_velocity_body_mps: [f64; 3],
    /// Total count of ALS approach light beacons detected across all frames.
    pub total_als_beacons_detected: usize,
    /// Average number of ALS approach light beacons detected per frame.
    pub mean_als_beacons_per_frame: f64,
    /// Number of frames that triggered the Optical Gust Lookahead alert.
    pub gust_lookahead_triggers: usize,
    /// Strictly `true` if and only if zero `NaN` or `Inf` values occurred in any output across all frames.
    pub zero_nan_or_inf: bool,
}

/// Aggregated summary across multiple real-world cockpit landing videos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiVideoBenchmarkSummary {
    /// Total number of real videos benchmarked.
    pub total_videos: usize,
    /// Total number of real video frames processed across all videos.
    pub total_frames_processed: usize,
    /// Overall mean processing time per frame (ms).
    pub overall_mean_frame_time_ms: f64,
    /// Overall real achieved FPS across all videos.
    pub overall_fps_achieved: f64,
    /// Strictly `true` if every frame of every video produced zero `NaN` or `Inf`.
    pub all_zero_nan_or_inf: bool,
    /// Individual video reports.
    pub videos: Vec<RealVideoBenchmarkReport>,
}

/// Converts a raw RGB24 frame (`width * height * 3` bytes) into a physical [`DualBandFrame`]:
///
/// 1. **Physical SWIR/Near-IR Approximation (`swir_band`)**:
///    $$I_{SWIR}(x, y) = 0.85\,R(x, y) + 0.15\,G(x, y) \in [0.0, 1.0]$$
///    Longer wavelengths penetrate atmospheric Mie/Rayleigh droplet scattering significantly better
///    and capture the warm thermal/halogen emission of runway ALS approach lights.
/// 2. **Short-Wavelength Visible Band (`vis_band`)**:
///    $$I_{VIS}(x, y) = 0.30\,G(x, y) + 0.70\,B(x, y) \in [0.0, 1.0]$$
///    Shorter blue-green wavelengths experience strong Mie scattering veil in radiation fog.
/// 3. **NaN-Free Ground Inverse Depth (`ground_inv_depth`)**:
///    Evaluates $\rho_{ground}(x_c, y_c) \ge 0$ from the 6-DoF aircraft attitude $(\phi, \theta, h_{AGL})$.
pub fn rgb24_to_dual_band_frame(
    rgb24_bytes: &[u8],
    width: usize,
    height: usize,
    frame_index: u64,
    timestamp_s: f64,
    kin: &CameraKinematics6DoF,
    prior_v_mor_m: f64,
) -> Result<DualBandFrame, RealVideoError> {
    let n_pixels = width.saturating_mul(height);
    let expected_bytes = n_pixels.saturating_mul(3);
    if width == 0 || height == 0 || rgb24_bytes.len() < expected_bytes {
        return Err(RealVideoError::InvalidBufferLength {
            width,
            height,
            expected: expected_bytes,
            actual: rgb24_bytes.len(),
        });
    }

    let w_f64 = width as f64;
    let h_f64 = height as f64;
    let inv_255 = 1.0_f32 / 255.0_f32;

    let mut vis_band = vec![0.0_f32; n_pixels];
    let mut swir_band = vec![0.0_f32; n_pixels];
    let mut ground_inv_depth = vec![0.0_f32; n_pixels];

    vis_band
        .par_chunks_mut(width)
        .zip(swir_band.par_chunks_mut(width))
        .zip(ground_inv_depth.par_chunks_mut(width))
        .enumerate()
        .for_each(|(py, ((vis_row, swir_row), rho_row))| {
            let y_c = ((py as f64 + 0.5) - 0.5 * h_f64) / (0.5 * h_f64);
            let row_byte_off = py * width * 3;

            for px in 0..width {
                let x_c = ((px as f64 + 0.5) - 0.5 * w_f64) / (0.5 * w_f64);
                let b_idx = row_byte_off + px * 3;

                let r = (rgb24_bytes[b_idx] as f32) * inv_255;
                let g = (rgb24_bytes[b_idx + 1] as f32) * inv_255;
                let b = (rgb24_bytes[b_idx + 2] as f32) * inv_255;

                // Long-wavelength channel (Red/Near-IR proxy for SWIR) vs Short-wavelength channel (Blue-Green VIS)
                swir_row[px] = (0.85 * r + 0.15 * g).clamp(0.0, 1.0);
                vis_row[px] = (0.30 * g + 0.70 * b).clamp(0.0, 1.0);

                let rho_g = compute_pixel_ground_inverse_depth(
                    x_c,
                    y_c,
                    kin.phi_rad,
                    kin.theta_rad,
                    kin.h_agl_m,
                );
                rho_row[px] = (rho_g as f32).max(0.0);
            }
        });

    let v_mor = prior_v_mor_m.max(5.0);
    let beta_vis = (3.0 / v_mor) as f32;
    let beta_swir = 0.22 * beta_vis;

    Ok(DualBandFrame {
        width,
        height,
        frame_index,
        timestamp_s,
        vis_band,
        swir_band,
        ground_inv_depth,
        true_als_centers_px: Vec::new(),
        phase: ApproachPhase::from_distance_m(kin.x_lrcv_m),
        beta_vis,
        beta_swir,
    })
}

/// Decodes up to `max_frames` real RGB24 frames scaled to `target_width x target_height`
/// from a video file using an `ffmpeg` subprocess pipe.
pub fn decode_real_video_frames_ffmpeg(
    video_path: &Path,
    target_width: usize,
    target_height: usize,
    max_frames: usize,
) -> Result<Vec<Vec<u8>>, RealVideoError> {
    if !video_path.exists() {
        return Err(RealVideoError::VideoFileNotFound(
            video_path.display().to_string(),
        ));
    }

    let w = target_width.max(16);
    let h = target_height.max(16);
    let n_max = max_frames.clamp(1, 5000);
    let scale_filter = format!("scale={w}:{h}");

    let output = Command::new("ffmpeg")
        .arg("-v")
        .arg("error")
        .arg("-i")
        .arg(video_path.as_os_str())
        .arg("-vf")
        .arg(&scale_filter)
        .arg("-pix_fmt")
        .arg("rgb24")
        .arg("-vframes")
        .arg(n_max.to_string())
        .arg("-f")
        .arg("rawvideo")
        .arg("pipe:1")
        .output()?;

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(RealVideoError::FfmpegNonZeroExit {
            status: code,
            stderr,
        });
    }

    let frame_bytes = w * h * 3;
    if output.stdout.len() < frame_bytes || frame_bytes == 0 {
        return Err(RealVideoError::EmptyVideoStream(
            video_path.display().to_string(),
        ));
    }

    let frames: Vec<Vec<u8>> = output
        .stdout
        .chunks_exact(frame_bytes)
        .map(|chunk| chunk.to_vec())
        .collect();

    if frames.is_empty() {
        return Err(RealVideoError::EmptyVideoStream(
            video_path.display().to_string(),
        ));
    }

    Ok(frames)
}

/// Strictly verifies that every scalar, vector, and matrix in a [`VisionFrameReport`] is finite
/// (zero `NaN` and zero `Inf`).
pub fn is_frame_report_strictly_finite(rep: &VisionFrameReport) -> bool {
    let fm = &rep.fusion_metrics;
    let fs = &rep.fog_scale;
    let me = &rep.motion_estimate;

    let scalars_ok = rep.timestamp_s.is_finite()
        && fm.mean_var_vis.is_finite()
        && fm.mean_var_swir.is_finite()
        && fm.mean_var_fused.is_finite()
        && fm.mean_swir_weight.is_finite()
        && fs.beta_ext_inv_m.is_finite()
        && fs.estimated_v_mor_m.is_finite()
        && fs.rho_fog_optical.is_finite()
        && fs.rho_fog_fused.is_finite()
        && fs.mean_fog_depth_m.is_finite()
        && rep.als_suppression.masked_pixel_fraction.is_finite()
        && me.gust_lead_time_s.is_finite()
        && me.crab_angle_rad.is_finite()
        && rep.mean_photonic_weight.is_finite();

    if !scalars_ok {
        return false;
    }

    let vectors_ok = me.preintegrated_omega_radps.iter().all(|v| v.is_finite())
        && me.v_ground_body_mps.iter().all(|v| v.is_finite())
        && me.v_ground_fisher_info.iter().all(|v| v.is_finite())
        && me.w_fog_body_mps.iter().all(|v| v.is_finite())
        && me.w_gust_lookahead_mps.iter().all(|v| v.is_finite())
        && me.gust_shear_delta_mps.iter().all(|v| v.is_finite());

    if !vectors_ok {
        return false;
    }

    rep.als_suppression.beacons.iter().all(|b| {
        b.x_als_px.is_finite()
            && b.y_als_px.is_finite()
            && b.center_norm.0.is_finite()
            && b.center_norm.1.is_finite()
            && b.r_core_px.is_finite()
            && b.r_halo_px.is_finite()
            && b.peak_swir_intensity.is_finite()
    })
}

/// Runs the complete [`VisionFogPipeline`] on a real video file and returns a [`RealVideoBenchmarkReport`].
pub fn run_real_video_benchmark(
    video_path: &Path,
    source_description: &str,
    max_frames: usize,
    config: VisionPipelineConfig,
) -> Result<RealVideoBenchmarkReport, RealVideoError> {
    let width = DEFAULT_IMG_WIDTH;
    let height = DEFAULT_IMG_HEIGHT;

    let raw_frames = decode_real_video_frames_ffmpeg(video_path, width, height, max_frames)?;
    let num_frames = raw_frames.len();

    let mut pipeline = VisionFogPipeline::new(width, height, config);
    let dt = pipeline.config.optical_flow.dt_frame_s.max(1.0 / 60.0);

    // Nominal ILS glideslope descent profile from 60m AGL down toward threshold during approach clip
    let mut kin = CameraKinematics6DoF {
        x_lrcv_m: -850.0,
        y_cross_m: 0.0,
        h_agl_m: 45.0,
        phi_rad: 0.0,
        theta_rad: -0.052, // -3 deg glideslope
        psi_rad: 0.0,
        u_body_mps: 68.0,
        v_body_mps: 0.0,
        w_body_mps: 3.4,
        p_radps: 0.0,
        q_radps: 0.0,
        r_radps: 0.0,
    };

    let mut frame_times_ms = Vec::with_capacity(num_frames);
    let mut sum_mor = 0.0_f64;
    let mut sum_beta = 0.0_f64;
    let mut sum_ground_patches = 0.0_f64;
    let mut sum_fog_patches = 0.0_f64;
    let mut sum_w_photon = 0.0_f64;
    let mut sum_v_ground = Vector3::<f64>::zeros();
    let mut sum_w_fog = Vector3::<f64>::zeros();
    let mut total_als_beacons = 0usize;
    let mut gust_triggers = 0usize;
    let mut all_finite = true;

    let prior_v_ground = Vector3::new(0.0, 3.4, 68.0);
    let gyro_bias = Vector3::zeros();
    let mut prev_swir: Option<Vec<f32>> = None;

    for (idx, rgb_bytes) in raw_frames.iter().enumerate() {
        let t_sec = idx as f64 * dt;
        kin.x_lrcv_m += kin.u_body_mps * dt;
        kin.h_agl_m = (45.0 - kin.w_body_mps * t_sec).max(2.5);

        let dual_frame =
            rgb24_to_dual_band_frame(rgb_bytes, width, height, idx as u64, t_sec, &kin, 120.0)?;

        // Pentru filmări MP4 fără telemetrie IMU atașată, estimăm rotația globală a camerei (shake q, r)
        // din banda superioară deasupra orizontului (unde rho_ground == 0) ca să nu confundăm zdruncinarea camerei cu vântul!
        if let Some(ref prev) = prev_swir {
            let sky_h = (height / 3).max(4);
            let mut sum_du = 0.0_f64;
            let mut sum_dv = 0.0_f64;
            let mut sum_w = 0.0_f64;
            for py in 2..sky_h.saturating_sub(2) {
                for px in 2..width.saturating_sub(2) {
                    let c = py * width + px;
                    let ix = 0.5 * f64::from(dual_frame.swir_band[c + 1] - dual_frame.swir_band[c - 1]);
                    let iy = 0.5 * f64::from(dual_frame.swir_band[c + width] - dual_frame.swir_band[c - width]);
                    let it = f64::from(dual_frame.swir_band[c] - prev[c]);
                    let g2 = ix * ix + iy * iy;
                    if g2 > 1e-3 {
                        sum_du += (-it * ix / (g2 + 1e-3)).clamp(-4.0, 4.0) * g2;
                        sum_dv += (-it * iy / (g2 + 1e-3)).clamp(-4.0, 4.0) * g2;
                        sum_w += g2;
                    }
                }
            }
            if sum_w > 1e-2 {
                let du_norm = (sum_du / sum_w) / (0.5 * width as f64);
                let dv_norm = (sum_dv / sum_w) / (0.5 * height as f64);
                kin.q_radps = (-dv_norm / dt).clamp(-0.65, 0.65);
                kin.r_radps = (-du_norm / dt).clamp(-0.65, 0.65);
            } else {
                kin.q_radps = 0.0;
                kin.r_radps = 0.0;
            }
        }
        prev_swir = Some(dual_frame.swir_band.clone());

        // Check input frame finiteness
        if !dual_frame.vis_band.iter().all(|v| v.is_finite())
            || !dual_frame.swir_band.iter().all(|v| v.is_finite())
            || !dual_frame.ground_inv_depth.iter().all(|v| v.is_finite())
        {
            all_finite = false;
        }

        let imu_samples = [
            ImuAngularSample {
                timestamp_s: t_sec - dt,
                omega_radps: Vector3::new(kin.p_radps, kin.q_radps, kin.r_radps),
            },
            ImuAngularSample {
                timestamp_s: t_sec,
                omega_radps: Vector3::new(kin.p_radps, kin.q_radps, kin.r_radps),
            },
        ];

        let t_start = Instant::now();
        let report = pipeline.process_frame(
            &dual_frame,
            &imu_samples,
            &gyro_bias,
            kin.psi_rad,
            None, // Estimate Koschmieder visibility directly from real video contrast profile!
            None,
            Some(&prior_v_ground),
        );
        let elapsed_ms = t_start.elapsed().as_secs_f64() * 1000.0;
        frame_times_ms.push(elapsed_ms);

        if !is_frame_report_strictly_finite(&report) {
            all_finite = false;
        }

        sum_mor += report.fog_scale.estimated_v_mor_m;
        sum_beta += report.fog_scale.beta_ext_inv_m;
        sum_ground_patches += report.motion_estimate.active_ground_patches as f64;
        sum_fog_patches += report.motion_estimate.active_fog_patches as f64;
        sum_w_photon += report.mean_photonic_weight;
        sum_v_ground += report.motion_estimate.v_ground_body_mps;
        let wf = report.motion_estimate.w_fog_body_mps;
        sum_w_fog += Vector3::new(
            wf.x.clamp(-24.0, 24.0),
            wf.y.clamp(-24.0, 24.0),
            wf.z.clamp(-12.0, 12.0),
        );
        total_als_beacons += report.als_suppression.beacons.len();
        if report.motion_estimate.gust_warning_active {
            gust_triggers += 1;
        }
    }

    let inv_n = 1.0 / (num_frames.max(1) as f64);
    let mean_frame_time_ms = frame_times_ms.iter().sum::<f64>() * inv_n;

    let mut sorted_times = frame_times_ms.clone();
    sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p99_idx = ((num_frames as f64 * 0.99).floor() as usize).min(num_frames.saturating_sub(1));
    let p99_frame_time_ms = sorted_times.get(p99_idx).copied().unwrap_or(mean_frame_time_ms);
    let real_fps_achieved = 1000.0 / mean_frame_time_ms.max(1e-6);

    let mean_v_g = sum_v_ground * inv_n;
    let mean_w_f = sum_w_fog * inv_n;

    let file_name = video_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| video_path.display().to_string());

    Ok(RealVideoBenchmarkReport {
        video_path: file_name,
        source_description: source_description.to_string(),
        frame_width: width,
        frame_height: height,
        frames_processed: num_frames,
        mean_frame_time_ms,
        p99_frame_time_ms,
        real_fps_achieved,
        estimated_mor_visibility_m: sum_mor * inv_n,
        mean_extinction_beta: sum_beta * inv_n,
        mean_valid_ground_patches: sum_ground_patches * inv_n,
        mean_valid_fog_patches: sum_fog_patches * inv_n,
        mean_photonic_weight: sum_w_photon * inv_n,
        mean_ground_velocity_body_mps: [mean_v_g.x, mean_v_g.y, mean_v_g.z],
        mean_fog_wind_velocity_body_mps: [mean_w_f.x, mean_w_f.y, mean_w_f.z],
        total_als_beacons_detected: total_als_beacons,
        mean_als_beacons_per_frame: total_als_beacons as f64 * inv_n,
        gust_lookahead_triggers: gust_triggers,
        zero_nan_or_inf: all_finite,
    })
}

/// Runs the real video benchmark across a list of `(PathBuf, description)` pairs and saves
/// the JSON report to `output_json_path`.
pub fn run_multi_video_benchmark_and_save(
    video_entries: &[(PathBuf, String)],
    max_frames_per_video: usize,
    output_json_path: &Path,
) -> Result<MultiVideoBenchmarkSummary, RealVideoError> {
    let mut reports = Vec::with_capacity(video_entries.len());
    let mut total_frames = 0usize;
    let mut weighted_time_sum_ms = 0.0_f64;
    let mut all_finite = true;

    for (path, desc) in video_entries {
        let rep = run_real_video_benchmark(
            path,
            desc,
            max_frames_per_video,
            VisionPipelineConfig::default(),
        )?;
        total_frames += rep.frames_processed;
        weighted_time_sum_ms += rep.mean_frame_time_ms * (rep.frames_processed as f64);
        if !rep.zero_nan_or_inf {
            all_finite = false;
        }
        reports.push(rep);
    }

    let overall_mean_frame_time_ms = if total_frames > 0 {
        weighted_time_sum_ms / (total_frames as f64)
    } else {
        0.0
    };
    let overall_fps_achieved = if overall_mean_frame_time_ms > 1e-6 {
        1000.0 / overall_mean_frame_time_ms
    } else {
        0.0
    };

    let summary = MultiVideoBenchmarkSummary {
        total_videos: reports.len(),
        total_frames_processed: total_frames,
        overall_mean_frame_time_ms,
        overall_fps_achieved,
        all_zero_nan_or_inf: all_finite,
        videos: reports,
    };

    let json_str = serde_json::to_string_pretty(&summary)?;
    if let Some(parent) = output_json_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(output_json_path, json_str)?;

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb24_to_dual_band_conversion_and_saturated_bounds() {
        let width = 32;
        let height = 32;
        let n_bytes = width * height * 3;
        let mut rgb = vec![0u8; n_bytes];

        // Fill half with saturated white [255, 255, 255], half with warm red/orange ALS glow [255, 140, 20]
        for i in 0..(width * height) {
            if i % 2 == 0 {
                rgb[i * 3] = 255;
                rgb[i * 3 + 1] = 255;
                rgb[i * 3 + 2] = 255;
            } else {
                rgb[i * 3] = 255;
                rgb[i * 3 + 1] = 140;
                rgb[i * 3 + 2] = 20;
            }
        }

        let kin = CameraKinematics6DoF {
            theta_rad: 0.12, // pitched up to test horizon cutoff
            phi_rad: -0.08,
            h_agl_m: 35.0,
            ..Default::default()
        };

        let frame_res = rgb24_to_dual_band_frame(&rgb, width, height, 0, 0.0, &kin, 50.0);
        assert!(frame_res.is_ok());
        if let Ok(frame) = frame_res {
            assert_eq!(frame.vis_band.len(), width * height);
            assert_eq!(frame.swir_band.len(), width * height);
            assert!(frame.vis_band.iter().all(|&v| v >= 0.0 && v <= 1.0 && v.is_finite()));
            assert!(frame.swir_band.iter().all(|&v| v >= 0.0 && v <= 1.0 && v.is_finite()));
            assert!(frame.ground_inv_depth.iter().all(|&r| r >= 0.0 && r.is_finite()));
            // Warm ALS pixel (index 1) must have SWIR > VIS
            assert!(frame.swir_band[1] > frame.vis_band[1]);
        }
    }

    #[test]
    fn test_real_aviation_videos_benchmark_and_json_export() {
        // Locate real_videos directory relative to crate root or workspace root
        let candidate_dirs = [
            PathBuf::from("../../real_videos"),
            PathBuf::from("real_videos"),
        ];

        let mut real_videos_dir = None;
        for d in &candidate_dirs {
            if d.exists() && d.is_dir() {
                real_videos_dir = Some(d.clone());
                break;
            }
        }

        let Some(dir) = real_videos_dir else {
            println!("Notice: real_videos directory not present in local test environment; skipping live file pass.");
            return;
        };

        if std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .is_err()
        {
            println!("Notice: ffmpeg binary not installed in PATH; skipping offline MP4 decode pass.");
            return;
        }

        let mut entries: Vec<(PathBuf, String)> = Vec::new();
        if let Ok(read_dir) = fs::read_dir(&dir) {
            for entry in read_dir.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("mp4") {
                    let fname = p
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "real_cockpit_video".to_string());
                    let desc = match fname.as_str() {
                        s if s.contains("cat3") || s.contains("fog") => {
                            "Real Cockpit ILS CAT III Dense Fog Approach & Autoland (ALS & Runway Threshold)".to_string()
                        }
                        s if s.contains("crosswind") || s.contains("gust") => {
                            "Real Cockpit Severe Crosswind & Low-Visibility Gust Approach".to_string()
                        }
                        _ => format!("Real Aviation Cockpit Low-Visibility Approach ({fname})"),
                    };
                    entries.push((p, desc));
                }
            }
        }

        entries.sort_by(|a, b| a.0.cmp(&b.0));
        if entries.is_empty() {
            println!("Notice: no .mp4 files found in {:?}.", dir);
            return;
        }

        let out_json = dir.join("benchmark_report.json");
        let summary_res = run_multi_video_benchmark_and_save(&entries, 300, &out_json);
        assert!(
            summary_res.is_ok(),
            "Real video benchmark failed: {:?}",
            summary_res.err()
        );

        if let Ok(summary) = summary_res {
            println!("\n================================================================================");
            println!("AERO-SHIELD LRCV V2.0 — REAL AVIATION COCKPIT VIDEO BENCHMARK REPORT");
            println!("================================================================================");
            println!("Total Real Videos Processed : {}", summary.total_videos);
            println!("Total Real Frames Processed : {}", summary.total_frames_processed);
            println!(
                "Overall Mean Frame Time     : {:.3} ms ({:.1} FPS)",
                summary.overall_mean_frame_time_ms, summary.overall_fps_achieved
            );
            println!("Strict Zero NaN/Inf Check   : {}", summary.all_zero_nan_or_inf);
            println!("--------------------------------------------------------------------------------");
            for v in &summary.videos {
                println!(
                    "Video: {} | Frames: {} | Mean: {:.3} ms | P99: {:.3} ms | FPS: {:.1} | MOR: {:.1} m | beta: {:.4} | GroundPatches: {:.1} | FogPatches: {:.1} | w_photon: {:.3} | ALS/frame: {:.2} | GustAlerts: {} | ZeroNaN: {}",
                    v.video_path,
                    v.frames_processed,
                    v.mean_frame_time_ms,
                    v.p99_frame_time_ms,
                    v.real_fps_achieved,
                    v.estimated_mor_visibility_m,
                    v.mean_extinction_beta,
                    v.mean_valid_ground_patches,
                    v.mean_valid_fog_patches,
                    v.mean_photonic_weight,
                    v.mean_als_beacons_per_frame,
                    v.gust_lookahead_triggers,
                    v.zero_nan_or_inf
                );
                assert!(v.frames_processed > 0);
                assert!(v.zero_nan_or_inf, "Strict Zero NaN/Inf violated on {}", v.video_path);
                assert!(
                    v.real_fps_achieved > 60.0,
                    "Real video FPS ({}) must exceed 60 Hz real-time requirement",
                    v.real_fps_achieved
                );
            }
            println!("================================================================================\n");
            assert!(summary.all_zero_nan_or_inf);
            assert!(out_json.exists());
        }
    }
}
