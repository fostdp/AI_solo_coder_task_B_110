/* ============================================================
 * UI 控制模块 - 朝代时间轴、面板、筛选等交互
 * ============================================================ */

class UI {
    constructor(starField) {
        this.sf = starField;
        this.dynasties = [];
        this.mansions = [];
        this.currentDynasty = null;
        this.compareDynasty = null;
        this.compareMode = false;

        this._bindTopBar();
        this._bindFilters();
        this._bindPanels();
    }

    setDynasties(list) {
        this.dynasties = list;
        this._renderTimeline();
    }

    setMansions(list) {
        this.mansions = list;
        const sel = document.getElementById('constellation-select');
        if (sel) {
            list.forEach(m => {
                const opt = document.createElement('option');
                opt.value = m.name_cn + '宿';
                opt.textContent = m.name_cn + '宿 (' + m.name_pinyin + ')';
                sel.appendChild(opt);
            });
        }
    }

    _bindTopBar() {
        document.querySelectorAll('.view-toggle button').forEach(btn => {
            btn.addEventListener('click', () => {
                document.querySelectorAll('.view-toggle button').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');
                const mode = btn.dataset.view;
                this.sf.setViewMode(mode);
                this._enableCompareMode(mode === 'compare');
            });
        });

        const displaySel = document.getElementById('display-select');
        if (displaySel) displaySel.addEventListener('change', () => {
            this.sf.setDisplayFilter(displaySel.value);
        });

        const styleSel = document.getElementById('style-select');
        if (styleSel) styleSel.addEventListener('change', () => {
            this.sf.setStyleMode(styleSel.value);
        });

        const magSlider = document.getElementById('mag-threshold');
        if (magSlider) magSlider.addEventListener('input', () => {
            this.sf.setMagThreshold(parseFloat(magSlider.value));
        });
    }

    _bindFilters() {
        const updateList = () => {
            const params = {};
            if (this.currentDynasty?.id != null) params.dynasty_id = this.currentDynasty.id;
            const book = document.getElementById('source-book-select')?.value;
            const cons = document.getElementById('constellation-select')?.value;
            const q = parseInt(document.getElementById('quality-select')?.value || '0');
            const search = document.getElementById('star-search')?.value?.trim();
            if (book) params.source_book = book;
            if (cons) params.constellation = cons;
            if (q > 0) params.quality_min = q;
            if (search) params.star_name = '%' + search + '%';
            params.limit = 100;
            this._refreshStarList(params);
        };

        ['source-book-select', 'constellation-select', 'quality-select'].forEach(id => {
            const el = document.getElementById(id);
            if (el) el.addEventListener('change', updateList);
        });
        const searchEl = document.getElementById('star-search');
        if (searchEl) {
            let t;
            searchEl.addEventListener('input', () => {
                clearTimeout(t);
                t = setTimeout(updateList, 300);
            });
        }
    }

    async _refreshStarList(params) {
        const listEl = document.getElementById('stars-list');
        if (!listEl) return;
        listEl.innerHTML = '<div style="text-align:center;color:#8090b0;padding:20px;">加载中...</div>';
        try {
            const data = await window.api.getStars(params);
            if (!data || !Array.isArray(data) || data.length === 0) {
                listEl.innerHTML = '<div style="text-align:center;color:#8090b0;padding:20px;">暂无数据</div>';
                return;
            }
            listEl.innerHTML = '';
            data.slice(0, 80).forEach(s => {
                const item = document.createElement('div');
                item.className = 'star-item';
                item.dataset.id = s.id;
                item.innerHTML = `
                    <span class="star-name">${s.star_name_cn}</span>
                    <span class="star-mag">${s.magnitude_num != null ? 'm' + s.magnitude_num.toFixed(1) : ''} · ${s.dynasty_name || ''}</span>
                `;
                item.addEventListener('click', () => {
                    document.querySelectorAll('.star-item').forEach(i => i.classList.remove('active'));
                    item.classList.add('active');
                    const ra = s.ra_j2000 ?? s.ra_ancient_conv;
                    const dec = s.dec_j2000 ?? s.dec_ancient_conv;
                    if (ra != null && dec != null) this.sf.flyTo(ra, dec, 2.5);
                    this._showStarDetail(s);
                });
                listEl.appendChild(item);
            });
            if (data.length > 80) {
                const more = document.createElement('div');
                more.style.cssText = 'text-align:center;color:#8090b0;padding:8px;font-size:11px;';
                more.textContent = `共 ${data.length} 条，仅显示前 80 条`;
                listEl.appendChild(more);
            }
            const cntEl = document.getElementById('star-count');
            if (cntEl) cntEl.textContent = `(${data.length})`;
        } catch (e) {
            listEl.innerHTML = `<div style="text-align:center;color:#c08080;padding:20px;">加载失败: ${e.message}</div>`;
        }
    }

