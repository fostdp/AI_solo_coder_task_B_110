/* ============================================================
 * star_chart_3d.js - Three.js 3D 星图渲染模块
 * 职责:
 *   - 天球视图: 恒星/彗星/客星/遗迹/星宿边界/自行箭头
 *   - ShaderMaterial 高性能点云 (1200+ 恒星)
 *   - Raycaster 射线拾取
 *   - 鼠标拖拽旋转 + 滚轮缩放
 *   - 不包含面板/时间轴逻辑 (由 UI 模块负责)
 * ============================================================ */

class StarChart3D {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.labelsCanvas = document.getElementById('labels-canvas');
        this.scene = null;
        this.camera = null;
        this.renderer = null;

        this.stars = [];
        this.starPoints = null;
        this.comets = [];
        this.guests = [];
        this.snr = [];
        this.mansions = [];
        this.dynasties = [];

        this.starColors = null;
        this.starSizes = null;
        this.starVelocities = null;

        this.rotationX = 0.3;
        this.rotationZ = 0;
        this.camDist = 2.8;

        this.viewMode = 'sphere';
        this.displayFilter = 'all';
        this.styleMode = 'planck';
        this.magThreshold = 7.0;

        this.isDragging = false;
        this.lastX = 0;
        this.lastY = 0;

        this.raycaster = new THREE.Raycaster();
        this.mouse = new THREE.Vector2();
        this.hoveredStar = null;
        this.selectedStar = null;

        this.SPHERE_RADIUS = 100;

        this.onStarSelected = null;
        this.onGuestSelected = null;

