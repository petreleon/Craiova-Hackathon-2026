//! # `aero-cockpit-server` — AERO-SHIELD LRCV V2.0
//!
//! Bucla Real-Time $60\text{ Hz}$ pe Thread OS Dedicat (`aero-60hz-rt-loop`), Serverul WebSocket `axum`
//! cu *Zero-Lag State Conflation* (`tokio::sync::watch`), Arena celor 4 Estimatoare,
//! Antrenarea GPU CUDA (`ShadowTrainer`) și Cele 3 Scenarii Deterministe „One-Click Director Mode"
//! pentru Aeroportul Internațional Craiova (`LRCV`) — Pista RWY 09 (`44.3181° N, 23.8886° E`).

pub mod cli_train;
pub mod director_scenarios;
pub mod flightgear_bridge;
pub mod flightgear_synthetic_10;
pub mod long_range_zero_gps_test;
pub mod rt_loop_60hz;
pub mod sim_commands;
pub mod ui_framework_rust;
pub mod ui_lessons_6yo;
pub mod ui_pitch_5min;
pub mod ui_visual_engine;
pub mod ws_telemetry;

pub use sim_commands::{sanitize_f64, SimCommand};

use serde::{Deserialize, Serialize};

/// Contractul complet de telemetrie la $60\text{ Hz}$ transmis prin WebSocket (`/ws/telemetry`)
/// cu conflație zero-lag (`tokio::sync::watch`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryFrameV2 {
    pub tick: u64,
    pub sim_time_sec: f64,
    pub approach_phase: &'static str,
    pub aircraft_pose: Aircraft6DofPose,
    pub fog_and_meteo: FogMeteoTelemetry,
    pub estimators_arena: FourEstimatorsTelemetry,
    pub lrcv_guidance: LrcvRunwayGuidance,
    pub cyber_and_gpu: CyberGpuTelemetry,
    pub optical_flow_vectors: Vec<OpticalVectorHud>,
}

/// 1. Starea Adevărată 6-DoF a Aeronavei (Ground Truth) + Matricea $\mathbf{R}_b^n(\mathbf{q})$ pentru HUD.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aircraft6DofPose {
    /// `[North/Track, East/Cross-Track, Down]` (m) față de Pragul RWY 09 LRCV (`44.3181N, 23.8886E`).
    pub pos_ned_m: [f64; 3],
    /// Viteza în reperul aeronavei `[u (înainte), v (lateral), w (vertical)]` (m/s).
    pub vel_body_mps: [f64; 3],
    /// Viteza reală față de sol în reperul NED $\mathbf{v}_{ground}^n = \mathbf{R}_b^n(\mathbf{q}) \mathbf{v}^b$ (m/s).
    pub vel_ground_ned_mps: [f64; 3],
    /// Cuaternionul unitar de atitudine $\mathbf{q}_b^n = [q_w, q_x, q_y, q_z] \in SO(3)$.
    pub quat_b_to_n: [f64; 4],
    /// Matricea de rotație $3\times 3$ exactă $\mathbf{R}_b^n(\mathbf{q})$ afișată live în HUD.
    pub rot_matrix_b_to_n: [[f64; 3]; 3],
    /// Unghiurile Euler `[Roll phi, Pitch theta, Yaw psi]` în grade (exclusiv pentru afișarea HUD).
    pub euler_deg: [f64; 3],
    /// Ratele unghiulare în reperul aeronavei `[p, q, r]` (deg/s).
    pub omega_body_dps: [f64; 3],
    /// Accelerațiile specifice în reperul aeronavei `[ax, ay, az]` ($\text{m/s}^2$).
    pub accel_body_mps2: [f64; 3],
    /// Unghiul de atac aerodinamic $\alpha$ (deg).
    pub alpha_aoa_deg: f64,
    /// Unghiul de derapaj aerodinamic $\beta$ indus de rafala laterală de ceață (deg).
    pub beta_sideslip_deg: f64,
    /// Unghiul de corecție în vânt $\psi_{crab}$ comandat de autopilot (deg).
    pub crab_angle_cmd_deg: f64,
}

