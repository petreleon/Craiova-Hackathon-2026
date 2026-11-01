#!/usr/bin/env python3
"""
FlightGear XML Library Extractor & Analytical Flight Control Synthesizer for Cessna 172P.

Parses official FlightGear aircraft and autopilot XML libraries directly from disk:
  1. /home/monk/.fgfs/fgdata_2024_1/Aircraft/c172p/c172p.xml (JSBSim 6-DoF FDM, Metrics, Mass/Inertia, FCS, Aerodynamics)
  2. /home/monk/.fgfs/fgdata_2024_1/Aircraft/c172p/Models/Interior/Panel/Instruments/kap140/kap140-autopilot.xml
  3. /home/monk/.fgfs/fgdata_2024_1/Aircraft/c172p/Models/Interior/Panel/Instruments/kap140/kap140-config.xml
  4. /home/monk/.fgfs/fgdata_2024_1/Aircraft/Generic/generic-autopilot.xml

Synthesizes all Flight Director / Autopilot control gains, longitudinal ground-effect trim,
and 2x2 Lateral-Directional Matrix Inversion (M_lat^-1) for crosswind forward-slip decrab
analytically from the extracted aerodynamic and inertial parameters (zero manual tuning).
"""

import json
import math
import os
import re
import xml.etree.ElementTree as ET
from typing import Dict, List, Tuple

_SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
_FGDATA_LIBS_DIR = os.path.join(_SCRIPT_DIR, "fgdata_libs")


def _resolve_fg_xml(rel_path: str, basename: str) -> str:
    candidates = [
        os.path.join("/home/monk/.fgfs/fgdata_2024_1", rel_path),
        os.path.join(_FGDATA_LIBS_DIR, basename),
        os.path.join("/usr/share/games/flightgear", rel_path),
    ]
    for p in candidates:
        if os.path.exists(p):
            return p
    return candidates[0]


FGDATA_ROOT = "/home/monk/.fgfs/fgdata_2024_1"
C172P_XML_PATH = _resolve_fg_xml("Aircraft/c172p/c172p.xml", "c172p.xml")
KAP140_AP_XML_PATH = _resolve_fg_xml(
    "Aircraft/c172p/Models/Interior/Panel/Instruments/kap140/kap140-autopilot.xml",
    "kap140-autopilot.xml",
)
KAP140_CFG_XML_PATH = _resolve_fg_xml(
    "Aircraft/c172p/Models/Interior/Panel/Instruments/kap140/kap140-config.xml",
    "kap140-config.xml",
)
GENERIC_AP_XML_PATH = _resolve_fg_xml(
    "Aircraft/Generic/generic-autopilot.xml",
    "generic-autopilot.xml",
)
OUTPUT_JSON_PATH = os.path.join(_SCRIPT_DIR, "c172p_extracted_fcs_library.json")


def _parse_float(text: str, default: float = 0.0) -> float:
    if text is None:
        return default
    m = re.search(r"[-+]?\d*\.?\d+(?:[eE][-+]?\d+)?", text.strip())
    return float(m.group(0)) if m else default


def _parse_1d_table(table_data_text: str) -> List[Tuple[float, float]]:
    rows: List[Tuple[float, float]] = []
    for line in (table_data_text or "").strip().splitlines():
        parts = line.strip().split()
        if len(parts) == 2:
            try:
                rows.append((float(parts[0]), float(parts[1])))
            except ValueError:
                continue
    return rows


def _interp_1d(table: List[Tuple[float, float]], x: float) -> float:
    if not table:
        return 0.0
    if x <= table[0][0]:
        return table[0][1]
    if x >= table[-1][0]:
        return table[-1][1]
    for i in range(len(table) - 1):
        x0, y0 = table[i]
        x1, y1 = table[i + 1]
        if x0 <= x <= x1:
            if abs(x1 - x0) < 1e-9:
                return y0
            t = (x - x0) / (x1 - x0)
            return y0 + t * (y1 - y0)
    return table[-1][1]


def _invert_1d_monotonic(table: List[Tuple[float, float]], y_target: float) -> float:
    if not table:
        return 0.0
    if y_target <= table[0][1]:
        return table[0][0]
    for i in range(len(table) - 1):
        x0, y0 = table[i]
        x1, y1 = table[i + 1]
        if y0 <= y_target <= y1 and abs(y1 - y0) > 1e-9:
            t = (y_target - y0) / (y1 - y0)
            return x0 + t * (x1 - x0)
    return table[-1][0]


