//! Box-drawing (U+2500–U+257F) and block-element (U+2580–U+259F) characters drawn as
//! shapes rather than taken from the font, so lines meet their neighbours with no seams
//! and half-block art fills its cells exactly, at any cell size.

/// A `w*h` coverage mask (row-major, 0–255) for a box or block character, `None` for
/// anything else.
pub fn shape(c: char, w: usize, h: usize) -> Option<Vec<u8>> {
    let mut m = Mask::new(w, h);
    match c as u32 {
        0x2500..=0x2570 | 0x2574..=0x257F => lines(&mut m, c as u32),
        0x2571..=0x2573 => diagonals(&mut m, c as u32),
        0x2580..=0x259F => blocks(&mut m, c as u32),
        _ => return None,
    }
    Some(m.px)
}

struct Mask {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Mask {
    fn new(w: usize, h: usize) -> Self {
        Mask {
            w,
            h,
            px: vec![0; w * h],
        }
    }

    /// Fills `[x0, x1) × [y0, y1)` with `a`, clipped to the cell.
    fn fill(&mut self, x0: isize, y0: isize, x1: isize, y1: isize, a: u8) {
        let (x0, x1) = (x0.max(0) as usize, (x1.max(0) as usize).min(self.w));
        let (y0, y1) = (y0.max(0) as usize, (y1.max(0) as usize).min(self.h));
        for y in y0..y1 {
            for x in x0..x1 {
                let p = &mut self.px[y * self.w + x];
                *p = (*p).max(a);
            }
        }
    }
}

/// An arm's weight: none, light, heavy, double.
const N: u8 = 0;
const L: u8 = 1;
const H: u8 = 2;
const D: u8 = 3;

/// Arms (left, right, up, down) for U+2500–U+257F; diagonals are drawn elsewhere and
/// hold `[N; 4]` here. Dashed lines carry the arms of the line they dash.
#[rustfmt::skip]
const ARMS: [[u8; 4]; 128] = [
    [L, L, N, N], [H, H, N, N], [N, N, L, L], [N, N, H, H], // ─ ━ │ ┃
    [L, L, N, N], [H, H, N, N], [N, N, L, L], [N, N, H, H], // ┄ ┅ ┆ ┇
    [L, L, N, N], [H, H, N, N], [N, N, L, L], [N, N, H, H], // ┈ ┉ ┊ ┋
    [N, L, N, L], [N, H, N, L], [N, L, N, H], [N, H, N, H], // ┌ ┍ ┎ ┏
    [L, N, N, L], [H, N, N, L], [L, N, N, H], [H, N, N, H], // ┐ ┑ ┒ ┓
    [N, L, L, N], [N, H, L, N], [N, L, H, N], [N, H, H, N], // └ ┕ ┖ ┗
    [L, N, L, N], [H, N, L, N], [L, N, H, N], [H, N, H, N], // ┘ ┙ ┚ ┛
    [N, L, L, L], [N, H, L, L], [N, L, H, L], [N, L, L, H], // ├ ┝ ┞ ┟
    [N, L, H, H], [N, H, H, L], [N, H, L, H], [N, H, H, H], // ┠ ┡ ┢ ┣
    [L, N, L, L], [H, N, L, L], [L, N, H, L], [L, N, L, H], // ┤ ┥ ┦ ┧
    [L, N, H, H], [H, N, H, L], [H, N, L, H], [H, N, H, H], // ┨ ┩ ┪ ┫
    [L, L, N, L], [H, L, N, L], [L, H, N, L], [H, H, N, L], // ┬ ┭ ┮ ┯
    [L, L, N, H], [H, L, N, H], [L, H, N, H], [H, H, N, H], // ┰ ┱ ┲ ┳
    [L, L, L, N], [H, L, L, N], [L, H, L, N], [H, H, L, N], // ┴ ┵ ┶ ┷
    [L, L, H, N], [H, L, H, N], [L, H, H, N], [H, H, H, N], // ┸ ┹ ┺ ┻
    [L, L, L, L], [H, L, L, L], [L, H, L, L], [H, H, L, L], // ┼ ┽ ┾ ┿
    [L, L, H, L], [L, L, L, H], [L, L, H, H], [H, L, H, L], // ╀ ╁ ╂ ╃
    [L, H, H, L], [H, L, L, H], [L, H, L, H], [H, H, H, L], // ╄ ╅ ╆ ╇
    [H, H, L, H], [H, L, H, H], [L, H, H, H], [H, H, H, H], // ╈ ╉ ╊ ╋
    [L, L, N, N], [H, H, N, N], [N, N, L, L], [N, N, H, H], // ╌ ╍ ╎ ╏
    [D, D, N, N], [N, N, D, D], [N, D, N, L], [N, L, N, D], // ═ ║ ╒ ╓
    [N, D, N, D], [D, N, N, L], [L, N, N, D], [D, N, N, D], // ╔ ╕ ╖ ╗
    [N, D, L, N], [N, L, D, N], [N, D, D, N], [D, N, L, N], // ╘ ╙ ╚ ╛
    [L, N, D, N], [D, N, D, N], [N, D, L, L], [N, L, D, D], // ╜ ╝ ╞ ╟
    [N, D, D, D], [D, N, L, L], [L, N, D, D], [D, N, D, D], // ╠ ╡ ╢ ╣
    [D, D, N, L], [L, L, N, D], [D, D, N, D], [D, D, L, N], // ╤ ╥ ╦ ╧
    [L, L, D, N], [D, D, D, N], [D, D, L, L], [L, L, D, D], // ╨ ╩ ╪ ╫
    [D, D, D, D], [N, L, N, L], [L, N, N, L], [L, N, L, N], // ╬ ╭ ╮ ╯
    [N, L, L, N], [N, N, N, N], [N, N, N, N], [N, N, N, N], // ╰ ╱ ╲ ╳
    [L, N, N, N], [N, N, L, N], [N, L, N, N], [N, N, N, L], // ╴ ╵ ╶ ╷
    [H, N, N, N], [N, N, H, N], [N, H, N, N], [N, N, N, H], // ╸ ╹ ╺ ╻
    [L, H, N, N], [N, N, L, H], [H, L, N, N], [N, N, H, L], // ╼ ╽ ╾ ╿
];

/// A light stroke's thickness in pixels for a cell `h` tall.
fn light(h: usize) -> isize {
    ((h as f32 / 16.0).round() as isize).max(1)
}

/// `[start, end)` of a stroke `t` thick centred on `c`.
fn span(c: isize, t: isize) -> (isize, isize) {
    let s = c - t / 2;
    (s, s + t)
}

fn lines(m: &mut Mask, cp: u32) {
    let [l, r, u, d] = ARMS[(cp - 0x2500) as usize];
    let t = light(m.h);
    let thick = |a: u8| if a == H { 2 * t } else { t };
    let (cx, cy) = ((m.w / 2) as isize, (m.h / 2) as isize);
    let (w, h) = (m.w as isize, m.h as isize);
    // The widest stroke crossing an axis, so a lighter arm reaches its far side.
    let across_v = thick(u.max(d).min(H)).max(if u == N && d == N { 0 } else { t });
    let across_h = thick(l.max(r).min(H)).max(if l == N && r == N { 0 } else { t });
    let v_double = u == D || d == D;
    let h_double = l == D || r == D;

    // Horizontal arms: each runs from the cell's edge to where it meets the vertical.
    for (weight, left_side) in [(l, true), (r, false)] {
        if weight == N {
            continue;
        }
        let other = if left_side { r } else { l };
        if weight == D {
            for (off, toward) in [(-t, u), (t, d)] {
                let meet = if v_double {
                    // Outer stroke runs to the far vertical; inner stops at the near one.
                    let near = if toward != N { -t } else { t };
                    let x = cx + if left_side { near } else { -near };
                    if left_side {
                        span(x, t).1
                    } else {
                        span(x, t).0
                    }
                } else if u != N || d != N {
                    if left_side {
                        span(cx, across_v).1
                    } else {
                        span(cx, across_v).0
                    }
                } else {
                    cx
                };
                let (y0, y1) = span(cy + off, t);
                if left_side {
                    m.fill(0, y0, meet, y1, 255)
                } else {
                    m.fill(meet, y0, w, y1, 255)
                }
            }
        } else {
            let meet = if v_double && other == N {
                let far = if u != N && d != N { t } else { -t };
                let x = cx + if left_side { -far } else { far };
                if left_side {
                    span(x, t).1
                } else {
                    span(x, t).0
                }
            } else if left_side {
                span(cx, across_v.max(thick(weight))).1
            } else {
                span(cx, across_v.max(thick(weight))).0
            };
            let (y0, y1) = span(cy, thick(weight));
            if left_side {
                m.fill(0, y0, meet, y1, 255)
            } else {
                m.fill(meet, y0, w, y1, 255)
            }
        }
    }

    // Vertical arms, the same with the axes swapped.
    for (weight, top_side) in [(u, true), (d, false)] {
        if weight == N {
            continue;
        }
        let other = if top_side { d } else { u };
        if weight == D {
            for (off, toward) in [(-t, l), (t, r)] {
                let meet = if h_double {
                    let near = if toward != N { -t } else { t };
                    let y = cy + if top_side { near } else { -near };
                    if top_side { span(y, t).1 } else { span(y, t).0 }
                } else if l != N || r != N {
                    if top_side {
                        span(cy, across_h).1
                    } else {
                        span(cy, across_h).0
                    }
                } else {
                    cy
                };
                let (x0, x1) = span(cx + off, t);
                if top_side {
                    m.fill(x0, 0, x1, meet, 255)
                } else {
                    m.fill(x0, meet, x1, h, 255)
                }
            }
        } else {
            let meet = if h_double && other == N {
                let far = if l != N && r != N { t } else { -t };
                let y = cy + if top_side { -far } else { far };
                if top_side { span(y, t).1 } else { span(y, t).0 }
            } else if top_side {
                span(cy, across_h.max(thick(weight))).1
            } else {
                span(cy, across_h.max(thick(weight))).0
            };
            let (x0, x1) = span(cx, thick(weight));
            if top_side {
                m.fill(x0, 0, x1, meet, 255)
            } else {
                m.fill(x0, meet, x1, h, 255)
            }
        }
    }

    let dashes = match cp {
        0x2504..=0x2507 => 3,
        0x2508..=0x250B => 4,
        0x254C..=0x254F => 2,
        _ => 0,
    };
    if dashes > 0 {
        let horizontal = l != N;
        let len = if horizontal { m.w } else { m.h };
        let gap = (len / (dashes * 4)).max(1);
        for i in 0..dashes {
            // Clear the gap at the end of each dash's share of the cell.
            let end = (i + 1) * len / dashes;
            let start = end.saturating_sub(gap);
            if horizontal {
                clear(m, start, 0, end, m.h);
            } else {
                clear(m, 0, start, m.w, end);
            }
        }
    }
}

fn clear(m: &mut Mask, x0: usize, y0: usize, x1: usize, y1: usize) {
    for y in y0..y1.min(m.h) {
        for x in x0..x1.min(m.w) {
            m.px[y * m.w + x] = 0;
        }
    }
}

fn diagonals(m: &mut Mask, cp: u32) {
    let t = light(m.h) as f32;
    let (w, h) = (m.w as f32, m.h as f32);
    let len = (w * w + h * h).sqrt();
    for y in 0..m.h {
        for x in 0..m.w {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // Distance to the line from top-left to bottom-right, and to its mirror.
            let down = (h * px - w * py).abs() / len;
            let up = (h * px + w * py - w * h).abs() / len;
            let dist = match cp {
                0x2571 => up,
                0x2572 => down,
                _ => up.min(down),
            };
            let a = (t / 2.0 + 0.5 - dist).clamp(0.0, 1.0);
            m.px[y * m.w + x] = (a * 255.0) as u8;
        }
    }
}

fn blocks(m: &mut Mask, cp: u32) {
    let (w, h) = (m.w as isize, m.h as isize);
    let rows = |eighths: isize| (h * eighths + 4) / 8;
    let cols = |eighths: isize| (w * eighths + 4) / 8;
    let (hw, hh) = (w / 2, h / 2);
    let quads = |m: &mut Mask, ul: bool, ur: bool, ll: bool, lr: bool| {
        if ul {
            m.fill(0, 0, hw, hh, 255);
        }
        if ur {
            m.fill(hw, 0, w, hh, 255);
        }
        if ll {
            m.fill(0, hh, hw, h, 255);
        }
        if lr {
            m.fill(hw, hh, w, h, 255);
        }
    };
    match cp {
        0x2580 => m.fill(0, 0, w, hh, 255),
        0x2581..=0x2588 => {
            let n = (cp - 0x2580) as isize;
            m.fill(0, h - rows(n), w, h, 255);
        }
        0x2589..=0x258F => {
            let n = (0x2590 - cp) as isize;
            m.fill(0, 0, cols(n), h, 255);
        }
        0x2590 => m.fill(hw, 0, w, h, 255),
        0x2591 => m.fill(0, 0, w, h, 64),
        0x2592 => m.fill(0, 0, w, h, 128),
        0x2593 => m.fill(0, 0, w, h, 192),
        0x2594 => m.fill(0, 0, w, rows(1), 255),
        0x2595 => m.fill(w - cols(1), 0, w, h, 255),
        0x2596 => quads(m, false, false, true, false),
        0x2597 => quads(m, false, false, false, true),
        0x2598 => quads(m, true, false, false, false),
        0x2599 => quads(m, true, false, true, true),
        0x259A => quads(m, true, false, false, true),
        0x259B => quads(m, true, true, true, false),
        0x259C => quads(m, true, true, false, true),
        0x259D => quads(m, false, true, false, false),
        0x259E => quads(m, false, true, true, false),
        _ => quads(m, false, true, true, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(m: &[u8], x: usize, y: usize) -> u8 {
        m[y * 10 + x]
    }

    #[test]
    fn every_box_and_block_char_has_a_shape() {
        for c in '\u{2500}'..='\u{259F}' {
            let m = shape(c, 10, 20).unwrap_or_else(|| panic!("{c} has no shape"));
            assert_eq!(m.len(), 200, "{c}");
            assert!(m.iter().any(|&a| a > 0), "{c} is empty");
        }
    }

    #[test]
    fn horizontal_line_spans_the_cell() {
        let m = shape('─', 10, 20).unwrap();
        assert_eq!(at(&m, 0, 10), 255);
        assert_eq!(at(&m, 9, 10), 255);
        assert!((0..10).all(|x| at(&m, x, 0) == 0));
    }

    #[test]
    fn lines_meet_at_the_centre() {
        let m = shape('┼', 10, 20).unwrap();
        assert!((0..10).all(|x| at(&m, x, 10) == 255));
        assert!((0..20).all(|y| at(&m, 5, y) == 255));
    }

    #[test]
    fn corners_close() {
        // The corner pixel of every light corner is inked, so a box has no notch.
        for c in ['┌', '┐', '└', '┘', '╔', '╗', '╚', '╝', '┏', '┛'] {
            let m = shape(c, 10, 20).unwrap();
            assert_eq!(
                at(&m, 5, 10).max(at(&m, 4, 9)).max(at(&m, 6, 11)),
                255,
                "{c}"
            );
        }
    }

    #[test]
    fn full_and_half_blocks() {
        assert!(shape('█', 10, 20).unwrap().iter().all(|&a| a == 255));
        let top = shape('▀', 10, 20).unwrap();
        assert!((0..10).all(|y| (0..10).all(|x| at(&top, x, y) == 255)));
        assert!((10..20).all(|y| (0..10).all(|x| at(&top, x, y) == 0)));
        let bottom = shape('▄', 10, 20).unwrap();
        assert!((0..10).all(|y| (0..10).all(|x| at(&bottom, x, y) == 0)));
        assert!((10..20).all(|y| (0..10).all(|x| at(&bottom, x, y) == 255)));
        let left = shape('▌', 10, 20).unwrap();
        assert!((0..20).all(|y| at(&left, 4, y) == 255 && at(&left, 5, y) == 0));
    }

    #[test]
    fn shades_are_partial() {
        for (c, a) in [('░', 64), ('▒', 128), ('▓', 192)] {
            assert!(shape(c, 10, 20).unwrap().iter().all(|&p| p == a), "{c}");
        }
    }

    #[test]
    fn not_a_shape() {
        assert_eq!(shape('a', 10, 20), None);
        assert_eq!(shape('◆', 10, 20), None);
    }
}
