//! LOAD GAME and the HALL OF FAME: the runs in the saves folder, one row each, and a
//! card for the one under the cursor.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::runs::describe::{clock, credits, first_part, played, saved_at};
use crate::runs::{RunHeader, RunInfo};
use crate::screens::broadcast::Broadcast;
use crate::ui::chrome::{CYAN, DIM, MAGENTA, TEXT, put, solid};
use crate::ui::panel::boxed;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use std::path::Path;

/// The row under the cursor.
pub(crate) const PICK: Color = Color::Rgb(0, 204, 184);
/// Headers, keys, and a file that can't be opened.
pub(crate) const FAINT: Color = Color::Rgb(100, 110, 125);
/// The rules across the panel.
const RULE: Color = Color::Rgb(31, 43, 56);

/// The panel at its widest and tallest, as the files panel is; it closes under its keys.
const PANEL_W: u16 = 80;
const PANEL_H: u16 = 24;
/// The panel's rows that aren't runs: its edges, the header row, the rules, the card,
/// the keys and the gaps between them.
const FRAME_ROWS: u16 = 14;
/// The panel's height with nobody in it.
const EMPTY_H: u16 = 9;
/// Columns from the runner's: RUNNER, BUILD, LVL, CITY, CLOCK; THE STORY runs on to the
/// tag. A cell is cut a character short of its column.
const WIDTHS: [u16; 5] = [10, 11, 5, 13, 13];
const LOAD_HEADS: [&str; 6] = ["RUNNER", "BUILD", "LVL", "CITY", "CLOCK", "THE STORY"];
const TAG: &str = "PERMA";

/// Which list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// The runs still going, newest saved first; space opens one.
    Load,
    /// The runs that flatlined; nothing opens from here.
    Hall,
}

pub struct RunsScreen {
    mode: Mode,
    runs: Vec<RunInfo>,
    selected: usize,
}

/// Opens the run at `path` and plays its broadcast, over nothing: what CONTINUE and
/// LOAD GAME's space do. The run is in place of whatever was under them (a pause menu
/// over another run's map), so the broadcast takes the whole stack. Why not, when the
/// file won't open.
pub(crate) fn resume(st: &mut AppState, path: &Path) -> Result<ScreenCmd, String> {
    crate::runs::open(st, path).map_err(|e| e.to_string())?;
    Ok(ScreenCmd::Switch {
        to: Box::new(Broadcast::resume(st)),
        fx: TransitionKind::Glitch,
    })
}

/// A cell's text cut to `n` characters.
fn cut(text: &str, n: u16) -> String {
    text.chars().take(n as usize).collect()
}

fn bold(c: Color) -> Style {
    Style::default().fg(c).add_modifier(Modifier::BOLD)
}

fn fg(c: Color) -> Style {
    Style::default().fg(c)
}

/// `parts` one after another from `x`, stopping before `right`.
fn spans(buf: &mut Buffer, mut x: u16, y: u16, parts: &[(&str, Style)], right: u16) {
    for (text, style) in parts {
        put(buf, x, y, text, *style, right);
        x = x.saturating_add(text.chars().count() as u16);
    }
}

impl RunsScreen {
    /// LOAD GAME: the runs still going in `st`'s saves folder, none without one.
    pub fn load(st: &AppState) -> Self {
        RunsScreen {
            mode: Mode::Load,
            runs: st
                .saves
                .as_deref()
                .map(crate::runs::living)
                .unwrap_or_default(),
            selected: 0,
        }
    }

    /// The HALL OF FAME. Nobody is in it until a run can flatline into it.
    pub fn hall(_st: &AppState) -> Self {
        RunsScreen {
            mode: Mode::Hall,
            runs: Vec::new(),
            selected: 0,
        }
    }

    /// One row up or down, round the ends.
    fn step(&mut self, down: bool) {
        let n = self.runs.len();
        if n > 0 {
            self.selected = if down {
                (self.selected + 1) % n
            } else {
                (self.selected + n - 1) % n
            };
        }
    }

    /// Opens the run under the cursor; one that won't open turns dim with its reason.
    fn open(&mut self, st: &mut AppState) -> Option<ScreenCmd> {
        let run = self.runs.get_mut(self.selected)?;
        run.header.as_ref().ok()?;
        match resume(st, &run.path) {
            Ok(cmd) => Some(cmd),
            Err(why) => {
                run.header = Err(why);
                None
            }
        }
    }

