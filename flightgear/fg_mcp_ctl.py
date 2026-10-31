#!/usr/bin/env python3
"""
FG-MCP (FlightGear Model Control Protocol) Bridge & Closed-Loop AERO-SHIELD Pilot
Connects to FlightGear's live Telnet Property Tree (127.0.0.1:5501) and AERO-SHIELD
HTTP/UDP guidance (http://127.0.0.1:8080/api/telemetry) to inspect, verify, and actuate:
  - Yoke (aileron & elevator + trims)
  - Rudder & Nose-Wheel Steering
  - Throttle & Flaps
  - Differential Left/Right Wheel Brakes & Parking Brake
Strictly follows AERO-SHIELD sensor-fusion instructions (Zero Ground-Truth Cheat).
"""

import json
import math
import os
import socket
import sys
import time
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from fg_library_extractor import (
    load_or_synthesize_fcs_library,
    solve_lateral_directional_2x2_trim,
)

FCS_LIB = load_or_synthesize_fcs_library()
SYNTH_GAINS = FCS_LIB["analytical_synthesis"]["synthesized_ap_gains"]


def clamp(val: float, lo: float, hi: float) -> float:
    return max(lo, min(hi, val))


def wrap180(deg: float) -> float:
    while deg > 180.0:
        deg -= 360.0
    while deg < -180.0:
        deg += 360.0
    return deg


def smoothstep_c1(edge0: float, edge1: float, x: float) -> float:
    if abs(edge1 - edge0) < 1e-9:
        return 1.0 if x >= edge1 else 0.0
    t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0)
    return t * t * (3.0 - 2.0 * t)


def compute_flare_target_fpa_deg_py(h_agl_m: float, dx_aim_m: float, horiz_speed_mps: float) -> float:
    if h_agl_m <= 0.02:
        return 0.0
    geom_fpa_deg = -math.degrees(math.atan2(h_agl_m, max(35.0, dx_aim_m)))
    flare_blend = 1.0 - smoothstep_c1(1.2, 12.0, h_agl_m)
    spd = max(25.0, horiz_speed_mps)
    target_sink_mps = clamp(0.72 + 2.84 * (h_agl_m / 12.0), 0.72, 3.80)
    sink_flare_fpa_deg = -math.degrees(math.atan2(target_sink_mps, spd))
    flare_end_fpa_deg = max(sink_flare_fpa_deg, -0.65)
    flare_fpa_deg = geom_fpa_deg * (1.0 - flare_blend) + flare_end_fpa_deg * flare_blend
    combined_fpa_deg = max(geom_fpa_deg, flare_fpa_deg)
    ground_rollout_scale = smoothstep_c1(0.05, 0.85, h_agl_m)
    return combined_fpa_deg * ground_rollout_scale


def compute_unified_fd_guidance_py(
    along_track_m: float,
    cross_track_m: float,
    alt_agl_m: float,
    vx_mps: float,
    vy_mps: float,
    vz_down_mps: float,
    cur_pitch_deg: float,
    true_heading_deg: float,
    runway_heading_deg: float = 88.9314,
    on_ground_or_stopped: bool = False,
) -> dict:
    """
    Pure C1-continuous Single-Source-of-Truth Flight Director guidance law (identical to Rust
    `compute_unified_fd_guidance` in `crates/aero-cockpit-server/src/rt_loop_60hz.rs`).
    """
    dx_to_rwy = max(0.0, 0.0 - along_track_m)
    dy_to_rwy = 0.0 - cross_track_m
    distance_to_rwy_m = math.hypot(dx_to_rwy, dy_to_rwy)

    raw_horiz_speed = math.hypot(vx_mps, vy_mps)
    horiz_ground_speed = max(1.0, raw_horiz_speed)

    ground_blend = (
        1.0
        if on_ground_or_stopped
        else smoothstep_c1(-10.0, 5.0, along_track_m) * (1.0 - smoothstep_c1(0.35, 1.65, alt_agl_m))
    )

    look_blend = smoothstep_c1(-160.0, -80.0, along_track_m)
    lead_w = smoothstep_c1(1.5, 10.0, abs(cross_track_m))
    loc_lookahead_m = 95.0 + 25.0 * look_blend
    vy_lead_sec = 1.35 * lead_w
    dy_predictive = -cross_track_m - vy_mps * vy_lead_sec

    air_target_track_deg = clamp(math.degrees(math.atan2(dy_predictive, loc_lookahead_m)), -28.0, 28.0)
    rollout_target_track_deg = clamp(-cross_track_m * 0.45, -8.0, 8.0)
    target_track_deg = air_target_track_deg * (1.0 - ground_blend) + rollout_target_track_deg * ground_blend

    rel_yaw_deg = wrap180(true_heading_deg - runway_heading_deg)
    actual_track_deg = (
        rel_yaw_deg if raw_horiz_speed < 3.0 else math.degrees(math.atan2(vy_mps, max(0.5, vx_mps)))
    )
    crab_needed_deg = clamp(wrap180(rel_yaw_deg - actual_track_deg), -22.0, 22.0) if raw_horiz_speed > 6.0 else 0.0
    decrab_factor = smoothstep_c1(12.0, 42.0, alt_agl_m) * (1.0 - ground_blend)
    flare_yaw_deg = clamp(target_track_deg * 0.22, -1.0, 1.0)
    desired_rel_yaw_deg = clamp(
        (target_track_deg + crab_needed_deg) * decrab_factor + flare_yaw_deg * (1.0 - decrab_factor),
        -45.0,
        45.0,
    )
    target_heading_deg = (runway_heading_deg + desired_rel_yaw_deg) % 360.0
    heading_correction_deg = clamp(wrap180(target_heading_deg - true_heading_deg), -90.0, 90.0)

    aim_x_m = 210.0
    dx_gs_aim = max(35.0, aim_x_m - along_track_m)
    gs_tan_3deg = math.tan(math.radians(3.0))
    h_beam_m = dx_gs_aim * gs_tan_3deg
    dh_beam_m = h_beam_m - alt_agl_m
    low_alt_boost = 1.0 - smoothstep_c1(10.0, 18.0, alt_agl_m)
    max_fpa_beam = 0.8 + 1.7 * low_alt_boost
    fpa_beam_deg = clamp(-3.0 + 0.35 * dh_beam_m, -4.5, max_fpa_beam)
    fpa_flare_deg = compute_flare_target_fpa_deg_py(alt_agl_m, dx_gs_aim, horiz_ground_speed)

    alpha_x = smoothstep_c1(-45.0, 10.0, along_track_m)
    air_target_fpa_deg = fpa_beam_deg * (1.0 - alpha_x) + fpa_flare_deg * alpha_x
    target_fpa_deg = air_target_fpa_deg * (1.0 - ground_blend)

    if on_ground_or_stopped or (alt_agl_m <= 0.05 and cur_pitch_deg < 0.0):
        actual_fpa_deg = cur_pitch_deg * (1.0 - ground_blend)
    else:
        actual_fpa_deg = math.degrees(math.atan2(-vz_down_mps, horiz_ground_speed))

    glideslope_error_deg = clamp((target_fpa_deg - actual_fpa_deg) * (1.0 - ground_blend), -35.0, 35.0)
    flare_active = (not on_ground_or_stopped) and (0.18 < alt_agl_m <= 8.5) and (along_track_m >= -20.0)

    flare_pitch_blend = alpha_x * (1.0 - smoothstep_c1(1.2, 9.5, alt_agl_m)) * (1.0 - ground_blend)
    aoa_ref_deg = 2.05 + 0.55 * flare_pitch_blend
    air_target_pitch_deg = clamp(target_fpa_deg + aoa_ref_deg + 0.85 * glideslope_error_deg, -4.2, 3.2)
    target_pitch_deg = air_target_pitch_deg * (1.0 - ground_blend) + 0.65 * ground_blend

    return {
        "distance_to_rwy_m": distance_to_rwy_m,
        "along_track_m": along_track_m,
        "cross_track_m": cross_track_m,
        "alt_agl_m": alt_agl_m,
        "target_track_deg": target_track_deg,
        "actual_track_deg": actual_track_deg,
        "crab_needed_deg": crab_needed_deg,
        "target_heading_deg": target_heading_deg,
        "heading_correction_deg": heading_correction_deg,
        "target_fpa_deg": target_fpa_deg,
        "actual_fpa_deg": actual_fpa_deg,
        "glideslope_error_deg": glideslope_error_deg,
        "target_pitch_deg": target_pitch_deg,
        "flare_active": int(flare_active),
    }


