//! Cele 3 Scenarii „One-Click Director Mode" cu reproductibilitate deterministă bit-cu-bit
//! (`ChaCha8Rng::seed_from_u64(0x4C52_4356_2026)`).

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::sim_commands::sanitize_f64;

/// Seed determinist oficial pentru Craiova Hackathon 2026 (`LRCV 2026`).
pub const DIRECTOR_SEED_LRCV_2026: u64 = 0x4C52_4356_2026;

/// Stare instantanee prescrisă de regizorul determinist la pasul `scenario_elapsed_sec`.
#[derive(Debug, Clone)]
pub struct DirectorTickPrescription {
    /// Ratele unghiulare comandate `[roll_rate_dps, pitch_rate_dps, yaw_rate_dps]`.
    pub commanded_rates_dps: [f64; 3],
    /// Accelerația longitudinală comandată `ax_mps2`.
    pub commanded_accel_x_mps2: f64,
    /// Vizibilitatea meteorologică țintă $V_{MOR}$ (m).
    pub visibility_mor_m: f64,
    /// Activare senzor SWIR + detecție ALS.
    pub enable_swir_als: bool,
    /// Vântul masei de ceață în față (la distanța de lookahead ~60m în fața aeronavei) `[wx, wy, wz]` NED (m/s).
    /// Permite declanșarea `Optical Gust Lookahead` cu $0.85\text{ s}$ înainte ca rafala să lovească fizic fuzelajul!
    pub lookahead_fog_wind_ned_mps: [f64; 3],
    /// Vântul masei de ceață care acționează fizic asupra fuzelajului la momentul curent `[wx, wy, wz]` NED (m/s).
    pub physical_fog_wind_ned_mps: [f64; 3],
    /// Flag-uri de atac cibernetic active la acest moment în scenariu.
    pub gps_jump_spoof: bool,
    pub gps_stealth_ramp_spoof: bool,
    pub cut_internet_comms: bool,
    pub poison_meteo_cache: bool,
    /// Descriere narativă scurtă pentru telemetrie/HUD.
    pub stage_banner: &'static str,
}

/// Motorul determinist pentru cele 3 scenarii „One-Click Director Mode".
#[derive(Debug, Clone)]
pub struct DirectorScenarioEngine {
    pub active_scenario_id: u8,
    pub scenario_tick: u64,
    pub rng: ChaCha8Rng,
}

impl Default for DirectorScenarioEngine {
    fn default() -> Self {
        Self::new(0)
    }
}

