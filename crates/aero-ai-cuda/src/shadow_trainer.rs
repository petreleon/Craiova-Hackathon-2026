//! # Arhitectura *Double-Buffered Shadow Trainer* (`ShadowTrainer` + `ArcSwap`)
//!
//! Implementează **Remedierea Critică #1** din `arhitectura/02_aero_ai_cuda.md`:
//! - Firul Real-Time de $60\text{ Hz}$ (`aero-60hz-rt-loop`) citește *wait-free* (`< 5 ns`)
//!   instanța activă [`FrozenFogNavModel`] din `Arc<ArcSwap<FrozenFogNavModel>>`.
//! - Când utilizatorul sau juriul apasă **`[⚡ LIVE CUDA FINE-TUNING (15s)]`**, se lansează
//!   firul dedicat `aero-cuda-shadow-trainer` care creează un `candle_nn::VarMap` **100% izolat**,
//!   rulează optimizatorul `candle_nn::AdamW` pe loturi sintetice 6-DoF și inserează un
//!   `std::thread::sleep(Duration::from_millis(2))` între pașii de gradient pentru a lăsa
//!   GPU-ul liber pentru cadrul de $60\text{ Hz}$.
//! - La finalul fiecărei epoci, `ShadowTrainer` extrage tensorii detașați (`.detach()`),
//!   construiește un nou [`FrozenFogNavModel`], salvează atomic `.safetensors` și execută:
//!   ```rust,ignore
//!   shared_engine.active_model.store(Arc::new(new_frozen_model));
//!   ```

use crate::frozen_inference::{FrozenFogNavModel, SharedFrozenInferenceEngine};
use crate::model_v2::{FogNavNetConfig, FogOpticalNavNetV2};
use crate::pinn_loss::{HeteroscedasticPinnLoss, LossBreakdown, PinnLossConfig};
use crate::synthetic_gpu_gen::{
    DomainRandomizationConfig, Synthetic6DoFFogGenerator, SyntheticBatch, DEFAULT_LRCV_RNG_SEED,
};
use crate::AeroAiError;
use candle_core::{DType, Device};
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};
use crossbeam_channel::{unbounded, Receiver};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Alias de compatibilitate cu structura `TelemetryFrameV2` din `aero-cockpit-server`.
pub type TrainingProgressTelemetry = TrainingEpochTelemetry;

/// Telemetria detaliată a unei epoci de antrenare în umbră, transmisă live prin WebSocket către HUD.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainingEpochTelemetry {
    /// Indexul epocii curente (1-indexat).
    pub epoch: usize,
    /// Numărul total de epoci planificate în sesiunea curentă de fine-tuning.
    pub total_epochs: usize,
    /// Pierderea totală medie pe epocă ($\mathcal{L}_{\text{total}}$).
    pub total_loss: f64,
    /// Componenta Heteroscedastic NLL pentru viteza față de sol.
    pub vel_nll: f64,
    /// Componenta Heteroscedastic NLL pentru viteza ceții.
    pub fog_nll: f64,
    /// RMSE pe viteza față de sol ($\text{m/s}$).
    pub vel_rmse_mps: f64,
    /// RMSE pe viteza ceții ($\text{m/s}$).
    pub fog_rmse_mps: f64,
    /// Reziduul fizic Inverse-Depth PINN.
    pub pinn_residual: f64,
    /// Componenta Binary Cross-Entropy pentru detecția anticipativă a rafalei.
    pub gust_bce: f64,
    /// Eroarea pătratică medie pe ghidajul terminal ALS RWY 09.
    pub als_mse: f64,
    /// Durata de calcul pe GPU/CPU a epocii în milisecunde.
    pub gpu_time_ms: f64,
    /// Versiunea modelului înghețat publicat prin `ArcSwap` la finalul epocii.
    pub model_version_epoch: u64,
    /// `true` dacă sesiunea de fine-tuning s-a încheiat complet.
    pub is_completed: bool,
}

/// Configurația antrenorului în umbră (`ShadowTrainer`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowTrainerConfig {
    /// Rata de învățare pentru `candle_nn::AdamW`.
    pub learning_rate: f64,
    /// Coeficientul de regularizare L2 (`weight_decay`) pentru `AdamW`.
    pub weight_decay: f64,
    /// Numărul de mini-loturi procesate per epocă.
    pub batches_per_epoch: usize,
    /// Pauza în milisecunde între pașii de gradient (`2 ms` conform specificației) pentru a evita
    /// contenția GPU cu bucla de $60\text{ Hz}$.
    pub inter_step_sleep_ms: u64,
    /// Calea opțională unde se salvează atomic `models/fog_optical_nav_v2_lrcv.safetensors`.
    pub checkpoint_path: Option<PathBuf>,
    /// Configurația arhitecturii `FogOpticalNavNetV2`.
    pub net_config: FogNavNetConfig,
    /// Ponderile funcției de cost fizic-informate `HeteroscedasticPinnLoss`.
    pub loss_config: PinnLossConfig,
    /// Configurația generatorului sintetic 6-DoF.
    pub domain_config: DomainRandomizationConfig,
}

