/* 公众科普交互可视化模块 (public_engagement) */

const API_BASE_HOROSCOPE = (() => {
    const p = window.location.protocol;
    const h = window.location.hostname;
    const port = window.location.port === '8080' ? '8080' : '8080';
    return `${p}//${h}:${port}/api`;
})();

const PRESET_CITIES = [
    { name: '北京', lat: 39.9042, lon: 116.4074 },
    { name: '上海', lat: 31.2304, lon: 121.4737 },
    { name: '广州', lat: 23.1291, lon: 113.2644 },
    { name: '深圳', lat: 22.5431, lon: 114.0579 },
    { name: '成都', lat: 30.5728, lon: 104.0668 },
    { name: '杭州', lat: 30.2741, lon: 120.1551 },
    { name: '武汉', lat: 30.5928, lon: 114.3055 },
    { name: '西安', lat: 34.3416, lon: 108.9398 },
    { name: '南京', lat: 32.0603, lon: 118.7969 },
    { name: '重庆', lat: 29.5630, lon: 106.5516 },
    { name: '天津', lat: 39.0842, lon: 117.2010 },
    { name: '苏州', lat: 31.2989, lon: 120.5853 },
    { name: '长沙', lat: 28.2282, lon: 112.9388 },
    { name: '郑州', lat: 34.7466, lon: 113.6254 },
    { name: '青岛', lat: 36.0671, lon: 120.3826 },
    { name: '大连', lat: 38.9140, lon: 121.6147 },
    { name: '厦门', lat: 24.4798, lon: 118.0894 },
    { name: '昆明', lat: 25.0389, lon: 102.7183 },
    { name: '拉萨', lat: 29.6520, lon: 91.1721 },
    { name: '乌鲁木齐', lat: 43.8256, lon: 87.6168 },
];

const SHICHEN_HOURS = [
    { name: '子时', hour: 0, label: '子时 (23:00-01:00)' },
    { name: '丑时', hour: 2, label: '丑时 (01:00-03:00)' },
    { name: '寅时', hour: 4, label: '寅时 (03:00-05:00)' },
    { name: '卯时', hour: 6, label: '卯时 (05:00-07:00)' },
    { name: '辰时', hour: 8, label: '辰时 (07:00-09:00)' },
    { name: '巳时', hour: 10, label: '巳时 (09:00-11:00)' },
    { name: '午时', hour: 12, label: '午时 (11:00-13:00)' },
    { name: '未时', hour: 14, label: '未时 (13:00-15:00)' },
    { name: '申时', hour: 16, label: '申时 (15:00-17:00)' },
    { name: '酉时', hour: 18, label: '酉时 (17:00-19:00)' },
    { name: '戌时', hour: 20, label: '戌时 (19:00-21:00)' },
    { name: '亥时', hour: 22, label: '亥时 (21:00-23:00)' },
];

const PLANET_COLORS = {
    'sun': '#ffd86b',
    'moon': '#f5f0e0',
    'mercury': '#b0a090',
    'venus': '#ffd0a0',
    'mars': '#e06050',
    'jupiter': '#e8c080',
    'saturn': '#d0b070',
};

class PublicEngagementView {
    constructor() {
        this.currentStarmap = null;
        this.currentShareCard = null;

        this.onStarmapGenerated = null;
        this.onShareCardGenerated = null;
        this.onStarClicked = null;

        this._formData = {};
        this._starHitAreas = [];
    }

    // ============================================================
    // API 方法
    // ============================================================

    async _json(url, method = 'GET', body = null) {
        const opts = {
            method,
            headers: { 'Content-Type': 'application/json' },
        };
        if (body) opts.body = JSON.stringify(body);
        const resp = await fetch(url, opts);
        if (!resp.ok) {
            const t = await resp.text();
            throw new Error(`HTTP ${resp.status}: ${t}`);
        }
        const json = await resp.json();
        if (json.code !== undefined && json.code !== 0) {
            throw new Error(json.message || 'API error');
        }
        return json.data;
    }

    async generateStarmap(request) {
        const data = await this._json(
            `${API_BASE_HOROSCOPE}/horoscope/starmap`,
            'POST',
            request
        );
        this.currentStarmap = data;
        if (typeof this.onStarmapGenerated === 'function') {
            this.onStarmapGenerated(data);
        }
        return data;
    }

    async getShareCard(hash) {
        const data = await this._json(
            `${API_BASE_HOROSCOPE}/horoscope/share/${hash}`
        );
        this.currentShareCard = data;
        if (typeof this.onShareCardGenerated === 'function') {
            this.onShareCardGenerated(data);
        }
        return data;
    }

    // ============================================================
    // 用户输入表单
    // ============================================================

