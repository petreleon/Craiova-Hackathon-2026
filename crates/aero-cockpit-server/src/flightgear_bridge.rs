//! Punți UDP bidirecționale în timp real ($60\text{ Hz}$ IN / $20\text{ Hz}$ OUT) către FlightGear:
//! - **Telemetrie FlightGear $\to$ AERO-SHIELD (`0.0.0.0:5505`)**:
//!   `--generic=socket,out,60,127.0.0.1,5505,udp,aero_shield`
//! - **Ghidaj & Autoland AERO-SHIELD $\to$ FlightGear (`127.0.0.1:5506`)**:
//!   `--generic=socket,in,20,127.0.0.1,5506,udp,aero_shield`
//!
//! Funcționează atât pe Aeroportul Internațional Craiova (`LRCV RWY 09`), cât și pe orice alt aeroport
//! din FlightGear (ancorând automat un prag de referință la direcția inițială a aeronavei dacă zborul nu este în Craiova).

use std::net::UdpSocket;
use std::sync::Arc;
use std::time::{Duration, Instant};

use arc_swap::ArcSwap;
use tracing::{info, warn};

use crate::rt_loop_60hz::{LRCV_RWY09_HEADING_DEG, LRCV_RWY09_LAT_DEG, LRCV_RWY09_LON_DEG};
use crate::sim_commands::sanitize_f64;

/// Pachet decodat din FlightGear la 60 Hz, deja proiectat în reperul pistei (`[x_track, y_cross, z_down]`).
#[derive(Debug, Clone)]
pub struct FlightGearLiveState {
    pub received_at: Instant,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub is_near_craiova_lrcv: bool,
    pub runway_heading_deg: f64,
    pub pos_ned_m: [f64; 3],
    pub airspeed_mps: f64,
    pub vel_track_ned_mps: [f64; 3],
    pub roll_deg: f64,
    pub pitch_deg: f64,
    pub rel_yaw_deg: f64,
    pub true_heading_deg: f64,
    pub omega_body_dps: [f64; 3],
    pub wind_ned_mps: [f64; 3],
    pub visibility_m: f64,
    pub gear_down: Option<bool>,
}

/// Pachet de ghidaj în buclă închisă trimis din `AERO-SHIELD` înapoi în arborele de proprietăți FlightGear (`UDP :5506`),
/// afișat pe parbrizul FlightGear prin `aero_shield_hud.nas` și utilizat opțional pentru CAT IIIc Autoland Assist.
#[derive(Debug, Clone, Copy)]
pub struct FlightGearGuidanceFeedback {
    pub distance_to_rwy_m: f64,
    pub cross_track_m: f64,
    pub alt_agl_m: f64,
    pub heading_correction_deg: f64,
    pub glideslope_error_deg: f64,
    pub crab_angle_deg: f64,
    pub target_heading_deg: f64,
    pub target_pitch_deg: f64,
    pub flare_active: bool,
    pub gnss_quarantined: bool,
    pub autoland_assist: bool,
}

/// Transmițător UDP non-blocking pentru canalul invers `AERO-SHIELD -> FlightGear` (`127.0.0.1:5506`).
pub struct FlightGearFeedbackSender {
    sock: Option<UdpSocket>,
    target_addr: String,
}

impl Default for FlightGearFeedbackSender {
    fn default() -> Self {
        Self::new()
    }
}

impl FlightGearFeedbackSender {
    #[must_use]
    pub fn new() -> Self {
        let target_addr = std::env::var("FG_IN_UDP_ADDR")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "127.0.0.1:5506".to_string());
        let sock = UdpSocket::bind("0.0.0.0:0").ok().map(|s| {
            let _ = s.set_nonblocking(true);
            s
        });
        Self { sock, target_addr }
    }

    /// Trimite linia CSV formatată conform blocului `<input>` din `flightgear/aero_shield.xml`.
    pub fn send_guidance(&self, fb: &FlightGearGuidanceFeedback) {
        let Some(ref sock) = self.sock else {
            return;
        };
        let csv_line = format!(
            "1,{:.1},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{},{},{}\n",
            sanitize_f64(fb.distance_to_rwy_m, 0.0, 50_000.0, 1000.0),
            sanitize_f64(fb.cross_track_m, -9999.0, 9999.0, 0.0),
            sanitize_f64(fb.alt_agl_m, 0.0, 12_000.0, 50.0),
            sanitize_f64(fb.heading_correction_deg, -90.0, 90.0, 0.0),
            sanitize_f64(fb.glideslope_error_deg, -45.0, 45.0, 0.0),
            sanitize_f64(fb.crab_angle_deg, -45.0, 45.0, 0.0),
            sanitize_f64(fb.target_heading_deg, 0.0, 360.0, LRCV_RWY09_HEADING_DEG),
            sanitize_f64(fb.target_pitch_deg, -18.0, 18.0, 0.0),
            i32::from(fb.flare_active),
            i32::from(fb.gnss_quarantined),
            i32::from(fb.autoland_assist),
        );
        let _ = sock.send_to(csv_line.as_bytes(), &self.target_addr);
    }
}

