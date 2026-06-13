import math
import random
from dataclasses import dataclass, field
from typing import List, Optional, Tuple

import numpy as np

from .dynasty_config import DynastyConfig
from .star_generator import (
    DEG2RAD, RAD2DEG,
    NGC_NGP_RA_DEG, NGC_NGP_DEC_DEG, NGC_LON_CP_DEG,
    normalize_angle_360,
    galactic_to_equatorial,
    find_mansion,
    j2000_to_ancient_epoch,
)


LIGHTCURVE_TYPES = [
    ("Ia", 0.15, "标准烛光型，峰值锐利，快速下降"),
    ("Ib", 0.10, "氦型，中等峰值，缓慢下降"),
    ("Ic", 0.10, "氢氦缺失型，宽光变"),
    ("IIP", 0.30, "II型平台期，峰值-17，长平台"),
    ("IIL", 0.10, "II型线性衰减，无平台"),
    ("IIn", 0.10, "相互作用型，双峰，长期尾迹"),
    ("IIb", 0.08, "过渡型，双峰"),
    ("超亮", 0.07, "超亮超新星，峰值极亮-21"),
]

FAMOUS_GUEST_STAR_TEMPLATES = [
    {
        "name": "周伯星",
        "description": "有星孛于大辰，西及汉。孛星光芒四射，色赤白，见于氐房之间，二十七日乃灭。",
        "position_desc": "入氐宿初度，去极九十七度半",
    },
    {
        "name": "宋景公客星",
        "description": "荧惑守心，有星孛于东井，色白，长数丈，或竟天。",
        "position_desc": "居心宿之中，去极百一度",
    },
    {
        "name": "秦始皇客星",
        "description": "始皇三十三年，明星出西方，色如火炬，照地赤，经月乃消。",
        "position_desc": "见于娄胃之间，去极八十三度",
    },
    {
        "name": "汉元光客星",
        "description": "元光元年六月，客星见于房。色白，大如瓜，旬余不见。",
        "position_desc": "入房宿二度，去极九十度",
    },
    {
        "name": "汉元始客星",
        "description": "元始二年春，有星孛于紫微宫，历七十余日乃消，占曰'天下有变'。",
        "position_desc": "入紫微垣，近钩陈，去极五十六度",
    },
    {
        "name": "后汉中元客星",
        "description": "中元元年十一月，客星出轩辕，色黄白，长二丈，历五十八日没。",
        "position_desc": "入轩辕十四星之旁，去极七十一度",
    },
    {
        "name": "晋太熙客星",
        "description": "太熙元年四月，客星在紫宫，长三尺，色苍白，占曰'内有兵起'。",
        "position_desc": "见于紫微华盖之下，去极五十五度",
    },
    {
        "name": "北魏天兴客星",
        "description": "天兴五年十月，客星见于太微，色赤，芒角四出，旬有七日不见。",
        "position_desc": "入太微端门之右，去极六十七度",
    },
    {
        "name": "隋开皇客星",
        "description": "开皇十四年三月，客星入北斗魁中，色青白，七日而消。",
        "position_desc": "居北斗天枢旁，去极五十二度",
    },
    {
        "name": "唐贞观客星",
        "description": "贞观二十二年，客星见于太微，犯帝座，色赤如火，历十五日乃没。",
        "position_desc": "入太微之端，近太子星，去极六十五度",
    },
    {
        "name": "唐天祐客星",
        "description": "天祐二年，有星如日，下有赤云，自东南流西北，声如雷，名曰'天鼓'。",
        "position_desc": "入奎宿九度，去极八十三度",
    },
    {
        "name": "宋景德客星",
        "description": "景德三年四月，客星见紫微之东，色青白，芒角森然，如半月状，凡四十二日而没。",
        "position_desc": "近文昌六星，去极五十八度",
    },
    {
        "name": "宋至和客星（天关客星）",
        "description": "至和元年五月己丑，客星晨出东方，守天关，昼见如太白，芒角四出，色赤白，凡见二十三日。",
        "position_desc": "入天关星旁，去极七十七度，芒角赤色，光芒出指天纪",
    },
    {
        "name": "宋治平客星",
        "description": "治平三年正月，客星出营室，色青白，如杯，历一百二十三日乃消。",
        "position_desc": "入室宿初度，去极九十六度",
    },
    {
        "name": "宋元丰客星",
        "description": "元丰五年六月，客星出胃，色白，渐生芒角，长三丈，经三月不见。",
        "position_desc": "入胃宿三度，去极八十四度",
    },
    {
        "name": "宋淳熙客星",
        "description": "淳熙三年八月，有星孛于张，长丈余，历三十日没。色赤白，占曰'边夷有兵'。",
        "position_desc": "入张宿五度，去极七十八度",
    },
    {
        "name": "元至元客星",
        "description": "至元二十八年冬，客星入参，色赤，芒角四射，经月余没。太史奏为'大人忧'。",
        "position_desc": "入参宿左肩，去极百十四度",
    },
    {
        "name": "明洪武客星",
        "description": "洪武十八年九月，客星入斗，色青白，芒角动揺，历四十四日没。",
        "position_desc": "入南斗魁中，去极百十六度",
    },
    {
        "name": "明隆庆客星（第谷新星）",
        "description": "隆庆六年冬十月，客星见东方，光芒赫然，昼见类太白，凡历二百四十日乃没。",
        "position_desc": "近阁道旁星，入奎宿八度，去极八十三度半",
    },
    {
        "name": "明万历客星（开普勒新星）",
        "description": "万历三十二年秋九月，客星出西方，色赤，有尾迹长数尺，经百二十日始消。",
        "position_desc": "入蛇尾星区，近尾宿八度，去极百十二度",
    },
]


