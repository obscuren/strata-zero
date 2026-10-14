//! Paths for feet and wheels over the city's A*.

use citygen::pathing::astar;
use citygen::world::{City, StratumKind};
use citygen::{Pos, Terrain};

/// The cost of stepping onto `to` on foot: street tiles at 1, roads at 3, nothing else.
pub fn walk_cost(city: &City, _from: Pos, to: Pos) -> Option<u32> {
    match city.tile(StratumKind::Street, to)?.terrain {
        Terrain::Sidewalk
        | Terrain::Plaza
        | Terrain::Alley
        | Terrain::Walkway
        | Terrain::Bridge
        | Terrain::Stair
        | Terrain::Door => Some(1),
        Terrain::Road => Some(3),
        _ => None,
    }
}

fn carries_cars(t: Terrain) -> bool {
    matches!(t, Terrain::Road | Terrain::Bridge | Terrain::Alley)
}

/// A pedestrian tile with road on both opposite sides: where the street crosses the
/// pavement, and the only pavement a car may touch.
pub fn is_crossing(city: &City, p: Pos) -> bool {
    let t = |q: Pos| {
        city.tile(StratumKind::Street, q)
            .is_some_and(|t| carries_cars(t.terrain))
    };
    let here = city.tile(StratumKind::Street, p).map(|t| t.terrain);
    matches!(here, Some(Terrain::Sidewalk | Terrain::Plaza))
        && ((t(p.offset(0, -1)) && t(p.offset(0, 1))) || (t(p.offset(-1, 0)) && t(p.offset(1, 0))))
}

/// The cost of driving onto `to`: roads and bridges at 1, alleys at 2 (the side
/// streets between blocks are alleys), a crossing at 4, nothing else.
pub fn drive_cost(city: &City, _from: Pos, to: Pos) -> Option<u32> {
    match city.tile(StratumKind::Street, to)?.terrain {
        Terrain::Road | Terrain::Bridge => Some(1),
        Terrain::Alley => Some(2),
        Terrain::Sidewalk | Terrain::Plaza if is_crossing(city, to) => Some(4),
        _ => None,
    }
}

pub fn walk_path(city: &City, from: Pos, to: Pos) -> Option<Vec<Pos>> {
    astar(city.size, from, to, |f, t| walk_cost(city, f, t))
}

pub fn drive_path(city: &City, from: Pos, to: Pos) -> Option<Vec<Pos>> {
    astar(city.size, from, to, |f, t| drive_cost(city, f, t))
}
