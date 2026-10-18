//! The Access Terminal on screen: a beige CRT over the map with a scrollback, a
//! prompt, and replies that type themselves out. One screen hosts either session.

use crate::access::{self, Session};
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::ui::console::{Console, ConsoleEvent};
use crate::ui::device::{self, CRT_H, CRT_W, Device, Led, Look};
use citygen::{Place, Pos};
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

/// Ticks the LED stays red after a denial has typed.
const DENIED_TICKS: u64 = 30;
const TICKS_PER_SECOND: u64 = 30;
/// Characters a screen row holds: the glass less a cell each side.
const COLUMNS: usize = (CRT_W - 4 - 2) as usize;

/// A reply typing itself out: its lines, and the tick it started.
struct Typing {
    lines: Vec<String>,
    started: u64,
    /// Characters a second.
    cps: u64,
    /// A key pressed mid-reply: it shows whole from now on.
    finished: bool,
    /// Set when the reply was EXIT: pop once it has typed.
    leaving: bool,
    /// The reply refused something: the LED reads red while it types.
    denied: bool,
}

impl Typing {
    fn new(lines: Vec<String>, started: u64, cps: u64) -> Typing {
        let lines = lines
            .iter()
            .flat_map(|l| {
                // Short lines keep their spacing (SCAN's columns); long ones wrap.
                if l.chars().count() <= COLUMNS {
                    vec![l.clone()]
                } else {
                    crate::ui::sidebar::wrap_text(l, COLUMNS)
                }
            })
            .collect();
        Typing {
            lines,
            started,
            cps: cps.max(1),
            finished: false,
            leaving: false,
            denied: false,
        }
    }

    /// How many characters have appeared by `tick`.
    fn shown(&self, tick: u64) -> usize {
        if self.finished {
            return self.total();
        }
        (tick.saturating_sub(self.started) * self.cps / TICKS_PER_SECOND) as usize
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
    look: Look,
    session: Session,
    scrollback: Vec<String>,
    typing: Option<Typing>,
    console: Console,
    denied_at: Option<u64>,
    /// Rows scrolled up from the bottom of the scrollback.
    scroll: usize,
}

impl Terminal {
    /// Powers the terminal on: a boot the first time this tile is used in a session.
    pub fn open(st: &mut AppState, session: Session, key: (Place, Pos)) -> Terminal {
        let device = match session {
            Session::District { district, .. } => Device::Terminal(district),
            Session::Rig { .. } => Device::Rig,
        };
        let look = Look::pick(st, device);
        Self::with_look(st, session, key, look)
    }

    /// Powers on with a given look.
    pub fn with_look(
        st: &mut AppState,
        session: Session,
        key: (Place, Pos),
        look: Look,
    ) -> Terminal {
        let fresh = st.journal.booted.insert(key);
        let typing = fresh.then(|| {
            Typing::new(
                access::boot_lines(st, &session, &look),
                st.tick,
                look.style.cps,
            )
        });
        Terminal {
            look,
            session,
            scrollback: Vec::new(),
            typing,
            console: Console::default(),
            denied_at: None,
            scroll: 0,
        }
    }

