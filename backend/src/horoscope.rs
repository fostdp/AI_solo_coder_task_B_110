//! 星占科普交互模块
//! 职责：个人星图生成、行星位置近似、二十八宿边界、古今星空对比、幸运星挑选、分享卡片规格生成

use crate::astronomy::constants::*;
use crate::config::HoroscopeConfig;
use crate::models::*;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::info;

#[derive(Debug, Clone, Deserialize)]
pub enum HoroscopeCommand {
    GenerateStarmap {
        request: PersonalStarmapRequest,
        stars: Vec<AncientStar>,
        mansions: Vec<LunarMansion>,
    },
    GetShareCard {
        starmap: Box<PersonalStarmapResponse>,
    },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HoroscopeEvent {
    StarmapGenerated(Box<PersonalStarmapResponse>),
    ShareCardGenerated(Box<ShareCardSpec>),
    Error {
        message: String,
    },
    ShutdownAck,
}

pub struct HoroscopeEngine {
    config: HoroscopeConfig,
}

impl HoroscopeEngine {
    pub fn new(config: HoroscopeConfig) -> Self {
        Self { config }
    }

    fn julian_day_gregorian(year: i32, month: i32, day: i32, hour_utc: f64) -> f64 {
        let y = if month <= 2 { year - 1 } else { year };
        let m = if month <= 2 { month + 12 } else { month };
        let d = day as f64 + hour_utc / 24.0;
        let a = (y as f64 / 100.0).floor();
        let b = 2.0 - a + (a / 4.0).floor();
        (365.25 * (y as f64 + 4716.0)).floor()
            + (30.6001 * (m as f64 + 1.0)).floor()
            + d + b - 1524.5
    }

    fn gmst_deg(jd_ut1: f64) -> f64 {
        let d = jd_ut1 - JD2000;
        let t = d / JULIAN_CENTURY;
        let gmst_sec = 24110.54841
            + 8640184.812866 * t
            + 0.093104 * t * t
            - 0.0000062 * t * t * t;
        let gmst_hours = (gmst_sec / 3600.0) % 24.0;
        let gmst_h = if gmst_hours < 0.0 {
            gmst_hours + 24.0
        } else {
            gmst_hours
        };
        gmst_h * 15.0
    }

    fn local_sidereal_time_deg(jd_ut1: f64, longitude_deg: f64) -> f64 {
        let gmst = Self::gmst_deg(jd_ut1);
        normalize_angle_360(gmst + longitude_deg)
    }

    fn sun_ecliptic_position(jd_ut1: f64) -> (f64, f64) {
        let d = jd_ut1 - JD2000;
        let mean_longitude = 280.460 + 0.9856474 * d;
        let mean_anomaly = 357.528 + 0.9856003 * d;
        let m = mean_anomaly * DEG2RAD;
        let ecliptic_lon = mean_longitude
            + 1.915 * m.sin()
            + 0.020 * (2.0 * m).sin();
        (normalize_angle_360(ecliptic_lon), 0.0)
    }

    fn moon_ecliptic_position(jd_ut1: f64) -> (f64, f64) {
        let d = jd_ut1 - JD2000;
        let lunar_mean_longitude = 218.316 + 13.176396 * d;
        let lunar_mean_anomaly = 134.963 + 13.064993 * d;
        let lunar_mean_distance = 93.272 + 13.229350 * d;
        let lunar_ascending_node = 125.045 - 0.052954 * d;
        let m_moon = lunar_mean_anomaly * DEG2RAD;
        let m_sun = (357.528 + 0.9856003 * d) * DEG2RAD;
        let d_elong = (297.850 + 12.190750 * d) * DEG2RAD;
        let f = lunar_mean_distance * DEG2RAD;
        let evection = 1.274 * (2.0 * d_elong - m_moon).sin();
        let variation = 0.658 * (2.0 * d_elong).sin();
        let annual_eq = 0.186 * m_sun.sin();
        let reduction_eq = 0.214 * (2.0 * f).sin();
        let ecliptic_lon =
            lunar_mean_longitude + evection + variation + annual_eq - reduction_eq;
        let latitude_amp = 5.128;
        let ecliptic_lat =
            latitude_amp * (lunar_mean_distance - lunar_ascending_node).to_radians().sin();
        (normalize_angle_360(ecliptic_lon), ecliptic_lat)
    }

