//! # Arhitectura Neurală `FogOpticalNavNetV2` (`candle-core` + `candle-nn`)
//!
//! Construită **100% din primitive oficiale `candle_nn`** (`Conv2d`, `GroupNorm`, `Linear`,
//! `candle_nn::rnn::GRU`) îmbinate cu un strat fizic explicit de **Attention-Weighted Spatial Pooling
//! & Inverse-Depth Decoupling**:
//!
//! ## Formule Matematice Principale
//! 1. **Dual-Branch Stem**:
//!    - *Ramura 1 (Ground & ALS Micro-Gradient)*:
//!      $$\mathbf{F}_{\text{gnd}} = \text{SiLU}\!\left(\text{Conv}_{3\times 3, s=2}\!\left(\text{SiLU}(\text{GN}_4(\text{Conv}_{3\times 3}(\mathbf{V})))\right)\right) \in \mathbb{R}^{B \times 64 \times \frac{H}{2} \times \frac{W}{2}}$$
//!    - *Ramura 2 (Volumetric Fog & Shear Flow)*:
//!      $$\mathbf{F}_{\text{fog}} = \text{SiLU}\!\left(\text{Conv}_{3\times 3, s=2}\!\left(\text{SiLU}(\text{Conv}_{3\times 3, d=2}(\mathbf{V}))\right)\right) \in \mathbb{R}^{B \times 64 \times \frac{H}{2} \times \frac{W}{2}}$$
//!
//! 2. **FogSpatialAttentionPool2d (cu Mascarea Haloului Mie $M_{\text{bloom}}$ și Pondere Fotonică $w_{\text{photon}}$)**:
//!    $$\mathbf{A} = \sigma\!\left(\text{Conv}_{1\times 1}([\mathbf{F}_{\text{gnd}}, \mathbf{F}_{\text{fog}}])\right) \odot \tilde{M}_{\text{bloom}} \in [0, 1]^{B \times 2 \times \frac{H}{2} \times \frac{W}{2}}$$
//!    $$\mathbf{f}_{\text{gnd}} = \frac{\sum_{u,v} (A_{\text{gnd}}(u,v)\tilde{w}_{\text{photon}}(u,v)) \mathbf{F}_{\text{gnd}}(u,v)}{\sum_{u,v} A_{\text{gnd}}(u,v)\tilde{w}_{\text{photon}}(u,v) + \epsilon}, \quad \mathbf{f}_{\text{fog}} = \frac{\sum_{u,v} A_{\text{fog}}(u,v) \mathbf{F}_{\text{fog}}(u,v)}{\sum_{u,v} A_{\text{fog}}(u,v) + \epsilon}$$
//!
//! 3. **Fuziune Cinematică 6-DoF & Temporal GRU ($60\text{ Hz}$)**:
//!    $$\mathbf{z}_t = \text{SiLU}\!\left(\mathbf{W}_{\text{phys}} [\mathbf{f}_{\text{gnd}}, \mathbf{f}_{\text{fog}}, \mathbf{K}_{6\text{dof}}] + \mathbf{b}_{\text{phys}}\right) \in \mathbb{R}^{B \times 128}$$
//!    $$\mathbf{h}_t = \text{GRU}(\mathbf{z}_t, \mathbf{h}_{t-1}) \in \mathbb{R}^{B \times 128}$$
//!
//! 4. **Capete Multi-Task Heteroscedastice cu Clampare Garantată $\mathbf{s}_v, \mathbf{s}_w \in [-10.0, 6.0]$**:
//!    $$\hat{\sigma}_{v,k}^2 = \exp(\text{clamp}(s_{v,k}, -10.0, 6.0)), \quad \hat{\sigma}_{w,k}^2 = \exp(\text{clamp}(s_{w,k}, -10.0, 6.0))$$

use crate::AeroAiError;
use candle_core::{Module, Tensor};
use candle_nn::ops::sigmoid;
use candle_nn::rnn::{gru, GRUConfig, GRUState, GRU, RNN};
use candle_nn::{conv2d, group_norm, linear, Conv2d, Conv2dConfig, GroupNorm, Linear, VarBuilder};
use serde::{Deserialize, Serialize};

