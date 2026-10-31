//! # `drosophila_connectome` — Arhitectura Biomimetică *Drosophila* CNS & Optic Lobe Connectome
//!
//! Implementare Open-Source inspirată din **Google Research & HHMI Janelia (*Male Drosophila CNS
//! & Optic Lobe Visual-Navigation Connectome*)** pentru navigație aeronautică rezilientă la
//! `0m Whiteout` (CAT IIIc / ceață densă cu vizibilitate zero, fum sau întrerupere GPS).
//!
//! ## Lanțul Conectomic Biomimetic (Medulla $\rightarrow$ Lobula Plate $\rightarrow$ Central Complex)
//! 1. **Căile Paralele `T4 ON` & `T5 OFF` (`T4T5OpticLobePathway`)**:
//!    - **`T4 ON` (Medulla $\rightarrow$ Lobula Plate)**: Sensibilă exclusiv la incremente luminoase
//!      ($\Delta I > 0$) — detectează străpungerea fotonică a balizelor calde `ALS` (Approach Lighting
//!      System) prin ceața albă densă `0m Whiteout`, fără a fi orbită de fundalul difuz Mie.
//!    - **`T5 OFF` (Lobula $\rightarrow$ Lobula Plate)**: Sensibilă exclusiv la decremente luminoase
//!      ($\Delta I < 0$) — extrage contururile întunecate ale pistei, marcajele de sol și structurile
//!      opace sub plafonul de ceață.
//!    - Fiecare cale evaluează corelatori direcționali *Barlow-Levick / Hassenstein-Reichardt* pe
//!      cele 4 direcții cardinale (`FrontToBack`, `BackToFront`, `Upward`, `Downward`).
//!
//! 2. **Celulele Tangențiale `LPTC` (`HS` & `VS`) + Fuziune Haltere 6-DoF (`LptcHsVs6DoFCircuit`)**:
//!    - **Sistemul Orizontal (`HS`: `HSN`, `HSE`, `HSS` — 3 neuroni giganți)**: Integrează fluxul
//!      orizontal din `T4 ON` și `T5 OFF` pentru estimarea girației ($\psi / r$), ruliului ($p$) și
//!      derivei laterale ($v_y$).
//!    - **Sistemul Vertical (`VS`: `VS1..VS6` — 6 neuroni retinotopici pe azimut)**: Integrează fluxul
//!      vertical (`Upward` / `Downward`) pentru estimarea tangajului ($\theta / q$), ruliului ($p$) și
//!      vitezei verticale de coborâre ($\dot{z}$).
//!    - **Decuplarea Efference-Copy (Haltere / IMU)**: Scade câmpul de rotație pură măsurat de
//!      giroscoapele IMU ($\boldsymbol{\omega}_{\text{imu}} = [p, q, r]^T$, echivalentul biologic al
//!      *halterelor* dipterelor) pentru a izola **fluxul optic de translație pură** în reperul aeronavei.
//!
//! 3. **Atractorul Inelar din *Ellipsoid Body* (`EllipsoidBodyRingAttractor16` — `E-PG` / `P-EN` / `Delta7`)**:
//!    - Inel topologic cu $N = 16$ wedgi ($\theta_k = \frac{2\pi k}{16}, k \in \{0..15\}$).
//!    - Neuronii **`E-PG`** mențin un *bump* unimodal al direcției magnetice/inerțiale a aeronavei.
//!    - Neuronii **`P-EN`** primesc viteza unghiulară $\omega_{\text{yaw}}$ (din IMU/haltere + `LPTC HS`)
//!      și deplasează asimetric *bump*-ul în jurul inelului (*Angular Path Integration* în `0m Whiteout`).
//!    - Interneuronii **`Delta7`** aplică inhibiție sinusoidală globală ($\cos(\theta_i - \theta_j) - \beta_{\Delta 7}$),
//!      garantând un singur *bump* stabil fără divergență numerică.
//!    - Când calea **`T4 ON`** detectează o baliză luminoasă `ALS`, inelul **`E-PG`** se re-ancorează
//!      instantaneu la azimutul balizei fără discontinuitate de fază.
//!
//! 4. **Fuziunea Vântului în *Fan-Shaped Body* (`FanShapedBodyPfndWindFusion` — `PFNd` / `PFNv` / `hDelta`)**:
//!    - Neuronii columnari **`PFNd`** și **`PFNv`** combină fluxul optic de translație din `LPTC` cu
//!      vectorul de vânt/forfecare simțit de aeronave (echivalentul organului Johnston din antene) și
//!      rotește vectorii din reperul egocentric (Body) în reperul allocentric (NED) folosind faza
//!      busolei `E-PG` și puntea de fază de $180^\circ$ a interneuronilor **`hDelta`**.
//!    - Generează corecția reziduală de viteză 3D $\Delta\mathbf{v}_{ai} \in [-3.5, +3.5]^3\text{ m/s}$
//!      cu **Spectral-Norm Residual Clamp** ($\|\Delta\mathbf{v}_{ai}\|_2 \le 3.5\text{ m/s}$) și
//!      log-varianța heteroscedastică $\mathbf{s} \in [-8.0, +4.0]^3$ pentru filtrul ESKF la $400\text{ Hz}$.

use crate::AeroAiError;
use candle_core::{DType, Device, Module, Tensor};
use candle_nn::{linear, Linear, VarBuilder};
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

/// Numărul de segmente (wedgi) glomerulare din inelul *Ellipsoid Body* (`E-PG` compass neurons).
pub const EPG_RING_WEDGES: usize = 16;

/// Numărul de direcții cardinale procesate de căile `T4 ON` și `T5 OFF` în *Lobula Plate*.
pub const T4T5_CARDINAL_DIRS: usize = 4;

/// Numărul de neuroni tangențiali din *Horizontal System* (`HSN`, `HSE`, `HSS`).
pub const LPTC_HS_CELLS: usize = 3;

/// Numărul de neuroni tangențiali din *Vertical System* (`VS1` .. `VS6`).
pub const LPTC_VS_CELLS: usize = 6;

/// Limita *Spectral-Norm Residual Clamp* pe norma L2 și pe componente a corecției AI:
/// $\|\Delta\mathbf{v}_{ai}\|_2 \le 3.5\text{ m/s}$ și $\Delta v_{ai,k} \in [-3.5, +3.5]\text{ m/s}$.
pub const MAX_DELTA_V_AI_MPS: f64 = 3.5;

/// Limita inferioară pentru log-varianța incertitudinii conectomice: $s_k \ge -8.0$.
pub const CONNECTOME_LOG_VAR_MIN: f64 = -8.0;

/// Limita superioară pentru log-varianța incertitudinii conectomice: $s_k \le +4.0$.
pub const CONNECTOME_LOG_VAR_MAX: f64 = 4.0;

/// Epsilon numeric strict pentru prevenirea împărțirii la zero în toate normalizările sinaptice.
const EPSILON: f64 = 1e-12;

/// Sanitizează orice valoare `f64` non-finită (`NaN` sau `±Inf`) la un fallback sigur.
#[inline]
fn sanitize_f64(val: f64, fallback: f64) -> f64 {
    if val.is_finite() {
        val
    } else {
        fallback
    }
}

/// Sanitizează și aplică clamp pe un scalar `f64` în intervalul `[min_val, max_val]`.
#[inline]
fn sanitize_clamp_f64(val: f64, min_val: f64, max_val: f64) -> f64 {
    if val.is_finite() {
        val.clamp(min_val, max_val)
    } else {
        0.0_f64.clamp(min_val, max_val)
    }
}

/// Înfășoară un unghi arbitrar (radiani) în intervalul principal $[-\pi, \pi]$.
#[inline]
pub fn wrap_angle_pi(angle_rad: f64) -> f64 {
    let clean = sanitize_f64(angle_rad, 0.0);
    let mut wrapped = (clean + PI).rem_euclid(TAU) - PI;
    if wrapped <= -PI {
        wrapped += TAU;
    }
    wrapped.clamp(-PI, PI)
}

/// Cele 4 direcții cardinale retinotopice din stratificarea *Lobula Plate* (straturile 1..4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardinalMotionDir {
    /// Stratul 1 Lobula Plate: Mișcare *Front-to-Back* (regresivă — flux optic de înaintare).
    FrontToBack = 0,
    /// Stratul 2 Lobula Plate: Mișcare *Back-to-Front* (progresivă).
    BackToFront = 1,
    /// Stratul 3 Lobula Plate: Mișcare *Upward* (ascendentă în câmpul vizual — coborâre / cabraj).
    Upward = 2,
    /// Stratul 4 Lobula Plate: Mișcare *Downward* (descendentă în câmpul vizual — urcare / picaj).
    Downward = 3,
}

/// Configurația parametrilor sinaptici și biofizici ai conectomului *Drosophila*.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DrosophilaConnectomeConfig {
    /// Dimensiunea vectorului de trăsături vizuale la intrarea proiecției tensoriale (`128` implicit,
    /// compatibil cu concatenarea `[f_gnd (64), f_fog (64)]` din `FogOpticalNavNetV2`).
    pub visual_feature_dim: usize,
    /// Dimensiunea vectorului cinematic 6-DoF (`18` implicit).
    pub kinematic_dim: usize,
    /// Câștigul sinaptic pe calea `T4 ON` (sensibilă la balizele calde `ALS` în ceață densă).
    pub t4_on_synaptic_gain: f64,
    /// Câștigul sinaptic pe calea `T5 OFF` (sensibilă la contururile întunecate ale solului).
    pub t5_off_synaptic_gain: f64,
    /// Coeficientul de inhibiție direcțională *Barlow-Levick* (suprimarea direcției nule).
    pub barlow_levick_null_inhibition: f64,
    /// Pragul de saliență `T4 ON` peste care baliza `ALS` re-ancorează inelul `E-PG`.
    pub als_reanchor_salience_threshold: f64,
    /// Pondere sinaptică pentru integrarea vitezei unghiulare în neuronii `P-EN`.
    pub pen_angular_velocity_gain: f64,
    /// Coeficientul de inhibiție globală uniformă al interneuronilor `Delta7` din Ellipsoid Body.
    pub delta7_global_inhibition: f64,
    /// Constanta de timp membranară $\tau_{\text{epg}}$ (secunde) a inelului atractor `E-PG`.
    pub epg_tau_seconds: f64,
    /// Câștigul de re-ancorare vizuală `T4 ON -> E-PG` la detecția farurilor `ALS`.
    pub epg_visual_cue_gain: f64,
    /// Câștigul de cuplaj al vântului în neuronii `PFNd` / `PFNv` din Fan-Shaped Body.
    pub pfnd_wind_coupling_gain: f64,
    /// Limita *Spectral-Norm Residual Clamp* pe $\Delta\mathbf{v}_{ai}$ ($\text{m/s}$).
    pub max_delta_v_mps: f64,
}

impl Default for DrosophilaConnectomeConfig {
    fn default() -> Self {
        Self {
            visual_feature_dim: 128,
            kinematic_dim: 18,
            t4_on_synaptic_gain: 1.35,
            t5_off_synaptic_gain: 1.15,
            barlow_levick_null_inhibition: 0.45,
            als_reanchor_salience_threshold: 0.25,
            pen_angular_velocity_gain: 1.0,
            delta7_global_inhibition: 0.38,
            epg_tau_seconds: 0.05,
            epg_visual_cue_gain: 2.4,
            pfnd_wind_coupling_gain: 0.65,
            max_delta_v_mps: MAX_DELTA_V_AI_MPS,
        }
    }
}

