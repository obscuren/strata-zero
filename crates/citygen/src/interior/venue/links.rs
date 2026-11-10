//! The links stage: each of the type's links given a stop on every level it serves,
//! lowest first. A stop lands on free floor of a room of its zone that anyone may enter: a
//! lift's bank is two tiles side by side, anything else one tile. Lifts, stairs, ladders
//! and crawls stack: each stop takes the tile of the stop below where it can, else a free
//! tile near it. An escalator's stop lands 3 to 5 tiles on from the one below, along the
//! spine. A landing and the floor beside it are reserved, and a spot there is a spot no
//! more.
//!
//! A landing keeps clear of the landings of the links laid before it, unless its level
//! has no such tile left in its zone: then it stands beside one, as today's lift bank and
//! stairwell do, but never on one, on a doorstep or on the tile a roof door keeps. A
//! lift, stair, ladder or crawl passes a level with no room left in its zone at all; an
//! escalator can't, nor can a link left with fewer than two stops, and every level of a
//! building of two levels or more keeps a stop of some link.

use super::rooms::Taken;
use super::zones::{self, Axes};
use super::{Alight, LevelLayout, LinkDef, LinkDir, Serves, VenueLayout, VenueType};
use crate::geom::{Dir, Pos};
use crate::interior::{FloorPlan, LevelLink, LinkKind, LinkStop};
use crate::world::StratumKind;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

/// The links stage's rng, from the layout seed.
pub(super) const STAGE: u64 = 0x4_11C5;

/// How far an escalator's upper landing lies from its lower one: the least and the most
/// tiles.
const RISE: (i32, i32) = (3, 5);

/// Gives each of the type's links, in its order, a stop on every level of the layout it
/// serves, passing those with no room for it; a link that serves fewer than two of them
/// has none. Each link's landings are reserved before the next is laid. Errs, naming the
/// link and the level, when an escalator finds no room, when passing leaves a link fewer
/// than two stops, or when a level of two or more is left with no stop at all.
pub(super) fn lay(
    t: &VenueType,
    layout: &mut VenueLayout,
    rng: &mut ChaCha8Rng,
) -> Result<(), String> {
    let front = zones::front(&layout.frame);
    let roofs: Vec<i32> = (layout.frame.doors.iter())
        .filter(|d| d.stratum == StratumKind::Heights)
        .map(|d| d.level)
        .collect();
    // The roof levels a stair already keeps a tile free beside.
    let mut roofed: BTreeSet<i32> = BTreeSet::new();
    let mut held: Vec<Held> = (layout.levels.iter())
        .map(|x| Held {
            steps: x.reserved.iter().copied().collect(),
            taken: BTreeSet::new(),
        })
        .collect();
    // Each passed level, and the first link that passed it.
    let mut passed: BTreeMap<usize, &str> = BTreeMap::new();
    layout.links.clear();
    for def in &t.links {
        let zone = t
            .zone(&def.zone)
            .ok_or_else(|| format!("{}: the {} names no zone `{}`", t.id, def.name, def.zone))?;
        let served: Vec<usize> = (0..layout.levels.len())
            .filter(|&i| serves(def, &layout.levels[i]))
            .collect();
        if served.len() < 2 {
            continue;
        }
        let mut wants = Vec::new();
        for &i in &served {
            let x = &layout.levels[i];
            let card = match def.cards.iter().find(|(role, _)| *role == x.role) {
                Some((_, z)) => t.zone(z).ok_or_else(|| {
                    format!("{}: the {}'s card names no zone `{z}`", t.id, def.name)
                })?,
                None => 0,
            };
            let roof = def.kind == LinkKind::Stair
                && roofs.contains(&x.level)
                && !roofed.contains(&x.level);
            wants.push(Want {
                card,
                zone: zone_on(x, if card > 0 { card } else { zone }),
                roof,
            });
        }
        let grounds: Vec<Ground> = (served.iter().zip(&wants))
            .map(|(&i, w)| Ground::new(&layout.levels[i], &held[i], front, def.kind, w))
            .collect();
        let full = |i: usize| no_room(t, &def.name, layout.levels[i].level);
        let at: Vec<Option<Pos>> = if def.kind == LinkKind::Escalator {
            let at = escalate(&grounds, rng).map_err(|k| full(served[k]))?;
            at.into_iter().map(Some).collect()
        } else {
            stack(&grounds, rng)
        };
        let laid: Vec<usize> = (0..served.len()).filter(|&k| at[k].is_some()).collect();
        if laid.len() < 2 {
            let k = (0..served.len()).find(|&k| at[k].is_none()).unwrap_or(0);
            return Err(full(served[k]));
        }
        for (k, &i) in served.iter().enumerate() {
            if at[k].is_none() {
                passed.entry(i).or_insert(&def.name);
            }
        }
        let mut stops = Vec::new();
        for (j, &k) in laid.iter().enumerate() {
            let (i, at) = (served[k], at[k].expect("a laid stop"));
            let x = &mut layout.levels[i];
            reserve(x, def.kind, at);
            held[i].taken.extend(landing(def.kind, at));
            if grounds[k].roof {
                roofed.insert(x.level);
                held[i].taken.insert(at.offset(1, 0));
            }
            let (on, off) = rides(def, j, laid.len());
            stops.push(LinkStop {
                level: x.level,
                at,
                card: wants[k].card,
                on,
                off,
            });
        }
        layout.links.push(LevelLink {
            kind: def.kind,
            stops,
            name: Some(def.name.clone()),
        });
    }
    // A level no link stops on can't be reached.
    if layout.levels.len() > 1 {
        for (i, x) in layout.levels.iter().enumerate() {
            if layout.links.iter().any(|k| k.stop(x.level).is_some()) {
                continue;
            }
            return Err(match passed.get(&i) {
                Some(name) => no_room(t, name, x.level),
                None => format!("{}: no link stops on {}", t.id, FloorPlan::label(x.level)),
            });
        }
    }
    Ok(())
}