impl Default for ShadowTrainerConfig {
    fn default() -> Self {
        Self {
            learning_rate: 3e-3,
            weight_decay: 1e-4,
            batches_per_epoch: 4,
            inter_step_sleep_ms: 2,
            checkpoint_path: None,
            net_config: FogNavNetConfig::default(),
            loss_config: PinnLossConfig::default(),
            domain_config: DomainRandomizationConfig::default(),
        }
    }
}

/// Handle returnat la lansarea firului dedicat `aero-cuda-shadow-trainer`.
pub struct ShadowTrainingHandle {
    /// Canal non-blocking `crossbeam_channel` pe care sosesc metricile fiecărei epoci.
    pub telemetry_rx: Receiver<TrainingEpochTelemetry>,
    /// Handle-ul firului OS de antrenare.
    pub join_handle: JoinHandle<Result<Vec<TrainingEpochTelemetry>, AeroAiError>>,
}

/// Antrenorul în umbră (`ShadowTrainer`) care operează pe un `VarMap` complet izolat de inferența la $60\text{ Hz}$.
pub struct ShadowTrainer {
    varmap: VarMap,
    trainable_model: FogOpticalNavNetV2,
    optimizer: AdamW,
    loss_fn: HeteroscedasticPinnLoss,
    config: ShadowTrainerConfig,
    device: Device,
    version_counter: u64,
}

impl ShadowTrainer {
    /// Construiește un `ShadowTrainer` nou cu un `VarMap` proaspăt inițializat pe `device`.
    pub fn new(config: ShadowTrainerConfig, device: Device) -> Result<Self, AeroAiError> {
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let trainable_model = FogOpticalNavNetV2::new(config.net_config, vb)?;

        let adamw_params = ParamsAdamW {
            lr: config.learning_rate,
            weight_decay: config.weight_decay,
            ..ParamsAdamW::default()
        };
        let optimizer = AdamW::new(varmap.all_vars(), adamw_params)?;
        let loss_fn = HeteroscedasticPinnLoss::new(config.loss_config);

        Ok(Self {
            varmap,
            trainable_model,
            optimizer,
            loss_fn,
            config,
            device,
            version_counter: 0,
        })
    }

    /// Construiește un `ShadowTrainer` inițializat cu greutățile unui [`FrozenFogNavModel`] existent,
    /// copiind valorile tensorilor într-un `VarMap` nou (fără nicio partajare de memorie mutabilă).
    pub fn from_frozen_model(
        frozen: &FrozenFogNavModel,
        config: ShadowTrainerConfig,
    ) -> Result<Self, AeroAiError> {
        let device = frozen.device().clone();
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let trainable_model = FogOpticalNavNetV2::new(config.net_config, vb)?;

        // Copiem greutățile din modelul înghețat în noul VarMap izolat
        {
            let data = varmap.data().lock().map_err(|e| {
                AeroAiError::ShadowTrainerError(format!("Failed to lock isolated VarMap: {e}"))
            })?;
            for (name, var) in data.iter() {
                if let Some(src_tensor) = frozen.detached_tensors().get(name) {
                    let detached_copy = src_tensor.detach().to_device(&device)?;
                    var.set(&detached_copy)?;
                }
            }
        }

        let adamw_params = ParamsAdamW {
            lr: config.learning_rate,
            weight_decay: config.weight_decay,
            ..ParamsAdamW::default()
        };
        let optimizer = AdamW::new(varmap.all_vars(), adamw_params)?;
        let loss_fn = HeteroscedasticPinnLoss::new(config.loss_config);

        Ok(Self {
            varmap,
            trainable_model,
            optimizer,
            loss_fn,
            config,
            device,
            version_counter: frozen.version_epoch(),
        })
    }

    /// Returnează configurația curentă a antrenorului.
    #[inline]
    pub const fn config(&self) -> &ShadowTrainerConfig {
        &self.config
    }

    /// Evaluează funcția de pierdere pe un lot `SyntheticBatch` fără pas de actualizare a gradienților.
    pub fn evaluate_batch(&self, batch: &SyntheticBatch) -> Result<LossBreakdown, AeroAiError> {
        let preds = self.trainable_model.forward_step(
            &batch.visual_tensor,
            &batch.kinematic_tensor,
            None,
        )?;
        let (_loss_tensor, breakdown) = self.loss_fn.compute_total_loss(&preds, &batch.targets)?;
        Ok(breakdown)
    }

