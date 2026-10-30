//! Dual-Layer Federated GNSS Watchdog & Art. 21 Cyber-Attack Sandbox (`gnss_watchdog_cusum.rs`).
//!
//! Implements Remediation #7 from the Red-Team Audit:
//! - **Layer 1 (Instantaneous Mahalanobis $\chi^2$ Gate, `< 20 ms`)**:
//!   $$\gamma_{inst}(t_k) = \mathbf{r}_k^T \mathbf{S}_k^{-1} \mathbf{r}_k > \tau_{\chi^2} = 22.46 \quad (\chi^2_{6,\,0.999})$$
//!   Catches *Jump Spoofing* in a single `60 Hz` frame (`16.67 ms`).
//! - **Layer 2 (Sliding-Window Directional Vector CUSUM / GLRT, `< 3.2 s`)**:
//!   Accumulates whitened 6D position-velocity residuals $\tilde{\mathbf{r}}_k = \mathbf{S}_k^{-1/2}\mathbf{r}_k$
//!   across a `5.0 s` (`300`-sample at `60 Hz`) horizon without polluting the isolated optical-inertial
//!   federated filter. Catches insidious *Stealth Slow-Ramp Spoofing* (`0.15 - 0.30 m/s` lateral drift)
//!   in `< 3.2 s` before lateral runway deviation reaches `0.5 m`.
//! - **Art. 21 Cybersecurity Sandbox**:
//!   100% software-contained simulation of `CleanNominal`, `GpsJammingBlackout`, `JumpSpoofing`,
//!   `StealthSlowRampSpoofing`, and `PreCutMeteoCachePoisoning`.

use std::collections::VecDeque;

use nalgebra::{SMatrix, SVector, Vector3};
use serde::{Deserialize, Serialize};

/// Default $\chi^2_{6, 0.999}$ threshold (`df = 6, p = 0.999`) for Layer 1 instantaneous Jump Spoofing gate.
pub const DEFAULT_CHI2_THRESHOLD_6DF: f64 = 22.46_f64;

/// Default directional Vector CUSUM threshold for Layer 2 Stealth Slow-Ramp Spoofing detection (`< 3.2 s`).
pub const DEFAULT_CUSUM_THRESHOLD: f64 = 8.50_f64;

/// Default Page's CUSUM drift allowance parameter $\nu_{drift}$ (`0.08`).
pub const DEFAULT_DRIFT_ALLOWANCE_NU: f64 = 0.08_f64;

/// Sliding window capacity at `60 Hz` corresponding to a `5.0 s` GLRT/CUSUM observation horizon (`300` frames).
pub const SLIDING_WINDOW_FRAMES_60HZ: usize = 300;

/// Cockpit status banner when GNSS is nominal and verified against optical-inertial reference.
pub const BANNER_GNSS_NOMINAL: &str = "GNSS VERIFIED BY OPTICAL-INERTIAL FEDERATED WATCHDOG";

/// Cockpit status banner when GNSS is quarantined and the aircraft switches to Fortress Optical-Inertial mode.
pub const BANNER_GNSS_QUARANTINED_FORTRESS: &str =
    "GPS QUARANTINED — FORTRESS OPTICAL-INERTIAL MODE ACTIVE";

/// Integrity verdict emitted at `60 Hz` by the `DualLayerGnssWatchdog`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum GnssIntegrityStatus {
    /// GNSS agrees with the isolated optical-inertial 6-DoF reference within both $\chi^2$ and CUSUM bounds.
    Nominal {
        chi2_stat: f64,
        cusum_norm: f64,
    },
    /// Layer 1 triggered: Instantaneous Mahalanobis $\chi^2 > 22.46$ detected *Jump Spoofing* (`< 20 ms`).
    QuarantinedJumpSpoof {
        chi2_stat: f64,
        cusum_norm: f64,
    },
    /// Layer 2 triggered: Vector CUSUM norm $> 8.50$ detected *Stealth Slow-Ramp Spoofing* (`< 3.2 s`).
    QuarantinedStealthRampSpoof {
        chi2_stat: f64,
        cusum_norm: f64,
    },
    /// GNSS receiver signal lost, jammed (`C/N0` collapse), or containing non-finite coordinates.
    QuarantinedJammingOrLoss {
        reason: &'static str,
    },
}

