//! # Funcția de Cost Fizic-Informată (`HeteroscedasticPinnLoss`)
//!
//! Implementează funcția de pierdere combinată **Heteroscedastic Gaussian NLL + Inverse-Depth PINN Loss**
//! din specificația `arhitectura/02_aero_ai_cuda.md`:
//!
//! $$\mathcal{L}_{\text{total}} = \underbrace{\frac{1}{2}\sum_{k=1}^3 \left( e^{-s_{v,k}}(v_k - \hat{v}_k)^2 + s_{v,k} \right)}_{\text{Heteroscedastic Ground Velocity NLL}} + \underbrace{\frac{1}{2}\sum_{k=1}^3 \left( e^{-s_{w,k}}(w_{\text{fog},k} - \hat{w}_{\text{fog},k})^2 + s_{w,k} \right)}_{\text{Heteroscedastic Fog Wind NLL}} + \lambda_{\text{PINN}} \underbrace{\left\| \dot{\mathbf{u}}_{\text{trans}} - \hat{\dot{\mathbf{u}}}(\hat{\mathbf{v}}_b, \hat{\mathbf{w}}_{\text{fog}}, \rho_{\text{ground}}, \hat{\rho}_{\text{fog}}) \right\|_2^2}_{\text{Physics-Informed Inverse-Depth Consistency}} + \lambda_{\text{gust}}\,\text{BCE}(\hat{p}_{\text{gust}}, y_{\text{gust}}^*) + \lambda_{\text{ALS}}\left\|\hat{\mathbf{y}}_{\text{ALS}} - \mathbf{y}_{\text{ALS}}^*\right\|_2^2$$
//!
//! ## Garanții Numerice (Critica #9 & #11)
//! - Log-varianțele $s_{v,k}$ și $s_{w,k}$ sunt clampate în $[-10.0, 6.0]$ înainte de evaluarea
//!   exponențialei $e^{-s}$, garantând $e^{-s} \in [e^{-6.0}, e^{10.0}] \approx [0.00248, 22026.46]$.
//! - Probabilitatea $\hat{p}_{\text{gust}}$ în termenul Binary Cross-Entropy este clampată în
//!   $[10^{-6}, 1 - 10^{-6}]$ înainte de aplicarea logaritmului natural $\ln(\cdot)$.
//! - Inversa adâncimii solului $\rho_{\text{ground}} \ge 0$ și inversa adâncimii ceții $\hat{\rho}_{\text{fog}} > 0$
//!   apar exclusiv multiplicativ în ecuația fluxului optic translațional:
//!   $$\hat{\dot{\mathbf{u}}}_{\text{trans}} = \bar{\rho}_{\text{ground}} \odot \hat{\mathbf{v}}_{\text{ground}} + \hat{\rho}_{\text{fog}} \odot (\hat{\mathbf{v}}_{\text{ground}} - \hat{\mathbf{w}}_{\text{fog}})$$
//!   Astfel, nu există **nicio împărțire la zero** ($Z \to 0$ sau $Z \to \infty$ la orizont).

use crate::model_v2::{FogNavForwardTensors, LOG_VAR_MAX, LOG_VAR_MIN};
use crate::AeroAiError;
use candle_core::Tensor;
use serde::{Deserialize, Serialize};

/// Ponderile hiperparametrilor din `HeteroscedasticPinnLoss`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PinnLossConfig {
    /// Coeficientul $\lambda_{\text{PINN}}$ pentru reziduul fizic al fluxului optic în inversa adâncimii.
    pub lambda_pinn: f64,
    /// Coeficientul $\lambda_{\text{gust}}$ pentru clasificarea anticipativă a rafalelor (BCE).
    pub lambda_gust: f64,
    /// Coeficientul $\lambda_{\text{ALS}}$ pentru eroarea de aliniere pe balizajul luminos ALS RWY 09.
    pub lambda_als: f64,
}

impl Default for PinnLossConfig {
    fn default() -> Self {
        Self {
            lambda_pinn: 2.5,
            lambda_gust: 1.0,
            lambda_als: 0.5,
        }
    }
}

