'use strict';
// The room round the screen: a runner's flat, not a study. Concrete and gunmetal, a pipe
// overhead with a tube of cyan under it and cables hanging off it, one bare bulb for
// warmth. On the desk: your deck cabled off to your jack, a little terminal scrolling,
// a holo-projector turning a wireframe, a can, the brew. B's wall carries a rack.

const CONCRETE = [20, 20, 26], SEAM = [12, 12, 16], CEIL = [10, 10, 14];
const METAL = [30, 33, 42], METAL_LIT = [64, 70, 86], METAL_DARK = [16, 17, 22];
const PIPE = [36, 38, 46], PIPE_LIT = [74, 78, 92], CABLE = [8, 8, 11];
const FRAME = [18, 19, 24], FRAME_LIT = [46, 50, 62];
const BEZEL = [22, 24, 30], BEZEL_LIT = [44, 48, 60];
const NEON_M = [255, 0, 200], NEON_C = [0, 229, 255], UV = [150, 60, 255];
const LED = [[0, 255, 140], [255, 180, 40], [0, 229, 255], [255, 40, 60]];
const BULB = [255, 190, 120];
const MUG = [34, 34, 44], BREW = [48, 26, 16], STEAM = [214, 218, 228];
const KEYS = [20, 22, 28], KEY_TOP = [40, 44, 56];

// ---------- three ways to frame the room, for a grid `w` cells by `h` rows (pixels are
// cells across and two to a cell down)
function layout(kind, w, h) {
  const H = 2 * h;
  const small = w < 110;
  if (kind === 'A') {
    // the wide window: the screen in the middle of a window the width of the wall
    const deskRow = h - Math.max(6, Math.round(h * 0.2));
    const deskY = 2 * deskRow;
    const sw = Math.min(46, Math.round(w * 0.42)), sh = Math.min(18, deskRow - 9);
    const mon = { x: Math.round((w - sw - 2) / 2), y: deskRow - 3 - (sh + 2) + 1, w: sw + 2, h: sh + 2 };
    const right = mon.x + mon.w;
    return {
      kind, w, h, deskRow, deskY,
      win: { x: 3, y: 4, w: w - 6, h: deskY - 14 },
      bars: { v: [1 / 3, 2 / 3], t: 0.2 },
      mon,
      kb: { x: Math.round(w / 2 - 17), y: deskY + 4, w: 34 },
      deck: { x: 3, y: deskY + 6, w: small ? 13 : 17 },
      mug: { x: mon.x - (small ? 9 : 11), y: deskY + 8, r: 3 },
      can: { x: mon.x - 3, y: deskY + 7 },
      crt: { x: right + 3, y: deskY - 12, w: small ? 13 : 16, h: 14 },
      holo: { x: w - (small ? 6 : 8), y: deskY + 5 },
      pipe: 1, bulb: { x: Math.round(w * 0.16), drop: 9 },
      plant: { x: 7 },
      cables: [[0.22, 0.42, 7], [0.55, 0.8, 5], [0.8, 0.98, 8]],
      seed: 7,
    };
  }
  if (kind === 'B') {
    // the corner: a tall window on the left, your wall on the right with its rack, the
    // screen off to the right under it
    const deskRow = h - Math.max(6, Math.round(h * 0.2));
    const deskY = 2 * deskRow;
    const sw = Math.min(40, Math.round(w * 0.38)), sh = Math.min(16, deskRow - 9);
    const mon = { x: Math.round(w * 0.5), y: deskRow - 3 - (sh + 2) + 1, w: sw + 2, h: sh + 2 };
    const winW = Math.round(w * 0.56);
    return {
      kind, w, h, deskRow, deskY,
      win: { x: 2, y: 4, w: winW, h: deskY - 14 },
      bars: { v: [1 / 2], t: 0.18 },
      mon,
      kb: { x: mon.x + 2, y: deskY + 4, w: sw - 2 },
      deck: { x: 4, y: deskY + 6, w: small ? 13 : 17 },
      mug: { x: mon.x - 12, y: deskY + 8, r: 3 },
      can: { x: mon.x - 5, y: deskY + 7 },
      holo: { x: Math.round(w * 0.36), y: deskY + 5 },
      rack: { x: winW + 6, y: 6, w: w - winW - 9, h: 2 * mon.y - 10 },
      vent: { x: mon.x + mon.w + 8, y: 2 * mon.y + 8, r: 5 },
      sign: { x: mon.x + mon.w + 1, y: mon.y + 9 },
      pipe: 1, bulb: null,
      cables: [[0.1, 0.3, 6], [0.35, 0.55, 9]],
      seed: 11,
    };
  }
  // C, close: the screen big in front of you, the city round it, the brew by your hand
  const deskRow = h - 5;
  const deskY = 2 * deskRow;
  const sw = Math.round(w * 0.56), sh = deskRow - 8;
  const mon = { x: Math.round((w - sw - 2) / 2) + 4, y: 4, w: sw + 2, h: sh + 2 };
  return {
    kind, w, h, deskRow, deskY,
    win: { x: 0, y: 4, w: w, h: deskY - 6 },
    bars: { v: [0.5], t: -1 },
    mon,
    kb: { x: Math.round(w / 2 - 22), y: deskY + 3, w: 44 },
    deck: { x: w - (small ? 16 : 20), y: H - 3, w: small ? 13 : 17 },
    mug: { x: 12, y: H - 2, r: 5 },
    can: { x: 22, y: H - 3 },
    holo: { x: mon.x + mon.w + 6, y: deskY + 4 },
    pipe: 1, bulb: { x: Math.round(w * 0.1), drop: 11 },
    cables: [[0.3, 0.5, 6], [0.62, 0.9, 9]],
    seed: 5,
  };
}