    /// The title on the panel's edge, what it says empty, and its keys.
    fn words(&self) -> (&'static str, &'static str, &'static str) {
        match self.mode {
            Mode::Load => (
                " LOAD GAME ",
                "No runs yet.",
                "↑↓ select · space continue · esc back",
            ),
            Mode::Hall => (
                " HALL OF FAME ",
                "Nobody you ran has flatlined. Yet.",
                "↑↓ select · esc back",
            ),
        }
    }

    /// One run's row: the cursor, its columns, the tag of a perma death run; a file that
    /// can't be read says why, dim.
    fn draw_row(&self, buf: &mut Buffer, x0: u16, y: u16, i: usize, right: u16) {
        let picked = i == self.selected;
        put(
            buf,
            x0 + 1,
            y,
            if picked { "▸ " } else { "  " },
            bold(PICK),
            right,
        );
        let h = match &self.runs[i].header {
            Ok(h) => h,
            Err(why) => {
                put(buf, x0 + 3, y, why, fg(FAINT), right);
                return;
            }
        };
        let tag_x = right.saturating_sub(1 + TAG.len() as u16);
        let story = h.objective.as_deref().map(first_part).unwrap_or_default();
        let cells = [
            h.handle.to_uppercase(),
            h.profession.clone(),
            h.level.to_string(),
            h.city.clone(),
            clock(h.day, h.minute),
            story.to_string(),
        ];
        let mut x = x0 + 3;
        for (n, text) in cells.iter().enumerate() {
            let w = WIDTHS.get(n).copied().unwrap_or(tag_x.saturating_sub(x));
            let style = match (picked, n) {
                (true, 0) => bold(PICK),
                (true, _) => fg(PICK),
                (false, 0) => fg(TEXT),
                (false, _) => fg(DIM),
            };
            put(buf, x, y, &cut(text, w.saturating_sub(1)), style, right);
            x += w;
        }
        if h.perma {
            put(buf, tag_x, y, TAG, bold(MAGENTA), right);
        }
    }

    /// The card of the run under the cursor, four rows from `y`.
    fn draw_card(&self, buf: &mut Buffer, x: u16, y: u16, h: &RunHeader, right: u16) {
        let perma = "PERMA DEATH";
        let tag_x = right.saturating_sub(1 + perma.len() as u16);
        let build = format!(" · {} · level {}", h.profession, h.level);
        spans(
            buf,
            x,
            y,
            &[(&h.handle.to_uppercase(), bold(CYAN)), (&build, fg(TEXT))],
            if h.perma {
                tag_x.saturating_sub(1)
            } else {
                right
            },
        );
        if h.perma {
            put(buf, tag_x, y, perma, bold(MAGENTA), right);
        }
        let mut place = format!("{} · {}", h.city, clock(h.day, h.minute));
        if !h.place.is_empty() {
            place = format!("{place} · {}", h.place);
        }
        put(buf, x, y + 1, &place, fg(DIM), right);
        let objective = h.objective.as_deref().unwrap_or_default();
        match h.mission.as_deref() {
            Some(m) if !objective.is_empty() => spans(
                buf,
                x,
                y + 2,
                &[(m, fg(TEXT)), (&format!(" · {objective}"), fg(DIM))],
                right,
            ),
            Some(m) => put(buf, x, y + 2, m, fg(TEXT), right),
            None => put(buf, x, y + 2, objective, fg(DIM), right),
        }
        let talents = match h.talents {
            1 => "1 talent".to_string(),
            n => format!("{n} talents"),
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let stats = format!(
            "{} · {} chrome · {talents} · played {} · saved {}",
            credits(h.credits),
            h.chrome.len(),
            played(h.played),
            saved_at(h.saved, now)
        );
        put(buf, x, y + 3, &stats, fg(FAINT), right);
    }
}

impl Screen for RunsScreen {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        for k in keys {
            match k.code {
                KeyCode::Up | KeyCode::Char('k') => self.step(false),
                KeyCode::Down | KeyCode::Char('j') => self.step(true),
                KeyCode::Esc => {
                    return ScreenCmd::Pop {
                        fx: TransitionKind::None,
                    };
                }
                KeyCode::Char(' ') if self.mode == Mode::Load => {
                    if let Some(cmd) = self.open(st) {
                        return cmd;
                    }
                }
                _ => {}
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, _st: &AppState, f: &mut Frame) {
        let area = f.area();
        let w = area.width.min(PANEL_W);
        let tall = area.height.min(PANEL_H);
        let x0 = area.x + (area.width - w) / 2;
        let y0 = area.y + (area.height - tall) / 2;
        // Rows for runs; the rest scroll, the cursor's always among them.
        let room = tall.saturating_sub(FRAME_ROWS).max(1) as usize;
        let shown = self.runs.len().min(room);
        let first = (self.selected + 1).saturating_sub(shown);
        let h = if shown == 0 {
            EMPTY_H
        } else {
            FRAME_ROWS + shown as u16
        };
        let panel = Rect::new(x0, y0, w, h.min(tall));
        let (title, empty, keys) = self.words();
        solid(f.buffer_mut(), panel);
        f.render_widget(boxed(title), panel);
        let buf = f.buffer_mut();
        let right = panel.right().saturating_sub(1);
        let x = x0 + 3;
        if shown == 0 {
            let ex =
                x0 + 1 + (w.saturating_sub(2)).saturating_sub(empty.chars().count() as u16) / 2;
            put(buf, ex, y0 + 3, empty, fg(DIM), right);
            put(buf, x0 + 2, y0 + 6, "esc back", fg(FAINT), right);
            return;
        }
        let mut hx = x;
        for (name, w) in LOAD_HEADS.iter().zip(WIDTHS.iter().chain([&0])) {
            put(buf, hx, y0 + 1, name, fg(FAINT), right);
            hx += w;
        }
        for (row, i) in (first..first + shown).enumerate() {
            self.draw_row(buf, x0, y0 + 2 + row as u16, i, right);
        }
        let rule = "─".repeat(w.saturating_sub(2) as usize);
        let below = y0 + 2 + shown as u16;
        put(buf, x0 + 1, below + 1, &rule, fg(RULE), right);
        if let Some(Ok(h)) = self.runs.get(self.selected).map(|r| r.header.as_ref()) {
            self.draw_card(buf, x, below + 3, h, right);
        }
        put(buf, x0 + 1, below + 8, &rule, fg(RULE), right);
        put(buf, x0 + 2, below + 9, keys, fg(FAINT), right);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, Screen, ScreenCmd};
    use crate::runs::testutil::{Dir, saving, small_city};
    use crate::screens::shots::draw_stack;
    use crate::screens::testutil::{buffer_text, key};
    use crossterm::event::KeyCode;
    use ratatui::buffer::Buffer;
    use ratatui::style::{Color, Modifier};
    use std::path::PathBuf;

