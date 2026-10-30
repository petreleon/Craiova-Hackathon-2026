//! `aero-meteo-cyber` — Lock-Free Open-Meteo & METAR LRCV Ingestor (`ArcSwap`),
//! Optical-Aerodynamic Meteo Sanity Gate, and Dual-Layer Federated GNSS Anti-Spoofing Watchdog.
//!
//! Designed for **AERO-SHIELD LRCV V2.0 (`FogVision 6-DoF — Bulletproof Edition`)** at
//! Craiova International Airport (`LRCV`, RWY 09 `44.3181° N, 23.8886° E`).

pub mod gnss_watchdog_cusum;
pub mod metar_parser;
pub mod meteo_sanity_gate;
pub mod open_meteo_lrcv;

// Specification module aliases from `arhitectura/04_aero_meteo_cyber.md`
pub use gnss_watchdog_cusum as gnss_dual_watchdog;
pub use open_meteo_lrcv as meteo_ingestor_lrcv;

pub use gnss_watchdog_cusum::{
    build_diagonal_innovation_inv_sqrt, compute_safe_cov_inv_sqrt_6x6, CyberAttackSandbox,
    DualLayerGnssWatchdog, GnssAttackScenario, GnssIntegrityStatus, SandboxSensorObservation,
    SlidingWindowVectorCusum, BANNER_GNSS_NOMINAL, BANNER_GNSS_QUARANTINED_FORTRESS,
    DEFAULT_CHI2_THRESHOLD_6DF, DEFAULT_CUSUM_THRESHOLD, DEFAULT_DRIFT_ALLOWANCE_NU,
};
pub use metar_parser::{
    compute_koschmieder_rho_fog_opt, parse_metar_lrcv, CloudCoverage, CloudLayerMetar,
    IcaoApproachCategory, MetarParseError, ParsedMetarLrcv, RunwayVisualRange, RvrBoundQualifier,
    RvrTrend, WeatherPhenomenon,
};
pub use meteo_sanity_gate::{
    compute_aerodynamic_wind_triangle_ned, compute_physical_wind_consensus_ned,
    fuse_meteo_prior_with_physical_consensus, verify_meteo_cache_integrity,
    MeteoSanityDiagnostics, MeteoSanityVerdict, ALERT_METEO_VERIFIED,
    ALERT_POISONED_METEO_REJECTED, METEO_SANITY_DISCREPANCY_THRESHOLD_MPS,
};
pub use open_meteo_lrcv::{
    met_wind_to_ned_vector, ned_vector_to_met_wind, AltitudeWindLayer, LrcvMeteoIngestor,
    MeteoIngestError, MeteoLayerProfileLrcv, OpenMeteoCurrentBlock, OpenMeteoHourlyBlock,
    OpenMeteoResponse, ALT_LRCV_MSL, DEFAULT_LRCV_CACHE_PATH, HELLMANN_ALPHA_RUNWAY,
    LAT_LRCV_RWY09, LON_LRCV_RWY09, OPEN_METEO_LRCV_URL, RWY09_HEADING_DEG,
};

use std::sync::Arc;

use nalgebra::{SMatrix, SVector, UnitQuaternion, Vector3};
use serde::Serialize;

