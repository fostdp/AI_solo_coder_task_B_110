//! 星表数据导入与清洗模块
//!
//! 职责:
//!   1. 接收原始星表数据 (由 handler 从 DB 拉取后传入)
//!   2. 数据清洗: 空值校验、字段标准化、质量标记
//!   3. 通过 tokio channel 将清洗后的数据发送出去
//!
//! 通道协议:
//!   输入: LoaderCommand (含原始数据)
//!   输出: LoaderEvent (含清洗后数据)

use crate::config::CatalogConfig;
use crate::models::{AncientStar, AncientComet, GuestStar};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoaderCommand {
    CleanStars { stars: Vec<AncientStar> },
    CleanComets { comets: Vec<AncientComet> },
    CleanGuestStars { guests: Vec<GuestStar> },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoaderEvent {
    StarsCleaned { count: usize, records: Vec<CleanedStarRecord> },
    CometsCleaned { count: usize, records: Vec<AncientComet> },
    GuestStarsCleaned { count: usize, records: Vec<GuestStar> },
    Error { message: String },
    ShutdownAck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanedStarRecord {
    pub id: i64,
    pub star_id_code: String,
    pub dynasty_id: i64,
    pub star_name_cn: String,
    pub ruxiu_du: Option<f64>,
    pub quji_du: Option<f64>,
    pub ra_j2000: Option<f64>,
    pub dec_j2000: Option<f64>,
    pub magnitude_num: Option<f64>,
    pub color_temp_k: Option<f64>,
    pub proper_motion_ra: Option<f64>,
    pub proper_motion_dec: Option<f64>,
    pub source_book: Option<String>,
    pub quality_flag: i32,
    pub cleaning_flags: u32,
}

pub const FLAG_RUXIU_OUTLIER: u32 = 1 << 0;
pub const FLAG_QUJI_OUTLIER: u32 = 1 << 1;
pub const FLAG_MAG_OUTLIER: u32 = 1 << 2;
pub const FLAG_COLOR_UNKNOWN: u32 = 1 << 3;
pub const FLAG_PM_MISSING: u32 = 1 << 4;

pub struct CatalogLoader {
    config: CatalogConfig,
    cmd_rx: mpsc::Receiver<LoaderCommand>,
    event_tx: mpsc::Sender<LoaderEvent>,
}

impl CatalogLoader {
    pub fn new(
        config: CatalogConfig,
    ) -> (Self, mpsc::Sender<LoaderCommand>, mpsc::Receiver<LoaderEvent>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        let (event_tx, event_rx) = mpsc::channel(32);
        (
            Self { config, cmd_rx, event_tx },
            cmd_tx,
            event_rx,
        )
    }

    pub async fn run(mut self) {
        info!("CatalogLoader started");
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                LoaderCommand::CleanStars { stars } => {
                    let cleaned: Vec<CleanedStarRecord> = stars.iter()
                        .map(|s| self.clean_star(s))
                        .collect();
                    let n = cleaned.len();
                    info!("CatalogLoader: cleaned {} stars", n);
                    let _ = self.event_tx.send(LoaderEvent::StarsCleaned {
                        count: n, records: cleaned,
                    }).await;
                }
                LoaderCommand::CleanComets { comets } => {
                    let _ = self.event_tx.send(LoaderEvent::CometsCleaned {
                        count: comets.len(), records: comets,
                    }).await;
                }
                LoaderCommand::CleanGuestStars { guests } => {
                    let _ = self.event_tx.send(LoaderEvent::GuestStarsCleaned {
                        count: guests.len(), records: guests,
                    }).await;
                }
                LoaderCommand::Shutdown => {
                    info!("CatalogLoader shutting down");
                    let _ = self.event_tx.send(LoaderEvent::ShutdownAck).await;
                    break;
                }
            }
        }
    }

    fn clean_star(&self, s: &AncientStar) -> CleanedStarRecord {
        let mut flags = 0u32;
        let rules = &self.config.cleaning_rules;

        if let Some(r) = s.ruxiu_du {
            if r < 0.0 || r > rules.max_ruxiu_du { flags |= FLAG_RUXIU_OUTLIER; }
        }
        if let Some(q) = s.quji_du {
            if q < 0.0 || q > rules.max_quji_du { flags |= FLAG_QUJI_OUTLIER; }
        }
        if let Some(m) = s.magnitude_num {
            if m < rules.min_magnitude || m > rules.max_magnitude { flags |= FLAG_MAG_OUTLIER; }
        }
        if let Some(ref c) = s.color_desc {
            if !rules.valid_color_descriptions.contains(c) { flags |= FLAG_COLOR_UNKNOWN; }
        } else {
            flags |= FLAG_COLOR_UNKNOWN;
        }
        if s.proper_motion_ra.is_none() || s.proper_motion_dec.is_none() {
            flags |= FLAG_PM_MISSING;
        }

        let color_temp = s.color_temp_k.or(Some(rules.default_color_temp_k));

        CleanedStarRecord {
            id: s.id,
            star_id_code: s.star_id_code.clone(),
            dynasty_id: s.dynasty_id,
            star_name_cn: s.star_name_cn.clone(),
            ruxiu_du: s.ruxiu_du,
            quji_du: s.quji_du,
            ra_j2000: s.ra_j2000,
            dec_j2000: s.dec_j2000,
            magnitude_num: s.magnitude_num,
            color_temp_k: color_temp,
            proper_motion_ra: s.proper_motion_ra,
            proper_motion_dec: s.proper_motion_dec,
            source_book: s.source_book.clone(),
            quality_flag: s.quality_flag,
            cleaning_flags: flags,
        }
    }
}

pub fn spawn_loader(
    config: CatalogConfig,
) -> (mpsc::Sender<LoaderCommand>, mpsc::Receiver<LoaderEvent>) {
    let (loader, cmd_tx, event_rx) = CatalogLoader::new(config);
    tokio::spawn(async move { loader.run().await });
    (cmd_tx, event_rx)
}
