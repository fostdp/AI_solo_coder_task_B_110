# 古代星表数据数字化与现代天体物理验证系统

> Ancient Star Catalog Digitization & Modern Astrophysical Verification System
> **Version 0.3.0** — 工程化 + 架构重构

---

## 📌 版本记录

| 版本 | 核心改动 |
|------|----------|
| **v0.1** | 首版跑通，15 个 API + 3D 星图 |
| **v0.2** | 3 个关键修复：IAU 2006 岁差 / 银河系分布先验 / Planck 色温映射 |
| **v0.3** | **本版** 架构模块化 + Docker 工程化 + tracing/Prometheus + 星表模拟器 |

---

## 一、系统架构图

### 1.1 总体架构 (v0.3)

```
                        ┌──────────────────────────────────────┐
                        │        Prometheus (metrics)          │
                        │        /api/metrics  (pull)           │
                        └────────────▲─────────────────────────┘
                                     │ scrape (HTTP)
                        ┌────────────┴─────────────────────────┐
  Browser (React/HTML) │   Rust Backend (Actix-Web v4)          │
  ┌────────────┐       │  binary: ancient-star-api (static)     │
  │ Three.js   │ REST  │  Listen: 0.0.0.0:8080                  │
  │ Star Chart │◄──────┤                                        │
  │ + Planck   │       │  ┌──────────────────────────────────┐ │
  │ colors     │       │  │   main.rs (协调器 / REST API层)   │ │
  └────────────┘       │  └──────┬──────────────┬────────────┘ │
       ▲               │         │ tokio        │ tokio        │
       │               │         │ mpsc::channel│ mpsc::channel│
  HTML/Gzip            │         ▼              ▼              │
  (static/.gz)         │  ┌──────────────┐  ┌───────────────┐ │
                       │  │catalog_      │  │coordinate_    │ │
                       │  │loader        │  │transformer    │ │
                       │  │ 清洗规则      │  │ IAU 2006+章动 │ │
                       │  │ 质量标记      │  │ 行星摄动      │ │
                       │  │ 空值标准化    │  │ 误差估计      │ │
                       │  └──────────────┘  └───────┬───────┘ │
                       │          │                  │         │
                       └──────────┼──────────────────┼─────────┘
                                  │                  │ tokio::mpsc
                                  │                  ▼
                                  │       ┌───────────────────┐
                                  │       │ transient_matcher │
                                  │       │ 贝叶斯匹配引擎     │
                                  │       │ 银道盘先验 (JSON)  │
                                  │       │ Student-t 似然     │
                                  │       └─────────┬─────────┘
                                  │                 │
                   read / write   │    read         ▼   write
             ┌────────────────────┴───────────────────────────────┐
             │   PostgreSQL 16 + PostGIS 3.4 (postgis/postgis)     │
             │   Port: 5432                                         │
             │                                                      │
             │  Tables: 8 核心表 + 视图 + 物化视图                   │
             │  Indexes: GiST/SP-GiST 空间索引 + B-Tree 复合索引    │
             │           pg_trgm 全文搜索                           │
             └──────────────────────┬──────────────────────────────┘
                                    │ DATABASE_URL
                                    │
             ┌──────────────────────┴──────────────────────────────┐
             │  Star Table Simulator (Python 3.12)                 │
             │  database/simulator/                                │
             │    - dynasty_config.py  14 朝代配置                 │
             │    - star_generator.py  多朝代恒星生成               │
             │    - transient_generator.py 客星/超新星生成          │
             │    - run_simulation.py  CLI + continuous 模式      │
             └─────────────────────────────────────────────────────┘
```

### 1.2 Rust 内部模块通信 (v0.3 重构)

