//! 坐标转换模块
//!
//! 职责:
//!   1. 从 catalog_loader 接收清洗后的星表数据 (通过 channel)
//!   2. 执行 IAU 2006 岁差 + 行星摄动 + 章动 + 自行转换
//!   3. 估计转换误差 (模型误差 + 观测误差合成)
//!   4. 通过 channel 将转换结果发送给 transient_matcher
//!
//! 模型参数全部从 config::PrecessionConfig 加载，不再硬编码

use crate::astronomy::constants::*;
use crate::config::PrecessionConfig;
use crate::catalog_loader::CleanedStarRecord;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransformCommand {
    ConvertSingle {
        ruxiu_du: f64,
        quji_du: f64,
        mansion_order: i32,
        epoch_yr: f64,
        pm_ra_mas: Option<f64>,
        pm_dec_mas: Option<f64>,
    },
    ConvertBatch {
        records: Vec<CleanedStarRecord>,
        epoch_yr: f64,
    },
    ComputeTrajectory {
        ra_j2000: f64,
        dec_j2000: f64,
        pm_ra_mas: f64,
        pm_dec_mas: f64,
        year_start: f64,
        year_end: f64,
        n_points: i32,
    },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransformEvent {
    SingleConverted(Box<TransformResult>),
    BatchConverted { count: usize, results: Vec<TransformResult> },
    TrajectoryComputed { points: Vec<TrajectoryPoint> },
    Error { message: String },
    ShutdownAck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformResult {
    pub star_id: i64,
    pub ancient_ra: f64,
    pub ancient_dec: f64,
    pub ra_j2000: f64,
    pub dec_j2000: f64,
    pub ra_without_pm: f64,
    pub dec_without_pm: f64,
    pub precession_matrix: [[f64; 3]; 3],
    pub nutation_correction: [f64; 2],
    pub planetary_correction_arcsec: f64,
    pub proper_motion_arrow: [f64; 3],
    pub error_estimate: TransformError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformError {
    pub ra_error_arcsec: f64,
    pub dec_error_arcsec: f64,
    pub model_error_arcsec: f64,
    pub observation_error_arcsec: f64,
    pub proper_motion_error_arcsec: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryPoint {
    pub year: f64,
    pub ra_deg: f64,
    pub dec_deg: f64,
}

type M3 = [[f64; 3]; 3];

pub struct CoordinateTransformer {
    config: PrecessionConfig,
    cmd_rx: mpsc::Receiver<TransformCommand>,
    event_tx: mpsc::Sender<TransformEvent>,
    psi_coeffs: Vec<f64>,
    omega_coeffs: Vec<f64>,
    chi_coeffs: Vec<f64>,
    zeta_coeffs: Vec<f64>,
    theta_coeffs: Vec<f64>,
    z_coeffs: Vec<f64>,
}

impl CoordinateTransformer {
    pub fn new(
        config: PrecessionConfig,
    ) -> (Self, mpsc::Sender<TransformCommand>, mpsc::Receiver<TransformEvent>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        let (event_tx, event_rx) = mpsc::channel(32);

        let psi_coeffs = config.psi_a_coeffs_mas.iter()
            .map(|&v| v / 1000.0).collect();
        let omega_coeffs = config.omega_a_coeffs_mas.iter()
            .map(|&v| v / 1000.0).collect();
        let chi_coeffs = config.chi_a_coeffs_mas.iter()
            .map(|&v| v / 1000.0).collect();

        (
            Self {
                config,
                cmd_rx,
                event_tx,
                psi_coeffs,
                omega_coeffs,
                chi_coeffs,
                zeta_coeffs: vec![],
                theta_coeffs: vec![],
                z_coeffs: vec![],
            },
            cmd_tx,
            event_rx,
        )
    }

    pub async fn run(mut self) {
        info!("CoordinateTransformer started (IAU 2006 model from config)");
        while let Some(cmd) = self.cmd_rx.recv().await {
            let event = match cmd {
                TransformCommand::ConvertSingle {
                    ruxiu_du, quji_du, mansion_order, epoch_yr, pm_ra_mas, pm_dec_mas
                } => self.handle_convert_single(
                    ruxiu_du, quji_du, mansion_order, epoch_yr,
                    pm_ra_mas.unwrap_or(0.0), pm_dec_mas.unwrap_or(0.0),
                ),
                TransformCommand::ConvertBatch { records, epoch_yr } =>
                    self.handle_convert_batch(&records, epoch_yr),
                TransformCommand::ComputeTrajectory {
                    ra_j2000, dec_j2000, pm_ra_mas, pm_dec_mas,
                    year_start, year_end, n_points
                } => self.handle_trajectory(
                    ra_j2000, dec_j2000, pm_ra_mas, pm_dec_mas,
                    year_start, year_end, n_points,
                ),
                TransformCommand::Shutdown => {
                    info!("CoordinateTransformer shutting down");
                    TransformEvent::ShutdownAck
                }
            };
            let _ = self.event_tx.send(event).await;
        }
    }

    fn handle_convert_single(
        &self,
        ruxiu_du: f64, quji_du: f64, mansion_order: i32,
        epoch_yr: f64, pm_ra: f64, pm_dec: f64,
    ) -> TransformEvent {
        use crate::astronomy::LUNAR_MANSIONS;

        let mansion = LUNAR_MANSIONS.iter()
            .find(|m| m.order == mansion_order)
            .unwrap_or(&LUNAR_MANSIONS[0]);

        let ancient_ra = normalize_angle_360(mansion.ra_offset_deg + ruxiu_du);
        let ancient_dec = 90.0 - quji_du;

        let t_centuries = (epoch_yr - 2000.0) / 100.0;

        let (ra_wopm, dec_wopm) = self.transform_epoch_to_j2000(
            ancient_ra, ancient_dec, t_centuries);

        let dt_yr = 2000.0 - epoch_yr;
        let (ra_j2000, dec_j2000) = self.apply_proper_motion_backward(
            ra_wopm, dec_wopm, pm_ra, pm_dec, dt_yr);

        let p = self.precession_matrix(t_centuries);
        let (dpsi, deps) = self.nutation(t_centuries);
        let chi = self.planetary_chi(t_centuries) / AS2RAD;
        let (dra, ddec, pa) = self.proper_motion_arrow(
            ra_j2000, dec_j2000, pm_ra, pm_dec, 1000.0);

        let err = self.estimate_error(ruxiu_du, quji_du, pm_ra, pm_dec, epoch_yr);

        TransformEvent::SingleConverted(Box::new(TransformResult {
            star_id: 0,
            ancient_ra,
            ancient_dec,
            ra_j2000,
            dec_j2000,
            ra_without_pm: ra_wopm,
            dec_without_pm: dec_wopm,
            precession_matrix: p,
            nutation_correction: [dpsi / AS2RAD, deps / AS2RAD],
            planetary_correction_arcsec: chi,
            proper_motion_arrow: [dra, ddec, pa],
            error_estimate: err,
        }))
    }

    fn handle_convert_batch(
        &self,
        records: &[CleanedStarRecord],
        _epoch_yr: f64,
    ) -> TransformEvent {
        let results: Vec<TransformResult> = records.iter()
            .filter_map(|r| {
                let ruxiu = r.ruxiu_du?;
                let quji = r.quji_du?;
                let mansion = 1;
                let epoch = 1000.0;
                let pm_ra = r.proper_motion_ra.unwrap_or(0.0);
                let pm_dec = r.proper_motion_dec.unwrap_or(0.0);

                if let TransformEvent::SingleConverted(boxed) = self.handle_convert_single(
                    ruxiu, quji, mansion, epoch, pm_ra, pm_dec,
                ) {
                    Some(TransformResult { star_id: r.id, ..*boxed })
                } else {
                    None
                }
            })
            .collect();

        TransformEvent::BatchConverted {
            count: results.len(),
            results,
        }
    }

    fn handle_trajectory(
        &self,
        ra_j2000: f64, dec_j2000: f64,
        pm_ra: f64, pm_dec: f64,
        year_start: f64, year_end: f64, n_points: i32,
    ) -> TransformEvent {
        let n = n_points.max(3) as usize;
        let mut pts = Vec::with_capacity(n);
        for i in 0..n {
            let frac = i as f64 / (n as f64 - 1.0);
            let year = year_start + frac * (year_end - year_start);
            let (ra, dec) = self.apply_proper_motion_forward(
                ra_j2000, dec_j2000, pm_ra, pm_dec, year - 2000.0);
            pts.push(TrajectoryPoint { year, ra_deg: ra, dec_deg: dec });
        }
        TransformEvent::TrajectoryComputed { points: pts }
    }

    fn estimate_error(
        &self,
        _ruxiu: f64, _quji: f64,
        pm_ra: f64, pm_dec: f64,
        epoch_yr: f64,
    ) -> TransformError {
        let obs_ruxiu: f64 = 0.05;
        let obs_quji: f64 = 0.05;
        let observation = (obs_ruxiu.powi(2) + obs_quji.powi(2)).sqrt() * 3600.0;

        let dt = (2000.0 - epoch_yr).abs();
        let model: f64 = if dt < 100.0 {
            0.001
        } else if dt < 1000.0 {
            0.01
        } else {
            0.1
        } * 3600.0;

        let pm_err = ((pm_ra.powi(2) + pm_dec.powi(2)).sqrt() * dt / 1000.0).max(0.1);

        let total = (model.powi(2) + observation.powi(2) + pm_err.powi(2)).sqrt();
        let ra_err = total;
        let dec_err = total;

        TransformError {
            ra_error_arcsec: ra_err,
            dec_error_arcsec: dec_err,
            model_error_arcsec: model,
            observation_error_arcsec: observation,
            proper_motion_error_arcsec: pm_err,
        }
    }

    fn poly(coeffs: &[f64], t: f64) -> f64 {
        let mut s = 0.0f64;
        let mut tp = 1.0f64;
        for c in coeffs {
            s += c * tp;
            tp *= t;
        }
        s
    }

    fn iau2006_angles(&self, t: f64) -> (f64, f64, f64, f64) {
        let zeta_a = Self::poly(&self.config.zeta_a_coeffs_arcsec, t) * AS2RAD;
        let theta_a = Self::poly(&self.config.theta_a_coeffs_arcsec, t) * AS2RAD;
        let z_a = Self::poly(&self.config.z_a_coeffs_arcsec, t) * AS2RAD;
        let omega_a = Self::poly(&self.omega_coeffs, t) * AS2RAD;
        (zeta_a, theta_a, z_a, omega_a)
    }

    fn planetary_chi(&self, t: f64) -> f64 {
        Self::poly(&self.chi_coeffs, t) * AS2RAD
    }

    fn precession_matrix(&self, t: f64) -> M3 {
        let (zeta_a, theta_a, z_a, _) = self.iau2006_angles(t);
        let rz1 = self.rot_z(-zeta_a);
        let rx = self.rot_x(theta_a);
        let rz2 = self.rot_z(-z_a);
        self.mul3(&rz2, &rx, &rz1)
    }

    fn nutation(&self, t: f64) -> (f64, f64) {
        let r = &self.config.nutation_delaunay_rates_arcsec_per_cy;
        let c = &self.config.nutation_delaunay_constants_arcsec;

        let l_arcsec = c.l + r.l * t;
        let lp_arcsec = c.lp + r.lp * t;
        let f_arcsec = c.F + r.F * t;
        let d_arcsec = c.D + r.D * t;
        let om_arcsec = c.Om + r.Om * t;

        let mut dpsi_mas = 0.0;
        let mut deps_mas = 0.0;

        for term in &self.config.iau2000b_nutation_terms {
            let arg = (term.l * l_arcsec
                + term.lp * lp_arcsec
                + term.F * f_arcsec
                + term.D * d_arcsec
                + term.Om * om_arcsec) * AS2RAD;
            let sin_a = arg.sin();
            let cos_a = arg.cos();
            dpsi_mas += term.dpsi_sin * sin_a + term.dpsi_cos * cos_a;
            deps_mas += term.deps_sin * cos_a + term.deps_cos * sin_a;
        }

        (dpsi_mas * MAS2RAD, deps_mas * MAS2RAD)
    }

    fn nutation_matrix(&self, omega_a: f64, dpsi: f64, deps: f64) -> M3 {
        let r1 = self.rot_x(omega_a);
        let r3 = self.rot_z(-dpsi);
        let r2 = self.rot_x(-(omega_a + deps));
        self.mul3(&r2, &r3, &r1)
    }

    fn planetary_matrix(&self, t: f64) -> M3 {
        let chi = self.planetary_chi(t);
        self.rot_x(chi)
    }

    fn transform_epoch_to_j2000(&self, ra_t: f64, dec_t: f64, t: f64) -> (f64, f64) {
        let ra_r = ra_t * DEG2RAD;
        let dec_r = dec_t * DEG2RAD;
        let v = [
            dec_r.cos() * ra_r.cos(),
            dec_r.cos() * ra_r.sin(),
            dec_r.sin(),
        ];

        let (_, _, _, omega_a) = self.iau2006_angles(t);
        let (dpsi, deps) = self.nutation(t);
        let n = self.nutation_matrix(omega_a, dpsi, deps);
        let pp = self.planetary_matrix(t);
        let p = self.precession_matrix(t);

        let v1 = self.mat_vec(&n, &v);
        let v2 = self.mat_vec(&pp, &v1);
        let v3 = self.mat_vec(&p, &v2);

        let dec = v3[2].asin() * RAD2DEG;
        let ra = v3[1].atan2(v3[0]) * RAD2DEG;
        (normalize_angle_360(ra), dec)
    }

    fn apply_proper_motion_forward(
        &self,
        ra_j2000: f64, dec_j2000: f64,
        pm_ra: f64, pm_dec: f64, delta_t_yr: f64,
    ) -> (f64, f64) {
        let dra = pm_ra * delta_t_yr / (3600.0 * 1000.0);
        let ddec = pm_dec * delta_t_yr / (3600.0 * 1000.0);
        let cos_dec = (dec_j2000 * DEG2RAD).cos();
        let ra_new = if cos_dec.abs() > 1e-6 { ra_j2000 + dra / cos_dec } else { ra_j2000 };
        (normalize_angle_360(ra_new), dec_j2000 + ddec)
    }

    fn apply_proper_motion_backward(
        &self,
        ra_t: f64, dec_t: f64,
        pm_ra: f64, pm_dec: f64, delta_t_yr: f64,
    ) -> (f64, f64) {
        self.apply_proper_motion_forward(ra_t, dec_t, -pm_ra, -pm_dec, delta_t_yr)
    }

    fn proper_motion_arrow(
        &self,
        _ra_j2000: f64, dec_j2000: f64,
        pm_ra: f64, pm_dec: f64, scale_yr: f64,
    ) -> (f64, f64, f64) {
        let dra = pm_ra * scale_yr / (3600.0 * 1000.0);
        let ddec = pm_dec * scale_yr / (3600.0 * 1000.0);
        let cos_dec = (dec_j2000 * DEG2RAD).cos();
        let dra_deg = if cos_dec.abs() > 1e-6 { dra / cos_dec } else { dra };
        let pa = ddec.atan2(dra_deg * cos_dec) * RAD2DEG;
        (dra_deg, ddec, pa)
    }

    fn eye(&self) -> M3 { [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] }

    fn mul(&self, a: &M3, b: &M3) -> M3 {
        let mut r = [[0.0; 3]; 3];
        for i in 0..3 { for j in 0..3 {
            r[i][j] = a[i][0]*b[0][j] + a[i][1]*b[1][j] + a[i][2]*b[2][j];
        }}
        r
    }

    fn mul3(&self, a: &M3, b: &M3, c: &M3) -> M3 {
        self.mul(&self.mul(a, b), c)
    }

    fn mat_vec(&self, m: &M3, v: &[f64; 3]) -> [f64; 3] {
        [
            m[0][0]*v[0] + m[0][1]*v[1] + m[0][2]*v[2],
            m[1][0]*v[0] + m[1][1]*v[1] + m[1][2]*v[2],
            m[2][0]*v[0] + m[2][1]*v[1] + m[2][2]*v[2],
        ]
    }

    fn rot_x(&self, alpha: f64) -> M3 {
        let c = alpha.cos();
        let s = alpha.sin();
        [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]
    }

    fn rot_z(&self, alpha: f64) -> M3 {
        let c = alpha.cos();
        let s = alpha.sin();
        [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
    }
}

pub fn spawn_transformer(
    config: PrecessionConfig,
) -> (mpsc::Sender<TransformCommand>, mpsc::Receiver<TransformEvent>) {
    let (tf, cmd_tx, event_rx) = CoordinateTransformer::new(config);
    tokio::spawn(async move { tf.run().await });
    (cmd_tx, event_rx)
}
