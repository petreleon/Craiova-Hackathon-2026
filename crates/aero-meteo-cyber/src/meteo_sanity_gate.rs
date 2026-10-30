//! Optical-Aerodynamic Meteo Sanity Gate — Protection Against *Pre-Cut Meteo Cache Poisoning* (Remediation #8).
//!
//! Cross-validates the cached Open-Meteo / METAR wind profile $\bar{\mathbf{w}}_{cache}(h_{AGL})$
//! against two independent onboard physical measurements at every frame (`60 Hz`):
//! 1. Optical volumetric fog velocity $\hat{\mathbf{w}}_{fog, opt}$ extracted by `FogOpticalNavNetV2` + IMU derotation,
//! 2. Aerodynamic wind triangle $\hat{\mathbf{w}}_{aero} = \hat{\mathbf{v}}_{ground, ned} - \mathbf{R}_b^n(\mathbf{q}_b^n)\mathbf{v}_{TAS, b}$.
//!
//! If the Euclidean discrepancy exceeds `4.5 m/s`, the cached meteo profile is immediately rejected
//! (`trust_weight = 0.0`, `is_cache_trusted = false`), neutralizing pre-cut network poisoning or
//! unmodeled severe wind shear.

use nalgebra::{UnitQuaternion, Vector3};
use serde::Serialize;

/// Maximum permissible discrepancy (`m/s`) between cached meteo wind and physical optical-aerodynamic consensus.
pub const METEO_SANITY_DISCREPANCY_THRESHOLD_MPS: f64 = 4.5_f64;

/// Relative weight assigned to the optical fog velocity estimate in the physical consensus vector.
pub const OPTICAL_FOG_CONSENSUS_WEIGHT: f64 = 0.65_f64;

/// Relative weight assigned to the aerodynamic velocity triangle in the physical consensus vector.
pub const AERO_TRIANGLE_CONSENSUS_WEIGHT: f64 = 0.35_f64;

/// Alert message emitted when the meteo cache is rejected due to poisoning or severe shear invalidation.
pub const ALERT_POISONED_METEO_REJECTED: &str =
    "POISONED OR SHEAR-INVALIDATED METEO CACHE REJECTED — 100% OPTICAL AUTONOMY";

/// Alert message emitted when the meteo cache is consistent with onboard optical-aerodynamic physics.
pub const ALERT_METEO_VERIFIED: &str =
    "METEO PROFILE VERIFIED BY OPTICAL-AERODYNAMIC CONSISTENCY";

/// Verdict returned by the Optical-Aerodynamic Meteo Sanity Gate at each `60 Hz` navigation cycle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MeteoSanityVerdict {
    /// `true` if `discrepancy_mps <= 4.5 m/s`, `false` if poisoned or invalidated by extreme shear.
    pub is_cache_trusted: bool,
    /// Continuous trust weight in `[0.0 = Poisoned/Rejected, 1.0 = Fully Trusted]`.
    pub trust_weight: f64,
    /// Euclidean norm $\|\bar{\mathbf{w}}_{cache}(h) - \mathbf{w}_{physical\_consensus}\|_2$ in `m/s`.
    pub discrepancy_mps: f64,
    /// Cockpit status banner string.
    pub alert_status: &'static str,
}

/// Detailed diagnostic telemetry for the cockpit HUD / Cyber-Defense panel.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MeteoSanityDiagnostics {
    pub verdict: MeteoSanityVerdict,
    pub w_cache_ned_mps: [f64; 3],
    pub w_optical_fog_ned_mps: [f64; 3],
    pub w_aerodynamic_triangle_ned_mps: [f64; 3],
    pub w_physical_consensus_ned_mps: [f64; 3],
    pub w_fused_effective_ned_mps: [f64; 3],
}

/// Computes the aerodynamic wind vector in the `NED` frame from the velocity triangle:
/// $$\hat{\mathbf{w}}_{aero, ned} = \hat{\mathbf{v}}_{ground, ned} - \mathbf{R}_b^n(\mathbf{q}_b^n)\,\mathbf{v}_{TAS, b}$$
/// where $\mathbf{q}_b^n \in \mathbb{H}_1$ is the unit quaternion rotating vectors from the aircraft
/// body frame (`Forward-Right-Down`) to the local `NED` navigation frame, and $\mathbf{v}_{TAS, b}$
/// is the true airspeed vector in body coordinates.
#[inline]
pub fn compute_aerodynamic_wind_triangle_ned(
    v_ground_opt_ned: Vector3<f64>,
    q_body_to_ned: &UnitQuaternion<f64>,
    v_true_airspeed_body: Vector3<f64>,
) -> Vector3<f64> {
    let v_tas_ned = q_body_to_ned.transform_vector(&v_true_airspeed_body);
    let diff = v_ground_opt_ned - v_tas_ned;
    if diff.iter().all(|c| c.is_finite()) {
        diff
    } else {
        Vector3::zeros()
    }
}

