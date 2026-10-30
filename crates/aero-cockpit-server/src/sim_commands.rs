//! Comenzi interactive 6-DoF, atacuri Red-Team și scenarii "One-Click Director Mode"
//! cu validare numerică strictă (`sanitize_f64`) fără `.unwrap()` / `.expect()`.

use serde::{Deserialize, Serialize};

/// Funcție utilitară DO-178C DAL-A: garantează că orice număr `f64` este finit și încadrat în `[min, max]`.
/// Dacă `val` este `NaN` sau `±Infinity`, returnează `fallback.clamp(min, max)`.
#[inline]
pub fn sanitize_f64(val: f64, min: f64, max: f64, fallback: f64) -> f64 {
    let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
    let safe_fallback = if fallback.is_finite() {
        fallback.clamp(lo, hi)
    } else {
        0.5 * (lo + hi)
    };
    if val.is_finite() {
        val.clamp(lo, hi)
    } else {
        safe_fallback
    }
}

/// Comenzile trimise de la interfața 3D/HUD (prin WebSocket `/ws/telemetry` sau REST `/api/command`)
/// către thread-ul real-time `aero-60hz-rt-loop`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum SimCommand {
    /// Manevră incrementală 6-DoF: Viraj Dreapta/Stânga (`d_yaw_deg`), Urcare/Coborâre (`d_pitch_deg`),
    /// Rotire/Înclinare (`d_roll_deg`) și Accelerare/Decelerare (`d_throttle_mps`).
    ManeuverStep {
        d_yaw_deg: f64,
        d_pitch_deg: f64,
        d_roll_deg: f64,
        d_throttle_mps: f64,
    },
    /// Rate continue de comandă 6-DoF pentru pilotare din tastatură (`WASD`, `QE`, `Shift/Ctrl`).
    SetContinuousRates {
        roll_rate_dps: f64,
        pitch_rate_dps: f64,
        yaw_rate_dps: f64,
        accel_x_mps2: f64,
    },
    /// Injectează o rafală bruscă de ceață / forfecare de vânt (`🌪️ INJECT SUDDEN FOG GUST / SHEAR`)
    /// fără a modifica forțat comanda pilotului (demonstrează decuplarea ceții de avion și Optical Gust Lookahead).
    InjectSuddenFogGust {
        wx_ned_mps: f64,
        wy_ned_mps: f64,
        wz_ned_mps: f64,
        turbulence_intensity: f64,
    },
    /// Setează vizibilitatea meteorologică MOR ($25\text{ m}$ CAT IIIc $\to 5000\text{ m}$ CAVOK) și activarea SWIR/ALS.
    SetFogDensity {
        visibility_mor_m: f64,
        enable_swir_als: bool,
    },
    /// Configurează vectorii de atac Red-Team în sandbox-ul izolat (Art. 21):
    /// - `gps_jump_spoof`: Salt brusc de 250m (detectat de $\chi^2$ Mahalanobis în 1 cadru)
    /// - `gps_stealth_ramp_spoof`: Rampă lentă de $0.35\text{ m/s}^2$ (detectată de CUSUM în $<2.9\text{ s}$)
    /// - `cut_internet_comms`: Tăierea conexiunii externe Open-Meteo (trecere pe Bayesian Cache)
    /// - `poison_meteo_cache`: Otrăvirea cache-ului meteo cu vânt fals de $+32\text{ m/s}$ (respins de Optical-Aerodynamic Sanity Gate)
    ConfigureCyberAttack {
        gps_jump_spoof: bool,
        gps_stealth_ramp_spoof: bool,
        cut_internet_comms: bool,
        poison_meteo_cache: bool,
    },
    /// Lansează `ShadowTrainer` pe GPU-ul NVIDIA (`candle-core` + `candle-nn`) timp de ~15 secunde
    /// cu actualizare *zero-downtime* prin `ArcSwap`.
    StartLiveCudaFineTuning {
        epochs: usize,
    },
    /// Declanșează unul dintre cele 3 scenarii regizate determinist (`ChaCha8Rng` seed `0x4C52_4356_2026`):
    /// - `1`: ACT 1 — Straight Flight & Sudden Fog Shear ($18\text{ m/s}$ cross-gust)
    /// - `2`: ACT 2 — Dynamic 6-DoF Maneuvers in Shifting Fog (Yaw/Pitch/Roll/Accel)
    /// - `3`: ACT 3 — Full Cyber-Siege CAT IIIc Touchdown on LRCV RWY 09
    TriggerDirectorScenario {
        scenario_id: u8,
    },
    /// Resetează aeronava la începutul conului de apropiere LRCV RWY 09.
    ResetApproach,
}