/// The error for a link with no room on a level.
fn no_room(t: &VenueType, link: &str, level: i32) -> String {
    format!(
        "{}: no room for the {link} on {}",
        t.id,
        FloorPlan::label(level)
    )
}

/// Whether `def` stops on level `x`.
fn serves(def: &LinkDef, x: &LevelLayout) -> bool {
    match &def.serves {
        Serves::All => true,
        Serves::Roles(names) => names.contains(&x.role),
        Serves::Levels(levels) => levels.contains(&x.level),
    }
}

/// The zone a stop asking for `want` lands in on `x`: `want` when the level has it, else
/// the next deeper zone it has, else its deepest.
fn zone_on(x: &LevelLayout, want: u8) -> u8 {
    let has: BTreeSet<u8> = x.bands.iter().map(|b| b.zone).collect();
    if has.contains(&want) {
        return want;
    }
    has.range(want..)
        .next()
        .or(has.last())
        .copied()
        .unwrap_or(want)
}

/// Where you can get on and off at stop `k` of `n`: a one-way link takes you on below
/// (`Up`) or above (`Down`) its last stop that way, and off beyond its first; a stair
/// you leave only at its ends lets you off nowhere else.
fn rides(def: &LinkDef, k: usize, n: usize) -> (bool, bool) {
    let (first, last) = (k == 0, k == n - 1);
    let (on, off) = match def.dir {
        LinkDir::Both => (true, true),
        LinkDir::Up => (!last, !first),
        LinkDir::Down => (!first, !last),
    };
    (on, off && (def.alight == Alight::All || first || last))
}

/// A landing's tiles from `at`: a lift's bank is `at` and the tile east of it.
fn landing(kind: LinkKind, at: Pos) -> Vec<Pos> {
    if kind == LinkKind::Lift {
        vec![at, at.offset(1, 0)]
    } else {
        vec![at]
    }
}

