//! Sites to test the Net of a site on: an office laid on a building away from home, plain
//! or with the DEVELOPER start's mission copy, and D'SHAWN with the programs a run wants;
//! where to stand on its Net, on its floors and outside it; programs run to their landings
//! and minutes passed as the clock runs them; and HELIX shells on its host and its street.

use crate::app::AppState;
use crate::cameras::Camera;
use crate::helix::bin::new_shell;
use crate::helix::shell::{HostId, Shell};
use crate::owners::Owner;
use crate::render::map::{FloorMap, MapRef};
use crate::render::netlook::{NetLook, NetLookView};
use crate::render::viewport::Overlay;
use citygen::venue::{DeviceKind, MissionInputs, MissionSize, VenuePack, WayIn, WayInKind, Ways};
use citygen::{
    BuildingId, BuildingKind, DoorLock, Floor, Grid, LinkKind, NetDevice, NetNode, NodeKind, Place,
    Pos, SiteNode, Size, StratumKind, Terrain, Tile,
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

/// Site `b`'s server cracked now: root on its host for the hour.
pub(crate) fn crack_server(st: &mut AppState, b: BuildingId) {
    let net = crate::sitenet::net_of(st, b).expect("a site's Net");
    let server = (net.nodes.iter())
        .find(|n| n.kind == NodeKind::Server)
        .expect("its server")
        .node;
    hack(st, server, NodeKind::Server, Op::Crack);
    assert_eq!(crate::access::tier_on(st, b), crate::access::Tier::Admin);
}

/// A shell logged in on site `b`'s host as a user: a terminal of it on the riser held from
/// the Net, its construct made.
pub(crate) fn shell_on(st: &mut AppState, b: BuildingId) -> Shell {
    crate::sitenet::ensure_construct(st, b).unwrap();
    let net = crate::sitenet::net_of(st, b).expect("a site's Net");
    let terminal = (net.nodes.iter())
        .find(|n| n.kind == NodeKind::Terminal && net.on_riser(n.node.level))
        .expect("a terminal on the riser")
        .node;
    let now = st.net.now(st);
    crate::holds::take(st, NetNode::Site(terminal), Owner::Building(b), now);
    let sh = new_shell(st, HostId::Building(b));
    assert_eq!(sh.user, "user", "a user's seat");
    sh
}

/// A district terminal's shell in site `b`'s district, and the site's number in its
/// `net` (as `scan` lists it and `ssh <n>` takes it).
pub(crate) fn street_shell_listing(st: &mut AppState, b: BuildingId) -> (Shell, usize) {
    let d = st.city.as_ref().expect("a city").building(b).district;
    let n = (crate::access::scannable(st, d).iter())
        .position(|x| *x == b)
        .expect("the site on its district's net");
    (new_shell(st, HostId::District(d)), n + 1)
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

/// The city's Net drawn with the look round tile `at` (80 by 40 tiles at most), its sites'
/// firewall locks filled from that view as the map fills them: a buffer whose cell (x, y)
/// is tile (x, y).
pub(crate) fn paint_grid_at(st: &AppState, at: Pos) -> ratatui::buffer::Buffer {
    let city = st.city.as_ref().expect("a city");
    let (w, h) = ((city.size.w as i32).min(80), (city.size.h as i32).min(40));
    let x0 = (at.x - w / 2).clamp(0, city.size.w as i32 - w);
    let y0 = (at.y - h / 2).clamp(0, city.size.h as i32 - h);
    let view = citygen::Rect::new(x0, y0, w, h);
    let look = NetLookView {
        firewalls: crate::sitenet::grid_locks(st, view),
        ..NetLookView::default()
    };
    let area = ratatui::layout::Rect::new(x0 as u16, y0 as u16, w as u16, h as u16);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    let map = MapRef::stratum_of(city, StratumKind::Net);
    let net = NetLook {
        view: &look,
        keep: None,
    };
    net.draw(&map, Pos::new(x0, y0), area, st.tick, &mut buf, &st.style);
    buf
}

/// The character drawn on tile `p` of a buffer `paint_grid_at` painted.
pub(crate) fn cell_char(buf: &ratatui::buffer::Buffer, p: Pos) -> char {
    let cell = &buf[(p.x as u16, p.y as u16)];
    cell.symbol().chars().next().unwrap_or(' ')
}

/// The game's clock a game minute on: the world's, which `NetState::now` reads, and the
/// frame tick with it. Nothing else of the minute runs (no cooling, no cameras).
pub(crate) fn next_minute(st: &mut AppState) {
    st.tick += sim::TICKS_PER_MINUTE;
    if let (Some(city), Some(w)) = (st.city.as_ref(), st.world.as_mut()) {
        w.advance(city, w.tick() + sim::TICKS_PER_MINUTE);
    }
}

/// Game minutes passing as the clock runs them, a minute at a time as a rest does
/// (`clock::run`): the Net's minute (traces cool, hacks run out), the cameras, the copies.
pub(crate) fn pass_minutes(st: &mut AppState, minutes: u64) {
    for _ in 0..minutes {
        crate::clock::run(
            st,
            Some(sim::TICKS_PER_MINUTE),
            crate::clock::PATHS_PER_FRAME,
        );
    }
}

/// Your heat on building `b`'s trace.
pub(crate) fn heat(st: &AppState, b: BuildingId) -> u8 {
    st.net.traces.get(&b).map_or(0, |t| t.heat)
}

/// You on foot out of site `b`, on the street by its main street door (the first open one),
/// on the nearest tile no camera sees.
pub(crate) fn stand_outside(st: &mut AppState, b: BuildingId) {
    let city = st.city.clone().expect("a city");
    let plan = crate::interiors::plan_now(st, b).expect("a plan");
    let v = plan.venue.as_deref().expect("a venue's plan");
    let door = (v.frame.doors.iter())
        .find(|d| d.stratum == StratumKind::Street && !d.enclosed)
        .expect("a street door")
        .door;
    let fp = v.frame.footprint;
    drop(plan);
    let mut spots: Vec<Pos> = (fp.grow(4).iter())
        .filter(|p| !fp.contains(*p) && city.walkable(StratumKind::Street, *p))
        .collect();
    spots.sort_by_key(|p| (p.manhattan(door), p.y, p.x));
    let street = Place::Stratum(StratumKind::Street);
    for at in spots {
        stand_on(st, street, at);
        if crate::cameras::seeing(st).is_empty() {
            return;
        }
    }
    panic!("nowhere outside {b:?} out of the cameras' sight");
}

/// Site `b`'s first node of `kind` on the level of its Net you stand on, and you on a tile
/// beside it there.
pub(crate) fn walk_beside_kind(st: &mut AppState, b: BuildingId, kind: NodeKind) -> NetDevice {
    let Some(Place::SiteNet { level, .. }) = st.player.as_ref().map(|p| p.place) else {
        panic!("on a site's Net");
    };
    let k = st.constructs.site(b).expect("the site's construct");
    let d = (k.devices.iter())
        .find(|d| d.kind == kind && d.node.level == level)
        .unwrap_or_else(|| panic!("no {kind:?} on level {level}"))
        .clone();
    walk_beside(st, d.at);
    d
}

/// A turnstile of the line gate switch `switch` works, the lock on its floor.
pub(crate) fn a_turnstile(st: &AppState, switch: SiteNode) -> DoorLock {
    let plan = crate::interiors::plan_now(st, switch.building).expect("a plan");
    let v = plan.venue.as_deref().expect("a venue's plan");
    let piece = citygen::venue::switch_line(v, switch.level, switch.at).expect("its line");
    let x = v.level(switch.level).expect("its level");
    let floor = crate::interiors::floor_now(st, &plan, switch.level).expect("its floor");
    (x.doors.iter())
        .filter(|d| x.pieces[piece].rect.contains(d.at))
        .find_map(|d| floor.lock_at(d.at).copied())
        .expect("a turnstile on the line")
}

/// A floor tile you may stand on beside `lock` on site `b`'s floor `level`, on the side it
/// doesn't open from by itself.
pub(crate) fn locked_side(st: &AppState, b: BuildingId, level: i32, lock: &DoorLock) -> Pos {
    let (_, floor) = st
        .interiors
        .floor(b, level)
        .expect("its floor, walked onto");
    (lock.at.neighbors4())
        .filter(|p| Some(*p) != lock.free_from)
        .find(|p| {
            floor.tiles.get(*p).is_some_and(|t| t.is_walkable()) && floor.lock_at(*p).is_none()
        })
        .expect("a tile on its locked side")
}

/// Site `b`'s lift-control node of a lift that stops on level `to`.
pub(crate) fn goods_lift(st: &AppState, b: BuildingId, to: i32) -> NetDevice {
    let plan = crate::interiors::plan_now(st, b).expect("a plan");
    let k = st.constructs.site(b).expect("the site's construct");
    let stops_at = |d: &NetDevice| {
        (plan.links.iter())
            .filter(|l| l.kind == LinkKind::Lift && l.stop(to).is_some())
            .any(|l| l.stop(d.node.level).is_some_and(|s| s.at == d.node.at))
    };
    (k.devices.iter())
        .find(|d| d.kind == NodeKind::LiftCtrl && stops_at(d))
        .unwrap_or_else(|| panic!("no lift of {b:?} stops on {to}"))
        .clone()
}

/// A program run to its landing on `target`, every roll a hit.
fn run_to_landing(st: &mut AppState, program: &str, target: HackTarget) -> Vec<String> {
    st.net.force_roll = Some(10);
    crate::net::start_run(st, ItemId::new(program), target).unwrap();
    let mut lines = Vec::new();
    while st.net.run.is_some() {
        lines.extend(crate::net::step_run(st));
    }
    lines
}

/// A spoof run to its landing on `target`, every roll a hit.
pub(crate) fn spoof_to_landing(st: &mut AppState, target: HackTarget) -> Vec<String> {
    run_to_landing(st, "spoof", target)
}

/// A copy run to its landing on `target`, every roll a hit.
pub(crate) fn copy_to_landing(st: &mut AppState, target: HackTarget) -> Vec<String> {
    run_to_landing(st, "copy", target)
}

/// You on foot on the lift at node `lift`'s stop (its level, its landing), riding it to
/// level `to` (`interiors::ride`).
pub(crate) fn ride_to(st: &mut AppState, lift: SiteNode, to: i32) -> Result<String, String> {
    let city = st.city.clone().expect("a city");
    (st.interiors.ensure(&city, lift.building, lift.level)).expect("a kept level");
    stand_on_floor(st, lift.building, lift.level, lift.at);
    crate::interiors::ride(st, lift.building, to)
}

/// You on foot on site `b`'s floor `level`, in front of its terminal there: its node.
pub(crate) fn stand_beside_terminal_on(st: &mut AppState, b: BuildingId, level: i32) -> SiteNode {
    let net = crate::sitenet::net_of(st, b).expect("a site's Net");
    let node = (net.nodes.iter())
        .find(|n| n.kind == NodeKind::Terminal && n.node.level == level)
        .unwrap_or_else(|| panic!("no terminal on level {level}"))
        .node;
    stand_beside_on_floor(st, node);
    node
}

/// From your body's floor tile, floor camera `cam` cracked from beside its node on its
/// level of the site's Net and looped (`o`), and you back on foot where you stood.
pub(crate) fn loop_from_the_net(st: &mut AppState, cam: &Camera) {
    let n = cam.node.site().expect("a site's camera");
    let (place, here) = st.player.as_ref().map(|p| (p.place, p.pos)).expect("you");
    crate::sitenet::ensure_construct(st, n.building).unwrap();
    let at = (st.constructs.site(n.building).unwrap().device(n))
        .expect("its node")
        .at;
    st.player.as_mut().unwrap().body = Some((place, here));
    let level = Place::SiteNet {
        building: n.building,
        level: n.level,
    };
    crate::constructs::land(st, level, at);
    walk_beside(st, at);
    st.net.beat = st.clock_tick();
    crate::net::tests::crack_to_landing(st, HackTarget::Site(n));
    crate::holds::use_verb(st, 'o').unwrap();
    crate::net::jack_out(st).unwrap();
    assert_eq!(
        st.player.as_ref().map(|p| (p.place, p.pos)),
        Some((place, here))
    );
}

/// You jacked in at site `b`'s lobby kiosk, on L.
pub(crate) fn jacked_in_at_the_kiosk(st: &mut AppState, b: BuildingId) {
    stand_beside_the_kiosk(st, b);
    crate::net::jack_in_here(st).unwrap();
}

/// You jacked in at the terminal of site `b`'s level behind its air gap: that level.
pub(crate) fn jacked_in_across_the_gap(st: &mut AppState, b: BuildingId) -> i32 {
    let gap = (crate::sitenet::net_of(st, b).expect("a site's Net").gap).expect("an air gap");
    stand_beside_terminal_on(st, b, gap);
    crate::net::jack_in_here(st).unwrap();
    gap
}

/// The map where you stand on a `size` terminal: its first frame learns the map's size,
/// then the camera lands on you as it does when you land on a level.
pub(crate) fn drawn(st: &mut AppState, size: (u16, u16)) -> ratatui::buffer::Buffer {
    let mut map = crate::screens::explore::MapScreen::walking(st);
    crate::screens::shots::draw_stack(st, &[&map], size);
    let you = st.player.as_ref().expect("you").pos;
    map.jump_to(st, you);
    crate::screens::shots::draw_stack(st, &[&map], size)
}

/// The map area of a drawn screen, left of the panel: a row of cells each, by the first
/// character of each cell.
pub(crate) fn map_cells(buf: &ratatui::buffer::Buffer) -> Vec<Vec<char>> {
    let map = crate::screens::explore::layout(buf.area, false).map;
    (map.y..map.bottom())
        .map(|y| {
            (map.x..map.right())
                .map(|x| buf[(x, y)].symbol().chars().next().unwrap_or(' '))
                .collect()
        })
        .collect()
}

/// Whether the open level's label is in the map area of a drawn screen: its key over its
/// role, both ending on one column (the tower's label column), or the one-row tag in the
/// map's top-left corner that stands for it, `▸ {key} {role}`.
pub(crate) fn label_shown(buf: &ratatui::buffer::Buffer, key: &str, role: &str) -> bool {
    let rows = map_cells(buf);
    let first: String = rows
        .first()
        .map_or_else(String::new, |r| r.iter().collect());
    if first.starts_with(&format!("▸ {key} {role}")) {
        return true;
    }
    // The columns `s` ends on in `row`, standing as a word of its own.
    let ends = |row: &[char], s: &str| -> Vec<usize> {
        let s: Vec<char> = s.chars().collect();
        if s.is_empty() || s.len() > row.len() {
            return Vec::new();
        }
        (0..=row.len() - s.len())
            .filter(|&i| row[i..i + s.len()] == s[..])
            .filter(|&i| i == 0 || row[i - 1] == ' ')
            .map(|i| i + s.len())
            .collect()
    };
    (rows.windows(2)).any(|w| {
        let keys = ends(&w[0], key);
        ends(&w[1], role).iter().any(|e| keys.contains(e))
    })
}

/// A `w` by `h` floor walled round, floor inside, unlit.
pub(crate) fn tiny_room(w: u32, h: u32) -> Floor {
    let size = Size { w, h };
    let mut tiles = Grid::filled(size, Tile::new(Terrain::Floor));
    let (r, b) = (w as i32 - 1, h as i32 - 1);
    for y in 0..=b {
        for x in 0..=r {
            if x == 0 || y == 0 || x == r || y == b {
                tiles.set(Pos::new(x, y), Tile::new(Terrain::Wall));
            }
        }
    }
    Floor {
        level: 0,
        tiles,
        light: Grid::filled(size, 0),
        glow: Grid::filled(size, (0, 0, 0)),
        rooms: Vec::new(),
        exits: Vec::new(),
        stations: Vec::new(),
        locks: Vec::new(),
    }
}

/// You on foot on a floor of site `b`'s live copy, in a camera's cone: on the first tile
/// of its cone it sees by the floor's own light, trying the copy's cameras in turn. That
/// camera.
pub(crate) fn in_a_floor_cameras_cone(st: &mut AppState, b: BuildingId) -> Camera {
    let mut levels: Vec<i32> = (crate::sitenet::layer_of(st, b).expect("a live copy"))
        .devices
        .iter()
        .filter(|d| d.kind == DeviceKind::Camera)
        .map(|d| d.level)
        .collect();
    levels.sort();
    levels.dedup();
    for level in levels {
        let city = st.city.as_ref().expect("a city");
        if st.interiors.ensure(city, b, level).is_none() {
            continue;
        }
        let place = Place::Floor { building: b, level };
        for cam in crate::cameras::floor_cameras(st, b, level) {
            let city = st.city.as_ref().expect("a city");
            let (plan, floor) = st.interiors.floor(b, level).expect("its floor");
            let map = MapRef::Floor(FloorMap { city, plan, floor });
            let spot = (crate::cameras::floor_cone(floor, &cam).into_iter())
                .filter(|p| floor.tiles.get(*p).is_some_and(|t| t.is_walkable()))
                .filter(|p| crate::foes::foe_at(st, place, *p).is_none())
                .find(|p| crate::cameras::sees(&map, &cam, *p));
            if let Some(at) = spot {
                stand_on_floor(st, b, level, at);
                return cam;
            }
        }
    }
    panic!("no camera of {b:?}'s copy sees a tile of its cone");
}
