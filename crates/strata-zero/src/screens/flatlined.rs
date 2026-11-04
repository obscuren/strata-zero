//! FLATLINED: black ICE took the meat through the jack. The screen reads the cause and
//! the runner, then how the run ends (`End`): with perma death on, a taunt from
//! `flavor/flatline.ron` and any key to the title, the run gone to the HALL OF FAME;
//! without, the last save to go back to, or the title.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::render::Rgb;
use crate::runs::{RunHeader, RunId, describe, file};
use crate::screens::runs::{FAINT, PINK};
use crate::screens::title::Title;
use crate::ui::chrome::{DIM, TEXT, put, solid};
use crate::ui::logo::{self, Logo};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::OnceLock;

const RED: Rgb = Rgb(255, 40, 40);
const TAUNTS: &str = include_str!("../../../../flavor/flatline.ron");

/// What a perma death flatline says to the runner: `flavor/flatline.ron`.
#[derive(Debug, Deserialize)]
pub struct Taunts {
    taunts: Vec<String>,
}

impl Taunts {
    /// The pack in `src`; one with no taunts is refused.
    pub fn parse(src: &str) -> Result<Taunts, String> {
        let t: Taunts = ron::from_str(src).map_err(|e| format!("flatline parse error: {e}"))?;
        if t.taunts.is_empty() {
            return Err("flatline: `taunts` is empty".to_string());
        }
        Ok(t)
    }

    /// The embedded `flavor/flatline.ron`, read once.
    pub fn load() -> &'static Taunts {
        static PACK: OnceLock<Taunts> = OnceLock::new();
        PACK.get_or_init(|| Taunts::parse(TAUNTS).expect("the flatline pack is valid"))
    }

    #[cfg(test)]
    pub fn all(&self) -> &[String] {
        &self.taunts
    }

    /// The run's own taunt, picked by its id: the same one every time.
    pub fn pick(&self, run: RunId) -> &str {
        let i = citygen::rng::hash64(run.0, 0xF1A7) % self.taunts.len() as u64;
        &self.taunts[i as usize]
    }
}

/// How the run ends, under the runner.
#[derive(Clone, Debug, PartialEq)]
pub enum End {
    /// Perma death: the run is marked and only in the HALL OF FAME now. One taunt, then
    /// any key to the title.
    Perma { taunt: String },
    /// The last save to go back to: its file, and its clock and place as its header
    /// says them (`day 2 07:40 · …`). Space loads it, esc goes to the title.
    Reload { path: PathBuf, line: String },
    /// No save to go back to (a dev start, or no folder to save in): the title.
    Title,
}

/// The run in `st` flatlined of `cause`: marked with it if perma death is on
/// (`runs::when::flatline`), and how its screen ends.
pub fn end(st: &mut AppState, cause: &str) -> End {
    let run = st.run.map(|r| r.id);
    let perma = st.perma_death;
    let last = crate::runs::when::flatline(st, cause);
    match (run, last, st.saves.as_deref()) {
        (Some(id), _, _) if perma => End::Perma {
            taunt: Taunts::load().pick(id).to_string(),
        },
        (Some(id), Some(h), Some(dir)) => End::Reload {
            path: file::path(dir, id),
            line: reload_line(&h),
        },
        _ => End::Title,
    }
}

/// Where the save at `h` puts you back: `day 2 07:40 · Velform Group Plaza 18F · at home`.
fn reload_line(h: &RunHeader) -> String {
    let clock = describe::clock(h.day, h.minute);
    if h.place.is_empty() {
        clock
    } else {
        format!("{clock} · {}", h.place)
    }
}

/// Off to the title. The runner is gone with the session they ran, a fresh one stands
/// ready, and the run is left: nothing on the title saves it again.
fn to_title(st: &mut AppState) -> ScreenCmd {
    st.net = Default::default();
    st.constructs = Default::default();
    if let Some(fresh) = game::Character::from_template(&st.content, &game::TemplateId::new("vex"))
    {
        st.character = fresh;
    }
    st.perma_death = true;
    st.tutorial_off = false;
    st.player = None;
    st.run = None;
    ScreenCmd::Switch {
        to: Box::new(Title::new(st, st.tick)),
        fx: TransitionKind::Glitch,
    }
}

pub struct Flatlined {
    cause: String,
    runner: String,
    opened: u64,
    end: End,
    /// Why the last save wouldn't open, once space has tried it.
    failed: Option<String>,
}