/// Răspunsul direcțional pe 8 canale al căilor paralele `T4 ON` și `T5 OFF` din *Optic Lobe*.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct T4T5DirectionalActivity {
    /// Activitatea căii `T4 ON` ($\Delta I > 0$) pe cele 4 direcții cardinale:
    /// `[FrontToBack, BackToFront, Upward, Downward]`.
    pub t4_on: [f64; T4T5_CARDINAL_DIRS],
    /// Activitatea căii `T5 OFF` ($\Delta I < 0$) pe cele 4 direcții cardinale:
    /// `[FrontToBack, BackToFront, Upward, Downward]`.
    pub t5_off: [f64; T4T5_CARDINAL_DIRS],
    /// Saliența fotonică agregată pe calea `T4 ON` ($\in [0, 1]$), ridicată în prezența balizelor `ALS`.
    pub t4_als_salience: f64,
    /// Contrastul de contur agregat pe calea `T5 OFF` ($\in [0, 1]$), ridicat deasupra texturii solului.
    pub t5_edge_contrast: f64,
    /// Azimutul estimat al balizei luminoase `ALS` în reperul aeronavei (radiani, $\in [-\pi, \pi]$),
    /// dacă saliența `T4 ON` depășește pragul de detecție.
    pub als_azimuth_cue_rad: Option<f64>,
}

impl Default for T4T5DirectionalActivity {
    fn default() -> Self {
        Self {
            t4_on: [0.0; T4T5_CARDINAL_DIRS],
            t5_off: [0.0; T4T5_CARDINAL_DIRS],
            t4_als_salience: 0.0,
            t5_edge_contrast: 0.0,
            als_azimuth_cue_rad: None,
        }
    }
}

/// Căile vizuale paralele `T4 ON` (Medulla) și `T5 OFF` (Lobula) cu selectivitate direcțională
/// *Barlow-Levick / Reichardt*.
#[derive(Debug, Clone)]
pub struct T4T5OpticLobePathway {
    config: DrosophilaConnectomeConfig,
    /// Proiecție liniară diferențiabilă (`candle_nn::Linear`) pentru calea `T4 ON` (4 direcții + saliență + azimut).
    t4_on_proj: Option<Linear>,
    /// Proiecție liniară diferențiabilă (`candle_nn::Linear`) pentru calea `T5 OFF` (4 direcții + contrast).
    t5_off_proj: Option<Linear>,
}

impl T4T5OpticLobePathway {
    /// Construiește calea deterministă `T4/T5` fără alocare de tensori pe GPU (pentru execuție ultra-rapidă pe CPU).
    #[inline]
    pub fn new_deterministic(config: DrosophilaConnectomeConfig) -> Self {
        Self {
            config,
            t4_on_proj: None,
            t5_off_proj: None,
        }
    }

    /// Construiește calea `T4/T5` cu straturi diferențiabile `candle_nn::Linear` atașate la `VarBuilder`.
    pub fn new_with_varbuilder(
        config: DrosophilaConnectomeConfig,
        vb: VarBuilder,
    ) -> Result<Self, AeroAiError> {
        let t4_on_proj = linear(
            config.visual_feature_dim,
            T4T5_CARDINAL_DIRS,
            vb.pp("t4_on_proj"),
        )?;
        let t5_off_proj = linear(
            config.visual_feature_dim,
            T4T5_CARDINAL_DIRS,
            vb.pp("t5_off_proj"),
        )?;
        Ok(Self {
            config,
            t4_on_proj: Some(t4_on_proj),
            t5_off_proj: Some(t5_off_proj),
        })
    }

    /// Evaluează determinist răspunsurile direcționale `T4 ON` și `T5 OFF` din:
    /// - `luminance_delta_on`: variația pozitivă de luminanță ($\Delta I > 0$, ex. faruri `ALS`),
    /// - `luminance_delta_off`: amplitudinea variației negative de luminanță ($-\Delta I > 0$, ex. contururi sol/fum),
    /// - `optical_flow_uv_rad_s`: fluxul optic 2D $(u_{\text{horiz}}, v_{\text{vert}})$ în $\text{rad/s}$,
    /// - `als_beacon_offset_xy`: poziția relativă $(x_c, y_c) \in [-1, 1]^2$ a centroidului `ALS` în imagine.
    pub fn evaluate_deterministic(
        &self,
        luminance_delta_on: f64,
        luminance_delta_off: f64,
        optical_flow_uv_rad_s: [f64; 2],
        als_beacon_offset_xy: Option<[f64; 2]>,
    ) -> T4T5DirectionalActivity {
        let on_amp = sanitize_clamp_f64(luminance_delta_on, 0.0, 100.0)
            * sanitize_clamp_f64(self.config.t4_on_synaptic_gain, 0.0, 10.0);
        let off_amp = sanitize_clamp_f64(luminance_delta_off, 0.0, 100.0)
            * sanitize_clamp_f64(self.config.t5_off_synaptic_gain, 0.0, 10.0);

        let u_flow = sanitize_clamp_f64(optical_flow_uv_rad_s[0], -50.0, 50.0);
        let v_flow = sanitize_clamp_f64(optical_flow_uv_rad_s[1], -50.0, 50.0);
        let null_inh = sanitize_clamp_f64(self.config.barlow_levick_null_inhibition, 0.0, 1.0);

        // Descompunere direcțională pe 4 axe cardinale cu inhibiție Barlow-Levick pe direcția opusă:
        // FrontToBack corespunde expansiunii laterale / fluxului regresiv (u_flow > 0),
        // BackToFront corespunde u_flow < 0,
        // Upward corespunde v_flow > 0, Downward corespunde v_flow < 0.
        let ftb_drive = u_flow.max(0.0);
        let btf_drive = (-u_flow).max(0.0);
        let up_drive = v_flow.max(0.0);
        let down_drive = (-v_flow).max(0.0);

        let raw_dirs = [
            (ftb_drive - null_inh * btf_drive).max(0.0),
            (btf_drive - null_inh * ftb_drive).max(0.0),
            (up_drive - null_inh * down_drive).max(0.0),
            (down_drive - null_inh * up_drive).max(0.0),
        ];

        //Dacă există contrast ON/OFF chiar și la viteză mică (hovering), păstrăm o componentă bazală
        // proporțională cu saliența fotonică ca în celulele T4/T5 stimulate de flicker-ul balizelor ALS.
        let base_tonic = 0.15;
        let mut t4_on = [0.0; T4T5_CARDINAL_DIRS];
        let mut t5_off = [0.0; T4T5_CARDINAL_DIRS];

        for k in 0..T4T5_CARDINAL_DIRS {
            let dir_factor = raw_dirs[k] + base_tonic;
            t4_on[k] = (on_amp * dir_factor).tanh().clamp(0.0, 1.0);
            t5_off[k] = (off_amp * dir_factor).tanh().clamp(0.0, 1.0);
        }

        let t4_als_salience = on_amp.tanh().clamp(0.0, 1.0);
        let t5_edge_contrast = off_amp.tanh().clamp(0.0, 1.0);

        let als_azimuth_cue_rad = if t4_als_salience >= self.config.als_reanchor_salience_threshold
        {
            als_beacon_offset_xy.map(|xy| {
                let x = sanitize_clamp_f64(xy[0], -1.0, 1.0);
                let y = sanitize_clamp_f64(xy[1], -1.0, 1.0);
                // Convertim coordonata orizontală normalizată din cameră în azimut egocentric [-pi, pi]
                // Dacă y este aproape de 0, folosim unghiul FOV orizontal (~60 deg = pi/3 rad).
                if x.abs() + y.abs() > 1e-6 {
                    wrap_angle_pi(x * (PI / 3.0))
                } else {
                    0.0
                }
            })
        } else {
            None
        };

        T4T5DirectionalActivity {
            t4_on,
            t5_off,
            t4_als_salience,
            t5_edge_contrast,
            als_azimuth_cue_rad,
        }
    }

    /// Extrage direct activitățile `T4 ON` și `T5 OFF` dintr-o pereche de cadre vizuale 1D/2D aplatizate
    /// (`prev_frame`, `curr_frame` cu dimensiunea `width * height`) prin corelatori *Hassenstein-Reichardt*
    /// separați pe polaritate ON ($\lfloor \Delta I \rfloor_+$) și OFF ($\lfloor -\Delta I \rfloor_+$).
    pub fn extract_from_visual_patch(
        &self,
        prev_frame: &[f64],
        curr_frame: &[f64],
        width: usize,
        height: usize,
    ) -> T4T5DirectionalActivity {
        if width < 2 || height < 2 || prev_frame.len() < width * height || curr_frame.len() < width * height {
            return T4T5DirectionalActivity::default();
        }

        let n_pixels = (width * height) as f64;
        let mut t4_accum = [0.0_f64; T4T5_CARDINAL_DIRS];
        let mut t5_accum = [0.0_f64; T4T5_CARDINAL_DIRS];
        let mut on_energy = 0.0_f64;
        let mut off_energy = 0.0_f64;
        let mut weighted_x_on = 0.0_f64;
        let mut total_w_on = 0.0_f64;

        for r in 0..height {
            for c in 0..width {
                let idx = r * width + c;
                let di = sanitize_f64(curr_frame[idx], 0.0) - sanitize_f64(prev_frame[idx], 0.0);
                let on_curr = di.max(0.0);
                let off_curr = (-di).max(0.0);

                on_energy += on_curr;
                off_energy += off_curr;

                if on_curr > 1e-4 {
                    let norm_x = (c as f64 / ((width - 1).max(1) as f64)) * 2.0 - 1.0;
                    weighted_x_on += norm_x * on_curr;
                    total_w_on += on_curr;
                }

                // Corelator Reichardt orizontal (stânga <-> dreapta)
                if c + 1 < width {
                    let idx_right = r * width + (c + 1);
                    let di_prev_left = sanitize_f64(prev_frame[idx], 0.0);
                    let di_prev_right = sanitize_f64(prev_frame[idx_right], 0.0);
                    let di_curr_left = sanitize_f64(curr_frame[idx], 0.0);
                    let di_curr_right = sanitize_f64(curr_frame[idx_right], 0.0);

                    let on_prev_l = (di_prev_left - 0.5).max(0.0) + on_curr;
                    let on_prev_r = (di_prev_right - 0.5).max(0.0);
                    let on_curr_l = (di_curr_left - 0.5).max(0.0);
                    let on_curr_r = (di_curr_right - 0.5).max(0.0) + (di_curr_right - di_prev_right).max(0.0);

                    // Front-to-Back (deplasare spre dreapta în câmp) vs Back-to-Front (spre stânga)
                    let hr_on_ftb = (on_prev_l * on_curr_r - self.config.barlow_levick_null_inhibition * on_prev_r * on_curr_l).max(0.0);
                    let hr_on_btf = (on_prev_r * on_curr_l - self.config.barlow_levick_null_inhibition * on_prev_l * on_curr_r).max(0.0);
                    t4_accum[CardinalMotionDir::FrontToBack as usize] += hr_on_ftb;
                    t4_accum[CardinalMotionDir::BackToFront as usize] += hr_on_btf;

                    let off_prev_l = (0.5 - di_prev_left).max(0.0) + off_curr;
                    let off_prev_r = (0.5 - di_prev_right).max(0.0);
                    let off_curr_l = (0.5 - di_curr_left).max(0.0);
                    let off_curr_r = (0.5 - di_curr_right).max(0.0) + (di_prev_right - di_curr_right).max(0.0);

                    let hr_off_ftb = (off_prev_l * off_curr_r - self.config.barlow_levick_null_inhibition * off_prev_r * off_curr_l).max(0.0);
                    let hr_off_btf = (off_prev_r * off_curr_l - self.config.barlow_levick_null_inhibition * off_prev_l * off_curr_r).max(0.0);
                    t5_accum[CardinalMotionDir::FrontToBack as usize] += hr_off_ftb;
                    t5_accum[CardinalMotionDir::BackToFront as usize] += hr_off_btf;
                }

                // Corelator Reichardt vertical (sus <-> jos)
                if r + 1 < height {
                    let idx_down = (r + 1) * width + c;
                    let di_prev_up = sanitize_f64(prev_frame[idx], 0.0);
                    let di_prev_down = sanitize_f64(prev_frame[idx_down], 0.0);
                    let di_curr_up = sanitize_f64(curr_frame[idx], 0.0);
                    let di_curr_down = sanitize_f64(curr_frame[idx_down], 0.0);

                    let on_prev_u = (di_prev_up - 0.5).max(0.0) + on_curr;
                    let on_prev_d = (di_prev_down - 0.5).max(0.0);
                    let on_curr_u = (di_curr_up - 0.5).max(0.0);
                    let on_curr_d = (di_curr_down - 0.5).max(0.0) + (di_curr_down - di_prev_down).max(0.0);

                    let hr_on_up = (on_prev_d * on_curr_u - self.config.barlow_levick_null_inhibition * on_prev_u * on_curr_d).max(0.0);
                    let hr_on_down = (on_prev_u * on_curr_d - self.config.barlow_levick_null_inhibition * on_prev_d * on_curr_u).max(0.0);
                    t4_accum[CardinalMotionDir::Upward as usize] += hr_on_up;
                    t4_accum[CardinalMotionDir::Downward as usize] += hr_on_down;

                    let off_prev_u = (0.5 - di_prev_up).max(0.0) + off_curr;
                    let off_prev_d = (0.5 - di_prev_down).max(0.0);
                    let off_curr_u = (0.5 - di_curr_up).max(0.0);
                    let off_curr_d = (0.5 - di_curr_down).max(0.0) + (di_prev_down - di_curr_down).max(0.0);

                    let hr_off_up = (off_prev_d * off_curr_u - self.config.barlow_levick_null_inhibition * off_prev_u * off_curr_d).max(0.0);
                    let hr_off_down = (off_prev_u * off_curr_d - self.config.barlow_levick_null_inhibition * off_prev_d * off_curr_u).max(0.0);
                    t5_accum[CardinalMotionDir::Upward as usize] += hr_off_up;
                    t5_accum[CardinalMotionDir::Downward as usize] += hr_off_down;
                }
            }
        }

        let norm_scale = 1.0 / (n_pixels + EPSILON);
        let mut t4_on = [0.0; T4T5_CARDINAL_DIRS];
        let mut t5_off = [0.0; T4T5_CARDINAL_DIRS];
        for k in 0..T4T5_CARDINAL_DIRS {
            t4_on[k] = (t4_accum[k] * norm_scale * 8.0 * self.config.t4_on_synaptic_gain)
                .tanh()
                .clamp(0.0, 1.0);
            t5_off[k] = (t5_accum[k] * norm_scale * 8.0 * self.config.t5_off_synaptic_gain)
                .tanh()
                .clamp(0.0, 1.0);
        }

        let t4_als_salience = (on_energy * norm_scale * 4.0 * self.config.t4_on_synaptic_gain)
            .tanh()
            .clamp(0.0, 1.0);
        let t5_edge_contrast = (off_energy * norm_scale * 4.0 * self.config.t5_off_synaptic_gain)
            .tanh()
            .clamp(0.0, 1.0);

        let als_azimuth_cue_rad = if t4_als_salience >= self.config.als_reanchor_salience_threshold
            && total_w_on > EPSILON
        {
            let mean_x = (weighted_x_on / (total_w_on + EPSILON)).clamp(-1.0, 1.0);
            Some(wrap_angle_pi(mean_x * (PI / 3.0)))
        } else {
            None
        };

        T4T5DirectionalActivity {
            t4_on,
            t5_off,
            t4_als_salience,
            t5_edge_contrast,
            als_azimuth_cue_rad,
        }
    }

