//! A gate line in a building of the city: its turnstiles take a code and the Keyjack as
//! well as a card, and who works behind it isn't met without a key. A station's paid
//! gate opens to a crack on its line's switch until an alarm locks the building down.

use super::*;
use citygen::venue::{MissionSize, PieceKind};

/// The first building of a kind whose type lays a gate line, its plan kept: the
/// building, the line's level and the lock on one of its turnstiles.
fn gate_line(st: &mut AppState) -> (BuildingId, i32, DoorLock) {
    let kind = (VenuePack::builtin().types.iter())
        .filter(|t| t.pieces.iter().any(|p| p.kind == PieceKind::GateLine))
        .find_map(|t| t.builds.first().copied())
        .expect("a building kind with a gate line");
    let b = venue_of(st, kind);
    let v = plan(st, b).venue.clone().unwrap();
    let (level, lock) = (v.levels.iter())
        .find_map(|x| {
            let line = x.pieces.iter().find(|p| p.kind == PieceKind::GateLine)?;
            let lock = (x.doors.iter())
                .filter(|d| line.rect.contains(d.at))
                .find_map(|d| d.lock)?;
            Some((x.level, lock))
        })
        .expect("a gate line locked in the plain building");
    let city = st.city.clone().unwrap();
    st.interiors.ensure(&city, b, level).unwrap();
    (b, level, lock)
}

#[test]
fn the_keyjack_alone_gets_past_a_gate_line() {
    use crate::screens::keypad::Keypad;
    let mut st = state();
    let (b, level, lock) = gate_line(&mut st);
    assert_eq!(lock.kind, LockKind::Keypad);
    for w in [Ways::CARD, Ways::CODE, Ways::KEYJACK] {
        assert!(lock.ways.has(w), "{w:?}");
    }
    let place = Place::Floor { building: b, level };
    let from = outside_of(floor_of(&st, b, level), &lock);
    // No card, no code, no crack: only the Keyjack in the pack.
    assert_eq!(crate::credentials::zone_card_held(&st, b), None);
    assert!(matches!(
        crate::keyjack::carried(&st),
        crate::keyjack::Carried::Fused(_)
    ));
    step_onto_from(&mut st, place, from, lock.at);
    assert_eq!(at(&st), (place, from), "shut");
    // Its keypad, beside it.
    let subjects = crate::interact::gather(&st, &|_, _| None);
    let turnstile = (subjects.iter())
        .find(|s| s.pos == lock.at)
        .expect("the pad");
    assert!((turnstile.choices.iter()).any(|c| {
        c.enabled
            && matches!(c.pick, Pick::Keypad { building, level: l, number }
                if (building, l, number) == (b, level, lock.number))
    }));
    // The Keyjack clipped on and run, every beat a hit.
    let mut pad = Keypad::open(&st, b, level, lock.number);
    pad.update(&mut st, &[key(KeyCode::Char('k'))]);
    for _ in 0..4 {
        pad.aim(&mut st, true);
        pad.update(&mut st, &[key(KeyCode::Char(' '))]);
    }
    assert_eq!(pad.clip_ended(), Some(true), "cracked");
    step_onto_from(&mut st, place, from, lock.at);
    assert_eq!(at(&st), (place, lock.at), "through: {:?}", last(&st));
    // Back out from behind it needs nothing.
    st.journal = Default::default();
    let behind = lock.free_from.unwrap();
    step_onto_from(&mut st, place, behind, lock.at);
    assert_eq!(at(&st), (place, lock.at), "out is free");
}

/// A station's paid gate as a DEVELOPER start lays it, plain or a large mission's copy:
/// the building, the gate's floor, a tile in front of it and its lock.
fn station_gate(st: &mut AppState, mission: bool) -> (BuildingId, Place, Pos, DoorLock) {
    let size = if mission {
        MissionSize::Large
    } else {
        MissionSize::Small
    };
    let b = crate::venues::dev_start(st, "metro", size, mission).unwrap();
    let v = plan(st, b).venue.clone().unwrap();
    let (level, lock) = (v.levels.iter())
        .find_map(|x| {
            let line = x.pieces.iter().find(|p| p.kind == PieceKind::GateLine)?;
            let lock = (x.doors.iter())
                .filter(|d| line.rect.contains(d.at))
                .find_map(|d| d.lock)?;
            Some((x.level, lock))
        })
        .expect("a locked gate line");
    let city = st.city.clone().unwrap();
    st.interiors
        .ensure(&city, b, level)
        .expect("the gate's floor");
    let from = outside_of(floor_of(st, b, level), &lock);
    (b, Place::Floor { building: b, level }, from, lock)
}

#[test]
fn a_stations_paid_gate_opens_to_a_crack_until_an_alarm_locks_it_down() {
    let mut st = state();
    for mission in [false, true] {
        let (b, place, from, lock) = station_gate(&mut st, mission);
        step_onto_from(&mut st, place, from, lock.at);
        assert_eq!(at(&st), (place, from), "shut before the crack");
        assert_eq!(last(&st), Some("locked · paid door"));
        // A venue is a site: the crack lands on the line's switch, which works the line.
        let Place::Floor { level, .. } = place else {
            unreachable!()
        };
        let v = plan(&st, b).venue.clone().unwrap();
        let line = v.gate_line_at(level, lock.at).expect("the gate's line");
        let switch = (citygen::venue::line_switches(&v, level).into_iter())
            .find(|s| s.piece == line)
            .expect("the line's switch");
        let node = citygen::SiteNode {
            building: b,
            level,
            at: switch.at,
        };
        let now = st.net.now(&st);
        st.net.land(
            b,
            game::HackTarget::Site(node),
            game::Target::Node(citygen::NodeKind::GateSwitch),
            game::Op::Crack,
            now,
        );
        step_onto_from(&mut st, place, from, lock.at);
        assert_eq!(
            at(&st),
            (place, lock.at),
            "mission {mission}: {:?}",
            last(&st)
        );
        // An alarm locks the building down, crack or none, and the gate says so.
        st.net.trace(b).alert_until = Some(now + 30);
        step_onto_from(&mut st, place, from, lock.at);
        assert_eq!(at(&st), (place, from), "shut in the alarm");
        assert_eq!(last(&st), Some("locked down · paid door"));
        // The paid side's card still opens it.
        st.character.give(zone_card_id(b, lock.zone), 1);
        step_onto_from(&mut st, place, from, lock.at);
        assert_eq!(at(&st), (place, lock.at), "{:?}", last(&st));
    }
}

#[test]
fn who_works_behind_a_gate_line_is_not_met_without_a_key() {
    let mut st = state();
    let (b, level, _) = gate_line(&mut st);
    let p = plan(&st, b).clone();
    let met = floors_met_without_a_key(&p).unwrap();
    let work = p.work_floors();
    assert!(
        met.contains(&level),
        "the line's own floor, in front of it: {work:?}"
    );
    assert!(work.iter().any(|l| !met.contains(l)), "{work:?}, all met");
    // A plan with no layout meets everyone.
    let classic = FloorPlan::classic(st.city.as_ref().unwrap(), b).unwrap();
    assert_eq!(floors_met_without_a_key(&classic), None);
}