    _renderTimeline() {
        const container = document.getElementById('dynasty-timeline');
        if (!container) return;
        container.innerHTML = '';
        const barWrap = document.createElement('div');
        barWrap.className = 'dynasty-bar-container';
        container.appendChild(barWrap);

        this.dynasties.forEach(d => {
            const bar = document.createElement('div');
            const cls = Astro.DYNASTY_STYLES[d.name_cn] || 'other';
            bar.className = `dynasty-bar ${cls}`;
            bar.dataset.id = d.id;
            const sYr = d.start_year < 0 ? `前${-d.start_year}` : d.start_year;
            const eYr = d.end_year < 0 ? `前${-d.end_year}` : d.end_year;
            bar.innerHTML = `
                ${d.name_cn}
                <div class="year-range">${sYr}~${eYr}</div>
            `;
            bar.addEventListener('click', () => this._onDynastyClick(d, bar));
            bar.addEventListener('dblclick', () => this._toggleCompareDynasty(d, bar));
            barWrap.appendChild(bar);
        });

        const song = this.dynasties.find(d => d.name_cn === '宋');
        if (song) {
            const songBar = barWrap.querySelector(`[data-id="${song.id}"]`);
            this._onDynastyClick(song, songBar);
        }
    }

    _onDynastyClick(dynasty, barEl) {
        this.currentDynasty = dynasty;
        document.querySelectorAll('.dynasty-bar').forEach(b => {
            b.classList.remove('active');
            if (!this.compareMode) b.classList.remove('comparing');
        });
        if (barEl) barEl.classList.add('active');
        this._updateTimelineInfo();
        if (window.onDynastyChange) window.onDynastyChange(dynasty);
        this._refreshStarList({ dynasty_id: dynasty.id, limit: 100 });
    }

    _toggleCompareDynasty(dynasty, barEl) {
        if (this.compareDynasty?.id === dynasty.id) {
            this.compareDynasty = null;
            if (barEl) barEl.classList.remove('comparing');
        } else {
            if (this.compareDynasty) {
                const old = document.querySelector(`.dynasty-bar[data-id="${this.compareDynasty.id}"]`);
                if (old) old.classList.remove('comparing');
            }
            this.compareDynasty = dynasty;
            if (barEl) barEl.classList.add('comparing');
        }
        this._updateTimelineInfo();
        if (window.onCompareChange) window.onCompareChange(this.compareDynasty);
    }

    _enableCompareMode(enabled) {
        this.compareMode = enabled;
        const overlay = document.querySelector('.compare-overlay');
        if (overlay) overlay.classList.toggle('active', enabled);
        const btn = document.getElementById('compare-btn');
        if (btn) {
            btn.classList.toggle('active', enabled);
            btn.textContent = enabled ? '退出对比' : '对比模式';
        }
    }

    _updateTimelineInfo() {
        const info = document.getElementById('timeline-info');
        if (!info) return;
        let txt = `当前：${this.currentDynasty?.name_cn || '-'} `;
        if (this.currentDynasty) {
            const yr = this.currentDynasty.canonical_epoch;
            txt += `· 公元 ${yr < 0 ? '前' + (-yr | 0) : (yr | 0)} 年`;
        }
        if (this.compareDynasty) txt += `  |  对比：${this.compareDynasty.name_cn}`;
        info.textContent = txt;
    }

    _bindPanels() {
        const btn = document.getElementById('compare-btn');
        if (btn) btn.addEventListener('click', () => {
            this._enableCompareMode(!this.compareMode);
            const viewCompare = document.querySelector('[data-view="compare"]');
            if (viewCompare) {
                document.querySelectorAll('.view-toggle button').forEach(b => b.classList.remove('active'));
                if (this.compareMode) viewCompare.classList.add('active');
                else document.querySelector('[data-view="sphere"]').classList.add('active');
            }
            this.sf.setViewMode(this.compareMode ? 'compare' : 'sphere');
        });
    }