// ---------- light: the screen cool, the neon in the room, the bulb warm
function lightsOf(L, t) {
  const out = [];
  const m = L.mon;
  out.push({ x: m.x + m.w / 2, y: 2 * (m.y + m.h / 2), rx: m.w / 2, ry: m.h, c: [120, 200, 255], s: 0.42, r: 22 });
  const pulse = 0.85 + 0.15 * Math.sin(t / 40);
  out.push({ seg: true, x: 0, x2: L.w, y: L.deskY + 1, c: NEON_M, s: 0.55 * pulse, r: 5 });
  if (L.pipe) out.push({ seg: true, x: 0, x2: L.w, y: 3, c: NEON_C, s: 0.35, r: 9 });
  if (L.bulb) out.push({ x: L.bulb.x, y: L.bulb.drop + 2, c: BULB, s: 0.75 * (0.96 + 0.04 * Math.sin(t / 3)), r: 11 });
  if (L.crt) out.push({ x: L.crt.x + L.crt.w / 2, y: L.crt.y + L.crt.h / 2, c: [80, 255, 140], s: 0.28, r: 9 });
  if (L.holo) out.push({ x: L.holo.x, y: L.holo.y - 10, c: NEON_C, s: 0.3 * (0.8 + 0.2 * Math.sin(t / 9)), r: 10 });
  if (L.sign) out.push({ x: 2 * L.sign.x / 2 + 3, y: 2 * L.sign.y + 2, c: [255, 60, 190], s: 0.6 * flicker(t, 3, true), r: 12 });
  if (L.plant) out.push({ x: L.plant.x + 2, y: L.win.y + L.win.h - 8, c: UV, s: 0.4, r: 6 });
  return out;
}
function lit(c, L, x, y, amb = 0.62) {
  let k = amb, col = [0, 0, 0];
  for (const l of L.lights) {
    let dx, dy;
    if (l.seg) { dx = x < l.x ? l.x - x : x > l.x2 ? x - l.x2 : 0; dy = y - l.y; }
    else { dx = Math.max(0, Math.abs(x - l.x) - (l.rx || 0)); dy = Math.max(0, Math.abs(y - l.y) - (l.ry || 0)); }
    const f = l.s / (1 + (dx * dx + dy * dy) / (l.r * l.r));
    k += 0.9 * f;
    col = add(col, l.c, 0.16 * f);
  }
  return add(mul(c, k), col);
}

