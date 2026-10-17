//! The keypad on a flat's door: a beige device over the map. Key the code, V to
//! verify; a hit is learned, three misses lock it.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::credentials::{self, Verdict};
use crate::ui::device::{self, DEVICE_H, DEVICE_W, Led};
use citygen::BuildingId;
use crossterm::event::{KeyCode, KeyEvent};
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

pub struct Keypad {
    building: BuildingId,
    level: i32,
    number: u16,
    label: String,
    /// The door's code, for the worn keys.
    code: u16,
    /// The code as the journal knows it.
    known: Option<u16>,
    anchor: Option<(u16, u16)>,
    typed: Vec<u8>,
    flash: Flash,
}

/// Where key `i` of the grid sits inside the device rect.
pub(crate) fn key_at(r: Rect, i: usize) -> (u16, u16) {
    (r.x + 4 + (i as u16 % 3) * 6, r.y + 6 + (i as u16 / 3) * 3)
}

impl Keypad {
    pub fn open(
        st: &AppState,
        building: BuildingId,
        level: i32,
        number: u16,
        anchor: Option<(u16, u16)>,
    ) -> Keypad {
        let seed = st.city.as_ref().map(|c| c.seed).unwrap_or(0);
        Keypad {
            building,
            level,
            number,
            label: format!("flat {number}"),
            code: citygen::rng::door_code(seed, building, level, number),
            known: st.journal.code_for(building, level, number).map(|c| c.code),
            anchor,
            typed: Vec::new(),
            flash: Flash::None,
        }
    }

    #[cfg(test)]
    pub(crate) fn typed(&self) -> &[u8] {
        &self.typed
    }

    /// Beside the door when anchored, else in the middle; always inside the area.
    pub(crate) fn rect(&self, area: Rect) -> Rect {
        let w = DEVICE_W.min(area.width);
        let h = DEVICE_H.min(area.height);
        let (x, y) = match self.anchor {
            Some((ax, ay)) => (
                ax.clamp(area.x, area.right().saturating_sub(w).max(area.x)),
                ay.clamp(area.y, area.bottom().saturating_sub(h).max(area.y)),
            ),
            None => (
                area.x + area.width.saturating_sub(w) / 2,
                area.y + area.height.saturating_sub(h) / 2,
            ),
        };
        Rect::new(x, y, w, h)
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
}

fn pop() -> ScreenCmd {
    ScreenCmd::Pop {
        fx: TransitionKind::None,
    }
}

impl Screen for Keypad {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        if let Flash::Open(t) = self.flash {
            if !keys.is_empty() || st.tick > t + OPEN_TICKS {
                return pop();
            }
            return ScreenCmd::Stay;
        }
        let locked = self.locked_until(st).is_some();
        for k in keys {
            match k.code {
                KeyCode::Esc | KeyCode::Char('q') => return pop(),
                KeyCode::Char(c @ '0'..='9') if !locked && self.typed.len() < 4 => {
                    self.typed.push(c as u8 - b'0');
                }
                KeyCode::Char('0'..='9') => self.flash = Flash::Refused(st.tick),
                KeyCode::Char('x' | 'X') | KeyCode::Backspace => self.typed.clear(),
                KeyCode::Char('v' | 'V') | KeyCode::Enter => self.verify(st),
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
        let (led, reading) = match (self.flash, locked) {
            (Flash::Open(_), _) => (Led::Green, Some(("OPEN".to_string(), device::PHOSPHOR))),
            (Flash::Denied(t), _) if tick <= t + DENIED_TICKS => {
                (Led::Red, Some(("DENIED".to_string(), device::LED_RED)))
            }
            (_, Some(until)) => (
                Led::Red,
                Some((
                    format!("LOCKED {}m", until.saturating_sub(now).max(1)),
                    device::LED_RED,
                )),
            ),
            (Flash::Refused(t), _) if tick <= t + REFUSED_TICKS => (Led::Red, None),
            _ => (Led::Amber, None),
        };
        let buf = f.buffer_mut();
        device::bezel(buf, r, "SECURLOK", led);
        let green = Style::default().fg(device::PHOSPHOR);
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
                green,
            ),
        };
        let second = match self
            .known
            .filter(|_| self.typed.is_empty() && reading.is_none())
        {
            Some(code) => format!("journal {code:04}"),
            None => self.label.clone(),
        };
        let lines = [first, (second, green.add_modifier(Modifier::DIM))];
        device::lcd(buf, Rect::new(r.x + 2, r.y + 2, r.width - 4, 4), &lines);
        let worn: Vec<char> = format!("{:04}", self.code).chars().collect();
        for (i, ch) in KEYS.iter().enumerate() {
            let (x, y) = key_at(r, i);
            let ink = match ch {
                'X' => Style::default().fg(device::LED_RED),
                'V' => Style::default().fg(device::LED_GREEN),
                c if worn.contains(c) => Style::default().fg(device::INK_WORN),
                _ => Style::default().fg(device::INK),
            };
            device::keycap(buf, x, y, *ch, ink);
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
        let mut k = Keypad::open(&st, b, level, number, None);
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
            text.contains("SECURLOK") && text.contains(&format!("flat {number}")),
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
        let mut k = Keypad::open(&st, b, level, number, None);
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
        let mut k = Keypad::open(&st, b, level, number, None);
        k.update(&mut st, &digits(code));
        k.update(&mut st, &[key(KeyCode::Enter)]);
        st.tick += OPEN_TICKS + 1;
        assert!(matches!(k.update(&mut st, &[]), ScreenCmd::Pop { .. }));
    }

    #[test]
    fn wrong_codes_deny_then_lock_and_locked_refuses_digits() {
        let (mut st, b, level, number, code) = state();
        let wrong = (code + 1) % 10_000;
        let mut k = Keypad::open(&st, b, level, number, None);
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
        let k2 = Keypad::open(&st, b, level, number, None);
        assert!(draw(&k2, &st).contains("LOCKED"));
    }

    #[test]
    fn v_on_empty_slots_keys_the_journals_code() {
        let (mut st, b, level, number, code) = state();
        crate::credentials::copy_codes(&mut st, b, 1);
        let mut k = Keypad::open(&st, b, level, number, None);
        assert!(draw(&k, &st).contains(&format!("journal {code:04}")));
        k.update(&mut st, &[key(KeyCode::Char('v'))]);
        assert!(draw(&k, &st).contains("OPEN"));
    }

    #[test]
    fn worn_keys_are_the_codes_digits() {
        let (st, b, level, number, code) = state();
        let k = Keypad::open(&st, b, level, number, None);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| k.draw(&st, f)).unwrap();
        let buf = t.backend().buffer();
        let worn: Vec<char> = format!("{code:04}").chars().collect();
        let r = k.rect(Rect::new(0, 0, 120, 40));
        for (i, ch) in KEYS.iter().enumerate() {
            let (x, y) = key_at(r, i);
            let cell = &buf[(x + 2, y + 1)];
            assert_eq!(cell.symbol(), ch.to_string());
            let expect_worn = ch.is_ascii_digit() && worn.contains(ch);
            assert_eq!(
                cell.fg == crate::ui::device::INK_WORN,
                expect_worn,
                "key {ch}"
            );
        }
    }
}