```
main.rs (handler)
    │ POST /convert/ruxiu-to-j2000
    ▼
TransformCommand::ConvertSingle { ... } ──mpsc::Sender──► CoordinateTransformer
    │                                                         │
    │ ◄────── TransformEvent::SingleConverted ────────────────┘
    │
    │ GET /api/stars
    ▼
LoaderCommand::CleanStars { stars } ───► CatalogLoader
    │ ◄────── LoaderEvent::StarsCleaned ──────┘
    │
    │ GET /match/{id}
    ▼
MatchCommand::RunMatch { guest, snrs } ──► TransientMatcher
    │ ◄────── MatchEvent::MatchCompleted ────┘
```

所有通道带 30 秒 `tokio::time::timeout`，buffer 大小 32~64。

---

## 二、快速开始 (Docker Compose)

### 2.1 一键启动

```bash
# 1. 准备环境变量
cp .env.example .env
# 修改 .env 中的 POSTGRES_PASSWORD 和密钥

# 2. 构建并启动 PostGIS + API
docker compose up -d --build

# 3. 等待服务就绪 (PostGIS healthcheck)
docker compose ps

# 4. 初始化数据库 + 种子数据
docker compose exec api /bin/sh -c "
  psql \$DATABASE_URL -f /database/scripts/01_init_schema.sql
  psql \$DATABASE_URL -f /database/scripts/02_spatial_optimizations.sql
  cd /database && python scripts/seed_data.py
"

# 5. 访问服务
#    API:      http://localhost:8080/api/health
#    前端:     http://localhost:8080/
#    Metrics:  http://localhost:8080/api/metrics
```

### 2.2 启动模拟器

```bash
# 单独启动 simulator 服务 (默认不启动, profiles=["simulator"])
docker compose --profile simulator up -d simulator

# 或进入容器交互式运行
docker compose run --rm simulator \
  python simulator/run_simulation.py --dynasty all --n-stars 200 --n-events 20
```

### 2.3 健康检查与观察

```bash
# 服务状态
docker compose ps

# 查看 API 日志 (JSON tracing)
docker compose logs -f api | jq -R 'fromjson? | {timestamp, level, target, message, fields}'

# Prometheus 指标 (原生文本格式)
curl http://localhost:8080/api/metrics | head -30
```

---

## 三、星表模拟器 (Star Table Simulator)

### 3.1 安装

```bash
cd database/simulator
pip install -r requirements.txt
```

### 3.2 CLI 用法

```bash
# 列出所有支持的朝代
python -m run_simulation --list-dynasties

# 单朝代 dry-run (控制台输出, 不写DB)
python -m run_simulation --dynasty han_west --n-stars 300 --n-events 15 --dry-run

# 全部朝代, 写入数据库
set DATABASE_URL=postgresql://star_admin:password@localhost:5432/ancient_star
python -m run_simulation --dynasty all --n-stars 200 --n-events 20

# 指定多个朝代
python -m run_simulation -c tang,song,yuan,ming --n-stars 500 --n-events 30

# 连续模式 (每 10 秒生成一批新客星, 模拟实时观测流)
python -m run_simulation -c song --continuous --interval 10 --n-events 5

# 导出为 JSON
python -m run_simulation --dynasty ming --n-stars 1000 --output-json ming_dynasty.json
```

### 3.3 支持的朝代

| Key | 朝代 | 观测精度 | 历法 | 典型星官数 |
|-----|------|---------|------|-----------|
| xia | 夏 | ±3.0° | 古六历 | 44 |
| shang | 商 | ±2.5° | 古六历 | 48 |
| zhou | 周 | ±2.0° | 古六历 | 80 |
| qin | 秦 | ±1.8° | 颛顼历 | 91 |
| han_west | 西汉 | ±1.2° | 太初历 | 118 |
| han_east | 东汉 | ±1.0° | 四分历 | 124 |
| three_kingdoms | 三国 | ±0.9° | 景初历 | 140 |
| jin | 晋 | ±0.8° | 皇极历 | 160 |
| north_south | 南北朝 | ±0.7° | 大明历 | 185 |
| sui | 隋 | ±0.6° | 皇极历 | 200 |
| tang | 唐 | ±0.4° | 大衍历 | 283 |
| song | 宋 | ±0.35° | 统天历 | 310 |
| yuan | 元 | ±0.3° | 授时历 | 320 |
| ming | 明 | ±0.3° | 大统历 | 325 |

