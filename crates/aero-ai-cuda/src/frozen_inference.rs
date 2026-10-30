//! # Motorul de Inferență *Lock-Free* la $60\text{ Hz}$ (`FrozenFogNavModel` + `ArcSwap`)
//!
//! Implementează separarea strictă dintre inferența de înaltă frecvență ($60\text{ Hz}$, fereastră de
//! $16.66\text{ ms}$, buget AI $< 1.8\text{ ms}$) și firul de antrenare în umbră (`ShadowTrainer`):
//! - [`FrozenFogNavModel`] deține exclusiv tensori detașați (`.detach()`), fără graf de gradient
//!   și fără niciun `Mutex` partajat cu `VarMap`-ul de antrenare.
//! - [`SharedFrozenInferenceEngine`] încapsulează `Arc<ArcSwap<FrozenFogNavModel>>`, permițând
//!   citirea *wait-free* în $< 5\text{ ns}$ pe firul Real-Time `aero-60hz-rt-loop` chiar în timp ce
//!   `ShadowTrainer` actualizează greutățile pe GPU.

use crate::model_v2::{
    FogNavForwardTensors, FogNavNetConfig, FogOpticalNavNetV2, NavPredictionOutput,
    KINEMATIC_VECTOR_DIM,
};
use crate::shadow_trainer::TrainingEpochTelemetry;
use crate::{AeroAiError, ComputeBackendInfo};
use arc_swap::{ArcSwap, ArcSwapOption};
use candle_core::{DType, Device, Tensor};
use candle_nn::rnn::GRUState;
use candle_nn::{VarBuilder, VarMap};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

/// Alias de compatibilitate cu diagrama din `arhitectura/02_aero_ai_cuda.md`.
pub type FrozenFogNavNetV2 = FrozenFogNavModel;

/// Statistici de latență pentru un pas de inferență la $60\text{ Hz}$.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InferenceTimingStats {
    /// Timpul de execuție al pasului `forward_step` în milisecunde (țintă $< 1.8\text{ ms}$).
    pub latency_ms: f64,
    /// Versiunea (epoca) modelului înghețat folosit la acest cadru.
    pub model_version_epoch: u64,
    /// `true` dacă execuția a rulat pe un dispozitiv CUDA.
    pub is_cuda: bool,
}

/// Model neural înghețat (`FrozenFogNavModel`) ale cărui greutăți sunt tensori detașați
/// (`Tensor::detach()`), complet decuplați de orice `VarMap` de antrenare.
#[derive(Debug, Clone)]
pub struct FrozenFogNavModel {
    model: FogOpticalNavNetV2,
    detached_tensors: HashMap<String, Tensor>,
    device: Device,
    dtype: DType,
    version_epoch: u64,
}

impl FrozenFogNavModel {
    /// Construiește un `FrozenFogNavModel` prin extragerea și detașarea (`.detach()`) tuturor
    /// tensorilor dintr-un `candle_nn::VarMap`.
    pub fn from_varmap(
        varmap: &VarMap,
        config: FogNavNetConfig,
        device: &Device,
        version_epoch: u64,
    ) -> Result<Self, AeroAiError> {
        let mut detached_tensors = HashMap::new();
        {
            let data = varmap.data().lock().map_err(|e| {
                AeroAiError::ShadowTrainerError(format!("Failed to lock VarMap mutex: {e}"))
            })?;
            for (name, var) in data.iter() {
                let detached = var.as_tensor().detach().to_device(device)?;
                detached_tensors.insert(name.clone(), detached);
            }
        }

        let vb = VarBuilder::from_tensors(detached_tensors.clone(), DType::F32, device);
        let model = FogOpticalNavNetV2::new(config, vb)?;

        Ok(Self {
            model,
            detached_tensors,
            device: device.clone(),
            dtype: DType::F32,
            version_epoch,
        })
    }

    /// Construiește un `FrozenFogNavModel` direct dintr-un dicționar de tensori deja încărcați.
    pub fn from_detached_map(
        mut tensor_map: HashMap<String, Tensor>,
        config: FogNavNetConfig,
        device: &Device,
        version_epoch: u64,
    ) -> Result<Self, AeroAiError> {
        for tensor in tensor_map.values_mut() {
            *tensor = tensor.detach().to_device(device)?;
        }
        let vb = VarBuilder::from_tensors(tensor_map.clone(), DType::F32, device);
        let model = FogOpticalNavNetV2::new(config, vb)?;

        Ok(Self {
            model,
            detached_tensors: tensor_map,
            device: device.clone(),
            dtype: DType::F32,
            version_epoch,
        })
    }

