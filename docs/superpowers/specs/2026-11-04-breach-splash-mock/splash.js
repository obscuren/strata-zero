'use strict';
// Breach splash, round 1: four splash screens that play once at launch and hand over to
// the title. Every frame is a grid of terminal cells, painted as the window paints them:
// Fira Code, block and box characters as shapes, 30 ticks a second like the game.

// ---------- the game's colours
const BG = [10, 11, 16], BLACK = [0, 0, 0];
const TEXT = [205, 212, 222], DIM = [160, 170, 185], FAINT = [120, 130, 143], FAINTER = [92, 102, 115];
const WHITE = [255, 255, 255], CYAN = [0, 229, 255], MAGENTA = [255, 0, 200], TEAL = [0, 255, 230];
const RED = [255, 40, 40], AMBER = [255, 200, 80], MENU_GREY = [170, 180, 195], HINT = [100, 110, 125];
const GLITCH = [...'▓▒░█▀▄▌▐╳╬≋≈ｦｱﾂﾈ01/\\'];

const clamp01 = (x) => (x < 0 ? 0 : x > 1 ? 1 : x);
const mix = (a, b, f) => { f = clamp01(f); return [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f]; };
const scale = (c, f) => [Math.min(255, c[0] * f), Math.min(255, c[1] * f), Math.min(255, c[2] * f)];
const outCubic = (p) => 1 - Math.pow(1 - clamp01(p), 3);
const pulse = (t, period) => 0.5 + 0.5 * Math.sin((2 * Math.PI * t) / period);
const hex2 = (b) => b.toString(16).toUpperCase().padStart(2, '0');
function hash(a, b = 0, c = 0) {
  let h = Math.imul((a | 0) ^ 0x9e3779b9, 0x85ebca6b) ^ Math.imul((b | 0) + 0x632be5ab, 0xc2b2ae35) ^ Math.imul((c | 0) + 0x165667b1, 0x27d4eb2f);
  h ^= h >>> 16; h = Math.imul(h, 0x7feb352d); h ^= h >>> 15; h = Math.imul(h, 0x846ca68b); h ^= h >>> 16;
  return (h >>> 0) / 4294967296;
}
function typewriter(s, p) { const a = [...s]; return a.slice(0, Math.floor(a.length * clamp01(p))).join(''); }
const slideX = (w, p) => Math.round(-w * (1 - clamp01(p)));
// anim::effects::flicker, as the title's neon uses it.
function flicker(t, v, broken) {
  if (broken) {
    const ph = (Math.floor(t / 3) + v * 7) % 64;
    if ((ph >= 10 && ph <= 13) || ph === 20 || ph === 21 || ph === 40) return 0.05;
  }
  return 0.8 + 0.2 * Math.sin(t / 9 + v);
}

// ---------- a grid of cells
class Grid {
  constructor(w, h) {
    this.w = w; this.h = h; const n = w * h;
    this.ch = new Array(n); this.fg = new Array(n); this.bg = new Array(n); this.bold = new Uint8Array(n);
    this.clear();
  }
  clear(bg = BG) { this.ch.fill(' '); this.fg.fill(TEXT); this.bg.fill(bg); this.bold.fill(0); }
  set(x, y, ch, fg, bg, bold) {
    x = Math.round(x); y = Math.round(y);
    if (x < 0 || y < 0 || x >= this.w || y >= this.h) return;
    const i = y * this.w + x;
    if (ch != null) this.ch[i] = ch;
    if (fg) this.fg[i] = fg;
    if (bg) this.bg[i] = bg;
    if (bold != null) this.bold[i] = bold ? 1 : 0;
  }
  put(x, y, s, fg, bg, bold) { let j = 0; for (const c of s) { this.set(x + j, y, c, fg, bg, bold); j++; } return x + j; }
  fill(x, y, w, h, bg) { for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) this.set(x + i, y + j, ' ', TEXT, bg, 0); }
  copy() { const g = new Grid(this.w, this.h); g.ch = this.ch.slice(); g.fg = this.fg.slice(); g.bg = this.bg.slice(); g.bold = this.bold.slice(); return g; }
  blank(i, bg = BG) { this.ch[i] = ' '; this.fg[i] = TEXT; this.bg[i] = bg; this.bold[i] = 0; }
  take(i, src, j) { this.ch[i] = src.ch[j]; this.fg[i] = src.fg[j]; this.bg[i] = src.bg[j]; this.bold[i] = src.bold[j]; }
  shiftRow(y, d) {
    if (!d) return;
    const src = this.copy(), w = this.w;
    for (let x = 0; x < w; x++) { const sx = x - d; if (sx >= 0 && sx < w) this.take(y * w + x, src, y * w + sx); else this.blank(y * w + x); }
  }
  shift(dx, dy) {
    if (!dx && !dy) return;
    const src = this.copy(), w = this.w, h = this.h;
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
      const sx = x - dx, sy = y - dy;
      if (sx >= 0 && sy >= 0 && sx < w && sy < h) this.take(y * w + x, src, sy * w + sx); else this.blank(y * w + x);
    }
  }
}

