//! # `aero-ai-cuda` — Rețeaua Neurală `FogOpticalNavNetV2`, Inferență Lock-Free `ArcSwap` (60 Hz) & `ShadowTrainer`
//!
//! Acest crate implementează arhitectura AI pe GPU/CPU pentru **AERO-SHIELD LRCV V2.0 (`FogVision 6-DoF`)**:
//! 1. **Selecție Automată a Dispozitivului (`init_compute_backend`)**: Inițializează transparent
//!    `Device::cuda_if_available(0)` (pentru NVIDIA L4 / T4 pe Google Cloud VM) cu fallback automat
//!    fără eroare pe `Device::Cpu` (Multi-Core SIMD).
//! 2. **Arhitectura `FogOpticalNavNetV2` (`model_v2`)**: Construită 100% din primitive oficiale
//!    `candle_nn` (`Conv2d`, `GroupNorm`, `Linear`, `candle_nn::rnn::GRU`) cu:
//!    - *Dual-Branch Stem*: Ramura 1 (Ground & ALS Micro-Gradient) + Ramura 2 (Volumetric Fog Dilated Conv),
//!    - *FogSpatialAttentionPool2d*: Poartă de atenție spațială ponderată cu masca inelară de suprimare
//!      a haloului Mie $M_{\text{bloom}}$ și ponderea de certitudine fotonică $w_{\text{photon}}$,
//!    - *Physics-Guided 6-DoF Fusion & Native GRU*: Fuziune cu vectorul cinematic 6-DoF $\mathbf{K}_{6\text{dof}} \in \mathbb{R}^{18}$
//!      și memorie recurentă cauzală $H_t$ la $60\text{ Hz}$,
//!    - *Heteroscedastic Multi-Task Heads*: Estimarea simultană a vitezei față de sol $\hat{\mathbf{v}}_{\text{ground}}$,
//!      a vitezei ceții $\hat{\mathbf{w}}_{\text{fog}}$, a log-varianțelor $\mathbf{s}_v, \mathbf{s}_w \in [-10.0, 6.0]$,
//!      a scorului anticipativ de rafală $\hat{p}_{\text{gust}}$ și a corecției față de balizajul ALS RWY 09 Craiova.
//! 3. **Inferență Lock-Free la $60\text{ Hz}$ (`frozen_inference`)**: `FrozenFogNavModel` conține exclusiv
//!    tensori detașați (`.detach()`), gestionați prin `ArcSwap<FrozenFogNavModel>` pentru citire wait-free
//!    în $< 5\text{ ns}$ fără blocarea buclei Real-Time.
//! 4. **Funcție de Pierdere Fizic-Informată (`pinn_loss`)**: *Heteroscedastic Gaussian NLL + Inverse-Depth PINN Loss*
//!    cu protecție matematică împotriva `NaN` / `Inf`.
//! 5. **Generator Sintetic 6-DoF pe Dispozitiv (`synthetic_gpu_gen`)**: Generează loturi vectorizate cu
//!    inversa adâncimii $\rho_{\text{ground}} \ge 0$ (fără împărțire la zero la orizont), atenuare Koschmieder,
//!    halouri ALS și forfecare de vânt Von Kármán.
//! 6. **Double-Buffered Shadow Trainer (`shadow_trainer`)**: Antrenează pe un `VarMap` izolat pe un thread
//!    OS separat (`aero-cuda-shadow-trainer`) cu `AdamW` și execută swap atomic `ArcSwap::store` la finalul epocii.

#![forbid(unsafe_code)]

pub mod frozen_inference;
pub mod model_v2;
pub mod pinn_loss;
pub mod shadow_trainer;
pub mod synthetic_gpu_gen;

