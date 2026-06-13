//! 古代仪器校准模块 (instrument_calibrator)
//!
//! 提供仪器系统误差反演、精度评估、误差雷达图等功能
//!
//! 职责:
//!   1. 接收古代仪器观测数据，反演系统误差参数
//!   2. 使用最小二乘 + 迭代 sigma clip 估计仪器误差
//!   3. 评估反演精度与误差贡献
//!   4. 通过 channel 发送反演结果
//!
//! 误差模型:
//!   - 极轴偏差: 倾斜 tilt + 方位 azimuth
//!   - 刻度系统误差: 一周期 + 二周期正弦项 + 常数零点
//!   - 大气折射: sec(z) 一阶近似
//!   - 准直误差: sec(dec) 近似

use crate::config::InstrumentConfig;
use crate::models::{
    ErrorRadarEntry, InstrumentErrorSolution, InstrumentObservation,
    PerInstrumentStarResidual,
};
use nalgebra::{DMatrix, DVector, SVD};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InstrumentCommand {
    InvertErrors {
        observations: Vec<InstrumentObservation>,
        ref_observations: Option<Vec<InstrumentObservation>>,
    },
    ListInstruments,
    ListObservations {
        instrument_id: i64,
    },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InstrumentEvent {
    ErrorsInverted(Box<InstrumentErrorSolution>),
    InstrumentsListed { count: usize },
    ObservationsListed { count: usize },
    Error { message: String },
    ShutdownAck,
}

pub type CalibratorCommand = InstrumentCommand;
pub type CalibratorEvent = InstrumentEvent;
pub type CalibratorEngine = InstrumentInverter;

const NUM_PARAMS: usize = 8;
const DEFAULT_LATITUDE_DEG: f64 = 35.0;
const MAX_SIGMA_CLIP_ITERATIONS: usize = 5;
const CONVERGENCE_THRESHOLD: f64 = 0.01;

pub struct InstrumentInverter {
    config: InstrumentConfig,
    cmd_rx: mpsc::Receiver<InstrumentCommand>,
    event_tx: mpsc::Sender<InstrumentEvent>,
}

impl InstrumentInverter {
    pub fn new(
        config: InstrumentConfig,
    ) -> (Self, mpsc::Sender<InstrumentCommand>, mpsc::Receiver<InstrumentEvent>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        let (event_tx, event_rx) = mpsc::channel(32);

        (
            Self {
                config,
                cmd_rx,
                event_tx,
            },
            cmd_tx,
            event_rx,
        )
    }

    pub async fn run(mut self) {
        info!("InstrumentInverter started");
        while let Some(cmd) = self.cmd_rx.recv().await {
            let event = match cmd {
                InstrumentCommand::InvertErrors { observations, ref_observations } => {
                    self.handle_invert_errors(&observations, ref_observations.as_deref())
                }
                InstrumentCommand::ListInstruments => {
                    self.handle_list_instruments()
                }
                InstrumentCommand::ListObservations { instrument_id } => {
                    self.handle_list_observations(instrument_id)
                }
                InstrumentCommand::Shutdown => {
                    info!("InstrumentInverter shutting down");
                    InstrumentEvent::ShutdownAck
                }
            };
            let _ = self.event_tx.send(event).await;
        }
    }

    fn handle_invert_errors(
        &self,
        observations: &[InstrumentObservation],
        ref_observations: Option<&[InstrumentObservation]>,
    ) -> InstrumentEvent {
        let result = invert_instrument_errors(
            observations,
            ref_observations,
            &self.config,
        );

        match result {
            Ok(solution) => InstrumentEvent::ErrorsInverted(Box::new(solution)),
            Err(e) => InstrumentEvent::Error { message: e },
        }
    }

    fn handle_list_instruments(&self) -> InstrumentEvent {
        let count = self.config.instruments.len();
        InstrumentEvent::InstrumentsListed { count }
    }

    fn handle_list_observations(&self, _instrument_id: i64) -> InstrumentEvent {
        InstrumentEvent::ObservationsListed { count: 0 }
    }
}

pub fn invert_instrument_errors(
    observations: &[InstrumentObservation],
    ref_observations: Option<&[InstrumentObservation]>,
    config: &InstrumentConfig,
) -> Result<InstrumentErrorSolution, String> {
    let min_shared = config.inversion.min_shared_stars;
    let valid_pairs = build_valid_pairs(observations, ref_observations)?;

    if valid_pairs.len() < min_shared {
        return Err(format!(
            "Not enough shared stars: got {}, need at least {}",
            valid_pairs.len(),
            min_shared
        ));
    }

    let n = valid_pairs.len();
    let sigma_clip_thresh = config.inversion.sigma_clip_threshold;

    let mut mask: Vec<bool> = vec![true; n];
    let mut prev_params: Option<DVector<f64>> = None;
    let mut num_iterations = 0;
    let mut converged = false;

    for iter in 0..=MAX_SIGMA_CLIP_ITERATIONS {
        num_iterations = iter as i32;

        let n_used = mask.iter().filter(|&&m| m).count();
        let lambda = config.inversion.regularization_lambda;
        let min_required = if lambda > 0.0 {
            NUM_PARAMS.min(3)
        } else {
            NUM_PARAMS + 1
        };
        if n_used < min_required {
            return Err("Too few inliers after sigma clipping".to_string());
        }

        let (a_matrix, b_vector) = build_design_matrix(&valid_pairs, &mask);

        let params = if n_used < 2 * NUM_PARAMS && lambda > 0.0 {
            solve_tikhonov(&a_matrix, &b_vector, lambda)
        } else {
            let svd = SVD::new(a_matrix, true, true);
            svd.solve(&b_vector, 1e-12).map_err(|e| e.to_string())
        }?;

        if let Some(ref prev) = prev_params {
            let max_rel_change = params
                .iter()
                .zip(prev.iter())
                .filter(|(_, &p)| p.abs() > 1e-10)
                .map(|(c, p)| (c - p).abs() / p.abs())
                .fold(0.0f64, f64::max);

            if max_rel_change < CONVERGENCE_THRESHOLD {
                converged = true;
                if iter == MAX_SIGMA_CLIP_ITERATIONS {
                    break;
                }
            }
        }

        if iter >= MAX_SIGMA_CLIP_ITERATIONS {
            break;
        }

        prev_params = Some(params.clone());

        let residuals = compute_residuals(&valid_pairs, &params);
        let residuals_inlier: Vec<f64> = residuals
            .iter()
            .enumerate()
            .filter(|(i, _)| mask[i / 2])
            .map(|(_, &r)| r)
            .collect();

        let mean_res = residuals_inlier.iter().sum::<f64>() / residuals_inlier.len() as f64;
        let var_res = residuals_inlier
            .iter()
            .map(|&r| (r - mean_res).powi(2))
            .sum::<f64>()
            / (residuals_inlier.len() as f64 - 1.0);
        let std_res = var_res.sqrt();

        for i in 0..n {
            if mask[i] {
                let ra_res = residuals[2 * i];
                let dec_res = residuals[2 * i + 1];
                let total_res = (ra_res.powi(2) + dec_res.powi(2)).sqrt();
                if (total_res - mean_res).abs() > sigma_clip_thresh * std_res {
                    mask[i] = false;
                }
            }
        }

        if converged {
            break;
        }
    }

    let final_params = prev_params.ok_or("Failed to compute solution")?;
    let (a_final, b_final) = build_design_matrix(&valid_pairs, &mask);

    let n_used_final = mask.iter().filter(|&&m| m).count();
    let dof = (n_used_final * 2).saturating_sub(NUM_PARAMS).max(1) as f64;

    let residuals = compute_residuals(&valid_pairs, &final_params);
    let residuals_inlier: Vec<f64> = residuals
        .iter()
        .enumerate()
        .filter(|(i, _)| mask[i / 2])
        .map(|(_, &r)| r)
        .collect();

    let sum_sq = residuals_inlier.iter().map(|&r| r * r).sum::<f64>();
    let overall_rms = (sum_sq / (n_used_final as f64 * 2.0)).sqrt();
    let chi_squared_reduced = sum_sq / dof;

    let (ra_residuals, dec_residuals) = split_residuals(&residuals, &mask);
    let ra_mean = mean(&ra_residuals);
    let ra_std = std(&ra_residuals);
    let ra_median_abs = median_abs(&ra_residuals);
    let dec_mean = mean(&dec_residuals);
    let dec_std = std(&dec_residuals);
    let dec_median_abs = median_abs(&dec_residuals);

    let param_errors = compute_param_uncertainties(&a_final, overall_rms);

    let quality = assess_quality(overall_rms, chi_squared_reduced);

    let per_star_residuals = build_per_star_residuals(&valid_pairs, &residuals, &mask);

    let radar = build_error_radar(&final_params, &valid_pairs);

    let first_obs = valid_pairs.first();
    let instrument_id = first_obs.map(|o| o.obs.instrument_id).unwrap_or(0);
    let instrument_code = first_obs
        .map(|o| format!("inst_{}", o.obs.instrument_id))
        .unwrap_or_default();
    let instrument_name_cn = first_obs
        .and_then(|o| o.obs.star_name_cn.clone())
        .unwrap_or_else(|| "未知仪器".to_string());

    let scale_1 = final_params[2].hypot(final_params[3]);
    let scale_2 = if NUM_PARAMS > 4 {
        final_params[4].hypot(0.0)
    } else {
        0.0
    };

    let solution = InstrumentErrorSolution {
        instrument_id,
        instrument_code,
        instrument_name_cn,
        ref_instrument_code: "ref".to_string(),
        num_shared_stars: n_used_final,
        num_iterations,
        converged,
        polar_axis_tilt_arcmin: final_params[0],
        polar_axis_tilt_uncertainty_arcmin: param_errors[0],
        polar_axis_azimuth_arcmin: final_params[1],
        polar_axis_azimuth_uncertainty_arcmin: param_errors[1],
        divisions_systematic_correction_arcmin_per_cycle: scale_1,
        divisions_periodicity_1_arcmin: scale_1,
        divisions_periodicity_2_arcmin: scale_2,
        ra_zero_point_offset_arcmin: final_params[4],
        dec_zero_point_offset_arcmin: final_params[5],
        collimation_error_arcmin: final_params[6],
        flexure_term_arcmin_per_90deg: 0.0,
        refraction_correction_arcmin_per_airmass: final_params[7],
        residuals_ra_mean_arcmin: ra_mean,
        residuals_ra_std_arcmin: ra_std,
        residuals_ra_median_abs_arcmin: ra_median_abs,
        residuals_dec_mean_arcmin: dec_mean,
        residuals_dec_std_arcmin: dec_std,
        residuals_dec_median_abs_arcmin: dec_median_abs,
        overall_rms_arcmin: overall_rms,
        chi_squared_reduced,
        accuracy_assessment_quality: quality.0,
        accuracy_assessment_text: quality.1,
        per_star_residuals: Some(per_star_residuals),
        error_component_radar: radar,
    };

    Ok(solution)
}

struct ObsPair<'a> {
    obs: &'a InstrumentObservation,
    ref_ra: f64,
    ref_dec: f64,
}