    fn planet_ecliptic_position(jd_ut1: f64, planet: &str) -> (f64, f64) {
        let d = jd_ut1 - JD2000;
        let (synodic_days, lon_at_j2000, eccentricity, arg_perihelion): (f64, f64, f64, f64) = match planet {
            "mercury" => (115.88, 252.25, 0.2056, 77.46),
            "venus" => (583.92, 181.98, 0.0068, 131.56),
            "mars" => (779.94, 355.43, 0.0934, 336.06),
            "jupiter" => (398.88, 34.35, 0.0490, 14.33),
            "saturn" => (378.09, 50.08, 0.0568, 93.06),
            _ => (365.25, 0.0, 0.0, 0.0),
        };
        let mean_anomaly = 360.0 * (d % synodic_days) / synodic_days;
        let m = mean_anomaly * DEG2RAD;
        let e2 = eccentricity * eccentricity;
        let e3 = eccentricity * eccentricity * eccentricity;
        let center_eq = (2.0 * eccentricity - 0.25 * e3) * m.sin()
            + 1.25 * e2 * (2.0 * m).sin()
            + (13.0 / 12.0) * e3 * (3.0 * m).sin();
        let true_anomaly = mean_anomaly + center_eq;
        let ecliptic_lon = lon_at_j2000 + arg_perihelion + true_anomaly;
        let inclination = match planet {
            "mercury" => 7.00,
            "venus" => 3.39,
            "mars" => 1.85,
            "jupiter" => 1.31,
            "saturn" => 2.49,
            _ => 0.0,
        };
        let ascending_node = match planet {
            "mercury" => 48.33,
            "venus" => 76.68,
            "mars" => 49.56,
            "jupiter" => 100.49,
            "saturn" => 113.64,
            _ => 0.0,
        };
        let arg_latitude_amp = inclination;
        let delta_lon = ecliptic_lon - ascending_node;
        let ecliptic_lat = arg_latitude_amp * (delta_lon * DEG2RAD).sin();
        (normalize_angle_360(ecliptic_lon), ecliptic_lat)
    }

    fn ecliptic_to_equatorial(lon_deg: f64, lat_deg: f64, eps_deg: f64) -> (f64, f64) {
        let lon = lon_deg * DEG2RAD;
        let lat = lat_deg * DEG2RAD;
        let eps = eps_deg * DEG2RAD;
        let sin_ra = lon.sin() * eps.cos() - lat.tan() * eps.sin();
        let cos_ra = lon.cos();
        let ra = sin_ra.atan2(cos_ra) * RAD2DEG;
        let sin_dec = lat.sin() * eps.cos() + lat.cos() * eps.sin() * lon.sin();
        let dec = sin_dec.asin() * RAD2DEG;
        (normalize_angle_360(ra), dec)
    }

    fn equatorial_to_horizontal(
        ra_deg: f64,
        dec_deg: f64,
        lst_deg: f64,
        latitude_deg: f64,
    ) -> (f64, f64) {
        let ha = normalize_angle_180(lst_deg - ra_deg) * DEG2RAD;
        let dec = dec_deg * DEG2RAD;
        let phi = latitude_deg * DEG2RAD;
        let sin_alt = phi.sin() * dec.sin() + phi.cos() * dec.cos() * ha.cos();
        let alt = sin_alt.asin() * RAD2DEG;
        let cos_alt = (alt * DEG2RAD).cos();
        let sin_az = -(dec.sin() * phi.cos() - dec.cos() * phi.sin() * ha.cos()) / cos_alt;
        let cos_az = (dec.cos() * ha.sin()) / cos_alt;
        let az = sin_az.atan2(cos_az) * RAD2DEG;
        (alt, normalize_angle_360(az))
    }

    fn stereographic_project(alt_deg: f64, az_deg: f64, scale: f64) -> (f64, f64) {
        let alt_clamped = alt_deg.max(-5.0);
        let z = (90.0 - alt_clamped) * DEG2RAD;
        let az = az_deg * DEG2RAD;
        let r = 2.0 * scale * (z / 2.0).tan();
        let x = r * az.sin();
        let y = -r * az.cos();
        (x, y)
    }

    fn compute_airmass(alt_deg: f64) -> f64 {
        let z = 90.0 - alt_deg.max(0.0);
        let z_rad = z * DEG2RAD;
        1.0 / z_rad.cos().max(0.01)
    }

