//! Type-Safe Coordinate Frames (`NedFrameLrcv`, `BodyFrame`, `CameraFrame`)
//! and Singularity-Free Unit Quaternion Attitude Representations for AERO-SHIELD LRCV V2.0.

use nalgebra::{Matrix3, SMatrix, SVector, UnitQuaternion, Vector3};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use std::marker::PhantomData;
use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

/// Latitudinea pragului pistei RWY 09 a Aeroportului Internațional Craiova (`LRCV`) [grade N].
pub const LAT_LRCV_RWY09: f64 = 44.3181_f64;
/// Longitudinea pragului pistei RWY 09 a Aeroportului Internațional Craiova (`LRCV`) [grade E].
pub const LON_LRCV_RWY09: f64 = 23.8886_f64;
/// Altitudinea pragului pistei RWY 09 `LRCV` față de nivelul mării (MSL) [metri].
pub const ALT_LRCV_MSL: f64 = 191.0_f64;
/// Capul compas (Heading) al pistei RWY 09 `LRCV` [grade].
pub const RWY09_HEADING_DEG: f64 = 87.0_f64;
/// Panta nominală de coborâre ILS pentru RWY 09 `LRCV` [grade].
pub const ILS_GLIDESLOPE_DEG: f64 = 3.0_f64;

/// Sanitizează o valoare `f64` externă garantând că nu este `NaN` sau `Inf`
/// și că se încadrează strict în intervalul `[min, max]`.
#[inline(always)]
pub fn sanitize_f64(val: f64, min: f64, max: f64, fallback: f64) -> f64 {
    let lo = if min.is_finite() && max.is_finite() {
        min.min(max)
    } else {
        -1.0e6
    };
    let hi = if min.is_finite() && max.is_finite() {
        min.max(max)
    } else {
        1.0e6
    };
    if val.is_finite() {
        val.clamp(lo, hi)
    } else if fallback.is_finite() {
        fallback.clamp(lo, hi)
    } else {
        0.5 * (lo + hi)
    }
}

/// Înfășoară un unghi (în radiani) în intervalul principal $[-\pi, \pi]$.
#[inline(always)]
pub fn wrap_angle_rad(angle: f64) -> f64 {
    if !angle.is_finite() {
        return 0.0;
    }
    let two_pi = 2.0 * PI;
    let mut wrapped = (angle + PI) % two_pi;
    if wrapped < 0.0 {
        wrapped += two_pi;
    }
    wrapped - PI
}

/// Înfășoară un unghi (în grade) în intervalul $[-180^\circ, 180^\circ]$.
#[inline(always)]
pub fn wrap_angle_deg(angle_deg: f64) -> f64 {
    wrap_angle_rad(angle_deg.to_radians()).to_degrees()
}

/// Reperul terestru local **North-East-Down ($\mathcal{F}_n$)** cu originea fixată pe pragul RWY 09 Craiova (`LRCV`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NedFrameLrcv;

/// Reperul solidar cu aeronava ($\mathcal{F}_b$: $X_b$ înainte, $Y_b$ aripa dreaptă, $Z_b$ în jos).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BodyFrame;

/// Reperul optic al camerei de la bord ($\mathcal{F}_c$: $X_c$ dreapta, $Y_c$ jos, $Z_c$ adâncimea pe axa optică).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CameraFrame;

/// Vector 3D puternic tipizat cu sistemul de coordonate prin Phantom Type `F`.
/// Previne la compilare adunarea sau amestecarea accidentală a vectorilor din repere diferite.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct FramedVec3<F> {
    pub raw: Vector3<f64>,
    #[serde(skip)]
    _marker: PhantomData<F>,
}

impl<F> Default for FramedVec3<F> {
    #[inline(always)]
    fn default() -> Self {
        Self::zero()
    }
}

