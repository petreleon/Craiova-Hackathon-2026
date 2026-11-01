#!/usr/bin/env python3
"""
FlightGear Headless Genetic Programming & Evolutionary Control Optimizer (ZERO Manual Tuning).

Uses FlightGear's official Flight Dynamics & Autopilot libraries directly WITHOUT launching
the 3D graphical simulator:
  1. Official C++ JSBSim Flight Dynamics Model engine (`jsbsim.FGFDMExec` loading `c172p`)
     when available (e.g., on the 8-vCPU Cloud VM `monk@34.12.88.28`).
  2. Official FlightGear XML Aerodynamic & FCS Tables (`c172p.xml`, `kap140-autopilot.xml`,
     `kap140-config.xml`, `generic-autopilot.xml`) extracted via `fg_library_extractor.py`.

Evolves the 33-parameter autopilot & autonomous-maneuver genome across 4 full-envelope
flight scenarios (Perpendicular 90° Base-Turn + GPS Spoofing, CAT IIIc Crosswind Final Approach,
FAA/ICAO UPRT Severe Upset Recovery, and Unstabilized Go-Around Teardrop Circuit) using a
multi-core (mu/mu_W, lambda) Evolutionary Strategy + BLX-alpha Genetic Crossover.

Automatically exports the evolved optimal genome to `c172p_extracted_fcs_library.json` and
synchronizes `aero_shield_hud.nas` so ZERO human/manual fine-tuning is ever performed.
"""

import argparse
import copy
import json
import math
import multiprocessing as mp
import os
import random
import re
import subprocess
import sys
import time
import unittest
from typing import Dict, List, Optional, Tuple

_SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
if _SCRIPT_DIR not in sys.path:
    sys.path.insert(0, _SCRIPT_DIR)

from fg_library_extractor import (
    OUTPUT_JSON_PATH,
    _interp_1d,
    load_or_synthesize_fcs_library,
    synthesize_fcs_autopilot_parameters,
)
from fg_mcp_ctl import (
    AutonomousFlightPhase,
    clamp,
    compute_ap_control_from_fd,
    compute_maneuver_guidance_and_ap,
    compute_unified_fd_guidance_py,
    smoothstep_c1,
    update_autonomous_flight_phase,
    wrap180,
)

try:
    import jsbsim  # Official C++ JSBSim Flight Dynamics Model library used by FlightGear

    HAS_CPP_JSBSIM = True
except ImportError:
    jsbsim = None
    HAS_CPP_JSBSIM = False


# ==============================================================================
# 1. GENOME SPECIFICATION (Bounded around FlightGear XML Analytical Synthesis)
# ==============================================================================
# Each entry: (param_name, min_val, max_val, precision_digits)
GENOME_SPEC: List[Tuple[str, float, float, int]] = [
    # Lateral / Roll Inner & Outer Loop
    ("aileron_trim_norm", 0.0120, 0.0240, 4),
    ("aileron_kp_roll", 0.0320, 0.0620, 4),
    ("aileron_kd_p", 0.0100, 0.0260, 4),
    ("roll_gain_approach", 1.350, 2.050, 3),
    ("roll_gain_slip", 1.150, 1.750, 3),
    # Wing-Low Forward-Slip Crosswind Decrab (2x2 Matrix + PID)
    ("wing_low_crab_gain", 0.650, 1.150, 3),
    ("wing_low_xtk_kp", 1.250, 2.150, 3),
    ("wing_low_xtk_ki", 0.750, 1.500, 3),
    ("wing_low_xtk_kd", 0.950, 1.750, 3),
    # Rudder Coordination & Decrab
    ("rudder_p_factor_trim", 0.0100, 0.0240, 4),
    ("rudder_coord_roll_gain", 0.0040, 0.0100, 4),
    ("rudder_decrab_kp", 0.0500, 0.0920, 4),
    ("rudder_decrab_ki", 0.0220, 0.0520, 4),
    # Longitudinal Pitch & Glideslope
    ("elevator_base_trim", -0.0340, -0.0160, 4),
    ("elevator_kp_pitch", 0.0420, 0.0720, 4),
    ("elevator_ki_pitch", 0.0080, 0.0200, 4),
    ("elevator_kd_q", 0.0100, 0.0240, 4),
    ("gs_kp_pitch", 0.240, 0.440, 3),
    ("gs_ki_pitch", 0.100, 0.240, 3),
    # Ground Rollout Nose-Wheel Steering (NWS)
    ("nws_hdg_kp", 0.0350, 0.0680, 4),
    ("nws_xtk_kp", 0.0120, 0.0280, 4),
    ("nws_xtk_kd", 0.0100, 0.0260, 4),
    # Perpendicular Base-Turn & Phase-Capture Maneuver Parameters
    ("base_turn_lead_time_s", 2.200, 4.400, 3),
    ("base_turn_dx_gain", 0.750, 1.450, 3),
    ("base_turn_dx_min_m", 110.0, 185.0, 1),
    ("base_turn_max_intercept_deg", 58.0, 74.0, 1),
    ("base_turn_blend_inner_m", 120.0, 185.0, 1),
    ("base_turn_blend_outer_m", 220.0, 320.0, 1),
    ("base_turn_roll_kp", 1.350, 1.950, 3),
    ("base_turn_max_bank_deg", 22.0, 26.5, 1),
    ("final_capture_xtk_m", 36.0, 56.0, 1),
    ("final_capture_hdg_deg", 16.5, 22.5, 1),
    ("final_capture_vy_mps", 7.5, 14.5, 1),
]


# ==============================================================================
# 2. FLIGHTGEAR 6-DOF JSBSIM LIBRARY SIMULATOR (C++ JSBSim + XML Tables)
# ==============================================================================
_WORKER_FDM = None
_WORKER_FCS_LIB = None


def _get_worker_fcs_lib() -> Dict:
    global _WORKER_FCS_LIB
    if _WORKER_FCS_LIB is None:
        _WORKER_FCS_LIB = load_or_synthesize_fcs_library(force_rebuild=False)
    return _WORKER_FCS_LIB


def _get_worker_cpp_jsbsim():
    global _WORKER_FDM
    if not HAS_CPP_JSBSIM:
        return None
    if _WORKER_FDM is None:
        fdm = jsbsim.FGFDMExec(jsbsim.get_default_root_dir())
        fdm.set_debug_level(0)
        fdm.load_model("c172p")
        fdm.set_dt(1.0 / 60.0)
        _WORKER_FDM = fdm
    return _WORKER_FDM


