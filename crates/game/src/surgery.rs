//! The doc's fees: what a ripperdoc charges to put a piece of chrome in or take it out,
//! by the piece's value at the rates in content.

use crate::action::{Action, Host};
use crate::character::Character;
use crate::content::Content;
use crate::model::ItemId;
use serde::{Deserialize, Serialize};

/// The doc's rates, in percent of a piece's value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Surgery {
    /// Putting a piece in: installing it at a site, or fitting it in a limb's socket.
    pub install: u32,
    /// Taking a piece out: pulling it from a site, or out of a socket.
    pub pull: u32,
}

impl Default for Surgery {
    fn default() -> Surgery {
        Surgery {
            install: 12,
            pull: 4,
        }
    }
}

/// `pct` of `value`, to the nearest ¤10, at least ¤10.
fn rate(value: u32, pct: u32) -> u32 {
    ((value * pct + 500) / 1000 * 10).max(10)
}

fn value(c: &Content, item: &ItemId) -> Option<u32> {
    c.item(item).map(|i| i.value)
}

/// What the doc charges to put `item` in; nothing for an unknown item.
pub fn put_in_fee(c: &Content, item: &ItemId) -> u32 {
    value(c, item).map_or(0, |v| rate(v, c.surgery.install))
}

/// What the doc charges to take `item` out; nothing for an unknown item.
pub fn take_out_fee(c: &Content, item: &ItemId) -> u32 {
    value(c, item).map_or(0, |v| rate(v, c.surgery.pull))
}

/// The fee for a chrome action: the piece it moves in, and what it moves out of the
/// place to make room. What is fitted inside a moved piece rides along free. `None`
/// for anything that isn't surgery.
pub fn surgery_fee(ch: &Character, c: &Content, action: &Action) -> Option<u32> {
    let out = |held: Option<&ItemId>| held.map_or(0, |i| take_out_fee(c, i));
    let in_socket = |site, socket: usize| {
        ch.chrome_copy(site)
            .and_then(|k| k.fitted.get(socket))
            .and_then(Option::as_ref)
            .map(|s| &s.item)
    };
    match action {
        Action::Install { item, site } => Some(put_in_fee(c, item) + out(ch.chrome_at(*site))),
        Action::InstallCopy { copy, site } => {
            let piece = ch.copy_in_pack(*copy).map(|s| &s.item);
            Some(piece.map_or(0, |i| put_in_fee(c, i)) + out(ch.chrome_at(*site)))
        }
        Action::Pull(site) => Some(out(ch.chrome_at(*site))),
        Action::Fit {
            host: Host::Site(site),
            socket,
            piece,
        } => {
            let piece = ch.copy_in_pack(*piece).map(|s| &s.item);
            Some(piece.map_or(0, |i| put_in_fee(c, i)) + out(in_socket(*site, *socket)))
        }
        Action::Unfit {
            host: Host::Site(site),
            socket,
        } => Some(out(in_socket(*site, *socket))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::Stack;
    use crate::model::{Site, Slot, TemplateId};

    fn id(s: &str) -> ItemId {
        ItemId::new(s)
    }

    fn vex(c: &Content) -> Character {
        Character::from_template(c, &TemplateId::new("vex")).unwrap()
    }

    /// A piston arm at the left arm with an arc hand in its hand socket.
    fn armed(c: &Content) -> Character {
        let mut ch = vex(c);
        let mut arm = ch.new_copy(c, &id("piston_arm"));
        let hand = ch.new_copy(c, &id("arc_hand"));
        arm.fitted[0] = Some(Stack {
            item: id("arc_hand"),
            qty: 1,
            copy: Some(hand),
        });
        ch.install(Site::LeftArm, id("piston_arm"), Some(arm));
        ch
    }

    fn pack_copy(ch: &Character, item: &str) -> crate::character::CopyId {
        ch.inventory
            .iter()
            .find(|s| s.item == id(item))
            .and_then(|s| s.copy_id())
            .unwrap()
    }

    #[test]
    fn fees_follow_the_table() {
        let c = Content::load().unwrap();
        for (item, put, take) in [
            ("smart_ink", 80, 30),
            ("neural_port", 180, 60),
            ("target_lens", 260, 90),
            ("arc_hand", 310, 100),
            ("piston_arm", 340, 110),
            ("grip_arm", 370, 120),
            ("wrecker_arm", 590, 200),
            ("redline_spine", 620, 210),
            ("ceramic_plating", 820, 270),
        ] {
            assert_eq!(put_in_fee(&c, &id(item)), put, "{item} in");
            assert_eq!(take_out_fee(&c, &id(item)), take, "{item} out");
        }
    }

    #[test]
    fn the_floor_is_ten() {
        let mut c = Content::load().unwrap();
        let cheap = c
            .items
            .iter_mut()
            .find(|i| i.id == id("smart_ink"))
            .unwrap();
        cheap.value = 50;
        assert_eq!(put_in_fee(&c, &id("smart_ink")), 10);
        assert_eq!(take_out_fee(&c, &id("smart_ink")), 10);
        assert_eq!(put_in_fee(&c, &id("no_such_thing")), 0);
    }

    #[test]
    fn a_swap_is_in_plus_out() {
        let c = Content::load().unwrap();
        let mut ch = armed(&c);
        ch.give_item(&c, id("grip_arm"), 1);
        let swap = Action::Install {
            item: id("grip_arm"),
            site: Site::LeftArm,
        };
        assert_eq!(surgery_fee(&ch, &c, &swap), Some(370 + 110));
        let copy = Action::InstallCopy {
            copy: pack_copy(&ch, "grip_arm"),
            site: Site::RightArm,
        };
        assert_eq!(surgery_fee(&ch, &c, &copy), Some(370));
    }

    #[test]
    fn fitted_pieces_ride_free() {
        let c = Content::load().unwrap();
        let mut ch = armed(&c);
        assert_eq!(
            surgery_fee(&ch, &c, &Action::Pull(Site::LeftArm)),
            Some(110)
        );
        let unfit = Action::Unfit {
            host: Host::Site(Site::LeftArm),
            socket: 0,
        };
        assert_eq!(surgery_fee(&ch, &c, &unfit), Some(100));
        ch.give_item(&c, id("arc_hand"), 1);
        let fit = Action::Fit {
            host: Host::Site(Site::LeftArm),
            socket: 1,
            piece: pack_copy(&ch, "arc_hand"),
        };
        assert_eq!(surgery_fee(&ch, &c, &fit), Some(310));
    }

    #[test]
    fn other_actions_have_no_fee() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        assert_eq!(
            surgery_fee(&ch, &c, &Action::Equip(id("salvage_jacket"))),
            None
        );
        assert_eq!(surgery_fee(&ch, &c, &Action::Use(id("stim_patch"))), None);
        ch.give_item(&c, id("arc_hand"), 1);
        let deck = Action::Fit {
            host: Host::Worn(Slot::Deck),
            socket: 0,
            piece: pack_copy(&ch, "arc_hand"),
        };
        assert_eq!(surgery_fee(&ch, &c, &deck), None);
    }

    #[test]
    fn rates_come_from_content() {
        let mut c = Content::load().unwrap();
        c.surgery = Surgery {
            install: 10,
            pull: 5,
        };
        assert_eq!(put_in_fee(&c, &id("grip_arm")), 310);
        assert_eq!(take_out_fee(&c, &id("grip_arm")), 160);
    }

    #[test]
    fn content_carries_the_rates() {
        assert_eq!(Content::load().unwrap().surgery, Surgery::default());
    }
}
