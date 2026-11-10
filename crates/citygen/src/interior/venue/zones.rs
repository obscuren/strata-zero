//! The zones stage: each level cut into bands front to back, a spine through them, and
//! the doors where the spine crosses from one band to the next, locked as the type's
//! borders say.

use super::{
    BandArea, LOCK_NUMBERS, LevelLayout, PlannedDoor, PlannedRoom, Spine, VenueFrame, VenueLayout,
    VenueType, Ways,
};
use crate::geom::{Dir, Pos, Rect, Size};
use crate::interior::{DoorLock, RoomKind};
use crate::rng::hash64;
use crate::world::StratumKind;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::BTreeSet;

/// The zones stage's rng, from the layout seed.
pub(super) const STAGE: u64 = 0x2_0E5;

/// The rows a band takes at least, on a level deep enough for every band to.
const BAND_ROWS: i32 = 5;
/// The rows no band goes under: the wall at its front and one row behind it.
const MIN_ROWS: i32 = 2;
/// How far the rng moves a cut, in points of the level's depth.
const JITTER: i32 = 10;
/// The floor a ring needs at least: tiles across, and rows deep.
const RING_FLOOR: (i32, i32) = (24, 16);
/// The rows a ring's band needs behind its wall: the corridor's two rows on either side of
/// a core one row deep.
const RING_ROWS: i32 = 5;

/// A level's floor inside its frame wall, seen from its front: a tile lies `a` rows deep
/// from the front wall and `c` tiles across.
#[derive(Clone, Copy, Debug)]
pub(super) struct Axes {
    pub front: Dir,
    pub inner: Rect,
}

impl Axes {
    pub fn new(size: Size, front: Dir) -> Axes {
        Axes {
            front,
            inner: size.rect().inset(1),
        }
    }

    /// Rows from the front wall to the back.
    pub fn depth(self) -> i32 {
        match self.front {
            Dir::N | Dir::S => self.inner.h,
            _ => self.inner.w,
        }
    }

    /// Tiles from side to side.
    pub fn width(self) -> i32 {
        match self.front {
            Dir::N | Dir::S => self.inner.w,
            _ => self.inner.h,
        }
    }

    /// The tile `a` rows deep and `c` across.
    pub fn pos(self, a: i32, c: i32) -> Pos {
        let r = self.inner;
        match self.front {
            Dir::S => Pos::new(r.x + c, r.bottom() - 1 - a),
            Dir::N => Pos::new(r.x + c, r.y + a),
            Dir::W => Pos::new(r.x + a, r.y + c),
            _ => Pos::new(r.right() - 1 - a, r.y + c),
        }
    }

    /// How many rows deep `p` lies.
    pub fn deep(self, p: Pos) -> i32 {
        let r = self.inner;
        match self.front {
            Dir::S => r.bottom() - 1 - p.y,
            Dir::N => p.y - r.y,
            Dir::W => p.x - r.x,
            _ => r.right() - 1 - p.x,
        }
    }

    /// The rect over rows `a.0..a.1` and tiles `c.0..c.1` across; neither range empty.
    pub fn rect(self, a: (i32, i32), c: (i32, i32)) -> Rect {
        debug_assert!(a.0 < a.1 && c.0 < c.1, "an empty rect: {a:?} by {c:?}");
        Rect::around(&[self.pos(a.0, c.0), self.pos(a.1 - 1, c.1 - 1)])
    }
}

/// The side every level's bands start from: the side of the frame's first street door,
/// so zones stack the same way up the building. A frame without one takes its first
/// door's side, and one with no door the south.
pub(super) fn front(frame: &VenueFrame) -> Dir {
    let (w, h) = (frame.size.w as i32, frame.size.h as i32);
    let street = frame
        .doors
        .iter()
        .filter(|d| d.stratum == StratumKind::Street);
    let at = street.chain(frame.doors.iter()).find_map(|d| d.at);
    match at {
        Some(p) if p.x == 0 => Dir::W,
        Some(p) if p.x == w - 1 => Dir::E,
        Some(p) if p.y == 0 => Dir::N,
        Some(p) if p.y == h - 1 => Dir::S,
        _ => Dir::S,
    }
}

