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
        this.currentFeature = null;

        this._bindTopBar();
        this._bindFilters();
        this._bindPanels();
        this._initFeatureTabs();
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

    _initFeatureTabs() {
        this.renderFeatureTabs();
    }

    renderFeatureTabs() {
        const sidePanel = document.getElementById('side-panel');
        if (!sidePanel) return;

        const tabsHTML = `
            <section class="panel-section feature-tabs-section">
                <div class="feature-tabs">
                    <button class="feature-tab active" data-feature="stars">恒星视图</button>
                    <button class="feature-tab" data-feature="eclipses">日食月食</button>
                    <button class="feature-tab" data-feature="instruments">古代仪器</button>
                    <button class="feature-tab" data-feature="variables">变星分析</button>
                    <button class="feature-tab" data-feature="starmap">星图生成</button>
                </div>
            </section>
        `;

        const firstSection = sidePanel.querySelector('.panel-section');
        if (firstSection) {
            const tempDiv = document.createElement('div');
            tempDiv.innerHTML = tabsHTML.trim();
            const tabSection = tempDiv.firstChild;
            sidePanel.insertBefore(tabSection, firstSection);
        }

        sidePanel.querySelectorAll('.feature-tab').forEach(tab => {
            tab.addEventListener('click', () => {
                const feature = tab.dataset.feature;
                this._onFeatureTabClick(feature, tab);
            });
        });
    }

    _onFeatureTabClick(featureName, tabEl) {
        document.querySelectorAll('.feature-tab').forEach(t => t.classList.remove('active'));
        if (tabEl) tabEl.classList.add('active');
        this.currentFeature = featureName;
        this.toggleFeaturePanel(featureName);
        if (window.onFeatureTabChange) window.onFeatureTabChange(featureName);
    }

    toggleFeaturePanel(featureName) {
        const sidePanel = document.getElementById('side-panel');
        if (!sidePanel) return;

        const starSections = sidePanel.querySelectorAll('.panel-section:not(.feature-tabs-section)');
        const eclipsePanel = document.getElementById('eclipse-panel');
        const instrumentPanel = document.getElementById('instrument-panel');
        const variablePanel = document.getElementById('variable-panel');
        const starmapPanel = document.getElementById('starmap-panel');

        if (!featureName || featureName === 'stars') {
            starSections.forEach(s => s.style.display = '');
            if (eclipsePanel) eclipsePanel.style.display = 'none';
            if (instrumentPanel) instrumentPanel.style.display = 'none';
            if (variablePanel) variablePanel.style.display = 'none';
            if (starmapPanel) starmapPanel.style.display = 'none';
            return;
        }

        starSections.forEach(s => s.style.display = 'none');

        if (eclipsePanel) eclipsePanel.style.display = featureName === 'eclipses' ? 'block' : 'none';
        if (instrumentPanel) instrumentPanel.style.display = featureName === 'instruments' ? 'block' : 'none';
        if (variablePanel) variablePanel.style.display = featureName === 'variables' ? 'block' : 'none';
        if (starmapPanel) starmapPanel.style.display = featureName === 'starmap' ? 'block' : 'none';

        this._renderFeatureContent(featureName);
    }

    _renderFeatureContent(featureName) {
        if (featureName === 'eclipses') {
            this._renderEclipsePanel();
        } else if (featureName === 'instruments') {
            this._renderInstrumentPanel();
        } else if (featureName === 'variables') {
            this._renderVariablePanel();
        } else if (featureName === 'starmap') {
            this._renderStarmapPanel();
        }
    }

    _renderEclipsePanel() {
        let panel = document.getElementById('eclipse-panel');
        if (!panel) {
            panel = document.createElement('div');
            panel.id = 'eclipse-panel';
            panel.className = 'feature-panel';
            panel.style.display = 'none';
            const sidePanel = document.getElementById('side-panel');
            if (sidePanel) sidePanel.appendChild(panel);
        }
        panel.innerHTML = `
            <section class="panel-section">
                <h3>日食月食记录</h3>
                <div id="eclipse-list" class="feature-list"></div>
            </section>
        `;
        const listEl = document.getElementById('eclipse-list');
        if (listEl && window.eclipseView?.renderEclipseList) {
            window.eclipseView.renderEclipseList(listEl, window.app?.eclipseRecords || []);
        } else if (listEl) {
            this._renderSimpleEclipseList(listEl);
        }
    }

    _renderSimpleEclipseList(container) {
        if (!container || !window.app?.eclipseRecords) return;
        const records = window.app.eclipseRecords;
        if (!records || records.length === 0) {
            container.innerHTML = '<div style="text-align:center;color:#8090b0;padding:20px;">暂无数据</div>';
            return;
        }
        container.innerHTML = '';
        records.slice(0, 50).forEach(r => {
            const item = document.createElement('div');
            item.className = 'star-item';
            item.dataset.id = r.id;
            const typeLabel = r.eclipse_type === 'solar' ? '日食' : (r.eclipse_type === 'lunar' ? '月食' : '');
            const yearStr = r.year < 0 ? `前${-r.year}年` : `公元${r.year}年`;
            item.innerHTML = `
                <span class="star-name">${r.title || r.name_cn || '日食记录'}</span>
                <span class="star-mag">${typeLabel} · ${yearStr}</span>
            `;
            item.addEventListener('click', () => {
                document.querySelectorAll('#eclipse-list .star-item').forEach(i => i.classList.remove('active'));
                item.classList.add('active');
                if (window.eclipseView) {
                    window.eclipseView.fetchEclipseDetail(r.id).then(detail => {
                        if (typeof window.eclipseView.onEclipseSelected === 'function') {
                            window.eclipseView.onEclipseSelected(detail);
                        }
                    }).catch(e => console.warn('加载日食详情失败:', e));
                }
            });
            container.appendChild(item);
        });
    }

    _renderInstrumentPanel() {
        let panel = document.getElementById('instrument-panel');
        if (!panel) {
            panel = document.createElement('div');
            panel.id = 'instrument-panel';
            panel.className = 'feature-panel';
            panel.style.display = 'none';
            const sidePanel = document.getElementById('side-panel');
            if (sidePanel) sidePanel.appendChild(panel);
        }
        panel.innerHTML = `
            <section class="panel-section">
                <h3>古代天文仪器</h3>
                <div id="instrument-list" class="feature-list"></div>
            </section>
        `;
        const listEl = document.getElementById('instrument-list');
        if (listEl && window.instrumentAnalyzer?.renderInstrumentList) {
            window.instrumentAnalyzer.renderInstrumentList(listEl, window.app?.instruments || []);
        } else if (listEl) {
            this._renderSimpleInstrumentList(listEl);
        }
    }

    _renderSimpleInstrumentList(container) {
        if (!container || !window.app?.instruments) return;
        const instruments = window.app.instruments;
        if (!instruments || instruments.length === 0) {
            container.innerHTML = '<div style="text-align:center;color:#8090b0;padding:20px;">暂无数据</div>';
            return;
        }
        container.innerHTML = '';
        instruments.forEach(inst => {
            const item = document.createElement('div');
            item.className = 'star-item';
            item.dataset.id = inst.id;
            item.innerHTML = `
                <span class="star-name">${inst.name_cn || inst.instrument_code}</span>
                <span class="star-mag">${inst.dynasty_name || ''} · ${inst.ring_count || 0}环</span>
            `;
            item.addEventListener('click', () => {
                document.querySelectorAll('#instrument-list .star-item').forEach(i => i.classList.remove('active'));
                item.classList.add('active');
                if (window.instrumentAnalyzer) {
                    window.instrumentAnalyzer.fetchInstrument(inst.id).then(detail => {
                        if (typeof window.instrumentAnalyzer.onInstrumentSelected === 'function') {
                            window.instrumentAnalyzer.onInstrumentSelected(detail);
                        }
                    }).catch(e => console.warn('加载仪器详情失败:', e));
                }
            });
            container.appendChild(item);
        });
    }

    _renderVariablePanel() {
        let panel = document.getElementById('variable-panel');
        if (!panel) {
            panel = document.createElement('div');
            panel.id = 'variable-panel';
            panel.className = 'feature-panel';
            panel.style.display = 'none';
            const sidePanel = document.getElementById('side-panel');
            if (sidePanel) sidePanel.appendChild(panel);
        }
        panel.innerHTML = `
            <section class="panel-section">
                <h3>变星列表</h3>
                <div id="variable-list" class="feature-list"></div>
            </section>
        `;
        const listEl = document.getElementById('variable-list');
        if (listEl && window.variableAnalyzer?.renderVariableList) {
            window.variableAnalyzer.renderVariableList(listEl, window.app?.variableStars || []);
        } else if (listEl) {
            this._renderSimpleVariableList(listEl);
        }
    }

    _renderSimpleVariableList(container) {
        if (!container || !window.app?.variableStars) return;
        const variables = window.app.variableStars;
        if (!variables || variables.length === 0) {
            container.innerHTML = '<div style="text-align:center;color:#8090b0;padding:20px;">暂无数据</div>';
            return;
        }
        container.innerHTML = '';
        variables.slice(0, 50).forEach(v => {
            const item = document.createElement('div');
            item.className = 'star-item';
            item.dataset.id = v.id;
            const periodStr = v.period_days ? v.period_days.toFixed(2) + ' 天' : '';
            item.innerHTML = `
                <span class="star-name">${v.star_name_cn || v.variable_name || '变星'}</span>
                <span class="star-mag">${v.variable_type || ''} ${periodStr ? '· ' + periodStr : ''}</span>
            `;
            item.addEventListener('click', () => {
                document.querySelectorAll('#variable-list .star-item').forEach(i => i.classList.remove('active'));
                item.classList.add('active');
                if (window.variableAnalyzer) {
                    window.variableAnalyzer.fetchVariable(v.id).then(detail => {
                        if (typeof window.variableAnalyzer.onVariableSelected === 'function') {
                            window.variableAnalyzer.onVariableSelected(detail);
                        }
                    }).catch(e => console.warn('加载变星详情失败:', e));
                }
            });
            container.appendChild(item);
        });
    }

    _renderStarmapPanel() {
        let panel = document.getElementById('starmap-panel');
        if (!panel) {
            panel = document.createElement('div');
            panel.id = 'starmap-panel';
            panel.className = 'feature-panel';
            panel.style.display = 'none';
            const sidePanel = document.getElementById('side-panel');
            if (sidePanel) sidePanel.appendChild(panel);
        }
        if (window.starmapGenerator?.renderForm) {
            window.starmapGenerator.renderForm(panel);
        } else {
            panel.innerHTML = `
                <section class="panel-section">
                    <h3>个人星图生成</h3>
                    <div style="padding:20px;text-align:center;color:#8090b0;">
                        星图生成器模块加载中...
                    </div>
                </section>
            `;
        }
    }

    showEclipseDetail(eclipse) {
        const panel = document.getElementById('detail-panel');
        const content = document.getElementById('detail-content');
        const titleEl = document.getElementById('detail-title');
        const dynastyTag = document.getElementById('dynasty-tag');
        if (!panel || !content || !titleEl) return;
        panel.style.display = 'flex';
        titleEl.textContent = eclipse.title || eclipse.name_cn || '日食详情';
        dynastyTag.textContent = eclipse.dynasty_name || eclipse.year ? (eclipse.year < 0 ? `前${-eclipse.year}年` : `公元${eclipse.year}年`) : '-';

        const yearStr = eclipse.year < 0 ? `前${-eclipse.year}年` : `公元${eclipse.year}年`;
        const typeLabel = eclipse.eclipse_type === 'solar' ? '日食' : (eclipse.eclipse_type === 'lunar' ? '月食' : eclipse.eclipse_type || '-');
        const magnitudeStr = eclipse.magnitude != null ? eclipse.magnitude.toFixed(3) : '-';

        content.innerHTML = `
            <div class="detail-section">
                <h3>基本信息</h3>
                <div class="info-grid">
                    <div class="info-item"><span class="key">类型：</span><span class="value">${typeLabel}</span></div>
                    <div class="info-item"><span class="key">朝代：</span><span class="value">${eclipse.dynasty_name || '-'}</span></div>
                    <div class="info-item"><span class="key">年份：</span><span class="value">${yearStr}</span></div>
                    <div class="info-item"><span class="key">食分：</span><span class="value">${magnitudeStr}</span></div>
                    <div class="info-item"><span class="key">食甚时间：</span><span class="value">${eclipse.max_time || '-'}</span></div>
                    <div class="info-item"><span class="key">持续时间：</span><span class="value">${eclipse.duration_min ? eclipse.duration_min + ' 分钟' : '-'}</span></div>
                </div>
            </div>
            ${eclipse.description ? `
            <div class="detail-section">
                <h3>文献记载</h3>
                <div style="font-size:13px;color:#a0b8d0;line-height:1.6;">${eclipse.description}</div>
            </div>
            ` : ''}
            ${eclipse.calculation_notes ? `
            <div class="detail-section">
                <h3>计算说明</h3>
                <div style="font-size:12px;color:#8090b0;line-height:1.5;">${eclipse.calculation_notes}</div>
            </div>
            ` : ''}
        `;
    }

    showInstrumentDetail(instrument) {
        const panel = document.getElementById('detail-panel');
        const content = document.getElementById('detail-content');
        const titleEl = document.getElementById('detail-title');
        const dynastyTag = document.getElementById('dynasty-tag');
        if (!panel || !content || !titleEl) return;
        panel.style.display = 'flex';
        titleEl.textContent = instrument.name_cn || instrument.instrument_code || '仪器详情';
        dynastyTag.textContent = instrument.dynasty_name || '-';

        const yearStr = instrument.erected_year != null
            ? (instrument.erected_year < 0 ? `前${-instrument.erected_year | 0}年` : `公元${instrument.erected_year | 0}年`)
            : '-';

        content.innerHTML = `
            <div class="detail-section">
                <h3>基本信息</h3>
                <div class="info-grid">
                    <div class="info-item"><span class="key">仪器名称：</span><span class="value">${instrument.name_cn || '-'}</span></div>
                    <div class="info-item"><span class="key">仪器代码：</span><span class="value">${instrument.instrument_code || '-'}</span></div>
                    <div class="info-item"><span class="key">朝代：</span><span class="value">${instrument.dynasty_name || '-'}</span></div>
                    <div class="info-item"><span class="key">建造年份：</span><span class="value">${yearStr}</span></div>
                    <div class="info-item"><span class="key">环数：</span><span class="value">${instrument.ring_count || 0} 环</span></div>
                    <div class="info-item"><span class="key">标称精度：</span><span class="value">${instrument.nominal_accuracy_arcmin != null ? instrument.nominal_accuracy_arcmin.toFixed(2) + "'" : '-'}</span></div>
                </div>
            </div>
            ${instrument.description ? `
            <div class="detail-section">
                <h3>仪器描述</h3>
                <div style="font-size:13px;color:#a0b8d0;line-height:1.6;">${instrument.description}</div>
            </div>
            ` : ''}
            ${instrument.history ? `
            <div class="detail-section">
                <h3>历史背景</h3>
                <div style="font-size:13px;color:#a0b8d0;line-height:1.6;">${instrument.history}</div>
            </div>
            ` : ''}
        `;
    }

    showVariableDetail(variable) {
        const panel = document.getElementById('detail-panel');
        const content = document.getElementById('detail-content');
        const titleEl = document.getElementById('detail-title');
        const dynastyTag = document.getElementById('dynasty-tag');
        if (!panel || !content || !titleEl) return;
        panel.style.display = 'flex';
        titleEl.textContent = variable.star_name_cn || variable.variable_name || '变星详情';
        dynastyTag.textContent = variable.variable_type || '-';

        const periodStr = variable.period_days != null ? variable.period_days.toFixed(3) + ' 天' : '-';
        const magMaxStr = variable.magnitude_max != null ? variable.magnitude_max.toFixed(2) : '-';
        const magMinStr = variable.magnitude_min != null ? variable.magnitude_min.toFixed(2) : '-';
        const amplitudeStr = variable.amplitude != null ? variable.amplitude.toFixed(2) + ' 等' : '-';

        content.innerHTML = `
            <div class="detail-section">
                <h3>基本信息</h3>
                <div class="info-grid">
                    <div class="info-item"><span class="key">星名：</span><span class="value">${variable.star_name_cn || '-'}</span></div>
                    <div class="info-item"><span class="key">变星类型：</span><span class="value">${variable.variable_type || '-'}</span></div>
                    <div class="info-item"><span class="key">周期：</span><span class="value">${periodStr}</span></div>
                    <div class="info-item"><span class="key">变光幅度：</span><span class="value">${amplitudeStr}</span></div>
                    <div class="info-item"><span class="key">最亮星等：</span><span class="value">${magMaxStr}</span></div>
                    <div class="info-item"><span class="key">最暗星等：</span><span class="value">${magMinStr}</span></div>
                </div>
            </div>
            <div class="detail-section">
                <h3>坐标信息</h3>
                <div class="info-grid">
                    <div class="info-item"><span class="key">RA (J2000)：</span><span class="value">${variable.ra_j2000 != null ? variable.ra_j2000.toFixed(4) + '°' : '-'}</span></div>
                    <div class="info-item"><span class="key">Dec (J2000)：</span><span class="value">${variable.dec_j2000 != null ? variable.dec_j2000.toFixed(4) + '°' : '-'}</span></div>
                    <div class="info-item"><span class="key">星宿：</span><span class="value">${variable.constellation || '-'}</span></div>
                    <div class="info-item"><span class="key">朝代：</span><span class="value">${variable.dynasty_name || '-'}</span></div>
                </div>
            </div>
            ${variable.notes ? `
            <div class="detail-section">
                <h3>备注</h3>
                <div style="font-size:13px;color:#a0b8d0;line-height:1.6;">${variable.notes}</div>
            </div>
            ` : ''}
        `;
    }

    showStarmapResult(result) {
        const panel = document.getElementById('detail-panel');
        const content = document.getElementById('detail-content');
        const titleEl = document.getElementById('detail-title');
        const dynastyTag = document.getElementById('dynasty-tag');
        if (!panel || !content || !titleEl) return;
        panel.style.display = 'flex';
        titleEl.textContent = result.title || '个人星图';
        dynastyTag.textContent = result.date_str || result.birth_date || '-';

        let planetsHTML = '';
        if (result.planets && result.planets.length > 0) {
            planetsHTML = `
                <div class="detail-section">
                    <h3>行星位置</h3>
                    <div class="info-grid">
                        ${result.planets.map(p => `
                            <div class="info-item">
                                <span class="key">${p.name_cn || p.name}：</span>
                                <span class="value">${p.ra != null ? p.ra.toFixed(2) + '°' : '-'} / ${p.dec != null ? p.dec.toFixed(2) + '°' : '-'}</span>
                            </div>
                        `).join('')}
                    </div>
                </div>
            `;
        }

        let mansionsHTML = '';
        if (result.mansions && result.mansions.length > 0) {
            mansionsHTML = `
                <div class="detail-section">
                    <h3>二十八宿</h3>
                    <div style="display:flex;flex-wrap:wrap;gap:8px;">
                        ${result.mansions.map(m => `
                            <span style="padding:4px 10px;background:rgba(60,80,120,0.4);border-radius:12px;font-size:12px;color:#a0c8ff;">
                                ${m.name_cn}宿
                            </span>
                        `).join('')}
                    </div>
                </div>
            `;
        }

        content.innerHTML = `
            <div class="detail-section">
                <h3>星图信息</h3>
                <div class="info-grid">
                    <div class="info-item"><span class="key">出生日期：</span><span class="value">${result.birth_date || '-'}</span></div>
                    <div class="info-item"><span class="key">出生时间：</span><span class="value">${result.birth_time || '-'}</span></div>
                    <div class="info-item"><span class="key">出生地点：</span><span class="value">${result.birth_place || '-'}</span></div>
                    <div class="info-item"><span class="key">恒星时：</span><span class="value">${result.sidereal_time != null ? result.sidereal_time.toFixed(2) + 'h' : '-'}</span></div>
                </div>
            </div>
            ${planetsHTML}
            ${mansionsHTML}
            ${result.share_link ? `
            <div class="detail-section">
                <h3>分享链接</h3>
                <div style="padding:10px;background:rgba(30,40,70,0.6);border-radius:6px;font-size:12px;color:#8090b0;word-break:break-all;">
                    ${result.share_link}
                </div>
            </div>
            ` : ''}
        `;
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