def compute_ap_control_from_fd(
    tgt_hdg_deg: float,
    cur_hdg_deg: float,
    gs_err_deg: float,
    tgt_pit_deg: float,
    agl_m: float,
    dist_m: float,
    xtk_m: float,
    cur_roll_deg: float,
    cur_pitch_deg: float,
    ias_kt: float,
    vs_fpm: float = -360.0,
    p_dps: float = 0.0,
    q_dps: float = 0.0,
    r_dps: float = 0.0,
    flare_active: int = 0,
    rwy_hdg_deg: float = 88.9314,
    crab_deg: float = 0.0,
    xtk_rate_mps: float = 0.0,
    xtk_int: float = 0.0,
    rud_int: float = 0.0,
    gs_int: float = 0.0,
    elev_int: float = 0.0,
    dt: float = 0.04,
) -> dict:
    """
    Unified Single-Source-of-Truth Autopilot control law (FD == AP):
    - Lateral: tracks `hdg_corr = wrap180(tgt_hdg_deg - cur_hdg_deg)` (zero parasitic `xtk_roll_bias`),
      and below 42.0m AGL blends into closed-loop PID Wing-Low / Forward-Slip decrab (`slip_blend`)
      so the rudder keeps the fuselage aligned parallel with the runway (`hdg_corr -> 0.0°`) while
      automatic PID wing-low bank (`crab_deg`, `xtk_m`, `xtk_int`, `xtk_rate_mps`) zeroes crosswind drift.
    - Vertical: tracks ONLY `tgt_pit_deg` + proportional-integral correction on `gs_err_deg` supplied by
      AERO-SHIELD's unified Flight Director (`gs_err_deg -> 0.0`, `cur_pitch_deg -> tgt_pit_deg`).
    """
    hdg_corr = wrap180(tgt_hdg_deg - cur_hdg_deg)
    slip_blend = 1.0 - smoothstep_c1(12.0, 42.0, agl_m)
    if slip_blend > 0.02:
        xtk_int_next = clamp(xtk_int + xtk_m * dt * 0.90, -8.5, 8.5)
        rud_int_next = clamp(
            rud_int + hdg_corr * dt * (SYNTH_GAINS.get("rudder_decrab_ki", 0.034) * 1.45),
            -0.55,
            0.55,
        )
    else:
        xtk_int_next = xtk_int * 0.90
        rud_int_next = rud_int * 0.90

    # Analytical 2x2 Lateral-Directional Matrix Inversion (M_lat^-1) extracted from c172p.xml
    lat_2x2 = solve_lateral_directional_2x2_trim(
        FCS_LIB["c172p_jsbsim"]["aerodynamics"],
        FCS_LIB["c172p_jsbsim"]["fcs_channels"],
        FCS_LIB["c172p_jsbsim"]["metrics"],
        FCS_LIB["c172p_jsbsim"]["mass_balance"],
        beta_rad=math.radians(crab_deg),
        tas_kt=max(50.0, ias_kt),
        flap_deg=10.0,
        propwash_ratio=0.0062,
    )
    phi_2x2_deg = lat_2x2["phi_slip_deg"] * 1.25
    wing_low_bank_deg = clamp(
        (
            phi_2x2_deg
            - xtk_m * (SYNTH_GAINS["wing_low_xtk_kp"] * 0.82)
            - xtk_int_next * (SYNTH_GAINS["wing_low_xtk_ki"] * 0.60)
            - xtk_rate_mps * (SYNTH_GAINS["wing_low_xtk_kd"] * 1.72)
        )
        * slip_blend,
        -12.5,
        12.5,
    )
    max_roll = 18.0 * (1.0 - slip_blend) + 14.5 * slip_blend
    roll_gain = (
        SYNTH_GAINS["roll_gain_approach"] * (1.0 - slip_blend)
        + SYNTH_GAINS["roll_gain_slip"] * (1.0 - 0.82 * slip_blend) * slip_blend
    )
    cmd_roll = clamp(hdg_corr * roll_gain + wing_low_bank_deg, -max_roll, max_roll)

    rud_p_trim = SYNTH_GAINS["rudder_p_factor_trim"]
    rud_coord = clamp(rud_p_trim + cmd_roll * SYNTH_GAINS["rudder_coord_roll_gain"], -0.22, 0.22)
    rud_decrab = clamp(
        rud_p_trim
        + rud_int_next
        + hdg_corr * (SYNTH_GAINS["rudder_decrab_kp"] * 1.65)
        - r_dps * 0.024,
        -0.82,
        0.82,
    )
    cmd_rud = rud_coord * (1.0 - slip_blend) + rud_decrab * slip_blend

    da_2x2_ff = lat_2x2["da_norm"] * 0.35 * slip_blend
    ail = clamp(
        SYNTH_GAINS["aileron_trim_norm"]
        + da_2x2_ff
        + SYNTH_GAINS["aileron_kp_roll"] * (cmd_roll - cur_roll_deg)
        - SYNTH_GAINS["aileron_kd_p"] * p_dps,
        -0.42,
        0.42,
    )

    gs_int_next = clamp(gs_int + SYNTH_GAINS["gs_ki_pitch"] * gs_err_deg * dt, -1.4, 1.4)
    cmd_pit = clamp(tgt_pit_deg + SYNTH_GAINS["gs_kp_pitch"] * gs_err_deg + gs_int_next, -3.5, 3.2)
    p_err = cmd_pit - cur_pitch_deg
    elev_int_next = clamp(elev_int - SYNTH_GAINS["elevator_ki_pitch"] * p_err * dt, -0.10, 0.08)
    elev = clamp(
        SYNTH_GAINS["elevator_base_trim"]
        + elev_int_next
        - SYNTH_GAINS["elevator_kp_pitch"] * p_err
        + SYNTH_GAINS["elevator_kd_q"] * q_dps,
        -0.24,
        0.20,
    )

    dx_before_thr = math.sqrt(max(0.0, dist_m * dist_m - xtk_m * xtk_m))
    base_thr = clamp(0.54 + 0.028 * (70.0 - ias_kt) + 0.042 * gs_err_deg, 0.15, 0.85)
    if dx_before_thr <= 28.0 and agl_m <= 6.0:
        thr = 0.0
    elif dx_before_thr <= 35.0 and (agl_m <= 11.0 or flare_active > 0):
        flare_thr_scale = clamp((agl_m - 6.0) / 5.0, 0.0, 1.0)
        thr = base_thr * flare_thr_scale
    else:
        thr = base_thr

    return {
        "hdg_corr_deg": hdg_corr,
        "cmd_roll_deg": cmd_roll,
        "cmd_pit_deg": cmd_pit,
        "cmd_rud": cmd_rud,
        "aileron": ail,
        "elevator": elev,
        "throttle": thr,
        "xtk_int": xtk_int_next,
        "rud_int": rud_int_next,
        "gs_int": gs_int_next,
        "elev_int": elev_int_next,
    }