impl<F> FramedVec3<F> {
    /// Creează un nou vector 3D în reperul `F`.
    #[inline(always)]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self {
            raw: Vector3::new(x, y, z),
            _marker: PhantomData,
        }
    }

    /// Creează un `FramedVec3<F>` direct dintr-un `nalgebra::Vector3<f64>`.
    #[inline(always)]
    pub const fn from_raw(raw: Vector3<f64>) -> Self {
        Self {
            raw,
            _marker: PhantomData,
        }
    }

    /// Creează un vector nul $[0, 0, 0]^T$ în reperul `F`.
    #[inline(always)]
    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    /// Creează un vector 3D cu toate componentele sanitizate în `[min, max]`.
    #[inline(always)]
    pub fn sanitized(x: f64, y: f64, z: f64, min: f64, max: f64) -> Self {
        Self::new(
            sanitize_f64(x, min, max, 0.0),
            sanitize_f64(y, min, max, 0.0),
            sanitize_f64(z, min, max, 0.0),
        )
    }

    /// Componenta X a vectorului.
    #[inline(always)]
    pub fn x(&self) -> f64 {
        self.raw.x
    }

    /// Componenta Y a vectorului.
    #[inline(always)]
    pub fn y(&self) -> f64 {
        self.raw.y
    }

    /// Componenta Z a vectorului.
    #[inline(always)]
    pub fn z(&self) -> f64 {
        self.raw.z
    }

    /// Norma euclidiană $\|\mathbf{v}\|_2$.
    #[inline(always)]
    pub fn norm(&self) -> f64 {
        self.raw.norm()
    }

    /// Pătratul normei euclidiene $\|\mathbf{v}\|_2^2$.
    #[inline(always)]
    pub fn norm_squared(&self) -> f64 {
        self.raw.norm_squared()
    }

    /// Produsul scalar în același reper `F`.
    #[inline(always)]
    pub fn dot(&self, other: &Self) -> f64 {
        self.raw.dot(&other.raw)
    }

    /// Produsul vectorial în același reper `F`.
    #[inline(always)]
    pub fn cross(&self, other: &Self) -> Self {
        Self::from_raw(self.raw.cross(&other.raw))
    }

    /// Scalarea cu un factor real.
    #[inline(always)]
    pub fn scale(&self, factor: f64) -> Self {
        Self::from_raw(self.raw * factor)
    }

    /// Conversie în tablou fix `[f64; 3]`.
    #[inline(always)]
    pub fn to_array(&self) -> [f64; 3] {
        [self.raw.x, self.raw.y, self.raw.z]
    }

    /// Construcție din tablou fix `[f64; 3]`.
    #[inline(always)]
    pub const fn from_array(arr: [f64; 3]) -> Self {
        Self::new(arr[0], arr[1], arr[2])
    }
}

impl<F> Add for FramedVec3<F> {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Self::from_raw(self.raw + rhs.raw)
    }
}

impl<F> AddAssign for FramedVec3<F> {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        self.raw += rhs.raw;
    }
}

impl<F> Sub for FramedVec3<F> {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        Self::from_raw(self.raw - rhs.raw)
    }
}

impl<F> SubAssign for FramedVec3<F> {
    #[inline(always)]
    fn sub_assign(&mut self, rhs: Self) {
        self.raw -= rhs.raw;
    }
}

impl<F> Neg for FramedVec3<F> {
    type Output = Self;
    #[inline(always)]
    fn neg(self) -> Self::Output {
        Self::from_raw(-self.raw)
    }
}

impl<F> Mul<f64> for FramedVec3<F> {
    type Output = Self;
    #[inline(always)]
    fn mul(self, rhs: f64) -> Self::Output {
        Self::from_raw(self.raw * rhs)
    }
}

impl<F> Mul<FramedVec3<F>> for f64 {
    type Output = FramedVec3<F>;
    #[inline(always)]
    fn mul(self, rhs: FramedVec3<F>) -> Self::Output {
        FramedVec3::from_raw(rhs.raw * self)
    }
}

/// Transformă un vector din `BodyFrame` ($\mathcal{F}_b$: $X_b$ înainte, $Y_b$ dreapta, $Z_b$ jos)
/// în `CameraFrame` ($\mathcal{F}_c$: $X_c$ dreapta, $Y_c$ jos, $Z_c$ înainte pe axa optică).
#[inline(always)]
pub fn body_to_camera(v_b: FramedVec3<BodyFrame>) -> FramedVec3<CameraFrame> {
    FramedVec3::new(v_b.raw.y, v_b.raw.z, v_b.raw.x)
}

/// Transformă un vector din `CameraFrame` ($\mathcal{F}_c$) în `BodyFrame` ($\mathcal{F}_b$).
#[inline(always)]
pub fn camera_to_body(v_c: FramedVec3<CameraFrame>) -> FramedVec3<BodyFrame> {
    FramedVec3::new(v_c.raw.z, v_c.raw.x, v_c.raw.y)
}