    fn zodiac_sign_from_ecliptic_lon(lon_deg: f64) -> &'static str {
        let lon = normalize_angle_360(lon_deg);
        let idx = (lon / 30.0).floor() as usize;
        match idx {
            0 => "白羊座",
            1 => "金牛座",
            2 => "双子座",
            3 => "巨蟹座",
            4 => "狮子座",
            5 => "处女座",
            6 => "天秤座",
            7 => "天蝎座",
            8 => "射手座",
            9 => "摩羯座",
            10 => "水瓶座",
            11 => "双鱼座",
            _ => "白羊座",
        }
    }

    fn lunar_mansion_from_ra<'a>(ra_deg: f64, mansions: &'a [LunarMansion]) -> &'a str {
        let ra = normalize_angle_360(ra_deg);
        for m in mansions {
            let start = m.ra_start_deg;
            let end = m.ra_end_deg;
            if end >= start {
                if ra >= start && ra < end {
                    return &m.name_cn;
                }
            } else if ra >= start || ra < end {
                return &m.name_cn;
            }
        }
        if let Some(first) = mansions.first() {
            &first.name_cn
        } else {
            "角宿"
        }
    }

    fn lucky_star_pick(
        stars: &[StarmapStar],
        altitudes: &[f64],
        azimuths: &[f64],
    ) -> Vec<LuckyStarEntry> {
        let meanings = [
            "事业亨通，贵人相助",
            "财运亨通，金玉满堂",
            "健康长寿，身心康泰",
            "姻缘美满，琴瑟和鸣",
            "学业有成，金榜题名",
            "平安顺遂，吉星高照",
            "智慧开明，灵感涌现",
            "福德圆满，所愿皆成",
        ];
        let mut candidates: Vec<(usize, &StarmapStar, f64, f64)> = Vec::new();
        for (i, s) in stars.iter().enumerate() {
            let alt = if i < altitudes.len() {
                altitudes[i]
            } else {
                s.altitude_at_birth_deg
            };
            let az = if i < azimuths.len() {
                azimuths[i]
            } else {
                s.azimuth_at_birth_deg
            };
            if s.apparent_magnitude <= 3.0 && alt > 30.0 {
                candidates.push((i, s, alt, az));
            }
        }
        candidates.sort_by(|a, b| a.1.apparent_magnitude.partial_cmp(&b.1.apparent_magnitude).unwrap());
        candidates
            .into_iter()
            .take(8)
            .enumerate()
            .map(|(mi, (_, s, alt, az))| LuckyStarEntry {
                star_name_cn: s.ancient_name_cn.clone().unwrap_or_else(|| {
                    s.modern_name.clone().unwrap_or_else(|| "吉星".into())
                }),
                modern_name: s.modern_name.clone(),
                magnitude: s.apparent_magnitude,
                altitude_deg: alt,
                azimuth_deg: az,
                distance_pc: None,
                meaning: meanings[mi % meanings.len()].to_string(),
            })
            .collect()
    }

    fn generate(
        &mut self,
        request: PersonalStarmapRequest,
        stars: Vec<AncientStar>,
        mansions: Vec<LunarMansion>,
    ) -> PersonalStarmapResponse {
        info!(
            "Generating personal starmap for {}-{}-{}",
            request.birth_year, request.birth_month, request.birth_day
        );

        let hour_utc = request.birth_hour_utc.unwrap_or(12.0);
        let lat = request
            .latitude_deg
            .unwrap_or(self.config.location.default_latitude_deg);
        let lon = request
            .longitude_deg
            .unwrap_or(self.config.location.default_longitude_deg);
        let city = request
            .city_name
            .clone()
            .unwrap_or(self.config.location.default_city_name.clone());
        let mag_limit = request.mag_limit.unwrap_or(6.0);
        let projection_mode = request
            .projection_mode
            .clone()
            .unwrap_or("stereographic".into());

        let jd = Self::julian_day_gregorian(
            request.birth_year,
            request.birth_month,
            request.birth_day,
            hour_utc,
        );
        let lst = Self::local_sidereal_time_deg(jd, lon);
        let eps = self.config.planet_ephemeris_approx.mean_obliquity_deg;

        let (sun_lon, _) = Self::sun_ecliptic_position(jd);
        let (moon_lon, moon_lat) = Self::moon_ecliptic_position(jd);
        let (sun_ra, _sun_dec) = Self::ecliptic_to_equatorial(sun_lon, 0.0, eps);
        let (moon_ra, _moon_dec) = Self::ecliptic_to_equatorial(moon_lon, moon_lat, eps);

        let zodiac_sun = Self::zodiac_sign_from_ecliptic_lon(sun_lon).to_string();
        let zodiac_moon = Self::zodiac_sign_from_ecliptic_lon(moon_lon).to_string();
        let mansion_sun = Self::lunar_mansion_from_ra(sun_ra, &mansions).to_string();
        let mansion_moon = Self::lunar_mansion_from_ra(moon_ra, &mansions).to_string();

        let personal_info = PersonalInfo {
            birth_date_ymd: [request.birth_year, request.birth_month, request.birth_day],
            birth_hour_utc_decimal: hour_utc,
            latitude_deg: lat,
            longitude_deg: lon,
            city_name: city,
            zodiacal_sun_sign: zodiac_sun,
            zodiacal_moon_sign: zodiac_moon,
            lunar_mansion_sun: mansion_sun,
            lunar_mansion_moon: mansion_moon,
        };

        let birth_datetime_iso = format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:00Z",
            request.birth_year,
            request.birth_month,
            request.birth_day,
            hour_utc.floor() as i32,
            ((hour_utc - hour_utc.floor()) * 60.0).floor() as i32
        );

        let projection_scale = self.config.epoch_alignment.projection_stereographic_scale;

        let mut starmap_stars: Vec<StarmapStar> = Vec::new();
        let mut star_alts: Vec<f64> = Vec::new();
        let mut star_azs: Vec<f64> = Vec::new();

        for ancient in &stars {
            let ra_j2000 = match ancient.ra_j2000 {
                Some(v) => v,
                None => continue,
            };
            let dec_j2000 = match ancient.dec_j2000 {
                Some(v) => v,
                None => continue,
            };
            let mag = ancient.magnitude_num.unwrap_or(6.0);
            if mag > mag_limit {
                continue;
            }
            let (alt, az) = Self::equatorial_to_horizontal(ra_j2000, dec_j2000, lst, lat);
            if alt < self.config.location.min_altitude_for_plot_deg {
                continue;
            }
            let (px, py) = Self::stereographic_project(alt, az, projection_scale);
            let extinction =
                self.config.location.atmospheric_extinction_coeff_per_airmass * Self::compute_airmass(alt);
            let app_mag = mag + extinction;
            starmap_stars.push(StarmapStar {
                star_id: Some(ancient.id),
                modern_name: Some(ancient.star_id_code.clone()),
                ancient_name_cn: Some(ancient.star_name_cn.clone()),
                ra_j2000_deg: ra_j2000,
                dec_j2000_deg: dec_j2000,
                ra_at_birth_deg: ra_j2000,
                dec_at_birth_deg: dec_j2000,
                altitude_at_birth_deg: alt,
                azimuth_at_birth_deg: az,
                projected_x: px,
                projected_y: py,
                apparent_magnitude: app_mag,
                color_temp_k: ancient.color_temp_k,
                magnitude_ancient_desc: ancient.magnitude_ancient.clone(),
            });
            star_alts.push(alt);
            star_azs.push(az);
        }

        let mut solar_system: Vec<SolarSystemBody> = Vec::new();
        let bodies = [
            ("sun", "太阳", -26.74, 1920.0),
            ("moon", "月亮", -12.6, 1872.0),
            ("mercury", "水星", 0.23, 6.7),
            ("venus", "金星", -4.4, 59.0),
            ("mars", "火星", -2.0, 8.0),
            ("jupiter", "木星", -2.7, 47.0),
            ("saturn", "土星", 0.5, 42.0),
        ];
        for (key, name_cn, default_mag, default_diam) in bodies {
            let (eclon, eclat) = if key == "sun" {
                Self::sun_ecliptic_position(jd)
            } else if key == "moon" {
                Self::moon_ecliptic_position(jd)
            } else {
                Self::planet_ecliptic_position(jd, key)
            };
            let (ra, dec) = Self::ecliptic_to_equatorial(eclon, eclat, eps);
            let (alt, az) = Self::equatorial_to_horizontal(ra, dec, lst, lat);
            let (px, py) = Self::stereographic_project(alt, az, projection_scale);
            let phase = if key == "moon" {
                let d = jd - JD2000;
                let syn_phase = ((d % 29.530588) / 29.530588 * 2.0 * std::f64::consts::PI).cos();
                Some(0.5 + 0.5 * syn_phase)
            } else if key == "venus" || key == "mercury" {
                Some(0.75)
            } else {
                None
            };
            solar_system.push(SolarSystemBody {
                body_name_en: key.to_string(),
                body_name_cn: name_cn.to_string(),
                ra_deg: ra,
                dec_deg: dec,
                ecliptic_lon_deg: eclon,
                ecliptic_lat_deg: eclat,
                altitude_deg: alt,
                azimuth_deg: az,
                apparent_magnitude: default_mag,
                angular_diameter_arcsec: default_diam,
                projected_x: px,
                projected_y: py,
                phase_fraction: phase,
            });
        }

        let mansion_boundaries: Option<Vec<LunarMansionBoundary>> =
            if request.show_lunar_mansions.unwrap_or(true) {
                let mut boundaries = Vec::new();
                for m in &mansions {
                    let mut samples = Vec::new();
                    let dec_steps = [-60.0, -30.0, 0.0, 30.0, 60.0, 89.0];
                    for dec_s in dec_steps {
                        samples.push([m.ra_start_deg, dec_s]);
                    }
                    boundaries.push(LunarMansionBoundary {
                        mansion_name_cn: m.name_cn.clone(),
                        ra_start_deg_at_epoch: m.ra_start_deg,
                        ra_end_deg_at_epoch: m.ra_end_deg,
                        dec_samples: samples,
                    });
                }
                Some(boundaries)
            } else {
                None
            };

        let ancient_compare: Option<AncientStarmapDiff> =
            if let Some(ancient_yr) = request.compare_with_ancient_epoch {
                let delta_yr = request.birth_year as f64 - ancient_yr;
                let ancient_jd = jd - delta_yr * 365.2425;
                let (a_sun_lon, _) = Self::sun_ecliptic_position(ancient_jd);
                let (a_moon_lon, _) = Self::moon_ecliptic_position(ancient_jd);
                let (a_sun_ra, _) = Self::ecliptic_to_equatorial(a_sun_lon, 0.0, eps);
                let (a_moon_ra, _) = Self::ecliptic_to_equatorial(a_moon_lon, 0.0, eps);
                let avg_shift = (delta_yr.abs() * 50.3 / 60.0).min(600.0);
                let shifted_count = (starmap_stars.len() as f64 * 0.6) as usize;
                Some(AncientStarmapDiff {
                    ancient_epoch_yr: ancient_yr,
                    ancient_sun_lunar_mansion: Self::lunar_mansion_from_ra(a_sun_ra, &mansions)
                        .to_string(),
                    ancient_moon_lunar_mansion: Self::lunar_mansion_from_ra(a_moon_ra, &mansions)
                        .to_string(),
                    num_stars_shifted_gt_1deg: shifted_count,
                    avg_angular_shift_arcmin: avg_shift,
                    max_shift_star_names: starmap_stars
                        .iter()
                        .take(3)
                        .filter_map(|s| s.ancient_name_cn.clone())
                        .collect(),
                    diff_diagram_json: None,
                })
            } else {
                None
            };

        let lucky = Self::lucky_star_pick(&starmap_stars, &star_alts, &star_azs);

        let share_card: Option<ShareCardSpec> =
            if request.generate_share_card.unwrap_or(true) {
                let hash_input = format!(
                    "{}-{}-{}-{}-{}-{}",
                    request.birth_year,
                    request.birth_month,
                    request.birth_day,
                    hour_utc,
                    lat,
                    lon
                );
                let mut hash_val: u64 = 5381;
                for b in hash_input.bytes() {
                    hash_val = hash_val
                        .wrapping_mul(33)
                        .wrapping_add(b as u64);
                }
                Some(ShareCardSpec {
                    width_px: self.config.card.width_px,
                    height_px: self.config.card.height_px,
                    title_text: format!("{} 的专属星图", personal_info.city_name),
                    subtitle_text: format!(
                        "太阳于{} · 月亮于{}",
                        personal_info.zodiacal_sun_sign, personal_info.zodiacal_moon_sign
                    ),
                    footer_text: "古代星表数字化系统 · 科普版".to_string(),
                    accent_color_hex: self.config.card.accent_color.clone(),
                    background_gradient_from_hex: self
                        .config
                        .card
                        .background_gradient_from
                        .clone(),
                    background_gradient_to_hex: self.config.card.background_gradient_to.clone(),
                    render_payload: serde_json::json!({
                        "stars_count": starmap_stars.len(),
                        "planets": solar_system.len(),
                        "mode": projection_mode,
                    })
                    .to_string(),
                    shareable_hash: format!("{:016x}", hash_val),
                })
            } else {
                None
            };

        let mut notable: Vec<String> = Vec::new();
        notable.push(format!(
            "太阳黄道星座：{}，对应星宿：{}",
            personal_info.zodiacal_sun_sign, personal_info.lunar_mansion_sun
        ));
        notable.push(format!(
            "月亮黄道星座：{}，对应星宿：{}",
            personal_info.zodiacal_moon_sign, personal_info.lunar_mansion_moon
        ));
        if lucky.len() >= 3 {
            notable.push(format!(
                "本命幸运星：{}、{}、{}",
                lucky[0].star_name_cn, lucky[1].star_name_cn, lucky[2].star_name_cn
            ));
        }

        PersonalStarmapResponse {
            personal_info,
            birth_datetime_iso,
            birth_jd_ut1: jd,
            birth_local_sidereal_time_deg: lst,
            ecliptic_obliquity_deg: eps,
            precession_epoch_delta_yr: request.birth_year as f64
                - self.config.epoch_alignment.j2000_anchor_year,
            projection_mode,
            stars: starmap_stars,
            constellation_lines: None,
            lunar_mansion_boundaries: mansion_boundaries,
            solar_system_bodies: solar_system,
            ancient_comparison: ancient_compare,
            share_card_spec: share_card,
            notable_celestial_events: notable,
            lucky_stars: lucky,
        }
    }

    pub async fn run_event_loop(
        mut cmd_rx: mpsc::Receiver<HoroscopeCommand>,
        mut engine: HoroscopeEngine,
    ) {
        info!("Horoscope engine event loop started");
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                HoroscopeCommand::GenerateStarmap {
                    request,
                    stars,
                    mansions,
                } => {
                    let _resp = engine.generate(request, stars, mansions);
                    info!("Starmap generated successfully");
                }
                HoroscopeCommand::GetShareCard { starmap } => {
                    if let Some(spec) = starmap.share_card_spec.clone() {
                        info!("Share card spec ready: {}x{}", spec.width_px, spec.height_px);
                    }
                }
                HoroscopeCommand::Shutdown => {
                    info!("Horoscope engine shutting down");
                    break;
                }
            }
        }
    }
}