    renderInputForm(container) {
        if (!container) return;

        const now = new Date();
        const defaultYear = now.getFullYear() - 25;
        const defaultMonth = 6;
        const defaultDay = 15;

        container.innerHTML = `
            <div class="horoscope-form" style="padding:16px;display:flex;flex-direction:column;gap:14px;">
                <h3 style="margin:0 0 4px 0;color:#e0e8ff;font-size:16px;">出生信息</h3>

                <div class="form-row">
                    <label>出生日期</label>
                    <div style="display:flex;gap:8px;">
                        <select id="birth-year" style="flex:1;"></select>
                        <select id="birth-month" style="flex:0.7;"></select>
                        <select id="birth-day" style="flex:0.7;"></select>
                    </div>
                </div>

                <div class="form-row">
                    <label>出生时间</label>
                    <div style="display:flex;gap:8px;align-items:center;">
                        <select id="time-mode" style="flex:0.8;">
                            <option value="shichen">时辰</option>
                            <option value="hour24">24小时制</option>
                        </select>
                        <div id="shichen-container" style="flex:1;">
                            <select id="birth-shichen" style="width:100%;"></select>
                        </div>
                        <div id="hour24-container" style="flex:1;display:none;">
                            <div style="display:flex;gap:4px;align-items:center;">
                                <input type="number" id="birth-hour" min="0" max="23" value="12"
                                       style="width:60px;padding:6px 8px;border-radius:4px;
                                              border:1px solid rgba(120,150,200,0.3);
                                              background:rgba(30,40,60,0.6);color:#e0e8ff;">
                                <span style="color:#8090b0;">时</span>
                                <input type="number" id="birth-minute" min="0" max="59" value="0"
                                       style="width:60px;padding:6px 8px;border-radius:4px;
                                              border:1px solid rgba(120,150,200,0.3);
                                              background:rgba(30,40,60,0.6);color:#e0e8ff;">
                                <span style="color:#8090b0;">分</span>
                            </div>
                        </div>
                    </div>
                </div>

                <div class="form-row">
                    <label>出生地点</label>
                    <div style="display:flex;gap:8px;flex-wrap:wrap;">
                        <select id="birth-city" style="flex:1;min-width:120px;"></select>
                    </div>
                    <div style="display:flex;gap:8px;margin-top:6px;">
                        <div style="flex:1;">
                            <span style="font-size:11px;color:#8090b0;">纬度</span>
                            <input type="number" id="birth-lat" step="0.0001"
                                   style="width:100%;padding:6px 8px;border-radius:4px;
                                          border:1px solid rgba(120,150,200,0.3);
                                          background:rgba(30,40,60,0.6);color:#e0e8ff;font-size:12px;">
                        </div>
                        <div style="flex:1;">
                            <span style="font-size:11px;color:#8090b0;">经度</span>
                            <input type="number" id="birth-lon" step="0.0001"
                                   style="width:100%;padding:6px 8px;border-radius:4px;
                                          border:1px solid rgba(120,150,200,0.3);
                                          background:rgba(30,40,60,0.6);color:#e0e8ff;font-size:12px;">
                        </div>
                    </div>
                </div>

                <div class="form-row">
                    <label>星图风格</label>
                    <div style="display:flex;gap:8px;">
                        <button class="style-btn active" data-style="ancient"
                                style="flex:1;padding:8px;border-radius:6px;
                                       border:1px solid #ffd86b;background:rgba(255,216,107,0.15);
                                       color:#ffd86b;cursor:pointer;font-size:12px;">
                            古典风格
                        </button>
                        <button class="style-btn" data-style="modern"
                                style="flex:1;padding:8px;border-radius:6px;
                                       border:1px solid rgba(120,150,200,0.3);
                                       background:rgba(30,40,60,0.4);
                                       color:#a0c8ff;cursor:pointer;font-size:12px;">
                            现代风格
                        </button>
                    </div>
                </div>

                <div class="form-error" id="form-error"
                     style="display:none;padding:8px 12px;background:rgba(248,113,113,0.15);
                            border:1px solid rgba(248,113,113,0.3);border-radius:6px;
                            color:#f87171;font-size:12px;">
                </div>

                <button id="generate-starmap-btn"
                        style="padding:10px 16px;border-radius:8px;
                               border:none;background:linear-gradient(135deg,#ffd86b,#f5a623);
                               color:#1a1a2e;cursor:pointer;font-weight:bold;font-size:14px;
                               transition:all 0.2s;"
                        onmouseover="this.style.transform='scale(1.02)';this.style.boxShadow='0 4px 20px rgba(255,216,107,0.4)'"
                        onmouseout="this.style.transform='scale(1)';this.style.boxShadow='none'">
                    ★ 生成我的星图
                </button>
            </div>
        `;

        this._populateYearSelect(container.querySelector('#birth-year'), defaultYear);
        this._populateMonthSelect(container.querySelector('#birth-month'), defaultMonth);
        this._populateDaySelect(container.querySelector('#birth-day'), defaultYear, defaultMonth, defaultDay);
        this._populateShichenSelect(container.querySelector('#birth-shichen'));
        this._populateCitySelect(container.querySelector('#birth-city'));

        this._bindFormEvents(container);
        this._updateLatLonFromCity(container);
    }

    _populateYearSelect(select, defaultYear) {
        if (!select) return;
        let html = '';
        for (let y = 2024; y >= 1930; y--) {
            html += `<option value="${y}" ${y === defaultYear ? 'selected' : ''}>${y}年</option>`;
        }
        select.innerHTML = html;
    }

    _populateMonthSelect(select, defaultMonth) {
        if (!select) return;
        let html = '';
        for (let m = 1; m <= 12; m++) {
            html += `<option value="${m}" ${m === defaultMonth ? 'selected' : ''}>${m}月</option>`;
        }
        select.innerHTML = html;
    }

    _populateDaySelect(select, year, month, defaultDay) {
        if (!select) return;
        const daysInMonth = new Date(year, month, 0).getDate();
        const currentDay = Math.min(defaultDay, daysInMonth);
        let html = '';
        for (let d = 1; d <= daysInMonth; d++) {
            html += `<option value="${d}" ${d === currentDay ? 'selected' : ''}>${d}日</option>`;
        }
        select.innerHTML = html;
    }

    _populateShichenSelect(select) {
        if (!select) return;
        select.innerHTML = SHICHEN_HOURS.map(s =>
            `<option value="${s.hour}">${s.label}</option>`
        ).join('');
    }

    _populateCitySelect(select) {
        if (!select) return;
        select.innerHTML = PRESET_CITIES.map(c =>
            `<option value="${c.name}" data-lat="${c.lat}" data-lon="${c.lon}">${c.name}</option>`
        ).join('');
    }

    _bindFormEvents(container) {
        const yearSel = container.querySelector('#birth-year');
        const monthSel = container.querySelector('#birth-month');
        const daySel = container.querySelector('#birth-day');
        const timeModeSel = container.querySelector('#time-mode');
        const shichenContainer = container.querySelector('#shichen-container');
        const hour24Container = container.querySelector('#hour24-container');
        const citySel = container.querySelector('#birth-city');
        const latInput = container.querySelector('#birth-lat');
        const lonInput = container.querySelector('#birth-lon');
        const styleBtns = container.querySelectorAll('.style-btn');
        const generateBtn = container.querySelector('#generate-starmap-btn');

        const updateDays = () => {
            const y = parseInt(yearSel.value);
            const m = parseInt(monthSel.value);
            const currentDay = parseInt(daySel.value) || 1;
            this._populateDaySelect(daySel, y, m, currentDay);
        };

        yearSel.addEventListener('change', updateDays);
        monthSel.addEventListener('change', updateDays);

        timeModeSel.addEventListener('change', () => {
            if (timeModeSel.value === 'shichen') {
                shichenContainer.style.display = 'block';
                hour24Container.style.display = 'none';
            } else {
                shichenContainer.style.display = 'none';
                hour24Container.style.display = 'block';
            }
        });

        citySel.addEventListener('change', () => {
            this._updateLatLonFromCity(container);
        });

        styleBtns.forEach(btn => {
            btn.addEventListener('click', () => {
                styleBtns.forEach(b => {
                    b.classList.remove('active');
                    b.style.borderColor = 'rgba(120,150,200,0.3)';
                    b.style.background = 'rgba(30,40,60,0.4)';
                    b.style.color = '#a0c8ff';
                });
                btn.classList.add('active');
                if (btn.dataset.style === 'ancient') {
                    btn.style.borderColor = '#ffd86b';
                    btn.style.background = 'rgba(255,216,107,0.15)';
                    btn.style.color = '#ffd86b';
                } else {
                    btn.style.borderColor = '#60a0ff';
                    btn.style.background = 'rgba(96,160,255,0.15)';
                    btn.style.color = '#60a0ff';
                }
            });
        });

        generateBtn.addEventListener('click', () => {
            this._handleGenerate(container);
        });
    }

