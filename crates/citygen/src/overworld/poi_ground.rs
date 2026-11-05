//! POIs one level down: each kind's place laid into the detail patch from the POI's
//! anchor and world-wide hashes alone, so a tile built alone is the same tile in any
//! region. See `2026-11-04-world-tiles-design.md`, "POIs, one level down".

use super::detail::{Patch, TILE, wet_in};
use super::{LINK_E, LINK_N, LINK_S, LINK_W, Overworld, Poi, PoiKind, Water};
use crate::geom::{Pos, Rect};
use crate::rng::hash64;
use crate::tile::{Feature, Furniture, Terrain};

/// How far any POI's layout reaches from its tile's middle, in cells each way.
const REACH: i32 = 80;

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
            PoiKind::Relay => site.relay(poi),
            PoiKind::Camp => site.camp(poi),
            PoiKind::Wreck => site.wreck(poi),
            PoiKind::Arcology => site.arcology(poi),
            PoiKind::Solar => site.solar(poi),
            PoiKind::WaterPlant => site.water_plant(poi),
            PoiKind::TestRange => site.test_range(poi),
            PoiKind::Rig => site.rig(poi),
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

/// How far round a sign a way past it is looked for.
const SIGN_ROUND: i32 = 3;

/// Whether `q` of patch `p` may be built on: in the world, not water, not a road's lanes.
fn open_in(p: &Patch, q: Pos) -> bool {
    p.in_world(q)
        && p.wet_at(q) == 0
        && p.get(q).is_some_and(|t| {
            !matches!(
                t.terrain,
                Terrain::Road
                    | Terrain::Bridge
                    | Terrain::Railing
                    | Terrain::Water
                    | Terrain::Cliff
                    | Terrain::Wall
                    | Terrain::Window
                    | Terrain::Door
                    | Terrain::Floor
                    | Terrain::Mass
            )
        })
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
        open_in(self.p, q)
    }

    /// Signs stood up where they part no way through. Each of `signs` is its spots in
    /// turn (every cell of a spot with what stands on it, posts and all): the first spot
    /// that cuts nothing is taken, and a sign none of whose spots will do is left out.
    /// They're tried against the ground round them as it lies with no POI's sign on it
    /// (but those taken before), built alone, so every patch that lays the POI decides
    /// the same.
    fn signs(&mut self, signs: &[Vec<Vec<(Pos, Feature)>>]) {
        if !self.p.signs {
            return;
        }
        let cells_of = |spot: &[(Pos, Feature)]| spot.iter().map(|(q, _)| *q).collect::<Vec<_>>();
        // the ground round every sign's first spot, built once; round a later spot
        // only when one past it is tried
        let firsts: Vec<Pos> = signs
            .iter()
            .filter_map(|spots| spots.first())
            .flat_map(|spot| cells_of(spot))
            .collect();
        if firsts.is_empty() {
            return;
        }
        let mut core = Rect::around(&firsts).grow(SIGN_ROUND);
        let mut ground = super::detail::signless(self.world, core);
        let mut taken: Vec<Pos> = Vec::new();
        for spots in signs {
            for spot in spots {
                let at = cells_of(spot);
                let window = Rect::around(&at).grow(SIGN_ROUND);
                if window.intersection(core) != Some(window) {
                    core = window;
                    ground = super::detail::signless(self.world, core);
                }
                // the cells it would stand on, as `put` lays it
                let solid: Vec<Pos> = at.into_iter().filter(|q| open_in(&ground, *q)).collect();
                let walkable =
                    |q: Pos| !taken.contains(&q) && ground.get(q).is_some_and(|t| t.is_walkable());
                if crate::pathing::cuts(window, walkable, &solid) {
                    continue;
                }
                for (q, f) in spot {
                    self.put(*q, *f);
                }
                taken.extend(solid);
                break;
            }
        }
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

    /// A two-cell gate in `pen`'s fence: on its bottom side near `x`, else its top, on a
    /// stretch the world has dry and in the world (decided from the overworld's tiles,
    /// so every patch picks the same).
    fn gate(&self, pen: Rect, x: i32) -> Rect {
        let dry = |q: Pos| {
            let t = Pos::new(q.x.div_euclid(TILE), q.y.div_euclid(TILE));
            self.p.in_world(q)
                && self
                    .world
                    .tile(t)
                    .is_some_and(|t| t.water == Water::None && t.river == 0)
        };
        let rows = [pen.y + pen.h - 1, pen.y];
        for y in rows {
            for d in 0..pen.w - 3 {
                for gx in [x + d, x - d] {
                    let ok = gx > pen.x && gx + 1 < pen.x + pen.w - 1;
                    if ok && dry(Pos::new(gx, y)) && dry(Pos::new(gx + 1, y)) {
                        return Rect::new(gx, y, 2, 1);
                    }
                }
            }
        }
        Rect::new(x, pen.y + pen.h - 1, 2, 1)
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
            // gaps between the shelves along each aisle (a row on a north-south road)
            .filter(|(q, _)| (if f.across { q.x } else { q.y }) % 4 != 0)
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
        // the sign by the road: the stop's name between two caps, on posts; past the
        // forecourt's far end, else past its near end where it would cut a way there
        let name: String = poi
            .name
            .split_whitespace()
            .next()
            .unwrap_or("STOP")
            .to_uppercase();
        let text: Vec<char> = std::iter::once('▣')
            .chain(name.chars().take(8))
            .chain(std::iter::once('▣'))
            .collect();
        // a letter a cell either way the road runs (a unit is half a row along a
        // north-south one)
        let step = if f.across { 1 } else { 2 };
        let n = text.len() as i32;
        let spot = |first: i32, post: i32| -> Vec<(Pos, Feature)> {
            let letters = text
                .iter()
                .enumerate()
                .map(|(k, ch)| (f.at(first + step * k as i32, 2), Feature::Sign(*ch)));
            let posts = [0, 1].map(|v| (f.at(post, v), Feature::Pipe));
            letters.chain(posts).collect()
        };
        self.signs(&[vec![spot(41, 43), spot(-2 - step * (n - 1), -4)]]);
    }

    /// The checkpoint: a gate across one lane with the other open, a booth on each side
    /// of the road with its door to it, floodlights, cameras, and a fence running off
    /// into the ground both ways.
    fn checkpoint(&mut self, poi: &Poi) {
        for side in [1, -1] {
            let f = Frame::beside(self.world, poi.at, 8, side);
            // three rows inside either way the road runs (a unit along a north-south road
            // is half a row)
            let along = if f.across { 4 } else { 6 };
            let room = f.rect(2, 1, along, 3);
            let door = f.at(2 + along / 2, 1);
            let window = [f.at(2 + along / 2 + 1, 1)];
            self.room(
                room,
                door,
                &window,
                &[(f.at(2 + along / 2, 2), Feature::Furniture(Furniture::Desk))],
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

    /// Where an off-road POI centres: its tile's middle, or beside the road when its
    /// tile carries one (six rows off an east-west road, twelve columns off a
    /// north-south one, the side by the POI's hash).
    fn centre(&self, poi: &Poi) -> Pos {
        let mid = Pos::new(TILE * poi.at.x + TILE / 2, TILE * poi.at.y + TILE / 2);
        let road = self.world.tile(poi.at).map_or(0, |t| t.road);
        let side = if self.roll(99).is_multiple_of(2) {
            1
        } else {
            -1
        };
        if road & (LINK_E | LINK_W) != 0 {
            mid.offset(0, 6 * side)
        } else if road & (LINK_N | LINK_S) != 0 {
            mid.offset(12 * side, 0)
        } else {
            mid
        }
    }

    /// `w` columns by `h` rows centred on `c`.
    fn centred(c: Pos, w: i32, h: i32) -> Rect {
        Rect::new(c.x - w / 2, c.y - h / 2, w, h)
    }

    /// A solid block of `t` (a hull, a tank, a megastructure), where it can stand.
    fn block(&mut self, r: Rect, t: Terrain) -> bool {
        if !self.clear(r) {
            return false;
        }
        for q in cells(r) {
            if self.p.in_world(q) {
                self.p.set(q, t);
                self.p.clear_feature(q);
            }
        }
        true
    }

    /// The nearest road lane cell within `reach` tiles of `from`'s tile that a track
    /// from `from` (across, then along) reaches without crossing water, from the
    /// overworld.
    fn lane_near(&self, from: Pos, reach: i32) -> Option<Pos> {
        let t = Pos::new(from.x.div_euclid(TILE), from.y.div_euclid(TILE));
        let dry = |a: Pos, b: Pos| {
            let (lo, hi) = (
                Pos::new(a.x.min(b.x), a.y.min(b.y)),
                Pos::new(a.x.max(b.x), a.y.max(b.y)),
            );
            (lo.y.div_euclid(TILE)..=hi.y.div_euclid(TILE)).all(|ty| {
                (lo.x.div_euclid(TILE)..=hi.x.div_euclid(TILE)).all(|tx| {
                    // only the tiles the track's two legs cross
                    let on_leg = tx == a.x.div_euclid(TILE) || ty == b.y.div_euclid(TILE);
                    !on_leg
                        || self
                            .world
                            .tile(Pos::new(tx, ty))
                            .is_some_and(|x| x.water == Water::None && x.river == 0)
                })
            })
        };
        (1..=reach).find_map(|r| {
            (-r..=r)
                .flat_map(|dy| (-r..=r).map(move |dx| (dx, dy)))
                .filter(|&(dx, dy)| dx.abs().max(dy.abs()) == r)
                .map(|(dx, dy)| t.offset(dx, dy))
                .filter(|n| self.world.tile(*n).is_some_and(|x| x.road != 0))
                .map(|n| Pos::new(TILE * n.x + 3, TILE * n.y + 3))
                .find(|lane| dry(from, *lane))
        })
    }

    /// A gravel track from `from` to `to`, across to `to`'s row then along it, on open
    /// ground only.
    fn track(&mut self, from: Pos, to: Pos) {
        let bend = Pos::new(from.x, to.y);
        let line = |a: Pos, b: Pos| -> Vec<Pos> {
            let n = (b.x - a.x).abs().max((b.y - a.y).abs());
            (0..=n)
                .map(|k| {
                    Pos::new(
                        a.x + (b.x - a.x).signum() * k.min((b.x - a.x).abs()),
                        a.y + (b.y - a.y).signum() * k.min((b.y - a.y).abs()),
                    )
                })
                .collect()
        };
        for q in line(from, bend).into_iter().chain(line(bend, to)) {
            if self.open(q) && self.p.get(q).is_some_and(|t| t.feature.is_none()) {
                self.p.set(q, Terrain::Gravel);
            }
        }
    }

    /// A relay mast: fenced, on its hill, with an equipment cabinet and a service track
    /// down to the nearest road.
    fn relay(&mut self, poi: &Poi) {
        let c = self.centre(poi);
        let pen = Self::centred(c, 12, 6);
        let lane = self.lane_near(c, 3);
        // the gate on the side facing the road
        let north = lane.is_some_and(|l| l.y < c.y);
        let gate_y = if north { pen.y } else { pen.y + pen.h - 1 };
        let gate = Rect::new(c.x, gate_y, 2, 1);
        self.ground(
            Rect::new(pen.x + 1, pen.y + 1, pen.w - 2, pen.h - 2),
            Terrain::Gravel,
        );
        self.fence(pen, Some(gate));
        self.ground(gate, Terrain::Gravel);
        self.put(c, Feature::Pylon);
        self.put(c.offset(1, 0), Feature::Pylon);
        self.put(c.offset(-3, 1), Feature::Terminal);
        self.put(c.offset(3, -1), Feature::Lamp);
        if let Some(lane) = lane {
            // out of the gate toward the road's row, then along to the road
            let out = Pos::new(c.x, if north { pen.y - 1 } else { pen.y + pen.h });
            self.track(out, lane);
        }
    }

    /// A nomad camp: vehicles drawn up in a ring, tents inside, fire barrels.
    fn camp(&mut self, poi: &Poi) {
        let c = self.centre(poi);
        let n = 7 + (self.roll(1) % 4) as i32;
        for k in 0..n {
            // round the ring at even angles, a vehicle three cells long
            let a = k as f32 / n as f32 * std::f32::consts::TAU;
            let (cx, cy) = (super::field::cos(a), super::field::sin(a));
            let at = Pos::new(
                c.x + (10.0 * cx).round() as i32,
                c.y + (5.0 * cy).round() as i32,
            );
            for d in -1..=1 {
                self.put(at.offset(d, 0), Feature::Furniture(Furniture::Couch));
            }
        }
        for k in 0..(3 + self.roll(2) % 3) {
            let r = self.roll(10 + k);
            let at = c.offset((r % 11) as i32 - 5, ((r >> 8) % 5) as i32 - 2);
            self.put(at, Feature::Stall);
        }
        self.put(c.offset(-1, 0), Feature::Lamp);
        self.put(c.offset(3, 1), Feature::Lamp);
        for k in 0..5 {
            let r = self.roll(30 + k);
            self.put(
                c.offset((r % 17) as i32 - 8, ((r >> 8) % 7) as i32 - 3),
                Feature::Trash,
            );
        }
    }

    /// A wreck: a crashed hull and its debris field, scorched ground round it.
    fn wreck(&mut self, poi: &Poi) {
        let c = self
            .centre(poi)
            .offset((self.roll(1) % 3) as i32 - 1, (self.roll(2) % 3) as i32 - 1);
        let long = 5 + (self.roll(3) % 4) as i32;
        let hull = if self.roll(4).is_multiple_of(2) {
            Self::centred(c, long, 2)
        } else {
            Self::centred(c, 3, long / 2 + 1)
        };
        self.block(hull, Terrain::Mass);
        for k in 0..24 {
            let r = self.roll(10 + k);
            let at = c.offset((r % 19) as i32 - 9, ((r >> 8) % 9) as i32 - 4);
            if hull.contains(at) {
                continue;
            }
            // two pieces in three are wreckage, the rest litter
            if !((r >> 20) % 3).is_multiple_of(3) {
                if self.open(at) {
                    self.p.set(at, Terrain::Rock);
                }
            } else {
                self.put(at, Feature::Trash);
            }
        }
        for k in 0..6 {
            let r = self.roll(40 + k);
            let at = c.offset((r % 9) as i32 - 4, ((r >> 8) % 5) as i32 - 2);
            if !hull.contains(at) && self.open(at) {
                self.p.set(at, Terrain::Spill);
            }
        }
    }

    /// The base of a dead arcology: a sealed mass, its wall's gate shut, rubble.
    fn arcology(&mut self, poi: &Poi) {
        let c = self.centre(poi);
        let base = Self::centred(c, 30, 12);
        if self.block(base, Terrain::Mass) {
            // a rim of wall, and the shut gate on its south face
            for q in cells(base) {
                let rim = q.x == base.x
                    || q.x == base.x + base.w - 1
                    || q.y == base.y
                    || q.y == base.y + base.h - 1;
                if rim && self.p.in_world(q) {
                    self.p.set(q, Terrain::Wall);
                }
            }
            for x in c.x - 2..c.x + 2 {
                let q = Pos::new(x, base.y + base.h - 1);
                if self.p.get(q).is_some() {
                    self.p.set_feature(q, Feature::Barrier);
                }
            }
        }
        for k in 0..30 {
            let r = self.roll(10 + k);
            let at = Pos::new(
                base.x - 4 + (r % (base.w as u64 + 8)) as i32,
                base.y + base.h + ((r >> 12) % 4) as i32,
            );
            if self.open(at) {
                self.p.set(at, Terrain::Rock);
            }
        }
    }

    /// A solar array: rows of panels in a fence, a maintenance shed, a lamp at the gate.
    fn solar(&mut self, poi: &Poi) {
        let c = self.centre(poi);
        let pen = Self::centred(c, 28, 11);
        self.ground(
            Rect::new(pen.x + 1, pen.y + 1, pen.w - 2, pen.h - 2),
            Terrain::Gravel,
        );
        let gate = self.gate(pen, pen.x + 2);
        self.fence(pen, Some(gate));
        for y in (pen.y + 2..pen.y + pen.h - 2).step_by(2) {
            for x in pen.x + 7..pen.x + pen.w - 2 {
                self.put(Pos::new(x, y), Feature::AcUnit);
            }
        }
        let shed = Rect::new(pen.x + 1, pen.y + 1, 5, 4);
        self.room(shed, Pos::new(pen.x + 3, pen.y + 4), &[], &[]);
        self.put(Pos::new(pen.x + 1, pen.y + pen.h), Feature::Lamp);
    }

    /// A water plant by its lake or river: tanks, a pump house, pipes running to the
    /// water, a fence.
    fn water_plant(&mut self, poi: &Poi) {
        let c = self.centre(poi);
        // the way to the water, from the overworld
        let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        let to_water = dirs
            .into_iter()
            .find(|d| {
                self.world
                    .tile(poi.at.offset(d.0, d.1))
                    .is_some_and(|t| t.water != Water::None)
            })
            .unwrap_or((1, 0));
        let pen = Self::centred(c.offset(-4 * to_water.0, -2 * to_water.1), 22, 9);
        let gate = self.gate(pen, pen.x + pen.w / 2);
        self.fence(pen, Some(gate));
        let mut tanks = Vec::new();
        for k in 0..3 {
            let t = Rect::new(pen.x + 2 + 5 * k, pen.y + 2, 3, 2);
            if self.block(t, Terrain::Mass) {
                tanks.push(t);
            }
        }
        let house = Rect::new(pen.x + pen.w - 7, pen.y + pen.h - 6, 6, 4);
        self.room(
            house,
            // its door on the yard's open side, toward the gate
            Pos::new(house.x, house.y + 2),
            &[],
            &[(
                Pos::new(house.x + 2, house.y + 2),
                Feature::Furniture(Furniture::Machine),
            )],
        );
        // the pipe along the tanks (with gaps to walk through) and on to the water from
        // the house's side that faces it
        let y = pen.y + 5;
        for x in pen.x + 2..pen.x + pen.w - 8 {
            if (x - pen.x) % 4 != 3 {
                self.put(Pos::new(x, y), Feature::Pipe);
            }
        }
        let from = match to_water {
            (1, _) => Pos::new(house.x + house.w, house.y + 1),
            (-1, _) => Pos::new(pen.x - 1, y),
            (_, 1) => Pos::new(house.x + 2, pen.y + pen.h),
            _ => Pos::new(pen.x + 2, pen.y - 1),
        };
        for k in 0..14 {
            self.put(
                from.offset(to_water.0 * k, to_water.1 * k / 2),
                Feature::Pipe,
            );
        }
        self.put(Pos::new(pen.x, pen.y), Feature::Lamp);
    }

    /// A test range on or by dead ground: a long fence with warning signs, craters, a
    /// watchtower.
    fn test_range(&mut self, poi: &Poi) {
        let c = self.centre(poi);
        let pen = Self::centred(c, 30, 13);
        let gate = self.gate(pen, pen.x + 3);
        self.fence(pen, Some(gate));
        let warnings: Vec<Vec<Vec<(Pos, Feature)>>> = (0..4)
            .flat_map(|k| {
                let x = pen.x + 4 + 7 * k;
                [Pos::new(x, pen.y - 1), Pos::new(x, pen.y + pen.h)]
            })
            .map(|q| vec![vec![(q, Feature::Sign('!'))]])
            .collect();
        self.signs(&warnings);
        for k in 0..5 {
            let r = self.roll(10 + k);
            let at = Pos::new(
                pen.x + 4 + (r % (pen.w as u64 - 8)) as i32,
                pen.y + 3 + ((r >> 10) % (pen.h as u64 - 6)) as i32,
            );
            // a rim of rock, open to the east so nobody is shut in a crater
            for q in at.neighbors8().filter(|q| q.x <= at.x) {
                if self.open(q) {
                    self.p.set(q, Terrain::Rock);
                }
            }
            if self.open(at) {
                self.p.set(at, Terrain::Spill);
            }
        }
        self.block(Rect::new(pen.x + pen.w - 4, pen.y + 1, 2, 2), Terrain::Mass);
        self.put(Pos::new(pen.x + pen.w - 5, pen.y + 1), Feature::Lamp);
    }

    /// An offshore rig: a deck on the sea with its works, and a pier from the nearest
    /// land.
    fn rig(&mut self, poi: &Poi) {
        let c = self.centre(poi);
        let deck = Self::centred(c, 14, 6);
        for q in cells(deck) {
            if self.p.in_world(q) && self.p.get(q).is_some() {
                self.p.set(q, Terrain::Walkway);
                self.p.clear_feature(q);
            }
        }
        for (dx, dy) in [(-4, -1), (2, -2), (3, 1)] {
            let q = c.offset(dx, dy);
            if self.p.get(q).is_some() {
                self.p.set(q, Terrain::Mass);
            }
        }
        for q in [
            Pos::new(deck.x, deck.y),
            Pos::new(deck.x + deck.w - 1, deck.y + deck.h - 1),
        ] {
            if self.p.get(q).is_some() {
                self.p.set_feature(q, Feature::Lamp);
            }
        }
        // the pier: straight to the nearest land tile's middle, two cells wide
        let land = (1..=8i32).find_map(|r| {
            (-r..=r)
                .flat_map(|dy| (-r..=r).map(move |dx| (dx, dy)))
                .filter(|&(dx, dy)| dx.abs().max(dy.abs()) == r)
                .map(|(dx, dy)| poi.at.offset(dx, dy))
                .find(|n| self.world.tile(*n).is_some_and(|t| t.water == Water::None))
        });
        let Some(land) = land else {
            return;
        };
        let to = Pos::new(TILE * land.x + TILE / 2, TILE * land.y + TILE / 2);
        let (dx, dy) = (to.x - c.x, to.y - c.y);
        let n = dx.abs().max(dy.abs()).max(1);
        for k in 0..=n {
            let q = Pos::new(c.x + dx * k / n, c.y + dy * k / n);
            for q in [
                q,
                if dx.abs() >= dy.abs() {
                    q.offset(0, 1)
                } else {
                    q.offset(1, 0)
                },
            ] {
                if self.p.in_world(q) && self.p.wet_at(q) != 0 && self.p.get(q).is_some() {
                    self.p.set(q, Terrain::Walkway);
                    self.p.clear_feature(q);
                }
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
    fn a_truck_stops_sign_is_framed() {
        // the roadside sign is the stop's name between two ▣ caps, as round 2's Kilo-9
        for seed in 0..6 {
            let w = small(seed);
            for poi in w.pois.iter().filter(|p| p.kind == PoiKind::TruckStop) {
                let (_, g) = round(&w, poi, 6);
                let signs = g
                    .iter()
                    .filter_map(|(p, t)| match t.feature {
                        Some(Feature::Sign(c)) if c != '!' => Some((p, c)),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let caps = signs.iter().filter(|(_, c)| *c == '▣').count();
                assert_eq!(caps, 2, "seed {seed} {:?}: {signs:?}", poi.at);
                assert!(signs.len() >= 4, "seed {seed} {:?}: {signs:?}", poi.at);
            }
        }
    }

    #[test]
    fn a_sign_never_cuts_a_way_through() {
        // a truck stop's road sign and a test range's warnings: every way round each
        // that was there without it is still there (four ways and eight), within a few
        // cells of it
        use crate::geom::Rect;
        use crate::pathing::cuts;
        let mut signs = 0;
        for seed in 0..12 {
            let w = small(seed);
            for poi in w
                .pois
                .iter()
                .filter(|p| matches!(p.kind, PoiKind::TruckStop | PoiKind::TestRange))
            {
                let (o, g) = round(&w, poi, 6);
                let sign = |p: Pos| {
                    g.get(p)
                        .is_some_and(|t| matches!(t.feature, Some(Feature::Sign(_))))
                };
                // each sign: its cells joined eight ways, and its posts
                let mut seen = Grid::filled(g.size(), false);
                for (p, _) in g.iter() {
                    if !sign(p) || seen[p] {
                        continue;
                    }
                    let mut shape = vec![p];
                    seen[p] = true;
                    let mut k = 0;
                    while k < shape.len() {
                        for q in shape[k].neighbors8() {
                            if sign(q) && !seen[q] {
                                seen[q] = true;
                                shape.push(q);
                            }
                        }
                        k += 1;
                    }
                    let posts: Vec<Pos> = Rect::new(0, 0, g.size().w as i32, g.size().h as i32)
                        .iter()
                        .filter(|q| {
                            g[*q].feature == Some(Feature::Pipe)
                                && shape.iter().any(|s| s.chebyshev(*q) <= 2)
                        })
                        .collect();
                    shape.extend(posts);
                    let (x0, x1) = (
                        shape.iter().map(|p| p.x).min().unwrap(),
                        shape.iter().map(|p| p.x).max().unwrap(),
                    );
                    let (y0, y1) = (
                        shape.iter().map(|p| p.y).min().unwrap(),
                        shape.iter().map(|p| p.y).max().unwrap(),
                    );
                    let window = Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1).grow(3);
                    let without = |q: Pos| {
                        g.get(q).is_some_and(|t| {
                            let mut t = *t;
                            if shape.contains(&q) {
                                t.feature = None;
                            }
                            t.is_walkable()
                        })
                    };
                    assert!(
                        !cuts(window, without, &shape),
                        "seed {seed} {:?}: the sign at {:?} cuts a way through",
                        poi.kind,
                        p.offset(o.x, o.y)
                    );
                    signs += 1;
                }
            }
        }
        assert!(signs >= 20, "{signs} signs");
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
    fn every_poi_lays_its_place() {
        let has = |g: &Grid<Tile>, f: &dyn Fn(&Tile) -> bool| g.count(|t| f(t));
        let feat = |f: Feature| move |t: &Tile| t.feature == Some(f);
        let terr = |x: Terrain| move |t: &Tile| t.terrain == x;
        for kind in [
            PoiKind::Relay,
            PoiKind::Camp,
            PoiKind::Wreck,
            PoiKind::Arcology,
            PoiKind::Solar,
            PoiKind::WaterPlant,
            PoiKind::TestRange,
            PoiKind::Rig,
        ] {
            let (w, poi) = find(kind);
            let (_, g) = round(&w, &poi, 3);
            let ok = match kind {
                PoiKind::Relay => {
                    has(&g, &terr(Terrain::Railing)) >= 10 && has(&g, &feat(Feature::Terminal)) >= 1
                }
                PoiKind::Camp => {
                    has(&g, &feat(Feature::Stall)) >= 3
                        && has(&g, &feat(Feature::Furniture(Furniture::Couch))) >= 6
                }
                PoiKind::Wreck => {
                    has(&g, &terr(Terrain::Mass)) >= 6 && has(&g, &terr(Terrain::Rock)) >= 6
                }
                PoiKind::Arcology => has(&g, &terr(Terrain::Mass)) >= 100,
                PoiKind::Solar => has(&g, &feat(Feature::AcUnit)) >= 20,
                PoiKind::WaterPlant => {
                    has(&g, &feat(Feature::Pipe)) >= 6 && has(&g, &terr(Terrain::Mass)) >= 6
                }
                PoiKind::TestRange => {
                    has(&g, &terr(Terrain::Railing)) >= 20
                        && has(&g, &feat(Feature::Sign('!'))) >= 2
                }
                PoiKind::Rig => has(&g, &terr(Terrain::Walkway)) >= 30,
                _ => true,
            };
            assert!(ok, "{kind:?} at {:?} lays too little", poi.at);
        }
    }

    #[test]
    fn no_building_is_cut_by_water() {
        for seed in 0..6 {
            let w = small(seed);
            for poi in &w.pois {
                let (_, g) = round(&w, poi, 3);
                for (p, t) in g.iter() {
                    if matches!(t.terrain, Terrain::Wall | Terrain::Window | Terrain::Door) {
                        let wet = p.neighbors8().any(|q| {
                            g.get(q).is_some_and(|t| {
                                matches!(t.terrain, Terrain::Water | Terrain::Shallows)
                            })
                        });
                        assert!(!wet, "seed {seed} {poi:?}: water by a wall at {p:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_rig_is_reached_by_its_pier() {
        let (w, poi) = find(PoiKind::Rig);
        let (_, g) = round(&w, &poi, 6);
        // from any dry ground, over walkable cells, onto the platform's deck
        let dry = |t: &Tile| {
            t.is_walkable()
                && !matches!(
                    t.terrain,
                    Terrain::Walkway | Terrain::Ford | Terrain::Bridge
                )
        };
        let mut seen = Grid::filled(g.size(), false);
        let mut stack: Vec<Pos> = g.iter().filter(|(_, t)| dry(t)).map(|(p, _)| p).collect();
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
        let deck: Vec<Pos> = g
            .iter()
            .filter(|(_, t)| t.terrain == Terrain::Walkway)
            .map(|(p, _)| p)
            .collect();
        assert!(deck.len() >= 30, "a deck and a pier");
        assert!(
            deck.iter().all(|p| seen[*p]),
            "the whole deck is reached from the shore"
        );
    }

    #[test]
    fn a_relay_mast_has_its_track_to_the_road() {
        let (mut checked, mut tracked) = (0, 0);
        for seed in 0..30 {
            let w = small(seed);
            for poi in w.pois.iter().filter(|p| p.kind == PoiKind::Relay) {
                let near_road = (-3..=3)
                    .flat_map(|dy| (-3..=3).map(move |dx| (dx, dy)))
                    .any(|(dx, dy)| w.tile(poi.at.offset(dx, dy)).is_some_and(|t| t.road != 0));
                if !near_road {
                    continue;
                }
                let (_, g) = round(&w, poi, 4);
                // from the cabinet over gravel to a road's lane
                let mut seen = Grid::filled(g.size(), false);
                let mut stack: Vec<Pos> = g
                    .iter()
                    .filter(|(_, t)| t.feature == Some(Feature::Terminal))
                    .map(|(p, _)| p)
                    .collect();
                let mut road = false;
                while let Some(p) = stack.pop() {
                    for q in p.neighbors8() {
                        let Some(t) = g.get(q) else { continue };
                        road |= t.terrain == Terrain::Road;
                        if !seen[q] && t.terrain == Terrain::Gravel {
                            seen[q] = true;
                            stack.push(q);
                        }
                    }
                }
                // a river or lake between the mast and its road leaves it without one
                tracked += road as usize;
                checked += 1;
            }
        }
        assert!(checked >= 3, "{checked} relays by a road");
        assert!(
            tracked * 10 >= checked * 6,
            "{tracked} of {checked} relays by a road have a track"
        );
    }

    /// Cells of `g` joined to its border over walkable cells.
    fn open_from_border(g: &Grid<Tile>) -> Grid<bool> {
        let size = g.size();
        let mut seen = Grid::filled(size, false);
        let mut stack: Vec<Pos> = g
            .iter()
            .map(|(p, _)| p)
            .filter(|p| {
                p.x == 0 || p.y == 0 || p.x == size.w as i32 - 1 || p.y == size.h as i32 - 1
            })
            .filter(|p| g[*p].is_walkable())
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
        seen
    }

    #[test]
    fn every_pump_house_and_booth_can_be_entered() {
        let mut doors = 0;
        for seed in 0..20 {
            let w = small(seed);
            for poi in w.pois.iter().filter(|p| {
                matches!(
                    p.kind,
                    PoiKind::WaterPlant | PoiKind::Checkpoint | PoiKind::Solar
                )
            }) {
                let (o, g) = round(&w, poi, 5);
                let open = open_from_border(&g);
                for (p, t) in g.iter() {
                    if t.terrain != Terrain::Door {
                        continue;
                    }
                    let inside = p
                        .neighbors4()
                        .any(|q| g.get(q).is_some_and(|t| t.terrain == Terrain::Floor));
                    let reached = open[p];
                    assert!(
                        inside && reached,
                        "seed {seed} {poi:?}: the door at {:?} (inside {inside}, reached {reached})",
                        p.offset(o.x, o.y)
                    );
                    doors += 1;
                }
            }
        }
        assert!(doors >= 20, "{doors} doors");
    }

    #[test]
    fn landing_on_a_poi_reaches_the_open() {
        use crate::overworld::detail::landing;
        let mut landed = 0;
        for seed in 0..20 {
            let w = small(seed);
            for poi in w.pois.iter().filter(|p| p.kind != PoiKind::Rig) {
                let Some(c) = landing(&w, poi.at) else {
                    continue;
                };
                let (o, g) = round(&w, poi, 4);
                let open = open_from_border(&g);
                assert!(
                    open[Pos::new(c.x - o.x, c.y - o.y)],
                    "seed {seed} {poi:?}: landed shut in at {c:?}"
                );
                landed += 1;
            }
        }
        assert!(landed > 100, "{landed}");
    }

    #[test]
    fn a_north_south_store_is_open_inside() {
        use crate::overworld::{LINK_E, LINK_N, LINK_S, LINK_W};
        let mut checked = 0;
        for seed in [12, 14, 16, 20, 23] {
            let w = small(seed);
            for poi in w.pois.iter().filter(|p| p.kind == PoiKind::TruckStop) {
                let ns = [
                    poi.at,
                    poi.at.offset(0, 1),
                    poi.at.offset(0, -1),
                    poi.at.offset(1, 0),
                    poi.at.offset(-1, 0),
                ]
                .iter()
                .filter_map(|t| w.tile(*t))
                .any(|t| t.road & (LINK_N | LINK_S) != 0 && t.road & (LINK_E | LINK_W) == 0);
                if !ns {
                    continue;
                }
                let (_, g) = round(&w, poi, 6);
                let open = open_from_border(&g);
                let floor: Vec<Pos> = g
                    .iter()
                    .filter(|(_, t)| t.terrain == Terrain::Floor)
                    .map(|(p, _)| p)
                    .collect();
                let shut = floor
                    .iter()
                    .filter(|p| {
                        !p.neighbors8()
                            .any(|q| open.get(q).copied().unwrap_or(false))
                    })
                    .count();
                assert!(
                    shut * 10 <= floor.len(),
                    "seed {seed} {poi:?}: {shut} of {} floor cells shut away",
                    floor.len()
                );
                checked += 1;
            }
        }
        assert!(checked >= 2, "{checked} north-south stops");
    }

    #[test]
    fn a_slid_stop_is_the_same_built_alone() {
        let w = small(60);
        let poi = w
            .pois
            .iter()
            .find(|p| p.kind == PoiKind::TruckStop && p.at == Pos::new(59, 24));
        let Some(poi) = poi else { return };
        let (o, r) = round(&w, poi, 9);
        for dy in -3..=3 {
            for dx in -8..=8 {
                let t = poi.at.offset(dx, dy);
                let g = ground(&w, t.x, t.y);
                for (c, a) in g.iter() {
                    let b = r[Pos::new(TILE * t.x + c.x - o.x, TILE * t.y + c.y - o.y)];
                    assert_eq!(
                        (a.terrain, a.feature),
                        (b.terrain, b.feature),
                        "{t:?} {c:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_poi_tile_alone_equals_its_region() {
        for kind in PoiKind::ALL {
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
