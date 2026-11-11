use super::*;
use crate::app::{AppState, Screen};
use crate::credentials::{
    How, Source, Verdict, open_with, opens_lock, parse_keycard, parse_zone_card, stock_zone_cards,
    try_code, zone_card_id,
};
use crate::messages::Kind;
use crate::screens::explore::MapScreen;
use crate::screens::popup::{Pick, WorldAct};
use crate::screens::testutil::key;
use citygen::venue::{LOCK_NUMBERS, MissionInputs, WayInKind, Ways, room_label};
use citygen::{BuildingKind, City, Floor, GenParams, Generator, Place, SizePreset};
use crossterm::event::KeyCode;

mod gate_line;
mod way_out;

/// The seed-8 city the story is played in.
fn home_city() -> City {
    Generator::run(GenParams {
        seed: 8,
        size: SizePreset::Medium,
        districts: 7,
        ..Default::default()
    })
    .unwrap()
}

/// The home city walked into, you on its street.
fn state() -> AppState {
    let mut st = AppState::new();
    st.enter_city(home_city());
    st.player = Some(crate::walker::Player::spawn(st.city.as_ref().unwrap(), 1));
    st
}

/// The first building of `kind` laid out as a venue, its plan kept and its cards stocked.
fn venue_of(st: &mut AppState, kind: BuildingKind) -> BuildingId {
    let city = st.city.clone().unwrap();
    let b = (city.buildings.iter())
        .filter(|b| b.kind == kind)
        .map(|b| b.id)
        .find(|b| (st.interiors.plan(&city, *b)).is_some_and(|p| p.venue.is_some()))
        .expect("a venue");
    crate::credentials::prepare(st, b);
    b
}

/// The home city, you on its street, and its first building of the kind the first venue
/// type builds.
fn first_venue() -> (AppState, BuildingId) {
    let mut st = state();
    let kind = VenuePack::builtin().types[0].builds[0];
    let b = venue_of(&mut st, kind);
    (st, b)
}

/// The first lock on a level of venue `b` that `pick` takes, that level drawn.
fn lock_where(
    st: &mut AppState,
    b: BuildingId,
    pick: impl Fn(&DoorLock) -> bool,
) -> (i32, DoorLock) {
    let city = st.city.clone().unwrap();
    let levels: Vec<i32> = st.interiors.plan_of(b).unwrap().levels().collect();
    for level in levels {
        let floor = st.interiors.ensure(&city, b, level).unwrap();
        if let Some(lock) = floor.locks.iter().find(|l| pick(l)) {
            return (level, *lock);
        }
    }
    panic!("no such lock in {b:?}");
}

/// The tile a step onto `lock`'s door comes from when it isn't free: beside it, walkable,
/// not its free side.
fn outside_of(floor: &Floor, lock: &DoorLock) -> Pos {
    lock.at
        .neighbors4()
        .find(|n| {
            Some(*n) != lock.free_from && floor.tiles.get(*n).is_some_and(|t| t.is_walkable())
        })
        .expect("a tile beside the door")
}

fn floor_of(st: &AppState, b: BuildingId, level: i32) -> &Floor {
    st.interiors.floor(b, level).unwrap().1
}

fn put(st: &mut AppState, place: Place, pos: Pos) {
    let p = st.player.as_mut().unwrap();
    p.place = place;
    p.pos = pos;
}

fn at(st: &AppState) -> (Place, Pos) {
    let p = st.player.as_ref().unwrap();
    (p.place, p.pos)
}

/// The key that steps from `from` to `to`, a tile apart.
fn step_key(from: Pos, to: Pos) -> KeyCode {
    KeyCode::Char(match (to.x - from.x, to.y - from.y) {
        (1, 0) => 'l',
        (-1, 0) => 'h',
        (0, 1) => 'j',
        _ => 'k',
    })
}

/// Stands you on `pos` of `place` and steps onto `to` on the map.
fn step_onto_from(st: &mut AppState, place: Place, pos: Pos, to: Pos) {
    put(st, place, pos);
    let mut s = MapScreen::walking(st);
    s.update(st, &[key(step_key(pos, to))]);
}

fn last(st: &AppState) -> Option<&str> {
    st.log.latest().map(|e| e.text.as_str())
}

fn warned(st: &AppState) -> bool {
    st.log.latest().is_some_and(|e| e.kind == Kind::Warn)
}

fn plan(st: &AppState, b: BuildingId) -> &FloorPlan {
    st.interiors.plan_of(b).unwrap()
}

/// A card lock of `zone` with the Net's way too, numbered as a level's first.
fn card_lock(zone: u8) -> DoorLock {
    DoorLock {
        at: Pos::new(5, 5),
        number: LOCK_NUMBERS,
        zone,
        kind: LockKind::Card,
        ways: Ways::CARD | Ways::NET,
        free_from: None,
    }
}