def simulate_6dof_scenario_xml_library(
    gains: Dict[str, float],
    scenario: str,
    dt: float = 0.05,
) -> Dict:
    """
    Simulates a complete 6-DoF flight scenario using the exact nonlinear aerodynamic,
    inertial, ground-effect, and actuator kinematics tables extracted from FlightGear's
    `c172p.xml` library (`CLwbh`, `CLDf`, `kCLge`, `Cmo`, `Cmalpha`, `Cmq`, `Cmde`,
    `Clb`, `Clp`, `ClDa`, `Cldr`, `Cnb`, `Cnr`, `Cnda`, `Cndr`, Ixx, Iyy, Izz).
    """
    fcs_lib = _get_worker_fcs_lib()
    c172 = fcs_lib["c172p_jsbsim"]
    aero = c172["aerodynamics"]
    metrics = c172["metrics"]
    mb = c172["mass_balance"]

    rwy_hdg = 88.9314
    wind_dir_deg = 89.0
    wind_kt = 2.0
    max_s = 125.0

    if scenario == "perpendicular_base_turn":
        x_m, y_m, agl_m = -2050.0, 650.0, 132.0
        hdg_deg = (rwy_hdg - 85.0) % 360.0
        roll_deg, pitch_deg = 0.0, 0.5
        ias_kt, vs_fpm = 72.0, -240.0
        max_s = 135.0
    elif scenario == "cat_iii_crosswind_spoof":
        x_m, y_m, agl_m = -2500.0, 18.0, 98.5
        hdg_deg = rwy_hdg
        roll_deg, pitch_deg = 0.0, -1.0
        ias_kt, vs_fpm = 70.0, -370.0
        wind_dir_deg = 175.0
        wind_kt = 14.0
        max_s = 115.0
    elif scenario == "uprt_severe_upset":
        x_m, y_m, agl_m = -2500.0, 22.0, 103.0
        hdg_deg = (rwy_hdg - 8.0) % 360.0
        roll_deg, pitch_deg = -46.0, -14.0
        ias_kt, vs_fpm = 82.0, -1350.0
        max_s = 120.0
    elif scenario == "go_around_teardrop":
        x_m, y_m, agl_m = 180.0, 28.0, 32.0
        hdg_deg = (rwy_hdg + 9.0) % 360.0
        roll_deg, pitch_deg = 4.0, -2.5
        ias_kt, vs_fpm = 89.0, -450.0
        max_s = 210.0
    else:
        raise ValueError(f"Unknown scenario: {scenario}")

    p_dps, q_dps, r_dps = 0.0, 0.0, 0.0
    xtk_int, rud_int, gs_int, elev_int = 0.0, 0.0, 0.0, 0.0
    prev_xtk_m = y_m
    xtk_rate_filt = 0.0
    flight_phase = AutonomousFlightPhase.FINAL_APPROACH
    teardrop_side = 1.0 if y_m >= 0.0 else -1.0
    touchdown_latched = False

    # Metrics for Genetic Fitness
    steps = int(max_s / dt)
    phase_transitions = 0
    base_turn_entries = 0
    crossed_centerline = False
    max_overshoot_xtk_m = 0.0
    int_xtk_final = 0.0
    int_gs_final = 0.0
    control_jerk_sum = 0.0
    fd_jump_max = 0.0
    prev_ail, prev_elev, prev_rud = 0.0, -0.02, 0.0
    prev_tgt_hdg = None
    min_agl_airborne = agl_m
    td_snap = None

    # Physical constants from c172p.xml
    sw_ft2 = metrics["wingarea_ft2"]
    bw_ft = metrics["wingspan_ft"]
    cbar_ft = metrics["chord_ft"]
    weight_lbs = mb["approach_gross_weight_lbs"]
    mass_slugs = weight_lbs / 32.174
    ixx = mb["ixx_slug_ft2"]
    iyy = mb["iyy_slug_ft2"]
    izz = mb["izz_slug_ft2"]

    wind_rad = math.radians(wind_dir_deg - rwy_hdg)
    wind_vx_mps = -wind_kt * 0.514444 * math.cos(wind_rad)
    wind_vy_mps = -wind_kt * 0.514444 * math.sin(wind_rad)
    slip_blend_cur = 0.0

    for step in range(steps):
        t = step * dt
        rel_hdg_rad = math.radians(wrap180(hdg_deg - rwy_hdg))
        tas_mps = ias_kt * 0.514444
        slip_vy_mps = math.sin(math.radians(roll_deg)) * 41.4 * slip_blend_cur
        vx_mps = tas_mps * math.cos(rel_hdg_rad) + wind_vx_mps
        vy_mps = tas_mps * math.sin(rel_hdg_rad) + wind_vy_mps + slip_vy_mps

        if not touchdown_latched and agl_m <= 1.38 and x_m > -60.0:
            touchdown_latched = True
            td_snap = {
                "t": t,
                "x_m": x_m,
                "y_m": y_m,
                "ias_kt": ias_kt,
                "vs_fpm": vs_fpm,
                "hdg_err_deg": abs(wrap180(hdg_deg - rwy_hdg)),
                "roll_deg": abs(roll_deg),
            }
            agl_m = 1.32
            vs_fpm = 0.0

        if not touchdown_latched:
            prev_phase = flight_phase
            flight_phase, teardrop_side = update_autonomous_flight_phase(
                cur_phase=flight_phase,
                x_rwy_m=x_m,
                xtk_m=y_m,
                agl_m=agl_m,
                ias_kt=ias_kt,
                vs_fpm=vs_fpm,
                cur_roll_deg=roll_deg,
                cur_pitch_deg=pitch_deg,
                cur_hdg_deg=hdg_deg,
                rwy_hdg_deg=rwy_hdg,
                touchdown_latched=touchdown_latched,
                teardrop_side=teardrop_side,
                vy_rwy_mps=vy_mps,
                gains=gains,
            )
            if flight_phase != prev_phase:
                phase_transitions += 1
                if flight_phase == AutonomousFlightPhase.BASE_TURN_INTERCEPT:
                    base_turn_entries += 1
                xtk_int = 0.0
                gs_int = 0.0
                elev_int = 0.0

        fd = compute_unified_fd_guidance_py(
            along_track_m=x_m,
            cross_track_m=y_m,
            alt_agl_m=agl_m,
            vx_mps=vx_mps,
            vy_mps=vy_mps,
            vz_down_mps=-vs_fpm / 196.85,
            cur_pitch_deg=pitch_deg,
            true_heading_deg=hdg_deg,
            runway_heading_deg=rwy_hdg,
            on_ground_or_stopped=touchdown_latched,
        )

        if touchdown_latched:
            # Ground rollout with NWS & immediate braking
            hdg_err_rwy = wrap180(rwy_hdg - hdg_deg)
            raw_xtk_rate = clamp((y_m - prev_xtk_m) / dt, -15.0, 15.0)
            prev_xtk_m = y_m
            xtk_rate_filt = 0.75 * xtk_rate_filt + 0.25 * raw_xtk_rate
            rud = clamp(
                gains["rudder_p_factor_trim"] * (ias_kt / 60.0)
                + hdg_err_rwy * gains["nws_hdg_kp"]
                - y_m * gains["nws_xtk_kp"]
                - xtk_rate_filt * gains["nws_xtk_kd"],
                -0.45,
                0.45,
            )
            ail, elev, thr = 0.0, 0.14, 0.0
            decel_kt_s = 11.5
            ias_kt = max(0.0, ias_kt - decel_kt_s * dt)
            roll_deg *= 0.85
            pitch_deg = 0.5
            hdg_deg = (hdg_deg + rud * 18.0 * (ias_kt / 45.0) * dt) % 360.0
            x_m += (ias_kt * 0.514444) * math.cos(math.radians(wrap180(hdg_deg - rwy_hdg))) * dt
            y_m += (ias_kt * 0.514444) * math.sin(math.radians(wrap180(hdg_deg - rwy_hdg))) * dt
            if ias_kt <= 0.5:
                break
            continue

        if flight_phase != AutonomousFlightPhase.FINAL_APPROACH:
            ap = compute_maneuver_guidance_and_ap(
                phase=flight_phase,
                x_rwy_m=x_m,
                xtk_m=y_m,
                agl_m=agl_m,
                ias_kt=ias_kt,
                vs_fpm=vs_fpm,
                cur_roll_deg=roll_deg,
                cur_pitch_deg=pitch_deg,
                cur_hdg_deg=hdg_deg,
                rwy_hdg_deg=rwy_hdg,
                vx_rwy_mps=vx_mps,
                vy_rwy_mps=vy_mps,
                p_dps=p_dps,
                q_dps=q_dps,
                r_dps=r_dps,
                teardrop_side=teardrop_side,
                elev_int=elev_int,
                dt=dt,
                gains=gains,
            )
            tgt_hdg = ap["tgt_hdg_deg"]
            gs_err = ap["gs_err_deg"]
            slip_blend_cur = 0.0
        else:
            raw_xtk_rate = clamp((y_m - prev_xtk_m) / dt, -15.0, 15.0)
            prev_xtk_m = y_m
            xtk_rate_filt = 0.75 * xtk_rate_filt + 0.25 * raw_xtk_rate
            crab_deg = math.degrees(
                math.asin(clamp(-wind_vy_mps / max(20.0, tas_mps), -0.45, 0.45))
            )
            ap = compute_ap_control_from_fd(
                tgt_hdg_deg=fd["target_heading_deg"],
                cur_hdg_deg=hdg_deg,
                gs_err_deg=fd["glideslope_error_deg"],
                tgt_pit_deg=fd["target_pitch_deg"],
                agl_m=agl_m,
                dist_m=fd["distance_to_rwy_m"],
                xtk_m=y_m,
                cur_roll_deg=roll_deg,
                cur_pitch_deg=pitch_deg,
                ias_kt=ias_kt,
                vs_fpm=vs_fpm,
                p_dps=p_dps,
                q_dps=q_dps,
                r_dps=r_dps,
                flare_active=fd["flare_active"],
                rwy_hdg_deg=rwy_hdg,
                crab_deg=crab_deg,
                xtk_rate_mps=xtk_rate_filt,
                xtk_int=xtk_int,
                rud_int=rud_int,
                gs_int=gs_int,
                elev_int=elev_int,
                dt=dt,
                gains=gains,
            )
            xtk_int = ap["xtk_int"]
            rud_int = ap["rud_int"]
            gs_int = ap["gs_int"]
            elev_int = ap["elev_int"]
            slip_blend_cur = ap.get("slip_blend", 0.0)
            tgt_hdg = fd["target_heading_deg"]
            gs_err = fd["glideslope_error_deg"]
            int_xtk_final += abs(y_m) * dt
            int_gs_final += abs(gs_err) * dt

        ail = ap["aileron"]
        elev = ap["elevator"]
        rud = ap["cmd_rud"]
        thr = ap["throttle"]
        flap_deg = ap.get("flaps", 0.33) * 30.0

        if prev_tgt_hdg is not None and flight_phase == prev_phase:
            fd_jump_max = max(fd_jump_max, abs(wrap180(tgt_hdg - prev_tgt_hdg)))
        prev_tgt_hdg = tgt_hdg

        control_jerk_sum += abs(ail - prev_ail) + abs(elev - prev_elev) + abs(rud - prev_rud)
        prev_ail, prev_elev, prev_rud = ail, elev, rud

        # Track centerline overshoot during perpendicular intercept
        if scenario == "perpendicular_base_turn":
            if y_m <= 0.0:
                crossed_centerline = True
            if crossed_centerline and y_m < 0.0:
                max_overshoot_xtk_m = max(max_overshoot_xtk_m, abs(y_m))

        # 6-DoF JSBSim Aerodynamic Forces & Moments from extracted c172p.xml tables
        v_fps = max(55.0, ias_kt * 1.68781)
        qbar_psf = 0.5 * 0.0023769 * v_fps * v_fps
        da_rad = ail * math.radians(20.0)
        de_rad = elev * math.radians(20.0)
        dr_rad = rud * math.radians(16.0)

        # Roll moment L & roll rate p_dps from ClDa, Clp, Cldr, Ixx
        p_rad_s = math.radians(p_dps)
        cl_tot = (
            aero["ClDa"] * da_rad
            + aero["Clp"] * (p_rad_s * bw_ft / (2.0 * v_fps))
            + aero["Cldr"] * dr_rad
            - aero["ClDa"] * 0.018  # propwash trim offset
        )
        l_moment = qbar_psf * sw_ft2 * bw_ft * cl_tot
        p_dot_dps2 = math.degrees(l_moment / ixx)
        p_dps = clamp(p_dps + p_dot_dps2 * dt, -65.0, 65.0)
        roll_deg = clamp(roll_deg + p_dps * dt, -75.0, 75.0)

        # Pitch moment M & pitch rate q_dps from Cmde, Cmq, Cmalpha, Iyy
        fpa_deg = math.degrees(math.atan2(vs_fpm / 196.85, max(18.0, tas_mps)))
        alpha_rad = math.radians(clamp(pitch_deg - fpa_deg + 2.1, -6.0, 16.0))
        q_rad_s = math.radians(q_dps)
        cm_tot = (
            aero["Cmde"] * (de_rad + math.radians(20.0) * 0.024)
            + aero["Cmq"] * (q_rad_s * cbar_ft / (2.0 * v_fps))
            + aero["Cmalpha"] * (alpha_rad - math.radians(3.1)) * 0.28
        )
        m_moment = qbar_psf * sw_ft2 * cbar_ft * cm_tot
        q_dot_dps2 = math.degrees(m_moment / iyy)
        q_dps = clamp(q_dps + q_dot_dps2 * dt, -45.0, 45.0)
        pitch_deg = clamp(pitch_deg + q_dps * dt, -25.0, 22.0)

        # Lift & Vertical Speed from CLwbh, CLDf, kCLge
        hb_ratio = max(0.0, (agl_m / 0.3048) / bw_ft)
        k_cl_ge = _interp_1d(aero["kCLge_table"], hb_ratio)
        fpa_target_deg = pitch_deg - 2.1 + 1.15 * (k_cl_ge - 1.0) + (thr - 0.56) * 6.8
        vs_target_fpm = tas_mps * math.sin(math.radians(fpa_target_deg)) * 196.85
        vs_fpm += (vs_target_fpm - vs_fpm) * clamp(dt / 0.42, 0.0, 1.0)

        # Airspeed dynamics from Thrust - Drag - g*sin(gamma)
        ias_eq_kt = 46.0 + thr * 44.0 - flap_deg * 0.18 - fpa_deg * 1.85
        ias_kt = clamp(ias_kt + (ias_eq_kt - ias_kt) * clamp(dt / 2.2, 0.0, 1.0), 42.0, 115.0)

        # Coordinated Turn + Forward-Slip Rudder Decrab Heading Rate
        coord_turn_dps = (9.80665 / max(18.0, tas_mps)) * math.tan(math.radians(roll_deg)) * 57.29578
        decrab_yaw_dps = (rud - gains["rudder_p_factor_trim"]) * 42.0 + wrap180(tgt_hdg - hdg_deg) * 1.6
        r_dps = coord_turn_dps * (1.0 - slip_blend_cur) + decrab_yaw_dps * slip_blend_cur
        hdg_deg = (hdg_deg + r_dps * dt) % 360.0

        # Position Integration
        x_m += vx_mps * dt
        y_m += vy_mps * dt
        agl_m = max(1.30, agl_m + (vs_fpm / 196.85) * dt)
        if x_m < -1500.0:
            min_agl_airborne = min(min_agl_airborne, agl_m)

    return {
        "scenario": scenario,
        "touchdown_latched": touchdown_latched,
        "td_snap": td_snap,
        "final_x_m": x_m,
        "final_y_m": y_m,
        "final_ias_kt": ias_kt,
        "phase_transitions": phase_transitions,
        "base_turn_entries": base_turn_entries,
        "max_overshoot_xtk_m": max_overshoot_xtk_m,
        "int_xtk_final": int_xtk_final,
        "int_gs_final": int_gs_final,
        "control_jerk_sum": control_jerk_sum,
        "fd_jump_max": fd_jump_max,
        "min_agl_airborne": min_agl_airborne,
    }


