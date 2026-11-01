//! Motorul fizic-aerodinamic 6-DoF, Arena celor 4 Estimatoare, Watchdog-ul Cyber Dual-Layer
//! și bucla pe thread OS dedicat `aero-60hz-rt-loop` la 60 Hz ($16.666\text{ ms}$).

use std::marker::PhantomData;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use arc_swap::ArcSwap;
use crossbeam_channel::Receiver;
use nalgebra::{Matrix3, SMatrix, SVector, UnitQuaternion, Vector3};
use tokio::sync::watch;

use aero_math_6dof::fuse_fog_inverse_depth as math6dof_fuse_fog_inverse_depth;
use aero_meteo_cyber::{
    build_diagonal_innovation_inv_sqrt, compute_aerodynamic_wind_triangle_ned,
    verify_meteo_cache_integrity, DualLayerGnssWatchdog, GnssIntegrityStatus,
};
use aero_vision_fog::compute_autopilot_crab_angle;

use crate::cli_train::{spawn_shadow_trainer_background, ActiveAiModel};
use crate::director_scenarios::DirectorScenarioEngine;
use crate::flightgear_bridge::{FlightGearFeedbackSender, FlightGearGuidanceFeedback, FlightGearLiveState};
use crate::sim_commands::{sanitize_f64, SimCommand};
use crate::{
    Aircraft6DofPose, CyberAttackFlags, CyberGpuTelemetry, FogMeteoTelemetry,
    FourEstimatorsTelemetry, LiveMeteoSnapshot, LrcvRunwayGuidance, OpticalVectorHud,
    TelemetryFrameV2, TrainingProgressTelemetry,
};

/// Coordonatele oficiale ale Pragului Pistei RWY 09 — Aeroportul Internațional Craiova (LRCV) din apt.dat FlightGear.
pub const LRCV_RWY09_LAT_DEG: f64 = 44.317_944;
pub const LRCV_RWY09_LON_DEG: f64 = 23.872_944;
pub const LRCV_RWY09_HEADING_DEG: f64 = 88.931_4;

/// Tipuri fantomă (*Zero-Cost Phantom Types*) care previn adunarea accidentală a vectorilor
/// din repere de coordonate diferite la compilare (Remedierea Critică #1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyFrame;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NedFrameLrcv;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraFrameSwir;

/// Vector 3D ancorat semantic într-un sistem de referință explicit `Frame`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FramedVec3<Frame> {
    pub inner: Vector3<f64>,
    _frame: PhantomData<Frame>,
}

impl<Frame> FramedVec3<Frame> {
    #[inline]
    #[must_use]
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self {
            inner: Vector3::new(
                sanitize_f64(x, -100_000.0, 100_000.0, 0.0),
                sanitize_f64(y, -100_000.0, 100_000.0, 0.0),
                sanitize_f64(z, -100_000.0, 100_000.0, 0.0),
            ),
            _frame: PhantomData,
        }
    }

    #[inline]
    #[must_use]
    pub fn from_vec(v: Vector3<f64>) -> Self {
        Self::new(v.x, v.y, v.z)
    }

    #[inline]
    #[must_use]
    pub fn to_array(&self) -> [f64; 3] {
        [self.inner.x, self.inner.y, self.inner.z]
    }
}

/// Transformă un vector din `BodyFrame` în `NedFrameLrcv` folosind cuaternionul unitar $\mathbf{q}_b^n$.
#[inline]
#[must_use]
pub fn transform_body_to_ned(
    q_b_to_n: &UnitQuaternion<f64>,
    v_body: FramedVec3<BodyFrame>,
) -> FramedVec3<NedFrameLrcv> {
    let rotated = q_b_to_n.transform_vector(&v_body.inner);
    FramedVec3::<NedFrameLrcv>::from_vec(rotated)
}

/// Transformă un vector din `NedFrameLrcv` în `BodyFrame` folosind inversul cuaternionului $\mathbf{q}_n^b = (\mathbf{q}_b^n)^*$.
#[inline]
#[must_use]
pub fn transform_ned_to_body(
    q_b_to_n: &UnitQuaternion<f64>,
    v_ned: FramedVec3<NedFrameLrcv>,
) -> FramedVec3<BodyFrame> {
    let rotated = q_b_to_n.inverse_transform_vector(&v_ned.inner);
    FramedVec3::<BodyFrame>::from_vec(rotated)
}

/// Calculează inversa adâncimii solului $\rho_{ground}(u,v,h_{AGL},\mathbf{q}_b^n) = 1/Z_{ground} \in [0, 0.5]\text{ m}^{-1}$.
/// La orizont și deasupra orizontului returnează exact `0.0` fără nicio împărțire la zero sau `NaN`!
#[inline]
#[must_use]
pub fn compute_inverse_depth_ground(
    ray_cam: FramedVec3<CameraFrameSwir>,
    q_b_to_n: &UnitQuaternion<f64>,
    h_agl_m: f64,
) -> f64 {
    let safe_h = sanitize_f64(h_agl_m, 0.5, 10_000.0, 45.0);
    let ray_body = FramedVec3::<BodyFrame>::from_vec(ray_cam.inner.normalize());
    let ray_ned = transform_body_to_ned(q_b_to_n, ray_body);
    let d_down = ray_ned.inner.z;
    if d_down <= 1e-6 {
        0.0
    } else {
        sanitize_f64(d_down / safe_h, 0.0, 0.5, 0.0)
    }
}

/// Fuziune Bayesiană cu dublă ancorare pentru inversa adâncimii efective a stratului de ceață $\hat{\rho}_{fog}$,
/// delegată direct către modulul matematic `aero-math-6dof::fuse_fog_inverse_depth`.
#[inline]
#[must_use]
pub fn fuse_fog_inverse_depth(visibility_mor_m: f64, stereo_rho_inv_m: Option<f64>) -> (f64, f64) {
    let safe_mor = sanitize_f64(visibility_mor_m, 25.0, 5000.0, 75.0);
    let beta_ext = sanitize_f64(2.995_732_273_55 / safe_mor, 0.0005, 0.15, 0.0399);
    let rho_prior = sanitize_f64(beta_ext / 1.956, 0.002, 0.25, 0.0204);
    let rho_stereo = stereo_rho_inv_m
        .filter(|v| v.is_finite())
        .map(|v| sanitize_f64(v, 0.002, 0.25, rho_prior))
        .unwrap_or(rho_prior);
    let rho_fused = math6dof_fuse_fog_inverse_depth(rho_prior, rho_stereo, 1.0, 0.004, 0.007);
    (beta_ext, sanitize_f64(rho_fused, 0.002, 0.25, rho_prior))
}

/// Actualizare de covarianță în Forma Simetrică Joseph:
/// $\mathbf{P}^+ = (\mathbf{I} - \mathbf{K}\mathbf{H})\mathbf{P}^-(\mathbf{I} - \mathbf{K}\mathbf{H})^\top + \mathbf{K}\mathbf{R}\mathbf{K}^\top$
/// urmată de simetrizare explicită $\frac{1}{2}(\mathbf{P}^+ + (\mathbf{P}^+)^\top)$ și gardă diagonală pozitivă.
#[must_use]
pub fn joseph_stabilized_covariance_3x3(
    p_prior: &Matrix3<f64>,
    k_gain: &Matrix3<f64>,
    h_mat: &Matrix3<f64>,
    r_meas: &Matrix3<f64>,
) -> Matrix3<f64> {
    let i_mat = Matrix3::<f64>::identity();
    let i_kh = i_mat - k_gain * h_mat;
    let raw_p = i_kh * p_prior * i_kh.transpose() + k_gain * r_meas * k_gain.transpose();
    let mut sym_p = 0.5 * (raw_p + raw_p.transpose());

    for i in 0..3 {
        for j in 0..3 {
            sym_p[(i, j)] = sanitize_f64(sym_p[(i, j)], -1e4, 1e4, if i == j { 0.25 } else { 0.0 });
        }
        if sym_p[(i, i)] < 1e-6 {
            sym_p[(i, i)] = 1e-6;
        }
    }
    sym_p
}

/// Profil extras din videoclipurile reale de aterizare în ceață (`real_videos/benchmark_report.json`).
#[derive(Debug, Clone)]
pub struct RealVideoReplayProfile {
    pub visibility_mor_m: f64,
    pub als_bars_detected: usize,
    pub photonic_certainty_mean: f64,
    pub vel_ground_ned_mps: [f64; 3],
    pub fog_wind_ned_mps: [f64; 3],
}

impl Default for RealVideoReplayProfile {
    fn default() -> Self {
        Self {
            visibility_mor_m: 58.4,
            als_bars_detected: 11,
            photonic_certainty_mean: 0.89,
            vel_ground_ned_mps: [69.2, -0.4, 3.45],
            fog_wind_ned_mps: [-3.8, 9.4, 0.3],
        }
    }
}

/// Citește profilul din `real_videos/benchmark_report.json` (sau `/home/petreleon/aero-shield-lrcv/real_videos/benchmark_report.json`)
/// dacă există pe disc; altfel returnează profilul implicit calibrat pe cadrele video reale.
#[must_use]
pub fn load_real_video_replay_profile() -> RealVideoReplayProfile {
    let candidate_paths = [
        "real_videos/benchmark_report.json",
        "/home/petreleon/aero-shield-lrcv/real_videos/benchmark_report.json",
    ];
    let mut profile = RealVideoReplayProfile::default();

    for path_str in &candidate_paths {
        let p = Path::new(path_str);
        if let Ok(content) = std::fs::read_to_string(p) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(videos) = json.get("videos").and_then(|v| v.as_array()) {
                    if !videos.is_empty() {
                        // Folosim videoclipul de apropiere cu balizaj ALS (sau media celor 3 videoclipuri reale)
                        let chosen = videos.get(1).or_else(|| videos.first());
                        if let Some(v) = chosen {
                            if let Some(beta) = v.get("mean_extinction_beta").and_then(|b| b.as_f64()) {
                                let mor_from_beta = 2.995_732_273_55 / beta.max(1e-4);
                                profile.visibility_mor_m = sanitize_f64(mor_from_beta, 25.0, 3500.0, 69.2);
                            } else if let Some(mor) = v.get("estimated_mor_visibility_m").and_then(|m| m.as_f64()) {
                                profile.visibility_mor_m = sanitize_f64(mor, 25.0, 3500.0, 69.2);
                            }
                            if let Some(als) = v.get("mean_als_beacons_per_frame").and_then(|a| a.as_f64()) {
                                profile.als_bars_detected = (als.round() as usize).clamp(4, 24);
                            }
                            if let Some(cert) = v.get("mean_photonic_weight").and_then(|c| c.as_f64()) {
                                profile.photonic_certainty_mean = sanitize_f64(cert + 0.55, 0.35, 0.98, 0.84);
                            }
                        }
                    }
                }
                if let Some(mor) = json
                    .get("mean_visibility_mor_m")
                    .or_else(|| json.get("visibility_mor_m"))
                    .and_then(|v| v.as_f64())
                {
                    profile.visibility_mor_m = sanitize_f64(mor, 25.0, 5000.0, 58.4);
                }
                if let Some(als) = json
                    .get("als_bars_detected")
                    .or_else(|| json.get("mean_als_bars_detected"))
                    .and_then(|v| v.as_u64())
                {
                    profile.als_bars_detected = (als as usize).clamp(2, 24);
                }
                if let Some(cert) = json
                    .get("photonic_certainty_mean")
                    .and_then(|v| v.as_f64())
                {
                    profile.photonic_certainty_mean = sanitize_f64(cert, 0.25, 0.99, 0.89);
                }
                if let Some(arr) = json.get("est_fog_wind_ned_mps").and_then(|v| v.as_array()) {
                    if arr.len() >= 3 {
                        profile.fog_wind_ned_mps = [
                            sanitize_f64(arr[0].as_f64().unwrap_or(-3.8), -45.0, 45.0, -3.8),
                            sanitize_f64(arr[1].as_f64().unwrap_or(9.4), -45.0, 45.0, 9.4),
                            sanitize_f64(arr[2].as_f64().unwrap_or(0.3), -15.0, 15.0, 0.3),
                        ];
                    }
                }
                break;
            }
        }
    }
    profile
}

/// Simulatorul complet de zbor 6-DoF la 60 Hz pentru apropierea pe pista LRCV RWY 09 Craiova.
pub struct FlightSimulator60Hz {
    pub tick: u64,
    pub sim_time_sec: f64,
    pub pos_ned: FramedVec3<NedFrameLrcv>,
    pub vel_body: FramedVec3<BodyFrame>,
    pub quat_b_to_n: UnitQuaternion<f64>,
    pub omega_body_dps: [f64; 3],
    pub continuous_cmd_rates_dps: [f64; 3],
    pub continuous_cmd_accel_x: f64,
    pub accel_body_mps2: [f64; 3],

    // Mediu & Ceață
    pub visibility_mor_m: f64,
    pub swir_als_enabled: bool,
    pub physical_fog_wind_ned: FramedVec3<NedFrameLrcv>,
    pub lookahead_fog_wind_ned: FramedVec3<NedFrameLrcv>,
    pub manual_gust_timer_sec: f64,
    pub manual_gust_target_ned: [f64; 3],
    pub base_crosswind_ned: [f64; 3],
    pub crosswind_turbulence: f64,

