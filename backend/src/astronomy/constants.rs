//! 天文常量与数学工具
//!
//! 修复 1 (IAU 2006):
//!   DEG2RAD / RAD2DEG 仍为标准值.
//!   新增 IAU 2006 岁差的 J2000 儒略日常量 JD2000
//!   新增 J2000.0 银道极点 (北银极 l=122.932, b=27.128) 用于银河系先验

use std::f64::consts::PI;

pub const DEG2RAD: f64 = PI / 180.0;
pub const RAD2DEG: f64 = 180.0 / PI;
pub const AS2RAD: f64 = PI / (180.0 * 3600.0);
pub const MAS2RAD: f64 = PI / (180.0 * 3600.0 * 1000.0);

/// J2000.0 儒略日
pub const JD2000: f64 = 2451545.0;
/// 儒略世纪长度 (天)
pub const JULIAN_CENTURY: f64 = 36525.0;

/// 归一化角度到 [0, 360) 度
pub fn normalize_angle_360(deg: f64) -> f64 {
    let mut a = deg % 360.0;
    if a < 0.0 { a += 360.0; }
    a
}

/// 归一化角度到 [-180, 180) 度
pub fn normalize_angle_180(deg: f64) -> f64 {
    let mut a = (deg + 180.0) % 360.0;
    if a < 0.0 { a += 360.0; }
    a - 180.0
}

/// 球面两点角距离 (Haversine 公式), 输入输出均为度
pub fn angular_distance_deg(ra1: f64, dec1: f64, ra2: f64, dec2: f64) -> f64 {
    let d_ra = (ra1 - ra2) * DEG2RAD;
    let d_dec = (dec1 - dec2) * DEG2RAD;
    let a = (d_dec / 2.0).sin().powi(2)
        + dec1.to_radians().cos() * dec2.to_radians().cos() * (d_ra / 2.0).sin().powi(2);
    2.0 * a.sqrt().asin() * RAD2DEG
}

/// 位置角 (从北点沿大圆向东测量, 度)
pub fn position_angle_deg(ra1: f64, dec1: f64, ra2: f64, dec2: f64) -> f64 {
    let dra = (ra2 - ra1) * DEG2RAD;
    let d1 = dec1 * DEG2RAD;
    let d2 = dec2 * DEG2RAD;
    let y = dra.sin() * d2.cos();
    let x = d1.sin() * d2.cos() * dra.cos() - d1.cos() * d2.sin();
    y.atan2(x) * RAD2DEG
}

/// ============================================================
/// ICRS → 银道坐标 (J2000.0)
/// 用于: 修复 2 的银河系分布先验
/// 北银极 (RA=192.8595°, Dec=+27.1284°)
/// 升交点在银经 l=122.9320°
/// ============================================================

const NGC_NGP_RA_DEG: f64 = 192.8595;   // 北银极 RA (J2000)
const NGC_NGP_DEC_DEG: f64 = 27.1284;   // 北银极 Dec (J2000)
const NGC_LON_CP_DEG: f64 = 122.9320;   // 北天极处的银经

/// 赤道 (J2000) → 银道
pub fn equatorial_to_galactic(ra_deg: f64, dec_deg: f64) -> (f64, f64) {
    let ra = ra_deg * DEG2RAD;
    let dec = dec_deg * DEG2RAD;
    let ngp_ra = NGC_NGP_RA_DEG * DEG2RAD;
    let ngp_dec = NGC_NGP_DEC_DEG * DEG2RAD;
    let lon_cp = NGC_LON_CP_DEG * DEG2RAD;

    let sin_b = dec.sin() * ngp_dec.sin()
        + dec.cos() * ngp_dec.cos() * (ra - ngp_ra).cos();
    let b = sin_b.asin();

    let y = dec.sin() * ngp_dec.cos() - dec.cos() * ngp_dec.sin() * (ra - ngp_ra).cos();
    let x = -dec.cos() * (ra - ngp_ra).sin();
    let l = y.atan2(x) + lon_cp;

    (normalize_angle_360(l * RAD2DEG), b * RAD2DEG)
}
