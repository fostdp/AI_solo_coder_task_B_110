import argparse
import json
import os
import sys
import time
from datetime import datetime
from typing import List, Optional

try:
    import psycopg2
    from psycopg2.extras import execute_values
    HAS_PSYCOPG2 = True
except ImportError:
    HAS_PSYCOPG2 = False

try:
    from tqdm import tqdm
    HAS_TQDM = True
except ImportError:
    HAS_TQDM = False

from .dynasty_config import (
    DynastyConfig,
    get_dynasty_config,
    get_all_dynasties,
    list_dynasty_names,
)
from .star_generator import StarGenerator, SimulatedStar
from .transient_generator import TransientGenerator, GuestStarEvent


def get_db_connection(database_url: Optional[str] = None):
    if not HAS_PSYCOPG2:
        return None
    url = database_url or os.environ.get("DATABASE_URL")
    if not url:
        return None
    try:
        return psycopg2.connect(url)
    except Exception:
        return None


def format_timestamp() -> str:
    return datetime.now().strftime("%Y-%m-%d %H:%M:%S")


def print_banner():
    banner = r"""
╔══════════════════════════════════════════════════════════════╗
║           古代星表模拟器 (Ancient Star Catalog Simulator)       ║
║   IAU 2006 岁差 + 银道盘分布 + 客星事件生成                    ║
╚══════════════════════════════════════════════════════════════╝
"""
    print(banner)


def print_dynasty_info(config: DynastyConfig):
    print(f"\n{'─' * 60}")
    print(f"  朝代: {config.name_cn} ({config.name})")
    print(f"  时期: {config.start_year:+d} ~ {config.end_year:+d}  (共 {config.duration_years} 年)")
    print(f"  首都: {config.capital_name}  (纬度 ~{config.capital_latitude:.1f}°N)")
    print(f"  宿度: {config.sudu_version}")
    print(f"  精度: ±{config.accuracy_error_deg:.2f}°")
    print(f"  典型星官数: {config.typical_asterism_count}")
    print(f"  颜色术语: {', '.join(config.color_terms)}")
    print(f"{'─' * 60}\n")


def print_stars_summary(stars: List[SimulatedStar], dynasty_name: str):
    if not stars:
        print(f"  [!] {dynasty_name}: 未生成恒星")
        return

    mags = [s.magnitude_num for s in stars]
    decs = [s.dec_j2000 for s in stars]
    gls = [s.galactic_l for s in stars]
    gbs = [s.galactic_b for s in stars]

    print(f"  ★ 恒星生成结果: {len(stars)} 颗")
    print(f"    星等范围: {min(mags):.2f} ~ {max(mags):.2f}  (均值: {sum(mags)/len(mags):.2f})")
    print(f"    赤纬范围: {min(decs):.1f}° ~ {max(decs):.1f}°")
    print(f"    银纬范围: {min(gbs):.1f}° ~ {max(gbs):.1f}°  (|b|<10°: {sum(1 for b in gbs if abs(b)<10)})")

    from collections import Counter
    spec_count = Counter(s.spectral_type for s in stars)
    color_count = Counter(s.color_desc for s in stars)
    print(f"    光谱分布: {dict(spec_count.most_common())}")
    print(f"    颜色描述: {dict(color_count.most_common(5))}")

    mansion_count = Counter(s.mansion_name for s in stars if s.mansion_name)
    print(f"    星宿分布(Top5): {dict(mansion_count.most_common(5))}")


def print_guest_stars_summary(events: List[GuestStarEvent], dynasty_name: str):
    if not events:
        print(f"  [!] {dynasty_name}: 未生成客星事件")
        return

    mags = [e.peak_mag for e in events]
    yrs = [e.year_ce for e in events]
    vis = [e.visibility_days for e in events]
    famous = sum(1 for e in events if e.is_famous)

    print(f"  ☄  客星事件生成: {len(events)} 件 (著名: {famous}, 普通: {len(events)-famous})")
    print(f"    峰值星等: {min(mags):.1f} ~ {max(mags):.1f}  (均值: {sum(mags)/len(mags):.1f})")
    print(f"    年份分布: {min(yrs):+.0f} ~ {max(yrs):+.0f}")
    print(f"    可见期: {min(vis)} ~ {max(vis)} 天  (均值: {sum(vis)/len(vis):.0f} 天)")

    from collections import Counter
    lc_count = Counter(e.lightcurve_type for e in events)
    print(f"    光变类型: {dict(lc_count.most_common())}")

    book_count = Counter(e.source_book for e in events)
    print(f"    记载来源(Top3): {dict(book_count.most_common(3))}")

    if famous > 0:
        famous_events = [e for e in events if e.is_famous]
        print(f"\n  ★★ 著名客星 ({famous} 件):")
        for fe in famous_events:
            print(f"    • {fe.star_name}: {fe.year_ce:+.0f}年, "
                  f"峰值 {fe.peak_mag:.1f}等, {fe.visibility_days}天, "
                  f"{fe.lightcurve_type}, 来源: {fe.source_book}")
            if fe.historical_text:
                text_short = fe.historical_text[:60] + "..." if len(fe.historical_text) > 60 else fe.historical_text
                print(f"      记载: {text_short}")


