//! LOAD GAME and the HALL OF FAME: the runs in the saves folder, one row each, and a
//! card for the one under the cursor.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::runs::describe::{clock, credits, date, first_part, lasted, played, saved_at};
use crate::runs::{Death, RunHeader, RunInfo};
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
/// How a run ended: the hall card's cause and the flatline's taunt.
pub(crate) const PINK: Color = Color::Rgb(248, 147, 147);
/// The hall card's FLATLINED.
const RED: Color = Color::Rgb(255, 79, 106);
/// The rules across the panel.
const RULE: Color = Color::Rgb(31, 43, 56);

/// The panel at its widest and tallest, as the files panel is; it closes under its keys.
const PANEL_W: u16 = 80;
const PANEL_H: u16 = 24;
/// The panel's rows that are neither runs nor the card: its edges, the header row, the
/// rules, the keys and the gaps between them.
const FRAME_ROWS: u16 = 10;
/// The panel's height with nobody in it.
const EMPTY_H: u16 = 9;
/// LOAD GAME's columns from the runner's: RUNNER, BUILD, LVL, CITY, CLOCK; THE STORY
/// runs on to the tag. A cell is cut a character short of its column.
const LOAD_WIDTHS: [u16; 5] = [10, 11, 5, 13, 13];
const LOAD_HEADS: [&str; 6] = ["RUNNER", "BUILD", "LVL", "CITY", "CLOCK", "THE STORY"];
/// The HALL OF FAME's: RUNNER, BUILD, LVL, CITY, LASTED, THE END.
const HALL_WIDTHS: [u16; 6] = [10, 11, 5, 13, 10, 26];
const HALL_HEADS: [&str; 6] = ["RUNNER", "BUILD", "LVL", "CITY", "LASTED", "THE END"];
const TAG: &str = "PERMA";

/// Which list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// The runs still going, newest saved first; space opens one.
    Load,
    /// The runs that flatlined, newest death first; nothing opens from here.
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

/// `9 talents`, `1 talent`.
fn talents(n: u32) -> String {
    match n {
        1 => "1 talent".to_string(),
        n => format!("{n} talents"),
    }
}

/// The mission in bright, then its objective dim: as much of the story as there is.
fn story<'a>(mission: Option<&'a str>, objective: &'a str) -> Vec<(&'a str, Style)> {
    match mission {
        Some(m) if !objective.is_empty() => {
            vec![(m, fg(TEXT)), (" · ", fg(DIM)), (objective, fg(DIM))]
        }
        Some(m) => vec![(m, fg(TEXT))],
        None if !objective.is_empty() => vec![(objective, fg(DIM))],
        None => Vec::new(),
    }
}