use candle_core::{DType, Device};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use frozen_inference::{
    FrozenFogNavModel, FrozenFogNavNetV2, InferenceTimingStats, SharedFrozenInferenceEngine,
};
pub use model_v2::{
    DualBandStem, FogNavForwardTensors, FogNavNetConfig, FogOpticalNavNetV2,
    FogSpatialAttentionPool2d, HeteroscedasticMultiTaskHeads, NavPredictionOutput,
    DEFAULT_GRU_HIDDEN_DIM, DEFAULT_IMAGE_SIZE, DEFAULT_VISUAL_CHANNELS, EXTENDED_VISUAL_CHANNELS,
    KINEMATIC_VECTOR_DIM, LOG_VAR_MAX, LOG_VAR_MIN,
};
pub use pinn_loss::{
    HeteroscedasticPinnLoss, LossBreakdown, PinnBatchTargets, PinnLossConfig,
};
pub use shadow_trainer::{
    ShadowTrainer, ShadowTrainerConfig, ShadowTrainingHandle, TrainingEpochTelemetry,
    TrainingProgressTelemetry,
};
pub use synthetic_gpu_gen::{
    CudaSyntheticFlightBatcher, DomainRandomizationConfig, Synthetic6DoFFogGenerator,
    SyntheticBatch,
};

