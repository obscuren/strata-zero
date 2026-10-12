//! Stage 8: connectivity repair and public connectors between strata.

use super::{GenCtx, Stage, StageRunner, StepOut};
use crate::geom::{Grid, Pos};
use crate::params::GenError;
use crate::pathing::{bfs_nearest, components};
use crate::tile::Terrain;
use crate::world::{City, Connector, ConnectorKind, StratumKind};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::collections::{HashMap, VecDeque};

const MAX_ROUNDS: u8 = 3;
const REACH_TARGET: f32 = 0.95;

enum Work {
    Round(u8),
    Check(u8),
}

#[derive(Default)]
pub(crate) struct ConnectivityStage {
    queue: VecDeque<Work>,
    rng: Option<ChaCha8Rng>,
}

/// Tiles reachable from the main Street component, walking 4-adjacency and connectors.
/// One grid per physical stratum, indexed by `StratumKind::index()`.
pub(crate) fn reachable_from_street(city: &City) -> Vec<Grid<bool>> {
    let size = city.size;
    let mut reach: Vec<Grid<bool>> = StratumKind::PHYSICAL
        .iter()
        .map(|_| Grid::filled(size, false))
        .collect();
    let street = city.stratum(StratumKind::Street).expect("street");
    let comps = components(size, |p| street.tiles[p].is_walkable());
    let Some(main) = comps.first() else {
        return reach;
    };

    let mut by_pos: HashMap<Pos, Vec<&Connector>> = HashMap::new();
    for c in &city.connectors {
        by_pos.entry(c.pos).or_default().push(c);
    }

    let mut queue: VecDeque<(StratumKind, Pos)> = VecDeque::new();
    for p in main {
        reach[StratumKind::Street.index()].set(*p, true);
        queue.push_back((StratumKind::Street, *p));
    }
    while let Some((k, p)) = queue.pop_front() {
        for n in p.neighbors4() {
            if city.walkable(k, n) && !reach[k.index()][n] {
                reach[k.index()].set(n, true);
                queue.push_back((k, n));
            }
        }
        if let Some(conns) = by_pos.get(&p) {
            for c in conns {
                if !c.serves.contains(&k) {
                    continue;
                }
                for s in &c.serves {
                    if *s != k
                        && *s != StratumKind::Net
                        && city.walkable(*s, p)
                        && !reach[s.index()][p]
                    {
                        reach[s.index()].set(p, true);
                        queue.push_back((*s, p));
                    }
                }
            }
        }
    }
    reach
}

/// What carving a non-walkable tile turns it into on a given stratum, if anything.
fn carve_into(k: StratumKind, t: Terrain, in_building: bool) -> Option<Terrain> {
    match (k, t) {
        (_, Terrain::Wall | Terrain::Window) if in_building => Some(Terrain::Door),
        (StratumKind::Street, Terrain::Void) => Some(Terrain::Alley),
        (StratumKind::Undercity, Terrain::Void) => Some(Terrain::Tunnel),
        (StratumKind::Skyways, Terrain::Void) => Some(Terrain::Walkway),
        (StratumKind::Heights, Terrain::Void) => Some(Terrain::Bridge),
        _ => None,
    }
}

fn carvable(city: &City, k: StratumKind, p: Pos) -> bool {
    city.tile(k, p)
        .is_some_and(|t| carve_into(k, t.terrain, t.building.is_some()).is_some())
}

fn carve_path(city: &mut City, k: StratumKind, path: &[Pos]) {
    for p in path {
        let Some(t) = city.tile(k, *p) else { continue };
        if t.is_walkable() {
            continue;
        }
        if let Some(new) = carve_into(k, t.terrain, t.building.is_some()) {
            let tile = city.tile_mut(k, *p).expect("in bounds");
            tile.terrain = new;
            tile.feature = None;
        }
    }
}

