//! The pause menu: resume, save, load, settings, quit; under the rows, when the run
//! was last saved.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::render::Rgb;
use crate::runs::when::save_now;
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

/// The line that says when the run was last saved.
const FAINT: Color = Color::Rgb(100, 110, 125);

/// A row of the pause menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    Resume,
    SaveGame,
    /// Dev mode: the city alone, to a `.city` file.
    SaveCity,
    LoadGame,
    Settings,
    QuitToTitle,
    QuitGame,
}

impl Row {
    /// The rows, SAVE CITY only in dev mode.
    fn all(dev: bool) -> Vec<Row> {
        let mut rows = vec![Row::Resume, Row::SaveGame];
        if dev {
            rows.push(Row::SaveCity);
        }
        rows.extend([
            Row::LoadGame,
            Row::Settings,
            Row::QuitToTitle,
            Row::QuitGame,
        ]);
        rows
    }

    fn label(self) -> &'static str {
        match self {
            Row::Resume => "RESUME",
            Row::SaveGame => "SAVE GAME",
            Row::SaveCity => "SAVE CITY",
            Row::LoadGame => "LOAD GAME",
            Row::Settings => "SETTINGS",
            Row::QuitToTitle => "QUIT TO TITLE",
            Row::QuitGame => "QUIT GAME",
        }
    }
}

pub struct Pause {
    menu: Menu,
    rows: Vec<Row>,
}

impl Pause {
    /// The pause menu over `st`'s game: SAVE CITY on it in dev mode.
    pub fn new(st: &AppState, tick: u64) -> Self {
        let rows = Row::all(st.dev);
        Pause {
            menu: Menu::new(rows.iter().map(|r| r.label().to_string()).collect(), tick),
            rows,
        }
    }

    /// What the row under the cursor does.
    fn choose(&self, st: &mut AppState) -> ScreenCmd {
        match self.rows[self.menu.selected] {
            Row::Resume => ScreenCmd::Pop {
                fx: TransitionKind::None,
            },
            Row::SaveGame => {
                if let Some(why) = crate::save::refusal(st) {
                    st.warn(why);
                } else {
                    save_now(st);
                }
                ScreenCmd::Stay
            }
            Row::SaveCity => {
                if let Some(why) = crate::save::refusal(st) {
                    st.warn(why);
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
            Row::LoadGame => {
                save_now(st);
                ScreenCmd::Push {
                    to: Box::new(Files::load(st.tick)),
                    fx: TransitionKind::None,
                }
            }
            Row::Settings => ScreenCmd::Push {
                to: Box::new(Settings::new()),
                fx: TransitionKind::None,
            },
            Row::QuitToTitle => {
                save_now(st);
                // The run is left: nothing on the title (the window closing) saves it
                // again with the time spent there.
                st.run = None;
                ScreenCmd::Switch {
                    to: Box::new(Title::new(st, st.tick)),
                    fx: TransitionKind::SlideUp,
                }
            }
            Row::QuitGame => {
                save_now(st);
                ScreenCmd::Quit
            }
        }
    }
}

/// How long ago a save was, `secs` by the wall clock: `just now` under a minute, then
/// `N min ago`, then `N h ago` from the hour.
fn ago(secs: u64) -> String {
    match secs / 60 {
        0 => "just now".to_string(),
        m if m < 60 => format!("{m} min ago"),
        m => format!("{} h ago", m / 60),
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
                    let cmd = self.choose(st);
                    if !matches!(cmd, ScreenCmd::Stay) {
                        return cmd;
                    }
                }
                _ => {}
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let rows = self.rows.len() as u16;
        // The rows, the line under them, a gap and the keys, in a box.
        let (w, h) = (36u16.min(area.width), (rows + 6).min(area.height));
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
            Rect::new(
                inner.x + 2,
                inner.y + 1,
                inner.width.saturating_sub(2),
                rows,
            ),
            f.buffer_mut(),
        );
        if let Some((_, at)) = st.last_save {
            let secs = std::time::SystemTime::now()
                .duration_since(at)
                .map_or(0, |d| d.as_secs());
            let y = inner.y + 1 + rows;
            if y < inner.bottom() {
                f.render_widget(
                    Paragraph::new(Span::styled(
                        format!("  last saved · {}", ago(secs)),
                        Style::default().fg(FAINT),
                    )),
                    Rect::new(inner.x + 1, y, inner.width.saturating_sub(1), 1),
                );
            }
        }
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
    use crate::runs::testutil::{Dir, saving, small_city};
    use crate::screens::testutil::{buffer_text, key};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::time::{Duration, SystemTime};

    /// Moves the cursor onto the row labelled `label`.
    fn pick(p: &mut Pause, st: &mut AppState, label: &str) {
        let at = p
            .menu
            .items
            .iter()
            .position(|r| r == label)
            .unwrap_or_else(|| panic!("no {label} row"));
        while p.menu.selected != at {
            p.update(st, &[key(KeyCode::Down)]);
        }
    }

    fn draw(p: &Pause, st: &AppState) -> ratatui::buffer::Buffer {
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| p.draw(st, f)).unwrap();
        t.backend().buffer().clone()
    }

