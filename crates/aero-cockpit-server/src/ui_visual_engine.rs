//! Motorul vizual pe înțelesul unui copil de 6 ani pentru panourile `#hudCanvas` și `#radarCanvas`.
//!
//! Transformă telemetria avionică și fuziunea multi-senzor LRCV V2.0 într-o poveste vizuală
//! intuitivă, cu 3 baloane explicative pe camera termică (`#hudCanvas`), o arenă interactivă
//! cu 4 avioane colorate (`✈️🟢`, `✈️🔴`, `✈️🟡`, `✈️🟠`) + momeala hackerului (`👾`) pe `#radarCanvas`,
//! comutator între `🎮 Vedere Joc 4 Avioane` și `📈 Grafic Eroare (m)`, precum și un indicator
//! luminos pas-cu-pas (`👇 UITE AICI!`) sincronizat cu cele 5 lecții interactive.

use axum::{
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// Specificație tipizată pentru un balon explicativ (callout) afișat pe `#hudCanvas`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VisualCalloutSpec {
    /// Identificator unic: `"fog_cloud"`, `"ground_als"`, `"our_plane"`.
    pub id: &'static str,
    /// Pictogramă emoji ilustrativă.
    pub icon: &'static str,
    /// Culoarea tematică în format hexazecimal.
    pub color_hex: &'static str,
    /// Titlu scurt pe înțelesul unui copil de 6 ani (Română).
    pub title_ro: &'static str,
    /// Titlu scurt pe înțelesul unui copil de 6 ani (Engleză).
    pub title_en: &'static str,
    /// Subtitlu explicativ (Română).
    pub subtitle_ro: &'static str,
    /// Subtitlu explicativ (Engleză).
    pub subtitle_en: &'static str,
    /// Insignă de alertă opțională (Română), ex. când hackerul atacă GPS-ul.
    pub alert_badge_ro: Option<&'static str>,
    /// Insignă de alertă opțională (Engleză).
    pub alert_badge_en: Option<&'static str>,
    /// Coordonate normalizate `[x_frac, y_frac, w_frac, h_frac]` în intervalul `0.0..=1.0`.
    pub box_norm: [f64; 4],
    /// Punctul țintă `[tx_frac, ty_frac]` spre care indică săgeata balonului explicativ.
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
    /// Numele prietenos al avionului (Română).
    pub label_ro: &'static str,
    /// Numele prietenos al avionului (Engleză).
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

/// Configurația arenei vizuale cu 4 avioane pentru `#radarCanvas`.
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

/// Specificația completă a motorului vizual pentru `#hudCanvas` și `#radarCanvas`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VisualCanvasEngineSpec {
    /// Cele 3 baloane explicative desenate pe `#hudCanvas`.
    pub hud_callouts: [VisualCalloutSpec; 3],
    /// Configurația arenei cu 4 avioane și momeala hackerului pe `#radarCanvas`.
    pub arena_config: FourPlanesArenaConfig,
    /// Cele 5 spoturi luminoase `👇 UITE AICI!` sincronizate cu pașii lecției.
    pub spotlights: [SpotlightPointerSpec; 5],
}