/// Joins every secondary walkable component on Street to the main one, or voids slivers.
pub(crate) fn join_street_components(city: &mut City) {
    let k = StratumKind::Street;
    let size = city.size;
    let comps = {
        let st = city.stratum(k).expect("street");
        components(size, |p| st.tiles[p].is_walkable())
    };
    let Some(main) = comps.first() else { return };
    let total: usize = comps.iter().map(|c| c.len()).sum();
    let mut in_main = Grid::filled(size, false);
    for p in main {
        in_main.set(*p, true);
    }
    for comp in comps.iter().skip(1) {
        let path = bfs_nearest(
            size,
            comp[0],
            |p| city.walkable(k, p) || carvable(city, k, p),
            |p| in_main[p],
            40,
        );
        match path {
            Some(path) => {
                carve_path(city, k, &path);
                for p in comp {
                    in_main.set(*p, true);
                }
                for p in &path {
                    in_main.set(*p, true);
                }
            }
            None => {
                let in_building = comp
                    .iter()
                    .any(|p| city.tile(k, *p).is_some_and(|t| t.building.is_some()));
                if comp.len() * 100 < total && !in_building {
                    for p in comp {
                        if let Some(t) = city.tile_mut(k, *p) {
                            t.terrain = Terrain::Void;
                            t.feature = None;
                        }
                    }
                }
            }
        }
    }
}

/// Joins unreached walkable components on one stratum to reached tiles, or voids slivers.
fn repair_stratum(city: &mut City, k: StratumKind, reach: &Grid<bool>) {
    let size = city.size;
    if reach.count(|&b| b) == 0 {
        return;
    }
    let comps = {
        let st = city.stratum(k).expect("stratum");
        components(size, |p| st.tiles[p].is_walkable() && !reach[p])
    };
    let total = city
        .stratum(k)
        .expect("stratum")
        .tiles
        .count(|t| t.is_walkable());
    let mut reached = reach.clone();
    for comp in comps {
        let path = bfs_nearest(
            size,
            comp[0],
            |p| city.walkable(k, p) || carvable(city, k, p),
            |p| reached[p],
            30,
        );
        match path {
            Some(path) => {
                carve_path(city, k, &path);
                for p in comp.iter().chain(path.iter()) {
                    reached.set(*p, true);
                }
            }
            None => {
                if drop_in_connector(city, k, &comp) {
                    for p in &comp {
                        reached.set(*p, true);
                    }
                    continue;
                }
                let in_building = comp
                    .iter()
                    .any(|p| city.tile(k, *p).is_some_and(|t| t.building.is_some()));
                if comp.len() * 100 < total && !in_building {
                    for p in comp {
                        if let Some(t) = city.tile_mut(k, p) {
                            t.terrain = Terrain::Void;
                            t.feature = None;
                        }
                    }
                }
            }
        }
    }
}

/// Gives an isolated component its own public connector from the street directly below
/// (or, for rooftops, a ladder from a walkway directly below). Returns true on success.
fn drop_in_connector(city: &mut City, k: StratumKind, comp: &[Pos]) -> bool {
    let street_ok = |city: &City, p: Pos| {
        city.tile(StratumKind::Street, p).is_some_and(|t| {
            t.feature.is_none()
                && matches!(
                    t.terrain,
                    Terrain::Sidewalk | Terrain::Alley | Terrain::Plaza | Terrain::Road
                )
        }) && city.connectors_at(p).next().is_none()
    };
    let spot = comp.iter().copied().find(|p| {
        city.tile(k, *p)
            .is_some_and(|t| t.feature.is_none() && !t.is_connector())
            && match k {
                StratumKind::Heights => {
                    city.walkable(StratumKind::Skyways, *p)
                        && city.connectors_at(*p).next().is_none()
                }
                _ => street_ok(city, *p),
            }
    });
    let Some(pos) = spot else { return false };
    let (kind, serves) = match k {
        StratumKind::Undercity => (
            ConnectorKind::Manhole,
            vec![StratumKind::Undercity, StratumKind::Street],
        ),
        StratumKind::Skyways => (
            ConnectorKind::Stair,
            vec![StratumKind::Street, StratumKind::Skyways],
        ),
        StratumKind::Heights => (
            ConnectorKind::Ladder,
            vec![StratumKind::Skyways, StratumKind::Heights],
        ),
        _ => return false,
    };
    ConnectivityStage::add_connector(city, kind, pos, serves);
    true
}

impl ConnectivityStage {
    fn has_public(city: &City, d: usize, target: StratumKind) -> bool {
        let id = city.districts[d].id;
        city.connectors.iter().any(|c| {
            c.public
                && c.serves.contains(&target)
                && city
                    .tile(StratumKind::Street, c.pos)
                    .is_some_and(|t| t.district == Some(id))
        })
    }

