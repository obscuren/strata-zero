//! The ways-in stage: the frame's doors and crawls as the layout's ways in.
//!
//! A frame door on a level the layout has is that level's exit, coming into the room that
//! holds its doorstep; one on a level it lacks is barred. A door into a zone above public,
//! but the main street door (the frame's first) and a door whose outside has no other way
//! off (`FrameDoor::enclosed`), is locked, always, with a keypad: past it go the ways of
//! the border into that zone, a code and the Keyjack, so every locked door from the city
//! takes the pad. It is one-way: a way out without a key. The roof door stands on the
//! floor east of a landing, its doorstep: east of a stair that kept the tile free, else
//! east of another landing, else on the free floor nearest the level's first landing
//! with free floor of its room west of it. Each of the type's crawls in from a stratum
//! the frame has a crawl from comes out on that crawl's level, when the layout has it, on
//! a grille: the free floor nearest where the crawl reaches the level, in a room of its
//! zone that anyone may enter (on a level without the zone, the next deeper zone it has,
//! as a link's stop). A frame that doesn't reach a stratum has no door or crawl from it,
//! so nothing comes in from there.
//!
//! A way in's tile is floor of a room, never a wall, a door or a doorless join, so it
//! opens no way between rooms that the doors don't. The roof door and a grille are
//! reserved with the floor beside them.

use super::links::{reserve, zone_on};
use super::rooms::Taken;
use super::zones::inward;
use super::{LOCK_NUMBERS, LevelLayout, LockKind, VenueLayout, VenueType, WayIn, WayInKind, Ways};
use crate::geom::Pos;
use crate::interior::{DoorLock, Exit, FloorPlan, LinkKind};
use crate::world::StratumKind;
use rand_chacha::ChaCha8Rng;
use std::collections::BTreeSet;

/// The ways-in stage's rng, from the layout seed.
pub(super) const STAGE: u64 = 0x5_3A75;

/// Lays the ways in: an exit for each frame door on a level the layout has and the rest
/// barred, in the frame's order, then a grille for each of the type's crawls in, in its
/// order; a crawl whose level has no free floor in its zone has none. Errs, naming the
/// level, when a door's doorstep lies in no room or a roof has no floor for its door.
pub(super) fn lay(
    t: &VenueType,
    layout: &mut VenueLayout,
    _rng: &mut ChaCha8Rng,
) -> Result<(), String> {
    layout.ways_in.clear();
    layout.barred.clear();
    for x in &mut layout.levels {
        x.exits.clear();
    }
    let main = (layout.frame.doors.iter()).position(|d| d.stratum == StratumKind::Street);
    for (k, d) in layout.frame.doors.clone().into_iter().enumerate() {
        let Some(i) = layout.levels.iter().position(|x| x.level == d.level) else {
            layout.barred.push((d.stratum, d.door));
            continue;
        };
        let on = FloorPlan::label(d.level);
        let at = match d.at {
            Some(at) => at,
            None => roof_door(layout, i)
                .ok_or_else(|| format!("{}: no floor for the roof door on {on}", t.id))?,
        };
        let x = &mut layout.levels[i];
        let step = doorstep(x, at);
        let room = (x.rooms.iter())
            .position(|r| r.rect.contains(step))
            .ok_or_else(|| {
                format!(
                    "{}: the {:?} door at {at:?} on {on} steps into no room",
                    t.id, d.stratum
                )
            })?;
        if d.at.is_none() {
            reserve(x, &[at]);
        }
        let zone = x.rooms[room].zone;
        let lock = if zone > 0 && Some(k) != main && !d.enclosed {
            Some(door_lock(t, x, &layout.ways_in, zone, at, step)?)
        } else {
            None
        };
        x.exits.push(Exit {
            at,
            stratum: d.stratum,
            landing: d.landing,
            door: d.door,
        });
        layout.ways_in.push(WayIn {
            level: d.level,
            room,
            at,
            kind: WayInKind::Door {
                stratum: d.stratum,
                door: d.door,
            },
            lock,
        });
    }
    for c in &t.crawls_in {
        let crawl = layout.frame.crawls.iter().find(|f| f.stratum == c.stratum);
        let Some(f) = crawl.copied() else {
            continue;
        };
        let Some(i) = layout.levels.iter().position(|x| x.level == f.level) else {
            continue;
        };
        let want = t.zone(&c.zone).ok_or_else(|| {
            format!(
                "{}: the crawl in from {:?} names no zone `{}`",
                t.id, c.stratum, c.zone
            )
        })?;
        let zone = zone_on(&layout.levels[i], want);
        let Some((room, at)) = grille(layout, i, zone, f.at) else {
            continue;
        };
        reserve(&mut layout.levels[i], &[at]);
        layout.ways_in.push(WayIn {
            level: f.level,
            room,
            at,
            kind: WayInKind::Crawl {
                stratum: c.stratum,
                outside: f.outside,
            },
            lock: None,
        });
    }
    Ok(())
}