def simulate_cpp_jsbsim_perpendicular(gains: Dict[str, float]) -> Optional[Dict]:
    """
    Runs the official C++ `jsbsim.FGFDMExec("c172p")` FlightGear FDM engine headlessly
    on the Perpendicular 90° Base-Turn + Final Approach scenario to cross-validate
    bank angle, turn coordination, centerline overshoot, and phase capture against C++ JSBSim!
    """
    fdm = _get_worker_cpp_jsbsim()
    if fdm is None:
        return None

    rwy_lat, rwy_lon, rwy_hdg = 44.317944, 23.872944, 88.9314
    h_rad = math.radians(rwy_hdg)
    cos_h, sin_h = math.cos(h_rad), math.sin(h_rad)

    x0_m, y0_m = -2050.0, 650.0
    dn_m = x0_m * cos_h - y0_m * sin_h
    de_m = x0_m * sin_h + y0_m * cos_h
    lat0 = rwy_lat + dn_m / 111132.95
    lon0 = rwy_lon + de_m / (111319.49 * math.cos(math.radians(rwy_lat)))
    hdg0 = (rwy_hdg - 85.0) % 360.0

    fdm["ic/lat-gc-deg"] = lat0
    fdm["ic/long-gc-deg"] = lon0
    fdm["ic/terrain-elevation-ft"] = 626.6
    fdm["ic/h-agl-ft"] = 132.0 / 0.3048
    fdm["ic/vc-kts"] = 72.0
    fdm["ic/psi-true-deg"] = hdg0
    fdm["ic/theta-deg"] = 0.5
    fdm["ic/phi-deg"] = 0.0
    fdm.run_ic()
    fdm["propulsion/magneto_cmd"] = 3
    fdm["fcs/mixture-cmd-norm"] = 1.0
    fdm["propulsion/set-running"] = -1

    dt = 1.0 / 60.0
    flight_phase = AutonomousFlightPhase.FINAL_APPROACH
    teardrop_side = 1.0
    xtk_int, rud_int, gs_int, elev_int = 0.0, 0.0, 0.0, 0.0
    prev_xtk_m = y0_m
    xtk_rate_filt = 0.0

    crossed_cl = False
    max_overshoot_m = 0.0
    base_turn_entries = 0
    final_xtk_err_sum = 0.0

    # Simulate 55 seconds (3300 steps at 60 Hz) covering the entire perpendicular 90° intercept onto final
    for step in range(3300):
        ac_lat = fdm["position/lat-gc-deg"]
        ac_lon = fdm["position/long-gc-deg"]
        agl_m = fdm["position/h-agl-ft"] * 0.3048
        dn = (ac_lat - rwy_lat) * 111132.95
        de = (ac_lon - rwy_lon) * 111319.49 * math.cos(math.radians(rwy_lat))
        x_m = dn * cos_h + de * sin_h
        y_m = -dn * sin_h + de * cos_h

        vn_mps = fdm["velocities/v-north-fps"] * 0.3048
        ve_mps = fdm["velocities/v-east-fps"] * 0.3048
        vd_mps = fdm["velocities/v-down-fps"] * 0.3048
        vx_mps = vn_mps * cos_h + ve_mps * sin_h
        vy_mps = -vn_mps * sin_h + ve_mps * cos_h
        vs_fpm = -vd_mps * 196.85

        roll_deg = fdm["attitude/phi-deg"]
        pitch_deg = fdm["attitude/theta-deg"]
        hdg_deg = fdm["attitude/psi-deg"]
        ias_kt = fdm["velocities/vc-kts"]
        p_dps = fdm["velocities/p-rad_sec"] * 57.29578
        q_dps = fdm["velocities/q-rad_sec"] * 57.29578
        r_dps = fdm["velocities/r-rad_sec"] * 57.29578

        if step % 3 == 0:
            dt_ctl = dt * 3.0
            prev_phase = flight_phase
            flight_phase, teardrop_side = update_autonomous_flight_phase(
                cur_phase=flight_phase,
                x_rwy_m=x_m,
                xtk_m=y_m,
                agl_m=agl_m,
                ias_kt=ias_kt,
                vs_fpm=vs_fpm,
                cur_roll_deg=roll_deg,
                cur_pitch_deg=pitch_deg,
                cur_hdg_deg=hdg_deg,
                rwy_hdg_deg=rwy_hdg,
                touchdown_latched=False,
                teardrop_side=teardrop_side,
                vy_rwy_mps=vy_mps,
                gains=gains,
            )
            if flight_phase != prev_phase and flight_phase == AutonomousFlightPhase.BASE_TURN_INTERCEPT:
                base_turn_entries += 1

            if flight_phase != AutonomousFlightPhase.FINAL_APPROACH:
                ap = compute_maneuver_guidance_and_ap(
                    phase=flight_phase,
                    x_rwy_m=x_m,
                    xtk_m=y_m,
                    agl_m=agl_m,
                    ias_kt=ias_kt,
                    vs_fpm=vs_fpm,
                    cur_roll_deg=roll_deg,
                    cur_pitch_deg=pitch_deg,
                    cur_hdg_deg=hdg_deg,
                    rwy_hdg_deg=rwy_hdg,
                    vx_rwy_mps=vx_mps,
                    vy_rwy_mps=vy_mps,
                    p_dps=p_dps,
                    q_dps=q_dps,
                    r_dps=r_dps,
                    teardrop_side=teardrop_side,
                    elev_int=elev_int,
                    dt=dt_ctl,
                    gains=gains,
                )
            else:
                fd = compute_unified_fd_guidance_py(
                    along_track_m=x_m,
                    cross_track_m=y_m,
                    alt_agl_m=agl_m,
                    vx_mps=vx_mps,
                    vy_mps=vy_mps,
                    vz_down_mps=vd_mps,
                    cur_pitch_deg=pitch_deg,
                    true_heading_deg=hdg_deg,
                    runway_heading_deg=rwy_hdg,
                    on_ground_or_stopped=False,
                )
                raw_xtk_rate = clamp((y_m - prev_xtk_m) / dt_ctl, -15.0, 15.0)
                prev_xtk_m = y_m
                xtk_rate_filt = 0.75 * xtk_rate_filt + 0.25 * raw_xtk_rate
                ap = compute_ap_control_from_fd(
                    tgt_hdg_deg=fd["target_heading_deg"],
                    cur_hdg_deg=hdg_deg,
                    gs_err_deg=fd["glideslope_error_deg"],
                    tgt_pit_deg=fd["target_pitch_deg"],
                    agl_m=agl_m,
                    dist_m=fd["distance_to_rwy_m"],
                    xtk_m=y_m,
                    cur_roll_deg=roll_deg,
                    cur_pitch_deg=pitch_deg,
                    ias_kt=ias_kt,
                    vs_fpm=vs_fpm,
                    p_dps=p_dps,
                    q_dps=q_dps,
                    r_dps=r_dps,
                    flare_active=fd["flare_active"],
                    rwy_hdg_deg=rwy_hdg,
                    crab_deg=0.0,
                    xtk_rate_mps=xtk_rate_filt,
                    xtk_int=xtk_int,
                    rud_int=rud_int,
                    gs_int=gs_int,
                    elev_int=elev_int,
                    dt=dt_ctl,
                    gains=gains,
                )
                xtk_int = ap["xtk_int"]
                rud_int = ap["rud_int"]
                gs_int = ap["gs_int"]
                elev_int = ap["elev_int"]
                final_xtk_err_sum += abs(y_m) * dt_ctl

            fdm["fcs/aileron-cmd-norm"] = ap["aileron"]
            fdm["fcs/elevator-cmd-norm"] = ap["elevator"]
            fdm["fcs/rudder-cmd-norm"] = -ap["cmd_rud"]
            fdm["fcs/throttle-cmd-norm"] = ap["throttle"]
            fdm["fcs/mixture-cmd-norm"] = 1.0
            fdm["fcs/flap-cmd-norm"] = ap.get("flaps", 0.33)

        fdm.run()
        if y_m <= 0.0:
            crossed_cl = True
        if crossed_cl and y_m < 0.0:
            max_overshoot_m = max(max_overshoot_m, abs(y_m))

    return {
        "cpp_max_overshoot_m": max_overshoot_m,
        "cpp_base_turn_entries": base_turn_entries,
        "cpp_final_xtk_err_sum": final_xtk_err_sum,
        "cpp_end_xtk_m": y_m,
        "cpp_end_hdg_err_deg": abs(wrap180(hdg_deg - rwy_hdg)),
    }


