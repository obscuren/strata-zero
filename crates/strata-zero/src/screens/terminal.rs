//! The Access Terminal on screen: a beige CRT over the map running a HELIX/OS shell,
//! with a scrollback, a prompt, replies that type themselves out, and nano on the glass.

use crate::access;
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::helix::bin::{AppIo, new_shell};
use crate::helix::hosts::{device_of, ensure_host};
use crate::helix::nano::{Nano, NanoEvent};
use crate::helix::shell::{self, HostId, Io, Shell};
use crate::helix::vfs::Kind;
use crate::ui::console::{Console, ConsoleEvent};
use crate::ui::device::{self, CRT_H, CRT_W, Led, Look};
use citygen::{Place, Pos};
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use std::cell::RefCell;

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
    /// Set when the reply was `exit` at the origin: pop once it has typed.
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
    shell: Shell,
    /// nano, while it owns the keys and the glass.
    nano: Option<Nano>,
    /// The shell's prompt, refreshed after every line (`draw` has no `AppIo`).
    prompt: String,
    scrollback: Vec<String>,
    typing: Option<Typing>,
    console: Console,
    denied_at: Option<u64>,
    /// Rows scrolled up from the bottom of the scrollback.
    scroll: usize,
}

impl Terminal {
    /// Powers the terminal on: a boot the first time this tile is used in a session.
    pub fn open(st: &mut AppState, origin: HostId, key: (Place, Pos)) -> Terminal {
        let look = Look::pick(st, device_of(origin));
        Self::with_look(st, origin, key, look)
    }