/// The lock on a door at `at` on `x` into `zone`, above public: a keypad, past which go
/// the ways of the type's border into the zone from the next shallower zone with one, a
/// code and the Keyjack; one-way, opening from the doorstep `step` without a key; numbered
/// on from the level's zone locks and the locks of the doors laid before it (`ways`).
fn door_lock(
    t: &VenueType,
    x: &LevelLayout,
    ways: &[WayIn],
    zone: u8,
    at: Pos,
    step: Pos,
) -> Result<DoorLock, String> {
    let name = &t.zones[usize::from(zone)];
    let b = (0..zone)
        .rev()
        .find_map(|z| t.border(z, zone))
        .ok_or_else(|| format!("{}: no border into `{name}`", t.id))?;
    let before = (ways.iter())
        .filter(|w| w.level == x.level)
        .filter_map(|w| w.lock);
    let last = (x.doors.iter().filter_map(|d| d.fit))
        .chain(before)
        .map(|l| l.number)
        .max();
    Ok(DoorLock {
        at,
        number: last.map_or(LOCK_NUMBERS, |n| n + 1),
        zone,
        kind: LockKind::Keypad,
        ways: Ways::of(&b.ways) | Ways::CODE | Ways::KEYJACK,
        free_from: Some(step),
    })
}

/// The floor tile an exit at `at` steps into: in from the frame wall, or west of a door
/// on the floor, as `Floor::doorstep` reads it.
fn doorstep(x: &LevelLayout, at: Pos) -> Pos {
    let inner = x.size.rect().inset(1);
    if inner.contains(at) {
        at.offset(-1, 0)
    } else {
        inward(inner, at)
    }
}

/// The floor of a level a way in may stand on: a room's, off the core, the pieces, the
/// doors, the landings, the spots, and the exits and their doorsteps.
struct Open<'a> {
    x: &'a LevelLayout,
    taken: BTreeSet<Pos>,
    /// The doors and the doorsteps in from the frame wall, and the tiles beside them.
    by_doors: BTreeSet<Pos>,
}