/// Cuts every level into its role's bands, lays its spine through them, and puts doors
/// where the spine crosses each cut, locked as the type's border between the two zones
/// says. Each level draws from its own rng, so the levels of one role differ.
pub(super) fn lay(
    t: &VenueType,
    layout: &mut VenueLayout,
    rng: &mut ChaCha8Rng,
) -> Result<(), String> {
    let front = front(&layout.frame);
    let seed: u64 = rng.random();
    for x in &mut layout.levels {
        let mut rng = ChaCha8Rng::seed_from_u64(hash64(seed, x.level as i64 as u64));
        level(t, &layout.frame, front, x, &mut rng)
            .map_err(|e| format!("{}: level {}: {e}", t.id, x.level))?;
    }
    Ok(())
}

/// One level's bands, spine rooms, core and border doors.
fn level(
    t: &VenueType,
    frame: &VenueFrame,
    front: Dir,
    x: &mut LevelLayout,
    rng: &mut ChaCha8Rng,
) -> Result<(), String> {
    let def = t
        .role(&x.role)
        .ok_or_else(|| format!("no level role `{}`", x.role))?;
    let zones = def
        .bands
        .iter()
        .map(|b| {
            t.zone(&b.zone)
                .ok_or_else(|| format!("no zone `{}`", b.zone))
        })
        .collect::<Result<Vec<u8>, String>>()?;
    let n = zones.len();
    if n == 0 {
        return Err("no bands".into());
    }
    let axes = Axes::new(x.size, front);
    let (depth, width) = (axes.depth(), axes.width());
    x.reserved = doorsteps(frame, x.level, axes.inner);
    let blocked: BTreeSet<i32> = x.reserved.iter().map(|p| axes.deep(*p)).collect();
    let shares: Vec<i32> = def.bands.iter().map(|b| i32::from(b.share)).collect();
    let starts = cuts(&shares, depth, &blocked, rng)?;

    // Each band's rows, and the rows behind the wall at its front.
    let rows: Vec<(i32, i32)> = (0..n)
        .map(|i| (starts[i], starts.get(i + 1).copied().unwrap_or(depth)))
        .collect();
    let clear = |i: usize| (rows[i].0 + i32::from(i > 0), rows[i].1);
    x.bands = (0..n)
        .map(|i| BandArea {
            zone: zones[i],
            rect: axes.rect(rows[i], (0, width)),
        })
        .collect();

    // The ring's band holds the level's middle row.
    let ring_band = rows.iter().rposition(|r| r.0 <= depth / 2).unwrap_or(0);
    let ring_fits = width >= RING_FLOOR.0 && depth >= RING_FLOOR.1 && {
        let (u, e) = clear(ring_band);
        e - u >= RING_ROWS
    };
    let spine = shape(def.spine, n, ring_fits, rng);
    x.spine = spine;

    // The 2-wide corridor's tiles across: the middle of the level.
    let line = (width / 2 - 1, width / 2 + 1);
    let turn_left = spine == Spine::Bent && rng.random_range(0..2) == 0;
    // The ring stands this far in from each side, leaving its band's rooms a quarter to a
    // third of the width there.
    let margin = if spine == Spine::Ring {
        rng.random_range(width / 4..=width / 3)
    } else {
        0
    };
    x.core = None;
    x.rooms = (0..n)
        .map(|i| {
            let (u, e) = clear(i);
            let rect = match spine {
                Spine::Hall => axes.rect((u, e), (0, width)),
                // The back band's piece turns from the spine to one side, behind its cut.
                Spine::Bent if i == n - 1 => {
                    let side = if turn_left {
                        (0, line.1)
                    } else {
                        (line.0, width)
                    };
                    axes.rect((u, (u + 2).min(e)), side)
                }
                Spine::Ring if i == ring_band => {
                    let (c0, c1) = (margin, width - margin);
                    x.core = Some(axes.rect((u + 2, e - 2), (c0 + 2, c1 - 2)));
                    axes.rect((u, e), (c0, c1))
                }
                _ => axes.rect((u, e), line),
            };
            PlannedRoom {
                rect,
                kind: RoomKind::Corridor,
                name: RoomKind::Corridor.label().to_string(),
                zone: zones[i],
                number: 0,
                private: false,
                furniture: Vec::new(),
                stations: Vec::new(),
                spots: Vec::new(),
            }
        })
        .collect();

    x.doors.clear();
    let mut number = LOCK_NUMBERS;
    for i in 1..n {
        let cut = rows[i].0;
        let (zf, zb) = (zones[i - 1], zones[i]);
        let border = if zf == zb {
            None
        } else {
            let name = |z: u8| &t.zones[usize::from(z)];
            let b = t
                .border(zf, zb)
                .ok_or_else(|| format!("no border between `{}` and `{}`", name(zf), name(zb)))?;
            Some(b)
        };
        // The deeper zone's side of the cut, in rows.
        let inside = if zb > zf { 1 } else { -1 };
        let count = usize::from(def.bands[i].doors.max(1));
        let left = count > 1 && rng.random_range(0..2) == 0;
        for c in door_columns(width, line, count, left) {
            let at = axes.pos(cut, c);
            let fit = border.map(|b| DoorLock {
                at,
                number,
                zone: zf.max(zb),
                kind: b.lock,
                ways: Ways::of(&b.ways),
                free_from: b.one_way.then(|| axes.pos(cut + inside, c)),
            });
            number += u16::from(fit.is_some());
            x.doors.push(PlannedDoor {
                at,
                rooms: (i - 1, i),
                lock: fit.filter(|_| border.is_some_and(|b| b.always)),
                fit,
                border: border.is_some(),
            });
        }
    }
    Ok(())
}

