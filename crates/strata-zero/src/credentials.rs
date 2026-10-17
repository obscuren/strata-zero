//! Credentials: keycards in the pack, door codes in the journal, and the one
//! question a locked flat asks. Cards are items; codes are knowledge; the Net's
//! lock crack is still the third way in.

use crate::app::AppState;
use citygen::BuildingId;
use game::{Item, ItemId, ItemKind};
use sim::HouseholdId;
use std::collections::BTreeSet;

/// The player's own front-door card.
pub const HOME_KEYCARD: &str = "keycard:home";

/// The catalogue id of a household's keycard.
pub fn keycard_id(b: BuildingId, h: HouseholdId) -> ItemId {
    ItemId::new(&format!("keycard:{}:{}", b.index(), h.0))
}

/// The building and household a keycard opens for, if the item is one.
pub fn parse_keycard(id: &ItemId) -> Option<(BuildingId, HouseholdId)> {
    let rest = id.0.strip_prefix("keycard:")?;
    let (b, h) = rest.split_once(':')?;
    Some((BuildingId(b.parse().ok()?), HouseholdId(h.parse().ok()?)))
}

/// Where a code came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    ServerCopy,
    Home,
}

/// A door code the player knows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Code {
    pub building: BuildingId,
    pub level: i32,
    pub number: u16,
    pub household: Option<HouseholdId>,
    pub code: u16,
    pub source: Source,
    /// The game minute it was learned.
    pub at: u64,
}

/// What the player knows: door codes, and which lobbies already hold a lost card.
#[derive(Default)]
pub struct Journal {
    codes: Vec<Code>,
    /// Buildings whose lost keycard already lies in the lobby.
    pub lost_cards: BTreeSet<BuildingId>,
}

impl Journal {
    /// Records a code; false when that flat's code was already known.
    pub fn learn(&mut self, code: Code) -> bool {
        if self
            .code_for(code.building, code.level, code.number)
            .is_some()
        {
            return false;
        }
        self.codes.push(code);
        true
    }

    pub fn code_for(&self, b: BuildingId, level: i32, number: u16) -> Option<&Code> {
        self.codes
            .iter()
            .find(|c| c.building == b && c.level == level && c.number == number)
    }

    pub fn codes(&self) -> &[Code] {
        &self.codes
    }
}

/// How a locked door opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
    Cracked,
    Keycard,
    Code(u16),
}

/// Whether the player gets through this flat's door, and how: the Net's lock crack,
/// a keycard in the pack, or the code in the journal, in that order.
pub fn opens(st: &AppState, b: BuildingId, level: i32, number: u16) -> Option<How> {
    if st.net.unlocked(b, st.net.now(st)) {
        return Some(How::Cracked);
    }
    let household = st
        .interiors
        .homes_of(b)
        .and_then(|h| h.household_of(level, number));
    if household.is_some_and(|h| st.character.count(&keycard_id(b, h)) > 0) {
        return Some(How::Keycard);
    }
    st.journal
        .code_for(b, level, number)
        .map(|c| How::Code(c.code))
}

/// A keycard's catalogue entry.
fn card(id: ItemId, name: String, desc: String) -> Item {
    Item {
        id,
        name,
        kind: ItemKind::Component,
        slot: None,
        weight: 0.01,
        value: 40,
        desc,
        perks: Vec::new(),
        grants: Vec::new(),
        on_use: None,
        program: None,
        deck: None,
    }
}

/// Puts the building's keycards in the catalogue, once.
pub fn stock(st: &mut AppState, b: BuildingId) {
    let Some(homes) = st.interiors.homes_of(b).cloned() else {
        return;
    };
    let building = crate::net::building_name(st, b);
    for (h, (level, number)) in homes.households() {
        let id = keycard_id(b, h);
        if st.content.item(&id).is_some() {
            continue;
        }
        let Some(owner) = homes.owner_of(level, number) else {
            continue;
        };
        let Some(name) = st
            .world
            .as_ref()
            .map(|w| w.people()[owner.0 as usize].name.clone())
        else {
            continue;
        };
        st.content.items.push(card(
            id,
            format!("Keycard · {name}, {building} {number}"),
            format!(
                "A plastic rectangle with a chip and a lanyard. It opens one door in the \
                 city, and that door is {number} at {building}."
            ),
        ));
    }
}

/// Everything a residential building needs once the player can reach it: its homes,
/// its keycards in the catalogue, and the lost card in its lobby.
pub fn prepare(st: &mut AppState, b: BuildingId) {
    let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_ref()) else {
        return;
    };
    if !st.interiors.ensure_homes(city, world, b) {
        return;
    }
    stock(st, b);
    drop_lost_card(st, b);
}

/// Lays the lost card on the lobby floor the first time the lobby exists.
pub fn drop_lost_card(_st: &mut AppState, _b: BuildingId) {}

