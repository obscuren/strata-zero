//! The outside of a megablock: a forecourt at the main door, a service door round
//! the back, and a little street life around both. Part of the Dressing stage.

use super::dressing::{bare, open_area};
use super::{GenCtx, outward};
use crate::geom::Pos;
use crate::rng::hash64;
use crate::tile::{Feature, Furniture, Terrain};
use crate::world::{Building, BuildingKind, City, StratumKind};
use rand::seq::IndexedRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Salts the exterior's own RNG, so no other stage's stream moves.
const EXTERIOR_SALT: u64 = 0x000E_7E0F_B10C;

const K: StratumKind = StratumKind::Street;

#[cfg(test)]
thread_local! {
    /// Test-only switch: generate a city as if this pass did not exist.
    static SKIP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
thread_local! {
    /// Test-only log of the blockers the pass put down, to check none was lifted.
    static PLACED: std::cell::RefCell<Vec<(Pos, Feature)>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
fn skipped() -> bool {
    SKIP.with(std::cell::Cell::get)
}

#[cfg(not(test))]
fn skipped() -> bool {
    false
}

/// The building's main street door: among its Street door tiles whose outward
/// neighbour is outdoor ground, the one nearest the middle of its side. Ties go
/// to the lowest `(y, x)`. Once the exterior is dressed, the door under the canopy
/// wins, so the service door cut later never takes over.
pub(crate) fn main_door(city: &City, b: &Building) -> Option<Pos> {
    let fp = b.footprint;
    fp.border()
        .filter(|p| city.tile(K, *p).is_some_and(|t| t.terrain == Terrain::Door))
        .filter_map(|p| {
            let t = city.tile(K, outward(fp, p))?;
            t.is_outdoor_ground()
                .then_some((t.feature != Some(Feature::Awning), p))
        })
        .min_by_key(|(plain, p)| (*plain, off_middle(b, *p), p.y, p.x))
        .map(|(_, p)| p)
}

/// Twice the distance of a border tile from the middle of its side.
fn off_middle(b: &Building, p: Pos) -> i32 {
    let fp = b.footprint;
    let o = outward(fp, p);
    if o.x != p.x {
        (2 * p.y - (fp.y + fp.bottom() - 1)).abs()
    } else {
        (2 * p.x - (fp.x + fp.right() - 1)).abs()
    }
}

/// Rows outward from a façade and columns along it, from one door.
#[derive(Clone, Copy)]
struct Frame {
    door: Pos,
    out: (i32, i32),
}

impl Frame {
    fn new(fp: crate::geom::Rect, door: Pos) -> Self {
        let o = outward(fp, door);
        Frame {
            door,
            out: (o.x - door.x, o.y - door.y),
        }
    }

    /// The tile `row` steps out from the façade and `col` steps along it.
    fn at(self, row: i32, col: i32) -> Pos {
        let (dx, dy) = self.out;
        let (ax, ay) = (dy.abs(), dx.abs());
        self.door.offset(dx * row + ax * col, dy * row + ay * col)
    }
}

/// A tile 4-adjacent to a street door: where people step out.
fn landing(ctx: &GenCtx, p: Pos) -> bool {
    p.neighbors4().any(|n| ctx.terrain(K, n) == Terrain::Door)
}

/// Outdoor ground other than road: where the pass may put things.
fn pavement(ctx: &GenCtx, p: Pos) -> bool {
    matches!(
        ctx.terrain(K, p),
        Terrain::Sidewalk | Terrain::Plaza | Terrain::Alley
    )
}

/// A piece may stand here: bare pavement that is not a door's landing.
fn free(ctx: &GenCtx, p: Pos) -> bool {
    pavement(ctx, p) && bare(ctx, K, p) && !landing(ctx, p)
}

/// True when a blocker on `p` would leave its walkable 4-neighbours joined by
/// the ring of tiles around it, so it can cut nothing off.
fn pinch_free(ctx: &GenCtx, p: Pos) -> bool {
    const RING: [(i32, i32); 8] = [
        (0, -1),
        (1, -1),
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
    ];
    let open: Vec<bool> = RING
        .iter()
        .map(|(dx, dy)| ctx.city.walkable(K, p.offset(*dx, *dy)))
        .collect();
    // Label each run of walkable ring tiles; the ring wraps round.
    let Some(start) = (0..8).find(|i| !open[*i]) else {
        return true;
    };
    let mut run = [usize::MAX; 8];
    let mut label = 0;
    for step in 1..=8 {
        let i = (start + step) % 8;
        if open[i] {
            run[i] = label;
        } else {
            label += 1;
        }
    }
    let mut sides = (0..8).step_by(2).filter(|i| open[*i]).map(|i| run[i]);
    let first = sides.next();
    sides.all(|r| Some(r) == first)
}

fn place(ctx: &mut GenCtx, p: Pos, f: Feature) -> bool {
    if !free(ctx, p) {
        return false;
    }
    ctx.set_feature(K, p, Some(f));
    if !ctx.city.walkable(K, p) {
        ctx.set_feature(K, p, None);
        if !pinch_free(ctx, p) {
            return false;
        }
        ctx.set_feature(K, p, Some(f));
    }
    #[cfg(test)]
    if !ctx.city.walkable(K, p) {
        PLACED.with(|l| l.borrow_mut().push((p, f)));
    }
    true
}

/// Dresses the outside of building `i` if it is a megablock.
pub(crate) fn dress(ctx: &mut GenCtx, i: usize) {
    let b = ctx.city.buildings[i].clone();
    if b.kind != BuildingKind::Megablock || skipped() {
        return;
    }
    let mut rng = ChaCha8Rng::seed_from_u64(hash64(ctx.seed ^ EXTERIOR_SALT, b.id.0 as u64));
    let Some(door) = main_door(&ctx.city, &b) else {
        return;
    };
    let front = Frame::new(b.footprint, door);
    let land = front.at(1, 0);
    if !pavement(ctx, land) {
        return;
    }
    let paved = forecourt(ctx, front);

    // The canopy: on the landing itself, whatever the street pass left there, and
    // the pavement either side of it.
    ctx.set_feature(K, land, Some(Feature::Awning));
    for col in [-1, 1] {
        place(ctx, front.at(1, col), Feature::Awning);
    }
    for side in [-1, 1] {
        let far = front.at(1, 3 * side);
        let lamp = if paved.contains(&far) {
            far
        } else {
            front.at(1, 2 * side)
        };
        place(ctx, lamp, Feature::Lamp);
    }
    for side in [-1, 1] {
        place(
            ctx,
            front.at(1, 2 * side),
            Feature::Furniture(Furniture::Vending),
        );
    }
    // Bench and terminal stand in the second row; on a pavement too narrow for
    // one they move to the façade, a step past the lamps.
    let side = if rng.random_bool(0.5) { 1 } else { -1 };
    for (f, side) in [
        (Feature::Furniture(Furniture::Bench), side),
        (Feature::Terminal, -side),
    ] {
        let _ = place(ctx, front.at(2, 3 * side), f) || place(ctx, front.at(1, 4 * side), f);
    }

    if let Some(back) = service_door(ctx, &b, front) {
        service(ctx, &mut rng, back);
    }
    stall(ctx, &mut rng, front, &paved);
    bikes(ctx, &mut rng, front);
}

/// Paves up to three rows of sidewalk, three columns either side of the door, in
/// front of it. A row stops at the first tile that isn't sidewalk, and a column
/// never reaches past a gap in the row before it. Returns the paved tiles.
fn forecourt(ctx: &mut GenCtx, front: Frame) -> Vec<Pos> {
    let mut paved = Vec::new();
    let mut open = [true; 7];
    for row in 1..=3 {
        let mut next = [false; 7];
        for side in [-1, 1] {
            for step in 0..=3 {
                let col = step * side;
                let slot = (col + 3) as usize;
                let p = front.at(row, col);
                let pave = open[slot]
                    && matches!(ctx.terrain(K, p), Terrain::Sidewalk | Terrain::Plaza)
                    && ctx.city.connectors_at(p).next().is_none();
                if !pave {
                    break;
                }
                next[slot] = true;
                if !paved.contains(&p) {
                    ctx.set_terrain(K, p, Terrain::Plaza);
                    paved.push(p);
                }
            }
        }
        open = next;
    }
    paved
}

/// The border tiles on the side facing `out` that a service door may take: wall,
/// window or an existing door, off the corners, with standable outdoor ground
/// outside.
fn door_spots(ctx: &GenCtx, b: &Building, out: (i32, i32)) -> Vec<Pos> {
    let fp = b.footprint;
    fp.border()
        .filter(|p| {
            let corner =
                (p.x == fp.x || p.x == fp.right() - 1) && (p.y == fp.y || p.y == fp.bottom() - 1);
            !corner && Frame::new(fp, *p).out == out
        })
        .filter(|p| {
            matches!(
                ctx.terrain(K, *p),
                Terrain::Wall | Terrain::Window | Terrain::Door
            )
        })
        .filter(|p| {
            ctx.tile(K, outward(fp, *p))
                .is_some_and(|t| t.is_outdoor_ground() && t.is_walkable())
        })
        .collect()
}

/// Cuts the service door: on the side opposite the main door, nearest its middle;
/// failing that on a flank, the flank farther from the main door first. Returns
/// the service door's frame.
fn service_door(ctx: &mut GenCtx, b: &Building, front: Frame) -> Option<Frame> {
    let (dx, dy) = front.out;
    let back = door_spots(ctx, b, (-dx, -dy))
        .into_iter()
        .min_by_key(|p| (off_middle(b, *p), p.y, p.x));
    let back = back.or_else(|| {
        let mut flanks = [(dy, dx), (-dy, -dx)];
        // The flank farther from the main door goes first.
        flanks.sort_by_key(|out| -flank_gap(b, front.door, *out));
        flanks.into_iter().find_map(|out| {
            door_spots(ctx, b, out)
                .into_iter()
                .min_by_key(|p| (off_middle(b, *p), p.y, p.x))
        })
    })?;
    ctx.set_terrain(K, back, Terrain::Door);
    ctx.set_feature(K, back, None);
    Some(Frame::new(b.footprint, back))
}

/// How far the door stands from the footprint side that faces `out`.
fn flank_gap(b: &Building, door: Pos, out: (i32, i32)) -> i32 {
    let fp = b.footprint;
    match out {
        (1, _) => fp.right() - 1 - door.x,
        (-1, _) => door.x - fp.x,
        (_, 1) => fp.bottom() - 1 - door.y,
        _ => door.y - fp.y,
    }
}

/// Dumpsters on one side of the service door, a barriered loading bay on the
/// other, and a lamp beside the door.
fn service(ctx: &mut GenCtx, rng: &mut ChaCha8Rng, back: Frame) {
    let side = if rng.random_bool(0.5) { 1 } else { -1 };
    let bins = rng.random_range(2..=3);
    for step in 1..=bins {
        let p = back.at(1, step * side);
        if ctx.terrain(K, p) == Terrain::Sidewalk {
            place(ctx, p, Feature::Trash);
        }
    }
    // The bay is the two tiles between its barriers, two tiles off the landing.
    for step in [2, 5] {
        place(ctx, back.at(1, -step * side), Feature::Barrier);
    }
    place(ctx, back.at(1, -side), Feature::Lamp);
}

/// One food stall on the sidewalk near the forecourt, not on it.
fn stall(ctx: &mut GenCtx, rng: &mut ChaCha8Rng, front: Frame, paved: &[Pos]) {
    let around: Vec<Pos> = if paved.is_empty() {
        vec![front.at(1, 0)]
    } else {
        paved.to_vec()
    };
    let near = |p: Pos| {
        around
            .iter()
            .map(|q| q.chebyshev(p))
            .min()
            .unwrap_or(i32::MAX)
    };
    let spots: Vec<Pos> = crate::geom::Rect::new(front.door.x - 10, front.door.y - 10, 21, 21)
        .iter()
        .filter(|p| (1..=6).contains(&near(*p)))
        .filter(|p| ctx.terrain(K, *p) == Terrain::Sidewalk)
        .filter(|p| free(ctx, *p) && open_area(ctx, K, *p))
        .collect();
    if let Some(p) = spots.choose(rng) {
        place(ctx, *p, Feature::Stall);
    }
}

/// A row of three to five bikes on the kerb along the main door's side, past the
/// forecourt.
fn bikes(ctx: &mut GenCtx, rng: &mut ChaCha8Rng, front: Frame) {
    let side = if rng.random_bool(0.5) { 1 } else { -1 };
    let want = rng.random_range(3..=5);
    let mut parked = 0;
    for step in 5..5 + 3 * want {
        if parked == want {
            break;
        }
        let kerb = (1..=4).map(|row| front.at(row, step * side)).find(|p| {
            ctx.terrain(K, *p) == Terrain::Sidewalk
                && p.neighbors4().any(|n| ctx.terrain(K, n) == Terrain::Road)
        });
        let Some(p) = kerb else { continue };
        if open_area(ctx, K, p) && place(ctx, p, Feature::Furniture(Furniture::Bike)) {
            parked += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Generator;
    use crate::generate::connectivity::reachable_from_street;
    use crate::params::{GenParams, SizePreset};
    use std::sync::OnceLock;

    /// Medium seeds 1, 2, 8 and 11 (the plan's fixtures) plus the rest of 1..=40.
    fn fixture_seeds() -> std::ops::RangeInclusive<u64> {
        1..=40
    }

    fn params(seed: u64, size: SizePreset, districts: u8) -> GenParams {
        GenParams {
            seed,
            size,
            districts,
            ..Default::default()
        }
    }

    fn medium_params(seed: u64) -> GenParams {
        params(seed, SizePreset::Medium, 7)
    }

    /// Generates a city, with or without this pass, and the blockers the pass put down.
    fn run(p: GenParams, skip: bool) -> (City, Vec<(Pos, Feature)>) {
        SKIP.with(|s| s.set(skip));
        PLACED.with(|l| l.borrow_mut().clear());
        let c = Generator::run(p);
        SKIP.with(|s| s.set(false));
        let placed = PLACED.with(|l| std::mem::take(&mut *l.borrow_mut()));
        (c.unwrap(), placed)
    }

    struct Fixture {
        seed: u64,
        city: City,
        placed: Vec<(Pos, Feature)>,
    }

    /// The medium fixture cities, generated once for every test here.
    fn mediums() -> &'static [Fixture] {
        static CITIES: OnceLock<Vec<Fixture>> = OnceLock::new();
        CITIES.get_or_init(|| {
            fixture_seeds()
                .map(|seed| {
                    let (city, placed) = run(medium_params(seed), false);
                    Fixture { seed, city, placed }
                })
                .collect()
        })
    }

    fn megablocks(c: &City) -> impl Iterator<Item = &Building> {
        c.buildings
            .iter()
            .filter(|b| b.kind == BuildingKind::Megablock)
    }

    #[test]
    fn every_megablock_gets_a_forecourt_a_canopy_and_lamps() {
        let mut seen = 0;
        for Fixture { seed, city: c, .. } in mediums() {
            for b in megablocks(c) {
                seen += 1;
                let door = main_door(c, b).expect("a megablock has a street door");
                let land = outward(b.footprint, door);
                let t = c.tile(K, land).unwrap();
                assert_eq!(t.feature, Some(Feature::Awning), "seed {seed}");
                assert!(t.is_walkable());
                let lamps = b
                    .footprint
                    .grow(4)
                    .iter()
                    .filter(|p| {
                        c.tile(K, *p)
                            .is_some_and(|t| t.feature == Some(Feature::Lamp))
                    })
                    .count();
                assert!(lamps >= 2, "seed {seed}");
            }
        }
        assert!(seen >= 4, "{seen} megablocks");
    }

    #[test]
    fn the_dressing_never_touches_roads_or_cuts_anything_off() {
        for Fixture {
            seed,
            city: after,
            placed,
        } in mediums()
        {
            if megablocks(after).next().is_none() {
                continue;
            }
            let (before, _) = run(medium_params(*seed), true);
            let (sb, sa) = (before.stratum(K).unwrap(), after.stratum(K).unwrap());
            for (p, t) in sb.tiles.iter() {
                if t.terrain == Terrain::Road {
                    assert_eq!(sa.tiles[p], *t, "seed {seed}: road at {p:?} changed");
                }
            }
            let (rb, ra) = (reachable_from_street(&before), reachable_from_street(after));
            let (rb, ra) = (&rb[K.index()], &ra[K.index()]);
            for (p, t) in sb.tiles.iter() {
                if t.is_outdoor_ground() && rb[p] && sa.tiles[p].is_walkable() {
                    assert!(ra[p], "seed {seed}: {p:?} was cut off");
                }
            }
            // Verify never had to lift a blocker the pass put down.
            assert!(!placed.is_empty(), "seed {seed}");
            for (p, f) in placed {
                assert_eq!(sa.tiles[*p].feature, Some(*f), "seed {seed}: {p:?} lifted");
            }
        }
    }

    #[test]
    fn the_service_door_is_a_real_street_exit() {
        let street_doors = |c: &City, b: &Building| -> Vec<Pos> {
            c.portals(b.id)
                .into_iter()
                .filter(|(k, _)| *k == K)
                .map(|(_, p)| p)
                .collect()
        };
        for Fixture { seed, city: c, .. } in mediums() {
            if megablocks(c).next().is_none() {
                continue;
            }
            let (plain, _) = run(medium_params(*seed), true);
            for b in megablocks(c) {
                let (after, before) = (street_doors(c, b), street_doors(&plain, b));
                assert!(
                    after.len() > before.len(),
                    "seed {seed}: megablock {} has {} street doors, {} before",
                    b.id.0,
                    after.len(),
                    before.len()
                );
                let front = Frame::new(b.footprint, main_door(c, b).unwrap()).out;
                for door in after.iter().filter(|p| !before.contains(p)) {
                    assert_ne!(
                        Frame::new(b.footprint, *door).out,
                        front,
                        "seed {seed}: the service door {door:?} is on the main door's side"
                    );
                    let t = c.tile(K, outward(b.footprint, *door)).unwrap();
                    assert!(
                        t.is_outdoor_ground() && t.is_walkable(),
                        "seed {seed}: the service door {door:?} opens onto {t:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_street_pieces_are_there_and_off_the_landings() {
        let (mut seen, mut paved, mut vending, mut stalls, mut bikes) = (0, 0, 0, 0, 0);
        for Fixture { seed, city: c, .. } in mediums() {
            let st = c.stratum(K).unwrap();
            for b in megablocks(c) {
                seen += 1;
                let near = b.footprint.grow(14);
                let count = |f: Feature| {
                    near.iter()
                        .filter(|p| st.tiles.get(*p).is_some_and(|t| t.feature == Some(f)))
                        .count()
                };
                let plaza = near
                    .iter()
                    .filter(|p| {
                        st.tiles
                            .get(*p)
                            .is_some_and(|t| t.terrain == Terrain::Plaza)
                    })
                    .count();
                paved += usize::from(plaza >= 3);
                vending += count(Feature::Furniture(Furniture::Vending)).min(1);
                stalls += count(Feature::Stall).min(1);
                bikes += count(Feature::Furniture(Furniture::Bike));
                // The megablock's door landings stay walkable.
                for p in b.footprint.border() {
                    if st.tiles[p].terrain == Terrain::Door {
                        let o = outward(b.footprint, p);
                        assert!(
                            st.tiles.get(o).is_none_or(|t| t.is_walkable()),
                            "seed {seed}"
                        );
                    }
                }
            }
        }
        // Narrow pavements squeeze pieces out, but most megablocks get them all.
        assert!(
            paved * 2 >= seen,
            "{paved} forecourts for {seen} megablocks"
        );
        assert!(
            vending * 2 >= seen,
            "{vending} vending for {seen} megablocks"
        );
        assert!(stalls * 2 >= seen, "{stalls} stalls for {seen} megablocks");
        assert!(bikes >= 3 * seen / 2, "{bikes} bikes for {seen} megablocks");
    }

    #[test]
    fn cities_without_a_megablock_are_unchanged() {
        for p in [
            params(6, SizePreset::Custom { w: 120, h: 60 }, 8),
            medium_params(16),
        ] {
            let seed = p.seed;
            let (with, _) = run(p.clone(), false);
            assert_eq!(megablocks(&with).count(), 0, "seed {seed}");
            let (without, _) = run(p, true);
            assert_eq!(
                postcard::to_allocvec(&with).unwrap(),
                postcard::to_allocvec(&without).unwrap(),
                "seed {seed}"
            );
        }
    }

    /// A megablock city: the pass changes the street (and so the Net), but no other
    /// stage's stream moves, so everything else generates exactly as before.
    #[test]
    fn the_pass_draws_from_no_other_stream() {
        let (with, _) = run(medium_params(8), false);
        let (without, _) = run(medium_params(8), true);
        assert!(megablocks(&with).count() > 0);
        assert_eq!(with.name, without.name);
        assert_eq!(with.buildings, without.buildings);
        assert_eq!(with.districts, without.districts);
        assert_eq!(with.arterials, without.arterials);
        for k in StratumKind::PHYSICAL {
            if k == K {
                continue;
            }
            assert!(with.stratum(k) == without.stratum(k), "{k:?} changed");
        }
        assert_ne!(with.stratum(K), without.stratum(K));
    }

    #[test]
    fn outward_steps_off_the_footprint() {
        let r = crate::geom::Rect::new(2, 3, 5, 4);
        assert_eq!(outward(r, Pos::new(2, 4)), Pos::new(1, 4));
        assert_eq!(outward(r, Pos::new(6, 4)), Pos::new(7, 4));
        assert_eq!(outward(r, Pos::new(4, 3)), Pos::new(4, 2));
        assert_eq!(outward(r, Pos::new(4, 6)), Pos::new(4, 7));
    }
}