impl GnssIntegrityStatus {
    /// Returns `true` if the status represents a quarantined / untrusted GNSS state.
    #[inline]
    pub fn is_quarantined(&self) -> bool {
        !matches!(self, Self::Nominal { .. })
    }

    /// Returns the cockpit alert banner corresponding to this integrity status.
    #[inline]
    pub fn cockpit_banner(&self) -> &'static str {
        match self {
            Self::Nominal { .. } => BANNER_GNSS_NOMINAL,
            _ => BANNER_GNSS_QUARANTINED_FORTRESS,
        }
    }
}

/// 5-second (`300`-sample at `60 Hz`) Sliding-Window Vector CUSUM & GLRT accumulator.
///
/// Tracks bounded-horizon directional innovations so long-duration approaches never accumulate
/// unbounded random walk, while coherent slow-ramp spoofing (`0.15 - 0.30 m/s`) is caught rapidly.
#[derive(Debug, Clone)]
pub struct SlidingWindowVectorCusum {
    window_capacity: usize,
    history: VecDeque<SVector<f64, 6>>,
    running_sum: SVector<f64, 6>,
    cusum_pos: SVector<f64, 6>,
    cusum_neg: SVector<f64, 6>,
    drift_allowance_nu: f64,
}

impl SlidingWindowVectorCusum {
    /// Creates a new `SlidingWindowVectorCusum` with the specified sample capacity and drift allowance $\nu$.
    pub fn new(window_capacity: usize, drift_allowance_nu: f64) -> Self {
        let cap = window_capacity.max(10);
        Self {
            window_capacity: cap,
            history: VecDeque::with_capacity(cap),
            running_sum: SVector::<f64, 6>::zeros(),
            cusum_pos: SVector::<f64, 6>::zeros(),
            cusum_neg: SVector::<f64, 6>::zeros(),
            drift_allowance_nu: drift_allowance_nu.max(0.0),
        }
    }

    /// Pushes a whitened 6D innovation vector $\tilde{\mathbf{r}}_k = \mathbf{S}_k^{-1/2}\mathbf{r}_k$
    /// and returns the updated 6D directional CUSUM vector $\mathbf{S}_{CUSUM}(t_k)$.
    pub fn push_whitened(&mut self, whitened: &SVector<f64, 6>) -> SVector<f64, 6> {
        if self.history.len() >= self.window_capacity {
            if let Some(oldest) = self.history.pop_front() {
                self.running_sum -= oldest;
            }
        }
        self.history.push_back(*whitened);
        self.running_sum += *whitened;

        let mut out = SVector::<f64, 6>::zeros();
        for i in 0..6 {
            let w_i = if whitened[i].is_finite() {
                whitened[i]
            } else {
                0.0
            };
            self.cusum_pos[i] = (self.cusum_pos[i] + w_i - self.drift_allowance_nu).max(0.0);
            self.cusum_neg[i] = (self.cusum_neg[i] - w_i - self.drift_allowance_nu).max(0.0);
            out[i] = self.cusum_pos[i].max(self.cusum_neg[i]);
        }
        out
    }

    /// Returns the coherent sliding-window GLRT statistic $\|\sum_{k \in W} \tilde{\mathbf{r}}_k\|_2 / \sqrt{|W|}$.
    pub fn coherent_glrt_norm(&self) -> f64 {
        let n = self.history.len();
        if n == 0 {
            return 0.0;
        }
        self.running_sum.norm() / (n as f64).sqrt()
    }

    /// Resets the sliding window and CUSUM accumulators to zero.
    pub fn reset(&mut self) {
        self.history.clear();
        self.running_sum = SVector::<f64, 6>::zeros();
        self.cusum_pos = SVector::<f64, 6>::zeros();
        self.cusum_neg = SVector::<f64, 6>::zeros();
    }
}

