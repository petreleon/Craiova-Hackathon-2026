//! # Generator Sintetic Vectorizat 6-DoF pe Dispozitiv (`Synthetic6DoFFogGenerator`)
//!
//! Generează loturi de antrenare și validare pe `Device` (`Device::Cuda(0)` sau `Device::Cpu`)
//! cu *Severe Domain Randomization* pentru pista **RWY 09 Craiova (`LRCV`)**:
//!
//! 1. **Inversa Adâncimii Fără Singularități la Orizont (Critica #11)**:
//!    $$\rho_{\text{ground}}(x_c, y_c) = \frac{\max\left(0,\; \cos\phi\cos\theta - x_c\sin\theta + y_c\sin\phi\cos\theta\right)}{\max(h_{\text{AGL}},\; 0.5\text{ m})} \ge 0$$
//!    Pentru orice rază vizuală deasupra orizontului (în timpul unui cabraj sau ruliu de $\pm 40^\circ$),
//!    numărătorul este clampat exact la $0.0$, prevenind matematic orice împărțire la zero (`NaN`/`Inf`).
//!
//! 2. **Atenuare Koschmieder & Penetrare SWIR/LWIR (Critica #1 & #3)**:
//!    $$\beta_{\text{VIS}} = \frac{3.0}{V_{\text{MOR}}}, \quad \beta_{\text{SWIR}} = 0.22\,\beta_{\text{VIS}}, \quad \hat{\rho}_{\text{fog}} = \text{clamp}(\beta_{\text{VIS}}, 0.002, 0.25)$$
//!
//! 3. **Mascarea Inelară a Haloului Mie pe Balizele ALS RWY 09 (Critica #2)**:
//!    În coroana de împrăștiere multiplă $r \in [r_{\text{core}}, 4 r_{\text{core}}]$ în jurul barelor
//!    luminoase ALS, masca este setată la $M_{\text{bloom}}(x_c, y_c) = 0.0$, blocând divergența falsă.

use crate::model_v2::{FogNavNetConfig, KINEMATIC_VECTOR_DIM};
use crate::pinn_loss::PinnBatchTargets;
use crate::AeroAiError;
use candle_core::{Device, Tensor};
use nalgebra::{Rotation3, UnitQuaternion, Vector3};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

/// Alias de compatibilitate cu specificația din `arhitectura/02_aero_ai_cuda.md`.
pub type CudaSyntheticFlightBatcher = Synthetic6DoFFogGenerator;

/// Seed-ul determinist standard al proiectului `AERO-SHIELD LRCV 2026`.
pub const DEFAULT_LRCV_RNG_SEED: u64 = 0x4C52_4356_2026;

/// Parametrii de *Severe Domain Randomization* pentru generarea sintetică 6-DoF.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DomainRandomizationConfig {
    /// Dimensiunea mini-lotului pe GPU (`batch_size = 16` implicit).
    pub batch_size: usize,
    /// Vizibilitatea meteorologică minimă $V_{\text{MOR}}$ ($\text{m}$), ex. $25\text{ m}$ (CAT IIIc).
    pub min_visibility_mor_m: f64,
    /// Vizibilitatea meteorologică maximă $V_{\text{MOR}}$ ($\text{m}$), ex. $800\text{ m}$.
    pub max_visibility_mor_m: f64,
    /// Probabilitatea de a injecta un front brusc de forfecare a ceții (*Sudden Fog Gust*).
    pub sudden_gust_probability: f64,
    /// Probabilitatea regimului extrem de `Flat Whiteout` (zero contrast în vizibil).
    pub flat_whiteout_probability: f64,
}

impl Default for DomainRandomizationConfig {
    fn default() -> Self {
        Self {
            batch_size: 16,
            min_visibility_mor_m: 30.0,
            max_visibility_mor_m: 600.0,
            sudden_gust_probability: 0.45,
            flat_whiteout_probability: 0.15,
        }
    }
}

