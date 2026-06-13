/* ============================================================
 * transient_panel.js - 客星/超新星匹配面板模块
 * 职责:
 *   - 客星详情与匹配结果渲染
 *   - 贝叶斯概率分布柱状图
 *   - 匹配算法元信息展示
 *   - 与 StarChart3D 联动 (点击候选飞至目标)
 * ============================================================ */

class TransientPanel {
    constructor(starChart) {
        this.sf = starChart;
        this.currentGuest = null;
        this.currentMatches = [];
        this.currentMethod = null;

        // 绑定面板关闭按钮
        this._bindPanelControls();
    }

    _bindPanelControls() {
        const closeBtn = document.getElementById('match-panel-close');
        if (closeBtn) {
            closeBtn.addEventListener('click', () => this.hide());
        }
    }

    setGuest(guest, matches, method) {
        this.currentGuest = guest;
        this.currentMatches = matches || [];
        this.currentMethod = method || null;
        this._render();
        this.show();
    }

    show() {
        const panel = document.getElementById('match-panel');
        if (panel) panel.style.display = 'flex';
    }

    hide() {
        const panel = document.getElementById('match-panel');
        if (panel) panel.style.display = 'none';
    }

    _render() {
        if (!this.currentGuest) return;

        const g = this.currentGuest;
        const panel = document.getElementById('match-panel');
        if (!panel) return;

        const guestHTML = this._renderGuestHeader(g);
        const matchesHTML = this._renderMatchCandidates(this.currentMatches);
        const methodHTML = this.currentMethod
            ? this._renderMethodInfo(this.currentMethod)
            : '';

        panel.innerHTML = `
            <div class="match-panel-inner">
                <div class="match-panel-header">
                    <div style="display:flex;align-items:center;justify-content:space-between;">
                        <h2 style="margin:0;color:#ff8080;">客星匹配结果</h2>
                        <button id="match-panel-close" class="icon-btn" title="关闭">×</button>
                    </div>
                </div>
                <div class="match-panel-content">
                    ${guestHTML}
                    ${methodHTML}
                    ${matchesHTML}
                </div>
            </div>
        `;

        this._bindMatchPanelEvents();
    }

    _renderGuestHeader(g) {
        const sYr = g.year_ce < 0 ? `前${-g.year_ce | 0}` : (g.year_ce | 0);
        const eYr = g.year_end_ce ? (g.year_end_ce < 0 ? `前${-g.year_end_ce | 0}` : (g.year_end_ce | 0)) : null;
        const yrStr = eYr ? `${sYr} - ${eYr}` : `公元 ${sYr}`;

        return `
            <div class="guest-detail-section">
                <h3 style="margin:0 0 12px 0;">${g.guest_id_code}</h3>
                <div class="info-grid">
                    <div class="info-item"><span class="key">朝代:</span><span class="value">${g.dynasty_name || '-'}</span></div>
                    <div class="info-item"><span class="key">年份:</span><span class="value">${yrStr}</span></div>
                    <div class="info-item"><span class="key">峰值星等:</span><span class="value">${g.peak_mag?.toFixed(2) || '-'}</span></div>
                    <div class="info-item"><span class="key">可见期:</span><span class="value">${g.visibility_days || '-'} 天</span></div>
                    <div class="info-item"><span class="key">位置:</span><span class="value">
                        RA ${g.ra_deg?.toFixed(2) || '-'}°, Dec ${g.dec_deg?.toFixed(2) || '-'}°
                    </span></div>
                    <div class="info-item"><span class="key">记载:</span><span class="value">${g.historical_record || '-'}</span></div>
                </div>
                <div style="margin-top:12px;padding:8px 12px;background:rgba(255,120,120,0.1);
                    border-left:3px solid #ff8080;border-radius:4px;font-size:12px;line-height:1.5;">
                    ${g.description || '无详细描述'}
                </div>
            </div>
        `;
    }

    _renderMethodInfo(method) {
        return `
            <div class="method-section" style="margin-top:16px;">
                <h4 style="margin:0 0 8px 0;color:#a0c8ff;">匹配算法</h4>
                <div class="info-grid" style="grid-template-columns:1fr 1fr;">
                    <div class="info-item"><span class="key">模型:</span><span class="value">${method.name}</span></div>
                    <div class="info-item"><span class="key">版本:</span><span class="value">${method.version}</span></div>
                    <div class="info-item"><span class="key">先验:</span><span class="value" style="font-size:11px;">${method.prior_model}</span></div>
                    <div class="info-item"><span class="key">评估候选:</span><span class="value">${method.n_candidates_evaluated}</span></div>
                    <div class="info-item"><span class="key">返回结果:</span><span class="value">${method.n_candidates_returned}</span></div>
                    <div class="info-item"><span class="key">log(K) Top1/2:</span>
                        <span class="value" style="color:${method.log_bayes_factor_top > Math.log(150) ? '#80ff80' :
                            method.log_bayes_factor_top > Math.log(10) ? '#ffcc80' : '#ff8080'}">
                            ${method.log_bayes_factor_top.toFixed(2)}
                        </span>
                    </div>
                </div>
            </div>
        `;
    }