impl<'a> Open<'a> {
    /// Level `i` of `layout`, its links laid and its exits so far.
    fn of(layout: &'a VenueLayout, i: usize) -> Open<'a> {
        let x = &layout.levels[i];
        let inner = x.size.rect().inset(1);
        let steps: Vec<Pos> = (layout.frame.doors.iter())
            .filter(|d| d.level == x.level)
            .filter_map(|d| d.at)
            .map(|at| inward(inner, at))
            .collect();
        let doors = x.doors.iter().map(|d| d.at);
        let taken = (x.exits.iter().map(|e| e.at))
            .chain(steps.iter().copied())
            .chain(doors.clone())
            .chain(x.rooms.iter().flat_map(|r| r.spots.iter().copied()))
            .chain(layout.links.iter().flat_map(|k| k.tiles(x.level)))
            .collect();
        let by_doors = (steps.into_iter().chain(doors))
            .flat_map(|p| std::iter::once(p).chain(p.neighbors4()))
            .collect();
        Open { x, taken, by_doors }
    }

    fn free(&self, p: Pos) -> bool {
        let x = self.x;
        x.rooms.iter().any(|r| r.rect.contains(p))
            && !x.core.is_some_and(|c| c.contains(p))
            && !x.pieces.iter().any(|q| q.rect.contains(p))
            && !self.taken.contains(&p)
    }
}

/// Where the roof door on level `i` stands: on open floor east of a stair's landing, else
/// east of another landing, in the links' order; else on the open floor nearest the
/// level's first landing (or its middle) with open floor of its room west of it.
fn roof_door(layout: &VenueLayout, i: usize) -> Option<Pos> {
    let x = &layout.levels[i];
    let open = Open::of(layout, i);
    let stairs = layout.links.iter().filter(|k| k.kind == LinkKind::Stair);
    let others = layout.links.iter().filter(|k| k.kind != LinkKind::Stair);
    let beside = (stairs.chain(others))
        .flat_map(|k| k.tiles(x.level))
        .map(|p| p.offset(1, 0))
        .find(|p| open.free(*p));
    beside.or_else(|| {
        let first = layout.links.iter().flat_map(|k| k.tiles(x.level)).next();
        let from = first.unwrap_or(x.size.rect().inset(1).center());
        (x.rooms.iter())
            .flat_map(|r| (r.rect.iter()).filter(move |p| r.rect.contains(p.offset(-1, 0))))
            .filter(|p| open.free(*p) && open.free(p.offset(-1, 0)))
            .min_by_key(|p| (p.manhattan(from), p.y, p.x))
    })
}

/// Where a crawl in comes out on level `i`, and its room: the open floor nearest `near` of
/// a room of `zone` that anyone may enter, never beside a door or a doorstep, and clear of
/// the reserved tiles where the zone has such floor (else beside a landing or the roof
/// door).
fn grille(layout: &VenueLayout, i: usize, zone: u8, near: Pos) -> Option<(usize, Pos)> {
    let x = &layout.levels[i];
    let (open, clear) = (Open::of(layout, i), Taken::of(x));
    let floor = || {
        (x.rooms.iter().enumerate())
            .filter(|(_, r)| r.zone == zone && !r.private)
            .flat_map(|(i, r)| r.rect.iter().map(move |p| (i, p)))
            .filter(|(_, p)| open.free(*p) && !open.by_doors.contains(p))
    };
    let nearest = |(_, p): &(usize, Pos)| (p.manhattan(near), p.y, p.x);
    (floor().filter(|(_, p)| clear.free(*p)).min_by_key(nearest))
        .or_else(|| floor().min_by_key(nearest))
}

#[cfg(test)]
mod tests {
    use super::LockKind;
    use crate::geom::Pos;
    use crate::interior::venue::building_seed;
    use crate::interior::venue::testutil::{cities, crowded, medium, test_type};
    use crate::interior::venue::{
        LOCK_NUMBERS, LevelLayout, Serves, VenueFrame, VenueLayout, VenuePack, VenueType, WayIn,
        WayInKind, Ways, lay_out,
    };
    use crate::interior::{Exit, LinkKind};
    use crate::tile::Terrain;
    use crate::world::{BuildingKind, City, ConnectorKind, StratumKind};
    use std::collections::BTreeSet;

    /// Every builtin type and the test type over its own frame, seeds 0..5, and over the
    /// buildings of its kinds in the test cities; a type that builds no kind over every
    /// building of the `Hotel` and `Office` kinds of building.
    fn sweep() -> Vec<(String, VenueType, VenueLayout)> {
        let cities = cities();
        let mut v = Vec::new();
        let test = test_type();
        let types = VenuePack::builtin().types.iter().cloned().chain([test]);
        for t in types {
            let builds: Vec<BuildingKind> = if t.builds.is_empty() {
                vec![BuildingKind::Hotel, BuildingKind::Office]
            } else {
                t.builds.clone()
            };
            let buildings = cities.iter().flat_map(|c| {
                c.buildings
                    .iter()
                    .filter(|b| builds.contains(&b.kind))
                    .filter_map(|b| VenueFrame::of_building(c, b.id))
            });
            let own = (0..5).map(|seed| (VenueFrame::own(&t), seed));
            let real = buildings.enumerate().map(|(i, f)| (f, i as u64));
            for (f, seed) in own.chain(real) {
                let what = format!("{} in {:?} {:?}, seed {seed}", t.id, f.kind, f.footprint);
                let l = lay_out(&t, f, seed).unwrap_or_else(|e| panic!("{what}: {e}"));
                v.push((what, t.clone(), l));
            }
        }
        v
    }

    /// The tile an exit at `at` steps into: in from the frame wall, or west of a door on
    /// the floor, as the roof door stands east of its landing.
    fn doorstep(x: &LevelLayout, at: Pos) -> Pos {
        let (w, h) = (x.size.w as i32, x.size.h as i32);
        if at.x == 0 {
            at.offset(1, 0)
        } else if at.x == w - 1 {
            at.offset(-1, 0)
        } else if at.y == 0 {
            at.offset(0, 1)
        } else if at.y == h - 1 {
            at.offset(0, -1)
        } else {
            at.offset(-1, 0)
        }
    }

    /// Every landing tile on `x`.
    fn landings(l: &VenueLayout, x: &LevelLayout) -> BTreeSet<Pos> {
        l.links.iter().flat_map(|k| k.tiles(x.level)).collect()
    }

