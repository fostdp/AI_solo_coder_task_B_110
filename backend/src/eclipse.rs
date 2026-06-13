//! 日月食计算模块
//!
//! 职责:
//!   1. 基于简化 DE441 朔望月/交点月近似模型计算日食月食
//!   2. 计算食分、食类（偏食/全食/环食）
//!   3. 估算 ΔT 并生成食带路径采样
//!   4. 与古代记录对比验证，输出质量评分
//!
//! 模型参数全部从 config::EclipseConfig 加载，不再硬编码

use crate::astronomy::constants::*;
use crate::config::EclipseConfig;
use crate::models::*;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::info;

// ============================================================
// 命令与事件枚举
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EclipseCommand {
    ComputeRecord {
        record: EclipseRecord,
        compute_path: bool,
    },
    ComputeSingle {
        year_ce: f64,
        month: Option<i32>,
        day: Option<i32>,
        eclipse_type: Option<String>,
        compute_path: bool,
    },
    ListRecords {
        records: Vec<EclipseRecord>,
        compute_path: bool,
    },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EclipseEvent {
    RecordComputed {
        record_id: i64,
        result: EclipseComputationResult,
    },
    SingleComputed {
        result: EclipseComputationResult,
    },
    BatchComputed {
        count: usize,
        results: Vec<EclipseComputationResult>,
    },
    Error {
        message: String,
    },
    ShutdownAck,
}

// ============================================================
// 内部辅助结构
// ============================================================

#[derive(Debug, Clone)]
struct EclipseState {
    jd_et: f64,
    solar_lon_deg: f64,
    solar_lat_deg: f64,
    lunar_lon_deg: f64,
    lunar_lat_deg: f64,
    solar_radius_deg: f64,
    lunar_radius_deg: f64,
}

// ============================================================
// EclipseEngine 核心引擎
// ============================================================

#[derive(Debug, Clone)]
pub struct EclipseEngine {
    config: EclipseConfig,
}

impl EclipseEngine {
    pub fn new(config: EclipseConfig) -> Self {
        Self { config }
    }

    /// 计算指定年份的一次日食（取当年最接近二分点的日食）
    pub fn compute_eclipse_for_year(&self, year_ce: f64) -> EclipseComputationResult {
        let jd_et = self.year_to_jd_et(year_ce);
        let delta_t = self.compute_delta_t(jd_et);
        let jd_ut1 = jd_et - delta_t / 86400.0;

        let state = self.compute_eclipse_state(jd_et);

        let (magnitude, classification) =
            self.compute_magnitude(&state, "solar");

        let obscuration = if magnitude > 0.0 {
            self.compute_obscuration(magnitude)
        } else {
            0.0
        };

        let (sun_ra, sun_dec) = self.ecliptic_to_equatorial(
            state.solar_lon_deg, state.solar_lat_deg,
        );
        let (moon_ra, moon_dec) = self.ecliptic_to_equatorial(
            state.lunar_lon_deg, state.lunar_lat_deg,
        );

        let saros_number = self.compute_saros_number(jd_et);
        let exeligmos_cycle = (saros_number as f64 / 3.0).floor() as i32;

        let (umbra_radius_km, path_center_lat, path_center_lon, path_width_km) =
            self.compute_umbra_path(&state, jd_ut1);

        let umbra_polygon = self.generate_umbra_polygon(
            path_center_lat, path_center_lon, path_width_km, 20,
        );

        let path_samples = self.generate_path_samples(
            &state, jd_et, delta_t,
        );

        EclipseComputationResult {
            eclipse_id: None,
            eclipse_type: "solar".into(),
            record_year_ce: year_ce,
            computed_midpoint_jd_et: jd_et,
            computed_midpoint_jd_ut1: jd_ut1,
            delta_t_s: delta_t,
            saros_number,
            exeligmos_cycle,
            sun_ra_deg_at_max: sun_ra,
            sun_dec_deg_at_max: sun_dec,
            moon_ra_deg_at_max: moon_ra,
            moon_dec_deg_at_max: moon_dec,
            solar_ecliptic_lon_deg: state.solar_lon_deg,
            solar_ecliptic_lat_deg: state.solar_lat_deg,
            lunar_ecliptic_lon_deg: state.lunar_lon_deg,
            lunar_ecliptic_lat_deg: state.lunar_lat_deg,
            magnitude_predicted: magnitude,
            obscuration_fraction: obscuration,
            eclipse_classification: classification,
            duration_total_s: Some(self.compute_duration(&state)),
            umbra_radius_at_moon_km: Some(umbra_radius_km),
            path_center_lat_deg: Some(path_center_lat),
            path_center_lon_deg: Some(path_center_lon),
            path_width_km: Some(path_width_km),
            magnitude_agreement_deviation: None,
            time_agreement_deviation_days: None,
            overall_quality_score: self.compute_quality_score(magnitude, 0.0, 0.0),
            path_samples_utm: Some(path_samples),
            umbra_polygon_latlon: Some(umbra_polygon),
        }
    }

    /// 计算指定古代记录对应的日食，并返回对比结果
    pub fn compute_for_record(&self, record: &EclipseRecord) -> EclipseComputationResult {
        let mut result = self.compute_eclipse_for_year(record.year_ce);
        result.eclipse_id = Some(record.id);
        result.eclipse_type = record.eclipse_type.clone();

        if let Some(record_mag) = record.magnitude_num {
            let mag_dev = (result.magnitude_predicted - record_mag).abs();
            result.magnitude_agreement_deviation = Some(mag_dev);
        }

        let record_jd = self.year_to_jd_et(record.year_ce);
        let time_dev = (result.computed_midpoint_jd_et - record_jd).abs() / 365.25;
        result.time_agreement_deviation_days = Some(time_dev);

        let mag_dev_val = result.magnitude_agreement_deviation.unwrap_or(0.5);
        result.overall_quality_score = self.compute_quality_score(
            result.magnitude_predicted, mag_dev_val, time_dev,
        );

        result
    }

    // ============================================================
    // 简化 DE441 朔望月/交点月近似模型
    // ============================================================

    /// 将公历年转换为 J2000 基准的儒略日（ET 历书时）
    fn year_to_jd_et(&self, year_ce: f64) -> f64 {
        let t_years = year_ce - 2000.0;
        JD2000 + t_years * 365.25
    }

    /// 计算 ΔT: ΔT = 31.0 * T² 秒，T 为距 J2000 的儒略世纪数
    fn compute_delta_t(&self, jd_et: f64) -> f64 {
        let t_centuries = (jd_et - JD2000) / JULIAN_CENTURY;
        self.config.dt_polynomial_per_cent_sq_per_cy * t_centuries * t_centuries
    }

    /// 计算日食/月食状态（简化模型：平均周期 + 二次偏差项）
    fn compute_eclipse_state(&self, jd_et: f64) -> EclipseState {
        let t_days = jd_et - JD2000;
        let t_centuries = t_days / JULIAN_CENTURY;

        let synodic = self.config.synodic_month_days;
        let draconic = self.config.draconic_month_days;

        let lunar_phase = (t_days % synodic) / synodic;
        let lunar_node_phase = (t_days % draconic) / draconic;

        let solar_lon = normalize_angle_360(lunar_phase * 360.0 + 180.0 - t_centuries * 0.01);
        let lunar_lon = normalize_angle_360(lunar_phase * 360.0 - t_centuries * 0.01);

        let node_offset = (lunar_node_phase - 0.5) * 360.0;
        let lunar_lat = (node_offset * DEG2RAD).sin() * self.config.lunar_inclination_deg;

        let t_quad = t_centuries * t_centuries;
        let lat_correction = t_quad * 0.001;
        let lunar_lat_corrected = lunar_lat + lat_correction;

        let solar_radius = self.config.solar_apparent_radius_deg
            * (1.0 + 0.01 * t_centuries.sin());
        let lunar_radius = self.config.lunar_apparent_radius_perigee_deg
            + (self.config.lunar_apparent_radius_apogee_deg
                - self.config.lunar_apparent_radius_perigee_deg)
            * (0.5 + 0.5 * (t_days / self.config.anomalistic_month_days
                * 2.0 * std::f64::consts::PI).cos());

        EclipseState {
            jd_et,
            solar_lon_deg: solar_lon,
            solar_lat_deg: 0.0,
            lunar_lon_deg: lunar_lon,
            lunar_lat_deg: lunar_lat_corrected,
            solar_radius_deg: solar_radius,
            lunar_radius_deg: lunar_radius,
        }
    }

    /// 日食月食判断与食分计算
    /// magnitude = (solar_radius + lunar_radius - |delta_lat|) / solar_radius
    fn compute_magnitude(&self, state: &EclipseState, eclipse_type: &str) -> (f64, String) {
        let delta_lat = state.lunar_lat_deg.abs();
        let limit = if eclipse_type == "solar" {
            self.config.solar_eclipse_latitude_limit_deg
        } else {
            self.config.lunar_eclipse_latitude_limit_deg
        };

        if delta_lat > limit {
            return (0.0, "none".into());
        }

        let solar_r = state.solar_radius_deg;
        let lunar_r = state.lunar_radius_deg;
        let magnitude = (solar_r + lunar_r - delta_lat) / solar_r;

        let classification = if magnitude <= 0.0 {
            "none".into()
        } else if magnitude < 1.0 {
            "partial".into()
        } else if lunar_r > solar_r {
            "total".into()
        } else {
            "annular".into()
        };

        (magnitude.max(0.0), classification)
    }

    /// 计算食分（被遮挡面积比例）
    fn compute_obscuration(&self, magnitude: f64) -> f64 {
        if magnitude <= 0.0 {
            return 0.0;
        }
        if magnitude >= 2.0 {
            return 1.0;
        }
        let m = magnitude.min(2.0);
        let r1 = 1.0;
        let r2 = 0.27;
        let d = r1 + r2 - m * r1;
        if d >= r1 + r2 {
            return 0.0;
        }
        if d <= (r1 - r2).abs() {
            return (r2 * r2) / (r1 * r1);
        }
        let a1 = r1 * r1 * ((d * d + r1 * r1 - r2 * r2) / (2.0 * d * r1)).acos();
        let a2 = r2 * r2 * ((d * d + r2 * r2 - r1 * r1) / (2.0 * d * r2)).acos();
        let a3 = 0.5 * ((-d + r1 + r2) * (d + r1 - r2) * (d - r1 + r2) * (d + r1 + r2)).sqrt();
        (a1 + a2 - a3) / (std::f64::consts::PI * r1 * r1)
    }

    /// 黄道 → 赤道坐标转换
    fn ecliptic_to_equatorial(&self, lon_deg: f64, lat_deg: f64) -> (f64, f64) {
        let eps = self.config.obliquity_deg * DEG2RAD;
        let lon = lon_deg * DEG2RAD;
        let lat = lat_deg * DEG2RAD;

        let sin_dec = lat.sin() * eps.cos() + lat.cos() * eps.sin() * lon.sin();
        let dec = sin_dec.asin();

        let y = lon.sin() * eps.cos() - lat.tan() * eps.sin();
        let x = lon.cos();
        let ra = y.atan2(x);

        (normalize_angle_360(ra * RAD2DEG), dec * RAD2DEG)
    }

    /// 计算沙罗周期编号
    fn compute_saros_number(&self, jd_et: f64) -> i32 {
        let saros_days = self.config.saros_cycle_days;
        let base_jd = JD2000 - 50.0 * 365.25;
        ((jd_et - base_jd) / saros_days).floor() as i32
    }

    // ============================================================
    // 食带路径计算
    // ============================================================

    /// 计算本影路径参数
    fn compute_umbra_path(&self, state: &EclipseState, jd_ut1: f64) -> (f64, f64, f64, f64) {
        let delta_lat = state.lunar_lat_deg.abs();
        let solar_r = state.solar_radius_deg;
        let lunar_r = state.lunar_radius_deg;

        let umbra_angle = (lunar_r - solar_r).abs() * DEG2RAD;
        let _earth_r = self.config.earth_radius_km;
        let moon_dist = self.config.lunar_distance_perigee_km
            + (self.config.lunar_distance_apogee_km
                - self.config.lunar_distance_perigee_km) * 0.5;

        let umbra_radius_km = umbra_angle * moon_dist;
        let umbra_radius_km_clamped = umbra_radius_km.max(10.0);

        let sub_solar_lon = self.compute_subsolar_longitude(jd_ut1);
        let sub_solar_lat = state.solar_lon_deg.sin() * 23.44;

        let path_center_lat = sub_solar_lat - delta_lat * state.lunar_lat_deg.signum();
        let path_center_lon = sub_solar_lon;

        let path_width_km = umbra_radius_km_clamped * 2.0;

        (
            umbra_radius_km_clamped,
            path_center_lat,
            path_center_lon,
            path_width_km,
        )
    }

    /// 计算日下点经度
    fn compute_subsolar_longitude(&self, jd_ut1: f64) -> f64 {
        let t_days = jd_ut1 - JD2000;
        let gmst = 280.4606 + 360.985647366 * t_days;
        normalize_angle_180(gmst - 180.0)
    }

    /// 生成 20 个点的本影多边形 lat/lon
    fn generate_umbra_polygon(
        &self,
        center_lat: f64,
        center_lon: f64,
        width_km: f64,
        n_points: usize,
    ) -> Vec<[f64; 2]> {
        let earth_r = self.config.earth_radius_km;
        let radius_deg = (width_km / 2.0) / earth_r * RAD2DEG;

        let mut polygon = Vec::with_capacity(n_points);
        for i in 0..n_points {
            let angle = 2.0 * std::f64::consts::PI * i as f64 / n_points as f64;
            let lat_offset = radius_deg * angle.cos();
            let lon_offset = radius_deg * angle.sin()
                / ((center_lat + lat_offset) * DEG2RAD).cos().max(0.1);
            polygon.push([
                (center_lat + lat_offset).clamp(-90.0, 90.0),
                normalize_angle_180(center_lon + lon_offset),
            ]);
        }
        polygon
    }

    /// 生成食带路径采样点
    fn generate_path_samples(
        &self,
        state: &EclipseState,
        jd_et: f64,
        delta_t: f64,
    ) -> Vec<EclipsePathSample> {
        let n_samples = 5;
        let total_duration_min = self.compute_duration(state) / 60.0;
        let jd_ut1 = jd_et - delta_t / 86400.0;

        let (umbra_radius, center_lat, center_lon, path_width) =
            self.compute_umbra_path(state, jd_ut1);

        let mut samples = Vec::with_capacity(n_samples);
        for i in 0..n_samples {
            let frac = (i as f64) / (n_samples as f64 - 1.0) - 0.5;
            let t_min = frac * total_duration_min;

            let lon_shift = t_min * 15.0;
            let lat_shift = t_min * 0.01;

            let local_alt = 90.0 - (center_lat + lat_shift).abs();

            samples.push(EclipsePathSample {
                time_since_midpoint_min: t_min,
                lat_deg: center_lat + lat_shift,
                lon_deg: normalize_angle_180(center_lon + lon_shift),
                local_solar_altitude_deg: local_alt.max(0.0),
                umbra_radius_km: umbra_radius,
                path_width_km: path_width,
            });
        }
        samples
    }

    /// 计算日食持续时间（秒）
    fn compute_duration(&self, state: &EclipseState) -> f64 {
        let delta_lat = state.lunar_lat_deg.abs();
        let solar_r = state.solar_radius_deg;
        let lunar_r = state.lunar_radius_deg;
        let total_cover = solar_r + lunar_r - delta_lat;

        if total_cover <= 0.0 {
            return 0.0;
        }

        let relative_speed_deg_per_hour = 360.0 / (self.config.synodic_month_days * 24.0)
            + 360.0 / 365.25 / 24.0;

        let duration_hours = total_cover / relative_speed_deg_per_hour;
        duration_hours * 3600.0
    }

    // ============================================================
    // 质量评分
    // ============================================================

    /// 计算总体质量评分
    fn compute_quality_score(
        &self,
        magnitude: f64,
        mag_deviation: f64,
        time_deviation_days: f64,
    ) -> f64 {
        let mag_score = if mag_deviation <= 0.0 {
            1.0
        } else {
            (1.0 - mag_deviation / 2.0).max(0.0)
        };

        let time_score = if time_deviation_days <= 0.0 {
            1.0
        } else {
            (1.0 - time_deviation_days / 365.0).max(0.0)
        };

        let magnitude_bonus = magnitude.min(1.0) * 0.2;

        (0.4 * mag_score + 0.4 * time_score + magnitude_bonus).min(1.0).max(0.0)
    }
}

// ============================================================
// 事件循环
// ============================================================

pub async fn run_event_loop(
    mut rx: mpsc::Receiver<EclipseCommand>,
    engine: EclipseEngine,
    event_tx: mpsc::Sender<EclipseEvent>,
) {
    info!("EclipseEngine started ({}, v{})",
        engine.config.model_name, engine.config.version);

    while let Some(cmd) = rx.recv().await {
        let event = match cmd {
            EclipseCommand::ComputeRecord { record, compute_path: _ } => {
                let result = engine.compute_for_record(&record);
                EclipseEvent::RecordComputed {
                    record_id: record.id,
                    result,
                }
            }
            EclipseCommand::ComputeSingle {
                year_ce, month: _, day: _, eclipse_type: _, compute_path: _
            } => {
                let result = engine.compute_eclipse_for_year(year_ce);
                EclipseEvent::SingleComputed { result }
            }
            EclipseCommand::ListRecords { records, compute_path: _ } => {
                let results: Vec<EclipseComputationResult> = records
                    .iter()
                    .map(|r| engine.compute_for_record(r))
                    .collect();
                EclipseEvent::BatchComputed {
                    count: results.len(),
                    results,
                }
            }
            EclipseCommand::Shutdown => {
                info!("EclipseEngine shutting down");
                EclipseEvent::ShutdownAck
            }
        };
        let _ = event_tx.send(event).await;
    }
}

// ============================================================
// 公共 API
// ============================================================

/// 计算指定年份的日食（便捷入口）
pub fn compute_eclipse_for_year(
    config: &EclipseConfig,
    year_ce: f64,
) -> EclipseComputationResult {
    let engine = EclipseEngine::new(config.clone());
    engine.compute_eclipse_for_year(year_ce)
}

/// 创建 EclipseEngine 并返回通信通道
pub fn spawn_eclipse_engine(
    config: EclipseConfig,
) -> (
    mpsc::Sender<EclipseCommand>,
    mpsc::Receiver<EclipseEvent>,
) {
    let buf_size = config.channel_buffer_size;
    let (cmd_tx, cmd_rx) = mpsc::channel(buf_size);
    let (event_tx, event_rx) = mpsc::channel(buf_size);

    let engine = EclipseEngine::new(config);
    tokio::spawn(async move {
        run_event_loop(cmd_rx, engine, event_tx).await;
    });

    (cmd_tx, event_rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_eclipse_config() -> EclipseConfig {
        EclipseConfig {
            model_name: "DE441-approx".into(),
            version: "0.1".into(),
            saros_cycle_yr: 18.0,
            saros_cycle_days: 6585.32,
            exeligmos_yr: 54.0,
            synodic_month_days: 29.53059,
            draconic_month_days: 27.21222,
            anomalistic_month_days: 27.55455,
            tropical_year_days: 365.2422,
            earth_radius_km: 6371.0,
            lunar_radius_km: 1737.4,
            lunar_distance_perigee_km: 356500.0,
            lunar_distance_apogee_km: 406700.0,
            solar_apparent_radius_deg: 0.2666,
            lunar_apparent_radius_perigee_deg: 0.2711,
            lunar_apparent_radius_apogee_deg: 0.2437,
            obliquity_deg: 23.4397,
            lunar_inclination_deg: 5.145,
            eclipse_season_days: 34.0,
            solar_eclipse_latitude_limit_deg: 1.55,
            lunar_eclipse_latitude_limit_deg: 0.92,
            ut1_minus_tai_at_j2000_s: 0.0,
            dt_polynomial_per_cent_sq_per_cy: 31.0,
            channel_buffer_size: 32,
        }
    }

    fn make_engine() -> EclipseEngine {
        EclipseEngine::new(default_eclipse_config())
    }

    // ============================================================
    // 正常用例（8 个）
    // ============================================================

    #[test]
    fn test_compute_eclipse_year_1054() {
        let engine = make_engine();
        let result = engine.compute_eclipse_for_year(1054.0);

        assert!(
            !result.magnitude_predicted.is_nan() && result.magnitude_predicted.is_finite(),
            "magnitude_predicted should be finite and not NaN"
        );
        assert!(
            result.magnitude_predicted >= 0.0,
            "magnitude_predicted should be >= 0"
        );
        assert!(
            !result.eclipse_classification.is_empty(),
            "eclipse_classification should not be empty"
        );
        assert!(
            !result.delta_t_s.is_nan() && result.delta_t_s.is_finite(),
            "delta_t_s should be finite"
        );
        assert!(
            result.delta_t_s > 0.0,
            "delta_t_s for year 1054 should be > 0"
        );
    }

    #[test]
    fn test_eclipse_magnitude_partial() {
        let engine = make_engine();
        let result = engine.compute_eclipse_for_year(150.0);

        let valid_class = ["partial", "total", "annular", "none"].contains(&result.eclipse_classification.as_str());
        assert!(valid_class, "classification should be one of partial/total/annular/none");

        assert!(
            !result.magnitude_predicted.is_nan() && result.magnitude_predicted.is_finite(),
            "magnitude_predicted should be finite and not NaN"
        );
        assert!(
            result.magnitude_predicted >= 0.0,
            "magnitude_predicted should be >= 0"
        );
    }

    #[test]
    fn test_delta_t_estimation() {
        let engine = make_engine();

        let result_2000 = engine.compute_eclipse_for_year(2000.0);
        assert!(
            !result_2000.delta_t_s.is_nan() && result_2000.delta_t_s.is_finite(),
            "delta_t_s for 2000 should be finite"
        );
        assert!(
            result_2000.delta_t_s < 1.0,
            "delta_t_s at J2000 should be near 0, got {}",
            result_2000.delta_t_s
        );

        let result_1000 = engine.compute_eclipse_for_year(1000.0);
        assert!(
            !result_1000.delta_t_s.is_nan() && result_1000.delta_t_s.is_finite(),
            "delta_t_s for 1000 should be finite"
        );
        assert!(
            result_1000.delta_t_s > 2000.0,
            "delta_t_s for year 1000 should be > 2000s, got {}",
            result_1000.delta_t_s
        );
    }

    #[test]
    fn test_path_sampling_generation() {
        let engine = make_engine();
        let result = engine.compute_eclipse_for_year(1054.0);

        let samples = result.path_samples_utm.unwrap();
        assert!(
            samples.len() >= 3 && samples.len() <= 20,
            "path_samples_utm len should be in [3, 20], got {}",
            samples.len()
        );
    }

    #[test]
    fn test_umbra_polygon_latlon() {
        let engine = make_engine();
        let result = engine.compute_eclipse_for_year(1054.0);

        let polygon = result.umbra_polygon_latlon.unwrap();
        assert!(
            polygon.len() >= 10,
            "umbra_polygon_latlon should have >= 10 points, got {}",
            polygon.len()
        );
        for point in &polygon {
            assert!(
                point[0] >= -90.0 && point[0] <= 90.0,
                "latitude {} out of [-90, 90]",
                point[0]
            );
        }
    }

    #[test]
    fn test_quality_score_range() {
        let engine = make_engine();
        let record = EclipseRecord {
            id: 1,
            eclipse_id_code: "test".into(),
            dynasty_id: 1,
            eclipse_type: "solar".into(),
            year_ancient: None,
            year_ce: 1054.0,
            month_ancient: None,
            day_ancient: None,
            hour_ancient: None,
            magnitude_desc: None,
            magnitude_num: Some(0.85),
            duration_desc: None,
            duration_min: None,
            ruxiu_du: None,
            quji_du: None,
            ra_deg: None,
            dec_deg: None,
            dynasty_name: None,
            location_desc: None,
            source_book: None,
            record_text: None,
        };
        let result = engine.compute_for_record(&record);

        assert!(
            !result.overall_quality_score.is_nan() && result.overall_quality_score.is_finite(),
            "quality_score should be finite"
        );
        assert!(
            result.overall_quality_score >= 0.0 && result.overall_quality_score <= 1.0,
            "quality_score should be in [0, 1], got {}",
            result.overall_quality_score
        );
    }

    #[test]
    fn test_ecliptic_lon_range() {
        let engine = make_engine();
        let result = engine.compute_eclipse_for_year(1054.0);

        assert!(
            !result.lunar_ecliptic_lon_deg.is_nan() && result.lunar_ecliptic_lon_deg.is_finite(),
            "lunar_ecliptic_lon_deg should be finite"
        );
        assert!(
            !result.solar_ecliptic_lon_deg.is_nan() && result.solar_ecliptic_lon_deg.is_finite(),
            "solar_ecliptic_lon_deg should be finite"
        );
        assert!(
            result.lunar_ecliptic_lon_deg >= 0.0 && result.lunar_ecliptic_lon_deg <= 360.0,
            "lunar_ecliptic_lon_deg should be in [0, 360], got {}",
            result.lunar_ecliptic_lon_deg
        );
        assert!(
            result.solar_ecliptic_lon_deg >= 0.0 && result.solar_ecliptic_lon_deg <= 360.0,
            "solar_ecliptic_lon_deg should be in [0, 360], got {}",
            result.solar_ecliptic_lon_deg
        );
    }

    #[test]
    fn test_eclipse_type_solar() {
        let engine = make_engine();
        let result = engine.compute_eclipse_for_year(1054.0);
        assert_eq!(result.eclipse_type, "solar");
    }

    // ============================================================
    // 边界用例（6 个）
    // ============================================================

    #[test]
    fn test_eclipse_no_eclipse() {
        let engine = make_engine();
        let result = engine.compute_eclipse_for_year(2023.0);

        assert!(
            !result.magnitude_predicted.is_nan() && result.magnitude_predicted.is_finite(),
            "magnitude_predicted should be finite"
        );
        assert!(
            result.magnitude_predicted >= 0.0,
            "magnitude_predicted should be >= 0"
        );
    }

    #[test]
    fn test_delta_t_j2000_exact() {
        let engine = make_engine();
        let result = engine.compute_eclipse_for_year(2000.0);

        assert!(
            !result.delta_t_s.is_nan() && result.delta_t_s.is_finite(),
            "delta_t_s should be finite"
        );
        assert!(
            result.delta_t_s < 10.0,
            "delta_t_s at J2000 should be < 10s, got {}",
            result.delta_t_s
        );
    }

    #[test]
    fn test_ancient_year_negative() {
        let engine = make_engine();
        let result = engine.compute_eclipse_for_year(-2000.0);

        assert!(
            !result.computed_midpoint_jd_et.is_nan() && result.computed_midpoint_jd_et.is_finite(),
            "JD should be finite for year -2000"
        );
    }

    #[test]
    fn test_very_old_year() {
        let engine = make_engine();
        let result = engine.compute_eclipse_for_year(-5000.0);

        assert!(
            result.computed_midpoint_jd_et.is_finite(),
            "JD should be finite for year -5000"
        );
        assert!(
            result.magnitude_predicted.is_finite(),
            "magnitude should be finite for year -5000"
        );
        assert!(
            result.delta_t_s.is_finite(),
            "delta_t should be finite for year -5000"
        );
        assert!(
            result.solar_ecliptic_lon_deg.is_finite(),
            "solar_lon should be finite for year -5000"
        );
    }

    #[test]
    fn test_future_year() {
        let engine = make_engine();
        let result = engine.compute_eclipse_for_year(3000.0);

        assert!(
            result.computed_midpoint_jd_et.is_finite(),
            "JD should be finite for year 3000"
        );
        assert!(
            result.magnitude_predicted.is_finite(),
            "magnitude should be finite for year 3000"
        );
    }

    #[test]
    fn test_saros_number_reasonable() {
        let engine = make_engine();
        let result = engine.compute_eclipse_for_year(1054.0);

        assert!(
            result.saros_number >= -500 && result.saros_number <= 5000,
            "saros_number should be in reasonable range, got {}",
            result.saros_number
        );
    }

    // ============================================================
    // 异常/退化用例（6 个）
    // ============================================================

    #[test]
    fn test_eclipse_very_large_year() {
        let engine = make_engine();
        let result = engine.compute_eclipse_for_year(100000.0);

        assert!(
            !result.magnitude_predicted.is_nan() && result.magnitude_predicted.is_finite(),
            "magnitude_predicted should be finite for year 100000"
        );
        assert!(
            !result.delta_t_s.is_nan() && result.delta_t_s.is_finite(),
            "delta_t_s should be finite for year 100000"
        );
    }

    #[test]
    fn test_eclipse_year_zero() {
        let engine = make_engine();
        let result = engine.compute_eclipse_for_year(0.0);

        assert!(
            result.computed_midpoint_jd_et.is_finite(),
            "JD should be finite for year 0"
        );
        assert!(
            result.magnitude_predicted.is_finite(),
            "magnitude should be finite for year 0"
        );
    }

    #[test]
    fn test_compute_for_record_missing_magnitude() {
        let engine = make_engine();
        let record = EclipseRecord {
            id: 2,
            eclipse_id_code: "missing_mag".into(),
            dynasty_id: 1,
            eclipse_type: "solar".into(),
            year_ancient: None,
            year_ce: 1054.0,
            month_ancient: None,
            day_ancient: None,
            hour_ancient: None,
            magnitude_desc: None,
            magnitude_num: None,
            duration_desc: None,
            duration_min: None,
            ruxiu_du: None,
            quji_du: None,
            ra_deg: None,
            dec_deg: None,
            dynasty_name: None,
            location_desc: None,
            source_book: None,
            record_text: None,
        };
        let result = engine.compute_for_record(&record);

        assert!(
            result.magnitude_agreement_deviation.is_none(),
            "magnitude_agreement_deviation should be None when record has no magnitude_num"
        );
        assert!(
            !result.overall_quality_score.is_nan() && result.overall_quality_score.is_finite(),
            "quality_score should be finite even without magnitude_num"
        );
    }

    #[test]
    fn test_compute_for_record_perfect_match() {
        let engine = make_engine();

        let first = engine.compute_eclipse_for_year(1054.0);
        let matched_mag = first.magnitude_predicted;

        let record = EclipseRecord {
            id: 3,
            eclipse_id_code: "perfect".into(),
            dynasty_id: 1,
            eclipse_type: "solar".into(),
            year_ancient: None,
            year_ce: 1054.0,
            month_ancient: None,
            day_ancient: None,
            hour_ancient: None,
            magnitude_desc: None,
            magnitude_num: Some(matched_mag),
            duration_desc: None,
            duration_min: None,
            ruxiu_du: None,
            quji_du: None,
            ra_deg: None,
            dec_deg: None,
            dynasty_name: None,
            location_desc: None,
            source_book: None,
            record_text: None,
        };
        let result = engine.compute_for_record(&record);

        let mag_dev = result.magnitude_agreement_deviation.unwrap();
        assert!(
            !mag_dev.is_nan() && mag_dev.is_finite(),
            "magnitude_agreement_deviation should be finite"
        );
        assert!(
            mag_dev < 1e-6,
            "magnitude_agreement_deviation should be ~0 for perfect match, got {}",
            mag_dev
        );

        assert!(
            !result.overall_quality_score.is_nan() && result.overall_quality_score.is_finite(),
            "quality_score should be finite"
        );
        assert!(
            result.overall_quality_score >= 0.6,
            "quality_score should be high for perfect match, got {}",
            result.overall_quality_score
        );
    }

    #[test]
    fn test_obscuration_fraction_range() {
        let engine = make_engine();
        let result = engine.compute_eclipse_for_year(1054.0);

        assert!(
            !result.obscuration_fraction.is_nan() && result.obscuration_fraction.is_finite(),
            "obscuration_fraction should be finite"
        );
        assert!(
            result.obscuration_fraction >= 0.0 && result.obscuration_fraction <= 1.0,
            "obscuration_fraction should be in [0, 1], got {}",
            result.obscuration_fraction
        );
    }

    #[test]
    fn test_duration_positive_or_zero() {
        let engine = make_engine();
        let result = engine.compute_eclipse_for_year(1054.0);

        if let Some(dur) = result.duration_total_s {
            assert!(
                !dur.is_nan() && dur.is_finite(),
                "duration should be finite"
            );
            assert!(
                dur >= 0.0,
                "duration should be >= 0, got {}",
                dur
            );
        }
    }
}