    /// Execută un pas complet de antrenare (`forward_step` + `compute_total_loss` + `AdamW::backward_step`)
    /// pe un singur mini-lot `SyntheticBatch`.
    pub fn train_mini_batch(
        &mut self,
        batch: &SyntheticBatch,
    ) -> Result<LossBreakdown, AeroAiError> {
        let preds = self.trainable_model.forward_step(
            &batch.visual_tensor,
            &batch.kinematic_tensor,
            None,
        )?;
        let (loss_tensor, breakdown) = self.loss_fn.compute_total_loss(&preds, &batch.targets)?;
        self.optimizer.backward_step(&loss_tensor)?;
        Ok(breakdown)
    }

    /// Exportă greutățile curente din `VarMap`-ul de antrenare într-o nouă instanță [`FrozenFogNavModel`]
    /// cu tensori complet detașați (`.detach()`).
    pub fn export_frozen_model(&mut self) -> Result<FrozenFogNavModel, AeroAiError> {
        self.version_counter += 1;
        FrozenFogNavModel::from_varmap(
            &self.varmap,
            self.config.net_config,
            &self.device,
            self.version_counter,
        )
    }

    /// Antrenează sincron `num_epochs` epoci și, dacă este furnizat `shared_engine`,
    /// execută swap-ul atomic `ArcSwap::store` la finalul fiecărei epoci.
    pub fn train_epochs_sync(
        &mut self,
        num_epochs: usize,
        shared_engine: Option<&SharedFrozenInferenceEngine>,
    ) -> Result<Vec<TrainingEpochTelemetry>, AeroAiError> {
        let total_epochs = num_epochs.max(1);
        let steps_per_epoch = self.config.batches_per_epoch.max(1);
        let mut generator = Synthetic6DoFFogGenerator::new(
            self.config.net_config,
            self.config.domain_config,
            self.device.clone(),
            DEFAULT_LRCV_RNG_SEED,
        );

        let mut history = Vec::with_capacity(total_epochs);

        for epoch_idx in 1..=total_epochs {
            let t0 = Instant::now();
            let mut sum_total = 0.0;
            let mut sum_vel_nll = 0.0;
            let mut sum_fog_nll = 0.0;
            let mut sum_vel_rmse = 0.0;
            let mut sum_fog_rmse = 0.0;
            let mut sum_pinn = 0.0;
            let mut sum_gust = 0.0;
            let mut sum_als = 0.0;

            for step in 0..steps_per_epoch {
                let batch = generator.next_batch()?;
                let b_loss = self.train_mini_batch(&batch)?;

                sum_total += b_loss.total_loss;
                sum_vel_nll += b_loss.vel_nll;
                sum_fog_nll += b_loss.fog_nll;
                sum_vel_rmse += b_loss.vel_rmse_mps;
                sum_fog_rmse += b_loss.fog_rmse_mps;
                sum_pinn += b_loss.pinn_residual;
                sum_gust += b_loss.gust_bce;
                sum_als += b_loss.als_mse;

                if self.config.inter_step_sleep_ms > 0 && step + 1 < steps_per_epoch {
                    thread::sleep(Duration::from_millis(self.config.inter_step_sleep_ms));
                }
            }

            let inv_n = 1.0 / (steps_per_epoch as f64);
            let new_frozen = self.export_frozen_model()?;
            let version_epoch = new_frozen.version_epoch();

            if let Some(ref ckpt_path) = self.config.checkpoint_path {
                let _ = new_frozen.save_safetensors(ckpt_path);
            }

            if let Some(engine) = shared_engine {
                engine.swap_active_model(Arc::new(new_frozen));
            }

            let gpu_time_ms = t0.elapsed().as_secs_f64() * 1000.0;
            let epoch_telemetry = TrainingEpochTelemetry {
                epoch: epoch_idx,
                total_epochs,
                total_loss: sum_total * inv_n,
                vel_nll: sum_vel_nll * inv_n,
                fog_nll: sum_fog_nll * inv_n,
                vel_rmse_mps: sum_vel_rmse * inv_n,
                fog_rmse_mps: sum_fog_rmse * inv_n,
                pinn_residual: sum_pinn * inv_n,
                gust_bce: sum_gust * inv_n,
                als_mse: sum_als * inv_n,
                gpu_time_ms,
                model_version_epoch: version_epoch,
                is_completed: epoch_idx == total_epochs,
            };

            if let Some(engine) = shared_engine {
                engine
                    .latest_training_telemetry
                    .store(Some(Arc::new(epoch_telemetry.clone())));
            }

            history.push(epoch_telemetry);
        }

        Ok(history)
    }