impl SimCommand {
    /// Sanitizează toți parametrii numerici ai comenzii folosind `sanitize_f64`.
    #[must_use]
    pub fn sanitize(self) -> Self {
        match self {
            SimCommand::ManeuverStep {
                d_yaw_deg,
                d_pitch_deg,
                d_roll_deg,
                d_throttle_mps,
            } => SimCommand::ManeuverStep {
                d_yaw_deg: sanitize_f64(d_yaw_deg, -45.0, 45.0, 0.0),
                d_pitch_deg: sanitize_f64(d_pitch_deg, -25.0, 25.0, 0.0),
                d_roll_deg: sanitize_f64(d_roll_deg, -60.0, 60.0, 0.0),
                d_throttle_mps: sanitize_f64(d_throttle_mps, -35.0, 35.0, 0.0),
            },
            SimCommand::SetContinuousRates {
                roll_rate_dps,
                pitch_rate_dps,
                yaw_rate_dps,
                accel_x_mps2,
            } => SimCommand::SetContinuousRates {
                roll_rate_dps: sanitize_f64(roll_rate_dps, -90.0, 90.0, 0.0),
                pitch_rate_dps: sanitize_f64(pitch_rate_dps, -45.0, 45.0, 0.0),
                yaw_rate_dps: sanitize_f64(yaw_rate_dps, -60.0, 60.0, 0.0),
                accel_x_mps2: sanitize_f64(accel_x_mps2, -15.0, 15.0, 0.0),
            },
            SimCommand::InjectSuddenFogGust {
                wx_ned_mps,
                wy_ned_mps,
                wz_ned_mps,
                turbulence_intensity,
            } => SimCommand::InjectSuddenFogGust {
                wx_ned_mps: sanitize_f64(wx_ned_mps, -45.0, 45.0, 0.0),
                wy_ned_mps: sanitize_f64(wy_ned_mps, -45.0, 45.0, 18.0),
                wz_ned_mps: sanitize_f64(wz_ned_mps, -15.0, 15.0, 0.0),
                turbulence_intensity: sanitize_f64(turbulence_intensity, 0.0, 1.0, 0.65),
            },
            SimCommand::SetFogDensity {
                visibility_mor_m,
                enable_swir_als,
            } => SimCommand::SetFogDensity {
                visibility_mor_m: sanitize_f64(visibility_mor_m, 25.0, 5000.0, 75.0),
                enable_swir_als,
            },
            SimCommand::ConfigureCyberAttack {
                gps_jump_spoof,
                gps_stealth_ramp_spoof,
                cut_internet_comms,
                poison_meteo_cache,
            } => SimCommand::ConfigureCyberAttack {
                gps_jump_spoof,
                gps_stealth_ramp_spoof,
                cut_internet_comms,
                poison_meteo_cache,
            },
            SimCommand::StartLiveCudaFineTuning { epochs } => SimCommand::StartLiveCudaFineTuning {
                epochs: epochs.clamp(1, 200),
            },
            SimCommand::TriggerDirectorScenario { scenario_id } => {
                SimCommand::TriggerDirectorScenario {
                    scenario_id: scenario_id.clamp(1, 3),
                }
            }
            SimCommand::ResetApproach => SimCommand::ResetApproach,
        }
    }
}