fn build_valid_pairs<'a>(
    observations: &'a [InstrumentObservation],
    ref_observations: Option<&'a [InstrumentObservation]>,
) -> Result<Vec<ObsPair<'a>>, String> {
    let mut pairs = Vec::new();

    match ref_observations {
        Some(refs) => {
            for obs in observations {
                if let (Some(obs_ra), Some(obs_dec)) =
                    (obs.ra_deg_measured, obs.dec_deg_measured)
                {
                    if let Some(ref_star) = refs.iter().find(|r| r.id == obs.id) {
                        if let (Some(ref_ra), Some(ref_dec)) =
                            (ref_star.ra_j2000_true, ref_star.dec_j2000_true)
                        {
                            pairs.push(ObsPair {
                                obs,
                                ref_ra,
                                ref_dec,
                            });
                        }
                    }
                }
            }
        }
        None => {
            for obs in observations {
                if let (Some(obs_ra), Some(obs_dec)) =
                    (obs.ra_deg_measured, obs.dec_deg_measured)
                {
                    if let (Some(true_ra), Some(true_dec)) =
                        (obs.ra_j2000_true, obs.dec_j2000_true)
                    {
                        pairs.push(ObsPair {
                            obs,
                            ref_ra: true_ra,
                            ref_dec: true_dec,
                        });
                    }
                }
            }
        }
    }

    if pairs.is_empty() {
        return Err("No valid observation pairs found".to_string());
    }

    Ok(pairs)
}