    /// Lansează pe un fir OS dedicat (`aero-cuda-shadow-trainer`) antrenarea live pe GPU/CPU
    /// pentru butonul **`[⚡ LIVE CUDA FINE-TUNING (15s)]`**, fără a bloca bucla de $60\text{ Hz}$.
    pub fn spawn_live_cuda_finetuning(
        shared_engine: SharedFrozenInferenceEngine,
        trainer_config: ShadowTrainerConfig,
        epochs: usize,
    ) -> Result<ShadowTrainingHandle, AeroAiError> {
        let (tx, rx) = unbounded();
        let initial_frozen = shared_engine.load_active_model();
        let mut trainer = Self::from_frozen_model(&initial_frozen, trainer_config)?;

        let join_handle = thread::Builder::new()
            .name("aero-cuda-shadow-trainer".to_string())
            .spawn(move || -> Result<Vec<TrainingEpochTelemetry>, AeroAiError> {
                let total_epochs = epochs.max(1);
                let steps_per_epoch = trainer.config.batches_per_epoch.max(1);
                let sleep_ms = trainer.config.inter_step_sleep_ms.max(2);

                let mut generator = Synthetic6DoFFogGenerator::new(
                    trainer.config.net_config,
                    trainer.config.domain_config,
                    trainer.device.clone(),
                    DEFAULT_LRCV_RNG_SEED,
                );

                let mut all_telemetry = Vec::with_capacity(total_epochs);

                for epoch_idx in 1..=total_epochs {
                    let t0 = Instant::now();
                    let mut sum_total = 0.0;
                    let mut sum_vel_nll = 0.0;
                    let mut sum_fog_nll = 0.0;
                    let mut sum_vel_rmse = 0.0;
                    let mut sum_fog_rmse = 0.0;
                    let mut sum_pinn = 0.0;
                    let mut sum_gust = 0.0;
                    let mut sum_als = 0.0;

                    for _step in 0..steps_per_epoch {
                        let batch = generator.next_batch()?;
                        let b_loss = trainer.train_mini_batch(&batch)?;

                        sum_total += b_loss.total_loss;
                        sum_vel_nll += b_loss.vel_nll;
                        sum_fog_nll += b_loss.fog_nll;
                        sum_vel_rmse += b_loss.vel_rmse_mps;
                        sum_fog_rmse += b_loss.fog_rmse_mps;
                        sum_pinn += b_loss.pinn_residual;
                        sum_gust += b_loss.gust_bce;
                        sum_als += b_loss.als_mse;

                        // Cedăm 2 ms între pașii de gradient pentru a lăsa GPU-ul 100% liber pentru cadrul de 60 Hz
                        thread::sleep(Duration::from_millis(sleep_ms));
                    }

                    let inv_n = 1.0 / (steps_per_epoch as f64);
                    let new_frozen = trainer.export_frozen_model()?;
                    let version_epoch = new_frozen.version_epoch();

                    if let Some(ref ckpt_path) = trainer.config.checkpoint_path {
                        let _ = new_frozen.save_safetensors(ckpt_path);
                    }

                    // Swap atomic Wait-Free (< 5 ns pentru cititorul de 60 Hz)
                    shared_engine.swap_active_model(Arc::new(new_frozen));

                    let gpu_time_ms = t0.elapsed().as_secs_f64() * 1000.0;
                    let telemetry = TrainingEpochTelemetry {
                        epoch: epoch_idx,
                        total_epochs,
                        total_loss: sum_total * inv_n,
                        vel_nll: sum_vel_nll * inv_n,
                        fog_nll: sum_fog_nll * inv_n,
                        vel_rmse_mps: sum_vel_rmse * inv_n,
                        fog_rmse_mps: sum_fog_rmse * inv_n,
                        pinn_residual: sum_pinn * inv_n,
                        gust_bce: sum_gust * inv_n,
                        als_mse: sum_als * inv_n,
                        gpu_time_ms,
                        model_version_epoch: version_epoch,
                        is_completed: epoch_idx == total_epochs,
                    };

                    shared_engine
                        .latest_training_telemetry
                        .store(Some(Arc::new(telemetry.clone())));
                    let _ = tx.send(telemetry.clone());
                    all_telemetry.push(telemetry);
                }

                Ok(all_telemetry)
            })
            .map_err(|e| {
                AeroAiError::ShadowTrainerError(format!(
                    "Failed to spawn aero-cuda-shadow-trainer thread: {e}"
                ))
            })?;

        Ok(ShadowTrainingHandle {
            telemetry_rx: rx,
            join_handle,
        })
    }
}