/// The runner's name in cyan and their build, with `tag` (`colour`, bold) at the right.
fn runner_line(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    h: &RunHeader,
    tag: Option<(&str, Color)>,
    right: u16,
) {
    let build = format!(" · {} · level {}", h.profession, h.level);
    let tag_x = tag.map(|(t, _)| right.saturating_sub(1 + t.chars().count() as u16));
    spans(
        buf,
        x,
        y,
        &[(&h.handle.to_uppercase(), bold(CYAN)), (&build, fg(TEXT))],
        tag_x.map_or(right, |t| t.saturating_sub(1)),
    );
    if let (Some((t, c)), Some(tx)) = (tag, tag_x) {
        put(buf, tx, y, t, bold(c), right);
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

    /// The HALL OF FAME: the runs in `st`'s saves folder that flatlined, none without one.
    pub fn hall(st: &AppState) -> Self {
        RunsScreen {
            mode: Mode::Hall,
            runs: st
                .saves
                .as_deref()
                .map(crate::runs::dead)
                .unwrap_or_default(),
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

    /// The header row's names and the columns' widths; a column past the widths runs
    /// on to the tag.
    fn columns(&self) -> (&'static [&'static str], &'static [u16]) {
        match self.mode {
            Mode::Load => (&LOAD_HEADS, &LOAD_WIDTHS),
            Mode::Hall => (&HALL_HEADS, &HALL_WIDTHS),
        }
    }

    /// The card's rows.
    fn card_rows(&self) -> u16 {
        match self.mode {
            Mode::Load => 4,
            Mode::Hall => 5,
        }
    }

    /// One run's row: the cursor, its columns, the tag of a perma death run in LOAD
    /// GAME; a file that can't be read says why, dim.
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
        let tagged = self.mode == Mode::Load && h.perma;
        let tag_x = right.saturating_sub(1 + TAG.len() as u16);
        let last = match (self.mode, h.death.as_ref()) {
            (Mode::Hall, Some(d)) => [lasted(d.lasted), d.cause.clone()],
            _ => [
                clock(h.day, h.minute),
                h.objective
                    .as_deref()
                    .map(first_part)
                    .unwrap_or_default()
                    .to_string(),
            ],
        };
        let cells = [
            h.handle.to_uppercase(),
            h.profession.clone(),
            h.level.to_string(),
            h.city.clone(),
        ]
        .into_iter()
        .chain(last);
        let (_, widths) = self.columns();
        let mut x = x0 + 3;
        for (n, text) in cells.enumerate() {
            let w = widths.get(n).copied().unwrap_or(tag_x.saturating_sub(x));
            let style = match (picked, n) {
                (true, 0) => bold(PICK),
                (true, _) => fg(PICK),
                (false, 0) => fg(TEXT),
                (false, _) => fg(DIM),
            };
            put(buf, x, y, &cut(&text, w.saturating_sub(1)), style, right);
            x += w;
        }
        if tagged {
            put(buf, tag_x, y, TAG, bold(MAGENTA), right);
        }
    }

    /// LOAD GAME's card of the run under the cursor, four rows from `y`.
    fn draw_card(&self, buf: &mut Buffer, x: u16, y: u16, h: &RunHeader, right: u16) {
        runner_line(
            buf,
            x,
            y,
            h,
            h.perma.then_some(("PERMA DEATH", MAGENTA)),
            right,
        );
        let mut place = format!("{} · {}", h.city, clock(h.day, h.minute));
        if !h.place.is_empty() {
            place = format!("{place} · {}", h.place);
        }
        put(buf, x, y + 1, &place, fg(DIM), right);
        let objective = h.objective.as_deref().unwrap_or_default();
        spans(
            buf,
            x,
            y + 2,
            &story(h.mission.as_deref(), objective),
            right,
        );
        let stats = format!(
            "{} · {} chrome · {} · played {} · saved {}",
            credits(h.credits),
            h.chrome.len(),
            talents(h.talents),
            played(h.played),
            saved_at(h.saved, crate::runs::describe::unix_now())
        );
        put(buf, x, y + 3, &stats, fg(FAINT), right);
    }

    /// The HALL OF FAME's card of the run under the cursor, five rows from `y`: how it
    /// ended, how long it lasted, how far the story got and what was left behind.
    fn draw_dead(&self, buf: &mut Buffer, x: u16, y: u16, h: &RunHeader, d: &Death, right: u16) {
        runner_line(buf, x, y, h, Some(("FLATLINED", RED)), right);
        let end = format!("{} · {} · {}", d.cause, clock(d.day, d.minute), date(d.at));
        put(buf, x, y + 1, &end, fg(PINK), right);
        let life = format!(
            "{} · lasted {} · played {}",
            h.city,
            lasted(d.lasted),
            played(h.played)
        );
        put(buf, x, y + 2, &life, fg(DIM), right);
        let objective = h.objective.as_deref().map(first_part).unwrap_or_default();
        let got = story(h.mission.as_deref(), objective);
        if !got.is_empty() {
            let mut parts = vec![("got as far as  ", fg(FAINT))];
            parts.extend(got);
            spans(buf, x, y + 3, &parts, right);
        }
        let mut left = vec![credits(h.credits)];
        left.extend(h.chrome.iter().cloned());
        left.push(talents(h.talents));
        spans(
            buf,
            x,
            y + 4,
            &[("left behind    ", fg(FAINT)), (&left.join(" · "), fg(DIM))],
            right,
        );
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
        let card = self.card_rows();
        // Rows for runs; the rest scroll, the cursor's always among them.
        let room = tall.saturating_sub(FRAME_ROWS + card).max(1) as usize;
        let shown = self.runs.len().min(room);
        let first = (self.selected + 1).saturating_sub(shown);
        let h = if shown == 0 {
            EMPTY_H
        } else {
            FRAME_ROWS + card + shown as u16
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
        let (heads, widths) = self.columns();
        let mut hx = x;
        for (n, name) in heads.iter().enumerate() {
            put(buf, hx, y0 + 1, name, fg(FAINT), right);
            hx += widths.get(n).copied().unwrap_or(0);
        }
        for (row, i) in (first..first + shown).enumerate() {
            self.draw_row(buf, x0, y0 + 2 + row as u16, i, right);
        }
        let rule = "─".repeat(w.saturating_sub(2) as usize);
        let below = y0 + 2 + shown as u16;
        put(buf, x0 + 1, below + 1, &rule, fg(RULE), right);
        if let Some(Ok(h)) = self.runs.get(self.selected).map(|r| r.header.as_ref()) {
            match (self.mode, h.death.as_ref()) {
                (Mode::Hall, Some(d)) => self.draw_dead(buf, x, below + 3, h, d, right),
                (Mode::Hall, None) => {}
                (Mode::Load, _) => self.draw_card(buf, x, below + 3, h, right),
            }
        }
        put(buf, x0 + 1, below + 4 + card, &rule, fg(RULE), right);
        put(buf, x0 + 2, below + 5 + card, keys, fg(FAINT), right);
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

    /// The cells of row `y` from `xs`.
    fn cells(buf: &Buffer, y: u16, xs: std::ops::Range<u16>) -> String {
        xs.map(|x| buf[(x, y)].symbol()).collect()
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
    const RED: Color = Color::Rgb(255, 79, 106);
    const PINK: Color = Color::Rgb(248, 147, 147);

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
    fn the_hall_is_empty_at_first() {
        let dir = Dir::new("hall");
        save(&dir, "Alive", true, 2_000);
        let mut st = with_saves(&dir);
        for saves in [Some(dir.0.clone()), None] {
            st.saves = saves;
            let mut s = RunsScreen::hall(&st);
            let buf = draw(&s, &st);
            assert_eq!(col(&buf, 8, "HALL OF FAME"), 23);
            let empty = "Nobody you ran has flatlined. Yet.";
            assert_eq!(col(&buf, 11, empty), 43);
            assert_eq!(buf[(43, 11)].fg, DIM);
            assert!(
                !buffer_text(&buf).contains("ALIVE"),
                "the living aren't in it"
            );
            assert_eq!(inner(&buf, 14), "esc back");
            assert_eq!(col(&buf, 14, "esc back"), 22);
            assert_eq!(buf[(20, 16)].symbol(), "╰");
            assert!(matches!(s.update(&mut st, &[key(SPACE)]), ScreenCmd::Stay));
            assert!(st.run.is_none());
            assert!(matches!(
                s.update(&mut st, &[key(KeyCode::Esc)]),
                ScreenCmd::Pop { .. }
            ));
        }
    }

    /// 2026-11-02 10:44 UTC.
    const NOV_2: u64 = 1_793_616_240;

    /// A run in `dir` by `handle` that flatlined at `at` (Unix seconds), after
    /// `lasted` city minutes, of `cause`.
    fn dead(dir: &Dir, handle: &str, at: u64, lasted: u64, cause: &str) -> PathBuf {
        let (_, path) = save(dir, handle, true, at);
        let mut h = crate::runs::file::header(&path).unwrap();
        h.death = Some(crate::runs::Death {
            cause: cause.into(),
            day: 4,
            minute: 3 * 60 + 12,
            at,
            lasted,
        });
        crate::runs::file::rewrite_header(&path, &h).unwrap();
        path
    }

    #[test]
    fn the_hall_lists_the_dead_newest_first() {
        let dir = Dir::new("hall-dead");
        // The living, saved after them all, aren't in it.
        save(&dir, "Alive", true, NOV_2 + 9_000);
        dead(
            &dir,
            "Nail",
            NOV_2 - 2 * 86_400,
            2 * 60 + 5,
            "black ICE · Aoki",
        );
        let moth = dead(
            &dir,
            "Moth",
            NOV_2,
            3 * 1440 + 4 * 60,
            "black ICE · Velform Group's vault",
        );
        dead(
            &dir,
            "Rix",
            NOV_2 - 86_400,
            19 * 60 + 30,
            "black ICE · Zenbo",
        );
        let mut h = crate::runs::file::header(&moth).unwrap();
        h.level = 6;
        h.mission = Some("The ledger".into());
        h.objective = Some("Bring it to Coz · the Brass Lamp · Ironside".into());
        h.credits = 2_340;
        h.chrome = vec!["neural port".into(), "target lens".into()];
        h.talents = 9;
        h.played = 5 * 3600 + 40 * 60;
        crate::runs::file::rewrite_header(&moth, &h).unwrap();
        let city = h.city.clone();
        let mut st = with_saves(&dir);
        let mut s = RunsScreen::hall(&st);
        let buf = draw(&s, &st);
        assert!(
            !buffer_text(&buf).contains("ALIVE"),
            "the living aren't in it"
        );
        // The panel, retitled; the header row, faint.
        assert_eq!(buf[(20, 8)].symbol(), "╭");
        assert_eq!(col(&buf, 8, "HALL OF FAME"), 23);
        assert_eq!(buf[(23, 8)].fg, CYAN);
        for (name, x) in [
            ("RUNNER", 23),
            ("BUILD", 33),
            ("LVL", 44),
            ("CITY", 49),
            ("LASTED", 62),
            ("THE END", 72),
        ] {
            assert_eq!(col(&buf, 9, name), x, "{name}");
            assert_eq!(buf[(x, 9)].fg, FAINT, "{name}");
        }
        // Newest death first, picked.
        for (y, name, lasted) in [
            (10, "MOTH", "3 days"),
            (11, "RIX", "19 hours"),
            (12, "NAIL", "2 hours"),
        ] {
            assert_eq!(col(&buf, y, name), 23, "{name}");
            assert_eq!(col(&buf, y, "NETRUNNER"), 33);
            assert_eq!(col(&buf, y, &city), 49);
            assert_eq!(col(&buf, y, lasted), 62, "{name}");
            assert_eq!(col(&buf, y, "black ICE · "), 72, "{name}");
            assert!(!row(&buf, y).contains("PERMA"), "no tag in the hall");
        }
        assert_eq!(buf[(21, 10)].symbol(), "▸");
        assert_eq!(buf[(23, 10)].fg, PICK);
        assert!(buf[(23, 10)].modifier.contains(Modifier::BOLD));
        assert_eq!(buf[(62, 10)].fg, PICK);
        assert_eq!(col(&buf, 10, "6 "), 44);
        assert_eq!(buf[(23, 11)].fg, TEXT);
        assert_eq!(buf[(62, 11)].fg, DIM);
        // THE END is the cause cut to fit its column.
        assert_eq!(
            cells(&buf, 10, 72..99).trim_end(),
            "black ICE · Velform Group"
        );
        assert_eq!(buf[(96, 10)].symbol(), "p");
        assert_eq!(buf[(97, 10)].symbol(), " ");
        // A rule, the card, a rule, the keys, the bottom.
        for y in [14, 22] {
            assert_eq!(buf[(21, y)].symbol(), "─", "rule at {y}");
            assert_eq!(buf[(21, y)].fg, Color::Rgb(31, 43, 56));
        }
        assert_eq!(col(&buf, 16, "MOTH · NETRUNNER · level 6"), 23);
        assert_eq!(buf[(23, 16)].fg, CYAN);
        assert!(buf[(23, 16)].modifier.contains(Modifier::BOLD));
        assert_eq!(buf[(27, 16)].fg, TEXT);
        assert_eq!(col(&buf, 16, "FLATLINED"), 89);
        assert_eq!(buf[(89, 16)].fg, RED);
        assert!(buf[(89, 16)].modifier.contains(Modifier::BOLD));
        let end = "black ICE · Velform Group's vault · day 4 03:12 · 2026-11-02";
        assert_eq!(inner(&buf, 17), end);
        assert_eq!(col(&buf, 17, end), 23);
        assert_eq!(buf[(23, 17)].fg, PINK);
        assert_eq!(buf[(82, 17)].fg, PINK);
        let life = format!("{city} · lasted 3 days · played 5h 40m");
        assert_eq!(inner(&buf, 18), life);
        assert_eq!(col(&buf, 18, &life), 23);
        assert_eq!(buf[(23, 18)].fg, DIM);
        assert_eq!(
            inner(&buf, 19),
            "got as far as  The ledger · Bring it to Coz"
        );
        assert_eq!(col(&buf, 19, "got as far as"), 23);
        assert_eq!(buf[(23, 19)].fg, FAINT);
        assert_eq!(buf[(38, 19)].fg, TEXT, "the mission");
        assert_eq!(buf[(48, 19)].fg, DIM, "the objective");
        assert_eq!(
            inner(&buf, 20),
            "left behind    ¤ 2,340 · neural port · target lens · 9 talents"
        );
        assert_eq!(buf[(23, 20)].fg, FAINT);
        assert_eq!(buf[(38, 20)].fg, DIM);
        assert_eq!(inner(&buf, 23), "↑↓ select · esc back");
        assert_eq!(col(&buf, 23, "↑↓ select"), 22);
        assert_eq!(buf[(22, 23)].fg, FAINT);
        assert_eq!(buf[(20, 25)].symbol(), "╰");
        assert_eq!(buf[(99, 25)].symbol(), "╯");
        // The card follows the cursor; nothing opens from here.
        s.update(&mut st, &[key(KeyCode::Down)]);
        let buf = draw(&s, &st);
        assert_eq!(buf[(21, 11)].symbol(), "▸");
        assert!(inner(&buf, 16).starts_with("RIX · NETRUNNER · level 1"));
        assert!(inner(&buf, 17).starts_with("black ICE · Zenbo · day 4 03:12 · 2026-11-01"));
        assert!(inner(&buf, 18).contains("lasted 19 hours"));
        assert!(matches!(s.update(&mut st, &[key(SPACE)]), ScreenCmd::Stay));
        assert!(st.run.is_none() && st.city.is_none(), "nothing opened");
        s.update(&mut st, &[key(KeyCode::Char('j'))]);
        assert_eq!(draw(&s, &st)[(21, 12)].symbol(), "▸");
        s.update(&mut st, &[key(KeyCode::Char('j'))]);
        assert_eq!(draw(&s, &st)[(21, 10)].symbol(), "▸", "round to the top");
    }

    #[test]
    fn a_dead_run_with_no_story_or_chrome_says_what_it_has() {
        let dir = Dir::new("hall-bare");
        let path = dead(&dir, "Bare", NOV_2, 40, "shot");
        let mut h = crate::runs::file::header(&path).unwrap();
        h.mission = None;
        h.objective = None;
        h.chrome.clear();
        h.talents = 1;
        h.credits = 0;
        crate::runs::file::rewrite_header(&path, &h).unwrap();
        let st = with_saves(&dir);
        let buf = draw(&RunsScreen::hall(&st), &st);
        assert_eq!(col(&buf, 10, "40 min"), 62);
        // One row: the card from row 14.
        assert!(inner(&buf, 14).starts_with("BARE · "));
        assert_eq!(inner(&buf, 15), "shot · day 4 03:12 · 2026-11-02");
        assert_eq!(inner(&buf, 17), "", "no story got under way");
        assert_eq!(inner(&buf, 18), "left behind    ¤ 0 · 1 talent");
        assert_eq!(buf[(21, 20)].symbol(), "─");
        assert_eq!(inner(&buf, 21), "↑↓ select · esc back");
        assert_eq!(buf[(20, 23)].symbol(), "╰");
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