    const SPACE: KeyCode = KeyCode::Char(' ');

    /// A run saved in `dir` by `handle`, its header stamped as saved at `saved`.
    fn save(dir: &Dir, handle: &str, perma: bool, saved: u64) -> (AppState, PathBuf) {
        let (mut st, _) = saving(dir, small_city());
        st.character.handle = handle.into();
        st.perma_death = perma;
        st.tick = 3_000;
        assert!(crate::runs::when::save_now(&mut st));
        let path = dir.0.join(format!("{}.run", st.run.unwrap().id));
        let mut h = crate::runs::file::header(&path).unwrap();
        h.saved = saved;
        crate::runs::file::rewrite_header(&path, &h).unwrap();
        (st, path)
    }

    fn with_saves(dir: &Dir) -> AppState {
        let mut st = AppState::new();
        st.saves = Some(dir.0.clone());
        st
    }

    fn draw(s: &RunsScreen, st: &AppState) -> Buffer {
        draw_stack(st, &[s], (120, 40))
    }

    fn row(buf: &Buffer, y: u16) -> String {
        buffer_text(buf)
            .lines()
            .nth(y as usize)
            .unwrap()
            .to_string()
    }

    /// Row `y` inside the panel's sides, trimmed.
    fn inner(buf: &Buffer, y: u16) -> String {
        (21..99)
            .map(|x| buf[(x, y)].symbol())
            .collect::<String>()
            .trim()
            .to_string()
    }

    /// The column `text` starts at on row `y`.
    fn col(buf: &Buffer, y: u16, text: &str) -> u16 {
        let r = row(buf, y);
        let at = r
            .find(text)
            .unwrap_or_else(|| panic!("{text:?} on row {y}: {r:?}"));
        r[..at].chars().count() as u16
    }

    const PICK: Color = Color::Rgb(0, 204, 184);
    const FAINT: Color = Color::Rgb(100, 110, 125);
    const DIM: Color = Color::Rgb(160, 170, 185);
    const TEXT: Color = Color::Rgb(205, 212, 222);
    const CYAN: Color = Color::Rgb(0, 229, 255);
    const MAGENTA: Color = Color::Rgb(255, 0, 200);

