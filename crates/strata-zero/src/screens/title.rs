//! The title screen: block-letter logo, animated menu.

use crate::anim::effects::typewriter;
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::render::Rgb;
use crate::screens::create::chooser::Chooser;
use crate::screens::files::Files;
use crate::screens::generating::Generating;
use crate::ui::logo::{self, Logo};
use crate::ui::menu::{Menu, MenuView};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::{Paragraph, Widget};

pub const TITLE: &str = "STRATA ZERO";
const SUBTITLE: &str = "h i g h   t e c h   ·   l o w   l i f e";

pub struct Title {
    menu: Menu,
    opened: u64,
}

impl Title {
    /// The current parameters with a seed nobody has played: the tick and the clock.
    pub fn fresh_params(st: &AppState, tick: u64) -> citygen::GenParams {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        // Capped like Configure's RANDOM SEED so the status line stays readable.
        let mut seed = citygen::rng::hash64(tick, nanos) % 1_000_000_007;
        if seed == 0 {
            seed = 1;
        }
        st.params.clone().with_seed(seed)
    }

    /// NEW GAME (DEBUG): D'SHAWN, maxed out, straight into a fresh city.
    fn debug_game(&self, st: &mut AppState) -> ScreenCmd {
        st.character = game::Character::debug_runner(&st.content);
        match Generating::new(Title::fresh_params(st, st.tick), st.tick) {
            Ok(g) => ScreenCmd::Switch {
                to: Box::new(g),
                fx: TransitionKind::Glitch,
            },
            Err(e) => {
                st.warn(e.to_string());
                ScreenCmd::Stay
            }
        }
    }

    pub fn new(tick: u64) -> Self {
        Title {
            menu: Menu::new(
                vec![
                    "NEW GAME".into(),
                    "NEW GAME (DEBUG)".into(),
                    "LOAD GAME".into(),
                    "QUIT".into(),
                ],
                tick + 20,
            ),
            opened: tick,
        }
    }
}