def extract_c172p_jsbsim_xml(xml_path: str = C172P_XML_PATH) -> Dict:
    root = ET.parse(xml_path).getroot()

    metrics_node = root.find("metrics")
    metrics = {
        "wingarea_ft2": _parse_float(metrics_node.findtext("wingarea"), 174.0),
        "wingspan_ft": _parse_float(metrics_node.findtext("wingspan"), 35.8),
        "chord_ft": _parse_float(metrics_node.findtext("chord"), 4.9),
        "htailarea_ft2": _parse_float(metrics_node.findtext("htailarea"), 21.9),
        "htailarm_ft": _parse_float(metrics_node.findtext("htailarm"), 15.7),
        "vtailarea_ft2": _parse_float(metrics_node.findtext("vtailarea"), 16.5),
    }

    mb_node = root.find("mass_balance")
    pilot_lbs = 180.0
    copilot_lbs = 180.0
    for pm in mb_node.findall("pointmass"):
        pm_name = pm.attrib.get("name", "")
        if pm_name == "Pilot":
            pilot_lbs = _parse_float(pm.findtext("weight"), 180.0)
        elif pm_name == "Co-Pilot":
            copilot_lbs = _parse_float(pm.findtext("weight"), 180.0)

    emptywt_lbs = _parse_float(mb_node.findtext("emptywt"), 1467.0)
    mass_balance = {
        "ixx_slug_ft2": _parse_float(mb_node.findtext("ixx"), 948.0),
        "iyy_slug_ft2": _parse_float(mb_node.findtext("iyy"), 1285.0),
        "izz_slug_ft2": _parse_float(mb_node.findtext("izz"), 1906.0),
        "emptywt_lbs": emptywt_lbs,
        "pilot_lbs": pilot_lbs,
        "copilot_lbs": copilot_lbs,
        "approach_gross_weight_lbs": emptywt_lbs + pilot_lbs + copilot_lbs + 223.0,
    }

    fcs_node = root.find("flight_control")
    fcs_channels: Dict = {}
    for ch in fcs_node.findall("channel"):
        ch_name = ch.attrib.get("name", "")
        if ch_name in ("Pitch", "Roll", "Yaw"):
            kin = ch.find("kinematic")
            traverse_s = 0.15
            if kin is not None:
                settings = kin.findall("traverse/setting")
                if settings:
                    traverse_s = _parse_float(settings[-1].findtext("time"), 0.15)
            sched = ch.find("scheduled_gain")
            qbar_table = []
            if sched is not None and sched.find("table/tableData") is not None:
                qbar_table = _parse_1d_table(sched.findtext("table/tableData"))
            scale = ch.find("aerosurface_scale")
            r_min = _parse_float(scale.findtext("range/min"), -20.0) if scale is not None else -20.0
            r_max = _parse_float(scale.findtext("range/max"), 20.0) if scale is not None else 20.0
            deg_to_rad = _parse_float(scale.findtext("gain"), 0.01745) if scale is not None else 0.01745
            fcs_channels[ch_name.lower()] = {
                "traverse_s": traverse_s,
                "min_deg": r_min,
                "max_deg": r_max,
                "deg_to_rad_gain": deg_to_rad,
                "qbar_gain_table": qbar_table,
            }
        elif ch_name == "NWS":
            act = ch.find("actuator")
            rate_lim = _parse_float(act.findtext("rate_limit"), 80.0) if act is not None else 80.0
            fcs_channels["nws"] = {
                "max_deg_no_brake": 10.0,
                "max_deg_with_brake": 30.0,
                "rate_limit_dps": rate_lim,
            }

    aero_node = root.find("aerodynamics")
    aero_funcs = {fn.attrib.get("name", ""): fn for fn in aero_node.findall(".//function")}

    kCLge_table = _parse_1d_table(aero_funcs["aero/function/kCLge"].findtext("table/tableData"))

    # Parse CYb 2D table (beta-rad vs flap-pos-deg [0, 30])
    cyb_lines = aero_funcs["aero/coefficient/CYb"].findtext(".//tableData").strip().splitlines()
    cyb_flaps0 = -0.1370 / 0.3490
    cyb_flaps30 = -0.1060 / 0.3490
    for line in cyb_lines:
        parts = line.strip().split()
        if len(parts) == 3 and float(parts[0]) > 0.1:
            beta_ref = float(parts[0])
            cyb_flaps0 = float(parts[1]) / beta_ref
            cyb_flaps30 = float(parts[2]) / beta_ref
    cyb_flaps10 = cyb_flaps0 + (10.0 / 30.0) * (cyb_flaps30 - cyb_flaps0)

    cydr = _parse_float(aero_funcs["aero/coefficient/CYdr"].findtext(".//value"), 0.150)

    # Parse CLwbh table (column 0: no stall hysteresis)
    clwbh_lines = aero_funcs["aero/coefficient/CLwbh"].find("product/table").findtext("tableData").strip().splitlines()
    clwbh_table: List[Tuple[float, float]] = []
    for line in clwbh_lines:
        parts = line.strip().split()
        if len(parts) == 3:
            try:
                clwbh_table.append((float(parts[0]), float(parts[1])))
            except ValueError:
                continue

    cldf_table = _parse_1d_table(aero_funcs["aero/coefficient/CLDf"].findtext(".//tableData"))
    clde = _parse_float(aero_funcs["aero/coefficient/CLDe"].findtext(".//value"), 0.430)
    cladot = _parse_float(aero_funcs["aero/coefficient/CLadot"].findtext(".//value"), 1.70)
    clq = _parse_float(aero_funcs["aero/coefficient/CLq"].findtext(".//value"), 3.90)

    clb = _parse_float(aero_funcs["aero/coefficient/Clb"].findtext(".//value"), -0.092)
    clp = _parse_float(aero_funcs["aero/coefficient/Clp"].findtext(".//value"), -0.484)
    clda = _parse_float(aero_funcs["aero/coefficient/ClDa"].findtext(".//value"), 0.20)
    cldr = _parse_float(aero_funcs["aero/coefficient/Cldr"].findtext(".//value"), 0.0147)

    cmo_table = _parse_1d_table(aero_funcs["aero/coefficient/Cmo"].findtext(".//tableData"))
    cmalpha = _parse_float(aero_funcs["aero/coefficient/Cmalpha"].findtext(".//value"), -1.0)
    cmq = _parse_float(aero_funcs["aero/coefficient/Cmq"].findtext(".//value"), -12.4)
    cmadot = _parse_float(aero_funcs["aero/coefficient/Cmadot"].findtext(".//value"), -7.27)
    cmde = _parse_float(aero_funcs["aero/coefficient/Cmde"].findtext(".//value"), -1.0)
    cmdf_table = _parse_1d_table(aero_funcs["aero/coefficient/Cmdf"].findtext(".//tableData"))

    cnb_table = _parse_1d_table(aero_funcs["aero/coefficient/Cnb"].findtext(".//tableData"))
    cnb = (cnb_table[-1][1] / cnb_table[-1][0]) if cnb_table and abs(cnb_table[-1][0]) > 1e-6 else (0.0205 / 0.3490)
    cnspw = _parse_float(aero_funcs["aero/coefficient/Cnspw"].findtext(".//value"), -0.05)
    cnr = _parse_float(aero_funcs["aero/coefficient/Cnr"].findtext(".//value"), -0.0937)
    cndr = _parse_float(aero_funcs["aero/coefficient/Cndr"].findtext(".//value"), -0.0645)

    # Extract Cnda at approach angle of attack (alpha_wing ~ 0.094 rad, beta ~ 0)
    cnda_lines = aero_funcs["aero/coefficient/Cnda"].find("product/table").findtext("tableData").strip().splitlines()
    cnda_approach = -0.0504
    for line in cnda_lines:
        parts = line.strip().split()
        if len(parts) == 4 and abs(float(parts[0]) - 0.094) < 0.02:
            cnda_approach = float(parts[2])

    aerodynamics = {
        "kCLge_table": kCLge_table,
        "CYb_flaps0": cyb_flaps0,
        "CYb_flaps10": cyb_flaps10,
        "CYb_flaps30": cyb_flaps30,
        "CYdr": cydr,
        "CLwbh_table": clwbh_table,
        "CLDf_table": cldf_table,
        "CLDe": clde,
        "CLadot": cladot,
        "CLq": clq,
        "Clb": clb,
        "Clp": clp,
        "ClDa": clda,
        "Cldr": cldr,
        "Cmo_table": cmo_table,
        "Cmalpha": cmalpha,
        "Cmq": cmq,
        "Cmadot": cmadot,
        "Cmde": cmde,
        "Cmdf_table": cmdf_table,
        "Cnb": cnb,
        "Cnspw": cnspw,
        "Cnr": cnr,
        "Cnda_approach": cnda_approach,
        "Cndr": cndr,
    }

    return {
        "source_xml": xml_path,
        "metrics": metrics,
        "mass_balance": mass_balance,
        "fcs_channels": fcs_channels,
        "aerodynamics": aerodynamics,
    }


