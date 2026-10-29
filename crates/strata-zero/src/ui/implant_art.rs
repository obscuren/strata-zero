//! A piece of chrome's card art drawn: its pixels in their own colours on the deck's panel,
//! as the skill cards' hardware art draws them, without the learned and neon rules.

use crate::ui::chrome::PANEL_BG;
use crate::ui::cyberdeck::Pen;
use crate::ui::pixels::Pixels;
use game::{IMPLANT_ART_H, IMPLANT_ART_W, ImplantArt};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// The art's width and height in cells.
pub const W: u16 = IMPLANT_ART_W as u16;
pub const ROWS: u16 = (IMPLANT_ART_H / 2) as u16;

/// Draws the first `rows` rows of a piece's art with its top-left at (x, y), nothing
/// outside `clip`. The art's whole block is cleared to the panel first, so its holes and
/// the rows still to scan in show the panel and never what lies under the deck.
pub fn draw(buf: &mut Buffer, x: u16, y: u16, clip: Rect, art: &ImplantArt, rows: u16) {
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
    for (py, row) in (0..).zip(art.rows.iter().take(usize::from(rows) * 2)) {
        for (px, ch) in (0..).zip(row.chars()) {
            let c = art.colours.get(&ch).filter(|_| ch != '.');
            canvas.px(px, py, c.map(|&(r, g, b)| Color::Rgb(r, g, b)));
        }
    }
    let mut pen = Pen {
        buf,
        ox: x,
        oy: y,
        clip: area,
    };
    canvas.fold(&mut pen, 0, 0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn draws_inside_its_clip() {
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        for cell in &mut buf.content {
            cell.set_char('x');
        }
        let mut rows = vec![".".repeat(IMPLANT_ART_W); IMPLANT_ART_H];
        rows[0] = "a".repeat(IMPLANT_ART_W);
        let art = ImplantArt {
            rows,
            colours: BTreeMap::from([('a', (200, 100, 50))]),
        };
        let clip = Rect::new(0, 0, 20, 4);
        draw(&mut buf, 2, 1, clip, &art, ROWS);
        for y in 0..10 {
            for x in 0..40 {
                let inside = clip.contains((x, y).into());
                let touched = buf[(x, y)].symbol() != "x";
                assert_eq!(touched, inside && x >= 2 && y >= 1, "({x}, {y})");
            }
        }
        assert_eq!(buf[(2, 1)].symbol(), "\u{2580}");
        assert_eq!(buf[(2, 1)].fg, Color::Rgb(200, 100, 50));
        assert_eq!(buf[(2, 2)].bg, PANEL_BG);
    }
}
