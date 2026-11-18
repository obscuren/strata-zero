//! The Keyjack clipped on a container's lock: the same timing game as a keypad's, with the
//! container's face where the pad would be. Space strikes while the marker sits in the lit
//! zone; four hits open the container, the last fault blows the fuse and shuts the Keyjack
//! out of this lock for a while (the card and a forced lock stay open to you).

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::containers::state::Opened;
use crate::containers::{self, Container, ContainerAt, Status, ways};
use crate::credentials::LOCKOUT_MINUTES;
use crate::keyjack::{self, Carried};
use crate::render::tileset::{container_body, container_detail};
use crate::screens::clip::{Clip, Ended, Strike};
use crate::screens::container::ContainerPanel;
use crate::screens::keypad::OPEN_TICKS;
use crate::ui::slab;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

/// The container's face, in cells.
const FACE_W: u16 = 14;
const FACE_H: u16 = 7;
/// The gap between the face and the slab (`slab::rect` puts it three cells off).
const GAP: u16 = 3;

/// The Keyjack on one container's lock.
#[derive(Clone, Debug)]
pub struct LockJack {
    at: ContainerAt,
    /// The lock's grade, for the face.
    grade: u8,
    clip: Clip,
    /// The lock's four digits.
    code: u16,
    /// The tick the container opened.
    opened: u64,
}

fn digit_of(code: u16, beat: usize) -> u8 {
    (code / 10u16.pow(3 - beat as u32) % 10) as u8
}

impl LockJack {
    /// The first fused Keyjack clipped on at once; `None` with no fused Keyjack, locked
    /// out of this lock, in a fight, or with nothing locked there.
    pub fn open(st: &AppState, at: ContainerAt) -> Option<LockJack> {
        if st.net.engaged.is_some() {
            return None;
        }
        let c = containers::find(st, at)?;
        let Status::Locked(lock) = containers::status(st, &c) else {
            return None;
        };
        let (faults, until) = ways::keyjack_now(st, at);
        if until.is_some() {
            return None;
        }
        let Carried::Fused(copy) = keyjack::carried(st) else {
            return None;
        };
        let run = keyjack::run_at(st, lock.grade, copy);
        let clip = Clip::new(copy.id, run, ways::keyjack_seed(st, &c), faults, st.tick);
        Some(LockJack {
            at,
            grade: lock.grade,
            clip,
            code: ways::lock_code(st, &c),
            opened: 0,
        })
    }

    /// The digits cracked so far.
    fn digits(&self) -> Vec<u8> {
        (0..self.clip.beat.min(4))
            .map(|b| digit_of(self.code, b))
            .collect()
    }

    /// Space: a hit moves to the next beat, the fourth opens it; a miss is a fault, the
    /// last blows the fuse and locks the Keyjack out.
    fn strike(&mut self, st: &mut AppState, c: &Container) {
        match self.clip.strike(st.tick) {
            Strike::Nothing | Strike::Hit { .. } => {}
            Strike::Cracked => {
                containers::open(st, c, Opened::Keyjack);
                self.opened = st.tick;
            }
            Strike::Miss => {
                let faults = ways::fault(st, self.at);
                if self.clip.faulted(faults, st.tick) {
                    ways::lock_out(st, self.at);
                    keyjack::blow_fuse(st, self.clip.copy);
                    st.warn(format!(
                        "The Keyjack's fuse blew. The lock is out of reach for {LOCKOUT_MINUTES}m."
                    ));
                }
            }
        }
    }

    /// The container's face and the slab, centred together; the face's rect.
    fn face(&self, area: Rect) -> Rect {
        let slab_w = (self.clip.run.bar + 8).max(26);
        // The slab stands two cells below the face's top and is 13 tall.
        let (w, h) = (FACE_W + GAP + slab_w, 15u16.max(FACE_H));
        Rect::new(
            area.x + area.width.saturating_sub(w) / 2,
            area.y + area.height.saturating_sub(h) / 2,
            FACE_W.min(area.width),
            FACE_H.min(area.height),
        )
    }
}

fn pop() -> ScreenCmd {
    ScreenCmd::Pop {
        fx: TransitionKind::None,
    }
}