// ---------- painting, as the window's PixelBackend does: shapes for blocks and boxes
const CW = 10, CH = 20, FPX = 16.6;
const cssCache = new Map();
function css(c) {
  const r = Math.max(0, Math.min(255, c[0] | 0)), g = Math.max(0, Math.min(255, c[1] | 0)), b = Math.max(0, Math.min(255, c[2] | 0));
  const k = (r << 16) | (g << 8) | b;
  let s = cssCache.get(k);
  if (!s) { s = '#' + k.toString(16).padStart(6, '0'); cssCache.set(k, s); }
  return s;
}
const RECTS = {
  '█': [[0, 0, 1, 1]], '▀': [[0, 0, 1, 0.5]], '▄': [[0, 0.5, 1, 0.5]], '▌': [[0, 0, 0.5, 1]], '▐': [[0.5, 0, 0.5, 1]],
  '▘': [[0, 0, 0.5, 0.5]], '▝': [[0.5, 0, 0.5, 0.5]], '▖': [[0, 0.5, 0.5, 0.5]], '▗': [[0.5, 0.5, 0.5, 0.5]],
  '▚': [[0, 0, 0.5, 0.5], [0.5, 0.5, 0.5, 0.5]], '▞': [[0.5, 0, 0.5, 0.5], [0, 0.5, 0.5, 0.5]],
  '▙': [[0, 0, 0.5, 1], [0.5, 0.5, 0.5, 0.5]], '▟': [[0.5, 0, 0.5, 1], [0, 0.5, 0.5, 0.5]],
  '▛': [[0, 0, 1, 0.5], [0, 0.5, 0.5, 0.5]], '▜': [[0, 0, 1, 0.5], [0.5, 0.5, 0.5, 0.5]],
  '▔': [[0, 0, 1, 0.125]], '▁': [[0, 0.875, 1, 0.125]], '▂': [[0, 0.75, 1, 0.25]], '▃': [[0, 0.625, 1, 0.375]],
  '▅': [[0, 0.375, 1, 0.625]], '▆': [[0, 0.25, 1, 0.75]], '▇': [[0, 0.125, 1, 0.875]],
  '▏': [[0, 0, 0.125, 1]], '▕': [[0.875, 0, 0.125, 1]],
};
const SHADES = { '░': 0.25, '▒': 0.5, '▓': 0.75 };
// Arms up, down, left, right: 1 light, 2 heavy, 3 double.
const BOX = {
  '─': [0, 0, 1, 1], '│': [1, 1, 0, 0], '┌': [0, 1, 0, 1], '┐': [0, 1, 1, 0], '└': [1, 0, 0, 1], '┘': [1, 0, 1, 0],
  '├': [1, 1, 0, 1], '┤': [1, 1, 1, 0], '┬': [0, 1, 1, 1], '┴': [1, 0, 1, 1], '┼': [1, 1, 1, 1],
  '╭': [0, 1, 0, 1], '╮': [0, 1, 1, 0], '╰': [1, 0, 0, 1], '╯': [1, 0, 1, 0],
  '━': [0, 0, 2, 2], '┃': [2, 2, 0, 0], '═': [0, 0, 3, 3], '║': [3, 3, 0, 0],
  '╔': [0, 3, 0, 3], '╗': [0, 3, 3, 0], '╚': [3, 0, 0, 3], '╝': [3, 0, 3, 0],
  '╠': [3, 3, 0, 3], '╣': [3, 3, 3, 0], '╦': [0, 3, 3, 3], '╩': [3, 0, 3, 3], '╬': [3, 3, 3, 3],
  '╵': [1, 0, 0, 0], '╷': [0, 1, 0, 0], '╴': [0, 0, 1, 0], '╶': [0, 0, 0, 1],
};
function drawBox(ctx, arms, X, Y) {
  const cx = X + CW / 2, cy = Y + CH / 2;
  const [u, d, l, r] = arms;
  const arm = (w, dir) => {
    if (!w) return;
    const t = w === 2 ? 2.6 : 1.3;
    for (const o of w === 3 ? [-2, 2] : [0]) {
      if (dir === 0) ctx.fillRect(cx - t / 2 + o, Y, t, CH / 2 + t / 2);
      if (dir === 1) ctx.fillRect(cx - t / 2 + o, cy - t / 2, t, CH / 2 + t / 2);
      if (dir === 2) ctx.fillRect(X, cy - t / 2 + o, CW / 2 + t / 2, t);
      if (dir === 3) ctx.fillRect(cx - t / 2, cy - t / 2 + o, CW / 2 + t / 2, t);
    }
  };
  arm(u, 0); arm(d, 1); arm(l, 2); arm(r, 3);
}
function shape(ctx, c, fg, bg, X, Y) {
  const r = RECTS[c];
  if (r) { ctx.fillStyle = css(fg); for (const [a, b, w, h] of r) ctx.fillRect(X + a * CW, Y + b * CH, w * CW, h * CH); return true; }
  const s = SHADES[c];
  if (s) { ctx.fillStyle = css(mix(bg, fg, s)); ctx.fillRect(X, Y, CW, CH); return true; }
  const bx = BOX[c];
  if (bx) { ctx.fillStyle = css(fg); drawBox(ctx, bx, X, Y); return true; }
  if (c === '╱' || c === '╲' || c === '╳') {
    ctx.strokeStyle = css(fg); ctx.lineWidth = 1.4; ctx.beginPath();
    if (c !== '╲') { ctx.moveTo(X, Y + CH); ctx.lineTo(X + CW, Y); }
    if (c !== '╱') { ctx.moveTo(X, Y); ctx.lineTo(X + CW, Y + CH); }
    ctx.stroke(); return true;
  }
  return false;
}
function paint(ctx, g) {
  for (let y = 0; y < g.h; y++) {
    let x = 0;
    while (x < g.w) {
      const b = css(g.bg[y * g.w + x]);
      let j = x + 1;
      while (j < g.w && css(g.bg[y * g.w + j]) === b) j++;
      ctx.fillStyle = b; ctx.fillRect(x * CW, y * CH, (j - x) * CW, CH);
      x = j;
    }
  }
  ctx.textBaseline = 'middle';
  let font = '';
  for (let y = 0; y < g.h; y++) for (let x = 0; x < g.w; x++) {
    const i = y * g.w + x, c = g.ch[i];
    if (c === ' ') continue;
    const X = x * CW, Y = y * CH;
    if (shape(ctx, c, g.fg[i], g.bg[i], X, Y)) continue;
    const f = (g.bold[i] ? '700 ' : '400 ') + FPX + 'px StrataFira, monospace';
    if (f !== font) { ctx.font = f; font = f; }
    ctx.fillStyle = css(g.fg[i]);
    ctx.fillText(c, X, Y + CH * 0.54);
  }
}