/// The tile inside the frame wall from a door on it.
fn inward(inner: Rect, at: Pos) -> Pos {
    if at.x < inner.x {
        at.offset(1, 0)
    } else if at.x >= inner.right() {
        at.offset(-1, 0)
    } else if at.y < inner.y {
        at.offset(0, 1)
    } else {
        at.offset(0, -1)
    }
}

/// The doorstep of each frame door on `level`, and the tiles beside it inside the frame
/// wall: tiles no cut or wall crosses.
fn doorsteps(frame: &VenueFrame, level: i32, inner: Rect) -> Vec<Pos> {
    let steps = frame
        .doors
        .iter()
        .filter(|d| d.level == level)
        .filter_map(|d| d.at)
        .map(|at| inward(inner, at));
    let near: BTreeSet<Pos> = steps
        .flat_map(|s| std::iter::once(s).chain(s.neighbors4()))
        .filter(|p| inner.contains(*p))
        .collect();
    near.into_iter().collect()
}

/// The row each band starts at, from the front: the first at 0, each next at its running
/// share of `depth` moved up to `JITTER` points by the rng, on a row no doorstep holds
/// (`blocked`). Full thickness is `BAND_ROWS` for every band, or on a level too shallow
/// for that an even share of it. A cut takes the clear row nearest where the rng put it,
/// trying in turn:
/// 1. its jitter window, every band at full thickness;
/// 2. anywhere every band keeps full thickness, when the window is all blocked;
/// 3. only when every row at full thickness is blocked, the clear row nearest them,
///    thinning a band to as little as `MIN_ROWS`.
fn cuts(
    shares: &[i32],
    depth: i32,
    blocked: &BTreeSet<i32>,
    rng: &mut ChaCha8Rng,
) -> Result<Vec<i32>, String> {
    let n = shares.len() as i32;
    let thinnest = BAND_ROWS.min(depth / n);
    if thinnest < MIN_ROWS {
        return Err(format!("{depth} rows is too shallow for {n} bands"));
    }
    let row = |points: i32| (points.clamp(0, 100) * depth + 50) / 100;
    let mut starts = vec![0];
    let mut running = 0;
    for (i, share) in shares[..shares.len() - 1].iter().enumerate() {
        running += share;
        let last = starts[i];
        let behind = n - 1 - i as i32;
        let room = |rows: i32| (last + rows, depth - behind * rows);
        let full = room(thinnest);
        let near = (
            row(running - JITTER).max(full.0),
            row(running + JITTER).min(full.1),
        );
        let want = row(running + rng.random_range(-JITTER..=JITTER));
        // A thinned band gives up no more rows than the doorsteps take.
        let nearest_full = want.max(full.0).min(full.1);
        let cut = clear_row(want, near, blocked)
            .or_else(|| clear_row(want, full, blocked))
            .or_else(|| clear_row(nearest_full, room(MIN_ROWS), blocked))
            .ok_or_else(|| format!("no row clear of the doorsteps for cut {}", i + 1))?;
        starts.push(cut);
    }
    Ok(starts)
}

/// The row in `range` nearest `want` that isn't `blocked`, the nearer front one first.
fn clear_row(want: i32, range: (i32, i32), blocked: &BTreeSet<i32>) -> Option<i32> {
    let (lo, hi) = range;
    if lo > hi {
        return None;
    }
    let want = want.clamp(lo, hi);
    (0..=hi - lo)
        .flat_map(|d| [want - d, want + d])
        .find(|r| (lo..=hi).contains(r) && !blocked.contains(r))
}