/// Unified `60 Hz` Cyber-Meteo frame report consumed by the Error-State EKF (`aero-math-6dof`)
/// and the Glass Cockpit WebSocket server (`aero-cockpit-server`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CyberMeteoFrameReport {
    /// Active LRCV multi-layer weather snapshot read wait-free via `ArcSwap`.
    pub meteo_snapshot: MeteoLayerProfileLrcv,
    /// Latest parsed METAR LRCV report, if one has been ingested.
    pub latest_metar: Option<ParsedMetarLrcv>,
    /// Sanity Gate diagnostics & effective fused wind vector in `NED` (`m/s`).
    pub meteo_sanity: MeteoSanityDiagnostics,
    /// Validated 3D wind/fog prior vector $\bar{\mathbf{w}}_{validated}(h_{AGL})$ in `NED` (`m/s`).
    pub validated_wind_ned_mps: [f64; 3],
    /// Koschmieder optical inverse-depth scale anchor $\hat{\rho}_{fog, opt} = \hat{\beta}_{ext}$ (`m^-1`).
    pub koschmieder_rho_fog_opt_inv_m: f64,
    /// Dual-Layer Federated GNSS Watchdog integrity verdict ($\chi^2$ + Sliding-Window Vector CUSUM).
    pub gnss_status: GnssIntegrityStatus,
    /// Coherent 5-second GLRT norm from the sliding-window GNSS watchdog.
    pub gnss_coherent_glrt_norm: f64,
    /// True if GNSS is quarantined (`FORTRESS OPTICAL-INERTIAL MODE ACTIVE`).
    pub is_gnss_quarantined: bool,
    /// True if `CUT INTERNET / COMMS BLACKOUT` mode is active.
    pub is_comms_blackout: bool,
}

/// Top-level orchestrator integrating `LrcvMeteoIngestor` (`ArcSwap`), `ParsedMetarLrcv`,
/// `MeteoSanityGate`, `DualLayerGnssWatchdog`, and the Art. 21 `CyberAttackSandbox`.
pub struct CyberMeteoShieldLrcv {
    pub meteo_ingestor: LrcvMeteoIngestor,
    pub gnss_watchdog: DualLayerGnssWatchdog,
    pub attack_sandbox: CyberAttackSandbox,
    latest_metar: Option<ParsedMetarLrcv>,
    s_inv_sqrt_6x6: SMatrix<f64, 6, 6>,
}

impl Default for CyberMeteoShieldLrcv {
    fn default() -> Self {
        Self::new()
    }
}

impl CyberMeteoShieldLrcv {
    /// Creates a new `CyberMeteoShieldLrcv` initialized with the Craiova LRCV RWY 09 CAT IIIc profile.
    pub fn new() -> Self {
        Self {
            meteo_ingestor: LrcvMeteoIngestor::with_default_cache(),
            gnss_watchdog: DualLayerGnssWatchdog::new(),
            attack_sandbox: CyberAttackSandbox::default(),
            latest_metar: parse_metar_lrcv(
                "METAR LRCV 301430Z 09006KT 0150 R09/0175N FG VV001 04/04 Q1022",
            )
            .ok(),
            s_inv_sqrt_6x6: build_diagonal_innovation_inv_sqrt(0.30, 0.15),
        }
    }

    /// Ingests a new `METAR LRCV` or `SPECI LRCV` string and updates the Koschmieder optical anchor.
    pub fn ingest_metar(&mut self, raw_metar: &str) -> Result<ParsedMetarLrcv, MetarParseError> {
        let parsed = parse_metar_lrcv(raw_metar)?;
        let current = self.meteo_ingestor.load_snapshot();
        let mut updated = (*current).clone();
        updated.visibility_mor_m = parsed.prevailing_visibility_mor_m;
        updated.koschmieder_beta_ext_inv_m = parsed.koschmieder_rho_fog_opt_inv_m;
        updated.temperature_2m_c = parsed.temperature_c;
        updated.dew_point_2m_c = parsed.dew_point_c;
        updated.surface_pressure_hpa = parsed.qnh_hpa;
        self.meteo_ingestor.store_snapshot(updated);
        self.latest_metar = Some(parsed.clone());
        Ok(parsed)
    }

    /// Sets the `CUT INTERNET / COMMS BLACKOUT` state (`< 1 us` atomic switch).
    #[inline]
    pub fn set_comms_blackout(&self, enabled: bool) {
        self.meteo_ingestor.set_comms_blackout(enabled);
    }

    /// Sets the active Art. 21 sandbox cyber-attack scenario.
    #[inline]
    pub fn set_attack_scenario(&mut self, scenario: GnssAttackScenario) {
        self.attack_sandbox.active_scenario = scenario;
    }

