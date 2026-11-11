use super::*;
use crate::credentials::{How, Verdict, opens_lock, try_code, zone_card_id};
use crate::look::{Group, Looks, look_card};
use crate::render::map::MapRef;
use crate::venues::{begin, end, guard_foes};
use crate::walker::Player;
use citygen::venue::{
    CodeSpot, LockKind, MissionLayer, MissionSize, VenueFrame, draw_level, lay_out,
};
use citygen::{BuildingKind, City, DoorLock, Feature, GenParams, Generator, LinkKind, SizePreset};

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

const STREET: Place = Place::Stratum(StratumKind::Street);

fn inputs(size: MissionSize) -> MissionInputs {
    MissionInputs {
        size,
        objective: 0,
        security: 128,
        seed: 7,
    }
}

/// The layer a mission of `inputs` lays on building `b` as the session plans it.
fn layer(st: &AppState, b: BuildingId, inputs: MissionInputs) -> Option<MissionLayer> {
    let layout = st.interiors.plan_of(b)?.venue.as_deref()?;
    let pack = VenuePack::builtin();
    let t = pack.by_id(&layout.type_id)?;
    MissionLayer::lay(layout, t, pack.size(inputs.size), inputs).ok()
}

fn put(st: &mut AppState, place: Place, pos: Pos) {
    let p = st.player.as_mut().unwrap();
    p.place = place;
    p.pos = pos;
}

fn floor(b: BuildingId, level: i32) -> Place {
    Place::Floor { building: b, level }
}

/// Building `b`'s first street door on the city map (its main one, which takes no lock)
/// and the tile outside it.
fn street_door(st: &AppState, b: BuildingId) -> (Pos, Pos) {
    let v = st.interiors.plan_of(b).unwrap().venue.as_deref().unwrap();
    let d = (v.frame.doors.iter())
        .find(|d| d.stratum == StratumKind::Street)
        .unwrap();
    (d.door, d.landing)
}

/// You on the street outside building `b`'s main door.
fn outside(st: &mut AppState, b: BuildingId) {
    let (_, landing) = street_door(st, b);
    put(st, STREET, landing);
}

/// In through building `b`'s main door, onto its doorstep.
fn walk_in(st: &mut AppState, b: BuildingId) {
    outside(st, b);
    let (door, _) = street_door(st, b);
    crate::interiors::enter(st, b, (StratumKind::Street, door)).unwrap();
}

/// The home city walked into and its first hotel whose venue holds a small job that `pick`
/// takes, you on the street outside its main door.
fn hotel_where(
    pick: impl Fn(&AppState, BuildingId, &MissionLayer) -> bool,
) -> (AppState, BuildingId) {
    let mut st = AppState::new();
    st.enter_city(home_city());
    let city = st.city.clone().unwrap();
    st.player = Some(Player::spawn(&city, 1));
    let b = (city.buildings.iter())
        .filter(|x| x.kind == BuildingKind::Hotel)
        .map(|x| x.id)
        .find(|&b| {
            let venue = (st.interiors.plan(&city, b)).is_some_and(|p| p.venue.is_some());
            venue && layer(&st, b, inputs(MissionSize::Small)).is_some_and(|l| pick(&st, b, &l))
        })
        .expect("a hotel that holds a small job");
    outside(&mut st, b);
    (st, b)
}

fn hotel() -> (AppState, BuildingId) {
    hotel_where(|_, _, _| true)
}

/// Building `b`'s kept levels' floors as the session has drawn them.
fn drawn(st: &AppState, b: BuildingId) -> Vec<(i32, Floor)> {
    let kept = st.interiors.copy(b).unwrap().kept.clone();
    (kept.into_iter())
        .filter_map(|l| Some((l, st.interiors.floor(b, l)?.1.clone())))
        .collect()
}

