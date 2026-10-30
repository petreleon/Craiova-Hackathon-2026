//! # `aero-vision-fog` — 100% Pure Rust Vision Pipeline for AERO-SHIELD LRCV V2.0
//!
//! High-assurance, zero-allocation real-time optical navigation pipeline in dense CAT IIIc
//! radiation fog for Craiova Airport (LRCV) Runway 09:
//! - [`synthetic_lrcv_camera`]: Deterministic Dual-Band (Visible + SWIR/LWIR) synthetic camera
//!   for the 3-Phase LRCV RWY 09 Approach Funnel ($10\text{ km} \to 1.5\text{ km} \to 300\text{ m} \to 0\text{ m}$)
//!   with exact NaN-free ground inverse depth $\rho_{ground}(x_c, y_c)$ (Remediation #11).
//! - [`swir_vis_fusion`]: Adaptive contrast-weighted Dual-Band fusion ($\beta_{SWIR} \approx 0.22\,\beta_{VIS}$).
//! - [`clahe_koschmieder`]: SNR-Gated CLAHE (Remediation #1) + Koschmieder Optical Extinction &
//!   Active Inertial Scale Probing for $\hat{\rho}_{fog}$ (Remediation #3).
//! - [`structure_tensor_3f`]: 3-Frame Spatio-Temporal Structure Tensor $(\lambda_1, \lambda_2)$,
//!   Photonic Certainty Weight $w_{photon, i} \in [0, 1]$, and Anisotropic Fisher Information
//!   Matrix $\mathbf{R}_{opt, i}^{-1}$ solving the *Aperture Problem* (Remediation #4).
//! - [`als_halo_suppressor`]: Multiple Scattering Mie Bloom halo ring masking ($M_{bloom}=0$)
//!   and sub-pixel $I_{SWIR}^2$-weighted ALS centroid extraction (Remediation #2).
//! - [`optical_flow_derot`]: Forward-Backward optical flow, IMU exposure pre-integration with
//!   $\hat{\Delta t}_{sync}$ compensation (Remediation #6), exact 3D derotation $\mathbf{B}_{rot}$,
//!   decoupled Ground/Fog velocity WLS, and Optical Gust Lookahead ($35\text{–}80\text{ m}$) with
//!   Autopilot Crab Angle $\psi_{crab}$ (Remediation #5).

pub mod als_halo_suppressor;
pub mod clahe_koschmieder;
pub mod optical_flow_derot;
pub mod structure_tensor_3f;
pub mod swir_vis_fusion;
pub mod synthetic_lrcv_camera;

pub use als_halo_suppressor::{
    suppress_als_halos_into, AlsDetectedBeacon, AlsHaloSuppressorConfig, AlsSuppressionResult,
};
pub use clahe_koschmieder::{
    apply_snr_gated_clahe_into, compute_active_inertial_scale_probe, compute_snr_clip_limit,
    estimate_fused_fog_scale, estimate_koschmieder_extinction, ClaheKoschmiederConfig,
    FogScaleEstimate,
};
pub use optical_flow_derot::{
    compute_autopilot_crab_angle, compute_forward_backward_flow_patches,
    compute_gust_lookahead_lead_time, compute_rotation_jacobian_b_rot,
    compute_translation_jacobian_a_tr, derotate_optical_flow, preintegrate_imu_angular_rate,
    solve_decoupled_ground_and_fog_velocity, DecoupledMotionEstimate, ImuAngularSample,
    OpticalFlowDerotConfig, PatchOpticalFlow,
};
pub use structure_tensor_3f::{
    compute_anisotropic_fisher_matrix, compute_photonic_weight, compute_structure_tensors_3f_into,
    decompose_structure_tensor_2x2, PatchStructureTensor, RingBuffer3Frames,
    StructureTensorConfig,
};
pub use swir_vis_fusion::{
    compute_local_variance_into, fuse_dual_band, fuse_dual_band_into, FusionMetrics,
    SwirVisFusionConfig,
};
pub use synthetic_lrcv_camera::{
    compute_ground_inverse_depth, compute_pixel_ground_inverse_depth, ApproachPhase,
    CameraKinematics6DoF, DualBandFrame, FogEnvironmentParams, SyntheticLrcvCamera,
    DEFAULT_IMG_HEIGHT, DEFAULT_IMG_WIDTH,
};