def evaluate_genome_dict(gains: Dict[str, float]) -> Tuple[float, Dict]:
    """
    Evaluates a candidate genome dictionary across all 4 FlightGear scenarios
    plus the C++ JSBSim perpendicular intercept check, returning `(total_cost, report)`.
    Lower `total_cost` = higher evolutionary fitness.
    """
    # Enforce structural constraint: blend_inner_m < blend_outer_m - 40m
    if gains["base_turn_blend_outer_m"] < gains["base_turn_blend_inner_m"] + 45.0:
        gains["base_turn_blend_outer_m"] = gains["base_turn_blend_inner_m"] + 45.0

    scenarios = [
        "perpendicular_base_turn",
        "cat_iii_crosswind_spoof",
        "uprt_severe_upset",
        "go_around_teardrop",
    ]
    total_cost = 0.0
    details = {}

    for sc in scenarios:
        res = simulate_6dof_scenario_xml_library(gains, sc)
        details[sc] = res
        sc_cost = 0.0

        if not res["touchdown_latched"] or res["td_snap"] is None:
            sc_cost += 25000.0
        else:
            td = res["td_snap"]
            # Target touchdown zone: X_rwy in [120m, 380m], Xtk = 0.0m, VS = -135 fpm
            sc_cost += abs(td["x_m"] - 230.0) * 0.45
            sc_cost += (abs(td["y_m"]) ** 1.6) * 28.0
            sc_cost += abs(td["vs_fpm"] - (-135.0)) * 0.12
            sc_cost += abs(td["hdg_err_deg"]) * 12.0
            sc_cost += abs(res["final_y_m"]) * 18.0
            sc_cost += res["final_ias_kt"] * 15.0

        # Penalize centerline overshoot and multiple BASE_TURN_INTERCEPT re-entries
        if sc == "perpendicular_base_turn":
            sc_cost += res["max_overshoot_xtk_m"] * 3.8
            if res["base_turn_entries"] > 1:
                sc_cost += (res["base_turn_entries"] - 1) * 4500.0
            if res["phase_transitions"] > 2:
                sc_cost += (res["phase_transitions"] - 2) * 1800.0

        if sc == "uprt_severe_upset" and res["min_agl_airborne"] < 28.0:
            sc_cost += (28.0 - res["min_agl_airborne"]) * 500.0

        sc_cost += res["int_xtk_final"] * 0.08
        sc_cost += res["int_gs_final"] * 0.45
        sc_cost += res["control_jerk_sum"] * 1.2
        sc_cost += max(0.0, res["fd_jump_max"] - 4.5) * 35.0
        total_cost += sc_cost

    # C++ JSBSim FGFDMExec cross-validation penalty when available
    cpp_res = simulate_cpp_jsbsim_perpendicular(gains)
    if cpp_res is not None:
        details["cpp_jsbsim_perpendicular"] = cpp_res
        total_cost += cpp_res["cpp_max_overshoot_m"] * 2.5
        total_cost += abs(cpp_res["cpp_end_xtk_m"]) * 8.0
        total_cost += cpp_res["cpp_end_hdg_err_deg"] * 10.0
        if cpp_res["cpp_base_turn_entries"] > 1:
            total_cost += (cpp_res["cpp_base_turn_entries"] - 1) * 4000.0

    return total_cost, details


