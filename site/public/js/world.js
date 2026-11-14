/* ───────────────────────────────────────────────────────────────────────────
   The night maple. Adapted from 21st.dev "Generative Tree" (mengto): same branching
   rules (2-3 children, progressive pruning, overlapping growth at 65 %), same
   multi-stroke painterly branches, ambient canopy glow, drifting particle sprites,
   multi-frequency sway, mouse wind and shake. Changes: seeded (the same tree every
   visit, so leaves can be read), growth driven by the page instead of a loop,
   copper palette, maple leaves as soft dabs, agent sub-trees in cool light,
   falling leaves that gather at the foot, blossoms and a sapling for the new idea.
   ─────────────────────────────────────────────────────────────────────────── */
(() => {
  const cv = document.getElementById('world');
  const ctx = cv.getContext('2d');
  const RM = matchMedia('(prefers-reduced-motion: reduce)');
  let reduce = RM.matches;
  let W = 0, H = 0, DPR = 1, small = false;

  const lerp = (a, b, t) => a + (b - a) * t;
  const clamp01 = v => v < 0 ? 0 : v > 1 ? 1 : v;
  const easeOutCubic = t => 1 - Math.pow(1 - t, 3);
  const smooth = (a, b, t) => { t = clamp01((t - a) / (b - a)); return t * t * (3 - 2 * t); };

  let R = Math.random;
  const rand = (lo, hi) => R() * (hi - lo) + lo;

  // ── Palettes: copper from bark to leaf tips; agents in cool light ──
  const COPPER = [[58, 30, 13], [98, 50, 22], [146, 76, 36], [192, 104, 56], [222, 134, 84], [236, 170, 122]];
  const COOL = {
    1: [[34, 58, 56], [46, 80, 76], [64, 112, 104], [92, 148, 134], [124, 182, 166], [164, 210, 196]],   // Claude
    2: [[32, 44, 70], [42, 58, 92], [58, 82, 124], [84, 112, 160], [116, 146, 198], [158, 182, 226]],    // Codex
  };
  function pal(P, t) {
    const idx = t * (P.length - 1), i0 = Math.floor(idx), i1 = Math.min(P.length - 1, i0 + 1), f = idx - i0;
    return [lerp(P[i0][0], P[i1][0], f), lerp(P[i0][1], P[i1][1], f), lerp(P[i0][2], P[i1][2], f)];
  }

  // ── Sprites (pre-rendered soft dabs, as the original's particle sprite) ──
  function dab(r, g, b, lobes) {
    const c = document.createElement('canvas'); c.width = c.height = 48;
    const x = c.getContext('2d');
    for (const [cx, cy, rad, a] of lobes) {
      const gr = x.createRadialGradient(cx, cy, 0, cx, cy, rad);
      gr.addColorStop(0, `rgba(${r},${g},${b},${a})`);
      gr.addColorStop(.42, `rgba(${r},${g},${b},${a * .5})`);
      gr.addColorStop(1, `rgba(${r},${g},${b},0)`);
      x.fillStyle = gr; x.fillRect(0, 0, 48, 48);
    }
    return c;
  }
  const leafLobes = [[21, 26, 17, 1], [28, 21, 15, .85], [24, 24, 9, .6]];
  const SPR = {
    copper: dab(231, 138, 83, leafLobes), burnt: dab(196, 72, 22, leafLobes), amber: dab(222, 160, 92, leafLobes),
    rust: dab(158, 62, 32, leafLobes), dry: dab(132, 78, 44, leafLobes),
    claude: dab(122, 192, 172, leafLobes), codex: dab(132, 164, 222, leafLobes),
    cream: dab(250, 232, 204, [[24, 24, 16, 1], [24, 24, 6, .9]]), fresh: dab(244, 196, 126, leafLobes),
    glow: dab(236, 186, 130, [[24, 24, 24, 1]]), mote: dab(230, 200, 155, [[24, 24, 24, 1]]),
  };
  const COPPER_SPR = ['copper', 'copper', 'amber', 'burnt', 'burnt', 'rust', 'copper', 'amber'];

  let LABELS = [];

  // ── The maple: painted frames rendered in Blender, chosen by the story's state ──
  // Every frame is the same tree at another moment (maple/maple-data.js lists them). The twig positions
  // exported with them pin the labels, the agents' light and the visitor's cases on the painted leaves.
  const youthScale = g => { const t = Math.min(1, g / .85); return .42 + .58 * t * t * (3 - 2 * t); };
  const MAPLE = (() => {
    let X = ctx;
    const D = window.MAPLE_DATA;
    // frames load when their season is near: the small ones (maple/px) for the pixel tree, the full ones for the painted test
    const img = { px: {}, hd: {} };
    let loaded = 0;   // a frame that arrives must redraw a scene drawn without it
    const load = (f, hd) => {
      const c = hd ? img.hd : img.px;
      if (c[f]) return c[f];
      if (hd) return null;
      // AVIF is a quarter lighter; a browser that cannot decode it gets the WebP
      const im = new Image(); im.decoding = 'async'; im.onload = () => { loaded++; kick(); };
      im.onerror = () => { im.onerror = null; im.src = '/maple/px/' + f + '.webp'; };
      im.src = '/maple/px/' + f + '.avif'; c[f] = im;
      return im;
    };
    const preload = (list, hd) => { for (const [, f] of list) load(f, hd); };
    const NODES = new Map(), KIDS = new Map();
    for (const [id, u, v, bn, rad, tip, x, z, par] of D.nodes) NODES.set(id, { id, u, v, bn, rad, tip, x, z, par });
    for (const n of NODES.values()) if (NODES.has(n.par)) { if (!KIDS.has(n.par)) KIDS.set(n.par, []); KIDS.get(n.par).push(n); }
    const sub = n => { const out = [n]; for (let i = 0; i < out.length; i++) out.push(...(KIDS.get(out[i].id) || [])); return out; };
    // a young tree is drawn smaller (the same law as the Blender script)
    const youth = g => { const t = Math.min(1, g / .85); return .42 + .58 * t * t * (3 - 2 * t); };
    const view = { bx: 0, by: 0, k: 1, g: 1, sh: 0, alpha: 0, p: null };
    const at = n => {
      const s = youth(view.g), y = view.by + (n.v - D.base[1]) * view.k * s;
      return [view.bx + (n.u - D.base[0]) * view.k * s + view.sh * (y - view.by), y];
    };
    const visible = n => n.bn <= view.g + 1e-6 && view.alpha > .05;

    // two agent limbs at the crown's edges (Claude left, Codex right): about a twentieth of the twigs each
    const limb = side => {
      let best = null, bs = 1e9;
      for (const n of NODES.values()) {
        if (n.x * side < 1.3 || n.rad < .01 || n.rad > .03) continue;
        const sc = Math.abs(sub(n).length / NODES.size - .05) * 10 - Math.abs(n.x) * .25;
        if (sc < bs) { bs = sc; best = n; }
      }
      return best ? sub(best) : [];
    };
    const AGENT = { 1: limb(-1), 2: limb(1) };
    const AGENT_TIPS = { 1: AGENT[1].filter(n => n.tip), 2: AGENT[2].filter(n => n.tip) };
    const agentOf = new Map(); for (const a of [1, 2]) for (const n of AGENT[a]) agentOf.set(n.id, a);
    const glowSpr = a => { const c = pal(COOL[a], .62); return dab(c[0] | 0, c[1] | 0, c[2] | 0, [[24, 24, 24, 1]]); };
    const GLOW = { 1: glowSpr(1), 2: glowSpr(2) };

    // tips of the full tree, highest first for each agent
    const tips = [...NODES.values()].filter(n => n.tip);
    const used = new Set();
    const tipNear = (x, z, pool = tips) => {
      let best = null, bs = 1e9;
      for (const n of pool) { if (used.has(n.id)) continue; const d = (n.x - x) ** 2 + (n.z - z) ** 2; if (d < bs) { bs = d; best = n; } }
      used.add(best.id); return best;
    };
    const topOf = list => { const t = list.filter(n => n.tip && !used.has(n.id)).sort((a, b) => b.z - a.z)[Math.min(2, list.length - 1)] || list[0]; used.add(t.id); return t; };

    // label leaves and pseudo-branches (b.tx, b.ty, b.vis), refreshed every frame
    const leaves = [], branches = new Map();
    const leaf = (n, opts = {}) => { const l = Object.assign({ n, x: 0, y: 0, on: false, b: null }, opts); leaves.push(l); return l; };
    const branch = n => { if (!branches.has(n.id)) branches.set(n.id, { n, tx: 0, ty: 0, vis: false, rx: n.x, ry: -n.z, agent: agentOf.get(n.id) || 0 }); return branches.get(n.id); };
    const tipsAt = g => [...NODES.values()].filter(n => n.bn <= g && !(KIDS.get(n.id) || []).some(c => c.bn <= g)).map(branch);

    function frameAt(list, t) {
      if (t <= list[0][0]) return [list[0][1], null, 0];
      for (let i = 0; i < list.length - 1; i++) if (t <= list[i + 1][0]) return [list[i][1], list[i + 1][1], (t - list[i][0]) / (list[i + 1][0] - list[i][0])];
      return [list[list.length - 1][1], null, 0];
    }
    function seqFor(p) {
      if (p.B > .002) return [D.frames.spring, p.B];
      if (p.F > .002) return [D.frames.fall, p.F];
      if (p.A > .002 && p.G > .995) return [D.frames.autumn, p.A];
      return [D.frames.grow, p.G];
    }
    const blit = (f, a) => {
      const im = load(f, X === ctx); if (!im || !im.complete || !im.naturalWidth || a < .003) return;
      const [cx, cy, cw, ch] = D.crop;
      X.globalAlpha = a;
      X.drawImage(im, view.bx + (cx - D.base[0]) * view.k, view.by + (cy - D.base[1]) * view.k, cw * view.k, ch * view.k);
    };

    function draw(bx, by, S, p, time, target = ctx) {
      X = target;
      Object.assign(view, { bx, by, k: 3.5 * S / D.crownW, g: p.G, alpha: p.alpha, p });
      // wind: the crown leans a little, the foot stays put
      view.sh = reduce ? 0 : (Math.sin(time * .0007) * .006 + Math.sin(time * .0017 + 1) * .003 + wind * .02 + shake * Math.sin(time * .03) * .02);
      const fade = clamp01(p.G / .04);
      const [list, t] = seqFor(p), [f0, f1, m] = frameAt(list, t);
      // the season after this one too, so scrolling on never waits for a frame
      preload(list, X === ctx); preload(list === D.frames.grow ? D.frames.autumn : list === D.frames.autumn ? D.frames.fall : D.frames.spring, X === ctx);
      X.save();
      X.transform(1, 0, view.sh, 1, -view.sh * by, 0);
      blit(f0, fade); if (f1) blit(f1, fade * m);
      X.globalAlpha = 1;
      // the agents' limbs in cool light, the rest dimmed when the page is about them
      const tint = p.cool * p.A * .42 * (1 - .85 * p.F * (1 - p.B)), dim = .42 * p.focus;
      if (tint > .02 || dim > .02) {
        X.globalCompositeOperation = 'source-atop';
        if (dim > .02) { X.fillStyle = `rgba(4,3,2,${dim})`; X.fillRect(bx - 3 * S, by - 4.5 * S, 6 * S, 5.5 * S); }
        if (tint > .02) for (const a of [1, 2]) {
          // only on their limb's tips, wide enough to merge into a patch: cooler leaves, not scattered specks
          const r = .055 * S * youth(view.g);
          X.globalAlpha = Math.min(1, tint * (dim > .02 ? 2 : 1.6));
          for (let i = 0; i < AGENT_TIPS[a].length; i++) {
            const n = AGENT_TIPS[a][i]; if (!visible(n)) continue;
            const [x, y] = at(n); X.drawImage(GLOW[a], x - r, y - r, r * 2, r * 2);
          }
        }
        X.globalCompositeOperation = 'source-over'; X.globalAlpha = 1;
      }
      X.restore();
      // a clump of earth at the foot of the idea on the desk: planted, not floating
      if (p.pl > .02) {
        const r = .2 * S, a = p.pl * p.alpha;
        const e = X.createRadialGradient(bx, by, 0, bx, by, r);
        e.addColorStop(0, `rgba(70,44,26,${a})`); e.addColorStop(.6, `rgba(52,32,19,${.9 * a})`); e.addColorStop(1, 'rgba(40,24,14,0)');
        X.save(); X.translate(bx, by); X.scale(1, .32); X.translate(-bx, -by); X.fillStyle = e; X.fillRect(bx - r, by - r, 2 * r, 2 * r); X.restore();
        for (let j = 0; j < 7; j++) {
          const sz = .035 * S, x = bx + (((j * 37) % 13) / 13 - .5) * r * 1.5, y = by + (((j * 53) % 7) / 7 - .55) * r * .25;
          X.globalAlpha = .8 * a; X.drawImage(SPR[COPPER_SPR[j % COPPER_SPR.length]], x - sz, y - sz, sz * 2, sz * 2);
        }
        X.globalAlpha = 1;
      }
      // leaves on their way down, while the crown empties
      if (p.F > .01 && p.F < .99 && !p.B) {
        const n = Math.round((small ? 18 : 34) * Math.sin(Math.PI * p.F));
        for (let j = 0; j < n; j++) {
          const src = tips[(j * 977) % tips.length]; if (!visible(src)) continue;
          const [x0, y0] = at(src), ph = (time * .00011 * (1 + (j % 5) * .17) + j * .137) % 1;
          const x = x0 + Math.sin(ph * 7 + j) * .12 * S + ph * .25 * S * ((j % 2) * 2 - 1), y = lerp(y0, by - .02 * S, ph);
          const sz = .045 * S; X.globalAlpha = .85 * p.alpha * (ph < .1 ? ph / .1 : 1 - Math.max(0, ph - .85) / .15);
          X.drawImage(SPR[COPPER_SPR[j % COPPER_SPR.length]], x - sz, y - sz, sz * 2, sz * 2);
        }
        X.globalAlpha = 1;
      }
      for (const b of branches.values()) { [b.tx, b.ty] = at(b.n); b.vis = visible(b.n); }
      for (const l of leaves) {
        if (l.ground) { l.x = bx + l.ground[0] * view.k; l.y = by + l.ground[1] * view.k; l.on = p.alpha > .3 && p.F > .6; continue; }
        [l.x, l.y] = at(l.n); l.on = visible(l.n) && p.alpha > .3 && (!l.spring || p.B > .3);
      }
    }
    // where the frames land on the page, with room for the wind and the falling leaves
    const box = (bx, by, S) => { const k = 3.5 * S / D.crownW, [cx, cy, cw, ch] = D.crop, m = .35 * S;
      return [bx + (cx - D.base[0]) * k - m, by + (cy - D.base[1]) * k - m, bx + (cx + cw - D.base[0]) * k + m, by + (cy + ch - D.base[1]) * k + m]; };
    // the full tree's height above its foot, in units of the scene's scale
    const height = () => (D.base[1] - D.crop[1]) * 3.5 / D.crownW;
    return { draw, box, height, loaded: () => loaded, leaf, branch, tipsAt, tipNear, topOf, AGENT, tips, reset: () => { leaves.length = 0; used.clear(); } };
  })();

  // ── Tree styles (test): the maple layer drawn small, then redrawn as pixels, characters or dots ──
  let treeStyle = 'pixel';
  const TS = { c: document.createElement('canvas'), out: document.createElement('canvas') };
  TS.x = TS.c.getContext('2d', { willReadFrequently: true }); TS.o = TS.out.getContext('2d');
  const BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5].map(v => v / 16 - .47);
  const RAMP = ' .:-=+*%#@';
  function drawStyled(bx, by, S, p, time, extra) {
    const cw = treeStyle === 'pixel' ? (small ? 3 : 4) : treeStyle === 'ascii' ? (small ? 6 : 8) : (small ? 6 : 8);
    const chh = treeStyle === 'ascii' ? Math.round(cw * 1.6) : cw;
    const w = Math.ceil(W / cw), h = Math.ceil(H / chh);
    if (TS.c.width !== w || TS.c.height !== h) { TS.c.width = TS.out.width = w; TS.c.height = TS.out.height = h; }
    const blitOut = () => { ctx.imageSmoothingEnabled = false; ctx.drawImage(TS.out, 0, 0, w * cw, h * chh); ctx.imageSmoothingEnabled = true; };
    // pixel art needs no more than 30 images a second at rest; any change of the scene redraws at once
    const key = [bx, by, S, p.G, p.A, p.F, p.B, p.alpha, p.focus, p.pl, PLAY.leaves.length, PLAY.shoots.length, CL.t0, MAPLE.loaded()].map(v => Math.round(v * 1000)).join();
    if (treeStyle === 'pixel' && TS.key === key && TS.cw === cw && time - TS.t < 33 && time >= TS.t) { blitOut(); return; }
    TS.x.setTransform(1 / cw, 0, 0, 1 / chh, 0, 0); TS.x.clearRect(0, 0, W, H);
    MAPLE.draw(bx, by, S, p, time, TS.x);
    if (extra) { TS.x.save(); extra(TS.x); TS.x.restore(); }
    // only the cells the tree can cover are read and dithered
    const [l, t0, r, b0] = MAPLE.box(bx, by, S);
    const x0 = Math.max(0, Math.floor(l / cw)), y0 = Math.max(0, Math.floor(t0 / chh));
    const bw = Math.min(w, Math.ceil(r / cw)) - x0, bh = Math.min(h, Math.ceil(b0 / chh)) - y0;
    if (bw <= 0 || bh <= 0) return;
    let img;
    try { img = treeStyle === 'pixel' ? TS.x.getImageData(x0, y0, bw, bh) : TS.x.getImageData(0, 0, w, h); } catch (e) { treeStyle = 'paint'; MAPLE.draw(bx, by, S, p, time); return; }
    const d = img.data;
    if (treeStyle === 'pixel') {
      // ordered dither: five levels a channel, the copper keeps its hue and the agents their cool light
      const st = 255 / 4;
      for (let y = 0; y < bh; y++) for (let x = 0; x < bw; x++) {
        const i = (y * bw + x) * 4, a = d[i + 3]; if (!a) continue;
        const t = BAYER[((y + y0) & 3) * 4 + ((x + x0) & 3)];
        // thin wood covers only part of a cell: its coverage counts double, so a young stem stays a line
        // the render's soft shadow is dark and half transparent: it stays out, or it would show as a band of dull pixels
        if (a < 235 && d[i] * .5 + d[i + 1] * .4 + d[i + 2] * .1 < 34) { d[i + 3] = 0; continue; }
        if (Math.min(1, a / 255 * 2) + t * .7 < .5) { d[i + 3] = 0; continue; }
        if (a < 255) { const u = Math.min(1.8, 255 / a); d[i] = Math.min(255, d[i] * u); d[i + 1] = Math.min(255, d[i + 1] * u); d[i + 2] = Math.min(255, d[i + 2] * u); }
        // dark wood and deep shade keep at least the first level of their hue: rounded to black, every
        // other cell vanished into the night and the trunk read as a checkerboard with holes
        const mx = Math.max(d[i], d[i + 1], d[i + 2]);
        if (mx > 0 && mx < st * .82) { const u = st * .82 / mx; d[i] *= u; d[i + 1] *= u; d[i + 2] *= u; }
        for (let k = 0; k < 3; k++) d[i + k] = Math.max(0, Math.min(255, Math.round(255 * Math.pow(d[i + k] / 255, .82) / st + t) * st));
        d[i + 3] = 255;
      }
      TS.o.clearRect(0, 0, w, h); TS.o.putImageData(img, x0, y0);
      TS.key = key; TS.t = time; TS.cw = cw;
      blitOut();
      return;
    }
    if (treeStyle === 'ascii') {
      ctx.font = `600 ${Math.round(chh * .92)}px "DM Mono", Consolas, monospace`; ctx.textBaseline = 'top';
      for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
        const i = (y * w + x) * 4, a = d[i + 3]; if (a < 80) continue;
        const r = d[i], g = d[i + 1], b = d[i + 2], l = Math.min(1, .22 + (r * .5 + g * .4 + b * .1) / 255 * 2.3);
        const c = RAMP[Math.max(2, Math.min(9, Math.round(l * 9)))];
        // dark wood still reads: every character gets a floor of warm light
        const k = 1.35 + .6 * l;
        ctx.fillStyle = `rgb(${Math.min(255, r * k + 70) | 0},${Math.min(255, g * k + 38) | 0},${Math.min(255, b * k + 22) | 0})`;
        ctx.fillText(c, x * cw, y * chh);
      }
      return;
    }
    // dots: a printer's halftone, the bigger the dot the lighter the leaf
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
      const i = (y * w + x) * 4, a = d[i + 3]; if (a < 50) continue;
      const r = d[i], g = d[i + 1], b = d[i + 2], l = (r * .5 + g * .4 + b * .1) / 255;
      const rad = cw * .52 * Math.min(1, .28 + l * 1.7) * (a / 255);
      ctx.fillStyle = `rgb(${Math.min(255, r * 1.4 + 52) | 0},${Math.min(255, g * 1.35 + 30) | 0},${Math.min(255, b * 1.25 + 18) | 0})`;
      ctx.beginPath(); ctx.arc(x * cw + cw / 2 + (y & 1 ? cw / 2 : 0), y * chh + chh / 2, rad, 0, 6.2832); ctx.fill();
    }
  }

  function grow() {
    small = W < 700;
    // readable leaves (example data), pinned on the painted maple
    MAPLE.reset();
    const rents = MAPLE.leaf(MAPLE.topOf(MAPLE.AGENT[1])), quotes = MAPLE.leaf(MAPLE.topOf(MAPLE.AGENT[2]));
    const plan = MAPLE.leaf(MAPLE.tipNear(1.4, 3.7)), lease = MAPLE.leaf(MAPLE.tipNear(-1.1, 4.1)), who = MAPLE.leaf(MAPLE.tipNear(.4, 4.4));
    const visit = MAPLE.leaf(null, { ground: [210, 12] });
    const top = MAPLE.leaf(MAPLE.tipNear(.9, 4.5), { spring: true });
    LABELS = [
      { leaf: rents, by: 'claude', nature: 'Research', title: 'Shipping options in Europe', meta: 'added by Claude · since 3 Oct', when: s => s.inCycle && s.ch >= 1 && s.p.G > .95, auto: s => s.inCycle && s.ch === 1 },
      { leaf: who, by: 'you', nature: 'Question', title: 'Who buys on mobile?', meta: 'proposed by Humcase, kept by you · 3 Oct', when: s => s.inCycle && s.ch >= 1 && s.p.G > .95 },
      { leaf: plan, by: 'you', nature: 'Task', title: 'Rewrite the product pages', meta: 'added by you · since 2 Oct', when: s => s.inCycle && s.ch >= 1 && s.p.G > .95 },
      { leaf: lease, by: 'you', nature: 'Decision', title: 'Checkout on a single page', meta: 'open · added by you · since 4 Oct', when: s => s.inCycle && s.ch >= 1 && s.p.G > .95 },
      { leaf: quotes, by: 'codex', nature: 'Task', title: 'Resize the product photos', meta: 'added by Codex · result waiting for you', when: s => s.inAgents || (s.inCycle && s.ch >= 1 && s.p.A > .9), auto: s => s.inAgents && !CL.t0 },
      { leaf: visit, by: 'archived', nature: 'Task · archived', title: 'Export the old catalogue', meta: 'done 28 Sep · archived, not deleted', when: s => s.inCycle && s.ch >= 2 && s.p.F > .97, auto: s => s.inCycle && s.ch === 2 },
      { leaf: top, by: 'you', nature: 'Idea', title: 'Pick-up at the workshop', meta: 'grew from “Shipping options in Europe” · 5 Oct', when: s => s.inCycle && s.ch >= 3 && s.p.B > .9, auto: s => s.inCycle && s.ch === 3 },
    ];
    // Claude's leaf, written from the terminal in "You and your agents"
    Object.assign(CL, { b: MAPLE.branch(MAPLE.topOf(MAPLE.AGENT[1])), ox: .01, oy: -.02, sz: .085, on: false });
    LABELS.push({ leaf: CL, by: 'claude', nature: 'Research', title: 'Compare payment providers', meta: 'added by Claude · just now · waiting for your review', when: s => s.inAgents && CL.t0 > 0, auto: s => s.inAgents });
    // a rebuilt tree drops the visitor's example cases
    PLAY.leaves = []; PLAY.shoots = []; PLAY.gT = PLAY.g = PLAY_BARE;
    buildTags();
  }

  // ── "Your turn": cases the visitor grows on an empty idea (example, nothing saved) ──
  const NAT = {
    idea: { name: 'Idea', spr: 'fresh', titles: ['Gift wrapping', 'A loyalty card', 'Free returns'] },
    task: { name: 'Task', spr: 'copper', titles: ['Write the shipping page', 'Fix the size guide', 'Set up order emails'] },
    question: { name: 'Question', spr: 'amber', titles: ['Ship abroad?', 'Keep the old links?', 'Which product goes first?'] },
    decision: { name: 'Decision', spr: 'burnt', titles: ['Free shipping over €50', 'Launch after the sales', 'Keep the same logo'] },
    research: { name: 'Research', spr: 'rust', titles: ['Shops we admire', 'Delivery prices compared', 'What customers ask most'] },
  };
  const PROPOSALS = [
    { n: 'task', t: 'Test checkout on an old phone' }, { n: 'decision', t: 'Card only, or PayPal too?' },
    { n: 'research', t: 'Three shops we like, side by side' }, { n: 'task', t: 'Send the preview to the client' },
    { n: 'question', t: 'Which photo goes first?' }, { n: 'idea', t: 'A 30-second product video' },
  ];
  const PLAY_BARE = .42;
  const PLAY = { g: PLAY_BARE, gT: PLAY_BARE, leaves: [], shoots: [], n: {}, prop: 0 };
  const CL = { t0: 0, b: null, on: false, x: 0, y: 0 };
  const now0 = () => performance.now();
  function chooseTip(g) {
    const cand = MAPLE.tipsAt(g);
    const taken = PLAY.leaves.map(l => l.b);
    let best = cand[0], bestS = -1e9;
    for (const b of cand) {
      const d = taken.length ? Math.min(...taken.map(t => Math.hypot(t.rx - b.rx, t.ry - b.ry))) : 0;
      const sc = d - b.ry * .15 + Math.random() * .05 - (taken.includes(b) ? 2 : 0);
      if (sc > bestS) { bestS = sc; best = b; }
    }
    return best;
  }
  function addCase(kind) {
    const N = NAT[kind]; if (!N) return;
    const i = PLAY.n[kind] = ((PLAY.n[kind] ?? -1) + 1) % N.titles.length;
    PLAY.gT = Math.min(.8, PLAY.gT + .065);
    const b = chooseTip(PLAY.gT);
    const leaf = { b, ox: (Math.random() - .5) * .04, oy: -Math.random() * .03, sz: .085, spr: N.spr, t0: now0(), on: false, x: 0, y: 0 };
    PLAY.leaves.push(leaf);
    const L = { leaf, by: 'you', nature: N.name, title: N.titles[i], meta: 'added by you · just now', dyn: true,
      when: s => s.inPlay, auto: s => s.inPlay && PLAY.leaves[PLAY.leaves.length - 1] === leaf && !PLAY.shoots.some(x => x.state === 'proposed') };
    leaf.L = L; addLabel(L);
    while (PLAY.leaves.length > 12) { const old = PLAY.leaves.shift(); removeLabel(old.L); }
    kick(); return leaf;
  }
  function sprout() {
    if (PLAY.shoots.some(x => x.state === 'proposed')) return;
    const from = PLAY.leaves.length ? PLAY.leaves[PLAY.leaves.length - 1].b : chooseTip(PLAY.gT);
    [-1, 1].forEach(dir => {
      const P = PROPOSALS[PLAY.prop++ % PROPOSALS.length];
      const sh = { b: from, dir, ang: -Math.PI / 2 + dir * (.62 + Math.random() * .16), len: .5, t0: now0(), state: 'proposed', spr: NAT[P.n].spr, on: false, x: 0, y: 0 };
      const L = { leaf: sh, by: 'sprout', nature: NAT[P.n].name, title: P.t, meta: '', side: dir < 0 ? 'left' : 'right', dyn: true,
        actions: true, when: s => s.inPlay, auto: s => s.inPlay && sh.state === 'proposed' };
      L.onKeep = () => { sh.state = 'kept'; sh.t1 = now0(); L.nature = NAT[P.n].name; L.meta = 'added by you · just now'; L.actions = false; L.by = 'you'; fillTag(L); L.pinned = false; L.hoverTag = false; kick(); window.KB3D && window.KB3D.refresh(); };
      L.onRefuse = () => { sh.state = 'gone'; sh.t1 = now0(); removeLabel(L); kick(); };
      sh.L = L; PLAY.shoots.push(sh); addLabel(L);
    });
    kick();
  }
  function empty() {
    PLAY.leaves.forEach(l => removeLabel(l.L)); PLAY.shoots.forEach(x => removeLabel(x.L));
    PLAY.leaves = []; PLAY.shoots = []; PLAY.gT = PLAY_BARE; kick();
  }
  window.HumWorld = {
    addCase, sprout, empty,
    treeStyle: () => treeStyle,
    count: () => PLAY.leaves.length + PLAY.shoots.filter(x => x.state === 'kept').length,
    claude: { show() { CL.t0 = now0(); kick(); }, hide() { CL.t0 = 0; kick(); } },
  };

  // sprites (the visitor's leaves, Claude's leaf) go where the tree is drawn, so the pixel style takes them too;
  // Sprout's proposals are lines of the interface, drawn sharp on the page every frame
  function drawPlay(S, now, X = ctx, mode = 'all') {
    const w = cur.pl, spr = mode !== 'lines', lines = mode !== 'sprites';
    for (const l of PLAY.leaves) {
      const b = l.b;
      if (w < .05 || !b.vis) { l.on = false; continue; }
      const g = reduce ? 1 : easeOutCubic(clamp01((now - l.t0) / 700));
      const x = b.tx + l.ox * S, y = b.ty + l.oy * S; l.x = x; l.y = y; l.on = g > .5;
      const sz = l.sz * S * g;
      if (!spr) continue;
      X.globalAlpha = .9 * w; X.drawImage(SPR[l.spr], x - sz, y - sz, sz * 2, sz * 2);
      X.globalAlpha = .55 * w * g; const c = sz * .38; X.drawImage(SPR.cream, x - c, y - c, c * 2, c * 2);
    }
    for (const sh of PLAY.shoots) {
      const b = sh.b;
      if (w < .05 || !b.vis || (sh.state === 'gone' && (reduce || now - sh.t1 > 400))) { sh.on = false; continue; }
      const g = reduce ? 1 : easeOutCubic(clamp01((now - sh.t0) / 600));
      const fade = sh.state === 'gone' ? 1 - clamp01((now - sh.t1) / 400) : 1;
      const L = sh.len * S * g, x0 = b.tx, y0 = b.ty;
      const x1 = x0 + Math.cos(sh.ang) * L, y1 = y0 + Math.sin(sh.ang) * L;
      const mx = x0 + Math.cos(sh.ang - sh.dir * .25) * L * .55, my = y0 + Math.sin(sh.ang - sh.dir * .25) * L * .55;
      sh.x = x1; sh.y = y1; sh.on = g > .6 && sh.state !== 'gone';
      if (lines) {
      ctx.save(); ctx.globalAlpha = w * fade; ctx.lineCap = 'round';
      if (sh.state === 'kept') { ctx.strokeStyle = 'rgba(214,128,76,.9)'; ctx.lineWidth = 2.2; }
      else { ctx.setLineDash([4, 5]); ctx.strokeStyle = 'rgba(246,230,206,.85)'; ctx.lineWidth = 1.5; }
      ctx.beginPath(); ctx.moveTo(x0, y0); ctx.quadraticCurveTo(mx, my, x1, y1); ctx.stroke(); ctx.restore();
      }
      if (spr && sh.state === 'kept') {
        const kg = reduce ? 1 : easeOutCubic(clamp01((now - sh.t1) / 600)), sz = .085 * S * kg;
        X.globalAlpha = .9 * w; X.drawImage(SPR[sh.spr], x1 - sz, y1 - sz, sz * 2, sz * 2);
      }
    }
    // Claude's leaf from the terminal
    if (CL.b && CL.t0 && CL.b.vis && cur.focus > .3) {
      const g = reduce ? 1 : easeOutCubic(clamp01((now - CL.t0) / 900));
      const x = CL.b.tx + CL.ox * S, y = CL.b.ty + CL.oy * S, sz = CL.sz * S * g;
      CL.x = x; CL.y = y; CL.on = g > .4;
      if (spr) {
        X.globalAlpha = .95 * cur.A; X.drawImage(SPR.claude, x - sz, y - sz, sz * 2, sz * 2);
        const c = sz * .4; X.globalAlpha = .6 * g; X.drawImage(SPR.cream, x - c, y - c, c * 2, c * 2);
      }
    } else CL.on = false;
    X.globalAlpha = 1; ctx.globalAlpha = 1;
  }

  // ── Scene parameters, driven by scroll (anchors) and smoothed in time ──
  const KEYS = ['cx', 'by', 's', 'alpha', 'G', 'A', 'F', 'B', 'focus', 'sap', 'pl', 'cool'];
  const HOLD = ['cx', 'by', 's', 'G', 'A', 'F', 'B', 'focus', 'sap', 'pl', 'cool'];
  let anchors = [], cur = null, heroStart = performance.now(), sceneState = { inCycle: false, inAgents: false, inPlay: false, ch: -1, chp: 0 };
  let cycleTop = 0, cycleEnd = 0, chapterTops = [], agentsTop = 0, agentsEnd = 0, playTop = 0, playEnd = 0;
  const top = el => el.getBoundingClientRect().top + scrollY;
  function heroG(now) { return reduce ? 1 : clamp01((now - heroStart - 200) / 5200); }

  let chapterHs = [];
  function computeAnchors() {
    // An empty viewport (a hidden tab or pane, 0 x 0) has no scene: its sizes divide by zero and
    // would leave the tree's state non-finite for good. The resize event recomputes once it has a size.
    if (!W || !H) return;
    const $ = s => document.querySelector(s);
    const L = small ? { side: .5, by: .86, s: .12 } : { side: W < 1100 ? .7 : .68, by: .93, s: W < 1700 ? .225 : .25 };
    const hero = { cx: .5, by: small ? 1.04 : 1.0, s: small ? .125 : .21, alpha: 1, G: 'hero', A: 'hero', F: 0, B: 0, focus: 0, sap: 0, pl: 0, cool: 0 };
    const ch = o => Object.assign({ cx: L.side, by: L.by, s: L.s, alpha: 1, G: 1, A: 1, F: 1, B: 1, focus: 0, sap: 1, pl: 0, cool: 0 }, o);
    // the idea on the desk is a young tree, drawn smaller than its scale: a larger scale keeps it readable
    const play = small ? { cx: .5, by: .64, s: .16, alpha: 1, G: 'play', A: 0, F: 0, B: 0, focus: 0, sap: 0, pl: 1, cool: 0 }
      : { cx: .5, by: .5, s: .19, alpha: 1, G: 'play', A: 0, F: 0, B: 0, focus: 0, sap: 0, pl: 1, cool: 0 };
    const agents = small ? ch({ alpha: .55, focus: 1, F: 0, B: 0, cool: 1 }) : ch({ s: W < 1700 ? .22 : .245, by: .95, focus: 1, F: 0, B: 0, cool: 1 });
    const dim = a => Object.assign({}, agents, { cx: .5, by: 1.02, s: small ? .125 : .25, focus: 0, alpha: a, sap: 0, cool: 0 });
    const fin0 = small ? { cx: .5, by: 1.0, s: .13 } : { cx: .5, by: 1.1, s: .285 };
    // the crown stays under the copy of its screen: smaller first, then lower (its foot leaves the screen)
    const HF = MAPLE.height();
    const under = (el, viewTop, p, sMin) => {
      const b = el.getBoundingClientRect().bottom + scrollY - viewTop + (small ? 20 : 32);
      let s = Math.min(p.s, (p.by * H - b) / (HF * H)), by = p.by;
      if (s < sMin) { s = sMin; by = (b + HF * s * H) / H; }
      return Object.assign({}, p, { s, by });
    };
    // the hero's tree fills the room under the offer: as big as the width allows, then lifted so its crown
    // starts just under the copy (capped at a fixed size, it left a wide gap on tall screens)
    {
      const b = $('#winget-note').getBoundingClientRect().bottom + scrollY + (small ? 28 : 36);
      const sW = W * (small ? .94 : .6) / (3.5 * H);
      const s = Math.max(small ? .1 : .17, Math.min((hero.by * H - b) / (HF * H), sW));
      Object.assign(hero, { s, by: Math.min(hero.by, (b + HF * s * H) / H) });
    }
    const chEls = [...document.querySelectorAll('[data-chapter]')];
    chapterTops = chEls.map(top); chapterHs = chEls.map(e => e.offsetHeight);
    // order of the page: hero, peek at the app, cycle, agents, app, trust, your turn, questions
    cycleTop = top($('#cycle')); cycleEnd = top($('#agents'));
    agentsTop = cycleEnd; agentsEnd = top($('#app'));
    const appTop = agentsEnd;
    const trustTop = top($('#trust'));
    playTop = top($('#play')); playEnd = top($('#faq'));
    const faqTop = playEnd, finalTop = top($('#final'));
    // the final call and the footer fill the last screen together, so its headline never slides under the bar
    const finEl = $('#final'), finMin = Math.max(0, H - $('.site-foot').offsetHeight) + 'px';
    if (finEl.style.minHeight !== finMin) finEl.style.minHeight = finMin;
    // the last screen is the end of the page: the copy sits higher there, and on a phone the footer would hide the tree's foot
    const finView = Math.max(finalTop - H * .05, document.documentElement.scrollHeight - H);
    const footTop = $('.site-foot').getBoundingClientRect().top + scrollY - finView;
    const fin = under($('#news-note'), finView, small ? Object.assign({}, fin0, { by: Math.min(fin0.by, footTop / H) }) : fin0, small ? .1 : .2);
    // a chapter's state runs while its copy is pinned: from its top to its end minus one screen
    const at = (i, f) => lerp(chapterTops[i] - H * .2, chapterTops[i] + chapterHs[i] - H * 1.05, f);
    const bare = .32;   // the maple's growth where it is still a slender young tree
    anchors = [
      { y: 0, p: hero },
      { y: H * .3, p: hero },
      { y: H * .62, p: Object.assign({}, hero, { alpha: 0 }) },
      // "Bare" looks closely at the young tree; the view pulls back while it grows
      { y: cycleTop - H * .1, p: ch({ G: .02, A: 0, F: 0, B: 0, alpha: 0, s: L.s * 2.9 }) },
      { y: at(0, 0), p: ch({ G: bare * .45, A: 0, F: 0, B: 0, s: L.s * 2.9 }) },
      { y: at(0, 1), p: ch({ G: bare * .97, A: 0, F: 0, B: 0, s: L.s * 2.4 }) },
      { y: at(1, 0), p: ch({ G: bare * 1.05, A: 0, F: 0, B: 0, s: L.s * 1.9 }) },
      { y: at(1, .5), p: ch({ G: 1, A: .2, F: 0, B: 0, cool: 1 }) },
      { y: at(1, .85), p: ch({ G: 1, A: 1, F: 0, B: 0, cool: 1 }) },
      { y: at(2, 0), p: ch({ F: 0, B: 0 }) },
      { y: at(2, .8), p: ch({ F: 1, B: 0 }) },
      { y: at(3, 0), p: ch({ B: 0 }) },
      { y: at(3, .8), p: ch({ B: 1 }) },
      { y: agentsTop - H * .75, p: ch({}) },
      { y: agentsTop - H * .3, p: Object.assign({}, agents, { alpha: 0 }) },
      { y: agentsTop + H * .05, p: agents },
      { y: agentsEnd - H * .55, p: agents },
      { y: appTop - H * .1, p: dim(.2) },
      { y: playTop - H * .45, p: Object.assign({}, play, { alpha: 0 }) },
      { y: playTop - H * .18, p: play },
      { y: Math.max(playTop - H * .1, playEnd - H * .85), p: play },
      { y: playEnd - H * .5, p: Object.assign({}, play, { alpha: 0 }) },
      { y: trustTop, p: dim(.18) },
      { y: faqTop, p: dim(.08) },
      { y: finalTop - H * .3, p: Object.assign(dim(.08), fin) },
      { y: finView, p: Object.assign(dim(1), fin) },
    ];
    anchors.sort((a, b) => a.y - b.y);
  }
  const heroA = now => reduce ? 1 : clamp01((now - heroStart - 5600) / 2400);
  const val = (p, k, now) => k === 'A' && p.A === 'hero' ? heroA(now) : k === 'G' && p.G === 'hero' ? heroG(now) : k === 'G' && p.G === 'play' ? PLAY.g : p[k];
  function targetAt(y, now) {
    const A = anchors, out = {};
    if (y <= A[0].y) { for (const k of KEYS) out[k] = val(A[0].p, k, now); return out; }
    for (let i = 0; i < A.length - 1; i++) {
      const a = A[i], b = A[i + 1];
      if (y <= b.y) {
        const t = smooth(0, 1, (y - a.y) / Math.max(1, b.y - a.y));
        for (const k of KEYS) out[k] = lerp(val(a.p, k, now), val(b.p, k, now), t);
        if (a.p.alpha === 0 || b.p.alpha === 0) { const src = a.p.alpha === 0 ? b.p : a.p; for (const k of HOLD) out[k] = val(src, k, now); }
        return out;
      }
    }
    const last = A[A.length - 1].p; for (const k of KEYS) out[k] = val(last, k, now); return out;
  }

  // ── Interaction from the component: wind from the pointer, shake on press ──
  let wind = 0, windT = 0, shake = 0;
  addEventListener('pointermove', e => { if (e.pointerType === 'mouse' && cur) windT = Math.max(-1, Math.min(1, (e.clientX - cur.cx * W) / (W / 2))) * .5; }, { passive: true });
  document.addEventListener('pointerleave', () => windT = 0);
  document.querySelectorAll('[data-shake]').forEach(el => el.addEventListener('pointerdown', () => { if (!reduce) shake = 1; }));

  // ── Particles (as the original) ──
  let parts = [];
  const mkPart = full => ({ x: rand(W * .1, W * .9), y: full ? rand(H * .1, H * .9) : rand(H * .5, H), vx: rand(-.12, .12), vy: rand(-.35, -.06),
    size: rand(.6, 2.2), alpha: rand(.04, .2), ph: rand(0, 6.28), fq: rand(.0004, .0015), life: full ? rand(0, 1) : 0, ls: rand(.0006, .0025) });
  function initParts() { R = Math.random; parts = Array.from({ length: small ? 34 : 60 }, () => mkPart(true)); }

  // ── Drawing ──
  function draw(now) {
    const p = cur;
    const S = p.s * H, bx = p.cx * W;
    const by = p.by * H;
    const time = reduce ? 0 : now;
    ctx.setTransform(DPR, 0, 0, DPR, 0, 0);
    // transparent: the sky canvas behind shows through (the page itself is black without it)
    ctx.clearRect(0, 0, W, H);
    // ambient canopy glow and ground glow (original), warmer
    const gA = Math.min(1, p.G * 1.3);
    // the crown's light: a faint wash rising from the ground, no shape of its own
    if (gA > .02) {
      const wash = ctx.createLinearGradient(0, by - S * 3.4, 0, by);
      wash.addColorStop(0, 'rgba(2,2,2,0)'); wash.addColorStop(.55, `rgba(44,24,10,${.12 * gA})`); wash.addColorStop(1, 'rgba(2,2,2,0)');
      ctx.fillStyle = wash; ctx.fillRect(0, by - S * 3.4, W, S * 3.4);
    }
    // the ground: a level band of warm earth across the whole width, darker below
    const top = by - S * .55, bot = by + S * .7;
    const ground = ctx.createLinearGradient(0, top, 0, bot);
    ground.addColorStop(0, 'rgba(40,22,9,0)'); ground.addColorStop(.3, 'rgba(40,22,9,.035)'); ground.addColorStop(.44, 'rgba(46,25,10,.09)');
    ground.addColorStop(.58, 'rgba(14,8,4,.22)'); ground.addColorStop(1, 'rgba(3,2,1,.55)');
    ctx.fillStyle = ground; ctx.fillRect(0, top, W, bot - top);
    ctx.fillStyle = 'rgba(3,2,1,.55)'; if (bot < H) ctx.fillRect(0, bot, W, H - bot);
    if (treeStyle === 'paint') MAPLE.draw(bx, by, S, p, time); else drawStyled(bx, by, S, p, time, X => drawPlay(S * youthScale(p.G), now, X, 'sprites'));
    if (window.HumSky) window.HumSky.halo(bx, by - 2.0 * S * youthScale(p.G), 1.6 * S * youthScale(p.G), .22 * p.alpha * clamp01(p.G * 3) * (1 - .6 * p.focus));
    drawPlay(S * youthScale(p.G), now, ctx, treeStyle === 'paint' ? 'all' : 'lines');

    // particles
    for (let i = 0; i < parts.length; i++) {
      const q = parts[i];
      if (!reduce) {
        q.x += q.vx + Math.sin(now * q.fq + q.ph) * .25; q.y += q.vy; q.life += q.ls;
        if (q.life > 1 || q.y < -10 || q.x < -10 || q.x > W + 10) { parts[i] = mkPart(false); continue; }
      }
      const lf = q.life < .15 ? q.life / .15 : q.life > .8 ? (1 - q.life) / .2 : 1;
      const a = q.alpha * lf; if (a < .004) continue;
      const s = q.size * 3; ctx.globalAlpha = a; ctx.drawImage(SPR.mote, q.x - s, q.y - s, s * 2, s * 2);
    }
    ctx.globalAlpha = 1;
    // vignette (original)
    const v = ctx.createRadialGradient(W / 2, H / 2, Math.min(W, H) * .3, W / 2, H / 2, Math.max(W, H) * .78);
    v.addColorStop(0, 'rgba(0,0,0,0)'); v.addColorStop(1, 'rgba(0,0,0,.42)');
    ctx.fillStyle = v; ctx.fillRect(0, 0, W, H);
  }

  // ── Readable leaves: small tags over the canvas ──
  const tagsEl = document.getElementById('tags');
  let tagN = 0;
  const TR = window.TR || (s => s);
  function fillTag(L) {
    const nature = TR(L.nature), title = TR(L.title), meta = TR(L.meta);
    L.tag.innerHTML = `<span class="tag__nature"><i data-by="${L.by}"></i>${nature}</span><span class="tag__title">${title}</span>${meta ? `<span class="tag__meta">${meta}</span>` : ''}`
      + (L.actions ? `<span class="tag__acts"><button class="key key--xs key--action" type="button" data-k>${TR('Keep')}</button><button class="key key--xs" type="button" data-r>${TR('Refuse')}</button></span>` : '');
    L.btn.setAttribute('aria-label', meta ? `${nature}: ${title}, ${meta}` : `${nature}: ${title}`);
    if (L.actions) {
      L.tag.querySelector('[data-k]').addEventListener('click', () => L.onKeep());
      L.tag.querySelector('[data-r]').addEventListener('click', () => L.onRefuse());
    }
    L.tag.classList.toggle('tag--acts', !!L.actions);
  }
  function addLabel(L) {
    const btn = document.createElement('button');
    btn.className = 'leaf'; btn.type = 'button'; btn.setAttribute('aria-expanded', 'false');
    const tag = document.createElement('div');
    tag.className = 'tag'; tag.setAttribute('role', 'tooltip'); tag.id = 'tag-' + (tagN++);
    btn.setAttribute('aria-describedby', tag.id);
    L.btn = btn; L.tag = tag; L.vis = false; fillTag(L);
    const open = v => { L.hover = v; };
    btn.addEventListener('pointerenter', () => open(true)); btn.addEventListener('pointerleave', () => open(false));
    btn.addEventListener('focus', () => open(true)); btn.addEventListener('blur', () => open(false));
    btn.addEventListener('click', () => { L.pinned = !L.pinned; });
    tag.addEventListener('pointerenter', () => { L.hoverTag = true; }); tag.addEventListener('pointerleave', () => { L.hoverTag = false; });
    tagsEl.append(btn, tag);
    if (!LABELS.includes(L)) LABELS.push(L);
  }
  function removeLabel(L) {
    if (!L) return;
    const i = LABELS.indexOf(L); if (i >= 0) LABELS.splice(i, 1);
    if (L.btn) L.btn.remove();
    if (L.tag) L.tag.remove();
  }
  function buildTags() {
    tagsEl.innerHTML = '';
    for (const L of LABELS.slice()) addLabel(L);
  }
  function placeTags() {
    const s = Object.assign({ p: cur }, sceneState);
    const visible = cur.alpha > .6;
    const anyHover = LABELS.some(L => L.hover || L.pinned || L.hoverTag);
    const shown = [], placed = [];
    // page copy the tags must not cover (the play section's heading and hint, a chapter's copy)
    const copySel = sceneState.inPlay ? (W < 768 ? '.play__head > *, .play__keys' : '.play__head > *') : sceneState.inCycle ? '.chapter .copy > *' : null;
    const keepOut = copySel ? [...document.querySelectorAll(copySel)].map(e => e.getBoundingClientRect()).filter(r => r.width > 40 && r.bottom > 0 && r.top < H) : [];
    for (const L of LABELS) {
      const show = visible && L.leaf.on && L.when(s);
      if (show !== L.vis) { L.btn.classList.toggle('on', show); L.vis = show; }
      if (show) { L.btn.style.transform = `translate(${L.leaf.x}px,${L.leaf.y}px)`; shown.push(L); }
    }
    for (const L of LABELS) {
      const want = L.vis && (L.hover || L.pinned || L.hoverTag || (L.actions && L.auto(s)) || (!anyHover && L.auto && L.auto(s)));
      // the class is set once, after the placement: toggling it twice in a frame restarts the fade and leaves the tag stuck
      const setOpen = open => { L.tag.classList.toggle('on', open); L.btn.setAttribute('aria-expanded', open ? 'true' : 'false'); L.tag.style.pointerEvents = open && L.actions ? 'auto' : 'none'; };
      if (!want) { setOpen(false); continue; }
      const x = L.leaf.x, y = L.leaf.y;
      const tw = L.tag.offsetWidth || 220, th = L.tag.offsetHeight || 70;
      const R = [[x + 18, y - th / 2], [x - 18 - tw, y - th / 2], [x + 14, y - th - 16], [x - tw - 14, y - th - 16], [x + 14, y + 16], [x - tw - 14, y + 16]];
      if (L.side === 'left') R.unshift(R.splice(1, 1)[0]);
      const clampP = ([tx, ty]) => [Math.max(12, Math.min(W - tw - 12, tx)), Math.max(76, Math.min(H - th - 76, ty))];
      const blocked = ([tx, ty]) => keepOut.some(r => tx < r.right + 10 && tx + tw + 10 > r.left && ty < r.bottom + 10 && ty + th + 10 > r.top)
        || placed.some(r => tx < r[0] + r[2] + 8 && tx + tw + 8 > r[0] && ty < r[1] + r[3] + 8 && ty + th + 8 > r[1]);
      const bad = ([tx, ty]) => blocked([tx, ty]) || shown.some(o => o !== L && o.leaf.x > tx - 14 && o.leaf.x < tx + tw + 14 && o.leaf.y > ty - 14 && o.leaf.y < ty + th + 14);
      let pos = clampP(R[0]);
      let found = false;
      for (const c of R) { const q = clampP(c); if (!bad(q)) { pos = q; found = true; break; } }
      // narrow screens: no room beside the leaf, so take the free band nearest to it; a tag far from its leaf reads as lost, so it waits
      if (!found) {
        const bands = [];
        for (let ty = 76; ty < H - th - 12; ty += 12) bands.push(ty);
        bands.sort((a, b) => Math.abs(a + th / 2 - y) - Math.abs(b + th / 2 - y));
        for (const ty of bands) { if (Math.abs(ty + th / 2 - y) > th / 2 + 120) break; const q = [Math.max(12, (W - tw) / 2), ty]; if (!blocked(q)) { pos = q; found = true; break; } }
      }
      setOpen(found);
      if (!found) continue;
      placed.push([pos[0], pos[1], tw, th]);
      L.tag.style.transform = `translate(${pos[0]}px,${pos[1]}px)`;
    }
  }

  // ── Chapter bar ──
  const bar = document.getElementById('chapbar');
  const barBtns = [...bar.querySelectorAll('button')];
  const LAND = [.9, .9, .85, .85];
  barBtns.forEach(b => b.addEventListener('click', () => {
    const i = +b.dataset.go;
    scrollTo({ top: lerp(chapterTops[i] - H * .2, chapterTops[i] + chapterHs[i] - H * 1.05, LAND[i]), behavior: reduce ? 'auto' : 'smooth' });
  }));
  let lastCh = -2;
  function updateScene() {
    const y = scrollY;
    const inCycle = y > chapterTops[0] - H * .6 && y < cycleEnd - H * .6;
    let ch = -1, chp = 0;
    for (let i = 0; i < chapterTops.length; i++) if (y >= chapterTops[i] - H * .45) { ch = i; chp = clamp01((y - (chapterTops[i] - H * .45)) / chapterHs[i]); }
    sceneState.inCycle = inCycle; sceneState.ch = inCycle ? ch : -1; sceneState.chp = chp;
    sceneState.inAgents = y > agentsTop - H * .2 && y < agentsEnd - H * .45;
    sceneState.inPlay = y > playTop - H * .35 && y < playEnd - H * .55;
    bar.classList.toggle('on', inCycle);
    if (ch !== lastCh) { barBtns.forEach((b, i) => b.toggleAttribute('aria-current', false) || (i === ch && b.setAttribute('aria-current', 'step'))); lastCh = ch; }
    barBtns.forEach((b, i) => { const f = b.querySelector('.fill'); f.style.transform = `scaleX(${i < ch ? 0 : i === ch ? chp : 0})`; });
  }

  // ── Loop: runs while visible; pauses when the tab is hidden or the world is dark ──
  let last = performance.now(), frameN = 0, running = false, dirty = true, gV = 0;
  function step(now) {
    running = true;
    const dt = Math.min(64, now - last); last = now; frameN++;
    const tgt = targetAt(scrollY, now);
    if (!cur) cur = Object.assign({}, tgt);
    const k = reduce ? 1 : 1 - Math.exp(-dt / 240), kl = reduce ? 1 : 1 - Math.exp(-dt / 380);
    for (const key of KEYS) {
      const snap = tgt.alpha < .04 && HOLD.includes(key);
      if (key === 'G' && !snap && !reduce) {
        const want = Math.max(-.42, Math.min(.42, (tgt.G - cur.G) * 2.6));
        gV = lerp(gV, want, 1 - Math.exp(-dt / 260));
        cur.G = clamp01(cur.G + gV * dt / 1000);
        continue;
      }
      if (key === 'G') gV = 0;
      cur[key] = snap ? tgt[key] : lerp(cur[key], tgt[key], (key === 'cx' || key === 'by' || key === 's') ? kl : k);
    }
    PLAY.g = reduce ? PLAY.gT : lerp(PLAY.g, PLAY.gT, 1 - Math.exp(-dt / 520));
    wind = lerp(wind, windT, reduce ? 0 : .04); if (shake > .01) shake *= .95; else shake = 0;
    cv.style.opacity = cur.alpha.toFixed(3);
    updateScene();
    const throttle = cur.alpha < .3 ? 3 : 1;
    if (cur.alpha > .015 && (frameN % throttle === 0)) draw(now);
    placeTags();
    if (document.hidden) { running = false; return; }
    if (reduce) {
      // Reduced motion: no growth animation; draw only when the scene changes.
      const settled = KEYS.every(key => Math.abs(cur[key] - tgt[key]) < 1e-4);
      if (settled && !dirty) { running = false; return; }
      dirty = false;
    }
    requestAnimationFrame(step);
  }
  const kick = () => { if (!anchors.length) return; dirty = true; if (!running) { last = performance.now(); requestAnimationFrame(step); } };

  function resize() {
    const w = innerWidth, h = innerHeight;
    if (!w || !h) return;
    const widthChanged = w !== W;
    W = w; H = h; DPR = Math.min(devicePixelRatio || 1, 1.5);
    cv.width = Math.round(W * DPR); cv.height = Math.round(H * DPR);
    if (widthChanged) { grow(); initParts(); }
    computeAnchors(); kick();
  }
  addEventListener('resize', resize);
  addEventListener('scroll', kick, { passive: true });
  document.addEventListener('visibilitychange', () => { if (!document.hidden) kick(); });
  RM.addEventListener('change', e => { reduce = e.matches; kick(); });
  new ResizeObserver(() => { computeAnchors(); kick(); }).observe(document.querySelector('main'));
  resize();
})();

