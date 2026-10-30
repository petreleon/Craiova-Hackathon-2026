//! Lock-Free Open-Meteo Aviation API Ingestor for LRCV (Craiova International Airport RWY 09).
//!
//! Provides wait-free `< 1 us` access to the multi-altitude atmospheric profile over
//! RWY 09 (`44.3181 N, 23.8886 E`) via `ArcSwap<MeteoLayerProfileLrcv>`, non-blocking
//! background fetch with strict `1500 ms` timeout, local JSON cache persistence,
//! Hellmann boundary-layer interpolation below `10 m AGL`, and recursive Bayesian
//! profile updating during `CUT INTERNET / COMMS BLACKOUT` using optical fog measurements.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Official WGS-84 latitude of Craiova International Airport (`LRCV`) RWY 09 threshold (degrees North).
pub const LAT_LRCV_RWY09: f64 = 44.3181_f64;

/// Official WGS-84 longitude of Craiova International Airport (`LRCV`) RWY 09 threshold (degrees East).
pub const LON_LRCV_RWY09: f64 = 23.8886_f64;

/// Official elevation MSL of Craiova International Airport (`LRCV`) RWY 09 threshold (meters).
pub const ALT_LRCV_MSL: f64 = 191.0_f64;

/// True heading of RWY 09 at Craiova International Airport (`LRCV`) in degrees.
pub const RWY09_HEADING_DEG: f64 = 87.0_f64;

/// Hellmann exponent for open airport runway terrain (`alpha ~ 1/7 = 0.143`).
pub const HELLMANN_ALPHA_RUNWAY: f64 = 0.143_f64;

/// Strict HTTP timeout for Open-Meteo Aviation API requests (`1500 ms`) so background tasks never stall.
pub const OPEN_METEO_TIMEOUT_MS: u64 = 1500;

/// Default local cache file path for LRCV weather snapshots.
pub const DEFAULT_LRCV_CACHE_PATH: &str = "data/lrcv_weather_cache.json";

/// Exact Open-Meteo Aviation API endpoint for LRCV RWY 09 (`44.3181° N, 23.8886° E`).
pub const OPEN_METEO_LRCV_URL: &str = "https://api.open-meteo.com/v1/forecast?latitude=44.3181&longitude=23.8886&current=temperature_2m,relative_humidity_2m,dew_point_2m,surface_pressure,cloud_cover,visibility,wind_speed_10m,wind_direction_10m,wind_gusts_10m&hourly=wind_speed_80m,wind_direction_80m,wind_speed_120m,wind_direction_120m,wind_speed_180m,wind_direction_180m,visibility,cloud_cover_low&wind_speed_unit=ms";

/// Errors emitted by the LRCV Open-Meteo ingestor and cache subsystem.
#[derive(Debug, Error)]
pub enum MeteoIngestError {
    #[error("Comms Blackout active (CUT INTERNET mode enabled) — live network fetch blocked")]
    CommsBlackoutActive,
    #[error("Open-Meteo HTTP request timed out after {0} ms")]
    Timeout(u64),
    #[error("HTTP client or transport error: {0}")]
    Http(String),
    #[error("JSON serialization or deserialization failed: {0}")]
    Json(String),
    #[error("Local cache file I/O error: {0}")]
    Io(String),
}

/// Raw JSON response payload from Open-Meteo Aviation API for LRCV.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenMeteoResponse {
    pub latitude: f64,
    pub longitude: f64,
    #[serde(default = "default_elevation_lrcv")]
    pub elevation: f64,
    pub current: OpenMeteoCurrentBlock,
    #[serde(default)]
    pub hourly: Option<OpenMeteoHourlyBlock>,
}

fn default_elevation_lrcv() -> f64 {
    ALT_LRCV_MSL
}

/// `current` weather block from Open-Meteo Aviation API (`wind_speed_unit=ms`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenMeteoCurrentBlock {
    #[serde(default)]
    pub time: String,
    pub temperature_2m: f64,
    pub relative_humidity_2m: f64,
    pub dew_point_2m: f64,
    pub surface_pressure: f64,
    pub cloud_cover: f64,
    pub visibility: f64,
    pub wind_speed_10m: f64,
    pub wind_direction_10m: f64,
    pub wind_gusts_10m: f64,
}