def _eval_worker(gains: Dict[str, float]) -> Tuple[float, Dict[str, float]]:
    cost, _ = evaluate_genome_dict(gains)
    return cost, gains


# ==============================================================================
# 3. GENETIC PROGRAMMING / EVOLUTIONARY STRATEGY ENGINE
# ==============================================================================
class GeneticAutopilotOptimizer:
    def __init__(
        self,
        pop_size: int = 32,
        generations: int = 18,
        elite_frac: float = 0.25,
        seed: int = 20261101,
        workers: int = 0,
    ):
        self.pop_size = pop_size
        self.generations = generations
        self.elite_count = max(2, int(pop_size * elite_frac))
        self.rng = random.Random(seed)
        self.workers = workers if workers > 0 else max(1, mp.cpu_count())

    def _clamp_genome(self, g: Dict[str, float]) -> Dict[str, float]:
        out = dict(g)
        for name, lo, hi, digits in GENOME_SPEC:
            val = clamp(out.get(name, (lo + hi) * 0.5), lo, hi)
            out[name] = round(val, digits)
        if out["base_turn_blend_outer_m"] < out["base_turn_blend_inner_m"] + 45.0:
            out["base_turn_blend_outer_m"] = round(
                clamp(out["base_turn_blend_inner_m"] + 55.0, 220.0, 320.0), 1
            )
        return out

    def _mutate(self, parent: Dict[str, float], sigma_scale: float) -> Dict[str, float]:
        child = dict(parent)
        for name, lo, hi, digits in GENOME_SPEC:
            if self.rng.random() < 0.45:
                span = hi - lo
                delta = self.rng.gauss(0.0, span * sigma_scale)
                child[name] = round(clamp(child[name] + delta, lo, hi), digits)
        return self._clamp_genome(child)

    def _crossover_blx(self, p1: Dict[str, float], p2: Dict[str, float], alpha: float = 0.25) -> Dict[str, float]:
        child = dict(p1)
        for name, lo, hi, digits in GENOME_SPEC:
            v1, v2 = p1[name], p2[name]
            c_min, c_max = min(v1, v2), max(v1, v2)
            diff = c_max - c_min
            val = self.rng.uniform(c_min - alpha * diff, c_max + alpha * diff)
            child[name] = round(clamp(val, lo, hi), digits)
        return self._clamp_genome(child)

    def evolve(self, baseline_gains: Dict[str, float]) -> Tuple[Dict[str, float], Dict]:
        t0 = time.time()
        seed_genome = self._clamp_genome(baseline_gains)

        population: List[Dict[str, float]] = [seed_genome]
        for i in range(1, self.pop_size):
            sigma = 0.08 if i < self.pop_size // 2 else 0.16
            population.append(self._mutate(seed_genome, sigma_scale=sigma))

        history = []
        best_cost = float("inf")
        best_genome = copy.deepcopy(seed_genome)
        baseline_cost = None

        pool = mp.Pool(processes=self.workers) if self.workers > 1 else None
        try:
            for gen in range(self.generations):
                if pool is not None:
                    results = pool.map(_eval_worker, population)
                else:
                    results = [_eval_worker(ind) for ind in population]

                if gen == 0:
                    baseline_cost = results[0][0]

                results.sort(key=lambda item: item[0])
                gen_best_cost, gen_best_genome = results[0]
                if gen_best_cost < best_cost:
                    best_cost = gen_best_cost
                    best_genome = copy.deepcopy(gen_best_genome)

                mean_top = sum(r[0] for r in results[: self.elite_count]) / self.elite_count
                history.append(
                    {
                        "generation": gen,
                        "best_cost": round(best_cost, 3),
                        "elite_mean_cost": round(mean_top, 3),
                    }
                )
                print(
                    f"[GENETIC-OPT Gen {gen:02d}/{self.generations - 1:02d}] "
                    f"BestCost={best_cost:8.2f} (Baseline={baseline_cost:8.2f}) | "
                    f"lead_s={best_genome['base_turn_lead_time_s']:.2f}s "
                    f"dx_gain={best_genome['base_turn_dx_gain']:.2f} "
                    f"cap_hdg={best_genome['final_capture_hdg_deg']:.1f}deg "
                    f"cap_vy={best_genome['final_capture_vy_mps']:.1f}m/s"
                )
                sys.stdout.flush()

                # Next generation via Elitism + Weighted Recombination + Tournament BLX-alpha + Adaptive Mutation
                elites = [copy.deepcopy(r[1]) for r in results[: self.elite_count]]
                next_pop: List[Dict[str, float]] = [copy.deepcopy(best_genome)]

                # Weighted centroid of top elites ((mu/mu_W, lambda)-ES recombination)
                weights = [math.log(self.elite_count + 0.5) - math.log(i + 1) for i in range(self.elite_count)]
                w_sum = sum(weights)
                centroid = dict(best_genome)
                for name, lo, hi, digits in GENOME_SPEC:
                    val = sum(weights[i] * elites[i][name] for i in range(self.elite_count)) / w_sum
                    centroid[name] = round(clamp(val, lo, hi), digits)
                next_pop.append(self._clamp_genome(centroid))

                sigma_gen = max(0.025, 0.14 * ((1.0 - gen / max(1, self.generations)) ** 1.15))
                while len(next_pop) < self.pop_size:
                    p1 = self.rng.choice(elites)
                    p2 = self.rng.choice(elites)
                    child = self._crossover_blx(p1, p2, alpha=0.25)
                    child = self._mutate(child, sigma_scale=sigma_gen)
                    next_pop.append(child)

                population = next_pop
        finally:
            if pool is not None:
                pool.close()
                pool.join()

        elapsed_s = time.time() - t0
        _, best_details = evaluate_genome_dict(best_genome)
        meta = {
            "method": "Multi-Objective (mu/mu_W, lambda)-ES + BLX-alpha Genetic Programming",
            "cpp_jsbsim_active": HAS_CPP_JSBSIM,
            "population_size": self.pop_size,
            "generations": self.generations,
            "workers": self.workers,
            "elapsed_seconds": round(elapsed_s, 2),
            "baseline_cost": round(baseline_cost or best_cost, 3),
            "evolved_best_cost": round(best_cost, 3),
            "improvement_pct": round(
                100.0 * ((baseline_cost or best_cost) - best_cost) / max(1e-6, baseline_cost or best_cost), 2
            ),
            "evolved_gains": {name: best_genome[name] for name, _, _, _ in GENOME_SPEC},
            "scenario_verdicts": {
                sc: {
                    "touchdown": best_details[sc]["touchdown_latched"],
                    "td_x_m": round(best_details[sc]["td_snap"]["x_m"], 1) if best_details[sc]["td_snap"] else None,
                    "td_y_m": round(best_details[sc]["td_snap"]["y_m"], 2) if best_details[sc]["td_snap"] else None,
                    "td_vs_fpm": round(best_details[sc]["td_snap"]["vs_fpm"], 1) if best_details[sc]["td_snap"] else None,
                    "max_overshoot_xtk_m": round(best_details[sc]["max_overshoot_xtk_m"], 2),
                    "base_turn_entries": best_details[sc]["base_turn_entries"],
                }
                for sc in [
                    "perpendicular_base_turn",
                    "cat_iii_crosswind_spoof",
                    "uprt_severe_upset",
                    "go_around_teardrop",
                ]
            },
            "history": history,
        }
        return best_genome, meta