/// The spine a level takes: its role's, or for `Any` one the rng picks. A `Bent` needs two
/// bands, and a `Ring` a floor `ring` says holds it; without them the spine runs straight.
fn shape(want: Spine, bands: usize, ring: bool, rng: &mut ChaCha8Rng) -> Spine {
    let fits = |s: Spine| match s {
        Spine::Bent => bands > 1,
        Spine::Ring => ring,
        _ => true,
    };
    if want == Spine::Any {
        let can: Vec<Spine> = [Spine::Straight, Spine::Bent, Spine::Ring, Spine::Hall]
            .into_iter()
            .filter(|s| fits(*s))
            .collect();
        can[rng.random_range(0..can.len())]
    } else if fits(want) {
        want
    } else {
        Spine::Straight
    }
}

/// Where a cut's `count` doors stand across a level `width` wide: the first on the
/// spine's `line`, the rest spread over the two sides clear of the spine and its walls,
/// the side `left` names taking the odd one.
fn door_columns(width: i32, line: (i32, i32), count: usize, left: bool) -> Vec<i32> {
    let mut cols = vec![line.0];
    let extra = count - 1;
    let sides = [(0, line.0 - 1), (line.1 + 1, width)];
    let (first, second) = if left {
        (sides[0], sides[1])
    } else {
        (sides[1], sides[0])
    };
    for ((lo, hi), m) in [(first, extra.div_ceil(2)), (second, extra / 2)] {
        let m = m as i32;
        for k in 0..m {
            let c = lo + (hi - lo) * (k + 1) / (m + 1);
            if lo < hi && !cols.contains(&c) {
                cols.push(c);
            }
        }
    }
    cols
}

#[cfg(test)]
mod tests {
    use super::cuts;
    use crate::geom::{Dir, Pos, Rect};
    use crate::interior::RoomKind;
    use crate::interior::venue::testutil::{cities, test_type};
    use crate::interior::venue::{
        BorderDef, LOCK_NUMBERS, LevelLayout, OwnDoor, Spine, VenueFrame, VenueLayout, VenuePack,
        VenueType, Ways, lay_out,
    };
    use crate::world::StratumKind;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;
    use std::collections::BTreeSet;

    fn layouts(t: &VenueType) -> Vec<(u64, VenueLayout)> {
        (0..20)
            .map(|s| (s, lay_out(t, VenueFrame::own(t), s).unwrap()))
            .collect()
    }

    /// The side of the frame's first street door, read off its floor tile.
    fn front(l: &VenueLayout) -> Dir {
        let d = l
            .frame
            .doors
            .iter()
            .find(|d| d.stratum == StratumKind::Street);
        let at = d.and_then(|d| d.at).unwrap();
        let (w, h) = (l.frame.size.w as i32, l.frame.size.h as i32);
        if at.x == 0 {
            Dir::W
        } else if at.x == w - 1 {
            Dir::E
        } else if at.y == 0 {
            Dir::N
        } else {
            assert_eq!(at.y, h - 1);
            Dir::S
        }
    }

    /// How many rows `p` lies in from the `front` side of `inner`.
    fn deep(front: Dir, inner: Rect, p: Pos) -> i32 {
        match front {
            Dir::S => inner.bottom() - 1 - p.y,
            Dir::N => p.y - inner.y,
            Dir::W => p.x - inner.x,
            _ => inner.right() - 1 - p.x,
        }
    }

    /// The rows a rect starts at, from the front, and how many it spans.
    fn rows(front: Dir, inner: Rect, r: Rect) -> (i32, i32) {
        let far = Pos::new(r.right() - 1, r.bottom() - 1);
        let (a, b) = (
            deep(front, inner, r.iter().next().unwrap()),
            deep(front, inner, far),
        );
        (a.min(b), (a - b).abs() + 1)
    }

    fn depth(front: Dir, inner: Rect) -> i32 {
        match front {
            Dir::N | Dir::S => inner.h,
            _ => inner.w,
        }
    }

    /// A door's neighbours across its cut: in the band in front, then in the band behind
    /// (the one whose first row holds the door).
    fn across(x: &LevelLayout, at: Pos) -> ((usize, Pos), (usize, Pos)) {
        let back = x.bands.iter().position(|b| b.rect.contains(at)).unwrap();
        let f = at
            .neighbors4()
            .find(|p| back > 0 && x.bands[back - 1].rect.contains(*p))
            .unwrap_or_else(|| panic!("the door at {at:?} is on no cut"));
        (
            (back - 1, f),
            (back, Pos::new(2 * at.x - f.x, 2 * at.y - f.y)),
        )
    }

