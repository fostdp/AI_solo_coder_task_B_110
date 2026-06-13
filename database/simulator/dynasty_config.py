from dataclasses import dataclass, field
from typing import Dict, List, Optional


@dataclass
class DynastyConfig:
    name: str
    name_cn: str
    start_year: int
    end_year: int
    capital_latitude: float
    sudu_version: str
    accuracy_error_deg: float
    typical_asterism_count: int
    color_terms: List[str]
    description: str = ""
    capital_name: str = ""
    max_observable_zenith_dist: float = 90.0

    @property
    def mid_year(self) -> float:
        return (self.start_year + self.end_year) / 2.0

    @property
    def duration_years(self) -> int:
        return self.end_year - self.start_year


DYNASTIES: Dict[str, DynastyConfig] = {}


def _register(config: DynastyConfig) -> None:
    DYNASTIES[config.name] = config


_register(DynastyConfig(
    name="xia",
    name_cn="夏",
    start_year=-2070,
    end_year=-1600,
    capital_latitude=35.0,
    capital_name="阳城/安邑",
    sudu_version="古六历",
    accuracy_error_deg=3.0,
    typical_asterism_count=80,
    color_terms=["赤", "黄", "苍", "白", "黑", "青", "素", "朱", "玄"],
    description="夏朝，传说中的第一个世袭制朝代，星象记载原始，观测误差较大",
    max_observable_zenith_dist=85.0,
))

_register(DynastyConfig(
    name="shang",
    name_cn="商",
    start_year=-1600,
    end_year=-1046,
    capital_latitude=36.0,
    capital_name="殷/亳",
    sudu_version="古六历",
    accuracy_error_deg=2.5,
    typical_asterism_count=100,
    color_terms=["赤", "黄", "苍", "白", "黑", "青", "朱"],
    description="商朝，甲骨文出现星象占卜，观测记录逐渐系统化",
    max_observable_zenith_dist=86.0,
))

_register(DynastyConfig(
    name="zhou",
    name_cn="周",
    start_year=-1046,
    end_year=-256,
    capital_latitude=34.5,
    capital_name="镐京/洛邑",
    sudu_version="古六历",
    accuracy_error_deg=2.0,
    typical_asterism_count=120,
    color_terms=["赤", "黄", "苍", "白", "黑", "青", "朱", "玄", "素"],
    description="周朝（西周+东周），确立二十八宿体系雏形",
    max_observable_zenith_dist=87.0,
))

_register(DynastyConfig(
    name="qin",
    name_cn="秦",
    start_year=-221,
    end_year=-207,
    capital_latitude=34.3,
    capital_name="咸阳",
    sudu_version="颛顼历",
    accuracy_error_deg=1.8,
    typical_asterism_count=140,
    color_terms=["赤", "黄", "苍", "白", "黑", "青"],
    description="秦朝，统一度量衡，天文观测制度初步统一",
    max_observable_zenith_dist=87.5,
))

_register(DynastyConfig(
    name="han_western",
    name_cn="西汉",
    start_year=-202,
    end_year=9,
    capital_latitude=34.3,
    capital_name="长安",
    sudu_version="太初历",
    accuracy_error_deg=1.5,
    typical_asterism_count=180,
    color_terms=["赤", "黄", "苍", "白", "黑", "青", "朱", "紫"],
    description="西汉，太初历确立二十八宿精确宿度，星官体系大成",
    max_observable_zenith_dist=88.0,
))

_register(DynastyConfig(
    name="han_eastern",
    name_cn="东汉",
    start_year=25,
    end_year=220,
    capital_latitude=34.6,
    capital_name="洛阳",
    sudu_version="太初历",
    accuracy_error_deg=1.2,
    typical_asterism_count=200,
    color_terms=["赤", "黄", "苍", "白", "黑", "青", "朱", "紫"],
    description="东汉，张衡等天文学家辈出，浑天说发展",
    max_observable_zenith_dist=88.0,
))

