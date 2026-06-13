/* ============================================================
 * instrument.js - 古代仪器误差反演可视化模块
 * 职责:
 *   - 仪器列表渲染与选择
 *   - 仪器误差反演结果可视化
 *   - 误差分量雷达图绘制
 *   - 残差散点图绘制
 *   - 精度评估展示
 * ============================================================ */

const API_BASE = (() => {
    const p = window.location.protocol;
    const h = window.location.hostname;
    const port = window.location.port === '8080' ? '8080' : '8080';
    return `${p}//${h}:${port}/api`;
})();

class InstrumentAnalyzer {
    constructor() {
        this.currentInstrument = null;
        this.currentObservations = [];
        this.currentSolution = null;

        this.onInstrumentSelected = null;
        this.onInversionComplete = null;
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

    async fetchInstruments() {
        return this._json(`${API_BASE}/instruments`);
    }

    async fetchInstrument(id) {
        return this._json(`${API_BASE}/instruments/${id}`);
    }

    async fetchObservations(instrumentId) {
        return this._json(`${API_BASE}/instruments/${instrumentId}/observations`);
    }

    async invertErrors(instrumentId, refId = null, useGaia = true) {
        const body = {
            target_instrument_id: instrumentId,
            reference_instrument_id: refId,
            use_gaia_as_reference: useGaia,
        };
        const solution = await this._json(
            `${API_BASE}/instruments/${instrumentId}/invert`,
            'POST',
            body
        );
        this.currentSolution = solution;
        if (typeof this.onInversionComplete === 'function') {
            this.onInversionComplete(solution);
        }
        return solution;
    }

    _getAccuracyColor(quality) {
        switch (quality) {
            case 'excellent': return '#4ade80';
            case 'good': return '#2dd4bf';
            case 'moderate': return '#facc15';
            case 'poor': return '#f87171';
            default: return '#94a3b8';
        }
    }

    _getAccuracyLabel(quality) {
        switch (quality) {
            case 'excellent': return '优秀';
            case 'good': return '良好';
            case 'moderate': return '一般';
            case 'poor': return '较差';
            default: return '未知';
        }
    }

    _getDynastyColor(dynastyName) {
        const colors = {
            '汉': '#ff6b6b',
            '东汉': '#ffa94d',
            '唐': '#ffd43b',
            '宋': '#69db7c',
            '元': '#4dabf7',
            '明': '#9775fa',
            '清': '#f783ac',
        };
        return colors[dynastyName] || '#868e96';
    }

    _getNominalAccuracyColor(nominalAccuracy) {
        if (nominalAccuracy <= 3) return '#4ade80';
        if (nominalAccuracy <= 6) return '#2dd4bf';
        if (nominalAccuracy <= 12) return '#facc15';
        return '#f87171';
    }

    renderInstrumentList(container, instruments) {
        if (!container) return;

        if (!instruments || instruments.length === 0) {
            container.innerHTML = `
                <div style="padding:40px;text-align:center;color:#8090b0;">
                    暂无仪器数据
                </div>
            `;
            return;
        }

        container.innerHTML = `
            <div style="display:flex;flex-direction:column;gap:8px;">
                ${instruments.map(inst => this._renderInstrumentCard(inst)).join('')}
            </div>
        `;

        container.querySelectorAll('.instrument-card').forEach(card => {
            card.addEventListener('click', () => {
                const id = parseInt(card.dataset.instrumentId);
                this.fetchInstrument(id).then(inst => {
                    this.currentInstrument = inst;
                    if (typeof this.onInstrumentSelected === 'function') {
                        this.onInstrumentSelected(inst);
                    }
                }).catch(e => {
                    console.error('Failed to fetch instrument:', e);
                });
            });
        });
    }

    _renderInstrumentCard(inst) {
        const dynastyColor = this._getDynastyColor(inst.dynasty_name);
        const accuracyColor = this._getNominalAccuracyColor(inst.nominal_accuracy_arcmin);
        const yearStr = inst.erected_year < 0
            ? `前${-inst.erected_year | 0}年`
            : `公元${inst.erected_year | 0}年`;

        return `
            <div class="instrument-card"
                 data-instrument-id="${inst.id}"
                 style="padding:12px 16px;background:rgba(30,40,60,0.8);border:1px solid rgba(120,150,200,0.2);
                        border-radius:6px;cursor:pointer;transition:all 0.2s;"
                 onmouseover="this.style.borderColor='${dynastyColor}';this.style.boxShadow='0 0 12px ${dynastyColor}40'"
                 onmouseout="this.style.borderColor='rgba(120,150,200,0.2)';this.style.boxShadow='none'">
                <div style="display:flex;align-items:center;justify-content:space-between;">
                    <div style="display:flex;align-items:center;gap:10px;">
                        <span style="display:inline-block;width:8px;height:8px;border-radius:50%;background:${dynastyColor};"></span>
                        <span style="font-weight:bold;color:#e0e8ff;font-size:14px;">${inst.name_cn}</span>
                        <span style="font-size:11px;color:#8090b0;">${inst.instrument_code}</span>
                    </div>
                    <div style="display:flex;align-items:center;gap:8px;">
                        <span style="font-size:11px;padding:2px 8px;border-radius:10px;
                                     background:${dynastyColor}30;color:${dynastyColor};">
                            ${inst.dynasty_name || '-'}
                        </span>
                        <span style="font-size:11px;padding:2px 8px;border-radius:10px;
                                     background:rgba(120,150,200,0.15);color:#a0b8d0;">
                            ${inst.ring_count} 环
                        </span>
                    </div>
                </div>
                <div style="display:flex;align-items:center;justify-content:space-between;margin-top:8px;">
                    <span style="font-size:12px;color:#8090b0;">${yearStr}</span>
                    <div style="display:flex;align-items:center;gap:6px;">
                        <span style="font-size:11px;color:#8090b0;">标称精度:</span>
                        <div style="position:relative;width:80px;height:6px;background:rgba(120,150,200,0.2);border-radius:3px;overflow:hidden;">
                            <div style="position:absolute;left:0;top:0;bottom:0;width:${Math.min(100, (inst.nominal_accuracy_arcmin / 30) * 100)}%;
                                        background:${accuracyColor};border-radius:3px;"></div>
                        </div>
                        <span style="font-size:12px;color:${accuracyColor};font-weight:bold;">
                            ${inst.nominal_accuracy_arcmin?.toFixed(1)}'
                        </span>
                    </div>
                </div>
                ${inst.description ? `
                <div style="margin-top:6px;font-size:11px;color:#8090b0;line-height:1.4;">
                    ${inst.description}
                </div>
                ` : ''}
            </div>
        `;
    }

    renderInversionResult(container, solution) {
        if (!container) return;

        if (!solution) {
            container.innerHTML = `
                <div style="padding:40px;text-align:center;color:#8090b0;">
                    请先执行误差反演
                </div>
            `;
            return;
        }

        container.innerHTML = `
            <div style="display:flex;flex-direction:column;gap:16px;">
                <div style="display:flex;align-items:center;justify-content:space-between;">
                    <h3 style="margin:0;color:#e0e8ff;">${solution.instrument_name_cn}</h3>
                    <span style="font-size:11px;color:#8090b0;">${solution.instrument_code}</span>
                </div>

                <div class="info-grid" style="grid-template-columns:1fr 1fr;gap:8px 16px;">
                    <div class="info-item"><span class="key">参考仪器:</span><span class="value">${solution.ref_instrument_code}</span></div>
                    <div class="info-item"><span class="key">共用星数:</span><span class="value">${solution.num_shared_stars}</span></div>
                    <div class="info-item"><span class="key">迭代次数:</span><span class="value">${solution.num_iterations}</span></div>
                    <div class="info-item"><span class="key">是否收敛:</span>
                        <span class="value" style="color:${solution.converged ? '#4ade80' : '#f87171'}">
                            ${solution.converged ? '是' : '否'}
                        </span>
                    </div>
                </div>

                <div style="padding:12px;background:rgba(60,80,120,0.3);border-radius:6px;">
                    <h4 style="margin:0 0 10px 0;color:#a0c8ff;">极轴偏差</h4>
                    <div class="info-grid" style="grid-template-columns:1fr 1fr;">
                        <div class="info-item"><span class="key">极轴倾斜:</span>
                            <span class="value" style="color:#ffcc80;">
                                ${solution.polar_axis_tilt_arcmin?.toFixed(3)}'
                                <span style="font-size:11px;color:#8090b0;">±${solution.polar_axis_tilt_uncertainty_arcmin?.toFixed(3)}'</span>
                            </span>
                        </div>
                        <div class="info-item"><span class="key">极轴方位:</span>
                            <span class="value" style="color:#ffcc80;">
                                ${solution.polar_axis_azimuth_arcmin?.toFixed(3)}'
                                <span style="font-size:11px;color:#8090b0;">±${solution.polar_axis_azimuth_uncertainty_arcmin?.toFixed(3)}'</span>
                            </span>
                        </div>
                    </div>
                </div>

                <div style="padding:12px;background:rgba(60,80,120,0.3);border-radius:6px;">
                    <h4 style="margin:0 0 10px 0;color:#a0c8ff;">刻度系统误差</h4>
                    <div class="info-grid" style="grid-template-columns:1fr 1fr;">
                        <div class="info-item"><span class="key">一周期项:</span>
                            <span class="value" style="color:#ffcc80;">${solution.divisions_periodicity_1_arcmin?.toFixed(3)}'</span>
                        </div>
                        <div class="info-item"><span class="key">二周期项:</span>
                            <span class="value" style="color:#ffcc80;">${solution.divisions_periodicity_2_arcmin?.toFixed(3)}'</span>
                        </div>
                        <div class="info-item"><span class="key">系统校正:</span>
                            <span class="value" style="color:#ffcc80;">${solution.divisions_systematic_correction_arcmin_per_cycle?.toFixed(3)}'/周</span>
                        </div>
                    </div>
                </div>

                <div style="padding:12px;background:rgba(60,80,120,0.3);border-radius:6px;">
                    <h4 style="margin:0 0 10px 0;color:#a0c8ff;">零点偏移</h4>
                    <div class="info-grid" style="grid-template-columns:1fr 1fr;">
                        <div class="info-item"><span class="key">RA 零点:</span>
                            <span class="value" style="color:#ffcc80;">${solution.ra_zero_point_offset_arcmin?.toFixed(3)}'</span>
                        </div>
                        <div class="info-item"><span class="key">Dec 零点:</span>
                            <span class="value" style="color:#ffcc80;">${solution.dec_zero_point_offset_arcmin?.toFixed(3)}'</span>
                        </div>
                    </div>
                </div>

                <div style="padding:12px;background:rgba(60,80,120,0.3);border-radius:6px;">
                    <h4 style="margin:0 0 10px 0;color:#a0c8ff;">其他误差</h4>
                    <div class="info-grid" style="grid-template-columns:1fr 1fr;">
                        <div class="info-item"><span class="key">准直误差:</span>
                            <span class="value" style="color:#ffcc80;">${solution.collimation_error_arcmin?.toFixed(3)}'</span>
                        </div>
                        <div class="info-item"><span class="key">挠曲项:</span>
                            <span class="value" style="color:#ffcc80;">${solution.flexure_term_arcmin_per_90deg?.toFixed(3)}'/90°</span>
                        </div>
                        <div class="info-item"><span class="key">折射校正:</span>
                            <span class="value" style="color:#ffcc80;">${solution.refraction_correction_arcmin_per_airmass?.toFixed(3)}'/大气质量</span>
                        </div>
                    </div>
                </div>

                <div style="padding:12px;background:rgba(60,80,120,0.3);border-radius:6px;">
                    <h4 style="margin:0 0 10px 0;color:#a0c8ff;">残差统计</h4>
                    <div class="info-grid" style="grid-template-columns:1fr 1fr;">
                        <div class="info-item"><span class="key">RA 残差均值:</span>
                            <span class="value">${solution.residuals_ra_mean_arcmin?.toFixed(3)}'</span>
                        </div>
                        <div class="info-item"><span class="key">Dec 残差均值:</span>
                            <span class="value">${solution.residuals_dec_mean_arcmin?.toFixed(3)}'</span>
                        </div>
                        <div class="info-item"><span