    /// Inițializează un `FrozenFogNavModel` nou (cu inițializarea standard Kaiming/Uniform din `candle_nn`)
    /// fără a păstra niciun `VarMap` activ după construire.
    pub fn new_random_initialized(
        config: FogNavNetConfig,
        device: &Device,
    ) -> Result<Self, AeroAiError> {
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, device);
        let _init_model = FogOpticalNavNetV2::new(config, vb)?;
        Self::from_varmap(&varmap, config, device, 0)
    }

    /// Returnează referința către rețeaua subiacentă `FogOpticalNavNetV2`.
    #[inline]
    pub const fn network(&self) -> &FogOpticalNavNetV2 {
        &self.model
    }

    /// Returnează dicționarul de tensori detașați (util pentru inițializarea `ShadowTrainer` sau export `.safetensors`).
    #[inline]
    pub const fn detached_tensors(&self) -> &HashMap<String, Tensor> {
        &self.detached_tensors
    }

    /// Returnează dispozitivul pe care rezidă greutățile modelului.
    #[inline]
    pub const fn device(&self) -> &Device {
        &self.device
    }

    /// Returnează tipul de date al tensorilor (`DType::F32`).
    #[inline]
    pub const fn dtype(&self) -> DType {
        self.dtype
    }

    /// Returnează versiunea (numărul epocii) acestui model înghețat.
    #[inline]
    pub const fn version_epoch(&self) -> u64 {
        self.version_epoch
    }

    ///Returnează configurația rețelei.
    #[inline]
    pub const fn config(&self) -> &FogNavNetConfig {
        self.model.config()
    }

    /// Creează starea `GRUState` zero pentru inferența secvențială.
    pub fn zero_gru_state(&self, batch_size: usize) -> Result<GRUState, AeroAiError> {
        let st = self.model.zero_gru_state(batch_size)?;
        Ok(GRUState { h: st.h().detach() })
    }

    /// Execută un pas de inferență la $60\text{ Hz}$ fără alocare de graf de gradient:
    /// - Detașează intrările și noua stare ascunsă `GRUState { h: h_t.detach() }` pentru a preveni
    ///   acumularea memoriei pe zboruri lungi.
    /// - Nu conține niciun `.unwrap()` sau `.expect()`.
    pub fn infer_step(
        &self,
        visual_tensor: &Tensor,
        kinematic_tensor: &Tensor,
        gru_state: &mut Option<GRUState>,
    ) -> Result<NavPredictionOutput, AeroAiError> {
        let v_in = visual_tensor.detach().to_device(&self.device)?;
        let k_in = kinematic_tensor.detach().to_device(&self.device)?;

        let detached_prev_state = match gru_state.as_ref() {
            Some(st) => Some(GRUState {
                h: st.h().detach().to_device(&self.device)?,
            }),
            None => None,
        };

        let out: FogNavForwardTensors =
            self.model
                .forward_step(&v_in, &k_in, detached_prev_state.as_ref())?;

        // Păstrăm exclusiv tensorul detașat în starea recurentă la 60 Hz
        *gru_state = Some(GRUState {
            h: out.next_gru_state.h().detach(),
        });

        out.decode_first_sample()
    }

    /// Execută un pas de inferență direct din slice-uri contigue `&[f32]` (pre-alocate în `aero-vision-fog`).
    pub fn infer_step_from_slices(
        &self,
        visual_slice: &[f32],
        kinematic_slice: &[f32; KINEMATIC_VECTOR_DIM],
        gru_state: &mut Option<GRUState>,
    ) -> Result<NavPredictionOutput, AeroAiError> {
        let cfg = self.model.config();
        let expected_len = cfg.visual_channels * cfg.image_size * cfg.image_size;
        if visual_slice.len() != expected_len {
            return Err(AeroAiError::InvalidTensorShape {
                tensor_name: "visual_slice",
                expected: vec![1, cfg.visual_channels, cfg.image_size, cfg.image_size],
                actual: vec![visual_slice.len()],
            });
        }

        let visual_tensor = Tensor::from_slice(
            visual_slice,
            (1, cfg.visual_channels, cfg.image_size, cfg.image_size),
            &self.device,
        )?;
        let kinematic_tensor = Tensor::from_slice(
            kinematic_slice.as_slice(),
            (1, KINEMATIC_VECTOR_DIM),
            &self.device,
        )?;

        self.infer_step(&visual_tensor, &kinematic_tensor, gru_state)
    }

    /// Metodă de inferență garantată fără eroare (*Zero-Panic Fallback*) pentru bucla critică de $60\text{ Hz}$.
    /// În cazul unei erori neprevăzute de dispozitiv, returnează `NavPredictionOutput::safe_fallback_high_uncertainty()`.
    pub fn infer_step_safe_fallback(
        &self,
        visual_tensor: &Tensor,
        kinematic_tensor: &Tensor,
        gru_state: &mut Option<GRUState>,
    ) -> NavPredictionOutput {
        match self.infer_step(visual_tensor, kinematic_tensor, gru_state) {
            Ok(pred) => pred,
            Err(err) => {
                tracing::warn!(
                    "AERO-AI-CUDA: infer_step fallback triggered ({}), returning high-uncertainty prior",
                    err
                );
                NavPredictionOutput::safe_fallback_high_uncertainty()
            }
        }
    }

    /// Salvează atomic toate greutățile detașate în format `.safetensors`.
    /// Scrie mai întâi într-un fișier temporar `.tmp` și apoi execută `std::fs::rename`
    /// pentru a garanta că fișierul de pe disc nu este niciodată corupt sau incomplet.
    pub fn save_safetensors<P: AsRef<Path>>(&self, path: P) -> Result<(), AeroAiError> {
        let target_path = path.as_ref();
        if let Some(parent) = target_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| AeroAiError::SafetensorsIo {
                    path: target_path.display().to_string(),
                    details: format!("Failed to create parent directory: {e}"),
                })?;
            }
        }

        let tmp_path = target_path.with_extension("safetensors.tmp");
        let mut cpu_tensors = HashMap::with_capacity(self.detached_tensors.len());
        for (name, tensor) in &self.detached_tensors {
            cpu_tensors.insert(name.clone(), tensor.detach().to_device(&Device::Cpu)?);
        }

        candle_core::safetensors::save(&cpu_tensors, &tmp_path).map_err(|e| {
            AeroAiError::SafetensorsIo {
                path: tmp_path.display().to_string(),
                details: format!("candle_core::safetensors::save failed: {e}"),
            }
        })?;

        std::fs::rename(&tmp_path, target_path).map_err(|e| AeroAiError::SafetensorsIo {
            path: target_path.display().to_string(),
            details: format!("Atomic rename failed: {e}"),
        })?;

        Ok(())
    }

    /// Încarcă un `FrozenFogNavModel` dintr-un fișier `.safetensors` direct pe `device`.
    pub fn load_safetensors<P: AsRef<Path>>(
        path: P,
        config: FogNavNetConfig,
        device: &Device,
        version_epoch: u64,
    ) -> Result<Self, AeroAiError> {
        let target_path = path.as_ref();
        let loaded_map = candle_core::safetensors::load(target_path, device).map_err(|e| {
            AeroAiError::SafetensorsIo {
                path: target_path.display().to_string(),
                details: format!("candle_core::safetensors::load failed: {e}"),
            }
        })?;

        Self::from_detached_map(loaded_map, config, device, version_epoch)
    }
}

