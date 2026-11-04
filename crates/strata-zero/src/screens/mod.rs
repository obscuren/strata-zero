//! Screens: one module per screen, plus shared test helpers.

pub mod atlas;
pub mod bench;
pub mod blueprint;
pub mod broadcast;
pub mod configure;
pub mod console;
pub mod create;
pub mod cyberdeck;
pub mod deck;
pub mod explore;
pub mod files;
pub mod flatlined;
pub mod help;
pub mod jobs_tab;
pub mod keypad;
pub mod kit;
pub mod messages;
pub mod outside;
pub mod pause;
pub mod popup;
pub mod rest;
pub mod runs;
pub mod settings;
pub mod sheet;
pub mod splash;
pub mod stickers;
pub mod terminal;
pub mod title;
pub mod workbench;

use crate::app::{Screen, ScreenCmd, TransitionKind};

/// The screen the app starts on: the splash, which hands over to the title.
pub fn initial(st: &crate::app::AppState) -> Box<dyn Screen> {
    Box::new(splash::Splash::at_launch(st.tick))
}

/// A city built at once from `params`.
pub fn build_city(params: citygen::GenParams) -> anyhow::Result<citygen::City> {
    Ok(citygen::Generator::run(params)?)
}

/// Puts a built city in place and drops onto its street.
pub fn enter_built_city(st: &mut crate::app::AppState, city: citygen::City) -> ScreenCmd {
    st.params = city.params.clone();
    st.enter_city(city);
    ScreenCmd::Switch {
        to: explore_screen(st),
        fx: TransitionKind::Glitch,
    }
}

/// Builds a city from `params` and drops onto its street; a city that fails to build
/// is a warning and nothing more.
pub fn start_city(st: &mut crate::app::AppState, params: citygen::GenParams) -> ScreenCmd {
    match build_city(params) {
        Ok(city) => enter_built_city(st, city),
        Err(e) => {
            st.warn(e.to_string());
            ScreenCmd::Stay
        }
    }
}

/// The screen shown once a city exists: the map, on foot, the story begun. Every new
/// game, city load and debug start comes through here; an opened run goes through
/// `resume_screen`.
pub fn explore_screen(st: &mut crate::app::AppState) -> Box<dyn Screen> {
    let map = explore::MapScreen::walking(st);
    crate::missions::begin(st);
    Box::new(map)
}

/// The map over a run opened from its file (CONTINUE, LOAD GAME): you where you stood,
/// the story where it was, nothing started over.
pub fn resume_screen(st: &mut crate::app::AppState) -> Box<dyn Screen> {
    let outside = st
        .outside
        .as_ref()
        .is_some_and(|o| o.at != crate::outside::At::City);
    if outside {
        Box::new(outside::OutsideScreen::new(st))
    } else {
        Box::new(explore::MapScreen::resumed(st))
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{AppState, ScreenCmd};

    #[test]
    fn the_game_opens_on_the_splash() {
        let mut st = AppState::new();
        let mut first = super::initial(&st);
        for tick in 1..super::splash::HAND_OVER {
            st.tick = tick;
            assert!(
                matches!(first.update(&mut st, &[]), ScreenCmd::Stay),
                "{tick}"
            );
        }
        st.tick = super::splash::HAND_OVER;
        assert!(matches!(
            first.update(&mut st, &[]),
            ScreenCmd::Switch { .. }
        ));
    }
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
pub(crate) mod shots;
