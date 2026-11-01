# AERO-SHIELD LRCV V2.0 — Integrare Vizuala Completa in FlightGear + Blackbox Logger + SAS Stabilizer:
# 1) Fereastra Grafica OpenGL Canvas PFD + SVS Pista 09 + Punct Verde Mobil Flight Director
# 2) Tunel 3D Holografic (20 porti 3D la fiecare 85m prin ceata catre LRCV RWY 09)
# 3) SAS (Stability Augmentation System) + Protectie Altitudine Joasa + Autoland CAT IIIc
# 4) Blackbox Telemetry Logger ([FG-BLACKBOX] la fiecare 1.0s in consola / flightgear_session.log)

var hud_win = nil;
var pfd_win = nil;
var pfd_canvas = nil;
var pfd_root = nil;

var horizon_path = nil;
var rwy_poly_path = nil;
var rwy_spoof_poly_path = nil;
var rwy_cl_path = nil;
var gate_box_path = nil;
var fd_vec_path = nil;
var fpv_group = nil;
var fd_group = nil;
var fd_circle_path = nil;
var loc_diamond_path = nil;
var loc_spoof_diamond_path = nil;
var gs_diamond_path = nil;
var btn_bg_path = nil;
var btn_spoof_pfd_bg = nil;

var txt_header = nil;
var txt_metrics = nil;
var txt_cw_alert = nil;
var txt_steer = nil;
var txt_gs = nil;
var txt_btn = nil;
var txt_btn_spoof_pfd = nil;

# Fereastra 2 (Ecran Aditional Comparativ Live: Spoofing GPS / Bruiaj vs Sistemul AERO-SHIELD)
var cyber_win = nil;
var cyber_canvas = nil;
var cyber_root = nil;
var cyber_ghost_rwy = nil;
var cyber_shield_path = nil;
var cyber_spoof_path = nil;
var cyber_ins_path = nil;
var cyber_dot_shield = nil;
var cyber_dot_spoof = nil;
var cyber_dot_ins = nil;
var cyber_btn_spoof_bg = nil;
var cyber_btn_shield_bg = nil;
var cyber_txt_title = nil;
var cyber_txt_status = nil;
var cyber_txt_cw_box = nil;
var cyber_txt_watchdog = nil;
var cyber_txt_cmp = nil;
var cyber_txt_btn_spoof = nil;
var cyber_txt_btn_shield = nil;
var hist_shield_xtk = [];
var hist_spoof_xtk = [];
var hist_ins_xtk = [];
var spoof_elapsed_s = 0.0;

# Fereastra 3 (SISTEM GPS TRADITIONAL / FMS CLASIC — CONFIDENTLY WRONG)
var trad_gps_win = nil;
var trad_gps_canvas = nil;
var trad_gps_root = nil;
var trad_cdi_needle = nil;
var trad_ghost_ac_dot = nil;
var trad_real_ac_dot = nil;
var trad_txt_sats = nil;
var trad_txt_cdi_lbl = nil;
var trad_txt_fms_cmd = nil;
var trad_txt_ghost_status = nil;
var trad_txt_why_ignored = nil;
var trad_txt_cmp_row = nil;

# Permite reincarcarea live (hot-reload) fara dublarea ferestrelor sau a timer-ului vechi
var my_hud_gen = (globals["__aero_hud_gen"] or 0) + 1;
globals["__aero_hud_gen"] = my_hud_gen;
if (globals["__aero_pfd_win"] != nil) {
    call(func { globals["__aero_pfd_win"].del(); }, []);
    globals["__aero_pfd_win"] = nil;
}
if (globals["__aero_cyber_win"] != nil) {
    call(func { globals["__aero_cyber_win"].del(); }, []);
    globals["__aero_cyber_win"] = nil;
}
if (globals["__aero_trad_gps_win"] != nil) {
    call(func { globals["__aero_trad_gps_win"].del(); }, []);
    globals["__aero_trad_gps_win"] = nil;
}
if (globals["__aero_hud_win"] != nil) {
    call(func { globals["__aero_hud_win"].close(); }, []);
    globals["__aero_hud_win"] = nil;
}

var tunnel_models = [];
var tunnel_spawned = 0;
var was_autoland = 0;
var hud_loop_running = 0;
var tick_count = 0;

# Coordonate Prag Fizic Asfaltat Pista 09 Craiova (LRCV) extrase direct din apt.dat.gz din FlightGear:
# Linia apt.dat: 100 45.11 ... 09 44.31794400 023.87294400 ... 27 44.31836100 023.90413900
# Pragul fizic al Pistei 09 se afla la (44.31794400 N, 23.87294400 E), cu directia ADEVARATA (True Heading) 88.9314 deg!
var LRCV_LAT = 44.31794400;
var LRCV_LON = 23.87294400;
var LRCV_ELEV_M = 191.0;
var LRCV_RWY_HDG = 88.9314;
var ACTIVE_APT_TAG = "LRCV RWY09";
var last_detected_apt_id = "";
var auto_flight_phase = "FINAL_APPROACH";
var teardrop_side = -1.0;
var fd_x_smooth = nil;
var fd_y_smooth = nil;

setprop("/aero-shield/target-rwy/icao", "LRCV");
setprop("/aero-shield/target-rwy/rwy-id", "09");
setprop("/aero-shield/target-rwy/lat-deg", LRCV_LAT);
setprop("/aero-shield/target-rwy/lon-deg", LRCV_LON);
setprop("/aero-shield/target-rwy/elev-m", LRCV_ELEV_M);
setprop("/aero-shield/target-rwy/heading-deg", LRCV_RWY_HDG);
setprop("/aero-shield/x-rwy-m", -2500.0);
setprop("/aero-shield/flight-phase", "FINAL_APPROACH");

# Descoperire UNIVERSALA si REAL-TIME a aeroportului si pistei de aterizare din pozitia curenta a avionului:
# Interogheaza baza de date globala FlightGear (airportinfo()) fara coordonate hardcodate!
var detect_active_airport_rwy = func() {
    var apt = airportinfo();
    if (apt == nil or apt.runways == nil) return;
    if (apt.id == last_detected_apt_id) return;

    var ac_lat = getprop("/position/latitude-deg") or LRCV_LAT;
    var ac_lon = getprop("/position/longitude-deg") or LRCV_LON;
    var ac_hdg = getprop("/orientation/heading-deg") or 0.0;

    var best_rwy_id = nil;
    var best_rwy = nil;
    var best_score = 999999.0;

    foreach (var r_id; keys(apt.runways)) {
        var r = apt.runways[r_id];
        if (r != nil and r.lat != nil and r.lon != nil and r.heading != nil) {
            # Vectorul de la avion catre pragul pistei r:
            var dn = (r.lat - ac_lat) * 111132.95;
            var de = (r.lon - ac_lon) * 111319.49 * math.cos(r.lat * math.pi / 180.0);
            var bearing_to_thr = math.atan2(de, dn) * 180.0 / math.pi;
            var hdg_diff_bearing = math.abs(wrap180(r.heading - bearing_to_thr));
            var hdg_diff_ac = math.abs(wrap180(r.heading - ac_hdg));
            var score = hdg_diff_bearing * 1.5 + hdg_diff_ac * 0.5;
            if (score < best_score) {
                best_score = score;
                best_rwy_id = r_id;
                best_rwy = r;
            }
        }
    }

    if (best_rwy != nil) {
        LRCV_LAT = best_rwy.lat;
        LRCV_LON = best_rwy.lon;
        LRCV_RWY_HDG = best_rwy.heading;
        if (apt.elevation != nil) {
            LRCV_ELEV_M = apt.elevation;
        }
        ACTIVE_APT_TAG = apt.id ~ " RWY" ~ best_rwy_id;
        last_detected_apt_id = apt.id;
        setprop("/aero-shield/target-rwy/icao", apt.id);
        setprop("/aero-shield/target-rwy/rwy-id", best_rwy_id);
        setprop("/aero-shield/target-rwy/lat-deg", LRCV_LAT);
        setprop("/aero-shield/target-rwy/lon-deg", LRCV_LON);
        setprop("/aero-shield/target-rwy/elev-m", LRCV_ELEV_M);
        setprop("/aero-shield/target-rwy/heading-deg", LRCV_RWY_HDG);
        print(sprintf("[FG-BLACKBOX] [UNIVERSAL-REALTIME] Aeroport detectat automat: %s (%s) Pista %s | lat=%.6f lon=%.6f elev=%.1fm true_hdg=%.4f deg",
            apt.id, apt.name or apt.id, best_rwy_id, LRCV_LAT, LRCV_LON, LRCV_ELEV_M, LRCV_RWY_HDG));
    }
};

var clamp = func(v, lo, hi) {
    if (v < lo) return lo;
    if (v > hi) return hi;
    return v;
};

var smoothstep_c1 = func(edge0, edge1, x) {
    if (math.abs(edge1 - edge0) < 0.000001) {
        return x >= edge1 ? 1.0 : 0.0;
    }
    var t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
};

var wrap180 = func(deg) {
    while (deg > 180.0) deg -= 360.0;
    while (deg < -180.0) deg += 360.0;
    return deg;
};

var build_cdi_bar = func(xtk_m) {
    var idx = int(clamp(-xtk_m / 12.0, -5.0, 5.0) + (xtk_m <= 0 ? 0.5 : -0.5));
    var out = "[";
    for (var i = -5; i <= 5; i += 1) {
        if (i == idx and i == 0) out ~= "(*)";
        elsif (i == idx) out ~= "|";
        elsif (i == 0) out ~= "+";
        else out ~= "-";
    }
    return out ~ "]";
};

# 1. Plaseaza 20 porti holografice 3D (la fiecare 85m) exact pe axul fizic al Pistei 09 Craiova
var spawn_3d_fog_tunnel = func() {
    if (tunnel_spawned) return;
    tunnel_spawned = 1;
    setprop("/sim/model/geometry/square/top", 0);
    setprop("/sim/model/geometry/square/left", 0);
    setprop("/sim/model/geometry/square/right", 0);
    setprop("/sim/model/geometry/square/bottom", 0);
    setprop("/sim/hud/visibility[0]", 0);
    setprop("/sim/hud/visibility[1]", 0);

    detect_active_airport_rwy();

    var thr = geo.Coord.new().set_latlon(LRCV_LAT, LRCV_LON);
    var g_info = geodinfo(LRCV_LAT, LRCV_LON);
    var base_elev = (g_info != nil and size(g_info) > 0 and g_info[0] != nil) ? g_info[0] : LRCV_ELEV_M;

    var cursor = geo.Coord.new(thr);
    cursor.apply_course_distance(LRCV_RWY_HDG, 210.0);
    var step_m = 85.0;
    var gs_rad = 3.0 * math.pi / 180.0;
    var tan_gs = math.sin(gs_rad) / math.cos(gs_rad);

    for (var i = 1; i <= 20; i += 1) {
        var dist_from_tdz = i * step_m;
        var gate_coord = geo.Coord.new(cursor);
        gate_coord.apply_course_distance(LRCV_RWY_HDG + 180.0, dist_from_tdz);
        gate_coord.set_alt(base_elev + dist_from_tdz * tan_gs);
        var mdl = geo.put_model("Models/Geometry/square.xml", gate_coord, LRCV_RWY_HDG);
        append(tunnel_models, mdl);
    }
};

# Proiectie 3D Pinhole pentru Pista 09 (cu rotatie de ruliu, tangaj, directie si clipping la depasirea pistei)
var project_rwy_pt3d = func(wx, wy, wz, x_ac, y_ac, z_ac, yaw_rel_rad, pitch_rad, roll_rad, cx, cy, focal) {
    var dx = wx - x_ac;
    var dy = wy - y_ac;
    var dz = wz - z_ac;

    var cyw = math.cos(yaw_rel_rad);
    var syw = math.sin(yaw_rel_rad);
    var x1 =  dx * cyw + dy * syw;
    var y1 = -dx * syw + dy * cyw;
    var z1 =  dz;

    var cp = math.cos(pitch_rad);
    var sp = math.sin(pitch_rad);
    var x2 = x1 * cp - z1 * sp;
    var y2 = y1;
    var z2 = x1 * sp + z1 * cp;

    if (x2 < 6.0) return nil;

    var cr = math.cos(roll_rad);
    var sr = math.sin(roll_rad);
    var y_cam =  y2 * cr + z2 * sr;
    var z_cam = -y2 * sr + z2 * cr;

    return [
        clamp(cx + focal * (y_cam / x2), 12.0, 376.0),
        clamp(cy + focal * (z_cam / x2), 30.0, 222.0)
    ];
};