#[derive(Debug, Clone, Copy)]
struct VirtualRunwayAnchor {
    threshold_lat_deg: f64,
    threshold_lon_deg: f64,
    runway_heading_deg: f64,
}

#[inline]
fn wrap_deg_180(mut deg: f64) -> f64 {
    while deg > 180.0 {
        deg -= 360.0;
    }
    while deg < -180.0 {
        deg += 360.0;
    }
    deg
}

/// Pornește un thread dedicat care ascultă pachete UDP de la FlightGear pe `0.0.0.0:5505` (sau `FG_UDP_PORT`).
pub fn spawn_flightgear_udp_listener(fg_state: Arc<ArcSwap<Option<FlightGearLiveState>>>) {
    let port: u16 = std::env::var("FG_UDP_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(5505);

    let _ = std::thread::Builder::new()
        .name("aero-fgfs-udp-5505".into())
        .spawn(move || {
            let bind_addr = format!("0.0.0.0:{}", port);
            let sock = match UdpSocket::bind(&bind_addr) {
                Ok(s) => {
                    let _ = s.set_read_timeout(Some(Duration::from_millis(500)));
                    info!(
                        "FlightGear Live UDP Telemetry Bridge listening on udp://{} (60 Hz)",
                        bind_addr
                    );
                    s
                }
                Err(err) => {
                    warn!(
                        "Could not bind FlightGear UDP port {} ({}); internal simulator remains active.",
                        bind_addr, err
                    );
                    return;
                }
            };

            let mut buf = [0u8; 2048];
            let mut virtual_anchor: Option<VirtualRunwayAnchor> = None;
            let mut logged_first_packet = false;
            let mut pkt_counter: u64 = 0;

            let _ = std::fs::write(
                "flightgear_blackbox.csv",
                "pkt,lat_deg,lon_deg,x_ned_m,y_ned_m,agl_m,ias_mps,vs_down_mps,roll_deg,pitch_deg,hdg_deg,vis_m,gear_down,anomaly\n",
            );

            loop {
                match sock.recv_from(&mut buf) {
                    Ok((n, src)) if n > 0 => {
                        if let Ok(line) = std::str::from_utf8(&buf[..n]) {
                            if let Some(parsed) =
                                parse_fg_csv_line(line.trim(), &mut virtual_anchor)
                            {
                                pkt_counter = pkt_counter.saturating_add(1);
                                let agl_m = (-parsed.pos_ned_m[2]).max(0.0);
                                let vs_down_mps = parsed.vel_track_ned_mps[2];
                                let anomaly = if agl_m < 15.0 && parsed.pos_ned_m[0] < -200.0 {
                                    "WARN_TOO_LOW_BEFORE_RUNWAY"
                                } else if parsed.airspeed_mps < 22.0 && agl_m > 3.0 {
                                    "WARN_LOW_AIRSPEED_STALL_RISK"
                                } else if parsed.roll_deg.abs() > 32.0 {
                                    "WARN_EXCESSIVE_BANK"
                                } else if vs_down_mps > 6.5 {
                                    "WARN_HIGH_SINK_RATE"
                                } else {
                                    "NOMINAL"
                                };

                                if !logged_first_packet {
                                    info!(
                                        "[FG-BLACKBOX] Connected LIVE to FlightGear from {} (lat={:.4}, lon={:.4}, agl={:.1}m, ias={:.1}m/s, near_LRCV={})",
                                        src,
                                        parsed.lat_deg,
                                        parsed.lon_deg,
                                        agl_m,
                                        parsed.airspeed_mps,
                                        parsed.is_near_craiova_lrcv
                                    );
                                    logged_first_packet = true;
                                }

                                if pkt_counter % 30 == 1 {
                                    info!(
                                        "[FG-BLACKBOX #{pkt_counter}] pos=({:.0}m, {:+.1}m) AGL={:.1}m IAS={:.1}m/s VS_down={:+.1}m/s att=(R:{:+.1}°, P:{:+.1}°, H:{:.1}°) status={}",
                                        parsed.pos_ned_m[0],
                                        parsed.pos_ned_m[1],
                                        agl_m,
                                        parsed.airspeed_mps,
                                        vs_down_mps,
                                        parsed.roll_deg,
                                        parsed.pitch_deg,
                                        parsed.true_heading_deg,
                                        anomaly
                                    );
                                    if let Ok(mut f) = std::fs::OpenOptions::new()
                                        .create(true)
                                        .append(true)
                                        .open("flightgear_blackbox.csv")
                                    {
                                        use std::io::Write;
                                        let _ = writeln!(
                                            f,
                                            "{},{:.6},{:.6},{:.1},{:.1},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.1},{:?},{}",
                                            pkt_counter,
                                            parsed.lat_deg,
                                            parsed.lon_deg,
                                            parsed.pos_ned_m[0],
                                            parsed.pos_ned_m[1],
                                            agl_m,
                                            parsed.airspeed_mps,
                                            vs_down_mps,
                                            parsed.roll_deg,
                                            parsed.pitch_deg,
                                            parsed.true_heading_deg,
                                            parsed.visibility_m,
                                            parsed.gear_down,
                                            anomaly
                                        );
                                    }
                                }
                                fg_state.store(Arc::new(Some(parsed)));
                            }
                        }
                    }
                    _ => {
                        // Dacă FlightGear s-a închis (niciun pachet de > 2.5 secunde), revenim automat la simulatorul intern
                        let cur = fg_state.load();
                        if let Some(ref st) = **cur {
                            if st.received_at.elapsed() > Duration::from_millis(2500) {
                                fg_state.store(Arc::new(None));
                                virtual_anchor = None;
                                logged_first_packet = false;
                                info!("FlightGear UDP stream idle; switched back to internal 60Hz simulator.");
                            }
                        }
                    }
                }
            }
        });
}

fn parse_fg_csv_line(
    line: &str,
    virtual_anchor: &mut Option<VirtualRunwayAnchor>,
) -> Option<FlightGearLiveState> {
    let last_line = line.lines().last()?.trim();
    let cols: Vec<f64> = last_line
        .split(',')
        .map(|s| s.trim().parse::<f64>().unwrap_or(0.0))
        .collect();

    if cols.len() < 7 {
        return None;
    }

    let lat_deg = sanitize_f64(cols[0], -89.9, 89.9, LRCV_RWY09_LAT_DEG);
    let lon_deg = sanitize_f64(cols[1], -180.0, 180.0, LRCV_RWY09_LON_DEG);
    let alt_agl_m = sanitize_f64(cols[2], 0.0, 12_000.0, 50.0);
    let roll_deg = sanitize_f64(cols[3], -179.0, 179.0, 0.0);
    let pitch_deg = sanitize_f64(cols[4], -89.0, 89.0, 0.0);
    let heading_deg = sanitize_f64(cols[5], 0.0, 360.0, LRCV_RWY09_HEADING_DEG);
    let airspeed_mps = sanitize_f64(cols[6], 0.0, 350.0, 55.0);

    let vn_mps = if cols.len() > 7 {
        sanitize_f64(cols[7], -350.0, 350.0, 0.0)
    } else {
        airspeed_mps * heading_deg.to_radians().cos()
    };
    let ve_mps = if cols.len() > 8 {
        sanitize_f64(cols[8], -350.0, 350.0, 0.0)
    } else {
        airspeed_mps * heading_deg.to_radians().sin()
    };
    let vd_mps = if cols.len() > 9 {
        sanitize_f64(cols[9], -80.0, 80.0, 0.0)
    } else {
        0.0
    };

    let p_dps = if cols.len() > 10 {
        sanitize_f64(cols[10], -180.0, 180.0, 0.0)
    } else {
        0.0
    };
    let q_dps = if cols.len() > 11 {
        sanitize_f64(cols[11], -180.0, 180.0, 0.0)
    } else {
        0.0
    };
    let r_dps = if cols.len() > 12 {
        sanitize_f64(cols[12], -180.0, 180.0, 0.0)
    } else {
        0.0
    };

    let wind_from_deg = if cols.len() > 13 {
        sanitize_f64(cols[13], 0.0, 360.0, 175.0)
    } else {
        175.0
    };
    let wind_mps = if cols.len() > 14 {
        sanitize_f64(cols[14], 0.0, 60.0, 3.0)
    } else {
        3.0
    };
    let visibility_m = if cols.len() > 15 {
        sanitize_f64(cols[15], 25.0, 20_000.0, 250.0)
    } else {
        250.0
    };
    let gear_down = if cols.len() > 16 {
        Some(sanitize_f64(cols[16], 0.0, 1.0, 1.0) >= 0.5)
    } else {
        None
    };

    // Universal Real-Time Runway Threshold Discovery:
    // Citim mai întâi pragul pistei detectat în timp real de FlightGear prin `airportinfo()` (cols[17..20]).
    // Dacă `airportinfo()` din FlightGear a publicat coordonatele pragului (`/aero-shield/target-rwy/*`),
    // sistemul se ancorează universal pe ORICE pistă de pe glob!
    let dyn_rwy_lat = if cols.len() > 17 { cols[17] } else { 0.0 };
    let dyn_rwy_lon = if cols.len() > 18 { cols[18] } else { 0.0 };
    let dyn_rwy_hdg = if cols.len() > 19 { cols[19] } else { 0.0 };
    let dyn_rwy_elev_m = if cols.len() > 20 {
        sanitize_f64(cols[20], -100.0, 6000.0, 191.0)
    } else {
        191.0
    };
    let alt_msl_m = if cols.len() > 21 {
        sanitize_f64(cols[21], -100.0, 15_000.0, alt_agl_m + dyn_rwy_elev_m)
    } else {
        alt_agl_m + dyn_rwy_elev_m
    };

    const FG_LRCV_RWY09_THR_LAT_DEG: f64 = 44.317_944;
    const FG_LRCV_RWY09_THR_LON_DEG: f64 = 23.872_944;
    const FG_LRCV_RWY09_TRUE_HDG_DEG: f64 = 88.931_4;
    let d_north_lrcv = (lat_deg - FG_LRCV_RWY09_THR_LAT_DEG) * 111_132.95;
    let d_east_lrcv =
        (lon_deg - FG_LRCV_RWY09_THR_LON_DEG) * 111_319.49 * FG_LRCV_RWY09_THR_LAT_DEG.to_radians().cos();
    let dist_lrcv = (d_north_lrcv * d_north_lrcv + d_east_lrcv * d_east_lrcv).sqrt();
    let is_near_craiova = dist_lrcv < 25_000.0;

    // Catalog universal rapid pentru aeroporturile de referință (inclusiv coastă/mare: Catania LICC, Keflavik BIKF, Honolulu PHNL)
    // folosit instantaneu chiar și înainte ca primul cadru `airportinfo()` din Nasal să populeze cols[17..20]:
    const UNIVERSAL_AIRPORTS: &[(f64, f64, f64, f64)] = &[
        (44.317_944, 23.872_944, 88.931_4, 191.0),   // LRCV RWY 09 (Craiova)
        (37.467_933, 15.080_108, 263.970_5, 11.89),  // LICC RWY 26 (Catania-Fontanarossa — Ionian Sea Approach)
        (63.985_000, -22.605_556, 108.000_0, 50.0),  // BIKF (Keflavik — North Atlantic)
        (21.318_681, -157.922_428, 70.000_0, 4.0),   // PHNL (Honolulu — Pacific Ocean)
    ];

    let (ref_lat, ref_lon, rwy_hdg_deg, rwy_elev_m) = if dyn_rwy_lat.abs() > 0.01 && dyn_rwy_lon.abs() > 0.01 {
        (
            sanitize_f64(dyn_rwy_lat, -89.9, 89.9, FG_LRCV_RWY09_THR_LAT_DEG),
            sanitize_f64(dyn_rwy_lon, -180.0, 180.0, FG_LRCV_RWY09_THR_LON_DEG),
            sanitize_f64(dyn_rwy_hdg, 0.0, 360.0, FG_LRCV_RWY09_TRUE_HDG_DEG),
            dyn_rwy_elev_m,
        )
    } else if is_near_craiova {
        (FG_LRCV_RWY09_THR_LAT_DEG, FG_LRCV_RWY09_THR_LON_DEG, FG_LRCV_RWY09_TRUE_HDG_DEG, 191.0)
    } else if let Some(&(ulat, ulon, uhdg, uelev)) = UNIVERSAL_AIRPORTS.iter().find(|&&(alat, alon, _, _)| {
        let dn = (lat_deg - alat) * 111_132.95;
        let de = (lon_deg - alon) * 111_319.49 * alat.to_radians().cos();
        dn.hypot(de) < 30_000.0
    }) {
        (ulat, ulon, uhdg, uelev)
    } else {
        let anchor = virtual_anchor.get_or_insert_with(|| {
            let hdg_rad = heading_deg.to_radians();
            let ahead_north_m = 1600.0 * hdg_rad.cos();
            let ahead_east_m = 1600.0 * hdg_rad.sin();
            let cos_lat = lat_deg.to_radians().cos().abs().max(0.1);
            VirtualRunwayAnchor {
                threshold_lat_deg: lat_deg + ahead_north_m / 111_132.0,
                threshold_lon_deg: lon_deg + ahead_east_m / (111_320.0 * cos_lat),
                runway_heading_deg: heading_deg,
            }
        });
        (
            anchor.threshold_lat_deg,
            anchor.threshold_lon_deg,
            anchor.runway_heading_deg,
            0.0,
        )
    };

    let cos_ref = ref_lat.to_radians().cos().abs().max(0.1);
    let dn = (lat_deg - ref_lat) * 111_132.95;
    let de = (lon_deg - ref_lon) * 111_319.49 * cos_ref;

    let psi_rwy = rwy_hdg_deg.to_radians();
    let cos_psi = psi_rwy.cos();
    let sin_psi = psi_rwy.sin();

    // Proiecție pe axa pistei (+X = de-a lungul pistei, +Y = abatere dreapta față de axul pistei)
    let x_track = sanitize_f64(dn * cos_psi + de * sin_psi, -15_000.0, 5_000.0, -900.0);
    let y_cross = sanitize_f64(-dn * sin_psi + de * cos_psi, -5_000.0, 5_000.0, 0.0);

    // Coastline Cliff Baro-Inertial Step Compensator:
    // Când avionul vine deasupra mării (0m MSL) către o pistă de coastă (rwy_elev_m < 35m),
    // raportăm altitudinea la planul pragului pistei înainte de țărm și o îmbinăm C1 cu AGL-ul radar deasupra pragului.
    let effective_agl_m = if rwy_elev_m < 35.0 && x_track < -20.0 && cols.len() > 21 {
        let rwy_plane_agl = (alt_msl_m - rwy_elev_m).max(0.5);
        let t = ((x_track - (-140.0)) / 120.0).clamp(0.0, 1.0);
        let radar_blend = t * t * (3.0 - 2.0 * t);
        rwy_plane_agl * (1.0 - radar_blend) + alt_agl_m * radar_blend
    } else {
        alt_agl_m
    };
    let z_down = -effective_agl_m;

    let vx_track = sanitize_f64(vn_mps * cos_psi + ve_mps * sin_psi, -250.0, 250.0, airspeed_mps);
    let vy_cross = sanitize_f64(-vn_mps * sin_psi + ve_mps * cos_psi, -150.0, 150.0, 0.0);

    // Vântul în coordonatele pistei (wind_from_deg indică de unde bate -> inversăm cu +180°)
    let wind_to_rad = (wind_from_deg + 180.0).to_radians();
    let wn = wind_mps * wind_to_rad.cos();
    let we = wind_mps * wind_to_rad.sin();
    let wx_track = sanitize_f64(wn * cos_psi + we * sin_psi, -45.0, 45.0, -2.0);
    let wy_cross = sanitize_f64(-wn * sin_psi + we * cos_psi, -45.0, 45.0, 1.5);

    let rel_yaw_deg = sanitize_f64(wrap_deg_180(heading_deg - rwy_hdg_deg), -180.0, 180.0, 0.0);

    Some(FlightGearLiveState {
        received_at: Instant::now(),
        lat_deg,
        lon_deg,
        is_near_craiova_lrcv: is_near_craiova,
        runway_heading_deg: rwy_hdg_deg,
        pos_ned_m: [x_track, y_cross, z_down],
        airspeed_mps: airspeed_mps.max(15.0),
        vel_track_ned_mps: [vx_track, vy_cross, vd_mps],
        roll_deg,
        pitch_deg,
        rel_yaw_deg,
        true_heading_deg: heading_deg,
        omega_body_dps: [p_dps, q_dps, r_dps],
        wind_ned_mps: [wx_track, wy_cross, 0.0],
        visibility_m,
        gear_down,
    })
}