/// `hourly` multi-altitude wind and visibility block from Open-Meteo Aviation API (`80m`, `120m`, `180m`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpenMeteoHourlyBlock {
    #[serde(default)]
    pub time: Vec<String>,
    #[serde(default)]
    pub wind_speed_80m: Vec<f64>,
    #[serde(default)]
    pub wind_direction_80m: Vec<f64>,
    #[serde(default)]
    pub wind_speed_120m: Vec<f64>,
    #[serde(default)]
    pub wind_direction_120m: Vec<f64>,
    #[serde(default)]
    pub wind_speed_180m: Vec<f64>,
    #[serde(default)]
    pub wind_direction_180m: Vec<f64>,
    #[serde(default)]
    pub visibility: Vec<f64>,
    #[serde(default)]
    pub cloud_cover_low: Vec<f64>,
}

/// Wind and fog state at a specific altitude ceiling above ground level (`h_agl_m`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AltitudeWindLayer {
    /// Altitude above ground level (meters), e.g., `10.0`, `80.0`, `120.0`, `180.0`.
    pub altitude_agl_m: f64,
    /// Scalar wind speed in meters per second (`>= 0.0`).
    pub speed_mps: f64,
    /// Meteorological wind direction in degrees (`[0, 360)`), indicating the direction FROM which wind blows.
    pub direction_from_deg: f64,
    /// 3D wind velocity vector in local NED (`North-East-Down`) frame (`m/s`).
    pub wind_ned_mps: [f64; 3],
    /// Prior variance `(m/s)^2` associated with this altitude layer (used for Bayesian recursive updates).
    pub variance_mps2: f64,
}

impl AltitudeWindLayer {
    /// Constructs a new `AltitudeWindLayer` from meteorological speed and direction.
    pub fn from_met(
        altitude_agl_m: f64,
        speed_mps: f64,
        direction_from_deg: f64,
        vertical_down_mps: f64,
        variance_mps2: f64,
    ) -> Self {
        let ned = met_wind_to_ned_vector(speed_mps, direction_from_deg, vertical_down_mps);
        Self {
            altitude_agl_m: altitude_agl_m.max(0.5),
            speed_mps: speed_mps.max(0.0),
            direction_from_deg: direction_from_deg.rem_euclid(360.0),
            wind_ned_mps: [ned.x, ned.y, ned.z],
            variance_mps2: variance_mps2.max(1e-4),
        }
    }

    /// Returns the 3D wind vector in local `NED` coordinates (`m/s`).
    #[inline]
    pub fn wind_ned(&self) -> Vector3<f64> {
        Vector3::new(
            self.wind_ned_mps[0],
            self.wind_ned_mps[1],
            self.wind_ned_mps[2],
        )
    }
}

/// Multi-layer atmospheric and fog profile over Craiova `LRCV` RWY 09 shared lock-free via `ArcSwap`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeteoLayerProfileLrcv {
    /// Station ICAO code (`"LRCV"`).
    pub station_icao: String,
    /// ISO-8601 or UTC timestamp identifier of the snapshot.
    pub timestamp_utc: String,
    /// WGS-84 latitude (`44.3181`).
    pub latitude_deg: f64,
    /// WGS-84 longitude (`23.8886`).
    pub longitude_deg: f64,
    /// Runway 09 elevation MSL (`191.0 m`).
    pub elevation_msl_m: f64,
    /// Ambient temperature at `2 m AGL` (Celsius).
    pub temperature_2m_c: f64,
    /// Dew point temperature at `2 m AGL` (Celsius).
    pub dew_point_2m_c: f64,
    /// Relative humidity at `2 m AGL` (`0..100 %`).
    pub relative_humidity_pct: f64,
    /// QNH / surface pressure in `hPa`.
    pub surface_pressure_hpa: f64,
    /// Total cloud cover (`0..100 %`).
    pub cloud_cover_pct: f64,
    /// Low-ceiling cloud cover (`0..100 %`).
    pub cloud_cover_low_pct: f64,
    /// Meteorological Optical Range visibility $V_{MOR}$ in meters.
    pub visibility_mor_m: f64,
    /// Koschmieder optical extinction coefficient $\hat{\beta}_{ext} = \hat{\rho}_{fog, opt} \approx 3.0 / V_{MOR}$ (`m^-1`).
    pub koschmieder_beta_ext_inv_m: f64,
    /// Wind gusts at `10 m AGL` in `m/s`.
    pub wind_gusts_10m_mps: f64,
    /// Ordered altitude wind layers (`10 m`, `80 m`, `120 m`, `180 m`).
    pub layers: [AltitudeWindLayer; 4],
    /// Indicates whether this profile is currently operating in `COMMS BLACKOUT` Bayesian-updated mode.
    pub is_comms_blackout_mode: bool,
    /// Number of recursive Bayesian optical updates applied during blackout.
    pub bayesian_update_count: u64,
}