@dataclass
class LightCurveParams:
    curve_type: str
    peak_mag: float
    rise_time_days: int
    decay_time_days: int
    plateau_duration_days: int
    tail_slope_mag_per_100d: float
    has_second_peak: bool
    second_peak_delay_days: int
    description: str


@dataclass
class GuestStarEvent:
    guest_id_code: str
    dynasty: str
    star_name: str
    year_ancient: int
    year_ce: float
    month_ancient: Optional[int]
    day_ancient: Optional[int]
    ra_j2000: float
    dec_j2000: float
    ra_ancient_obs: float
    dec_ancient_obs: float
    ruxiu_du: Optional[float]
    quji_du: Optional[float]
    mansion_order: Optional[int]
    mansion_name: Optional[str]
    ra_err_deg: float
    dec_err_deg: float
    peak_mag: float
    peak_mag_err: float
    visibility_days: int
    lightcurve_type: str
    lightcurve: LightCurveParams
    galactic_l: float
    galactic_b: float
    description: str
    position_desc: str
    source_book: str
    is_famous: bool
    historical_text: Optional[str]
    observation_error_deg: float


SPECTRAL_SN_TYPE_MAP = {
    "Ia": ("白", 0.12),
    "Ib": ("青白", 0.10),
    "Ic": ("青白", 0.10),
    "IIP": ("黄白", 0.30),
    "IIL": ("黄", 0.12),
    "IIn": ("赤白", 0.10),
    "IIb": ("橙白", 0.08),
    "超亮": ("金白", 0.08),
}

SOURCE_BOOKS = [
    "史记·天官书",
    "汉书·天文志",
    "后汉书·天文志",
    "晋书·天文志",
    "宋书·天文志",
    "魏书·天象志",
    "隋书·天文志",
    "新唐书·天文志",
    "旧五代史·天文志",
    "宋史·天文志",
    "辽史·历象志",
    "金史·天文志",
    "元史·天文志",
    "明史·天文志",
    "文献通考·象纬考",
    "通志·天文略",
    "开元占经",
    "乙巳占",
    "灵台秘苑",
]


