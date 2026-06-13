/* 日食预测可视化模块 (eclipse_predictor) */

class EclipsePredictorView {
    constructor() {
        this.API_BASE = (() => {
            const p = window.location.protocol;
            const h = window.location.hostname;
            const port = window.location.port === '8080' ? '8080' : '8080';
            return `${p}//${h}:${port}/api`;
        })();

        this.records = [];
        this.currentRecord = null;
        this.currentResult = null;
        this.dynasties = [];

        this.onEclipseSelected = null;
        this.onPathHover = null;

        this._animationFrame = null;
        this._animationStartTime = 0;

        this._hoveredSample = null;
    }

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

    async fetchEclipses(params = {}) {
        const q = new URLSearchParams();
        for (const [k, v] of Object.entries(params)) {
            if (v != null && v !== '') q.set(k, v);
        }
        const s = q.toString();
        const data = await this._json(`${this.API_BASE}/eclipses${s ? '?' + s : ''}`);
        this.records = Array.isArray(data) ? data : (data?.records || []);
        return this.records;
    }

    async fetchEclipseDetail(id) {
        const data = await this._json(`${this.API_BASE}/eclipses/${id}`);
        this.currentRecord = data;
        return data;
    }

    async computeEclipse(id) {
        const data = await this._json(`${this.API_BASE}/eclipses/${id}/compute`, 'POST');
        this.currentResult = data;
        return data;
    }

    setDynasties(d) {
        this.dynasties = Array.isArray(d) ? d : [];
    }

    _getDynastyColor(dynastyName) {
        const colorMap = {
            '汉': '#c8283c',
            '三国': '#c87828',
            '晋': '#78a050',
            '南北朝': '#3c6496',
            '隋': '#a028b4',
            '唐': '#f0b428',
            '五代': '#8c5050',
            '宋': '#8c50dc',
            '辽': '#3c8c8c',
            '金': '#a0643c',
            '元': '#3264b4',
            '明': '#dc503c',
            '清': '#3c8c50',
        };
        if (!dynastyName) return '#6080b0';
        const d = this.dynasties.find(x => x.name_cn === dynastyName);
        if (d && d.color_hex) return d.color_hex;
        return colorMap[dynastyName] || '#6080b0';
    }

    _getEclipseTypeIcon(type) {
        const t = (type || '').toLowerCase();
        if (t.includes('solar') || t.includes('日')) return '☀';
        if (t.includes('lunar') || t.includes('月')) return '☾';
        return '◉';
    }

    _getEclipseTypeLabel(type) {
        const t = (type || '').toLowerCase();
        if (t.includes('solar') || t.includes('日')) return '日食';
        if (t.includes('lunar') || t.includes('月')) return '月食';
        return '未知';
    }

    _getClassificationLabel(cls) {
        const map = {
            'total': '全食',
            'annular': '环食',
            'partial': '偏食',
            'none': '无食',
        };
        return map[cls] || cls || '-';
    }

    _formatYear(yearCe) {
        if (yearCe == null) return '-';
        const y = Math.round(yearCe);
        return y < 0 ? `前${-y}` : `公元 ${y}`;
    }

    _jdToDateStr(jd) {
        if (jd == null) return '-';
        const jd2000 = 2451545.0;
        const msPerDay = 86400000;
        const unixMs = (jd - jd2000) * msPerDay + Date.UTC(2000, 0, 1, 12, 0, 0);
        const d = new Date(unixMs);
        return d.toISOString().slice(0, 16).replace('T', ' ');
    }

    renderEclipseList(container, records) {
        if (!container) return;
        const list = Array.isArray(records) ? records : this.records;

        if (!list || list.length === 0) {
            container.innerHTML = `
                <div style="padding:30px;text-align:center;color:#8090b0;">
                    暂无日食月食记录
                </div>
            `;
            return;
        }

        container.innerHTML = `
            <div class="panel-section">
                <h3 style="margin:0 0 10px 0;color:#a0c8ff;font-size:13px;padding-bottom:6px;
                    border-bottom:1px solid rgba(80,120,200,0.2);">
                    日食月食记录 (${list.length})
                </h3>
                <div class="stars-list" id="eclipse-list-container">
                    ${list.map((r, i) => this._renderEclipseItem(r, i)).join('')}
                </div>
            </div>
        `;

        container.querySelectorAll('.eclipse-item').forEach(el => {
            el.addEventListener('click', () => {
                const id = parseInt(el.dataset.id);
                const rec = list.find(r => r.id === id);
                if (rec) {
                    container.querySelectorAll('.eclipse-item').forEach(x => x.classList.remove('active'));
                    el.classList.add('active');
                    this.currentRecord = rec;
                    if (this.onEclipseSelected) {
                        this.onEclipseSelected(rec);
                    }
                }
            });
        });
    }

