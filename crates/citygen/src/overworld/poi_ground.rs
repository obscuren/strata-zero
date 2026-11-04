//! POIs one level down: each kind's place laid into the detail patch from the POI's
//! anchor and world-wide hashes alone, so a tile built alone is the same tile in any
//! region. See `2026-11-04-world-tiles-design.md`, "POIs, one level down".

use super::detail::{Patch, TILE, wet_in};
use super::{LINK_E, LINK_N, LINK_S, LINK_W, Overworld, Poi, PoiKind};
use crate::geom::{Pos, Rect};
use crate::rng::hash64;
use crate::tile::{Feature, Furniture, Terrain};

/// How far any POI's layout reaches from its tile's middle, in cells each way.
const REACH: i32 = 40;

/// Lays every POI that reaches the patch.
pub(crate) fn lay(p: &mut Patch) {
    let world = p.world;
    for (i, poi) in world.pois.iter().enumerate() {
        let mid = Pos::new(TILE * poi.at.x + TILE / 2, TILE * poi.at.y + TILE / 2);
        if !p.near(mid, REACH) {
            continue;
        }
        let h = hash64(hash64(world.seed, 3100), i as u64);
        let mut site = Site { p, world, h };
        match poi.kind {
            PoiKind::TruckStop => site.truck_stop(poi),
            PoiKind::Checkpoint => site.checkpoint(poi),
            _ => {}
        }
    }
}

/// One POI being laid: the patch, its world and the POI's own hash.
struct Site<'p, 'w> {
    p: &'p mut Patch<'w>,
    world: &'w Overworld,
    h: u64,
}

/// A road POI's frame: `along` the road and `away` from it, mapped to cells with the
/// screen's aspect kept (along a north-south road a unit is half a row; away from it,
/// two columns).
#[derive(Clone, Copy)]
struct Frame {
    origin: Pos,
    /// The road runs east-west.
    across: bool,
    /// Which side of the road: 1 north (or east), -1 south (or west).
    side: i32,
}

impl Frame {
    /// The frame beside the road on tile `t`, `len` units long centred on the tile, on
    /// `side`; the road's direction from the tile's links (east-west if it has any).
    fn beside(world: &Overworld, t: Pos, len: i32, side: i32) -> Frame {
        let road = world.tile(t).map_or(0, |x| x.road);
        Frame::beside_road(t, road, len, side)
    }

    fn beside_road(t: Pos, road: u8, len: i32, side: i32) -> Frame {
        let across = road & (LINK_E | LINK_W) != 0 || road & (LINK_N | LINK_S) == 0;
        let (x0, y0) = (TILE * t.x, TILE * t.y);
        let origin = if across {
            // the first row past the shoulder (rows 2 and 5), away from the lanes
            Pos::new(
                x0 + TILE / 2 - len / 2,
                if side > 0 { y0 + 1 } else { y0 + 6 },
            )
        } else {
            Pos::new(
                if side > 0 { x0 + 6 } else { x0 + 1 },
                y0 + TILE / 2 - len / 4,
            )
        };
        Frame {
            origin,
            across,
            side,
        }
    }

    /// The same frame `by` units further along the road.
    fn slid(mut self, by: i32) -> Frame {
        if self.across {
            self.origin.x += by;
        } else {
            self.origin.y += by.div_euclid(2);
        }
        self
    }

    /// The cell at `u` along and `v` away.
    fn at(&self, u: i32, v: i32) -> Pos {
        if self.across {
            Pos::new(self.origin.x + u, self.origin.y - self.side * v)
        } else {
            Pos::new(
                self.origin.x + self.side * 2 * v,
                self.origin.y + u.div_euclid(2),
            )
        }
    }

    /// The cells of `w` along by `h` away from `(u, v)`, as a rectangle of cells.
    fn rect(&self, u: i32, v: i32, w: i32, h: i32) -> Rect {
        let (a, b) = (self.at(u, v), self.at(u + w - 1, v + h - 1));
        let (b, extra) = if self.across {
            (b, (0, 0))
        } else {
            // two columns a unit away, so the far unit's second column too
            (b, (self.side.max(0), (-self.side).max(0)))
        };
        let (x0, x1) = (a.x.min(b.x) - extra.1, a.x.max(b.x) + extra.0);
        let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
        Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1)
    }
}