    /// Puts lines on the screen as a reply would: wrapped, typed out.
    #[cfg(test)]
    pub(crate) fn show(&mut self, st: &AppState, lines: Vec<String>) {
        self.typing = Some(Typing::new(lines, st.tick, self.look.style.cps));
        self.scroll = 0;
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

    fn badge(&self) -> String {
        match self.session {
            Session::District { .. } => self.look.terminal_badge(),
            Session::Rig { .. } => format!("{} · HOME RIG", self.look.terminal_badge()),
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
        if t.denied {
            self.denied_at = Some(tick);
        }
        self.scrollback.extend(t.lines);
        t.leaving
    }

    /// The rows a screen of `height` shows, with its markers, given the typing so far.
    fn page(&self, rows: &[String], height: usize) -> (Vec<String>, usize) {
        let keep = height.max(1);
        let max_scroll = rows.len().saturating_sub(keep);
        let scroll = self.scroll.min(max_scroll);
        let end = rows.len() - scroll;
        let start = end.saturating_sub(keep);
        let mut page: Vec<String> = rows[start..end].to_vec();
        if start > 0 && !page.is_empty() {
            page[0] = "▲ MORE ABOVE · PGUP".to_string();
        }
        if scroll > 0 && !page.is_empty() {
            let last = page.len() - 1;
            page[last] = "▼ MORE BELOW · PGDN".to_string();
        }
        (page, keep)
    }

    fn run(&mut self, st: &mut AppState, line: String) {
        let prompt = access::prompt(st, &self.session);
        self.scrollback
            .push(format!("{prompt}{}", line.to_uppercase()));
        let reply = access::run(st, &mut self.session, &line);
        let mut lines = reply.lines;
        if reply.denied
            && let Some(first) = &self.look.style.denied
        {
            lines.insert(0, first.clone());
        }
        let mut typing = Typing::new(lines, st.tick, self.look.style.cps);
        typing.leaving = line.trim().eq_ignore_ascii_case("exit");
        typing.denied = reply.denied;
        self.typing = Some(typing);
        self.scroll = 0;
    }

    pub(crate) fn rect(area: Rect) -> Rect {
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
            t.finished = true;
            if self.settle(st.tick) {
                return pop();
            }
            return ScreenCmd::Stay;
        }
        let verbs = access::verbs(st, &self.session);
        let page = (CRT_H as usize).saturating_sub(6);
        for k in keys {
            match k.code {
                crossterm::event::KeyCode::PageUp => {
                    self.scroll += page;
                    continue;
                }
                crossterm::event::KeyCode::PageDown => {
                    self.scroll = self.scroll.saturating_sub(page);
                    continue;
                }
                _ => {}
            }
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
        let typing_now = self.typing.as_ref().filter(|t| !t.done(tick));
        let denied_typing = typing_now.is_some_and(|t| t.denied);
        let led = match typing_now {
            Some(t) if t.denied => Led::Fail,
            Some(_) => Led::Ok,
            None if self.denied_at.is_some_and(|d| tick <= d + DENIED_TICKS) => Led::Fail,
            None => Led::Idle,
        };
        let buf = f.buffer_mut();
        let screen = device::crt(buf, r, &self.badge(), led, &self.look);
        let glass = self.look.glass();
        let flicker = self.look.flickering(tick);
        let tone = |c: ratatui::style::Color| {
            let s = Style::default().fg(c).bg(glass);
            if flicker { device::dimmed(s) } else { s }
        };
        let live = tone(self.look.text());
        let deny = tone(device::DENY);
        let quiet = tone(self.look.text_dim());
        let bold = live.add_modifier(Modifier::BOLD);
        let mut rows: Vec<(String, Style)> =
            self.scrollback.iter().map(|l| (l.clone(), live)).collect();
        if let Some(t) = &self.typing {
            let style = if t.denied { deny } else { live };
            rows.extend(t.visible(tick).into_iter().map(|l| (l, style)));
        }
        let typing = typing_now.is_some();
        let mut tail = Vec::new();
        if !typing {
            if let Some(h) = &self.console.hint {
                tail.push((h.clone(), quiet));
            }
            tail.push((
                format!(
                    "{}{}▮",
                    access::prompt(st, &self.session),
                    self.console.input
                ),
                bold,
            ));
        }
        let avail = screen.height as usize;
        let texts: Vec<String> = rows.iter().map(|(l, _)| l.clone()).collect();
        let (page, _) = self.page(&texts, avail.saturating_sub(tail.len()));
        // The page's rows carry the style of the row they came from; markers are quiet.
        let styles: Vec<Style> = page
            .iter()
            .map(|l| {
                if l.starts_with('▲') || l.starts_with('▼') {
                    quiet
                } else {
                    rows.iter()
                        .rev()
                        .find(|(t, _)| t == l)
                        .map(|(_, s)| *s)
                        .unwrap_or(live)
                }
            })
            .collect();
        let width = screen.width.saturating_sub(2) as usize;
        let mut y = screen.y;
        for (row, style) in page.iter().zip(styles) {
            let line: String = row.chars().take(width).collect();
            crate::ui::chrome::put(buf, screen.x + 1, y, &line, style, screen.right());
            y += 1;
        }
        for (text, style) in tail {
            let line: String = text.chars().take(width).collect();
            crate::ui::chrome::put(buf, screen.x + 1, y, &line, style, screen.right());
            y += 1;
        }
        // A refusal scatters a little garbage over the glass, fading as it types.
        if denied_typing && let Some(t) = typing_now {
            let progress = (tick.saturating_sub(t.started)) as f32 / DENIED_TICKS as f32;
            device::glitch(buf, screen, tick, 0.7 + 0.3 * progress.min(1.0));
        }
        if self.look.style.scanlines {
            device::scanlines(buf, screen);
        }
        device::dead_cells(buf, r, screen, &self.look);
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
        let boot = crate::access::boot_lines(
            &st,
            &district(d),
            &crate::ui::device::Look::pick(&st, crate::ui::device::Device::Terminal(d)),
        );
        let t = Terminal::open(&mut st, district(d), KEY_A);
        assert!(
            t.text(st.tick + 10_000).iter().any(|l| *l == boot[0]),
            "{:?}",
            t.text(st.tick + 10_000)
        );
        let again = Terminal::open(&mut st, district(d), KEY_A);
        assert!(!again.text(st.tick + 10_000).iter().any(|l| *l == boot[0]));
        let other = Terminal::open(&mut st, district(d), KEY_B);
        assert!(other.text(st.tick + 10_000).iter().any(|l| *l == boot[0]));
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
        assert!(text.contains(&t.look.terminal_badge()), "{text}");
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
        let name = crate::access::building_label(&st, b);
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
        let name = crate::access::building_label(&st, b);
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

    #[test]
    fn long_replies_scroll_with_page_keys_and_wide_lines_wrap() {
        let (mut st, d, _) = state();
        let mut t = Terminal::open(&mut st, district(d), KEY_A);
        st.tick += 1000;
        // Forty numbered lines, one of them far wider than the screen.
        let mut lines: Vec<String> = (1..=40).map(|i| format!("ROW {i:02}")).collect();
        lines.push("W".repeat(150));
        t.show(&st, lines);
        st.tick += 100_000;
        let drawn = draw(&t, &st);
        assert!(
            drawn.contains("ROW 40") && !drawn.contains("ROW 01"),
            "{drawn}"
        );
        let all = t.text(st.tick);
        let wide: Vec<&str> = all
            .iter()
            .filter(|l| l.starts_with("WWW"))
            .map(|s| s.as_str())
            .collect();
        assert!(wide.len() >= 3, "wrapped into rows: {}", wide.len());
        assert!(wide.iter().all(|l| l.chars().count() <= 66));
        t.update(&mut st, &[key(KeyCode::PageUp)]);
        let up = draw(&t, &st);
        assert!(up.contains("▲"), "a more marker: {up}");
        t.update(&mut st, &[key(KeyCode::PageUp), key(KeyCode::PageUp)]);
        let top = draw(&t, &st);
        assert!(top.contains("ROW 01"), "{top}");
        t.update(
            &mut st,
            &[
                key(KeyCode::PageDown),
                key(KeyCode::PageDown),
                key(KeyCode::PageDown),
            ],
        );
        assert!(draw(&t, &st).contains("ROW 40"));
    }

    #[test]
    fn a_denied_reply_lights_the_led_red_and_any_key_finishes_early_in_a_run() {
        let (mut st, d, b) = state();
        let mut t = Terminal::open(
            &mut st,
            Session::District {
                district: d,
                connected: Some(b),
            },
            KEY_A,
        );
        st.tick += 1000;
        t.update(&mut st, &typed("door unlock"));
        st.tick += 2;
        let mut term = Term::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| t.draw(&st, f)).unwrap();
        let r = Terminal::rect(Rect::new(0, 0, 120, 40));
        let led = &term.backend().buffer()[(r.right() - 3, r.y + 1)];
        assert_eq!(led.fg, device::DENY, "magenta while a denial types");
        // A fresh run: tick 3, a long reply, one key finishes it.
        let mut st2 = st;
        st2.tick = 3;
        let mut t2 = Terminal::open(&mut st2, district(d), KEY_B);
        // The boot is still typing: one key finishes it, then HELP, then a key.
        t2.update(&mut st2, &[key(KeyCode::Char(' '))]);
        t2.update(&mut st2, &typed("help"));
        t2.update(&mut st2, &[key(KeyCode::Char('x'))]);
        assert!(t2.text(st2.tick).join("\n").contains("EXIT"));
    }

    #[test]
    fn the_theme_sets_the_voice_and_the_speed() {
        let (mut st, d, b) = state();
        let corp = crate::ui::device::Look::named(&st, "megacorp", Some(d), 1);
        let mut t = Terminal::with_look(
            &mut st,
            Session::District {
                district: d,
                connected: Some(b),
            },
            KEY_A,
            corp,
        );
        st.tick += 10_000;
        t.update(&mut st, &typed("door unlock"));
        st.tick += 10_000;
        let text = t.text(st.tick).join("\n");
        assert!(
            text.contains("YOUR CLEARANCE DOES NOT PERMIT THIS ACTION"),
            "{text}"
        );
        assert!(text.contains("ACCESS DENIED"), "{text}");
        let street = crate::ui::device::Look::named(&st, "street", Some(d), 1);
        let slow = Terminal::with_look(&mut st, district(d), KEY_B, street);
        let corp = crate::ui::device::Look::named(&st, "megacorp", Some(d), 1);
        let fast = Terminal::with_look(
            &mut st,
            district(d),
            (
                citygen::Place::Stratum(StratumKind::Street),
                citygen::Pos { x: 5, y: 5 },
            ),
            corp,
        );
        let t0 = st.tick;
        let shown = |t: &Terminal| {
            t.text(t0 + 10)
                .iter()
                .map(|l| l.chars().count())
                .sum::<usize>()
        };
        assert!(
            shown(&slow) < shown(&fast),
            "street types slower than megacorp"
        );
    }
}