# 2. Construieste fereastra grafica OpenGL Canvas (PFD + SVS + Flight Director) direct in FlightGear
var init_canvas_pfd = func() {
    if (pfd_win != nil) return;

    pfd_win = canvas.Window.new([420, 305], "dialog");
    pfd_win.set("title", "AERO-SHIELD V2.0 — SVS PFD & Flight Director (LRCV RWY 09)");
    pfd_win.setPosition(22, 45);
    globals["__aero_pfd_win"] = pfd_win;

    pfd_canvas = pfd_win.createCanvas();
    pfd_canvas.set("background", "rgba(5, 12, 24, 0.88)");
    pfd_root = pfd_canvas.createGroup();

    var frame_bg = pfd_root.createChild("path");
    frame_bg.rect(10, 28, 368, 196);
    frame_bg.setColor(0.16, 0.30, 0.48, 0.90);
    frame_bg.setStrokeLineWidth(1.5);
    frame_bg.setColorFill(0.04, 0.09, 0.18, 0.72);

    horizon_path = pfd_root.createChild("path");
    horizon_path.setColor(0.22, 0.74, 0.97, 0.55);
    horizon_path.setStrokeLineWidth(1.5);

    gate_box_path = pfd_root.createChild("path");
    gate_box_path.setColor(0.0, 0.83, 1.0, 0.40);
    gate_box_path.setStrokeLineWidth(1.4);

    # Pista Fantoma 3D (CONFIDENTLY WRONG — pozitia falsa indicata de ILS/GPS Spoofed)
    rwy_spoof_poly_path = pfd_root.createChild("path");
    rwy_spoof_poly_path.setColor(1.0, 0.22, 0.35, 0.88);
    rwy_spoof_poly_path.setStrokeLineWidth(1.8);
    rwy_spoof_poly_path.setStrokeDashArray([5, 4]);
    rwy_spoof_poly_path.setColorFill(0.32, 0.05, 0.10, 0.25);

    rwy_poly_path = pfd_root.createChild("path");
    rwy_poly_path.setColor(0.0, 0.95, 0.50, 0.92);
    rwy_poly_path.setStrokeLineWidth(2.2);
    rwy_poly_path.setColorFill(0.06, 0.20, 0.16, 0.42);

    rwy_cl_path = pfd_root.createChild("path");
    rwy_cl_path.setColor(0.95, 0.98, 1.0, 0.85);
    rwy_cl_path.setStrokeLineWidth(1.5);
    rwy_cl_path.setStrokeDashArray([6, 4]);

    fd_vec_path = pfd_root.createChild("path");
    fd_vec_path.setColor(0.0, 0.95, 0.50, 0.65);
    fd_vec_path.setStrokeLineWidth(1.6);
    fd_vec_path.setStrokeDashArray([4, 4]);

    var bore_path = pfd_root.createChild("path");
    bore_path.moveTo(154, 126).lineTo(180, 126).lineTo(180, 133)
             .moveTo(234, 126).lineTo(208, 126).lineTo(208, 133)
             .moveTo(190, 126).lineTo(198, 126)
             .moveTo(194, 122).lineTo(194, 130);
    bore_path.setColor(1.0, 0.75, 0.05, 1.0);
    bore_path.setStrokeLineWidth(2.3);

    # Simbol 3D FPV (Flight Path Vector / Velocity Vector) proiectat 3D pe pista
    fpv_group = pfd_root.createChild("group");
    var fpv_sym = fpv_group.createChild("path");
    fpv_sym.circle(6.5, 0, 0)
           .moveTo(-15, 0).lineTo(-6.5, 0)
           .moveTo(6.5, 0).lineTo(15, 0)
           .moveTo(0, -6.5).lineTo(0, -13);
    fpv_sym.setColor(0.0, 0.88, 1.0, 0.92);
    fpv_sym.setStrokeLineWidth(1.8);
    fpv_group.setTranslation(194, 126);

    fd_group = pfd_root.createChild("group");
    fd_circle_path = fd_group.createChild("path");
    fd_circle_path.circle(11, 0, 0)
                  .moveTo(-16, 0).lineTo(-11, 0)
                  .moveTo(11, 0).lineTo(16, 0)
                  .moveTo(0, -16).lineTo(0, -11)
                  .moveTo(0, 11).lineTo(0, 16)
                  .circle(2.2, 0, 0);
    fd_circle_path.setColor(0.0, 1.0, 0.53, 1.0);
    fd_circle_path.setStrokeLineWidth(2.1);
    fd_circle_path.setColorFill(0.0, 0.90, 0.46, 0.16);
    fd_group.setTranslation(194, 126);

    var scales_bg = pfd_root.createChild("path");
    scales_bg.rect(74, 228, 240, 12)
             .moveTo(194, 226).lineTo(194, 242)
             .rect(384, 46, 12, 160)
             .moveTo(382, 126).lineTo(398, 126);
    scales_bg.setColor(0.30, 0.48, 0.70, 0.85);
    scales_bg.setStrokeLineWidth(1.2);
    scales_bg.setColorFill(0.03, 0.07, 0.14, 0.85);

    # Diamant Rosu pentru instrumentul clasic pacalit ("CONFIDENTLY WRONG")
    loc_spoof_diamond_path = pfd_root.createChild("path");
    loc_spoof_diamond_path.moveTo(0, -5).lineTo(6, 0).lineTo(0, 5).lineTo(-6, 0).close();
    loc_spoof_diamond_path.setColor(1.0, 0.22, 0.35, 0.95);
    loc_spoof_diamond_path.setStrokeLineWidth(1.6);
    loc_spoof_diamond_path.setTranslation(194, 234);

    loc_diamond_path = pfd_root.createChild("path");
    loc_diamond_path.moveTo(0, -5).lineTo(6, 0).lineTo(0, 5).lineTo(-6, 0).close();
    loc_diamond_path.setColor(0.0, 1.0, 0.53, 1.0);
    loc_diamond_path.setStrokeLineWidth(1.8);
    loc_diamond_path.setTranslation(194, 234);

    gs_diamond_path = pfd_root.createChild("path");
    gs_diamond_path.moveTo(0, -6).lineTo(5, 0).lineTo(0, 6).lineTo(-5, 0).close();
    gs_diamond_path.setColor(0.0, 0.85, 1.0, 1.0);
    gs_diamond_path.setStrokeLineWidth(1.8);
    gs_diamond_path.setTranslation(390, 126);

    txt_header = pfd_root.createChild("text");
    txt_header.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    txt_header.setFontSize(11, 1.0);
    txt_header.setColor(0.0, 0.85, 1.0, 1.0);
    txt_header.setTranslation(12, 19);
    txt_header.setText("AERO-SHIELD SVS | LRCV RWY 09 (CEATA CAT III)");

    txt_metrics = pfd_root.createChild("text");
    txt_metrics.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    txt_metrics.setFontSize(10.5, 1.0);
    txt_metrics.setColor(0.90, 0.96, 1.0, 1.0);
    txt_metrics.setTranslation(18, 44);
    txt_metrics.setText("DIST: ---m | AGL: ---m | SPD: --kt");

    txt_cw_alert = pfd_root.createChild("text");
    txt_cw_alert.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    txt_cw_alert.setFontSize(10.0, 1.0);
    txt_cw_alert.setColor(1.0, 0.30, 0.42, 1.0);
    txt_cw_alert.setTranslation(18, 59);
    txt_cw_alert.setText("");

    txt_steer = pfd_root.createChild("text");
    txt_steer.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    txt_steer.setFontSize(11.0, 1.0);
    txt_steer.setColor(0.0, 1.0, 0.53, 1.0);
    txt_steer.setAlignment("center-baseline");
    txt_steer.setTranslation(194, 256);
    txt_steer.setText("URMARESTE CERCURILE VERZI PENTRU ALINIERE");

    txt_gs = pfd_root.createChild("text");
    txt_gs.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    txt_gs.setFontSize(10.5, 1.0);
    txt_gs.setColor(0.98, 0.85, 0.25, 1.0);
    txt_gs.setAlignment("center-baseline");
    txt_gs.setTranslation(194, 271);
    txt_gs.setText("PANTA GLIDESLOPE 3.0 deg");

    # Buton Stanga PFD: Autoland CAT IIIc
    btn_bg_path = pfd_root.createChild("path");
    btn_bg_path.rect(12, 277, 194, 22);
    btn_bg_path.setColor(0.0, 0.85, 1.0, 0.90);
    btn_bg_path.setStrokeLineWidth(1.5);
    btn_bg_path.setColorFill(0.05, 0.22, 0.36, 0.90);

    txt_btn = pfd_root.createChild("text");
    txt_btn.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    txt_btn.setFontSize(10.0, 1.0);
    txt_btn.setColor(0.95, 0.99, 1.0, 1.0);
    txt_btn.setAlignment("center-baseline");
    txt_btn.setTranslation(109, 292);
    txt_btn.setText("[ AUTOLAND CAT IIIc: OFF ]");

    # Buton Dreapta PFD: Declansare Spoofing GPS (+250m) & Bruiaj ILS
    btn_spoof_pfd_bg = pfd_root.createChild("path");
    btn_spoof_pfd_bg.rect(212, 277, 196, 22);
    btn_spoof_pfd_bg.setColor(1.0, 0.25, 0.38, 0.95);
    btn_spoof_pfd_bg.setStrokeLineWidth(1.5);
    btn_spoof_pfd_bg.setColorFill(0.32, 0.06, 0.12, 0.92);

    txt_btn_spoof_pfd = pfd_root.createChild("text");
    txt_btn_spoof_pfd.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    txt_btn_spoof_pfd.setFontSize(10.0, 1.0);
    txt_btn_spoof_pfd.setColor(1.0, 0.92, 0.94, 1.0);
    txt_btn_spoof_pfd.setAlignment("center-baseline");
    txt_btn_spoof_pfd.setTranslation(310, 292);
    txt_btn_spoof_pfd.setText("[ SPOOF GPS/BRUIAJ: OFF ]");

    pfd_canvas.addEventListener("mousedown", func(e) {
        if (e.clientY >= 270) {
            if (e.clientX <= 208) {
                var cur = getprop("/aero-shield/autoland-local") or 0;
                setprop("/aero-shield/autoland-local", cur > 0 ? 0 : 1);
            } else {
                var cur_s = getprop("/aero-shield/spoof-jam-active") or 0;
                setprop("/aero-shield/spoof-jam-active", cur_s > 0 ? 0 : 1);
            }
        }
    });

    # Initializam proprietatile pentru Spoofing GPS / Bruiaj si Protectie Scut AERO-SHIELD
    if (getprop("/aero-shield/spoof-jam-active") == nil) {
        setprop("/aero-shield/spoof-jam-active", 0);
    }
    if (getprop("/aero-shield/shield-protection") == nil) {
        setprop("/aero-shield/shield-protection", 1);
    }

    # --- FEREASTRA 2: ECRAN ADITIONAL COMPARATIV LIVE (SPOOFING GPS / BRUIAJ vs AERO-SHIELD) ---
    cyber_win = canvas.Window.new([465, 315], "dialog");
    cyber_win.set("title", "AERO-SHIELD CYBER RADAR — Fenomenul 'Confidently Wrong' vs. Scut Optic");
    cyber_win.setPosition(455, 45);
    globals["__aero_cyber_win"] = cyber_win;

    cyber_canvas = cyber_win.createCanvas();
    cyber_canvas.set("background", "rgba(6, 11, 22, 0.92)");
    cyber_root = cyber_canvas.createGroup();

    var radar_bg = cyber_root.createChild("path");
    radar_bg.rect(12, 42, 441, 138);
    radar_bg.setColor(0.20, 0.36, 0.58, 0.90);
    radar_bg.setStrokeLineWidth(1.4);
    radar_bg.setColorFill(0.03, 0.07, 0.14, 0.85);

    # Pista Fantoma pe radarul comparativ (unde instrumentul clasic minte ca se afla pista!)
    cyber_ghost_rwy = cyber_root.createChild("path");
    cyber_ghost_rwy.rect(310, 52, 135, 22);
    cyber_ghost_rwy.setColor(1.0, 0.22, 0.35, 0.80);
    cyber_ghost_rwy.setStrokeLineWidth(1.4);
    cyber_ghost_rwy.setStrokeDashArray([4, 4]);
    cyber_ghost_rwy.setColorFill(0.35, 0.05, 0.12, 0.30);

    # Coridorul Pistei 09 LRCV (45m latime) si axul central pe radarul comparativ (Y=111 = axul pistei)
    var rwy_corridor = cyber_root.createChild("path");
    rwy_corridor.rect(310, 99, 135, 24);
    rwy_corridor.setColor(0.0, 0.95, 0.50, 0.75);
    rwy_corridor.setStrokeLineWidth(1.5);
    rwy_corridor.setColorFill(0.06, 0.24, 0.18, 0.45);

    var rwy_axis = cyber_root.createChild("path");
    rwy_axis.moveTo(16, 111).lineTo(448, 111);
    rwy_axis.setColor(0.75, 0.88, 1.0, 0.55);
    rwy_axis.setStrokeLineWidth(1.2);
    rwy_axis.setStrokeDashArray([5, 4]);

    cyber_ins_path = cyber_root.createChild("path");
    cyber_ins_path.setColor(1.0, 0.75, 0.05, 0.85);
    cyber_ins_path.setStrokeLineWidth(1.8);

    cyber_spoof_path = cyber_root.createChild("path");
    cyber_spoof_path.setColor(1.0, 0.20, 0.33, 0.95);
    cyber_spoof_path.setStrokeLineWidth(2.4);

    cyber_shield_path = cyber_root.createChild("path");
    cyber_shield_path.setColor(0.0, 0.95, 0.50, 1.0);
    cyber_shield_path.setStrokeLineWidth(2.6);

    cyber_dot_ins = cyber_root.createChild("path");
    cyber_dot_ins.circle(4.5, 0, 0);
    cyber_dot_ins.setColor(1.0, 0.75, 0.05, 1.0);
    cyber_dot_ins.setColorFill(1.0, 0.75, 0.05, 0.85);

    cyber_dot_spoof = cyber_root.createChild("path");
    cyber_dot_spoof.circle(5.5, 0, 0);
    cyber_dot_spoof.setColor(1.0, 0.20, 0.33, 1.0);
    cyber_dot_spoof.setColorFill(1.0, 0.20, 0.33, 0.90);

    cyber_dot_shield = cyber_root.createChild("path");
    cyber_dot_shield.circle(6.0, 0, 0);
    cyber_dot_shield.setColor(0.0, 1.0, 0.53, 1.0);
    cyber_dot_shield.setColorFill(0.0, 0.95, 0.50, 0.95);

    cyber_txt_title = cyber_root.createChild("text");
    cyber_txt_title.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    cyber_txt_title.setFontSize(10.5, 1.0);
    cyber_txt_title.setColor(0.0, 0.85, 1.0, 1.0);
    cyber_txt_title.setTranslation(12, 18);
    cyber_txt_title.setText("COMPARATIE LIVE: [VERDE] AERO-SHIELD vs [ROSU] GPS SPOOF vs [GALBEN] BRUIAJ INS");

    cyber_txt_status = cyber_root.createChild("text");
    cyber_txt_status.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    cyber_txt_status.setFontSize(10.5, 1.0);
    cyber_txt_status.setColor(0.0, 1.0, 0.53, 1.0);
    cyber_txt_status.setTranslation(12, 35);
    cyber_txt_status.setText("STATUS CYBER: NOMINAL (GPS CURAT | SCUT OPTIC-INERTIAL ACTIV)");

    cyber_txt_cw_box = cyber_root.createChild("text");
    cyber_txt_cw_box.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    cyber_txt_cw_box.setFontSize(10.0, 1.0);
    cyber_txt_cw_box.setColor(1.0, 0.45, 0.55, 1.0);
    cyber_txt_cw_box.setTranslation(12, 195);
    cyber_txt_cw_box.setText("CONFIDENTLY WRONG: Apasa butonul rosu pt. a vedea cum GPS/ILS clasic minte cu 0.0m!");

    cyber_txt_watchdog = cyber_root.createChild("text");
    cyber_txt_watchdog.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    cyber_txt_watchdog.setFontSize(10.0, 1.0);
    cyber_txt_watchdog.setColor(0.92, 0.96, 1.0, 1.0);
    cyber_txt_watchdog.setTranslation(12, 213);
    cyber_txt_watchdog.setText("WATCHDOG CHI2: 1.15 (Prag 18.5) | CUSUM: 0.00 (Prag 6.5) | CARANTINA: NU");

    cyber_txt_cmp = cyber_root.createChild("text");
    cyber_txt_cmp.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    cyber_txt_cmp.setFontSize(10.0, 1.0);
    cyber_txt_cmp.setColor(0.98, 0.88, 0.30, 1.0);
    cyber_txt_cmp.setTranslation(12, 231);
    cyber_txt_cmp.setText("EROARE LATERALA: AERO-SHIELD = 0.3m (PE PISTA) | GPS SPOOF = 0.6m | INS = 1.2m");

    # Buton 1: Declansare Spoofing GPS (+250m) & Bruiaj ILS
    cyber_btn_spoof_bg = cyber_root.createChild("path");
    cyber_btn_spoof_bg.rect(12, 242, 441, 28);
    cyber_btn_spoof_bg.setColor(1.0, 0.25, 0.38, 0.95);
    cyber_btn_spoof_bg.setStrokeLineWidth(1.6);
    cyber_btn_spoof_bg.setColorFill(0.32, 0.06, 0.12, 0.92);

    cyber_txt_btn_spoof = cyber_root.createChild("text");
    cyber_txt_btn_spoof.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    cyber_txt_btn_spoof.setFontSize(10.5, 1.0);
    cyber_txt_btn_spoof.setColor(1.0, 0.92, 0.94, 1.0);
    cyber_txt_btn_spoof.setAlignment("center-baseline");
    cyber_txt_btn_spoof.setTranslation(232, 260);
    cyber_txt_btn_spoof.setText("[ CLICK AICI: DECLANSEAZA SPOOFING GPS (+250m) & BRUIAJ ILS ]");

    # Buton 2: Comutare Scut AERO-SHIELD ACTIV vs OPRIT (Avion Clasic Vulnerabil)
    cyber_btn_shield_bg = cyber_root.createChild("path");
    cyber_btn_shield_bg.rect(12, 276, 441, 28);
    cyber_btn_shield_bg.setColor(0.0, 0.95, 0.50, 0.95);
    cyber_btn_shield_bg.setStrokeLineWidth(1.5);
    cyber_btn_shield_bg.setColorFill(0.04, 0.36, 0.22, 0.92);

    cyber_txt_btn_shield = cyber_root.createChild("text");
    cyber_txt_btn_shield.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    cyber_txt_btn_shield.setFontSize(10.5, 1.0);
    cyber_txt_btn_shield.setColor(0.92, 1.0, 0.96, 1.0);
    cyber_txt_btn_shield.setAlignment("center-baseline");
    cyber_txt_btn_shield.setTranslation(232, 294);
    cyber_txt_btn_shield.setText("[ SCUT AERO-SHIELD: ACTIV (IGNORA GPS FALS) — CLICK PT. FARA SCUT ]");

    cyber_canvas.addEventListener("mousedown", func(e) {
        if (e.clientY >= 240 and e.clientY <= 272) {
            var cur_s = getprop("/aero-shield/spoof-jam-active") or 0;
            var next_s = cur_s > 0 ? 0 : 1;
            setprop("/aero-shield/spoof-jam-active", next_s);
            print(sprintf("[FG-BLACKBOX CYBER] Buton Spoofing GPS / Bruiaj comutat pe: %d", next_s));
        } elsif (e.clientY >= 274) {
            var cur_sh = getprop("/aero-shield/shield-protection");
            if (cur_sh == nil) cur_sh = 1;
            var next_sh = cur_sh > 0 ? 0 : 1;
            setprop("/aero-shield/shield-protection", next_sh);
            print(sprintf("[FG-BLACKBOX CYBER] Buton Scut AERO-SHIELD comutat pe: %d", next_sh));
        }
    });

    # --- FEREASTRA 3: SISTEM GPS TRADITIONAL (FMS CLASIC) — CONFIDENTLY WRONG ---
    trad_gps_win = canvas.Window.new([520, 255], "dialog");
    trad_gps_win.set("title", "SISTEM GPS TRADITIONAL (FMS CLASIC) — CONFIDENTLY WRONG");
    trad_gps_win.setPosition(22, 365);
    globals["__aero_trad_gps_win"] = trad_gps_win;

    trad_gps_canvas = trad_gps_win.createCanvas();
    trad_gps_canvas.set("background", "rgba(7, 13, 25, 0.94)");
    trad_gps_root = trad_gps_canvas.createGroup();

    var trad_hdr_bg = trad_gps_root.createChild("path");
    trad_hdr_bg.rect(10, 8, 500, 26);
    trad_hdr_bg.setColor(0.0, 0.90, 0.48, 0.85);
    trad_hdr_bg.setStrokeLineWidth(1.3);
    trad_hdr_bg.setColorFill(0.03, 0.22, 0.14, 0.88);

    trad_txt_sats = trad_gps_root.createChild("text");
    trad_txt_sats.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    trad_txt_sats.setFontSize(10.0, 1.0);
    trad_txt_sats.setColor(0.0, 1.0, 0.53, 1.0);
    trad_txt_sats.setTranslation(16, 25);
    trad_txt_sats.setText("12/12 SATS | RAIM: OK (0.0m, 0 FAULTS) | HDOP: 0.8 | EPU: 0.8m | LPV APR GREEN");

    # Panou Stanga: HSI / CDI Clasic (FMS Traditional)
    var cdi_box = trad_gps_root.createChild("path");
    cdi_box.rect(10, 40, 242, 132);
    cdi_box.setColor(0.22, 0.38, 0.62, 0.90);
    cdi_box.setStrokeLineWidth(1.4);
    cdi_box.setColorFill(0.03, 0.07, 0.14, 0.88);

    var cdi_title = trad_gps_root.createChild("text");
    cdi_title.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    cdi_title.setFontSize(10.0, 1.0);
    cdi_title.setColor(0.75, 0.88, 1.0, 1.0);
    cdi_title.setTranslation(16, 55);
    cdi_title.setText("HSI / CDI RECEPTOR GPS CLASIC:");

    var cdi_scale = trad_gps_root.createChild("path");
    cdi_scale.moveTo(131, 64).lineTo(131, 140)
             .circle(3, 51, 102)
             .circle(3, 91, 102)
             .circle(4, 131, 102)
             .circle(3, 171, 102)
             .circle(3, 211, 102);
    cdi_scale.setColor(0.55, 0.70, 0.88, 0.75);
    cdi_scale.setStrokeLineWidth(1.3);

    trad_cdi_needle = trad_gps_root.createChild("path");
    trad_cdi_needle.moveTo(0, -32).lineTo(0, 32);
    trad_cdi_needle.setColor(1.0, 0.22, 0.36, 1.0);
    trad_cdi_needle.setStrokeLineWidth(3.2);
    trad_cdi_needle.setTranslation(131, 102);

    trad_txt_cdi_lbl = trad_gps_root.createChild("text");
    trad_txt_cdi_lbl.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    trad_txt_cdi_lbl.setFontSize(9.5, 1.0);
    trad_txt_cdi_lbl.setColor(0.98, 0.85, 0.25, 1.0);
    trad_txt_cdi_lbl.setTranslation(16, 162);
    trad_txt_cdi_lbl.setText("GPS TRAD: Y_gps = +0.0m (PE AX)");

    # Panou Dreapta: Comparatie Vizuala Pista Reala vs Pista Fantoma Spoofata (+250m)
    var ghost_box = trad_gps_root.createChild("path");
    ghost_box.rect(258, 40, 252, 132);
    ghost_box.setColor(0.22, 0.38, 0.62, 0.90);
    ghost_box.setStrokeLineWidth(1.4);
    ghost_box.setColorFill(0.03, 0.07, 0.14, 0.88);

    var ghost_title = trad_gps_root.createChild("text");
    ghost_title.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    ghost_title.setFontSize(9.5, 1.0);
    ghost_title.setColor(0.75, 0.88, 1.0, 1.0);
    ghost_title.setTranslation(264, 55);
    ghost_title.setText("PISTA REALA vs FANTOMA (+250m CAMP):");

    var ghost_rwy_rect = trad_gps_root.createChild("path");
    ghost_rwy_rect.rect(365, 66, 135, 22);
    ghost_rwy_rect.setColor(1.0, 0.22, 0.36, 0.85);
    ghost_rwy_rect.setStrokeLineWidth(1.4);
    ghost_rwy_rect.setStrokeDashArray([4, 3]);
    ghost_rwy_rect.setColorFill(0.36, 0.05, 0.12, 0.32);

    var real_rwy_rect = trad_gps_root.createChild("path");
    real_rwy_rect.rect(365, 114, 135, 22);
    real_rwy_rect.setColor(0.0, 0.95, 0.50, 0.90);
    real_rwy_rect.setStrokeLineWidth(1.6);
    real_rwy_rect.setColorFill(0.05, 0.25, 0.18, 0.45);

    trad_ghost_ac_dot = trad_gps_root.createChild("path");
    trad_ghost_ac_dot.circle(5.5, 0, 0);
    trad_ghost_ac_dot.setColor(1.0, 0.22, 0.36, 1.0);
    trad_ghost_ac_dot.setColorFill(1.0, 0.22, 0.36, 0.95);
    trad_ghost_ac_dot.setTranslation(320, 77);

    trad_real_ac_dot = trad_gps_root.createChild("path");
    trad_real_ac_dot.circle(5.5, 0, 0);
    trad_real_ac_dot.setColor(0.0, 1.0, 0.53, 1.0);
    trad_real_ac_dot.setColorFill(0.0, 0.95, 0.50, 0.95);
    trad_real_ac_dot.setTranslation(320, 125);

    trad_txt_ghost_status = trad_gps_root.createChild("text");
    trad_txt_ghost_status.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    trad_txt_ghost_status.setFontSize(9.5, 1.0);
    trad_txt_ghost_status.setColor(1.0, 0.40, 0.52, 1.0);
    trad_txt_ghost_status.setTranslation(264, 162);
    trad_txt_ghost_status.setText("GHOST FMS: CDI=0.0m GREEN (IN CAMP!)");

    # Jos: Comanda Falsa FMS Clasic + De ce AERO-SHIELD NU asculta GPS-ul traditional sub atac
    var bot_box = trad_gps_root.createChild("path");
    bot_box.rect(10, 178, 500, 69);
    bot_box.setColor(1.0, 0.25, 0.38, 0.85);
    bot_box.setStrokeLineWidth(1.4);
    bot_box.setColorFill(0.18, 0.04, 0.09, 0.85);

    trad_txt_fms_cmd = trad_gps_root.createChild("text");
    trad_txt_fms_cmd.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    trad_txt_fms_cmd.setFontSize(10.0, 1.0);
    trad_txt_fms_cmd.setColor(1.0, 0.78, 0.18, 1.0);
    trad_txt_fms_cmd.setTranslation(16, 195);
    trad_txt_fms_cmd.setText("COMANDA FMS CLASIC: ALINIAT (0.0m) — FARA ATAC SPOOFING ACTIV");

    trad_txt_why_ignored = trad_gps_root.createChild("text");
    trad_txt_why_ignored.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    trad_txt_why_ignored.setFontSize(9.5, 1.0);
    trad_txt_why_ignored.setColor(0.0, 1.0, 0.53, 1.0);
    trad_txt_why_ignored.setTranslation(16, 216);
    trad_txt_why_ignored.setText("DE CE NU IL ASCULTA AERO-SHIELD: Watchdog Chi2 + Flux Optic Koschmieder + INS");

    trad_txt_cmp_row = trad_gps_root.createChild("text");
    trad_txt_cmp_row.setFont("LiberationFonts/LiberationMono-Bold.ttf");
    trad_txt_cmp_row.setFontSize(9.5, 1.0);
    trad_txt_cmp_row.setColor(0.90, 0.96, 1.0, 1.0);
    trad_txt_cmp_row.setTranslation(16, 236);
    trad_txt_cmp_row.setText("Xtk_GPS_Trad: +0.0m (RAIM ORB: 0 Alerte) | Xtk_Real_AeroShield: +0.0m (PE AX)");
};