impl Screen for Title {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        for k in keys {
            match k.code {
                KeyCode::Up | KeyCode::Char('k') => self.menu.up(),
                KeyCode::Down | KeyCode::Char('j') => self.menu.down(),
                KeyCode::Char('q') | KeyCode::Esc => return ScreenCmd::Quit,
                KeyCode::Enter => {
                    return match self.menu.selected {
                        0 => ScreenCmd::Push {
                            to: Box::new(Chooser::new(st, st.tick)),
                            fx: TransitionKind::SlideUp,
                        },
                        1 => self.debug_game(st),
                        2 => ScreenCmd::Push {
                            to: Box::new(Files::load(st.tick)),
                            fx: TransitionKind::SlideUp,
                        },
                        _ => ScreenCmd::Quit,
                    };
                }
                _ => {}
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let w = logo::width(TITLE);
        let x = area.x + area.width.saturating_sub(w) / 2;
        let y = area.y + area.height / 6;
        Logo {
            text: TITLE,
            tick: st.tick,
            accent: Rgb(255, 0, 200),
            steady: false,
        }
        .render(
            Rect::new(x, y, w.min(area.width), logo::ROWS),
            f.buffer_mut(),
        );

        let p = ((st.tick.saturating_sub(self.opened)) as f32 / 30.0).clamp(0.0, 1.0);
        let sub = typewriter(SUBTITLE, p);
        let sx = area.x + area.width.saturating_sub(SUBTITLE.chars().count() as u16) / 2;
        f.render_widget(
            Paragraph::new(Span::styled(
                sub,
                Style::default().fg(Color::Rgb(0, 229, 255)),
            )),
            Rect::new(
                sx,
                y + logo::ROWS + 1,
                area.width.saturating_sub(sx - area.x),
                1,
            ),
        );

        let menu_y = y + logo::ROWS + 4;
        let menu_x = area.x + area.width / 2 - 6;
        MenuView {
            menu: &self.menu,
            tick: st.tick,
            accent: Rgb(0, 255, 230),
        }
        .render(
            Rect::new(
                menu_x,
                menu_y,
                area.width.saturating_sub(menu_x - area.x),
                self.menu.items.len() as u16,
            ),
            f.buffer_mut(),
        );

        let hint = "↑↓ select · enter confirm · q quit";
        let hx = area.x + area.width.saturating_sub(hint.chars().count() as u16) / 2;
        f.render_widget(
            Paragraph::new(Span::styled(
                hint,
                Style::default().fg(Color::Rgb(100, 110, 125)),
            )),
            Rect::new(
                hx,
                area.bottom() - 2,
                area.width.saturating_sub(hx - area.x),
                1,
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, Screen, ScreenCmd};
    use crate::screens::testutil::{buffer_text, key};
    use crossterm::event::KeyCode;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn title_logo_fits_an_80_column_terminal() {
        assert!(logo::width(TITLE) <= 80);
    }

    #[test]
    fn title_renders_logo_and_menu_and_quits() {
        let mut st = AppState::new();
        let mut title = Title::new(0);
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        // The menu types itself in: by tick 120 all four items stand.
        st.tick = 120;
        t.draw(|f| title.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(
            text.contains("NEW GAME")
                && text.contains("NEW GAME (DEBUG)")
                && text.contains("LOAD GAME")
                && text.contains("QUIT"),
            "{text}"
        );
        assert!(text.contains('█'));
        st.tick = 600;
        t.draw(|f| title.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("h i g h   t e c h"), "{text}");
        assert!(!text.contains("g e n e r a t o r"), "{text}");
        assert!(matches!(
            title.update(&mut st, &[key(KeyCode::Char('q'))]),
            ScreenCmd::Quit
        ));
        title.update(
            &mut st,
            &[key(KeyCode::Down), key(KeyCode::Down), key(KeyCode::Down)],
        );
        assert!(matches!(
            title.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Quit
        ));
        title.update(
            &mut st,
            &[key(KeyCode::Up), key(KeyCode::Up), key(KeyCode::Up)],
        );
        assert!(matches!(
            title.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Push { .. }
        ));
        title.update(&mut st, &[key(KeyCode::Char('j')), key(KeyCode::Char('j'))]);
        assert!(
            matches!(
                title.update(&mut st, &[key(KeyCode::Enter)]),
                ScreenCmd::Push { .. }
            ),
            "load is pushed over the title"
        );
    }

    #[test]
    fn new_game_opens_the_chooser() {
        let mut st = AppState::new();
        let mut t = Title::new(0);
        assert!(
            matches!(
                t.update(&mut st, &[key(KeyCode::Enter)]),
                ScreenCmd::Push {
                    fx: TransitionKind::SlideUp,
                    ..
                }
            ),
            "new game opens the chooser"
        );
        let mut t = Title::new(0);
        t.update(&mut st, &[key(KeyCode::Down), key(KeyCode::Down)]);
        assert!(matches!(
            t.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Push { .. }
        ));
    }

    #[test]
    fn new_game_debug_starts_dshawn_maxed_out_in_a_fresh_city() {
        let mut st = AppState::new();
        let mut t = Title::new(0);
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        st.tick = 60;
        term.draw(|f| t.draw(&st, f)).unwrap();
        assert!(buffer_text(term.backend().buffer()).contains("NEW GAME (DEBUG)"));
        t.update(&mut st, &[key(KeyCode::Down)]);
        let ScreenCmd::Switch {
            to: mut generating,
            fx: TransitionKind::Glitch,
        } = t.update(&mut st, &[key(KeyCode::Enter)])
        else {
            panic!("straight to generating")
        };
        assert_eq!(st.character.handle, "D'SHAWN");
        assert_eq!(st.character.level, 50);
        assert!(st.character.flags(&st.content).contains("deep_end"));
        let ScreenCmd::Switch { .. } = generating.update(&mut st, &[key(KeyCode::Enter)]) else {
            panic!("ENTER skips to the city")
        };
        let p = st.player.as_ref().expect("walking in the new city");
        if let Some(h) = st.home {
            assert_eq!(
                p.place,
                citygen::Place::Floor {
                    building: h.home.building,
                    level: h.home.level
                },
                "a new game wakes up at home"
            );
        }
    }

    #[test]
    fn new_game_rolls_a_fresh_seed() {
        let st = AppState::new();
        let a = Title::fresh_params(&st, 10);
        let b = Title::fresh_params(&st, 11);
        assert_ne!(a.seed, 0);
        assert_ne!(a.seed, b.seed, "two new games are two cities");
        assert_eq!(a.size, st.params.size, "everything but the seed is kept");
        assert!(
            a.seed < 1_000_000_007 && b.seed < 1_000_000_007,
            "capped like Configure's RANDOM SEED so the HUD stays readable"
        );
    }
}
