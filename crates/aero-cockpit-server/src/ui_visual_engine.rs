//! Motorul vizual aerisit pe înțelesul unui copil de 6 ani pentru panourile `#hudCanvas` (Pagina 2)
//! și `#radarCanvas` (Pagina 3).
//!
//! Respectă principiul UX: **„Interfață aerisită — câte un singur concept pe pagină, fără texte
//! suprapuse sau înghesuite!”**
//! - **Pagina 2 (Parbrizul Avionului — `#hudCanvas`, `900x460px`)**: 3 baloane explicative scurte,
//!   poziționate strict în colțuri (`fog_cloud` stânga-sus, `ground_als` stânga-jos, `our_plane` dreapta-sus),
//!   lăsând centrul pistei și avionul 100% libere și vizibile.
//! - **Pagina 3 (Cursa celor 4 Avioane — `#radarCanvas`, `900x440px`)**: toată lățimea de `900px` este
//!   liberă pentru zborul celor 4 avioane (`✈️🟢 AERO-SHIELD`, `✈️🔴 GPS PĂCĂLIT`, `✈️🟡 CEAȚĂ SIMPLĂ`,
//!   `✈️🟠 FĂRĂ CAMERĂ`), cu separare verticală generoasă, anti-coliziune între etichete și clasament
//!   într-o bară orizontală aerisită.

use axum::{
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// Specificație tipizată pentru un balon explicativ (callout) afișat în colțurile `#hudCanvas`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VisualCalloutSpec {
    /// Identificator unic: `"fog_cloud"`, `"ground_als"`, `"our_plane"`.
    pub id: &'static str,
    /// Pictogramă emoji ilustrativă.
    pub icon: &'static str,
    /// Culoarea tematică în format hexazecimal.
    pub color_hex: &'static str,
    /// Titlu scurt și aerisit pe înțelesul unui copil de 6 ani (Română).
    pub title_ro: &'static str,
    /// Titlu scurt și aerisit pe înțelesul unui copil de 6 ani (Engleză).
    pub title_en: &'static str,
    /// Subtitlu explicativ concis (Română).
    pub subtitle_ro: &'static str,
    /// Subtitlu explicativ concis (Engleză).
    pub subtitle_en: &'static str,
    /// Insignă compactă opțională (Română).
    pub alert_badge_ro: Option<&'static str>,
    /// Insignă compactă opțională (Engleză).
    pub alert_badge_en: Option<&'static str>,
    /// Coordonate normalizate `[x_frac, y_frac, w_frac, h_frac]` strict în colțuri (`0.0..=1.0`).
    pub box_norm: [f64; 4],
    /// Punctul țintă `[tx_frac, ty_frac]` spre care indică săgeata fină a balonului explicativ.
    pub target_norm: [f64; 2],
}

/// Specificație vizuală pentru unul dintre cele 4 avioane din arena `#radarCanvas`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PlaneVisualSpec {
    /// Cheia internă a estimatorului (`"shield"`, `"gnss"`, `"naive"`, `"ins"`).
    pub key: &'static str,
    /// Insigna emoji a avionului (`"✈️🟢"`, `"✈️🔴"`, `"✈️🟡"`, `"✈️🟠"`).
    pub emoji_badge: &'static str,
    /// Culoarea asociată avionului și traseului său (hex).
    pub color_hex: &'static str,
    /// Etichetă scurtă și aerisită a avionului (Română).
    pub label_ro: &'static str,
    /// Etichetă scurtă și aerisită a avionului (Engleză).
    pub label_en: &'static str,
    /// Povestea scurtă despre cum zboară acest avion (Română).
    pub story_ro: &'static str,
    /// Povestea scurtă despre cum zboară acest avion (Engleză).
    pub story_en: &'static str,
}

/// Specificație pentru indicatorul luminos `👇 UITE AICI!` aferent fiecărui pas din lecție (`1..=5`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SpotlightPointerSpec {
    /// Numărul pasului din ghidul interactiv (`1..=5`).
    pub step_number: u8,
    /// Textul insignei indicatoare în Română (`"👇 UITE AICI!"`).
    pub pointer_badge_ro: &'static str,
    /// Textul insignei indicatoare în Engleză (`"👇 LOOK HERE!"`).
    pub pointer_badge_en: &'static str,
    /// ID-ul canvas-ului evidențiat (`"hudCanvas"` sau `"radarCanvas"`).
    pub target_canvas_id: &'static str,
    /// ID-ul elementului focalizat în cadrul canvas-ului.
    pub focus_callout_id: &'static str,
    /// Explicația pe înțelesul unui copil de 6 ani în Română.
    pub explanation_6yo_ro: &'static str,
    /// Explicația pe înțelesul unui copil de 6 ani în Engleză.
    pub explanation_6yo_en: &'static str,
}

/// Configurația arenei vizuale aerisite cu 4 avioane pentru `#radarCanvas` (`900x440px`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FourPlanesArenaConfig {
    /// Eticheta pistei sigure pe mijloc (Română).
    pub runway_title_ro: &'static str,
    /// Eticheta pistei sigure pe mijloc (Engleză).
    pub runway_title_en: &'static str,
    /// Eticheta zonei periculoase cu iarbă în afara pistei (Română).
    pub grass_danger_ro: &'static str,
    /// Eticheta zonei periculoase cu iarbă în afara pistei (Engleză).
    pub grass_danger_en: &'static str,
    /// Eticheta momelii hackerului GPS fals (Română).
    pub hacker_decoy_ro: &'static str,
    /// Eticheta momelii hackerului GPS fals (Engleză).
    pub hacker_decoy_en: &'static str,
    /// Textul butonului pentru vederea joc cu 4 avioane (Română).
    pub mode_game_btn_ro: &'static str,
    /// Textul butonului pentru vederea joc cu 4 avioane (Engleză).
    pub mode_game_btn_en: &'static str,
    /// Textul butonului pentru graficul tehnic de eroare în metri (Română).
    pub mode_chart_btn_ro: &'static str,
    /// Textul butonului pentru graficul tehnic de eroare în metri (Engleză).
    pub mode_chart_btn_en: &'static str,
    /// Cele 4 avioane comparate în timp real.
    pub planes: [PlaneVisualSpec; 4],
}

/// Specificația completă a motorului vizual pentru `#hudCanvas` (Pagina 2) și `#radarCanvas` (Pagina 3).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VisualCanvasEngineSpec {
    /// Cele 3 baloane explicative poziționate strict în colțurile `#hudCanvas`.
    pub hud_callouts: [VisualCalloutSpec; 3],
    /// Configurația arenei aerisite cu 4 avioane și momeala hackerului pe `#radarCanvas`.
    pub arena_config: FourPlanesArenaConfig,
    /// Cele 5 spoturi luminoase `👇 UITE AICI!` sincronizate cu pașii lecției.
    pub spotlights: [SpotlightPointerSpec; 5],
}

