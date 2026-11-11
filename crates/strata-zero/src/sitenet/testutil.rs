//! Sites to test the Net of a site on: an office laid on a building away from home, plain
//! or with the DEVELOPER start's mission copy, and D'SHAWN with the programs a run wants;
//! and where to stand on its Net.

use crate::app::AppState;
use citygen::venue::{MissionInputs, MissionSize, VenuePack, WayIn, WayInKind, Ways};
use citygen::{
    BuildingId, BuildingKind, DoorLock, NetDevice, NodeKind, Place, Pos, SiteNode, StratumKind,
};
use game::{HackTarget, ItemId, Op, Target};

/// `jobs::testutil::state_home`'s city, D'SHAWN in dev mode with crack, spoof, blind, copy
/// and backdoor loaded.
fn runner_home() -> AppState {
    let mut st = crate::jobs::testutil::state_home();
    st.character = game::Character::debug_runner(&st.content);
    st.unlock_dev();
    for id in ["crack", "spoof", "blind", "copy", "backdoor"] {
        let id = ItemId::new(id);
        if st.character.count(&id) == 0 {
            st.character.give(id.clone(), 1);
        }
        if !st.character.loaded.contains(&id) {
            st.character.loaded.push(id);
        }
    }
    st
}

/// An office laid on a building away from home, no job on it, you on the street at its
/// main door.
pub(crate) fn office_plain() -> (AppState, BuildingId) {
    let mut st = runner_home();
    let t = VenuePack::builtin()
        .for_kind(BuildingKind::Office)
        .expect("a type builds offices");
    let inputs = MissionInputs {
        size: MissionSize::Medium,
        objective: 0,
        security: 128,
        seed: st.tick,
    };
    let b = crate::venues::dev_start_with(&mut st, &t.id, inputs, false).unwrap();
    (st, b)
}

/// A locked Street way in of `b`'s plan with the Net's way, onto a level it keeps, at a
/// city-map door with a lock on the city's Net.
fn street_way_in(st: &AppState, b: BuildingId) -> Option<WayIn> {
    let city = st.city.as_ref()?;
    let plan = st.interiors.plan_of(b)?;
    let v = plan.venue.as_deref()?;
    (v.ways_in.iter())
        .find(|w| {
            let WayInKind::Door { stratum, door } = w.kind else {
                return false;
            };
            stratum == StratumKind::Street
                && plan.has_level(w.level)
                && w.lock.is_some_and(|l| l.ways.has(Ways::NET))
                && (city.net.node_at(stratum, door)).is_some_and(|n| n.kind == NodeKind::DoorLock)
        })
        .copied()
}

/// An office site whose plan has a locked Street way in: the plain office first, else
/// each closed office building of the city as it lays, its plan kept.
pub(crate) fn a_locked_street_way_in() -> (AppState, BuildingId, i32, WayIn) {
    let (mut st, b) = office_plain();
    if let Some(way) = street_way_in(&st, b) {
        return (st, b, way.level, way);
    }
    let city = st.city.clone().unwrap();
    let offices: Vec<BuildingId> = (city.buildings.iter())
        .filter(|x| x.kind == BuildingKind::Office && x.kind.is_closed())
        .map(|x| x.id)
        .collect();
    for b in offices {
        if st
            .interiors
            .plan(&city, b)
            .is_none_or(|p| p.venue.is_none())
        {
            continue;
        }
        if let Some(way) = street_way_in(&st, b) {
            return (st, b, way.level, way);
        }
    }
    panic!("no office with a locked street door on the city's Net");
}

/// The DEVELOPER start's office copy at `security`: its server room's job begun, you on
/// the street at its main door.
pub(crate) fn office_copy(security: u8) -> (AppState, BuildingId) {
    let mut st = runner_home();
    let b = crate::venues::dev_site_net(&mut st, security).unwrap();
    (st, b)
}

/// You on level `level`'s hub of site `b`'s Net, its construct made: jacked in from the
/// street where you stand (the body left there, outside the building), or put there from
/// wherever you stand on its Net.
pub(crate) fn on_hub(st: &mut AppState, b: BuildingId, level: i32) {
    crate::sitenet::ensure_construct(st, b).unwrap();
    let hub = (st.constructs.site(b))
        .and_then(|k| k.net_level(level))
        .expect("a kept level")
        .hub;
    let p = st.player.as_mut().expect("you");
    if !matches!(p.place, Place::SiteNet { building, .. } if building == b) {
        assert!(p.place.is_street(), "on the street to jack in");
        p.body = Some((p.place, p.pos));
    }
    st.net.beat = st.clock_tick();
    crate::constructs::land(st, Place::SiteNet { building: b, level }, hub);
}

