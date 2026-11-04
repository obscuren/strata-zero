//! The pause menu: resume, save, load, settings, quit; under the rows, when the run
//! was last saved, or why the save it just tried failed.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::render::Rgb;
use crate::runs::when::{save_leaving, try_save};
use crate::screens::files::Files;
use crate::screens::runs::RunsScreen;
use crate::screens::settings::Settings;
use crate::screens::title::Title;
use crate::ui::chrome::{MAGENTA, solid};
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
    /// The last save this menu tried failed: on which row, and why. The line under the
    /// rows says so; the same row pressed again goes on without the save.
    failed: Option<(Row, String)>,
}

impl Pause {
    /// The pause menu over `st`'s game: SAVE CITY on it in dev mode.
    pub fn new(st: &AppState, tick: u64) -> Self {
        let rows = Row::all(st.dev);
        Pause {
            menu: Menu::new(rows.iter().map(|r| r.label().to_string()).collect(), tick),
            rows,
            failed: None,
        }
    }

    /// The save a way out of the run takes before it goes on (`save_leaving`: a perma
    /// death runner killed this frame is marked flatlined instead): true to go on. A
    /// save that fails keeps the menu up, saying why; pressed again, the row goes on
    /// without it.
    fn saved_first(&mut self, st: &mut AppState, row: Row) -> bool {
        if self.failed.as_ref().is_some_and(|(r, _)| *r == row) {
            return true;
        }
        let tried = save_leaving(st);
        self.took(row, tried)
    }

    /// What a save `row` tried came to, kept for the line: false when the write failed.
    fn took(&mut self, row: Row, tried: Option<Result<(), String>>) -> bool {
        match tried {
            Some(Err(why)) => {
                self.failed = Some((row, why));
                false
            }
            Some(Ok(())) => {
                self.failed = None;
                true
            }
            None => true,
        }
    }