    /// Street tiles of a district that may host a public connector.
    fn street_spots(city: &City, d: usize, allow_plaza: bool) -> Vec<Pos> {
        let dist = &city.districts[d];
        let st = city.stratum(StratumKind::Street).expect("street");
        dist.bounds
            .iter()
            .filter(|p| {
                st.tiles.get(*p).is_some_and(|t| {
                    t.district == Some(dist.id)
                        && t.feature.is_none()
                        && matches!(t.terrain, Terrain::Sidewalk | Terrain::Alley)
                            | (allow_plaza && t.terrain == Terrain::Plaza)
                })
            })
            .filter(|p| city.connectors_at(*p).next().is_none())
            .collect()
    }

    fn add_connector(city: &mut City, kind: ConnectorKind, pos: Pos, serves: Vec<StratumKind>) {
        let terrain = match kind {
            ConnectorKind::Stair => Terrain::Stair,
            ConnectorKind::Lift => Terrain::Lift,
            _ => Terrain::Ladder,
        };
        for k in &serves {
            if let Some(t) = city.tile_mut(*k, pos) {
                t.terrain = terrain;
                t.feature = None;
            }
        }
        city.connectors.push(Connector {
            kind,
            pos,
            serves,
            public: true,
        });
    }

    /// A public link from Street down (Manhole) or up (Stair) via `other`, carving if needed.
    fn link_street(city: &mut City, rng: &mut ChaCha8Rng, d: usize, other: StratumKind) {
        let kind = if other == StratumKind::Undercity {
            ConnectorKind::Manhole
        } else {
            ConnectorKind::Stair
        };
        let serves = if other == StratumKind::Undercity {
            vec![StratumKind::Undercity, StratumKind::Street]
        } else {
            vec![StratumKind::Street, StratumKind::Skyways]
        };
        let spots = Self::street_spots(city, d, other == StratumKind::Skyways);
        if spots.is_empty() {
            return;
        }
        let direct: Vec<Pos> = spots
            .iter()
            .copied()
            .filter(|p| city.walkable(other, *p))
            .collect();
        if !direct.is_empty() {
            let pos = direct[rng.random_range(0..direct.len())];
            Self::add_connector(city, kind, pos, serves);
            return;
        }
        // Carve from under/over a random spot to the nearest walkable tile on `other`.
        for _ in 0..8 {
            let pos = spots[rng.random_range(0..spots.len())];
            if city.terrain_at(other, pos) != Terrain::Void {
                continue;
            }
            let path = bfs_nearest(
                city.size,
                pos,
                |p| carvable(city, other, p),
                |p| city.walkable(other, p),
                30,
            );
            if let Some(path) = path {
                carve_path(city, other, &path);
                Self::add_connector(city, kind, pos, serves);
                return;
            }
        }
    }

    /// A fire ladder from a Skyways tile beside a tall building up to its roof edge.
    fn link_heights(city: &mut City, rng: &mut ChaCha8Rng, d: usize) {
        let id = city.districts[d].id;
        let talls: Vec<_> = city
            .buildings
            .iter()
            .filter(|b| b.district == id && b.span.contains(StratumKind::Heights))
            .map(|b| b.footprint)
            .collect();
        for fp in talls {
            let spots: Vec<Pos> = fp
                .grow(1)
                .border()
                .filter(|p| !fp.contains(*p))
                .filter(|p| city.walkable(StratumKind::Skyways, *p))
                .filter(|p| city.terrain_at(StratumKind::Heights, *p) == Terrain::Void)
                .filter(|p| city.connectors_at(*p).next().is_none())
                .filter(|p| p.neighbors4().any(|n| fp.contains(n)))
                .collect();
            if spots.is_empty() {
                continue;
            }
            let pos = spots[rng.random_range(0..spots.len())];
            Self::add_connector(
                city,
                ConnectorKind::Ladder,
                pos,
                vec![StratumKind::Skyways, StratumKind::Heights],
            );
            return;
        }
    }

    fn round(&mut self, ctx: &mut GenCtx) {
        let rng = self.rng.as_mut().expect("begin sets rng");
        let city = &mut ctx.city;
        for d in 0..city.districts.len() {
            if !Self::has_public(city, d, StratumKind::Undercity) {
                Self::link_street(city, rng, d, StratumKind::Undercity);
            }
            if !Self::has_public(city, d, StratumKind::Skyways) {
                Self::link_street(city, rng, d, StratumKind::Skyways);
            }
            if !Self::has_public(city, d, StratumKind::Heights) {
                Self::link_heights(city, rng, d);
            }
        }
        join_street_components(city);
        let reach = reachable_from_street(city);
        for k in [
            StratumKind::Undercity,
            StratumKind::Skyways,
            StratumKind::Heights,
        ] {
            repair_stratum(city, k, &reach[k.index()]);
        }
    }