def print_sample_stars(stars: List[SimulatedStar], n: int = 5):
    if not stars:
        return
    samples = stars[:n]
    print(f"\n  恒星样本 (前 {len(samples)} 颗):")
    print(f"  {'ID':<22} {'RA(J2000)':>10} {'Dec(J2000)':>10} "
          f"{'宿':<4} {'入宿':>6} {'去极':>6} {'星等':>5} {'颜色':<4} {'光谱':<3}")
    print(f"  {'─'*82}")
    for s in samples:
        mansion = s.mansion_name or "—"
        ruxiu = f"{s.ruxiu_du:.2f}°" if s.ruxiu_du is not None else "—"
        quji = f"{s.quji_du:.2f}°" if s.quji_du is not None else "—"
        print(f"  {s.star_id_code:<22} {s.ra_j2000:>9.4f}° {s.dec_j2000:>9.4f}° "
              f"{mansion:<4} {ruxiu:>6} {quji:>6} {s.magnitude_num:>5.2f} {s.color_desc:<4} {s.spectral_type:<3}")


def print_sample_guests(events: List[GuestStarEvent], n: int = 3):
    if not events:
        return
    samples = events[:n]
    print(f"\n  客星样本 (前 {len(samples)} 件):")
    print(f"  {'ID':<26} {'名称':<18} {'年份':>8} {'RA(J2000)':>10} {'Dec(J2000)':>10} "
          f"{'峰值':>6} {'光变':<5} {'可见':>6} {'著名':<4}")
    print(f"  {'─'*110}")
    for e in samples:
        name = (e.star_name[:16] + "..") if len(e.star_name) > 16 else e.star_name
        famous = "★★" if e.is_famous else "—"
        print(f"  {e.guest_id_code:<26} {name:<18} {e.year_ce:+8.1f} "
              f"{e.ra_j2000:>9.4f}° {e.dec_j2000:>9.4f}° "
              f"{e.peak_mag:>5.1f} {e.lightcurve_type:<5} {e.visibility_days:>5}天 {famous:<4}")


def insert_stars_to_db(conn, stars: List[SimulatedStar], dynasty_id_map: dict) -> int:
    if not conn or not stars:
        return 0

    cursor = conn.cursor()
    try:
        rows = []
        for s in stars:
            dynasty_id = dynasty_id_map.get(s.dynasty)
            rows.append((
                s.star_id_code,
                dynasty_id,
                s.mansion_order,
                None,
                None,
                None,
                s.ruxiu_du,
                s.quji_du,
                s.ra_ancient,
                s.dec_ancient,
                s.ra_j2000,
                s.dec_j2000,
                s.magnitude_ancient,
                s.magnitude_num,
                s.color_desc,
                s.color_class,
                s.color_temp_k,
                s.proper_motion_ra,
                s.proper_motion_dec,
                None,
                f"simulator_seed_{int(time.time())}",
                1,
                f"模拟生成, 观测误差±{s.observation_error_deg:.2f}°",
            ))

        sql = """
        INSERT INTO ancient_stars (
            star_id_code, dynasty_id, mansion_id,
            star_name_cn, star_name_alt, constellation,
            ruxiu_du, quji_du,
            ra_ancient_conv, dec_ancient_conv,
            ra_j2000, dec_j2000,
            magnitude_ancient, magnitude_num,
            color_desc, color_class, color_temp_k,
            proper_motion_ra, proper_motion_dec,
            parallax, source_book, quality_flag, notes
        ) VALUES %s
        ON CONFLICT (star_id_code) DO NOTHING
        """
        execute_values(cursor, sql, rows)
        inserted = cursor.rowcount
        conn.commit()
        return inserted
    except Exception as e:
        conn.rollback()
        print(f"  [DB ERROR] 插入恒星失败: {e}")
        return 0
    finally:
        cursor.close()