#[test]
fn a_zone_card_opens_its_zone_and_those_below() {
    let (mut st, b) = first_venue();
    let from = Pos::new(4, 5);
    assert_eq!(zone_card_id(b, 2).0, format!("keycard:{}:zone2", b.index()));
    assert_eq!(parse_zone_card(&zone_card_id(b, 2)), Some((b, 2)));
    assert_eq!(parse_keycard(&zone_card_id(b, 2)), None);
    assert_eq!(opens_lock(&st, b, 0, &card_lock(1), from), None);
    st.character.give(zone_card_id(b, 2), 1);
    assert_eq!(
        opens_lock(&st, b, 0, &card_lock(2), from),
        Some(How::Keycard)
    );
    assert_eq!(
        opens_lock(&st, b, 0, &card_lock(1), from),
        Some(How::Keycard)
    );
    assert_eq!(opens_lock(&st, b, 0, &card_lock(3), from), None);
    // Another building's card opens nothing here.
    let other = BuildingId(b.0 + 1);
    assert_eq!(opens_lock(&st, other, 0, &card_lock(1), from), None);
    // Each zone above public has its card in the catalogue, once.
    let zones = plan(&st, b).venue.as_ref().unwrap().zones.clone();
    let building = crate::net::building_name(&st, b);
    stock_zone_cards(&mut st, b);
    for (z, zone) in zones.iter().enumerate() {
        let item = st.content.item(&zone_card_id(b, z as u8));
        match z {
            0 => assert!(item.is_none(), "public has no card"),
            _ => assert_eq!(item.unwrap().name, format!("Keycard · {building}, {zone}")),
        }
    }
    let ours = |st: &AppState| {
        (st.content.items.iter())
            .filter(|i| parse_zone_card(&i.id).is_some_and(|(x, _)| x == b))
            .count()
    };
    assert_eq!(ours(&st), zones.len() - 1);
    stock_zone_cards(&mut st, b);
    assert_eq!(ours(&st), zones.len() - 1, "never twice");
}

#[test]
fn a_card_lock_stays_shut_and_says_whose() {
    let (mut st, b) = first_venue();
    let (level, lock) = lock_where(&mut st, b, |l| l.kind == LockKind::Card);
    let place = Place::Floor { building: b, level };
    let from = outside_of(floor_of(&st, b, level), &lock);
    let zone = zone_name(plan(&st, b), lock.zone);
    step_onto_from(&mut st, place, from, lock.at);
    assert_eq!(at(&st), (place, from), "the player stays put");
    assert_eq!(last(&st), Some(format!("locked · {zone} door").as_str()));
    assert!(warned(&st));
    // Its zone's card opens it, and the log says so.
    st.character.give(zone_card_id(b, lock.zone), 1);
    step_onto_from(&mut st, place, from, lock.at);
    assert_eq!(at(&st), (place, lock.at));
    assert_eq!(
        last(&st),
        Some(format!("{zone} door · the keycard opens it").as_str())
    );
}

#[test]
fn a_keypad_lock_takes_its_code_and_the_keyjack() {
    let (mut st, b) = first_venue();
    let (level, lock) = lock_where(&mut st, b, |l| l.kind == LockKind::Keypad);
    let place = Place::Floor { building: b, level };
    let from = outside_of(floor_of(&st, b, level), &lock);
    let zone = zone_name(plan(&st, b), lock.zone);
    let pad = crate::screens::keypad::Keypad::open(&st, b, level, lock.number);
    assert_eq!(pad.label(), format!("{zone} door"));
    // Shut, it says it's a keypad.
    step_onto_from(&mut st, place, from, lock.at);
    assert_eq!(at(&st).1, from);
    assert_eq!(
        last(&st),
        Some(format!("locked · {zone} door, keypad").as_str())
    );
    let seed = st.city.as_ref().unwrap().seed;
    let code = citygen::rng::door_code(seed, b, level, lock.number);
    assert_eq!(
        try_code(&mut st, b, level, lock.number, code),
        Verdict::Open
    );
    assert_eq!(
        last(&st),
        Some(format!("{zone} door · the keypad reads OPEN").as_str())
    );
    assert_eq!(
        opens_lock(&st, b, level, &lock, from),
        Some(How::Code(code))
    );
    let name = crate::net::building_name(&st, b);
    let journal: Vec<String> = (crate::credentials::entries(&st).into_iter())
        .flat_map(|(_, _, es)| es)
        .map(|e| e.text)
        .collect();
    assert!(
        (journal.iter()).any(|t| t.contains(&format!("`{name}`, {zone} door · keyed in"))),
        "{journal:?}"
    );
    step_onto_from(&mut st, place, from, lock.at);
    assert_eq!(at(&st).1, lock.at);
    assert_eq!(
        last(&st),
        Some(format!("{zone} door · you key in {code:04}").as_str())
    );
    // The Keyjack's crack learns the code as well.
    st.journal = Default::default();
    assert_eq!(opens_lock(&st, b, level, &lock, from), None);
    assert_eq!(
        open_with(&mut st, b, level, lock.number, Source::Keyjack),
        Verdict::Open
    );
    assert_eq!(
        opens_lock(&st, b, level, &lock, from),
        Some(How::Code(code))
    );
}