/// Construiește configurația deterministă completă în Română și Engleză pentru motorul vizual.
#[must_use]
pub const fn build_default_visual_engine_spec() -> VisualCanvasEngineSpec {
    VisualCanvasEngineSpec {
        hud_callouts: [
            VisualCalloutSpec {
                id: "fog_cloud",
                icon: "☁️",
                color_hex: "#94a3b8",
                title_ro: "☁️ CEAȚĂ GROASĂ (PILOTUL NU VEDE CU OCHII)",
                title_en: "☁️ THICK FOG (PILOT CANNOT SEE OUTSIDE)",
                subtitle_ro: "Ochelarii cu infraroșu LWIR privesc direct prin norul rece!",
                subtitle_en: "LWIR thermal goggles see straight through the cold cloud!",
                alert_badge_ro: Some("👁️ OCHII CU INFRAROȘU VĂD PRIN CEAȚĂ!"),
                alert_badge_en: Some("👁️ INFRARED EYES SEE THROUGH FOG!"),
                box_norm: [0.03, 0.04, 0.44, 0.16],
                target_norm: [0.30, 0.25],
            },
            VisualCalloutSpec {
                id: "ground_als",
                icon: "🔥",
                color_hex: "#f59e0b",
                title_ro: "🔥 BECURILE CALDE DE PE PISTĂ (REPERE REALE LA SOL)",
                title_en: "🔥 WARM RUNWAY LIGHTS (REAL GROUND LANDMARKS)",
                subtitle_ro: "Becurile pistei 09 Craiova strălucesc la 80°C și nu pot fi mutate de hacker!",
                subtitle_en: "Craiova RWY 09 lamps glow warm at 80°C and cannot be moved by hackers!",
                alert_badge_ro: Some("🔒 ANCORĂ FIZICĂ SIGURĂ LA SOL"),
                alert_badge_en: Some("🔒 SAFE PHYSICAL GROUND ANCHOR"),
                box_norm: [0.03, 0.78, 0.48, 0.17],
                target_norm: [0.50, 0.62],
            },
            VisualCalloutSpec {
                id: "our_plane",
                icon: "✈️🟢",
                color_hex: "#00ff9d",
                title_ro: "✈️🟢 AVIONUL NOSTRU (MERGE DREPT PE PISTĂ)",
                title_en: "✈️🟢 OUR PLANE (FLIES STRAIGHT ON RUNWAY)",
                subtitle_ro: "Scutul AERO-SHIELD ține avionul lipit de linia luminoasă a pistei!",
                subtitle_en: "AERO-SHIELD keeps our plane locked onto the warm runway centerline!",
                alert_badge_ro: Some("👾 GPS FALS RESPINS DE SCUT!"),
                alert_badge_en: Some("👾 FAKE GPS REJECTED BY SHIELD!"),
                box_norm: [0.53, 0.04, 0.44, 0.16],
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
                    label_ro: "✈️🟢 Avionul cu Scut AERO-SHIELD (SALVAT)",
                    label_en: "✈️🟢 AERO-SHIELD Protected Plane (SAVED)",
                    story_ro: "Ignoră minciuna hackerului și aterizează drept pe centrul pistei!",
                    story_en: "Ignores the hacker's lie and lands safely on the runway centerline!",
                },
                PlaneVisualSpec {
                    key: "gnss",
                    emoji_badge: "✈️🔴",
                    color_hex: "#ff3355",
                    label_ro: "✈️🔴 Avionul păcălit de GPS (ÎN IARBĂ)",
                    label_en: "✈️🔴 Plane Tricked by Fake GPS (IN THE GRASS)",
                    story_ro: "Ascultă orbește de momeala hackerului 👾 și iese în iarbă la 200m!",
                    story_en: "Blindly follows the hacker's decoy 👾 and veers 200m into the grass!",
                },
                PlaneVisualSpec {
                    key: "naive",
                    emoji_badge: "✈️🟡",
                    color_hex: "#eab308",
                    label_ro: "✈️🟡 Avionul cu EKF Clasic (DERUTAT)",
                    label_en: "✈️🟡 Classic EKF Plane (CONFUSED)",
                    story_ro: "Amestecă adevărul cu minciuna și tot ajunge în afara pistei!",
                    story_en: "Mixes truth with the hacker's lie and still drifts off the runway!",
                },
                PlaneVisualSpec {
                    key: "ins",
                    emoji_badge: "✈️🟠",
                    color_hex: "#f97316",
                    label_ro: "✈️🟠 Avionul legat la ochi (DOAR ROȚI / INS)",
                    label_en: "✈️🟠 Blindfolded Plane (INS / WHEELS ONLY)",
                    story_ro: "Nu ascultă hackerul, dar amețește treptat fără să vadă becurile calde!",
                    story_en: "Ignores the hacker, but slowly drifts without seeing the warm lights!",
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
                explanation_6yo_ro: "Pasul 1: Afară e ceață groasă ☁️, dar camera cu infraroșu vede becurile calde 🔥 de pe pistă!",
                explanation_6yo_en: "Step 1: Outside is thick fog ☁️, but the infrared camera sees the warm runway lamps 🔥!",
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
                explanation_6yo_ro: "Pasul 3: Scutul compară minciuna GPS cu becurile calde 🔥 de pe sol și prinde hackerul pe loc!",
                explanation_6yo_en: "Step 3: The shield compares the GPS lie against the warm ground lamps 🔥 and catches the hacker immediately!",
            },
            SpotlightPointerSpec {
                step_number: 4,
                pointer_badge_ro: "👇 UITE AICI!",
                pointer_badge_en: "👇 LOOK HERE!",
                target_canvas_id: "radarCanvas",
                focus_callout_id: "shield_plane",
                explanation_6yo_ro: "Pasul 4: Avionul verde cu Scut ✈️🟢 taie sfoara hackerului și rămâne lipit de mijlocul pistei (< 0.8m)!",
                explanation_6yo_en: "Step 4: The green Shielded plane ✈️🟢 cuts the hacker's string and stays glued to the runway center (< 0.8m)!",
            },
            SpotlightPointerSpec {
                step_number: 5,
                pointer_badge_ro: "👇 UITE AICI!",
                pointer_badge_en: "👇 LOOK HERE!",
                target_canvas_id: "hudCanvas",
                focus_callout_id: "our_plane",
                explanation_6yo_ro: "Pasul 5: Aterizare 100% sigură la Craiova! Roțile ating centrul pistei, în timp ce avionul fără scut ✈️🔴 e în iarbă!",
                explanation_6yo_en: "Step 5: 100% safe landing in Craiova! Wheels touch the runway center while the unshielded red plane ✈️🔴 is in the grass!",
            },
        ],
    }
}

