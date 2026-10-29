//! Cells into pixels: a framebuffer, the colours and modifiers of a ratatui `Cell`, and
//! its character from `shapes` or the font.

use super::font::{CellMetrics, Fonts};
use super::shapes;
use ratatui::buffer::Cell;
use ratatui::style::{Color, Modifier};

/// `Reset` text: the shot harness's foreground.
pub const FG: u32 = 0xcdd4de;
/// `Reset` background: the style pack's background (`flavor/style.ron`), which the map's
/// tiles are painted in, so the frame cells left on the default match them.
pub const BG: u32 = 0x0a0b10;

/// Pixels as `0x00RRGGBB`, row-major.
pub struct Frame {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u32>,
}

impl Frame {
    pub fn new(w: usize, h: usize) -> Self {
        Frame {
            w,
            h,
            px: vec![BG; w * h],
        }
    }

    /// Fills `[x0, x1) × [y0, y1)`, clipped to the frame.
    fn fill(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, c: u32) {
        let (x1, y1) = (x1.min(self.w), y1.min(self.h));
        for y in y0.min(y1)..y1 {
            self.px[y * self.w + x0.min(x1)..y * self.w + x1].fill(c);
        }
    }

    /// `c` over the pixel at (`x`, `y`) with coverage `a`.
    fn blend(&mut self, x: usize, y: usize, c: u32, a: u8) {
        let p = &mut self.px[y * self.w + x];
        *p = mix(*p, c, a as u32);
    }

    /// The frame as RGBA bytes, for a PNG.
    pub fn to_rgba(&self) -> Vec<u8> {
        self.px
            .iter()
            .flat_map(|&p| [(p >> 16) as u8, (p >> 8) as u8, p as u8, 255])
            .collect()
    }
}

/// `a/255` of the way from `from` to `to`, per channel.
fn mix(from: u32, to: u32, a: u32) -> u32 {
    let ch = |shift: u32| {
        let (f, t) = ((from >> shift) & 0xff, (to >> shift) & 0xff);
        ((f * (255 - a) + t * a + 127) / 255) << shift
    };
    ch(16) | ch(8) | ch(0)
}

/// The 16 ANSI colours, xterm's.
const ANSI: [u32; 16] = [
    0x000000, 0xcd0000, 0x00cd00, 0xcdcd00, 0x0000ee, 0xcd00cd, 0x00cdcd, 0xe5e5e5, 0x7f7f7f,
    0xff0000, 0x00ff00, 0xffff00, 0x5c5cff, 0xff00ff, 0x00ffff, 0xffffff,
];

/// A ratatui colour as `0xRRGGBB`; `Reset` is `fallback`.
pub fn rgb(c: Color, fallback: u32) -> u32 {
    match c {
        Color::Reset => fallback,
        Color::Black => ANSI[0],
        Color::Red => ANSI[1],
        Color::Green => ANSI[2],
        Color::Yellow => ANSI[3],
        Color::Blue => ANSI[4],
        Color::Magenta => ANSI[5],
        Color::Cyan => ANSI[6],
        Color::Gray => ANSI[7],
        Color::DarkGray => ANSI[8],
        Color::LightRed => ANSI[9],
        Color::LightGreen => ANSI[10],
        Color::LightYellow => ANSI[11],
        Color::LightBlue => ANSI[12],
        Color::LightMagenta => ANSI[13],
        Color::LightCyan => ANSI[14],
        Color::White => ANSI[15],
        Color::Rgb(r, g, b) => (r as u32) << 16 | (g as u32) << 8 | b as u32,
        Color::Indexed(i) => match i {
            0..=15 => ANSI[i as usize],
            16..=231 => {
                let level = |v: u8| if v == 0 { 0 } else { 55 + 40 * v as u32 };
                let i = i - 16;
                level(i / 36) << 16 | level(i / 6 % 6) << 8 | level(i % 6)
            }
            _ => {
                let v = 8 + 10 * (i as u32 - 232);
                v << 16 | v << 8 | v
            }
        },
    }
}

/// A framebuffer and what it takes to paint cells into it at one text size.
pub struct Canvas {
    pub frame: Frame,
    fonts: Fonts,
    px: f32,
    cell: CellMetrics,
}

impl Canvas {
    /// A canvas `w` by `h` pixels for text `px` pixels high.
    pub fn new(fonts: Fonts, px: f32, w: usize, h: usize) -> Self {
        let cell = fonts.metrics(px);
        Canvas {
            frame: Frame::new(w, h),
            fonts,
            px,
            cell,
        }
    }

