//! Zero-Panic Parser for `METAR LRCV` and `SPECI LRCV` Aviation Weather Reports.
//!
//! Extracts Runway 09 RVR (`R09/...`), Meteorological Optical Range ($V_{MOR}$),
//! Koschmieder optical inverse-depth anchor $\hat{\rho}_{fog, opt} = \hat{\beta}_{ext} = -\ln(0.05) / V_{MOR}$,
//! wind speed/direction (`KT` and `MPS`), vertical visibility (`VV...`), cloud ceilings,
//! fog phenomena (`FG`, `FZFG`, `BR`), temperature/dewpoint, QNH, and ICAO approach category.

use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::open_meteo_lrcv::met_wind_to_ned_vector;

/// Exact conversion factor from knots (`KT`) to meters per second (`m/s`).
pub const KNOTS_TO_MPS: f64 = 0.514_444_f64;

/// Exact conversion factor from feet (`ft`) to meters (`m`).
pub const FEET_TO_METERS: f64 = 0.3048_f64;

/// Exact Koschmieder contrast threshold $-\ln(0.05) \approx 2.995732273553991$.
pub const KOSCHMIEDER_LN20: f64 = 2.995_732_273_553_991_f64;

/// Errors returned when parsing a malformed METAR/SPECI report.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MetarParseError {
    #[error("Empty or whitespace-only METAR string")]
    EmptyInput,
    #[error("Missing valid 4-letter ICAO station identifier")]
    MissingStationIcao,
}

/// RVR (Runway Visual Range) tendency indicator at Craiova RWY 09.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RvrTrend {
    /// `U` — RVR is increasing (Upward).
    Upward,
    /// `D` — RVR is decreasing (Downward).
    Downward,
    /// `N` — RVR shows no distinct change (NoChange).
    NoChange,
    /// No trend suffix specified in the token.
    Unspecified,
}

/// RVR qualifier prefix (`P` = Plus/above, `M` = Minus/below, `Exact` = exact value).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RvrBoundQualifier {
    Exact,
    AboveMax,
    BelowMin,
}

/// Parsed Runway Visual Range (`RVR`) measurement (e.g., `R09/0175N` or `R09/M0050D`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunwayVisualRange {
    /// Runway designator (e.g., `"09"` or `"27"`).
    pub runway: String,
    /// Visual range in meters.
    pub rvr_m: f64,
    /// Qualifier (`Exact`, `AboveMax` for `P`, `BelowMin` for `M`).
    pub qualifier: RvrBoundQualifier,
    /// RVR tendency (`Upward`, `Downward`, `NoChange`, `Unspecified`).
    pub trend: RvrTrend,
}

/// Obscuration and precipitation phenomena extracted from METAR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeatherPhenomenon {
    /// `FG` — Dense fog (visibility `< 1000 m`).
    Fog,
    /// `FZFG` — Freezing supercooled fog.
    FreezingFog,
    /// `MIFG` — Shallow ground fog.
    ShallowFog,
    /// `BCFG` — Patches of fog.
    PatchesFog,
    /// `PRFG` — Partial aerodrome fog.
    PartialFog,
    /// `BR` — Mist (`1000 m <= visibility <= 5000 m`).
    Mist,
    /// `HZ` — Haze.
    Haze,
    /// `RA` — Rain.
    Rain,
    /// `SN` — Snow.
    Snow,
}

/// Cloud coverage octas group in METAR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloudCoverage {
    Few,
    Scattered,
    Broken,
    Overcast,
}

/// Cloud layer entry (`FEW`, `SCT`, `BKN`, `OVC` with base height in meters AGL).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CloudLayerMetar {
    pub coverage: CloudCoverage,
    pub base_agl_m: f64,
}

/// ICAO Precision Instrument Approach Category determined from RVR and $V_{MOR}$ / decision height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IcaoApproachCategory {
    /// Visual Meteorological Conditions or standard Non-Precision / CAT I (`RVR >= 550 m`).
    CatI,
    /// Category II (`300 m <= RVR < 550 m`).
    CatII,
    /// Category IIIa (`175 m <= RVR < 300 m`).
    CatIIIa,
    /// Category IIIb (`50 m <= RVR < 175 m`).
    CatIIIb,
    /// Category IIIc (`RVR < 50 m` or zero-ceiling dense fog requiring 100% synthetic/optical autonomy).
    CatIIIc,
}