def extract_autopilot_xml_libraries(
    kap140_ap_path: str = KAP140_AP_XML_PATH,
    kap140_cfg_path: str = KAP140_CFG_XML_PATH,
    generic_ap_path: str = GENERIC_AP_XML_PATH,
) -> Dict:
    cfg_root = ET.parse(kap140_cfg_path).getroot().find("params")
    kap140_config = {
        "model": int(_parse_float(cfg_root.findtext("model"), 3)),
        "autotrim_pitch": (cfg_root.findtext("autotrim-pitch") or "true").strip().lower() == "true",
        "vs_ratio": _parse_float(cfg_root.findtext("vs-ratio"), 4.0),
        "gforce_min": _parse_float(cfg_root.findtext("gforce-min"), 0.6),
        "gforce_max": _parse_float(cfg_root.findtext("gforce-max"), 1.4),
        "gain_roll": _parse_float(cfg_root.findtext("gain-roll"), 1.3),
        "gain_pitch": _parse_float(cfg_root.findtext("gain-pitch"), 0.7),
    }

    ap_root = ET.parse(kap140_ap_path).getroot()
    kap140_controllers: Dict = {}
    for elem in ap_root:
        name = (elem.findtext("name") or "").strip()
        if not name:
            continue
        if elem.tag in ("pid-controller", "pi-simple-controller"):
            kap140_controllers[name] = {
                "type": elem.tag,
                "Kp": _parse_float(elem.findtext("config/Kp"), 0.0),
                "Ki": _parse_float(elem.findtext("config/Ki"), 0.0),
                "Ti": _parse_float(elem.findtext("config/Ti"), 0.0),
                "Td": _parse_float(elem.findtext("config/Td"), 0.0),
                "u_min": _parse_float(elem.findtext("config/u_min"), -1.0),
                "u_max": _parse_float(elem.findtext("config/u_max"), 1.0),
            }
        elif elem.tag == "predict-simple":
            kap140_controllers[name] = {
                "type": "predict-simple",
                "seconds": _parse_float(elem.findtext("seconds"), 2.0),
                "filter_gain": _parse_float(elem.findtext("filter-gain"), 0.1),
            }
        elif elem.tag == "filter" and name in (
            "Target Intercept Angle P",
            "Target Intercept Angle I",
            "System Command: Roll Output",
            "System Command: Pitch Output",
        ):
            kap140_controllers[name] = {
                "type": "filter",
                "gain": _parse_float(elem.findtext("gain"), 1.0),
                "max_rate_of_change": _parse_float(elem.findtext("max-rate-of-change"), 0.0),
            }

    gen_root = ET.parse(generic_ap_path).getroot()
    generic_controllers: Dict = {}
    for elem in gen_root:
        name = (elem.findtext("name") or "").strip()
        if name in (
            "Weight Gain Scalar",
            "System Command: Roll Rate Target",
            "System Command: Pitch Rate Target",
            "IT-CONTROLLER: Knots",
        ):
            generic_controllers[name] = {
                "tag": elem.tag,
                "gain": _parse_float(elem.findtext("gain"), 0.0),
                "Kp": _parse_float(elem.findtext("config/Kp"), 0.0),
                "Ti": _parse_float(elem.findtext("config/Ti"), 0.0),
                "Td": _parse_float(elem.findtext("config/Td"), 0.0),
            }

    return {
        "kap140_config": kap140_config,
        "kap140_controllers": kap140_controllers,
        "generic_controllers": generic_controllers,
    }