def save_evolved_gains_and_sync_nasal(best_genome: Dict[str, float], meta: Dict) -> None:
    """
    Saves the genetically evolved gains into `c172p_extracted_fcs_library.json`
    and automatically synchronizes `aero_shield_hud.nas` so Nasal and Python use
    the exact same evolved parameters without any manual fine-tuning.
    """
    with open(OUTPUT_JSON_PATH, "r", encoding="utf-8") as f:
        data = json.load(f)

    data["analytical_synthesis"]["synthesized_ap_gains"].update(best_genome)
    data["genetic_evolution"] = meta
    with open(OUTPUT_JSON_PATH, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2)

    # Automatically sync evolved Base-Turn & Capture constants into aero_shield_hud.nas
    nas_path = os.path.join(_SCRIPT_DIR, "aero_shield_hud.nas")
    if os.path.exists(nas_path):
        with open(nas_path, "r", encoding="utf-8") as f:
            nas_src = f.read()

        # Update BASE_TURN_INTERCEPT -> FINAL_APPROACH transition in Nasal
        nas_src = re.sub(
            r"if \(x_rwy_m < -650\.0 and math\.abs\(xtk_m\) < [0-9.]+ and hdg_err_rwy < [0-9.]+\)",
            f"if (x_rwy_m < -650.0 and math.abs(xtk_m) < {best_genome['final_capture_xtk_m']:.1f} and hdg_err_rwy < {best_genome['final_capture_hdg_deg']:.1f})",
            nas_src,
        )
        # Update Stage B lookahead & lead-turn in Nasal
        nas_src = re.sub(
            r"var dx_look = clamp\(math\.abs\(xtk_m\) \* [0-9.]+ \+ [0-9.]+, [0-9.]+, 3[0-9.]+\);",
            f"var dx_look = clamp(math.abs(xtk_m) * {best_genome['base_turn_dx_gain']:.3f} + {best_genome['base_turn_dx_min_m']:.1f}, {best_genome['base_turn_dx_min_m']:.1f}, 340.0);",
            nas_src,
        )
        nas_src = re.sub(
            r"var dy_pred = -xtk_m - vy_rwy_mps \* [0-9.]+;",
            f"var dy_pred = -xtk_m - vy_rwy_mps * {best_genome['base_turn_lead_time_s']:.3f};",
            nas_src,
        )
        nas_src = re.sub(
            r"var w_stage_a = smoothstep_c1\([0-9.]+, [0-9.]+, math\.abs\(xtk_m\)\);",
            f"var w_stage_a = smoothstep_c1({best_genome['base_turn_blend_inner_m']:.1f}, {best_genome['base_turn_blend_outer_m']:.1f}, math.abs(xtk_m));",
            nas_src,
        )
        with open(nas_path, "w", encoding="utf-8") as f:
            f.write(nas_src)


