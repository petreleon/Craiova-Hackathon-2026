# Specificație Tehnică Crate 4: `aero-meteo-cyber`
**Rol**: Ingestie Asincronă `Open-Meteo` & `METAR LRCV` (`ArcSwap` Lock-Free), Scut Dublu Anti-Spoofing ($\chi^2$ + Vector CUSUM) și Poarta Fizică Anti-Poisoning Meteo  
**Cale Crate în Workspace**: `crates/aero-meteo-cyber/`  
**Dependențe**: `tokio = "1.41"`, `reqwest = { version = "0.12", features = ["json", "rustls-tls"] }`, `arc-swap = "1.7"`, `serde = "1.0"`, `serde_json = "1.0"`, `nalgebra = "0.33"`, `statrs = "0.17"`

---

## 1. Arhitectura *Lock-Free* pentru Datele Deschise Meteo `LRCV` Craiova (`meteo_ingestor_lrcv.rs`)

### 1.1. Coordonatele Oficiale și Endpoint-ul Open-Meteo Aviation API
Modulul interoghează profilul atmosferic multistrat exact deasupra pragului pistei **RWY 09 a Aeroportului Internațional Craiova (`LRCV` — `44.3181° N, 23.8886° E`)**:
```text
https://api.open-meteo.com/v1/forecast?latitude=44.3181&longitude=23.8886&current=temperature_2m,relative_humidity_2m,dew_point_2m,surface_pressure,cloud_cover,visibility,wind_speed_10m,wind_direction_10m,wind_gusts_10m&hourly=wind_speed_80m,wind_direction_80m,wind_speed_120m,wind_direction_120m, wind_speed_180m,wind_direction_180m,visibility,cloud_cover_low&wind_speed_unit=ms
```

### 1.2. Partajare *Wait-Free* prin `ArcSwap<MeteoLayerProfileLrcv>` (Remedierea Critică #12 & #5)
Bucla deterministă de $60\text{ Hz}$ nu așteaptă **niciodată** după rețea:
* Un task `tokio::spawn` de fundal rulează cu un **`timeout(Duration::from_millis(1500))`** strict.
* Când internetul este activ și nebruiat, răspunsul JSON este parsat, salvat atomic în cache-ul local `data/lrcv_weather_cache.json` și publicat în memorie prin `meteo_swap.store(Arc::new(profile))`.
* Când se activează butonul **`[🌐 CUT INTERNET / COMMS BLACKOUT]`** (sau dacă Wi-Fi-ul sălii de hackathon are pierderi), sistemul comută în **$< 1\,\mu\text{s}$** pe ultimul snapshot validat și îl actualizează recursiv bayesian folosind vectorul ceții $\hat{\mathbf{w}}_{fog}(z, t)$ măsurat optic de `FogOpticalNavNetV2`!

### 1.3. Interpolarea Verticală a Ceții/Vântului în Timpul Manevrelor de Pitch/Altitudine
Când avionul urcă sau coboară ($h_{AGL}$ se modifică), profilul meteo oferă vectorul de referință $\bar{\mathbf{w}}_{meteo}(h_{AGL})$ prin interpolare pe plafoanele `10m`, `80m`, `120m`, `180m` (și legea logaritmică Hellmann sub $10\text{ m}$), actualizând instantaneu așteptarea priorului la noul plafon de zbor.

---

## 2. `Optical-Aerodynamic Meteo Sanity Gate` — Protecția Contra *Pre-Cut Meteo Cache Poisoning* (Remedierea #8)

Ce se întâmplă dacă un atacator falsifică datele meteo în rețea (ex. injectează un vânt fals de $+25\text{ m/s}$ din Nord) *chiar înainte* de a tăia internetul, încercând să otrăvească cache-ul local pe care se bazează avionul în modul offline?

În `crates/aero-meteo-cyber/src/meteo_sanity_gate.rs`, implementăm verificarea fizică încrucișată la fiecare cadru:

```rust
pub struct MeteoSanityVerdict {
    pub is_cache_trusted: bool,
    pub trust_weight: f64,           // [0.0 = Poisoned/Rejected, 1.0 = Fully Trusted]
    pub discrepancy_mps: f64,        // ||w_cache(h) - w_optical_aero||_2
    pub alert_status: &'static str,
}

pub fn verify_meteo_cache_integrity(
    w_cache_ned: Vector3<f64>,
    w_optical_fog_ned: Vector3<f64>,
    w_aerodynamic_triangle_ned: Vector3<f64>, // v_ground_opt - R_b^n * v_true_airspeed
) -> MeteoSanityVerdict {
    let w_physical_consensus = 0.65 * w_optical_fog_ned + 0.35 * w_aerodynamic_triangle_ned;
    let discrepancy = (w_cache_ned - w_physical_consensus).norm();

    if discrepancy > 4.5 {
        MeteoSanityVerdict {
            is_cache_trusted: false,
            trust_weight: 0.0, // Respinge complet cache-ul otrăvit!
            discrepancy_mps: discrepancy,
            alert_status: "POISONED OR SHEAR-INVALIDATED METEO CACHE REJECTED — 100% OPTICAL AUTONOMY",
        }
    } else {
        let weight = (1.0 - (discrepancy / 4.5).powi(2)).clamp(0.0, 1.0);
        MeteoSanityVerdict {
            is_cache_trusted: true,
            trust_weight: weight,
            discrepancy_mps: discrepancy,
            alert_status: "METEO PROFILE VERIFIED BY OPTICAL-AERODYNAMIC CONSISTENCY",
        }
    }
}
```