    _updateLatLonFromCity(container) {
        const citySel = container.querySelector('#birth-city');
        const latInput = container.querySelector('#birth-lat');
        const lonInput = container.querySelector('#birth-lon');
        const selectedOption = citySel.options[citySel.selectedIndex];
        if (selectedOption) {
            latInput.value = parseFloat(selectedOption.dataset.lat).toFixed(4);
            lonInput.value = parseFloat(selectedOption.dataset.lon).toFixed(4);
        }
    }

    _validateForm(container) {
        const errors = [];

        const year = parseInt(container.querySelector('#birth-year').value);
        const month = parseInt(container.querySelector('#birth-month').value);
        const day = parseInt(container.querySelector('#birth-day').value);
        const lat = parseFloat(container.querySelector('#birth-lat').value);
        const lon = parseFloat(container.querySelector('#birth-lon').value);

        if (isNaN(year) || year < 1900 || year > 2100) {
            errors.push('请输入有效的出生年份');
        }
        if (isNaN(month) || month < 1 || month > 12) {
            errors.push('请输入有效的出生月份');
        }
        if (isNaN(day) || day < 1 || day > 31) {
            errors.push('请输入有效的出生日期');
        }
        if (isNaN(lat) || lat < -90 || lat > 90) {
            errors.push('请输入有效的纬度 (-90 到 90)');
        }
        if (isNaN(lon) || lon < -180 || lon > 180) {
            errors.push('请输入有效的经度 (-180 到 180)');
        }

        return errors;
    }

    _getFormData(container) {
        const year = parseInt(container.querySelector('#birth-year').value);
        const month = parseInt(container.querySelector('#birth-month').value);
        const day = parseInt(container.querySelector('#birth-day').value);
        const timeMode = container.querySelector('#time-mode').value;

        let hourUtc;
        if (timeMode === 'shichen') {
            const shichenHour = parseInt(container.querySelector('#birth-shichen').value);
            hourUtc = (shichenHour + 1) % 24;
        } else {
            const hour = parseInt(container.querySelector('#birth-hour').value) || 12;
            const minute = parseInt(container.querySelector('#birth-minute').value) || 0;
            hourUtc = hour + minute / 60;
        }

        const lat = parseFloat(container.querySelector('#birth-lat').value);
        const lon = parseFloat(container.querySelector('#birth-lon').value);
        const cityName = container.querySelector('#birth-city').value;
        const activeStyleBtn = container.querySelector('.style-btn.active');
        const cardStyle = activeStyleBtn ? activeStyleBtn.dataset.style : 'ancient';

        return {
            birth_year: year,
            birth_month: month,
            birth_day: day,
            birth_hour_utc: hourUtc,
            latitude_deg: lat,
            longitude_deg: lon,
            city_name: cityName,
            card_style: cardStyle,
            show_moon_planets: true,
            show_lunar_mansions: true,
            show_constellation_lines: false,
            mag_limit: 6.0,
            compare_with_ancient_epoch: -1000,
            generate_share_card: true,
        };
    }

    async _handleGenerate(container) {
        const errorDiv = container.querySelector('#form-error');
        const generateBtn = container.querySelector('#generate-starmap-btn');

        const errors = this._validateForm(container);
        if (errors.length > 0) {
            errorDiv.textContent = errors.join('；');
            errorDiv.style.display = 'block';
            return;
        }
        errorDiv.style.display = 'none';

        const request = this._getFormData(container);
        this._formData = request;

        generateBtn.disabled = true;
        generateBtn.textContent = '生成中...';
        generateBtn.style.opacity = '0.7';

        try {
            await this.generateStarmap(request);
        } catch (e) {
            errorDiv.textContent = '生成失败: ' + e.message;
            errorDiv.style.display = 'block';
            console.error('Starmap generation failed:', e);
        } finally {
            generateBtn.disabled = false;
            generateBtn.textContent = '★ 生成我的星图';
            generateBtn.style.opacity = '1';
        }
    }

    // ============================================================
    // 星图渲染
    // ============================================================

    renderStarmap(canvas, response) {
        if (!canvas || !response) return;

        const ctx = canvas.getContext('2d');
        const w = canvas.width;
        const h = canvas.height;
        const cx = w / 2;
        const cy = h / 2;
        const radius = Math.min(w, h) * 0.45;

        ctx.clearRect(0, 0, w, h);
        this._starHitAreas = [];

        const style = response.projection_mode === 'ancient' ? 'ancient' : 'modern';

        this._drawSkyBackground(ctx, cx, cy, radius, style);
        this._drawHorizonCircle(ctx, cx, cy, radius, style);
        this._drawAzimuthMarks(ctx, cx, cy, radius, style);
        this._drawLunarMansions(ctx, cx, cy, radius, response, style);
        this._drawStars(ctx, cx, cy, radius, response, style);
        this._drawPlanets(ctx, cx, cy, radius, response, style);
        this._drawSun(ctx, cx, cy, radius, response, style);
        this._drawMoon(ctx, cx, cy, radius, response, style);
    }

    _drawSkyBackground(ctx, cx, cy, radius, style) {
        const gradient = ctx.createRadialGradient(cx, cy, 0, cx, cy, radius);
        if (style === 'ancient') {
            gradient.addColorStop(0, 'rgba(20, 25, 50, 0.9)');
            gradient.addColorStop(0.7, 'rgba(10, 15, 35, 0.95)');
            gradient.addColorStop(1, 'rgba(5, 8, 20, 1)');
        } else {
            gradient.addColorStop(0, 'rgba(15, 20, 45, 0.9)');
            gradient.addColorStop(0.7, 'rgba(8, 12, 30, 0.95)');
            gradient.addColorStop(1, 'rgba(3, 5, 15, 1)');
        }

        ctx.beginPath();
        ctx.arc(cx, cy, radius, 0, Math.PI * 2);
        ctx.fillStyle = gradient;
        ctx.fill();

        ctx.save();
        ctx.beginPath();
        ctx.arc(cx, cy, radius, 0, Math.PI * 2);
        ctx.clip();

        const milkyGradient = ctx.createRadialGradient(
            cx + radius * 0.2, cy - radius * 0.3, 0,
            cx + radius * 0.2, cy - radius * 0.3, radius * 0.8
        );
        milkyGradient.addColorStop(0, 'rgba(200, 220, 255, 0.08)');
        milkyGradient.addColorStop(0.5, 'rgba(150, 180, 220, 0.04)');
        milkyGradient.addColorStop(1, 'rgba(100, 140, 180, 0)');
        ctx.fillStyle = milkyGradient;
        ctx.fillRect(cx - radius, cy - radius, radius * 2, radius * 2);

        ctx.restore();
    }

