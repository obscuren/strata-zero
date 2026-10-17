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
    ("F1-F4", "panes: here · quest · map · log"),
    ("F5 F6", "inspect · legend (dev mode)"),
    ("v", "view: the pane toggles as a popup"),
    ("m", "messages: the whole log"),
    ("d", "digital overlay"),
    ("s", "save city"),
    ("r", "regenerate with a fresh seed"),
    ("g", "configure"),
    (":", "dev console (dev mode)"),
    ("`", "dev mode on / off"),
    ("?", "this help"),
    ("esc q", "pause menu"),
];

const WALK: &[(&str, &str)] = &[
    ("hjkl yubn ↑↓←→", "move"),
    ("< >", "take stairs, lift, ladder or manhole"),
    (
        "space",
        "interact: take, jack, stairs, look · continue a run",
    ),
    ("1-5", "fire an ability · jacked in: run a program"),
    ("a", "abilities: the full list"),
    ("enter", "jack in at an adjacent terminal / jack out"),
    (".", "hurry the beat · in a fight: end your turn"),
    (
        "e",
        "jacked in: turns by hand, and back when nothing hunts you",
    ),
    ("tab", "open the deck"),
    ("F1-F4", "panes: here · quest · map · log"),
    ("F5 F6", "inspect · legend (dev mode)"),
    ("v", "view: the pane toggles as a popup"),
    ("m", "messages: the whole log"),
    ("d", "digital overlay"),
    ("c", "back to free camera (dev mode)"),
    (":", "dev console (dev mode)"),
    ("`", "dev mode on / off"),
    ("s", "save city"),
    ("?", "this help"),
    ("esc q", "pause menu"),
];

/// Draws the key list for `mode` centred over whatever is already on screen.
pub fn draw(f: &mut Frame, mode: Mode) {
    let area = f.area();
    let w = area.width.min(60);
    let h = area.height.min(22);
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

    #[test]
    fn help_lists_the_hud_keys() {
        use crate::screens::testutil::buffer_text;
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| draw(f, Mode::Walk)).unwrap();
        let walk = buffer_text(t.backend().buffer());
        assert!(
            walk.contains("fire an ability") && walk.contains("messages") && walk.contains("view"),
            "{walk}"
        );
        assert!(
            !walk.contains("inspector") && walk.contains("pause menu"),
            "{walk}"
        );
        t.draw(|f| draw(f, Mode::Explore)).unwrap();
        let explore = buffer_text(t.backend().buffer());
        assert!(
            explore.contains("messages") && !explore.contains("inspector"),
            "{explore}"
        );
    }
}