def insert_guests_to_db(conn, events: List[GuestStarEvent], dynasty_id_map: dict) -> int:
    if not conn or not events:
        return 0

    cursor = conn.cursor()
    try:
        rows = []
        for e in events:
            dynasty_id = dynasty_id_map.get(e.dynasty)
            rows.append((
                e.guest_id_code,
                dynasty_id,
                e.star_name,
                e.year_ancient,
                e.year_ce,
                e.month_ancient,
                e.day_ancient,
                e.ruxiu_du,
                e.quji_du,
                e.ra_j2000,
                e.dec_j2000,
                e.ra_err_deg,
                e.dec_err_deg,
                e.peak_mag,
                e.peak_mag_err,
                e.visibility_days,
                e.lightcurve_type,
                e.description,
                e.position_desc,
                e.source_book,
            ))

        sql = """
        INSERT INTO guest_stars (
            guest_id_code, dynasty_id, star_name,
            year_ancient, year_ce, month_ancient, day_ancient,
            ruxiu_du, quji_du,
            ra_deg, dec_deg,
            ra_err, dec_err,
            peak_mag, peak_mag_err,
            visibility_days, lightcurve_type,
            description, position_desc, source_book
        ) VALUES %s
        ON CONFLICT (guest_id_code) DO NOTHING
        """
        execute_values(cursor, sql, rows)
        inserted = cursor.rowcount
        conn.commit()
        return inserted
    except Exception as e:
        conn.rollback()
        print(f"  [DB ERROR] 插入客星失败: {e}")
        return 0
    finally:
        cursor.close()


def load_dynasty_id_map(conn) -> dict:
    if not conn:
        return {}
    cursor = conn.cursor()
    try:
        cursor.execute("SELECT id, name_pinyin FROM dynasties")
        rows = cursor.fetchall()
        mapping = {}
        name_norm = {
            "han": "Han",
            "han_western": "Han",
            "han_eastern": "Han",
            "western han": "Han",
            "eastern han": "Han",
            "three kingdoms": "Three Kingdoms",
            "three_kingdoms": "Three Kingdoms",
            "jin": "Jin",
            "northern_southern": "North-South",
            "north-south": "North-South",
            "sui": "Sui",
            "tang": "Tang",
            "song": "Song",
            "yuan": "Yuan",
            "ming": "Ming",
            "xia": "Han",
            "shang": "Han",
            "zhou": "Han",
            "qin": "Han",
        }
        for db_id, name_pinyin in rows:
            key = name_pinyin.lower().replace(" ", "_")
            mapping[key] = db_id
            if name_pinyin:
                mapping[name_pinyin.lower()] = db_id
        for sim_key, db_key in name_norm.items():
            if db_key.lower() in mapping:
                mapping[sim_key] = mapping[db_key.lower()]
        return mapping
    except Exception as e:
        print(f"  [WARN] 加载朝代映射失败: {e}")
        return {}
    finally:
        cursor.close()


def run_single_dynasty(
    config: DynastyConfig,
    n_stars: int,
    n_events: int,
    seed: int,
    star_gen: StarGenerator,
    transient_gen: TransientGenerator,
    conn,
    dynasty_id_map: dict,
    dry_run: bool,
) -> dict:
    print_dynasty_info(config)

    iter_stars = range(n_stars)
    iter_events = range(n_events)
    if HAS_TQDM and (n_stars > 100 or n_events > 20):
        if n_stars > 100:
            iter_stars = tqdm(range(n_stars), desc="  生成恒星", unit="颗", leave=False)
        if n_events > 20:
            iter_events = tqdm(range(n_events), desc="  生成客星", unit="件", leave=False)

    seed_stars = seed + hash(config.name) % 10000
    seed_events = seed + hash(config.name + "_events") % 10000

    stars = star_gen.generate_stars_for_dynasty(config, n_stars, seed=seed_stars)
    events = transient_gen.generate_guest_stars(config, n_events, seed=seed_events)

    print_stars_summary(stars, config.name_cn)
    print_guest_stars_summary(events, config.name_cn)
    print_sample_stars(stars, n=5)
    print_sample_guests(events, n=3)

    result = {
        "dynasty": config.name,
        "dynasty_cn": config.name_cn,
        "n_stars_generated": len(stars),
        "n_events_generated": len(events),
        "stars_inserted": 0,
        "events_inserted": 0,
    }

    if not dry_run and conn:
        si = insert_stars_to_db(conn, stars, dynasty_id_map)
        gi = insert_guests_to_db(conn, events, dynasty_id_map)
        result["stars_inserted"] = si
        result["events_inserted"] = gi
        print(f"\n  ✓ 数据库写入: 恒星 {si} 颗, 客星 {gi} 件")
    elif dry_run:
        print(f"\n  [DRY-RUN] 跳过数据库写入 (--dry-run)")
    else:
        print(f"\n  [!] 未连接数据库，跳过写入 (设置 DATABASE_URL 以启用)")

    return result