    _drawHorizonCircle(ctx, cx, cy, radius, style) {
        const borderColor = style === 'ancient' ? '#c9a84c' : '#5a7ab0';

        ctx.beginPath();
        ctx.arc(cx, cy, radius, 0, Math.PI * 2);
        ctx.strokeStyle = borderColor;
        ctx.lineWidth = 2;
        ctx.stroke();

        ctx.beginPath();
        ctx.arc(cx, cy, radius + 3, 0, Math.PI * 2);
        ctx.strokeStyle = style === 'ancient' ? 'rgba(201, 168, 76, 0.3)' : 'rgba(90, 122, 176, 0.3)';
        ctx.lineWidth = 1;
        ctx.stroke();
    }

    _drawAzimuthMarks(ctx, cx, cy, radius, style) {
        const marks = [
            { angle: -Math.PI / 2, label: '南' },
            { angle: 0, label: '西' },
            { angle: Math.PI / 2, label: '北' },
            { angle: Math.PI, label: '东' },
        ];

        const textColor = style === 'ancient' ? '#c9a84c' : '#8ab4f8';
        const markColor = style === 'ancient' ? 'rgba(201, 168, 76, 0.6)' : 'rgba(138, 180, 248, 0.6)';

        marks.forEach(mark => {
            const x = cx + Math.cos(mark.angle) * radius;
            const y = cy + Math.sin(mark.angle) * radius;

            ctx.beginPath();
            ctx.arc(x, y, 4, 0, Math.PI * 2);
            ctx.fillStyle = markColor;
            ctx.fill();

            ctx.font = 'bold 13px "Noto Serif SC", serif';
            ctx.fillStyle = textColor;
            ctx.textAlign = 'center';
            ctx.textBaseline = 'middle';

            const labelDist = radius + 22;
            const lx = cx + Math.cos(mark.angle) * labelDist;
            const ly = cy + Math.sin(mark.angle) * labelDist;
            ctx.fillText(mark.label, lx, ly);
        });

        for (let i = 0; i < 360; i += 30) {
            const angle = (i - 90) * Astro.DEG2RAD;
            const x1 = cx + Math.cos(angle) * (radius - 6);
            const y1 = cy + Math.sin(angle) * (radius - 6);
            const x2 = cx + Math.cos(angle) * radius;
            const y2 = cy + Math.sin(angle) * radius;

            ctx.beginPath();
            ctx.moveTo(x1, y1);
            ctx.lineTo(x2, y2);
            ctx.strokeStyle = markColor;
            ctx.lineWidth = i % 90 === 0 ? 2 : 1;
            ctx.stroke();
        }
    }

    _drawLunarMansions(ctx, cx, cy, radius, response, style) {
        const boundaries = response.lunar_mansion_boundaries;
        if (!boundaries || boundaries.length === 0) return;

        const lineColor = style === 'ancient'
            ? 'rgba(255, 215, 100, 0.5)'
            : 'rgba(100, 180, 255, 0.4)';

        ctx.save();
        ctx.setLineDash([6, 4]);
        ctx.strokeStyle = lineColor;
        ctx.lineWidth = 1.5;

        boundaries.forEach(boundary => {
            if (!boundary.dec_samples || boundary.dec_samples.length < 2) return;

            ctx.beginPath();
            let first = true;
            boundary.dec_samples.forEach(([ra, dec]) => {
                const proj = this._stereographicProjection(ra, dec, radius);
                const x = cx + proj.x;
                const y = cy - proj.y;
                if (first) {
                    ctx.moveTo(x, y);
                    first = false;
                } else {
                    ctx.lineTo(x, y);
                }
            });
            ctx.stroke();
        });

        ctx.restore();

        if (style === 'ancient') {
            const textColor = 'rgba(255, 215, 100, 0.7)';
            boundaries.forEach((boundary, idx) => {
                const midRa = (boundary.ra_start_deg_at_epoch + boundary.ra_end_deg_at_epoch) / 2;
                const proj = this._stereographicProjection(midRa, 20, radius * 0.9);
                const x = cx + proj.x;
                const y = cy - proj.y;

                if (Math.sqrt(proj.x * proj.x + proj.y * proj.y) < radius * 0.85) {
                    ctx.font = '10px "Noto Serif SC", serif';
                    ctx.fillStyle = textColor;
                    ctx.textAlign = 'center';
                    ctx.fillText(boundary.mansion_name_cn, x, y);
                }
            });
        }
    }

    _drawStars(ctx, cx, cy, radius, response, style) {
        const stars = response.stars || [];

        stars.forEach(star => {
            if (star.altitude_at_birth_deg < -5) return;

            const x = cx + star.projected_x * radius;
            const y = cy - star.projected_y * radius;

            if (Math.abs(star.projected_x) > 1.2 || Math.abs(star.projected_y) > 1.2) return;

            const mag = star.apparent_magnitude;
            const size = Math.max(0.5, Math.min(4, (6 - mag) * 0.8));

            let colorHex;
            if (style === 'ancient') {
                if (star.magnitude_ancient_desc) {
                    colorHex = '#ffe8a0';
                } else {
                    colorHex = '#ffffff';
                }
            } else {
                const temp = star.color_temp_k || 5770;
                const color = Astro.tempToRGB(temp, mag);
                colorHex = color.hex;
            }

            const gradient = ctx.createRadialGradient(x, y, 0, x, y, size * 2);
            gradient.addColorStop(0, colorHex);
            gradient.addColorStop(0.5, colorHex + '80');
            gradient.addColorStop(1, colorHex + '00');

            ctx.beginPath();
            ctx.arc(x, y, size * 2, 0, Math.PI * 2);
            ctx.fillStyle = gradient;
            ctx.fill();

            ctx.beginPath();
            ctx.arc(x, y, size * 0.6, 0, Math.PI * 2);
            ctx.fillStyle = colorHex;
            ctx.fill();

            if (mag <= 3.5 && star.ancient_name_cn) {
                ctx.font = '10px "Noto Serif SC", sans-serif';
                ctx.fillStyle = style === 'ancient' ? 'rgba(255, 215, 100, 0.8)' : 'rgba(200, 220, 255, 0.8)';
                ctx.textAlign = 'center';
                ctx.fillText(star.ancient_name_cn, x, y - size - 4);
            }

            this._starHitAreas.push({
                x, y,
                radius: size * 3,
                star: star,
            });
        });
    }

