//! Getting out is free: a venue's zone locks open from their deeper side, a private room
//! from inside, and a lift asks for a card only where you get off, so nobody whose crack
//! lapses is shut in.

use super::*;
use crate::track::Target;
use crate::track::route::{self, Through};
use citygen::venue::{NodeAt, VenueGraph};

/// Walks the track to `target` from where you stand, a step at a time on the map, riding
/// the lift and climbing the stairs each leg ends at, for at most `legs` legs.
fn walk_track(st: &mut AppState, target: Target, legs: usize) {
    for _ in 0..legs {
        let leg = route::leg(st, target)
            .unwrap_or_else(|e| panic!("{e:?} from {:?}: {:?}", at(st), last(st)));
        for &p in &leg.path {
            let (place, here) = at(st);
            step_onto_from(st, place, here, p);
            if at(st).0 != place {
                // Through a door in or out.
                break;
            }
            assert_eq!(at(st).1, p, "held at {here:?} on {place:?}: {:?}", last(st));
        }
        let b = match at(st).0 {
            Place::Floor { building, .. } => Some(building),
            _ => None,
        };
        match leg.through {
            Through::Arrive => return,
            Through::Lift(l) => {
                crate::interiors::ride(st, b.unwrap(), l).unwrap();
            }
            Through::Stairs { up, .. } => {
                crate::interiors::climb(st, up).unwrap();
            }
            Through::Door(_) | Through::Exit => {}
            other => panic!("{other:?} at {:?}", at(st)),
        }
    }
    panic!("not there after {legs} legs: at {:?}", at(st));
}

/// The far rooms of venue `b` the Net's crack reaches on foot: of each zone's, the most
/// locks in from the city on its highest level and on its lowest; each by its level and a
/// walkable tile in it.
fn far_rooms(st: &mut AppState, b: BuildingId) -> Vec<(i32, Pos)> {
    let v = plan(st, b).venue.clone().unwrap();
    let mut g = VenueGraph::of(&v, &[], None);
    // On foot: no crawl.
    g.edges
        .retain(|e| e.lock.is_none_or(|l| l.ways != Ways::CRAWL));
    let reached = g.reach(&g.starts, Ways::NET, Default::default()).nodes;
    // (zone, depth, level, room) of every room reached.
    let rooms: Vec<(u8, u32, i32, usize)> = (0..g.nodes.len())
        .filter(|&n| reached[n])
        .filter_map(|n| match g.nodes[n] {
            NodeAt::Room { level, room } => {
                let zone = v.level(level)?.rooms[room].zone;
                Some((zone, g.depth(n)?, level, room))
            }
            NodeAt::Outside(_) => None,
        })
        .collect();
    let mut far: Vec<(i32, usize)> = Vec::new();
    for zone in 0..v.zones.len() as u8 {
        let of = || rooms.iter().filter(|r| r.0 == zone);
        let high = of().max_by_key(|r| (r.2, r.1, std::cmp::Reverse(r.3)));
        let low = of().max_by_key(|r| (-r.2, r.1, std::cmp::Reverse(r.3)));
        for &(_, _, level, room) in high.into_iter().chain(low) {
            if !far.contains(&(level, room)) {
                far.push((level, room));
            }
        }
    }
    let city = st.city.clone().unwrap();
    (far.into_iter())
        .map(|(level, room)| {
            let rect = v.level(level).unwrap().rooms[room].rect;
            let floor = st.interiors.ensure(&city, b, level).unwrap();
            let at = (rect.iter())
                .find(|p| {
                    let t = &floor.tiles[*p];
                    t.is_walkable() && t.feature.is_none() && !t.is_connector()
                })
                .expect("a free tile in the room");
            (level, at)
        })
        .collect()
}

/// The Net's crack on building `b`'s door locks, landed now.
fn crack(st: &mut AppState, b: BuildingId) {
    let node = (st.city.as_ref().unwrap().net.nodes.iter())
        .find(|n| n.building == Some(b) && n.kind == citygen::NodeKind::DoorLock)
        .map(|n| n.id)
        .expect("a door lock on the building's Net");
    let now = st.net.now(st);
    st.net.land(
        b,
        game::HackTarget::Node(node),
        game::Target::Node(citygen::NodeKind::DoorLock),
        game::Op::Crack,
        now,
    );
    assert!(st.net.unlocked(b, now));
}