    _renderEclipseItem(r, idx) {
        const color = this._getDynastyColor(r.dynasty_name);
        const icon = this._getEclipseTypeIcon(r.eclipse_type);
        const typeLabel = this._getEclipseTypeLabel(r.eclipse_type);
        const mag = r.magnitude_num != null ? r.magnitude_num.toFixed(2) : '-';
        const year = this._formatYear(r.year_ce);

        return `
            <div class="eclipse-item star-item" data-id="${r.id}">
                <div style="display:flex;align-items:center;gap:8px;min-width:0;">
                    <span style="display:inline-block;width:3px;height:20px;background:${color};
                        border-radius:2px;flex-shrink:0;"></span>
                    <span style="font-size:16px;flex-shrink:0;">${icon}</span>
                    <div style="min-width:0;">
                        <div style="color:#e0e8ff;font-size:12px;white-space:nowrap;
                            overflow:hidden;text-overflow:ellipsis;">
                            ${r.eclipse_id_code || `Eclipse-${idx + 1}`}
                        </div>
                        <div style="color:#8090b0;font-size:10px;">
                            ${r.dynasty_name || '-'} · ${year}
                        </div>
                    </div>
                </div>
                <div style="text-align:right;flex-shrink:0;">
                    <div style="color:#a0c8ff;font-size:11px;">${typeLabel}</div>
                    <div style="color:#ffcc60;font-size:11px;font-family:Consolas,monospace;">
                        食分 ${mag}
                    </div>
                </div>
            </div>
        `;
    }