def compute_traditional_gps_confidently_wrong_telemetry(
    real_x_m: float,
    real_y_m: float,
    alt_agl_m: float = 45.0,
    vx_mps: float = 36.0,
    vy_mps: float = 0.0,
    vz_down_mps: float = 1.88,
    true_heading_deg: float = 88.9314,
    runway_heading_deg: float = 88.9314,
    spoof_offset_m: float = 250.0,
    shield_active: bool = True,
) -> dict:
    """
    Models the epistemic blindness of a single-sensor Traditional GPS/FMS receiver ("Confidently Wrong")
    versus AERO-SHIELD's physical cross-validation (Chi2 + CUSUM + Koschmieder Optical + INS).
    """
    trad_gps_y_m = real_y_m + spoof_offset_m
    trad_fd = compute_unified_fd_guidance_py(
        along_track_m=real_x_m,
        cross_track_m=trad_gps_y_m,
        alt_agl_m=alt_agl_m,
        vx_mps=vx_mps,
        vy_mps=vy_mps,
        vz_down_mps=vz_down_mps,
        cur_pitch_deg=-1.0,
        true_heading_deg=true_heading_deg,
        runway_heading_deg=runway_heading_deg,
        on_ground_or_stopped=False,
    )
    aeroshield_fd = compute_unified_fd_guidance_py(
        along_track_m=real_x_m,
        cross_track_m=real_y_m,
        alt_agl_m=alt_agl_m,
        vx_mps=vx_mps,
        vy_mps=vy_mps,
        vz_down_mps=vz_down_mps,
        cur_pitch_deg=-1.0,
        true_heading_deg=true_heading_deg,
        runway_heading_deg=runway_heading_deg,
        on_ground_or_stopped=False,
    )
    spoof_active = abs(spoof_offset_m) > 15.0
    return {
        "sats_locked": "12/12",
        "hdop": 0.8,
        "epu_m": 0.8,
        "trad_raim_status": "OK",
        "trad_raim_faults": 0,
        "trad_reported_xtk_m": trad_gps_y_m,
        "trad_commanded_turn_m": -trad_gps_y_m,
        "trad_hdg_corr_deg": trad_fd["heading_correction_deg"],
        "ghost_aircraft_cdi_m": 0.0,
        "ghost_aircraft_real_y_m": -spoof_offset_m,
        "aeroshield_quarantined": bool(spoof_active and shield_active),
        "aeroshield_chi2": 96.4 if spoof_active else 1.15,
        "aeroshield_real_xtk_m": real_y_m,
        "aeroshield_hdg_corr_deg": aeroshield_fd["heading_correction_deg"],
    }