impl IcaoApproachCategory {
    /// Classifies the effective visual range (RVR on RWY 09 or prevailing $V_{MOR}$) into an ICAO category.
    pub fn from_effective_rvr_m(effective_rvr_m: f64, has_dense_fog: bool) -> Self {
        let rvr = if effective_rvr_m.is_finite() {
            effective_rvr_m.max(0.0)
        } else {
            150.0
        };

        if rvr < 50.0 || (has_dense_fog && rvr <= 75.0) {
            Self::CatIIIc
        } else if rvr < 175.0 {
            Self::CatIIIb
        } else if rvr < 300.0 {
            Self::CatIIIa
        } else if rvr < 550.0 {
            Self::CatII
        } else {
            Self::CatI
        }
    }
}

/// Complete structured representation of a parsed `METAR LRCV` or `SPECI LRCV` report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedMetarLrcv {
    /// Raw original METAR string.
    pub raw_metar: String,
    /// True if report starts with `SPECI`.
    pub is_speci: bool,
    /// 4-letter ICAO station code (e.g., `"LRCV"`).
    pub station_icao: String,
    /// Observation timestamp token (`DDHHMMZ`, e.g., `"301430Z"`).
    pub observation_time_z: String,
    /// Wind direction in degrees from true North (`None` if `VRB` variable).
    pub wind_direction_deg: Option<f64>,
    /// True if wind direction is marked `VRB`.
    pub wind_is_variable: bool,
    /// Sustained wind speed at `10 m AGL` in `m/s`.
    pub wind_speed_mps: f64,
    /// Peak wind gust speed at `10 m AGL` in `m/s` (if `G..` is present).
    pub wind_gust_mps: Option<f64>,
    /// 3D wind vector in local `NED` coordinates (`m/s`).
    pub wind_ned_mps: [f64; 3],
    /// Prevailing Meteorological Optical Range $V_{MOR}$ in meters.
    pub prevailing_visibility_mor_m: f64,
    /// Runway Visual Range specifically on RWY 09 (`R09/...`), if reported.
    pub rvr_rwy09: Option<RunwayVisualRange>,
    /// All RVR entries parsed from the METAR.
    pub all_rvr: Vec<RunwayVisualRange>,
    /// Active obscuration / weather phenomena (`FG`, `FZFG`, `BR`, etc.).
    pub phenomena: Vec<WeatherPhenomenon>,
    /// Vertical visibility in meters (`VV001` -> `30.48 m`), if reported.
    pub vertical_visibility_m: Option<f64>,
    /// Cloud layers reported in the METAR.
    pub cloud_layers: Vec<CloudLayerMetar>,
    /// Ambient temperature at `2 m AGL` in Celsius.
    pub temperature_c: f64,
    /// Dew point temperature at `2 m AGL` in Celsius.
    pub dew_point_c: f64,
    /// QNH altimeter setting in `hPa` (e.g., `Q1022` -> `1022.0`).
    pub qnh_hpa: f64,
    /// Koschmieder optical extinction & inverse-depth scale anchor:
    /// $\hat{\rho}_{fog, opt} = \hat{\beta}_{ext} = -\ln(0.05) / \max(V_{eff}, 12.0)$ (`m^-1`).
    pub koschmieder_rho_fog_opt_inv_m: f64,
    /// Mean optical fog depth $\bar{Z}_{fog, opt} = 1 / \hat{\rho}_{fog, opt}$ (`m`).
    pub mean_fog_depth_z_m: f64,
    /// Determined ICAO Approach Category (`CAT I` .. `CAT IIIc`).
    pub icao_category: IcaoApproachCategory,
}

impl ParsedMetarLrcv {
    /// Returns the 3D `NED` wind vector (`m/s`) parsed from the METAR wind group.
    #[inline]
    pub fn wind_ned(&self) -> Vector3<f64> {
        Vector3::new(
            self.wind_ned_mps[0],
            self.wind_ned_mps[1],
            self.wind_ned_mps[2],
        )
    }

    /// Returns true if any fog phenomenon (`FG`, `FZFG`, `MIFG`, `BCFG`, `PRFG`) is active.
    pub fn has_fog(&self) -> bool {
        self.phenomena.iter().any(|p| {
            matches!(
                p,
                WeatherPhenomenon::Fog
                    | WeatherPhenomenon::FreezingFog
                    | WeatherPhenomenon::ShallowFog
                    | WeatherPhenomenon::PatchesFog
                    | WeatherPhenomenon::PartialFog
            )
        })
    }
}