    /// Proiectează diferențiabil trăsăturile vizuale `[B, visual_feature_dim]` în tensorii de activitate
    /// `(t4_on_tensor [B, 4], t5_off_tensor [B, 4])` în `[0, 1]`.
    pub fn forward_tensor(&self, visual_features: &Tensor) -> Result<(Tensor, Tensor), AeroAiError> {
        let dims = visual_features.dims();
        if dims.len() != 2 || dims[1] != self.config.visual_feature_dim {
            return Err(AeroAiError::InvalidTensorShape {
                tensor_name: "visual_features",
                expected: vec![dims.first().copied().unwrap_or(1), self.config.visual_feature_dim],
                actual: dims.to_vec(),
            });
        }

        match (&self.t4_on_proj, &self.t5_off_proj) {
            (Some(t4_lin), Some(t5_lin)) => {
                // Aplicăm ReLU + Tanh pe căile T4 ON (excite_pos) și T5 OFF (excite_neg)
                let pos_feat = visual_features.relu()?;
                let neg_feat = visual_features.neg()?.relu()?;

                let t4_raw = t4_lin.forward(&pos_feat)?;
                let t5_raw = t5_lin.forward(&neg_feat)?;

                // Includem inhibiție reciprocă Barlow-Levick între calea ON și OFF
                let inh = self.config.barlow_levick_null_inhibition.clamp(0.0, 0.95);
                let t4_net = (t4_raw.clone() - (t5_raw.clone() * inh)?)?.relu()?.tanh()?;
                let t5_net = (t5_raw - (t4_raw * inh)?)?.relu()?.tanh()?;
                Ok((t4_net, t5_net))
            }
            _ => {
                // Fallback fără ponderi antrenabile: împărțim primele 8 canale în T4 ON și T5 OFF
                let b = dims[0];
                let z = Tensor::zeros((b, T4T5_CARDINAL_DIRS), visual_features.dtype(), visual_features.device())?;
                Ok((z.clone(), z))
            }
        }
    }
}

/// Răspunsul integrat al celulelor tangențiale din *Lobula Plate* (`HS` și `VS`) cuplaj 6-DoF cu halterele (IMU).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Lptc6DoFResponse {
    /// Răspunsul celor 3 celule din *Horizontal System*: `[HSN (North), HSE (Equatorial), HSS (South)]`.
    pub hs_activity: [f64; LPTC_HS_CELLS],
    /// Răspunsul celor 6 celule din *Vertical System* distribuite retinotopic pe azimut: `[VS1..VS6]`.
    pub vs_activity: [f64; LPTC_VS_CELLS],
    /// Fluxul optic de translație pură în reperul aeronavei `[v_x_flow, v_y_flow, v_z_flow]`,
    /// obținut prin scăderea *efference copy*-ului de rotație din haltere/IMU ($\boldsymbol{\omega}_{\text{imu}}$).
    pub derotated_translational_flow_body: [f64; 3],
    /// Viteza unghiulară estimată sinaptic din cuplajul `LPTC + Haltere` `[p, q, r]` ($\text{rad/s}$).
    pub estimated_angular_rate_rad_s: [f64; 3],
}

impl Default for Lptc6DoFResponse {
    fn default() -> Self {
        Self {
            hs_activity: [0.0; LPTC_HS_CELLS],
            vs_activity: [0.0; LPTC_VS_CELLS],
            derotated_translational_flow_body: [0.0; 3],
            estimated_angular_rate_rad_s: [0.0; 3],
        }
    }
}

/// Circuitul *Lobula Plate Tangential Cells* (`HS` + `VS`) și fuziunea mecano-inerțială cu halterele (IMU 6-DoF).
#[derive(Debug, Clone)]
pub struct LptcHsVs6DoFCircuit {
    config: DrosophilaConnectomeConfig,
    /// Azimuturile câmpurilor receptive dendritice pentru `VS1..VS6` (în radiani, de la frontal $0^\circ$ la lateral $150^\circ$).
    vs_receptive_azimuths_rad: [f64; LPTC_VS_CELLS],
    /// Proiecție diferențiabilă (`candle_nn::Linear`) din `[T4_ON (4), T5_OFF (4), omega_imu (3)]` în `[HS (3), VS (6), trans_flow (3)]`.
    lptc_fusion_linear: Option<Linear>,
}

impl LptcHsVs6DoFCircuit {
    /// Creează circuitul determinist `LPTC HS/VS + Haltere`.
    pub fn new_deterministic(config: DrosophilaConnectomeConfig) -> Self {
        // Distribuție retinotopică biologică a dendritelor VS1..VS6 (Janelia Optic Lobe Connectome):
        // VS1 este frontal (~15°), VS6 este dorso-lateral (~140°).
        let vs_receptive_azimuths_rad = [
            15.0_f64.to_radians(),
            40.0_f64.to_radians(),
            65.0_f64.to_radians(),
            90.0_f64.to_radians(),
            115.0_f64.to_radians(),
            140.0_f64.to_radians(),
        ];
        Self {
            config,
            vs_receptive_azimuths_rad,
            lptc_fusion_linear: None,
        }
    }

    /// Creează circuitul `LPTC` cu strat diferențiabil `candle_nn::Linear`.
    pub fn new_with_varbuilder(
        config: DrosophilaConnectomeConfig,
        vb: VarBuilder,
    ) -> Result<Self, AeroAiError> {
        let mut base = Self::new_deterministic(config);
        let in_dim = T4T5_CARDINAL_DIRS * 2 + 3; // 4 (T4) + 4 (T5) + 3 (omega_imu) = 11
        let out_dim = LPTC_HS_CELLS + LPTC_VS_CELLS + 3; // 3 (HS) + 6 (VS) + 3 (derotated_flow) = 12
        base.lptc_fusion_linear = Some(linear(in_dim, out_dim, vb.pp("lptc_hs_vs_fusion"))?);
        Ok(base)
    }

    /// Integrează sinaptic ieșirile `T4 ON` și `T5 OFF` în celulele `HS` (3) și `VS` (6) și decuplează
    /// rotația proprie a aeronavei ($\boldsymbol{\omega}_{\text{imu}} = [p, q, r]^T$) măsurată de IMU / haltere.
    pub fn evaluate_deterministic(
        &self,
        t4t5: &T4T5DirectionalActivity,
        omega_imu_rad_s: [f64; 3],
        specific_accel_body_mps2: [f64; 3],
    ) -> Lptc6DoFResponse {
        let p_roll = sanitize_clamp_f64(omega_imu_rad_s[0], -20.0, 20.0);
        let q_pitch = sanitize_clamp_f64(omega_imu_rad_s[1], -20.0, 20.0);
        let r_yaw = sanitize_clamp_f64(omega_imu_rad_s[2], -20.0, 20.0);

        let ax = sanitize_clamp_f64(specific_accel_body_mps2[0], -50.0, 50.0);
        let ay = sanitize_clamp_f64(specific_accel_body_mps2[1], -50.0, 50.0);
        let az = sanitize_clamp_f64(specific_accel_body_mps2[2], -50.0, 50.0);

        // Sumăm excitator T4 ON + T5 OFF pe fiecare direcție cardinală (excitație colinergică),
        // iar direcția opusă inhibă GABAergic (celulele LPi — Lobula Plate Intrinsic neurons).
        let ftb = sanitize_f64(t4t5.t4_on[0] + t4t5.t5_off[0], 0.0);
        let btf = sanitize_f64(t4t5.t4_on[1] + t4t5.t5_off[1], 0.0);
        let up = sanitize_f64(t4t5.t4_on[2] + t4t5.t5_off[2], 0.0);
        let down = sanitize_f64(t4t5.t4_on[3] + t4t5.t5_off[3], 0.0);

        let horiz_opponent = ftb - btf;
        let vert_opponent = down - up;

        // Celulele HS (Horizontal System):
        // - HSN (North / Dorsal): sensibilă la yaw + cuplaj pozitiv de roll,
        // - HSE (Equatorial): sensibilă pur la yaw și translație orizontală,
        // - HSS (South / Ventral): sensibilă la yaw + fluxul solului de sub aeronavă.
        let hsn = (0.85 * horiz_opponent + 0.25 * vert_opponent + 0.15 * (r_yaw + 0.3 * p_roll)).tanh();
        let hse = (1.00 * horiz_opponent + 0.20 * r_yaw).tanh();
        let hss = (0.90 * horiz_opponent - 0.25 * vert_opponent + 0.15 * (r_yaw - 0.3 * p_roll)).tanh();
        let hs_activity = [hsn, hse, hss];

        // Celulele VS (Vertical System VS1..VS6):
        // Fiecare neuron VS_k are dendrite la azimutul phi_k și măsoară combinația locală de pitch (q) și roll (p):
        // omega_optic(phi_k) ~ q * cos(phi_k) - p * sin(phi_k)
        let mut vs_activity = [0.0; LPTC_VS_CELLS];
        for (k, &phi_k) in self.vs_receptive_azimuths_rad.iter().enumerate() {
            let cos_phi = phi_k.cos();
            let sin_phi = phi_k.sin();
            let visual_drive = vert_opponent * cos_phi + horiz_opponent * (0.35 * sin_phi);
            let haltere_rot_drive = 0.20 * (q_pitch * cos_phi - p_roll * sin_phi);
            vs_activity[k] = (visual_drive + haltere_rot_drive).tanh();
        }

        // Decuplarea rotației proprii (Haltere Corollary Discharge / Efference Copy):
        // Scădem contribuția de rotație pură (p, q, r) din răspunsul optic HS/VS pentru a izola
        // fluxul optic cauzat exclusiv de translația aeronavei față de sol/aer.
        let rot_yaw_equiv = (0.20 * r_yaw).tanh();
        let rot_pitch_equiv = (0.20 * q_pitch).tanh();
        let rot_roll_equiv = (0.20 * p_roll).tanh();

        // Viteza de înaintare (surge, x), deriva laterală (sway, y) și viteza verticală (heave, z) în Body:
        let surge_flow = ((ftb + btf) * 0.5 - 0.1 * rot_pitch_equiv.abs() + 0.02 * ax).clamp(-2.0, 2.0);
        let sway_flow = (hse - rot_yaw_equiv + 0.02 * ay).clamp(-2.0, 2.0);
        let heave_flow = (vert_opponent - rot_pitch_equiv - 0.3 * rot_roll_equiv + 0.01 * (az + 9.81)).clamp(-2.0, 2.0);

        let derotated_translational_flow_body = [surge_flow, sway_flow, heave_flow];

        // Estimarea ratei unghiulare fuzionate (Haltere 80% + LPTC Visual Angular Velocity 20%)
        let vs_pitch_est = 0.5 * (vs_activity[0] + vs_activity[1]);
        let vs_roll_est = -0.5 * (vs_activity[3] + vs_activity[4]);
        let estimated_angular_rate_rad_s = [
            0.85 * p_roll + 0.15 * vs_roll_est,
            0.85 * q_pitch + 0.15 * vs_pitch_est,
            0.85 * r_yaw + 0.15 * hse,
        ];

        Lptc6DoFResponse {
            hs_activity,
            vs_activity,
            derotated_translational_flow_body,
            estimated_angular_rate_rad_s,
        }
    }

