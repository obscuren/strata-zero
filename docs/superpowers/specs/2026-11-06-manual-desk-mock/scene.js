'use strict';
// The MANUAL's opening scene: you, at your desk, a high floor, the screen in front of a
// window over the city at night. Three ways to frame it (A, B, C). Every frame is a grid
// of terminal cells; the art is half-block pixels, two to a cell, as the game draws its
// pictures. 30 ticks a second. `space` leans in to the screen and the booklet; `esc`
// leans back. The room and what's in it are in room.js.

// ---------- a seeded random
function mulberry32(a) {
  return () => {
    a |= 0; a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const add = (a, b, f = 1) => [a[0] + b[0] * f, a[1] + b[1] * f, a[2] + b[2] * f];
const mul = (a, f) => [a[0] * f, a[1] * f, a[2] * f];
const ease = (p) => (p < 0.5 ? 4 * p * p * p : 1 - Math.pow(-2 * p + 2, 3) / 2);

// ---------- the palette
const SKY_TOP = [6, 5, 18], SKY_LOW = [64, 20, 66];
const FAR = [46, 30, 74], MID = [24, 18, 44], NEAR = [11, 9, 20];
const LIGHTS = [[255, 196, 110], [255, 214, 150], [120, 220, 255], [255, 90, 200], [230, 230, 240]];
const WALL = [17, 17, 22], SCREEN = [5, 9, 15];

// ---------- pixels: two to a cell, folded into half blocks at the end
class Px {
  constructor(w, h) { this.w = w; this.h = h; this.c = new Array(w * h).fill(WALL); this.glass = new Uint8Array(w * h); }
  ok(x, y) { return x >= 0 && y >= 0 && x < this.w && y < this.h; }
  get(x, y) { x = Math.round(x); y = Math.round(y); return this.ok(x, y) ? this.c[y * this.w + x] : SCREEN; }
  set(x, y, c, glass = 0) { x = Math.round(x); y = Math.round(y); if (this.ok(x, y)) { this.c[y * this.w + x] = c; this.glass[y * this.w + x] = glass; } }
  tint(x, y, c, f) { x = Math.round(x); y = Math.round(y); if (this.ok(x, y)) this.c[y * this.w + x] = mix(this.c[y * this.w + x], c, f); }
  rect(x, y, w, h, c, glass = 0) { for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) this.set(x + i, y + j, c, glass); }
}

// ---------- the city through the glass: three rows of towers, their windows, signs,
// an ad screen, the cars in the air, the rain
function makeCity(L) {
  const r = mulberry32(L.seed * 977 + L.w);
  const { win } = L;
  const towers = [];
  const rows = [
    { layer: 0, lo: 0.30, hi: 0.62, wmin: 4, wmax: 9, gap: 1 },
    { layer: 1, lo: 0.40, hi: 0.82, wmin: 5, wmax: 11, gap: 2 },
    { layer: 2, lo: 0.25, hi: 0.70, wmin: 8, wmax: 16, gap: 5 },
  ];
  for (const row of rows) {
    let x = win.x - Math.floor(r() * 6);
    while (x < win.x + win.w) {
      const tw = row.wmin + Math.floor(r() * (row.wmax - row.wmin));
      const top = win.y + win.h - Math.round((row.lo + r() * (row.hi - row.lo)) * win.h);
      towers.push({ ...row, x, w: tw, top, s: Math.floor(r() * 1e6), sign: r() < 0.35, mast: r() < 0.4, ad: false });
      x += tw + Math.floor(r() * (row.gap + 1)) + (row.layer === 2 ? Math.floor(r() * 10) : 0);
    }
  }
  // one near tower carries the big ad screen
  const near = towers.filter((t) => t.layer === 2 && t.w >= 10);
  if (near.length) near[Math.floor(r() * near.length)].ad = true;
  const cars = [];
  for (let i = 0; i < 7; i++) {
    const depth = r();
    cars.push({
      y: win.y + Math.round(win.h * (0.12 + 0.45 * r())),
      speed: 0.25 + depth * 0.9, dir: r() < 0.5 ? 1 : -1, off: r() * 5000,
      size: depth < 0.4 ? 1 : depth < 0.8 ? 2 : 3, depth,
    });
  }
  return { towers, cars };
}

function drawCity(px, L, city, t) {
  const { win } = L;
  const x1 = win.x + win.w, y1 = win.y + win.h;
  // sky: dark overhead, the city's own glow low down
  for (let y = win.y; y < y1; y++) {
    const f = (y - win.y) / win.h;
    const c = mix(SKY_TOP, SKY_LOW, Math.pow(f, 1.6));
    for (let x = win.x; x < x1; x++) px.set(x, y, c, 1);
  }
  // a far beam sweeping the cloud
  const beam = win.x + win.w * (0.5 + 0.45 * Math.sin(t / 140));
  for (let y = win.y; y < win.y + win.h * 0.5; y++) {
    const bx = beam + (y - win.y) * 0.6;
    for (let k = -1; k <= 1; k++) px.tint(bx + k, y, [120, 110, 170], 0.08 * (1 - Math.abs(k) * 0.5) * (1 - (y - win.y) / (win.h * 0.5)));
  }
  // cars behind the towers go first (far lanes), the rest after the middle row
  const inside = (x) => x >= win.x && x < x1;
  const drawCars = (pred) => {
    const set = (x, y, c) => { if (inside(Math.round(x))) px.set(x, y, c, 1); };
    const tint = (x, y, c, f) => { if (inside(Math.round(x))) px.tint(x, y, c, f); };
    for (const car of city.cars) {
      if (!pred(car)) continue;
      const span = win.w + 40;
      const run = t * car.speed + car.off;
      const lap = Math.floor(run / span);
      if (hash(lap, Math.round(car.off)) > 0.55) continue; // not every lap: now and then
      let x = (run % span) - 20;
      x = car.dir > 0 ? win.x + x : x1 - x;
      const head = car.dir > 0 ? 1 : -1;
      const fade = 0.4 + 0.6 * car.depth;
      for (let k = 1; k <= 8 * car.size; k++) tint(x - head * (k + car.size), car.y, [255, 60, 90], 0.35 * fade * (1 - k / (8 * car.size)));
      for (let k = 0; k < car.size + 1; k++) set(x - head * k, car.y, mix([18, 18, 26], [60, 60, 74], car.depth));
      if (car.size >= 2) for (let k = 0; k < car.size; k++) set(x - head * k, car.y + 1, [14, 14, 20]);
      set(x + head, car.y, mix([140, 130, 110], [255, 244, 210], fade));
      set(x - head * (car.size + 1), car.y, [255, 40, 70]);
    }
  };
  drawCars((c) => c.depth < 0.45);
  for (const tw of city.towers) {
    const base = tw.layer === 0 ? FAR : tw.layer === 1 ? MID : NEAR;
    const r = mulberry32(tw.s);
    for (let y = Math.max(tw.top, win.y); y < y1; y++) {
      for (let x = Math.max(tw.x, win.x); x < Math.min(tw.x + tw.w, x1); x++) {
        // haze: the far row melts into the glow
        const hz = tw.layer === 0 ? 0.25 * ((y - win.y) / win.h) : 0;
        px.set(x, y, mix(base, SKY_LOW, hz), 1);
      }
    }
    if (tw.mast && tw.top - 4 > win.y) {
      const mx = tw.x + Math.floor(tw.w / 2);
      for (let k = 1; k <= 4; k++) px.set(mx, tw.top - k, base, 1);
      if (Math.floor(t / 20 + tw.s) % 3 === 0) px.set(mx, tw.top - 5, [255, 40, 60], 1);
    }
    // windows: a grid on the near row, scattered further back; a few blink on and off
    const step = tw.layer === 2 ? 2 : 2;
    const density = tw.layer === 0 ? 0.18 : tw.layer === 1 ? 0.3 : 0.42;
    for (let y = tw.top + 2; y < y1 - 1; y += step) {
      for (let x = tw.x + 1; x < tw.x + tw.w - 1; x += step) {
        if (x < win.x || x >= x1 || y < win.y) continue;
        const k = hash(x, y, tw.s);
        if (k > density) continue;
        const period = 300 + Math.floor(hash(y, x) * 900);
        if (hash(x, y, Math.floor((t + k * 9000) / period)) < 0.12) continue; // someone's turned it off
        const c = LIGHTS[Math.floor(hash(y, x, tw.s) * LIGHTS.length)];
        px.set(x, y, mul(c, tw.layer === 0 ? 0.45 : tw.layer === 1 ? 0.7 : 0.95), 1);
      }
    }
    // a neon sign up the side, a tube that won't stay lit
    if (tw.sign && tw.layer >= 1) {
      const c = [[255, 0, 200], [0, 229, 255], [255, 120, 40]][tw.s % 3];
      const f = flicker(t, tw.s % 7, tw.s % 4 === 0);
      const sx = tw.x + (tw.s % 2 ? 1 : tw.w - 2);
      const sy = tw.top + 3 + (tw.s % 5);
      for (let k = 0; k < 6 + (tw.s % 5); k++) {
        if (sy + k >= y1) break;
        px.set(sx, sy + k, mul(c, f), 1);
        px.tint(sx - 1, sy + k, c, 0.25 * f);
        px.tint(sx + 1, sy + k, c, 0.25 * f);
      }
    }
    // the ad: a screen of colour washing through its loop
    if (tw.ad) {
      const ax = tw.x + 2, ay = tw.top + 4, aw = tw.w - 4, ah = 6;
      for (let j = 0; j < ah; j++) {
        for (let i = 0; i < aw; i++) {
          const ph = (t / 60 + i / aw * 0.5 + j / ah * 0.2) % 1;
          const c = ph < 0.5 ? mix([255, 0, 200], [0, 229, 255], ph * 2) : mix([0, 229, 255], [255, 0, 200], (ph - 0.5) * 2);
          const bar = Math.floor(t / 4 + i) % aw < 3 && j === 3 ? 1.4 : 1;
          px.set(ax + i, ay + j, mul(c, 0.8 * bar), 1);
        }
      }
      for (let i = -2; i < aw + 2; i++) for (const j of [-1, ah]) px.tint(ax + i, ay + j, [255, 0, 200], 0.2);
    }
  }
  drawCars((c) => c.depth >= 0.45);
  // rain: thin streaks, falling a little sideways
  const n = Math.round(win.w * win.h / 45);
  for (let i = 0; i < n; i++) {
    const speed = 1.2 + hash(i, 1) * 1.4;
    const x0 = win.x + hash(i, 2) * (win.w + 20);
    const y = win.y + ((t * speed + hash(i, 3) * win.h * 3) % (win.h + 6)) - 3;
    for (let k = 0; k < 4; k++) {
      const yy = Math.round(y - k), xx = Math.round(x0 - (y - k) * 0.35);
      if (xx >= win.x && xx < x1 && yy >= win.y && yy < y1) px.tint(xx, yy, [160, 180, 220], 0.34 - k * 0.07);
    }
  }
}

// drops on the glass, sliding down slow; after the frames go on, so only on the glass
function drawDrops(px, L, t) {
  const { win } = L;
  const n = Math.round(win.w * win.h / 160);
  for (let i = 0; i < n; i++) {
    const x = Math.round(win.x + hash(i, 11) * win.w);
    const still = hash(i, 12) < 0.5;
    const speed = still ? 0 : 0.03 + hash(i, 13) * 0.12;
    const y = Math.round(win.y + ((hash(i, 14) * win.h + t * speed) % win.h));
    const i0 = y * px.w + x;
    if (!px.ok(x, y) || !px.glass[i0]) continue;
    px.tint(x, y, [190, 205, 235], 0.32);
    if (!still) for (let k = 1; k < 4; k++) if (px.ok(x, y - k) && px.glass[(y - k) * px.w + x]) px.tint(x, y - k, [170, 185, 220], 0.12 - k * 0.03);
  }
}

// ---------- what's on the screen: the manual's cover, small
const LOGO_W = (s) => [...s].length * 6 - 1;
function drawLogoWord(g, word, x, y, c) {
  [...word].forEach((ch, i) => {
    (GLYPH[ch] || GLYPH[' ']).forEach((row, r) => {
      [...row].forEach((cell, k) => { if (cell !== ' ') g.set(x + i * 6 + k, y + r, cell, c, null, 0); });
    });
  });
}
function drawScreen(g, L, t) {
  const m = L.mon, ix = m.x + 1, iy = m.y + 1, iw = m.w - 2, ih = m.h - 2;
  const scan = Math.floor(t / 3) % (ih + 8);
  for (let j = 0; j < ih; j++) for (let i = 0; i < iw; i++) {
    g.set(ix + i, iy + j, ' ', TEXT, j === scan ? [9, 15, 24] : SCREEN, 0);
  }
  const big = ih >= 18 && iw >= 40;
  const pink = mul(MAGENTA, 0.9 + 0.1 * Math.sin(t / 11));
  let y = iy + Math.max(1, Math.floor((ih - (big ? 16 : 7)) / 2));
  if (big) {
    const lx = ix + Math.floor((iw - LOGO_W('STRATA')) / 2);
    drawLogoWord(g, 'STRATA', lx, y, pink);
    drawLogoWord(g, 'ZERO', lx + LOGO_W('STRATA') - LOGO_W('ZERO'), y + 6, pink);
    y += 12;
  } else {
    const title = 'S T R A T A   Z E R O';
    g.put(ix + Math.floor((iw - title.length) / 2), y, title, pink, null, 1);
    y += 2;
  }
  const line = (s, c, b) => { if (y < iy + ih) g.put(ix + Math.floor((iw - [...s].length) / 2), y, s, c, null, b); y += 1; };
  line("PLAYER'S MANUAL", CYAN, 1);
  line('high tech · low life', DIM, 0);
  y += 1;
  if (Math.floor(t / 18) % 2 === 0) line('▸ press space to read', [120, 230, 200], 0);
}

// ---------- folding pixels into cells
function fold(g, px) {
  for (let y = 0; y < g.h; y++) for (let x = 0; x < g.w; x++) {
    const a = px.get(x, 2 * y), b = px.get(x, 2 * y + 1);
    const i = y * g.w + x;
    if (css(a) === css(b)) { g.ch[i] = ' '; g.bg[i] = a; }
    else { g.ch[i] = '▀'; g.fg[i] = a; g.bg[i] = b; }
    g.bold[i] = 0;
  }
}

// ---------- leaning in: the screen grows to the booklet's place, the room round it
const PAGE_H = 28, SPREAD_W = 94;
function spreadRect(w, h) { return { x: Math.floor((w - SPREAD_W) / 2), y: Math.floor((h - PAGE_H - 1) / 2), w: SPREAD_W, h: PAGE_H }; }
function zoomed(L, px, e) {
  const T = spreadRect(L.w, L.h), S = { x: L.mon.x + 1, y: L.mon.y + 1, w: L.mon.w - 2, h: L.mon.h - 2 };
  const out = new Px(L.w, 2 * L.h);
  for (let Y = 0; Y < out.h; Y++) for (let X = 0; X < out.w; X++) {
    const sx = S.x + (X - T.x) * S.w / T.w, sy = 2 * S.y + (Y - 2 * T.y) * S.h / T.h;
    const inScreen = X >= T.x && X < T.x + T.w && Y >= 2 * T.y && Y < 2 * (T.y + T.h);
    const c = px.get(X + e * (sx - X), Y + e * (sy - Y));
    out.c[Y * out.w + X] = inScreen ? c : mix(c, [0, 0, 0], 0.45 * e);
  }
  return out;
}
function drawBooklet(g, L, t) {
  const b = BOOKLET[`${L.w}x${L.h}`];
  if (!b) return;
  const T = spreadRect(L.w, L.h);
  for (let y = T.y; y < T.y + T.h + 1; y++) for (let x = T.x; x < T.x + T.w; x++) {
    const i = y * g.w + x;
    g.ch[i] = b.ch[i]; g.fg[i] = b.fg[i]; g.bg[i] = b.bg[i]; g.bold[i] = b.bold[i];
  }
}

// ---------- the player
class Scene {
  constructor(el) {
    this.el = el; this.kind = 'A'; this.size = [120, 40]; this.t = 0;
    this.mode = 'scene'; this.p = 0;
    this.canvas = el.querySelector('canvas'); this.ctx = this.canvas.getContext('2d');
    this.resize();
    document.addEventListener('keydown', (e) => this.key(e));
    el.querySelectorAll('[data-kind]').forEach((b) => b.addEventListener('click', () => { this.kind = b.dataset.kind; this.mode = 'scene'; this.p = 0; this.sync(); }));
    el.querySelectorAll('[data-size]').forEach((b) => b.addEventListener('click', () => { this.size = b.dataset.size.split('x').map(Number); this.resize(); this.sync(); }));
    el.querySelector('[data-lean]').addEventListener('click', () => this.lean());
    this.sync();
    setInterval(() => { this.t += 1; this.frame(); }, 1000 / 30);
  }
  sync() {
    this.el.querySelectorAll('[data-kind]').forEach((b) => b.classList.toggle('on', b.dataset.kind === this.kind));
    this.el.querySelectorAll('[data-size]').forEach((b) => b.classList.toggle('on', b.dataset.size === this.size.join('x')));
  }
  resize() {
    const [w, h] = this.size, dpr = window.devicePixelRatio || 1;
    this.canvas.width = w * CW * dpr; this.canvas.height = h * CH * dpr;
    this.canvas.style.aspectRatio = `${w * CW} / ${h * CH}`;
    this.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    this.L = null;
  }
  lean() { if (this.mode === 'scene') this.mode = 'in'; else if (this.mode === 'book') this.mode = 'out'; }
  key(e) {
    if (e.key === ' ') { e.preventDefault(); if (this.mode === 'scene') this.mode = 'in'; }
    if (e.key === 'Escape') { if (this.mode === 'book') this.mode = 'out'; }
    if ('abc'.includes(e.key.toLowerCase()) && e.key.length === 1) { this.kind = e.key.toUpperCase(); this.mode = 'scene'; this.p = 0; this.sync(); }
  }
  frame() {
    const [w, h] = this.size;
    const L = layout(this.kind, w, h);
    const g = new Grid(w, h);
    const px = scenePx(L, this.t);
    if (this.mode === 'in') { this.p = Math.min(1, this.p + 1 / 22); if (this.p >= 1) this.mode = 'book'; }
    if (this.mode === 'out') { this.p = Math.max(0, this.p - 1 / 22); if (this.p <= 0) this.mode = 'scene'; }
    const e = ease(this.p);
    if (this.p > 0) {
      fold(g, zoomed(L, px, e));
      if (this.mode === 'book') drawBooklet(g, L, this.t);
    } else {
      fold(g, px);
      drawScreen(g, L, this.t);
      drawRoomText(g, L, this.t);
    }
    const hint = this.mode === 'book' ? '← → turn the page · c contents · esc lean back' : this.p > 0 ? '' : 'space read the manual · esc close';
    if (hint) {
      const x = Math.floor((w - [...hint].length) / 2);
      for (let i = x - 1; i < x + [...hint].length + 1; i++) g.set(i, h - 1, ' ', TEXT, [4, 6, 10], 0);
      g.put(x, h - 1, hint, DIM, [4, 6, 10], 0);
    }
    paint(this.ctx, g);
  }
}