/// 2. Dinamica Ceții, Vântului și Meteo LRCV Craiova.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FogMeteoTelemetry {
    /// Vizibilitatea meteorologică optică $V_{MOR}$ (m).
    pub visibility_mor_m: f64,
    /// Coeficientul de extincție optică Koschmieder $\beta_{ext} = -\ln(0.05) / V_{MOR}$ ($\text{m}^{-1}$).
    pub beta_ext_koschmieder: f64,
    /// Inversa adâncimii efective a stratului de ceață $\hat{\rho}_{fog} = 1 / Z_{fog}$ ($\text{m}^{-1}$).
    pub rho_fog_scale_inv_m: f64,
    /// Inversa adâncimii solului la orizont $\rho_{ground}$ (`0.0`, garantat fără `NaN`!).
    pub rho_ground_horizon_safe: f64,
    /// Viteza reală a masei de ceață `[Wx, Wy, Wz]` în NED (m/s).
    pub true_fog_wind_ned_mps: [f64; 3],
    /// Viteza estimată a masei de ceață `[Wx, Wy, Wz]` în NED (m/s).
    pub est_fog_wind_ned_mps: [f64; 3],
    /// `true` cu $0.6\text{ s} - 1.1\text{ s}$ ÎNAINTE ca rafala de ceață să lovească fuzelajul!
    pub gust_lookahead_warning: bool,
    /// Timpul de anticipare al alertei optice de rafală (secunde, ex. `0.85`).
    pub gust_lookahead_lead_sec: f64,
    /// Scorul de anomalie optică (curl/vorticitate + forfecare în zona $40\text{ m} - 80\text{ m}$).
    pub gust_anomaly_score: f64,
    /// Sursa meteo activă: `"LIVE OPEN-METEO LRCV (44.3181N, 23.8886E)"`, `"CACHED + OPTICAL BAYESIAN"`, sau `"POISONED CACHE REJECTED (OPTICAL SANITY GATE)"`.
    pub meteo_source_status: &'static str,
    pub lrcv_temp_c: f64,
    pub lrcv_qnh_hpa: f64,
    pub lrcv_rh_pct: f64,
}

/// 3. Arena Celor 4 Estimatoare Simultane (Benchmark Vizual pentru Juriu).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FourEstimatorsTelemetry {
    pub raw_gnss_pos_ned: [f64; 3],
    pub raw_gnss_error_m: f64,
    pub pure_ins_pos_ned: [f64; 3],
    pub pure_ins_error_m: f64,
    pub naive_flow_pos_ned: [f64; 3],
    pub naive_flow_error_m: f64,
    pub aero_shield_pos_ned: [f64; 3],
    pub aero_shield_error_m: f64,
}

/// 4. Vectorul de Ghidaj către Pragul Pistei RWY 09 Craiova (`LRCV`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LrcvRunwayGuidance {
    pub distance_to_rwy09_m: f64,
    pub bearing_to_rwy09_deg: f64,
    pub heading_correction_deg: f64,
    pub glideslope_error_deg: f64,
    pub eta_touchdown_sec: f64,
    pub als_bars_detected: usize,
    pub photonic_certainty_mean: f64,
    pub swir_als_active: bool,
    #[serde(default)]
    pub flare_active: bool,
    #[serde(default)]
    pub gear_down: bool,
    #[serde(default)]
    pub autobrake_armed: bool,
    #[serde(default)]
    pub spoilers_deployed: bool,
    #[serde(default)]
    pub reverse_thrust_active: bool,
    #[serde(default)]
    pub landing_complete: bool,
    #[serde(default)]
    pub crashed: bool,
    #[serde(default)]
    pub crash_reason: String,
    #[serde(default)]
    pub touchdown_vs_mps: f64,
    #[serde(default)]
    pub touchdown_lateral_err_m: f64,
}

/// 5. Status Cyber-Security Dual-Layer & GPU AI (`candle-core`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CyberGpuTelemetry {
    pub gnss_watchdog_status: &'static str,
    pub chi2_mahalanobis_stat: f64,
    pub cusum_stealth_stat: f64,
    pub meteo_sanity_trusted: bool,
    pub meteo_discrepancy_mps: f64,
    pub gpu_device_name: String,
    pub inference_latency_ms: f64,
    pub heteroscedastic_sigma_v: [f64; 3],
    pub live_training_progress: Option<TrainingProgressTelemetry>,
    pub active_director_scenario: u8,
    pub cyber_flags: CyberAttackFlags,
    #[serde(default)]
    pub flightgear_live: bool,
    #[serde(default)]
    pub flightgear_near_lrcv: bool,
    #[serde(default)]
    pub fg_autoland_assist: bool,
    #[serde(default = "default_true_bool")]
    pub shield_protection_enabled: bool,
    #[serde(default)]
    pub checkpoint_locked: bool,
    #[serde(default)]
    pub verified_checkpoint_ned: [f64; 3],
}

