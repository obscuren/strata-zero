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

/// The keys on foot; the Controls chapter's On foot table lists the same keys
/// (`manual::tests::the_on_foot_table_is_the_key_list`).
pub(crate) const WALK: &[(&str, &str)] = &[
    ("hjkl yubn ↑↓←→", "move"),
    ("< >", "down · up: stairs, a lift, a ladder, a manhole"),
    ("W", "the overworld, from the open street or the Net"),
    ("space", "the thing in reach: look, talk, take, jack in"),
    ("space", "on turns: stay put · jacked in: run the program"),
    ("x", "look: a cursor on the map · tab next · esc back"),
    ("1-5", "fire an ability · jacked in: pick a program"),
    ("a", "abilities: the full list"),
    ("J", "jacked in: jack out (asks first)"),
    ("e", "turns by hand · again: back to real time"),
    (".", "end your turn · jacked in: hurry the beat"),
    ("g", "on turns: aim a walk · enter walks · esc drops"),
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
pub(crate) const DEV_KEYS: &[&str] = &["F5 F6", "c", ":", "`"];

/// The line under the keys: the way on to the manual.
pub const MANUAL_LINE: &str = " space opens the manual · ? closes";

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
    draw_keys_and(f, title, &rows, Some(MANUAL_LINE));
}

/// A key list under `title`, centred over whatever is already on screen.
pub fn draw_keys(f: &mut Frame, title: &str, rows: &[(&str, &str)]) {
    draw_keys_and(f, title, rows, None);
}

/// `draw_keys`, with `foot` dim on a last line of its own.
fn draw_keys_and(f: &mut Frame, title: &str, rows: &[(&str, &str)], foot: Option<&str>) {
    let area = f.area();
    let w = area.width.min(72);
    let extra = if foot.is_some() { 2 } else { 0 };
    let h = area.height.min(rows.len() as u16 + 2 + extra);
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
    let mut lines: Vec<Line> = rows
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
    if let Some(foot) = foot {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            foot.to_string(),
            Style::default().fg(Color::Rgb(120, 130, 145)),
        )));
    }
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
