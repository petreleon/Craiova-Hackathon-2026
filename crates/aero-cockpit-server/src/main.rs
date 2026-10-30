//! Punctul de intrare principal pentru `aero-cockpit-server` (AERO-SHIELD LRCV V2.0).
//! Lansează:
//! 1. Opțional job-ul CLI `--train-gpu` (`candle-core` + `candle-nn` + `.safetensors`),
//! 2. Thread-ul OS Real-Time dedicat `aero-60hz-rt-loop` ($60.0\text{ Hz}$, $16.666\text{ ms}$),
//! 3. Poller-ul async Open-Meteo pentru Aeroportul Internațional Craiova (`LRCV`, `44.3181°N, 23.8886°E`),
//! 4. Serverul HTTP + WebSocket `axum` cu *Zero-Lag State Conflation* (`tokio::sync::watch`).

use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use tokio::sync::watch;
use tracing::{error, info};

use aero_cockpit_server::cli_train::{run_candle_fine_tuning_job, ActiveAiModel};
use aero_cockpit_server::rt_loop_60hz::{
    spawn_dedicated_60hz_rt_thread, FlightSimulator60Hz, LRCV_RWY09_LAT_DEG, LRCV_RWY09_LON_DEG,
};
use aero_cockpit_server::ws_telemetry::{
    build_cockpit_router, spawn_lrcv_open_meteo_poller, CockpitServerState,
};
use aero_cockpit_server::{LiveMeteoSnapshot, SimCommand, TrainingProgressTelemetry};

#[tokio::main]
async fn main() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(env_filter).try_init();

    let args: Vec<String> = std::env::args().collect();
    let run_cli_train = args.iter().any(|a| a == "--train-gpu" || a == "--train");
    let train_only = args.iter().any(|a| a == "--train-only");

    let active_model = Arc::new(ArcSwap::from_pointee(ActiveAiModel::default()));
    let training_progress: Arc<ArcSwap<Option<TrainingProgressTelemetry>>> =
        Arc::new(ArcSwap::from_pointee(None));
    let live_meteo = Arc::new(ArcSwap::from_pointee(LiveMeteoSnapshot::default()));
    let is_training_flag = Arc::new(AtomicBool::new(false));

    info!(
        "AERO-SHIELD LRCV V2.0 starting — Target Threshold: Craiova RWY 09 ({:.4}N, {:.4}E)",
        LRCV_RWY09_LAT_DEG, LRCV_RWY09_LON_DEG
    );
    info!(
        "Compute Engine initialized: {}",
        active_model.load().device_name
    );

    if run_cli_train {
        info!("Starting CLI GPU/SIMD Physics-Informed Fine-Tuning (Candle + SafeTensors)...");
        let weights_path = Path::new("aero_shield_v2_lrcv.safetensors");
        match run_candle_fine_tuning_job(
            80,
            Duration::from_millis(10),
            Some(weights_path),
            &active_model,
            &training_progress,
        ) {
            Ok(trained) => {
                info!(
                    "Training complete on {} | Generation {} | Final Heteroscedastic Loss: {:.5}",
                    trained.device_name, trained.generation, trained.final_loss
                );
            }
            Err(err) => {
                error!("Training job encountered error (continuing with baseline weights): {}", err);
            }
        }

        if train_only {
            info!("--train-only flag detected; exiting cleanly after SafeTensors export.");
            return;
        }
    }

    // Inițializăm canalul de comenzi lock-free crossbeam și canalul watch de conflație zero-lag
    let (cmd_tx, cmd_rx) = crossbeam_channel::bounded::<SimCommand>(256);

    let mut boot_sim = FlightSimulator60Hz::new(
        Arc::clone(&active_model),
        Arc::clone(&training_progress),
        Arc::clone(&live_meteo),
        Arc::clone(&is_training_flag),
    );
    let initial_frame = Arc::new(boot_sim.step_60hz(1.0 / 60.0));
    let (telemetry_tx, telemetry_rx) = watch::channel(initial_frame);

    // 1. Lansăm thread-ul OS dedicat aero-60hz-rt-loop
    if let Err(err) = spawn_dedicated_60hz_rt_thread(
        cmd_rx,
        telemetry_tx,
        Arc::clone(&active_model),
        Arc::clone(&training_progress),
        Arc::clone(&live_meteo),
        Arc::clone(&is_training_flag),
    ) {
        error!("Failed to spawn aero-60hz-rt-loop thread: {}", err);
        return;
    }

    // 2. Lansăm poller-ul async Open-Meteo pentru Craiova LRCV (timeout strict 1500ms)
    spawn_lrcv_open_meteo_poller(Arc::clone(&live_meteo));

    // 3. Construim și pornim serverul Axum pe 0.0.0.0:8080 (sau variabila de mediu PORT)
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(8080);
    let bind_addr = SocketAddr::from(([0, 0, 0, 0], port));

    let app_state = CockpitServerState {
        cmd_tx,
        telemetry_rx,
        live_meteo,
    };
    let app = build_cockpit_router(app_state);

    info!(
        "AERO-SHIELD 3D/HUD Cyber-Cockpit Server listening on http://{}",
        bind_addr
    );

    match tokio::net::TcpListener::bind(bind_addr).await {
        Ok(listener) => {
            if let Err(err) = axum::serve(listener, app).await {
                error!("Axum server terminated with error: {}", err);
            }
        }
        Err(err) => {
            error!("Failed to bind TCP listener on {}: {}", bind_addr, err);
        }
    }
}
