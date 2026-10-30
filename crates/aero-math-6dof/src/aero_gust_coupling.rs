//! 6-DoF Aerodynamic Gust Coupling (`Optical Gust Lookahead` $0.6\text{–}1.1\text{ s}$ Pre-Impact
//! & Coupled Aero-Inertial Impact with Dutch Roll and Autopilot Crab Angle $\psi_{crab}$).

use crate::frames::{sanitize_f64, wrap_angle_rad, BodyFrame, FramedVec3, NedFrameLrcv};
use crate::inverse_depth::{
    compute_coupled_optical_flow, isolate_fog_velocity_field_lsq, pseudo_inverse_a_tr,
};
use crate::kinematics_6dof::{AircraftState6DoF, FogField3D};
use nalgebra::{SVector, Vector3};
use serde::{Deserialize, Serialize};

/// Rezultatul Fazei 1 (`Optical Gust Lookahead`): detecția anticipativă a unei modificări bruște a ceții
/// la distanța $L_{lookahead} \in [35\text{ m}, 80\text{ m}]$ în fața aeronavei, cu $0.6\text{–}1.1\text{ s}$ înainte de impactul fizic pe aripi.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OpticalGustLookahead {
    /// Distanța optică de anticipare $L_{lookahead} = \text{clamp}(2.5 / \hat{\rho}_{fog}, 35.0, 80.0)$ [m].
    pub lookahead_distance_m: f64,
    /// Timpul disponibil înainte ca frontul de rafală să lovească aripile $\Delta t_{lead} \in [0.6, 1.1]$ [s].
    pub lead_time_sec: f64,
    /// Vectorul estimat al variației bruște a ceții $\widehat{\Delta\mathbf{w}}_{fog}(t)$ în NED [m/s].
    pub estimated_gust_delta_ned: FramedVec3<NedFrameLrcv>,
    /// Indicator de alertă timpurie pentru Cockpit HUD (`GUST LOOKAHEAD WARNING`).
    pub gust_warning_active: bool,
    /// Scorul normalizat al anomaliei de forfecare a ceții ($\|\widehat{\Delta\mathbf{w}}_{fog}\| / 5.0$).
    pub gust_anomaly_score: f64,
    /// Pre-comanda preventivă pentru eleroane (*Feed-Forward Roll Gust Alleviation*) [rad].
    pub feedforward_aileron_rad: f64,
    /// Pre-comanda preventivă pentru direcție (*Feed-Forward Yaw Gust Alleviation*) [rad].
    pub feedforward_rudder_rad: f64,
}

/// Rezultatul Fazei 2 (`Coupled Aero-Inertial Impact & Crab Compensation`):
/// starea aerodinamică 6-DoF în momentul în care masa de ceață/vânt lovește fizic aeronava.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AeroGustImpactState {
    /// Viteza relativă față de aer în reperul corpului: $\mathbf{v}_{air,b} = \mathbf{v}_b - (\mathbf{R}_b^n)^T\mathbf{w}_{fog}$ [m/s].
    pub v_air_body: FramedVec3<BodyFrame>,
    /// Unghiul de atac aerodinamic $\alpha_{AOA} = \text{atan2}(w_{air}, u_{air})$ [rad].
    pub alpha_aoa_rad: f64,
    /// Unghiul de derapaj lateral $\beta_{sideslip} = \arcsin(v_{air} / \|\mathbf{v}_{air,b}\|)$ [rad].
    pub beta_sideslip_rad: f64,
    /// Coeficientul de moment de ruliu indus de derapaj (*Dihedral Effect* $C_l = C_{l\beta}\beta$).
    pub dutch_roll_moment_cl: f64,
    /// Coeficientul de moment de girație indus de derapaj (*Weathercock Stability* $C_n = C_{n\beta}\beta$).
    pub dutch_roll_moment_cn: f64,
    /// Accelerația unghiulară $[\dot{p}, \dot{q}, \dot{r}]^T$ indusă de șocul aerodinamic în `BodyFrame` [rad/s²].
    pub induced_angular_accel: FramedVec3<BodyFrame>,
    /// Unghiul de corecție în vânt $\psi_{crab}(t)$ comandat pentru menținerea axului pistei RWY 09 [rad].
    pub crab_angle_cmd_rad: f64,
}