/// Converts meteorological wind `(speed_mps, direction_from_deg)` into a 3D `NED` velocity vector.
///
/// In meteorological convention, a wind direction of $\theta_{met} = 0^\circ$ (North) means the
/// air mass blows **from** the North **toward** the South:
/// $$v_N = -V \cos(\theta_{met}), \quad v_E = -V \sin(\theta_{met}), \quad v_D = w_D$$
#[inline]
pub fn met_wind_to_ned_vector(
    speed_mps: f64,
    direction_from_deg: f64,
    vertical_down_mps: f64,
) -> Vector3<f64> {
    let clean_speed = if speed_mps.is_finite() {
        speed_mps.max(0.0)
    } else {
        0.0
    };
    let clean_dir_rad = if direction_from_deg.is_finite() {
        direction_from_deg.rem_euclid(360.0).to_radians()
    } else {
        0.0
    };
    let clean_down = if vertical_down_mps.is_finite() {
        vertical_down_mps
    } else {
        0.0
    };

    let v_north = -clean_speed * clean_dir_rad.cos();
    let v_east = -clean_speed * clean_dir_rad.sin();
    Vector3::new(v_north, v_east, clean_down)
}

/// Converts a 3D `NED` wind vector back into `(speed_mps, direction_from_deg)`.
#[inline]
pub fn ned_vector_to_met_wind(wind_ned: &Vector3<f64>) -> (f64, f64) {
    let speed_horiz = (wind_ned.x * wind_ned.x + wind_ned.y * wind_ned.y).sqrt();
    if speed_horiz < 1e-9 {
        return (0.0, 0.0);
    }
    // Since v_N = -V cos(theta) and v_E = -V sin(theta), atan2(-v_E, -v_N) yields theta_from.
    let dir_rad = (-wind_ned.y).atan2(-wind_ned.x);
    let dir_deg = dir_rad.to_degrees().rem_euclid(360.0);
    (speed_horiz, dir_deg)
}

impl MeteoLayerProfileLrcv {
    /// Returns the deterministic CAT IIIc Craiova (`LRCV`) fallback safety profile.
    ///
    /// Used at cold-start before network response or when both network and local disk cache are unavailable.
    pub fn default_cat_iiic_lrcv() -> Self {
        let visibility_mor_m = 150.0;
        let koschmieder_beta_ext_inv_m = (-0.05_f64.ln()) / visibility_mor_m;

        Self {
            station_icao: "LRCV".to_string(),
            timestamp_utc: "2026-10-30T14:30:00Z".to_string(),
            latitude_deg: LAT_LRCV_RWY09,
            longitude_deg: LON_LRCV_RWY09,
            elevation_msl_m: ALT_LRCV_MSL,
            temperature_2m_c: 4.0,
            dew_point_2m_c: 4.0,
            relative_humidity_pct: 100.0,
            surface_pressure_hpa: 1022.0,
            cloud_cover_pct: 100.0,
            cloud_cover_low_pct: 100.0,
            visibility_mor_m,
            koschmieder_beta_ext_inv_m,
            wind_gusts_10m_mps: 5.2,
            layers: [
                AltitudeWindLayer::from_met(10.0, 3.1, 90.0, 0.0, 1.25),
                AltitudeWindLayer::from_met(80.0, 6.4, 102.0, 0.05, 1.80),
                AltitudeWindLayer::from_met(120.0, 8.2, 110.0, 0.08, 2.25),
                AltitudeWindLayer::from_met(180.0, 10.5, 118.0, 0.10, 2.80),
            ],
            is_comms_blackout_mode: false,
            bayesian_update_count: 0,
        }
    }

