//! One level down: the walkable detail map of an overworld tile.
//!
//! One overworld tile is 8×8 detail tiles, the street's own `Tile`s, so walking, sight
//! and the renderer work on open ground as they do in the city. Detail coordinates are
//! world-wide: tile `(tx, ty)` covers `8tx..8tx+8` by `8ty..8ty+8`. Everything that
//! must agree across a tile edge (noise, crossings, hashes) comes from world-wide
//! coordinates and the seed, and every build covers its rectangle plus a margin wide
//! enough for every rule's reach before it crops, so a tile built alone is exactly the
//! same tile built inside any region. See `2026-11-04-world-tiles-design.md`.

use super::field::Fbm;
use super::{Ground, Overworld, Water, WorldTile};
use crate::geom::{Grid, Pos, Size};
use crate::rng::{hash64, tile_hash};
use crate::tile::{Terrain, Tile};
use crate::world::StratumKind;

/// Detail tiles to an overworld tile, each way.
pub const TILE: i32 = 8;

/// How far any rule looks past the cell it sets: builds cover this much more, then crop.
const MARGIN: i32 = 8;

/// The 8×8 detail patch of overworld tile `(tx, ty)`.
pub fn ground(world: &Overworld, tx: i32, ty: i32) -> Grid<Tile> {
    region(world, TILE * tx, TILE * ty, TILE as u32, TILE as u32)
}

/// Any rectangle of detail coordinates; cells outside the world are `Void`.
pub fn region(world: &Overworld, x0: i32, y0: i32, w: u32, h: u32) -> Grid<Tile> {
    let mut p = Patch::new(
        world,
        x0 - MARGIN,
        y0 - MARGIN,
        w + 2 * MARGIN as u32,
        h + 2 * MARGIN as u32,
    );
    p.cover();
    p.water();
    p.shores();
    p.crop(x0, y0, w, h)
}

/// A rectangle of the detail world while it is being built.
struct Patch<'a> {
    world: &'a Overworld,
    x0: i32,
    y0: i32,
    grid: Grid<Tile>,
    /// What water a cell is: 0 none, 1 the sea, 2 a lake, 3 a river.
    wet: Grid<u8>,
}

const SEA: u8 = 1;
const LAKE: u8 = 2;
#[allow(dead_code)]
const RIVER: u8 = 3;

