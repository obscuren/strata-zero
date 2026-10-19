//! Screens: one module per screen, plus shared test helpers.

pub mod bench;
pub mod blueprint;
pub mod configure;
pub mod console;
pub mod create;
pub mod cyberdeck;
pub mod deck;
pub mod explore;
pub mod files;
pub mod flatlined;
pub mod generating;
pub mod help;
pub mod keypad;
pub mod kit;
pub mod messages;
pub mod pause;
pub mod popup;
pub mod settings;
pub mod stickers;
pub mod terminal;
pub mod title;

use crate::app::Screen;

/// The screen the app starts on.
pub fn initial() -> impl Screen {
    title::Title::new(0)
}

/// The screen shown once a city exists: the map, on foot.
pub fn explore_screen(st: &mut crate::app::AppState) -> Box<dyn Screen> {
    Box::new(explore::MapScreen::walking(st))
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

#[cfg(test)]
mod shots;