/// Numărul implicit de canale vizuale în specificația `02_aero_ai_cuda.md`:
/// - `0..3`: 3 cadre consecutive $(I_{t-2}, I_{t-1}, I_t)$ Vizibil + SWIR CLAHE,
/// - `3..7`: Flux optic derotat pe cuaternioni $(u_{1\times}, v_{1\times}, u_{0.5\times}, v_{0.5\times})$,
/// - `7`: Harta ponderii de certitudine fotonică $w_{\text{photon}} \in [0, 1]$,
/// - `8`: Masca inelară de suprimare a haloului ALS $M_{\text{bloom}} \in [0, 1]$,
/// - `9`: Harta inversei adâncimii solului $\rho_{\text{ground}}(x_c, y_c) \ge 0$.
pub const DEFAULT_VISUAL_CHANNELS: usize = 10;

/// Configurația extinsă cu 12 canale (inclusiv CoordConv explicit $(x_c, y_c)$ și SWIR separat).
pub const EXTENDED_VISUAL_CHANNELS: usize = 12;

/// Rezoluția spațială implicită a grilei vizuale procesate la $60\text{ Hz}$ ($64 \times 64$).
pub const DEFAULT_IMAGE_SIZE: usize = 64;

/// Dimensiunea vectorului cinematic 6-DoF + meteo $\mathbf{K}_{6\text{dof}} \in \mathbb{R}^{18}$:
/// - `0..4`: Cuaternionul unitar $\mathbf{q}_b^n = [q_w, q_x, q_y, q_z]$,
/// - `4..10`: $[\sin\phi, \cos\phi, \sin\theta, \cos\theta, \sin\psi, \cos\psi]$,
/// - `10..13`: Ratele unghiulare pre-integrate $\bar{\boldsymbol{\omega}}_k = [\bar{p}, \bar{q}, \bar{r}]$,
/// - `13..16`: Accelerațiile în reperul aeronavei $\mathbf{a}_b = [a_x, a_y, a_z]$,
/// - `16`: Altitudinea barometrică normalizată $h_{\text{baro}}$,
/// - `17`: Scara inversei adâncimii ceții $\hat{\rho}_{\text{fog}} \approx 3.0 / V_{\text{MOR}}$.
pub const KINEMATIC_VECTOR_DIM: usize = 18;

/// Dimensiunea stării ascunse a celulei `candle_nn::rnn::GRU` ($128$).
pub const DEFAULT_GRU_HIDDEN_DIM: usize = 128;

/// Limita inferioară pentru log-varianța incertitudinii heteroscedastice: $\exp(-10) \approx 4.54 \times 10^{-5}$.
pub const LOG_VAR_MIN: f32 = -10.0;

/// Limita superioară pentru log-varianța incertitudinii heteroscedastice: $\exp(6) \approx 403.43$.
pub const LOG_VAR_MAX: f32 = 6.0;

/// Configurația arhitecturală a modelului `FogOpticalNavNetV2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FogNavNetConfig {
    /// Numărul de canale ale tensorului vizual (`10` implicit sau `12` extins).
    pub visual_channels: usize,
    /// Rezoluția spațială de intrare ($H = W$, ex. `64` sau `128`).
    pub image_size: usize,
    /// Dimensiunea vectorului cinematic 6-DoF (`18`).
    pub kinematic_dim: usize,
    /// Dimensiunea stării ascunse `GRU` (`128`).
    pub gru_hidden_dim: usize,
    /// Indexul canalului $w_{\text{photon}}$ în tensorul vizual.
    pub w_photon_channel_idx: usize,
    /// Indexul canalului $M_{\text{bloom}}$ în tensorul vizual.
    pub m_bloom_channel_idx: usize,
    /// Indexul canalului $\rho_{\text{ground}}$ în tensorul vizual.
    pub rho_ground_channel_idx: usize,
}

impl Default for FogNavNetConfig {
    fn default() -> Self {
        Self::default_10ch()
    }
}

impl FogNavNetConfig {
    /// Configurația standard cu 10 canale vizuale `[B, 10, 64, 64]` din `arhitectura/02_aero_ai_cuda.md`.
    #[inline]
    pub const fn default_10ch() -> Self {
        Self {
            visual_channels: DEFAULT_VISUAL_CHANNELS,
            image_size: DEFAULT_IMAGE_SIZE,
            kinematic_dim: KINEMATIC_VECTOR_DIM,
            gru_hidden_dim: DEFAULT_GRU_HIDDEN_DIM,
            w_photon_channel_idx: 7,
            m_bloom_channel_idx: 8,
            rho_ground_channel_idx: 9,
        }
    }

