//! 配置加载模块
//! 从 JSON 文件加载岁差系数、匹配参数等模型参数
//! 替代原硬编码

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrecessionConfig {
    pub model_name: String,
    pub version: String,
    pub omega_a_t0_arcsec: f64,
    pub j2000_jd: f64,
    pub julian_century_days: f64,
    pub psi_a_coeffs_mas: Vec<f64>,
    pub omega_a_coeffs_mas: Vec<f64>,
    pub chi_a_coeffs_mas: Vec<f64>,
    pub zeta_a_coeffs_arcsec: Vec<f64>,
    pub theta_a_coeffs_arcsec: Vec<f64>,
    pub z_a_coeffs_arcsec: Vec<f64>,
    pub iau2000b_nutation_terms: Vec<NutationTerm>,
    pub nutation_delaunay_rates_arcsec_per_cy: DelaunayRates,
    pub nutation_delaunay_constants_arcsec: DelaunayConstants,
    pub proper_motion: ProperMotionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct NutationTerm {
    pub l: f64,
    pub lp: f64,
    pub F: f64,
    pub D: f64,
    pub Om: f64,
    pub dpsi_sin: f64,
    pub deps_sin: f64,
    pub dpsi_cos: f64,
    pub deps_cos: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct DelaunayRates {
    pub l: f64,
    pub lp: f64,
    pub F: f64,
    pub D: f64,
    pub Om: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct DelaunayConstants {
    pub l: f64,
    pub lp: f64,
    pub F: f64,
    pub D: f64,
    pub Om: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProperMotionConfig {
    pub default_pm_ra_mas_per_yr: f64,
    pub default_pm_dec_mas_per_yr: f64,
    pub cos_dec_eps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchingConfig {
    pub model_name: String,
    pub version: String,
    pub default_config: MatchDefaultConfig,
    pub galactic_prior: GalacticPriorConfig,
    pub likelihood: LikelihoodConfig,
    pub channel_buffer_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchDefaultConfig {
    pub spatial_sigma_scale: f64,
    pub temporal_sigma_yr: f64,
    pub magnitude_sigma: f64,
    pub lightcurve_sigma_days: f64,
    pub min_sep_arcmin: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalacticPriorConfig {
    pub r_sun_kpc: f64,
    pub r_disk_scale_kpc: f64,
    pub z_disk_scale_kpc: f64,
    pub prior_floor_log: f64,
    pub ngp_ra_deg: f64,
    pub ngp_dec_deg: f64,
    pub lon_cp_deg: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LikelihoodConfig {
    pub spatial: SpatialLikelihoodConfig,
    pub temporal: TemporalLikelihoodConfig,
    pub magnitude: MagnitudeLikelihoodConfig,
    pub lightcurve: LightcurveLikelihoodConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialLikelihoodConfig {
    pub snr_position_uncertainty_deg: f64,
    pub cauchy_scale_deg: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalLikelihoodConfig {
    pub nu: f64,
    pub min_sigma_yr: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagnitudeLikelihoodConfig {
    pub nu: f64,
    pub min_sigma: f64,
    pub default_extinction_av: f64,
    pub default_extinction_err: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightcurveLikelihoodConfig {
    pub nu: f64,
    pub min_sigma_days: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogConfig {
    pub model_name: String,
    pub version: String,
    pub data_sources: Vec<DataSource>,
    pub cleaning_rules: CleaningRules,
    pub batch_size: usize,
    pub parallelism: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSource {
    pub name: String,
    pub dynasty: String,
    pub epoch_year: f64,
    pub quality_weight: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleaningRules {
    pub max_ruxiu_du: f64,
    pub max_quji_du: f64,
    pub min_magnitude: f64,
    pub max_magnitude: f64,
    pub valid_color_descriptions: Vec<String>,
    pub default_color_temp_k: f64,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub precession: PrecessionConfig,
    pub matching: MatchingConfig,
    pub catalog: CatalogConfig,
    pub eclipse: EclipseConfig,
    pub instrument: InstrumentConfig,
    pub variable: VariableConfig,
    pub horoscope: HoroscopeConfig,
}

impl AppConfig {
    pub fn load(config_dir: &str) -> Result<Self, String> {
        let prec = load_json::<PrecessionConfig>(&format!("{}/precession.json", config_dir))?;
        let match_cfg = load_json::<MatchingConfig>(&format!("{}/matching.json", config_dir))?;
        let cat = load_json::<CatalogConfig>(&format!("{}/catalog.json", config_dir))?;
        let ec = load_json::<EclipseConfig>(&format!("{}/eclipse.json", config_dir))?;
        let ins = load_json::<InstrumentConfig>(&format!("{}/instrument.json", config_dir))?;
        let var = load_json::<VariableConfig>(&format!("{}/variable.json", config_dir))?;
        let hor = load_json::<HoroscopeConfig>(&format!("{}/horoscope.json", config_dir))?;
        Ok(Self {
            precession: prec,
            matching: match_cfg,
            catalog: cat,
            eclipse: ec,
            instrument: ins,
            variable: var,
            horoscope: hor,
        })
    }
}

fn load_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(format!("Config file not found: {}", path));
    }
    let content = fs::read_to_string(p).map_err(|e| format!("Read {}: {}", path, e))?;
    serde_json::from_str(&content).map_err(|e| format!("Parse {}: {}", path, e))
}

// ============================================================
// 新 Feature: Eclipse 日月食配置
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EclipseConfig {
    pub model_name: String,
    pub version: String,
    pub saros_cycle_yr: f64,
    pub saros_cycle_days: f64,
    pub exeligmos_yr: f64,
    pub synodic_month_days: f64,
    pub draconic_month_days: f64,
    pub anomalistic_month_days: f64,
    pub tropical_year_days: f64,
    pub earth_radius_km: f64,
    pub lunar_radius_km: f64,
    pub lunar_distance_perigee_km: f64,
    pub lunar_distance_apogee_km: f64,
    pub solar_apparent_radius_deg: f64,
    pub lunar_apparent_radius_perigee_deg: f64,
    pub lunar_apparent_radius_apogee_deg: f64,
    pub obliquity_deg: f64,
    pub lunar_inclination_deg: f64,
    pub eclipse_season_days: f64,
    pub solar_eclipse_latitude_limit_deg: f64,
    pub lunar_eclipse_latitude_limit_deg: f64,
    pub ut1_minus_tai_at_j2000_s: f64,
    pub dt_polynomial_per_cent_sq_per_cy: f64,
    pub channel_buffer_size: usize,
    pub num_threads: Option<usize>,
}

// ============================================================
// 新 Feature: Instrument 仪器误差反演配置
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentMeta {
    pub code: String,
    pub name_cn: String,
    pub dynasty: String,
    pub erected_year: f64,
    pub ring_count: i32,
    pub nominal_accuracy_arcmin: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InversionParams {
    pub min_shared_stars: usize,
    pub max_residual_outlier_sigma: f64,
    pub iterative_reweight_max_iter: i32,
    pub sigma_clip_threshold: f64,
    pub regularization_lambda: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorComponentPriors {
    pub polar_axis_tilt_prior_arcmin: f64,
    pub polar_axis_azimuth_prior_arcmin: f64,
    pub divisions_systematic_prior_arcmin_per_cycle: f64,
    pub micrometer_vernier_error_prior_arcmin: f64,
    pub atmospheric_refraction_prior_arcmin: f64,
    pub channel_buffer_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentConfig {
    pub model_name: String,
    pub version: String,
    pub instruments: Vec<InstrumentMeta>,
    pub inversion: InversionParams,
    pub error_components: ErrorComponentPriors,
}

// ============================================================
// 新 Feature: Variable 变星亮度演化配置
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagnitudeBracket {
    pub text: String,
    pub mag_range: [f64; 2],
    pub mag_default: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LombScargleConfig {
    pub min_period_days: f64,
    pub max_period_days: f64,
    pub freq_oversampling_factor: f64,
    pub false_alarm_level: f64,
    pub num_peaks_to_return: i32,
    pub window_function_pad_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiraTemplateConfig {
    pub default_period_days: f64,
    pub min_period_days: f64,
    pub max_period_days: f64,
    pub default_amplitude_mag: f64,
    pub default_mean_mag: f64,
    pub pdot_per_cent_per_myr: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableConfig {
    pub model_name: String,
    pub version: String,
    pub ancient_magnitude_brackets: Vec<MagnitudeBracket>,
    pub magnitude_text_to_value_uncertainty: f64,
    pub lomb_scargle: LombScargleConfig,
    pub mira_variable_template: MiraTemplateConfig,
    pub channel_buffer_size: usize,
}

// ============================================================
// 新 Feature: Horoscope 科普交互配置
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardStyleConfig {
    pub width_px: i32,
    pub height_px: i32,
    pub title_font_family: String,
    pub body_font_family: String,
    pub accent_color: String,
    pub background_gradient_from: String,
    pub background_gradient_to: String,
    pub star_density_multiplier: f64,
    pub mag_limit_for_labels: f64,
    pub label_font_size_px: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationDefaults {
    pub default_latitude_deg: f64,
    pub default_longitude_deg: f64,
    pub default_city_name: String,
    pub atmospheric_extinction_coeff_per_airmass: f64,
    pub min_altitude_for_plot_deg: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochAlignmentConfig {
    pub j2000_anchor_year: f64,
    pub max_allowable_epoch_gap_yr_for_stars: f64,
    pub projection_stereographic_scale: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanetApproxEphemeris {
    pub mercury_synodic_days: f64,
    pub venus_synodic_days: f64,
    pub mars_synodic_days: f64,
    pub jupiter_synodic_days: f64,
    pub saturn_synodic_days: f64,
    pub mean_obliquity_deg: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoroscopeConfig {
    pub model_name: String,
    pub version: String,
    pub card: CardStyleConfig,
    pub location: LocationDefaults,
    pub epoch_alignment: EpochAlignmentConfig,
    pub zodiacal_lunar_mansions_inclusion: bool,
    pub planet_ephemeris_approx: PlanetApproxEphemeris,
}