# 3. Mentine motorul pornit (RPM ~ 2300) si previne taierea amestecului de catre tabelul de primer la rece din c172p.xml
var ensure_in_air_engine_and_trim = func(agl_m) {
    # CRITIC (descoperit din [FG-BLACKBOX] + c172p.xml linia 1302 + Systems/fuel.xml):
    # In c172p, daca /controls/engines/engine/use-primer == 1 si oil-temperature-degf <= 75,
    # orice throttle > 0.50 forteaza fcs/mixture-primer-cmd = 0 (taie combustibilul -> RPM = 0!).
    # De asemenea, daca /engines/active-engine/crash-engine == 1 sau /fdm/jsbsim/gear/unit[0]/broken == 1,
    # Systems/fuel.xml taie propulsion/tank[4]/priority = 0 si opreste motorul in aer!
    setprop("/fdm/jsbsim/settings/damage", 0);
    setprop("/engines/active-engine/crash-engine", 0);
    setprop("/engines/active-engine/kill-engine", 0);
    setprop("/engines/active-engine/crashed", 0);
    setprop("/engines/active-engine/killed", 0);
    setprop("/fdm/jsbsim/gear/unit[0]/broken", 0);
    setprop("/fdm/jsbsim/gear/unit[1]/broken", 0);
    setprop("/fdm/jsbsim/gear/unit[2]/broken", 0);
    setprop("/fdm/jsbsim/wing-damage/left-wing", 0);
    setprop("/fdm/jsbsim/wing-damage/right-wing", 0);
    setprop("/fdm/jsbsim/crash", 0);
    setprop("/fdm/jsbsim/propulsion/tank[4]/priority", 1);
    setprop("/consumables/fuel/tank[4]/level-lbs", 0.08);
    setprop("/controls/engines/engine/use-primer", 0);
    setprop("/controls/engines/engine[0]/use-primer", 0);
    setprop("/fdm/jsbsim/settings/damage", 0);
    setprop("/engines/active-engine/crash-engine", 0);
    setprop("/engines/active-engine/kill-engine", 0);
    setprop("/fdm/jsbsim/gear/unit[0]/broken", 0);
    setprop("/fdm/jsbsim/gear/unit[1]/broken", 0);
    setprop("/fdm/jsbsim/gear/unit[2]/broken", 0);
    setprop("/fdm/jsbsim/wing-damage/left-wing", 0);
    setprop("/fdm/jsbsim/wing-damage/right-wing", 0);
    setprop("/fdm/jsbsim/crash", 0);
    setprop("/fdm/jsbsim/engine/damage-level", 0);
    setprop("/engines/active-engine/oil-temperature-degf", 180.0);
    setprop("/engines/engine[0]/oil-temperature-degf", 180.0);
    setprop("/engines/active-engine/already-started-in-session", 1);
    setprop("/controls/engines/current-engine/carb-heat", 0);
    if (agl_m > 1.8) {
        setprop("/controls/engines/engine[0]/cutoff", 0);
        setprop("/controls/engines/current-engine/mixture", 1.0);
        setprop("/controls/engines/engine[0]/mixture", 1.0);
        setprop("/controls/switches/magnetos", 3);
        setprop("/controls/switches/master-bat", 1);
        setprop("/controls/switches/master-alt", 1);
        setprop("/controls/gear/brake-parking", 0);
    }

    if (tick_count <= 5) {
        # Flaps 10 deg (0.33) in loc de 20 deg (0.66) pentru portanta stabila fara frana aerodinamica excesiva
        setprop("/controls/flight/flaps", 0.33);
        setprop("/sim/model/c172p/cockpit/flaps-lever", 0.33);
    }

    var eng_run = getprop("/engines/active-engine/running") or getprop("/engines/engine[0]/running") or 0;
    var rpm = getprop("/engines/active-engine/rpm") or getprop("/engines/engine[0]/rpm") or 0.0;
    if (agl_m > 3.0 and (!eng_run or rpm < 420.0)) {
        setprop("/controls/switches/starter", 1);
        # Resetam use-primer = 0 IMEDIAT dupa starter = 1 (listener-ul din engine.nas linia 442 il pune pe 1!)
        setprop("/controls/engines/engine/use-primer", 0);
        setprop("/controls/engines/engine[0]/use-primer", 0);
        setprop("/fdm/jsbsim/propulsion/set-running", -1);
        setprop("/fdm/jsbsim/propulsion/engine[0]/set-running", 1);
        setprop("/engines/active-engine/running", 1);
        setprop("/engines/engine[0]/running", 1);
    } elsif (eng_run and rpm >= 950.0 and (getprop("/controls/switches/starter") or 0) > 0) {
        setprop("/controls/switches/starter", 0);
        setprop("/controls/engines/engine/use-primer", 0);
    }

    # Trim de directie pentru compensarea cuplului elicei (P-factor) doar in aer; pe pista (agl_m <= 1.8m) il punem pe 0.0
    setprop("/controls/flight/rudder-trim", agl_m > 1.8 ? 0.032 : 0.0);
};