    _drawPlanets(ctx, cx, cy, radius, response, style) {
        const bodies = response.solar_system_bodies || [];

        bodies.forEach(body => {
            if (body.body_name_en === 'sun' || body.body_name_en === 'moon') return;
            if (body.altitude_deg < -5) return;

            const x = cx + body.projected_x * radius;
            const y = cy - body.projected_y * radius;

            if (Math.abs(body.projected_x) > 1.2 || Math.abs(body.projected_y) > 1.2) return;

            const color = PLANET_COLORS[body.body_name_en] || '#c0c0c0';
            const size = Math.max(4, 8 - body.apparent_magnitude * 0.8);

            const glow = ctx.createRadialGradient(x, y, 0, x, y, size * 2.5);
            glow.addColorStop(0, color + 'aa');
            glow.addColorStop(0.5, color + '40');
            glow.addColorStop(1, color + '00');
            ctx.beginPath();
            ctx.arc(x, y, size * 2.5, 0, Math.PI * 2);
            ctx.fillStyle = glow;
            ctx.fill();

            ctx.beginPath();
            ctx.arc(x, y, size, 0, Math.PI * 2);
            ctx.fillStyle = color;
            ctx.fill();

            ctx.font = 'bold 11px "Noto Serif SC", sans-serif';
            ctx.fillStyle = style === 'ancient' ? '#ffd86b' : '#a0c8ff';
            ctx.textAlign = 'center';
            ctx.fillText(body.body_name_cn, x, y + size + 14);
        });
    }

    _drawSun(ctx, cx, cy, radius, response, style) {
        const sun = (response.solar_system_bodies || []).find(b => b.body_name_en === 'sun');
        if (!sun || sun.altitude_deg < -5) return;

        const x = cx + sun.projected_x * radius;
        const y = cy - sun.projected_y * radius;

        if (Math.abs(sun.projected_x) > 1.2 || Math.abs(sun.projected_y) > 1.2) return;

        const size = 12;

        for (let i = 3; i >= 1; i--) {
            const glowSize = size * (1 + i * 1.5);
            const gradient = ctx.createRadialGradient(x, y, 0, x, y, glowSize);
            gradient.addColorStop(0, `rgba(255, 220, 100, ${0.3 / i})`);
            gradient.addColorStop(1, 'rgba(255, 220, 100, 0)');
            ctx.beginPath();
            ctx.arc(x, y, glowSize, 0, Math.PI * 2);
            ctx.fillStyle = gradient;
            ctx.fill();
        }

        const sunGradient = ctx.createRadialGradient(x - size * 0.3, y - size * 0.3, 0, x, y, size);
        sunGradient.addColorStop(0, '#fff8e0');
        sunGradient.addColorStop(0.7, '#ffd86b');
        sunGradient.addColorStop(1, '#f5a623');
        ctx.beginPath();
        ctx.arc(x, y, size, 0, Math.PI * 2);
        ctx.fillStyle = sunGradient;
        ctx.fill();

        ctx.font = 'bold 12px "Noto Serif SC", sans-serif';
        ctx.fillStyle = '#ffd86b';
        ctx.textAlign = 'center';
        ctx.fillText('太阳', x, y + size + 16);
    }

    _drawMoon(ctx, cx, cy, radius, response, style) {
        const moon = (response.solar_system_bodies || []).find(b => b.body_name_en === 'moon');
        if (!moon || moon.altitude_deg < -5) return;

        const x = cx + moon.projected_x * radius;
        const y = cy - moon.projected_y * radius;

        if (Math.abs(moon.projected_x) > 1.2 || Math.abs(moon.projected_y) > 1.2) return;

        const size = 10;
        const phase = moon.phase_fraction != null ? moon.phase_fraction : 0.7;

        const glow = ctx.createRadialGradient(x, y, 0, x, y, size * 2);
        glow.addColorStop(0, 'rgba(245, 240, 224, 0.5)');
        glow.addColorStop(1, 'rgba(245, 240, 224, 0)');
        ctx.beginPath();
        ctx.arc(x, y, size * 2, 0, Math.PI * 2);
        ctx.fillStyle = glow;
        ctx.fill();

        ctx.beginPath();
        ctx.arc(x, y, size, 0, Math.PI * 2);
        ctx.fillStyle = '#f5f0e0';
        ctx.fill();

        if (phase < 0.95) {
            const shadowOffset = size * 2 * (0.5 - phase);
            ctx.save();
            ctx.beginPath();
            ctx.arc(x, y, size, 0, Math.PI * 2);
            ctx.clip();

            const shadowColor = 'rgba(20, 30, 60, 0.9)';
            if (phase < 0.5) {
                ctx.fillStyle = shadowColor;
                ctx.fillRect(x - size, y - size, size * 2, size * 2);

                const sliverX = x + shadowOffset;
                const sliverGrad = ctx.createRadialGradient(
                    sliverX, y, 0,
                    sliverX, y, size
                );
                sliverGrad.addColorStop(0, '#f5f0e0');
                sliverGrad.addColorStop(1, '#d0c8b0');
                ctx.beginPath();
                ctx.arc(sliverX, y, size, 0, Math.PI * 2);
                ctx.fillStyle = sliverGrad;
                ctx.fill();
            } else {
                const sliverX = x - shadowOffset + size;
                ctx.fillStyle = shadowColor;
                ctx.beginPath();
                ctx.arc(sliverX, y, size, 0, Math.PI * 2);
                ctx.fill();
            }

            ctx.restore();
        }

        ctx.font = 'bold 11px "Noto Serif SC", sans-serif';
        ctx.fillStyle = style === 'ancient' ? '#ffd86b' : '#e0e8ff';
        ctx.textAlign = 'center';
        ctx.fillText('月亮', x, y + size + 16);
    }

    _stereographicProjection(ra, dec, radius) {
        const raRad = ra * Astro.DEG2RAD;
        const decRad = dec * Astro.DEG2RAD;

        const scale = radius;
        const k = 2 * scale / (1 + Math.sin(decRad));
        const x = k * Math.cos(decRad) * Math.sin(raRad);
        const y = k * Math.cos(decRad) * Math.cos(raRad);

        return { x, y };
    }

    // ============================================================
    // 古今对比视图
    // ============================================================