/// Whether `p` is floor of a room on `x` that a landing may stand beside: out of the core
/// and off the pieces.
fn floor(x: &LevelLayout, p: Pos) -> bool {
    x.rooms.iter().any(|r| r.rect.contains(p))
        && !x.core.is_some_and(|c| c.contains(p))
        && !x.pieces.iter().any(|q| q.rect.contains(p))
}

/// Reserves a landing of `kind` at `at` on `x` and the floor beside it, and takes the
/// spots there from their rooms. A room left with no spot takes the free tile nearest its
/// centre, as the rooms stage gives one.
fn reserve(x: &mut LevelLayout, kind: LinkKind, at: Pos) {
    let near: BTreeSet<Pos> = landing(kind, at)
        .into_iter()
        .flat_map(|p| std::iter::once(p).chain(p.neighbors4()))
        .filter(|p| floor(x, *p))
        .collect();
    let mut reserved: BTreeSet<Pos> = x.reserved.iter().copied().collect();
    reserved.extend(&near);
    x.reserved = reserved.into_iter().collect();
    let taken = Taken::of(x);
    for r in &mut x.rooms {
        let had = r.spots.len();
        r.spots.retain(|s| !near.contains(s));
        if had > 0 && r.spots.is_empty() {
            r.spots.extend(taken.nearest(r.rect));
        }
    }
}

/// What a link asks of its stop on one level.
struct Want {
    /// The zone card it asks for there, 0 for none.
    card: u8,
    /// The zone its landing opens into.
    zone: u8,
    /// The first stair on a level with a roof door keeps the tile east of it free, for
    /// the door, where it can.
    roof: bool,
}

/// What the links stage keeps of one level as it lays.
struct Held {
    /// The doorsteps and the tiles beside them, reserved before any link.
    steps: BTreeSet<Pos>,
    /// The landings laid so far, and the tile a roof door keeps beside its stair.
    taken: BTreeSet<Pos>,
}

/// The tiles a link's stop can land on, on one level.
struct Ground {
    axes: Axes,
    /// Each tile a landing's first tile fits on, and how many of the tiles around the
    /// landing are floor. Empty: no room, and a stacking link passes the level.
    fits: BTreeMap<Pos, usize>,
    /// Each place keeps free floor east of it, for a roof door.
    roof: bool,
}

impl Ground {
    /// Where a landing of `kind` fits on `x`: every tile of it on free floor of one room of
    /// the zone `w` wants that anyone may enter (not on a door, a doorstep, another
    /// landing or beside one, a piece or the core), with free floor east of it on a roof.
    /// A level with no such place left lets it stand beside another landing; a roof's
    /// stair with nowhere to keep its east tile free stands where it can.
    fn new(x: &LevelLayout, held: &Held, front: Dir, kind: LinkKind, w: &Want) -> Ground {
        let reserved: BTreeSet<Pos> = x.reserved.iter().copied().collect();
        let doors: BTreeSet<Pos> = x.doors.iter().map(|d| d.at).collect();
        let clear = |p: Pos| floor(x, p) && !reserved.contains(&p) && !doors.contains(&p);
        let crowded = |p: Pos| {
            floor(x, p)
                && !held.steps.contains(&p)
                && !held.taken.contains(&p)
                && !doors.contains(&p)
        };
        let fits = |free: &dyn Fn(Pos) -> bool, roof: bool| {
            let mut fits = BTreeMap::new();
            for r in x.rooms.iter().filter(|r| r.zone == w.zone && !r.private) {
                for at in r.rect.iter() {
                    let tiles = landing(kind, at);
                    let fit = tiles.iter().all(|p| r.rect.contains(*p) && free(*p))
                        && (!roof || free(at.offset(1, 0)));
                    if !fit {
                        continue;
                    }
                    let around: BTreeSet<Pos> = (tiles.iter())
                        .flat_map(|p| p.neighbors4())
                        .filter(|p| !tiles.contains(p))
                        .collect();
                    fits.insert(at, around.into_iter().filter(|p| floor(x, *p)).count());
                }
            }
            fits
        };
        let tiers: &[(&dyn Fn(Pos) -> bool, bool)] = if w.roof {
            &[
                (&clear, true),
                (&crowded, true),
                (&clear, false),
                (&crowded, false),
            ]
        } else {
            &[(&clear, false), (&crowded, false)]
        };
        let (fits, roof) = (tiers.iter())
            .map(|&(free, roof)| (fits(free, roof), roof))
            .find(|(f, _)| !f.is_empty())
            .unwrap_or_default();
        Ground {
            axes: Axes::new(x.size, front),
            fits,
            roof,
        }
    }