_register(DynastyConfig(
    name="three_kingdoms",
    name_cn="三国",
    start_year=220,
    end_year=280,
    capital_latitude=34.0,
    capital_name="洛阳/建业/成都",
    sudu_version="太初历",
    accuracy_error_deg=1.2,
    typical_asterism_count=220,
    color_terms=["赤", "黄", "苍", "白", "黑", "青"],
    description="三国时期，战乱频繁但天文观测延续汉代传统",
    max_observable_zenith_dist=87.5,
))

_register(DynastyConfig(
    name="jin",
    name_cn="晋",
    start_year=266,
    end_year=420,
    capital_latitude=33.5,
    capital_name="洛阳/建康",
    sudu_version="太初历",
    accuracy_error_deg=1.1,
    typical_asterism_count=240,
    color_terms=["赤", "黄", "苍", "白", "黑", "青", "朱"],
    description="两晋时期，东晋南迁，观测纬度略南移",
    max_observable_zenith_dist=86.5,
))

_register(DynastyConfig(
    name="northern_southern",
    name_cn="南北朝",
    start_year=420,
    end_year=589,
    capital_latitude=33.0,
    capital_name="建康/平城",
    sudu_version="太初历",
    accuracy_error_deg=1.0,
    typical_asterism_count=260,
    color_terms=["赤", "黄", "苍", "白", "黑", "青"],
    description="南北朝，南北分立，天文交流融合",
    max_observable_zenith_dist=86.0,
))

_register(DynastyConfig(
    name="sui",
    name_cn="隋",
    start_year=581,
    end_year=618,
    capital_latitude=34.3,
    capital_name="大兴",
    sudu_version="皇极历",
    accuracy_error_deg=0.9,
    typical_asterism_count=280,
    color_terms=["赤", "黄", "苍", "白", "黑", "青", "朱", "金"],
    description="隋朝，重新统一，为唐代天文高峰奠定基础",
    max_observable_zenith_dist=88.0,
))

_register(DynastyConfig(
    name="tang",
    name_cn="唐",
    start_year=618,
    end_year=907,
    capital_latitude=34.3,
    capital_name="长安",
    sudu_version="大衍历",
    accuracy_error_deg=0.7,
    typical_asterism_count=300,
    color_terms=["赤", "黄", "苍", "白", "黑", "青", "朱", "紫", "金", "银"],
    description="唐朝，大衍历精确测量二十八宿距度，一行实测",
    max_observable_zenith_dist=88.5,
))

_register(DynastyConfig(
    name="song",
    name_cn="宋",
    start_year=960,
    end_year=1279,
    capital_latitude=32.5,
    capital_name="开封/临安",
    sudu_version="大衍历",
    accuracy_error_deg=0.5,
    typical_asterism_count=320,
    color_terms=["赤", "黄", "苍", "白", "黑", "青", "朱", "紫", "明"],
    description="宋朝，宋代星图精确，客星记录详尽",
    max_observable_zenith_dist=87.0,
))

_register(DynastyConfig(
    name="yuan",
    name_cn="元",
    start_year=1271,
    end_year=1368,
    capital_latitude=39.9,
    capital_name="大都",
    sudu_version="授时历",
    accuracy_error_deg=0.3,
    typical_asterism_count=340,
    color_terms=["赤", "黄", "苍", "白", "黑", "青", "朱", "紫"],
    description="元朝，郭守敬授时历，简仪等精密仪器，观测精度极高",
    max_observable_zenith_dist=90.0,
))

_register(DynastyConfig(
    name="ming",
    name_cn="明",
    start_year=1368,
    end_year=1644,
    capital_latitude=39.9,
    capital_name="北京/南京",
    sudu_version="授时历",
    accuracy_error_deg=0.3,
    typical_asterism_count=360,
    color_terms=["赤", "黄", "苍", "白", "黑", "青", "朱", "紫", "金"],
    description="明朝，沿用授时历体系，传统天文学延续发展",
    max_observable_zenith_dist=90.0,
))


def get_dynasty_config(name: str) -> Optional[DynastyConfig]:
    return DYNASTIES.get(name.lower())


def get_all_dynasties() -> List[DynastyConfig]:
    return list(DYNASTIES.values())


def list_dynasty_names() -> List[str]:
    return list(DYNASTIES.keys())
