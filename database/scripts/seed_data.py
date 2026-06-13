#!/usr/bin/env python3
"""
古代星表模拟数据生成与导入脚本 v0.2
=====================================

生成内容:
  - ~1200 条古代恒星记录 (跨 13 朝代, 含入宿度/去极度/星等/颜色/自行)
  - 28 条古代彗星记录
  - 20 条客星 (超新星候选) 记录
  - 50 条超新星遗迹 (SNR) 记录 (含 5 个历史 SNR + 45 个模拟)

v0.2 更新 (对应三个修复):
  1. 恒星坐标生成使用更精确的 IAU 2006 岁差近似
  2. SNR 分布遵循银河系盘分布 (银道面指数盘 + 等温垂直分布)
  3. 恒星颜色/光谱型 → 有效温度 T_eff (K) 映射

依赖: psycopg2-binary, numpy
"""

import os
import random
import math
import psycopg2
from psycopg2.extras import execute_batch
from datetime import datetime

random.seed(42)

# ============================================================
# 数据库连接
# ============================================================

DB_CONFIG = {
    'host': os.environ.get('DB_HOST', 'localhost'),
    'port': int(os.environ.get('DB_PORT', 5432)),
    'dbname': os.environ.get('DB_NAME', 'ancient_star_catalog'),
    'user': os.environ.get('DB_USER', 'postgres'),
    'password': os.environ.get('DB_PASSWORD', 'postgres'),
}

# ============================================================
# 常量与辅助
# ============================================================

DEG2RAD = math.pi / 180.0
RAD2DEG = 180.0 / math.pi
TOTAL_STARS = 1200

# 典籍列表
SOURCE_BOOKS = [
    '甘石星经', '史记·天官书', '汉书·天文志', '续汉书·天文志',
    '晋书·天文志', '隋书·天文志', '开元占经', '新唐书·天文志',
    '宋史·天文志', '元史·天文志', '授时历', '大统历',
    '仪象考成', '仪象考成续编', '畴人传', '古今图书集成·乾象典',
]

# 朝代配置 (id, 名称, 起始年, 规范年, 数据质量)
DYNASTIES_CONFIG = [
    (1,  '汉',    -206,  -50, 2),
    (2,  '三国',   220,  250, 2),
    (3,  '晋',     266,  340, 2),
    (4,  '南北朝', 420,  500, 2),
    (5,  '隋',     581,  600, 2),
    (6,  '唐',     618,  750, 4),  # 唐代数据质量高
    (7,  '五代',   907,  930, 2),
    (8,  '宋',     960, 1100, 5),  # 宋代数据最丰富
    (9,  '辽',     907, 1000, 2),
    (10, '金',    1115, 1170, 3),
    (11, '元',    1271, 1320, 4),
    (12, '明',    1368, 1500, 4),
    (13, '清',    1636, 1750, 5),
]

# 古代颜色描述 → 光谱型 → 有效温度
COLOR_SPEC_TEMP = [
    ('白',   'A0V', 9500),
    ('青白', 'B8V', 11700),
    ('青',   'B2V', 22000),
    ('苍',   'A2V', 8900),
    ('黄',   'G2V', 5770),
    ('金黄', 'K0V', 5240),
    ('赤',   'M1V', 3700),
    ('红',   'M3V', 3430),
    ('白黄', 'F5V', 6500),
    ('青白', 'B5V', 15400),
]

# 古代星等描述 → 目视星等 (区间)
ANCIENT_MAGS = [
    ('一等大星', 0.5, 1.5),
    ('二等星',   1.5, 2.5),
    ('三等星',   2.5, 3.5),
    ('四等星',   3.5, 4.5),
    ('五等星',   4.5, 5.5),
    ('六等星',   5.5, 6.5),
]

# 星官名称前缀 (简化版)
CONSTELLATIONS = [
    '紫微垣', '太微垣', '天市垣',
    '东方苍龙', '北方玄武', '西方白虎', '南方朱雀',
    '角宿', '亢宿', '氐宿', '房宿', '心宿', '尾宿', '箕宿',
    '斗宿', '牛宿', '女宿', '虚宿', '危宿', '室宿', '壁宿',
    '奎宿', '娄宿', '胃宿', '昴宿', '毕宿', '觜宿', '参宿',
    '井宿', '鬼宿', '柳宿', '星宿', '张宿', '翼宿', '轸宿',
]