/// The road tile a POI on `t` faces: its own if it carries a road, else the nearest of
/// its four neighbours that does; and the side of that road the POI's tile lies on (1
/// north or east of it, -1 south or west), or `prefer` on its own road.
fn facing(world: &Overworld, t: Pos, prefer: i32) -> (Pos, i32) {
    let road = |p: Pos| world.tile(p).is_some_and(|x| x.road != 0);
    if road(t) {
        return (t, prefer);
    }
    for (d, side) in [((0, 1), 1), ((0, -1), -1), ((1, 0), -1), ((-1, 0), 1)] {
        let n = t.offset(d.0, d.1);
        if road(n) {
            return (n, side);
        }
    }
    (t, prefer)
}

/// Whether cell `q` is on a road's lanes or shoulders, from the overworld's links (so
/// every patch agrees).
fn on_road(world: &Overworld, q: Pos) -> bool {
    let t = Pos::new(q.x.div_euclid(TILE), q.y.div_euclid(TILE));
    let Some(m) = world.tile(t).map(|x| x.road).filter(|m| *m != 0) else {
        return false;
    };
    let (x, y) = (q.x.rem_euclid(TILE), q.y.rem_euclid(TILE));
    let band = |c: i32| (2..=5).contains(&c);
    (band(y) && (m & LINK_E != 0 && x >= 3 || m & LINK_W != 0 && x <= 4))
        || (band(x) && (m & LINK_S != 0 && y >= 3 || m & LINK_N != 0 && y <= 4))
}

/// The cells of `r`.
fn cells(r: Rect) -> impl Iterator<Item = Pos> {
    (r.y..r.y + r.h).flat_map(move |y| (r.x..r.x + r.w).map(move |x| Pos::new(x, y)))
}

impl Site<'_, '_> {
    fn roll(&self, k: u64) -> u64 {
        hash64(self.h, k)
    }

    /// Whether `q` may be built on: in the world, not water, not a road's lanes.
    fn open(&self, q: Pos) -> bool {
        self.p.in_world(q)
            && self.p.wet_at(q) == 0
            && self.p.get(q).is_some_and(|t| {
                !matches!(
                    t.terrain,
                    Terrain::Road | Terrain::Bridge | Terrain::Railing | Terrain::Water
                )
            })
    }

    fn ground(&mut self, r: Rect, t: Terrain) {
        for q in cells(r) {
            if self.open(q) {
                self.p.set(q, t);
            }
        }
    }

    fn put(&mut self, q: Pos, f: Feature) {
        if self.open(q) {
            self.p.set_feature(q, f);
        }
    }

    /// A fence round `r`, with a gap at `gap` (if any).
    fn fence(&mut self, r: Rect, gap: Option<Rect>) {
        for q in cells(r) {
            let edge = q.x == r.x || q.y == r.y || q.x == r.x + r.w - 1 || q.y == r.y + r.h - 1;
            if edge && gap.is_none_or(|g| !g.contains(q)) && self.open(q) {
                self.p.set(q, Terrain::Railing);
            }
        }
    }

    /// A room: walls round `r` with windows on the side facing `front`, a door at
    /// `door`, floor and `inside` within. Left out where its footprint meets water.
    fn room(&mut self, r: Rect, door: Pos, windows: &[Pos], inside: &[(Pos, Feature)]) {
        if !self.clear(r) {
            return;
        }
        for q in cells(r) {
            let edge = q.x == r.x || q.y == r.y || q.x == r.x + r.w - 1 || q.y == r.y + r.h - 1;
            if !self.p.in_world(q) {
                continue;
            }
            let t = if q == door {
                Terrain::Door
            } else if edge && windows.contains(&q) {
                Terrain::Window
            } else if edge {
                Terrain::Wall
            } else {
                Terrain::Floor
            };
            self.p.set(q, t);
            self.p.clear_feature(q);
        }
        for (q, f) in inside {
            if r.contains(*q) {
                self.p.set_feature(*q, *f);
            }
        }
    }

    /// Whether a building may stand on `r`: no water in or round it, no road through it.
    fn clear(&self, r: Rect) -> bool {
        !wet_in(self.world, r) && !cells(r).any(|q| on_road(self.world, q))
    }