    renderEclipsePath(canvasOrSvg, result) {
        if (!canvasOrSvg || !result) return;
        const canvas = canvasOrSvg;
        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        const dpr = window.devicePixelRatio || 1;
        const w = canvas.clientWidth || canvas.width || 400;
        const h = canvas.clientHeight || canvas.height || 300;
        canvas.width = w * dpr;
        canvas.height = h * dpr;
        canvas.style.width = w + 'px';
        canvas.style.height = h + 'px';
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

        ctx.clearRect(0, 0, w, h);

        const cx = w / 2;
        const cy = h / 2;
        const R = Math.min(w, h) * 0.42;

        const centerLat = result.path_center_lat_deg ?? 30;
        const centerLon = result.path_center_lon_deg ?? 110;

        const project = (latDeg, lonDeg) => {
            const lat = latDeg * Math.PI / 180;
            const lon = lonDeg * Math.PI / 180;
            const clat = centerLat * Math.PI / 180;
            const clon = centerLon * Math.PI / 180;

            const cosC = Math.sin(clat) * Math.sin(lat) +
                Math.cos(clat) * Math.cos(lat) * Math.cos(lon - clon);
            if (cosC < 0) return null;

            const x = Math.cos(lat) * Math.sin(lon - clon);
            const y = Math.cos(clat) * Math.sin(lat) -
                Math.sin(clat) * Math.cos(lat) * Math.cos(lon - clon);

            return {
                x: cx + x * R,
                y: cy - y * R,
                visible: true
            };
        };

        const bgGrad = ctx.createRadialGradient(cx - R * 0.3, cy - R * 0.3, R * 0.1, cx, cy, R);
        bgGrad.addColorStop(0, '#1a3060');
        bgGrad.addColorStop(0.5, '#0c1a3a');
        bgGrad.addColorStop(1, '#050a1a');
        ctx.fillStyle = bgGrad;
        ctx.beginPath();
        ctx.arc(cx, cy, R, 0, Math.PI * 2);
        ctx.fill();

        ctx.strokeStyle = 'rgba(64,144,255,0.4)';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(cx, cy, R, 0, Math.PI * 2);
        ctx.stroke();

        ctx.strokeStyle = 'rgba(80,120,200,0.15)';
        ctx.lineWidth = 0.8;

        for (let lat = -60; lat <= 60; lat += 30) {
            ctx.beginPath();
            let started = false;
            for (let lon = -180; lon <= 180; lon += 3) {
                const p = project(lat, lon);
                if (p) {
                    if (!started) {
                        ctx.moveTo(p.x, p.y);
                        started = true;
                    } else {
                        ctx.lineTo(p.x, p.y);
                    }
                } else {
                    started = false;
                }
            }
            ctx.stroke();
        }

        for (let lon = -180; lon < 180; lon += 30) {
            ctx.beginPath();
            let started = false;
            for (let lat = -90; lat <= 90; lat += 3) {
                const p = project(lat, lon);
                if (p) {
                    if (!started) {
                        ctx.moveTo(p.x, p.y);
                        started = true;
                    } else {
                        ctx.lineTo(p.x, p.y);
                    }
                } else {
                    started = false;
                }
            }
            ctx.stroke();
        }

        ctx.strokeStyle = 'rgba(80,120,200,0.3)';
        ctx.lineWidth = 1;
        ctx.beginPath();
        let started = false;
        for (let lon = -180; lon <= 180; lon += 2) {
            const p = project(0, lon);
            if (p) {
                if (!started) { ctx.moveTo(p.x, p.y); started = true; }
                else ctx.lineTo(p.x, p.y);
            } else { started = false; }
        }
        ctx.stroke();

        if (result.umbra_polygon_latlon && result.umbra_polygon_latlon.length > 0) {
            const penumbraPts = [];
            result.umbra_polygon_latlon.forEach(([lat, lon]) => {
                const dLat = (lat - centerLat) * 2.5;
                const dLon = (lon - centerLon) * 2.5;
                penumbraPts.push([centerLat + dLat, centerLon + dLon]);
            });

            ctx.fillStyle = 'rgba(255,160,60,0.25)';
            ctx.strokeStyle = 'rgba(255,160,60,0.5)';
            ctx.lineWidth = 1;
            ctx.beginPath();
            penumbraPts.forEach(([lat, lon], i) => {
                const p = project(lat, lon);
                if (p) {
                    if (i === 0) ctx.moveTo(p.x, p.y);
                    else ctx.lineTo(p.x, p.y);
                }
            });
            ctx.closePath();
            ctx.fill();
            ctx.stroke();
        }

        if (result.umbra_polygon_latlon && result.umbra_polygon_latlon.length > 0) {
            ctx.fillStyle = 'rgba(255,60,60,0.6)';
            ctx.strokeStyle = 'rgba(255,80,80,0.9)';
            ctx.lineWidth = 1.5;
            ctx.beginPath();
            result.umbra_polygon_latlon.forEach(([lat, lon], i) => {
                const p = project(lat, lon);
                if (p) {
                    if (i === 0) ctx.moveTo(p.x, p.y);
                    else ctx.lineTo(p.x, p.y);
                }
            });
            ctx.closePath();
            ctx.fill();
            ctx.stroke();
        }

        if (result.path_samples_utm && result.path_samples_utm.length > 1) {
            ctx.setLineDash([6, 4]);
            ctx.strokeStyle = '#ffcc40';
            ctx.lineWidth = 2;
            ctx.beginPath();
            result.path_samples_utm.forEach((s, i) => {
                const p = project(s.lat_deg, s.lon_deg);
                if (p) {
                    if (i === 0) ctx.moveTo(p.x, p.y);
                    else ctx.lineTo(p.x, p.y);
                }
            });
            ctx.stroke();
            ctx.setLineDash([]);

            result.path_samples_utm.forEach(s => {
                const p = project(s.lat_deg, s.lon_deg);
                if (p) {
                    ctx.fillStyle = '#ffcc40';
                    ctx.beginPath();
                    ctx.arc(p.x, p.y, 3, 0, Math.PI * 2);
                    ctx.fill();
                }
            });
        }

        const centerP = project(centerLat, centerLon);
        if (centerP) {
            ctx.strokeStyle = '#fff';
            ctx.lineWidth = 2;
            ctx.beginPath();
            ctx.moveTo(centerP.x - 8, centerP.y);
            ctx.lineTo(centerP.x + 8, centerP.y);
            ctx.moveTo(centerP.x, centerP.y - 8);
            ctx.lineTo(centerP.x, centerP.y + 8);
            ctx.stroke();

            ctx.fillStyle = '#fff';
            ctx.font = '10px Consolas, monospace';
            ctx.fillText(`食甚 (${centerLat.toFixed(1)}°, ${centerLon.toFixed(1)}°)`,
                centerP.x + 10, centerP.y - 10);
        }

        this._drawPathLegend(ctx, w, h);

        this._bindPathHover(canvas, result, project);
    }

