//! The HOLO's caller: a hologram face that flickers and, every second or so, glitches.
//! Calm, the scanlines crawl; in a burst, pixel rows slip sideways, the colour splits
//! into a magenta and a cyan fringe, and a band loses a channel or drops out.

use super::content::Face;
use crate::ui::chrome::{CYAN, MAGENTA, PANEL_BG};
use crate::ui::cyberdeck::Pen;
use crate::ui::pixels::Pixels;
use citygen::rng::hash64;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::collections::BTreeSet;

/// The face's width in cells: 16, and two empty columns each side for the slips.
#[allow(dead_code)] // the HOLO feed and call use it (Tasks 6, 7)
pub const FACE_W: u16 = 20;
const PAD: usize = 2;
/// Ticks in one calm-then-burst cycle.
const CYCLE: u64 = 60;
const RECIPES: u64 = 5;
/// The height the recipes' rows are written for.
const DESIGN_ROWS: usize = 18;

type Rgb = (u8, u8, u8);
type Grid = Vec<Vec<Option<Rgb>>>;

const CYAN_RGB: Rgb = (0, 229, 255);
const MAGENTA_RGB: Rgb = (255, 0, 200);

/// The face's height in cells.
#[allow(dead_code)] // the HOLO feed and call draw it (Tasks 6, 7)
pub fn rows(face: &Face) -> u16 {
    (face.rows.len() / 2) as u16
}

/// The recipe (0..5) drawn at this tick, `None` when calm. Each 60-tick cycle holds
/// one burst of 2 to 5 frames of 2 or 3 ticks, starting somewhere in its first half.
#[allow(dead_code)] // the HOLO feed and call draw it (Tasks 6, 7)
pub fn burst(tick: u64, seed: u64) -> Option<u8> {
    let h = hash64(seed, tick / CYCLE);
    let at = tick % CYCLE;
    let start = h % 30;
    let frames = 2 + h % 4;
    let each = 2 + (h >> 8) % 2;
    if at < start || at >= start + frames * each {
        return None;
    }
    let frame = (at - start) / each;
    Some((((h >> 16) + frame) % RECIPES) as u8)
}

/// The face at this tick, 20 pixels wide and twice `rows` high. Pure.
#[allow(dead_code)] // the HOLO feed and call draw it (Tasks 6, 7)
pub fn pixels(face: &Face, tick: u64, seed: u64) -> Pixels {
    render(face, ((tick / 4) % 2) as usize, burst(tick, seed))
}

fn render(face: &Face, phase: usize, recipe: Option<u8>) -> Pixels {
    let mut g = grid(face, phase);
    if let Some(r) = recipe {
        apply(&mut g, r);
    }
    let mut out = Pixels::new(FACE_W, rows(face));
    for (y, row) in g.iter().enumerate() {
        for (x, c) in row.iter().enumerate() {
            out.px(x as i32, y as i32, c.map(|(r, g, b)| Color::Rgb(r, g, b)));
        }
    }
    out
}

fn scale(c: Rgb, f: f32) -> Rgb {
    let s = |v: u8| (f32::from(v) * f).round().min(255.0) as u8;
    (s(c.0), s(c.1), s(c.2))
}

/// The face's pixels, scanlines on pixel rows of `phase`, padded for the slips.
fn grid(face: &Face, phase: usize) -> Grid {
    face.rows
        .iter()
        .enumerate()
        .map(|(j, row)| {
            let mut line = vec![None; PAD];
            for ch in row.chars().take(16) {
                let c = match ch {
                    'd' => Some(scale(CYAN_RGB, 0.22)),
                    'm' => Some(scale(CYAN_RGB, 0.45)),
                    'l' => Some(scale(CYAN_RGB, 0.75)),
                    'h' => Some(CYAN_RGB),
                    'b' => Some(scale(CYAN_RGB, 0.3)),
                    'e' => Some((232, 255, 255)),
                    'X' => Some(face.eye),
                    _ => None,
                };
                let dark = j % 2 == phase && !matches!(ch, 'X' | 'e');
                line.push(c.map(|c| if dark { scale(c, 0.6) } else { c }));
            }
            line.resize(usize::from(FACE_W), None);
            line
        })
        .collect()
}

/// Shifts the rows sideways; pixels pushed past the edge are dropped.
fn slip(g: &mut Grid, rows: &[usize], dx: i32) {
    for &j in rows {
        let r = &g[j];
        let w = r.len();
        let mut n = vec![None; w];
        for (i, c) in r.iter().enumerate() {
            let t = i as i32 + dx;
            if (0..w as i32).contains(&t) {
                n[t as usize] = *c;
            }
        }
        g[j] = n;
    }
}

