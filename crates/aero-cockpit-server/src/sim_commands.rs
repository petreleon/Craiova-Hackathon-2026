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
    /// Activează redarea și fuziunea metricilor extrase din videoclipuri reale de aviație (aterizare în ceață)
    /// din `real_videos/` (`🎬 TEST REAL FOG VIDEO`).
    ReplayRealVideo,
    /// Configurează manual trenul de aterizare (`gear_down`) și sistemul `Autobrake` (`autobrake_armed`).
    ConfigureLandingSystems {
        gear_down: Option<bool>,
        autobrake_armed: Option<bool>,
    },
    /// Setează vântul lateral / frontal continuu în simulator (`wy_ned_mps`: pozitiv = împinge spre dreapta pistei).
    SetCrosswind {
        wx_ned_mps: f64,
        wy_ned_mps: f64,
        turbulence_intensity: f64,
    },
    /// Activează/dezactivează protecția în buclă închisă `AERO-SHIELD` (`enabled = false` demonstrează
    /// ce pățește un avion clasic când urmează orbește semnalul GPS falsificat!).
    SetShieldProtection {
        enabled: bool,
    },
    /// Activează/dezactivează asistența Autoland CAT IIIc transmisă înapoi către FlightGear prin UDP (`:5506`).
    SetFlightGearAutoland {
        enabled: bool,
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
                    scenario_id: scenario_id.clamp(1, 4),
                }
            }
            SimCommand::ReplayRealVideo => SimCommand::ReplayRealVideo,
            SimCommand::ConfigureLandingSystems {
                gear_down,
                autobrake_armed,
            } => SimCommand::ConfigureLandingSystems {
                gear_down,
                autobrake_armed,
            },
            SimCommand::SetCrosswind {
                wx_ned_mps,
                wy_ned_mps,
                turbulence_intensity,
            } => SimCommand::SetCrosswind {
                wx_ned_mps: sanitize_f64(wx_ned_mps, -35.0, 35.0, 0.0),
                wy_ned_mps: sanitize_f64(wy_ned_mps, -35.0, 35.0, 0.0),
                turbulence_intensity: sanitize_f64(turbulence_intensity, 0.0, 1.0, 0.25),
            },
            SimCommand::SetShieldProtection { enabled } => {
                SimCommand::SetShieldProtection { enabled }
            }
            SimCommand::SetFlightGearAutoland { enabled } => {
                SimCommand::SetFlightGearAutoland { enabled }
            }
            SimCommand::ResetApproach => SimCommand::ResetApproach,
        }
    }

    /// Parsează atât formatul canonic `{ "type": "...", "payload": { ... } }`, cât și
    /// variantele shorthand/snake_case folosite de scripturile de audit sau REST (`inject_sudden_fog_gust`,
    /// `turn_right`, `turn_left`, `pitch_up`, `pitch_down`, `roll_right`, `roll_left`, `accelerate`, `decelerate`,
    /// `replay_real_video`, `reset`).
    #[must_use]
    pub fn parse_flexible_json(val: &serde_json::Value) -> Option<Self> {
        if let Ok(cmd) = serde_json::from_value::<SimCommand>(val.clone()) {
            return Some(cmd.sanitize());
        }

        let cmd_type = val.get("type").and_then(|v| v.as_str())?;
        let parsed = match cmd_type {
            "inject_sudden_fog_gust" | "InjectSuddenFogGust" => {
                let intensity = val
                    .get("intensity_mps")
                    .or_else(|| val.get("wy_ned_mps"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(14.5);
                let wx = val
                    .get("wx_ned_mps")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(-4.0);
                let wz = val
                    .get("wz_ned_mps")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1.2);
                SimCommand::InjectSuddenFogGust {
                    wx_ned_mps: wx,
                    wy_ned_mps: intensity,
                    wz_ned_mps: wz,
                    turbulence_intensity: 0.75,
                }
            }
            "turn_right" => SimCommand::ManeuverStep {
                d_yaw_deg: 6.0,
                d_pitch_deg: 0.0,
                d_roll_deg: 12.0,
                d_throttle_mps: 0.0,
            },
            "turn_left" => SimCommand::ManeuverStep {
                d_yaw_deg: -6.0,
                d_pitch_deg: 0.0,
                d_roll_deg: -12.0,
                d_throttle_mps: 0.0,
            },
            "pitch_up" | "climb" => SimCommand::ManeuverStep {
                d_yaw_deg: 0.0,
                d_pitch_deg: 4.0,
                d_roll_deg: 0.0,
                d_throttle_mps: 0.0,
            },
            "pitch_down" | "descend" => SimCommand::ManeuverStep {
                d_yaw_deg: 0.0,
                d_pitch_deg: -4.0,
                d_roll_deg: 0.0,
                d_throttle_mps: 0.0,
            },
            "roll_right" => SimCommand::ManeuverStep {
                d_yaw_deg: 0.0,
                d_pitch_deg: 0.0,
                d_roll_deg: 15.0,
                d_throttle_mps: 0.0,
            },
            "roll_left" => SimCommand::ManeuverStep {
                d_yaw_deg: 0.0,
                d_pitch_deg: 0.0,
                d_roll_deg: -15.0,
                d_throttle_mps: 0.0,
            },
            "accelerate" => SimCommand::ManeuverStep {
                d_yaw_deg: 0.0,
                d_pitch_deg: 0.0,
                d_roll_deg: 0.0,
                d_throttle_mps: 5.0,
            },
            "decelerate" => SimCommand::ManeuverStep {
                d_yaw_deg: 0.0,
                d_pitch_deg: 0.0,
                d_roll_deg: 0.0,
                d_throttle_mps: -5.0,
            },
            "replay_real_video" | "ReplayRealVideo" => SimCommand::ReplayRealVideo,
            "configure_landing_systems" | "ConfigureLandingSystems" | "arm_landing" => {
                let gear_down = val.get("gear_down").and_then(|v| v.as_bool());
                let autobrake_armed = val.get("autobrake_armed").and_then(|v| v.as_bool());
                SimCommand::ConfigureLandingSystems {
                    gear_down: gear_down.or(Some(true)),
                    autobrake_armed: autobrake_armed.or(Some(true)),
                }
            }
            "set_crosswind" | "SetCrosswind" | "crosswind" => {
                let wy = val
                    .get("wy_ned_mps")
                    .or_else(|| val.get("crosswind_mps"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let wx = val
                    .get("wx_ned_mps")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let turb = val
                    .get("turbulence_intensity")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.25);
                SimCommand::SetCrosswind {
                    wx_ned_mps: wx,
                    wy_ned_mps: wy,
                    turbulence_intensity: turb,
                }
            }
            "set_shield_protection" | "SetShieldProtection" => {
                let enabled = val.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
                SimCommand::SetShieldProtection { enabled }
            }
            "set_flightgear_autoland" | "SetFlightGearAutoland" => {
                let enabled = val.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                SimCommand::SetFlightGearAutoland { enabled }
            }
            "reset" | "reset_approach" | "ResetApproach" => SimCommand::ResetApproach,
            _ => return None,
        };

        Some(parsed.sanitize())
    }
}
