/* ───────────────────────────────────────────────────────────────────────────
   3D Mechanical Keyboard, ported from 21st.dev "Mechanical Keyboard" (ayushmxxn,
   React Three Fiber) to plain three.js: same layout builder, sculpted keycap geometry
   (12 rings, concave dish), canvas-drawn keycap textures, key travel (lerp at delta*32,
   160 ms release), CNC unibody chassis with milled key well and display pocket,
   machined knobs, braided cable, smart display, shadow updates only while keys move,
   pause when off-screen or the tab is hidden. Rethemed to the Humcase night: dark
   caps, copper keys for the natures, DM Mono silk-screen legends, a desk in the dark.
   Not ported: orbit controls (they would steal the page's scroll) and the key sound.
   ─────────────────────────────────────────────────────────────────────────── */
const desk = document.getElementById('desk');
const kbCanvas = document.getElementById('kb');
const wide = matchMedia('(min-width: 768px)');
const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;
let started = false;
const near = new IntersectionObserver(es => { if (es.some(e => e.isIntersecting)) start(); }, { rootMargin: '700px 0px' });
near.observe(desk);
wide.addEventListener('change', () => { if (wide.matches) near.observe(desk); });
function start() {
  if (started || !wide.matches) return;
  started = true; near.disconnect(); init();
}