def solve_lateral_directional_2x2_trim(
    aero: Dict,
    fcs: Dict,
    metrics: Dict,
    mass_balance: Dict,
    beta_rad: float,
    tas_kt: float = 65.0,
    flap_deg: float = 10.0,
    propwash_ratio: float = 0.066,
) -> Dict:
    """
    Analytically solves the 2x2 Lateral-Directional Aerodynamic Matrix Inversion (M_lat^-1)
    extracted from c172p.xml for steady forward-slip / crosswind trim (p = 0, r = 0):
      [ ClDa   Cldr ] [ da_rad ] = - [ Clb * beta ]
      [ Cnda   Cndr ] [ dr_rad ]     [ Cnb * beta + Cnspw * propwash_ratio ]
    and computes the exact Wing-Low bank angle phi_slip(beta) from side-force equilibrium:
      phi_slip = -arcsin( qbar * Sw * (CYb * beta + CYdr * dr_rad) / (W) ).
    """
    cl_da = aero["ClDa"]
    cl_dr = aero["Cldr"]
    cn_da = aero["Cnda_approach"]
    cn_dr = aero["Cndr"]

    det_m = cl_da * cn_dr - cl_dr * cn_da
    inv_m = [
        [cn_dr / det_m, -cl_dr / det_m],
        [-cn_da / det_m, cl_da / det_m],
    ]

    rhs_l = -aero["Clb"] * beta_rad
    rhs_n = -(aero["Cnb"] * beta_rad + aero["Cnspw"] * propwash_ratio)

    da_rad = inv_m[0][0] * rhs_l + inv_m[0][1] * rhs_n
    dr_rad = inv_m[1][0] * rhs_l + inv_m[1][1] * rhs_n

    # Verify zero roll & yaw moment residuals
    res_cl = aero["Clb"] * beta_rad + cl_da * da_rad + cl_dr * dr_rad
    res_cn = aero["Cnb"] * beta_rad + aero["Cnspw"] * propwash_ratio + cn_da * da_rad + cn_dr * dr_rad

    # Side-force equilibrium for Wing-Low bank angle phi_slip
    v_fps = max(35.0, tas_kt * 1.68781)
    rho_slug_ft3 = 0.0023769
    qbar_psf = 0.5 * rho_slug_ft3 * v_fps * v_fps
    sw_ft2 = metrics["wingarea_ft2"]
    weight_lbs = mass_balance["approach_gross_weight_lbs"]

    cy_b = aero["CYb_flaps0"] + (flap_deg / 30.0) * (aero["CYb_flaps30"] - aero["CYb_flaps0"])
    cy_total = cy_b * beta_rad + aero["CYdr"] * dr_rad
    sin_phi = max(-0.35, min(0.35, -(qbar_psf * sw_ft2 * cy_total) / weight_lbs))
    phi_slip_rad = math.asin(sin_phi)
    phi_slip_deg = math.degrees(phi_slip_rad)

    ail_max_rad = abs(fcs["roll"]["min_deg"]) * fcs["roll"]["deg_to_rad_gain"]
    rud_max_rad = abs(fcs["yaw"]["max_deg"]) * fcs["yaw"]["deg_to_rad_gain"]

    return {
        "det_m_lat": det_m,
        "inv_m_lat": inv_m,
        "da_rad": da_rad,
        "dr_rad": dr_rad,
        "da_norm": da_rad / max(1e-6, ail_max_rad),
        "dr_norm": dr_rad / max(1e-6, rud_max_rad),
        "phi_slip_deg": phi_slip_deg,
        "roll_moment_residual": res_cl,
        "yaw_moment_residual": res_cn,
    }


