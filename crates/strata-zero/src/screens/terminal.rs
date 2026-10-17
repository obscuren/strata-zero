//! The Access Terminal on screen: a beige CRT over the map with a scrollback, a
//! prompt, and replies that type themselves out. One screen hosts either session.

use crate::access::{self, Session};
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::ui::console::{Console, ConsoleEvent};
use crate::ui::device::{self, CRT_H, CRT_W, Led};
use citygen::{Place, Pos};
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

/// Characters a reply types a second.
pub const TYPE_CPS: u64 = 60;
/// Ticks the LED stays red after a denial.
const DENIED_TICKS: u64 = 30;
const TICKS_PER_SECOND: u64 = 30;

/// A reply typing itself out: its lines, and the tick it started.
struct Typing {
    lines: Vec<String>,
    started: u64,
    /// Set when the reply was EXIT: pop once it has typed.
    leaving: bool,
}

impl Typing {
    /// How many characters have appeared by `tick`.
    fn shown(&self, tick: u64) -> usize {
        (tick.saturating_sub(self.started) * TYPE_CPS / TICKS_PER_SECOND) as usize
    }

    fn total(&self) -> usize {
        self.lines.iter().map(|l| l.chars().count() + 1).sum()
    }

    fn done(&self, tick: u64) -> bool {
        self.shown(tick) >= self.total()
    }

    /// The lines as far as typed by `tick`.
    fn visible(&self, tick: u64) -> Vec<String> {
        let mut left = self.shown(tick);
        let mut out = Vec::new();
        for l in &self.lines {
            if left == 0 {
                break;
            }
            let n = l.chars().count();
            if left > n {
                out.push(l.clone());
                left -= n + 1;
            } else {
                out.push(l.chars().take(left).collect());
                break;
            }
        }
        out
    }
}

pub struct Terminal {
    session: Session,
    scrollback: Vec<String>,
    typing: Option<Typing>,
    console: Console,
    denied_at: Option<u64>,
}

impl Terminal {
    /// Powers the terminal on: a boot the first time this tile is used in a session.
    pub fn open(st: &mut AppState, session: Session, key: (Place, Pos)) -> Terminal {
        let fresh = st.journal.booted.insert(key);
        let typing = fresh.then(|| Typing {
            lines: access::boot_lines(st, &session),
            started: st.tick,
            leaving: false,
        });
        Terminal {
            session,
            scrollback: Vec::new(),
            typing,
            console: Console::default(),
            denied_at: None,
        }
    }

    /// Everything on the screen at `tick`: the scrollback and the reply as far as typed.
    #[cfg(test)]
    pub(crate) fn text(&self, tick: u64) -> Vec<String> {
        let mut out = self.scrollback.clone();
        if let Some(t) = &self.typing {
            out.extend(t.visible(tick));
        }
        out
    }

    #[cfg(test)]
    pub(crate) fn input(&self) -> &str {
        &self.console.input
    }

    fn badge(&self) -> &'static str {
        match self.session {
            Session::District { .. } => "MUNI-OS",
            Session::Rig { .. } => "HOME RIG",
        }
    }

    /// Moves a finished reply into the scrollback.
    fn settle(&mut self, tick: u64) -> bool {
        let Some(t) = &self.typing else {
            return false;
        };
        if !t.done(tick) {
            return false;
        }
        let t = self.typing.take().unwrap();
        self.scrollback.extend(t.lines);
        t.leaving
    }

    fn run(&mut self, st: &mut AppState, line: String) {
        let prompt = access::prompt(st, &self.session);
        self.scrollback
            .push(format!("{prompt}{}", line.to_uppercase()));
        let reply = access::run(st, &mut self.session, &line);
        if reply.denied {
            self.denied_at = Some(st.tick);
        }
        let leaving = line.trim().eq_ignore_ascii_case("exit");
        self.typing = Some(Typing {
            lines: reply.lines,
            started: st.tick,
            leaving,
        });
    }

    fn rect(area: Rect) -> Rect {
        let w = CRT_W.min(area.width);
        let h = CRT_H.min(area.height);
        Rect::new(
            area.x + area.width.saturating_sub(w) / 2,
            area.y + area.height.saturating_sub(h) / 2,
            w,
            h,
        )
    }
}

fn pop() -> ScreenCmd {
    ScreenCmd::Pop {
        fx: TransitionKind::None,
    }
}