    /// Configurația extinsă cu 12 canale vizuale `[B, 12, 64, 64]`.
    #[inline]
    pub const fn extended_12ch(image_size: usize) -> Self {
        Self {
            visual_channels: EXTENDED_VISUAL_CHANNELS,
            image_size,
            kinematic_dim: KINEMATIC_VECTOR_DIM,
            gru_hidden_dim: DEFAULT_GRU_HIDDEN_DIM,
            w_photon_channel_idx: 9,
            m_bloom_channel_idx: 4,
            rho_ground_channel_idx: 11,
        }
    }
}

/// Extractorul de trăsături cu două ramuri paralele (`DualBandStem`):
/// - **Ramura 1 (`gnd`)**: Receptive field compact ($k=3, d=1$) + `GroupNorm(4, 32)` pentru
///   micro-gradienții solului și barele luminoase ALS RWY 09.
/// - **Ramura 2 (`fog`)**: Convoluție dilatată ($k=3, d=2, \text{pad}=2$) pentru structura
///   volumetrică a ceții și fronturile de forfecare a vântului (*wind shear*).
#[derive(Debug, Clone)]
pub struct DualBandStem {
    gnd_conv1: Conv2d,
    gnd_gn1: GroupNorm,
    gnd_conv2: Conv2d,
    fog_conv1: Conv2d,
    fog_conv2: Conv2d,
}

impl DualBandStem {
    /// Construiește `DualBandStem` folosind primitive oficiale `candle_nn`.
    pub fn new(in_channels: usize, vb: VarBuilder) -> Result<Self, AeroAiError> {
        let cfg_k3_s1 = Conv2dConfig {
            padding: 1,
            stride: 1,
            dilation: 1,
            groups: 1,
            ..Default::default()
        };
        let cfg_k3_s2 = Conv2dConfig {
            padding: 1,
            stride: 2,
            dilation: 1,
            groups: 1,
            ..Default::default()
        };
        let cfg_k3_dil2 = Conv2dConfig {
            padding: 2,
            stride: 1,
            dilation: 2,
            groups: 1,
            ..Default::default()
        };

        let gnd_conv1 = conv2d(in_channels, 32, 3, cfg_k3_s1, vb.pp("gnd_conv1"))?;
        let gnd_gn1 = group_norm(4, 32, 1e-5, vb.pp("gnd_gn1"))?;
        let gnd_conv2 = conv2d(32, 64, 3, cfg_k3_s2, vb.pp("gnd_conv2"))?;

        let fog_conv1 = conv2d(in_channels, 32, 3, cfg_k3_dil2, vb.pp("fog_conv1"))?;
        let fog_conv2 = conv2d(32, 64, 3, cfg_k3_s2, vb.pp("fog_conv2"))?;

        Ok(Self {
            gnd_conv1,
            gnd_gn1,
            gnd_conv2,
            fog_conv1,
            fog_conv2,
        })
    }

    /// Evaluează ambele ramuri pe tensorul vizual `[B, C_in, H, W]` și returnează
    /// `(f_gnd_map, f_fog_map)` fiecare de dimensiune `[B, 64, H/2, W/2]`.
    pub fn forward(&self, visual: &Tensor) -> Result<(Tensor, Tensor), AeroAiError> {
        // Ramura 1: Ground & ALS Micro-Gradient
        let g1 = self.gnd_conv1.forward(visual)?;
        let g1_norm = self.gnd_gn1.forward(&g1)?.silu()?;
        let f_gnd_map = self.gnd_conv2.forward(&g1_norm)?.silu()?;

        // Ramura 2: Volumetric Fog & Shear Flow (Dilated Conv d=2)
        let f1 = self.fog_conv1.forward(visual)?.silu()?;
        let f_fog_map = self.fog_conv2.forward(&f1)?.silu()?;

        Ok((f_gnd_map, f_fog_map))
    }
}

/// Poarta de atenție spațială informată fizic (`FogSpatialAttentionPool2d`):
/// Aplică `Conv2d(128 -> 2, k=1)` + `Sigmoid`, suprimă divergența falsă din coroana
/// haloului Mie prin multiplicarea cu masca $M_{\text{bloom}}$ și ponderează trăsăturile
/// solului cu certitudinea fotonică $w_{\text{photon}}$.
#[derive(Debug, Clone)]
pub struct FogSpatialAttentionPool2d {
    att_conv: Conv2d,
}

impl FogSpatialAttentionPool2d {
    /// Construiește stratul de atenție spațială `Conv2d(128 -> 2, k=1)`.
    pub fn new(vb: VarBuilder) -> Result<Self, AeroAiError> {
        let cfg_k1 = Conv2dConfig {
            padding: 0,
            stride: 1,
            dilation: 1,
            groups: 1,
            ..Default::default()
        };
        let att_conv = conv2d(128, 2, 1, cfg_k1, vb.pp("att_conv"))?;
        Ok(Self { att_conv })
    }