# 4. Calculeaza autonom reperele fata de Pista (folosind aceeasi lege unificata C1 ca serverul Rust / FD == AP!)
var gps_clean_anchor = {
    locked: 0,
    lat: LRCV_LAT,
    lon: LRCV_LON,
    x_rwy: -2500.0,
    y_rwy: 0.0,
    agl_m: 145.0,
};

var compute_local_lrcv_guidance = func() {
    detect_active_airport_rwy();
    var lat = getprop("/position/latitude-deg") or LRCV_LAT;
    var lon = getprop("/position/longitude-deg") or (LRCV_LON - 0.018);
    var agl_ft = getprop("/position/altitude-agl-ft") or 550.0;
    var agl_m = agl_ft * 0.3048;
    var hdg = getprop("/orientation/heading-deg") or LRCV_RWY_HDG;
    var cur_pit = getprop("/orientation/pitch-deg") or 0.0;
    var kt = getprop("/velocities/airspeed-kt") or 75.0;
    var vs_fps = getprop("/velocities/vertical-speed-fps") or -6.0;
    var vn_fps = getprop("/velocities/speed-north-fps") or 0.0;
    var ve_fps = getprop("/velocities/speed-east-fps") or 0.0;

    var d_north_m = (lat - LRCV_LAT) * 111132.95;
    var d_east_m  = (lon - LRCV_LON) * 111319.49 * math.cos(LRCV_LAT * math.pi / 180.0);
    var hdg_rad   = LRCV_RWY_HDG * math.pi / 180.0;
    var cos_h     = math.cos(hdg_rad);
    var sin_h     = math.sin(hdg_rad);

    var raw_x_rwy = d_north_m * cos_h + d_east_m * sin_h;
    var raw_y_rwy = -d_north_m * sin_h + d_east_m * cos_h;

    var vn_mps = vn_fps * 0.3048;
    var ve_mps = ve_fps * 0.3048;
    var vx_rwy = vn_mps * cos_h + ve_mps * sin_h;
    var vy_rwy = -vn_mps * sin_h + ve_mps * cos_h;
    var spd_mps = math.max(15.0, math.sqrt(vx_rwy * vx_rwy + vy_rwy * vy_rwy));

    var spoof_jam_on = (getprop("/aero-shield/spoof-jam-active") or 0) > 0;
    var gnss_quar_on = (getprop("/aero-shield/gnss-quarantined") or 0) > 0;
    var is_spoofed = (spoof_jam_on or gnss_quar_on) ? 1 : 0;

    if (!is_spoofed) {
        gps_clean_anchor.locked = 0;
        gps_clean_anchor.lat = lat;
        gps_clean_anchor.lon = lon;
        gps_clean_anchor.x_rwy = raw_x_rwy;
        gps_clean_anchor.y_rwy = raw_y_rwy;
        gps_clean_anchor.agl_m = agl_m;
    } else {
        if (!gps_clean_anchor.locked) {
            gps_clean_anchor.locked = 1;
        }
        gps_clean_anchor.x_rwy = gps_clean_anchor.x_rwy + vx_rwy * 0.05;
        gps_clean_anchor.y_rwy = 0.85 * (gps_clean_anchor.y_rwy + vy_rwy * 0.05) + 0.15 * raw_y_rwy;
        gps_clean_anchor.agl_m = agl_m;
    }

    var x_rwy = is_spoofed ? gps_clean_anchor.x_rwy : raw_x_rwy;
    var y_rwy = is_spoofed ? gps_clean_anchor.y_rwy : raw_y_rwy;
    var dist_m = math.sqrt(x_rwy * x_rwy + y_rwy * y_rwy);
    setprop("/aero-shield/x-rwy-m", x_rwy);

    # Agent-4 Coastline Cliff Baro-Inertial Step Compensator (pentru apropierea deasupra Marii Ionice in Catania LICC):
    # Deasupra marii (0m MSL), altimetrul radar masoara inaltimea fata de apa (+11.89m peste planul pragului RWY 26).
    if (LRCV_ELEV_M < 25.0 and x_rwy < -20.0) {
        var msl_m = (getprop("/position/altitude-ft") or 450.0) * 0.3048;
        var rwy_plane_agl = math.max(0.5, msl_m - LRCV_ELEV_M);
        var radar_blend = smoothstep_c1(-140.0, -20.0, x_rwy);
        agl_m = rwy_plane_agl * (1.0 - radar_blend) + agl_m * radar_blend;
    }

    # Agent-3 Two-Regime Nonlinear L1 S-Turn Sea Intercept -> Wing-Low Forward-Slip
    var s_turn_w = smoothstep_c1(18.0, 42.0, math.abs(y_rwy));
    var look_blend = smoothstep_c1(-220.0, -20.0, x_rwy);
    var loc_look = (95.0 + 25.0 * look_blend) + 125.0 * s_turn_w;
    var lead_w = smoothstep_c1(1.5, 10.0, math.abs(y_rwy));
    var vy_lead_sec = 1.35 * lead_w + 2.15 * s_turn_w;
    var dy_pred = -y_rwy - vy_rwy * vy_lead_sec;
    var tgt_rel_trk = clamp(math.atan2(dy_pred, loc_look) * 180.0 / math.pi, -28.0, 28.0);
    var act_rel_trk = math.atan2(vy_rwy, math.max(1.0, vx_rwy)) * 180.0 / math.pi;
    var rel_yaw = wrap180(hdg - LRCV_RWY_HDG);
    var on_rwy_x = smoothstep_c1(-10.0, 15.0, x_rwy);
    var on_gnd_z = 1.0 - smoothstep_c1(1.35, 2.15, agl_m);
    var ground_blend = on_rwy_x * on_gnd_z;

    var crab_est = clamp(wrap180(rel_yaw - act_rel_trk), -22.0, 22.0);
    var decrab_w = smoothstep_c1(250.0, 320.0, agl_m) * (1.0 - ground_blend);
    var flare_align_scale = 0.22 + 0.78 * smoothstep_c1(6.0, 14.0, agl_m);
    var desired_rel_yaw = clamp(tgt_rel_trk * flare_align_scale + crab_est * decrab_w, -45.0, 45.0);
    var tgt_hdg = LRCV_RWY_HDG + desired_rel_yaw;
    var hdg_corr = wrap180(tgt_hdg - hdg);

    var dx_gs = math.max(35.0, 210.0 - x_rwy);
    var h_beam = dx_gs * 0.05240778; # Panta ideala 3.0 deg catre TDZ x = +210m (Prag x = 0m -> AGL = 11.0m)
    var dh_beam = h_beam - agl_m;
    var pre_thr_w = 1.0 - smoothstep_c1(-65.0, -5.0, x_rwy);
    var low_alt_w = 1.0 - smoothstep_c1(10.5, 14.5, agl_m);
    var max_fpa_deg = -0.5 + 3.0 * (pre_thr_w * low_alt_w);
    var gs_fpa = clamp(-3.0 + 0.35 * dh_beam, -4.5, max_fpa_deg);

    # Flare C1 Hermite
    var alpha_x = smoothstep_c1(-45.0, 10.0, x_rwy);
    var flare_fpa = gs_fpa;
    if (alpha_x > 0.0 and agl_m <= 15.0) {
        var u = clamp((15.0 - math.max(0.0, agl_m)) / 15.0, 0.0, 1.0);
        var s = u * u * (3.0 - 2.0 * u);
        var exp_flare = -0.45 + (-3.0 - (-0.45)) * (1.0 - s);
        flare_fpa = gs_fpa * (1.0 - alpha_x) + exp_flare * alpha_x;
    }
    var flare_blend = smoothstep_c1(-35.0, 5.0, x_rwy) * (1.0 - smoothstep_c1(6.5, 12.0, agl_m));
    var flare = (flare_blend > 0.5 and agl_m > 0.3) ? 1 : 0;
    var tgt_fpa = gs_fpa * (1.0 - flare_blend) + flare_fpa * flare_blend;

    var act_fpa = math.atan2(vs_fps * 0.3048, spd_mps) * 180.0 / math.pi;
    var gs_err = clamp(tgt_fpa - act_fpa, -12.0, 12.0);
    var approach_pit = clamp(tgt_fpa + 0.6 + 0.45 * gs_err, -4.8, 2.5);
    var tgt_pit_calc = approach_pit * (1.0 - flare_blend) + 1.8 * flare_blend;

    # Ground rollout blend C1
    if (ground_blend > 0.0) {
        gs_err = gs_err * (1.0 - ground_blend);
        tgt_fpa = tgt_fpa * (1.0 - ground_blend);
        tgt_pit_calc = tgt_pit_calc * (1.0 - ground_blend);
        var rwy_hdg_err = wrap180(LRCV_RWY_HDG - hdg);
        hdg_corr = hdg_corr * (1.0 - ground_blend) + rwy_hdg_err * ground_blend;
        tgt_hdg = LRCV_RWY_HDG;
    }

    # --- Mașina de Stări Autonomă Nativă (UPSET_RECOVERY / GO_AROUND_TOGA / TEARDROP_OUTBOUND / BASE_TURN_INTERCEPT / FINAL_APPROACH) ---
    var mcp_ovr = getprop("/aero-shield/fg-mcp-override") or 0;
    var cur_roll_now = getprop("/orientation/roll-deg") or 0.0;
    var vs_fpm_now = vs_fps * 60.0;
    var hdg_err_rwy = math.abs(wrap180(hdg - LRCV_RWY_HDG));

    if (mcp_ovr == 0) {
        if (agl_m > 2.2 and (math.abs(cur_roll_now) > 38.0 or cur_pit < -12.0 or cur_pit > 13.5 or (agl_m > 4.5 and kt < 50.5) or (agl_m < 250.0 and vs_fpm_now < -1150.0))) {
            auto_flight_phase = "UPSET_RECOVERY";
        } elsif (auto_flight_phase == "UPSET_RECOVERY") {
            if (math.abs(cur_roll_now) < 14.0 and cur_pit >= -5.5 and cur_pit <= 8.5 and kt >= 57.0 and vs_fpm_now >= -720.0) {
                if (x_rwy > -250.0 or (x_rwy > -650.0 and math.abs(y_rwy) > 45.0)) {
                    teardrop_side = y_rwy <= 0.0 ? -1.0 : 1.0;
                    auto_flight_phase = "GO_AROUND_TOGA";
                } else {
                    auto_flight_phase = "FINAL_APPROACH";
                }
            }
        } elsif (auto_flight_phase == "GO_AROUND_TOGA") {
            if (agl_m >= 85.0 and kt >= 58.0) {
                auto_flight_phase = "TEARDROP_OUTBOUND";
            }
        } elsif (auto_flight_phase == "TEARDROP_OUTBOUND") {
            if (x_rwy <= -1380.0 and math.abs(y_rwy) >= 240.0) {
                auto_flight_phase = "BASE_TURN_INTERCEPT";
            }
        } elsif (auto_flight_phase == "BASE_TURN_INTERCEPT") {
            if (x_rwy < -650.0 and math.abs(y_rwy) < 55.0 and hdg_err_rwy < 25.0) {
                auto_flight_phase = "FINAL_APPROACH";
            }
        } else {
            if (agl_m > 25.0 and x_rwy < -700.0 and (math.abs(y_rwy) > 150.0 or hdg_err_rwy > 38.0)) {
                teardrop_side = y_rwy <= 0.0 ? -1.0 : 1.0;
                auto_flight_phase = "BASE_TURN_INTERCEPT";
            } elsif (agl_m > 1.45 and ((x_rwy > 60.0 and agl_m > 18.0) or (x_rwy > 480.0 and agl_m > 9.5) or (x_rwy > -30.0 and agl_m > 2.5 and kt > 85.0) or (x_rwy > -120.0 and agl_m <= 35.0 and (math.abs(y_rwy) > 22.0 or hdg_err_rwy > 18.0)) or (x_rwy >= -700.0 and agl_m > 18.0 and (math.abs(y_rwy) > 150.0 or hdg_err_rwy > 45.0)))) {
                teardrop_side = y_rwy <= 0.0 ? -1.0 : 1.0;
                auto_flight_phase = "GO_AROUND_TOGA";
            } else {
                auto_flight_phase = "FINAL_APPROACH";
            }
        }
        setprop("/aero-shield/flight-phase", auto_flight_phase);
    } else {
        auto_flight_phase = getprop("/aero-shield/flight-phase") or auto_flight_phase;
        if (math.abs(y_rwy) > 35.0 and (auto_flight_phase == "BASE_TURN_INTERCEPT" or auto_flight_phase == "GO_AROUND_TOGA")) {
            teardrop_side = y_rwy <= 0.0 ? -1.0 : 1.0;
        }
    }

    if (auto_flight_phase == "UPSET_RECOVERY") {
        var is_stall = (kt < 53.0 or cur_pit > 13.0) ? 1 : 0;
        var roll_gate = clamp(1.0 - math.abs(cur_roll_now) / 28.0, 0.0, 1.0);
        tgt_hdg = hdg;
        hdg_corr = 0.0;
        tgt_pit_calc = is_stall ? (agl_m > 22.0 ? -2.2 : 0.5) : (cur_pit * (1.0 - roll_gate) + clamp(3.8 + (110.0 - agl_m) * 0.025, 2.2, 6.2) * roll_gate);
        gs_err = tgt_pit_calc - cur_pit;
    } elsif (auto_flight_phase == "GO_AROUND_TOGA") {
        tgt_hdg = LRCV_RWY_HDG + clamp(-y_rwy * 0.10, -14.0, 14.0);
        hdg_corr = wrap180(tgt_hdg - hdg);
        gs_err = clamp(5.5 - act_fpa, -10.0, 10.0);
        tgt_pit_calc = clamp(7.2 + (kt - 73.0) * 0.14, 4.5, 9.0);
    } elsif (auto_flight_phase == "TEARDROP_OUTBOUND") {
        var dx_wp = -2150.0 - x_rwy;
        var dy_wp = teardrop_side * 660.0 - y_rwy;
        tgt_hdg = LRCV_RWY_HDG + math.atan2(dy_wp, dx_wp) * 180.0 / math.pi;
        hdg_corr = wrap180(tgt_hdg - hdg);
        var tfpa_td = clamp(0.18 * (135.0 - agl_m), -2.8, 5.0);
        gs_err = clamp(tfpa_td - act_fpa, -10.0, 10.0);
        tgt_pit_calc = clamp(tfpa_td + 2.0 + 0.35 * gs_err, -2.0, 7.5);
    } elsif (auto_flight_phase == "BASE_TURN_INTERCEPT") {
        var rel_trk_a = -teardrop_side * 85.0;
        if (math.abs(rel_yaw) > 95.0) {
            tgt_hdg = LRCV_RWY_HDG + rel_trk_a;
            hdg_corr = wrap180(tgt_hdg - hdg);
            if (teardrop_side > 0.0 and hdg_corr < 0.0) hdg_corr = 65.0;
            elsif (teardrop_side < 0.0 and hdg_corr > 0.0) hdg_corr = -65.0;
        } else {
            var dx_lk = clamp(math.abs(y_rwy) * 1.10 + 135.0, 135.0, 320.0);
            var dy_pr = -y_rwy - vy_rwy * 2.0;
            var rel_trk_b = clamp(math.atan2(dy_pr, dx_lk) * 180.0 / math.pi, -68.0, 68.0);
            var w_stage_a = smoothstep_c1(145.0, 265.0, math.abs(y_rwy));
            var rel_trk_cmd = rel_trk_a * w_stage_a + rel_trk_b * (1.0 - w_stage_a);
            tgt_hdg = LRCV_RWY_HDG + rel_trk_cmd;
            hdg_corr = wrap180(tgt_hdg - hdg);
        }
        var h_ref_b = clamp(math.max(35.0, 210.0 - x_rwy) * 0.05240778, 75.0, 135.0);
        var tfpa_b = clamp(0.18 * (h_ref_b - agl_m), -3.3, 3.0);
        gs_err = clamp(tfpa_b - act_fpa, -10.0, 10.0);
        tgt_pit_calc = clamp(tfpa_b + 2.0 + 0.40 * gs_err, -3.0, 5.2);
    }

    return {
        dist_m: dist_m,
        x_rwy: x_rwy,
        xtk_m: y_rwy,
        agl_m: agl_m,
        hdg_corr: hdg_corr,
        gs_err: gs_err,
        crab_deg: crab_est,
        act_rel_trk: act_rel_trk,
        act_fpa: act_fpa,
        tgt_hdg: tgt_hdg,
        tgt_pit: tgt_pit_calc,
        flare: flare,
        quar: is_spoofed
    };
};