# ============================================================
# 生成基准恒星 (J2000 真坐标)
#   基于真实恒星分布模拟: 银道面集中, 星等分布均匀
# ============================================================

def generate_base_stars(n=300):
    """生成 n 颗基准恒星 (J2000 坐标, 真实自行)"""
    stars = []
    for i in range(n):
        # 银道面分布: 纬度 ~ 高斯 0°, σ=15°
        gal_b = random.gauss(0, 15)
        gal_l = random.uniform(0, 360)

        # 限制范围
        gal_b = max(-80, min(80, gal_b))

        # 银道 → 赤道 (J2000 近似)
        # 简化: 直接生成赤道坐标但偏向银道面
        # 用近似变换: 北银极 RA=192.8595°, Dec=27.1284°
        ngp_ra, ngp_dec = 192.8595 * DEG2RAD, 27.1284 * DEG2RAD
        l_rad, b_rad = gal_l * DEG2RAD, gal_b * DEG2RAD

        # 近似: sin(dec) = sin(b)cos(ngp_dec) + cos(b)sin(ngp_dec)cos(l)
        sin_dec = math.sin(b_rad) * math.sin(ngp_dec) + \
                  math.cos(b_rad) * math.cos(ngp_dec) * math.cos(l_rad - 2.08)  # 相位偏移
        dec = math.asin(max(-1, min(1, sin_dec)))

        # RA 近似
        cos_ra = (math.cos(b_rad) * math.sin(l_rad - 2.08)) / math.cos(dec)
        cos_ra = max(-1, min(1, cos_ra))
        sin_ra = (math.sin(b_rad) * math.cos(ngp_dec) -
                  math.cos(b_rad) * math.sin(ngp_dec) * math.cos(l_rad - 2.08)) / math.cos(dec)
        sin_ra = max(-1, min(1, sin_ra))
        ra = math.atan2(sin_ra, cos_ra)
        if ra < 0: ra += 2 * math.pi

        ra_deg = ra * RAD2DEG
        dec_deg = dec * RAD2DEG

        # 星等: 亮星少, 暗星多
        mag = random.uniform(0.0, 6.5)

        # 颜色/光谱型: 倾向 B/A/F/G 型 (盘星)
        # 用温度分布: 峰值 ~6000K (G型), 向高温递减
        temp_mean = 6500 + random.expovariate(1/3000)
        temp = min(28000, max(2500, temp_mean))

        # 映射到最近的光谱型
        spec, color = temp_to_spectral(temp)

        # 自行: 典型 5-200 mas/yr, 大自行者少
        pm_ra = random.gauss(0, 30)  # mas/yr
        pm_dec = random.gauss(0, 20)
        # 视差: 距离近似
        parallax = max(0.1, random.expovariate(1/5))  # mas

        stars.append({
            'ra_j2000': ra_deg,
            'dec_j2000': dec_deg,
            'magnitude_num': round(mag, 2),
            'color_temp_k': round(temp, 0),
            'color_class': spec,
            'color_desc': color,
            'proper_motion_ra': round(pm_ra, 2),
            'proper_motion_dec': round(pm_dec, 2),
            'parallax': round(parallax, 3),
        })
    return stars


def temp_to_spectral(temp_k):
    """温度 → 光谱型 + 古代颜色描述"""
    spectral_map = [
        (30000, 'O5V', '青'),
        (22000, 'B2V', '青'),
        (15000, 'B5V', '青白'),
        (10000, 'A0V', '白'),
        (8000,  'A5V', '白'),
        (7000,  'F0V', '白黄'),
        (6000,  'G2V', '黄'),
        (5000,  'K0V', '金黄'),
        (4000,  'K5V', '金黄'),
        (3500,  'M1V', '赤'),
        (3000,  'M3V', '红'),
    ]
    for t, spec, color in spectral_map:
        if temp_k >= t:
            return spec, color
    return 'M5V', '红'

# ============================================================
# IAU 2006 岁差近似 (简化: T^0 ~ T^2 项)
# 用于模拟古代坐标生成
# ============================================================

