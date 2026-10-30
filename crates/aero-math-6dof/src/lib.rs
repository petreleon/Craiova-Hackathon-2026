//! # `aero-math-6dof` — AERO-SHIELD LRCV V2.0 (`FogVision 6-DoF — Bulletproof Edition`)
//!
//! Motorul Cinematic și Dinamic 6-DoF pe Cuaternioni Unitari ($\mathbf{q}_b^n \in \mathbb{H}_1$),
//! Fluxul Optic în Inversa Adâncimii Fără Singularități ($\rho_{ground} = 1 / Z_{ground} \ge 0$),
//! Cuplajul Aerodinamic la Rafale Bruște de Ceață (`Optical Gust Lookahead` + `Aero-Inertial Impact`),
//! Error-State EKF 22-Stări cu Forma Stabilă Joseph și Arena celor 4 Estimatoare Simultane.

pub mod aero_gust_coupling;
pub mod es_ekf_22state;
pub mod four_estimators;
pub mod frames;
pub mod inverse_depth;
pub mod kinematics_6dof;

// Aliasuri de module compatibile cu denumirile din documentația de arhitectură
pub use four_estimators as four_estimators_arena;
pub use inverse_depth as inverse_depth_warp;
pub use kinematics_6dof as quaternion_kinematics;

// Re-exporturi principale la rădăcina crate-ului
pub use aero_gust_coupling::{
    compute_aero_gust_impact, compute_optical_gust_lookahead,
    estimate_gust_delta_from_residual_flow, AeroGustImpactState, OpticalGustLookahead,
};
pub use es_ekf_22state::{
    kalman_joseph_update, EsEkf22State, IDX_ACCEL_BIAS, IDX_ALS_OFFSET, IDX_ATT_THETA,
    IDX_DT_SYNC, IDX_FOG_WIND_NED, IDX_GYRO_BIAS, IDX_POS_NED, IDX_RHO_FOG, IDX_VEL_BODY,
};
pub use four_estimators::{EstimatorSnapshot, FourEstimatorsArena};
pub use frames::{
    body_to_camera, camera_to_body, sanitize_f64, skew_symmetric, wrap_angle_deg, wrap_angle_rad,
    AttitudeQuaternion, BodyFrame, CameraFrame, FramedVec3, NedFrameLrcv, ALT_LRCV_MSL,
    ILS_GLIDESLOPE_DEG, LAT_LRCV_RWY09, LON_LRCV_RWY09, RWY09_HEADING_DEG,
};
pub use inverse_depth::{
    compute_coupled_optical_flow, compute_inverse_depth_ground, fuse_fog_inverse_depth,
    isolate_fog_velocity_field_lsq, isolate_fog_velocity_from_flow, pseudo_inverse_a_tr,
    rotation_flow_matrix_b, translation_flow_matrix_a, CoupledOpticalFlowPixel,
};
pub use kinematics_6dof::{
    AircraftState6DoF, ControlInput6DoF, FogField3D, ImuPreintegrator, LrcvGuidanceMetrics,
};