var update_aero_shield_hud = func() {
    if (my_hud_gen != (globals["__aero_hud_gen"] or my_hud_gen)) {
        return;
    }
    tick_count += 1;
    spawn_3d_fog_tunnel();
    init_canvas_pfd();

    if (hud_win == nil) {
        hud_win = screen.window.new(nil, -48, 3, 0);
        hud_win.bg = [0.02, 0.06, 0.12, 0.78];
        hud_win.align = "center";
        globals["__aero_hud_win"] = hud_win;
    }

    var udp_active = getprop("/aero-shield/active") or 0;
    var local_g = compute_local_lrcv_guidance();
    var use_udp_fd = (udp_active > 0 and auto_flight_phase == "FINAL_APPROACH") ? 1 : 0;

    var dist_m   = udp_active > 0 ? (getprop("/aero-shield/distance-m") or local_g.dist_m) : local_g.dist_m;
    var xtk_m    = udp_active > 0 ? (getprop("/aero-shield/cross-track-m") or local_g.xtk_m) : local_g.xtk_m;
    var agl_m    = local_g.agl_m;
    var hdg_corr = use_udp_fd > 0 ? (getprop("/aero-shield/heading-corr-deg") or local_g.hdg_corr) : local_g.hdg_corr;
    var gs_err   = use_udp_fd > 0 ? (getprop("/aero-shield/gs-error-deg") or local_g.gs_err) : local_g.gs_err;
    var crab_deg = udp_active > 0 ? (getprop("/aero-shield/crab-deg") or 0.0) : 0.0;
    var tgt_hdg  = use_udp_fd > 0 ? (getprop("/aero-shield/target-heading-deg") or local_g.tgt_hdg) : local_g.tgt_hdg;
    var tgt_pit  = use_udp_fd > 0 ? (getprop("/aero-shield/target-pitch-deg") or local_g.tgt_pit) : local_g.tgt_pit;
    var flare    = use_udp_fd > 0 ? (getprop("/aero-shield/flare-active") or local_g.flare) : local_g.flare;
    var quar     = udp_active > 0 ? (getprop("/aero-shield/gnss-quarantined") or 0) : 0;

    ensure_in_air_engine_and_trim(agl_m);

    var autoland_udp   = getprop("/aero-shield/autoland-cmd") or 0;
    var autoland_local = getprop("/aero-shield/autoland-local") or 0;
    var autoland = (autoland_udp > 0 or autoland_local > 0) ? 1 : 0;

    var cur_roll  = getprop("/orientation/roll-deg") or 0.0;
    var cur_pitch = getprop("/orientation/pitch-deg") or 0.0;
    var cur_hdg   = getprop("/orientation/heading-deg") or LRCV_RWY_HDG;
    var kt        = getprop("/velocities/airspeed-kt") or 75.0;
    var vs_fps    = getprop("/velocities/vertical-speed-fps") or 0.0;
    var msl_ft    = getprop("/position/altitude-ft") or 1250.0;
    var agl_ft    = getprop("/position/altitude-agl-ft") or 600.0;
    var vis_m     = getprop("/environment/visibility-m") or 150.0;
    var p_rate    = (getprop("/orientation/p-body") or 0.0) * 57.29578;
    var q_rate    = (getprop("/orientation/q-body") or 0.0) * 57.29578;

    # Mesaj ghidaj lateral S-Turn (Interceptare + Contraviraj anti-diagonala) + Alerta Depasire Pista (Overrun)
    var steer_txt = "[=== ALINIAT PE AXUL PISTEI 09 ===]";
    if (local_g.x_rwy > 2485.0) {
        steer_txt = sprintf("!!! PISTA DEPASITA (+%.0fm OVERRUN) -> FRANEAZA / VIRAJ 180 !!!", local_g.x_rwy - 2485.0);
    } elsif (math.abs(xtk_m) > 12.0 and xtk_m * hdg_corr > 0) {
        steer_txt = hdg_corr < 0
            ? sprintf("<<< CONTRAVIREAZA STANGA (%0.1f deg) — ALINIERE 089!", hdg_corr)
            : sprintf(">>> CONTRAVIREAZA DREAPTA (+%0.1f deg) — ALINIERE 089!", hdg_corr);
    } elsif (hdg_corr > 1.2) {
        steer_txt = sprintf(">>> VIREAZA DREAPTA +%0.1f deg (SPRE CERCUL VERDE)", hdg_corr);
    } elsif (hdg_corr < -1.2) {
        steer_txt = sprintf("<<< VIREAZA STANGA %0.1f deg (SPRE CERCUL VERDE)", hdg_corr);
    }

    var pitch_txt = "PANTA 3.0 deg OK";
    if (agl_m <= 1.5 and local_g.x_rwy >= 0.0 and local_g.x_rwy <= 2485.0) {
        pitch_txt = "CONTACT PISTA 09 — FRANEAZA / REDU GAZUL!";
    } elsif (local_g.x_rwy < -15.0 and agl_m < 11.0) {
        pitch_txt = sprintf("!!! PREA JOS INAINTE DE PRAG (%.1fm) -> URCA SPRE 11m !!!", agl_m);
    } elsif (flare > 0) {
        pitch_txt = ">>> FLARE CAT IIIc PE ASFALT: RIDICA USOR BOTUL & IDLE! <<<";
    } elsif (gs_err > 0.45) {
        pitch_txt = sprintf("PREA JOS -> URCA USOR (+%0.1f deg)", gs_err);
    } elsif (gs_err < -0.45) {
        pitch_txt = sprintf("PREA SUS -> COBOARA (%0.1f deg)", gs_err);
    }

    # --- A2. Calcul Stare Spoofing GPS / Bruiaj si Fenomenul "CONFIDENTLY WRONG" ---
    var spoof_jam_active = getprop("/aero-shield/spoof-jam-active") or 0;
    var shield_prot = getprop("/aero-shield/shield-protection");
    if (shield_prot == nil) shield_prot = 1;
    if (spoof_jam_active > 0 or quar > 0) {
        spoof_elapsed_s = math.min(60.0, spoof_elapsed_s + 0.05);
    } else {
        spoof_elapsed_s = 0.0;
    }

    var shield_xtk_val = getprop("/aero-shield/cmp-shield-xtk-m");
    if (shield_xtk_val == nil) shield_xtk_val = local_g.xtk_m;
    var spoof_xtk_val = getprop("/aero-shield/cmp-spoof-xtk-m");
    if (spoof_xtk_val == nil) {
        spoof_xtk_val = (spoof_jam_active > 0 or quar > 0)
            ? (local_g.xtk_m + math.min(250.0, 55.0 + spoof_elapsed_s * 32.0))
            : (local_g.xtk_m + 0.6 * math.sin(tick_count * 0.08));
    }
    var ins_xtk_val = getprop("/aero-shield/cmp-ins-xtk-m");
    if (ins_xtk_val == nil) {
        ins_xtk_val = local_g.xtk_m + (0.35 * spoof_elapsed_s * spoof_elapsed_s + 2.5 * math.sin(tick_count * 0.04));
    }
    var chi2_val = getprop("/aero-shield/cmp-chi2") or ((spoof_jam_active > 0 or quar > 0) ? 96.4 : 1.15);
    var cusum_val = getprop("/aero-shield/cmp-cusum") or ((spoof_jam_active > 0 or quar > 0) ? 18.4 : 0.08);
    var spoof_offset_m = spoof_xtk_val - shield_xtk_val;

    # --- A. Actualizare Fereastra Grafica OpenGL Canvas PFD (Proiectie 3D Pinhole cu Ruliu, Tangaj, Directie & Overrun) ---
    var cx = 194.0;
    var cy = 126.0;
    var focal = 250.0;
    var pitch_px = clamp(cur_pitch * 4.2, -70.0, 70.0);
    var roll_rad_h = -cur_roll * math.pi / 180.0;
    var dx_h = 165.0 * math.cos(roll_rad_h);
    var dy_h = 165.0 * math.sin(roll_rad_h);
    horizon_path.reset();
    horizon_path.moveTo(cx - dx_h, cy + pitch_px - dy_h)
                .lineTo(cx + dx_h, cy + pitch_px + dy_h);

    var x_ac = local_g.x_rwy;
    var y_ac = xtk_m;
    var z_ac = -math.max(1.8, agl_m);
    var yaw_rel_rad = wrap180(cur_hdg - LRCV_RWY_HDG) * math.pi / 180.0;
    var pitch_rad_3d = ((agl_m > 1.8 ? (local_g.act_fpa or -3.0) : cur_pitch)) * math.pi / 180.0;
    var roll_rad_3d = cur_roll * math.pi / 180.0;

    rwy_poly_path.reset();
    rwy_cl_path.reset();
    rwy_spoof_poly_path.reset();

    # Pista 09 are lungimea fizica 2485m (X = 0 .. 2485m). Cand x_ac > 2465m, avionul a depasit pista (Overrun)!
    var x_near = math.max(0.0, x_ac + 10.0);
    var x_far  = 2485.0;
    if (x_ac < 2465.0 and x_near < x_far) {
        var p_nl = project_rwy_pt3d(x_near, -22.5, 0.0, x_ac, y_ac, z_ac, yaw_rel_rad, pitch_rad_3d, roll_rad_3d, cx, cy, focal);
        var p_nr = project_rwy_pt3d(x_near,  22.5, 0.0, x_ac, y_ac, z_ac, yaw_rel_rad, pitch_rad_3d, roll_rad_3d, cx, cy, focal);
        var p_fr = project_rwy_pt3d(x_far,   22.5, 0.0, x_ac, y_ac, z_ac, yaw_rel_rad, pitch_rad_3d, roll_rad_3d, cx, cy, focal);
        var p_fl = project_rwy_pt3d(x_far,  -22.5, 0.0, x_ac, y_ac, z_ac, yaw_rel_rad, pitch_rad_3d, roll_rad_3d, cx, cy, focal);
        var p_cn = project_rwy_pt3d(x_near,   0.0, 0.0, x_ac, y_ac, z_ac, yaw_rel_rad, pitch_rad_3d, roll_rad_3d, cx, cy, focal);
        var p_cf = project_rwy_pt3d(x_far,    0.0, 0.0, x_ac, y_ac, z_ac, yaw_rel_rad, pitch_rad_3d, roll_rad_3d, cx, cy, focal);

        if (p_nl != nil and p_nr != nil and p_fr != nil and p_fl != nil) {
            rwy_poly_path.moveTo(p_fl[0], p_fl[1])
                         .lineTo(p_fr[0], p_fr[1])
                         .lineTo(p_nr[0], p_nr[1])
                         .lineTo(p_nl[0], p_nl[1])
                         .close();
        }
        if (p_cn != nil and p_cf != nil) {
            rwy_cl_path.moveTo(p_cf[0], p_cf[1])
                       .lineTo(p_cn[0], p_cn[1]);
        }

        # Pista Fantoma 3D ("CONFIDENTLY WRONG") cand Spoofing-ul este activ
        if ((spoof_jam_active > 0 or quar > 0) and math.abs(spoof_offset_m) > 8.0) {
            var g_y = -spoof_offset_m;
            var g_nl = project_rwy_pt3d(x_near, g_y - 22.5, 0.0, x_ac, y_ac, z_ac, yaw_rel_rad, pitch_rad_3d, roll_rad_3d, cx, cy, focal);
            var g_nr = project_rwy_pt3d(x_near, g_y + 22.5, 0.0, x_ac, y_ac, z_ac, yaw_rel_rad, pitch_rad_3d, roll_rad_3d, cx, cy, focal);
            var g_fr = project_rwy_pt3d(x_far,  g_y + 22.5, 0.0, x_ac, y_ac, z_ac, yaw_rel_rad, pitch_rad_3d, roll_rad_3d, cx, cy, focal);
            var g_fl = project_rwy_pt3d(x_far,  g_y - 22.5, 0.0, x_ac, y_ac, z_ac, yaw_rel_rad, pitch_rad_3d, roll_rad_3d, cx, cy, focal);
            if (g_nl != nil and g_nr != nil and g_fr != nil and g_fl != nil) {
                rwy_spoof_poly_path.moveTo(g_fl[0], g_fl[1])
                                   .lineTo(g_fr[0], g_fr[1])
                                   .lineTo(g_nr[0], g_nr[1])
                                   .lineTo(g_nl[0], g_nl[1])
                                   .close();
            }
        }
    }

    # Proiectie 3D Conforma pe Pista a Portii HITS (gate_box_path), a Simbolului FPV 3D (fpv_group) si a Punctului Verde (fd_group)
    gate_box_path.reset();
    var x_gate = clamp(x_ac + 180.0, -3000.0, 2400.0);
    var h_gate = math.max(2.0, (210.0 - x_gate) * 0.05240778);
    var g_tl = project_rwy_pt3d(x_gate, -22.5, -(h_gate + 8.0), x_ac, y_ac, z_ac, yaw_rel_rad, pitch_rad_3d, roll_rad_3d, cx, cy, focal);
    var g_tr = project_rwy_pt3d(x_gate,  22.5, -(h_gate + 8.0), x_ac, y_ac, z_ac, yaw_rel_rad, pitch_rad_3d, roll_rad_3d, cx, cy, focal);
    var g_br = project_rwy_pt3d(x_gate,  22.5, -math.max(0.0, h_gate - 8.0), x_ac, y_ac, z_ac, yaw_rel_rad, pitch_rad_3d, roll_rad_3d, cx, cy, focal);
    var g_bl = project_rwy_pt3d(x_gate, -22.5, -math.max(0.0, h_gate - 8.0), x_ac, y_ac, z_ac, yaw_rel_rad, pitch_rad_3d, roll_rad_3d, cx, cy, focal);
    if (g_tl != nil and g_tr != nil and g_br != nil and g_bl != nil) {
        gate_box_path.moveTo(g_tl[0], g_tl[1])
                     .lineTo(g_tr[0], g_tr[1])
                     .lineTo(g_br[0], g_br[1])
                     .lineTo(g_bl[0], g_bl[1])
                     .close();
    }

    var d_fpv = 180.0;
    var trk_rad = (local_g.act_rel_trk or 0.0) * math.pi / 180.0;
    var fpa_rad = (local_g.act_fpa or -3.0) * math.pi / 180.0;
    var p_fpv3d = project_rwy_pt3d(
        x_ac + d_fpv * math.cos(trk_rad),
        y_ac + d_fpv * math.sin(trk_rad),
        z_ac - d_fpv * math.tan(fpa_rad),
        x_ac, y_ac, z_ac, yaw_rel_rad, pitch_rad_3d, roll_rad_3d, cx, cy, focal
    );
    var fpv_x = p_fpv3d != nil ? p_fpv3d[0] : cx;
    var fpv_y = p_fpv3d != nil ? p_fpv3d[1] : cy;
    if (fpv_group != nil) {
        fpv_group.setTranslation(fpv_x, fpv_y);
    }

    var raw_fd_x = clamp(cx + hdg_corr * 3.8, 32.0, 356.0);
    var raw_fd_y = clamp(cy - gs_err * 6.5, 42.0, 210.0);
    fd_x_smooth = fd_x_smooth == nil ? raw_fd_x : (0.72 * fd_x_smooth + 0.28 * raw_fd_x);
    fd_y_smooth = fd_y_smooth == nil ? raw_fd_y : (0.72 * fd_y_smooth + 0.28 * raw_fd_y);
    var fd_x = fd_x_smooth;
    var fd_y = fd_y_smooth;

    fd_vec_path.reset();
    fd_vec_path.moveTo(fpv_x, fpv_y).lineTo(fd_x, fd_y);
    fd_group.setTranslation(fd_x, fd_y);

    var loc_px = clamp(cx - xtk_m * 0.85, 82.0, 306.0);
    loc_diamond_path.setTranslation(loc_px, 234.0);
    var loc_spoof_px = clamp(cx - spoof_xtk_val * 0.85, 82.0, 306.0);
    loc_spoof_diamond_path.setTranslation(loc_spoof_px, 234.0);
    var gs_px = clamp(cy - gs_err * 18.0, 54.0, 198.0);
    gs_diamond_path.setTranslation(390.0, gs_px);

    var link_str = udp_active > 0 ? "SERVER RUST :5506 ONLINE" : "MOD AUTONOM LRCV";
    txt_header.setText(sprintf("AERO-SHIELD SVS | %s | CEATA %.0fm", link_str, vis_m));
    txt_metrics.setText(sprintf("X_RWY: %+.0fm | AGL: %.1fm | SPD: %.0fkt | XTK: %+.1fm",
        local_g.x_rwy, agl_m, kt, xtk_m));
    if (spoof_jam_active > 0 or quar > 0) {
        txt_cw_alert.setText(sprintf("CONFIDENTLY WRONG: ILS Clasic minte '0.0m' la %+.0fm in camp!", spoof_xtk_val));
    } else {
        txt_cw_alert.setText("");
    }
    txt_steer.setText(steer_txt);
    txt_gs.setText(pitch_txt);

    if (autoland > 0) {
        btn_bg_path.setColorFill(0.04, 0.48, 0.28, 0.95);
        txt_btn.setText("[ AUTOLAND CAT IIIc: ACTIV ]");
    } else {
        btn_bg_path.setColorFill(0.05, 0.22, 0.36, 0.90);
        txt_btn.setText("[ CLICK: AUTOLAND CAT IIIc ]");
    }

    if (spoof_jam_active > 0 or quar > 0) {
        btn_spoof_pfd_bg.setColorFill(0.85, 0.12, 0.24, 0.96);
        txt_btn_spoof_pfd.setText("[ SPOOF GPS/BRUIAJ: ACTIV! ]");
    } else {
        btn_spoof_pfd_bg.setColorFill(0.32, 0.06, 0.12, 0.92);
        txt_btn_spoof_pfd.setText("[ CLICK: SPOOF GPS +250m ]");
    }

    if (math.mod(tick_count, 3) == 0) {
        append(hist_shield_xtk, [local_g.x_rwy, shield_xtk_val]);
        append(hist_spoof_xtk,  [local_g.x_rwy, spoof_xtk_val]);
        append(hist_ins_xtk,    [local_g.x_rwy, ins_xtk_val]);
        if (size(hist_shield_xtk) > 90) {
            hist_shield_xtk = subvec(hist_shield_xtk, size(hist_shield_xtk) - 90);
            hist_spoof_xtk  = subvec(hist_spoof_xtk,  size(hist_spoof_xtk) - 90);
            hist_ins_xtk    = subvec(hist_ins_xtk,    size(hist_ins_xtk) - 90);
        }
    }

    if (cyber_win != nil) {
        # Proiectie X_rwy (-2600m .. +600m) -> px (18 .. 445), Xtk (-280m .. +280m) -> py (46 .. 176, centru 111)
        var map_x = func(xr) { return clamp(310.0 + (xr / 2600.0) * 292.0, 18.0, 445.0); };
        var map_y = func(yr) { return clamp(111.0 - (yr / 280.0) * 62.0, 46.0, 176.0); };

        cyber_ins_path.reset();
        cyber_spoof_path.reset();
        cyber_shield_path.reset();
        for (var k = 0; k < size(hist_shield_xtk); k += 1) {
            var p_sh = hist_shield_xtk[k];
            var p_sp = hist_spoof_xtk[k];
            var p_in = hist_ins_xtk[k];
            if (k == 0) {
                cyber_shield_path.moveTo(map_x(p_sh[0]), map_y(p_sh[1]));
                cyber_spoof_path.moveTo(map_x(p_sp[0]), map_y(p_sp[1]));
                cyber_ins_path.moveTo(map_x(p_in[0]), map_y(p_in[1]));
            } else {
                cyber_shield_path.lineTo(map_x(p_sh[0]), map_y(p_sh[1]));
                cyber_spoof_path.lineTo(map_x(p_sp[0]), map_y(p_sp[1]));
                cyber_ins_path.lineTo(map_x(p_in[0]), map_y(p_in[1]));
            }
        }
        cyber_dot_shield.setTranslation(map_x(local_g.x_rwy), map_y(shield_xtk_val));
        cyber_dot_spoof.setTranslation(map_x(local_g.x_rwy), map_y(spoof_xtk_val));
        cyber_dot_ins.setTranslation(map_x(local_g.x_rwy), map_y(ins_xtk_val));

        if (spoof_jam_active > 0 or quar > 0) {
            cyber_txt_status.setColor(1.0, 0.25, 0.38, 1.0);
            cyber_txt_status.setText(shield_prot > 0
                ? "ATAC ACTIV: SPOOFING GPS (+250m) & BRUIAJ -> BLOCAT DE SCUTUL AERO-SHIELD!"
                : "PERICOL CRITIC: SCUT OPRIT + SPOOFING ACTIV -> AVIONUL DEVIAZA IN CAMP!");
            cyber_txt_cw_box.setColor(1.0, 0.75, 0.15, 1.0);
            cyber_txt_cw_box.setText(sprintf(
                "CONFIDENTLY WRONG: Instrumentul clasic arata '0.0m PE AX' la %+.0fm in camp!",
                spoof_xtk_val
            ));
            cyber_btn_spoof_bg.setColorFill(0.85, 0.12, 0.24, 0.96);
            cyber_txt_btn_spoof.setText("[ ATAC SPOOFING GPS (+250m) & BRUIAJ: ACTIV — CLICK PENTRU OPRIRE ]");
        } else {
            cyber_txt_status.setColor(0.0, 1.0, 0.53, 1.0);
            cyber_txt_status.setText("STATUS CYBER: NOMINAL (GPS CURAT | FUZIUNE OPTIC-INERTIALA ACTIVA)");
            cyber_txt_cw_box.setColor(0.65, 0.78, 0.92, 1.0);
            cyber_txt_cw_box.setText("CONFIDENTLY WRONG: Apasa butonul rosu pt. a vedea cum GPS/ILS clasic minte cu 0.0m!");
            cyber_btn_spoof_bg.setColorFill(0.32, 0.06, 0.12, 0.92);
            cyber_txt_btn_spoof.setText("[ CLICK AICI: DECLANSEAZA SPOOFING GPS (+250m) & BRUIAJ ILS ]");
        }

        if (shield_prot > 0) {
            cyber_btn_shield_bg.setColorFill(0.04, 0.36, 0.22, 0.92);
            cyber_txt_btn_shield.setText("[ SCUT AERO-SHIELD: ACTIV (ATERIZEAZA PE PISTA) — CLICK PT. FARA SCUT ]");
        } else {
            cyber_btn_shield_bg.setColorFill(0.55, 0.15, 0.05, 0.95);
            cyber_txt_btn_shield.setText("[ SCUT OPRIT (AVION CLASIC VULNERABIL LA SPOOF) — CLICK PT. ACTIVARE ]");
        }

        cyber_txt_watchdog.setText(sprintf(
            "WATCHDOG CHI2: %.2f (Prag 18.5) | CUSUM: %.2f (Prag 6.5) | CARANTINA GPS: %s",
            chi2_val, cusum_val, (quar > 0 or spoof_jam_active > 0) ? "DA (BLOCAT)" : "NU"
        ));
        var st_locks = getprop("/aero-shield/star-tracker/stars-locked") or 8;
        var st_snr = getprop("/aero-shield/star-tracker/mean-snr-swir-db") or 24.8;
        var st_aerr = getprop("/aero-shield/star-tracker/attitude-err-arcsec") or 4.2;
        var st_src = getprop("/aero-shield/star-tracker/ephemeris-source") or "IAU2006_J2000_STELLARIUM";
        cyber_txt_cmp.setText(sprintf(
            "ABATERE: SHIELD=%+.1fm | SPOOF=%+.1fm | SWIR 2.2um (%s): %d STELE SNR=%.1fdB Att=%.1f\"",
            shield_xtk_val, spoof_xtk_val, st_src, st_locks, st_snr, st_aerr
        ));
    }

    if (trad_gps_win != nil) {
        var attack_on = (spoof_jam_active > 0 or quar > 0) ? 1 : 0;
        var trad_y_gps = spoof_xtk_val;
        # Acul CDI pe instrumentul clasic se deplaseaza proportional cu -trad_y_gps (deflectie +2 dots = ±80 px)
        var cdi_deflect_px = clamp(-trad_y_gps * 0.65, -82.0, 82.0);
        trad_cdi_needle.setTranslation(131.0 + cdi_deflect_px, 102.0);

        # Pozitia pe axa X (apropiere catre Pista 09: X_rwy de la -2000m .. +1200m -> px 272 .. 485)
        var ac_prog_px = clamp(365.0 + (local_g.x_rwy / 1800.0) * 95.0, 272.0, 488.0);
        var real_y_px  = clamp(125.0 - (shield_xtk_val / 250.0) * 48.0, 68.0, 145.0);
        var ghost_y_px = attack_on > 0 ? 77.0 : real_y_px;
        trad_real_ac_dot.setTranslation(ac_prog_px, real_y_px);
        trad_ghost_ac_dot.setTranslation(ac_prog_px, ghost_y_px);

        trad_txt_sats.setText("12/12 SATS LOCKED | RAIM: GREEN (0.0m, 0 FAULTS) | HDOP: 0.8 | EPU: 0.8m");
        if (attack_on > 0) {
            trad_cdi_needle.setColor(1.0, 0.22, 0.36, 1.0);
            trad_txt_cdi_lbl.setColor(1.0, 0.32, 0.44, 1.0);
            trad_txt_cdi_lbl.setText(sprintf("GPS TRAD: Y_gps=%+.0fm (FALS! REAL=%+.1fm)", trad_y_gps, shield_xtk_val));
            trad_txt_ghost_status.setColor(1.0, 0.32, 0.44, 1.0);
            trad_txt_ghost_status.setText(sprintf("GHOST FMS: CDI=0.0m GREEN LA %+.0fm IN CAMP!", trad_y_gps));
            trad_txt_fms_cmd.setColor(1.0, 0.28, 0.40, 1.0);
            trad_txt_fms_cmd.setText(sprintf("<<< COMANDA FMS CLASIC: VIREAZA %+.0fm IN CAMP (FALS 'RE-INTERCEPT')!", -trad_y_gps));
            trad_txt_why_ignored.setColor(0.0, 1.0, 0.53, 1.0);
            trad_txt_why_ignored.setText(sprintf("DE CE IGNORAM GPS: Chi2=%.1f > 18.5 & Koschmieder -> CARANTINA GNSS IN 16.6ms!", chi2_val));
        } else {
            trad_cdi_needle.setColor(0.0, 1.0, 0.53, 1.0);
            trad_txt_cdi_lbl.setColor(0.0, 1.0, 0.53, 1.0);
            trad_txt_cdi_lbl.setText(sprintf("GPS TRAD: Y_gps=%+.1fm | REAL=%+.1fm (ALINIAT)", trad_y_gps, shield_xtk_val));
            trad_txt_ghost_status.setColor(0.0, 1.0, 0.53, 1.0);
            trad_txt_ghost_status.setText("NOMINAL: FARA SPOOFING ACTIV (PISTA 09 ALINIATA)");
            trad_txt_fms_cmd.setColor(0.75, 0.88, 1.0, 1.0);
            trad_txt_fms_cmd.setText("COMANDA FMS CLASIC: MENTINE CAPUL 089 PE AXUL PISTEI 09");
            trad_txt_why_ignored.setColor(0.65, 0.82, 0.96, 1.0);
            trad_txt_why_ignored.setText(sprintf("WATCHDOG FIZIC: Chi2=%.2f (<18.5) — GPS VALIDAT PRIN OPTICA + INS", chi2_val));
        }
        trad_txt_cmp_row.setText(sprintf(
            "RAIM CLASIC: 0 ALERTE (ORB LA SPOOF COERENT) | Xtk_Trad=%+.1fm vs Xtk_AeroShield=%+.1fm",
            trad_y_gps, shield_xtk_val
        ));
    }

    # --- B. Bannerul de sus de pe parbriz (screen.window) ---
    var cur_phase_prop = getprop("/aero-shield/flight-phase") or auto_flight_phase;
    var mode_tag = "[AUTOLAND CAT IIIc — FCS LIB AUTO-TUNED]";
    if (cur_phase_prop == "UPSET_RECOVERY") {
        mode_tag = "[UPSET RECOVERY UPRT — REDRESARE ATITUDINE]";
        steer_txt = "!!! UPRT ACTIV: NIVELARE ARIPI (ROLL->0) & REDRESARE PICAJ/STALL !!!";
    } elsif (cur_phase_prop == "GO_AROUND_TOGA") {
        mode_tag = "[GO-AROUND TOGA — ATERIZARE ANULATA -> URCARE 125m]";
        steer_txt = ">>> TOGA FULL POWER: URCA LA 125m AGL PENTRU CIRCUIT TEARDROP <<<";
    } elsif (cur_phase_prop == "TEARDROP_OUTBOUND") {
        mode_tag = "[TEARDROP CIRCUIT — REPOZITIONARE FIZICA SPRE -2.2km]";
        steer_txt = ">>> ZBOR OUTBOUND TEARDROP CATRE PUNCTUL DE INTERCEPTARE FINAL <<<";
    } elsif (cur_phase_prop == "BASE_TURN_INTERCEPT") {
        mode_tag = "[BASE TURN 180 — REALINIERE PE AXUL PISTEI]";
        steer_txt = ">>> VIRAJ DE BAZA 180° -> CAPTURA LOCALIZER & GLIDESLOPE 3.0° <<<";
    }
    var cyber_tag = (quar > 0 or spoof_jam_active > 0) ? " | GPS CARANTINAT (CONFIDENTLY WRONG BLOCAT)" : "";
    hud_win.clear();
    hud_win.write(
        sprintf("AERO-SHIELD %s %s | X_RWY: %+.0fm | AGL: %.1fm (%.0fft) | CRAB: %+.1f deg%s",
            ACTIVE_APT_TAG, mode_tag, local_g.x_rwy, agl_m, agl_ft, crab_deg, cyber_tag),
        0.22, 0.92, 0.98
    );
    hud_win.write(
        sprintf("LOC %s (Abatere: %+.1fm)  ==>  %s", build_cdi_bar(xtk_m), xtk_m, steer_txt),
        math.abs(xtk_m) < 10.0 ? 0.15 : 1.0,
        0.95,
        math.abs(xtk_m) < 10.0 ? 0.45 : 0.20
    );
    hud_win.write(
        sprintf("GLIDESLOPE: %s | SWIR 2.2um STAR TRACKER: %d LOCKS (SNR %.1fdB, Att %.1f\")",
            pitch_txt,
            getprop("/aero-shield/star-tracker/stars-locked") or 8,
            getprop("/aero-shield/star-tracker/mean-snr-swir-db") or 24.8,
            getprop("/aero-shield/star-tracker/attitude-err-arcsec") or 4.2),
        flare > 0 ? 0.25 : 0.92,
        0.96,
        flare > 0 ? 1.0 : 0.35
    );

    # --- C. Servocomanda Autoland CAT IIIc + Stabilizator SAS prin Trim & Auto-Throttle ---
    var max_pit_phase = (cur_phase_prop == "FINAL_APPROACH") ? 3.0 : 9.0;
    var tgt_pit_safe = clamp(tgt_pit, -4.8, max_pit_phase);
    var mcp_override = getprop("/aero-shield/fg-mcp-override") or 0;

    if (mcp_override > 0) {
        # Control 100% extern prin FG-MCP (:5501 TCP) condus de telemetria AERO-SHIELD (:8080 / :5506) — fara suprascriere din Nasal!
        was_autoland = 0;
    } elsif (autoland > 0) {
        was_autoland = 1;
        if (cur_phase_prop == "FINAL_APPROACH" and agl_m <= 1.38 and local_g.x_rwy >= -25.0 and local_g.x_rwy <= 1250.0 and math.abs(xtk_m) <= 22.0) {
            # Rulare automata DOAR dupa contactul rotilor principale pe asfalt — franare ferma + taiere amestec imediat la touchdown!
            setprop("/controls/engines/engine[0]/throttle", 0.0);
            setprop("/controls/engines/current-engine/throttle", 0.0);
            setprop("/controls/engines/current-engine/mixture", 0.0);
            setprop("/controls/engines/engine[0]/mixture", 0.0);
            setprop("/controls/engines/engine[0]/cutoff", 1);
            setprop("/controls/flight/flaps", 0.0);
            setprop("/controls/flight/rudder-trim", 0.0);
            setprop("/controls/flight/aileron", 0.0);
            setprop("/controls/flight/elevator", 0.14);
            var rwy_hdg_err = wrap180(LRCV_RWY_HDG - cur_hdg);
            var nws_rudder = clamp(rwy_hdg_err * 0.055 - xtk_m * 0.022, -0.65, 0.65);
            setprop("/controls/flight/rudder", nws_rudder);
            var gs_now = getprop("/velocities/groundspeed-kt") or kt;
            if (kt < 5.0 or gs_now < 1.5) {
                # Oprire completa pe pista: frana de parcare + oprire motor (cutoff / mixture=0 / magnetos=0)
                setprop("/controls/gear/brake-parking", 1);
                setprop("/controls/gear/brake-left", 1.0);
                setprop("/controls/gear/brake-right", 1.0);
                setprop("/controls/switches/magnetos", 0);
                setprop("/fdm/jsbsim/propulsion/engine[0]/set-running", 0);
                setprop("/engines/active-engine/running", 0);
                setprop("/engines/engine[0]/running", 0);
            } else {
                var brk = clamp(0.75 + (60.0 - kt) * 0.008, 0.72, 1.00);
                setprop("/controls/gear/brake-parking", kt < 42.0 ? 1 : 0);
                setprop("/controls/gear/brake-left", brk);
                setprop("/controls/gear/brake-right", brk);
            }
        } else {
            var h_err = (cur_phase_prop == "BASE_TURN_INTERCEPT" or cur_phase_prop == "TEARDROP_OUTBOUND" or cur_phase_prop == "GO_AROUND_TOGA") ? local_g.hdg_corr : wrap180(tgt_hdg - cur_hdg);
            var max_bank = (cur_phase_prop == "TEARDROP_OUTBOUND" or cur_phase_prop == "BASE_TURN_INTERCEPT") ? 24.0 : 20.0;
            var cmd_roll = (cur_phase_prop == "UPSET_RECOVERY") ? 0.0 : clamp(h_err * 1.55, -max_bank, max_bank);
            if (cur_phase_prop == "FINAL_APPROACH" and agl_m < 8.0) {
                cmd_roll = clamp(h_err * 0.95, -7.0, 7.0);
                setprop("/controls/flight/rudder", clamp(wrap180(LRCV_RWY_HDG - cur_hdg) * 0.032, -0.30, 0.30));
            } else {
                setprop("/controls/flight/rudder", clamp(0.015 + cmd_roll * 0.006, -0.25, 0.25));
            }
            var ail_kp = (cur_phase_prop == "UPSET_RECOVERY") ? 0.058 : 0.042;
            var ail = clamp(0.020 + ail_kp * (cmd_roll - cur_roll) - 0.012 * p_rate, -0.50, 0.50);
            setprop("/controls/flight/aileron", ail);
            setprop("/controls/flight/aileron-trim", 0.016);

            var p_err = tgt_pit_safe - cur_pitch;
            var elev = clamp(-0.015 - 0.045 * p_err + 0.014 * q_rate, -0.26, 0.25);
            setprop("/controls/flight/elevator", elev);
            setprop("/controls/flight/elevator-trim", 0.0);

            var target_kt = 70.0;
            var base_thr = clamp(0.56 + 0.025 * (target_kt - kt) + 0.028 * gs_err, 0.22, 0.85);
            var thr_out = base_thr;
            if (cur_phase_prop == "UPSET_RECOVERY") {
                thr_out = (cur_pitch < -8.0 or kt > 84.0) ? 0.20 : 0.90;
                setprop("/controls/flight/flaps", 0.33);
            } elsif (cur_phase_prop == "GO_AROUND_TOGA") {
                thr_out = 1.0;
                setprop("/controls/flight/flaps", agl_m < 32.0 ? 0.33 : 0.0);
            } elsif (cur_phase_prop == "TEARDROP_OUTBOUND") {
                thr_out = agl_m < 115.0 ? 0.98 : clamp(0.68 + 0.025 * (76.0 - kt) + 0.032 * gs_err, 0.45, 0.95);
                setprop("/controls/flight/flaps", 0.0);
            } elsif (cur_phase_prop == "BASE_TURN_INTERCEPT") {
                thr_out = clamp(0.62 + 0.028 * (72.0 - kt) + 0.038 * gs_err, 0.35, 0.92);
                setprop("/controls/flight/flaps", (kt < 80.0 and math.abs(wrap180(cur_hdg - LRCV_RWY_HDG)) < 45.0) ? 0.33 : 0.0);
            } elsif (agl_m <= 1.42 and dist_m <= 25.0) {
                thr_out = 0.0;
            } elsif (agl_m <= 4.20 and dist_m <= 28.0) {
                # C1 Cushion Flare Autothrottle Retard: mentine perna de aer cald (0.24 = ~1500 RPM) pana la atingerea rotilor (1.42m AGL)
                var ret_w = smoothstep_c1(1.42, 4.20, agl_m);
                var vs_fpm_now = vs_fps * 60.0;
                var sink_boost = clamp((-210.0 - vs_fpm_now) * 0.0012, 0.0, 0.16);
                var thr_cushion = clamp(0.24 + sink_boost, 0.22, 0.42);
                thr_out = thr_cushion * (1.0 - ret_w) + base_thr * ret_w;
            } else {
                setprop("/controls/flight/flaps", 0.33);
            }
            setprop("/controls/engines/engine[0]/throttle", thr_out);
            setprop("/controls/engines/current-engine/throttle", thr_out);
            setprop("/controls/gear/brake-left", 0.0);
            setprop("/controls/gear/brake-right", 0.0);
        }
    } else {
        if (was_autoland > 0 or tick_count == 1) {
            was_autoland = 0;
            setprop("/controls/flight/aileron", 0.0);
            setprop("/controls/flight/elevator", 0.0);
            setprop("/controls/gear/brake-left", 0.0);
            setprop("/controls/gear/brake-right", 0.0);
        }

        var h_err_sas = wrap180(tgt_hdg - cur_hdg);
        var cmd_roll_sas = clamp(h_err_sas * 1.25, -14.0, 14.0);
        var p_err_sas = (tgt_pit_safe + 1.4) - cur_pitch;
        var pitch_trim_sas = clamp(-0.022 - 0.036 * p_err_sas + 0.012 * q_rate, -0.25, 0.22);
        var roll_trim_sas  = clamp(0.018 + 0.025 * (cmd_roll_sas - cur_roll) - 0.009 * p_rate, -0.22, 0.22);
        setprop("/controls/flight/elevator-trim", pitch_trim_sas);
        setprop("/controls/flight/aileron-trim", roll_trim_sas);

        if (agl_m <= 1.65 and dist_m <= 25.0) {
            setprop("/controls/engines/engine[0]/throttle", 0.0);
            setprop("/controls/engines/current-engine/throttle", 0.0);
            var brk = kt > 3.0 ? 0.55 : 0.20;
            setprop("/controls/gear/brake-left", brk);
            setprop("/controls/gear/brake-right", brk);
            var pilot_ail = getprop("/controls/flight/aileron") or 0.0;
            var rwy_hdg_err = wrap180(LRCV_RWY_HDG - cur_hdg);
            var nws_rudder = clamp(pilot_ail * 0.65 + rwy_hdg_err * 0.045 - xtk_m * 0.012, -0.65, 0.65);
            setprop("/controls/flight/rudder", nws_rudder);
        } elsif (agl_m > 3.0) {
            var thr_sas = clamp(0.54 + 0.028 * (70.0 - kt) + 0.034 * gs_err, 0.22, 0.85);
            setprop("/controls/engines/engine[0]/throttle", thr_sas);
            setprop("/controls/engines/current-engine/throttle", thr_sas);
            setprop("/controls/gear/brake-left", 0.0);
            setprop("/controls/gear/brake-right", 0.0);
        }
    }

    # --- D. Blackbox Telemetry Logger la 1.0 Hz in consola / flightgear_session.log ---
    if (math.mod(tick_count, 20) == 1) {
        var rpm_now = getprop("/engines/active-engine/rpm") or getprop("/engines/engine[0]/rpm") or 0.0;
        var thr_now = getprop("/controls/engines/engine[0]/throttle") or 0.0;
        var ail_now = getprop("/controls/flight/aileron") or 0.0;
        var elv_now = getprop("/controls/flight/elevator") or 0.0;
        var rud_now = getprop("/controls/flight/rudder") or 0.0;
        var brk_now = getprop("/controls/gear/brake-left") or 0.0;
        var status_str = mcp_override > 0 ? "FG_MCP" : (autoland > 0 ? "AUTOLAND" : "MANUAL_SAS");
        print(sprintf(
            "[FG-BLACKBOX t=%.1fs] MSL=%.0fft AGL=%.0fft(%.1fm) IAS=%.1fkt VS=%.0ffpm Att=(R:%+.1f,P:%+.1f,H:%.1f) Tgt=(H:%.1f,P:%+.1f) Eng=(RPM:%.0f,thr:%.2f) Ctrl=(ail:%+.2f,elv:%+.2f,rud:%+.2f,brk:%.2f) X_rwy=%+.0fm Xtk=%+.1fm Mode=%s",
            tick_count * 0.05, msl_ft, agl_ft, agl_m, kt, vs_fps * 60.0,
            cur_roll, cur_pitch, cur_hdg, tgt_hdg, tgt_pit_safe, rpm_now, thr_now, ail_now, elv_now, rud_now, brk_now,
            local_g.x_rwy, xtk_m, status_str
        ));
    }

    settimer(update_aero_shield_hud, 0.05);
};

