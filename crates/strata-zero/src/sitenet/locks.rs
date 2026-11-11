//! What a site's nodes open on its floors: each door, private room's keypad, lift and line
//! of turnstiles opens to a hack on its own node, never the whole building; a street
//! door's lock on the city's Net opens only its own door. An alarm shuts them all; a lift
//! answers in one to a spoof landed since. The spec's "What a node does":
//! `docs/superpowers/specs/2026-11-11-site-net-design.md`.

use crate::app::AppState;
use crate::net::Hack;
use citygen::venue::{WayInKind, Ways, switch_line};
use citygen::{BuildingId, DoorLock, LinkKind, NodeId, NodeKind, SiteNode};
use game::{HackTarget, Op, Target};

/// The ops that hold a door or a line of turnstiles open.
const OPENS: [Op; 2] = [Op::Crack, Op::Spoof];

/// A live Crack or Spoof (`ops`) on `node`.
pub fn hacked(st: &AppState, node: SiteNode, ops: &[Op]) -> bool {
    let now = st.net.now(st);
    (st.net.live(now)).any(|h| h.target == HackTarget::Site(node) && ops.contains(&h.op))
}

/// The live hacks of `ops` on a `kind` node of site `b`'s own Net.
fn site_hacks<'a>(
    st: &'a AppState,
    b: BuildingId,
    kind: NodeKind,
    ops: &'a [Op],
) -> impl Iterator<Item = (SiteNode, &'a Hack)> + 'a {
    let now = st.net.now(st);
    (st.net.live(now))
        .filter(move |h| h.kind == Target::Node(kind) && ops.contains(&h.op))
        .filter_map(move |h| match h.target {
            HackTarget::Site(s) if s.building == b => Some((s, h)),
            _ => None,
        })
}

/// Whether the Net holds `lock` open: a hack on its own node, on its line's switch for a
/// turnstile, or, for a way in's lock at a city-map door, on that door's city lock; never
/// in an alarm.
pub fn lock_open(st: &AppState, b: BuildingId, level: i32, lock: &DoorLock) -> bool {
    if !lock.ways.has(Ways::NET) || st.net.alert(b, st.net.now(st)) {
        return false;
    }
    let own = SiteNode {
        building: b,
        level,
        at: lock.at,
    };
    if hacked(st, own, &OPENS) {
        return true;
    }
    let Some(plan) = crate::interiors::plan_now(st, b) else {
        return false;
    };
    let Some(v) = plan.venue.as_deref() else {
        return false;
    };
    if let Some(piece) = v.gate_line_at(level, lock.at) {
        // Each held switch's line is read off the layout (`switch_line`), the level drawn
        // only where two lines' ends meet: the track asks this at every step of a route.
        return site_hacks(st, b, NodeKind::GateSwitch, &OPENS)
            .filter(|(s, _)| s.level == level)
            .any(|(s, _)| switch_line(v, level, s.at) == Some(piece));
    }
    // A way in's lock: the city's door locks held now, before the city's nodes are searched.
    let now = st.net.now(st);
    let held: Vec<NodeId> = (st.net.live(now))
        .filter(|h| h.kind == Target::Node(NodeKind::DoorLock) && OPENS.contains(&h.op))
        .filter_map(|h| match h.target {
            HackTarget::Node(id) => Some(id),
            _ => None,
        })
        .collect();
    let Some(city) = st.city.as_ref().filter(|_| !held.is_empty()) else {
        return false;
    };
    (v.ways_in.iter())
        .filter(|w| w.level == level && w.lock.is_some_and(|l| l.at == lock.at))
        .any(|w| match w.kind {
            WayInKind::Door { stratum, door } => (city.net.node_at(stratum, door))
                .is_some_and(|n| n.kind == NodeKind::DoorLock && held.contains(&n.id)),
            WayInKind::Crawl { .. } => false,
        })
}

/// Whether the Net holds private room `number` on `level` open: a hack on its door's
/// node; never in an alarm.
pub fn room_open(st: &AppState, b: BuildingId, level: i32, number: u16) -> bool {
    if st.net.alert(b, st.net.now(st)) {
        return false;
    }
    let door = crate::interiors::plan_now(st, b)
        .and_then(|plan| plan.venue.as_deref()?.room_door(level, number));
    door.is_some_and(|at| {
        let node = SiteNode {
            building: b,
            level,
            at,
        };
        hacked(st, node, &OPENS)
    })
}

/// A live Spoof on lift `link`'s own node (a stop of it) that landed at minute `since` or
/// later.
fn spoofed_since(st: &AppState, b: BuildingId, link: usize, since: u64) -> bool {
    let mut spoofs = site_hacks(st, b, NodeKind::LiftCtrl, &[Op::Spoof])
        .filter(|(_, h)| h.at >= since)
        .map(|(s, _)| s)
        .peekable();
    if spoofs.peek().is_none() {
        return false;
    }
    let Some(plan) = crate::interiors::plan_now(st, b) else {
        return false;
    };
    let Some(k) = plan.links.get(link).filter(|k| k.kind == LinkKind::Lift) else {
        return false;
    };
    spoofs.any(|s| (k.stops.iter()).any(|stop| (stop.level, stop.at) == (s.level, s.at)))
}

/// A live Spoof on a lift-control node of site `b` that stands at a stop of lift `link`.
pub fn lift_spoofed(st: &AppState, b: BuildingId, link: usize) -> bool {
    spoofed_since(st, b, link, 0)
}

/// Whether lift `link` of building `b` answers. Off alert, every lift answers; on alert,
/// one spoofed since the alarm: on a site a spoof on the lift's own node, elsewhere one on
/// the building's lift control on the city's Net (`NetState::lifts_answer`).
pub fn lift_answers(st: &AppState, b: BuildingId, link: usize) -> bool {
    let now = st.net.now(st);
    if !super::is_site(st, b) {
        return st.net.lifts_answer(b, now);
    }
    match st.net.alarm_since(b, now) {
        None => true,
        Some(since) => spoofed_since(st, b, link, since),
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::*;
    use super::super::*;
    use crate::credentials::How;
    use citygen::venue::{LineSwitch, PieceKind, WayInKind, Ways, line_switches};
    use citygen::{DoorLock, Floor, LinkKind, NodeKind, Pos, SiteNode};
    use game::{HackTarget, Op, Target};

    /// `lock`'s own node: its door's tile on its level.
    fn lock_node(b: BuildingId, level: i32, lock: &DoorLock) -> SiteNode {
        SiteNode {
            building: b,
            level,
            at: lock.at,
        }
    }

    /// Level `level`'s floor of `b`, ensured and cloned.
    fn floor_of(st: &mut AppState, b: BuildingId, level: i32) -> Floor {
        let city = st.city.clone().unwrap();
        st.interiors.ensure(&city, b, level).unwrap().clone()
    }

    /// The tile a step onto `lock`'s door comes from on its locked side: beside it,
    /// walkable, not its free side. The level's floor is ensured.
    fn outside_of(st: &AppState, b: BuildingId, level: i32, lock: &DoorLock) -> Pos {
        let (_, floor) = st.interiors.floor(b, level).unwrap();
        (lock.at.neighbors4())
            .find(|n| {
                Some(*n) != lock.free_from && floor.tiles.get(*n).is_some_and(|t| t.is_walkable())
            })
            .expect("a tile beside the door")
    }

    /// Two locks with the Net's way on the kept levels of `b`, lowest level first, each by
    /// its level: each a door-lock node of its own, with a locked side to step from. (A
    /// copy's floors keep a zone lock or so each; its private rooms' keypads are no locks.)
    fn two_net_locks(st: &mut AppState, b: BuildingId) -> [(i32, DoorLock); 2] {
        let net = net_of(st, b).unwrap();
        let levels: Vec<i32> = st.interiors.plan_of(b).unwrap().levels().collect();
        let mut found: Vec<(i32, DoorLock)> = Vec::new();
        for level in levels {
            let floor = floor_of(st, b, level);
            let locks = (floor.locks.iter())
                .filter(|l| l.ways.has(Ways::NET))
                .filter(|l| {
                    net.node(lock_node(b, level, l))
                        .is_some_and(|n| n.kind == NodeKind::DoorLock)
                })
                .filter(|l| {
                    (l.at.neighbors4()).any(|n| {
                        Some(n) != l.free_from
                            && floor.tiles.get(n).is_some_and(|t| t.is_walkable())
                    })
                });
            found.extend(locks.map(|l| (level, *l)));
        }
        match found[..] {
            [a, z, ..] => [a, z],
            _ => panic!("no two Net locks in {b:?}: {found:?}"),
        }
    }

    /// A gate line of `b`: its level, its switch and the locks on its turnstiles.
    fn a_gate_line(st: &AppState, b: BuildingId) -> (i32, LineSwitch, Vec<DoorLock>) {
        let plan = st.interiors.plan_of(b).unwrap();
        let v = plan.venue.as_deref().unwrap();
        plan.levels()
            .find_map(|level| {
                let switch = line_switches(v, level).into_iter().next()?;
                let x = v.level(level)?;
                let line = &x.pieces[switch.piece];
                assert_eq!(line.kind, PieceKind::GateLine);
                let turnstiles: Vec<DoorLock> = (x.doors.iter())
                    .filter(|d| line.rect.contains(d.at))
                    .filter_map(|d| d.lock)
                    .collect();
                (!turnstiles.is_empty()).then_some((level, switch, turnstiles))
            })
            .expect("a gate line with its switch")
    }

    /// Two private rooms on one kept level of `b`, by number, each behind a door of its
    /// own that is a door-lock node.
    fn two_private_rooms(st: &AppState, b: BuildingId) -> (i32, u16, u16) {
        let net = net_of(st, b).unwrap();
        let plan = st.interiors.plan_of(b).unwrap();
        let v = plan.venue.as_deref().unwrap();
        plan.levels()
            .find_map(|level| {
                let x = v.level(level)?;
                let rooms: Vec<(u16, Pos)> = (x.rooms.iter())
                    .filter(|r| r.private && r.number > 0)
                    .filter_map(|r| Some((r.number, v.room_door(level, r.number)?)))
                    .filter(|&(_, at)| {
                        let node = SiteNode {
                            building: b,
                            level,
                            at,
                        };
                        net.node(node).is_some_and(|n| n.kind == NodeKind::DoorLock)
                    })
                    .collect();
                let (n1, d1) = *rooms.first()?;
                let &(n2, _) = rooms.iter().find(|(n, d)| *n != n1 && *d != d1)?;
                Some((level, n1, n2))
            })
            .expect("two private rooms on a level")
    }

    #[test]
    fn cracking_one_doors_node_opens_that_door_and_no_other() {
        let (mut st, b) = office_copy(190);
        let [(level, a), (zl, z)] = two_net_locks(&mut st, b);
        hack(
            &mut st,
            lock_node(b, level, &a),
            NodeKind::DoorLock,
            Op::Crack,
        );
        let from = |l: i32, k: &DoorLock| outside_of(&st, b, l, k);
        assert_eq!(
            crate::credentials::opens_lock(&st, b, level, &a, from(level, &a)),
            Some(How::Cracked)
        );
        assert_eq!(
            crate::credentials::opens_lock(&st, b, zl, &z, from(zl, &z)),
            None
        );
        // Nor a private room's door on the cracked door's level.
        let plan = st.interiors.plan_of(b).unwrap();
        let v = plan.venue.as_deref().unwrap();
        let rooms = (v.level(level).unwrap().rooms.iter())
            .filter(|r| r.private && r.number > 0)
            .filter(|r| v.room_door(level, r.number).is_some_and(|d| d != a.at));
        let mut shut = 0;
        for r in rooms {
            let how = crate::credentials::opens(&st, b, level, r.number);
            assert_ne!(how, Some(How::Cracked), "{}", r.name);
            shut += 1;
        }
        assert!(shut > 0, "a private room beside the cracked door");
    }

    #[test]
    fn a_lifts_spoof_frees_its_stops_and_no_other_lifts() {
        let (mut st, b) = office_copy(190);
        let plan = st.interiors.plan_of(b).unwrap().clone();
        let carded: Vec<(usize, i32, u8)> = (0..plan.links.len())
            .filter(|&i| plan.links[i].kind == LinkKind::Lift)
            .flat_map(|i| {
                (plan.links[i].stops.iter())
                    .filter(|s| s.card > 0)
                    .map(move |s| (i, s.level, s.card))
            })
            .collect();
        let (one, _, card) = carded[0];
        let other = carded.iter().find(|c| c.0 != one).copied();
        let name = plan.links[one].name.clone().unwrap();
        let node = (net_of(&st, b).unwrap().nodes.into_iter())
            .find(|n| n.kind == NodeKind::LiftCtrl && n.name == name)
            .unwrap()
            .node;
        assert_eq!(crate::credentials::opens_stop(&st, b, one, card), None);
        hack(&mut st, node, NodeKind::LiftCtrl, Op::Spoof);
        assert_eq!(
            crate::credentials::opens_stop(&st, b, one, card),
            Some(How::Cracked)
        );
        if let Some((o, _, c)) = other {
            assert_eq!(crate::credentials::opens_stop(&st, b, o, c), None);
        }
    }

    #[test]
    fn in_an_alarm_a_lift_answers_only_to_a_spoof_on_its_own_node_since() {
        let (mut st, b) = office_copy(190);
        let plan = st.interiors.plan_of(b).unwrap().clone();
        let net = net_of(&st, b).unwrap();
        let lifts: Vec<(usize, SiteNode)> = (0..plan.links.len())
            .filter(|&i| plan.links[i].kind == LinkKind::Lift)
            .filter_map(|i| {
                let k = &plan.links[i];
                let n = (net.nodes.iter()).find(|n| {
                    n.kind == NodeKind::LiftCtrl
                        && k.stop(n.node.level).is_some_and(|s| s.at == n.node.at)
                })?;
                Some((i, n.node))
            })
            .collect();
        let [(one, node), (other, _), ..] = lifts[..] else {
            panic!("two lifts with nodes: {lifts:?}");
        };
        assert!(
            lift_answers(&st, b, one) && lift_answers(&st, b, other),
            "off alert"
        );
        let card = (plan.links[one].stops.iter())
            .find(|s| s.card > 0)
            .expect("a carded stop")
            .card;
        // A spoof landed a minute before the alarm doesn't count in it.
        hack(&mut st, node, NodeKind::LiftCtrl, Op::Spoof);
        match (st.city.as_ref(), st.world.as_mut()) {
            (Some(city), Some(world)) => {
                world.advance(city, world.tick() + sim::TICKS_PER_MINUTE);
            }
            _ => st.tick += sim::TICKS_PER_MINUTE,
        }
        let now = st.net.now(&st);
        st.net.trace(b).alert_until = Some(now + game::ALERT_MINUTES);
        assert!(lift_spoofed(&st, b, one), "still live");
        assert!(!lift_answers(&st, b, one), "spoofed before the alarm");
        assert!(!lift_answers(&st, b, other));
        let how = crate::credentials::opens_stop(&st, b, one, card);
        assert_eq!(how, None, "its stops held in the alarm");
        // One landed since makes it answer, and frees its stops.
        hack(&mut st, node, NodeKind::LiftCtrl, Op::Spoof);
        assert!(lift_answers(&st, b, one));
        assert!(!lift_answers(&st, b, other), "only its own node");
        let how = crate::credentials::opens_stop(&st, b, one, card);
        assert_eq!(how, Some(How::Cracked), "its stops freed");
        // On the floor: the spoofed lift rides, the other stays locked down.
        let ride_from = |st: &mut AppState, link: usize| {
            let stop = *plan.links[link].stop(0).expect("a stop on G");
            let p = st.player.as_mut().unwrap();
            p.place = citygen::Place::Floor {
                building: b,
                level: 0,
            };
            p.pos = stop.at;
            let to = plan.rides(link, 0)[0];
            crate::interiors::ride(st, b, to)
        };
        floor_of(&mut st, b, 0);
        assert_eq!(
            ride_from(&mut st, other),
            Err("lifts locked down".to_string())
        );
        assert!(ride_from(&mut st, one).is_ok());
    }

    #[test]
    fn a_gate_switch_opens_its_whole_line() {
        let (mut st, b) = office_plain();
        let (level, switch, turnstiles) = a_gate_line(&st, b);
        for l in &turnstiles {
            assert!(!lock_open(&st, b, level, l), "{:?} before", l.at);
        }
        let node = SiteNode {
            building: b,
            level,
            at: switch.at,
        };
        hack(&mut st, node, NodeKind::GateSwitch, Op::Crack);
        for l in &turnstiles {
            assert!(lock_open(&st, b, level, l), "{:?}", l.at);
        }
    }

    #[test]
    fn a_rooms_keypad_node_opens_that_room_only() {
        let (mut st, b) = office_plain();
        let (level, n1, n2) = two_private_rooms(&st, b);
        let door = (st.interiors.plan_of(b).unwrap().venue.as_deref())
            .unwrap()
            .room_door(level, n1)
            .unwrap();
        assert_ne!(
            crate::credentials::opens(&st, b, level, n1),
            Some(How::Cracked)
        );
        let node = SiteNode {
            building: b,
            level,
            at: door,
        };
        hack(&mut st, node, NodeKind::DoorLock, Op::Crack);
        assert_eq!(
            crate::credentials::opens(&st, b, level, n1),
            Some(How::Cracked)
        );
        assert_ne!(
            crate::credentials::opens(&st, b, level, n2),
            Some(How::Cracked)
        );
    }

    #[test]
    fn a_street_doors_city_lock_opens_only_its_door() {
        let (mut st, b, level, way) = a_locked_street_way_in();
        let WayInKind::Door { stratum, door } = way.kind else {
            unreachable!()
        };
        let id = (st.city.as_ref().unwrap().net.node_at(stratum, door))
            .unwrap()
            .id;
        let lock = way.lock.unwrap();
        assert!(!lock_open(&st, b, level, &lock), "shut before the crack");
        let now = st.net.now(&st);
        st.net.land(
            b,
            HackTarget::Node(id),
            Target::Node(NodeKind::DoorLock),
            Op::Crack,
            now,
        );
        assert!(lock_open(&st, b, level, &lock));
        let levels = st.interiors.plan_of(b).unwrap().levels.clone();
        let mut others = 0;
        for l in levels {
            let floor = floor_of(&mut st, b, l);
            for lock in floor.locks.iter().filter(|k| (l, k.at) != (level, way.at)) {
                assert!(!lock_open(&st, b, l, lock), "{l} {:?}", lock.at);
                others += 1;
            }
        }
        assert!(others > 0, "other locks to stay shut");
    }

    #[test]
    fn an_alarm_relocks_a_cracked_door_and_says_locked_down() {
        let (mut st, b) = office_copy(190);
        let [(level, a), _] = two_net_locks(&mut st, b);
        hack(
            &mut st,
            lock_node(b, level, &a),
            NodeKind::DoorLock,
            Op::Crack,
        );
        assert!(lock_open(&st, b, level, &a));
        crate::net::lock_down(&mut st, b);
        assert!(!lock_open(&st, b, level, &a));
        let plan = st.interiors.plan_of(b).unwrap().clone();
        let from = outside_of(&st, b, level, &a);
        assert!(matches!(
            crate::venues::meets_lock(&st, &plan, level, &a, from),
            crate::venues::Meets::Held(w) if w.starts_with("locked down")
        ));
    }
}
