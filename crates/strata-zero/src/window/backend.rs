//! A ratatui backend that paints pixels: the cells ratatui reports changed go through
//! the `Canvas` into a framebuffer the window presents.

use super::font::Fonts;
use super::raster::{Canvas, Frame, span_of};
use ratatui::backend::{Backend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use std::io;

/// A rectangle of the frame that changed, in pixels: `[x0, x1) × [y0, y1)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Damage {
    pub x0: usize,
    pub y0: usize,
    pub x1: usize,
    pub y1: usize,
}

impl Damage {
    /// The smallest rectangle holding both.
    pub fn union(self, o: Damage) -> Damage {
        Damage {
            x0: self.x0.min(o.x0),
            y0: self.y0.min(o.y0),
            x1: self.x1.max(o.x1),
            y1: self.y1.max(o.y1),
        }
    }

    /// `a` and `b` together, where either may be nothing.
    pub fn join(a: Option<Damage>, b: Option<Damage>) -> Option<Damage> {
        match (a, b) {
            (Some(a), Some(b)) => Some(a.union(b)),
            (a, b) => a.or(b),
        }
    }
}

pub struct PixelBackend {
    canvas: Canvas,
    /// What each cell shows, so a redraw of either half of a wide glyph repaints it whole
    /// and a resize keeps the picture.
    grid: Vec<Cell>,
    cols: u16,
    rows: u16,
    /// What was painted since `take_damage` last asked.
    damage: Option<Damage>,
    /// `clear` was called: the frame is blanked by the draw that follows it, not at once,
    /// so a window redraw in between never shows an empty frame.
    blank_pending: bool,
}

impl PixelBackend {
    /// A backend for a window `width` by `height` pixels, text `px` pixels high.
    pub fn new(fonts: Fonts, px: f32, width: usize, height: usize) -> Self {
        let mut b = PixelBackend {
            canvas: Canvas::new(fonts, px, width, height),
            grid: Vec::new(),
            cols: 0,
            rows: 0,
            damage: None,
            blank_pending: false,
        };
        b.regrid();
        b
    }

    /// A new pixel size. The grid keeps what fits and is repainted at once. Cells that
    /// fell off are gone, so the caller clears the terminal before its next draw: ratatui
    /// only resends everything when it sees the size change itself, which a resize there
    /// and back between two frames hides from it.
    pub fn resize(&mut self, width: usize, height: usize) {
        self.canvas.resize(width, height);
        self.regrid();
    }

    /// A new text size, as `resize`.
    pub fn set_px(&mut self, px: f32) {
        self.canvas.set_px(px);
        let (w, h) = (self.canvas.frame.w, self.canvas.frame.h);
        self.canvas.resize(w, h);
        self.regrid();
    }

    pub fn px(&self) -> f32 {
        self.canvas.px()
    }

    /// The cell for text `px` pixels high, whatever the current size.
    pub fn metrics_at(&self, px: f32) -> super::font::CellMetrics {
        self.canvas.metrics_at(px)
    }

    pub fn frame(&self) -> &Frame {
        &self.canvas.frame
    }

    /// What was painted since the last call, if anything.
    pub fn take_damage(&mut self) -> Option<Damage> {
        self.damage.take()
    }

    fn damage_all(&mut self) {
        let f = &self.canvas.frame;
        self.damage = Some(Damage {
            x0: 0,
            y0: 0,
            x1: f.w,
            y1: f.h,
        });
    }

    /// One cell's size in pixels.
    pub fn cell_px(&self) -> (usize, usize) {
        let m = self.canvas.cell();
        (m.w, m.h)
    }

    fn regrid(&mut self) {
        let (cw, ch) = self.cell_px();
        let cols = (self.canvas.frame.w / cw).clamp(1, u16::MAX as usize) as u16;
        let rows = (self.canvas.frame.h / ch).clamp(1, u16::MAX as usize) as u16;
        if (cols, rows) != (self.cols, self.rows) {
            let mut grid = vec![Cell::default(); cols as usize * rows as usize];
            for y in 0..rows.min(self.rows) as usize {
                for x in 0..cols.min(self.cols) as usize {
                    grid[y * cols as usize + x] = self.grid[y * self.cols as usize + x].clone();
                }
            }
            self.grid = grid;
            self.cols = cols;
            self.rows = rows;
        }
        for y in 0..rows {
            let mut x = 0;
            while x < cols {
                let cell = &self.grid[y as usize * cols as usize + x as usize];
                let span = span_of(cell).min(cols - x);
                self.canvas.paint(x, y, cell, span);
                x += span;
            }
        }
        self.damage_all();
    }