impl Screen for LockJack {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        let Some(c) = containers::find(st, self.at) else {
            return pop();
        };
        match self.clip.ended {
            Some(Ended::Cracked) => {
                if !keys.is_empty() || st.tick > self.opened + OPEN_TICKS {
                    return ScreenCmd::Replace {
                        to: Box::new(ContainerPanel::open(st, self.at)),
                        fx: TransitionKind::None,
                    };
                }
                return ScreenCmd::Stay;
            }
            Some(Ended::Blown) => {
                return if keys.is_empty() {
                    ScreenCmd::Stay
                } else {
                    pop()
                };
            }
            None => {}
        }
        for k in keys {
            match k.code {
                KeyCode::Esc | KeyCode::Char('q') => return pop(),
                KeyCode::Char(' ') => self.strike(st, &c),
                _ => {}
            }
            if self.clip.ended.is_some() {
                break;
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let Some(c) = containers::find(st, self.at) else {
            return;
        };
        self.draw_with(st, f, &c);
    }
}

impl LockJack {
    fn draw_with(&self, st: &AppState, f: &mut Frame, c: &Container) {
        let r = self.face(f.area());
        if r.width < FACE_W || r.height < FACE_H {
            return;
        }
        let body = container_body(c.kind).to_color();
        let (glyph, detail) = container_detail(c.kind);
        let buf = f.buffer_mut();
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                let cell = &mut buf[(x, y)];
                cell.reset();
                cell.set_char(' ');
                cell.set_bg(body);
            }
        }
        let ink = ink_on(body);
        let centred = |buf: &mut ratatui::buffer::Buffer, y: u16, s: &str, fg: Color| {
            let w = s.chars().count() as u16;
            let x = r.x + r.width.saturating_sub(w) / 2;
            buf.set_string(x, y, s, Style::reset().fg(fg).bg(body));
        };
        centred(buf, r.y + 2, &glyph.to_string(), detail.to_color());
        centred(buf, r.y + 3, &c.kind.word().to_uppercase(), ink);
        centred(buf, r.y + 4, &format!("grade {}", self.grade), ink);
        slab::draw(buf, r, body, &self.clip.panel(st.tick, &self.digits()));
    }
}