/// Reprezentarea fără singularități a atitudinii aeronavei prin cuaternion unitar $\mathbf{q}_b^n \in \mathbb{H}_1$.
/// Transformarea între `BodyFrame` și `NedFrameLrcv` este permisă exclusiv prin această structură.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AttitudeQuaternion {
    pub q_b_to_n: UnitQuaternion<f64>,
}

impl Default for AttitudeQuaternion {
    #[inline(always)]
    fn default() -> Self {
        Self::identity()
    }
}

impl AttitudeQuaternion {
    /// Cuaternionul identitate (Roll = 0, Pitch = 0, Yaw = 0).
    #[inline(always)]
    pub fn identity() -> Self {
        Self {
            q_b_to_n: UnitQuaternion::identity(),
        }
    }

    /// Construiește `AttitudeQuaternion` dintr-un `UnitQuaternion<f64>` și îl renormalizează.
    #[inline(always)]
    pub fn from_unit_quaternion(mut q: UnitQuaternion<f64>) -> Self {
        q.renormalize();
        Self { q_b_to_n: q }
    }

    /// Construiește cuaternionul $\mathbf{q}_b^n$ din unghiurile Euler aeronautice Z-Y-X în radiani:
    /// `roll` ($\phi$), `pitch` ($\theta$), `yaw` ($\psi$).
    #[inline(always)]
    pub fn from_euler_rad(roll: f64, pitch: f64, yaw: f64) -> Self {
        let r = wrap_angle_rad(sanitize_f64(roll, -10.0 * PI, 10.0 * PI, 0.0));
        let p = wrap_angle_rad(sanitize_f64(pitch, -10.0 * PI, 10.0 * PI, 0.0));
        let y = wrap_angle_rad(sanitize_f64(yaw, -10.0 * PI, 10.0 * PI, 0.0));
        let mut q = UnitQuaternion::from_euler_angles(r, p, y);
        q.renormalize();
        Self { q_b_to_n: q }
    }

    /// Construiește cuaternionul $\mathbf{q}_b^n$ din unghiurile Euler în grade:
    /// `roll_deg` ($\phi$), `pitch_deg` ($\theta$), `yaw_deg` ($\psi$).
    #[inline(always)]
    pub fn from_euler_deg(roll_deg: f64, pitch_deg: f64, yaw_deg: f64) -> Self {
        Self::from_euler_rad(
            roll_deg.to_radians(),
            pitch_deg.to_radians(),
            yaw_deg.to_radians(),
        )
    }

    /// Extrage unghiurile Euler `(roll_rad, pitch_rad, yaw_rad)` fără `NaN` chiar și la $\theta = \pm 90^\circ$.
    #[inline(always)]
    pub fn to_euler_rad(&self) -> (f64, f64, f64) {
        let (r, p, y) = self.q_b_to_n.euler_angles();
        (
            if r.is_finite() { r } else { 0.0 },
            if p.is_finite() { p } else { 0.0 },
            if y.is_finite() { y } else { 0.0 },
        )
    }

    /// Extrage unghiurile Euler `[roll_deg, pitch_deg, yaw_deg]` pentru afișarea în Cockpit HUD.
    #[inline(always)]
    pub fn to_euler_deg(&self) -> [f64; 3] {
        let (r, p, y) = self.to_euler_rad();
        [r.to_degrees(), p.to_degrees(), y.to_degrees()]
    }

    /// Returnează componentele cuaternionului unitar ca `[qw, qx, qy, qz]`.
    #[inline(always)]
    pub fn quat_array(&self) -> [f64; 4] {
        [
            self.q_b_to_n.w,
            self.q_b_to_n.i,
            self.q_b_to_n.j,
            self.q_b_to_n.k,
        ]
    }

    /// Calculează matricea exactă de rotație 3D $\mathbf{R}_b^n(\mathbf{q}) \in SO(3)$ conform ecuației (2.1):
    /// transformă vectorii din `BodyFrame` în `NedFrameLrcv`.
    #[inline(always)]
    pub fn rotation_matrix_b_to_n(&self) -> Matrix3<f64> {
        let qw = self.q_b_to_n.w;
        let qx = self.q_b_to_n.i;
        let qy = self.q_b_to_n.j;
        let qz = self.q_b_to_n.k;

        Matrix3::new(
            1.0 - 2.0 * (qy * qy + qz * qz),
            2.0 * (qx * qy - qw * qz),
            2.0 * (qx * qz + qw * qy),
            2.0 * (qx * qy + qw * qz),
            1.0 - 2.0 * (qx * qx + qz * qz),
            2.0 * (qy * qz - qw * qx),
            2.0 * (qx * qz - qw * qy),
            2.0 * (qy * qz + qw * qx),
            1.0 - 2.0 * (qx * qx + qy * qy),
        )
    }

