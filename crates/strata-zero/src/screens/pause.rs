//! The pause menu: resume, save, load, settings, quit.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::render::Rgb;
use crate::screens::files::Files;
use crate::screens::settings::Settings;
use crate::screens::title::Title;
use crate::ui::chrome::solid;
use crate::ui::menu::{Menu, MenuView};
use crate::ui::panel::boxed;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::{Paragraph, Widget};

const ITEMS: [&str; 6] = [
    "RESUME",
    "SAVE GAME",
    "LOAD GAME",
    "SETTINGS",
    "QUIT TO TITLE",
    "QUIT GAME",
];

pub struct Pause {
    menu: Menu,
}

impl Pause {
    pub fn new(tick: u64) -> Self {
        Pause {
            menu: Menu::new(ITEMS.iter().map(|s| s.to_string()).collect(), tick),
        }
    }
}

impl Screen for Pause {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        for k in keys {
            match k.code {
                KeyCode::Up | KeyCode::Char('k') => self.menu.up(),
                KeyCode::Down | KeyCode::Char('j') => self.menu.down(),
                KeyCode::Esc | KeyCode::Char('q') => {
                    return ScreenCmd::Pop {
                        fx: TransitionKind::None,
                    };
                }
                KeyCode::Enter => {
                    return match self.menu.selected {
                        0 => ScreenCmd::Pop {
                            fx: TransitionKind::None,
                        },
                        1 => {
                            if st.city.as_ref().is_some_and(|c| c.is_lot()) {
                                st.warn("nothing to save on the lot");
                                return ScreenCmd::Stay;
                            }
                            let name = st
                                .city
                                .as_ref()
                                .map(|c| c.name.clone())
                                .unwrap_or_else(|| "city".to_string());
                            ScreenCmd::Push {
                                to: Box::new(Files::save(st.tick, &name)),
                                fx: TransitionKind::None,
                            }
                        }
                        2 => ScreenCmd::Push {
                            to: Box::new(Files::load(st.tick)),
                            fx: TransitionKind::None,
                        },
                        3 => ScreenCmd::Push {
                            to: Box::new(Settings::new()),
                            fx: TransitionKind::None,
                        },
                        4 => ScreenCmd::Switch {
                            to: Box::new(Title::new(st.tick)),
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
        let (w, h) = (36u16.min(area.width), 12u16.min(area.height));
        let panel = Rect::new(
            area.x + (area.width - w) / 2,
            area.y + (area.height - h) / 2,
            w,
            h,
        );
        solid(f.buffer_mut(), panel);
        let block = boxed("PAUSED");
        let inner = block.inner(panel);
        f.render_widget(block, panel);
        MenuView {
            menu: &self.menu,
            tick: st.tick,
            accent: Rgb(0, 255, 230),
        }
        .render(
            Rect::new(inner.x + 2, inner.y + 1, inner.width.saturating_sub(2), 6),
            f.buffer_mut(),
        );
        f.render_widget(
            Paragraph::new(Span::styled(
                " ↑↓ select · enter confirm · esc resume",
                Style::default().fg(Color::Rgb(160, 170, 185)),
            )),
            Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::testutil::{buffer_text, key};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn pause_menu_is_opaque() {
        let st = AppState::new();
        let p = Pause::new(0);
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| {
            let area = f.area();
            for y in area.y..area.bottom() {
                for x in area.x..area.right() {
                    f.buffer_mut()[(x, y)].set_char('x');
                }
            }
            p.draw(&st, f);
        })
        .unwrap();
        let buf = t.backend().buffer();
        // The panel is 36x12 centred: nothing from beneath shows inside it.
        let (x0, y0) = ((100 - 36) / 2, (30 - 12) / 2);
        for y in y0..y0 + 12 {
            for x in x0..x0 + 36 {
                assert_ne!(buf[(x, y)].symbol(), "x", "({x},{y}) shows through");
            }
        }
        assert_eq!(buf[(x0 + 1, y0 + 1)].bg, crate::ui::chrome::PANEL_BG);
        assert_eq!(buf[(x0 - 1, y0)].symbol(), "x");
    }

    #[test]
    fn save_is_refused_on_the_lot() {
        let mut st = AppState::new();
        st.enter_city(citygen::City::lot(1));
        let mut p = Pause::new(0);
        p.update(&mut st, &[key(KeyCode::Down)]);
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Stay
        ));
        assert_eq!(
            st.log.latest().map(|e| e.text.as_str()),
            Some("nothing to save on the lot")
        );
    }

    #[test]
    fn pause_menu_routes_every_item() {
        let mut st = AppState::new();
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut p = Pause::new(0);
        st.tick = 60;
        t.draw(|f| p.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(
            text.contains("PAUSED")
                && text.contains("RESUME")
                && text.contains("SAVE GAME")
                && text.contains("LOAD GAME")
                && text.contains("QUIT GAME")
                && !text.contains("CITY"),
            "{text}"
        );
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Pop { .. }
        ));
        let down = |p: &mut Pause, st: &mut AppState| {
            p.update(st, &[key(KeyCode::Down)]);
        };
        down(&mut p, &mut st);
        assert!(
            matches!(
                p.update(&mut st, &[key(KeyCode::Enter)]),
                ScreenCmd::Push { .. }
            ),
            "save"
        );
        down(&mut p, &mut st);
        assert!(
            matches!(
                p.update(&mut st, &[key(KeyCode::Enter)]),
                ScreenCmd::Push { .. }
            ),
            "load"
        );
        down(&mut p, &mut st);
        assert!(
            matches!(
                p.update(&mut st, &[key(KeyCode::Enter)]),
                ScreenCmd::Push { .. }
            ),
            "settings"
        );
        down(&mut p, &mut st);
        assert!(
            matches!(
                p.update(&mut st, &[key(KeyCode::Enter)]),
                ScreenCmd::Switch { .. }
            ),
            "title"
        );
        down(&mut p, &mut st);
        assert!(
            matches!(p.update(&mut st, &[key(KeyCode::Enter)]), ScreenCmd::Quit),
            "quit"
        );
    }
}
