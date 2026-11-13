/* ── Film grain: a 180 px noise tile drawn once (light and dark specks, very low alpha) ── */
(() => {
  const c = document.createElement('canvas'); c.width = c.height = 180;
  const x = c.getContext('2d'), img = x.createImageData(180, 180), d = img.data;
  for (let i = 0; i < d.length; i += 4) {
    const r = Math.random();
    if (r < .5) { d[i] = d[i + 1] = d[i + 2] = 0; d[i + 3] = (.5 - r) * 2 * 46; }
    else { d[i] = d[i + 1] = d[i + 2] = 255; d[i + 3] = (r - .5) * 2 * 22; }
  }
  x.putImageData(img, 0, 0);
  document.documentElement.style.setProperty('--grain', `url(${c.toDataURL()})`);
})();

/* ── "Your turn": keys of the visitor's real keyboard, the 3D keyboard and the touch keys ── */
(() => {
  const sec = document.getElementById('play');
  const NATURE_CODES = { KeyI: 'idea', KeyT: 'task', KeyQ: 'question', KeyD: 'decision', KeyR: 'research' };
  // KEY_MAP of the 21st Mechanical Keyboard, adapted to the Windows bottom row
  const KEY_MAP = { Escape: 'esc', Backspace: 'bsp', Tab: 'tab', Enter: 'enter', NumpadEnter: 'enter', Space: 'space', CapsLock: 'caps',
    ShiftLeft: 'lsh', ShiftRight: 'rsh', ControlLeft: 'ctrl', ControlRight: 'ctrl', AltLeft: 'lalt', AltRight: 'ralt', MetaLeft: 'win', MetaRight: 'win',
    ArrowUp: 'up', ArrowLeft: 'left', ArrowDown: 'down', ArrowRight: 'right', Delete: 'del', Digit1: 'k1', Digit2: 'k2', Digit3: 'k3', Digit4: 'k4',
    Digit5: 'k5', Digit6: 'k6', Digit7: 'k7', Digit8: 'k8', Digit9: 'k9', Digit0: 'k0', Backquote: 'grav', Minus: 'min', Equal: 'equ',
    BracketLeft: 'lb', BracketRight: 'rb', Backslash: 'bsl', Semicolon: 'sem', Quote: 'quo', Comma: 'com', Period: 'per', Slash: 'sla' };
  const idOf = code => KEY_MAP[code] || (code.startsWith('Key') ? code.slice(3).toLowerCase() : /^F\d+$/.test(code) ? code.toLowerCase() : null);
  let inView = false;
  new IntersectionObserver(es => { inView = es[es.length - 1].intersectionRatio >= .45; }, { threshold: [0, .45, .7, 1] }).observe(sec);
  function act(kind) {
    const W = window.HumWorld; if (!W) return;
    if (kind === 'sprout') W.sprout(); else if (kind === 'empty') W.empty(); else W.addCase(kind);
    if (window.KB3D) window.KB3D.refresh();
  }
  window.HumPlay = { act, inView: () => inView };
  addEventListener('keydown', e => {
    if (!inView || e.ctrlKey || e.metaKey || e.altKey) return;
    const t = e.target;
    if (t && t.closest && t.closest('input, textarea, select, [contenteditable]')) return;
    const id = idOf(e.code);
    if (id && window.KB3D) window.KB3D.down(id);
    if (e.repeat) return;
    if (NATURE_CODES[e.code]) act(NATURE_CODES[e.code]);
    else if (e.code === 'Enter' || e.code === 'NumpadEnter') {
      if (t && t.closest && t.closest('button, a, summary')) return;
      e.preventDefault(); act('sprout');
    } else if (e.code === 'Escape') act('empty');
  });
  addEventListener('keyup', e => { const id = idOf(e.code); if (id && window.KB3D) window.KB3D.up(id); });
  addEventListener('blur', () => window.KB3D && window.KB3D.releaseAll());
  document.querySelectorAll('[data-act]').forEach(b => b.addEventListener('click', () => act(b.dataset.act)));
})();

/* ── Terminal Typewriter (21st 29812), ported: commands typed at typeSpeed, output after
   outputDelay at 15 ms a character, delay between commands, restart once complete. ── */
(() => {
  const box = document.getElementById('term'), body = document.getElementById('term-body'), replay = document.getElementById('term-replay');
  const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const TYPE = 46, OUT = 15, BETWEEN = 900, TR = window.TR || (s => s);
  const CMDS = [
    { prompt: TR('PS C:\\Users\\you\\shop> '), command: 'claude', output: TR('Claude Code · Humcase MCP server connected'), outputDelay: 500 },
    { prompt: '> ', command: TR('Compare three payment providers for a small shop, and file it in Humcase'), output: TR('Reading the fees and docs of three providers…\n✓ 3 sources read, summary written'), outputDelay: 700 },
    { output: '● humcase · add_note (research)', cls: 'tool', outputDelay: 350 },
    { output: TR('→ Humcase: research “Compare payment providers” added to “Redesign the online shop”'), cls: 'ok', outputDelay: 450,
      onDone: () => window.HumWorld && window.HumWorld.claude.show() },
    { output: TR('  It waits for your review: accept it, or ask for a rework.'), outputDelay: 600 },
  ];
  let run = 0;
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  const el = (tag, cls, text) => { const n = document.createElement(tag); if (cls) n.className = cls; if (text != null) n.textContent = text; return n; };
  async function play() {
    const me = ++run;
    body.textContent = ''; replay.hidden = true;
    if (window.HumWorld) window.HumWorld.claude.hide();
    const caret = el('span', 'caret');
    for (let i = 0; i < CMDS.length; i++) {
      const c = CMDS[i], row = el('div', 'term__row'); body.append(row);
      if (c.command) {
        const line = el('div'); line.append(el('span', 'term__prompt', c.prompt));
        const cmd = el('span'); line.append(cmd, caret); row.append(line);
        for (let k = 1; k <= c.command.length; k++) { if (me !== run) return; cmd.textContent = c.command.slice(0, k); if (!reduce) await sleep(TYPE); }
      }
      if (!reduce) await sleep(c.outputDelay ?? 500);
      if (me !== run) return;
      if (c.output) {
        const out = el('div', 'term__out' + (c.cls ? ' ' + c.cls : '')); const txt = el('span'); out.append(txt, caret); row.append(out);
        if (reduce) txt.textContent = c.output;
        else for (let k = 1; k <= c.output.length; k++) { if (me !== run) return; txt.textContent = c.output.slice(0, k); await sleep(OUT); }
      }
      if (c.onDone) c.onDone();
      if (i < CMDS.length - 1 && !reduce) await sleep(c.command ? BETWEEN : 380);
    }
    const end = el('div', 'term__row'); end.append(el('span', 'term__prompt', '> '), caret); body.append(end);
    replay.hidden = false;
  }
  // idle prompt before it plays (as the original's empty state)
  const idle = el('div', 'term__row'); idle.append(el('span', 'term__prompt', CMDS[0].prompt), el('span', 'caret')); body.append(idle);
  const io = new IntersectionObserver(es => { if (es.some(e => e.isIntersecting)) { io.disconnect(); play(); } }, { threshold: .3 });
  io.observe(box);
  replay.addEventListener('click', play);
})();