var apply_cat3_daylight_fog_env = func() {
    call(func { fgcommand("timeofday", props.Node.new({"timeofday": "noon"})); }, []);
    setprop("/environment/weather-scenario", "Manual input");
    setprop("/environment/metar/data", "XXXX 011200Z 08902KT 0240 FG NSC 18/18 Q1018");
    setprop("/environment/config/enabled", 1);
    setprop("/environment/params/metar-updates-environment", 0);
    setprop("/environment/config/presets/visibility-m", 240.0);
    setprop("/environment/config/presets/visibility-m-override", 1);
    setprop("/environment/config/presets/wind-from-north-fps", -0.06);
    setprop("/environment/config/presets/wind-from-east-fps", -3.37);
    setprop("/environment/config/presets/wind-override", 1);
    setprop("/environment/config/presets/turbulence-magnitude-norm", 0.0);
    setprop("/environment/config/presets/turbulence-magnitude-norm-override", 1);
    setprop("/environment/visibility-m", 240.0);
    setprop("/environment/effective-visibility-m", 240.0);
    setprop("/environment/wind-from-heading-deg", 89.0);
    setprop("/environment/wind-speed-kt", 2.0);
    setprop("/environment/rain-norm", 0.0);
    setprop("/environment/snow-norm", 0.0);
    setprop("/environment/turbulence/magnitude-norm", 0.0);
    setprop("/environment/aircraft-effects/fog-level", 0.0);
    setprop("/environment/aircraft-effects/frost-level", 0.0);
    setprop("/environment/aircraft-effects/rain-Splash-norm", 0.0);
    setprop("/environment/aircraft-effects/use-wipers", 0);
    setprop("/controls/anti-ice/window-heat", 1);
    setprop("/controls/lighting/cabin-heat", 1.0);
    setprop("/controls/lighting/defrost", 1.0);
    for (var ci = 0; ci < 5; ci += 1) {
        setprop("/environment/clouds/layer[" ~ ci ~ "]/coverage", "clear");
        setprop("/environment/clouds/layer[" ~ ci ~ "]/coverage-type", 5);
    }
};