/// Computes the Koschmieder optical extinction coefficient and inverse-depth anchor
/// $\hat{\rho}_{fog, opt} = \hat{\beta}_{ext} = \frac{-\ln(0.05)}{\max(V_{MOR},\, 12.0)}$.
#[inline]
pub fn compute_koschmieder_rho_fog_opt(visibility_mor_m: f64) -> f64 {
    let v_safe = if visibility_mor_m.is_finite() {
        visibility_mor_m.clamp(12.0, 50_000.0)
    } else {
        150.0
    };
    KOSCHMIEDER_LN20 / v_safe
}

/// Parses a `METAR` or `SPECI` string (Zero-Panic).
pub fn parse_metar_lrcv(raw_input: &str) -> Result<ParsedMetarLrcv, MetarParseError> {
    let trimmed = raw_input.trim();
    if trimmed.is_empty() {
        return Err(MetarParseError::EmptyInput);
    }

    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(MetarParseError::EmptyInput);
    }

    let mut idx = 0;
    let mut is_speci = false;

    if let Some(&first) = tokens.first() {
        if first.eq_ignore_ascii_case("METAR") {
            idx += 1;
        } else if first.eq_ignore_ascii_case("SPECI") {
            is_speci = true;
            idx += 1;
        }
    }

    // Extract 4-letter station ICAO
    let station_icao = match tokens.get(idx) {
        Some(&tok) if tok.len() == 4 && tok.chars().all(|c| c.is_ascii_alphabetic()) => {
            idx += 1;
            tok.to_ascii_uppercase()
        }
        _ => return Err(MetarParseError::MissingStationIcao),
    };

    let mut observation_time_z = String::from("000000Z");
    if let Some(&tok) = tokens.get(idx) {
        if tok.ends_with('Z') && tok.len() == 7 && tok[..6].chars().all(|c| c.is_ascii_digit()) {
            observation_time_z = tok.to_string();
            idx += 1;
        }
    }

    // Skip optional AUTO / COR modifier
    if let Some(&tok) = tokens.get(idx) {
        if tok == "AUTO" || tok == "COR" {
            idx += 1;
        }
    }

    let mut wind_direction_deg: Option<f64> = Some(0.0);
    let mut wind_is_variable = false;
    let mut wind_speed_mps = 0.0_f64;
    let mut wind_gust_mps: Option<f64> = None;
    let mut prevailing_visibility_mor_m = 10_000.0_f64;
    let mut visibility_was_set = false;
    let mut all_rvr: Vec<RunwayVisualRange> = Vec::new();
    let mut phenomena: Vec<WeatherPhenomenon> = Vec::new();
    let mut vertical_visibility_m: Option<f64> = None;
    let mut cloud_layers: Vec<CloudLayerMetar> = Vec::new();
    let mut temperature_c = 4.0_f64;
    let mut dew_point_c = 4.0_f64;
    let mut qnh_hpa = 1013.25_f64;

    for &tok in tokens.iter().skip(idx) {
        let upper = tok.to_ascii_uppercase();

        // 1. Check Wind group (...KT or ...MPS)
        if let Some((dir_opt, is_vrb, spd_mps, gust_opt)) = try_parse_wind_token(&upper) {
            wind_direction_deg = dir_opt;
            wind_is_variable = is_vrb;
            wind_speed_mps = spd_mps;
            wind_gust_mps = gust_opt;
            continue;
        }

        // 2. Check CAVOK
        if upper == "CAVOK" {
            prevailing_visibility_mor_m = 10_000.0;
            visibility_was_set = true;
            continue;
        }

        // 3. Check 4-digit Prevailing Visibility in meters (e.g., 0150, 0050, 9999)
        if !visibility_was_set
            && upper.len() == 4
            && upper.chars().all(|c| c.is_ascii_digit())
        {
            if let Ok(vis_val) = upper.parse::<u32>() {
                prevailing_visibility_mor_m = if vis_val >= 9999 {
                    10_000.0
                } else {
                    f64::from(vis_val)
                };
                visibility_was_set = true;
                continue;
            }
        }

        // 4. Check RVR token (e.g., R09/0175N, R09/P0200U, R09/M0050D)
        if let Some(rvr) = try_parse_rvr_token(&upper) {
            all_rvr.push(rvr);
            continue;
        }

        // 5. Check Vertical Visibility (e.g., VV001 -> 100 ft -> 30.48 m)
        if let Some(vv_rest) = upper.strip_prefix("VV") {
            if vv_rest.len() == 3 && vv_rest.chars().all(|c| c.is_ascii_digit()) {
                if let Ok(hundreds_ft) = vv_rest.parse::<u32>() {
                    vertical_visibility_m =
                        Some(f64::from(hundreds_ft) * 100.0 * FEET_TO_METERS);
                    continue;
                }
            }
        }

        // 6. Check Cloud Layer (FEW, SCT, BKN, OVC)
        if let Some(cloud) = try_parse_cloud_token(&upper) {
            cloud_layers.push(cloud);
            continue;
        }

        // 7. Check QNH pressure (e.g., Q1022)
        if let Some(q_rest) = upper.strip_prefix('Q') {
            if q_rest.len() == 4 && q_rest.chars().all(|c| c.is_ascii_digit()) {
                if let Ok(q_val) = q_rest.parse::<u32>() {
                    qnh_hpa = f64::from(q_val);
                    continue;
                }
            }
        }

        // 8. Check Temperature / DewPoint group (e.g., 04/04, M02/M02)
        if let Some((t_c, dp_c)) = try_parse_temp_dewpoint(&upper) {
            temperature_c = t_c;
            dew_point_c = dp_c;
            continue;
        }

        // 9. Check Weather Phenomena (FG, FZFG, MIFG, BCFG, PRFG, BR, HZ, RA, SN)
        if let Some(phenom) = try_parse_phenomenon(&upper) {
            if !phenomena.contains(&phenom) {
                phenomena.push(phenom);
            }
        }
    }

    let rvr_rwy09 = all_rvr
        .iter()
        .find(|r| r.runway == "09" || r.runway == "09L" || r.runway == "09R")
        .cloned();

    // Effective optical range on RWY 09 prioritizes RVR on RWY 09 when available
    let effective_vis_m = match &rvr_rwy09 {
        Some(rvr) => rvr.rvr_m.min(prevailing_visibility_mor_m),
        None => prevailing_visibility_mor_m,
    };

    let koschmieder_rho = compute_koschmieder_rho_fog_opt(effective_vis_m);
    let mean_fog_depth_z_m = 1.0 / koschmieder_rho.max(1e-9);

    let has_dense_fog = phenomena.iter().any(|p| {
        matches!(
            p,
            WeatherPhenomenon::Fog | WeatherPhenomenon::FreezingFog
        )
    });

    let icao_category = IcaoApproachCategory::from_effective_rvr_m(effective_vis_m, has_dense_fog);

    let wind_ned = met_wind_to_ned_vector(
        wind_speed_mps,
        wind_direction_deg.unwrap_or(RWY09_DEFAULT_WIND_DIR_DEG),
        0.0,
    );

    Ok(ParsedMetarLrcv {
        raw_metar: trimmed.to_string(),
        is_speci,
        station_icao,
        observation_time_z,
        wind_direction_deg,
        wind_is_variable,
        wind_speed_mps,
        wind_gust_mps,
        wind_ned_mps: [wind_ned.x, wind_ned.y, wind_ned.z],
        prevailing_visibility_mor_m,
        rvr_rwy09,
        all_rvr,
        phenomena,
        vertical_visibility_m,
        cloud_layers,
        temperature_c,
        dew_point_c,
        qnh_hpa,
        koschmieder_rho_fog_opt_inv_m: koschmieder_rho,
        mean_fog_depth_z_m,
        icao_category,
    })
}