    /// Execută pooling-ul global ponderat cu atenție, $M_{\text{bloom}}$ și $w_{\text{photon}}$,
    /// returnând `(f_gnd [B, 64], f_fog [B, 64])`.
    pub fn forward(
        &self,
        f_gnd_map: &Tensor,
        f_fog_map: &Tensor,
        w_photon_full: &Tensor,
        m_bloom_full: &Tensor,
    ) -> Result<(Tensor, Tensor), AeroAiError> {
        // Concatenare [B, 64, H/2, W/2] + [B, 64, H/2, W/2] -> [B, 128, H/2, W/2]
        let f_cat = Tensor::cat(&[f_gnd_map, f_fog_map], 1)?;
        let att_logits = self.att_conv.forward(&f_cat)?;
        let att_gates = sigmoid(&att_logits)?;

        let att_gnd = att_gates.narrow(1, 0, 1)?; // [B, 1, H/2, W/2]
        let att_fog = att_gates.narrow(1, 1, 1)?; // [B, 1, H/2, W/2]

        // Sub-eșantionare 2x2 a măștilor fizice la rezoluția hărților de trăsături [B, 1, H/2, W/2]
        let w_photon_ds = w_photon_full.avg_pool2d(2)?.clamp(0.0f32, 1.0f32)?;
        let m_bloom_ds = m_bloom_full.avg_pool2d(2)?.clamp(0.0f32, 1.0f32)?;

        // Ponderile pentru sol: suprimă atât haloul Mie (M_bloom = 0), cât și zgomotul din Flat Whiteout (w_photon = 0)
        let w_gnd = (&(&att_gnd * &m_bloom_ds)? * &w_photon_ds)?.affine(1.0, 1e-4)?;
        // Ponderile pentru ceață: suprimă haloul artificial ALS (M_bloom = 0), păstrând fluxul volumetric
        let w_fog_mod = w_photon_ds.affine(0.85, 0.15)?;
        let w_fog = (&(&att_fog * &m_bloom_ds)? * &w_fog_mod)?.affine(1.0, 1e-4)?;

        // Weighted Global Spatial Pooling peste dimensiunile spațiale (2, 3)
        let weighted_gnd = f_gnd_map.broadcast_mul(&w_gnd)?;
        let num_gnd = weighted_gnd.sum((2, 3))?; // [B, 64]
        let den_gnd = w_gnd.sum((2, 3))?.affine(1.0, 1e-5)?; // [B, 1]
        let f_gnd = num_gnd.broadcast_div(&den_gnd)?;

        let weighted_fog = f_fog_map.broadcast_mul(&w_fog)?;
        let num_fog = weighted_fog.sum((2, 3))?; // [B, 64]
        let den_fog = w_fog.sum((2, 3))?.affine(1.0, 1e-5)?; // [B, 1]
        let f_fog = num_fog.broadcast_div(&den_fog)?;

        Ok((f_gnd, f_fog))
    }
}

/// Cele 3 capete multi-task heteroscedastice (`HeteroscedasticMultiTaskHeads`):
/// - **Head 1**: $\hat{\mathbf{v}}_{\text{ground}} \in \mathbb{R}^{B \times 3}$ + log-varianța $\mathbf{s}_v \in [-10.0, 6.0]^{B \times 3}$
/// - **Head 2**: $\hat{\mathbf{w}}_{\text{fog}} \in \mathbb{R}^{B \times 3}$ + log-varianța $\mathbf{s}_w \in [-10.0, 6.0]^{B \times 3}$ + `gust_logit` $\in \mathbb{R}^{B \times 1}$
/// - **Head 3**: Corecția terminală ALS RWY 09 $(\Delta y_{\text{ALS}}, \Delta\psi_{\text{ALS}}) \in \mathbb{R}^{B \times 2}$
#[derive(Debug, Clone)]
pub struct HeteroscedasticMultiTaskHeads {
    head1_fc1: Linear,
    head1_out: Linear,
    head2_fc1: Linear,
    head2_out: Linear,
    head3_fc1: Linear,
    head3_out: Linear,
}