    async _showStarDetail(star) {
        const panel = document.getElementById('detail-panel');
        const content = document.getElementById('detail-content');
        if (!panel || !content) return;
        panel.style.display = 'flex';
        document.getElementById('dynasty-tag').textContent = star.dynasty_name || '-';
        document.getElementById('detail-title').textContent = star.star_name_cn || '恒星详情';
        content.innerHTML = `<div style="text-align:center;color:#8090b0;padding:20px;">加载详情中...</div>`;

        try {
            let crossData = [];
            try { crossData = await window.api.getCrossDynasty(star.id) || []; } catch(_) {}

            let convData = null;
            if (star.ruxiu_du != null && star.quji_du != null && star.mansion_order) {
                try {
                    convData = await window.api.convertRuxiuToJ2000({
                        ruxiu_du: star.ruxiu_du,
                        quji_du: star.quji_du,
                        mansion_order: star.mansion_order,
                        epoch_yr: this.currentDynasty?.canonical_epoch || 1000,
                        pm_ra_mas: star.proper_motion_ra,
                        pm_dec_mas: star.proper_motion_dec,
                    });
                } catch(_) {}
            }

            let trajData = null;
            if (star.ra_j2000 != null && star.proper_motion_ra != null) {
                try {
                    trajData = await window.api.getTrajectory({
                        ra_j2000: star.ra_j2000,
                        dec_j2000: star.dec_j2000,
                        pm_ra_mas: star.proper_motion_ra,
                        pm_dec_mas: star.proper_motion_dec,
                        year_start: -200, year_end: 2500, n_points: 50,
                    });
                } catch(_) {}
            }

            // ★ 修复 3: 前端用 Planck 色温显示色温信息
            const col = Astro.starToColor(star, 'planck');

            content.innerHTML = this._renderDetailHTML(star, convData, trajData, crossData, col);
        } catch (e) {
            content.innerHTML = `<div style="color:#c08080;padding:20px;">加载失败: ${e.message}</div>`;
        }
    }