/* ── Text Reveal (Mask): port of 21st 19257, split by measured lines ── */
(() => {
  const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const els = [...document.querySelectorAll('[data-reveal]')];
  const esc = s => s.replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
  function split(el) {
    if (!el.dataset.src) el.dataset.src = el.innerHTML;
    const tmp = document.createElement('div'); tmp.innerHTML = el.dataset.src;
    const words = []; let cur = '';
    const flush = () => { if (cur) { words.push(cur); cur = ''; } };
    (function walk(n, em) {
      for (const c of n.childNodes) {
        if (c.nodeType === 3) { for (const part of c.textContent.split(/(\s+)/)) { if (!part) continue; if (/^\s+$/.test(part)) flush(); else cur += em ? `<em>${esc(part)}</em>` : esc(part); } }
        else if (c.nodeName === 'BR') { flush(); words.push('\n'); }
        else walk(c, em || c.nodeName === 'EM');
      }
    })(tmp, false); flush();
    el.setAttribute('aria-label', tmp.textContent.replace(/\s+/g, ' ').trim());
    el.innerHTML = words.map((w, i) => w === '\n' ? '<br>' : `<span data-w="${i}" style="display:inline-block">${w}</span>`).join(' ');
    const lines = []; let line = [], lastTop = null;
    words.forEach((w, i) => {
      if (w === '\n') { if (line.length) lines.push(line); line = []; lastTop = null; return; }
      const t = el.querySelector(`[data-w="${i}"]`).offsetTop;
      if (lastTop !== null && t > lastTop + 2) { lines.push(line); line = []; }
      line.push(w); lastTop = t;
    });
    if (line.length) lines.push(line);
    el.innerHTML = lines.map((ln, i) => `<span class="rl" aria-hidden="true"><span class="ri" style="transition-delay:${i * 90}ms">${ln.join(' ')}</span></span>`).join('');
    el.dataset.w = el.clientWidth;
  }
  if (!reduce) {
    const run = () => {
      els.forEach(split);
      const io = new IntersectionObserver(es => es.forEach(e => { if (e.isIntersecting) { e.target.classList.add('revealed'); io.unobserve(e.target); } }), { rootMargin: '0px 0px -22% 0px' });
      els.forEach(el => el.dataset.reveal === 'load' ? requestAnimationFrame(() => setTimeout(() => el.classList.add('revealed'), 120)) : io.observe(el));
      let t; addEventListener('resize', () => { clearTimeout(t); t = setTimeout(() => els.forEach(el => { if (+el.dataset.w !== el.clientWidth) split(el); }), 150); });
    };
    (document.fonts && document.fonts.ready ? document.fonts.ready : Promise.resolve()).then(run);
  }
  // fade-rise
  const rises = document.querySelectorAll('.rise');
  if (reduce || !('IntersectionObserver' in window)) rises.forEach(e => e.classList.add('in'));
  else {
    const io2 = new IntersectionObserver(es => es.forEach(e => { if (e.isIntersecting) { e.target.classList.add('in'); io2.unobserve(e.target); } }), { rootMargin: '0px 0px -10% 0px' });
    const heroRises = [...document.querySelectorAll('.hero .rise')];
    heroRises.forEach((e, i) => setTimeout(() => e.classList.add('in'), 900 + i * 160));
    rises.forEach(e => { if (!heroRises.includes(e)) io2.observe(e); });
  }
})();

