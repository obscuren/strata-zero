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
use crate::geom::{Dir, Grid, Pos, Size};
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
    p.rivers();
    p.washes();
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
            if wet != 0 && self.get(p).is_some_and(|t| t.terrain == Terrain::Water) {
                let by_land = ring
                    .map(|(dx, dy)| p.offset(dx, dy))
                    .any(|q| self.in_world(q) && self.get(q).is_some() && self.wet_at(q) == 0);
                if by_land {
                    shallow.push(p);
                }
            } else if wet == 0 && self.in_world(p) {
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

    /// The overworld tiles whose lines (rivers, washes, roads) can reach the patch.
    fn tiles_in_reach(&self) -> Vec<Pos> {
        let Size { w, h } = self.grid.size();
        let (tx0, ty0) = (self.x0.div_euclid(TILE) - 1, self.y0.div_euclid(TILE) - 1);
        let (tx1, ty1) = (
            (self.x0 + w as i32).div_euclid(TILE) + 1,
            (self.y0 + h as i32).div_euclid(TILE) + 1,
        );
        (ty0..=ty1)
            .flat_map(|ty| (tx0..=tx1).map(move |tx| Pos::new(tx, ty)))
            .filter(|t| self.world.tile(*t).is_some())
            .collect()
    }

    /// Stamps a square brush `width` wide round each of `cells` with `f`.
    fn brush(&mut self, cells: &[Pos], width: i32, mut f: impl FnMut(&mut Self, Pos)) {
        let lo = -(width / 2);
        for c in cells {
            for dy in lo..lo + width {
                for dx in lo..lo + width {
                    let p = c.offset(dx, dy);
                    if self.get(p).is_some() && self.in_world(p) {
                        f(self, p);
                    }
                }
            }
        }
    }

    /// Rivers: water along each river tile's line, as wide as the river is there, the
    /// narrow upper reaches with a ford here and there.
    fn rivers(&mut self) {
        let w = self.world;
        let mut fords = Vec::new();
        for t in self.tiles_in_reach() {
            let mask = w.tile(t).unwrap().river;
            if mask == 0 {
                continue;
            }
            let width = river_width(w, t).max(3) as i32;
            let line = centreline(w, t.x, t.y, mask, false, 2021);
            self.brush(&line, width, |p, q| {
                if p.wet_at(q) != SEA {
                    p.set(q, Terrain::Water);
                    p.set_wet(q, RIVER);
                }
            });
            if width == 3 {
                fords.extend(
                    line.iter()
                        .copied()
                        .filter(|c| roll(w.seed, 2022, *c) < 1.0 / 6.0),
                );
            }
        }
        self.brush(&fords, 3, |p, q| {
            if p.wet_at(q) == RIVER {
                p.set(q, Terrain::Ford);
            }
        });
    }

    /// Dry washes: a sandy bed along each wash tile's line, 3 wide (4 in its lower half),
    /// scrub along its banks; never over water.
    fn washes(&mut self) {
        let w = self.world;
        let mut bed = Vec::new();
        for t in self.tiles_in_reach() {
            let mask = w.tile(t).unwrap().wash;
            if mask == 0 {
                continue;
            }
            let lower = w.washes.iter().any(|c| c[c.len() / 2..].contains(&t));
            let line = centreline(w, t.x, t.y, mask, false, 2031);
            self.brush(&line, if lower { 4 } else { 3 }, |p, q| {
                if p.wet_at(q) == 0 {
                    bed.push(q);
                }
            });
        }
        for &q in &bed {
            self.set(q, Terrain::Wash);
        }
        let banks: Vec<Pos> = bed
            .iter()
            .flat_map(|q| q.neighbors8())
            .filter(|q| {
                self.get(*q).is_some_and(|t| {
                    matches!(t.terrain, Terrain::Dirt | Terrain::Rise | Terrain::Crops)
                })
            })
            .collect();
        for q in banks {
            self.set(q, Terrain::Scrub);
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

/// How wide a river is (detail tiles) on overworld tile `p`: 3 at its spring, 7 at its
/// last tile; 0 where no river runs.
pub fn river_width(world: &Overworld, p: Pos) -> u8 {
    world
        .rivers
        .iter()
        .filter_map(|r| {
            let k = r.iter().position(|q| *q == p)?;
            Some((3.0 + 4.0 * k as f32 / (r.len().max(2) - 1) as f32).round() as u8)
        })
        .max()
        .unwrap_or(0)
}

/// Where a line out of tile `(tx, ty)` on `side` crosses into the next tile: the cell
/// just inside this tile. A road crosses at the edge's middle, always; a river or wash
/// at a spot the edge's own hash picks near the middle; both tiles compute the same.
fn crossing(world: &Overworld, tx: i32, ty: i32, side: Dir, road: bool, salt: u64) -> Pos {
    let offset = |etx: i32, ety: i32, vertical_edge: bool| -> i32 {
        if road {
            3
        } else {
            let h = hash64(
                hash64(world.seed, salt),
                ((etx as u32 as u64) << 33) ^ ((ety as u32 as u64) << 1) ^ vertical_edge as u64,
            );
            2 + (h % 4) as i32
        }
    };
    match side {
        Dir::E => Pos::new(TILE * tx + TILE - 1, TILE * ty + offset(tx, ty, true)),
        Dir::W => Pos::new(TILE * tx, TILE * ty + offset(tx - 1, ty, true)),
        Dir::S => Pos::new(TILE * tx + offset(tx, ty, false), TILE * ty + TILE - 1),
        _ => Pos::new(TILE * tx + offset(tx, ty - 1, false), TILE * ty),
    }
}

/// Four-connected cells from `a` to `b`, stepping along the longer way.
fn straight(a: Pos, b: Pos) -> Vec<Pos> {
    let (mut p, mut out) = (a, vec![a]);
    while p != b {
        let (dx, dy) = (b.x - p.x, b.y - p.y);
        if dx != 0 && dx.abs() * 2 >= dy.abs() {
            p.x += dx.signum();
        } else {
            p.y += dy.signum();
        }
        out.push(p);
    }
    out
}

/// The cells a line runs on through tile `(tx, ty)`, from the link mask: straight
/// across when it passes through, else from each crossing to the tile's middle (a road
/// exactly there, so its corners line up with its crossings).
fn centreline(world: &Overworld, tx: i32, ty: i32, mask: u8, road: bool, salt: u64) -> Vec<Pos> {
    use super::{LINK_E, LINK_N, LINK_S, LINK_W};
    let cross = |d: Dir| crossing(world, tx, ty, d, road, salt);
    if mask == LINK_E | LINK_W {
        return straight(cross(Dir::W), cross(Dir::E));
    }
    if mask == LINK_N | LINK_S {
        return straight(cross(Dir::N), cross(Dir::S));
    }
    let jiggle = |k: u64| {
        if road {
            0
        } else {
            (hash64(
                world.seed ^ salt,
                ((tx as u32 as u64) << 32 | ty as u32 as u64) + k,
            ) % 2) as i32
        }
    };
    let mid = Pos::new(TILE * tx + 3 + jiggle(1), TILE * ty + 3 + jiggle(2));
    let mut out = vec![mid];
    for (bit, d) in [
        (LINK_N, Dir::N),
        (LINK_E, Dir::E),
        (LINK_S, Dir::S),
        (LINK_W, Dir::W),
    ] {
        if mask & bit == 0 {
            continue;
        }
        let c = cross(d);
        // along the crossing's own axis first, so a road leaves its edge square
        let bend = if matches!(d, Dir::E | Dir::W) {
            Pos::new(mid.x, c.y)
        } else {
            Pos::new(c.x, mid.y)
        };
        out.extend(straight(c, bend));
        out.extend(straight(bend, mid));
    }
    out
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
                                    | Terrain::Ford
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
    fn rivers_run_unbroken_across_tiles() {
        for seed in 0..3 {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            let r = whole(&w);
            let wet = |p: Pos| {
                r.get(p).is_some_and(|t| {
                    matches!(
                        t.terrain,
                        Terrain::Water | Terrain::Shallows | Terrain::Ford | Terrain::Bridge
                    )
                })
            };
            for river in &w.rivers {
                for pair in river.windows(2) {
                    let (a, b) = (pair[0], pair[1]);
                    // water in tile a reaches the edge it shares with tile b
                    let edge: Vec<Pos> = (0..8)
                        .map(|k| match (b.x - a.x, b.y - a.y) {
                            (1, 0) => Pos::new(8 * a.x + 7, 8 * a.y + k),
                            (-1, 0) => Pos::new(8 * a.x, 8 * a.y + k),
                            (0, 1) => Pos::new(8 * a.x + k, 8 * a.y + 7),
                            _ => Pos::new(8 * a.x + k, 8 * a.y),
                        })
                        .collect();
                    let across = |p: Pos| Pos::new(p.x + (b.x - a.x), p.y + (b.y - a.y));
                    assert!(
                        edge.iter().any(|p| wet(*p) && wet(across(*p))),
                        "seed {seed}: the river breaks between {a:?} and {b:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn rivers_widen_toward_the_mouth() {
        let w = Overworld::generate(WorldParams {
            seed: 0,
            ..Default::default()
        })
        .unwrap();
        let river = w.rivers.iter().max_by_key(|r| r.len()).unwrap();
        assert_eq!(river_width(&w, river[0]), 3);
        assert_eq!(river_width(&w, *river.last().unwrap()), 7);
        assert_eq!(river_width(&w, Pos::new(-5, -5)), 0);
    }

    #[test]
    fn washes_run_unbroken_and_dry() {
        for seed in 0..4 {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            for wash in &w.washes {
                for pair in wash.windows(2) {
                    let (a, b) = (pair[0], pair[1]);
                    let r = region(&w, 8 * a.x.min(b.x), 8 * a.y.min(b.y), 16, 16);
                    assert!(
                        r.count(|t| t.terrain == Terrain::Wash) >= 6,
                        "seed {seed}: the wash breaks at {a:?}-{b:?}"
                    );
                }
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