    _renderDetailHTML(star, conv, traj, cross, colorInfo) {
        const modernRa = star.ra_j2000 != null ? star.ra_j2000.toFixed(4) + '°' : '-';
        const modernDec = star.dec_j2000 != null ? star.dec_j2000.toFixed(4) + '°' : '-';
        const raH = star.ra_j2000 != null ? (star.ra_j2000 / 15).toFixed(3) + 'h' : '-';
        const deltaRa = (conv && star.ra_j2000 != null)
            ? (conv.with_proper_motion[0] - star.ra_j2000).toFixed(3) : null;
        const deltaDec = (conv && star.dec_j2000 != null)
            ? (conv.with_proper_motion[1] - star.dec_j2000).toFixed(3) : null;

        let crossHTML = '';
        if (cross && cross.length > 0) {
            crossHTML = `
                <div class="detail-section">
                    <h3>跨朝代坐标对比</h3>
                    <div style="max-height:150px;overflow-y:auto;">
                        ${cross.slice(0, 5).map(c => `
                            <div style="padding:8px;background:rgba(30,40,70,0.4);border-radius:4px;margin-bottom:6px;">
                                <div style="font-size:12px;color:#a0c8ff;margin-bottom:4px;">
                                    ${c.dynasty_1.name} → ${c.dynasty_2.name}
                                </div>
                                <div class="coord-compare">
                                    <span>Δ入宿度:</span>
                                    <span class="delta ${c.delta_ruxiu >= 0 ? 'pos' : 'neg'}">
                                        ${c.delta_ruxiu >= 0 ? '+' : ''}${c.delta_ruxiu.toFixed(3)}°
                                    </span>
                                </div>
                                <div class="coord-compare">
                                    <span>Δ去极度:</span>
                                    <span class="delta ${c.delta_quji >= 0 ? 'pos' : 'neg'}">
                                        ${c.delta_quji >= 0 ? '+' : ''}${c.delta_quji.toFixed(3)}°
                                    </span>
                                </div>
                            </div>
                        `).join('')}
                    </div>
                </div>
            `;
        }

        const pmRa = star.proper_motion_ra != null ? star.proper_motion_ra.toFixed(2) : '-';
        const pmDec = star.proper_motion_dec != null ? star.proper_motion_dec.toFixed(2) : '-';
        const pmMag = (star.proper_motion_ra != null && star.proper_motion_dec != null)
            ? Math.sqrt(star.proper_motion_ra ** 2 + star.proper_motion_dec ** 2).toFixed(2)
            : '-';
        const pmAngleDeg = (star.proper_motion_ra != null && star.proper_motion_dec != null)
            ? Math.atan2(star.proper_motion_dec, star.proper_motion_ra) * 180 / Math.PI : 0;

        // 颜色温度显示
        const tempStr = colorInfo && colorInfo.temp ? `${Math.round(colorInfo.temp)} K` : '-';
        const hexStr = colorInfo && colorInfo.hex ? colorInfo.hex : '#fff';

        return `
            <div class="detail-section">
                <h3>基本信息</h3>
                <div class="info-grid">
                    <div class="info-item"><span class="key">星名：</span><span class="value">${star.star_name_cn}</span></div>
                    <div class="info-item"><span class="key">星官：</span><span class="value">${star.constellation || '-'}</span></div>
                    <div class="info-item"><span class="key">朝代：</span><span class="value">${star.dynasty_name || '-'}</span></div>
                    <div class="info-item"><span class="key">典籍：</span><span class="value">${star.source_book || '-'}</span></div>
                    <div class="info-item"><span class="key">古代星等：</span><span class="value">${star.magnitude_ancient || '-'}</span></div>
                    <div class="info-item"><span class="key">目视星等：</span><span class="value">${star.magnitude_num != null ? star.magnitude_num.toFixed(2) : '-'}</span></div>
                    <div class="info-item"><span class="key">古代描述：</span><span class="value">${star.color_desc || '-'}</span></div>
                    <div class="info-item">
                        <span class="key">现代色温：</span>
                        <span class="value" style="display:flex;align-items:center;gap:6px;">
                            <span style="width:12px;height:12px;border-radius:50%;background:${hexStr};box-shadow:0 0 4px ${hexStr};"></span>
                            ${tempStr}
                        </span>
                    </div>
                </div>
            </div>

            <div class="detail-section">
                <h3>古代坐标（入宿度 / 去极度）</h3>
                <div class="coord-grid">
                    <div class="coord-card ancient">
                        <h4>入宿度</h4>
                        <div class="value">${star.ruxiu_du?.toFixed?.(3) || '-'}°</div>
                        <div class="label">${conv?.ruxiu_raw_cn || ''}</div>
                    </div>
                    <div class="coord-card ancient">
                        <h4>去极度</h4>
                        <div class="value">${star.quji_du?.toFixed?.(3) || '-'}°</div>
                        <div class="label">${conv?.quji_raw_cn || ''}</div>
                    </div>
                    <div class="coord-card ancient">
                        <h4>古代赤经</h4>
                        <div class="value">${conv?.ancient_ra?.toFixed?.(4) || '-'}°</div>
                        <div class="label">历元 ${this.currentDynasty?.canonical_epoch || '-'}</div>
                    </div>
                    <div class="coord-card ancient">
                        <h4>古代赤纬</h4>
                        <div class="value">${conv?.ancient_dec?.toFixed?.(4) || '-'}°</div>
                        <div class="label">δ = 90° - 去极度</div>
                    </div>
                </div>
            </div>

            <div class="detail-section">
                <h3>现代坐标 (J2000.0) 与偏差</h3>
                <div class="coord-grid">
                    <div class="coord-card modern">
                        <h4>证认 RA</h4>
                        <div class="value">${modernRa}</div>
                        <div class="label">${raH}</div>
                    </div>
                    <div class="coord-card modern">
                        <h4>证认 Dec</h4>
                        <div class="value">${modernDec}</div>
                        <div class="label">J2000.0</div>
                    </div>
                    <div class="coord-card modern">
                        <h4>ΔRA (转换-证认)</h4>
                        <div class="value" style="color:${deltaRa && Math.abs(deltaRa) < 1 ? '#80c080' : '#c08080'}">
                            ${deltaRa != null ? (deltaRa >= 0 ? '+' : '') + deltaRa + '°' : '-'}
                        </div>
                        <div class="label">角秒: ${deltaRa != null ? (Math.abs(deltaRa) * 3600).toFixed(1) + '"' : '-'}</div>
                    </div>
                    <div class="coord-card modern">
                        <h4>ΔDec (转换-证认)</h4>
                        <div class="value" style="color:${deltaDec && Math.abs(deltaDec) < 1 ? '#80c080' : '#c08080'}">
                            ${deltaDec != null ? (deltaDec >= 0 ? '+' : '') + deltaDec + '°' : '-'}
                        </div>
                        <div class="label">角秒: ${deltaDec != null ? (Math.abs(deltaDec) * 3600).toFixed(1) + '"' : '-'}</div>
                    </div>
                </div>
            </div>

            <div class="detail-section">
                <h3>岁差 / 章动 修正量 (IAU 2006)</h3>
                <div class="info-grid">
                    <div class="info-item"><span class="key">行星摄动χ:</span><span class="value">
                        ${conv?.planetary_correction_arcsec != null ? conv.planetary_correction_arcsec.toFixed(4) + '"' : '-'}
                    </span></div>
                    <div class="info-item"><span class="key">章动Δψ:</span><span class="value">
                        ${conv?.nutation_correction?.[0] != null ? conv.nutation_correction[0].toFixed(3) + '"' : '-'}
                    </span></div>
                    <div class="info-item"><span class="key">章动Δε:</span><span class="value">
                        ${conv?.nutation_correction?.[1] != null ? conv.nutation_correction[1].toFixed(3) + '"' : '-'}
                    </span></div>
                    <div class="info-item"><span class="key">总精度:</span><span class="value">
                        <span style="color:#80ff80;">★ IAU 2006</span>
                    </span></div>
                </div>
            </div>

            <div class="detail-section">
                <h3>自行 Proper Motion</h3>
                <div class="proper-motion-section">
                    <div class="pm-arrow-visual">
                        <div class="pm-circle">
                            <div class="pm-dot"></div>
                            <div class="pm-arrow" style="transform: rotate(${pmAngleDeg}deg); width:${Math.min(30, (parseFloat(pmMag) || 0) / 10)}px;"></div>
                        </div>
                    </div>
                    <div class="pm-info">
                        μα = ${pmRa} mas/yr &nbsp;|&nbsp; μδ = ${pmDec} mas/yr &nbsp;|&nbsp; |μ| = ${pmMag} mas/yr
                    </div>
                    <div class="info-grid" style="margin-top:10px;">
                        <div class="info-item"><span class="key">100年后位移:</span><span class="value">
                            ${pmMag !== '-' ? (parseFloat(pmMag) * 100 / 1000).toFixed(3) + '"' : '-'}
                        </span></div>
                        <div class="info-item"><span class="key">1000年后位移:</span><span class="value">
                            ${pmMag !== '-' ? (parseFloat(pmMag) * 1000 / 3600).toFixed(3) + '°' : '-'}
                        </span></div>
                        <div class="info-item"><span class="key">视差:</span><span class="value">
                            ${star.parallax != null ? star.parallax.toFixed(3) + ' mas' : '-'}
                        </span></div>
                        <div class="info-item"><span class="key">距离:</span><span class="value">
                            ${star.parallax != null && star.parallax > 0 ? (1000 / star.parallax).toFixed(1) + ' pc' : '-'}
                        </span></div>
                    </div>
                </div>
            </div>

            ${crossHTML}
        `;
    }
}

window.toggleSidePanel = () => {
    const p = document.getElementById('side-panel');
    const b = document.getElementById('fab-side-toggle');
    if (!p) return;
    const hidden = p.style.display === 'none';
    p.style.display = hidden ? 'flex' : 'none';
    if (b) b.classList.toggle('show', !hidden);
};

window.closeDetailPanel = () => {
    const p = document.getElementById('detail-panel');
    if (p) p.style.display = 'none';
    if (window.starField) window.starField._deselectStar();
};

window.closeMatchPanel = () => {
    const p = document.getElementById('match-panel');
    if (p) p.style.display = 'none';
};

window.UI = UI;