    #[test]
    fn load_game_lists_living_runs_only() {
        let dir = Dir::new("living");
        save(&dir, "Alive", true, 2_000);
        let (_, gone) = save(&dir, "Gone", true, 3_000);
        let mut h = crate::runs::file::header(&gone).unwrap();
        h.death = Some(crate::runs::Death {
            cause: "black ICE".into(),
            day: 1,
            minute: 30,
            at: 3_000,
            lasted: 90,
        });
        crate::runs::file::rewrite_header(&gone, &h).unwrap();
        let mut st = with_saves(&dir);
        let s = RunsScreen::load(&st);
        let text = buffer_text(&draw(&s, &st));
        assert!(text.contains("ALIVE"), "{text}");
        assert!(!text.contains("GONE"), "the dead are in the hall: {text}");
        assert_eq!(text.matches("PERMA").count(), 2, "its row and its card");

        // A file that isn't a run: a dim row of its own, that opens nothing.
        std::fs::write(dir.0.join("broken.run"), b"not a run at all").unwrap();
        let mut s = RunsScreen::load(&st);
        let buf = draw(&s, &st);
        assert!(row(&buf, 10).contains("ALIVE"), "{}", row(&buf, 10));
        assert_eq!(inner(&buf, 11), "not a run", "its reason is the row");
        let x = col(&buf, 11, "not a run");
        assert_eq!(x, 23);
        assert_eq!(buf[(x, 11)].fg, FAINT, "dim");
        s.update(&mut st, &[key(KeyCode::Down)]);
        let buf = draw(&s, &st);
        assert_eq!(buf[(21, 11)].symbol(), "▸", "the cursor on it");
        assert!(matches!(s.update(&mut st, &[key(SPACE)]), ScreenCmd::Stay));
        assert!(st.run.is_none() && st.city.is_none(), "nothing opened");
        // Back up, space opens the run.
        s.update(&mut st, &[key(KeyCode::Char('k'))]);
        assert!(matches!(
            s.update(&mut st, &[key(SPACE)]),
            ScreenCmd::Switch { .. }
        ));
        assert_eq!(st.character.handle, "Alive");
        assert!(st.run.is_some() && st.player.is_some());
    }

