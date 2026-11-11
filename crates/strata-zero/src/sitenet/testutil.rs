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

/// You on the city's Net on tile `at`: jacked in from the street where you stand (the body
/// left there), or moved there from wherever you are on the Net.
fn on_the_grid(st: &mut AppState, at: Pos) {
    let p = st.player.as_mut().expect("you");
    if !p.jacked_in() {
        assert!(p.place.is_street(), "on the street to jack in");
        p.body = Some((p.place, p.pos));
    }
    st.net.beat = st.clock_tick();
    crate::constructs::land(st, Place::Stratum(StratumKind::Net), at);
}

/// You on the city's Net on an open tile beside `at`, off every building's footprint.
pub(crate) fn stand_on_the_grid_beside(st: &mut AppState, at: Pos) {
    let city = st.city.as_ref().unwrap();
    let off = |p: Pos| {
        city.walkable(StratumKind::Net, p)
            && !(city.buildings.iter()).any(|x| x.footprint.contains(p))
    };
    let spot = (at.neighbors8())
        .find(|p| off(*p))
        .unwrap_or_else(|| panic!("no grid tile beside {at:?}"));
    on_the_grid(st, spot);
}

/// You on the city's Net beside site `b`'s gate, jacked in.
pub(crate) fn at_the_gate(st: &mut AppState, b: BuildingId) {
    let gate = crate::constructs::city_gate(st.city.as_ref().unwrap(), b).expect("a gate");
    stand_on_the_grid_beside(st, gate);
}

/// You, on a site's Net, put on a tile beside `at` on its level you may stand on.
pub(crate) fn walk_beside(st: &mut AppState, at: Pos) {
    let Some(Place::SiteNet { building: b, .. }) = st.player.as_ref().map(|p| p.place) else {
        panic!("on a site's Net");
    };
    let k = st.constructs.site(b).unwrap();
    let level = k.level_at(at).expect("a tile of a level").level;
    let spot = (at.neighbors8())
        .find(|p| open(st, b, *p) && k.level_at(*p).is_some_and(|l| l.level == level))
        .unwrap_or_else(|| panic!("no tile beside {at:?}"));
    crate::constructs::land(st, Place::SiteNet { building: b, level }, spot);
}

/// You on site `b`'s Net, on the gate inside (its construct's entry).
pub(crate) fn walk_to_the_gate_inside(st: &mut AppState, b: BuildingId) {
    let k = st.constructs.site(b).expect("the site's construct");
    let (entry, level) = (k.entry, k.level_at(k.entry).expect("on a level").level);
    assert!(
        st.player.as_ref().is_some_and(|p| p.jacked_in()),
        "jacked in"
    );
    crate::constructs::land(st, Place::SiteNet { building: b, level }, entry);
}

/// A door-lock node of site `b`'s Net on a level on the riser whose door has a lock on its
/// floor: its level and the node.
pub(crate) fn a_door_node(st: &AppState, b: BuildingId) -> (i32, SiteNode) {
    let net = crate::sitenet::net_of(st, b).expect("a site's Net");
    (net.nodes.iter())
        .filter(|n| n.kind == NodeKind::DoorLock && net.on_riser(n.node.level))
        .map(|n| (n.node.level, n.node))
        .find(|&(level, node)| floor_lock(st, b, level, node).is_some())
        .expect("a door node with a lock on its floor")
}

/// The lock on its floor of site `b`'s door node `door` on `level`.
pub(crate) fn lock_of(st: &AppState, b: BuildingId, level: i32, door: SiteNode) -> DoorLock {
    floor_lock(st, b, level, door).expect("a lock on the floor")
}

/// The lock on its floor at site `b`'s door node `door` on `level`, if one is there.
fn floor_lock(st: &AppState, b: BuildingId, level: i32, door: SiteNode) -> Option<DoorLock> {
    let plan = crate::interiors::plan_now(st, b)?;
    let floor = crate::interiors::floor_now(st, &plan, level)?;
    floor.lock_at(door.at).copied()
}

/// The tiles of site `b`'s floor `level` you may stand on beside `at` (Chebyshev 1):
/// walkable, nobody standing there. The floor is made.
fn floor_tiles_beside(st: &mut AppState, b: BuildingId, level: i32, at: Pos) -> Vec<Pos> {
    let city = st.city.clone().expect("a city");
    let floor = st.interiors.ensure(&city, b, level).expect("a kept level");
    let place = Place::Floor { building: b, level };
    let open: Vec<Pos> = (at.neighbors8())
        .filter(|p| floor.tiles.get(*p).is_some_and(|t| t.is_walkable()))
        .collect();
    (open.into_iter())
        .filter(|p| crate::foes::foe_at(st, place, *p).is_none())
        .collect()
}

/// You on foot at `place`, on tile `at`.
fn stand_on(st: &mut AppState, place: Place, at: Pos) {
    let p = st.player.as_mut().expect("you");
    assert!(!p.jacked_in(), "on foot");
    crate::constructs::land(st, place, at);
}