        this._init();
    }

    _init() {
        this.scene = new THREE.Scene();
        this.scene.background = null;

        const w = this.canvas.clientWidth || window.innerWidth;
        const h = this.canvas.clientHeight || (window.innerHeight - 162);

        this.camera = new THREE.PerspectiveCamera(60, w / h, 0.1, 2000);
        this.camera.position.set(0, 0, this.camDist * this.SPHERE_RADIUS * 0.03);
        this.camera.lookAt(0, 0, 0);

        this.renderer = new THREE.WebGLRenderer({
            canvas: this.canvas,
            antialias: true,
            alpha: true,
        });
        this.renderer.setPixelRatio(window.devicePixelRatio);
        this.renderer.setSize(w, h, false);

        this._addSkySphere();
        this._addReferenceGrids();
        this._bindEvents();
        this._animate();
    }

    _addSkySphere() {
        const geom = new THREE.SphereGeometry(this.SPHERE_RADIUS, 64, 64);
        const mat = new THREE.ShaderMaterial({
            side: THREE.BackSide,
            transparent: true,
            uniforms: {},
            vertexShader: `
                varying vec3 vPos;
                void main() {
                    vPos = position;
                    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
                }
            `,
            fragmentShader: `
                varying vec3 vPos;
                void main() {
                    float r = length(vPos.xy) / 100.0;
                    vec3 col1 = vec3(0.02, 0.04, 0.10);
                    vec3 col2 = vec3(0.005, 0.008, 0.03);
                    vec3 c = mix(col2, col1, smoothstep(0.0, 1.0, r));
                    float gal = smoothstep(0.0, 0.3, 0.3 - abs(vPos.z / 100.0)) * 0.1;
                    c += vec3(0.15, 0.18, 0.25) * gal;
                    gl_FragColor = vec4(c, 1.0);
                }
            `,
        });
        this.scene.add(new THREE.Mesh(geom, mat));
    }

    _addReferenceGrids() {
        const grp = new THREE.Group();

        const eqPts = [];
        for (let ra = 0; ra <= 360; ra += 3) {
            const [x, y, z] = Astro.equatorialToCartesian(ra, 0, this.SPHERE_RADIUS * 0.995);
            eqPts.push(new THREE.Vector3(x, y, z));
        }
        const eqGeom = new THREE.BufferGeometry().setFromPoints(eqPts);
        grp.add(new THREE.Line(eqGeom, new THREE.LineBasicMaterial({
            color: 0x4060a0, transparent: true, opacity: 0.35
        })));

        const ecPts = [];
        for (let lon = 0; lon <= 360; lon += 3) {
            const eps = 23.44 * Astro.DEG2RAD;
            const lam = lon * Astro.DEG2RAD;
            const beta = 0;
            const ra = Math.atan2(
                Math.sin(lam) * Math.cos(eps) - Math.tan(beta) * Math.sin(eps),
                Math.cos(lam)
            ) * Astro.RAD2DEG;
            const dec = Math.asin(
                Math.sin(beta) * Math.cos(eps) + Math.cos(beta) * Math.sin(eps) * Math.sin(lam)
            ) * Astro.RAD2DEG;
            const [x, y, z] = Astro.equatorialToCartesian(Astro.normalize360(ra), dec, this.SPHERE_RADIUS * 0.995);
            ecPts.push(new THREE.Vector3(x, y, z));
        }
        const ecGeom = new THREE.BufferGeometry().setFromPoints(ecPts);
        grp.add(new THREE.Line(ecGeom, new THREE.LineBasicMaterial({
            color: 0xffaa40, transparent: true, opacity: 0.35
        })));

        this.scene.add(grp);
    }

    _bindEvents() {
        const c = this.canvas;

        c.addEventListener('mousedown', e => {
            this.isDragging = true;
            this.lastX = e.clientX;
            this.lastY = e.clientY;
        });
        window.addEventListener('mouseup', () => { this.isDragging = false; });

        window.addEventListener('mousemove', e => {
            if (this.isDragging) {
                const dx = e.clientX - this.lastX;
                const dy = e.clientY - this.lastY;
                this.rotationZ += dx * 0.005;
                this.rotationX += dy * 0.005;
                this.rotationX = Math.max(-Math.PI / 2 + 0.1, Math.min(Math.PI / 2 - 0.1, this.rotationX));
                this.lastX = e.clientX;
                this.lastY = e.clientY;
            } else {
                this._handleHover(e);
            }
        });

        c.addEventListener('wheel', e => {
            e.preventDefault();
            const s = e.deltaY > 0 ? 1.15 : 0.87;
            this.zoom(s);
        }, { passive: false });

        c.addEventListener('click', e => {
            if (this.isDragging) return;
            this._handleClick(e);
        });

        window.addEventListener('resize', () => this._onResize());
        this._onResize();
    }

    _onResize() {
        const w = this.canvas.clientWidth || window.innerWidth;
        const h = this.canvas.clientHeight || (window.innerHeight - 162);
        this.camera.aspect = w / h;
        this.camera.updateProjectionMatrix();
        this.renderer.setSize(w, h, false);
        if (this.labelsCanvas) {
            this.labelsCanvas.width = w;
            this.labelsCanvas.height = h;
        }
    }

    zoom(factor) {
        this.camDist = Math.max(1.3, Math.min(8.0, this.camDist * factor));
        this.camera.position.setLength(this.camDist * this.SPHERE_RADIUS * 0.03);
    }

    resetView() {
        this.rotationX = 0.3;
        this.rotationZ = 0;
        this.camDist = 2.8;
        this.camera.position.set(0, 0, this.camDist * this.SPHERE_RADIUS * 0.03);
    }

    flyTo(ra, dec, dist = 2.2) {
        const targetZ = -ra * Astro.DEG2RAD;
        const targetX = dec * Astro.DEG2RAD;
        this.rotationZ = targetZ;
        this.rotationX = targetX;
        this.camDist = dist;
        this.camera.position.set(0, 0, this.camDist * this.SPHERE_RADIUS * 0.03);
    }

    setViewMode(m) { this.viewMode = m; this._rebuildStars(); }
    setDisplayFilter(f) { this.displayFilter = f; this._rebuildStars(); this._refreshVisibility(); }
    setStyleMode(s) { this.styleMode = s; this._rebuildStars(); }
    setMagThreshold(v) { this.magThreshold = v; this._rebuildStars(); }

    setStars(stars) { this.stars = Array.isArray(stars) ? stars : []; this._rebuildStars(); }
    setDynasties(d) { this.dynasties = d; }
    setMansions(m) { this.mansions = m; this._rebuildMansionBoundaries(); }
    setComets(c) { this.comets = c || []; this._rebuildComets(); }
    setGuestStars(g) { this.guests = g || []; this._rebuildGuests(); }
    setSnr(s) { this.snr = s || []; this._rebuildSnr(); }

    _deselectStar() {
        this.selectedStar = null;
        this._hideTooltip();
    }

    _rebuildStars() {
        if (this.starPoints) {
            this.scene.remove(this.starPoints);
            this.starPoints.geometry.dispose();
            this.starPoints.material.dispose();
            this.starPoints = null;
        }

        const filtered = this.stars.filter(s => {
            const m = s.magnitude_num != null ? s.magnitude_num : 6;
            if (m > this.magThreshold) return false;
            if (this.displayFilter !== 'all' && this.displayFilter !== 'stars') return false;
            return s.ra_j2000 != null || s.ra_ancient_conv != null;
        });

        if (filtered.length === 0) return;

        const positions = new Float32Array(filtered.length * 3);
        const colors = new Float32Array(filtered.length * 3);
        const sizes = new Float32Array(filtered.length);
        const velocities = new Float32Array(filtered.length * 3);

        filtered.forEach((s, i) => {
            const ra = (s.ra_j2000 != null ? s.ra_j2000 : s.ra_ancient_conv) || 0;
            const dec = (s.dec_j2000 != null ? s.dec_j2000 : s.dec_ancient_conv) || 0;
            const [x, y, z] = Astro.equatorialToCartesian(ra, dec, this.SPHERE_RADIUS * 0.98);
            positions[i*3] = x; positions[i*3+1] = y; positions[i*3+2] = z;

            const col = Astro.starToColor(s, this.styleMode);
            colors[i*3] = col.r / 255;
            colors[i*3+1] = col.g / 255;
            colors[i*3+2] = col.b / 255;

            const mag = s.magnitude_num != null ? s.magnitude_num : 6;
            sizes[i] = 8.0 * Math.pow(0.63, mag);

            const pmRa = s.proper_motion_ra || 0;
            const pmDec = s.proper_motion_dec || 0;
            const cosDec = Math.cos(dec * Astro.DEG2RAD) || 1;
            const draDeg = pmRa * 1000 / (3600 * 1000) / cosDec;
            const ddecDeg = pmDec * 1000 / (3600 * 1000);
            const ra2 = Astro.normalize360(ra + draDeg);
            const dec2 = Math.max(-89, Math.min(89, dec + ddecDeg));
            const [x2, y2, z2] = Astro.equatorialToCartesian(ra2, dec2, this.SPHERE_RADIUS * 0.98);
            velocities[i*3] = x2 - x;
            velocities[i*3+1] = y2 - y;
            velocities[i*3+2] = z2 - z;
        });

        const geom = new THREE.BufferGeometry();
        geom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
        geom.setAttribute('color', new THREE.BufferAttribute(colors, 3));
        geom.setAttribute('size', new THREE.BufferAttribute(sizes, 1));

        const mat = new THREE.ShaderMaterial({
            uniforms: {
                uPixelRatio: { value: window.devicePixelRatio },
            },
            vertexShader: `
                attribute vec3 color;
                attribute float size;
                varying vec3 vColor;
                uniform float uPixelRatio;
                void main() {
                    vColor = color;
                    vec4 mv = modelViewMatrix * vec4(position, 1.0);
                    gl_PointSize = size * uPixelRatio * (1.0 / -mv.z) * 300.0;
                    gl_Position = projectionMatrix * mv;
                }
            `,
            fragmentShader: `
                varying vec3 vColor;
                void main() {
                    vec2 uv = gl_PointCoord - 0.5;
                    float d = length(uv);
                    if (d > 0.5) discard;
                    float alpha = exp(-d * d * 14.0);
                    vec3 c = vColor * (1.0 + smoothstep(0.0, 0.2, 0.2 - d) * 0.8);
                    gl_FragColor = vec4(c, alpha);
                }
            `,
            transparent: true,
            depthWrite: false,
            blending: THREE.AdditiveBlending,
        });

        this.starPoints = new THREE.Points(geom, mat);
        this.starPoints.userData.filtered = filtered;
        this.scene.add(this.starPoints);
    }

    _rebuildMansionBoundaries() {
        this.scene.children.forEach(obj => {
            if (obj.userData && obj.userData.isMansion) {
                this.scene.remove(obj);
            }
        });
        if (!this.mansions || this.mansions.length === 0) return;

        const grp = new THREE.Group();
        grp.userData.isMansion = true;

        this.mansions.forEach(m => {
            const pts = [];
            for (let dec = 80; dec >= -80; dec -= 4) {
                const [x, y, z] = Astro.equatorialToCartesian(m.ra_start_deg, dec, this.SPHERE_RADIUS * 0.99);
                pts.push(new THREE.Vector3(x, y, z));
            }
            const g = new THREE.BufferGeometry().setFromPoints(pts);
            grp.add(new THREE.Line(g, new THREE.LineBasicMaterial({
                color: 0x5070b0,
                transparent: true,
                opacity: 0.2,
            })));
        });
        this.scene.add(grp);
    }

    _rebuildComets() {
        this.scene.children.forEach(o => {
            if (o.userData?.isComet) this.scene.remove(o);
        });
        if (this.displayFilter !== 'all' && this.displayFilter !== 'comets') return;

        const grp = new THREE.Group();
        grp.userData.isComet = true;
        this.comets.forEach(c => {
            if (c.ra_deg == null) return;
            const [x, y, z] = Astro.equatorialToCartesian(c.ra_deg, c.dec_deg, this.SPHERE_RADIUS * 0.97);
            const size = 1.2 + Math.pow(0.6, c.magnitude ?? 4) * 1.5;

            const canvas = document.createElement('canvas');
            canvas.width = 64; canvas.height = 64;
            const ctx = canvas.getContext('2d');
            const g = ctx.createRadialGradient(32, 32, 0, 32, 32, 30);
            g.addColorStop(0, 'rgba(112,192,255,1)');
            g.addColorStop(0.3, 'rgba(112,192,255,0.7)');
            g.addColorStop(1, 'rgba(112,192,255,0)');
            ctx.fillStyle = g;
            ctx.beginPath();
            ctx.moveTo(32, 6); ctx.lineTo(56, 32); ctx.lineTo(32, 58); ctx.lineTo(8, 32);
            ctx.closePath();
            ctx.fill();

            const tex = new THREE.CanvasTexture(canvas);
            const mat = new THREE.SpriteMaterial({ map: tex, transparent: true, depthWrite: false });
            const sp = new THREE.Sprite(mat);
            sp.position.set(x, y, z);
            sp.scale.set(size * 4, size * 4, 1);
            sp.userData = { type: 'comet', data: c };
            grp.add(sp);
        });
        this.scene.add(grp);
    }

    _rebuildGuests() {
        this.scene.children.forEach(o => {
            if (o.userData?.isGuest) this.scene.remove(o);
        });
        if (this.displayFilter !== 'all' && this.displayFilter !== 'guests') return;

        const grp = new THREE.Group();
        grp.userData.isGuest = true;
        this.guests.forEach(g => {
            if (g.ra_deg == null) return;
            const [x, y, z] = Astro.equatorialToCartesian(g.ra_deg, g.dec_deg, this.SPHERE_RADIUS * 0.97);
            const size = 2.5 + Math.pow(0.6, g.peak_mag ?? 2) * 3;

            const canvas = document.createElement('canvas');
            canvas.width = 128; canvas.height = 128;
            const ctx = canvas.getContext('2d');
            ctx.fillStyle = 'rgba(255,100,100,0.95)';
            ctx.beginPath(); ctx.arc(64, 64, 8, 0, Math.PI * 2); ctx.fill();
            ctx.strokeStyle = 'rgba(255,160,160,0.7)';
            ctx.lineWidth = 2;
            ctx.beginPath();
            ctx.moveTo(4, 64); ctx.lineTo(124, 64);
            ctx.moveTo(64, 4); ctx.lineTo(64, 124);
            ctx.stroke();
            const grad = ctx.createRadialGradient(64, 64, 5, 64, 64, 60);
            grad.addColorStop(0, 'rgba(255,120,120,0.6)');
            grad.addColorStop(1, 'rgba(255,120,120,0)');
            ctx.fillStyle = grad;
            ctx.beginPath(); ctx.arc(64, 64, 60, 0, Math.PI * 2); ctx.fill();

            const tex = new THREE.CanvasTexture(canvas);
            const mat = new THREE.SpriteMaterial({
                map: tex, transparent: true, depthWrite: false,
            });
            const sp = new THREE.Sprite(mat);
            sp.position.set(x, y, z);
            sp.scale.set(size * 3, size * 3, 1);
            sp.userData = { type: 'guest', data: g };
            grp.add(sp);
        });
        this.scene.add(grp);
    }

    _rebuildSnr() {
        this.scene.children.forEach(o => {
            if (o.userData?.isSnr) this.scene.remove(o);
        });
        if (this.displayFilter !== 'all' && this.displayFilter !== 'snr') return;

        const grp = new THREE.Group();
        grp.userData.isSnr = true;
        this.snr.forEach(s => {
            const [x, y, z] = Astro.equatorialToCartesian(s.ra_deg, s.dec_deg, this.SPHERE_RADIUS * 0.965);
            const dpc = s.diameter_pc ?? 10;
            const size = 1.5 + Math.log10(dpc + 1) * 1.8;

            const canvas = document.createElement('canvas');
            canvas.width = 64; canvas.height = 64;
            const ctx = canvas.getContext('2d');
            ctx.strokeStyle = 'rgba(192,112,255,0.9)';
            ctx.lineWidth = 2;
            ctx.beginPath(); ctx.arc(32, 32, 24, 0, Math.PI * 2); ctx.stroke();
            ctx.strokeStyle = 'rgba(192,112,255,0.4)';
            ctx.lineWidth = 1;
            ctx.beginPath(); ctx.arc(32, 32, 14, 0, Math.PI * 2); ctx.stroke();

            const tex = new THREE.CanvasTexture(canvas);
            const mat = new THREE.SpriteMaterial({ map: tex, transparent: true, depthWrite: false });
            const sp = new THREE.Sprite(mat);
            sp.position.set(x, y, z);
            sp.scale.set(size * 2.5, size * 2.5, 1);
            sp.userData = { type: 'snr', data: s };
            grp.add(sp);
        });
        this.scene.add(grp);
    }

    _refreshVisibility() {
        this._rebuildComets();
        this._rebuildGuests();
        this._rebuildSnr();
    }

    _handleHover(e) {
        const rect = this.canvas.getBoundingClientRect();
        this.mouse.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
        this.mouse.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;

        this.raycaster.setFromCamera(this.mouse, this.camera);

        const sprites = [];
        this.scene.traverse(o => {
            if (o.isSprite && o.userData?.type) sprites.push(o);
        });
        const spriteHit = this.raycaster.intersectObjects(sprites)[0];
        if (spriteHit) {
            const ud = spriteHit.object.userData;
            this._showTooltip(e.clientX, e.clientY, ud.type, ud.data);
            this.canvas.style.cursor = 'pointer';
            return;
        }

        if (this.starPoints) {
            const hits = this.raycaster.intersectObject(this.starPoints);
            if (hits.length > 0) {
                const idx = hits[0].index;
                const s = this.starPoints.userData.filtered[idx];
                this.hoveredStar = s;
                this._showTooltip(e.clientX, e.clientY, 'star', s);
                this.canvas.style.cursor = 'pointer';
                return;
            }
        }

        this.hoveredStar = null;
        this._hideTooltip();
        this.canvas.style.cursor = 'grab';
    }

    _handleClick(e) {
        const rect = this.canvas.getBoundingClientRect();
        this.mouse.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
        this.mouse.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;
        this.raycaster.setFromCamera(this.mouse, this.camera);

        const sprites = [];
        this.scene.traverse(o => {
            if (o.isSprite && o.userData?.type) sprites.push(o);
        });
        const spriteHit = this.raycaster.intersectObjects(sprites)[0];
        if (spriteHit) {
            const ud = spriteHit.object.userData;
            if (ud.type === 'guest' && this.onGuestSelected) {
                this.onGuestSelected(ud.data);
            }
            return;
        }

        if (this.starPoints) {
            const hits = this.raycaster.intersectObject(this.starPoints);
            if (hits.length > 0) {
                const idx = hits[0].index;
                const s = this.starPoints.userData.filtered[idx];
                this.selectedStar = s;
                if (this.onStarSelected) {
                    this.onStarSelected(s);
                }
            }
        }
    }

    _showTooltip(x, y, type, data) {
        const el = document.getElementById('star-tooltip');
        if (!el) return;
        el.style.display = 'block';
        el.style.left = (x + 14) + 'px';
        el.style.top = (y + 14) + 'px';

        let html = '';
        if (type === 'star') {
            html = `
                <div class="tt-title">${data.star_name_cn || '未命名星'}</div>
                <div class="tt-row"><span>朝代:</span><b>${data.dynasty_name || '-'}</b></div>
                <div class="tt-row"><span>RA:</span><b>${(data.ra_j2000 ?? data.ra_ancient_conv ?? 0).toFixed(2)}°</b></div>
                <div class="tt-row"><span>Dec:</span><b>${(data.dec_j2000 ?? data.dec_ancient_conv ?? 0).toFixed(2)}°</b></div>
                <div class="tt-row"><span>星等:</span><b>${data.magnitude_num != null ? data.magnitude_num.toFixed(1) : '-'}</b></div>
                <div class="tt-row"><span>颜色:</span><b>${data.color_desc || '-'}</b></div>
            `;
        } else if (type === 'comet') {
            html = `
                <div class="tt-title" style="color:#70c0ff;">${data.comet_id_code}</div>
                <div class="tt-row"><span>朝代:</span><b>${data.dynasty_name || '-'}</b></div>
                <div class="tt-row"><span>年份:</span><b>${data.year_ce || '-'}</b></div>
                <div class="tt-row"><span>星等:</span><b>${data.magnitude != null ? data.magnitude.toFixed(1) : '-'}</b></div>
            `;
        } else if (type === 'guest') {
            html = `
                <div class="tt-title" style="color:#ff8080;">${data.guest_id_code} · 客星</div>
                <div class="tt-row"><span>朝代:</span><b>${data.dynasty_name || '-'}</b></div>
                <div class="tt-row"><span>年份:</span><b>公元 ${Math.round(data.year_ce)}</b></div>
                <div class="tt-row"><span>峰值星等:</span><b>${data.peak_mag?.toFixed(1)}</b></div>
                <div class="tt-row"><span>可见期:</span><b>${data.visibility_days || '-'} 天</b></div>
                <div style="margin-top:6px;font-size:10px;color:#ffa060;">点击查看匹配结果 →</div>
            `;
        } else if (type === 'snr') {
            html = `
                <div class="tt-title" style="color:#c070ff;">${data.remnant_name}</div>
                <div class="tt-row"><span>类型:</span><b>${data.sn_type}</b></div>
                <div class="tt-row"><span>年龄:</span><b>${Math.round(data.age_yr)} 年</b></div>
                <div class="tt-row"><span>距离:</span><b>${data.distance_kpc?.toFixed(2)} kpc</b></div>
            `;
        }
        el.innerHTML = html;
    }

    _hideTooltip() {
        const el = document.getElementById('star-tooltip');
        if (el) el.style.display = 'none';
    }

    _animate() {
        requestAnimationFrame(() => this._animate());

        this.scene.rotation.x = this.rotationX;
        this.scene.rotation.y = this.rotationZ;

        const t = performance.now() * 0.001;
        this.scene.traverse(o => {
            if (o.userData?.type === 'guest' && o.material) {
                o.material.opacity = 0.75 + Math.sin(t * 1.5) * 0.25;
                const s = o.scale.x;
                const bs = 2.0;
                o.scale.setScalar(s + Math.sin(t * 2) * 0.02 * bs);
            }
        });

        this.renderer.render(this.scene, this.camera);
    }
}

window.StarChart3D = StarChart3D;