    fn check(city: &City) -> Result<(), StratumKind> {
        let reach = reachable_from_street(city);
        for k in StratumKind::PHYSICAL {
            let walk = city
                .stratum(k)
                .expect("stratum")
                .tiles
                .count(|t| t.is_walkable());
            if walk == 0 {
                continue;
            }
            let got = reach[k.index()].count(|&b| b);
            if (got as f32) < REACH_TARGET * walk as f32 {
                return Err(k);
            }
        }
        Ok(())
    }
}

impl City {
    fn terrain_at(&self, k: StratumKind, p: Pos) -> Terrain {
        self.tile(k, p).map(|t| t.terrain).unwrap_or(Terrain::Void)
    }
}

impl StageRunner for ConnectivityStage {
    fn begin(&mut self, ctx: &mut GenCtx) -> Result<(), GenError> {
        self.rng = Some(ctx.rng(Stage::Connectivity));
        self.queue.clear();
        for r in 0..MAX_ROUNDS {
            self.queue.push_back(Work::Round(r));
            self.queue.push_back(Work::Check(r));
        }
        Ok(())
    }

    fn step(&mut self, ctx: &mut GenCtx) -> Result<StepOut, GenError> {
        let Some(work) = self.queue.pop_front() else {
            return Ok(StepOut::Finished);
        };
        match work {
            Work::Round(r) => {
                self.round(ctx);
                Ok(StepOut::Continue((r as f32 + 0.5) / MAX_ROUNDS as f32))
            }
            Work::Check(r) => match Self::check(&ctx.city) {
                Ok(()) => {
                    self.queue.clear();
                    Ok(StepOut::Continue(1.0))
                }
                Err(stratum) if r + 1 >= MAX_ROUNDS => Err(GenError::Unconnected { stratum }),
                Err(_) => Ok(StepOut::Continue((r as f32 + 1.0) / MAX_ROUNDS as f32)),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::testutil::small;
    use crate::geom::{Pos, Rect};
    use crate::params::{GenParams, SizePreset, WaterMode};
    use crate::pathing::components;
    use crate::tile::{Terrain, Tile};
    use crate::world::{City, StratumKind};

    #[test]
    fn every_stratum_is_reachable_from_street() {
        for seed in [71, 72, 73] {
            let c = small(seed, WaterMode::Bay, 7);
            let reach = reachable_from_street(&c);
            for (i, k) in StratumKind::PHYSICAL.iter().enumerate() {
                let st = c.stratum(*k).unwrap();
                let walk = st.tiles.count(|t| t.is_walkable());
                let got = reach[i].count(|&b| b);
                assert!(
                    got as f32 >= 0.95 * walk as f32,
                    "seed {seed} {k:?}: {got}/{walk} reachable"
                );
            }
        }
    }

    #[test]
    fn every_district_has_a_public_connector() {
        let c = small(74, WaterMode::River, 8);
        for d in &c.districts {
            assert!(
                c.connectors.iter().any(|k| {
                    k.public
                        && d.bounds.contains(k.pos)
                        && c.tile(StratumKind::Street, k.pos).unwrap().district == Some(d.id)
                }),
                "{:?}",
                d.id
            );
        }
    }

    #[test]
    fn repair_joins_two_halves() {
        let mut c = City::empty(GenParams {
            size: SizePreset::Custom { w: 80, h: 40 },
            ..Default::default()
        });
        let st = c.stratum_mut(StratumKind::Street).unwrap();
        st.tiles
            .fill_rect(Rect::new(0, 0, 20, 5), Tile::new(Terrain::Sidewalk));
        for y in 0..5 {
            st.tiles.set(Pos::new(10, y), Tile::new(Terrain::Wall));
        }
        join_street_components(&mut c);
        let st = c.stratum(StratumKind::Street).unwrap();
        assert_eq!(
            components(st.tiles.size(), |p| st.tiles[p].is_walkable()).len(),
            1
        );
    }
}