/// A hack of `op` landed on site node `node` (a `kind`), now.
pub(crate) fn hack(st: &mut AppState, node: SiteNode, kind: NodeKind, op: Op) {
    let now = st.net.now(st);
    st.net.land(
        node.building,
        HackTarget::Site(node),
        Target::Node(kind),
        op,
        now,
    );
}

/// Site `b`'s construct's first node of `kind`.
pub(crate) fn device(st: &AppState, b: BuildingId, kind: NodeKind) -> NetDevice {
    let k = st.constructs.site(b).expect("the site's construct");
    (k.devices.iter())
        .find(|d| d.kind == kind)
        .unwrap_or_else(|| panic!("no {kind:?} on {b:?}'s Net"))
        .clone()
}

/// A tile of site `b`'s construct you may stand on: walkable, no ICE on it.
fn open(st: &AppState, b: BuildingId, p: Pos) -> bool {
    st.constructs
        .site(b)
        .is_some_and(|k| k.tiles.get(p).is_some_and(|t| t.is_walkable()) && k.guard_at(p).is_none())
}

/// You on site `b`'s Net on `d`'s level, through its hub, then put on a tile beside it.
fn stand_beside(st: &mut AppState, b: BuildingId, d: &NetDevice) {
    on_hub(st, b, d.node.level);
    let at = (d.at.neighbors8())
        .find(|p| open(st, b, *p))
        .expect("a tile beside the node");
    let place = Place::SiteNet {
        building: b,
        level: d.node.level,
    };
    crate::constructs::land(st, place, at);
}

/// You on site `b`'s Net beside its first node of `kind`, on its level.
pub(crate) fn beside_device(st: &mut AppState, b: BuildingId, kind: NodeKind) -> NetDevice {
    crate::sitenet::ensure_construct(st, b).unwrap();
    let d = device(st, b, kind);
    stand_beside(st, b, &d);
    d
}

/// Site `b`'s door-lock nodes on levels on the riser whose door has a lock on its floor,
/// each with that lock; the construct made.
fn door_nodes(st: &mut AppState, b: BuildingId) -> Vec<(NetDevice, DoorLock)> {
    crate::sitenet::ensure_construct(st, b).unwrap();
    let k = st.constructs.site(b).unwrap();
    let plan = st.interiors.plan_of(b).unwrap();
    (k.devices.iter())
        .filter(|d| d.kind == NodeKind::DoorLock)
        .filter(|d| k.net_level(d.node.level).is_some_and(|l| !l.gapped))
        .filter_map(|d| {
            let floor = crate::interiors::floor_now(st, plan, d.node.level)?;
            Some((d.clone(), *floor.lock_at(d.node.at)?))
        })
        .collect()
}

/// You on site `b`'s Net beside a door-lock node on a level on the riser whose door has a
/// lock on its floor: its level and the node.
pub(crate) fn beside_a_door_node(st: &mut AppState, b: BuildingId) -> (i32, NetDevice) {
    let (d, _) = (door_nodes(st, b).into_iter())
        .find(|(d, _)| d.at.neighbors8().any(|p| open(st, b, p)))
        .expect("a door node with a tile beside it");
    stand_beside(st, b, &d);
    (d.node.level, d)
}

/// You on site `b`'s Net on the level of a door-lock node whose floor lock opens from a
/// free side: the tile on its locked side and the one on its free side, in the grid, each
/// one you may stand on, and the node.
pub(crate) fn across_a_door_node(st: &mut AppState, b: BuildingId) -> (Pos, Pos, NetDevice) {
    for (d, lock) in door_nodes(st, b) {
        let k = st.constructs.site(b).unwrap();
        let grid = |p: Pos| k.on_level(d.node.level, p).filter(|q| open(st, b, *q));
        let Some(free) = lock.free_from.and_then(grid) else {
            continue;
        };
        let locked = (lock.at.neighbors4())
            .filter(|n| Some(*n) != lock.free_from)
            .find_map(grid);
        if let Some(locked) = locked {
            on_hub(st, b, d.node.level);
            return (locked, free, d);
        }
    }
    panic!("no door node with a free side on {b:?}'s Net")
}
