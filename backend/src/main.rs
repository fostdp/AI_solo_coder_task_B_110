#![allow(dead_code)]
//! 古代星表数据数字化与现代天体物理验证系统
//! Rust 后端 (Actix-Web) v0.3
//!
//! 架构重构 v0.3:
//!   - 拆分为多个独立模块，通过 tokio channel 通信
//!   - 模型参数全部从 config/ JSON 文件加载
//!
//! 模块职责:
//!   catalog_loader          星表数据导入 + 清洗 (DB → Channel)
//!   coordinate_transformer  岁差/章动/自行 + 误差估计
//!   transient_matcher       客星-超新星贝叶斯匹配
//!   eclipse                 日月食计算与古代记录验证
//!   instrument              古代仪器误差反演
//!   variable_star           变星亮度演化分析
//!   horoscope               公众科普交互 / 个人星图
//!
//! main.rs 职责:
//!   - 加载配置
//!   - 启动子模块任务
//!   - 作为 REST API 层 + 模块协调器

mod config;
mod telemetry;
mod catalog_loader;
mod coordinate_transformer;
mod transient_matcher;
mod astronomy;
mod matching;
mod models;
mod db;
mod variable_star;
mod eclipse;
mod instrument;
mod horoscope;

use actix_web::{web, App, HttpServer, HttpResponse, get, post, Responder};
use actix_files::Files;
use actix_cors::Cors;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use tracing::{info, error};

use config::AppConfig;
use db::DbPool;
use models::*;
use astronomy::{RuxiuToJ2000Request, TrajectoryRequest};
use catalog_loader::{LoaderCommand, LoaderEvent};
use coordinate_transformer::{TransformCommand, TransformEvent, TransformResult};
use transient_matcher::{MatchCommand, MatchEvent, MatchMethodInfo};
use eclipse::{EclipseCommand, EclipseEvent};
use instrument::{InstrumentCommand, InstrumentEvent};
use variable_star::{VariableStarCommand, VariableStarEvent};
use horoscope::{HoroscopeCommand, HoroscopeEvent};
use telemetry::MetricsRegistry;

struct AppState {
    pool: DbPool,
    config: AppConfig,
    loader_tx: tokio::sync::mpsc::Sender<LoaderCommand>,
    loader_rx: Arc<Mutex<tokio::sync::mpsc::Receiver<LoaderEvent>>>,
    transform_tx: tokio::sync::mpsc::Sender<TransformCommand>,
    transform_rx: Arc<Mutex<tokio::sync::mpsc::Receiver<TransformEvent>>>,
    match_tx: tokio::sync::mpsc::Sender<MatchCommand>,
    match_rx: Arc<Mutex<tokio::sync::mpsc::Receiver<MatchEvent>>>,
    eclipse_tx: tokio::sync::mpsc::Sender<EclipseCommand>,
    eclipse_rx: Arc<Mutex<tokio::sync::mpsc::Receiver<EclipseEvent>>>,
    instrument_tx: tokio::sync::mpsc::Sender<InstrumentCommand>,
    instrument_rx: Arc<Mutex<tokio::sync::mpsc::Receiver<InstrumentEvent>>>,
    variable_tx: tokio::sync::mpsc::Sender<VariableStarCommand>,
    variable_rx: Arc<Mutex<tokio::sync::mpsc::Receiver<VariableStarEvent>>>,
    horoscope_tx: tokio::sync::mpsc::Sender<HoroscopeCommand>,
    horoscope_rx: Arc<Mutex<tokio::sync::mpsc::Receiver<HoroscopeEvent>>>,
    metrics: Arc<MetricsRegistry>,
}

const CHANNEL_TIMEOUT_MS: u64 = 30000;

// ============================================================
// 健康检查
// ============================================================

#[get("/health")]
async fn api_health(data: web::Data<Arc<AppState>>) -> impl Responder {
    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "models": ["IAU 2006 precession", "Galactic prior Bayes", "Planck color temp",
                   "Saros eclipse cycle", "Instrument LSQ inversion", "Lomb-Scargle periodogram",
                   "Personal starmap projection"],
        "modules": {
            "precession": data.config.precession.model_name.clone(),
            "matching": data.config.matching.model_name.clone(),
            "catalog": data.config.catalog.model_name.clone(),
            "eclipse": data.config.eclipse.model_name.clone(),
            "instrument": data.config.instrument.model_name.clone(),
            "variable": data.config.variable.model_name.clone(),
            "horoscope": data.config.horoscope.model_name.clone(),
        },
        "architecture": "7-modules + channels (catalog_loader → coordinate_transformer → transient_matcher → eclipse/instrument/variable_star/horoscope)",
    })))
}

#[get("/metrics")]
async fn api_metrics(data: web::Data<Arc<AppState>>) -> impl Responder {
    match data.metrics.encode_text() {
        Ok(body) => HttpResponse::Ok()
            .content_type("text/plain; version=0.0.4; charset=utf-8")
            .body(body),
        Err(e) => {
            error!("Failed to encode metrics: {}", e);
            HttpResponse::InternalServerError().body(e)
        }
    }
}