impl HeteroscedasticMultiTaskHeads {
    /// Construiește capetele de predicție din starea ascunsă GRU (`hidden_dim = 128`).
    pub fn new(hidden_dim: usize, vb: VarBuilder) -> Result<Self, AeroAiError> {
        let head1_fc1 = linear(hidden_dim, 64, vb.pp("head1_fc1"))?;
        let head1_out = linear(64, 6, vb.pp("head1_out"))?;

        let head2_fc1 = linear(hidden_dim, 64, vb.pp("head2_fc1"))?;
        let head2_out = linear(64, 7, vb.pp("head2_out"))?;

        let head3_fc1 = linear(hidden_dim, 32, vb.pp("head3_fc1"))?;
        let head3_out = linear(32, 2, vb.pp("head3_out"))?;

        Ok(Self {
            head1_fc1,
            head1_out,
            head2_fc1,
            head2_out,
            head3_fc1,
            head3_out,
        })
    }

    /// Evaluează cele 3 capete pe starea ascunsă `h_t` `[B, hidden_dim]`.
    /// Log-varianțele `s_v` și `s_w` sunt clampate obligatoriu în `[-10.0, 6.0]`.
    pub fn forward(
        &self,
        h_t: &Tensor,
    ) -> Result<(Tensor, Tensor, Tensor, Tensor, Tensor, Tensor, Tensor), AeroAiError> {
        // Head 1: Ground Velocity + Log-Variance s_v
        let h1 = self.head1_fc1.forward(h_t)?.silu()?;
        let out1 = self.head1_out.forward(&h1)?;
        let v_ground_ned = out1.narrow(1, 0, 3)?;
        let s_v_raw = out1.narrow(1, 3, 3)?;
        let s_v_log_var = s_v_raw.clamp(LOG_VAR_MIN, LOG_VAR_MAX)?;

        // Head 2: Fog Velocity + Log-Variance s_w + Gust Lookahead Score
        let h2 = self.head2_fc1.forward(h_t)?.silu()?;
        let out2 = self.head2_out.forward(&h2)?;
        let w_fog_ned = out2.narrow(1, 0, 3)?;
        let s_w_raw = out2.narrow(1, 3, 3)?;
        let s_w_log_var = s_w_raw.clamp(LOG_VAR_MIN, LOG_VAR_MAX)?;
        let gust_logit = out2.narrow(1, 6, 1)?;
        let gust_prob = sigmoid(&gust_logit)?;

        // Head 3: Terminal ALS Offset (dy_ALS, dpsi_ALS)
        let h3 = self.head3_fc1.forward(h_t)?.silu()?;
        let als_offset = self.head3_out.forward(&h3)?;

        Ok((
            v_ground_ned,
            s_v_log_var,
            w_fog_ned,
            s_w_log_var,
            gust_logit,
            gust_prob,
            als_offset,
        ))
    }
}

/// Ieșirea în format tensorial a unui pas `forward_step` al rețelei `FogOpticalNavNetV2`.
#[derive(Debug, Clone)]
pub struct FogNavForwardTensors {
    /// Viteza estimată față de sol în reperul NED `[B, 3]` ($\text{m/s}$).
    pub v_ground_ned: Tensor,
    /// Log-varianța incertitudinii vitezei față de sol $\mathbf{s}_v \in [-10.0, 6.0]$ `[B, 3]`.
    pub s_v_log_var: Tensor,
    /// Viteza estimată a ceții/vântului în reperul NED `[B, 3]` ($\text{m/s}$).
    pub w_fog_ned: Tensor,
    /// Log-varianța incertitudinii vitezei ceții $\mathbf{s}_w \in [-10.0, 6.0]$ `[B, 3]`.
    pub s_w_log_var: Tensor,
    /// Logit-ul brut pentru detecția anticipativă a rafalei `[B, 1]`.
    pub gust_logit: Tensor,
    /// Probabilitatea $[0, 1]$ a unei rafale la $35\text{–}80\text{ m}$ în fața aeronavei `[B, 1]`.
    pub gust_prob: Tensor,
    /// Corecția terminală față de axul luminos ALS RWY 09 $(\Delta y_{\text{ALS}}, \Delta\psi_{\text{ALS}})$ `[B, 2]`.
    pub als_offset: Tensor,
    /// Starea ascunsă actualizată a celulei `GRU` `[B, 128]`.
    pub next_gru_state: GRUState,
    /// Trăsăturile extrase pe ramura de sol `[B, 64]`.
    pub f_gnd_features: Tensor,
    /// Trăsăturile extrase pe ramura de ceață volumetrică `[B, 64]`.
    pub f_fog_features: Tensor,
}