#[test]
fn a_venue_room_reads_its_label() {
    let (st, b) = first_venue();
    let v = plan(&st, b).venue.clone().unwrap();
    let (below, number) = (v.levels.iter())
        .filter(|x| x.level < 0)
        .find_map(|x| Some((x.level, x.rooms.iter().find(|r| r.number > 0)?.number)))
        .expect("a numbered room below the street");
    let label = crate::credentials::door_label(&st, b, below, number);
    assert_eq!(label, room_label(number));
    assert!(label.starts_with(&format!("B{}-", -below)), "{label}");
    assert!(!label.contains("flat"), "{label}");
    let (above, number) = (v.levels.iter())
        .filter(|x| x.level > 0)
        .find_map(|x| Some((x.level, x.rooms.iter().find(|r| r.number > 0)?.number)))
        .expect("a numbered room above the street");
    assert_eq!(
        crate::credentials::door_label(&st, b, above, number),
        number.to_string()
    );
    // A flat's number still reads as a flat.
    let mut st = st;
    let flats = st.home.unwrap().home.building;
    crate::credentials::prepare(&mut st, flats);
    let (level, number) = st
        .interiors
        .homes_of(flats)
        .unwrap()
        .flats()
        .next()
        .unwrap();
    assert_eq!(
        crate::credentials::door_label(&st, flats, level, number),
        crate::credentials::flat_label(&st, flats, level, number)
    );
}

#[test]
fn a_one_way_door_opens_from_its_free_side() {
    let (mut st, b) = first_venue();
    let (level, lock) = lock_where(&mut st, b, |l| l.free_from.is_some());
    let free = lock.free_from.unwrap();
    let other = outside_of(floor_of(&st, b, level), &lock);
    assert_eq!(opens_lock(&st, b, level, &lock, free), Some(How::Free));
    assert_eq!(opens_lock(&st, b, level, &lock, other), None);
    // On the map: through from its free side, saying nothing; shut from the other.
    let place = Place::Floor { building: b, level };
    step_onto_from(&mut st, place, other, lock.at);
    assert_eq!(at(&st).1, other);
    assert!(warned(&st));
    let said = st.log.iter().count();
    step_onto_from(&mut st, place, free, lock.at);
    assert_eq!(at(&st).1, lock.at);
    assert_eq!(st.log.iter().count(), said, "nothing to say");
}

#[test]
fn a_net_crack_opens_only_locks_the_net_reaches() {
    let (mut st, b) = first_venue();
    let (level, lock) = lock_where(&mut st, b, |l| l.ways.has(Ways::NET));
    let from = outside_of(floor_of(&st, b, level), &lock);
    let node = (st.city.as_ref().unwrap().net.nodes.iter())
        .find(|n| n.building == Some(b) && n.kind == citygen::NodeKind::DoorLock)
        .map(|n| n.id)
        .expect("a door lock on the building's Net");
    let now = st.net.now(&st);
    st.net.land(
        b,
        game::HackTarget::Node(node),
        game::Target::Node(citygen::NodeKind::DoorLock),
        game::Op::Crack,
        now,
    );
    assert!(st.net.unlocked(b, now));
    assert_eq!(opens_lock(&st, b, level, &lock, from), Some(How::Cracked));
    let no_net = DoorLock {
        ways: Ways::CARD,
        ..card_lock(1)
    };
    assert_eq!(opens_lock(&st, b, level, &no_net, from), None);
    // The crack opens a carded lift stop too.
    assert_eq!(
        crate::credentials::opens_stop(&st, b, 3),
        Some(How::Cracked)
    );
}