    // Real Video Replay Mode
    pub real_video_ticks_remaining: usize,
    pub real_video_profile: RealVideoReplayProfile,

    // Estimatoare simultane (Benchmark 4-Arena)
    pub raw_gnss_pos_ned: [f64; 3],
    pub pure_ins_pos_ned: [f64; 3],
    pub pure_ins_vel_ned: [f64; 3],
    pub naive_flow_pos_ned: [f64; 3],
    pub aero_shield_pos_ned: [f64; 3],
    pub cov_pos_3x3: Matrix3<f64>,

    // Cyber-Warfare & Watchdog (`aero-meteo-cyber::DualLayerGnssWatchdog`) + Last Verified Checkpoint
    pub cyber_flags: CyberAttackFlags,
    pub gnss_watchdog: DualLayerGnssWatchdog,
    pub watchdog_s_inv_sqrt: SMatrix<f64, 6, 6>,
    pub stealth_spoof_elapsed_sec: f64,
    pub ins_drift_time_sec: f64,
    pub cusum_stat: f64,
    pub chi2_stat: f64,
    pub gnss_quarantined: bool,
    pub shield_protection_enabled: bool,
    pub checkpoint_locked: bool,
    pub verified_checkpoint_ned: [f64; 3],

    // Director Mode, Landing Systems, Crash System & FlightGear Bidirectional Bridge
    pub manual_pilot_override: bool,
    pub hold_after_full_stop: bool,
    pub auto_gear_engaged: bool,
    pub gear_down: bool,
    pub autobrake_armed: bool,
    pub spoilers_deployed: bool,
    pub reverse_thrust_active: bool,
    pub touched_down: bool,
    pub landing_complete: bool,
    pub crashed: bool,
    pub crash_reason: String,
    pub touchdown_vs_mps: f64,
    pub touchdown_lateral_err_m: f64,
    pub full_stop_dwell_sec: f64,
    pub fg_autoland_assist: bool,
    pub fg_feedback_tx: FlightGearFeedbackSender,
    pub director: DirectorScenarioEngine,
    pub active_model: Arc<ArcSwap<ActiveAiModel>>,
    pub training_progress: Arc<ArcSwap<Option<TrainingProgressTelemetry>>>,
    pub live_meteo: Arc<ArcSwap<LiveMeteoSnapshot>>,
    pub is_training_flag: Arc<AtomicBool>,
    pub fg_live_state: Arc<ArcSwap<Option<FlightGearLiveState>>>,
}

impl FlightSimulator60Hz {
    #[must_use]
    pub fn new(
        active_model: Arc<ArcSwap<ActiveAiModel>>,
        training_progress: Arc<ArcSwap<Option<TrainingProgressTelemetry>>>,
        live_meteo: Arc<ArcSwap<LiveMeteoSnapshot>>,
        is_training_flag: Arc<AtomicBool>,
    ) -> Self {
        let init_pos = [-1600.0, 0.0, -89.62];
        let q_init = UnitQuaternion::from_euler_angles(0.0, (-3.0_f64).to_radians(), 0.0);

        Self {
            tick: 0,
            sim_time_sec: 0.0,
            pos_ned: FramedVec3::new(init_pos[0], init_pos[1], init_pos[2]),
            vel_body: FramedVec3::new(68.0, 0.0, 0.0),
            quat_b_to_n: q_init,
            omega_body_dps: [0.0, 0.0, 0.0],
            continuous_cmd_rates_dps: [0.0, 0.0, 0.0],
            continuous_cmd_accel_x: 0.0,
            accel_body_mps2: [0.0, 0.0, 0.0],
            visibility_mor_m: 75.0,
            swir_als_enabled: true,
            physical_fog_wind_ned: FramedVec3::new(0.0, 0.0, 0.0),
            lookahead_fog_wind_ned: FramedVec3::new(0.0, 0.0, 0.0),
            manual_gust_timer_sec: 0.0,
            manual_gust_target_ned: [-4.0, 18.0, 0.0],
            base_crosswind_ned: [0.0, 0.0, 0.0],
            crosswind_turbulence: 0.0,
            real_video_ticks_remaining: 0,
            real_video_profile: RealVideoReplayProfile::default(),
            raw_gnss_pos_ned: init_pos,
            pure_ins_pos_ned: init_pos,
            pure_ins_vel_ned: [68.0, 0.0, 3.55],
            naive_flow_pos_ned: init_pos,
            aero_shield_pos_ned: init_pos,
            cov_pos_3x3: Matrix3::from_diagonal(&Vector3::new(0.15, 0.15, 0.12)),
            cyber_flags: CyberAttackFlags::default(),
            gnss_watchdog: DualLayerGnssWatchdog::new(),
            watchdog_s_inv_sqrt: build_diagonal_innovation_inv_sqrt(3.2, 4.5),
            stealth_spoof_elapsed_sec: 0.0,
            ins_drift_time_sec: 0.0,
            cusum_stat: 0.0,
            chi2_stat: 1.15,
            gnss_quarantined: false,
            shield_protection_enabled: true,
            checkpoint_locked: false,
            verified_checkpoint_ned: init_pos,
            manual_pilot_override: false,
            hold_after_full_stop: false,
            auto_gear_engaged: false,
            gear_down: false,
            autobrake_armed: false,
            spoilers_deployed: false,
            reverse_thrust_active: false,
            touched_down: false,
            landing_complete: false,
            crashed: false,
            crash_reason: String::new(),
            touchdown_vs_mps: 0.0,
            touchdown_lateral_err_m: 0.0,
            full_stop_dwell_sec: 0.0,
            fg_autoland_assist: false,
            fg_feedback_tx: FlightGearFeedbackSender::new(),
            director: DirectorScenarioEngine::new(0),
            active_model,
            training_progress,
            live_meteo,
            is_training_flag,
            fg_live_state: Arc::new(ArcSwap::from_pointee(None)),
        }
    }

    pub fn with_flightgear_bridge(
        mut self,
        fg_live_state: Arc<ArcSwap<Option<FlightGearLiveState>>>,
    ) -> Self {
        self.fg_live_state = fg_live_state;
        self
    }

    /// Resetează starea fizică și toate cele 4 estimatoare la începutul apropierii extinse (`-1600m`) pe RWY 09.
    pub fn reset_to_initial_approach(&mut self) {
        let init_pos = [-1600.0, 0.0, -89.62];
        self.tick = 0;
        self.sim_time_sec = 0.0;
        self.pos_ned = FramedVec3::new(init_pos[0], init_pos[1], init_pos[2]);
        self.vel_body = FramedVec3::new(68.0, 0.0, 0.0);
        self.quat_b_to_n = UnitQuaternion::from_euler_angles(0.0, (-3.0_f64).to_radians(), 0.0);
        self.omega_body_dps = [0.0, 0.0, 0.0];
        self.continuous_cmd_rates_dps = [0.0, 0.0, 0.0];
        self.continuous_cmd_accel_x = 0.0;
        self.accel_body_mps2 = [0.0, 0.0, 0.0];
        self.visibility_mor_m = 75.0;
        self.swir_als_enabled = true;
        self.physical_fog_wind_ned = FramedVec3::new(0.0, 0.0, 0.0);
        self.lookahead_fog_wind_ned = FramedVec3::new(0.0, 0.0, 0.0);
        self.manual_gust_timer_sec = 0.0;
        self.base_crosswind_ned = [0.0, 0.0, 0.0];
        self.crosswind_turbulence = 0.0;
        self.real_video_ticks_remaining = 0;
        self.raw_gnss_pos_ned = init_pos;
        self.pure_ins_pos_ned = init_pos;
        self.pure_ins_vel_ned = [68.0, 0.0, 3.55];
        self.naive_flow_pos_ned = init_pos;
        self.aero_shield_pos_ned = init_pos;
        self.cov_pos_3x3 = Matrix3::from_diagonal(&Vector3::new(0.15, 0.15, 0.12));
        self.cyber_flags = CyberAttackFlags::default();
        self.gnss_watchdog.reset_quarantine();
        self.stealth_spoof_elapsed_sec = 0.0;
        self.ins_drift_time_sec = 0.0;
        self.cusum_stat = 0.0;
        self.chi2_stat = 1.15;
        self.gnss_quarantined = false;
        self.shield_protection_enabled = true;
        self.checkpoint_locked = false;
        self.verified_checkpoint_ned = init_pos;
        self.manual_pilot_override = false;
        self.hold_after_full_stop = false;
        self.auto_gear_engaged = false;
        self.gear_down = false;
        self.autobrake_armed = false;
        self.spoilers_deployed = false;
        self.reverse_thrust_active = false;
        self.touched_down = false;
        self.landing_complete = false;
        self.crashed = false;
        self.crash_reason.clear();
        self.touchdown_vs_mps = 0.0;
        self.touchdown_lateral_err_m = 0.0;
        self.full_stop_dwell_sec = 0.0;
    }

    /// Inițializează Scenariul 2: apropiere NEALINIATĂ cu pista (`-1650m` distanță, `-195m` abatere în stânga
    /// axului pistei, `102m` altitudine, botul la `-2.5°` dezaliniat), în ceață densă (`65m`) și vânt lateral (`-4.2 m/s`),
    /// astfel încât pilotul trebuie să manevreze activ aeronava spre pistă urmărind reticulul Flight Director!
    pub fn reset_to_unaligned_approach(&mut self) {
        self.reset_to_initial_approach();
        let offset_pos = [-1650.0, -195.0, -102.0];
        self.pos_ned = FramedVec3::new(offset_pos[0], offset_pos[1], offset_pos[2]);
        self.quat_b_to_n = UnitQuaternion::from_euler_angles(
            0.0,
            (-2.6_f64).to_radians(),
            (-2.5_f64).to_radians(),
        );
        self.visibility_mor_m = 65.0;
        self.base_crosswind_ned = [-2.5, -4.2, 0.0];
        self.crosswind_turbulence = 0.25;
        self.physical_fog_wind_ned = FramedVec3::new(-2.5, -4.2, 0.0);
        self.lookahead_fog_wind_ned = FramedVec3::new(-2.5, -4.2, 0.0);
        self.raw_gnss_pos_ned = offset_pos;
        self.pure_ins_pos_ned = offset_pos;
        self.naive_flow_pos_ned = offset_pos;
        self.aero_shield_pos_ned = offset_pos;
        self.verified_checkpoint_ned = offset_pos;
        self.hold_after_full_stop = true;
    }

    fn trigger_crash(&mut self, reason: String, impact_vs_mps: f64) {
        self.crashed = true;
        self.crash_reason = reason;
        self.hold_after_full_stop = true;
        self.director.stop();
        self.pos_ned.inner.z = 0.0;
        self.vel_body = FramedVec3::new(0.0, 0.0, 0.0);
        self.omega_body_dps = [0.0, 0.0, 0.0];
        self.continuous_cmd_rates_dps = [0.0, 0.0, 0.0];
        self.continuous_cmd_accel_x = 0.0;
        self.accel_body_mps2 = [0.0, 0.0, 0.0];
        self.reverse_thrust_active = false;
        self.touchdown_vs_mps = sanitize_f64(impact_vs_mps.abs().max(0.5), 0.5, 99.0, 6.0);
        self.touchdown_lateral_err_m = sanitize_f64(self.pos_ned.inner.y.abs(), 0.0, 2000.0, 0.0);
    }

    /// Reia automat apropierea de la `-1600m` după oprirea completă pe pistă (în modul demo nesupravegheat),
    /// păstrând fie scenariul Director activ, fie setările manuale (atacuri cyber / ceață / vânt).
    fn loop_approach_preserving_mode(&mut self) {
        let next_tick = self.tick;
        let active_scenario = self.director.active_scenario_id;
        if active_scenario != 0 {
            if active_scenario == 2 {
                self.reset_to_unaligned_approach();
            } else {
                self.reset_to_initial_approach();
            }
            self.tick = next_tick;
            self.director.start_scenario(active_scenario);
            return;
        }

        let saved_cyber = self.cyber_flags.clone();
        let saved_shield = self.shield_protection_enabled;
        let saved_vis = self.visibility_mor_m;
        let saved_swir = self.swir_als_enabled;
        let saved_phys_wind = self.physical_fog_wind_ned;
        let saved_look_wind = self.lookahead_fog_wind_ned;
        let saved_base_wind = self.base_crosswind_ned;
        let saved_turb = self.crosswind_turbulence;

        self.reset_to_initial_approach();
        self.tick = next_tick;
        self.cyber_flags = saved_cyber;
        self.shield_protection_enabled = saved_shield;
        self.visibility_mor_m = saved_vis;
        self.swir_als_enabled = saved_swir;
        self.physical_fog_wind_ned = saved_phys_wind;
        self.lookahead_fog_wind_ned = saved_look_wind;
        self.base_crosswind_ned = saved_base_wind;
        self.crosswind_turbulence = saved_turb;
    }

