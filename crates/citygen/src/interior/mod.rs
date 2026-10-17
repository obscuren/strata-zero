//! Interiors: a closed building's floors, planned once and generated one at a time,
//! deterministically from the city seed, the building and the floor.

mod floor;
mod layouts;
mod plan;

pub use floor::*;
pub use plan::*;

use crate::pathing::components;
use crate::tile::{Feature, Terrain};
use crate::world::City;

/// Builds one floor of a plan.
pub fn generate_floor(city: &City, plan: &FloorPlan, level: i32) -> Floor {
    layouts::generate(city, plan, level)
}

/// The rules every floor keeps: one walkable piece that holds the shafts and the
/// doorsteps, exits that are doors on the border with a clear doorstep, locked
/// rooms with a door, nothing standing where people must pass.
pub fn validate_floor(plan: &FloorPlan, floor: &Floor) -> Result<(), String> {
    let size = floor.tiles.size();
    if size != plan.size {
        return Err(format!(
            "floor {} is {:?}, the plan says {:?}",
            floor.level, size, plan.size
        ));
    }
    let comps = components(size, |p| floor.tiles[p].is_walkable());
    if comps.len() != 1 {
        return Err(format!("floor {} is {} pieces", floor.level, comps.len()));
    }
    let clear = |p: crate::geom::Pos, what: &str| -> Result<(), String> {
        let t = &floor.tiles[p];
        if matches!(t.feature, Some(Feature::Furniture(_))) {
            return Err(format!("furniture on {what} at {p:?}"));
        }
        if !t.is_walkable() {
            return Err(format!("{what} at {p:?} is not walkable"));
        }
        Ok(())
    };
    for p in plan.lift_bank() {
        if floor.tiles[p].terrain != Terrain::Lift {
            return Err(format!("no lift at {p:?} on floor {}", floor.level));
        }
    }
    if plan.floors() >= 2 && floor.tiles[plan.stair].terrain != Terrain::Stair {
        return Err(format!(
            "no stair at {:?} on floor {}",
            plan.stair, floor.level
        ));
    }
    let frame = size.rect();
    let doorsteps: Vec<crate::geom::Pos> = floor.exits.iter().map(|e| floor.doorstep(e)).collect();
    for e in &floor.exits {
        if floor.tiles[e.at].terrain != Terrain::Door {
            return Err(format!("exit at {:?} is not a door", e.at));
        }
        if e.stratum != crate::world::StratumKind::Heights && !frame.border().any(|p| p == e.at) {
            return Err(format!("exit at {:?} is not on the border", e.at));
        }
        clear(floor.doorstep(e), "a doorstep")?;
    }
    let private: Vec<&Room> = floor.rooms.iter().filter(|r| r.private).collect();
    for r in &floor.rooms {
        if r.private && r.door.is_none() {
            return Err(format!("locked {:?} without a door", r.kind));
        }
        if let Some(d) = r.door
            && floor.tiles[d].terrain != Terrain::Door
        {
            return Err(format!("{:?}'s door at {d:?} is not a door", r.kind));
        }
        if r.rect.intersection(frame.inset(1)) != Some(r.rect) {
            return Err(format!("{:?} at {:?} leaves the floor", r.kind, r.rect));
        }
        if r.private {
            // Sealed: the ring around the room is wall, window or its one door.
            let ring = r.rect.grow(1);
            for p in ring.border() {
                // A corner of the ring touches the room only diagonally.
                if !frame.contains(p)
                    || ((p.x == ring.x || p.x == ring.right() - 1)
                        && (p.y == ring.y || p.y == ring.bottom() - 1))
                {
                    continue;
                }
                let t = &floor.tiles[p];
                let ok = match t.terrain {
                    Terrain::Wall | Terrain::Window => true,
                    Terrain::Door => Some(p) == r.door,
                    _ => false,
                };
                if !ok {
                    return Err(format!(
                        "locked {:?} {} at {:?} has a gap at {p:?} ({:?})",
                        r.kind, r.number, r.rect, t.terrain
                    ));
                }
            }
            // And nothing of the open floor spills in: no doorstep, no station of
            // another trade.
            if doorsteps.iter().any(|d| r.rect.contains(*d)) {
                return Err(format!("locked {:?} holds an exit's doorstep", r.kind));
            }
        }
    }
    for s in &floor.stations {
        clear(s.pos, "a station")?;
        if doorsteps.contains(&s.pos) {
            return Err(format!(
                "a {:?} station on a doorstep at {:?}",
                s.role, s.pos
            ));
        }
        if s.role != Role::Desk && private.iter().any(|r| r.rect.contains(s.pos)) {
            return Err(format!(
                "a {:?} station inside a locked room at {:?}",
                s.role, s.pos
            ));
        }
    }
    // Every exit reaches the shafts without crossing a locked room.
    if plan.floors() >= 2 {
        let shafts: Vec<crate::geom::Pos> =
            plan.lift_bank().into_iter().chain([plan.stair]).collect();
        for e in &floor.exits {
            let step = floor.doorstep(e);
            let reached = crate::pathing::bfs_nearest(
                size,
                step,
                |p| floor.tiles[p].is_walkable() && !private.iter().any(|r| r.rect.contains(p)),
                |p| shafts.contains(&p),
                4096,
            );
            if reached.is_none() {
                return Err(format!(
                    "the {:?} exit at {:?} cannot reach the shafts without a locked room",
                    e.stratum, e.at
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::Furniture;
    use crate::{BuildingKind, GenParams, Generator, SizePreset, StratumKind};

    fn city() -> City {
        Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 8,
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn plans_follow_height_class_span_and_kind() {
        let c = city();
        let mut seen_tall = false;
        for b in c.buildings.iter().filter(|b| b.kind.is_closed()) {
            let plan = FloorPlan::new(&c, b.id).expect("a plan for a closed building");
            assert_eq!(plan.size.w as i32, (b.footprint.w * 3).max(12));
            assert_eq!(plan.size.h as i32, (b.footprint.h * 3).max(9));
            assert_eq!(
                plan.lowest,
                if b.span.contains(StratumKind::Undercity) {
                    -1
                } else {
                    0
                }
            );
            match b.span.hi {
                StratumKind::Street => assert!(plan.top <= 2),
                StratumKind::Skyways => assert!((3..=8).contains(&plan.top)),
                StratumKind::Heights => {
                    assert!(plan.top >= 7);
                    assert!(plan.roof);
                    seen_tall = true;
                }
                _ => unreachable!(),
            }
            let has_bridge_door = c
                .portals(b.id)
                .iter()
                .any(|(k, _)| *k == StratumKind::Skyways);
            assert_eq!(plan.bridge.is_some(), has_bridge_door);
            if let Some(f) = plan.bridge {
                assert!(f >= 1 && f <= plan.top);
            }
            assert_eq!(plan.lift.is_some(), plan.floors() >= 3);
            assert_eq!(plan.levels().count() as i32, plan.floors());
        }
        assert!(seen_tall);
        let open = c.buildings.iter().find(|b| !b.kind.is_closed()).unwrap();
        assert!(FloorPlan::new(&c, open.id).is_none());
        assert_eq!(FloorPlan::label(-1), "B1");
        assert_eq!(FloorPlan::label(0), "L");
        assert_eq!(FloorPlan::label(12), "12F");
    }

    #[test]
    fn a_tower_lobby_has_a_counter_lifts_stairs_and_its_street_exits() {
        let c = city();
        let b = c
            .buildings
            .iter()
            .find(|b| b.kind == BuildingKind::CorpTower)
            .unwrap();
        let plan = FloorPlan::new(&c, b.id).unwrap();
        let lobby = generate_floor(&c, &plan, 0);
        validate_floor(&plan, &lobby).unwrap();
        assert!(lobby.rooms.iter().any(|r| r.kind == RoomKind::Lobby));
        assert!(lobby.stations.iter().any(|s| s.role == Role::Reception));
        assert!(
            lobby
                .tiles
                .iter()
                .any(|(_, t)| t.feature == Some(Feature::Furniture(Furniture::Counter)))
        );
        assert!(
            lobby.tiles.count(|t| t.terrain == Terrain::Lift) >= 2,
            "a lift bank"
        );
        assert_eq!(lobby.tiles.count(|t| t.terrain == Terrain::Stair), 1);
        let street_doors: Vec<_> = c
            .portals(b.id)
            .into_iter()
            .filter(|(k, _)| *k == StratumKind::Street)
            .collect();
        assert_eq!(lobby.exits.len(), street_doors.len());
        for e in &lobby.exits {
            assert!(street_doors.iter().any(|(_, d)| *d == e.door));
            assert_eq!(lobby.tiles[e.at].terrain, Terrain::Door);
            assert!(
                c.tile(StratumKind::Street, e.landing)
                    .unwrap()
                    .is_outdoor_ground()
            );
            let fp = b.footprint;
            let dx = ((e.door.x - fp.x) * 3 + 1).clamp(1, plan.size.w as i32 - 2);
            let dy = ((e.door.y - fp.y) * 3 + 1).clamp(1, plan.size.h as i32 - 2);
            assert!(
                e.at.x == dx || e.at.y == dy,
                "{:?} for door {:?}",
                e.at,
                e.door
            );
        }
        assert!(lobby.light.as_slice().iter().any(|l| *l > 100));
    }

    #[test]
    fn corridor_floors_have_locked_numbered_flats_and_the_bridge_floor_its_door() {
        let c = city();
        let b = c
            .buildings
            .iter()
            .find(|b| {
                matches!(b.kind, BuildingKind::Megablock | BuildingKind::Apartment)
                    && b.span.contains(StratumKind::Skyways)
            })
            .expect("a residential block with a skyways level");
        let plan = FloorPlan::new(&c, b.id).unwrap();
        let f = generate_floor(&c, &plan, 2);
        validate_floor(&plan, &f).unwrap();
        let flats: Vec<&Room> = f
            .rooms
            .iter()
            .filter(|r| r.kind == RoomKind::Flat)
            .collect();
        assert!(flats.len() >= 2, "{} flats", flats.len());
        for r in &flats {
            assert!(r.private && r.door.is_some());
            assert_eq!(f.tiles[r.door.unwrap()].terrain, Terrain::Door);
            assert!(r.number >= 201 && r.number < 300, "{}", r.number);
        }
        assert!(f.rooms.iter().any(|r| r.kind == RoomKind::Corridor));
        if plan.bridge != Some(2) {
            assert!(f.exits.is_empty());
        }
        if let Some(bf) = plan.bridge {
            let bridge = generate_floor(&c, &plan, bf);
            validate_floor(&plan, &bridge).unwrap();
            assert_eq!(bridge.exits.len(), 1);
            assert_eq!(bridge.exits[0].stratum, StratumKind::Skyways);
            assert!(c.walkable(StratumKind::Skyways, bridge.exits[0].landing));
        }
    }

    #[test]
    fn the_top_floor_opens_onto_the_roof_and_the_basement_onto_the_tunnel() {
        let c = city();
        let b = c
            .buildings
            .iter()
            .find(|b| {
                b.kind.is_closed()
                    && b.span.contains(StratumKind::Heights)
                    && b.span.contains(StratumKind::Undercity)
            })
            .expect("a tower with a basement");
        let plan = FloorPlan::new(&c, b.id).unwrap();
        let top = generate_floor(&c, &plan, plan.top);
        validate_floor(&plan, &top).unwrap();
        let roof = top
            .exits
            .iter()
            .find(|e| e.stratum == StratumKind::Heights)
            .expect("roof door");
        assert_eq!(
            c.tile(StratumKind::Heights, roof.landing).unwrap().terrain,
            Terrain::Door
        );
        assert_eq!(top.tiles[roof.at].terrain, Terrain::Door);
        let cellar = generate_floor(&c, &plan, -1);
        validate_floor(&plan, &cellar).unwrap();
        if c.portals(b.id)
            .iter()
            .any(|(k, _)| *k == StratumKind::Undercity)
        {
            let hatch = cellar
                .exits
                .iter()
                .find(|e| e.stratum == StratumKind::Undercity)
                .expect("basement door");
            assert!(c.walkable(StratumKind::Undercity, hatch.landing));
        }
        assert!(cellar.rooms.iter().any(|r| r.kind == RoomKind::Storage));
        assert!(cellar.stations.iter().any(|s| s.role == Role::Shelf));
    }

    #[test]
    fn every_floor_of_the_fixture_validates_and_repeats() {
        let c = city();
        for b in c.buildings.iter().filter(|b| b.kind.is_closed()) {
            let plan = FloorPlan::new(&c, b.id).unwrap();
            for level in [plan.lowest, 0, plan.top, plan.bridge.unwrap_or(0)] {
                let a = generate_floor(&c, &plan, level);
                validate_floor(&plan, &a).unwrap_or_else(|e| panic!("{:?} {level}: {e}", b.id));
                let again = generate_floor(&c, &plan, level);
                assert_eq!(a.tiles.as_slice(), again.tiles.as_slice());
                assert_eq!(a.stations, again.stations);
            }
        }
    }

    #[test]
    fn the_smallest_footprint_still_fits_a_lobby() {
        let c = city();
        let b = c
            .buildings
            .iter()
            .filter(|b| b.kind.is_closed())
            .min_by_key(|b| b.footprint.area())
            .unwrap();
        let plan = FloorPlan::new(&c, b.id).unwrap();
        let f = generate_floor(&c, &plan, 0);
        validate_floor(&plan, &f).unwrap();
        assert!(!f.exits.is_empty());
        for e in &f.exits {
            let step = f.doorstep(e);
            assert!(f.tiles[step].is_walkable());
        }
    }

    #[test]
    fn bars_clubs_clinics_temples_halls_factories_and_garages_have_their_rooms() {
        let c = city();
        let more: Vec<City> = [8u64, 9, 11]
            .into_iter()
            .map(|seed| {
                Generator::run(GenParams {
                    seed,
                    size: SizePreset::Medium,
                    districts: 7,
                    ..Default::default()
                })
                .unwrap()
            })
            .collect();
        let find = |kind: BuildingKind| -> (&City, &crate::world::Building) {
            std::iter::once(&c)
                .chain(more.iter())
                .find_map(|city| {
                    city.buildings
                        .iter()
                        .find(|b| b.kind == kind)
                        .map(|b| (city, b))
                })
                .unwrap_or_else(|| panic!("a {kind:?} in four cities"))
        };
        let expect =
            |kind: BuildingKind, rooms: &[RoomKind], roles: &[Role]| -> (FloorPlan, Floor) {
                let (city, b) = find(kind);
                let plan = FloorPlan::new(city, b.id).unwrap();
                let f = generate_floor(city, &plan, 0);
                validate_floor(&plan, &f).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
                for r in rooms {
                    assert!(f.rooms.iter().any(|x| x.kind == *r), "{kind:?} lacks {r:?}");
                }
                for r in roles {
                    assert!(
                        f.stations.iter().any(|s| s.role == *r),
                        "{kind:?} lacks {r:?}"
                    );
                }
                assert!(
                    f.rooms.iter().any(|r| r.private),
                    "{kind:?} has a locked room"
                );
                (plan, f)
            };
        expect(
            BuildingKind::Bar,
            &[RoomKind::Bar, RoomKind::Toilets, RoomKind::BackRoom],
            &[Role::Bar, Role::Table],
        );
        let (plan, _) = expect(
            BuildingKind::Club,
            &[RoomKind::Bar, RoomKind::Stage],
            &[Role::Bar, Role::Table],
        );
        assert_eq!(plan.top, 1);
        let (city, _) = find(BuildingKind::Club);
        let vip = generate_floor(city, &plan, 1);
        validate_floor(&plan, &vip).unwrap();
        assert!(
            vip.rooms
                .iter()
                .any(|r| r.kind == RoomKind::Vip && !r.private)
        );
        assert!(vip.stations.iter().any(|s| s.role == Role::Table));
        expect(
            BuildingKind::Clinic,
            &[RoomKind::Waiting, RoomKind::Surgery],
            &[Role::Surgery],
        );
        // Temples are rare (one weight in Old Town): lay one out on a bar's plan.
        {
            let (city, b) = find(BuildingKind::Bar);
            let mut plan = FloorPlan::new(city, b.id).unwrap();
            plan.kind = BuildingKind::Temple;
            let f = generate_floor(city, &plan, 0);
            validate_floor(&plan, &f).unwrap_or_else(|e| panic!("Temple: {e}"));
            for r in [RoomKind::Hall, RoomKind::Shrine] {
                assert!(f.rooms.iter().any(|x| x.kind == r), "Temple lacks {r:?}");
            }
            assert!(f.stations.iter().any(|s| s.role == Role::Shrine));
            assert!(
                f.rooms.iter().any(|r| r.private),
                "Temple has a locked room"
            );
        }
        expect(BuildingKind::Warehouse, &[RoomKind::Hall], &[Role::Shelf]);
        expect(BuildingKind::Depot, &[RoomKind::Hall], &[Role::Shelf]);
        expect(
            BuildingKind::Factory,
            &[RoomKind::Hall, RoomKind::Lockers],
            &[Role::Machine],
        );
        expect(BuildingKind::Garage, &[RoomKind::Bays], &[Role::Bay]);
    }

    #[test]
    fn locked_rooms_are_sealed_and_every_exit_reaches_the_shafts() {
        let mut cities = vec![city()];
        for seed in [8u64, 9] {
            cities.push(
                Generator::run(GenParams {
                    seed,
                    size: SizePreset::Medium,
                    districts: 7,
                    ..Default::default()
                })
                .unwrap(),
            );
        }
        let (mut floors, mut locked) = (0, 0);
        for c in &cities {
            for b in c.buildings.iter().filter(|b| b.kind.is_closed()) {
                let plan = FloorPlan::new(c, b.id).unwrap();
                let mut levels = vec![plan.lowest, 0, plan.top];
                levels.extend(plan.bridge);
                levels.push(1.min(plan.top));
                levels.sort();
                levels.dedup();
                for level in levels {
                    let f = generate_floor(c, &plan, level);
                    validate_floor(&plan, &f).unwrap_or_else(|e| {
                        panic!("seed {} {:?} {:?} {level}: {e}", c.seed, b.kind, b.id)
                    });
                    floors += 1;
                    locked += f.rooms.iter().filter(|r| r.private).count();
                }
                // Every portal opens onto ground someone can stand on.
                for (k, door) in c.portals(b.id) {
                    if k == StratumKind::Heights {
                        continue;
                    }
                    let out = super::layouts::outward(b.footprint, door);
                    let t = c.tile(k, out).expect("outward tile in bounds");
                    assert!(
                        crate::tile::Tile::new(t.terrain).is_walkable(),
                        "seed {} {:?} {k:?} door {door:?} faces {:?}",
                        c.seed,
                        b.id,
                        t.terrain
                    );
                }
            }
        }
        assert!(
            floors > 300 && locked > 300,
            "{floors} floors, {locked} locked rooms"
        );
    }

    /// Coordinates the smoke and screenshot scripts drive to on seed 8, Medium:
    /// `cargo test -p citygen print_seed8_spots -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn print_seed8_spots() {
        let c = Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap();
        for kind in [
            BuildingKind::CorpTower,
            BuildingKind::Megablock,
            BuildingKind::Club,
            BuildingKind::Bar,
        ] {
            let b = c.buildings.iter().find(|b| b.kind == kind).unwrap();
            let plan = FloorPlan::new(&c, b.id).unwrap();
            let fp = b.footprint;
            let doors: Vec<_> = c
                .portals(b.id)
                .into_iter()
                .filter(|(k, _)| *k == StratumKind::Street)
                .collect();
            let (_, door) = doors[0];
            let outside = door
                .neighbors4()
                .find(|n| !fp.contains(*n) && c.walkable(StratumKind::Street, *n))
                .unwrap();
            let lobby = generate_floor(&c, &plan, 0);
            let exit = lobby.exits.iter().find(|e| e.door == door).unwrap();
            // The Net side: the gate, the nearest port, the construct's rooms.
            if let Some(k) = crate::construct::generate_construct(&c, b.id) {
                let gate = fp
                    .border()
                    .find(|p| {
                        c.tile(StratumKind::Net, *p)
                            .is_some_and(|t| t.terrain == crate::tile::Terrain::NetGate)
                    })
                    .unwrap();
                let port = c
                    .connectors
                    .iter()
                    .filter(|k| k.kind == crate::world::ConnectorKind::Jack)
                    .min_by_key(|k| k.pos.dist2(gate))
                    .map(|k| k.pos);
                let nodes: Vec<String> = k
                    .nodes
                    .iter()
                    .map(|(id, p)| format!("{}@{},{}", c.net.node(*id).kind.label(), p.x, p.y))
                    .collect();
                let outside = [
                    crate::geom::Dir::N,
                    crate::geom::Dir::S,
                    crate::geom::Dir::E,
                    crate::geom::Dir::W,
                ]
                .into_iter()
                .find_map(|d| {
                    let o = gate.step(d);
                    (!fp.contains(o) && c.walkable(StratumKind::Net, o))
                        .then_some((o, d.opposite()))
                });
                let port_stand =
                    port.and_then(|p| p.neighbors8().find(|n| c.walkable(StratumKind::Street, *n)));
                let beside = |pos: crate::geom::Pos| {
                    pos.neighbors4()
                        .find(|n| k.tiles.get(*n).is_some_and(|t| t.is_walkable()))
                };
                let lock_beside = k
                    .nodes
                    .iter()
                    .find(|(id, _)| c.net.node(*id).kind == crate::net::NodeKind::DoorLock)
                    .and_then(|(_, p)| beside(*p));
                println!(
                    "  gate outside {:?} step {:?} · stand by port {:?} · beside lock {:?}",
                    outside.map(|(o, _)| (o.x, o.y)),
                    outside.map(|(_, d)| d),
                    port_stand.map(|p| (p.x, p.y)),
                    lock_beside.map(|p| (p.x, p.y))
                );
                let guards: Vec<String> = k
                    .guards
                    .iter()
                    .map(|g| format!("{}@{},{}", g.grade.label(), g.pos.x, g.pos.y))
                    .collect();
                println!(
                    "  net gate {},{} port {:?} security {} size {:?} entry {},{} guards {:?} nodes {:?}",
                    gate.x, gate.y, port, k.security, k.size, k.entry.x, k.entry.y, guards, nodes
                );
            }
            println!(
                "{kind:?} {} door {},{} outside {},{} doorstep {},{} lift {:?} stair {},{} top {} floors {}..={} bridge {:?} roof {}",
                b.name.clone().unwrap_or_default(),
                door.x,
                door.y,
                outside.x,
                outside.y,
                lobby.doorstep(exit).x,
                lobby.doorstep(exit).y,
                plan.lift,
                plan.stair.x,
                plan.stair.y,
                plan.top,
                plan.lowest,
                plan.top,
                plan.bridge,
                plan.roof
            );
        }
    }

    /// A look at a few floors: `cargo test -p citygen print_floors -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn print_floors() {
        let c = city();
        let dump = |plan: &FloorPlan, f: &Floor| {
            let (w, h) = (f.tiles.size().w as i32, f.tiles.size().h as i32);
            println!(
                "{:?} {} ({:?}) {w}x{h}",
                plan.kind,
                FloorPlan::label(f.level),
                plan.floor_type(f.level)
            );
            for y in 0..h {
                let row: String = (0..w)
                    .map(|x| {
                        let p = crate::geom::Pos::new(x, y);
                        let t = &f.tiles[p];
                        if f.stations.iter().any(|s| s.pos == p) {
                            return '@';
                        }
                        match (t.terrain, t.feature) {
                            (_, Some(Feature::Furniture(Furniture::Desk))) => 'd',
                            (_, Some(Feature::Furniture(Furniture::Counter))) => '=',
                            (_, Some(Feature::Furniture(Furniture::Bed))) => 'b',
                            (_, Some(Feature::Furniture(Furniture::Shelf))) => 's',
                            (_, Some(Feature::Furniture(Furniture::Machine))) => 'm',
                            (_, Some(Feature::Furniture(_))) => 'f',
                            (_, Some(Feature::Lamp)) => '*',
                            (Terrain::Wall, _) => '#',
                            (Terrain::Window, _) => 'w',
                            (Terrain::Door, _) => '+',
                            (Terrain::Lift, _) => 'L',
                            (Terrain::Stair, _) => 'S',
                            (Terrain::Floor, _) => '.',
                            _ => '?',
                        }
                    })
                    .collect();
                println!("{row}");
            }
            println!();
        };
        for kind in [
            BuildingKind::CorpTower,
            BuildingKind::Megablock,
            BuildingKind::Hotel,
        ] {
            let b = c.buildings.iter().find(|b| b.kind == kind).unwrap();
            let plan = FloorPlan::new(&c, b.id).unwrap();
            for level in [plan.lowest, 0, 2.min(plan.top), plan.top] {
                dump(&plan, &generate_floor(&c, &plan, level));
            }
        }
        let more: Vec<City> = [8u64, 9, 11]
            .into_iter()
            .map(|seed| {
                Generator::run(GenParams {
                    seed,
                    size: SizePreset::Medium,
                    districts: 7,
                    ..Default::default()
                })
                .unwrap()
            })
            .collect();
        for kind in [
            BuildingKind::Bar,
            BuildingKind::Club,
            BuildingKind::Clinic,
            BuildingKind::Temple,
            BuildingKind::Warehouse,
            BuildingKind::Factory,
            BuildingKind::Garage,
        ] {
            let Some((city, b)) = std::iter::once(&c).chain(more.iter()).find_map(|city| {
                city.buildings
                    .iter()
                    .find(|b| b.kind == kind)
                    .map(|b| (city, b))
            }) else {
                println!("no {kind:?} in four cities");
                continue;
            };
            let plan = FloorPlan::new(city, b.id).unwrap();
            for level in 0..=plan.top {
                dump(&plan, &generate_floor(city, &plan, level));
            }
        }
    }

    /// A cracked lock always opens something: every closed building keeps at least
    /// one locked room on some floor.
    #[test]
    fn every_closed_building_keeps_a_locked_room() {
        let c = crate::generate::cyberspace::tests::city();
        let mut seen = std::collections::HashSet::new();
        for b in &c.buildings {
            let Some(plan) = FloorPlan::new(&c, b.id) else {
                continue;
            };
            seen.insert(b.kind);
            let any = (plan.lowest..=plan.top)
                .any(|l| generate_floor(&c, &plan, l).rooms.iter().any(|r| r.private));
            assert!(any, "{:?} ({:?}) has nothing locked", b.kind, b.id);
        }
        assert!(seen.len() >= 6, "{seen:?}");
    }

    /// The Medium cities with a megablock big enough for an atrium.
    fn atrium_cities() -> Vec<City> {
        [1u64, 2, 8, 11]
            .into_iter()
            .map(|seed| {
                Generator::run(GenParams {
                    seed,
                    size: SizePreset::Medium,
                    districts: 7,
                    ..Default::default()
                })
                .unwrap()
            })
            .collect()
    }

    fn atrium_plans(c: &City) -> Vec<FloorPlan> {
        c.buildings
            .iter()
            .filter_map(|b| FloorPlan::new(c, b.id))
            .filter(|p| p.atrium.is_some())
            .collect()
    }

    #[test]
    fn atrium_floors_validate_and_share_one_atrium() {
        let cities = atrium_cities();
        let mut seen = 0;
        for c in &cities {
            for plan in atrium_plans(c) {
                seen += 1;
                let a = plan.atrium.unwrap();
                assert_eq!(a, plan.size.rect().inset(BAND + 1 + WALK));
                assert!(
                    !a.contains(plan.stair) && plan.lift_bank().iter().all(|p| !a.contains(*p))
                );
                assert_eq!(plan.floor_type(0), FloorType::Plaza);
                for level in plan.levels() {
                    let f = generate_floor(c, &plan, level);
                    validate_floor(&plan, &f)
                        .unwrap_or_else(|e| panic!("seed {} level {level}: {e}", c.seed));
                    if level >= 1 {
                        assert_eq!(plan.floor_type(level), FloorType::Atrium);
                        for p in a.border() {
                            assert_eq!(f.tiles[p].terrain, Terrain::Railing, "{p:?}");
                        }
                        for p in a.inset(1).iter() {
                            assert_eq!(f.tiles[p].terrain, Terrain::Atrium);
                        }
                        let flats: Vec<&Room> = f
                            .rooms
                            .iter()
                            .filter(|r| r.kind == RoomKind::Flat && r.private)
                            .collect();
                        assert!(flats.len() >= 8, "level {level} has {} flats", flats.len());
                        for r in &flats {
                            assert_eq!(r.number as i32 / 100, level, "{}", r.number);
                        }
                    }
                }
                assert!(plan.corridor_floors().len() as i32 >= plan.top);
            }
        }
        assert_eq!(
            seen, 4,
            "one atrium megablock in each of seeds 1, 2, 8 and 11"
        );
    }

    #[test]
    fn small_megablocks_keep_corridor_floors() {
        // A hand-shrunk copy: a Megablock under the threshold plans no atrium.
        let mut c = atrium_cities().swap_remove(3);
        let b = c
            .buildings
            .iter()
            .position(|b| b.kind == BuildingKind::Megablock)
            .unwrap();
        c.buildings[b].footprint.w = 10;
        c.buildings[b].footprint.h = 8;
        let plan = FloorPlan::new(&c, c.buildings[b].id).unwrap();
        assert!(plan.atrium.is_none());
        assert_eq!(plan.floor_type(0), FloorType::Lobby);
        assert_eq!(plan.floor_type(1), FloorType::Corridor);
    }
}