    /// Whether `p` is free floor of a room on `x` for a way in: in a room, out of the core,
    /// off the pieces, the doors, the landings, the spots and the exits' doorsteps.
    fn free_floor(l: &VenueLayout, x: &LevelLayout, p: Pos) -> bool {
        x.rooms.iter().any(|r| r.rect.contains(p))
            && !x.core.is_some_and(|c| c.contains(p))
            && !x.pieces.iter().any(|q| q.rect.contains(p))
            && !x.doors.iter().any(|d| d.at == p)
            && !landings(l, x).contains(&p)
            && !x.rooms.iter().any(|r| r.spots.contains(&p))
            && !x.exits.iter().any(|e| doorstep(x, e.at) == p)
    }

    /// The Heights exits of `l`, with their level.
    fn roof_doors(l: &VenueLayout) -> Vec<(&LevelLayout, Exit)> {
        l.levels
            .iter()
            .flat_map(|x| x.exits.iter().map(move |e| (x, *e)))
            .filter(|(_, e)| e.stratum == StratumKind::Heights)
            .collect()
    }

    /// The zone a crawl in asking for `want` comes out in on `x`: `want` when the level
    /// has it, else the next deeper zone it has, else its deepest.
    fn zone_on(x: &LevelLayout, want: u8) -> u8 {
        let has: BTreeSet<u8> = x.bands.iter().map(|b| b.zone).collect();
        if has.contains(&want) {
            want
        } else {
            let deeper = has.iter().find(|z| **z > want);
            *deeper.or(has.last()).unwrap()
        }
    }

    /// Each way in comes into its room: a door's doorstep, or the grille, lies in it.
    fn assert_rooms_hold(what: &str, l: &VenueLayout) {
        for w in &l.ways_in {
            let x = l.level(w.level).unwrap();
            let r = &x.rooms[w.room];
            let tile = match w.kind {
                WayInKind::Door { .. } => doorstep(x, w.at),
                WayInKind::Crawl { .. } => w.at,
            };
            assert!(r.rect.contains(tile), "{what}: {w:?} comes into {tile:?}");
        }
    }

    #[test]
    fn every_frame_door_is_an_exit_or_barred() {
        for (what, _, l) in sweep() {
            let mut exits = 0;
            for d in &l.frame.doors {
                let barred = l.barred.contains(&(d.stratum, d.door));
                let kind = WayInKind::Door {
                    stratum: d.stratum,
                    door: d.door,
                };
                let ways: Vec<&WayIn> = l.ways_in.iter().filter(|w| w.kind == kind).collect();
                let Some(x) = l.level(d.level) else {
                    assert!(barred, "{what}: {d:?} isn't barred");
                    assert!(ways.is_empty(), "{what}: {d:?} is barred and a way in");
                    continue;
                };
                exits += 1;
                assert!(!barred, "{what}: {d:?} is an exit and barred");
                let e: Vec<&Exit> = (x.exits.iter())
                    .filter(|e| e.stratum == d.stratum && e.door == d.door)
                    .collect();
                assert_eq!(e.len(), 1, "{what}: {d:?} has {} exits", e.len());
                let e = e[0];
                assert_eq!(e.landing, d.landing, "{what}: {d:?}");
                if let Some(at) = d.at {
                    assert_eq!(e.at, at, "{what}: {d:?}");
                }
                assert_eq!(ways.len(), 1, "{what}: {d:?} has {} ways in", ways.len());
                assert_eq!(
                    (ways[0].level, ways[0].at),
                    (d.level, e.at),
                    "{what}: {d:?}"
                );
            }
            let all: usize = l.levels.iter().map(|x| x.exits.len()).sum();
            assert_eq!(all, exits, "{what}: an exit for no frame door");
            assert_eq!(l.barred.len(), l.frame.doors.len() - exits, "{what}");
            assert_rooms_hold(&what, &l);
        }
    }

    #[test]
    fn the_roof_door_stands_east_of_a_stair() {
        let mut roofs = 0;
        for (what, _, l) in sweep() {
            for (x, e) in roof_doors(&l) {
                roofs += 1;
                let stairs = (l.links.iter())
                    .filter(|k| k.kind == LinkKind::Stair)
                    .filter_map(|k| k.stop(x.level));
                assert!(
                    stairs.map(|s| s.at.offset(1, 0)).any(|p| p == e.at),
                    "{what}: the roof door at {:?} stands east of no stair",
                    e.at
                );
                assert!(
                    free_floor(&l, x, e.at),
                    "{what}: the roof door on {:?}",
                    e.at
                );
                assert!(
                    x.reserved.contains(&e.at),
                    "{what}: {:?} isn't reserved",
                    e.at
                );
            }
        }
        assert!(roofs > 0);
    }

