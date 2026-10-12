//! Screens: one module per screen, plus shared test helpers.

use crate::app::{AppState, Screen, ScreenCmd};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;

/// Draws nothing; stands in until the title screen lands.
pub struct Blank;

impl Screen for Blank {
    fn update(&mut self, _st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        if keys
            .iter()
            .any(|k| matches!(k.code, KeyCode::Char('q') | KeyCode::Esc))
        {
            ScreenCmd::Quit
        } else {
            ScreenCmd::Stay
        }
    }
    fn draw(&self, _st: &AppState, _f: &mut Frame) {}
}

/// The screen the app starts on.
pub fn initial() -> impl Screen {
    Blank
}

#[cfg(test)]
pub(crate) mod testutil {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::buffer::Buffer;

    pub fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    pub fn buffer_text(b: &Buffer) -> String {
        let mut s = String::new();
        for y in 0..b.area.height {
            for x in 0..b.area.width {
                s.push_str(b[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }
}
