//! What the rig does, read off the sleds in its rack: seats, clock, backdoor lines,
//! how long a backdoor holds and how hard the firewall fights a trace.

pub mod jobs;

use crate::app::AppState;
use game::{Character, Content, Mod};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RigSpec {
    /// Programs the rig runs at once.
    pub seats: u8,
    /// Clock, in percent; 100 is stock.
    pub speed: u16,
    /// Backdoors the rig can keep.
    pub lines: usize,
    /// Days a Net backdoor holds on this line.
    pub hold_days: u64,
    /// 0 none, 1 sandbag, 2 tripwire, 3 killswitch.
    pub firewall: u8,
}

/// The stock Zenbo rig: ZB-8, R-3 and CU-1.
#[cfg(test)] // the tests' yardstick
pub const STOCK: RigSpec = RigSpec {
    seats: 1,
    speed: 100,
    lines: 3,
    hold_days: 3,
    firewall: 0,
};

/// What the sleds fitted in the character's rig give: seats and lines add up, the
/// rest take the best fitted.
pub fn spec(ch: &Character, c: &Content) -> RigSpec {
    let mut s = RigSpec {
        seats: 0,
        speed: 100,
        lines: 0,
        hold_days: 0,
        firewall: 0,
    };
    for (_, sled) in ch.rig_fitted(c) {
        let Some(item) = sled.and_then(|id| c.item(&id)) else {
            continue;
        };
        let Some(hw) = &item.hardware else { continue };
        for m in &hw.mods {
            match *m {
                Mod::Seats(n) => s.seats = s.seats.saturating_add(n),
                Mod::Speed(n) => s.speed = s.speed.max(n),
                Mod::Lines(n) => s.lines += n as usize,
                Mod::Hold(n) => s.hold_days = s.hold_days.max(n as u64),
                Mod::Firewall(n) => s.firewall = s.firewall.max(n),
                _ => {}
            }
        }
    }
    s
}

pub fn of(st: &AppState) -> RigSpec {
    spec(&st.character, &st.content)
}

/// A new rack of `id` into the pack and fitted as the rig, as the tests and shots set
/// one up; the old rack goes to the pack.
#[cfg(test)]
pub(crate) fn fit_new_rack(ch: &mut Character, c: &Content, id: &str) {
    ch.give_item(c, game::ItemId::new(id), 1);
    ch.set_frame(c, &game::ItemId::new(id)).expect("a rack");
}

#[cfg(test)]
mod tests {
    use super::*;
    use game::{ItemId, Stack, TemplateId};

    fn vex(c: &Content) -> Character {
        Character::from_template(c, &TemplateId::new("vex")).unwrap()
    }

    fn fit(ch: &mut Character, c: &Content, bay: usize, sled: &str) {
        ch.rig_copy_mut(c).fitted[bay] = Some(Stack {
            item: ItemId::new(sled),
            qty: 1,
            copy: None,
        });
    }

    #[test]
    fn the_stock_rig_is_todays_numbers() {
        let c = Content::load().unwrap();
        assert_eq!(spec(&vex(&c), &c), STOCK);
    }

    #[test]
    fn two_cores_add_their_seats() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        crate::rig::fit_new_rack(&mut ch, &c, "rack_lotus");
        fit(&mut ch, &c, 0, "sled_tinman");
        fit(&mut ch, &c, 1, "sled_qstack");
        let s = spec(&ch, &c);
        assert_eq!((s.seats, s.speed), (5, 150));
    }

    #[test]
    fn an_empty_bay_gives_nothing() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        ch.rig_copy_mut(&c).fitted[1] = None;
        assert_eq!(spec(&ch, &c).lines, 0);
        ch.rig_copy_mut(&c).fitted[2] = None;
        assert_eq!(spec(&ch, &c).hold_days, 0);
    }

    #[test]
    fn the_best_firewall_counts() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        crate::rig::fit_new_rack(&mut ch, &c, "rack_civitas");
        fit(&mut ch, &c, 3, "sled_tripwire");
        assert_eq!(spec(&ch, &c).firewall, 2);
    }
}