impl Flatlined {
    pub fn new(cause: String, handle: &str, level: u8, tick: u64, end: End) -> Self {
        Flatlined {
            cause,
            runner: format!("{} · level {level}", handle.to_uppercase()),
            opened: tick,
            end,
            failed: None,
        }
    }
}

impl Screen for Flatlined {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        // A breath before any key counts, so the hit that killed does not skip the screen.
        if keys.is_empty() || st.tick < self.opened + 10 {
            return ScreenCmd::Stay;
        }
        let End::Reload { path, .. } = &self.end else {
            return to_title(st);
        };
        for k in keys {
            match k.code {
                KeyCode::Esc => return to_title(st),
                // Straight back onto the map where the save stood, no broadcast.
                KeyCode::Char(' ') if self.failed.is_none() => match crate::runs::open(st, path) {
                    Ok(()) => {
                        return ScreenCmd::Switch {
                            to: crate::screens::resume_screen(st),
                            fx: TransitionKind::Glitch,
                        };
                    }
                    Err(e) => self.failed = Some(e.to_string()),
                },
                _ => {}
            }
        }
        ScreenCmd::Stay
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
        let faint = Style::default().fg(FAINT);
        match (&self.end, &self.failed) {
            (End::Perma { taunt }, _) => {
                centred(buf, y + 3, taunt, Style::default().fg(PINK));
                centred(buf, y + 5, "any key · title", faint);
            }
            (End::Reload { line, .. }, None) => {
                let back = format!("reload last save point · {line}");
                centred(buf, y + 3, &back, Style::default().fg(DIM));
                centred(buf, y + 5, "space · load last save     esc · title", faint);
            }
            (End::Reload { .. }, Some(why)) => {
                centred(buf, y + 3, why, faint);
                centred(buf, y + 5, "esc · title", faint);
            }
            (End::Title, _) => centred(buf, y + 3, "any key · title", Style::default().fg(DIM)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runs::{RunId, RunMeta};
    use crate::screens::shots::draw_stack;
    use crate::screens::testutil::{buffer_text, key};
    use crossterm::event::KeyCode;
    use ratatui::buffer::Buffer;
    use ratatui::style::Color;

    const PINK: Color = Color::Rgb(248, 147, 147);
    const DIM: Color = Color::Rgb(160, 170, 185);
    const FAINT: Color = Color::Rgb(100, 110, 125);
    const SPACE: KeyCode = KeyCode::Char(' ');

    /// The spec's six, in order.
    const SIX: [&str; 6] = [
        "somebody's already wearing your jacket.",
        "your rent's still due.",
        "should've stayed in bed.",
        "the street forgot you before you hit the floor.",
        "the ICE didn't even slow down.",
        "they'll sell your chrome by morning.",
    ];

    fn draw(f: &Flatlined, st: &AppState) -> Buffer {
        draw_stack(st, &[f], (120, 40))
    }

    /// Row `y`, trimmed.
    fn row(buf: &Buffer, y: u16) -> String {
        buffer_text(buf)
            .lines()
            .nth(y as usize)
            .unwrap()
            .trim()
            .to_string()
    }

    /// Asserts row `y` is `text` centred on the 120 columns, in `fg`.
    fn centred(buf: &Buffer, y: u16, text: &str, fg: Color) {
        assert_eq!(row(buf, y), text, "row {y}");
        let x = (120 - text.chars().count() as u16) / 2;
        assert_eq!(
            buf[(x, y)].symbol(),
            text.chars().next().unwrap().to_string()
        );
        assert_eq!(buf[(x, y)].fg, fg, "row {y}'s colour");
    }

    /// A state with a run under way, as a flatline finds it.
    fn in_a_run() -> AppState {
        let mut st = AppState::new();
        st.run = Some(RunMeta {
            id: RunId(7),
            started: 0,
            born: 0,
        });
        st
    }

    #[test]
    fn the_taunts_load() {
        let t = Taunts::load();
        assert_eq!(t.all().len(), 6);
        assert_eq!(t.all()[0], "somebody's already wearing your jacket.");
        assert_eq!(t.all(), SIX);
        assert!(
            Taunts::parse("(taunts: [])").is_err(),
            "a pack with no taunts is refused"
        );
        assert!(Taunts::parse("(taunts: [\"x\"])").is_ok());
        // One per run, by its id.
        for id in [0, 1, 7, 0xdead_beef, u64::MAX] {
            let i = (citygen::rng::hash64(id, 0xF1A7) % 6) as usize;
            assert_eq!(t.pick(RunId(id)), SIX[i], "run {id:x}");
        }
        let picked: std::collections::BTreeSet<&str> = (0..64).map(|i| t.pick(RunId(i))).collect();
        assert_eq!(picked.len(), 6, "every taunt comes up");
    }

    #[test]
    fn a_flatline_resets_the_flags() {
        let mut st = AppState::new();
        st.perma_death = false;
        st.tutorial_off = true;
        let mut f = Flatlined::new("black ICE".into(), "Nyx", 3, 0, End::Title);
        st.tick = 20;
        assert!(matches!(
            f.update(&mut st, &[key(KeyCode::Char(' '))]),
            ScreenCmd::Switch { .. }
        ));
        assert!(st.perma_death && !st.tutorial_off);
    }

    #[test]
    fn with_perma_death_it_taunts() {
        let mut st = in_a_run();
        let taunt = SIX[0].to_string();
        let mut f = Flatlined::new(
            "black ICE at the bank".into(),
            "Vex",
            1,
            0,
            End::Perma { taunt },
        );
        st.tick = 40;
        let buf = draw(&f, &st);
        centred(&buf, 19, "black ICE at the bank", TEXT);
        centred(&buf, 20, "VEX · level 1", DIM);
        // The taunt where `any key · title` was, the keys two rows lower.
        centred(&buf, 22, SIX[0], PINK);
        assert_eq!(row(&buf, 23), "");
        centred(&buf, 24, "any key · title", FAINT);
        let text = buffer_text(&buf);
        assert!(!text.contains("HALL OF FAME"), "not a word about it");
        assert!(!text.contains("reload"));
        // Not before a breath, then any key: the title.
        st.tick = 5;
        assert!(matches!(
            f.update(&mut st, &[key(KeyCode::Char('x'))]),
            ScreenCmd::Stay
        ));
        st.tick = 40;
        assert!(matches!(
            f.update(&mut st, &[key(KeyCode::Char('x'))]),
            ScreenCmd::Switch { .. }
        ));
        assert!(st.run.is_none() && st.player.is_none());
    }

    #[test]
    fn without_perma_death_it_offers_the_last_save() {
        let dir = crate::runs::testutil::Dir::new("flatline-reload");
        let mut st = in_a_run();
        st.perma_death = false;
        st.net.death = Some("black ICE at the bank".into());
        let mut f = Flatlined::new(
            "black ICE at the bank".into(),
            "Vex",
            1,
            0,
            End::Reload {
                path: dir.0.join("gone.run"),
                line: "day 2 07:40 · at home".into(),
            },
        );
        st.tick = 40;
        let buf = draw(&f, &st);
        centred(&buf, 19, "black ICE at the bank", TEXT);
        centred(&buf, 20, "VEX · level 1", DIM);
        centred(
            &buf,
            22,
            "reload last save point · day 2 07:40 · at home",
            DIM,
        );
        centred(&buf, 24, "space · load last save     esc · title", FAINT);
        // Keys that aren't its own do nothing.
        for k in [KeyCode::Enter, KeyCode::Char('x'), KeyCode::Down] {
            assert!(matches!(f.update(&mut st, &[key(k)]), ScreenCmd::Stay));
        }
        assert!(st.run.is_some(), "still on the flatline");
        // A save that won't open: why, in its place, and only esc left.
        assert!(matches!(f.update(&mut st, &[key(SPACE)]), ScreenCmd::Stay));
        let buf = draw(&f, &st);
        centred(&buf, 22, "can't read it", FAINT);
        centred(&buf, 24, "esc · title", FAINT);
        assert!(st.net.death.is_some(), "nothing was put in place");
        // Esc: the title, the run left behind, a fresh runner ready.
        let ScreenCmd::Switch { .. } = f.update(&mut st, &[key(KeyCode::Esc)]) else {
            panic!("esc goes to the title")
        };
        assert!(st.run.is_none(), "nothing on the title saves it");
        assert!(st.net.death.is_none() && st.player.is_none());
        assert!(st.perma_death && !st.tutorial_off);
    }

    #[test]
    fn with_no_save_any_key_is_the_title() {
        let mut st = AppState::new();
        let mut f = Flatlined::new("black ICE".into(), "Vex", 1, 0, End::Title);
        st.tick = 40;
        let buf = draw(&f, &st);
        centred(&buf, 22, "any key · title", DIM);
        assert_eq!(row(&buf, 24), "");
        assert!(matches!(
            f.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Switch { .. }
        ));
    }
}