    _drawPathLegend(ctx, w, h) {
        const lx = 10, ly = h - 80;
        ctx.fillStyle = 'rgba(10,18,38,0.85)';
        ctx.strokeStyle = 'rgba(80,120,200,0.3)';
        ctx.lineWidth = 1;
        ctx.fillRect(lx, ly, 120, 72);
        ctx.strokeRect(lx, ly, 120, 72);

        ctx.font = '11px -apple-system, sans-serif';
        ctx.fillStyle = '#a0c8ff';
        ctx.fillText('图例', lx + 8, ly + 16);

        ctx.fillStyle = 'rgba(255,60,60,0.6)';
        ctx.fillRect(lx + 8, ly + 24, 14, 10);
        ctx.fillStyle = '#e0e8ff';
        ctx.fillText('本影区', lx + 28, ly + 33);

        ctx.fillStyle = 'rgba(255,160,60,0.35)';
        ctx.fillRect(lx + 8, ly + 40, 14, 10);
        ctx.fillStyle = '#e0e8ff';
        ctx.fillText('半影区', lx + 28, ly + 49);

        ctx.strokeStyle = '#ffcc40';
        ctx.lineWidth = 2;
        ctx.setLineDash([4, 3]);
        ctx.beginPath();
        ctx.moveTo(lx + 8, ly + 61);
        ctx.lineTo(lx + 22, ly + 61);
        ctx.stroke();
        ctx.setLineDash([]);
        ctx.fillStyle = '#e0e8ff';
        ctx.fillText('中心线', lx + 28, ly + 65);
    }

    _bindPathHover(canvas, result, project) {
        if (!result.path_samples_utm || result.path_samples_utm.length === 0) return;

        canvas.onmousemove = (e) => {
            const rect = canvas.getBoundingClientRect();
            const mx = e.clientX - rect.left;
            const my = e.clientY - rect.top;

            let closest = null;
            let minDist = 15;

            result.path_samples_utm.forEach(s => {
                const p = project(s.lat_deg, s.lon_deg);
                if (!p) return;
                const d = Math.hypot(mx - p.x, my - p.y);
                if (d < minDist) {
                    minDist = d;
                    closest = s;
                }
            });

            this._hoveredSample = closest;
            canvas.style.cursor = closest ? 'pointer' : 'default';

            if (this.onPathHover) {
                this.onPathHover(closest);
            }
        };

        canvas.onmouseleave = () => {
            this._hoveredSample = null;
            if (this.onPathHover) this.onPathHover(null);
        };
    }

