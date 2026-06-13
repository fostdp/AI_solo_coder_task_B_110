import math
import random
from dataclasses import dataclass, field
from typing import List, Optional, Tuple

import numpy as np

from .dynasty_config import DynastyConfig


DEG2RAD = math.pi / 180.0
RAD2DEG = 180.0 / math.pi

JD2000 = 2451545.0
JULIAN_CENTURY = 36525.0

NGC_NGP_RA_DEG = 192.8595
NGC_NGP_DEC_DEG = 27.1284
NGC_LON_CP_DEG = 122.9320

ZETA_A_COEFFS = [0.0, 2.5976176, 0.01015026, -0.00002587, -0.000000022, 0.00000000006]
THETA_A_COEFFS = [0.0, 2004.3109, -0.8533041, -0.00021547, -0.000000195, -0.00000000017]
Z_A_COEFFS = [0.0, -2.5976176, 0.01015026, 0.00002562, -0.000000023, -0.00000000003]
OMEGA_A_COEFFS = [84381.406, -46.836769, -0.000183, 0.002003, -0.000000576, -0.0000000434]
CHI_A_COEFFS = [0.0, 10.5526, -2.38064, -0.001211, 0.000170, -0.0000000009]


LUNAR_MANSIONS = [
    (1, "角", 12.0, 189.5),
    (2, "亢", 9.0, 201.5),
    (3, "氐", 15.0, 210.5),
    (4, "房", 5.0, 225.5),
    (5, "心", 5.0, 230.5),
    (6, "尾", 18.0, 235.5),
    (7, "箕", 11.0, 253.5),
    (8, "斗", 26.0, 264.5),
    (9, "牛", 8.0, 290.5),
    (10, "女", 12.0, 298.5),
    (11, "虚", 10.0, 310.5),
    (12, "危", 17.0, 320.5),
    (13, "室", 16.0, 337.5),
    (14, "壁", 9.0, 353.5),
    (15, "奎", 16.0, 362.5),
    (16, "娄", 12.0, 18.5),
    (17, "胃", 14.0, 30.5),
    (18, "昴", 11.0, 44.5),
    (19, "毕", 16.0, 55.5),
    (20, "觜", 3.0, 71.5),
    (21, "参", 9.0, 74.5),
    (22, "井", 33.0, 83.5),
    (23, "鬼", 4.0, 116.5),
    (24, "柳", 15.0, 120.5),
    (25, "星", 7.0, 135.5),
    (26, "张", 6.0, 142.5),
    (27, "翼", 18.0, 148.5),
    (28, "轸", 5.0, 166.5),
]

MAG_DISTRIBUTION = [
    (0.0, 0.01),
    (1.0, 0.03),
    (2.0, 0.08),
    (3.0, 0.15),
    (4.0, 0.25),
    (5.0, 0.28),
    (6.0, 0.15),
    (7.0, 0.04),
    (8.0, 0.01),
]

COLOR_BV_TO_TEMP = {
    "O": (-0.33, 33000, "青", "蓝白"),
    "B": (-0.18, 21000, "青", "青白"),
    "A": (0.00, 9500, "白", "白"),
    "F": (0.33, 7240, "白", "黄白"),
    "G": (0.58, 5920, "黄", "黄"),
    "K": (0.89, 5300, "橙", "橙"),
    "M": (1.45, 3850, "赤", "红"),
}

SPECTRAL_TYPE_WEIGHTS = [
    ("O", 0.003),
    ("B", 0.03),
    ("A", 0.06),
    ("F", 0.15),
    ("G", 0.20),
    ("K", 0.35),
    ("M", 0.207),
]


@dataclass
class SimulatedStar:
    star_id_code: str
    dynasty: str
    ra_j2000: float
    dec_j2000: float
    ra_ancient: float
    dec_ancient: float
    ruxiu_du: Optional[float]
    quji_du: Optional[float]
    mansion_order: Optional[int]
    mansion_name: Optional[str]
    magnitude_num: float
    magnitude_ancient: str
    color_desc: str
    color_class: str
    color_temp_k: float
    spectral_type: str
    proper_motion_ra: float
    proper_motion_dec: float
    galactic_l: float
    galactic_b: float
    observation_error_deg: float


def normalize_angle_360(deg: float) -> float:
    a = deg % 360.0
    if a < 0:
        a += 360.0
    return a


def normalize_angle_180(deg: float) -> float:
    a = (deg + 180.0) % 360.0
    if a < 0:
        a += 360.0
    return a - 180.0