def sample_snr_galactic(rng: random.Random, n_events: int = 1) -> Tuple[np.ndarray, np.ndarray]:
    if n_events == 1:
        theta = 2.0 * math.pi * rng.random()
        r_outer = np.random.exponential(5.0)
        r = min(25.0, r_outer)
        l = normalize_angle_360(theta * RAD2DEG)
        b_sigma = max(0.5, 3.0 * math.exp(-r / 4.0))
        b = rng.gauss(0.0, b_sigma)
        b = max(-20.0, min(20.0, b))
        return np.array([l]), np.array([b])

    rng_np = np.random.RandomState(rng.randint(0, 2**31 - 1))
    theta = 2.0 * math.pi * rng_np.rand(n_events)
    r_outer = rng_np.exponential(5.0, n_events)
    r = np.minimum(25.0, r_outer)
    l = (theta * RAD2DEG) % 360.0
    b_sigma = np.maximum(0.5, 3.0 * np.exp(-r / 4.0))
    b = rng_np.normal(0.0, b_sigma)
    b = np.clip(b, -20.0, 20.0)
    return l, b


def sample_peak_magnitude(rng: random.Random, year_ce: float) -> float:
    magnitude_range = (-3.0, -8.0)

    era_factor = 1.0
    if year_ce < -500:
        era_factor = 0.85
    elif year_ce < 0:
        era_factor = 0.90
    elif year_ce < 500:
        era_factor = 0.95
    elif year_ce < 1000:
        era_factor = 0.98
    else:
        era_factor = 1.0

    u = rng.random() ** era_factor
    return magnitude_range[0] + u * (magnitude_range[1] - magnitude_range[0])


def generate_lightcurve(rng: random.Random, peak_mag: float) -> LightCurveParams:
    total_w = sum(w for _, w, _ in LIGHTCURVE_TYPES)
    r = rng.random() * total_w
    cumsum = 0.0
    curve_type = "IIP"
    curve_desc = ""
    for ct, w, desc in LIGHTCURVE_TYPES:
        cumsum += w
        if r <= cumsum:
            curve_type = ct
            curve_desc = desc
            break

    base_bright = abs(peak_mag)
    bright_factor = max(0.6, min(1.4, base_bright / 5.0))

    type_params = {
        "Ia": (15, 35, 0, 1.2, False, 0),
        "Ib": (18, 50, 0, 0.9, False, 0),
        "Ic": (20, 60, 0, 0.8, True, 30),
        "IIP": (14, 100, 80, 0.5, False, 0),
        "IIL": (12, 70, 0, 0.8, False, 0),
        "IIn": (20, 120, 0, 0.4, True, 60),
        "IIb": (16, 80, 0, 0.7, True, 25),
        "超亮": (30, 200, 150, 0.3, True, 80),
    }

    rise, decay, plateau, tail_slope, has_peak2, peak2_delay = type_params.get(
        curve_type, (14, 80, 50, 0.6, False, 0))

    return LightCurveParams(
        curve_type=curve_type,
        peak_mag=peak_mag,
        rise_time_days=max(5, int(rise * bright_factor + rng.gauss(0, 3))),
        decay_time_days=max(20, int(decay * bright_factor + rng.gauss(0, 10))),
        plateau_duration_days=max(0, int(plateau * bright_factor + rng.gauss(0, 8))),
        tail_slope_mag_per_100d=max(0.1, tail_slope + rng.gauss(0, 0.1)),
        has_second_peak=has_peak2 and rng.random() < 0.6,
        second_peak_delay_days=peak2_delay + rng.randint(-5, 15),
        description=curve_desc,
    )