// ---------- a line of pixels
function line(px, x0, y0, x1, y1, c, f = 1) {
  const n = Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0), 1);
  for (let i = 0; i <= n; i++) {
    const x = x0 + (x1 - x0) * i / n, y = y0 + (y1 - y0) * i / n;
    if (f >= 1) px.set(x, y, c); else px.tint(x, y, c, f);
  }
}

// ---------- the room: concrete, the window's frame, the pipe and its cables, the desk
function drawRoom(px, L, t) {
  const W = px.w, H = px.h, { win } = L, deskY = L.deskY;
  for (let y = 0; y < deskY; y++) for (let x = 0; x < W; x++) {
    if (px.glass[y * W + x]) continue;
    const seam = x % 24 === 0 || y % 18 === 0;
    const grit = hash(x, y, 3) < 0.08 ? 0.85 : 1;
    px.set(x, y, lit(mul(y < win.y - 1 ? CEIL : seam ? SEAM : CONCRETE, grit), L, x, y));
  }
  // the frame: heavy, riveted
  const fr = (x, y, w, h) => {
    for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) {
      const rivet = (i + j) % 7 === 3 && (w <= 2 || h <= 2);
      px.set(x + i, y + j, lit(rivet ? FRAME_LIT : j === 0 || i === 0 ? mix(FRAME, FRAME_LIT, 0.5) : FRAME, L, x + i, y + j));
    }
  };
  fr(win.x - 2, win.y - 2, win.w + 4, 2);
  fr(win.x - 2, win.y, 2, win.h);
  fr(win.x + win.w, win.y, 2, win.h);
  for (const v of L.bars.v) fr(win.x + Math.round(win.w * v) - 1, win.y, 2, win.h);
  if (L.bars.t > 0) fr(win.x, win.y + Math.round(win.h * L.bars.t), win.w, 1);
  fr(win.x - 3, win.y + win.h, win.w + 6, 3);
  if (L.plant) drawPlant(px, L, t);
  if (L.rack) drawRack(px, L, t);
  if (L.vent) drawVent(px, L, t);
  drawDesk(px, L, t);
}

// the pipe along the ceiling, the cyan tube under it, the cables sagging off it, a bulb
function drawCeiling(px, L, t) {
  const W = px.w;
  for (let x = 0; x < W; x++) {
    px.set(x, 0, lit(PIPE_LIT, L, x, 0));
    px.set(x, 1, lit(PIPE, L, x, 1));
    if (x % 22 === 5) { px.set(x, 2, lit(PIPE_LIT, L, x, 2)); px.set(x + 1, 2, lit(PIPE, L, x, 2)); }
  }
  const tube = 0.9 + 0.1 * Math.sin(t / 13);
  for (let x = 2; x < W - 2; x++) {
    if (Math.floor(W * 0.62) < x && x < Math.floor(W * 0.66)) continue; // a dead stretch
    px.set(x, 3, mul(NEON_C, tube * (Math.floor(W * 0.58) < x && x < Math.floor(W * 0.62) ? flicker(t, 5, true) : 1)));
  }
  for (const [a, b, sag] of L.cables || []) {
    const x0 = Math.round(W * a), x1 = Math.round(W * b);
    const sway = Math.sin(t / 70 + a * 9) * 0.8;
    for (let x = x0; x <= x1; x++) {
      const u = (x - x0) / (x1 - x0);
      const y = 4 + sag * 4 * u * (1 - u) + sway * 4 * u * (1 - u);
      px.set(x, y, CABLE);
    }
  }
  if (L.bulb) {
    const { x, drop } = L.bulb;
    const sway = Math.round(Math.sin(t / 55) * 0.6);
    for (let y = 4; y < drop; y++) px.set(x + (y > drop / 2 ? sway : 0), y, CABLE);
    px.set(x + sway, drop, [60, 54, 50]);
    px.set(x + sway, drop + 1, BULB);
    px.set(x + sway, drop + 2, mul(BULB, 0.8));
    for (const [dx, dy, f] of [[-1, 1, 0.35], [1, 1, 0.35], [-1, 2, 0.25], [1, 2, 0.25], [0, 3, 0.25]]) px.tint(x + sway + dx, drop + dy, BULB, f);
  }
}

