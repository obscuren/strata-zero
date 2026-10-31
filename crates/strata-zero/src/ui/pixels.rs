//! A canvas of half-cell pixels: two stacked in every cell, folded into half blocks.

use super::cyberdeck::Pen;
use ratatui::style::Color;

/// Pixels `w` wide and twice `rows` high, each empty or one colour.
pub(crate) struct Pixels {
    w: i32,
    h: i32,
    px: Vec<Option<Color>>,
}

impl Pixels {
    /// A canvas of `w` by `rows` cells with every pixel empty.
    pub fn new(w: u16, rows: u16) -> Self {
        let (w, h) = (i32::from(w), i32::from(rows) * 2);
        Pixels {
            w,
            h,
            px: vec![None; (w * h) as usize],
        }
    }

    /// The canvas's width and rows in cells.
    pub fn cells(&self) -> (u16, u16) {
        (self.w as u16, (self.h / 2) as u16)
    }

    /// Sets one pixel; a pixel off the canvas is dropped.
    pub fn px(&mut self, x: i32, y: i32, c: Option<Color>) {
        if (0..self.w).contains(&x) && (0..self.h).contains(&y) {
            self.px[(y * self.w + x) as usize] = c;
        }
    }

    /// Fills x0..x1 by y0..y1, half open.
    pub fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Color) {
        for y in y0..y1 {
            for x in x0..x1 {
                self.px(x, y, Some(c));
            }
        }
    }

    /// A row of pixels, x0..x1 at row `y`.
    pub fn hline(&mut self, x0: i32, x1: i32, y: i32, c: Color) {
        self.rect(x0, y, x1, y + 1, c);
    }

    /// A column of pixels, y0..y1 at column `x`.
    pub fn vline(&mut self, x: i32, y0: i32, y1: i32, c: Color) {
        self.rect(x, y0, x + 1, y1, c);
    }

    /// The pixel at (x, y); off the canvas is empty.
    #[cfg(test)]
    pub fn get(&self, x: i32, y: i32) -> Option<Color> {
        if (0..self.w).contains(&x) && (0..self.h).contains(&y) {
            self.at(x, y)
        } else {
            None
        }
    }

    fn at(&self, x: i32, y: i32) -> Option<Color> {
        self.px[(y * self.w + x) as usize]
    }

    /// Writes the cells with the canvas's top-left at pen cell (x, y). Equal halves
    /// make a blank cell of that colour; one empty half leaves a half block whose
    /// other side is whatever is already there; a cell of two empty halves is not
    /// written at all.
    pub fn fold(&self, p: &mut Pen, x: i32, y: i32) {
        for cy in 0..self.h / 2 {
            for cx in 0..self.w {
                let (top, bottom) = (self.at(cx, 2 * cy), self.at(cx, 2 * cy + 1));
                let (ch, fg, bg) = match (top, bottom) {
                    (None, None) => continue,
                    (Some(t), Some(b)) if t == b => (' ', t, Some(t)),
                    (Some(t), Some(b)) => ('▀', t, Some(b)),
                    (None, Some(b)) => ('▄', b, None),
                    (Some(t), None) => ('▀', t, None),
                };
                p.cell(x + cx, y + cy, ch, fg, bg);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    const RED: Color = Color::Rgb(0xff, 0, 0);
    const BLUE: Color = Color::Rgb(0, 0, 0xff);
    const PANEL: Color = Color::Rgb(1, 2, 3);

    fn folded(c: &Pixels) -> Buffer {
        let area = Rect::new(0, 0, 4, 2);
        let mut buf = Buffer::empty(area);
        for cell in &mut buf.content {
            cell.set_bg(PANEL);
        }
        let mut pen = Pen {
            buf: &mut buf,
            ox: 0,
            oy: 0,
            clip: area,
        };
        c.fold(&mut pen, 0, 0);
        buf
    }

    #[test]
    fn folding_follows_the_reference_rule() {
        let mut c = Pixels::new(2, 1);
        c.px(0, 0, Some(RED));
        c.px(0, 1, Some(RED));
        c.px(1, 0, Some(RED));
        let b = folded(&c);
        assert_eq!(
            (b[(0, 0)].symbol(), b[(0, 0)].fg, b[(0, 0)].bg),
            (" ", RED, RED)
        );
        assert_eq!(
            (b[(1, 0)].symbol(), b[(1, 0)].fg, b[(1, 0)].bg),
            ("▀", RED, PANEL)
        );

        let mut c = Pixels::new(2, 1);
        c.px(0, 1, Some(BLUE));
        c.px(1, 0, Some(RED));
        c.px(1, 1, Some(BLUE));
        let b = folded(&c);
        assert_eq!(
            (b[(0, 0)].symbol(), b[(0, 0)].fg, b[(0, 0)].bg),
            ("▄", BLUE, PANEL)
        );
        assert_eq!(
            (b[(1, 0)].symbol(), b[(1, 0)].fg, b[(1, 0)].bg),
            ("▀", RED, BLUE)
        );
    }

    #[test]
    fn an_empty_cell_is_not_written() {
        let mut c = Pixels::new(2, 1);
        c.rect(0, 0, 1, 2, RED);
        let b = folded(&c);
        assert_eq!(b[(1, 0)].symbol(), " ");
        assert_eq!(b[(1, 0)].bg, PANEL);
        assert_eq!(b[(1, 0)].fg, Color::Reset);
    }
}