    #[test]
    fn load_game_draws_as_the_mock() {
        let dir = Dir::new("mock");
        let (vex, path) = save(&dir, "Vex", true, 2_000);
        let mut h = crate::runs::file::header(&path).unwrap();
        h.level = 4;
        h.day = 2;
        h.minute = 7 * 60 + 40;
        h.mission = Some("The ledger".into());
        h.objective = Some("Bring it to Coz · the Brass Lamp · Ironside".into());
        h.credits = 1_240;
        h.chrome = vec!["Neural port".into(), "Target lens".into()];
        h.talents = 6;
        h.played = 3 * 3600 + 12 * 60;
        h.place = "Velform Group Plaza 18F · at home".into();
        crate::runs::file::rewrite_header(&path, &h).unwrap();
        save(&dir, "Bram", false, 1_000);
        let st = with_saves(&dir);
        let s = RunsScreen::load(&st);
        let buf = draw(&s, &st);
        let city = vex.city.as_ref().unwrap().name.clone();
        // The panel, retitled.
        assert_eq!(buf[(20, 8)].symbol(), "╭");
        assert_eq!(buf[(99, 8)].symbol(), "╮");
        assert_eq!(col(&buf, 8, "LOAD GAME"), 23);
        assert_eq!(buf[(23, 8)].fg, CYAN);
        // The header row, faint.
        for (name, x) in [
            ("RUNNER", 23),
            ("BUILD", 33),
            ("LVL", 44),
            ("CITY", 49),
            ("CLOCK", 62),
            ("THE STORY", 75),
        ] {
            assert_eq!(col(&buf, 9, name), x, "{name}");
            assert_eq!(buf[(x, 9)].fg, FAINT, "{name}");
        }
        // The newest first, picked: teal with ▸, the runner bold.
        assert_eq!(buf[(21, 10)].symbol(), "▸");
        assert_eq!(col(&buf, 10, "VEX"), 23);
        assert_eq!(buf[(23, 10)].fg, PICK);
        assert!(buf[(23, 10)].modifier.contains(Modifier::BOLD));
        assert_eq!(col(&buf, 10, "NETRUNNER"), 33);
        assert_eq!(buf[(33, 10)].fg, PICK);
        assert_eq!(col(&buf, 10, "4 "), 44);
        assert_eq!(col(&buf, 10, &city), 49);
        assert_eq!(col(&buf, 10, "day 2 07:40"), 62);
        assert_eq!(col(&buf, 10, "Bring it to Coz"), 75);
        assert_eq!(col(&buf, 10, "PERMA"), 93);
        assert_eq!(buf[(93, 10)].fg, MAGENTA);
        assert!(buf[(93, 10)].modifier.contains(Modifier::BOLD));
        // The other: the runner in text, the rest dim, no tag.
        assert_eq!(buf[(21, 11)].symbol(), " ");
        assert_eq!(col(&buf, 11, "BRAM"), 23);
        assert_eq!(buf[(23, 11)].fg, TEXT);
        assert_eq!(buf[(33, 11)].fg, DIM);
        assert!(!row(&buf, 11).contains("PERMA"));
        // A rule, the card, a rule, the keys, the bottom.
        for y in [13, 20] {
            assert_eq!(row(&buf, y)[..].chars().nth(21), Some('─'), "rule at {y}");
            assert_eq!(buf[(21, y)].fg, Color::Rgb(31, 43, 56));
            assert_eq!(buf[(98, y)].symbol(), "─");
        }
        assert_eq!(col(&buf, 15, "VEX · NETRUNNER · level 4"), 23);
        assert_eq!(buf[(23, 15)].fg, CYAN);
        assert!(buf[(23, 15)].modifier.contains(Modifier::BOLD));
        assert_eq!(buf[(26, 15)].fg, TEXT);
        assert_eq!(col(&buf, 15, "PERMA DEATH"), 87);
        assert_eq!(buf[(87, 15)].fg, MAGENTA);
        let place = format!("{city} · day 2 07:40 · Velform Group Plaza 18F · at home");
        assert_eq!(col(&buf, 16, &place), 23);
        assert_eq!(buf[(23, 16)].fg, DIM);
        let story = "The ledger · Bring it to Coz · the Brass Lamp · Ironside";
        assert_eq!(col(&buf, 17, story), 23);
        assert_eq!(buf[(23, 17)].fg, TEXT);
        assert_eq!(buf[(23 + 10, 17)].fg, DIM);
        let stats = "¤ 1,240 · 2 chrome · 6 talents · played 3h 12m · saved 1970-01-01 00:33";
        assert_eq!(col(&buf, 18, stats), 23);
        assert_eq!(buf[(23, 18)].fg, FAINT);
        assert_eq!(inner(&buf, 21), "↑↓ select · space continue · esc back");
        assert_eq!(col(&buf, 21, "↑↓ select"), 22);
        assert_eq!(buf[(22, 21)].fg, FAINT);
        assert_eq!(buf[(20, 23)].symbol(), "╰");
        assert_eq!(buf[(99, 23)].symbol(), "╯");
        assert_eq!(
            buf[(20, 24)].symbol(),
            " ",
            "the panel closes under its keys"
        );
    }

    #[test]
    fn the_list_moves_and_wraps() {
        let dir = Dir::new("moves");
        save(&dir, "One", false, 3_000);
        save(&dir, "Two", false, 2_000);
        save(&dir, "Three", false, 1_000);
        let mut st = with_saves(&dir);
        let mut s = RunsScreen::load(&st);
        let picked = |s: &RunsScreen, st: &AppState| {
            let buf = draw(s, st);
            (10..13).find(|&y| buf[(21, y)].symbol() == "▸").unwrap() - 10
        };
        assert_eq!(picked(&s, &st), 0);
        s.update(&mut st, &[key(KeyCode::Down), key(KeyCode::Char('j'))]);
        assert_eq!(picked(&s, &st), 2);
        s.update(&mut st, &[key(KeyCode::Down)]);
        assert_eq!(picked(&s, &st), 0, "round to the top");
        s.update(&mut st, &[key(KeyCode::Up)]);
        assert_eq!(picked(&s, &st), 2);
        // The card is the picked run's.
        let card = inner(&draw(&s, &st), 16);
        assert!(card.starts_with("THREE · "), "{card}");
        assert!(
            matches!(s.update(&mut st, &[key(KeyCode::Enter)]), ScreenCmd::Stay),
            "enter doesn't confirm a new screen"
        );
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
        assert!(st.run.is_none());
    }