#[test]
fn begin_flags_every_door_and_bars_the_dropped_levels() {
    let (mut st, b) = hotel_where(|st, b, l| {
        let v = st.interiors.plan_of(b).unwrap().venue.as_deref().unwrap();
        v.frame.doors.iter().any(|d| !l.kept.contains(&d.level))
    });
    assert!(door_marks(&st).iter().all(|m| m.2 != DoorMark::Flagged));
    begin(&mut st, b, inputs(MissionSize::Small)).unwrap();
    let copy = st.interiors.copy(b).unwrap().clone();
    assert!(copy.live && !copy.ending, "{copy:?}");
    let city = st.city.clone().unwrap();
    let marks = door_marks(&st);
    let plan = st.interiors.plan_of(b).unwrap().clone();
    let mut barred = None;
    for (k, p) in city.portals(b) {
        let level = plan.door_level(k).unwrap();
        let want = if copy.kept.contains(&level) {
            DoorMark::Flagged
        } else {
            barred = Some((k, p));
            DoorMark::Barred
        };
        assert!(marks.contains(&(k, p, want)), "{k:?} {p:?} on {level}");
        assert_eq!(door_mark_at(&st, k, p), Some((b, want)));
    }
    let (k, p) = barred.expect("a door onto a level the copy drops");
    let was = (
        st.player.as_ref().unwrap().place,
        st.player.as_ref().unwrap().pos,
    );
    assert_eq!(
        crate::interiors::enter(&mut st, b, (k, p)),
        Err("barred".to_string())
    );
    let you = st.player.as_ref().unwrap();
    assert_eq!((you.place, you.pos), was);
    assert!(
        crate::venues::door_shut(&st, b, k, p),
        "no way in for a walk"
    );
    assert!(crate::venues::locked_door_in(&st, k, p).is_none(), "no pad");
    // It says it's shut for the job, to someone outside it.
    let v = plan.venue.as_deref().unwrap();
    let d = (v.frame.doors.iter()).find(|d| d.stratum == k && d.door == p);
    let mut you = Player::at(Place::Stratum(k), d.unwrap().landing);
    you.update_fov(&MapRef::stratum_of(&city, k), 0);
    st.player = Some(you);
    let map = MapRef::stratum_of(&city, k);
    let card = look_card(&st, &map, Place::Stratum(k), p, &|_, _| None, &[]);
    assert_eq!(card.header.as_deref(), Some("Barred door"), "{card:?}");
    assert_eq!(card.second.as_deref(), Some("shut for the job"));
    let line = Looks::load().line(Group::Fixture, "barred_door", 0, Pos::new(0, 0));
    assert_eq!(card.body, vec![line.unwrap().to_string()]);
    // The flagged door you stand by says what it is.
    let (door, landing) = street_door(&st, b);
    let mut you = Player::at(STREET, landing);
    you.update_fov(&MapRef::stratum_of(&city, StratumKind::Street), 0);
    st.player = Some(you);
    let map = MapRef::stratum_of(&city, StratumKind::Street);
    let card = look_card(&st, &map, STREET, door, &|_, _| None, &[]);
    assert_eq!(card.header.as_deref(), Some("Flagged door"), "{card:?}");
    let kind = &plan.venue.as_deref().unwrap().type_name;
    assert_eq!(card.second, Some(format!("{kind} · a job inside")));
    let line = Looks::load().line(Group::Fixture, "flagged_door", 0, Pos::new(0, 0));
    assert_eq!(card.body, vec![line.unwrap().to_string()]);
}

#[test]
fn the_copy_keeps_only_its_floors() {
    let (mut st, b) = hotel();
    let plain = st.interiors.plan_of(b).unwrap().clone();
    begin(&mut st, b, inputs(MissionSize::Small)).unwrap();
    let kept = st.interiors.copy(b).unwrap().kept.clone();
    let plan = st.interiors.plan_of(b).unwrap().clone();
    assert_eq!(plan.levels, kept);
    for (i, link) in plan.links.iter().enumerate() {
        for &level in &kept {
            let rides = plan.rides(i, level);
            assert!(
                rides.iter().all(|r| kept.contains(r)),
                "{link:?}: {rides:?}"
            );
        }
    }
    // A climb from a kept level lands on the next kept level its link serves.
    let city = st.city.clone().unwrap();
    let mut climbed = 0;
    for (i, link) in plan.links.iter().enumerate() {
        if matches!(link.kind, LinkKind::Lift | LinkKind::Crawl) {
            continue;
        }
        let was = (plain.links.iter())
            .find(|l| l.kind == link.kind && l.name == link.name)
            .unwrap();
        for s in link.stops.iter().filter(|s| s.on) {
            let Some(next) = plan.next_stop(i, s.level, true) else {
                continue;
            };
            let want = (was.stops.iter())
                .filter(|w| w.level > s.level && w.off && kept.contains(&w.level))
                .map(|w| w.level)
                .min();
            assert_eq!(Some(next), want, "{link:?} from {}", s.level);
            st.interiors.ensure(&city, b, s.level).unwrap();
            put(&mut st, floor(b, s.level), s.at);
            crate::interiors::climb(&mut st, true).unwrap();
            assert_eq!(st.player.as_ref().unwrap().place, floor(b, next));
            climbed += 1;
        }
    }
    assert!(climbed > 0, "no stair climbs between the kept floors");
    for level in plain.levels().filter(|l| !kept.contains(l)) {
        assert!(st.interiors.ensure(&city, b, level).is_none(), "{level}");
    }
}