#[test]
fn the_lift_asks_for_its_card_at_a_carded_stop() {
    let (mut st, b) = first_venue();
    let mut p = plan(&st, b).clone();
    let lift = (p.links.iter())
        .position(|l| {
            l.kind == LinkKind::Lift && l.stops.len() >= 2 && l.stops.iter().all(|s| s.card == 0)
        })
        .expect("a lift that asks for no card");
    p.links[lift].stops[1].card = 3;
    let (from, to) = (p.links[lift].stops[0], p.links[lift].stops[1]);
    let zone = zone_name(&p, 3);
    st.interiors.put_plan(p);
    let city = st.city.clone().unwrap();
    st.interiors.ensure(&city, b, from.level).unwrap();
    let floor = Place::Floor {
        building: b,
        level: from.level,
    };
    put(&mut st, floor, from.at);
    assert_eq!(
        crate::interiors::ride(&mut st, b, to.level),
        Err(format!("the lift wants a {zone} card"))
    );
    assert_eq!(at(&st), (floor, from.at));
    // The panel says so before you press.
    let rows = crate::interact::ride_rows(&st, &mut crate::interact::FREE_LETTERS.chars()).unwrap();
    let row = (rows.iter())
        .find(|c| matches!(c.pick, Pick::World(WorldAct::Lift(l)) if l == to.level))
        .unwrap();
    assert!(!row.enabled);
    assert!(row.tail.contains(&format!("{zone} card")), "{}", row.tail);
    st.character.give(zone_card_id(b, 3), 1);
    assert!(crate::interiors::ride(&mut st, b, to.level).is_ok());
    assert_eq!(
        at(&st),
        (
            Place::Floor {
                building: b,
                level: to.level
            },
            to.at
        )
    );
}

#[test]
fn a_grille_wants_tech() {
    let (mut st, b) = first_venue();
    let p = plan(&st, b).clone();
    let crawl = (p.links.iter())
        .position(|l| l.kind == LinkKind::Crawl)
        .expect("a crawl");
    let stop = p.links[crawl].stops[0];
    let up = p.next_stop(crawl, stop.level, true).expect("a stop above");
    let city = st.city.clone().unwrap();
    st.interiors.ensure(&city, b, stop.level).unwrap();
    let place = Place::Floor {
        building: b,
        level: stop.level,
    };
    let beside = (stop.at.neighbors4())
        .find(|n| {
            floor_of(&st, b, stop.level).tiles[*n].is_walkable() && !grille_at(&p, stop.level, *n)
        })
        .unwrap();
    st.character.attributes.tech = crawl_tech() - 1;
    assert!(!can_crawl(&st));
    step_onto_from(&mut st, place, beside, stop.at);
    assert_eq!(at(&st).1, beside, "refused");
    assert_eq!(last(&st), Some(screwed_shut().as_str()));
    assert!(screwed_shut().starts_with("a grille, screwed shut"));
    // Space beside it names the Tech.
    let subjects = crate::interact::gather(&st, &|_, _| None);
    let grille = subjects
        .iter()
        .find(|s| s.pos == stop.at)
        .expect("the grille");
    assert!(
        (grille.choices.iter())
            .any(|c| !c.enabled && c.tail.contains(&format!("Tech {}", crawl_tech())))
    );
    // Stood on it somehow, the climb refuses too.
    put(&mut st, place, stop.at);
    assert_eq!(crate::interiors::climb(&mut st, true), Err(screwed_shut()));
    st.character.attributes.tech = crawl_tech();
    assert!(can_crawl(&st));
    step_onto_from(&mut st, place, beside, stop.at);
    assert_eq!(at(&st).1, stop.at, "through");
    assert!(crate::interiors::climb(&mut st, true).is_ok());
    let next = p.links[crawl].stop(up).unwrap().at;
    assert_eq!(
        at(&st),
        (
            Place::Floor {
                building: b,
                level: up
            },
            next
        )
    );
}

#[test]
fn crawling_in_from_the_roof_lands_inside() {
    let mut st = state();
    let city = st.city.clone().unwrap();
    let (b, way) = (city.buildings.iter())
        .find_map(|x| {
            let plan = FloorPlan::new(&city, x.id)?;
            let way = (plan.crawls.iter()).position(|c| c.stratum == StratumKind::Heights)?;
            Some((x.id, way))
        })
        .expect("a venue with a crawl in from the roof");
    let w = FloorPlan::new(&city, b).unwrap().crawls[way];
    let roof = Place::Stratum(StratumKind::Heights);
    put(&mut st, roof, w.outside);
    st.character.attributes.tech = crawl_tech();
    assert_eq!(
        crawl_in_at(&st, StratumKind::Heights, w.outside),
        Some((b, way))
    );
    let here = crate::interact::gather(&st, &|_, _| None);
    let row = (here.iter())
        .filter(|s| s.dir.is_none())
        .flat_map(|s| s.choices.iter())
        .find(|c| c.thing.as_ref().is_some_and(|(t, _)| t == "Crawl in"))
        .expect("a Crawl in row on the roof tile");
    assert!(row.enabled);
    assert!(matches!(
        row.pick,
        Pick::World(WorldAct::Crawl(Some((x, i)))) if x == b && i == way
    ));
    assert!(crate::interiors::crawl_in(&mut st, b, way).is_ok());
    let inside = Place::Floor {
        building: b,
        level: w.level,
    };
    assert_eq!(at(&st), (inside, w.at));
    // Inside, the grille offers the way back out.
    let here = crate::interact::gather(&st, &|_, _| None);
    assert!(
        (here.iter())
            .filter(|s| s.dir.is_none())
            .flat_map(|s| s.choices.iter())
            .any(|c| c.enabled
                && c.thing.as_ref().is_some_and(|(t, _)| t == "Crawl out")
                && matches!(c.pick, Pick::World(WorldAct::Crawl(None))))
    );
    assert!(crate::interiors::crawl_out(&mut st).is_ok());
    assert_eq!(at(&st), (roof, w.outside));
    // Without the Tech, the grille stays shut.
    st.character.attributes.tech = crawl_tech() - 1;
    assert_eq!(
        crate::interiors::crawl_in(&mut st, b, way),
        Err(screwed_shut())
    );
    assert_eq!(at(&st), (roof, w.outside));
}