/// Un mini-lot complet (intrări + ținte fizice PINN) alocat pe `Device` (`Cuda` sau `Cpu`).
#[derive(Debug, Clone)]
pub struct SyntheticBatch {
    /// Tensorul vizual multi-modal `[B, C_in, H, W]` pe dispozitivul țintă.
    pub visual_tensor: Tensor,
    /// Tensorul cinematic 6-DoF + meteo `[B, 18]` pe dispozitivul țintă.
    pub kinematic_tensor: Tensor,
    /// Țintele *Ground Truth* și mărimile fizice observate pentru `HeteroscedasticPinnLoss`.
    pub targets: PinnBatchTargets,
}

/// Generatorul sintetic 6-DoF pentru antrenarea și testarea `FogOpticalNavNetV2`.
#[derive(Debug, Clone)]
pub struct Synthetic6DoFFogGenerator {
    net_config: FogNavNetConfig,
    domain_config: DomainRandomizationConfig,
    device: Device,
    rng: ChaCha8Rng,
}

impl Synthetic6DoFFogGenerator {
    /// Creează un generator sintetic determinist pe `device`.
    pub fn new(
        net_config: FogNavNetConfig,
        domain_config: DomainRandomizationConfig,
        device: Device,
        seed: u64,
    ) -> Self {
        Self {
            net_config,
            domain_config,
            device,
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    /// Creează generatorul cu configurația standard LRCV V2.0.
    pub fn with_defaults(device: Device) -> Self {
        Self::new(
            FogNavNetConfig::default(),
            DomainRandomizationConfig::default(),
            device,
            DEFAULT_LRCV_RNG_SEED,
        )
    }

    /// Returnează referința către configurația rețelei.
    #[inline]
    pub const fn net_config(&self) -> &FogNavNetConfig {
        &self.net_config
    }

    /// Returnează referința către configurația de randomizare a domeniului.
    #[inline]
    pub const fn domain_config(&self) -> &DomainRandomizationConfig {
        &self.domain_config
    }

    /// Calculează exact inversa adâncimii planului solului $\rho_{\text{ground}}(x_c, y_c) = 1 / Z_{\text{ground}}$
    /// conform Remedierii Critice #11 (fără împărțire la zero la sau deasupra orizontului).
    #[inline]
    pub fn compute_inverse_depth_ground(
        xc: f64,
        yc: f64,
        roll_phi_rad: f64,
        pitch_theta_rad: f64,
        h_agl_meters: f64,
    ) -> f64 {
        let (s_phi, c_phi) = roll_phi_rad.sin_cos();
        let (s_theta, c_theta) = pitch_theta_rad.sin_cos();
        let ray_dot_down = c_phi * c_theta - xc * s_theta + yc * s_phi * c_theta;
        let safe_h = h_agl_meters.max(0.5);
        (ray_dot_down.max(0.0)) / safe_h
    }

    /// Calculează masca inelară de suprimare a haloului Mie $M_{\text{bloom}}(x_c, y_c) \in \{0.0, 1.0\}$
    /// în jurul unei balize luminoase ALS cu centrul $(x_{\text{ALS}}, y_{\text{ALS}})$ și raza nucleului $r_{\text{core}}$.
    #[inline]
    pub fn compute_als_bloom_mask(xc: f64, yc: f64, x_als: f64, y_als: f64, r_core: f64) -> f32 {
        let dx = xc - x_als;
        let dy = yc - y_als;
        let r = (dx * dx + dy * dy).sqrt();
        if r >= r_core && r <= 4.0 * r_core {
            0.0
        } else {
            1.0
        }
    }

    /// Generează următorul lot sintetic `SyntheticBatch` pe dispozitivul configurat (`Cuda` sau `Cpu`).
    pub fn next_batch(&mut self) -> Result<SyntheticBatch, AeroAiError> {
        let b_size = self.domain_config.batch_size.max(1);
        self.generate_batch_with_rng(b_size)
    }

    /// Generează un lot repetabil (cu seed fixat) util pentru benchmark-uri și teste unitare de convergență.
    pub fn generate_repeatable_calibration_batch(
        &self,
        batch_size: usize,
        seed: u64,
    ) -> Result<SyntheticBatch, AeroAiError> {
        let mut temp_gen = Self::new(
            self.net_config,
            self.domain_config,
            self.device.clone(),
            seed,
        );
        temp_gen.generate_batch_with_rng(batch_size.max(1))
    }

    fn generate_batch_with_rng(&mut self, batch_size: usize) -> Result<SyntheticBatch, AeroAiError> {
        let c_in = self.net_config.visual_channels;
        let h_img = self.net_config.image_size;
        let w_img = self.net_config.image_size;
        let pixels_per_ch = h_img * w_img;
        let stride_per_sample = c_in * pixels_per_ch;

        let mut visual_buf = vec![0.0_f32; batch_size * stride_per_sample];
        let mut kinematic_buf = vec![0.0_f32; batch_size * KINEMATIC_VECTOR_DIM];

        let mut v_ground_buf = vec![0.0_f32; batch_size * 3];
        let mut w_fog_buf = vec![0.0_f32; batch_size * 3];
        let mut rho_gnd_mean_buf = vec![0.0_f32; batch_size * 3];
        let mut rho_fog_scale_buf = vec![0.0_f32; batch_size * 3];
        let mut u_dot_obs_buf = vec![0.0_f32; batch_size * 3];
        let mut gust_label_buf = vec![0.0_f32; batch_size];
        let mut als_offset_buf = vec![0.0_f32; batch_size * 2];

        for b in 0..batch_size {
            // 1. Atitudinea 6-DoF pe cuaternioni unitari (Roll, Pitch, Yaw față de RWY 09)
            let roll_phi: f64 = self.rng.gen_range(-0.45..0.45); // +/- 25.7 deg
            let pitch_theta: f64 = self.rng.gen_range(-0.22..0.28); // coborâre / cabraj peste orizont
            let yaw_psi: f64 = self.rng.gen_range(-0.30..0.30);

            let rot = Rotation3::from_euler_angles(roll_phi, pitch_theta, yaw_psi);
            let mut quat = UnitQuaternion::from_rotation_matrix(&rot);
            quat.renormalize();

            let qw = quat.w;
            let qx = quat.i;
            let qy = quat.j;
            let qz = quat.k;

            // 2. Altitudinea AGL și vizibilitatea Koschmieder V_MOR
            let h_agl_m: f64 = self.rng.gen_range(8.0..220.0);
            let v_mor_m: f64 = self.rng.gen_range(
                self.domain_config.min_visibility_mor_m..self.domain_config.max_visibility_mor_m,
            );
            let beta_vis = 3.0 / v_mor_m.max(10.0);
            let rho_fog = beta_vis.clamp(0.002, 0.25);

            // 3. Viteza aeronavei în reperul corpului și transformarea prin q_b^n în reperul NED
            let u_body: f64 = self.rng.gen_range(55.0..82.0);
            let v_body: f64 = self.rng.gen_range(-6.0..6.0);
            let w_body: f64 = self.rng.gen_range(1.5..5.5); // rata de coborâre pe glideslope
            let v_body_vec = Vector3::new(u_body, v_body, w_body);
            let v_ground_ned = quat.transform_vector(&v_body_vec);

            // 4. Viteza ceții / vântului și evenimentul de rafală bruscă (Sudden Fog Gust)
            let is_gust = self.rng.gen_bool(self.domain_config.sudden_gust_probability);
            let gust_mult = if is_gust { 2.4 } else { 0.6 };
            let wx_ned: f64 = self.rng.gen_range(-10.0..10.0) * gust_mult;
            let wy_ned: f64 = self.rng.gen_range(-14.0..14.0) * gust_mult;
            let wz_ned: f64 = self.rng.gen_range(-2.5..2.5) * gust_mult;
            let w_fog_ned = Vector3::new(wx_ned, wy_ned, wz_ned);

            // 5. Ratele unghiulare pre-integrate și accelerațiile IMU
            let p_rate: f64 = self.rng.gen_range(-0.6..0.6);
            let q_rate: f64 = self.rng.gen_range(-0.35..0.35);
            let r_rate: f64 = self.rng.gen_range(-0.35..0.35);

            let ax_body: f64 = self.rng.gen_range(-2.5..2.5);
            let ay_body: f64 = (wy_ned * 0.12) + self.rng.gen_range(-1.0..1.0);
            let az_body: f64 = -9.81 + self.rng.gen_range(-1.2..1.2);

            // 6. Offsetul terminal față de axul luminos ALS RWY 09
            let dy_als_m: f64 = self.rng.gen_range(-18.0..18.0);
            let dpsi_als_rad: f64 = yaw_psi;
            let x_als_cam = (dy_als_m / 25.0).clamp(-0.75, 0.75);
            let y_als_cam = 0.25_f64;
            let r_core = 0.08_f64;

            let is_flat_whiteout = self
                .rng
                .gen_bool(self.domain_config.flat_whiteout_probability);

            // Populăm vectorul cinematic [B, 18]
            let k_base = b * KINEMATIC_VECTOR_DIM;
            let (s_phi, c_phi) = roll_phi.sin_cos();
            let (s_theta, c_theta) = pitch_theta.sin_cos();
            let (s_psi, c_psi) = yaw_psi.sin_cos();

            kinematic_buf[k_base] = qw as f32;
            kinematic_buf[k_base + 1] = qx as f32;
            kinematic_buf[k_base + 2] = qy as f32;
            kinematic_buf[k_base + 3] = qz as f32;
            kinematic_buf[k_base + 4] = s_phi as f32;
            kinematic_buf[k_base + 5] = c_phi as f32;
            kinematic_buf[k_base + 6] = s_theta as f32;
            kinematic_buf[k_base + 7] = c_theta as f32;
            kinematic_buf[k_base + 8] = s_psi as f32;
            kinematic_buf[k_base + 9] = c_psi as f32;
            kinematic_buf[k_base + 10] = p_rate as f32;
            kinematic_buf[k_base + 11] = q_rate as f32;
            kinematic_buf[k_base + 12] = r_rate as f32;
            kinematic_buf[k_base + 13] = (ax_body * 0.1) as f32;
            kinematic_buf[k_base + 14] = (ay_body * 0.1) as f32;
            kinematic_buf[k_base + 15] = (az_body * 0.1) as f32;
            kinematic_buf[k_base + 16] = (h_agl_m / 250.0) as f32;
            kinematic_buf[k_base + 17] = rho_fog as f32;

            // Populăm grila vizuală [C_in, H, W]
            let v_base = b * stride_per_sample;
            let mut rho_gnd_accum = 0.0_f64;

            let w_photon_ch = self.net_config.w_photon_channel_idx.min(c_in - 1);
            let m_bloom_ch = self.net_config.m_bloom_channel_idx.min(c_in - 1);
            let rho_gnd_ch = self.net_config.rho_ground_channel_idx.min(c_in - 1);

            for iy in 0..h_img {
                let yc = -1.0 + 2.0 * (iy as f64 + 0.5) / (h_img as f64);
                for ix in 0..w_img {
                    let xc = -1.0 + 2.0 * (ix as f64 + 0.5) / (w_img as f64);
                    let px_idx = iy * w_img + ix;

                    let rho_gnd = Self::compute_inverse_depth_ground(
                        xc,
                        yc,
                        roll_phi,
                        pitch_theta,
                        h_agl_m,
                    );
                    rho_gnd_accum += rho_gnd;

                    let m_bloom =
                        Self::compute_als_bloom_mask(xc, yc, x_als_cam, y_als_cam, r_core);
                    let w_photon: f32 = if is_flat_whiteout {
                        0.02
                    } else if rho_gnd > 0.0 {
                        ((-beta_vis * 0.22 / rho_gnd.max(1e-3)).exp().clamp(0.15, 0.98)) as f32
                    } else {
                        0.25
                    };

                    // Fluxul translațional derotat pe pixel (combinând solul și stratul de ceață)
                    let flow_gnd_x = rho_gnd * v_ground_ned.x;
                    let flow_gnd_y = rho_gnd * v_ground_ned.y;
                    let flow_fog_x = rho_fog * (v_ground_ned.x - w_fog_ned.x);
                    let flow_fog_y = rho_fog * (v_ground_ned.y - w_fog_ned.y);
                    let flow_fog_z = rho_fog * (v_ground_ned.z - w_fog_ned.z);

                    // Canalele 0..3: Intensitatea Vizibil + SWIR CLAHE + Semnătura ALS
                    let dist_als_sq = (xc - x_als_cam).powi(2) + (yc - y_als_cam).powi(2);
                    let als_beacon = (-dist_als_sq / (2.0 * r_core * r_core)).exp();
                    visual_buf[v_base + px_idx] =
                        (0.35 * als_beacon + 0.02 * v_ground_ned.x * rho_gnd) as f32;
                    visual_buf[v_base + pixels_per_ch + px_idx] =
                        (0.55 * als_beacon + 0.02 * v_ground_ned.y * rho_gnd) as f32;
                    visual_buf[v_base + 2 * pixels_per_ch + px_idx] =
                        (0.75 * als_beacon - 0.04 * dpsi_als_rad) as f32;

                    // Canalele 3..7: Fluxul optic derotat multi-scală (1x și 0.5x)
                    if c_in >= 7 {
                        visual_buf[v_base + 3 * pixels_per_ch + px_idx] =
                            (0.05 * (flow_gnd_x + flow_fog_x)) as f32;
                        visual_buf[v_base + 4 * pixels_per_ch + px_idx] =
                            (0.05 * (flow_gnd_y + flow_fog_y)) as f32;
                        visual_buf[v_base + 5 * pixels_per_ch + px_idx] =
                            (0.05 * (flow_fog_x - 0.5 * flow_gnd_x)) as f32;
                        visual_buf[v_base + 6 * pixels_per_ch + px_idx] =
                            (0.05 * (flow_fog_y + flow_fog_z)) as f32;
                    }

                    // Canalele fizice dedicate: w_photon, M_bloom, rho_ground
                    visual_buf[v_base + w_photon_ch * pixels_per_ch + px_idx] = w_photon;
                    visual_buf[v_base + m_bloom_ch * pixels_per_ch + px_idx] = m_bloom;
                    visual_buf[v_base + rho_gnd_ch * pixels_per_ch + px_idx] = rho_gnd as f32;
                }
            }

            let mean_rho_gnd = (rho_gnd_accum / (pixels_per_ch as f64)).max(1e-4);

            // Populăm țintele fizice PINN
            let t3 = b * 3;
            v_ground_buf[t3] = v_ground_ned.x as f32;
            v_ground_buf[t3 + 1] = v_ground_ned.y as f32;
            v_ground_buf[t3 + 2] = v_ground_ned.z as f32;

            w_fog_buf[t3] = w_fog_ned.x as f32;
            w_fog_buf[t3 + 1] = w_fog_ned.y as f32;
            w_fog_buf[t3 + 2] = w_fog_ned.z as f32;

            for k in 0..3 {
                rho_gnd_mean_buf[t3 + k] = mean_rho_gnd as f32;
                rho_fog_scale_buf[t3 + k] = rho_fog as f32;
                let vg = v_ground_buf[t3 + k] as f64;
                let wf = w_fog_buf[t3 + k] as f64;
                u_dot_obs_buf[t3 + k] = (mean_rho_gnd * vg + rho_fog * (vg - wf)) as f32;
            }

            gust_label_buf[b] = if is_gust { 1.0 } else { 0.0 };

            let t2 = b * 2;
            als_offset_buf[t2] = dy_als_m as f32;
            als_offset_buf[t2 + 1] = dpsi_als_rad as f32;
        }

        let visual_tensor = Tensor::from_vec(
            visual_buf,
            (batch_size, c_in, h_img, w_img),
            &self.device,
        )?;
        let kinematic_tensor = Tensor::from_vec(
            kinematic_buf,
            (batch_size, KINEMATIC_VECTOR_DIM),
            &self.device,
        )?;

        let targets = PinnBatchTargets {
            v_ground_ned: Tensor::from_vec(v_ground_buf, (batch_size, 3), &self.device)?,
            w_fog_ned: Tensor::from_vec(w_fog_buf, (batch_size, 3), &self.device)?,
            rho_ground_mean: Tensor::from_vec(rho_gnd_mean_buf, (batch_size, 3), &self.device)?,
            rho_fog_scale: Tensor::from_vec(rho_fog_scale_buf, (batch_size, 3), &self.device)?,
            u_dot_trans_obs: Tensor::from_vec(u_dot_obs_buf, (batch_size, 3), &self.device)?,
            gust_label: Tensor::from_vec(gust_label_buf, (batch_size, 1), &self.device)?,
            als_offset: Tensor::from_vec(als_offset_buf, (batch_size, 2), &self.device)?,
        };

        Ok(SyntheticBatch {
            visual_tensor,
            kinematic_tensor,
            targets,
        })
    }
}