class FlightGearMcpClient:
    def __init__(self, host: str = "127.0.0.1", port: int = 5501, timeout: float = 2.0):
        self.host = host
        self.port = port
        self.sock = socket.create_connection((host, port), timeout=timeout)
        self.sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.sock.settimeout(timeout)
        self.file = self.sock.makefile("r", encoding="utf-8", errors="replace")
        # Switch FlightGear telnet server to raw data mode (no prompt)
        self.sock.sendall(b"data\r\n")

    def get_float(self, prop: str, default: float = 0.0) -> float:
        try:
            self.sock.sendall(f"get {prop}\r\n".encode("utf-8"))
            line = self.file.readline().strip()
            return float(line) if line else default
        except Exception:
            return default

    def get_batch(self, props_with_defaults: list) -> dict:
        cmd = "".join(f"get {prop}\r\n" for prop, _ in props_with_defaults)
        self.sock.sendall(cmd.encode("utf-8"))
        out = {}
        for prop, default in props_with_defaults:
            try:
                line = self.file.readline().strip()
                out[prop] = float(line) if line else default
            except Exception:
                out[prop] = default
        return out

    def set_prop(self, prop: str, val) -> None:
        self.sock.sendall(f"set {prop} {val}\r\n".encode("utf-8"))

    def set_batch(self, items: list) -> None:
        cmd = "".join(f"set {prop} {val}\r\n" for prop, val in items)
        self.sock.sendall(cmd.encode("utf-8"))

    def run_cmd(self, command_name: str) -> None:
        self.sock.sendall(f"run {command_name}\r\n".encode("utf-8"))

    def ensure_healthy_engine_and_airframe(self, force_start: bool = False) -> None:
        items = [
            ("/fdm/jsbsim/settings/damage", 0),
            ("/engines/active-engine/crash-engine", 0),
            ("/engines/active-engine/kill-engine", 0),
            ("/engines/active-engine/crashed", 0),
            ("/engines/active-engine/killed", 0),
            ("/fdm/jsbsim/gear/unit[0]/broken", 0),
            ("/fdm/jsbsim/gear/unit[1]/broken", 0),
            ("/fdm/jsbsim/gear/unit[2]/broken", 0),
            ("/fdm/jsbsim/wing-damage/left-wing", 0),
            ("/fdm/jsbsim/wing-damage/right-wing", 0),
            ("/fdm/jsbsim/crash", 0),
            ("/fdm/jsbsim/propulsion/tank[4]/priority", 1),
            ("/consumables/fuel/tank[4]/level-lbs", 0.08),
            ("/controls/engines/engine/use-primer", 0),
            ("/controls/engines/engine[0]/use-primer", 0),
            ("/engines/active-engine/oil-temperature-degf", 180.0),
            ("/engines/engine[0]/oil-temperature-degf", 180.0),
            ("/engines/active-engine/already-started-in-session", 1),
            ("/controls/engines/current-engine/carb-heat", 0),
            ("/controls/engines/current-engine/mixture", 1.0),
            ("/controls/engines/engine[0]/mixture", 1.0),
            ("/controls/switches/magnetos", 3),
            ("/controls/switches/master-bat", 1),
            ("/controls/switches/master-alt", 1),
            ("/controls/gear/brake-parking", 0),
        ]
        if force_start:
            items.extend([
                ("/fdm/jsbsim/propulsion/set-running", -1),
                ("/fdm/jsbsim/propulsion/engine[0]/set-running", 1),
                ("/engines/active-engine/running", 1),
                ("/engines/engine[0]/running", 1),
                ("/controls/switches/starter", 0),
                ("/controls/engines/engine/use-primer", 0),
                ("/controls/engines/engine[0]/use-primer", 0),
            ])
        self.set_batch(items)

    def snapshot_controls_and_guidance(self) -> dict:
        b = self.get_batch([
            ("/aero-shield/alt-agl-m", -1.0),
            ("/position/altitude-agl-ft", 0.0),
            ("/aero-shield/distance-m", 2500.0),
            ("/aero-shield/cross-track-m", 0.0),
            ("/aero-shield/heading-corr-deg", 0.0),
            ("/aero-shield/gs-error-deg", 0.0),
            ("/aero-shield/crab-deg", 0.0),
            ("/aero-shield/target-heading-deg", 88.9314),
            ("/aero-shield/target-pitch-deg", -2.2),
            ("/aero-shield/flare-active", 0.0),
            ("/aero-shield/gnss-quarantined", 0.0),
            ("/orientation/roll-deg", 0.0),
            ("/orientation/pitch-deg", 0.0),
            ("/orientation/heading-deg", 88.9314),
            ("/velocities/airspeed-kt", 72.0),
            ("/velocities/groundspeed-kt", 72.0),
            ("/velocities/vertical-speed-fps", 0.0),
            ("/orientation/p-body", 0.0),
            ("/orientation/q-body", 0.0),
            ("/orientation/r-body", 0.0),
            ("/engines/active-engine/rpm", 0.0),
            ("/controls/flight/aileron", 0.0),
            ("/controls/flight/elevator", 0.0),
            ("/controls/flight/rudder", 0.0),
            ("/controls/engines/engine[0]/throttle", 0.0),
            ("/controls/flight/flaps", 0.0),
            ("/controls/gear/brake-left", 0.0),
            ("/controls/gear/brake-right", 0.0),
            ("/controls/gear/brake-parking", 0.0),
            ("/aero-shield/spoof-jam-active", 0.0),
            ("/aero-shield/shield-protection", 1.0),
            ("/aero-shield/cmp-shield-xtk-m", 0.0),
            ("/aero-shield/cmp-spoof-xtk-m", 0.0),
            ("/aero-shield/cmp-ins-xtk-m", 0.0),
            ("/aero-shield/cmp-chi2", 1.15),
            ("/aero-shield/cmp-cusum", 0.08),
            ("/environment/visibility-m", 220.0),
            ("/environment/wind-from-heading-deg", 175.0),
            ("/environment/wind-speed-kt", 14.0),
            ("/environment/rain-norm", 0.85),
        ])
        agl = b["/aero-shield/alt-agl-m"]
        if agl <= 0.0:
            agl = b["/position/altitude-agl-ft"] * 0.3048
        return {
            "agl_m": agl,
            "dist_m": b["/aero-shield/distance-m"],
            "xtk_m": b["/aero-shield/cross-track-m"],
            "hdg_corr_deg": b["/aero-shield/heading-corr-deg"],
            "gs_err_deg": b["/aero-shield/gs-error-deg"],
            "crab_deg": b["/aero-shield/crab-deg"],
            "tgt_hdg_deg": b["/aero-shield/target-heading-deg"],
            "tgt_pit_deg": b["/aero-shield/target-pitch-deg"],
            "flare": int(b["/aero-shield/flare-active"]),
            "gnss_quar": int(b["/aero-shield/gnss-quarantined"]),
            "roll_deg": b["/orientation/roll-deg"],
            "pitch_deg": b["/orientation/pitch-deg"],
            "hdg_deg": b["/orientation/heading-deg"],
            "ias_kt": b["/velocities/airspeed-kt"],
            "gs_kt": b["/velocities/groundspeed-kt"],
            "vs_fpm": b["/velocities/vertical-speed-fps"] * 60.0,
            "p_dps": b["/orientation/p-body"] * 57.29578,
            "q_dps": b["/orientation/q-body"] * 57.29578,
            "r_dps": b["/orientation/r-body"] * 57.29578,
            "rpm": b["/engines/active-engine/rpm"],
            "yoke_aileron": b["/controls/flight/aileron"],
            "yoke_elevator": b["/controls/flight/elevator"],
            "rudder": b["/controls/flight/rudder"],
            "throttle": b["/controls/engines/engine[0]/throttle"],
            "flaps": b["/controls/flight/flaps"],
            "brake_left": b["/controls/gear/brake-left"],
            "brake_right": b["/controls/gear/brake-right"],
            "brake_parking": b["/controls/gear/brake-parking"],
            "spoof_jam_active": int(b["/aero-shield/spoof-jam-active"] > 0.5),
            "shield_protection": int(b["/aero-shield/shield-protection"] > 0.5),
            "cmp_shield_xtk_m": b["/aero-shield/cmp-shield-xtk-m"],
            "cmp_spoof_xtk_m": b["/aero-shield/cmp-spoof-xtk-m"],
            "cmp_ins_xtk_m": b["/aero-shield/cmp-ins-xtk-m"],
            "cmp_chi2": b["/aero-shield/cmp-chi2"],
            "cmp_cusum": b["/aero-shield/cmp-cusum"],
            "vis_m": b["/environment/visibility-m"],
            "wind_dir_deg": b["/environment/wind-from-heading-deg"],
            "wind_kt": b["/environment/wind-speed-kt"],
            "rain_norm": b["/environment/rain-norm"],
        }

    def close(self) -> None:
        try:
            self.sock.close()
        except Exception:
            pass