    #[test]
    fn a_roof_no_stair_serves_keeps_its_door_beside_a_landing() {
        let mut t = test_type();
        let top = VenueFrame::own(&t).storeys.1;
        let below: Vec<i32> = (-(t.levels.below.len() as i32)..top).collect();
        for d in t.links.iter_mut().filter(|d| d.kind == LinkKind::Stair) {
            d.serves = Serves::Levels(below.clone());
        }
        for seed in 0..10 {
            let what = format!("seed {seed}");
            let l = lay_out(&t, VenueFrame::own(&t), seed).unwrap_or_else(|e| panic!("{e}"));
            let roofs = roof_doors(&l);
            assert_eq!(roofs.len(), 1, "{what}: the roof door is gone");
            let (x, e) = roofs[0];
            assert_eq!(x.level, top);
            let landings = landings(&l, x);
            assert!(
                landings.contains(&e.at.offset(-1, 0)),
                "{what}: the roof door at {:?} stands east of no landing",
                e.at
            );
            assert!(
                free_floor(&l, x, e.at),
                "{what}: the roof door on {:?}",
                e.at
            );
            assert_rooms_hold(&what, &l);
        }
    }

    #[test]
    fn a_later_stair_keeps_the_roof_door_when_the_first_passes_the_roof() {
        let mut t = test_type();
        let first = t
            .links
            .iter()
            .position(|d| d.kind == LinkKind::Stair)
            .unwrap();
        let mut second = t.links[first].clone();
        second.name = format!("second {}", second.name);
        second.zone = t.zones.last().unwrap().clone();
        t.links.insert(first + 1, second.clone());
        let top = VenueFrame::own(&t).storeys.1;
        let zone = t.zone(&t.links[first].zone).unwrap();
        for seed in 0..10 {
            let what = format!("seed {seed}");
            let l = crowded(&t, seed, top, |r| r.zone == zone).unwrap_or_else(|e| panic!("{e}"));
            let stop = |name: &str| {
                let k = l.links.iter().find(|k| k.name.as_deref() == Some(name));
                k.and_then(|k| k.stop(top)).copied()
            };
            assert_eq!(
                stop(&t.links[first].name),
                None,
                "{what}: the first stair stops"
            );
            let s = stop(&second.name).unwrap_or_else(|| panic!("{what}: no second stair"));
            let roofs = roof_doors(&l);
            assert_eq!(roofs.len(), 1, "{what}");
            assert_eq!(roofs[0].1.at, s.at.offset(1, 0), "{what}");
        }
    }

    /// Whether `p` is a planned door on `x` or beside one; `open`: doorless joins count.
    fn by_door(x: &LevelLayout, p: Pos, open: bool) -> bool {
        (x.doors.iter())
            .filter(|d| open || !d.open)
            .any(|d| d.at == p || d.at.neighbors4().any(|n| n == p))
    }

    /// Whether a grille may stand on `p`: free floor, on no exit, off the doors and the
    /// tiles beside them, and off the tiles beside a doorstep in from the frame wall.
    fn grille_floor(l: &VenueLayout, x: &LevelLayout, p: Pos) -> bool {
        let inner = x.size.rect().inset(1);
        let steps = (x.exits.iter())
            .filter(|e| !inner.contains(e.at))
            .map(|e| doorstep(x, e.at));
        free_floor(l, x, p)
            && !x.exits.iter().any(|e| e.at == p)
            && !by_door(x, p, true)
            && !steps.into_iter().any(|s| s.neighbors4().any(|n| n == p))
    }