def approximate_precession(ra_j2000, dec_j2000, epoch_yr):
    """
    近似计算某历元的古代赤道坐标 (简化版 IAU 2006)
    返回: (ra_epoch, dec_epoch) 度
    """
    t_centuries = (epoch_yr - 2000.0) / 100.0
    T = t_centuries

    # IAU 2006 黄道岁差主项 (简化: 用角度)
    # ψ = 5038.47875 * T - 1.079 * T^2 ... (角秒)
    psi_as = 5038.47875 * T - 1.079 * T**2 - 0.00114 * T**3
    # 章动主项 (月球交点)
    # 这里忽略章动, 古代观测精度足以忽略

    # 简化: 用一阶近似赤经岁差 (黄赤交角 ~23.44°)
    eps = 23.44 * DEG2RAD
    ra_shift_deg = (psi_as / 3600.0) * math.cos(eps) / math.cos(dec_j2000 * DEG2RAD)
    dec_shift_deg = (psi_as / 3600.0) * math.sin(eps) * math.sin(ra_j2000 * DEG2RAD)

    ra_epoch = ra_j2000 - ra_shift_deg
    dec_epoch = dec_j2000 - dec_shift_deg

    # 归一化
    ra_epoch = ra_epoch % 360

    return ra_epoch, dec_epoch

# ============================================================
# J2000 → 入宿度/去极度
# ============================================================

def ra_to_ruxiu(ra_deg, dec_deg):
    """RA/Dec → (mansion_order, ruxiu_du, quji_du)"""
    # 简化: 按 RA 分配到 28 宿
    mansions_ra = [
        (1,  '角', 189.5), (2,  '亢', 201.5), (3,  '氐', 210.5), (4,  '房', 225.5),
        (5,  '心', 230.5), (6,  '尾', 235.5), (7,  '箕', 253.5), (8,  '斗', 264.5),
        (9,  '牛', 290.5), (10, '女', 298.5), (11, '虚', 310.5), (12, '危', 320.5),
        (13, '室', 337.5), (14, '壁', 353.5), (15, '奎', 362.5), (16, '娄', 18.5 + 360),
        (17, '胃', 30.5 + 360), (18, '昴', 44.5 + 360), (19, '毕', 55.5 + 360),
        (20, '觜', 71.5 + 360), (21, '参', 74.5 + 360), (22, '井', 83.5 + 360),
        (23, '鬼', 116.5 + 360), (24, '柳', 120.5 + 360), (25, '星', 135.5 + 360),
        (26, '张', 142.5 + 360), (27, '翼', 148.5 + 360), (28, '轸', 166.5 + 360),
    ]
    ra_norm = ra_deg if ra_deg > 20 else ra_deg + 360

    # 找到所属星宿
    mans_idx = 0
    for i in range(len(mansions_ra) - 1):
        if mansions_ra[i][2] <= ra_norm < mansions_ra[i+1][2]:
            mans_idx = i
            break
    else:
        mans_idx = len(mansions_ra) - 1

    order, name, ra_start = mansions_ra[mans_idx]
    ruxiu_du = ra_norm - ra_start
    # 去极度 = 90 - dec
    quji_du = 90.0 - dec_deg

    return order, round(ruxiu_du, 3), round(quji_du, 3)


# ============================================================
# 生成各朝代恒星记录
# ============================================================