def equatorial_to_galactic(ra_deg: float, dec_deg: float) -> Tuple[float, float]:
    ra = ra_deg * DEG2RAD
    dec = dec_deg * DEG2RAD
    ngp_ra = NGC_NGP_RA_DEG * DEG2RAD
    ngp_dec = NGC_NGP_DEC_DEG * DEG2RAD
    lon_cp = NGC_LON_CP_DEG * DEG2RAD

    sin_b = math.sin(dec) * math.sin(ngp_dec) + math.cos(dec) * math.cos(ngp_dec) * math.cos(ra - ngp_ra)
    b = math.asin(sin_b)

    y = math.sin(dec) * math.cos(ngp_dec) - math.cos(dec) * math.sin(ngp_dec) * math.cos(ra - ngp_ra)
    x = -math.cos(dec) * math.sin(ra - ngp_ra)
    l = math.atan2(y, x) + lon_cp

    return normalize_angle_360(l * RAD2DEG), b * RAD2DEG


def galactic_to_equatorial(l_deg: float, b_deg: float) -> Tuple[float, float]:
    l = l_deg * DEG2RAD
    b = b_deg * DEG2RAD
    ngp_ra = NGC_NGP_RA_DEG * DEG2RAD
    ngp_dec = NGC_NGP_DEC_DEG * DEG2RAD
    lon_cp = NGC_LON_CP_DEG * DEG2RAD

    l_adj = l - lon_cp
    sin_dec = math.sin(b) * math.sin(ngp_dec) + math.cos(b) * math.cos(ngp_dec) * math.cos(l_adj)
    dec = math.asin(sin_dec)

    y = -math.cos(b) * math.sin(l_adj)
    x = math.sin(b) * math.cos(ngp_dec) - math.cos(b) * math.sin(ngp_dec) * math.cos(l_adj)
    ra = math.atan2(y, x) + ngp_ra

    return normalize_angle_360(ra * RAD2DEG), dec * RAD2DEG


def poly(coeffs: List[float], t: float) -> float:
    s = 0.0
    tp = 1.0
    for c in coeffs:
        s += c * tp
        tp *= t
    return s


def iau2006_precession_angles(t: float) -> Tuple[float, float, float, float]:
    AS2RAD = math.pi / (180.0 * 3600.0)
    zeta_a = poly(ZETA_A_COEFFS, t) * AS2RAD
    theta_a = poly(THETA_A_COEFFS, t) * AS2RAD
    z_a = poly(Z_A_COEFFS, t) * AS2RAD
    omega_a = poly(OMEGA_A_COEFFS, t) * AS2RAD
    return zeta_a, theta_a, z_a, omega_a


def planetary_precession_chi(t: float) -> float:
    AS2RAD = math.pi / (180.0 * 3600.0)
    return poly(CHI_A_COEFFS, t) * AS2RAD


def eye() -> List[List[float]]:
    return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]


def mat_mul(a: List[List[float]], b: List[List[float]]) -> List[List[float]]:
    r = [[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]]
    for i in range(3):
        for j in range(3):
            r[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j]
    return r


def mat_mul3(a, b, c):
    return mat_mul(mat_mul(a, b), c)


def mat_vec(m: List[List[float]], v: List[float]) -> List[float]:
    return [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]


def rot_x(alpha: float) -> List[List[float]]:
    c = math.cos(alpha)
    s = math.sin(alpha)
    return [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]


def rot_z(alpha: float) -> List[List[float]]:
    c = math.cos(alpha)
    s = math.sin(alpha)
    return [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]


def precession_matrix_t_from_j2000(t: float) -> List[List[float]]:
    zeta_a, theta_a, z_a, _ = iau2006_precession_angles(t)
    rz_1 = rot_z(z_a)
    rx = rot_x(-theta_a)
    rz_2 = rot_z(zeta_a)
    return mat_mul3(rz_1, rx, rz_2)


def iau2000b_nutation(t: float) -> Tuple[float, float]:
    MAS2RAD = math.pi / (180.0 * 3600.0 * 1000.0)
    AS2RAD = math.pi / (180.0 * 3600.0)

    l_arcsec = 485866.733 + 1717915922.633 * t
    lp_arcsec = 1287099.804 + 129596581.224 * t
    f_arcsec = 335778.877 + 1739527262.847 * t
    d_arcsec = 1072261.307 + 1602961601.029 * t
    om_arcsec = 450160.280 - 6962890.266 * t

    terms = [
        [0.0, 0.0, -2.0, 2.0, -2.0, -172064161.0, -13170906.0, 2062.0, 8556.0],
        [0.0, 0.0, -2.0, 0.0, 1.0, -13187.0, -1369.0, 8166.0, 573036.0],
        [1.0, 0.0, 0.0, 0.0, 0.0, -2274.0, -207.0, 3424.0, 2798.0],
        [0.0, 1.0, 0.0, 0.0, 0.0, 2062.0, 189.0, -24.0, -65.0],
        [-2.0, 0.0, -2.0, 2.0, -2.0, 1426.0, 123.0, -517.0, -301.0],
    ]

    dpsi_mas = 0.0
    deps_mas = 0.0
    for term in terms:
        arg = (term[0] * l_arcsec + term[1] * lp_arcsec + term[2] * f_arcsec +
               term[3] * d_arcsec + term[4] * om_arcsec) * AS2RAD
        sin_a = math.sin(arg)
        cos_a = math.cos(arg)
        dpsi_mas += term[5] * sin_a + term[7] * cos_a
        deps_mas += term[6] * cos_a + term[8] * sin_a

    return dpsi_mas * MAS2RAD, deps_mas * MAS2RAD