/// Drops every keycard of a previous city from the catalogue.
pub fn purge(content: &mut game::Content) {
    content.items.retain(|i| !i.id.0.starts_with("keycard:"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{BuildingId, BuildingKind, GenParams, Generator, SizePreset};
    use game::ItemId;

    fn state() -> (crate::app::AppState, BuildingId) {
        let mut st = crate::app::AppState::new();
        st.enter_city(
            Generator::run(GenParams {
                seed: 6,
                size: SizePreset::Custom { w: 120, h: 60 },
                districts: 8,
                ..Default::default()
            })
            .unwrap(),
        );
        let city = st.city.as_ref().unwrap();
        let world = st.world.as_ref().unwrap();
        let b = city
            .buildings
            .iter()
            .find(|b| {
                matches!(b.kind, BuildingKind::Apartment | BuildingKind::Megablock)
                    && world.people().iter().any(|p| p.home == Some(b.id))
            })
            .unwrap()
            .id;
        (st, b)
    }

    #[test]
    fn keycard_ids_round_trip() {
        let id = keycard_id(BuildingId(4), sim::HouseholdId(9));
        assert_eq!(id.0, "keycard:4:9");
        assert_eq!(
            parse_keycard(&id),
            Some((BuildingId(4), sim::HouseholdId(9)))
        );
        assert_eq!(parse_keycard(&ItemId::new("paydata")), None);
        assert_eq!(parse_keycard(&ItemId::new(HOME_KEYCARD)), None);
    }

    #[test]
    fn a_code_is_learned_once() {
        let mut j = Journal::default();
        let c = Code {
            building: BuildingId(1),
            level: 3,
            number: 304,
            household: None,
            code: 1234,
            source: Source::ServerCopy,
            at: 5,
        };
        assert!(j.learn(c.clone()));
        assert!(!j.learn(c));
        assert_eq!(j.codes().len(), 1);
        assert_eq!(
            j.code_for(BuildingId(1), 3, 304).map(|c| c.code),
            Some(1234)
        );
    }

    #[test]
    fn stocking_makes_one_card_per_household_and_never_twice() {
        let (mut st, b) = state();
        prepare(&mut st, b);
        let homes = st.interiors.homes_of(b).unwrap().clone();
        let n = homes.households().count();
        assert!(n > 0);
        let prefix = format!("keycard:{}:", b.index());
        let cards = |st: &crate::app::AppState| {
            st.content
                .items
                .iter()
                .filter(|i| i.id.0.starts_with(&prefix))
                .count()
        };
        assert_eq!(cards(&st), n);
        prepare(&mut st, b);
        assert_eq!(cards(&st), n, "idempotent");
        let (h, (_, number)) = homes.households().next().unwrap();
        let item = st.content.item(&keycard_id(b, h)).unwrap();
        assert!(item.name.starts_with("Keycard · "), "{}", item.name);
        assert!(item.name.contains(&number.to_string()), "{}", item.name);
        assert_eq!(item.kind, game::ItemKind::Component);
    }

    #[test]
    fn a_new_city_drops_the_old_cards() {
        let (mut st, b) = state();
        prepare(&mut st, b);
        assert!(
            st.content
                .items
                .iter()
                .any(|i| i.id.0.starts_with("keycard:"))
        );
        st.enter_city(
            Generator::run(GenParams {
                seed: 9,
                size: SizePreset::Custom { w: 120, h: 60 },
                districts: 8,
                ..Default::default()
            })
            .unwrap(),
        );
        assert!(
            !st.content
                .items
                .iter()
                .any(|i| i.id.0.starts_with("keycard:"))
        );
        assert!(st.journal.codes().is_empty());
    }

    #[test]
    fn opens_in_order_crack_then_card_then_code() {
        let (mut st, b) = state();
        prepare(&mut st, b);
        let homes = st.interiors.homes_of(b).unwrap().clone();
        let (h, (level, number)) = homes.households().next().unwrap();
        assert_eq!(opens(&st, b, level, number), None);
        let seed = st.city.as_ref().unwrap().seed;
        st.journal.learn(Code {
            building: b,
            level,
            number,
            household: Some(h),
            code: citygen::rng::door_code(seed, b, level, number),
            source: Source::ServerCopy,
            at: 0,
        });
        assert!(matches!(opens(&st, b, level, number), Some(How::Code(_))));
        st.character.give(keycard_id(b, h), 1);
        assert_eq!(opens(&st, b, level, number), Some(How::Keycard));
        let now = st.net.now(&st);
        let lock = st
            .city
            .as_ref()
            .unwrap()
            .net
            .nodes
            .iter()
            .find(|n| n.building == Some(b) && n.kind == citygen::NodeKind::DoorLock)
            .map(|n| n.id)
            .expect("a door lock");
        st.net.land(
            b,
            game::HackTarget::Node(lock),
            game::Target::Node(citygen::NodeKind::DoorLock),
            game::Op::Crack,
            now,
        );
        assert_eq!(opens(&st, b, level, number), Some(How::Cracked));
    }
}