use nalgebra::{Vector2, Vector3};
use serde::{Deserialize, Serialize};

/// Unified configuration for the complete `VisionFogPipeline`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VisionPipelineConfig {
    pub fusion: SwirVisFusionConfig,
    pub clahe_koschmieder: ClaheKoschmiederConfig,
    pub structure_tensor: StructureTensorConfig,
    pub als_suppressor: AlsHaloSuppressorConfig,
    pub optical_flow: OpticalFlowDerotConfig,
}

/// Pre-allocated workspace holding all intermediate image buffers, 3-frame ring buffer,
/// and patch vectors so that the 60 Hz flight control loop performs **zero heap allocations**.
#[derive(Debug, Clone)]
pub struct VisionWorkspace {
    pub width: usize,
    pub height: usize,
    pub var_vis_buf: Vec<f32>,
    pub var_swir_buf: Vec<f32>,
    pub fused_image_buf: Vec<f32>,
    pub fused_var_buf: Vec<f32>,
    pub clahe_image_buf: Vec<f32>,
    pub clip_limits_buf: Vec<f32>,
    pub bloom_mask_buf: Vec<f32>,
    pub ring_buffer_3f: RingBuffer3Frames,
    pub patch_flows: Vec<PatchOpticalFlow>,
    pub cycle_errors: Vec<f64>,
    pub patch_tensors: Vec<PatchStructureTensor>,
}

impl VisionWorkspace {
    /// Pre-allocates all buffers for `width x height` images and `patch_size_px` patches.
    pub fn new(width: usize, height: usize, patch_size_px: usize) -> Self {
        let w = width.max(16);
        let h = height.max(16);
        let n = w * h;
        let psz = patch_size_px.clamp(2, 32);
        let n_patches = (w / psz) * (h / psz);

        Self {
            width: w,
            height: h,
            var_vis_buf: vec![0.0; n],
            var_swir_buf: vec![0.0; n],
            fused_image_buf: vec![0.0; n],
            fused_var_buf: vec![0.0; n],
            clahe_image_buf: vec![0.0; n],
            clip_limits_buf: vec![1.0; n],
            bloom_mask_buf: vec![1.0; n],
            ring_buffer_3f: RingBuffer3Frames::new(w, h),
            patch_flows: Vec::with_capacity(n_patches),
            cycle_errors: vec![0.0; n_patches],
            patch_tensors: Vec::with_capacity(n_patches),
        }
    }
}

impl Default for VisionWorkspace {
    fn default() -> Self {
        Self::new(DEFAULT_IMG_WIDTH, DEFAULT_IMG_HEIGHT, 8)
    }
}

/// Complete telemetry & navigation measurement report produced for each dual-band frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionFrameReport {
    pub frame_index: u64,
    pub timestamp_s: f64,
    pub phase: ApproachPhase,
    pub fusion_metrics: FusionMetrics,
    pub fog_scale: FogScaleEstimate,
    pub als_suppression: AlsSuppressionResult,
    pub motion_estimate: DecoupledMotionEstimate,
    /// Mean photonic certainty weight $\bar{w}_{photon} \in [0, 1]$ across all patches.
    pub mean_photonic_weight: f64,
    /// True if the entire frame is in `100% Flat Whiteout` ($\bar{w}_{photon} < 10^{-4}$).
    pub is_flat_whiteout: bool,
}

/// Complete high-speed 100% Rust Vision Pipeline for AERO-SHIELD LRCV V2.0.
#[derive(Debug, Clone)]
pub struct VisionFogPipeline {
    pub config: VisionPipelineConfig,
    pub workspace: VisionWorkspace,
}

impl Default for VisionFogPipeline {
    fn default() -> Self {
        Self::new(
            DEFAULT_IMG_WIDTH,
            DEFAULT_IMG_HEIGHT,
            VisionPipelineConfig::default(),
        )
    }
}