// a plastic plant on the sill under a little UV grow light
function drawPlant(px, L, t) {
  const { win } = L, x = L.plant.x, sy = win.y + win.h;
  px.rect(x, sy - 3, 5, 3, lit([40, 40, 50], L, x, sy - 2));
  for (let k = 0; k < 16; k++) {
    const lx = x + 2 + Math.sin(k * 2.3) * (1 + k * 0.25);
    px.set(lx, sy - 4 - (k % 7), lit([30, 120, 70], L, lx, sy - 4));
  }
  px.rect(x - 1, sy - 13, 7, 1, [44, 40, 56]);
  for (let i = 0; i < 5; i++) px.set(x + i, sy - 12, mul(UV, 0.9));
}

// B's rack: gunmetal uprights, three shelves, servers blinking, an arm on hooks, an eye
// in a jar
function drawRack(px, L, t) {
  const r = L.rack;
  const shelves = [r.y + Math.round(r.h * 0.33), r.y + Math.round(r.h * 0.66), r.y + r.h];
  for (const x of [r.x, r.x + r.w - 1]) for (let y = r.y; y <= r.y + r.h; y++) px.set(x, y, lit(METAL_LIT, L, x, y));
  for (const y of shelves) for (let x = r.x; x < r.x + r.w; x++) {
    px.set(x, y, lit(METAL_LIT, L, x, y));
    if (x % 3 === 0) px.set(x, y + 1, lit(METAL_DARK, L, x, y + 1));
  }
  // shelf 1: two server units, their lights going
  const s1 = shelves[0];
  for (let u = 0; u < 2; u++) {
    const ux = r.x + 2 + u * Math.round(r.w * 0.38), uw = Math.round(r.w * 0.34);
    px.rect(ux, s1 - 4, uw, 4, lit(METAL, L, ux, s1 - 3));
    for (let i = 0; i < uw - 2; i += 2) {
      const on = hash(ux + i, u, Math.floor(t / (4 + ((i * 7) % 11)))) < 0.55;
      if (on) px.set(ux + 1 + i, s1 - 3, LED[(i + u) % 2]);
      if (hash(i, u, Math.floor(t / 23)) < 0.3) px.set(ux + 1 + i, s1 - 2, LED[2]);
    }
  }
  // shelf 2: the eye in its jar, a coil of cable, a box of shards
  const s2 = shelves[1];
  const jx = r.x + 3;
  px.rect(jx, s2 - 7, 5, 7, lit([30, 70, 60], L, jx, s2 - 4));
  px.rect(jx, s2 - 8, 5, 1, lit(METAL_LIT, L, jx, s2 - 8));
  const look = [[0, 0], [1, 0], [-1, 0], [0, -1]][Math.floor(hash(Math.floor(t / 70), 9) * 4)];
  px.set(jx + 2, s2 - 4, [230, 236, 230]);
  px.set(jx + 2 + look[0], s2 - 4 + look[1], NEON_C);
  for (let k = 0; k < 3; k++) px.tint(jx + 1 + k, s2 - 6 + ((t / 30 + k) | 0) % 5, [120, 255, 200], 0.25);
  for (let k = 0; k < 6; k++) px.set(jx + 9 + Math.round(Math.cos(k) * 2), s2 - 2 - Math.round(Math.sin(k) * 1.2), CABLE);
  px.rect(jx + 14, s2 - 3, 6, 3, lit([60, 30, 40], L, jx + 14, s2 - 2));
  for (let i = 0; i < 3; i++) px.set(jx + 15 + i * 2, s2 - 3, LED[(i + 1) % 4]);
  // shelf 3: a chrome arm hung on two hooks, its joint lit
  const s3 = shelves[2];
  const ax = r.x + Math.round(r.w * 0.55);
  for (const hx of [ax, ax + 9]) px.set(hx, s3 - 9, lit(METAL_LIT, L, hx, s3 - 9));
  line(px, ax, s3 - 8, ax + 8, s3 - 8, lit([150, 156, 170], L, ax, s3 - 8));
  line(px, ax, s3 - 7, ax + 8, s3 - 7, lit([90, 96, 110], L, ax, s3 - 7));
  line(px, ax + 8, s3 - 7, ax + 11, s3 - 2, lit([130, 136, 150], L, ax + 9, s3 - 4));
  px.rect(ax + 10, s3 - 2, 3, 2, lit([110, 116, 130], L, ax + 10, s3 - 1));
  px.set(ax + 8, s3 - 8, mul(NEON_C, 0.7 + 0.3 * Math.sin(t / 8)));
}