/// Generează fragmentul HTML SSR pentru panourile vizuale `#hudCanvas` și `#radarCanvas`,
/// incluzând comutatorul `🎮 Vedere Joc 4 Avioane` / `📈 Grafic Eroare (m)`, legenda celor 4 avioane
/// și insigna luminoasă `👇 UITE AICI!`.
#[must_use]
pub fn render_visual_canvases_component_html() -> String {
    let spec = build_default_visual_engine_spec();
    let p0 = &spec.arena_config.planes[0];
    let p1 = &spec.arena_config.planes[1];
    let p2 = &spec.arena_config.planes[2];
    let p3 = &spec.arena_config.planes[3];
    let s0 = &spec.spotlights[0];

    format!(
        r#"<div id="aeroVisualEngineRoot" class="visual-engine-root" data-active-step="1" data-radar-mode="game">
  <!-- Insigna luminoasă pas-cu-pas 👇 UITE AICI! -->
  <div id="spotlightBanner6yo" class="spotlight-banner-6yo" role="status" aria-live="polite" style="display:flex;align-items:center;gap:10px;padding:8px 14px;margin-bottom:8px;border-radius:10px;background:linear-gradient(90deg,rgba(0,255,157,0.16),rgba(56,189,248,0.14));border:1px solid rgba(0,255,157,0.45);color:#f8fafc;font-size:13px;font-weight:700;">
    <span id="spotlightBadgeText" class="spotlight-badge-pill" style="background:#eab308;color:#0f172a;padding:3px 9px;border-radius:999px;font-weight:900;letter-spacing:0.4px;animation:pulse 1.2s infinite;">{pointer_ro}</span>
    <span id="spotlightStepExplanation" style="flex:1;">{exp_ro}</span>
  </div>

  <!-- Bara de control pentru #radarCanvas: Comutator Joc 4 Avioane vs Grafic Eroare (m) -->
  <div id="radarArenaToolbar6yo" class="radar-arena-toolbar" style="display:flex;flex-wrap:wrap;align-items:center;justify-content:space-between;gap:8px;padding:6px 10px;margin-bottom:6px;border-radius:8px;background:rgba(15,23,42,0.85);border:1px solid #1e293b;">
    <div style="display:flex;align-items:center;gap:6px;">
      <button type="button" id="btnRadarModeGame" onclick="window.setRadarMode6yo && window.setRadarMode6yo('game')" style="cursor:pointer;padding:5px 11px;border-radius:7px;border:1px solid #00ff9d;background:rgba(0,255,157,0.18);color:#00ff9d;font-weight:800;font-size:12px;">
        {mode_game_ro}
      </button>
      <button type="button" id="btnRadarModeChart" onclick="window.setRadarMode6yo && window.setRadarMode6yo('chart')" style="cursor:pointer;padding:5px 11px;border-radius:7px;border:1px solid #334155;background:rgba(30,41,59,0.7);color:#cbd5e1;font-weight:700;font-size:12px;">
        {mode_chart_ro}
      </button>
    </div>
    <div id="fourPlanesMiniLegend" style="display:flex;flex-wrap:wrap;gap:8px;font-size:11px;color:#e2e8f0;">
      <span title="{p0_story}" style="padding:2px 7px;border-radius:6px;background:rgba(0,255,157,0.12);border:1px solid {p0_col};color:{p0_col};font-weight:700;">{p0_label}</span>
      <span title="{p1_story}" style="padding:2px 7px;border-radius:6px;background:rgba(255,51,85,0.12);border:1px solid {p1_col};color:{p1_col};font-weight:700;">{p1_label}</span>
      <span title="{p2_story}" style="padding:2px 7px;border-radius:6px;background:rgba(234,179,8,0.12);border:1px solid {p2_col};color:{p2_col};font-weight:700;">{p2_label}</span>
      <span title="{p3_story}" style="padding:2px 7px;border-radius:6px;background:rgba(249,115,22,0.12);border:1px solid {p3_col};color:{p3_col};font-weight:700;">{p3_label}</span>
    </div>
  </div>
</div>"#,
        pointer_ro = s0.pointer_badge_ro,
        exp_ro = s0.explanation_6yo_ro,
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

/// Generează scriptul JavaScript complet (60 FPS Canvas 2D) pentru `#hudCanvas` și `#radarCanvas`,
/// serializând direct `VisualCanvasEngineSpec` din Rust și expunând funcțiile globale:
/// - `window.renderHudCanvas6yo(ctx, canvas, state, history)`
/// - `window.renderRadarCanvas6yo(ctx, canvas, state, history)`
/// - `window.setRadarMode6yo(mode)`
/// - `window.setSpotlightLessonStep(stepNumber)`
#[must_use]
pub fn render_visual_canvases_script() -> String {
    let spec = build_default_visual_engine_spec();
    let spec_json = serde_json::to_string(&spec).unwrap_or_else(|_| "{}".to_string());

    format!(
        r#"// === AERO-SHIELD V2.0 - MOTOR VIZUAL 60 FPS PE ÎNȚELESUL UNUI COPIL DE 6 ANI ===
(function() {{
  'use strict';

  const VISUAL_ENGINE_SPEC = {spec_json};
  window.AERO_VISUAL_ENGINE_SPEC = VISUAL_ENGINE_SPEC;
  window.aeroRadarMode6yo = window.aeroRadarMode6yo || 'game'; // 'game' | 'chart'
  window.aeroSpotlightStep6yo = window.aeroSpotlightStep6yo || 1; // 1..5

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

  window.setRadarMode6yo = function(mode) {{
    window.aeroRadarMode6yo = (mode === 'chart') ? 'chart' : 'game';
    const root = document.getElementById('aeroVisualEngineRoot');
    if (root) root.setAttribute('data-radar-mode', window.aeroRadarMode6yo);
    const btnGame = document.getElementById('btnRadarModeGame');
    const btnChart = document.getElementById('btnRadarModeChart');
    if (btnGame && btnChart) {{
      const isGame = window.aeroRadarMode6yo === 'game';
      btnGame.style.borderColor = isGame ? '#00ff9d' : '#334155';
      btnGame.style.background = isGame ? 'rgba(0,255,157,0.18)' : 'rgba(30,41,59,0.7)';
      btnGame.style.color = isGame ? '#00ff9d' : '#cbd5e1';
      btnChart.style.borderColor = !isGame ? '#38bdf8' : '#334155';
      btnChart.style.background = !isGame ? 'rgba(56,189,248,0.18)' : 'rgba(30,41,59,0.7)';
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
  }};

  // === 1. RANDARE #hudCanvas PE ÎNȚELESUL UNUI COPIL DE 6 ANI ===
  window.renderHudCanvas6yo = function(ctx, canvas, s) {{
    if (!ctx || !canvas || !s) return;
    const w = canvas.width;
    const h = canvas.height;
    const cx = w / 2;
    const cy = h / 2;
    const nowMs = Date.now();
    const pulse = 0.5 + 0.5 * Math.sin(nowMs * 0.006);

    // Fundal termic LWIR + straturi de ceață groasă sus
    const grad = ctx.createLinearGradient(0, 0, 0, h);
    grad.addColorStop(0, '#07131c');
    grad.addColorStop(0.46, '#0b2230');
    grad.addColorStop(0.50, '#102e26');
    grad.addColorStop(1, '#040a0e');
    ctx.fillStyle = grad;
    ctx.fillRect(0, 0, w, h);

    // Norul de ceață groasă în jumătatea superioară
    ctx.save();
    ctx.fillStyle = 'rgba(148, 163, 184, 0.10)';
    for (let i = 0; i < 4; i++) {{
      const cloudX = ((nowMs * 0.015 + i * 190) % (w + 220)) - 110;
      const cloudY = h * (0.18 + (i % 2) * 0.07);
      ctx.beginPath();
      ctx.ellipse(cloudX, cloudY, 95, 22, 0, 0, Math.PI * 2);
      ctx.fill();
    }}
    ctx.restore();

    // Orizontul și perspectiva Pistei 09 Craiova
    ctx.strokeStyle = 'rgba(0, 255, 157, 0.35)';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, cy);
    ctx.lineTo(w, cy);
    ctx.stroke();

    const rwyShift = Math.max(-w * 0.25, Math.min(w * 0.25, -(s.shield_err_m || 0) * 6.0));
    const topW = Math.max(34, (1400 - Math.min(1250, s.dist_to_threshold_m || 0)) * 0.045);
    const botW = topW * 5.2;
    const rwyTopY = cy + 16;
    const rwyBotY = h - 18;

    // Suprafața asfaltului pistei
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

    // Linia centrală a pistei
    ctx.save();
    ctx.strokeStyle = 'rgba(0, 255, 157, 0.85)';
    ctx.lineWidth = 2;
    ctx.setLineDash([10, 8]);
    ctx.beginPath();
    ctx.moveTo(cx + rwyShift, rwyTopY);
    ctx.lineTo(cx + rwyShift, rwyBotY);
    ctx.stroke();
    ctx.restore();

    // 🔥 Becurile calde ALS de pe pistă (semnătura termică LWIR 80°C)
    const rows = 7;
    for (let r = 1; r <= rows; r++) {{
      const frac = r / rows;
      const ly = rwyTopY + (rwyBotY - rwyTopY) * frac;
      const halfSpan = topW + (botW - topW) * frac;
      const glowRadius = 2.5 + frac * 3.5;

      // Becuri margine stânga/dreapta + centru
      const lampPositions = [
        cx + rwyShift - halfSpan,
        cx + rwyShift + halfSpan,
        cx + rwyShift
      ];
      lampPositions.forEach((lx, idx) => {{
        ctx.beginPath();
        ctx.fillStyle = idx === 2 ? '#fde047' : '#fb923c';
        ctx.arc(lx, ly, glowRadius, 0, Math.PI * 2);
        ctx.fill();
      }});
    }}

    // Dacă hackerul atacă GPS-ul, desenăm fantoma roșie a pistei false în dreapta
    if (s.spoof_offset_m > 5.0) {{
      const ghostShift = Math.min(w * 0.34, s.spoof_offset_m * 0.95);
      ctx.save();
      ctx.strokeStyle = 'rgba(255, 51, 85, 0.75)';
      ctx.setLineDash([6, 5]);
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(cx + ghostShift - topW * 0.8, rwyTopY + 12);
      ctx.lineTo(cx + ghostShift + topW * 0.8, rwyTopY + 12);
      ctx.lineTo(cx + ghostShift + botW * 0.75, rwyBotY);
      ctx.lineTo(cx + ghostShift - botW * 0.75, rwyBotY);
      ctx.closePath();
      ctx.stroke();

      const ghostLabel = pickText(
        '👾 PISTA FALSĂ A HACKERULUI (+' + s.spoof_offset_m.toFixed(0) + 'm ÎN IARBĂ)',
        '👾 HACKER FAKE RUNWAY (+' + s.spoof_offset_m.toFixed(0) + 'm IN GRASS)'
      );
      drawRoundedBox(ctx, cx + ghostShift - 115, cy + 36, 230, 24, 6, 'rgba(127, 29, 29, 0.88)', '#ff3355', 1.5);
      ctx.fillStyle = '#fecdd3';
      ctx.font = 'bold 10px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(ghostLabel, cx + ghostShift, cy + 52);
      ctx.restore();
    }}

    // ✈️🟢 Avionul nostru în centrul țintei optice
    drawRoundedBox(ctx, cx - 26, cy - 18, 52, 34, 8, 'rgba(0, 255, 157, 0.16)', '#00ff9d', 2);
    ctx.font = '18px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText('✈️🟢', cx, cy + 6);

    // Cele 3 baloane explicative (Callout-uri vizuale) pentru un copil de 6 ani
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

      // Săgeată punctată de la balon la elementul din imagine
      ctx.save();
      ctx.strokeStyle = c.color_hex;
      ctx.lineWidth = isFocused ? 2.5 : 1.4;
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      ctx.moveTo(bx + bw * 0.5, by + bh * 0.5);
      ctx.lineTo(tx, ty);
      ctx.stroke();
      ctx.restore();

      // Cutia balonului explicativ
      const borderCol = isFocused ? '#fde047' : c.color_hex;
      const borderWidth = isFocused ? (2.2 + pulse * 1.5) : 1.4;
      drawRoundedBox(ctx, bx, by, bw, bh, 8, 'rgba(15, 23, 42, 0.88)', borderCol, borderWidth);

      ctx.save();
      ctx.textAlign = 'left';
      ctx.fillStyle = c.color_hex;
      ctx.font = 'bold 11px sans-serif';
      ctx.fillText(pickText(c.title_ro, c.title_en), bx + 8, by + 17);

      ctx.fillStyle = '#e2e8f0';
      ctx.font = '10px sans-serif';
      ctx.fillText(pickText(c.subtitle_ro, c.subtitle_en), bx + 8, by + 33);

      const badgeText = pickText(c.alert_badge_ro, c.alert_badge_en);
      if (badgeText) {{
        ctx.fillStyle = '#fde047';
        ctx.font = 'bold 9.5px sans-serif';
        ctx.fillText(badgeText, bx + 8, by + 48);
      }}

      // Insigna 👇 UITE AICI! direct pe balonul activ din pasul lecției
      if (isFocused && activeSpotlight) {{
        const ptrText = pickText(activeSpotlight.pointer_badge_ro, activeSpotlight.pointer_badge_en);
        drawRoundedBox(ctx, bx + bw - 98, by - 11, 94, 19, 9, '#fde047', '#0f172a', 1.5);
        ctx.fillStyle = '#0f172a';
        ctx.font = 'bold 9.5px sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(ptrText, bx + bw - 51, by + 2);
      }}
      ctx.restore();
    }});
  }};

  // === 2. RANDARE #radarCanvas: VEDERE JOC CU 4 AVIOANE vs GRAFIC EROARE (m) ===
  window.renderRadarCanvas6yo = function(ctx, canvas, s, history) {{
    if (!ctx || !canvas || !s) return;
    const w = canvas.width;
    const h = canvas.height;
    const arena = VISUAL_ENGINE_SPEC.arena_config;
    const nowMs = Date.now();
    const pulse = 0.5 + 0.5 * Math.sin(nowMs * 0.007);

    ctx.fillStyle = '#060c14';
    ctx.fillRect(0, 0, w, h);

    if (window.aeroRadarMode6yo === 'chart') {{
      renderTechnicalChartMode(ctx, w, h, s, history || [], arena);
      return;
    }}

    // === MODUL IMPLICIT: 🎮 VEDERE JOC 4 AVIOANE DE SUS (TOP-DOWN RUNWAY ARENA) ===
    const rwyCenterY = h * 0.34;
    const rwyHalfHeight = h * 0.15;

    // 1. Zonele de IARBĂ (Pericol de ieșire de pe pistă) sus și jos
    ctx.fillStyle = 'rgba(20, 83, 45, 0.32)';
    ctx.fillRect(0, 0, w, rwyCenterY - rwyHalfHeight);
    ctx.fillRect(0, rwyCenterY + rwyHalfHeight, w, h - (rwyCenterY + rwyHalfHeight));

    // Marcaj text pentru zona de iarbă
    ctx.fillStyle = '#86efac';
    ctx.font = 'bold 10px sans-serif';
    ctx.textAlign = 'left';
    ctx.fillText(pickText(arena.grass_danger_ro, arena.grass_danger_en), 12, h - 10);

    // 2. Banda gri a Pistei 09 Craiova pe mijloc
    drawRoundedBox(
      ctx,
      10,
      rwyCenterY - rwyHalfHeight,
      w - 20,
      rwyHalfHeight * 2,
      6,
      'rgba(30, 41, 59, 0.92)',
      '#00ff9d',
      1.8
    );

    // Linia centrală punctată a pistei
    ctx.save();
    ctx.strokeStyle = '#e2e8f0';
    ctx.lineWidth = 2;
    ctx.setLineDash([14, 10]);
    ctx.beginPath();
    ctx.moveTo(24, rwyCenterY);
    ctx.lineTo(w - 24, rwyCenterY);
    ctx.stroke();
    ctx.restore();

    // Titlul pistei sigure
    ctx.fillStyle = '#00ff9d';
    ctx.font = 'bold 11px sans-serif';
    ctx.textAlign = 'left';
    ctx.fillText(pickText(arena.runway_title_ro, arena.runway_title_en), 18, rwyCenterY - rwyHalfHeight + 15);

    // Funcție de mapare a abaterii în metri (0..220m) pe axa verticală Y a arenei
    function mapErrMetersToArenaY(errMeters) {{
      const clamped = Math.max(0, Math.min(230, Number(errMeters) || 0));
      if (clamped <= 15) {{
        // În interiorul pistei (0..15m)
        return rwyCenterY + (clamped / 15.0) * (rwyHalfHeight * 0.72);
      }}
      // În afara pistei, în iarbă (15..230m)
      const grassSpan = h - (rwyCenterY + rwyHalfHeight) - 24;
      const normGrass = (clamped - 15.0) / 215.0;
      return rwyCenterY + rwyHalfHeight + 10 + normGrass * Math.max(20, grassSpan);
    }}

    // Poziția orizontală X a avioanelor în funcție de progresul apropierii spre pragul pistei
    const progress = 1.0 - Math.max(0, Math.min(1, (s.dist_to_threshold_m || 0) / 1400.0));
    const planeBaseX = 55 + progress * (w - 210);

    // 3. Momeala Hackerului 👾 (dacă atacul GPS Spoofing este activ)
    const spoofMeters = Number(s.spoof_offset_m) || 0;
    const decoyY = mapErrMetersToArenaY(Math.max(spoofMeters, 180));
    const decoyX = Math.min(w - 95, planeBaseX + 55);

    if (spoofMeters > 2.0) {{
      // Sfoară roșie punctată cu care hackerul trage avionul fără scut în iarbă
      const gnssY = mapErrMetersToArenaY(s.gnss_err_m);
      ctx.save();
      ctx.strokeStyle = '#ff3355';
      ctx.lineWidth = 2;
      ctx.setLineDash([5, 5]);
      ctx.beginPath();
      ctx.moveTo(planeBaseX, gnssY);
      ctx.lineTo(decoyX, decoyY);
      ctx.stroke();
      ctx.restore();

      drawRoundedBox(ctx, decoyX - 78, decoyY - 16, 156, 28, 8, 'rgba(127, 29, 29, 0.92)', '#ff3355', 1.8);
      ctx.fillStyle = '#fecdd3';
      ctx.font = 'bold 10px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(pickText(arena.hacker_decoy_ro, arena.hacker_decoy_en), decoyX, decoyY + 2);
    }}

    // 4. Desenarea celor 4 avioane și a urmelor lor
    const planeErrors = {{
      shield: Number(s.shield_err_m) || 0,
      ins: Number(s.ins_err_m) || 0,
      naive: Number(s.naive_ekf_err_m) || 0,
      gnss: Number(s.gnss_err_m) || 0
    }};

    const xOffsets = {{
      shield: 18,
      ins: 6,
      naive: -8,
      gnss: -20
    }};

    (arena.planes || []).forEach(p => {{
      const errM = planeErrors[p.key] || 0;
      const py = mapErrMetersToArenaY(errM);
      const px = Math.max(36, Math.min(w - 150, planeBaseX + (xOffsets[p.key] || 0)));

      // Traseul punctat al avionului
      ctx.save();
      ctx.strokeStyle = p.color_hex;
      ctx.lineWidth = p.key === 'shield' ? 2.8 : 1.8;
      ctx.beginPath();
      ctx.moveTo(24, rwyCenterY);
      ctx.quadraticCurveTo(px * 0.55, (rwyCenterY + py) * 0.5, px, py);
      ctx.stroke();
      ctx.restore();

      // Insigna avionului + eroarea în metri
      const pillText = p.emoji_badge + ' ' + errM.toFixed(1) + 'm';
      drawRoundedBox(ctx, px - 28, py - 11, 138, 22, 7, 'rgba(15, 23, 42, 0.92)', p.color_hex, p.key === 'shield' ? 2.2 : 1.4);
      ctx.fillStyle = p.color_hex;
      ctx.font = 'bold 10.5px sans-serif';
      ctx.textAlign = 'left';
      ctx.fillText(pillText, px - 22, py + 4);
    }});

    // 5. Insigna 👇 UITE AICI! pe #radarCanvas dacă pasul curent vizează radarCanvas
    const spotlights = VISUAL_ENGINE_SPEC.spotlights || [];
    const activeSpotlight = spotlights.find(sp => sp.step_number === window.aeroSpotlightStep6yo);
    if (activeSpotlight && activeSpotlight.target_canvas_id === 'radarCanvas') {{
      const focusY = activeSpotlight.focus_callout_id === 'shield_plane'
        ? mapErrMetersToArenaY(planeErrors.shield) - 26
        : Math.max(24, decoyY - 34);
      const focusX = activeSpotlight.focus_callout_id === 'shield_plane'
        ? Math.min(w - 120, planeBaseX + 20)
        : Math.max(70, decoyX - 10);
      const ptrText = pickText(activeSpotlight.pointer_badge_ro, activeSpotlight.pointer_badge_en);
      drawRoundedBox(ctx, focusX - 52, focusY - 12, 104, 22, 10, '#fde047', '#0f172a', 1.8 + pulse);
      ctx.fillStyle = '#0f172a';
      ctx.font = 'bold 10px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(ptrText, focusX, focusY + 3);
    }}
  }};

  // Mod secundar pentru juriul tehnic: 📈 Grafic Eroare (m) cu pragul CAT IIIb (2.0m)
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
      return h - 18 - norm * (h - 38);
    }}

    // Pragul de siguranță CAT IIIb (2.0m)
    const cat3Y = mapLogY(2.0);
    ctx.save();
    ctx.strokeStyle = 'rgba(0, 255, 157, 0.45)';
    ctx.setLineDash([6, 4]);
    ctx.beginPath();
    ctx.moveTo(0, cat3Y);
    ctx.lineTo(w, cat3Y);
    ctx.stroke();
    ctx.fillStyle = '#00ff9d';
    ctx.font = 'bold 10px sans-serif';
    ctx.textAlign = 'left';
    ctx.fillText('CAT IIIb MAX SAFE LIMIT (2.0m)', 10, cat3Y - 5);
    ctx.restore();

    function drawCurve(field, colorHex, widthPx) {{
      if (!history || history.length < 2) return;
      ctx.beginPath();
      ctx.strokeStyle = colorHex;
      ctx.lineWidth = widthPx;
      history.forEach((pt, idx) => {{
        const x = (idx / Math.max(1, history.length - 1)) * (w - 20) + 10;
        const y = mapLogY(pt[field]);
        if (idx === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }});
      ctx.stroke();
    }}

    drawCurve('gnss_err_m', '#ff3355', 2.0);
    drawCurve('naive_ekf_err_m', '#eab308', 2.0);
    drawCurve('ins_err_m', '#f97316', 2.0);
    drawCurve('shield_err_m', '#00ff9d', 3.0);

    // Sumar live în colțul stânga-sus
    (arena.planes || []).forEach((p, idx) => {{
      const val = p.key === 'shield' ? s.shield_err_m
        : p.key === 'gnss' ? s.gnss_err_m
        : p.key === 'naive' ? s.naive_ekf_err_m
        : s.ins_err_m;
      ctx.fillStyle = p.color_hex;
      ctx.font = 'bold 10.5px sans-serif';
      ctx.textAlign = 'left';
      ctx.fillText(
        pickText(p.label_ro, p.label_en) + ': ' + Number(val || 0).toFixed(2) + 'm',
        12,
        18 + idx * 15
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
    fn test_default_visual_engine_spec_completeness() {
        let spec = build_default_visual_engine_spec();

        // 1. Verificăm cele 3 baloane explicative pe #hudCanvas
        assert_eq!(spec.hud_callouts.len(), 3);
        assert_eq!(spec.hud_callouts[0].id, "fog_cloud");
        assert!(spec.hud_callouts[0].title_ro.contains("CEAȚĂ GROASĂ"));
        assert_eq!(spec.hud_callouts[1].id, "ground_als");
        assert!(spec.hud_callouts[1].title_ro.contains("BECURILE CALDE"));
        assert_eq!(spec.hud_callouts[2].id, "our_plane");
        assert!(spec.hud_callouts[2].title_ro.contains("AVIONUL NOSTRU"));

        // 2. Verificăm cele 4 avioane și momeala hackerului pe #radarCanvas
        assert_eq!(spec.arena_config.planes.len(), 4);
        assert_eq!(spec.arena_config.planes[0].emoji_badge, "✈️🟢");
        assert_eq!(spec.arena_config.planes[1].emoji_badge, "✈️🔴");
        assert_eq!(spec.arena_config.planes[2].emoji_badge, "✈️🟡");
        assert_eq!(spec.arena_config.planes[3].emoji_badge, "✈️🟠");
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
    fn test_render_visual_canvases_ssr_and_js_zero_panic() {
        let html = render_visual_canvases_component_html();
        assert!(html.contains("spotlightBanner6yo"));
        assert!(html.contains("👇 UITE AICI!"));
        assert!(html.contains("🎮 Vedere Joc 4 Avioane"));
        assert!(html.contains("📈 Grafic Eroare (m)"));
        assert!(html.contains("✈️🟢"));
        assert!(html.contains("✈️🔴"));
        assert!(html.contains("✈️🟡"));
        assert!(html.contains("✈️🟠"));

        let js = render_visual_canvases_script();
        assert!(js.contains("window.renderHudCanvas6yo"));
        assert!(js.contains("window.renderRadarCanvas6yo"));
        assert!(js.contains("window.setRadarMode6yo"));
        assert!(js.contains("window.setSpotlightLessonStep"));
        assert!(js.contains("MOMEALA HACKERULUI"));
    }
}