async function init() {
  let THREE, RoundedBoxGeometry, RoomEnvironment;
  try {
    THREE = await import('three');
    ({ RoundedBoxGeometry } = await import('three/addons/geometries/RoundedBoxGeometry.js'));
    ({ RoomEnvironment } = await import('three/addons/environments/RoomEnvironment.js'));
  } catch (err) {
    console.warn('3D keyboard unavailable, touch keys shown instead', err);
    document.documentElement.classList.add('no-kb3d');
    return;
  }
  try { await document.fonts.load('500 48px "DM Mono"'); } catch { /* the fallback mono is fine */ }

  // ── Config (KEYBOARD_CONFIG of the original, Humcase night colours) ──
  const C = {
    keycap: '#2b2a27', accent: '#e78a53', chassis: '#1d1c1a', chassisHi: '#3a3834', plate: '#060606', knob: '#151513', bezel: '#070707',
    legend: '#ebe8e1', sub: '#8f8c86', accentLegend: '#17120e', accentSub: 'rgba(23,18,14,.74)',
    font: '"DM Mono", "Cascadia Mono", ui-monospace, monospace',
    unit: .228, gap: .004, travel: .006,
  };
  const KNOBS = { topZ: -.552, bottomZ: -.37, angTop: .75, angBottom: -.48, radius: .082, finW: .144, finH: .026, finL: .022 };
  const LAYOUT = [
    [{ id: 'esc', label: 'esc', sub: 'empty', w: 1.35, a: true }, ...Array.from({ length: 12 }, (_, i) => ({ id: 'f' + (i + 1), label: 'F' + (i + 1) })), { id: 'del', label: 'del', w: 1.4 }],
    [{ id: 'grav', label: '`', sub: '~' }, ...'1234567890'.split('').map((n, i) => ({ id: 'k' + n, label: n, sub: '!@#$%^&*()'[i] })), { id: 'min', label: '-', sub: '_' }, { id: 'equ', label: '=', sub: '+' }, { id: 'bsp', label: 'backspace', w: 1.75 }],
    [{ id: 'tab', label: 'tab', w: 1.5 }, { id: 'q', label: 'Q', nat: 'question', a: true }, { id: 'w', label: 'W' }, { id: 'e', label: 'E' }, { id: 'r', label: 'R', nat: 'research', a: true },
      { id: 't', label: 'T', nat: 'task', a: true }, { id: 'y', label: 'Y' }, { id: 'u', label: 'U' }, { id: 'i', label: 'I', nat: 'idea', a: true }, { id: 'o', label: 'O' }, { id: 'p', label: 'P' },
      { id: 'lb', label: '{', sub: '[' }, { id: 'rb', label: '}', sub: ']' }, { id: 'bsl', label: '\\', sub: '|', w: 1.25 }],
    [{ id: 'caps', label: 'caps', w: 1.75 }, { id: 'a', label: 'A' }, { id: 's', label: 'S' }, { id: 'd', label: 'D', nat: 'decision', a: true }, { id: 'f', label: 'F', bump: true },
      { id: 'g', label: 'G' }, { id: 'h', label: 'H' }, { id: 'j', label: 'J', bump: true }, { id: 'k', label: 'K' }, { id: 'l', label: 'L' },
      { id: 'sem', label: ';', sub: ':' }, { id: 'quo', label: "'", sub: '"' }, { id: 'enter', label: '↵', sub: 'sprout', w: 2.0, a: true }],
    [{ id: 'lsh', label: 'shift', w: 2.25 }, ...'ZXCVBNM'.split('').map(l => ({ id: l.toLowerCase(), label: l })), { id: 'com', label: ',', sub: '<' }, { id: 'per', label: '.', sub: '>' },
      { id: 'sla', label: '/', sub: '?' }, { id: 'rsh', label: 'shift', w: 1.5 }, { id: 'up', label: '↑' }],
    [{ id: 'ctrl', label: 'ctrl', w: 1.25 }, { id: 'fn1', label: 'fn' }, { id: 'win', label: 'win', w: 1.25 }, { id: 'lalt', label: 'alt', w: 1.25 }, { id: 'space', label: '', w: 6.0 },
      { id: 'ralt', label: 'alt' }, { id: 'fn2', label: 'fn' }, { id: 'left', label: '←' }, { id: 'down', label: '↓' }, { id: 'right', label: '→' }],
  ];
  const ACTION = { i: 'idea', t: 'task', q: 'question', d: 'decision', r: 'research', enter: 'sprout', esc: 'empty' };

  // ── Derived proportions (as the original) ──
  const U = C.unit, KEY_GAP = C.gap, CHASSIS_THICK = .04, WELL_DEPTH = .012, WELL_PAD = .012;
  const COLS_MAIN = 14.75, COLS_EXT = 15.75, ROWS = 6, RIM = .068;
  const DISPLAY_W = .176, DISPLAY_H = .58, DISPLAY_Z = .124;
  const KEY_WELL_MAIN_W = COLS_MAIN * U + WELL_PAD * 2, KEY_WELL_EXT_W = COLS_EXT * U + WELL_PAD * 2, KEY_WELL_D = ROWS * U + WELL_PAD * 2;
  const CHASSIS_W = RIM + KEY_WELL_EXT_W + RIM, CHASSIS_D = RIM + KEY_WELL_D + RIM;
  const CHASSIS_TOP_Y = .026, PLATE_Y = CHASSIS_TOP_Y - WELL_DEPTH;
  const KW_X1 = -CHASSIS_W / 2 + RIM, KW_MAIN_X2 = KW_X1 + KEY_WELL_MAIN_W, KW_EXT_X2 = KW_X1 + KEY_WELL_EXT_W, KW_Z1 = -KEY_WELL_D / 2;
  const RIGHT_PANEL_X = KW_X1 + WELL_PAD + 15.25 * U, DISPLAY_X = RIGHT_PANEL_X + .015;

  const defs = [];
  LAYOUT.forEach((row, r) => { let x = 0; for (const d of row) { const w = d.w ?? 1; defs.push({ ...d, col: x, row: r, wUnits: w, width: w * U - KEY_GAP, depth: U - KEY_GAP }); x += w * U; } });

  // ── Renderer, scene, camera ──
  const renderer = new THREE.WebGLRenderer({ canvas: kbCanvas, antialias: true, alpha: true, powerPreference: 'high-performance' });
  renderer.setPixelRatio(Math.min(devicePixelRatio || 1, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping; renderer.toneMappingExposure = 1.12;
  renderer.shadowMap.enabled = true; renderer.shadowMap.type = THREE.PCFSoftShadowMap; renderer.shadowMap.autoUpdate = false;
  const scene = new THREE.Scene();
  const pmrem = new THREE.PMREMGenerator(renderer);
  scene.environment = pmrem.fromScene(new RoomEnvironment(), .04).texture; scene.environmentIntensity = .16;
  const camera = new THREE.PerspectiveCamera(26, 2, .01, 100);

  // lights: the studio rig of the original, rethemed to a lamp-less night (warm key, copper rim from the tree, cool fill)
  const key = new THREE.DirectionalLight('#ffe0c2', 1.45); key.position.set(2.4, 5.5, 1.8); key.castShadow = true;
  key.shadow.mapSize.set(1024, 1024); Object.assign(key.shadow.camera, { near: .1, far: 24, left: -4, right: 4, top: 3.5, bottom: -3.5 }); key.shadow.bias = -.0002; key.shadow.normalBias = .01;
  const fill = new THREE.DirectionalLight('#8ea6c6', .32); fill.position.set(-2.8, 4, -.8);
  const rim = new THREE.DirectionalLight('#ff9a55', 1.0); rim.position.set(0, 2.4, -5);
  const sheen = new THREE.DirectionalLight('#f0f4fc', .22); sheen.position.set(1.6, 4.2, -1);
  scene.add(key, fill, rim, sheen, new THREE.AmbientLight('#3e322a', .9));

  const root = new THREE.Group(); root.rotation.y = -.035; scene.add(root);

  // ── Keycap textures (getKeyTexture, dark PBT) ──
  function centered(ctx, text, cx, cy) {
    const m = ctx.measureText(text), off = ((m.actualBoundingBoxAscent ?? 0) - (m.actualBoundingBoxDescent ?? 0)) / 2;
    ctx.textBaseline = 'alphabetic'; ctx.fillText(text, cx, cy + off);
  }
  function keyTexture(d) {
    const accent = !!d.a, aspect = d.width / d.depth;
    const SW = Math.round(512 * Math.max(1, aspect)), SH = 512;
    const cv = document.createElement('canvas'); cv.width = SW; cv.height = SH;
    const ctx = cv.getContext('2d');
    ctx.fillStyle = accent ? C.accent : C.keycap; ctx.fillRect(0, 0, SW, SH);
    // molded PBT micro-grain
    ctx.globalAlpha = accent ? .02 : .035;
    for (let y = 0; y < SH; y += 4) for (let x = 0; x < SW; x += 4) { const n = (x * 17 + y * 37) % 7; if (n < 2) { ctx.fillStyle = n ? '#fff' : '#000'; ctx.fillRect(x, y, 2, 2); } }
    ctx.globalAlpha = 1;
    // inner bevel, rear slope shadow, front rim highlight, dish
    const mx = SW * .125, my = SH * .125, bw = SW - mx * 2, bh = SH - my * 2, br = Math.min(84, bw * .26, bh * .26);
    ctx.lineWidth = 10; ctx.strokeStyle = accent ? 'rgba(0,0,0,.14)' : 'rgba(0,0,0,.3)'; ctx.beginPath(); ctx.roundRect(mx, my, bw, bh, br); ctx.stroke();
    let g = ctx.createLinearGradient(0, my - 4, 0, my + 38); g.addColorStop(0, accent ? 'rgba(0,0,0,.18)' : 'rgba(0,0,0,.32)'); g.addColorStop(1, 'rgba(0,0,0,0)');
    ctx.fillStyle = g; ctx.beginPath(); ctx.roundRect(mx, my - 2, bw, 40, br); ctx.fill();
    g = ctx.createLinearGradient(0, my + bh - 30, 0, my + bh + 6); g.addColorStop(0, 'rgba(255,255,255,0)'); g.addColorStop(1, accent ? 'rgba(255,255,255,.24)' : 'rgba(255,255,255,.07)');
    ctx.fillStyle = g; ctx.beginPath(); ctx.roundRect(mx, my + bh - 30, bw, 36, br); ctx.fill();
    g = ctx.createRadialGradient(SW / 2, SH / 2, 24, SW / 2, SH / 2, SW * .44); g.addColorStop(0, accent ? 'rgba(0,0,0,.04)' : 'rgba(0,0,0,.1)'); g.addColorStop(1, 'rgba(0,0,0,0)');
    ctx.fillStyle = g; ctx.fillRect(0, 0, SW, SH);
    // silk-screened legends (DM Mono)
    const text = accent ? C.accentLegend : C.legend, sub = accent ? C.accentSub : C.sub;
    ctx.textAlign = 'center'; ctx.fillStyle = text;
    const F = (w, px) => `${w} ${px}px ${C.font}`;
    const lab = d.label || '';
    if (d.nat || d.id === 'enter' || d.id === 'esc') {
      const word = (window.TR || (w => w))(d.nat || d.sub).toUpperCase();
      // fit the word to the keycap's visible top face (narrower than the texture), measured
      let px = d.id === 'enter' ? 112 : 104; ctx.font = F(500, px);
      while (px > 30 && ctx.measureText(word).width > bw * .66) { px -= 2; ctx.font = F(500, px); } centered(ctx, word, SW / 2, SH * .6);
      ctx.font = F(500, 58); ctx.fillStyle = sub; ctx.textAlign = 'left';
      centered(ctx, d.id === 'enter' ? '↵' : lab, mx + 18, SH * .3); ctx.textAlign = 'center';
    } else if (/^F\d+$/.test(lab)) {
      ctx.font = F(400, 52); centered(ctx, lab, SW / 2, SH * .5);
    } else if (d.sub) {
      ctx.font = F(500, 64); centered(ctx, lab, SW / 2, SH * .38);
      ctx.font = F(400, 50); ctx.fillStyle = sub; centered(ctx, d.sub, SW / 2, SH * .66);
    } else if (['shift', 'ctrl', 'alt', 'caps', 'backspace', 'tab', 'del', 'fn', 'win'].includes(lab)) {
      ctx.font = F(400, lab.length > 5 ? 44 : 48); centered(ctx, lab, SW / 2, SH * .5);
    } else if (lab) {
      ctx.font = F(500, 74); centered(ctx, lab, SW / 2, SH * (d.bump ? .44 : .5));
      if (d.bump) { ctx.beginPath(); ctx.roundRect(SW / 2 - 26, SH * .73 - 4.5, 52, 9, 4.5); ctx.fill(); }
    }
    const tex = new THREE.CanvasTexture(cv);
    tex.colorSpace = THREE.SRGBColorSpace; tex.anisotropy = 8; tex.minFilter = THREE.LinearMipmapLinearFilter;
    return tex;
  }

  // ── Sculpted keycap geometry (getSculptedKeycapGeometry, unchanged) ──
  function roundedRectPoints(w, d, r, count) {
    const shape = new THREE.Shape(), hw = w / 2, hd = d / 2, kr = Math.max(.002, Math.min(r, hw * .45, hd * .45));
    shape.moveTo(-hw + kr, hd); shape.lineTo(hw - kr, hd); shape.quadraticCurveTo(hw, hd, hw, hd - kr);
    shape.lineTo(hw, -hd + kr); shape.quadraticCurveTo(hw, -hd, hw - kr, -hd); shape.lineTo(-hw + kr, -hd);
    shape.quadraticCurveTo(-hw, -hd, -hw, -hd + kr); shape.lineTo(-hw, hd - kr); shape.quadraticCurveTo(-hw, hd, -hw + kr, hd);
    return shape.getSpacedPoints(count).slice(0, count);
  }
  const geoCache = new Map();
  function keycapGeometry(w, d) {
    const k = w.toFixed(4) + '_' + d.toFixed(4); if (geoCache.has(k)) return geoCache.get(k);
    const Hh = .044, isWide = w / d > 1.4, N = Math.max(48, Math.round(48 * ((w + d) / (2 * U))));
    const rings = [
      { y: 0, s: 1, rS: 1 }, { y: Hh * .22, s: .972, rS: 1.02 }, { y: Hh * .5, s: .94, rS: 1.06 }, { y: Hh * .7, s: .906, rS: 1.12 },
      { y: Hh * .88, s: .87, rS: 1.18 }, { y: Hh * .97, s: .838, rS: 1.24 }, { y: Hh, s: .808, rS: 1.28 },
      { y: Hh * .91, s: .748, rS: 1.22 }, { y: Hh * .82, s: .688, rS: 1.15 }, { y: Hh * .77, s: .638, rS: 1.11 },
      { y: Hh * .762, s: .442, rS: 1.07 }, { y: Hh * .752, s: .222, rS: 1.03 },
    ];
    const baseR = isWide ? Math.min(.042, d * .22) : Math.min(.05, w * .24, d * .24);
    const pos = [], uvs = [], idx = [];
    rings.forEach((ring, ri) => {
      const cd = d * ring.s, cw = isWide ? Math.max(w * .4, w - (d - cd)) : w * ring.s;
      const pts = roundedRectPoints(cw, cd, baseR * ring.rS, N);
      for (let i = 0; i < N; i++) {
        const x = pts[i].x, z = pts[i].y; pos.push(x, ring.y, z);
        if (ri >= 4) { const tw = isWide ? w - d * .22 : w * .78, td = d * .78; uvs.push(Math.max(0, Math.min(1, x / tw * .48 + .5)), Math.max(0, Math.min(1, .5 - z / td * .48))); }
        else uvs.push(.02, .02);
      }
    });
    const ci = pos.length / 3; pos.push(0, Hh * .745, 0); uvs.push(.5, .5);
    const bi = pos.length / 3; pos.push(0, 0, 0); uvs.push(.02, .02);
    for (let r = 0; r < rings.length - 1; r++) for (let i = 0; i < N; i++) {
      const n = (i + 1) % N, a1 = r * N + i, a2 = r * N + n, b1 = (r + 1) * N + i, b2 = (r + 1) * N + n;
      idx.push(a1, a2, b1, a2, b2, b1);
    }
    const inner = (rings.length - 1) * N;
    for (let i = 0; i < N; i++) idx.push(inner + i, inner + (i + 1) % N, ci);
    for (let i = 0; i < N; i++) idx.push(bi, (i + 1) % N, i);
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
    geo.setAttribute('uv', new THREE.Float32BufferAttribute(uvs, 2));
    geo.setIndex(idx); geo.computeVertexNormals();
    geoCache.set(k, geo); return geo;
  }

  // ── Keycaps ──
  const keys = new Map(), meshes = [];
  const originX = KW_X1 + WELL_PAD, originZ = KW_Z1 + WELL_PAD, keyBaseY = PLATE_Y + .003;
  for (const d of defs) {
    const mat = new THREE.MeshStandardMaterial({ map: keyTexture(d), roughness: d.a ? .38 : .52, metalness: .03, envMapIntensity: 1 });
    const mesh = new THREE.Mesh(keycapGeometry(d.width, d.depth), mat); mesh.castShadow = true; mesh.receiveShadow = true;
    const grp = new THREE.Group(); grp.position.set(originX + d.col + d.wUnits * U / 2, keyBaseY, originZ + (d.row + .5) * U); grp.add(mesh); root.add(grp);
    const k = { d, grp, cur: 0, down: false, held: false, hover: false, timer: 0 };
    mesh.userData.key = k; keys.set(d.id, k); meshes.push(mesh);
  }

  // ── Materials ──
  const chassisMat = new THREE.MeshStandardMaterial({ color: C.chassis, metalness: .72, roughness: .34, envMapIntensity: 1.2 });
  const plateMat = new THREE.MeshStandardMaterial({ color: C.plate, metalness: .05, roughness: .96 });
  const knobMat = new THREE.MeshStandardMaterial({ color: C.knob, metalness: .38, roughness: .48 });

  // ── Unibody chassis: base slab + top deck with milled key well and display pocket ──
  {
    const baseThick = CHASSIS_THICK - WELL_DEPTH;
    const base = new THREE.Mesh(new RoundedBoxGeometry(CHASSIS_W, baseThick, CHASSIS_D, 6, .012), chassisMat);
    base.position.y = PLATE_Y - baseThick / 2; base.castShadow = base.receiveShadow = true; root.add(base);
    const shape = new THREE.Shape(), hw = CHASSIS_W / 2, hd = CHASSIS_D / 2, r = .036;
    shape.moveTo(-hw + r, -hd); shape.lineTo(hw - r, -hd); shape.quadraticCurveTo(hw, -hd, hw, -hd + r); shape.lineTo(hw, hd - r);
    shape.quadraticCurveTo(hw, hd, hw - r, hd); shape.lineTo(-hw + r, hd); shape.quadraticCurveTo(-hw, hd, -hw, hd - r); shape.lineTo(-hw, -hd + r); shape.quadraticCurveTo(-hw, -hd, -hw + r, -hd);
    const yTop = KEY_WELL_D / 2, ySplit = KEY_WELL_D / 2 - WELL_PAD - 5 * U, yBot = -KEY_WELL_D / 2, kr = .016;
    const kw = new THREE.Path();
    kw.moveTo(KW_X1 + kr, yTop); kw.lineTo(KW_MAIN_X2 - kr, yTop); kw.quadraticCurveTo(KW_MAIN_X2, yTop, KW_MAIN_X2, yTop - kr);
    kw.lineTo(KW_MAIN_X2, ySplit + kr); kw.quadraticCurveTo(KW_MAIN_X2, ySplit, KW_MAIN_X2 + kr, ySplit); kw.lineTo(KW_EXT_X2 - kr, ySplit);
    kw.quadraticCurveTo(KW_EXT_X2, ySplit, KW_EXT_X2, ySplit - kr); kw.lineTo(KW_EXT_X2, yBot + kr); kw.quadraticCurveTo(KW_EXT_X2, yBot, KW_EXT_X2 - kr, yBot);
    kw.lineTo(KW_X1 + kr, yBot); kw.quadraticCurveTo(KW_X1, yBot, KW_X1, yBot + kr); kw.lineTo(KW_X1, yTop - kr); kw.quadraticCurveTo(KW_X1, yTop, KW_X1 + kr, yTop);
    shape.holes.push(kw);
    const dp = new THREE.Path(), dw = DISPLAY_W + .01, dh = DISPLAY_H + .01, dhw = dw / 2, dhh = dh / 2, cx = DISPLAY_X, cy = -DISPLAY_Z, rd = dw * .2611, kk = rd * .5522847498;
    dp.moveTo(cx - dhw + rd, cy + dhh); dp.lineTo(cx + dhw - rd, cy + dhh); dp.bezierCurveTo(cx + dhw - rd + kk, cy + dhh, cx + dhw, cy + dhh - rd + kk, cx + dhw, cy + dhh - rd);
    dp.lineTo(cx + dhw, cy - dhh + rd); dp.bezierCurveTo(cx + dhw, cy - dhh + rd - kk, cx + dhw - rd + kk, cy - dhh, cx + dhw - rd, cy - dhh);
    dp.lineTo(cx - dhw + rd, cy - dhh); dp.bezierCurveTo(cx - dhw + rd - kk, cy - dhh, cx - dhw, cy - dhh + rd - kk, cx - dhw, cy - dhh + rd);
    dp.lineTo(cx - dhw, cy + dhh - rd); dp.bezierCurveTo(cx - dhw, cy + dhh - rd + kk, cx - dhw + rd - kk, cy + dhh, cx - dhw + rd, cy + dhh);
    shape.holes.push(dp);
    const deck = new THREE.Mesh(new THREE.ExtrudeGeometry(shape, { depth: WELL_DEPTH, bevelEnabled: true, bevelThickness: .003, bevelSize: .003, bevelSegments: 3, curveSegments: 24 }), chassisMat);
    deck.position.y = CHASSIS_TOP_Y; deck.rotation.x = -Math.PI / 2; deck.castShadow = deck.receiveShadow = true; root.add(deck);
    const kcx = (KW_X1 + KW_MAIN_X2) / 2;
    const plate = new THREE.Mesh(new RoundedBoxGeometry(KEY_WELL_MAIN_W + .008, .005, KEY_WELL_D + .008, 4, .002), plateMat);
    plate.position.set(kcx, PLATE_Y + .002, 0); plate.receiveShadow = true; root.add(plate);
    const ao = new THREE.Mesh(new THREE.PlaneGeometry(KEY_WELL_MAIN_W + .014, KEY_WELL_D + .014), new THREE.MeshBasicMaterial({ color: '#000', opacity: .32, transparent: true, depthWrite: false }));
    ao.rotation.x = -Math.PI / 2; ao.position.set(kcx, PLATE_Y + .005, 0); root.add(ao);
    const plate2 = new THREE.Mesh(new RoundedBoxGeometry(U + WELL_PAD * 2 + .006, .005, U + WELL_PAD * 2 + .006, 3, .002), plateMat);
    plate2.position.set(RIGHT_PANEL_X, PLATE_Y + .002, KW_Z1 + WELL_PAD + 5.5 * U); root.add(plate2);
    const footMat = new THREE.MeshStandardMaterial({ color: '#141517', roughness: .94, metalness: .05 });
    for (const [fx, fz] of [[-.38, .38], [.38, .38], [-.38, -.38], [.38, -.38]]) {
      const f = new THREE.Mesh(new RoundedBoxGeometry(.18, .006, .032, 3, .003), footMat); f.position.set(CHASSIS_W * fx, CHASSIS_TOP_Y - CHASSIS_THICK - .002, CHASSIS_D * fz); root.add(f);
    }
    // power switch, copper
    const sw = new THREE.Mesh(new RoundedBoxGeometry(.046, .02, .022, 3, .006), new THREE.MeshStandardMaterial({ color: C.accent, roughness: .4, metalness: .1 }));
    sw.position.set(KW_X1 + .26, CHASSIS_TOP_Y + .002, -CHASSIS_D / 2 - .006); sw.castShadow = true; root.add(sw);
  }

  // ── Machined knobs ──
  {
    const R_BASE = .082, R_RIM = .076, R_TOP = .072, H_BODY = .052, H_RIM = .048, NP = 96, NS = 12, NB = 6;
    const pos = [], idx = [];
    pos.push(0, H_BODY, 0);
    const top0 = pos.length / 3; for (let j = 0; j < NP; j++) { const ph = j / NP * Math.PI * 2; pos.push(R_TOP * Math.cos(ph), H_BODY, R_TOP * Math.sin(ph)); }
    for (let j = 0; j < NP; j++) idx.push(0, top0 + (j + 1) % NP, top0 + j);
    const bev0 = pos.length / 3;
    for (let b = 0; b <= NB; b++) { const a = b / NB * Math.PI / 2, r = R_TOP + (R_RIM - R_TOP) * Math.sin(a), y = H_BODY - (H_BODY - H_RIM) * (1 - Math.cos(a)); for (let j = 0; j < NP; j++) { const ph = j / NP * Math.PI * 2; pos.push(r * Math.cos(ph), y, r * Math.sin(ph)); } }
    for (let b = 0; b < NB; b++) for (let j = 0; j < NP; j++) { const A = bev0 + b * NP, B = A + NP, j1 = (j + 1) % NP; idx.push(A + j, A + j1, B + j, A + j1, B + j1, B + j); }
    const side0 = pos.length / 3;
    for (let k = 0; k <= NS; k++) { const t = k / NS, y = H_RIM * (1 - t), r = R_RIM + (R_BASE - R_RIM) * t; for (let j = 0; j < NP; j++) { const ph = j / NP * Math.PI * 2; pos.push(r * Math.cos(ph), y, r * Math.sin(ph)); } }
    for (let k = 0; k < NS; k++) for (let j = 0; j < NP; j++) { const A = side0 + k * NP, B = A + NP, j1 = (j + 1) % NP; idx.push(A + j, A + j1, B + j, A + j1, B + j1, B + j); }
    const bot0 = pos.length / 3; for (let j = 0; j < NP; j++) { const ph = j / NP * Math.PI * 2; pos.push(R_BASE * Math.cos(ph), 0, R_BASE * Math.sin(ph)); }
    const bc = pos.length / 3; pos.push(0, 0, 0); for (let j = 0; j < NP; j++) idx.push(bc, bot0 + j, bot0 + (j + 1) % NP);
    const geo = new THREE.BufferGeometry(); geo.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3)); geo.setIndex(idx); geo.computeVertexNormals();
    const sc = document.createElement('canvas'); sc.width = sc.height = 128; const sx = sc.getContext('2d');
    const rg = sx.createRadialGradient(64, 64, 18, 64, 64, 62); rg.addColorStop(0, 'rgba(0,0,0,.95)'); rg.addColorStop(.35, 'rgba(0,0,0,.7)'); rg.addColorStop(.65, 'rgba(0,0,0,.25)'); rg.addColorStop(1, 'rgba(0,0,0,0)');
    sx.fillStyle = rg; sx.fillRect(0, 0, 128, 128);
    const shadowTex = new THREE.CanvasTexture(sc);
    for (const [z, ang] of [[KNOBS.topZ, KNOBS.angTop], [KNOBS.bottomZ, KNOBS.angBottom]]) {
      const g = new THREE.Group(); g.position.set(DISPLAY_X, CHASSIS_TOP_Y, z);
      const sh = new THREE.Mesh(new THREE.PlaneGeometry(R_BASE * 2.5, R_BASE * 2.5), new THREE.MeshBasicMaterial({ map: shadowTex, transparent: true, opacity: .82, depthWrite: false }));
      sh.rotation.x = -Math.PI / 2; sh.position.set(.003, .0008, .005); g.add(sh);
      const spin = new THREE.Group(); spin.position.y = .0005; spin.rotation.y = ang;
      const body = new THREE.Mesh(geo, knobMat); body.castShadow = body.receiveShadow = true; spin.add(body);
      const fin = new THREE.Mesh(new RoundedBoxGeometry(KNOBS.finW, KNOBS.finH, KNOBS.finL, 4, .0035), knobMat);
      fin.position.y = .052 + KNOBS.finH / 2 - .001; fin.castShadow = true; spin.add(fin);
      g.add(spin); root.add(g);
    }
  }

  // ── Smart display: the day on the object's screen, and the cases grown here ──
  const dispCv = document.createElement('canvas'); dispCv.width = 360; dispCv.height = 1186;
  const dispTex = new THREE.CanvasTexture(dispCv); dispTex.colorSpace = THREE.SRGBColorSpace; dispTex.anisotropy = 8;
  function drawDisplay() {
    const ctx = dispCv.getContext('2d'), Wd = 360, Hd = 1186, RC = Math.round(Wd * .2611);
    const n = window.HumWorld ? window.HumWorld.count() : 0;
    const now = new Date(), time = now.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' }), date = now.toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric', month: 'short' });
    ctx.save(); ctx.clearRect(0, 0, Wd, Hd);
    ctx.beginPath(); ctx.roundRect(0, 0, Wd, Hd, RC); ctx.clip();
    ctx.fillStyle = C.bezel; ctx.fillRect(0, 0, Wd, Hd);
    const sx = 18, sy = 18, sw = Wd - 36, sh = Hd - 36;
    ctx.save(); ctx.beginPath(); ctx.roundRect(sx, sy, sw, sh, RC - 14); ctx.clip();
    const g = ctx.createLinearGradient(0, sy, 0, sy + sh); g.addColorStop(0, '#1d1814'); g.addColorStop(.55, '#121110'); g.addColorStop(1, '#0c0c0b');
    ctx.fillStyle = g; ctx.fillRect(sx, sy, sw, sh);
    ctx.fillStyle = 'rgba(255,255,255,.025)'; for (let y = sy; y < sy + sh; y += 4) ctx.fillRect(sx, y, sw, 1); for (let x = sx; x < sx + sw; x += 4) ctx.fillRect(x, sy, 1, sh);
    const cx0 = sx + 22, cy0 = sy + 22, cw = sw - 44, ch = 182;
    ctx.fillStyle = '#050505'; ctx.beginPath(); ctx.roundRect(cx0, cy0, cw, ch, 56); ctx.fill();
    ctx.lineWidth = 2; ctx.strokeStyle = 'rgba(231,138,83,.85)'; ctx.stroke();
    ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
    ctx.fillStyle = '#f3efe8'; ctx.font = `500 66px ${C.font}`; ctx.fillText(time, Wd / 2, cy0 + 70);
    ctx.fillStyle = '#e78a53'; ctx.font = `500 26px ${C.font}`; ctx.fillText(date, Wd / 2, cy0 + 128);
    ctx.fillStyle = '#8f8c86'; ctx.font = `500 30px ${C.font}`; ctx.fillText('CASES', Wd / 2, cy0 + ch + 120);
    ctx.fillStyle = n ? '#e78a53' : '#4a4743'; ctx.font = `500 150px ${C.font}`; ctx.fillText(String(n), Wd / 2, cy0 + ch + 250);
    const gl = ctx.createLinearGradient(sx, sy, sx + sw * .85, sy + sh * .42); gl.addColorStop(0, 'rgba(255,255,255,.1)'); gl.addColorStop(.3, 'rgba(255,255,255,.02)'); gl.addColorStop(.6, 'rgba(255,255,255,0)');
    ctx.fillStyle = gl; ctx.fillRect(sx, sy, sw, sh);
    ctx.restore(); ctx.restore();
    dispTex.needsUpdate = true;
  }
  drawDisplay();
  {
    const g = new THREE.Group(); g.position.set(DISPLAY_X, CHASSIS_TOP_Y, DISPLAY_Z);
    const floor = new THREE.Mesh(new THREE.PlaneGeometry(DISPLAY_W, DISPLAY_H), new THREE.MeshBasicMaterial({ color: '#050608' }));
    floor.rotation.x = -Math.PI / 2; floor.position.y = -WELL_DEPTH + .002; g.add(floor);
    const glass = new THREE.Mesh(new THREE.PlaneGeometry(DISPLAY_W, DISPLAY_H), new THREE.MeshBasicMaterial({ map: dispTex, transparent: true, alphaTest: .02, toneMapped: false }));
    glass.rotation.x = -Math.PI / 2; glass.position.y = .001; g.add(glass); root.add(g);
  }

  // ── Braided cable, copper ──
  {
    const deskY = CHASSIS_TOP_Y - CHASSIS_THICK + .008, portY = CHASSIS_TOP_Y * .5;
    const g = new THREE.Group(); g.position.set(DISPLAY_X, 0, -CHASSIS_D / 2);
    const plug = new THREE.Mesh(new RoundedBoxGeometry(.04, .02, .062, 3, .0025), new THREE.MeshStandardMaterial({ color: '#141517', roughness: .46, metalness: .45 }));
    plug.position.set(0, portY, -.031); g.add(plug);
    const boot = new THREE.Mesh(new THREE.CylinderGeometry(.0088, .0088, .026, 24), new THREE.MeshStandardMaterial({ color: '#161719', roughness: .82, metalness: .08 }));
    boot.rotation.x = Math.PI / 2; boot.position.set(0, portY, -.075); g.add(boot);
    const curve = new THREE.CatmullRomCurve3([[0, portY, 0], [.006, portY * .4, -.09], [.016, deskY, -.22], [-.018, deskY, -.52], [.024, deskY, -.88], [.01, deskY, -1.35]].map(v => new THREE.Vector3(...v)));
    const cable = new THREE.Mesh(new THREE.TubeGeometry(curve, 36, .014, 12, false), new THREE.MeshStandardMaterial({ color: '#b8612f', roughness: .52, metalness: .05 }));
    cable.position.z = -.082; cable.castShadow = true; g.add(cable); root.add(g);
  }

  // ── The desk: a dark surface that catches the shadow and melts into the night ──
  {
    const dc = document.createElement('canvas'); dc.width = dc.height = 256; const dx = dc.getContext('2d');
    const rg = dx.createRadialGradient(128, 128, 20, 128, 128, 128); rg.addColorStop(0, '#fff'); rg.addColorStop(.55, '#777'); rg.addColorStop(1, '#000');
    dx.fillStyle = rg; dx.fillRect(0, 0, 256, 256);
    const alpha = new THREE.CanvasTexture(dc);
    const y = CHASSIS_TOP_Y - CHASSIS_THICK - .006;
    const deskMesh = new THREE.Mesh(new THREE.PlaneGeometry(8, 5), new THREE.MeshStandardMaterial({ color: '#1b140e', roughness: .82, metalness: 0, transparent: true, alphaMap: alpha, depthWrite: false }));
    deskMesh.rotation.x = -Math.PI / 2; deskMesh.position.set(0, y - .001, .3); deskMesh.receiveShadow = true; scene.add(deskMesh);
    const shadow = new THREE.Mesh(new THREE.PlaneGeometry(40, 40), new THREE.ShadowMaterial({ opacity: .55 }));
    shadow.rotation.x = -Math.PI / 2; shadow.position.y = y; shadow.receiveShadow = true; scene.add(shadow);
  }

  // ── Camera that fits the keyboard (ResponsiveCamera, lower and oblique: a keyboard lying on a desk) ──
  function fit() {
    const w = desk.clientWidth, h = desk.clientHeight; if (!w || !h) return;
    renderer.setSize(w, h, false); camera.aspect = w / h;
    const v = camera.fov * Math.PI / 180;
    const dW = CHASSIS_W * 1.06 / (2 * Math.tan(v / 2) * camera.aspect), dH = CHASSIS_D * 1.12 / (2 * Math.tan(v / 2));
    const dist = Math.max(dW, dH);
    const dir = new THREE.Vector3(0, .74, .67).normalize();
    camera.position.copy(dir.multiplyScalar(dist)); camera.lookAt(0, -.06, .1); camera.updateProjectionMatrix();
    renderer.shadowMap.needsUpdate = true; kick();
  }

  // ── Key travel (useFrame of the original) ──
  function stepKey(k, dt) {
    const target = k.down || k.held ? -C.travel : k.hover ? .0015 : 0, diff = target - k.cur;
    if (Math.abs(diff) > .00005 && !reduce) { k.cur += diff * Math.min(1, dt * 32); k.grp.position.y = keyBaseY + k.cur; return true; }
    if (k.cur !== target) { k.cur = target; k.grp.position.y = keyBaseY + target; return true; }
    return false;
  }
  let raf = 0, last = 0, visible = false;
  function frame(t) {
    raf = 0; const dt = last ? Math.min(.05, (t - last) / 1000) : .016; last = t;
    let moving = false; for (const k of keys.values()) moving = stepKey(k, dt) || moving;
    if (moving) renderer.shadowMap.needsUpdate = true;
    renderer.render(scene, camera);
    if (moving && visible) raf = requestAnimationFrame(frame); else last = 0;
  }
  function kick() { if (!raf && visible && !document.hidden) raf = requestAnimationFrame(frame); }
  new IntersectionObserver(es => { visible = es[es.length - 1].isIntersecting; if (visible) kick(); }, { threshold: .05, rootMargin: '120px' }).observe(desk);
  document.addEventListener('visibilitychange', kick);
  new ResizeObserver(fit).observe(desk);

  // ── Pointer: raycast, hover lift, press for 160 ms or until release ──
  const ray = new THREE.Raycaster(), ndc = new THREE.Vector2();
  function hit(e) {
    const r = kbCanvas.getBoundingClientRect();
    ndc.set((e.clientX - r.left) / r.width * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
    ray.setFromCamera(ndc, camera);
    const h = ray.intersectObjects(meshes, false)[0];
    return h ? h.object.userData.key : null;
  }
  let hovered = null;
  kbCanvas.addEventListener('pointermove', e => {
    const k = hit(e);
    if (k !== hovered) { if (hovered) hovered.hover = false; hovered = k; if (k) k.hover = true; kbCanvas.style.cursor = k ? 'pointer' : ''; kick(); }
  });
  kbCanvas.addEventListener('pointerleave', () => { if (hovered) { hovered.hover = false; hovered = null; kick(); } });
  kbCanvas.addEventListener('pointerdown', e => {
    const k = hit(e); if (!k) return;
    e.preventDefault();
    k.down = true; clearTimeout(k.timer); k.timer = setTimeout(() => { k.down = false; kick(); }, 160); kick();
    const a = ACTION[k.d.id]; if (a) window.HumPlay.act(a);
  });
  addEventListener('pointerup', () => { for (const k of keys.values()) if (k.down) { k.down = false; } kick(); });

  window.KB3D = {
    down(id) { const k = keys.get(id); if (k) { k.held = true; kick(); } },
    up(id) { const k = keys.get(id); if (k) { k.held = false; kick(); } },
    releaseAll() { for (const k of keys.values()) k.held = false; kick(); },
    refresh() { drawDisplay(); renderer.shadowMap.needsUpdate = true; kick(); },
  };
  setInterval(() => { if (visible) { drawDisplay(); kick(); } }, 30000);

  // the shaders compile in the background where the browser can (KHR_parallel_shader_compile):
  // compiled on the first render instead, they froze the page for a moment while it scrolled
  try { await renderer.compileAsync(scene, camera); } catch { /* the first render compiles them */ }
  visible = desk.getBoundingClientRect().top < innerHeight + 120;
  fit();
  renderer.shadowMap.needsUpdate = true; renderer.render(scene, camera);
  requestAnimationFrame(() => kbCanvas.classList.add('ready'));
}