#[test]
fn a_cracked_in_runner_walks_out_when_the_crack_lapses() {
    let mut st = state();
    // A runner the track walks across a crawl's grille, which no leg climbs.
    st.character.attributes.tech = crawl_tech();
    let kinds: Vec<BuildingKind> = (VenuePack::builtin().types.iter())
        .filter_map(|t| t.builds.first().copied())
        .collect();
    let mut walked = 0;
    for kind in kinds {
        let b = venue_of(&mut st, kind);
        let v = plan(&st, b).venue.clone().unwrap();
        let street = (v.frame.doors.iter())
            .find(|d| d.stratum == StratumKind::Street)
            .unwrap();
        let outside = Place::Stratum(StratumKind::Street);
        let landing = Target::Spot {
            place: outside,
            pos: street.landing,
            radius: 0,
        };
        for (level, far) in far_rooms(&mut st, b) {
            put(&mut st, outside, street.landing);
            // In on the crack, every lock on the way opening to it.
            crack(&mut st, b);
            let inside = Place::Floor { building: b, level };
            let room = Target::Spot {
                place: inside,
                pos: far,
                radius: 0,
            };
            walk_track(&mut st, room, 12);
            assert_eq!(at(&st), (inside, far));
            // The crack lapses with you inside, holding no card and no code.
            let now = st.net.now(&st);
            for h in st.net.hacks.iter_mut().filter(|h| h.building == b) {
                h.until = Some(now);
            }
            assert!(!st.net.unlocked(b, now));
            assert_eq!(crate::credentials::zone_card_held(&st, b), None);
            // And you walk out, back onto the street.
            walk_track(&mut st, landing, 12);
            assert_eq!(
                at(&st),
                (outside, street.landing),
                "out of {far:?} on {level}"
            );
            walked += 1;
        }
    }
    assert!(walked >= 8, "{walked} rooms walked out of");
}

#[test]
fn a_lift_asks_for_no_card_where_you_get_off_without_one() {
    let (mut st, b) = first_venue();
    let mut p = plan(&st, b).clone();
    let lift = (p.links.iter())
        .position(|l| {
            l.kind == LinkKind::Lift && l.stops.len() >= 2 && l.stops.iter().all(|s| s.card == 0)
        })
        .expect("a lift that asks for no card");
    p.links[lift].stops[1].card = 3;
    let (carded, free) = (p.links[lift].stops[1], p.links[lift].stops[0]);
    st.interiors.put_plan(p);
    let city = st.city.clone().unwrap();
    st.interiors.ensure(&city, b, carded.level).unwrap();
    let on = Place::Floor {
        building: b,
        level: carded.level,
    };
    put(&mut st, on, carded.at);
    // On at the carded stop, off at one that asks none: no card.
    assert!(crate::interiors::ride(&mut st, b, free.level).is_ok());
    let off = Place::Floor {
        building: b,
        level: free.level,
    };
    assert_eq!(at(&st), (off, free.at));
    // The panel offers the ride back only with the card.
    let rows = crate::interact::ride_rows(&st, &mut crate::interact::FREE_LETTERS.chars()).unwrap();
    let row = (rows.iter())
        .find(|c| matches!(c.pick, Pick::World(WorldAct::Lift(l)) if l == carded.level))
        .unwrap();
    assert!(!row.enabled);
}

#[test]
fn a_private_room_opens_from_inside_with_no_key() {
    let (mut st, b) = first_venue();
    let city = st.city.clone().unwrap();
    let levels: Vec<i32> = plan(&st, b).levels().collect();
    let (level, room) = levels
        .into_iter()
        .find_map(|level| {
            let floor = st.interiors.ensure(&city, b, level)?;
            let r = floor.rooms.iter().find(|r| r.private && r.door.is_some())?;
            Some((level, r.clone()))
        })
        .expect("a private room");
    let door = room.door.unwrap();
    let inside = (door.neighbors4())
        .find(|n| room.rect.contains(*n))
        .unwrap();
    let place = Place::Floor { building: b, level };
    step_onto_from(&mut st, place, inside, door);
    assert_eq!(at(&st), (place, door), "{:?}", last(&st));
    assert!(!warned(&st));
}