impl Screen for Terminal {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        // A reply that has finished typing settles; EXIT leaves once typed.
        if self.settle(st.tick) {
            return pop();
        }
        // One still typing: any key finishes it and does nothing else.
        if let Some(t) = &mut self.typing {
            if keys.is_empty() {
                return ScreenCmd::Stay;
            }
            t.started = 0;
            if self.settle(st.tick) {
                return pop();
            }
            return ScreenCmd::Stay;
        }
        let verbs = access::verbs(st, &self.session);
        for k in keys {
            let complete = |head: &str, word: &str| -> Vec<String> {
                if !head.is_empty() {
                    return Vec::new();
                }
                let w = word.to_uppercase();
                verbs
                    .iter()
                    .filter(|v| v.starts_with(&w))
                    .map(|v| v.to_string())
                    .collect()
            };
            match self.console.key(*k, &complete) {
                ConsoleEvent::Run(line) => {
                    self.run(st, line);
                    break;
                }
                ConsoleEvent::Close => return pop(),
                ConsoleEvent::None => {}
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let r = Self::rect(f.area());
        if r.width < CRT_W || r.height < CRT_H {
            return;
        }
        let tick = st.tick;
        let led = if self.typing.as_ref().is_some_and(|t| !t.done(tick)) {
            Led::Green
        } else if self.denied_at.is_some_and(|d| tick <= d + DENIED_TICKS) {
            Led::Red
        } else {
            Led::Amber
        };
        let buf = f.buffer_mut();
        let screen = device::crt(buf, r, self.badge(), led);
        let green = Style::default().fg(device::PHOSPHOR).bg(device::LCD_BG);
        let dim = green.add_modifier(Modifier::DIM);
        let mut rows: Vec<String> = self.scrollback.clone();
        if let Some(t) = &self.typing {
            rows.extend(t.visible(tick));
        }
        let typing = self.typing.as_ref().is_some_and(|t| !t.done(tick));
        let mut tail = Vec::new();
        if !typing {
            if let Some(h) = &self.console.hint {
                tail.push((h.clone(), dim));
            }
            tail.push((
                format!(
                    "{}{}▮",
                    access::prompt(st, &self.session),
                    self.console.input
                ),
                green,
            ));
        }
        let avail = screen.height as usize;
        let keep = avail.saturating_sub(tail.len());
        let start = rows.len().saturating_sub(keep);
        let width = screen.width.saturating_sub(2) as usize;
        let mut y = screen.y;
        for row in rows.iter().skip(start) {
            let line: String = row.chars().take(width).collect();
            crate::ui::chrome::put(buf, screen.x + 1, y, &line, green, screen.right());
            y += 1;
        }
        for (text, style) in tail {
            let line: String = text.chars().take(width).collect();
            crate::ui::chrome::put(buf, screen.x + 1, y, &line, style, screen.right());
            y += 1;
        }
        device::scanlines(buf, screen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::Session;
    use crate::app::{AppState, Screen, ScreenCmd};
    use crate::screens::testutil::{buffer_text, key};
    use citygen::{BuildingKind, GenParams, Generator, NodeKind, SizePreset, StratumKind};
    use crossterm::event::KeyCode;
    use ratatui::Terminal as Term;
    use ratatui::backend::TestBackend;

    fn state() -> (AppState, citygen::DistrictId, citygen::BuildingId) {
        let mut st = AppState::new();
        st.enter_city(
            Generator::run(GenParams {
                seed: 6,
                size: SizePreset::Custom { w: 120, h: 60 },
                districts: 8,
                ..Default::default()
            })
            .unwrap(),
        );
        let (d, b) = {
            let city = st.city.as_ref().unwrap();
            let b = city
                .buildings
                .iter()
                .find(|b| {
                    matches!(b.kind, BuildingKind::Apartment | BuildingKind::Megablock)
                        && city
                            .net
                            .nodes
                            .iter()
                            .any(|n| n.building == Some(b.id) && n.kind == NodeKind::Server)
                })
                .unwrap();
            (b.district, b.id)
        };
        st.player = Some(crate::walker::Player::spawn(st.city.as_ref().unwrap(), 1));
        (st, d, b)
    }

    fn district(d: citygen::DistrictId) -> Session {
        Session::District {
            district: d,
            connected: None,
        }
    }

    fn draw(t: &Terminal, st: &AppState) -> String {
        let mut term = Term::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| t.draw(st, f)).unwrap();
        buffer_text(term.backend().buffer())
    }

    fn typed(s: &str) -> Vec<crossterm::event::KeyEvent> {
        s.chars()
            .map(|c| key(KeyCode::Char(c)))
            .chain(std::iter::once(key(KeyCode::Enter)))
            .collect()
    }

    const KEY_A: (citygen::Place, citygen::Pos) = (
        citygen::Place::Stratum(StratumKind::Street),
        citygen::Pos { x: 3, y: 4 },
    );
    const KEY_B: (citygen::Place, citygen::Pos) = (
        citygen::Place::Stratum(StratumKind::Street),
        citygen::Pos { x: 9, y: 9 },
    );

    #[test]
    fn boots_once_per_terminal_and_types_the_reply() {
        let (mut st, d, _) = state();
        let t = Terminal::open(&mut st, district(d), KEY_A);
        assert!(
            t.text(st.tick + 10_000)
                .iter()
                .any(|l| l.contains("MUNI-OS")),
            "{:?}",
            t.text(st.tick + 10_000)
        );
        let again = Terminal::open(&mut st, district(d), KEY_A);
        assert!(
            !again
                .text(st.tick + 10_000)
                .iter()
                .any(|l| l.contains("MUNI-OS"))
        );
        let other = Terminal::open(&mut st, district(d), KEY_B);
        assert!(
            other
                .text(st.tick + 10_000)
                .iter()
                .any(|l| l.contains("MUNI-OS"))
        );
        let mut t = again;
        st.tick += 1000;
        t.update(&mut st, &typed("help"));
        let early = t.text(st.tick).join("\n");
        assert!(early.contains("NET > HELP"), "{early}");
        st.tick += 1000;
        let late = t.text(st.tick).join("\n");
        assert!(late.len() > early.len(), "the reply types out");
        assert!(late.contains("SCAN"), "{late}");
        // Mid-reply, a key finishes it rather than typing.
        let mut t2 = Terminal::open(&mut st, district(d), KEY_A);
        t2.update(&mut st, &typed("help"));
        t2.update(&mut st, &[key(KeyCode::Char('x'))]);
        assert!(t2.text(st.tick).join("\n").contains("EXIT"));
        assert_eq!(t2.input(), "");
    }

    #[test]
    fn the_prompt_shows_the_tier_and_esc_leaves() {
        let (mut st, d, b) = state();
        let mut t = Terminal::open(&mut st, district(d), KEY_A);
        st.tick += 1000;
        let text = draw(&t, &st);
        let dn = st.city.as_ref().unwrap().district(d).name.to_uppercase();
        assert!(text.contains(&format!("{dn} NET > ")), "{text}");
        assert!(text.contains("MUNI-OS"), "{text}");
        assert!(matches!(
            t.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
        let mut c = Terminal::open(
            &mut st,
            Session::District {
                district: d,
                connected: Some(b),
            },
            KEY_B,
        );
        st.tick += 1000;
        let name = crate::net::building_name(&st, b).to_uppercase();
        assert!(draw(&c, &st).contains(&format!("{name} [GUEST] > ")));
        c.update(&mut st, &typed("exit"));
        st.tick += 1000;
        assert!(matches!(c.update(&mut st, &[]), ScreenCmd::Pop { .. }));
    }

    #[test]
    fn a_session_walks_scan_connect_who_on_screen() {
        let (mut st, d, b) = state();
        let mut t = Terminal::open(&mut st, district(d), KEY_A);
        st.tick += 1000;
        t.update(&mut st, &typed("scan"));
        st.tick += 10_000;
        let name = crate::net::building_name(&st, b).to_uppercase();
        let text = t.text(st.tick).join("\n");
        let row = text
            .lines()
            .find(|l| l.contains(&name))
            .unwrap_or_else(|| panic!("{text}"));
        let n: usize = row.split_whitespace().next().unwrap().parse().unwrap();
        t.update(&mut st, &typed(&format!("connect {n}")));
        st.tick += 10_000;
        t.update(&mut st, &typed("who"));
        st.tick += 10_000;
        let drawn = draw(&t, &st);
        assert!(drawn.contains(&format!("{name} [GUEST] > ")), "{drawn}");
        assert!(
            drawn.contains("AT HOME") || drawn.contains("NOBODY INSIDE"),
            "{drawn}"
        );
    }
}