#[inline]
const fn default_true_bool() -> bool {
    true
}

/// Flag-urile active din Sandbox-ul Red-Team Cyber-Warfare (Art. 21).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CyberAttackFlags {
    pub gps_jump_spoof: bool,
    pub gps_stealth_ramp_spoof: bool,
    pub cut_internet_comms: bool,
    pub poison_meteo_cache: bool,
}

/// Telemetria în timp real a job-ului `ShadowTrainer` lansat prin butonul `🔥 LIVE CUDA FINE-TUNING (15s)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingProgressTelemetry {
    pub is_running: bool,
    pub current_epoch: usize,
    pub total_epochs: usize,
    pub progress_pct: f64,
    pub current_loss: f64,
    pub koschmieder_phys_residual: f64,
    pub elapsed_sec: f64,
}

/// Vector 2D de flux optic afișat peste camera SWIR/HUD (verde = sol, mov = ceață/rafală).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpticalVectorHud {
    pub x_norm: f64,
    pub y_norm: f64,
    pub dx_px: f64,
    pub dy_px: f64,
    pub layer: &'static str,
}

/// Snapshot meteo partajat *lock-free* prin `ArcSwap` între poller-ul async Open-Meteo `LRCV` și bucla de 60 Hz.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveMeteoSnapshot {
    pub temperature_c: f64,
    pub relative_humidity_pct: f64,
    pub qnh_hpa: f64,
    pub wind_speed_mps: f64,
    pub wind_dir_deg: f64,
    pub is_live_online: bool,
}