    /// The truck stop (round 2's Kilo-9): a forecourt against the road with pumps under
    /// a canopy, the store behind it, a fenced back lot of parked rigs beside the store,
    /// lamps, a camera on the pumps and a sign by the road.
    fn truck_stop(&mut self, poi: &Poi) {
        let prefer = if self.roll(1).is_multiple_of(2) {
            1
        } else {
            -1
        };
        let (road, side) = facing(self.world, poi.at, prefer);
        // which end the lot is at
        let lot_first = self.roll(2).is_multiple_of(2);
        let (lot_u, store_u) = if lot_first { (0, 14) } else { (24, 4) };
        let store_w = 15 + (self.roll(3) % 5) as i32;
        // where the store can stand: the POI's own side first, then the other, then
        // slid along the road onto dry ground
        let frame =
            |(side, slide): (i32, i32)| Frame::beside(self.world, road, 40, side).slid(slide);
        let tries = [0, -16, 16, -32, 32]
            .into_iter()
            .flat_map(|slide| [(side, slide), (-side, slide)]);
        let Some(f) = tries
            .map(frame)
            .find(|f| self.clear(f.rect(store_u, 6, store_w, 7)))
        else {
            return;
        };
        // the forecourt and its canopy over the pumps
        self.ground(f.rect(0, 0, 40, 6), Terrain::Plaza);
        let pumps = 3 + (self.roll(4) % 3) as i32;
        let first = 20 - 2 * pumps;
        for k in 0..pumps {
            for v in [1, 3] {
                self.put(
                    f.at(first + 4 * k, v),
                    Feature::Furniture(Furniture::Vending),
                );
            }
        }
        for u in first - 2..first + 4 * pumps - 1 {
            self.put(f.at(u, 5), Feature::Awning);
        }
        // the store, its door onto the forecourt
        let room = f.rect(store_u, 6, store_w, 7);
        let door = f.at(store_u + store_w / 2, 6);
        let front: Vec<Pos> = (store_u + 2..store_u + store_w - 2)
            .filter(|u| u % 3 == 0)
            .map(|u| f.at(u, 6))
            .collect();
        let inside: Vec<(Pos, Feature)> = (store_u + 2..store_u + store_w - 2)
            .flat_map(|u| {
                [
                    (f.at(u, 9), Feature::Furniture(Furniture::Shelf)),
                    (f.at(u, 11), Feature::Furniture(Furniture::Shelf)),
                ]
            })
            .filter(|(q, _)| q.x % 4 != 0)
            .chain([(f.at(store_u + 2, 8), Feature::Furniture(Furniture::Counter))])
            .collect();
        self.room(room, door, &front, &inside);
        // the back lot: gravel, parked rigs, a fence with its gate on the forecourt
        let lot = f.rect(lot_u, 6, 14, 9);
        self.ground(lot, Terrain::Gravel);
        for (k, v) in [8, 10, 12].into_iter().enumerate() {
            if !self.roll(10 + k as u64).is_multiple_of(4) {
                for u in lot_u + 2..lot_u + 10 {
                    self.put(f.at(u, v), Feature::Furniture(Furniture::Couch));
                }
            }
        }
        let gate = f.rect(lot_u + 5, 6, 3, 1);
        self.fence(f.rect(lot_u - 1, 6, 16, 10), Some(gate));
        // lamps, the pump camera, a vent, litter
        for (u, v) in [
            (4, 0),
            (14, 0),
            (24, 0),
            (34, 0),
            (store_u + store_w / 2 - 1, 5),
        ] {
            self.put(f.at(u, v), Feature::Lamp);
        }
        self.put(f.at(first + 4 * pumps, 4), Feature::Camera);
        self.put(f.at(store_u + store_w - 3, 12), Feature::Vent);
        for k in 0..4 {
            let r = self.roll(20 + k);
            self.put(f.at((r % 40) as i32, ((r >> 8) % 6) as i32), Feature::Trash);
        }
        // the sign by the road: the stop's name on posts
        let name: String = poi
            .name
            .split_whitespace()
            .next()
            .unwrap_or("STOP")
            .to_uppercase();
        for (k, ch) in name.chars().take(8).enumerate() {
            self.put(f.at(41 + k as i32, 2), Feature::Sign(ch));
        }
        self.put(f.at(43, 0), Feature::Pipe);
        self.put(f.at(43, 1), Feature::Pipe);
    }

