//! # `aero-cockpit-server` — AERO-SHIELD LRCV V2.0
//!
//! Bucla Real-Time $60\text{ Hz}$ pe Thread OS Dedicat (`aero-60hz-rt-loop`), Serverul WebSocket `axum`
//! cu *Zero-Lag State Conflation* (`tokio::sync::watch`), Arena celor 4 Estimatoare,
//! Antrenarea GPU CUDA (`ShadowTrainer`) și Cele 3 Scenarii Deterministe „One-Click Director Mode"
//! pentru Aeroportul Internațional Craiova (`LRCV`) — Pista RWY 09 (`44.3181° N, 23.8886° E`).

pub mod cli_train;
pub mod director_scenarios;
pub mod flightgear_synthetic_10;
pub mod long_range_zero_gps_test;
pub mod rt_loop_60hz;
pub mod sim_commands;
pub mod ui_framework_rust;
pub mod ui_lessons_6yo;
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
    fn test_maud_rust_ui_framework_and_6yo_lessons() {
        let lessons = crate::ui_framework_rust::get_all_6_interactive_lessons();
        assert_eq!(lessons.len(), 6);

        let mut sim = make_test_sim();
        let init_frame = Arc::new(sim.step_60hz(1.0 / 60.0));
        let (cmd_tx, _cmd_rx) = crossbeam_channel::bounded::<SimCommand>(16);
        let (_tel_tx, tel_rx) = tokio::sync::watch::channel(init_frame);
        let state = crate::ws_telemetry::CockpitServerState {
            cmd_tx,
            telemetry_rx: tel_rx,
            live_meteo: Arc::new(ArcSwap::from_pointee(LiveMeteoSnapshot::default())),
        };

        let html_str = crate::ui_framework_rust::render_aero_shield_app_maud(&state).into_string();
        assert!(html_str.starts_with("<!DOCTYPE html>"));
        assert!(html_str.contains("RUST MAUD SSR"));
        assert!(html_str.contains("Lecția 1:"));
        assert!(html_str.contains("Lecția 6:"));
        assert!(html_str.contains("FG-01"));
        assert!(html_str.contains("FG-10"));

        for p in 1..=6 {
            let page_html =
                crate::ui_framework_rust::render_aero_shield_app_maud_page(&state, p).into_string();
            let active_div = format!("id=\"concept-page-{}\" class=\"concept-page-view active-page\"", p);
            assert!(page_html.contains(&active_div));
            assert!(page_html.contains(&format!("Pagina {} din 6:", p)));
        }
    }
}