    fn fits(&self, at: Pos) -> bool {
        self.fits.contains_key(&at)
    }

    /// How many of the tiles around a landing at `at` are floor.
    fn open(&self, at: Pos) -> usize {
        self.fits.get(&at).copied().unwrap_or(0)
    }
}

/// How many levels from `k` on a landing at `at` fits, one after another; a level the
/// link passes doesn't break the run.
fn run(grounds: &[Ground], k: usize, at: Pos) -> usize {
    (grounds[k..].iter())
        .filter(|g| !g.fits.is_empty())
        .take_while(|g| g.fits(at))
        .count()
}

/// A stacking link's landings, one per ground, None where it passes a level with no room
/// for it: the first where it fits the most levels running up and has the most floor
/// around it, the rng picking among equals; each next on the tile of the one below where
/// it fits, else on the tile that fits the most levels running up from there, the nearest
/// of those (then the one with the most floor around it). Choosing the longest run each
/// time it moves, it moves the fewest times.
fn stack(grounds: &[Ground], rng: &mut ChaCha8Rng) -> Vec<Option<Pos>> {
    let rank = |k: usize, at: Pos| (run(grounds, k, at), grounds[k].open(at));
    let mut below: Option<Pos> = None;
    let mut at = Vec::new();
    for (k, g) in grounds.iter().enumerate() {
        let next = match below {
            _ if g.fits.is_empty() => None,
            Some(b) if g.fits(b) => Some(b),
            Some(b) => (g.fits.keys().copied()).min_by_key(|&p| {
                let (run, open) = rank(k, p);
                (Reverse(run), p.manhattan(b), Reverse(open), p.y, p.x)
            }),
            None => {
                let best = g.fits.keys().map(|&p| rank(k, p)).max();
                let firsts: Vec<Pos> = (g.fits.keys().copied())
                    .filter(|&p| Some(rank(k, p)) == best)
                    .collect();
                Some(firsts[rng.random_range(0..firsts.len())])
            }
        };
        below = next.or(below);
        at.push(next);
    }
    at
}

/// An escalator's landings, one per ground: the first on a tile with the most floor
/// around it (the rng picking among equals) from which the rest can follow, each next 3
/// to 5 tiles on from the one below, straight along the spine where it can (away from
/// the front first), the rng drawing how far. Err: the ground the furthest try found no
/// place on.
fn escalate(grounds: &[Ground], rng: &mut ChaCha8Rng) -> Result<Vec<Pos>, usize> {
    let want = rng.random_range(RISE.0..=RISE.1);
    let mut firsts: Vec<Pos> = grounds[0].fits.keys().copied().collect();
    for i in (1..firsts.len()).rev() {
        let j = rng.random_range(0..=i);
        firsts.swap(i, j);
    }
    firsts.sort_by_key(|&at| Reverse(grounds[0].open(at)));
    let mut furthest = 0;
    for first in firsts {
        let mut at = vec![first];
        for (k, g) in grounds.iter().enumerate().skip(1) {
            let below = at[k - 1];
            let next = (g.fits.keys().copied())
                .filter(|p| (RISE.0..=RISE.1).contains(&p.manhattan(below)))
                .min_by_key(|&p| {
                    let (on, across) = (
                        g.axes.deep(p) - g.axes.deep(below),
                        g.axes.across(p) - g.axes.across(below),
                    );
                    let along = match (across, on > 0) {
                        (0, true) => 0,
                        (0, false) => 1,
                        _ => 2,
                    };
                    (along, (p.manhattan(below) - want).abs(), p.y, p.x)
                });
            match next {
                Some(p) => at.push(p),
                None => {
                    furthest = furthest.max(k);
                    break;
                }
            }
        }
        if at.len() == grounds.len() {
            return Ok(at);
        }
    }
    Err(furthest)
}