    /// Builds a `MeteoLayerProfileLrcv` from a parsed `OpenMeteoResponse`.
    pub fn from_open_meteo_response(resp: &OpenMeteoResponse, hour_index: usize) -> Self {
        let cur = &resp.current;
        let vis_m = cur.visibility.clamp(12.0, 50_000.0);
        let beta_ext = (-0.05_f64.ln()) / vis_m;

        let extract_hourly = |vec_opt: Option<&Vec<f64>>, fallback: f64| -> f64 {
            vec_opt
                .and_then(|v| v.get(hour_index).or_else(|| v.first()).copied())
                .filter(|val| val.is_finite())
                .unwrap_or(fallback)
        };

        let hourly_ref = resp.hourly.as_ref();
        let ws_10 = cur.wind_speed_10m.max(0.0);
        let wd_10 = cur.wind_direction_10m.rem_euclid(360.0);

        let ws_80 = extract_hourly(hourly_ref.map(|h| &h.wind_speed_80m), ws_10 * 1.35);
        let wd_80 = extract_hourly(hourly_ref.map(|h| &h.wind_direction_80m), wd_10);

        let ws_120 = extract_hourly(hourly_ref.map(|h| &h.wind_speed_120m), ws_80 * 1.15);
        let wd_120 = extract_hourly(hourly_ref.map(|h| &h.wind_direction_120m), wd_80);

        let ws_180 = extract_hourly(hourly_ref.map(|h| &h.wind_speed_180m), ws_120 * 1.15);
        let wd_180 = extract_hourly(hourly_ref.map(|h| &h.wind_direction_180m), wd_120);

        let cloud_low = extract_hourly(hourly_ref.map(|h| &h.cloud_cover_low), cur.cloud_cover);

        Self {
            station_icao: "LRCV".to_string(),
            timestamp_utc: if cur.time.is_empty() {
                "2026-10-30T14:30:00Z".to_string()
            } else {
                cur.time.clone()
            },
            latitude_deg: resp.latitude,
            longitude_deg: resp.longitude,
            elevation_msl_m: resp.elevation,
            temperature_2m_c: cur.temperature_2m,
            dew_point_2m_c: cur.dew_point_2m,
            relative_humidity_pct: cur.relative_humidity_2m.clamp(0.0, 100.0),
            surface_pressure_hpa: cur.surface_pressure.clamp(850.0, 1085.0),
            cloud_cover_pct: cur.cloud_cover.clamp(0.0, 100.0),
            cloud_cover_low_pct: cloud_low.clamp(0.0, 100.0),
            visibility_mor_m: vis_m,
            koschmieder_beta_ext_inv_m: beta_ext,
            wind_gusts_10m_mps: cur.wind_gusts_10m.max(ws_10),
            layers: [
                AltitudeWindLayer::from_met(10.0, ws_10, wd_10, 0.0, 1.25),
                AltitudeWindLayer::from_met(80.0, ws_80, wd_80, 0.0, 1.80),
                AltitudeWindLayer::from_met(120.0, ws_120, wd_120, 0.0, 2.25),
                AltitudeWindLayer::from_met(180.0, ws_180, wd_180, 0.0, 2.80),
            ],
            is_comms_blackout_mode: false,
            bayesian_update_count: 0,
        }
    }

    /// Interpolates the 3D wind/fog vector in `NED` coordinates (`m/s`) at altitude `h_agl_m` above ground.
    ///
    /// - Below `10 m AGL`: applies the Hellmann boundary-layer power law:
    ///   $$\mathbf{w}(h) = \mathbf{w}_{10m} \cdot \left(\frac{\max(h, 0.5)}{10.0}\right)^{\alpha_{hellmann}}$$
    /// - Between `10 m`, `80 m`, `120 m`, and `180 m`: performs continuous linear interpolation across ceilings.
    /// - Above `180 m`: applies gentle upper-layer power scaling capped at `500 m AGL`.
    pub fn interpolate_wind_ned_at_agl(&self, h_agl_m: f64) -> Vector3<f64> {
        let h_safe = if h_agl_m.is_finite() {
            h_agl_m.max(0.5)
        } else {
            10.0
        };

        let l0 = &self.layers[0]; // 10m
        let l1 = &self.layers[1]; // 80m
        let l2 = &self.layers[2]; // 120m
        let l3 = &self.layers[3]; // 180m

        if h_safe <= l0.altitude_agl_m {
            let ratio = (h_safe / l0.altitude_agl_m.max(0.5)).clamp(0.05, 1.0);
            let scale = ratio.powf(HELLMANN_ALPHA_RUNWAY);
            return l0.wind_ned() * scale;
        }

        if h_safe <= l1.altitude_agl_m {
            let span = (l1.altitude_agl_m - l0.altitude_agl_m).max(1e-6);
            let alpha = ((h_safe - l0.altitude_agl_m) / span).clamp(0.0, 1.0);
            return l0.wind_ned() * (1.0 - alpha) + l1.wind_ned() * alpha;
        }

        if h_safe <= l2.altitude_agl_m {
            let span = (l2.altitude_agl_m - l1.altitude_agl_m).max(1e-6);
            let alpha = ((h_safe - l1.altitude_agl_m) / span).clamp(0.0, 1.0);
            return l1.wind_ned() * (1.0 - alpha) + l2.wind_ned() * alpha;
        }

        if h_safe <= l3.altitude_agl_m {
            let span = (l3.altitude_agl_m - l2.altitude_agl_m).max(1e-6);
            let alpha = ((h_safe - l2.altitude_agl_m) / span).clamp(0.0, 1.0);
            return l2.wind_ned() * (1.0 - alpha) + l3.wind_ned() * alpha;
        }

        // Above 180m AGL: weak boundary-layer shear up to 500m AGL
        let h_clamped = h_safe.min(500.0);
        let ratio = (h_clamped / l3.altitude_agl_m.max(1.0)).max(1.0);
        let scale = ratio.powf(0.07);
        l3.wind_ned() * scale
    }