    /// The border between zones ranked `a` and `b`, found by name in either order.
    fn border(t: &VenueType, a: u8, b: u8) -> &BorderDef {
        let (a, b) = (&t.zones[a as usize], &t.zones[b as usize]);
        t.borders
            .iter()
            .find(|d| (&d.zones.0, &d.zones.1) == (a, b) || (&d.zones.0, &d.zones.1) == (b, a))
            .unwrap()
    }

    /// A level's bands tile its floor, none under 2 rows, and no reserved tile lies on a
    /// cut. A band is under full thickness only when its cut found every row at full
    /// thickness blocked by a doorstep: the cut behind it, or the back band's in front.
    fn cuts_hold(front: Dir, x: &LevelLayout, what: &str) {
        let inner = x.size.rect().inset(1);
        let depth = depth(front, inner);
        let n = x.bands.len() as i32;
        let full = 5.min(depth / n);
        let blocked: BTreeSet<i32> = x.reserved.iter().map(|p| deep(front, inner, *p)).collect();
        let spans: Vec<(i32, i32)> = x.bands.iter().map(|b| rows(front, inner, b.rect)).collect();
        let mut next = 0;
        for (b, (start, thick)) in x.bands.iter().zip(&spans) {
            assert_eq!(b.rect.intersection(inner), Some(b.rect), "{what}");
            assert_eq!(*start, next, "{what}: the bands run on");
            assert!(*thick >= 2, "{what}: a band {thick} rows deep");
            next = start + thick;
        }
        assert_eq!(next, depth, "{what}");
        let area: usize = x.bands.iter().map(|b| b.rect.area()).sum();
        assert_eq!(area, inner.area(), "{what}: the bands tile the floor");
        for (j, (_, thick)) in spans.iter().enumerate() {
            if *thick >= full {
                continue;
            }
            let i = if j + 1 < spans.len() { j + 1 } else { j };
            let behind = n - i as i32;
            let (lo, hi) = (spans[i - 1].0 + full, depth - behind * full);
            assert!(
                (lo..=hi).all(|r| blocked.contains(&r)),
                "{what}: band {j} is {thick} rows, yet a row of {lo}..={hi} was clear"
            );
        }
        for p in &x.reserved {
            let i = x.bands.iter().position(|b| b.rect.contains(*p)).unwrap();
            assert!(
                i == 0 || deep(front, inner, *p) != spans[i].0,
                "{what}: {p:?} on a cut"
            );
        }
    }

    #[test]
    fn every_type_lays_its_zones_in_its_own_frame_and_its_buildings() {
        let cities = cities();
        for t in VenuePack::builtin()
            .types
            .iter()
            .cloned()
            .chain([test_type()])
        {
            let buildings = cities.iter().flat_map(|c| {
                c.buildings
                    .iter()
                    .filter(|b| t.builds.contains(&b.kind))
                    .filter_map(|b| VenueFrame::of_building(c, b.id))
            });
            for f in std::iter::once(VenueFrame::own(&t)).chain(buildings) {
                for seed in 0..5 {
                    let what = format!("{} in {:?} {:?}, seed {seed}", t.id, f.kind, f.footprint);
                    let l = lay_out(&t, f.clone(), seed).unwrap_or_else(|e| panic!("{what}: {e}"));
                    for x in &l.levels {
                        cuts_hold(front(&l), x, &format!("{what}, level {}", x.level));
                    }
                }
            }
        }
    }

