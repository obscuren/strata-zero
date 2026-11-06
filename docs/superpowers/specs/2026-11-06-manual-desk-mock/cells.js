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