    renderVerificationPanel(container, result) {
        if (!container || !result) return;
        const rec = this.currentRecord || {};

        const recMag = rec.magnitude_num;
        const calcMag = result.magnitude_predicted;
        const magDev = result.magnitude_agreement_deviation;
        const timeDev = result.time_agreement_deviation_days;
        const quality = result.overall_quality_score ?? 0;

        const magColor = magDev == null ? '#8090b0' :
            magDev < 0.1 ? '#80ff80' : magDev < 0.3 ? '#ffcc60' : '#ff6060';
        const timeColor = timeDev == null ? '#8090b0' :
            timeDev < 30 ? '#80ff80' : timeDev < 180 ? '#ffcc60' : '#ff6060';
        const qualityColor = quality > 0.7 ? '#80ff80' :
            quality > 0.4 ? '#ffcc60' : '#ff6060';

        const recType = this._getEclipseTypeLabel(rec.eclipse_type);
        const calcType = this._getClassificationLabel(result.eclipse_classification);
        const typeMatch = (rec.eclipse_type || '').toLowerCase() ===
            (result.eclipse_type || '').toLowerCase();

        container.innerHTML = `
            <div class="panel-section">
                <h3 style="margin:0 0 10px 0;color:#a0c8ff;font-size:13px;padding-bottom:6px;
                    border-bottom:1px solid rgba(80,120,200,0.2);">
                    文献记载 vs 计算验证
                </h3>

                <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:12px;">
                    <div style="padding:8px 10px;border-radius:4px;
                        background:rgba(40,30,10,0.3);border-left:3px solid #ffcc60;">
                        <div style="font-size:10px;color:#8090b0;">文献记载</div>
                        <div style="font-size:11px;color:#e0e8ff;margin-top:2px;">
                            ${recType}
                        </div>
                    </div>
                    <div style="padding:8px 10px;border-radius:4px;
                        background:rgba(10,25,50,0.5);border-left:3px solid #4090ff;">
                        <div style="font-size:10px;color:#8090b0;">计算结果</div>
                        <div style="font-size:11px;color:#e0e8ff;margin-top:2px;">
                            ${calcType}
                            ${typeMatch ? '<span style="color:#80ff80;margin-left:4px;">✓</span>' :
                                '<span style="color:#ff8080;margin-left:4px;">✗</span>'}
                        </div>
                    </div>
                </div>

                <div style="display:flex;flex-direction:column;gap:10px;">
                    ${this._renderVerifyRow('食分',
                        recMag != null ? recMag.toFixed(2) : '-',
                        calcMag != null ? calcMag.toFixed(2) : '-',
                        magDev != null ? magDev.toFixed(2) : '-',
                        magColor
                    )}
                    ${this._renderVerifyRow('食甚时刻',
                        rec.year_ce != null ? this._formatYear(rec.year_ce) : '-',
                        result.computed_midpoint_jd_ut1 != null ?
                            this._jdToDateStr(result.computed_midpoint_jd_ut1) : '-',
                        timeDev != null ? `${(timeDev).toFixed(0)} 天` : '-',
                        timeColor
                    )}
                    ${this._renderVerifyRow('食类',
                        recType,
                        this._getClassificationLabel(result.eclipse_classification),
                        typeMatch ? '匹配' : '不符',
                        typeMatch ? '#80ff80' : '#ff8080'
                    )}
                </div>

                <div style="margin-top:14px;padding:10px;border-radius:5px;
                    background:rgba(10,18,38,0.6);border:1px solid rgba(80,120,200,0.2);">
                    <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:6px;">
                        <span style="font-size:11px;color:#8090b0;">综合质量评分</span>
                        <span style="font-size:16px;font-family:Consolas,monospace;color:${qualityColor};
                            font-weight:700;">${(quality * 100).toFixed(0)}%</span>
                    </div>
                    <div style="height:6px;background:rgba(40,60,100,0.4);border-radius:3px;overflow:hidden;">
                        <div style="height:100%;width:${(quality * 100).toFixed(0)}%;
                            background:linear-gradient(90deg,
                                ${quality > 0.7 ? '#20c060' : quality > 0.4 ? '#c08020' : '#c03030'},
                                ${quality > 0.7 ? '#80ffa0' : quality > 0.4 ? '#ffcc60' : '#ff6060'});
                            border-radius:3px;"></div>
                    </div>
                </div>

                <div style="margin-top:12px;display:grid;grid-template-columns:1fr 1fr;gap:6px;
                    font-size:10px;color:#8090b0;">
                    <div>沙罗周期: <span style="color:#a0c8ff;font-family:Consolas,monospace;">
                        ${result.saros_number || '-'}</span></div>
                    <div>ΔT: <span style="color:#a0c8ff;font-family:Consolas,monospace;">
                        ${result.delta_t_s != null ? result.delta_t_s.toFixed(1) + 's' : '-'}</span></div>
                    <div>遮挡比例: <span style="color:#ffcc60;font-family:Consolas,monospace;">
                        ${result.obscuration_fraction != null ?
                            (result.obscuration_fraction * 100).toFixed(1) + '%' : '-'}</span></div>
                    <div>持续时间: <span style="color:#a0c8ff;font-family:Consolas,monospace;">
                        ${result.duration_total_s != null ?
                            (result.duration_total_s / 60).toFixed(1) + 'min' : '-'}</span></div>
                </div>

                ${rec.record_text ? `
                    <div style="margin-top:12px;padding:8px 10px;border-radius:4px;
                        background:rgba(255,204,96,0.08);border-left:3px solid #ffcc60;
                        font-size:11px;line-height:1.6;color:#c8b890;">
                        <div style="color:#ffcc60;font-size:10px;margin-bottom:4px;">原始记载</div>
                        ${rec.record_text}
                    </div>
                ` : ''}
            </div>
        `;
    }