#[test]
fn the_copy_draws_its_layer() {
    let (mut st, b) = hotel();
    assert!(st.foes.is_empty());
    begin(&mut st, b, inputs(MissionSize::Small)).unwrap();
    let layer = layer(&st, b, inputs(MissionSize::Small)).unwrap();
    assert!(!layer.devices.is_empty());
    let city = st.city.clone().unwrap();
    for &level in &layer.kept {
        let f = st.interiors.ensure(&city, b, level).unwrap();
        for d in layer.devices.iter().filter(|d| d.level == level) {
            let want = match d.kind {
                citygen::venue::DeviceKind::Camera => citygen::Feature::Camera,
                citygen::venue::DeviceKind::Turret => citygen::Feature::Turret,
            };
            assert_eq!(f.tiles[d.at].feature, Some(want), "{level} {d:?}");
        }
        for c in layer.containers.iter().filter(|c| c.level == level) {
            assert_eq!(f.tiles[c.at].feature, Some(citygen::Feature::Container));
        }
        if layer.objective.level == level {
            let at = layer.objective.at;
            assert_eq!(f.tiles[at].feature, Some(citygen::Feature::Objective));
        }
        let v = st.interiors.plan_of(b).unwrap().venue.clone().unwrap();
        for &(_, k) in layer.fitted.iter().filter(|f| f.0 == level) {
            let fit = v.level(level).unwrap().doors[k].fit.unwrap();
            let f = st.interiors.floor(b, level).unwrap().1;
            assert!(f.locks.contains(&fit), "{level}: door {k}");
        }
    }
    assert_eq!(st.foes, guard_foes(&layer, b));
    let copy = st.interiors.copy(b).unwrap();
    assert_eq!(copy.left.len(), layer.cards.len());
    for c in &layer.cards {
        let id = zone_card_id(b, c.zone);
        let lying = st.ground.at(floor(b, c.level), c.at);
        assert!(lying.iter().any(|s| s.item == id), "{c:?}");
    }
    let zones = st
        .interiors
        .plan_of(b)
        .unwrap()
        .venue
        .as_ref()
        .unwrap()
        .zones
        .len();
    for z in 1..zones {
        assert!(
            st.content.item(&zone_card_id(b, z as u8)).is_some(),
            "zone {z}"
        );
    }
}

#[test]
fn a_mission_waits_while_you_are_inside() {
    let (mut st, b) = hotel();
    walk_in(&mut st, b);
    let city = st.city.clone().unwrap();
    st.interiors.ensure(&city, b, 0).unwrap();
    let before = st.interiors.floor(b, 0).unwrap().1.clone();
    let plan = st.interiors.plan_of(b).unwrap().clone();
    assert!(begin(&mut st, b, inputs(MissionSize::Small)).is_ok());
    let waiting = |st: &AppState| st.interiors.copy(b).is_some_and(|c| !c.live);
    assert!(waiting(&st));
    assert!(door_marks(&st).iter().all(|m| m.2 != DoorMark::Flagged));
    assert_eq!(st.interiors.floor(b, 0).unwrap().1, &before);
    assert_eq!(st.interiors.plan_of(b), Some(&plan));
    assert!(st.foes.is_empty());
    tick(&mut st);
    assert!(waiting(&st), "still inside");
    outside(&mut st, b);
    tick(&mut st);
    assert!(copy_live(&st, b));
    assert!(door_marks(&st).iter().any(|m| m.2 == DoorMark::Flagged));
}

#[test]
fn leave_and_come_back_and_it_is_as_you_left_it() {
    let (mut st, b) = hotel_where(|_, _, l| !l.cards.is_empty());
    begin(&mut st, b, inputs(MissionSize::Small)).unwrap();
    let card = layer(&st, b, inputs(MissionSize::Small)).unwrap().cards[0];
    let city = st.city.clone().unwrap();
    st.interiors.ensure(&city, b, card.level).unwrap();
    let place = floor(b, card.level);
    let id = zone_card_id(b, card.zone);
    put(&mut st, place, card.at);
    assert_eq!(st.ground.take(place, card.at, &id, 1), 1);
    st.character.give(id.clone(), 1);
    let coil = game::ItemId::new("copper_coil");
    st.ground.add(place, card.at, coil.clone(), 1);
    outside(&mut st, b);
    let foes = st.foes.clone();
    let floors = drawn(&st, b);
    assert!(!floors.is_empty());
    for _ in 0..24 {
        crate::clock::run(
            &mut st,
            Some(60 * sim::TICKS_PER_MINUTE),
            crate::clock::PATHS_PER_FRAME,
        );
    }
    tick(&mut st);
    walk_in(&mut st, b);
    assert_eq!(st.character.count(&id), 1, "the card in the pack");
    assert_eq!(
        st.ground.at(place, card.at)[0].item,
        coil,
        "the coil where it lay"
    );
    assert_eq!(st.foes, foes);
    for (level, f) in floors {
        assert_eq!(st.interiors.floor(b, level).unwrap().1, &f, "{level}");
    }
    assert!(copy_live(&st, b));
}