const RWY09_DEFAULT_WIND_DIR_DEG: f64 = 90.0;

fn try_parse_wind_token(token: &str) -> Option<(Option<f64>, bool, f64, Option<f64>)> {
    let (core, unit_scale) = if let Some(stripped) = token.strip_suffix("KT") {
        (stripped, KNOTS_TO_MPS)
    } else if let Some(stripped) = token.strip_suffix("MPS") {
        (stripped, 1.0_f64)
    } else {
        return None;
    };

    if core.len() < 5 {
        return None;
    }

    let dir_part = &core[..3];
    let rest = &core[3..];

    let (dir_opt, is_vrb) = if dir_part == "VRB" {
        (None, true)
    } else if dir_part.chars().all(|c| c.is_ascii_digit()) {
        let dir_val = dir_part.parse::<u32>().ok()?;
        (Some(f64::from(dir_val).rem_euclid(360.0)), false)
    } else {
        return None;
    };

    let (spd_str, gust_str_opt) = if let Some((s, g)) = rest.split_once('G') {
        (s, Some(g))
    } else {
        (rest, None)
    };

    if spd_str.is_empty() || !spd_str.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let spd_raw = spd_str.parse::<u32>().ok()?;
    let spd_mps = f64::from(spd_raw) * unit_scale;

    let gust_mps = match gust_str_opt {
        Some(g_str) if !g_str.is_empty() && g_str.chars().all(|c| c.is_ascii_digit()) => {
            let g_raw = g_str.parse::<u32>().ok()?;
            Some(f64::from(g_raw) * unit_scale)
        }
        Some(_) => return None,
        None => None,
    };

    Some((dir_opt, is_vrb, spd_mps, gust_mps))
}