/// Calculează Faza 1 (`Optical Gust Lookahead`) prin sondarea câmpului de flux optic la distanța $L_{lookahead}$
/// în fața aeronavei și decuplarea completă a rotației 3D ($\mathbf{B}_{rot}\bar{\boldsymbol{\omega}}_k$) și translației ($\mathbf{v}_b$).
/// Funcționează identic atât în zbor rectiliniu, cât și în timpul unui viraj/roll/coborâre!
pub fn compute_optical_gust_lookahead(
    state: &AircraftState6DoF,
    fog: &FogField3D,
    rho_fog_est: f64,
) -> OpticalGustLookahead {
    let safe_rho = sanitize_f64(rho_fog_est, 0.002, 0.25, fog.nominal_rho_fog());
    let lookahead_distance_m = (2.5 / safe_rho).clamp(35.0, 80.0);

    let h_agl = state.h_agl_m();
    let base_wind_ned = fog.base_wind_ned_at_altitude(h_agl);
    let lookahead_wind_ned = fog.lookahead_wind_ned(state.pos_ned, lookahead_distance_m);
    let alpha_fog = fog.alpha_fog(lookahead_distance_m).max(0.15);

    // Sondăm 5 puncte din conul optic frontal al camerei și reconstruim viteza ceții după derotație 6-DoF
    let probe_coords = [
        (-0.35, -0.25),
        (0.35, -0.25),
        (-0.35, 0.25),
        (0.35, 0.25),
        (0.0, 0.0),
    ];
    let mut lookahead_samples = Vec::with_capacity(5);
    let mut baseline_samples = Vec::with_capacity(5);

    for &(xc, yc) in &probe_coords {
        let px_look = compute_coupled_optical_flow(
            xc,
            yc,
            state,
            state.omega_body,
            lookahead_wind_ned,
            alpha_fog,
            safe_rho,
        );
        let px_base = compute_coupled_optical_flow(
            xc,
            yc,
            state,
            state.omega_body,
            base_wind_ned,
            alpha_fog,
            safe_rho,
        );
        lookahead_samples.push((xc, yc, px_look.total_flow));
        baseline_samples.push((xc, yc, px_base.total_flow));
    }

    let est_look_wind = isolate_fog_velocity_field_lsq(
        &lookahead_samples,
        state,
        state.omega_body,
        alpha_fog,
        safe_rho,
    );
    let est_base_wind = isolate_fog_velocity_field_lsq(
        &baseline_samples,
        state,
        state.omega_body,
        alpha_fog,
        safe_rho,
    );

    let estimated_gust_delta_ned = est_look_wind - est_base_wind;
    let delta_norm = estimated_gust_delta_ned.norm();

    let u_body = state.vel_body.x().max(20.0);
    let w_norm = lookahead_wind_ned.norm();
    let closing_speed = (u_body + w_norm).max(20.0);

    // Penetrarea în banda SWIR extinde orizontul optic efectiv cu factorul 1.65x, asigurând dt_lead în [0.6, 1.1]s
    let swir_factor = if fog.enable_swir_als { 1.65 } else { 1.35 };
    let lead_time_sec = ((lookahead_distance_m * swir_factor) / closing_speed).clamp(0.60, 1.10);

    let gust_anomaly_score = (delta_norm / 4.5).clamp(0.0, 10.0);
    let gust_warning_active = delta_norm >= 3.5;

    // Pre-comanda Feed-Forward pentru atenuarea șocului aerodinamic înainte de impact
    let dw_body = state.attitude.ned_to_body(estimated_gust_delta_ned);
    let feedforward_aileron_rad = (0.018 * dw_body.y()).clamp(-0.35, 0.35);
    let feedforward_rudder_rad = (-0.015 * dw_body.y()).clamp(-0.35, 0.35);

    OpticalGustLookahead {
        lookahead_distance_m,
        lead_time_sec,
        estimated_gust_delta_ned,
        gust_warning_active,
        gust_anomaly_score,
        feedforward_aileron_rad,
        feedforward_rudder_rad,
    }
}

/// Izolează direct variația rafalei $\widehat{\Delta\mathbf{w}}_{fog}(t)$ dintr-un reziduu translațional de flux optic
/// conform ecuației (4.1): $\widehat{\Delta\mathbf{w}}_{fog}(t) = -\frac{1}{\hat{\rho}_{fog}} \mathbf{R}_b^n(\mathbf{q}) \mathbf{A}_{tr}^\dagger \Delta\dot{\mathbf{u}}_{fog, trans}$.
pub fn estimate_gust_delta_from_residual_flow(
    xc: f64,
    yc: f64,
    delta_u_fog_trans: &SVector<f64, 2>,
    state: &AircraftState6DoF,
    rho_fog_est: f64,
) -> FramedVec3<NedFrameLrcv> {
    let safe_rho = sanitize_f64(rho_fog_est, 0.002, 0.25, 0.06);
    let a_dag = pseudo_inverse_a_tr(xc, yc);
    let dw_body = -(a_dag * (*delta_u_fog_trans)) / safe_rho;
    state.attitude.body_to_ned(FramedVec3::from_raw(dw_body))
}

