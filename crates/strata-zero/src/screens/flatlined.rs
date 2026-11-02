//! FLATLINED: the first death in the game. Black ICE took the meat through the
//! jack; the screen reads the cause and the runner, and any key goes to the title.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::render::Rgb;
use crate::screens::title::Title;
use crate::ui::chrome::{DIM, TEXT, put, solid};
use crate::ui::logo::{self, Logo};
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;

const RED: Rgb = Rgb(255, 40, 40);

pub struct Flatlined {
    cause: String,
    runner: String,
    opened: u64,
}

impl Flatlined {
    pub fn new(cause: String, handle: &str, level: u8, tick: u64) -> Self {
        Flatlined {
            cause,
            runner: format!("{} · level {level}", handle.to_uppercase()),
            opened: tick,
        }
    }
}

impl Screen for Flatlined {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        // A breath before any key counts, so the hit that killed does not skip the screen.
        if keys.is_empty() || st.tick < self.opened + 10 {
            return ScreenCmd::Stay;
        }
        // The runner is gone with the session they ran: a fresh one stands ready.
        st.net = Default::default();
        st.constructs = Default::default();
        if let Some(fresh) =
            game::Character::from_template(&st.content, &game::TemplateId::new("vex"))
        {
            st.character = fresh;
        }
        st.perma_death = true;
        st.tutorial_off = false;
        st.player = None;
        ScreenCmd::Switch {
            to: Box::new(Title::new(st, st.tick)),
            fx: TransitionKind::Glitch,
        }
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let buf = f.buffer_mut();
        solid(buf, area);
        let right = area.right();
        let word = "FLATLINED";
        let mut y = area.y + area.height / 3;
        let w = logo::width(word);
        if area.height >= 16 && w <= area.width {
            Logo {
                text: word,
                tick: st.tick,
                accent: RED,
                steady: false,
            }
            .render(
                Rect::new(area.x + (area.width - w) / 2, y, w, logo::ROWS),
                buf,
            );
            y += logo::ROWS + 1;
        } else {
            let x = area.x + area.width.saturating_sub(word.len() as u16) / 2;
            put(
                buf,
                x,
                y,
                word,
                Style::default()
                    .fg(RED.to_color())
                    .add_modifier(Modifier::BOLD),
                right,
            );
            y += 2;
        }
        let centred = |buf: &mut ratatui::buffer::Buffer, y: u16, text: &str, style: Style| {
            let x = area.x + area.width.saturating_sub(text.chars().count() as u16) / 2;
            put(buf, x, y, text, style, right);
        };
        centred(buf, y, &self.cause, Style::default().fg(TEXT));
        centred(buf, y + 1, &self.runner, Style::default().fg(DIM));
        centred(buf, y + 3, "any key · title", Style::default().fg(DIM));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::testutil::key;
    use crossterm::event::KeyCode;

    #[test]
    fn a_flatline_resets_the_flags() {
        let mut st = AppState::new();
        st.perma_death = false;
        st.tutorial_off = true;
        let mut f = Flatlined::new("black ICE".into(), "Nyx", 3, 0);
        st.tick = 20;
        assert!(matches!(
            f.update(&mut st, &[key(KeyCode::Char(' '))]),
            ScreenCmd::Switch { .. }
        ));
        assert!(st.perma_death && !st.tutorial_off);
    }
}