fn try_parse_rvr_token(token: &str) -> Option<RunwayVisualRange> {
    let rest = token.strip_prefix('R')?;
    let (rwy_part, val_part_raw) = rest.split_once('/')?;
    if rwy_part.is_empty() || val_part_raw.is_empty() {
        return None;
    }

    // Handle variable RVR like R09/0100V0200U by taking the minimum conservative RVR value
    let mut val_part = val_part_raw;
    let trend = if let Some(stripped) = val_part.strip_suffix('U') {
        val_part = stripped;
        RvrTrend::Upward
    } else if let Some(stripped) = val_part.strip_suffix('D') {
        val_part = stripped;
        RvrTrend::Downward
    } else if let Some(stripped) = val_part.strip_suffix('N') {
        val_part = stripped;
        RvrTrend::NoChange
    } else {
        RvrTrend::Unspecified
    };

    // If variable (e.g. 0100V0200), take the first (minimum) part for safety
    let primary_val_str = if let Some((first_v, _second_v)) = val_part.split_once('V') {
        first_v
    } else {
        val_part
    };

    let (qualifier, digits_str) = if let Some(stripped) = primary_val_str.strip_prefix('P') {
        (RvrBoundQualifier::AboveMax, stripped)
    } else if let Some(stripped) = primary_val_str.strip_prefix('M') {
        (RvrBoundQualifier::BelowMin, stripped)
    } else {
        (RvrBoundQualifier::Exact, primary_val_str)
    };

    if digits_str.is_empty() || !digits_str.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let rvr_m = f64::from(digits_str.parse::<u32>().ok()?);
    Some(RunwayVisualRange {
        runway: rwy_part.to_string(),
        rvr_m,
        qualifier,
        trend,
    })
}

fn try_parse_cloud_token(token: &str) -> Option<CloudLayerMetar> {
    if token.len() < 6 {
        return None;
    }
    let prefix = &token[..3];
    let coverage = match prefix {
        "FEW" => CloudCoverage::Few,
        "SCT" => CloudCoverage::Scattered,
        "BKN" => CloudCoverage::Broken,
        "OVC" => CloudCoverage::Overcast,
        _ => return None,
    };

    let height_digits = &token[3..6];
    if !height_digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let hundreds_ft = height_digits.parse::<u32>().ok()?;
    let base_agl_m = f64::from(hundreds_ft) * 100.0 * FEET_TO_METERS;
    Some(CloudLayerMetar {
        coverage,
        base_agl_m,
    })
}

fn try_parse_temp_dewpoint(token: &str) -> Option<(f64, f64)> {
    let (t_str, dp_str) = token.split_once('/')?;
    let parse_signed_temp = |s: &str| -> Option<f64> {
        if let Some(neg_digits) = s.strip_prefix('M') {
            if neg_digits.len() == 2 && neg_digits.chars().all(|c| c.is_ascii_digit()) {
                let val = neg_digits.parse::<i32>().ok()?;
                return Some(-f64::from(val));
            }
            None
        } else if s.len() == 2 && s.chars().all(|c| c.is_ascii_digit()) {
            let val = s.parse::<i32>().ok()?;
            Some(f64::from(val))
        } else {
            None
        }
    };

    let t_c = parse_signed_temp(t_str)?;
    let dp_c = parse_signed_temp(dp_str)?;
    Some((t_c, dp_c))
}

