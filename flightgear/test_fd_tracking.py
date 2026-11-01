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
    AutonomousFlightPhase,
    compute_ap_control_from_fd,
    compute_maneuver_guidance_and_ap,
    compute_traditional_gps_confidently_wrong_telemetry,
    compute_unified_fd_guidance_py,
    update_autonomous_flight_phase,
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

    def test_c1_cushion_flare_and_universal_unaligned_intercept(self):
        """
        Verifies:
          1. C1 Cushion Flare progressive autothrottle retard (no abrupt throttle cut at 6.0m AGL,
             maintains flight-idle power cushion >= 0.24 down to 1.42m AGL main-gear contact).
          2. Universal Two-Regime S-Turn localizer intercept from +165m unaligned offset.
        """
        rwy_hdg = 256.82  # Universal runway test (Catania RWY 26)
        for agl_test in (5.8, 4.5, 3.0, 1.65):
            ap_flare = compute_ap_control_from_fd(
                tgt_hdg_deg=rwy_hdg,
                cur_hdg_deg=rwy_hdg,
                gs_err_deg=0.0,
                tgt_pit_deg=1.8,
                agl_m=agl_test,
                dist_m=0.0,
                xtk_m=0.0,
                cur_roll_deg=0.0,
                cur_pitch_deg=1.2,
                ias_kt=62.0,
                vs_fpm=-210.0,
                flare_active=True,
                rwy_hdg_deg=rwy_hdg,
            )
            self.assertGreaterEqual(
                ap_flare["throttle"],
                0.24,
                f"Expected warm throttle cushion >= 0.24 at {agl_test}m AGL, got {ap_flare['throttle']:.3f}",
            )

        ap_td = compute_ap_control_from_fd(
            tgt_hdg_deg=rwy_hdg,
            cur_hdg_deg=rwy_hdg,
            gs_err_deg=0.0,
            tgt_pit_deg=2.0,
            agl_m=1.38,
            dist_m=0.0,
            xtk_m=0.0,
            cur_roll_deg=0.0,
            cur_pitch_deg=1.8,
            ias_kt=57.0,
            vs_fpm=-95.0,
            flare_active=True,
            rwy_hdg_deg=rwy_hdg,
        )
        self.assertAlmostEqual(ap_td["throttle"], 0.0, places=4)

        fd_unaligned = compute_unified_fd_guidance_py(
            along_track_m=-2100.0,
            cross_track_m=165.0,
            alt_agl_m=118.0,
            vx_mps=34.0,
            vy_mps=12.0,
            vz_down_mps=1.8,
            cur_pitch_deg=-1.0,
            true_heading_deg=rwy_hdg - 22.0,
            runway_heading_deg=rwy_hdg,
            on_ground_or_stopped=False,
        )
        # Target intercept heading is -28 deg left of runway heading, so from -22 deg it commands -6 deg more left
        self.assertAlmostEqual(wrap180(fd_unaligned["target_heading_deg"] - rwy_hdg), -28.0, places=2)
        self.assertAlmostEqual(fd_unaligned["heading_correction_deg"], -6.0, places=2)

    def test_uprt_upset_recovery_and_go_around_teardrop_state_machine(self):
        """
        Verifies:
          1. FAA/ICAO UPRT Upset Recovery:
             - Spiral dive (roll=-48 deg, pitch=-15 deg, vs=-1450 fpm) triggers UPSET_RECOVERY.
             - Roll gate keeps pitch pull moderated while |roll| > 30 deg (roll wings level BEFORE pulling G).
             - Stall (ias=47 kt, pitch=+15 deg) triggers UPSET_RECOVERY with full power (thr=1.0) and nose-down unload.
          2. ICAO Doc 8168 Balked Landing / Go-Around + Teardrop Circuit:
             - Unstabilized aircraft past threshold (x_rwy=+120m, agl=26m, ias=88kt) triggers GO_AROUND_TOGA.
             - Climbs at TOGA power (thr=1.0, flaps=0.0, pitch=+7.2 deg) -> transitions to TEARDROP_OUTBOUND at 85m+ AGL
               -> flies outbound to x_rwy <= -1850m -> transitions to BASE_TURN_INTERCEPT -> re-captures FINAL_APPROACH.
        """
        rwy_hdg = 88.9314

        # 1. Spiral Dive UPRT Trigger & Roll-Gate Verification
        phase, td_side = update_autonomous_flight_phase(
            cur_phase=AutonomousFlightPhase.FINAL_APPROACH,
            x_rwy_m=-2100.0,
            xtk_m=35.0,
            agl_m=140.0,
            ias_kt=82.0,
            vs_fpm=-1450.0,
            cur_roll_deg=-48.0,
            cur_pitch_deg=-15.0,
            cur_hdg_deg=72.0,
            rwy_hdg_deg=rwy_hdg,
            touchdown_latched=False,
        )
        self.assertEqual(phase, AutonomousFlightPhase.UPSET_RECOVERY)

        ap_spiral = compute_maneuver_guidance_and_ap(
            phase=phase,
            x_rwy_m=-2100.0,
            xtk_m=35.0,
            agl_m=140.0,
            ias_kt=82.0,
            vs_fpm=-1450.0,
            cur_roll_deg=-48.0,
            cur_pitch_deg=-15.0,
            cur_hdg_deg=72.0,
            rwy_hdg_deg=rwy_hdg,
            teardrop_side=td_side,
        )
        # Commands strong right aileron (> 0) to level wings first, while roll_gate limits pitch pull
        self.assertGreater(ap_spiral["aileron"], 0.35)
        self.assertAlmostEqual(ap_spiral["cmd_roll_deg"], 0.0, places=3)

        # 2. Stall Recovery Verification
        phase_stall, _ = update_autonomous_flight_phase(
            cur_phase=AutonomousFlightPhase.FINAL_APPROACH,
            x_rwy_m=-1500.0,
            xtk_m=5.0,
            agl_m=95.0,
            ias_kt=47.0,
            vs_fpm=-650.0,
            cur_roll_deg=8.0,
            cur_pitch_deg=15.0,
            cur_hdg_deg=rwy_hdg,
            rwy_hdg_deg=rwy_hdg,
            touchdown_latched=False,
        )
        self.assertEqual(phase_stall, AutonomousFlightPhase.UPSET_RECOVERY)
        ap_stall = compute_maneuver_guidance_and_ap(
            phase=phase_stall,
            x_rwy_m=-1500.0,
            xtk_m=5.0,
            agl_m=95.0,
            ias_kt=47.0,
            vs_fpm=-650.0,
            cur_roll_deg=8.0,
            cur_pitch_deg=15.0,
            cur_hdg_deg=rwy_hdg,
            rwy_hdg_deg=rwy_hdg,
        )
        self.assertAlmostEqual(ap_stall["throttle"], 1.0, places=3)
        self.assertLess(ap_stall["cmd_pit_deg"], 0.0)  # Unloads AoA nose-down

        # 3. Go-Around & Teardrop Circuit Full Transition Chain
        phase_ga, td_side = update_autonomous_flight_phase(
            cur_phase=AutonomousFlightPhase.FINAL_APPROACH,
            x_rwy_m=120.0,  # Past threshold!
            xtk_m=26.0,
            agl_m=26.0,     # Too high over runway!
            ias_kt=88.0,    # Too fast!
            vs_fpm=-200.0,
            cur_roll_deg=5.0,
            cur_pitch_deg=-1.0,
            cur_hdg_deg=rwy_hdg + 10.0,
            rwy_hdg_deg=rwy_hdg,
            touchdown_latched=False,
        )
        self.assertEqual(phase_ga, AutonomousFlightPhase.GO_AROUND_TOGA)

        ap_toga = compute_maneuver_guidance_and_ap(
            phase=phase_ga,
            x_rwy_m=120.0,
            xtk_m=26.0,
            agl_m=26.0,
            ias_kt=72.0,
            vs_fpm=450.0,
            cur_roll_deg=0.0,
            cur_pitch_deg=5.0,
            cur_hdg_deg=rwy_hdg,
            rwy_hdg_deg=rwy_hdg,
            teardrop_side=td_side,
        )
        self.assertAlmostEqual(ap_toga["throttle"], 1.0, places=3)
        self.assertAlmostEqual(ap_toga["flaps"], 0.33, places=3)  # Keep 10 deg flaps below 32m AGL to prevent sink
        self.assertGreater(ap_toga["cmd_pit_deg"], 6.0)

        ap_toga_clean = compute_maneuver_guidance_and_ap(
            phase=phase_ga,
            x_rwy_m=220.0,
            xtk_m=20.0,
            agl_m=45.0,
            ias_kt=74.0,
            vs_fpm=500.0,
            cur_roll_deg=0.0,
            cur_pitch_deg=6.5,
            cur_hdg_deg=rwy_hdg,
            rwy_hdg_deg=rwy_hdg,
            teardrop_side=td_side,
        )
        self.assertAlmostEqual(ap_toga_clean["flaps"], 0.0, places=3)  # Retract to 0 deg once > 32m AGL

        # Climb reaches 92m AGL past x_rwy = +380m -> transitions to TEARDROP_OUTBOUND
        phase_td, td_side = update_autonomous_flight_phase(
            cur_phase=phase_ga,
            x_rwy_m=380.0,
            xtk_m=15.0,
            agl_m=92.0,
            ias_kt=70.0,
            vs_fpm=520.0,
            cur_roll_deg=0.0,
            cur_pitch_deg=6.8,
            cur_hdg_deg=rwy_hdg,
            rwy_hdg_deg=rwy_hdg,
            touchdown_latched=False,
            teardrop_side=td_side,
        )
        self.assertEqual(phase_td, AutonomousFlightPhase.TEARDROP_OUTBOUND)

        # Outbound leg reaches x_rwy = -1920m at 135m AGL -> transitions to BASE_TURN_INTERCEPT
        phase_base, td_side = update_autonomous_flight_phase(
            cur_phase=phase_td,
            x_rwy_m=-1920.0,
            xtk_m=-480.0,
            agl_m=135.0,
            ias_kt=78.0,
            vs_fpm=0.0,
            cur_roll_deg=0.0,
            cur_pitch_deg=1.5,
            cur_hdg_deg=(rwy_hdg + 210.0) % 360.0,
            rwy_hdg_deg=rwy_hdg,
            touchdown_latched=False,
            teardrop_side=td_side,
        )
        self.assertEqual(phase_base, AutonomousFlightPhase.BASE_TURN_INTERCEPT)

        # Base turn aligns within |xtk| < 48m and |hdg_err| < 22 deg -> transitions back to FINAL_APPROACH
        phase_final, _ = update_autonomous_flight_phase(
            cur_phase=phase_base,
            x_rwy_m=-1950.0,
            xtk_m=-32.0,
            agl_m=118.0,
            ias_kt=71.0,
            vs_fpm=-320.0,
            cur_roll_deg=8.0,
            cur_pitch_deg=-1.0,
            cur_hdg_deg=rwy_hdg + 16.0,
            rwy_hdg_deg=rwy_hdg,
            touchdown_latched=False,
            teardrop_side=td_side,
        )
        self.assertEqual(phase_final, AutonomousFlightPhase.FINAL_APPROACH)

    def test_closed_loop_go_around_teardrop_circuit_to_touchdown(self):
        """
        Full 60 Hz closed-loop trajectory simulation of a balked landing:
        Aircraft arrives over the runway too high, too fast, and unaligned
        (x_rwy=+180m, xtk=+28m, agl=32m, ias=88kt, hdg=98 deg).
        Verifies the complete autonomous sequence:
        FINAL_APPROACH -> GO_AROUND_TOGA -> TEARDROP_OUTBOUND -> BASE_TURN_INTERCEPT -> FINAL_APPROACH -> TOUCHDOWN.
        """
        dt = 1.0 / 60.0
        rwy_hdg = 88.9314
        x_m = 180.0
        y_m = 28.0
        h_agl = 32.0
        ias_kt = 88.0
        cur_hdg = 98.0
        cur_roll = 12.0
        cur_pitch = -2.0
        vs_fpm = -400.0
        aoa_trim_deg = 2.05

        phase = AutonomousFlightPhase.FINAL_APPROACH
        td_side = 1.0
        visited_phases = [phase]
        xtk_int = 0.0
        rud_int = 0.0
        gs_int = 0.0
        elev_int = 0.0
        touchdown_latched = False

        for _ in range(12500):
            spd_mps = ias_kt * 0.514444
            rel_trk_rad = math.radians(wrap180(cur_hdg - rwy_hdg))
            vx = spd_mps * math.cos(rel_trk_rad)
            vy = spd_mps * math.sin(rel_trk_rad)
            vz_down = -vs_fpm / 196.85

            phase, td_side = update_autonomous_flight_phase(
                cur_phase=phase,
                x_rwy_m=x_m,
                xtk_m=y_m,
                agl_m=h_agl,
                ias_kt=ias_kt,
                vs_fpm=vs_fpm,
                cur_roll_deg=cur_roll,
                cur_pitch_deg=cur_pitch,
                cur_hdg_deg=cur_hdg,
                rwy_hdg_deg=rwy_hdg,
                touchdown_latched=touchdown_latched,
                teardrop_side=td_side,
            )
            if phase != visited_phases[-1]:
                visited_phases.append(phase)
                xtk_int = 0.0
                gs_int = 0.0
                elev_int = 0.0

            if phase == AutonomousFlightPhase.FINAL_APPROACH and h_agl <= 1.38 and -25.0 <= x_m <= 1150.0 and abs(y_m) <= 22.0:
                touchdown_latched = True
                visited_phases.append(AutonomousFlightPhase.ROLLOUT_BRAKE)
                break

            if phase != AutonomousFlightPhase.FINAL_APPROACH:
                ap = compute_maneuver_guidance_and_ap(
                    phase=phase,
                    x_rwy_m=x_m,
                    xtk_m=y_m,
                    agl_m=h_agl,
                    ias_kt=ias_kt,
                    vs_fpm=vs_fpm,
                    cur_roll_deg=cur_roll,
                    cur_pitch_deg=cur_pitch,
                    cur_hdg_deg=cur_hdg,
                    rwy_hdg_deg=rwy_hdg,
                    vx_rwy_mps=vx,
                    vy_rwy_mps=vy,
                    teardrop_side=td_side,
                    elev_int=elev_int,
                    dt=dt,
                )
            else:
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
                    ias_kt=ias_kt,
                    vs_fpm=vs_fpm,
                    flare_active=fd["flare_active"],
                    rwy_hdg_deg=rwy_hdg,
                    xtk_rate_mps=vy,
                    xtk_int=xtk_int,
                    rud_int=rud_int,
                    gs_int=gs_int,
                    elev_int=elev_int,
                    dt=dt,
                )
                xtk_int = ap["xtk_int"]
                rud_int = ap["rud_int"]
                gs_int = ap["gs_int"]
                elev_int = ap["elev_int"]

            cur_roll += (ap["cmd_roll_deg"] - cur_roll) * min(1.0, 5.0 * dt)
            yaw_rate_dps = cur_roll * 1.42
            cur_hdg = (cur_hdg + yaw_rate_dps * dt) % 360.0
            cur_pitch += (ap["cmd_pit_deg"] - cur_pitch) * min(1.0, 4.2 * dt)
            fpa_deg = cur_pitch - aoa_trim_deg
            target_ias = 45.0 + ap["throttle"] * 38.0 - fpa_deg * 1.2
            ias_kt += (target_ias - ias_kt) * min(1.0, 0.85 * dt)
            vz_down = -spd_mps * math.tan(math.radians(fpa_deg))
            vs_fpm = -vz_down * 196.85
            x_m += vx * dt
            y_m += vy * dt
            h_agl = max(1.0, h_agl - vz_down * dt)

        expected_sequence = [
            AutonomousFlightPhase.FINAL_APPROACH,
            AutonomousFlightPhase.GO_AROUND_TOGA,
            AutonomousFlightPhase.TEARDROP_OUTBOUND,
            AutonomousFlightPhase.BASE_TURN_INTERCEPT,
            AutonomousFlightPhase.FINAL_APPROACH,
            AutonomousFlightPhase.ROLLOUT_BRAKE,
        ]
        self.assertEqual(visited_phases, expected_sequence)
        self.assertTrue(touchdown_latched)
        self.assertGreaterEqual(x_m, 0.0)
        self.assertLessEqual(x_m, 650.0)
        self.assertLess(abs(y_m), 1.5)
        self.assertLess(abs(wrap180(cur_hdg - rwy_hdg)), 1.5)

    def test_closed_loop_uprt_spiral_dive_and_stall_recovery_to_touchdown(self):
        """
        Verifies 60 Hz closed-loop recovery from:
          A) A severe pilot-induced spiral dive (roll=-48 deg, pitch=-14.5 deg, vs=-1500 fpm)
             at x_rwy=-2200m, xtk=-35m -> recovers in UPSET_RECOVERY -> transitions directly to
             FINAL_APPROACH (without false Go-Around) -> lands with |xtk| < 1.0m.
          B) An aerodynamic stall (ias=46kt, pitch=+15.5 deg, roll=+22 deg) at x_rwy=-1650m
             -> unloads AoA & applies full power in UPSET_RECOVERY -> transitions to FINAL_APPROACH
             -> lands with |xtk| < 1.0m.
        """
        dt = 1.0 / 60.0
        rwy_hdg = 88.9314
        aoa_trim_deg = 2.05

        for scenario_name, init_state in (
            (
                "spiral_dive",
                {
                    "x_m": -2200.0,
                    "y_m": -35.0,
                    "h_agl": 140.0,
                    "ias_kt": 78.0,
                    "cur_hdg": 72.0,
                    "cur_roll": -48.0,
                    "cur_pitch": -14.5,
                    "vs_fpm": -1500.0,
                },
            ),
            (
                "aerodynamic_stall",
                {
                    "x_m": -1650.0,
                    "y_m": 22.0,
                    "h_agl": 105.0,
                    "ias_kt": 46.0,
                    "cur_hdg": 99.0,
                    "cur_roll": 22.0,
                    "cur_pitch": 15.5,
                    "vs_fpm": -750.0,
                },
            ),
        ):
            x_m = init_state["x_m"]
            y_m = init_state["y_m"]
            h_agl = init_state["h_agl"]
            ias_kt = init_state["ias_kt"]
            cur_hdg = init_state["cur_hdg"]
            cur_roll = init_state["cur_roll"]
            cur_pitch = init_state["cur_pitch"]
            vs_fpm = init_state["vs_fpm"]

            phase = AutonomousFlightPhase.FINAL_APPROACH
            td_side = 1.0
            visited_phases = [phase]
            xtk_int = 0.0
            rud_int = 0.0
            gs_int = 0.0
            elev_int = 0.0
            touchdown_latched = False

            for _ in range(6000):
                spd_mps = ias_kt * 0.514444
                rel_trk_rad = math.radians(wrap180(cur_hdg - rwy_hdg))
                vx = spd_mps * math.cos(rel_trk_rad)
                vy = spd_mps * math.sin(rel_trk_rad)
                vz_down = -vs_fpm / 196.85

                phase, td_side = update_autonomous_flight_phase(
                    cur_phase=phase,
                    x_rwy_m=x_m,
                    xtk_m=y_m,
                    agl_m=h_agl,
                    ias_kt=ias_kt,
                    vs_fpm=vs_fpm,
                    cur_roll_deg=cur_roll,
                    cur_pitch_deg=cur_pitch,
                    cur_hdg_deg=cur_hdg,
                    rwy_hdg_deg=rwy_hdg,
                    touchdown_latched=touchdown_latched,
                    teardrop_side=td_side,
                )
                if phase != visited_phases[-1]:
                    visited_phases.append(phase)

                if phase == AutonomousFlightPhase.FINAL_APPROACH and h_agl <= 1.38 and -25.0 <= x_m <= 1150.0 and abs(y_m) <= 22.0:
                    touchdown_latched = True
                    visited_phases.append(AutonomousFlightPhase.ROLLOUT_BRAKE)
                    break

                if phase != AutonomousFlightPhase.FINAL_APPROACH:
                    ap = compute_maneuver_guidance_and_ap(
                        phase=phase,
                        x_rwy_m=x_m,
                        xtk_m=y_m,
                        agl_m=h_agl,
                        ias_kt=ias_kt,
                        vs_fpm=vs_fpm,
                        cur_roll_deg=cur_roll,
                        cur_pitch_deg=cur_pitch,
                        cur_hdg_deg=cur_hdg,
                        rwy_hdg_deg=rwy_hdg,
                        vx_rwy_mps=vx,
                        vy_rwy_mps=vy,
                        teardrop_side=td_side,
                        elev_int=elev_int,
                        dt=dt,
                    )
                else:
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
                        ias_kt=ias_kt,
                        vs_fpm=vs_fpm,
                        flare_active=fd["flare_active"],
                        rwy_hdg_deg=rwy_hdg,
                        xtk_rate_mps=vy,
                        xtk_int=xtk_int,
                        rud_int=rud_int,
                        gs_int=gs_int,
                        elev_int=elev_int,
                        dt=dt,
                    )
                    xtk_int = ap["xtk_int"]
                    rud_int = ap["rud_int"]
                    gs_int = ap["gs_int"]
                    elev_int = ap["elev_int"]

                cur_roll += (ap["cmd_roll_deg"] - cur_roll) * min(1.0, 5.2 * dt)
                yaw_rate_dps = cur_roll * 1.42
                cur_hdg = (cur_hdg + yaw_rate_dps * dt) % 360.0
                cur_pitch += (ap["cmd_pit_deg"] - cur_pitch) * min(1.0, 4.4 * dt)
                fpa_deg = cur_pitch - aoa_trim_deg
                target_ias = 45.0 + ap["throttle"] * 38.0 - fpa_deg * 1.2
                ias_kt += (target_ias - ias_kt) * min(1.0, 1.15 * dt)
                vz_down = -spd_mps * math.tan(math.radians(fpa_deg))
                vs_fpm = -vz_down * 196.85
                x_m += vx * dt
                y_m += vy * dt
                h_agl = max(1.0, h_agl - vz_down * dt)

            self.assertEqual(
                visited_phases,
                [
                    AutonomousFlightPhase.FINAL_APPROACH,
                    AutonomousFlightPhase.UPSET_RECOVERY,
                    AutonomousFlightPhase.FINAL_APPROACH,
                    AutonomousFlightPhase.ROLLOUT_BRAKE,
                ],
                f"Unexpected phase sequence in {scenario_name}: {visited_phases}",
            )
            self.assertTrue(touchdown_latched, f"Did not land in {scenario_name}")
            self.assertLess(abs(y_m), 1.0, f"Expected |xtk| < 1.0m in {scenario_name}, got {y_m:.3f}m")
            self.assertLess(abs(wrap180(cur_hdg - rwy_hdg)), 1.2)

    def test_closed_loop_perpendicular_base_turn_to_touchdown(self):
        """
        End-to-end 20 Hz closed-loop simulation of a perpendicular (~85 deg) Base-Leg entry:
        Starts at X_rwy=-2050m, Xtk=+650m, AGL=132m, Heading=3.9 deg (perpendicular to LRCV RWY 09 88.93 deg).
        Verifies automatic transition FINAL_APPROACH -> BASE_TURN_INTERCEPT -> FINAL_APPROACH -> ROLLOUT_BRAKE
        and sub-meter centerline touchdown!
        """
        rwy_hdg = 88.9314
        dt = 0.05
        x_m = -2050.0
        y_m = 650.0
        h_agl = 132.0
        cur_hdg = (rwy_hdg - 85.0) % 360.0
        cur_roll = 0.0
        cur_pitch = 0.5
        ias_kt = 71.0
        vs_fpm = -180.0
        aoa_trim_deg = 2.05

        phase = AutonomousFlightPhase.FINAL_APPROACH
        td_side = 1.0
        visited_phases = [phase]
        xtk_int = 0.0
        rud_int = 0.0
        gs_int = 0.0
        elev_int = 0.0
        touchdown_latched = False
        prev_base_tgt_hdg = None
        max_base_turn_tgt_hdg_step = 0.0

        for _ in range(int(180.0 / dt)):
            spd_mps = ias_kt * 0.514444
            trk_rad = math.radians(wrap180(cur_hdg - rwy_hdg))
            vx = spd_mps * math.cos(trk_rad)
            vy = spd_mps * math.sin(trk_rad)
            vz_down = -vs_fpm / 196.85

            if h_agl <= 1.48 and x_m >= -10.0:
                touchdown_latched = True

            next_phase, td_side = update_autonomous_flight_phase(
                cur_phase=phase,
                x_rwy_m=x_m,
                xtk_m=y_m,
                agl_m=h_agl,
                ias_kt=ias_kt,
                vs_fpm=vs_fpm,
                cur_roll_deg=cur_roll,
                cur_pitch_deg=cur_pitch,
                cur_hdg_deg=cur_hdg,
                rwy_hdg_deg=rwy_hdg,
                touchdown_latched=touchdown_latched,
                teardrop_side=td_side,
            )
            if next_phase != phase:
                phase = next_phase
                visited_phases.append(phase)
                xtk_int = 0.0
                gs_int = 0.0
                elev_int = 0.0

            if touchdown_latched:
                break

            if phase != AutonomousFlightPhase.FINAL_APPROACH:
                ap = compute_maneuver_guidance_and_ap(
                    phase=phase,
                    x_rwy_m=x_m,
                    xtk_m=y_m,
                    agl_m=h_agl,
                    ias_kt=ias_kt,
                    vs_fpm=vs_fpm,
                    cur_roll_deg=cur_roll,
                    cur_pitch_deg=cur_pitch,
                    cur_hdg_deg=cur_hdg,
                    rwy_hdg_deg=rwy_hdg,
                    vx_rwy_mps=vx,
                    vy_rwy_mps=vy,
                    teardrop_side=td_side,
                    elev_int=elev_int,
                    dt=dt,
                )
                if phase == AutonomousFlightPhase.BASE_TURN_INTERCEPT:
                    if prev_base_tgt_hdg is not None:
                        step_deg = abs(wrap180(ap["tgt_hdg_deg"] - prev_base_tgt_hdg))
                        max_base_turn_tgt_hdg_step = max(max_base_turn_tgt_hdg_step, step_deg)
                    prev_base_tgt_hdg = ap["tgt_hdg_deg"]
            else:
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
                    ias_kt=ias_kt,
                    vs_fpm=vs_fpm,
                    flare_active=fd["flare_active"],
                    rwy_hdg_deg=rwy_hdg,
                    xtk_rate_mps=vy,
                    xtk_int=xtk_int,
                    rud_int=rud_int,
                    gs_int=gs_int,
                    elev_int=elev_int,
                    dt=dt,
                )
                xtk_int = ap["xtk_int"]
                rud_int = ap["rud_int"]
                gs_int = ap["gs_int"]
                elev_int = ap["elev_int"]

            cur_roll += (ap["cmd_roll_deg"] - cur_roll) * min(1.0, 5.2 * dt)
            yaw_rate_dps = cur_roll * 1.42
            cur_hdg = (cur_hdg + yaw_rate_dps * dt) % 360.0
            cur_pitch += (ap["cmd_pit_deg"] - cur_pitch) * min(1.0, 4.4 * dt)
            fpa_deg = cur_pitch - aoa_trim_deg
            target_ias = 45.0 + ap["throttle"] * 38.0 - fpa_deg * 1.2
            ias_kt += (target_ias - ias_kt) * min(1.0, 1.15 * dt)
            vz_down = -spd_mps * math.tan(math.radians(fpa_deg))
            vs_fpm = -vz_down * 196.85
            x_m += vx * dt
            y_m += vy * dt
            h_agl = max(1.0, h_agl - vz_down * dt)

        self.assertEqual(
            visited_phases,
            [
                AutonomousFlightPhase.FINAL_APPROACH,
                AutonomousFlightPhase.BASE_TURN_INTERCEPT,
                AutonomousFlightPhase.FINAL_APPROACH,
                AutonomousFlightPhase.ROLLOUT_BRAKE,
            ],
        )
        self.assertTrue(touchdown_latched, "Perpendicular base-turn scenario did not reach touchdown")
        self.assertGreater(x_m, 30.0)
        self.assertLess(x_m, 650.0)
        self.assertLess(abs(y_m), 1.0, f"Expected |xtk| < 1.0m at touchdown, got {y_m:.3f}m")
        self.assertLess(abs(wrap180(cur_hdg - rwy_hdg)), 1.2)
        self.assertLess(
            max_base_turn_tgt_hdg_step,
            3.0,
            f"Expected smooth C1 BASE_TURN_INTERCEPT target heading without jumps (< 3.0 deg/frame), got {max_base_turn_tgt_hdg_step:.2f} deg",
        )


if __name__ == "__main__":
    unittest.main()