// ---------- the title, as screens/title.rs draws it
const GLYPH = {
  S: ['▄▀▀▀', '█   ', '▀▀▀▄', '   █', '▀▀▀ '], T: ['▀▀█▀▀', '  █  ', '  █  ', '  █  ', '  ▀  '],
  R: ['█▀▀▄', '█  █', '█▀▀▄', '█  █', '▀  ▀'], A: ['▄▀▀▄', '█  █', '█▀▀█', '█  █', '▀  ▀'],
  Z: ['▀▀▀▀█', '   █ ', '  █  ', ' █   ', '█▀▀▀▀'], E: ['█▀▀▀', '█   ', '█▀▀ ', '█   ', '▀▀▀▀'],
  O: ['▄▀▀▄', '█  █', '█  █', '█  █', '▀▀▀▀'], B: ['█▀▀▄', '█  █', '█▀▀▄', '█  █', '▀▀▀ '],
  C: ['▄▀▀▀', '█   ', '█   ', '█   ', '▀▀▀▀'], H: ['█  █', '█  █', '█▀▀█', '█  █', '▀  ▀'],
  ' ': ['    ', '    ', '    ', '    ', '    '],
};
const LOGO = 'STRATA ZERO';
const SUB = 'h i g h   t e c h   ·   l o w   l i f e';
const ITEMS = ['NEW GAME', 'LOAD GAME', 'QUIT'];
const HINT_TEXT = '↑↓ select · enter confirm · q quit';
const logoOrigin = (g) => ({ x: Math.floor((g.w - LOGO.length * 6) / 2), y: Math.floor(g.h / 6) });
function logoCells(g) {
  const o = logoOrigin(g), out = [];
  [...LOGO].forEach((c, i) => GLYPH[c].forEach((row, r) => [...row].forEach((ch, col) => {
    if (ch !== ' ') out.push({ x: o.x + i * 6 + col, y: o.y + r, ch, i, r });
  })));
  return out;
}
function logoColour(t, i, r) {
  let c = scale(MAGENTA, flicker(t, i, i % 7 === 3));
  if (r === Math.floor(t / 2) % 15) c = mix(c, WHITE, 0.5);
  return c;
}
function drawLogo(g, t) { for (const c of logoCells(g)) g.set(c.x, c.y, c.ch, logoColour(t, c.i, c.r)); }
// The title opened at `opened`: the subtitle types over 30 ticks, the menu slides in
// from 20 ticks on, an item every 4.
function drawTitle(g, t, opened, logo = true) {
  if (logo) drawLogo(g, t);
  const o = logoOrigin(g);
  g.put(Math.floor((g.w - [...SUB].length) / 2), o.y + 6, typewriter(SUB, (t - opened) / 30), CYAN);
  const my = o.y + 9, mx = Math.floor(g.w / 2) - 6;
  ITEMS.forEach((it, i) => {
    const p = outCubic((t - (opened + 20 + i * 4)) / 8);
    if (p <= 0) return;
    const sel = i === 0;
    const text = (sel ? '▸ ' : '  ') + typewriter(it, p);
    const off = slideX(it.length + 2, p);
    const fg = sel ? scale(TEAL, 0.6 + 0.4 * pulse(t, 30)) : MENU_GREY;
    let j = 0;
    for (const ch of text) { const x = mx + off + j; if (x >= mx) g.set(x, my + i, ch, fg, null, sel); j++; }
  });
  if (t >= opened) g.put(Math.floor((g.w - [...HINT_TEXT].length) / 2), g.h - 2, HINT_TEXT, HINT);
}
// The logo's letters lighting one after another from `start`, each stuttering on.
function drawLitLogo(g, t, start) {
  for (const c of logoCells(g)) {
    const on = start + c.i * 2;
    if (t < on) continue;
    const age = t - on;
    let fg = logoColour(t, c.i, c.r);
    if (age < 1) fg = WHITE;
    else if (age < 6 && hash(c.i, t, 3) < 0.45) fg = scale(fg, 0.2);
    g.set(c.x, c.y, c.ch, fg);
  }
}
// The logo font at twice the size: each cell two wide, its halves whole rows.
function bigCells(text, x0, y0) {
  const out = [];
  [...text].forEach((c, i) => GLYPH[c].forEach((row, r) => [...row].forEach((ch, col) => {
    if (ch === ' ') return;
    const top = ch === '█' || ch === '▀', bot = ch === '█' || ch === '▄';
    for (const dx of [0, 1]) {
      const x = x0 + (i * 6 + col) * 2 + dx;
      if (top) out.push({ x, y: y0 + r * 2, i });
      if (bot) out.push({ x, y: y0 + r * 2 + 1, i });
    }
  })));
  return out;
}
const PIX = {
  B: ['#######.', '########', '##....##', '##....##', '##...##.', '#######.', '#######.', '##...##.', '##....##', '##....##', '########', '#######.'],
  R: ['#######.', '########', '##....##', '##....##', '##....##', '########', '#######.', '##..##..', '##...##.', '##....##', '##....##', '##....##'],
  E: ['########', '########', '##......', '##......', '##......', '######..', '######..', '##......', '##......', '##......', '########', '########'],
  A: ['.######.', '########', '##....##', '##....##', '##....##', '########', '########', '##....##', '##....##', '##....##', '##....##', '##....##'],
  C: ['.#######', '########', '##......', '##......', '##......', '##......', '##......', '##......', '##......', '##......', '########', '.#######'],
  H: ['##....##', '##....##', '##....##', '##....##', '##....##', '########', '########', '##....##', '##....##', '##....##', '##....##', '##....##'],
};
// Cells of `text` in the pixel font: 9 columns a letter, 6 rows, each a half-block pair.
function pixCells(text, x0, y0) {
  const out = [];
  [...text].forEach((c, i) => {
    const px = PIX[c];
    for (let r = 0; r < 6; r++) for (let col = 0; col < 8; col++) {
      const top = px[r * 2][col] === '#', bot = px[r * 2 + 1][col] === '#';
      if (top || bot) out.push({ x: x0 + i * 9 + col, y: y0 + r, i, ch: top && bot ? '█' : top ? '▀' : '▄' });
    }
  });
  return out;
}
function panel(g, x, y, w, h, fg, title, bg = [6, 8, 13]) {
  g.fill(x, y, w, h, bg);
  g.put(x, y, '╭' + '─'.repeat(w - 2) + '╮', fg, bg);
  for (let j = 1; j < h - 1; j++) { g.set(x, y + j, '│', fg, bg); g.set(x + w - 1, y + j, '│', fg, bg); }
  g.put(x, y + h - 1, '╰' + '─'.repeat(w - 2) + '╯', fg, bg);
  if (title) g.put(x + 2, y, ' ' + title + ' ', fg, bg, true);
}
function bar(g, x, y, n, f, on, off) {
  const k = Math.round(n * clamp01(f));
  for (let i = 0; i < n; i++) g.set(x + i, y, i < k ? '█' : '░', i < k ? on : off);
}

