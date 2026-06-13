//! 数据库访问层

use deadpool_postgres::{Config, PoolConfig, Runtime, ManagerConfig, RecyclingMethod};
use tokio_postgres::{NoTls, Row, types::ToSql};
use std::env;
use tracing::{info, trace, error};

pub use deadpool_postgres::Pool as DbPool;

use crate::models::*;
use crate::matching::{GuestStarObs, SupernovaRemnant as SnrMatchInput, MatchCandidate};

/// 创建数据库连接池
pub fn create_pool() -> Result<DbPool, String> {
    let mut cfg = Config::new();
    cfg.host = Some(env::var("DB_HOST").unwrap_or_else(|_| "localhost".into()));
    cfg.port = Some(env::var("DB_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(5432));
    cfg.dbname = Some(env::var("DB_NAME").unwrap_or_else(|_| "ancient_star_catalog".into()));
    cfg.user = Some(env::var("DB_USER").unwrap_or_else(|_| "postgres".into()));
    cfg.password = Some(env::var("DB_PASSWORD").unwrap_or_else(|_| "postgres".into()));
    let max_conn: usize = env::var("MAX_DB_CONN").ok()
        .and_then(|s| s.parse().ok()).unwrap_or(16);
    cfg.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });
    cfg.pool = Some(PoolConfig::new(max_conn));

    info!(
        "DB: Creating connection pool for {}:{}/{} (max_conn={})",
        cfg.host.as_deref().unwrap_or("?"),
        cfg.port.unwrap_or(5432),
        cfg.dbname.as_deref().unwrap_or("?"),
        max_conn
    );
    let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls).map_err(|e| {
        error!("DB: Failed to create pool: {}", e);
        e.to_string()
    })?;
    info!("DB: Connection pool created successfully");
    Ok(pool)
}

// ============================================================
// 通用: Row → Struct
// ============================================================

fn get_opt<T: tokio_postgres::types::FromSqlOwned>(row: &Row, col: &str) -> Option<T> {
    row.try_get(col).ok()
}
fn get_str(row: &Row, col: &str) -> String {
    row.try_get::<&str, Option<String>>(col).ok().flatten().unwrap_or_default()
}

fn row_to_dynasty(r: &Row) -> Dynasty {
    Dynasty {
        id: r.get("id"),
        name_cn: get_str(r, "name_cn"),
        name_pinyin: get_str(r, "name_pinyin"),
        start_year: r.get("start_year"),
        end_year: r.get("end_year"),
        canonical_epoch: r.get("canonical_epoch"),
        color_hex: get_str(r, "color_hex"),
    }
}

fn row_to_mansion(r: &Row) -> LunarMansion {
    LunarMansion {
        id: r.get("id"),
        mansion_order: r.get("mansion_order"),
        name_cn: get_str(r, "name_cn"),
        name_pinyin: get_str(r, "name_pinyin"),
        ruxiu_width_deg: r.get("ruxiu_width_deg"),
        ra_start_deg: r.get("ra_start_deg"),
        ra_end_deg: r.get("ra_end_deg"),
    }
}

fn row_to_star(r: &Row) -> AncientStar {
    AncientStar {
        id: r.get("id"),
        star_id_code: get_str(r, "star_id_code"),
        dynasty_id: r.get("dynasty_id"),
        mansion_id: get_opt(r, "mansion_id"),
        star_name_cn: get_str(r, "star_name_cn"),
        star_name_alt: get_opt(r, "star_name_alt"),
        constellation: get_opt(r, "constellation"),
        ruxiu_du: get_opt(r, "ruxiu_du"),
        quji_du: get_opt(r, "quji_du"),
        ra_ancient_conv: get_opt(r, "ra_ancient_conv"),
        dec_ancient_conv: get_opt(r, "dec_ancient_conv"),
        ra_j2000: get_opt(r, "ra_j2000"),
        dec_j2000: get_opt(r, "dec_j2000"),
        magnitude_ancient: get_opt(r, "magnitude_ancient"),
        magnitude_num: get_opt(r, "magnitude_num"),
        color_desc: get_opt(r, "color_desc"),
        color_class: get_opt(r, "color_class"),
        color_temp_k: get_opt(r, "color_temp_k"),
        proper_motion_ra: get_opt(r, "proper_motion_ra"),
        proper_motion_dec: get_opt(r, "proper_motion_dec"),
        parallax: get_opt(r, "parallax"),
        source_book: get_opt(r, "source_book"),
        quality_flag: r.get("quality_flag"),
        notes: get_opt(r, "notes"),
        modern_hd_id: get_opt(r, "modern_hd_id"),
        cross_match_id: get_opt(r, "cross_match_id"),
        dynasty_name: get_opt(r, "dynasty_name"),
        mansion_name: get_opt(r, "mansion_name"),
        mansion_order: get_opt(r, "mansion_order"),
    }
}