/// An always-locked keypad door on a level the copy keeps: its level and lock.
fn kept_keypad(st: &AppState, b: BuildingId, l: &MissionLayer) -> Option<(i32, DoorLock)> {
    let v = st.interiors.plan_of(b)?.venue.as_deref()?;
    (l.kept.iter()).find_map(|&level| {
        let x = v.level(level)?;
        let lock = (x.doors.iter())
            .filter_map(|d| d.lock)
            .find(|k| k.kind == LockKind::Keypad)?;
        Some((level, lock))
    })
}

#[test]
fn ending_brings_the_plain_building_back() {
    let (mut st, b) = hotel_where(|st, b, l| kept_keypad(st, b, l).is_some());
    let layout = (st.interiors.plan_of(b).unwrap().venue.clone()).unwrap();
    begin(&mut st, b, inputs(MissionSize::Small)).unwrap();
    let layer = layer(&st, b, inputs(MissionSize::Small)).unwrap();
    let (level, lock) = kept_keypad(&st, b, &layer).unwrap();
    let city = st.city.clone().unwrap();
    st.interiors.ensure(&city, b, level).unwrap();
    // The code keyed into the pad in the copy.
    let place = floor(b, level);
    let f = st.interiors.floor(b, level).unwrap().1;
    let from = (lock.at.neighbors4())
        .find(|n| Some(*n) != lock.free_from && f.tiles.get(*n).is_some_and(|t| t.is_walkable()))
        .unwrap();
    assert_eq!(opens_lock(&st, b, level, &lock, from), None, "locked");
    put(&mut st, place, from);
    let code = citygen::rng::door_code(city.seed, b, level, lock.number);
    assert_eq!(
        try_code(&mut st, b, level, lock.number, code),
        Verdict::Open
    );
    // A coil dropped on the floor there.
    let coil = game::ItemId::new("copper_coil");
    st.ground.add(place, from, coil.clone(), 1);
    let left = st.interiors.copy(b).unwrap().left.clone();
    outside(&mut st, b);
    end(&mut st, b).unwrap();
    assert!(st.interiors.copy(b).is_none());
    assert!(
        (st.foes.iter()).all(|f| f.post.as_ref().is_none_or(|p| p.building != b)),
        "a guard stayed"
    );
    for (place, at, id) in &left {
        assert!(st.ground.at(*place, *at).iter().all(|s| &s.item != id));
    }
    assert!(st.ground.at(place, from).iter().any(|s| s.item == coil));
    let levels: Vec<i32> = layout.levels.iter().map(|x| x.level).collect();
    assert_eq!(st.interiors.plan_of(b).unwrap().levels, levels);
    for &l in &layer.kept {
        let plain = draw_level(&layout, l).unwrap();
        assert_eq!(st.interiors.ensure(&city, b, l), Some(&plain), "{l}");
    }
    let marks = door_marks(&st);
    assert!(
        (city.portals(b).iter()).all(|(k, p)| !marks.contains(&(*k, *p, DoorMark::Flagged))),
        "{marks:?}"
    );
    // The code learned in the copy opens the plain building's door.
    assert_eq!(
        opens_lock(&st, b, level, &lock, from),
        Some(How::Code(code))
    );
}

#[test]
fn a_copy_whose_type_is_gone_ends_and_says_so() {
    let (mut st, b) = hotel();
    begin(&mut st, b, inputs(MissionSize::Small)).unwrap();
    let left = st.interiors.copy(b).unwrap().left.clone();
    walk_in(&mut st, b);
    // A save from a build whose data had a type this one lacks.
    let mut plan = st.interiors.plan_of(b).unwrap().clone();
    let v = plan.venue.as_mut().unwrap();
    v.type_id = "no longer in the data".to_string();
    let levels: Vec<i32> = v.levels.iter().map(|x| x.level).collect();
    st.interiors.put_plan(plan);
    tick(&mut st);
    assert!(st.interiors.copy(b).is_none(), "ended at once, you inside");
    assert!(st.foes.is_empty());
    for (place, at, id) in &left {
        assert!(st.ground.at(*place, *at).iter().all(|s| &s.item != id));
    }
    assert_eq!(st.interiors.plan_of(b).unwrap().levels, levels);
    // The floor you stand on is drawn again, plain.
    let here = st.player.as_ref().unwrap().place;
    let Place::Floor { level, .. } = here else {
        panic!("inside: {here:?}");
    };
    assert!(st.interiors.floor(b, level).is_some());
    let name = crate::net::building_name(&st, b);
    let line = format!("lost in the update · the job in {name}");
    assert!(st.log.iter().any(|e| e.text == line), "{line}");
}