    /// Performs a recursive Bayesian update of the atmospheric wind layers using the optical fog
    /// velocity $\hat{\mathbf{w}}_{fog}(z, t)$ measured onboard by `FogOpticalNavNetV2`.
    ///
    /// Used when `CUT INTERNET / COMMS BLACKOUT` is active so the altitude wind profile evolves
    /// dynamically with real-time optical shear measurements.
    pub fn bayesian_recursive_fog_update(
        &self,
        h_agl_m: f64,
        w_optical_fog_ned: Vector3<f64>,
        optical_variance_mps2: f64,
    ) -> Self {
        let mut updated = self.clone();
        let h_safe = if h_agl_m.is_finite() {
            h_agl_m.clamp(0.5, 500.0)
        } else {
            10.0
        };
        let r_meas = if optical_variance_mps2.is_finite() {
            optical_variance_mps2.max(1e-3)
        } else {
            1.0
        };

        // Vertical correlation length scale for boundary-layer shear coupling (65 meters).
        let vertical_corr_scale_m = 65.0_f64;

        for layer in &mut updated.layers {
            let dz = (layer.altitude_agl_m - h_safe).abs();
            let spatial_coupling = (-(dz * dz)
                / (2.0 * vertical_corr_scale_m * vertical_corr_scale_m))
                .exp()
                .clamp(0.0, 1.0);

            if spatial_coupling > 1e-3 {
                // Adjust effective measurement variance by spatial distance from aircraft altitude
                let r_eff = r_meas / spatial_coupling.max(1e-3);
                // Add slight process noise floor to prevent covariance collapse during prolonged blackout
                let p_prior = (layer.variance_mps2 + 0.015).min(25.0);
                let kalman_gain = p_prior / (p_prior + r_eff).max(1e-9);

                // Project optical measurement to the target layer altitude using Hellmann ratio if below 10m
                let altitude_scale = if h_safe < 10.0 && layer.altitude_agl_m >= 10.0 {
                    (10.0 / h_safe.max(0.5)).powf(HELLMANN_ALPHA_RUNWAY)
                } else {
                    1.0
                };
                let target_meas_ned = w_optical_fog_ned * altitude_scale;

                let prior_ned = layer.wind_ned();
                let fused_ned = prior_ned + kalman_gain * (target_meas_ned - prior_ned);
                let (new_speed, new_dir) = ned_vector_to_met_wind(&fused_ned);

                layer.wind_ned_mps = [fused_ned.x, fused_ned.y, fused_ned.z];
                layer.speed_mps = new_speed;
                layer.direction_from_deg = new_dir;
                layer.variance_mps2 = ((1.0 - kalman_gain) * p_prior).clamp(1e-3, 25.0);
            }
        }

        updated.is_comms_blackout_mode = true;
        updated.bayesian_update_count = updated.bayesian_update_count.saturating_add(1);
        updated
    }
}