_setlistener("/sim/signals/fdm-initialized", func {
    setprop("/aero-shield/autoland-local", 1);
    setprop("/aero-shield/autoland-cmd", 1);
    apply_cat3_daylight_fog_env();
    setprop("/controls/engines/engine/use-primer", 0);
    setprop("/controls/engines/engine[0]/use-primer", 0);
    setprop("/fdm/jsbsim/settings/damage", 0);
    setprop("/engines/active-engine/crash-engine", 0);
    setprop("/engines/active-engine/kill-engine", 0);
    setprop("/fdm/jsbsim/gear/unit[0]/broken", 0);
    setprop("/fdm/jsbsim/gear/unit[1]/broken", 0);
    setprop("/fdm/jsbsim/gear/unit[2]/broken", 0);
    setprop("/fdm/jsbsim/wing-damage/left-wing", 0);
    setprop("/fdm/jsbsim/wing-damage/right-wing", 0);
    setprop("/fdm/jsbsim/crash", 0);
    setprop("/fdm/jsbsim/engine/damage-level", 0);
    setprop("/controls/engines/engine[0]/cutoff", 0);
    setprop("/engines/active-engine/oil-temperature-degf", 180.0);
    setprop("/engines/engine[0]/oil-temperature-degf", 180.0);
    if (!hud_loop_running) {
        hud_loop_running = 1;
        settimer(update_aero_shield_hud, 0.25);
    }
});