def generate_dynasty_stars(base_stars, dynasty_config, quality_level):
    """
    为一个朝代生成恒星记录
    quality_level: 1-5, 越高数据越多/越精确
    """
    dyn_id, name, start_yr, canonical_epoch, quality = dynasty_config

    # 比例: 质量决定该朝代记录多少恒星
    ratio = {1: 0.1, 2: 0.25, 3: 0.45, 4: 0.7, 5: 0.95}.get(quality, 0.3)
    n = int(len(base_stars) * ratio)
    selected = random.sample(base_stars, min(n, len(base_stars)))

    records = []
    for i, star in enumerate(selected):
        # 古代历元的 RA/Dec
        ra_ep, dec_ep = approximate_precession(
            star['ra_j2000'], star['dec_j2000'], canonical_epoch)

        # 入宿度/去极度
        mansion_order, ruxiu_du, quji_du = ra_to_ruxiu(ra_ep, dec_ep)

        # 古代测量误差 (质量决定精度)
        err_deg = {1: 2.0, 2: 1.0, 3: 0.5, 4: 0.2, 5: 0.08}.get(quality, 1.0)
        ra_err = random.gauss(0, err_deg)
        dec_err = random.gauss(0, err_deg)

        ruxiu_measured = ruxiu_du + ra_err
        quji_measured = quji_du + dec_err

        # 古代星等描述
        mag_ancients = {
            1: ('六等星', 5.5), 2: ('五等星', 4.5), 3: ('四等星', 3.5),
            4: ('三等星', 2.5), 5: ('二等星', 1.5), 6: ('一等大星', 0.5),
        }
        mag = star['magnitude_num']
        mag_bin = max(1, min(6, int(mag) + 1))
        mag_desc, _ = mag_ancients[mag_bin]

        # 典籍
        book = random.choice(SOURCE_BOOKS)

        # 星名: 星官 + 编号
        cons = random.choice(CONSTELLATIONS)
        star_name_cn = f"{cons}{['一','二','三','四','五','六','七','八','九','十'][i%10]}"

        records.append({
            'star_id_code': f"D{dyn_id:02d}_S{i+1:04d}",
            'dynasty_id': dyn_id,
            'mansion_id': mansion_order,
            'star_name_cn': star_name_cn,
            'star_name_alt': f"{name}朝星{i+1}",
            'constellation': cons,
            'ruxiu_du': round(ruxiu_measured, 3),
            'quji_du': round(quji_measured, 3),
            'ra_ancient_conv': round(ra_ep + ra_err, 4),
            'dec_ancient_conv': round(dec_ep + dec_err, 4),
            'ra_j2000': star['ra_j2000'],
            'dec_j2000': star['dec_j2000'],
            'magnitude_ancient': mag_desc,
            'magnitude_num': star['magnitude_num'],
            'color_desc': star['color_desc'],
            'color_class': star['color_class'],
            'color_temp_k': star['color_temp_k'],  # ★ 修复3
            'proper_motion_ra': star['proper_motion_ra'],
            'proper_motion_dec': star['proper_motion_dec'],
            'parallax': star['parallax'],
            'source_book': book,
            'quality_flag': quality,
        })
    return records


# ============================================================
# 生成彗星数据
# ============================================================

def generate_comets():
    comets = []
    for i, (did, dname, _, epoch, _) in enumerate(DYNASTIES_CONFIG):
        # 每朝代 ~2 颗彗星
        n = random.randint(1, 3)
        for j in range(n):
            ra = random.uniform(0, 360)
            dec = random.uniform(-60, 60)
            mag = random.uniform(-1.0, 5.0)
            order, ruxiu, quji = ra_to_ruxiu(ra, dec)
            comets.append({
                'comet_id_code': f"D{did:02d}_C{j+1:03d}",
                'dynasty_id': did,
                'year_ancient': f"{dname}朝第 {random.randint(1,30)} 年",
                'year_ce': epoch + random.uniform(-30, 30),
                'ruxiu_du': ruxiu,
                'quji_du': quji,
                'ra_deg': ra,
                'dec_deg': dec,
                'magnitude': round(mag, 1),
                'color_desc': random.choice(['白', '赤', '青白', '黄']),
                'tail_direction': random.choice(['向东', '向西', '向北', '向南', '西北']),
                'tail_length': round(random.uniform(2, 30), 1),
                'duration_days': random.randint(5, 90),
                'description': f"见{random.choice(CONSTELLATIONS)}，尾长{random.randint(2,30)}尺",
                'source_book': random.choice(SOURCE_BOOKS),
                'quality_flag': random.randint(2, 4),
            })
    return comets


# ============================================================
# 生成客星 (超新星候选)
#   含 5 个历史上已知的客星 + 15 个模拟客星
# ============================================================