def run_continuous_mode(
    configs: List[DynastyConfig],
    n_stars_per_batch: int,
    n_events_per_batch: int,
    base_seed: int,
    interval_sec: float,
    conn,
    dynasty_id_map: dict,
    dry_run: bool,
):
    star_gen = StarGenerator(base_seed=base_seed)
    transient_gen = TransientGenerator(base_seed=base_seed + 999)

    batch_count = 0
    total_stars = 0
    total_events = 0

    print(f"\n{'═' * 60}")
    print(f"  进入持续模拟模式 (间隔 {interval_sec}s, Ctrl+C 退出)")
    print(f"  朝代: {', '.join(c.name_cn for c in configs)}")
    print(f"{'═' * 60}\n")

    try:
        while True:
            batch_count += 1
            batch_seed = base_seed + batch_count * 1000000
            print(f"\n[{format_timestamp()}] ====== 批次 #{batch_count} ======")

            for config in configs:
                seed = batch_seed + hash(config.name) % 10000
                stars = star_gen.generate_stars_for_dynasty(
                    config, n_stars_per_batch, seed=seed)
                events = transient_gen.generate_guest_stars(
                    config, n_events_per_batch, seed=seed + 123)

                total_stars += len(stars)
                total_events += len(events)

                mag_avg = sum(e.peak_mag for e in events) / len(events) if events else 0
                print(f"  {config.name_cn}: {len(stars):>4}★  {len(events):>3}☄  "
                      f"(客星均亮 {mag_avg:.1f}等)")

                if not dry_run and conn:
                    insert_stars_to_db(conn, stars, dynasty_id_map)
                    insert_guests_to_db(conn, events, dynasty_id_map)

            print(f"  └ 累计: 恒星 {total_stars}, 客星 {total_events}")

            if interval_sec > 0:
                try:
                    time.sleep(interval_sec)
                except KeyboardInterrupt:
                    raise
    except KeyboardInterrupt:
        print(f"\n\n[{format_timestamp()}] 用户中断，退出持续模式")
        print(f"  总批次: {batch_count}")
        print(f"  总恒星: {total_stars}")
        print(f"  总客星: {total_events}")