    renderComparison(canvas, response) {
        if (!canvas || !response) return;

        const ctx = canvas.getContext('2d');
        const w = canvas.width;
        const h = canvas.height;
        const cx = w / 2;
        const cy = h / 2;
        const radius = Math.min(w, h) * 0.42;

        ctx.clearRect(0, 0, w, h);

        this._drawSkyBackground(ctx, cx, cy, radius, 'modern');
        this._drawHorizonCircle(ctx, cx, cy, radius, 'modern');
        this._drawAzimuthMarks(ctx, cx, cy, radius, 'modern');

        const diff = response.ancient_comparison;
        const stars = response.stars || [];

        const shiftScale = 50;

        const highlightedStars = [];

        stars.forEach(star => {
            if (star.altitude_at_birth_deg < -5) return;

            const xModern = cx + star.projected_x * radius;
            const yModern = cy - star.projected_y * radius;

            if (Math.abs(star.projected_x) > 1.2 || Math.abs(star.projected_y) > 1.2) return;

            const mag = star.apparent_magnitude;
            const sizeModern = Math.max(0.5, Math.min(3, (6 - mag) * 0.6));

            const temp = star.color_temp_k || 5770;
            const color = Astro.tempToRGB(temp, mag);

            ctx.beginPath();
            ctx.arc(xModern, yModern, sizeModern * 0.7, 0, Math.PI * 2);
            ctx.fillStyle = '#60a0ff';
            ctx.fill();

            const properMotionRa = star.proper_motion_ra || 0;
            const properMotionDec = star.proper_motion_dec || 0;
            const shiftMag = Math.sqrt(properMotionRa * properMotionRa + properMotionDec * properMotionDec);

            const ancientX = xModern + properMotionRa * shiftScale;
            const ancientY = yModern - properMotionDec * shiftScale;

            const sizeAncient = sizeModern * 0.9;
            ctx.beginPath();
            ctx.arc(ancientX, ancientY, sizeAncient * 0.7, 0, Math.PI * 2);
            ctx.fillStyle = '#ffd86b';
            ctx.fill();

            if (shiftMag > 0.01 && mag <= 5) {
                ctx.beginPath();
                ctx.moveTo(xModern, yModern);
                ctx.lineTo(ancientX, ancientY);
                ctx.strokeStyle = 'rgba(255, 200, 100, 0.4)';
                ctx.lineWidth = 0.8;
                ctx.stroke();

                const arrowSize = 3;
                const angle = Math.atan2(-(ancientY - yModern), ancientX - xModern);
                ctx.beginPath();
                ctx.moveTo(ancientX, ancientY);
                ctx.lineTo(
                    ancientX - arrowSize * Math.cos(angle - Math.PI / 6),
                    ancientY + arrowSize * Math.sin(angle - Math.PI / 6)
                );
                ctx.lineTo(
                    ancientX - arrowSize * Math.cos(angle + Math.PI / 6),
                    ancientY + arrowSize * Math.sin(angle + Math.PI / 6)
                );
                ctx.closePath();
                ctx.fillStyle = 'rgba(255, 200, 100, 0.6)';
                ctx.fill();
            }

            if (diff && diff.max_shift_star_names && diff.max_shift_star_names.includes(star.ancient_name_cn)) {
                highlightedStars.push({
                    star,
                    modernX: xModern,
                    modernY: yModern,
                    ancientX,
                    ancientY,
                });
            }
        });

        highlightedStars.forEach(({ star, modernX, modernY, ancientX, ancientY }) => {
            const glowColor = '#ffd86b';

            [modernX, ancientX].forEach((sx, i) => {
                const sy = i === 0 ? modernY : ancientY;
                const glow = ctx.createRadialGradient(sx, sy, 0, sx, sy, 12);
                glow.addColorStop(0, glowColor + '80');
                glow.addColorStop(1, glowColor + '00');
                ctx.beginPath();
                ctx.arc(sx, sy, 12, 0, Math.PI * 2);
                ctx.fillStyle = glow;
                ctx.fill();

                ctx.beginPath();
                ctx.arc(sx, sy, 4, 0, Math.PI * 2);
                ctx.fillStyle = glowColor;
                ctx.fill();
            });

            if (star.ancient_name_cn) {
                ctx.font = 'bold 12px "Noto Serif SC", sans-serif';
                ctx.fillStyle = '#ffd86b';
                ctx.textAlign = 'center';
                ctx.fillText(star.ancient_name_cn, modernX, modernY - 14);
            }
        });

        this._drawComparisonLegend(ctx, w, h);

        if (diff) {
            this._drawComparisonStats(ctx, diff, w, h);
        }
    }

    _drawComparisonLegend(ctx, w, h) {
        const legendX = 20;
        const legendY = 20;

        ctx.fillStyle = 'rgba(20, 30, 60, 0.8)';
        ctx.fillRect(legendX - 10, legendY - 10, 150, 70);
        ctx.strokeStyle = 'rgba(120, 150, 200, 0.3)';
        ctx.strokeRect(legendX - 10, legendY - 10, 150, 70);

        ctx.beginPath();
        ctx.arc(legendX + 10, legendY + 12, 4, 0, Math.PI * 2);
        ctx.fillStyle = '#60a0ff';
        ctx.fill();
        ctx.font = '11px sans-serif';
        ctx.fillStyle = '#a0c8ff';
        ctx.textAlign = 'left';
        ctx.fillText('现代星空 (J2000)', legendX + 22, legendY + 16);

        ctx.beginPath();
        ctx.arc(legendX + 10, legendY + 32, 4, 0, Math.PI * 2);
        ctx.fillStyle = '#ffd86b';
        ctx.fill();
        ctx.fillStyle = '#ffd86b';
        ctx.fillText('古代星空 (约公元前1000年)', legendX + 22, legendY + 36);

        ctx.beginPath();
        ctx.moveTo(legendX + 5, legendY + 52);
        ctx.lineTo(legendX + 18, legendY + 52);
        ctx.strokeStyle = 'rgba(255, 200, 100, 0.6)';
        ctx.lineWidth = 1.5;
        ctx.stroke();

        ctx.beginPath();
        ctx.moveTo(legendX + 18, legendY + 52);
        ctx.lineTo(legendX + 14, legendY + 49);
        ctx.lineTo(legendX + 14, legendY + 55);
        ctx.closePath();
        ctx.fillStyle = 'rgba(255, 200, 100, 0.6)';
        ctx.fill();

        ctx.fillStyle = '#8090b0';
        ctx.fillText('自行方向 (已放大)', legendX + 26, legendY + 56);
    }

    _drawComparisonStats(ctx, diff, w, h) {
        const statsX = w - 20;
        const statsY = 20;
        const boxW = 180;
        const boxH = 90;

        ctx.fillStyle = 'rgba(20, 30, 60, 0.8)';
        ctx.fillRect(statsX - boxW, statsY - 10, boxW, boxH);
        ctx.strokeStyle = 'rgba(120, 150, 200, 0.3)';
        ctx.strokeRect(statsX - boxW, statsY - 10, boxW, boxH);

        ctx.font = 'bold 12px "Noto Serif SC", sans-serif';
        ctx.fillStyle = '#ffd86b';
        ctx.textAlign = 'right';
        ctx.fillText('古今差异统计', statsX - 10, statsY + 6);

        ctx.font = '11px sans-serif';
        ctx.fillStyle = '#a0c8ff';
        ctx.fillText(`参照年代: 公元前 ${Math.abs(Math.round(diff.ancient_epoch_yr))} 年`, statsX - 10, statsY + 28);

        ctx.fillStyle = '#8090b0';
        ctx.fillText(`位移 > 1° 的恒星: ${diff.num_stars_shifted_gt_1deg} 颗`, statsX - 10, statsY + 46);

        ctx.fillStyle = '#8090b0';
        ctx.fillText(`平均角位移: ${diff.avg_angular_shift_arcmin?.toFixed(2)}'`, statsX - 10, statsY + 64);

        if (diff.ancient_sun_lunar_mansion) {
            ctx.fillStyle = '#ffd86b';
            ctx.font = '10px "Noto Serif SC", sans-serif';
            ctx.fillText(`日宿: ${diff.ancient_sun_lunar_mansion}`, statsX - 10, statsY + 78);
        }
    }