    #[test]
    fn a_long_list_keeps_the_cursor_in_sight() {
        let dir = Dir::new("long");
        let (_, first) = save(&dir, "R00", false, 1_000);
        // Eleven more under other names, newer each.
        for n in 1..12u64 {
            let path = dir.0.join(format!("r{n:02}.run"));
            std::fs::copy(&first, &path).unwrap();
            let mut h = crate::runs::file::header(&path).unwrap();
            h.handle = format!("R{n:02}");
            h.saved = 1_000 + n;
            crate::runs::file::rewrite_header(&path, &h).unwrap();
        }
        let mut st = with_saves(&dir);
        let mut s = RunsScreen::load(&st);
        let buf = draw(&s, &st);
        // Ten rows in the panel's 24, newest first.
        assert_eq!(col(&buf, 10, "R11"), 23);
        assert_eq!(col(&buf, 19, "R02"), 23);
        assert_eq!(buf[(20, 31)].symbol(), "╰", "the panel at its tallest");
        // Up from the top is the oldest, at the bottom of the rows.
        s.update(&mut st, &[key(KeyCode::Up)]);
        let buf = draw(&s, &st);
        assert_eq!(buf[(21, 19)].symbol(), "▸");
        assert_eq!(col(&buf, 19, "R00"), 23);
        assert!(inner(&buf, 23).starts_with("R00 · "), "its card");
        s.update(&mut st, &[key(KeyCode::Down)]);
        assert_eq!(draw(&s, &st)[(21, 10)].symbol(), "▸", "round to the top");
    }

    #[test]
    fn no_runs_yet() {
        let dir = Dir::new("none");
        let mut st = with_saves(&dir);
        for st_saves in [Some(dir.0.clone()), None] {
            st.saves = st_saves;
            let mut s = RunsScreen::load(&st);
            let buf = draw(&s, &st);
            assert_eq!(col(&buf, 8, "LOAD GAME"), 23);
            let empty = "No runs yet.";
            assert_eq!(col(&buf, 11, empty), (120 - empty.len() as u16) / 2);
            assert_eq!(buf[(54, 11)].fg, DIM);
            assert_eq!(inner(&buf, 14), "esc back");
            assert_eq!(col(&buf, 14, "esc back"), 22);
            assert_eq!(buf[(20, 16)].symbol(), "╰");
            assert!(matches!(s.update(&mut st, &[key(SPACE)]), ScreenCmd::Stay));
            assert!(matches!(
                s.update(&mut st, &[key(KeyCode::Esc)]),
                ScreenCmd::Pop { .. }
            ));
        }
    }

    #[test]
    fn the_hall_waits_for_its_dead() {
        let dir = Dir::new("hall");
        save(&dir, "Alive", true, 2_000);
        let mut st = with_saves(&dir);
        let mut s = RunsScreen::hall(&st);
        let buf = draw(&s, &st);
        assert_eq!(col(&buf, 8, "HALL OF FAME"), 23);
        let empty = "Nobody you ran has flatlined. Yet.";
        assert_eq!(col(&buf, 11, empty), 43);
        assert!(
            !buffer_text(&buf).contains("ALIVE"),
            "the living aren't in it"
        );
        assert_eq!(col(&buf, 14, "esc back"), 22);
        assert!(matches!(s.update(&mut st, &[key(SPACE)]), ScreenCmd::Stay));
        assert!(st.run.is_none());
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
    }

    #[test]
    fn a_run_that_wont_open_says_why() {
        let dir = Dir::new("wont-open");
        let (_, path) = save(&dir, "Vex", false, 2_000);
        // The header stands, the body is noise.
        let mut bytes = std::fs::read(&path).unwrap();
        let end = 10 + u32::from_le_bytes(bytes[6..10].try_into().unwrap()) as usize;
        for (i, b) in bytes[end..].iter_mut().enumerate() {
            *b = citygen::rng::hash64(i as u64, 7) as u8;
        }
        std::fs::write(&path, &bytes).unwrap();
        let mut st = with_saves(&dir);
        let mut s = RunsScreen::load(&st);
        assert!(row(&draw(&s, &st), 10).contains("VEX"));
        assert!(matches!(s.update(&mut st, &[key(SPACE)]), ScreenCmd::Stay));
        assert!(st.run.is_none() && st.city.is_none(), "nothing half opened");
        let buf = draw(&s, &st);
        assert_eq!(inner(&buf, 10), "▸ the run is broken");
        assert_eq!(col(&buf, 10, "the run is broken"), 23);
        assert_eq!(buf[(23, 10)].fg, FAINT);
        assert!(matches!(s.update(&mut st, &[key(SPACE)]), ScreenCmd::Stay));
    }
}