/// Wait-Free / Lock-Free Meteo Ingestor for Craiova (`LRCV`) using `ArcSwap<MeteoLayerProfileLrcv>`.
///
/// Guarantees that the `60 Hz` deterministic flight navigation loop reads the current atmospheric
/// snapshot in `< 1 us` without any mutex contention or network stall.
pub struct LrcvMeteoIngestor {
    profile_swap: Arc<ArcSwap<MeteoLayerProfileLrcv>>,
    comms_blackout: Arc<AtomicBool>,
    cache_path: PathBuf,
    api_url: String,
}

impl LrcvMeteoIngestor {
    /// Creates a new `LrcvMeteoIngestor`, loading from local JSON cache if available or falling back
    /// to the deterministic CAT IIIc Craiova safety profile.
    pub fn new<P: AsRef<Path>>(cache_path: P) -> Self {
        let path_buf = cache_path.as_ref().to_path_buf();
        let initial_profile = Self::load_profile_from_disk(&path_buf)
            .unwrap_or_else(|_| MeteoLayerProfileLrcv::default_cat_iiic_lrcv());

        Self {
            profile_swap: Arc::new(ArcSwap::from_pointee(initial_profile)),
            comms_blackout: Arc::new(AtomicBool::new(false)),
            cache_path: path_buf,
            api_url: OPEN_METEO_LRCV_URL.to_string(),
        }
    }

    /// Creates a new `LrcvMeteoIngestor` with the default cache path (`data/lrcv_weather_cache.json`).
    pub fn with_default_cache() -> Self {
        Self::new(DEFAULT_LRCV_CACHE_PATH)
    }

    /// Wait-free `< 1 us` read of the active `MeteoLayerProfileLrcv` snapshot.
    #[inline]
    pub fn load_snapshot(&self) -> Arc<MeteoLayerProfileLrcv> {
        self.profile_swap.load_full()
    }

    /// Atomically stores a new `MeteoLayerProfileLrcv` snapshot in memory.
    #[inline]
    pub fn store_snapshot(&self, mut profile: MeteoLayerProfileLrcv) {
        profile.is_comms_blackout_mode = self.is_comms_blackout();
        self.profile_swap.store(Arc::new(profile));
    }

    /// Returns a clone of the underlying `Arc<ArcSwap<MeteoLayerProfileLrcv>>` for zero-lock sharing across threads.
    #[inline]
    pub fn shared_swap(&self) -> Arc<ArcSwap<MeteoLayerProfileLrcv>> {
        Arc::clone(&self.profile_swap)
    }

    /// Enables or disables `CUT INTERNET / COMMS BLACKOUT` mode in `< 1 us`.
    pub fn set_comms_blackout(&self, blackout_enabled: bool) {
        self.comms_blackout
            .store(blackout_enabled, Ordering::SeqCst);
        let current = self.profile_swap.load_full();
        if current.is_comms_blackout_mode != blackout_enabled {
            let mut updated = (*current).clone();
            updated.is_comms_blackout_mode = blackout_enabled;
            self.profile_swap.store(Arc::new(updated));
        }
    }

    /// Returns `true` if `CUT INTERNET / COMMS BLACKOUT` mode is currently active.
    #[inline]
    pub fn is_comms_blackout(&self) -> bool {
        self.comms_blackout.load(Ordering::SeqCst)
    }

    /// Interpolates the 3D wind vector in `NED` coordinates at `h_agl_m` from the active lock-free snapshot.
    #[inline]
    pub fn interpolate_wind_ned_at_agl(&self, h_agl_m: f64) -> Vector3<f64> {
        self.profile_swap
            .load()
            .interpolate_wind_ned_at_agl(h_agl_m)
    }

    /// Performs a lock-free recursive Bayesian update of the cached profile using optical fog measurements.
    pub fn update_from_optical_fog(
        &self,
        h_agl_m: f64,
        w_optical_fog_ned: Vector3<f64>,
        optical_variance_mps2: f64,
    ) -> Arc<MeteoLayerProfileLrcv> {
        let current = self.profile_swap.load_full();
        let updated = current.bayesian_recursive_fog_update(
            h_agl_m,
            w_optical_fog_ned,
            optical_variance_mps2,
        );
        let arc_updated = Arc::new(updated);
        self.profile_swap.store(Arc::clone(&arc_updated));
        arc_updated
    }

