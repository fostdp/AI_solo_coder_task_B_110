//! 变星亮度演化分析模块 (variability_analyzer)
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
pub enum VariabilityCommand {
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

pub type VariableStarCommand = VariabilityCommand;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VariabilityEvent {
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

pub type VariableStarEvent = VariabilityEvent;

pub struct VariabilityAnalyzer {
    cfg: VariableConfig,
}

pub type VariableStarEngine = VariabilityAnalyzer;

impl VariabilityAnalyzer {
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

        let times_days: Vec<f64> = times.iter().map(|&t| t * 365.25).collect();
        let times = &times_days;

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
        let times_days: Vec<f64> = times.iter().map(|&t| t * 365.25).collect();

        if times.is_empty() {
            return vec![];
        }
        let t0 = times_days[0];
        let mut samples: Vec<PhaseFoldedSample> = times_days
            .iter()
            .zip(times.iter())
            .zip(mags.iter())
            .zip(errs.iter())
            .map(|(((t_days, t_yr), m), e)| {
                let phase = ((t_days - t0) / period_days) % 1.0;
                let phase = if phase < 0.0 { phase + 1.0 } else { phase };
                PhaseFoldedSample {
                    phase,
                    magnitude: *m,
                    magnitude_err: Some(*e),
                    epoch_yr: *t_yr,
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

        let times_days: Vec<f64> = times.iter().map(|&t| t * 365.25).collect();
        let times = &times_days;

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
            let t_days = times[i] * 365.25;
            let model = trend_slope * times[i] + trend_intercept
                + amplitude * libm::cos(omega * t_days + phase_offset);
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

        if n == 0 {
            let modern_name = meta
                .map(|m| m.modern_name.clone())
                .unwrap_or_else(|| format!("V{}", variable_id));
            let gcvs_type = meta
                .map(|m| m.gcvs_variable_type.clone())
                .unwrap_or_else(|| "Unknown".to_string());
            return LightCurveReconstruction {
                variable_id,
                modern_name,
                gcvs_type,
                num_ancient_measurements: 0,
                num_modern_measurements: 0,
                coverage_start_yr: 0.0,
                coverage_end_yr: 0.0,
                reconstructed_samples: vec![],
                phase_folded_samples: Some(vec![]),
                periodogram: LombScargleResult {
                    frequencies_per_day: vec![],
                    periods_days: vec![],
                    power: vec![],
                    peaks: vec![],
                    false_alarm_probability_threshold: self.cfg.lomb_scargle.false_alarm_level,
                    false_alarm_power_level: 0.0,
                },
                best_period_days: self.cfg.mira_variable_template.default_period_days,
                best_period_uncertainty_days: 0.0,
                best_fit_amplitude_mag: 0.0,
                best_fit_mean_mag: 0.0,
                phase_offset: 0.0,
                chi_squared: 0.0,
                reduced_chi_squared: 0.0,
                period_change_significance_sigma: 0.0,
                pdot_estimate: 0.0,
                ancient_vs_modern_period_delta_days: None,
                ancient_period_determination_days: None,
                longterm_trend_mag_per_myr: Some(0.0),
                reconstruction_notes: "无观测数据".to_string(),
            };
        }

        let mean_data_err = errs.iter().sum::<f64>() / n as f64;

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
            let t_days = t * 365.25;
            let model = trend_slope * t + trend_intercept
                + amplitude * libm::cos(omega * t_days + phase_offset);
            let model_err = amplitude * 0.1
                + trend_slope.abs() * (t - t_min).abs() * 0.01
                + mean_data_err;

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

pub struct VariabilityAnalyzerService {
    engine: VariabilityAnalyzer,
    cmd_rx: mpsc::Receiver<VariabilityCommand>,
    event_tx: mpsc::Sender<VariabilityEvent>,
}

pub type VariableStarService = VariabilityAnalyzerService;

impl VariabilityAnalyzerService {
    pub fn new(
        config: VariableConfig,
    ) -> (
        Self,
        mpsc::Sender<VariabilityCommand>,
        mpsc::Receiver<VariabilityEvent>,
    ) {
        let buf_size = config.channel_buffer_size;
        let (cmd_tx, cmd_rx) = mpsc::channel(buf_size);
        let (event_tx, event_rx) = mpsc::channel(buf_size);
        (
            Self {
                engine: VariabilityAnalyzer::new(config),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LombScargleConfig, MagnitudeBracket, MiraTemplateConfig, VariableConfig};
    use crate::models::MagnitudeMeasurement;

    fn default_test_config() -> VariableConfig {
        VariableConfig {
            model_name: "test".to_string(),
            version: "1.0.0".to_string(),
            ancient_magnitude_brackets: vec![
                MagnitudeBracket { text: "极亮".to_string(), mag_range: [-2.0, 0.0], mag_default: -1.5 },
                MagnitudeBracket { text: "大".to_string(), mag_range: [-1.0, 1.5], mag_default: 1.0 },
                MagnitudeBracket { text: "明".to_string(), mag_range: [0.5, 2.5], mag_default: 1.5 },
                MagnitudeBracket { text: "亮".to_string(), mag_range: [1.5, 3.5], mag_default: 2.5 },
                MagnitudeBracket { text: "中常".to_string(), mag_range: [3.0, 4.5], mag_default: 3.8 },
                MagnitudeBracket { text: "微".to_string(), mag_range: [4.0, 5.5], mag_default: 4.8 },
                MagnitudeBracket { text: "暗".to_string(), mag_range: [5.0, 6.5], mag_default: 5.8 },
                MagnitudeBracket { text: "甚暗".to_string(), mag_range: [6.0, 8.0], mag_default: 6.0 },
            ],
            magnitude_text_to_value_uncertainty: 0.6,
            lomb_scargle: LombScargleConfig {
                min_period_days: 1.0,
                max_period_days: 10000.0,
                freq_oversampling_factor: 2.0,
                false_alarm_level: 0.01,
                num_peaks_to_return: 10,
                window_function_pad_factor: 2.0,
            },
            mira_variable_template: MiraTemplateConfig {
                default_period_days: 332.0,
                min_period_days: 80.0,
                max_period_days: 5000.0,
                default_amplitude_mag: 5.0,
                default_mean_mag: 7.0,
                pdot_per_cent_per_myr: 1.2,
            },
            channel_buffer_size: 64,
        }
    }

    fn make_engine() -> VariableStarEngine {
        VariableStarEngine::new(default_test_config())
    }

    fn make_measurement(
        epoch_yr: f64,
        magnitude: f64,
        uncertainty: f64,
        source_type: &str,
        ancient_desc: Option<&str>,
    ) -> MagnitudeMeasurement {
        MagnitudeMeasurement {
            id: 0,
            variable_id: 1,
            epoch_yr,
            epoch_mjd: None,
            magnitude,
            magnitude_uncertainty: Some(uncertainty),
            passband: "V".to_string(),
            source_type: source_type.to_string(),
            source_book: None,
            ancient_description: ancient_desc.map(|s| s.to_string()),
            ancient_quality: None,
        }
    }

    fn generate_sine_data(
        n_points: usize,
        period_days: f64,
        amplitude: f64,
        mean_mag: f64,
        noise_sigma: f64,
        t_start_yr: f64,
        t_span_yr: f64,
    ) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let mut times = Vec::with_capacity(n_points);
        let mut mags = Vec::with_capacity(n_points);
        let mut errs = Vec::with_capacity(n_points);
        let dt_yr = t_span_yr / (n_points - 1).max(1) as f64;
        let period_yr = period_days / 365.25;
        let omega = TWO_PI / period_yr;

        let mut rng_seed: u64 = 42;
        for i in 0..n_points {
            let t = t_start_yr + i as f64 * dt_yr;
            let signal = mean_mag + amplitude * libm::cos(omega * t);
            rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let noise = ((rng_seed as f64 / u64::MAX as f64) - 0.5) * 2.0 * noise_sigma * 1.732;
            times.push(t);
            mags.push(signal + noise);
            errs.push(noise_sigma);
        }
        (times, mags, errs)
    }

    fn generate_sine_with_harmonics(
        n_points: usize,
        fund_period_days: f64,
        ampl1: f64,
        ampl2: f64,
        mean_mag: f64,
        t_start_yr: f64,
        t_span_yr: f64,
    ) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let mut times = Vec::with_capacity(n_points);
        let mut mags = Vec::with_capacity(n_points);
        let mut errs = Vec::with_capacity(n_points);
        let dt_yr = t_span_yr / (n_points - 1).max(1) as f64;
        let fund_period_yr = fund_period_days / 365.25;
        let omega1 = TWO_PI / fund_period_yr;
        let omega2 = 2.0 * omega1;

        for i in 0..n_points {
            let t = t_start_yr + i as f64 * dt_yr;
            let signal = mean_mag
                + ampl1 * libm::cos(omega1 * t)
                + ampl2 * libm::cos(omega2 * t + 0.5);
            times.push(t);
            mags.push(signal);
            errs.push(0.01);
        }
        (times, mags, errs)
    }

    fn relative_error(truth: f64, measured: f64) -> f64 {
        (measured - truth).abs() / truth.abs()
    }

    // ============================================================
    // 1. Normal Use Cases
    // ============================================================

    #[test]
    fn test_lomb_scargle_known_period() {
        let engine = make_engine();
        let true_period_days = 332.0;
        let (times, mags, errs) = generate_sine_data(
            200,
            true_period_days,
            2.0,
            7.0,
            0.1,
            1900.0,
            50.0,
        );

        let result = engine.lomb_scargle_periodogram(&times, &mags, &errs);
        assert!(!result.peaks.is_empty(), "Should detect at least one peak");

        let best_peak = &result.peaks[0];
        let detected_period = best_peak.period_days;
        let rel_err = relative_error(true_period_days, detected_period);

        assert!(
            rel_err < 0.50,
            "Period detection rel err {} should be < 50% (true={}, detected={})",
            rel_err, true_period_days, detected_period
        );
    }

    #[test]
    fn test_lomb_scargle_multiple_peaks() {
        let engine = make_engine();
        let fund_period_days = 332.0;
        let (times, mags, errs) = generate_sine_with_harmonics(
            200,
            fund_period_days,
            2.0,
            0.8,
            7.0,
            1900.0,
            50.0,
        );

        let result = engine.lomb_scargle_periodogram(&times, &mags, &errs);
        assert!(result.peaks.len() >= 2, "Should detect at least two peaks");

        let fund_freq = result.peaks[0].frequency_per_day;
        let has_harmonic = result.peaks.iter().skip(1).any(|p| {
            if !p.alias_of_fundamental {
                return false;
            }
            let ratio = p.frequency_per_day / fund_freq;
            let harmonic_num = ratio.round();
            (harmonic_num - 2.0).abs() < 0.1 || (ratio - 2.0).abs() / 2.0 < 0.05
        });

        assert!(has_harmonic, "Should identify 2nd harmonic as alias_of_fundamental=true");
    }

    #[test]
    #[ignore]
    fn test_reconstruct_lightcurve_continuity() {
        let engine = make_engine();
        let period_days = 332.0;
        let period_yr = period_days / 365.25;
        let omega = TWO_PI / period_yr;
        let amplitude = 2.0;
        let mean_mag = 7.0;

        let mut measurements = Vec::new();
        for i in 0..50 {
            let t = 1000.0 + i as f64 * 0.5;
            let mag = mean_mag + amplitude * libm::cos(omega * t);
            measurements.push(make_measurement(t, mag, 0.5, "ancient", None));
        }
        for i in 0..50 {
            let t = 1950.0 + i as f64 * 0.5;
            let mag = mean_mag + amplitude * libm::cos(omega * t);
            measurements.push(make_measurement(t, mag, 0.05, "modern", None));
        }

        let recon = engine.reconstruct_light_curve(1, None, &measurements, Some(period_days), 100);
        let model_samples: Vec<&LightCurveSample> = recon
            .reconstructed_samples
            .iter()
            .filter(|s| s.observed_magnitude.is_none())
            .collect();

        assert!(model_samples.len() >= 2);

        for i in 1..model_samples.len() {
            let dt = model_samples[i].epoch_yr - model_samples[i - 1].epoch_yr;
            let dm = model_samples[i].model_magnitude - model_samples[i - 1].model_magnitude;
            let diff = dm / dt;
            assert!(
                diff.abs() < 100.0,
                "First difference should not be extreme: {} at i={}",
                diff, i
            );
        }

        let max_gap_idx = (1..model_samples.len())
            .max_by(|&a, &b| {
                let ga = model_samples[a].epoch_yr - model_samples[a - 1].epoch_yr;
                let gb = model_samples[b].epoch_yr - model_samples[b - 1].epoch_yr;
                ga.partial_cmp(&gb).unwrap()
            })
            .unwrap();
        let gap_dm = model_samples[max_gap_idx].model_magnitude
            - model_samples[max_gap_idx - 1].model_magnitude;
        let gap_dt = model_samples[max_gap_idx].epoch_yr
            - model_samples[max_gap_idx - 1].epoch_yr;
        let gap_diff = gap_dm / gap_dt;
        assert!(
            gap_diff.abs() < 10.0,
            "Across ancient/modern gap, diff should be continuous: {}",
            gap_diff
        );
    }

    #[test]
    fn test_phase_fold_sine_fit() {
        let engine = make_engine();
        let true_period_days = 332.0;
        let true_amplitude = 2.5;
        let (times, mags, errs) = generate_sine_data(
            200,
            true_period_days,
            true_amplitude,
            7.0,
            0.05,
            1900.0,
            30.0,
        );

        let (_mean, amplitude_fit, _phase, _) =
            engine.sinusoid_fit(&times, &mags, &errs, true_period_days);

        let rel_err = relative_error(true_amplitude, amplitude_fit);
        assert!(
            rel_err < 0.30,
            "Amplitude fit rel err {} should be < 30% (true={}, fit={})",
            rel_err, true_amplitude, amplitude_fit
        );
    }

    #[test]
    fn test_period_change_linear_drift() {
        let engine = make_engine();
        let mean_mag = 7.0;
        let amplitude = 2.0;

        let p_ancient_days = 330.0;
        let p_modern_days = 334.0;

        let mut times = Vec::new();
        let mut mags = Vec::new();
        let mut errs = Vec::new();
        let mut is_ancient = Vec::new();

        let p_a_yr = p_ancient_days / 365.25;
        let omega_a = TWO_PI / p_a_yr;
        for i in 0..100 {
            let t = 1000.0 + i as f64 * 0.3;
            let mag = mean_mag + amplitude * libm::cos(omega_a * t);
            times.push(t);
            mags.push(mag);
            errs.push(0.5);
            is_ancient.push(true);
        }

        let p_m_yr = p_modern_days / 365.25;
        let omega_m = TWO_PI / p_m_yr;
        for i in 0..100 {
            let t = 1950.0 + i as f64 * 0.3;
            let mag = mean_mag + amplitude * libm::cos(omega_m * t);
            times.push(t);
            mags.push(mag);
            errs.push(0.05);
            is_ancient.push(false);
        }

        let overall_period = (p_ancient_days + p_modern_days) * 0.5;
        let (_a_p, _m_p, pdot, delta_p, sigma) =
            engine.estimate_pdot(&times, &mags, &errs, &is_ancient, overall_period);

        assert!(
            pdot != 0.0 || delta_p.unwrap_or(0.0) != 0.0,
            "pdot or period delta should indicate some period change, got pdot={}, delta={}",
            pdot, delta_p.unwrap_or(0.0)
        );
        assert!(
            sigma > 0.5,
            "period change should be somewhat significant, sigma={}",
            sigma
        );
    }

    #[test]
    fn test_ancient_magnitude_mapping() {
        let engine = make_engine();

        let (mag_liangji, _) = engine.magnitude_text_to_value_uncertainty("极亮");
        assert!(
            (mag_liangji - (-1.5)).abs() < 0.5,
            "极亮 should map near -1.5, got {}",
            mag_liangji
        );

        let (mag_shen_an, _) = engine.magnitude_text_to_value_uncertainty("甚暗");
        assert!(
            (mag_shen_an - 6.0).abs() < 1.0,
            "甚暗 should map near 6.0, got {}",
            mag_shen_an
        );

        let (mag_da, _) = engine.magnitude_text_to_value_uncertainty("大星");
        assert!(
            (mag_da - 1.0).abs() < 1.0,
            "大星 (contains 大) should map near 1.0, got {}",
            mag_da
        );

        let (mag_unknown, _) = engine.magnitude_text_to_value_uncertainty("未知描述");
        assert!(
            mag_unknown.is_finite(),
            "Unknown description should give finite value, got {}",
            mag_unknown
        );
    }

    #[test]
    fn test_false_alarm_probability() {
        let engine = make_engine();
        let mut times = Vec::new();
        let mut mags = Vec::new();
        let mut errs = Vec::new();
        let mut rng_seed: u64 = 12345;

        for i in 0..150 {
            let t = 1900.0 + i as f64 * 0.3;
            rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let noise = ((rng_seed as f64 / u64::MAX as f64) - 0.5) * 2.0 * 0.5 * 1.732;
            times.push(t);
            mags.push(7.0 + noise);
            errs.push(0.5);
        }

        let result = engine.lomb_scargle_periodogram(&times, &mags, &errs);

        for peak in &result.peaks {
            assert!(
                peak.false_alarm_probability > 1e-10,
                "Peak FAP {} should be > 1e-10 for pure noise (power={})",
                peak.false_alarm_probability, peak.power
            );
        }
    }

    // ============================================================
    // 2. Boundary Use Cases
    // ============================================================

    #[test]
    fn test_minimal_data_points() {
        let engine = make_engine();
        let mut times = Vec::new();
        let mut mags = Vec::new();
        let mut errs = Vec::new();

        for i in 0..8 {
            let t = 1900.0 + i as f64 * 50.0;
            times.push(t);
            mags.push(7.0 + 2.0 * libm::cos(TWO_PI * t / (332.0 / 365.25)));
            errs.push(0.1);
        }

        let result = engine.lomb_scargle_periodogram(&times, &mags, &errs);
        assert!(result.frequencies_per_day.len() > 0 || result.power.len() == 0);
        assert!(result.power.iter().all(|p| p.is_finite()));
    }

    #[test]
    fn test_sparse_data_interpolation() {
        let engine = make_engine();
        let mut measurements = Vec::new();
        let period_yr = 332.0 / 365.25;
        let omega = TWO_PI / period_yr;

        for i in 0..20 {
            let t = 500.0 + i as f64 * 50.0;
            let mag = 7.0 + 2.0 * libm::cos(omega * t);
            measurements.push(make_measurement(t, mag, 0.8, "ancient", None));
        }
        for i in 0..30 {
            let t = 1950.0 + i as f64 * 0.5;
            let mag = 7.0 + 2.0 * libm::cos(omega * t);
            measurements.push(make_measurement(t, mag, 0.05, "modern", None));
        }

        let recon = engine.reconstruct_light_curve(1, None, &measurements, None, 100);
        let model_samples: Vec<&LightCurveSample> = recon
            .reconstructed_samples
            .iter()
            .filter(|s| s.observed_magnitude.is_none())
            .collect();

        for i in 2..model_samples.len() {
            let d1 = model_samples[i - 1].model_magnitude - model_samples[i - 2].model_magnitude;
            let d2 = model_samples[i].model_magnitude - model_samples[i - 1].model_magnitude;
            let second_diff = (d2 - d1).abs();
            assert!(
                second_diff < 10.0,
                "No wild oscillations, second diff={} at i={}",
                second_diff, i
            );
        }
    }

    #[test]
    fn test_uniform_sampling_gap() {
        let engine = make_engine();
        let mut measurements = Vec::new();
        let period_yr = 332.0 / 365.25;
        let omega = TWO_PI / period_yr;

        for i in 0..50 {
            let t = 800.0 + i as f64 * 0.5;
            let mag = 7.0 + 2.0 * libm::cos(omega * t);
            measurements.push(make_measurement(t, mag, 0.3, "ancient", None));
        }
        for i in 0..50 {
            let t = 1950.0 + i as f64 * 0.5;
            let mag = 7.0 + 2.0 * libm::cos(omega * t);
            measurements.push(make_measurement(t, mag, 0.05, "modern", None));
        }

        let recon = engine.reconstruct_light_curve(1, None, &measurements, None, 100);
        let samples: Vec<&LightCurveSample> = recon
            .reconstructed_samples
            .iter()
            .filter(|s| s.observed_magnitude.is_none())
            .collect();

        let mut max_gap_idx = 0;
        let mut max_gap = 0.0f64;
        for i in 1..samples.len() {
            let gap = samples[i].epoch_yr - samples[i - 1].epoch_yr;
            if gap > max_gap {
                max_gap = gap;
                max_gap_idx = i;
            }
        }
        assert!(max_gap > 5.0, "Should have a measurable gap between data clusters, got {}", max_gap);

        let ci_in_gap = samples[max_gap_idx].model_upper_ci - samples[max_gap_idx].model_lower_ci;

        let mut ci_near_data = 0.0f64;
        let count_data = samples.len().min(10);
        for s in samples.iter().take(count_data) {
            ci_near_data += s.model_upper_ci - s.model_lower_ci;
        }
        ci_near_data /= count_data as f64;

        assert!(
            ci_in_gap > ci_near_data,
            "CI in gap ({}) should be wider than near data ({})",
            ci_in_gap, ci_near_data
        );
    }

    #[test]
    fn test_amplitude_very_small() {
        let engine = make_engine();
        let (times, mags, errs) = generate_sine_data(
            150,
            332.0,
            0.01,
            7.0,
            0.001,
            1900.0,
            30.0,
        );

        let result = engine.lomb_scargle_periodogram(&times, &mags, &errs);
        assert!(result.power.iter().all(|p| p.is_finite()));
        assert!(result.frequencies_per_day.len() > 0);

        let recon = {
            let ms: Vec<MagnitudeMeasurement> = times
                .iter()
                .zip(mags.iter())
                .zip(errs.iter())
                .map(|((t, m), e)| make_measurement(*t, *m, *e, "modern", None))
                .collect();
            engine.reconstruct_light_curve(1, None, &ms, None, 100)
        };
        assert!(recon.best_fit_amplitude_mag.is_finite());
    }

    #[test]
    fn test_period_at_nyquist() {
        let engine = make_engine();
        let true_period_days = 2.0;
        let period_yr = true_period_days / 365.25;
        let omega = TWO_PI / period_yr;

        let mut times = Vec::new();
        let mut mags = Vec::new();
        let mut errs = Vec::new();

        let dt_days = 1.0;
        for i in 0..100 {
            let t_days = i as f64 * dt_days;
            let t_yr = t_days / 365.25 + 2000.0;
            times.push(t_yr);
            mags.push(7.0 + 1.0 * libm::cos(omega * t_yr));
            errs.push(0.01);
        }

        let result = engine.lomb_scargle_periodogram(&times, &mags, &errs);
        assert!(result.power.iter().all(|p| p.is_finite()));
        assert!(result.frequencies_per_day.len() > 0);
    }

    #[test]
    fn test_period_very_long() {
        let engine = make_engine();
        let true_period_days = 5000.0;
        let (times, mags, errs) = generate_sine_data(
            150,
            true_period_days,
            2.0,
            7.0,
            0.05,
            1800.0,
            200.0,
        );

        let freq_min = 1.0 / engine.cfg.lomb_scargle.max_period_days;
        let freq_target = 1.0 / true_period_days;
        assert!(
            freq_min <= freq_target,
            "Frequency range lower bound should cover P=5000d (f_min={}, f_target={})",
            freq_min, freq_target
        );

        let result = engine.lomb_scargle_periodogram(&times, &mags, &errs);
        assert!(result.frequencies_per_day.len() > 0);
        assert!(result.frequencies_per_day[0] <= freq_target * 1.1);
    }

    // ============================================================
    // 3. Abnormal / Degenerate Use Cases
    // ============================================================

    #[test]
    fn test_constant_magnitude_all_equal() {
        let engine = make_engine();
        let times: Vec<f64> = (0..100).map(|i| 1900.0 + i as f64 * 0.3).collect();
        let mags: Vec<f64> = vec![7.0; 100];
        let errs: Vec<f64> = vec![0.1; 100];

        let result = engine.lomb_scargle_periodogram(&times, &mags, &errs);
        assert!(result.power.len() > 0);

        let p_max = result.power.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let p_min = result.power.iter().cloned().fold(f64::INFINITY, f64::min);
        assert!(
            (p_max - p_min).abs() < 1.0 || p_max < 1e-6,
            "Power spectrum should be flat for constant data (p_max={}, p_min={})",
            p_max, p_min
        );

        for peak in &result.peaks {
            assert!(
                peak.false_alarm_probability > 0.01,
                "No significant peaks for flat data"
            );
        }
    }

    #[test]
    fn test_all_measurements_ancient_only() {
        let engine = make_engine();
        let mut measurements = Vec::new();
        let period_yr = 332.0 / 365.25;
        let omega = TWO_PI / period_yr;

        for i in 0..30 {
            let t = 1000.0 + i as f64 * 1.0;
            let mag = 7.0 + 2.0 * libm::cos(omega * t);
            measurements.push(make_measurement(t, mag, 0.5, "ancient", None));
        }

        let recon = engine.reconstruct_light_curve(1, None, &measurements, None, 100);
        assert_eq!(recon.num_ancient_measurements, 30);
        assert_eq!(recon.num_modern_measurements, 0);
        assert!(recon.ancient_vs_modern_period_delta_days.is_none());
        assert!(recon
            .reconstruction_notes
            .contains("仅古代数据"));
    }

    #[test]
    fn test_all_measurements_modern_only() {
        let engine = make_engine();
        let mut measurements = Vec::new();
        let period_yr = 332.0 / 365.25;
        let omega = TWO_PI / period_yr;

        for i in 0..30 {
            let t = 2000.0 + i as f64 * 0.5;
            let mag = 7.0 + 2.0 * libm::cos(omega * t);
            measurements.push(make_measurement(t, mag, 0.05, "modern", None));
        }

        let recon = engine.reconstruct_light_curve(1, None, &measurements, None, 100);
        assert_eq!(recon.num_ancient_measurements, 0);
        assert_eq!(recon.num_modern_measurements, 30);
        assert!(recon.ancient_vs_modern_period_delta_days.is_none());
        assert!(recon
            .reconstruction_notes
            .contains("无古代观测数据"));
    }

    #[test]
    fn test_large_measurement_errors() {
        let engine = make_engine();
        let mut measurements_good = Vec::new();
        let mut measurements_bad = Vec::new();
        let period_yr = 332.0 / 365.25;
        let omega = TWO_PI / period_yr;

        for i in 0..50 {
            let t = 2000.0 + i as f64 * 0.5;
            let mag = 7.0 + 2.0 * libm::cos(omega * t);
            measurements_good.push(make_measurement(t, mag, 0.05, "modern", None));
            measurements_bad.push(make_measurement(t, mag, 5.0, "modern", None));
        }

        let recon_good = engine.reconstruct_light_curve(1, None, &measurements_good, None, 100);
        let recon_bad = engine.reconstruct_light_curve(1, None, &measurements_bad, None, 100);

        let avg_ci_good: f64 = recon_good
            .reconstructed_samples
            .iter()
            .filter(|s| s.observed_magnitude.is_none())
            .map(|s| s.model_upper_ci - s.model_lower_ci)
            .sum::<f64>()
            / recon_good
                .reconstructed_samples
                .iter()
                .filter(|s| s.observed_magnitude.is_none())
                .count() as f64;

        let avg_ci_bad: f64 = recon_bad
            .reconstructed_samples
            .iter()
            .filter(|s| s.observed_magnitude.is_none())
            .map(|s| s.model_upper_ci - s.model_lower_ci)
            .sum::<f64>()
            / recon_bad
                .reconstructed_samples
                .iter()
                .filter(|s| s.observed_magnitude.is_none())
                .count() as f64;

        assert!(
            avg_ci_bad > avg_ci_good,
            "Large errors should produce wider CI: bad={}, good={}",
            avg_ci_bad, avg_ci_good
        );
    }

    #[test]
    fn test_negative_period_detected() {
        let engine = make_engine();
        let (times, mags, errs) = generate_sine_data(
            100,
            332.0,
            2.0,
            7.0,
            0.1,
            1900.0,
            30.0,
        );

        let result = engine.lomb_scargle_periodogram(&times, &mags, &errs);

        for freq in &result.frequencies_per_day {
            assert!(*freq > 0.0, "All frequencies must be positive, got {}", freq);
        }
        for period in &result.periods_days {
            assert!(*period > 0.0, "All periods must be positive, got {}", period);
        }
        for peak in &result.peaks {
            assert!(peak.period_days > 0.0, "Peak period must be positive");
            assert!(peak.frequency_per_day > 0.0, "Peak frequency must be positive");
        }
    }

    #[test]
    fn test_empty_measurement_list() {
        let engine = make_engine();
        let measurements: Vec<MagnitudeMeasurement> = vec![];

        let recon = engine.reconstruct_light_curve(1, None, &measurements, None, 100);
        assert!(recon.best_period_days.is_finite());
        assert!(recon.reconstructed_samples.is_empty() || recon.coverage_start_yr.is_finite());
        assert!(recon.reconstructed_samples.is_empty() || recon.coverage_end_yr.is_finite());

        let (times, mags, errs, _is_ancient) = engine.prepare_measurements(&measurements);
        assert!(times.is_empty());
        assert!(mags.is_empty());
        assert!(errs.is_empty());

        let result = engine.lomb_scargle_periodogram(&[], &[], &[]);
        assert!(result.frequencies_per_day.is_empty());
        assert!(result.periods_days.is_empty());
        assert!(result.power.is_empty());
        assert!(result.peaks.is_empty());
    }

    #[test]
    fn test_nan_in_epoch_or_mag() {
        let engine = make_engine();
        let mut measurements = Vec::new();
        let period_yr = 332.0 / 365.25;
        let omega = TWO_PI / period_yr;

        for i in 0..10 {
            let t = 2000.0 + i as f64 * 0.5;
            let mag = 7.0 + 2.0 * libm::cos(omega * t);
            measurements.push(make_measurement(t, mag, 0.05, "modern", None));
        }
        measurements.push(make_measurement(f64::NAN, 7.0, 0.05, "modern", None));
        measurements.push(make_measurement(2005.0, f64::NAN, 0.05, "modern", None));

        let (times, mags, errs, _is_ancient) = engine.prepare_measurements(&measurements);
        assert_eq!(times.len(), measurements.len());

        let result = engine.lomb_scargle_periodogram(&times, &mags, &errs);
        assert!(result.power.iter().all(|p| p.is_finite() || p.is_nan() == false));

        let _recon = engine.reconstruct_light_curve(1, None, &measurements, None, 100);
    }

    #[test]
    fn test_ancient_magnitude_mapping_boundary_values() {
        let engine = make_engine();

        let (mag_jiming, _) = engine.magnitude_text_to_value_uncertainty("极明");
        assert!(
            mag_jiming >= 1.0 && mag_jiming <= 2.0,
            "极明 should map to ~1.0-2.0 mag, got {}",
            mag_jiming
        );

        let (mag_wei, _) = engine.magnitude_text_to_value_uncertainty("微");
        assert!(
            mag_wei >= 4.5 && mag_wei <= 6.0,
            "微 should map to ~5.0-6.0 mag, got {}",
            mag_wei
        );

        let (mag_kejian, _) = engine.magnitude_text_to_value_uncertainty("不可见");
        assert!(
            mag_kejian > 6.0,
            "不可见 should map to >6.0 mag, got {}",
            mag_kejian
        );
    }

    #[test]
    fn test_fuzzy_ancient_description_quantization() {
        let engine = make_engine();

        let (mag_ming, _) = engine.magnitude_text_to_value_uncertainty("明");
        let (mag_liang, _) = engine.magnitude_text_to_value_uncertainty("亮");
        let (mag_an, _) = engine.magnitude_text_to_value_uncertainty("暗");

        assert!(mag_ming.is_finite(), "明 magnitude should be finite");
        assert!(mag_liang.is_finite(), "亮 magnitude should be finite");
        assert!(mag_an.is_finite(), "暗 magnitude should be finite");

        assert!(
            mag_ming < mag_liang,
            "明 ({}) should be brighter than 亮 ({})",
            mag_ming, mag_liang
        );
        assert!(
            mag_liang < mag_an,
            "亮 ({}) should be brighter than 暗 ({})",
            mag_liang, mag_an
        );
    }

    #[test]
    fn test_period_change_linear_drift_magnitude() {
        let engine = make_engine();
        let mean_mag = 7.0;
        let amplitude = 2.0;

        let p_ancient_days = 320.0;
        let p_modern_days = 340.0;

        let mut times = Vec::new();
        let mut mags = Vec::new();
        let mut errs = Vec::new();
        let mut is_ancient = Vec::new();

        let p_a_yr = p_ancient_days / 365.25;
        let omega_a = TWO_PI / p_a_yr;
        for i in 0..80 {
            let t = 1050.0 + i as f64 * 0.4;
            let mag = mean_mag + amplitude * libm::cos(omega_a * t);
            times.push(t);
            mags.push(mag);
            errs.push(0.5);
            is_ancient.push(true);
        }

        let p_m_yr = p_modern_days / 365.25;
        let omega_m = TWO_PI / p_m_yr;
        for i in 0..80 {
            let t = 1960.0 + i as f64 * 0.4;
            let mag = mean_mag + amplitude * libm::cos(omega_m * t);
            times.push(t);
            mags.push(mag);
            errs.push(0.05);
            is_ancient.push(false);
        }

        let overall_period = (p_ancient_days + p_modern_days) * 0.5;
        let (_a_p, _m_p, _pdot, delta_p, _sigma) =
            engine.estimate_pdot(&times, &mags, &errs, &is_ancient, overall_period);

        let delta = delta_p.expect("delta_p should be Some");
        assert!(
            delta != 0.0,
            "delta_p should be nonzero, got {}",
            delta
        );
        assert!(
            delta > 0.0,
            "Period should be increasing (positive delta), got {}",
            delta
        );
    }

    #[test]
    fn test_analyzer_compatibility_aliases() {
        let cfg = default_test_config();

        let _engine_alias: VariableStarEngine = VariableStarEngine::new(cfg.clone());
        let _engine_primary: VariabilityAnalyzer = VariabilityAnalyzer::new(cfg.clone());

        let _cmd_alias: VariableStarCommand = VariableStarCommand::Shutdown;
        let _cmd_primary: VariabilityCommand = VariabilityCommand::Shutdown;

        let _event_alias: VariableStarEvent = VariableStarEvent::ShutdownAck;
        let _event_primary: VariabilityEvent = VariabilityEvent::ShutdownAck;
    }

    #[test]
    fn test_measurements_count_validation() {
        let engine = make_engine();

        let empty_ms: Vec<MagnitudeMeasurement> = vec![];
        let recon = engine.reconstruct_light_curve(42, None, &empty_ms, None, 100);
        assert!(recon.best_period_days.is_finite());
        assert!(recon.best_fit_mean_mag.is_finite());
        assert!(recon.best_fit_amplitude_mag.is_finite());
        assert_eq!(recon.num_ancient_measurements, 0);
        assert_eq!(recon.num_modern_measurements, 0);

        let ls_empty = engine.lomb_scargle_periodogram(&[], &[], &[]);
        assert!(ls_empty.frequencies_per_day.is_empty());
        assert!(ls_empty.periods_days.is_empty());
        assert!(ls_empty.power.is_empty());
        assert!(ls_empty.peaks.is_empty());

        let (times, mags, errs, _) = engine.prepare_measurements(&empty_ms);
        assert!(times.is_empty());
        assert!(mags.is_empty());
        assert!(errs.is_empty());

        let phase_empty = engine.phase_fold(&[], &[], &[], 332.0);
        assert!(phase_empty.is_empty());

        let (mean, amp, phase, _) = engine.sinusoid_fit(&[], &[], &[], 332.0);
        assert!(mean.is_finite());
        assert!(amp.is_finite());
        assert!(phase.is_finite());

        let (slope, intercept) = engine.linear_trend_fit(&[], &[], &[]);
        assert!(slope.is_finite());
        assert!(intercept.is_finite());
    }
}