def nutation_matrix(omega_a: float, dpsi: float, deps: float) -> List[List[float]]:
    r1 = rot_x(omega_a)
    r3 = rot_z(-dpsi)
    r2 = rot_x(-(omega_a + deps))
    return mat_mul3(r2, r3, r1)


def j2000_to_ancient_epoch(ra_j2000: float, dec_j2000: float, epoch_year: float) -> Tuple[float, float]:
    t_centuries = (epoch_year - 2000.0) / 100.0

    ra_r = ra_j2000 * DEG2RAD
    dec_r = dec_j2000 * DEG2RAD
    v = [
        math.cos(dec_r) * math.cos(ra_r),
        math.cos(dec_r) * math.sin(ra_r),
        math.sin(dec_r),
    ]

    _, _, _, omega_a = iau2006_precession_angles(t_centuries)
    dpsi, deps = iau2000b_nutation(t_centuries)

    p_inv = precession_matrix_t_from_j2000(t_centuries)
    n_inv = nutation_matrix(omega_a, -dpsi, -deps)
    pp = rot_x(-planetary_precession_chi(t_centuries))

    v1 = mat_vec(p_inv, v)
    v2 = mat_vec(pp, v1)
    v3 = mat_vec(n_inv, v2)

    dec_t = math.asin(v3[2]) * RAD2DEG
    ra_t = math.atan2(v3[1], v3[0]) * RAD2DEG
    return normalize_angle_360(ra_t), dec_t


def find_mansion(ra_deg: float) -> Tuple[int, str, float]:
    best_idx = 0
    best_offset = 0.0
    best_name = ""
    for idx, (order, name, width, ra_start) in enumerate(LUNAR_MANSIONS):
        ra_end = ra_start + width
        ra_norm = normalize_angle_360(ra_deg)
        if ra_start > ra_end:
            if ra_norm >= ra_start or ra_norm < ra_end - 360.0:
                offset = normalize_angle_360(ra_norm - ra_start)
                return order, name, offset
        else:
            if ra_start <= ra_norm < ra_end:
                offset = ra_norm - ra_start
                return order, name, offset

    order, name, width, ra_start = LUNAR_MANSIONS[0]
    offset = normalize_angle_360(ra_deg - ra_start)
    return order, name, offset


def sample_from_weighted(items, rng: random.Random):
    total = sum(w for _, w in items)
    r = rng.random() * total
    cumsum = 0.0
    for item, w in items:
        cumsum += w
        if r <= cumsum:
            return item
    return items[-1][0]


def ancient_magnitude_desc(mag: float) -> str:
    if mag < 1.0:
        return "大星"
    elif mag < 2.0:
        return "明大"
    elif mag < 3.5:
        return "明"
    elif mag < 5.0:
        return "中等"
    else:
        return "暗小"


def map_color_to_dynasty(spectral_type: str, dynasty_terms: List[str], rng: random.Random) -> Tuple[str, str]:
    bv, temp, cn_color, cn_color2 = COLOR_BV_TO_TEMP[spectral_type]

    if cn_color in dynasty_terms:
        color_desc = cn_color
    elif cn_color2 in dynasty_terms:
        color_desc = cn_color2
    else:
        aliases = {
            "青": ["苍", "玄", "素"],
            "蓝白": ["白", "青"],
            "青白": ["白", "青"],
            "白": ["素", "明"],
            "黄白": ["黄", "白"],
            "黄": ["金"],
            "橙": ["黄", "赤"],
            "赤": ["朱", "玄"],
            "红": ["赤", "朱"],
        }
        candidates = aliases.get(cn_color, [dynasty_terms[0]])
        valid = [c for c in candidates if c in dynasty_terms]
        color_desc = valid[0] if valid else dynasty_terms[0]

    if rng.random() < 0.3 and len(dynasty_terms) > 3:
        alt_choices = [c for c in dynasty_terms if c not in [color_desc, cn_color, cn_color2]]
        if alt_choices:
            color_desc = rng.choice(alt_choices[:2])

    return color_desc, spectral_type


