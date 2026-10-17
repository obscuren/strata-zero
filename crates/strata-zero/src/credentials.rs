//! Credentials: keycards in the pack, door codes in the journal, and the one
//! question a locked flat asks. Cards are items; codes are knowledge; the Net's
//! lock crack is still the third way in.

use crate::app::AppState;
use citygen::{BuildingId, Place, Role};
use game::{Item, ItemId, ItemKind};
use sim::HouseholdId;
use std::collections::{BTreeMap, BTreeSet};

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

/// The household that left its card at the lobby desk.
pub fn lost_household(st: &AppState, b: BuildingId) -> Option<HouseholdId> {
    let homes = st.interiors.homes_of(b)?;
    let hs: Vec<HouseholdId> = homes.households().map(|(h, _)| h).collect();
    if hs.is_empty() {
        return None;
    }
    let seed = st.city.as_ref()?.seed;
    let pick = citygen::rng::hash64(seed, b.index() as u64 ^ 0xCA2D) % hs.len() as u64;
    Some(hs[pick as usize])
}

/// Lays the lost card on the lobby floor, once: on the reception desk, else a table,
/// else the first free floor tile. The lobby is generated for it if need be.
pub fn drop_lost_card(st: &mut AppState, b: BuildingId) {
    if st.journal.lost_cards.contains(&b) {
        return;
    }
    let Some(h) = lost_household(st, b) else {
        return;
    };
    let Some(city) = st.city.as_ref() else {
        return;
    };
    if st.interiors.ensure(city, b, 0).is_none() {
        return;
    }
    let Some((_, floor)) = st.interiors.floor(b, 0) else {
        return;
    };
    let station = |role: Role| {
        floor
            .stations
            .iter()
            .find(|s| s.role == role)
            .map(|s| s.pos)
    };
    let tile = station(Role::Reception)
        .or_else(|| station(Role::Table))
        .filter(|p| floor.tiles[*p].is_walkable())
        .or_else(|| {
            floor
                .tiles
                .iter()
                .find(|(_, t)| t.is_walkable() && t.feature.is_none())
                .map(|(p, _)| p)
        });
    let Some(tile) = tile else {
        return;
    };
    st.ground.add(
        Place::Floor {
            building: b,
            level: 0,
        },
        tile,
        keycard_id(b, h),
        1,
    );
    st.journal.lost_cards.insert(b);
}

/// What a person has on them: their flat's keycard, unless it is the lost one.
pub fn carries(st: &AppState, person: sim::PersonId) -> Vec<ItemId> {
    let Some(world) = st.world.as_ref() else {
        return Vec::new();
    };
    let p = &world.people()[person.0 as usize];
    let Some(b) = p.home else {
        return Vec::new();
    };
    let Some(homes) = st.interiors.homes_of(b) else {
        return Vec::new();
    };
    if homes.flat_of(p.household).is_none() || lost_household(st, b) == Some(p.household) {
        return Vec::new();
    }
    vec![keycard_id(b, p.household)]
}

/// A server copy on a residential building: every flat's code into the journal.
/// `None` when nobody lives there, else how many codes were new.
pub fn copy_codes(st: &mut AppState, b: BuildingId, now: u64) -> Option<usize> {
    prepare(st, b);
    let homes = st.interiors.homes_of(b).filter(|h| !h.is_empty())?.clone();
    let seed = st.city.as_ref()?.seed;
    let mut new = 0;
    for (h, (level, number)) in homes.households() {
        let learned = st.journal.learn(Code {
            building: b,
            level,
            number,
            household: Some(h),
            code: citygen::rng::door_code(seed, b, level, number),
            source: Source::ServerCopy,
            at: now,
        });
        if learned {
            new += 1;
        }
    }
    Some(new)
}

/// `day N HH:MM` for a game minute.
pub fn stamp(minute: u64) -> String {
    let tick = minute * sim::TICKS_PER_MINUTE;
    let (h, m) = crate::ui::hud::clock_at(tick);
    format!("day {} {h:02}:{m:02}", crate::ui::hud::day_at(tick))
}

/// What a journal line is about.
#[derive(Clone, Debug, PartialEq)]
pub enum EntryKind {
    Code(Code),
    Card(ItemId),
    Backdoor,
}

/// One journal line.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub building: BuildingId,
    pub kind: EntryKind,
    /// One line, names in backticks.
    pub text: String,
    /// The game minute it dates from; a card has none.
    pub at: Option<u64>,
}