    /// The checkpoint: a gate across one lane with the other open, a booth on each side
    /// of the road with its door to it, floodlights, cameras, and a fence running off
    /// into the ground both ways.
    fn checkpoint(&mut self, poi: &Poi) {
        for side in [1, -1] {
            let f = Frame::beside(self.world, poi.at, 8, side);
            let room = f.rect(2, 1, 4, 3);
            let door = f.at(3, 1);
            let window = [f.at(4, 1)];
            self.room(
                room,
                door,
                &window,
                &[(f.at(3, 2), Feature::Furniture(Furniture::Desk))],
            );
            self.put(f.at(0, 0), Feature::Lamp);
            self.put(f.at(7, 0), Feature::Camera);
            // the fence off into the ground
            let len = 8 + (self.roll(if side > 0 { 1 } else { 2 }) % 6) as i32;
            for v in 0..len {
                let q = f.at(-1, v);
                if self.open(q) {
                    self.p.set(q, Terrain::Railing);
                }
                if !f.across {
                    let q2 = Pos::new(q.x + side, q.y);
                    if self.open(q2) {
                        self.p.set(q2, Terrain::Railing);
                    }
                }
            }
        }
        // the gate: a barrier across one lane and its shoulder
        let f = Frame::beside(self.world, poi.at, 8, 1);
        let t = poi.at;
        let (x0, y0) = (TILE * t.x, TILE * t.y);
        let gate: Vec<Pos> = if f.across {
            (2..=3).map(|y| Pos::new(f.at(-2, 0).x, y0 + y)).collect()
        } else {
            (2..=3).map(|x| Pos::new(x0 + x, f.at(-2, 0).y)).collect()
        };
        for q in gate {
            if self.p.in_world(q) && self.p.get(q).is_some() {
                self.p.set_feature(q, Feature::Barrier);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Grid;
    use crate::geom::Pos;
    use crate::overworld::detail::{TILE, ground, region};
    use crate::overworld::{Overworld, Poi, PoiKind, WorldParams, WorldSize};
    use crate::tile::{Feature, Furniture, Terrain, Tile};

    fn small(seed: u64) -> Overworld {
        Overworld::generate(WorldParams {
            seed,
            size: WorldSize::Small,
            ..Default::default()
        })
        .unwrap()
    }

    /// A POI of `kind` in the first of seeds 0.. that has one, and its world.
    fn find(kind: PoiKind) -> (Overworld, Poi) {
        for seed in 0..20 {
            let w = small(seed);
            if let Some(p) = w.pois.iter().find(|p| p.kind == kind).cloned() {
                return (w, p);
            }
        }
        panic!("no {kind:?}")
    }

    /// The cells round a POI's tile, `r` tiles each way, and the region's origin.
    fn round(w: &Overworld, poi: &Poi, r: i32) -> (Pos, Grid<Tile>) {
        let o = Pos::new(TILE * (poi.at.x - r), TILE * (poi.at.y - r));
        let n = (TILE * (2 * r + 1)) as u32;
        (o, region(w, o.x, o.y, n, n))
    }

    fn count(g: &Grid<Tile>, f: impl Fn(&Tile) -> bool) -> usize {
        g.count(|t| f(t))
    }

    #[test]
    #[ignore]
    fn dbg_stop() {
        let w = small(3);
        let poi = w
            .pois
            .iter()
            .find(|p| p.kind == PoiKind::TruckStop)
            .cloned()
            .unwrap();
        let (_, g) = round(&w, &poi, 3);
        eprintln!("{poi:?} road {:04b}", w.tile(poi.at).unwrap().road);
        for y in 0..g.size().h as i32 {
            let row: String = (0..g.size().w as i32)
                .map(|x| {
                    let t = g[Pos::new(x, y)];
                    match (t.feature, t.terrain) {
                        (Some(Feature::Furniture(Furniture::Vending)), _) => 'P',
                        (Some(Feature::Awning), _) => '^',
                        (Some(Feature::Furniture(Furniture::Couch)), _) => '=',
                        (Some(Feature::Furniture(_)), _) => 'f',
                        (Some(Feature::Lamp), _) => 'L',
                        (Some(Feature::Camera), _) => 'C',
                        (Some(Feature::Sign(c)), _) => c,
                        (_, Terrain::Road) => '#',
                        (_, Terrain::Plaza) => '_',
                        (_, Terrain::Wall) => 'W',
                        (_, Terrain::Window) => 'w',
                        (_, Terrain::Door) => 'D',
                        (_, Terrain::Floor) => ' ',
                        (_, Terrain::Gravel) => ':',
                        (_, Terrain::Railing) => '|',
                        (_, Terrain::Water | Terrain::Shallows) => '~',
                        _ => '.',
                    }
                })
                .collect();
            eprintln!("{row}");
        }
    }

    #[test]
    fn a_truck_stop_has_pumps_a_store_and_a_lot() {
        let (w, poi) = find(PoiKind::TruckStop);
        let (_, g) = round(&w, &poi, 3);
        let pumps = count(&g, |t| {
            t.feature == Some(Feature::Furniture(Furniture::Vending))
        });
        assert!(pumps >= 4, "{pumps} pumps");
        assert!(
            count(&g, |t| t.feature == Some(Feature::Awning)) >= 6,
            "a canopy"
        );
        assert!(count(&g, |t| t.terrain == Terrain::Door) >= 1, "a door");
        assert!(
            count(&g, |t| t.terrain == Terrain::Floor) >= 20,
            "a store's floor"
        );
        assert!(
            count(&g, |t| t.terrain == Terrain::Railing) >= 10,
            "the lot's fence"
        );
        assert!(
            count(&g, |t| t.feature == Some(Feature::Lamp)) >= 2,
            "lamps"
        );
        assert!(
            count(&g, |t| t.feature == Some(Feature::Camera)) >= 1,
            "a camera"
        );
    }

    #[test]
    fn the_stores_door_is_reached_from_the_road() {
        for seed in 0..12 {
            let w = small(seed);
            for poi in w.pois.iter().filter(|p| p.kind == PoiKind::TruckStop) {
                let (o, g) = round(&w, poi, 3);
                let door = g
                    .iter()
                    .find(|(_, t)| t.terrain == Terrain::Door)
                    .map(|(p, _)| p);
                let Some(door) = door else {
                    panic!("seed {seed} {poi:?}: no door")
                };
                // flood from the road over walkable cells
                let mut seen = Grid::filled(g.size(), false);
                let mut stack: Vec<Pos> = g
                    .iter()
                    .filter(|(_, t)| t.terrain == Terrain::Road)
                    .map(|(p, _)| p)
                    .collect();
                for p in &stack {
                    seen[*p] = true;
                }
                while let Some(p) = stack.pop() {
                    for q in p.neighbors8() {
                        if g.get(q).is_some_and(|t| t.is_walkable()) && !seen[q] {
                            seen[q] = true;
                            stack.push(q);
                        }
                    }
                }
                assert!(
                    seen[door],
                    "seed {seed} {poi:?} door at {:?}",
                    door.offset(o.x, o.y)
                );
            }
        }
    }

    #[test]
    fn a_checkpoint_gates_its_road() {
        let (w, poi) = find(PoiKind::Checkpoint);
        let g = ground(&w, poi.at.x, poi.at.y);
        let (_, around) = round(&w, &poi, 2);
        assert!(
            count(&around, |t| t.feature == Some(Feature::Barrier)) >= 2,
            "a gate"
        );
        assert!(
            count(&around, |t| t.terrain == Terrain::Door) >= 2,
            "booths both sides"
        );
        assert!(
            count(&around, |t| t.terrain == Terrain::Railing) >= 12,
            "a fence off it"
        );
        assert!(
            count(&g, |t| t.terrain == Terrain::Road) >= 8,
            "the road runs on"
        );
    }

    #[test]
    fn a_poi_tile_alone_equals_its_region() {
        for kind in [PoiKind::TruckStop, PoiKind::Checkpoint] {
            let (w, poi) = find(kind);
            let (o, r) = round(&w, &poi, 2);
            for dy in -2..=2 {
                for dx in -2..=2 {
                    let t = poi.at.offset(dx, dy);
                    let g = ground(&w, t.x, t.y);
                    for (c, a) in g.iter() {
                        let q = Pos::new(TILE * t.x + c.x - o.x, TILE * t.y + c.y - o.y);
                        let b = r[q];
                        assert_eq!(
                            (a.terrain, a.feature),
                            (b.terrain, b.feature),
                            "{kind:?} {t:?} {c:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn road_pois_keep_their_shape_on_a_north_south_road() {
        use crate::overworld::{LINK_E, LINK_N, LINK_S, LINK_W};
        // the forecourt, as it looks on screen (a row is two columns), either way
        let looks = |road: u8, side: i32| {
            let f = Frame::beside_road(Pos::new(10, 10), road, 40, side);
            let r = f.rect(0, 0, 40, 6);
            let (w, h) = (r.w as f32, 2.0 * r.h as f32);
            if road & (LINK_E | LINK_W) != 0 {
                w / h
            } else {
                h / w
            }
        };
        for side in [1, -1] {
            let (a, b) = (looks(LINK_E | LINK_W, side), looks(LINK_N | LINK_S, side));
            assert!(
                a > 2.0 && (a - b).abs() / a < 0.15,
                "side {side}: {a} against {b}"
            );
        }
        // and it sits against the road's shoulder on the side asked for
        let f = Frame::beside_road(Pos::new(10, 10), LINK_N | LINK_S, 40, 1);
        assert_eq!(f.at(0, 0).x, 8 * 10 + 6, "east of the lanes");
        let f = Frame::beside_road(Pos::new(10, 10), LINK_N | LINK_S, 40, -1);
        assert_eq!(f.at(0, 0).x, 8 * 10 + 1, "west of the lanes");
    }
}