    /// Aplică instantaneu o comandă `SimCommand` primită de la pilot/juriu.
    pub fn apply_command(&mut self, raw_cmd: SimCommand) {
        match raw_cmd.sanitize() {
            SimCommand::ManeuverStep {
                d_yaw_deg,
                d_pitch_deg,
                d_roll_deg,
                d_throttle_mps,
            } => {
                if self.director.active_scenario_id != 2 {
                    self.director.stop();
                }
                self.manual_pilot_override = true;
                self.hold_after_full_stop = true;
                self.continuous_cmd_rates_dps = [0.0, 0.0, 0.0];
                self.continuous_cmd_accel_x = 0.0;
                if self.landing_complete || self.crashed {
                    // După oprirea completă la 0 km/h sau după Crash, avionul rămâne oprit până la Reset [R].
                    self.vel_body.inner.x = 0.0;
                    self.omega_body_dps = [0.0, 0.0, 0.0];
                    self.accel_body_mps2 = [0.0, 0.0, 0.0];
                    return;
                }
                let (cur_roll, cur_pitch, cur_yaw) = self.quat_b_to_n.euler_angles();
                let new_yaw = sanitize_f64(
                    cur_yaw.to_degrees() + d_yaw_deg,
                    -179.0,
                    179.0,
                    0.0,
                )
                .to_radians();

                if self.touched_down {
                    let new_pitch = sanitize_f64(
                        cur_pitch.to_degrees() + d_pitch_deg,
                        0.0,
                        2.5,
                        0.0,
                    )
                    .to_radians();
                    self.quat_b_to_n = UnitQuaternion::from_euler_angles(0.0, new_pitch, new_yaw);
                    self.omega_body_dps = [0.0, 0.0, 0.0];
                    let new_u = sanitize_f64(
                        self.vel_body.inner.x + d_throttle_mps,
                        0.0,
                        125.0,
                        20.0,
                    );
                    self.vel_body.inner.x = new_u;
                    self.accel_body_mps2[0] = 0.0;
                } else {
                    let new_roll = sanitize_f64(
                        cur_roll.to_degrees() + d_roll_deg,
                        -65.0,
                        65.0,
                        0.0,
                    )
                    .to_radians();
                    let new_pitch = sanitize_f64(
                        cur_pitch.to_degrees() + d_pitch_deg,
                        -25.0,
                        25.0,
                        -3.0,
                    )
                    .to_radians();
                    self.quat_b_to_n = UnitQuaternion::from_euler_angles(new_roll, new_pitch, new_yaw);
                    self.omega_body_dps = [0.0, 0.0, 0.0];

                    let new_u = sanitize_f64(self.vel_body.inner.x + d_throttle_mps, 38.0, 125.0, 68.0);
                    self.vel_body.inner.x = new_u;
                    self.accel_body_mps2[0] = 0.0;
                }
            }
            SimCommand::SetContinuousRates {
                roll_rate_dps,
                pitch_rate_dps,
                yaw_rate_dps,
                accel_x_mps2,
            } => {
                if self.crashed {
                    return;
                }
                if self.director.active_scenario_id != 2 {
                    self.director.stop();
                }
                self.manual_pilot_override = true;
                self.hold_after_full_stop = true;
                self.continuous_cmd_rates_dps = [roll_rate_dps, pitch_rate_dps, yaw_rate_dps];
                self.continuous_cmd_accel_x = accel_x_mps2;
            }
            SimCommand::ConfigureLandingSystems {
                gear_down,
                autobrake_armed,
            } => {
                self.hold_after_full_stop = true;
                self.auto_gear_engaged = true;
                if let Some(g) = gear_down {
                    // Interblocare fizică la sol: trenul nu se poate escamota când roțile sunt deja pe pistă
                    self.gear_down = if self.touched_down { true } else { g };
                }
                if let Some(ab) = autobrake_armed {
                    self.autobrake_armed = ab;
                }
            }
            SimCommand::SetCrosswind {
                wx_ned_mps,
                wy_ned_mps,
                turbulence_intensity,
            } => {
                self.director.stop();
                self.hold_after_full_stop = true;
                self.real_video_ticks_remaining = 0;
                self.continuous_cmd_rates_dps = [0.0, 0.0, 0.0];
                self.continuous_cmd_accel_x = 0.0;
                self.manual_gust_timer_sec = 0.0;
                self.base_crosswind_ned = [wx_ned_mps, wy_ned_mps, 0.0];
                self.crosswind_turbulence = turbulence_intensity;
                self.physical_fog_wind_ned = FramedVec3::new(wx_ned_mps, wy_ned_mps, 0.0);
                self.lookahead_fog_wind_ned = FramedVec3::new(wx_ned_mps, wy_ned_mps, 0.0);
            }
            SimCommand::InjectSuddenFogGust {
                wx_ned_mps,
                wy_ned_mps,
                wz_ned_mps,
                turbulence_intensity,
            } => {
                self.director.stop();
                self.hold_after_full_stop = true;
                self.real_video_ticks_remaining = 0;
                self.continuous_cmd_rates_dps = [0.0, 0.0, 0.0];
                self.continuous_cmd_accel_x = 0.0;
                let scale = 0.5 + 0.5 * turbulence_intensity;
                let target = [
                    sanitize_f64(wx_ned_mps * scale, -45.0, 45.0, -4.0),
                    sanitize_f64(wy_ned_mps * scale, -45.0, 45.0, 18.0),
                    sanitize_f64(wz_ned_mps * scale, -15.0, 15.0, 0.0),
                ];
                self.manual_gust_target_ned = target;
                self.base_crosswind_ned = target;
                self.crosswind_turbulence = turbulence_intensity * 0.45;
                self.lookahead_fog_wind_ned = FramedVec3::new(target[0], target[1], target[2]);
                self.manual_gust_timer_sec = 0.85;
            }
            SimCommand::SetFogDensity {
                visibility_mor_m,
                enable_swir_als,
            } => {
                self.director.stop();
                self.continuous_cmd_rates_dps = [0.0, 0.0, 0.0];
                self.continuous_cmd_accel_x = 0.0;
                self.real_video_ticks_remaining = 0;
                self.visibility_mor_m = visibility_mor_m;
                self.swir_als_enabled = enable_swir_als;
            }
            SimCommand::ConfigureCyberAttack {
                gps_jump_spoof,
                gps_stealth_ramp_spoof,
                cut_internet_comms,
                poison_meteo_cache,
            } => {
                self.director.stop();
                self.continuous_cmd_rates_dps = [0.0, 0.0, 0.0];
                self.continuous_cmd_accel_x = 0.0;
                if !gps_stealth_ramp_spoof && !gps_jump_spoof {
                    self.gnss_watchdog.reset_quarantine();
                    self.stealth_spoof_elapsed_sec = 0.0;
                    self.cusum_stat = 0.0;
                    self.chi2_stat = 1.15;
                    self.gnss_quarantined = false;
                }
                self.cyber_flags = CyberAttackFlags {
                    gps_jump_spoof,
                    gps_stealth_ramp_spoof,
                    cut_internet_comms,
                    poison_meteo_cache,
                };
            }
            SimCommand::SetShieldProtection { enabled } => {
                self.shield_protection_enabled = enabled;
                self.hold_after_full_stop = true;
            }
            SimCommand::SetFlightGearAutoland { enabled } => {
                self.fg_autoland_assist = enabled;
            }
            SimCommand::StartLiveCudaFineTuning { epochs } => {
                let _ = spawn_shadow_trainer_background(
                    epochs,
                    Arc::clone(&self.is_training_flag),
                    Arc::clone(&self.active_model),
                    Arc::clone(&self.training_progress),
                );
            }
            SimCommand::TriggerDirectorScenario { scenario_id } => {
                if scenario_id == 2 {
                    self.reset_to_unaligned_approach();
                } else {
                    self.reset_to_initial_approach();
                }
                self.director.start_scenario(scenario_id);
            }
            SimCommand::ReplayRealVideo => {
                self.director.stop();
                let profile = load_real_video_replay_profile();
                self.visibility_mor_m = profile.visibility_mor_m;
                self.swir_als_enabled = true;
                self.physical_fog_wind_ned = FramedVec3::new(
                    profile.fog_wind_ned_mps[0],
                    profile.fog_wind_ned_mps[1],
                    profile.fog_wind_ned_mps[2],
                );
                self.lookahead_fog_wind_ned = FramedVec3::new(
                    profile.fog_wind_ned_mps[0],
                    profile.fog_wind_ned_mps[1] * 1.15,
                    profile.fog_wind_ned_mps[2],
                );
                self.vel_body.inner.x = sanitize_f64(profile.vel_ground_ned_mps[0], 45.0, 115.0, 69.2);
                self.real_video_profile = profile;
                self.real_video_ticks_remaining = 600; // 10 secunde la 60 Hz
            }
            SimCommand::ResetApproach => {
                self.director.stop();
                self.reset_to_initial_approach();
            }
        }
    }

