//! The desk the MANUAL opens at (spec 2026-11-06-players-manual-design §3a): you, first
//! person, a high floor at night, the manual's cover on the screen in front of a tall
//! window over the city. Rain, neon, the odd car in the air, a brew steaming. Your wall
//! on the right carries a rack, a vent fan and a sign.
//!
//! Everything is half-block pixels, two to a cell, worked out from the tick: the same
//! tick draws the same picture. Leaning in grows the screen until it is the booklet's
//! spread, the room dimmed round it.

use crate::anim::effects::flicker;
use crate::ui::logo;
use citygen::rng::hash64;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// A colour while it's being worked on.
type C = [f32; 3];

const SKY_TOP: C = [6.0, 5.0, 18.0];
const SKY_LOW: C = [64.0, 20.0, 66.0];
const FAR: C = [46.0, 30.0, 74.0];
const MID: C = [24.0, 18.0, 44.0];
const NEAR: C = [11.0, 9.0, 20.0];
const WINDOWS: [C; 5] = [
    [255.0, 196.0, 110.0],
    [255.0, 214.0, 150.0],
    [120.0, 220.0, 255.0],
    [255.0, 90.0, 200.0],
    [230.0, 230.0, 240.0],
];
const WALL: C = [17.0, 17.0, 22.0];
const SCREEN: C = [5.0, 9.0, 15.0];
const CONCRETE: C = [20.0, 20.0, 26.0];
const SEAM: C = [12.0, 12.0, 16.0];
const CEIL: C = [10.0, 10.0, 14.0];
const METAL: C = [30.0, 33.0, 42.0];
const METAL_LIT: C = [64.0, 70.0, 86.0];
const METAL_DARK: C = [16.0, 17.0, 22.0];
const PIPE: C = [36.0, 38.0, 46.0];
const PIPE_LIT: C = [74.0, 78.0, 92.0];
const CABLE: C = [8.0, 8.0, 11.0];
const FRAME: C = [18.0, 19.0, 24.0];
const FRAME_LIT: C = [46.0, 50.0, 62.0];
const BEZEL: C = [22.0, 24.0, 30.0];
const BEZEL_LIT: C = [44.0, 48.0, 60.0];
const NEON_M: C = [255.0, 0.0, 200.0];
const NEON_C: C = [0.0, 229.0, 255.0];
const LEDS: [C; 4] = [
    [0.0, 255.0, 140.0],
    [255.0, 180.0, 40.0],
    [0.0, 229.0, 255.0],
    [255.0, 40.0, 60.0],
];
const MUG: C = [34.0, 34.0, 44.0];
const BREW: C = [48.0, 26.0, 16.0];
const STEAM: C = [214.0, 218.0, 228.0];
const KEYS: C = [20.0, 22.0, 28.0];
const KEY_TOP: C = [40.0, 44.0, 56.0];
const BLACK: C = [0.0, 0.0, 0.0];

fn mix(a: C, b: C, f: f32) -> C {
    let f = f.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * f,
        a[1] + (b[1] - a[1]) * f,
        a[2] + (b[2] - a[2]) * f,
    ]
}

fn mul(a: C, f: f32) -> C {
    [a[0] * f, a[1] * f, a[2] * f]
}

fn add(a: C, b: C, f: f32) -> C {
    [a[0] + b[0] * f, a[1] + b[1] * f, a[2] + b[2] * f]
}

fn rgb(c: C) -> (u8, u8, u8) {
    let q = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    (q(c[0]), q(c[1]), q(c[2]))
}

fn color(c: C) -> Color {
    let (r, g, b) = rgb(c);
    Color::Rgb(r, g, b)
}