/// Construiește configurația deterministă completă în Română și Engleză pentru motorul vizual aerisit.
#[must_use]
pub const fn build_default_visual_engine_spec() -> VisualCanvasEngineSpec {
    VisualCanvasEngineSpec {
        hud_callouts: [
            VisualCalloutSpec {
                id: "fog_cloud",
                icon: "☁️💨",
                color_hex: "#e879f9",
                title_ro: "☁️💨 VÂNT ÎN CEAȚĂ",
                title_en: "☁️💨 FOG WIND",
                subtitle_ro: "Săgețile mov arată vântul din nor",
                subtitle_en: "Purple arrows show wind in the fog",
                alert_badge_ro: Some("👁️ VĂZUT ÎN INFRAROȘU"),
                alert_badge_en: Some("👁️ SEEN IN INFRARED"),
                box_norm: [0.02, 0.04, 0.29, 0.14],
                target_norm: [0.28, 0.24],
            },
            VisualCalloutSpec {
                id: "ground_als",
                icon: "💡",
                color_hex: "#00ff9d",
                title_ro: "💡 BECURILE PISTEI (VITEZA SOL)",
                title_en: "💡 RUNWAY LIGHTS (GROUND SPEED)",
                subtitle_ro: "Repere fixe la sol pe Pista 09",
                subtitle_en: "Fixed ground lights on RWY 09",
                alert_badge_ro: Some("🔒 ANCORĂ FIZICĂ LA SOL"),
                alert_badge_en: Some("🔒 SAFE GROUND ANCHOR"),
                box_norm: [0.02, 0.80, 0.31, 0.14],
                target_norm: [0.46, 0.68],
            },
            VisualCalloutSpec {
                id: "our_plane",
                icon: "✈️",
                color_hex: "#00e5ff",
                title_ro: "✈️ AVIONUL NOSTRU (PE CENTRU)",
                title_en: "✈️ OUR PLANE (ON CENTERLINE)",
                subtitle_ro: "Urmărește linia luminoasă a pistei",
                subtitle_en: "Follows the bright runway centerline",
                alert_badge_ro: Some("🔄 CORECȚIE VIRAJ ACTIVĂ"),
                alert_badge_en: Some("🔄 TURN DEROTATION ACTIVE"),
                box_norm: [0.67, 0.04, 0.31, 0.14],
                target_norm: [0.50, 0.50],
            },
        ],
        arena_config: FourPlanesArenaConfig {
            runway_title_ro: "🛬 PISTA 09 CRAIOVA (SIGUR PE MIJLOC ✅)",
            runway_title_en: "🛬 CRAIOVA RUNWAY 09 (SAFE ON CENTER ✅)",
            grass_danger_ro: "🌱 IARBĂ (PERICOL DE IEȘIRE DE PE PISTĂ!)",
            grass_danger_en: "🌱 GRASS (OFF-RUNWAY DANGER ZONE!)",
            hacker_decoy_ro: "👾 MOMEALA HACKERULUI (GPS FALS)",
            hacker_decoy_en: "👾 HACKER'S DECOY (FAKE GPS)",
            mode_game_btn_ro: "🎮 Vedere Joc 4 Avioane",
            mode_game_btn_en: "🎮 4-Planes Game View",
            mode_chart_btn_ro: "📈 Grafic Eroare (m)",
            mode_chart_btn_en: "📈 Error Chart (m)",
            planes: [
                PlaneVisualSpec {
                    key: "shield",
                    emoji_badge: "✈️🟢",
                    color_hex: "#00ff9d",
                    label_ro: "✈️🟢 AERO-SHIELD",
                    label_en: "✈️🟢 AERO-SHIELD",
                    story_ro: "Ignoră minciuna hackerului și aterizează drept pe centrul pistei!",
                    story_en: "Ignores the hacker's lie and lands safely on the runway centerline!",
                },
                PlaneVisualSpec {
                    key: "gnss",
                    emoji_badge: "✈️🔴",
                    color_hex: "#ff3355",
                    label_ro: "✈️🔴 GPS PĂCĂLIT",
                    label_en: "✈️🔴 HACKED GPS",
                    story_ro: "Ascultă orbește de momeala hackerului 👾 și iese în iarbă!",
                    story_en: "Blindly follows the hacker's decoy 👾 and veers into the grass!",
                },
                PlaneVisualSpec {
                    key: "naive",
                    emoji_badge: "✈️🟡",
                    color_hex: "#eab308",
                    label_ro: "✈️🟡 CEAȚĂ SIMPLĂ",
                    label_en: "✈️🟡 NAIVE FOG",
                    story_ro: "Se lasă împins de vântul din ceață în afara pistei!",
                    story_en: "Gets pushed by the moving fog cloud off the runway!",
                },
                PlaneVisualSpec {
                    key: "ins",
                    emoji_badge: "✈️🟠",
                    color_hex: "#f97316",
                    label_ro: "✈️🟠 FĂRĂ CAMERĂ",
                    label_en: "✈️🟠 NO CAMERA",
                    story_ro: "Zboară legat la ochi și amețește treptat fără repere vizuale!",
                    story_en: "Flies blindfolded and slowly drifts without visual landmarks!",
                },
            ],
        },
        spotlights: [
            SpotlightPointerSpec {
                step_number: 1,
                pointer_badge_ro: "👇 UITE AICI!",
                pointer_badge_en: "👇 LOOK HERE!",
                target_canvas_id: "hudCanvas",
                focus_callout_id: "fog_cloud",
                explanation_6yo_ro: "Pasul 1: În colțul stânga-sus vezi vântul din ceață ☁️💨, iar pe mijloc pista rămâne liberă și clară!",
                explanation_6yo_en: "Step 1: Top-left shows the fog wind ☁️💨, while the center runway stays completely clear!",
            },
            SpotlightPointerSpec {
                step_number: 2,
                pointer_badge_ro: "👇 UITE AICI!",
                pointer_badge_en: "👇 LOOK HERE!",
                target_canvas_id: "radarCanvas",
                focus_callout_id: "hacker_decoy",
                explanation_6yo_ro: "Pasul 2: Hackerul 👾 aruncă o momeală GPS falsă în iarbă și trage avionul roșu ✈️🔴 afară de pe pistă!",
                explanation_6yo_en: "Step 2: The hacker 👾 drops a fake GPS decoy in the grass and pulls the red plane ✈️🔴 off the runway!",
            },
            SpotlightPointerSpec {
                step_number: 3,
                pointer_badge_ro: "👇 UITE AICI!",
                pointer_badge_en: "👇 LOOK HERE!",
                target_canvas_id: "hudCanvas",
                focus_callout_id: "ground_als",
                explanation_6yo_ro: "Pasul 3: În colțul stânga-jos, becurile pistei 💡 măsoară viteza reală față de pământ!",
                explanation_6yo_en: "Step 3: Bottom-left shows how runway lights 💡 measure true ground speed!",
            },
            SpotlightPointerSpec {
                step_number: 4,
                pointer_badge_ro: "👇 UITE AICI!",
                pointer_badge_en: "👇 LOOK HERE!",
                target_canvas_id: "radarCanvas",
                focus_callout_id: "shield_plane",
                explanation_6yo_ro: "Pasul 4: Avionul verde ✈️🟢 AERO-SHIELD rămâne singurul pe mijlocul pistei (< 0.8m)!",
                explanation_6yo_en: "Step 4: The green ✈️🟢 AERO-SHIELD plane stays safely on the runway centerline (< 0.8m)!",
            },
            SpotlightPointerSpec {
                step_number: 5,
                pointer_badge_ro: "👇 UITE AICI!",
                pointer_badge_en: "👇 LOOK HERE!",
                target_canvas_id: "hudCanvas",
                focus_callout_id: "our_plane",
                explanation_6yo_ro: "Pasul 5: În colțul dreapta-sus, avionul nostru ✈️ aterizează în siguranță pe centrul Pistei 09 Craiova!",
                explanation_6yo_en: "Step 5: Top-right confirms our plane ✈️ lands safely on Craiova Runway 09 centerline!",
            },
        ],
    }
}