    _renderMatchCandidates(matches) {
        if (!matches || matches.length === 0) {
            return `
                <div class="candidates-section">
                    <h4 style="margin:0 0 8px 0;color:#c070ff;">候选超新星遗迹</h4>
                    <div style="padding:20px;text-align:center;color:#8090b0;">
                        未找到匹配的超新星遗迹
                    </div>
                </div>
            `;
        }

        const maxProb = Math.max(...matches.map(m => m.match_probability));

        return `
            <div class="candidates-section" style="margin-top:16px;">
                <h4 style="margin:0 0 8px 0;color:#c070ff;">候选超新星遗迹 (${matches.length})</h4>
                <div style="max-height:320px;overflow-y:auto;display:flex;flex-direction:column;gap:8px;">
                    ${matches.map((m, i) => this._renderCandidateRow(m, i, maxProb)).join('')}
                </div>
            </div>
        `;
    }

    _renderCandidateRow(m, idx, maxProb) {
        const prob = (m.match_probability * 100).toFixed(1);
        const isTop = idx === 0;
        const snrName = m.remnant_name || `G ${m.gal_l?.toFixed(2)}+${m.gal_b?.toFixed(2)}`;

        const barWidth = (m.match_probability / maxProb) * 100;
        const barColor = isTop ? '#80ff80' :
            m.match_probability > 0.1 ? '#ffcc80' : '#c08080';

        const bfLabel = idx === 0 && m.bayes_factor > 1
            ? `K = ${m.bayes_factor.toExponential(2)}`
            : '';

        return `
            <div class="candidate-row ${isTop ? 'top-candidate' : ''}"
                 data-snr-id="${m.remnant_id}"
                 data-ra="${m.ra_deg}" data-dec="${m.dec_deg}"
                 style="cursor:pointer;">
                <div style="display:flex;align-items:center;gap:8px;">
                    <span class="candidate-rank">${idx + 1}</span>
                    <div style="flex:1;min-width:0;">
                        <div style="display:flex;align-items:center;gap:6px;">
                            <span class="candidate-name" title="${snrName}">${snrName}</span>
                            <span class="candidate-type">${m.remnant_type || '?'}</span>
                            ${bfLabel ? `<span class="bayes-factor">${bfLabel}</span>` : ''}
                        </div>
                        <div class="prob-bar-container">
                            <div class="prob-bar" style="width:${barWidth}%;background:${barColor};">
                            </div>
                            <span class="prob-text">${prob}%</span>
                        </div>
                    </div>
                </div>
                <div class="candidate-details">
                    <span>${m.ra_deg?.toFixed(2)}°, ${m.dec_deg?.toFixed(2)}°</span>
                    <span>Δ ${m.angular_sep_arcmin?.toFixed(1)}'</span>
                    <span>Δt ${m.time_delta_yr > 0 ? '+' : ''}${(m.time_delta_yr | 0)}年</span>
                </div>
            </div>
        `;
    }

    _bindMatchPanelEvents() {
        const closeBtn = document.getElementById('match-panel-close');
        if (closeBtn) {
            closeBtn.addEventListener('click', () => this.hide());
        }

        document.querySelectorAll('.candidate-row').forEach(row => {
            row.addEventListener('click', () => {
                const ra = parseFloat(row.dataset.ra);
                const dec = parseFloat(row.dataset.dec);
                if (!isNaN(ra) && !isNaN(dec) && this.sf) {
                    this.sf.flyTo(ra, dec, 2.5);
                }
            });
        });
    }

    async loadAndRenderMatches(guestId) {
        try {
            const result = await window.api.runMatch(guestId);
            if (result && result.guest) {
                this.setGuest(result.guest, result.candidates, result.method);
            }
        } catch (e) {
            console.error('Failed to load matches:', e);
            const panel = document.getElementById('match-panel');
            if (panel) {
                panel.innerHTML = `
                    <div style="padding:40px;text-align:center;color:#c08080;">
                        加载匹配结果失败: ${e.message}
                    </div>
                `;
            }
            this.show();
        }
    }
}

window.TransientPanel = TransientPanel;