/// Dual-Layer Federated GNSS Watchdog combining Layer 1 ($\chi^2$ Mahalanobis Gate) and
/// Layer 2 (`SlidingWindowVectorCusum` directional ramp detector).
#[derive(Debug, Clone)]
pub struct DualLayerGnssWatchdog {
    /// Active 6D CUSUM state vector $\mathbf{S}_{CUSUM}(t_k)$.
    pub cusum_vec: SVector<f64, 6>,
    /// Layer 1 $\chi^2$ threshold (`22.46` for `df = 6, p = 0.999`).
    pub chi2_threshold: f64,
    /// Layer 2 Vector CUSUM Euclidean norm threshold (`8.50`).
    pub cusum_threshold: f64,
    /// Page's CUSUM drift allowance parameter $\nu$ (`0.08`).
    pub drift_allowance_nu: f64,
    /// Permanent quarantine latch (`true` once spoofing or jamming is detected until explicit reset).
    pub is_quarantined: bool,
    /// Sliding-window directional accumulator (`300` samples = `5.0 s` at `60 Hz`).
    sliding_window: SlidingWindowVectorCusum,
    /// Last integrity verdict for telemetry reporting.
    last_status: GnssIntegrityStatus,
}

impl Default for DualLayerGnssWatchdog {
    fn default() -> Self {
        Self::new()
    }
}

impl DualLayerGnssWatchdog {
    /// Creates a new `DualLayerGnssWatchdog` configured with the canonical Craiova LRCV V2.0 thresholds.
    pub fn new() -> Self {
        Self {
            cusum_vec: SVector::<f64, 6>::zeros(),
            chi2_threshold: DEFAULT_CHI2_THRESHOLD_6DF,
            cusum_threshold: DEFAULT_CUSUM_THRESHOLD,
            drift_allowance_nu: DEFAULT_DRIFT_ALLOWANCE_NU,
            is_quarantined: false,
            sliding_window: SlidingWindowVectorCusum::new(
                SLIDING_WINDOW_FRAMES_60HZ,
                DEFAULT_DRIFT_ALLOWANCE_NU,
            ),
            last_status: GnssIntegrityStatus::Nominal {
                chi2_stat: 0.0,
                cusum_norm: 0.0,
            },
        }
    }

    /// Evaluates a 6D GNSS vs. Optical-Inertial innovation vector $\mathbf{r}_k = [\Delta\mathbf{p}^T, \Delta\mathbf{v}^T]^T$
    /// using the inverse square-root innovation covariance matrix $\mathbf{S}_k^{-1/2}$.
    ///
    /// Once spoofing is detected by either Layer 1 ($\chi^2 > 22.46$) or Layer 2 ($\|\mathbf{S}_{CUSUM}\|_2 > 8.50$),
    /// `self.is_quarantined` is latched to `true` so the federated optical-inertial loop remains 100% isolated.
    pub fn evaluate_gnss_sample(
        &mut self,
        innovation_6d: &SVector<f64, 6>,
        s_cov_inv_sqrt: &SMatrix<f64, 6, 6>,
    ) -> GnssIntegrityStatus {
        if !innovation_6d.iter().all(|x| x.is_finite())
            || !s_cov_inv_sqrt.iter().all(|x| x.is_finite())
        {
            self.is_quarantined = true;
            let status = GnssIntegrityStatus::QuarantinedJammingOrLoss {
                reason: "NON-FINITE GNSS INNOVATION OR COVARIANCE",
            };
            self.last_status = status;
            return status;
        }

        let whitened: SVector<f64, 6> = s_cov_inv_sqrt * innovation_6d;
        let chi2_stat = whitened.dot(&whitened);

        // Update directional sliding-window Vector CUSUM (prevents sign cancellation on coherent ramps
        // while allowing zero-mean thermal/multipath oscillations to cancel out).
        self.cusum_vec = self.sliding_window.push_whitened(&whitened);
        let cusum_norm = self.cusum_vec.norm();

        if chi2_stat > self.chi2_threshold {
            self.is_quarantined = true;
            let status = GnssIntegrityStatus::QuarantinedJumpSpoof {
                chi2_stat,
                cusum_norm,
            };
            self.last_status = status;
            status
        } else if cusum_norm > self.cusum_threshold {
            self.is_quarantined = true;
            let status = GnssIntegrityStatus::QuarantinedStealthRampSpoof {
                chi2_stat,
                cusum_norm,
            };
            self.last_status = status;
            status
        } else if self.is_quarantined {
            // Permanent quarantine latch: once an attacker has spoofed/jammed GNSS during approach,
            // GNSS remains quarantined unless explicitly reset by crew/system.
            match self.last_status {
                GnssIntegrityStatus::QuarantinedJumpSpoof { .. } => {
                    GnssIntegrityStatus::QuarantinedJumpSpoof {
                        chi2_stat,
                        cusum_norm,
                    }
                }
                GnssIntegrityStatus::QuarantinedJammingOrLoss { reason } => {
                    GnssIntegrityStatus::QuarantinedJammingOrLoss { reason }
                }
                _ => GnssIntegrityStatus::QuarantinedStealthRampSpoof {
                    chi2_stat,
                    cusum_norm,
                },
            }
        } else {
            let status = GnssIntegrityStatus::Nominal {
                chi2_stat,
                cusum_norm,
            };
            self.last_status = status;
            status
        }
    }