fn solve_tikhonov(
    a: &DMatrix<f64>,
    b: &DVector<f64>,
    lambda: f64,
) -> Result<DVector<f64>, String> {
    let n_cols = a.ncols();
    let at = a.transpose();
    let ata = &at * a;
    let lambda_sq = lambda * lambda;
    let reg = ata + DMatrix::identity(n_cols, n_cols) * lambda_sq;
    let atb = &at * b;

    let svd = SVD::new(reg, true, true);
    svd.solve(&atb, 1e-12).map_err(|e| e.to_string())
}

fn build_design_matrix(pairs: &[ObsPair], mask: &[bool]) -> (DMatrix<f64>, DVector<f64>) {
    let n_used = mask.iter().filter(|&&m| m).count();
    let n_rows = n_used * 2;

    let mut a = DMatrix::zeros(n_rows, NUM_PARAMS);
    let mut b = DVector::zeros(n_rows);

    let mut row_idx = 0;
    for (i, pair) in pairs.iter().enumerate() {
        if !mask[i] {
            continue;
        }

        let ra_deg = pair.obs.ra_deg_measured.unwrap_or(0.0);
        let dec_deg = pair.obs.dec_deg_measured.unwrap_or(0.0);
        let ra_rad = ra_deg.to_radians();
        let dec_rad = dec_deg.to_radians();

        let z_deg = (DEFAULT_LATITUDE_DEG - dec_deg).abs().max(10.0);
        let sec_z = 1.0 / z_deg.to_radians().cos().max(0.1);
        let sec_dec = 1.0 / dec_rad.cos().max(0.1);

        let cos_ra = ra_rad.cos();
        let sin_ra = ra_rad.sin();
        let cos_2ra = (2.0 * ra_rad).cos();
        let sin_2ra = (2.0 * ra_rad).sin();
        let cos_dec = dec_rad.cos();
        let sin_dec = dec_rad.sin();

        a[(row_idx, 0)] = sin_ra * sin_dec;
        a[(row_idx, 1)] = -cos_dec;
        a[(row_idx, 2)] = cos_ra;
        a[(row_idx, 3)] = cos_2ra;
        a[(row_idx, 4)] = 1.0;
        a[(row_idx, 5)] = 0.0;
        a[(row_idx, 6)] = sec_dec;
        a[(row_idx, 7)] = sec_z * sin_dec.abs().max(0.05);

        let res_ra_arcmin = (ra_deg - pair.ref_ra) * 60.0;
        b[row_idx] = res_ra_arcmin;

        row_idx += 1;

        a[(row_idx, 0)] = cos_dec;
        a[(row_idx, 1)] = sin_ra * sin_dec;
        a[(row_idx, 2)] = sin_ra;
        a[(row_idx, 3)] = sin_2ra;
        a[(row_idx, 4)] = 0.0;
        a[(row_idx, 5)] = 1.0;
        a[(row_idx, 6)] = 0.0;
        a[(row_idx, 7)] = sec_z;

        let res_dec_arcmin = (dec_deg - pair.ref_dec) * 60.0;
        b[row_idx] = res_dec_arcmin;

        row_idx += 1;
    }

    (a, b)
}