#[test]
fn staff_carry_their_zones_card() {
    let mut st = state();
    let city = st.city.clone().unwrap();
    let people = sim::People::load().unwrap();
    let mut w = sim::World::seed(&city, &people, &sim::SimParams::default(), 22 * 60, 0);
    // On to ten in the morning.
    for m in 1..=12 * 60 {
        w.advance(&city, m * sim::TICKS_PER_MINUTE);
        w.path_some(&city, usize::MAX);
    }
    st.world = Some(w);
    let world = st.world.as_ref().unwrap();
    let mut staff = None;
    let mut public = None;
    for b in city.buildings.iter().filter(|b| b.kind.is_closed()) {
        let Some(plan) = crate::interiors::plan_now(&st, b.id) else {
            continue;
        };
        if plan.venue.is_none() {
            continue;
        }
        for o in world.inside(b.id) {
            if o.role != sim::OccupantRole::Working {
                continue;
            }
            let Some((Place::Floor { building, level }, pos)) =
                crate::interiors::drawn_at(&st, o.id)
            else {
                continue;
            };
            let floor = crate::interiors::floor_now(&st, &plan, level).unwrap();
            let zone = floor.room_at(pos).unwrap().zone;
            if zone > 0 {
                staff.get_or_insert((o.id, building, zone));
            } else {
                public.get_or_insert(o.id);
            }
        }
        if staff.is_some() && public.is_some() {
            break;
        }
    }
    let (id, b, zone) = staff.expect("someone at work past a locked door at ten");
    assert!(crate::credentials::carries(&st, id).contains(&zone_card_id(b, zone)));
    let id = public.expect("someone at work in public at ten");
    assert!(
        (crate::credentials::carries(&st, id).iter()).all(|c| parse_zone_card(c).is_none()),
        "nobody in public carries a zone card"
    );
}

#[test]
fn zone_cards_survive_a_reopen_and_a_trip() {
    use crate::outside::tests::{beside_another, home_run};
    let mut st = home_run();
    let city = st.city.clone().unwrap();
    let b = (city.buildings.iter())
        .map(|x| x.id)
        .find(|x| {
            (st.interiors.plan(&city, *x)).is_some_and(|p| {
                p.venue.as_ref().is_some_and(|v| {
                    (v.levels.iter()).any(|l| {
                        l.doors
                            .iter()
                            .any(|d| d.lock.is_some_and(|k| k.kind == LockKind::Card))
                    })
                })
            })
        })
        .expect("a venue with a card lock");
    crate::credentials::prepare(&mut st, b);
    let (level, lock) = lock_where(&mut st, b, |l| l.kind == LockKind::Card);
    let from = outside_of(floor_of(&st, b, level), &lock);
    st.character.give(zone_card_id(b, lock.zone), 1);
    assert_eq!(opens_lock(&st, b, level, &lock, from), Some(How::Keycard));
    // Saved and opened again.
    let dir = crate::runs::testutil::Dir::new("zone-cards");
    let st = crate::runs::testutil::running(st);
    let path = crate::runs::file::write(&dir.0, &st).unwrap();
    let mut back = AppState::new();
    crate::runs::open(&mut back, &path).unwrap();
    let again = lock_numbered(plan(&back, b).venue.as_ref().unwrap(), level, lock.number).unwrap();
    assert_eq!(again, lock);
    assert_eq!(opens_lock(&back, b, level, &lock, from), Some(How::Keycard));
    assert!(back.content.item(&zone_card_id(b, lock.zone)).is_some());
    // Into another city, where it opens nothing, and back.
    let mut st = st;
    let (s, _) = beside_another(&st);
    crate::outside::enter(&mut st, s).unwrap();
    assert_eq!(crate::credentials::zone_card_held(&st, b), None);
    let home = st.outside.as_ref().unwrap().home();
    crate::outside::enter(&mut st, home).unwrap();
    assert_eq!(opens_lock(&st, b, level, &lock, from), Some(How::Keycard));
    assert!(st.content.item(&zone_card_id(b, lock.zone)).is_some());
}