impl FogNavForwardTensors {
    /// Decodează primul element din batch (`batch_idx = 0`) într-o structură Rust `NavPredictionOutput`
    /// cu verificarea finitudinii și fără niciun `.unwrap()`.
    pub fn decode_first_sample(&self) -> Result<NavPredictionOutput, AeroAiError> {
        self.decode_batch_sample(0)
    }

    /// Decodează elementul `batch_idx` din batch în `NavPredictionOutput`.
    pub fn decode_batch_sample(&self, batch_idx: usize) -> Result<NavPredictionOutput, AeroAiError> {
        let v_gnd_vec = extract_row_f32(&self.v_ground_ned, batch_idx, 3, "v_ground_ned")?;
        let s_v_vec = extract_row_f32(&self.s_v_log_var, batch_idx, 3, "s_v_log_var")?;
        let w_fog_vec = extract_row_f32(&self.w_fog_ned, batch_idx, 3, "w_fog_ned")?;
        let s_w_vec = extract_row_f32(&self.s_w_log_var, batch_idx, 3, "s_w_log_var")?;
        let gust_vec = extract_row_f32(&self.gust_prob, batch_idx, 1, "gust_prob")?;
        let als_vec = extract_row_f32(&self.als_offset, batch_idx, 2, "als_offset")?;

        let mut v_ground_ned = [0.0_f64; 3];
        let mut sigma2_v_ground = [1.0_f64; 3];
        let mut w_fog_ned = [0.0_f64; 3];
        let mut sigma2_w_fog = [1.0_f64; 3];

        for k in 0..3 {
            let vg = sanitize_scalar(v_gnd_vec[k] as f64, 0.0, -250.0, 250.0);
            let sv = sanitize_scalar(
                s_v_vec[k] as f64,
                0.0,
                LOG_VAR_MIN as f64,
                LOG_VAR_MAX as f64,
            );
            let wf = sanitize_scalar(w_fog_vec[k] as f64, 0.0, -120.0, 120.0);
            let sw = sanitize_scalar(
                s_w_vec[k] as f64,
                0.0,
                LOG_VAR_MIN as f64,
                LOG_VAR_MAX as f64,
            );

            v_ground_ned[k] = vg;
            sigma2_v_ground[k] = sv.exp();
            w_fog_ned[k] = wf;
            sigma2_w_fog[k] = sw.exp();
        }

        let gust_lookahead_score = sanitize_scalar(gust_vec[0] as f64, 0.0, 0.0, 1.0);
        let als_lateral_offset_m = sanitize_scalar(als_vec[0] as f64, 0.0, -150.0, 150.0);
        let als_heading_error_rad = sanitize_scalar(als_vec[1] as f64, 0.0, -1.57, 1.57);

        Ok(NavPredictionOutput {
            v_ground_ned,
            sigma2_v_ground,
            w_fog_ned,
            sigma2_w_fog,
            gust_lookahead_score,
            als_lateral_offset_m,
            als_heading_error_rad,
        })
    }
}

/// Structura de ieșire decodată în precizie dublă (`f64`) pentru bucla de $60\text{ Hz}$ și filtrul ES-EKF.
/// Conformă 100% cu specificația din `arhitectura/02_aero_ai_cuda.md`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NavPredictionOutput {
    /// Viteza estimată față de sol $[v_N, v_E, v_D]$ ($\text{m/s}$).
    pub v_ground_ned: [f64; 3],
    /// Incertitudinea heteroscedastică $\exp(\mathbf{s}_v)$ pentru matricea $\mathbf{R}_{\text{AI}}$ din ES-EKF ($\text{m}^2/\text{s}^2$).
    pub sigma2_v_ground: [f64; 3],
    /// Viteza estimată a masei de ceață/vânt $[w_N, w_E, w_D]$ ($\text{m/s}$).
    pub w_fog_ned: [f64; 3],
    /// Incertitudinea heteroscedastică $\exp(\mathbf{s}_w)$ pentru viteza ceții ($\text{m}^2/\text{s}^2$).
    pub sigma2_w_fog: [f64; 3],
    /// Probabilitatea $[0, 1]$ a unei rafale la $35\text{–}80\text{ m}$ în fața avionului.
    pub gust_lookahead_score: f64,
    /// Eroarea laterală față de axul luminos ALS RWY 09 ($\text{m}$).
    pub als_lateral_offset_m: f64,
    /// Eroarea unghiulară față de axul pistei RWY 09 ($\text{rad}$).
    pub als_heading_error_rad: f64,
}