#[test]
fn ending_waits_until_you_are_out() {
    let (mut st, b) = hotel();
    begin(&mut st, b, inputs(MissionSize::Small)).unwrap();
    walk_in(&mut st, b);
    end(&mut st, b).unwrap();
    assert!(copy_live(&st, b), "still live while you're in");
    assert!(st.interiors.copy(b).unwrap().ending);
    tick(&mut st);
    assert!(copy_live(&st, b));
    outside(&mut st, b);
    tick(&mut st);
    assert!(st.interiors.copy(b).is_none());
    assert!(st.foes.is_empty());
}

#[test]
fn one_mission_per_site() {
    let (mut st, b) = hotel();
    begin(&mut st, b, inputs(MissionSize::Small)).unwrap();
    let copy = st.interiors.copy(b).cloned();
    assert_eq!(
        begin(&mut st, b, inputs(MissionSize::Small)),
        Err("a job is already on there".to_string())
    );
    assert_eq!(st.interiors.copy(b).cloned(), copy);
    // And a building no venue type lays out holds none.
    let city = st.city.clone().unwrap();
    let home = citygen::home(&city).unwrap().building;
    assert!(begin(&mut st, home, inputs(MissionSize::Small)).is_err());
    assert!(st.interiors.copy(home).is_none());
}

#[test]
fn the_real_building_carries_on_unseen() {
    // Ten in the morning, at work.
    let mut st = AppState::new();
    st.enter_city(home_city());
    let city = st.city.clone().unwrap();
    st.player = Some(Player::spawn(&city, 1));
    let ahead = (10 * 60 + 1440 - st.minute_of_day() as u64) % 1440;
    let w = st.world.as_mut().unwrap();
    let w_tick = w.tick() + ahead * sim::TICKS_PER_MINUTE;
    w.place_at(&city, w_tick);
    // Someone at work in a venue, on a floor its small copy keeps.
    let (b, id) = (city.buildings.iter())
        .filter(|x| VenuePack::builtin().for_kind(x.kind).is_some())
        .find_map(|x| {
            st.interiors.plan(&city, x.id)?.venue.as_ref()?;
            let kept = layer(&st, x.id, inputs(MissionSize::Small))?.kept;
            let world = st.world.as_ref()?;
            let id = (world.inside(x.id).iter())
                .filter(|o| o.role == sim::OccupantRole::Working)
                .map(|o| o.id)
                .find(|&id| {
                    matches!(crate::interiors::drawn_at(&st, id),
                        Some((Place::Floor { level, .. }, _)) if kept.contains(&level))
                })?;
            Some((x.id, id))
        })
        .expect("someone at work on a floor a copy keeps");
    outside(&mut st, b);
    begin(&mut st, b, inputs(MissionSize::Small)).unwrap();
    assert!(copy_live(&st, b));
    assert_eq!(crate::interiors::drawn_at(&st, id), None);
    end(&mut st, b).unwrap();
    assert!(crate::interiors::drawn_at(&st, id).is_some());
}

#[test]
fn a_copy_comes_back_whole() {
    use crate::runs::testutil::{Dir, round, running, same};
    let (mut st, b) = hotel();
    begin(&mut st, b, inputs(MissionSize::Small)).unwrap();
    walk_in(&mut st, b);
    let kept = st.interiors.copy(b).unwrap().kept.clone();
    assert!(
        matches!(st.player.as_ref().unwrap().place, Place::Floor { level, .. } if kept.contains(&level))
    );
    let back = round(&st.interiors);
    assert!(same(&st.interiors, &back));
    assert_eq!(back.copy(b), st.interiors.copy(b));
    // Saved and opened again.
    let dir = Dir::new("mission-copy");
    let st = running(st);
    let path = crate::runs::file::write(&dir.0, &st).unwrap();
    let mut back = AppState::new();
    crate::runs::open(&mut back, &path).unwrap();
    assert!(same(&st.interiors, &back.interiors));
    assert!(same(&st.foes, &back.foes));
    assert!(same(&st.ground, &back.ground));
    assert!(copy_live(&back, b));
    assert_eq!(back.interiors.plan_of(b).unwrap().levels, kept);
    assert!(door_marks(&back).iter().any(|m| m.2 == DoorMark::Flagged));
    // Nothing more to take or end: the next tick leaves it as it was.
    let mut back = back;
    tick(&mut back);
    assert!(same(&st.interiors, &back.interiors));
}

/// Each kept level of building `b`'s copy drawn afresh: its locks, the cameras, turrets,
/// containers and objective standing on it, and its code notes as read.
type Drawn = Vec<(i32, Vec<DoorLock>, Vec<(Pos, Feature)>, Vec<Option<String>>)>;

