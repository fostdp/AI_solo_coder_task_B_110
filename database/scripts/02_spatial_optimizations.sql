-- ============================================================
-- 02_spatial_optimizations.sql
-- PostgreSQL + PostGIS 空间索引与性能优化
--
-- 用途:
--   1. 空间索引 (GiST) - 天球坐标查询加速
--   2. B-Tree 索引 - 时间/朝代/星等维度筛选
--   3. GIN 索引 - 文本搜索
--   4. 分区表 - 客星事件按朝代分区
--   5. 统计信息收集
--   6. 物化视图 - 跨朝代星对缓存
-- ============================================================

-- ============================================================
-- 1. PostGIS 空间索引
-- ============================================================

-- 古代恒星: 天球空间坐标 GiST 索引 (RA/Dec 赤道坐标)
CREATE INDEX IF NOT EXISTS idx_ancient_stars_geom_spgist
    ON ancient_stars USING SPGIST (geom);

CREATE INDEX IF NOT EXISTS idx_ancient_stars_geom_gist
    ON ancient_stars USING GIST (geom);

-- J2000 转换后坐标 (如果建了额外的 geom_j2000 列)
-- 用 ST_SetSRID 构造索引表达式
CREATE INDEX IF NOT EXISTS idx_ancient_stars_j2000_geog
    ON ancient_stars USING GIST (
        ST_SetSRID(
            ST_MakePoint(
                COALESCE(ra_j2000, ra_ancient_conv, 0),
                COALESCE(dec_j2000, dec_ancient_conv, 0)
            ),
            4035
        )::geography
    );

-- 超新星遗迹空间索引
CREATE INDEX IF NOT EXISTS idx_snr_geom_gist
    ON supernova_remnants USING GIST (geom);

CREATE INDEX IF NOT EXISTS idx_snr_galactic
    ON supernova_remnants USING SPGIST (
        point(gal_l, gal_b)
    );

-- 客星事件空间索引
CREATE INDEX IF NOT EXISTS idx_guest_stars_geom_gist
    ON guest_stars USING GIST (geom);

-- ============================================================
-- 2. B-Tree 复合索引
-- ============================================================

-- 恒星: 按朝代筛选 + 按质量过滤 + 按星等排序
CREATE INDEX IF NOT EXISTS idx_stars_dynasty_quality_mag
    ON ancient_stars (dynasty_id, quality_flag, magnitude_num)
    WHERE magnitude_num IS NOT NULL;

-- 恒星: 星宿+宿度范围查找 (入宿度查询)
CREATE INDEX IF NOT EXISTS idx_stars_mansion_ruxiu
    ON ancient_stars (lunar_mansion_id, ruxiu_du)
    WHERE ruxiu_du IS NOT NULL;

-- 恒星: J2000 RA/Dec 范围检索
CREATE INDEX IF NOT EXISTS idx_stars_ra_dec
    ON ancient_stars (
        COALESCE(ra_j2000, ra_ancient_conv, 0),
        COALESCE(dec_j2000, dec_ancient_conv, 0)
    );

-- 恒星: 色温索引 ( Planck 颜色映射优化查询 )
CREATE INDEX IF NOT EXISTS idx_stars_color_temp
    ON ancient_stars (color_temp_k)
    WHERE color_temp_k IS NOT NULL;

-- 客星: 按年代 + 朝代排序
CREATE INDEX IF NOT EXISTS idx_guest_year_dynasty
    ON guest_stars (year_ce, dynasty_id);

-- 客星: 峰值星等筛选
CREATE INDEX IF NOT EXISTS idx_guest_peak_mag
    ON guest_stars (peak_mag)
    WHERE peak_mag IS NOT NULL;

-- SNR: 类型 + 距离筛选
CREATE INDEX IF NOT EXISTS idx_snr_type_distance
    ON supernova_remnants (sn_type, distance_kpc);

-- SNR: 银道面带筛选  (|b| < 阈值)
CREATE INDEX IF NOT EXISTS idx_snr_gal_b_abs
    ON supernova_remnants (ABS(gal_b));

-- 匹配结果: 客星ID + 概率排序
CREATE INDEX IF NOT EXISTS idx_matches_guest_prob
    ON guest_star_matches (guest_star_id, match_probability DESC);

-- ============================================================
-- 3. GIN 索引 - 文本搜索
-- ============================================================

CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- 恒星中文名搜索
CREATE INDEX IF NOT EXISTS idx_stars_name_trgm
    ON ancient_stars USING GIN (star_name_cn gin_trgm_ops);