fn owner_name(st: &AppState, b: BuildingId, level: i32, number: u16) -> Option<String> {
    let owner = st.interiors.homes_of(b)?.owner_of(level, number)?;
    Some(st.world.as_ref()?.people()[owner.0 as usize].name.clone())
}

fn whose_flat(st: &AppState, b: BuildingId, level: i32, number: u16) -> String {
    owner_name(st, b, level, number)
        .map(|n| format!("`{n}`'s flat"))
        .unwrap_or_else(|| "a flat".to_string())
}

/// The journal, grouped per building: (building, header, entries). Codes first,
/// then cards, then the backdoor; buildings with a dated entry first, newest first,
/// then those known only by a card.
pub fn entries(st: &AppState) -> Vec<(BuildingId, String, Vec<Entry>)> {
    let mut by: BTreeMap<BuildingId, Vec<Entry>> = BTreeMap::new();
    for c in st.journal.codes() {
        let whose = whose_flat(st, c.building, c.level, c.number);
        let from = match c.source {
            Source::ServerCopy => format!("copied off the server, {}", stamp(c.at)),
            Source::Home => "your own door".to_string(),
        };
        by.entry(c.building).or_default().push(Entry {
            building: c.building,
            kind: EntryKind::Code(c.clone()),
            text: format!(
                "door code {:04} · {whose} `{}`, level {} · {from}",
                c.code, c.number, c.level
            ),
            at: Some(c.at),
        });
    }
    for s in &st.character.inventory {
        if s.item.0 == HOME_KEYCARD {
            if let Some(rig) = st.home {
                let h = rig.home;
                by.entry(h.building).or_default().push(Entry {
                    building: h.building,
                    kind: EntryKind::Card(s.item.clone()),
                    text: format!(
                        "keycard · your flat `{}`, level {} · in your pack",
                        h.number, h.level
                    ),
                    at: None,
                });
            }
            continue;
        }
        let Some((b, h)) = parse_keycard(&s.item) else {
            continue;
        };
        let Some((level, number)) = st.interiors.homes_of(b).and_then(|x| x.flat_of(h)) else {
            continue;
        };
        let whose = whose_flat(st, b, level, number);
        by.entry(b).or_default().push(Entry {
            building: b,
            kind: EntryKind::Card(s.item.clone()),
            text: format!("keycard · {whose} `{number}`, level {level} · in your pack"),
            at: None,
        });
    }
    for b in &st.net.backdoors {
        let at = st
            .net
            .hacks
            .iter()
            .find(|h| h.building == *b && h.op == game::Op::Backdoor)
            .map(|h| h.at);
        let name = crate::net::building_name(st, *b);
        let when = at
            .map(|a| format!(" · planted {}", stamp(a)))
            .unwrap_or_default();
        by.entry(*b).or_default().push(Entry {
            building: *b,
            kind: EntryKind::Backdoor,
            text: format!("backdoor · `{name}`'s gate is yours{when}"),
            at,
        });
    }
    let district = |b: BuildingId| -> String {
        st.city
            .as_ref()
            .and_then(|c| {
                c.building_opt(b)
                    .map(|x| c.district(x.district).name.clone())
            })
            .unwrap_or_default()
    };
    let mut groups: Vec<(BuildingId, String, Vec<Entry>)> = by
        .into_iter()
        .map(|(b, es)| {
            let name = crate::net::building_name(st, b).to_uppercase();
            (b, format!("{name} · {}", district(b)), es)
        })
        .collect();
    groups.sort_by_key(|(b, _, es)| {
        (
            std::cmp::Reverse(es.iter().filter_map(|e| e.at).max()),
            b.index(),
        )
    });
    groups
}

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

    #[test]
    fn one_lost_card_lies_in_the_lobby_once_and_its_owner_goes_without() {
        let (mut st, b) = state();
        prepare(&mut st, b);
        let lost = lost_household(&st, b).expect("a household lost its card");
        let id = keycard_id(b, lost);
        let lobby = citygen::Place::Floor {
            building: b,
            level: 0,
        };
        let on_floor = |st: &crate::app::AppState| {
            st.ground
                .on(lobby)
                .map(|(_, s)| {
                    s.iter()
                        .filter(|s| s.item == id)
                        .map(|s| s.qty)
                        .sum::<u32>()
                })
                .sum::<u32>()
        };
        assert_eq!(on_floor(&st), 1);
        let (pos, _) = st
            .ground
            .on(lobby)
            .find(|(_, s)| s.iter().any(|s| s.item == id))
            .unwrap();
        let (_, floor) = st.interiors.floor(b, 0).unwrap();
        assert!(floor.tiles[pos].is_walkable());
        prepare(&mut st, b);
        assert_eq!(on_floor(&st), 1, "no second card");
        st.ground.take(lobby, pos, &id, 1);
        prepare(&mut st, b);
        assert_eq!(on_floor(&st), 0, "picked up stays picked up");
        let homes = st.interiors.homes_of(b).unwrap().clone();
        let world = st.world.as_ref().unwrap();
        let loser = world
            .people()
            .iter()
            .find(|p| p.household == lost)
            .unwrap()
            .id;
        assert!(carries(&st, loser).is_empty());
        let other = homes
            .households()
            .map(|(h, _)| h)
            .find(|h| *h != lost)
            .map(|h| world.people().iter().find(|p| p.household == h).unwrap().id);
        if let Some(o) = other {
            assert_eq!(
                carries(&st, o),
                vec![keycard_id(b, world.people()[o.0 as usize].household)]
            );
        }
    }

    #[test]
    fn a_server_copy_learns_every_flats_code_once() {
        let (mut st, b) = state();
        // Never walked in: copy_codes prepares the building itself.
        assert!(st.interiors.homes_of(b).is_none());
        let n = copy_codes(&mut st, b, 7).expect("a residential building");
        let flats = st.interiors.homes_of(b).unwrap().flats().count();
        assert_eq!(n, flats);
        assert_eq!(st.journal.codes().len(), flats);
        assert!(
            st.journal
                .codes()
                .iter()
                .all(|c| { c.source == Source::ServerCopy && c.at == 7 && c.household.is_some() })
        );
        assert_eq!(copy_codes(&mut st, b, 8), Some(0));
        let office = st
            .city
            .as_ref()
            .unwrap()
            .buildings
            .iter()
            .find(|x| x.kind == BuildingKind::Office)
            .unwrap()
            .id;
        assert_eq!(copy_codes(&mut st, office, 9), None);
    }

    #[test]
    fn journal_entries_group_by_building_and_say_whose() {
        let (mut st, b) = state();
        copy_codes(&mut st, b, 130).unwrap();
        let homes = st.interiors.homes_of(b).unwrap().clone();
        let (h, (level, number)) = homes.households().next().unwrap();
        st.character.give(keycard_id(b, h), 1);
        let now = st.net.now(&st);
        let server = st
            .city
            .as_ref()
            .unwrap()
            .net
            .nodes
            .iter()
            .find(|n| n.building == Some(b) && n.kind == citygen::NodeKind::Server)
            .map(|n| n.id)
            .expect("a server");
        st.net.land(
            b,
            game::HackTarget::Node(server),
            game::Target::Node(citygen::NodeKind::Server),
            game::Op::Backdoor,
            now,
        );
        let groups = entries(&st);
        assert_eq!(groups.len(), 1);
        let (gb, header, es) = &groups[0];
        assert_eq!(*gb, b);
        let name = crate::net::building_name(&st, b);
        assert!(header.starts_with(&name.to_uppercase()), "{header}");
        assert!(header.contains(" · "), "{header}");
        let owner = st.world.as_ref().unwrap().people()
            [homes.owner_of(level, number).unwrap().0 as usize]
            .name
            .clone();
        let code = es
            .iter()
            .find(|e| matches!(&e.kind, EntryKind::Code(c) if c.number == number))
            .unwrap();
        assert!(code.text.starts_with("door code "), "{}", code.text);
        assert!(
            code.text
                .contains(&format!("`{owner}`'s flat `{number}`, level {level}")),
            "{}",
            code.text
        );
        assert!(
            code.text.ends_with("copied off the server, day 2 00:10"),
            "{}",
            code.text
        );
        let card = es
            .iter()
            .find(|e| matches!(e.kind, EntryKind::Card(_)))
            .unwrap();
        assert!(
            card.text.starts_with("keycard · ") && card.text.ends_with("in your pack"),
            "{}",
            card.text
        );
        let door = es
            .iter()
            .find(|e| matches!(e.kind, EntryKind::Backdoor))
            .unwrap();
        assert!(
            door.text.starts_with(&format!(
                "backdoor · `{name}`'s gate is yours · planted day"
            )),
            "{}",
            door.text
        );
        assert_eq!(stamp(0), "day 1 22:00");
        assert_eq!(stamp(120), "day 2 00:00");
    }
}
