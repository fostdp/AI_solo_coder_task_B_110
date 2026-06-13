/* ============================================================
 * variable.js - 变星亮度演化分析可视化模块
 * 职责:
 *   - 变星列表渲染与筛选
 *   - 光变曲线可视化 (长期/相位折叠)
 *   - Lomb-Scargle 周期图可视化
 *   - 周期变化分析面板
 * ============================================================ */

class VariableStarAnalyzer {
    constructor() {
        this.API_BASE = (() => {
            const p = window.location.protocol;
            const h = window.location.hostname;
            const port = window.location.port === '8080' ? '8080' : '8080';
            return `${p}//${h}:${port}/api`;
        })();

        this.variables = [];
        this.currentVariable = null;
        this.currentMeasurements = [];
        this.currentReconstruction = null;
        this.currentPeriodogram = null;
        this.currentPeriodChange = null;

        this.phaseFoldMode = false;
        this.currentPeriod = null;

        this.onVariableSelected = null;
        this.onReconstructionComplete = null;
        this.onPeriodAnalysisComplete = null;

        this._hoveredPoint = null;
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

    async fetchVariables(params = {}) {
        const q = new URLSearchParams();
        for (const [k, v] of Object.entries(params)) {
            if (v != null && v !== '') q.set(k, v);
        }
        const s = q.toString();
        const data = await this._json(`${this.API_BASE}/variables${s ? '?' + s : ''}`);
        this.variables = Array.isArray(data) ? data : (data?.records || []);
        return this.variables;
    }

    async fetchVariable(id) {
        const data = await this._json(`${this.API_BASE}/variables/${id}`);
        this.currentVariable = data;
        return data;
    }

    async fetchMeasurements(id) {
        const data = await this._json(`${this.API_BASE}/variables/${id}/measurements`);
        this.currentMeasurements = Array.isArray(data) ? data : (data?.measurements || []);
        return this.currentMeasurements;
    }

    async reconstruct(id, opts = {}) {
        const data = await this._json(
            `${this.API_BASE}/variables/${id}/reconstruct`,
            'POST',
            opts
        );
        this.currentReconstruction = data;
        if (this.onReconstructionComplete) {
            this.onReconstructionComplete(data);
        }
        return data;
    }

    async analyzePeriod(id, opts = {}) {
        const data = await this._json(
            `${this.API_BASE}/variables/${id}/period-analysis`,
            'POST',
            opts
        );
        this.currentPeriodogram = data;
        if (data?.best_period) {
            this.currentPeriod = data.best_period;
        }
        if (this.onPeriodAnalysisComplete) {
            this.onPeriodAnalysisComplete(data);
        }
        return data;
    }

    // ============================================================
    // 辅助方法
    // ============================================================

    _getVariableTypeColor(type) {
        const colorMap = {
            'Mira': '#ff6060',
            'RR Lyrae': '#ffaa40',
            'Cepheid': '#ffdd40',
            'Delta Scuti': '#80ff80',
            'Eclipsing': '#40c0ff',
            'RS CVn': '#8080ff',
            'Irregular': '#c080ff',
            'Semi-regular': '#ff80c0',
        };
        return colorMap[type] || '#8090b0';
    }

    _getVariableTypeLabel(type) {
        return type || '未知类型';
    }

    _formatYear(yr) {
        if (yr == null) return '-';
        const y = Math.round(yr);
        return y < 0 ? `前${-y}` : `公元 ${y}`;
    }

    _formatPeriod(periodDays) {
        if (periodDays == null) return '-';
        if (periodDays < 1) {
            return `${(periodDays * 24).toFixed(2)} h`;
        }
        if (periodDays < 100) {
            return `${periodDays.toFixed(2)} 天`;
        }
        if (periodDays < 365) {
            return `${periodDays.toFixed(1)} 天`;
        }
        return `${(periodDays / 365.25).toFixed(2)} 年`;
    }

    // ============================================================
    // 列表渲染
    // ============================================================

    renderVariableList(container, variables) {
        if (!container) return;
        const list = Array.isArray(variables) ? variables : this.variables;

        if (!list || list.length === 0) {
            container.innerHTML = `
                <div style="padding:30px;text-align:center;color:#8090b0;">
                    暂无变星数据
                </div>
            `;
            return;
        }

        container.innerHTML = `
            <div class="panel-section">
                <h3 style="margin:0 0 10px 0;color:#a0c8ff;font-size:13px;padding-bottom:6px;
                    border-bottom:1px solid rgba(80,120,200,0.2);">
                    变星列表 (${list.length})
                </h3>
                <div class="stars-list" id="variable-list-container">
                    ${list.map((v, i) => this._renderVariableItem(v, i)).join('')}
                </div>
            </div>
        `;

        container.querySelectorAll('.variable-item').forEach(el => {
            el.addEventListener('click', () => {
                const id = parseInt(el.dataset.id);
                const variable = list.find(v => v.id === id);
                if (variable) {
                    container.querySelectorAll('.variable-item').forEach(x => x.classList.remove('active'));
                    el.classList.add('active');
                    this.currentVariable = variable;
                    if (this.onVariableSelected) {
                        this.onVariableSelected(variable);
                    }
                }
            });
        });
    }

    _renderVariableItem(v, idx) {
        const color = this._getVariableTypeColor(v.variable_type);
        const typeLabel = this._getVariableTypeLabel(v.variable_type);
        const period = v.period_days != null ? this._formatPeriod(v.period_days) : '-';
        const ampRange = v.amplitude_min != null && v.amplitude_max != null
            ? `${v.amplitude_min.toFixed(2)} - ${v.amplitude_max.toFixed(2)} mag`
            : (v.amplitude != null ? `${v.amplitude.toFixed(2)} mag` : '-');

        return `
            <div class="variable-item star-item" data-id="${v.id}">
                <div style="display:flex;align-items:center;gap:8px;min-width:0;">
                    <span style="display:inline-block;width:3px;height:20px;background:${color};
                        border-radius:2px;flex-shrink:0;"></span>
                    <span style="font-size:14px;flex-shrink:0;">✦</span>
                    <div style="min-width:0;">
                        <div style="color:#e0e8ff;font-size:12px;white-space:nowrap;
                            overflow:hidden;text-overflow:ellipsis;">
                            ${v.name || v.variable_id_code || `Variable-${idx + 1}`}
                        </div>
                        <div style="color:#8090b0;font-size:10px;">
                            ${typeLabel} · 周期 ${period}
                        </div>
                    </div>
                </div>
                <div style="text-align:right;flex-shrink:0;">
                    <div style="color:${color};font-size:11px;">${typeLabel}</div>
                    <div style="color:#ffcc60;font-size:10px;font-family:Consolas,monospace;">
                        ${ampRange}
                    </div>
                </div>
            </div>
        `;
    }

    // ============================================================
    // 光变曲线可视化
    // ============================================================

    renderLightCurve(canvas, reconstruction) {
        if (!canvas) return;
        const data = reconstruction || this.currentReconstruction;
        if (!data) return;

        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        const dpr = window.devicePixelRatio || 1;
        const w = canvas.clientWidth || canvas.width || 600;
        const h = canvas.clientHeight || canvas.height || 300;
        canvas.width = w * dpr;
        canvas.height = h * dpr;
        canvas.style.width = w + 'px';
        canvas.style.height = h + 'px';
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

        ctx.clearRect(0, 0, w, h);

        const pad = { left: 55, right: 20, top: 20, bottom: 40 };
        const plotW = w - pad.left - pad.right;
        const plotH = h - pad.top - pad.bottom;

        const measurements = data.measurements || this.currentMeasurements || [];
        const modelCurve = data.model_curve || [];
        const confidenceBand = data.confidence_band || null;

        const ancientPts = measurements.filter(m => m.era === 'ancient' || m.is_ancient);
        const modernPts = measurements.filter(m => m.era === 'modern' || !m.is_ancient);

        let xData, yData, yErrData;
        if (this.phaseFoldMode && this.currentPeriod) {
            xData = measurements.map(m => {
                const phase = ((m.epoch_yr * 365.25) % this.currentPeriod) / this.currentPeriod;
                return phase < 0 ? phase + 1 : phase;
            });
            yData = measurements.map(m => m.magnitude);
            yErrData = measurements.map(m => m.magnitude_error || 0);
        } else {
            xData = measurements.map(m => m.epoch_yr);
            yData = measurements.map(m => m.magnitude);
            yErrData = measurements.map(m => m.magnitude_error || 0);
        }

        const allY = [...yData];
        if (modelCurve.length > 0) {
            allY.push(...modelCurve.map(p => p.magnitude));
        }
        if (confidenceBand && confidenceBand.upper) {
            allY.push(...confidenceBand.upper.map(p => p.magnitude));
        }
        if (confidenceBand && confidenceBand.lower) {
            allY.push(...confidenceBand.lower.map(p => p.magnitude));
        }

        let yMin = Math.min(...allY) - 0.2;
        let yMax = Math.max(...allY) + 0.2;
        let xMin, xMax;

        if (this.phaseFoldMode) {
            xMin = 0;
            xMax = 1;
        } else {
            xMin = Math.min(...xData);
            xMax = Math.max(...xData);
            if (modelCurve.length > 0) {
                const modelX = modelCurve.map(p => p.epoch_yr);
                xMin = Math.min(xMin, ...modelX);
                xMax = Math.max(xMax, ...modelX);
            }
            const xPad = (xMax - xMin) * 0.05;
            xMin -= xPad;
            xMax += xPad;
        }

        const xScale = x => pad.left + (x - xMin) / (xMax - xMin) * plotW;
        const yScale = y => pad.top + (1 - (y - yMin) / (yMax - yMin)) * plotH;

        this._drawAxes(ctx, pad, plotW, plotH, xMin, xMax, yMin, yMax,
            this.phaseFoldMode ? '相位' : '年份 (年)',
            '星等 (mag)');

        if (confidenceBand && confidenceBand.upper && confidenceBand.lower && modelCurve.length > 0) {
            this._drawConfidenceBand(ctx, confidenceBand, xScale, yScale, xMin, xMax);
        }

        if (modelCurve.length > 0) {
            this._drawModelCurve(ctx, modelCurve, xScale, yScale, this.phaseFoldMode, this.currentPeriod);
        }

        this._drawMeasurements(ctx, ancientPts, modernPts, xScale, yScale,
            this.phaseFoldMode, this.currentPeriod);

        this._drawLightCurveLegend(ctx, w, h);

        this._bindLightCurveHover(canvas, measurements, xScale, yScale,
            this.phaseFoldMode, this.currentPeriod);
    }

    _drawAxes(ctx, pad, plotW, plotH, xMin, xMax, yMin, yMax, xLabel, yLabel) {
        ctx.strokeStyle = 'rgba(80,120,200,0.3)';
        ctx.lineWidth = 1;

        ctx.beginPath();
        ctx.moveTo(pad.left, pad.top);
        ctx.lineTo(pad.left, pad.top + plotH);
        ctx.lineTo(pad.left + plotW, pad.top + plotH);
        ctx.stroke();

        ctx.strokeStyle = 'rgba(80,120,200,0.1)';
        ctx.lineWidth = 0.8;

        const yTicks = 5;
        for (let i = 0; i <= yTicks; i++) {
            const y = pad.top + (i / yTicks) * plotH;
            const val = yMax - (i / yTicks) * (yMax - yMin);
            ctx.beginPath();
            ctx.moveTo(pad.left, y);
            ctx.lineTo(pad.left + plotW, y);
            ctx.stroke();

            ctx.fillStyle = '#8090b0';
            ctx.font = '10px Consolas, monospace';
            ctx.textAlign = 'right';
            ctx.fillText(val.toFixed(1), pad.left - 6, y + 3);
        }

        const xTicks = 6;
        for (let i = 0; i <= xTicks; i++) {
            const x = pad.left + (i / xTicks) * plotW;
            const val = xMin + (i / xTicks) * (xMax - xMin);
            ctx.beginPath();
            ctx.moveTo(x, pad.top + plotH);
            ctx.lineTo(x, pad.top + plotH + 4);
            ctx.stroke();

            ctx.fillStyle = '#8090b0';
            ctx.font = '10px Consolas, monospace';
            ctx.textAlign = 'center';
            const label = Math.abs(val) < 10 ? val.toFixed(2) :
                Math.abs(val) < 100 ? val.toFixed(0) : val.toFixed(0);
            ctx.fillText(label, x, pad.top + plotH + 16);
        }

        ctx.fillStyle = '#a0c8ff';
        ctx.font = '11px -apple-system, sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(xLabel, pad.left + plotW / 2, pad.top + plotH + 32);

        ctx.save();
        ctx.translate(14, pad.top + plotH / 2);
        ctx.rotate(-Math.PI / 2);
        ctx.textAlign = 'center';
        ctx.fillText(yLabel, 0, 0);
        ctx.restore();
    }

    _drawConfidenceBand(ctx, band, xScale, yScale, xMin, xMax) {
        if (!band.upper || !band.lower) return;

        const upper = band.upper;
        const lower = band.lower;

        ctx.fillStyle = 'rgba(80,180,255,0.15)';
        ctx.beginPath();

        for (let i = 0; i < upper.length; i++) {
            const x = xScale(upper[i].epoch_yr);
            const y = yScale(upper[i].magnitude);
            if (i === 0) ctx.moveTo(x, y);
            else ctx.lineTo(x, y);
        }

        for (let i = lower.length - 1; i >= 0; i--) {
            const x = xScale(lower[i].epoch_yr);
            const y = yScale(lower[i].magnitude);
            ctx.lineTo(x, y);
        }

        ctx.closePath();
        ctx.fill();
    }

    _drawModelCurve(ctx, curve, xScale, yScale, phaseFold, period) {
        ctx.strokeStyle = '#40c0ff';
        ctx.lineWidth = 1.5;
        ctx.beginPath();

        if (phaseFold && period) {
            const phasePts = curve.map(p => {
                let phase = ((p.epoch_yr * 365.25) % period) / period;
                if (phase < 0) phase += 1;
                return { phase, mag: p.magnitude };
            }).sort((a, b) => a.phase - b.phase);

            for (let i = 0; i < phasePts.length; i++) {
                const x = xScale(phasePts[i].phase);
                const y = yScale(phasePts[i].mag);
                if (i === 0) ctx.moveTo(x, y);
                else ctx.lineTo(x, y);
            }
        } else {
            for (let i = 0; i < curve.length; i++) {
                const x = xScale(curve[i].epoch_yr);
                const y = yScale(curve[i].magnitude);
                if (i === 0) ctx.moveTo(x, y);
                else ctx.lineTo(x, y);
            }
        }

        ctx.stroke();
    }

    _drawMeasurements(ctx, ancient, modern, xScale, yScale, phaseFold, period) {
        const drawPoint = (m, color, shape) => {
            let xVal;
            if (phaseFold && period) {
                xVal = ((m.epoch_yr * 365.25) % period) / period;
                if (xVal < 0) xVal += 1;
            } else {
                xVal = m.epoch_yr;
            }
            const x = xScale(xVal);
            const y = yScale(m.magnitude);
            const err = m.magnitude_error || 0;

            if (err > 0) {
                ctx.strokeStyle = color;
                ctx.lineWidth = 1;
                ctx.globalAlpha = 0.6;
                ctx.beginPath();
                ctx.moveTo(x, yScale(m.magnitude - err));
                ctx.lineTo(x, yScale(m.magnitude + err));
                ctx.stroke();

                ctx.beginPath();
                ctx.moveTo(x - 3, yScale(m.magnitude - err));
                ctx.lineTo(x + 3, yScale(m.magnitude - err));
                ctx.stroke();

                ctx.beginPath();
                ctx.moveTo(x - 3, yScale(m.magnitude + err));
                ctx.lineTo(x + 3, yScale(m.magnitude + err));
                ctx.stroke();
                ctx.globalAlpha = 1;
            }

            ctx.fillStyle = color;
            ctx.strokeStyle = color;
            ctx.lineWidth = 1;

            if (shape === 'circle') {
                ctx.beginPath();
                ctx.arc(x, y, 4, 0, Math.PI * 2);
                ctx.fill();
            } else if (shape === 'square') {
                ctx.fillRect(x - 3.5, y - 3.5, 7, 7);
            }
        };

        ancient.forEach(m => drawPoint(m, '#ff8c40', 'circle'));
        modern.forEach(m => drawPoint(m, '#4080ff', 'square'));
    }

    _drawLightCurveLegend(ctx, w, h) {
        const lx = w - 130, ly = 10;
        ctx.fillStyle = 'rgba(10,18,38,0.85)';
        ctx.strokeStyle = 'rgba(80,120,200,0.3)';
        ctx.lineWidth = 1;
        ctx.fillRect(lx, ly, 120, 78);
        ctx.strokeRect(lx, ly, 120, 78);

        ctx.font = '11px -apple-system, sans-serif';

        ctx.fillStyle = '#ff8c40';
        ctx.beginPath();
        ctx.arc(lx + 12, ly + 20, 4, 0, Math.PI * 2);
        ctx.fill();
        ctx.fillStyle = '#e0e8ff';
        ctx.fillText('古代测量', lx + 24, ly + 24);

        ctx.fillStyle = '#4080ff';
        ctx.fillRect(lx + 8, ly + 36, 8, 8);
        ctx.fillStyle = '#e0e8ff';
        ctx.fillText('现代测量', lx + 24, ly + 44);

        ctx.strokeStyle = '#40c0ff';
        ctx.lineWidth = 1.5;
        ctx.beginPath();
        ctx.moveTo(lx + 8, ly + 60);
        ctx.lineTo(lx + 20, ly + 60);
        ctx.stroke();
        ctx.fillStyle = '#e0e8ff';
        ctx.fillText('模型拟合', lx + 24, ly + 64);

        ctx.fillStyle = 'rgba(80,180,255,0.3)';
        ctx.fillRect(lx + 8, ly + 70, 12, 6);
        ctx.fillStyle = '#e0e8ff';
        ctx.fillText('置信区间', lx + 24, ly + 76);
    }

    _bindLightCurveHover(canvas, measurements, xScale, yScale, phaseFold, period) {
        canvas.onmousemove = (e) => {
            const rect = canvas.getBoundingClientRect();
            const mx = e.clientX - rect.left;
            const my = e.clientY - rect.top;

            let closest = null;
            let minDist = 12;

            measurements.forEach(m => {
                let xVal;
                if (phaseFold && period) {
                    xVal = ((m.epoch_yr * 365.25) % period) / period;
                    if (xVal < 0) xVal += 1;
                } else {
                    xVal = m.epoch_yr;
                }
                const px = xScale(xVal);
                const py = yScale(m.magnitude);
                const d = Math.hypot(mx - px, my - py);
                if (d < minDist) {
                    minDist = d;
                    closest = m;
                }
            });

            this._hoveredPoint = closest;
            canvas.style.cursor = closest ? 'pointer' : 'default';
        };

        canvas.onmouseleave = () => {
            this._hoveredPoint = null;
        };
    }

    setPhaseFoldMode(enabled, period) {
        this.phaseFoldMode = enabled;
        if (period) this.currentPeriod = period;
    }

    // ============================================================
    // 周期图可视化
    // ============================================================

    renderPeriodogram(canvas, periodogram) {
        if (!canvas) return;
        const data = periodogram || this.currentPeriodogram;
        if (!data) return;

        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        const dpr = window.devicePixelRatio || 1;
        const w = canvas.clientWidth || canvas.width || 600;
        const h = canvas.clientHeight || canvas.height || 280;
        canvas.width = w * dpr;
        canvas.height = h * dpr;
        canvas.style.width = w + 'px';
        canvas.style.height = h + 'px';
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

        ctx.clearRect(0, 0, w, h);

        const pad = { left: 55, right: 20, top: 20, bottom: 45 };
        const plotW = w - pad.left - pad.right;
        const plotH = h - pad.top - pad.bottom;

        const freqs = data.frequencies || [];
        const powers = data.powers || [];
        const peaks = data.peaks || [];
        const fapThreshold = data.fap_threshold || null;

        const periods = freqs.map(f => 1 / f);
        const logPeriods = periods.map(p => Math.log10(p));

        let yMin = 0;
        let yMax = Math.max(...powers) * 1.1;
        if (fapThreshold) {
            yMax = Math.max(yMax, fapThreshold * 1.1);
        }
        const xMin = Math.min(...logPeriods);
        const xMax = Math.max(...logPeriods);

        const xScale = x => pad.left + (x - xMin) / (xMax - xMin) * plotW;
        const yScale = y => pad.top + (1 - (y - yMin) / (yMax - yMin)) * plotH;

        this._drawPeriodogramAxes(ctx, pad, plotW, plotH, xMin, xMax, yMin, yMax);

        if (fapThreshold != null) {
            ctx.strokeStyle = 'rgba(255,80,80,0.6)';
            ctx.lineWidth = 1;
            ctx.setLineDash([5, 3]);
            ctx.beginPath();
            const y = yScale(fapThreshold);
            ctx.moveTo(pad.left, y);
            ctx.lineTo(pad.left + plotW, y);
            ctx.stroke();
            ctx.setLineDash([]);

            ctx.fillStyle = 'rgba(255,80,80,0.8)';
            ctx.font = '9px Consolas, monospace';
            ctx.textAlign = 'left';
            ctx.fillText('FAP 阈值', pad.left + 6, y - 3);
        }

        ctx.strokeStyle = '#6090ff';
        ctx.lineWidth = 1;
        ctx.beginPath();
        for (let i = 0; i < logPeriods.length; i++) {
            const x = xScale(logPeriods[i]);
            const y = yScale(powers[i]);
            if (i === 0) ctx.moveTo(x, y);
            else ctx.lineTo(x, y);
        }
        ctx.stroke();

        const fundColor = '#ff4040';
        const harmColor = '#ffaa40';

        peaks.forEach((peak, idx) => {
            const period = peak.period_days || (1 / peak.frequency);
            const logP = Math.log10(period);
            const x = xScale(logP);
            const y = yScale(peak.power || 0);

            const isFundamental = idx === 0 || peak.is_fundamental;
            const color = isFundamental ? fundColor : harmColor;

            ctx.strokeStyle = color;
            ctx.lineWidth = 1;
            ctx.setLineDash([3, 2]);
            ctx.beginPath();
            ctx.moveTo(x, pad.top + plotH);
            ctx.lineTo(x, y);
            ctx.stroke();
            ctx.setLineDash([]);

            ctx.fillStyle = color;
            ctx.strokeStyle = '#fff';
            ctx.lineWidth = 1.5;
            ctx.beginPath();
            ctx.arc(x, y, 5, 0, Math.PI * 2);
            ctx.fill();
            ctx.stroke();

            ctx.fillStyle = color;
            ctx.font = '9px Consolas, monospace';
            ctx.textAlign = 'center';
            const label = period < 1 ? `${(period * 24).toFixed(1)}h` :
                period < 100 ? `${period.toFixed(2)}d` : `${period.toFixed(0)}d`;
            ctx.fillText(label, x, y - 8);
        });

        this._drawPeriodogramLegend(ctx, w, h);
    }

    _drawPeriodogramAxes(ctx, pad, plotW, plotH, xMin, xMax, yMin, yMax) {
        ctx.strokeStyle = 'rgba(80,120,200,0.3)';
        ctx.lineWidth = 1;

        ctx.beginPath();
        ctx.moveTo(pad.left, pad.top);
        ctx.lineTo(pad.left, pad.top + plotH);
        ctx.lineTo(pad.left + plotW, pad.top + plotH);
        ctx.stroke();

        ctx.strokeStyle = 'rgba(80,120,200,0.1)';
        ctx.lineWidth = 0.8;

        const yTicks = 5;
        for (let i = 0; i <= yTicks; i++) {
            const y = pad.top + (i / yTicks) * plotH;
            const val = yMax - (i / yTicks) * (yMax - yMin);
            ctx.beginPath();
            ctx.moveTo(pad.left, y);
            ctx.lineTo(pad.left + plotW, y);
            ctx.stroke();

            ctx.fillStyle = '#8090b0';
            ctx.font = '10px Consolas, monospace';
            ctx.textAlign = 'right';
            ctx.fillText(val.toFixed(2), pad.left - 6, y + 3);
        }

        const xTicks = 6;
        for (let i = 0; i <= xTicks; i++) {
            const x = pad.left + (i / xTicks) * plotW;
            const logVal = xMin + (i / xTicks) * (xMax - xMin);
            const val = Math.pow(10, logVal);
            ctx.beginPath();
            ctx.moveTo(x, pad.top + plotH);
            ctx.lineTo(x, pad.top + plotH + 4);
            ctx.stroke();

            ctx.fillStyle = '#8090b0';
            ctx.font = '10px Consolas, monospace';
            ctx.textAlign = 'center';
            let label;
            if (val < 1) label = `${(val * 24).toFixed(0)}h`;
            else if (val < 100) label = `${val.toFixed(0)}d`;
            else if (val < 365) label = `${val.toFixed(0)}d`;
            else label = `${(val / 365.25).toFixed(1)}y`;
            ctx.fillText(label, x, pad.top + plotH + 16);
        }

        ctx.fillStyle = '#a0c8ff';
        ctx.font = '11px -apple-system, sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText('周期 (天, 对数刻度)', pad.left + plotW / 2, pad.top + plotH + 34);

        ctx.save();
        ctx.translate(14, pad.top + plotH / 2);
        ctx.rotate(-Math.PI / 2);
        ctx.textAlign = 'center';
        ctx.fillText('功率', 0, 0);
        ctx.restore();
    }

    _drawPeriodogramLegend(ctx, w, h) {
        const lx = w - 110, ly = 10;
        ctx.fillStyle = 'rgba(10,18,38,0.85)';
        ctx.strokeStyle = 'rgba(80,120,200,0.3)';
        ctx.lineWidth = 1;
        ctx.fillRect(lx, ly, 100, 58);
        ctx.strokeRect(lx, ly, 100, 58);

        ctx.font = '11px -apple-system, sans-serif';

        ctx.strokeStyle = '#6090ff';
        ctx.lineWidth = 1.5;
        ctx.beginPath();
        ctx.moveTo(lx + 8, ly + 20);
        ctx.lineTo(lx + 22, ly + 20);
        ctx.stroke();
        ctx.fillStyle = '#e0e8ff';
        ctx.fillText('周期图', lx + 28, ly + 24);

        ctx.fillStyle = '#ff4040';
        ctx.beginPath();
        ctx.arc(lx + 15, ly + 38, 4, 0, Math.PI * 2);
        ctx.fill();
        ctx.fillStyle = '#e0e8ff';
        ctx.fillText('基频', lx + 28, ly + 42);

        ctx.fillStyle = '#ffaa40';
        ctx.beginPath();
        ctx.arc(lx + 15, ly + 52, 4, 0, Math.PI * 2);
        ctx.fill();
        ctx.fillStyle = '#e0e8ff';
        ctx.fillText('谐波', lx + 28, ly + 56);
    }

    // ============================================================
    // 周期变化分析
    // ============================================================

    renderPeriodChange(container, result) {
        if (!container) return;
        const data = result || this.currentPeriodChange;
        if (!data) return;

        const ancientPeriod = data.ancient_period;
        const modernPeriod = data.modern_period;
        const periodChangeRate = data.period_change_rate;
        const periodChangeSignificance = data.period_change_significance;
        const brightnessTrend = data.brightness_trend;
        const brightnessTrendError = data.brightness_trend_error;

        const periodDiff = ancientPeriod != null && modernPeriod != null
            ? modernPeriod - ancientPeriod : null;
        const periodDiffPct = periodDiff != null && ancientPeriod != null
            ? (periodDiff / ancientPeriod) * 100 : null;

        const pdotSignificant = periodChangeSignificance != null && periodChangeSignificance > 2;
        const trendSignificant = brightnessTrendError != null && brightnessTrend != null &&
            Math.abs(brightnessTrend) > Math.abs(brightnessTrendError) * 2;

        const pdotColor = pdotSignificant ? '#ff6060' : '#8090b0';
        const trendColor = trendSignificant ? '#ffcc60' : '#8090b0';

        container.innerHTML = `
            <div class="panel-section">
                <h3 style="margin:0 0 12px 0;color:#a0c8ff;font-size:13px;padding-bottom:6px;
                    border-bottom:1px solid rgba(80,120,200,0.2);">
                    周期变化分析
                </h3>

                <div style="display:grid;grid-template-columns:1fr 1fr;gap:10px;margin-bottom:14px;">
                    <div style="padding:10px 12px;border-radius:5px;
                        background:rgba(255,140,64,0.1);border:1px solid rgba(255,140,64,0.3);">
                        <div style="font-size:10px;color:#ffaa60;margin-bottom:4px;">古代周期</div>
                        <div style="font-size:16px;font-family:Consolas,monospace;color:#ffcc80;">
                            ${ancientPeriod != null ? this._formatPeriod(ancientPeriod) : '-'}
                        </div>
                        ${data.ancient_period_error != null ? `
                            <div style="font-size:10px;color:#8090b0;font-family:Consolas,monospace;">
                                ± ${data.ancient_period_error.toFixed(3)} d
                            </div>
                        ` : ''}
                    </div>

                    <div style="padding:10px 12px;border-radius:5px;
                        background:rgba(64,128,255,0.1);border:1px solid rgba(64,128,255,0.3);">
                        <div style="font-size:10px;color:#6090ff;margin-bottom:4px;">现代周期</div>
                        <div style="font-size:16px;font-family:Consolas,monospace;color:#80b0ff;">
                            ${modernPeriod != null ? this._formatPeriod(modernPeriod) : '-'}
                        </div>
                        ${data.modern_period_error != null ? `
                            <div style="font-size:10px;color:#8090b0;font-family:Consolas,monospace;">
                                ± ${data.modern_period_error.toFixed(3)} d
                            </div>
                        ` : ''}
                    </div>
                </div>

                ${periodDiff != null ? `
                    <div style="padding:8px 12px;border-radius:4px;
                        background:rgba(80,120,200,0.1);margin-bottom:12px;">
                        <div style="display:flex;justify-content:space-between;align-items:center;">
                            <span style="font-size:11px;color:#8090b0;">周期变化量</span>
                            <span style="font-family:Consolas,monospace;font-size:12px;
                                color:${periodDiff >= 0 ? '#ff8080' : '#80ff80'};">
                                ${periodDiff >= 0 ? '+' : ''}${periodDiff.toFixed(4)} 天
                                (${periodDiffPct >= 0 ? '+' : ''}${periodDiffPct.toFixed(2)}%)
                            </span>
                        </div>
                    </div>
                ` : ''}

                <div style="margin-bottom:14px;">
                    <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:6px;">
                        <span style="font-size:11px;color:#8090b0;">周期变化率 Ṗ</span>
                        <span style="font-family:Consolas,monospace;font-size:12px;color:${pdotColor};">
                            ${periodChangeRate != null ? 
                                (periodChangeRate >= 0 ? '+' : '') + periodChangeRate.toExponential(2) + ' d/d' : '-'}
                        </span>
                    </div>
                    <div style="display:flex;justify-content:space-between;align-items:center;">
                        <span style="font-size:10px;color:#6080a0;">显著性</span>
                        <span style="font-family:Consolas,monospace;font-size:10px;
                            color:${pdotSignificant ? '#80ff80' : '#8090b0'};">
                            ${periodChangeSignificance != null ? 
                                periodChangeSignificance.toFixed(2) + ' σ' : '-'}
                            ${pdotSignificant ? '✓ 显著' : ' 不显著'}
                        </span>
                    </div>
                </div>

                <div style="padding-top:10px;border-top:1px solid rgba(80,120,200,0.2);">
                    <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:6px;">
                        <span style="font-size:11px;color:#8090b0;">长期亮度趋势</span>
                        <span style="font-family:Consolas,monospace;font-size:12px;color:${trendColor};">
                            ${brightnessTrend != null ?
                                (brightnessTrend >= 0 ? '+' : '') + brightnessTrend.toFixed(3) + ' mag/Myr' : '-'}
                        </span>
                    </div>
                    ${brightnessTrendError != null ? `
                        <div style="font-size:10px;color:#6080a0;text-align:right;
                            font-family:Consolas,monospace;">
                            ± ${brightnessTrendError.toFixed(3)} mag/Myr
                        </div>
                    ` : ''}
                    <div style="margin-top:8px;height:4px;background:rgba(40,60,100,0.4);
                        border-radius:2px;overflow:hidden;">
                        <div style="height:100%;width:50%;
                            background:linear-gradient(90deg,#ff6060,#ffcc60,#80ff80);
                            border-radius:2px;"></div>
                    </div>
                    <div style="display:flex;justify-content:space-between;margin-top:2px;
                        font-size:9px;color:#6080a0;font-family:Consolas,monospace;">
                        <span>变暗</span>
                        <span>变亮</span>
                    </div>
                </div>

                ${data.notes ? `
                    <div style="margin-top:12px;padding:8px 10px;border-radius:4px;
                        background:rgba(255,204,96,0.08);border-left:3px solid #ffcc60;
                        font-size:11px;line-height:1.5;color:#c8b890;">
                        <div style="color:#ffcc60;font-size:10px;margin-bottom:4px;">备注</div>
                        ${data.notes}
                    </div>
                ` : ''}
            </div>
        `;
    }
}

window.VariableStarAnalyzer = VariableStarAnalyzer;