/// Generează **Pagina 2 dedicată exclusiv Parbrizului Avionului (`#hudCanvas`, dimensiune mare `900x460px`)**.
///
/// Respectă regula „câte un singur concept pe pagină, fără texte suprapuse sau înghesuite”:
/// elimină benzile laterale înghesuite și lasă tot parbrizul `900x460px` larg, curat și aerisit,
/// cu cele 3 baloane explicative plasate strict în colțuri.
#[must_use]
pub fn render_page2_hud_canvas_html() -> String {
    let spec = build_default_visual_engine_spec();
    let c0 = &spec.hud_callouts[0];
    let c1 = &spec.hud_callouts[1];
    let c2 = &spec.hud_callouts[2];

    format!(
        r#"<section id="page2HudWindshieldSection" class="spacious-canvas-page" data-page-concept="hud-windshield" style="max-width:940px;margin:0 auto;padding:14px 18px;border-radius:14px;background:rgba(7,15,28,0.92);border:1px solid rgba(0,229,255,0.28);box-shadow:0 12px 32px rgba(0,0,0,0.45);">
  <header style="display:flex;flex-wrap:wrap;align-items:center;justify-content:space-between;gap:12px;margin-bottom:12px;">
    <div>
      <span style="display:inline-block;padding:3px 10px;border-radius:999px;background:rgba(0,229,255,0.14);border:1px solid #00e5ff;color:#00e5ff;font-size:11px;font-weight:800;letter-spacing:0.5px;">PAGINA 2 • PARBRIZUL AVIONULUI ÎN CEAȚĂ</span>
      <h2 style="margin:6px 0 0 0;font-size:18px;color:#f8fafc;font-weight:800;">🔦 Ce Vede Camera Infraroșu (SWIR) prin Ceață — Pista Liberă pe Centru</h2>
    </div>
    <div style="display:flex;flex-wrap:wrap;gap:8px;font-size:11.5px;">
      <span style="padding:4px 10px;border-radius:8px;background:rgba(232,121,249,0.12);border:1px solid {c0_col};color:{c0_col};font-weight:700;">{c0_title}</span>
      <span style="padding:4px 10px;border-radius:8px;background:rgba(0,255,157,0.12);border:1px solid {c1_col};color:{c1_col};font-weight:700;">{c1_title}</span>
      <span style="padding:4px 10px;border-radius:8px;background:rgba(0,229,255,0.12);border:1px solid {c2_col};color:{c2_col};font-weight:700;">{c2_title}</span>
    </div>
  </header>

  <div class="hud-canvas-wide-frame" style="position:relative;width:100%;max-width:900px;margin:0 auto;border-radius:12px;overflow:hidden;border:1.5px solid #1e293b;background:#040a12;">
    <canvas id="hudCanvas" width="900" height="460" style="display:block;width:100%;height:460px;"></canvas>
  </div>
</section>"#,
        c0_col = c0.color_hex,
        c0_title = c0.title_ro,
        c1_col = c1.color_hex,
        c1_title = c1.title_ro,
        c2_col = c2.color_hex,
        c2_title = c2.title_ro,
    )
}