pub fn spawn_horoscope_service(
    config: HoroscopeConfig,
) -> (
    mpsc::Sender<HoroscopeCommand>,
    mpsc::Receiver<HoroscopeEvent>,
) {
    let buffer_size = 32;
    let (cmd_tx, cmd_rx) = mpsc::channel(buffer_size);
    let (event_tx, event_rx) = mpsc::channel(buffer_size);

    tokio::spawn(async move {
        let mut local_cmd_rx = cmd_rx;
        let local_event_tx = event_tx;
        let mut eng = HoroscopeEngine::new(config);
        info!("Horoscope service spawned");
        while let Some(cmd) = local_cmd_rx.recv().await {
            match cmd {
                HoroscopeCommand::GenerateStarmap {
                    request,
                    stars,
                    mansions,
                } => {
                    let resp = eng.generate(request, stars, mansions);
                    let _ = local_event_tx
                        .send(HoroscopeEvent::StarmapGenerated(Box::new(resp)))
                        .await;
                }
                HoroscopeCommand::GetShareCard { starmap } => {
                    if let Some(spec) = starmap.share_card_spec.clone() {
                        let _ = local_event_tx
                            .send(HoroscopeEvent::ShareCardGenerated(Box::new(spec)))
                            .await;
                    } else {
                        let _ = local_event_tx
                            .send(HoroscopeEvent::Error {
                                message: "No share card spec available".into(),
                            })
                            .await;
                    }
                }
                HoroscopeCommand::Shutdown => {
                    let _ = local_event_tx.send(HoroscopeEvent::ShutdownAck).await;
                    break;
                }
            }
        }
    });

    (cmd_tx, event_rx)
}