def pick_source_book(year_ce: float, rng: random.Random) -> str:
    if year_ce < -206:
        pool = ["史记·天官书", "汉书·天文志", "开元占经"]
    elif year_ce < 220:
        pool = ["史记·天官书", "汉书·天文志", "后汉书·天文志", "开元占经", "乙巳占"]
    elif year_ce < 420:
        pool = ["后汉书·天文志", "晋书·天文志", "宋书·天文志", "灵台秘苑"]
    elif year_ce < 589:
        pool = ["宋书·天文志", "魏书·天象志", "隋书·天文志"]
    elif year_ce < 907:
        pool = ["隋书·天文志", "新唐书·天文志", "旧五代史·天文志", "开元占经", "乙巳占"]
    elif year_ce < 1279:
        pool = ["新唐书·天文志", "宋史·天文志", "辽史·历象志", "金史·天文志", "文献通考·象纬考"]
    else:
        pool = ["元史·天文志", "明史·天文志", "宋史·天文志", "文献通考·象纬考", "通志·天文略"]

    weights = [1.0 / (i + 1) for i in range(len(pool))]
    total = sum(weights)
    r = rng.random() * total
    cumsum = 0.0
    for book, w in zip(pool, weights):
        cumsum += w
        if r <= cumsum:
            return book
    return pool[0]


def generate_historical_text(rng: random.Random, peak_mag: float, curve_type: str,
                              dynasty: str, mansion_name: Optional[str]) -> Tuple[str, str]:
    brightness = abs(peak_mag)
    if brightness >= 7:
        bright_desc = "昼见如太白"
        vis_class = "非常明亮"
    elif brightness >= 6:
        bright_desc = "大如半月，光芒赫然"
        vis_class = "极亮"
    elif brightness >= 5:
        bright_desc = "明如大星，芒角四出"
        vis_class = "明亮"
    elif brightness >= 4:
        bright_desc = "色青白，大如杯"
        vis_class = "较亮"
    else:
        bright_desc = "色苍白，类明星"
        vis_class = "一般"

    color_map, _ = SPECTRAL_SN_TYPE_MAP.get(curve_type, ("白", 0.1))

    vis_days_min = 20
    vis_days_max = 100
    if curve_type == "超亮":
        vis_days_min, vis_days_max = 180, 300
    elif curve_type == "IIP":
        vis_days_min, vis_days_max = 80, 150
    elif curve_type == "Ia":
        vis_days_min, vis_days_max = 30, 60

    vis_days = rng.randint(vis_days_min, vis_days_max)

    mansion_part = ""
    if mansion_name:
        mansion_part = f"见于{ mansion_name }宿"

    templates_common = [
        f"{ dynasty }某年某月，客星{mansion_part}，{ bright_desc }，色{ color_map }，历{ vis_days }日乃没。",
        f"有星孛{mansion_part}，{ bright_desc }，色{ color_map }，尾迹数尺，凡见{ vis_days }日。",
        f"客星出{mansion_part}，芒角森然，{ bright_desc }，{ vis_class }，经{ vis_days }日而消。",
        f"异星见{mansion_part}，{ bright_desc }，或有尾迹，太史奏占，历{ vis_days }日始伏。",
    ]

    return rng.choice(templates_common), mansion_part if mansion_part else "无详细位置描述"