/// You on foot on site `b`'s floor `level`, on tile `at`.
fn stand_on_floor(st: &mut AppState, b: BuildingId, level: i32, at: Pos) {
    stand_on(st, Place::Floor { building: b, level }, at);
}

/// You on foot on site `b`'s floor beside the wall tile of its node `node`: on the tile in
/// front of it, else another beside it.
fn stand_beside_on_floor(st: &mut AppState, node: SiteNode) {
    let net = crate::sitenet::net_of(st, node.building).expect("a site's Net");
    let front = net.node(node).expect("its node").net;
    let mut tiles = floor_tiles_beside(st, node.building, node.level, node.at);
    tiles.sort_by_key(|p| *p != front);
    let at = *tiles.first().expect("a tile beside it");
    stand_on_floor(st, node.building, node.level, at);
}

/// You on foot on L of site `b`, beside the wall tile of its public kiosk (the terminals
/// pass's public one): its node.
pub(crate) fn stand_beside_the_kiosk(st: &mut AppState, b: BuildingId) -> SiteNode {
    let plan = crate::interiors::plan_now(st, b)
        .expect("a plan")
        .into_owned();
    let v = plan.venue.as_deref().expect("a venue's plan");
    let floor = citygen::venue::draw_level(v, 0).expect("its L");
    let t = VenuePack::builtin().by_id(&v.type_id);
    let kiosk = (citygen::venue::terminals(v, t, 0, &floor, false).into_iter())
        .find(|d| d.public)
        .expect("L's public kiosk");
    let node = SiteNode {
        building: b,
        level: 0,
        at: kiosk.at,
    };
    stand_beside_on_floor(st, node);
    node
}

/// You on foot beside the wall tile of site `b`'s first gate switch, on its level: its
/// node.
pub(crate) fn stand_beside_the_switch(st: &mut AppState, b: BuildingId) -> SiteNode {
    let net = crate::sitenet::net_of(st, b).expect("a site's Net");
    let node = (net.nodes.iter())
        .find(|n| n.kind == NodeKind::GateSwitch)
        .expect("a gate switch")
        .node;
    stand_beside_on_floor(st, node);
    node
}

/// You on foot beside a camera of site `b` on its floor, on a tile with no terminal or
/// gate switch beside it: the camera's node.
pub(crate) fn stand_beside_a_camera(st: &mut AppState, b: BuildingId) -> SiteNode {
    let net = crate::sitenet::net_of(st, b).expect("a site's Net");
    let ports: Vec<SiteNode> = (net.nodes.iter())
        .filter(|n| matches!(n.kind, NodeKind::Terminal | NodeKind::GateSwitch))
        .map(|n| n.node)
        .collect();
    let cameras = (net.nodes.iter()).filter(|n| n.kind == NodeKind::Camera);
    for cam in cameras.map(|n| n.node) {
        let lone =
            |p: &Pos| !(ports.iter()).any(|d| d.level == cam.level && d.at.chebyshev(*p) <= 1);
        let tiles = floor_tiles_beside(st, b, cam.level, cam.at);
        if let Some(at) = tiles.into_iter().find(lone) {
            stand_on_floor(st, b, cam.level, at);
            return cam;
        }
    }
    panic!("no camera of {b:?} with a tile beside it away from its ports");
}

/// You on foot behind the wall of a terminal of site `b`: beside its wall tile, in the next
/// room or the corridor, not beside the floor tile in front of it, and on the far side of
/// every other terminal and switch near too. The terminal's node.
pub(crate) fn stand_behind_a_terminal(st: &mut AppState, b: BuildingId) -> SiteNode {
    let net = crate::sitenet::net_of(st, b).expect("a site's Net");
    let ports: Vec<(SiteNode, Pos)> = (net.nodes.iter())
        .filter(|n| matches!(n.kind, NodeKind::Terminal | NodeKind::GateSwitch))
        .map(|n| (n.node, n.net))
        .collect();
    for &(node, _) in ports
        .iter()
        .filter(|(n, _)| net.node(*n).unwrap().kind == NodeKind::Terminal)
    {
        let behind = |p: &Pos| {
            !(ports.iter()).any(|(d, front)| {
                d.level == node.level && d.at.chebyshev(*p) <= 1 && front.chebyshev(*p) <= 1
            })
        };
        let tiles = floor_tiles_beside(st, b, node.level, node.at);
        if let Some(at) = tiles.into_iter().find(behind) {
            stand_on_floor(st, b, node.level, at);
            return node;
        }
    }
    panic!("no terminal of {b:?} with a floor tile behind its wall");
}

/// You at home, on foot in the flat beside the rig.
pub(crate) fn stand_at_the_rig(st: &mut AppState) {
    let h = st.home.expect("a home");
    let city = st.city.clone().expect("a city");
    let floor = (st.interiors)
        .ensure(&city, h.home.building, h.home.level)
        .expect("the flat's floor");
    let at = (h.home.rig.neighbors8())
        .find(|q| h.home.rect.contains(*q) && floor.tiles[*q].is_walkable())
        .expect("a tile beside the rig");
    stand_on(st, h.place(), at);
}