fn drawn_afresh(st: &mut AppState, b: BuildingId) -> Drawn {
    let city = st.city.clone().unwrap();
    st.interiors.drop_floors(b);
    let copy = st.interiors.copy(b).unwrap().clone();
    let layer = copy.layer.unwrap();
    (copy.kept.iter())
        .map(|&level| {
            let f = st.interiors.ensure(&city, b, level).unwrap().clone();
            let marks = (f.tiles.iter())
                .filter_map(|(p, t)| match t.feature {
                    Some(
                        k @ (Feature::Camera
                        | Feature::Turret
                        | Feature::Container
                        | Feature::Objective),
                    ) => Some((p, k)),
                    _ => None,
                })
                .collect();
            let notes = (layer.codes.iter())
                .filter(|c| c.level == level)
                .map(|c| note_at(st, floor(b, level), c.at))
                .collect();
            (level, f.locks, marks, notes)
        })
        .collect()
}

#[test]
fn a_copy_keeps_its_layer_through_a_save_and_a_change_to_its_type() {
    use crate::runs::testutil::{Dir, running};
    let (mut st, b) = hotel_where(|_, _, l| !l.codes.is_empty() && !l.devices.is_empty());
    let was = inputs(MissionSize::Small);
    begin(&mut st, b, was).unwrap();
    let laid = st.interiors.copy(b).unwrap().layer.clone().unwrap();
    assert_eq!(
        Some(&laid),
        layer(&st, b, was).as_ref(),
        "laid once, at begin"
    );
    let before = drawn_afresh(&mut st, b);
    assert!(before.iter().any(|(_, locks, marks, notes)| {
        !locks.is_empty() && !marks.is_empty() && notes.iter().all(Option::is_some)
    }));
    // Saved and opened again.
    let dir = Dir::new("copy-layer");
    let st = running(st);
    let path = crate::runs::file::write(&dir.0, &st).unwrap();
    let mut back = AppState::new();
    crate::runs::open(&mut back, &path).unwrap();
    assert_eq!(back.interiors.copy(b).unwrap().layer.as_ref(), Some(&laid));
    // A build that lays this copy otherwise, as a change to its type's security would:
    // here its inputs' security moves, which lays other guards and devices.
    let now = MissionInputs {
        security: was.security.wrapping_add(127),
        ..was
    };
    back.interiors.copy_mut(b).unwrap().inputs = now;
    assert_ne!(
        layer(&back, b, now).as_ref(),
        Some(&laid),
        "laid again, it differs"
    );
    // Drawn again, its floors are as they were.
    assert_eq!(drawn_afresh(&mut back, b), before);
}

#[test]
fn a_copy_saved_with_no_layer_is_lost() {
    let (mut st, b) = hotel();
    begin(&mut st, b, inputs(MissionSize::Small)).unwrap();
    let left = st.interiors.copy(b).unwrap().left.clone();
    // As a save from before copies kept their layer reads.
    st.interiors.copy_mut(b).unwrap().layer = None;
    tick(&mut st);
    assert!(st.interiors.copy(b).is_none(), "ended");
    assert!(st.foes.is_empty());
    for (place, at, id) in &left {
        assert!(st.ground.at(*place, *at).iter().all(|s| &s.item != id));
    }
    let name = crate::net::building_name(&st, b);
    let line = format!("lost in the update · the job in {name}");
    assert!(st.log.iter().any(|e| e.text == line), "{line}");
}

/// The first venue of `st`'s city whose plan holds a small job, laid out for the session.
fn any_venue(st: &mut AppState) -> BuildingId {
    let city = st.city.clone().unwrap();
    (city.buildings.iter())
        .filter(|x| VenuePack::builtin().for_kind(x.kind).is_some())
        .map(|x| x.id)
        .find(|&b| {
            (st.interiors.plan(&city, b)).is_some_and(|p| p.venue.is_some())
                && layer(st, b, inputs(MissionSize::Small)).is_some()
        })
        .expect("a venue that holds a small job")
}

#[test]
fn a_copy_travels_with_its_city() {
    use crate::outside::tests::{beside_another, home_run, reopen};
    let mut st = home_run();
    let b = any_venue(&mut st);
    outside(&mut st, b);
    begin(&mut st, b, inputs(MissionSize::Small)).unwrap();
    let (copy, foes) = (st.interiors.copy(b).cloned(), st.foes.clone());
    assert!(copy.as_ref().is_some_and(|c| c.live));
    let (s, _) = beside_another(&st);
    let home = st.outside.as_ref().unwrap().home();
    crate::outside::enter(&mut st, s).unwrap();
    assert!(st.interiors.copy(b).is_none(), "another city's interiors");
    crate::outside::enter(&mut st, home).unwrap();
    assert_eq!(st.interiors.copy(b).cloned(), copy);
    assert_eq!(st.foes, foes);
    // A copy in the other city, kept there as you go home, saved and opened again.
    crate::outside::enter(&mut st, s).unwrap();
    let there = any_venue(&mut st);
    outside(&mut st, there);
    begin(&mut st, there, inputs(MissionSize::Small)).unwrap();
    let copy = st.interiors.copy(there).cloned();
    crate::outside::enter(&mut st, home).unwrap();
    let mut back = reopen(st, "copy-kept");
    crate::outside::enter(&mut back, s).unwrap();
    assert_eq!(back.interiors.copy(there).cloned(), copy);
    assert!(copy_live(&back, there));
}

