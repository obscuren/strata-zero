//! Block-letter logo with neon flicker and a scanline sweep.

use crate::anim::effects::flicker;
use crate::render::Rgb;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

/// Columns per letter including the gap.
pub const CELL_W: u16 = 6;
pub const ROWS: u16 = 5;

/// Five rows of up to five cells for a letter; shorter rows are padded with spaces.
pub fn glyph(c: char) -> [&'static str; 5] {
    match c.to_ascii_uppercase() {
        'A' => ["▄▀▀▄", "█  █", "█▀▀█", "█  █", "▀  ▀"],
        'B' => ["█▀▀▄", "█  █", "█▀▀▄", "█  █", "▀▀▀ "],
        'C' => ["▄▀▀▀", "█   ", "█   ", "█   ", "▀▀▀▀"],
        'D' => ["█▀▀▄", "█  █", "█  █", "█  █", "▀▀▀ "],
        'E' => ["█▀▀▀", "█   ", "█▀▀ ", "█   ", "▀▀▀▀"],
        'F' => ["█▀▀▀", "█   ", "█▀▀ ", "█   ", "▀   "],
        'G' => ["▄▀▀▀", "█   ", "█ ▀█", "█  █", "▀▀▀▀"],
        'H' => ["█  █", "█  █", "█▀▀█", "█  █", "▀  ▀"],
        'I' => ["▀█▀ ", " █  ", " █  ", " █  ", "▀▀▀ "],
        'J' => ["  ▀█", "   █", "   █", "█  █", "▀▀▀ "],
        'K' => ["█  █", "█ █ ", "██  ", "█ █ ", "▀  ▀"],
        'L' => ["█   ", "█   ", "█   ", "█   ", "▀▀▀▀"],
        'M' => ["█▄ ▄█", "█ ▀ █", "█   █", "█   █", "▀   ▀"],
        'N' => ["█▄  █", "█ █ █", "█  ██", "█   █", "▀   ▀"],
        'O' => ["▄▀▀▄", "█  █", "█  █", "█  █", "▀▀▀▀"],
        'P' => ["█▀▀▄", "█  █", "█▀▀ ", "█   ", "▀   "],
        'Q' => ["▄▀▀▄ ", "█  █ ", "█  █ ", "█ ▄█ ", "▀▀▀▀▄"],
        'R' => ["█▀▀▄", "█  █", "█▀▀▄", "█  █", "▀  ▀"],
        'S' => ["▄▀▀▀", "█   ", "▀▀▀▄", "   █", "▀▀▀ "],
        'T' => ["▀▀█▀▀", "  █  ", "  █  ", "  █  ", "  ▀  "],
        'U' => ["█  █", "█  █", "█  █", "█  █", "▀▀▀▀"],
        'V' => ["█   █", "█   █", "█   █", " █ █ ", "  ▀  "],
        'W' => ["█   █", "█   █", "█ ▄ █", "█▀ ▀█", "▀   ▀"],
        'X' => ["█   █", " █ █ ", "  █  ", " █ █ ", "▀   ▀"],
        'Y' => ["█   █", " █ █ ", "  █  ", "  █  ", "  ▀  "],
        'Z' => ["▀▀▀▀█", "   █ ", "  █  ", " █   ", "█▀▀▀▀"],
        '0' => ["▄▀▀▄", "█ ▄█", "█▀ █", "█  █", "▀▀▀▀"],
        '1' => [" ▄█ ", "  █ ", "  █ ", "  █ ", " ▀▀▀"],
        '2' => ["▀▀▀▄", "   █", "▄▀▀▀", "█   ", "▀▀▀▀"],
        '3' => ["▀▀▀▄", "   █", " ▀▀█", "   █", "▀▀▀ "],
        '4' => ["█  █", "█  █", "▀▀▀█", "   █", "   ▀"],
        '5' => ["█▀▀▀", "█   ", "▀▀▀▄", "   █", "▀▀▀ "],
        '6' => ["▄▀▀▀", "█   ", "█▀▀▄", "█  █", "▀▀▀▀"],
        '7' => ["▀▀▀█", "   █", "  █ ", "  █ ", "  ▀ "],
        '8' => ["▄▀▀▄", "█  █", "▄▀▀▄", "█  █", "▀▀▀▀"],
        '9' => ["▄▀▀▄", "█  █", "▀▀▀█", "   █", "▀▀▀ "],
        _ => ["    ", "    ", "    ", "    ", "    "],
    }
}

/// Whether every character of `text` has a glyph; spaces count, the rest must have ink.
pub fn covers(text: &str) -> bool {
    text.chars()
        .all(|c| c == ' ' || glyph(c).iter().any(|row| !row.trim().is_empty()))
}

/// Rendered width of `text` in cells.
pub fn width(text: &str) -> u16 {
    text.chars().count() as u16 * CELL_W
}

pub struct Logo<'a> {
    pub text: &'a str,
    pub tick: u64,
    pub accent: Rgb,
}

impl Widget for Logo<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let scan = ((self.tick / 2) % (ROWS as u64 + 10)) as u16;
        for (i, c) in self.text.chars().enumerate() {
            let broken = i % 7 == 3;
            let f = flicker(self.tick, i as u8, broken);
            let color = self.accent.scale(f);
            let rows = glyph(c);
            for (r, row) in rows.iter().enumerate() {
                let y = area.y + r as u16;
                if y >= area.bottom() {
                    break;
                }
                let bright = r as u16 == scan;
                let fg = if bright {
                    color.mix(Rgb(255, 255, 255), 0.5)
                } else {
                    color
                };
                for (col, ch) in row.chars().enumerate() {
                    if ch == ' ' {
                        continue;
                    }
                    let x = area.x + i as u16 * CELL_W + col as u16;
                    if x >= area.right() {
                        break;
                    }
                    buf[(x, y)].set_char(ch).set_fg(fg.to_color());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Rgb;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Widget;

    #[test]
    fn logo_width_and_render() {
        assert_eq!(width("AB"), 12);
        for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 ".chars() {
            let g = glyph(c);
            assert!(g.iter().all(|r| r.chars().count() <= 5), "{c}");
        }
        let mut t = Terminal::new(TestBackend::new(60, 7)).unwrap();
        t.draw(|f| {
            Logo {
                text: "CITY 9",
                tick: 3,
                accent: Rgb(255, 0, 200),
            }
            .render(f.area(), f.buffer_mut())
        })
        .unwrap();
        assert!((0..60).any(|x| t.backend().buffer()[(x, 0)].symbol() == "█"));
        let mut tiny = Terminal::new(TestBackend::new(3, 2)).unwrap();
        tiny.draw(|f| {
            Logo {
                text: "CITY",
                tick: 3,
                accent: Rgb(255, 0, 200),
            }
            .render(f.area(), f.buffer_mut())
        })
        .unwrap();
    }
}