/// Motorul partajat de inferență *Wait-Free* bazat pe [`ArcSwap<FrozenFogNavModel>`].
/// Poate fi clonat ieftin (`Clone`) între firul Real-Time de $60\text{ Hz}$ și firul `ShadowTrainer`.
#[derive(Debug, Clone)]
pub struct SharedFrozenInferenceEngine {
    /// Pointerul atomic *lock-free* către modelul înghețat activ (`load()` în $< 5\text{ ns}$).
    pub active_model: Arc<ArcSwap<FrozenFogNavModel>>,
    /// Informațiile despre dispozitivul de calcul (CUDA GPU sau CPU SIMD).
    pub backend_info: ComputeBackendInfo,
    /// Telemetria ultimei epoci de antrenare din `ShadowTrainer` (citită *wait-free* de HUD).
    pub latest_training_telemetry: Arc<ArcSwapOption<TrainingEpochTelemetry>>,
}

impl SharedFrozenInferenceEngine {
    /// Creează un `SharedFrozenInferenceEngine` nou cu greutăți inițializate pe `backend_info.device`.
    pub fn new(
        backend_info: ComputeBackendInfo,
        config: FogNavNetConfig,
    ) -> Result<Self, AeroAiError> {
        let initial_model = FrozenFogNavModel::new_random_initialized(config, &backend_info.device)?;
        Ok(Self {
            active_model: Arc::new(ArcSwap::from_pointee(initial_model)),
            backend_info,
            latest_training_telemetry: Arc::new(ArcSwapOption::empty()),
        })
    }