    /// Each of the type's crawls in from a stratum the frame has a crawl from comes in on
    /// that crawl's level when the layout has it, onto free floor of a room of its zone
    /// that anyone may enter, never beside a door, or not at all where the zone has no
    /// floor for it. The ways in it lays, and how many it leaves out.
    fn assert_crawls(what: &str, t: &VenueType, l: &VenueLayout) -> (usize, usize) {
        let crawls: Vec<&WayIn> = (l.ways_in.iter())
            .filter(|w| matches!(w.kind, WayInKind::Crawl { .. }))
            .collect();
        let (mut laid, mut none) = (0, 0);
        for c in &t.crawls_in {
            let frame = (l.frame.crawls.iter())
                .find(|f| f.stratum == c.stratum)
                .filter(|f| l.level(f.level).is_some());
            let ways: Vec<&&WayIn> = (crawls.iter())
                .filter(
                    |w| matches!(w.kind, WayInKind::Crawl { stratum, .. } if stratum == c.stratum),
                )
                .collect();
            let Some(f) = frame else {
                assert!(ways.is_empty(), "{what}: a crawl in from {:?}", c.stratum);
                continue;
            };
            let x = l.level(f.level).unwrap();
            let zone = zone_on(x, t.zone(&c.zone).unwrap());
            let Some(w) = ways.first() else {
                let floor = (x.rooms.iter())
                    .filter(|r| r.zone == zone && !r.private)
                    .flat_map(|r| r.rect.iter())
                    .find(|p| grille_floor(l, x, *p));
                assert_eq!(floor, None, "{what}: no crawl in from {:?}", c.stratum);
                none += 1;
                continue;
            };
            laid += 1;
            assert_eq!(ways.len(), 1, "{what}: crawls in from {:?}", c.stratum);
            let kind = WayInKind::Crawl {
                stratum: c.stratum,
                outside: f.outside,
            };
            assert_eq!((w.level, w.kind), (f.level, kind), "{what}");
            let r = &x.rooms[w.room];
            assert_eq!(r.zone, zone, "{what}: {w:?} in {}", r.name);
            assert!(!r.private, "{what}: {w:?} in {}", r.name);
            assert!(r.rect.contains(w.at), "{what}: {w:?} out of {}", r.name);
            assert!(free_floor(l, x, w.at), "{what}: the grille on {:?}", w.at);
            assert!(!x.exits.iter().any(|e| e.at == w.at), "{what}: {w:?}");
            assert!(!by_door(x, w.at, false), "{what}: {w:?} beside a door");
            let beside = w.at.neighbors4().filter(|p| free_floor(l, x, *p));
            for p in std::iter::once(w.at).chain(beside) {
                assert!(x.reserved.contains(&p), "{what}: {p:?} by the grille");
            }
        }
        assert_eq!(crawls.len(), laid, "{what}: {crawls:?}");
        assert_rooms_hold(what, l);
        (laid, none)
    }

    #[test]
    fn crawls_in_land_in_their_zone_on_a_stratum_level() {
        // Every crawl of the test type comes in over its own frame.
        let t = test_type();
        for seed in 0..10 {
            let l = lay_out(&t, VenueFrame::own(&t), seed).unwrap();
            let what = format!("seed {seed}");
            assert_eq!(
                assert_crawls(&what, &t, &l),
                (t.crawls_in.len(), 0),
                "{what}"
            );
        }
        // Every type over its own frame and the buildings of its kinds lays every crawl in;
        // one is left out only where a building of a kind the type doesn't build has no
        // floor for it in its zone.
        let mut laid = 0;
        for (what, t, l) in sweep() {
            let (a, none) = assert_crawls(&what, &t, &l);
            laid += a;
            let foreign = l.frame.building.is_some() && !t.builds.contains(&l.frame.kind);
            assert!(foreign || none == 0, "{what}: {none} crawls in left out");
        }
        assert!(laid > 0);
    }

    /// The zone a door way in comes into, and whether it is the frame's main street door
    /// (its first street door).
    fn door_zone(l: &VenueLayout, w: &WayIn) -> Option<(u8, bool)> {
        let WayInKind::Door { stratum, door } = w.kind else {
            return None;
        };
        let main = (l.frame.doors.iter()).find(|d| d.stratum == StratumKind::Street);
        let main = main.is_some_and(|d| (d.stratum, d.door) == (stratum, door));
        Some((l.level(w.level).unwrap().rooms[w.room].zone, main))
    }

    /// The lock a door into `zone` above public carries: a keypad at the door, past which
    /// go the ways of the border into the zone from the next shallower zone with one, a
    /// code and the Keyjack; one-way from its doorstep.
    fn assert_door_lock(what: &str, t: &VenueType, l: &VenueLayout, w: &WayIn, zone: u8) {
        let x = l.level(w.level).unwrap();
        let b = (0..zone)
            .rev()
            .find_map(|z| t.border(z, zone))
            .unwrap_or_else(|| panic!("{what}: no border into {zone}"));
        let lock = w
            .lock
            .unwrap_or_else(|| panic!("{what}: {w:?} into {zone} is open"));
        let ways = Ways::of(&b.ways) | Ways::CODE | Ways::KEYJACK;
        let want = (w.at, zone, LockKind::Keypad, ways);
        assert_eq!((lock.at, lock.zone, lock.kind, lock.ways), want, "{what}");
        assert_eq!(lock.free_from, Some(doorstep(x, w.at)), "{what}: {w:?}");
    }