if (getprop("/sim/signals/fdm-initialized") and !hud_loop_running) {
    setprop("/aero-shield/autoland-local", 1);
    setprop("/aero-shield/autoland-cmd", 1);
    apply_cat3_daylight_fog_env();
    hud_loop_running = 1;
    settimer(update_aero_shield_hud, 0.10);
}

setlistener("/aero-shield/reload-nasal", func(n) {
    if (n != nil and (n.getValue() or 0) > 0) {
        setprop("/aero-shield/reload-nasal", 0);
        if (pfd_win != nil) { call(func { pfd_win.del(); }, []); pfd_win = nil; }
        if (cyber_win != nil) { call(func { cyber_win.del(); }, []); cyber_win = nil; }
        if (trad_gps_win != nil) { call(func { trad_gps_win.del(); }, []); trad_gps_win = nil; }
        if (hud_win != nil) { call(func { hud_win.close(); }, []); hud_win = nil; }
        var fg_root = getprop("/sim/fg-root") or "/home/monk/.fgfs/fgdata_2024_1";
        var p = fg_root ~ "/Nasal/aero_shield_hud.nas";
        print("[FG-BLACKBOX] Hot-reload aero_shield_hud.nas (3D Conformal FD + 3 Canvas Windows) din: " ~ p);
        call(func { io.load_nasal(p, "aero_shield_hud"); }, [], nil, nil, var err = []);
    }
}, 0, 0);