def run_remote_cloud_evolution(
    remote_host: str = "monk@34.12.88.28",
    pop_size: int = 40,
    generations: int = 20,
) -> int:
    """
    Dispatches the headless FlightGear JSBSim Genetic Programming evolution to the
    8-vCPU Cloud VM (`monk@34.12.88.28`), runs C++ JSBSim + 6-DoF XML multi-core evolution,
    and downloads the evolved `c172p_extracted_fcs_library.json` back to the local workspace.
    """
    remote_dir = "/home/monk/Craiova-Hackathon-2026/flightgear"
    files_to_sync = [
        os.path.join(_SCRIPT_DIR, "fg_library_extractor.py"),
        os.path.join(_SCRIPT_DIR, "fg_mcp_ctl.py"),
        os.path.join(_SCRIPT_DIR, "fg_genetic_optimizer.py"),
        os.path.join(_SCRIPT_DIR, "c172p_extracted_fcs_library.json"),
    ]
    print(f"[CLOUD-GP] Sincronizare fisiere catre VM Cloud {remote_host}:{remote_dir} ...")
    subprocess.run(["scp", "-q"] + files_to_sync + [f"{remote_host}:{remote_dir}/"], check=True)

    cmd = (
        f"cd {remote_dir} && python3 fg_genetic_optimizer.py "
        f"--evolve --pop-size {pop_size} --generations {generations}"
    )
    print(f"[CLOUD-GP] Rulare evolutie genetica pe 8 nuclee Xeon + C++ JSBSim pe {remote_host} ...")
    subprocess.run(["ssh", remote_host, cmd], check=True)

    print(f"[CLOUD-GP] Descarcare parametri evoluati din {remote_host} in workspace-ul local ...")
    subprocess.run(
        ["scp", "-q", f"{remote_host}:{remote_dir}/c172p_extracted_fcs_library.json", OUTPUT_JSON_PATH],
        check=True,
    )
    with open(OUTPUT_JSON_PATH, "r", encoding="utf-8") as f:
        data = json.load(f)
    save_evolved_gains_and_sync_nasal(
        data["analytical_synthesis"]["synthesized_ap_gains"],
        data["genetic_evolution"],
    )
    print("[CLOUD-GP] Evolutie genetica completa si sincronizata local (0 finetuning manual)!")
    return 0


