//! The city cut open, top to bottom: the Heights' spires, the skyway bridges, the
//! Street's neon, the Undercity's pipes and the Net's lattice, in half-block pixels.
//! Drawn from a city's seed, so each city's skyline differs a little; it never moves.

use super::cyberdeck::Pen;
use super::pixels::Pixels;
use crate::render::Rgb;
use citygen::rng::hash64;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// The strata, top first, and where each starts as a share of the picture's height.
pub const BANDS: [&str; 5] = ["HEIGHTS", "SKYWAYS", "STREET", "UNDERCITY", "NET"];
const STARTS: [f32; 5] = [0.0, 0.30, 0.50, 0.70, 0.86];

const SKY: Rgb = Rgb(5, 6, 13);
const HAZE: Rgb = Rgb(58, 18, 64);
const BELOW: Rgb = Rgb(8, 8, 12);
const DEEP: Rgb = Rgb(2, 4, 10);
const RIM: Rgb = Rgb(122, 136, 176);
const BACK: [Rgb; 2] = [Rgb(20, 26, 42), Rgb(23, 30, 48)];
const FRONT: [Rgb; 3] = [Rgb(31, 39, 64), Rgb(36, 45, 72), Rgb(27, 34, 54)];
const WINDOWS: [Rgb; 4] = [
    Rgb(255, 210, 122),
    Rgb(159, 216, 255),
    Rgb(255, 233, 176),
    Rgb(106, 124, 168),
];
const BEACON: Rgb = Rgb(255, 48, 64);
const NEON: [Rgb; 4] = [
    Rgb(255, 0, 200),
    Rgb(0, 229, 255),
    Rgb(255, 200, 80),
    Rgb(255, 79, 94),
];
const BRIDGE: Rgb = Rgb(59, 143, 168);
const POD: Rgb = Rgb(255, 0, 200);
const ROAD: Rgb = Rgb(90, 42, 94);
const CARS: [Rgb; 2] = [Rgb(255, 233, 176), Rgb(255, 48, 64)];
const PILLAR: Rgb = Rgb(20, 22, 30);
const PIPE: Rgb = Rgb(42, 36, 24);
const LAMP: Rgb = Rgb(208, 144, 48);
const LATTICE: Rgb = Rgb(0, 229, 255);

/// Each band's first cell row in a picture `rows` high.
pub fn band_rows(rows: u16) -> [u16; 5] {
    STARTS.map(|f| (f32::from(rows * 2) * f) as u16 / 2)
}

/// Rolls from a seed: the same seed, the same picture.
struct Dice {
    seed: u64,
    n: u64,
}

impl Dice {
    fn below(&mut self, n: i32) -> i32 {
        self.n += 1;
        if n <= 0 {
            return 0;
        }
        (hash64(self.seed, self.n) % n as u64) as i32
    }

    /// lo..hi, or lo when the range is empty.
    fn within(&mut self, lo: i32, hi: i32) -> i32 {
        lo + self.below(hi - lo)
    }

    fn chance(&mut self, p: f32) -> bool {
        (self.below(10_000) as f32) < p * 10_000.0
    }

    fn pick<T: Copy>(&mut self, from: &[T]) -> T {
        from[self.below(from.len() as i32) as usize]
    }
}

