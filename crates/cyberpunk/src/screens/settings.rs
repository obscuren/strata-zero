//! Settings: dev mode, rain and the FPS counter, flipped in place.
#![allow(dead_code)] // pushed by the pause menu from Task 6

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::ui::panel::boxed;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const ROWS: [&str; 3] = ["Dev mode", "Rain", "FPS counter"];
const ACCENT: Color = Color::Rgb(0, 229, 255);
const DIM: Color = Color::Rgb(120, 130, 145);
const TEXT: Color = Color::Rgb(205, 212, 222);

pub struct Settings {
    selected: usize,
}

impl Settings {
    pub fn new() -> Self {
        Settings { selected: 0 }
    }

    fn get(st: &AppState, i: usize) -> bool {
        match i {
            0 => st.dev,
            1 => st.rain,
            _ => st.show_fps,
        }
    }

    fn flip(st: &mut AppState, i: usize) {
        match i {
            0 => st.dev = !st.dev,
            1 => st.rain = !st.rain,
            _ => st.show_fps = !st.show_fps,
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
        for k in keys {
            match k.code {
                KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    self.selected = (self.selected + 1).min(ROWS.len() - 1)
                }
                KeyCode::Left | KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => {
                    Self::flip(st, self.selected)
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
        for (i, label) in ROWS.iter().enumerate() {
            let on = Self::get(st, i);
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
                    if on { "[ ON ]" } else { "[ OFF ]" },
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
    fn settings_flip_state_and_pop() {
        let mut st = AppState::new();
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
}