#[test]
fn a_walk_across_a_level_stops_at_a_shut_door() {
    // Whether the room of `to` is among those reached from `from`.
    let walks = |st: &AppState, p: &FloorPlan, level: i32, from: Pos, to: Pos| {
        let rooms = reached(st, p, level, from).unwrap();
        room_of(p, level, to).is_some_and(|r| rooms[r])
    };
    let (mut st, b) = first_venue();
    let (level, lock) = lock_where(&mut st, b, |l| l.kind == LockKind::Card);
    let p = plan(&st, b).clone();
    let floor = floor_of(&st, b, level).clone();
    // The two sides of the door: shut from its locked side, and out free from the other.
    let (a, z) = (outside_of(&floor, &lock), lock.free_from.unwrap());
    assert!(!walks(&st, &p, level, a, z));
    assert!(walks(&st, &p, level, z, a), "getting out is free");
    st.character.give(zone_card_id(b, lock.zone), 1);
    assert!(walks(&st, &p, level, a, z));
    assert!(walks(&st, &p, level, z, a));
    // A plan with no layout walks everywhere.
    let classic = FloorPlan::classic(st.city.as_ref().unwrap(), b).unwrap();
    assert!(reached(&st, &classic, level, a).is_none());
}

#[test]
fn a_lock_door_and_a_grille_say_what_they_are() {
    use crate::render::map::MapRef;
    let (mut st, b) = first_venue();
    let (level, lock) = lock_where(&mut st, b, |l| l.kind == LockKind::Card);
    let place = Place::Floor { building: b, level };
    let from = outside_of(floor_of(&st, b, level), &lock);
    put(&mut st, place, from);
    let look = |st: &mut AppState, pos: Pos| {
        let city = st.city.clone().unwrap();
        let minute = st.minute_of_day();
        let map = MapRef::for_place(&city, &st.interiors, &st.constructs, place, minute).unwrap();
        st.player.as_mut().unwrap().update_fov(&map, 0);
        crate::look::look_card(st, &map, place, pos, &|_, _| None, &[])
    };
    let zone = zone_name(plan(&st, b), lock.zone);
    let card = look(&mut st, lock.at);
    assert_eq!(
        card.header,
        Some(crate::ui::hud::capitalised(&format!("{zone} door")))
    );
    assert_eq!(card.second.as_deref(), Some("card · locked"));
    assert_eq!(card.body.len(), 1, "a line of its own");
    st.character.give(zone_card_id(b, lock.zone), 1);
    assert_eq!(
        look(&mut st, lock.at).second.as_deref(),
        Some("card · unlocked")
    );
    // A crawl's grille, wherever one stands on a level of the building.
    let p = plan(&st, b).clone();
    let crawl = (p.links.iter())
        .position(|l| l.kind == LinkKind::Crawl)
        .unwrap();
    let stop = p.links[crawl].stops[0];
    let city = st.city.clone().unwrap();
    st.interiors.ensure(&city, b, stop.level).unwrap();
    let beside = (stop.at.neighbors4())
        .find(|n| floor_of(&st, b, stop.level).tiles[*n].is_walkable())
        .unwrap();
    let place = Place::Floor {
        building: b,
        level: stop.level,
    };
    put(&mut st, place, beside);
    let minute = st.minute_of_day();
    let map = MapRef::for_place(&city, &st.interiors, &st.constructs, place, minute).unwrap();
    st.player.as_mut().unwrap().update_fov(&map, 0);
    let card = crate::look::look_card(&st, &map, place, stop.at, &|_, _| None, &[]);
    assert_eq!(card.header.as_deref(), Some("Grille"));
    assert_eq!(card.second, Some(format!("a crawl, Tech {}", crawl_tech())));
    assert_eq!(card.body.len(), 1, "a line of its own");
}

/// The first building of the first venue type's kind with a locked way in from the roof,
/// planned: the building, the roof's city-map door, and the lock's level and lock.
fn locked_roof_door(st: &mut AppState) -> (BuildingId, Pos, i32, DoorLock) {
    let city = st.city.clone().unwrap();
    let kind = VenuePack::builtin().types[0].builds[0];
    (city.buildings.iter())
        .filter(|x| x.kind == kind)
        .find_map(|x| {
            let v = st.interiors.plan(&city, x.id)?.venue.as_deref()?;
            v.ways_in.iter().find_map(|w| match w.kind {
                WayInKind::Door {
                    stratum: StratumKind::Heights,
                    door,
                } => Some((x.id, door, w.level, w.lock?)),
                _ => None,
            })
        })
        .expect("a locked way in from the roof")
}

