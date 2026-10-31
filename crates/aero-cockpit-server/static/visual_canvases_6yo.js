// =============================================================================
// AERO-SHIELD CRAIOVA 2026 — ZERO MOVING TEXT & MAX CONTRAST (#050811 / #FFFFFF / #FEF08A)
// =============================================================================
// 1. ZERO TEXTE MIȘCĂTOARE PE #hudCanvas ȘI #radarCanvas:
//    - Pe #radarCanvas: pe pistă se mișcă DOAR siluetele avioanelor cu un cerc mic
//      având numărul fix (1, 2, 3, 4 scris cu #FFFFFF pe fond opac #050811).
//      Numele și explicațiile ([1] ✈️🟢 AERO-SHIELD, [2] ✈️🔴 GPS PĂCĂLIT,
//      [3] ✈️🟡 CEAȚĂ SIMPLĂ, [4] ✈️🟠 FĂRĂ CAMERĂ) stau într-o coloană 100% FIXĂ,
//      STATICĂ în stânga și în bara orizontală.
//    - Pe #hudCanvas: zero texte pe scala rotită, zero texte care urmăresc crabPx,
//      zero Math.sin / pulsații. Toate casetele stau 100% FIXE în colțuri!
// 2. CONTRAST MAXIM:
//    - Fundal 100% OPAC #050811 pe toate casetele de text, bordură groasă de 2px
//      (#00ff9d, #00e5ff, #f0abfc, #ffb800, #ff2a5f) și text ALB PUR (#FFFFFF)
//      + GALBEN STRĂLUCITOR (#FEF08A).