class TransientGenerator:
    def __init__(self, base_seed: Optional[int] = None):
        self.base_seed = base_seed if base_seed is not None else 42
        self._famous_used: set = set()

    def generate_guest_stars(
        self,
        dynasty_config: DynastyConfig,
        n_events: int,
        seed: Optional[int] = None,
    ) -> List[GuestStarEvent]:
        rng = random.Random(seed if seed is not None else self.base_seed)

        lat = dynasty_config.capital_latitude
        max_z = dynasty_config.max_observable_zenith_dist
        epoch_base = dynasty_config.mid_year
        start_yr = dynasty_config.start_year
        end_yr = dynasty_config.end_year

        results: List[GuestStarEvent] = []
        attempts = 0
        max_attempts = n_events * 30

        while len(results) < n_events and attempts < max_attempts:
            batch_size = min((n_events - len(results)) * 2, 50)
            l_arr, b_arr = sample_snr_galactic(rng, batch_size)

            for i in range(batch_size):
                if len(results) >= n_events:
                    break
                attempts += 1

                l = float(l_arr[i])
                b = float(b_arr[i])

                ra_j2000, dec_j2000 = galactic_to_equatorial(l, b)

                year_ce = rng.uniform(start_yr, end_yr)
                epoch_yr = year_ce

                ra_ancient_t, dec_ancient_t = j2000_to_ancient_epoch(ra_j2000, dec_j2000, epoch_yr)

                obs_err = dynasty_config.accuracy_error_deg
                snr_ra_err = max(0.2, obs_err * 1.2)
                snr_dec_err = max(0.2, obs_err * 1.0)
                ra_err = rng.gauss(0.0, snr_ra_err)
                dec_err = rng.gauss(0.0, snr_dec_err)
                ra_obs = normalize_angle_360(ra_ancient_t + ra_err)
                dec_obs = dec_ancient_t + dec_err

                zenith_dist = 90.0 - (dec_obs - lat)
                if zenith_dist > max_z:
                    continue
                alt = 90.0 - zenith_dist
                if alt < 8.0:
                    continue

                peak_mag = sample_peak_magnitude(rng, year_ce)
                peak_mag_err = max(0.3, 0.8 - zenith_dist / 200.0)
                lc = generate_lightcurve(rng, peak_mag)

                vis_total = (lc.rise_time_days + lc.decay_time_days +
                             lc.plateau_duration_days +
                             (lc.second_peak_delay_days if lc.has_second_peak else 0))

                mansion_order, mansion_name, ruxiu_du = find_mansion(ra_obs)
                quji_du = 90.0 - dec_obs

                is_famous = rng.random() < 0.05
                hist_text = None
                pos_desc_text = None
                star_name = ""

                if is_famous and len(self._famous_used) < len(FAMOUS_GUEST_STAR_TEMPLATES):
                    available = [i for i in range(len(FAMOUS_GUEST_STAR_TEMPLATES))
                                 if i not in self._famous_used]
                    if available:
                        idx = rng.choice(available)
                        self._famous_used.add(idx)
                        template = FAMOUS_GUEST_STAR_TEMPLATES[idx]
                        star_name = template["name"]
                        hist_text = template["description"]
                        pos_desc_text = template["position_desc"]
                else:
                    is_famous = False

                if hist_text is None:
                    hist_text, pos_desc_text = generate_historical_text(
                        rng, peak_mag, lc.curve_type,
                        dynasty_config.name_cn, mansion_name)

                if not star_name:
                    naming = [
                        f"{ dynasty_config.name_cn }客星{ len(results) + 1 }",
                        f"{ mansion_name if mansion_name else '无名' }客星",
                        f"孛星{ len(results) + 1 }",
                        f"异星{ len(results) + 1 }",
                    ]
                    star_name = rng.choice(naming)

                source_book = pick_source_book(year_ce, rng)

                year_ancient = int(round(year_ce))
                month_ancient = rng.randint(1, 12) if rng.random() < 0.8 else None
                day_ancient = rng.randint(1, 28) if month_ancient is not None and rng.random() < 0.6 else None

                event_idx = len(results) + 1
                guest_id = f"SNR-SIM-{dynasty_config.name.upper()}-{event_idx:04d}"

                results.append(GuestStarEvent(
                    guest_id_code=guest_id,
                    dynasty=dynasty_config.name,
                    star_name=star_name,
                    year_ancient=year_ancient,
                    year_ce=year_ce,
                    month_ancient=month_ancient,
                    day_ancient=day_ancient,
                    ra_j2000=ra_j2000,
                    dec_j2000=dec_j2000,
                    ra_ancient_obs=ra_obs,
                    dec_ancient_obs=dec_obs,
                    ruxiu_du=ruxiu_du,
                    quji_du=quji_du,
                    mansion_order=mansion_order,
                    mansion_name=mansion_name,
                    ra_err_deg=snr_ra_err,
                    dec_err_deg=snr_dec_err,
                    peak_mag=peak_mag,
                    peak_mag_err=peak_mag_err,
                    visibility_days=vis_total,
                    lightcurve_type=lc.curve_type,
                    lightcurve=lc,
                    galactic_l=l,
                    galactic_b=b,
                    description=hist_text,
                    position_desc=pos_desc_text,
                    source_book=source_book,
                    is_famous=is_famous,
                    historical_text=hist_text,
                    observation_error_deg=obs_err,
                ))

        return results