-- 恒星出处典籍
CREATE INDEX IF NOT EXISTS idx_stars_source_trgm
    ON ancient_stars USING GIN (source_book gin_trgm_ops)
    WHERE source_book IS NOT NULL;

-- SNR 名称 / 别名
CREATE INDEX IF NOT EXISTS idx_snr_name_trgm
    ON supernova_remnants USING GIN (remnant_name gin_trgm_ops);

-- ============================================================
-- 4. 客星事件按朝代分区
-- ============================================================

-- 先检查表是否为空，安全创建分区父表
DO $$
DECLARE
    cnt INTEGER;
BEGIN
    EXECUTE 'SELECT COUNT(*) FROM guest_stars' INTO cnt;
    IF cnt = 0 THEN
        RAISE NOTICE 'guest_stars is empty, skipping partition migration';
        RETURN;
    END IF;
    RAISE NOTICE 'guest_stars has % rows, partition migration skipped (manual operation)', cnt;
END $$;

-- 分区策略建议:
-- CREATE TABLE guest_stars (...) PARTITION BY LIST (dynasty_id);
-- CREATE TABLE guest_stars_qin PARTITION OF guest_stars FOR VALUES IN (1);
-- ... 每个朝代建一张子表

-- ============================================================
-- 5. 统计信息收集
-- ============================================================

-- 强制 Postgres 为 GiST/SP-GiST 空间索引收集统计
ALTER TABLE ancient_stars ALTER COLUMN geom SET STATISTICS 1000;
ALTER TABLE supernova_remnants ALTER COLUMN geom SET STATISTICS 1000;
ALTER TABLE guest_stars ALTER COLUMN geom SET STATISTICS 1000;

ANALYZE ancient_stars (geom, dynasty_id, magnitude_num, quality_flag);
ANALYZE supernova_remnants (geom, sn_type, distance_kpc, gal_l, gal_b);
ANALYZE guest_stars (geom, year_ce, peak_mag, dynasty_id);
ANALYZE guest_star_matches (guest_star_id, match_probability);
ANALYZE lunar_mansions;
ANALYZE dynasties;

-- ============================================================
-- 6. 物化视图 - 跨朝代星对缓存
-- ============================================================

-- 使用现有的 v_star_cross_dynasty 视图，但建议创建物化视图:
-- CREATE MATERIALIZED VIEW mv_star_cross_dynasty AS
--     SELECT * FROM v_star_cross_dynasty;
-- CREATE UNIQUE INDEX idx_mv_cross_dynasty_pair
--     ON mv_star_cross_dynasty (star_han_id, star_modern_id);
-- CREATE INDEX idx_mv_cross_dynasty_angular ON mv_star_cross_dynasty (angular_sep_arcmin);

-- ============================================================
-- 7. 常用查询性能调优
-- ============================================================

-- 启用 Index Only Scan 优化 (对 B-Tree 复合索引有效)
SET enable_indexonlyscan = on;

-- 空间查询: KNN 搜索优先级
SET effective_io_concurrency = 200;  -- SSD 推荐值
SET random_page_cost = 1.1;          -- SSD 推荐值 (机械盘 4.0)

-- 提高 GiST 索引查询的内存预算
-- SET maintenance_work_mem = '256MB';  -- 索引构建时

-- ============================================================
-- 8. 验证索引使用情况
-- ============================================================

-- 执行后运行以下 SQL 检查索引是否生效:
--
-- -- 空间索引使用检查
-- SELECT indexname, indexdef FROM pg_indexes
-- WHERE tablename LIKE '%star%' OR tablename LIKE '%remnant%'
-- ORDER BY tablename, indexname;
--
-- -- 索引大小统计
-- SELECT
--     schemaname || '.' || tablename AS table_full_name,
--     indexname,
--     pg_size_pretty(pg_relation_size(schemaname || '.' || indexname)) AS index_size
-- FROM pg_stat_user_indexes
-- JOIN pg_indexes USING (schemaname, tablename, indexname)
-- WHERE pg_relation_size(schemaname || '.' || indexname) > 0
-- ORDER BY pg_relation_size(schemaname || '.' || indexname) DESC;
--
-- -- 空间查询 EXPLAIN 示例:
-- EXPLAIN (ANALYZE, BUFFERS)
-- SELECT s.star_name_cn,
--        ST_Distance(s.geom, ST_MakePoint(180, 30)::geometry) * 57.29577951 AS deg
-- FROM ancient_stars s
-- WHERE magnitude_num < 6
-- ORDER BY s.geom <-> ST_MakePoint(180, 30)::geometry
-- LIMIT 20;