/// Generează **Pagina 3 dedicată exclusiv Cursei celor 4 Avioane (`#radarCanvas`, dimensiune mare `900x440px`)**.
///
/// Mută clasamentul într-o bară orizontală aerisită deasupra pistei ca să lase toată lățimea de `900px`
/// liberă pentru zborul celor 4 avioane (`✈️🟢 AERO-SHIELD`, `✈️🔴 GPS PĂCĂLIT`, `✈️🟡 CEAȚĂ SIMPLĂ`,
/// `✈️🟠 FĂRĂ CAMERĂ`) fără coliziuni între etichete.
#[must_use]
pub fn render_page3_radar_canvas_html() -> String {
    let spec = build_default_visual_engine_spec();
    let p0 = &spec.arena_config.planes[0];
    let p1 = &spec.arena_config.planes[1];
    let p2 = &spec.arena_config.planes[2];
    let p3 = &spec.arena_config.planes[3];

    format!(
        r#"<section id="page3RadarRaceSection" class="spacious-canvas-page" data-page-concept="radar-4planes-race" style="max-width:940px;margin:0 auto;padding:14px 18px;border-radius:14px;background:rgba(7,15,28,0.92);border:1px solid rgba(0,255,157,0.28);box-shadow:0 12px 32px rgba(0,0,0,0.45);">
  <header style="display:flex;flex-wrap:wrap;align-items:center;justify-content:space-between;gap:12px;margin-bottom:10px;">
    <div>
      <span style="display:inline-block;padding:3px 10px;border-radius:999px;background:rgba(0,255,157,0.14);border:1px solid #00ff9d;color:#00ff9d;font-size:11px;font-weight:800;letter-spacing:0.5px;">PAGINA 3 • CURSA CELOR 4 AVIOANE VĂZUTĂ DE SUS</span>
      <h2 style="margin:6px 0 0 0;font-size:18px;color:#f8fafc;font-weight:800;">🏁 Cine Rămâne pe Pista 09 Craiova și Cine Iese în Iarbă?</h2>
    </div>
    <div style="display:flex;align-items:center;gap:8px;">
      <button type="button" id="btnRadarModeGame" onclick="window.setRadarMode6yo && window.setRadarMode6yo('game')" style="cursor:pointer;padding:6px 13px;border-radius:8px;border:1.5px solid #00ff9d;background:rgba(0,255,157,0.18);color:#00ff9d;font-weight:800;font-size:12px;">
        {mode_game_ro}
      </button>
      <button type="button" id="btnRadarModeChart" onclick="window.setRadarMode6yo && window.setRadarMode6yo('chart')" style="cursor:pointer;padding:6px 13px;border-radius:8px;border:1.5px solid #334155;background:rgba(30,41,59,0.75);color:#cbd5e1;font-weight:700;font-size:12px;">
        {mode_chart_ro}
      </button>
    </div>
  </header>

  <!-- Bară orizontală aerisită cu clasamentul celor 4 avioane (lasă toți cei 900px liberi pe #radarCanvas) -->
  <div id="fourPlanesHorizontalScoreboard" class="four-planes-horizontal-bar" style="display:grid;grid-template-columns:repeat(4,minmax(160px,1fr));gap:10px;padding:8px 12px;margin-bottom:10px;border-radius:10px;background:rgba(15,23,42,0.88);border:1px solid #1e293b;">
    <div title="{p0_story}" style="display:flex;align-items:center;justify-content:space-between;padding:6px 10px;border-radius:8px;background:rgba(0,255,157,0.12);border:1px solid {p0_col};color:{p0_col};font-weight:800;font-size:12px;">
      <span>{p0_label}</span>
      <span id="scorePillShield">0.3m ✅</span>
    </div>
    <div title="{p1_story}" style="display:flex;align-items:center;justify-content:space-between;padding:6px 10px;border-radius:8px;background:rgba(255,51,85,0.12);border:1px solid {p1_col};color:{p1_col};font-weight:800;font-size:12px;">
      <span>{p1_label}</span>
      <span id="scorePillGnss">GPS</span>
    </div>
    <div title="{p2_story}" style="display:flex;align-items:center;justify-content:space-between;padding:6px 10px;border-radius:8px;background:rgba(234,179,8,0.12);border:1px solid {p2_col};color:{p2_col};font-weight:800;font-size:12px;">
      <span>{p2_label}</span>
      <span id="scorePillNaive">FLOW</span>
    </div>
    <div title="{p3_story}" style="display:flex;align-items:center;justify-content:space-between;padding:6px 10px;border-radius:8px;background:rgba(249,115,22,0.12);border:1px solid {p3_col};color:{p3_col};font-weight:800;font-size:12px;">
      <span>{p3_label}</span>
      <span id="scorePillIns">INS</span>
    </div>
  </div>

  <div class="radar-canvas-wide-frame" style="position:relative;width:100%;max-width:900px;margin:0 auto;border-radius:12px;overflow:hidden;border:1.5px solid #1e293b;background:#040a12;">
    <canvas id="radarCanvas" width="900" height="440" style="display:block;width:100%;height:440px;"></canvas>
  </div>
</section>"#,
        mode_game_ro = spec.arena_config.mode_game_btn_ro,
        mode_chart_ro = spec.arena_config.mode_chart_btn_ro,
        p0_col = p0.color_hex,
        p0_label = p0.label_ro,
        p0_story = p0.story_ro,
        p1_col = p1.color_hex,
        p1_label = p1.label_ro,
        p1_story = p1.story_ro,
        p2_col = p2.color_hex,
        p2_label = p2.label_ro,
        p2_story = p2.story_ro,
        p3_col = p3.color_hex,
        p3_label = p3.label_ro,
        p3_story = p3.story_ro,
    )
}

/// Generează componenta SSR aerisită care afișează câte o singură pagină vizuală activă o dată
/// (Pagina 2: Parbrizul `#hudCanvas` `900x460px` sau Pagina 3: Cursa celor 4 Avioane `#radarCanvas` `900x440px`),
/// fără să le înghesuie una sub alta.
#[must_use]
pub fn render_visual_canvases_component_html() -> String {
    let spec = build_default_visual_engine_spec();
    let s0 = &spec.spotlights[0];
    let page2_html = render_page2_hud_canvas_html();
    let page3_html = render_page3_radar_canvas_html();

    format!(
        r#"<div id="aeroVisualEngineRoot" class="visual-engine-root" data-active-step="1" data-active-page="page2" data-radar-mode="game" style="max-width:960px;margin:0 auto;">
  <!-- Insigna luminoasă pas-cu-pas 👇 UITE AICI! + Comutator Pagina 2 vs Pagina 3 (câte un singur concept pe pagină) -->
  <div id="spotlightBanner6yo" class="spotlight-banner-6yo" role="status" aria-live="polite" style="display:flex;flex-wrap:wrap;align-items:center;justify-content:space-between;gap:12px;padding:10px 16px;margin-bottom:12px;border-radius:12px;background:linear-gradient(90deg,rgba(0,255,157,0.14),rgba(56,189,248,0.12));border:1px solid rgba(0,255,157,0.42);color:#f8fafc;font-size:13px;font-weight:700;">
    <div style="display:flex;align-items:center;gap:10px;flex:1;min-width:260px;">
      <span id="spotlightBadgeText" class="spotlight-badge-pill" style="background:#eab308;color:#0f172a;padding:4px 10px;border-radius:999px;font-weight:900;letter-spacing:0.4px;">{pointer_ro}</span>
      <span id="spotlightStepExplanation" style="flex:1;">{exp_ro}</span>
    </div>
    <div id="visualPageSwitcherTabs" style="display:flex;align-items:center;gap:8px;">
      <button type="button" id="btnSwitchPage2Hud" onclick="window.setVisualEnginePage6yo && window.setVisualEnginePage6yo('page2')" style="cursor:pointer;padding:6px 12px;border-radius:8px;border:1.5px solid #00e5ff;background:rgba(0,229,255,0.18);color:#00e5ff;font-weight:800;font-size:12px;">
        🔦 Pagina 2: Parbriz (900×460)
      </button>
      <button type="button" id="btnSwitchPage3Radar" onclick="window.setVisualEnginePage6yo && window.setVisualEnginePage6yo('page3')" style="cursor:pointer;padding:6px 12px;border-radius:8px;border:1.5px solid #334155;background:rgba(15,23,42,0.75);color:#cbd5e1;font-weight:700;font-size:12px;">
        🏁 Pagina 3: Cursa 4 Avioane (900×440)
      </button>
    </div>
  </div>

  <!-- Container Pagina 2 (Activ implicit — un singur concept pe ecran) -->
  <div id="visualPage2Container" style="display:block;">
    {page2_html}
  </div>

  <!-- Container Pagina 3 (Comutabil — un singur concept pe ecran, fără înghesuire verticală) -->
  <div id="visualPage3Container" style="display:none;">
    {page3_html}
  </div>
</div>"#,
        pointer_ro = s0.pointer_badge_ro,
        exp_ro = s0.explanation_6yo_ro,
        page2_html = page2_html,
        page3_html = page3_html,
    )
}

/// Generează scriptul JavaScript complet (60 FPS Canvas 2D) pentru `#hudCanvas` (`900x460px`)
/// și `#radarCanvas` (`900x440px`), serializând direct `VisualCanvasEngineSpec` din Rust și expunând:
/// - `window.renderHudCanvas6yo(ctx, canvas, state, history)`
/// - `window.renderRadarCanvas6yo(ctx, canvas, state, history)`
/// - `window.setRadarMode6yo(mode)`
/// - `window.setVisualEnginePage6yo(pageKey)`
/// - `window.setSpotlightLessonStep(stepNumber)`
#[must_use]
pub fn render_visual_canvases_script() -> String {
    let spec = build_default_visual_engine_spec();
    let spec_json = serde_json::to_string(&spec).unwrap_or_else(|_| "{}".to_string());

    format!(
        r#"// === AERO-SHIELD V2.0 - MOTOR VIZUAL AERISIT 60 FPS (PAGINA 2 PARBRIZ 900x460 & PAGINA 3 CURSA 900x440) ===
(function() {{
  'use strict';

  const VISUAL_ENGINE_SPEC = {spec_json};
  window.AERO_VISUAL_ENGINE_SPEC = VISUAL_ENGINE_SPEC;
  window.aeroRadarMode6yo = window.aeroRadarMode6yo || 'game'; // 'game' | 'chart'
  window.aeroSpotlightStep6yo = window.aeroSpotlightStep6yo || 1; // 1..5
  window.aeroActiveVisualPage6yo = window.aeroActiveVisualPage6yo || 'page2'; // 'page2' | 'page3'

  function currentLang() {{
    if (typeof window.currentLang === 'string') return window.currentLang;
    if (document.documentElement && document.documentElement.lang === 'en') return 'en';
    return 'ro';
  }}

  function pickText(roText, enText) {{
    return currentLang() === 'en' ? (enText || roText) : (roText || enText);
  }}

  function drawRoundedBox(ctx, x, y, w, h, r, fillStyle, strokeStyle, lineWidth) {{
    ctx.save();
    ctx.beginPath();
    const radius = Math.min(r, w / 2, h / 2);
    ctx.moveTo(x + radius, y);
    ctx.lineTo(x + w - radius, y);
    ctx.quadraticCurveTo(x + w, y, x + w, y + radius);
    ctx.lineTo(x + w, y + h - radius);
    ctx.quadraticCurveTo(x + w, y + h, x + w - radius, y + h);
    ctx.lineTo(x + radius, y + h);
    ctx.quadraticCurveTo(x, y + h, x, y + h - radius);
    ctx.lineTo(x, y + radius);
    ctx.quadraticCurveTo(x, y, x + radius, y);
    ctx.closePath();
    if (fillStyle) {{
      ctx.fillStyle = fillStyle;
      ctx.fill();
    }}
    if (strokeStyle) {{
      ctx.strokeStyle = strokeStyle;
      ctx.lineWidth = lineWidth || 1.5;
      ctx.stroke();
    }}
    ctx.restore();
  }}

  // Comută aerisit între Pagina 2 (#hudCanvas 900x460) și Pagina 3 (#radarCanvas 900x440)
  window.setVisualEnginePage6yo = function(pageKey) {{
    const isPage3 = pageKey === 'page3' || pageKey === 'radarCanvas';
    window.aeroActiveVisualPage6yo = isPage3 ? 'page3' : 'page2';
    const root = document.getElementById('aeroVisualEngineRoot');
    if (root) root.setAttribute('data-active-page', window.aeroActiveVisualPage6yo);

    const p2 = document.getElementById('visualPage2Container');
    const p3 = document.getElementById('visualPage3Container');
    if (p2 && p3) {{
      p2.style.display = isPage3 ? 'none' : 'block';
      p3.style.display = isPage3 ? 'block' : 'none';
    }}

    const b2 = document.getElementById('btnSwitchPage2Hud');
    const b3 = document.getElementById('btnSwitchPage3Radar');
    if (b2 && b3) {{
      b2.style.borderColor = !isPage3 ? '#00e5ff' : '#334155';
      b2.style.background = !isPage3 ? 'rgba(0,229,255,0.18)' : 'rgba(15,23,42,0.75)';
      b2.style.color = !isPage3 ? '#00e5ff' : '#cbd5e1';
      b3.style.borderColor = isPage3 ? '#00ff9d' : '#334155';
      b3.style.background = isPage3 ? 'rgba(0,255,157,0.18)' : 'rgba(15,23,42,0.75)';
      b3.style.color = isPage3 ? '#00ff9d' : '#cbd5e1';
    }}
  }};

  window.setRadarMode6yo = function(mode) {{
    window.aeroRadarMode6yo = (mode === 'chart') ? 'chart' : 'game';
    const root = document.getElementById('aeroVisualEngineRoot');
    if (root) root.setAttribute('data-radar-mode', window.aeroRadarMode6yo);
    const btnGame = document.getElementById('btnRadarModeGame');
    const btnChart = document.getElementById('btnRadarModeChart');
    if (btnGame && btnChart) {{
      const isGame = window.aeroRadarMode6yo === 'game';
      btnGame.style.borderColor = isGame ? '#00ff9d' : '#334155';
      btnGame.style.background = isGame ? 'rgba(0,255,157,0.18)' : 'rgba(30,41,59,0.75)';
      btnGame.style.color = isGame ? '#00ff9d' : '#cbd5e1';
      btnChart.style.borderColor = !isGame ? '#38bdf8' : '#334155';
      btnChart.style.background = !isGame ? 'rgba(56,189,248,0.18)' : 'rgba(30,41,59,0.75)';
      btnChart.style.color = !isGame ? '#38bdf8' : '#cbd5e1';
    }}
  }};

  window.setSpotlightLessonStep = function(stepNumber) {{
    const step = Math.max(1, Math.min(5, Number(stepNumber) || 1));
    window.aeroSpotlightStep6yo = step;
    const root = document.getElementById('aeroVisualEngineRoot');
    if (root) root.setAttribute('data-active-step', String(step));
    const spotlights = VISUAL_ENGINE_SPEC.spotlights || [];
    const active = spotlights.find(s => s.step_number === step) || spotlights[0];
    if (!active) return;
    const badgeEl = document.getElementById('spotlightBadgeText');
    const expEl = document.getElementById('spotlightStepExplanation');
    if (badgeEl) badgeEl.textContent = pickText(active.pointer_badge_ro, active.pointer_badge_en);
    if (expEl) expEl.textContent = pickText(active.explanation_6yo_ro, active.explanation_6yo_en);
    if (active.target_canvas_id === 'radarCanvas') {{
      window.setVisualEnginePage6yo('page3');
    }} else {{
      window.setVisualEnginePage6yo('page2');
    }}
  }};

  // Anti-coliziune pe axa Y pentru etichetele celor 4 avioane (garantează distanța minimă minGapPx)
  function resolveLabelCollisionsY(items, minY, maxY, minGapPx) {{
    const sorted = items.slice().sort((a, b) => a.labelY - b.labelY);
    for (let pass = 0; pass < 4; pass++) {{
      for (let i = 1; i < sorted.length; i++) {{
        const prev = sorted[i - 1];
        const curr = sorted[i];
        if (curr.labelY - prev.labelY < minGapPx) {{
          const mid = 0.5 * (prev.labelY + curr.labelY);
          prev.labelY = Math.max(minY, mid - 0.5 * minGapPx);
          curr.labelY = Math.min(maxY, prev.labelY + minGapPx);
        }}
      }}
    }}
    return sorted;
  }}

  // === 1. PAGINA 2: RANDARE PARBRIZ #hudCanvas (900x460px, BALOANE STRICT ÎN COLȚURI, CENTRU 100% LIBER) ===
  window.renderHudCanvas6yo = function(ctx, canvas, s) {{
    if (!ctx || !canvas || !s) return;
    const w = canvas.width || 900;
    const h = canvas.height || 460;
    const cx = w / 2;
    const cy = h / 2;
    const nowMs = Date.now();
    const pulse = 0.5 + 0.5 * Math.sin(nowMs * 0.006);

    // Fundal aerisit de parbriz termic SWIR (fără benzi laterale care să înghesuie imaginea)
    const grad = ctx.createLinearGradient(0, 0, 0, h);
    grad.addColorStop(0, '#05101a');
    grad.addColorStop(0.47, '#0b2232');
    grad.addColorStop(0.50, '#0d2b22');
    grad.addColorStop(1, '#040a0e');
    ctx.fillStyle = grad;
    ctx.fillRect(0, 0, w, h);

    // Nori diafani de ceață în treimea superioară + săgeți mov de vânt
    ctx.save();
    ctx.fillStyle = 'rgba(232, 121, 249, 0.08)';
    for (let i = 0; i < 4; i++) {{
      const cloudX = ((nowMs * 0.018 + i * 220) % (w + 240)) - 120;
      const cloudY = h * (0.22 + (i % 2) * 0.06);
      ctx.beginPath();
      ctx.ellipse(cloudX, cloudY, 90, 20, 0, 0, Math.PI * 2);
      ctx.fill();
    }}
    ctx.restore();

    // Săgeți mov pentru vântul din ceață (în zona superioară-centrală)
    ctx.save();
    ctx.strokeStyle = '#e879f9';
    ctx.fillStyle = '#e879f9';
    ctx.lineWidth = 2;
    for (let k = -1; k <= 1; k++) {{
      const ax = cx + k * 95 - 18;
      const ay = h * 0.25 + (k === 0 ? -8 : 6);
      ctx.beginPath();
      ctx.moveTo(ax, ay);
      ctx.lineTo(ax + 34, ay);
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(ax + 34, ay);
      ctx.lineTo(ax + 27, ay - 4);
      ctx.lineTo(ax + 27, ay + 4);
      ctx.closePath();
      ctx.fill();
    }}
    ctx.restore();

    // Linia orizontului
    ctx.strokeStyle = 'rgba(0, 255, 157, 0.38)';
    ctx.lineWidth = 1.4;
    ctx.beginPath();
    ctx.moveTo(0, cy);
    ctx.lineTo(w, cy);
    ctx.stroke();

    // Perspectiva Pistei 09 Craiova complet liberă pe centru
    const rwyShift = Math.max(-w * 0.22, Math.min(w * 0.22, -(s.shield_err_m || 0) * 5.5));
    const topW = Math.max(36, (1400 - Math.min(1250, s.dist_to_threshold_m || 0)) * 0.045);
    const botW = topW * 5.0;
    const rwyTopY = cy + 14;
    const rwyBotY = h - 16;

    ctx.fillStyle = 'rgba(0, 255, 157, 0.08)';
    ctx.beginPath();
    ctx.moveTo(cx + rwyShift - topW, rwyTopY);
    ctx.lineTo(cx + rwyShift + topW, rwyTopY);
    ctx.lineTo(cx + rwyShift + botW, rwyBotY);
    ctx.lineTo(cx + rwyShift - botW, rwyBotY);
    ctx.closePath();
    ctx.fill();

    ctx.strokeStyle = '#00ff9d';
    ctx.lineWidth = 2.2;
    ctx.stroke();

    // Axul central al pistei
    ctx.save();
    ctx.strokeStyle = '#ffffff';
    ctx.lineWidth = 2.2;
    ctx.setLineDash([12, 9]);
    ctx.beginPath();
    ctx.moveTo(cx + rwyShift, rwyTopY);
    ctx.lineTo(cx + rwyShift, rwyBotY);
    ctx.stroke();
    ctx.restore();

    // 💡 Becurile calde ALS de pe pistă
    const rows = 7;
    for (let r = 1; r <= rows; r++) {{
      const frac = r / rows;
      const ly = rwyTopY + (rwyBotY - rwyTopY) * frac;
      const halfSpan = topW + (botW - topW) * frac;
      const glowRadius = 2.4 + frac * 3.2;
      const lampPositions = [
        cx + rwyShift - halfSpan,
        cx + rwyShift + halfSpan,
        cx + rwyShift
      ];
      lampPositions.forEach((lx, idx) => {{
        ctx.beginPath();
        ctx.fillStyle = idx === 2 ? '#fde047' : '#00ff9d';
        ctx.arc(lx, ly, glowRadius, 0, Math.PI * 2);
        ctx.fill();
      }});
    }}

    // ✈️ Reticulul avionului nostru în centru (fără niciun text suprapus peste centru!)
    ctx.save();
    ctx.strokeStyle = '#00e5ff';
    ctx.lineWidth = 2.6;
    ctx.beginPath();
    ctx.moveTo(cx - 28, cy);
    ctx.lineTo(cx - 8, cy);
    ctx.lineTo(cx - 8, cy + 7);
    ctx.moveTo(cx + 8, cy);
    ctx.lineTo(cx + 28, cy);
    ctx.lineTo(cx + 8, cy + 7);
    ctx.stroke();
    ctx.restore();

    // Cele 3 baloane explicative poziționate STRICT ÎN COLȚURI (fără să atingă centrul pistei)
    const callouts = VISUAL_ENGINE_SPEC.hud_callouts || [];
    const spotlights = VISUAL_ENGINE_SPEC.spotlights || [];
    const activeSpotlight = spotlights.find(sp => sp.step_number === window.aeroSpotlightStep6yo);

    callouts.forEach(c => {{
      const bx = c.box_norm[0] * w;
      const by = c.box_norm[1] * h;
      const bw = c.box_norm[2] * w;
      const bh = c.box_norm[3] * h;
      const tx = c.target_norm[0] * w;
      const ty = c.target_norm[1] * h;

      const isFocused = activeSpotlight &&
        activeSpotlight.target_canvas_id === 'hudCanvas' &&
        activeSpotlight.focus_callout_id === c.id;

      // Săgeată fină punctată de la colț spre elementul indicat
      const startX = bx < cx ? (bx + bw) : bx;
      const startY = by < cy ? (by + bh * 0.7) : (by + bh * 0.3);
      ctx.save();
      ctx.strokeStyle = c.color_hex;
      ctx.lineWidth = isFocused ? 2.2 : 1.3;
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      ctx.moveTo(startX, startY);
      ctx.lineTo(tx, ty);
      ctx.stroke();
      ctx.fillStyle = c.color_hex;
      ctx.beginPath();
      ctx.arc(tx, ty, 3.5, 0, Math.PI * 2);
      ctx.fill();
      ctx.restore();

      // Cutia aerisită din colț
      const borderCol = isFocused ? '#fde047' : c.color_hex;
      const borderWidth = isFocused ? (2.2 + pulse * 1.2) : 1.5;
      drawRoundedBox(ctx, bx, by, bw, bh, 9, 'rgba(7, 15, 28, 0.90)', borderCol, borderWidth);

      ctx.save();
      ctx.textAlign = 'left';
      ctx.fillStyle = c.color_hex;
      ctx.font = 'bold 12px sans-serif';
      ctx.fillText(pickText(c.title_ro, c.title_en), bx + 10, by + 21);

      ctx.fillStyle = '#e2e8f0';
      ctx.font = '11px sans-serif';
      ctx.fillText(pickText(c.subtitle_ro, c.subtitle_en), bx + 10, by + 39);

      const badgeText = pickText(c.alert_badge_ro, c.alert_badge_en);
      if (badgeText) {{
        ctx.fillStyle = '#fde047';
        ctx.font = 'bold 10px sans-serif';
        ctx.fillText(badgeText, bx + 10, by + 55);
      }}

      if (isFocused && activeSpotlight) {{
        const ptrText = pickText(activeSpotlight.pointer_badge_ro, activeSpotlight.pointer_badge_en);
        drawRoundedBox(ctx, bx + bw - 96, by + 4, 90, 18, 9, '#fde047', '#0f172a', 1.2);
        ctx.fillStyle = '#0f172a';
        ctx.font = 'bold 9.5px sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(ptrText, bx + bw - 51, by + 16);
      }}
      ctx.restore();
    }});
  }};

  // === 2. PAGINA 3: RANDARE CURSA CELOR 4 AVIOANE #radarCanvas (900x440px, TOATĂ LĂȚIMEA LIBERĂ, ANTI-COLIZIUNE) ===
  window.renderRadarCanvas6yo = function(ctx, canvas, s, history) {{
    if (!ctx || !canvas || !s) return;
    const w = canvas.width || 900;
    const h = canvas.height || 440;
    const arena = VISUAL_ENGINE_SPEC.arena_config;
    const nowMs = Date.now();
    const pulse = 0.5 + 0.5 * Math.sin(nowMs * 0.007);

    ctx.fillStyle = '#050c15';
    ctx.fillRect(0, 0, w, h);

    if (window.aeroRadarMode6yo === 'chart') {{
      renderTechnicalChartMode(ctx, w, h, s, history || [], arena);
      return;
    }}

    // Actualizăm și bara orizontală de clasament de deasupra canvas-ului (dacă există în DOM)
    const planeErrors = {{
      shield: Number(s.shield_err_m) || 0,
      ins: Number(s.ins_err_m) || 0,
      naive: Number(s.naive_ekf_err_m) || 0,
      gnss: Number(s.gnss_err_m) || 0
    }};
    const pillShield = document.getElementById('scorePillShield');
    const pillGnss = document.getElementById('scorePillGnss');
    const pillNaive = document.getElementById('scorePillNaive');
    const pillIns = document.getElementById('scorePillIns');
    if (pillShield) pillShield.textContent = planeErrors.shield.toFixed(1) + 'm ✅';
    if (pillGnss) pillGnss.textContent = planeErrors.gnss.toFixed(1) + 'm';
    if (pillNaive) pillNaive.textContent = planeErrors.naive.toFixed(1) + 'm';
    if (pillIns) pillIns.textContent = planeErrors.ins.toFixed(1) + 'm';

    // Toți cei 900px lățime sunt dedicați pistei și zborului celor 4 avioane!
    const rwyCenterY = h * 0.42;
    const rwyHalfHeight = h * 0.16;

    // 1. Zonele de IARBĂ sus și jos
    ctx.fillStyle = 'rgba(20, 83, 45, 0.34)';
    ctx.fillRect(0, 0, w, rwyCenterY - rwyHalfHeight);
    ctx.fillRect(0, rwyCenterY + rwyHalfHeight, w, h - (rwyCenterY + rwyHalfHeight));

    ctx.fillStyle = '#86efac';
    ctx.font = 'bold 11px sans-serif';
    ctx.textAlign = 'left';
    ctx.fillText(pickText(arena.grass_danger_ro, arena.grass_danger_en), 16, 20);
    ctx.fillText(pickText(arena.grass_danger_ro, arena.grass_danger_en), 16, h - 12);

    // 2. Pista 09 Craiova pe toată lățimea (16..w-16)
    drawRoundedBox(
      ctx,
      14,
      rwyCenterY - rwyHalfHeight,
      w - 28,
      rwyHalfHeight * 2,
      8,
      'rgba(30, 41, 59, 0.94)',
      '#00ff9d',
      2.0
    );

    // Linia centrală punctată a pistei
    ctx.save();
    ctx.strokeStyle = '#e2e8f0';
    ctx.lineWidth = 2.2;
    ctx.setLineDash([16, 12]);
    ctx.beginPath();
    ctx.moveTo(28, rwyCenterY);
    ctx.lineTo(w - 28, rwyCenterY);
    ctx.stroke();
    ctx.restore();

    ctx.fillStyle = '#00ff9d';
    ctx.font = 'bold 12px sans-serif';
    ctx.textAlign = 'left';
    ctx.fillText(pickText(arena.runway_title_ro, arena.runway_title_en), 24, rwyCenterY - rwyHalfHeight + 18);

    // Offset-uri vizuale distincte pe axa Y (lane separation) ca cele 4 avioane să aibă spațiu generos între ele
    const laneOffsetY = {{
      shield: 0,
      ins: -46,
      naive: 48,
      gnss: 98
    }};

    function mapPlaneToArenaY(key, errMeters) {{
      const clamped = Math.max(0, Math.min(230, Number(errMeters) || 0));
      if (key === 'shield') {{
        return rwyCenterY + Math.min(10, clamped * 2.0);
      }}
      if (key === 'ins') {{
        // Zboară în jumătatea superioară (deasupra axului pistei, spre iarba de sus)
        const upShift = Math.min(rwyCenterY - 36, 26 + (clamped / 120.0) * (rwyCenterY - 50));
        return rwyCenterY - upShift;
      }}
      const baseDown = rwyCenterY + laneOffsetY[key] + (clamped / 210.0) * (h - rwyCenterY - 95);
      return Math.max(36, Math.min(h - 34, baseDown));
    }}

    const progress = 1.0 - Math.max(0, Math.min(1, (s.dist_to_threshold_m || 0) / 1400.0));
    const planeBaseX = 90 + progress * (w - 290);

    const xOffsets = {{
      shield: 36,
      ins: 8,
      naive: -18,
      gnss: -44
    }};

    // 3. Momeala Hackerului 👾 în iarbă (jos-dreapta)
    const spoofMeters = Number(s.spoof_offset_m) || 0;
    const gnssY = mapPlaneToArenaY('gnss', planeErrors.gnss);
    const gnssX = Math.max(50, Math.min(w - 210, planeBaseX + xOffsets.gnss));
    const decoyX = Math.min(w - 125, gnssX + 95);
    const decoyY = Math.min(h - 32, gnssY + 26);

    if (spoofMeters > 2.0) {{
      ctx.save();
      ctx.strokeStyle = '#ff3355';
      ctx.lineWidth = 2.2;
      ctx.setLineDash([5, 5]);
      ctx.beginPath();
      ctx.moveTo(gnssX, gnssY);
      ctx.lineTo(decoyX, decoyY);
      ctx.stroke();
      ctx.restore();

      drawRoundedBox(ctx, decoyX - 88, decoyY - 14, 176, 26, 8, 'rgba(127, 29, 29, 0.92)', '#ff3355', 1.8);
      ctx.fillStyle = '#fecdd3';
      ctx.font = 'bold 10.5px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(pickText(arena.hacker_decoy_ro, arena.hacker_decoy_en), decoyX, decoyY + 3);
    }}

    // 4. Calculăm pozițiile și rezolvăm anti-coliziunea etichetelor celor 4 avioane
    const rawPlaneItems = (arena.planes || []).map(p => {{
      const errM = planeErrors[p.key] || 0;
      const py = mapPlaneToArenaY(p.key, errM);
      const px = Math.max(50, Math.min(w - 210, planeBaseX + (xOffsets[p.key] || 0)));
      return {{
        spec: p,
        errM: errM,
        px: px,
        py: py,
        labelY: py
      }};
    }});

    const safeItems = resolveLabelCollisionsY(rawPlaneItems, 34, h - 34, 34);

    safeItems.forEach(item => {{
      const p = item.spec;
      const px = item.px;
      const py = item.py;
      const ly = item.labelY;

      // Traseul neted al avionului
      ctx.save();
      ctx.strokeStyle = p.color_hex;
      ctx.lineWidth = p.key === 'shield' ? 3.2 : 2.0;
      if (p.key === 'ins') ctx.setLineDash([6, 5]);
      ctx.beginPath();
      ctx.moveTo(28, rwyCenterY);
      ctx.quadraticCurveTo(px * 0.55, (rwyCenterY + py) * 0.5, px, py);
      ctx.stroke();
      ctx.restore();

      // Punctul avionului pe traiectorie
      ctx.save();
      ctx.fillStyle = p.color_hex;
      ctx.beginPath();
      ctx.arc(px, py, p.key === 'shield' ? 6 : 4.5, 0, Math.PI * 2);
      ctx.fill();
      ctx.restore();

      // Etichetă scurtă și aerisită garantat fără suprapunere
      const shortLabel = pickText(p.label_ro, p.label_en) + ' (' + item.errM.toFixed(1) + 'm)';
      const boxW = 176;
      const boxH = 24;
      const boxX = Math.min(w - boxW - 14, px + 12);
      const boxY = ly - boxH / 2;

      drawRoundedBox(ctx, boxX, boxY, boxW, boxH, 7, 'rgba(7, 15, 28, 0.92)', p.color_hex, p.key === 'shield' ? 2.2 : 1.5);
      ctx.fillStyle = p.color_hex;
      ctx.font = 'bold 11px sans-serif';
      ctx.textAlign = 'left';
      ctx.fillText(shortLabel, boxX + 8, boxY + 16);
    }});

    // 5. Insigna 👇 UITE AICI! pe #radarCanvas
    const spotlights = VISUAL_ENGINE_SPEC.spotlights || [];
    const activeSpotlight = spotlights.find(sp => sp.step_number === window.aeroSpotlightStep6yo);
    if (activeSpotlight && activeSpotlight.target_canvas_id === 'radarCanvas') {{
      const shieldItem = safeItems.find(it => it.spec.key === 'shield') || safeItems[0];
      const focusY = activeSpotlight.focus_callout_id === 'shield_plane'
        ? Math.max(28, shieldItem.labelY - 28)
        : Math.max(28, decoyY - 32);
      const focusX = activeSpotlight.focus_callout_id === 'shield_plane'
        ? Math.min(w - 130, shieldItem.px + 60)
        : Math.max(80, decoyX);
      const ptrText = pickText(activeSpotlight.pointer_badge_ro, activeSpotlight.pointer_badge_en);
      drawRoundedBox(ctx, focusX - 54, focusY - 11, 108, 22, 10, '#fde047', '#0f172a', 1.8 + pulse);
      ctx.fillStyle = '#0f172a';
      ctx.font = 'bold 10px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(ptrText, focusX, focusY + 4);
    }}
  }};

  function renderTechnicalChartMode(ctx, w, h, s, history, arena) {{
    ctx.strokeStyle = '#1e293b';
    ctx.lineWidth = 1;
    for (let i = 1; i < 4; i++) {{
      const gy = (h / 4) * i;
      ctx.beginPath();
      ctx.moveTo(0, gy);
      ctx.lineTo(w, gy);
      ctx.stroke();
    }}

    function mapLogY(errMeters) {{
      const v = Math.max(0, Number(errMeters) || 0);
      const norm = Math.log10(1 + v) / Math.log10(251);
      return h - 22 - norm * (h - 48);
    }}

    const cat3Y = mapLogY(2.0);
    ctx.save();
    ctx.strokeStyle = 'rgba(0, 255, 157, 0.5)';
    ctx.setLineDash([6, 4]);
    ctx.beginPath();
    ctx.moveTo(0, cat3Y);
    ctx.lineTo(w, cat3Y);
    ctx.stroke();
    ctx.fillStyle = '#00ff9d';
    ctx.font = 'bold 11px sans-serif';
    ctx.textAlign = 'left';
    ctx.fillText('CAT IIIb MAX SAFE LIMIT (2.0m)', 14, cat3Y - 6);
    ctx.restore();

    function drawCurve(field, colorHex, widthPx) {{
      if (!history || history.length < 2) return;
      ctx.beginPath();
      ctx.strokeStyle = colorHex;
      ctx.lineWidth = widthPx;
      history.forEach((pt, idx) => {{
        const x = (idx / Math.max(1, history.length - 1)) * (w - 28) + 14;
        const y = mapLogY(pt[field]);
        if (idx === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }});
      ctx.stroke();
    }}

    drawCurve('gnss_err_m', '#ff3355', 2.2);
    drawCurve('naive_ekf_err_m', '#eab308', 2.2);
    drawCurve('ins_err_m', '#f97316', 2.2);
    drawCurve('shield_err_m', '#00ff9d', 3.4);

    // Legendă orizontală aerisită sus în modul grafic
    (arena.planes || []).forEach((p, idx) => {{
      const val = p.key === 'shield' ? s.shield_err_m
        : p.key === 'gnss' ? s.gnss_err_m
        : p.key === 'naive' ? s.naive_ekf_err_m
        : s.ins_err_m;
      ctx.fillStyle = p.color_hex;
      ctx.font = 'bold 11px sans-serif';
      ctx.textAlign = 'left';
      ctx.fillText(
        pickText(p.label_ro, p.label_en) + ': ' + Number(val || 0).toFixed(2) + 'm',
        16 + idx * 210,
        22
      );
    }});
  }}
}})();
"#
    )
}

/// Handler Axum care servește scriptul JS al motorului vizual (`application/javascript; charset=utf-8`).
pub async fn serve_visual_engine_js() -> Response {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        render_visual_canvases_script(),
    )
        .into_response()
}

/// Handler Axum care servește configurația JSON a motorului vizual (`VisualCanvasEngineSpec`).
pub async fn serve_visual_engine_spec_json() -> Json<VisualCanvasEngineSpec> {
    Json(build_default_visual_engine_spec())
}

/// Handler Axum care servește fragmentul SSR HTML al controalelor vizuale (`text/html; charset=utf-8`).
pub async fn serve_visual_canvases_ssr_fragment() -> Html<String> {
    Html(render_visual_canvases_component_html())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_visual_engine_spec_corners_and_airy_labels() {
        let spec = build_default_visual_engine_spec();

        // 1. Verificăm cele 3 baloane explicative pe #hudCanvas poziționate strict în colțuri
        assert_eq!(spec.hud_callouts.len(), 3);
        assert_eq!(spec.hud_callouts[0].id, "fog_cloud");
        assert_eq!(spec.hud_callouts[0].title_ro, "☁️💨 VÂNT ÎN CEAȚĂ");
        assert_eq!(spec.hud_callouts[0].box_norm, [0.02, 0.04, 0.29, 0.14]);

        assert_eq!(spec.hud_callouts[1].id, "ground_als");
        assert_eq!(
            spec.hud_callouts[1].title_ro,
            "💡 BECURILE PISTEI (VITEZA SOL)"
        );
        assert_eq!(spec.hud_callouts[1].box_norm, [0.02, 0.80, 0.31, 0.14]);

        assert_eq!(spec.hud_callouts[2].id, "our_plane");
        assert_eq!(spec.hud_callouts[2].title_ro, "✈️ AVIONUL NOSTRU (PE CENTRU)");
        assert_eq!(spec.hud_callouts[2].box_norm, [0.67, 0.04, 0.31, 0.14]);

        // 2. Verificăm etichetele scurte și aerisite ale celor 4 avioane pe #radarCanvas
        assert_eq!(spec.arena_config.planes.len(), 4);
        assert_eq!(spec.arena_config.planes[0].label_ro, "✈️🟢 AERO-SHIELD");
        assert_eq!(spec.arena_config.planes[1].label_ro, "✈️🔴 GPS PĂCĂLIT");
        assert_eq!(spec.arena_config.planes[2].label_ro, "✈️🟡 CEAȚĂ SIMPLĂ");
        assert_eq!(spec.arena_config.planes[3].label_ro, "✈️🟠 FĂRĂ CAMERĂ");
        assert!(spec.arena_config.hacker_decoy_ro.contains("👾"));
        assert!(spec.arena_config.mode_game_btn_ro.contains("🎮"));
        assert!(spec.arena_config.mode_chart_btn_ro.contains("📈"));

        // 3. Verificăm cele 5 spoturi 👇 UITE AICI! sincronizate cu pașii lecției
        assert_eq!(spec.spotlights.len(), 5);
        for (idx, spot) in spec.spotlights.iter().enumerate() {
            assert_eq!(usize::from(spot.step_number), idx + 1);
            assert_eq!(spot.pointer_badge_ro, "👇 UITE AICI!");
            assert_eq!(spot.pointer_badge_en, "👇 LOOK HERE!");
            assert!(
                spot.target_canvas_id == "hudCanvas" || spot.target_canvas_id == "radarCanvas"
            );
        }
    }

    #[test]
    fn test_render_page2_and_page3_dedicated_canvases_and_anti_collision() {
        let page2 = render_page2_hud_canvas_html();
        assert!(page2.contains("id=\"hudCanvas\""));
        assert!(page2.contains("width=\"900\" height=\"460\""));
        assert!(page2.contains("☁️💨 VÂNT ÎN CEAȚĂ"));
        assert!(page2.contains("💡 BECURILE PISTEI (VITEZA SOL)"));
        assert!(page2.contains("✈️ AVIONUL NOSTRU (PE CENTRU)"));

        let page3 = render_page3_radar_canvas_html();
        assert!(page3.contains("id=\"radarCanvas\""));
        assert!(page3.contains("width=\"900\" height=\"440\""));
        assert!(page3.contains("fourPlanesHorizontalScoreboard"));
        assert!(page3.contains("✈️🟢 AERO-SHIELD"));
        assert!(page3.contains("✈️🔴 GPS PĂCĂLIT"));
        assert!(page3.contains("✈️🟡 CEAȚĂ SIMPLĂ"));
        assert!(page3.contains("✈️🟠 FĂRĂ CAMERĂ"));

        let html = render_visual_canvases_component_html();
        assert!(html.contains("spotlightBanner6yo"));
        assert!(html.contains("👇 UITE AICI!"));
        assert!(html.contains("visualPage2Container"));
        assert!(html.contains("visualPage3Container"));

        let js = render_visual_canvases_script();
        assert!(js.contains("window.renderHudCanvas6yo"));
        assert!(js.contains("window.renderRadarCanvas6yo"));
        assert!(js.contains("window.setRadarMode6yo"));
        assert!(js.contains("window.setVisualEnginePage6yo"));
        assert!(js.contains("resolveLabelCollisionsY"));
    }
}