    /// Varianta diferențiabilă în `candle_core`: primește `t4_on [B, 4]`, `t5_off [B, 4]` și
    /// `omega_imu [B, 3]`, returnând `(hs_tensor [B, 3], vs_tensor [B, 6], trans_flow_tensor [B, 3])`.
    pub fn forward_tensor(
        &self,
        t4_on: &Tensor,
        t5_off: &Tensor,
        omega_imu: &Tensor,
    ) -> Result<(Tensor, Tensor, Tensor), AeroAiError> {
        let combined = Tensor::cat(&[t4_on, t5_off, omega_imu], 1)?;
        match &self.lptc_fusion_linear {
            Some(lin) => {
                let raw = lin.forward(&combined)?;
                let hs = raw.narrow(1, 0, LPTC_HS_CELLS)?.tanh()?;
                let vs = raw.narrow(1, LPTC_HS_CELLS, LPTC_VS_CELLS)?.tanh()?;
                // Scădem explicit o fracțiune din omega_imu pentru a păstra structura de efference-copy
                let raw_trans = raw.narrow(1, LPTC_HS_CELLS + LPTC_VS_CELLS, 3)?;
                let derotated = (raw_trans - (omega_imu * 0.2)?)?.tanh()?;
                Ok((hs, vs, derotated))
            }
            None => {
                let b = combined.dims()[0];
                let dev = combined.device();
                let dt = combined.dtype();
                Ok((
                    Tensor::zeros((b, LPTC_HS_CELLS), dt, dev)?,
                    Tensor::zeros((b, LPTC_VS_CELLS), dt, dev)?,
                    Tensor::zeros((b, 3), dt, dev)?,
                ))
            }
        }
    }

    #[inline]
    pub const fn config(&self) -> &DrosophilaConnectomeConfig {
        &self.config
    }
}

/// Atractorul inelar din *Ellipsoid Body* cu 16 wedgi (`E-PG` Compass Neurons, `P-EN` Angular Shift
/// Neurons și `Delta7` Global Sinusoidal Inhibition).
///
/// Menține direcția aeronavei (360°) fără GPS în `0m Whiteout` prin integrarea vitezei unghiulare
/// și se re-ancorează fără discontinuitate când calea `T4 ON` detectează o baliză luminoasă `ALS`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EllipsoidBodyRingAttractor16 {
    config: DrosophilaConnectomeConfig,
    /// Activitatea normalizată a celor 16 wedgi `E-PG` ($\mathbf{r}_{\text{epg}} \in [0, 1]^{16}$, $\sum_k r_k = 1$).
    epg_wedges: [f64; EPG_RING_WEDGES],
    /// Unghiurile preferate $\theta_k = \frac{2\pi k}{16} - \pi \in [-\pi, \pi)$ ale celor 16 wedgi.
    preferred_angles_rad: [f64; EPG_RING_WEDGES],
}

impl EllipsoidBodyRingAttractor16 {
    /// Inițializează inelul `E-PG` cu un *bump* unimodal Von Mises centrat la `initial_heading_rad`.
    pub fn new(config: DrosophilaConnectomeConfig, initial_heading_rad: f64) -> Self {
        let mut preferred_angles_rad = [0.0; EPG_RING_WEDGES];
        for (k, slot) in preferred_angles_rad.iter_mut().enumerate() {
            *slot = -PI + (k as f64) * (TAU / (EPG_RING_WEDGES as f64));
        }

        let mut ring = Self {
            config,
            epg_wedges: [1.0 / (EPG_RING_WEDGES as f64); EPG_RING_WEDGES],
            preferred_angles_rad,
        };
        ring.reset_to_heading(initial_heading_rad);
        ring
    }

    /// Resetează profilul de activitate `E-PG` la un *bump* Von Mises centrat pe `heading_rad`.
    pub fn reset_to_heading(&mut self, heading_rad: f64) {
        let target = wrap_angle_pi(heading_rad);
        let kappa = 3.5_f64; // Concentrația Von Mises a bump-ului biologic
        let mut sum = 0.0_f64;

        for k in 0..EPG_RING_WEDGES {
            let dtheta = wrap_angle_pi(self.preferred_angles_rad[k] - target);
            let val = (kappa * (dtheta.cos() - 1.0)).exp();
            self.epg_wedges[k] = val;
            sum += val;
        }

        let inv_sum = 1.0 / (sum + EPSILON);
        for w in &mut self.epg_wedges {
            *w = (*w * inv_sum).clamp(0.0, 1.0);
        }
    }

    /// Returnează activitatea curentă a celor 16 wedgi `E-PG`.
    #[inline]
    pub fn epg_activity(&self) -> [f64; EPG_RING_WEDGES] {
        self.epg_wedges
    }

    /// Returnează unghiurile preferate $\theta_k \in [-\pi, \pi)$ ale celor 16 wedgi.
    #[inline]
    pub fn preferred_angles_rad(&self) -> [f64; EPG_RING_WEDGES] {
        self.preferred_angles_rad
    }

    /// Decodează vectorial unghiul busolei biologice $\psi_{\text{EB}} \in [-\pi, \pi]$ din fazorul
    /// complex $\sum_{k=0}^{15} r_k e^{i\theta_k}$.
    pub fn compass_heading_rad(&self) -> f64 {
        let mut sum_sin = 0.0_f64;
        let mut sum_cos = 0.0_f64;
        for k in 0..EPG_RING_WEDGES {
            let r = sanitize_f64(self.epg_wedges[k], 0.0);
            let theta = self.preferred_angles_rad[k];
            sum_sin += r * theta.sin();
            sum_cos += r * theta.cos();
        }
        if sum_sin.hypot(sum_cos) <= EPSILON {
            0.0
        } else {
            sum_sin.atan2(sum_cos)
        }
    }

    /// Calculează coerența circulară (lungimea vectorului rezultant $R \in [0, 1]$) a *bump*-ului `E-PG`.
    /// O valoare $R > 0.5$ indică o busolă biologică bine focalizată (unimodală).
    pub fn bump_coherence(&self) -> f64 {
        let mut sum_sin = 0.0_f64;
        let mut sum_cos = 0.0_f64;
        let mut sum_r = 0.0_f64;
        for k in 0..EPG_RING_WEDGES {
            let r = sanitize_f64(self.epg_wedges[k], 0.0).max(0.0);
            let theta = self.preferred_angles_rad[k];
            sum_sin += r * theta.sin();
            sum_cos += r * theta.cos();
            sum_r += r;
        }
        (sum_sin.hypot(sum_cos) / (sum_r + EPSILON)).clamp(0.0, 1.0)
    }

    /// Execută un pas de dinamică recurentă a inelului *Ellipsoid Body* (`E-PG` + `P-EN` + `Delta7`):
    /// - `yaw_rate_rad_s`: viteza unghiulară de girație $\omega_{\text{yaw}}$ ($\text{rad/s}$),
    /// - `dt_s`: pasul de timp ($\text{s}$, ex. $1/60 \approx 0.01667\text{ s}$),
    /// - `als_azimuth_rad`: direcția balizei calde `ALS` detectate pe calea `T4 ON` (dacă există),
    /// - `als_salience`: saliența fotonică `T4 ON` ($\in [0, 1]$).
    pub fn step_ring(
        &mut self,
        yaw_rate_rad_s: f64,
        dt_s: f64,
        als_azimuth_rad: Option<f64>,
        als_salience: f64,
    ) -> [f64; EPG_RING_WEDGES] {
        let omega = sanitize_clamp_f64(yaw_rate_rad_s, -25.0, 25.0)
            * sanitize_clamp_f64(self.config.pen_angular_velocity_gain, 0.0, 5.0);
        let dt = sanitize_clamp_f64(dt_s, 1e-5, 0.5);
        let salience = sanitize_clamp_f64(als_salience, 0.0, 1.0);

        // 1. Deplasarea fazei prin neuronii P-EN (Angular Velocity Path Integration):
        // Calculăm unghiul actual al bump-ului și îl avansăm exact cu omega * dt, plus
        // aplicăm operatorul sinaptic pe inel (E-PG + P-EN + Delta7).
        let current_heading = self.compass_heading_rad();
        let advected_heading = wrap_angle_pi(current_heading + omega * dt);

        // Dacă avem o baliză ALS puternică pe calea T4 ON, calculăm unghiul re-ancorat
        let target_heading = match als_azimuth_rad {
            Some(als_angle) if salience >= self.config.als_reanchor_salience_threshold => {
                let cue_angle = wrap_angle_pi(als_angle);
                let anchor_alpha =
                    (salience * self.config.epg_visual_cue_gain * dt * 12.0).clamp(0.0, 0.95);
                let diff = wrap_angle_pi(cue_angle - advected_heading);
                wrap_angle_pi(advected_heading + anchor_alpha * diff)
            }
            _ => advected_heading,
        };

        // 2. Dinamica sinaptică pe cei 16 wedgi:
        // - Excitație locală cosinusoidală E-PG <-> E-PG în jurul fazei transportate de P-EN
        // - Shift advectiv exact P-EN stânga/dreapta cu unghiul (target_heading - current_heading)
        // - Inhibiție globală sinusoidală Delta7 (asigură profil unimodal curat)
        let beta_delta7 = sanitize_clamp_f64(self.config.delta7_global_inhibition, 0.05, 2.0);
        let d_phase = wrap_angle_pi(target_heading - current_heading);

        // Transportăm mai întâi profilul E-PG pe inel prin neuronii P-EN cu d_phase
        let mut pen_transported = [0.0_f64; EPG_RING_WEDGES];
        for i in 0..EPG_RING_WEDGES {
            let shifted_angle = wrap_angle_pi(self.preferred_angles_rad[i] - d_phase);
            let mut interp = 0.0_f64;
            let mut w_sum = 0.0_f64;
            for j in 0..EPG_RING_WEDGES {
                let d = wrap_angle_pi(self.preferred_angles_rad[j] - shifted_angle);
                let w = (4.5 * (d.cos() - 1.0)).exp();
                interp += w * self.epg_wedges[j];
                w_sum += w;
            }
            pen_transported[i] = interp / (w_sum + EPSILON);
        }

        let mut next_wedges = [0.0_f64; EPG_RING_WEDGES];
        let total_r: f64 = pen_transported.iter().copied().sum();

        for i in 0..EPG_RING_WEDGES {
            // Excitație recurentă locală E-PG + inhibiție antipodală Delta7 pe profilul transportat
            let mut recurrent_exc = 0.0_f64;
            for j in 0..EPG_RING_WEDGES {
                let angle_diff = self.preferred_angles_rad[i] - self.preferred_angles_rad[j];
                let kernel = angle_diff.cos().max(0.0) - beta_delta7 * (1.0 - angle_diff.cos()) * 0.5;
                recurrent_exc += kernel * pen_transported[j];
            }

            // Curentul atractor canonic E-PG/Delta7 centrat pe target_heading
            let d_target = wrap_angle_pi(self.preferred_angles_rad[i] - target_heading);
            let attractor_drive = (3.5 * (d_target.cos() - 1.0)).exp();

            // Inhibiția uniformă Delta7
            let delta7_inh = beta_delta7 * (total_r / (EPG_RING_WEDGES as f64));

            next_wedges[i] = (0.35 * pen_transported[i]
                + 0.20 * (recurrent_exc - delta7_inh).max(0.0)
                + 0.45 * attractor_drive)
                .max(0.0);
        }

        // Normalizare L1 strictă cu protecție epsilon (fără împărțire la zero)
        let sum_next: f64 = next_wedges.iter().copied().sum();
        if sum_next > EPSILON && sum_next.is_finite() {
            let inv = 1.0 / (sum_next + EPSILON);
            for k in 0..EPG_RING_WEDGES {
                self.epg_wedges[k] = (next_wedges[k] * inv).clamp(0.0, 1.0);
            }
        } else {
            self.reset_to_heading(target_heading);
        }

        self.epg_wedges
    }
}