    /// Saves the given profile to the local JSON cache file (`data/lrcv_weather_cache.json`).
    pub fn save_profile_to_disk(
        &self,
        profile: &MeteoLayerProfileLrcv,
    ) -> Result<(), MeteoIngestError> {
        if let Some(parent) = self.cache_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| MeteoIngestError::Io(e.to_string()))?;
            }
        }
        let json_bytes = serde_json::to_vec_pretty(profile)
            .map_err(|e| MeteoIngestError::Json(e.to_string()))?;
        std::fs::write(&self.cache_path, json_bytes)
            .map_err(|e| MeteoIngestError::Io(e.to_string()))?;
        Ok(())
    }

    /// Loads a cached `MeteoLayerProfileLrcv` from disk.
    pub fn load_profile_from_disk(path: &Path) -> Result<MeteoLayerProfileLrcv, MeteoIngestError> {
        let bytes = std::fs::read(path).map_err(|e| MeteoIngestError::Io(e.to_string()))?;
        serde_json::from_slice::<MeteoLayerProfileLrcv>(&bytes)
            .map_err(|e| MeteoIngestError::Json(e.to_string()))
    }

    /// Asynchronously fetches the latest LRCV weather snapshot from Open-Meteo Aviation API with a strict
    /// `1500 ms` timeout. Never blocks the `60 Hz` real-time loop.
    ///
    /// If `Comms Blackout` is active or the network times out, returns the appropriate error while keeping
    /// the last validated snapshot active in `ArcSwap`.
    pub async fn fetch_live_open_meteo_once(
        &self,
    ) -> Result<Arc<MeteoLayerProfileLrcv>, MeteoIngestError> {
        if self.is_comms_blackout() {
            return Err(MeteoIngestError::CommsBlackoutActive);
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(OPEN_METEO_TIMEOUT_MS))
            .build()
            .map_err(|e| MeteoIngestError::Http(e.to_string()))?;

        let request_fut = async {
            let response = client
                .get(&self.api_url)
                .send()
                .await
                .map_err(|e| MeteoIngestError::Http(e.to_string()))?;

            let body_text = response
                .text()
                .await
                .map_err(|e| MeteoIngestError::Http(e.to_string()))?;

            let parsed: OpenMeteoResponse = serde_json::from_str(&body_text)
                .map_err(|e| MeteoIngestError::Json(e.to_string()))?;

            Ok::<OpenMeteoResponse, MeteoIngestError>(parsed)
        };

        let parsed_resp = tokio::time::timeout(
            Duration::from_millis(OPEN_METEO_TIMEOUT_MS),
            request_fut,
        )
        .await
        .map_err(|_| MeteoIngestError::Timeout(OPEN_METEO_TIMEOUT_MS))??;

        let profile = MeteoLayerProfileLrcv::from_open_meteo_response(&parsed_resp, 0);
        let _ = self.save_profile_to_disk(&profile);
        let arc_profile = Arc::new(profile);
        self.profile_swap.store(Arc::clone(&arc_profile));

        Ok(arc_profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_met_wind_to_ned_conversion() {
        // Wind FROM North (0 deg) at 10 m/s blows TOWARD South: v_N = -10, v_E = 0
        let v_north_wind = met_wind_to_ned_vector(10.0, 0.0, 0.0);
        assert!((v_north_wind.x - (-10.0)).abs() < 1e-9);
        assert!(v_north_wind.y.abs() < 1e-9);

        // Wind FROM East (90 deg) at 6 m/s blows TOWARD West: v_N = 0, v_E = -6
        let v_east_wind = met_wind_to_ned_vector(6.0, 90.0, 0.0);
        assert!(v_east_wind.x.abs() < 1e-9);
        assert!((v_east_wind.y - (-6.0)).abs() < 1e-9);

        // Round-trip conversion back to meteorological speed and direction
        let (spd, dir) = ned_vector_to_met_wind(&v_east_wind);
        assert!((spd - 6.0).abs() < 1e-9);
        assert!((dir - 90.0).abs() < 1e-9);
    }

    #[test]
    fn test_vertical_interpolation_and_hellmann_law() {
        let profile = MeteoLayerProfileLrcv::default_cat_iiic_lrcv();

        // Exact ceiling matches at 10m, 80m, 120m, 180m
        let w_10 = profile.interpolate_wind_ned_at_agl(10.0);
        let w_80 = profile.interpolate_wind_ned_at_agl(80.0);
        let w_120 = profile.interpolate_wind_ned_at_agl(120.0);
        let w_180 = profile.interpolate_wind_ned_at_agl(180.0);

        assert!((w_10.norm() - 3.1).abs() < 1e-6);
        assert!((w_80 - profile.layers[1].wind_ned()).norm() < 1e-9);
        assert!((w_120 - profile.layers[2].wind_ned()).norm() < 1e-9);
        assert!((w_180 - profile.layers[3].wind_ned()).norm() < 1e-9);

        // Hellmann power law below 10m (e.g. at 2.0m flare height)
        let w_2m = profile.interpolate_wind_ned_at_agl(2.0);
        let expected_scale = (2.0_f64 / 10.0_f64).powf(HELLMANN_ALPHA_RUNWAY);
        assert!((w_2m.norm() - 3.1 * expected_scale).abs() < 1e-6);
        assert!(w_2m.norm() < w_10.norm());

        // Midpoint between 80m and 120m (100m AGL)
        let w_100 = profile.interpolate_wind_ned_at_agl(100.0);
        let expected_100 = 0.5 * (w_80 + w_120);
        assert!((w_100 - expected_100).norm() < 1e-9);
    }

    #[test]
    fn test_open_meteo_json_parsing_lrcv() {
        let sample_json = r#"{
            "latitude": 44.3181,
            "longitude": 23.8886,
            "elevation": 191.0,
            "current": {
                "time": "2026-10-30T14:30",
                "temperature_2m": 3.8,
                "relative_humidity_2m": 99.0,
                "dew_point_2m": 3.7,
                "surface_pressure": 1021.5,
                "cloud_cover": 100.0,
                "visibility": 140.0,
                "wind_speed_10m": 3.5,
                "wind_direction_10m": 88.0,
                "wind_gusts_10m": 5.8
            },
            "hourly": {
                "time": ["2026-10-30T14:00"],
                "wind_speed_80m": [6.8],
                "wind_direction_80m": [95.0],
                "wind_speed_120m": [8.9],
                "wind_direction_120m": [105.0],
                "wind_speed_180m": [11.4],
                "wind_direction_180m": [112.0],
                "visibility": [140.0],
                "cloud_cover_low": [100.0]
            }
        }"#;

        let parsed: Result<OpenMeteoResponse, _> = serde_json::from_str(sample_json);
        assert!(parsed.is_ok());
        if let Ok(resp) = parsed {
            let profile = MeteoLayerProfileLrcv::from_open_meteo_response(&resp, 0);
            assert_eq!(profile.station_icao, "LRCV");
            assert!((profile.visibility_mor_m - 140.0).abs() < 1e-6);
            assert!((profile.layers[0].speed_mps - 3.5).abs() < 1e-6);
            assert!((profile.layers[1].speed_mps - 6.8).abs() < 1e-6);
            assert!((profile.layers[2].speed_mps - 8.9).abs() < 1e-6);
            assert!((profile.layers[3].speed_mps - 11.4).abs() < 1e-6);
        }
    }

    #[test]
    fn test_arcswap_comms_blackout_and_bayesian_update() {
        let ingestor = LrcvMeteoIngestor::new("/tmp/non_existent_lrcv_test_cache.json");
        assert!(!ingestor.is_comms_blackout());

        // Switch to Comms Blackout mode (< 1 us lock-free)
        ingestor.set_comms_blackout(true);
        assert!(ingestor.is_comms_blackout());
        let snap1 = ingestor.load_snapshot();
        assert!(snap1.is_comms_blackout_mode);

        // Perform recursive Bayesian update using optical fog measurement at 80m AGL
        let optical_fog_ned = Vector3::new(-4.0, -8.0, 0.0);
        let snap2 = ingestor.update_from_optical_fog(80.0, optical_fog_ned, 0.5);
        assert_eq!(snap2.bayesian_update_count, 1);

        // Layer at 80m must move closer to the optical fog measurement
        let prior_dist = (snap1.layers[1].wind_ned() - optical_fog_ned).norm();
        let post_dist = (snap2.layers[1].wind_ned() - optical_fog_ned).norm();
        assert!(post_dist < prior_dist);
    }

    #[tokio::test]
    async fn test_fetch_blocked_during_comms_blackout() {
        let ingestor = LrcvMeteoIngestor::new("/tmp/non_existent_lrcv_blackout_cache.json");
        ingestor.set_comms_blackout(true);
        let res = ingestor.fetch_live_open_meteo_once().await;
        assert!(matches!(res, Err(MeteoIngestError::CommsBlackoutActive)));
    }
}