    /// Powers on with a given look: logged in on `origin` as your standing allows.
    pub fn with_look(st: &mut AppState, origin: HostId, key: (Place, Pos), look: Look) -> Terminal {
        let shell = new_shell(st, origin);
        let fresh = st.journal.booted.insert(key);
        let typing = fresh.then(|| {
            let mut lines = access::boot_lines(st, origin, &look);
            lines.extend(motd(st, &shell));
            Typing::new(lines, st.tick, look.style.cps)
        });
        let prompt = shell::prompt(&AppIo { st }, &shell);
        Terminal {
            look,
            shell,
            nano: None,
            prompt,
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
        self.look.terminal_badge()
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

    /// The rows a screen of `height` shows, given the scroll.
    fn page(&self, rows: &[String], height: usize) -> (Vec<String>, usize) {
        let keep = height.max(1);
        let max_scroll = rows.len().saturating_sub(keep);
        let scroll = self.scroll.min(max_scroll);
        let end = rows.len() - scroll;
        let start = end.saturating_sub(keep);
        (rows[start..end].to_vec(), keep)
    }

    fn run(&mut self, st: &mut AppState, line: String) {
        self.scrollback.push(format!("{}{line}", self.prompt));
        let out = shell::run(&mut AppIo { st }, &mut self.shell, &line);
        if out.clear {
            self.scrollback.clear();
        }
        let mut lines = out.lines;
        if out.denied
            && let Some(first) = &self.look.style.denied
        {
            lines.insert(0, first.clone());
        }
        if let Some(path) = out.open_nano {
            let sh = &self.shell;
            match Nano::open(
                &ensure_host(st, sh.host).fs,
                &sh.user,
                &sh.cwd,
                &sh.home,
                &path,
            ) {
                Ok(n) => {
                    // what nano shows is read: the world learns from it as from `cat`
                    let read = match ensure_host(st, sh.host)
                        .fs
                        .read(&sh.user, "/", "/", &n.path)
                    {
                        Ok(Kind::File(text)) => Some(text.clone()),
                        _ => None,
                    };
                    if let Some(text) = read {
                        AppIo { st }.on_read(sh.host, &n.path, &text);
                    }
                    self.nano = Some(n);
                }
                Err(e) => lines.push(e),
            }
        }
        // Replies land at once; only the power-on boot types itself out.
        let mut typing = Typing::new(lines, st.tick, self.look.style.cps);
        typing.finished = true;
        typing.leaving = out.exit;
        typing.denied = out.denied;
        self.typing = Some(typing);
        self.scroll = 0;
        self.prompt = shell::prompt(&AppIo { st }, &self.shell);
    }

    /// One key to nano; true once it has closed and given the keys back.
    fn nano_key(&mut self, st: &mut AppState, k: KeyEvent) -> bool {
        let Some(nano) = &mut self.nano else {
            return true;
        };
        let fs = &mut ensure_host(st, self.shell.host).fs;
        match nano.key(fs, &self.shell.user, k) {
            NanoEvent::Stay => false,
            NanoEvent::Closed(lines) => {
                self.nano = None;
                self.scrollback.extend(lines);
                self.scroll = 0;
                true
            }
        }
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

/// The host's `/etc/motd` as the shell's user reads it; nothing if they cannot.
fn motd(st: &mut AppState, sh: &Shell) -> Vec<String> {
    let fs = &ensure_host(st, sh.host).fs;
    match fs.read(&sh.user, &sh.cwd, &sh.home, "/etc/motd") {
        Ok(Kind::File(text)) => text.lines().map(str::to_string).collect(),
        _ => Vec::new(),
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
        // nano open: every key is nano's until it closes.
        if self.nano.is_some() {
            for k in keys {
                if self.nano_key(st, *k) {
                    break;
                }
            }
            return ScreenCmd::Stay;
        }
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
            let event = {
                let io = RefCell::new(AppIo { st: &mut *st });
                let sh = &self.shell;
                let complete = |head: &str, word: &str| -> Vec<String> {
                    shell::complete(&mut *io.borrow_mut(), sh, head, word)
                };
                self.console.key(*k, &complete)
            };
            match event {
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
        // nano owns the glass while it is open; the bezel, LED and scanlines stay.
        if let Some(nano) = &self.nano {
            nano.draw(buf, screen, &self.look);
            if self.look.style.scanlines {
                device::scanlines(buf, screen);
            }
            device::dead_cells(buf, r, screen, &self.look);
            return;
        }
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
            tail.push((format!("{}{}▮", self.prompt, self.console.input), bold));
        }
        let avail = screen.height as usize;
        let texts: Vec<String> = rows.iter().map(|(l, _)| l.clone()).collect();
        let (page, _) = self.page(&texts, avail.saturating_sub(tail.len()));
        // The page's rows carry the style of the row they came from.
        let styles: Vec<Style> = page
            .iter()
            .map(|l| {
                {
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
    use crate::app::{AppState, Screen, ScreenCmd};
    use crate::helix::hosts::hostname;
    use crate::screens::testutil::{buffer_text, key};
    use crate::ui::device::Device;
    use citygen::{BuildingKind, GenParams, Generator, NodeKind, SizePreset, StratumKind};
    use crossterm::event::{KeyCode, KeyModifiers};
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

    fn draw(t: &Terminal, st: &AppState) -> String {
        let mut term = Term::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| t.draw(st, f)).unwrap();
        buffer_text(term.backend().buffer())
    }

    fn typed(s: &str) -> Vec<KeyEvent> {
        s.chars()
            .map(|c| key(KeyCode::Char(c)))
            .chain(std::iter::once(key(KeyCode::Enter)))
            .collect()
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    /// Lets whatever is typing settle, types `line`, lets the reply settle; the
    /// command the settling update returned.
    fn say(t: &mut Terminal, st: &mut AppState, line: &str) -> ScreenCmd {
        st.tick += 10_000;
        t.update(st, &[]);
        t.update(st, &typed(line));
        st.tick += 10_000;
        t.update(st, &[])
    }

    /// The reply to the last line typed: everything after its prompt row.
    fn reply(t: &Terminal, st: &AppState, line: &str) -> String {
        let text = t.text(st.tick);
        let at = text
            .iter()
            .rposition(|l| l.ends_with(&format!("$ {line}")) || l.ends_with(&format!("# {line}")))
            .unwrap_or_else(|| panic!("no prompt row for {line}: {text:?}"));
        text[at + 1..].join("\n")
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
    fn boots_with_motd_and_shows_a_unix_prompt() {
        let (mut st, d, _) = state();
        let origin = HostId::District(d);
        let boot = access::boot_lines(&st, origin, &Look::pick(&st, Device::Terminal(d)));
        let t = Terminal::open(&mut st, origin, KEY_A);
        let text = t.text(st.tick + 10_000);
        let at = text.iter().position(|l| *l == boot[0]).expect("the boot");
        let motd = text
            .iter()
            .position(|l| l == "Authorised users only. Sessions are logged.")
            .unwrap_or_else(|| panic!("the motd: {text:?}"));
        assert!(at < motd, "the boot, then the motd: {text:?}");
        let again = Terminal::open(&mut st, origin, KEY_A);
        assert!(!again.text(st.tick + 10_000).iter().any(|l| *l == boot[0]));
        let other = Terminal::open(&mut st, origin, KEY_B);
        assert!(other.text(st.tick + 10_000).iter().any(|l| *l == boot[0]));
        let mut t = again;
        st.tick += 1000;
        let slug = hostname(&st, origin);
        let drawn = draw(&t, &st);
        assert!(drawn.contains(&format!("guest@{slug}:/$ ")), "{drawn}");
        assert!(drawn.contains(&t.look.terminal_badge()), "{drawn}");
        // what you type stays as typed, and the reply is there at once
        t.update(&mut st, &typed("help"));
        let early = t.text(st.tick).join("\n");
        assert!(early.contains(&format!("guest@{slug}:/$ help")), "{early}");
        assert!(early.contains("ssh") && early.contains("whoami"), "{early}");
        st.tick += 1000;
        let late = t.text(st.tick).join("\n");
        assert_eq!(late, early, "nothing types out after a reply");
        // the one-time boot still types, and a key finishes it
        let mut t2 = Terminal::open(
            &mut st,
            origin,
            (
                citygen::Place::Stratum(StratumKind::Street),
                citygen::Pos { x: 6, y: 6 },
            ),
        );
        let mid = t2.text(st.tick).join("\n");
        assert!(!mid.contains("Authorised users only"), "{mid}");
        t2.update(&mut st, &[key(KeyCode::Char('x'))]);
        assert!(
            t2.text(st.tick)
                .join("\n")
                .contains("Authorised users only")
        );
        assert_eq!(t2.input(), "");
        // Tab completes through the shell: a path, as typed
        let mut keys: Vec<KeyEvent> = "ls /ne".chars().map(|c| key(KeyCode::Char(c))).collect();
        keys.push(key(KeyCode::Tab));
        t2.update(&mut st, &keys);
        assert_eq!(t2.input(), "ls /net/");
        assert!(matches!(
            t2.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
    }

    #[test]
    fn a_session_walks_ls_net_ssh_and_a_denied_cat() {
        let (mut st, d, b) = state();
        let mut t = Terminal::open(&mut st, HostId::District(d), KEY_A);
        say(&mut t, &mut st, "ls /net");
        let name = hostname(&st, HostId::Building(b));
        assert!(
            reply(&t, &st, "ls /net").lines().any(|l| l == name),
            "{:?}",
            t.text(st.tick)
        );
        let ssh = format!("ssh {name}");
        say(&mut t, &mut st, &ssh);
        assert!(reply(&t, &st, &ssh).contains("CONNECTED TO"));
        let drawn = draw(&t, &st);
        assert!(drawn.contains(&format!("guest@{name}:/$ ")), "{drawn}");
        say(&mut t, &mut st, "ls /home");
        let homes = reply(&t, &st, "ls /home");
        let home = homes.split_whitespace().next().expect("a resident's home");
        let line = format!("cat /home/{home}/notes.txt");
        say(&mut t, &mut st, &line);
        assert!(
            reply(&t, &st, &line).ends_with("Permission denied"),
            "{:?}",
            t.text(st.tick)
        );
        assert!(t.denied_at.is_some(), "the LED lit for the denial");
        say(&mut t, &mut st, "whoami");
        assert_eq!(reply(&t, &st, "whoami"), "guest");
        assert!(matches!(say(&mut t, &mut st, "exit"), ScreenCmd::Stay));
        assert_eq!(t.shell.host, HostId::District(d));
        let slug = hostname(&st, HostId::District(d));
        assert!(draw(&t, &st).contains(&format!("guest@{slug}:/$ ")));
        assert!(matches!(
            say(&mut t, &mut st, "exit"),
            ScreenCmd::Pop { .. }
        ));
    }

    #[test]
    fn clear_empties_the_scrollback() {
        let (mut st, d, _) = state();
        let mut t = Terminal::open(&mut st, HostId::District(d), KEY_A);
        say(&mut t, &mut st, "ls /net");
        assert!(t.text(st.tick).len() > 3);
        say(&mut t, &mut st, "clear");
        assert!(t.text(st.tick).is_empty(), "{:?}", t.text(st.tick));
        let drawn = draw(&t, &st);
        assert!(!drawn.contains("ls /net"), "{drawn}");
        assert!(drawn.contains("guest@"), "the prompt stays: {drawn}");
    }

    #[test]
    fn nano_takes_the_keys_and_gives_them_back() {
        let (mut st, d, _) = state();
        let mut t = Terminal::open(&mut st, HostId::District(d), KEY_A);
        say(&mut t, &mut st, "nano /tmp/x");
        assert!(t.nano.is_some(), "{:?}", t.text(st.tick));
        let drawn = draw(&t, &st);
        assert!(drawn.contains("HELIX nano"), "nano owns the glass: {drawn}");
        assert!(
            !drawn.contains("nano /tmp/x"),
            "not the scrollback: {drawn}"
        );
        // keys go to nano: Esc does not leave, letters land in the buffer
        let mut keys: Vec<KeyEvent> = "hi there".chars().map(|c| key(KeyCode::Char(c))).collect();
        keys.push(key(KeyCode::Esc));
        assert!(matches!(t.update(&mut st, &keys), ScreenCmd::Stay));
        assert_eq!(t.input(), "", "the console saw none of it");
        t.update(&mut st, &[ctrl('o')]);
        t.update(&mut st, &[key(KeyCode::Enter)]);
        t.update(&mut st, &[ctrl('x')]);
        assert!(t.nano.is_none(), "^X gives the keys back");
        assert!(
            t.text(st.tick).iter().any(|l| l == "[ Wrote 1 lines ]"),
            "{:?}",
            t.text(st.tick)
        );
        say(&mut t, &mut st, "cat /tmp/x");
        assert_eq!(reply(&t, &st, "cat /tmp/x"), "hi there");
        // a directory refuses with nano's own message, and nothing opens
        say(&mut t, &mut st, "nano /tmp");
        assert!(t.nano.is_none());
        assert!(reply(&t, &st, "nano /tmp").contains("is a directory"));
    }

    #[test]
    fn nano_on_notes_learns_their_code() {
        let (mut st, ..) = state();
        let mut found = None;
        let ids: Vec<citygen::BuildingId> = st
            .city
            .as_ref()
            .unwrap()
            .buildings
            .iter()
            .filter(|b| matches!(b.kind, BuildingKind::Apartment | BuildingKind::Megablock))
            .map(|b| b.id)
            .collect();
        'all: for b in ids {
            let h = crate::helix::hosts::ensure_host(&mut st, HostId::Building(b)).clone();
            for u in h.users.iter().filter(|u| u.person.is_some()) {
                let Ok(id) = h.fs.resolve("/", "/", &format!("{}/notes.txt", u.home)) else {
                    continue;
                };
                if let crate::helix::vfs::Kind::File(t) = &h.fs.node(id).kind
                    && t.contains("door is")
                {
                    found = Some((b, u.name.clone(), u.home.clone()));
                    break 'all;
                }
            }
        }
        let (b, user, home) = found.expect("a notes file with a code");
        let mut t = Terminal::open(&mut st, HostId::Building(b), KEY_A);
        t.shell.user = user;
        t.shell.cwd = home.clone();
        t.shell.home = home;
        assert!(st.journal.codes().is_empty());
        say(&mut t, &mut st, "nano ~/notes.txt");
        assert!(t.nano.is_some(), "{:?}", t.text(st.tick));
        assert!(
            !st.journal.codes().is_empty(),
            "nano showed a code unlearned"
        );
    }

    #[test]
    fn the_rig_opens_at_home() {
        let mut st = AppState::new();
        st.enter_city(
            Generator::run(GenParams {
                seed: 8,
                size: SizePreset::Medium,
                districts: 7,
                ..Default::default()
            })
            .unwrap(),
        );
        let home = st.home.unwrap().home;
        crate::credentials::prepare(&mut st, home.building);
        crate::credentials::move_in(&mut st);
        let me = crate::helix::hosts::user_for(&st, HostId::Rig);
        let mut t = Terminal::open(
            &mut st,
            HostId::Rig,
            (
                citygen::Place::Floor {
                    building: home.building,
                    level: home.level,
                },
                home.rig,
            ),
        );
        st.tick += 10_000;
        t.update(&mut st, &[]);
        let drawn = draw(&t, &st);
        assert!(drawn.contains(&format!("{me}@zenbo:~$ ")), "{drawn}");
        say(&mut t, &mut st, "ls mail");
        assert!(
            reply(&t, &st, "ls mail").contains("001-a-favour.txt"),
            "{:?}",
            t.text(st.tick)
        );
    }

    #[test]
    fn a_file_written_on_a_host_survives_ssh_away_and_back() {
        let (mut st, d, _) = state();
        let mut t = Terminal::open(&mut st, HostId::District(d), KEY_A);
        say(&mut t, &mut st, "ssh 1");
        say(&mut t, &mut st, "echo kept > /tmp/x");
        say(&mut t, &mut st, "exit");
        assert_eq!(t.shell.host, HostId::District(d));
        say(&mut t, &mut st, "ssh 1");
        say(&mut t, &mut st, "cat /tmp/x");
        assert_eq!(reply(&t, &st, "cat /tmp/x"), "kept");
    }

    #[test]
    fn every_name_ls_net_prints_can_be_sshed() {
        let (mut st, d, _) = state();
        let mut t = Terminal::open(&mut st, HostId::District(d), KEY_A);
        say(&mut t, &mut st, "ls /net");
        let names: Vec<String> = reply(&t, &st, "ls /net")
            .split_whitespace()
            .map(str::to_string)
            .collect();
        assert!(!names.is_empty());
        for n in names {
            let line = format!("ssh {n}");
            say(&mut t, &mut st, &line);
            assert!(
                reply(&t, &st, &line).contains("CONNECTED TO"),
                "{n}: {:?}",
                t.text(st.tick)
            );
            assert_eq!(
                hostname(&st, t.shell.host),
                n,
                "ssh {n} lands on the host it names"
            );
            say(&mut t, &mut st, "exit");
        }
    }

    #[test]
    fn long_replies_scroll_with_page_keys_and_wide_lines_wrap() {
        let (mut st, d, _) = state();
        let mut t = Terminal::open(&mut st, HostId::District(d), KEY_A);
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
        assert!(
            !up.contains("MORE ABOVE") && !up.contains("MORE BELOW"),
            "no markers: {up}"
        );
        assert!(
            !up.contains("ROW 40") && up.contains("ROW 20"),
            "scrolled up a page: {up}"
        );
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
        let (mut st, d, _) = state();
        let mut t = Terminal::open(&mut st, HostId::District(d), KEY_A);
        say(&mut t, &mut st, "ssh 1");
        st.tick += 1000;
        t.update(&mut st, &typed("door release"));
        st.tick += 2;
        // The next frame settles the reply and lights the LED.
        t.update(&mut st, &[]);
        let mut term = Term::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| t.draw(&st, f)).unwrap();
        let r = Terminal::rect(Rect::new(0, 0, 120, 40));
        let led = &term.backend().buffer()[(r.right() - 3, r.y + 1)];
        assert_eq!(led.fg, device::DENY, "magenta after a denial");
        // A fresh run: tick 3, a long reply, one key finishes it.
        let mut st2 = st;
        st2.tick = 3;
        let mut t2 = Terminal::open(&mut st2, HostId::District(d), KEY_B);
        // The boot is still typing: one key finishes it, then help, then a key.
        t2.update(&mut st2, &[key(KeyCode::Char(' '))]);
        t2.update(&mut st2, &typed("help"));
        t2.update(&mut st2, &[key(KeyCode::Char('x'))]);
        assert!(t2.text(st2.tick).join("\n").contains("whoami"));
    }

    #[test]
    fn the_theme_sets_the_voice_and_the_speed() {
        let (mut st, d, _) = state();
        let corp = Look::named(&st, "megacorp", Some(d), 1);
        let mut t = Terminal::with_look(&mut st, HostId::District(d), KEY_A, corp);
        say(&mut t, &mut st, "ssh 1");
        say(&mut t, &mut st, "door release");
        let text = reply(&t, &st, "door release");
        assert!(
            text.contains("YOUR CLEARANCE DOES NOT PERMIT THIS ACTION"),
            "{text}"
        );
        assert!(
            text.contains("door: /dev/door: Permission denied"),
            "{text}"
        );
        let street = Look::named(&st, "street", Some(d), 1);
        let slow = Terminal::with_look(&mut st, HostId::District(d), KEY_B, street);
        let corp = Look::named(&st, "megacorp", Some(d), 1);
        let fast = Terminal::with_look(
            &mut st,
            HostId::District(d),
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