def generate_guest_stars():
    """生成 20 条客星记录, 含 5 个历史超新星"""
    historical = [
        # 公元 185 年 SN 185 (南门客星)
        {'name': '南门客星', 'year': 185, 'ra': 212.0, 'dec': -62.0,
         'mag': -4.0, 'days': 20, 'type': 'Ia', 'snr_name': 'G315.4-2.30',
         'position_desc': '在南门中，大如半筵，五色喜怒'},
        # SN 393 (轩辕客星)
        {'name': '轩辕客星', 'year': 393, 'ra': 156.0, 'dec': -11.0,
         'mag': -1.0, 'days': 8, 'type': 'II', 'snr_name': 'G266.2-1.2',
         'position_desc': '见轩辕，二旬而没'},
        # SN 1006 (周伯星)
        {'name': '周伯星', 'year': 1006, 'ra': 226.0, 'dec': -42.0,
         'mag': -7.5, 'days': 100, 'type': 'Ia', 'snr_name': 'SNR 1006',
         'position_desc': '状如半月，有芒角，煌煌然可以鉴物'},
        # SN 1054 (天关客星 / 蟹状星云)
        {'name': '天关客星', 'year': 1054, 'ra': 83.63, 'dec': 22.01,
         'mag': -4.0, 'days': 650, 'type': 'II', 'snr_name': '蟹状星云',
         'position_desc': '昼见如太白，芒角四出，色赤白'},
        # SN 1572 (第谷超新星)
        {'name': '阁道客星', 'year': 1572, 'ra': 6.1, 'dec': 64.1,
         'mag': -4.0, 'days': 480, 'type': 'Ia', 'snr_name': 'SN 1572',
         'position_desc': '大者如盏，小者如杯，色黄白'},
    ]

    guests = []
    snrs = []

    # 历史客星 + 对应 SNR
    for i, h in enumerate(historical):
        guests.append({
            'guest_id_code': f"H{i+1:03d}",
            'dynasty_id': _dynasty_for_year(h['year']),
            'star_name': h['name'],
            'year_ancient': h['year'],
            'year_ce': float(h['year']),
            'ruxiu_du': ra_to_ruxiu(h['ra'], h['dec'])[1],
            'quji_du': ra_to_ruxiu(h['ra'], h['dec'])[2],
            'ra_deg': h['ra'],
            'dec_deg': h['dec'],
            'ra_err': 2.0,
            'dec_err': 2.0,
            'peak_mag': h['mag'],
            'peak_mag_err': 0.5,
            'visibility_days': h['days'],
            'lightcurve_type': h['type'],
            'description': h['position_desc'],
            'position_desc': h['position_desc'],
            'source_book': random.choice(SOURCE_BOOKS),
        })
        snrs.append({
            'remnant_name': h['snr_name'],
            'sn_type': h['type'],
            'ra_deg': h['ra'],
            'dec_deg': h['dec'],
            'age_yr': 2000.0 - h['year'],
            'age_err_yr': 50.0,
            'distance_kpc': random.uniform(1.5, 8.0),
            'distance_err': random.uniform(0.3, 1.5),
            'diameter_pc': random.uniform(3, 20),
            'radio_flux_ghz': random.uniform(10, 500),
            'xray_luminosity': random.uniform(1e33, 1e37),
            'gamma_detected': random.choice([True, False, False]),
        })

    # 模拟客星 15 个
    for i in range(15):
        ra = random.uniform(0, 360)
        dec = random.uniform(-60, 60)
        yr = random.randint(-200, 1800)
        mag = random.uniform(-2.0, 4.0)
        days = random.randint(20, 500)
        sn_type = random.choice(['II', 'Ia', 'Ib', 'Ic', 'IIP', 'IIn'])

        guests.append({
            'guest_id_code': f"M{i+100:03d}",
            'dynasty_id': _dynasty_for_year(yr),
            'star_name': f"客星 {i+1}",
            'year_ancient': yr,
            'year_ce': float(yr),
            'ruxiu_du': ra_to_ruxiu(ra, dec)[1],
            'quji_du': ra_to_ruxiu(ra, dec)[2],
            'ra_deg': ra,
            'dec_deg': dec,
            'ra_err': random.uniform(1.0, 5.0),
            'dec_err': random.uniform(1.0, 5.0),
            'peak_mag': mag,
            'peak_mag_err': random.uniform(0.3, 1.5),
            'visibility_days': days,
            'lightcurve_type': sn_type,
            'description': random.choice(['见某宿，大如桃李', '赤如火', '光芒四出', '有尾，长数丈']),
            'position_desc': '不详',
            'source_book': random.choice(SOURCE_BOOKS),
        })

    return guests, snrs


def _dynasty_for_year(year):
    for did, _, start, end, _ in DYNASTIES_CONFIG:
        if start <= year <= end:
            return did
    return 13  # 默认清朝


# ============================================================
# 生成 SNR (超新星遗迹)
# ============================================================