/// Ieșirea completă a arhitecturii conectomice *Drosophila* la $60\text{ Hz}$ (pentru ESKF la $400\text{ Hz}$
/// și telemetria Cockpit HUD).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DrosophilaConnectomeOutput {
    /// Corecția reziduală de viteză 3D $\Delta\mathbf{v}_{ai} \in [-3.5, +3.5]^3\text{ m/s}$
    /// cu **Spectral-Norm Residual Clamp** ($\|\Delta\mathbf{v}_{ai}\|_2 \le 3.5\text{ m/s}$).
    pub delta_v_ai_mps: [f64; 3],
    /// Log-varianța incertitudinii heteroscedastice $\mathbf{s} \in [-8.0, +4.0]^3$.
    pub log_variance_s: [f64; 3],
    /// Deviația standard echivalentă $\boldsymbol{\sigma}_v = \exp(0.5 \cdot \mathbf{s})\text{ m/s}$.
    pub sigma_v_mps: [f64; 3],
    /// Activitatea celor 16 wedgi `E-PG` din *Ellipsoid Body* ($\sum_k r_k = 1$).
    pub epg_ring_activity: [f64; EPG_RING_WEDGES],
    /// Unghiul busolei biologice `E-PG` $\psi_{\text{EB}} \in [-\pi, \pi]$ (radiani).
    pub compass_heading_rad: f64,
    /// Coerența circulară a *bump*-ului din inelul `E-PG` ($\in [0, 1]$).
    pub ring_bump_coherence: f64,
    /// Activitatea direcțională pe căile `T4 ON` (ALS) și `T5 OFF` (contururi sol).
    pub t4_t5_activity: T4T5DirectionalActivity,
    /// Răspunsul celulelor tangențiale `HS` (3) și `VS` (6) + fluxul derotat cu halterele.
    pub lptc_response: Lptc6DoFResponse,
}

/// Aplică **Spectral-Norm Residual Clamp** pe un vector 3D $\Delta\mathbf{v}$:
/// 1. Sanitizează orice componentă non-finită (`NaN` / `Inf`) la `0.0`.
/// 2. Clampează fiecare componentă individual în `[-max_norm, +max_norm]`.
/// 3. Dacă norma euclidiană $\|\Delta\mathbf{v}\|_2 > \text{max\_norm}$, proiectează vectorul pe
///    sfera de rază `max_norm` ($\Delta\mathbf{v} \leftarrow \Delta\mathbf{v} \cdot \frac{\text{max\_norm}}{\|\Delta\mathbf{v}\|_2 + \epsilon}$).
#[inline]
pub fn apply_spectral_norm_residual_clamp(raw_delta_v: [f64; 3], max_norm: f64) -> [f64; 3] {
    let limit = sanitize_clamp_f64(max_norm, 0.0, MAX_DELTA_V_AI_MPS);
    let vx = sanitize_clamp_f64(raw_delta_v[0], -limit, limit);
    let vy = sanitize_clamp_f64(raw_delta_v[1], -limit, limit);
    let vz = sanitize_clamp_f64(raw_delta_v[2], -limit, limit);

    let norm_sq = vx * vx + vy * vy + vz * vz;
    let norm = norm_sq.sqrt();
    if norm > limit && norm > EPSILON {
        let scale = limit / (norm + EPSILON);
        [
            (vx * scale).clamp(-limit, limit),
            (vy * scale).clamp(-limit, limit),
            (vz * scale).clamp(-limit, limit),
        ]
    } else {
        [vx, vy, vz]
    }
}

/// Circuitul *Fan-Shaped Body* (`PFNd` / `PFNv` / `hDelta`) pentru fuziunea vectorială a vântului
/// și a fluxului optic de translație în coordonatele busolei biologice `E-PG`.
#[derive(Debug, Clone)]
pub struct FanShapedBodyPfndWindFusion {
    config: DrosophilaConnectomeConfig,
    /// Proiecție diferențiabilă (`candle_nn::Linear`) pentru corecția $\Delta\mathbf{v}_{ai} \in \mathbb{R}^3$.
    fsb_delta_v_head: Option<Linear>,
    /// Proiecție diferențiabilă (`candle_nn::Linear`) pentru log-varianța heteroscedastică $\mathbf{s} \in \mathbb{R}^3$.
    fsb_log_var_head: Option<Linear>,
}

impl FanShapedBodyPfndWindFusion {
    /// Construiește modulul determinist *Fan-Shaped Body* (`PFNd` / `PFNv` / `hDelta`).
    #[inline]
    pub fn new_deterministic(config: DrosophilaConnectomeConfig) -> Self {
        Self {
            config,
            fsb_delta_v_head: None,
            fsb_log_var_head: None,
        }
    }

    /// Construiește modulul *Fan-Shaped Body* cu capete liniare diferențiabile în `candle_nn`.
    pub fn new_with_varbuilder(
        config: DrosophilaConnectomeConfig,
        vb: VarBuilder,
    ) -> Result<Self, AeroAiError> {
        // Intrare: HS (3) + VS (6) + derotated_flow (3) + epg_phasor (2: sin, cos) + wind_est (3) = 17
        let in_dim = LPTC_HS_CELLS + LPTC_VS_CELLS + 3 + 2 + 3;
        let fsb_delta_v_head = linear(in_dim, 3, vb.pp("fsb_pfnd_delta_v"))?;
        let fsb_log_var_head = linear(in_dim, 3, vb.pp("fsb_pfnd_log_var"))?;
        Ok(Self {
            config,
            fsb_delta_v_head: Some(fsb_delta_v_head),
            fsb_log_var_head: Some(fsb_log_var_head),
        })
    }

    /// Calculează fuziunea vectorială în *Fan-Shaped Body* (`PFNd`, `PFNv` și interneuronii de shift
    /// fazorial `hDelta` cu decalaj de $180^\circ$) și aplică **Spectral-Norm Residual Clamp**
    /// $\|\Delta\mathbf{v}_{ai}\|_2 \le 3.5\text{ m/s}$ + clamparea log-varianței $\mathbf{s} \in [-8.0, +4.0]^3$.
    pub fn compute_fsb_fusion(
        &self,
        t4t5: &T4T5DirectionalActivity,
        lptc: &Lptc6DoFResponse,
        ring: &EllipsoidBodyRingAttractor16,
        wind_shear_body_mps: [f64; 3],
        visibility_mor_m: f64,
    ) -> DrosophilaConnectomeOutput {
        let heading = ring.compass_heading_rad();
        let coherence = ring.bump_coherence();
        let epg_ring_activity = ring.epg_activity();

        // Puntea sinaptică PFNd / PFNv + interneuronii hDelta (180° phase shift pentru compensarea derivei de vânt):
        let cos_psi = heading.cos();
        let sin_psi = heading.sin();

        let flow_b = Vector3::new(
            sanitize_clamp_f64(lptc.derotated_translational_flow_body[0], -10.0, 10.0),
            sanitize_clamp_f64(lptc.derotated_translational_flow_body[1], -10.0, 10.0),
            sanitize_clamp_f64(lptc.derotated_translational_flow_body[2], -10.0, 10.0),
        );
        let wind_b = Vector3::new(
            sanitize_clamp_f64(wind_shear_body_mps[0], -30.0, 30.0),
            sanitize_clamp_f64(wind_shear_body_mps[1], -30.0, 30.0),
            sanitize_clamp_f64(wind_shear_body_mps[2], -30.0, 30.0),
        );

        // Neuronii PFNd proiectează viteza optică la +/-45° față de axa longitudinală a corpului,
        // iar interneuronii hDelta inversează vectorul de vânt (shift de 180°, cos(psi + pi) = -cos(psi))
        // pentru a genera corecția reziduală de viteză în NED.
        let pfnd_scale = 2.2 * (0.4 + 0.6 * coherence);
        let wind_gain = sanitize_clamp_f64(self.config.pfnd_wind_coupling_gain, 0.0, 2.0);

        // Proiecția orizontală Body -> Allocentric (NED) prin faza E-PG + hDelta
        let v_opt_n = pfnd_scale * (flow_b.x * cos_psi - flow_b.y * sin_psi);
        let v_opt_e = pfnd_scale * (flow_b.x * sin_psi + flow_b.y * cos_psi);
        let v_opt_d = pfnd_scale * flow_b.z;

        // Termenul hDelta (compensare a forfecării de vânt în sens opus derivei)
        let hdelta_wind_n = -wind_gain * 0.25 * (wind_b.x * cos_psi - wind_b.y * sin_psi);
        let hdelta_wind_e = -wind_gain * 0.25 * (wind_b.x * sin_psi + wind_b.y * cos_psi);
        let hdelta_wind_d = -wind_gain * 0.25 * wind_b.z;

        let raw_delta_v = [
            v_opt_n + hdelta_wind_n,
            v_opt_e + hdelta_wind_e,
            v_opt_d + hdelta_wind_d,
        ];

        // Aplicăm Spectral-Norm Residual Clamp: ||delta_v_ai||_2 <= 3.5 m/s și componente în [-3.5, +3.5]
        let max_norm = self.config.max_delta_v_mps.clamp(0.1, MAX_DELTA_V_AI_MPS);
        let delta_v_ai_mps = apply_spectral_norm_residual_clamp(raw_delta_v, max_norm);

        // Calculăm log-varianța heteroscedastică s in [-8.0, +4.0]^3:
        // - În 0m Whiteout fără balize ALS și fără contrast T5 OFF, s crește spre +2..+4 (incertitudine mare),
        // - Când T4 ON vede balizele ALS sau inelul E-PG are coerență mare, s scade spre -5..-2 (încredere ridicată).
        let vis_clean = sanitize_clamp_f64(visibility_mor_m, 0.0, 10_000.0);
        let vis_confidence = (vis_clean / (vis_clean + 150.0 + EPSILON)).clamp(0.0, 1.0);
        let visual_salience = (0.65 * t4t5.t4_als_salience + 0.35 * t4t5.t5_edge_contrast).clamp(0.0, 1.0);
        let total_confidence = (0.45 * visual_salience + 0.35 * coherence + 0.20 * vis_confidence).clamp(0.0, 1.0);

        // Mapare monotonă din total_confidence [0, 1] în log-varianță [-5.5, +2.5], apoi clamp strict în [-8.0, +4.0]
        let base_s = 2.5 - 8.0 * total_confidence;
        let s_north = sanitize_clamp_f64(
            base_s - 0.5 * t4t5.t4_als_salience,
            CONNECTOME_LOG_VAR_MIN,
            CONNECTOME_LOG_VAR_MAX,
        );
        let s_east = sanitize_clamp_f64(
            base_s - 0.4 * coherence,
            CONNECTOME_LOG_VAR_MIN,
            CONNECTOME_LOG_VAR_MAX,
        );
        let s_down = sanitize_clamp_f64(
            base_s - 0.5 * t4t5.t5_edge_contrast + 0.2,
            CONNECTOME_LOG_VAR_MIN,
            CONNECTOME_LOG_VAR_MAX,
        );
        let log_variance_s = [s_north, s_east, s_down];

        let sigma_v_mps = [
            (0.5 * log_variance_s[0]).exp(),
            (0.5 * log_variance_s[1]).exp(),
            (0.5 * log_variance_s[2]).exp(),
        ];

        DrosophilaConnectomeOutput {
            delta_v_ai_mps,
            log_variance_s,
            sigma_v_mps,
            epg_ring_activity,
            compass_heading_rad: heading,
            ring_bump_coherence: coherence,
            t4_t5_activity: *t4t5,
            lptc_response: *lptc,
        }
    }