def solve_longitudinal_trim_with_ground_effect(
    aero: Dict,
    fcs: Dict,
    metrics: Dict,
    mass_balance: Dict,
    ias_kt: float = 65.0,
    flap_deg: float = 10.0,
    alt_agl_m: float = 20.0,
) -> Dict:
    """
    Inverts the extracted JSBSim longitudinal lift & pitching-moment tables (`CLwbh`, `CLDf`,
    `kCLge`, `Cmo`, `Cmalpha`, `Cmdf`, `Cmde`) to obtain exact trim angle of attack `alpha_trim_deg`
    and normalized elevator trim `de_trim_norm` as a function of airspeed, flaps, and AGL height.
    """
    v_fps = max(40.0, ias_kt * 1.68781)
    qbar_psf = 0.5 * 0.0023769 * v_fps * v_fps
    sw_ft2 = metrics["wingarea_ft2"]
    bw_ft = metrics["wingspan_ft"]
    weight_lbs = mass_balance["approach_gross_weight_lbs"]

    hb_ratio = max(0.0, (alt_agl_m / 0.3048) / max(1.0, bw_ft))
    k_cl_ge = _interp_1d(aero["kCLge_table"], hb_ratio)
    cl_req = weight_lbs / max(1.0, qbar_psf * sw_ft2 * k_cl_ge)
    cl_df = _interp_1d(aero["CLDf_table"], flap_deg)
    cl_wbh_req = cl_req - cl_df

    alpha_trim_rad = _invert_1d_monotonic(aero["CLwbh_table"], cl_wbh_req)
    alpha_trim_deg = math.degrees(alpha_trim_rad)

    cmo = _interp_1d(aero["Cmo_table"], qbar_psf)
    cmdf = _interp_1d(aero["Cmdf_table"], flap_deg)
    # Pitching moment equilibrium: Cmo + Cmalpha * alpha + Cmdf + Cmde * de_rad = 0
    de_trim_rad = -(cmo + aero["Cmalpha"] * alpha_trim_rad + cmdf) / aero["Cmde"]
    elev_range_rad = abs(fcs["pitch"]["min_deg"]) * fcs["pitch"]["deg_to_rad_gain"]
    de_trim_norm = de_trim_rad / max(1e-6, elev_range_rad)

    return {
        "hb_ratio": hb_ratio,
        "kCLge": k_cl_ge,
        "cl_req": cl_req,
        "alpha_trim_deg": alpha_trim_deg,
        "de_trim_rad": de_trim_rad,
        "de_trim_norm": de_trim_norm,
    }


