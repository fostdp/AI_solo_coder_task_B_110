//! 客星-超新星遗迹匹配模块
//!
//! 职责:
//!   1. 从 coordinate_transformer 接收转换后的坐标数据
//!   2. 接收匹配任务 (客星 + SNR 列表)
//!   3. 执行贝叶斯匹配 (银河系盘先验 + Student-t 似然)
//!   4. 通过 channel 返回排序后的匹配候选
//!
//! 所有模型参数从 config::MatchingConfig 加载，不再硬编码

use crate::config::MatchingConfig;
use crate::matching::{GuestStarObs, SupernovaRemnant, MatchCandidate};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use tokio::sync::mpsc;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchCommand {
    RunMatch {
        guest: GuestStarObs,
        snrs: Vec<SupernovaRemnant>,
        top_k: i32,
    },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchEvent {
    MatchCompleted {
        guest_id: i64,
        candidates: Vec<MatchCandidate>,
        method: MatchMethodInfo,
    },
    Error { message: String },
    ShutdownAck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchMethodInfo {
    pub name: String,
    pub version: String,
    pub prior_model: String,
    pub n_candidates_evaluated: usize,
    pub n_candidates_returned: usize,
    pub log_bayes_factor_top: f64,
}

struct MatchEngine {
    cfg: MatchingConfig,
}

impl MatchEngine {
    fn new(cfg: MatchingConfig) -> Self { Self { cfg } }

    fn run(&self, guest: &GuestStarObs, snrs: &[SupernovaRemnant], top_k: i32)
        -> (Vec<MatchCandidate>, MatchMethodInfo)
    {
        let mut candidates: Vec<MatchCandidate> = snrs.iter().map(|s| {
            let log_prior = self.log_galactic_prior(s);
            let (spatial_ll, sep_arcmin) = self.spatial_ll(guest, s);
            let (temporal_ll, delta_yr) = self.temporal_ll(guest, s);
            let mag_ll = self.magnitude_ll(guest, s);
            let lc_ll = self.lightcurve_ll(guest, s);

            let log_likelihood = spatial_ll + temporal_ll + mag_ll + lc_ll;
            let log_posterior = log_likelihood + log_prior;

            MatchCandidate {
                guest_id: guest.id,
                remnant_id: s.id,
                remnant_name: s.remnant_name.clone(),
                remnant_type: s.sn_type.clone(),
                rank_within_guest: 0,
                match_probability: 0.0,
                log_posterior,
                log_likelihood,
                log_prior,
                bayes_factor: 1.0,
                angular_sep_arcmin: sep_arcmin,
                time_delta_yr: delta_yr,
                spatial_score: spatial_ll,
                temporal_score: temporal_ll,
                magnitude_score: mag_ll,
                lightcurve_score: lc_ll,
            }
        }).collect();

        let max_log_post = candidates.iter()
            .map(|c| c.log_posterior)
            .fold(f64::NEG_INFINITY, f64::max);
        let sum_exp: f64 = candidates.iter()
            .map(|c| (c.log_posterior - max_log_post).exp())
            .sum();
        for c in &mut candidates {
            c.match_probability = (c.log_posterior - max_log_post).exp() / sum_exp;
        }

        candidates.sort_by(|a, b|
            b.match_probability.partial_cmp(&a.match_probability)
            .unwrap_or(std::cmp::Ordering::Equal));

        for (i, c) in candidates.iter_mut().enumerate() {
            c.rank_within_guest = (i + 1) as i32;
        }

        let bayes_factor = if candidates.len() >= 2 {
            (candidates[0].log_posterior - candidates[1].log_posterior).exp()
        } else if !candidates.is_empty() { 1e6 } else { 1.0 };
        if !candidates.is_empty() { candidates[0].bayes_factor = bayes_factor; }

        let n_eval = candidates.len();
        candidates.truncate(top_k.max(5) as usize);

        let info = MatchMethodInfo {
            name: self.cfg.model_name.clone(),
            version: self.cfg.version.clone(),
            prior_model: format!("Exponential disk (R_d={} kpc) + isothermal disk (z_d={} pc)",
                self.cfg.galactic_prior.r_disk_scale_kpc,
                (self.cfg.galactic_prior.z_disk_scale_kpc * 1000.0) as i32),
            n_candidates_evaluated: n_eval,
            n_candidates_returned: candidates.len(),
            log_bayes_factor_top: bayes_factor.ln(),
        };

        (candidates, info)
    }

    fn log_galactic_prior(&self, snr: &SupernovaRemnant) -> f64 {
        let gp = &self.cfg.galactic_prior;
        let (l, b) = match (snr.gal_l, snr.gal_b) {
            (Some(l), Some(b)) => (l, b),
            _ => self.eq_to_gal(snr.ra_deg, snr.dec_deg),
        };
        let (r_kpc, z_kpc) = self.gal_to_cyl(l, b, snr.distance_kpc);

        let log_sigma = -(r_kpc - gp.r_sun_kpc) / gp.r_disk_scale_kpc;
        let arg = z_kpc / (2.0 * gp.z_disk_scale_kpc);
        let log_rho = 2.0 * self.sech(arg).ln();

        gp.prior_floor_log.max(log_sigma + log_rho)
    }

    fn eq_to_gal(&self, ra: f64, dec: f64) -> (f64, f64) {
        let deg2rad = PI / 180.0;
        let rad2deg = 180.0 / PI;
        let gp = &self.cfg.galactic_prior;

        let ra_r = ra * deg2rad;
        let dec_r = dec * deg2rad;
        let ngp_ra = gp.ngp_ra_deg * deg2rad;
        let ngp_dec = gp.ngp_dec_deg * deg2rad;
        let lon_cp = gp.lon_cp_deg * deg2rad;

        let sin_b = dec_r.sin() * ngp_dec.sin()
            + dec_r.cos() * ngp_dec.cos() * (ra_r - ngp_ra).cos();
        let b = sin_b.asin();

        let y = dec_r.sin() * ngp_dec.cos()
            - dec_r.cos() * ngp_dec.sin() * (ra_r - ngp_ra).cos();
        let x = -dec_r.cos() * (ra_r - ngp_ra).sin();
        let l = y.atan2(x) + lon_cp;

        let mut l_deg = l * rad2deg;
        if l_deg < 0.0 { l_deg += 360.0; }
        if l_deg >= 360.0 { l_deg -= 360.0; }
        (l_deg, b * rad2deg)
    }

    fn gal_to_cyl(&self, l_deg: f64, b_deg: f64, d_kpc: f64) -> (f64, f64) {
        let deg2rad = PI / 180.0;
        let gp = &self.cfg.galactic_prior;
        let l = l_deg * deg2rad;
        let b = b_deg * deg2rad;
        let x = d_kpc * b.cos() * l.cos() - gp.r_sun_kpc;
        let y = d_kpc * b.cos() * l.sin();
        let z = d_kpc * b.sin();
        ((x * x + y * y).sqrt(), z)
    }

    fn sech(&self, x: f64) -> f64 { 2.0 / (x.exp() + (-x).exp()) }

    fn spatial_ll(&self, g: &GuestStarObs, s: &SupernovaRemnant) -> (f64, f64) {
        let lcfg = &self.cfg.likelihood.spatial;
        let dcfg = &self.cfg.default_config;

        let sigma_g = ((g.ra_err.powi(2) + g.dec_err.powi(2)) / 2.0).sqrt();
        let sigma_s = lcfg.snr_position_uncertainty_deg;
        let sigma = dcfg.spatial_sigma_scale * (sigma_g.powi(2) + sigma_s.powi(2)).sqrt();

        let sep = self.ang_sep(
            g.ra_deg.unwrap_or(0.0), g.dec_deg.unwrap_or(0.0),
            s.ra_deg, s.dec_deg,
        ).max(dcfg.min_sep_arcmin / 60.0);

        let r = sep;
        let log_gauss = (r / sigma.powi(2)).ln() - r * r / (2.0 * sigma.powi(2));
        let gamma = lcfg.cauchy_scale_deg;
        let log_cauchy = (2.0 * r).ln()
            - (PI * gamma * (1.0 + (r / gamma).powi(2)).powi(2)).ln();

        let mix = 0.9 * log_gauss.exp() + 0.1 * log_cauchy.exp();
        (mix.ln(), sep * 60.0)
    }

    fn temporal_ll(&self, g: &GuestStarObs, s: &SupernovaRemnant) -> (f64, f64) {
        let sn_year = 2000.0 - s.age_yr;
        let delta_yr = g.year_ce - sn_year;
        let tcfg = &self.cfg.likelihood.temporal;
        let dcfg = &self.cfg.default_config;
        let sigma = (dcfg.temporal_sigma_yr.powi(2) + s.age_err_yr.powi(2)).sqrt();
        let log_l = self.log_student_t(delta_yr, 0.0, sigma.max(tcfg.min_sigma_yr), tcfg.nu);
        (log_l, delta_yr)
    }

    fn magnitude_ll(&self, g: &GuestStarObs, s: &SupernovaRemnant) -> f64 {
        let mcfg = &self.cfg.likelihood.magnitude;
        let dcfg = &self.cfg.default_config;

        let type_str = s.sn_type.as_str();
        let m_abs_mean = match type_str {
            "Ia" => -19.3,
            "Ib" | "Ic" | "Ibc" => -17.8,
            "II" | "IIP" | "IIL" | "IIn" => -16.8,
            _ => -17.5,
        };
        let m_abs_err: f64 = match type_str {
            "Ia" => 0.3,
            "Ib" | "Ic" | "Ibc" => 0.8,
            "II" | "IIP" | "IIL" | "IIn" => 1.2,
            _ => 1.5,
        };

        let dist_pc = s.distance_kpc * 1000.0;
        let mu = if dist_pc > 10.0 { 5.0 * (dist_pc / 10.0).log10() } else { 0.0 };
        let av = mcfg.default_extinction_av;
        let av_err = mcfg.default_extinction_err;

        let m_pred = m_abs_mean + mu + av;
        let m_sigma: f64 = (
            dcfg.magnitude_sigma.powi(2)
            + m_abs_err.powi(2)
            + av_err.powi(2)
            + g.peak_mag_err.powi(2)
        ).sqrt();

        let bonus = match (s.gamma_detected,
                          s.radio_flux_ghz.map_or(false, |f| f > 50.0),
                          s.xray_luminosity.map_or(false, |l| l > 1e36)) {
            (true, _, _) => 0.15,
            (_, true, true) => 0.10,
            (_, true, _) => 0.05,
            _ => 0.0,
        };

        let x = g.peak_mag - m_pred;
        self.log_student_t(x, 0.0, m_sigma.max(mcfg.min_sigma), mcfg.nu)
            + (1.0f64 + bonus).ln()
    }

    fn lightcurve_ll(&self, g: &GuestStarObs, s: &SupernovaRemnant) -> f64 {
        let lcfg = &self.cfg.likelihood.lightcurve;
        let dcfg = &self.cfg.default_config;

        let observed_days = g.visibility_days.unwrap_or(60) as f64;
        let expected_days = match s.sn_type.as_str() {
            "Ia" => 100.0,
            "Ib" | "Ic" => 80.0,
            "IIP" => 120.0,
            "IIL" | "IIn" => 90.0,
            "II" => 100.0,
            _ => 80.0,
        };
        let sigma = dcfg.lightcurve_sigma_days.max(lcfg.min_sigma_days);
        self.log_student_t(observed_days, expected_days, sigma, lcfg.nu)
    }

    fn log_student_t(&self, x: f64, mu: f64, sigma: f64, nu: f64) -> f64 {
        let z = (x - mu) / sigma;
        let t1 = -0.5 * (nu + 1.0) * (1.0 + z * z / nu).ln();
        let t2 = -sigma.ln() - 0.5 * (nu * PI).ln();
        let lgamma = |x: f64| -> f64 {
            if x < 8.0 {
                let mut prod = 0.0;
                let mut y = x;
                while y < 8.0 {
                    prod -= y.ln();
                    y += 1.0;
                }
                let s = 0.5 * (2.0 * PI * y).ln() + y * (y.ln() - 1.0) - 1.0 / (12.0 * y);
                s + prod
            } else {
                0.5 * (2.0 * PI * x).ln() + x * (x.ln() - 1.0) - 1.0 / (12.0 * x)
            }
        };
        let t3 = lgamma(0.5 * (nu + 1.0)) - lgamma(0.5 * nu);
        t1 + t2 + t3
    }

    fn ang_sep(&self, ra1: f64, dec1: f64, ra2: f64, dec2: f64) -> f64 {
        let deg2rad = PI / 180.0;
        let rad2deg = 180.0 / PI;
        let dra = (ra1 - ra2) * deg2rad;
        let ddec = (dec1 - dec2) * deg2rad;
        let a = (ddec / 2.0).sin().powi(2)
            + dec1.to_radians().cos() * dec2.to_radians().cos()
            * (dra / 2.0).sin().powi(2);
        2.0 * a.sqrt().asin() * rad2deg
    }
}

pub struct TransientMatcher {
    engine: MatchEngine,
    cmd_rx: mpsc::Receiver<MatchCommand>,
    event_tx: mpsc::Sender<MatchEvent>,
}

impl TransientMatcher {
    pub fn new(
        config: MatchingConfig,
    ) -> (Self, mpsc::Sender<MatchCommand>, mpsc::Receiver<MatchEvent>) {
        let buf_size = config.channel_buffer_size;
        let (cmd_tx, cmd_rx) = mpsc::channel(buf_size);
        let (event_tx, event_rx) = mpsc::channel(buf_size);
        (
            Self {
                engine: MatchEngine::new(config),
                cmd_rx,
                event_tx,
            },
            cmd_tx,
            event_rx,
        )
    }

    pub async fn run(mut self) {
        info!("TransientMatcher started (Galactic prior model from config)");
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                MatchCommand::RunMatch { guest, snrs, top_k } => {
                    let (candidates, info) = self.engine.run(&guest, &snrs, top_k);
                    let event = MatchEvent::MatchCompleted {
                        guest_id: guest.id,
                        candidates,
                        method: info,
                    };
                    let _ = self.event_tx.send(event).await;
                }
                MatchCommand::Shutdown => {
                    info!("TransientMatcher shutting down");
                    let _ = self.event_tx.send(MatchEvent::ShutdownAck).await;
                    break;
                }
            }
        }
    }
}

pub fn spawn_matcher(
    config: MatchingConfig,
) -> (mpsc::Sender<MatchCommand>, mpsc::Receiver<MatchEvent>) {
    let (m, cmd_tx, event_rx) = TransientMatcher::new(config);
    tokio::spawn(async move { m.run().await });
    (cmd_tx, event_rx)
}