fn row_to_comet(r: &Row) -> AncientComet {
    AncientComet {
        id: r.get("id"),
        comet_id_code: get_str(r, "comet_id_code"),
        dynasty_id: r.get("dynasty_id"),
        year_ancient: get_opt(r, "year_ancient"),
        year_ce: get_opt(r, "year_ce"),
        ruxiu_du: get_opt(r, "ruxiu_du"),
        quji_du: get_opt(r, "quji_du"),
        ra_deg: get_opt(r, "ra_deg"),
        dec_deg: get_opt(r, "dec_deg"),
        magnitude: get_opt(r, "magnitude"),
        color_desc: get_opt(r, "color_desc"),
        tail_direction: get_opt(r, "tail_direction"),
        tail_length: get_opt(r, "tail_length"),
        duration_days: get_opt(r, "duration_days"),
        description: get_opt(r, "description"),
        dynasty_name: get_opt(r, "dynasty_name"),
    }
}

fn row_to_guest(r: &Row) -> GuestStar {
    GuestStar {
        id: r.get("id"),
        guest_id_code: get_str(r, "guest_id_code"),
        dynasty_id: r.get("dynasty_id"),
        star_name: get_opt(r, "star_name"),
        year_ancient: r.get("year_ancient"),
        year_ce: r.get("year_ce"),
        month_ancient: get_opt(r, "month_ancient"),
        day_ancient: get_opt(r, "day_ancient"),
        ruxiu_du: get_opt(r, "ruxiu_du"),
        quji_du: get_opt(r, "quji_du"),
        ra_deg: get_opt(r, "ra_deg"),
        dec_deg: get_opt(r, "dec_deg"),
        ra_err: r.get("ra_err"),
        dec_err: r.get("dec_err"),
        peak_mag: r.get("peak_mag"),
        peak_mag_err: r.get("peak_mag_err"),
        visibility_days: get_opt(r, "visibility_days"),
        lightcurve_type: get_str(r, "lightcurve_type"),
        description: get_opt(r, "description"),
        position_desc: get_opt(r, "position_desc"),
        dynasty_name: get_opt(r, "dynasty_name"),
        matched_snr_id: get_opt(r, "matched_snr_id"),
    }
}

fn row_to_snr(r: &Row) -> SupernovaRemnantDb {
    SupernovaRemnantDb {
        id: r.get("id"),
        remnant_name: get_str(r, "remnant_name"),
        sn_type: get_str(r, "sn_type"),
        ra_deg: r.get("ra_deg"),
        dec_deg: r.get("dec_deg"),
        gal_l: get_opt(r, "gal_l"),
        gal_b: get_opt(r, "gal_b"),
        age_yr: r.get("age_yr"),
        age_err_yr: r.get("age_err_yr"),
        distance_kpc: r.get("distance_kpc"),
        distance_err: r.get("distance_err"),
        diameter_pc: get_opt(r, "diameter_pc"),
        radio_flux_ghz: get_opt(r, "radio_flux_ghz"),
        xray_luminosity: get_opt(r, "xray_luminosity"),
        gamma_detected: r.get("gamma_detected"),
        historical_sn_id: get_opt(r, "historical_sn_id"),
    }
}

fn row_to_match(r: &Row) -> MatchResult {
    MatchResult {
        id: r.get("id"),
        guest_id: r.get("guest_id"),
        remnant_id: r.get("remnant_id"),
        remnant_name: get_str(r, "remnant_name"),
        remnant_type: get_str(r, "remnant_type"),
        rank_within_guest: r.get("rank_within_guest"),
        match_probability: r.get("match_probability"),
        log_posterior: r.get("log_posterior"),
        log_likelihood: r.get("log_likelihood"),
        log_prior: r.get("log_prior"),
        bayes_factor: r.get("bayes_factor"),
        angular_sep_arcmin: r.get("angular_sep_arcmin"),
        time_delta_yr: r.get("time_delta_yr"),
        spatial_score: r.get("spatial_score"),
        temporal_score: r.get("temporal_score"),
        magnitude_score: r.get("magnitude_score"),
        lightcurve_score: r.get("lightcurve_score"),
        model_version: get_str(r, "model_version"),
    }
}

fn row_to_dynasty_info(r: &Row, prefix: &str) -> DynastyInfo {
    let id_col = format!("{prefix}_id");
    let name_col = format!("{prefix}_name");
    let year_col = format!("{prefix}_year");
    DynastyInfo {
        id: r.get(id_col.as_str()),
        name: get_str(r, name_col.as_str()),
        year: r.get(year_col.as_str()),
    }
}