    _renderVerifyRow(label, recordVal, calcVal, deviation, devColor) {
        return `
            <div style="display:flex;flex-direction:column;gap:3px;">
                <div style="font-size:10px;color:#8090b0;">${label}</div>
                <div style="display:grid;grid-template-columns:1fr 1fr auto;gap:8px;align-items:center;">
                    <div style="font-size:12px;font-family:Consolas,monospace;color:#ffcc60;">
                        ${recordVal}
                    </div>
                    <div style="text-align:center;color:#6080b0;font-size:10px;">→</div>
                    <div style="font-size:12px;font-family:Consolas,monospace;color:#70c0ff;">
                        ${calcVal}
                    </div>
                </div>
                <div style="font-size:10px;color:${devColor};font-family:Consolas,monospace;
                    text-align:right;">
                    Δ ${deviation}
                </div>
            </div>
        `;
    }

    drawEclipseAnimation(canvas, result, timestamp) {
        if (!canvas || !result) return;
        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        const dpr = window.devicePixelRatio || 1;
        const w = canvas.clientWidth || canvas.width || 300;
        const h = canvas.clientHeight || canvas.height || 200;
        canvas.width = w * dpr;
        canvas.height = h * dpr;
        canvas.style.width = w + 'px';
        canvas.style.height = h + 'px';
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

        ctx.clearRect(0, 0, w, h);

        const cx = w / 2;
        const cy = h / 2;
        const sunR = Math.min(w, h) * 0.32;
        const moonR = sunR * (result.eclipse_type === 'lunar' ? 0.98 : 0.96);

        const magnitude = result.magnitude_predicted || 0;
        const duration = result.duration_total_s || 7200;
        const cycleMs = Math.max(6000, duration * 2);

        let t;
        if (timestamp != null) {
            t = ((timestamp % cycleMs) / cycleMs) * 2 - 1;
        } else {
            const now = performance.now();
            if (!this._animationStartTime) this._animationStartTime = now;
            t = (((now - this._animationStartTime) % cycleMs) / cycleMs) * 2 - 1;
        }

        const maxOffset = sunR + moonR;
        const moonOffset = t * maxOffset;
        const dist = Math.abs(moonOffset);

        const overlap = Math.max(0, sunR + moonR - dist);
        const currentMagnitude = Math.min(magnitude * 2, overlap / sunR);
        const obscuration = this._calcObscuration(sunR, moonR, dist);

        const isLunar = (result.eclipse_type || '').toLowerCase().includes('lunar');

        if (isLunar) {
            this._drawLunarEclipse(ctx, cx, cy, sunR, moonR, moonOffset, obscuration, currentMagnitude);
        } else {
            this._drawSolarEclipse(ctx, cx, cy, sunR, moonR, moonOffset, obscuration, currentMagnitude);
        }

        this._drawAnimationInfo(ctx, w, h, t, currentMagnitude, obscuration, result);
    }

