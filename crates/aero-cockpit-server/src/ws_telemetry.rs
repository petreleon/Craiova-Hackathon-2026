//! Serverul Axum HTTP + WebSocket cu *Zero-Lag State Conflation* (`tokio::sync::watch`)
//! și task-ul async de ingestie meteo `LRCV` (`44.3181° N, 23.8886° E`) cu timeout strict de 1500 ms.

use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::{Json, Router};
use crossbeam_channel::Sender;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tower_http::cors::CorsLayer;

use crate::rt_loop_60hz::{LRCV_RWY09_LAT_DEG, LRCV_RWY09_LON_DEG};
use crate::sim_commands::{sanitize_f64, SimCommand};
use crate::{LiveMeteoSnapshot, TelemetryFrameV2};

/// HTML-ul static încapsulat în binar (`include_str!`) pentru funcționare 100% autonomă.
pub const EMBEDDED_INDEX_HTML: &str = include_str!("../static/index.html");

/// Starea partajată a serverului Axum.
#[derive(Clone)]
pub struct CockpitServerState {
    pub cmd_tx: Sender<SimCommand>,
    pub telemetry_rx: watch::Receiver<Arc<TelemetryFrameV2>>,
    pub live_meteo: Arc<ArcSwap<LiveMeteoSnapshot>>,
}

/// Răspuns JSON standard pentru `/api/health`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub airport_icao: &'static str,
    pub runway_threshold: &'static str,
    pub rwy09_lat_deg: f64,
    pub rwy09_lon_deg: f64,
    pub current_tick: u64,
    pub gpu_device: String,
}

/// Răspuns JSON standard pentru `/api/command`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandAckResponse {
    pub accepted: bool,
    pub message: &'static str,
}

/// Construiește router-ul Axum complet pentru `aero-cockpit-server`.
pub fn build_cockpit_router(state: CockpitServerState) -> Router {
    Router::new()
        .route("/", get(serve_index_html))
        .route("/index.html", get(serve_index_html))
        .route("/ws/telemetry", get(ws_telemetry_handler))
        .route("/api/command", post(post_sim_command_handler))
        .route("/api/health", get(get_health_handler))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn serve_index_html() -> Html<&'static str> {
    Html(EMBEDDED_INDEX_HTML)
}

async fn get_health_handler(State(state): State<CockpitServerState>) -> Json<HealthResponse> {
    let latest = state.telemetry_rx.borrow().clone();
    Json(HealthResponse {
        status: "ONLINE_60HZ_DO178C",
        airport_icao: "LRCV (Craiova International Airport)",
        runway_threshold: "RWY 09",
        rwy09_lat_deg: LRCV_RWY09_LAT_DEG,
        rwy09_lon_deg: LRCV_RWY09_LON_DEG,
        current_tick: latest.tick,
        gpu_device: latest.cyber_and_gpu.gpu_device_name.clone(),
    })
}

async fn post_sim_command_handler(
    State(state): State<CockpitServerState>,
    Json(cmd): Json<SimCommand>,
) -> (StatusCode, Json<CommandAckResponse>) {
    let clean_cmd = cmd.sanitize();
    match state.cmd_tx.try_send(clean_cmd) {
        Ok(()) => (
            StatusCode::OK,
            Json(CommandAckResponse {
                accepted: true,
                message: "SimCommand queued for next 60Hz frame (<16.6ms)",
            }),
        ),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(CommandAckResponse {
                accepted: false,
                message: "Command channel full or closed",
            }),
        ),
    }
}

async fn ws_telemetry_handler(
    ws: WebSocketUpgrade,
    State(state): State<CockpitServerState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_client(socket, state))
}

