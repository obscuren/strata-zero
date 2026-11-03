//! Settings: dev mode (once unlocked), rain and the FPS counter, flipped in place, and in
//! a window that draws through the GPU the screen effects, cycled.

use crate::app::{AppState, Screen, ScreenCmd, ScreenFx, TransitionKind};
use crate::ui::panel::boxed;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// A row of the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    Dev,
    Rain,
    Fps,
    Cord,
    Effects,
}

impl Row {
    fn label(self) -> &'static str {
        match self {
            Row::Dev => "Dev mode",
            Row::Rain => "Rain",
            Row::Fps => "FPS counter",
            Row::Cord => "Cord on the Net",
            Row::Effects => "Screen effects",
        }
    }
}

const ACCENT: Color = Color::Rgb(0, 229, 255);
const DIM: Color = Color::Rgb(160, 170, 185);
const TEXT: Color = Color::Rgb(205, 212, 222);

pub struct Settings {
    selected: usize,
}

impl Settings {
    pub fn new() -> Self {
        Settings { selected: 0 }
    }

    /// The rows there are: dev mode only once it is unlocked, the screen effects only
    /// where they can be drawn.
    fn rows(st: &AppState) -> Vec<Row> {
        let mut rows = Vec::new();
        if st.dev_unlocked {
            rows.push(Row::Dev);
        }
        rows.extend([Row::Rain, Row::Fps, Row::Cord]);
        if st.effects {
            rows.push(Row::Effects);
        }
        rows
    }

    /// The row's value and whether it reads as on.
    fn get(st: &AppState, row: Row) -> (&'static str, bool) {
        let flag = |on: bool| (if on { "ON" } else { "OFF" }, on);
        match row {
            Row::Dev => flag(st.dev),
            Row::Rain => flag(st.rain),
            Row::Fps => flag(st.show_fps),
            Row::Cord => flag(st.cord),
            Row::Effects => (st.screen_fx.label(), st.screen_fx != ScreenFx::Off),
        }
    }