fn row_to_cross(r: &Row) -> CrossDynastyPair {
    CrossDynastyPair {
        dynasty_1: row_to_dynasty_info(r, "d1"),
        dynasty_2: row_to_dynasty_info(r, "d2"),
        star_id_1: r.get("s1_id"),
        star_id_2: r.get("s2_id"),
        delta_ruxiu: r.get("delta_ruxiu"),
        delta_quji: r.get("delta_quji"),
        delta_ra: r.get("delta_ra"),
        delta_dec: r.get("delta_dec"),
    }
}

// ============================================================
// 查询函数
// ============================================================

pub async fn list_dynasties(pool: &DbPool) -> Result<Vec<Dynasty>, String> {
    trace!("DB: list_dynasties");
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let rows = client.query(
        "SELECT * FROM dynasties ORDER BY start_year", &[]).await
        .map_err(|e| { error!("DB: list_dynasties query error: {}", e); e.to_string() })?;
    trace!("DB: list_dynasties returned {} rows", rows.len());
    Ok(rows.iter().map(row_to_dynasty).collect())
}

pub async fn list_mansions(pool: &DbPool) -> Result<Vec<LunarMansion>, String> {
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let rows = client.query(
        "SELECT * FROM lunar_mansions ORDER BY mansion_order", &[]).await
        .map_err(|e| e.to_string())?;
    Ok(rows.iter().map(row_to_mansion).collect())
}

pub async fn query_stars(pool: &DbPool, params: &StarQueryParams)
    -> Result<(Vec<AncientStar>, i64), String>
{
    trace!(
        "DB: query_stars dynasty={:?} mansion={:?} name={:?} limit={:?}",
        params.dynasty_id, params.mansion_id, params.star_name, params.limit
    );
    let mut sql = String::from(
        "SELECT s.*, d.name_cn AS dynasty_name, m.name_cn AS mansion_name,
                m.mansion_order AS mansion_order
         FROM ancient_stars s
         LEFT JOIN dynasties d ON s.dynasty_id = d.id
         LEFT JOIN lunar_mansions m ON s.mansion_id = m.id
         WHERE 1=1"
    );
    let mut psql: Vec<Box<dyn ToSql + Sync>> = Vec::new();
    let mut idx: i32 = 1;

    if let Some(v) = params.dynasty_id {
        sql.push_str(&format!(" AND s.dynasty_id = ${}", idx));
        idx += 1;
        psql.push(Box::new(v));
    }
    if let Some(ref v) = params.dynasty_name {
        sql.push_str(&format!(" AND d.name_cn ILIKE ${}", idx));
        idx += 1;
        psql.push(Box::new(v.clone()));
    }
    if let Some(v) = params.mansion_id {
        sql.push_str(&format!(" AND s.mansion_id = ${}", idx));
        idx += 1;
        psql.push(Box::new(v));
    }
    if let Some(ref v) = params.constellation {
        sql.push_str(&format!(" AND s.constellation ILIKE ${}", idx));
        idx += 1;
        psql.push(Box::new(v.clone()));
    }
    if let Some(ref v) = params.star_name {
        sql.push_str(&format!(" AND (s.star_name_cn ILIKE ${} OR s.star_name_alt ILIKE ${})", idx, idx));
        idx += 1;
        psql.push(Box::new(v.clone()));
    }
    if let Some(v) = params.mag_min {
        sql.push_str(&format!(" AND s.magnitude_num >= ${}", idx));
        idx += 1;
        psql.push(Box::new(v));
    }
    if let Some(v) = params.mag_max {
        sql.push_str(&format!(" AND s.magnitude_num <= ${}", idx));
        idx += 1;
        psql.push(Box::new(v));
    }
    if let Some(v) = params.ra_min {
        sql.push_str(&format!(" AND s.ra_j2000 >= ${}", idx));
        idx += 1;
        psql.push(Box::new(v));
    }
    if let Some(v) = params.ra_max {
        sql.push_str(&format!(" AND s.ra_j2000 <= ${}", idx));
        idx += 1;
        psql.push(Box::new(v));
    }
    if let Some(v) = params.dec_min {
        sql.push_str(&format!(" AND s.dec_j2000 >= ${}", idx));
        idx += 1;
        psql.push(Box::new(v));
    }
    if let Some(v) = params.dec_max {
        sql.push_str(&format!(" AND s.dec_j2000 <= ${}", idx));
        idx += 1;
        psql.push(Box::new(v));
    }
    if let Some(v) = params.quality_min {
        sql.push_str(&format!(" AND s.quality_flag >= ${}", idx));
        idx += 1;
        psql.push(Box::new(v));
    }
    if let Some(ref v) = params.source_book {
        sql.push_str(&format!(" AND s.source_book ILIKE ${}", idx));
        idx += 1;
        psql.push(Box::new(v.clone()));
    }

    let psql_ref: Vec<&(dyn ToSql + Sync)> = psql.iter().map(|b| b.as_ref()).collect();

    let count_sql = format!("SELECT COUNT(*) FROM ({}) q", sql);
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let cnt_row = client.query_one(&count_sql, &psql_ref).await.map_err(|e| e.to_string())?;
    let count: i64 = cnt_row.get(0);

    sql.push_str(" ORDER BY s.magnitude_num NULLS LAST, s.id");
    if let Some(v) = params.limit {
        sql.push_str(&format!(" LIMIT ${}", idx));
        idx += 1;
        psql.push(Box::new(v));
    }
    if let Some(v) = params.offset {
        sql.push_str(&format!(" OFFSET ${}", idx));
        psql.push(Box::new(v));
    }

    let psql_ref2: Vec<&(dyn ToSql + Sync)> = psql.iter().map(|b| b.as_ref()).collect();
    let rows = client.query(&sql, &psql_ref2).await
        .map_err(|e| { error!("DB: query_stars error: {}", e); e.to_string() })?;
    trace!("DB: query_stars returned {}/{} rows", rows.len(), count);
    Ok((rows.iter().map(row_to_star).collect(), count))
}