impl<'a> Patch<'a> {
    fn new(world: &'a Overworld, x0: i32, y0: i32, w: u32, h: u32) -> Patch<'a> {
        Patch {
            world,
            x0,
            y0,
            grid: Grid::filled(Size { w, h }, Tile::new(Terrain::Void)),
            wet: Grid::filled(Size { w, h }, 0),
        }
    }

    /// Every world-wide detail position in the patch.
    fn cells(&self) -> impl Iterator<Item = Pos> + use<> {
        let (x0, y0, Size { w, h }) = (self.x0, self.y0, self.grid.size());
        (0..h as i32).flat_map(move |y| (0..w as i32).map(move |x| Pos::new(x0 + x, y0 + y)))
    }

    fn local(&self, p: Pos) -> Pos {
        Pos::new(p.x - self.x0, p.y - self.y0)
    }

    fn get(&self, p: Pos) -> Option<&Tile> {
        self.grid.get(self.local(p))
    }

    fn set(&mut self, p: Pos, terrain: Terrain) {
        let l = self.local(p);
        if let Some(t) = self.grid.get_mut(l) {
            t.terrain = terrain;
        }
    }

    /// Whether `p` lies inside the world.
    fn in_world(&self, p: Pos) -> bool {
        p.x >= 0
            && p.y >= 0
            && p.x < TILE * self.world.size.w as i32
            && p.y < TILE * self.world.size.h as i32
    }

    /// The ground cover: each cell takes the ground of the overworld tile it falls in,
    /// displaced a little by noise so kinds meet in curves, with rocks in the hills,
    /// passes through the mountains and low rises on open dirt.
    fn cover(&mut self) {
        let w = self.world;
        let (jx, jy) = (Fbm::new(w.seed, 2001), Fbm::new(w.seed, 2002));
        let (pass, rise) = (Fbm::new(w.seed, 2003), Fbm::new(w.seed, 2004));
        for p in self.cells().collect::<Vec<_>>() {
            if !self.in_world(p) {
                continue;
            }
            let (x, y) = (p.x as f32, p.y as f32);
            let dx = (3.0 * (2.0 * jx.at(x / 9.0, y / 4.5, 2) - 1.0)).round() as i32;
            let dy = (1.5 * (2.0 * jy.at(x / 9.0, y / 4.5, 2) - 1.0)).round() as i32;
            let t = tile_at(w, p.x + dx, p.y + dy);
            let terrain = match t.ground {
                Ground::Flats => Terrain::Dirt,
                Ground::Scrub => Terrain::Scrub,
                Ground::Fields => Terrain::Crops,
                Ground::Salt => Terrain::Salt,
                Ground::Spill => Terrain::Spill,
                Ground::Hills if roll(w.seed, 2005, p) < 0.12 => Terrain::Rock,
                Ground::Hills => Terrain::Dirt,
                Ground::Mountains if pass.at(x / 4.0, y / 2.0, 3) < 0.45 => Terrain::Dirt,
                Ground::Mountains | Ground::Peak => Terrain::Cliff,
            };
            let terrain = if terrain == Terrain::Dirt && rise.at(x / 8.0, y / 4.0, 3) > 0.66 {
                Terrain::Rise
            } else {
                terrain
            };
            self.set(p, terrain);
        }
    }

    fn wet_at(&self, p: Pos) -> u8 {
        self.wet.get(self.local(p)).copied().unwrap_or(0)
    }

    fn set_wet(&mut self, p: Pos, kind: u8) {
        let l = self.local(p);
        if let Some(v) = self.wet.get_mut(l) {
            *v = kind;
        }
    }

    /// The sea and lakes: water where the overworld's water, blended between tile
    /// centres, plus a shoreline noise at three scales (coves, points, the odd spit),
    /// passes one half.
    fn water(&mut self) {
        let w = self.world;
        let n = [
            Fbm::new(w.seed, 2011),
            Fbm::new(w.seed, 2012),
            Fbm::new(w.seed, 2013),
        ];
        for p in self.cells().collect::<Vec<_>>() {
            if !self.in_world(p) {
                continue;
            }
            let (x, y) = (p.x as f32, p.y as f32);
            let shore = 0.34 * (n[0].at(x / 16.0, y / 8.0, 3) - 0.5)
                + 0.22 * (n[1].at(x / 6.0, y / 3.0, 3) - 0.5)
                + 0.10 * (n[2].at(x / 2.5, y / 1.25, 2) - 0.5);
            let sea = blend(w, x, y, |t| (t.water == Water::Sea) as u8 as f32) + shore;
            let lake = blend(w, x, y, |t| (t.water == Water::Lake) as u8 as f32) + shore;
            if sea > 0.5 {
                self.set(p, Terrain::Water);
                self.set_wet(p, SEA);
            } else if lake > 0.5 {
                self.set(p, Terrain::Water);
                self.set_wet(p, LAKE);
            }
        }
    }

    /// The shore bands: water within reach of land is shallows; dry ground beside water
    /// is a beach at the sea and reeds at a lake or river. Rock, cliffs and the spill
    /// keep their own shore.
    fn shores(&mut self) {
        let near = |dx: i32, dy: i32| dx.abs() <= 1 && dy.abs() <= 1 || dx.abs() == 2 && dy == 0;
        let cells: Vec<Pos> = self.cells().collect();
        let mut shallow = Vec::new();
        let mut beach = Vec::new();
        for &p in &cells {
            let wet = self.wet_at(p);
            let ring = (-2..=2)
                .flat_map(|dx| (-1..=1).map(move |dy| (dx, dy)))
                .filter(|&(dx, dy)| (dx, dy) != (0, 0) && near(dx, dy));
            if wet != 0 {
                let by_land = ring
                    .map(|(dx, dy)| p.offset(dx, dy))
                    .any(|q| self.in_world(q) && self.get(q).is_some() && self.wet_at(q) == 0);
                if by_land {
                    shallow.push(p);
                }
            } else if self.in_world(p) {
                let touching: Vec<u8> = (-1..=1)
                    .flat_map(|dx| (-1..=1).map(move |dy| (dx, dy)))
                    .map(|(dx, dy)| self.wet_at(p.offset(dx, dy)))
                    .filter(|k| *k != 0)
                    .collect();
                if !touching.is_empty() {
                    let kind = if touching.contains(&SEA) {
                        Terrain::Sand
                    } else {
                        Terrain::Reeds
                    };
                    beach.push((p, kind));
                }
            }
        }
        for p in shallow {
            self.set(p, Terrain::Shallows);
        }
        for (p, kind) in beach {
            let keeps = self.get(p).is_some_and(|t| {
                matches!(t.terrain, Terrain::Rock | Terrain::Cliff | Terrain::Spill)
            });
            if !keeps {
                self.set(p, kind);
            }
        }
    }

    /// The cells of `x0, y0, w, h`, each with its stable variant.
    fn crop(&self, x0: i32, y0: i32, w: u32, h: u32) -> Grid<Tile> {
        let mut out = Grid::filled(Size { w, h }, Tile::new(Terrain::Void));
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let p = Pos::new(x0 + x, y0 + y);
                if let Some(t) = self.get(p) {
                    let mut t = *t;
                    t.variant = tile_hash(self.world.seed, StratumKind::Street, p);
                    out[Pos::new(x, y)] = t;
                }
            }
        }
        out
    }
}

