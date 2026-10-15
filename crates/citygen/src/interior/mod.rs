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
    for e in &floor.exits {
        if floor.tiles[e.at].terrain != Terrain::Door {
            return Err(format!("exit at {:?} is not a door", e.at));
        }
        if e.stratum != crate::world::StratumKind::Heights && !frame.border().any(|p| p == e.at) {
            return Err(format!("exit at {:?} is not on the border", e.at));
        }
        clear(floor.doorstep(e), "a doorstep")?;
    }
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
    }
    for s in &floor.stations {
        clear(s.pos, "a station")?;
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
            .find(|b| b.kind == BuildingKind::Megablock && b.span.contains(StratumKind::Skyways))
            .expect("a megablock with a skyways level");
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
}