impl NavPredictionOutput {
    /// Returnează diagonala matricei de covarianță $\mathbf{R}_{\text{AI}}(t) = \text{diag}(\exp(\mathbf{s}_v))$
    /// garantat pozitivă în intervalul $[\exp(-10), \exp(6)] \approx [4.54\times 10^{-5}, 403.43]$.
    #[inline]
    pub fn r_ai_diagonal_covariance(&self) -> [f64; 3] {
        let min_var = (LOG_VAR_MIN as f64).exp();
        let max_var = (LOG_VAR_MAX as f64).exp();
        [
            sanitize_scalar(self.sigma2_v_ground[0], max_var, min_var, max_var),
            sanitize_scalar(self.sigma2_v_ground[1], max_var, min_var, max_var),
            sanitize_scalar(self.sigma2_v_ground[2], max_var, min_var, max_var),
        ]
    }

    /// Construiește direct matricea diagonală $3 \times 3$ `nalgebra::Matrix3<f64>` pentru actualizarea Joseph din ES-EKF.
    #[inline]
    pub fn r_ai_matrix3(&self) -> nalgebra::Matrix3<f64> {
        let diag = self.r_ai_diagonal_covariance();
        nalgebra::Matrix3::from_diagonal(&nalgebra::Vector3::new(diag[0], diag[1], diag[2]))
    }

    /// Returnează deviația standard $\hat{\sigma}_v = \sqrt{\exp(\mathbf{s}_v)}$ pentru telemetria HUD (`heteroscedastic_sigma_v`).
    #[inline]
    pub fn heteroscedastic_sigma_v(&self) -> [f64; 3] {
        let var = self.r_ai_diagonal_covariance();
        [var[0].sqrt(), var[1].sqrt(), var[2].sqrt()]
    }

    /// Fallback ultra-sigur cu incertitudine maximă ($\exp(6.0) \approx 403.43\text{ m}^2/\text{s}^2$),
    /// astfel încât filtrul Kalman să ignore măsurătoarea AI dacă apare vreo anomalie hardware.
    #[inline]
    pub fn safe_fallback_high_uncertainty() -> Self {
        let max_var = (LOG_VAR_MAX as f64).exp();
        Self {
            v_ground_ned: [0.0; 3],
            sigma2_v_ground: [max_var; 3],
            w_fog_ned: [0.0; 3],
            sigma2_w_fog: [max_var; 3],
            gust_lookahead_score: 0.0,
            als_lateral_offset_m: 0.0,
            als_heading_error_rad: 0.0,
        }
    }
}

/// Rețeaua neurală completă `FogOpticalNavNetV2` în `candle-nn`.
#[derive(Debug, Clone)]
pub struct FogOpticalNavNetV2 {
    config: FogNavNetConfig,
    stem: DualBandStem,
    attention_pool: FogSpatialAttentionPool2d,
    physics_proj: Linear,
    temporal_gru: GRU,
    heads: HeteroscedasticMultiTaskHeads,
}

impl FogOpticalNavNetV2 {
    /// Construiește `FogOpticalNavNetV2` din `FogNavNetConfig` și un `VarBuilder`.
    pub fn new(config: FogNavNetConfig, vb: VarBuilder) -> Result<Self, AeroAiError> {
        let stem = DualBandStem::new(config.visual_channels, vb.pp("stem"))?;
        let attention_pool = FogSpatialAttentionPool2d::new(vb.pp("att_pool"))?;

        let fusion_in_dim = 128 + config.kinematic_dim; // 64 (gnd) + 64 (fog) + 18 (kin) = 146
        let physics_proj = linear(
            fusion_in_dim,
            config.gru_hidden_dim,
            vb.pp("physics_proj"),
        )?;

        let temporal_gru = gru(
            config.gru_hidden_dim,
            config.gru_hidden_dim,
            GRUConfig::default(),
            vb.pp("temporal_gru"),
        )?;

        let heads = HeteroscedasticMultiTaskHeads::new(config.gru_hidden_dim, vb.pp("heads"))?;

        Ok(Self {
            config,
            stem,
            attention_pool,
            physics_proj,
            temporal_gru,
            heads,
        })
    }

    /// Returnează configurația rețelei.
    #[inline]
    pub const fn config(&self) -> &FogNavNetConfig {
        &self.config
    }

    /// Creează starea inițială zero `GRUState` pentru un `batch_size` dat.
    pub fn zero_gru_state(&self, batch_size: usize) -> Result<GRUState, AeroAiError> {
        Ok(self.temporal_gru.zero_state(batch_size)?)
    }

