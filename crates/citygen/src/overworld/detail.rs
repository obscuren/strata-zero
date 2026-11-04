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
use super::{Ground, Overworld, WorldTile};
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
    p.crop(x0, y0, w, h)
}

/// A rectangle of the detail world while it is being built.
struct Patch<'a> {
    world: &'a Overworld,
    x0: i32,
    y0: i32,
    grid: Grid<Tile>,
}

impl<'a> Patch<'a> {
    fn new(world: &'a Overworld, x0: i32, y0: i32, w: u32, h: u32) -> Patch<'a> {
        Patch {
            world,
            x0,
            y0,
            grid: Grid::filled(Size { w, h }, Tile::new(Terrain::Void)),
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
#[allow(dead_code)]
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
