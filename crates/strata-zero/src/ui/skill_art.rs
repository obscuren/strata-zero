//! A skill's card art drawn: hardware in its own colours, greyed until learned, or a neon
//! sign lit in the skill's attribute colour, cold until learned. The rules are the art tool's
//! (docs/superpowers/specs/2026-10-23-skill-cards-art/skillart.py), so the game draws the
//! pixels the tool drew.

use crate::screens::create::attribute_rgb;
use crate::ui::chrome::PANEL_BG;
use crate::ui::cyberdeck::Pen;
use crate::ui::pixels::Pixels;
use game::{Attribute, SKILL_ART_H, SKILL_ART_W, SkillArt};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// The art's width and height in cells.
pub const W: u16 = SKILL_ART_W as u16;
pub const ROWS: u16 = (SKILL_ART_H / 2) as u16;

type Rgb = (u8, u8, u8);

const PLATE: Rgb = (0x10, 0x12, 0x17);
const PLATE_HI: Rgb = (0x1b, 0x1e, 0x25);
const SCREW: Rgb = (0x4a, 0x4f, 0x58);
const COLD: Rgb = (0x36, 0x3b, 0x44);

/// Draws the first `rows` rows of a skill's art with its top-left at (x, y), nothing
/// outside `clip`. The art's whole block is cleared to the panel first, so its holes and
/// the rows still to scan in show the panel and never what lies under the deck.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    clip: Rect,
    art: &SkillArt,
    attribute: Attribute,
    learned: bool,
    rows: u16,
) {
    let rows = rows.min(ROWS);
    let block = Rect::new(x, y, W, ROWS).intersection(clip);
    for cy in block.y..block.bottom() {
        for cx in block.x..block.right() {
            buf[(cx, cy)].reset();
            buf[(cx, cy)].set_bg(PANEL_BG);
        }
    }
    let area = Rect::new(x, y, W, rows).intersection(clip);
    let mut canvas = Pixels::new(W, rows);
    for (py, row) in (0..).zip(pixels(art, attribute, learned)) {
        for (px, c) in (0..).zip(row) {
            canvas.px(px, py, c.map(colour));
        }
    }
    let mut pen = Pen {
        buf,
        ox: x,
        oy: y,
        clip: area,
    };
    canvas.fold(&mut pen, 0, 0);
    if let SkillArt::Hardware { labels, .. } = art {
        let lit = |c: Rgb| colour(if learned { c } else { grey(c) });
        for (lx, ly, text, ink, ground) in labels.iter().filter(|l| l.1 < rows) {
            pen.put(*lx, *ly, text, lit(*ink), Some(lit(*ground)), false);
        }
    }
}

/// Every pixel of the art, row by row: hardware in its colours (greyed unlearned), or the
/// sign on its plate. Lit, every plate pixel beside a tube glows with that tube's colour,
/// the main tubes' halos first and the accent's over them; cold, no halo and grey glass.
fn pixels(art: &SkillArt, attribute: Attribute, learned: bool) -> Vec<Vec<Option<Rgb>>> {
    match art {
        SkillArt::Hardware { rows, colours, .. } => rows
            .iter()
            .map(|r| {
                r.chars()
                    .map(|ch| {
                        let c = colours.get(&ch).filter(|_| ch != '.');
                        c.map(|&c| if learned { c } else { grey(c) })
                    })
                    .collect()
            })
            .collect(),
        SkillArt::Neon { rows, accent } => {
            let tube = |x: usize, y: usize| {
                rows.get(y)
                    .and_then(|r| r.chars().nth(x))
                    .filter(|&c| c != '.')
            };
            let (w, h) = (SKILL_ART_W, SKILL_ART_H);
            let mut out = vec![vec![Some(PLATE); w]; h];
            out[0] = vec![Some(PLATE_HI); w];
            for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
                out[y][x] = Some(SCREW);
            }
            let main = attribute_rgb(attribute);
            let main = (main.0, main.1, main.2);
            if learned {
                for (ch, c) in [('#', main), ('+', *accent)] {
                    let halo = mix(PLATE, c, 0.28);
                    for (y, x) in (0..h).flat_map(|y| (0..w).map(move |x| (y, x))) {
                        if tube(x, y) != Some(ch) {
                            continue;
                        }
                        for (dx, dy) in (-1..=1).flat_map(|dx| (-1..=1).map(move |dy| (dx, dy))) {
                            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                            if (0..w as i32).contains(&nx)
                                && (0..h as i32).contains(&ny)
                                && tube(nx as usize, ny as usize).is_none()
                            {
                                out[ny as usize][nx as usize] = Some(halo);
                            }
                        }
                    }
                }
            }
            for (y, row) in out.iter_mut().enumerate() {
                for (x, px) in row.iter_mut().enumerate() {
                    match (tube(x, y), learned) {
                        (Some('#'), true) => *px = Some(main),
                        (Some(_), true) => *px = Some(*accent),
                        (Some(_), false) => *px = Some(COLD),
                        (None, _) => {}
                    }
                }
            }
            out
        }
    }
}