    #[test]
    fn bands_run_front_to_back_in_data_order() {
        let t = test_type();
        for (seed, l) in layouts(&t) {
            let front = front(&l);
            for x in &l.levels {
                let what = format!("seed {seed} level {}", x.level);
                let def = t.role(&x.role).unwrap();
                let inner = x.size.rect().inset(1);
                let depth = depth(front, inner);
                assert_eq!(x.bands.len(), def.bands.len(), "{what}");
                let thinnest = 5.min(depth / def.bands.len() as i32);
                let (mut running, mut next, mut area) = (0, 0, 0);
                for (i, (b, d)) in x.bands.iter().zip(&def.bands).enumerate() {
                    assert_eq!(Some(b.zone), t.zone(&d.zone), "{what} band {i}");
                    assert_eq!(b.rect.intersection(inner), Some(b.rect), "{what} band {i}");
                    let (start, thick) = rows(front, inner, b.rect);
                    assert_eq!(start, next, "{what}: band {i} starts where the last ends");
                    assert!(thick >= thinnest, "{what}: band {i} is {thick} thick");
                    if i > 0 {
                        let lo = (running - 10) * depth / 100;
                        let hi = ((running + 10) * depth + 99) / 100;
                        assert!(
                            (lo..=hi).contains(&start),
                            "{what}: cut {i} at {start}, share {running} of {depth}"
                        );
                    }
                    running += i32::from(d.share);
                    next = start + thick;
                    area += b.rect.area();
                }
                assert_eq!(next, depth, "{what}");
                assert_eq!(area, inner.area(), "{what}: the bands tile the floor");
                for (i, a) in x.bands.iter().enumerate() {
                    for b in &x.bands[i + 1..] {
                        assert!(!a.rect.intersects(b.rect), "{what}");
                    }
                }
            }
        }
    }

    #[test]
    fn border_doors_join_neighbouring_bands_on_the_spine() {
        let t = test_type();
        let mut deeper_in_front = false;
        for (seed, l) in layouts(&t) {
            for x in &l.levels {
                let what = format!("seed {seed} level {}", x.level);
                let def = t.role(&x.role).unwrap();
                assert_ne!(x.spine, Spine::Any, "{what}");
                for d in x.doors.iter().filter(|d| !d.border) {
                    assert_eq!((d.lock, d.fit), (None, None), "{what}: a plain door");
                }
                for i in 1..x.bands.len() {
                    let (zf, zb) = (x.bands[i - 1].zone, x.bands[i].zone);
                    if zf == zb {
                        continue;
                    }
                    deeper_in_front |= zf > zb;
                    let b = border(&t, zf, zb);
                    let on_cut: Vec<_> = x
                        .doors
                        .iter()
                        .filter(|d| d.border && across(x, d.at).1.0 == i)
                        .collect();
                    let want = usize::from(def.bands[i].doors.max(1));
                    assert!(on_cut.len() >= want, "{what}: cut {i} has {}", on_cut.len());
                    let first = on_cut[0];
                    let ((_, f), (_, back)) = across(x, first.at);
                    let (rf, rb) = (&x.rooms[first.rooms.0], &x.rooms[first.rooms.1]);
                    assert!(rf.rect.contains(f) && rb.rect.contains(back), "{what}");
                    assert_eq!((rf.zone, rb.zone), (zf, zb), "{what}");
                    for r in [rf, rb] {
                        assert!(r.kind == RoomKind::Corridor || x.spine == Spine::Hall);
                    }
                    for d in on_cut {
                        let fit = d.fit.unwrap();
                        assert_eq!(
                            (fit.at, fit.kind, fit.ways, fit.zone),
                            (d.at, b.lock, Ways::of(&b.ways), zf.max(zb)),
                            "{what}"
                        );
                        assert_eq!(d.lock, b.always.then_some(fit), "{what}");
                    }
                }
            }
        }
        assert!(deeper_in_front, "a level puts a deeper zone in front");
    }

    #[test]
    fn a_one_way_border_opens_free_from_its_inner_side() {
        let t = test_type();
        let mut one_way = 0;
        for (seed, l) in layouts(&t) {
            for x in &l.levels {
                for d in x.doors.iter().filter(|d| d.border) {
                    let ((f, fp), (b, bp)) = across(x, d.at);
                    let (zf, zb) = (x.bands[f].zone, x.bands[b].zone);
                    let fit = d.fit.unwrap();
                    if border(&t, zf, zb).one_way {
                        let inside = if zb > zf { bp } else { fp };
                        assert_eq!(fit.free_from, Some(inside), "seed {seed} level {}", x.level);
                        one_way += 1;
                    } else {
                        assert_eq!(fit.free_from, None);
                    }
                    if let Some(lock) = d.lock {
                        assert_eq!(lock, fit);
                    }
                }
            }
        }
        assert!(one_way > 0);
    }