// =====================================================================================
// A · HIJACK: the rig boots, and someone else finishes the boot.
// =====================================================================================
const BOOT = [
  { at: 4, s: 'ARASU·TEK RIGBIOS 4.17', fg: TEXT, bold: true },
  { at: 8, s: 'cold boot · 2 cores · 64 lanes', fg: FAINT },
  { at: 15, s: 'cortex bus', ok: 'ok' },
  { at: 20, s: 'optic feed L/R', ok: 'ok' },
  { at: 25, s: 'neural jack', ok: 'seated' },
  { at: 30, s: 'local ICE', ok: 'grey ×2' },
  { at: 37, s: 'netlink', ok: 'handshake', late: 48 },
  { at: 50, s: '  › relay 44.12.0.9', ok: 'ok' },
  { at: 55, s: '  › relay 44.12.0.10', ok: 'ok' },
  { at: 60, s: '  › relay ??.??.?.??', ok: '…', hang: true },
  { at: 80, s: '!! unsigned packet on jack port 7', fg: RED, bold: true },
  { at: 88, s: '!! local ICE layer 1 down', fg: RED },
  { at: 93, s: '!! local ICE layer 2 down', fg: RED },
  { at: 98, s: '!! optic feed: foreign source', fg: RED, bold: true },
];
function bootLines(t) {
  const lines = [];
  for (const e of BOOT) {
    if (t < e.at) break;
    const p = (t - e.at) / 4;
    if (!e.ok) { lines.push([[typewriter(e.s, p), e.fg, e.bold]]); continue; }
    const label = e.s, dots = ' ' + '.'.repeat(Math.max(2, 26 - [...label].length)) + ' ';
    const seg = [[typewriter(label, p), DIM], [typewriter(dots, (t - e.at - 3) / 2), FAINTER]];
    if (t >= e.at + 6) {
      if (e.hang) seg.push(['…', Math.floor(t / 6) % 2 ? AMBER : scale(AMBER, 0.4)]);
      else if (e.late) seg.push(t >= e.late ? ['ok', CYAN] : [e.ok + '…', AMBER]);
      else seg.push([e.ok, CYAN]);
    }
    lines.push(seg);
  }
  // Past 100 the jack spills garbage faster than it can scroll.
  const n = Math.max(0, Math.floor((t - 100) * 1.4));
  for (let m = 0; m < n; m++) {
    const k = 6 + Math.floor(hash(m, 1, 9) * 10);
    let s = '';
    for (let j = 0; j < k; j++) s += hex2(Math.floor(hash(m, j, 4) * 256)) + (j % 4 === 3 ? '  ' : ' ');
    const loud = hash(m, 2, 9) < 0.25;
    lines.push([[loud ? '!! ' + s : '   ' + s, loud ? RED : scale(RED, 0.55)]]);
  }
  return lines;
}
function collapse(g, p) {
  const src = g.copy(), W = g.w, H = g.h, c = Math.floor(H / 2);
  g.clear(BLACK);
  if (p < 0.7) {
    const s = 1 - (p / 0.7) * 0.96;
    for (let y = 0; y < H; y++) {
      const sy = Math.round(c + (y - c) / s);
      if (sy < 0 || sy >= H) continue;
      for (let x = 0; x < W; x++) {
        const i = y * W + x, j = sy * W + x;
        g.take(i, src, j);
        g.fg[i] = mix(g.fg[i], WHITE, p * 1.2);
        g.bg[i] = mix(g.bg[i], [40, 40, 52], p * 0.6);
      }
    }
  } else {
    const q = (p - 0.7) / 0.3, w = Math.max(1, Math.round(W * 0.95 * (1 - q)));
    const x0 = Math.floor((W - w) / 2);
    for (let i = 0; i < w; i++) {
      g.set(x0 + i, c, '━', WHITE);
      g.set(x0 + i, c - 1, '▁', scale(CYAN, 0.35));
      g.set(x0 + i, c + 1, '▔', scale(CYAN, 0.35));
    }
  }
}
const HIJACK = {
  key: 'A', T: 250, titleAt: 166,
  prepare() { return {}; },
  draw(g, t) {
    if (t >= 166) { drawLitLogo(g, t, 166); drawTitle(g, t, 166, false); return; }
    if (t >= 162) { g.clear(BLACK); return; }
    const W = g.w, H = g.h;
    const lines = bootLines(t), rows = H - 4, first = Math.max(0, lines.length - rows);
    for (let k = first; k < lines.length; k++) {
      let x = 4;
      for (const [s, fg, bold] of lines[k]) x = g.put(x, 2 + k - first, s, fg, null, bold);
      if (k === lines.length - 1 && t < 100 && Math.floor(t / 8) % 2 === 0) g.set(x, 2 + k - first, '█', TEXT);
    }
    // The tear: rows jump sideways in cyan or magenta, glyphs rot, red bars.
    const k = clamp01((t - 80) / 36), ph = Math.floor(t / 2);
    for (let y = 0; y < H; y++) {
      if (hash(y, ph, 1) < k * 0.35) {
        g.shiftRow(y, Math.round((hash(y, ph, 2) * 2 - 1) * 12 * k));
        const tint = t % 2 ? CYAN : MAGENTA;
        for (let x = 0; x < W; x++) { const i = y * W + x; if (g.ch[i] !== ' ') g.fg[i] = mix(g.fg[i], tint, 0.75); }
      }
    }
    for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
      const ink = g.ch[y * W + x] !== ' ';
      if (hash(x + y * 977, t, 3) < (ink ? k * 0.3 : k * k * 0.015)) {
        g.set(x, y, GLITCH[Math.floor(hash(x, y, t) * GLITCH.length)], hash(y, x, t) < 0.5 ? CYAN : MAGENTA);
      }
    }
    if (k > 0.3) for (let y = 0; y < H; y++) {
      if (hash(y, Math.floor(t / 3), 5) < 0.05 * k) {
        g.fill(0, y, W, 1, [70, 0, 14]);
        g.put(Math.floor(hash(y, t, 6) * (W - 14)), y, '▌ INTRUSION ▐', [255, 130, 140], [70, 0, 14], true);
      }
    }
    // BREACH, twice the logo's size, and the intruder's one line.
    if (t >= 112) {
      const bt = t - 112, bx = Math.floor((W - 53) / 2), by = Math.floor(H / 2) - 6;
      g.fill(bx - 6, by - 2, 65, 12, [12, 0, 6]);
      for (const c of pixCells('BREACH', bx, by)) {
        const dx = bt < 10 ? Math.round((hash(c.i, t, 7) * 2 - 1) * 3) : hash(c.i, t, 8) < 0.06 ? (hash(c.i, t, 9) < 0.5 ? -1 : 1) : 0;
        const fg = bt < 8 && t % 2 === 0 ? WHITE : mix(RED, [255, 150, 150], c.y === by ? 0.35 : 0);
        g.set(c.x + dx, c.y, c.ch, fg);
      }
      const line = "relax. we're only borrowing your eyes.";
      g.put(Math.floor((W - line.length) / 2), by + 8, typewriter(line, (bt - 10) / 18), TEXT);
      for (let y = by - 2; y < by + 10; y++) if (hash(y, ph, 11) < 0.12) g.shiftRow(y, hash(y, ph, 12) < 0.5 ? -2 : 2);
    }
    if (t >= 150) collapse(g, (t - 150) / 12);
  },
};