/// Text that reads on `bg`: dark on a light body, light on a dark one.
fn ink_on(bg: Color) -> Color {
    let Color::Rgb(r, g, b) = bg else {
        return Color::White;
    };
    if (r as u32 * 3 + g as u32 * 6 + b as u32) / 10 > 140 {
        Color::Rgb(0x20, 0x26, 0x2e)
    } else {
        Color::Rgb(0xcd, 0xd4, 0xde)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::containers::testutil::*;
    use crate::screens::container::ContainerPanel;
    use crate::screens::testutil::{buffer_text, key};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn space() -> KeyEvent {
        key(KeyCode::Char(' '))
    }

    /// Strikes on a frame that shows the marker in the zone (or out of it).
    fn strike(j: &mut LockJack, st: &mut AppState, hit: bool) -> ScreenCmd {
        st.tick = j.clip.aim_tick(st.tick, hit);
        j.update(st, &[space()])
    }

    fn popped(cmd: &ScreenCmd) -> bool {
        matches!(cmd, ScreenCmd::Pop { .. })
    }

    fn draw(st: &AppState, j: &LockJack) -> String {
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| j.draw(st, f)).unwrap();
        buffer_text(t.backend().buffer())
    }

    fn panel_text(st: &AppState, at: ContainerAt) -> String {
        let p = ContainerPanel::open(st, at);
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| p.draw(st, f)).unwrap();
        buffer_text(t.backend().buffer())
    }

    /// A locked container with a fused Keyjack in the kit, you beside it.
    fn locked() -> (AppState, Container) {
        let (mut st, c) = copy_with_spot(150);
        beside(&mut st, &c);
        assert!(crate::keyjack::fuses(&st) > 0, "the kit carries a fuse");
        (st, c)
    }

    #[test]
    fn four_hits_open_the_container() {
        let (mut st, c) = locked();
        let mut j = LockJack::open(&st, c.at).expect("clipped on");
        let text = draw(&st, &j);
        assert!(text.contains("KEYJACK") && text.contains("grade"), "{text}");
        for beat in 0..4 {
            assert_eq!(j.clip.beat, beat);
            assert!(matches!(strike(&mut j, &mut st, true), ScreenCmd::Stay));
        }
        assert_eq!(
            crate::containers::state(&st, c.at).and_then(|s| s.opened),
            Some(Opened::Keyjack)
        );
        assert_eq!(j.digits().len(), 4);
        let cmd = j.update(&mut st, &[key(KeyCode::Char('1'))]);
        assert!(matches!(cmd, ScreenCmd::Replace { .. }));
    }

    #[test]
    fn it_goes_by_itself_to_the_open_panel() {
        let (mut st, c) = locked();
        let mut j = LockJack::open(&st, c.at).unwrap();
        for _ in 0..4 {
            strike(&mut j, &mut st, true);
        }
        assert!(matches!(j.update(&mut st, &[]), ScreenCmd::Stay));
        st.tick += OPEN_TICKS + 1;
        assert!(matches!(j.update(&mut st, &[]), ScreenCmd::Replace { .. }));
    }

    #[test]
    fn the_last_fault_burns_the_fuse_and_locks_out_only_the_keyjack() {
        let (mut st, c) = locked();
        let fuses = crate::keyjack::fuses(&st);
        let mut j = LockJack::open(&st, c.at).unwrap();
        let allowed = j.clip.run.faults;
        for _ in 0..allowed {
            assert!(matches!(strike(&mut j, &mut st, false), ScreenCmd::Stay));
        }
        let now = st.net.now(&st);
        assert_eq!(
            ways::keyjack_now(&st, c.at),
            (allowed, Some(now + LOCKOUT_MINUTES))
        );
        assert_eq!(crate::keyjack::fuses(&st), fuses - 1, "the fuse is gone");
        assert!(
            st.log.iter().any(|e| {
                e.text == "The Keyjack's fuse blew. The lock is out of reach for 30m."
            })
        );
        assert!(draw(&st, &j).contains("FUSE  BLOWN"));
        // It stays until a key, then pops.
        assert!(matches!(j.update(&mut st, &[]), ScreenCmd::Stay));
        assert!(popped(&j.update(&mut st, &[space()])));
        assert!(LockJack::open(&st, c.at).is_none());
        let text = panel_text(&st, c.at);
        assert!(text.contains("locked 30m"), "{text}");
        // The other ways stay: force it is still a row, and the card opens it.
        assert!(text.contains("force it"), "{text}");
        let mut p = ContainerPanel::open(&st, c.at);
        p.update(&mut st, &[]);
        let before = st.log.len();
        p.update(&mut st, &[key(KeyCode::Char('k'))]);
        assert_eq!(st.log.len(), before, "k does nothing while locked out");
        let b = c.at.building;
        crate::credentials::stock_zone_cards(&mut st, b);
        let need = c.zone.unwrap_or(0).max(1);
        st.character
            .give_item(&st.content, crate::credentials::zone_card_id(b, need), 1);
        let mut p = ContainerPanel::open(&st, c.at);
        p.update(&mut st, &[]);
        assert_eq!(
            crate::containers::state(&st, c.at).and_then(|s| s.opened),
            Some(Opened::Card)
        );
    }

    #[test]
    fn a_lockout_run_out_clears_the_faults() {
        let (mut st, c) = locked();
        let mut j = LockJack::open(&st, c.at).unwrap();
        for _ in 0..j.clip.run.faults {
            strike(&mut j, &mut st, false);
        }
        assert!(LockJack::open(&st, c.at).is_none());
        // Give it a fuse back, and let the half hour pass.
        let spare = game::ItemId::new("fuse_stock");
        st.character.give(spare.clone(), 1);
        for s in st.character.inventory.iter_mut() {
            if s.item.0 == crate::keyjack::ITEM
                && let Some(slot) = s.copy.as_mut().and_then(|k| k.fitted.first_mut())
            {
                *slot = Some(game::Stack::new(spare.clone(), 1));
            }
        }
        let (city, world) = (st.city.as_ref().unwrap(), st.world.as_mut().unwrap());
        world.advance(
            city,
            world.tick() + (LOCKOUT_MINUTES + 1) * sim::TICKS_PER_MINUTE,
        );
        assert_eq!(ways::keyjack_now(&st, c.at), (0, None));
        assert!(LockJack::open(&st, c.at).is_some());
        assert!(!panel_text(&st, c.at).contains("locked 30m"));
    }

    #[test]
    fn faults_stay_when_it_is_pulled_off() {
        let (mut st, c) = locked();
        let mut j = LockJack::open(&st, c.at).unwrap();
        strike(&mut j, &mut st, true);
        strike(&mut j, &mut st, false);
        assert!(popped(&j.update(&mut st, &[key(KeyCode::Esc)])));
        assert_eq!(ways::keyjack_now(&st, c.at), (1, None));
        let j = LockJack::open(&st, c.at).unwrap();
        assert_eq!((j.clip.beat, j.clip.faults), (0, 1), "beats start over");
    }

    #[test]
    fn not_in_a_fight() {
        let (mut st, c) = locked();
        st.net.engaged = Some(crate::net::Engagement {
            place: citygen::Place::Stratum(citygen::StratumKind::Net),
            spent_at: None,
            boosted: false,
            round: 1,
            order: vec![crate::net::Actor::You],
            turn: 0,
            moves: 1,
            ap: 2,
            phase: crate::net::Phase::Act,
            manual: false,
            acting_since: 0,
        });
        assert!(LockJack::open(&st, c.at).is_none());
        let mut p = ContainerPanel::open(&st, c.at);
        p.update(&mut st, &[]);
        let cmd = p.update(&mut st, &[key(KeyCode::Char('k'))]);
        assert!(matches!(cmd, ScreenCmd::Stay));
        assert_eq!(st.log.latest().unwrap().text, "not now");
    }

    #[test]
    fn k_on_the_panel_clips_it_on() {
        let (mut st, c) = locked();
        let mut p = ContainerPanel::open(&st, c.at);
        p.update(&mut st, &[]);
        let cmd = p.update(&mut st, &[key(KeyCode::Char('k'))]);
        assert!(matches!(cmd, ScreenCmd::Replace { .. }));
    }
}