    #[test]
    fn lock_numbers_are_their_own() {
        let t = test_type();
        let mut locks = 0;
        for (seed, l) in layouts(&t) {
            for x in &l.levels {
                let mut numbers: Vec<u16> = x
                    .doors
                    .iter()
                    .filter_map(|d| d.fit.or(d.lock))
                    .map(|l| l.number)
                    .collect();
                locks += numbers.len();
                assert!(numbers.iter().all(|n| *n >= LOCK_NUMBERS));
                let all = numbers.len();
                numbers.sort_unstable();
                numbers.dedup();
                assert_eq!(numbers.len(), all, "seed {seed} level {}", x.level);
            }
        }
        assert!(locks > 0);
    }

    #[test]
    fn middle_levels_vary_their_spine() {
        let t = test_type();
        let middle = &t.levels.middle.as_ref().unwrap().name;
        let mut shapes = Vec::new();
        for seed in 0..10 {
            let l = lay_out(&t, VenueFrame::own(&t).raised(9), seed).unwrap();
            let levels: Vec<_> = l.levels.iter().filter(|x| &x.role == middle).collect();
            assert_eq!(levels.len(), 8);
            for x in &levels {
                if !shapes.contains(&x.spine) {
                    shapes.push(x.spine);
                }
            }
            let first = (levels[0].spine, &levels[0].bands);
            assert!(
                levels.iter().any(|x| (x.spine, &x.bands) != first),
                "seed {seed}: eight middle levels alike"
            );
        }
        assert!(shapes.len() >= 2, "{shapes:?}");
    }

    #[test]
    fn a_ring_runs_round_a_core_nothing_uses() {
        let t = test_type();
        let mut rings = 0;
        for (seed, l) in layouts(&t) {
            let front = front(&l);
            for x in &l.levels {
                let what = format!("seed {seed} level {}", x.level);
                if t.role(&x.role).unwrap().spine == Spine::Ring {
                    assert_eq!(x.spine, Spine::Ring, "{what}: the own frame holds its ring");
                }
                assert_eq!(x.core.is_some(), x.spine == Spine::Ring, "{what}");
                let Some(core) = x.core else { continue };
                rings += 1;
                assert!(!core.is_empty(), "{what}");
                // The ring is its band's spine room, two tiles round the core.
                let ring = x
                    .rooms
                    .iter()
                    .position(|r| r.kind == RoomKind::Corridor && r.rect.inset(2) == core)
                    .unwrap_or_else(|| panic!("{what}: no ring round {core:?}"));
                let inner = x.size.rect().inset(1);
                let band = x
                    .bands
                    .iter()
                    .find(|b| b.rect.intersection(x.rooms[ring].rect) == Some(x.rooms[ring].rect))
                    .unwrap();
                let (start, thick) = rows(front, inner, band.rect);
                let middle = depth(front, inner) / 2;
                assert!((start..start + thick).contains(&middle), "{what}");
                // Nothing else uses it.
                for (i, r) in x.rooms.iter().enumerate() {
                    assert!(i == ring || !r.rect.intersects(core), "{what}: room {i}");
                    assert!(r.spots.iter().all(|p| !core.contains(*p)), "{what}");
                }
                for d in &x.doors {
                    assert!(!core.grow(1).contains(d.at), "{what}: a door at {:?}", d.at);
                }
                assert!(x.reserved.iter().all(|p| !core.contains(*p)), "{what}");
            }
        }
        assert!(rings > 0);
    }

    #[test]
    fn a_ring_too_small_for_its_level_runs_straight() {
        let mut t = test_type();
        // 24 x 15: the floor is 22 across, under the ring's 24.
        t.frame.footprint = (8, 5);
        for seed in 0..5 {
            let l = lay_out(&t, VenueFrame::own(&t), seed).unwrap();
            for x in &l.levels {
                assert!(
                    x.spine != Spine::Ring && x.core.is_none(),
                    "level {}",
                    x.level
                );
                if t.role(&x.role).unwrap().spine == Spine::Ring {
                    assert_eq!(x.spine, Spine::Straight);
                }
            }
        }
    }

    #[test]
    fn doorsteps_are_reserved_and_no_cut_crosses_them() {
        let t = test_type();
        for (seed, l) in layouts(&t) {
            let front = front(&l);
            for x in &l.levels {
                let inner = x.size.rect().inset(1);
                for d in l.frame.doors.iter().filter(|d| d.level == x.level) {
                    let Some(at) = d.at else { continue };
                    let step = at.neighbors4().find(|p| inner.contains(*p)).unwrap();
                    for p in std::iter::once(step).chain(step.neighbors4()) {
                        if inner.contains(p) {
                            assert!(x.reserved.contains(&p), "seed {seed} level {}", x.level);
                        }
                    }
                }
                for p in &x.reserved {
                    let i = x.bands.iter().position(|b| b.rect.contains(*p)).unwrap();
                    let cut = rows(front, inner, x.bands[i].rect).0;
                    assert!(i == 0 || deep(front, inner, *p) != cut, "seed {seed} {p:?}");
                }
            }
        }
    }

