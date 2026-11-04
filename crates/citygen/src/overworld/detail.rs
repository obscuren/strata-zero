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
use crate::tile::{Feature, Terrain, Tile};
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
    p.roads();
    super::poi_ground::lay(&mut p);
    p.power_lines();
    p.litter();
    p.crop(x0, y0, w, h)
}

/// Whether any water (or its shallows) lies in world-wide `rect` or a cell round it, from
/// the ground and water alone: what a POI's building is placed by, the same whichever
/// patch asks.
pub(crate) fn wet_in(world: &Overworld, rect: crate::geom::Rect) -> bool {
    let mut p = Patch::new(
        world,
        rect.x - 1 - MARGIN,
        rect.y - 1 - MARGIN,
        (rect.w + 2 + 2 * MARGIN) as u32,
        (rect.h + 2 + 2 * MARGIN) as u32,
    );
    p.cover();
    p.water();
    p.rivers();
    (rect.y - 1..rect.y + rect.h + 1)
        .flat_map(|y| (rect.x - 1..rect.x + rect.w + 1).map(move |x| Pos::new(x, y)))
        .any(|q| p.wet_at(q) != 0)
}

/// A rectangle of the detail world while it is being built.
pub(crate) struct Patch<'a> {
    pub(crate) world: &'a Overworld,
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

    pub(crate) fn get(&self, p: Pos) -> Option<&Tile> {
        self.grid.get(self.local(p))
    }

    pub(crate) fn set(&mut self, p: Pos, terrain: Terrain) {
        let l = self.local(p);
        if let Some(t) = self.grid.get_mut(l) {
            t.terrain = terrain;
        }
    }

    /// Whether `p` lies inside the world.
    pub(crate) fn in_world(&self, p: Pos) -> bool {
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
            let dx = (4.0 * wave(&jx, x / 9.0, y / 4.5)).round() as i32;
            let dy = (2.0 * wave(&jy, x / 9.0, y / 4.5)).round() as i32;
            let t = tile_at(w, p.x + dx, p.y + dy);
            let terrain = match t.ground {
                Ground::Flats => Terrain::Dirt,
                Ground::Scrub => Terrain::Scrub,
                Ground::Fields => Terrain::Crops,
                Ground::Salt => Terrain::Salt,
                Ground::Spill => Terrain::Spill,
                Ground::Hills if roll(w.seed, 2005, p) < 0.12 => Terrain::Rock,
                Ground::Hills => Terrain::Dirt,
                Ground::Mountains if wave(&pass, x / 7.0, y / 3.5) < 0.0 => Terrain::Dirt,
                Ground::Mountains | Ground::Peak => Terrain::Cliff,
            };
            let terrain = if terrain == Terrain::Dirt && wave(&rise, x / 8.0, y / 4.0) > 0.45 {
                Terrain::Rise
            } else {
                terrain
            };
            self.set(p, terrain);
        }
    }

    pub(crate) fn wet_at(&self, p: Pos) -> u8 {
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
        let warp = [Fbm::new(w.seed, 2014), Fbm::new(w.seed, 2015)];
        for p in self.cells().collect::<Vec<_>>() {
            if !self.in_world(p) {
                continue;
            }
            let (x, y) = (p.x as f32, p.y as f32);
            let shore = 0.4
                * (0.34 * wave(&n[0], x / 16.0, y / 8.0)
                    + 0.22 * wave(&n[1], x / 6.0, y / 3.0)
                    + 0.10 * wave(&n[2], x / 2.5, y / 1.25));
            // the blend read a few cells off, so the shore leaves the tile lines
            let (bx, by) = (
                x + 5.0 * wave(&warp[0], x / 10.0, y / 5.0),
                y + 3.5 * wave(&warp[1], x / 10.0, y / 5.0),
            );
            let sea = blend(w, bx, by, |t| (t.water == Water::Sea) as u8 as f32) + shore;
            let lake = blend(w, bx, by, |t| (t.water == Water::Lake) as u8 as f32) + shore;
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

    /// Every cell within `radius` cells of the points of a course (in cells both ways,
    /// so a river running east-west is as many rows deep as one running north-south is
    /// columns wide, as round 4 drew them).
    fn channel(
        &mut self,
        pts: &[(f32, f32)],
        radius: impl Fn(f32, f32) -> f32,
        mut f: impl FnMut(&mut Self, Pos),
    ) {
        let mut hit = Vec::new();
        for &(x, y) in pts {
            let r = radius(x, y).max(1.2);
            let reach = r.ceil() as i32;
            let (cx, cy) = (x.floor() as i32, y.floor() as i32);
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let p = Pos::new(cx + dx, cy + dy);
                    let (ex, ey) = (p.x as f32 + 0.5 - x, p.y as f32 + 0.5 - y);
                    if ex * ex + ey * ey <= r * r && self.get(p).is_some() && self.in_world(p) {
                        hit.push(p);
                    }
                }
            }
        }
        for p in hit {
            f(self, p);
        }
    }

    /// Rivers: water along each river tile's course, as wide as the river is there, the
    /// narrow upper reaches with a ford here and there.
    fn rivers(&mut self) {
        let w = self.world;
        let mut fords = Vec::new();
        for t in self.tiles_in_reach() {
            let mask = w.tile(t).unwrap().river;
            if mask == 0 {
                continue;
            }
            let width = river_width(w, t).max(3) as f32;
            let pts = course(w, t.x, t.y, mask, 2021);
            let bank = Fbm::new(w.seed, 2023);
            let radius = |x: f32, y: f32| 0.5 * width + 0.3 + 1.4 * wave(&bank, x / 5.0, y / 2.5);
            self.channel(&pts, radius, |p, q| {
                if p.wet_at(q) != SEA {
                    p.set(q, Terrain::Water);
                    p.set_wet(q, RIVER);
                }
            });
            // a third of the narrow reaches' tiles have a ford, at a point their hash picks
            let h = hash64(
                hash64(w.seed, 2022),
                ((t.x as u32 as u64) << 32) | t.y as u32 as u64,
            );
            // not where a wider river joins: the water there is both rivers' width
            let joins = t.neighbors8().any(|q| river_width(w, q) > 3);
            if width == 3.0 && h.is_multiple_of(3) && !joins && !pts.is_empty() {
                let (x, y) = pts[(h >> 8) as usize % pts.len()];
                fords.push((x, y, radius(x, y).max(1.2)));
            }
        }
        // a ford is the river's whole width there, bank to bank
        for (x, y, r) in fords {
            let reach = (r + 1.5).ceil() as i32;
            let (cx, cy) = (x.floor() as i32, y.floor() as i32);
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let q = Pos::new(cx + dx, cy + dy);
                    let (ex, ey) = (q.x as f32 + 0.5 - x, q.y as f32 + 0.5 - y);
                    if ex * ex + ey * ey <= (r + 1.5) * (r + 1.5) && self.wet_at(q) == RIVER {
                        self.set(q, Terrain::Ford);
                    }
                }
            }
        }
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
            let width = if lower { 4.0 } else { 3.0 };
            let pts = course(w, t.x, t.y, mask, 2031);
            let bank = Fbm::new(w.seed, 2032);
            let radius = |x: f32, y: f32| 0.5 * width + 0.2 + 0.8 * wave(&bank, x / 5.0, y / 2.5);
            self.channel(&pts, radius, |p, q| {
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

    /// Highways: two lanes along each road tile's line (rows or columns 3–4 of the tile,
    /// so a straight run never winds and every road meets itself at the edge), gravel
    /// shoulders either side; over water the lanes are a bridge with rails for
    /// shoulders, over a dry wash the road rides a culvert whose ends show as grates.
    fn roads(&mut self) {
        let w = self.world;
        let mut lanes = Vec::new();
        for t in self.tiles_in_reach() {
            let mask = w.tile(t).unwrap().road;
            if mask != 0 {
                lanes.extend(centreline(w, t.x, t.y, mask));
            }
        }
        let mut deck = Vec::new();
        self.brush_from(&lanes, 2, |_, q| deck.push(q));
        let mut on_deck = Grid::filled(self.grid.size(), false);
        for &q in &deck {
            if let Some(v) = on_deck.get_mut(self.local(q)) {
                *v = true;
            }
        }
        let shoulders: Vec<Pos> = deck
            .iter()
            .flat_map(|q| q.neighbors8())
            .filter(|q| self.in_world(*q))
            .filter(|q| on_deck.get(self.local(*q)) == Some(&false))
            .collect();
        for &q in &deck {
            let lane = if self.wet_at(q) != 0 {
                Terrain::Bridge
            } else {
                Terrain::Road
            };
            self.set(q, lane);
        }
        // decided before any is set: a shoulder cell is reached from several lane cells
        let sides: Vec<(Pos, Terrain)> = shoulders
            .into_iter()
            .filter_map(|q| {
                let side = match self.get(q)?.terrain {
                    _ if self.wet_at(q) != 0 => Terrain::Railing,
                    Terrain::Wash => Terrain::Grate,
                    _ => Terrain::Gravel,
                };
                Some((q, side))
            })
            .collect();
        for (q, side) in sides {
            self.set(q, side);
        }
    }

    /// Power lines on pylons beside a third of the highways (the route's hash picks
    /// which, and which side): the line keeps 3 rows off a road running east-west and 6
    /// columns off one running north-south, about the same distance on screen, with a
    /// pylon at every corner and every other tile. Nothing stands on water, a road or
    /// rough ground; the line just has a gap there.
    fn power_lines(&mut self) {
        let w = self.world;
        let Size { w: pw, h: ph } = self.grid.size();
        let lo = Pos::new(self.x0.div_euclid(TILE) - 2, self.y0.div_euclid(TILE) - 2);
        let hi = Pos::new(
            (self.x0 + pw as i32).div_euclid(TILE) + 2,
            (self.y0 + ph as i32).div_euclid(TILE) + 2,
        );
        let near = |p: Pos| p.x >= lo.x && p.y >= lo.y && p.x <= hi.x && p.y <= hi.y;
        let mut line = Vec::new();
        let mut pylons = Vec::new();
        for (i, r) in w.routes.iter().enumerate() {
            let h = hash64(hash64(w.seed, 2041), i as u64);
            if !h.is_multiple_of(3) || r.path.len() < 2 {
                continue;
            }
            let side = if (h >> 8) & 1 == 0 { 1 } else { -1 };
            let points: Vec<Option<(Pos, bool)>> = (0..r.path.len())
                .map(|k| line_point(&r.path, k, side))
                .collect();
            for k in 0..r.path.len() - 1 {
                if !(near(r.path[k]) || near(r.path[k + 1])) {
                    continue;
                }
                let (Some((a, a_pylon)), Some((b, b_pylon))) = (points[k], points[k + 1]) else {
                    continue;
                };
                let cells = straight(a, b);
                for (j, c) in cells.iter().enumerate() {
                    let next = cells.get(j + 1).or(cells.get(j.wrapping_sub(1)));
                    let north_south = next.is_some_and(|n| n.x == c.x);
                    line.push((*c, north_south));
                }
                if a_pylon {
                    pylons.push(a);
                }
                if b_pylon {
                    pylons.push(b);
                }
            }
        }
        for (c, north_south) in line {
            if self.bare(c, false) {
                self.set_feature(c, Feature::PowerLine(north_south));
            }
        }
        // a pylon only where its line reaches it (not alone by a river or a road)
        for c in pylons {
            let wired = c.neighbors8().any(|q| {
                self.get(q)
                    .is_some_and(|t| matches!(t.feature, Some(Feature::PowerLine(_))))
            });
            if wired && self.bare(c, true) {
                self.set_feature(c, Feature::Pylon);
            }
        }
    }

    /// Litter and the odd wreck on and just off the highways' shoulders.
    fn litter(&mut self) {
        let seed = self.world.seed;
        for p in self.cells().collect::<Vec<_>>() {
            if !self.bare(p, false) || self.get(p).is_some_and(|t| t.terrain == Terrain::Wash) {
                continue;
            }
            let by_road = std::iter::once(p)
                .chain(p.neighbors8())
                .any(|q| self.get(q).is_some_and(|t| t.terrain == Terrain::Gravel));
            if !by_road {
                continue;
            }
            if roll(seed, 2051, p) < 0.03 {
                self.set_feature(p, Feature::Trash);
            } else if roll(seed, 2052, p) < 0.006 {
                self.set_feature(p, Feature::Barrier);
            }
        }
    }

    /// Whether dressing may stand on `p`: open ground in the world with nothing on it
    /// (a pylon may replace its own line; it may not stand on a shoulder or in a wash).
    fn bare(&self, p: Pos, pylon: bool) -> bool {
        self.in_world(p)
            && self.get(p).is_some_and(|t| {
                let free = t.feature.is_none()
                    || pylon && matches!(t.feature, Some(Feature::PowerLine(_)));
                let ground = matches!(
                    t.terrain,
                    Terrain::Dirt
                        | Terrain::Scrub
                        | Terrain::Crops
                        | Terrain::Salt
                        | Terrain::Spill
                        | Terrain::Sand
                        | Terrain::Rise
                        | Terrain::Reeds
                ) || !pylon && matches!(t.terrain, Terrain::Gravel | Terrain::Wash);
                free && ground
            })
    }

    /// Whether `p` lies within `reach` cells of the patch.
    pub(crate) fn near(&self, p: Pos, reach: i32) -> bool {
        let Size { w, h } = self.grid.size();
        p.x >= self.x0 - reach
            && p.y >= self.y0 - reach
            && p.x < self.x0 + w as i32 + reach
            && p.y < self.y0 + h as i32 + reach
    }

    pub(crate) fn clear_feature(&mut self, p: Pos) {
        let l = self.local(p);
        if let Some(t) = self.grid.get_mut(l) {
            t.feature = None;
        }
    }

    pub(crate) fn set_feature(&mut self, p: Pos, f: Feature) {
        let l = self.local(p);
        if let Some(t) = self.grid.get_mut(l) {
            t.feature = Some(f);
        }
    }

    /// Stamps a square brush `width` wide reaching right and down from each of `cells`.
    fn brush_from(&mut self, cells: &[Pos], width: i32, mut f: impl FnMut(&mut Self, Pos)) {
        for c in cells {
            for dy in 0..width {
                for dx in 0..width {
                    let p = c.offset(dx, dy);
                    if self.get(p).is_some() && self.in_world(p) {
                        f(self, p);
                    }
                }
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

/// Where a power line beside route `path` stands on its `k`th tile, and whether a pylon
/// stands there: off the road's lanes by the normal of each step on `side`, mitred at a
/// corner (which always has a pylon), else on every other tile. `None` where the path
/// doesn't step to a neighbour.
fn line_point(path: &[Pos], k: usize, side: i32) -> Option<(Pos, bool)> {
    let step = |a: Pos, b: Pos| {
        let d = (b.x - a.x, b.y - a.y);
        (d.0.abs() + d.1.abs() == 1).then_some(d)
    };
    let d_in = if k > 0 {
        step(path[k - 1], path[k])?
    } else {
        step(path[0], path[1])?
    };
    let d_out = match path.get(k + 1) {
        Some(&n) => step(path[k], n)?,
        None => d_in,
    };
    let t = path[k];
    let (ox, oy) = (TILE * t.x, TILE * t.y);
    // a step east-west puts the line on row 7 (south) or row 0 (north) of the tile; a
    // step north-south on column 10 (east) or -3 (west)
    let row = |(dx, _): (i32, i32)| oy + if side * dx > 0 { 7 } else { 0 };
    let col = |(_, dy): (i32, i32)| ox + if -side * dy > 0 { 10 } else { -3 };
    let point = match (d_in.0 != 0, d_out.0 != 0) {
        (true, true) => (Pos::new(ox + 3, row(d_in)), false),
        (false, false) => (Pos::new(col(d_in), oy + 3), false),
        (true, false) => (Pos::new(col(d_out), row(d_in)), true),
        (false, true) => (Pos::new(col(d_in), row(d_out)), true),
    };
    Some((point.0, point.1 || (t.x + t.y).rem_euclid(2) == 0))
}

/// Where a river or wash crosses `side` of tile `(tx, ty)`, as a point on the edge
/// itself, and the way into the tile from there; both tiles of the edge agree.
fn edge_point(
    world: &Overworld,
    tx: i32,
    ty: i32,
    side: Dir,
    salt: u64,
) -> ((f32, f32), (f32, f32)) {
    let c = crossing(world, tx, ty, side, false, salt);
    let (x0, y0) = ((TILE * tx) as f32, (TILE * ty) as f32);
    let (cx, cy) = (c.x as f32 + 0.5, c.y as f32 + 0.5);
    match side {
        Dir::E => ((x0 + TILE as f32, cy), (-1.0, 0.0)),
        Dir::W => ((x0, cy), (1.0, 0.0)),
        Dir::S => ((cx, y0 + TILE as f32), (0.0, -1.0)),
        _ => ((cx, y0), (0.0, 1.0)),
    }
}

/// A river's or wash's course through tile `(tx, ty)` from its link mask, as points in
/// detail coordinates: a curve leaving each crossing square to its edge, through to the
/// other crossing (or to the tile's middle where one, three or four meet), then
/// meandered by a world-wide field so long runs don't line up tile after tile.
fn course(world: &Overworld, tx: i32, ty: i32, mask: u8, salt: u64) -> Vec<(f32, f32)> {
    use super::{LINK_E, LINK_N, LINK_S, LINK_W};
    const HANDLE: f32 = 3.0;
    const STEPS: usize = 24;
    let ends: Vec<((f32, f32), (f32, f32))> = [
        (LINK_N, Dir::N),
        (LINK_E, Dir::E),
        (LINK_S, Dir::S),
        (LINK_W, Dir::W),
    ]
    .into_iter()
    .filter(|(bit, _)| mask & bit != 0)
    .map(|(_, d)| edge_point(world, tx, ty, d, salt))
    .collect();
    let mut out = Vec::new();
    let mut curve = |a: (f32, f32), na: (f32, f32), b: (f32, f32), nb: (f32, f32)| {
        let p1 = (a.0 + HANDLE * na.0, a.1 + HANDLE * na.1);
        let p2 = (b.0 + HANDLE * nb.0, b.1 + HANDLE * nb.1);
        for i in 0..=STEPS {
            let t = i as f32 / STEPS as f32;
            let u = 1.0 - t;
            let (k0, k1, k2, k3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            out.push((
                k0 * a.0 + k1 * p1.0 + k2 * p2.0 + k3 * b.0,
                k0 * a.1 + k1 * p1.1 + k2 * p2.1 + k3 * b.1,
            ));
        }
    };
    if let [(a, na), (b, nb)] = ends[..] {
        curve(a, na, b, nb);
    } else {
        let mid = ((TILE * tx) as f32 + 4.0, (TILE * ty) as f32 + 4.0);
        for &(a, na) in &ends {
            curve(a, na, mid, (0.0, 0.0));
        }
    }
    let (mx, my) = (
        Fbm::new(world.seed, salt as u32 + 7),
        Fbm::new(world.seed, salt as u32 + 8),
    );
    out.into_iter()
        .map(|(x, y)| {
            (
                x + 2.5 * wave(&mx, x / 9.0, y / 4.5),
                y + 1.4 * wave(&my, x / 9.0, y / 4.5),
            )
        })
        .collect()
}

/// Noise spread over -1..1: the field keeps near its middle (about 0.5 ± 0.09), so
/// this stretches it, clamping the rare extremes.
pub(crate) fn wave(n: &Fbm, x: f32, y: f32) -> f32 {
    ((n.at(x, y, 2) - 0.5) * 5.0).clamp(-1.0, 1.0)
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

/// The cells a road runs on through tile `(tx, ty)`, from the link mask: straight
/// across when it passes through, else from each crossing to the tile's middle, so its
/// corners line up with its crossings and a straight road never winds.
pub(crate) fn centreline(world: &Overworld, tx: i32, ty: i32, mask: u8) -> Vec<Pos> {
    use super::{LINK_E, LINK_N, LINK_S, LINK_W};
    let cross = |d: Dir| crossing(world, tx, ty, d, true, 0);
    if mask == LINK_E | LINK_W {
        return straight(cross(Dir::W), cross(Dir::E));
    }
    if mask == LINK_N | LINK_S {
        return straight(cross(Dir::N), cross(Dir::S));
    }
    let mid = Pos::new(TILE * tx + 3, TILE * ty + 3);
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
pub(crate) fn blend(world: &Overworld, x: f32, y: f32, f: impl Fn(&WorldTile) -> f32) -> f32 {
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

/// A hash of tile `(tx, ty)`'s ground, cell by cell, for the pinned-seed tests.
#[cfg(test)]
pub fn fingerprint(world: &Overworld, tx: i32, ty: i32) -> u64 {
    let g = ground(world, tx, ty);
    let cells: Vec<(Terrain, Option<Feature>, u8)> = g
        .iter()
        .map(|(_, t)| (t.terrain, t.feature, t.variant))
        .collect();
    let bytes = postcard::to_allocvec(&cells).unwrap();
    bytes
        .chunks(8)
        .fold(hash64(world.seed, 0xD7A1), |h, chunk| {
            let mut b = [0u8; 8];
            b[..chunk.len()].copy_from_slice(chunk);
            hash64(h, u64::from_le_bytes(b))
        })
}

/// A detail cell's stable roll in 0..1 under `salt`.
pub(crate) fn roll(seed: u64, salt: u64, p: Pos) -> f32 {
    let packed = ((p.x as u32 as u64) << 32) | p.y as u32 as u64;
    (hash64(hash64(seed, salt), packed) >> 40) as f32 / (1u64 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Pos, Size};
    use crate::overworld::{Ground, Overworld, Water, WorldParams, WorldSize};
    use crate::tile::Terrain;

    /// The small world (160×52): the rules one level down don't depend on the world's
    /// size, and a whole one builds a quarter as fast as the default.
    fn small(seed: u64) -> Overworld {
        Overworld::generate(WorldParams {
            seed,
            size: WorldSize::Small,
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn a_tile_alone_equals_the_same_tile_in_a_region() {
        let w = small(0);
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
            let w = small(seed);
            for (p, t) in w.tiles.iter() {
                if t.water != Water::None || t.road != 0 || t.wash != 0 || t.urban.is_some() {
                    continue;
                }
                // a wide river's or wash's banks spill into the tiles beside it, and so
                // does a coast
                let by_line = p.neighbors8().any(|q| {
                    w.tile(q)
                        .is_some_and(|n| n.river != 0 || n.wash != 0 || n.water != Water::None)
                });
                // and a POI's place is its own, not the ground's
                let by_poi = w
                    .pois
                    .iter()
                    .any(|o| (o.at.x - p.x).abs() <= 5 && (o.at.y - p.y).abs() <= 3);
                if by_line || by_poi {
                    continue;
                }
                let own = |c: Terrain| match t.ground {
                    Ground::Flats => matches!(c, Terrain::Dirt | Terrain::Rise),
                    Ground::Hills => matches!(c, Terrain::Dirt | Terrain::Rise | Terrain::Rock),
                    Ground::Scrub => c == Terrain::Scrub,
                    Ground::Fields => c == Terrain::Crops,
                    Ground::Salt => c == Terrain::Salt,
                    Ground::Spill => c == Terrain::Spill,
                    // cliffs, and the passes through them
                    Ground::Mountains | Ground::Peak => {
                        matches!(c, Terrain::Cliff | Terrain::Dirt | Terrain::Rise)
                    }
                };
                // a good part of the tile is its own ground: its edges blend with its
                // neighbours', and where the jitter runs one way the whole tile leans
                let g = ground(&w, p.x, p.y);
                let mine = g.count(|c| own(c.terrain));
                assert!(
                    mine >= 24,
                    "seed {seed} {p:?}: {mine} of 64 cells are {:?}'s",
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
            let w = small(seed);
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
        let w = small(0);
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
            let w = small(seed);
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
    fn river_banks_curve_rather_than_step() {
        // Banks that run dead straight for a tile or more read as a canal of boxes.
        let w = small(0);
        let g = whole(&w);
        let wet = |p: Pos| {
            g.get(p).is_some_and(|t| {
                matches!(
                    t.terrain,
                    Terrain::Water
                        | Terrain::Shallows
                        | Terrain::Ford
                        | Terrain::Bridge
                        | Terrain::Railing
                )
            })
        };
        let river_only = |p: Pos| {
            let t = Pos::new(p.x.div_euclid(8), p.y.div_euclid(8));
            w.tile(t).is_some_and(|t| t.water == Water::River)
                && (-1..=1).all(|dx| {
                    (-1..=1).all(|dy| {
                        w.tile(t.offset(dx, dy))
                            .is_none_or(|t| !matches!(t.water, Water::Sea | Water::Lake))
                    })
                })
        };
        let size = g.size();
        let (mut bank, mut straight) = (0, 0);
        let mut tally = |run: &mut i32| {
            bank += *run;
            if *run >= 8 {
                straight += *run;
            }
            *run = 0;
        };
        for side in [-1, 1] {
            for y in 0..size.h as i32 {
                let mut run = 0;
                for x in 0..size.w as i32 {
                    let p = Pos::new(x, y);
                    if wet(p)
                        && !wet(p.offset(0, side))
                        && river_only(p)
                        && g.get(p.offset(0, side)).is_some()
                    {
                        run += 1;
                    } else {
                        tally(&mut run);
                    }
                }
                tally(&mut run);
            }
            for x in 0..size.w as i32 {
                let mut run = 0;
                for y in 0..size.h as i32 {
                    let p = Pos::new(x, y);
                    if wet(p)
                        && !wet(p.offset(side, 0))
                        && river_only(p)
                        && g.get(p.offset(side, 0)).is_some()
                    {
                        run += 1;
                    } else {
                        tally(&mut run);
                    }
                }
                tally(&mut run);
            }
        }
        assert!(bank > 500, "{bank} bank cells");
        assert!(
            straight * 100 < bank * 15,
            "{straight} of {bank} bank cells run straight for 8 or more"
        );
    }

    /// Of the shore cells (`wet` with dry ground right beside on one side) that `count`
    /// takes, how many run dead straight along a row or column for 8 or more.
    fn straight_banks(
        g: &Grid<Tile>,
        wet: impl Fn(Pos) -> bool,
        count: impl Fn(Pos) -> bool,
    ) -> (i32, i32) {
        let size = g.size();
        let (mut bank, mut straight) = (0, 0);
        let mut tally = |run: &mut i32| {
            bank += *run;
            if *run >= 8 {
                straight += *run;
            }
            *run = 0;
        };
        let edge = |p: Pos, q: Pos| wet(p) && !wet(q) && g.get(q).is_some() && count(p);
        for side in [-1, 1] {
            for y in 0..size.h as i32 {
                let mut run = 0;
                for x in 0..size.w as i32 {
                    let p = Pos::new(x, y);
                    if edge(p, p.offset(0, side)) {
                        run += 1;
                    } else {
                        tally(&mut run);
                    }
                }
                tally(&mut run);
            }
            for x in 0..size.w as i32 {
                let mut run = 0;
                for y in 0..size.h as i32 {
                    let p = Pos::new(x, y);
                    if edge(p, p.offset(side, 0)) {
                        run += 1;
                    } else {
                        tally(&mut run);
                    }
                }
                tally(&mut run);
            }
        }
        (bank, straight)
    }

    #[test]
    fn coasts_and_lake_shores_curve_off_the_tile_lines() {
        for seed in [0, 3] {
            let w = small(seed);
            let g = whole(&w);
            let wet = |p: Pos| {
                g.get(p)
                    .is_some_and(|t| matches!(t.terrain, Terrain::Water | Terrain::Shallows))
            };
            // shores of the sea and lakes, away from rivers
            let coast = |p: Pos| {
                let t = Pos::new(p.x.div_euclid(8), p.y.div_euclid(8));
                (-1..=1).all(|dx| {
                    (-1..=1).all(|dy| w.tile(t.offset(dx, dy)).is_none_or(|t| t.river == 0))
                })
            };
            let (bank, straight) = straight_banks(&g, wet, coast);
            assert!(bank > 1000, "seed {seed}: {bank} shore cells");
            assert!(
                straight * 100 < bank * 15,
                "seed {seed}: {straight} of {bank} shore cells run straight for 8 or more"
            );
        }
    }

    #[test]
    fn ground_kinds_meet_off_the_tile_lines() {
        let w = small(0);
        let g = whole(&w);
        let kind = |t: &Tile| match t.terrain {
            Terrain::Dirt | Terrain::Rise | Terrain::Rock => Some(0),
            Terrain::Scrub => Some(1),
            Terrain::Crops => Some(2),
            Terrain::Salt => Some(3),
            Terrain::Spill => Some(4),
            // cliffs and passes follow their own noise, not the tile's ground
            _ => None,
        };
        let (mut seams, mut on_lines) = (0, 0);
        for (p, t) in g.iter() {
            let (Some(a), Some(b)) = (kind(t), g.get(p.offset(1, 0)).and_then(kind)) else {
                continue;
            };
            // scrub also lines washes; the seams that matter are between ground kinds
            if a != b && a >= 2 || b >= 2 && a != b {
                seams += 1;
                // a seam between x and x + 1, within a cell of a tile line
                on_lines += matches!((p.x + 1).rem_euclid(8), 7 | 0 | 1) as i32;
            }
        }
        assert!(seams > 500, "{seams}");
        assert!(
            on_lines * 100 < seams * 60,
            "{on_lines} of {seams} ground seams sit within a cell of a tile line"
        );
    }

    #[test]
    fn passes_run_through_the_mountains() {
        let w = small(0);
        let g = whole(&w);
        let in_range = |p: Pos| {
            w.tile(Pos::new(p.x.div_euclid(8), p.y.div_euclid(8)))
                .is_some_and(|t| t.ground == Ground::Mountains)
        };
        let open = |p: Pos| in_range(p) && g.get(p).is_some_and(|t| t.is_walkable());
        let mut seen = Grid::filled(g.size(), false);
        let (mut all, mut big) = (0, 0);
        for (p, _) in g.iter() {
            if seen[p] || !open(p) {
                continue;
            }
            let mut stack = vec![p];
            seen[p] = true;
            let mut n = 0;
            while let Some(q) = stack.pop() {
                n += 1;
                for r in q.neighbors8() {
                    if g.get(r).is_some() && !seen[r] && open(r) {
                        seen[r] = true;
                        stack.push(r);
                    }
                }
            }
            all += n;
            if n >= 60 {
                big += n;
            }
        }
        assert!(all > 1000, "{all} open cells in the mountains");
        assert!(
            big * 100 > all * 75,
            "{big} of {all} open mountain cells in passes of 60+"
        );
    }

    #[test]
    fn every_ford_crosses_the_river() {
        let mut fords = 0;
        for seed in 0..4 {
            let w = small(seed);
            let g = whole(&w);
            let land = |p: Pos| {
                g.get(p)
                    .is_some_and(|t| t.is_walkable() && t.terrain != Terrain::Ford)
            };
            let mut seen = Grid::filled(g.size(), false);
            for (p, t) in g.iter() {
                if t.terrain != Terrain::Ford || seen[p] {
                    continue;
                }
                // the ford's patch
                let mut patch = vec![];
                let mut stack = vec![p];
                seen[p] = true;
                while let Some(q) = stack.pop() {
                    patch.push(q);
                    for r in q.neighbors8() {
                        if g.get(r).is_some_and(|t| t.terrain == Terrain::Ford) && !seen[r] {
                            seen[r] = true;
                            stack.push(r);
                        }
                    }
                }
                let tile = |q: Pos| w.tile(Pos::new(q.x.div_euclid(8), q.y.div_euclid(8)));
                if patch.iter().any(|q| {
                    tile(*q).is_some_and(|t| matches!(t.ground, Ground::Hills | Ground::Mountains))
                }) {
                    continue; // a gorge: cliffs for banks
                }
                // the dry ground it touches, in pieces joined within a window round it
                let (x0, x1) = (
                    patch.iter().map(|q| q.x).min().unwrap() - 4,
                    patch.iter().map(|q| q.x).max().unwrap() + 4,
                );
                let (y0, y1) = (
                    patch.iter().map(|q| q.y).min().unwrap() - 4,
                    patch.iter().map(|q| q.y).max().unwrap() + 4,
                );
                let inside = |q: Pos| q.x >= x0 && q.x <= x1 && q.y >= y0 && q.y <= y1;
                let mut piece: std::collections::BTreeMap<Pos, usize> = Default::default();
                let mut pieces = 0;
                for &q in &patch {
                    for start in q.neighbors8().filter(|r| land(*r)) {
                        if piece.contains_key(&start) {
                            continue;
                        }
                        let mut stack = vec![start];
                        piece.insert(start, pieces);
                        while let Some(r) = stack.pop() {
                            for n in r.neighbors8() {
                                if inside(n) && land(n) && !piece.contains_key(&n) {
                                    piece.insert(n, pieces);
                                    stack.push(n);
                                }
                            }
                        }
                        pieces += 1;
                    }
                }
                assert!(
                    pieces >= 2,
                    "seed {seed}: a ford at {p:?} reaches {pieces} bank(s)"
                );
                fords += 1;
            }
        }
        assert!(fords >= 10, "{fords} fords");
    }

    #[test]
    fn rivers_widen_toward_the_mouth() {
        let w = small(0);
        let river = w.rivers.iter().max_by_key(|r| r.len()).unwrap();
        assert_eq!(river_width(&w, river[0]), 3);
        assert_eq!(river_width(&w, *river.last().unwrap()), 7);
        assert_eq!(river_width(&w, Pos::new(-5, -5)), 0);
    }

    #[test]
    fn washes_run_unbroken_and_dry() {
        for seed in 0..4 {
            let w = small(seed);
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

    use crate::overworld::{LINK_E, LINK_N, LINK_S, LINK_W};

    #[test]
    fn a_straight_highway_is_straight_one_level_down() {
        let w = small(0);
        let run: Vec<Pos> = w
            .tiles
            .iter()
            .filter(|(_, t)| t.road == LINK_E | LINK_W && t.water == Water::None)
            .map(|(p, _)| p)
            .collect();
        assert!(run.len() > 10, "{} straight tiles", run.len());
        for p in run {
            let g = ground(&w, p.x, p.y);
            // a lake's shore may reach the road, which bridges it, still straight
            let lane = |t: Terrain| matches!(t, Terrain::Road | Terrain::Bridge);
            for x in 0..8 {
                assert!(lane(g[Pos::new(x, 3)].terrain), "{p:?} col {x}");
                assert!(lane(g[Pos::new(x, 4)].terrain), "{p:?} col {x}");
                assert!(!lane(g[Pos::new(x, 2)].terrain), "{p:?} col {x}");
                assert!(!lane(g[Pos::new(x, 5)].terrain), "{p:?} col {x}");
            }
        }
    }

    #[test]
    fn every_road_meets_itself_across_every_edge() {
        let ok = |t: Terrain| matches!(t, Terrain::Road | Terrain::Bridge);
        for seed in 0..6 {
            let w = small(seed);
            for (p, t) in w.tiles.iter() {
                if t.road & LINK_E != 0 && w.tile(p.offset(1, 0)).is_some() {
                    let (a, b) = (ground(&w, p.x, p.y), ground(&w, p.x + 1, p.y));
                    for y in [3, 4] {
                        assert!(
                            ok(a[Pos::new(7, y)].terrain) && ok(b[Pos::new(0, y)].terrain),
                            "seed {seed} {p:?} row {y}"
                        );
                    }
                }
                if t.road & LINK_S != 0 && w.tile(p.offset(0, 1)).is_some() {
                    let (a, b) = (ground(&w, p.x, p.y), ground(&w, p.x, p.y + 1));
                    for x in [3, 4] {
                        assert!(
                            ok(a[Pos::new(x, 7)].terrain) && ok(b[Pos::new(x, 0)].terrain),
                            "seed {seed} {p:?} col {x}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_bridge_carries_the_road_across_the_whole_river() {
        let mut bridges = 0;
        for seed in 0..10 {
            let w = small(seed);
            for (p, t) in w
                .tiles
                .iter()
                .filter(|(_, t)| t.road != 0 && t.water == Water::River)
            {
                let g = ground(&w, p.x, p.y);
                let lanes: Vec<Pos> = match t.road {
                    m if m == LINK_E | LINK_W => (0..8)
                        .flat_map(|x| [Pos::new(x, 3), Pos::new(x, 4)])
                        .collect(),
                    m if m == LINK_N | LINK_S => (0..8)
                        .flat_map(|y| [Pos::new(3, y), Pos::new(4, y)])
                        .collect(),
                    _ => continue,
                };
                for c in lanes {
                    assert!(
                        matches!(g[c].terrain, Terrain::Road | Terrain::Bridge),
                        "seed {seed} {p:?} {c:?}: {:?}",
                        g[c].terrain
                    );
                }
                assert!(
                    g.count(|c| c.terrain == Terrain::Railing) >= 2,
                    "seed {seed} {p:?}: a bridge has rails"
                );
                bridges += 1;
            }
        }
        assert!(bridges >= 3, "{bridges} bridges");
    }

    #[test]
    fn a_highway_rides_over_a_wash_on_a_culvert() {
        let mut seen = 0;
        for seed in 0..8 {
            let w = small(seed);
            for (p, _) in w
                .tiles
                .iter()
                .filter(|(_, t)| t.road != 0 && t.wash != 0 && t.water == Water::None)
            {
                let g = ground(&w, p.x, p.y);
                assert!(
                    g.count(|c| c.terrain == Terrain::Grate) >= 2,
                    "seed {seed} {p:?}: no culvert"
                );
                seen += 1;
            }
        }
        assert!(seen >= 4, "{seen} culverts");
    }

    #[test]
    fn a_road_off_the_map_runs_to_the_edge() {
        for seed in 0..10 {
            let w = small(seed);
            let (mw, mh) = (w.size.w as i32, w.size.h as i32);
            for (p, t) in w.tiles.iter() {
                let g = ground(&w, p.x, p.y);
                let road = |x: i32, y: i32| {
                    matches!(g[Pos::new(x, y)].terrain, Terrain::Road | Terrain::Bridge)
                };
                if p.x == mw - 1 && t.road & LINK_E != 0 {
                    assert!(road(7, 3) && road(7, 4), "seed {seed} {p:?}");
                }
                if p.x == 0 && t.road & LINK_W != 0 {
                    assert!(road(0, 3) && road(0, 4), "seed {seed} {p:?}");
                }
                if p.y == mh - 1 && t.road & LINK_S != 0 {
                    assert!(road(3, 7) && road(4, 7), "seed {seed} {p:?}");
                }
                if p.y == 0 && t.road & LINK_N != 0 {
                    assert!(road(3, 0) && road(4, 0), "seed {seed} {p:?}");
                }
            }
        }
    }

    #[test]
    fn power_lines_run_beside_some_highways() {
        use crate::tile::Feature;
        let w = small(0);
        let g = whole(&w);
        let pylons = g.count(|t| t.feature == Some(Feature::Pylon));
        let line = g.count(|t| matches!(t.feature, Some(Feature::PowerLine(_))));
        assert!(pylons >= 20, "{pylons} pylons");
        assert!(
            line >= 8 * pylons,
            "{line} cells of line for {pylons} pylons"
        );
        // every pylon has its line beside it
        for (p, t) in g.iter() {
            if t.feature == Some(Feature::Pylon) {
                let wired = p.neighbors8().any(|q| {
                    g.get(q).is_some_and(|t| {
                        matches!(t.feature, Some(Feature::PowerLine(_) | Feature::Pylon))
                    })
                });
                assert!(wired, "a lone pylon at {p:?}");
            }
        }
    }

    fn t_of(p: Pos) -> Option<Pos> {
        Some(Pos::new(p.x.div_euclid(8), p.y.div_euclid(8)))
    }

    #[test]
    fn nothing_stands_on_water_or_the_road() {
        for seed in 0..2 {
            let w = small(seed);
            let g = whole(&w);
            assert!(
                g.count(|t| t.feature.is_some()) > 100,
                "seed {seed}: no dressing"
            );
            // a checkpoint's gate stands across its road
            let gate = |p: Pos| {
                t_of(p).is_some_and(|t| {
                    w.pois
                        .iter()
                        .any(|o| o.kind == crate::overworld::PoiKind::Checkpoint && o.at == t)
                })
            };
            for (p, t) in g.iter() {
                if t.feature.is_some() && !(t.feature == Some(Feature::Barrier) && gate(p)) {
                    assert!(
                        !matches!(
                            t.terrain,
                            Terrain::Water
                                | Terrain::Shallows
                                | Terrain::Ford
                                | Terrain::Road
                                | Terrain::Bridge
                                | Terrain::Railing
                                | Terrain::Cliff
                        ),
                        "seed {seed} {p:?}: {:?} on {:?}",
                        t.feature,
                        t.terrain
                    );
                }
            }
        }
    }

    #[test]
    fn detail_fingerprints_are_pinned() {
        // A shared seed must give the same ground on every platform; CI runs this on
        // macOS, Linux and Windows. Update only for a change meant to move the ground.
        let w = Overworld::generate(WorldParams {
            seed: 0,
            ..Default::default()
        })
        .unwrap();
        let pick = |f: &dyn Fn(&WorldTile) -> bool| {
            w.tiles.iter().find(|(_, t)| f(t)).map(|(p, _)| p).unwrap()
        };
        let by_sea = |p: Pos| {
            p.neighbors8()
                .any(|q| w.tile(q).is_some_and(|t| t.water == Water::Sea))
        };
        let near = |f: &dyn Fn(Pos, &WorldTile) -> bool| {
            w.tiles
                .iter()
                .find(|(p, t)| f(*p, t))
                .map(|(p, _)| p)
                .unwrap()
        };
        let tiles = [
            (
                "coast",
                near(&|p, t| t.water == Water::None && t.road == 0 && by_sea(p)),
            ),
            ("mouth", near(&|p, t| t.water == Water::River && by_sea(p))),
            ("river", pick(&|t| t.water == Water::River && t.road == 0)),
            ("bridge", pick(&|t| t.water == Water::River && t.road != 0)),
            ("junction", pick(&|t| t.road.count_ones() >= 3)),
            ("mountains", pick(&|t| t.ground == Ground::Mountains)),
            ("salt", pick(&|t| t.ground == Ground::Salt && t.road == 0)),
        ];
        let got: Vec<(&str, u64)> = tiles
            .iter()
            .map(|(n, p)| (*n, fingerprint(&w, p.x, p.y)))
            .collect();
        assert_eq!(
            got,
            vec![
                ("coast", 4851722150823226721),
                ("mouth", 2172678762758755685),
                ("river", 11201036963274430611),
                ("bridge", 12795655540901140372),
                ("junction", 14601247688220562134),
                ("mountains", 464793460433013172),
                ("salt", 4141889093027210155),
            ]
        );
    }

    #[test]
    fn tiles_build_fast() {
        let w = small(1);
        let start = std::time::Instant::now();
        for ty in 0..w.size.h as i32 {
            for tx in 0..w.size.w as i32 {
                let _ = ground(&w, tx, ty);
            }
        }
        let per = start.elapsed() / (w.size.w * w.size.h);
        assert!(
            per < std::time::Duration::from_millis(3),
            "{per:?} a tile (debug build)"
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