pub async fn get_star(pool: &DbPool, id: i64) -> Result<Option<AncientStar>, String> {
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let rows = client.query(
        "SELECT s.*, d.name_cn AS dynasty_name, m.name_cn AS mansion_name,
                m.mansion_order AS mansion_order
         FROM ancient_stars s
         LEFT JOIN dynasties d ON s.dynasty_id = d.id
         LEFT JOIN lunar_mansions m ON s.mansion_id = m.id
         WHERE s.id = $1", &[&id]).await
        .map_err(|e| e.to_string())?;
    Ok(rows.first().map(row_to_star))
}

pub async fn list_comets(pool: &DbPool) -> Result<Vec<AncientComet>, String> {
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let rows = client.query(
        "SELECT c.*, d.name_cn AS dynasty_name FROM ancient_comets c
         LEFT JOIN dynasties d ON c.dynasty_id = d.id", &[]).await
        .map_err(|e| e.to_string())?;
    Ok(rows.iter().map(row_to_comet).collect())
}

pub async fn list_guest_stars(pool: &DbPool) -> Result<Vec<GuestStar>, String> {
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let rows = client.query(
        "SELECT g.*, d.name_cn AS dynasty_name FROM guest_stars g
         LEFT JOIN dynasties d ON g.dynasty_id = d.id
         ORDER BY g.year_ce", &[]).await
        .map_err(|e| e.to_string())?;
    Ok(rows.iter().map(row_to_guest).collect())
}

pub async fn get_guest_star(pool: &DbPool, id: i64) -> Result<Option<GuestStar>, String> {
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let rows = client.query(
        "SELECT g.*, d.name_cn AS dynasty_name FROM guest_stars g
         LEFT JOIN dynasties d ON g.dynasty_id = d.id
         WHERE g.id = $1", &[&id]).await
        .map_err(|e| e.to_string())?;
    Ok(rows.first().map(row_to_guest))
}

pub async fn list_snr(pool: &DbPool) -> Result<Vec<SupernovaRemnantDb>, String> {
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let rows = client.query("SELECT * FROM supernova_remnants ORDER BY id", &[]).await
        .map_err(|e| e.to_string())?;
    Ok(rows.iter().map(row_to_snr).collect())
}

// ============================================================
// 匹配相关
// ============================================================

pub async fn get_guest_for_match(pool: &DbPool, id: i64)
    -> Result<Option<GuestStarObs>, String>
{
    Ok(get_guest_star(pool, id).await?.map(|g| GuestStarObs {
        id: g.id,
        guest_id_code: g.guest_id_code,
        year_ancient: g.year_ancient,
        year_ce: g.year_ce,
        month_ancient: g.month_ancient,
        day_ancient: g.day_ancient,
        ruxiu_du: g.ruxiu_du,
        quji_du: g.quji_du,
        ra_deg: g.ra_deg,
        dec_deg: g.dec_deg,
        ra_err: g.ra_err,
        dec_err: g.dec_err,
        peak_mag: g.peak_mag,
        peak_mag_err: g.peak_mag_err,
        visibility_days: g.visibility_days,
        lightcurve_type: g.lightcurve_type,
        description: g.description,
        position_desc: g.position_desc,
        dynasty_name: g.dynasty_name,
    }))
}

pub async fn list_snr_for_match(pool: &DbPool) -> Result<Vec<SnrMatchInput>, String> {
    Ok(list_snr(pool).await?.into_iter().map(|s| SnrMatchInput {
        id: s.id,
        remnant_name: s.remnant_name,
        sn_type: s.sn_type,
        ra_deg: s.ra_deg,
        dec_deg: s.dec_deg,
        gal_l: s.gal_l,
        gal_b: s.gal_b,
        age_yr: s.age_yr,
        age_err_yr: s.age_err_yr,
        distance_kpc: s.distance_kpc,
        distance_err: s.distance_err,
        diameter_pc: s.diameter_pc,
        radio_flux_ghz: s.radio_flux_ghz,
        xray_luminosity: s.xray_luminosity,
        gamma_detected: s.gamma_detected,
        historical_sn_id: s.historical_sn_id,
    }).collect())
}

pub async fn save_match_result(
    pool: &DbPool,
    guest_id: i64,
    candidates: &[MatchCandidate],
    model_version: &str,
) -> Result<(), String> {
    trace!(
        "DB: save_match_result guest_id={} candidates={} version={}",
        guest_id, candidates.len(), model_version
    );
    let client = pool.get().await.map_err(|e| e.to_string())?;
    client.execute("DELETE FROM guest_star_matches WHERE guest_id = $1", &[&guest_id])
        .await.map_err(|e| { error!("DB: save_match_result delete error: {}", e); e.to_string() })?;
    for c in candidates {
        client.execute(
            "INSERT INTO guest_star_matches
             (guest_id, remnant_id, rank_within_guest, match_probability,
              log_posterior, log_likelihood, log_prior, bayes_factor,
              angular_sep_arcmin, time_delta_yr, spatial_score,
              temporal_score, magnitude_score, lightcurve_score, model_version)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
            &[&c.guest_id, &c.remnant_id, &c.rank_within_guest,
              &c.match_probability, &c.log_posterior, &c.log_likelihood,
              &c.log_prior, &c.bayes_factor, &c.angular_sep_arcmin,
              &c.time_delta_yr, &c.spatial_score, &c.temporal_score,
              &c.magnitude_score, &c.lightcurve_score, &model_version],
        ).await.map_err(|e| { error!("DB: save_match_result insert error: {}", e); e.to_string() })?;
    }
    info!("DB: save_match_result guest_id={} saved {} candidates", guest_id, candidates.len());
    Ok(())
}

pub async fn get_match_results(pool: &DbPool, guest_id: i64)
    -> Result<Vec<MatchResult>, String>
{
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let rows = client.query(
        "SELECT m.*, r.remnant_name, r.sn_type AS remnant_type
         FROM guest_star_matches m
         JOIN supernova_remnants r ON m.remnant_id = r.id
         WHERE m.guest_id = $1
         ORDER BY m.rank_within_guest", &[&guest_id]).await
        .map_err(|e| e.to_string())?;
    Ok(rows.iter().map(row_to_match).collect())
}

// ============================================================
// 跨朝代对比
// ============================================================

pub async fn get_star_cross_dynasty(pool: &DbPool, _star_id: Option<i64>, star_name: Option<String>)
    -> Result<Vec<CrossDynastyPair>, String>
{
    // 简化版: 通过星名精确匹配跨朝代记录
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let rows = if let Some(ref name) = star_name {
        client.query(
            "SELECT
                s1.id AS s1_id, s2.id AS s2_id,
                s1.ruxiu_du - s2.ruxiu_du AS delta_ruxiu,
                s1.quji_du - s2.quji_du AS delta_quji,
                s1.ra_j2000 - s2.ra_j2000 AS delta_ra,
                s1.dec_j2000 - s2.dec_j2000 AS delta_dec,
                d1.id AS d1_id, d1.name_cn AS d1_name, d1.canonical_epoch::int AS d1_year,
                d2.id AS d2_id, d2.name_cn AS d2_name, d2.canonical_epoch::int AS d2_year
             FROM ancient_stars s1
             JOIN ancient_stars s2 ON s1.star_name_cn = s2.star_name_cn AND s1.dynasty_id < s2.dynasty_id
             JOIN dynasties d1 ON s1.dynasty_id = d1.id
             JOIN dynasties d2 ON s2.dynasty_id = d2.id
             WHERE s1.star_name_cn = $1 AND s1.ruxiu_du IS NOT NULL AND s2.ruxiu_du IS NOT NULL
             ORDER BY d1.start_year, d2.start_year
             LIMIT 20",
            &[&name.clone()],
        ).await.map_err(|e| e.to_string())?
    } else {
        client.query(
            "SELECT
                s1.id AS s1_id, s2.id AS s2_id,
                s1.ruxiu_du - s2.ruxiu_du AS delta_ruxiu,
                s1.quji_du - s2.quji_du AS delta_quji,
                s1.ra_j2000 - s2.ra_j2000 AS delta_ra,
                s1.dec_j2000 - s2.dec_j2000 AS delta_dec,
                d1.id AS d1_id, d1.name_cn AS d1_name, d1.canonical_epoch::int AS d1_year,
                d2.id AS d2_id, d2.name_cn AS d2_name, d2.canonical_epoch::int AS d2_year
             FROM ancient_stars s1
             JOIN ancient_stars s2 ON s1.star_name_cn = s2.star_name_cn AND s1.dynasty_id < s2.dynasty_id
             JOIN dynasties d1 ON s1.dynasty_id = d1.id
             JOIN dynasties d2 ON s2.dynasty_id = d2.id
             WHERE s1.ruxiu_du IS NOT NULL AND s2.ruxiu_du IS NOT NULL
             ORDER BY d1.start_year, d2.start_year
             LIMIT 50",
            &[],
        ).await.map_err(|e| e.to_string())?
    };
    Ok(rows.iter().map(row_to_cross).collect())
}

// ============================================================
// 新 Feature: Row → Struct 映射
// ============================================================

fn row_to_eclipse(r: &Row) -> EclipseRecord {
    EclipseRecord {
        id: r.get("id"),
        eclipse_id_code: get_str(r, "eclipse_id_code"),
        dynasty_id: r.get("dynasty_id"),
        eclipse_type: get_str(r, "eclipse_type"),
        year_ancient: get_opt(r, "year_ancient"),
        year_ce: r.get("year_ce"),
        month_ancient: get_opt(r, "month_ancient"),
        day_ancient: get_opt(r, "day_ancient"),
        hour_ancient: get_opt(r, "hour_ancient"),
        magnitude_desc: get_opt(r, "magnitude_desc"),
        magnitude_num: get_opt(r, "magnitude_num"),
        duration_desc: get_opt(r, "duration_desc"),
        duration_min: get_opt(r, "duration_min"),
        ruxiu_du: get_opt(r, "ruxiu_du"),
        quji_du: get_opt(r, "quji_du"),
        ra_deg: get_opt(r, "ra_deg"),
        dec_deg: get_opt(r, "dec_deg"),
        dynasty_name: get_opt(r, "dynasty_name"),
        location_desc: get_opt(r, "location_desc"),
        source_book: get_opt(r, "source_book"),
        record_text: get_opt(r, "record_text"),
    }
}

fn row_to_instrument(r: &Row) -> AncientInstrument {
    AncientInstrument {
        id: r.get("id"),
        instrument_code: get_str(r, "instrument_code"),
        name_cn: get_str(r, "name_cn"),
        dynasty_id: r.get("dynasty_id"),
        dynasty_name: get_opt(r, "dynasty_name"),
        erected_year: r.get("erected_year"),
        location_lat_deg: get_opt(r, "location_lat_deg"),
        location_lon_deg: get_opt(r, "location_lon_deg"),
        location_name: get_opt(r, "location_name"),
        ring_count: r.get("ring_count"),
        nominal_accuracy_arcmin: r.get("nominal_accuracy_arcmin"),
        divisions_circle: get_opt(r, "divisions_circle"),
        vernier_resolution_arcmin: get_opt(r, "vernier_resolution_arcmin"),
        description: get_opt(r, "description"),
    }
}

fn row_to_instrument_observation(r: &Row) -> InstrumentObservation {
    InstrumentObservation {
        id: r.get("id"),
        instrument_id: r.get("instrument_id"),
        star_id: get_opt(r, "star_id"),
        star_name_cn: get_opt(r, "star_name_cn"),
        observation_year_ce: r.get("observation_year_ce"),
        ruxiu_du_measured: get_opt(r, "ruxiu_du_measured"),
        quji_du_measured: get_opt(r, "quji_du_measured"),
        ra_deg_measured: get_opt(r, "ra_deg_measured"),
        dec_deg_measured: get_opt(r, "dec_deg_measured"),
        ra_j2000_true: get_opt(r, "ra_j2000_true"),
        dec_j2000_true: get_opt(r, "dec_j2000_true"),
        source_book: get_opt(r, "source_book"),
        quality_flag: r.get("quality_flag"),
    }
}

fn row_to_variable(r: &Row) -> VariableStarMeta {
    let ancient_names_json: Option<serde_json::Value> = get_opt(r, "ancient_names_json");
    let ancient_names = ancient_names_json.and_then(|v| {
        let mut names: Vec<String> = Vec::new();
        if let Some(name_cn) = v.get("name_cn").and_then(|n| n.as_str()) {
            names.push(name_cn.to_string());
        }
        if let Some(alias) = v.get("alias").and_then(|a| a.as_array()) {
            for a in alias {
                if let Some(s) = a.as_str() {
                    names.push(s.to_string());
                }
            }
        }
        if names.is_empty() { None } else { Some(names) }
    });

    VariableStarMeta {
        id: r.get("id"),
        modern_name: get_str(r, "modern_name"),
        ancient_names,
        constellation_code: get_opt(r, "constellation_code"),
        hd_id: get_opt(r, "hd_id"),
        hr_id: get_opt(r, "hr_id"),
        hipparcos_id: get_opt(r, "hipparcos_id"),
        gcvs_variable_type: get_str(r, "gcvs_variable_type"),
        ra_j2000_deg: r.get("ra_j2000_deg"),
        dec_j2000_deg: r.get("dec_j2000_deg"),
        distance_pc: get_opt(r, "distance_pc"),
        distance_err: get_opt(r, "distance_err"),
        spectral_type: get_opt(r, "spectral_type"),
        luminosity_class: get_opt(r, "luminosity_class"),
        min_mag_v: get_opt(r, "min_mag_v"),
        max_mag_v: get_opt(r, "max_mag_v"),
        mean_mag_v: get_opt(r, "mean_mag_v"),
        epoch_mjd_max: get_opt(r, "epoch_mjd_max"),
        published_period_days: get_opt(r, "published_period_days"),
        published_period_err: get_opt(r, "published_period_err"),
        period_change_rate_pdot: get_opt(r, "period_change_rate_pdot"),
    }
}

fn row_to_magnitude(r: &Row) -> MagnitudeMeasurement {
    MagnitudeMeasurement {
        id: r.get("id"),
        variable_id: r.get("variable_id"),
        epoch_yr: r.get("epoch_yr"),
        epoch_mjd: get_opt(r, "epoch_mjd"),
        magnitude: r.get("magnitude"),
        magnitude_uncertainty: get_opt(r, "magnitude_uncertainty"),
        passband: get_str(r, "passband"),
        source_type: get_str(r, "source_type"),
        source_book: get_opt(r, "source_book"),
        ancient_description: get_opt(r, "ancient_description"),
        ancient_quality: get_opt(r, "ancient_quality"),
    }
}

// ============================================================
// 新 Feature: 查询函数
// ============================================================

pub async fn list_eclipse_records(
    pool: &DbPool,
    dynasty_id: Option<i64>,
    eclipse_type: Option<&str>,
    year_min: Option<f64>,
    year_max: Option<f64>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<EclipseRecord>, i64), String> {
    trace!(
        "DB: list_eclipse_records dynasty={:?} type={:?} year_min={:?} year_max={:?}",
        dynasty_id, eclipse_type, year_min, year_max
    );
    let mut sql = String::from(
        "SELECT e.*, d.name_cn AS dynasty_name
         FROM solar_eclipse_records e
         LEFT JOIN dynasties d ON e.dynasty_id = d.id
         WHERE 1=1"
    );
    let mut psql: Vec<Box<dyn ToSql + Sync>> = Vec::new();
    let mut idx: i32 = 1;

    if let Some(v) = dynasty_id {
        sql.push_str(&format!(" AND e.dynasty_id = ${}", idx));
        idx += 1;
        psql.push(Box::new(v));
    }
    if let Some(v) = eclipse_type {
        sql.push_str(&format!(" AND e.eclipse_type = ${}", idx));
        idx += 1;
        psql.push(Box::new(v.to_string()));
    }
    if let Some(v) = year_min {
        sql.push_str(&format!(" AND e.year_ce >= ${}", idx));
        idx += 1;
        psql.push(Box::new(v));
    }
    if let Some(v) = year_max {
        sql.push_str(&format!(" AND e.year_ce <= ${}", idx));
        idx += 1;
        psql.push(Box::new(v));
    }

    let psql_ref: Vec<&(dyn ToSql + Sync)> = psql.iter().map(|b| b.as_ref()).collect();

    let count_sql = format!("SELECT COUNT(*) FROM ({}) q", sql);
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let cnt_row = client.query_one(&count_sql, &psql_ref).await.map_err(|e| e.to_string())?;
    let count: i64 = cnt_row.get(0);

    sql.push_str(" ORDER BY e.year_ce, e.id");
    sql.push_str(&format!(" LIMIT ${}", idx));
    idx += 1;
    psql.push(Box::new(limit));
    sql.push_str(&format!(" OFFSET ${}", idx));
    psql.push(Box::new(offset));

    let psql_ref2: Vec<&(dyn ToSql + Sync)> = psql.iter().map(|b| b.as_ref()).collect();
    let rows = client.query(&sql, &psql_ref2).await
        .map_err(|e| { error!("DB: list_eclipse_records error: {}", e); e.to_string() })?;
    trace!("DB: list_eclipse_records returned {}/{} rows", rows.len(), count);
    Ok((rows.iter().map(row_to_eclipse).collect(), count))
}

pub async fn get_eclipse_record(pool: &DbPool, id: i64) -> Result<Option<EclipseRecord>, String> {
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let rows = client.query(
        "SELECT e.*, d.name_cn AS dynasty_name
         FROM solar_eclipse_records e
         LEFT JOIN dynasties d ON e.dynasty_id = d.id
         WHERE e.id = $1", &[&id]).await
        .map_err(|e| e.to_string())?;
    Ok(rows.first().map(row_to_eclipse))
}

pub async fn list_instruments(pool: &DbPool) -> Result<Vec<AncientInstrument>, String> {
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let rows = client.query(
        "SELECT i.*, d.name_cn AS dynasty_name FROM ancient_instruments i
         LEFT JOIN dynasties d ON i.dynasty_id = d.id
         ORDER BY i.erected_year, i.id", &[]).await
        .map_err(|e| e.to_string())?;
    Ok(rows.iter().map(row_to_instrument).collect())
}

pub async fn get_instrument(pool: &DbPool, id: i64) -> Result<Option<AncientInstrument>, String> {
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let rows = client.query(
        "SELECT i.*, d.name_cn AS dynasty_name FROM ancient_instruments i
         LEFT JOIN dynasties d ON i.dynasty_id = d.id
         WHERE i.id = $1", &[&id]).await
        .map_err(|e| e.to_string())?;
    Ok(rows.first().map(row_to_instrument))
}

pub async fn list_instrument_observations(pool: &DbPool, instrument_id: i64)
    -> Result<Vec<InstrumentObservation>, String>
{
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let rows = client.query(
        "SELECT * FROM instrument_observations
         WHERE instrument_id = $1
         ORDER BY observation_year_ce, id", &[&instrument_id]).await
        .map_err(|e| e.to_string())?;
    Ok(rows.iter().map(row_to_instrument_observation).collect())
}

pub async fn list_variable_stars(
    pool: &DbPool,
    gcvs_type: Option<&str>,
    min_amplitude: Option<f64>,
    max_period: Option<f64>,
    search_name: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<VariableStarMeta>, i64), String> {
    trace!(
        "DB: list_variable_stars type={:?} min_amp={:?} max_period={:?} name={:?}",
        gcvs_type, min_amplitude, max_period, search_name
    );
    let mut sql = String::from(
        "SELECT * FROM variable_stars WHERE 1=1"
    );
    let mut psql: Vec<Box<dyn ToSql + Sync>> = Vec::new();
    let mut idx: i32 = 1;

    if let Some(v) = gcvs_type {
        sql.push_str(&format!(" AND gcvs_variable_type ILIKE ${}", idx));
        idx += 1;
        psql.push(Box::new(v.to_string()));
    }
    if let Some(v) = min_amplitude {
        sql.push_str(&format!(" AND (max_mag_v - min_mag_v) >= ${}", idx));
        idx += 1;
        psql.push(Box::new(v));
    }
    if let Some(v) = max_period {
        sql.push_str(&format!(" AND published_period_days <= ${}", idx));
        idx += 1;
        psql.push(Box::new(v));
    }
    if let Some(v) = search_name {
        sql.push_str(&format!(" AND (modern_name ILIKE ${} OR ancient_names_json->>'name_cn' ILIKE ${})", idx, idx));
        idx += 1;
        psql.push(Box::new(format!("%{}%", v)));
    }

    let psql_ref: Vec<&(dyn ToSql + Sync)> = psql.iter().map(|b| b.as_ref()).collect();

    let count_sql = format!("SELECT COUNT(*) FROM ({}) q", sql);
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let cnt_row = client.query_one(&count_sql, &psql_ref).await.map_err(|e| e.to_string())?;
    let count: i64 = cnt_row.get(0);

    sql.push_str(" ORDER BY mean_mag_v NULLS LAST, id");
    sql.push_str(&format!(" LIMIT ${}", idx));
    idx += 1;
    psql.push(Box::new(limit));
    sql.push_str(&format!(" OFFSET ${}", idx));
    psql.push(Box::new(offset));

    let psql_ref2: Vec<&(dyn ToSql + Sync)> = psql.iter().map(|b| b.as_ref()).collect();
    let rows = client.query(&sql, &psql_ref2).await
        .map_err(|e| { error!("DB: list_variable_stars error: {}", e); e.to_string() })?;
    trace!("DB: list_variable_stars returned {}/{} rows", rows.len(), count);
    Ok((rows.iter().map(row_to_variable).collect(), count))
}

pub async fn get_variable_star(pool: &DbPool, id: i64) -> Result<Option<VariableStarMeta>, String> {
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let rows = client.query(
        "SELECT * FROM variable_stars WHERE id = $1", &[&id]).await
        .map_err(|e| e.to_string())?;
    Ok(rows.first().map(row_to_variable))
}

pub async fn list_magnitude_measurements(pool: &DbPool, variable_id: i64)
    -> Result<Vec<MagnitudeMeasurement>, String>
{
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let rows = client.query(
        "SELECT * FROM magnitude_measurements
         WHERE variable_id = $1
         ORDER BY epoch_yr, id", &[&variable_id]).await
        .map_err(|e| e.to_string())?;
    Ok(rows.iter().map(row_to_magnitude).collect())
}