    _drawSolarEclipse(ctx, cx, cy, sunR, moonR, moonOffset, obscuration, currentMagnitude) {
        const haloR = sunR * 1.6;
        const halo = ctx.createRadialGradient(cx, cy, sunR * 0.8, cx, cy, haloR);
        halo.addColorStop(0, 'rgba(255,200,80,0.5)');
        halo.addColorStop(0.4, 'rgba(255,160,40,0.2)');
        halo.addColorStop(1, 'rgba(255,120,20,0)');
        ctx.fillStyle = halo;
        ctx.beginPath();
        ctx.arc(cx, cy, haloR, 0, Math.PI * 2);
        ctx.fill();

        const sunGrad = ctx.createRadialGradient(
            cx - sunR * 0.3, cy - sunR * 0.3, 0,
            cx, cy, sunR
        );
        sunGrad.addColorStop(0, '#fff8d0');
        sunGrad.addColorStop(0.3, '#ffdd60');
        sunGrad.addColorStop(0.7, '#ffaa20');
        sunGrad.addColorStop(1, '#ff7010');
        ctx.fillStyle = sunGrad;
        ctx.beginPath();
        ctx.arc(cx, cy, sunR, 0, Math.PI * 2);
        ctx.fill();

        ctx.strokeStyle = 'rgba(255,180,40,0.8)';
        ctx.lineWidth = 1.5;
        ctx.beginPath();
        ctx.arc(cx, cy, sunR, 0, Math.PI * 2);
        ctx.stroke();

        const mx = cx + moonOffset;
        const my = cy;

        ctx.save();
        const moonGrad = ctx.createRadialGradient(
            mx - moonR * 0.3, my - moonR * 0.3, 0,
            mx, my, moonR
        );
        moonGrad.addColorStop(0, '#4a4a5a');
        moonGrad.addColorStop(0.6, '#2a2a3a');
        moonGrad.addColorStop(1, '#1a1a2a');
        ctx.fillStyle = moonGrad;
        ctx.beginPath();
        ctx.arc(mx, my, moonR, 0, Math.PI * 2);
        ctx.fill();

        ctx.fillStyle = 'rgba(30,30,45,0.5)';
        const craters = [
            [-moonR * 0.3, -moonR * 0.2, moonR * 0.15],
            [moonR * 0.2, moonR * 0.3, moonR * 0.12],
            [-moonR * 0.1, moonR * 0.35, moonR * 0.08],
            [moonR * 0.35, -moonR * 0.1, moonR * 0.1],
        ];
        craters.forEach(([dx, dy, r]) => {
            ctx.beginPath();
            ctx.arc(mx + dx, my + dy, r, 0, Math.PI * 2);
            ctx.fill();
        });

        ctx.restore();

        if (obscuration > 0.95) {
            const coronaR = sunR * 1.8;
            const corona = ctx.createRadialGradient(cx, cy, sunR * 0.95, cx, cy, coronaR);
            corona.addColorStop(0, 'rgba(200,220,255,0.8)');
            corona.addColorStop(0.3, 'rgba(150,180,255,0.4)');
            corona.addColorStop(0.6, 'rgba(100,140,255,0.15)');
            corona.addColorStop(1, 'rgba(80,120,255,0)');
            ctx.fillStyle = corona;
            ctx.beginPath();
            ctx.arc(cx, cy, coronaR, 0, Math.PI * 2);
            ctx.fill();
        }
    }

    _drawLunarEclipse(ctx, cx, cy, sunR, moonR, moonOffset, obscuration, currentMagnitude) {
        const mx = cx;
        const my = cy;

        const moonGrad = ctx.createRadialGradient(
            mx - moonR * 0.3, my - moonR * 0.3, 0,
            mx, my, moonR
        );
        moonGrad.addColorStop(0, '#e8e8f0');
        moonGrad.addColorStop(0.5, '#c0c0d0');
        moonGrad.addColorStop(1, '#8a8aa0');
        ctx.fillStyle = moonGrad;
        ctx.beginPath();
        ctx.arc(mx, my, moonR, 0, Math.PI * 2);
        ctx.fill();

        ctx.fillStyle = 'rgba(140,140,160,0.4)';
        const maria = [
            [-moonR * 0.25, -moonR * 0.15, moonR * 0.25],
            [moonR * 0.2, moonR * 0.25, moonR * 0.2],
            [-moonR * 0.35, moonR * 0.1, moonR * 0.15],
            [moonR * 0.1, -moonR * 0.3, moonR * 0.18],
        ];
        maria.forEach(([dx, dy, r]) => {
            ctx.beginPath();
            ctx.arc(mx + dx, my + dy, r, 0, Math.PI * 2);
            ctx.fill();
        });

        const shadowX = cx + moonOffset;
        const shadowR = moonR * 1.3;

        ctx.save();
        ctx.globalCompositeOperation = 'source-over';

        const shadowGrad = ctx.createRadialGradient(
            shadowX, cy, shadowR * 0.3,
            shadowX, cy, shadowR
        );
        shadowGrad.addColorStop(0, 'rgba(10,5,20,0.95)');
        shadowGrad.addColorStop(0.6, 'rgba(30,10,40,0.85)');
        shadowGrad.addColorStop(1, 'rgba(60,20,60,0)');

        ctx.fillStyle = shadowGrad;
        ctx.beginPath();
        ctx.arc(shadowX, cy, shadowR, 0, Math.PI * 2);
        ctx.fill();

        if (obscuration > 0.9) {
            ctx.globalCompositeOperation = 'source-atop';
            const copperGrad = ctx.createRadialGradient(mx, my, 0, mx, my, moonR);
            copperGrad.addColorStop(0, 'rgba(180,60,20,0.7)');
            copperGrad.addColorStop(0.5, 'rgba(120,40,15,0.6)');
            copperGrad.addColorStop(1, 'rgba(80,30,10,0.4)');
            ctx.fillStyle = copperGrad;
            ctx.beginPath();
            ctx.arc(mx, my, moonR, 0, Math.PI * 2);
            ctx.fill();
        }

        ctx.restore();
    }