// a vent fan in the wall, turning
function drawVent(px, L, t) {
  const v = L.vent;
  for (let j = -v.r; j <= v.r; j++) for (let i = -v.r; i <= v.r; i++) {
    const d = Math.hypot(i, j);
    if (d <= v.r) px.set(v.x + i, v.y + j, lit(d > v.r - 1 ? METAL_LIT : METAL_DARK, L, v.x + i, v.y + j));
  }
  for (let b = 0; b < 3; b++) {
    const a = t * 0.18 + b * (2 * Math.PI / 3);
    line(px, v.x, v.y, v.x + Math.cos(a) * (v.r - 1), v.y + Math.sin(a) * (v.r - 1), lit(METAL, L, v.x, v.y));
  }
  px.set(v.x, v.y, lit(METAL_LIT, L, v.x, v.y));
}

// the desk: a slab of gunmetal, worn, a neon strip along its back edge
function drawDesk(px, L, t) {
  const W = px.w, H = px.h, deskY = L.deskY;
  for (let y = deskY; y < H; y++) {
    const f = (y - deskY) / (H - deskY);
    for (let x = 0; x < W; x++) {
      const panel = x % 40 === 0 ? 0.8 : 1;
      const wear = hash(x, y, 5) < 0.04 ? 1.25 : 1;
      px.set(x, y, lit(mul(mix(METAL_DARK, METAL, 0.4 + f * 0.6), panel * wear), L, x, y, 0.5));
    }
  }
  const pulse = 0.85 + 0.15 * Math.sin(t / 40);
  for (let x = 0; x < W; x++) {
    px.set(x, deskY, mul(NEON_M, pulse));
    px.tint(x, deskY + 1, NEON_M, 0.35 * pulse);
    px.tint(x, deskY - 1, NEON_M, 0.25 * pulse);
  }
}

// the screen on its arm, stickers on the bezel
function drawMonitor(px, L, t) {
  const m = L.mon, x = m.x, y = 2 * m.y, w = m.w, h = 2 * m.h;
  for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) {
    const edge = j < 2 || j >= h - 2 || i === 0 || i === w - 1;
    px.set(x + i, y + j, edge ? (j === 0 ? BEZEL_LIT : BEZEL) : SCREEN);
  }
  px.rect(x + 2, y + h - 2, 2, 2, [230, 196, 70]);
  px.rect(x + 5, y + h - 2, 3, 1, NEON_M);
  px.rect(x + w - 6, y, 3, 2, NEON_C);
  // a status light on the bezel
  px.set(x + w - 3, y + h - 1, Math.floor(t / 45) % 2 ? [0, 255, 140] : [0, 120, 70]);
  const nx = x + Math.floor(w / 2) - 1;
  for (let j = y + h; j < L.deskY + 3; j++) px.rect(nx, j, 3, 1, lit(METAL, L, nx, j));
  px.rect(nx - 6, L.deskY + 3, 15, 1, lit(METAL_LIT, L, nx, L.deskY + 3));
}

function drawKeyboard(px, L, t) {
  const k = L.kb;
  for (let j = 0; j < 4; j++) for (let i = 0; i < k.w; i++) {
    const key = j < 3 && i % 2 === 0;
    px.set(k.x + i, k.y + j, lit(key ? KEY_TOP : KEYS, L, k.x + i, k.y + j));
  }
  for (let i = -1; i <= k.w; i++) {
    const ph = (i / k.w + t / 300) % 1;
    px.tint(k.x + i, k.y + 4, mix(NEON_M, NEON_C, Math.abs(ph * 2 - 1)), 0.5);
  }
}