/// Eroare unificată și tipizată pentru toate operațiile din `aero-ai-cuda`.
/// Elimină complet necesitatea apelurilor `.unwrap()` pe calea critică de $60\text{ Hz}$.
#[derive(Debug, Error)]
pub enum AeroAiError {
    #[error("Candle tensor/device error: {0}")]
    Candle(#[from] candle_core::Error),

    #[error("Invalid input tensor shape for `{tensor_name}`: expected {expected:?}, got {actual:?}")]
    InvalidTensorShape {
        tensor_name: &'static str,
        expected: Vec<usize>,
        actual: Vec<usize>,
    },

    #[error("Non-finite value (NaN or Inf) detected in `{context}`")]
    NonFiniteValue { context: &'static str },

    #[error("Safetensors I/O error at path `{path}`: {details}")]
    SafetensorsIo { path: String, details: String },

    #[error("Shadow trainer synchronization or thread error: {0}")]
    ShadowTrainerError(String),
}

/// Informații despre backend-ul de calcul selectat la inițializare (CUDA GPU vs. CPU Fallback).
#[derive(Debug, Clone)]
pub struct ComputeBackendInfo {
    /// Dispozitivul `candle_core::Device` activ (`Device::Cuda(0)` sau `Device::Cpu`).
    pub device: Device,
    /// Tipul de date pentru tensorii rețelei (`DType::F32`).
    pub dtype: DType,
    /// Eticheta descriptivă afișată în telemetria Cockpit HUD.
    pub backend_label: String,
    /// `true` dacă execuția este accelerată hardware pe GPU NVIDIA CUDA.
    pub is_cuda_accelerated: bool,
}

///Descriere serializabilă a backend-ului de calcul pentru telemetria WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComputeBackendSummary {
    pub backend_label: String,
    pub is_cuda_accelerated: bool,
    pub dtype_name: String,
}

impl ComputeBackendInfo {
    /// Returnează un sumar serializabil JSON pentru HUD.
    #[inline]
    pub fn to_summary(&self) -> ComputeBackendSummary {
        ComputeBackendSummary {
            backend_label: self.backend_label.clone(),
            is_cuda_accelerated: self.is_cuda_accelerated,
            dtype_name: format!("{:?}", self.dtype),
        }
    }
}

/// Inițializează automat dispozitivul de calcul:
/// - Încearcă `Device::cuda_if_available(0)` pentru rularea pe NVIDIA L4 / T4 GPU.
/// - Dacă placa CUDA nu este disponibilă (sau feature-ul `cuda` nu este activ),
///   revine transparent pe `Device::Cpu` fără nicio eroare la pornire.
pub fn init_compute_backend() -> ComputeBackendInfo {
    match Device::cuda_if_available(0) {
        Ok(dev @ Device::Cuda(_)) => {
            tracing::info!("AERO-AI-CUDA: Activated NVIDIA CUDA GPU backend on ordinal 0");
            ComputeBackendInfo {
                device: dev,
                dtype: DType::F32,
                backend_label: "NVIDIA CUDA GPU (Candle Native Kernel)".to_string(),
                is_cuda_accelerated: true,
            }
        }
        Ok(dev) => {
            tracing::info!("AERO-AI-CUDA: Using CPU Multi-Core SIMD backend (Auto-Fallback)");
            ComputeBackendInfo {
                device: dev,
                dtype: DType::F32,
                backend_label: "CPU Multi-Core SIMD (Auto-Fallback)".to_string(),
                is_cuda_accelerated: false,
            }
        }
        Err(err) => {
            tracing::warn!(
                "AERO-AI-CUDA: CUDA probe returned error ({}), falling back to CPU SIMD",
                err
            );
            ComputeBackendInfo {
                device: Device::Cpu,
                dtype: DType::F32,
                backend_label: "CPU Multi-Core SIMD (Auto-Fallback)".to_string(),
                is_cuda_accelerated: false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_nn::rnn::GRUState;
    use candle_nn::{VarBuilder, VarMap};

    #[test]
    fn test_1_forward_pass_dimensions_and_gru_state_persistence() -> Result<(), AeroAiError> {
        let backend = init_compute_backend();
        let cfg_10 = FogNavNetConfig::default_10ch();
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, backend.dtype, &backend.device);
        let net = FogOpticalNavNetV2::new(cfg_10, vb)?;

        let batch_size = 4;
        let visual = candle_core::Tensor::zeros(
            (batch_size, cfg_10.visual_channels, cfg_10.image_size, cfg_10.image_size),
            backend.dtype,
            &backend.device,
        )?;
        let kinematic = candle_core::Tensor::zeros(
            (batch_size, cfg_10.kinematic_dim),
            backend.dtype,
            &backend.device,
        )?;

        // Pasul 1 fără stare GRU anterioară
        let out1 = net.forward_step(&visual, &kinematic, None)?;
        assert_eq!(out1.v_ground_ned.dims(), &[batch_size, 3]);
        assert_eq!(out1.s_v_log_var.dims(), &[batch_size, 3]);
        assert_eq!(out1.w_fog_ned.dims(), &[batch_size, 3]);
        assert_eq!(out1.s_w_log_var.dims(), &[batch_size, 3]);
        assert_eq!(out1.gust_logit.dims(), &[batch_size, 1]);
        assert_eq!(out1.gust_prob.dims(), &[batch_size, 1]);
        assert_eq!(out1.als_offset.dims(), &[batch_size, 2]);
        assert_eq!(out1.f_gnd_features.dims(), &[batch_size, 64]);
        assert_eq!(out1.f_fog_features.dims(), &[batch_size, 64]);
        assert_eq!(out1.next_gru_state.h().dims(), &[batch_size, cfg_10.gru_hidden_dim]);

        // Pasul 2 cu starea GRU persistentă de la Pasul 1
        let visual_step2 = candle_core::Tensor::ones(
            (batch_size, cfg_10.visual_channels, cfg_10.image_size, cfg_10.image_size),
            backend.dtype,
            &backend.device,
        )?;
        let out2 = net.forward_step(&visual_step2, &kinematic, Some(&out1.next_gru_state))?;
        assert_eq!(out2.next_gru_state.h().dims(), &[batch_size, cfg_10.gru_hidden_dim]);

        // Verificăm și configurația extinsă cu 12 canale
        let cfg_12 = FogNavNetConfig::extended_12ch(64);
        let varmap_12 = VarMap::new();
        let vb_12 = VarBuilder::from_varmap(&varmap_12, backend.dtype, &backend.device);
        let net_12 = FogOpticalNavNetV2::new(cfg_12, vb_12)?;
        let visual_12 = candle_core::Tensor::zeros(
            (2, 12, 64, 64),
            backend.dtype,
            &backend.device,
        )?;
        let kin_12 = candle_core::Tensor::zeros((2, 18), backend.dtype, &backend.device)?;
        let out_12 = net_12.forward_step(&visual_12, &kin_12, None)?;
        assert_eq!(out_12.v_ground_ned.dims(), &[2, 3]);

        Ok(())
    }

    #[test]
    fn test_2_log_variance_clamping_and_positive_covariance() -> Result<(), AeroAiError> {
        let backend = init_compute_backend();
        let cfg = FogNavNetConfig::default_10ch();
        let frozen = FrozenFogNavModel::new_random_initialized(cfg, &backend.device)?;

        // Intrări cu amplitudine mare pentru a forța activări mari în capetele rețelei
        let visual_extreme = candle_core::Tensor::ones(
            (1, cfg.visual_channels, cfg.image_size, cfg.image_size),
            backend.dtype,
            &backend.device,
        )?
        .affine(500.0, -250.0)?;
        let kinematic_extreme = candle_core::Tensor::ones(
            (1, cfg.kinematic_dim),
            backend.dtype,
            &backend.device,
        )?
        .affine(200.0, -100.0)?;

        let raw_out = frozen
            .network()
            .forward_step(&visual_extreme, &kinematic_extreme, None)?;
        let sv_vals = raw_out.s_v_log_var.squeeze(0)?.to_vec1::<f32>()?;
        let sw_vals = raw_out.s_w_log_var.squeeze(0)?.to_vec1::<f32>()?;

        for &sv in &sv_vals {
            assert!(sv.is_finite());
            assert!(sv >= LOG_VAR_MIN && sv <= LOG_VAR_MAX);
        }
        for &sw in &sw_vals {
            assert!(sw.is_finite());
            assert!(sw >= LOG_VAR_MIN && sw <= LOG_VAR_MAX);
        }

        let decoded = raw_out.decode_first_sample()?;
        let r_diag = decoded.r_ai_diagonal_covariance();
        let min_cov = (LOG_VAR_MIN as f64).exp();
        let max_cov = (LOG_VAR_MAX as f64).exp();

        for &sigma2 in &r_diag {
            assert!(sigma2.is_finite());
            assert!(sigma2 > 0.0);
            assert!(sigma2 >= min_cov - 1e-9 && sigma2 <= max_cov + 1e-6);
        }

        Ok(())
    }

    #[test]
    fn test_3_heteroscedastic_pinn_loss_zero_nan_on_extreme_values() -> Result<(), AeroAiError> {
        let backend = init_compute_backend();
        let cfg = FogNavNetConfig::default_10ch();
        let gen = Synthetic6DoFFogGenerator::with_defaults(backend.device.clone());
        let batch = gen.generate_repeatable_calibration_batch(4, 42)?;

        // Verificăm că inversa adâncimii la cabraj extrem peste orizont nu dă niciodată NaN sau valori negative
        let rho_above_horizon = Synthetic6DoFFogGenerator::compute_inverse_depth_ground(
            0.0,
            -0.95,
            0.5,
            0.65,
            0.0, // chiar și la h_AGL = 0.0 m este clampat la 0.5 m
        );
        assert!(rho_above_horizon.is_finite());
        assert!(rho_above_horizon >= 0.0);

        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, backend.dtype, &backend.device);
        let net = FogOpticalNavNetV2::new(cfg, vb)?;
        let preds = net.forward_step(&batch.visual_tensor, &batch.kinematic_tensor, None)?;

        let loss_fn = HeteroscedasticPinnLoss::default();
        let (loss_tensor, breakdown) = loss_fn.compute_total_loss(&preds, &batch.targets)?;
        let loss_scalar = loss_tensor.to_scalar::<f32>()?;

        assert!(loss_scalar.is_finite());
        assert!(breakdown.total_loss.is_finite());
        assert!(breakdown.vel_nll.is_finite());
        assert!(breakdown.fog_nll.is_finite());
        assert!(breakdown.pinn_residual.is_finite() && breakdown.pinn_residual >= 0.0);
        assert!(breakdown.gust_bce.is_finite() && breakdown.gust_bce >= 0.0);
        assert!(breakdown.als_mse.is_finite() && breakdown.als_mse >= 0.0);

        Ok(())
    }

    #[test]
    fn test_4_shadow_trainer_adamw_loss_decreases_on_mini_batch() -> Result<(), AeroAiError> {
        let backend = init_compute_backend();
        let mut t_cfg = ShadowTrainerConfig::default();
        t_cfg.learning_rate = 5e-3;
        t_cfg.inter_step_sleep_ms = 0;
        t_cfg.domain_config.batch_size = 4;

        let mut trainer = ShadowTrainer::new(t_cfg.clone(), backend.device.clone())?;
        let gen = Synthetic6DoFFogGenerator::new(
            t_cfg.net_config,
            t_cfg.domain_config,
            backend.device.clone(),
            2026,
        );
        let fixed_batch = gen.generate_repeatable_calibration_batch(4, 2026)?;

        let initial_eval = trainer.evaluate_batch(&fixed_batch)?;
        let loss_before = initial_eval.total_loss;

        let mut last_loss = loss_before;
        for _ in 0..8 {
            let step_metrics = trainer.train_mini_batch(&fixed_batch)?;
            last_loss = step_metrics.total_loss;
        }

        let final_eval = trainer.evaluate_batch(&fixed_batch)?;
        let loss_after = final_eval.total_loss.min(last_loss);

        assert!(
            loss_after < loss_before,
            "Expected AdamW training to reduce total PINN loss: before={loss_before}, after={loss_after}"
        );

        Ok(())
    }

    #[test]
    fn test_5_lock_free_arc_swap_and_safetensors_roundtrip() -> Result<(), AeroAiError> {
        let backend = init_compute_backend();
        let cfg = FogNavNetConfig::default_10ch();
        let shared_engine = SharedFrozenInferenceEngine::new(backend.clone(), cfg)?;

        assert_eq!(shared_engine.load_active_model().version_epoch(), 0);

        // Rulăm un pas de inferență la 60 Hz înainte de antrenare
        let visual = candle_core::Tensor::zeros(
            (1, cfg.visual_channels, cfg.image_size, cfg.image_size),
            backend.dtype,
            &backend.device,
        )?;
        let kinematic = candle_core::Tensor::zeros(
            (1, cfg.kinematic_dim),
            backend.dtype,
            &backend.device,
        )?;
        let mut gru_state: Option<GRUState> = None;

        let (pred_before, stats_before) =
            shared_engine.infer_60hz_step(&visual, &kinematic, &mut gru_state)?;
        assert_eq!(stats_before.model_version_epoch, 0);
        assert!(pred_before.v_ground_ned[0].is_finite());
        assert!(gru_state.is_some());

        // Lansăm ShadowTrainer pe firul dedicat și verificăm swap-ul atomic
        let mut trainer_cfg = ShadowTrainerConfig::default();
        trainer_cfg.batches_per_epoch = 2;
        trainer_cfg.domain_config.batch_size = 2;
        trainer_cfg.inter_step_sleep_ms = 2;

        let tmp_safetensors = std::env::temp_dir()
            .join(format!("aero_shield_test_{}.safetensors", std::process::id()));
        trainer_cfg.checkpoint_path = Some(tmp_safetensors.clone());

        let handle = ShadowTrainer::spawn_live_cuda_finetuning(
            shared_engine.clone(),
            trainer_cfg,
            2,
        )?;

        // În timp ce ShadowTrainer rulează în paralel, firul de 60 Hz continuă inferența fără blocare
        let (pred_concurrent, _) =
            shared_engine.infer_60hz_step(&visual, &kinematic, &mut gru_state)?;
        assert!(pred_concurrent.v_ground_ned[0].is_finite());

        let epochs_telemetry = handle
            .join_handle
            .join()
            .map_err(|_| AeroAiError::ShadowTrainerError("Thread panicked".to_string()))??;
        assert_eq!(epochs_telemetry.len(), 2);
        assert!(epochs_telemetry[1].is_completed);

        // Verificăm că modelul activ a fost actualizat atomic la versiunea epocii 2
        let active_after = shared_engine.load_active_model();
        assert_eq!(active_after.version_epoch(), 2);

        // Verificăm încărcarea din .safetensors salvat atomic
        let reloaded = FrozenFogNavModel::load_safetensors(
            &tmp_safetensors,
            cfg,
            &backend.device,
            99,
        )?;
        assert_eq!(reloaded.version_epoch(), 99);
        let mut reload_gru = None;
        let pred_reloaded = reloaded.infer_step(&visual, &kinematic, &mut reload_gru)?;
        let mut active_gru = None;
        let pred_active = active_after.infer_step(&visual, &kinematic, &mut active_gru)?;

        for k in 0..3 {
            assert!((pred_reloaded.v_ground_ned[k] - pred_active.v_ground_ned[k]).abs() < 1e-5);
        }

        let _ = std::fs::remove_file(&tmp_safetensors);
        Ok(())
    }
}
