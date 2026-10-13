//! A scanline wash that darkens a frame so a menu can sit over it.

use ratatui::buffer::Buffer;
use ratatui::style::Color;

const DIM: f32 = 0.35;
const SCANLINE: f32 = 0.8;

fn scale(c: Color, f: f32) -> Color {
    match c {
        Color::Rgb(r, g, b) => Color::Rgb(
            (r as f32 * f).round() as u8,
            (g as f32 * f).round() as u8,
            (b as f32 * f).round() as u8,
        ),
        other => other,
    }
}

/// Darkens every cell; odd rows a little more, for scanlines.
pub fn wash(buf: &mut Buffer) {
    let area = buf.area;
    for y in area.y..area.bottom() {
        let f = if y % 2 == 1 { DIM * SCANLINE } else { DIM };
        for x in area.x..area.right() {
            let cell = &mut buf[(x, y)];
            let fg = scale(cell.fg, f);
            let bg = scale(cell.bg, f);
            cell.set_fg(fg).set_bg(bg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Color;

    #[test]
    fn wash_darkens_rgb_and_leaves_reset_alone() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 2));
        buf[(0, 0)]
            .set_fg(Color::Rgb(200, 100, 50))
            .set_bg(Color::Rgb(20, 20, 20));
        buf[(0, 1)].set_fg(Color::Rgb(200, 100, 50));
        buf[(1, 0)].set_fg(Color::Reset);
        wash(&mut buf);
        assert_eq!(buf[(0, 0)].fg, Color::Rgb(70, 35, 18));
        assert_eq!(buf[(0, 0)].bg, Color::Rgb(7, 7, 7));
        assert_eq!(
            buf[(0, 1)].fg,
            Color::Rgb(56, 28, 14),
            "odd rows are darker"
        );
        assert_eq!(buf[(1, 0)].fg, Color::Reset);
    }
}