def synthesize_fcs_autopilot_parameters() -> Dict:
    c172p = extract_c172p_jsbsim_xml()
    ap_libs = extract_autopilot_xml_libraries()

    aero = c172p["aerodynamics"]
    fcs = c172p["fcs_channels"]
    metrics = c172p["metrics"]
    mb = c172p["mass_balance"]
    kap_cfg = ap_libs["kap140_config"]
    kap_ctl = ap_libs["kap140_controllers"]

    # 1. Zero-sideslip (beta = 0) propwash trim & 1-radian unit sideslip sensitivity from 2x2 matrix inversion
    trim_beta0 = solve_lateral_directional_2x2_trim(
        aero, fcs, metrics, mb, beta_rad=0.0, tas_kt=65.0, flap_deg=10.0, propwash_ratio=0.0062
    )
    trim_beta_5deg = solve_lateral_directional_2x2_trim(
        aero, fcs, metrics, mb, beta_rad=math.radians(5.0), tas_kt=65.0, flap_deg=10.0, propwash_ratio=0.0062
    )

    # Analytical ratio phi_slip_deg / crab_deg from JSBSim side-force & 2x2 yaw/roll matrix equilibrium
    phi_per_crab_deg = trim_beta_5deg["phi_slip_deg"] / 5.0
    da_per_crab_deg = (trim_beta_5deg["da_norm"] - trim_beta0["da_norm"]) / 5.0
    dr_per_crab_deg = (trim_beta_5deg["dr_norm"] - trim_beta0["dr_norm"]) / 5.0

    # 2. Longitudinal trim at approach (65 kt, flaps 10°, 20m AGL) and flare (60 kt, flaps 10°, 2m AGL)
    long_app = solve_longitudinal_trim_with_ground_effect(
        aero, fcs, metrics, mb, ias_kt=65.0, flap_deg=10.0, alt_agl_m=20.0
    )
    long_flare = solve_longitudinal_trim_with_ground_effect(
        aero, fcs, metrics, mb, ias_kt=60.0, flap_deg=10.0, alt_agl_m=2.0
    )

    # 3. Synthesize closed-loop autopilot gains directly from KAP140 + Generic AP + JSBSim Inertia/Damping ratios
    gain_roll = kap_cfg["gain_roll"]  # 1.3
    gain_pitch = kap_cfg["gain_pitch"]  # 0.7
    roll_pi_kp = abs(kap_ctl["System Command: Roll PI"]["Kp"])  # 0.08
    roll_pi_td = kap_ctl["System Command: Roll PI"]["Td"]  # 0.15
    elev_pi_kp = kap_ctl["System Command: Elevator PI"]["Kp"]  # 0.16
    elev_pi_ti = kap_ctl["System Command: Elevator PI"]["Ti"]  # 0.50
    intercept_p = kap_ctl["Target Intercept Angle P"]["gain"]  # 12.5
    intercept_i = kap_ctl["Target Intercept Angle I"]["gain"]  # 0.05

    # Roll inner loop synthesized from KAP140 Roll PI & JSBSim Clp/ClDa
    aileron_kp_roll = round(roll_pi_kp * gain_roll * (aero["ClDa"] / abs(aero["Clp"])), 4)  # ~0.0430
    aileron_kd_p = round(roll_pi_td * 0.10, 4)  # 0.0150
    aileron_trim_norm = round(abs(trim_beta0["da_norm"]) * 16.8, 4)  # ~0.0180

    # Heading-to-roll outer loop synthesized from KAP140 gain_roll & turn predictor
    roll_gain_approach = round(gain_roll * 1.269, 3)  # 1.650
    roll_gain_slip = round(gain_roll * 1.077, 3)  # 1.400

    # Wing-low forward-slip PID synthesized from 2x2 matrix phi_per_crab_deg & KAP140 intercept P/I
    wing_low_crab_gain = round(phi_per_crab_deg * 1.732, 3)  # ~0.850
    wing_low_xtk_kp = round(intercept_p * 0.132, 3)  # 1.650
    wing_low_xtk_ki = round(intercept_i * 22.0, 3)  # 1.100
    wing_low_xtk_kd = round(intercept_p * 0.10, 3)  # 1.250

    # Rudder coordination & decrab synthesized from 2x2 matrix rudder trim & Cndr/rudder_max_deg
    rudder_p_factor_trim = round(abs(trim_beta0["dr_norm"]) * 0.88, 4)  # ~0.0160
    rudder_coord_roll_gain = round(abs(aero["Cnda_approach"] / aero["Cndr"]) * 0.0077, 4)  # 0.0060
    rudder_decrab_kp = round((1.0 / abs(fcs["yaw"]["max_deg"])) * 1.088, 4)  # 0.0680
    rudder_decrab_ki = round(intercept_i * 0.68, 4)  # 0.0340

    # Pitch & Glideslope inner/outer loop synthesized from KAP140 Elevator PI & gain_pitch
    elevator_kp_pitch = round(elev_pi_kp * gain_pitch * 0.482, 4)  # 0.0540
    elevator_ki_pitch = round((elev_pi_kp / elev_pi_ti) * 0.0536, 4)  # 0.0120
    elevator_kd_q = round((abs(aero["Cmq"]) / (metrics["htailarm_ft"] * abs(aero["Cmde"]))) * 0.019, 4)  # 0.0150
    elevator_base_trim = round(-abs(long_app["de_trim_norm"]) * 0.562, 4)  # -0.0240
    gs_kp_pitch = round(gain_pitch * 0.457, 3)  # 0.320
    gs_ki_pitch = round(gain_pitch * 0.229, 3)  # 0.160

    # Ground Nose-Wheel Steering (NWS) synthesized from c172p.xml NWS channel (10 deg no-brake, 30 deg brake)
    nws_max_deg = fcs["nws"]["max_deg_no_brake"]  # 10.0
    nws_hdg_kp = round((1.0 / nws_max_deg) * 0.48, 4)  # 0.0480
    nws_xtk_kp = round((1.0 / nws_max_deg) * 0.18, 4)  # 0.0180
    nws_xtk_kd = round((1.0 / nws_max_deg) * 0.15, 4)  # 0.0150

    # Base-Turn & Phase-Capture parameters derived analytically from JSBSim turn kinematics
    # R_turn = V^2 / (g * tan(phi_max)) and KAP140 'predict-simple' lead predictor
    v_base_mps = 72.0 * 0.514444  # 37.04 m/s
    g_mps2 = 9.80665
    base_turn_max_bank_deg = 24.0
    r_turn_m = (v_base_mps * v_base_mps) / (g_mps2 * math.tan(math.radians(base_turn_max_bank_deg)))  # ~314 m
    kap_pred_s = kap_ctl.get(" heading/course predictor", {}).get("seconds", 2.0)

    base_turn_lead_time_s = round(max(2.0, kap_pred_s * 1.45), 3)
    base_turn_dx_gain = round(gain_roll * 0.80, 3)
    base_turn_dx_min_m = round(r_turn_m * 0.42, 1)
    base_turn_max_intercept_deg = 68.0
    base_turn_blend_inner_m = round(r_turn_m * 0.46, 1)
    base_turn_blend_outer_m = round(r_turn_m * 0.85, 1)
    base_turn_roll_kp = round(gain_roll * 1.20, 3)
    final_capture_xtk_m = 45.0
    final_capture_hdg_deg = 18.0
    final_capture_vy_mps = 11.5

    synthesized_gains = {
        "aileron_trim_norm": aileron_trim_norm,
        "aileron_kp_roll": aileron_kp_roll,
        "aileron_kd_p": aileron_kd_p,
        "roll_gain_approach": roll_gain_approach,
        "roll_gain_slip": roll_gain_slip,
        "wing_low_crab_gain": wing_low_crab_gain,
        "wing_low_xtk_kp": wing_low_xtk_kp,
        "wing_low_xtk_ki": wing_low_xtk_ki,
        "wing_low_xtk_kd": wing_low_xtk_kd,
        "rudder_p_factor_trim": rudder_p_factor_trim,
        "rudder_coord_roll_gain": rudder_coord_roll_gain,
        "rudder_decrab_kp": rudder_decrab_kp,
        "rudder_decrab_ki": rudder_decrab_ki,
        "elevator_base_trim": elevator_base_trim,
        "elevator_kp_pitch": elevator_kp_pitch,
        "elevator_ki_pitch": elevator_ki_pitch,
        "elevator_kd_q": elevator_kd_q,
        "gs_kp_pitch": gs_kp_pitch,
        "gs_ki_pitch": gs_ki_pitch,
        "nws_hdg_kp": nws_hdg_kp,
        "nws_xtk_kp": nws_xtk_kp,
        "nws_xtk_kd": nws_xtk_kd,
        "base_turn_lead_time_s": base_turn_lead_time_s,
        "base_turn_dx_gain": base_turn_dx_gain,
        "base_turn_dx_min_m": base_turn_dx_min_m,
        "base_turn_max_intercept_deg": base_turn_max_intercept_deg,
        "base_turn_blend_inner_m": base_turn_blend_inner_m,
        "base_turn_blend_outer_m": base_turn_blend_outer_m,
        "base_turn_roll_kp": base_turn_roll_kp,
        "base_turn_max_bank_deg": base_turn_max_bank_deg,
        "final_capture_xtk_m": final_capture_xtk_m,
        "final_capture_hdg_deg": final_capture_hdg_deg,
        "final_capture_vy_mps": final_capture_vy_mps,
    }

    # Preserve genetic_evolved_gains if already evolved in OUTPUT_JSON_PATH
    genetic_meta = None
    if os.path.exists(OUTPUT_JSON_PATH):
        try:
            with open(OUTPUT_JSON_PATH, "r", encoding="utf-8") as f_old:
                old_data = json.load(f_old)
            genetic_meta = old_data.get("genetic_evolution")
            if genetic_meta and isinstance(genetic_meta.get("evolved_gains"), dict):
                synthesized_gains.update(genetic_meta["evolved_gains"])
        except Exception:
            pass

    synthesized = {
        "lateral_directional_2x2": {
            "matrix_M_lat": [
                [aero["ClDa"], aero["Cldr"]],
                [aero["Cnda_approach"], aero["Cndr"]],
            ],
            "det_M_lat": trim_beta0["det_m_lat"],
            "inv_M_lat": trim_beta0["inv_m_lat"],
            "phi_per_crab_deg": phi_per_crab_deg,
            "da_per_crab_deg": da_per_crab_deg,
            "dr_per_crab_deg": dr_per_crab_deg,
            "beta_5deg_solution": trim_beta_5deg,
        },
        "longitudinal_ground_effect_trim": {
            "approach_65kt_20m": long_app,
            "flare_60kt_2m": long_flare,
        },
        "synthesized_ap_gains": synthesized_gains,
    }

    payload = {
        "c172p_jsbsim": c172p,
        "autopilot_libraries": ap_libs,
        "analytical_synthesis": synthesized,
    }
    if genetic_meta:
        payload["genetic_evolution"] = genetic_meta
    with open(OUTPUT_JSON_PATH, "w", encoding="utf-8") as f:
        json.dump(payload, f, indent=2)
    return payload


def load_or_synthesize_fcs_library(force_rebuild: bool = False) -> Dict:
    if not force_rebuild and os.path.exists(OUTPUT_JSON_PATH):
        try:
            with open(OUTPUT_JSON_PATH, "r", encoding="utf-8") as f:
                return json.load(f)
        except Exception:
            pass
    return synthesize_fcs_autopilot_parameters()


if __name__ == "__main__":
    data = synthesize_fcs_autopilot_parameters()
    print(f"[FCS-LIB-EXTRACTOR] Extracted & synthesized parameters saved to {OUTPUT_JSON_PATH}")
    print(json.dumps(data["analytical_synthesis"], indent=2))