/* ── Keys: copy winget, journal demo, newsletter (download not open yet) ── */
(() => {
  const TR = window.TR || (s => s);
  document.querySelectorAll('[data-copy]').forEach(btn => btn.addEventListener('click', async () => {
    const label = btn.querySelector('.copy-state');
    try { await navigator.clipboard.writeText(btn.dataset.copy); label.textContent = TR('Copied'); }
    catch { label.textContent = TR('Select and copy'); }
    setTimeout(() => label.textContent = btn.dataset.copy, 1600);
  }));
  document.querySelectorAll('a[href="#download"]').forEach(a => a.addEventListener('click', e => {
    e.preventDefault();
    const note = document.getElementById('winget-note');
    note.textContent = TR('The download isn’t open yet. Windows 10 and 11 · unsigned preview');
  }));

  const rows = document.getElementById('journal-rows'), status = document.getElementById('journal-status');
  const accept = document.getElementById('j-accept'), rework = document.getElementById('j-rework'), reset = document.getElementById('j-reset');
  const initial = { rows: rows.innerHTML, status: status.innerHTML };
  const add = (who, by, text) => { const li = document.createElement('li'); li.className = 'new'; li.innerHTML = `<b><i${by ? ` data-by="${by}"` : ''}></i>${TR(who)}</b><span>${TR(text)}</span><time>${TR('just now')}</time>`; rows.append(li); };
  accept.addEventListener('click', () => { add('You', '', 'accepted the result'); status.innerHTML = `${TR('Done, accepted by you')}<small>${TR('the decision is in the history')}</small>`; accept.disabled = rework.disabled = true; reset.hidden = false; });
  rework.addEventListener('click', () => { add('You', '', 'asked for a rework: “keep the white background”'); status.innerHTML = `${TR('Back to Codex, in progress')}<small>${TR('your reason travels with the task')}</small>`; accept.disabled = rework.disabled = true; reset.hidden = false; });
  reset.addEventListener('click', () => { rows.innerHTML = initial.rows; status.innerHTML = initial.status; accept.disabled = rework.disabled = false; reset.hidden = true; });

  // Release news: the server sends a confirmation email; the address joins the list only once confirmed.
  // Cloudflare Turnstile vouches that a person asks (the server checks its token): its script loads only
  // once the visitor turns to the form, so a visit that never does makes no request to Cloudflare.
  const news = document.getElementById('news'), newsNote = document.getElementById('news-note');
  let check = null;
  const startCheck = () => {
    if (check) return check;
    let widget = null, token = '', blocked = false;
    const waiters = [];
    const settle = value => { token = value; while (waiters.length) waiters.shift()(value); };
    check = {
      // A token for one sign-up: the one ready, or the next one. Empty at once if the script could not
      // load, and after 2 minutes otherwise (time to solve a challenge; Turnstile retries its own errors).
      token: () => token || blocked ? Promise.resolve(token) : new Promise(r => { waiters.push(r); setTimeout(() => r(token), 120000); }),
      // Tokens are single use: once one was sent, ask for a new one. Without a token, nothing was spent,
      // and a challenge the visitor is solving stays as it is.
      blocked: () => blocked,
      spent: () => { token = ''; if (widget !== null) window.turnstile.reset(widget); },
    };
    window.humTurnstile = () => {
      widget = window.turnstile.render('#news-check', {
        sitekey: news.dataset.sitekey, action: 'news', appearance: 'interaction-only', size: 'flexible',
        language: window.HUM_LANG || 'auto',
        callback: settle,
        'expired-callback': () => { token = ''; },
        'error-callback': () => true,
      });
    };
    const script = document.createElement('script');
    script.src = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit&onload=humTurnstile';
    script.async = true;
    script.onerror = () => { blocked = true; settle(''); };
    document.head.append(script);
    return check;
  };
  news.addEventListener('focusin', startCheck, { once: true });
  news.addEventListener('pointerdown', startCheck, { once: true });
  news.addEventListener('submit', async e => {
    e.preventDefault();
    const input = document.getElementById('news-email'), button = news.querySelector('button[type=submit]');
    const email = input.value.trim();
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) { newsNote.textContent = TR('This email address doesn’t look right.'); input.focus(); return; }
    button.disabled = true; newsNote.textContent = TR('Sending…');
    const human = startCheck();
    let turnstile = '';
    try {
      turnstile = await human.token();
      if (human.blocked()) { newsNote.textContent = TR('The robot check couldn’t load. Turn off your blocker for humcase.com, then try again.'); return; }
      const res = await fetch('/api/subscribe', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ email, lang: window.HUM_LANG || 'en', website: news.querySelector('.news-trap').value, turnstile }) });
      if (res.ok) { newsNote.textContent = TR('Almost there: open the email we sent and confirm.'); input.value = ''; }
      else newsNote.textContent = TR(res.status === 400 ? 'This email address doesn’t look right.' : 'That didn’t work. Try again in a moment.');
    } catch { newsNote.textContent = TR('That didn’t work. Try again in a moment.'); }
    finally { if (turnstile) human.spent(); button.disabled = false; }
  });
})();