/// The overworld tile under detail point `(x, y)`, the nearest in-world one past the edge.
fn tile_at(world: &Overworld, x: i32, y: i32) -> &WorldTile {
    let tx = x.div_euclid(TILE).clamp(0, world.size.w as i32 - 1);
    let ty = y.div_euclid(TILE).clamp(0, world.size.h as i32 - 1);
    world
        .tile(Pos::new(tx, ty))
        .expect("clamped into the world")
}

/// `f` blended between overworld tile centres at detail point `(x, y)` (bilinear); the
/// world's outer tiles blend with themselves.
fn blend(world: &Overworld, x: f32, y: f32, f: impl Fn(&WorldTile) -> f32) -> f32 {
    let half = (TILE as f32 - 1.0) / 2.0;
    let (fx, fy) = ((x - half) / TILE as f32, (y - half) / TILE as f32);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (ux, uy) = (fx - x0, fy - y0);
    let at = |dx: i32, dy: i32| {
        f(tile_at(
            world,
            (x0 as i32 + dx) * TILE,
            (y0 as i32 + dy) * TILE,
        ))
    };
    at(0, 0) * (1.0 - ux) * (1.0 - uy)
        + at(1, 0) * ux * (1.0 - uy)
        + at(0, 1) * (1.0 - ux) * uy
        + at(1, 1) * ux * uy
}