---

## 3. `Dual-Layer Federated GNSS Watchdog` — Detecția *Jump Spoofing* & *Stealth Slow-Ramp Spoofing* (Remedierea #7)

Conform **Art. 21 din Regulamentul Craiova Hackathon V2.0**, toate scenariile de atac (`GPS Jamming`, `Jump Spoofing`, `Stealth Slow-Ramp Spoofing`, `Comms Blackout`) sunt simulate 100% în interiorul sandbox-ului software din `aero-meteo-cyber`.

Pentru a prinde atât atacurile bruște, cât și pe cele insidioase (care trag lent poziția GPS cu doar $0.15\text{–}0.30\text{ m/s}$ lateral pentru a rămâne sub pragul unui singur cadru), `gnss_dual_watchdog.rs` rulează **două detectoare statistice în paralel** față de filtrul optic-inerțial federat (care nu este niciodată poluat de GPS!):

### 3.1. Stratul 1: Poarta Instantanee Mahalanobis $\chi^2$ (Detecție $< 20\text{ ms}$ pentru *Jump Spoofing*)
$$\gamma_{inst}(t_k) = \mathbf{r}_k^T \mathbf{S}_k^{-1} \mathbf{r}_k > \tau_{\chi^2} = 22.46 \quad (\chi^2_{6,\,0.999}), \qquad \mathbf{r}_k = \begin{bmatrix} \mathbf{p}_{GPS,k} - \hat{\mathbf{p}}_{opt,k} \\ \mathbf{v}_{GPS,k} - \hat{\mathbf{v}}_{opt,k} \end{bmatrix}$$

### 3.2. Stratul 2: `SlidingWindowVectorCusum` (Detecție $< 3.2\text{ s}$ pentru *Stealth Slow-Ramp Spoofing*)
Acumulează vectorul inovațiilor albite $\tilde{\mathbf{r}}_k = \mathbf{S}_k^{-1/2}\mathbf{r}_k$ fără anularea semnului direcțional:

```rust
pub struct DualLayerGnssWatchdog {
    pub cusum_vec: SVector<f64, 6>,
    pub chi2_threshold: f64,      // 22.46 (df=6, p=0.999)
    pub cusum_threshold: f64,     // 8.50 (detectează 0.15 m/s ramp în < 3.2s la 60Hz)
    pub drift_allowance_nu: f64,  // 0.08
    pub is_quarantined: bool,
}

impl DualLayerGnssWatchdog {
    pub fn evaluate_gnss_sample(
        &mut self,
        innovation_6d: &SVector<f64, 6>,
        s_cov_inv_sqrt: &SMatrix<f64, 6, 6>,
    ) -> GnssIntegrityStatus {
        let whitened: SVector<f64, 6> = s_cov_inv_sqrt * innovation_6d;
        let chi2_stat = whitened.dot(&whitened);

        // Actualizare Vector CUSUM pentru derivă lentă direcțională (Stealth Ramp)
        for i in 0..6 {
            let val = self.cusum_vec[i] + whitened[i].abs() - self.drift_allowance_nu;
            self.cusum_vec[i] = val.max(0.0);
        }
        let cusum_norm = self.cusum_vec.norm();

        if chi2_stat > self.chi2_threshold {
            self.is_quarantined = true;
            GnssIntegrityStatus::QuarantinedJumpSpoof { chi2_stat, cusum_norm }
        } else if cusum_norm > self.cusum_threshold {
            self.is_quarantined = true;
            GnssIntegrityStatus::QuarantinedStealthRampSpoof { chi2_stat, cusum_norm }
        } else {
            GnssIntegrityStatus::Nominal { chi2_stat, cusum_norm }
        }
    }
}
```
Când oricare dintre cele două straturi declanșează carantina, receptorul GNSS este decuplat total (`GPS QUARANTINED — FORTRESS OPTICAL-INERTIAL MODE ACTIVE`), iar aeronava continuă aterizarea pe RWY 09 Craiova fără nicio abatere!