/// Paints the cutaway over `area`, every cell of it.
pub fn draw(buf: &mut Buffer, area: Rect, seed: u64) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let mut d = Dice { seed, n: 0 };
    let mut p = Pixels::new(area.width, area.height);
    let (w, ph) = (i32::from(area.width), i32::from(area.height) * 2);
    let at = |f: f32| (ph as f32 * f) as i32;
    let (skyways, street, under, net) =
        (at(STARTS[1]), at(STARTS[2]), at(STARTS[3]), at(STARTS[4]));
    let put = |p: &mut Pixels, x: i32, y: i32, c: Rgb| p.px(x, y, Some(c.to_color()));

    // The sky, the Street's haze rising into it; below the street, the dark.
    for y in 0..ph {
        let c = if y < under {
            SKY.mix(HAZE, (y as f32 / under.max(1) as f32).powf(2.2))
        } else if y < net {
            BELOW
        } else {
            DEEP
        };
        p.hline(0, w, y, c.to_color());
    }

    // Towers, the back row first (dim, tall), then the front (lit, shorter).
    for front in [false, true] {
        let (bodies, tops, lit): (&[Rgb], (i32, i32), f32) = if front {
            (&FRONT, (skyways - 6, street - 2), 0.18)
        } else {
            (&BACK, (3, skyways - 2), 0.10)
        };
        let mut x = d.within(0, 3);
        while x < w {
            let tw = if front {
                d.pick(&[3, 4, 5, 5, 6, 7])
            } else {
                d.pick(&[4, 5, 6])
            };
            let top = d.within(tops.0, tops.1);
            let body = d.pick(bodies);
            let rim = body.mix(RIM, 0.25);
            for y in top..under {
                for xx in x..(x + tw).min(w) {
                    put(&mut p, xx, y, if xx == x { rim } else { body });
                }
            }
            // Windows in columns, a few lit.
            for y in (top + 2..under - 1).step_by(2) {
                for xx in (x + 2..(x + tw).min(w) - 1).step_by(2) {
                    if d.chance(lit) {
                        let c = d.pick(&WINDOWS);
                        put(&mut p, xx, y, c);
                    }
                }
            }
            // A mast and a red beacon on the tall ones.
            if !front && top < skyways && top >= 3 {
                let mid = x + tw / 2;
                put(&mut p, mid, top - 1, body.mix(RIM, 0.3));
                put(&mut p, mid, top - 2, BEACON);
            }
            // Neon down the face at street level.
            if front && tw >= 4 && d.chance(0.6) {
                let c = d.pick(&NEON);
                let sy = d.within(street, (street + 1).max(under - 4));
                for k in 0..3 {
                    put(&mut p, x + tw - 2, sy + k, c);
                }
            }
            x += tw + d.pick(&[1, 1, 2, 3]);
        }
    }

    // Two skyway bridges, pods riding them.
    for by in [skyways + 2, skyways + 7] {
        p.hline(0, w, by, BRIDGE.to_color());
        for _ in 0..(w / 14).max(1) {
            let px = d.below(w - 1);
            put(&mut p, px, by - 1, POD);
            put(&mut p, px + 1, by - 1, POD);
        }
    }

    // The road glowing, cars' lights on it.
    p.hline(0, w, under - 1, ROAD.to_color());
    for _ in 0..(w / 7).max(1) {
        let px = d.below(w - 1);
        let c = d.pick(&CARS);
        put(&mut p, px, under - 1, c);
    }

    // The Undercity: pillars down from the street, pipes, a few lamps.
    for x in (0..w).step_by(7) {
        p.vline(x, under, net, PILLAR.to_color());
    }
    for y in [under + 3, under + 6] {
        for x in (0..w).filter(|x| (x / 11) % 3 != 2) {
            put(&mut p, x, y, PIPE);
        }
    }
    for _ in 0..(w / 8).max(1) {
        let (px, py) = (d.below(w), d.within(under + 1, net - 1));
        put(&mut p, px, py, LAMP);
    }

    // The Net: a cyan lattice in the dark, brighter where it crosses, a few nodes lit.
    for y in net + 1..ph {
        for x in 0..w {
            let (row, col) = ((y - net) % 3 == 0, x % 6 == 0);
            if row || col {
                put(
                    &mut p,
                    x,
                    y,
                    LATTICE.scale(if row && col { 0.45 } else { 0.2 }),
                );
            }
        }
    }
    for _ in 0..(w / 10).max(1) {
        let px = d.below((w + 5) / 6) * 6;
        let py = net + 3 * d.within(1, ((ph - net) / 3).max(2));
        if py < ph {
            put(&mut p, px, py, LATTICE);
        }
    }

    let mut pen = Pen {
        buf,
        ox: area.x,
        oy: area.y,
        clip: area,
    };
    p.fold(&mut pen, 0, 0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::chrome::PANEL_BG;

    fn painted(seed: u64) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 30));
        for cell in &mut buf.content {
            cell.set_bg(PANEL_BG);
        }
        draw(&mut buf, Rect::new(5, 3, 48, 22), seed);
        buf
    }

    #[test]
    fn the_cutaway_fills_its_area_and_no_more() {
        let buf = painted(7);
        let inside = Rect::new(5, 3, 48, 22);
        for y in 0..30 {
            for x in 0..60 {
                let cell = &buf[(x, y)];
                if inside.contains((x, y).into()) {
                    assert!(
                        cell.bg != PANEL_BG || cell.symbol() == "▀",
                        "({x}, {y}) is painted"
                    );
                } else {
                    assert_eq!((cell.symbol(), cell.bg), (" ", PANEL_BG), "({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn the_cutaway_is_the_same_for_a_seed() {
        assert_eq!(painted(7), painted(7));
        assert_ne!(painted(7), painted(8));
    }

    #[test]
    fn band_rows_split_the_strata() {
        assert_eq!(band_rows(22), [0, 6, 11, 15, 18]);
    }
}