    /// Evaluates raw 3D position and velocity measurements from the GNSS receiver against the
    /// isolated Optical-Inertial 6-DoF state estimate $(\hat{\mathbf{p}}_{opt}, \hat{\mathbf{v}}_{opt})$.
    pub fn evaluate_position_velocity(
        &mut self,
        p_gnss_ned_m: Option<Vector3<f64>>,
        v_gnss_ned_mps: Option<Vector3<f64>>,
        p_opt_inertial_ned_m: Vector3<f64>,
        v_opt_inertial_ned_mps: Vector3<f64>,
        s_cov_6x6: &SMatrix<f64, 6, 6>,
    ) -> GnssIntegrityStatus {
        let (p_gps, v_gps) = match (p_gnss_ned_m, v_gnss_ned_mps) {
            (Some(p), Some(v)) => (p, v),
            _ => {
                self.is_quarantined = true;
                let status = GnssIntegrityStatus::QuarantinedJammingOrLoss {
                    reason: "RF JAMMING / LOSS OF GNSS SATELLITE TRACKING",
                };
                self.last_status = status;
                return status;
            }
        };

        let dp = p_gps - p_opt_inertial_ned_m;
        let dv = v_gps - v_opt_inertial_ned_mps;
        let innovation_6d = SVector::<f64, 6>::new(dp.x, dp.y, dp.z, dv.x, dv.y, dv.z);
        let s_inv_sqrt = compute_safe_cov_inv_sqrt_6x6(s_cov_6x6);

        self.evaluate_gnss_sample(&innovation_6d, &s_inv_sqrt)
    }

    /// Explicitly clears the quarantine latch and resets the CUSUM state.
    pub fn reset_quarantine(&mut self) {
        self.cusum_vec = SVector::<f64, 6>::zeros();
        self.is_quarantined = false;
        self.sliding_window.reset();
        self.last_status = GnssIntegrityStatus::Nominal {
            chi2_stat: 0.0,
            cusum_norm: 0.0,
        };
    }

    /// Returns the coherent sliding-window GLRT norm for cockpit diagnostics.
    #[inline]
    pub fn coherent_glrt_norm(&self) -> f64 {
        self.sliding_window.coherent_glrt_norm()
    }
}

/// Safely computes the symmetric inverse square-root matrix $\mathbf{S}^{-1/2}$ of a $6 \times 6$
/// innovation covariance matrix without `.unwrap()` or panic.
///
/// Uses Cholesky decomposition when $\mathbf{S}$ is strictly positive-definite, falling back to
/// diagonal regularization if near-singular.
pub fn compute_safe_cov_inv_sqrt_6x6(s_cov: &SMatrix<f64, 6, 6>) -> SMatrix<f64, 6, 6> {
    // Symmetrize and add a tiny positive-definite floor (1e-6) on the diagonal
    let mut reg = 0.5 * (s_cov + s_cov.transpose());
    for i in 0..6 {
        let diag_val = reg[(i, i)];
        reg[(i, i)] = if diag_val.is_finite() {
            diag_val.max(1e-4)
        } else {
            1.0
        };
    }

    if let Some(chol) = reg.cholesky() {
        // L * L^T = S  =>  S^{-1/2} = L^{-1} so that ||L^{-1} r||_2^2 = r^T L^{-T} L^{-1} r = r^T S^{-1} r
        let l_mat = chol.l();
        if let Some(l_inv) = l_mat.try_inverse() {
            return l_inv;
        }
    }

    // Deterministic diagonal fallback if off-diagonal entries are ill-conditioned
    let mut diag_inv_sqrt = SMatrix::<f64, 6, 6>::zeros();
    for i in 0..6 {
        let sigma2 = reg[(i, i)].max(1e-4);
        diag_inv_sqrt[(i, i)] = 1.0 / sigma2.sqrt();
    }
    diag_inv_sqrt
}