// =====================================================================================
// B · ICE WALL: a firewall builds down the screen, ICEPICK cracks it on the logo's spot,
// the wall shatters and falls away from the logo.
// =====================================================================================
const HITS = [36, 52, 66, 78], RADII = [8, 17, 30, 95], INTEG = [100, 71, 43, 16, 0], SHATTER = 88;
function wallCell(x, y, t) {
  const off = (Math.floor(y / 3) % 2) * 4, lx = (x + off) % 8, ly = y % 3;
  const bid = Math.floor((x + off) / 8) * 131 + Math.floor(y / 3);
  const s = (((x + y * 2 - t * 3) % 150) + 150) % 150, glow = s < 8 ? 1 + 0.8 * (1 - s / 8) : 1;
  const face = scale([6, 34, 50], glow), mortar = [2, 10, 16];
  if (ly === 2 || lx === 0) return [' ', TEXT, mortar];
  if (ly === 0) return ['▀', scale([16, 74, 100], glow), face];
  if (ly === 1 && (lx === 3 || lx === 4)) return [hex2(Math.floor(hash(bid, 7, 1) * 256))[lx - 3], scale([0, 130, 160], glow), face];
  return [' ', TEXT, face];
}
const ICEWALL = {
  key: 'B', T: 250, titleAt: 118,
  // Radial cracks from the impact and broken rings round it, as glass breaks; the
  // wall's shards are the pieces between them.
  prepare(W, H) {
    const P = { x: Math.floor(W / 2), y: Math.floor(H / 6) + 2 }, TAU = Math.PI * 2, NC = 9;
    const RINGS = [5, 11, 19, 30, 45, 66, 95];
    const base = Array.from({ length: NC }, (_, j) => (j * TAU) / NC + (hash(j, 41) - 0.5) * 0.5);
    const polar = (x, y) => { const dx = x - P.x, dy = (y - P.y) * 2; return [Math.hypot(dx, dy), Math.atan2(dy, dx)]; };
    const sectorOf = (rho, th) => {
      let best = 0, bd = Infinity;
      for (let j = 0; j < NC; j++) {
        const a = base[j] + 0.22 * Math.sin(rho * 0.12 + j * 1.7);
        const d = (((th - a) % TAU) + TAU) % TAU;
        if (d < bd) { bd = d; best = j; }
      }
      return best;
    };
    const ringOf = (rho, th) => RINGS.filter((r, k) => rho > r * (1 + 0.12 * Math.sin(3 * th + k * 2.1))).length;
    const n = W * H, dist = new Float32Array(n), sector = new Int8Array(n), ring = new Int8Array(n), theta = new Float32Array(n);
    for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
      const i = y * W + x, [rho, th] = polar(x, y);
      dist[i] = rho; theta[i] = th; sector[i] = sectorOf(rho, th); ring[i] = ringOf(rho, th);
    }
    const along = (psi) => {
      const c = Math.cos(psi), s = Math.sin(psi);
      if (Math.abs(s) < 0.6) return '─';
      if (Math.abs(c) < 0.45) return '│';
      return c * s > 0 ? '╲' : '╱';
    };
    const crack = new Array(n).fill(null);
    for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
      const i = y * W + x;
      const nb = [x + 1 < W ? i + 1 : -1, y + 1 < H ? i + W : -1].filter((j) => j >= 0);
      if (nb.some((j) => sector[j] !== sector[i])) crack[i] = along(theta[i]);
      else if (nb.some((j) => ring[j] !== ring[i]) && ring[i] < 4 && hash(sector[i], ring[i], 5) < 0.75) crack[i] = along(theta[i] + Math.PI / 2);
    }
    const chunks = new Map();
    for (let i = 0; i < n; i++) {
      const id = sector[i] * 10 + ring[i];
      if (!chunks.has(id)) {
        const mid = base[sector[i]] + Math.PI / NC, r = ring[i] ? RINGS[ring[i] - 1] : 0;
        chunks.set(id, { cells: [], start: SHATTER + r * 0.16 + hash(id, 5) * 4, vx: Math.cos(mid) * 0.45, vy: -0.25 + Math.sin(mid) * 0.2 });
      }
      chunks.get(id).cells.push(i);
    }
    return { P, dist, crack, chunks: [...chunks.values()] };
  },
  draw(g, t, Z) {
    const W = g.w, H = g.h, { P, dist, crack, chunks } = Z;
    if (t >= SHATTER) drawLogo(g, t);
    if (t >= this.titleAt) drawTitle(g, t, this.titleAt, false);
    let hit = -1, R = 0;
    HITS.forEach((h, k) => { if (t >= h) { hit = k; R = (k ? RADII[k - 1] : 0) + (RADII[k] - (k ? RADII[k - 1] : 0)) * outCubic((t - h) / 5); } });
    const built = t * 2.2, glowing = t >= 80 && t < SHATTER;
    const cellAt = (i) => {
      const x = i % W, y = (i / W) | 0;
      let [ch, fg, bg] = wallCell(x, y, t);
      if (y > built - 1.5) bg = mix(bg, CYAN, 0.6);
      if (crack[i] && dist[i] < R) {
        ch = crack[i];
        fg = glowing ? scale(MAGENTA, 0.7 + 0.3 * pulse(t, 6)) : mix(WHITE, CYAN, dist[i] / Math.max(R, 1));
      }
      return [x, y, ch, fg, bg];
    };
    const falling = [];
    for (const c of chunks) {
      const dt = t - c.start;
      for (const i of c.cells) {
        if (((i / W) | 0) >= built) continue;
        if (dt > 0) { falling.push([i, c, dt]); continue; }
        const [x, y, ch, fg, bg] = cellAt(i);
        g.set(x, y, ch, fg, bg);
      }
    }
    for (const [i, c, dt] of falling) {
      let [x, y, ch, fg, bg] = cellAt(i);
      const ny = y + c.vy * dt + 0.1 * dt * dt, nx = x + c.vx * dt;
      if (ny >= H || ny < 0) continue;
      const f = clamp01(dt / 24);
      g.set(nx, ny, ch, mix(fg, BG, f), mix(bg, BG, f));
    }
    // Each hit: a white flash on the spot and a ring running out from it.
    for (const h of HITS) {
      const dt = t - h;
      if (dt < 0 || dt >= 8) continue;
      const rr = dt * 3.4;
      for (let i = 0; i < W * H; i++) {
        if (dt < 2 && dist[i] < 3) g.bg[i] = WHITE;
        else if (Math.abs(dist[i] - rr) < 1.3) g.bg[i] = mix(g.bg[i], mix(CYAN, WHITE, 0.5), 0.6 * (1 - dt / 8));
      }
    }
    // ICEPICK's readout.
    if (t >= 12 && t < 108) {
      const integ = hit < 0 ? 100 : Math.round(INTEG[hit] + (INTEG[hit + 1] - INTEG[hit]) * outCubic((t - HITS[hit]) / 4));
      const broken = t >= SHATTER;
      const fg = broken ? MAGENTA : CYAN;
      panel(g, 3, 1, 38, 4, scale(fg, 0.7), 'ICE');
      g.put(5, 2, broken ? 'BREACHED' : `GREY · LAYER ${Math.max(1, Math.min(4, hit + 2))}/4`, broken ? MAGENTA : TEXT, null, true);
      g.put(5, 3, 'INTEGRITY', FAINT);
      bar(g, 15, 3, 16, integ / 100, integ < 30 ? RED : CYAN, [30, 50, 60]);
      g.put(32, 3, (String(integ) + '%').padStart(5), integ < 30 ? RED : TEXT);
      const msg = broken ? "› ICE down. you're in." : hit < 0 ? '› ICEPICK 2.1 · jack port 7 · aiming' : `› ICEPICK 2.1 · jack port 7 · hit ${hit + 1}/4`;
      g.fill(3, H - 2, msg.length + 2, 1, [6, 8, 13]);
      g.put(4, H - 2, typewriter(msg, (t - 14) / 10), broken ? MAGENTA : DIM);
    }
    for (const h of HITS) { const dt = t - h; if (dt >= 1 && dt <= 4) g.shift([0, 2, -2, 1, -1][dt], 0); }
  },
};

