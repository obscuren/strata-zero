//! A ratatui backend that paints pixels: the cells ratatui reports changed go through
//! the `Canvas` into a framebuffer the window presents.

use super::font::Fonts;
use super::raster::{Canvas, Frame, span_of};
use ratatui::backend::{Backend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use std::io;

pub struct PixelBackend {
    canvas: Canvas,
    /// What each cell shows, so a redraw of either half of a wide glyph repaints it whole.
    grid: Vec<Cell>,
    cols: u16,
    rows: u16,
    dirty: bool,
}

impl PixelBackend {
    /// A backend for a window `width` by `height` pixels, text `px` pixels high.
    pub fn new(fonts: Fonts, px: f32, width: usize, height: usize) -> Self {
        let mut b = PixelBackend {
            canvas: Canvas::new(fonts, px, width, height),
            grid: Vec::new(),
            cols: 0,
            rows: 0,
            dirty: true,
        };
        b.regrid();
        b
    }

    /// A new pixel size: a blank frame, and a new grid when the cell count changed
    /// (ratatui then redraws every cell), else the old grid repainted.
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

    pub fn frame(&self) -> &Frame {
        &self.canvas.frame
    }

    /// Whether anything was painted since the last call.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
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
        if (cols, rows) == (self.cols, self.rows) {
            for i in 0..self.grid.len() {
                let (x, y) = ((i % cols as usize) as u16, (i / cols as usize) as u16);
                self.paint(x, y);
            }
        } else {
            self.cols = cols;
            self.rows = rows;
            self.grid = vec![Cell::default(); cols as usize * rows as usize];
        }
        self.dirty = true;
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
    }
}

impl Backend for PixelBackend {
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let mut changed = Vec::new();
        for (x, y, cell) in content {
            if x < self.cols && y < self.rows {
                self.grid[y as usize * self.cols as usize + x as usize] = cell.clone();
                changed.push((x, y));
            }
        }
        for &(x, y) in &changed {
            self.paint(x, y);
        }
        self.dirty |= !changed.is_empty();
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
        let (w, h) = (self.canvas.frame.w, self.canvas.frame.h);
        self.canvas.resize(w, h);
        self.dirty = true;
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
    use crate::window::raster::tests::fonts;
    use ratatui::Terminal;
    use ratatui::style::{Color, Style};
    use ratatui::widgets::Paragraph;

    const RED: u32 = 0xc80000;

    fn backend(cols: usize, rows: usize) -> Option<PixelBackend> {
        let fonts = fonts()?;
        let m = fonts.metrics(20.0);
        Some(PixelBackend::new(fonts, 20.0, m.w * cols, m.h * rows))
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
        let Some(b) = backend(10, 3) else { return };
        assert_eq!(b.size().unwrap(), Size::new(10, 3));
    }

    #[test]
    fn draw_through_a_terminal() {
        let Some(mut b) = backend(10, 3) else { return };
        b.take_dirty();
        let mut t = Terminal::new(b).unwrap();
        t.draw(|f| red_block(f, 2, 1)).unwrap();
        let b = t.backend_mut();
        let (cw, ch) = b.cell_px();
        assert_eq!(pixel(b, 2 * cw, ch), RED);
        assert_eq!(pixel(b, 3 * cw - 1, 2 * ch - 1), RED);
        assert_ne!(pixel(b, 3 * cw, ch), RED);
        assert!(b.take_dirty());
        assert!(!b.take_dirty());
    }

    #[test]
    fn zero_and_tiny_sizes_never_panic() {
        let Some(mut b) = backend(10, 3) else { return };
        b.resize(0, 0);
        assert_eq!(b.size().unwrap(), Size::new(1, 1));
        let mut t = Terminal::new(b).unwrap();
        t.draw(crate::app::draw_too_small).unwrap();
        t.backend_mut().resize(3, 3);
        t.draw(crate::app::draw_too_small).unwrap();
    }

    #[test]
    fn resize_cells_repaints_everything() {
        let Some(b) = backend(10, 3) else { return };
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
        let Some(mut b) = backend(10, 3) else { return };
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
}
