//! The keypad on a flat's door: a beige device over the map. Key the code, V to
//! verify; a hit is learned, three misses lock it. Or `k` clips a Keyjack on and cracks
//! the code a beat at a time: space strikes while the marker sits in the lit zone, a hit
//! keys that digit in, a miss is a fault on the door, and the last fault locks the pad and
//! burns the Keyjack's fuse.

use crate::app::{AppState, FPS, Screen, ScreenCmd, TransitionKind};
use crate::credentials::{self, Source, Verdict};
use crate::keyjack::{self, Carried};
use crate::ui::device::{self, DEVICE_H, DEVICE_W, Device, KeyLit, Led, Look};
use crate::ui::slab::{self, Panel, Shows};
use citygen::BuildingId;
use crossterm::event::{KeyCode, KeyEvent};
use game::CopyId;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

/// Ticks an OPEN pad stays before it pops by itself.
pub const OPEN_TICKS: u64 = 24;
/// Ticks DENIED shows after a miss.
pub const DENIED_TICKS: u64 = 18;
/// Ticks the LED flashes red at a refused key.
const REFUSED_TICKS: u64 = 6;
/// The keys in grid order, three a row.
pub(crate) const KEYS: [char; 12] = ['1', '2', '3', '4', '5', '6', '7', '8', '9', 'X', '0', 'V'];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flash {
    None,
    Open(u64),
    Denied(u64),
    Refused(u64),
}

/// How the Keyjack's run ended, while it still hangs on the pad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ended {
    /// The last fault burnt its fuse and locked the pad.
    Blown,
    /// The fourth hit opened the door.
    Cracked,
}

/// The Keyjack clipped on the pad: the copy that runs, the run it makes of this door and
/// the beat it is on. The faults are the door's, in the journal; `faults` is their count
/// as this run last saw it, for the lights (which stay lit on OPEN, though the door's
/// clear).
#[derive(Clone, Debug)]
struct Clip {
    copy: CopyId,
    run: keyjack::Run,
    /// The door's seed, for where its zones sit.
    seed: u64,
    /// The digit being cracked, left to right; 4 once all are.
    beat: usize,
    /// The zone's left cell.
    at: u16,
    /// The tick the marker sets off from the bar's left end. After a miss it lies ahead,
    /// and ERR shows until then.
    from: u64,
    /// The last strike missed.
    missed: bool,
    faults: u8,
    ended: Option<Ended>,
}

impl Clip {
    /// The zone's width on this beat.
    fn width(&self) -> u16 {
        self.run.zones[self.beat.min(3)]
    }

    /// The marker's cell at `tick`: off from the left end at the beat's speed, end to end
    /// and back, a cell's time at each end. A tick is a frame.
    fn marker(&self, tick: u64) -> u16 {
        let bar = self.run.bar.max(1);
        let speed = self.run.speed[self.beat.min(3)] as f64;
        let cells = tick.saturating_sub(self.from) as f64 * speed / FPS as f64;
        let p = (cells % (2 * bar) as f64) as u16;
        if p < bar { p } else { 2 * bar - 1 - p }
    }

    /// A strike with the marker on `cell` lands in the zone.
    fn hits(&self, cell: u16) -> bool {
        cell >= self.at && cell < self.at + self.width()
    }

    /// Puts this beat's zone where the door and its faults so far put it; after a miss,
    /// never back where it was.
    fn place(&mut self) {
        let (w, bar) = (self.width(), self.run.bar);
        let seed = citygen::rng::hash64(self.seed, self.faults as u64);
        let mut at = keyjack::zone_at(seed, self.beat, w, bar);
        let room = bar.saturating_sub(w);
        if self.missed && at == self.at && room > 0 {
            at = (at + room.div_ceil(2)) % (room + 1);
        }
        self.at = at;
    }