// =====================================================================================
// C · DRILL: down through the city's strata, a slab at a time, to stratum zero.
// =====================================================================================
const BAND = 40, SLAB = 4, PER = BAND + SLAB, ZERO_AT = 4 * PER;
const STRATA = ['HEIGHTS', 'SKYWAYS', 'STREET', 'UNDERCITY'];
const IMP = [40, 66, 86, 102];
const metres = (wy) => Math.round(470 - wy * 3);
function towers(x, W) {
  const slot = Math.floor((x + W) / 7), sx = (x + W) % 7;
  return { slot, sx, top: 10 + Math.floor(hash(slot, 1, 5) * 24) };
}
function heights(x, r, t, W) {
  const bg = mix([5, 7, 20], [18, 10, 34], r / BAND), { slot, sx, top } = towers(x, W);
  const ad = ' ◆ ORODYNE ', ax = Math.floor(W * 0.6);
  if (r === 29 && x >= ax && x < ax + ad.length) return [ad[x - ax], MAGENTA, [40, 0, 34]];
  if (sx !== 0 && r >= top) {
    const body = [16, 20, 34];
    if (r === top) return ['▄', [44, 50, 74], bg];
    if (sx % 2 === 1 && r % 2 === 0 && hash(slot * 13 + sx, r, 6) < 0.3) return ['▄', hash(slot, r, 8) < 0.5 ? [150, 110, 50] : [40, 120, 140], body];
    return [' ', TEXT, body];
  }
  if (sx === 3 && top < 24 && r >= top - 4 && r < top) {
    if (r === top - 4) return ['▀', Math.floor(t / 12) % 2 ? RED : [90, 20, 20], bg];
    return ['│', [70, 76, 96], bg];
  }
  if (r < 22 && hash(x, r, 7) < 0.025) return ['·', hash(x, r, 8) < 0.5 ? FAINT : DIM, bg];
  return [' ', TEXT, bg];
}
function skyways(x, r, t, W) {
  const bg = [8, 9, 22], { slot, sx } = towers(x, W);
  const ad = ' ARASU·TEK ', ax = Math.floor(W * 0.2);
  if (r === 16 && x >= ax && x < ax + ad.length) return [ad[x - ax], TEAL, [0, 30, 36], true];
  for (const br of [9, 23, 33]) {
    if (r === br) return ['═', [0, 150, 140], bg];
    if (r === br + 1 && hash(x, br, 2) < 0.15) return ['▀', [150, 110, 50], bg];
    if (r === br - 1 && ((x - Math.floor(t * 1.4)) % 70 + 70) % 70 < 10) return ['▄', ((x % 2) ? MAGENTA : mix(MAGENTA, WHITE, 0.4)), bg];
  }
  if (sx > 1 && sx < 6 && hash(slot, 9) < 0.6) {
    const body = [14, 17, 30];
    if (sx % 2 === 1 && r % 2 === 0 && hash(slot * 13 + sx, r, 16) < 0.25) return ['▄', [40, 120, 140], body];
    return [' ', TEXT, body];
  }
  return [' ', TEXT, bg];
}
const SIGNS = [['NOODLES', MAGENTA], ['BAR', CYAN], ['24H', AMBER], ['CLINIC', TEAL], ['PACHINKO', MAGENTA]];
function street(x, r, t, W) {
  const bg = [10, 8, 16], { slot, sx } = towers(x, W);
  if (r === 36) return ['▀', [70, 70, 80], [24, 24, 30]];
  if (r > 36) return hash(x, r, 3) < 0.2 ? ['≈', [40, 70, 110], [16, 16, 20]] : [' ', TEXT, [16, 16, 20]];
  const sign = SIGNS[slot % SIGNS.length];
  if (sx === 3 && hash(slot, 4) < 0.7) {
    const top = 14 + (slot % 5) * 2, k = r - top;
    if (k >= 0 && k < sign[0].length) return [sign[0][k], sign[1], scale(sign[1], 0.14), true];
  }
  if (sx !== 0 && r >= 26) {
    if (r === 26) return ['▄', [60, 40, 70], bg];
    if (r >= 33 && sx >= 2 && sx <= 5) return ['▀', scale(sign[1], 0.35), [14, 10, 20]];
    return [' ', TEXT, [18, 16, 26]];
  }
  if (hash(x, Math.floor(r - t * 1.6), 9) < 0.05) return ['│', [50, 70, 110], bg];
  return [' ', TEXT, bg];
}
function undercity(x, r, t, W) {
  const bg = [8, 10, 10];
  const vp = x % 17 === 3, hp = r === 5 || r === 18 || r === 29;
  if (r >= 34) return [((x + Math.floor(t / 3)) % 3 === 0) ? '≈' : '~', [30, 80, 120], [4, 14, 24]];
  const g = ' STRATUM 0 ↓ ', gx = Math.floor(W * 0.62);
  if (r === 24 && x >= gx && x < gx + g.length) return [g[x - gx], AMBER, bg, true];
  if (vp && hp) return ['╬', [160, 90, 45], bg];
  if (hp) return ['═', [150, 80, 40], bg];
  if (vp) return ['║', [110, 60, 30], bg];
  if (x % 11 === 7 && (r + Math.floor(t / 2)) % 9 === 0) return ['╷', [60, 100, 140], bg];
  return [' ', TEXT, bg];
}
const BANDS = [heights, skyways, street, undercity];
const DRILL = {
  key: 'C', T: 210, titleAt: 114,
  prepare(W, H) {
    const head = Math.floor(H / 2) - 3;
    const slabCam = (k) => k * PER + BAND - head - 1;
    const keys = [[0, 0], [14, 0], [IMP[0], slabCam(0), 'in'], [48, slabCam(0)], [IMP[1], slabCam(1), 'in'], [72, slabCam(1)],
      [IMP[2], slabCam(2), 'in'], [91, slabCam(2)], [IMP[3], slabCam(3), 'in'], [106, slabCam(3)], [114, ZERO_AT, 'out']];
    return { head, keys };
  },
  cam(t, keys) {
    for (let i = 1; i < keys.length; i++) {
      if (t <= keys[i][0]) {
        const a = keys[i - 1], b = keys[i], p = clamp01((t - a[0]) / (b[0] - a[0]));
        const e = b[2] === 'in' ? p * p : b[2] === 'out' ? outCubic(p) : p;
        return a[1] + (b[1] - a[1]) * e;
      }
    }
    return keys[keys.length - 1][1];
  },
  world(x, wy, t, W) {
    if (wy < 0) return [' ', TEXT, [5, 7, 20]];
    if (wy >= ZERO_AT) return [' ', TEXT, BG];
    const k = Math.floor(wy / PER), r = wy % PER;
    if (r < BAND) return BANDS[k](x, r, t, W);
    const s = r - BAND, dt = t - IMP[k], hole = dt < 0 ? 0 : Math.min(9, dt * 3), dx = Math.abs(x - Math.floor(W / 2));
    if (dx < hole) return [' ', TEXT, [0, 0, 0]];
    let cell;
    if (dx < hole + 1 && hole > 0) cell = [x < W / 2 ? '▌' : '▐', [96, 100, 110], [0, 0, 0]];
    else if (s === 0) cell = ['▀', [96, 100, 110], [38, 40, 46]];
    else if (s === SLAB - 1) cell = ['▄', [18, 19, 23], [38, 40, 46]];
    else { const h = hash(x, k * 7 + s, 13); cell = h < 0.08 ? ['▒', [62, 64, 72], [38, 40, 46]] : h < 0.11 ? ['═', [140, 80, 40], [38, 40, 46]] : [' ', TEXT, [38, 40, 46]]; }
    if (dt >= 0 && dt < 2) cell = [cell[0], cell[1], mix(cell[2], WHITE, 0.85)];
    return cell;
  },
  draw(g, t, Z) {
    const W = g.w, H = g.h, { head, keys } = Z;
    const c = this.cam(t, keys), v = c - this.cam(t - 1, keys), c0 = Math.floor(c), mid = Math.floor(W / 2);
    for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
      const wy = c0 + y;
      let [ch, fg, bg, bold] = this.world(x, wy, t, W);
      if (v > 1.2 && wy < ZERO_AT) {
        if (ch !== ' ') { ch = v > 3 ? '┃' : '│'; fg = scale(fg, 0.9); }
        else if (hash(x, wy, t) < Math.min(0.07, v * 0.012)) { ch = '│'; fg = mix(bg, WHITE, 0.25); }
      }
      g.set(x, y, ch, fg, bg, bold);
    }
    // The drill: a beam down the middle and its head just above the next slab.
    if (t < 110) {
      for (let y = 0; y < head - 1; y++) g.set(mid, y, '┃', mix(CYAN, BG, 0.35 + 0.25 * hash(y, t, 2)));
      g.set(mid - 1, head - 1, '▐', CYAN); g.set(mid, head - 1, '█', WHITE); g.set(mid + 1, head - 1, '▌', CYAN);
      g.set(mid, head, '▀', mix(CYAN, WHITE, pulse(t, 4)));
    }
    // Each slab it hits: sparks and a tag.
    IMP.forEach((imp, k) => {
      const dt = t - imp;
      if (dt < 0 || dt >= 14) return;
      for (let j = 0; j < 22; j++) {
        if (dt >= 10) break;
        const a = Math.PI + hash(j, k, 1) * Math.PI, sp = 0.6 + hash(j, k, 2) * 1.4;
        g.set(mid + Math.cos(a) * sp * dt * 2.2, head + 1 + Math.sin(a) * sp * dt + 0.14 * dt * dt, hash(j, k, 3) < 0.5 ? '·' : '*', mix(AMBER, WHITE, hash(j, k, 4)));
      }
      if (k < 3 || dt < 6) g.put(mid + 12, head + 1, ` BREACH ${k + 1}/4 `, BLACK, MAGENTA, true);
    });
    // The depth gauge, left.
    for (let y = 0; y < H; y++) {
      const wy = c0 + y;
      if (wy >= ZERO_AT) continue;
      if (wy % 3 === 0) g.set(1, y, '─', FAINTER);
      if (wy % 12 === 0) { const m = metres(wy); g.put(3, y, (m > 0 ? '+' : '') + m + ' m', FAINT); }
    }
    if (t < 112) g.set(0, head, '▸', CYAN);
    // The stratum, right; it counts down to zero.
    const k = Math.floor((c0 + head) / PER);
    if (t < 128) {
      const zero = k >= 4 || t >= 106;
      const fg = zero ? MAGENTA : CYAN, px = W - 27;
      panel(g, px, 1, 25, 4, scale(fg, 0.7));
      g.put(px + 2, 2, `STRATUM ${zero ? 0 : 4 - k}`, zero ? MAGENTA : WHITE, null, true);
      g.put(px + 2, 3, zero ? 'ZERO' : STRATA[k], fg);
      if (!zero) { const m = metres(c0 + head); g.put(px + 14, 3, ((m > 0 ? '+' : '') + m + ' m').padStart(8), FAINT); }
    }
    // Zero: the logo slams in.
    if (t >= 112) {
      if (t < 114) for (const cc of logoCells(g)) g.set(cc.x, cc.y, cc.ch, WHITE);
      else drawLogo(g, t);
      drawTitle(g, t, this.titleAt, false);
    }
    for (const imp of [...IMP, 112]) { const dt = t - imp; if (dt >= 0 && dt <= 3) g.shift(0, [1, -1, 1, 0][dt]); }
  },
};