    /// Executes one deterministic `60 Hz` Cyber-Meteo verification & fusion step.
    ///
    /// - Reads the cached weather profile wait-free via `ArcSwap`,
    /// - Verifies cache integrity against onboard optical fog velocity $\hat{\mathbf{w}}_{fog, opt}$
    ///   and the quaternion aerodynamic triangle $\hat{\mathbf{w}}_{aero}$,
    /// - Updates the cached profile recursively via Bayesian fusion if `Comms Blackout` is active and
    ///   the cache was not poisoned,
    /// - Evaluates the Dual-Layer GNSS Watchdog ($\chi^2$ + `SlidingWindowVectorCusum`) against the
    ///   isolated Optical-Inertial 6-DoF state estimate.
    #[allow(clippy::too_many_arguments)]
    pub fn step_60hz(
        &mut self,
        h_agl_m: f64,
        p_opt_inertial_ned_m: Vector3<f64>,
        v_opt_inertial_ned_mps: Vector3<f64>,
        q_body_to_ned: &UnitQuaternion<f64>,
        v_true_airspeed_body_mps: Vector3<f64>,
        w_optical_fog_ned_mps: Vector3<f64>,
        optical_fog_variance_mps2: f64,
        p_gnss_ned_m: Option<Vector3<f64>>,
        v_gnss_ned_mps: Option<Vector3<f64>>,
    ) -> CyberMeteoFrameReport {
        // 1. Wait-free load of the atmospheric profile (< 1 us)
        let mut active_profile: Arc<MeteoLayerProfileLrcv> = self.meteo_ingestor.load_snapshot();

        // 2. Interpolate cached wind at current aircraft altitude h_AGL (including Hellmann < 10m)
        let w_cache_ned = match self.attack_sandbox.active_scenario {
            GnssAttackScenario::PreCutMeteoCachePoisoning { fake_wind_ned_mps } => {
                fake_wind_ned_mps
            }
            _ => active_profile.interpolate_wind_ned_at_agl(h_agl_m),
        };

        // 3. Compute aerodynamic velocity triangle and run Optical-Aerodynamic Meteo Sanity Gate
        let w_aero_ned = compute_aerodynamic_wind_triangle_ned(
            v_opt_inertial_ned_mps,
            q_body_to_ned,
            v_true_airspeed_body_mps,
        );
        let (w_validated_ned, meteo_sanity) = fuse_meteo_prior_with_physical_consensus(
            w_cache_ned,
            w_optical_fog_ned_mps,
            w_aero_ned,
        );

        // 4. If in Comms Blackout mode and cache is not poisoned, perform recursive Bayesian update
        if self.meteo_ingestor.is_comms_blackout() && meteo_sanity.verdict.is_cache_trusted {
            active_profile = self.meteo_ingestor.update_from_optical_fog(
                h_agl_m,
                w_optical_fog_ned_mps,
                optical_fog_variance_mps2,
            );
        }

        // 5. Evaluate Dual-Layer Federated GNSS Watchdog
        let gnss_status = match (p_gnss_ned_m, v_gnss_ned_mps) {
            (Some(p_gps), Some(v_gps)) => {
                let dp = p_gps - p_opt_inertial_ned_m;
                let dv = v_gps - v_opt_inertial_ned_mps;
                let innov_6d = SVector::<f64, 6>::new(dp.x, dp.y, dp.z, dv.x, dv.y, dv.z);
                self.gnss_watchdog
                    .evaluate_gnss_sample(&innov_6d, &self.s_inv_sqrt_6x6)
            }
            _ => {
                self.gnss_watchdog.is_quarantined = true;
                GnssIntegrityStatus::QuarantinedJammingOrLoss {
                    reason: "RF JAMMING / LOSS OF GNSS SATELLITE TRACKING",
                }
            }
        };

        let koschmieder_rho = self
            .latest_metar
            .as_ref()
            .map(|m| m.koschmieder_rho_fog_opt_inv_m)
            .unwrap_or(active_profile.koschmieder_beta_ext_inv_m);

        CyberMeteoFrameReport {
            meteo_snapshot: (*active_profile).clone(),
            latest_metar: self.latest_metar.clone(),
            meteo_sanity,
            validated_wind_ned_mps: [w_validated_ned.x, w_validated_ned.y, w_validated_ned.z],
            koschmieder_rho_fog_opt_inv_m: koschmieder_rho,
            gnss_status,
            gnss_coherent_glrt_norm: self.gnss_watchdog.coherent_glrt_norm(),
            is_gnss_quarantined: self.gnss_watchdog.is_quarantined,
            is_comms_blackout: self.meteo_ingestor.is_comms_blackout(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integrated_cyber_meteo_shield_simultaneous_multi_vector_attack() {
        let mut shield = CyberMeteoShieldLrcv::new();

        // 1. Ingest CAT IIIc METAR for LRCV RWY 09
        let metar_res = shield
            .ingest_metar("METAR LRCV 301430Z 09006KT 0150 R09/0175N FG VV001 04/04 Q1022");
        assert!(metar_res.is_ok());

        // 2. Attacker poisons meteo cache (+25 m/s North wind) right before cutting internet
        shield.set_attack_scenario(GnssAttackScenario::PreCutMeteoCachePoisoning {
            fake_wind_ned_mps: Vector3::new(-25.0, 0.0, 0.0),
        });
        shield.set_comms_blackout(true);

        let q_east = UnitQuaternion::from_euler_angles(0.0, 0.0, std::f64::consts::FRAC_PI_2);
        let p_true = Vector3::new(0.0, -1200.0, -80.0);
        let v_true = Vector3::new(0.0, 62.0, 3.0);
        let v_tas_body = Vector3::new(65.1, 0.0, 3.0);
        let w_opt_fog = Vector3::new(0.0, -3.1, 0.0);

        let report_poison = shield.step_60hz(
            80.0,
            p_true,
            v_true,
            &q_east,
            v_tas_body,
            w_opt_fog,
            0.25,
            Some(p_true),
            Some(v_true),
        );

        // Sanity Gate must reject the poisoned cache and output 100% physical consensus
        assert!(!report_poison.meteo_sanity.verdict.is_cache_trusted);
        assert_eq!(report_poison.meteo_sanity.verdict.trust_weight, 0.0);
        assert!(report_poison.is_comms_blackout);
        assert!((report_poison.validated_wind_ned_mps[1] - (-3.1)).abs() < 1e-6);

        // 3. Attacker switches to Stealth Slow-Ramp GPS Spoofing (0.20 m/s lateral pull)
        shield.set_attack_scenario(GnssAttackScenario::StealthSlowRampSpoofing {
            drift_rate_ned_mps: Vector3::new(0.20, 0.0, 0.0),
            start_time_s: 0.0,
        });

        let dt = 1.0 / 60.0;
        let mut quarantined_at_frame: Option<usize> = None;

        for frame in 1..=180 {
            let t_s = (frame as f64) * dt;
            let obs = shield.attack_sandbox.synthesize_observation(
                t_s,
                p_true,
                v_true,
                Vector3::zeros(),
                Vector3::zeros(),
            );

            let rep = shield.step_60hz(
                80.0,
                p_true,
                v_true,
                &q_east,
                v_tas_body,
                w_opt_fog,
                0.25,
                obs.p_gnss_ned_m,
                obs.v_gnss_ned_mps,
            );

            if rep.is_gnss_quarantined {
                quarantined_at_frame = Some(frame);
                assert!(matches!(
                    rep.gnss_status,
                    GnssIntegrityStatus::QuarantinedStealthRampSpoof { .. }
                ));
                break;
            }
        }

        // Must quarantine Stealth Slow-Ramp GPS Spoofing in well under 3.2s (192 frames)
        assert!(quarantined_at_frame.is_some());
        if let Some(f) = quarantined_at_frame {
            assert!(f < 150);
        }
    }
}