/// Țintele (*Ground Truth* + mărimi fizice observate) pentru un mini-batch de antrenare PINN.
#[derive(Debug, Clone)]
pub struct PinnBatchTargets {
    /// Viteza reală față de sol în reperul NED `[B, 3]` ($\text{m/s}$).
    pub v_ground_ned: Tensor,
    /// Viteza reală a masei de ceață/vânt în reperul NED `[B, 3]` ($\text{m/s}$).
    pub w_fog_ned: Tensor,
    /// Inversa adâncimii medii sub-orizont a solului $\bar{\rho}_{\text{ground}} \ge 0$ `[B, 3]` ($\text{m}^{-1}$).
    pub rho_ground_mean: Tensor,
    /// Inversa adâncimii efective a stratului de ceață $\hat{\rho}_{\text{fog}} \approx 3.0 / V_{\text{MOR}}$ `[B, 3]` ($\text{m}^{-1}$).
    pub rho_fog_scale: Tensor,
    /// Fluxul optic translațional derotat observat $\dot{\mathbf{u}}_{\text{trans}}$ `[B, 3]` ($\text{rad/s}$).
    pub u_dot_trans_obs: Tensor,
    /// Eticheta binară $[0, 1]$ pentru prezența unui front de rafală la $35\text{–}80\text{ m}$ în față `[B, 1]`.
    pub gust_label: Tensor,
    /// Eroarea reală față de axul pistei RWY 09 $(\Delta y_{\text{ALS}}^*, \Delta\psi_{\text{ALS}}^*)$ `[B, 2]`.
    pub als_offset: Tensor,
}

/// Descompunerea detaliată a pierderii pe componente pentru telemetria live din Cockpit HUD.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LossBreakdown {
    /// Valoarea totală a funcției de cost $\mathcal{L}_{\text{total}}$.
    pub total_loss: f64,
    /// Componenta Heteroscedastic Gaussian NLL pentru viteza la sol $\hat{\mathbf{v}}_{\text{ground}}$.
    pub vel_nll: f64,
    /// Componenta Heteroscedastic Gaussian NLL pentru viteza ceții $\hat{\mathbf{w}}_{\text{fog}}$.
    pub fog_nll: f64,
    /// Reziduul fizic Inverse-Depth PINN $\|\dot{\mathbf{u}}_{\text{trans}} - \hat{\dot{\mathbf{u}}}\|_2^2$.
    pub pinn_residual: f64,
    /// Eroarea Binary Cross-Entropy pentru detecția anticipativă a rafalei.
    pub gust_bce: f64,
    /// Eroarea pătratică medie pe ghidajul terminal ALS RWY 09.
    pub als_mse: f64,
    /// Eroarea pătratică medie (RMSE) a vitezei la sol în $\text{m/s}$.
    pub vel_rmse_mps: f64,
    /// Eroarea pătratică medie (RMSE) a vitezei ceții în $\text{m/s}$.
    pub fog_rmse_mps: f64,
}

/// Calculatorul funcției de pierdere fizic-informate `HeteroscedasticPinnLoss`.
#[derive(Debug, Clone, Copy)]
pub struct HeteroscedasticPinnLoss {
    config: PinnLossConfig,
}

impl Default for HeteroscedasticPinnLoss {
    fn default() -> Self {
        Self::new(PinnLossConfig::default())
    }
}

impl HeteroscedasticPinnLoss {
    /// Creează o nouă instanță `HeteroscedasticPinnLoss` cu ponderile specificate.
    #[inline]
    pub const fn new(config: PinnLossConfig) -> Self {
        Self { config }
    }

    /// Returnează configurația curentă a funcției de pierdere.
    #[inline]
    pub const fn config(&self) -> &PinnLossConfig {
        &self.config
    }

