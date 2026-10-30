// =============================================================================
// AERO-SHIELD CRAIOVA 2026 — VISUAL CANVASES FOR 6-YEAR-OLDS & ZERO-EXPERIENCE
// =============================================================================
// Overhauls #hudCanvas ("Ce vede Camera prin Ceață") and #radarCanvas
// ("Cursa celor 4 Avioane Văzută de Sus") with ultra-clear visual callouts,
// playful airplane silhouettes, grass vs. runway safety zones, hacker bait
// targets, fog cloud animations, and an interactive Game View / Error Chart toggle.

(function () {
  "use strict";

  let radarViewMode6yo = "game"; // "game" | "chart"
  let radarToggleBound = false;
  let errorHistory6yo = []; // Rolling buffer of { t, shield, gnss, naive, ins }

  function getLang() {
    return typeof currentLang !== "undefined" && currentLang === "en" ? "en" : "ro";
  }

  // Helper: Draw rounded rectangle
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

  // Helper: Draw a high-contrast Callout Bubble with an indicator arrow pointing to (targetX, targetY)
  function drawCalloutBubble(ctx, opts) {
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
      titleColor,
      subtitleColor,
      pulse = false,
    } = opts;

    ctx.save();

    // Pointer triangle / line from box to target
    if (typeof targetX === "number" && typeof targetY === "number") {
      const boxCx = x + w / 2;
      const boxCy = y + h / 2;
      ctx.strokeStyle = borderColor;
      ctx.lineWidth = 2.4;
      ctx.setLineDash([4, 3]);
      ctx.beginPath();
      ctx.moveTo(boxCx, boxCy);
      ctx.lineTo(targetX, targetY);
      ctx.stroke();
      ctx.setLineDash([]);

      // Glowing dot at target
      ctx.fillStyle = borderColor;
      ctx.beginPath();
      ctx.arc(targetX, targetY, 4.5, 0, Math.PI * 2);
      ctx.fill();
    }

    // Box background & border
    drawRoundRect(ctx, x, y, w, h, 8);
    ctx.fillStyle = bgColor || "rgba(4, 12, 26, 0.90)";
    ctx.fill();
    ctx.lineWidth = pulse ? 2.8 : 2.0;
    ctx.strokeStyle = borderColor || "#00e5ff";
    ctx.stroke();

    // Title
    ctx.fillStyle = titleColor || borderColor || "#ffffff";
    ctx.font = "bold 12px 'Segoe UI', system-ui, sans-serif";
    ctx.fillText(title, x + 10, y + 18);

    // Subtitle
    if (subtitle) {
      ctx.fillStyle = subtitleColor || "#dbeafe";
      ctx.font = "11px 'Segoe UI', system-ui, sans-serif";
      ctx.fillText(subtitle, x + 10, y + 34);
    }

    ctx.restore();
  }

  // Helper: Draw an arrow with a clear arrowhead
  function drawArrow(ctx, x1, y1, x2, y2, color, lineWidth = 2.2, headLen = 7) {
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
  // 1. HUD CANVAS 6YO ("CE VEDE CAMERA PRIN CEAȚĂ + 3 BALOANE EXPLICATIVE")
  // ===========================================================================
  function renderHudCanvas6yo() {
    const canvas = document.getElementById("hudCanvas");
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    if (canvas.width !== rect.width || canvas.height !== rect.height) {
      canvas.width = rect.width;
      canvas.height = rect.height;
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
    // STEP 1: 3D Sky, Ground, Horizon & Perspective Runway RWY 09 Craiova
    // -------------------------------------------------------------------------
    ctx.save();
    ctx.translate(w / 2, h / 2);
    ctx.rotate(-rollRad);

    const pitchOffsetPx = pitchDeg * 6.5;

    // Sky (upper half)
    const skyGrad = ctx.createLinearGradient(0, -h, 0, pitchOffsetPx);
    skyGrad.addColorStop(0, "#050d24");
    skyGrad.addColorStop(1, "#182c52");
    ctx.fillStyle = skyGrad;
    ctx.fillRect(-w, -h, w * 2, h + pitchOffsetPx);

    // Subtle animated fog wisps in the sky/upper zone
    ctx.fillStyle = "rgba(217, 70, 239, 0.09)";
    const windShiftPx = ((tSec * (m.true_fog_wind_ned_mps[1] || 5) * 8) % 240) - 120;
    for (let i = -2; i <= 2; i++) {
      ctx.beginPath();
      ctx.ellipse(
        i * 140 + windShiftPx,
        pitchOffsetPx - 55 + (i % 2) * 18,
        85,
        20,
        0,
        0,
        Math.PI * 2
      );
      ctx.fill();
    }

    // Ground (lower half — lush dark green terrain)
    const gndGrad = ctx.createLinearGradient(0, pitchOffsetPx, 0, h);
    gndGrad.addColorStop(0, "#0a281e");
    gndGrad.addColorStop(1, "#04120d");
    ctx.fillStyle = gndGrad;
    ctx.fillRect(-w, pitchOffsetPx, w * 2, h * 2);

    // Horizon Line
    ctx.strokeStyle = "#00ff9d";
    ctx.lineWidth = 2.2;
    ctx.beginPath();
    ctx.moveTo(-w, pitchOffsetPx);
    ctx.lineTo(w, pitchOffsetPx);
    ctx.stroke();

    // Perspective Runway RWY 09 Craiova + ALS Approach Lights
    const lateralShiftPx = -p.pos_ned_m[1] * 2.2 - yawDeg * 5.0;
    const rwyTopY = pitchOffsetPx + 6;
    const rwyBotY = h * 0.88;
    const topHalfW = 24;
    const botHalfW = 205;

    // Runway asphalt polygon
    ctx.fillStyle = "rgba(18, 42, 58, 0.85)";
    ctx.strokeStyle = "#00e5ff";
    ctx.lineWidth = 2.5;
    ctx.beginPath();
    ctx.moveTo(lateralShiftPx - topHalfW, rwyTopY);
    ctx.lineTo(lateralShiftPx + topHalfW, rwyTopY);
    ctx.lineTo(lateralShiftPx + botHalfW, rwyBotY);
    ctx.lineTo(lateralShiftPx - botHalfW, rwyBotY);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();

    // Runway Centerline dashed
    ctx.strokeStyle = "#ffffff";
    ctx.lineWidth = 2.2;
    ctx.setLineDash([14, 10]);
    ctx.beginPath();
    ctx.moveTo(lateralShiftPx, rwyTopY);
    ctx.lineTo(lateralShiftPx, rwyBotY);
    ctx.stroke();
    ctx.setLineDash([]);

    // ALS SWIR Approach Light Bars (bright glowing bulbs!)
    const numBars = Math.min(g.als_bars_detected || 8, 12);
    let sampleBulbLocalX = lateralShiftPx;
    let sampleBulbLocalY = rwyTopY + (rwyBotY - rwyTopY) * 0.55;

    for (let i = 1; i <= numBars; i++) {
      const frac = i / (numBars + 1);
      const by = rwyTopY + frac * (rwyBotY - rwyTopY) * 0.82;
      const barHalfW = topHalfW + frac * (botHalfW - topHalfW) * 0.48;
      const isRedWing = i % 3 === 0;
      ctx.strokeStyle = isRedWing ? "#ff2a5f" : "#00ffcc";
      ctx.lineWidth = 3.0;
      ctx.beginPath();
      ctx.moveTo(lateralShiftPx - barHalfW, by);
      ctx.lineTo(lateralShiftPx + barHalfW, by);
      ctx.stroke();

      // Draw bright individual light bulbs along the bar
      for (let b = -2; b <= 2; b++) {
        const bx = lateralShiftPx + (b / 2) * barHalfW;
        ctx.fillStyle = b === 0 ? "#ffd600" : isRedWing ? "#ff5277" : "#00ff9d";
        ctx.beginPath();
        ctx.arc(bx, by, 2.5 + frac * 1.8, 0, Math.PI * 2);
        ctx.fill();
      }
      if (i === Math.floor(numBars * 0.65)) {
        sampleBulbLocalX = lateralShiftPx + barHalfW * 0.6;
        sampleBulbLocalY = by;
      }
    }

    // Label on the runway itself
    ctx.fillStyle = "rgba(0, 229, 255, 0.9)";
    ctx.font = "bold 11px monospace";
    ctx.fillText(
      lang === "ro" ? "🛬 PISTA 09 CRAIOVA" : "🛬 CRAIOVA RWY 09",
      lateralShiftPx - 52,
      rwyTopY + 22
    );

    ctx.restore();

    // -------------------------------------------------------------------------
    // STEP 2: Optical Flow Arrows (Purple = Fog Wind above, Green = Ground below)
    // -------------------------------------------------------------------------
    let fogTargetX = w * 0.35;
    let fogTargetY = h * 0.28;
    let groundTargetX = w * 0.5 + sampleBulbLocalX;
    let groundTargetY = h * 0.5 + sampleBulbLocalY;

    if (latestFrame.optical_flow_vectors && latestFrame.optical_flow_vectors.length > 0) {
      let foundFog = false;
      let foundGround = false;
      for (const vec of latestFrame.optical_flow_vectors) {
        const sx = vec.x_norm * w;
        const sy = vec.y_norm * h;
        const ex = sx + vec.dx_px * 1.25;
        const ey = sy + vec.dy_px * 1.25;
        const isFog = vec.layer === "fog";
        const color = isFog ? "#e879f9" : "#00ff9d";
        drawArrow(ctx, sx, sy, ex, ey, color, isFog ? 2.5 : 2.2, 6.5);

        if (isFog && !foundFog && sy > 55 && sy < h * 0.42 && sx > w * 0.25 && sx < w * 0.75) {
          fogTargetX = ex;
          fogTargetY = ey;
          foundFog = true;
        }
        if (!isFog && !foundGround && sy > h * 0.58 && sy < h * 0.82 && sx > w * 0.3 && sx < w * 0.7) {
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
    // STEP 4: Fixed Airplane Target Crosshair & Crab Angle Marker in Center
    // -------------------------------------------------------------------------
    const cx = w / 2;
    const cy = h / 2;

    // Airplane nose reticle
    ctx.strokeStyle = "#00e5ff";
    ctx.lineWidth = 2.8;
    ctx.beginPath();
    ctx.moveTo(cx - 30, cy);
    ctx.lineTo(cx - 8, cy);
    ctx.lineTo(cx - 8, cy + 7);
    ctx.moveTo(cx + 8, cy);
    ctx.lineTo(cx + 30, cy);
    ctx.lineTo(cx + 8, cy + 7);
    ctx.stroke();

    // Crab angle diamond
    const crabPx = (p.crab_angle_cmd_deg || 0) * 4.5;
    ctx.strokeStyle = "#ffb800";
    ctx.lineWidth = 2.2;
    ctx.beginPath();
    ctx.moveTo(cx + crabPx, cy - 9);
    ctx.lineTo(cx + crabPx + 9, cy);
    ctx.lineTo(cx + crabPx, cy + 9);
    ctx.lineTo(cx + crabPx - 9, cy);
    ctx.closePath();
    ctx.stroke();

    // -------------------------------------------------------------------------
    // STEP 5: THE 3 BIG KID-FRIENDLY EXPLANATORY CALLOUT BUBBLES
    // -------------------------------------------------------------------------
    const windSpeedLat = Math.abs(m.true_fog_wind_ned_mps[1] || 0);
    const groundSpeed = p.vel_body_mps[0] || 0;
    const shieldErr = e && typeof e.aero_shield_error_m === "number" ? e.aero_shield_error_m : 0.3;

    // BUBBLE 1 (TOP — PURPLE: FOG CLOUD PUSHED BY WIND)
    const b1W = Math.min(w - 24, 410);
    const b1H = 42;
    const b1X = 12;
    const b1Y = 10;
    drawCalloutBubble(ctx, {
      x: b1X,
      y: b1Y,
      w: b1W,
      h: b1H,
      targetX: fogTargetX,
      targetY: fogTargetY,
      borderColor: "#e879f9",
      bgColor: "rgba(28, 7, 38, 0.88)",
      titleColor: "#f0abfc",
      subtitleColor: "#f5d0fe",
      title:
        lang === "ro"
          ? `☁️💨 NORUL DE CEAȚĂ (Împins de vânt: ${windSpeedLat.toFixed(1)} m/s)`
          : `☁️💨 FOG CLOUD (Pushed by wind: ${windSpeedLat.toFixed(1)} m/s)`,
      subtitle:
        lang === "ro"
          ? "Săgețile MOV arată încotro suflă vântul prin ceață"
          : "PURPLE arrows show where the wind pushes the fog",
    });

    // Animated Early Gust Warning Banner (if gust lookahead active or strong crosswind > 8 m/s)
    if (m.gust_lookahead_warning || windSpeedLat > 8.0) {
      const pulseOn = Math.sin(tSec * 8) > -0.2;
      const gwW = Math.min(w - 24, 460);
      const gwH = 28;
      const gwX = 12;
      const gwY = b1Y + b1H + 5;
      ctx.save();
      drawRoundRect(ctx, gwX, gwY, gwW, gwH, 6);
      ctx.fillStyle = pulseOn ? "rgba(220, 38, 38, 0.94)" : "rgba(153, 27, 27, 0.90)";
      ctx.fill();
      ctx.strokeStyle = "#fde047";
      ctx.lineWidth = 2.2;
      ctx.stroke();
      ctx.fillStyle = "#fef08a";
      ctx.font = "bold 11.5px 'Segoe UI', system-ui, sans-serif";
      const leadSec = (m.gust_lookahead_lead_sec || 1.1).toFixed(1);
      ctx.fillText(
        lang === "ro"
          ? `⚠️ VINE O RAFALĂ ÎN ${leadSec} SECUNDE! (Am văzut-o în ceață înainte să lovească aripile!)`
          : `⚠️ WIND GUST IN ${leadSec} SECONDS! (Spotted in the fog before hitting our wings!)`,
        gwX + 10,
        gwY + 18
      );
      ctx.restore();
    }

    // BUBBLE 3 (CENTER-RIGHT — CYAN: OUR PLANE FOLLOWING THE RUNWAY LINE)
    const b3W = Math.min(w - 24, 375);
    const b3H = 38;
    const b3X = Math.max(12, w - b3W - 12);
    const b3Y = Math.max(58, cy - 58);
    drawCalloutBubble(ctx, {
      x: b3X,
      y: b3Y,
      w: b3W,
      h: b3H,
      targetX: cx,
      targetY: cy,
      borderColor: "#00e5ff",
      bgColor: "rgba(4, 22, 38, 0.88)",
      titleColor: "#00e5ff",
      subtitleColor: "#bae6fd",
      title:
        lang === "ro"
          ? `✈️ AVIONUL NOSTRU (Se ține după pistă — Eroare: ${shieldErr.toFixed(1)}m)`
          : `✈️ OUR PLANE (Follows the runway line — Error: ${shieldErr.toFixed(1)}m)`,
      subtitle:
        lang === "ro"
          ? `Altitudine: ${(-p.pos_ned_m[2]).toFixed(0)}m | Vizibilitate în ceață: ${m.visibility_mor_m.toFixed(0)}m`
          : `Altitude: ${(-p.pos_ned_m[2]).toFixed(0)}m | Fog visibility: ${m.visibility_mor_m.toFixed(0)}m`,
    });

    // Tilt / Derotation Visual Badge when airplane banks/turns
    const isTilting =
      Math.abs(rollDeg) > 2.5 ||
      Math.abs(p.omega_body_dps[0] || 0) > 1.5 ||
      Math.abs(yawDeg) > 2.5;

    if (isTilting) {
      const tiltW = Math.min(w - 24, 430);
      const tiltH = 26;
      const tiltX = Math.max(12, w - tiltW - 12);
      const tiltY = b3Y + b3H + 5;
      ctx.save();
      drawRoundRect(ctx, tiltX, tiltY, tiltW, tiltH, 6);
      ctx.fillStyle = "rgba(180, 83, 9, 0.93)";
      ctx.fill();
      ctx.strokeStyle = "#fde047";
      ctx.lineWidth = 2.0;
      ctx.stroke();
      ctx.fillStyle = "#fffbeb";
      ctx.font = "bold 11px 'Segoe UI', system-ui, sans-serif";
      ctx.fillText(
        lang === "ro"
          ? `🔄 AVIONUL SE ÎNCLINĂ (${rollDeg.toFixed(0)}°) — Scădem automat înclinarea din poză!`
          : `🔄 PLANE IS TILTING (${rollDeg.toFixed(0)}°) — We automatically subtract tilt from the picture!`,
        tiltX + 10,
        tiltY + 17
      );
      ctx.restore();
    }

    // BUBBLE 2 (BOTTOM — BRIGHT GREEN: GROUND & RUNWAY LIGHTS)
    const b2W = Math.min(w - 24, 445);
    const b2H = 42;
    const b2X = 12;
    const b2Y = h - b2H - 10;
    drawCalloutBubble(ctx, {
      x: b2X,
      y: b2Y,
      w: b2W,
      h: b2H,
      targetX: groundTargetX,
      targetY: groundTargetY,
      borderColor: "#00ff9d",
      bgColor: "rgba(4, 28, 18, 0.90)",
      titleColor: "#00ff9d",
      subtitleColor: "#d1fae5",
      title:
        lang === "ro"
          ? `💡 PĂMÂNTUL & BECURILE PISTEI (Stau pe loc! Viteza: ${groundSpeed.toFixed(1)} m/s)`
          : `💡 GROUND & RUNWAY LIGHTS (Stay still! Plane speed: ${groundSpeed.toFixed(1)} m/s)`,
      subtitle:
        lang === "ro"
          ? "Săgețile VERZI măsoară cât de repede zboară avionul peste pământ"
          : "GREEN arrows measure how fast our plane flies over the ground",
    });
  }

  // ===========================================================================
  // 2. RADAR CANVAS 6YO ("CURSA CELOR 4 AVIOANE VĂZUTĂ DE SUS — CA ÎNTR-UN JOC")
  // ===========================================================================
  function setRadarMode6yo(mode) {
    radarViewMode6yo = mode === "chart" ? "chart" : "game";
    const btnGame = document.getElementById("btnRadarModeGame6yo");
    const btnChart = document.getElementById("btnRadarModeChart6yo");
    if (btnGame && btnChart) {
      btnGame.style.background =
        radarViewMode6yo === "game" ? "#00ff9d" : "rgba(10, 25, 47, 0.85)";
      btnGame.style.color = radarViewMode6yo === "game" ? "#031525" : "#a5f3fc";
      btnChart.style.background =
        radarViewMode6yo === "chart" ? "#00e5ff" : "rgba(10, 25, 47, 0.85)";
      btnChart.style.color = radarViewMode6yo === "chart" ? "#031525" : "#a5f3fc";
    }
  }

  // Helper: Draw a cute top-down airplane icon facing right (+X towards runway)
  function drawTopDownAirplane(ctx, x, y, color, scale = 1.0, headingRad = 0, halo = false) {
    ctx.save();
    ctx.translate(x, y);
    ctx.rotate(headingRad);

    if (halo) {
      ctx.fillStyle = "rgba(0, 255, 157, 0.24)";
      ctx.beginPath();
      ctx.arc(0, 0, 16 * scale, 0, Math.PI * 2);
      ctx.fill();
      ctx.strokeStyle = "#00ff9d";
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }

    ctx.fillStyle = color;
    ctx.strokeStyle = "#020813";
    ctx.lineWidth = 1.5;

    // Airplane fuselage + swept wings + tail
    ctx.beginPath();
    ctx.moveTo(13 * scale, 0); // Nose tip
    ctx.lineTo(5 * scale, -3.2 * scale);
    ctx.lineTo(-1 * scale, -12 * scale); // Left wingtip
    ctx.lineTo(-5 * scale, -12 * scale);
    ctx.lineTo(-3 * scale, -3.2 * scale);
    ctx.lineTo(-10 * scale, -2.5 * scale);
    ctx.lineTo(-13 * scale, -7 * scale); // Left tail
    ctx.lineTo(-15 * scale, -7 * scale);
    ctx.lineTo(-13 * scale, 0);
    ctx.lineTo(-15 * scale, 7 * scale); // Right tail
    ctx.lineTo(-13 * scale, 7 * scale);
    ctx.lineTo(-10 * scale, 2.5 * scale);
    ctx.lineTo(-3 * scale, 3.2 * scale);
    ctx.lineTo(-5 * scale, 12 * scale);
    ctx.lineTo(-1 * scale, 12 * scale); // Right wingtip
    ctx.lineTo(5 * scale, 3.2 * scale);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();

    // Cockpit windshield highlight
    ctx.fillStyle = "#ffffff";
    ctx.beginPath();
    ctx.ellipse(6.5 * scale, 0, 2.5 * scale, 1.3 * scale, 0, 0, Math.PI * 2);
    ctx.fill();

    ctx.restore();
  }

  // Bind click on #radarCanvas for the top-right toggle buttons & inject DOM buttons
  function ensureRadarToggleSetup(canvas) {
    if (!radarToggleBound && canvas) {
      radarToggleBound = true;
      canvas.style.cursor = "pointer";
      canvas.addEventListener("click", (ev) => {
        const r = canvas.getBoundingClientRect();
        const cx = ev.clientX - r.left;
        const cy = ev.clientY - r.top;
        // Check top-right button region
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

  function renderRadarCanvas6yo() {
    const canvas = document.getElementById("radarCanvas");
    if (!canvas) return;
    ensureRadarToggleSetup(canvas);

    const rect = canvas.getBoundingClientRect();
    if (canvas.width !== rect.width || canvas.height !== rect.height) {
      canvas.width = rect.width;
      canvas.height = rect.height;
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
    // MODE 1: "VEDERE JOC 4 AVIOANE VĂZUTĂ DE SUS" (DEFAULT)
    // =========================================================================
    // Reserve right 185px for Live Scoreboard if canvas is wide enough
    const scoreboardW = w > 520 ? 188 : 0;
    const mapW = w - scoreboardW;

    const mapX = (xNed) => ((xNed + 950) / 1130) * (mapW - 44) + 22;
    const mapY = (yNed) => h / 2 + (yNed / 145) * (h * 0.43);

    const rwyTopY = mapY(-24);
    const rwyBotY = mapY(24);
    const rwyHeight = rwyBotY - rwyTopY;

    // 1. GREEN GRASS DANGER ZONES ON TOP & BOTTOM
    const grassGradTop = ctx.createLinearGradient(0, 0, 0, rwyTopY);
    grassGradTop.addColorStop(0, "#0b2e13");
    grassGradTop.addColorStop(1, "#144520");
    ctx.fillStyle = grassGradTop;
    ctx.fillRect(0, 0, mapW, rwyTopY);

    const grassGradBot = ctx.createLinearGradient(0, rwyBotY, 0, h);
    grassGradBot.addColorStop(0, "#144520");
    grassGradBot.addColorStop(1, "#0b2e13");
    ctx.fillStyle = grassGradBot;
    ctx.fillRect(0, rwyBotY, mapW, h - rwyBotY);

    // Subtle lawn mowing stripes in grass
    ctx.fillStyle = "rgba(255, 255, 255, 0.03)";
    for (let sx = 0; sx < mapW; sx += 48) {
      ctx.fillRect(sx, 0, 24, rwyTopY);
      ctx.fillRect(sx, rwyBotY, 24, h - rwyBotY);
    }

    // Grass danger watermarks
    ctx.fillStyle = "rgba(134, 239, 172, 0.55)";
    ctx.font = "bold 10.5px 'Segoe UI', system-ui, sans-serif";
    ctx.fillText(
      lang === "ro"
        ? "🌱 IARBĂ (PERICOL DE IEȘIRE DE PE PISTĂ!)"
        : "🌱 GRASS (OFF-RUNWAY DANGER ZONE!)",
      14,
      20
    );
    ctx.fillText(
      lang === "ro"
        ? "🌱 IARBĂ (PERICOL DE IEȘIRE DE PE PISTĂ!)"
        : "🌱 GRASS (OFF-RUNWAY DANGER ZONE!)",
      14,
      h - 10
    );

    // 2. SAFE APPROACH CORRIDOR & ASPHALT RUNWAY 09 IN THE MIDDLE
    ctx.fillStyle = "rgba(30, 41, 59, 0.92)";
    ctx.fillRect(0, rwyTopY, mapW, rwyHeight);

    // Safe corridor borders (white/green edge lines)
    ctx.strokeStyle = "rgba(0, 255, 157, 0.55)";
    ctx.lineWidth = 2.0;
    ctx.beginPath();
    ctx.moveTo(0, rwyTopY);
    ctx.lineTo(mapW, rwyTopY);
    ctx.moveTo(0, rwyBotY);
    ctx.lineTo(mapW, rwyBotY);
    ctx.stroke();

    // Dashed white approach centerline
    ctx.strokeStyle = "rgba(255, 255, 255, 0.65)";
    ctx.lineWidth = 2.0;
    ctx.setLineDash([10, 8]);
    ctx.beginPath();
    ctx.moveTo(mapX(-950), mapY(0));
    ctx.lineTo(mapX(150), mapY(0));
    ctx.stroke();
    ctx.setLineDash([]);

    // RWY 09 Paved Threshold & Zebra Markings (X = 0m to +150m)
    const rwyX0 = mapX(0);
    const rwyX1 = mapX(150);
    ctx.fillStyle = "#1e293b";
    ctx.strokeStyle = "#00e5ff";
    ctx.lineWidth = 2.5;
    ctx.fillRect(rwyX0, rwyTopY - 2, rwyX1 - rwyX0, rwyHeight + 4);
    ctx.strokeRect(rwyX0, rwyTopY - 2, rwyX1 - rwyX0, rwyHeight + 4);

    // Zebra piano keys at threshold
    ctx.fillStyle = "#f8fafc";
    for (let z = -16; z <= 16; z += 8) {
      ctx.fillRect(rwyX0 + 4, mapY(z) - 2, 14, 4);
    }

    // Label on the safe corridor
    ctx.fillStyle = "#00ff9d";
    ctx.font = "bold 11.5px 'Segoe UI', system-ui, sans-serif";
    ctx.fillText(
      lang === "ro"
        ? "🛬 PISTA 09 CRAIOVA (SIGUR PE MIJLOC ✅)"
        : "🛬 CRAIOVA RUNWAY 09 (SAFE ON CENTERLINE ✅)",
      Math.max(14, rwyX0 - 245),
      mapY(0) - rwyHeight / 2 + 13
    );

    if (typeof latestFrame === "undefined" || !latestFrame) {
      drawModeToggleOnCanvas(ctx, w, lang);
      return;
    }

    const e = latestFrame.estimators_arena;
    const m = latestFrame.fog_and_meteo;
    const c = latestFrame.cyber_and_gpu ? latestFrame.cyber_and_gpu.cyber_flags : {};
    const tSec = performance.now() * 0.001;

    // 3. SPECIAL VISUAL EFFECTS FOR HACKER SPOOFING & FOG WIND PUSH
    const hasGpsHack =
      (c && (c.gps_jump_spoof || c.gps_stealth_ramp_spoof)) || e.raw_gnss_error_m > 8.0;
    const windSpeedLat = Math.abs(m.true_fog_wind_ned_mps[1] || 0);

    // If GPS Hack is active, draw a pulsing "HACKER BAIT TARGET" in the grass!
    if (hasGpsHack) {
      const gnssPx = mapX(e.raw_gnss_pos_ned[0]);
      const gnssPy = mapY(e.raw_gnss_pos_ned[1]);
      const baitX = Math.min(mapW - 40, gnssPx + 24);
      const baitY = gnssPy > h / 2 ? Math.min(h - 22, gnssPy + 18) : Math.max(24, gnssPy - 18);

      // Red attraction beam
      ctx.strokeStyle = "#ff2a5f";
      ctx.lineWidth = 2.2;
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      ctx.moveTo(gnssPx, gnssPy);
      ctx.lineTo(baitX, baitY);
      ctx.stroke();
      ctx.setLineDash([]);

      // Target bullseye
      ctx.fillStyle = "rgba(255, 42, 95, 0.25)";
      ctx.beginPath();
      ctx.arc(baitX, baitY, 14 + Math.sin(tSec * 7) * 3, 0, Math.PI * 2);
      ctx.fill();
      ctx.strokeStyle = "#ff2a5f";
      ctx.lineWidth = 2;
      ctx.stroke();

      ctx.fillStyle = "#fda4af";
      ctx.font = "bold 10.5px 'Segoe UI', system-ui, sans-serif";
      const baitText =
        lang === "ro" ? "👾 MOMEALA HACKERULUI (GPS FALS)" : "👾 HACKER BAIT (FAKE GPS)";
      const btW = ctx.measureText(baitText).width;
      ctx.fillText(baitText, Math.max(8, Math.min(mapW - btW - 8, baitX - btW / 2)), baitY - 16);
    }

    // If Fog Wind is blowing, draw purple wind arrows pushing the yellow plane
    if (windSpeedLat > 2.5) {
      const naivePx = mapX(e.naive_flow_pos_ned[0]);
      const naivePy = mapY(e.naive_flow_pos_ned[1]);
      const pushDir = (m.true_fog_wind_ned_mps[1] || 1) >= 0 ? 1 : -1;
      drawArrow(
        ctx,
        naivePx,
        naivePy - pushDir * 24,
        naivePx,
        naivePy - pushDir * 7,
        "#e879f9",
        2.4,
        6
      );
      ctx.fillStyle = "#f0abfc";
      ctx.font = "bold 10px 'Segoe UI', system-ui, sans-serif";
      ctx.fillText(
        lang === "ro" ? "☁️💨 Vânt din ceață" : "☁️💨 Fog wind push",
        Math.max(8, naivePx - 45),
        naivePy - pushDir * 28
      );
    }

    // 4. DRAW COLORED TRAILS FOR ALL 4 PLANES
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
      drawTrail(trailHistory.ins, "#ff9100", 2.2, true);
      drawTrail(trailHistory.gnss, "#ff2a5f", 2.4);
      drawTrail(trailHistory.naive, "#ffd600", 2.5);
      drawTrail(trailHistory.shield, "#00ff9d", 3.8);
    }

    // 5. DRAW THE 4 AIRPLANES + NON-OVERLAPPING KID-FRIENDLY LABELS
    const planes = [
      {
        id: "ins",
        pos: e.pure_ins_pos_ned,
        err: e.pure_ins_error_m,
        color: "#ff9100",
        scale: 0.9,
        halo: false,
        label:
          lang === "ro"
            ? `✈️🟠 FĂRĂ CAMERĂ (${e.pure_ins_error_m.toFixed(1)}m)`
            : `✈️🟠 BLINDFOLDED (${e.pure_ins_error_m.toFixed(1)}m)`,
        labelOffsetY: -22,
      },
      {
        id: "naive",
        pos: e.naive_flow_pos_ned,
        err: e.naive_flow_error_m,
        color: "#ffd600",
        scale: 0.95,
        halo: false,
        label:
          lang === "ro"
            ? `✈️🟡 PĂCĂLIT DE CEAȚĂ (${e.naive_flow_error_m.toFixed(1)}m)`
            : `✈️🟡 FOOLED BY FOG (${e.naive_flow_error_m.toFixed(1)}m)`,
        labelOffsetY: 24,
      },
      {
        id: "gnss",
        pos: e.raw_gnss_pos_ned,
        err: e.raw_gnss_error_m,
        color: "#ff2a5f",
        scale: 0.95,
        halo: false,
        label:
          lang === "ro"
            ? `✈️🔴 GPS PĂCĂLIT (${e.raw_gnss_error_m.toFixed(1)}m)`
            : `✈️🔴 HACKED GPS (${e.raw_gnss_error_m.toFixed(1)}m)`,
        labelOffsetY: -24,
      },
      {
        id: "shield",
        pos: e.aero_shield_pos_ned,
        err: e.aero_shield_error_m,
        color: "#00ff9d",
        scale: 1.18,
        halo: true,
        label:
          lang === "ro"
            ? `✈️🟢 AVIONUL NOSTRU (${e.aero_shield_error_m.toFixed(1)}m ✅)`
            : `✈️🟢 OUR PLANE (${e.aero_shield_error_m.toFixed(1)}m ✅)`,
        labelOffsetY: 22,
      },
    ];

    for (const pl of planes) {
      const px = mapX(pl.pos[0]);
      const py = mapY(pl.pos[1]);
      // Slight heading tilt if drifting off centerline
      const driftAngle = Math.max(-0.45, Math.min(0.45, (pl.pos[1] / 90) * 0.35));
      drawTopDownAirplane(
        ctx,
        px,
        py,
        pl.color,
        pl.scale,
        pl.id === "shield" ? 0 : driftAngle,
        pl.halo
      );

      // Pill badge near the plane, clamped inside map bounds
      ctx.font = "bold 10.5px 'Segoe UI', system-ui, sans-serif";
      const tw = ctx.measureText(pl.label).width + 12;
      const th = 18;
      const bx = Math.max(6, Math.min(mapW - tw - 6, px - tw / 2));
      const by = Math.max(26, Math.min(h - th - 6, py + pl.labelOffsetY - th / 2));

      drawRoundRect(ctx, bx, by, tw, th, 5);
      ctx.fillStyle = "rgba(4, 12, 24, 0.88)";
      ctx.fill();
      ctx.strokeStyle = pl.color;
      ctx.lineWidth = 1.4;
      ctx.stroke();

      ctx.fillStyle = pl.color;
      ctx.fillText(pl.label, bx + 6, by + 13);
    }

    // 6. RIGHT-HAND LIVE SCOREBOARD PANEL (IF CANVAS IS WIDE ENOUGH)
    if (scoreboardW > 0) {
      const sx = mapW + 4;
      const sy = 36;
      const sw = scoreboardW - 8;
      const sh = h - sy - 6;

      drawRoundRect(ctx, sx, sy, sw, sh, 8);
      ctx.fillStyle = "rgba(4, 13, 26, 0.94)";
      ctx.fill();
      ctx.strokeStyle = "#164e63";
      ctx.lineWidth = 1.5;
      ctx.stroke();

      ctx.fillStyle = "#e2e8f0";
      ctx.font = "bold 10.5px 'Segoe UI', system-ui, sans-serif";
      ctx.fillText(
        lang === "ro" ? "🏆 CINE STĂ PE PISTĂ?" : "🏆 WHO STAYS ON RUNWAY?",
        sx + 8,
        sy + 16
      );

      const rows = [
        {
          name: lang === "ro" ? "🟢 Avionul Nostru" : "🟢 Our Plane",
          err: e.aero_shield_error_m,
          color: "#00ff9d",
          ok: e.aero_shield_error_m < 5,
        },
        {
          name: lang === "ro" ? "🟡 Păcălit de Ceață" : "🟡 Fooled by Fog",
          err: e.naive_flow_error_m,
          color: "#ffd600",
          ok: e.naive_flow_error_m < 8,
        },
        {
          name: lang === "ro" ? "🔴 GPS Păcălit" : "🔴 Hacked GPS",
          err: e.raw_gnss_error_m,
          color: "#ff2a5f",
          ok: e.raw_gnss_error_m < 8,
        },
        {
          name: lang === "ro" ? "🟠 Fără Cameră" : "🟠 Blindfolded",
          err: e.pure_ins_error_m,
          color: "#ff9100",
          ok: e.pure_ins_error_m < 8,
        },
      ];

      const rowH = Math.max(24, Math.floor((sh - 24) / 4));
      rows.forEach((r, idx) => {
        const ry = sy + 24 + idx * rowH;
        ctx.fillStyle = r.color;
        ctx.font = "bold 10px 'Segoe UI', system-ui, sans-serif";
        const statusIcon = r.ok ? "✅" : "🌱❌";
        ctx.fillText(`${r.name}: ${r.err.toFixed(1)}m ${statusIcon}`, sx + 8, ry + 10);

        // Visual progress bar of error (capped at 60m)
        const barMaxW = sw - 16;
        const fillW = Math.max(4, Math.min(barMaxW, (r.err / 60.0) * barMaxW));
        ctx.fillStyle = "rgba(255,255,255,0.1)";
        ctx.fillRect(sx + 8, ry + 14, barMaxW, 5);
        ctx.fillStyle = r.color;
        ctx.fillRect(sx + 8, ry + 14, fillW, 5);
      });
    }

    // 7. TOP-RIGHT INTERACTIVE TOGGLE BUTTONS ON CANVAS
    drawModeToggleOnCanvas(ctx, w, lang);
  }

  // ===========================================================================
  // MODE 2: "📈 GRAFIC EROARE & TRASEU" (CHART VIEW WHEN TOGGLED)
  // ===========================================================================
  function renderRadarChartMode6yo(ctx, w, h, lang) {
    ctx.fillStyle = "#040d1a";
    ctx.fillRect(0, 0, w, h);

    const padL = 46;
    const padR = 18;
    const padT = 40;
    const padB = 26;
    const cw = w - padL - padR;
    const ch = h - padT - padB;

    // Safe zone highlight (< 5m error)
    const maxErr = 75;
    const yForErr = (err) => padT + ch - (Math.min(maxErr, Math.max(0, err)) / maxErr) * ch;

    const safeY = yForErr(6);
    ctx.fillStyle = "rgba(0, 255, 157, 0.12)";
    ctx.fillRect(padL, safeY, cw, padT + ch - safeY);

    // Grid lines
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
        ? "📈 GRAFIC EROARE ÎN TIMP REAL: Cât de departe de centrul pistei este fiecare avion (0m = Perfect pe mijloc)"
        : "📈 LIVE ERROR CHART: How far each plane is from the runway centerline (0m = Perfect on centerline)",
      12,
      22
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

    // Button 1: Game View
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

    // Button 2: Chart View
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

  // ===========================================================================
  // 3. SAFE AUTO-INTEGRATION WITH INDEX.HTML
  // ===========================================================================
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
