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
    ("tab", "the deck: Q E switch tabs · tab again closes"),
    ("F1-F4", "panes: here · jobs · map · log"),
    ("F5 F6", "inspect · legend (dev mode)"),
    ("v", "view: the pane toggles as a popup"),
    ("m", "messages: the whole log"),
    ("d", "digital overlay"),
    ("s", "save game"),
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
        "W",
        "up to the overworld, from the open street · on the Net, the Net over it",
    ),
    (
        "space",
        "the thing in reach: look, talk, take, jack in, stairs · continue a run",
    ),
    ("1-5", "fire an ability · jacked in: run a program"),
    ("a", "abilities: the full list"),
    ("J", "jacked in: jack out (asks first)"),
    (
        "x",
        "look: a cursor on the map · tab next thing · space opens it · esc back",
    ),
    (".", "hurry the beat · in a fight: end your turn"),
    (
        "e",
        "jacked in: turns by hand, and back when nothing hunts you",
    ),
    (
        "g",
        "aim a walk (on turns the arrows aim) · space or enter walks · esc drops",
    ),
    (
        "space",
        "on turns: stay put (move half over) · runs the picked program",
    ),
    (
        "1-5",
        "jacked in: pick a program, then its target (tab) · space runs it",
    ),
    ("tab", "the deck: Q E switch tabs · tab again closes"),
    ("F1-F4", "panes: here · jobs · map · log"),
    ("F5 F6", "inspect · legend (dev mode)"),
    ("v", "view: the pane toggles as a popup"),
    ("m", "messages: the whole log"),
    ("d", "digital overlay"),
    ("c", "back to free camera (dev mode)"),
    (":", "dev console (dev mode)"),
    ("`", "dev mode on / off"),
    ("s", "save game"),
    ("?", "this help"),
    ("esc q", "pause menu"),
];

/// The keys that only work in dev mode, left off the list until it is unlocked.
const DEV_KEYS: &[&str] = &["F5 F6", "c", ":", "`"];

/// Draws the key list for `mode` centred over whatever is already on screen; the dev
/// mode keys only with `dev` (dev mode unlocked).
pub fn draw(f: &mut Frame, mode: Mode, dev: bool) {
    let rows: Vec<(&str, &str)> = match mode {
        Mode::Explore => EXPLORE,
        Mode::Walk => WALK,
    }
    .iter()
    .filter(|(k, _)| dev || !DEV_KEYS.contains(k))
    .copied()
    .collect();
    let title = match mode {
        Mode::Explore => "KEYS · EXPLORE",
        Mode::Walk => "KEYS · WALK",
    };
    draw_keys(f, title, &rows);
}

/// A key list under `title`, centred over whatever is already on screen.
pub fn draw_keys(f: &mut Frame, title: &str, rows: &[(&str, &str)]) {
    let area = f.area();
    let w = area.width.min(72);
    let h = area.height.min(rows.len() as u16 + 2);
    let rect = Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    );
    f.render_widget(Clear, rect);
    let block = boxed(title);
    let inner = block.inner(rect);
    f.render_widget(block, rect);
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
            for dev in [false, true] {
                t.draw(|f| draw(f, Mode::Explore, dev)).unwrap();
                t.draw(|f| draw(f, Mode::Walk, dev)).unwrap();
            }
        }
    }

    #[test]
    fn help_lists_the_hud_keys() {
        use crate::screens::testutil::buffer_text;
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| draw(f, Mode::Walk, true)).unwrap();
        let walk = buffer_text(t.backend().buffer());
        assert!(
            walk.contains("fire an ability") && walk.contains("messages") && walk.contains("view"),
            "{walk}"
        );
        assert!(
            !walk.contains("inspector") && walk.contains("pause menu"),
            "{walk}"
        );
        t.draw(|f| draw(f, Mode::Explore, true)).unwrap();
        let explore = buffer_text(t.backend().buffer());
        assert!(
            explore.contains("messages") && !explore.contains("inspector"),
            "{explore}"
        );
    }

    #[test]
    fn the_dev_keys_are_listed_only_once_dev_mode_is_unlocked() {
        use crate::screens::testutil::buffer_text;
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        for mode in [Mode::Walk, Mode::Explore] {
            t.draw(|f| draw(f, mode, false)).unwrap();
            let locked = buffer_text(t.backend().buffer());
            assert!(
                !locked.contains("dev ") && locked.contains("pause menu"),
                "{locked}"
            );
            t.draw(|f| draw(f, mode, true)).unwrap();
            let unlocked = buffer_text(t.backend().buffer());
            assert!(
                unlocked.contains("dev console") && unlocked.contains("dev mode on / off"),
                "{unlocked}"
            );
        }
    }
}
