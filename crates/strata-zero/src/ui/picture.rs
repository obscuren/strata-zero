//! An item drawn from its picture: plain pixel art from the content, folded into half
//! blocks, with its labels over the cells. It has no sockets, no back and takes no
//! stickers: the window only shows it.

use crate::ui::art::{Art, SocketPlace};
use crate::ui::cyberdeck::Pen;
use crate::ui::pixels::Pixels;
use game::Picture;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// An item's picture as the blueprint window and the viewer draw it.
#[derive(Clone, Debug, PartialEq)]
pub struct PictureArt {
    pub picture: Picture,
}

fn rgb((r, g, b): (u8, u8, u8)) -> Color {
    Color::Rgb(r, g, b)
}

impl Art for PictureArt {
    fn size(&self) -> (u16, u16) {
        let p = &self.picture;
        (p.width() as u16, (p.rows.len() / 2) as u16)
    }

    fn draw(&self, (ox, oy): (u16, u16), clip: Rect, buf: &mut Buffer) {
        let (w, rows) = self.size();
        let mut canvas = Pixels::new(w, rows);
        for (y, row) in (0..).zip(&self.picture.rows) {
            for (x, ch) in (0..).zip(row.chars()) {
                let c = self.picture.colours.get(&ch).filter(|_| ch != '.');
                canvas.px(x, y, c.copied().map(rgb));
            }
        }
        let mut pen = Pen {
            clip: clip.intersection(buf.area),
            buf,
            ox,
            oy,
        };
        canvas.fold(&mut pen, 0, 0);
        // The text keeps the cell's ground: the pixels under it.
        for l in &self.picture.labels {
            pen.put(l.x, l.y, &l.text, rgb(l.colour), None, true);
        }
    }

    fn sockets(&self) -> Vec<SocketPlace> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game::PictureLabel;
    use ratatui::style::Modifier;
    use std::collections::BTreeMap;

    const A: (u8, u8, u8) = (200, 10, 10);
    const B: (u8, u8, u8) = (10, 10, 200);
    const INK: (u8, u8, u8) = (1, 2, 3);
    const BACK: Color = Color::Rgb(9, 9, 9);

    fn art() -> PictureArt {
        PictureArt {
            picture: Picture {
                rows: ["aa..", "aa..", "..bb", "..bb"].map(String::from).to_vec(),
                colours: BTreeMap::from([('a', A), ('b', B)]),
                labels: vec![PictureLabel {
                    x: 3,
                    y: 0,
                    text: "X".into(),
                    colour: INK,
                }],
            },
        }
    }

    #[test]
    fn draws_its_pixels_and_labels() {
        let area = Rect::new(0, 0, 6, 3);
        let mut buf = Buffer::empty(area);
        for cell in &mut buf.content {
            cell.set_char('~').set_bg(BACK);
        }
        let art = art();
        assert_eq!(art.size(), (4, 2));
        art.draw((1, 1), area, &mut buf);
        let at = |x: u16, y: u16| &buf[(x, y)];
        assert_eq!((at(1, 1).symbol(), at(1, 1).bg), (" ", rgb(A)));
        assert_eq!((at(3, 2).symbol(), at(3, 2).bg), (" ", rgb(B)));
        // The label over an empty cell: bold, in its colour, on what was there.
        assert_eq!(
            (at(4, 1).symbol(), at(4, 1).fg, at(4, 1).bg),
            ("X", rgb(INK), BACK)
        );
        assert!(at(4, 1).modifier.contains(Modifier::BOLD));
        // Empty pixels write nothing; nothing lands outside the drawing.
        assert_eq!(at(3, 1).symbol(), "~");
        assert_eq!(at(0, 0).symbol(), "~");
        assert_eq!(at(5, 2).symbol(), "~");
    }

    #[test]
    fn draws_nothing_outside_its_clip() {
        let area = Rect::new(0, 0, 6, 3);
        let mut buf = Buffer::empty(area);
        for cell in &mut buf.content {
            cell.set_char('~');
        }
        art().draw((1, 1), Rect::new(1, 1, 2, 1), &mut buf);
        assert_eq!(buf[(1, 1)].symbol(), " ");
        assert_eq!(buf[(4, 1)].symbol(), "~", "the label is clipped too");
        assert_eq!(buf[(3, 2)].symbol(), "~");
    }
}