/// Computes the physical consensus wind vector combining optical volumetric fog velocity (`65%`)
/// and the aerodynamic airspeed triangle (`35%`).
#[inline]
pub fn compute_physical_wind_consensus_ned(
    w_optical_fog_ned: Vector3<f64>,
    w_aerodynamic_triangle_ned: Vector3<f64>,
) -> Vector3<f64> {
    OPTICAL_FOG_CONSENSUS_WEIGHT * w_optical_fog_ned
        + AERO_TRIANGLE_CONSENSUS_WEIGHT * w_aerodynamic_triangle_ned
}

/// Verifies the integrity of the cached meteo wind vector $\bar{\mathbf{w}}_{cache}(h)$ against
/// onboard optical fog flow and the aerodynamic velocity triangle.
///
/// Implements the exact mathematical gate from `arhitectura/04_aero_meteo_cyber.md` (Section 2):
/// - $\mathbf{w}_{physical} = 0.65\,\hat{\mathbf{w}}_{fog, opt} + 0.35\,\hat{\mathbf{w}}_{aero}$
/// - $d = \|\bar{\mathbf{w}}_{cache} - \mathbf{w}_{physical}\|_2$
/// - Rejects cache completely (`trust_weight = 0.0`) if $d > 4.5\text{ m/s}$ (or non-finite).
pub fn verify_meteo_cache_integrity(
    w_cache_ned: Vector3<f64>,
    w_optical_fog_ned: Vector3<f64>,
    w_aerodynamic_triangle_ned: Vector3<f64>,
) -> MeteoSanityVerdict {
    if !w_cache_ned.iter().all(|x| x.is_finite())
        || !w_optical_fog_ned.iter().all(|x| x.is_finite())
        || !w_aerodynamic_triangle_ned.iter().all(|x| x.is_finite())
    {
        return MeteoSanityVerdict {
            is_cache_trusted: false,
            trust_weight: 0.0,
            discrepancy_mps: f64::INFINITY,
            alert_status: ALERT_POISONED_METEO_REJECTED,
        };
    }

    let w_physical_consensus =
        compute_physical_wind_consensus_ned(w_optical_fog_ned, w_aerodynamic_triangle_ned);
    let discrepancy = (w_cache_ned - w_physical_consensus).norm();

    if discrepancy > METEO_SANITY_DISCREPANCY_THRESHOLD_MPS {
        MeteoSanityVerdict {
            is_cache_trusted: false,
            trust_weight: 0.0, // Rejects poisoned cache completely!
            discrepancy_mps: discrepancy,
            alert_status: ALERT_POISONED_METEO_REJECTED,
        }
    } else {
        let weight = (1.0
            - (discrepancy / METEO_SANITY_DISCREPANCY_THRESHOLD_MPS).powi(2))
        .clamp(0.0, 1.0);
        MeteoSanityVerdict {
            is_cache_trusted: true,
            trust_weight: weight,
            discrepancy_mps: discrepancy,
            alert_status: ALERT_METEO_VERIFIED,
        }
    }
}