    /// Calculează matricea inversă (transpusă) $\mathbf{R}_n^b(\mathbf{q}) = (\mathbf{R}_b^n(\mathbf{q}))^T \in SO(3)$.
    #[inline(always)]
    pub fn rotation_matrix_n_to_b(&self) -> Matrix3<f64> {
        self.rotation_matrix_b_to_n().transpose()
    }

    /// Rotește un vector din `BodyFrame` ($\mathcal{F}_b$) în `NedFrameLrcv` ($\mathcal{F}_n$).
    #[inline(always)]
    pub fn body_to_ned(&self, v_b: FramedVec3<BodyFrame>) -> FramedVec3<NedFrameLrcv> {
        FramedVec3 {
            raw: self.q_b_to_n.transform_vector(&v_b.raw),
            _marker: PhantomData,
        }
    }

    /// Rotește un vector din `NedFrameLrcv` ($\mathcal{F}_n$) în `BodyFrame` ($\mathcal{F}_b$).
    #[inline(always)]
    pub fn ned_to_body(&self, v_n: FramedVec3<NedFrameLrcv>) -> FramedVec3<BodyFrame> {
        FramedVec3 {
            raw: self.q_b_to_n.inverse_transform_vector(&v_n.raw),
            _marker: PhantomData,
        }
    }

    /// Transformă un vector din `BodyFrame` ($\mathcal{F}_b$) în `CameraFrame` ($\mathcal{F}_c$).
    #[inline(always)]
    pub fn body_to_camera(&self, v_b: FramedVec3<BodyFrame>) -> FramedVec3<CameraFrame> {
        body_to_camera(v_b)
    }

    /// Transformă un vector din `CameraFrame` ($\mathcal{F}_c$) în `BodyFrame` ($\mathcal{F}_b$).
    #[inline(always)]
    pub fn camera_to_body(&self, v_c: FramedVec3<CameraFrame>) -> FramedVec3<BodyFrame> {
        camera_to_body(v_c)
    }

    /// Transformă direct un vector din `NedFrameLrcv` în `CameraFrame`.
    #[inline(always)]
    pub fn ned_to_camera(&self, v_n: FramedVec3<NedFrameLrcv>) -> FramedVec3<CameraFrame> {
        self.body_to_camera(self.ned_to_body(v_n))
    }

    /// Transformă direct un vector din `CameraFrame` în `NedFrameLrcv`.
    #[inline(always)]
    pub fn camera_to_ned(&self, v_c: FramedVec3<CameraFrame>) -> FramedVec3<NedFrameLrcv> {
        self.body_to_ned(self.camera_to_body(v_c))
    }

    /// Integrează pe varietatea $SO(3)$ viteza unghiulară $\boldsymbol{\omega}_b = [p, q, r]^T$ pe pasul `dt`:
    /// $\mathbf{q}_{k+1} = \mathbf{q}_k \otimes \exp(\frac{1}{2}\boldsymbol{\omega}_b \Delta t)$.
    #[inline(always)]
    pub fn integrate_body_rate(&mut self, omega_body: FramedVec3<BodyFrame>, dt: f64) {
        let safe_dt = sanitize_f64(dt, 0.0, 1.0, 0.0);
        let scaled_axis = Vector3::new(
            sanitize_f64(omega_body.raw.x, -50.0, 50.0, 0.0) * safe_dt,
            sanitize_f64(omega_body.raw.y, -50.0, 50.0, 0.0) * safe_dt,
            sanitize_f64(omega_body.raw.z, -50.0, 50.0, 0.0) * safe_dt,
        );
        self.q_b_to_n = self.q_b_to_n * UnitQuaternion::from_scaled_axis(scaled_axis);
        self.q_b_to_n.renormalize();
    }