    pub fn cell(&self) -> CellMetrics {
        self.cell
    }

    pub fn px(&self) -> f32 {
        self.px
    }

    /// A new text size: new cell metrics, the old glyphs dropped.
    pub fn set_px(&mut self, px: f32) {
        self.px = px;
        self.cell = self.fonts.metrics(px);
        self.fonts.clear_cache();
    }

    /// A blank frame of `w` by `h` pixels.
    pub fn resize(&mut self, w: usize, h: usize) {
        self.frame = Frame::new(w, h);
    }

    /// Paints `cell` at column `col`, row `row`, over `span` cells (2 for a wide glyph).
    pub fn paint(&mut self, col: u16, row: u16, cell: &Cell, span: u16) {
        let CellMetrics { w, h, baseline } = self.cell;
        let (x0, y0) = (col as usize * w, row as usize * h);
        let (x1, y1) = (x0 + w * span as usize, y0 + h);
        if x0 >= self.frame.w || y0 >= self.frame.h {
            return;
        }
        let (x1c, y1c) = (x1.min(self.frame.w), y1.min(self.frame.h));
        let mods = cell.modifier;
        let (mut fg, mut bg) = (rgb(cell.fg, FG), rgb(cell.bg, BG));
        if mods.contains(Modifier::REVERSED) {
            std::mem::swap(&mut fg, &mut bg);
        }
        if mods.contains(Modifier::DIM) {
            fg = mix(fg, bg, 128);
        }
        self.frame.fill(x0, y0, x1c, y1c, bg);

        let c = cell.symbol().chars().next().unwrap_or(' ');
        let line = ((h as f32 / 16.0).round() as usize).max(1);
        if mods.contains(Modifier::UNDERLINED) {
            let y = (y0 + baseline + line).min(y1 - line);
            self.frame.fill(x0, y, x1c, y + line, fg);
        }
        if mods.contains(Modifier::CROSSED_OUT) {
            let y = y0 + h / 2;
            self.frame.fill(x0, y, x1c, y + line, fg);
        }
        if c == ' ' || mods.contains(Modifier::HIDDEN) {
            return;
        }
        if let Some(mask) = shapes::shape(c, w, h) {
            for sy in 0..h.min(y1c - y0) {
                for sx in 0..w.min(x1c - x0) {
                    let a = mask[sy * w + sx];
                    if a > 0 {
                        self.frame.blend(x0 + sx, y0 + sy, fg, a);
                    }
                }
            }
            return;
        }
        let bold = mods.contains(Modifier::BOLD);
        let Some(g) = self.fonts.glyph(c, bold, self.px) else {
            // No face has it: a hollow box inside the cell.
            let (bx0, by0, bx1, by1) =
                (x0 + 1, y0 + 2, x1c.saturating_sub(1), y1c.saturating_sub(2));
            self.frame.fill(bx0, by0, bx1, by0 + 1, fg);
            self.frame.fill(bx0, by1 - 1, bx1, by1, fg);
            self.frame.fill(bx0, by0, bx0 + 1, by1, fg);
            self.frame.fill(bx1 - 1, by0, bx1, by1, fg);
            return;
        };
        for gy in 0..g.h {
            let y = y0 as i32 + g.top + gy as i32;
            if y < y0 as i32 || y >= y1c as i32 {
                continue;
            }
            for gx in 0..g.w {
                let x = x0 as i32 + g.left + gx as i32;
                if x < x0 as i32 || x >= x1c as i32 {
                    continue;
                }
                let a = g.alpha[gy * g.w + gx];
                if a > 0 {
                    // Lift thin coverage a little, as macOS text reads heavier.
                    let a = (255.0 * (a as f32 / 255.0).powf(0.8)) as u8;
                    self.frame.blend(x as usize, y as usize, fg, a);
                }
            }
        }
    }

    /// Paints all of `buf` into a frame sized to it, for a screenshot.
    #[cfg(test)]
    pub fn paint_buffer(&mut self, buf: &ratatui::buffer::Buffer) {
        let CellMetrics { w, h, .. } = self.cell;
        self.resize(buf.area.width as usize * w, buf.area.height as usize * h);
        for y in 0..buf.area.height {
            let mut x = 0;
            while x < buf.area.width {
                let cell = &buf[(x, y)];
                let span = span_of(cell).min(buf.area.width - x);
                self.paint(x, y, cell, span);
                x += span;
            }
        }
    }
}