(function () {
  "use strict";

  const SOLID_BG_DARK = "#050811";
  const TEXT_PURE_WHITE = "#FFFFFF";
  const TEXT_BRIGHT_YELLOW = "#FEF08A";

  let radarViewMode6yo = "game"; // "game" | "chart"
  let radarToggleBound = false;
  let errorHistory6yo = []; // Rolling buffer of { shield, gnss, naive, ins }

  function getLang() {
    return typeof currentLang !== "undefined" && currentLang === "en" ? "en" : "ro";
  }

  function drawRoundRect(ctx, x, y, w, h, r) {
    const radius = Math.min(r, w / 2, h / 2);
    ctx.beginPath();
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
  }

  // Draws a 100% static, 100% opaque (#050811) corner callout bubble with 2px border
  function drawCornerCalloutBubble(ctx, opts) {
    const {
      x,
      y,
      w,
      h,
      targetX,
      targetY,
      borderColor,
      title,
      subtitle,
      badge,
    } = opts;

    ctx.save();

    if (typeof targetX === "number" && typeof targetY === "number") {
      const anchorX = targetX > x + w / 2 ? x + w : x;
      const anchorY = targetY > y + h / 2 ? y + h * 0.72 : y + h * 0.28;
      ctx.strokeStyle = borderColor || "#00e5ff";
      ctx.lineWidth = 2.0;
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      ctx.moveTo(anchorX, anchorY);
      ctx.lineTo(targetX, targetY);
      ctx.stroke();
      ctx.setLineDash([]);

      ctx.fillStyle = borderColor || "#00e5ff";
      ctx.beginPath();
      ctx.arc(targetX, targetY, 4.0, 0, Math.PI * 2);
      ctx.fill();
    }

    drawRoundRect(ctx, x, y, w, h, 9);
    ctx.fillStyle = SOLID_BG_DARK;
    ctx.fill();
    ctx.lineWidth = 2.2;
    ctx.strokeStyle = borderColor || "#00e5ff";
    ctx.stroke();

    // Title in PURE WHITE (#FFFFFF)
    ctx.fillStyle = TEXT_PURE_WHITE;
    ctx.font = "bold 12.5px 'Segoe UI', system-ui, sans-serif";
    ctx.fillText(title, x + 10, y + 21);

    // Subtitle in BRIGHT YELLOW (#FEF08A)
    if (subtitle) {
      ctx.fillStyle = TEXT_BRIGHT_YELLOW;
      ctx.font = "bold 11px 'Segoe UI', system-ui, sans-serif";
      ctx.fillText(subtitle, x + 10, y + 39);
    }

    // Badge in PURE WHITE (#FFFFFF)
    if (badge) {
      ctx.fillStyle = TEXT_PURE_WHITE;
      ctx.font = "bold 10.5px 'Segoe UI', system-ui, sans-serif";
      ctx.fillText(badge, x + 10, y + 55);
    }

    ctx.restore();
  }

  function drawArrow(ctx, x1, y1, x2, y2, color, lineWidth = 2.0, headLen = 6.5) {
    const dx = x2 - x1;
    const dy = y2 - y1;
    const len = Math.hypot(dx, dy);
    if (len < 1) return;
    const angle = Math.atan2(dy, dx);

    ctx.save();
    ctx.strokeStyle = color;
    ctx.fillStyle = color;
    ctx.lineWidth = lineWidth;

    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.lineTo(x2, y2);
    ctx.stroke();

    ctx.beginPath();
    ctx.moveTo(x2, y2);
    ctx.lineTo(
      x2 - headLen * Math.cos(angle - Math.PI / 6),
      y2 - headLen * Math.sin(angle - Math.PI / 6)
    );
    ctx.lineTo(
      x2 - headLen * Math.cos(angle + Math.PI / 6),
      y2 - headLen * Math.sin(angle + Math.PI / 6)
    );
    ctx.closePath();
    ctx.fill();
    ctx.restore();
  }

  // ===========================================================================
  // 1. PAGE 2: HUD CANVAS ("PARBRIZUL AVIONULUI", 900x460px, ZERO MOVING TEXT)
  // ===========================================================================
  function renderHudCanvas6yo() {
    const canvas = document.getElementById("hudCanvas");
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const targetW = rect.width > 100 ? Math.round(rect.width) : 900;
    const targetH = rect.height > 100 ? Math.round(rect.height) : 460;
    if (canvas.width !== targetW || canvas.height !== targetH) {
      canvas.width = targetW;
      canvas.height = targetH;
    }
    const ctx = canvas.getContext("2d");
    const w = canvas.width;
    const h = canvas.height;

    ctx.fillStyle = SOLID_BG_DARK;
    ctx.fillRect(0, 0, w, h);

    const lang = getLang();

    if (typeof latestFrame === "undefined" || !latestFrame) {
      drawRoundRect(ctx, 20, 20, 480, 44, 8);
      ctx.fillStyle = SOLID_BG_DARK;
      ctx.fill();
      ctx.strokeStyle = "#00ff9d";
      ctx.lineWidth = 2.0;
      ctx.stroke();
      ctx.fillStyle = TEXT_PURE_WHITE;
      ctx.font = "bold 14px 'Segoe UI', monospace";
      ctx.fillText(
        lang === "ro"
          ? "⏳ PORNIM CAMERA AVIONULUI (60 POZE / SECUNDĂ)..."
          : "⏳ STARTING AIRPLANE CAMERA (60 FRAMES / SECOND)...",
        34,
        47
      );
      return;
    }

    const p = latestFrame.aircraft_pose;
    const m = latestFrame.fog_and_meteo;
    const g = latestFrame.lrcv_guidance;
    const e = latestFrame.estimators_arena;
    const rollDeg = p.euler_deg[0] || 0;
    const pitchDeg = p.euler_deg[1] || 0;
    const yawDeg = p.euler_deg[2] || 0;
    const rollRad = (rollDeg * Math.PI) / 180.0;

    // -------------------------------------------------------------------------
    // STEP 1: 3D Sky, Ground, Horizon & Runway 09 Craiova (NO ROTATED TEXT!)
    // -------------------------------------------------------------------------
    ctx.save();
    ctx.translate(w / 2, h / 2);
    ctx.rotate(-rollRad);

    const pitchOffsetPx = pitchDeg * 6.0;

    // Sky
    const skyGrad = ctx.createLinearGradient(0, -h, 0, pitchOffsetPx);
    skyGrad.addColorStop(0, "#050811");
    skyGrad.addColorStop(1, "#0d1f3a");
    ctx.fillStyle = skyGrad;
    ctx.fillRect(-w, -h, w * 2, h + pitchOffsetPx);

    // Ground
    const gndGrad = ctx.createLinearGradient(0, pitchOffsetPx, 0, h);
    gndGrad.addColorStop(0, "#081f17");
    gndGrad.addColorStop(1, "#050811");
    ctx.fillStyle = gndGrad;
    ctx.fillRect(-w, pitchOffsetPx, w * 2, h * 2);

    // Horizon Line
    ctx.strokeStyle = "#00ff9d";
    ctx.lineWidth = 2.0;
    ctx.beginPath();
    ctx.moveTo(-w, pitchOffsetPx);
    ctx.lineTo(w, pitchOffsetPx);
    ctx.stroke();

    // Pitch ladder guide lines ONLY (zero rotated text!)
    ctx.strokeStyle = "#00e5ff";
    ctx.lineWidth = 1.6;
    for (let step = -2; step <= 2; step++) {
      if (step === 0) continue;
      const py = pitchOffsetPx + step * 30;
      ctx.beginPath();
      ctx.moveTo(-40, py);
      ctx.lineTo(-16, py);
      ctx.moveTo(16, py);
      ctx.lineTo(40, py);
      ctx.stroke();
    }

    // Perspective Runway RWY 09 Craiova
    const lateralShiftPx = -p.pos_ned_m[1] * 2.2 - yawDeg * 4.5;
    const rwyTopY = pitchOffsetPx + 8;
    const rwyBotY = h * 0.88;
    const topHalfW = 26;
    const botHalfW = 215;

    ctx.fillStyle = "#0b1a2a";
    ctx.strokeStyle = "#00e5ff";
    ctx.lineWidth = 2.4;
    ctx.beginPath();
    ctx.moveTo(lateralShiftPx - topHalfW, rwyTopY);
    ctx.lineTo(lateralShiftPx + topHalfW, rwyTopY);
    ctx.lineTo(lateralShiftPx + botHalfW, rwyBotY);
    ctx.lineTo(lateralShiftPx - botHalfW, rwyBotY);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();

    // Centerline dashed
    ctx.strokeStyle = TEXT_PURE_WHITE;
    ctx.lineWidth = 2.2;
    ctx.setLineDash([14, 10]);
    ctx.beginPath();
    ctx.moveTo(lateralShiftPx, rwyTopY);
    ctx.lineTo(lateralShiftPx, rwyBotY);
    ctx.stroke();
    ctx.setLineDash([]);

    // Warm ALS Approach Light Bars (pure visual bulbs, zero moving text)
    const numBars = Math.min(g.als_bars_detected || 8, 11);
    for (let i = 1; i <= numBars; i++) {
      const frac = i / (numBars + 1);
      const by = rwyTopY + frac * (rwyBotY - rwyTopY) * 0.82;
      const barHalfW = topHalfW + frac * (botHalfW - topHalfW) * 0.48;
      const isRedWing = i % 3 === 0;
      ctx.strokeStyle = isRedWing ? "#ff2a5f" : "#00ff9d";
      ctx.lineWidth = 2.6;
      ctx.beginPath();
      ctx.moveTo(lateralShiftPx - barHalfW, by);
      ctx.lineTo(lateralShiftPx + barHalfW, by);
      ctx.stroke();

      for (let b = -2; b <= 2; b++) {
        const bx = lateralShiftPx + (b / 2) * barHalfW;
        ctx.fillStyle = b === 0 ? TEXT_BRIGHT_YELLOW : isRedWing ? "#ff2a5f" : "#00ff9d";
        ctx.beginPath();
        ctx.arc(bx, by, 2.5 + frac * 1.6, 0, Math.PI * 2);
        ctx.fill();
      }
    }

    ctx.restore();

    // -------------------------------------------------------------------------
    // STEP 2: Optical Flow Arrows (Purple = Fog Wind, Green = Ground)
    // -------------------------------------------------------------------------
    if (latestFrame.optical_flow_vectors && latestFrame.optical_flow_vectors.length > 0) {
      for (const vec of latestFrame.optical_flow_vectors) {
        const sx = vec.x_norm * w;
        const sy = vec.y_norm * h;
        const ex = sx + vec.dx_px * 1.2;
        const ey = sy + vec.dy_px * 1.2;
        const isFog = vec.layer === "fog";
        const color = isFog ? "#f0abfc" : "#00ff9d";
        drawArrow(ctx, sx, sy, ex, ey, color, isFog ? 2.2 : 2.0, 6.0);
      }
    }

    // -------------------------------------------------------------------------
    // STEP 3: Real Video AI Overlay (if active)
    // -------------------------------------------------------------------------
    if (
      typeof mainViewMode !== "undefined" &&
      mainViewMode !== "sim" &&
      typeof videoOpacityPct !== "undefined" &&
      videoOpacityPct > 0 &&
      typeof renderRealVideoAiOverlay === "function"
    ) {
      renderRealVideoAiOverlay(ctx, w, h);
    }

    // -------------------------------------------------------------------------
    // STEP 4: Fixed Airplane Reticle in Center (NO moving crab text!)
    // -------------------------------------------------------------------------
    const cx = w / 2;
    const cy = h / 2;

    ctx.strokeStyle = "#00e5ff";
    ctx.lineWidth = 2.8;
    ctx.beginPath();
    ctx.moveTo(cx - 28, cy);
    ctx.lineTo(cx - 8, cy);
    ctx.lineTo(cx - 8, cy + 7);
    ctx.moveTo(cx + 8, cy);
    ctx.lineTo(cx + 28, cy);
    ctx.lineTo(cx + 8, cy + 7);
    ctx.stroke();

    // -------------------------------------------------------------------------
    // STEP 5: THE 3 CORNER CALLOUT BUBBLES (100% FIXED POSITIONS & TARGETS, #050811 OPAC)
    // -------------------------------------------------------------------------
    const windSpeedLat = Math.abs(m.true_fog_wind_ned_mps[1] || 0);
    const groundSpeed = p.vel_body_mps[0] || 0;
    const shieldErr = e && typeof e.aero_shield_error_m === "number" ? e.aero_shield_error_m : 0.3;
    const crabDeg = p.crab_angle_cmd_deg || 0;

    // 1. TOP-LEFT CORNER: `fog_cloud` (`box_norm: [0.02, 0.04, 0.29, 0.14]`)
    const b1X = Math.round(w * 0.02);
    const b1Y = Math.round(h * 0.04);
    const b1W = Math.max(245, Math.round(w * 0.29));
    const b1H = Math.max(62, Math.round(h * 0.14));
    const gustAlert =
      m.gust_lookahead_warning || windSpeedLat > 8.0
        ? lang === "ro"
          ? `⚠️ RAFALĂ ÎN ${(m.gust_lookahead_lead_sec || 0.9).toFixed(1)}s (Vânt: ${windSpeedLat.toFixed(1)} m/s)`
          : `⚠️ GUST IN ${(m.gust_lookahead_lead_sec || 0.9).toFixed(1)}s (Wind: ${windSpeedLat.toFixed(1)} m/s)`
        : lang === "ro"
          ? `Vânt lateral în ceață: ${windSpeedLat.toFixed(1)} m/s`
          : `Fog crosswind: ${windSpeedLat.toFixed(1)} m/s`;

    drawCornerCalloutBubble(ctx, {
      x: b1X,
      y: b1Y,
      w: b1W,
      h: b1H,
      targetX: Math.round(w * 0.28),
      targetY: Math.round(h * 0.24),
      borderColor: "#f0abfc",
      title: lang === "ro" ? "☁️💨 VÂNT ÎN CEAȚĂ" : "☁️💨 FOG WIND",
      subtitle:
        lang === "ro"
          ? "Săgețile mov arată vântul din nor"
          : "Purple arrows show wind in fog",
      badge: gustAlert,
    });

    // 2. BOTTOM-LEFT CORNER: `ground_als` (`box_norm: [0.02, 0.80, 0.31, 0.14]`)
    const b2X = Math.round(w * 0.02);
    const b2Y = Math.round(h * 0.80);
    const b2W = Math.max(260, Math.round(w * 0.31));
    const b2H = Math.max(62, Math.round(h * 0.14));
    drawCornerCalloutBubble(ctx, {
      x: b2X,
      y: b2Y,
      w: b2W,
      h: b2H,
      targetX: Math.round(w * 0.46),
      targetY: Math.round(h * 0.68),
      borderColor: "#00ff9d",
      title:
        lang === "ro"
          ? "💡 BECURILE PISTEI (VITEZA SOL)"
          : "💡 RUNWAY LIGHTS (GROUND SPEED)",
      subtitle:
        lang === "ro"
          ? `Repere fixe la sol (${groundSpeed.toFixed(1)} m/s)`
          : `Fixed ground lights (${groundSpeed.toFixed(1)} m/s)`,
      badge:
        lang === "ro"
          ? "🔒 ANCORĂ FIZICĂ PE PISTA 09"
          : "🔒 SAFE GROUND ANCHOR RWY 09",
    });

    // 3. TOP-RIGHT CORNER: `our_plane` (`box_norm: [0.67, 0.04, 0.31, 0.14]`)
    const b3W = Math.max(265, Math.round(w * 0.31));
    const b3H = Math.max(62, Math.round(h * 0.14));
    const b3X = Math.round(w - b3W - w * 0.02);
    const b3Y = Math.round(h * 0.04);
    const planeStatusBadge =
      lang === "ro"
        ? `✅ Eroare: ${shieldErr.toFixed(1)}m • Înclinare: ${rollDeg.toFixed(0)}° • Crab: ${crabDeg.toFixed(1)}°`
        : `✅ Error: ${shieldErr.toFixed(1)}m • Bank: ${rollDeg.toFixed(0)}° • Crab: ${crabDeg.toFixed(1)}°`;

    drawCornerCalloutBubble(ctx, {
      x: b3X,
      y: b3Y,
      w: b3W,
      h: b3H,
      targetX: Math.round(cx),
      targetY: Math.round(cy),
      borderColor: "#00e5ff",
      title:
        lang === "ro"
          ? "✈️ AVIONUL NOSTRU (PE CENTRU)"
          : "✈️ OUR PLANE (ON CENTERLINE)",
      subtitle:
        lang === "ro"
          ? `Alt: ${(-p.pos_ned_m[2]).toFixed(0)}m • Vizibilitate: ${m.visibility_mor_m.toFixed(0)}m`
          : `Alt: ${(-p.pos_ned_m[2]).toFixed(0)}m • Visibility: ${m.visibility_mor_m.toFixed(0)}m`,
      badge: planeStatusBadge,
    });
  }

  // ===========================================================================
  // 2. PAGE 3: RADAR CANVAS ("CURSA CELOR 4 AVIOANE", 900x440px, ZERO MOVING TEXT)
  // ===========================================================================
  function setRadarMode6yo(mode) {
    radarViewMode6yo = mode === "chart" ? "chart" : "game";
    const btnGame =
      document.getElementById("btnRadarModeGame6yo") ||
      document.getElementById("btnRadarModeGame");
    const btnChart =
      document.getElementById("btnRadarModeChart6yo") ||
      document.getElementById("btnRadarModeChart");
    if (btnGame && btnChart) {
      btnGame.style.background = SOLID_BG_DARK;
      btnGame.style.borderColor = radarViewMode6yo === "game" ? "#00ff9d" : "#ffb800";
      btnGame.style.color = radarViewMode6yo === "game" ? TEXT_PURE_WHITE : TEXT_BRIGHT_YELLOW;

      btnChart.style.background = SOLID_BG_DARK;
      btnChart.style.borderColor = radarViewMode6yo === "chart" ? "#00e5ff" : "#ffb800";
      btnChart.style.color = radarViewMode6yo === "chart" ? TEXT_PURE_WHITE : TEXT_BRIGHT_YELLOW;
    }
  }

  // Draws airplane silhouette + a small high-contrast circle badge with fixed number "1", "2", "3", "4"
  function drawTopDownAirplaneWithNumber(
    ctx,
    x,
    y,
    color,
    numStr,
    scale = 1.0,
    headingRad = 0
  ) {
    ctx.save();
    ctx.translate(x, y);
    ctx.rotate(headingRad);

    ctx.fillStyle = color;
    ctx.strokeStyle = SOLID_BG_DARK;
    ctx.lineWidth = 1.6;

    ctx.beginPath();
    ctx.moveTo(14 * scale, 0);
    ctx.lineTo(5 * scale, -3.5 * scale);
    ctx.lineTo(-1 * scale, -12 * scale);
    ctx.lineTo(-5 * scale, -12 * scale);
    ctx.lineTo(-3 * scale, -3.5 * scale);
    ctx.lineTo(-10 * scale, -2.5 * scale);
    ctx.lineTo(-13 * scale, -7 * scale);
    ctx.lineTo(-15 * scale, -7 * scale);
    ctx.lineTo(-13 * scale, 0);
    ctx.lineTo(-15 * scale, 7 * scale);
    ctx.lineTo(-13 * scale, 7 * scale);
    ctx.lineTo(-10 * scale, 2.5 * scale);
    ctx.lineTo(-3 * scale, 3.5 * scale);
    ctx.lineTo(-5 * scale, 12 * scale);
    ctx.lineTo(-1 * scale, 12 * scale);
    ctx.lineTo(5 * scale, 3.5 * scale);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();
    ctx.restore();

    // Small opaque circle (#050811) with 2px border and #FFFFFF number (1, 2, 3, 4)
    ctx.save();
    ctx.beginPath();
    ctx.arc(x, y, 10.5, 0, Math.PI * 2);
    ctx.fillStyle = SOLID_BG_DARK;
    ctx.fill();
    ctx.lineWidth = 2.0;
    ctx.strokeStyle = color;
    ctx.stroke();

    ctx.fillStyle = TEXT_PURE_WHITE;
    ctx.font = "bold 11.5px 'Segoe UI', system-ui, sans-serif";
    ctx.textAlign = "center";
    ctx.fillText(numStr, x, y + 4);
    ctx.restore();
  }

  function ensureRadarToggleSetup(canvas) {
    if (!radarToggleBound && canvas) {
      radarToggleBound = true;
      canvas.style.cursor = "pointer";
      canvas.addEventListener("click", (ev) => {
        const r = canvas.getBoundingClientRect();
        const cx = ev.clientX - r.left;
        const cy = ev.clientY - r.top;
        if (cy >= 4 && cy <= 34 && cx >= r.width - 310) {
          const midX = r.width - 155;
          if (cx < midX) {
            setRadarMode6yo("game");
          } else {
            setRadarMode6yo("chart");
          }
        }
      });
    }
  }

  // 100% STATIC Left-Hand Legend Column for the 4 airplanes [1]..[4]
  function drawStaticLeftPlanesPanel(ctx, panelW, h, e, lang) {
    drawRoundRect(ctx, 8, 8, panelW - 8, h - 16, 10);
    ctx.fillStyle = SOLID_BG_DARK;
    ctx.fill();
    ctx.strokeStyle = "#00e5ff";
    ctx.lineWidth = 2.2;
    ctx.stroke();

    ctx.fillStyle = TEXT_PURE_WHITE;
    ctx.font = "bold 12px 'Segoe UI', system-ui, sans-serif";
    ctx.textAlign = "left";
    ctx.fillText(
      lang === "ro" ? "🏁 AVIOANE ÎN CURSĂ (FIX):" : "🏁 PLANES IN RACE (FIXED):",
      16,
      28
    );

    const cards = [
      {
        num: "1",
        title: lang === "ro" ? "[1] ✈️🟢 AERO-SHIELD" : "[1] ✈️🟢 AERO-SHIELD",
        err: e ? e.aero_shield_error_m || 0 : 0.3,
        color: "#00ff9d",
        desc:
          lang === "ro"
            ? "Scut activ • Stă pe mijloc ✅"
            : "Shield active • On centerline ✅",
      },
      {
        num: "2",
        title: lang === "ro" ? "[2] ✈️🔴 GPS PĂCĂLIT" : "[2] ✈️🔴 HACKED GPS",
        err: e ? e.raw_gnss_error_m || 0 : 0.0,
        color: "#ff2a5f",
        desc:
          lang === "ro"
            ? "Tras de GPS fals în iarbă"
            : "Pulled by fake GPS to grass",
      },
      {
        num: "3",
        title: lang === "ro" ? "[3] ✈️🟡 CEAȚĂ SIMPLĂ" : "[3] ✈️🟡 NAIVE FOG",
        err: e ? e.naive_flow_error_m || 0 : 0.0,
        color: "#ffb800",
        desc:
          lang === "ro"
            ? "Împins de vântul din ceață"
            : "Pushed by fog crosswind",
      },
      {
        num: "4",
        title: lang === "ro" ? "[4] ✈️🟠 FĂRĂ CAMERĂ" : "[4] ✈️🟠 NO CAMERA",
        err: e ? e.pure_ins_error_m || 0 : 0.0,
        color: "#ff9100",
        desc:
          lang === "ro"
            ? "Derivă lentă fără repere"
            : "Slow drift without landmarks",
      },
    ];

    const startY = 38;
    const availH = h - startY - 14;
    const slotH = Math.floor(availH / 4);

    cards.forEach((c, idx) => {
      const cy = startY + idx * slotH;
      const ch = slotH - 6;
      drawRoundRect(ctx, 14, cy, panelW - 20, ch, 8);
      ctx.fillStyle = SOLID_BG_DARK;
      ctx.fill();
      ctx.strokeStyle = c.color;
      ctx.lineWidth = 2.0;
      ctx.stroke();

      ctx.fillStyle = TEXT_PURE_WHITE;
      ctx.font = "bold 11.5px 'Segoe UI', system-ui, sans-serif";
      ctx.textAlign = "left";
      ctx.fillText(c.title, 22, cy + 19);

      ctx.fillStyle = TEXT_BRIGHT_YELLOW;
      ctx.font = "bold 11px 'Segoe UI', system-ui, sans-serif";
      const errLabel =
        lang === "ro"
          ? `Abatere: ${c.err.toFixed(1)}m`
          : `Error: ${c.err.toFixed(1)}m`;
      ctx.fillText(errLabel, 22, cy + 37);

      if (ch >= 54) {
        ctx.fillStyle = TEXT_PURE_WHITE;
        ctx.font = "10px 'Segoe UI', system-ui, sans-serif";
        ctx.fillText(c.desc, 22, cy + 53);
      }
    });
  }

  function renderRadarCanvas6yo() {
    const canvas = document.getElementById("radarCanvas");
    if (!canvas) return;
    ensureRadarToggleSetup(canvas);

    const rect = canvas.getBoundingClientRect();
    const targetW = rect.width > 100 ? Math.round(rect.width) : 900;
    const targetH = rect.height > 100 ? Math.round(rect.height) : 440;
    if (canvas.width !== targetW || canvas.height !== targetH) {
      canvas.width = targetW;
      canvas.height = targetH;
    }
    const ctx = canvas.getContext("2d");
    const w = canvas.width;
    const h = canvas.height;

    ctx.fillStyle = SOLID_BG_DARK;
    ctx.fillRect(0, 0, w, h);

    const lang = getLang();

    if (typeof latestFrame !== "undefined" && latestFrame && latestFrame.estimators_arena) {
      const ea = latestFrame.estimators_arena;
      errorHistory6yo.push({
        shield: ea.aero_shield_error_m || 0,
        gnss: ea.raw_gnss_error_m || 0,
        naive: ea.naive_flow_error_m || 0,
        ins: ea.pure_ins_error_m || 0,
      });
      if (errorHistory6yo.length > 220) errorHistory6yo.shift();

      // Update static horizontal scoreboard pills in DOM if present
      const pillShield = document.getElementById("scorePillShield");
      const pillGnss = document.getElementById("scorePillGnss");
      const pillNaive = document.getElementById("scorePillNaive");
      const pillIns = document.getElementById("scorePillIns");
      if (pillShield) pillShield.textContent = `${(ea.aero_shield_error_m || 0).toFixed(1)}m ✅`;
      if (pillGnss) pillGnss.textContent = `${(ea.raw_gnss_error_m || 0).toFixed(1)}m`;
      if (pillNaive) pillNaive.textContent = `${(ea.naive_flow_error_m || 0).toFixed(1)}m`;
      if (pillIns) pillIns.textContent = `${(ea.pure_ins_error_m || 0).toFixed(1)}m`;
    }

    if (radarViewMode6yo === "chart") {
      renderRadarChartMode6yo(ctx, w, h, lang);
      drawModeToggleOnCanvas(ctx, w, lang);
      return;
    }

    // =========================================================================
    // MODE 1: STATIC LEFT PANEL (228px) + RUNWAY ARENA ON THE RIGHT (ZERO MOVING TEXT)
    // =========================================================================
    const staticPanelW = w > 560 ? 228 : 185;
    const trackLeftX = staticPanelW + 8;
    const trackRightX = w - 12;
    const topArenaY = 38;
    const botArenaY = h - 10;
    const arenaH = botArenaY - topArenaY;
    const centerY = topArenaY + arenaH * 0.5;

    const e =
      typeof latestFrame !== "undefined" && latestFrame
        ? latestFrame.estimators_arena
        : null;

    // Draw 100% static left column with the 4 airplane names [1]..[4]
    drawStaticLeftPlanesPanel(ctx, staticPanelW, h, e, lang);

    const mapX = (xNed) =>
      trackLeftX + 24 + ((xNed + 950) / 1130) * Math.max(120, trackRightX - trackLeftX - 48);
    const mapY = (yNed) => centerY + (yNed / 150) * (arenaH * 0.44);

    const rwyTopY = mapY(-24);
    const rwyBotY = mapY(24);
    const rwyHeight = rwyBotY - rwyTopY;

    // 1. GREEN GRASS DANGER ZONES
    ctx.fillStyle = "#082012";
    ctx.fillRect(trackLeftX, topArenaY, trackRightX - trackLeftX, rwyTopY - topArenaY);
    ctx.fillRect(trackLeftX, rwyBotY, trackRightX - trackLeftX, botArenaY - rwyBotY);

    // Static opaque grass warning boxes
    drawRoundRect(ctx, trackLeftX + 8, topArenaY + 6, 315, 22, 6);
    ctx.fillStyle = SOLID_BG_DARK;
    ctx.fill();
    ctx.strokeStyle = "#00ff9d";
    ctx.lineWidth = 2.0;
    ctx.stroke();
    ctx.fillStyle = TEXT_BRIGHT_YELLOW;
    ctx.font = "bold 10.5px 'Segoe UI', system-ui, sans-serif";
    ctx.textAlign = "left";
    ctx.fillText(
      lang === "ro"
        ? "🌱 IARBĂ (PERICOL DE IEȘIRE DE PE PISTĂ!)"
        : "🌱 GRASS (OFF-RUNWAY DANGER ZONE!)",
      trackLeftX + 16,
      topArenaY + 21
    );

    drawRoundRect(ctx, trackLeftX + 8, botArenaY - 28, 315, 22, 6);
    ctx.fillStyle = SOLID_BG_DARK;
    ctx.fill();
    ctx.strokeStyle = "#00ff9d";
    ctx.lineWidth = 2.0;
    ctx.stroke();
    ctx.fillStyle = TEXT_BRIGHT_YELLOW;
    ctx.fillText(
      lang === "ro"
        ? "🌱 IARBĂ (PERICOL DE IEȘIRE DE PE PISTĂ!)"
        : "🌱 GRASS (OFF-RUNWAY DANGER ZONE!)",
      trackLeftX + 16,
      botArenaY - 13
    );

    // 2. SAFE RUNWAY 09 IN THE MIDDLE
    ctx.fillStyle = "#111c2d";
    ctx.fillRect(trackLeftX, rwyTopY, trackRightX - trackLeftX, rwyHeight);

    ctx.strokeStyle = "#00ff9d";
    ctx.lineWidth = 2.2;
    ctx.beginPath();
    ctx.moveTo(trackLeftX, rwyTopY);
    ctx.lineTo(trackRightX, rwyTopY);
    ctx.moveTo(trackLeftX, rwyBotY);
    ctx.lineTo(trackRightX, rwyBotY);
    ctx.stroke();

    ctx.strokeStyle = TEXT_PURE_WHITE;
    ctx.lineWidth = 2.0;
    ctx.setLineDash([12, 9]);
    ctx.beginPath();
    ctx.moveTo(trackLeftX + 12, centerY);
    ctx.lineTo(trackRightX - 12, centerY);
    ctx.stroke();
    ctx.setLineDash([]);

    // Fixed opaque Runway Title Box
    drawRoundRect(ctx, trackLeftX + 10, rwyTopY + 5, 315, 22, 6);
    ctx.fillStyle = SOLID_BG_DARK;
    ctx.fill();
    ctx.strokeStyle = "#00ff9d";
    ctx.lineWidth = 2.0;
    ctx.stroke();
    ctx.fillStyle = TEXT_PURE_WHITE;
    ctx.font = "bold 11px 'Segoe UI', system-ui, sans-serif";
    ctx.textAlign = "left";
    ctx.fillText(
      lang === "ro"
        ? "🛬 PISTA 09 CRAIOVA (SIGUR PE MIJLOC ✅)"
        : "🛬 CRAIOVA RUNWAY 09 (SAFE ON CENTERLINE ✅)",
      trackLeftX + 18,
      rwyTopY + 20
    );

    if (!e) {
      drawModeToggleOnCanvas(ctx, w, lang);
      return;
    }

    const c =
      latestFrame.cyber_and_gpu && latestFrame.cyber_and_gpu.cyber_flags
        ? latestFrame.cyber_and_gpu.cyber_flags
        : {};

    // 3. COLORED TRAILS
    const drawTrail = (pts, color, width, dashed = false) => {
      if (!pts || pts.length < 2) return;
      ctx.save();
      ctx.strokeStyle = color;
      ctx.lineWidth = width;
      if (dashed) ctx.setLineDash([4, 4]);
      ctx.beginPath();
      for (let i = 0; i < pts.length; i++) {
        const px = mapX(pts[i][0]);
        const py = mapY(pts[i][1]);
        if (i === 0) ctx.moveTo(px, py);
        else ctx.lineTo(px, py);
      }
      ctx.stroke();
      ctx.restore();
    };

    if (typeof trailHistory !== "undefined") {
      drawTrail(trailHistory.ins, "#ff9100", 2.0, true);
      drawTrail(trailHistory.gnss, "#ff2a5f", 2.2);
      drawTrail(trailHistory.naive, "#ffb800", 2.2);
      drawTrail(trailHistory.shield, "#00ff9d", 3.4);
    }

    // 4. POSITIONS OF THE 4 AIRPLANES (ONLY SILHOUETTES + NUMBERS 1..4 MOVE!)
    const laneVisualSpreadY = {
      ins: -24,
      shield: 0,
      naive: 24,
      gnss: 48,
    };

    const planes = [
      {
        id: "ins",
        num: "4",
        pos: e.pure_ins_pos_ned,
        color: "#ff9100",
        scale: 0.95,
      },
      {
        id: "naive",
        num: "3",
        pos: e.naive_flow_pos_ned,
        color: "#ffb800",
        scale: 0.98,
      },
      {
        id: "gnss",
        num: "2",
        pos: e.raw_gnss_pos_ned,
        color: "#ff2a5f",
        scale: 0.98,
      },
      {
        id: "shield",
        num: "1",
        pos: e.aero_shield_pos_ned,
        color: "#00ff9d",
        scale: 1.15,
      },
    ].map((pl) => {
      const px = Math.max(
        trackLeftX + 18,
        Math.min(trackRightX - 18, mapX(pl.pos[0]))
      );
      const basePy = mapY(pl.pos[1]) + (laneVisualSpreadY[pl.id] || 0) * 0.35;
      const py = Math.max(topArenaY + 34, Math.min(botArenaY - 34, basePy));
      return { ...pl, px, py };
    });

    // 5. HACKER DECOY BOX AT 100% FIXED COORDINATES (NO PULSATION, NO MOVING TEXT!)
    const hasGpsHack =
      (c && (c.gps_jump_spoof || c.gps_stealth_ramp_spoof)) || e.raw_gnss_error_m > 8.0;
    const gnssPlane = planes.find((p) => p.id === "gnss");
    if (hasGpsHack && gnssPlane) {
      const fixedBaitW = 232;
      const fixedBaitH = 24;
      const fixedBaitX = trackRightX - fixedBaitW - 8;
      const fixedBaitY = botArenaY - 28;

      ctx.strokeStyle = "#ff2a5f";
      ctx.lineWidth = 2.0;
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      ctx.moveTo(gnssPlane.px, gnssPlane.py);
      ctx.lineTo(fixedBaitX + fixedBaitW / 2, fixedBaitY);
      ctx.stroke();
      ctx.setLineDash([]);

      drawRoundRect(ctx, fixedBaitX, fixedBaitY, fixedBaitW, fixedBaitH, 6);
      ctx.fillStyle = SOLID_BG_DARK;
      ctx.fill();
      ctx.strokeStyle = "#ff2a5f";
      ctx.lineWidth = 2.0;
      ctx.stroke();

      ctx.fillStyle = TEXT_PURE_WHITE;
      ctx.font = "bold 10.5px 'Segoe UI', system-ui, sans-serif";
      ctx.textAlign = "center";
      ctx.fillText(
        lang === "ro" ? "👾 MOMEALA HACKERULUI (GPS FALS)" : "👾 HACKER BAIT (FAKE GPS)",
        fixedBaitX + fixedBaitW / 2,
        fixedBaitY + 16
      );
    }

    // 6. DRAW ONLY THE AIRPLANE SILHOUETTES WITH FIXED NUMBER BADGES (1, 2, 3, 4)
    for (const pl of planes) {
      const driftAngle = Math.max(-0.35, Math.min(0.35, (pl.pos[1] / 90) * 0.28));
      drawTopDownAirplaneWithNumber(
        ctx,
        pl.px,
        pl.py,
        pl.color,
        pl.num,
        pl.scale,
        pl.id === "shield" ? 0 : driftAngle
      );
    }

    // 7. TOP-RIGHT STATIC MODE TOGGLE BUTTONS
    drawModeToggleOnCanvas(ctx, w, lang);
  }

  function renderRadarChartMode6yo(ctx, w, h, lang) {
    ctx.fillStyle = SOLID_BG_DARK;
    ctx.fillRect(0, 0, w, h);

    const padL = 48;
    const padR = 18;
    const padT = 44;
    const padB = 28;
    const cw = w - padL - padR;
    const ch = h - padT - padB;

    const maxErr = 75;
    const yForErr = (err) => padT + ch - (Math.min(maxErr, Math.max(0, err)) / maxErr) * ch;

    ctx.strokeStyle = "#1e293b";
    ctx.lineWidth = 1.2;
    for (let eVal = 0; eVal <= maxErr; eVal += 25) {
      const gy = yForErr(eVal);
      ctx.beginPath();
      ctx.moveTo(padL, gy);
      ctx.lineTo(padL + cw, gy);
      ctx.stroke();
      ctx.fillStyle = TEXT_PURE_WHITE;
      ctx.font = "bold 10.5px monospace";
      ctx.textAlign = "left";
      ctx.fillText(`${eVal}m`, 8, gy + 4);
    }

    drawRoundRect(ctx, 12, 6, 470, 26, 6);
    ctx.fillStyle = SOLID_BG_DARK;
    ctx.fill();
    ctx.strokeStyle = "#00ff9d";
    ctx.lineWidth = 2.0;
    ctx.stroke();
    ctx.fillStyle = TEXT_PURE_WHITE;
    ctx.font = "bold 11px 'Segoe UI', system-ui, sans-serif";
    ctx.textAlign = "left";
    ctx.fillText(
      lang === "ro"
        ? "📈 GRAFIC EROARE (m): [1] ✈️🟢 AERO-SHIELD rămâne pe centru (< 0.8m)"
        : "📈 LIVE ERROR CHART (m): [1] ✈️🟢 AERO-SHIELD stays on centerline (< 0.8m)",
      20,
      23
    );

    if (errorHistory6yo.length < 2) return;

    const plotSeries = (key, color, width) => {
      ctx.strokeStyle = color;
      ctx.lineWidth = width;
      ctx.beginPath();
      for (let i = 0; i < errorHistory6yo.length; i++) {
        const x = padL + (i / (errorHistory6yo.length - 1)) * cw;
        const y = yForErr(errorHistory6yo[i][key]);
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.stroke();
    };

    plotSeries("ins", "#ff9100", 2.0);
    plotSeries("gnss", "#ff2a5f", 2.2);
    plotSeries("naive", "#ffb800", 2.2);
    plotSeries("shield", "#00ff9d", 3.5);
  }

  function drawModeToggleOnCanvas(ctx, w, lang) {
    const btnW = 148;
    const btnH = 24;
    const gap = 6;
    const x1 = w - btnW * 2 - gap - 8;
    const x2 = w - btnW - 8;
    const y = 6;

    drawRoundRect(ctx, x1, y, btnW, btnH, 6);
    ctx.fillStyle = SOLID_BG_DARK;
    ctx.fill();
    ctx.strokeStyle = radarViewMode6yo === "game" ? "#00ff9d" : "#ffb800";
    ctx.lineWidth = 2.0;
    ctx.stroke();
    ctx.fillStyle = radarViewMode6yo === "game" ? TEXT_PURE_WHITE : TEXT_BRIGHT_YELLOW;
    ctx.font = "bold 10.5px 'Segoe UI', system-ui, sans-serif";
    ctx.textAlign = "left";
    ctx.fillText(
      lang === "ro" ? "🎮 Vedere Joc 4 Avioane" : "🎮 4-Planes Game View",
      x1 + 10,
      y + 16
    );

    drawRoundRect(ctx, x2, y, btnW, btnH, 6);
    ctx.fillStyle = SOLID_BG_DARK;
    ctx.fill();
    ctx.strokeStyle = radarViewMode6yo === "chart" ? "#00e5ff" : "#ffb800";
    ctx.lineWidth = 2.0;
    ctx.stroke();
    ctx.fillStyle = radarViewMode6yo === "chart" ? TEXT_PURE_WHITE : TEXT_BRIGHT_YELLOW;
    ctx.font = "bold 10.5px 'Segoe UI', system-ui, sans-serif";
    ctx.textAlign = "left";
    ctx.fillText(
      lang === "ro" ? "📈 Grafic Eroare (m)" : "📈 Error Chart (m)",
      x2 + 12,
      y + 16
    );
  }

  function install6yoCanvases() {
    window.renderHudCanvas = renderHudCanvas6yo;
    window.renderRadarCanvas = renderRadarCanvas6yo;
    window.setRadarMode6yo = setRadarMode6yo;
  }

  window.VisualCanvases6yo = {
    renderHudCanvas: renderHudCanvas6yo,
    renderRadarCanvas: renderRadarCanvas6yo,
    setRadarMode: setRadarMode6yo,
    install: install6yoCanvases,
  };

  install6yoCanvases();
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", install6yoCanvases);
  }
  window.addEventListener("load", install6yoCanvases);
})();