/// `a` toward `b` by `t`, each channel truncated, as the art tool mixes.
fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let m = |a: u8, b: u8| (f64::from(a) + (f64::from(b) - f64::from(a)) * t) as u8;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// A colour unlit: each channel three quarters toward the luma, then darkened to 0.6.
fn grey((r, g, b): Rgb) -> Rgb {
    let l = 0.3 * f64::from(r) + 0.59 * f64::from(g) + 0.11 * f64::from(b);
    let d = |v: u8| ((0.25 * f64::from(v) + 0.75 * l) * 0.6) as u8;
    (d(r), d(g), d(b))
}

fn colour((r, g, b): Rgb) -> Color {
    Color::Rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: W,
        height: ROWS,
    };

    fn blank() -> Vec<String> {
        vec![".".repeat(SKILL_ART_W); SKILL_ART_H]
    }

    fn set(rows: &mut [String], x: usize, y: usize, ch: char) {
        rows[y].replace_range(x..x + 1, &ch.to_string());
    }

    fn drawn(art: &SkillArt, attribute: Attribute, learned: bool, rows: u16, clip: Rect) -> Buffer {
        let mut buf = Buffer::empty(AREA);
        for cell in &mut buf.content {
            cell.set_char('x');
        }
        draw(&mut buf, 0, 0, clip, art, attribute, learned, rows);
        buf
    }

    fn rgb((r, g, b): Rgb) -> Color {
        Color::Rgb(r, g, b)
    }

    fn hardware() -> SkillArt {
        let mut rows = blank();
        set(&mut rows, 0, 0, 'a');
        set(&mut rows, 0, 1, 'a');
        SkillArt::Hardware {
            rows,
            colours: BTreeMap::from([('a', (200, 100, 50))]),
            labels: vec![(2, 1, "OK".into(), (1, 2, 3), (4, 5, 6))],
        }
    }

    #[test]
    fn hardware_draws_its_colours_and_greys_unlearned() {
        let b = drawn(&hardware(), Attribute::Tech, true, ROWS, AREA);
        assert_eq!(
            (b[(0, 0)].symbol(), b[(0, 0)].bg),
            (" ", rgb((200, 100, 50)))
        );
        // An empty cell is the panel, not what was under it.
        assert_eq!((b[(5, 5)].symbol(), b[(5, 5)].bg), (" ", PANEL_BG));
        let b = drawn(&hardware(), Attribute::Tech, false, ROWS, AREA);
        assert_eq!(b[(0, 0)].bg, rgb((86, 71, 63)));
    }

    #[test]
    fn a_label_goes_over_the_cells() {
        let b = drawn(&hardware(), Attribute::Tech, true, ROWS, AREA);
        for (x, ch) in [(2, "O"), (3, "K")] {
            assert_eq!(
                (b[(x, 1)].symbol(), b[(x, 1)].fg, b[(x, 1)].bg),
                (ch, rgb((1, 2, 3)), rgb((4, 5, 6)))
            );
        }
    }

    fn sign() -> SkillArt {
        let mut rows = blank();
        set(&mut rows, 10, 10, '#');
        set(&mut rows, 20, 10, '+');
        SkillArt::Neon {
            rows,
            accent: (255, 0, 0),
        }
    }

    #[test]
    fn neon_lights_in_the_attribute_colour_with_a_halo() {
        let net = (110, 150, 255);
        let b = drawn(&sign(), Attribute::Net, true, ROWS, AREA);
        // Pixel (10, 10) is the top half of cell (10, 5); (11, 10) is beside it.
        assert_eq!(b[(10, 5)].fg, rgb(net));
        assert_eq!(b[(11, 5)].fg, rgb(mix(PLATE, net, 0.28)));
        assert_eq!(b[(21, 5)].fg, rgb(mix(PLATE, (255, 0, 0), 0.28)));
        assert_eq!(b[(0, 0)].fg, rgb(SCREW));
        let b = drawn(&sign(), Attribute::Net, false, ROWS, AREA);
        assert_eq!(b[(10, 5)].fg, rgb(COLD));
        assert_eq!(b[(11, 5)].bg, rgb(PLATE));
    }

    #[test]
    fn only_the_rows_asked_for_draw_and_nothing_past_the_clip() {
        // The rows still to scan in are bare panel, not what lay under them.
        let b = drawn(&sign(), Attribute::Net, true, 3, AREA);
        assert_eq!((b[(0, 3)].symbol(), b[(0, 3)].bg), (" ", PANEL_BG));
        assert_eq!(b[(0, 2)].bg, rgb(PLATE));
        let b = drawn(
            &sign(),
            Attribute::Net,
            true,
            ROWS,
            Rect::new(0, 0, 10, ROWS),
        );
        assert_eq!(b[(10, 0)].symbol(), "x");
        assert_ne!(b[(9, 0)].symbol(), "x");
    }
}