/// How many cells `cell`'s symbol covers.
pub fn span_of(cell: &Cell) -> u16 {
    unicode_width::UnicodeWidthStr::width(cell.symbol()).clamp(1, 2) as u16
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use ratatui::style::Style;

    /// The machine's fonts, or `None` (and a note) where it has none.
    pub(crate) fn fonts() -> Option<Fonts> {
        match Fonts::load() {
            Ok(f) => Some(f),
            Err(e) => {
                eprintln!("skipped: {e}");
                None
            }
        }
    }

    fn cell(sym: &str, style: Style) -> Cell {
        let mut c = Cell::default();
        c.set_symbol(sym);
        c.set_style(style);
        c
    }

    /// A one-cell canvas painted with `c`; the cell's pixels.
    fn painted(c: &Cell) -> Option<(CellMetrics, Vec<u32>)> {
        let fonts = fonts()?;
        let m = fonts.metrics(20.0);
        let mut canvas = Canvas::new(fonts, 20.0, m.w * 3, m.h);
        canvas.paint(1, 0, c, 1);
        let px = (0..m.h)
            .flat_map(|y| (m.w..2 * m.w).map(move |x| (x, y)))
            .map(|(x, y)| canvas.frame.px[y * canvas.frame.w + x])
            .collect();
        // Nothing painted outside the cell.
        for y in 0..m.h {
            for x in (0..m.w).chain(2 * m.w..3 * m.w) {
                assert_eq!(
                    canvas.frame.px[y * canvas.frame.w + x],
                    BG,
                    "spilled at {x},{y}"
                );
            }
        }
        Some((m, px))
    }

    #[test]
    fn default_bg_is_the_style_pack_s() {
        let b = crate::app::AppState::new().style.background.to_color();
        assert_eq!(rgb(b, 0), BG);
    }

    #[test]
    fn colours() {
        assert_eq!(rgb(Color::Rgb(1, 2, 3), 0), 0x010203);
        assert_eq!(rgb(Color::Reset, FG), FG);
        assert_eq!(rgb(Color::Red, 0), 0xcd0000);
        assert_eq!(rgb(Color::Indexed(196), 0), 0xff0000);
        assert_eq!(rgb(Color::Indexed(232), 0), 0x080808);
    }

    #[test]
    fn block_fills_with_fg() {
        let Some((_, px)) = painted(&cell("█", Style::new().fg(Color::Rgb(200, 0, 0)))) else {
            return;
        };
        assert!(px.iter().all(|&p| p == 0xc80000));
    }

    #[test]
    fn space_fills_with_bg() {
        let Some((_, px)) = painted(&cell(" ", Style::new().bg(Color::Rgb(0, 0, 200)))) else {
            return;
        };
        assert!(px.iter().all(|&p| p == 0x0000c8));
    }

    #[test]
    fn reversed_swaps() {
        let style = Style::new()
            .fg(Color::Rgb(200, 0, 0))
            .bg(Color::Rgb(0, 0, 200))
            .add_modifier(Modifier::REVERSED);
        let Some((_, px)) = painted(&cell("█", style)) else {
            return;
        };
        assert!(px.iter().all(|&p| p == 0x0000c8));
    }

    #[test]
    fn dim_halves_toward_bg() {
        let style = Style::new()
            .fg(Color::Rgb(200, 100, 0))
            .bg(Color::Rgb(0, 0, 0))
            .add_modifier(Modifier::DIM);
        let Some((_, px)) = painted(&cell("█", style)) else {
            return;
        };
        assert!(px.iter().all(|&p| p == 0x643200), "{:06x}", px[0]);
    }

    #[test]
    fn letter_has_ink() {
        let style = Style::new()
            .fg(Color::Rgb(255, 255, 255))
            .bg(Color::Rgb(0, 0, 0));
        let Some((_, px)) = painted(&cell("M", style)) else {
            return;
        };
        assert!(px.contains(&0xffffff));
        assert!(px.contains(&0));
    }

    #[test]
    fn missing_glyph_draws_a_box() {
        let style = Style::new()
            .fg(Color::Rgb(255, 255, 255))
            .bg(Color::Rgb(0, 0, 0));
        let Some((m, px)) = painted(&cell("\u{10FFFD}", style)) else {
            return;
        };
        assert_eq!(px[2 * m.w + 1], 0xffffff, "the box's top-left corner");
        assert_eq!(px[(m.h / 2) * m.w + m.w / 2], 0, "the box's centre");
    }
}