    /// Whether the frame door a door way in comes through has no other way off outside it.
    fn enclosed(l: &VenueLayout, w: &WayIn) -> bool {
        let WayInKind::Door { stratum, door } = w.kind else {
            return false;
        };
        (l.frame.doors.iter()).any(|d| d.stratum == stratum && d.door == door && d.enclosed)
    }

    #[test]
    fn a_hotels_roof_door_into_a_deep_zone_is_locked_but_from_inside() {
        let t = VenuePack::builtin().for_kind(BuildingKind::Hotel).unwrap();
        for seed in 0..6 {
            let what = format!("seed {seed}");
            let l = lay_out(t, VenueFrame::own(t), seed).unwrap();
            let roof = (l.ways_in.iter())
                .find(|w| matches!(w.kind, WayInKind::Door { stratum, .. } if stratum == StratumKind::Heights))
                .unwrap_or_else(|| panic!("{what}: no roof door"));
            let (zone, _) = door_zone(&l, roof).unwrap();
            assert!(zone > 0, "{what}: the roof door comes into public");
            assert_door_lock(&what, t, &l, roof, zone);
        }
    }

    /// Whether the roof ground a walk joins from the roof door `door` (never through a door)
    /// reaches another door, a connector or the map's edge.
    fn a_roof_way_off(c: &City, door: Pos) -> bool {
        let k = StratumKind::Heights;
        let tiles = &c.stratum(k).unwrap().tiles;
        let (w, h) = (tiles.size().w as i32, tiles.size().h as i32);
        let mut seen = BTreeSet::from([door]);
        let mut todo = vec![door];
        while let Some(p) = todo.pop() {
            let Some(t) = tiles.get(p).filter(|t| t.is_walkable()) else {
                continue;
            };
            let ladder = (c.connectors_at(p)).any(|x| {
                x.kind != ConnectorKind::Jack && x.serves.len() > 1 && x.serves.contains(&k)
            });
            let edge = p.x == 0 || p.y == 0 || p.x == w - 1 || p.y == h - 1;
            if (p != door && t.terrain == Terrain::Door) || ladder || edge {
                return true;
            }
            todo.extend(p.neighbors4().filter(|n| seen.insert(*n)));
        }
        false
    }

    #[test]
    fn a_roof_door_with_no_other_way_off_its_roof_carries_no_lock() {
        let (mut alone, mut joined) = (0, 0);
        for c in cities() {
            for b in &c.buildings {
                let (Some(t), Some(f)) = (
                    VenuePack::builtin().for_kind(b.kind),
                    VenueFrame::of_building(&c, b.id),
                ) else {
                    continue;
                };
                let Some(d) = (f.doors.iter()).find(|d| d.stratum == StratumKind::Heights) else {
                    continue;
                };
                assert_eq!(d.enclosed, !a_roof_way_off(&c, d.door), "{:?}", b.id);
                let (door, shut_in) = (d.door, d.enclosed);
                let Ok(l) = lay_out(t, f, building_seed(c.seed, b.id)) else {
                    continue;
                };
                let roof = (l.ways_in.iter()).find(|w| {
                    matches!(w.kind, WayInKind::Door { stratum: StratumKind::Heights, door: x } if x == door)
                });
                let Some(w) = roof else { continue };
                if door_zone(&l, w).is_none_or(|(zone, _)| zone == 0) {
                    continue;
                }
                if shut_in {
                    alone += 1;
                    assert_eq!(w.lock, None, "{:?}: nobody reaches its roof", b.id);
                } else {
                    joined += 1;
                    assert!(w.lock.is_some(), "{:?}: its roof joins a way off", b.id);
                }
            }
        }
        assert!(alone > 0, "no roof with only its own door");
        assert!(joined > 0, "no roof that joins a way off");
        // A type's own frame stands in no city: its roof door keeps its lock.
        for t in &VenuePack::builtin().types {
            assert!(
                VenueFrame::own(t).doors.iter().all(|d| !d.enclosed),
                "{}",
                t.id
            );
        }
    }

    #[test]
    fn a_door_into_public_carries_no_lock() {
        let mut open = 0;
        for (what, _, l) in sweep() {
            for w in &l.ways_in {
                match door_zone(&l, w) {
                    Some((0, _)) => {
                        open += 1;
                        assert_eq!(w.lock, None, "{what}: {w:?}");
                    }
                    Some((_, true)) => assert_eq!(w.lock, None, "{what}: the main door {w:?}"),
                    None => assert_eq!(w.lock, None, "{what}: the crawl {w:?}"),
                    Some(_) => {}
                }
            }
        }
        assert!(open > 0);
    }

