//! 变星亮度演化分析模块
//!
//! 职责:
//!   1. Lomb-Scargle 周期图分析
//!   2. 古代星等文字描述数值化
//!   3. 长期光变曲线重建
//!   4. 周期变化率 Ṗ 估计
//!
//! 所有模型参数从 config::VariableConfig 加载

use crate::config::VariableConfig;
use crate::models::{
    LightCurveReconstruction, LightCurveSample, LombScargleResult, MagnitudeMeasurement,
    PeriodogramPeak, PhaseFoldedSample, VariableStarMeta,
};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use std::f64::consts::SQRT_2;
use tokio::sync::mpsc;
use tracing::info;

const TWO_PI: f64 = 2.0 * PI;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VariableStarCommand {
    ListVariables {
        gcvs_type: Option<String>,
        min_amplitude_mag: Option<f64>,
        max_period_days: Option<f64>,
        search_name: Option<String>,
        limit: Option<i64>,
        offset: Option<i64>,
    },
    GetLightCurve {
        variable_id: i64,
        measurements: Vec<MagnitudeMeasurement>,
        use_published_period: Option<bool>,
        override_period_days: Option<f64>,
        include_ancient_only_fit: Option<bool>,
        reconstruction_resolution_per_phase: Option<i32>,
    },
    ReconstructLongTerm {
        variable_id: i64,
        meta: VariableStarMeta,
        measurements: Vec<MagnitudeMeasurement>,
    },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VariableStarEvent {
    VariableList {
        variables: Vec<VariableStarMeta>,
        total: i64,
    },
    LightCurveResult {
        variable_id: i64,
        reconstruction: LightCurveReconstruction,
    },
    LongTermReconstruction {
        variable_id: i64,
        reconstruction: LightCurveReconstruction,
    },
    Error {
        message: String,
    },
    ShutdownAck,
}

pub struct VariableStarEngine {
    cfg: VariableConfig,
}

impl VariableStarEngine {
    pub fn new(cfg: VariableConfig) -> Self {
        Self { cfg }
    }

    fn magnitude_text_to_value_uncertainty(&self, text: &str) -> (f64, f64) {
        let t = text.trim();
        for bracket in &self.cfg.ancient_magnitude_brackets {
            if t.contains(&bracket.text) {
                let uncertainty = self.cfg.magnitude_text_to_value_uncertainty;
                return (bracket.mag_default, uncertainty);
            }
        }
        (self.cfg.mira_variable_template.default_mean_mag, 1.0)
    }

    fn lomb_scargle_periodogram(
        &self,
        times: &[f64],
        mags: &[f64],
        errs: &[f64],
    ) -> LombScargleResult {
        let ls_cfg = &self.cfg.lomb_scargle;
        let n = times.len();
        if n < 3 {
            return LombScargleResult {
                frequencies_per_day: vec![],
                periods_days: vec![],
                power: vec![],
                peaks: vec![],
                false_alarm_probability_threshold: ls_cfg.false_alarm_level,
                false_alarm_power_level: 0.0,
            };
        }

        let t_min = times.iter().cloned().fold(f64::INFINITY, f64::min);
        let t_max = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let t_span = (t_max - t_min).max(1.0);

        let freq_min = 1.0 / ls_cfg.max_period_days;
        let freq_max = 1.0 / ls_cfg.min_period_days;

        let df = 1.0 / (t_span * ls_cfg.freq_oversampling_factor);
        let n_freq = ((freq_max - freq_min) / df).ceil() as usize;
        let n_freq = n_freq.max(100);

        let mean_mag = mags.iter().sum::<f64>() / n as f64;
        let variance = mags
            .iter()
            .map(|m| (m - mean_mag).powi(2))
            .sum::<f64>()
            / n as f64;
        let variance = variance.max(1e-10);

        let mut frequencies = Vec::with_capacity(n_freq);
        let mut periods = Vec::with_capacity(n_freq);
        let mut power = Vec::with_capacity(n_freq);

        for i in 0..n_freq {
            let freq = freq_min + i as f64 * df;
            let omega = TWO_PI * freq;

            let mut sum_sin_2wt = 0.0;
            let mut sum_cos_2wt = 0.0;
            let mut sum_y_cos = 0.0;
            let mut sum_y_sin = 0.0;
            let mut sum_cos2 = 0.0;
            let mut sum_sin2 = 0.0;

            for j in 0..n {
                let wt = omega * times[j];
                let y = mags[j] - mean_mag;
                let w = if errs[j] > 0.0 {
                    1.0 / errs[j].powi(2)
                } else {
                    1.0
                };

                let sin_wt = libm::sin(wt);
                let cos_wt = libm::cos(wt);
                let sin_2wt = 2.0 * sin_wt * cos_wt;
                let cos_2wt = cos_wt * cos_wt - sin_wt * sin_wt;

                sum_sin_2wt += w * sin_2wt;
                sum_cos_2wt += w * cos_2wt;
                sum_y_cos += w * y * cos_wt;
                sum_y_sin += w * y * sin_wt;
                sum_cos2 += w * cos_wt * cos_wt;
                sum_sin2 += w * sin_wt * sin_wt;
            }

            let tau = 0.5 * libm::atan2(sum_sin_2wt, sum_cos_2wt);

            let sin_omega_tau = libm::sin(omega * tau);
            let cos_omega_tau = libm::cos(omega * tau);

            let wc_term = sum_y_cos * cos_omega_tau + sum_y_sin * sin_omega_tau;
            let ws_term = sum_y_sin * cos_omega_tau - sum_y_cos * sin_omega_tau;

            let cc_term = sum_cos2 * cos_omega_tau * cos_omega_tau
                + sum_sin2 * sin_omega_tau * sin_omega_tau;
            let ss_term = sum_sin2 * cos_omega_tau * cos_omega_tau
                + sum_cos2 * sin_omega_tau * sin_omega_tau;

            let p = if cc_term > 0.0 && ss_term > 0.0 {
                (wc_term * wc_term / cc_term + ws_term * ws_term / ss_term)
                    / (2.0 * variance)
            } else {
                0.0
            };

            frequencies.push(freq);
            periods.push(1.0 / freq);
            power.push(p);
        }

        let peaks = self.find_peaks(&frequencies, &periods, &power, n);

        let fap_level = self.baluev_fap_level(&power, n, ls_cfg.false_alarm_level);

        LombScargleResult {
            frequencies_per_day: frequencies,
            periods_days: periods,
            power,
            peaks,
            false_alarm_probability_threshold: ls_cfg.false_alarm_level,
            false_alarm_power_level: fap_level,
        }
    }

    fn find_peaks(
        &self,
        frequencies: &[f64],
        periods: &[f64],
        power: &[f64],
        n_data: usize,
    ) -> Vec<PeriodogramPeak> {
        let ls_cfg = &self.cfg.lomb_scargle;
        let n = frequencies.len();
        let mut peaks = Vec::new();

        if n < 3 {
            return peaks;
        }

        for i in 1..n - 1 {
            if power[i] > power[i - 1] && power[i] > power[i + 1] {
                let fap = self.baluev_approx_fap(power[i], n_data, n);
                peaks.push(PeriodogramPeak {
                    rank: 0,
                    frequency_per_day: frequencies[i],
                    period_days: periods[i],
                    power: power[i],
                    false_alarm_probability: fap,
                    alias_of_fundamental: false,
                });
            }
        }

        peaks.sort_by(|a, b| {
            b.power
                .partial_cmp(&a.power)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for (i, p) in peaks.iter_mut().enumerate() {
            p.rank = (i + 1) as i32;
        }

        if !peaks.is_empty() {
            let fund_freq = peaks[0].frequency_per_day;
            for p in peaks.iter_mut().skip(1) {
                let ratio = p.frequency_per_day / fund_freq;
                let harmonic_num = ratio.round();
                if harmonic_num >= 2.0
                    && harmonic_num <= 10.0
                    && (ratio - harmonic_num).abs() / harmonic_num < 0.05
                {
                    p.alias_of_fundamental = true;
                }
            }
        }

        peaks.truncate(ls_cfg.num_peaks_to_return as usize);
        peaks
    }

    fn baluev_approx_fap(&self, power: f64, _n_data: usize, n_freq: usize) -> f64 {
        if power <= 0.0 {
            return 1.0;
        }
        let n_eff = n_freq as f64 * (2.0 * SQRT_2 / PI);
        let z = power;
        let exp_term = libm::exp(-z);
        let fap_single = exp_term;
        let fap = 1.0 - (1.0 - fap_single).powf(n_eff);
        fap.max(0.0).min(1.0)
    }

    fn baluev_fap_level(&self, power: &[f64], n_data: usize, fap_target: f64) -> f64 {
        if power.is_empty() {
            return 0.0;
        }
        let p_max = power.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let mut lo = 0.0;
        let mut hi = p_max.max(1.0);
        for _ in 0..50 {
            let mid = (lo + hi) * 0.5;
            let fap = self.baluev_approx_fap(mid, n_data, power.len());
            if fap > fap_target {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        (lo + hi) * 0.5
    }

    fn phase_fold(
        &self,
        times: &[f64],
        mags: &[f64],
        errs: &[f64],
        period_days: f64,
    ) -> Vec<PhaseFoldedSample> {
        if times.is_empty() {
            return vec![];
        }
        let t0 = times[0];
        let mut samples: Vec<PhaseFoldedSample> = times
            .iter()
            .zip(mags.iter())
            .zip(errs.iter())
            .map(|((t, m), e)| {
                let phase = ((t - t0) / period_days) % 1.0;
                let phase = if phase < 0.0 { phase + 1.0 } else { phase };
                PhaseFoldedSample {
                    phase,
                    magnitude: *m,
                    magnitude_err: Some(*e),
                    epoch_yr: *t,
                }
            })
            .collect();

        samples.sort_by(|a, b| {
            a.phase
                .partial_cmp(&b.phase)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        samples
    }

    fn sinusoid_fit(
        &self,
        times: &[f64],
        mags: &[f64],
        errs: &[f64],
        period_days: f64,
    ) -> (f64, f64, f64, f64) {
        let n = times.len();
        if n < 3 {
            return (0.0, 0.0, 0.0, 0.0);
        }

        let omega = TWO_PI / period_days;

        let mut sum_w = 0.0;
        let mut sum_wy = 0.0;
        let mut sum_w_cos = 0.0;
        let mut sum_w_sin = 0.0;
        let mut sum_wy_cos = 0.0;
        let mut sum_wy_sin = 0.0;
        let mut sum_w_cos2 = 0.0;
        let mut sum_w_sin2 = 0.0;
        let mut sum_w_cos_sin = 0.0;

        for i in 0..n {
            let w = if errs[i] > 0.0 {
                1.0 / errs[i].powi(2)
            } else {
                1.0
            };
            let wt = omega * times[i];
            let c = libm::cos(wt);
            let s = libm::sin(wt);
            let y = mags[i];

            sum_w += w;
            sum_wy += w * y;
            sum_w_cos += w * c;
            sum_w_sin += w * s;
            sum_wy_cos += w * y * c;
            sum_wy_sin += w * y * s;
            sum_w_cos2 += w * c * c;
            sum_w_sin2 += w * s * s;
            sum_w_cos_sin += w * c * s;
        }

        let mean = sum_wy / sum_w;

        let _s_yy = sum_wy * sum_wy / sum_w;
        let s_cy = sum_wy_cos - sum_wy * sum_w_cos / sum_w;
        let s_sy = sum_wy_sin - sum_wy * sum_w_sin / sum_w;
        let s_cc = sum_w_cos2 - sum_w_cos * sum_w_cos / sum_w;
        let s_ss = sum_w_sin2 - sum_w_sin * sum_w_sin / sum_w;
        let s_cs = sum_w_cos_sin - sum_w_cos * sum_w_sin / sum_w;

        let det = s_cc * s_ss - s_cs * s_cs;
        let (a_cos, a_sin) = if det.abs() > 1e-20 {
            let a_c = (s_cy * s_ss - s_sy * s_cs) / det;
            let a_s = (s_sy * s_cc - s_cy * s_cs) / det;
            (a_c, a_s)
        } else {
            (0.0, 0.0)
        };

        let amplitude = libm::sqrt(a_cos * a_cos + a_sin * a_sin);
        let phase_offset = libm::atan2(-a_sin, a_cos);

        (mean, amplitude, phase_offset, 0.0)
    }

    fn linear_trend_fit(
        &self,
        times: &[f64],
        mags: &[f64],
        errs: &[f64],
    ) -> (f64, f64) {
        let n = times.len();
        if n < 2 {
            return (0.0, 0.0);
        }

        let mut sum_w = 0.0;
        let mut sum_wx = 0.0;
        let mut sum_wy = 0.0;
        let mut sum_wxx = 0.0;
        let mut sum_wxy = 0.0;

        for i in 0..n {
            let w = if errs[i] > 0.0 {
                1.0 / errs[i].powi(2)
            } else {
                1.0
            };
            let x = times[i];
            let y = mags[i];

            sum_w += w;
            sum_wx += w * x;
            sum_wy += w * y;
            sum_wxx += w * x * x;
            sum_wxy += w * x * y;
        }

        let det = sum_w * sum_wxx - sum_wx * sum_wx;
        if det.abs() < 1e-20 {
            return (0.0, sum_wy / sum_w);
        }

        let slope = (sum_w * sum_wxy - sum_wx * sum_wy) / det;
        let intercept = (sum_wy * sum_wxx - sum_wx * sum_wxy) / det;

        (slope, intercept)
    }

    fn chi_squared(
        &self,
        times: &[f64],
        mags: &[f64],
        errs: &[f64],
        _mean: f64,
        amplitude: f64,
        phase_offset: f64,
        period_days: f64,
        trend_slope: f64,
        trend_intercept: f64,
    ) -> f64 {
        let omega = TWO_PI / period_days;
        let mut chi2 = 0.0;
        for i in 0..times.len() {
            let model = trend_slope * times[i] + trend_intercept
                + amplitude * libm::cos(omega * times[i] + phase_offset);
            let err = errs[i].max(0.01);
            chi2 += ((mags[i] - model) / err).powi(2);
        }
        chi2
    }

    fn prepare_measurements(
        &self,
        measurements: &[MagnitudeMeasurement],
    ) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<bool>) {
        let mut times = Vec::new();
        let mut mags = Vec::new();
        let mut errs = Vec::new();
        let mut is_ancient = Vec::new();

        for m in measurements {
            let t = if let Some(mjd) = m.epoch_mjd {
                mjd / 365.25 + 1858.87
            } else {
                m.epoch_yr
            };

            let (mag, err) = if m.source_type == "ancient"
                || m.ancient_description.is_some()
            {
                if let Some(desc) = &m.ancient_description {
                    let (m_val, m_err) = self.magnitude_text_to_value_uncertainty(desc);
                    (m_val, m_err)
                } else {
                    (m.magnitude, m.magnitude_uncertainty.unwrap_or(0.5))
                }
            } else {
                (m.magnitude, m.magnitude_uncertainty.unwrap_or(0.01))
            };

            times.push(t);
            mags.push(mag);
            errs.push(err);
            is_ancient.push(m.source_type == "ancient");
        }

        (times, mags, errs, is_ancient)
    }

    fn reconstruct_light_curve(
        &self,
        variable_id: i64,
        meta: Option<&VariableStarMeta>,
        measurements: &[MagnitudeMeasurement],
        override_period: Option<f64>,
        _resolution_per_phase: i32,
    ) -> LightCurveReconstruction {
        let (times, mags, errs, is_ancient) = self.prepare_measurements(measurements);
        let n = times.len();

        let num_ancient = is_ancient.iter().filter(|&&x| x).count();
        let num_modern = n - num_ancient;

        let coverage_start = times.iter().cloned().fold(f64::INFINITY, f64::min);
        let coverage_end = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        let periodogram = self.lomb_scargle_periodogram(&times, &mags, &errs);

        let best_period = if let Some(p) = override_period {
            p
        } else if let Some(peak) = periodogram.peaks.first() {
            peak.period_days
        } else {
            self.cfg.mira_variable_template.default_period_days
        };

        let (mean_mag, amplitude, phase_offset, _) =
            self.sinusoid_fit(&times, &mags, &errs, best_period);

        let (trend_slope, trend_intercept) = self.linear_trend_fit(&times, &mags, &errs);

        let chi2 = self.chi_squared(
            &times,
            &mags,
            &errs,
            mean_mag,
            amplitude,
            phase_offset,
            best_period,
            trend_slope,
            trend_intercept,
        );

        let dof = (n as i32 - 5).max(1) as f64;
        let reduced_chi2 = chi2 / dof;

        let phase_folded = self.phase_fold(&times, &mags, &errs, best_period);

        let n_samples = 200;
        let t_min = times.iter().cloned().fold(f64::INFINITY, f64::min);
        let t_max = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let dt = (t_max - t_min).max(1.0) / (n_samples - 1) as f64;
        let omega = TWO_PI / best_period;

        let mut reconstructed = Vec::with_capacity(n_samples);
        for i in 0..n_samples {
            let t = t_min + i as f64 * dt;
            let model = trend_slope * t + trend_intercept
                + amplitude * libm::cos(omega * t + phase_offset);
            let model_err = amplitude * 0.1 + trend_slope.abs() * (t - t_min).abs() * 0.01;

            reconstructed.push(LightCurveSample {
                epoch_yr: t,
                model_magnitude: model,
                model_lower_ci: model - model_err,
                model_upper_ci: model + model_err,
                observed_magnitude: None,
                passband: None,
                source_type: None,
            });
        }

        for (i, m) in measurements.iter().enumerate() {
            if i < times.len() {
                reconstructed.push(LightCurveSample {
                    epoch_yr: times[i],
                    model_magnitude: mags[i],
                    model_lower_ci: mags[i] - errs[i],
                    model_upper_ci: mags[i] + errs[i],
                    observed_magnitude: Some(mags[i]),
                    passband: Some(m.passband.clone()),
                    source_type: Some(m.source_type.clone()),
                });
            }
        }

        reconstructed.sort_by(|a, b| {
            a.epoch_yr
                .partial_cmp(&b.epoch_yr)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let (ancient_period, _modern_period, pdot, period_delta, pdot_sigma) =
            self.estimate_pdot(&times, &mags, &errs, &is_ancient, best_period);

        let longterm_trend = trend_slope * 1e6;

        let notes = if num_ancient == 0 {
            "无古代观测数据，仅基于现代数据重建".to_string()
        } else if num_modern == 0 {
            "仅古代数据，周期估计可能有较大不确定性".to_string()
        } else {
            format!(
                "结合古代({})与现代({})观测数据重建",
                num_ancient, num_modern
            )
        };

        let modern_name = meta
            .map(|m| m.modern_name.clone())
            .unwrap_or_else(|| format!("V{}", variable_id));
        let gcvs_type = meta
            .map(|m| m.gcvs_variable_type.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        LightCurveReconstruction {
            variable_id,
            modern_name,
            gcvs_type,
            num_ancient_measurements: num_ancient,
            num_modern_measurements: num_modern,
            coverage_start_yr: coverage_start,
            coverage_end_yr: coverage_end,
            reconstructed_samples: reconstructed,
            phase_folded_samples: Some(phase_folded),
            periodogram,
            best_period_days: best_period,
            best_period_uncertainty_days: best_period * 0.01,
            best_fit_amplitude_mag: amplitude,
            best_fit_mean_mag: mean_mag,
            phase_offset,
            chi_squared: chi2,
            reduced_chi_squared: reduced_chi2,
            period_change_significance_sigma: pdot_sigma,
            pdot_estimate: pdot,
            ancient_vs_modern_period_delta_days: period_delta,
            ancient_period_determination_days: ancient_period,
            longterm_trend_mag_per_myr: Some(longterm_trend),
            reconstruction_notes: notes,
        }
    }

    fn estimate_pdot(
        &self,
        times: &[f64],
        mags: &[f64],
        errs: &[f64],
        is_ancient: &[bool],
        overall_period: f64,
    ) -> (Option<f64>, Option<f64>, f64, Option<f64>, f64) {
        let ancient_indices: Vec<usize> = (0..times.len())
            .filter(|&i| is_ancient[i])
            .collect();
        let modern_indices: Vec<usize> = (0..times.len())
            .filter(|&i| !is_ancient[i])
            .collect();

        if ancient_indices.len() < 5 || modern_indices.len() < 5 {
            return (None, None, 0.0, None, 0.0);
        }

        let a_times: Vec<f64> = ancient_indices.iter().map(|&i| times[i]).collect();
        let a_mags: Vec<f64> = ancient_indices.iter().map(|&i| mags[i]).collect();
        let a_errs: Vec<f64> = ancient_indices.iter().map(|&i| errs[i]).collect();

        let m_times: Vec<f64> = modern_indices.iter().map(|&i| times[i]).collect();
        let m_mags: Vec<f64> = modern_indices.iter().map(|&i| mags[i]).collect();
        let m_errs: Vec<f64> = modern_indices.iter().map(|&i| errs[i]).collect();

        let a_ls = self.lomb_scargle_periodogram(&a_times, &a_mags, &a_errs);
        let m_ls = self.lomb_scargle_periodogram(&m_times, &m_mags, &m_errs);

        let a_period = a_ls
            .peaks
            .first()
            .map(|p| p.period_days)
            .unwrap_or(overall_period);
        let m_period = m_ls
            .peaks
            .first()
            .map(|p| p.period_days)
            .unwrap_or(overall_period);

        let a_t_mean = a_times.iter().sum::<f64>() / a_times.len() as f64;
        let m_t_mean = m_times.iter().sum::<f64>() / m_times.len() as f64;
        let delta_t_yr = (m_t_mean - a_t_mean).abs();

        if delta_t_yr < 10.0 {
            return (
                Some(a_period),
                Some(m_period),
                0.0,
                Some(m_period - a_period),
                0.0,
            );
        }

        let delta_p = m_period - a_period;
        let pdot = delta_p / (delta_t_yr * 365.25);

        let a_period_err = a_period * 0.05;
        let m_period_err = m_period * 0.01;
        let delta_p_err = libm::sqrt(a_period_err.powi(2) + m_period_err.powi(2));
        let sigma = if delta_p_err > 0.0 {
            delta_p.abs() / delta_p_err
        } else {
            0.0
        };

        (
            Some(a_period),
            Some(m_period),
            pdot,
            Some(delta_p),
            sigma,
        )
    }
}

pub fn reconstruct_light_curve(
    measurements: &[MagnitudeMeasurement],
    config: &VariableConfig,
) -> LightCurveReconstruction {
    let engine = VariableStarEngine::new(config.clone());
    engine.reconstruct_light_curve(0, None, measurements, None, 100)
}

pub struct VariableStarService {
    engine: VariableStarEngine,
    cmd_rx: mpsc::Receiver<VariableStarCommand>,
    event_tx: mpsc::Sender<VariableStarEvent>,
}

impl VariableStarService {
    pub fn new(
        config: VariableConfig,
    ) -> (
        Self,
        mpsc::Sender<VariableStarCommand>,
        mpsc::Receiver<VariableStarEvent>,
    ) {
        let buf_size = config.channel_buffer_size;
        let (cmd_tx, cmd_rx) = mpsc::channel(buf_size);
        let (event_tx, event_rx) = mpsc::channel(buf_size);
        (
            Self {
                engine: VariableStarEngine::new(config),
                cmd_rx,
                event_tx,
            },
            cmd_tx,
            event_rx,
        )
    }

    pub async fn run(mut self) {
        info!("VariableStarService started");
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                VariableStarCommand::ListVariables { .. } => {
                    let event = VariableStarEvent::Error {
                        message: "ListVariables requires database access".into(),
                    };
                    let _ = self.event_tx.send(event).await;
                }
                VariableStarCommand::GetLightCurve {
                    variable_id,
                    measurements,
                    override_period_days,
                    reconstruction_resolution_per_phase,
                    ..
                } => {
                    let reconstruction = self.engine.reconstruct_light_curve(
                        variable_id,
                        None,
                        &measurements,
                        override_period_days,
                        reconstruction_resolution_per_phase.unwrap_or(100),
                    );
                    let event = VariableStarEvent::LightCurveResult {
                        variable_id,
                        reconstruction,
                    };
                    let _ = self.event_tx.send(event).await;
                }
                VariableStarCommand::ReconstructLongTerm {
                    variable_id,
                    meta,
                    measurements,
                } => {
                    let reconstruction = self.engine.reconstruct_light_curve(
                        variable_id,
                        Some(&meta),
                        &measurements,
                        meta.published_period_days,
                        100,
                    );
                    let event = VariableStarEvent::LongTermReconstruction {
                        variable_id,
                        reconstruction,
                    };
                    let _ = self.event_tx.send(event).await;
                }
                VariableStarCommand::Shutdown => {
                    info!("VariableStarService shutting down");
                    let _ = self.event_tx.send(VariableStarEvent::ShutdownAck).await;
                    break;
                }
            }
        }
    }
}

pub async fn run_event_loop(
    mut rx: mpsc::Receiver<VariableStarCommand>,
    engine: VariableStarEngine,
) {
    info!("VariableStar event loop started");
    while let Some(cmd) = rx.recv().await {
        match cmd {
            VariableStarCommand::ListVariables { .. } => {
                info!("ListVariables command received (requires DB)");
            }
            VariableStarCommand::GetLightCurve {
                variable_id,
                measurements,
                override_period_days,
                reconstruction_resolution_per_phase,
                ..
            } => {
                let _ = engine.reconstruct_light_curve(
                    variable_id,
                    None,
                    &measurements,
                    override_period_days,
                    reconstruction_resolution_per_phase.unwrap_or(100),
                );
            }
            VariableStarCommand::ReconstructLongTerm {
                variable_id,
                meta,
                measurements,
            } => {
                let _ = engine.reconstruct_light_curve(
                    variable_id,
                    Some(&meta),
                    &measurements,
                    meta.published_period_days,
                    100,
                );
            }
            VariableStarCommand::Shutdown => {
                info!("VariableStar event loop shutting down");
                break;
            }
        }
    }
}

pub fn spawn_variable_star_service(
    config: VariableConfig,
) -> (
    mpsc::Sender<VariableStarCommand>,
    mpsc::Receiver<VariableStarEvent>,
) {
    let (service, cmd_tx, event_rx) = VariableStarService::new(config);
    tokio::spawn(async move {
        service.run().await;
    });
    (cmd_tx, event_rx)
}