    // ============================================================
    // 分享卡片
    // ============================================================

    renderShareCard(canvas, response, cardSpec) {
        if (!canvas || !response) return;

        const ctx = canvas.getContext('2d');
        const spec = cardSpec || {
            width_px: 1080,
            height_px: 1920,
            title_text: '你的专属古代星图',
            subtitle_text: '穿越千年的星空邂逅',
            footer_text: '古代星表数字化与现代验证系统',
            accent_color_hex: '#ffd86b',
            background_gradient_from_hex: '#050810',
            background_gradient_to_hex: '#1a2040',
        };

        const w = spec.width_px || 1080;
        const h = spec.height_px || 1920;

        canvas.width = w;
        canvas.height = h;

        const bgGradient = ctx.createLinearGradient(0, 0, 0, h);
        bgGradient.addColorStop(0, spec.background_gradient_from_hex || '#050810');
        bgGradient.addColorStop(1, spec.background_gradient_to_hex || '#1a2040');
        ctx.fillStyle = bgGradient;
        ctx.fillRect(0, 0, w, h);

        this._drawCardStars(ctx, w, h);

        const titleY = 120;
        ctx.font = 'bold 56px "Noto Serif SC", serif';
        ctx.fillStyle = spec.accent_color_hex || '#ffd86b';
        ctx.textAlign = 'center';
        ctx.fillText(spec.title_text || '你的专属古代星图', w / 2, titleY);

        ctx.font = '24px "Noto Serif SC", serif';
        ctx.fillStyle = 'rgba(200, 220, 255, 0.7)';
        ctx.fillText(spec.subtitle_text || '穿越千年的星空邂逅', w / 2, titleY + 50);

        const starmapSize = 700;
        const starmapX = w / 2;
        const starmapY = 580;

        ctx.save();
        ctx.translate(starmapX, starmapY);

        const outerGlow = ctx.createRadialGradient(0, 0, starmapSize * 0.4, 0, 0, starmapSize * 0.6);
        outerGlow.addColorStop(0, 'rgba(255, 216, 107, 0.15)');
        outerGlow.addColorStop(1, 'rgba(255, 216, 107, 0)');
        ctx.beginPath();
        ctx.arc(0, 0, starmapSize * 0.6, 0, Math.PI * 2);
        ctx.fillStyle = outerGlow;
        ctx.fill();

        const ringGradient = ctx.createRadialGradient(0, 0, starmapSize * 0.48, 0, 0, starmapSize * 0.5);
        ringGradient.addColorStop(0, 'rgba(201, 168, 76, 0)');
        ringGradient.addColorStop(0.5, spec.accent_color_hex || '#ffd86b');
        ringGradient.addColorStop(1, 'rgba(201, 168, 76, 0)');
        ctx.beginPath();
        ctx.arc(0, 0, starmapSize * 0.5, 0, Math.PI * 2);
        ctx.strokeStyle = ringGradient;
        ctx.lineWidth = 4;
        ctx.stroke();

        ctx.restore();

        const tempCanvas = document.createElement('canvas');
        tempCanvas.width = starmapSize;
        tempCanvas.height = starmapSize;
        const tempCtx = tempCanvas.getContext('2d');

        const tempResponse = JSON.parse(JSON.stringify(response));
        if (tempResponse.stars) {
            tempResponse.stars = tempResponse.stars.filter(s => s.altitude_at_birth_deg > -10);
        }

        this.renderStarmap(tempCanvas, tempResponse);

        ctx.save();
        ctx.beginPath();
        ctx.arc(starmapX, starmapY, starmapSize * 0.45, 0, Math.PI * 2);
        ctx.clip();
        ctx.drawImage(
            tempCanvas,
            starmapX - starmapSize / 2,
            starmapY - starmapSize / 2,
            starmapSize,
            starmapSize
        );
        ctx.restore();

        const infoY = 1050;
        const info = response.personal_info;

        ctx.font = 'bold 32px "Noto Serif SC", serif';
        ctx.fillStyle = '#e0e8ff';
        ctx.textAlign = 'center';
        ctx.fillText('个人信息', w / 2, infoY);

        const infoLineY = infoY + 20;
        ctx.strokeStyle = spec.accent_color_hex || '#ffd86b';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(w / 2 - 50, infoLineY);
        ctx.lineTo(w / 2 + 50, infoLineY);
        ctx.stroke();

        const infoStartY = infoY + 60;
        const lineHeight = 44;

        ctx.font = '22px "Noto Serif SC", sans-serif';
        ctx.textAlign = 'left';

        const infoItems = [
            { label: '出生日期', value: info ? `${info.birth_date_ymd[0]}年${info.birth_date_ymd[1]}月${info.birth_date_ymd[2]}日` : '-' },
            { label: '出生地点', value: info ? info.city_name : '-' },
            { label: '太阳星宿', value: info ? info.lunar_mansion_sun : '-', accent: true },
            { label: '月亮星宿', value: info ? info.lunar_mansion_moon : '-', accent: true },
        ];

        const infoX = 150;
        infoItems.forEach((item, idx) => {
            const y = infoStartY + idx * lineHeight;

            ctx.fillStyle = '#8090b0';
            ctx.fillText(item.label, infoX, y);

            ctx.fillStyle = item.accent ? (spec.accent_color_hex || '#ffd86b') : '#e0e8ff';
            ctx.textAlign = 'right';
            ctx.fillText(item.value, w - infoX, y);
            ctx.textAlign = 'left';
        });

        const luckyY = infoStartY + infoItems.length * lineHeight + 50;

        ctx.font = 'bold 32px "Noto Serif SC", serif';
        ctx.fillStyle = '#e0e8ff';
        ctx.textAlign = 'center';
        ctx.fillText('你的幸运星', w / 2, luckyY);

        const luckyLineY = luckyY + 20;
        ctx.strokeStyle = spec.accent_color_hex || '#ffd86b';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(w / 2 - 50, luckyLineY);
        ctx.lineTo(w / 2 + 50, luckyLineY);
        ctx.stroke();

        const luckyStars = response.lucky_stars || [];
        const luckyStartY = luckyY + 60;
        const luckyItemHeight = 50;

        luckyStars.slice(0, 5).forEach((star, idx) => {
            const y = luckyStartY + idx * luckyItemHeight;

            const starX = 160;
            const starColor = Astro.tempToRGB(star.magnitude ? 5770 : 6500);

            const glow = ctx.createRadialGradient(starX, y, 0, starX, y, 16);
            glow.addColorStop(0, spec.accent_color_hex || '#ffd86b');
            glow.addColorStop(1, (spec.accent_color_hex || '#ffd86b') + '00');
            ctx.beginPath();
            ctx.arc(starX, y, 16, 0, Math.PI * 2);
            ctx.fillStyle = glow;
            ctx.fill();

            ctx.beginPath();
            ctx.arc(starX, y, 6, 0, Math.PI * 2);
            ctx.fillStyle = spec.accent_color_hex || '#ffd86b';
            ctx.fill();

            ctx.font = '20px "Noto Serif SC", sans-serif';
            ctx.fillStyle = '#e0e8ff';
            ctx.textAlign = 'left';
            ctx.fillText(star.star_name_cn, starX + 30, y + 6);

            if (star.modern_name) {
                ctx.font = '16px sans-serif';
                ctx.fillStyle = '#8090b0';
                ctx.fillText(star.modern_name, starX + 30, y + 28);
            }

            ctx.font = '18px sans-serif';
            ctx.fillStyle = '#8090b0';
            ctx.textAlign = 'right';
            ctx.fillText(`${star.magnitude?.toFixed(2)} 等`, w - 160, y + 6);
        });

        const qrSize = 140;
        const qrX = w / 2 - qrSize / 2;
        const qrY = h - 280;

        ctx.fillStyle = 'rgba(255, 255, 255, 0.95)';
        ctx.fillRect(qrX - 10, qrY - 10, qrSize + 20, qrSize + 20);
        ctx.strokeStyle = 'rgba(120, 150, 200, 0.3)';
        ctx.lineWidth = 2;
        ctx.strokeRect(qrX - 10, qrY - 10, qrSize + 20, qrSize + 20);

        this._drawPlaceholderQR(ctx, qrX, qrY, qrSize);

        ctx.font = '18px "Noto Serif SC", sans-serif';
        ctx.fillStyle = '#8090b0';
        ctx.textAlign = 'center';
        ctx.fillText('扫码查看你的专属星图', w / 2, qrY + qrSize + 36);

        ctx.font = '16px serif';
        ctx.fillStyle = 'rgba(160, 180, 220, 0.5)';
        ctx.fillText(spec.footer_text || '古代星表数字化与现代验证系统', w / 2, h - 60);
    }