    /// Paints the cell at (`x`, `y`), or the wide glyph covering it from the left.
    fn paint(&mut self, x: u16, y: u16) {
        let at = |x: u16| y as usize * self.cols as usize + x as usize;
        let x = if x > 0 && span_of(&self.grid[at(x - 1)]) == 2 {
            x - 1
        } else {
            x
        };
        let cell = &self.grid[at(x)];
        let span = span_of(cell).min(self.cols - x);
        self.canvas.paint(x, y, cell, span);
        let (cw, ch) = self.cell_px();
        let (fw, fh) = (self.canvas.frame.w, self.canvas.frame.h);
        let cell = Damage {
            x0: (x as usize * cw).min(fw),
            y0: (y as usize * ch).min(fh),
            x1: ((x + span) as usize * cw).min(fw),
            y1: ((y + 1) as usize * ch).min(fh),
        };
        if cell.x0 < cell.x1 && cell.y0 < cell.y1 {
            self.damage = Damage::join(self.damage, Some(cell));
        }
    }
}

impl Backend for PixelBackend {
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        if std::mem::take(&mut self.blank_pending) {
            self.canvas.frame.blank();
            self.damage_all();
        }
        let mut changed = Vec::new();
        for (x, y, cell) in content {
            if x < self.cols && y < self.rows {
                let i = y as usize * self.cols as usize + x as usize;
                self.grid[i] = cell.clone();
                // ratatui never sends the cell a wide glyph covers; blank it so an old
                // glyph there can't be repainted over this one.
                if span_of(cell) == 2 && x + 1 < self.cols {
                    self.grid[i + 1] = Cell::default();
                }
                changed.push((x, y));
            }
        }
        for &(x, y) in &changed {
            self.paint(x, y);
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        Ok(Position::ORIGIN)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, _position: P) -> io::Result<()> {
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.grid.fill(Cell::default());
        self.blank_pending = true;
        Ok(())
    }