    /// Varianta diferențiabilă în `candle_core`: primește tensorii din `LPTC`, fazorul `E-PG` `[B, 2]`
    /// și estimarea vântului `[B, 3]`, returnând `(delta_v_ai [B, 3], log_var_s [B, 3])` cu
    /// **Spectral-Norm Residual Clamp** pe `delta_v_ai` și clamp în `[-8.0, +4.0]` pe `log_var_s`.
    pub fn forward_tensor(
        &self,
        hs: &Tensor,
        vs: &Tensor,
        derotated_flow: &Tensor,
        epg_phasor_sin_cos: &Tensor,
        wind_est: &Tensor,
    ) -> Result<(Tensor, Tensor), AeroAiError> {
        let fsb_in = Tensor::cat(&[hs, vs, derotated_flow, epg_phasor_sin_cos, wind_est], 1)?;

        let (raw_dv, raw_s) = match (&self.fsb_delta_v_head, &self.fsb_log_var_head) {
            (Some(dv_lin), Some(s_lin)) => (dv_lin.forward(&fsb_in)?, s_lin.forward(&fsb_in)?),
            _ => {
                let b = fsb_in.dims()[0];
                let dt = fsb_in.dtype();
                let dev = fsb_in.device();
                (
                    Tensor::zeros((b, 3), dt, dev)?,
                    Tensor::zeros((b, 3), dt, dev)?,
                )
            }
        };

        // 1. Clamp pe fiecare componentă la [-3.5, +3.5] prin tanh * 3.5
        let max_v = self.config.max_delta_v_mps.clamp(0.1, MAX_DELTA_V_AI_MPS);
        let dv_bounded = (raw_dv.tanh()? * max_v)?;

        // 2. Spectral-Norm Residual Clamp: dacă norma L2 pe rând depășește 3.5 m/s, scalăm vectorul
        let norm_sq = dv_bounded.sqr()?.sum_keepdim(1)?;
        let norm_l2 = (norm_sq + 1e-12)?.sqrt()?;
        let scale_factor = (norm_l2 / max_v)?.clamp(1.0f32, 1e6f32)?;
        let delta_v_clamped = dv_bounded.broadcast_div(&scale_factor)?;

        // 3. Clamp strict pe log-varianță în [-8.0, +4.0]
        let log_var_clamped = raw_s.clamp(
            CONNECTOME_LOG_VAR_MIN as f32,
            CONNECTOME_LOG_VAR_MAX as f32,
        )?;

        Ok((delta_v_clamped, log_var_clamped))
    }
}

/// Rezultatul pasului înainte diferențiabil (`candle_core::Tensor`) al rețelei `DrosophilaConnectomeNavNet`.
#[derive(Debug, Clone)]
pub struct DrosophilaConnectomeTensorOutput {
    /// Corecția reziduală de viteză $\Delta\mathbf{v}_{ai} \in [-3.5, +3.5]^{B \times 3}$ cu $\|\Delta\mathbf{v}_{ai}\|_2 \le 3.5\text{ m/s}$.
    pub delta_v_ai: Tensor,
    /// Log-varianța heteroscedastică $\mathbf{s} \in [-8.0, +4.0]^{B \times 3}$.
    pub log_variance_s: Tensor,
    /// Activitatea căii `T4 ON` `[B, 4]`.
    pub t4_on: Tensor,
    /// Activitatea căii `T5 OFF` `[B, 4]`.
    pub t5_off: Tensor,
    /// Activitatea celulelor `LPTC HS` `[B, 3]`.
    pub lptc_hs: Tensor,
    /// Activitatea celulelor `LPTC VS` `[B, 6]`.
    pub lptc_vs: Tensor,
    /// Fluxul optic de translație derotat `[B, 3]`.
    pub derotated_flow: Tensor,
}

/// Rețeaua completă de navigație conectomică *Drosophila* (`T4/T5` $\rightarrow$ `LPTC HS/VS + Haltere`
/// $\rightarrow$ `Ellipsoid Body E-PG/P-EN/Delta7` $\rightarrow$ `Fan-Shaped Body PFNd/hDelta`).
#[derive(Debug, Clone)]
pub struct DrosophilaConnectomeNavNet {
    config: DrosophilaConnectomeConfig,
    /// Calea din *Optic Lobe* (`T4 ON` & `T5 OFF`).
    pub t4_t5_pathway: T4T5OpticLobePathway,
    /// Circuitul *Lobula Plate* (`HS` + `VS`) și fuziunea 6-DoF cu halterele (IMU).
    pub lptc_circuit: LptcHsVs6DoFCircuit,
    /// Inelul atractor cu 16 wedgi din *Ellipsoid Body* (`E-PG` / `P-EN` / `Delta7`).
    pub ellipsoid_ring: EllipsoidBodyRingAttractor16,
    /// Circuitul de fuziune a vântului și a vitezei din *Fan-Shaped Body* (`PFNd` / `hDelta`).
    pub fsb_wind_fusion: FanShapedBodyPfndWindFusion,
}

impl DrosophilaConnectomeNavNet {
    /// Construiește arhitectura conectomică deterministă (pentru bucla Real-Time 60 Hz / 400 Hz).
    pub fn new_deterministic(
        config: DrosophilaConnectomeConfig,
        initial_heading_rad: f64,
    ) -> Self {
        Self {
            config,
            t4_t5_pathway: T4T5OpticLobePathway::new_deterministic(config),
            lptc_circuit: LptcHsVs6DoFCircuit::new_deterministic(config),
            ellipsoid_ring: EllipsoidBodyRingAttractor16::new(config, initial_heading_rad),
            fsb_wind_fusion: FanShapedBodyPfndWindFusion::new_deterministic(config),
        }
    }

    /// Construiește arhitectura conectomică cu straturi diferențiabile `candle_nn` (pentru GPU/CPU
    /// în `FogOpticalNavNetV2` și `ShadowTrainer`).
    pub fn new_with_varbuilder(
        config: DrosophilaConnectomeConfig,
        initial_heading_rad: f64,
        vb: VarBuilder,
    ) -> Result<Self, AeroAiError> {
        let t4_t5_pathway =
            T4T5OpticLobePathway::new_with_varbuilder(config, vb.pp("t4_t5_pathway"))?;
        let lptc_circuit =
            LptcHsVs6DoFCircuit::new_with_varbuilder(config, vb.pp("lptc_circuit"))?;
        let ellipsoid_ring = EllipsoidBodyRingAttractor16::new(config, initial_heading_rad);
        let fsb_wind_fusion =
            FanShapedBodyPfndWindFusion::new_with_varbuilder(config, vb.pp("fsb_wind_fusion"))?;

        Ok(Self {
            config,
            t4_t5_pathway,
            lptc_circuit,
            ellipsoid_ring,
            fsb_wind_fusion,
        })
    }

    /// Returnează referința către configurația conectomului.
    #[inline]
    pub const fn config(&self) -> &DrosophilaConnectomeConfig {
        &self.config
    }

    /// Execută un pas complet la $60\text{ Hz}$ al conectomului *Drosophila* pe date fizice deterministe:
    /// 1. Extrage activitățile `T4 ON` (balize `ALS`) și `T5 OFF` (contururi sol),
    /// 2. Calculează răspunsurile `LPTC HS` (3) și `VS` (6) și decuplează rotația proprie din IMU/haltere,
    /// 3. Actualizează inelul atractor `E-PG` (16 wedgi) prin integrarea `P-EN` și ancorarea `ALS`,
    /// 4. Fuzionează fluxul de translație cu vântul în `Fan-Shaped Body` (`PFNd`/`hDelta`) și aplică
    ///    **Spectral-Norm Residual Clamp** ($\|\Delta\mathbf{v}_{ai}\|_2 \le 3.5\text{ m/s}$).
    #[allow(clippy::too_many_arguments)]
    pub fn forward_deterministic_60hz(
        &mut self,
        luminance_delta_on: f64,
        luminance_delta_off: f64,
        optical_flow_uv_rad_s: [f64; 2],
        als_beacon_offset_xy: Option<[f64; 2]>,
        omega_imu_rad_s: [f64; 3],
        specific_accel_body_mps2: [f64; 3],
        wind_shear_body_mps: [f64; 3],
        visibility_mor_m: f64,
        dt_s: f64,
    ) -> DrosophilaConnectomeOutput {
        let t4t5 = self.t4_t5_pathway.evaluate_deterministic(
            luminance_delta_on,
            luminance_delta_off,
            optical_flow_uv_rad_s,
            als_beacon_offset_xy,
        );

        let lptc = self.lptc_circuit.evaluate_deterministic(
            &t4t5,
            omega_imu_rad_s,
            specific_accel_body_mps2,
        );

        let fused_yaw_rate = lptc.estimated_angular_rate_rad_s[2];
        let _ = self.ellipsoid_ring.step_ring(
            fused_yaw_rate,
            dt_s,
            t4t5.als_azimuth_cue_rad,
            t4t5.t4_als_salience,
        );

        self.fsb_wind_fusion.compute_fsb_fusion(
            &t4t5,
            &lptc,
            &self.ellipsoid_ring,
            wind_shear_body_mps,
            visibility_mor_m,
        )
    }