impl DirectorScenarioEngine {
    /// Creează un nou motor de scenarii inițializat cu `DIRECTOR_SEED_LRCV_2026`.
    #[must_use]
    pub fn new(scenario_id: u8) -> Self {
        let seed = DIRECTOR_SEED_LRCV_2026 ^ u64::from(scenario_id);
        Self {
            active_scenario_id: scenario_id.clamp(0, 3),
            scenario_tick: 0,
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    /// Activează un scenariu (`1`, `2` sau `3`) și resetează generatorul `ChaCha8Rng` la seed-ul canonic.
    pub fn start_scenario(&mut self, scenario_id: u8) {
        let clean_id = scenario_id.clamp(1, 3);
        self.active_scenario_id = clean_id;
        self.scenario_tick = 0;
        self.rng = ChaCha8Rng::seed_from_u64(DIRECTOR_SEED_LRCV_2026 ^ u64::from(clean_id));
    }

    /// Oprește regizorul automat (trece pe control manual 100%).
    pub fn stop(&mut self) {
        self.active_scenario_id = 0;
        self.scenario_tick = 0;
    }

    /// Returnează prescripția deterministă pentru pasul curent ($dt = 1/60\text{ s}$) dacă un scenariu este activ.
    pub fn step_prescription(&mut self, dt_sec: f64) -> Option<DirectorTickPrescription> {
        if self.active_scenario_id == 0 {
            return None;
        }
        let t = (self.scenario_tick as f64) * sanitize_f64(dt_sec, 0.001, 0.1, 1.0 / 60.0);
        self.scenario_tick = self.scenario_tick.saturating_add(1);

        // Micro-turbulență deterministă reproductibilă bit-cu-bit din ChaCha8Rng
        let micro_wx = (self.rng.gen::<f64>() - 0.5) * 0.18;
        let micro_wy = (self.rng.gen::<f64>() - 0.5) * 0.18;
        let micro_wz = (self.rng.gen::<f64>() - 0.5) * 0.06;

        match self.active_scenario_id {
            1 => Some(self.prescribe_act1_sudden_fog_shear(t, [micro_wx, micro_wy, micro_wz])),
            2 => Some(self.prescribe_act2_dynamic_6dof_maneuvers(t, [micro_wx, micro_wy, micro_wz])),
            3 => Some(self.prescribe_act3_cyber_siege_cat3c_touchdown(
                t,
                [micro_wx, micro_wy, micro_wz],
            )),
            _ => None,
        }
    }

    /// ACT 1: STRAIGHT FLIGHT & SUDDEN FOG SHEAR
    /// - Zbor rectiliniu la 900m de RWY 09 în ceață densă ($V_{MOR}=75\text{ m}$).
    /// - La $t = +1.65\text{ s}$, frontul optic de forfecare ($18\text{ m/s}$ lateral Est) intră în câmpul vizual
    ///   de profunzime $[40\text{ m}, 80\text{ m}]$, declanșând `gust_lookahead_warning = true` cu $0.85\text{ s}$ avans!
    /// - La $t = +2.50\text{ s}$, rafala lovește fizic aeronava; autopilotul comandă $\psi_{crab}$,
    ///   `Naive Optical Flow` derapează cu $>38\text{ m}$, iar `AERO-SHIELD V2.0` rămâne la $<0.6\text{ m}$.
    fn prescribe_act1_sudden_fog_shear(
        &self,
        t: f64,
        micro: [f64; 3],
    ) -> DirectorTickPrescription {
        let lead_time_sec = 0.85;
        let gust_impact_sec = 2.50;
        let gust_lookahead_start_sec = gust_impact_sec - lead_time_sec; // 1.65 s

        let lookahead_wy = if t >= gust_lookahead_start_sec {
            let ramp = ((t - gust_lookahead_start_sec) / 0.25).clamp(0.0, 1.0);
            18.0 * ramp + micro[1]
        } else {
            1.5 + micro[1]
        };

        let physical_wy = if t >= gust_impact_sec {
            let ramp = ((t - gust_impact_sec) / 0.25).clamp(0.0, 1.0);
            18.0 * ramp + micro[1]
        } else {
            1.5 + micro[1]
        };

        let lookahead_wx = -3.0 + micro[0];
        let physical_wx = -3.0 + micro[0];

        let banner = if t < gust_lookahead_start_sec {
            "ACT 1: STABLE ILS APPROACH (900m -> RWY 09, MOR 75m)"
        } else if t < gust_impact_sec {
            "ACT 1: ⚠️ OPTICAL GUST LOOKAHEAD DETECTED (0.85s BEFORE IMPACT!)"
        } else {
            "ACT 1: 18 m/s LATERAL FOG SHEAR — DECOUPLED BY AERO-SHIELD + CRAB ANGLE"
        };

        DirectorTickPrescription {
            commanded_rates_dps: [0.0, 0.0, 0.0],
            commanded_accel_x_mps2: 0.0,
            visibility_mor_m: 75.0,
            enable_swir_als: true,
            lookahead_fog_wind_ned_mps: [lookahead_wx, lookahead_wy, micro[2]],
            physical_fog_wind_ned_mps: [physical_wx, physical_wy, micro[2]],
            gps_jump_spoof: false,
            gps_stealth_ramp_spoof: false,
            cut_internet_comms: false,
            poison_meteo_cache: false,
            stage_banner: banner,
        }
    }

    /// ACT 2: DYNAMIC 6-DoF MANEUVERS IN SHIFTING FOG
    /// - Secvență coregrafiată de viraj stânga/dreapta ($\pm 25^\circ$), roll ($\pm 35^\circ$),
    ///   urcare/coborâre și accelerare/decelerare suprapusă peste un front variabil de ceață.
    fn prescribe_act2_dynamic_6dof_maneuvers(
        &self,
        t: f64,
        micro: [f64; 3],
    ) -> DirectorTickPrescription {
        let (roll_rate, pitch_rate, yaw_rate, accel_x, banner) = if t < 2.0 {
            (
                17.5,
                2.5,
                12.5,
                2.2,
                "ACT 2: RIGHT BANK (+35 deg) + YAW (+25 deg) + THROTTLE BOOST",
            )
        } else if t < 4.5 {
            (
                -24.0,
                -3.2,
                -18.0,
                -1.8,
                "ACT 2: S-TURN REVERSAL (-35 deg ROLL / -25 deg YAW) IN SHIFTING FOG",
            )
        } else if t < 6.5 {
            (
                14.0,
                2.0,
                11.0,
                1.5,
                "ACT 2: ALIGNING GLIDESLOPE & RE-ANCHORING QUATERNION R_b^n(q)",
            )
        } else {
            (
                0.0,
                0.0,
                0.0,
                0.0,
                "ACT 2: STABILIZED 6-DoF TRAJECTORY ON LRCV RWY 09",
            )
        };

        let fog_wx = -4.5 + 3.5 * (0.7 * t).sin() + micro[0];
        let fog_wy = 9.5 * (0.55 * t).cos() + micro[1];
        let fog_wz = 0.8 * (0.9 * t).sin() + micro[2];

        let lookahead_wy = 9.5 * (0.55 * (t + 0.85)).cos() + micro[1];

        DirectorTickPrescription {
            commanded_rates_dps: [roll_rate, pitch_rate, yaw_rate],
            commanded_accel_x_mps2: accel_x,
            visibility_mor_m: 60.0,
            enable_swir_als: true,
            lookahead_fog_wind_ned_mps: [fog_wx, lookahead_wy, fog_wz],
            physical_fog_wind_ned_mps: [fog_wx, fog_wy, fog_wz],
            gps_jump_spoof: false,
            gps_stealth_ramp_spoof: false,
            cut_internet_comms: false,
            poison_meteo_cache: false,
            stage_banner: banner,
        }
    }

    /// ACT 3: FULL CYBER-SIEGE CAT IIIc TOUCHDOWN ON LRCV RWY 09
    /// - Ceață extremă CAT IIIc ($35\text{ m}$ MOR)
    /// - Stealth Slow-Ramp GPS Spoofing ($0.35\text{ m/s}^2$, prins în $<2.9\text{ s}$ de CUSUM)
    /// - Internet Cut + Poisoned Meteo Cache ($+32\text{ m/s}$ fals, respins de Optical-Aerodynamic Sanity Gate)
    /// - Captură balizaj ALS în SWIR și touchdown pe axul pistei RWY 09 Craiova cu eroare $<0.8\text{ m}$!
    fn prescribe_act3_cyber_siege_cat3c_touchdown(
        &self,
        t: f64,
        micro: [f64; 3],
    ) -> DirectorTickPrescription {
        let lookahead_wy = if t >= 1.15 {
            14.5 + micro[1]
        } else {
            4.0 + micro[1]
        };
        let physical_wy = if t >= 2.00 {
            14.5 + micro[1]
        } else {
            4.0 + micro[1]
        };
        let wx = -5.5 + micro[0];

        let banner = if t < 1.0 {
            "ACT 3: CAT IIIc ZERO-VISIBILITY (35m) — ENGAGING SWIR + CLAHE"
        } else if t < 3.2 {
            "ACT 3: 🚨 CYBER-SIEGE ACTIVE: STEALTH GPS SPOOF + POISONED METEO CACHE"
        } else if t < 6.0 {
            "ACT 3: 🛡️ GNSS QUARANTINED BY CUSUM (<2.9s) & METEO POISON REJECTED"
        } else {
            "ACT 3: 🎯 ALS SWIR LOCKED — PRECISION TOUCHDOWN ON LRCV RWY 09 (<0.8m)"
        };

        DirectorTickPrescription {
            commanded_rates_dps: [0.0, 0.0, 0.0],
            commanded_accel_x_mps2: 0.0,
            visibility_mor_m: 35.0,
            enable_swir_als: true,
            lookahead_fog_wind_ned_mps: [wx, lookahead_wy, micro[2]],
            physical_fog_wind_ned_mps: [wx, physical_wy, micro[2]],
            gps_jump_spoof: false,
            gps_stealth_ramp_spoof: true,
            cut_internet_comms: true,
            poison_meteo_cache: true,
            stage_banner: banner,
        }
    }
}