    /// The slab's frame at `tick`, with `digits` cracked so far.
    fn panel(&self, tick: u64, digits: &[u8]) -> Panel {
        let shows = match self.ended {
            Some(Ended::Blown) => Shows::Blown,
            Some(Ended::Cracked) => Shows::Open,
            None if self.missed && tick < self.from => Shows::Err,
            None => Shows::Sweep,
        };
        Panel {
            shows,
            bar: self.run.bar,
            zone: (self.at, self.width()),
            marker: self.marker(tick),
            digits: digits.to_vec(),
            faults: self.faults,
            allowed: self.run.faults,
            beat: self.beat.min(3),
            rating: self.run.rating,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Keypad {
    look: Look,
    building: BuildingId,
    level: i32,
    number: u16,
    label: String,
    /// The door's code, for the worn keys.
    code: u16,
    /// The code as the journal knows it.
    known: Option<u16>,
    typed: Vec<u8>,
    flash: Flash,
    /// Drawn by a scene: it never asks `credentials`, never pops itself and holds its
    /// verdict on the glass.
    scripted: bool,
    /// The Keyjack, while it is clipped on.
    clip: Option<Clip>,
}

/// Where key `i` of the grid sits inside the device rect: three a row, a row of
/// air between the rows.
pub(crate) fn key_at(r: Rect, i: usize) -> (u16, u16) {
    (r.x + 4 + (i as u16 % 3) * 6, r.y + 9 + (i as u16 / 3) * 2)
}

impl Keypad {
    pub fn open(st: &AppState, building: BuildingId, level: i32, number: u16) -> Keypad {
        let seed = st.city.as_ref().map(|c| c.seed).unwrap_or(0);
        let look = Look::pick(st, Device::Keypad(building));
        Keypad {
            look,
            building,
            level,
            number,
            label: format!("flat {number}"),
            code: citygen::rng::door_code(seed, building, level, number),
            known: st.journal.code_for(building, level, number).map(|c| c.code),
            typed: Vec::new(),
            flash: Flash::None,
            scripted: false,
            clip: None,
        }
    }

    /// The pad of that flat with nothing typed, for a scene to type on.
    pub fn scripted(st: &AppState, building: BuildingId, level: i32, number: u16) -> Keypad {
        Keypad {
            scripted: true,
            ..Keypad::open(st, building, level, number)
        }
    }

    /// A digit appears on the display (the first four only).
    pub fn show_digit(&mut self, d: u8) {
        if self.typed.len() < 4 {
            self.typed.push(d);
        }
    }

    /// Flashes OPEN or DENIED as the real pad does; nothing else is touched.
    pub fn show_verdict(&mut self, open: bool, tick: u64) {
        self.flash = if open {
            Flash::Open(tick)
        } else {
            Flash::Denied(tick)
        };
    }

    #[cfg(test)]
    pub(crate) fn typed(&self) -> &[u8] {
        &self.typed
    }

    /// Moves the clock on to the frame after one that shows the marker in the zone (or
    /// out of it), so a strike now hits (or misses), for a test or a shot to aim with.
    #[cfg(test)]
    pub(crate) fn aim(&self, st: &mut AppState, hit: bool) {
        let c = self.clip.as_ref().expect("clipped on");
        let start = st.tick.max(c.from);
        let shown = (start..start + 3_000)
            .find(|t| c.hits(c.marker(*t)) == hit)
            .expect("the marker gets there");
        st.tick = shown + 1;
    }

    /// Whether the run is over (blown or cracked), for a shot to stop on.
    #[cfg(test)]
    pub(crate) fn clip_ended(&self) -> Option<bool> {
        self.clip
            .as_ref()
            .and_then(|c| c.ended)
            .map(|e| e == Ended::Cracked)
    }

    /// In the middle of the screen, always inside the area.
    pub(crate) fn rect(&self, area: Rect) -> Rect {
        let w = DEVICE_W.min(area.width);
        let h = DEVICE_H.min(area.height);
        Rect::new(
            area.x + area.width.saturating_sub(w) / 2,
            area.y + area.height.saturating_sub(h) / 2,
            w,
            h,
        )
    }

    fn locked_until(&self, st: &AppState) -> Option<u64> {
        st.journal
            .keypad_locked_until(self.building, self.level, self.number, st.net.now(st))
    }

    fn verify(&mut self, st: &mut AppState) {
        let typed = if self.typed.is_empty() {
            self.known
        } else if self.typed.len() == 4 {
            Some(self.typed.iter().fold(0u16, |n, d| n * 10 + *d as u16))
        } else {
            None
        };
        let Some(code) = typed else {
            self.flash = Flash::Refused(st.tick);
            return;
        };
        match credentials::try_code(st, self.building, self.level, self.number, code) {
            Verdict::Open => {
                self.known = Some(self.code);
                self.flash = Flash::Open(st.tick);
            }
            Verdict::Denied { .. } => {
                self.typed.clear();
                self.flash = Flash::Denied(st.tick);
            }
            Verdict::Locked { .. } => {
                self.typed.clear();
                self.flash = Flash::Refused(st.tick);
            }
        }
    }

    /// `k`: clips the first Keyjack with a fuse in onto the pad and starts its first beat.
    /// Refused on a locked pad; with no fuse in any, the log says so.
    fn clip_on(&mut self, st: &mut AppState) {
        let (copy, run) = match keyjack::carried(st) {
            Carried::None => return,
            _ if self.locked_until(st).is_some() => {
                self.flash = Flash::Refused(st.tick);
                return;
            }
            Carried::NoFuse => {
                st.warn("No fuse in the Keyjack.");
                return;
            }
            Carried::Fused(c) => (c.id, keyjack::run_for(st, self.building, c)),
        };
        let now = st.net.now(st);
        let faults = st
            .journal
            .keypad_now(self.building, self.level, self.number, now)
            .faults;
        let mut clip = Clip {
            copy,
            run,
            seed: keyjack::door_seed(st, self.building, self.level, self.number),
            beat: 0,
            at: 0,
            from: st.tick,
            missed: false,
            faults,
            ended: None,
        };
        clip.place();
        self.typed.clear();
        self.flash = Flash::None;
        self.clip = Some(clip);
    }

    /// Space: a hit keys the beat's digit in and starts the next beat, the fourth opens
    /// the door; a miss is a fault and starts the beat over, the last burns the fuse and
    /// locks the pad. Nothing while ERR shows or once the run is over.
    ///
    /// The frame moves the tick on before the keys come in, so the space was pressed on
    /// the frame before: that is where it is judged, on the marker the player saw. A beat
    /// starts on this tick, so a second space in the same keys, pressed before its first
    /// frame showed, strikes nothing.
    fn strike(&mut self, st: &mut AppState) {
        let (b, level, number) = (self.building, self.level, self.number);
        let Some(c) = self.clip.as_mut() else {
            return;
        };
        let shown = st.tick.saturating_sub(1);
        if c.ended.is_some() || shown < c.from {
            return;
        }
        if c.hits(c.marker(shown)) {
            let digit = (self.code / 10u16.pow(3 - c.beat as u32) % 10) as u8;
            self.typed.push(digit);
            c.beat += 1;
            c.missed = false;
            if c.beat == 4 {
                c.ended = Some(Ended::Cracked);
                credentials::open_with(st, b, level, number, Source::Keyjack);
                self.known = Some(self.code);
                self.flash = Flash::Open(st.tick);
            } else {
                c.from = st.tick;
                c.place();
            }
            return;
        }
        c.faults = credentials::fault(st, b, level, number);
        if c.faults >= c.run.faults {
            credentials::lock_out(st, b, level, number);
            keyjack::blow_fuse(st, c.copy);
            c.ended = Some(Ended::Blown);
            self.typed.clear();
            self.flash = Flash::Denied(st.tick);
            st.warn(format!(
                "The Keyjack's fuse blew. The pad locked for {}m.",
                credentials::LOCKOUT_MINUTES
            ));
        } else {
            c.missed = true;
            c.from = st.tick + DENIED_TICKS;
            c.place();
        }
    }

    /// Esc on the Keyjack: it comes off, the pad stays open with nothing typed, and the
    /// faults stay on the door.
    fn pull(&mut self) {
        self.clip = None;
        self.typed.clear();
    }
}

fn pop() -> ScreenCmd {
    ScreenCmd::Pop {
        fx: TransitionKind::None,
    }
}

impl Screen for Keypad {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        if self.scripted {
            return ScreenCmd::Stay;
        }
        if let Flash::Open(t) = self.flash {
            if !keys.is_empty() || st.tick > t + OPEN_TICKS {
                return pop();
            }
            return ScreenCmd::Stay;
        }
        if self.clip.is_some() {
            for k in keys {
                match k.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        self.pull();
                        return ScreenCmd::Stay;
                    }
                    KeyCode::Char(' ') => self.strike(st),
                    _ => {}
                }
                if matches!(self.flash, Flash::Open(_)) {
                    break;
                }
            }
            return ScreenCmd::Stay;
        }
        let locked = self.locked_until(st).is_some();
        for k in keys {
            match k.code {
                KeyCode::Esc | KeyCode::Char('q') => return pop(),
                KeyCode::Char(c @ '0'..='9') if !locked && self.typed.len() < 4 => {
                    self.typed.push(c as u8 - b'0');
                    // Typing again puts the slots back in place of DENIED.
                    if matches!(self.flash, Flash::Denied(_)) {
                        self.flash = Flash::None;
                    }
                }
                KeyCode::Char('0'..='9') => self.flash = Flash::Refused(st.tick),
                KeyCode::Char('x' | 'X') | KeyCode::Backspace => self.typed.clear(),
                KeyCode::Char('v' | 'V') | KeyCode::Enter => self.verify(st),
                KeyCode::Char('k' | 'K') => self.clip_on(st),
                _ => {}
            }
            if matches!(self.flash, Flash::Open(_)) {
                break;
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let r = self.rect(f.area());
        if r.width < DEVICE_W || r.height < DEVICE_H {
            return;
        }
        let tick = st.tick;
        let now = st.net.now(st);
        let locked = self.locked_until(st);
        // The reading on the glass, its colour, and whether the glass glitches.
        let (led, reading, glitch) = match (self.flash, locked) {
            (Flash::Open(_), _) => (Led::Ok, Some(("OPEN".to_string(), self.look.text())), None),
            (Flash::Denied(t), _) if self.scripted || tick <= t + DENIED_TICKS => (
                Led::Fail,
                Some(("DENIED".to_string(), device::DENY)),
                // The glitch runs its frames; a scripted pad keeps the word after.
                Some(tick.saturating_sub(t) as f32 / DENIED_TICKS as f32).filter(|p| *p <= 1.0),
            ),
            (_, Some(until)) => (
                Led::Fail,
                Some((
                    format!("LOCKED {}m", until.saturating_sub(now).max(1)),
                    device::DENY,
                )),
                None,
            ),
            (Flash::Refused(t), _) if tick <= t + REFUSED_TICKS => (Led::Fail, None, None),
            _ => (Led::Idle, None, None),
        };
        let buf = f.buffer_mut();
        device::bezel(buf, r, &self.look.keypad_badge(), led, &self.look);
        let live = Style::default().fg(self.look.text());
        let quiet = Style::default().fg(self.look.text_dim());
        let first = match &reading {
            Some((text, colour)) => (
                text.clone(),
                Style::default().fg(*colour).add_modifier(Modifier::BOLD),
            ),
            None => (
                (0..4)
                    .map(|i| match self.typed.get(i) {
                        Some(d) => format!("[{d}]"),
                        None => "[_]".to_string(),
                    })
                    .collect(),
                live,
            ),
        };
        let second = match self
            .known
            .filter(|_| self.typed.is_empty() && reading.is_none())
        {
            Some(code) => format!("journal {code:04}"),
            None => self.label.clone(),
        };
        let lines = [first, (second, quiet)];
        let glass = Rect::new(r.x + 2, r.y + 3, r.width - 4, 4);
        device::glass(buf, glass, &lines, &self.look, tick);
        // The refusal scrambles the label row under the reading, not the reading.
        if let Some(p) = glitch {
            let under = Rect::new(glass.x + 1, glass.y + 2, glass.width - 2, 1);
            device::glitch(buf, under, tick, p);
        }
        let worn: Vec<char> = format!("{:04}", self.code).chars().collect();
        for (i, ch) in KEYS.iter().enumerate() {
            let (x, y) = key_at(r, i);
            let lit = match ch {
                'X' => KeyLit::Deny,
                'V' => KeyLit::Accept,
                c if worn.contains(c) => KeyLit::Worn,
                _ => KeyLit::Normal,
            };
            device::key(buf, x, y, *ch, lit, &self.look);
        }
        device::dead_cells(buf, r, glass, &self.look);
        match &self.clip {
            Some(c) => slab::draw(buf, r, self.look.body(), &c.panel(tick, &self.typed)),
            None if self.scripted || matches!(self.flash, Flash::Open(_)) => {}
            None => match keyjack::carried(st) {
                Carried::None => {}
                Carried::NoFuse => slab::clip_line(buf, r, false),
                Carried::Fused(_) => slab::clip_line(buf, r, true),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, Screen, ScreenCmd};
    use crate::screens::testutil::{buffer_text, key};
    use citygen::{BuildingId, BuildingKind, GenParams, Generator, SizePreset};
    use crossterm::event::KeyCode;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn state() -> (AppState, BuildingId, i32, u16, u16) {
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
        let b = {
            let city = st.city.as_ref().unwrap();
            let world = st.world.as_ref().unwrap();
            city.buildings
                .iter()
                .find(|b| {
                    matches!(b.kind, BuildingKind::Apartment | BuildingKind::Megablock)
                        && world.people().iter().any(|p| p.home == Some(b.id))
                })
                .unwrap()
                .id
        };
        crate::credentials::prepare(&mut st, b);
        let (level, number) = st.interiors.homes_of(b).unwrap().flats().next().unwrap();
        let code = citygen::rng::door_code(st.city.as_ref().unwrap().seed, b, level, number);
        (st, b, level, number, code)
    }

    fn draw(k: &Keypad, st: &AppState) -> String {
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| k.draw(st, f)).unwrap();
        buffer_text(t.backend().buffer())
    }

    fn digits(code: u16) -> Vec<crossterm::event::KeyEvent> {
        format!("{code:04}")
            .chars()
            .map(|c| key(KeyCode::Char(c)))
            .collect()
    }

    #[test]
    fn digits_fill_four_slots_and_x_clears() {
        let (mut st, b, level, number, _) = state();
        let mut k = Keypad::open(&st, b, level, number);
        k.update(
            &mut st,
            &[
                key(KeyCode::Char('1')),
                key(KeyCode::Char('2')),
                key(KeyCode::Char('3')),
                key(KeyCode::Char('4')),
                key(KeyCode::Char('5')),
            ],
        );
        assert_eq!(k.typed(), &[1, 2, 3, 4]);
        let text = draw(&k, &st);
        assert!(text.contains("[1][2][3][4]"), "{text}");
        assert!(
            text.contains(&k.look.keypad_badge()) && text.contains(&format!("flat {number}")),
            "{text}"
        );
        assert!(!text.contains('`'));
        k.update(&mut st, &[key(KeyCode::Char('x'))]);
        assert!(k.typed().is_empty());
        k.update(&mut st, &[key(KeyCode::Char('9')), key(KeyCode::Backspace)]);
        assert!(k.typed().is_empty());
        assert!(draw(&k, &st).contains("[_][_][_][_]"));
    }

    #[test]
    fn the_right_code_opens_and_the_next_key_pops() {
        let (mut st, b, level, number, code) = state();
        let mut k = Keypad::open(&st, b, level, number);
        k.update(&mut st, &digits(code));
        assert!(matches!(
            k.update(&mut st, &[key(KeyCode::Char('v'))]),
            ScreenCmd::Stay
        ));
        assert!(draw(&k, &st).contains("OPEN"));
        assert!(st.journal.code_for(b, level, number).is_some());
        assert!(matches!(
            k.update(&mut st, &[key(KeyCode::Char('1'))]),
            ScreenCmd::Pop { .. }
        ));
    }

    #[test]
    fn the_beat_pops_the_open_pad_by_itself() {
        let (mut st, b, level, number, code) = state();
        let mut k = Keypad::open(&st, b, level, number);
        k.update(&mut st, &digits(code));
        k.update(&mut st, &[key(KeyCode::Enter)]);
        st.tick += OPEN_TICKS + 1;
        assert!(matches!(k.update(&mut st, &[]), ScreenCmd::Pop { .. }));
    }

    #[test]
    fn wrong_codes_deny_then_lock_and_locked_refuses_digits() {
        let (mut st, b, level, number, code) = state();
        let wrong = (code + 1) % 10_000;
        let mut k = Keypad::open(&st, b, level, number);
        for _ in 0..2 {
            k.update(&mut st, &digits(wrong));
            k.update(&mut st, &[key(KeyCode::Char('v'))]);
            assert!(draw(&k, &st).contains("DENIED"));
            assert!(k.typed().is_empty());
        }
        k.update(&mut st, &digits(wrong));
        k.update(&mut st, &[key(KeyCode::Char('v'))]);
        assert!(draw(&k, &st).contains("LOCKED 30m"), "{}", draw(&k, &st));
        k.update(&mut st, &[key(KeyCode::Char('1'))]);
        assert!(k.typed().is_empty(), "locked refuses digits");
        assert!(matches!(
            k.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
        // A fresh pad on the same door still shows the countdown.
        let k2 = Keypad::open(&st, b, level, number);
        assert!(draw(&k2, &st).contains("LOCKED"));
    }

    #[test]
    fn v_on_empty_slots_keys_the_journals_code() {
        let (mut st, b, level, number, code) = state();
        crate::credentials::copy_codes(&mut st, b, 1);
        let mut k = Keypad::open(&st, b, level, number);
        assert!(draw(&k, &st).contains(&format!("journal {code:04}")));
        k.update(&mut st, &[key(KeyCode::Char('v'))]);
        assert!(draw(&k, &st).contains("OPEN"));
    }

    #[test]
    fn worn_keys_are_the_codes_digits() {
        let (st, b, level, number, code) = state();
        let k = Keypad::open(&st, b, level, number);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| k.draw(&st, f)).unwrap();
        let buf = t.backend().buffer();
        let worn: Vec<char> = format!("{code:04}").chars().collect();
        let r = k.rect(Rect::new(0, 0, 120, 40));
        let (_, worn_fg) = k.look.key_colours(KeyLit::Worn);
        for (i, ch) in KEYS.iter().enumerate() {
            let (x, y) = key_at(r, i);
            let cell = &buf[(x + 2, y)];
            assert_eq!(cell.symbol(), ch.to_string());
            let expect_worn = ch.is_ascii_digit() && worn.contains(ch);
            assert_eq!(cell.fg == worn_fg, expect_worn, "key {ch}");
        }
    }

    #[test]
    fn the_pad_is_centred_on_the_screen() {
        let (st, b, level, number, _) = state();
        let k = Keypad::open(&st, b, level, number);
        let r = k.rect(Rect::new(0, 0, 120, 40));
        assert_eq!((r.x, r.y, r.width, r.height), (47, 10, DEVICE_W, DEVICE_H));
        let r = k.rect(Rect::new(0, 0, 101, 31));
        assert_eq!((r.x, r.y), (38, 6));
    }

    #[test]
    fn typing_again_after_denied_shows_the_digits() {
        let (mut st, b, level, number, code) = state();
        let wrong = (code + 1) % 10_000;
        let mut k = Keypad::open(&st, b, level, number);
        k.update(&mut st, &digits(wrong));
        k.update(&mut st, &[key(KeyCode::Char('v'))]);
        assert!(draw(&k, &st).contains("DENIED"));
        k.update(&mut st, &[key(KeyCode::Char('1'))]);
        let text = draw(&k, &st);
        assert!(
            text.contains("[1][_][_][_]") && !text.contains("DENIED"),
            "{text}"
        );
    }

    #[test]
    fn scripted_digits_show() {
        let (st, b, level, number, _) = state();
        let mut k = Keypad::scripted(&st, b, level, number);
        assert!(draw(&k, &st).contains("[_][_][_][_]"));
        for d in [4, 0, 7, 2, 9] {
            k.show_digit(d);
        }
        assert_eq!(k.typed(), &[4, 0, 7, 2]);
        assert!(draw(&k, &st).contains("[4][0][7][2]"));
    }

    #[test]
    fn scripted_verdict_touches_nothing() {
        let (mut st, b, level, number, _) = state();
        let mut k = Keypad::scripted(&st, b, level, number);
        let codes = st.journal.codes().len();
        let state = st.journal.keypad(b, level, number);
        let log = st.log.len();
        for d in [0, 0, 0, 0] {
            k.show_digit(d);
        }
        k.show_verdict(false, st.tick);
        assert_eq!(st.journal.codes().len(), codes);
        assert_eq!(st.journal.keypad(b, level, number), state);
        assert_eq!(st.log.len(), log);
        // It never pops itself, whatever the frame ticks do.
        st.tick += OPEN_TICKS * 10;
        assert!(matches!(k.update(&mut st, &[]), ScreenCmd::Stay));
    }

    /// The Keyjacks in the pack, in order.
    fn keyjacks(st: &AppState) -> Vec<&game::ItemCopy> {
        st.character
            .inventory
            .iter()
            .filter(|s| s.item.0 == crate::keyjack::ITEM)
            .filter_map(|s| s.copy.as_ref())
            .collect()
    }

    /// The `n`th Keyjack in the pack.
    fn keyjack_mut(st: &mut AppState, n: usize) -> &mut game::ItemCopy {
        st.character
            .inventory
            .iter_mut()
            .filter(|s| s.item.0 == crate::keyjack::ITEM)
            .filter_map(|s| s.copy.as_mut())
            .nth(n)
            .expect("a Keyjack in the pack")
    }

    fn fuse(item: &str) -> Option<game::Stack> {
        Some(game::Stack::new(game::ItemId::new(item), 1))
    }

    /// The code's four digits, left to right.
    fn code_digits(code: u16) -> Vec<u8> {
        format!("{code:04}").bytes().map(|b| b - b'0').collect()
    }

    /// Waits for a frame that shows the marker in the zone (or out of it), then strikes
    /// on the next, as `Runner::frame` hands over a key pressed while it showed.
    fn strike(k: &mut Keypad, st: &mut AppState, hit: bool) {
        k.aim(st, hit);
        k.update(st, &[key(KeyCode::Char(' '))]);
    }

    /// The row of the drawn screen holding `needle`, from the needle on.
    fn row_from(text: &str, needle: &str) -> String {
        text.lines()
            .find_map(|l| l.find(needle).map(|i| l[i..].to_string()))
            .unwrap_or_default()
    }

    #[test]
    fn the_clip_line_shows_with_a_keyjack() {
        let (mut st, b, level, number, _) = state();
        let mut k = Keypad::open(&st, b, level, number);
        assert!(!keyjacks(&st).is_empty(), "the kit carries one");
        let text = draw(&k, &st);
        assert!(text.contains("k  clip the keyjack"), "{text}");
        // No fuse in it: the line greys and `k` says so.
        keyjack_mut(&mut st, 0).fitted[0] = None;
        let text = draw(&k, &st);
        assert!(
            text.contains("k  keyjack · no fuse") && !text.contains("clip the keyjack"),
            "{text}"
        );
        let log = st.log.len();
        k.update(&mut st, &[key(KeyCode::Char('k'))]);
        assert!(k.clip.is_none());
        assert_eq!(st.log.len(), log + 1);
        assert!(st.log.iter().any(|e| e.text == "No fuse in the Keyjack."));
        // No Keyjack: no line, and `k` does nothing.
        st.character
            .inventory
            .retain(|s| s.item.0 != crate::keyjack::ITEM);
        let text = draw(&k, &st);
        assert!(!text.contains("keyjack"), "{text}");
        let log = st.log.len();
        k.update(&mut st, &[key(KeyCode::Char('k'))]);
        assert!(k.clip.is_none());
        assert_eq!(st.log.len(), log);
    }

    #[test]
    fn a_capital_k_clips_it_on_too() {
        let (mut st, b, level, number, _) = state();
        let mut k = Keypad::open(&st, b, level, number);
        k.update(&mut st, &[key(KeyCode::Char('K'))]);
        assert!(k.clip.is_some(), "as V and X take their capitals");
    }

    #[test]
    fn k_is_refused_while_locked() {
        let (mut st, b, level, number, _) = state();
        crate::credentials::lock_out(&mut st, b, level, number);
        let mut k = Keypad::open(&st, b, level, number);
        k.update(&mut st, &[key(KeyCode::Char('k'))]);
        assert!(k.clip.is_none());
        assert_eq!(k.flash, Flash::Refused(st.tick));
        assert!(draw(&k, &st).contains("LOCKED 30m"));
    }

    #[test]
    fn hits_key_the_code_and_open_the_door() {
        let (mut st, b, level, number, code) = state();
        let digits = code_digits(code);
        let mut k = Keypad::open(&st, b, level, number);
        k.update(&mut st, &[key(KeyCode::Char('k'))]);
        let run = k.clip.as_ref().expect("clipped on").run.clone();
        assert!(draw(&k, &st).contains("KEYJACK"));
        for beat in 0..4 {
            let c = k.clip.as_ref().unwrap();
            assert_eq!(c.beat, beat);
            assert_eq!(c.width(), run.zones[beat]);
            assert!(c.at + c.width() <= run.bar);
            // Digits do nothing while it's on.
            k.update(&mut st, &[key(KeyCode::Char('1'))]);
            assert_eq!(k.typed(), &digits[..beat]);
            strike(&mut k, &mut st, true);
            assert_eq!(k.typed(), &digits[..beat + 1], "beat {beat}");
        }
        let text = draw(&k, &st);
        assert!(text.contains("OPEN"), "{text}");
        let learned = st.journal.code_for(b, level, number).expect("learned");
        assert_eq!(
            (learned.code, learned.source),
            (code, crate::credentials::Source::Keyjack)
        );
        assert_eq!(st.journal.keypad(b, level, number).faults, 0);
        assert!(matches!(k.update(&mut st, &[]), ScreenCmd::Stay));
        assert!(matches!(
            k.update(&mut st, &[key(KeyCode::Char(' '))]),
            ScreenCmd::Pop { .. }
        ));
    }

    /// `Runner::frame` moves the tick on before the screen reads the keys, so a space
    /// pressed while frame `t` showed arrives at `t + 1`: the strike is judged where the
    /// player saw the marker.
    #[test]
    fn a_strike_is_judged_on_the_frame_shown() {
        let (mut st, b, level, number, code) = state();
        let mut k = Keypad::open(&st, b, level, number);
        k.update(&mut st, &[key(KeyCode::Char('k'))]);
        let c = k.clip.as_ref().unwrap();
        // The last frame that shows the marker in the zone before it leaves.
        let shown = (c.from..c.from + 3_000)
            .find(|t| c.hits(c.marker(*t)) && !c.hits(c.marker(t + 1)))
            .expect("the marker leaves the zone");
        st.tick = shown + 1;
        k.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(st.journal.keypad(b, level, number).faults, 0, "a hit");
        assert_eq!(k.typed(), &code_digits(code)[..1]);
    }

    /// Two spaces in one frame's keys: the first hits and starts the next beat, the
    /// second came before the player saw that beat and strikes nothing.
    #[test]
    fn a_second_space_in_the_frame_of_a_hit_strikes_nothing() {
        let (mut st, b, level, number, code) = state();
        let mut k = Keypad::open(&st, b, level, number);
        k.update(&mut st, &[key(KeyCode::Char('k'))]);
        let c = k.clip.as_ref().unwrap();
        // Well inside the zone, so the first strike hits on either frame.
        let shown = (c.from..c.from + 3_000)
            .find(|t| c.hits(c.marker(*t)) && c.hits(c.marker(t + 1)))
            .expect("the marker crosses the zone");
        st.tick = shown + 1;
        let space = key(KeyCode::Char(' '));
        k.update(&mut st, &[space, space]);
        assert_eq!(st.journal.keypad(b, level, number).faults, 0, "no fault");
        assert_eq!(k.typed(), &code_digits(code)[..1], "one digit");
        assert_eq!(k.clip.as_ref().unwrap().beat, 1);
    }

    #[test]
    fn a_miss_is_a_fault_and_repeats_the_beat() {
        let (mut st, b, level, number, code) = state();
        let digits = code_digits(code);
        let mut k = Keypad::open(&st, b, level, number);
        k.update(&mut st, &[key(KeyCode::Char('k'))]);
        strike(&mut k, &mut st, true);
        let (at, width) = {
            let c = k.clip.as_ref().unwrap();
            (c.at, c.width())
        };
        strike(&mut k, &mut st, false);
        assert_eq!(st.journal.keypad(b, level, number).faults, 1);
        let c = k.clip.as_ref().unwrap();
        assert_eq!((c.beat, c.width()), (1, width));
        assert_ne!(c.at, at, "a new position");
        assert_eq!(k.typed(), &digits[..1]);
        let text = draw(&k, &st);
        assert!(text.contains("ERR"), "{text}");
        // While ERR shows, a strike does nothing.
        k.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(st.journal.keypad(b, level, number).faults, 1);
        st.tick += DENIED_TICKS;
        assert!(!draw(&k, &st).contains("ERR"));
        assert_eq!(k.clip.as_ref().unwrap().marker(st.tick), 0, "set off again");
    }

    #[test]
    fn the_last_fault_locks_and_burns_the_fitted_fuse() {
        let (mut st, b, level, number, _) = state();
        let spares = st.character.count(&game::ItemId::new("fuse_stock"));
        assert_eq!(spares, 2);
        let mut k = Keypad::open(&st, b, level, number);
        k.update(&mut st, &[key(KeyCode::Char('k'))]);
        let allowed = k.clip.as_ref().unwrap().run.faults;
        assert_eq!(allowed, 3);
        for _ in 0..allowed {
            strike(&mut k, &mut st, false);
        }
        let now = st.net.now(&st);
        assert_eq!(
            st.journal.keypad_locked_until(b, level, number, now),
            Some(now + crate::credentials::LOCKOUT_MINUTES)
        );
        assert_eq!(keyjacks(&st)[0].fitted, vec![None], "the fuse is gone");
        assert_eq!(st.character.count(&game::ItemId::new("fuse_stock")), spares);
        assert!(
            st.log
                .iter()
                .any(|e| e.text == "The Keyjack's fuse blew. The pad locked for 30m.")
        );
        let text = draw(&k, &st);
        assert!(
            text.contains("FUSE  BLOWN") && text.contains("DENIED"),
            "{text}"
        );
        // Strikes do nothing now; esc pulls it off and the pad stays.
        k.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(matches!(
            k.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Stay
        ));
        assert!(k.clip.is_none());
        st.tick += DENIED_TICKS + 1;
        let text = draw(&k, &st);
        assert!(
            text.contains("LOCKED 30m") && text.contains("keyjack · no fuse"),
            "{text}"
        );
    }

    #[test]
    fn faults_survive_pulling_it_off() {
        let (mut st, b, level, number, _) = state();
        let mut k = Keypad::open(&st, b, level, number);
        k.update(&mut st, &[key(KeyCode::Char('k'))]);
        strike(&mut k, &mut st, true);
        strike(&mut k, &mut st, false);
        assert!(matches!(
            k.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Stay
        ));
        assert!(k.clip.is_none() && k.typed().is_empty());
        assert_eq!(st.journal.keypad(b, level, number).faults, 1);
        // A fresh pad on the same door, clipped again: the beats start over, the fault stays.
        let mut k = Keypad::open(&st, b, level, number);
        k.update(&mut st, &[key(KeyCode::Char('k'))]);
        assert_eq!(k.clip.as_ref().unwrap().beat, 0);
        strike(&mut k, &mut st, false);
        assert_eq!(st.journal.keypad(b, level, number).faults, 2);
        // Cracking it clears them.
        for _ in 0..4 {
            strike(&mut k, &mut st, true);
        }
        assert_eq!(st.journal.keypad(b, level, number).faults, 0);
    }

    #[test]
    fn the_first_fused_keyjack_runs() {
        let (mut st, b, level, number, _) = state();
        let empty = st
            .character
            .new_copy(&st.content, &game::ItemId::new(crate::keyjack::ITEM));
        assert_eq!(empty.fitted, vec![None]);
        st.character.inventory.insert(
            0,
            game::Stack {
                item: game::ItemId::new(crate::keyjack::ITEM),
                qty: 1,
                copy: Some(empty),
            },
        );
        let fused = keyjacks(&st)[1].id;
        let mut k = Keypad::open(&st, b, level, number);
        assert!(draw(&k, &st).contains("k  clip the keyjack"));
        k.update(&mut st, &[key(KeyCode::Char('k'))]);
        assert_eq!(k.clip.as_ref().expect("clipped on").copy, fused);
    }

    #[test]
    fn the_panel_shows_a_light_per_allowed_fault() {
        let (mut st, b, level, number, _) = state();
        let lights = |k: &Keypad, st: &AppState| {
            row_from(&draw(k, st), "FAULT")
                .chars()
                .filter(|c| *c == '●')
                .count()
        };
        let mut k = Keypad::open(&st, b, level, number);
        k.update(&mut st, &[key(KeyCode::Char('k'))]);
        assert_eq!(lights(&k, &st), 3);
        k.update(&mut st, &[key(KeyCode::Esc)]);
        keyjack_mut(&mut st, 0).fitted[0] = fuse("fuse_ceramic");
        st.character
            .talents
            .insert(game::TalentId::new("skeleton_key"), 1);
        k.update(&mut st, &[key(KeyCode::Char('k'))]);
        assert_eq!(k.clip.as_ref().unwrap().run.faults, 5);
        assert_eq!(lights(&k, &st), 5);
        // A fault lights the first.
        strike(&mut k, &mut st, false);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| k.draw(&st, f)).unwrap();
        let buf = t.backend().buffer();
        let row = buffer_text(buf)
            .lines()
            .position(|l| l.contains("FAULT"))
            .unwrap() as u16;
        let lit = (0..120u16)
            .filter(|x| {
                let c = &buf[(*x, row)];
                c.symbol() == "●" && c.fg == crate::ui::slab::FAULT_LIT
            })
            .count();
        assert_eq!(lit, 1);
        // The lock's rating sits beside BEAT.
        let rating = k.clip.as_ref().unwrap().run.rating;
        let text = draw(&k, &st);
        assert!(
            row_from(&text, "BEAT").starts_with(&format!("BEAT 1/4  R{rating}")),
            "{text}"
        );
    }

    #[test]
    fn the_marker_sweeps_end_to_end_and_back() {
        let (mut st, b, level, number, _) = state();
        let mut k = Keypad::open(&st, b, level, number);
        k.update(&mut st, &[key(KeyCode::Char('k'))]);
        let c = k.clip.as_ref().unwrap();
        let (from, bar, speed) = (c.from, c.run.bar, c.run.speed[0]);
        // Ticks for the marker to travel `cells`, a tick a frame.
        let ticks = |cells: u16| (cells as f32 * FPS as f32 / speed).ceil() as u64;
        assert_eq!(c.marker(from), 0);
        assert_eq!(c.marker(from + ticks(bar - 1)), bar - 1, "the right end");
        assert_eq!(c.marker(from + ticks(bar)), bar - 1, "and back");
        assert_eq!(c.marker(from + ticks(2 * bar - 1)), 0, "the left end");
        assert_eq!(c.marker(from + ticks(2 * bar)), 0);
        let seen: std::collections::BTreeSet<u16> =
            (from..from + ticks(2 * bar)).map(|t| c.marker(t)).collect();
        assert_eq!(seen, (0..bar).collect(), "every cell");
    }

    #[test]
    fn the_panel_and_the_pad_fit_the_smallest_screen() {
        let (mut st, b, level, number, _) = state();
        let mut k = Keypad::open(&st, b, level, number);
        k.update(&mut st, &[key(KeyCode::Char('k'))]);
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| k.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(
            text.contains("KEYJACK") && text.contains("esc") && text.contains("pull"),
            "{text}"
        );
        assert!(text.contains(&k.look.keypad_badge()), "{text}");
    }

    #[test]
    fn scripted_denied_holds() {
        let (mut st, b, level, number, _) = state();
        let mut k = Keypad::scripted(&st, b, level, number);
        let t = st.tick;
        k.show_verdict(false, t);
        assert!(draw(&k, &st).contains("DENIED"));
        st.tick = t + 100;
        let text = draw(&k, &st);
        assert!(text.contains("DENIED") && !text.contains("[_]"), "{text}");
        k.show_verdict(true, t);
        assert!(draw(&k, &st).contains("OPEN"));
    }
}
