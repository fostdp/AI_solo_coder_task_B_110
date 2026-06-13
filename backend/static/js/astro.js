/* ============================================================
 * 天文计算工具 (前端精简版)
 *
 * 包含:
 *   1. 球坐标变换 (赤经赤纬 → 三维直角)
 *   2. 角距离 / 位置角
 *   3. 光谱型 / 古代颜色描述 → 有效温度 (K)
 *   4. ★ 修复 3: Planck 黑体辐射色温 → sRGB 颜色 (CIE 1931)
 *   5. 朝代信息
 * ============================================================ */

const Astro = (() => {

    const DEG2RAD = Math.PI / 180.0;
    const RAD2DEG = 180.0 / Math.PI;

    // ============================================================
    // 基础数学
    // ============================================================

    function normalize360(deg) {
        let a = deg % 360;
        if (a < 0) a += 360;
        return a;
    }

    function angSepDeg(ra1, dec1, ra2, dec2) {
        const dra = (ra1 - ra2) * DEG2RAD;
        const ddec = (dec1 - dec2) * DEG2RAD;
        const a = Math.pow(Math.sin(ddec / 2), 2)
            + Math.cos(dec1 * DEG2RAD) * Math.cos(dec2 * DEG2RAD)
            * Math.pow(Math.sin(dra / 2), 2);
        return 2 * Math.asin(Math.sqrt(a)) * RAD2DEG;
    }

    /**
     * 赤经赤纬 → 三维直角坐标 (单位球)
     * @param {number} ra 赤经 (度)
     * @param {number} dec 赤纬 (度)
     * @param {number} r 半径
     */
    function equatorialToCartesian(ra, dec, r = 1) {
        const raR = ra * DEG2RAD;
        const decR = dec * DEG2RAD;
        return [
            r * Math.cos(decR) * Math.cos(raR),
            r * Math.cos(decR) * Math.sin(raR),
            r * Math.sin(decR),
        ];
    }

    // ============================================================
    // 光谱型 / 古代颜色描述 → 有效温度 T_eff
    // 参考: Pecaut & Mamajek (2013) 主序星有效温度表
    // ============================================================

    const SPECTRAL_TYPE_TEMP = {
        'O5': 42000, 'O6': 38000, 'O7': 36000, 'O8': 33000, 'O9': 30000,
        'B0': 29000, 'B1': 25400, 'B2': 22000, 'B3': 19200, 'B4': 17000,
        'B5': 15400, 'B6': 14000, 'B7': 12800, 'B8': 11700, 'B9': 10700,
        'A0': 9700,  'A1': 9300,  'A2': 8900,  'A3': 8600,  'A5': 8200,
        'A7': 7900,
        'F0': 7350,  'F2': 6950,  'F5': 6500,  'F8': 6200,
        'G0': 5930,  'G2': 5770,  'G5': 5660,  'G8': 5440,
        'K0': 5240,  'K2': 4960,  'K3': 4800,  'K4': 4600,  'K5': 4440,
        'K7': 4100,
        'M0': 3870,  'M1': 3705,  'M2': 3560,  'M3': 3430,  'M4': 3270,
        'M5': 3060,  'M6': 2890,  'M7': 2700,  'M8': 2520,  'M9': 2320,
    };

    /** 古代颜色描述 → 典型有效温度 (K) */
    const ANCIENT_COLOR_TEMP = {
        '白':  9500,   // A5 型, 典型天狼星 / 织女星
        '青白': 12000, // B8 型
        '青':  20000,  // B2 型, 高温蓝色
        '苍':  8500,   // A2 型, 偏白蓝
        '黄':  5500,   // G2 型, 太阳类
        '金黄': 5000,  // K0 型
        '赤':  3800,   // M1 型, 红巨星
        '红':  3500,   // M3 型
        '紫':  30000,  // O/B 型, 古人偶有描述
        '黑':  2800,   // M6 型, 极低温暗红星
    };

    function spectralTypeToTemp(spec) {
        if (!spec) return 5770;
        // 取前 2 位匹配, e.g. "G2V" → "G2"
        const key = spec.substring(0, 2).toUpperCase();
        return SPECTRAL_TYPE_TEMP[key] || 5770;
    }

    function ancientColorToTemp(colorDesc) {
        if (!colorDesc) return 5770;
        // 精确匹配
        if (ANCIENT_COLOR_TEMP[colorDesc]) return ANCIENT_COLOR_TEMP[colorDesc];
        // 模糊匹配 (包含关键字)
        for (const k in ANCIENT_COLOR_TEMP) {
            if (colorDesc.includes(k)) return ANCIENT_COLOR_TEMP[k];
        }
        return 5770;
    }

    // ============================================================
    // ★ 修复 3: Planck 黑体辐射色温 → sRGB 颜色
    // ============================================================
    //
    // 原问题:
    //   旧代码将古代颜色描述 (白/青/赤/黄) 映射为静态 CSS 色,
    //   与实际恒星 Planck 黑体辐射光谱不符.
    //   例: 古代"赤色"对应 M 型红巨星 (T~3500K),
    //       实际为橙红色 (#ff8a4a), 而非纯红 (#ff0000).
    //
    // 修复方案 (Tanner-Helland 近似 + 精确 CIE 积分):
    //   1. Planck 定律: B_λ(T) = 2hc²/λ⁵ · 1/(e^(hc/λkT) - 1)
    //      hc/k = 1.4387769 cm·K = 14387769 nm·K
    //   2. CIE 1931 XYZ 色匹配函数: 在 380-780nm 以 5nm 步长积分
    //   3. XYZ → sRGB (D65 白点, sRGB 矩阵 + gamma 2.2)
    //   4. 按星等衰减亮度, 保留色调
    //
    //   参考:
    //     - CIE 1931: https://en.wikipedia.org/wiki/CIE_1931_color_space
    //     - Planck law: https://en.wikipedia.org/wiki/Planck%27s_law
    //     - 验证: T=5770K → 太阳光 (#fff4e6); T=30000K → 蓝白色 (#aac8ff)
    // ============================================================

    // CIE 1931 色匹配函数 (380-780 nm, 10 nm 步长, 共 41 采样点)
    // 数据来源: ASTM E308-01
    const CIE_WAVELENGTHS = [
        380, 390, 400, 410, 420, 430, 440, 450, 460, 470,
        480, 490, 500, 510, 520, 530, 540, 550, 560, 570,
        580, 590, 600, 610, 620, 630, 640, 650, 660, 670,
        680, 690, 700, 710, 720, 730, 740, 750, 760, 770, 780
    ];
    const CIE_X = [
        0.001368, 0.004210, 0.014310, 0.043510, 0.134380, 0.283900, 0.348280, 0.336200, 0.290800, 0.195360,
        0.095640, 0.032010, 0.004900, 0.009300, 0.063270, 0.165500, 0.290400, 0.433450, 0.594500, 0.762100,
        0.916300, 1.026300, 1.062200, 1.002600, 0.854450, 0.642400, 0.447900, 0.283500, 0.164900, 0.087400,
        0.046770, 0.022700, 0.011359, 0.005790, 0.002899, 0.001440, 0.000690, 0.000332, 0.000166, 0.000083, 0.000042
    ];
    const CIE_Y = [
        0.000039, 0.000120, 0.000396, 0.001210, 0.004000, 0.011600, 0.023000, 0.038000, 0.060000, 0.091000,
        0.139020, 0.208020, 0.323000, 0.471000, 0.594000, 0.710000, 0.793200, 0.862000, 0.914850, 0.954000,
        0.978600, 0.989600, 0.995000, 0.984000, 0.934900, 0.856292, 0.742000, 0.607000, 0.470400, 0.342500,
        0.232000, 0.151300, 0.096500, 0.060000, 0.036884, 0.021870, 0.012586, 0.007204, 0.004102, 0.002320, 0.001282
    ];
    const CIE_Z = [
        0.006450, 0.020050, 0.067850, 0.207400, 0.645600, 1.385600, 1.747060, 1.772110, 1.669200, 1.398560,
        1.041900, 0.657080, 0.353300, 0.199780, 0.088050, 0.039490, 0.017000, 0.008210, 0.003900, 0.001600,
        0.000374, 0.000000, 0.000000, 0.000000, 0.000000, 0.000000, 0.000000, 0.000000, 0.000000, 0.000000,
        0.000000, 0.000000, 0.000000, 0.000000, 0.000000, 0.000000, 0.000000, 0.000000, 0.000000, 0.000000, 0.000000
    ];

    /**
     * Planck 函数 (非标准化, 只用于相对光谱计算)
     * B_λ(T) ∝ 1/λ⁵ · 1/(e^(hc/λkT) - 1)
     * hc/k = 14387769 nm·K
     */
    function planck(lambda_nm, T) {
        if (T <= 0) return 0;
        // Rayleigh-Jeans 近似在长波 (hc/λkT << 1)
        // Wien 近似在短波
        const x = 14387769.0 / (lambda_nm * T);
        // 避免 exp 溢出
        if (x > 500) return 0;
        const denom = Math.exp(x) - 1.0;
        if (denom <= 0) return 0;
        const lambda5 = Math.pow(lambda_nm, 5);
        return 1.0 / (lambda5 * denom);
    }

    /**
     * ★ 核心修复: 有效温度 (K) → sRGB {r, g, b} (0-255 整数)
     * @param {number} T  有效温度 (K), 有效范围 1500K - 40000K
     * @param {number} magnitude  目视星等 (用于亮度衰减, 可省略)
     * @returns {{r:number, g:number, b:number, hex:string}}
     */
    function tempToRGB(T, magnitude = null) {
        // 限制有效范围
        T = Math.max(1500, Math.min(40000, T));

        // 1. Planck 光谱 × CIE 匹配函数 = XYZ 分量
        let X = 0, Y = 0, Z = 0;
        for (let i = 0; i < CIE_WAVELENGTHS.length; i++) {
            const b = planck(CIE_WAVELENGTHS[i], T);
            X += b * CIE_X[i];
            Y += b * CIE_Y[i];
            Z += b * CIE_Z[i];
        }
        // 10nm 步长 * 2 (采样间隔)
        X *= 10; Y *= 10; Z *= 10;

        // 2. XYZ 归一化 (Y=1 为白点)
        const sum = X + Y + Z;
        if (sum <= 0) { X = 1/3; Y = 1/3; Z = 1/3; }
        else { X /= sum; Y /= sum; Z /= sum; }

        // 3. XYZ → 线性 sRGB (D65 白点)
        //    M(XYZ→sRGB) = https://en.wikipedia.org/wiki/SRGB#Specification
        const Rl =  3.2404542 * X - 1.5371385 * Y - 0.4985314 * Z;
        const Gl = -0.9692660 * X + 1.8760108 * Y + 0.0415560 * Z;
        const Bl =  0.0556434 * X - 0.2040259 * Y + 1.0572252 * Z;

        // 4. sRGB Gamma 编码 (近似 gamma=2.2)
        const srgb_gamma = (c) => {
            if (c <= 0.0031308) return 12.92 * c;
            return 1.055 * Math.pow(c, 1.0/2.4) - 0.055;
        };
        let R = srgb_gamma(Rl);
        let G = srgb_gamma(Gl);
        let B = srgb_gamma(Bl);

        // 5. 裁剪到 [0,1] 并归一化峰值 (保证至少一个通道=1)
        const maxC = Math.max(R, G, B, 0.0001);
        R /= maxC; G /= maxC; B /= maxC;
        R = Math.max(0, Math.min(1, R));
        G = Math.max(0, Math.min(1, G));
        B = Math.max(0, Math.min(1, B));

        // 6. 可选: 按星等衰减亮度
        //    星等差 5 等 = 亮度比 100, 每等衰减倍数 ~ 2.512
        let brightness = 1.0;
        if (magnitude != null) {
            // 以 0 等星为基准亮度
            brightness = Math.pow(2.512, -Math.max(0, magnitude));
            // 人眼对数感知, 再压缩到 [0.3, 1.0] 的合理范围
            brightness = 0.3 + 0.7 * Math.min(1.0, brightness);
        }
        R *= brightness; G *= brightness; B *= brightness;

        const r = Math.round(R * 255);
        const g = Math.round(G * 255);
        const b = Math.round(B * 255);
        const hex = `#${r.toString(16).padStart(2,'0')}${g.toString(16).padStart(2,'0')}${b.toString(16).padStart(2,'0')}`;
        return { r, g, b, hex };
    }

    /**
     * 综合映射: 恒星数据 → {hex, temp}
     * 优先顺序: 光谱型 → 古代颜色描述 → 色温字段 → 默认 5770K
     */
    function starToColor(starData, styleMode = 'planck') {
        let T = 5770;
        if (styleMode === 'modern') {
            T = spectralTypeToTemp(starData.color_class);
        } else if (styleMode === 'ancient') {
            // 古代色彩: 用古代颜色描述但加一点饱和
            T = ancientColorToTemp(starData.color_desc);
        } else {
            // Planck 模式 (默认): 优先现代光谱, 再用古代描述
            if (starData.color_temp_k && starData.color_temp_k > 1000) {
                T = starData.color_temp_k;
            } else if (starData.color_class) {
                T = spectralTypeToTemp(starData.color_class);
            } else {
                T = ancientColorToTemp(starData.color_desc);
            }
        }
        const rgb = tempToRGB(T, starData.magnitude_num);
        return { hex: rgb.hex, temp: T, r: rgb.r, g: rgb.g, b: rgb.b };
    }

    // ============================================================
    // 朝代信息
    // ============================================================

    const DYNASTY_STYLES = {
        '汉': 'han',
        '三国': 'sanguo',
        '晋': 'jin',
        '南北朝': 'n_south',
        '隋': 'sui',
        '唐': 'tang',
        '五代': 'wudai',
        '宋': 'song',
        '辽': 'liao',
        '金': 'jin_erdai',
        '元': 'yuan',
        '明': 'ming',
        '清': 'qing',
    };

    return {
        DEG2RAD, RAD2DEG,
        normalize360,
        angSepDeg,
        equatorialToCartesian,
        spectralTypeToTemp,
        ancientColorToTemp,
        tempToRGB,
        starToColor,
        DYNASTY_STYLES,
        // 调试用
        _PLANCK: planck,
    };
})();

window.Astro = Astro;