    _calcObscuration(r1, r2, d) {
        if (d >= r1 + r2) return 0;
        if (d <= Math.abs(r1 - r2)) return Math.min(r1, r2) ** 2 / (r1 ** 2);
        const a1 = r1 * r1 * Math.acos((d * d + r1 * r1 - r2 * r2) / (2 * d * r1));
        const a2 = r2 * r2 * Math.acos((d * d + r2 * r2 - r1 * r1) / (2 * d * r2));
        const a3 = 0.5 * Math.sqrt((-d + r1 + r2) * (d + r1 - r2) * (d - r1 + r2) * (d + r1 + r2));
        return (a1 + a2 - a3) / (Math.PI * r1 * r1);
    }

    _drawAnimationInfo(ctx, w, h, t, magnitude, obscuration, result) {
        const phase = t < -0.5 ? '初亏前' :
            t < 0 ? '初亏 → 食甚' :
            t < 0.5 ? '食甚 → 复圆' : '复圆后';

        ctx.fillStyle = 'rgba(10,18,38,0.85)';
        ctx.fillRect(6, 6, 140, 54);
        ctx.strokeStyle = 'rgba(80,120,200,0.3)';
        ctx.lineWidth = 1;
        ctx.strokeRect(6, 6, 140, 54);

        ctx.font = '10px -apple-system, sans-serif';
        ctx.fillStyle = '#8090b0';
        ctx.fillText('阶段', 12, 20);
        ctx.fillStyle = '#a0c8ff';
        ctx.fillText(phase, 48, 20);

        ctx.fillStyle = '#8090b0';
        ctx.fillText('食分', 12, 35);
        ctx.fillStyle = '#ffcc60';
        ctx.font = '11px Consolas, monospace';
        ctx.fillText(magnitude.toFixed(3), 48, 35);

        ctx.fillStyle = '#8090b0';
        ctx.font = '10px -apple-system, sans-serif';
        ctx.fillText('遮挡', 12, 50);
        ctx.fillStyle = '#70c0ff';
        ctx.font = '11px Consolas, monospace';
        ctx.fillText((obscuration * 100).toFixed(1) + '%', 48, 50);

        const isLunar = (result.eclipse_type || '').toLowerCase().includes('lunar');
        ctx.fillStyle = isLunar ? '#a0a0c0' : '#ffcc60';
        ctx.font = 'bold 12px -apple-system, sans-serif';
        ctx.fillText(isLunar ? '☾ 月食' : '☀ 日食', w - 60, 20);

        const cls = this._getClassificationLabel(result.eclipse_classification);
        ctx.fillStyle = '#8090b0';
        ctx.font = '10px -apple-system, sans-serif';
        ctx.fillText(cls, w - 60, 36);
    }

    startAnimation(canvas, result) {
        this.stopAnimation();
        if (!canvas || !result) return;

        const loop = () => {
            this.drawEclipseAnimation(canvas, result);
            this._animationFrame = requestAnimationFrame(loop);
        };
        this._animationFrame = requestAnimationFrame(loop);
    }

    stopAnimation() {
        if (this._animationFrame) {
            cancelAnimationFrame(this._animationFrame);
            this._animationFrame = null;
        }
        this._animationStartTime = 0;
    }
}

const EclipseView = EclipsePredictorView;
window.EclipsePredictorView = EclipsePredictorView;
window.EclipseView = EclipseView;