fn try_parse_phenomenon(token: &str) -> Option<WeatherPhenomenon> {
    let clean = token.trim_start_matches(['-', '+']);
    match clean {
        "FG" => Some(WeatherPhenomenon::Fog),
        "FZFG" => Some(WeatherPhenomenon::FreezingFog),
        "MIFG" => Some(WeatherPhenomenon::ShallowFog),
        "BCFG" => Some(WeatherPhenomenon::PatchesFog),
        "PRFG" => Some(WeatherPhenomenon::PartialFog),
        "BR" => Some(WeatherPhenomenon::Mist),
        "HZ" => Some(WeatherPhenomenon::Haze),
        "RA" | "DZ" | "SHRA" => Some(WeatherPhenomenon::Rain),
        "SN" | "SHSN" => Some(WeatherPhenomenon::Snow),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_canonical_lrcv_cat_iii_metar() {
        let raw = "METAR LRCV 301430Z 09006KT 0150 R09/0175N FG VV001 04/04 Q1022";
        let res = parse_metar_lrcv(raw);
        assert!(res.is_ok());
        if let Ok(m) = res {
            assert_eq!(m.station_icao, "LRCV");
            assert_eq!(m.observation_time_z, "301430Z");
            assert!(!m.is_speci);
            assert_eq!(m.wind_direction_deg, Some(90.0));
            assert!((m.wind_speed_mps - 6.0 * KNOTS_TO_MPS).abs() < 1e-6);
            assert!((m.prevailing_visibility_mor_m - 150.0).abs() < 1e-6);

            assert!(m.rvr_rwy09.is_some());
            if let Some(rvr) = &m.rvr_rwy09 {
                assert_eq!(rvr.runway, "09");
                assert!((rvr.rvr_m - 175.0).abs() < 1e-6);
                assert_eq!(rvr.trend, RvrTrend::NoChange);
            }

            assert!(m.has_fog());
            assert!(m.vertical_visibility_m.is_some());
            if let Some(vv) = m.vertical_visibility_m {
                assert!((vv - 30.48).abs() < 1e-4);
            }
            assert!((m.temperature_c - 4.0).abs() < 1e-6);
            assert!((m.dew_point_c - 4.0).abs() < 1e-6);
            assert!((m.qnh_hpa - 1022.0).abs() < 1e-6);

            // Effective vis is min(175, 150) = 150m -> CAT IIIb, Koschmieder ~ 2.9957 / 150 ~ 0.01997 m^-1
            assert_eq!(m.icao_category, IcaoApproachCategory::CatIIIb);
            assert!((m.koschmieder_rho_fog_opt_inv_m - (KOSCHMIEDER_LN20 / 150.0)).abs() < 1e-6);
        }
    }

    #[test]
    fn test_parse_cat_iiic_freezing_fog_and_mps_wind() {
        let raw = "SPECI LRCV 301500Z 08005G09MPS 0040 R09/M0050D FZFG VV001 M02/M02 Q1024";
        let res = parse_metar_lrcv(raw);
        assert!(res.is_ok());
        if let Ok(m) = res {
            assert!(m.is_speci);
            assert_eq!(m.station_icao, "LRCV");
            assert!((m.wind_speed_mps - 5.0).abs() < 1e-6);
            assert_eq!(m.wind_gust_mps, Some(9.0));
            assert!((m.temperature_c - (-2.0)).abs() < 1e-6);
            assert!((m.dew_point_c - (-2.0)).abs() < 1e-6);
            assert_eq!(m.icao_category, IcaoApproachCategory::CatIIIc);
            assert!(m.phenomena.contains(&WeatherPhenomenon::FreezingFog));
        }
    }

    #[test]
    fn test_malformed_metar_never_panics() {
        assert!(matches!(
            parse_metar_lrcv(""),
            Err(MetarParseError::EmptyInput)
        ));
        assert!(matches!(
            parse_metar_lrcv("   "),
            Err(MetarParseError::EmptyInput)
        ));
        assert!(matches!(
            parse_metar_lrcv("METAR 123"),
            Err(MetarParseError::MissingStationIcao)
        ));

        // Partial or unusual tokens after valid station never panic
        let res = parse_metar_lrcv("METAR LRCV GARBAGE_TOKEN //// R09/BAD QNH_BAD");
        assert!(res.is_ok());
    }
}