impl VisionFogPipeline {
    /// Creates a new `VisionFogPipeline` with pre-allocated `VisionWorkspace`.
    pub fn new(width: usize, height: usize, config: VisionPipelineConfig) -> Self {
        let workspace = VisionWorkspace::new(width, height, config.structure_tensor.patch_size_px);
        Self { config, workspace }
    }

    /// Processes a `DualBandFrame` along with IMU angular samples, gyro bias, and optional inertial
    /// probing inputs in a single zero-allocation pass.
    #[allow(clippy::too_many_arguments)]
    pub fn process_frame(
        &mut self,
        frame: &DualBandFrame,
        imu_samples: &[ImuAngularSample],
        gyro_bias_radps: &Vector3<f64>,
        psi_lrcv_rad: f64,
        prior_v_mor_m: Option<f64>,
        inertial_probe_inputs: Option<(&Vector2<f64>, &Vector3<f64>, f64, f64)>,
        prior_v_ground_body: Option<&Vector3<f64>>,
    ) -> VisionFrameReport {
        let w = self.workspace.width;
        let h = self.workspace.height;

        // Step 1: Dual-Band Visible + SWIR/LWIR Adaptive Contrast Fusion
        let fusion_metrics = fuse_dual_band_into(
            &frame.vis_band,
            &frame.swir_band,
            w,
            h,
            &self.config.fusion,
            &mut self.workspace.var_vis_buf,
            &mut self.workspace.var_swir_buf,
            &mut self.workspace.fused_image_buf,
            &mut self.workspace.fused_var_buf,
        );

        // Step 2: Koschmieder Optical Extinction + Active Inertial Scale Probing
        let fog_scale = estimate_fused_fog_scale(
            &self.workspace.fused_var_buf,
            &frame.ground_inv_depth,
            prior_v_mor_m,
            inertial_probe_inputs,
            &self.config.clahe_koschmieder,
        );

        // Step 3: SNR-Gated CLAHE (blocks thermal noise amplification in Flat Whiteout)
        apply_snr_gated_clahe_into(
            &self.workspace.fused_image_buf,
            &self.workspace.fused_var_buf,
            w,
            h,
            &self.config.clahe_koschmieder,
            &mut self.workspace.clahe_image_buf,
            &mut self.workspace.clip_limits_buf,
        );

        // Step 4: ALS Multiple Scattering Mie Bloom Halo Suppression & Sub-Pixel Centroid Extraction
        let als_suppression = suppress_als_halos_into(
            &frame.swir_band,
            w,
            h,
            fog_scale.beta_ext_inv_m,
            &self.config.als_suppressor,
            &mut self.workspace.bloom_mask_buf,
        );

        // Step 5: Push SNR-Gated CLAHE image into 3-Frame Ring Buffer
        self.workspace
            .ring_buffer_3f
            .push_frame(&self.workspace.clahe_image_buf);

        // Step 6: IMU Pre-Integration over exposure window + \Delta t_{sync} compensation & 3D Derotated Optical Flow
        let omega_bar = preintegrate_imu_angular_rate(
            imu_samples,
            gyro_bias_radps,
            self.config.optical_flow.dt_sync_s,
        );

        compute_forward_backward_flow_patches(
            &self.workspace.ring_buffer_3f,
            &omega_bar,
            &frame.ground_inv_depth,
            &self.workspace.bloom_mask_buf,
            &self.config.optical_flow,
            &mut self.workspace.patch_flows,
            &mut self.workspace.cycle_errors,
        );

        // Step 7: 3-Frame Spatio-Temporal Structure Tensor, Photonic Weight & Anisotropic Fisher Info
        let cycle_slice = if self.workspace.cycle_errors.is_empty() {
            None
        } else {
            Some(self.workspace.cycle_errors.as_slice())
        };
        compute_structure_tensors_3f_into(
            &self.workspace.ring_buffer_3f,
            cycle_slice,
            &self.config.structure_tensor,
            &mut self.workspace.patch_tensors,
        );

        let n_patches = self.workspace.patch_tensors.len().max(1);
        let mean_photonic_weight = self
            .workspace
            .patch_tensors
            .iter()
            .map(|p| p.w_photon)
            .sum::<f64>()
            / n_patches as f64;
        let is_flat_whiteout = mean_photonic_weight < 1e-4;

        // Step 8: Decoupled Ground/Fog WLS Velocity Estimation & Optical Gust Lookahead
        let motion_estimate = solve_decoupled_ground_and_fog_velocity(
            &self.workspace.patch_flows,
            &self.workspace.patch_tensors,
            fog_scale.rho_fog_fused,
            &omega_bar,
            psi_lrcv_rad,
            prior_v_ground_body,
            &self.config.optical_flow,
        );

        VisionFrameReport {
            frame_index: frame.frame_index,
            timestamp_s: frame.timestamp_s,
            phase: frame.phase,
            fusion_metrics,
            fog_scale,
            als_suppression,
            motion_estimate,
            mean_photonic_weight,
            is_flat_whiteout,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_pipeline_3_phase_lrcv_approach_and_flat_whiteout_safety() {
        let cam = SyntheticLrcvCamera::new(64, 64, 2026);
        let cfg = VisionPipelineConfig::default();
        let mut pipeline = VisionFogPipeline::new(64, 64, cfg);

        // 1. Test Phase 2 (800m from RWY 09 threshold in CAT IIIc fog with oncoming gust front)
        let kin = CameraKinematics6DoF {
            x_lrcv_m: -780.0,
            y_cross_m: 1.5,
            h_agl_m: 40.0,
            ..Default::default()
        };
        let fog = FogEnvironmentParams {
            v_mor_m: 35.0,
            w_fog_mps: [-3.0, 5.0, 0.0],
            gust_front_distance_m: Some(55.0),
            gust_delta_w_mps: [-4.0, 12.0, 0.0],
            sigma_sensor_noise: 0.005,
            flat_whiteout_override: false,
        };

        let imu_samples = vec![
            ImuAngularSample {
                timestamp_s: 0.0,
                omega_radps: Vector3::new(0.0, 0.0, 0.0),
            },
            ImuAngularSample {
                timestamp_s: 0.0166,
                omega_radps: Vector3::new(0.0, 0.0, 0.0),
            },
        ];

        let mut last_report = None;
        for f_idx in 0..3 {
            let t = f_idx as f64 * (1.0 / 60.0);
            let mut k_step = kin;
            k_step.x_lrcv_m += k_step.u_body_mps * t;
            let frame = cam.render_frame(f_idx, t, &k_step, &fog);
            let rep = pipeline.process_frame(
                &frame,
                &imu_samples,
                &Vector3::zeros(),
                0.0,
                Some(fog.v_mor_m),
                None,
                Some(&Vector3::new(0.0, 3.5, 68.0)),
            );
            last_report = Some(rep);
        }

        let rep = last_report.as_ref().unwrap_or_else(|| unreachable!());
        assert_eq!(rep.phase, ApproachPhase::Phase2AlsSwirCapture);
        assert!(!rep.is_flat_whiteout);
        assert!(rep.mean_photonic_weight > 0.05);
        assert!(!rep.als_suppression.beacons.is_empty());

        // 2. Test 100% Flat Whiteout safety: zero noise hallucination (w_photon == 0)
        pipeline.workspace.ring_buffer_3f.clear();
        let whiteout_fog = FogEnvironmentParams {
            flat_whiteout_override: true,
            sigma_sensor_noise: 0.008,
            ..Default::default()
        };
        let mut whiteout_rep = None;
        for f_idx in 0..3 {
            let t = f_idx as f64 * (1.0 / 60.0);
            let frame = cam.render_frame(f_idx, t, &kin, &whiteout_fog);
            whiteout_rep = Some(pipeline.process_frame(
                &frame,
                &imu_samples,
                &Vector3::zeros(),
                0.0,
                Some(20.0),
                None,
                None,
            ));
        }
        let w_rep = whiteout_rep.as_ref().unwrap_or_else(|| unreachable!());
        assert!(
            w_rep.is_flat_whiteout,
            "100% Flat Whiteout must be detected with mean_photonic_weight ~ 0, got {}",
            w_rep.mean_photonic_weight
        );
        assert_eq!(w_rep.motion_estimate.v_ground_fisher_info.norm(), 0.0);
    }
}