// your deck: a slab with its keys, a little screen tracing a signal, its lights, and the
// cable from it running off the bottom of the picture, to the jack behind your ear
function drawDeck(px, L, t) {
  const d = L.deck;
  if (!d) return;
  px.rect(d.x, d.y, d.w, 4, lit(METAL, L, d.x, d.y + 2));
  for (let i = 0; i < d.w; i++) px.set(d.x + i, d.y, lit(METAL_LIT, L, d.x + i, d.y));
  for (let i = 1; i < d.w - 7; i += 2) px.set(d.x + i, d.y + 2, lit(KEY_TOP, L, d.x + i, d.y + 2));
  const sx = d.x + d.w - 6;
  px.rect(sx, d.y + 1, 5, 2, [4, 16, 10]);
  for (let i = 0; i < 5; i++) {
    const v = Math.sin((i + t * 0.3) * 1.3) > 0.2 ? 0 : 1;
    px.set(sx + i, d.y + 1 + v, [80, 255, 140]);
  }
  px.set(d.x + 1, d.y + 3, Math.floor(t / 12) % 3 ? NEON_M : [80, 0, 60]);
  // the jack cable, off the bottom edge
  const H = px.h;
  for (let k = 0; k < 40; k++) {
    const u = k / 39;
    const x = d.x + 3 + Math.sin(u * 3) * 4 - u * 2;
    const y = d.y + 4 + u * (H - d.y - 3);
    px.set(x, y, CABLE);
    if (k % 6 === 0) px.set(x + 1, y, [30, 30, 40]);
  }
}

// the little terminal beside the screen: green text crawling up it (drawn as text)
function drawCrt(px, L, t) {
  const c = L.crt;
  if (!c) return;
  px.rect(c.x, c.y, c.w, c.h, lit([38, 36, 42], L, c.x, c.y));
  for (let i = 0; i < c.w; i++) px.set(c.x + i, c.y, lit([60, 58, 66], L, c.x + i, c.y));
  px.rect(c.x + 1, c.y + 2, c.w - 2, c.h - 4, [4, 14, 8]);
  px.rect(c.x + 3, c.y + c.h, c.w - 6, 1, lit(METAL_DARK, L, c.x, c.y + c.h));
}

// the holo-projector: a puck on the desk throwing a wireframe that turns
function drawHolo(px, L, t) {
  const h = L.holo;
  if (!h) return;
  px.rect(h.x - 2, h.y, 5, 2, lit([28, 28, 36], L, h.x, h.y));
  px.set(h.x, h.y, NEON_C);
  const cy = h.y - 9, size = 3.6;
  for (let j = 1; j < 9; j++) for (let i = -Math.floor(j / 3); i <= Math.floor(j / 3); i++) px.tint(h.x + i, h.y - j, NEON_C, 0.07);
  const a = t * 0.045, b = 0.55;
  const V = [];
  for (const X of [-1, 1]) for (const Y of [-1, 1]) for (const Z of [-1, 1]) {
    let x = X * Math.cos(a) - Z * Math.sin(a), z = X * Math.sin(a) + Z * Math.cos(a);
    let y = Y * Math.cos(b) - z * Math.sin(b);
    V.push([h.x + x * size, cy + y * size]);
  }
  const E = [[0, 1], [2, 3], [4, 5], [6, 7], [0, 2], [1, 3], [4, 6], [5, 7], [0, 4], [1, 5], [2, 6], [3, 7]];
  const glitch = Math.floor(t / 4) % 37 === 0;
  for (const [p, q] of E) line(px, V[p][0] + (glitch ? 1 : 0), V[p][1], V[q][0], V[q][1], NEON_C, 0.7);
}