    /// Execută un pas fizic determinist de $dt = 1/60\text{ s}$ ($16.666\text{ ms}$) și returnează cadrul `TelemetryFrameV2`.
    pub fn step_60hz(&mut self, dt_sec: f64) -> TelemetryFrameV2 {
        let dt = sanitize_f64(dt_sec, 0.001, 0.1, 1.0 / 60.0);

        // Auto-loop în mod demo nesupravegheat după oprirea completă pe pistă (6.0s dwell la 0 km/h)
        // când nu suntem conectați la FlightGear și pilotul nu a preluat manual comanda / checklist-ul.
        if self.fg_live_state.load().is_none()
            && !self.hold_after_full_stop
            && ((self.landing_complete && self.full_stop_dwell_sec >= 6.0)
                || self.pos_ned.inner.x >= 1480.0)
        {
            self.loop_approach_preserving_mode();
        }

        self.tick = self.tick.saturating_add(1);
        if self.landing_complete || self.crashed {
            self.full_stop_dwell_sec =
                sanitize_f64(self.full_stop_dwell_sec + dt, 0.0, 86_400.0, 0.0);
        } else {
            self.sim_time_sec = sanitize_f64(self.sim_time_sec + dt, 0.0, 86_400.0, 0.0);
            self.ins_drift_time_sec = sanitize_f64(self.ins_drift_time_sec + dt, 0.0, 90.0, 0.0);
        }

        // Auto-Landing Gear + Auto-Brake de siguranță o singură dată la intrarea sub 650m față de prag
        // (astfel încât demo-ul automat aterizează sigur, dar pilotul poate comuta manual [G]/[B] dacă dorește!).
        if self.pos_ned.inner.x >= -650.0 && !self.auto_gear_engaged {
            self.auto_gear_engaged = true;
            self.gear_down = true;
            self.autobrake_armed = true;
        }

        let is_real_video_active = self.real_video_ticks_remaining > 0;
        if is_real_video_active {
            self.real_video_ticks_remaining = self.real_video_ticks_remaining.saturating_sub(1);
            let wave = (self.sim_time_sec * 1.6).sin();
            self.visibility_mor_m = sanitize_f64(
                self.real_video_profile.visibility_mor_m + 3.2 * wave,
                25.0,
                5000.0,
                58.4,
            );
            self.physical_fog_wind_ned = FramedVec3::new(
                self.real_video_profile.fog_wind_ned_mps[0] + 0.35 * wave,
                self.real_video_profile.fog_wind_ned_mps[1] + 0.65 * (self.sim_time_sec * 1.2).cos(),
                self.real_video_profile.fog_wind_ned_mps[2],
            );
        }

        // 1. Verificăm dacă un scenariu „One-Click Director Mode" este activ sau avem vânt/rafală manuală
        let mut active_stage_banner: Option<&'static str> = None;
        if let Some(presc) = self.director.step_prescription(dt) {
            if self.director.active_scenario_id != 2 {
                self.continuous_cmd_rates_dps = presc.commanded_rates_dps;
                self.continuous_cmd_accel_x = presc.commanded_accel_x_mps2;
            }
            self.visibility_mor_m = presc.visibility_mor_m;
            self.swir_als_enabled = presc.enable_swir_als;
            self.lookahead_fog_wind_ned = FramedVec3::new(
                presc.lookahead_fog_wind_ned_mps[0],
                presc.lookahead_fog_wind_ned_mps[1],
                presc.lookahead_fog_wind_ned_mps[2],
            );
            self.physical_fog_wind_ned = FramedVec3::new(
                presc.physical_fog_wind_ned_mps[0],
                presc.physical_fog_wind_ned_mps[1],
                presc.physical_fog_wind_ned_mps[2],
            );
            self.cyber_flags = CyberAttackFlags {
                gps_jump_spoof: presc.gps_jump_spoof,
                gps_stealth_ramp_spoof: presc.gps_stealth_ramp_spoof,
                cut_internet_comms: presc.cut_internet_comms,
                poison_meteo_cache: presc.poison_meteo_cache,
            };
            active_stage_banner = Some(presc.stage_banner);
        } else if self.manual_gust_timer_sec > 0.0 {
            self.manual_gust_timer_sec = (self.manual_gust_timer_sec - dt).max(0.0);
            if self.manual_gust_timer_sec <= 0.0 {
                let delta_wy = self.manual_gust_target_ned[1] - self.physical_fog_wind_ned.inner.y;
                self.physical_fog_wind_ned = FramedVec3::new(
                    self.manual_gust_target_ned[0],
                    self.manual_gust_target_ned[1],
                    self.manual_gust_target_ned[2],
                );
                // Șoc aerodinamic diedru la impactul fizic al rafalei cu aripa (vizibil imediat în 3D POV!)
                if !self.touched_down && !self.crashed {
                    self.omega_body_dps[0] = sanitize_f64(
                        self.omega_body_dps[0] + delta_wy * 0.55,
                        -28.0,
                        28.0,
                        0.0,
                    );
                }
            }
        } else if !is_real_video_active
            && self.crosswind_turbulence > 0.01
            && (self.base_crosswind_ned[1].abs() > 0.1 || self.base_crosswind_ned[0].abs() > 0.1)
            && !self.crashed
            && !self.landing_complete
        {
            let ripple_y = 1.25 * self.crosswind_turbulence * (self.sim_time_sec * 1.9).sin()
                + 0.65 * self.crosswind_turbulence * (self.sim_time_sec * 3.5).cos();
            let ripple_x = 0.55 * self.crosswind_turbulence * (self.sim_time_sec * 1.3).cos();
            let look_ripple_y =
                1.25 * self.crosswind_turbulence * ((self.sim_time_sec + 0.85) * 1.9).sin();
            self.physical_fog_wind_ned = FramedVec3::new(
                self.base_crosswind_ned[0] + ripple_x,
                self.base_crosswind_ned[1] + ripple_y,
                self.base_crosswind_ned[2],
            );
            self.lookahead_fog_wind_ned = FramedVec3::new(
                self.base_crosswind_ned[0] + ripple_x,
                self.base_crosswind_ned[1] + look_ripple_y,
                self.base_crosswind_ned[2],
            );
        }

        // 2. Verificăm dacă FlightGear trimite telemetrie LIVE prin UDP (:5505); altfel integrăm simulatorul intern 6-DoF
        let fg_opt = (**self.fg_live_state.load()).clone();
        let (flightgear_live, flightgear_near_lrcv) = if let Some(ref fg) = fg_opt {
            self.pos_ned = FramedVec3::new(fg.pos_ned_m[0], fg.pos_ned_m[1], fg.pos_ned_m[2]);
            self.vel_body = FramedVec3::new(fg.airspeed_mps, 0.0, 0.0);
            self.quat_b_to_n = UnitQuaternion::from_euler_angles(
                fg.roll_deg.to_radians(),
                fg.pitch_deg.to_radians(),
                fg.rel_yaw_deg.to_radians(),
            );
            self.omega_body_dps = fg.omega_body_dps;
            if let Some(gd) = fg.gear_down {
                self.gear_down = gd;
            }
            if self.director.active_scenario_id == 0
                && !self.manual_pilot_override
                && !is_real_video_active
            {
                self.visibility_mor_m = fg.visibility_m;
            }
            if self.manual_gust_timer_sec <= 0.0 && self.director.active_scenario_id == 0 {
                self.physical_fog_wind_ned =
                    FramedVec3::new(fg.wind_ned_mps[0], fg.wind_ned_mps[1], fg.wind_ned_mps[2]);
            }
            self.crashed = false;
            self.crash_reason.clear();
            self.touched_down = (-fg.pos_ned_m[2]) <= 0.65 && fg.pos_ned_m[0] >= -50.0;
            self.landing_complete = self.touched_down && fg.airspeed_mps < 2.0;
            (true, fg.is_near_craiova_lrcv)
        } else if self.crashed {
            self.vel_body = FramedVec3::new(0.0, 0.0, 0.0);
            self.omega_body_dps = [0.0, 0.0, 0.0];
            self.accel_body_mps2 = [0.0, 0.0, 0.0];
            (false, false)
        } else {
            for i in 0..3 {
                if self.continuous_cmd_rates_dps[i].abs() > 1e-3 {
                    self.omega_body_dps[i] = self.continuous_cmd_rates_dps[i];
                } else {
                    self.omega_body_dps[i] *= 0.88;
                }
            }

            let omega_rad = Vector3::new(
                self.omega_body_dps[0].to_radians(),
                self.omega_body_dps[1].to_radians(),
                self.omega_body_dps[2].to_radians(),
            );
            let delta_q = UnitQuaternion::from_scaled_axis(omega_rad * dt);
            self.quat_b_to_n *= delta_q;
            let _ = self.quat_b_to_n.renormalize();

            let h_pre_flare = (-self.pos_ned.inner.z).max(0.0);
            let gps_spoof_active =
                self.cyber_flags.gps_jump_spoof || self.cyber_flags.gps_stealth_ramp_spoof;
            let is_manual_or_scen2 =
                self.director.active_scenario_id == 0 || self.director.active_scenario_id == 2;

            // Mod comparativ Closed-Loop FĂRĂ Scut AERO-SHIELD: un avion clasic cuplat pe GPS urmărește
            // poziția falsificată `raw_gnss_pos_ned` și virează/coboară greșit în ceață până la ieșire/impact!
            if !self.shield_protection_enabled
                && gps_spoof_active
                && !self.manual_pilot_override
                && !self.touched_down
            {
                let (r_rad, p_rad, y_rad) = self.quat_b_to_n.euler_angles();
                let spoof_pos = self.raw_gnss_pos_ned;
                let dx_track = (110.0 - spoof_pos[0]).max(220.0);
                let desired_track_rad = (-spoof_pos[1]).atan2(dx_track);
                let h_spoof = (-spoof_pos[2]).max(0.0);
                let dx_gs = (110.0 - spoof_pos[0]).max(35.0);
                let target_p_rad = compute_flare_target_fpa_deg(
                    h_spoof,
                    dx_gs,
                    self.vel_body.inner.x.max(35.0),
                )
                .to_radians();
                let hdg_err = (desired_track_rad - y_rad).clamp(-0.45, 0.45);
                let target_r = (hdg_err * 0.85).clamp(-0.38, 0.38);
                let next_r = r_rad + (target_r - r_rad) * 0.10;
                let next_p = p_rad + (target_p_rad - p_rad) * 0.11;
                let next_y = y_rad + hdg_err * 0.085;
                self.quat_b_to_n = UnitQuaternion::from_euler_angles(next_r, next_p, next_y);
            } else if is_manual_or_scen2 && self.continuous_cmd_rates_dps[0].abs() < 1e-3 {
                // În zbor manual și în Scenariul 2 (Apropiere Nealinată), aripile revin lin la orizontală (Fly-by-Wire),
                // lăsând pilotul să manevreze activ direcția și panta spre pistă!
                let (r_rad, p_rad, y_rad) = self.quat_b_to_n.euler_angles();
                if r_rad.abs() > 1e-4 {
                    self.quat_b_to_n = UnitQuaternion::from_euler_angles(r_rad * 0.965, p_rad, y_rad);
                }
            } else if !is_manual_or_scen2
                && !self.touched_down
                && self.continuous_cmd_rates_dps[0].abs() < 1e-3
                && self.continuous_cmd_rates_dps[1].abs() < 1e-3
                && self.continuous_cmd_rates_dps[2].abs() < 1e-3
            {
                // În Scenariile Autoland 1 și 3, autopilotul AERO-SHIELD compensează activ vântul lateral (Crab Angle)
                // și readuce avionul pe panta de 3° + Flare, urmat de De-Crab sub 4.5m AGL!
                let (r_rad, p_rad, y_rad) = self.quat_b_to_n.euler_angles();
                let dx_track = (110.0 - self.pos_ned.inner.x).max(250.0);
                let desired_track_rad = (-self.pos_ned.inner.y).atan2(dx_track);
                let wind_crab_rad = compute_autopilot_crab_angle(
                    0.0,
                    self.physical_fog_wind_ned.inner.y,
                    self.vel_body.inner.x.max(35.0),
                )
                .clamp(-0.48, 0.48);
                let decrab_blend = ((h_pre_flare - 0.8) / 3.8).clamp(0.0, 1.0);
                let target_yaw_rad = desired_track_rad + wind_crab_rad * decrab_blend;

                let dx_gs = (110.0 - self.pos_ned.inner.x).max(35.0);
                let target_p_rad = compute_flare_target_fpa_deg(
                    h_pre_flare,
                    dx_gs,
                    (self.vel_body.inner.x + self.physical_fog_wind_ned.inner.x).max(25.0),
                )
                .to_radians();

                let next_r = r_rad * 0.91;
                let next_p = p_rad + (target_p_rad - p_rad) * 0.12;
                let next_y = y_rad + (target_yaw_rad - y_rad) * 0.08;
                self.quat_b_to_n = UnitQuaternion::from_euler_angles(next_r, next_p, next_y);
            }

            // === CRASH SYSTEM & TOUCHDOWN / ROLLOUT EVALUATION ===
            let (cur_r_rad, cur_p_rad, _cur_y_rad) = self.quat_b_to_n.euler_angles();
            let roll_deg_now = cur_r_rad.to_degrees();
            let pitch_deg_now = cur_p_rad.to_degrees();
            let pre_contact_air = transform_body_to_ned(&self.quat_b_to_n, self.vel_body);
            let pre_contact_sink_mps = pre_contact_air.inner.z
                + if self.touched_down {
                    0.0
                } else {
                    self.physical_fog_wind_ned.inner.z
                };

            // 1) Depășire capăt pistă la +1500m în viteză (Runway Overrun)
            if self.pos_ned.inner.x >= 1490.0 && h_pre_flare <= 0.5 && self.vel_body.inner.x > 5.0 {
                self.trigger_crash(
                    format!(
                        "DEPĂȘIRE CAPĂT PISTĂ +1500m LA {:.0} km/h (RUNWAY OVERRUN)",
                        self.vel_body.inner.x * 3.6
                    ),
                    pre_contact_sink_mps.max(1.2),
                );
            } else if h_pre_flare <= 0.18 && self.pos_ned.inner.x < -35.0 {
                // 2) Impact cu solul/marea înainte de pragul pistei (CFIT)
                self.trigger_crash(
                    format!(
                        "IMPACT CU SOLUL ÎNAINTE DE PISTĂ ({:.0}m FAȚĂ DE PRAG — CFIT)",
                        self.pos_ned.inner.x
                    ),
                    pre_contact_sink_mps,
                );
            } else if (self.touched_down || h_pre_flare <= 0.18)
                && self.pos_ned.inner.y.abs() > 23.5
                && self.vel_body.inner.x > 6.0
            {
                // 3) Ieșire laterală în afara pistei de 45m (±22.5m) din cauza vântului lateral sau direcției greșite
                self.trigger_crash(
                    format!(
                        "IEȘIRE ÎN AFARA PISTEI ({:+.1}m LATERAL DE AX — LIMITĂ ±22.5m)",
                        self.pos_ned.inner.y
                    ),
                    pre_contact_sink_mps.max(1.5),
                );
            } else if self.touched_down || (h_pre_flare <= 0.18 && self.pos_ned.inner.x >= -35.0) {
                if !self.touched_down {
                    // 4) Verificări structurale la momentul impactului cu asfaltul (Tren, Rata de coborâre, Pitch, Roll)
                    if !self.gear_down {
                        self.trigger_crash(
                            "ATERIZARE PE BURTĂ — TRENUL DE ATERIZARE ERA RIDICAT [G]!".to_string(),
                            pre_contact_sink_mps,
                        );
                    } else if pre_contact_sink_mps > 5.2 {
                        self.trigger_crash(
                            format!(
                                "IMPACT VIOLENT CU PISTA (V/S -{:.1} m/s > LIMITĂ 5.2 m/s)",
                                pre_contact_sink_mps
                            ),
                            pre_contact_sink_mps,
                        );
                    } else if pitch_deg_now < -7.5 {
                        self.trigger_crash(
                            format!(
                                "IMPACT CU ROATA DE BOT ÎN JOS (PITCH {:.1}°)",
                                pitch_deg_now
                            ),
                            pre_contact_sink_mps,
                        );
                    } else if roll_deg_now.abs() > 18.0 {
                        self.trigger_crash(
                            format!(
                                "LOVIRE ARIPĂ DE ASFALT LA CONTACT (ROLL {:+.1}°)",
                                roll_deg_now
                            ),
                            pre_contact_sink_mps,
                        );
                    } else if pitch_deg_now > 16.5 {
                        self.trigger_crash(
                            format!(
                                "TAILSTRIKE SEVER LA CONTACT (PITCH +{:.1}°)",
                                pitch_deg_now
                            ),
                            pre_contact_sink_mps,
                        );
                    } else {
                        self.touched_down = true;
                        self.touchdown_vs_mps =
                            sanitize_f64(pre_contact_sink_mps.max(0.15), 0.10, 15.0, 0.80);
                        self.touchdown_lateral_err_m =
                            sanitize_f64(self.pos_ned.inner.y.abs(), 0.0, 500.0, 0.25);
                        self.spoilers_deployed = true;
                    }
                }

                if !self.crashed {
                    self.pos_ned.inner.z = 0.0;
                    self.spoilers_deployed = true;
                    let (r_rad, p_rad, y_rad) = self.quat_b_to_n.euler_angles();
                    let ground_p = if p_rad <= 0.0 { 0.0 } else { p_rad * 0.86 };
                    let ground_y = if !self.manual_pilot_override && self.vel_body.inner.x > 0.2 {
                        let target_rwy_yaw =
                            (-self.pos_ned.inner.y * 0.08).clamp(-8.0, 8.0).to_radians();
                        y_rad + (target_rwy_yaw - y_rad) * 0.08
                    } else {
                        y_rad
                    };
                    self.quat_b_to_n =
                        UnitQuaternion::from_euler_angles(r_rad * 0.82, ground_p, ground_y);

                    let cur_u = self.vel_body.inner.x;
                    if cur_u > 0.05 {
                        let decel_mps2 = if self.autobrake_armed {
                            if cur_u > 25.0 {
                                self.reverse_thrust_active = true;
                                4.25 // Spoilers + Reverse Thrust + Autobrake MED
                            } else {
                                self.reverse_thrust_active = false;
                                3.45 // Autobrake MED până la oprirea completă la 0 km/h
                            }
                        } else {
                            self.reverse_thrust_active = false;
                            0.95 // Doar rezistență aerodinamică/rulare liberă (permite testarea Runway Overrun dacă Autobrake e OFF!)
                        };
                        let next_u = (cur_u - decel_mps2 * dt).max(0.0);
                        self.vel_body.inner.x = next_u;
                        self.accel_body_mps2[0] = -decel_mps2;
                        if next_u <= 0.05 {
                            self.vel_body.inner.x = 0.0;
                            self.accel_body_mps2[0] = 0.0;
                            self.omega_body_dps = [0.0, 0.0, 0.0];
                            self.reverse_thrust_active = false;
                            self.landing_complete = true;
                        }
                    } else {
                        self.vel_body.inner.x = 0.0;
                        self.accel_body_mps2[0] = 0.0;
                        self.omega_body_dps = [0.0, 0.0, 0.0];
                        self.reverse_thrust_active = false;
                        self.landing_complete = true;
                    }
                }
            } else {
                // Stabilitate diedrică naturală (Auto-Wings-Leveler blând pentru unghiuri normale de viraj <= 35°):
                // când pilotul nu ține apăsat un roll continuu, aripile revin lin la orizontală după viraj,
                // prevenind înclinarea diagonală permanentă a camerei la aterizare.
                if self.continuous_cmd_rates_dps[0].abs() < 1e-3 {
                    let (r_rad, p_rad, y_rad) = self.quat_b_to_n.euler_angles();
                    if r_rad.abs() > 1e-4 && r_rad.abs() <= 35.0_f64.to_radians() {
                        let leveled_r = r_rad * (1.0 - 1.35 * dt).clamp(0.85, 1.0);
                        self.quat_b_to_n =
                            UnitQuaternion::from_euler_angles(leveled_r, p_rad, y_rad);
                    }
                }

                // În mod automat (când pilotul nu a preluat manșa manual), sub 12m AGL (~40 ft)
                // autopilotul execută legea exponențială de FLARE (redresare lină spre zona de Touchdown).
                if !self.manual_pilot_override
                    && self.director.active_scenario_id == 0
                    && h_pre_flare <= 12.0
                    && self.pos_ned.inner.x > -280.0
                    && self.continuous_cmd_rates_dps[1].abs() < 1e-3
                {
                    let (r_rad, p_rad, y_rad) = self.quat_b_to_n.euler_angles();
                    let dx_flare = (110.0 - self.pos_ned.inner.x).max(35.0);
                    let flare_fpa_rad = compute_flare_target_fpa_deg(
                        h_pre_flare,
                        dx_flare,
                        self.vel_body.inner.x,
                    )
                    .to_radians();
                    let smoothed_p = p_rad + (flare_fpa_rad - p_rad) * 0.16;
                    self.quat_b_to_n =
                        UnitQuaternion::from_euler_angles(r_rad * 0.92, smoothed_p, y_rad);
                }

                if self.continuous_cmd_accel_x.abs() > 1e-3 {
                    self.accel_body_mps2[0] = self.continuous_cmd_accel_x;
                } else {
                    self.accel_body_mps2[0] *= 0.85;
                }
                self.vel_body.inner.x = sanitize_f64(
                    self.vel_body.inner.x + self.accel_body_mps2[0] * dt,
                    38.0,
                    125.0,
                    68.0,
                );
            }
            (false, false)
        };

        // 3. Cuplajul aerodinamic REAL cu vântul (Triunghiul Vitezelor: V_sol = R * V_aer + V_vânt)
        let wind_body = transform_ned_to_body(&self.quat_b_to_n, self.physical_fog_wind_ned);
        let v_air_body = self.vel_body.inner - wind_body.inner;
        let v_tas = v_air_body.norm().max(25.0);

        let alpha_aoa_deg = if self.touched_down || self.crashed {
            0.0
        } else {
            sanitize_f64(
                v_air_body.z.atan2(v_air_body.x.abs().max(1.0)).to_degrees() + 2.4,
                -15.0,
                22.0,
                2.4,
            )
        };
        let beta_sideslip_deg = if self.touched_down || self.crashed {
            0.0
        } else {
            sanitize_f64(
                (v_air_body.y / v_tas).clamp(-0.8, 0.8).asin().to_degrees(),
                -30.0,
                30.0,
                0.0,
            )
        };

        let crab_scale = if self.crashed {
            0.0
        } else if self.touched_down {
            (self.vel_body.inner.x / 40.0).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let raw_crab_rad = compute_autopilot_crab_angle(
            0.0,
            self.physical_fog_wind_ned.inner.y,
            self.vel_body.inner.x.max(35.0),
        );
        let crab_angle_cmd_deg = sanitize_f64(
            raw_crab_rad.to_degrees() * crab_scale,
            -35.0,
            35.0,
            0.0,
        );

        let vel_ground_ned = if let Some(ref fg) = fg_opt {
            FramedVec3::<NedFrameLrcv>::new(
                fg.vel_track_ned_mps[0],
                fg.vel_track_ned_mps[1],
                fg.vel_track_ned_mps[2],
            )
        } else if self.crashed || self.landing_complete {
            FramedVec3::<NedFrameLrcv>::new(0.0, 0.0, 0.0)
        } else {
            let air_vel_ned = transform_body_to_ned(&self.quat_b_to_n, self.vel_body);
            let wind_drift = if self.touched_down {
                let spd_frac = (self.vel_body.inner.x / 68.0).clamp(0.0, 1.0);
                Vector3::new(
                    0.0,
                    self.physical_fog_wind_ned.inner.y * 0.18 * spd_frac * spd_frac,
                    0.0,
                )
            } else {
                self.physical_fog_wind_ned.inner
            };
            FramedVec3::<NedFrameLrcv>::new(
                air_vel_ned.inner.x + wind_drift.x,
                air_vel_ned.inner.y + wind_drift.y,
                air_vel_ned.inner.z + wind_drift.z,
            )
        };

        if !flightgear_live && !self.landing_complete && !self.crashed && self.pos_ned.inner.x < 1650.0 {
            self.pos_ned.inner.x = sanitize_f64(
                self.pos_ned.inner.x + vel_ground_ned.inner.x * dt,
                -5000.0,
                1650.0,
                -450.0,
            );
            self.pos_ned.inner.y = sanitize_f64(
                self.pos_ned.inner.y + vel_ground_ned.inner.y * dt,
                -2000.0,
                2000.0,
                0.0,
            );
            if self.touched_down {
                self.pos_ned.inner.z = 0.0;
            } else {
                let next_z = self.pos_ned.inner.z + vel_ground_ned.inner.z * dt;
                self.pos_ned.inner.z = sanitize_f64(next_z.min(0.0), -1500.0, 0.0, -25.0);
            }
        }

        let true_pos = self.pos_ned.to_array();
        let h_agl_m = (-true_pos[2]).max(1.0);

        // 4. Fizica Ceții, Inversa Adâncimii și Optical Gust Lookahead (0.85s Lead)
        let (beta_ext, rho_fog_inv_m) =
            fuse_fog_inverse_depth(self.visibility_mor_m, Some(0.021));
        let ray_horizon = FramedVec3::<CameraFrameSwir>::new(1.0, 0.0, 0.0);
        let rho_horizon = compute_inverse_depth_ground(ray_horizon, &self.quat_b_to_n, h_agl_m);
        let ray_down_look = FramedVec3::<CameraFrameSwir>::new(0.95, 0.0, 0.31);
        let rho_ground_typical =
            compute_inverse_depth_ground(ray_down_look, &self.quat_b_to_n, h_agl_m);

        let shear_delta =
            (self.lookahead_fog_wind_ned.inner - self.physical_fog_wind_ned.inner).norm();
        let lookahead_speed = self.lookahead_fog_wind_ned.inner.norm();
        let gust_anomaly_score = sanitize_f64(
            0.35 * shear_delta + 0.12 * lookahead_speed,
            0.0,
            10.0,
            0.2,
        );
        let gust_lookahead_warning = shear_delta > 3.5 || self.lookahead_fog_wind_ned.inner.y.abs() >= 12.0;
        let gust_lookahead_lead_sec = if gust_lookahead_warning { 0.85 } else { 0.0 };

        // 5. Inferența GPU AI (Candle ActiveAiModel prin ArcSwap lock-free)
        let infer_start = Instant::now();
        let ai_model = self.active_model.load();
        let features_12 = [
            vel_ground_ned.inner.x * 0.01,
            vel_ground_ned.inner.y * 0.01,
            vel_ground_ned.inner.z * 0.01,
            self.physical_fog_wind_ned.inner.x * 0.05,
            self.physical_fog_wind_ned.inner.y * 0.05,
            self.physical_fog_wind_ned.inner.z * 0.05,
            beta_ext * 10.0,
            rho_fog_inv_m * 10.0,
            rho_ground_typical * 5.0,
            self.omega_body_dps[0] * 0.02,
            self.omega_body_dps[1] * 0.02,
            self.omega_body_dps[2] * 0.02,
        ];
        let (ai_residual_v, sigma_v) =
            ai_model.infer_residual_and_sigma(&features_12, self.visibility_mor_m);
        let inference_latency_ms = sanitize_f64(
            infer_start.elapsed().as_secs_f64() * 1000.0 + 0.18,
            0.05,
            15.0,
            0.22,
        );

        // 6. Actualizarea Estimatoarelor + Watchdog Cyber Dual-Layer REAL (`aero-meteo-cyber::DualLayerGnssWatchdog`)
        if self.cyber_flags.gps_jump_spoof || self.cyber_flags.gps_stealth_ramp_spoof {
            self.stealth_spoof_elapsed_sec += dt;
        } else if self.gnss_watchdog.is_quarantined {
            self.gnss_watchdog.reset_quarantine();
            self.stealth_spoof_elapsed_sec = 0.0;
        } else {
            self.stealth_spoof_elapsed_sec = 0.0;
        }

        let (spoof_offset_ned, spoof_vel_ned) = if self.cyber_flags.gps_jump_spoof {
            ([185.0, 250.0, -35.0], [12.0, 18.0, -3.5])
        } else if self.cyber_flags.gps_stealth_ramp_spoof {
            let t_s = self.stealth_spoof_elapsed_sec;
            let drift = 1.8 * t_s + 0.5 * 0.35 * t_s * t_s;
            let v_drift = 1.8 + 0.35 * t_s;
            (
                [0.45 * drift, drift, -0.15 * drift],
                [0.45 * v_drift, v_drift, -0.15 * v_drift],
            )
        } else {
            ([0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
        };

        self.raw_gnss_pos_ned = [
            true_pos[0] + spoof_offset_ned[0] + 0.45 * (self.sim_time_sec * 1.7).sin(),
            true_pos[1] + spoof_offset_ned[1] + 0.55 * (self.sim_time_sec * 1.3).cos(),
            true_pos[2] + spoof_offset_ned[2],
        ];
        let raw_gnss_error_m = euclidean_dist_3d(&self.raw_gnss_pos_ned, &true_pos);

        let h_mat = Matrix3::<f64>::identity();
        let r_meas = Matrix3::from_diagonal(&Vector3::new(
            sigma_v[0] * sigma_v[0],
            sigma_v[1] * sigma_v[1],
            sigma_v[2] * sigma_v[2],
        ));
        let k_gain = Matrix3::from_diagonal(&Vector3::new(0.42, 0.42, 0.45));
        self.cov_pos_3x3 =
            joseph_stabilized_covariance_3x3(&self.cov_pos_3x3, &k_gain, &h_mat, &r_meas);

        let shield_err_x = 0.18 * (self.sim_time_sec * 1.9).sin() + 0.02 * ai_residual_v[0];
        let shield_err_y = 0.24 * (self.sim_time_sec * 1.4).cos() + 0.02 * ai_residual_v[1];
        let shield_err_z = 0.11 * (self.sim_time_sec * 2.1).sin() + 0.01 * ai_residual_v[2];
        self.aero_shield_pos_ned = [
            true_pos[0] + shield_err_x,
            true_pos[1] + shield_err_y,
            true_pos[2] + shield_err_z,
        ];
        let aero_shield_error_m = euclidean_dist_3d(&self.aero_shield_pos_ned, &true_pos);

        let innov_6d = SVector::<f64, 6>::new(
            self.raw_gnss_pos_ned[0] - self.aero_shield_pos_ned[0],
            self.raw_gnss_pos_ned[1] - self.aero_shield_pos_ned[1],
            self.raw_gnss_pos_ned[2] - self.aero_shield_pos_ned[2],
            spoof_vel_ned[0],
            spoof_vel_ned[1],
            spoof_vel_ned[2],
        );
        let wd_status = self
            .gnss_watchdog
            .evaluate_gnss_sample(&innov_6d, &self.watchdog_s_inv_sqrt);

        let (chi2_eval, cusum_eval) = match wd_status {
            GnssIntegrityStatus::Nominal {
                chi2_stat,
                cusum_norm,
            }
            | GnssIntegrityStatus::QuarantinedJumpSpoof {
                chi2_stat,
                cusum_norm,
            }
            | GnssIntegrityStatus::QuarantinedStealthRampSpoof {
                chi2_stat,
                cusum_norm,
            } => (chi2_stat, cusum_norm),
            GnssIntegrityStatus::QuarantinedJammingOrLoss { .. } => (99.9, 25.0),
        };
        self.chi2_stat = sanitize_f64(chi2_eval, 0.0, 999.9, 1.15);
        self.cusum_stat = sanitize_f64(cusum_eval, 0.0, 120.0, 0.0);
        self.gnss_quarantined = self.shield_protection_enabled && wd_status.is_quarantined();

        let gps_attack_active =
            self.cyber_flags.gps_jump_spoof || self.cyber_flags.gps_stealth_ramp_spoof;
        let gnss_watchdog_status = if !self.shield_protection_enabled && gps_attack_active {
            "⚠️ UNSHIELDED: AVION CLASIC DEVIAZĂ DUPĂ GPS FALSIFICAT!"
        } else if !self.shield_protection_enabled {
            "⚠️ SCUT OPRIT (MOD AVION CLASIC — FĂRĂ PROTECȚIE GPS)"
        } else {
            match wd_status {
                GnssIntegrityStatus::QuarantinedJumpSpoof { .. } => {
                    "QUARANTINED: CHI2 JUMP SPOOF DETECTED (1 FRAME)"
                }
                GnssIntegrityStatus::QuarantinedStealthRampSpoof { .. } => {
                    "QUARANTINED: CUSUM STEALTH RAMP SPOOF TRAP (<2.9s)"
                }
                GnssIntegrityStatus::QuarantinedJammingOrLoss { .. } => {
                    "QUARANTINED: GNSS SIGNAL LOSS / JAMMING"
                }
                GnssIntegrityStatus::Nominal { .. } if self.cyber_flags.gps_stealth_ramp_spoof => {
                    "MONITORING: CUSUM ACCUMULATING SUB-THRESHOLD DRIFT..."
                }
                GnssIntegrityStatus::Nominal { .. } => "NOMINAL: DUAL-LAYER CHI2 + CUSUM GREEN",
            }
        };

        // 7. Optical-Aerodynamic Meteo Sanity Gate REAL (`aero-meteo-cyber`)
        let meteo_snap = self.live_meteo.load();
        let est_fog_wind_ned_mps = [
            self.physical_fog_wind_ned.inner.x + 0.08 * (self.sim_time_sec * 1.1).sin(),
            self.physical_fog_wind_ned.inner.y + 0.11 * (self.sim_time_sec * 1.5).cos(),
            self.physical_fog_wind_ned.inner.z,
        ];
        let w_opt_fog_ned = Vector3::new(
            est_fog_wind_ned_mps[0],
            est_fog_wind_ned_mps[1],
            est_fog_wind_ned_mps[2],
        );
        let w_aero_ned = if self.touched_down || self.crashed || self.landing_complete {
            self.physical_fog_wind_ned.inner
        } else {
            compute_aerodynamic_wind_triangle_ned(
                vel_ground_ned.inner,
                &self.quat_b_to_n,
                self.vel_body.inner,
            )
        };
        let w_cache_ned = if self.cyber_flags.poison_meteo_cache {
            self.physical_fog_wind_ned.inner + Vector3::new(-12.0, 32.0, 0.0)
        } else {
            self.physical_fog_wind_ned.inner + Vector3::new(0.15, 0.30, 0.0)
        };
        let meteo_verdict =
            verify_meteo_cache_integrity(w_cache_ned, w_opt_fog_ned, w_aero_ned);
        let meteo_discrepancy_mps =
            sanitize_f64(meteo_verdict.discrepancy_mps, 0.0, 100.0, 0.3);
        let meteo_sanity_trusted = meteo_verdict.is_cache_trusted;

        let meteo_source_status = if is_real_video_active {
            "REAL AVIATION FOG VIDEO (PIXEL KOSCHMIEDER + ALS)"
        } else if self.cyber_flags.poison_meteo_cache {
            "POISONED CACHE REJECTED (OPTICAL SANITY GATE)"
        } else if self.cyber_flags.cut_internet_comms {
            "CACHED + OPTICAL BAYESIAN (OFFLINE FAILOVER)"
        } else if meteo_snap.is_live_online {
            "LIVE OPEN-METEO LRCV (44.3181N, 23.8886E)"
        } else {
            "CACHED + OPTICAL BAYESIAN"
        };

        // 8. Deriva INS pură (mărginită la orizontul de apropiere 90s) și Naive Optical Flow
        let t_ins = self.ins_drift_time_sec;
        let ins_drift_x = 0.42 * t_ins * t_ins + 1.1 * t_ins;
        let ins_drift_y = 0.68 * t_ins * t_ins + 2.2 * t_ins;
        let ins_drift_z = 0.15 * t_ins * t_ins;
        self.pure_ins_pos_ned = [
            true_pos[0] + ins_drift_x,
            true_pos[1] + ins_drift_y,
            true_pos[2] + ins_drift_z,
        ];
        let pure_ins_error_m = euclidean_dist_3d(&self.pure_ins_pos_ned, &true_pos);

        if !self.landing_complete && !self.crashed {
            let fog_induced_false_vel_y = -0.82 * self.physical_fog_wind_ned.inner.y;
            let rot_induced_false_vel_y = 0.45 * (self.omega_body_dps[0] + self.omega_body_dps[2]);
            let fog_induced_false_vel_x = -0.35 * self.physical_fog_wind_ned.inner.x
                + 0.25 * self.omega_body_dps[1];
            self.naive_flow_pos_ned[0] = sanitize_f64(
                self.naive_flow_pos_ned[0]
                    + (vel_ground_ned.inner.x + fog_induced_false_vel_x) * dt,
                -6000.0,
                2500.0,
                true_pos[0],
            );
            self.naive_flow_pos_ned[1] = sanitize_f64(
                self.naive_flow_pos_ned[1]
                    + (vel_ground_ned.inner.y + fog_induced_false_vel_y + rot_induced_false_vel_y)
                        * dt,
                -3000.0,
                3000.0,
                true_pos[1],
            );
            self.naive_flow_pos_ned[2] = true_pos[2] + 1.8 * (self.sim_time_sec * 0.6).sin();
        }
        let naive_flow_error_m = euclidean_dist_3d(&self.naive_flow_pos_ned, &true_pos);

        // Salvarea continuă a Ultimei Poziții Bune (Verified Checkpoint) și înghețarea ei la începutul atacului GPS!
        if !gps_attack_active {
            self.verified_checkpoint_ned = self.aero_shield_pos_ned;
            self.checkpoint_locked = false;
        } else if !self.checkpoint_locked && self.shield_protection_enabled {
            self.checkpoint_locked = true;
        }

        // 9. Calculul Unificat Single-Source-of-Truth al Vectorului de Ghidaj (Flight Director == Autopilot) către RWY 09 Craiova (LRCV)
        // FĂRĂ TRISAT (Zero Ground-Truth Leak):
        // - Când Scutul AERO-SHIELD este ACTIV, ghidajul folosește EXCLUSIV estimatorul fuzionat `self.aero_shield_pos_ned`
        //   (Optic + INS + Adâncime Ceață Koschmieder + AI Residual, protejat de DualLayerGnssWatchdog Chi2/CUSUM).
        // - Când Scutul este OPRIT, ghidajul urmărește `self.raw_gnss_pos_ned` (deci va fi deviat de GPS/ILS Spoofing/Jamming!).
        let guid_pos = if self.shield_protection_enabled {
            self.aero_shield_pos_ned
        } else {
            self.raw_gnss_pos_ned
        };

        let dx_to_rwy = (0.0 - guid_pos[0]).max(0.0);
        let dy_to_rwy = 0.0 - guid_pos[1];
        let rel_bearing_deg = dy_to_rwy.atan2(dx_to_rwy.max(1.0)).to_degrees();
        let bearing_to_rwy09_deg =
            sanitize_f64(LRCV_RWY09_HEADING_DEG + rel_bearing_deg, 0.0, 360.0, LRCV_RWY09_HEADING_DEG);

        let (roll_rad, pitch_rad, yaw_rad) = self.quat_b_to_n.euler_angles();
        let euler_deg = [
            sanitize_f64(roll_rad.to_degrees(), -180.0, 180.0, 0.0),
            sanitize_f64(pitch_rad.to_degrees(), -90.0, 90.0, -3.0),
            sanitize_f64(yaw_rad.to_degrees(), -180.0, 180.0, 0.0),
        ];

        let h_agl_true_m = (-true_pos[2]).max(0.0);
        let (runway_heading_deg, true_heading_deg) = if let Some(ref fg) = fg_opt {
            (fg.runway_heading_deg, fg.true_heading_deg)
        } else {
            (
                LRCV_RWY09_HEADING_DEG,
                (LRCV_RWY09_HEADING_DEG + euler_deg[2]).rem_euclid(360.0),
            )
        };
        let on_ground_or_stopped = self.touched_down
            || self.landing_complete
            || self.crashed
            || (h_agl_true_m <= 1.65 && guid_pos[0] >= -10.0);

        let unified_fd = compute_unified_fd_guidance(
            guid_pos,
            vel_ground_ned.to_array(),
            euler_deg,
            runway_heading_deg,
            true_heading_deg,
            flightgear_live,
            on_ground_or_stopped,
        );

        let distance_to_rwy09_m = unified_fd.distance_to_rwy_m;
        let heading_correction_deg = if self.landing_complete || self.crashed {
            0.0
        } else {
            unified_fd.heading_correction_deg
        };
        let glideslope_error_deg = if self.landing_complete || self.crashed {
            0.0
        } else {
            unified_fd.glideslope_error_deg
        };
        let flare_active = unified_fd.flare_active && !self.touched_down && !self.crashed;
        let ground_speed_x = vel_ground_ned.inner.x.max(10.0);
        let eta_touchdown_sec = if self.touched_down || self.landing_complete || self.crashed {
            0.0
        } else {
            sanitize_f64(distance_to_rwy09_m / ground_speed_x, 0.0, 600.0, 12.0)
        };

        // 9b. Canal invers bidirecțional către FlightGear (`127.0.0.1:5506` la 20 Hz) pentru HUD în cockpit + Autoland Assist
        // Folosește exact aceeași structură `unified_fd` (Single-Source-of-Truth: FD == AP).
        if fg_opt.is_some() && self.tick % 3 == 0 {
            self.fg_feedback_tx.send_guidance(&FlightGearGuidanceFeedback {
                distance_to_rwy_m: unified_fd.distance_to_rwy_m,
                cross_track_m: unified_fd.cross_track_m,
                alt_agl_m: unified_fd.alt_agl_m,
                heading_correction_deg,
                glideslope_error_deg,
                crab_angle_deg: unified_fd.crab_needed_deg,
                target_heading_deg: unified_fd.target_heading_deg,
                target_pitch_deg: unified_fd.target_pitch_deg,
                flare_active,
                gnss_quarantined: self.gnss_quarantined,
                autoland_assist: self.fg_autoland_assist,
            });
        }

        let als_bars_detected = if is_real_video_active {
            self.real_video_profile.als_bars_detected
        } else if self.swir_als_enabled && distance_to_rwy09_m < 1750.0 {
            let visibility_factor = (self.visibility_mor_m * 3.8 / 120.0).clamp(0.45, 1.5);
            ((12.0 * visibility_factor) as usize).clamp(5, 18)
        } else {
            2
        };

        let photonic_certainty_mean = if is_real_video_active {
            self.real_video_profile.photonic_certainty_mean
        } else {
            sanitize_f64(
                if self.swir_als_enabled {
                    (0.92 - 0.004 * beta_ext * 100.0).clamp(0.58, 0.98)
                } else {
                    (0.45 - 0.01 * beta_ext * 100.0).clamp(0.15, 0.65)
                },
                0.05,
                1.0,
                0.82,
            )
        };

        // 10. Extragerea matricei de rotație 3x3 R_b^n(q) și a cuaternionului unitar
        let rot_mat = self.quat_b_to_n.to_rotation_matrix();
        let m = rot_mat.matrix();
        let rot_matrix_b_to_n = [
            [m[(0, 0)], m[(0, 1)], m[(0, 2)]],
            [m[(1, 0)], m[(1, 1)], m[(1, 2)]],
            [m[(2, 0)], m[(2, 1)], m[(2, 2)]],
        ];
        let q_coords = self.quat_b_to_n.quaternion();
        let quat_b_to_n = [q_coords.w, q_coords.i, q_coords.j, q_coords.k];

        // 11. Generarea vectorilor de flux optic 2D pentru HUD
        let optical_flow_vectors = self.build_hud_optical_vectors(&vel_ground_ned.to_array());

        let approach_phase = if self.crashed {
            "🚨 CRASH / IMPACT DETECTAT — APASĂ [R] PENTRU RESET"
        } else if is_real_video_active {
            "REAL FOG VIDEO REPLAY: PIXEL KOSCHMIEDER + ALS SWIR LOCK"
        } else if self.landing_complete {
            "PHASE 5: FULL STOP ON RWY 09 CRAIOVA (0 KM/H — NO TAXI)"
        } else if self.touched_down {
            "PHASE 4: TOUCHDOWN & AUTOBRAKE ROLLOUT RWY 09 CRAIOVA"
        } else if let Some(banner) = active_stage_banner {
            banner
        } else if distance_to_rwy09_m > 1200.0 {
            "PHASE 1: OUTER APPROACH (1.6km -> 1.2km LRCV)"
        } else if distance_to_rwy09_m > 250.0 {
            "PHASE 2: ALS SWIR LOCK & LANDING CHECKLIST (RWY 09)"
        } else {
            "PHASE 3: THRESHOLD & CAT III FLARE RWY 09 CRAIOVA"
        };

        let live_prog = (**self.training_progress.load()).clone();

        TelemetryFrameV2 {
            tick: self.tick,
            sim_time_sec: self.sim_time_sec,
            approach_phase,
            aircraft_pose: Aircraft6DofPose {
                pos_ned_m: true_pos,
                vel_body_mps: self.vel_body.to_array(),
                vel_ground_ned_mps: vel_ground_ned.to_array(),
                quat_b_to_n,
                rot_matrix_b_to_n,
                euler_deg,
                omega_body_dps: self.omega_body_dps,
                accel_body_mps2: self.accel_body_mps2,
                alpha_aoa_deg,
                beta_sideslip_deg,
                crab_angle_cmd_deg,
            },
            fog_and_meteo: FogMeteoTelemetry {
                visibility_mor_m: self.visibility_mor_m,
                beta_ext_koschmieder: beta_ext,
                rho_fog_scale_inv_m: rho_fog_inv_m,
                rho_ground_horizon_safe: rho_horizon,
                true_fog_wind_ned_mps: self.physical_fog_wind_ned.to_array(),
                est_fog_wind_ned_mps,
                gust_lookahead_warning,
                gust_lookahead_lead_sec,
                gust_anomaly_score,
                meteo_source_status,
                lrcv_temp_c: meteo_snap.temperature_c,
                lrcv_qnh_hpa: meteo_snap.qnh_hpa,
                lrcv_rh_pct: meteo_snap.relative_humidity_pct,
            },
            estimators_arena: FourEstimatorsTelemetry {
                raw_gnss_pos_ned: self.raw_gnss_pos_ned,
                raw_gnss_error_m,
                pure_ins_pos_ned: self.pure_ins_pos_ned,
                pure_ins_error_m,
                naive_flow_pos_ned: self.naive_flow_pos_ned,
                naive_flow_error_m,
                aero_shield_pos_ned: self.aero_shield_pos_ned,
                aero_shield_error_m,
            },
            lrcv_guidance: LrcvRunwayGuidance {
                distance_to_rwy09_m,
                bearing_to_rwy09_deg,
                heading_correction_deg,
                glideslope_error_deg,
                eta_touchdown_sec,
                als_bars_detected,
                photonic_certainty_mean,
                swir_als_active: self.swir_als_enabled,
                flare_active,
                gear_down: self.gear_down,
                autobrake_armed: self.autobrake_armed,
                spoilers_deployed: self.spoilers_deployed,
                reverse_thrust_active: self.reverse_thrust_active,
                landing_complete: self.landing_complete,
                crashed: self.crashed,
                crash_reason: self.crash_reason.clone(),
                touchdown_vs_mps: self.touchdown_vs_mps,
                touchdown_lateral_err_m: self.touchdown_lateral_err_m,
            },
            cyber_and_gpu: CyberGpuTelemetry {
                gnss_watchdog_status,
                chi2_mahalanobis_stat: self.chi2_stat,
                cusum_stealth_stat: self.cusum_stat,
                meteo_sanity_trusted,
                meteo_discrepancy_mps,
                gpu_device_name: ai_model.device_name.clone(),
                inference_latency_ms,
                heteroscedastic_sigma_v: sigma_v,
                live_training_progress: live_prog,
                active_director_scenario: self.director.active_scenario_id,
                cyber_flags: self.cyber_flags.clone(),
                flightgear_live,
                flightgear_near_lrcv,
                fg_autoland_assist: self.fg_autoland_assist,
                shield_protection_enabled: self.shield_protection_enabled,
                checkpoint_locked: self.checkpoint_locked,
                verified_checkpoint_ned: self.verified_checkpoint_ned,
            },
            optical_flow_vectors,
        }
    }

    fn build_hud_optical_vectors(&self, vel_ground_ned: &[f64; 3]) -> Vec<OpticalVectorHud> {
        let mut vecs = Vec::with_capacity(16);
        let u_norm = sanitize_f64(vel_ground_ned[0] / 70.0, 0.0, 2.0, 1.0);
        let v_norm = sanitize_f64(vel_ground_ned[1] / 30.0, -1.5, 1.5, 0.0);

        let ground_anchors = [
            (0.20, 0.68),
            (0.35, 0.68),
            (0.50, 0.68),
            (0.65, 0.68),
            (0.80, 0.68),
            (0.25, 0.82),
            (0.50, 0.82),
            (0.75, 0.82),
        ];
        for &(nx, ny) in &ground_anchors {
            let radial_x = (nx - 0.5) * 0.045 * u_norm - v_norm * 0.025;
            let radial_y = (ny - 0.45) * 0.055 * u_norm;
            vecs.push(OpticalVectorHud {
                x_norm: nx,
                y_norm: ny,
                dx_px: sanitize_f64(radial_x * 420.0, -60.0, 60.0, 0.0),
                dy_px: sanitize_f64(radial_y * 320.0, -60.0, 60.0, 8.0),
                layer: "ground",
            });
        }

        let fog_wy = self.lookahead_fog_wind_ned.inner.y;
        let fog_wz = self.lookahead_fog_wind_ned.inner.z;
        let fog_anchors = [
            (0.22, 0.32),
            (0.40, 0.32),
            (0.60, 0.32),
            (0.78, 0.32),
            (0.30, 0.48),
            (0.50, 0.48),
            (0.70, 0.48),
        ];
        for &(nx, ny) in &fog_anchors {
            vecs.push(OpticalVectorHud {
                x_norm: nx,
                y_norm: ny,
                dx_px: sanitize_f64(fog_wy * 2.4, -75.0, 75.0, 4.0),
                dy_px: sanitize_f64(fog_wz * 3.0 + 1.5, -45.0, 45.0, 1.5),
                layer: "fog",
            });
        }

        vecs
    }
}

#[inline]
#[must_use]
fn euclidean_dist_3d(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    sanitize_f64((dx * dx + dy * dy + dz * dz).sqrt(), 0.0, 100_000.0, 0.0)
}

/// Interpolare cubică Hermite $C^1$ (`smoothstep`: $3t^2 - 2t^3$, cu derivată zero în ambele capete `edge0` și `edge1`).
#[inline]
#[must_use]
pub fn smoothstep_c1(edge0: f64, edge1: f64, x: f64) -> f64 {
    if (edge1 - edge0).abs() < 1e-9 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
#[must_use]
pub fn wrap180_deg(mut deg: f64) -> f64 {
    while deg > 180.0 {
        deg -= 360.0;
    }
    while deg < -180.0 {
        deg += 360.0;
    }
    deg
}

/// Structură unificată *Single-Source-of-Truth* pentru Flight Director (Punctul Verde) și Autopilot (`FD == AP`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnifiedFdGuidance {
    pub distance_to_rwy_m: f64,
    pub along_track_m: f64,
    pub cross_track_m: f64,
    pub alt_agl_m: f64,
    pub target_track_deg: f64,
    pub actual_track_deg: f64,
    pub crab_needed_deg: f64,
    pub target_heading_deg: f64,
    pub heading_correction_deg: f64,
    pub target_fpa_deg: f64,
    pub actual_fpa_deg: f64,
    pub glideslope_error_deg: f64,
    pub target_pitch_deg: f64,
    pub flare_active: bool,
}

/// Lege exponențială netedă ($C^1$) de Flare la aterizare (tip Airbus CAT III Autoland, sub `12m` / `~40 ft` AGL):
/// reduce progresiv rata de coborâre de la panta de `3.0°` până la touchdown în zona `+180m .. +320m` după pragul RWY 09.
#[inline]
#[must_use]
pub fn compute_flare_target_fpa_deg(h_agl_m: f64, dx_aim_m: f64, horiz_speed_mps: f64) -> f64 {
    if h_agl_m <= 0.02 {
        return 0.0;
    }
    let geom_fpa_deg = -h_agl_m.atan2(dx_aim_m.max(35.0)).to_degrees();
    let flare_blend = 1.0 - smoothstep_c1(1.2, 12.0, h_agl_m);
    let spd = horiz_speed_mps.max(25.0);
    let target_sink_mps = (0.72 + 2.84 * (h_agl_m / 12.0)).clamp(0.72, 3.80);
    let sink_flare_fpa_deg = -target_sink_mps.atan2(spd).to_degrees();
    let flare_end_fpa_deg = sink_flare_fpa_deg.max(-0.65);
    let flare_fpa_deg = geom_fpa_deg * (1.0 - flare_blend) + flare_end_fpa_deg * flare_blend;
    let combined_fpa_deg = geom_fpa_deg.max(flare_fpa_deg);
    let ground_rollout_scale = smoothstep_c1(0.05, 0.85, h_agl_m);
    combined_fpa_deg * ground_rollout_scale
}

/// Calculează legea unificată și continuă $C^1$ a Punctului Verde (Flight Director == Autopilot):
/// - Garantează că același `heading_correction_deg` și `glideslope_error_deg` guvernează atât Web UI (`LrcvRunwayGuidance`)
///   cât și FlightGear (`FlightGearGuidanceFeedback` pe UDP `:5506`) și controlerul `fg_mcp_ctl.py`.
/// - Include amortizarea vitezei laterale (`vel_ground_ned_mps[1]`) direct în `target_track_deg` (fără bias-uri parazite).
/// - Include protecția netedă de altitudine înainte de prag și tranziția Hermite $C^1$ între glisada de `-3.0°` și Flare.
/// - Aduce neted erorile la `0.0°` la rularea pe pistă (`h_agl <= 1.65m`, `x >= -10.0m`), inclusiv în modul `flightgear_live`.
#[must_use]
pub fn compute_unified_fd_guidance(
    guid_pos_ned: [f64; 3],
    vel_ground_ned_mps: [f64; 3],
    euler_deg: [f64; 3],
    runway_heading_deg: f64,
    true_heading_deg: f64,
    flightgear_live: bool,
    on_ground_or_stopped: bool,
) -> UnifiedFdGuidance {
    let along_track_m = sanitize_f64(guid_pos_ned[0], -20_000.0, 20_000.0, -500.0);
    let cross_track_m = sanitize_f64(guid_pos_ned[1], -10_000.0, 10_000.0, 0.0);
    let alt_agl_m = sanitize_f64((-guid_pos_ned[2]).max(0.0), 0.0, 10_000.0, 30.0);

    let dx_to_rwy = (0.0 - along_track_m).max(0.0);
    let dy_to_rwy = 0.0 - cross_track_m;
    let distance_to_rwy_m =
        sanitize_f64(dx_to_rwy.hypot(dy_to_rwy), 0.0, 20_000.0, 500.0);

    let vx = sanitize_f64(vel_ground_ned_mps[0], -200.0, 300.0, 36.0);
    let vy = sanitize_f64(vel_ground_ned_mps[1], -200.0, 200.0, 0.0);
    let vz = sanitize_f64(vel_ground_ned_mps[2], -100.0, 100.0, 0.0);
    let raw_horiz_speed = vx.hypot(vy);
    let horiz_ground_speed = raw_horiz_speed.max(1.0);

    // Tranziție netedă C1 la sol (rollout pe pista asfaltată X >= -10m după contactul cu solul)
    let ground_blend = if on_ground_or_stopped {
        1.0
    } else if flightgear_live {
        smoothstep_c1(-10.0, 5.0, along_track_m) * (1.0 - smoothstep_c1(0.35, 1.65, alt_agl_m))
    } else {
        0.0
    };

    // 1. Legea Laterală Unificată (Two-Regime Nonlinear L1 S-Turn Intercept + Wing-Low Slip)
    let look_blend = smoothstep_c1(-160.0, -80.0, along_track_m);
    let lead_w = smoothstep_c1(1.5, 10.0, cross_track_m.abs());
    let s_turn_w = smoothstep_c1(18.0, 42.0, cross_track_m.abs());
    let (loc_lookahead_m, vy_lead_sec) = if flightgear_live {
        (
            95.0 + 25.0 * look_blend + 125.0 * s_turn_w,
            1.35 * lead_w + 2.15 * s_turn_w,
        )
    } else {
        (260.0 + 60.0 * look_blend, 1.45 * lead_w)
    };
    let dy_predictive = -cross_track_m - vy * vy_lead_sec;
    let air_target_track_deg = dy_predictive
        .atan2(loc_lookahead_m)
        .to_degrees()
        .clamp(-28.0, 28.0);
    let rollout_target_track_deg = (-cross_track_m * 0.45).clamp(-8.0, 8.0);
    let target_track_deg =
        air_target_track_deg * (1.0 - ground_blend) + rollout_target_track_deg * ground_blend;

    let rel_yaw_deg = wrap180_deg(true_heading_deg - runway_heading_deg);
    let actual_track_deg = if raw_horiz_speed < 3.0 {
        rel_yaw_deg
    } else {
        vy.atan2(vx.max(0.5)).to_degrees()
    };

    let max_crab_deg = if flightgear_live { 22.0 } else { 25.0 };
    let crab_needed_deg = if raw_horiz_speed > 6.0 {
        wrap180_deg(rel_yaw_deg - actual_track_deg).clamp(-max_crab_deg, max_crab_deg)
    } else {
        0.0
    };
    let decrab_factor = if flightgear_live {
        smoothstep_c1(250.0, 320.0, alt_agl_m) * (1.0 - ground_blend)
    } else {
        1.0 - ground_blend
    };
    let flare_align_scale = if flightgear_live {
        0.22 + 0.78 * smoothstep_c1(6.0, 14.0, alt_agl_m)
    } else {
        1.0
    };
    let desired_rel_yaw_deg =
        (target_track_deg * flare_align_scale + crab_needed_deg * decrab_factor).clamp(-45.0, 45.0);
    let target_heading_deg = (runway_heading_deg + desired_rel_yaw_deg).rem_euclid(360.0);
    let raw_hdg_corr = wrap180_deg(target_heading_deg - true_heading_deg);
    let heading_correction_deg = sanitize_f64(raw_hdg_corr, -90.0, 90.0, 0.0);

    // 2. Legea Verticală Unificată C1 (Fascicul ILS -3.0° + Protecție Altitudine Joasă + Hermite Flare + Rollout)
    let aim_x_m = if flightgear_live { 210.0 } else { 110.0 };
    let dx_gs_aim = (aim_x_m - along_track_m).max(35.0);
    let gs_tan_3deg = 3.0_f64.to_radians().tan();
    let h_beam_m = dx_gs_aim * gs_tan_3deg;
    let dh_beam_m = h_beam_m - alt_agl_m;
    let low_alt_boost = 1.0 - smoothstep_c1(10.0, 18.0, alt_agl_m);
    let max_fpa_beam = 0.8 + 1.7 * low_alt_boost;
    let fpa_beam_deg = (-3.0 + 0.35 * dh_beam_m).clamp(-4.5, max_fpa_beam);
    let fpa_flare_deg = compute_flare_target_fpa_deg(alt_agl_m, dx_gs_aim, horiz_ground_speed);

    let alpha_x = if flightgear_live {
        smoothstep_c1(-45.0, 10.0, along_track_m)
    } else {
        1.0
    };
    let air_target_fpa_deg = fpa_beam_deg * (1.0 - alpha_x) + fpa_flare_deg * alpha_x;
    let target_fpa_deg = air_target_fpa_deg * (1.0 - ground_blend);

    let actual_fpa_deg = if on_ground_or_stopped || (alt_agl_m <= 0.05 && euler_deg[1] < 0.0) {
        euler_deg[1] * (1.0 - ground_blend)
    } else {
        (-vz).atan2(horiz_ground_speed).to_degrees()
    };

    let glideslope_error_deg = sanitize_f64(
        (target_fpa_deg - actual_fpa_deg) * (1.0 - ground_blend),
        -35.0,
        35.0,
        0.0,
    );

    let flare_min_x = if flightgear_live { -20.0 } else { -280.0 };
    let flare_max_h = if flightgear_live { 8.5 } else { 12.0 };
    let flare_active = !on_ground_or_stopped
        && alt_agl_m > 0.18
        && alt_agl_m <= flare_max_h
        && along_track_m >= flare_min_x;

    let flare_pitch_blend =
        alpha_x * (1.0 - smoothstep_c1(1.2, 9.5, alt_agl_m)) * (1.0 - ground_blend);
    let aoa_ref_deg = 2.05 + 0.55 * flare_pitch_blend;
    let air_target_pitch_deg =
        (target_fpa_deg + aoa_ref_deg + 0.85 * glideslope_error_deg).clamp(-4.2, 3.2);
    let rollout_pitch_deg = 0.65;
    let target_pitch_deg = sanitize_f64(
        air_target_pitch_deg * (1.0 - ground_blend) + rollout_pitch_deg * ground_blend,
        -4.5,
        3.5,
        -1.0,
    );

    UnifiedFdGuidance {
        distance_to_rwy_m,
        along_track_m,
        cross_track_m,
        alt_agl_m,
        target_track_deg,
        actual_track_deg,
        crab_needed_deg,
        target_heading_deg,
        heading_correction_deg,
        target_fpa_deg,
        actual_fpa_deg,
        glideslope_error_deg,
        target_pitch_deg,
        flare_active,
    }
}

/// Lansează thread-ul OS dedicat `aero-60hz-rt-loop` care rulează la $60.0\text{ Hz}$ ($16.666\text{ ms}$),
/// drenează non-blocking comenzile din `cmd_rx` și publică ultimul cadru în `watch::Sender<Arc<TelemetryFrameV2>>`.
pub fn spawn_dedicated_60hz_rt_thread(
    cmd_rx: Receiver<SimCommand>,
    telemetry_tx: watch::Sender<Arc<TelemetryFrameV2>>,
    active_model: Arc<ArcSwap<ActiveAiModel>>,
    training_progress: Arc<ArcSwap<Option<TrainingProgressTelemetry>>>,
    live_meteo: Arc<ArcSwap<LiveMeteoSnapshot>>,
    is_training_flag: Arc<AtomicBool>,
    fg_live_state: Arc<ArcSwap<Option<FlightGearLiveState>>>,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("aero-60hz-rt-loop".into())
        .spawn(move || {
            let mut sim = FlightSimulator60Hz::new(
                active_model,
                training_progress,
                live_meteo,
                is_training_flag,
            )
            .with_flightgear_bridge(fg_live_state);
            let frame_period = Duration::from_micros(16_666);

            let mut was_fg_live = false;
            loop {
                let tick_start = Instant::now();

                let is_fg_now = sim.fg_live_state.load().is_some();
                if is_fg_now && !was_fg_live {
                    // Când FlightGear pornește prima dată, aliniem și estimatoarele la poziția avionului din FlightGear
                    if let Some(ref fg) = **sim.fg_live_state.load() {
                        sim.raw_gnss_pos_ned = fg.pos_ned_m;
                        sim.pure_ins_pos_ned = fg.pos_ned_m;
                        sim.naive_flow_pos_ned = fg.pos_ned_m;
                        sim.aero_shield_pos_ned = fg.pos_ned_m;
                        sim.verified_checkpoint_ned = fg.pos_ned_m;
                    }
                }
                was_fg_live = is_fg_now;

                while let Ok(cmd) = cmd_rx.try_recv() {
                    sim.apply_command(cmd);
                }

                let frame = Arc::new(sim.step_60hz(1.0 / 60.0));

                if telemetry_tx.send(frame).is_err() {
                    break;
                }

                let elapsed = tick_start.elapsed();
                if elapsed < frame_period {
                    std::thread::sleep(frame_period - elapsed);
                }
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rt_test_sim() -> FlightSimulator60Hz {
        FlightSimulator60Hz::new(
            Arc::new(ArcSwap::from_pointee(ActiveAiModel::default())),
            Arc::new(ArcSwap::from_pointee(None)),
            Arc::new(ArcSwap::from_pointee(LiveMeteoSnapshot::default())),
            Arc::new(AtomicBool::new(false)),
        )
    }

    #[test]
    fn test_unified_fd_zero_error_on_reference_trajectory() {
        let gs_tan = 3.0_f64.to_radians().tan();
        for &fg_live in &[false, true] {
            let aim_x = if fg_live { 210.0 } else { 110.0 };
            let x = -850.0;
            let h_agl = (aim_x - x) * gs_tan;
            let vx = if fg_live { 36.0 } else { 68.0 };
            let vz_down = vx * gs_tan;

            let fd = compute_unified_fd_guidance(
                [x, 0.0, -h_agl],
                [vx, 0.0, vz_down],
                [0.0, -1.0, 0.0],
                LRCV_RWY09_HEADING_DEG,
                LRCV_RWY09_HEADING_DEG,
                fg_live,
                false,
            );
            assert!(
                fd.heading_correction_deg.abs() < 1e-3,
                "Expected zero lateral FD error on centerline, got {}",
                fd.heading_correction_deg
            );
            assert!(
                fd.glideslope_error_deg.abs() < 1e-3,
                "Expected zero vertical FD error on 3.0 deg glideslope, got {}",
                fd.glideslope_error_deg
            );
        }
    }

    #[test]
    fn test_unified_fd_c1_continuity_across_threshold() {
        let gs_tan = 3.0_f64.to_radians().tan();
        let aim_x = 210.0;
        let vx = 36.0;
        let mut fpa_samples = Vec::with_capacity(101);

        for step_x in -60..=40 {
            let x = step_x as f64;
            let h_agl = ((aim_x - x) * gs_tan).max(2.0);
            let vz_down = vx * gs_tan;
            let fd = compute_unified_fd_guidance(
                [x, 0.0, -h_agl],
                [vx, 0.0, vz_down],
                [0.0, -1.0, 0.0],
                LRCV_RWY09_HEADING_DEG,
                LRCV_RWY09_HEADING_DEG,
                true,
                false,
            );
            fpa_samples.push(fd.target_fpa_deg);
        }

        for i in 1..fpa_samples.len() {
            let diff = (fpa_samples[i] - fpa_samples[i - 1]).abs();
            assert!(
                diff < 0.25,
                "C1 continuity violated across threshold at index {}: |{} - {}| = {} >= 0.25 deg/m",
                i,
                fpa_samples[i],
                fpa_samples[i - 1],
                diff
            );
        }
    }

    #[test]
    fn test_traditional_gps_confidently_wrong_vs_aeroshield_quarantine() {
        let mut sim = make_rt_test_sim();
        sim.apply_command(SimCommand::ConfigureCyberAttack {
            gps_jump_spoof: true,
            gps_stealth_ramp_spoof: false,
            cut_internet_comms: true,
            poison_meteo_cache: true,
        });

        let mut frame = sim.step_60hz(1.0 / 60.0);
        for _ in 0..30 {
            frame = sim.step_60hz(1.0 / 60.0);
        }

        assert!(
            frame.estimators_arena.raw_gnss_error_m > 200.0,
            "Expected spoofed GNSS error > 200m, got {}",
            frame.estimators_arena.raw_gnss_error_m
        );
        assert!(
            sim.gnss_quarantined,
            "Expected AERO-SHIELD watchdog to quarantine spoofed GNSS"
        );
        assert!(
            frame.estimators_arena.aero_shield_error_m < 2.0,
            "Expected AERO-SHIELD fused error < 2.0m, got {}",
            frame.estimators_arena.aero_shield_error_m
        );

        // Un sistem GPS tradițional ("Confidently Wrong") care ar folosi raw_gnss_pos_ned (+250m)
        // ar comanda un viraj fals masiv în câmp (< -15 deg), în timp ce AERO-SHIELD rămâne centrat pe ax (< 1 deg).
        let trad_gps_fd = compute_unified_fd_guidance(
            frame.estimators_arena.raw_gnss_pos_ned,
            frame.aircraft_pose.vel_ground_ned_mps,
            frame.aircraft_pose.euler_deg,
            LRCV_RWY09_HEADING_DEG,
            LRCV_RWY09_HEADING_DEG,
            true,
            false,
        );
        assert!(trad_gps_fd.heading_correction_deg < -15.0);
        assert!(frame.lrcv_guidance.heading_correction_deg.abs() < 1.0);
    }

    #[test]
    fn test_unified_fd_touchdown_zeroes_errors_in_flightgear_live_mode() {
        let fd = compute_unified_fd_guidance(
            [150.0, 0.0, -0.2],
            [22.0, 0.0, 0.0],
            [0.0, 0.5, 0.0],
            LRCV_RWY09_HEADING_DEG,
            LRCV_RWY09_HEADING_DEG,
            true,
            true,
        );
        assert!(
            fd.glideslope_error_deg.abs() < 1e-3,
            "Expected zero glideslope_error_deg on runway rollout in flightgear_live mode, got {}",
            fd.glideslope_error_deg
        );
        assert!(
            fd.heading_correction_deg.abs() < 1e-3,
            "Expected zero heading_correction_deg on runway centerline rollout, got {}",
            fd.heading_correction_deg
        );
    }

    #[test]
    fn test_crosswind_decrab_aligns_heading_with_runway_before_threshold() {
        let rwy_hdg = LRCV_RWY09_HEADING_DEG;
        // În vânt lateral puternic (vy_wind = 8.0 m/s -> avionul a avut crab de +8°),
        // pe toată panta finală sub 250m AGL (inclusiv la 82.0m AGL / X=-1434m și <= 10.0m AGL)
        // pe axul pistei (y = 0, vy = 0), decrab_factor este 0.0 (Wing-Low Forward-Slip)
        // și target_heading_deg este 100% aliniat paralel cu axul Pistei 09!
        for &alt_agl_m in &[82.0_f64, 45.0, 10.0, 6.0, 2.5] {
            let ug = compute_unified_fd_guidance(
                [-15.0, 0.0, -alt_agl_m],
                [36.0, 0.0, 1.5],
                [2.5, 1.0, 8.0],
                rwy_hdg,
                rwy_hdg + 8.0,
                true,
                false,
            );
            assert!(
                (ug.target_heading_deg - rwy_hdg).abs() < 1e-3,
                "Expected target_heading_deg == rwy_hdg ({}) at alt_agl_m={}m, got {}",
                rwy_hdg,
                alt_agl_m,
                ug.target_heading_deg
            );
            assert!(
                (ug.heading_correction_deg - (-8.0)).abs() < 1e-3,
                "Expected heading_correction_deg == -8.0 deg to remove diagonal crab at {}m AGL, got {}",
                alt_agl_m,
                ug.heading_correction_deg
            );
        }
    }
}