    fn size(&self) -> io::Result<Size> {
        Ok(Size::new(self.cols, self.rows))
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        Ok(WindowSize {
            columns_rows: Size::new(self.cols, self.rows),
            pixels: Size::new(self.canvas.frame.w as u16, self.canvas.frame.h as u16),
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::style::{Color, Style};
    use ratatui::widgets::Paragraph;

    const RED: u32 = 0xc80000;

    fn backend(cols: usize, rows: usize) -> PixelBackend {
        let fonts = crate::window::font::Fonts::load();
        let m = fonts.metrics(20.0);
        PixelBackend::new(fonts, 20.0, m.w * cols, m.h * rows)
    }

    fn pixel(b: &PixelBackend, x: usize, y: usize) -> u32 {
        b.frame().px[y * b.frame().w + x]
    }

    fn red_block(f: &mut ratatui::Frame, x: u16, y: u16) {
        let area = ratatui::layout::Rect::new(x, y, 1, 1);
        f.render_widget(
            Paragraph::new("█").style(Style::new().fg(Color::Rgb(200, 0, 0))),
            area,
        );
    }

    #[test]
    fn size_is_pixels_over_cells() {
        let b = backend(10, 3);
        assert_eq!(b.size().unwrap(), Size::new(10, 3));
    }

    #[test]
    fn draw_through_a_terminal() {
        let mut b = backend(10, 3);
        b.take_damage();
        let mut t = Terminal::new(b).unwrap();
        t.draw(|f| red_block(f, 2, 1)).unwrap();
        let b = t.backend_mut();
        let (cw, ch) = b.cell_px();
        assert_eq!(pixel(b, 2 * cw, ch), RED);
        assert_eq!(pixel(b, 3 * cw - 1, 2 * ch - 1), RED);
        assert_ne!(pixel(b, 3 * cw, ch), RED);
        assert_eq!(
            b.take_damage(),
            Some(Damage {
                x0: 2 * cw,
                y0: ch,
                x1: 3 * cw,
                y1: 2 * ch
            }),
            "the damage is the painted cell"
        );
        assert_eq!(b.take_damage(), None);
    }

    #[test]
    fn zero_and_tiny_sizes_never_panic() {
        let mut b = backend(10, 3);
        b.resize(0, 0);
        assert_eq!(b.size().unwrap(), Size::new(1, 1));
        let mut t = Terminal::new(b).unwrap();
        t.draw(crate::app::draw_too_small).unwrap();
        t.backend_mut().resize(3, 3);
        t.draw(crate::app::draw_too_small).unwrap();
    }

    #[test]
    fn resize_cells_repaints_everything() {
        let b = backend(10, 3);
        let mut t = Terminal::new(b).unwrap();
        t.draw(|f| red_block(f, 0, 0)).unwrap();
        t.backend_mut().set_px(30.0);
        t.draw(|f| red_block(f, 0, 0)).unwrap();
        let b = t.backend_mut();
        let (cw, ch) = b.cell_px();
        assert_eq!(pixel(b, cw - 1, ch - 1), RED);
        // A size change that keeps the cell count repaints from the grid.
        t.backend_mut().set_px(30.01);
        let b = t.backend_mut();
        let (cw, ch) = b.cell_px();
        assert_eq!(pixel(b, cw - 1, ch - 1), RED);
    }

    #[test]
    fn wide_glyph_spans_two_cells() {
        let mut b = backend(10, 3);
        let (cw, ch) = b.cell_px();
        let mut wide = Cell::default();
        wide.set_symbol("⚡").set_fg(Color::Rgb(200, 0, 0));
        b.draw([(0, 0, &wide)].into_iter()).unwrap();
        let ink = |b: &PixelBackend| {
            (0..ch).any(|y| (cw..2 * cw).any(|x| pixel(b, x, y) != crate::window::raster::BG))
        };
        assert!(ink(&b), "the glyph reaches into the second cell");
        let blank = Cell::default();
        b.draw([(1, 0, &blank)].into_iter()).unwrap();
        assert!(ink(&b), "a redraw of the second cell keeps the glyph");
    }

    #[test]
    fn a_wide_glyph_moved_left_leaves_no_ghost() {
        let mut b = backend(10, 3);
        let (cw, ch) = b.cell_px();
        let mut wide = Cell::default();
        wide.set_symbol("⚡").set_fg(Color::Rgb(200, 0, 0));
        b.draw([(1, 0, &wide)].into_iter()).unwrap();
        // ratatui moves it one left: the glyph at 0, the freed cell 2; never cell 1.
        let blank = Cell::default();
        b.draw([(0, 0, &wide), (2, 0, &blank)].into_iter()).unwrap();
        let bg = crate::window::raster::BG;
        assert!((0..ch).all(|y| (2 * cw..3 * cw).all(|x| pixel(&b, x, y) == bg)));
    }

    #[test]
    fn regrid_damages_everything() {
        let mut b = backend(10, 3);
        b.take_damage();
        b.set_px(30.0);
        let f = b.frame();
        let all = Damage {
            x0: 0,
            y0: 0,
            x1: f.w,
            y1: f.h,
        };
        assert_eq!(b.take_damage(), Some(all));
    }

    #[test]
    fn a_resize_keeps_the_picture() {
        let b = backend(10, 3);
        let mut t = Terminal::new(b).unwrap();
        t.draw(|f| red_block(f, 0, 0)).unwrap();
        t.backend_mut().set_px(30.0);
        let b = t.backend_mut();
        let (cw, ch) = b.cell_px();
        assert_eq!(pixel(b, cw - 1, ch - 1), RED, "repainted before any draw");
    }

    #[test]
    fn clear_blanks_at_the_next_draw_not_before() {
        let b = backend(10, 3);
        let mut t = Terminal::new(b).unwrap();
        t.draw(|f| red_block(f, 0, 0)).unwrap();
        t.clear().unwrap();
        assert_eq!(
            pixel(t.backend(), 0, 0),
            RED,
            "a redraw now shows the old frame"
        );
        t.draw(|_| {}).unwrap();
        assert_eq!(pixel(t.backend(), 0, 0), crate::window::raster::BG);
    }

    #[test]
    fn a_resize_there_and_back_then_a_clear_repaints_every_cell() {
        let b = backend(10, 3);
        let mut t = Terminal::new(b).unwrap();
        let draw = |f: &mut ratatui::Frame| {
            red_block(f, 0, 0);
            red_block(f, 9, 2);
        };
        t.draw(draw).unwrap();
        // Bigger text drops the far corner; back again, ratatui sees no size change.
        t.backend_mut().set_px(40.0);
        t.backend_mut().set_px(20.0);
        t.clear().unwrap();
        t.draw(draw).unwrap();
        let b = t.backend();
        let (cw, ch) = b.cell_px();
        assert_eq!(pixel(b, 10 * cw - 1, 3 * ch - 1), RED);
    }
}
