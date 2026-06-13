//! 数据模型定义

use serde::{Deserialize, Serialize};

// ============================================================
// 数据库实体
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dynasty {
    pub id: i64,
    pub name_cn: String,
    pub name_pinyin: String,
    pub start_year: i32,
    pub end_year: i32,
    pub canonical_epoch: f64,
    pub color_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LunarMansion {
    pub id: i64,
    pub mansion_order: i32,
    pub name_cn: String,
    pub name_pinyin: String,
    pub ruxiu_width_deg: f64,
    pub ra_start_deg: f64,
    pub ra_end_deg: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AncientStar {
    pub id: i64,
    pub star_id_code: String,
    pub dynasty_id: i64,
    pub mansion_id: Option<i64>,
    pub star_name_cn: String,
    pub star_name_alt: Option<String>,
    pub constellation: Option<String>,
    pub ruxiu_du: Option<f64>,
    pub quji_du: Option<f64>,
    pub ra_ancient_conv: Option<f64>,
    pub dec_ancient_conv: Option<f64>,
    pub ra_j2000: Option<f64>,
    pub dec_j2000: Option<f64>,
    pub magnitude_ancient: Option<String>,
    pub magnitude_num: Option<f64>,
    pub color_desc: Option<String>,
    pub color_class: Option<String>,
    pub color_temp_k: Option<f64>,
    pub proper_motion_ra: Option<f64>,
    pub proper_motion_dec: Option<f64>,
    pub parallax: Option<f64>,
    pub source_book: Option<String>,
    pub quality_flag: i32,
    pub notes: Option<String>,
    pub modern_hd_id: Option<i64>,
    pub cross_match_id: Option<i64>,
    // JOIN 字段
    pub dynasty_name: Option<String>,
    pub mansion_name: Option<String>,
    pub mansion_order: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AncientComet {
    pub id: i64,
    pub comet_id_code: String,
    pub dynasty_id: i64,
    pub year_ancient: Option<String>,
    pub year_ce: Option<f64>,
    pub ruxiu_du: Option<f64>,
    pub quji_du: Option<f64>,
    pub ra_deg: Option<f64>,
    pub dec_deg: Option<f64>,
    pub magnitude: Option<f64>,
    pub color_desc: Option<String>,
    pub tail_direction: Option<String>,
    pub tail_length: Option<f64>,
    pub duration_days: Option<i32>,
    pub description: Option<String>,
    pub dynasty_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestStar {
    pub id: i64,
    pub guest_id_code: String,
    pub dynasty_id: i64,
    pub star_name: Option<String>,
    pub year_ancient: i32,
    pub year_ce: f64,
    pub month_ancient: Option<i32>,
    pub day_ancient: Option<i32>,
    pub ruxiu_du: Option<f64>,
    pub quji_du: Option<f64>,
    pub ra_deg: Option<f64>,
    pub dec_deg: Option<f64>,
    pub ra_err: f64,
    pub dec_err: f64,
    pub peak_mag: f64,
    pub peak_mag_err: f64,
    pub visibility_days: Option<i32>,
    pub lightcurve_type: String,
    pub description: Option<String>,
    pub position_desc: Option<String>,
    pub dynasty_name: Option<String>,
    pub matched_snr_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupernovaRemnantDb {
    pub id: i64,
    pub remnant_name: String,
    pub sn_type: String,
    pub ra_deg: f64,
    pub dec_deg: f64,
    pub gal_l: Option<f64>,
    pub gal_b: Option<f64>,
    pub age_yr: f64,
    pub age_err_yr: f64,
    pub distance_kpc: f64,
    pub distance_err: f64,
    pub diameter_pc: Option<f64>,
    pub radio_flux_ghz: Option<f64>,
    pub xray_luminosity: Option<f64>,
    pub gamma_detected: bool,
    pub historical_sn_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchResult {
    pub id: i64,
    pub guest_id: i64,
    pub remnant_id: i64,
    pub remnant_name: String,
    pub remnant_type: String,
    pub rank_within_guest: i32,
    pub match_probability: f64,
    pub log_posterior: f64,
    pub log_likelihood: f64,
    pub log_prior: f64,
    pub bayes_factor: f64,
    pub angular_sep_arcmin: f64,
    pub time_delta_yr: f64,
    pub spatial_score: f64,
    pub temporal_score: f64,
    pub magnitude_score: f64,
    pub lightcurve_score: f64,
    pub model_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossDynastyPair {
    pub dynasty_1: DynastyInfo,
    pub dynasty_2: DynastyInfo,
    pub star_id_1: i64,
    pub star_id_2: i64,
    pub delta_ruxiu: f64,
    pub delta_quji: f64,
    pub delta_ra: f64,
    pub delta_dec: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynastyInfo {
    pub id: i64,
    pub name: String,
    pub year: i32,
}

// ============================================================
// API 请求 / 响应
// ============================================================

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StarQueryParams {
    pub dynasty_id: Option<i64>,
    pub dynasty_name: Option<String>,
    pub mansion_id: Option<i64>,
    pub constellation: Option<String>,
    pub star_name: Option<String>,
    pub mag_min: Option<f64>,
    pub mag_max: Option<f64>,
    pub ra_min: Option<f64>,
    pub ra_max: Option<f64>,
    pub dec_min: Option<f64>,
    pub dec_max: Option<f64>,
    pub quality_min: Option<i32>,
    pub source_book: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MatchRequest {
    pub guest_id: Option<i64>,
    pub include_snr: Option<bool>,
    pub top_k: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub message: String,
    pub data: Option<T>,
    pub total: Option<i64>,
    pub version: String,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            code: 0,
            message: "success".into(),
            data: Some(data),
            total: None,
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }

    pub fn ok_with_count(data: T, total: i64) -> Self {
        Self {
            code: 0,
            message: "success".into(),
            data: Some(data),
            total: Some(total),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }

    pub fn err<S: Into<String>>(msg: S) -> Self {
        Self {
            code: -1,
            message: msg.into(),
            data: None,
            total: None,
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}

// ============================================================
// 新 Feature: 日食月食 Eclipse
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EclipseRecord {
    pub id: i64,
    pub eclipse_id_code: String,
    pub dynasty_id: i64,
    pub eclipse_type: String,
    pub year_ancient: Option<String>,
    pub year_ce: f64,
    pub month_ancient: Option<i32>,
    pub day_ancient: Option<i32>,
    pub hour_ancient: Option<String>,
    pub magnitude_desc: Option<String>,
    pub magnitude_num: Option<f64>,
    pub duration_desc: Option<String>,
    pub duration_min: Option<f64>,
    pub ruxiu_du: Option<f64>,
    pub quji_du: Option<f64>,
    pub ra_deg: Option<f64>,
    pub dec_deg: Option<f64>,
    pub dynasty_name: Option<String>,
    pub location_desc: Option<String>,
    pub source_book: Option<String>,
    pub record_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EclipseComputationResult {
    pub eclipse_id: Option<i64>,
    pub eclipse_type: String,
    pub record_year_ce: f64,
    pub computed_midpoint_jd_et: f64,
    pub computed_midpoint_jd_ut1: f64,
    pub delta_t_s: f64,
    pub saros_number: i32,
    pub exeligmos_cycle: i32,
    pub sun_ra_deg_at_max: f64,
    pub sun_dec_deg_at_max: f64,
    pub moon_ra_deg_at_max: f64,
    pub moon_dec_deg_at_max: f64,
    pub solar_ecliptic_lon_deg: f64,
    pub solar_ecliptic_lat_deg: f64,
    pub lunar_ecliptic_lon_deg: f64,
    pub lunar_ecliptic_lat_deg: f64,
    pub magnitude_predicted: f64,
    pub obscuration_fraction: f64,
    pub eclipse_classification: String,
    pub duration_total_s: Option<f64>,
    pub umbra_radius_at_moon_km: Option<f64>,
    pub path_center_lat_deg: Option<f64>,
    pub path_center_lon_deg: Option<f64>,
    pub path_width_km: Option<f64>,
    pub magnitude_agreement_deviation: Option<f64>,
    pub time_agreement_deviation_days: Option<f64>,
    pub overall_quality_score: f64,
    pub path_samples_utm: Option<Vec<EclipsePathSample>>,
    pub umbra_polygon_latlon: Option<Vec<[f64; 2]>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EclipsePathSample {
    pub time_since_midpoint_min: f64,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub local_solar_altitude_deg: f64,
    pub umbra_radius_km: f64,
    pub path_width_km: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EclipseRequest {
    pub dynasty_id: Option<i64>,
    pub year_ce_min: Option<f64>,
    pub year_ce_max: Option<f64>,
    pub eclipse_type: Option<String>,
    pub compute_path: Option<bool>,
    pub path_sample_resolution_km: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EclipseComputeSingleRequest {
    pub year_ce: f64,
    pub month: Option<i32>,
    pub day: Option<i32>,
    pub eclipse_type: Option<String>,
    pub compute_path: Option<bool>,
}

// ============================================================
// 新 Feature: 古代仪器误差反演 Instrument
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AncientInstrument {
    pub id: i64,
    pub instrument_code: String,
    pub name_cn: String,
    pub dynasty_id: i64,
    pub dynasty_name: Option<String>,
    pub erected_year: f64,
    pub location_lat_deg: Option<f64>,
    pub location_lon_deg: Option<f64>,
    pub location_name: Option<String>,
    pub ring_count: i32,
    pub nominal_accuracy_arcmin: f64,
    pub divisions_circle: Option<i32>,
    pub vernier_resolution_arcmin: Option<f64>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentObservation {
    pub id: i64,
    pub instrument_id: i64,
    pub star_id: Option<i64>,
    pub star_name_cn: Option<String>,
    pub observation_year_ce: f64,
    pub ruxiu_du_measured: Option<f64>,
    pub quji_du_measured: Option<f64>,
    pub ra_deg_measured: Option<f64>,
    pub dec_deg_measured: Option<f64>,
    pub ra_j2000_true: Option<f64>,
    pub dec_j2000_true: Option<f64>,
    pub source_book: Option<String>,
    pub quality_flag: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentErrorSolution {
    pub instrument_id: i64,
    pub instrument_code: String,
    pub instrument_name_cn: String,
    pub ref_instrument_code: String,
    pub num_shared_stars: usize,
    pub num_iterations: i32,
    pub converged: bool,
    pub polar_axis_tilt_arcmin: f64,
    pub polar_axis_tilt_uncertainty_arcmin: f64,
    pub polar_axis_azimuth_arcmin: f64,
    pub polar_axis_azimuth_uncertainty_arcmin: f64,
    pub divisions_systematic_correction_arcmin_per_cycle: f64,
    pub divisions_periodicity_1_arcmin: f64,
    pub divisions_periodicity_2_arcmin: f64,
    pub ra_zero_point_offset_arcmin: f64,
    pub dec_zero_point_offset_arcmin: f64,
    pub collimation_error_arcmin: f64,
    pub flexure_term_arcmin_per_90deg: f64,
    pub refraction_correction_arcmin_per_airmass: f64,
    pub residuals_ra_mean_arcmin: f64,
    pub residuals_ra_std_arcmin: f64,
    pub residuals_ra_median_abs_arcmin: f64,
    pub residuals_dec_mean_arcmin: f64,
    pub residuals_dec_std_arcmin: f64,
    pub residuals_dec_median_abs_arcmin: f64,
    pub overall_rms_arcmin: f64,
    pub chi_squared_reduced: f64,
    pub accuracy_assessment_quality: String,
    pub accuracy_assessment_text: String,
    pub per_star_residuals: Option<Vec<PerInstrumentStarResidual>>,
    pub error_component_radar: Vec<ErrorRadarEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerInstrumentStarResidual {
    pub star_name_cn: String,
    pub ra_residual_arcmin: f64,
    pub dec_residual_arcmin: f64,
    pub total_angular_residual_arcmin: f64,
    pub outlier_flag: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRadarEntry {
    pub component_name: String,
    pub magnitude_arcmin: f64,
    pub relative_contribution_per_cent: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstrumentInversionRequest {
    pub target_instrument_id: i64,
    pub reference_instrument_id: Option<i64>,
    pub use_gaia_as_reference: Option<bool>,
    pub sigma_clip: Option<bool>,
}

// ============================================================
// 新 Feature: 变星亮度演化 Variable Star
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableStarMeta {
    pub id: i64,
    pub modern_name: String,
    pub ancient_names: Option<Vec<String>>,
    pub constellation_code: Option<String>,
    pub hd_id: Option<i64>,
    pub hr_id: Option<i64>,
    pub hipparcos_id: Option<i64>,
    pub gcvs_variable_type: String,
    pub ra_j2000_deg: f64,
    pub dec_j2000_deg: f64,
    pub distance_pc: Option<f64>,
    pub distance_err: Option<f64>,
    pub spectral_type: Option<String>,
    pub luminosity_class: Option<String>,
    pub min_mag_v: Option<f64>,
    pub max_mag_v: Option<f64>,
    pub mean_mag_v: Option<f64>,
    pub epoch_mjd_max: Option<f64>,
    pub published_period_days: Option<f64>,
    pub published_period_err: Option<f64>,
    pub period_change_rate_pdot: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagnitudeMeasurement {
    pub id: i64,
    pub variable_id: i64,
    pub epoch_yr: f64,
    pub epoch_mjd: Option<f64>,
    pub magnitude: f64,
    pub magnitude_uncertainty: Option<f64>,
    pub passband: String,
    pub source_type: String,
    pub source_book: Option<String>,
    pub ancient_description: Option<String>,
    pub ancient_quality: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightCurveReconstruction {
    pub variable_id: i64,
    pub modern_name: String,
    pub gcvs_type: String,
    pub num_ancient_measurements: usize,
    pub num_modern_measurements: usize,
    pub coverage_start_yr: f64,
    pub coverage_end_yr: f64,
    pub reconstructed_samples: Vec<LightCurveSample>,
    pub phase_folded_samples: Option<Vec<PhaseFoldedSample>>,
    pub periodogram: LombScargleResult,
    pub best_period_days: f64,
    pub best_period_uncertainty_days: f64,
    pub best_fit_amplitude_mag: f64,
    pub best_fit_mean_mag: f64,
    pub phase_offset: f64,
    pub chi_squared: f64,
    pub reduced_chi_squared: f64,
    pub period_change_significance_sigma: f64,
    pub pdot_estimate: f64,
    pub ancient_vs_modern_period_delta_days: Option<f64>,
    pub ancient_period_determination_days: Option<f64>,
    pub longterm_trend_mag_per_myr: Option<f64>,
    pub reconstruction_notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightCurveSample {
    pub epoch_yr: f64,
    pub model_magnitude: f64,
    pub model_lower_ci: f64,
    pub model_upper_ci: f64,
    pub observed_magnitude: Option<f64>,
    pub passband: Option<String>,
    pub source_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseFoldedSample {
    pub phase: f64,
    pub magnitude: f64,
    pub magnitude_err: Option<f64>,
    pub epoch_yr: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LombScargleResult {
    pub frequencies_per_day: Vec<f64>,
    pub periods_days: Vec<f64>,
    pub power: Vec<f64>,
    pub peaks: Vec<PeriodogramPeak>,
    pub false_alarm_probability_threshold: f64,
    pub false_alarm_power_level: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodogramPeak {
    pub rank: i32,
    pub frequency_per_day: f64,
    pub period_days: f64,
    pub power: f64,
    pub false_alarm_probability: f64,
    pub alias_of_fundamental: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VariableStarQuery {
    pub gcvs_type: Option<String>,
    pub min_amplitude_mag: Option<f64>,
    pub max_period_days: Option<f64>,
    pub search_name: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LightCurveRequest {
    pub variable_id: i64,
    pub use_published_period: Option<bool>,
    pub override_period_days: Option<f64>,
    pub include_ancient_only_fit: Option<bool>,
    pub reconstruction_resolution_per_phase: Option<i32>,
}

// ============================================================
// 新 Feature: 公众科普交互 Personal Starmap
// ============================================================

#[derive(Debug, Clone, Deserialize)]
pub struct PersonalStarmapRequest {
    pub birth_year: i32,
    pub birth_month: i32,
    pub birth_day: i32,
    pub birth_hour_utc: Option<f64>,
    pub latitude_deg: Option<f64>,
    pub longitude_deg: Option<f64>,
    pub city_name: Option<String>,
    pub projection_mode: Option<String>,
    pub card_style: Option<String>,
    pub show_constellation_lines: Option<bool>,
    pub show_moon_planets: Option<bool>,
    pub show_lunar_mansions: Option<bool>,
    pub mag_limit: Option<f64>,
    pub compare_with_ancient_epoch: Option<f64>,
    pub generate_share_card: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalStarmapResponse {
    pub personal_info: PersonalInfo,
    pub birth_datetime_iso: String,
    pub birth_jd_ut1: f64,
    pub birth_local_sidereal_time_deg: f64,
    pub ecliptic_obliquity_deg: f64,
    pub precession_epoch_delta_yr: f64,
    pub projection_mode: String,
    pub stars: Vec<StarmapStar>,
    pub constellation_lines: Option<Vec<[i64; 2]>>,
    pub lunar_mansion_boundaries: Option<Vec<LunarMansionBoundary>>,
    pub solar_system_bodies: Vec<SolarSystemBody>,
    pub ancient_comparison: Option<AncientStarmapDiff>,
    pub share_card_spec: Option<ShareCardSpec>,
    pub notable_celestial_events: Vec<String>,
    pub lucky_stars: Vec<LuckyStarEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalInfo {
    pub birth_date_ymd: [i32; 3],
    pub birth_hour_utc_decimal: f64,
    pub latitude_deg: f64,
    pub longitude_deg: f64,
    pub city_name: String,
    pub zodiacal_sun_sign: String,
    pub zodiacal_moon_sign: String,
    pub lunar_mansion_sun: String,
    pub lunar_mansion_moon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarmapStar {
    pub star_id: Option<i64>,
    pub modern_name: Option<String>,
    pub ancient_name_cn: Option<String>,
    pub ra_j2000_deg: f64,
    pub dec_j2000_deg: f64,
    pub ra_at_birth_deg: f64,
    pub dec_at_birth_deg: f64,
    pub altitude_at_birth_deg: f64,
    pub azimuth_at_birth_deg: f64,
    pub projected_x: f64,
    pub projected_y: f64,
    pub apparent_magnitude: f64,
    pub color_temp_k: Option<f64>,
    pub magnitude_ancient_desc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LunarMansionBoundary {
    pub mansion_name_cn: String,
    pub ra_start_deg_at_epoch: f64,
    pub ra_end_deg_at_epoch: f64,
    pub dec_samples: Vec<[f64; 2]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolarSystemBody {
    pub body_name_en: String,
    pub body_name_cn: String,
    pub ra_deg: f64,
    pub dec_deg: f64,
    pub ecliptic_lon_deg: f64,
    pub ecliptic_lat_deg: f64,
    pub altitude_deg: f64,
    pub azimuth_deg: f64,
    pub apparent_magnitude: f64,
    pub angular_diameter_arcsec: f64,
    pub projected_x: f64,
    pub projected_y: f64,
    pub phase_fraction: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AncientStarmapDiff {
    pub ancient_epoch_yr: f64,
    pub ancient_sun_lunar_mansion: String,
    pub ancient_moon_lunar_mansion: String,
    pub num_stars_shifted_gt_1deg: usize,
    pub avg_angular_shift_arcmin: f64,
    pub max_shift_star_names: Vec<String>,
    pub diff_diagram_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareCardSpec {
    pub width_px: i32,
    pub height_px: i32,
    pub title_text: String,
    pub subtitle_text: String,
    pub footer_text: String,
    pub accent_color_hex: String,
    pub background_gradient_from_hex: String,
    pub background_gradient_to_hex: String,
    pub render_payload: String,
    pub shareable_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LuckyStarEntry {
    pub star_name_cn: String,
    pub modern_name: Option<String>,
    pub magnitude: f64,
    pub altitude_deg: f64,
    pub azimuth_deg: f64,
    pub distance_pc: Option<f64>,
    pub meaning: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PersonalStarmapShareRequest {
    pub hash: String,
}