    fn flip(st: &mut AppState, row: Row) {
        match row {
            Row::Dev => st.set_dev(!st.dev),
            Row::Rain => st.rain = !st.rain,
            Row::Fps => st.show_fps = !st.show_fps,
            Row::Cord => st.cord = !st.cord,
            Row::Effects => st.screen_fx = st.screen_fx.next(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for Settings {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        let rows = Self::rows(st);
        let n = rows.len();
        for k in keys {
            match k.code {
                KeyCode::Up | KeyCode::Char('k') => self.selected = (self.selected + n - 1) % n,
                KeyCode::Down | KeyCode::Char('j') => self.selected = (self.selected + 1) % n,
                KeyCode::Left | KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => {
                    Self::flip(st, rows[self.selected])
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    return ScreenCmd::Pop {
                        fx: TransitionKind::None,
                    };
                }
                _ => {}
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let (w, h) = (42u16.min(area.width), 9u16.min(area.height));
        let panel = Rect::new(
            area.x + (area.width - w) / 2,
            area.y + (area.height - h) / 2,
            w,
            h,
        );
        let block = boxed("SETTINGS");
        let inner = block.inner(panel);
        f.render_widget(block, panel);
        let mut lines: Vec<Line> = Vec::new();
        for (i, row) in Self::rows(st).into_iter().enumerate() {
            let (value, on) = Self::get(st, row);
            let label = row.label();
            let sel = i == self.selected;
            let style = if sel {
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(TEXT)
            };
            lines.push(Line::from(vec![
                Span::styled(if sel { " ▸ " } else { "   " }, style),
                Span::styled(format!("{label:<14}"), style),
                Span::styled(
                    format!("[ {value} ]"),
                    Style::default().fg(if on { ACCENT } else { DIM }),
                ),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " ↑↓ select · ←→ flip · esc back",
            Style::default().fg(DIM),
        )));
        f.render_widget(Paragraph::new(lines), inner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::testutil::{buffer_text, key};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn the_cord_row_flips() {
        let mut st = AppState::new();
        assert!(st.cord, "on by default");
        assert_eq!(Settings::get(&st, Row::Cord), ("ON", true));
        Settings::flip(&mut st, Row::Cord);
        assert!(!st.cord);
        assert_eq!(Settings::get(&st, Row::Cord), ("OFF", false));
    }

    #[test]
    fn settings_dev_off_clears_dev_sections() {
        use crate::ui::sidebar::Section;
        let mut st = AppState::new();
        st.unlock_dev();
        st.sidebar.set(Section::Inspect, true);
        st.sidebar.set(Section::Legend, true);
        Settings::flip(&mut st, Row::Dev);
        assert!(!st.dev);
        assert!(!st.sidebar.is_on(Section::Inspect) && !st.sidebar.is_on(Section::Legend));
    }

    #[test]
    fn settings_flip_state_and_pop() {
        let mut st = AppState::new();
        st.unlock_dev();
        let mut s = Settings::new();
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(
            text.contains("SETTINGS") && text.contains("Dev mode") && text.contains("[ ON ]"),
            "{text}"
        );
        s.update(&mut st, &[key(KeyCode::Down), key(KeyCode::Right)]);
        assert!(!st.rain);
        s.update(&mut st, &[key(KeyCode::Down), key(KeyCode::Enter)]);
        assert!(!st.show_fps);
        s.update(
            &mut st,
            &[key(KeyCode::Up), key(KeyCode::Up), key(KeyCode::Char(' '))],
        );
        assert!(!st.dev);
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert!(buffer_text(t.backend().buffer()).contains("[ OFF ]"));
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
    }

    #[test]
    fn no_dev_mode_row_until_it_is_unlocked() {
        let mut st = AppState::new();
        let mut s = Settings::new();
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(
            text.contains("Rain") && !text.contains("Dev mode"),
            "{text}"
        );
        // The top row is rain, and nothing a key does turns dev mode on.
        s.update(&mut st, &[key(KeyCode::Enter)]);
        assert!(!st.rain && !st.dev);
        st.set_dev(true);
        assert!(!st.dev, "locked");
        st.unlock_dev();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(
            text.contains("Dev mode") && text.contains("[ ON ]"),
            "{text}"
        );
    }

    #[test]
    fn settings_rows_wrap() {
        let mut st = AppState::new();
        let mut s = Settings::new();
        s.update(&mut st, &[key(KeyCode::Up)]);
        assert_eq!(s.selected, Settings::rows(&st).len() - 1);
        s.update(&mut st, &[key(KeyCode::Down)]);
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn screen_effects_row_only_where_they_draw() {
        let mut st = AppState::new();
        let s = Settings::new();
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert!(!buffer_text(t.backend().buffer()).contains("Screen effects"));
        assert_eq!(Settings::rows(&st), [Row::Rain, Row::Fps, Row::Cord]);
        st.effects = true;
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(
            text.contains("Screen effects") && text.contains("[ SOFT ]"),
            "{text}"
        );
    }

    #[test]
    fn screen_effects_cycle() {
        let mut st = AppState::new();
        st.effects = true;
        let mut s = Settings::new();
        s.update(&mut st, &[key(KeyCode::Up)]);
        assert_eq!(
            Settings::rows(&st)[s.selected],
            Row::Effects,
            "up from the top wraps to the effects row"
        );
        s.update(&mut st, &[key(KeyCode::Right)]);
        assert_eq!(st.screen_fx, ScreenFx::Crt);
        s.update(&mut st, &[key(KeyCode::Enter)]);
        assert_eq!(st.screen_fx, ScreenFx::Off);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(st.screen_fx, ScreenFx::Soft);
        assert_eq!(ScreenFx::parse("CRT"), Some(ScreenFx::Crt));
        assert_eq!(ScreenFx::parse("glow"), None);
    }
}