// =====================================================================================
// D · KEY: the scanner cracks a key in a memory dump; the key is the name, and its
// bytes fly up into the logo.
// =====================================================================================
const KEY = [0x53, 0x54, 0x52, 0x41, 0x54, 0x41, 0x20, 0x5a, 0x45, 0x52, 0x4f];
const KROW = 640, DUMP_SPAN = 560, FOUND = 70;
const byteX = (j) => 12 + j * 3 + (j >= 8 ? 1 : 0);
const addr = (n) => '0x' + (0xa1f000 + n * 16).toString(16).toUpperCase().padStart(8, '0');
function rowBytes(n) {
  if (n === KROW) return [0x00, 0x1c, 0x7f, ...KEY, 0x00, 0x00];
  return Array.from({ length: 16 }, (_, j) => (hash(n, j, 17) < 0.28 ? 0 : Math.floor(hash(n, j, 3) * 256)));
}
const KEYDUMP = {
  key: 'D', T: 230, titleAt: 134,
  prepare(W, H) {
    const X0 = Math.floor((W - 79) / 2), keyY = Math.floor(H / 2) + 4, top = 2, bottom = H - 4;
    const parts = logoCells({ w: W, h: H }).map((c, k) => {
      const hx = hex2(KEY[c.i]), side = hash(k, 1) < 0.5 ? 0 : 1;
      return { ...c, sx: X0 + byteX(3 + c.i) + side, sy: keyY, digit: hx[side], start: 92 + c.i * 2.2 + hash(k, 2) * 8, dur: 14 + hash(k, 3) * 10, arc: 2 + hash(k, 4) * 5 };
    });
    return { X0, keyY, top, bottom, parts, sEnd: KROW - (keyY - top) };
  },
  draw(g, t, Z) {
    const W = g.w, H = g.h, { X0, keyY, top, bottom, parts, sEnd } = Z;
    const s = t < FOUND ? sEnd - DUMP_SPAN * Math.pow(1 - t / FOUND, 3) : sEnd;
    const v = t < FOUND ? (3 * DUMP_SPAN * Math.pow(1 - t / FOUND, 2)) / FOUND : 0;
    const sI = Math.floor(s), found = t >= FOUND;
    if (t < 140) {
      g.put(X0, 0, 'ICEPICK 2.1  ·  memory read  ·  jack port 7', FAINT);
      if (!found) { g.fill(X0 - 2, keyY, 83, 1, [0, 28, 38]); g.set(X0 - 2, keyY, '▶', CYAN); }
      else g.fill(X0 - 2, keyY, 83, 1, t < 72 ? [120, 60, 110] : [46, 0, 38]);
      for (let y = top; y <= bottom; y++) {
        const n = sI + (y - top), bytes = rowBytes(n), isKey = n === KROW && found;
        const base = v > 6 ? FAINTER : v > 2 ? FAINT : DIM;
        g.put(X0, y, addr(n), scale(CYAN, v > 6 ? 0.35 : 0.55));
        bytes.forEach((b, j) => {
          const k = isKey && j >= 3 && j < 14;
          let fg = k ? MAGENTA : b === 0 ? scale(base, 0.6) : base;
          if (!found && hash(n, j, 11) < 0.025) fg = MAGENTA;
          g.put(X0 + byteX(j), y, hex2(b), fg, null, k);
        });
        g.set(X0 + 61, y, '│', FAINTER);
        bytes.forEach((b, j) => {
          const k = isKey && j >= 3 && j < 14;
          g.set(X0 + 62 + j, y, b >= 0x20 && b < 0x7f ? String.fromCharCode(b) : '.', k ? WHITE : FAINTER, null, k);
        });
        g.set(X0 + 78, y, '│', FAINTER);
      }
      if (found) g.put(X0 + 80, keyY, '◀ KEY', MAGENTA, null, true);
      // The scanner's status line: the key fills in a byte at a time.
      const sy = H - 2;
      if (!found) {
        let x = g.put(X0, sy, 'SCAN ', FAINT);
        x = g.put(x, sy, addr(sI + (keyY - top)) + '  ', DIM);
        bar(g, x, sy, 16, t / FOUND, CYAN, [30, 40, 50]); x += 17;
        x = g.put(x, sy, (Math.floor((100 * t) / FOUND) + '%').padStart(4) + '   KEY ', FAINT);
        const known = Math.floor(11 * clamp01((t - 6) / (FOUND - 10)));
        KEY.forEach((b, j) => { x = g.put(x, sy, (j < known ? hex2(b) : '??') + ' ', j < known ? MAGENTA : FAINTER, null, j < known); });
      } else {
        let x = g.put(X0, sy, 'KEY FOUND  ', MAGENTA, null, true);
        x = g.put(x, sy, addr(KROW) + '+03   ', DIM);
        KEY.forEach((b) => { x = g.put(x, sy, hex2(b) + ' ', MAGENTA, null, true); });
      }
    }
    // The dump dissolves around the key; then the key's own bytes go as they fly.
    if (t >= 84) {
      const keyX0 = X0 + byteX(3), keyX1 = X0 + byteX(13) + 1, ascX0 = X0 + 65, ascX1 = X0 + 75;
      for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
        const i = y * W + x;
        const prot = y === keyY && ((x >= keyX0 && x <= keyX1) || (x >= ascX0 && x <= ascX1));
        const vt = prot ? 124 + hash(x, y, 22) * 14 : 84 + hash(x, y, 21) * 20;
        if (t >= vt) g.blank(i);
        else if (t >= vt - 2 && g.ch[i] !== ' ') { g.ch[i] = GLITCH[Math.floor(hash(x, y, t) * GLITCH.length)]; g.fg[i] = scale(CYAN, 0.6); }
      }
    }
    // The bytes fly up and become the logo.
    for (const p of parts) {
      const q = (t - p.start) / p.dur;
      if (q < 0) continue;
      if (q < 1) {
        const e = outCubic(q), e1 = outCubic((t - 1 - p.start) / p.dur);
        const at = (e) => [p.sx + (p.x - p.sx) * e, p.sy + (p.y - p.sy) * e - Math.sin(Math.PI * e) * p.arc];
        const [tx, ty] = at(e1);
        g.set(tx, ty, '·', scale(MAGENTA, 0.45));
        const [x, y] = at(e);
        g.set(x, y, p.digit, mix(MAGENTA, WHITE, 0.35), null, true);
      } else {
        const age = t - (p.start + p.dur);
        g.set(p.x, p.y, p.ch, age < 2 ? WHITE : logoColour(t, p.i, p.r));
      }
    }
    drawTitle(g, t, this.titleAt, false);
  },
};