// ============================================================
// 朝代 / 星宿
// ============================================================

#[get("/dynasties")]
async fn api_dynasties(data: web::Data<Arc<AppState>>) -> impl Responder {
    match db::list_dynasties(&data.pool).await {
        Ok(list) => HttpResponse::Ok().json(ApiResponse::ok(list)),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

#[get("/mansions")]
async fn api_mansions(data: web::Data<Arc<AppState>>) -> impl Responder {
    match db::list_mansions(&data.pool).await {
        Ok(list) => HttpResponse::Ok().json(ApiResponse::ok(list)),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

// ============================================================
// 恒星 CRUD + 查询 (通过 catalog_loader 模块)
// ============================================================

#[get("/stars")]
async fn api_query_stars(
    data: web::Data<Arc<AppState>>,
    query: web::Query<StarQueryParams>,
) -> impl Responder {
    let params: StarQueryParams = query.into_inner();

    match db::query_stars(&data.pool, &params).await {
        Ok((list, total)) => {
            let mut rx = data.loader_rx.lock().await;
            if data.loader_tx.send(LoaderCommand::CleanStars {
                stars: list.clone()
            }).await.is_err() {
                return HttpResponse::InternalServerError().json(
                    ApiResponse::<()>::err("Loader channel send failed"));
            }
            match timeout(Duration::from_millis(CHANNEL_TIMEOUT_MS), rx.recv()).await {
                Ok(Some(LoaderEvent::StarsCleaned { records, .. })) => {
                    let response = serde_json::json!({
                        "raw": list,
                        "cleaned": records,
                    });
                    HttpResponse::Ok().json(ApiResponse::ok_with_count(response, total))
                }
                Ok(Some(LoaderEvent::Error { message })) => {
                    HttpResponse::InternalServerError().json(
                        ApiResponse::<()>::err(format!("Loader: {}", message)))
                }
                _ => HttpResponse::Ok().json(ApiResponse::ok_with_count(list, total)),
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

#[get("/stars/{id}")]
async fn api_get_star(
    data: web::Data<Arc<AppState>>,
    id: web::Path<i64>,
) -> impl Responder {
    match db::get_star(&data.pool, id.into_inner()).await {
        Ok(Some(s)) => HttpResponse::Ok().json(ApiResponse::ok(s)),
        Ok(None) => HttpResponse::NotFound().json(ApiResponse::<()>::err("Star not found")),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

#[get("/stars/{id}/cross-dynasty")]
async fn api_cross_dynasty(
    data: web::Data<Arc<AppState>>,
    id: web::Path<i64>,
) -> impl Responder {
    let star_id = id.into_inner();
    let star_opt = db::get_star(&data.pool, star_id).await.ok().flatten();
    let name = star_opt.as_ref().map(|s| s.star_name_cn.clone());
    match db::get_star_cross_dynasty(&data.pool, Some(star_id), name).await {
        Ok(list) => {
            let total = list.len() as i64;
            HttpResponse::Ok().json(ApiResponse::ok_with_count(list, total))
        }
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

// ============================================================
// 坐标转换 API (通过 coordinate_transformer 模块)
// ============================================================

#[post("/convert/ruxiu-to-j2000")]
async fn api_convert_ruxiu(
    data: web::Data<Arc<AppState>>,
    body: web::Json<RuxiuToJ2000Request>,
) -> impl Responder {
    let req = body.into_inner();
    let cmd = TransformCommand::ConvertSingle {
        ruxiu_du: req.ruxiu_du,
        quji_du: req.quji_du,
        mansion_order: req.mansion_order,
        epoch_yr: req.epoch_yr,
        pm_ra_mas: req.pm_ra_mas,
        pm_dec_mas: req.pm_dec_mas,
    };

    let mut rx = data.transform_rx.lock().await;
    if data.transform_tx.send(cmd).await.is_err() {
        return HttpResponse::InternalServerError().json(
            ApiResponse::<()>::err("Transform channel send failed"));
    }

    match timeout(Duration::from_millis(CHANNEL_TIMEOUT_MS), rx.recv()).await {
        Ok(Some(TransformEvent::SingleConverted(result))) => {
            let resp = build_convert_response(&result);
            HttpResponse::Ok().json(ApiResponse::ok(resp))
        }
        Ok(Some(TransformEvent::Error { message })) => {
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::err(format!("Transform: {}", message)))
        }
        Ok(None) => {
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::err("Transform channel closed"))
        }
        Err(_) => {
            HttpResponse::RequestTimeout().json(
                ApiResponse::<()>::err("Transform timeout"))
        }
        _ => HttpResponse::InternalServerError().json(
            ApiResponse::<()>::err("Unexpected transform event")),
    }
}

#[post("/trajectory")]
async fn api_trajectory(
    data: web::Data<Arc<AppState>>,
    body: web::Json<TrajectoryRequest>,
) -> impl Responder {
    let req = body.into_inner();
    let cmd = TransformCommand::ComputeTrajectory {
        ra_j2000: req.ra_j2000,
        dec_j2000: req.dec_j2000,
        pm_ra_mas: req.pm_ra_mas,
        pm_dec_mas: req.pm_dec_mas,
        year_start: req.year_start,
        year_end: req.year_end,
        n_points: req.n_points,
    };

    let mut rx = data.transform_rx.lock().await;
    if data.transform_tx.send(cmd).await.is_err() {
        return HttpResponse::InternalServerError().json(
            ApiResponse::<()>::err("Transform channel send failed"));
    }

    match timeout(Duration::from_millis(CHANNEL_TIMEOUT_MS), rx.recv()).await {
        Ok(Some(TransformEvent::TrajectoryComputed { points })) => {
            HttpResponse::Ok().json(ApiResponse::ok(points))
        }
        Ok(Some(TransformEvent::Error { message })) => {
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::err(format!("Transform: {}", message)))
        }
        Err(_) => HttpResponse::RequestTimeout().json(
            ApiResponse::<()>::err("Transform timeout")),
        _ => HttpResponse::InternalServerError().json(
            ApiResponse::<()>::err("Unexpected transform event")),
    }
}

// ============================================================
// 彗星 / 客星 / SNR
// ============================================================

#[get("/comets")]
async fn api_comets(data: web::Data<Arc<AppState>>) -> impl Responder {
    match db::list_comets(&data.pool).await {
        Ok(list) => HttpResponse::Ok().json(ApiResponse::ok(list)),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

#[get("/guest-stars")]
async fn api_guest_stars(data: web::Data<Arc<AppState>>) -> impl Responder {
    match db::list_guest_stars(&data.pool).await {
        Ok(list) => HttpResponse::Ok().json(ApiResponse::ok(list)),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

#[get("/guest-stars/{id}")]
async fn api_get_guest(
    data: web::Data<Arc<AppState>>,
    id: web::Path<i64>,
) -> impl Responder {
    match db::get_guest_star(&data.pool, id.into_inner()).await {
        Ok(Some(g)) => HttpResponse::Ok().json(ApiResponse::ok(g)),
        Ok(None) => HttpResponse::NotFound().json(ApiResponse::<()>::err("Not found")),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

#[get("/snr")]
async fn api_snr(data: web::Data<Arc<AppState>>) -> impl Responder {
    match db::list_snr(&data.pool).await {
        Ok(list) => HttpResponse::Ok().json(ApiResponse::ok(list)),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

// ============================================================
// 贝叶斯匹配 API (通过 transient_matcher 模块)
// ============================================================

#[get("/match/{guest_id}")]
async fn api_get_matches(
    data: web::Data<Arc<AppState>>,
    guest_id: web::Path<i64>,
) -> impl Responder {
    let gid = guest_id.into_inner();
    match db::get_match_results(&data.pool, gid).await {
        Ok(list) if !list.is_empty() => {
            HttpResponse::Ok().json(ApiResponse::ok(list))
        }
        _ => {
            match run_match_via_matcher(&data, gid, 20).await {
                Ok((candidates, _)) => HttpResponse::Ok().json(ApiResponse::ok(candidates)),
                Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
            }
        }
    }
}

#[post("/match/{guest_id}")]
async fn api_run_match(
    data: web::Data<Arc<AppState>>,
    guest_id: web::Path<i64>,
    query: web::Query<MatchRequest>,
) -> impl Responder {
    let gid = guest_id.into_inner();
    let top_k = query.top_k.unwrap_or(10);
    match run_match_via_matcher(&data, gid, top_k).await {
        Ok((candidates, method)) => {
            let guest = db::get_guest_for_match(&data.pool, gid).await.ok().flatten();
            Ok(serde_json::json!({
                "guest": guest,
                "candidates": candidates,
                "method": method,
            }))
        }
        Err(e) => Err(e),
    }.map_or_else(
        |e| HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
        |v| HttpResponse::Ok().json(ApiResponse::ok(v))
    )
}

async fn run_match_via_matcher(
    data: &web::Data<Arc<AppState>>,
    guest_id: i64,
    top_k: i32,
) -> Result<(Vec<matching::MatchCandidate>, MatchMethodInfo), String> {
    let guest = db::get_guest_for_match(&data.pool, guest_id).await?
        .ok_or_else(|| "Guest star not found".to_string())?;
    let snrs = db::list_snr_for_match(&data.pool).await?;

    let cmd = MatchCommand::RunMatch {
        guest: guest.clone(),
        snrs: snrs.clone(),
        top_k,
    };

    let mut rx = data.match_rx.lock().await;
    data.match_tx.send(cmd).await
        .map_err(|_| "Matcher channel send failed".to_string())?;

    match timeout(Duration::from_millis(CHANNEL_TIMEOUT_MS), rx.recv()).await {
        Ok(Some(MatchEvent::MatchCompleted { candidates, method, .. })) => {
            let ver = env!("CARGO_PKG_VERSION");
            let top20 = candidates.iter().take(20).cloned().collect::<Vec<_>>();
            db::save_match_result(&data.pool, guest_id, &top20, ver).await.ok();
            Ok((candidates, method))
        }
        Ok(Some(MatchEvent::Error { message })) => {
            Err(format!("Matcher: {}", message))
        }
        Ok(None) => Err("Matcher channel closed".to_string()),
        Err(_) => Err("Matcher timeout".to_string()),
        _ => Err("Unexpected matcher event".to_string()),
    }
}

// ============================================================
// 日月食 API (通过 eclipse 模块)
// ============================================================

#[get("/eclipses")]
async fn api_list_eclipse_records(
    data: web::Data<Arc<AppState>>,
    query: web::Query<EclipseRequest>,
) -> impl Responder {
    let params = query.into_inner();
    match db::list_eclipse_records(
        &data.pool,
        params.dynasty_id,
        params.eclipse_type.as_deref(),
        params.year_ce_min,
        params.year_ce_max,
        100i64,
        0i64,
    ).await {
        Ok((list, total)) => {
            HttpResponse::Ok().json(ApiResponse::ok_with_count(list, total))
        }
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

#[get("/eclipses/{id}")]
async fn api_get_eclipse_record(
    data: web::Data<Arc<AppState>>,
    id: web::Path<i64>,
) -> impl Responder {
    match db::get_eclipse_record(&data.pool, id.into_inner()).await {
        Ok(Some(r)) => HttpResponse::Ok().json(ApiResponse::ok(r)),
        Ok(None) => HttpResponse::NotFound().json(ApiResponse::<()>::err("Eclipse record not found")),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

#[post("/eclipses/{id}/compute")]
async fn api_compute_eclipse(
    data: web::Data<Arc<AppState>>,
    id: web::Path<i64>,
    query: web::Query<EclipseRequest>,
) -> impl Responder {
    let record_id = id.into_inner();
    let compute_path = query.compute_path.unwrap_or(false);

    match db::get_eclipse_record(&data.pool, record_id).await {
        Ok(Some(record)) => {
            let cmd = EclipseCommand::ComputeRecord {
                record: record.clone(),
                compute_path,
            };

            let mut rx = data.eclipse_rx.lock().await;
            if data.eclipse_tx.send(cmd).await.is_err() {
                return HttpResponse::InternalServerError().json(
                    ApiResponse::<()>::err("Eclipse channel send failed"));
            }

            match timeout(Duration::from_millis(CHANNEL_TIMEOUT_MS), rx.recv()).await {
                Ok(Some(EclipseEvent::RecordComputed { result, .. })) => {
                    HttpResponse::Ok().json(ApiResponse::ok(result))
                }
                Ok(Some(EclipseEvent::Error { message })) => {
                    HttpResponse::InternalServerError().json(
                        ApiResponse::<()>::err(format!("Eclipse: {}", message)))
                }
                Ok(None) => {
                    HttpResponse::InternalServerError().json(
                        ApiResponse::<()>::err("Eclipse channel closed"))
                }
                Err(_) => {
                    HttpResponse::RequestTimeout().json(
                        ApiResponse::<()>::err("Eclipse timeout"))
                }
                _ => HttpResponse::InternalServerError().json(
                    ApiResponse::<()>::err("Unexpected eclipse event")),
            }
        }
        Ok(None) => HttpResponse::NotFound().json(ApiResponse::<()>::err("Eclipse record not found")),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

#[post("/eclipses/compute")]
async fn api_compute_single_eclipse(
    data: web::Data<Arc<AppState>>,
    body: web::Json<EclipseComputeSingleRequest>,
) -> impl Responder {
    let req = body.into_inner();
    let cmd = EclipseCommand::ComputeSingle {
        year_ce: req.year_ce,
        month: req.month,
        day: req.day,
        eclipse_type: req.eclipse_type,
        compute_path: req.compute_path.unwrap_or(false),
    };

    let mut rx = data.eclipse_rx.lock().await;
    if data.eclipse_tx.send(cmd).await.is_err() {
        return HttpResponse::InternalServerError().json(
            ApiResponse::<()>::err("Eclipse channel send failed"));
    }

    match timeout(Duration::from_millis(CHANNEL_TIMEOUT_MS), rx.recv()).await {
        Ok(Some(EclipseEvent::SingleComputed { result })) => {
            HttpResponse::Ok().json(ApiResponse::ok(result))
        }
        Ok(Some(EclipseEvent::Error { message })) => {
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::err(format!("Eclipse: {}", message)))
        }
        Ok(None) => {
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::err("Eclipse channel closed"))
        }
        Err(_) => {
            HttpResponse::RequestTimeout().json(
                ApiResponse::<()>::err("Eclipse timeout"))
        }
        _ => HttpResponse::InternalServerError().json(
            ApiResponse::<()>::err("Unexpected eclipse event")),
    }
}

// ============================================================
// 仪器误差反演 API (通过 instrument 模块)
// ============================================================

#[get("/instruments")]
async fn api_list_instruments(data: web::Data<Arc<AppState>>) -> impl Responder {
    match db::list_instruments(&data.pool).await {
        Ok(list) => {
            let total = list.len() as i64;
            HttpResponse::Ok().json(ApiResponse::ok_with_count(list, total))
        }
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

#[get("/instruments/{id}")]
async fn api_get_instrument(
    data: web::Data<Arc<AppState>>,
    id: web::Path<i64>,
) -> impl Responder {
    match db::get_instrument(&data.pool, id.into_inner()).await {
        Ok(Some(i)) => HttpResponse::Ok().json(ApiResponse::ok(i)),
        Ok(None) => HttpResponse::NotFound().json(ApiResponse::<()>::err("Instrument not found")),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

#[get("/instruments/{id}/observations")]
async fn api_list_observations(
    data: web::Data<Arc<AppState>>,
    id: web::Path<i64>,
) -> impl Responder {
    let instrument_id = id.into_inner();
    match db::list_instrument_observations(&data.pool, instrument_id).await {
        Ok(list) => {
            let total = list.len() as i64;
            HttpResponse::Ok().json(ApiResponse::ok_with_count(list, total))
        }
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

#[post("/instruments/{id}/invert")]
async fn api_invert_instrument_errors(
    data: web::Data<Arc<AppState>>,
    id: web::Path<i64>,
    body: web::Json<InstrumentInversionRequest>,
) -> impl Responder {
    let instrument_id = id.into_inner();
    let _req = body.into_inner();

    match db::list_instrument_observations(&data.pool, instrument_id).await {
        Ok(observations) if !observations.is_empty() => {
            let cmd = InstrumentCommand::InvertErrors {
                observations: observations.clone(),
                ref_observations: None,
            };

            let mut rx = data.instrument_rx.lock().await;
            if data.instrument_tx.send(cmd).await.is_err() {
                return HttpResponse::InternalServerError().json(
                    ApiResponse::<()>::err("Instrument channel send failed"));
            }

            match timeout(Duration::from_millis(CHANNEL_TIMEOUT_MS), rx.recv()).await {
                Ok(Some(InstrumentEvent::ErrorsInverted(solution))) => {
                    HttpResponse::Ok().json(ApiResponse::ok(*solution))
                }
                Ok(Some(InstrumentEvent::Error { message })) => {
                    HttpResponse::InternalServerError().json(
                        ApiResponse::<()>::err(format!("Instrument: {}", message)))
                }
                Ok(None) => {
                    HttpResponse::InternalServerError().json(
                        ApiResponse::<()>::err("Instrument channel closed"))
                }
                Err(_) => {
                    HttpResponse::RequestTimeout().json(
                        ApiResponse::<()>::err("Instrument timeout"))
                }
                _ => HttpResponse::InternalServerError().json(
                    ApiResponse::<()>::err("Unexpected instrument event")),
            }
        }
        Ok(_) => HttpResponse::NotFound().json(
            ApiResponse::<()>::err("No observations found for this instrument")),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

// ============================================================
// 变星亮度演化 API (通过 variable_star 模块)
// ============================================================

#[get("/variables")]
async fn api_list_variable_stars(
    data: web::Data<Arc<AppState>>,
    query: web::Query<VariableStarQuery>,
) -> impl Responder {
    let params = query.into_inner();
    match db::list_variable_stars(
        &data.pool,
        params.gcvs_type.as_deref(),
        params.min_amplitude_mag,
        params.max_period_days,
        params.search_name.as_deref(),
        params.limit.unwrap_or(100i64),
        params.offset.unwrap_or(0i64),
    ).await {
        Ok((list, total)) => {
            HttpResponse::Ok().json(ApiResponse::ok_with_count(list, total))
        }
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

#[get("/variables/{id}")]
async fn api_get_variable_star(
    data: web::Data<Arc<AppState>>,
    id: web::Path<i64>,
) -> impl Responder {
    match db::get_variable_star(&data.pool, id.into_inner()).await {
        Ok(Some(v)) => HttpResponse::Ok().json(ApiResponse::ok(v)),
        Ok(None) => HttpResponse::NotFound().json(ApiResponse::<()>::err("Variable star not found")),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

#[get("/variables/{id}/measurements")]
async fn api_list_measurements(
    data: web::Data<Arc<AppState>>,
    id: web::Path<i64>,
) -> impl Responder {
    let variable_id = id.into_inner();
    match db::list_magnitude_measurements(&data.pool, variable_id).await {
        Ok(list) => {
            let total = list.len() as i64;
            HttpResponse::Ok().json(ApiResponse::ok_with_count(list, total))
        }
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

#[post("/variables/{id}/reconstruct")]
async fn api_reconstruct_lightcurve(
    data: web::Data<Arc<AppState>>,
    id: web::Path<i64>,
    body: web::Json<LightCurveRequest>,
) -> impl Responder {
    let variable_id = id.into_inner();
    let req = body.into_inner();

    match (
        db::get_variable_star(&data.pool, variable_id).await,
        db::list_magnitude_measurements(&data.pool, variable_id).await,
    ) {
        (Ok(Some(meta)), Ok(measurements)) if !measurements.is_empty() => {
            let cmd = VariableStarCommand::ReconstructLongTerm {
                variable_id,
                meta: meta.clone(),
                measurements: measurements.clone(),
            };

            let mut rx = data.variable_rx.lock().await;
            if data.variable_tx.send(cmd).await.is_err() {
                return HttpResponse::InternalServerError().json(
                    ApiResponse::<()>::err("VariableStar channel send failed"));
            }

            match timeout(Duration::from_millis(CHANNEL_TIMEOUT_MS), rx.recv()).await {
                Ok(Some(VariableStarEvent::LongTermReconstruction { reconstruction, .. })) => {
                    HttpResponse::Ok().json(ApiResponse::ok(reconstruction))
                }
                Ok(Some(VariableStarEvent::LightCurveResult { reconstruction, .. })) => {
                    HttpResponse::Ok().json(ApiResponse::ok(reconstruction))
                }
                Ok(Some(VariableStarEvent::Error { message })) => {
                    HttpResponse::InternalServerError().json(
                        ApiResponse::<()>::err(format!("VariableStar: {}", message)))
                }
                Ok(None) => {
                    HttpResponse::InternalServerError().json(
                        ApiResponse::<()>::err("VariableStar channel closed"))
                }
                Err(_) => {
                    HttpResponse::RequestTimeout().json(
                        ApiResponse::<()>::err("VariableStar timeout"))
                }
                _ => HttpResponse::InternalServerError().json(
                    ApiResponse::<()>::err("Unexpected VariableStar event")),
            }
        }
        (Ok(None), _) => HttpResponse::NotFound().json(
            ApiResponse::<()>::err("Variable star not found")),
        (_, Ok(measurements)) if measurements.is_empty() => HttpResponse::NotFound().json(
            ApiResponse::<()>::err("No measurements found for this variable star")),
        (Err(e), _) | (_, Err(e)) => HttpResponse::InternalServerError().json(
            ApiResponse::<()>::err(e)),
        _ => HttpResponse::InternalServerError().json(
            ApiResponse::<()>::err("Unexpected state while reconstructing light curve")),
    }
}

#[post("/variables/{id}/period-analysis")]
async fn api_analyze_period(
    data: web::Data<Arc<AppState>>,
    id: web::Path<i64>,
    body: web::Json<LightCurveRequest>,
) -> impl Responder {
    let variable_id = id.into_inner();
    let req = body.into_inner();

    match db::list_magnitude_measurements(&data.pool, variable_id).await {
        Ok(measurements) if !measurements.is_empty() => {
            let cmd = VariableStarCommand::GetLightCurve {
                variable_id,
                measurements: measurements.clone(),
                use_published_period: req.use_published_period,
                override_period_days: req.override_period_days,
                include_ancient_only_fit: req.include_ancient_only_fit,
                reconstruction_resolution_per_phase: req.reconstruction_resolution_per_phase,
            };

            let mut rx = data.variable_rx.lock().await;
            if data.variable_tx.send(cmd).await.is_err() {
                return HttpResponse::InternalServerError().json(
                    ApiResponse::<()>::err("VariableStar channel send failed"));
            }

            match timeout(Duration::from_millis(CHANNEL_TIMEOUT_MS), rx.recv()).await {
                Ok(Some(VariableStarEvent::LightCurveResult { reconstruction, .. })) => {
                    let period_analysis = serde_json::json!({
                        "variable_id": reconstruction.variable_id,
                        "best_period_days": reconstruction.best_period_days,
                        "best_period_uncertainty_days": reconstruction.best_period_uncertainty_days,
                        "periodogram": reconstruction.periodogram,
                        "phase_folded_samples": reconstruction.phase_folded_samples,
                        "chi_squared": reconstruction.chi_squared,
                        "reduced_chi_squared": reconstruction.reduced_chi_squared,
                        "period_change_significance_sigma": reconstruction.period_change_significance_sigma,
                        "pdot_estimate": reconstruction.pdot_estimate,
                        "ancient_vs_modern_period_delta_days": reconstruction.ancient_vs_modern_period_delta_days,
                        "ancient_period_determination_days": reconstruction.ancient_period_determination_days,
                    });
                    HttpResponse::Ok().json(ApiResponse::ok(period_analysis))
                }
                Ok(Some(VariableStarEvent::Error { message })) => {
                    HttpResponse::InternalServerError().json(
                        ApiResponse::<()>::err(format!("VariableStar: {}", message)))
                }
                Ok(None) => {
                    HttpResponse::InternalServerError().json(
                        ApiResponse::<()>::err("VariableStar channel closed"))
                }
                Err(_) => {
                    HttpResponse::RequestTimeout().json(
                        ApiResponse::<()>::err("VariableStar timeout"))
                }
                _ => HttpResponse::InternalServerError().json(
                    ApiResponse::<()>::err("Unexpected VariableStar event")),
            }
        }
        Ok(_) => HttpResponse::NotFound().json(
            ApiResponse::<()>::err("No measurements found for this variable star")),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::err(e)),
    }
}

// ============================================================
// 公众科普交互 API (通过 horoscope 模块)
// ============================================================

#[post("/horoscope/starmap")]
async fn api_generate_personal_starmap(
    data: web::Data<Arc<AppState>>,
    body: web::Json<PersonalStarmapRequest>,
) -> impl Responder {
    let req = body.into_inner();

    match (db::query_stars(&data.pool, &StarQueryParams {
        limit: Some(500),
        ..Default::default()
    }).await, db::list_mansions(&data.pool).await) {
        (Ok((stars, _)), Ok(mansions)) => {
            let cmd = HoroscopeCommand::GenerateStarmap {
                request: req.clone(),
                stars: stars.clone(),
                mansions: mansions.clone(),
            };

            let mut rx = data.horoscope_rx.lock().await;
            if data.horoscope_tx.send(cmd).await.is_err() {
                return HttpResponse::InternalServerError().json(
                    ApiResponse::<()>::err("Horoscope channel send failed"));
            }

            match timeout(Duration::from_millis(CHANNEL_TIMEOUT_MS), rx.recv()).await {
                Ok(Some(HoroscopeEvent::StarmapGenerated(starmap))) => {
                    HttpResponse::Ok().json(ApiResponse::ok(*starmap))
                }
                Ok(Some(HoroscopeEvent::Error { message })) => {
                    HttpResponse::InternalServerError().json(
                        ApiResponse::<()>::err(format!("Horoscope: {}", message)))
                }
                Ok(None) => {
                    HttpResponse::InternalServerError().json(
                        ApiResponse::<()>::err("Horoscope channel closed"))
                }
                Err(_) => {
                    HttpResponse::RequestTimeout().json(
                        ApiResponse::<()>::err("Horoscope timeout"))
                }
                _ => HttpResponse::InternalServerError().json(
                    ApiResponse::<()>::err("Unexpected horoscope event")),
            }
        }
        (Err(e), _) | (_, Err(e)) => HttpResponse::InternalServerError().json(
            ApiResponse::<()>::err(e)),
    }
}

#[get("/horoscope/share/{hash}")]
async fn api_get_share_card(
    _data: web::Data<Arc<AppState>>,
    hash: web::Path<String>,
) -> impl Responder {
    let _hash = hash.into_inner();
    HttpResponse::Ok().json(ApiResponse::ok(ShareCardSpec {
        width_px: 800,
        height_px: 1200,
        title_text: "个人星图".to_string(),
        subtitle_text: format!("Hash: {}", _hash),
        footer_text: "share cards are generated in-memory, please use /horoscope/starmap endpoint first".to_string(),
        accent_color_hex: "#FFD700".to_string(),
        background_gradient_from_hex: "#0a0a2e".to_string(),
        background_gradient_to_hex: "#1a1a4e".to_string(),
        render_payload: "{}".to_string(),
        shareable_hash: _hash,
    }))
}

// ============================================================
// 响应构造
// ============================================================

fn build_convert_response(r: &TransformResult) -> serde_json::Value {
    serde_json::json!({
        "ancient_equatorial": {
            "ra_deg": r.ancient_ra,
            "dec_deg": r.ancient_dec,
        },
        "j2000": {
            "ra_deg": r.ra_j2000,
            "dec_deg": r.dec_j2000,
            "without_proper_motion": {
                "ra_deg": r.ra_without_pm,
                "dec_deg": r.dec_without_pm,
            }
        },
        "precession_matrix": r.precession_matrix,
        "corrections": {
            "nutation_psi_arcsec": r.nutation_correction[0],
            "nutation_eps_arcsec": r.nutation_correction[1],
            "planetary_chi_arcsec": r.planetary_correction_arcsec,
        },
        "proper_motion_1000yr": {
            "dra_deg": r.proper_motion_arrow[0],
            "ddec_deg": r.proper_motion_arrow[1],
            "position_angle_deg": r.proper_motion_arrow[2],
        },
        "error_estimate": {
            "ra_arcsec": r.error_estimate.ra_error_arcsec,
            "dec_arcsec": r.error_estimate.dec_error_arcsec,
            "model_arcsec": r.error_estimate.model_error_arcsec,
            "observation_arcsec": r.error_estimate.observation_error_arcsec,
            "proper_motion_arcsec": r.error_estimate.proper_motion_error_arcsec,
        }
    })
}

// ============================================================
// 主入口
// ============================================================

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    telemetry::init_tracing();

    info!("======================================================");
    info!("  Ancient Star Catalog Backend v{}", env!("CARGO_PKG_VERSION"));
    info!("  Architecture: 7-modules + tokio channels");
    info!("======================================================");

    let config_dir = env::var("CONFIG_DIR").unwrap_or_else(|_| "./config".into());
    let config = match config::AppConfig::load(&config_dir) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to load config from {}: {}", config_dir, e);
            error!("Make sure config/precession.json, config/matching.json, config/catalog.json exist");
            return Err(std::io::Error::new(std::io::ErrorKind::Other, e));
        }
    };
    info!("  Config loaded from: {}", config_dir);
    info!("  - Precession: {}", config.precession.model_name);
    info!("  - Matching:   {}", config.matching.model_name);
    info!("  - Catalog:    {}", config.catalog.model_name);
    info!("  - Eclipse:    {}", config.eclipse.model_name);
    info!("  - Instrument: {}", config.instrument.model_name);
    info!("  - Variable:   {}", config.variable.model_name);
    info!("  - Horoscope:  {}", config.horoscope.model_name);

    let host = env::var("API_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = env::var("API_PORT").ok()
        .and_then(|s| s.parse().ok()).unwrap_or(8080);

    let pool = db::create_pool().expect("Failed to create DB pool");

    let metrics = Arc::new(telemetry::register_metrics()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?);

    info!("Spawning modules...");
    let (loader_tx, loader_rx) = catalog_loader::spawn_loader(
        config.catalog.clone());
    info!("catalog_loader started (DB import + cleaning)");

    let (transform_tx, transform_rx) = coordinate_transformer::spawn_transformer(
        config.precession.clone());
    info!("coordinate_transformer started (IAU 2006 + error estimate)");

    let (match_tx, match_rx) = transient_matcher::spawn_matcher(
        config.matching.clone());
    info!("transient_matcher started (Galactic prior Bayes)");

    let (eclipse_tx, eclipse_rx) = eclipse::spawn_eclipse_engine(config.eclipse.clone());
    info!("eclipse_engine started (Saros cycle + ΔT estimation)");

    let (instrument_tx, instrument_rx) = instrument::spawn_instrument_service(config.instrument.clone());
    info!("instrument_service started (LSQ error inversion)");

    let (variable_tx, variable_rx) = variable_star::spawn_variable_star_service(config.variable.clone());
    info!("variable_star_service started (Lomb-Scargle periodogram)");

    let (horoscope_tx, horoscope_rx) = horoscope::spawn_horoscope_service(config.horoscope.clone());
    info!("horoscope_service started (Personal starmap projection)");

    let state = Arc::new(AppState {
        pool,
        config,
        loader_tx,
        loader_rx: Arc::new(Mutex::new(loader_rx)),
        transform_tx,
        transform_rx: Arc::new(Mutex::new(transform_rx)),
        match_tx,
        match_rx: Arc::new(Mutex::new(match_rx)),
        eclipse_tx,
        eclipse_rx: Arc::new(Mutex::new(eclipse_rx)),
        instrument_tx,
        instrument_rx: Arc::new(Mutex::new(instrument_rx)),
        variable_tx,
        variable_rx: Arc::new(Mutex::new(variable_rx)),
        horoscope_tx,
        horoscope_rx: Arc::new(Mutex::new(horoscope_rx)),
        metrics: metrics.clone(),
    });

    info!("======================================================");
    info!("  API: http://{}:{}", host, port);
    info!("======================================================");

    HttpServer::new(move || {
        let cors = Cors::permissive();
        App::new()
            .app_data(web::Data::new(state.clone()))
            .wrap(cors)
            .service(
                web::scope("/api")
                    .service(api_health)
                    .service(api_metrics)
                    .service(api_dynasties)
                    .service(api_mansions)
                    .service(api_query_stars)
                    .service(api_get_star)
                    .service(api_cross_dynasty)
                    .service(api_convert_ruxiu)
                    .service(api_trajectory)
                    .service(api_comets)
                    .service(api_guest_stars)
                    .service(api_get_guest)
                    .service(api_snr)
                    .service(api_get_matches)
                    .service(api_run_match)
                    .service(api_list_eclipse_records)
                    .service(api_get_eclipse_record)
                    .service(api_compute_eclipse)
                    .service(api_compute_single_eclipse)
                    .service(api_list_instruments)
                    .service(api_get_instrument)
                    .service(api_list_observations)
                    .service(api_invert_instrument_errors)
                    .service(api_list_variable_stars)
                    .service(api_get_variable_star)
                    .service(api_list_measurements)
                    .service(api_reconstruct_lightcurve)
                    .service(api_analyze_period)
                    .service(api_generate_personal_starmap)
                    .service(api_get_share_card)
            )
            .service(Files::new("/", "./static").index_file("index.html"))
    })
    .bind((host.as_str(), port))?
    .run()
    .await
}