    #[test]
    fn every_other_door_into_a_deep_zone_carries_its_borders_lock() {
        let mut locked = 0;
        for (what, t, l) in sweep() {
            for w in &l.ways_in {
                if let Some((zone, false)) = door_zone(&l, w)
                    && zone > 0
                {
                    if enclosed(&l, w) {
                        assert_eq!(w.lock, None, "{what}: {w:?} has no outside to lock");
                        continue;
                    }
                    locked += 1;
                    assert_door_lock(&what, &t, &l, w, zone);
                }
            }
            // Its number follows the level's zone locks, one apart from every other.
            for x in &l.levels {
                let zones = x.doors.iter().filter_map(|d| d.fit).map(|f| f.number);
                let doors = (l.ways_in.iter())
                    .filter(|w| w.level == x.level)
                    .filter_map(|w| w.lock)
                    .map(|k| k.number);
                let mut numbers: Vec<u16> = zones.chain(doors).collect();
                let all = numbers.len();
                assert!(numbers.iter().all(|n| *n >= LOCK_NUMBERS), "{what}");
                numbers.sort();
                numbers.dedup();
                assert_eq!(numbers.len(), all, "{what}: level {}", x.level);
                if let Some(last) = numbers.last() {
                    assert_eq!(*last as usize, LOCK_NUMBERS as usize + all - 1, "{what}");
                }
            }
        }
        assert!(locked > 0);
    }

    #[test]
    fn a_type_with_no_upper_roles_bars_the_doors_above() {
        let c = medium(8);
        let frame = (c.buildings.iter())
            .filter(|b| b.kind == BuildingKind::Office)
            .filter_map(|b| VenueFrame::of_building(&c, b.id))
            .max_by_key(|f| (f.storeys.1, f.building.map(|b| b.index())))
            .unwrap();
        let above = |k: StratumKind| matches!(k, StratumKind::Skyways | StratumKind::Heights);
        assert!(frame.doors.iter().any(|d| above(d.stratum)));
        let mut low = 0;
        for t in (VenuePack::builtin().types.iter())
            .filter(|t| t.levels.middle.is_none() && t.levels.top.is_empty())
        {
            low += 1;
            let l = lay_out(t, frame.clone(), 0).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(l.levels.last().unwrap().level, 0, "{}", t.id);
            for d in frame.doors.iter().filter(|d| above(d.stratum)) {
                assert!(l.barred.contains(&(d.stratum, d.door)), "{}: {d:?}", t.id);
            }
            assert_rooms_hold(&t.id, &l);
        }
        assert!(low > 0);
    }

    #[test]
    fn a_frame_short_of_the_undercity_has_no_way_in_below() {
        let mut t = test_type();
        t.frame.undercity = false;
        let mut frames: Vec<(VenueType, VenueFrame)> = vec![(t.clone(), VenueFrame::own(&t))];
        for c in cities() {
            for b in &c.buildings {
                let (Some(t), Some(f)) = (
                    VenuePack::builtin().for_kind(b.kind),
                    VenueFrame::of_building(&c, b.id),
                ) else {
                    continue;
                };
                if f.storeys.0 == 0 && !t.levels.below.is_empty() {
                    frames.push((t.clone(), f));
                }
            }
        }
        assert!(frames.len() > 1, "no building short of the Undercity");
        for (t, f) in frames {
            let below = |k: StratumKind| k == StratumKind::Undercity;
            assert!(!f.doors.iter().any(|d| below(d.stratum)));
            assert!(!f.crawls.iter().any(|c| below(c.stratum)));
            for seed in 0..3 {
                let what = format!("{} in {:?} {:?}, seed {seed}", t.id, f.kind, f.footprint);
                let l = lay_out(&t, f.clone(), seed).unwrap_or_else(|e| panic!("{what}: {e}"));
                assert!(l.level(-1).is_some(), "{what}: no level below");
                assert!(l.levels.iter().all(|x| x.level >= 0 || x.exits.is_empty()));
                assert!(l.ways_in.iter().all(|w| w.level >= 0), "{what}");
                assert!(l.ways_in.iter().any(|w| w.level == 0), "{what}: no way in");
                assert_rooms_hold(&what, &l);
            }
        }
    }

    #[test]
    fn the_same_seed_lays_the_same_ways() {
        let t = test_type();
        let a = lay_out(&t, VenueFrame::own(&t), 4).unwrap();
        assert_eq!(a, lay_out(&t, VenueFrame::own(&t), 4).unwrap());
        assert!(!a.ways_in.is_empty());
    }
}