    /// What the row under the cursor does.
    fn choose(&mut self, st: &mut AppState) -> ScreenCmd {
        let row = self.rows[self.menu.selected];
        match row {
            Row::Resume => ScreenCmd::Pop {
                fx: TransitionKind::None,
            },
            Row::SaveGame => {
                if let Some(why) = crate::save::refusal(st) {
                    st.warn(why);
                } else {
                    let tried = try_save(st);
                    self.took(row, tried);
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
            // The ways out of the run save it first (LOAD GAME, so it is in the list);
            // one whose save fails stays, saying why.
            Row::LoadGame | Row::QuitToTitle | Row::QuitGame if !self.saved_first(st, row) => {
                ScreenCmd::Stay
            }
            Row::LoadGame => ScreenCmd::Push {
                to: Box::new(RunsScreen::load(st)),
                fx: TransitionKind::None,
            },
            Row::Settings => ScreenCmd::Push {
                to: Box::new(Settings::new()),
                fx: TransitionKind::None,
            },
            Row::QuitToTitle => {
                // The run is left: nothing on the title (the window closing) saves it
                // again with the time spent there.
                st.run = None;
                ScreenCmd::Switch {
                    to: Box::new(Title::new(st, st.tick)),
                    fx: TransitionKind::SlideUp,
                }
            }
            Row::QuitGame => ScreenCmd::Quit,
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
        // Under the rows: why the last save this menu tried failed, else when it last
        // saved.
        let line = match (&self.failed, st.last_save) {
            (Some((_, why)), _) => Some((format!("  couldn't save · {why}"), MAGENTA)),
            (None, Some((_, at))) => {
                let secs = std::time::SystemTime::now()
                    .duration_since(at)
                    .map_or(0, |d| d.as_secs());
                Some((format!("  last saved · {}", ago(secs)), FAINT))
            }
            (None, None) => None,
        };
        let y = inner.y + 1 + rows;
        if let Some((text, colour)) = line
            && y < inner.bottom()
        {
            f.render_widget(
                Paragraph::new(Span::styled(text, Style::default().fg(colour))),
                Rect::new(inner.x + 1, y, inner.width.saturating_sub(1), 1),
            );
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
    use crate::runs::when::save_now;
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
            let went = match (row, cmd) {
                ("LOAD GAME", ScreenCmd::Push { to, .. }) => {
                    // The list of runs, the one just saved in it.
                    let text = buffer_text(&crate::screens::shots::draw_stack(
                        &st,
                        &[to.as_ref()],
                        (120, 40),
                    ));
                    assert!(text.contains("space continue"), "{text}");
                    assert!(text.contains(&st.character.handle.to_uppercase()), "{text}");
                    true
                }
                ("QUIT TO TITLE", cmd) => matches!(cmd, ScreenCmd::Switch { .. }),
                ("QUIT GAME", cmd) => matches!(cmd, ScreenCmd::Quit),
                _ => false,
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

    /// The row the panel's line is on, under the six rows, and its text.
    fn status_row(p: &Pause, st: &AppState) -> (ratatui::buffer::Buffer, u16, String) {
        let buf = draw(p, st);
        let y = (30 - 12) / 2 + 1 + 1 + 6;
        let row = buffer_text(&buf)
            .lines()
            .nth(y as usize)
            .unwrap()
            .to_string();
        (buf, y, row)
    }

    #[test]
    fn a_failed_save_on_the_way_out_stays_and_says_why() {
        let dir = Dir::new("pause-failed");
        let (mut st, _map) = saving(&dir, small_city());
        assert!(save_now(&mut st));
        let path = crate::runs::list(&dir.0)[0].path.clone();
        let before = std::fs::read(&path).unwrap();
        // The folder to save in is a file now: every save fails.
        let blocked = dir.0.join("not-a-folder");
        std::fs::write(&blocked, b"a file").unwrap();
        st.saves = Some(blocked.clone());
        let os = std::fs::create_dir_all(&blocked).unwrap_err().to_string();
        let run = st.run;
        // QUIT TO TITLE: the menu stays, the line says why in place of `last saved`.
        let mut p = Pause::new(&st, st.tick);
        pick(&mut p, &mut st, "QUIT TO TITLE");
        st.tick += 1;
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Stay
        ));
        assert_eq!(st.run, run, "still in the run");
        let (buf, y, row) = status_row(&p, &st);
        let want = format!("couldn't save · {os}");
        assert!(row.contains(&format!("  {}", &want[..20])), "{row:?}");
        assert!(!row.contains("last saved"), "{row:?}");
        let x = row[..row.find("couldn't").unwrap()].chars().count() as u16;
        assert_eq!(
            buf[(x, y)].fg,
            crate::ui::chrome::MAGENTA,
            "the warning's colour"
        );
        // The second press goes on, without saving.
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Switch { .. }
        ));
        assert_eq!(st.run, None, "the run is left");
        assert_eq!(std::fs::read(&path).unwrap(), before, "the old save stands");
        // LOAD GAME and QUIT GAME the same; SAVE GAME tries every press and says so.
        st.run = run;
        for (row, went) in [("LOAD GAME", "a push"), ("QUIT GAME", "a quit")] {
            let mut p = Pause::new(&st, st.tick);
            pick(&mut p, &mut st, row);
            let cmd = p.update(&mut st, &[key(KeyCode::Enter)]);
            assert!(matches!(cmd, ScreenCmd::Stay), "{row} stays");
            assert!(status_row(&p, &st).2.contains("couldn't save · "), "{row}");
            let cmd = p.update(&mut st, &[key(KeyCode::Enter)]);
            let ok = matches!(
                (row, cmd),
                ("LOAD GAME", ScreenCmd::Push { .. }) | ("QUIT GAME", ScreenCmd::Quit)
            );
            assert!(ok, "{row}: then {went}");
        }
        let mut p = Pause::new(&st, st.tick);
        pick(&mut p, &mut st, "SAVE GAME");
        for _ in 0..2 {
            p.update(&mut st, &[key(KeyCode::Enter)]);
            assert!(status_row(&p, &st).2.contains("couldn't save · "));
        }
        // A save that goes puts `last saved` back.
        st.saves = Some(dir.0.clone());
        p.update(&mut st, &[key(KeyCode::Enter)]);
        let row = status_row(&p, &st).2;
        assert!(row.contains("last saved · just now"), "{row:?}");
    }

    /// Killed by a key, then the pause menu or the window's close in the same frame,
    /// before the map's next frame puts up FLATLINED: a perma death run is marked
    /// flatlined on the way out, not left living on its save from before.
    #[test]
    fn a_perma_run_left_the_frame_it_dies_is_flatlined() {
        use crate::app::Runner;
        let cause = "black ICE at Kiroflux Mart";
        for way in ["QUIT TO TITLE", "QUIT GAME", "LOAD GAME", "the window"] {
            let dir = Dir::new("dies-leaving");
            let (mut st, map) = saving(&dir, small_city());
            st.perma_death = true;
            assert!(save_now(&mut st));
            st.net.death = Some(cause.to_string());
            if way == "the window" {
                let mut runner = Runner::new(Box::new(map), st);
                runner.save_on_exit();
                assert_eq!(runner.st.run, None, "{way}: the run is over");
            } else {
                let mut p = Pause::new(&st, st.tick);
                pick(&mut p, &mut st, way);
                let cmd = p.update(&mut st, &[key(KeyCode::Enter)]);
                assert!(!matches!(cmd, ScreenCmd::Stay), "{way} goes on");
                assert_eq!(st.run, None, "{way}: the run is over");
            }
            assert!(crate::runs::living(&dir.0).is_empty(), "{way}: not living");
            let dead = crate::runs::dead(&dir.0);
            assert_eq!(dead.len(), 1, "{way}: in the HALL OF FAME");
            let h = dead[0].header.as_ref().unwrap();
            assert_eq!(h.death.as_ref().map(|d| d.cause.as_str()), Some(cause));
        }
        // Perma death off: nothing is marked, the last save stands to go back to.
        let dir = Dir::new("dies-leaving-off");
        let (mut st, _) = saving(&dir, small_city());
        st.perma_death = false;
        assert!(save_now(&mut st));
        let path = crate::runs::list(&dir.0)[0].path.clone();
        let before = std::fs::read(&path).unwrap();
        st.net.death = Some(cause.to_string());
        let mut p = Pause::new(&st, st.tick);
        pick(&mut p, &mut st, "QUIT GAME");
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Quit
        ));
        assert_eq!(std::fs::read(&path).unwrap(), before);
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
