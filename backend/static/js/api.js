/* ============================================================
 * API 客户端封装
 * ============================================================ */

const API_BASE = (() => {
    const p = window.location.protocol;
    const h = window.location.hostname;
    const port = window.location.port === '8080' ? '8080' : '8080';
    // 如果是 3000 等开发端口, 仍连接 8080 后端
    return `${p}//${h}:${port}/api`;
})();

const api = {
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
    },

    health() { return this._json(`${API_BASE}/health`); },
    getDynasties() { return this._json(`${API_BASE}/dynasties`); },
    getMansions() { return this._json(`${API_BASE}/mansions`); },

    getStars(params = {}) {
        const q = new URLSearchParams();
        for (const [k, v] of Object.entries(params)) {
            if (v != null && v !== '') q.set(k, v);
        }
        const s = q.toString();
        return this._json(`${API_BASE}/stars${s ? '?' + s : ''}`);
    },

    getStar(id) { return this._json(`${API_BASE}/stars/${id}`); },

    getCrossDynasty(id) {
        return this._json(`${API_BASE}/stars/${id}/cross-dynasty`);
    },

    convertRuxiuToJ2000(body) {
        return this._json(`${API_BASE}/convert/ruxiu-to-j2000`, 'POST', body);
    },

    getTrajectory(body) {
        return this._json(`${API_BASE}/trajectory`, 'POST', body);
    },

    getComets() { return this._json(`${API_BASE}/comets`); },

    getGuestStars() { return this._json(`${API_BASE}/guest-stars`); },
    getGuest(id) { return this._json(`${API_BASE}/guest-stars/${id}`); },

    getSnr() { return this._json(`${API_BASE}/snr`); },

    runMatch(guestId, topK = 10) {
        return this._json(
            `${API_BASE}/match/${guestId}?top_k=${topK}`,
            'POST'
        );
    },
    getMatches(guestId) {
        return this._json(`${API_BASE}/match/${guestId}`);
    },
};

window.api = api;
