pub mod constants;
pub mod precession;

pub use constants::*;

use serde::{Deserialize, Serialize};

/// 入宿度/去极度 → J2000 坐标转换输入
#[derive(Debug, Clone, Deserialize)]
pub struct RuxiuToJ2000Request {
    pub ruxiu_du: f64,
    pub quji_du: f64,
    pub mansion_order: i32,
    pub epoch_yr: f64,
    pub pm_ra_mas: Option<f64>,
    pub pm_dec_mas: Option<f64>,
}

/// 坐标转换响应
#[derive(Debug, Clone, Serialize)]
pub struct CoordinateTransform {
    pub ancient_ra: f64,
    pub ancient_dec: f64,
    pub ruxiu_raw_cn: String,
    pub quji_raw_cn: String,
    pub without_proper_motion: [f64; 2],
    pub with_proper_motion: [f64; 2],
    pub precession_matrix: [[f64; 3]; 3],
    pub nutation_correction: [f64; 2],
    pub planetary_correction_arcsec: f64,
    pub proper_motion_arrow_deg: [f64; 3],
}

/// 自行轨迹采样请求
#[derive(Debug, Clone, Deserialize)]
pub struct TrajectoryRequest {
    pub ra_j2000: f64,
    pub dec_j2000: f64,
    pub pm_ra_mas: f64,
    pub pm_dec_mas: f64,
    pub year_start: f64,
    pub year_end: f64,
    pub n_points: i32,
}

/// 轨迹点
#[derive(Debug, Clone, Serialize)]
pub struct TrajectoryPoint {
    pub year: f64,
    pub ra_deg: f64,
    pub dec_deg: f64,
}

/// 二十八宿宿度 (用于入宿度换算)
#[derive(Debug, Clone, Copy)]
pub struct MansionInfo {
    pub order: i32,
    pub name_cn: &'static str,
    pub ra_offset_deg: f64,
}

/// 中国古代二十八宿宿度 (西汉至清代平均)
/// 对应每一宿的起始赤经 (度, 按 28 宿平均分配 360°)
pub const LUNAR_MANSIONS: [MansionInfo; 28] = [
    MansionInfo { order: 1,  name_cn: "角", ra_offset_deg: 351.4 },
    MansionInfo { order: 2,  name_cn: "亢", ra_offset_deg: 363.3 },
    MansionInfo { order: 3,  name_cn: "氐", ra_offset_deg: 375.4 },
    MansionInfo { order: 4,  name_cn: "房", ra_offset_deg: 387.0 },
    MansionInfo { order: 5,  name_cn: "心", ra_offset_deg: 396.2 },
    MansionInfo { order: 6,  name_cn: "尾", ra_offset_deg: 404.0 },
    MansionInfo { order: 7,  name_cn: "箕", ra_offset_deg: 414.8 },
    MansionInfo { order: 8,  name_cn: "斗", ra_offset_deg: 425.6 },
    MansionInfo { order: 9,  name_cn: "牛", ra_offset_deg: 444.8 },
    MansionInfo { order: 10, name_cn: "女", ra_offset_deg: 459.0 },
    MansionInfo { order: 11, name_cn: "虚", ra_offset_deg: 470.8 },
    MansionInfo { order: 12, name_cn: "危", ra_offset_deg: 480.9 },
    MansionInfo { order: 13, name_cn: "室", ra_offset_deg: 494.5 },
    MansionInfo { order: 14, name_cn: "壁", ra_offset_deg: 510.6 },
    MansionInfo { order: 15, name_cn: "奎", ra_offset_deg: 525.8 },
    MansionInfo { order: 16, name_cn: "娄", ra_offset_deg: 542.3 },
    MansionInfo { order: 17, name_cn: "胃", ra_offset_deg: 555.2 },
    MansionInfo { order: 18, name_cn: "昴", ra_offset_deg: 566.7 },
    MansionInfo { order: 19, name_cn: "毕", ra_offset_deg: 578.7 },
    MansionInfo { order: 20, name_cn: "觜", ra_offset_deg: 591.3 },
    MansionInfo { order: 21, name_cn: "参", ra_offset_deg: 599.7 },
    MansionInfo { order: 22, name_cn: "井", ra_offset_deg: 616.0 },
    MansionInfo { order: 23, name_cn: "鬼", ra_offset_deg: 638.4 },
    MansionInfo { order: 24, name_cn: "柳", ra_offset_deg: 649.4 },
    MansionInfo { order: 25, name_cn: "星", ra_offset_deg: 658.7 },
    MansionInfo { order: 26, name_cn: "张", ra_offset_deg: 667.3 },
    MansionInfo { order: 27, name_cn: "翼", ra_offset_deg: 677.7 },
    MansionInfo { order: 28, name_cn: "轸", ra_offset_deg: 691.6 },
];