#[cfg(test)]
mod tests {
    use super::super::{levels, rooms, zones};
    use super::{STAGE, lay};
    use crate::geom::Pos;
    use crate::interior::venue::testutil::test_type;
    use crate::interior::venue::{
        Alight, LevelLayout, LinkDef, LinkDir, PlannedRoom, Serves, VenueFrame, VenueLayout,
        VenuePack, VenueType, lay_out,
    };
    use crate::interior::{FloorPlan, LevelLink, LinkKind};
    use crate::rng::hash64;
    use crate::world::StratumKind;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;
    use std::collections::BTreeSet;

    fn layouts(t: &VenueType) -> Vec<(u64, VenueLayout)> {
        (0..20)
            .map(|s| (s, lay_out(t, VenueFrame::own(t), s).unwrap()))
            .collect()
    }

    /// `t` over its own frame from `seed`, laid as `lay_out` lays it, but with every room
    /// on `level` that `shut` picks made private before the links, so no landing stands
    /// in it.
    fn crowded(
        t: &VenueType,
        seed: u64,
        level: i32,
        shut: impl Fn(&PlannedRoom) -> bool,
    ) -> Result<VenueLayout, String> {
        let rng = |stage: u64| ChaCha8Rng::seed_from_u64(hash64(seed, stage));
        let mut l = VenueLayout {
            type_id: t.id.clone(),
            type_name: t.name.clone(),
            seed,
            frame: VenueFrame::own(t),
            zones: t.zones.clone(),
            levels: Vec::new(),
            links: Vec::new(),
        };
        levels::place(t, &mut l, &mut rng(levels::STAGE))?;
        zones::lay(t, &mut l, &mut rng(zones::STAGE))?;
        rooms::lay(t, &mut l, &mut rng(rooms::STAGE))?;
        let x = l.levels.iter_mut().find(|x| x.level == level).unwrap();
        for r in x.rooms.iter_mut().filter(|r| shut(r)) {
            r.private = true;
        }
        lay(t, &mut l, &mut rng(STAGE))?;
        Ok(l)
    }

    /// The levels of `l` that `def` serves, lowest first.
    fn served(def: &LinkDef, l: &VenueLayout) -> Vec<i32> {
        l.levels
            .iter()
            .filter(|x| match &def.serves {
                Serves::All => true,
                Serves::Roles(names) => names.contains(&x.role),
                Serves::Levels(levels) => levels.contains(&x.level),
            })
            .map(|x| x.level)
            .collect()
    }

    /// The zone `def`'s stop on `x` lands in: its card's zone where it asks for one, else
    /// its own; on a level without that zone, the next deeper zone the level has, else
    /// its deepest.
    fn stop_zone(t: &VenueType, def: &LinkDef, x: &LevelLayout) -> u8 {
        let card = def.cards.iter().find(|(role, _)| *role == x.role);
        let want = t.zone(card.map_or(&def.zone, |(_, z)| z)).unwrap();
        let has: BTreeSet<u8> = x.bands.iter().map(|b| b.zone).collect();
        if has.contains(&want) {
            want
        } else {
            let deeper = has.iter().find(|z| **z > want);
            *deeper.or(has.last()).unwrap()
        }
    }

    /// Each frame door's tile inside the frame wall on `x`, and the tiles beside it.
    fn doorsteps(l: &VenueLayout, x: &LevelLayout) -> BTreeSet<Pos> {
        let inner = x.size.rect().inset(1);
        l.frame
            .doors
            .iter()
            .filter(|d| d.level == x.level)
            .filter_map(|d| d.at)
            .flat_map(|at| {
                let step = at.neighbors4().find(|p| inner.contains(*p)).unwrap();
                std::iter::once(step).chain(step.neighbors4())
            })
            .filter(|p| inner.contains(*p))
            .collect()
    }

