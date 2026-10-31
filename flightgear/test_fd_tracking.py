#!/usr/bin/env python3
"""
Unit tests for AERO-SHIELD Unified Flight Director / Autopilot Tracking (FD == AP)
and Traditional GPS "Confidently Wrong" Telemetry vs. AERO-SHIELD Quarantine.
"""

import math
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from fg_mcp_ctl import (
    compute_ap_control_from_fd,
    compute_traditional_gps_confidently_wrong_telemetry,
    compute_unified_fd_guidance_py,
    wrap180,
)
from fg_library_extractor import (
    extract_autopilot_xml_libraries,
    extract_c172p_jsbsim_xml,
    load_or_synthesize_fcs_library,
    solve_lateral_directional_2x2_trim,
)
from fast_storm_verdict import run_fast_storm_simulation


class TestFlightDirectorTrackingAndConfidentlyWrong(unittest.TestCase):
    def test_zero_error_on_reference_trajectory_produces_zero_fd_and_ap_offsets(self):
        rwy_hdg = 88.9314
        x_m = -900.0
        gs_tan = math.tan(math.radians(3.0))
        h_agl = (210.0 - x_m) * gs_tan
        vx = 36.0
        vz_down = vx * gs_tan

        fd = compute_unified_fd_guidance_py(
            along_track_m=x_m,
            cross_track_m=0.0,
            alt_agl_m=h_agl,
            vx_mps=vx,
            vy_mps=0.0,
            vz_down_mps=vz_down,
            cur_pitch_deg=-0.95,
            true_heading_deg=rwy_hdg,
            runway_heading_deg=rwy_hdg,
            on_ground_or_stopped=False,
        )
        self.assertLess(abs(fd["heading_correction_deg"]), 1e-4)
        self.assertLess(abs(fd["glideslope_error_deg"]), 1e-4)

        ap = compute_ap_control_from_fd(
            tgt_hdg_deg=fd["target_heading_deg"],
            cur_hdg_deg=rwy_hdg,
            gs_err_deg=fd["glideslope_error_deg"],
            tgt_pit_deg=fd["target_pitch_deg"],
            agl_m=h_agl,
            dist_m=abs(x_m),
            xtk_m=0.0,
            cur_roll_deg=0.0,
            cur_pitch_deg=fd["target_pitch_deg"],
            ias_kt=70.0,
        )
        self.assertAlmostEqual(ap["cmd_roll_deg"], 0.0, places=4)
        self.assertAlmostEqual(ap["cmd_pit_deg"], fd["target_pitch_deg"], places=4)

    def test_closed_loop_600_steps_60hz_convergence_to_green_dot(self):
        """
        Simulates >= 600 steps at 60 Hz starting from lateral offset y = 18.0m,
        heading error hdg_err = +8.0 deg, and glideslope error gs_err = +1.2 deg.
        Verifies convergence to |hdg_corr| < 0.15 deg, |gs_err| < 0.10 deg, and |y| < 0.5m.
        """
        dt = 1.0 / 60.0
        rwy_hdg = 88.9314
        gs_tan = math.tan(math.radians(3.0))

        x_m = -1350.0
        y_m = 18.0
        cur_hdg = rwy_hdg + 8.0
        cur_roll = 0.0

        spd_mps = 36.0
        # Initial FPA is -4.2 deg (1.2 deg steeper than -3.0 deg reference -> gs_err = +1.2 deg)
        cur_fpa_deg = -4.2
        aoa_trim_deg = 2.05
        cur_pitch = cur_fpa_deg + aoa_trim_deg
        h_agl = (210.0 - x_m) * gs_tan

        gs_int = 0.0
        elev_int = 0.0

        for _ in range(720):
            rel_trk_rad = math.radians(wrap180(cur_hdg - rwy_hdg))
            vx = spd_mps * math.cos(rel_trk_rad)
            vy = spd_mps * math.sin(rel_trk_rad)
            cur_fpa_deg = cur_pitch - aoa_trim_deg
            vz_down = -spd_mps * math.tan(math.radians(cur_fpa_deg))

            fd = compute_unified_fd_guidance_py(
                along_track_m=x_m,
                cross_track_m=y_m,
                alt_agl_m=h_agl,
                vx_mps=vx,
                vy_mps=vy,
                vz_down_mps=vz_down,
                cur_pitch_deg=cur_pitch,
                true_heading_deg=cur_hdg,
                runway_heading_deg=rwy_hdg,
                on_ground_or_stopped=False,
            )

            ap = compute_ap_control_from_fd(
                tgt_hdg_deg=fd["target_heading_deg"],
                cur_hdg_deg=cur_hdg,
                gs_err_deg=fd["glideslope_error_deg"],
                tgt_pit_deg=fd["target_pitch_deg"],
                agl_m=h_agl,
                dist_m=fd["distance_to_rwy_m"],
                xtk_m=y_m,
                cur_roll_deg=cur_roll,
                cur_pitch_deg=cur_pitch,
                ias_kt=70.0,
                vs_fpm=-vz_down * 196.85,
                p_dps=0.0,
                q_dps=0.0,
                flare_active=fd["flare_active"],
                rwy_hdg_deg=rwy_hdg,
                gs_int=gs_int,
                elev_int=elev_int,
                dt=dt,
            )
            gs_int = ap["gs_int"]
            elev_int = ap["elev_int"]

            # First-order roll, heading, and pitch airframe response
            cur_roll += (ap["cmd_roll_deg"] - cur_roll) * min(1.0, 5.5 * dt)
            yaw_rate_dps = cur_roll * 1.45
            cur_hdg = (cur_hdg + yaw_rate_dps * dt) % 360.0
            cur_pitch += (ap["cmd_pit_deg"] - cur_pitch) * min(1.0, 4.5 * dt)

            x_m += vx * dt
            y_m += vy * dt
            h_agl = max(0.0, h_agl - vz_down * dt)

        self.assertLess(
            abs(ap["hdg_corr_deg"]),
            0.15,
            f"Expected |hdg_corr| < 0.15 deg, got {ap['hdg_corr_deg']:.4f} deg",
        )
        self.assertLess(
            abs(fd["glideslope_error_deg"]),
            0.10,
            f"Expected |gs_err| < 0.10 deg, got {fd['glideslope_error_deg']:.4f} deg",
        )
        self.assertLess(
            abs(y_m),
            0.50,
            f"Expected |y| < 0.50 m, got {y_m:.4f} m",
        )

    def test_traditional_gps_confidently_wrong_telemetry(self):
        """
        When spoof_offset_m = 250.0 and real_y_m = 0.0 (aircraft is on real Runway 09 centerline),
        traditional GPS reports +250m and commands a -250m turn into the field with RAIM = OK,
        while AERO-SHIELD quarantines GNSS and reports hdg_corr = 0.0 deg on the real runway.
        """
        t = compute_traditional_gps_confidently_wrong_telemetry(
            real_x_m=-800.0,
            real_y_m=0.0,
            alt_agl_m=(210.0 - (-800.0)) * math.tan(math.radians(3.0)),
            vx_mps=36.0,
            vy_mps=0.0,
            vz_down_mps=36.0 * math.tan(math.radians(3.0)),
            true_heading_deg=88.9314,
            runway_heading_deg=88.9314,
            spoof_offset_m=250.0,
            shield_active=True,
        )
        self.assertEqual(t["sats_locked"], "12/12")
        self.assertEqual(t["trad_raim_status"], "OK")
        self.assertEqual(t["trad_raim_faults"], 0)
        self.assertAlmostEqual(t["trad_reported_xtk_m"], 250.0, places=3)
        self.assertAlmostEqual(t["trad_commanded_turn_m"], -250.0, places=3)
        self.assertLess(t["trad_hdg_corr_deg"], -20.0)
        self.assertTrue(t["aeroshield_quarantined"])
        self.assertGreater(t["aeroshield_chi2"], 18.5)
        self.assertAlmostEqual(t["aeroshield_real_xtk_m"], 0.0, places=3)
        self.assertAlmostEqual(t["aeroshield_hdg_corr_deg"], 0.0, places=3)

    def test_wing_low_crosswind_alignment_zero_diagonal_at_touchdown(self):
        """
        Simulates short final and flare under crosswind (started with +7.0 deg crab at 38m AGL)
        down to touchdown zone (alt_agl <= 10.0m) using Wing-Low / Forward-Slip decrab.
        Verifies that at touchdown:
          - |cur_hdg - 88.9314| < 1.2 deg (zero diagonal landing!)
          - |hdg_corr| < 0.25 deg
          - |y| < 1.0 m
        """
        dt = 1.0 / 60.0
        rwy_hdg = 88.9314
        gs_tan = math.tan(math.radians(3.0))

        x_m = -510.0
        y_m = 1.5
        cur_hdg = rwy_hdg + 7.0
        cur_roll = 0.0
        spd_mps = 35.0
        aoa_trim_deg = 2.05
        cur_fpa_deg = -3.0
        cur_pitch = cur_fpa_deg + aoa_trim_deg
        h_agl = (210.0 - x_m) * gs_tan
        wy_wind_mps = -4.2  # Crosswind from right (South 175 deg) pushing left (-Y)

        xtk_int = 0.0
        gs_int = 0.0
        elev_int = 0.0

        for _ in range(950):
            rel_yaw_rad = math.radians(wrap180(cur_hdg - rwy_hdg))
            # Wing-low sideslip velocity cancels crosswind drift without yawing the nose
            slip_vy_mps = math.sin(math.radians(cur_roll)) * 41.4
            vx = spd_mps * math.cos(rel_yaw_rad)
            vy = spd_mps * math.sin(rel_yaw_rad) + wy_wind_mps + slip_vy_mps
            cur_fpa_deg = cur_pitch - aoa_trim_deg
            vz_down = -spd_mps * math.tan(math.radians(cur_fpa_deg))

            fd = compute_unified_fd_guidance_py(
                along_track_m=x_m,
                cross_track_m=y_m,
                alt_agl_m=h_agl,
                vx_mps=vx,
                vy_mps=vy,
                vz_down_mps=vz_down,
                cur_pitch_deg=cur_pitch,
                true_heading_deg=cur_hdg,
                runway_heading_deg=rwy_hdg,
                on_ground_or_stopped=False,
            )

            crab_est_deg = math.degrees(math.atan2(-wy_wind_mps, spd_mps))
            ap = compute_ap_control_from_fd(
                tgt_hdg_deg=fd["target_heading_deg"],
                cur_hdg_deg=cur_hdg,
                gs_err_deg=fd["glideslope_error_deg"],
                tgt_pit_deg=fd["target_pitch_deg"],
                agl_m=h_agl,
                dist_m=fd["distance_to_rwy_m"],
                xtk_m=y_m,
                cur_roll_deg=cur_roll,
                cur_pitch_deg=cur_pitch,
                ias_kt=68.0,
                vs_fpm=-vz_down * 196.85,
                p_dps=0.0,
                q_dps=0.0,
                flare_active=fd["flare_active"],
                rwy_hdg_deg=rwy_hdg,
                crab_deg=crab_est_deg,
                xtk_rate_mps=vy,
                xtk_int=xtk_int,
                gs_int=gs_int,
                elev_int=elev_int,
                dt=dt,
            )
            xtk_int = ap["xtk_int"]
            gs_int = ap["gs_int"]
            elev_int = ap["elev_int"]

            cur_roll += (ap["cmd_roll_deg"] - cur_roll) * min(1.0, 6.0 * dt)
            # Rudder decrab directly aligns nose heading with tgt_hdg_deg while wing-low holds bank
            yaw_rate_dps = (ap["cmd_rud"] - 0.016) * 48.0 + wrap180(fd["target_heading_deg"] - cur_hdg) * 1.8
            cur_hdg = (cur_hdg + yaw_rate_dps * dt) % 360.0
            cur_pitch += (ap["cmd_pit_deg"] - cur_pitch) * min(1.0, 4.5 * dt)

            x_m += vx * dt
            y_m += vy * dt
            h_agl = max(1.8, h_agl - vz_down * dt)
            if h_agl <= 6.0 and x_m >= -10.0:
                break

        self.assertLessEqual(h_agl, 10.0)
        self.assertLess(
            abs(wrap180(cur_hdg - rwy_hdg)),
            1.2,
            f"Expected |cur_hdg - rwy_hdg| < 1.2 deg at touchdown, got {wrap180(cur_hdg - rwy_hdg):.4f} deg",
        )
        self.assertLess(
            abs(ap["hdg_corr_deg"]),
            0.25,
            f"Expected |hdg_corr| < 0.25 deg at touchdown, got {ap['hdg_corr_deg']:.4f} deg",
        )
        self.assertLess(
            abs(y_m),
            1.0,
            f"Expected |y| < 1.0 m at touchdown, got {y_m:.4f} m",
        )

    def test_fg_xml_library_extraction_and_2x2_matrix_inversion(self):
        """
        Verifies automatic extraction from FlightGear XML libraries (c172p.xml, kap140-autopilot.xml,
        kap140-config.xml, generic-autopilot.xml) and analytical 2x2 matrix inversion M_lat^-1
        with zero moment residual (< 1e-12).
        """
        lib = load_or_synthesize_fcs_library(force_rebuild=True)
        self.assertIn("c172p_jsbsim", lib)
        self.assertIn("autopilot_libraries", lib)
        self.assertIn("analytical_synthesis", lib)

        jsb = extract_c172p_jsbsim_xml()
        ap_xml = extract_autopilot_xml_libraries()
        self.assertAlmostEqual(jsb["metrics"]["wingarea_ft2"], 174.0, places=2)
        self.assertAlmostEqual(jsb["metrics"]["wingspan_ft"], 35.8, places=2)
        self.assertLess(jsb["aerodynamics"]["Clb"], 0.0)
        self.assertGreater(jsb["aerodynamics"]["Cnb"], 0.0)
        self.assertGreater(ap_xml["kap140_config"]["gain_roll"], 0.0)
        self.assertIn("System Command: Roll PI", ap_xml["kap140_controllers"])

        trim_14kt = solve_lateral_directional_2x2_trim(
            aero=jsb["aerodynamics"],
            fcs=jsb["fcs_channels"],
            metrics=jsb["metrics"],
            mass_balance=jsb["mass_balance"],
            beta_rad=math.radians(12.2),
            tas_kt=68.0,
            flap_deg=10.0,
            propwash_ratio=0.0062,
        )
        self.assertNotAlmostEqual(trim_14kt["det_m_lat"], 0.0, places=5)
        self.assertLess(abs(trim_14kt["roll_moment_residual"]), 1e-12)
        self.assertLess(abs(trim_14kt["yaw_moment_residual"]), 1e-12)
        self.assertGreater(trim_14kt["phi_slip_deg"], 2.0)

    def test_fast_storm_simulation_verdict_pass(self):
        """
        Runs the 60 Hz 14-knot storm + GPS spoofing (+250m) simulation and verifies
        VERDICT == PASS (|Xtk| < 1.5m, |Hdg - 88.93°| < 1.5°, |hdg_corr| < 0.5°, |gs_err| < 0.3°).
        """
        res = run_fast_storm_simulation()
        self.assertEqual(res["verdict"], "PASS", f"Expected VERDICT PASS, got: {res}")
        self.assertTrue(res["touchdown_on_runway"])
        self.assertLess(abs(res["touchdown"]["xtk_m"]), 1.5)
        self.assertLess(abs(res["rollout_stop"]["xtk_m"]), 1.5)
        self.assertLess(abs(res["touchdown"]["hdg_err_rwy_deg"]), 1.5)
        self.assertLess(abs(res["rollout_stop"]["hdg_err_rwy_deg"]), 1.5)
        self.assertLess(abs(res["touchdown"]["hdg_corr_deg"]), 0.5)
        self.assertLess(abs(res["touchdown"]["gs_err_deg"]), 0.3)

    def test_stellarium_swir_star_tracker_fast_verdict(self):
        """
        Verifies the Stellarium J2000/ICRS + SWIR 2.2um Infrared Star Tracker suite:
          - SWIR SNR > 20 dB and > +18 dB gain over Visible in 220m storm fog
          - 100% Spherical Triangle Star-ID match rate & < 0.02 px sub-pixel centroiding
          - SVD Wahba 3D attitude error < 0.01 deg (canceling +3.5 deg INS gyro drift)
          - Combined Astro-Inertial + Storm + GPS Spoofing autoland VERDICT == PASS
        """
        from fast_stellarium_swir_verdict import run_all_fast_stellarium_swir_tests

        summary = run_all_fast_stellarium_swir_tests()
        self.assertEqual(summary["verdict"], "PASS", f"Expected VERDICT PASS, got: {summary}")
        self.assertTrue(summary["tests"]["test1_stellarium_ephemeris_and_swir_snr"]["pass"])
        self.assertTrue(summary["tests"]["test2_fpa_centroiding_triangle_id_and_wahba_svd"]["pass"])
        self.assertTrue(summary["tests"]["test3_combined_fg_stellarium_swir_storm_autoland"]["pass"])


if __name__ == "__main__":
    unittest.main()