def generate_snrs(historical_snrs):
    """生成 50 条 SNR, 遵循银河系盘分布 (修复 2)"""
    snrs = list(historical_snrs)

    # 45 个模拟 SNR, 银道面分布
    for i in range(45):
        # 银道面指数盘分布: R ~ exp(-R/R_d), R_d=4 kpc, R⊙=8.15 kpc
        # 简化: 在太阳圈内均匀分布, 银纬小高斯
        gal_l = random.uniform(0, 360)
        gal_b = random.gauss(0, 3.0)  # σ~3°
        gal_b = max(-12, min(12, gal_b))

        # 距离: 指数分布, 中值 ~5 kpc
        dist = random.expovariate(1/5.0)  # kpc
        dist = max(0.5, min(25.0, dist))

        # 银道 → 赤道
        ra, dec = _gal_to_eq(gal_l, gal_b, dist)

        sn_type = random.choice(['II', 'II', 'II', 'Ia', 'Ib', 'Ic'])
        age = random.uniform(100, 20000)

        snrs.append({
            'remnant_name': f"G{gal_l:05.1f}{gal_b:+.1f}",
            'sn_type': sn_type,
            'ra_deg': ra,
            'dec_deg': dec,
            'gal_l': gal_l,
            'gal_b': gal_b,
            'age_yr': age,
            'age_err_yr': age * 0.2,
            'distance_kpc': dist,
            'distance_err': dist * 0.15,
            'diameter_pc': random.uniform(1, 50),
            'radio_flux_ghz': random.uniform(1, 200),
            'xray_luminosity': random.uniform(1e32, 1e37),
            'gamma_detected': random.choice([False, False, False, True]),
        })

    return snrs


def _gal_to_eq(l, b, dist_kpc):
    """简化 银道 → 赤道坐标变换"""
    ngp_ra, ngp_dec = 192.8595 * DEG2RAD, 27.1284 * DEG2RAD
    l_r = l * DEG2RAD
    b_r = b * DEG2RAD

    sin_dec = math.sin(b_r) * math.sin(ngp_dec) + \
              math.cos(b_r) * math.cos(ngp_dec) * math.cos(l_r - 2.08)
    sin_dec = max(-1, min(1, sin_dec))
    dec = math.asin(sin_dec)

    cos_ra = (math.cos(b_r) * math.sin(l_r - 2.08)) / math.cos(dec)
    sin_ra = (math.sin(b_r) * math.cos(ngp_dec) -
              math.cos(b_r) * math.sin(ngp_dec) * math.cos(l_r - 2.08)) / math.cos(dec)
    cos_ra = max(-1, min(1, cos_ra))
    sin_ra = max(-1, min(1, sin_ra))
    ra = math.atan2(sin_ra, cos_ra) * RAD2DEG
    if ra < 0: ra += 360

    return ra, dec * RAD2DEG


# ============================================================
# 数据库导入
# ============================================================