    /// Aplică o corecție de eroare de atitudine $\delta\boldsymbol{\theta} \in \mathfrak{so}(3)$ din ES-EKF:
    /// $\mathbf{q} \leftarrow \mathbf{q} \otimes \exp(\frac{1}{2}\delta\boldsymbol{\theta})$.
    #[inline(always)]
    pub fn inject_error_angle(&mut self, delta_theta: &SVector<f64, 3>) {
        let safe_dtheta = Vector3::new(
            sanitize_f64(delta_theta[0], -1.0, 1.0, 0.0),
            sanitize_f64(delta_theta[1], -1.0, 1.0, 0.0),
            sanitize_f64(delta_theta[2], -1.0, 1.0, 0.0),
        );
        self.q_b_to_n = self.q_b_to_n * UnitQuaternion::from_scaled_axis(safe_dtheta);
        self.q_b_to_n.renormalize();
    }
}

/// Construiește matricea antisimetrică $[\mathbf{v}]_\times \in \mathfrak{so}(3)$ pentru produsul vectorial.
#[inline(always)]
pub fn skew_symmetric(v: &Vector3<f64>) -> SMatrix<f64, 3, 3> {
    SMatrix::<f64, 3, 3>::new(
        0.0, -v.z, v.y,
        v.z, 0.0, -v.x,
        -v.y, v.x, 0.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_f64_handles_nan_inf_and_clamping() {
        assert_eq!(sanitize_f64(f64::NAN, -10.0, 10.0, 2.5), 2.5);
        assert_eq!(sanitize_f64(f64::INFINITY, -10.0, 10.0, -3.0), -3.0);
        assert_eq!(sanitize_f64(f64::NEG_INFINITY, -10.0, 10.0, 99.0), 10.0);
        assert_eq!(sanitize_f64(25.0, -10.0, 10.0, 0.0), 10.0);
        assert_eq!(sanitize_f64(-25.0, -10.0, 10.0, 0.0), -10.0);
        assert_eq!(sanitize_f64(4.2, -10.0, 10.0, 0.0), 4.2);
    }

    #[test]
    fn test_rotation_matrix_orthogonality_and_norm_invariance() {
        let att = AttitudeQuaternion::from_euler_deg(37.5, -24.0, 87.0);
        let r_mat = att.rotation_matrix_b_to_n();
        let prod = r_mat * r_mat.transpose();
        let id = Matrix3::<f64>::identity();
        assert!((prod - id).norm() < 1e-12);
        assert!((r_mat.determinant() - 1.0).abs() < 1e-12);

        let v_body = FramedVec3::<BodyFrame>::new(68.5, -4.2, 3.1);
        let v_ned = att.body_to_ned(v_body);
        let v_body_back = att.ned_to_body(v_ned);

        assert!((v_body.norm() - v_ned.norm()).abs() < 1e-12);
        assert!((v_body.raw - v_body_back.raw).norm() < 1e-12);
        assert!((r_mat * v_body.raw - v_ned.raw).norm() < 1e-12);
    }

    #[test]
    fn test_gimbal_lock_immunity_at_pitch_plus_minus_90_deg() {
        for &pitch_deg in &[90.0_f64, -90.0_f64, 89.999_f64, -89.999_f64] {
            let mut att = AttitudeQuaternion::from_euler_deg(15.0, pitch_deg, 87.0);
            att.integrate_body_rate(FramedVec3::new(0.4, -0.3, 0.25), 0.0166);
            let q_arr = att.quat_array();
            for c in q_arr {
                assert!(c.is_finite());
            }
            let norm_q = (q_arr[0] * q_arr[0]
                + q_arr[1] * q_arr[1]
                + q_arr[2] * q_arr[2]
                + q_arr[3] * q_arr[3])
                .sqrt();
            assert!((norm_q - 1.0).abs() < 1e-12);

            let euler = att.to_euler_deg();
            assert!(euler[0].is_finite() && euler[1].is_finite() && euler[2].is_finite());
        }
    }

    #[test]
    fn test_body_camera_frame_roundtrip() {
        let att = AttitudeQuaternion::from_euler_deg(10.0, -3.0, 87.0);
        let v_b = FramedVec3::<BodyFrame>::new(70.0, -5.0, 2.5);
        let v_c = att.body_to_camera(v_b);
        assert_eq!(v_c.x(), -5.0);
        assert_eq!(v_c.y(), 2.5);
        assert_eq!(v_c.z(), 70.0);
        let v_b_rec = att.camera_to_body(v_c);
        assert!((v_b.raw - v_b_rec.raw).norm() < 1e-12);
    }
}