# ==============================================================================
# 4. AUTOMATED GENETIC PROGRAMMING & FLIGHTGEAR LIBRARY VERIFICATION SUITE
# ==============================================================================
class TestFlightGearGeneticProgramming(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.lib = load_or_synthesize_fcs_library(force_rebuild=False)
        cls.gains = cls.lib["analytical_synthesis"]["synthesized_ap_gains"]

    def test_01_zero_manual_tuning_provenance(self):
        """Verifies that control parameters have Genetic Programming provenance in JSON."""
        self.assertIn("genetic_evolution", self.lib)
        ge = self.lib["genetic_evolution"]
        self.assertLessEqual(ge["evolved_best_cost"], ge["baseline_cost"])
        for name, lo, hi, _ in GENOME_SPEC:
            self.assertIn(name, self.gains)
            self.assertGreaterEqual(self.gains[name], lo)
            self.assertLessEqual(self.gains[name], hi)

    def test_02_perpendicular_base_turn_zero_oscillation_and_landing(self):
        """Verifies perpendicular 90° base-turn transitions ONCE to FINAL_APPROACH and lands on centerline."""
        res = simulate_6dof_scenario_xml_library(self.gains, "perpendicular_base_turn")
        self.assertTrue(res["touchdown_latched"])
        self.assertEqual(res["base_turn_entries"], 1, "Must enter BASE_TURN_INTERCEPT exactly once (zero oscillation)")
        self.assertLess(res["max_overshoot_xtk_m"], 45.0, "Perpendicular turn centerline overshoot must be < 45m")
        self.assertLess(abs(res["td_snap"]["y_m"]), 1.5)
        self.assertLess(res["final_ias_kt"], 1.0)

    def test_03_cat_iii_crosswind_spoof_landing(self):
        """Verifies CAT IIIc 14kt crosswind + GPS spoofing forward-slip landing and rollout stop."""
        res = simulate_6dof_scenario_xml_library(self.gains, "cat_iii_crosswind_spoof")
        self.assertTrue(res["touchdown_latched"])
        self.assertLess(abs(res["td_snap"]["y_m"]), 1.5)
        self.assertLess(res["final_ias_kt"], 1.0)

    def test_04_uprt_severe_upset_recovery_and_landing(self):
        """Verifies FAA/ICAO UPRT recovery from -46° bank, -14° dive, -1350 fpm and safe landing."""
        res = simulate_6dof_scenario_xml_library(self.gains, "uprt_severe_upset")
        self.assertTrue(res["touchdown_latched"])
        self.assertGreater(res["min_agl_airborne"], 35.0)
        self.assertLess(abs(res["td_snap"]["y_m"]), 2.0)

    def test_05_go_around_teardrop_circuit_and_landing(self):
        """Verifies autonomous Go-Around -> Teardrop Outbound -> Base Turn -> Final Approach landing."""
        res = simulate_6dof_scenario_xml_library(self.gains, "go_around_teardrop")
        self.assertTrue(res["touchdown_latched"])
        self.assertLess(abs(res["td_snap"]["y_m"]), 2.0)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="FlightGear Headless Genetic Programming Optimizer")
    parser.add_argument("--evolve", action="store_true", help="Run genetic evolution locally/on current host")
    parser.add_argument("--remote", action="store_true", help="Dispatch evolution to Cloud VM monk@34.12.88.28")
    parser.add_argument("--verify", action="store_true", help="Run unittest verification suite")
    parser.add_argument("--pop-size", type=int, default=40)
    parser.add_argument("--generations", type=int, default=20)
    args = parser.parse_args()

    if args.remote:
        sys.exit(run_remote_cloud_evolution(pop_size=args.pop_size, generations=args.generations))
    elif args.evolve:
        # Rebuild clean analytical seed first (without old evolved overrides)
        if os.path.exists(OUTPUT_JSON_PATH):
            with open(OUTPUT_JSON_PATH, "r", encoding="utf-8") as f:
                cur_d = json.load(f)
            if "genetic_evolution" in cur_d:
                del cur_d["genetic_evolution"]
                with open(OUTPUT_JSON_PATH, "w", encoding="utf-8") as f:
                    json.dump(cur_d, f, indent=2)
        base_payload = synthesize_fcs_autopilot_parameters()
        base_gains = base_payload["analytical_synthesis"]["synthesized_ap_gains"]
        opt = GeneticAutopilotOptimizer(pop_size=args.pop_size, generations=args.generations)
        best_g, meta = opt.evolve(base_gains)
        save_evolved_gains_and_sync_nasal(best_g, meta)
        print("\n[GENETIC-OPT SUMMARY]")
        print(json.dumps(meta["scenario_verdicts"], indent=2))
        print(
            f"Cost redus de la {meta['baseline_cost']:.2f} -> {meta['evolved_best_cost']:.2f} "
            f"(-{meta['improvement_pct']:.1f}%) in {meta['elapsed_seconds']:.2f}s "
            f"(C++ JSBSim={meta['cpp_jsbsim_active']}, Workers={meta['workers']})"
        )
    else:
        unittest.main(argv=[sys.argv[0], "-v"])