// a can of something loud
function drawCan(px, L, t) {
  const c = L.can;
  if (!c) return;
  for (let j = 0; j < 6; j++) for (let i = 0; i < 3; i++) {
    const col = j === 0 || j === 5 ? [150, 156, 170] : j === 2 ? [240, 240, 250] : mix(NEON_M, [120, 0, 90], i / 2);
    px.set(c.x + i, c.y - 6 + j, lit(col, L, c.x + i, c.y - 3, 0.75));
  }
}

// the brew: a black mug with a band of light round it, its steam curling up and thinning
function drawMug(px, L, t) {
  const { x, y, r } = L.mug;
  const w = 2 * r + 1, h = Math.round(r * 1.9) + 1;
  const top = y - h;
  for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) {
    const side = Math.abs(i - r) / r;
    const band = j === Math.round(h * 0.55);
    const c = band ? mul(NEON_C, 0.85 - side * 0.4) : mix(mix(MUG, [60, 62, 78], 0.3), [10, 10, 14], Math.pow(side, 2) * 0.8);
    px.set(x - r + i, top + j, band ? c : lit(c, L, x - r + i, top + j, 0.75));
  }
  for (let i = 1; i < w - 1; i++) px.set(x - r + i, top, mix(BREW, [90, 54, 34], i === 1 ? 0.5 : 0));
  const hx = x + r + 1;
  for (let j = 2; j < h - 2; j++) px.set(hx + (j === 2 || j === h - 3 ? 0 : 1), top + j, lit(MUG, L, hx, top + j));
  px.set(hx, top + 2, lit(MUG, L, hx, top + 2));
  const tall = Math.round(r * 4.5) + 6;
  for (let k = 0; k < 3; k++) {
    for (let s = 1; s < tall; s++) {
      const life = s / tall;
      const sway = Math.sin(s * 0.38 - t * 0.07 + k * 2.1) * (0.6 + life * r * 0.5) + (k - 1) * r * 0.45;
      const a = (1 - life) * 0.38 * (0.55 + 0.45 * Math.sin(t * 0.045 + k * 1.9 + s * 0.15));
      if (a > 0.02) px.tint(x + sway, top - s, STEAM, a);
    }
  }
}

// ---------- what's written in the room: the terminal's crawl, B's sign
const HEX = '0123456789abcdef';
function drawRoomText(g, L, t) {
  const c = L.crt;
  if (c) {
    const x0 = c.x + 2, y0 = Math.ceil((c.y + 2) / 2), rows = Math.floor((c.y + c.h - 2) / 2) - y0;
    const roll = Math.floor(t / 10);
    for (let r = 0; r < rows; r++) {
      const n = roll + r;
      let s = '';
      for (let k = 0; k < c.w - 4; k++) s += k % 3 === 2 ? ' ' : HEX[Math.floor(hash(n, k) * 16)];
      if (hash(n, 99) < 0.2) s = ('ICE ' + ['OK', 'WARM', '??'][n % 3]).padEnd(c.w - 4).slice(0, c.w - 4);
      g.put(x0, y0 + r, s, mul([80, 255, 140], r === rows - 1 ? 1 : 0.65), null, 0);
    }
  }
  if (L.sign) {
    const f = flicker(t, 3, true);
    const pink = mul([255, 70, 190], f);
    g.put(L.sign.x + 1, L.sign.y, '◆ STAY', pink, null, 1);
    g.put(L.sign.x + 1, L.sign.y + 1, ' WIRED', pink, null, 1);
  }
}

// ---------- the whole room at tick t, as pixels
const cities = new Map();
function scenePx(L, t) {
  const key = `${L.kind}${L.w}x${L.h}`;
  if (!cities.has(key)) cities.set(key, makeCity(L));
  L.lights = lightsOf(L, t);
  const px = new Px(L.w, 2 * L.h);
  drawCity(px, L, cities.get(key), t);
  drawRoom(px, L, t);
  drawDrops(px, L, t);
  drawCeiling(px, L, t);
  drawMonitor(px, L, t);
  drawCrt(px, L, t);
  drawKeyboard(px, L, t);
  drawDeck(px, L, t);
  drawCan(px, L, t);
  drawMug(px, L, t);
  drawHolo(px, L, t);
  return px;
}