#[test]
fn a_track_to_a_dropped_floor_finds_no_way() {
    use crate::track::route::NoLeg;
    use crate::track::{Source, leg_now};
    let (mut st, b) = hotel_where(|st, b, l| {
        let plan = st.interiors.plan_of(b).unwrap();
        plan.levels().any(|level| !l.kept.contains(&level))
    });
    let kept = layer(&st, b, inputs(MissionSize::Small)).unwrap().kept;
    let v = st.interiors.plan_of(b).unwrap().venue.clone().unwrap();
    let x = (v.levels.iter())
        .find(|x| !kept.contains(&x.level))
        .unwrap();
    let spot = Source::Spot {
        place: floor(b, x.level),
        pos: *x.rooms.iter().find_map(|r| r.spots.first()).unwrap(),
        radius: 1,
    };
    crate::track::set(&mut st, spot).unwrap();
    crate::track::tick(&mut st);
    assert!(matches!(leg_now(&st), Some(Ok(_))));
    begin(&mut st, b, inputs(MissionSize::Small)).unwrap();
    crate::track::tick(&mut st);
    assert_eq!(leg_now(&st), Some(Err(NoLeg::NoWay)));
    end(&mut st, b).unwrap();
    tick(&mut st);
    crate::track::tick(&mut st);
    assert!(matches!(leg_now(&st), Some(Ok(_))), "{:?}", leg_now(&st));
}

#[test]
fn a_copy_seals_its_track_ends() {
    use citygen::venue::{PieceKind, building_seed};
    let mut st = AppState::new();
    st.enter_city(home_city());
    let city = st.city.clone().unwrap();
    st.player = Some(Player::spawn(&city, 1));
    // A type the data lays a track in.
    let t = (VenuePack::builtin().types.iter())
        .find(|t| {
            let l = lay_out(t, VenueFrame::own(t), 0).unwrap();
            (l.levels.iter()).any(|x| x.pieces.iter().any(|p| p.kind == PieceKind::Track))
        })
        .expect("a type with a track");
    // On the largest closed building but the home, raised to the type's own top, that
    // holds a large job: one with ways in enough.
    let home = citygen::home(&city).unwrap().building;
    let mut closed: Vec<&citygen::Building> = (city.buildings.iter())
        .filter(|x| x.kind.is_closed() && x.id != home)
        .collect();
    closed.sort_by_key(|x| (std::cmp::Reverse(x.footprint.w * x.footprint.h), x.id));
    let pack = VenuePack::builtin();
    let (b, layout) = (closed.into_iter())
        .find_map(|x| {
            let frame = VenueFrame::of_building(&city, x.id)?.raised(t.frame.top);
            let layout = lay_out(t, frame, building_seed(city.seed, x.id)).ok()?;
            let large = inputs(MissionSize::Large);
            MissionLayer::lay(&layout, t, pack.size(large.size), large).ok()?;
            Some((x.id, layout))
        })
        .expect("a building that holds a large job");
    st.interiors.put_plan(FloorPlan::of_layout(layout.clone()));
    let (level, shutter, on) = (layout.levels.iter())
        .find_map(|x| {
            let p = x.pieces.iter().find(|p| p.kind == PieceKind::Track)?;
            let end = p.ends[0];
            let on = p.rect.iter().find(|q| q.manhattan(end) == 1)?;
            Some((x.level, end, on))
        })
        .unwrap();
    let place = floor(b, level);
    let look = |st: &mut AppState| {
        st.interiors.ensure(&city, b, level).unwrap();
        let map = MapRef::for_place(&city, &st.interiors, &st.constructs, place, 0).unwrap();
        let mut you = Player::at(place, on);
        you.update_fov(&map, 0);
        st.player = Some(you);
        let map = MapRef::for_place(&city, &st.interiors, &st.constructs, place, 0).unwrap();
        look_card(st, &map, place, shutter, &|_, _| None, &[])
    };
    let line = |key| {
        let l = Looks::load().line(Group::Feature, key, 0, Pos::new(0, 0));
        vec![l.unwrap().to_string()]
    };
    let c = look(&mut st);
    assert_eq!(c.header.as_deref(), Some("Shutter"), "{c:?}");
    assert_eq!(c.body, line("shutter"));
    // The copy keeps every level.
    outside(&mut st, b);
    begin(&mut st, b, inputs(MissionSize::Large)).unwrap();
    assert!(copy_live(&st, b));
    assert!(st.interiors.copy(b).unwrap().kept.contains(&level));
    let c = look(&mut st);
    assert_eq!(c.header.as_deref(), Some("Shutter"), "{c:?}");
    assert_eq!(c.body, line("shutter_sealed"));
}

