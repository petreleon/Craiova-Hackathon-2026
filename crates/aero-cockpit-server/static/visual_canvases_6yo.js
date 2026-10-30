// =============================================================================
// AERO-SHIELD CRAIOVA 2026 — SPACIOUS VISUAL CANVASES FOR 6-YEAR-OLDS
// =============================================================================
// Respects the core UX rule: "Interfață aerisită — câte un singur concept pe
// pagină, fără texte suprapuse sau înghesuite!"
// - Page 2 (#hudCanvas, 900x460px): 3 short, clean callout bubbles strictly in
//   the corners (Top-Left, Bottom-Left, Top-Right) so the runway center and our
//   airplane are NEVER covered by text.
// - Page 3 (#radarCanvas, 900x440px): Full 900px width dedicated to the 4
//   airplanes (✈️🟢 AERO-SHIELD, ✈️🔴 GPS PĂCĂLIT, ✈️🟡 CEAȚĂ SIMPLĂ, ✈️🟠 FĂRĂ CAMERĂ),
//   generous vertical lane spacing, Y-axis label anti-collision, and a sleek
//   horizontal scoreboard bar at the top so the entire width is open for flight.

(function () {
  "use strict";

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

  // Draws a compact, corner-anchored callout bubble with a fine pointer line
  function drawCornerCalloutBubble(ctx, opts) {
    const {
      x,
      y,
      w,
      h,
      targetX,
      targetY,
      borderColor,
      bgColor,
      title,
      subtitle,
      badge,
      titleColor,
      subtitleColor,
    } = opts;

    ctx.save();

    if (typeof targetX === "number" && typeof targetY === "number") {
      const anchorX = targetX > x + w / 2 ? x + w : x;
      const anchorY = targetY > y + h / 2 ? y + h * 0.72 : y + h * 0.28;
      ctx.strokeStyle = borderColor;
      ctx.lineWidth = 1.6;
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      ctx.moveTo(anchorX, anchorY);
      ctx.lineTo(targetX, targetY);
      ctx.stroke();
      ctx.setLineDash([]);

      ctx.fillStyle = borderColor;
      ctx.beginPath();
      ctx.arc(targetX, targetY, 3.8, 0, Math.PI * 2);
      ctx.fill();
    }

    drawRoundRect(ctx, x, y, w, h, 9);
    ctx.fillStyle = bgColor || "rgba(6, 14, 26, 0.90)";
    ctx.fill();
    ctx.lineWidth = 1.8;
    ctx.strokeStyle = borderColor || "#00e5ff";
    ctx.stroke();

    ctx.fillStyle = titleColor || borderColor || "#ffffff";
    ctx.font = "bold 12px 'Segoe UI', system-ui, sans-serif";
    ctx.fillText(title, x + 10, y + 20);

    if (subtitle) {
      ctx.fillStyle = subtitleColor || "#dbeafe";
      ctx.font = "11px 'Segoe UI', system-ui, sans-serif";
      ctx.fillText(subtitle, x + 10, y + 37);
    }

    if (badge) {
      ctx.fillStyle = "#fde047";
      ctx.font = "bold 10px 'Segoe UI', system-ui, sans-serif";
      ctx.fillText(badge, x + 10, y + 53);
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

  // Deterministic Y-axis anti-collision for the 4 airplane pills
  function resolveLabelCollisionsY(items, minY, maxY, minGapPx) {
    const sorted = items.slice().sort((a, b) => a.labelY - b.labelY);
    for (let pass = 0; pass < 4; pass++) {
      for (let i = 1; i < sorted.length; i++) {
        const prev = sorted[i - 1];
        const curr = sorted[i];
        if (curr.labelY - prev.labelY < minGapPx) {
          const mid = 0.5 * (prev.labelY + curr.labelY);
          prev.labelY = Math.max(minY, mid - 0.5 * minGapPx);
          curr.labelY = Math.min(maxY, prev.labelY + minGapPx);
        }
      }
    }
    return sorted;
  }

  // ===========================================================================
  // 1. PAGE 2: HUD CANVAS ("PARBRIZUL AVIONULUI", 900x460px, CORNER CALLOUTS ONLY)
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
    ctx.clearRect(0, 0, w, h);

    const lang = getLang();

    if (typeof latestFrame === "undefined" || !latestFrame) {
      ctx.fillStyle = "#00ff9d";
      ctx.font = "bold 14px 'Segoe UI', monospace";
      ctx.fillText(
        lang === "ro"
          ? "⏳ PORNIM CAMERA AVIONULUI (60 POZE / SECUNDĂ)..."
          : "⏳ STARTING AIRPLANE CAMERA (60 FRAMES / SECOND)...",
        24,
        40
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
    const tSec = performance.now() * 0.001;

    // -------------------------------------------------------------------------
    // STEP 1: Spacious 3D Sky, Ground, Horizon & Runway 09 Craiova
    // -------------------------------------------------------------------------
    ctx.save();
    ctx.translate(w / 2, h / 2);
    ctx.rotate(-rollRad);

    const pitchOffsetPx = pitchDeg * 6.0;

    // Sky
    const skyGrad = ctx.createLinearGradient(0, -h, 0, pitchOffsetPx);
    skyGrad.addColorStop(0, "#050d22");
    skyGrad.addColorStop(1, "#142748");
    ctx.fillStyle = skyGrad;
    ctx.fillRect(-w, -h, w * 2, h + pitchOffsetPx);

    // Soft fog clouds in upper middle
    ctx.fillStyle = "rgba(217, 70, 239, 0.08)";
    const windShiftPx = ((tSec * (m.true_fog_wind_ned_mps[1] || 5) * 8) % 240) - 120;
    for (let i = -2; i <= 2; i++) {
      ctx.beginPath();
      ctx.ellipse(
        i * 150 + windShiftPx,
        pitchOffsetPx - 64 + (i % 2) * 18,
        85,
        18,
        0,
        0,
        Math.PI * 2
      );
      ctx.fill();
    }

    // Ground
    const gndGrad = ctx.createLinearGradient(0, pitchOffsetPx, 0, h);
    gndGrad.addColorStop(0, "#09261c");
    gndGrad.addColorStop(1, "#04110c");
    ctx.fillStyle = gndGrad;
    ctx.fillRect(-w, pitchOffsetPx, w * 2, h * 2);

    // Horizon Line
    ctx.strokeStyle = "rgba(0, 255, 157, 0.55)";
    ctx.lineWidth = 1.8;
    ctx.beginPath();
    ctx.moveTo(-w, pitchOffsetPx);
    ctx.lineTo(w, pitchOffsetPx);
    ctx.stroke();

    // Perspective Runway RWY 09 Craiova completely clear in center
    const lateralShiftPx = -p.pos_ned_m[1] * 2.2 - yawDeg * 4.5;
    const rwyTopY = pitchOffsetPx + 8;
    const rwyBotY = h * 0.88;
    const topHalfW = 26;
    const botHalfW = 215;

    ctx.fillStyle = "rgba(18, 42, 58, 0.85)";
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
    ctx.strokeStyle = "#ffffff";
    ctx.lineWidth = 2.2;
    ctx.setLineDash([14, 10]);
    ctx.beginPath();
    ctx.moveTo(lateralShiftPx, rwyTopY);
    ctx.lineTo(lateralShiftPx, rwyBotY);
    ctx.stroke();
    ctx.setLineDash([]);

    // Warm ALS Approach Light Bars
    const numBars = Math.min(g.als_bars_detected || 8, 11);
    let sampleBulbLocalX = lateralShiftPx;
    let sampleBulbLocalY = rwyTopY + (rwyBotY - rwyTopY) * 0.58;

    for (let i = 1; i <= numBars; i++) {
      const frac = i / (numBars + 1);
      const by = rwyTopY + frac * (rwyBotY - rwyTopY) * 0.82;
      const barHalfW = topHalfW + frac * (botHalfW - topHalfW) * 0.48;
      const isRedWing = i % 3 === 0;
      ctx.strokeStyle = isRedWing ? "#ff2a5f" : "#00ffcc";
      ctx.lineWidth = 2.6;
      ctx.beginPath();
      ctx.moveTo(lateralShiftPx - barHalfW, by);
      ctx.lineTo(lateralShiftPx + barHalfW, by);
      ctx.stroke();

      for (let b = -2; b <= 2; b++) {
        const bx = lateralShiftPx + (b / 2) * barHalfW;
        ctx.fillStyle = b === 0 ? "#ffd600" : isRedWing ? "#ff5277" : "#00ff9d";
        ctx.beginPath();
        ctx.arc(bx, by, 2.4 + frac * 1.6, 0, Math.PI * 2);
        ctx.fill();
      }
      if (i === Math.floor(numBars * 0.65)) {
        sampleBulbLocalX = lateralShiftPx - barHalfW * 0.35;
        sampleBulbLocalY = by;
      }
    }

    ctx.restore();

    // -------------------------------------------------------------------------
    // STEP 2: Optical Flow Arrows (Purple = Fog Wind, Green = Ground)
    // -------------------------------------------------------------------------
    let fogTargetX = w * 0.28;
    let fogTargetY = h * 0.24;
    let groundTargetX = w * 0.5 + sampleBulbLocalX;
    let groundTargetY = h * 0.5 + sampleBulbLocalY;

    if (latestFrame.optical_flow_vectors && latestFrame.optical_flow_vectors.length > 0) {
      let foundFog = false;
      let foundGround = false;
      for (const vec of latestFrame.optical_flow_vectors) {
        const sx = vec.x_norm * w;
        const sy = vec.y_norm * h;
        const ex = sx + vec.dx_px * 1.2;
        const ey = sy + vec.dy_px * 1.2;
        const isFog = vec.layer === "fog";
        const color = isFog ? "#e879f9" : "#00ff9d";
        drawArrow(ctx, sx, sy, ex, ey, color, isFog ? 2.2 : 2.0, 6.0);

        if (isFog && !foundFog && sy > h * 0.2 && sy < h * 0.42 && sx > w * 0.24 && sx < w * 0.65) {
          fogTargetX = ex;
          fogTargetY = ey;
          foundFog = true;
        }
        if (!isFog && !foundGround && sy > h * 0.58 && sy < h * 0.8 && sx > w * 0.32 && sx < w * 0.65) {
          groundTargetX = ex;
          groundTargetY = ey;
          foundGround = true;
        }
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
    // STEP 4: Clean Airplane Nose Reticle in Center (NO text covering center!)
    // -------------------------------------------------------------------------
    const cx = w / 2;
    const cy = h / 2;

    ctx.strokeStyle = "#00e5ff";
    ctx.lineWidth = 2.6;
    ctx.beginPath();
    ctx.moveTo(cx - 28, cy);
    ctx.lineTo(cx - 8, cy);
    ctx.lineTo(cx - 8, cy + 7);
    ctx.moveTo(cx + 8, cy);
    ctx.lineTo(cx + 28, cy);
    ctx.lineTo(cx + 8, cy + 7);
    ctx.stroke();

    // -------------------------------------------------------------------------
    // STEP 5: THE 3 CORNER CALLOUT BUBBLES (STRICTLY IN CORNERS, SHORT & AIRY)
    // -------------------------------------------------------------------------
    const windSpeedLat = Math.abs(m.true_fog_wind_ned_mps[1] || 0);
    const groundSpeed = p.vel_body_mps[0] || 0;
    const shieldErr = e && typeof e.aero_shield_error_m === "number" ? e.aero_shield_error_m : 0.3;
    const isTilting =
      Math.abs(rollDeg) > 2.5 ||
      Math.abs(p.omega_body_dps[0] || 0) > 1.5 ||
      Math.abs(yawDeg) > 2.5;

    // 1. TOP-LEFT CORNER: `fog_cloud` (`box_norm: [0.02, 0.04, 0.29, 0.14]`)
    const b1X = w * 0.02;
    const b1Y = h * 0.04;
    const b1W = Math.max(230, w * 0.29);
    const b1H = Math.max(58, h * 0.14);
    const gustAlert =
      m.gust_lookahead_warning || windSpeedLat > 8.0
        ? lang === "ro"
          ? `⚠️ RAFALĂ VĂZUTĂ ÎNAINTE (${(m.gust_lookahead_lead_sec || 0.9).toFixed(1)}s)`
          : `⚠️ GUST SPOTTED AHEAD (${(m.gust_lookahead_lead_sec || 0.9).toFixed(1)}s)`
        : lang === "ro"
          ? `Vânt lateral: ${windSpeedLat.toFixed(1)} m/s`
          : `Crosswind: ${windSpeedLat.toFixed(1)} m/s`;

    drawCornerCalloutBubble(ctx, {
      x: b1X,
      y: b1Y,
      w: b1W,
      h: b1H,
      targetX: fogTargetX,
      targetY: fogTargetY,
      borderColor: "#e879f9",
      bgColor: "rgba(24, 7, 35, 0.88)",
      titleColor: "#f0abfc",
      subtitleColor: "#f5d0fe",
      title: lang === "ro" ? "☁️💨 VÂNT ÎN CEAȚĂ" : "☁️💨 FOG WIND",
      subtitle:
        lang === "ro"
          ? "Săgețile mov arată vântul din nor"
          : "Purple arrows show wind in fog",
      badge: gustAlert,
    });

    // 2. BOTTOM-LEFT CORNER: `ground_als` (`box_norm: [0.02, 0.80, 0.31, 0.14]`)
    const b2X = w * 0.02;
    const b2Y = h * 0.80;
    const b2W = Math.max(250, w * 0.31);
    const b2H = Math.max(58, h * 0.14);
    drawCornerCalloutBubble(ctx, {
      x: b2X,
      y: b2Y,
      w: b2W,
      h: b2H,
      targetX: groundTargetX,
      targetY: groundTargetY,
      borderColor: "#00ff9d",
      bgColor: "rgba(4, 26, 18, 0.88)",
      titleColor: "#00ff9d",
      subtitleColor: "#d1fae5",
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
    const b3W = Math.max(250, w * 0.31);
    const b3H = Math.max(58, h * 0.14);
    const b3X = w - b3W - w * 0.02;
    const b3Y = h * 0.04;
    const turnBadge = isTilting
      ? lang === "ro"
        ? `🔄 ÎNCLINARE ${rollDeg.toFixed(0)}° COMPENSATĂ`
        : `🔄 TILT ${rollDeg.toFixed(0)}° COMPENSATED`
      : lang === "ro"
        ? `✅ EROARE: ${shieldErr.toFixed(1)}m (PE AX)`
        : `✅ ERROR: ${shieldErr.toFixed(1)}m (ON AXIS)`;

    drawCornerCalloutBubble(ctx, {
      x: b3X,
      y: b3Y,
      w: b3W,
      h: b3H,
      targetX: cx,
      targetY: cy,
      borderColor: "#00e5ff",
      bgColor: "rgba(4, 20, 36, 0.88)",
      titleColor: "#00e5ff",
      subtitleColor: "#bae6fd",
      title:
        lang === "ro"
          ? "✈️ AVIONUL NOSTRU (PE CENTRU)"
          : "✈️ OUR PLANE (ON CENTERLINE)",
      subtitle:
        lang === "ro"
          ? `Alt: ${(-p.pos_ned_m[2]).toFixed(0)}m • MOR: ${m.visibility_mor_m.toFixed(0)}m`
          : `Alt: ${(-p.pos_ned_m[2]).toFixed(0)}m • MOR: ${m.visibility_mor_m.toFixed(0)}m`,
      badge: turnBadge,
    });
  }

  // ===========================================================================
  // 2. PAGE 3: RADAR CANVAS ("CURSA CELOR 4 AVIOANE", 900x440px, FULL WIDTH)
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
      btnGame.style.background =
        radarViewMode6yo === "game" ? "#00ff9d" : "rgba(10, 25, 47, 0.85)";
      btnGame.style.color = radarViewMode6yo === "game" ? "#031525" : "#a5f3fc";
      btnChart.style.background =
        radarViewMode6yo === "chart" ? "#00e5ff" : "rgba(10, 25, 47, 0.85)";
      btnChart.style.color = radarViewMode6yo === "chart" ? "#031525" : "#a5f3fc";
    }
  }

  function drawTopDownAirplane(ctx, x, y, color, scale = 1.0, headingRad = 0, halo = false) {
    ctx.save();
    ctx.translate(x, y);
    ctx.rotate(headingRad);

    if (halo) {
      ctx.fillStyle = "rgba(0, 255, 157, 0.22)";
      ctx.beginPath();
      ctx.arc(0, 0, 15 * scale, 0, Math.PI * 2);
      ctx.fill();
      ctx.strokeStyle = "#00ff9d";
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }

    ctx.fillStyle = color;
    ctx.strokeStyle = "#020813";
    ctx.lineWidth = 1.4;

    ctx.beginPath();
    ctx.moveTo(13 * scale, 0);
    ctx.lineTo(5 * scale, -3.2 * scale);
    ctx.lineTo(-1 * scale, -11.5 * scale);
    ctx.lineTo(-5 * scale, -11.5 * scale);
    ctx.lineTo(-3 * scale, -3.2 * scale);
    ctx.lineTo(-10 * scale, -2.5 * scale);
    ctx.lineTo(-13 * scale, -6.5 * scale);
    ctx.lineTo(-15 * scale, -6.5 * scale);
    ctx.lineTo(-13 * scale, 0);
    ctx.lineTo(-15 * scale, 6.5 * scale);
    ctx.lineTo(-13 * scale, 6.5 * scale);
    ctx.lineTo(-10 * scale, 2.5 * scale);
    ctx.lineTo(-3 * scale, 3.2 * scale);
    ctx.lineTo(-5 * scale, 11.5 * scale);
    ctx.lineTo(-1 * scale, 11.5 * scale);
    ctx.lineTo(5 * scale, 3.2 * scale);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();

    ctx.fillStyle = "#ffffff";
    ctx.beginPath();
    ctx.ellipse(6.5 * scale, 0, 2.4 * scale, 1.2 * scale, 0, 0, Math.PI * 2);
    ctx.fill();

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

  function drawHorizontalScoreboardBar(ctx, w, topY, e, lang) {
    const barH = 30;
    drawRoundRect(ctx, 10, topY, w - 20, barH, 8);
    ctx.fillStyle = "rgba(7, 15, 28, 0.92)";
    ctx.fill();
    ctx.strokeStyle = "#1e293b";
    ctx.lineWidth = 1.4;
    ctx.stroke();

    const items = [
      {
        label: lang === "ro" ? "✈️🟢 AERO-SHIELD" : "✈️🟢 AERO-SHIELD",
        err: e.aero_shield_error_m || 0,
        color: "#00ff9d",
        ok: (e.aero_shield_error_m || 0) < 5,
      },
      {
        label: lang === "ro" ? "✈️🟡 CEAȚĂ SIMPLĂ" : "✈️🟡 NAIVE FOG",
        err: e.naive_flow_error_m || 0,
        color: "#ffd600",
        ok: (e.naive_flow_error_m || 0) < 8,
      },
      {
        label: lang === "ro" ? "✈️🔴 GPS PĂCĂLIT" : "✈️🔴 HACKED GPS",
        err: e.raw_gnss_error_m || 0,
        color: "#ff2a5f",
        ok: (e.raw_gnss_error_m || 0) < 8,
      },
      {
        label: lang === "ro" ? "✈️🟠 FĂRĂ CAMERĂ" : "✈️🟠 NO CAMERA",
        err: e.pure_ins_error_m || 0,
        color: "#ff9100",
        ok: (e.pure_ins_error_m || 0) < 8,
      },
    ];

    const slotW = (w - 36) / 4;
    items.forEach((it, idx) => {
      const sx = 18 + idx * slotW;
      ctx.fillStyle = it.color;
      ctx.font = "bold 11px 'Segoe UI', system-ui, sans-serif";
      const icon = it.ok ? "✅" : "🌱";
      ctx.fillText(`${it.label}: ${it.err.toFixed(1)}m ${icon}`, sx, topY + 19);
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
    ctx.clearRect(0, 0, w, h);

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
    }

    if (radarViewMode6yo === "chart") {
      renderRadarChartMode6yo(ctx, w, h, lang);
      drawModeToggleOnCanvas(ctx, w, lang);
      return;
    }

    // =========================================================================
    // MODE 1: FULL-WIDTH 900px 4-PLANES RACE (NO SIDE SCOREBOARD STEALING WIDTH)
    // =========================================================================
    const topArenaY = 40;
    const botArenaY = h - 38;
    const arenaH = botArenaY - topArenaY;
    const centerY = topArenaY + arenaH * 0.5;

    const mapX = (xNed) => ((xNed + 950) / 1130) * (w - 190) + 32;
    const mapY = (yNed) => centerY + (yNed / 150) * (arenaH * 0.44);

    const rwyTopY = mapY(-24);
    const rwyBotY = mapY(24);
    const rwyHeight = rwyBotY - rwyTopY;

    // 1. GREEN GRASS DANGER ZONES ON FULL WIDTH
    const grassGradTop = ctx.createLinearGradient(0, topArenaY, 0, rwyTopY);
    grassGradTop.addColorStop(0, "#0b2e13");
    grassGradTop.addColorStop(1, "#144520");
    ctx.fillStyle = grassGradTop;
    ctx.fillRect(0, topArenaY, w, rwyTopY - topArenaY);

    const grassGradBot = ctx.createLinearGradient(0, rwyBotY, 0, botArenaY);
    grassGradBot.addColorStop(0, "#144520");
    grassGradBot.addColorStop(1, "#0b2e13");
    ctx.fillStyle = grassGradBot;
    ctx.fillRect(0, rwyBotY, w, botArenaY - rwyBotY);

    ctx.fillStyle = "rgba(134, 239, 172, 0.60)";
    ctx.font = "bold 10.5px 'Segoe UI', system-ui, sans-serif";
    ctx.fillText(
      lang === "ro"
        ? "🌱 IARBĂ (PERICOL DE IEȘIRE DE PE PISTĂ!)"
        : "🌱 GRASS (OFF-RUNWAY DANGER ZONE!)",
      16,
      topArenaY + 16
    );
    ctx.fillText(
      lang === "ro"
        ? "🌱 IARBĂ (PERICOL DE IEȘIRE DE PE PISTĂ!)"
        : "🌱 GRASS (OFF-RUNWAY DANGER ZONE!)",
      16,
      botArenaY - 8
    );

    // 2. SAFE APPROACH CORRIDOR & ASPHALT RUNWAY 09 ACROSS FULL 900px WIDTH
    ctx.fillStyle = "rgba(30, 41, 59, 0.94)";
    ctx.fillRect(0, rwyTopY, w, rwyHeight);

    ctx.strokeStyle = "rgba(0, 255, 157, 0.58)";
    ctx.lineWidth = 2.0;
    ctx.beginPath();
    ctx.moveTo(0, rwyTopY);
    ctx.lineTo(w, rwyTopY);
    ctx.moveTo(0, rwyBotY);
    ctx.lineTo(w, rwyBotY);
    ctx.stroke();

    ctx.strokeStyle = "rgba(255, 255, 255, 0.68)";
    ctx.lineWidth = 2.0;
    ctx.setLineDash([12, 9]);
    ctx.beginPath();
    ctx.moveTo(20, centerY);
    ctx.lineTo(w - 20, centerY);
    ctx.stroke();
    ctx.setLineDash([]);

    const rwyX0 = mapX(0);
    const rwyX1 = mapX(150);
    ctx.fillStyle = "#1e293b";
    ctx.strokeStyle = "#00e5ff";
    ctx.lineWidth = 2.4;
    ctx.fillRect(rwyX0, rwyTopY - 2, rwyX1 - rwyX0, rwyHeight + 4);
    ctx.strokeRect(rwyX0, rwyTopY - 2, rwyX1 - rwyX0, rwyHeight + 4);

    ctx.fillStyle = "#00ff9d";
    ctx.font = "bold 11.5px 'Segoe UI', system-ui, sans-serif";
    ctx.fillText(
      lang === "ro"
        ? "🛬 PISTA 09 CRAIOVA (SIGUR PE MIJLOC ✅)"
        : "🛬 CRAIOVA RUNWAY 09 (SAFE ON CENTERLINE ✅)",
      18,
      rwyTopY + 15
    );

    if (typeof latestFrame === "undefined" || !latestFrame) {
      drawModeToggleOnCanvas(ctx, w, lang);
      return;
    }

    const e = latestFrame.estimators_arena;
    const c = latestFrame.cyber_and_gpu ? latestFrame.cyber_and_gpu.cyber_flags : {};
    const tSec = performance.now() * 0.001;

    // 3. GENEROUS VERTICAL LANE OFFSETS + ANTI-COLLISION FOR THE 4 AIRPLANES
    const laneVisualSpreadY = {
      ins: -28,
      shield: 0,
      naive: 28,
      gnss: 54,
    };

    const rawPlanes = [
      {
        id: "ins",
        pos: e.pure_ins_pos_ned,
        err: e.pure_ins_error_m || 0,
        color: "#ff9100",
        scale: 0.92,
        halo: false,
        shortLabel:
          lang === "ro"
            ? `✈️🟠 FĂRĂ CAMERĂ (${(e.pure_ins_error_m || 0).toFixed(1)}m)`
            : `✈️🟠 NO CAMERA (${(e.pure_ins_error_m || 0).toFixed(1)}m)`,
      },
      {
        id: "shield",
        pos: e.aero_shield_pos_ned,
        err: e.aero_shield_error_m || 0,
        color: "#00ff9d",
        scale: 1.16,
        halo: true,
        shortLabel:
          lang === "ro"
            ? `✈️🟢 AERO-SHIELD (${(e.aero_shield_error_m || 0).toFixed(1)}m ✅)`
            : `✈️🟢 AERO-SHIELD (${(e.aero_shield_error_m || 0).toFixed(1)}m ✅)`,
      },
      {
        id: "naive",
        pos: e.naive_flow_pos_ned,
        err: e.naive_flow_error_m || 0,
        color: "#ffd600",
        scale: 0.95,
        halo: false,
        shortLabel:
          lang === "ro"
            ? `✈️🟡 CEAȚĂ SIMPLĂ (${(e.naive_flow_error_m || 0).toFixed(1)}m)`
            : `✈️🟡 NAIVE FOG (${(e.naive_flow_error_m || 0).toFixed(1)}m)`,
      },
      {
        id: "gnss",
        pos: e.raw_gnss_pos_ned,
        err: e.raw_gnss_error_m || 0,
        color: "#ff2a5f",
        scale: 0.95,
        halo: false,
        shortLabel:
          lang === "ro"
            ? `✈️🔴 GPS PĂCĂLIT (${(e.raw_gnss_error_m || 0).toFixed(1)}m)`
            : `✈️🔴 HACKED GPS (${(e.raw_gnss_error_m || 0).toFixed(1)}m)`,
      },
    ].map((pl) => {
      const px = mapX(pl.pos[0]);
      const basePy = mapY(pl.pos[1]) + (laneVisualSpreadY[pl.id] || 0) * 0.35;
      const py = Math.max(topArenaY + 18, Math.min(botArenaY - 18, basePy));
      return {
        ...pl,
        px,
        py,
        labelY: py,
      };
    });

    const safePlanes = resolveLabelCollisionsY(
      rawPlanes,
      topArenaY + 18,
      botArenaY - 18,
      30
    );

    // 4. COLORED TRAILS
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
      drawTrail(trailHistory.naive, "#ffd600", 2.2);
      drawTrail(trailHistory.shield, "#00ff9d", 3.4);
    }

    // 5. HACKER BAIT TARGET IF GPS SPOOF IS ACTIVE
    const hasGpsHack =
      (c && (c.gps_jump_spoof || c.gps_stealth_ramp_spoof)) || e.raw_gnss_error_m > 8.0;
    const gnssPlane = safePlanes.find((p) => p.id === "gnss");
    if (hasGpsHack && gnssPlane) {
      const baitX = Math.min(w - 90, gnssPlane.px + 42);
      const baitY = Math.min(botArenaY - 18, gnssPlane.py + 22);
      ctx.strokeStyle = "#ff2a5f";
      ctx.lineWidth = 2.0;
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      ctx.moveTo(gnssPlane.px, gnssPlane.py);
      ctx.lineTo(baitX, baitY);
      ctx.stroke();
      ctx.setLineDash([]);

      ctx.fillStyle = "rgba(255, 42, 95, 0.24)";
      ctx.beginPath();
      ctx.arc(baitX, baitY, 12 + Math.sin(tSec * 7) * 2.5, 0, Math.PI * 2);
      ctx.fill();
    }

    // 6. DRAW AIRPLANES + SHORT NON-OVERLAPPING LABELS
    for (const pl of safePlanes) {
      const driftAngle = Math.max(-0.4, Math.min(0.4, (pl.pos[1] / 90) * 0.3));
      drawTopDownAirplane(
        ctx,
        pl.px,
        pl.py,
        pl.color,
        pl.scale,
        pl.id === "shield" ? 0 : driftAngle,
        pl.halo
      );

      ctx.font = "bold 11px 'Segoe UI', system-ui, sans-serif";
      const tw = ctx.measureText(pl.shortLabel).width + 14;
      const th = 22;
      const bx = Math.max(10, Math.min(w - tw - 10, pl.px + 16));
      const by = pl.labelY - th / 2;

      // Subtle connector if anti-collision shifted the label vertically
      if (Math.abs(pl.labelY - pl.py) > 4) {
        ctx.strokeStyle = pl.color;
        ctx.lineWidth = 1.2;
        ctx.beginPath();
        ctx.moveTo(pl.px + 8, pl.py);
        ctx.lineTo(bx, pl.labelY);
        ctx.stroke();
      }

      drawRoundRect(ctx, bx, by, tw, th, 6);
      ctx.fillStyle = "rgba(5, 13, 25, 0.92)";
      ctx.fill();
      ctx.strokeStyle = pl.color;
      ctx.lineWidth = pl.id === "shield" ? 2.0 : 1.4;
      ctx.stroke();

      ctx.fillStyle = pl.color;
      ctx.fillText(pl.shortLabel, bx + 7, by + 15);
    }

    // 7. HORIZONTAL SCOREBOARD BAR AT THE BOTTOM OF CANVAS (FULL 900px RUNWAY WIDTH PRESERVED)
    drawHorizontalScoreboardBar(ctx, w, h - 34, e, lang);

    // 8. TOP-RIGHT MODE TOGGLE BUTTONS
    drawModeToggleOnCanvas(ctx, w, lang);
  }

  function renderRadarChartMode6yo(ctx, w, h, lang) {
    ctx.fillStyle = "#040d1a";
    ctx.fillRect(0, 0, w, h);

    const padL = 46;
    const padR = 18;
    const padT = 42;
    const padB = 28;
    const cw = w - padL - padR;
    const ch = h - padT - padB;

    const maxErr = 75;
    const yForErr = (err) => padT + ch - (Math.min(maxErr, Math.max(0, err)) / maxErr) * ch;

    const safeY = yForErr(6);
    ctx.fillStyle = "rgba(0, 255, 157, 0.12)";
    ctx.fillRect(padL, safeY, cw, padT + ch - safeY);

    ctx.strokeStyle = "rgba(148, 163, 184, 0.2)";
    ctx.lineWidth = 1;
    for (let eVal = 0; eVal <= maxErr; eVal += 25) {
      const gy = yForErr(eVal);
      ctx.beginPath();
      ctx.moveTo(padL, gy);
      ctx.lineTo(padL + cw, gy);
      ctx.stroke();
      ctx.fillStyle = "#94a3b8";
      ctx.font = "10px monospace";
      ctx.fillText(`${eVal}m`, 8, gy + 3);
    }

    ctx.fillStyle = "#00ff9d";
    ctx.font = "bold 11px 'Segoe UI', system-ui, sans-serif";
    ctx.fillText(
      lang === "ro"
        ? "📈 GRAFIC EROARE (m): ✈️🟢 AERO-SHIELD rămâne pe centru (< 0.8m)"
        : "📈 LIVE ERROR CHART (m): ✈️🟢 AERO-SHIELD stays on centerline (< 0.8m)",
      14,
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
    plotSeries("naive", "#ffd600", 2.2);
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
    ctx.fillStyle = radarViewMode6yo === "game" ? "#00ff9d" : "rgba(15, 23, 42, 0.9)";
    ctx.fill();
    ctx.strokeStyle = "#00ff9d";
    ctx.lineWidth = 1.5;
    ctx.stroke();
    ctx.fillStyle = radarViewMode6yo === "game" ? "#022c22" : "#a7f3d0";
    ctx.font = "bold 10.5px 'Segoe UI', system-ui, sans-serif";
    ctx.fillText(
      lang === "ro" ? "🎮 Vedere Joc 4 Avioane" : "🎮 4-Planes Game View",
      x1 + 10,
      y + 16
    );

    drawRoundRect(ctx, x2, y, btnW, btnH, 6);
    ctx.fillStyle = radarViewMode6yo === "chart" ? "#00e5ff" : "rgba(15, 23, 42, 0.9)";
    ctx.fill();
    ctx.strokeStyle = "#00e5ff";
    ctx.lineWidth = 1.5;
    ctx.stroke();
    ctx.fillStyle = radarViewMode6yo === "chart" ? "#082f49" : "#bae6fd";
    ctx.font = "bold 10.5px 'Segoe UI', system-ui, sans-serif";
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