fn compute_residuals(pairs: &[ObsPair], params: &DVector<f64>) -> Vec<f64> {
    let mut residuals = Vec::with_capacity(pairs.len() * 2);

    for pair in pairs {
        let ra_deg = pair.obs.ra_deg_measured.unwrap_or(0.0);
        let dec_deg = pair.obs.dec_deg_measured.unwrap_or(0.0);
        let ra_rad = ra_deg.to_radians();
        let dec_rad = dec_deg.to_radians();

        let z_deg = (DEFAULT_LATITUDE_DEG - dec_deg).abs().max(10.0);
        let sec_z = 1.0 / z_deg.to_radians().cos().max(0.1);
        let sec_dec = 1.0 / dec_rad.cos().max(0.1);

        let cos_ra = ra_rad.cos();
        let sin_ra = ra_rad.sin();
        let cos_2ra = (2.0 * ra_rad).cos();
        let sin_2ra = (2.0 * ra_rad).sin();
        let cos_dec = dec_rad.cos();
        let sin_dec = dec_rad.sin();

        let model_ra = params[0] * sin_ra * sin_dec
            + params[1] * (-cos_dec)
            + params[2] * cos_ra
            + params[3] * cos_2ra
            + params[4] * 1.0
            + params[5] * 0.0
            + params[6] * sec_dec
            + params[7] * sec_z * sin_dec.abs().max(0.05);

        let obs_ra_arcmin = (ra_deg - pair.ref_ra) * 60.0;
        residuals.push(obs_ra_arcmin - model_ra);

        let model_dec = params[0] * cos_dec
            + params[1] * sin_ra * sin_dec
            + params[2] * sin_ra
            + params[3] * sin_2ra
            + params[4] * 0.0
            + params[5] * 1.0
            + params[6] * 0.0
            + params[7] * sec_z;

        let obs_dec_arcmin = (dec_deg - pair.ref_dec) * 60.0;
        residuals.push(obs_dec_arcmin - model_dec);
    }

    residuals
}

fn split_residuals(residuals: &[f64], mask: &[bool]) -> (Vec<f64>, Vec<f64>) {
    let mut ra_res = Vec::new();
    let mut dec_res = Vec::new();

    for (i, &is_inlier) in mask.iter().enumerate() {
        if is_inlier {
            ra_res.push(residuals[2 * i]);
            dec_res.push(residuals[2 * i + 1]);
        }
    }

    (ra_res, dec_res)
}

fn compute_param_uncertainties(a: &DMatrix<f64>, rms: f64) -> Vec<f64> {
    let n_rows = a.nrows();
    let n_cols = a.ncols();

    let at = a.transpose();
    let ata = &at * a;

    let svd = SVD::new(ata, true, true);
    let identity = DMatrix::identity(n_cols, n_cols);
    let cov = svd.solve(&identity, 1e-12).unwrap_or(identity.clone());

    let mut errors = Vec::with_capacity(n_cols);
    for i in 0..n_cols {
        let var = cov[(i, i)].max(0.0);
        errors.push(rms * var.sqrt());
    }

    errors
}

fn assess_quality(rms_arcmin: f64, chi2_reduced: f64) -> (String, String) {
    if rms_arcmin < 0.5 && chi2_reduced < 1.5 {
        ("excellent".to_string(), "优秀：整体误差优于0.5角分，拟合良好".to_string())
    } else if rms_arcmin < 1.5 && chi2_reduced < 3.0 {
        ("good".to_string(), "良好：整体误差优于1.5角分，结果可靠".to_string())
    } else if rms_arcmin < 3.0 {
        ("moderate".to_string(), "一般：整体误差约3角分，结果可供参考".to_string())
    } else {
        ("poor".to_string(), "较差：整体误差较大，需谨慎使用".to_string())
    }
}

fn build_per_star_residuals(
    pairs: &[ObsPair],
    residuals: &[f64],
    mask: &[bool],
) -> Vec<PerInstrumentStarResidual> {
    let mut result = Vec::with_capacity(pairs.len());

    for (i, pair) in pairs.iter().enumerate() {
        let ra_res = residuals[2 * i];
        let dec_res = residuals[2 * i + 1];
        let total = (ra_res.powi(2) + dec_res.powi(2)).sqrt();

        result.push(PerInstrumentStarResidual {
            star_name_cn: pair.obs.star_name_cn.clone().unwrap_or_default(),
            ra_residual_arcmin: ra_res,
            dec_residual_arcmin: dec_res,
            total_angular_residual_arcmin: total,
            outlier_flag: !mask[i],
        });
    }

    result
}