/// 入宿度 + 去极度 → 古代赤经/赤纬 (古代观测历元的赤道坐标)
///   古代赤经 = 该宿起始赤经 + 入宿度 (mod 360°)
///   古代赤纬 = 90° - 去极度
pub fn ruxiu_to_ancient_equatorial(ruxiu_du: f64, quji_du: f64, mansion_order: i32) -> (f64, f64, String, String) {
    let mansion = LUNAR_MANSIONS.iter()
        .find(|m| m.order == mansion_order)
        .unwrap_or(&LUNAR_MANSIONS[0]);

    let ancient_ra = normalize_angle_360(mansion.ra_offset_deg + ruxiu_du);
    let ancient_dec = 90.0 - quji_du;

    let raw_ruxiu = format!("{}宿 {:.2}度", mansion.name_cn, ruxiu_du);
    let raw_quji = format!("去极度 {:.2}度", quji_du);

    (ancient_ra, ancient_dec, raw_ruxiu, raw_quji)
}

/// 完整坐标转换入口: 入宿度/去极度 → J2000
pub fn convert_ruxiu_to_j2000(req: &RuxiuToJ2000Request) -> CoordinateTransform {
    let (ancient_ra, ancient_dec, ruxiu_raw, quji_raw) =
        ruxiu_to_ancient_equatorial(req.ruxiu_du, req.quji_du, req.mansion_order);

    let pm_ra = req.pm_ra_mas.unwrap_or(0.0);
    let pm_dec = req.pm_dec_mas.unwrap_or(0.0);

    let (ra_j2000, dec_j2000, ra_wopm, dec_wopm, _, _) = precession::ancient_to_j2000_full(
        ancient_ra, ancient_dec, req.epoch_yr, pm_ra, pm_dec,
    );

    // 岁差矩阵
    let t_centuries = (req.epoch_yr - 2000.0) / 100.0;
    let p = precession::precession_matrix_j2000_from_t(t_centuries);

    // 章动修正 (Δψ, Δε 弧秒)
    let (dpsi, deps) = precession::iau2000b_nutation(t_centuries);
    let dpsi_as = dpsi / AS2RAD;
    let deps_as = deps / AS2RAD;

    // 行星摄动修正量 (弧秒)
    let chi = precession::planetary_precession_chi(t_centuries) / AS2RAD;

    // 自行箭头 (1000 年尺度)
    let (dra, ddec, pa) = precession::proper_motion_arrow(
        ra_j2000, dec_j2000, pm_ra, pm_dec, 1000.0);

    CoordinateTransform {
        ancient_ra,
        ancient_dec,
        ruxiu_raw_cn: ruxiu_raw,
        quji_raw_cn: quji_raw,
        without_proper_motion: [ra_wopm, dec_wopm],
        with_proper_motion: [ra_j2000, dec_j2000],
        precession_matrix: p,
        nutation_correction: [dpsi_as, deps_as],
        planetary_correction_arcsec: chi,
        proper_motion_arrow_deg: [dra, ddec, pa],
    }
}

/// 自行轨迹采样
pub fn compute_trajectory(req: &TrajectoryRequest) -> Vec<TrajectoryPoint> {
    let n = req.n_points.max(3) as usize;
    let mut pts = Vec::with_capacity(n);
    for i in 0..n {
        let frac = i as f64 / (n as f64 - 1.0);
        let year = req.year_start + frac * (req.year_end - req.year_start);
        let (ra, dec) = precession::apply_proper_motion_forward(
            req.ra_j2000, req.dec_j2000,
            req.pm_ra_mas, req.pm_dec_mas,
            year - 2000.0,
        );
        pts.push(TrajectoryPoint { year, ra_deg: ra, dec_deg: dec });
    }
    pts
}

/// 跨朝代对比: 同一恒星在两朝代记录的坐标差异
pub fn compare_coords_across_epochs(
    ra_j2000: f64, dec_j2000: f64,
    pm_ra_mas: f64, pm_dec_mas: f64,
    epochs: &[f64],
) -> Vec<(f64, f64, f64)> {
    epochs.iter().map(|&ep| {
        // 先从 J2000 + 自行 推到历元 ep
        let (ra_ep, dec_ep) = precession::apply_proper_motion_forward(
            ra_j2000, dec_j2000, pm_ra_mas, pm_dec_mas, ep - 2000.0);
        // 再用岁差反向变换
        let (ra_j2000_back, dec_j2000_back, _, _, _, _) = precession::ancient_to_j2000_full(
            ra_ep, dec_ep, ep, pm_ra_mas, pm_dec_mas);
        (ep, ra_j2000_back, dec_j2000_back)
    }).collect()
}