/// A number in 0..1 for three integers, the same every time.
fn hash(a: i64, b: i64, c: i64) -> f32 {
    let h = hash64(hash64(a as u64, (b as u64) ^ 0x51ed), (c as u64) ^ 0xa7c3);
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// A seeded run of numbers in 0..1.
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> f32 {
        self.0 = hash64(self.0, 0x2545_f491_4f6c_dd1d);
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// The picture: pixels `w` wide and twice the rows high, each knowing whether it's glass.
struct Px {
    w: i32,
    h: i32,
    c: Vec<C>,
    glass: Vec<bool>,
}

impl Px {
    fn new(w: i32, h: i32) -> Self {
        Px {
            w,
            h,
            c: vec![WALL; (w * h).max(0) as usize],
            glass: vec![false; (w * h).max(0) as usize],
        }
    }

    fn at(&self, x: f32, y: f32) -> Option<usize> {
        let (x, y) = (x.round() as i32, y.round() as i32);
        (x >= 0 && y >= 0 && x < self.w && y < self.h).then(|| (y * self.w + x) as usize)
    }

    /// The pixel nearest (x, y), the edge's for one off the picture.
    fn get(&self, x: f32, y: f32) -> C {
        let x = (x.round() as i32).clamp(0, self.w - 1);
        let y = (y.round() as i32).clamp(0, self.h - 1);
        self.c[(y * self.w + x) as usize]
    }

    fn set(&mut self, x: f32, y: f32, c: C) {
        if let Some(i) = self.at(x, y) {
            self.c[i] = c;
            self.glass[i] = false;
        }
    }

    fn set_glass(&mut self, x: f32, y: f32, c: C) {
        if let Some(i) = self.at(x, y) {
            self.c[i] = c;
            self.glass[i] = true;
        }
    }

    fn tint(&mut self, x: f32, y: f32, c: C, f: f32) {
        if let Some(i) = self.at(x, y) {
            self.c[i] = mix(self.c[i], c, f);
        }
    }

    fn is_glass(&self, x: f32, y: f32) -> bool {
        self.at(x, y).is_some_and(|i| self.glass[i])
    }

    fn rect(&mut self, x: f32, y: f32, w: i32, h: i32, c: C) {
        for j in 0..h {
            for i in 0..w {
                self.set(x + i as f32, y + j as f32, c);
            }
        }
    }

    fn line(&mut self, (x0, y0): (f32, f32), (x1, y1): (f32, f32), c: C, f: f32) {
        let n = (x1 - x0).abs().max((y1 - y0).abs()).max(1.0).round() as i32;
        for i in 0..=n {
            let u = i as f32 / n as f32;
            let (x, y) = (x0 + (x1 - x0) * u, y0 + (y1 - y0) * u);
            if f >= 1.0 {
                self.set(x, y, c);
            } else {
                self.tint(x, y, c, f);
            }
        }
    }
}

/// A rectangle in pixels (or, for the screen and the words, in cells).
#[derive(Clone, Copy, Debug)]
struct Area {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

struct Tower {
    layer: usize,
    x: i32,
    w: i32,
    top: i32,
    s: u64,
    sign: bool,
    mast: bool,
    ad: bool,
}

struct Car {
    y: i32,
    speed: f32,
    dir: f32,
    off: f32,
    size: i32,
    depth: f32,
}

/// A light in the room: a point (with a size), or a line across at a height.
struct Light {
    line: bool,
    x: f32,
    x2: f32,
    y: f32,
    rx: f32,
    ry: f32,
    c: C,
    s: f32,
    r: f32,
}

/// The desk laid out for a terminal `w` cells by `h` rows.
pub struct Desk {
    w: i32,
    h: i32,
    desk_y: i32,
    /// The window, in pixels.
    win: Area,
    /// The screen with its bezel, in cells.
    mon: Area,
    kb: Area,
    deck: Area,
    mug: (f32, f32, i32),
    can: (f32, f32),
    holo: (f32, f32),
    rack: Area,
    vent: (f32, f32, i32),
    /// The sign's first cell.
    sign: (i32, i32),
    towers: Vec<Tower>,
    cars: Vec<Car>,
}

impl Desk {
    pub fn new(w: u16, h: u16) -> Desk {
        let (w, h) = (i32::from(w), i32::from(h));
        let desk_row = h - 6.max((h as f32 * 0.2).round() as i32);
        let desk_y = 2 * desk_row;
        let sw = ((w as f32 * 0.38).round() as i32).min(40 + (w - 120).max(0) / 3);
        let sh = (16 + (h - 40).max(0) / 2).min(desk_row - 9).max(6);
        let mon = Area {
            x: (w as f32 * 0.5).round() as i32,
            y: desk_row - 3 - (sh + 2) + 1,
            w: sw + 2,
            h: sh + 2,
        };
        let win_w = (w as f32 * 0.56).round() as i32;
        let win = Area {
            x: 2,
            y: 4,
            w: win_w,
            h: desk_y - 14,
        };
        let small = w < 110;
        let mut desk = Desk {
            w,
            h,
            desk_y,
            win,
            mon,
            kb: Area {
                x: mon.x + 2,
                y: desk_y + 4,
                w: sw - 2,
                h: 4,
            },
            deck: Area {
                x: 4,
                y: desk_y + 6,
                w: if small { 13 } else { 17 },
                h: 4,
            },
            mug: ((mon.x - 12) as f32, (desk_y + 8) as f32, 3),
            can: ((mon.x - 5) as f32, (desk_y + 7) as f32),
            holo: ((mon.x - 21) as f32, (desk_y + 5) as f32),
            rack: Area {
                x: win_w + 6,
                y: 6,
                w: w - win_w - 9,
                h: 2 * mon.y - 10,
            },
            vent: ((mon.x + mon.w + 8) as f32, (2 * mon.y + 8) as f32, 5),
            sign: (mon.x + mon.w + 1, mon.y + 9),
            towers: Vec::new(),
            cars: Vec::new(),
        };
        desk.make_city();
        desk
    }

    /// The towers through the glass and the cars in the air, the same for a size.
    fn make_city(&mut self) {
        let win = self.win;
        let mut r = Dice(11 * 977 + self.w as u64);
        let rows = [
            (0usize, 0.30f32, 0.62f32, 4i32, 9i32, 1i32),
            (1, 0.40, 0.82, 5, 11, 2),
            (2, 0.25, 0.70, 8, 16, 5),
        ];
        for (layer, lo, hi, wmin, wmax, gap) in rows {
            let mut x = win.x - (r.next() * 6.0) as i32;
            while x < win.x + win.w {
                let tw = wmin + (r.next() * (wmax - wmin) as f32) as i32;
                let top =
                    win.y + win.h - ((lo + r.next() * (hi - lo)) * win.h as f32).round() as i32;
                let s = (r.next() * 1e6) as u64;
                let sign = r.next() < 0.35;
                let mast = r.next() < 0.4;
                self.towers.push(Tower {
                    layer,
                    x,
                    w: tw,
                    top,
                    s,
                    sign,
                    mast,
                    ad: false,
                });
                x += tw + (r.next() * (gap + 1) as f32) as i32;
                if layer == 2 {
                    x += (r.next() * 10.0) as i32;
                }
            }
        }
        let near: Vec<usize> = (0..self.towers.len())
            .filter(|&i| self.towers[i].layer == 2 && self.towers[i].w >= 10)
            .collect();
        if !near.is_empty() {
            let k = near[(r.next() * near.len() as f32) as usize % near.len()];
            self.towers[k].ad = true;
        }
        for _ in 0..7 {
            let depth = r.next();
            let y = win.y + (win.h as f32 * (0.12 + 0.45 * r.next())).round() as i32;
            let speed = 0.25 + depth * 0.9;
            let dir = if r.next() < 0.5 { 1.0 } else { -1.0 };
            let off = r.next() * 5000.0;
            let size = if depth < 0.4 {
                1
            } else if depth < 0.8 {
                2
            } else {
                3
            };
            self.cars.push(Car {
                y,
                speed,
                dir,
                off,
                size,
                depth,
            });
        }
    }

    /// The desk at `tick` into `area`, the cover's words on the screen.
    pub fn draw(&self, buf: &mut Buffer, area: Rect, tick: u64) {
        let px = self.pixels(tick);
        self.fold(buf, area, |x, y| px.get(x as f32, y as f32));
        self.words(buf, area, tick);
    }

    /// The desk leaned in by `e` (0 at the desk, 1 all the way): the screen grown toward
    /// `to` (a rect of `area`, in cells), the room round it dimmed as it comes. No words.
    pub fn draw_leaned(&self, buf: &mut Buffer, area: Rect, tick: u64, e: f32, to: Rect) {
        let px = self.pixels(tick);
        let e = e.clamp(0.0, 1.0);
        let t = Area {
            x: i32::from(to.x.saturating_sub(area.x)),
            y: i32::from(to.y.saturating_sub(area.y)),
            w: i32::from(to.width).max(1),
            h: i32::from(to.height).max(1),
        };
        let s = Area {
            x: self.mon.x + 1,
            y: self.mon.y + 1,
            w: self.mon.w - 2,
            h: self.mon.h - 2,
        };
        self.fold(buf, area, |x, y| {
            let (x, y) = (x as f32, y as f32);
            let sx = s.x as f32 + (x - t.x as f32) * s.w as f32 / t.w as f32;
            let sy = 2.0 * s.y as f32 + (y - 2.0 * t.y as f32) * s.h as f32 / t.h as f32;
            let c = px.get(x + e * (sx - x), y + e * (sy - y));
            let inside = x >= t.x as f32
                && x < (t.x + t.w) as f32
                && y >= (2 * t.y) as f32
                && y < (2 * (t.y + t.h)) as f32;
            if inside { c } else { mix(c, BLACK, 0.45 * e) }
        });
    }

    /// Two pixels to a cell: equal halves a blank cell of that colour, else an upper half
    /// block over the lower.
    fn fold(&self, buf: &mut Buffer, area: Rect, pixel: impl Fn(i32, i32) -> C) {
        let r = area.intersection(buf.area);
        for cy in 0..i32::from(r.height) {
            for cx in 0..i32::from(r.width) {
                let (top, bottom) = (rgb(pixel(cx, 2 * cy)), rgb(pixel(cx, 2 * cy + 1)));
                let cell = &mut buf[(r.x + cx as u16, r.y + cy as u16)];
                cell.reset();
                if top == bottom {
                    cell.set_char(' ').set_bg(Color::Rgb(top.0, top.1, top.2));
                } else {
                    cell.set_char('▀')
                        .set_fg(Color::Rgb(top.0, top.1, top.2))
                        .set_bg(Color::Rgb(bottom.0, bottom.1, bottom.2));
                }
            }
        }
    }

    /// The whole room at `tick`, as pixels.
    fn pixels(&self, tick: u64) -> Px {
        let mut f = Frame {
            d: self,
            t: tick,
            tf: tick as f32,
            px: Px::new(self.w, 2 * self.h),
            lights: self.lights(tick),
        };
        f.city();
        f.room();
        f.drops();
        f.ceiling();
        f.monitor();
        f.keyboard();
        f.deck();
        f.can();
        f.mug();
        f.holo();
        f.px
    }

    fn lights(&self, tick: u64) -> Vec<Light> {
        let t = tick as f32;
        let m = self.mon;
        let point = |x: f32, y: f32, c: C, s: f32, r: f32| Light {
            line: false,
            x,
            x2: x,
            y,
            rx: 0.0,
            ry: 0.0,
            c,
            s,
            r,
        };
        let across = |y: f32, c: C, s: f32, r: f32| Light {
            line: true,
            x: 0.0,
            x2: self.w as f32,
            y,
            rx: 0.0,
            ry: 0.0,
            c,
            s,
            r,
        };
        vec![
            Light {
                rx: m.w as f32 / 2.0,
                ry: m.h as f32,
                ..point(
                    m.x as f32 + m.w as f32 / 2.0,
                    2.0 * (m.y as f32 + m.h as f32 / 2.0),
                    [120.0, 200.0, 255.0],
                    0.42,
                    22.0,
                )
            },
            across(
                (self.desk_y + 1) as f32,
                NEON_M,
                0.55 * (0.85 + 0.15 * (t / 40.0).sin()),
                5.0,
            ),
            across(3.0, NEON_C, 0.35, 9.0),
            point(
                self.holo.0,
                self.holo.1 - 10.0,
                NEON_C,
                0.3 * (0.8 + 0.2 * (t / 9.0).sin()),
                10.0,
            ),
            point(
                (self.sign.0 + 3) as f32,
                (2 * self.sign.1 + 2) as f32,
                [255.0, 60.0, 190.0],
                0.6 * flicker(tick, 3, true),
                12.0,
            ),
        ]
    }

    /// The words: the cover on the screen, the sign on the wall.
    fn words(&self, buf: &mut Buffer, area: Rect, tick: u64) {
        let put = |buf: &mut Buffer, x: i32, y: i32, s: &str, fg: C, bold: bool| {
            for (k, ch) in s.chars().enumerate() {
                let (cx, cy) = (x + k as i32, y);
                if cx < 0 || cy < 0 || cx >= i32::from(area.width) || cy >= i32::from(area.height) {
                    continue;
                }
                let cell = &mut buf[(area.x + cx as u16, area.y + cy as u16)];
                let bg = cell.bg;
                let mut style = Style::default().fg(color(fg)).bg(bg);
                if bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                cell.set_char(ch).set_style(style);
            }
        };
        let m = self.mon;
        let (ix, iy, iw, ih) = (m.x + 1, m.y + 1, m.w - 2, m.h - 2);
        // a faint line rolling down the glass
        let scan = (tick / 3) as i32 % (ih + 8);
        if scan < ih {
            for i in 0..iw {
                let (cx, cy) = (ix + i, iy + scan);
                if cx < i32::from(area.width) && cy < i32::from(area.height) {
                    let cell = &mut buf[(area.x + cx as u16, area.y + cy as u16)];
                    cell.reset();
                    cell.set_bg(color([9.0, 15.0, 24.0]));
                }
            }
        }
        let t = tick as f32;
        let pink = mul(NEON_M, 0.9 + 0.1 * (t / 11.0).sin());
        let big = ih >= 18 && iw >= 40;
        let mut y = iy + std::cmp::max(1, (ih - if big { 16 } else { 7 }) / 2);
        if big {
            let width = |s: &str| logo::width(s) as i32 - 1;
            let lx = ix + (iw - width("STRATA")) / 2;
            for (word, wx, wy) in [
                ("STRATA", lx, y),
                ("ZERO", lx + width("STRATA") - width("ZERO"), y + 6),
            ] {
                for (i, c) in word.chars().enumerate() {
                    for (r, row) in logo::glyph(c).iter().enumerate() {
                        for (k, cell) in row.chars().enumerate() {
                            if cell != ' ' {
                                let x = wx + i as i32 * logo::CELL_W as i32 + k as i32;
                                put(buf, x, wy + r as i32, &cell.to_string(), pink, false);
                            }
                        }
                    }
                }
            }
            y += 12;
        } else {
            let title = "S T R A T A   Z E R O";
            put(
                buf,
                ix + (iw - title.chars().count() as i32) / 2,
                y,
                title,
                pink,
                true,
            );
            y += 2;
        }
        let mut line = |buf: &mut Buffer, s: &str, c: C, bold: bool| {
            if y < iy + ih {
                put(buf, ix + (iw - s.chars().count() as i32) / 2, y, s, c, bold);
            }
            y += 1;
        };
        line(buf, "PLAYER'S MANUAL", NEON_C, true);
        line(buf, "high tech · low life", [160.0, 170.0, 185.0], false);
        line(buf, "", BLACK, false);
        if (tick / 18).is_multiple_of(2) {
            line(buf, "▸ press space to read", [120.0, 230.0, 200.0], false);
        }
        let neon = mul([255.0, 70.0, 190.0], flicker(tick, 3, true));
        put(buf, self.sign.0 + 1, self.sign.1, "◆ STAY", neon, true);
        put(buf, self.sign.0 + 1, self.sign.1 + 1, " WIRED", neon, true);
    }
}

/// One frame's drawing: the desk, the tick, the pixels so far and the room's lights.
struct Frame<'a> {
    d: &'a Desk,
    t: u64,
    tf: f32,
    px: Px,
    lights: Vec<Light>,
}

impl Frame<'_> {
    /// `c` as the room's light leaves it at (x, y): the screen cool, the strip magenta,
    /// the tube cyan, the sign pink.
    fn lit(&self, c: C, x: f32, y: f32, amb: f32) -> C {
        let mut k = amb;
        let mut col = [0.0; 3];
        for l in &self.lights {
            let (dx, dy) = if l.line {
                let dx = if x < l.x {
                    l.x - x
                } else if x > l.x2 {
                    x - l.x2
                } else {
                    0.0
                };
                (dx, y - l.y)
            } else {
                (
                    ((x - l.x).abs() - l.rx).max(0.0),
                    ((y - l.y).abs() - l.ry).max(0.0),
                )
            };
            let f = l.s / (1.0 + (dx * dx + dy * dy) / (l.r * l.r));
            k += 0.9 * f;
            col = add(col, l.c, 0.16 * f);
        }
        add(mul(c, k), col, 1.0)
    }

    fn set_lit(&mut self, x: f32, y: f32, c: C) {
        let c = self.lit(c, x, y, 0.62);
        self.px.set(x, y, c);
    }

    fn inside_window(&self, x: f32, y: f32) -> bool {
        let w = self.d.win;
        let (x, y) = (x.round() as i32, y.round() as i32);
        x >= w.x && x < w.x + w.w && y >= w.y && y < w.y + w.h
    }

    fn glass(&mut self, x: f32, y: f32, c: C) {
        if self.inside_window(x, y) {
            self.px.set_glass(x, y, c);
        }
    }

    fn glass_tint(&mut self, x: f32, y: f32, c: C, f: f32) {
        if self.inside_window(x, y) {
            self.px.tint(x, y, c, f);
        }
    }

    /// The city: sky, a searchlight, three rows of towers with their windows, signs and
    /// an ad, the cars, the rain.
    fn city(&mut self) {
        let (t, d) = (self.tf, self.d);
        let win = d.win;
        let (x1, y1) = (win.x + win.w, win.y + win.h);
        for y in win.y..y1 {
            let f = (y - win.y) as f32 / win.h as f32;
            let c = mix(SKY_TOP, SKY_LOW, f.powf(1.6));
            for x in win.x..x1 {
                self.glass(x as f32, y as f32, c);
            }
        }
        let beam = win.x as f32 + win.w as f32 * (0.5 + 0.45 * (t / 140.0).sin());
        let half = win.h as f32 * 0.5;
        for y in win.y..win.y + half as i32 {
            let bx = beam + (y - win.y) as f32 * 0.6;
            for k in -1..=1 {
                let f = 0.08 * (1.0 - (k as f32).abs() * 0.5) * (1.0 - (y - win.y) as f32 / half);
                self.glass_tint(bx + k as f32, y as f32, [120.0, 110.0, 170.0], f);
            }
        }
        self.cars(|c| c.depth < 0.45);
        for tw in &d.towers {
            let base = [FAR, MID, NEAR][tw.layer];
            for y in tw.top.max(win.y)..y1 {
                for x in tw.x.max(win.x)..(tw.x + tw.w).min(x1) {
                    let hz = if tw.layer == 0 {
                        0.25 * (y - win.y) as f32 / win.h as f32
                    } else {
                        0.0
                    };
                    self.glass(x as f32, y as f32, mix(base, SKY_LOW, hz));
                }
            }
            if tw.mast && tw.top - 4 > win.y {
                let mx = (tw.x + tw.w / 2) as f32;
                for k in 1..=4 {
                    self.glass(mx, (tw.top - k) as f32, base);
                }
                if (self.t / 20 + tw.s).is_multiple_of(3) {
                    self.glass(mx, (tw.top - 5) as f32, [255.0, 40.0, 60.0]);
                }
            }
            // the windows: a grid near, scattered further back; now and then one goes off
            let density = [0.18, 0.3, 0.42][tw.layer];
            let bright = [0.45, 0.7, 0.95][tw.layer];
            let mut y = tw.top + 2;
            while y < y1 - 1 {
                let mut x = tw.x + 1;
                while x < tw.x + tw.w - 1 {
                    let (xi, yi) = (i64::from(x), i64::from(y));
                    let k = hash(xi, yi, tw.s as i64);
                    if k <= density {
                        let period = 300 + (hash(yi, xi, 0) * 900.0) as u64;
                        let phase = (self.t + (k * 9000.0) as u64) / period;
                        if hash(xi, yi, phase as i64) >= 0.12 {
                            let c = WINDOWS[(hash(yi, xi, tw.s as i64) * 5.0) as usize % 5];
                            self.glass(x as f32, y as f32, mul(c, bright));
                        }
                    }
                    x += 2;
                }
                y += 2;
            }
            if tw.sign && tw.layer >= 1 {
                let c = [NEON_M, NEON_C, [255.0, 120.0, 40.0]][(tw.s % 3) as usize];
                let f = flicker(self.t, (tw.s % 7) as u8, tw.s % 4 == 0);
                let sx = (tw.x + if tw.s % 2 == 1 { 1 } else { tw.w - 2 }) as f32;
                let sy = tw.top + 3 + (tw.s % 5) as i32;
                for k in 0..6 + (tw.s % 5) as i32 {
                    if sy + k >= y1 {
                        break;
                    }
                    let y = (sy + k) as f32;
                    self.glass(sx, y, mul(c, f));
                    self.glass_tint(sx - 1.0, y, c, 0.25 * f);
                    self.glass_tint(sx + 1.0, y, c, 0.25 * f);
                }
            }
            if tw.ad {
                let (ax, ay, aw, ah) = (tw.x + 2, tw.top + 4, tw.w - 4, 6);
                for j in 0..ah {
                    for i in 0..aw {
                        let ph =
                            (t / 60.0 + i as f32 / aw as f32 * 0.5 + j as f32 / ah as f32 * 0.2)
                                % 1.0;
                        let c = if ph < 0.5 {
                            mix(NEON_M, NEON_C, ph * 2.0)
                        } else {
                            mix(NEON_C, NEON_M, (ph - 0.5) * 2.0)
                        };
                        let bar = if (self.t / 4 + i as u64) % aw as u64 <= 2 && j == 3 {
                            1.4
                        } else {
                            1.0
                        };
                        self.glass((ax + i) as f32, (ay + j) as f32, mul(c, 0.8 * bar));
                    }
                }
                for i in -2..aw + 2 {
                    for j in [-1, ah] {
                        self.glass_tint((ax + i) as f32, (ay + j) as f32, NEON_M, 0.2);
                    }
                }
            }
        }
        self.cars(|c| c.depth >= 0.45);
        // the rain, falling a little sideways
        let n = (win.w * win.h) / 45;
        for i in 0..i64::from(n) {
            let speed = 1.2 + hash(i, 1, 0) * 1.4;
            let x0 = win.x as f32 + hash(i, 2, 0) * (win.w + 20) as f32;
            let y = win.y as f32
                + ((t * speed + hash(i, 3, 0) * win.h as f32 * 3.0) % (win.h + 6) as f32)
                - 3.0;
            for k in 0..4 {
                let yy = (y - k as f32).round();
                let xx = (x0 - (y - k as f32) * 0.35).round();
                self.glass_tint(xx, yy, [160.0, 180.0, 220.0], 0.34 - k as f32 * 0.07);
            }
        }
    }

    /// The cars `pick` takes: a body, a headlight, a red streak behind; now and then.
    fn cars(&mut self, pick: impl Fn(&Car) -> bool) {
        let d = self.d;
        let win = d.win;
        let span = (win.w + 40) as f32;
        for car in d.cars.iter().filter(|c| pick(c)) {
            let run = self.tf * car.speed + car.off;
            let lap = (run / span) as i64;
            if hash(lap, car.off.round() as i64, 0) > 0.55 {
                continue;
            }
            let along = run % span - 20.0;
            let x = if car.dir > 0.0 {
                win.x as f32 + along
            } else {
                (win.x + win.w) as f32 - along
            };
            let (head, y, size) = (car.dir, car.y as f32, car.size as f32);
            let fade = 0.4 + 0.6 * car.depth;
            let trail = 8 * car.size;
            for k in 1..=trail {
                let f = 0.35 * fade * (1.0 - k as f32 / trail as f32);
                self.glass_tint(x - head * (k as f32 + size), y, [255.0, 60.0, 90.0], f);
            }
            for k in 0..=car.size {
                let c = mix([18.0, 18.0, 26.0], [60.0, 60.0, 74.0], car.depth);
                self.glass(x - head * k as f32, y, c);
            }
            if car.size >= 2 {
                for k in 0..car.size {
                    self.glass(x - head * k as f32, y + 1.0, [14.0, 14.0, 20.0]);
                }
            }
            self.glass(
                x + head,
                y,
                mix([140.0, 130.0, 110.0], [255.0, 244.0, 210.0], fade),
            );
            self.glass(x - head * (size + 1.0), y, [255.0, 40.0, 70.0]);
        }
    }

    /// Drops on the glass: some still, some sliding down slow.
    fn drops(&mut self) {
        let win = self.d.win;
        let n = (win.w * win.h) / 160;
        for i in 0..i64::from(n) {
            let x = (win.x as f32 + hash(i, 11, 0) * win.w as f32).round();
            let still = hash(i, 12, 0) < 0.5;
            let speed = if still {
                0.0
            } else {
                0.03 + hash(i, 13, 0) * 0.12
            };
            let y = (win.y as f32
                + (hash(i, 14, 0) * win.h as f32 + self.tf * speed) % win.h as f32)
                .round();
            if !self.px.is_glass(x, y) {
                continue;
            }
            self.px.tint(x, y, [190.0, 205.0, 235.0], 0.32);
            if !still {
                for k in 1..4 {
                    if self.px.is_glass(x, y - k as f32) {
                        self.px.tint(
                            x,
                            y - k as f32,
                            [170.0, 185.0, 220.0],
                            0.12 - k as f32 * 0.03,
                        );
                    }
                }
            }
        }
    }

    /// The room: concrete with its seams, the riveted frame, the rack, the vent, the desk.
    fn room(&mut self) {
        let d = self.d;
        let (w, win) = (self.px.w, d.win);
        for y in 0..d.desk_y {
            for x in 0..w {
                if self.px.glass[(y * w + x) as usize] {
                    continue;
                }
                let seam = x % 24 == 0 || y % 18 == 0;
                let grit = if hash(i64::from(x), i64::from(y), 3) < 0.08 {
                    0.85
                } else {
                    1.0
                };
                let base = if y < win.y - 1 {
                    CEIL
                } else if seam {
                    SEAM
                } else {
                    CONCRETE
                };
                self.set_lit(x as f32, y as f32, mul(base, grit));
            }
        }
        self.girder(win.x - 2, win.y - 2, win.w + 4, 2);
        self.girder(win.x - 2, win.y, 2, win.h);
        self.girder(win.x + win.w, win.y, 2, win.h);
        self.girder(
            win.x + (win.w as f32 * 0.5).round() as i32 - 1,
            win.y,
            2,
            win.h,
        );
        self.girder(
            win.x,
            win.y + (win.h as f32 * 0.18).round() as i32,
            win.w,
            1,
        );
        self.girder(win.x - 3, win.y + win.h, win.w + 6, 3);
        self.rack();
        self.vent();
        self.desk();
    }

    /// A run of the window's frame, riveted where it's thin.
    fn girder(&mut self, x: i32, y: i32, w: i32, h: i32) {
        for j in 0..h {
            for i in 0..w {
                let rivet = (i + j) % 7 == 3 && (w <= 2 || h <= 2);
                let c = if rivet {
                    FRAME_LIT
                } else if j == 0 || i == 0 {
                    mix(FRAME, FRAME_LIT, 0.5)
                } else {
                    FRAME
                };
                self.set_lit((x + i) as f32, (y + j) as f32, c);
            }
        }
    }

    /// The rack: gunmetal uprights, three shelves; servers blinking, an eye in a jar, a
    /// coil of cable, a box of shards, a chrome arm hung on two hooks.
    fn rack(&mut self) {
        let r = self.d.rack;
        if r.w < 24 || r.h < 12 {
            return;
        }
        let t = self.t;
        let shelves = [
            r.y + (r.h as f32 * 0.33).round() as i32,
            r.y + (r.h as f32 * 0.66).round() as i32,
            r.y + r.h,
        ];
        for x in [r.x, r.x + r.w - 1] {
            for y in r.y..=r.y + r.h {
                self.set_lit(x as f32, y as f32, METAL_LIT);
            }
        }
        for y in shelves {
            for x in r.x..r.x + r.w {
                self.set_lit(x as f32, y as f32, METAL_LIT);
                if x % 3 == 0 {
                    self.set_lit(x as f32, (y + 1) as f32, METAL_DARK);
                }
            }
        }
        // two servers, their lights going at their own rates
        let s1 = shelves[0];
        for u in 0..2 {
            let ux = r.x + 2 + u * (r.w as f32 * 0.38).round() as i32;
            let uw = (r.w as f32 * 0.34).round() as i32;
            for j in 0..4 {
                for i in 0..uw {
                    self.set_lit((ux + i) as f32, (s1 - 4 + j) as f32, METAL);
                }
            }
            let mut i = 0;
            while i < uw - 2 {
                let rate = 4 + ((i * 7) % 11) as u64;
                if hash(i64::from(ux + i), i64::from(u), (t / rate) as i64) < 0.55 {
                    self.px.set(
                        (ux + 1 + i) as f32,
                        (s1 - 3) as f32,
                        LEDS[((i + u) % 2) as usize],
                    );
                }
                if hash(i64::from(i), i64::from(u), (t / 23) as i64) < 0.3 {
                    self.px.set((ux + 1 + i) as f32, (s1 - 2) as f32, LEDS[2]);
                }
                i += 2;
            }
        }
        // the eye in its jar, looking round
        let s2 = shelves[1];
        let jx = r.x + 3;
        for j in 0..7 {
            for i in 0..5 {
                self.set_lit((jx + i) as f32, (s2 - 7 + j) as f32, [30.0, 70.0, 60.0]);
            }
        }
        for i in 0..5 {
            self.set_lit((jx + i) as f32, (s2 - 8) as f32, METAL_LIT);
        }
        let look =
            [(0, 0), (1, 0), (-1, 0), (0, -1)][(hash((t / 70) as i64, 9, 0) * 4.0) as usize % 4];
        self.px
            .set((jx + 2) as f32, (s2 - 4) as f32, [230.0, 236.0, 230.0]);
        self.px
            .set((jx + 2 + look.0) as f32, (s2 - 4 + look.1) as f32, NEON_C);
        for k in 0..3 {
            let bubble = ((t / 30 + k) % 5) as i32;
            self.px.tint(
                (jx + 1 + k as i32) as f32,
                (s2 - 6 + bubble) as f32,
                [120.0, 255.0, 200.0],
                0.25,
            );
        }
        for k in 0..6 {
            let a = k as f32;
            self.px.set(
                (jx + 9) as f32 + (a.cos() * 2.0).round(),
                (s2 - 2) as f32 - (a.sin() * 1.2).round(),
                CABLE,
            );
        }
        for j in 0..3 {
            for i in 0..6 {
                self.set_lit(
                    (jx + 14 + i) as f32,
                    (s2 - 3 + j) as f32,
                    [60.0, 30.0, 40.0],
                );
            }
        }
        for i in 0..3 {
            self.px.set(
                (jx + 15 + i * 2) as f32,
                (s2 - 3) as f32,
                LEDS[((i + 1) % 4) as usize],
            );
        }
        // the arm, hung on its hooks, its joint lit
        let s3 = shelves[2] as f32;
        let ax = (r.x + (r.w as f32 * 0.55).round() as i32) as f32;
        for hx in [ax, ax + 9.0] {
            self.set_lit(hx, s3 - 9.0, METAL_LIT);
        }
        let upper = self.lit([150.0, 156.0, 170.0], ax, s3 - 8.0, 0.62);
        let under = self.lit([90.0, 96.0, 110.0], ax, s3 - 7.0, 0.62);
        let fore = self.lit([130.0, 136.0, 150.0], ax + 9.0, s3 - 4.0, 0.62);
        self.px
            .line((ax, s3 - 8.0), (ax + 8.0, s3 - 8.0), upper, 1.0);
        self.px
            .line((ax, s3 - 7.0), (ax + 8.0, s3 - 7.0), under, 1.0);
        self.px
            .line((ax + 8.0, s3 - 7.0), (ax + 11.0, s3 - 2.0), fore, 1.0);
        let hand = self.lit([110.0, 116.0, 130.0], ax + 10.0, s3 - 1.0, 0.62);
        self.px.rect(ax + 10.0, s3 - 2.0, 3, 2, hand);
        self.px.set(
            ax + 8.0,
            s3 - 8.0,
            mul(NEON_C, 0.7 + 0.3 * (self.tf / 8.0).sin()),
        );
    }

    /// The vent fan in the wall, turning.
    fn vent(&mut self) {
        let (vx, vy, r) = self.d.vent;
        if vx + r as f32 >= self.px.w as f32 {
            return;
        }
        for j in -r..=r {
            for i in -r..=r {
                let d = ((i * i + j * j) as f32).sqrt();
                if d <= r as f32 {
                    let c = if d > (r - 1) as f32 {
                        METAL_LIT
                    } else {
                        METAL_DARK
                    };
                    self.set_lit(vx + i as f32, vy + j as f32, c);
                }
            }
        }
        let blade = self.lit(METAL, vx, vy, 0.62);
        for b in 0..3 {
            let a = self.tf * 0.18 + b as f32 * (2.0 * std::f32::consts::PI / 3.0);
            let reach = (r - 1) as f32;
            self.px.line(
                (vx, vy),
                (vx + a.cos() * reach, vy + a.sin() * reach),
                blade,
                1.0,
            );
        }
        self.set_lit(vx, vy, METAL_LIT);
    }

    /// The desk: worn gunmetal, a neon strip along its back edge.
    fn desk(&mut self) {
        let (w, h, top) = (self.px.w, self.px.h, self.d.desk_y);
        for y in top..h {
            let f = (y - top) as f32 / (h - top).max(1) as f32;
            for x in 0..w {
                let panel = if x % 40 == 0 { 0.8 } else { 1.0 };
                let wear = if hash(i64::from(x), i64::from(y), 5) < 0.04 {
                    1.25
                } else {
                    1.0
                };
                let c = mul(mix(METAL_DARK, METAL, 0.4 + f * 0.6), panel * wear);
                let c = self.lit(c, x as f32, y as f32, 0.5);
                self.px.set(x as f32, y as f32, c);
            }
        }
        let pulse = 0.85 + 0.15 * (self.tf / 40.0).sin();
        for x in 0..w {
            let x = x as f32;
            self.px.set(x, top as f32, mul(NEON_M, pulse));
            self.px.tint(x, (top + 1) as f32, NEON_M, 0.35 * pulse);
            self.px.tint(x, (top - 1) as f32, NEON_M, 0.25 * pulse);
        }
    }

    /// The pipe along the ceiling, the cyan tube under it (a stretch dead, a stretch
    /// flickering), the cables sagging off it.
    fn ceiling(&mut self) {
        let w = self.px.w;
        for x in 0..w {
            let xf = x as f32;
            self.set_lit(xf, 0.0, PIPE_LIT);
            self.set_lit(xf, 1.0, PIPE);
            if x % 22 == 5 {
                self.set_lit(xf, 2.0, PIPE_LIT);
                self.set_lit(xf + 1.0, 2.0, PIPE);
            }
        }
        let tube = 0.9 + 0.1 * (self.tf / 13.0).sin();
        let (dead, flick) = ((w as f32 * 0.62) as i32, (w as f32 * 0.58) as i32);
        let dead_end = (w as f32 * 0.66) as i32;
        for x in 2..w - 2 {
            if dead < x && x < dead_end {
                continue;
            }
            let f = if flick < x && x < dead {
                flicker(self.t, 5, true)
            } else {
                1.0
            };
            self.px.set(x as f32, 3.0, mul(NEON_C, tube * f));
        }
        for (a, b, sag) in [(0.1f32, 0.3f32, 6.0f32), (0.35, 0.55, 9.0)] {
            let (x0, x1) = ((w as f32 * a).round() as i32, (w as f32 * b).round() as i32);
            let sway = (self.tf / 70.0 + a * 9.0).sin() * 0.8;
            for x in x0..=x1 {
                let u = (x - x0) as f32 / (x1 - x0).max(1) as f32;
                let y = 4.0 + (sag + sway) * 4.0 * u * (1.0 - u);
                self.px.set(x as f32, y, CABLE);
            }
        }
    }

    /// The screen on its arm: the bezel, stickers, a status light, the glass dark.
    fn monitor(&mut self) {
        let m = self.d.mon;
        let (x, y, w, h) = (m.x, 2 * m.y, m.w, 2 * m.h);
        for j in 0..h {
            for i in 0..w {
                let edge = j < 2 || j >= h - 2 || i == 0 || i == w - 1;
                let c = if !edge {
                    SCREEN
                } else if j == 0 {
                    BEZEL_LIT
                } else {
                    BEZEL
                };
                self.px.set((x + i) as f32, (y + j) as f32, c);
            }
        }
        let (xf, yf, wf, hf) = (x as f32, y as f32, w as f32, h as f32);
        self.px
            .rect(xf + 2.0, yf + hf - 2.0, 2, 2, [230.0, 196.0, 70.0]);
        self.px.rect(xf + 5.0, yf + hf - 2.0, 3, 1, NEON_M);
        self.px.rect(xf + wf - 6.0, yf, 3, 2, NEON_C);
        let status = if (self.t / 45) % 2 == 1 {
            [0.0, 255.0, 140.0]
        } else {
            [0.0, 120.0, 70.0]
        };
        self.px.set(xf + wf - 3.0, yf + hf - 1.0, status);
        let nx = x + w / 2 - 1;
        for j in y + h..self.d.desk_y + 3 {
            for i in 0..3 {
                self.set_lit((nx + i) as f32, j as f32, METAL);
            }
        }
        for i in 0..15 {
            self.set_lit((nx - 6 + i) as f32, (self.d.desk_y + 3) as f32, METAL_LIT);
        }
    }

    /// The keyboard and its underglow turning through the neon.
    fn keyboard(&mut self) {
        let k = self.d.kb;
        for j in 0..4 {
            for i in 0..k.w {
                let c = if j < 3 && i % 2 == 0 { KEY_TOP } else { KEYS };
                self.set_lit((k.x + i) as f32, (k.y + j) as f32, c);
            }
        }
        for i in -1..=k.w {
            let ph = (i as f32 / k.w as f32 + self.tf / 300.0) % 1.0;
            let c = mix(NEON_M, NEON_C, (ph * 2.0 - 1.0).abs());
            self.px.tint((k.x + i) as f32, (k.y + 4) as f32, c, 0.5);
        }
    }

    /// Your deck: keys, a screen tracing a signal, a light, and its cable running off the
    /// bottom of the picture to the jack behind your ear.
    fn deck(&mut self) {
        let d = self.d.deck;
        let (x, y) = (d.x as f32, d.y as f32);
        for j in 0..4 {
            for i in 0..d.w {
                let c = if j == 0 { METAL_LIT } else { METAL };
                self.set_lit(x + i as f32, y + j as f32, c);
            }
        }
        let mut i = 1;
        while i < d.w - 7 {
            self.set_lit(x + i as f32, y + 2.0, KEY_TOP);
            i += 2;
        }
        let sx = x + (d.w - 6) as f32;
        self.px.rect(sx, y + 1.0, 5, 2, [4.0, 16.0, 10.0]);
        for i in 0..5 {
            let v = if ((i as f32 + self.tf * 0.3) * 1.3).sin() > 0.2 {
                0.0
            } else {
                1.0
            };
            self.px
                .set(sx + i as f32, y + 1.0 + v, [80.0, 255.0, 140.0]);
        }
        let light = if (self.t / 12).is_multiple_of(3) {
            [80.0, 0.0, 60.0]
        } else {
            NEON_M
        };
        self.px.set(x + 1.0, y + 3.0, light);
        let h = self.px.h as f32;
        for k in 0..40 {
            let u = k as f32 / 39.0;
            let cx = x + 3.0 + (u * 3.0).sin() * 4.0 - u * 2.0;
            let cy = y + 4.0 + u * (h - y - 3.0);
            self.px.set(cx, cy, CABLE);
            if k % 6 == 0 {
                self.px.set(cx + 1.0, cy, [30.0, 30.0, 40.0]);
            }
        }
    }

    /// A can of something loud.
    fn can(&mut self) {
        let (x, y) = self.d.can;
        for j in 0..6 {
            for i in 0..3 {
                let c = match j {
                    0 | 5 => [150.0, 156.0, 170.0],
                    2 => [240.0, 240.0, 250.0],
                    _ => mix(NEON_M, [120.0, 0.0, 90.0], i as f32 / 2.0),
                };
                let c = self.lit(c, x + i as f32, y - 3.0, 0.75);
                self.px.set(x + i as f32, y - 6.0 + j as f32, c);
            }
        }
    }

    /// The brew: a black mug with a band of light round it, its steam curling up and
    /// thinning out.
    fn mug(&mut self) {
        let (x, y, r) = self.d.mug;
        let (w, h) = (2 * r + 1, (r as f32 * 1.9).round() as i32 + 1);
        let top = y - h as f32;
        for j in 0..h {
            for i in 0..w {
                let side = (i - r).abs() as f32 / r as f32;
                let (px, py) = (x - r as f32 + i as f32, top + j as f32);
                let c = if j == (h as f32 * 0.55).round() as i32 {
                    mul(NEON_C, 0.85 - side * 0.4)
                } else {
                    let c = mix(
                        mix(MUG, [60.0, 62.0, 78.0], 0.3),
                        [10.0, 10.0, 14.0],
                        side * side * 0.8,
                    );
                    self.lit(c, px, py, 0.75)
                };
                self.px.set(px, py, c);
            }
        }
        for i in 1..w - 1 {
            let c = if i == 1 {
                mix(BREW, [90.0, 54.0, 34.0], 0.5)
            } else {
                BREW
            };
            self.px.set(x - r as f32 + i as f32, top, c);
        }
        let hx = x + r as f32 + 1.0;
        for j in 2..h - 2 {
            let out = if j == 2 || j == h - 3 { 0.0 } else { 1.0 };
            self.set_lit(hx + out, top + j as f32, MUG);
        }
        self.set_lit(hx, top + 2.0, MUG);
        let tall = (r as f32 * 4.5).round() as i32 + 6;
        let t = self.tf;
        for k in 0..3 {
            let kf = k as f32;
            for s in 1..tall {
                let sf = s as f32;
                let life = sf / tall as f32;
                let sway = (sf * 0.38 - t * 0.07 + kf * 2.1).sin() * (0.6 + life * r as f32 * 0.5)
                    + (kf - 1.0) * r as f32 * 0.45;
                let a =
                    (1.0 - life) * 0.38 * (0.55 + 0.45 * (t * 0.045 + kf * 1.9 + sf * 0.15).sin());
                if a > 0.02 {
                    self.px.tint(x + sway, top - sf, STEAM, a);
                }
            }
        }
    }

    /// The holo-projector: a puck throwing a wireframe cube that turns, and glitches.
    fn holo(&mut self) {
        let (x, y) = self.d.holo;
        for j in 0..2 {
            for i in -2..=2 {
                self.set_lit(x + i as f32, y + j as f32, [28.0, 28.0, 36.0]);
            }
        }
        self.px.set(x, y, NEON_C);
        for j in 1..9 {
            for i in -(j / 3)..=(j / 3) {
                self.px.tint(x + i as f32, y - j as f32, NEON_C, 0.07);
            }
        }
        let (cy, size) = (y - 9.0, 3.6);
        let (a, b) = (self.tf * 0.045, 0.55f32);
        let mut v = Vec::with_capacity(8);
        for xs in [-1.0f32, 1.0] {
            for ys in [-1.0f32, 1.0] {
                for zs in [-1.0f32, 1.0] {
                    let rx = xs * a.cos() - zs * a.sin();
                    let rz = xs * a.sin() + zs * a.cos();
                    let ry = ys * b.cos() - rz * b.sin();
                    v.push((x + rx * size, cy + ry * size));
                }
            }
        }
        let glitch = if (self.t / 4).is_multiple_of(37) {
            1.0
        } else {
            0.0
        };
        for (p, q) in [
            (0, 1),
            (2, 3),
            (4, 5),
            (6, 7),
            (0, 2),
            (1, 3),
            (4, 6),
            (5, 7),
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
        ] {
            let from = (v[p].0 + glitch, v[p].1);
            self.px.line(from, v[q], NEON_C, 0.7);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::testutil::buffer_text;

    fn draw(w: u16, h: u16, tick: u64) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        let area = buf.area;
        Desk::new(w, h).draw(&mut buf, area, tick);
        buf
    }

    #[test]
    fn the_desk_draws_at_every_size_with_the_cover_on_its_screen() {
        for (w, h) in [(100u16, 30u16), (120, 40), (200, 60), (160, 50)] {
            for tick in [0, 17, 999] {
                let text = buffer_text(&draw(w, h, tick));
                assert!(
                    text.contains("PLAYER'S MANUAL"),
                    "{w}x{h} at {tick}:\n{text}"
                );
                assert!(text.contains("STAY"), "{w}x{h}: the sign");
            }
        }
    }

    #[test]
    fn the_same_tick_draws_the_same_picture_and_time_moves_it() {
        let a = draw(120, 40, 300);
        assert_eq!(a, draw(120, 40, 300));
        assert_ne!(
            a,
            draw(120, 40, 301),
            "the rain, the steam and the rest move"
        );
    }

    #[test]
    fn leaning_in_turns_the_screen_into_the_target() {
        let (w, h) = (120u16, 40u16);
        let to = Rect::new(13, 5, 94, 28);
        let desk = Desk::new(w, h);
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        let area = buf.area;
        desk.draw_leaned(&mut buf, area, 50, 1.0, to);
        // all the way in, the target is the dark of the screen's glass
        let mid = &buf[(to.x + 40, to.y + 14)];
        assert_eq!(mid.bg, color(SCREEN), "the screen fills the target");
        // halfway, and at the desk, nothing panics at any size
        for (w, h) in [(100u16, 30u16), (94, 29), (200, 60)] {
            let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
            let to = Rect::new((w - 94) / 2, (h - 29) / 2, 94, 28);
            for e in [0.0, 0.5, 1.0] {
                let area = buf.area;
                Desk::new(w, h).draw_leaned(&mut buf, area, 9, e, to);
            }
        }
    }
}