// ---------- players
const CONCEPTS = { A: HIJACK, B: ICEWALL, C: DRILL, D: KEYDUMP };
const players = [];
class Player {
  constructor(el, concept) {
    this.c = concept; this.el = el; this.t = 0; this.playing = true; this.speed = 1; this.visible = true; this.last = -1;
    el.innerHTML = `<div class="screen"><canvas title="click: skip to the title, as a key would"></canvas></div>
      <div class="ctl"><button data-a="replay">⟲ replay</button><button data-a="play">❚❚ pause</button>
      <input type="range" min="0" max="${concept.T - 1}" value="0"><span class="tm">0.00 s</span>
      <span class="seg"><button data-a="size" data-w="120" data-h="36" class="on">120×36</button><button data-a="size" data-w="100" data-h="30">100×30</button></span>
      <span class="seg"><button data-a="speed" data-s="1" class="on">1×</button><button data-a="speed" data-s="0.25">¼×</button></span>
      <button data-a="skip">skip ⏭</button></div>`;
    this.canvas = el.querySelector('canvas'); this.ctx = this.canvas.getContext('2d');
    this.range = el.querySelector('input'); this.tm = el.querySelector('.tm'); this.playBtn = el.querySelector('[data-a=play]');
    this.range.addEventListener('input', () => { this.t = +this.range.value; this.setPlaying(false); this.draw(); });
    this.canvas.addEventListener('click', () => this.skip());
    el.querySelectorAll('button').forEach((b) => b.addEventListener('click', () => {
      const a = b.dataset.a;
      if (a === 'replay') { this.t = 0; this.setPlaying(true); }
      if (a === 'play') this.setPlaying(!this.playing);
      if (a === 'skip') this.skip();
      if (a === 'size') { b.parentNode.querySelectorAll('button').forEach((x) => x.classList.toggle('on', x === b)); this.resize(+b.dataset.w, +b.dataset.h); }
      if (a === 'speed') { b.parentNode.querySelectorAll('button').forEach((x) => x.classList.toggle('on', x === b)); this.speed = +b.dataset.s; }
      this.draw();
    }));
    this.resize(120, 36);
  }
  skip() { if (this.t < this.c.titleAt) this.t = this.c.titleAt; this.setPlaying(true); this.draw(); }
  setPlaying(p) { this.playing = p; this.playBtn.textContent = p ? '❚❚ pause' : '▶ play'; }
  resize(w, h) {
    this.g = new Grid(w, h); this.Z = this.c.prepare(w, h);
    const dpr = window.devicePixelRatio || 1;
    this.canvas.width = w * CW * dpr; this.canvas.height = h * CH * dpr;
    this.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    this.canvas.style.aspectRatio = `${w * CW} / ${h * CH}`;
    this.canvas.style.maxWidth = `${w * CW}px`;
    this.last = -1; this.draw();
  }
  draw() {
    const t = Math.floor(this.t);
    this.g.clear(BG); this.c.draw(this.g, t, this.Z); paint(this.ctx, this.g);
    this.range.value = t; this.tm.textContent = (t / 30).toFixed(2) + ' s'; this.last = t;
  }
  advance(ms) {
    if (!this.playing || !this.visible) return;
    this.t += (ms / 1000) * 30 * this.speed;
    if (this.t >= this.c.T) this.t = 0;
    if (Math.floor(this.t) !== this.last) this.draw();
  }
}
// For checking frames from outside: window.frameAt('A', 120)
window.frameAt = (k, t) => { const p = players.find((p) => p.c.key === k); p.t = t; p.setPlaying(false); p.draw(); return p; };

// Checking: #sheet=A:70,A:110:100:30 draws those frames side by side, nothing animated.
function sheet(spec) {
  document.body.innerHTML = '';
  document.body.style.cssText = 'margin:0;padding:0;background:#000;max-width:none';
  for (const item of spec.split(',')) {
    const [k, t, w, h] = item.split(':'), W = +(w || 120), H = +(h || 36), c = CONCEPTS[k];
    const g = new Grid(W, H), Z = c.prepare(W, H), cv = document.createElement('canvas');
    cv.width = W * CW; cv.height = H * CH;
    g.clear(BG); c.draw(g, +t, Z); paint(cv.getContext('2d'), g);
    const d = document.createElement('div');
    d.style.cssText = 'display:inline-block;margin:4px;color:#888;font:14px monospace;vertical-align:top';
    d.append(item, document.createElement('br'), cv);
    document.body.append(d);
  }
}

document.fonts.load('16px StrataFira').then(() => document.fonts.load('700 16px StrataFira')).then(() => {
  if (location.hash.startsWith('#sheet=')) { sheet(decodeURIComponent(location.hash.slice(7))); return; }
  document.querySelectorAll('[data-concept]').forEach((el) => players.push(new Player(el, CONCEPTS[el.dataset.concept])));
  const io = new IntersectionObserver((es) => es.forEach((e) => { const p = players.find((p) => p.el === e.target); if (p) p.visible = e.isIntersecting; }));
  players.forEach((p) => io.observe(p.el));
  let last = performance.now();
  const loop = (now) => { const dt = Math.min(100, now - last); last = now; players.forEach((p) => p.advance(dt)); requestAnimationFrame(loop); };
  requestAnimationFrame(loop);
});