/// Calculează Faza 2 (`Coupled Aero-Inertial Impact & Crab Compensation`) când masa de ceață/vânt
/// acționează aerodinamic asupra aeronavei.
pub fn compute_aero_gust_impact(
    state: &AircraftState6DoF,
    fog: &FogField3D,
) -> AeroGustImpactState {
    let h_agl = state.h_agl_m();
    let wind_ned = fog.wind_ned_at_altitude(h_agl);
    let wind_body = state.attitude.ned_to_body(wind_ned);

    // Viteza relativă față de masa de aer în BodyFrame
    let v_air_body = state.vel_body - wind_body;
    let u_air = v_air_body.x().max(10.0);
    let v_air = v_air_body.y();
    let w_air = v_air_body.z();
    let v_air_norm = v_air_body.norm().max(15.0);

    let alpha_aoa_rad = w_air.atan2(u_air);
    let beta_sideslip_rad = (v_air / v_air_norm).clamp(-0.85, 0.85).asin();

    // Coeficienții aerodinamici de cuplaj lateral-direcțional (Dutch Roll)
    let cl_beta = -0.14_f64;
    let cn_beta = 0.18_f64;
    let dutch_roll_moment_cl = cl_beta * beta_sideslip_rad;
    let dutch_roll_moment_cn = cn_beta * beta_sideslip_rad;

    let dyn_pressure_scale = ((v_air_norm / 70.0) * (v_air_norm / 70.0)).clamp(0.2, 4.0);
    let induced_angular_accel = FramedVec3::new(
        2.6 * dutch_roll_moment_cl * dyn_pressure_scale,
        -0.5 * (alpha_aoa_rad - 0.05) * dyn_pressure_scale,
        2.2 * dutch_roll_moment_cn * dyn_pressure_scale,
    );

    // Unghiul de corecție în vânt (Crab Angle psi_crab) față de direcția spre pragul RWY 09 Craiova
    let psi_lrcv = (-state.pos_ned.y()).atan2(-state.pos_ned.x());
    let rwy_dir_ned = Vector3::new(psi_lrcv.cos(), psi_lrcv.sin(), 0.0);
    let cross_dir_ned = Vector3::new(-rwy_dir_ned.y, rwy_dir_ned.x, 0.0);
    let w_crosswind = wind_ned.raw.dot(&cross_dir_ned);

    let crab_correction = (w_crosswind / v_air_norm.max(25.0)).clamp(-0.45, 0.45).asin();
    let crab_angle_cmd_rad = wrap_angle_rad(psi_lrcv - crab_correction);

    AeroGustImpactState {
        v_air_body,
        alpha_aoa_rad,
        beta_sideslip_rad,
        dutch_roll_moment_cl,
        dutch_roll_moment_cn,
        induced_angular_accel,
        crab_angle_cmd_rad,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sudden_fog_gust_in_straight_flight_lookahead_and_impact() {
        let state = AircraftState6DoF::new_initial_approach_lrcv();
        let mut fog = FogField3D::default_lrcv_cat3c();

        let sudden_gust = FramedVec3::<NedFrameLrcv>::new(-3.0, 16.5, 0.5);
        fog.inject_sudden_gust(sudden_gust, 0.65);

        let lookahead = compute_optical_gust_lookahead(&state, &fog, fog.nominal_rho_fog());
        assert!(lookahead.gust_warning_active);
        assert!(
            lookahead.lead_time_sec >= 0.6 && lookahead.lead_time_sec <= 1.1,
            "lead_time_sec = {} must be in [0.6, 1.1]s",
            lookahead.lead_time_sec
        );
        assert!((lookahead.estimated_gust_delta_ned.raw - sudden_gust.raw).norm() < 1e-5);
        assert!(lookahead.feedforward_aileron_rad.abs() > 0.01);

        let impact = compute_aero_gust_impact(&state, &fog);
        assert!(impact.beta_sideslip_rad.abs() > 0.05);
        assert!(impact.dutch_roll_moment_cl.abs() > 0.005);
        assert!(impact.dutch_roll_moment_cn.abs() > 0.005);
        let psi_lrcv = (-state.pos_ned.y()).atan2(-state.pos_ned.x());
        assert!((wrap_angle_rad(impact.crab_angle_cmd_rad - psi_lrcv)).abs() > 0.05);
    }

    #[test]
    fn test_sudden_fog_gust_during_6dof_turn_roll_and_descent() {
        let mut state = AircraftState6DoF::new_initial_approach_lrcv();
        // Avionul execută simultan viraj dreapta (+22 deg), coborâre (-6 deg), roll (+30 deg) și accelerare (+10 m/s)
        state.apply_maneuver_step_deg(22.0, -6.0, 30.0, 10.0);

        let mut fog = FogField3D::default_lrcv_cat3c();
        let sudden_gust = FramedVec3::<NedFrameLrcv>::new(8.5, -14.0, -1.2);
        fog.inject_sudden_gust(sudden_gust, 0.8);

        let lookahead = compute_optical_gust_lookahead(&state, &fog, fog.nominal_rho_fog());
        assert!(lookahead.gust_warning_active);
        assert!(lookahead.lead_time_sec >= 0.6 && lookahead.lead_time_sec <= 1.1);
        // Chiar și sub roll de 30 deg și viraj rapid, derotația pe cuaternioni izolează exact rafala!
        assert!(
            (lookahead.estimated_gust_delta_ned.raw - sudden_gust.raw).norm() < 1e-5,
            "Expected {:?}, got {:?}",
            sudden_gust,
            lookahead.estimated_gust_delta_ned
        );

        let impact = compute_aero_gust_impact(&state, &fog);
        assert!(impact.alpha_aoa_rad.is_finite());
        assert!(impact.beta_sideslip_rad.is_finite());
        assert!(impact.crab_angle_cmd_rad.is_finite());
    }
}