    /// Execută pasul înainte (`forward_step`) pe tensorul vizual `[B, C_in, H, W]`,
    /// vectorul cinematic `[B, 18]` și starea opțională `GRUState`.
    pub fn forward_step(
        &self,
        visual_tensor: &Tensor,
        kinematic_tensor: &Tensor,
        gru_state: Option<&GRUState>,
    ) -> Result<FogNavForwardTensors, AeroAiError> {
        let v_dims = visual_tensor.dims();
        if v_dims.len() != 4 || v_dims[1] != self.config.visual_channels {
            return Err(AeroAiError::InvalidTensorShape {
                tensor_name: "visual_tensor",
                expected: vec![
                    v_dims.first().copied().unwrap_or(1),
                    self.config.visual_channels,
                    self.config.image_size,
                    self.config.image_size,
                ],
                actual: v_dims.to_vec(),
            });
        }

        let batch_size = v_dims[0];
        let k_dims = kinematic_tensor.dims();
        if k_dims.len() != 2 || k_dims[0] != batch_size || k_dims[1] != self.config.kinematic_dim {
            return Err(AeroAiError::InvalidTensorShape {
                tensor_name: "kinematic_tensor",
                expected: vec![batch_size, self.config.kinematic_dim],
                actual: k_dims.to_vec(),
            });
        }

        // Extragem canalele fizice w_photon și M_bloom din tensorul vizual
        let c_max = v_dims[1].saturating_sub(1);
        let w_photon_ch = self.config.w_photon_channel_idx.min(c_max);
        let m_bloom_ch = self.config.m_bloom_channel_idx.min(c_max);

        let w_photon_full = visual_tensor.narrow(1, w_photon_ch, 1)?;
        let m_bloom_full = visual_tensor.narrow(1, m_bloom_ch, 1)?;

        // 1. Dual-Branch Feature Extractor
        let (f_gnd_map, f_fog_map) = self.stem.forward(visual_tensor)?;

        // 2. Fog-Aware Spatial Attention & Inverse-Depth / ALS Halo Gating
        let (f_gnd_features, f_fog_features) = self.attention_pool.forward(
            &f_gnd_map,
            &f_fog_map,
            &w_photon_full,
            &m_bloom_full,
        )?;

        // 3. Physics-Guided 6-DoF Concatenation + Linear Projection + Causal GRU
        let fused_input = Tensor::cat(
            &[&f_gnd_features, &f_fog_features, kinematic_tensor],
            1,
        )?;
        let proj_features = self.physics_proj.forward(&fused_input)?.silu()?;

        let owned_zero_state;
        let state_ref = match gru_state {
            Some(st) if st.h().dims().first().copied() == Some(batch_size) => st,
            _ => {
                owned_zero_state = self.temporal_gru.zero_state(batch_size)?;
                &owned_zero_state
            }
        };

        let next_gru_state = self.temporal_gru.step(&proj_features, state_ref)?;

        // 4. Heteroscedastic Multi-Task Heads
        let (
            v_ground_ned,
            s_v_log_var,
            w_fog_ned,
            s_w_log_var,
            gust_logit,
            gust_prob,
            als_offset,
        ) = self.heads.forward(next_gru_state.h())?;

        Ok(FogNavForwardTensors {
            v_ground_ned,
            s_v_log_var,
            w_fog_ned,
            s_w_log_var,
            gust_logit,
            gust_prob,
            als_offset,
            next_gru_state,
            f_gnd_features,
            f_fog_features,
        })
    }
}

/// Extrage în siguranță un rând `batch_idx` dintr-un tensor 2D `[B, D]` ca `Vec<f32>`.
fn extract_row_f32(
    tensor: &Tensor,
    batch_idx: usize,
    expected_cols: usize,
    name: &'static str,
) -> Result<Vec<f32>, AeroAiError> {
    let row = tensor.narrow(0, batch_idx, 1)?.squeeze(0)?;
    let vec = row.to_vec1::<f32>()?;
    if vec.len() != expected_cols {
        return Err(AeroAiError::InvalidTensorShape {
            tensor_name: name,
            expected: vec![1, expected_cols],
            actual: vec![1, vec.len()],
        });
    }
    Ok(vec)
}

#[inline]
fn sanitize_scalar(val: f64, fallback: f64, min_val: f64, max_val: f64) -> f64 {
    if val.is_finite() {
        val.clamp(min_val, max_val)
    } else {
        fallback.clamp(min_val, max_val)
    }
}