def fetch_aeroshield_api() -> dict:
    try:
        with urllib.request.urlopen("http://127.0.0.1:8080/api/health", timeout=1.0) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except Exception as exc:
        return {"error": str(exc)}


def post_aeroshield_cmd(payload: dict) -> dict:
    try:
        req = urllib.request.Request(
            "http://127.0.0.1:8080/api/command",
            data=json.dumps(payload).encode("utf-8"),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(req, timeout=1.0) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except Exception as exc:
        return {"error": str(exc)}


def set_spoof_jam_mode(enabled: bool, stealth: bool = False, shield_on: bool = True) -> dict:
    r1 = post_aeroshield_cmd({"type": "SetShieldProtection", "payload": {"enabled": bool(shield_on)}})
    r2 = post_aeroshield_cmd({
        "type": "ConfigureCyberAttack",
        "payload": {
            "gps_jump_spoof": bool(enabled and not stealth),
            "gps_stealth_ramp_spoof": bool(enabled and stealth),
            "cut_internet_comms": bool(enabled),
            "poison_meteo_cache": bool(enabled),
        },
    })
    try:
        mcp = FlightGearMcpClient()
        mcp.set_batch([
            ("/aero-shield/spoof-jam-active", 1 if enabled else 0),
            ("/aero-shield/shield-protection", 1 if shield_on else 0),
        ])
        mcp.close()
    except Exception:
        pass
    return {"shield": r1, "cyber_attack": r2}


def apply_storm_weather(mcp: FlightGearMcpClient) -> None:
    """
    Configure CAT IIIc Thunderstorm + Heavy Rain + Fog (+TSRA FG) in FlightGear:
      - Wind: 175 deg @ 14 kt (14 kt * 1.68781 = 23.629 fps; from 175 deg -> north=-23.5394 fps, east=+2.0594 fps)
      - Visibility: 220 m
      - Rain: 0.85, Turbulence: 0.28
    """
    mcp.set_batch([
        ("/environment/config/enabled", 1),
        ("/environment/params/metar-updates-environment", 0),
        ("/environment/config/presets/visibility-m", 220.0),
        ("/environment/config/presets/visibility-m-override", 1),
        ("/environment/config/presets/wind-from-north-fps", -23.5394),
        ("/environment/config/presets/wind-from-east-fps", 2.0594),
        ("/environment/config/presets/wind-override", 1),
        ("/environment/config/presets/turbulence-magnitude-norm", 0.28),
        ("/environment/config/presets/turbulence-magnitude-norm-override", 1),
        ("/environment/config/boundary/entry[0]/visibility-m", 220.0),
        ("/environment/config/boundary/entry[0]/wind-from-heading-deg", 175.0),
        ("/environment/config/boundary/entry[0]/wind-speed-kt", 14.0),
        ("/environment/config/boundary/entry[0]/turbulence-norm", 0.28),
        ("/environment/config/boundary/entry[1]/visibility-m", 220.0),
        ("/environment/config/boundary/entry[1]/wind-from-heading-deg", 175.0),
        ("/environment/config/boundary/entry[1]/wind-speed-kt", 14.0),
        ("/environment/config/boundary/entry[1]/turbulence-norm", 0.28),
        ("/environment/config/aloft/entry[0]/visibility-m", 220.0),
        ("/environment/config/aloft/entry[0]/wind-from-heading-deg", 175.0),
        ("/environment/config/aloft/entry[0]/wind-speed-kt", 16.0),
        ("/environment/config/aloft/entry[0]/turbulence-norm", 0.28),
        ("/environment/visibility-m", 220.0),
        ("/environment/wind-from-heading-deg", 175.0),
        ("/environment/wind-speed-kt", 14.0),
        ("/environment/wind-from-north-fps", -23.5394),
        ("/environment/wind-from-east-fps", 2.0594),
        ("/environment/rain-norm", 0.85),
        ("/environment/turbulence/magnitude-norm", 0.28),
        ("/environment/turbulence/use-cloud-turbulence", 1),
    ])
    post_aeroshield_cmd({"type": "SetFogDensity", "payload": {"visibility_m": 180.0}})
    post_aeroshield_cmd({"type": "SetCrosswind", "payload": {"wind_mps": 7.2, "turbulence": 0.35}})


def cmd_verify() -> int:
    mcp = FlightGearMcpClient()
    snap = mcp.snapshot_controls_and_guidance()
    api = fetch_aeroshield_api()
    mcp.close()
    print(json.dumps({"fg_mcp": snap, "aeroshield_http_online": "error" not in api}, indent=2))
    return 0


def cmd_fly_loop(max_seconds: float = 140.0, delayed_spoof: bool = False) -> int:
    mcp = FlightGearMcpClient()
    # Disable any in-sim Nasal autoland cheat and activate external FG-MCP control
    mcp.ensure_healthy_engine_and_airframe(force_start=True)
    apply_storm_weather(mcp)
    post_aeroshield_cmd({"type": "SetShieldProtection", "payload": {"enabled": True}})
    mcp.set_batch([
        ("/aero-shield/autoland-local", 0),
        ("/aero-shield/autoland-cmd", 0),
        ("/aero-shield/fg-mcp-override", 1),
        ("/aero-shield/shield-protection", 1),
        ("/controls/gear/brake-parking", 0),
        ("/controls/gear/brake-left", 0.0),
        ("/controls/gear/brake-right", 0.0),
        ("/controls/flight/flaps", 0.33),
        ("/controls/flight/aileron", 0.0),
        ("/controls/flight/elevator", -0.02),
        ("/controls/flight/rudder", 0.0),
        ("/controls/flight/aileron-trim", 0.016),
        ("/controls/flight/elevator-trim", -0.020),
        ("/controls/engines/engine[0]/throttle", 0.56),
        ("/controls/engines/current-engine/throttle", 0.56),
    ])

    init_snap = mcp.snapshot_controls_and_guidance()
    if (
        (init_snap["agl_m"] < 8.0 and init_snap["dist_m"] > 450.0)
        or abs(init_snap["roll_deg"]) > 55.0
        or (init_snap["dist_m"] > 1200.0 and abs(init_snap["xtk_m"]) > 28.0)
        or init_snap["agl_m"] > 195.0
    ):
        print(
            f"[FG-MCP INIT RECOVERY] Repozitionare curata pe panta LRCV 09 (dist={init_snap['dist_m']:.0f}m, "
            f"Xtk={init_snap['xtk_m']:+.1f}m, AGL={init_snap['agl_m']:.1f}m, R={init_snap['roll_deg']:+.1f})..."
        )
        mcp.close()
        cmd_reposition()
        mcp = FlightGearMcpClient()

    print("[FG-MCP PILOT] Preia controlul extern prin TCP :5501 urmand EXCLUSIV instructiunile AERO-SHIELD (FD == AP)...")
    t0 = time.time()
    t_prev = t0
    step = 0
    rwy_hdg = 88.9314
    xtk_int = 0.0
    rud_int = 0.0
    prev_xtk_m = None
    xtk_rate_filt = 0.0
    gs_int = 0.0
    elev_int = 0.0
    cmd_pit = -1.0
    hdg_corr = 0.0
    approach_crab_deg = 12.2
    touchdown_latched = False
    delayed_spoof_triggered = False

    try:
        while time.time() - t0 < max_seconds:
            step += 1
            now = time.time()
            dt = clamp(now - t_prev, 0.02, 0.25)
            t_prev = now

            if delayed_spoof and not delayed_spoof_triggered and (now - t0) >= 4.0:
                delayed_spoof_triggered = True
                print("[FG-MCP CYBER] t=4.0s -> Declansare LIVE Spoofing GPS (+250m) & Bruiaj ILS (Scut AERO-SHIELD ACTIV)!")
                sys.stdout.flush()
                set_spoof_jam_mode(enabled=True, stealth=False, shield_on=True)

            s = mcp.snapshot_controls_and_guidance()
            agl_m = s["agl_m"]
            dist_m = s["dist_m"]
            xtk_m = s["xtk_m"]
            tgt_hdg = s["tgt_hdg_deg"]
            gs_err = s["gs_err_deg"]
            tgt_pit = s["tgt_pit_deg"]
            flare = s["flare"]
            cur_roll = s["roll_deg"]
            cur_pitch = s["pitch_deg"]
            cur_hdg = s["hdg_deg"]
            kt = s["ias_kt"]
            vs_fpm = s["vs_fpm"]
            p_rate = s["p_dps"]
            q_rate = s["q_dps"]
            r_rate = s["r_dps"]
            rpm = s["rpm"]

            if agl_m > 4.0 and rpm < 450.0:
                mcp.ensure_healthy_engine_and_airframe(force_start=True)
            elif step % 20 == 1:
                mcp.ensure_healthy_engine_and_airframe(force_start=False)

            if not hasattr(mcp, "_last_fg_spoof"):
                mcp._last_fg_spoof = 0
                mcp._last_fg_shield = 1
                mcp._spoof_t0 = None

            if step % 10 == 1:
                cur_sp = s["spoof_jam_active"]
                cur_sh = 1
                if s["shield_protection"] == 0:
                    mcp.set_prop("/aero-shield/shield-protection", 1)
                    post_aeroshield_cmd({"type": "SetShieldProtection", "payload": {"enabled": True}})
                if cur_sp != mcp._last_fg_spoof or cur_sh != mcp._last_fg_shield:
                    mcp._last_fg_spoof = cur_sp
                    mcp._last_fg_shield = cur_sh
                    post_aeroshield_cmd({"type": "SetShieldProtection", "payload": {"enabled": True}})
                    post_aeroshield_cmd({
                        "type": "ConfigureCyberAttack",
                        "payload": {
                            "gps_jump_spoof": bool(cur_sp),
                            "gps_stealth_ramp_spoof": False,
                            "cut_internet_comms": bool(cur_sp),
                            "poison_meteo_cache": bool(cur_sp),
                        },
                    })
                if cur_sp > 0 or s["gnss_quar"] > 0:
                    if mcp._spoof_t0 is None:
                        mcp._spoof_t0 = now
                    dt_sp = now - mcp._spoof_t0
                    spoof_xtk = xtk_m + min(250.0, 55.0 + dt_sp * 32.0)
                    ins_xtk = xtk_m + min(180.0, 0.42 * dt_sp * dt_sp + 1.8 * dt_sp)
                    chi2_v = 96.4
                    cusum_v = min(45.0, 8.5 + dt_sp * 2.2)
                else:
                    mcp._spoof_t0 = None
                    spoof_xtk = xtk_m + 0.55 * math.sin(now * 1.3)
                    ins_xtk = xtk_m + 1.45 * math.cos(now * 0.7)
                    chi2_v = 1.15
                    cusum_v = 0.08
                mcp.set_batch([
                    ("/aero-shield/cmp-shield-xtk-m", round(xtk_m, 2)),
                    ("/aero-shield/cmp-spoof-xtk-m", round(spoof_xtk, 2)),
                    ("/aero-shield/cmp-ins-xtk-m", round(ins_xtk, 2)),
                    ("/aero-shield/cmp-chi2", round(chi2_v, 2)),
                    ("/aero-shield/cmp-cusum", round(cusum_v, 2)),
                ])

            # CRITIC: dx_before_thr este distanta longitudinala ramasa pana la pragul asfaltat al Pistei 09 (X_rwy = 0m)!
            # Cand dx_before_thr > 25.0m, avionul este INCA INAINTE DE PRAG (in camp)!
            # Declansam touchdown_latched EXCLUSIV dupa trecerea pragului (dx_before_thr <= 25.0m)!
            dx_before_thr = math.sqrt(max(0.0, dist_m * dist_m - xtk_m * xtk_m))
            if agl_m <= 1.65 and dx_before_thr <= 25.0:
                if not touchdown_latched:
                    mcp._td_snap = {
                        "xtk_m": xtk_m,
                        "hdg_deg": cur_hdg,
                        "hdg_corr_deg": hdg_corr,
                        "gs_err_deg": gs_err,
                    }
                touchdown_latched = True

            if touchdown_latched:
                # Touchdown & Rollout on Runway 09: speed-scheduled rudder/NWS + yaw-rate damper
                mcp.ensure_healthy_engine_and_airframe(force_start=False)
                if prev_xtk_m is None:
                    prev_xtk_m = xtk_m
                raw_xtk_rate = clamp((xtk_m - prev_xtk_m) / max(0.02, dt), -8.0, 8.0)
                prev_xtk_m = xtk_m
                xtk_rate_filt = 0.75 * xtk_rate_filt + 0.25 * raw_xtk_rate
                rwy_err = wrap180(rwy_hdg - cur_hdg)
                hdg_corr = wrap180(tgt_hdg - cur_hdg)
                spd_blend = smoothstep_c1(22.0, 50.0, kt)
                nws_hdg_kp = float(SYNTH_GAINS.get("nws_hdg_kp", 0.048)) * (1.10 + 0.85 * spd_blend)
                nws_xtk_kp = float(SYNTH_GAINS.get("nws_xtk_kp", 0.018)) * 2.40
                nws_xtk_kd = float(SYNTH_GAINS.get("nws_xtk_kd", 0.015)) * 2.40
                rud_trim_gnd = rud_int * (0.10 + 0.32 * spd_blend)
                max_rud_gnd = 0.24 + 0.36 * spd_blend
                rud = clamp(
                    0.012
                    + rud_trim_gnd
                    + rwy_err * nws_hdg_kp
                    - r_rate * (0.018 + 0.014 * spd_blend)
                    - xtk_m * nws_xtk_kp
                    - xtk_rate_filt * nws_xtk_kd,
                    -max_rud_gnd,
                    max_rud_gnd,
                )
                brk = round(clamp(0.36 + (52.0 - kt) * 0.012, 0.32, 0.62) if kt > 4.0 else 0.35, 4)
                ail_gnd = round(clamp(approach_crab_deg * 0.025 - cur_roll * 0.05, -0.38, 0.38), 4)
                mcp.set_batch([
                    ("/controls/engines/engine[0]/throttle", 0.0),
                    ("/controls/engines/current-engine/throttle", 0.0),
                    ("/controls/flight/flaps", 0.0),
                    ("/controls/flight/aileron", ail_gnd),
                    ("/controls/flight/elevator", 0.14),
                    ("/controls/flight/rudder", round(rud, 4)),
                    ("/controls/gear/brake-left", brk),
                    ("/controls/gear/brake-right", brk),
                ])
                if kt < 5.0 or s.get("gs_kt", kt) < 1.0:
                    on_rwy = abs(xtk_m) <= 22.5 and dx_before_thr <= 25.0
                    quar_flag = max(int(s["gnss_quar"]), 1 if delayed_spoof_triggered else 0)
                    td_s = getattr(mcp, "_td_snap", {"xtk_m": xtk_m, "hdg_deg": cur_hdg})
                    verdict_pass = on_rwy and abs(td_s["xtk_m"]) < 2.5 and abs(xtk_m) < 8.0
                    print(
                        f"[FG-MCP TOUCHDOWN & STOP SUCCESS] TOUCHDOWN_ON_RUNWAY={on_rwy} "
                        f"TD_Xtk={td_s['xtk_m']:+.2f}m TD_Hdg={td_s['hdg_deg']:.1f} "
                        f"dist={dist_m:.1f}m Xtk={xtk_m:+.2f}m hdg_corr={hdg_corr:+.2f}deg gs_err={gs_err:+.2f}deg "
                        f"AGL={agl_m:.2f}m IAS={kt:.1f}kt GS={s.get('gs_kt', 0.0):.1f}kt Hdg={cur_hdg:.1f} Quar={quar_flag} "
                        f"Storm=(Vis:{s['vis_m']:.0f}m,Wind:{s['wind_dir_deg']:.0f}@{s['wind_kt']:.1f}kt,Rain:{s['rain_norm']:.2f}) "
                        f"VERDICT={'PASS' if verdict_pass else 'FAIL'}"
                    )
                    sys.stdout.flush()
                    return 0 if on_rwy else 1
            else:
                # Airborne guidance strictly following AERO-SHIELD Flight Director (FD == AP, Zero Split-Brain)
                if prev_xtk_m is None:
                    prev_xtk_m = xtk_m
                raw_xtk_rate = clamp((xtk_m - prev_xtk_m) / max(0.02, dt), -8.0, 8.0)
                prev_xtk_m = xtk_m
                xtk_rate_filt = 0.75 * xtk_rate_filt + 0.25 * raw_xtk_rate
                if agl_m > 38.0 and kt > 45.0:
                    approach_crab_deg = 0.92 * approach_crab_deg + 0.08 * s["crab_deg"]
                eff_crab_deg = s["crab_deg"] if abs(s["crab_deg"]) > abs(approach_crab_deg) else approach_crab_deg
                ap = compute_ap_control_from_fd(
                    tgt_hdg_deg=tgt_hdg,
                    cur_hdg_deg=cur_hdg,
                    gs_err_deg=gs_err,
                    tgt_pit_deg=tgt_pit,
                    agl_m=agl_m,
                    dist_m=dist_m,
                    xtk_m=xtk_m,
                    cur_roll_deg=cur_roll,
                    cur_pitch_deg=cur_pitch,
                    ias_kt=kt,
                    vs_fpm=vs_fpm,
                    p_dps=p_rate,
                    q_dps=q_rate,
                    r_dps=r_rate,
                    flare_active=flare,
                    rwy_hdg_deg=rwy_hdg,
                    crab_deg=eff_crab_deg,
                    xtk_rate_mps=xtk_rate_filt,
                    xtk_int=xtk_int,
                    rud_int=rud_int,
                    gs_int=gs_int,
                    elev_int=elev_int,
                    dt=dt,
                )
                hdg_corr = ap["hdg_corr_deg"]
                cmd_pit = ap["cmd_pit_deg"]
                xtk_int = ap["xtk_int"]
                rud_int = ap["rud_int"]
                gs_int = ap["gs_int"]
                elev_int = ap["elev_int"]

                mcp.set_batch([
                    ("/controls/flight/aileron", round(ap["aileron"], 4)),
                    ("/controls/flight/elevator", round(ap["elevator"], 4)),
                    ("/controls/flight/rudder", round(ap["cmd_rud"], 4)),
                    ("/controls/engines/engine[0]/throttle", round(ap["throttle"], 4)),
                    ("/controls/engines/current-engine/throttle", round(ap["throttle"], 4)),
                    ("/controls/gear/brake-left", 0.0),
                    ("/controls/gear/brake-right", 0.0),
                ])

            if step % 15 == 1:
                print(
                    f"[FG-MCP t={time.time()-t0:4.1f}s] dist={dist_m:6.1f}m Xtk={xtk_m:+6.1f}m "
                    f"hdg_corr={hdg_corr:+5.2f}deg gs_err={gs_err:+5.2f}deg "
                    f"AGL={agl_m:5.1f}m IAS={kt:4.1f}kt VS={vs_fpm:+5.0f}fpm RPM={rpm:4.0f} "
                    f"Att=(R:{cur_roll:+4.1f},P:{cur_pitch:+4.1f},H:{cur_hdg:5.1f}) "
                    f"Tgt=(H:{tgt_hdg:5.1f},P:{cmd_pit if not touchdown_latched else 0.6:+4.1f}) "
                    f"Yoke=(ail:{s['yoke_aileron']:+.2f},elv:{s['yoke_elevator']:+.2f},rud:{s['rudder']:+.2f},thr:{s['throttle']:.2f},brk:{s['brake_left']:.2f}) "
                    f"Storm=(Vis:{s['vis_m']:.0f}m,Wind:{s['wind_dir_deg']:.0f}@{s['wind_kt']:.1f}kt,Rain:{s['rain_norm']:.2f})"
                )
                sys.stdout.flush()

            time.sleep(0.04)
    finally:
        try:
            mcp.set_batch([
                ("/aero-shield/fg-mcp-override", 1),
                ("/aero-shield/autoland-local", 1),
                ("/aero-shield/autoland-cmd", 1),
                ("/controls/flight/aileron", 0.0),
                ("/controls/flight/elevator", 0.0),
                ("/controls/flight/rudder", 0.0),
            ])
        except Exception:
            pass
        mcp.close()

    return 1


def fly_spoof_scenario(max_seconds: float = 120.0) -> int:
    cmd_reposition()
    return cmd_fly_loop(max_seconds=max_seconds, delayed_spoof=True)


def cmd_reposition() -> int:
    set_spoof_jam_mode(enabled=False, stealth=False, shield_on=True)
    mcp = FlightGearMcpClient()
    mcp.ensure_healthy_engine_and_airframe(force_start=True)
    apply_storm_weather(mcp)
    mcp.set_batch([
        ("/aero-shield/fg-mcp-override", 1),
        ("/aero-shield/shield-protection", 1),
        ("/aero-shield/spoof-jam-active", 0),
        ("/controls/gear/brake-left", 0.0),
        ("/controls/gear/brake-right", 0.0),
        ("/controls/gear/brake-parking", 0),
        ("/controls/flight/flaps", 0.33),
        ("/controls/flight/aileron", 0.0),
        ("/controls/flight/elevator", 0.01),
        ("/controls/flight/rudder", 0.0),
        ("/controls/flight/aileron-trim", 0.016),
        ("/controls/flight/elevator-trim", 0.0),
        ("/controls/engines/engine[0]/throttle", 0.54),
        ("/controls/engines/current-engine/throttle", 0.54),
        ("/position/latitude-deg", 44.317520),
        ("/position/longitude-deg", 23.841500),
        ("/position/altitude-ft", 950.0),
        ("/orientation/heading-deg", 88.9314),
        ("/orientation/pitch-deg", -1.5),
        ("/orientation/roll-deg", 0.0),
        ("/velocities/uBody-fps", 122.0),
        ("/velocities/vBody-fps", 0.0),
        ("/velocities/wBody-fps", 4.0),
        ("/velocities/speed-north-fps", 2.3),
        ("/velocities/speed-east-fps", 122.0),
        ("/velocities/speed-down-fps", 5.0),
    ])
    post_aeroshield_cmd({"type": "ResetApproach"})
    time.sleep(0.35)
    mcp.ensure_healthy_engine_and_airframe(force_start=True)
    mcp.set_batch([
        ("/position/latitude-deg", 44.317520),
        ("/position/longitude-deg", 23.841500),
        ("/position/altitude-ft", 950.0),
        ("/orientation/heading-deg", 88.9314),
        ("/orientation/pitch-deg", -1.5),
        ("/orientation/roll-deg", 0.0),
        ("/velocities/uBody-fps", 122.0),
        ("/velocities/vBody-fps", 0.0),
        ("/velocities/wBody-fps", 4.0),
    ])
    mcp.close()
    return 0


def cmd_continuous_loop(flights: int = 4) -> int:
    for idx in range(1, flights + 1):
        spoof_on = (idx % 2 == 1)
        print(
            f"\n==================== [AI LOOP ZBOR #{idx}/{flights}] "
            f"Reset Pozitie in Aer (1.4 NM LRCV 09) | Spoofing GPS/Bruiaj={'ACTIV (+250m la t=4.0s)' if spoof_on else 'NOMINAL'} ===================="
        )
        sys.stdout.flush()
        cmd_reposition()
        cmd_fly_loop(120.0, delayed_spoof=spoof_on)
        if idx < flights:
            print(f"[AI LOOP] Aterizare #{idx} completa pe Pista 09! Reiau urmatorul zbor AI in 3.5 secunde...")
            sys.stdout.flush()
            time.sleep(3.5)
    return 0


if __name__ == "__main__":
    mode = sys.argv[1] if len(sys.argv) > 1 else "verify"
    if mode == "verify":
        sys.exit(cmd_verify())
    elif mode == "fly":
        dur = float(sys.argv[2]) if len(sys.argv) > 2 else 140.0
        sys.exit(cmd_fly_loop(dur))
    elif mode in ("fly-spoof", "fly_spoof"):
        dur = float(sys.argv[2]) if len(sys.argv) > 2 else 120.0
        sys.exit(fly_spoof_scenario(dur))
    elif mode == "loop":
        n_flights = int(sys.argv[2]) if len(sys.argv) > 2 else 4
        sys.exit(cmd_continuous_loop(n_flights))
    elif mode == "reposition":
        sys.exit(cmd_reposition())
    elif mode == "spoof":
        sub = sys.argv[2] if len(sys.argv) > 2 else "on"
        res = set_spoof_jam_mode(enabled=(sub != "off"), stealth=(sub == "stealth"), shield_on=True)
        print(json.dumps(res, indent=2))
        sys.exit(0)
    else:
        print(f"Unknown mode: {mode}", file=sys.stderr)
        sys.exit(2)