async fn handle_ws_client(mut socket: WebSocket, state: CockpitServerState) {
    let mut rx = state.telemetry_rx.clone();
    let cmd_tx = state.cmd_tx.clone();

    // Trimitem imediat primul cadru disponibil la conectare
    {
        let initial_frame = rx.borrow_and_update().clone();
        if let Ok(json_text) = serde_json::to_string(&*initial_frame) {
            if socket.send(Message::Text(json_text)).await.is_err() {
                return;
            }
        }
    }

    loop {
        tokio::select! {
            // 1. Conflație Zero-Lag: rx.changed() livrează EXCLUSIV cel mai recent cadru 60 Hz!
            changed_res = rx.changed() => {
                if changed_res.is_err() {
                    break;
                }
                let latest_frame = rx.borrow_and_update().clone();
                if let Ok(json_text) = serde_json::to_string(&*latest_frame) {
                    if socket.send(Message::Text(json_text)).await.is_err() {
                        break;
                    }
                }
            }
            // 2. Comenzi interactive de la juriu/pilot (tastatură, butoane 6-DoF, Director Mode, Cyber-Warfare)
            incoming_msg = socket.recv() => {
                match incoming_msg {
                    Some(Ok(Message::Text(txt))) => {
                        if let Ok(parsed_cmd) = serde_json::from_str::<SimCommand>(&txt) {
                            let _ = cmd_tx.try_send(parsed_cmd.sanitize());
                        }
                    }
                    Some(Ok(Message::Binary(bin))) => {
                        if let Ok(parsed_cmd) = serde_json::from_slice::<SimCommand>(&bin) {
                            let _ = cmd_tx.try_send(parsed_cmd.sanitize());
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Structură internă pentru deserializarea răspunsului JSON de la Open-Meteo pentru Craiova (`LRCV`).
#[derive(Debug, Deserialize)]
struct OpenMeteoApiResponse {
    current: Option<OpenMeteoCurrent>,
}

#[derive(Debug, Deserialize)]
struct OpenMeteoCurrent {
    temperature_2m: Option<f64>,
    relative_humidity_2m: Option<f64>,
    surface_pressure: Option<f64>,
    wind_speed_10m: Option<f64>,
    wind_direction_10m: Option<f64>,
}

/// Lansează task-ul async în `tokio` care interoghează periodic Open-Meteo pe coordonatele
/// Pragului RWY 09 Craiova (`44.3181° N, 23.8886° E`) cu timeout strict de `1500 ms`.
/// Dacă internetul din sală cade, păstrează ultimul cache valid fără a bloca nimic!
pub fn spawn_lrcv_open_meteo_poller(live_meteo: Arc<ArcSwap<LiveMeteoSnapshot>>) {
    tokio::spawn(async move {
        let client_res = reqwest::Client::builder()
            .timeout(Duration::from_millis(1500))
            .build();
        let client = match client_res {
            Ok(c) => c,
            Err(_) => return,
        };

        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={:.4}&longitude={:.4}&current=temperature_2m,relative_humidity_2m,surface_pressure,wind_speed_10m,wind_direction_10m&wind_speed_unit=ms",
            LRCV_RWY09_LAT_DEG, LRCV_RWY09_LON_DEG
        );

        loop {
            if let Ok(resp) = client.get(&url).send().await {
                if let Ok(parsed) = resp.json::<OpenMeteoApiResponse>().await {
                    if let Some(cur) = parsed.current {
                        let prev = live_meteo.load();
                        let updated = LiveMeteoSnapshot {
                            temperature_c: sanitize_f64(
                                cur.temperature_2m.unwrap_or(prev.temperature_c),
                                -45.0,
                                55.0,
                                8.5,
                            ),
                            relative_humidity_pct: sanitize_f64(
                                cur.relative_humidity_2m.unwrap_or(prev.relative_humidity_pct),
                                5.0,
                                100.0,
                                98.0,
                            ),
                            qnh_hpa: sanitize_f64(
                                cur.surface_pressure.unwrap_or(prev.qnh_hpa),
                                920.0,
                                1065.0,
                                1016.2,
                            ),
                            wind_speed_mps: sanitize_f64(
                                cur.wind_speed_10m.unwrap_or(prev.wind_speed_mps),
                                0.0,
                                65.0,
                                4.5,
                            ),
                            wind_dir_deg: sanitize_f64(
                                cur.wind_direction_10m.unwrap_or(prev.wind_dir_deg),
                                0.0,
                                360.0,
                                175.0,
                            ),
                            is_live_online: true,
                        };
                        live_meteo.store(Arc::new(updated));
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });
}
