//! The key reference overlay.

use crate::screens::explore::Mode;
use crate::ui::panel::boxed;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

const EXPLORE: &[(&str, &str)] = &[
    ("↑↓←→ hjkl", "pan the camera (shift: fast)"),
    ("[ ]", "stratum down / up (Net on top)"),
    ("w", "walk: drop the @ into the city"),
    ("tab", "open the deck"),
    ("i m e d", "inspector · minimap · legend · digital overlay"),
    ("s", "save city"),
    ("r", "regenerate with a fresh seed"),
    ("g", "configure"),
    ("`", "dev mode on / off"),
    ("?", "this help"),
    ("esc q", "pause menu"),
];

const WALK: &[(&str, &str)] = &[
    ("hjkl yubn ↑↓←→", "move"),
    ("< >", "take stairs, lift, ladder or manhole"),
    ("enter", "jack in at an adjacent terminal / jack out"),
    (".", "wait a tick"),
    ("tab", "open the deck"),
    ("c", "back to free camera (dev mode)"),
    ("`", "dev mode on / off"),
    ("i m e d", "inspector · minimap · legend · digital overlay"),
    ("s", "save city"),
    ("?", "this help"),
    ("esc q", "pause menu"),
];

/// Draws the key list for `mode` centred over whatever is already on screen.
pub fn draw(f: &mut Frame, mode: Mode) {
    let area = f.area();
    let w = area.width.min(60);
    let h = area.height.min(16);
    let rect = Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    );
    f.render_widget(Clear, rect);
    let title = match mode {
        Mode::Explore => "KEYS · EXPLORE",
        Mode::Walk => "KEYS · WALK",
    };
    let block = boxed(title);
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let rows = match mode {
        Mode::Explore => EXPLORE,
        Mode::Walk => WALK,
    };
    let lines: Vec<Line> = rows
        .iter()
        .map(|(k, what)| {
            Line::from(vec![
                Span::styled(
                    format!(" {k:<16}"),
                    Style::default().fg(Color::Rgb(0, 229, 255)),
                ),
                Span::styled(
                    what.to_string(),
                    Style::default().fg(Color::Rgb(205, 212, 222)),
                ),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::explore::Mode;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn help_fits_small_and_large_frames() {
        for (w, h) in [(100u16, 30u16), (200, 60), (20, 5)] {
            let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
            t.draw(|f| draw(f, Mode::Explore)).unwrap();
            t.draw(|f| draw(f, Mode::Walk)).unwrap();
        }
    }
}
