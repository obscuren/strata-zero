//! Paths for feet and wheels over the city's A*.

use citygen::pathing::astar;
use citygen::world::{City, StratumKind};
use citygen::{Feature, Grid, Pos, Terrain};
use std::collections::VecDeque;

/// The cost of stepping onto `to` on foot: street tiles at 1, roads at 3, nothing else.
/// A sign (a billboard on a lot) stands in a walker's way as it stands in yours; what
/// else stands on the street a walker goes by (a stall's keeper stands in hers).
pub fn walk_cost(city: &City, _from: Pos, to: Pos) -> Option<u32> {
    let tile = city.tile(StratumKind::Street, to)?;
    if matches!(tile.feature, Some(Feature::Sign(_))) {
        return None;
    }
    match tile.terrain {
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

/// Where a walk can go: each cell a walker can stand on numbered (from 1) by the stretch
/// of street a walk from it reaches, stepping as `walk_path` steps; 0 elsewhere. Two
/// cells share a number when a walk goes from one to the other.
pub fn walk_regions(city: &City) -> Grid<u32> {
    let mut region = Grid::filled(city.size, 0u32);
    let mut n = 0;
    for y in 0..city.size.h as i32 {
        for x in 0..city.size.w as i32 {
            let p = Pos::new(x, y);
            if region[p] != 0 || walk_cost(city, p, p).is_none() {
                continue;
            }
            n += 1;
            region.set(p, n);
            let mut queue = VecDeque::from([p]);
            while let Some(q) = queue.pop_front() {
                for r in q.neighbors4() {
                    if region.get(r) == Some(&0) && walk_cost(city, r, r).is_some() {
                        region.set(r, n);
                        queue.push_back(r);
                    }
                }
            }
        }
    }
    region
}

pub fn walk_path(city: &City, from: Pos, to: Pos) -> Option<Vec<Pos>> {
    astar(city.size, from, to, |f, t| walk_cost(city, f, t))
}

pub fn drive_path(city: &City, from: Pos, to: Pos) -> Option<Vec<Pos>> {
    astar(city.size, from, to, |f, t| drive_cost(city, f, t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{GenParams, Generator, SizePreset};

    #[test]
    fn a_sign_stands_in_a_walkers_way() {
        let mut c = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        let k = StratumKind::Street;
        let p = c
            .stratum(k)
            .unwrap()
            .tiles
            .iter()
            .find(|(_, t)| t.terrain == Terrain::Plaza && t.feature.is_none())
            .map(|(p, _)| p)
            .unwrap();
        assert_eq!(walk_cost(&c, p, p), Some(1));
        c.tile_mut(k, p).unwrap().feature = Some(Feature::Sign('▣'));
        assert_eq!(walk_cost(&c, p, p), None, "a billboard on a lot");
    }
}