/// Constructs a canonical diagonal innovation covariance $\mathbf{S}$ and its inverse square-root
/// $\mathbf{S}^{-1/2}$ from position standard deviation $\sigma_p$ (`m`) and velocity standard
/// deviation $\sigma_v$ (`m/s`).
pub fn build_diagonal_innovation_inv_sqrt(
    sigma_pos_m: f64,
    sigma_vel_mps: f64,
) -> SMatrix<f64, 6, 6> {
    let sp = sigma_pos_m.max(1e-3);
    let sv = sigma_vel_mps.max(1e-3);
    let mut inv_sqrt = SMatrix::<f64, 6, 6>::zeros();
    for i in 0..3 {
        inv_sqrt[(i, i)] = 1.0 / sp;
        inv_sqrt[(i + 3, i + 3)] = 1.0 / sv;
    }
    inv_sqrt
}

/// Cyber-Attack Scenarios simulated 100% inside the software sandbox in compliance with
/// **Art. 21 Cybersecurity Boundaries** of the Craiova Hackathon V2.0 Regulation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum GnssAttackScenario {
    /// Clean, unjammed, unspoofed GNSS reception.
    CleanNominal,
    /// Broadband RF jamming / loss of satellite lock (`GPS JAMMING / BLACKOUT`).
    GpsJammingBlackout,
    /// Instantaneous coordinate jump offset (`Jump Spoofing`, e.g., `+50 m` or `+250 m` lateral offset).
    JumpSpoofing {
        offset_ned_m: Vector3<f64>,
    },
    /// Insidious slow-ramp spoofing (`Stealth Slow-Ramp Spoofing`, e.g., `0.15 - 0.30 m/s` lateral pull
    /// toward the edge of RWY 09 Craiova designed to evade single-frame $\chi^2$ gates).
    StealthSlowRampSpoofing {
        drift_rate_ned_mps: Vector3<f64>,
        start_time_s: f64,
    },
    /// Pre-Cut Meteo Cache Poisoning (attacker injects a false `+25 m/s` crosswind into the weather
    /// feed right before cutting internet).
    PreCutMeteoCachePoisoning {
        fake_wind_ned_mps: Vector3<f64>,
    },
}

/// Output of the Art. 21 Cybersecurity Sandbox for a given simulation timestamp `t_s`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SandboxSensorObservation {
    /// Simulated GNSS position in `NED` (`None` if jammed).
    pub p_gnss_ned_m: Option<Vector3<f64>>,
    /// Simulated GNSS velocity in `NED` (`None` if jammed).
    pub v_gnss_ned_mps: Option<Vector3<f64>>,
    /// Injected false wind vector if `PreCutMeteoCachePoisoning` is active (`None` otherwise).
    pub poisoned_meteo_wind_ned_mps: Option<Vector3<f64>>,
}

/// Software-contained Cyber-Attack Sandbox (Art. 21 Compliant).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CyberAttackSandbox {
    pub active_scenario: GnssAttackScenario,
}

impl Default for CyberAttackSandbox {
    fn default() -> Self {
        Self {
            active_scenario: GnssAttackScenario::CleanNominal,
        }
    }
}

impl CyberAttackSandbox {
    /// Creates a new `CyberAttackSandbox` with the given attack scenario.
    pub fn new(active_scenario: GnssAttackScenario) -> Self {
        Self { active_scenario }
    }