#[test]
fn a_locked_way_in_holds_from_the_street_side_only() {
    let mut st = state();
    let (b, door, level, lock) = locked_roof_door(&mut st);
    let zone = zone_name(plan(&st, b), lock.zone);
    let roof = Place::Stratum(StratumKind::Heights);
    put(&mut st, roof, door);
    assert_eq!(
        crate::interiors::enter(&mut st, b, (StratumKind::Heights, door)),
        Err(format!("locked · {zone} door, keypad"))
    );
    assert_eq!(at(&st), (roof, door), "still on the roof");
    let card = zone_card_id(b, lock.zone);
    st.character.give(card.clone(), 1);
    assert_eq!(
        crate::interiors::enter(&mut st, b, (StratumKind::Heights, door)),
        Ok(format!("{zone} door · the keycard opens it"))
    );
    let inside = Place::Floor { building: b, level };
    assert_eq!(at(&st), (inside, lock.free_from.unwrap()));
    // Out through it from inside needs nothing.
    st.character.inventory.retain(|s| s.item != card);
    step_onto_from(&mut st, inside, lock.free_from.unwrap(), lock.at);
    assert_eq!(at(&st).0, roof);
    assert!(!warned(&st));
}

#[test]
fn the_track_takes_no_door_in_that_stays_shut() {
    use crate::track::{Target, route};
    let mut st = state();
    let (b, door, _, lock) = locked_roof_door(&mut st);
    let city = st.city.clone().unwrap();
    let roof = Place::Stratum(StratumKind::Heights);
    let beside = (door.neighbors4())
        .find(|n| {
            city.walkable(StratumKind::Heights, *n)
                && city.tile(StratumKind::Heights, *n).unwrap().terrain != citygen::Terrain::Door
        })
        .expect("roof beside the door");
    put(&mut st, roof, beside);
    let target = Target::Building(b);
    assert!(
        route::leg(&st, target).map_or(true, |l| l.goal != door),
        "not to the shut door"
    );
    st.character.give(zone_card_id(b, lock.zone), 1);
    let leg = route::leg(&st, target).unwrap();
    assert_eq!((leg.goal, leg.through), (door, route::Through::Door(b)));
}

#[test]
fn standing_on_a_crawls_tile_lays_its_venue_out_once() {
    let mut st = state();
    let city = st.city.clone().unwrap();
    let (b, w) = (city.buildings.iter())
        .find_map(|x| {
            let plan = FloorPlan::new(&city, x.id)?;
            let w = *plan.crawls.first()?;
            Some((x.id, w))
        })
        .expect("a venue with a crawl in");
    assert!(st.interiors.plan_of(b).is_none(), "not planned yet");
    put(&mut st, Place::Stratum(w.stratum), w.outside);
    let mut s = MapScreen::walking(&mut st);
    s.update(&mut st, &[]);
    let first: *const FloorPlan = st.interiors.plan_of(b).expect("kept on the first frame");
    s.update(&mut st, &[]);
    assert!(
        std::ptr::eq(first, st.interiors.plan_of(b).unwrap()),
        "borrowed on the next"
    );
    assert_eq!(crawl_in_at(&st, w.stratum, w.outside), Some((b, 0)));
}

/// A locked door in from the city off the roof, a step from where you'd stand outside it,
/// whose type's border into its zone is a `border` lock: the building, the stratum, the
/// city-map door, the tile outside it, and the lock's level and lock.
fn street_side_door(
    st: &mut AppState,
    border: LockKind,
) -> (BuildingId, StratumKind, Pos, Pos, i32, DoorLock) {
    let city = st.city.clone().unwrap();
    (city.buildings.iter())
        .find_map(|x| {
            let t = VenuePack::builtin().for_kind(x.kind)?;
            let v = st.interiors.plan(&city, x.id)?.venue.clone()?;
            v.ways_in.iter().find_map(|w| {
                let WayInKind::Door { stratum, door } = w.kind else {
                    return None;
                };
                let lock = w.lock?;
                let b = (0..lock.zone).rev().find_map(|z| t.border(z, lock.zone))?;
                let exit = v.level(w.level)?.exits.iter().find(|e| e.door == door)?;
                let beside = exit.landing.neighbors4().any(|n| n == door);
                (stratum != StratumKind::Heights && b.lock == border && beside).then_some((
                    x.id,
                    stratum,
                    door,
                    exit.landing,
                    w.level,
                    lock,
                ))
            })
        })
        .unwrap_or_else(|| panic!("a locked door in behind a {border:?} border"))
}