    #[test]
    fn pause_menu_is_opaque() {
        let st = AppState::new();
        let p = Pause::new(&st, 0);
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
        let mut p = Pause::new(&st, 0);
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
        let mut p = Pause::new(&st, 0);
        st.tick = 60;
        let text = buffer_text(&draw(&p, &st));
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
            matches!(p.update(&mut st, &[key(KeyCode::Enter)]), ScreenCmd::Stay),
            "save: on the spot, the menu stays"
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

    #[test]
    fn dev_mode_adds_save_city() {
        let mut st = AppState::new();
        st.unlock_dev();
        st.enter_city(crate::runs::testutil::small_city());
        let mut p = Pause::new(&st, 0);
        assert_eq!(
            p.menu.items,
            [
                "RESUME",
                "SAVE GAME",
                "SAVE CITY",
                "LOAD GAME",
                "SETTINGS",
                "QUIT TO TITLE",
                "QUIT GAME"
            ]
        );
        st.tick = 60;
        let text = buffer_text(&draw(&p, &st));
        assert!(
            text.contains("SAVE CITY") && text.contains("QUIT GAME"),
            "{text}"
        );
        pick(&mut p, &mut st, "SAVE CITY");
        assert!(
            matches!(
                p.update(&mut st, &[key(KeyCode::Enter)]),
                ScreenCmd::Push { .. }
            ),
            "the city's own save"
        );
        st.enter_city(citygen::City::lot(1));
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
    fn pause_quits_save_first() {
        let dir = Dir::new("pause");
        let (mut st, _map) = saving(&dir, small_city());
        let saved_at = |st: &AppState| st.last_save.map(|(t, _)| t);
        // SAVE GAME: on the spot, and the menu stays.
        let mut p = Pause::new(&st, st.tick);
        pick(&mut p, &mut st, "SAVE GAME");
        st.tick += 1;
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Stay
        ));
        assert_eq!(saved_at(&st), Some(st.tick));
        let path = crate::runs::list(&dir.0)[0].path.clone();
        // LOAD GAME, QUIT GAME and QUIT TO TITLE each save first, then go on.
        for row in ["LOAD GAME", "QUIT GAME", "QUIT TO TITLE"] {
            let mut p = Pause::new(&st, st.tick);
            pick(&mut p, &mut st, row);
            st.tick += 1;
            let cmd = p.update(&mut st, &[key(KeyCode::Enter)]);
            let went = match row {
                "LOAD GAME" => matches!(cmd, ScreenCmd::Push { .. }),
                "QUIT TO TITLE" => matches!(cmd, ScreenCmd::Switch { .. }),
                _ => matches!(cmd, ScreenCmd::Quit),
            };
            assert!(went, "{row} goes on");
            assert_eq!(saved_at(&st), Some(st.tick), "{row} saved first");
        }
        assert_eq!(crate::runs::list(&dir.0).len(), 1, "one run, saved over");
        assert!(crate::runs::file::header(&path).is_ok());
        // On the title the run is left: the window closing there saves nothing.
        assert_eq!(st.run, None);
        st.tick += 1;
        assert!(!save_now(&mut st));
        assert_eq!(saved_at(&st), Some(st.tick - 1));
    }

    #[test]
    fn pause_shows_when_it_last_saved() {
        let mut st = AppState::new();
        let p = Pause::new(&st, 0);
        st.tick = 60;
        let line = |st: &AppState| {
            let buf = draw(&p, st);
            let text = buffer_text(&buf);
            text.lines()
                .enumerate()
                .find(|(_, l)| l.contains("last saved"))
                .map(|(y, l)| (buf.clone(), y as u16, l.to_string()))
        };
        assert!(line(&st).is_none(), "no line before a save");
        st.last_save = Some((10, SystemTime::now() - Duration::from_secs(120)));
        let (buf, y, row) = line(&st).expect("the line");
        assert!(row.contains("  last saved · 2 min ago"), "{row:?}");
        // Under the rows, over the keys, inside the panel.
        let (x0, y0) = ((100 - 36) / 2, (30 - 12) / 2);
        assert_eq!(y, y0 + 1 + 1 + 6, "right under the six rows");
        let x = row[..row.find("last").unwrap()].chars().count() as u16;
        assert_eq!(x, x0 + 4, "{row:?}");
        assert_eq!(buf[(x, y)].fg, FAINT);
        assert!(buffer_text(&buf).contains("enter confirm"));
    }

    #[test]
    fn a_save_reads_its_age_by_the_wall_clock() {
        assert_eq!(ago(0), "just now");
        assert_eq!(ago(59), "just now");
        assert_eq!(ago(60), "1 min ago");
        assert_eq!(ago(119), "1 min ago");
        assert_eq!(ago(120), "2 min ago");
        assert_eq!(ago(59 * 60 + 59), "59 min ago");
        assert_eq!(ago(60 * 60), "1 h ago");
        assert_eq!(ago(3 * 3600 + 1800), "3 h ago");
    }
}