    /// Every landing tile on `x`.
    fn landings(l: &VenueLayout, x: &LevelLayout) -> BTreeSet<Pos> {
        l.links.iter().flat_map(|k| k.tiles(x.level)).collect()
    }

    /// Whether `p` is floor of a room on `x`: in a room, out of the core, on no door and
    /// no piece.
    fn floor(x: &LevelLayout, p: Pos) -> bool {
        x.rooms.iter().any(|r| r.rect.contains(p))
            && !x.core.is_some_and(|c| c.contains(p))
            && !x.doors.iter().any(|d| d.at == p)
            && !x.pieces.iter().any(|q| q.rect.contains(p))
    }

    fn link<'a>(l: &'a VenueLayout, def: &LinkDef) -> &'a LevelLink {
        l.links
            .iter()
            .find(|k| k.name.as_deref() == Some(def.name.as_str()))
            .unwrap_or_else(|| panic!("no {}", def.name))
    }

    #[test]
    fn every_link_serves_its_levels_from_rooms_of_its_zone() {
        let t = test_type();
        for (seed, l) in layouts(&t) {
            let names: Vec<Option<&str>> = l.links.iter().map(|k| k.name.as_deref()).collect();
            let want: Vec<Option<&str>> = t.links.iter().map(|d| Some(d.name.as_str())).collect();
            assert_eq!(names, want, "seed {seed}: in the type's order");
            for (def, k) in t.links.iter().zip(&l.links) {
                let what = format!("seed {seed}: the {}", def.name);
                assert_eq!(k.kind, def.kind, "{what}");
                let levels: Vec<i32> = k.stops.iter().map(|s| s.level).collect();
                assert_eq!(levels, served(def, &l), "{what}");
                for s in &k.stops {
                    let x = l.level(s.level).unwrap();
                    let what = format!("{what} on {}", s.level);
                    let zone = stop_zone(&t, def, x);
                    let steps = doorsteps(&l, x);
                    let spots: BTreeSet<Pos> =
                        x.rooms.iter().flat_map(|r| r.spots.clone()).collect();
                    let tiles = k.tiles(s.level);
                    assert_eq!(tiles.len(), if k.kind == LinkKind::Lift { 2 } else { 1 });
                    for p in tiles {
                        let r = x.rooms.iter().find(|r| r.rect.contains(p));
                        let r = r.unwrap_or_else(|| panic!("{what}: {p:?} in no room"));
                        assert_eq!(r.zone, zone, "{what}: {p:?} in {}", r.name);
                        assert!(!r.private, "{what}: {p:?} in {}", r.name);
                        assert!(floor(x, p), "{what}: {p:?} on a door, a piece or the core");
                        assert!(!steps.contains(&p), "{what}: {p:?} on a doorstep");
                        assert!(!spots.contains(&p), "{what}: {p:?} on a spot");
                    }
                }
            }
        }
    }

    #[test]
    fn lifts_stairs_and_crawls_stack_where_they_can() {
        let t = test_type();
        let stacked = |k: LinkKind| matches!(k, LinkKind::Lift | LinkKind::Stair | LinkKind::Crawl);
        let want = t.links.iter().filter(|d| stacked(d.kind)).count();
        for (seed, l) in layouts(&t) {
            let links: Vec<&LevelLink> = l.links.iter().filter(|k| stacked(k.kind)).collect();
            assert_eq!(links.len(), want, "seed {seed}");
            for k in links {
                let pairs = k.stops.len() - 1;
                let shared = k.stops.windows(2).filter(|w| w[0].at == w[1].at).count();
                assert!(
                    2 * shared >= pairs,
                    "seed {seed}: the {:?} shares {shared} of {pairs}: {:?}",
                    k.name,
                    k.stops.iter().map(|s| s.at).collect::<Vec<_>>()
                );
            }
        }
    }

    #[test]
    fn an_escalator_lands_a_few_tiles_on() {
        let t = test_type();
        let def = t
            .links
            .iter()
            .find(|d| d.kind == LinkKind::Escalator && d.dir == LinkDir::Up)
            .unwrap();
        for (seed, l) in layouts(&t) {
            let k = link(&l, def);
            assert_eq!(k.stops.len(), 2, "seed {seed}");
            let (low, high) = (k.stops[0], k.stops[1]);
            let apart = low.at.manhattan(high.at);
            assert!((3..=5).contains(&apart), "seed {seed}: {apart} apart");
            assert!(low.on && !low.off, "seed {seed}: {low:?}");
            assert!(!high.on && high.off, "seed {seed}: {high:?}");
        }
    }

    #[test]
    fn a_fire_stair_lets_you_off_only_at_its_ends() {
        let t = test_type();
        let def = t.links.iter().find(|d| d.alight == Alight::Ends).unwrap();
        for (seed, l) in layouts(&t) {
            let stops = &link(&l, def).stops;
            assert!(stops.len() > 2, "seed {seed}");
            for (i, s) in stops.iter().enumerate() {
                let end = i == 0 || i == stops.len() - 1;
                assert!(s.on, "seed {seed}: {s:?}");
                assert_eq!(s.off, end, "seed {seed}: {s:?}");
            }
        }
    }

    #[test]
    fn a_lift_asks_for_its_card_where_the_type_says() {
        let t = test_type();
        let def = t
            .links
            .iter()
            .find(|d| d.kind == LinkKind::Lift && !d.cards.is_empty())
            .unwrap();
        let (role, zone) = &def.cards[0];
        let mut carded = 0;
        for (seed, l) in layouts(&t) {
            for s in &link(&l, def).stops {
                let x = l.level(s.level).unwrap();
                let want = if &x.role == role {
                    carded += 1;
                    t.zone(zone).unwrap()
                } else {
                    0
                };
                assert_eq!(s.card, want, "seed {seed}: {s:?}");
                assert!(s.on && s.off, "seed {seed}: {s:?}");
            }
        }
        assert!(carded > 0);
    }

    #[test]
    fn a_roof_level_keeps_a_stair_with_room_beside_it() {
        let t = test_type();
        let mut roofs = 0;
        for (seed, l) in layouts(&t) {
            for d in l
                .frame
                .doors
                .iter()
                .filter(|d| d.stratum == StratumKind::Heights)
            {
                let x = l.level(d.level).unwrap();
                let landings = landings(&l, x);
                let spots: BTreeSet<Pos> = x.rooms.iter().flat_map(|r| r.spots.clone()).collect();
                let beside = l
                    .links
                    .iter()
                    .filter(|k| k.kind == LinkKind::Stair)
                    .filter_map(|k| k.stop(d.level))
                    .map(|s| s.at.offset(1, 0))
                    .find(|e| floor(x, *e) && !landings.contains(e) && !spots.contains(e));
                assert!(
                    beside.is_some(),
                    "seed {seed}: no stair with room beside it"
                );
                roofs += 1;
            }
        }
        assert!(roofs > 0);
    }

    #[test]
    fn landings_leave_no_spot_reserved() {
        let pack = VenuePack::builtin();
        let types = pack.types.iter().cloned().chain([test_type()]);
        let mut landed = 0;
        for t in types {
            for (seed, l) in layouts(&t) {
                for x in &l.levels {
                    let what = format!("{} seed {seed} level {}", t.id, x.level);
                    let reserved: BTreeSet<Pos> = x.reserved.iter().copied().collect();
                    // Each landing and the floor beside it are reserved.
                    for p in landings(&l, x) {
                        landed += 1;
                        assert!(reserved.contains(&p), "{what}: {p:?}");
                        for n in p.neighbors4().filter(|n| floor(x, *n)) {
                            assert!(reserved.contains(&n), "{what}: beside {p:?}");
                        }
                    }
                    for r in &x.rooms {
                        for s in &r.spots {
                            assert!(!reserved.contains(s), "{what}: the spot {s:?} is reserved");
                        }
                    }
                }
            }
        }
        assert!(landed > 0);
    }

    #[test]
    fn a_link_passes_a_level_with_no_room_left_in_its_zone() {
        let t = test_type();
        // The fire stair's zone shut on a middle floor: the links of that zone pass it,
        // and the others still stop there.
        let stair = t.links.iter().find(|d| d.alight == Alight::Ends).unwrap();
        let zone = t.zone(&stair.zone).unwrap();
        let level = 2;
        for seed in 0..10 {
            let plain = lay_out(&t, VenueFrame::own(&t), seed).unwrap();
            assert_eq!(
                crowded(&t, seed, level, |_| false).as_ref(),
                Ok(&plain),
                "seed {seed}: crowded lays as lay_out does"
            );
            let x = plain.level(level).unwrap();
            assert_eq!(Some(&x.role), t.levels.middle.as_ref().map(|m| &m.name));
            let l = crowded(&t, seed, level, |r| r.zone == zone)
                .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
            let mut passed = 0;
            for (k, p) in l.links.iter().zip(&plain.links) {
                let what = format!("seed {seed}: the {:?}", k.name);
                let levels = |k: &LevelLink| k.stops.iter().map(|s| s.level).collect::<Vec<_>>();
                let in_zone = p.stop(level).is_some_and(|s| {
                    x.rooms
                        .iter()
                        .any(|r| r.rect.contains(s.at) && r.zone == zone)
                });
                if in_zone {
                    passed += 1;
                    let want: Vec<i32> = levels(p).into_iter().filter(|l| *l != level).collect();
                    assert_eq!(levels(k), want, "{what}: passes {level}, stops elsewhere");
                } else {
                    assert_eq!(levels(k), levels(p), "{what}");
                }
                if k.name == Some(stair.name.clone()) {
                    let (first, last) = (k.stops[0], k.stops[k.stops.len() - 1]);
                    assert!(first.off && last.off, "{what}: its ends still let you off");
                }
            }
            assert!(passed >= 2, "seed {seed}: the stair and the duct pass");
            assert!(
                l.links.iter().any(|k| k.stop(level).is_some()),
                "seed {seed}: {level} is still reached"
            );
        }
    }

    #[test]
    fn what_cant_pass_a_crowded_level_is_an_error() {
        let t = test_type();
        let plain = lay_out(&t, VenueFrame::own(&t), 1).unwrap();
        let def = |kind| t.links.iter().find(|d| d.kind == kind).unwrap();
        let no_room = |def: &LinkDef, level: i32| {
            format!(
                "no room for the {} on {}",
                def.name,
                FloorPlan::label(level)
            )
        };
        // Every room of a middle floor shut: no link stops there, and the first that
        // passed it is named.
        let e = crowded(&t, 1, 2, |_| true).unwrap_err();
        assert!(e.contains(&no_room(&t.links[0], 2)), "{e}");
        // The ladder's lower level shut to its zone: it would have one stop.
        let ladder = def(LinkKind::Ladder);
        let (low, zone) = (served(ladder, &plain)[0], t.zone(&ladder.zone).unwrap());
        let e = crowded(&t, 1, low, |r| r.zone == zone).unwrap_err();
        assert!(e.contains(&no_room(ladder, low)), "{e}");
        // An escalator can't pass: its upper level shut to its zone.
        let up = def(LinkKind::Escalator);
        let high = *served(up, &plain).last().unwrap();
        let zone = t.zone(&up.zone).unwrap();
        let e = crowded(&t, 1, high, |r| r.zone == zone).unwrap_err();
        assert!(e.contains(&no_room(up, high)), "{e}");
        // Links that leave a level out entirely leave it unreached.
        let mut few = t.clone();
        few.links.retain(|d| d.kind == LinkKind::Escalator);
        let lowest = plain.levels[0].level;
        let e = lay_out(&few, VenueFrame::own(&t), 1).unwrap_err();
        assert!(
            e.contains(&format!("no link stops on {}", FloorPlan::label(lowest))),
            "{e}"
        );
    }
}