/// Chromatic split: a magenta ghost a pixel left of the face, a cyan one right.
fn fringe(g: &mut Grid, rows: &[usize]) {
    for &j in rows {
        let r = g[j].clone();
        for (i, c) in r.iter().enumerate() {
            if c.is_none() {
                continue;
            }
            if i > 0 && r[i - 1].is_none() {
                g[j][i - 1] = Some(scale(MAGENTA_RGB, 0.8));
            }
            if i + 1 < r.len() && r[i + 1].is_none() {
                g[j][i + 1] = Some((127, 246, 255));
            }
        }
    }
}

/// A band keeps only some of its colour: `mul` is the (r, g, b) factors.
fn cut(g: &mut Grid, rows: &[usize], mul: (f32, f32, f32)) {
    let ch = |v: u8, m: f32| (f32::from(v) * m).min(255.0) as u8;
    for &j in rows {
        for c in g[j].iter_mut().flatten() {
            *c = (ch(c.0, mul.0), ch(c.1, mul.1), ch(c.2, mul.2));
        }
    }
}

fn drop_out(g: &mut Grid, rows: &[usize]) {
    for &j in rows {
        for c in g[j].iter_mut().flatten() {
            *c = scale(*c, 0.25);
        }
    }
}

/// Rows written for an 18-row face, as rows of this grid.
fn rowset(g: &Grid, rs: impl IntoIterator<Item = usize>) -> Vec<usize> {
    let h = g.len();
    rs.into_iter()
        .map(|r| (r * h / DESIGN_ROWS).min(h.saturating_sub(1)))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Rows `a..=b` of an 18-row face, as rows of this grid.
fn band(g: &Grid, a: usize, b: usize) -> Vec<usize> {
    rowset(g, a..=b)
}

fn apply(g: &mut Grid, recipe: u8) {
    if g.is_empty() {
        return;
    }
    match recipe {
        0 => {
            let r = band(g, 6, 9);
            slip(g, &r, 2);
            fringe(g, &r);
        }
        1 => {
            let (a, b, c) = (band(g, 2, 4), band(g, 12, 14), band(g, 10, 11));
            slip(g, &a, -1);
            slip(g, &b, 1);
            cut(g, &c, (3.0, 0.2, 1.0));
        }
        2 => {
            let (all, d) = (band(g, 0, 17), band(g, 5, 7));
            slip(g, &all, 1);
            fringe(g, &all);
            drop_out(g, &d);
        }
        3 => {
            let (a, b) = (band(g, 8, 10), band(g, 14, 17));
            slip(g, &a, -2);
            cut(g, &b, (0.2, 1.0, 0.4));
            fringe(g, &a);
        }
        _ => {
            let (right, left, c) = (
                rowset(g, [3, 4, 9, 10, 15]),
                rowset(g, [6, 7, 12]),
                band(g, 6, 7),
            );
            slip(g, &right, 1);
            slip(g, &left, -1);
            cut(g, &c, (2.4, 0.3, 1.2));
        }
    }
}

/// Draws the face with its top-left at (x, y), on the panel colour.
#[allow(dead_code)] // the HOLO feed and call draw it (Tasks 6, 7)
pub fn draw(buf: &mut Buffer, x: u16, y: u16, face: &Face, tick: u64, seed: u64) {
    let area = buf.area;
    let mut pen = Pen {
        buf,
        ox: 0,
        oy: 0,
        clip: area,
    };
    let rs = rows(face);
    for cy in 0..rs {
        pen.put(
            x,
            y + cy,
            &" ".repeat(usize::from(FACE_W)),
            CYAN,
            Some(PANEL_BG),
            false,
        );
    }
    pixels(face, tick, seed).fold(&mut pen, i32::from(x), i32::from(y));
}

/// Spec 7's frame around `area`: half-block brackets at the corners, a faint rule
/// between, on the panel colour.
#[allow(dead_code)] // the HOLO feed and call draw it (Tasks 6, 7)
pub fn bracket_frame(buf: &mut Buffer, area: Rect) {
    if area.width < 6 || area.height < 4 {
        return;
    }
    let clip = buf.area;
    let mut pen = Pen {
        buf,
        ox: 0,
        oy: 0,
        clip,
    };
    let faint = {
        let (r, g, b) = scale(MAGENTA_RGB, 0.35);
        Color::Rgb(r, g, b)
    };
    let bg = Some(PANEL_BG);
    let (x, y, w, h) = (area.x, area.y, area.width, area.height);
    for dy in 0..h {
        pen.put(x, y + dy, &" ".repeat(usize::from(w)), faint, bg, false);
    }
    for i in 0..w {
        pen.put(x + i, y, "▀", faint, bg, false);
        pen.put(x + i, y + h - 1, "▄", faint, bg, false);
    }
    for j in 0..h {
        pen.put(x, y + j, "▌", faint, bg, false);
        pen.put(x + w - 1, y + j, "▐", faint, bg, false);
    }
    pen.put(x, y, "▛▀▀", MAGENTA, bg, false);
    pen.put(x + w - 3, y, "▀▀▜", MAGENTA, bg, false);
    pen.put(x, y + h - 1, "▙▄▄", MAGENTA, bg, false);
    pen.put(x + w - 3, y + h - 1, "▄▄▟", MAGENTA, bg, false);
    for (cx, ch) in [(x, "▌"), (x + w - 1, "▐")] {
        pen.put(cx, y + 1, ch, MAGENTA, bg, false);
        pen.put(cx, y + h - 2, ch, MAGENTA, bg, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face() -> Face {
        let mut rows = vec!["................".to_string()];
        for _ in 0..16 {
            rows.push(".dmlhbeXdmlhbeX..".get(..16).unwrap().to_string());
        }
        rows.push("................".to_string());
        Face {
            rows,
            eye: (255, 0, 200),
        }
    }

    fn solid() -> Face {
        Face {
            rows: vec!["hhhhhhhhhhhhhhhh".to_string(); 18],
            eye: (255, 0, 200),
        }
    }

    fn all(p: &Pixels) -> Vec<Option<Color>> {
        let (w, r) = p.cells();
        (0..i32::from(r) * 2)
            .flat_map(|y| (0..i32::from(w)).map(move |x| (x, y)))
            .map(|(x, y)| p.get(x, y))
            .collect()
    }

    fn calm_seed() -> (u64, u64) {
        // a (seed, tick) that is calm at ticks 0 and 4
        for seed in 0..100 {
            if burst(0, seed).is_none() && burst(4, seed).is_none() {
                return (seed, 0);
            }
        }
        panic!("no calm seed");
    }

    #[test]
    fn pixels_are_pure() {
        let f = face();
        for tick in [0, 7, 33, 61] {
            assert_eq!(all(&pixels(&f, tick, 5)), all(&pixels(&f, tick, 5)));
        }
        assert_eq!(pixels(&f, 0, 1).cells(), (FACE_W, 9));
    }

    #[test]
    fn the_scanlines_crawl() {
        let f = face();
        let (seed, _) = calm_seed();
        assert_ne!(
            all(&pixels(&f, 0, seed)),
            all(&pixels(&f, 4, seed)),
            "the phase flips every four ticks"
        );
        // dark rows are 60 % of the ramp: with phase 0 the even pixel rows dim.
        let p = render(&solid(), 0, None);
        let (even, odd) = (p.get(2, 4), p.get(2, 5));
        assert_eq!(odd, Some(Color::Rgb(0, 229, 255)));
        assert_eq!(even, Some(Color::Rgb(0, 137, 153)));
    }

    #[test]
    fn every_cycle_has_one_burst_of_two_to_five_frames() {
        for seed in [0u64, 1, 99] {
            for cycle in 0..21u64 {
                let on: Vec<u64> = (0..60)
                    .filter(|t| burst(cycle * 60 + t, seed).is_some())
                    .collect();
                assert!((4..=15).contains(&on.len()), "cycle {cycle}: {on:?}");
                assert_eq!(
                    on.last().unwrap() - on[0] + 1,
                    on.len() as u64,
                    "contiguous"
                );
                assert!(*on.last().unwrap() < 45);
            }
        }
    }

    #[test]
    fn a_burst_frame_differs_from_calm() {
        let f = face();
        for recipe in 0..5 {
            let phase = 1;
            assert_ne!(
                all(&render(&f, phase, Some(recipe))),
                all(&render(&f, phase, None)),
                "recipe {recipe}"
            );
        }
    }

    #[test]
    fn slips_stay_on_the_canvas() {
        for recipe in 0..5 {
            let p = render(&solid(), 0, Some(recipe));
            assert_eq!(p.cells(), (FACE_W, 9));
            // nothing is written beyond the 20 columns
            assert_eq!(p.get(-1, 3), None);
            assert_eq!(p.get(20, 3), None);
        }
        // a whole-face slip of one pixel loses the pixel pushed past the edge
        let mut g = grid(&solid(), 0);
        slip(&mut g, &[0], 2);
        assert_eq!(g[0].iter().flatten().count(), 16);
        assert!(g[0][0].is_none() && g[0][1].is_none());
    }

    #[test]
    fn the_frame_has_its_corners() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 6));
        bracket_frame(&mut buf, Rect::new(0, 0, 30, 6));
        assert_eq!(buf[(0, 0)].symbol(), "▛");
        assert_eq!(buf[(0, 0)].fg, MAGENTA);
        assert_eq!(buf[(29, 5)].symbol(), "▟");
        assert_eq!(buf[(29, 5)].fg, MAGENTA);
        assert_eq!(buf[(29, 0)].symbol(), "▜");
        assert_eq!(buf[(0, 5)].symbol(), "▙");
        assert_eq!(buf[(5, 0)].symbol(), "▀");
        assert_eq!(buf[(0, 1)].symbol(), "▌");
        assert_eq!(buf[(0, 1)].fg, MAGENTA);
        assert_eq!(buf[(0, 3)].symbol(), "▌");
        assert_ne!(buf[(0, 3)].fg, MAGENTA);
        assert_eq!(buf[(5, 5)].symbol(), "▄");
    }
}