def generate_galactic_position(rng: random.Random, n_stars: int = 1) -> Tuple[np.ndarray, np.ndarray]:
    if n_stars == 1:
        u = rng.random()
        v = rng.random()
        r = math.sqrt(-2.0 * math.log(max(u, 1e-10))) * 8.0
        theta = 2.0 * math.pi * v
        l = normalize_angle_360(theta * RAD2DEG)
        b_sigma = min(8.0, max(2.0, 10.0 / (r + 1.0)))
        b = rng.gauss(0.0, b_sigma)
        b = max(-80.0, min(80.0, b))
        return np.array([l]), np.array([b])

    rng_np = np.random.RandomState(rng.randint(0, 2**31 - 1))
    u = rng_np.rand(n_stars)
    v = rng_np.rand(n_stars)
    r = np.sqrt(-2.0 * np.log(np.maximum(u, 1e-10))) * 8.0
    theta = 2.0 * math.pi * v
    l = (theta * RAD2DEG) % 360.0
    b_sigma = np.minimum(8.0, np.maximum(2.0, 10.0 / (r + 1.0)))
    b = rng_np.normal(0.0, b_sigma)
    b = np.clip(b, -80.0, 80.0)
    return l, b


class StarGenerator:
    def __init__(self, base_seed: Optional[int] = None):
        self.base_seed = base_seed if base_seed is not None else 42

    def generate_stars_for_dynasty(
        self,
        dynasty_config: DynastyConfig,
        n_stars: int,
        seed: Optional[int] = None,
    ) -> List[SimulatedStar]:
        rng = random.Random(seed if seed is not None else self.base_seed)

        lat = dynasty_config.capital_latitude
        max_z = dynasty_config.max_observable_zenith_dist
        epoch_yr = dynasty_config.mid_year

        results: List[SimulatedStar] = []
        attempts = 0
        max_attempts = n_stars * 50

        while len(results) < n_stars and attempts < max_attempts:
            batch_size = min((n_stars - len(results)) * 3, 100)
            l_arr, b_arr = generate_galactic_position(rng, batch_size)

            for i in range(batch_size):
                if len(results) >= n_stars:
                    break
                attempts += 1

                l = float(l_arr[i])
                b = float(b_arr[i])

                ra_j2000, dec_j2000 = galactic_to_equatorial(l, b)

                ra_ancient_t, dec_ancient_t = j2000_to_ancient_epoch(ra_j2000, dec_j2000, epoch_yr)

                obs_err = dynasty_config.accuracy_error_deg
                ra_err = rng.gauss(0.0, obs_err)
                dec_err = rng.gauss(0.0, obs_err * 0.8)
                ra_obs = normalize_angle_360(ra_ancient_t + ra_err)
                dec_obs = dec_ancient_t + dec_err

                zenith_dist = 90.0 - (dec_obs - lat)
                if zenith_dist > max_z:
                    continue

                alt = 90.0 - zenith_dist
                if alt < 5.0:
                    continue

                mag_val = sample_from_weighted(MAG_DISTRIBUTION, rng)
                mag_jitter = rng.gauss(0.0, 0.3)
                magnitude_num = max(-1.5, min(8.5, mag_val + mag_jitter))

                if magnitude_num > 6.5:
                    extinction_extra = max(0.0, (zenith_dist - 60.0) / 30.0) * 1.5
                    if rng.random() < 0.5 + extinction_extra:
                        continue

                spectral = sample_from_weighted(SPECTRAL_TYPE_WEIGHTS, rng)
                _, color_temp_k, _, _ = COLOR_BV_TO_TEMP[spectral]

                color_desc, _ = map_color_to_dynasty(spectral, dynasty_config.color_terms, rng)

                pm_ra = rng.gauss(0.0, 5.0)
                pm_dec = rng.gauss(0.0, 5.0)

                mansion_order, mansion_name, ruxiu_du = find_mansion(ra_obs)
                quji_du = 90.0 - dec_obs

                star_idx = len(results) + 1
                star_id = f"SIM-{dynasty_config.name.upper()}-{star_idx:06d}"

                mag_ancient = ancient_magnitude_desc(magnitude_num)

                results.append(SimulatedStar(
                    star_id_code=star_id,
                    dynasty=dynasty_config.name,
                    ra_j2000=ra_j2000,
                    dec_j2000=dec_j2000,
                    ra_ancient=ra_obs,
                    dec_ancient=dec_obs,
                    ruxiu_du=ruxiu_du,
                    quji_du=quji_du,
                    mansion_order=mansion_order,
                    mansion_name=mansion_name,
                    magnitude_num=magnitude_num,
                    magnitude_ancient=mag_ancient,
                    color_desc=color_desc,
                    color_class=spectral,
                    color_temp_k=color_temp_k + rng.gauss(0.0, color_temp_k * 0.05),
                    spectral_type=spectral,
                    proper_motion_ra=pm_ra,
                    proper_motion_dec=pm_dec,
                    galactic_l=l,
                    galactic_b=b,
                    observation_error_deg=obs_err,
                ))

        return results