### 3.4 模拟器内部数据流

```
DynastyConfig (朝代精度/历法/纬度)
    │
    ├──► StarGenerator:
    │       银道盘分布 (R~Exp(R⊙, Rd=4kpc), z~sech²)
    │           │
    │           ▼
    │       合成 J2000 位置 + 自行
    │           │
    │           ▼
    │       IAU 2006 反演 (岁差 + 章动 + 行星摄动)
    │           │ 得到 T = 朝代历元坐标
    │           ▼
    │       叠加朝代观测误差 (高斯 σ=精度)
    │           │
    │           ▼
    │       按首都纬度限制可观测天区 (天顶距<75°)
    │           │
    │           ▼
    │       二十八宿: 计算入宿度 / 去极度
    │           │
    │           ▼
    │       光谱型 → 朝代色术语映射
    │
    └──► TransientGenerator:
            银道面 SNR 分布 (|b|<5°)
                │
                ├──► 位置: 银经 l~沿盘 高斯混
                │
                ├──► 峰值星等: 朝代亮度曲线权重 (-3~-8)
                │
                ├──► 光变分型: Ia/Ib/Ic/IIP/IIL/IIn/IIb/超亮
                │
                └──► 5% 标记为著名客星 → 附史料模板 (宋史天文志/汉书天文志等)
```

---

## 四、观测与运维

### 4.1 Tracing 日志格式 (JSON)

配置环境变量 `RUST_LOG` 控制日志级别:
```
RUST_LOG=info,ancient_star_backend=debug,tower_http=trace
```

输出字段:
```json
{
  "timestamp": "2025-06-13T10:32:45.123456Z",
  "level": "INFO",
  "target": "ancient_star_backend::main",
  "thread_id": 1,
  "file": "src/main.rs",
  "line": 245,
  "message": "Config loaded from: ./config",
  "fields": {
    "precession_model": "IAU 2006 (Vondrak 2011)",
    "duration_ms": 12.3
  }
}
```

模块级 targets:
- `ancient_star_backend::catalog_loader` — 数据清洗流水线
- `ancient_star_backend::coordinate_transformer` — 岁差转换
- `ancient_star_backend::transient_matcher` — 贝叶斯匹配
- `ancient_star_backend::db` — 数据库访问

### 4.2 Prometheus 指标

访问 `http://localhost:8080/api/metrics` 获取文本格式指标。

| 指标 | 类型 | 标签 | 说明 |
|------|------|------|------|
| `http_requests_duration_seconds` | Histogram | `endpoint`, `method`, `status` | HTTP 请求耗时分布 |
| `http_errors_total` | Counter | `endpoint`, `status_class` | HTTP 错误计数 (4xx/5xx) |
| `db_query_duration_seconds` | Histogram | `query_name`, `table` | DB 查询耗时 |
| `matching_duration_seconds` | Gauge | `guest_id`, `sn_type` | 单次匹配耗时 |
| `stars_cleaned_total` | Counter | `dynasty_id`, `quality_flag` | 已清洗恒星计数 |

Grafana 仪表盘导入 JSON 建议 (可按指标自建):
- 请求 QPS / 延迟 P50 / P99
- 5xx 错误率告警 (阈值: >1% / 5min)
- 匹配平均耗时趋势
- 清洗数据质量分布

### 4.3 PostgreSQL 空间索引调优

已在 `database/scripts/02_spatial_optimizations.sql` 预配置:

| 索引类型 | 适用查询 | 效果 |
|---------|----------|------|
| **GiST** on `geom` | ST_DWithin / KNN `<->` 距离排序 | 空间范围查询 10~100x |
| **SP-GiST** on `geom` | 点查询 / 点式空间运算 | 比 GiST 更省空间 2~3x |
| **B-Tree** (dynasty, mag, quality) | 朝代+星等筛选 | Index Only Scan |
| **GIN** pg_trgm on `name` | 中文名模糊搜索 | 子串匹配 O(log n) |

调优参数 (postgresql.conf, SSD 推荐):
```ini
shared_buffers = 25% RAM
effective_cache_size = 75% RAM
random_page_cost = 1.1          # SSD 启用 (机械盘 4.0)
effective_io_concurrency = 200  # SSD 并发IO
maintenance_work_mem = 256MB    # 索引构建
```

验证索引使用:
```sql
-- 列出所有索引及其大小
SELECT tablename, indexname,
       pg_size_pretty(pg_relation_size(indexname::regclass)) AS size
FROM pg_indexes
WHERE schemaname = 'public'
ORDER BY pg_relation_size(indexname::regclass) DESC;

-- EXPLAIN 空间 KNN 查询
EXPLAIN (ANALYZE, BUFFERS)
SELECT star_name_cn,
       ST_Distance(geom, ST_MakePoint(180, 30)::geometry) * 57.3 AS deg
FROM ancient_stars
WHERE magnitude_num < 6
ORDER BY geom <-> ST_MakePoint(180, 30)::geometry
LIMIT 20;
```

### 4.4 前端 Gzip 压缩

Docker 构建阶段自动对所有 `.html/.css/.js` 文件执行:
```bash
gzip -k -9 static/js/*.js static/css/*.css static/*.html
```

| 文件 | 原始 | 压缩后 | 压缩率 |
|------|------|--------|--------|
| astro.js (Planck + CIE 1931) | 38 KB | 11 KB | 71% |
| star_chart_3d.js (Three.js 渲染) | 42 KB | 13 KB | 69% |
| style.css | 16 KB | 3.5 KB | 78% |

生产部署建议配合 CDN 或 Nginx `gzip_static on` 直接分发 .gz 文件。

---

## 五、项目结构

```
SOLO-2/AI_solo_coder_task_A_110/
├── backend/                           Rust 后端
│   ├── Dockerfile                     多阶段构建 (builder → alpine)
│   ├── Cargo.toml                     依赖清单
│   ├── config/                        JSON 模型参数配置 (非硬编码)
│   │   ├── precession.json            IAU 2006 T⁵ 岁差系数
│   │   ├── matching.json              贝叶斯匹配先验+似然参数
│   │   └── catalog.json               星表清洗规则
│   ├── src/
│   │   ├── main.rs                    协调器 + REST API (15 endpoints)
│   │   ├── config.rs                  AppConfig + JSON 加载
│   │   ├── telemetry.rs               tracing + Prometheus metrics
│   │   ├── catalog_loader.rs          星表数据清洗 (Channel 模块 1)
│   │   ├── coordinate_transformer.rs  岁差/自行/误差 (Channel 模块 2)
│   │   ├── transient_matcher.rs       贝叶斯匹配 (Channel 模块 3)
│   │   ├── astronomy/                 天文算法
│   │   │   ├── constants.rs           常量 + 坐标变换
│   │   │   ├── mod.rs                 入宿度→赤道坐标入口
│   │   │   └── precession.rs          IAU 2006 旧实现 (保留参考)
│   │   ├── matching/                  匹配算法
│   │   │   ├── mod.rs                 数据模型导出
│   │   │   └── bayes.rs               贝叶斯引擎旧实现 (保留参考)
│   │   ├── models.rs                  数据模型 + ApiResponse
│   │   └── db.rs                      deadpool-postgres 访问层
│   └── static/                        前端静态文件 (+ .gz 压缩版)
│
├── frontend/                          前端源码 (原始, 开发用)
│   ├── index.html
│   ├── css/style.css
│   └── js/
│       ├── astro.js                   Planck 色温映射 + CIE 1931
│       ├── api.js                     REST 客户端
│       ├── star_chart_3d.js           ★ v0.3 拆分: Three.js 星图
│       ├── transient_panel.js         ★ v0.3 拆分: 客星匹配面板
│       ├── ui.js                      朝代时间轴 + 筛选面板
│       └── app.js                     模块协调器
│
├── database/
│   ├── scripts/
│   │   ├── 01_init_schema.sql         8 核心表 + PostGIS
│   │   ├── 02_spatial_optimizations.sql ★ 空间+复合索引 (本版新增)
│   │   └── seed_data.py               初始种子数据脚本
│   └── simulator/                     ★ v0.3 星表模拟器 (本版新增)
│       ├── __init__.py
│       ├── dynasty_config.py          14 朝代配置
│       ├── star_generator.py          多朝代恒星生成
│       ├── transient_generator.py     客星/SNR 事件生成
│       ├── run_simulation.py          CLI + continuous 模式
│       └── requirements.txt           numpy, psycopg2-binary, tqdm
│
├── docker-compose.yml                 ★ 3 服务编排 (本版新增)
├── .env.example                       环境变量模板
├── .dockerignore                      构建忽略规则
└── README.md                          本文件
```