    #[test]
    fn a_level_too_shallow_for_its_bands_is_an_error() {
        let mut t = test_type();
        t.frame.footprint = (4, 3);
        t.frame.doors = vec![OwnDoor {
            stratum: StratumKind::Street,
            side: Dir::S,
            at: 1,
        }];
        let band = t.levels.ground.bands[1].clone();
        t.levels.ground.bands.extend([band.clone(), band]);
        for b in &mut t.levels.ground.bands {
            b.share = 25;
        }
        // 12 x 9: 7 rows deep, four bands of one row and a wall.
        let e = lay_out(&t, VenueFrame::own(&t), 1).unwrap_err();
        assert!(e.contains("level 0") && e.contains("too shallow"), "{e}");
    }

    #[test]
    fn a_thinned_band_gives_up_only_the_rows_the_doorsteps_take() {
        // 16 rows in three bands of five at full thickness. The first cut's full-thickness
        // rows, 5 and 6, are blocked: it takes the clear row next to them, not the jitter's.
        let blocked = BTreeSet::from([5, 6, 7]);
        for seed in 0..20 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let starts = cuts(&[15, 65, 20], 16, &blocked, &mut rng).unwrap();
            assert_eq!(starts, vec![0, 4, 11], "seed {seed}");
        }
    }

    #[test]
    fn a_cut_leaves_its_jitter_to_keep_full_thickness() {
        // 22 rows in halves: the jitter's rows 9 to 13 are all blocked, so the cut goes
        // just outside them, both bands keeping five rows or more.
        let blocked = BTreeSet::from([9, 10, 11, 12, 13]);
        let mut seen = BTreeSet::new();
        for seed in 0..20 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let starts = cuts(&[50, 50], 22, &blocked, &mut rng).unwrap();
            assert!(starts[1] == 8 || starts[1] == 14, "seed {seed}: {starts:?}");
            seen.insert(starts[1]);
        }
        assert_eq!(seen.len(), 2, "each side of the blocked rows");
    }

    #[test]
    fn neighbouring_bands_of_one_zone_get_a_plain_door() {
        let mut t = test_type();
        let role = t.levels.middle.as_mut().unwrap();
        // Its first two bands in one zone, the third in the second's: a plain cut, then a
        // border.
        let (first, second) = (role.bands[0].zone.clone(), role.bands[1].zone.clone());
        role.bands[1].zone = first;
        role.bands[2].zone = second;
        let (name, doors) = (role.name.clone(), usize::from(role.bands[1].doors));
        let mut seen = 0;
        for (seed, l) in layouts(&t) {
            for x in l.levels.iter().filter(|x| x.role == name) {
                let what = format!("seed {seed} level {}", x.level);
                let plain: Vec<_> = x
                    .doors
                    .iter()
                    .filter(|d| across(x, d.at).1.0 == 1)
                    .collect();
                assert_eq!(plain.len(), doors, "{what}");
                for d in &plain {
                    assert!(!d.border && d.fit.is_none() && d.lock.is_none(), "{what}");
                }
                let ((_, f), (_, b)) = across(x, plain[0].at);
                let (rf, rb) = (&x.rooms[plain[0].rooms.0], &x.rooms[plain[0].rooms.1]);
                assert!(rf.rect.contains(f) && rb.rect.contains(b), "{what}");
                assert_eq!((rf.zone, rb.zone), (x.bands[0].zone, x.bands[1].zone));
                for r in [rf, rb] {
                    assert!(r.kind == RoomKind::Corridor || x.spine == Spine::Hall);
                }
                // The plain doors take no lock number: the border's count from the first.
                let numbers: Vec<u16> = x
                    .doors
                    .iter()
                    .filter_map(|d| d.fit)
                    .map(|l| l.number)
                    .collect();
                let want: Vec<u16> = (LOCK_NUMBERS..).take(numbers.len()).collect();
                assert!(!numbers.is_empty(), "{what}");
                assert_eq!(numbers, want, "{what}");
                seen += 1;
            }
        }
        assert!(seen > 0);
    }
}