    /// Applies the active sandbox cyber-attack scenario to the true aircraft trajectory at time `t_s`.
    pub fn synthesize_observation(
        &self,
        t_s: f64,
        true_p_ned_m: Vector3<f64>,
        true_v_ned_mps: Vector3<f64>,
        sensor_pos_noise_ned_m: Vector3<f64>,
        sensor_vel_noise_ned_mps: Vector3<f64>,
    ) -> SandboxSensorObservation {
        let base_p = true_p_ned_m + sensor_pos_noise_ned_m;
        let base_v = true_v_ned_mps + sensor_vel_noise_ned_mps;

        match self.active_scenario {
            GnssAttackScenario::CleanNominal => SandboxSensorObservation {
                p_gnss_ned_m: Some(base_p),
                v_gnss_ned_mps: Some(base_v),
                poisoned_meteo_wind_ned_mps: None,
            },
            GnssAttackScenario::GpsJammingBlackout => SandboxSensorObservation {
                p_gnss_ned_m: None,
                v_gnss_ned_mps: None,
                poisoned_meteo_wind_ned_mps: None,
            },
            GnssAttackScenario::JumpSpoofing { offset_ned_m } => SandboxSensorObservation {
                p_gnss_ned_m: Some(base_p + offset_ned_m),
                v_gnss_ned_mps: Some(base_v),
                poisoned_meteo_wind_ned_mps: None,
            },
            GnssAttackScenario::StealthSlowRampSpoofing {
                drift_rate_ned_mps,
                start_time_s,
            } => {
                let dt = (t_s - start_time_s).max(0.0);
                let ramp_offset = drift_rate_ned_mps * dt;
                // Stealth spoofer suppresses or dampens velocity innovation so single-frame chi2 stays tiny
                let subtle_vel_bias = 0.25 * drift_rate_ned_mps;
                SandboxSensorObservation {
                    p_gnss_ned_m: Some(base_p + ramp_offset),
                    v_gnss_ned_mps: Some(base_v + subtle_vel_bias),
                    poisoned_meteo_wind_ned_mps: None,
                }
            }
            GnssAttackScenario::PreCutMeteoCachePoisoning { fake_wind_ned_mps } => {
                SandboxSensorObservation {
                    p_gnss_ned_m: Some(base_p),
                    v_gnss_ned_mps: Some(base_v),
                    poisoned_meteo_wind_ned_mps: Some(fake_wind_ned_mps),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_nominal_flight_no_false_alarms_at_60hz() {
        let mut watchdog = DualLayerGnssWatchdog::new();
        let s_inv_sqrt = build_diagonal_innovation_inv_sqrt(0.35, 0.15);
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        // Simulate 10 seconds (600 frames at 60 Hz) of normal zero-mean sensor noise
        for _ in 0..600 {
            let r = SVector::<f64, 6>::new(
                rng.gen_range(-0.04..0.04),
                rng.gen_range(-0.04..0.04),
                rng.gen_range(-0.04..0.04),
                rng.gen_range(-0.015..0.015),
                rng.gen_range(-0.015..0.015),
                rng.gen_range(-0.015..0.015),
            );
            let status = watchdog.evaluate_gnss_sample(&r, &s_inv_sqrt);
            assert!(matches!(status, GnssIntegrityStatus::Nominal { .. }));
            assert!(!watchdog.is_quarantined);
        }
    }

    #[test]
    fn test_jump_spoofing_detected_in_single_frame_under_20ms() {
        let mut watchdog = DualLayerGnssWatchdog::new();
        let s_inv_sqrt = build_diagonal_innovation_inv_sqrt(0.35, 0.15);

        // Attacker injects a sudden +15m lateral jump on RWY 09
        let jump_innovation = SVector::<f64, 6>::new(0.0, 15.0, 0.0, 0.0, 0.0, 0.0);
        let status = watchdog.evaluate_gnss_sample(&jump_innovation, &s_inv_sqrt);

        assert!(matches!(
            status,
            GnssIntegrityStatus::QuarantinedJumpSpoof { .. }
        ));
        assert!(watchdog.is_quarantined);
        assert_eq!(status.cockpit_banner(), BANNER_GNSS_QUARANTINED_FORTRESS);
    }

    #[test]
    fn test_stealth_slow_ramp_spoofing_caught_under_3_2_seconds_and_0_5m_error() {
        // Test both 0.15 m/s and 0.25 m/s stealth lateral ramps at 60 Hz
        for &ramp_rate_mps in &[0.15_f64, 0.25_f64] {
            let mut watchdog = DualLayerGnssWatchdog::new();
            let s_inv_sqrt = build_diagonal_innovation_inv_sqrt(0.30, 0.15);
            let sandbox = CyberAttackSandbox::new(GnssAttackScenario::StealthSlowRampSpoofing {
                drift_rate_ned_mps: Vector3::new(0.0, ramp_rate_mps, 0.0),
                start_time_s: 0.0,
            });

            let dt = 1.0 / 60.0;
            let mut detected_frame: Option<usize> = None;
            let mut lateral_error_at_detection_m = 0.0;

            for frame_idx in 1..=240 {
                let t_s = (frame_idx as f64) * dt;
                let true_p = Vector3::new(1000.0 + 65.0 * t_s, 0.0, -50.0);
                let true_v = Vector3::new(65.0, 0.0, 0.0);

                let obs = sandbox.synthesize_observation(
                    t_s,
                    true_p,
                    true_v,
                    Vector3::zeros(),
                    Vector3::zeros(),
                );

                if let (Some(p_gps), Some(v_gps)) = (obs.p_gnss_ned_m, obs.v_gnss_ned_mps) {
                    let dp = p_gps - true_p;
                    let dv = v_gps - true_v;
                    let innov = SVector::<f64, 6>::new(dp.x, dp.y, dp.z, dv.x, dv.y, dv.z);

                    // Verify that single-frame chi2 is way below 22.46 (evades Layer 1)
                    let whitened = s_inv_sqrt * innov;
                    let single_frame_chi2 = whitened.dot(&whitened);
                    assert!(
                        single_frame_chi2 < DEFAULT_CHI2_THRESHOLD_6DF,
                        "Stealth ramp should stay under single-frame chi2 threshold"
                    );

                    let status = watchdog.evaluate_gnss_sample(&innov, &s_inv_sqrt);
                    if matches!(
                        status,
                        GnssIntegrityStatus::QuarantinedStealthRampSpoof { .. }
                    ) {
                        detected_frame = Some(frame_idx);
                        lateral_error_at_detection_m = dp.y.abs();
                        break;
                    }
                }
            }

            assert!(
                detected_frame.is_some(),
                "Layer 2 CUSUM must detect {} m/s stealth ramp",
                ramp_rate_mps
            );
            if let Some(f_idx) = detected_frame {
                let detection_time_s = (f_idx as f64) * dt;
                // Must detect in < 3.2 seconds (< 192 frames at 60 Hz) and before lateral drift exceeds 0.5 m
                assert!(
                    detection_time_s < 3.2,
                    "Detection time {}s exceeded 3.2s limit",
                    detection_time_s
                );
                assert!(
                    lateral_error_at_detection_m < 0.50,
                    "Lateral error {}m at detection exceeded 0.5m limit",
                    lateral_error_at_detection_m
                );
            }
        }
    }

    #[test]
    fn test_permanent_quarantine_latch_and_jamming_detection() {
        let mut watchdog = DualLayerGnssWatchdog::new();
        let s_cov = SMatrix::<f64, 6, 6>::identity() * 0.09;

        // Simulate RF Jamming (None position/velocity)
        let status = watchdog.evaluate_position_velocity(
            None,
            None,
            Vector3::zeros(),
            Vector3::zeros(),
            &s_cov,
        );
        assert!(matches!(
            status,
            GnssIntegrityStatus::QuarantinedJammingOrLoss { .. }
        ));
        assert!(watchdog.is_quarantined);

        // Even if attacker restores seemingly clean coordinates later, receiver remains quarantined
        // until explicit crew reset
        let after_status = watchdog.evaluate_position_velocity(
            Some(Vector3::zeros()),
            Some(Vector3::zeros()),
            Vector3::zeros(),
            Vector3::zeros(),
            &s_cov,
        );
        assert!(after_status.is_quarantined());

        watchdog.reset_quarantine();
        assert!(!watchdog.is_quarantined);
    }
}