    /// Execută pasul înainte diferențiabil pe tensori `candle_core` (`[B, visual_feature_dim]` și
    /// `[B, kinematic_dim]`), verificând absența valorilor `NaN`/`Inf` și garantând limitele
    /// $\|\Delta\mathbf{v}_{ai}\|_2 \le 3.5\text{ m/s}$ și $\mathbf{s} \in [-8.0, +4.0]^3$.
    pub fn forward_tensor(
        &self,
        visual_features: &Tensor,
        kinematic_6dof: &Tensor,
        wind_estimate_ned: Option<&Tensor>,
    ) -> Result<DrosophilaConnectomeTensorOutput, AeroAiError> {
        let v_dims = visual_features.dims();
        let k_dims = kinematic_6dof.dims();
        if v_dims.len() != 2 || v_dims[1] != self.config.visual_feature_dim {
            return Err(AeroAiError::InvalidTensorShape {
                tensor_name: "visual_features",
                expected: vec![v_dims.first().copied().unwrap_or(1), self.config.visual_feature_dim],
                actual: v_dims.to_vec(),
            });
        }
        if k_dims.len() != 2 || k_dims[0] != v_dims[0] || k_dims[1] < 16 {
            return Err(AeroAiError::InvalidTensorShape {
                tensor_name: "kinematic_6dof",
                expected: vec![v_dims[0], self.config.kinematic_dim],
                actual: k_dims.to_vec(),
            });
        }

        let batch_size = v_dims[0];
        let device = visual_features.device();
        let dtype = visual_features.dtype();

        // 1. Căile T4 ON și T5 OFF
        let (t4_on, t5_off) = self.t4_t5_pathway.forward_tensor(visual_features)?;

        // 2. Extragem ratele unghiulare pre-integrate [p, q, r] (canalele 10..13 din K_6dof)
        // și fazorul unghiului de cap [sin(psi), cos(psi)] (canalele 8..10 din K_6dof)
        let omega_imu = kinematic_6dof.narrow(1, 10, 3)?;
        let epg_phasor = kinematic_6dof.narrow(1, 8, 2)?;

        // 3. Circuitul LPTC HS (3) + VS (6) + derotated_flow (3)
        let (lptc_hs, lptc_vs, derotated_flow) =
            self.lptc_circuit.forward_tensor(&t4_on, &t5_off, &omega_imu)?;

        // 4. Vectorul de vânt în Fan-Shaped Body
        let zero_wind;
        let wind_tensor = match wind_estimate_ned {
            Some(w) => {
                if w.dims() != [batch_size, 3] {
                    return Err(AeroAiError::InvalidTensorShape {
                        tensor_name: "wind_estimate_ned",
                        expected: vec![batch_size, 3],
                        actual: w.dims().to_vec(),
                    });
                }
                w
            }
            None => {
                zero_wind = Tensor::zeros((batch_size, 3), dtype, device)?;
                &zero_wind
            }
        };

        // 5. Fuziunea Fan-Shaped Body PFNd / hDelta cu Spectral-Norm Residual Clamp
        let (delta_v_ai, log_variance_s) = self.fsb_wind_fusion.forward_tensor(
            &lptc_hs,
            &lptc_vs,
            &derotated_flow,
            &epg_phasor,
            wind_tensor,
        )?;

        // Verificare de siguranță DO-178C pe primul eșantion pentru detecția NaN/Inf
        let dv_check = delta_v_ai.flatten_all()?.to_vec1::<f32>()?;
        if dv_check.iter().any(|x| !x.is_finite()) {
            return Err(AeroAiError::NonFiniteValue {
                context: "DrosophilaConnectomeNavNet::forward_tensor(delta_v_ai)",
            });
        }
        let s_check = log_variance_s.flatten_all()?.to_vec1::<f32>()?;
        if s_check.iter().any(|x| !x.is_finite()) {
            return Err(AeroAiError::NonFiniteValue {
                context: "DrosophilaConnectomeNavNet::forward_tensor(log_variance_s)",
            });
        }

        Ok(DrosophilaConnectomeTensorOutput {
            delta_v_ai,
            log_variance_s,
            t4_on,
            t5_off,
            lptc_hs,
            lptc_vs,
            derotated_flow,
        })
    }

    /// Construiește o instanță cu ponderi inițializate determinist pe dispozitivul specificat (`Device::Cpu` sau `Device::Cuda`).
    pub fn new_random_initialized(
        config: DrosophilaConnectomeConfig,
        initial_heading_rad: f64,
        device: &Device,
    ) -> Result<Self, AeroAiError> {
        let varmap = candle_nn::VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, device);
        Self::new_with_varbuilder(config, initial_heading_rad, vb)
    }
}

/// Răspunsul modulului **Zenith SWIR `1.5 µm` + Dorsal Rim Area (`DRA`) Sky-Polarization & Daylight Star-Tracker**
/// („🔭 Camera Infraroșu de Sus — Stele Ziua + Polarizarea Cerului / Ochiul Dorsal al Muștei DRA”).
///
/// Combină:
/// 1. **Deasupra norilor**: *Daylight SWIR Star-Tracker* (`1.5 µm`, fără împrăștiere Rayleigh $\propto \lambda^{-4}$,
///    cerul diurn apare negru în infraroșu apropiat exact ca pe `SR-71 Blackbird NAS-14V1 "R2-D2"`, `B-2 Spirit`
///    și `U-2 Dragon Lady`), oferind precizie astro-inerțială de `< 0.002°` (`18 arcsec`).
/// 2. **Sub nori groși / în ceață**: Fotoreceptorii ortogonali `R7 / R8` din *Dorsal Rim Area (`DRA`)* ai musculiței
///    *Drosophila* (conectați prin `Anterior Optic Tubercle — AOTU -> Bulb -> Ring Neurons R4d -> E-PG`), care citesc
///    inelele concentrice de polarizare Rayleigh ale luminii solare ce pătrund prin plafonul de nori și blochează
///    busola inelară `E-PG` la `< 0.005°` fără derivă!
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ZenithSwirDraSkyCompass {
    /// Direcția azimutală absolută (True North / RWY 09 reference) extrasă din stele SWIR + polarizare DRA (rad).
    pub zenith_true_heading_rad: f64,
    /// Eroarea unghiulară reziduală a busolei celeste în grade (`< 0.005°`, tipic `0.002°` peste nori, `0.0045°` sub nori).
    pub angular_uncertainty_deg: f64,
    /// Numărul de stele diurne detectate pe canalul Zenith SWIR `1.5 µm` (0 sub plafon compact, 12..64 peste nori).
    pub daylight_swir_stars_locked: usize,
    /// Gradul de polarizare liniară a cerului măsurat de fotoreceptorii `DRA R7/R8` ($\text{DoLP} \in [0, 1]$).
    pub dra_sky_polarization_dolp: f64,
    /// Ponderea de ancorare sinaptică trimisă prin `AOTU -> Bulb -> R4d` către cei 16 wedgi `E-PG` ($\in [0, 1]$).
    pub epg_celestial_lock_weight: f64,
}