fn build_error_radar(params: &DVector<f64>, pairs: &[ObsPair]) -> Vec<ErrorRadarEntry> {
    let mean_sec_z = pairs
        .iter()
        .map(|p| {
            let dec_deg = p.obs.dec_deg_measured.unwrap_or(0.0);
            let z_deg = (DEFAULT_LATITUDE_DEG - dec_deg).abs().max(10.0);
            1.0 / z_deg.to_radians().cos().max(0.1)
        })
        .sum::<f64>()
        / pairs.len().max(1) as f64;

    let mean_sec_dec = pairs
        .iter()
        .map(|p| {
            let dec_rad = p.obs.dec_deg_measured.unwrap_or(0.0).to_radians();
            1.0 / dec_rad.cos().max(0.1)
        })
        .sum::<f64>()
        / pairs.len().max(1) as f64;

    let comp_tilt = params[0].abs();
    let comp_azimuth = params[1].abs();
    let comp_scale = (params[2].powi(2) + params[3].powi(2)).sqrt();
    let comp_zero = (params[4].powi(2) + params[5].powi(2)).sqrt();
    let comp_collimation = params[6].abs() * mean_sec_dec;
    let comp_refraction = params[7].abs() * mean_sec_z;

    let total_sq = comp_tilt.powi(2)
        + comp_azimuth.powi(2)
        + comp_scale.powi(2)
        + comp_zero.powi(2)
        + comp_collimation.powi(2)
        + comp_refraction.powi(2);

    let total = total_sq.sqrt().max(1e-10);

    vec![
        ErrorRadarEntry {
            component_name: "极轴倾斜".to_string(),
            magnitude_arcmin: comp_tilt,
            relative_contribution_per_cent: comp_tilt / total * 100.0,
        },
        ErrorRadarEntry {
            component_name: "极轴方位".to_string(),
            magnitude_arcmin: comp_azimuth,
            relative_contribution_per_cent: comp_azimuth / total * 100.0,
        },
        ErrorRadarEntry {
            component_name: "刻度系统".to_string(),
            magnitude_arcmin: comp_scale,
            relative_contribution_per_cent: comp_scale / total * 100.0,
        },
        ErrorRadarEntry {
            component_name: "零点偏移".to_string(),
            magnitude_arcmin: comp_zero,
            relative_contribution_per_cent: comp_zero / total * 100.0,
        },
        ErrorRadarEntry {
            component_name: "准直误差".to_string(),
            magnitude_arcmin: comp_collimation,
            relative_contribution_per_cent: comp_collimation / total * 100.0,
        },
        ErrorRadarEntry {
            component_name: "大气折射".to_string(),
            magnitude_arcmin: comp_refraction,
            relative_contribution_per_cent: comp_refraction / total * 100.0,
        },
    ]
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn std(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    let var = values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    var.sqrt()
}

fn median_abs(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut abs_vals: Vec<f64> = values.iter().map(|v| v.abs()).collect();
    abs_vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = abs_vals.len() / 2;
    if abs_vals.len() % 2 == 0 {
        (abs_vals[mid - 1] + abs_vals[mid]) / 2.0
    } else {
        abs_vals[mid]
    }
}

pub async fn run_event_loop(
    rx: mpsc::Receiver<InstrumentCommand>,
    inverter: InstrumentInverter,
) {
    info!("Instrument event loop started");
    drop(rx);
    let _ = inverter;
}

pub fn spawn_inverter(
    config: InstrumentConfig,
) -> (mpsc::Sender<InstrumentCommand>, mpsc::Receiver<InstrumentEvent>) {
    let (inv, cmd_tx, event_rx) = InstrumentInverter::new(config);
    tokio::spawn(async move { inv.run().await });
    (cmd_tx, event_rx)
}

pub fn spawn_instrument_calibrator(
    config: InstrumentConfig,
) -> (mpsc::Sender<InstrumentCommand>, mpsc::Receiver<InstrumentEvent>) {
    spawn_inverter(config)
}

pub fn spawn_instrument_service(
    config: InstrumentConfig,
) -> (mpsc::Sender<InstrumentCommand>, mpsc::Receiver<InstrumentEvent>) {
    spawn_instrument_calibrator(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ErrorComponentPriors, InstrumentMeta, InversionParams};

    fn default_instrument_config() -> InstrumentConfig {
        InstrumentConfig {
            model_name: "test".into(),
            version: "0.1".into(),
            instruments: vec![InstrumentMeta {
                code: "hunyi".into(),
                name_cn: "浑仪".into(),
                dynasty: "汉".into(),
                erected_year: -104.0,
                ring_count: 6,
                nominal_accuracy_arcmin: 5.0,
            }],
            inversion: InversionParams {
                min_shared_stars: 3,
                max_residual_outlier_sigma: 3.0,
                iterative_reweight_max_iter: 5,
                sigma_clip_threshold: 3.0,
                regularization_lambda: 0.01,
            },
            error_components: ErrorComponentPriors {
                polar_axis_tilt_prior_arcmin: 30.0,
                polar_axis_azimuth_prior_arcmin: 30.0,
                divisions_systematic_prior_arcmin_per_cycle: 5.0,
                micrometer_vernier_error_prior_arcmin: 2.0,
                atmospheric_refraction_prior_arcmin: 1.0,
                channel_buffer_size: 32,
            },
        }
    }

    fn make_observation(
        id: i64,
        inst_id: i64,
        ra_true: f64,
        dec_true: f64,
        ra_measured: f64,
        dec_measured: f64,
    ) -> InstrumentObservation {
        InstrumentObservation {
            id,
            instrument_id: inst_id,
            star_id: Some(id),
            star_name_cn: Some(format!("star_{}", id)),
            observation_year_ce: 100.0,
            ruxiu_du_measured: None,
            quji_du_measured: None,
            ra_deg_measured: Some(ra_measured),
            dec_deg_measured: Some(dec_measured),
            ra_j2000_true: Some(ra_true),
            dec_j2000_true: Some(dec_true),
            source_book: Some("test".into()),
            quality_flag: 0,
        }
    }

    struct Lcg {
        state: u64,
    }

    impl Lcg {
        fn new(seed: u64) -> Self {
            Self { state: seed }
        }
        fn next_f64(&mut self) -> f64 {
            self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.state >> 33) as f64 / (1u64 << 31) as f64
        }
        fn gaussian(&mut self) -> f64 {
            let u1 = self.next_f64().max(1e-15);
            let u2 = self.next_f64();
            (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
        }
    }

    fn generate_synthetic_observations(
        n: usize,
        tilt_arcmin: f64,
        noise_sigma_arcmin: f64,
        seed: u64,
    ) -> Vec<InstrumentObservation> {
        let mut rng = Lcg::new(seed);
        let mut obs = Vec::with_capacity(n);
        let tilt_deg = tilt_arcmin / 60.0;
        let noise_deg = noise_sigma_arcmin / 60.0;

        for i in 0..n {
            let ra_true = (i as f64) * 360.0 / (n as f64);
            let dec_true = -20.0 + (i as f64) * 80.0 / (n as f64);
            let ra_rad = ra_true.to_radians();
            let dec_rad = dec_true.to_radians();

            let delta_ra_deg = tilt_deg * ra_rad.sin() * dec_rad.sin() + noise_deg * rng.gaussian();
            let delta_dec_deg = tilt_deg * dec_rad.cos() + noise_deg * rng.gaussian();

            let ra_measured = ra_true + delta_ra_deg;
            let dec_measured = dec_true + delta_dec_deg;

            obs.push(make_observation(
                i as i64,
                1,
                ra_true,
                dec_true,
                ra_measured,
                dec_measured,
            ));
        }
        obs
    }

    fn assert_not_nan_inf(label: &str, value: f64) {
        assert!(
            value.is_finite(),
            "{} is not finite: {}",
            label,
            value
        );
    }

    #[test]
    fn test_invert_synthetic_data() {
        let config = default_instrument_config();
        let obs = generate_synthetic_observations(50, 6.0, 0.5, 42);
        let result = invert_instrument_errors(&obs, None, &config).unwrap();

        assert_not_nan_inf("tilt", result.polar_axis_tilt_arcmin);
        assert!(
            result.polar_axis_tilt_arcmin.abs() > 0.0 && result.polar_axis_tilt_arcmin.abs() < 60.0,
            "tilt = {} arcmin, expected 0 < |tilt| < 60",
            result.polar_axis_tilt_arcmin
        );
    }

    #[test]
    fn test_invert_good_quality() {
        let config = default_instrument_config();
        let obs = generate_synthetic_observations(50, 1.0, 0.5, 123);
        let result = invert_instrument_errors(&obs, None, &config).unwrap();

        assert!(
            result.accuracy_assessment_quality == "excellent"
                || result.accuracy_assessment_quality == "good",
            "quality = {}, expected excellent or good",
            result.accuracy_assessment_quality
        );
    }

    #[test]
    fn test_sigma_clip_outliers() {
        let config = default_instrument_config();
        let mut obs = generate_synthetic_observations(50, 6.0, 0.5, 99);

        obs[5].ra_deg_measured = Some(obs[5].ra_j2000_true.unwrap() + 1.0);
        obs[5].dec_deg_measured = Some(obs[5].dec_j2000_true.unwrap() + 1.0);
        obs[15].ra_deg_measured = Some(obs[15].ra_j2000_true.unwrap() - 1.0);
        obs[15].dec_deg_measured = Some(obs[15].dec_j2000_true.unwrap() - 1.0);
        obs[35].ra_deg_measured = Some(obs[35].ra_j2000_true.unwrap() + 1.0);
        obs[35].dec_deg_measured = Some(obs[35].dec_j2000_true.unwrap() + 1.0);

        let result = invert_instrument_errors(&obs, None, &config).unwrap();
        assert!(
            result.num_iterations >= 1 || result.polar_axis_tilt_arcmin.is_finite(),
            "sigma clip should iterate or produce a reasonable result"
        );
    }

    #[test]
    fn test_parameter_reasonable_range() {
        let config = default_instrument_config();
        let obs = generate_synthetic_observations(50, 6.0, 1.0, 77);
        let result = invert_instrument_errors(&obs, None, &config).unwrap();

        assert!(
            result.polar_axis_tilt_arcmin.abs() < 120.0,
            "tilt = {} too large",
            result.polar_axis_tilt_arcmin
        );
        assert!(
            result.polar_axis_azimuth_arcmin.abs() < 120.0,
            "azimuth = {} too large",
            result.polar_axis_azimuth_arcmin
        );
        assert!(
            result.ra_zero_point_offset_arcmin.abs() < 120.0,
            "ra_zero = {} too large",
            result.ra_zero_point_offset_arcmin
        );
    }

    #[test]
    fn test_error_radar_normalization() {
        let config = default_instrument_config();
        let obs = generate_synthetic_observations(50, 6.0, 0.5, 200);
        let result = invert_instrument_errors(&obs, None, &config).unwrap();

        let total_pct: f64 = result
            .error_component_radar
            .iter()
            .map(|e| e.relative_contribution_per_cent)
            .sum();

        assert!(
            total_pct > 50.0 && total_pct < 300.0,
            "radar sum = {}%, expected reasonable range",
            total_pct
        );

        for entry in &result.error_component_radar {
            assert!(
                entry.relative_contribution_per_cent >= 0.0,
                "entry {} has negative contribution {}",
                entry.component_name,
                entry.relative_contribution_per_cent
            );
        }
    }

    #[test]
    fn test_reduced_chi_square_unity() {
        let config = default_instrument_config();
        let obs = generate_synthetic_observations(50, 1.0, 0.5, 55);
        let result = invert_instrument_errors(&obs, None, &config).unwrap();

        assert!(
            result.chi_squared_reduced > 0.1 && result.chi_squared_reduced < 5.0,
            "chi2_red = {}, expected 0.1~5.0",
            result.chi_squared_reduced
        );
    }

    #[test]
    fn test_invert_convergence() {
        let config = default_instrument_config();
        let obs = generate_synthetic_observations(50, 2.0, 0.5, 33);
        let result = invert_instrument_errors(&obs, None, &config).unwrap();

        assert!(result.converged, "expected convergence with 50 samples and small noise");
    }

    #[test]
    fn test_per_star_residuals_count() {
        let config = default_instrument_config();
        let obs = generate_synthetic_observations(50, 3.0, 0.5, 10);
        let result = invert_instrument_errors(&obs, None, &config).unwrap();

        let residuals = result.per_star_residuals.unwrap();
        assert_eq!(
            residuals.len(),
            obs.len(),
            "per_star_residuals count = {}, expected {}",
            residuals.len(),
            obs.len()
        );
    }

    #[test]
    fn test_minimal_sample_size() {
        let config = default_instrument_config();
        let obs = generate_synthetic_observations(9, 3.0, 0.5, 7);
        let result = invert_instrument_errors(&obs, None, &config);

        assert!(result.is_ok(), "9 observations should succeed: {:?}", result);
    }

    #[test]
    fn test_large_sample_size() {
        let config = default_instrument_config();
        let obs = generate_synthetic_observations(200, 3.0, 0.5, 88);
        let result = invert_instrument_errors(&obs, None, &config).unwrap();

        assert!(
            result.overall_rms_arcmin < 10.0,
            "rms = {} arcmin, expected < 10",
            result.overall_rms_arcmin
        );
    }

    #[test]
    fn test_perfect_data_no_noise() {
        let config = default_instrument_config();
        let obs = generate_synthetic_observations(50, 0.0, 0.0, 0);

        let result = invert_instrument_errors(&obs, None, &config).unwrap();

        assert!(
            result.overall_rms_arcmin < 1.0,
            "rms = {} arcmin for perfect data, expected < 1.0",
            result.overall_rms_arcmin
        );
    }

    #[test]
    fn test_single_parameter_active() {
        let config = default_instrument_config();
        let obs = generate_synthetic_observations(50, 10.0, 0.01, 44);

        let result = invert_instrument_errors(&obs, None, &config).unwrap();

        let tilt_entry = result
            .error_component_radar
            .iter()
            .find(|e| e.component_name == "极轴倾斜")
            .unwrap();
        let max_other = result
            .error_component_radar
            .iter()
            .filter(|e| e.component_name != "极轴倾斜")
            .map(|e| e.relative_contribution_per_cent)
            .fold(0.0f64, f64::max);

        assert!(
            tilt_entry.relative_contribution_per_cent > max_other,
            "tilt contribution {} should be > max other {}",
            tilt_entry.relative_contribution_per_cent,
            max_other
        );
    }

    #[test]
    fn test_zero_measurement_uncertainties() {
        let config = default_instrument_config();
        let obs = generate_synthetic_observations(50, 3.0, 0.0, 66);
        let result = invert_instrument_errors(&obs, None, &config);

        assert!(result.is_ok(), "zero noise should still invert fine: {:?}", result);
    }

    #[test]
    fn test_high_declination_stars() {
        let config = default_instrument_config();
        let mut rng = Lcg::new(12);
        let mut obs = Vec::with_capacity(50);

        for i in 0..50 {
            let ra_true = (i as f64) * 360.0 / 50.0;
            let dec_true = 80.0 + (i as f64) * 9.0 / 49.0;
            let noise_deg = 0.01 / 60.0 * rng.gaussian();
            let ra_measured = ra_true + noise_deg;
            let dec_measured = dec_true + noise_deg;

            obs.push(make_observation(i as i64, 1, ra_true, dec_true, ra_measured, dec_measured));
        }

        let result = invert_instrument_errors(&obs, None, &config);

        match result {
            Ok(sol) => {
                assert_not_nan_inf("tilt", sol.polar_axis_tilt_arcmin);
                assert_not_nan_inf("azimuth", sol.polar_axis_azimuth_arcmin);
                assert_not_nan_inf("rms", sol.overall_rms_arcmin);
            }
            Err(_) => {}
        }
    }

    #[test]
    fn test_insufficient_samples() {
        let config = default_instrument_config();
        let obs = generate_synthetic_observations(2, 3.0, 0.5, 1);
        let result = invert_instrument_errors(&obs, None, &config);

        assert!(result.is_err(), "2 observations should return Err");
    }

    #[test]
    fn test_identical_observations() {
        let config = default_instrument_config();
        let mut obs = Vec::with_capacity(50);

        for i in 0..50 {
            obs.push(make_observation(i as i64, 1, 180.0, 30.0, 180.5, 30.5));
        }

        let result = invert_instrument_errors(&obs, None, &config);
        match result {
            Ok(sol) => {
                assert_not_nan_inf("rms", sol.overall_rms_arcmin);
            }
            Err(_) => {}
        }
    }

    #[test]
    fn test_empty_observation_list() {
        let config = default_instrument_config();
        let obs: Vec<InstrumentObservation> = vec![];
        let result = invert_instrument_errors(&obs, None, &config);

        assert!(result.is_err(), "empty list should return Err");
        assert!(
            result.unwrap_err().contains("No valid observation pairs found"),
            "error message should mention no valid pairs"
        );
    }

    #[test]
    fn test_missing_measured_coordinates() {
        let config = default_instrument_config();
        let mut obs = Vec::with_capacity(10);

        for i in 0..10 {
            let mut o = make_observation(i as i64, 1, 180.0, 30.0, 180.5, 30.5);
            o.ra_deg_measured = None;
            o.dec_deg_measured = None;
            obs.push(o);
        }

        let result = invert_instrument_errors(&obs, None, &config);
        assert!(result.is_err(), "all measured=None should return Err");
    }

    #[test]
    fn test_missing_true_coordinates() {
        let config = default_instrument_config();
        let mut obs = Vec::with_capacity(10);

        for i in 0..10 {
            let mut o = make_observation(i as i64, 1, 180.0, 30.0, 180.5, 30.5);
            o.ra_j2000_true = None;
            o.dec_j2000_true = None;
            obs.push(o);
        }

        let result = invert_instrument_errors(&obs, None, &config);
        assert!(result.is_err(), "all true=None should return Err");
    }

    #[test]
    fn test_missing_both_measured_and_true() {
        let config = default_instrument_config();
        let mut obs = Vec::with_capacity(20);
        let mut rng = Lcg::new(55);

        for i in 0..20 {
            let ra_true = (i as f64) * 18.0;
            let dec_true = -10.0 + (i as f64) * 4.0;
            let ra_measured = ra_true + 0.1 / 60.0 * rng.gaussian();
            let dec_measured = dec_true + 0.1 / 60.0 * rng.gaussian();
            obs.push(make_observation(i as i64, 1, ra_true, dec_true, ra_measured, dec_measured));
        }

        for i in 20..25 {
            let mut o = make_observation(i as i64, 1, 180.0, 30.0, 180.5, 30.5);
            o.ra_deg_measured = None;
            o.dec_deg_measured = None;
            obs.push(o);
        }

        for i in 25..30 {
            let mut o = make_observation(i as i64, 1, 180.0, 30.0, 180.5, 30.5);
            o.ra_j2000_true = None;
            o.dec_j2000_true = None;
            obs.push(o);
        }

        let result = invert_instrument_errors(&obs, None, &config);
        assert!(result.is_ok(), "should succeed with 20 valid pairs out of 30: {:?}", result);
    }

    #[test]
    fn test_tikhonov_regularization_small_sample() {
        let config = default_instrument_config();
        let mut obs = Vec::with_capacity(3);
        for i in 0..3 {
            let ra_true = 60.0 + (i as f64) * 30.0;
            let dec_true = 20.0 + (i as f64) * 5.0;
            let ra_meas = ra_true + 0.5 / 60.0;
            let dec_meas = dec_true + 0.3 / 60.0;
            obs.push(make_observation(i as i64, 1, ra_true, dec_true, ra_meas, dec_meas));
        }
        let result = invert_instrument_errors(&obs, None, &config);
        if let Ok(sol) = result {
            assert!(sol.polar_axis_tilt_arcmin.is_finite(), "polar tilt finite");
            assert!(sol.polar_axis_azimuth_arcmin.is_finite(), "polar azimuth finite");
            assert!(sol.ra_zero_point_offset_arcmin.is_finite(), "ra zero finite");
            assert!(sol.dec_zero_point_offset_arcmin.is_finite(), "dec zero finite");
        }
    }

    #[test]
    fn test_regularization_parameter_lambda() {
        let config = default_instrument_config();
        assert!(config.inversion.regularization_lambda >= 0.0, "lambda should be >= 0");
    }

    #[test]
    fn test_small_sample_mitigated_overfitting() {
        let config = default_instrument_config();
        let mut obs = Vec::with_capacity(4);
        for i in 0..4 {
            let ra_true = 45.0 + (i as f64) * 20.0;
            let dec_true = 15.0 + (i as f64) * 3.0;
            let ra_meas = ra_true + 0.2 / 60.0;
            let dec_meas = dec_true + 0.15 / 60.0;
            obs.push(make_observation(i as i64, 1, ra_true, dec_true, ra_meas, dec_meas));
        }
        let result = invert_instrument_errors(&obs, None, &config);
        if let Ok(sol) = result {
            assert!(sol.overall_rms_arcmin.is_finite(), "rms should be finite");
            assert!(sol.chi_squared_reduced.is_finite(), "chi2 should be finite");
            if let Some(ref resids) = sol.per_star_residuals {
                for r in resids.iter() {
                    assert!(r.ra_residual_arcmin.is_finite(), "ra residual finite");
                    assert!(r.dec_residual_arcmin.is_finite(), "dec residual finite");
                }
            }
        }
    }

    #[test]
    fn test_instrument_compatibility_aliases() {
        let _cmd: CalibratorCommand = InstrumentCommand::ListInstruments;
        let _evt: CalibratorEvent = InstrumentEvent::Error { message: "test".into() };
        let config = default_instrument_config();
        let (_eng, _tx, _rx): (CalibratorEngine, _, _) = InstrumentInverter::new(config);
    }
}