    /// Încearcă încărcarea checkpoint-ului `.safetensors` de la `checkpoint_path` (`< 0.4s` Warm-Start);
    /// dacă fișierul nu există încă, inițializează un model nou și îl salvează pe disc.
    pub fn load_or_init<P: AsRef<Path>>(
        checkpoint_path: P,
        backend_info: ComputeBackendInfo,
        config: FogNavNetConfig,
    ) -> Result<Self, AeroAiError> {
        let path_ref = checkpoint_path.as_ref();
        let model = if path_ref.exists() {
            match FrozenFogNavModel::load_safetensors(path_ref, config, &backend_info.device, 1) {
                Ok(m) => {
                    tracing::info!(
                        "AERO-AI-CUDA: Warm-started FrozenFogNavModel from {}",
                        path_ref.display()
                    );
                    m
                }
                Err(err) => {
                    tracing::warn!(
                        "AERO-AI-CUDA: Could not load checkpoint {} ({}), initializing fresh weights",
                        path_ref.display(),
                        err
                    );
                    FrozenFogNavModel::new_random_initialized(config, &backend_info.device)?
                }
            }
        } else {
            let fresh = FrozenFogNavModel::new_random_initialized(config, &backend_info.device)?;
            let _ = fresh.save_safetensors(path_ref);
            fresh
        };

        Ok(Self {
            active_model: Arc::new(ArcSwap::from_pointee(model)),
            backend_info,
            latest_training_telemetry: Arc::new(ArcSwapOption::empty()),
        })
    }

    /// Încarcă *wait-free* (`< 5 ns`) referința către modelul înghețat activ.
    #[inline]
    pub fn load_active_model(&self) -> arc_swap::Guard<Arc<FrozenFogNavModel>> {
        self.active_model.load()
    }

    /// Înlocuiește atomic (`wait-free`) modelul activ cu o nouă instanță `FrozenFogNavModel`
    /// exportată de `ShadowTrainer`.
    #[inline]
    pub fn swap_active_model(&self, new_model: Arc<FrozenFogNavModel>) {
        self.active_model.store(new_model);
    }

    /// Returnează cea mai recentă telemetrie de antrenare live (dacă `ShadowTrainer` a rulat sau rulează).
    #[inline]
    pub fn latest_training_progress(&self) -> Option<TrainingEpochTelemetry> {
        self.latest_training_telemetry
            .load_full()
            .map(|arc_t| (*arc_t).clone())
    }

    /// Execută pasul de inferență la $60\text{ Hz}$ pe modelul activ și măsoară latența de execuție.
    pub fn infer_60hz_step(
        &self,
        visual_tensor: &Tensor,
        kinematic_tensor: &Tensor,
        gru_state: &mut Option<GRUState>,
    ) -> Result<(NavPredictionOutput, InferenceTimingStats), AeroAiError> {
        let t0 = Instant::now();
        let guard = self.active_model.load();
        let pred = guard.infer_step(visual_tensor, kinematic_tensor, gru_state)?;
        let latency_ms = t0.elapsed().as_secs_f64() * 1000.0;

        let stats = InferenceTimingStats {
            latency_ms,
            model_version_epoch: guard.version_epoch(),
            is_cuda: self.backend_info.is_cuda_accelerated,
        };

        Ok((pred, stats))
    }
}