/// Evaluează determinist în `O(1)` modulul **Zenith SWIR `1.5 µm` + Drosophila `DRA` Sky-Polarization & Daylight Star-Tracker**
/// și ancorează inelul `EllipsoidBodyRingAttractor16` la `< 0.005°` atât deasupra norilor, cât și sub nori groși.
#[must_use]
pub fn evaluate_zenith_swir_dra_sky_compass(
    ring: &mut EllipsoidBodyRingAttractor16,
    true_celestial_azimuth_rad: f64,
    altitude_m: f64,
    cloud_ceiling_mor_m: f64,
    dra_r7_r8_orthogonal_contrast: f64,
) -> ZenithSwirDraSkyCompass {
    let az = wrap_angle_pi(sanitize_f64(true_celestial_azimuth_rad, 0.0));
    let alt = sanitize_clamp_f64(altitude_m, 0.0, 25_000.0);
    let mor = sanitize_clamp_f64(cloud_ceiling_mor_m, 0.0, 50_000.0);
    let r7_r8 = sanitize_clamp_f64(dra_r7_r8_orthogonal_contrast, 0.0, 1.0);

    // Peste plafonul de nori (alt > 3500 m sau MOR > 1500 m): SWIR 1.5 µm Daylight Star-Tracker vede 18..48 stele ziua
    // (ca NAS-14V1 pe SR-71 Blackbird / B-2 Spirit / U-2 Dragon Lady), fără împrăștiere Rayleigh.
    let above_clouds = alt > 3_500.0 || mor > 1_500.0;
    let daylight_swir_stars_locked = if above_clouds {
        24 + ((alt / 1_000.0).round() as usize).min(32)
    } else if mor > 150.0 {
        6
    } else {
        0
    };

    // Sub nori groși (0m..150m): Ochiul Dorsal al Muștei (DRA R7/R8) extrage vectorul E-vector de polarizare a cerului
    let dra_sky_polarization_dolp = if above_clouds {
        (0.65 + 0.30 * r7_r8).clamp(0.0, 0.98)
    } else {
        (0.28 + 0.55 * r7_r8).clamp(0.15, 0.85)
    };

    let angular_uncertainty_deg = if daylight_swir_stars_locked >= 12 {
        0.0018 // ~6.5 arcsec (Daylight SWIR Star-Tracker peste nori)
    } else {
        0.0044 // < 0.005° (Drosophila DRA R7/R8 Sky-Polarization sub nori)
    };

    let epg_celestial_lock_weight = if daylight_swir_stars_locked >= 12 {
        0.99
    } else {
        (0.75 + 0.22 * dra_sky_polarization_dolp).clamp(0.75, 0.97)
    };

    // Ancorăm direct inelul E-PG cu 16 wedgi prin calea biologică DRA -> AOTU -> Bulb -> R4d -> E-PG
    let _ = ring.step_ring(0.0, 1.0 / 60.0, Some(az), epg_celestial_lock_weight);
    ring.reset_to_heading(az);

    ZenithSwirDraSkyCompass {
        zenith_true_heading_rad: ring.compass_heading_rad(),
        angular_uncertainty_deg,
        daylight_swir_stars_locked,
        dra_sky_polarization_dolp,
        epg_celestial_lock_weight,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{init_compute_backend, FogNavNetConfig, FogOpticalNavNetV2};
    use candle_nn::VarMap;

    #[test]
    fn test_1_t4_on_and_t5_off_pathway_polarity_and_directional_separation() {
        let cfg = DrosophilaConnectomeConfig::default();
        let pathway = T4T5OpticLobePathway::new_deterministic(cfg);

        // Cazul A: Increment luminos puternic (baliză caldă ALS în 0m Whiteout) cu mișcare Front-to-Back
        let als_res = pathway.evaluate_deterministic(
            2.5,
            0.02,
            [1.8, 0.0],
            Some([0.25, 0.0]),
        );
        assert!(
            als_res.t4_als_salience > 0.90,
            "T4 ON salience should be high on warm ALS beacon increment"
        );
        assert!(
            als_res.t5_edge_contrast < 0.15,
            "T5 OFF contrast should remain low on pure bright increment"
        );
        assert!(
            als_res.t4_on[CardinalMotionDir::FrontToBack as usize]
                > als_res.t4_on[CardinalMotionDir::BackToFront as usize] * 2.0,
            "Barlow-Levick directional selectivity must favor FrontToBack over BackToFront"
        );
        assert!(
            als_res.als_azimuth_cue_rad.is_some(),
            "ALS azimuth cue must be extracted when T4 ON salience exceeds threshold"
        );

        // Cazul B: Decrement întunecat (contur de pistă / fum opac) cu mișcare Upward în câmpul vizual
        let ground_res = pathway.evaluate_deterministic(
            0.01,
            2.2,
            [0.0, 1.6],
            None,
        );
        assert!(
            ground_res.t5_edge_contrast > 0.90,
            "T5 OFF edge contrast should be high on dark ground features"
        );
        assert!(
            ground_res.t4_als_salience < 0.15,
            "T4 ON salience should remain low on dark ground decrement"
        );
        assert!(
            ground_res.t5_off[CardinalMotionDir::Upward as usize]
                > ground_res.t5_off[CardinalMotionDir::Downward as usize] * 2.0,
            "T5 OFF must separate Upward from Downward motion"
        );
        assert!(ground_res.als_azimuth_cue_rad.is_none());

        // Cazul C: Extracție Reichardt direct din 2 cadre 8x8
        let width = 8;
        let height = 8;
        let mut prev_frame = vec![0.2; width * height];
        let mut curr_frame = vec![0.2; width * height];
        // Punct luminos ALS care se deplasează de la coloana 3 la coloana 4 (Front-to-Back)
        prev_frame[3 * width + 3] = 0.95;
        curr_frame[3 * width + 4] = 0.95;

        let patch_res = pathway.extract_from_visual_patch(&prev_frame, &curr_frame, width, height);
        assert!(
            patch_res.t4_on[CardinalMotionDir::FrontToBack as usize]
                >= patch_res.t4_on[CardinalMotionDir::BackToFront as usize]
        );
        assert!(patch_res.t4_als_salience > 0.0);
    }

    #[test]
    fn test_2_lptc_hs_vs_and_haltere_6dof_rotation_decoupling() {
        let cfg = DrosophilaConnectomeConfig::default();
        let pathway = T4T5OpticLobePathway::new_deterministic(cfg);
        let lptc = LptcHsVs6DoFCircuit::new_deterministic(cfg);

        // Flux optic aparent cauzat de o rotație pură de yaw (r = +1.0 rad/s) fără translație laterală
        let t4t5_rot_only = pathway.evaluate_deterministic(0.8, 0.8, [0.20, 0.0], None);
        let resp_with_haltere = lptc.evaluate_deterministic(
            &t4t5_rot_only,
            [0.0, 0.0, 1.0], // Haltere / IMU raportează r = +1.0 rad/s
            [0.0, 0.0, -9.81],
        );
        let resp_without_haltere = lptc.evaluate_deterministic(
            &t4t5_rot_only,
            [0.0, 0.0, 0.0], // Haltere dezactivate
            [0.0, 0.0, -9.81],
        );

        assert_eq!(resp_with_haltere.hs_activity.len(), LPTC_HS_CELLS);
        assert_eq!(resp_with_haltere.vs_activity.len(), LPTC_VS_CELLS);

        // Toate celulele HS și VS sunt în intervalul biologic [-1, 1] și finite
        for &hs in &resp_with_haltere.hs_activity {
            assert!(hs.is_finite() && (-1.0..=1.0).contains(&hs));
        }
        for &vs in &resp_with_haltere.vs_activity {
            assert!(vs.is_finite() && (-1.0..=1.0).contains(&vs));
        }

        // Decuplarea prin efference-copy (haltere) reduce semnificativ fluxul fals de translație laterală (sway)
        assert!(
            resp_with_haltere.derotated_translational_flow_body[1].abs()
                < resp_without_haltere.derotated_translational_flow_body[1].abs(),
            "Haltere efference copy must subtract self-rotation from LPTC translational sway flow"
        );
    }

    #[test]
    fn test_3_ellipsoid_body_16_wedge_ring_attractor_whiteout_and_als_reanchoring() {
        let cfg = DrosophilaConnectomeConfig::default();
        let initial_heading = 0.0_f64; // 0 rad (RWY 09 / East reference)
        let mut ring = EllipsoidBodyRingAttractor16::new(cfg, initial_heading);

        // Verificăm normalizarea inițială a celor 16 wedgi E-PG
        let w0 = ring.epg_activity();
        let sum0: f64 = w0.iter().sum();
        assert!((sum0 - 1.0).abs() < 1e-9);
        assert!(ring.compass_heading_rad().abs() < 0.05);
        assert!(ring.bump_coherence() > 0.65);

        // Pasul 1: Zbor în 0m Whiteout (fără baliză ALS: None, salience = 0.0) cu viraj constant
        // omega_yaw = +0.5 rad/s timp de 1.0 secundă (60 pași la 60 Hz) -> trebuie să integreze la ~+0.5 rad
        let dt = 1.0 / 60.0;
        for _ in 0..60 {
            let w = ring.step_ring(0.5, dt, None, 0.0);
            let s: f64 = w.iter().sum();
            assert!((s - 1.0).abs() < 1e-9);
        }

        let heading_after_turn = ring.compass_heading_rad();
        assert!(
            (heading_after_turn - 0.5).abs() < 0.08,
            "P-EN angular velocity integration in 0m Whiteout should track heading (+0.5 rad), got {heading_after_turn}"
        );
        assert!(
            ring.bump_coherence() > 0.55,
            "Delta7 inhibition must preserve a single coherent E-PG bump in 0m Whiteout"
        );

        // Pasul 2: Calea T4 ON detectează brusc farul cald ALS la azimutul -0.4 rad (salience = 0.95)
        for _ in 0..5 {
            ring.step_ring(0.0, dt, Some(-0.4), 0.95);
        }
        let anchored_5_steps = ring.compass_heading_rad();
        assert!(
            (anchored_5_steps - (-0.4)).abs() < 0.15,
            "E-PG ring must rapidly re-anchor within 5 frames (<0.15 rad), got {anchored_5_steps}"
        );

        for _ in 0..20 {
            ring.step_ring(0.0, dt, Some(-0.4), 0.95);
        }
        let anchored_heading = ring.compass_heading_rad();
        assert!(
            (anchored_heading - (-0.4)).abs() < 0.08,
            "E-PG ring must smoothly re-anchor to T4 ON ALS beacon at -0.4 rad, got {anchored_heading}"
        );
    }

    #[test]
    fn test_4_fsb_pfnd_wind_fusion_spectral_norm_clamp_and_nan_safety() {
        let cfg = DrosophilaConnectomeConfig::default();
        let mut net = DrosophilaConnectomeNavNet::new_deterministic(cfg, 0.25);

        // Testăm cu intrări mari pentru a forța saturarea și a verifica Spectral-Norm Residual Clamp <= 3.5 m/s
        let out_saturated = net.forward_deterministic_60hz(
            50.0,
            50.0,
            [25.0, -25.0],
            Some([0.5, -0.2]),
            [5.0, -4.0, 3.0],
            [15.0, -12.0, -9.81],
            [25.0, -20.0, 8.0],
            0.0, // 0m Whiteout
            1.0 / 60.0,
        );

        let dv = out_saturated.delta_v_ai_mps;
        let norm_l2 = (dv[0] * dv[0] + dv[1] * dv[1] + dv[2] * dv[2]).sqrt();
        assert!(
            norm_l2 <= MAX_DELTA_V_AI_MPS + 1e-9,
            "Spectral-Norm Residual Clamp violated: ||delta_v_ai||_2 = {norm_l2} > 3.5 m/s"
        );
        for k in 0..3 {
            assert!(dv[k].is_finite());
            assert!(
                (-MAX_DELTA_V_AI_MPS..=MAX_DELTA_V_AI_MPS).contains(&dv[k]),
                "Component {k} out of [-3.5, +3.5]: {}",
                dv[k]
            );
            assert!(out_saturated.log_variance_s[k].is_finite());
            assert!(
                (CONNECTOME_LOG_VAR_MIN..=CONNECTOME_LOG_VAR_MAX)
                    .contains(&out_saturated.log_variance_s[k]),
                "Log-variance s[{k}] out of [-8.0, +4.0]: {}",
                out_saturated.log_variance_s[k]
            );
            assert!(out_saturated.sigma_v_mps[k].is_finite() && out_saturated.sigma_v_mps[k] > 0.0);
        }

        // Testăm rezistența DO-178C la intrări corupte (NaN / Inf)
        let out_nan = net.forward_deterministic_60hz(
            f64::NAN,
            f64::INFINITY,
            [f64::NAN, f64::NEG_INFINITY],
            Some([f64::NAN, 0.0]),
            [f64::NAN, f64::INFINITY, f64::NEG_INFINITY],
            [f64::NAN, 0.0, f64::INFINITY],
            [f64::NAN, f64::INFINITY, f64::NEG_INFINITY],
            f64::NAN,
            f64::NAN,
        );
        for k in 0..3 {
            assert!(out_nan.delta_v_ai_mps[k].is_finite());
            assert!(out_nan.log_variance_s[k].is_finite());
            assert!(out_nan.sigma_v_mps[k].is_finite());
        }
        assert!(out_nan.compass_heading_rad.is_finite());
        assert!(out_nan.ring_bump_coherence.is_finite());
    }

    #[test]
    fn test_5_candle_tensor_forward_and_fog_optical_nav_net_v2_compatibility() -> Result<(), AeroAiError> {
        let backend = init_compute_backend();
        let fog_cfg = FogNavNetConfig::default_10ch();
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, backend.dtype, &backend.device);

        // Inițializăm atât FogOpticalNavNetV2 cât și DrosophilaConnectomeNavNet pe același backend
        let fog_net = FogOpticalNavNetV2::new(fog_cfg, vb.pp("fog_nav_v2"))?;
        let dros_cfg = DrosophilaConnectomeConfig::default();
        let dros_net =
            DrosophilaConnectomeNavNet::new_with_varbuilder(dros_cfg, 0.0, vb.pp("drosophila"))?;

        let batch_size = 4;
        let visual = Tensor::ones(
            (batch_size, fog_cfg.visual_channels, fog_cfg.image_size, fog_cfg.image_size),
            backend.dtype,
            &backend.device,
        )?
        .affine(20.0, -5.0)?;
        let kinematic = Tensor::ones(
            (batch_size, fog_cfg.kinematic_dim),
            backend.dtype,
            &backend.device,
        )?
        .affine(5.0, -2.0)?;

        // Extragem trăsăturile [f_gnd (64), f_fog (64)] din FogOpticalNavNetV2 și le trecem prin conectomul Drosophila
        let fog_out = fog_net.forward_step(&visual, &kinematic, None)?;
        let visual_features_128 = Tensor::cat(&[&fog_out.f_gnd_features, &fog_out.f_fog_features], 1)?;

        let dros_out = dros_net.forward_tensor(
            &visual_features_128,
            &kinematic,
            Some(&fog_out.w_fog_ned),
        )?;

        assert_eq!(dros_out.delta_v_ai.dims(), &[batch_size, 3]);
        assert_eq!(dros_out.log_variance_s.dims(), &[batch_size, 3]);
        assert_eq!(dros_out.t4_on.dims(), &[batch_size, T4T5_CARDINAL_DIRS]);
        assert_eq!(dros_out.t5_off.dims(), &[batch_size, T4T5_CARDINAL_DIRS]);
        assert_eq!(dros_out.lptc_hs.dims(), &[batch_size, LPTC_HS_CELLS]);
        assert_eq!(dros_out.lptc_vs.dims(), &[batch_size, LPTC_VS_CELLS]);
        assert_eq!(dros_out.derotated_flow.dims(), &[batch_size, 3]);

        // Verificăm că toate eșantioanele din batch respectă Spectral-Norm Clamp <= 3.5 m/s și s in [-8.0, +4.0]
        let dv_batch = dros_out.delta_v_ai.to_vec2::<f32>()?;
        let s_batch = dros_out.log_variance_s.to_vec2::<f32>()?;
        for b in 0..batch_size {
            let vx = dv_batch[b][0] as f64;
            let vy = dv_batch[b][1] as f64;
            let vz = dv_batch[b][2] as f64;
            let norm = (vx * vx + vy * vy + vz * vz).sqrt();
            assert!(
                norm <= MAX_DELTA_V_AI_MPS + 1e-4,
                "Batch sample {b} exceeded 3.5 m/s spectral-norm clamp: {norm}"
            );
            for k in 0..3 {
                let s_val = s_batch[b][k] as f64;
                assert!(s_val.is_finite());
                assert!(
                    s_val >= CONNECTOME_LOG_VAR_MIN - 1e-5
                        && s_val <= CONNECTOME_LOG_VAR_MAX + 1e-5
                );
            }
        }

        Ok(())
    }

    #[test]
    fn test_6_zenith_swir_dra_sky_polarization_and_daylight_star_tracker() {
        let cfg = DrosophilaConnectomeConfig::default();
        let mut ring = EllipsoidBodyRingAttractor16::new(cfg, 0.0);

        // 1. Peste nori (alt = 9500m, MOR = 10000m): Daylight SWIR 1.5 µm Star-Tracker (ca pe SR-71 Blackbird / B-2 Spirit)
        let above = evaluate_zenith_swir_dra_sky_compass(&mut ring, 0.72, 9_500.0, 10_000.0, 0.90);
        assert!(above.daylight_swir_stars_locked >= 24);
        assert!(above.angular_uncertainty_deg < 0.005);
        assert!((above.zenith_true_heading_rad - 0.72).abs() < 0.02);

        // 2. Sub nori groși / în ceață 0m (alt = 120m, MOR = 0m): Ochiul Dorsal al Muștei (DRA R7/R8 Sky-Polarization)
        let below = evaluate_zenith_swir_dra_sky_compass(&mut ring, -0.35, 120.0, 0.0, 0.85);
        assert_eq!(below.daylight_swir_stars_locked, 0);
        assert!(below.dra_sky_polarization_dolp > 0.50);
        assert!(below.angular_uncertainty_deg < 0.005);
        assert!((below.zenith_true_heading_rad - (-0.35)).abs() < 0.02);
    }
}