    _drawCardStars(ctx, w, h) {
        const numStars = 100;
        for (let i = 0; i < numStars; i++) {
            const x = Math.random() * w;
            const y = Math.random() * h;
            const size = Math.random() * 1.5 + 0.5;
            const opacity = Math.random() * 0.5 + 0.2;

            ctx.beginPath();
            ctx.arc(x, y, size, 0, Math.PI * 2);
            ctx.fillStyle = `rgba(255, 255, 255, ${opacity})`;
            ctx.fill();
        }

        for (let i = 0; i < 20; i++) {
            const x = Math.random() * w;
            const y = Math.random() * h * 0.4;
            const size = Math.random() * 3 + 2;

            const glow = ctx.createRadialGradient(x, y, 0, x, y, size * 3);
            glow.addColorStop(0, 'rgba(255, 216, 107, 0.3)');
            glow.addColorStop(1, 'rgba(255, 216, 107, 0)');
            ctx.beginPath();
            ctx.arc(x, y, size * 3, 0, Math.PI * 2);
            ctx.fillStyle = glow;
            ctx.fill();
        }
    }

    _drawPlaceholderQR(ctx, x, y, size) {
        const cellSize = size / 21;

        ctx.fillStyle = '#1a1a2e';

        const finderPositions = [
            [0, 0],
            [14, 0],
            [0, 14],
        ];

        finderPositions.forEach(([fx, fy]) => {
            const px = x + fx * cellSize;
            const py = y + fy * cellSize;
            const finderSize = cellSize * 7;

            ctx.fillRect(px, py, finderSize, finderSize);
            ctx.fillStyle = '#ffffff';
            ctx.fillRect(px + cellSize, py + cellSize, finderSize - cellSize * 2, finderSize - cellSize * 2);
            ctx.fillStyle = '#1a1a2e';
            ctx.fillRect(px + cellSize * 2, py + cellSize * 2, finderSize - cellSize * 4, finderSize - cellSize * 4);
        });

        const pattern = [
            [3, 3], [3, 4], [3, 5], [3, 7], [3, 9],
            [4, 3], [4, 7], [4, 9],
            [5, 3], [5, 5], [5, 7],
            [7, 3], [7, 4], [7, 5], [7, 7], [7, 8], [7, 9],
            [9, 3], [9, 4], [9, 6], [9, 7], [9, 9],
            [10, 3], [10, 5], [10, 8],
            [11, 3], [11, 5], [11, 7], [11, 9],
            [13, 3], [13, 4], [13, 6], [13, 8], [13, 9],
            [14, 3], [14, 5], [14, 7], [14, 9],
            [15, 4], [15, 6], [15, 8],
            [17, 3], [17, 5], [17, 7], [17, 9],
            [18, 4], [18, 6], [18, 8],
        ];

        pattern.forEach(([px, py]) => {
            ctx.fillRect(
                x + px * cellSize,
                y + py * cellSize,
                cellSize,
                cellSize
            );
        });

        ctx.fillStyle = '#1a1a2e';
        ctx.fillRect(x + size / 2 - cellSize, y + size / 2 - cellSize, cellSize * 2, cellSize * 2);
    }

    // ============================================================
    // 星星点击检测
    // ============================================================

    handleCanvasClick(event, canvas) {
        if (!canvas || this._starHitAreas.length === 0) return null;

        const rect = canvas.getBoundingClientRect();
        const scaleX = canvas.width / rect.width;
        const scaleY = canvas.height / rect.height;
        const clickX = (event.clientX - rect.left) * scaleX;
        const clickY = (event.clientY - rect.top) * scaleY;

        for (let i = this._starHitAreas.length - 1; i >= 0; i--) {
            const hit = this._starHitAreas[i];
            const dx = clickX - hit.x;
            const dy = clickY - hit.y;
            const dist = Math.sqrt(dx * dx + dy * dy);

            if (dist <= hit.radius) {
                if (typeof this.onStarClicked === 'function') {
                    this.onStarClicked(hit.star);
                }
                return hit.star;
            }
        }

        return null;
    }

    // ============================================================
    // 工具方法
    // ============================================================

    getFormData() {
        return this._formData;
    }

    getCurrentStarmap() {
        return this.currentStarmap;
    }

    getCurrentShareCard() {
        return this.currentShareCard;
    }
}

const StarmapGenerator = PublicEngagementView;
window.PublicEngagementView = PublicEngagementView;
window.StarmapGenerator = StarmapGenerator;