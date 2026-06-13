/* ============================================================
 * 应用主入口 v0.3
 * 职责: 模块协调器，负责初始化、事件桥接、数据流
 * 依赖: StarChart3D (星图) + UI (筛选/时间轴) + TransientPanel (匹配)
 * ============================================================ */

class Application {
    constructor() {
        this.starChart = null;
        this.ui = null;
        this.transientPanel = null;

        this.allStars = [];
        this.dynastyStars = {};
        this.guests = [];
        this.snr = [];
        this.dynasties = [];
        this.mansions = [];

        this._init();
    }

    async _init() {
        this._showLoading(true);

        try {
            // 1. 初始化 3D 星图 (StarChart3D)
            this.starChart = new StarChart3D('star-canvas');
            window.starField = this.starChart;
            window.starChart = this.starChart;

            // 2. 初始化 UI (筛选面板/时间轴/恒星详情)
            this.ui = new UI(this.starChart);
            window.ui = this.ui;

            // 3. 初始化客星匹配面板 (TransientPanel)
            this.transientPanel = new TransientPanel(this.starChart);
            window.transientPanel = this.transientPanel;

            // 4. 事件桥接 (StarChart3D → UI / TransientPanel)
            this._bindModuleEvents();

            // 5. 加载数据
            await this._loadAllData();

        } catch (e) {
            console.error('Init failed:', e);
            this._showError('初始化失败: ' + e.message);
        }

        this._showLoading(false);
    }

    _bindModuleEvents() {
        // StarChart3D → UI：恒星选中 → 显示详情
        this.starChart.onStarSelected = (star) => {
            this.ui._showStarDetail(star);
        };

        // StarChart3D → TransientPanel：客星选中 → 加载匹配
        this.starChart.onGuestSelected = (guest) => {
            this._openMatchPanel(guest);
        };

        // UI → StarChart3D：朝代切换 → 筛选恒星
        window.onDynastyChange = (dynasty) => {
            this._filterStarsByDynasty(dynasty);
        };

        // UI → StarChart3D：对比模式切换
        window.onCompareChange = (cmp) => {
            if (cmp && this.ui.currentDynasty) {
                const id1 = this.ui.currentDynasty.id;
                const id2 = cmp.id;
                this.starChart.setStars(this.allStars.filter(s =>
                    s.dynasty_id === id1 || s.dynasty_id === id2));
            } else if (this.ui.currentDynasty && !this.ui.compareMode) {
                this._filterStarsByDynasty(this.ui.currentDynasty);
            } else {
                this.starChart.setStars(this.allStars);
            }
        };
    }

    async _loadAllData() {
        await Promise.all([this._loadDynasties(), this._loadMansions()]);
        await Promise.all([
            this._loadComets(),
            this._loadGuestStars(),
            this._loadSnr()
        ]);
        await this._loadAllStars();
    }

    async _loadDynasties() {
        const list = await window.api.getDynasties();
        this.dynasties = list || [];
        this.starChart.setDynasties(this.dynasties);
        this.ui.setDynasties(this.dynasties);
    }

    async _loadMansions() {
        const list = await window.api.getMansions();
        this.mansions = list || [];
        this.starChart.setMansions(this.mansions);
        this.ui.setMansions(this.mansions);
    }

    async _loadAllStars() {
        try {
            const data = await window.api.getStars({ limit: 2000 });
            this.allStars = Array.isArray(data) ? data : [];
            this.dynastyStars = {};
            this.allStars.forEach(s => {
                if (!this.dynastyStars[s.dynasty_id]) this.dynastyStars[s.dynasty_id] = [];
                this.dynastyStars[s.dynasty_id].push(s);
            });
            this.starChart.setStars(this.allStars);
        } catch (e) {
            console.warn('加载恒星失败:', e);
            this.allStars = [];
        }
    }

    async _loadComets() {
        try {
            const list = await window.api.getComets();
            this.starChart.setComets(list || []);
        } catch (e) { console.warn('彗星加载失败:', e); }
    }

    async _loadGuestStars() {
        try {
            const list = await window.api.getGuestStars();
            this.guests = list || [];
            this.starChart.setGuestStars(this.guests);
        } catch (e) { console.warn('客星加载失败:', e); }
    }

    async _loadSnr() {
        try {
            const list = await window.api.getSnr();
            this.snr = list || [];
            this.starChart.setSnr(this.snr);
        } catch (e) { console.warn('SNR加载失败:', e); }
    }

    _filterStarsByDynasty(dynasty) {
        if (!dynasty || this.ui.compareMode) {
            if (this.ui.compareMode && this.ui.currentDynasty && this.ui.compareDynasty) {
                const id1 = this.ui.currentDynasty.id;
                const id2 = this.ui.compareDynasty.id;
                this.starChart.setStars(this.allStars.filter(s =>
                    s.dynasty_id === id1 || s.dynasty_id === id2));
                return;
            }
            this.starChart.setStars(this.allStars);
            return;
        }
        const list = this.dynastyStars[dynasty.id] || [];
        this.starChart.setStars(list);
    }

    async _openMatchPanel(guest) {
        // 使用 TransientPanel 模块，而非 app 内联逻辑
        this.transientPanel.setGuest(guest, null, null);
        try {
            const result = await window.api.runMatch(guest.id, 10);
            if (result) {
                this.transientPanel.setGuest(
                    result.guest || guest,
                    result.candidates || result,
                    result.method
                );
            }
        } catch (e) {
            console.error('匹配失败:', e);
            try {
                const saved = await window.api.getMatches(guest.id);
                if (saved && saved.length > 0) {
                    this.transientPanel.setGuest(guest, saved, {
                        name: 'Bayesian match (cached)',
                        version: '2.0',
                        prior_model: 'Exponential disk + isothermal disk',
                        n_candidates_evaluated: saved.length,
                        n_candidates_returned: saved.length,
                        log_bayes_factor_top: 0,
                    });
                }
            } catch (_) {}
        }
    }

    _showLoading(show) {
        const el = document.getElementById('loading');
        if (el) el.style.display = show ? 'block' : 'none';
    }

    _showError(msg) {
        const el = document.getElementById('loading');
        if (el) {
            el.innerHTML = `<div style="color:#ff8080;">${msg}</div>`;
            setTimeout(() => (el.style.display = 'none'), 5000);
        }
    }
}

window.addEventListener('DOMContentLoaded', () => { window.app = new Application(); });