#[test]
fn a_locked_way_in_takes_a_code_and_the_keyjack_from_the_city() {
    for border in [LockKind::Card, LockKind::Keypad] {
        let mut st = state();
        let (b, k, door, outside, level, lock) = street_side_door(&mut st, border);
        let what = format!("{border:?} border, {b:?} {k:?} {door:?}");
        assert_eq!(lock.kind, LockKind::Keypad, "{what}");
        assert!(
            lock.ways.has(Ways::CODE) && lock.ways.has(Ways::KEYJACK),
            "{what}"
        );
        let zone = zone_name(plan(&st, b), lock.zone);
        let city_side = Place::Stratum(k);
        put(&mut st, city_side, outside);
        // Space beside it offers its pad.
        let subjects = crate::interact::gather(&st, &|_, _| None);
        let pad = (subjects.iter())
            .filter(|s| s.pos == door)
            .flat_map(|s| s.choices.iter())
            .find(|c| {
                matches!(c.pick, Pick::Keypad { building, level: l, number }
                if building == b && l == level && number == lock.number)
            });
        assert!(pad.is_some_and(|c| c.enabled), "{what}: no keypad");
        assert_eq!(
            crate::interiors::enter(&mut st, b, (k, door)),
            Err(format!("locked · {zone} door, keypad")),
            "{what}"
        );
        // Its code, keyed in there.
        let seed = st.city.as_ref().unwrap().seed;
        let code = citygen::rng::door_code(seed, b, level, lock.number);
        assert_eq!(
            try_code(&mut st, b, level, lock.number, code),
            Verdict::Open
        );
        assert_eq!(
            crate::interiors::enter(&mut st, b, (k, door)),
            Ok(format!("{zone} door · you key in {code:04}")),
            "{what}"
        );
        assert_eq!(at(&st).0, Place::Floor { building: b, level }, "{what}");
        // Back out, the code forgotten: the Keyjack cracks it.
        put(&mut st, city_side, outside);
        st.journal = Default::default();
        assert!(
            crate::interiors::enter(&mut st, b, (k, door)).is_err(),
            "{what}"
        );
        assert_eq!(
            open_with(&mut st, b, level, lock.number, Source::Keyjack),
            Verdict::Open
        );
        assert!(
            crate::interiors::enter(&mut st, b, (k, door)).is_ok(),
            "{what}"
        );
    }
}

#[test]
fn a_card_readers_door_reads_as_one_on_the_map() {
    use crate::render::map::{MapRef, TileMap};
    let (mut st, b) = first_venue();
    let (level, card) = lock_where(&mut st, b, |l| l.kind == LockKind::Card);
    let place = Place::Floor { building: b, level };
    let map = MapRef::for_place(
        st.city.as_ref().unwrap(),
        &st.interiors,
        &st.constructs,
        place,
        0,
    )
    .unwrap();
    assert!(map.reader(card.at));
    let floor = floor_of(&st, b, level);
    let plain = (floor.tiles.iter())
        .find(|(p, t)| t.terrain == Terrain::Door && floor.lock_at(*p).is_none())
        .map(|(p, _)| p)
        .expect("a door with no lock");
    assert!(!map.reader(plain));
    let pad = floor.locks.iter().find(|l| l.kind == LockKind::Keypad);
    assert!(
        pad.is_none_or(|k| !map.reader(k.at)),
        "a keypad is no reader"
    );
}

#[test]
fn guards_are_inert_foes_at_their_posts() {
    let (st, b) = first_venue();
    let plan = st.interiors.plan_of(b).unwrap();
    let layout = plan.venue.as_deref().unwrap();
    let pack = VenuePack::builtin();
    let t = pack.by_id(&layout.type_id).unwrap();
    let layer = (pack.sizes.iter())
        .flat_map(|rule| {
            (0..t.objectives.len()).map(move |objective| {
                let inputs = MissionInputs {
                    size: rule.size,
                    objective,
                    security: 128,
                    seed: 7,
                };
                MissionLayer::lay(layout, t, rule, inputs)
            })
        })
        .find_map(Result::ok)
        .expect("the venue holds a mission");
    let foes = guard_foes(&layer, b);
    assert!(!layer.guards.is_empty());
    assert_eq!(foes.len(), layer.guards.len());
    for (f, g) in foes.iter().zip(&layer.guards) {
        assert_eq!((f.name.as_str(), f.glyph), ("guard", '@'));
        assert!(f.inert && !f.down && !f.hunting(), "{f:?}");
        let place = Place::Floor {
            building: b,
            level: g.level,
        };
        assert_eq!((f.place, f.pos), (place, g.at));
        let post = crate::foes::Post {
            building: b,
            zone: g.zone,
            patrol: g.patrol.clone(),
        };
        assert_eq!(f.post, Some(post));
    }
}