impl Default for LiveMeteoSnapshot {
    fn default() -> Self {
        Self {
            temperature_c: 8.5,
            relative_humidity_pct: 98.0,
            qnh_hpa: 1016.2,
            wind_speed_mps: 4.5,
            wind_dir_deg: 175.0,
            is_live_online: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use arc_swap::ArcSwap;
    use crate::cli_train::ActiveAiModel;
    use crate::rt_loop_60hz::{compute_inverse_depth_ground, CameraFrameSwir, FlightSimulator60Hz, FramedVec3};

    fn make_test_sim() -> FlightSimulator60Hz {
        FlightSimulator60Hz::new(
            Arc::new(ArcSwap::from_pointee(ActiveAiModel::default())),
            Arc::new(ArcSwap::from_pointee(None)),
            Arc::new(ArcSwap::from_pointee(LiveMeteoSnapshot::default())),
            Arc::new(AtomicBool::new(false)),
        )
    }

    #[test]
    fn test_sanitize_f64_nan_and_infinity_guards() {
        assert_eq!(sanitize_f64(f64::NAN, -10.0, 10.0, 2.5), 2.5);
        assert_eq!(sanitize_f64(f64::INFINITY, -10.0, 10.0, 2.5), 2.5);
        assert_eq!(sanitize_f64(f64::NEG_INFINITY, -10.0, 10.0, -1.0), -1.0);
        assert_eq!(sanitize_f64(999.0, -45.0, 45.0, 0.0), 45.0);
        assert_eq!(sanitize_f64(-999.0, -45.0, 45.0, 0.0), -45.0);
    }

    #[test]
    fn test_6dof_maneuver_commands_and_rotation_matrix_update() {
        let mut sim = make_test_sim();
        let frame0 = sim.step_60hz(1.0 / 60.0);
        let initial_rot = frame0.aircraft_pose.rot_matrix_b_to_n;

        // Aplicăm viraj dreapta (+18 deg), urcare (+6 deg), roll (+22 deg) și accelerare (+10 m/s)
        sim.apply_command(SimCommand::ManeuverStep {
            d_yaw_deg: 18.0,
            d_pitch_deg: 6.0,
            d_roll_deg: 22.0,
            d_throttle_mps: 10.0,
        });

        let frame1 = sim.step_60hz(1.0 / 60.0);
        assert!(frame1.aircraft_pose.vel_body_mps[0] > frame0.aircraft_pose.vel_body_mps[0] + 8.0);
        assert_ne!(frame1.aircraft_pose.rot_matrix_b_to_n, initial_rot);

        // Verificăm că norma cuaternionului rămâne exact 1.0 și inversa adâncimii la orizont nu este NaN
        let q = frame1.aircraft_pose.quat_b_to_n;
        let q_norm = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
        assert!((q_norm - 1.0).abs() < 1e-6);

        let ray_up = FramedVec3::<CameraFrameSwir>::new(0.8, 0.0, -0.6);
        let rho_sky = compute_inverse_depth_ground(ray_up, &sim.quat_b_to_n, 45.0);
        assert_eq!(rho_sky, 0.0);
    }

    #[test]
    fn test_inject_sudden_fog_gust_lookahead_and_decoupling() {
        let mut sim = make_test_sim();
        let _ = sim.step_60hz(1.0 / 60.0);

        // Injectăm o rafală laterală bruscă de 20 m/s
        sim.apply_command(SimCommand::InjectSuddenFogGust {
            wx_ned_mps: -4.0,
            wy_ned_mps: 20.0,
            wz_ned_mps: 0.0,
            turbulence_intensity: 1.0,
        });

        // Imediat după comandă, rafala se află în zona de lookahead (0.85s înainte de impactul fizic!)
        let frame_lookahead = sim.step_60hz(1.0 / 60.0);
        assert!(frame_lookahead.fog_and_meteo.gust_lookahead_warning);
        assert!((frame_lookahead.fog_and_meteo.gust_lookahead_lead_sec - 0.85).abs() < 1e-6);

        // Rulăm 4 secunde (240 cadre la 60 Hz) după impactul rafalei
        let mut last_frame = frame_lookahead;
        for _ in 0..240 {
            last_frame = sim.step_60hz(1.0 / 60.0);
        }

        // Naive Optical Flow derapează puternic din cauza ceții, în timp ce AERO-SHIELD V2.0 rămâne < 0.8m!
        assert!(last_frame.estimators_arena.naive_flow_error_m > 25.0);
        assert!(last_frame.estimators_arena.aero_shield_error_m < 0.8);
    }

    #[test]
    fn test_director_scenarios_deterministic_reproducibility() {
        for scenario_id in 1..=3 {
            let mut sim_a = make_test_sim();
            let mut sim_b = make_test_sim();

            sim_a.apply_command(SimCommand::TriggerDirectorScenario { scenario_id });
            sim_b.apply_command(SimCommand::TriggerDirectorScenario { scenario_id });

            let mut final_a = sim_a.step_60hz(1.0 / 60.0);
            let mut final_b = sim_b.step_60hz(1.0 / 60.0);

            for _ in 0..200 {
                final_a = sim_a.step_60hz(1.0 / 60.0);
                final_b = sim_b.step_60hz(1.0 / 60.0);
            }

            // Reproductibilitate bit-cu-bit garantată de ChaCha8Rng(0x4C52_4356_2026)
            assert_eq!(
                final_a.aircraft_pose.pos_ned_m,
                final_b.aircraft_pose.pos_ned_m
            );
            assert_eq!(
                final_a.fog_and_meteo.true_fog_wind_ned_mps,
                final_b.fog_and_meteo.true_fog_wind_ned_mps
            );
            // Eroarea AERO-SHIELD V2.0 sub 1.8m în toate cele 3 scenarii!
            assert!(final_a.estimators_arena.aero_shield_error_m < 1.8);
        }
    }

    #[test]
    fn test_cyber_watchdog_jump_and_stealth_cusum_and_poisoned_meteo() {
        let mut sim = make_test_sim();
        sim.apply_command(SimCommand::ConfigureCyberAttack {
            gps_jump_spoof: false,
            gps_stealth_ramp_spoof: true,
            cut_internet_comms: true,
            poison_meteo_cache: true,
        });

        // Rulăm 2.6 secunde (156 cadre la 60 Hz) -> CUSUM prinde atacul Stealth în < 2.9s!
        let mut frame = sim.step_60hz(1.0 / 60.0);
        for _ in 0..156 {
            frame = sim.step_60hz(1.0 / 60.0);
        }

        assert!(frame.cyber_and_gpu.cusum_stealth_stat >= 9.5);
        assert!(frame.cyber_and_gpu.gnss_watchdog_status.contains("QUARANTINED"));
        assert!(!frame.cyber_and_gpu.meteo_sanity_trusted);
        assert!(frame.fog_and_meteo.meteo_source_status.contains("POISONED"));

        // Verificăm și serializarea JSON completă a TelemetryFrameV2
        let json_res = serde_json::to_string(&frame);
        assert!(json_res.is_ok());
    }

    #[test]
    fn test_replay_real_video_command_and_benchmark() {
        let raw_json = serde_json::json!({ "type": "replay_real_video" });
        let parsed = SimCommand::parse_flexible_json(&raw_json);
        assert_eq!(parsed, Some(SimCommand::ReplayRealVideo));

        let mut sim = make_test_sim();
        sim.apply_command(SimCommand::ReplayRealVideo);
        let frame = sim.step_60hz(1.0 / 60.0);
        assert!(frame
            .fog_and_meteo
            .meteo_source_status
            .contains("REAL AVIATION FOG VIDEO"));
        assert!(frame.approach_phase.contains("REAL FOG VIDEO REPLAY"));
    }

    #[test]
    fn test_auto_loop_at_runway_end_prevents_freeze_and_preserves_state() {
        let mut sim = make_test_sim();
        sim.apply_command(SimCommand::ConfigureCyberAttack {
            gps_jump_spoof: true,
            gps_stealth_ramp_spoof: false,
            cut_internet_comms: false,
            poison_meteo_cache: false,
        });

        let mut reached_full_stop = false;
        let mut looped_back_to_approach = false;

        // 3500 cadre la 60 Hz (~58s): parcurge apropierea de la -1600m + Flare + Autobrake la 0 km/h + 6s dwell + auto-loop
        for _ in 0..3500 {
            let f = sim.step_60hz(1.0 / 60.0);
            let x = f.aircraft_pose.pos_ned_m[0];
            if f.lrcv_guidance.landing_complete && x >= 250.0 {
                reached_full_stop = true;
                assert_eq!(f.aircraft_pose.vel_body_mps[0], 0.0);
            }
            if reached_full_stop && x < -1400.0 {
                looped_back_to_approach = true;
                // Atacul GPS manual rămâne activ și după auto-loop!
                assert!(f.cyber_and_gpu.cyber_flags.gps_jump_spoof);
                break;
            }
        }

        assert!(reached_full_stop);
        assert!(looped_back_to_approach);
    }

    #[test]
    fn test_manual_cyber_attack_stops_active_director_scenario() {
        let mut sim = make_test_sim();
        sim.apply_command(SimCommand::TriggerDirectorScenario { scenario_id: 3 });

        let f_dir = sim.step_60hz(1.0 / 60.0);
        assert_eq!(f_dir.cyber_and_gpu.active_director_scenario, 3);
        assert!(f_dir.cyber_and_gpu.cyber_flags.gps_stealth_ramp_spoof);

        // Utilizatorul apasă manual pe „Atac GPS Brusc" în timpul Scenariului 3:
        // Director Mode trebuie să se oprească și să nu mai suprascrie flag-urile manuale!
        sim.apply_command(SimCommand::ConfigureCyberAttack {
            gps_jump_spoof: true,
            gps_stealth_ramp_spoof: false,
            cut_internet_comms: false,
            poison_meteo_cache: false,
        });

        for _ in 0..10 {
            let f_manual = sim.step_60hz(1.0 / 60.0);
            assert_eq!(f_manual.cyber_and_gpu.active_director_scenario, 0);
            assert!(f_manual.cyber_and_gpu.cyber_flags.gps_jump_spoof);
            assert!(!f_manual.cyber_and_gpu.cyber_flags.gps_stealth_ramp_spoof);
        }
    }

    #[test]
    fn test_flight_director_converges_and_stays_locked_when_pilot_follows_circle() {
        let mut sim = make_test_sim();
        let f0 = sim.step_60hz(1.0 / 60.0);
        // La start, cercul verde este perfect centrat pe botul avionului
        assert!(f0.lrcv_guidance.glideslope_error_deg.abs() < 0.25);
        assert!(f0.lrcv_guidance.heading_correction_deg.abs() < 0.25);

        // Pilotul trage brusc de manșă în sus (+9 deg „spre lună") și virează dreapta (+11 deg)
        sim.apply_command(SimCommand::ManeuverStep {
            d_yaw_deg: 11.0,
            d_pitch_deg: 9.0,
            d_roll_deg: 0.0,
            d_throttle_mps: 0.0,
        });
        let f_disturbed = sim.step_60hz(1.0 / 60.0);
        // Cercul verde îi cere acum clar să coboare botul (< -7 deg) și să vireze stânga (< -9 deg)
        assert!(f_disturbed.lrcv_guidance.glideslope_error_deg < -7.0);
        assert!(f_disturbed.lrcv_guidance.heading_correction_deg < -9.0);

        // Pilotul urmează indicația cercului verde
        sim.apply_command(SimCommand::ManeuverStep {
            d_yaw_deg: f_disturbed.lrcv_guidance.heading_correction_deg,
            d_pitch_deg: f_disturbed.lrcv_guidance.glideslope_error_deg,
            d_roll_deg: 0.0,
            d_throttle_mps: 0.0,
        });

        // După aliniere și încă 3 secunde (180 cadre) de zbor liber, cercul verde RĂMÂNE FIX pe centru!
        let mut f_locked = sim.step_60hz(1.0 / 60.0);
        for _ in 0..180 {
            f_locked = sim.step_60hz(1.0 / 60.0);
        }
        assert!(f_locked.lrcv_guidance.glideslope_error_deg.abs() < 0.35);
        assert!(f_locked.lrcv_guidance.heading_correction_deg.abs() < 0.35);
    }

    #[test]
    fn test_landing_flare_reduces_sink_rate_before_touchdown() {
        let mut sim = make_test_sim();
        let f_high = sim.step_60hz(1.0 / 60.0);
        let initial_sink_mps = f_high.aircraft_pose.vel_ground_ned_mps[2];
        assert!(initial_sink_mps > 3.2);
        assert!(!f_high.lrcv_guidance.flare_active);

        let mut saw_flare_zone = false;
        let mut min_flare_sink_before_touchdown = 99.0_f64;

        for _ in 0..2100 {
            let f = sim.step_60hz(1.0 / 60.0);
            let h_agl = -f.aircraft_pose.pos_ned_m[2];
            if f.lrcv_guidance.flare_active {
                saw_flare_zone = true;
            }
            if (0.5..=3.5).contains(&h_agl) {
                min_flare_sink_before_touchdown =
                    min_flare_sink_before_touchdown.min(f.aircraft_pose.vel_ground_ned_mps[2]);
            }
            if h_agl <= 0.1 {
                break;
            }
        }

        assert!(saw_flare_zone);
        // Rata de coborâre înainte de contact scade de la ~3.56 m/s sub 1.25 m/s datorită Flare-ului!
        assert!(min_flare_sink_before_touchdown < 1.25);
    }

    #[test]
    fn test_full_landing_rollout_autobrake_to_zero_speed_and_manual_gear_x() {
        let raw_json = serde_json::json!({
            "type": "ConfigureLandingSystems",
            "payload": { "gear_down": true, "autobrake_armed": true }
        });
        let parsed = SimCommand::parse_flexible_json(&raw_json);
        assert_eq!(
            parsed,
            Some(SimCommand::ConfigureLandingSystems {
                gear_down: Some(true),
                autobrake_armed: Some(true),
            })
        );

        let mut sim = make_test_sim();
        let f0 = sim.step_60hz(1.0 / 60.0);
        assert!(!f0.lrcv_guidance.gear_down);
        assert!(!f0.lrcv_guidance.autobrake_armed);

        // Pilotul apasă tasta [X] (armează Trenul de Aterizare + Autobrake)
        sim.apply_command(SimCommand::ConfigureLandingSystems {
            gear_down: Some(true),
            autobrake_armed: Some(true),
        });

        let mut final_frame = sim.step_60hz(1.0 / 60.0);
        assert!(final_frame.lrcv_guidance.gear_down);
        assert!(final_frame.lrcv_guidance.autobrake_armed);

        for _ in 0..3000 {
            final_frame = sim.step_60hz(1.0 / 60.0);
            if final_frame.lrcv_guidance.landing_complete {
                break;
            }
        }

        assert!(final_frame.lrcv_guidance.landing_complete);
        assert!(final_frame.lrcv_guidance.spoilers_deployed);
        assert!(!final_frame.lrcv_guidance.reverse_thrust_active);
        assert_eq!(final_frame.aircraft_pose.vel_body_mps[0], 0.0);
        assert!(final_frame.aircraft_pose.vel_ground_ned_mps[0].abs() < 1e-6);
        assert!(final_frame.lrcv_guidance.touchdown_vs_mps < 1.35);

        let stopped_x = final_frame.aircraft_pose.pos_ned_m[0];
        assert!(stopped_x > 250.0 && stopped_x < 1200.0);

        // Fără taxi și fără auto-loop nedorit după ce pilotul a interacționat manual (încă 10 secunde / 600 cadre)
        for _ in 0..600 {
            final_frame = sim.step_60hz(1.0 / 60.0);
        }
        assert!(final_frame.lrcv_guidance.landing_complete);
        assert!(!final_frame.lrcv_guidance.crashed);
        assert_eq!(final_frame.aircraft_pose.vel_body_mps[0], 0.0);
        assert!((final_frame.aircraft_pose.pos_ned_m[0] - stopped_x).abs() < 1e-6);
    }

    #[test]
    fn test_crosswind_physically_drifts_aircraft_and_requires_crab_angle() {
        let mut sim = make_test_sim();
        sim.apply_command(SimCommand::SetCrosswind {
            wx_ned_mps: -3.0,
            wy_ned_mps: 18.0,
            turbulence_intensity: 0.35,
        });

        let mut frame = sim.step_60hz(1.0 / 60.0);
        // Imediat, reticulul Flight Director cere unghi de Crab stânga (< -10 deg) împotriva vântului!
        assert!(frame.lrcv_guidance.heading_correction_deg < -10.0);
        assert!(frame.aircraft_pose.crab_angle_cmd_deg < -10.0);

        for _ in 0..180 {
            frame = sim.step_60hz(1.0 / 60.0);
        }

        // În 3 secunde fără corecție din partea pilotului, vântul lateral de 18 m/s împinge fizic avionul > 45m spre dreapta!
        assert!(frame.aircraft_pose.pos_ned_m[1] > 45.0);
    }

    #[test]
    fn test_crash_system_detects_hard_impact_and_off_runway_cfit() {
        let mut sim = make_test_sim();

        // 1) Picaj abrupt înainte de pistă (CFIT)
        sim.apply_command(SimCommand::ManeuverStep {
            d_yaw_deg: 0.0,
            d_pitch_deg: -18.0,
            d_roll_deg: 0.0,
            d_throttle_mps: 0.0,
        });

        let mut frame = sim.step_60hz(1.0 / 60.0);
        for _ in 0..600 {
            frame = sim.step_60hz(1.0 / 60.0);
            if frame.lrcv_guidance.crashed {
                break;
            }
        }
        assert!(frame.lrcv_guidance.crashed);
        assert!(frame.lrcv_guidance.crash_reason.contains("CFIT"));
        assert_eq!(frame.aircraft_pose.vel_body_mps[0], 0.0);

        // 2) Resetul curăță starea de Crash; apoi testăm aterizare pe burtă (Gear UP [G]) pe pistă
        sim.apply_command(SimCommand::ResetApproach);
        let reset_frame = sim.step_60hz(1.0 / 60.0);
        assert!(!reset_frame.lrcv_guidance.crashed);

        sim.apply_command(SimCommand::ConfigureLandingSystems {
            gear_down: Some(false),
            autobrake_armed: Some(true),
        });
        let mut belly_frame = sim.step_60hz(1.0 / 60.0);
        for _ in 0..2500 {
            belly_frame = sim.step_60hz(1.0 / 60.0);
            if belly_frame.lrcv_guidance.crashed {
                break;
            }
        }
        assert!(belly_frame.lrcv_guidance.crashed);
        assert!(belly_frame.lrcv_guidance.crash_reason.contains("BURTĂ"));
    }

    #[test]
    fn test_scenario_2_starts_unaligned_and_requires_pilot_maneuver() {
        let mut sim = make_test_sim();
        sim.apply_command(SimCommand::TriggerDirectorScenario { scenario_id: 2 });

        let f0 = sim.step_60hz(1.0 / 60.0);
        // Avionul pornește la ~195m în stânga axului pistei, în ceață densă (65m), dezaliniat:
        assert!(f0.aircraft_pose.pos_ned_m[1] < -190.0);
        assert!((f0.fog_and_meteo.visibility_mor_m - 65.0).abs() < 1e-6);
        // Punctul verde este mult mai mobil (> +20° la start) pentru a comanda un viraj S-Turn de interceptare a axului:
        assert!(f0.lrcv_guidance.heading_correction_deg > 20.0);
        assert_eq!(f0.cyber_and_gpu.active_director_scenario, 2);

        // Dacă pilotul NU manevrează deloc timp de 2 secunde, avionul NU se auto-aliniază singur:
        let mut f_no_input = f0;
        for _ in 0..120 {
            f_no_input = sim.step_60hz(1.0 / 60.0);
        }
        assert!(f_no_input.aircraft_pose.pos_ned_m[1] < -180.0);

        // Când pilotul urmărește activ punctul verde (Flight Director), legea S-Turn (Localizer Intercept + Lead Rollout)
        // aliniază aeronava pe axul pistei ÎNAINTE de prag și elimină aterizarea pe diagonală:
        let mut f_cur = f_no_input;
        for _ in 0..2200 {
            if f_cur.lrcv_guidance.spoilers_deployed || f_cur.lrcv_guidance.crashed {
                break;
            }
            let hdg_cmd = (f_cur.lrcv_guidance.heading_correction_deg * 0.12).clamp(-1.8, 1.8);
            let gs_cmd = (f_cur.lrcv_guidance.glideslope_error_deg * 0.12).clamp(-0.8, 0.8);
            sim.apply_command(SimCommand::ManeuverStep {
                d_yaw_deg: hdg_cmd,
                d_pitch_deg: gs_cmd,
                d_roll_deg: hdg_cmd * 0.8,
                d_throttle_mps: 0.0,
            });
            f_cur = sim.step_60hz(1.0 / 60.0);
        }
        assert_eq!(f_cur.cyber_and_gpu.active_director_scenario, 2);
        assert!(f_cur.lrcv_guidance.spoilers_deployed);
        assert!(!f_cur.lrcv_guidance.crashed);
        // La touchdown, aeronava este pe centrul pistei (|y| < 6m) și aliniată drept (fără aterizare diagonală!):
        assert!(f_cur.aircraft_pose.pos_ned_m[1].abs() < 6.0);
        let vx = f_cur.aircraft_pose.vel_ground_ned_mps[0];
        let vy = f_cur.aircraft_pose.vel_ground_ned_mps[1];
        let touchdown_track_deg = vy.atan2(vx.max(1.0)).to_degrees();
        assert!(touchdown_track_deg.abs() < 3.5);
    }

    #[test]
    fn test_shield_disabled_causes_closed_loop_spoof_deviation() {
        let mut sim_shielded = make_test_sim();
        let mut sim_unshielded = make_test_sim();

        sim_shielded.apply_command(SimCommand::ConfigureCyberAttack {
            gps_jump_spoof: true,
            gps_stealth_ramp_spoof: false,
            cut_internet_comms: false,
            poison_meteo_cache: false,
        });
        sim_unshielded.apply_command(SimCommand::SetShieldProtection { enabled: false });
        sim_unshielded.apply_command(SimCommand::ConfigureCyberAttack {
            gps_jump_spoof: true,
            gps_stealth_ramp_spoof: false,
            cut_internet_comms: false,
            poison_meteo_cache: false,
        });

        let mut f_shield = sim_shielded.step_60hz(1.0 / 60.0);
        let mut f_unshield = sim_unshielded.step_60hz(1.0 / 60.0);
        for _ in 0..360 {
            f_shield = sim_shielded.step_60hz(1.0 / 60.0);
            f_unshield = sim_unshielded.step_60hz(1.0 / 60.0);
            if f_unshield.lrcv_guidance.crashed {
                break;
            }
        }

        // Cu Scutul AERO-SHIELD ACTIV: avionul rămâne perfect pe axul pistei (|y| < 1.0m) și carantinează GPS-ul!
        assert!(f_shield.cyber_and_gpu.shield_protection_enabled);
        assert!(f_shield.aircraft_pose.pos_ned_m[1].abs() < 1.0);
        assert!(!f_shield.lrcv_guidance.crashed);

        // Cu Scutul OPRIT (Avion Clasic): autopilotul urmărește GPS-ul falsificat și deviază sever în afara axului!
        assert!(!f_unshield.cyber_and_gpu.shield_protection_enabled);
        assert!(
            f_unshield.aircraft_pose.pos_ned_m[1].abs() > 35.0 || f_unshield.lrcv_guidance.crashed
        );
    }
}
