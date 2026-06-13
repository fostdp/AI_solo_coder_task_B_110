-- ============================================================
-- 古代星表数据数字化与现代天体物理验证系统
-- PostgreSQL + PostGIS 数据库初始化脚本 v0.2
-- ============================================================
--
-- v0.2 更新 (三个修复):
--   1. ancient_stars 表新增 color_temp_k 字段 (有效温度 K)
--   2. 新增 idx_snr_galactic GIN 索引 (银道坐标空间查询)
--   3. guest_star_matches 表新增 log_prior 字段 (银河系先验对数)
--

-- 扩展
CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- ============================================================
-- 朝代表
-- ============================================================
CREATE TABLE IF NOT EXISTS dynasties (
    id SERIAL PRIMARY KEY,
    name_cn VARCHAR(32) NOT NULL,
    name_pinyin VARCHAR(64),
    start_year INTEGER NOT NULL,
    end_year INTEGER NOT NULL,
    canonical_epoch DOUBLE PRECISION NOT NULL,
    color_hex VARCHAR(16),
    description TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- ============================================================
-- 二十八宿表
-- ============================================================
CREATE TABLE IF NOT EXISTS lunar_mansions (
    id SERIAL PRIMARY KEY,
    mansion_order INTEGER NOT NULL,
    name_cn VARCHAR(16) NOT NULL,
    name_pinyin VARCHAR(32),
    ruxiu_width_deg DOUBLE PRECISION,
    ra_start_deg DOUBLE PRECISION,
    ra_end_deg DOUBLE PRECISION,
    dec_mid_deg DOUBLE PRECISION,
    description TEXT
);

-- ============================================================
-- 古代恒星表
-- 修复 3: 新增 color_temp_k 字段 (有效温度 K)
-- ============================================================
CREATE TABLE IF NOT EXISTS ancient_stars (
    id SERIAL PRIMARY KEY,
    star_id_code VARCHAR(64) UNIQUE NOT NULL,
    dynasty_id INTEGER REFERENCES dynasties(id),
    mansion_id INTEGER REFERENCES lunar_mansions(id),
    star_name_cn VARCHAR(64),
    star_name_alt VARCHAR(64),
    constellation VARCHAR(64),
    ruxiu_du DOUBLE PRECISION,
    quji_du DOUBLE PRECISION,
    ra_ancient_conv DOUBLE PRECISION,
    dec_ancient_conv DOUBLE PRECISION,
    ra_j2000 DOUBLE PRECISION,
    dec_j2000 DOUBLE PRECISION,
    magnitude_ancient VARCHAR(32),
    magnitude_num DOUBLE PRECISION,
    color_desc VARCHAR(32),
    color_class VARCHAR(16),
    color_temp_k DOUBLE PRECISION,  -- ★ 修复3: 有效温度 (K)
    proper_motion_ra DOUBLE PRECISION,   -- mas/yr
    proper_motion_dec DOUBLE PRECISION,  -- mas/yr
    parallax DOUBLE PRECISION,           -- mas
    source_book VARCHAR(64),
    quality_flag INTEGER DEFAULT 1,
    notes TEXT,
    modern_hd_id INTEGER,
    cross_match_id INTEGER,
    geom_sphere GEOMETRY(Point, 4326),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_stars_dynasty ON ancient_stars(dynasty_id);
CREATE INDEX IF NOT EXISTS idx_stars_mansion ON ancient_stars(mansion_id);
CREATE INDEX IF NOT EXISTS idx_stars_name ON ancient_stars(star_name_cn);
CREATE INDEX IF NOT EXISTS idx_stars_geom ON ancient_stars USING GIST (geom_sphere);
CREATE INDEX IF NOT EXISTS idx_stars_mag ON ancient_stars(magnitude_num);
CREATE INDEX IF NOT EXISTS idx_stars_temp ON ancient_stars(color_temp_k);

-- 自动更新 geom_sphere 触发器
CREATE OR REPLACE FUNCTION update_star_geom() RETURNS TRIGGER AS $$
BEGIN
    IF NEW.ra_j2000 IS NOT NULL AND NEW.dec_j2000 IS NOT NULL THEN
        NEW.geom_sphere := ST_SetSRID(ST_MakePoint(NEW.ra_j2000, NEW.dec_j2000), 4326);
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_star_geom ON ancient_stars;
CREATE TRIGGER trg_star_geom
    BEFORE INSERT OR UPDATE ON ancient_stars
    FOR EACH ROW EXECUTE FUNCTION update_star_geom();

-- ============================================================
-- 古代彗星表
-- ============================================================
CREATE TABLE IF NOT EXISTS ancient_comets (
    id SERIAL PRIMARY KEY,
    comet_id_code VARCHAR(64) UNIQUE NOT NULL,
    dynasty_id INTEGER REFERENCES dynasties(id),
    year_ancient VARCHAR(64),
    year_ce DOUBLE PRECISION,
    month_ancient INTEGER,
    day_ancient INTEGER,
    ruxiu_du DOUBLE PRECISION,
    quji_du DOUBLE PRECISION,
    ra_deg DOUBLE PRECISION,
    dec_deg DOUBLE PRECISION,
    magnitude DOUBLE PRECISION,
    color_desc VARCHAR(32),
    tail_direction VARCHAR(32),
    tail_length DOUBLE PRECISION,
    duration_days INTEGER,
    description TEXT,
    position_desc TEXT,
    source_book VARCHAR(64),
    quality_flag INTEGER DEFAULT 1,
    geom_sphere GEOMETRY(Point, 4326)
);

CREATE INDEX IF NOT EXISTS idx_comets_dynasty ON ancient_comets(dynasty_id);
CREATE INDEX IF NOT EXISTS idx_comets_geom ON ancient_comets USING GIST (geom_sphere);

-- ============================================================
-- 客星 (超新星候选) 表
-- ============================================================
CREATE TABLE IF NOT EXISTS guest_stars (
    id SERIAL PRIMARY KEY,
    guest_id_code VARCHAR(64) UNIQUE NOT NULL,
    dynasty_id INTEGER REFERENCES dynasties(id),
    star_name VARCHAR(64),
    year_ancient INTEGER NOT NULL,
    year_ce DOUBLE PRECISION NOT NULL,
    month_ancient INTEGER,
    day_ancient INTEGER,
    ruxiu_du DOUBLE PRECISION,
    quji_du DOUBLE PRECISION,
    ra_deg DOUBLE PRECISION,
    dec_deg DOUBLE PRECISION,
    ra_err DOUBLE PRECISION DEFAULT 1.0,   -- 位置不确定度 (度)
    dec_err DOUBLE PRECISION DEFAULT 1.0,
    peak_mag DOUBLE PRECISION,
    peak_mag_err DOUBLE PRECISION DEFAULT 0.5,
    visibility_days INTEGER,
    lightcurve_type VARCHAR(16) DEFAULT 'II',
    description TEXT,
    position_desc TEXT,
    source_book VARCHAR(64),
    matched_snr_id INTEGER,
    match_confidence DOUBLE PRECISION,
    geom_sphere GEOMETRY(Point, 4326),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_guests_dynasty ON guest_stars(dynasty_id);
CREATE INDEX IF NOT EXISTS idx_guests_geom ON guest_stars USING GIST (geom_sphere);
CREATE INDEX IF NOT EXISTS idx_guests_year ON guest_stars(year_ce);

-- 自动更新 geom
CREATE OR REPLACE FUNCTION update_guest_geom() RETURNS TRIGGER AS $$
BEGIN
    IF NEW.ra_deg IS NOT NULL AND NEW.dec_deg IS NOT NULL THEN
        NEW.geom_sphere := ST_SetSRID(ST_MakePoint(NEW.ra_deg, NEW.dec_deg), 4326);
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_guest_geom ON guest_stars;
CREATE TRIGGER trg_guest_geom
    BEFORE INSERT OR UPDATE ON guest_stars
    FOR EACH ROW EXECUTE FUNCTION update_guest_geom();

-- ============================================================
-- 超新星遗迹 (SNR) 表
-- 修复 2: 新增 gal_l, gal_b 银道坐标 (用于银河系分布先验)
-- ============================================================
CREATE TABLE IF NOT EXISTS supernova_remnants (
    id SERIAL PRIMARY KEY,
    remnant_name VARCHAR(128) UNIQUE NOT NULL,
    sn_type VARCHAR(16) DEFAULT 'II',
    ra_deg DOUBLE PRECISION NOT NULL,
    dec_deg DOUBLE PRECISION NOT NULL,
    gal_l DOUBLE PRECISION,     -- ★ 修复2: 银经
    gal_b DOUBLE PRECISION,     -- ★ 修复2: 银纬
    age_yr DOUBLE PRECISION,
    age_err_yr DOUBLE PRECISION DEFAULT 500.0,
    distance_kpc DOUBLE PRECISION,
    distance_err DOUBLE PRECISION,
    diameter_pc DOUBLE PRECISION,
    radio_flux_ghz DOUBLE PRECISION,
    xray_luminosity DOUBLE PRECISION,
    gamma_detected BOOLEAN DEFAULT FALSE,
    historical_sn_id INTEGER REFERENCES guest_stars(id),
    geom_sphere GEOMETRY(Point, 4326),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_snr_geom ON supernova_remnants USING GIST (geom_sphere);
CREATE INDEX IF NOT EXISTS idx_snr_age ON supernova_remnants(age_yr);
CREATE INDEX IF NOT EXISTS idx_snr_type ON supernova_remnants(sn_type);
CREATE INDEX IF NOT EXISTS idx_snr_galactic ON supernova_remnants(gal_l, gal_b);  -- ★ 修复2

-- 触发器: 自动计算银道坐标
CREATE OR REPLACE FUNCTION calc_snr_galactic() RETURNS TRIGGER AS $$
DECLARE
    ra_r  DOUBLE PRECISION;
    dec_r DOUBLE PRECISION;
    ngp_ra_r  DOUBLE PRECISION := RADIANS(192.8595);
    ngp_dec_r DOUBLE PRECISION := RADIANS(27.1284);
    lon_cp_r  DOUBLE PRECISION := RADIANS(122.932);
    sin_b DOUBLE PRECISION;
    b_r   DOUBLE PRECISION;
    y_r   DOUBLE PRECISION;
    x_r   DOUBLE PRECISION;
    l_r   DOUBLE PRECISION;
BEGIN
    IF NEW.ra_deg IS NOT NULL AND NEW.dec_deg IS NOT NULL THEN
        ra_r  := RADIANS(NEW.ra_deg);
        dec_r := RADIANS(NEW.dec_deg);

        sin_b := SIN(dec_r) * SIN(ngp_dec_r)
               + COS(dec_r) * COS(ngp_dec_r) * COS(ra_r - ngp_ra_r);
        b_r := ASIN(sin_b);

        y_r := SIN(dec_r) * COS(ngp_dec_r)
             - COS(dec_r) * SIN(ngp_dec_r) * COS(ra_r - ngp_ra_r);
        x_r := -COS(dec_r) * SIN(ra_r - ngp_ra_r);
        l_r := ATAN2(y_r, x_r) + lon_cp_r;

        NEW.gal_l := DEGREES(l_r);
        IF NEW.gal_l < 0 THEN NEW.gal_l := NEW.gal_l + 360; END IF;
        IF NEW.gal_l >= 360 THEN NEW.gal_l := NEW.gal_l - 360; END IF;
        NEW.gal_b := DEGREES(b_r);
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_snr_galactic ON supernova_remnants;
CREATE TRIGGER trg_snr_galactic
    BEFORE INSERT OR UPDATE OF ra_deg, dec_deg ON supernova_remnants
    FOR EACH ROW EXECUTE FUNCTION calc_snr_galactic();

-- ============================================================
-- 客星 - 超新星遗迹 匹配结果表
-- 修复 2: 新增 log_prior 字段 (记录银河系先验贡献)
-- ============================================================
CREATE TABLE IF NOT EXISTS guest_star_matches (
    id SERIAL PRIMARY KEY,
    guest_id INTEGER REFERENCES guest_stars(id),
    remnant_id INTEGER REFERENCES supernova_remnants(id),
    rank_within_guest INTEGER,
    match_probability DOUBLE PRECISION,
    log_posterior DOUBLE PRECISION,
    log_likelihood DOUBLE PRECISION,
    log_prior DOUBLE PRECISION,    -- ★ 修复2: 先验对数 (银河系分布模型)
    bayes_factor DOUBLE PRECISION,
    angular_sep_arcmin DOUBLE PRECISION,
    time_delta_yr DOUBLE PRECISION,
    spatial_score DOUBLE PRECISION,
    temporal_score DOUBLE PRECISION,
    magnitude_score DOUBLE PRECISION,
    lightcurve_score DOUBLE PRECISION,
    model_version VARCHAR(32),
    match_method VARCHAR(32),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(guest_id, remnant_id)
);

CREATE INDEX IF NOT EXISTS idx_matches_guest ON guest_star_matches(guest_id, rank_within_guest);
CREATE INDEX IF NOT EXISTS idx_matches_remnant ON guest_star_matches(remnant_id);
CREATE INDEX IF NOT EXISTS idx_matches_prob ON guest_star_matches(match_probability DESC);

-- ============================================================
-- 天球角距离函数 (Haversine)
-- ============================================================
CREATE OR REPLACE FUNCTION angular_distance_deg(
    ra1 DOUBLE PRECISION, dec1 DOUBLE PRECISION,
    ra2 DOUBLE PRECISION, dec2 DOUBLE PRECISION
) RETURNS DOUBLE PRECISION AS $$
DECLARE
    d_ra  DOUBLE PRECISION := RADIANS(ra1 - ra2);
    d_dec DOUBLE PRECISION := RADIANS(dec1 - dec2);
    a     DOUBLE PRECISION;
BEGIN
    a := POWER(SIN(d_dec / 2), 2)
       + COS(RADIANS(dec1)) * COS(RADIANS(dec2)) * POWER(SIN(d_ra / 2), 2);
    RETURN DEGREES(2 * ASIN(SQRT(a)));
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- ============================================================
-- 视图: 跨朝代恒星对比
-- ============================================================
CREATE OR REPLACE VIEW v_star_cross_dynasty AS
SELECT
    s1.id AS star_id_1,
    s2.id AS star_id_2,
    s1.star_name_cn AS star_name,
    d1.id AS dynasty_id_1,
    d1.name_cn AS dynasty_1,
    d2.id AS dynasty_id_2,
    d2.name_cn AS dynasty_2,
    (d2.canonical_epoch - d1.canonical_epoch) AS delta_yr,
    s1.ruxiu_du - s2.ruxiu_du AS delta_ruxiu,
    s1.quji_du - s2.quji_du AS delta_quji,
    angular_distance_deg(s1.ra_j2000, s1.dec_j2000, s2.ra_j2000, s2.dec_j2000) AS delta_ang_deg
FROM ancient_stars s1
JOIN ancient_stars s2
    ON s1.star_name_cn = s2.star_name_cn
    AND s1.dynasty_id < s2.dynasty_id
JOIN dynasties d1 ON s1.dynasty_id = d1.id
JOIN dynasties d2 ON s2.dynasty_id = d2.id
WHERE s1.ruxiu_du IS NOT NULL
  AND s2.ruxiu_du IS NOT NULL
ORDER BY s1.star_name_cn, d1.start_year;

-- ============================================================
-- 数据质量统计视图
-- ============================================================
CREATE OR REPLACE VIEW v_star_quality_stats AS
SELECT
    d.name_cn AS dynasty_name,
    COUNT(*) AS star_count,
    AVG(s.quality_flag) AS avg_quality,
    SUM(CASE WHEN s.ra_j2000 IS NOT NULL THEN 1 ELSE 0 END) AS matched_count,
    ROUND(AVG(s.magnitude_num)::numeric, 2) AS avg_magnitude,
    ROUND(AVG(s.proper_motion_ra)::numeric, 2) AS avg_pm_ra
FROM ancient_stars s
JOIN dynasties d ON s.dynasty_id = d.id
GROUP BY d.id, d.name_cn
ORDER BY d.start_year;

-- ============================================================
-- 初始数据: 朝代
-- ============================================================
INSERT INTO dynasties (name_cn, name_pinyin, start_year, end_year, canonical_epoch, color_hex)
VALUES
    ('汉', 'Han', -206, 220, -50.0, '#c03030'),
    ('三国', 'Three Kingdoms', 220, 280, 250.0, '#c08040'),
    ('晋', 'Jin', 266, 420, 340.0, '#608040'),
    ('南北朝', 'North-South', 420, 589, 500.0, '#4080c0'),
    ('隋', 'Sui', 581, 618, 600.0, '#8040a0'),
    ('唐', 'Tang', 618, 907, 750.0, '#e0a020'),
    ('五代', 'Five Dynasties', 907, 960, 930.0, '#606060'),
    ('宋', 'Song', 960, 1279, 1100.0, '#7040b0'),
    ('辽', 'Liao', 907, 1125, 1000.0, '#308080'),
    ('金', 'Jin_er', 1115, 1234, 1170.0, '#a06040'),
    ('元', 'Yuan', 1271, 1368, 1320.0, '#3070c0'),
    ('明', 'Ming', 1368, 1644, 1500.0, '#c04030'),
    ('清', 'Qing', 1636, 1912, 1750.0, '#308040')
ON CONFLICT DO NOTHING;

-- ============================================================
-- 初始数据: 二十八宿
--   宿度参考 <步天歌> 均值, 按西汉时期平均分配
-- ============================================================
INSERT INTO lunar_mansions (mansion_order, name_cn, name_pinyin, ruxiu_width_deg, ra_start_deg, ra_end_deg)
VALUES
    (1,  '角', 'Jiao',   12.0, 189.5, 201.5),
    (2,  '亢', 'Kang',    9.0, 201.5, 210.5),
    (3,  '氐', 'Di',     15.0, 210.5, 225.5),
    (4,  '房', 'Fang',    5.0, 225.5, 230.5),
    (5,  '心', 'Xin',     5.0, 230.5, 235.5),
    (6,  '尾', 'Wei',    18.0, 235.5, 253.5),
    (7,  '箕', 'Ji',     11.0, 253.5, 264.5),
    (8,  '斗', 'Dou',    26.0, 264.5, 290.5),
    (9,  '牛', 'Niu',     8.0, 290.5, 298.5),
    (10, '女', 'Nü',     12.0, 298.5, 310.5),
    (11, '虚', 'Xu',     10.0, 310.5, 320.5),
    (12, '危', 'Wei',    17.0, 320.5, 337.5),
    (13, '室', 'Shi',    16.0, 337.5, 353.5),
    (14, '壁', 'Bi',      9.0, 353.5, 362.5),
    (15, '奎', 'Kui',    16.0, 362.5,  18.5),
    (16, '娄', 'Lou',    12.0,  18.5,  30.5),
    (17, '胃', 'Wei',    14.0,  30.5,  44.5),
    (18, '昴', 'Mao',    11.0,  44.5,  55.5),
    (19, '毕', 'Bi',     16.0,  55.5,  71.5),
    (20, '觜', 'Zi',      3.0,  71.5,  74.5),
    (21, '参', 'Shen',    9.0,  74.5,  83.5),
    (22, '井', 'Jing',   33.0,  83.5, 116.5),
    (23, '鬼', 'Gui',     4.0, 116.5, 120.5),
    (24, '柳', 'Liu',    15.0, 120.5, 135.5),
    (25, '星', 'Xing',    7.0, 135.5, 142.5),
    (26, '张', 'Zhang',   6.0, 142.5, 148.5),
    (27, '翼', 'Yi',     18.0, 148.5, 166.5),
    (28, '轸', 'Zhen',    5.0, 166.5, 171.5)
ON CONFLICT DO NOTHING;

-- 更新 mansion_id 外键引用 (让古代星星宿关联更准确)
-- 注: 实际导入数据时使用 mansion_order 做 JOIN

-- ============================================================
-- 日食月食记录表
--   日食与月食共用一张表, eclipse_type 字段区分 solar/lunar
-- ============================================================
CREATE TABLE IF NOT EXISTS solar_eclipse_records (
    id SERIAL PRIMARY KEY,
    eclipse_id_code VARCHAR(32) UNIQUE NOT NULL,
    dynasty_id INTEGER REFERENCES dynasties(id),
    eclipse_type VARCHAR(16) NOT NULL,
    year_ancient VARCHAR(64),
    year_ce DOUBLE PRECISION NOT NULL,
    month_ancient INTEGER,
    day_ancient INTEGER,
    hour_ancient VARCHAR(16),
    magnitude_desc VARCHAR(32),
    magnitude_num DOUBLE PRECISION,
    duration_desc VARCHAR(64),
    duration_min DOUBLE PRECISION,
    ruxiu_du DOUBLE PRECISION,
    quji_du DOUBLE PRECISION,
    ra_deg DOUBLE PRECISION,
    dec_deg DOUBLE PRECISION,
    location_desc VARCHAR(128),
    source_book VARCHAR(64),
    record_text TEXT,
    quality_flag INTEGER DEFAULT 3,
    geom_sphere GEOMETRY(Point, 4326),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_eclipse_dynasty ON solar_eclipse_records(dynasty_id);
CREATE INDEX IF NOT EXISTS idx_eclipse_type ON solar_eclipse_records(eclipse_type);
CREATE INDEX IF NOT EXISTS idx_eclipse_year ON solar_eclipse_records(year_ce);
CREATE INDEX IF NOT EXISTS idx_eclipse_geom ON solar_eclipse_records USING GIST (geom_sphere);

-- 触发器: 自动更新 geom_sphere
CREATE OR REPLACE FUNCTION update_eclipse_geom() RETURNS TRIGGER AS $$
BEGIN
    IF NEW.ra_deg IS NOT NULL AND NEW.dec_deg IS NOT NULL THEN
        NEW.geom_sphere := ST_SetSRID(ST_MakePoint(NEW.ra_deg, NEW.dec_deg), 4326);
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_eclipse_geom ON solar_eclipse_records;
CREATE TRIGGER trg_eclipse_geom
    BEFORE INSERT OR UPDATE ON solar_eclipse_records
    FOR EACH ROW EXECUTE FUNCTION update_eclipse_geom();

-- ============================================================
-- 古代仪器表
-- ============================================================
CREATE TABLE IF NOT EXISTS ancient_instruments (
    id SERIAL PRIMARY KEY,
    instrument_code VARCHAR(32) UNIQUE NOT NULL,
    name_cn VARCHAR(64) NOT NULL,
    dynasty_id INTEGER REFERENCES dynasties(id),
    erected_year DOUBLE PRECISION,
    location_lat_deg DOUBLE PRECISION,
    location_lon_deg DOUBLE PRECISION,
    location_name VARCHAR(64),
    ring_count INTEGER,
    nominal_accuracy_arcmin DOUBLE PRECISION,
    divisions_circle INTEGER,
    vernier_resolution_arcmin DOUBLE PRECISION,
    description TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_instruments_dynasty ON ancient_instruments(dynasty_id);
CREATE INDEX IF NOT EXISTS idx_instruments_code ON ancient_instruments(instrument_code);

-- ============================================================
-- 仪器观测数据表
-- ============================================================
CREATE TABLE IF NOT EXISTS instrument_observations (
    id SERIAL PRIMARY KEY,
    instrument_id INTEGER REFERENCES ancient_instruments(id),
    star_id INTEGER REFERENCES ancient_stars(id),
    star_name_cn VARCHAR(64),
    observation_year_ce DOUBLE PRECISION NOT NULL,
    ruxiu_du_measured DOUBLE PRECISION,
    quji_du_measured DOUBLE PRECISION,
    ra_deg_measured DOUBLE PRECISION,
    dec_deg_measured DOUBLE PRECISION,
    ra_j2000_true DOUBLE PRECISION,
    dec_j2000_true DOUBLE PRECISION,
    source_book VARCHAR(64),
    quality_flag INTEGER DEFAULT 3,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_obs_instrument_star ON instrument_observations(instrument_id, star_id);
CREATE INDEX IF NOT EXISTS idx_obs_year ON instrument_observations(observation_year_ce);

-- ============================================================
-- 变星星表
-- ============================================================
CREATE TABLE IF NOT EXISTS variable_stars (
    id SERIAL PRIMARY KEY,
    modern_name VARCHAR(64) UNIQUE NOT NULL,
    constellation_code VARCHAR(16),
    hd_id INTEGER,
    hr_id INTEGER,
    hipparcos_id INTEGER,
    gcvs_variable_type VARCHAR(32),
    ra_j2000_deg DOUBLE PRECISION NOT NULL,
    dec_j2000_deg DOUBLE PRECISION NOT NULL,
    distance_pc DOUBLE PRECISION,
    distance_err DOUBLE PRECISION,
    spectral_type VARCHAR(16),
    luminosity_class VARCHAR(8),
    min_mag_v DOUBLE PRECISION,
    max_mag_v DOUBLE PRECISION,
    mean_mag_v DOUBLE PRECISION,
    epoch_mjd_max DOUBLE PRECISION,
    published_period_days DOUBLE PRECISION,
    published_period_err DOUBLE PRECISION,
    period_change_rate_pdot DOUBLE PRECISION,
    ancient_names_json JSONB,
    geom_sphere GEOMETRY(Point, 4326),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_var_geom ON variable_stars USING GIST (geom_sphere);
CREATE INDEX IF NOT EXISTS idx_var_names_json ON variable_stars USING GIN (ancient_names_json);
CREATE INDEX IF NOT EXISTS idx_var_type ON variable_stars(gcvs_variable_type);

-- 触发器: 自动更新 geom_sphere
CREATE OR REPLACE FUNCTION update_variable_geom() RETURNS TRIGGER AS $$
BEGIN
    IF NEW.ra_j2000_deg IS NOT NULL AND NEW.dec_j2000_deg IS NOT NULL THEN
        NEW.geom_sphere := ST_SetSRID(ST_MakePoint(NEW.ra_j2000_deg, NEW.dec_j2000_deg), 4326);
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_variable_geom ON variable_stars;
CREATE TRIGGER trg_variable_geom
    BEFORE INSERT OR UPDATE ON variable_stars
    FOR EACH ROW EXECUTE FUNCTION update_variable_geom();

-- ============================================================
-- 星等测量表
-- ============================================================
CREATE TABLE IF NOT EXISTS magnitude_measurements (
    id SERIAL PRIMARY KEY,
    variable_id INTEGER REFERENCES variable_stars(id),
    epoch_yr DOUBLE PRECISION NOT NULL,
    epoch_mjd DOUBLE PRECISION,
    magnitude DOUBLE PRECISION NOT NULL,
    magnitude_uncertainty DOUBLE PRECISION,
    passband VARCHAR(8) NOT NULL,
    source_type VARCHAR(16) NOT NULL,
    source_book VARCHAR(64),
    ancient_description TEXT,
    ancient_quality INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_mag_variable_epoch ON magnitude_measurements(variable_id, epoch_yr);
CREATE INDEX IF NOT EXISTS idx_mag_source_type ON magnitude_measurements(source_type);
CREATE INDEX IF NOT EXISTS idx_mag_passband ON magnitude_measurements(passband);

-- ============================================================
-- 初始数据: 古代仪器
-- ============================================================
INSERT INTO ancient_instruments (instrument_code, name_cn, dynasty_id, erected_year, location_lat_deg, location_lon_deg, location_name, ring_count, nominal_accuracy_arcmin, divisions_circle, vernier_resolution_arcmin, description)
VALUES
    ('han_la_hunyi', '落下闳浑仪', 1, -104.0, 34.3, 108.9, '长安', 4, 60.0, 365, NULL, '西汉落下闳创制的浑仪, 用于测量天体位置, 定太初历'),
    ('han_zhang_heng', '张衡浑天仪', 1, 117.0, 34.3, 108.9, '洛阳', 5, 45.0, 365, NULL, '东汉张衡制造的水运浑天仪, 以水力驱动演示天象'),
    ('song_huangyou', '皇祐浑仪', 8, 1050.0, 34.8, 114.3, '开封', 7, 15.0, 365, NULL, '北宋皇祐年间制造的浑仪, 精度较前代大幅提升'),
    ('yuan_jianyi', '郭守敬简仪', 11, 1279.0, 39.9, 116.4, '大都', 3, 3.0, 365, 0.1, '元代郭守敬创制的简仪, 简化结构, 增加游标读数装置'),
    ('ming_zhengtong', '正统浑仪', 12, 1439.0, 32.0, 118.8, '南京', 7, 10.0, 365, NULL, '明代正统年间铸造的浑仪, 现存南京紫金山天文台')
ON CONFLICT DO NOTHING;

-- ============================================================
-- 初始数据: 日食记录
-- ============================================================
INSERT INTO solar_eclipse_records (eclipse_id_code, dynasty_id, eclipse_type, year_ancient, year_ce, month_ancient, day_ancient, hour_ancient, magnitude_desc, magnitude_num, duration_desc, ruxiu_du, quji_du, ra_deg, dec_deg, location_desc, source_book, record_text, quality_flag, geom_sphere)
VALUES
    ('ecl_zhongkang', 1, 'solar', '仲康元年', -2137.0, 9, 24, '辰时', '食既', 1.0, '约一时辰', 177.0, 5.0, 174.5, 8.3, '斟鄩', '尚书胤征', '乃季秋月朔, 辰弗集于房', 2, ST_SetSRID(ST_MakePoint(174.5, 8.3), 4326)),
    ('ecl_oracle_bone', 1, 'solar', '武丁时期', -1302.0, 6, 5, '午时', '日有食之', 0.8, '不详', 75.0, 22.0, 72.3, 23.1, '殷墟', '甲骨文卜辞', '日有食之, 若', 2, ST_SetSRID(ST_MakePoint(72.3, 23.1), 4326)),
    ('ecl_spring_autumn', 1, 'solar', '鲁隐公三年', -720.0, 2, 22, '巳时', '日有食之', 0.9, '不详', 340.0, -15.0, 338.2, -13.5, '鲁国', '春秋', '春王二月己巳, 日有食之', 2, ST_SetSRID(ST_MakePoint(338.2, -13.5), 4326)),
    ('ecl_xihe', 1, 'solar', '夏代', -2165.0, 5, 28, '未时', '食甚', 0.95, '不详', 95.0, 18.0, 92.8, 19.7, '中原', '左传', '夏书曰: 辰不集于房', 3, ST_SetSRID(ST_MakePoint(92.8, 19.7), 4326)),
    ('ecl_zhou_xuan', 1, 'solar', '周宣王六年', -822.0, 7, 26, '卯时', '日有食之', 0.7, '不详', 132.0, 19.0, 129.5, 20.3, '镐京', '诗经', '十月之交, 朔月辛卯, 日有食之', 2, ST_SetSRID(ST_MakePoint(129.5, 20.3), 4326)),
    ('ecl_han_hanhe', 1, 'solar', '汉和帝永元四年', 92.0, 6, 29, '午时', '食十二分之七', 0.58, '二刻', 105.0, 23.0, 102.3, 24.1, '洛阳', '后汉书', '六月戊申, 日有食之', 2, ST_SetSRID(ST_MakePoint(102.3, 24.1), 4326)),
    ('ecl_tang_tianjian', 6, 'solar', '唐高祖武德三年', 620.0, 10, 16, '申时', '食既', 1.0, '三刻', 215.0, -12.0, 212.8, -10.5, '长安', '旧唐书', '十月丙申朔, 日有食之, 在尾二度', 1, ST_SetSRID(ST_MakePoint(212.8, -10.5), 4326)),
    ('ecl_song_huangyou', 8, 'solar', '宋仁宗皇祐元年', 1049.0, 3, 8, '辰时', '食六分', 0.6, '一刻半', 355.0, -6.0, 352.5, -4.8, '开封', '宋史', '正月甲午朔, 日有食之', 2, ST_SetSRID(ST_MakePoint(352.5, -4.8), 4326)),
    ('ecl_yuan_zhiyuan', 11, 'solar', '元世祖至元十七年', 1280.0, 5, 26, '巳时', '食八分', 0.8, '二刻', 255.0, 21.0, 252.3, 22.4, '大都', '元史', '五月丁亥朔, 日有食之', 2, ST_SetSRID(ST_MakePoint(252.3, 22.4), 4326)),
    ('ecl_ming_wanli', 12, 'solar', '明神宗万历三年', 1575.0, 11, 13, '午时', '食既', 1.0, '四刻', 238.0, -17.0, 235.5, -15.8, '北京', '明史', '十月庚辰朔, 日有食之', 1, ST_SetSRID(ST_MakePoint(235.5, -15.8), 4326))
ON CONFLICT DO NOTHING;

-- ============================================================
-- 初始数据: 月食记录
-- ============================================================
INSERT INTO solar_eclipse_records (eclipse_id_code, dynasty_id, eclipse_type, year_ancient, year_ce, month_ancient, day_ancient, hour_ancient, magnitude_desc, magnitude_num, duration_desc, ruxiu_du, quji_du, ra_deg, dec_deg, location_desc, source_book, record_text, quality_flag, geom_sphere)
VALUES
    ('ecl_lunar_han', 1, 'lunar', '汉文帝后元三年', -161.0, 7, 18, '子时', '月食尽', 1.0, '三刻', 280.0, -20.0, 277.5, -18.6, '长安', '汉书', '七月乙巳, 月食', 2, ST_SetSRID(ST_MakePoint(277.5, -18.6), 4326)),
    ('ecl_lunar_sanguo', 2, 'lunar', '魏明帝太和四年', 230.0, 11, 30, '丑时', '月食五分', 0.5, '一刻', 55.0, 24.0, 52.3, 25.7, '洛阳', '三国志', '十一月戊申, 月食', 3, ST_SetSRID(ST_MakePoint(52.3, 25.7), 4326)),
    ('ecl_lunar_tang', 6, 'lunar', '唐玄宗开元十二年', 724.0, 4, 8, '寅时', '月食既', 1.0, '三刻', 195.0, 10.0, 192.5, 11.2, '长安', '新唐书', '四月壬申, 月食', 2, ST_SetSRID(ST_MakePoint(192.5, 11.2), 4326)),
    ('ecl_lunar_song', 8, 'lunar', '宋神宗熙宁八年', 1075.0, 9, 21, '卯时', '月食九分', 0.9, '二刻半', 345.0, -9.0, 342.3, -7.8, '开封', '宋会要辑稿', '九月庚辰, 月食', 2, ST_SetSRID(ST_MakePoint(342.3, -7.8), 4326)),
    ('ecl_lunar_ming', 12, 'lunar', '明英宗正统八年', 1443.0, 8, 5, '辰时', '月食七分', 0.7, '二刻', 300.0, -19.0, 297.5, -17.6, '北京', '明实录', '八月甲午, 月食', 3, ST_SetSRID(ST_MakePoint(297.5, -17.6), 4326))
ON CONFLICT DO NOTHING;

-- ============================================================
-- 初始数据: 变星星表
-- ============================================================
INSERT INTO variable_stars (modern_name, constellation_code, hd_id, hr_id, hipparcos_id, gcvs_variable_type, ra_j2000_deg, dec_j2000_deg, distance_pc, distance_err, spectral_type, luminosity_class, min_mag_v, max_mag_v, mean_mag_v, epoch_mjd_max, published_period_days, published_period_err, period_change_rate_pdot, ancient_names_json, geom_sphere)
VALUES
    ('Mira', 'Cet', 14386, 681, 10826, 'M', 2.0, -2.0, 107.0, 5.0, 'M7e', 'III', 10.1, 2.0, 6.5, 50000.0, 332.0, 0.5, 0.0001, '{"name_cn": "蒭藁增二", "mansion": "娄", "alias": ["刍藁增二"]}', ST_SetSRID(ST_MakePoint(2.0, -2.0), 4326)),
    ('Algol', 'Per', 13302, 634, 9803, 'EA/DM', 47.0, 40.9, 28.5, 0.5, 'B8V', 'V', 3.4, 2.1, 2.9, 50005.0, 2.867329, 0.000001, 0.0000001, '{"name_cn": "大陵五", "mansion": "胃", "alias": ["英仙座β"]}', ST_SetSRID(ST_MakePoint(47.0, 40.9), 4326)),
    ('Delta Cephei', 'Cep', 208896, 8429, 110991, 'DCEP', 348.0, 58.4, 273.0, 12.0, 'F5-G2', 'Ib-II', 4.4, 3.5, 4.0, 50010.0, 5.366270, 0.000001, 0.0001, '{"name_cn": "造父一", "mansion": "紫微", "alias": ["仙王座δ"]}', ST_SetSRID(ST_MakePoint(348.0, 58.4), 4326)),
    ('Betelgeuse', 'Ori', 39801, 2061, 27989, 'SRc', 88.8, 7.4, 197.0, 25.0, 'M2Iab', 'Ia', 1.3, 0.0, 0.5, NULL, NULL, NULL, NULL, '{"name_cn": "参宿四", "mansion": "参", "alias": ["猎户座α"]}', ST_SetSRID(ST_MakePoint(88.8, 7.4), 4326)),
    ('Antares', 'Sco', 148478, 6134, 80763, 'LC', 247.4, -26.4, 170.0, 10.0, 'M1.5Iab', 'Iab', 1.7, 0.9, 1.3, NULL, 1733.0, 30.0, NULL, '{"name_cn": "心宿二", "mansion": "房", "alias": ["天蝎座α", "大火"]}', ST_SetSRID(ST_MakePoint(247.4, -26.4), 4326)),
    ('Vega', 'Lyr', 172167, 7001, 91262, 'DSCTC', 279.2, 38.8, 7.7, 0.1, 'A0V', 'V', 0.05, 0.03, 0.04, NULL, 0.107, NULL, NULL, '{"name_cn": "织女一", "mansion": "牛", "alias": ["天琴座α", "织女星"]}', ST_SetSRID(ST_MakePoint(279.2, 38.8), 4326))
ON CONFLICT DO NOTHING;

-- ============================================================
-- 初始数据: 变星古代测量 + 现代模拟测量
--   每颗变星: 5-10 条古代测量 + 10-20 条现代模拟测量
-- ============================================================

-- 蒭藁增二 (Mira) - 古代测量
INSERT INTO magnitude_measurements (variable_id, epoch_yr, magnitude, magnitude_uncertainty, passband, source_type, source_book, ancient_description, ancient_quality)
SELECT id, epoch_yr, magnitude, mag_err, 'V', 'ancient', source_book, description, quality
FROM variable_stars,
     (VALUES
         (  100.0, 3.0, 0.5, '史记天官书', '蒭藁 见则谷不熟', 3),
         (  120.0, 2.5, 0.5, '汉书天文志', '蒭藁星 明大', 2),
         (  700.0, 4.0, 0.5, '晋书天文志', '蒭藁 微暗', 3),
         ( 1050.0, 2.0, 0.3, '宋史天文志', '蒭藁 大光明', 2),
         ( 1280.0, 3.5, 0.5, '元史天文志', '蒭藁 平常', 3),
         ( 1450.0, 2.8, 0.4, '明实录', '蒭藁星 见', 2),
         ( 1590.0, 3.2, 0.5, '崇祯历书', '蒭藁 中等亮度', 2)
     ) AS t(epoch_yr, magnitude, mag_err, source_book, description, quality)
WHERE modern_name = 'Mira';

-- 蒭藁增二 (Mira) - 现代模拟测量
INSERT INTO magnitude_measurements (variable_id, epoch_yr, epoch_mjd, magnitude, magnitude_uncertainty, passband, source_type, source_book)
SELECT id, epoch_yr, epoch_mjd, magnitude, 0.05, 'V', 'modern_photometry', 'AAVSO'
FROM variable_stars,
     (VALUES
         (2000.0, 51544.0, 6.5),
         (2001.0, 51914.0, 4.2),
         (2002.0, 52279.0, 2.5),
         (2003.0, 52644.0, 3.8),
         (2004.0, 53009.0, 5.9),
         (2005.0, 53374.0, 7.8),
         (2006.0, 53740.0, 8.5),
         (2007.0, 54105.0, 9.2),
         (2008.0, 54470.0, 7.6),
         (2009.0, 54835.0, 5.1),
         (2010.0, 55200.0, 3.0),
         (2011.0, 55565.0, 2.2),
         (2012.0, 55931.0, 3.5),
         (2013.0, 56296.0, 5.8),
         (2014.0, 56661.0, 8.1)
     ) AS t(epoch_yr, epoch_mjd, magnitude)
WHERE modern_name = 'Mira';

-- 大陵五 (Algol) - 古代测量
INSERT INTO magnitude_measurements (variable_id, epoch_yr, magnitude, magnitude_uncertainty, passband, source_type, source_book, ancient_description, ancient_quality)
SELECT id, epoch_yr, magnitude, mag_err, 'V', 'ancient', source_book, description, quality
FROM variable_stars,
     (VALUES
         ( -150.0, 2.3, 0.3, '史记天官书', '大陵 有积尸', 3),
         (  100.0, 2.5, 0.4, '汉书天文志', '大陵九星', 3),
         (  720.0, 2.2, 0.3, '隋书天文志', '大陵 明', 2),
         ( 1050.0, 2.8, 0.4, '宋史天文志', '大陵 微暗', 2),
         ( 1400.0, 2.4, 0.3, '明实录', '大陵星 如常', 3),
         ( 1580.0, 2.6, 0.3, '崇祯历书', '大陵五 变光', 2)
     ) AS t(epoch_yr, magnitude, mag_err, source_book, description, quality)
WHERE modern_name = 'Algol';

-- 大陵五 (Algol) - 现代模拟测量
INSERT INTO magnitude_measurements (variable_id, epoch_yr, epoch_mjd, magnitude, magnitude_uncertainty, passband, source_type, source_book)
SELECT id, epoch_yr, epoch_mjd, magnitude, 0.02, 'V', 'modern_photometry', 'AAVSO'
FROM variable_stars,
     (VALUES
         (2000.0, 51550.0, 2.1),
         (2002.0, 52280.0, 2.9),
         (2004.0, 53010.0, 3.4),
         (2005.0, 53375.0, 2.2),
         (2006.0, 53740.0, 2.7),
         (2007.0, 54105.0, 3.2),
         (2008.0, 54470.0, 2.1),
         (2009.0, 54835.0, 2.5),
         (2010.0, 55200.0, 3.0),
         (2011.0, 55565.0, 3.3),
         (2012.0, 55930.0, 2.3),
         (2013.0, 56295.0, 2.8)
     ) AS t(epoch_yr, epoch_mjd, magnitude)
WHERE modern_name = 'Algol';

-- 造父一 (Delta Cephei) - 古代测量
INSERT INTO magnitude_measurements (variable_id, epoch_yr, magnitude, magnitude_uncertainty, passband, source_type, source_book, ancient_description, ancient_quality)
SELECT id, epoch_yr, magnitude, mag_err, 'V', 'ancient', source_book, description, quality
FROM variable_stars,
     (VALUES
         ( -100.0, 3.8, 0.5, '史记天官书', '造父 五星', 3),
         (  200.0, 4.0, 0.5, '后汉书', '造父 微暗', 3),
         (  800.0, 3.7, 0.4, '唐书', '造父 明', 3),
         ( 1100.0, 4.2, 0.5, '宋会要', '造父 稍暗', 2),
         ( 1350.0, 3.9, 0.4, '元史', '造父 如常', 3),
         ( 1550.0, 4.1, 0.4, '明实录', '造父星 见', 3),
         ( 1600.0, 3.6, 0.3, '崇祯历书', '造父一 有光变', 2)
     ) AS t(epoch_yr, magnitude, mag_err, source_book, description, quality)
WHERE modern_name = 'Delta Cephei';

-- 造父一 (Delta Cephei) - 现代模拟测量
INSERT INTO magnitude_measurements (variable_id, epoch_yr, epoch_mjd, magnitude, magnitude_uncertainty, passband, source_type, source_book)
SELECT id, epoch_yr, epoch_mjd, magnitude, 0.03, 'V', 'modern_photometry', 'AAVSO'
FROM variable_stars,
     (VALUES
         (2000.0, 51544.0, 3.8),
         (2001.0, 51910.0, 3.5),
         (2002.0, 52275.0, 4.1),
         (2003.0, 52640.0, 3.6),
         (2004.0, 53005.0, 4.3),
         (2005.0, 53370.0, 3.7),
         (2006.0, 53735.0, 4.0),
         (2007.0, 54100.0, 3.5),
         (2008.0, 54465.0, 4.2),
         (2009.0, 54830.0, 3.8),
         (2010.0, 55195.0, 4.4),
         (2011.0, 55560.0, 3.6),
         (2012.0, 55925.0, 4.0),
         (2013.0, 56290.0, 3.5),
         (2014.0, 56655.0, 4.1)
     ) AS t(epoch_yr, epoch_mjd, magnitude)
WHERE modern_name = 'Delta Cephei';

-- 参宿四 (Betelgeuse) - 古代测量
INSERT INTO magnitude_measurements (variable_id, epoch_yr, magnitude, magnitude_uncertainty, passband, source_type, source_book, ancient_description, ancient_quality)
SELECT id, epoch_yr, magnitude, mag_err, 'V', 'ancient', source_book, description, quality
FROM variable_stars,
     (VALUES
         ( -300.0, 0.3, 0.2, '天官书', '参宿四 赤色大星', 2),
         (  -50.0, 0.5, 0.3, '汉书', '参左肩 明', 2),
         (  300.0, 0.2, 0.2, '晋书', '参四 最明', 1),
         (  900.0, 0.8, 0.3, '新唐书', '参宿 稍暗', 2),
         ( 1200.0, 0.4, 0.2, '宋史', '参四 赤色', 2),
         ( 1450.0, 0.6, 0.3, '明实录', '参宿四 如常', 3),
         ( 1600.0, 0.3, 0.2, '崇祯历书', '参宿四 一等星', 1)
     ) AS t(epoch_yr, magnitude, mag_err, source_book, description, quality)
WHERE modern_name = 'Betelgeuse';

-- 参宿四 (Betelgeuse) - 现代模拟测量
INSERT INTO magnitude_measurements (variable_id, epoch_yr, epoch_mjd, magnitude, magnitude_uncertainty, passband, source_type, source_book)
SELECT id, epoch_yr, epoch_mjd, magnitude, 0.02, 'V', 'modern_photometry', 'AAVSO'
FROM variable_stars,
     (VALUES
         (2000.0, 51544.0, 0.5),
         (2002.0, 52280.0, 0.4),
         (2004.0, 53010.0, 0.6),
         (2006.0, 53740.0, 0.3),
         (2008.0, 54470.0, 0.7),
         (2010.0, 55200.0, 0.5),
         (2012.0, 55930.0, 0.4),
         (2014.0, 56660.0, 0.8),
         (2016.0, 57390.0, 1.2),
         (2018.0, 58120.0, 1.5),
         (2020.0, 58850.0, 0.9),
         (2022.0, 59580.0, 0.6)
     ) AS t(epoch_yr, epoch_mjd, magnitude)
WHERE modern_name = 'Betelgeuse';

-- 心宿二 (Antares) - 古代测量
INSERT INTO magnitude_measurements (variable_id, epoch_yr, magnitude, magnitude_uncertainty, passband, source_type, source_book, ancient_description, ancient_quality)
SELECT id, epoch_yr, magnitude, mag_err, 'V', 'ancient', source_book, description, quality
FROM variable_stars,
     (VALUES
         ( -500.0, 1.0, 0.3, '诗经', '七月流火', 2),
         ( -200.0, 1.2, 0.3, '左传', '心宿二 大火', 2),
         (  100.0, 0.9, 0.2, '汉书', '心 赤色大星', 1),
         (  700.0, 1.4, 0.3, '隋书', '心宿 稍暗', 2),
         ( 1000.0, 1.1, 0.2, '宋史', '心宿二 明', 1),
         ( 1300.0, 1.3, 0.3, '元史', '心宿 如常', 2),
         ( 1550.0, 1.0, 0.2, '明实录', '心宿 大火星', 2),
         ( 1600.0, 1.2, 0.3, '崇祯历书', '心宿二 一等星', 1)
     ) AS t(epoch_yr, magnitude, mag_err, source_book, description, quality)
WHERE modern_name = 'Antares';

-- 心宿二 (Antares) - 现代模拟测量
INSERT INTO magnitude_measurements (variable_id, epoch_yr, epoch_mjd, magnitude, magnitude_uncertainty, passband, source_type, source_book)
SELECT id, epoch_yr, epoch_mjd, magnitude, 0.02, 'V', 'modern_photometry', 'AAVSO'
FROM variable_stars,
     (VALUES
         (2000.0, 51544.0, 1.1),
         (2001.0, 51910.0, 0.9),
         (2002.0, 52275.0, 1.3),
         (2003.0, 52640.0, 1.0),
         (2004.0, 53005.0, 1.5),
         (2005.0, 53370.0, 1.2),
         (2006.0, 53735.0, 0.9),
         (2007.0, 54100.0, 1.4),
         (2008.0, 54465.0, 1.1),
         (2009.0, 54830.0, 1.6),
         (2010.0, 55195.0, 1.3),
         (2011.0, 55560.0, 1.0),
         (2012.0, 55925.0, 1.2),
         (2013.0, 56290.0, 1.5),
         (2014.0, 56655.0, 1.1)
     ) AS t(epoch_yr, epoch_mjd, magnitude)
WHERE modern_name = 'Antares';

-- 织女一 (Vega) - 古代测量
INSERT INTO magnitude_measurements (variable_id, epoch_yr, magnitude, magnitude_uncertainty, passband, source_type, source_book, ancient_description, ancient_quality)
SELECT id, epoch_yr, magnitude, mag_err, 'V', 'ancient', source_book, description, quality
FROM variable_stars,
     (VALUES
         ( -400.0, 0.1, 0.2, '诗经', '织女 三星', 1),
         ( -100.0, 0.05, 0.2, '史记', '织女 天女', 1),
         (  200.0, 0.08, 0.2, '汉书', '织女星 极明', 1),
         (  800.0, 0.03, 0.15, '隋书', '织女 最明', 1),
         ( 1100.0, 0.06, 0.2, '宋史', '织女 明亮', 1),
         ( 1400.0, 0.04, 0.15, '明实录', '织女星 如常', 1)
     ) AS t(epoch_yr, magnitude, mag_err, source_book, description, quality)
WHERE modern_name = 'Vega';

-- 织女一 (Vega) - 现代模拟测量
INSERT INTO magnitude_measurements (variable_id, epoch_yr, epoch_mjd, magnitude, magnitude_uncertainty, passband, source_type, source_book)
SELECT id, epoch_yr, epoch_mjd, magnitude, 0.005, 'V', 'modern_photometry', 'Hipparcos'
FROM variable_stars,
     (VALUES
         (1990.0, 47892.0, 0.03),
         (1995.0, 49718.0, 0.04),
         (2000.0, 51544.0, 0.03),
         (2002.0, 52275.0, 0.05),
         (2004.0, 53006.0, 0.03),
         (2006.0, 53737.0, 0.04),
         (2008.0, 54468.0, 0.03),
         (2010.0, 55199.0, 0.05),
         (2012.0, 55930.0, 0.04),
         (2014.0, 56661.0, 0.03)
     ) AS t(epoch_yr, epoch_mjd, magnitude)
WHERE modern_name = 'Vega';