/// A detail cell's stable roll in 0..1 under `salt`.
fn roll(seed: u64, salt: u64, p: Pos) -> f32 {
    let packed = ((p.x as u32 as u64) << 32) | p.y as u32 as u64;
    (hash64(hash64(seed, salt), packed) >> 40) as f32 / (1u64 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Pos, Size};
    use crate::overworld::{Ground, Overworld, Water, WorldParams, WorldSize};
    use crate::tile::Terrain;

    #[test]
    fn a_tile_alone_equals_the_same_tile_in_a_region() {
        let w = Overworld::generate(WorldParams {
            seed: 0,
            ..Default::default()
        })
        .unwrap();
        let r = region(&w, 8 * 40, 8 * 20, 8 * 6, 8 * 4);
        for ty in 20..24 {
            for tx in 40..46 {
                let g = ground(&w, tx, ty);
                for y in 0..8 {
                    for x in 0..8 {
                        let a = g[Pos::new(x, y)];
                        let b = r[Pos::new(8 * (tx - 40) + x, 8 * (ty - 20) + y)];
                        assert_eq!(
                            (a.terrain, a.feature, a.variant),
                            (b.terrain, b.feature, b.variant),
                            "tile {tx},{ty} cell {x},{y}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn ground_follows_its_overworld_tile() {
        for seed in 0..5 {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            for (p, t) in w.tiles.iter() {
                if t.water != Water::None || t.road != 0 || t.wash != 0 || t.urban.is_some() {
                    continue;
                }
                let want = match t.ground {
                    Ground::Flats | Ground::Hills => Terrain::Dirt,
                    Ground::Scrub => Terrain::Scrub,
                    Ground::Fields => Terrain::Crops,
                    Ground::Salt => Terrain::Salt,
                    Ground::Spill => Terrain::Spill,
                    Ground::Mountains | Ground::Peak => Terrain::Cliff,
                };
                // the centre of the patch is the tile's own ground (its edges blend)
                let c = ground(&w, p.x, p.y)[Pos::new(4, 4)].terrain;
                assert!(
                    c == want || matches!(c, Terrain::Rock | Terrain::Dirt | Terrain::Rise),
                    "seed {seed} {p:?}: {c:?} for {:?}",
                    t.ground
                );
            }
        }
    }

    /// The whole world's detail, for tests that look across it.
    fn whole(w: &Overworld) -> Grid<Tile> {
        region(w, 0, 0, 8 * w.size.w, 8 * w.size.h)
    }

    #[test]
    fn water_never_touches_dry_land() {
        for seed in 0..3 {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            let r = whole(&w);
            for (p, t) in r.iter() {
                if t.terrain != Terrain::Water {
                    continue;
                }
                for q in p.neighbors8() {
                    if let Some(n) = r.get(q) {
                        assert!(
                            matches!(
                                n.terrain,
                                Terrain::Water
                                    | Terrain::Shallows
                                    | Terrain::Void
                                    | Terrain::Bridge
                                    | Terrain::Railing
                            ),
                            "seed {seed}: deep water at {p:?} next to {:?}",
                            n.terrain
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_sea_tile_is_water_and_its_shore_is_sand() {
        let w = Overworld::generate(WorldParams {
            seed: 0,
            ..Default::default()
        })
        .unwrap();
        let (p, _) = w
            .tiles
            .iter()
            .find(|(p, t)| {
                t.water == Water::Sea
                    && p.neighbors8()
                        .all(|q| w.tile(q).is_some_and(|n| n.water == Water::Sea))
            })
            .unwrap();
        assert!(
            ground(&w, p.x, p.y)
                .iter()
                .all(|(_, t)| matches!(t.terrain, Terrain::Water | Terrain::Shallows))
        );
        assert!(
            whole(&w).count(|t| t.terrain == Terrain::Sand) > 50,
            "a coast has a beach"
        );
    }

    #[test]
    fn the_world_edge_and_corners_build() {
        let w = Overworld::generate(WorldParams {
            seed: 3,
            size: WorldSize::Small,
            ..Default::default()
        })
        .unwrap();
        let (mw, mh) = (w.size.w as i32, w.size.h as i32);
        for (tx, ty) in [
            (0, 0),
            (mw - 1, 0),
            (0, mh - 1),
            (mw - 1, mh - 1),
            (mw / 2, 0),
        ] {
            let g = ground(&w, tx, ty);
            assert_eq!(g.size(), Size { w: 8, h: 8 });
        }
        let r = region(&w, -8, -8, 24, 24);
        assert_eq!(
            r[Pos::new(0, 0)].terrain,
            Terrain::Void,
            "outside the world is void"
        );
    }
}