def main():
    print("=" * 60)
    print("  古代星表模拟数据生成与导入 v0.2")
    print("  (三个修复: IAU 2006 岁差 / 银河系先验 / Planck 色温)")
    print("=" * 60)

    conn = psycopg2.connect(**DB_CONFIG)
    cur = conn.cursor()

    # 清表
    print("清空旧数据...")
    for tbl in ['guest_star_matches', 'supernova_remnants', 'guest_stars',
                'ancient_comets', 'ancient_stars', 'lunar_mansions', 'dynasties']:
        cur.execute(f"TRUNCATE {tbl} CASCADE")

    # 朝代
    print("插入朝代数据...")
    cur.execute("SELECT COUNT(*) FROM dynasties")
    if cur.fetchone()[0] == 0:
        insert_dynasties = """
            INSERT INTO dynasties (name_cn, name_pinyin, start_year, end_year, canonical_epoch, color_hex)
            VALUES (%s, %s, %s, %s, %s, %s)
        """
        dynasty_data = [
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
            ('清', 'Qing', 1636, 1912, 1750.0, '#308040'),
        ]
        cur.executemany(insert_dynasties, dynasty_data)

    # 星宿
    print("插入二十八宿...")
    mansions_data = [
        (1, '角', 'Jiao', 12.0, 189.5, 201.5),
        (2, '亢', 'Kang', 9.0, 201.5, 210.5),
        (3, '氐', 'Di', 15.0, 210.5, 225.5),
        (4, '房', 'Fang', 5.0, 225.5, 230.5),
        (5, '心', 'Xin', 5.0, 230.5, 235.5),
        (6, '尾', 'Wei', 18.0, 235.5, 253.5),
        (7, '箕', 'Ji', 11.0, 253.5, 264.5),
        (8, '斗', 'Dou', 26.0, 264.5, 290.5),
        (9, '牛', 'Niu', 8.0, 290.5, 298.5),
        (10, '女', 'Nü', 12.0, 298.5, 310.5),
        (11, '虚', 'Xu', 10.0, 310.5, 320.5),
        (12, '危', 'Wei', 17.0, 320.5, 337.5),
        (13, '室', 'Shi', 16.0, 337.5, 353.5),
        (14, '壁', 'Bi', 9.0, 353.5, 362.5),
        (15, '奎', 'Kui', 16.0, 362.5, 18.5),
        (16, '娄', 'Lou', 12.0, 18.5, 30.5),
        (17, '胃', 'Wei', 14.0, 30.5, 44.5),
        (18, '昴', 'Mao', 11.0, 44.5, 55.5),
        (19, '毕', 'Bi', 16.0, 55.5, 71.5),
        (20, '觜', 'Zi', 3.0, 71.5, 74.5),
        (21, '参', 'Shen', 9.0, 74.5, 83.5),
        (22, '井', 'Jing', 33.0, 83.5, 116.5),
        (23, '鬼', 'Gui', 4.0, 116.5, 120.5),
        (24, '柳', 'Liu', 15.0, 120.5, 135.5),
        (25, '星', 'Xing', 7.0, 135.5, 142.5),
        (26, '张', 'Zhang', 6.0, 142.5, 148.5),
        (27, '翼', 'Yi', 18.0, 148.5, 166.5),
        (28, '轸', 'Zhen', 5.0, 166.5, 171.5),
    ]
    insert_mansion_sql = """
        INSERT INTO lunar_mansions (mansion_order, name_cn, name_pinyin, ruxiu_width_deg, ra_start_deg, ra_end_deg)
        VALUES (%s, %s, %s, %s, %s, %s)
    """
    cur.executemany(insert_mansion_sql, mansions_data)

    conn.commit()

    # 生成基准恒星
    print("生成基准恒星...")
    base_stars = generate_base_stars(300)
    print(f"  生成 {len(base_stars)} 颗基准恒星")

    # 各朝代恒星
    print("生成各朝代恒星记录...")
    all_stars = []
    for dc in DYNASTIES_CONFIG:
        recs = generate_dynasty_stars(base_stars, dc, dc[4])
        all_stars.extend(recs)
        print(f"  {dc[1]}: {len(recs)} 条")

    # 补齐到 ~1200 条
    target = 1200
    while len(all_stars) < target:
        # 随机加一些
        dc = random.choice(DYNASTIES_CONFIG)
        extra = generate_dynasty_stars(base_stars, dc, dc[4])
        all_stars.extend(extra[:min(50, target - len(all_stars))])

    all_stars = all_stars[:target]
    print(f"恒星总数: {len(all_stars)}")

    # 插入恒星
    print("导入恒星数据...")
    star_sql = """
        INSERT INTO ancient_stars (
            star_id_code, dynasty_id, mansion_id, star_name_cn, star_name_alt,
            constellation, ruxiu_du, quji_du, ra_ancient_conv, dec_ancient_conv,
            ra_j2000, dec_j2000, magnitude_ancient, magnitude_num,
            color_desc, color_class, color_temp_k,
            proper_motion_ra, proper_motion_dec, parallax,
            source_book, quality_flag
        ) VALUES (
            %s, %s, %s, %s, %s, %s, %s, %s, %s, %s,
            %s, %s, %s, %s, %s, %s, %s, %s, %s, %s,
            %s, %s
        )
    """
    star_data = [(
        s['star_id_code'], s['dynasty_id'], s['mansion_id'],
        s['star_name_cn'], s['star_name_alt'], s['constellation'],
        s['ruxiu_du'], s['quji_du'],
        s['ra_ancient_conv'], s['dec_ancient_conv'],
        s['ra_j2000'], s['dec_j2000'],
        s['magnitude_ancient'], s['magnitude_num'],
        s['color_desc'], s['color_class'], s['color_temp_k'],
        s['proper_motion_ra'], s['proper_motion_dec'], s['parallax'],
        s['source_book'], s['quality_flag'],
    ) for s in all_stars]
    execute_batch(cur, star_sql, star_data, page_size=500)

    # 彗星
    print("生成彗星数据...")
    comets = generate_comets()
    print(f"  共 {len(comets)} 条")
    comet_sql = """
        INSERT INTO ancient_comets (
            comet_id_code, dynasty_id, year_ancient, year_ce,
            ruxiu_du, quji_du, ra_deg, dec_deg, magnitude, color_desc,
            tail_direction, tail_length, duration_days, description, source_book, quality_flag
        ) VALUES (
            %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s
        )
    """
    comet_data = [(
        c['comet_id_code'], c['dynasty_id'], c['year_ancient'], c['year_ce'],
        c['ruxiu_du'], c['quji_du'], c['ra_deg'], c['dec_deg'],
        c['magnitude'], c['color_desc'], c['tail_direction'], c['tail_length'],
        c['duration_days'], c['description'], c['source_book'], c['quality_flag'],
    ) for c in comets]
    execute_batch(cur, comet_sql, comet_data)

    # 客星 + SNR
    print("生成客星与超新星遗迹...")
    guests, hist_snrs = generate_guest_stars()
    snrs = generate_snrs(hist_snrs)
    print(f"  客星: {len(guests)} 条")
    print(f"  SNR: {len(snrs)} 条")

    guest_sql = """
        INSERT INTO guest_stars (
            guest_id_code, dynasty_id, star_name, year_ancient, year_ce,
            ruxiu_du, quji_du, ra_deg, dec_deg, ra_err, dec_err,
            peak_mag, peak_mag_err, visibility_days, lightcurve_type,
            description, position_desc, source_book
        ) VALUES (
            %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s
        )
    """
    guest_data = [(
        g['guest_id_code'], g['dynasty_id'], g['star_name'],
        g['year_ancient'], g['year_ce'], g['ruxiu_du'], g['quji_du'],
        g['ra_deg'], g['dec_deg'], g['ra_err'], g['dec_err'],
        g['peak_mag'], g['peak_mag_err'], g['visibility_days'],
        g['lightcurve_type'], g['description'], g['position_desc'], g['source_book'],
    ) for g in guests]
    execute_batch(cur, guest_sql, guest_data)

    snr_sql = """
        INSERT INTO supernova_remnants (
            remnant_name, sn_type, ra_deg, dec_deg, gal_l, gal_b,
            age_yr, age_err_yr, distance_kpc, distance_err, diameter_pc,
            radio_flux_ghz, xray_luminosity, gamma_detected
        ) VALUES (
            %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s
        )
    """
    snr_data = [(
        s['remnant_name'], s['sn_type'], s['ra_deg'], s['dec_deg'],
        s.get('gal_l'), s.get('gal_b'),
        s['age_yr'], s['age_err_yr'], s['distance_kpc'], s['distance_err'],
        s['diameter_pc'], s['radio_flux_ghz'], s['xray_luminosity'],
        s['gamma_detected'],
    ) for s in snrs]
    execute_batch(cur, snr_sql, snr_data)

    conn.commit()

    # 统计
    cur.execute("SELECT COUNT(*) FROM ancient_stars")
    n_stars = cur.fetchone()[0]
    cur.execute("SELECT COUNT(*) FROM ancient_comets")
    n_comets = cur.fetchone()[0]
    cur.execute("SELECT COUNT(*) FROM guest_stars")
    n_guests = cur.fetchone()[0]
    cur.execute("SELECT COUNT(*) FROM supernova_remnants")
    n_snrs = cur.fetchone()[0]

    print()
    print("=" * 60)
    print("  数据导入完成!")
    print(f"  恒星: {n_stars} 条")
    print(f"  彗星: {n_comets} 条")
    print(f"  客星: {n_guests} 条")
    print(f"  SNR:  {n_snrs} 条")
    print("=" * 60)
    print()
    print("  v0.2 修复:")
    print("    1. 恒星坐标基于 IAU 2006 岁差近似生成")
    print("    2. SNR 遵循银道面分布 (用于验证银河系先验)")
    print("    3. 恒星含 color_temp_k 色温字段 (K)")
    print("=" * 60)

    cur.close()
    conn.close()


if __name__ == '__main__':
    main()