#[test]
fn a_code_note_reads_under_its_tile() {
    let (mut st, b) = hotel_where(|_, _, l| !l.codes.is_empty());
    begin(&mut st, b, inputs(MissionSize::Small)).unwrap();
    let layer = layer(&st, b, inputs(MissionSize::Small)).unwrap();
    let note = layer.codes[0];
    let place = floor(b, note.level);
    let (level, number) = note.lock;
    let city = st.city.clone().unwrap();
    let code = citygen::rng::door_code(city.seed, b, level, number);
    let door = crate::credentials::door_label(&st, b, level, number);
    let want = format!("a sticky note: {code:04} · {door}");
    assert_eq!(note_at(&st, place, note.at), Some(want.clone()));
    assert_eq!(note_at(&st, place, note.at.offset(0, -100)), None);
    // Looked at, under the tile's own lines.
    st.interiors.ensure(&city, b, note.level).unwrap();
    let map = MapRef::for_place(&city, &st.interiors, &st.constructs, place, 0).unwrap();
    let mut you = Player::at(place, note.at);
    you.update_fov(&map, 0);
    st.player = Some(you);
    let map = MapRef::for_place(&city, &st.interiors, &st.constructs, place, 0).unwrap();
    let c = look_card(&st, &map, place, note.at, &|_, _| None, &[]);
    assert_eq!(c.body.last(), Some(&want), "{c:?}");
    // Ended, the note's gone with the copy.
    outside(&mut st, b);
    end(&mut st, b).unwrap();
    assert_eq!(note_at(&st, place, note.at), None);
}

/// The layer's code note for a locked room's door, not a zone lock's.
fn room_note(st: &AppState, b: BuildingId, l: &MissionLayer) -> Option<CodeSpot> {
    let v = st.interiors.plan_of(b)?.venue.as_deref()?;
    l.codes.iter().copied().find(|c| {
        let (level, number) = c.lock;
        (v.level(level)).is_some_and(|x| x.rooms.iter().any(|r| r.private && r.number == number))
    })
}

#[test]
fn a_rooms_code_note_and_the_keyjack_open_its_door() {
    use crate::screens::popup::Pick;
    let (mut st, b) = hotel_where(|st, b, l| room_note(st, b, l).is_some());
    begin(&mut st, b, inputs(MissionSize::Small)).unwrap();
    let layer = layer(&st, b, inputs(MissionSize::Small)).unwrap();
    let (level, number) = room_note(&st, b, &layer).unwrap().lock;
    let city = st.city.clone().unwrap();
    st.interiors.ensure(&city, b, level).unwrap();
    let f = st.interiors.floor(b, level).unwrap().1.clone();
    let room = (f.rooms.iter())
        .find(|r| r.private && r.number == number)
        .unwrap()
        .clone();
    let door = room.door.unwrap();
    let from = (door.neighbors4())
        .find(|n| !room.rect.contains(*n) && f.tiles.get(*n).is_some_and(|t| t.is_walkable()))
        .unwrap();
    let place = floor(b, level);
    let step = |st: &mut AppState| {
        use crate::app::Screen;
        use crossterm::event::KeyCode;
        put(st, place, from);
        let k = match (door.x - from.x, door.y - from.y) {
            (1, 0) => 'l',
            (-1, 0) => 'h',
            (0, 1) => 'j',
            _ => 'k',
        };
        let mut s = crate::screens::explore::MapScreen::walking(st);
        s.update(st, &[crate::screens::testutil::key(KeyCode::Char(k))]);
        st.player.as_ref().unwrap().pos
    };
    // Beside its door, its keypad; the step holds.
    put(&mut st, place, from);
    let subjects = crate::interact::gather(&st, &|_, _| None);
    let pad = (subjects.iter()).find(|s| s.pos == door).expect("the door");
    let keypad = |p: &Pick| {
        matches!(*p, Pick::Keypad { building, level: l, number: n }
            if (building, l, n) == (b, level, number))
    };
    assert!(
        (pad.choices.iter()).any(|c| c.enabled && keypad(&c.pick)),
        "a keypad on the door"
    );
    assert_eq!(step(&mut st), from, "locked");
    // The note's code keyed in opens it.
    let code = citygen::rng::door_code(city.seed, b, level, number);
    assert_eq!(try_code(&mut st, b, level, number, code), Verdict::Open);
    assert_eq!(step(&mut st), door);
    // And the Keyjack's crack, the code forgotten.
    st.journal = Default::default();
    assert_eq!(step(&mut st), from, "locked again");
    let keyjack = crate::credentials::Source::Keyjack;
    let opened = crate::credentials::open_with(&mut st, b, level, number, keyjack);
    assert_eq!(opened, Verdict::Open);
    assert_eq!(step(&mut st), door);
}