/// Fuses the cached meteo wind prior with the live physical optical-aerodynamic consensus using
/// the sanity gate's `trust_weight`.
///
/// When `is_cache_trusted == false`, `trust_weight` is `0.0` and the returned vector is `100%`
/// governed by the onboard optical-aerodynamic physical consensus.
pub fn fuse_meteo_prior_with_physical_consensus(
    w_cache_ned: Vector3<f64>,
    w_optical_fog_ned: Vector3<f64>,
    w_aerodynamic_triangle_ned: Vector3<f64>,
) -> (Vector3<f64>, MeteoSanityDiagnostics) {
    let verdict = verify_meteo_cache_integrity(
        w_cache_ned,
        w_optical_fog_ned,
        w_aerodynamic_triangle_ned,
    );
    let w_physical =
        compute_physical_wind_consensus_ned(w_optical_fog_ned, w_aerodynamic_triangle_ned);

    // Maximum prior blend ratio is 0.30 even when cache is 100% trusted, so live optics always lead;
    // drops to 0.0 immediately when poisoned.
    let effective_cache_blend = 0.30 * verdict.trust_weight;
    let w_fused =
        effective_cache_blend * w_cache_ned + (1.0 - effective_cache_blend) * w_physical;

    let diagnostics = MeteoSanityDiagnostics {
        verdict,
        w_cache_ned_mps: [w_cache_ned.x, w_cache_ned.y, w_cache_ned.z],
        w_optical_fog_ned_mps: [
            w_optical_fog_ned.x,
            w_optical_fog_ned.y,
            w_optical_fog_ned.z,
        ],
        w_aerodynamic_triangle_ned_mps: [
            w_aerodynamic_triangle_ned.x,
            w_aerodynamic_triangle_ned.y,
            w_aerodynamic_triangle_ned.z,
        ],
        w_physical_consensus_ned_mps: [w_physical.x, w_physical.y, w_physical.z],
        w_fused_effective_ned_mps: [w_fused.x, w_fused.y, w_fused.z],
    };

    (w_fused, diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_consistent_meteo_cache_accepted() {
        let w_cache = Vector3::new(0.0, -3.2, 0.0);
        let w_opt = Vector3::new(-0.2, -3.5, 0.0);
        let w_aero = Vector3::new(-0.1, -3.3, 0.0);

        let verdict = verify_meteo_cache_integrity(w_cache, w_opt, w_aero);
        assert!(verdict.is_cache_trusted);
        assert!(verdict.discrepancy_mps < 0.5);
        assert!(verdict.trust_weight > 0.98);
        assert_eq!(verdict.alert_status, ALERT_METEO_VERIFIED);
    }

    #[test]
    fn test_pre_cut_poisoned_meteo_cache_rejected() {
        // Attacker injects +25 m/s North wind (-25 m/s along North axis in NED) right before cutting internet
        let w_poisoned_cache = Vector3::new(-25.0, 0.0, 0.0);
        // True physical wind measured onboard at LRCV RWY 09 is ~3.5 m/s East wind
        let w_opt = Vector3::new(0.0, -3.5, 0.0);
        let w_aero = Vector3::new(-0.1, -3.4, 0.0);

        let (w_fused, diag) =
            fuse_meteo_prior_with_physical_consensus(w_poisoned_cache, w_opt, w_aero);

        assert!(!diag.verdict.is_cache_trusted);
        assert_eq!(diag.verdict.trust_weight, 0.0);
        assert!(diag.verdict.discrepancy_mps > 25.0);
        assert_eq!(diag.verdict.alert_status, ALERT_POISONED_METEO_REJECTED);

        // Fused wind must equal 100% the physical consensus, completely ignoring the poisoned cache
        let expected_physical = compute_physical_wind_consensus_ned(w_opt, w_aero);
        assert!((w_fused - expected_physical).norm() < 1e-12);
    }

    #[test]
    fn test_threshold_boundary_at_4_5_mps() {
        let w_opt = Vector3::new(0.0, 0.0, 0.0);
        let w_aero = Vector3::new(0.0, 0.0, 0.0);

        // Slightly below 4.5 m/s -> trusted with small positive weight
        let w_below = Vector3::new(4.4, 0.0, 0.0);
        let v_below = verify_meteo_cache_integrity(w_below, w_opt, w_aero);
        assert!(v_below.is_cache_trusted);
        assert!(v_below.trust_weight > 0.0 && v_below.trust_weight < 0.1);

        // Slightly above 4.5 m/s -> rejected with 0.0 weight
        let w_above = Vector3::new(4.51, 0.0, 0.0);
        let v_above = verify_meteo_cache_integrity(w_above, w_opt, w_aero);
        assert!(!v_above.is_cache_trusted);
        assert_eq!(v_above.trust_weight, 0.0);
    }

    #[test]
    fn test_quaternion_aerodynamic_wind_triangle() {
        // Aircraft flying due East (yaw = +90 deg = FRAC_PI_2) at 65 m/s TAS,
        // facing a 5 m/s headwind from the East (so ground speed East is 60 m/s).
        let q_body_to_ned =
            UnitQuaternion::from_euler_angles(0.0, 0.0, std::f64::consts::FRAC_PI_2);
        let v_tas_body = Vector3::new(65.0, 0.0, 0.0);
        let v_ground_ned = Vector3::new(0.0, 60.0, 0.0);

        let w_aero_ned =
            compute_aerodynamic_wind_triangle_ned(v_ground_ned, &q_body_to_ned, v_tas_body);
        assert!(w_aero_ned.x.abs() < 1e-9);
        assert!((w_aero_ned.y - (-5.0)).abs() < 1e-9);
        assert!(w_aero_ned.z.abs() < 1e-9);
    }
}