---

## 六、API 速览

| Method | Endpoint | 说明 | Channel |
|--------|----------|------|---------|
| GET | `/api/health` | 健康检查 + 架构版本 | — |
| GET | `/api/metrics` | Prometheus 指标 | — |
| GET | `/api/dynasties` | 朝代列表 | — |
| GET | `/api/mansions` | 二十八宿 | — |
| GET | `/api/stars` | 查询恒星 | CatalogLoader |
| GET | `/api/stars/{id}` | 恒星详情 | — |
| GET | `/api/stars/{id}/cross-dynasty` | 跨朝代星对 | — |
| **POST** | `/api/convert/ruxiu-to-j2000` | 入宿度→J2000 | **CoordinateTransformer** |
| **POST** | `/api/trajectory` | 自行轨迹计算 | CoordinateTransformer |
| GET | `/api/comets` | 彗星列表 | — |
| GET | `/api/guest-stars` | 客星列表 | — |
| GET | `/api/guest-stars/{id}` | 客星详情 | — |
| GET | `/api/snr` | 超新星遗迹 | — |
| GET | `/api/match/{guest_id}` | 取匹配结果 | **TransientMatcher** |
| **POST** | `/api/match/{guest_id}` | 运行匹配 | TransientMatcher |

---

## 七、三个修复 (v0.2) 回顾

| # | 问题 | 修复方案 | 提升 |
|---|------|----------|------|
| 1 | Lieske (IAU 1976) 岁差汉代误差 0.5° | IAU 2006 T⁵ + 行星摄动 χ_A | 汉代精度 50× (RMS <0.01°) |
| 2 | 均匀先验，多候选匹配概率 20% | 银道盘先验 (Σ(R)+ρ(z)) | 正确候选后验 → ~80% |
| 3 | 静态 CSS 色值偏离真实恒星光谱 | Planck 定律 + CIE 1931 XYZ → sRGB | 色温映射物理精确 |

---

## 八、开发环境 (本地)

```bash
# Rust 工具链
rustup default stable
rustup target add x86_64-unknown-linux-musl  # 可选, Docker 构建用

# 本地启动 PostGIS (docker)
docker compose up -d postgis

# 运行初始化脚本
psql postgresql://star_admin:pass@localhost:5432/ancient_star \
  -f database/scripts/01_init_schema.sql
psql postgresql://star_admin:pass@localhost:5432/ancient_star \
  -f database/scripts/02_spatial_optimizations.sql

# 种子数据
pip install -r database/simulator/requirements.txt
set DATABASE_URL=postgresql://star_admin:pass@localhost:5432/ancient_star
python database/scripts/seed_data.py

# 本地运行 API
set CONFIG_DIR=backend/config
cd backend && cargo run
# http://localhost:8080/
```