def parse_args():
    parser = argparse.ArgumentParser(
        description="古代星表模拟器 - 生成恒星与客星模拟数据",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
示例:
  %(prog)s --dynasty all --n-stars 200 --n-events 20 --seed 42
  %(prog)s --dynasty tang --n-stars 500 --n-events 30 --dry-run
  %(prog)s --dynasty song,yuan,ming --continuous --n-events 5
  %(prog)s --dynasty all --continuous --seed 12345 --n-stars 100
        """,
    )

    parser.add_argument(
        "--dynasty", "-d",
        type=str,
        default="all",
        help="朝代名 (单个或逗号分隔), 或 'all' 全部。可用: " + ", ".join(list_dynasty_names()),
    )
    parser.add_argument(
        "--n-stars", "-s",
        type=int,
        default=200,
        help="每朝代生成恒星数量 (默认: 200)",
    )
    parser.add_argument(
        "--n-events", "-e",
        type=int,
        default=20,
        help="每朝代生成客星事件数量 (默认: 20)",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=42,
        help="随机种子 (默认: 42)",
    )
    parser.add_argument(
        "--continuous", "-c",
        action="store_true",
        help="持续模式: 每10秒生成一批新客星 (Ctrl+C退出)",
    )
    parser.add_argument(
        "--interval",
        type=float,
        default=10.0,
        help="持续模式间隔秒数 (默认: 10.0)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="仅输出到控制台，不写入数据库",
    )
    parser.add_argument(
        "--database-url",
        type=str,
        default=None,
        help="PostgreSQL 连接 URL (也可从环境变量 DATABASE_URL 读取)",
    )
    parser.add_argument(
        "--list-dynasties",
        action="store_true",
        help="列出所有可用朝代并退出",
    )
    parser.add_argument(
        "--output-json",
        type=str,
        default=None,
        help="将结果导出为 JSON 文件",
    )

    return parser.parse_args()


def resolve_dynasties(dynasty_arg: str) -> List[DynastyConfig]:
    if dynasty_arg.lower() == "all":
        return get_all_dynasties()

    names = [n.strip() for n in dynasty_arg.split(",")]
    configs = []
    missing = []
    for n in names:
        cfg = get_dynasty_config(n)
        if cfg:
            configs.append(cfg)
        else:
            missing.append(n)

    if missing:
        print(f"[WARN] 未找到朝代: {', '.join(missing)}")
        print(f"       可用朝代: {', '.join(list_dynasty_names())}")

    if not configs:
        print("[ERROR] 未指定任何有效朝代")
        sys.exit(1)

    return configs


def main():
    args = parse_args()

    print_banner()

    if args.list_dynasties:
        print("  可用朝代列表:\n")
        for cfg in get_all_dynasties():
            print(f"    {cfg.name:<20} {cfg.name_cn:<4} "
                  f"{cfg.start_year:+6d} ~ {cfg.end_year:+6d}  "
                  f"{cfg.sudu_version:<6}  "
                  f"精度±{cfg.accuracy_error_deg:.1f}°")
        return

    configs = resolve_dynasties(args.dynasty)

    print(f"  配置: 种子={args.seed}, 恒星/朝代={args.n_stars}, "
          f"客星/朝代={args.n_events}, 持续模式={'是' if args.continuous else '否'}, "
          f"Dry-Run={'是' if args.dry_run else '否'}")
    print(f"  psycopg2: {'可用' if HAS_PSYCOPG2 else '未安装'}, "
          f"tqdm: {'可用' if HAS_TQDM else '未安装'}")

    conn = None
    dynasty_id_map = {}
    if not args.dry_run:
        conn = get_db_connection(args.database_url)
        if conn:
            dynasty_id_map = load_dynasty_id_map(conn)
            print(f"  ✓ 数据库连接成功 (已映射 {len(dynasty_id_map)} 个朝代)")
        else:
            if args.database_url or os.environ.get("DATABASE_URL"):
                print(f"  [WARN] 数据库连接失败")
            else:
                print(f"  [INFO] 未设置 DATABASE_URL，跳过数据库操作")

    star_gen = StarGenerator(base_seed=args.seed)
    transient_gen = TransientGenerator(base_seed=args.seed + 777)

    if args.continuous:
        run_continuous_mode(
            configs=configs,
            n_stars_per_batch=args.n_stars,
            n_events_per_batch=args.n_events,
            base_seed=args.seed,
            interval_sec=args.interval,
            conn=conn,
            dynasty_id_map=dynasty_id_map,
            dry_run=args.dry_run,
        )
        if conn:
            conn.close()
        return

    all_results = []
    for cfg in configs:
        result = run_single_dynasty(
            config=cfg,
            n_stars=args.n_stars,
            n_events=args.n_events,
            seed=args.seed,
            star_gen=star_gen,
            transient_gen=transient_gen,
            conn=conn,
            dynasty_id_map=dynasty_id_map,
            dry_run=args.dry_run,
        )
        all_results.append(result)

    print(f"\n\n{'═' * 60}")
    print(f"  总结 ({len(all_results)} 个朝代):")
    print(f"  {'朝代':<16} {'恒星':>8} {'客星':>6} {'★写入':>8} {'☄写入':>8}")
    print(f"  {'─' * 50}")
    total_stars = 0
    total_events = 0
    total_si = 0
    total_gi = 0
    for r in all_results:
        print(f"  {r['dynasty_cn'] + '(' + r['dynasty'] + ')':<16} "
              f"{r['n_stars_generated']:>8} {r['n_events_generated']:>6} "
              f"{r['stars_inserted']:>8} {r['events_inserted']:>8}")
        total_stars += r["n_stars_generated"]
        total_events += r["n_events_generated"]
        total_si += r["stars_inserted"]
        total_gi += r["events_inserted"]
    print(f"  {'─' * 50}")
    print(f"  {'合计':<16} {total_stars:>8} {total_events:>6} {total_si:>8} {total_gi:>8}")
    print(f"{'═' * 60}")

    if args.output_json:
        try:
            export_data = {
                "generated_at": format_timestamp(),
                "seed": args.seed,
                "n_stars_per_dynasty": args.n_stars,
                "n_events_per_dynasty": args.n_events,
                "dry_run": args.dry_run,
                "results": all_results,
            }
            with open(args.output_json, "w", encoding="utf-8") as f:
                json.dump(export_data, f, ensure_ascii=False, indent=2)
            print(f"\n  ✓ 结果已导出到: {args.output_json}")
        except Exception as e:
            print(f"\n  [ERROR] 导出 JSON 失败: {e}")

    if conn:
        conn.close()

    print(f"\n  模拟完成 ✓")


if __name__ == "__main__":
    main()
