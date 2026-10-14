//! The title screen: block-letter logo, animated menu.

use crate::anim::effects::typewriter;
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::render::Rgb;
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

pub const TITLE: &str = "CYBERPUNK";
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

    pub fn new(tick: u64) -> Self {
        Title {
            menu: Menu::new(
                vec!["NEW GAME".into(), "LOAD GAME".into(), "QUIT".into()],
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
                        0 => match Generating::new(Self::fresh_params(st, st.tick), st.tick) {
                            Ok(next) => ScreenCmd::Switch {
                                to: Box::new(next),
                                fx: TransitionKind::Glitch,
                            },
                            Err(e) => {
                                st.warn(e.to_string());
                                ScreenCmd::Stay
                            }
                        },
                        1 => ScreenCmd::Push {
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
                3,
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
    fn title_renders_logo_and_menu_and_quits() {
        let mut st = AppState::new();
        let mut title = Title::new(0);
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        st.tick = 60;
        t.draw(|f| title.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(
            text.contains("NEW GAME") && text.contains("LOAD GAME") && text.contains("QUIT"),
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
        title.update(&mut st, &[key(KeyCode::Down), key(KeyCode::Down)]);
        assert!(matches!(
            title.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Quit
        ));
        title.update(&mut st, &[key(KeyCode::Up), key(KeyCode::Up)]);
        assert!(matches!(
            title.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Switch { .. }
        ));
        title.update(&mut st, &[key(KeyCode::Char('j'))]);
        assert!(
            matches!(
                title.update(&mut st, &[key(KeyCode::Enter)]),
                ScreenCmd::Push { .. }
            ),
            "load is pushed over the title"
        );
    }

    #[test]
    fn new_game_generates_at_once() {
        let mut st = AppState::new();
        let mut t = Title::new(0);
        assert!(
            matches!(
                t.update(&mut st, &[key(KeyCode::Enter)]),
                ScreenCmd::Switch {
                    fx: TransitionKind::Glitch,
                    ..
                }
            ),
            "new game goes straight to generation"
        );
        let mut t = Title::new(0);
        t.update(&mut st, &[key(KeyCode::Down)]);
        assert!(matches!(
            t.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Push { .. }
        ));
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