    /// Calculează tensorul diferențiabil `total_loss` (pentru `AdamW::backward_step`)
    /// împreună cu structura scalară `LossBreakdown` (garantată fără `NaN` / `Inf`).
    pub fn compute_total_loss(
        &self,
        preds: &FogNavForwardTensors,
        targets: &PinnBatchTargets,
    ) -> Result<(Tensor, LossBreakdown), AeroAiError> {
        // 1. Clampare defensivă a log-varianțelor în [-10.0, 6.0]
        let s_v = preds.s_v_log_var.clamp(LOG_VAR_MIN, LOG_VAR_MAX)?;
        let s_w = preds.s_w_log_var.clamp(LOG_VAR_MIN, LOG_VAR_MAX)?;

        // Clampare defensivă a erorilor pătratice pentru a preveni overflow-ul chiar și pe intrări extreme
        let diff_v = (&preds.v_ground_ned - &targets.v_ground_ned)?.clamp(-1e4f32, 1e4f32)?;
        let diff_v_sq = diff_v.sqr()?;

        let diff_w = (&preds.w_fog_ned - &targets.w_fog_ned)?.clamp(-1e4f32, 1e4f32)?;
        let diff_w_sq = diff_w.sqr()?;

        // 2. Heteroscedastic Ground Velocity NLL: 0.5 * sum_k (exp(-s_v,k) * (v_k - \hat{v}_k)^2 + s_v,k)
        let inv_var_v = s_v.affine(-1.0, 0.0)?.exp()?;
        let nll_v_terms = ((&inv_var_v * &diff_v_sq)? + &s_v)?.affine(0.5, 0.0)?;
        let vel_nll_tensor = nll_v_terms.sum(1)?.mean_all()?;

        // 3. Heteroscedastic Fog Velocity NLL: 0.5 * sum_k (exp(-s_w,k) * (w_k - \hat{w}_k)^2 + s_w,k)
        let inv_var_w = s_w.affine(-1.0, 0.0)?.exp()?;
        let nll_w_terms = ((&inv_var_w * &diff_w_sq)? + &s_w)?.affine(0.5, 0.0)?;
        let fog_nll_tensor = nll_w_terms.sum(1)?.mean_all()?;

        // 4. Physics-Informed Inverse-Depth Optical Flow Residual:
        // \hat{\dot{u}}_trans = \rho_ground * \hat{v}_ground + \hat{\rho}_fog * (\hat{v}_ground - \hat{w}_fog)
        let rho_gnd_safe = targets.rho_ground_mean.clamp(0.0f32, 10.0f32)?;
        let rho_fog_safe = targets.rho_fog_scale.clamp(0.0f32, 2.0f32)?;

        let rel_vel_pred = (&preds.v_ground_ned - &preds.w_fog_ned)?.clamp(-1e4f32, 1e4f32)?;
        let flow_gnd_pred = (&preds.v_ground_ned.clamp(-1e4f32, 1e4f32)? * &rho_gnd_safe)?;
        let flow_fog_pred = (&rel_vel_pred * &rho_fog_safe)?;
        let u_dot_pred = (&flow_gnd_pred + &flow_fog_pred)?;

        let u_dot_diff = (&targets.u_dot_trans_obs.clamp(-1e4f32, 1e4f32)? - &u_dot_pred)?;
        let pinn_residual_tensor = u_dot_diff.sqr()?.sum(1)?.mean_all()?;

        // 5. Gust Lookahead Binary Cross-Entropy (cu clampare numerică în [1e-6, 1 - 1e-6])
        let p_gust = preds.gust_prob.clamp(1e-6f32, 1.0 - 1e-6f32)?;
        let one_minus_p = p_gust.affine(-1.0, 1.0)?.clamp(1e-6f32, 1.0 - 1e-6f32)?;
        let y_gust = targets.gust_label.clamp(0.0f32, 1.0f32)?;
        let one_minus_y = y_gust.affine(-1.0, 1.0)?;

        let bce_pos = (&y_gust * &p_gust.log()?)?;
        let bce_neg = (&one_minus_y * &one_minus_p.log()?)?;
        let gust_bce_tensor = (&bce_pos + &bce_neg)?.affine(-1.0, 0.0)?.mean_all()?;

        // 6. Terminal ALS RWY 09 Guidance MSE
        let als_diff = (&preds.als_offset - &targets.als_offset)?.clamp(-1e4f32, 1e4f32)?;
        let als_mse_tensor = als_diff.sqr()?.sum(1)?.mean_all()?;

        // 7. Combinarea ponderată a pierderii totale
        let weighted_pinn = pinn_residual_tensor.affine(self.config.lambda_pinn, 0.0)?;
        let weighted_gust = gust_bce_tensor.affine(self.config.lambda_gust, 0.0)?;
        let weighted_als = als_mse_tensor.affine(self.config.lambda_als, 0.0)?;

        let total_loss_tensor = (&(&(&vel_nll_tensor + &fog_nll_tensor)? + &weighted_pinn)?
            + &(&weighted_gust + &weighted_als)?)?;

        // Extragem metricile scalare pentru telemetrie și verificăm finitudinea
        let vel_mse = diff_v_sq.mean_all()?.to_scalar::<f32>()? as f64;
        let fog_mse = diff_w_sq.mean_all()?.to_scalar::<f32>()? as f64;

        let total_loss_val = total_loss_tensor.to_scalar::<f32>()? as f64;
        let vel_nll_val = vel_nll_tensor.to_scalar::<f32>()? as f64;
        let fog_nll_val = fog_nll_tensor.to_scalar::<f32>()? as f64;
        let pinn_res_val = pinn_residual_tensor.to_scalar::<f32>()? as f64;
        let gust_bce_val = gust_bce_tensor.to_scalar::<f32>()? as f64;
        let als_mse_val = als_mse_tensor.to_scalar::<f32>()? as f64;

        if !total_loss_val.is_finite() {
            return Err(AeroAiError::NonFiniteValue {
                context: "HeteroscedasticPinnLoss::total_loss",
            });
        }

        let breakdown = LossBreakdown {
            total_loss: total_loss_val,
            vel_nll: vel_nll_val,
            fog_nll: fog_nll_val,
            pinn_residual: pinn_res_val,
            gust_bce: gust_bce_val,
            als_mse: als_mse_val,
            vel_rmse_mps: vel_mse.max(0.0).sqrt(),
            fog_rmse_mps: fog_mse.max(0.0).sqrt(),
        };

        Ok((total_loss_tensor, breakdown))
    }
}
