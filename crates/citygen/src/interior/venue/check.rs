//! The venue checker: a layout's links, ways in and locks where they should be, every
//! room reached through its graph, and every level drawn and checked as a floor.

use super::{
    Keys, Means, NodeAt, SizeRule, VenueGraph, VenueLayout, VenueType, WayInKind, Ways, draw_level,
};
use crate::interior::floor::doorstep_of;
use crate::interior::{FloorPlan, validate_floor};
use crate::rng::hash64;
use crate::world::StratumKind;
use std::collections::BTreeSet;

/// Every means at once: someone holding every key.
const EVERY: [Means; 7] = [
    Means::Card,
    Means::Code,
    Means::Keyjack,
    Means::Net,
    Means::Force,
    Means::Talk,
    Means::Crawl,
];

impl VenueLayout {
    /// Doors a mission can fit a lock to: `fit` set, `lock` not.
    pub fn candidates(&self) -> Vec<(i32, usize)> {
        (self.levels.iter())
            .flat_map(|x| {
                (x.doors.iter().enumerate())
                    .filter(|(_, d)| d.fit.is_some() && d.lock.is_none())
                    .map(move |(k, _)| (x.level, k))
            })
            .collect()
    }

    /// Whether this layout can hold a mission of `size`: floors and ways in enough, and
    /// some room of one of the type's objectives (on its role's levels when it names one)
    /// at least the size's locks deep with every candidate door locked.
    pub fn holds(&self, t: &VenueType, rule: &SizeRule) -> bool {
        if self.levels.len() < usize::from(rule.floors.0)
            || self.ways_in.len() < usize::from(rule.ways_in)
        {
            return false;
        }
        let g = VenueGraph::of(self, &self.candidates(), None);
        let deep = |n: usize| g.depth(n).is_some_and(|d| d >= u32::from(rule.locks.0));
        t.objectives.iter().any(|o| {
            (self.levels.iter())
                .filter(|x| o.level.as_ref().is_none_or(|role| *role == x.role))
                .any(|x| {
                    (x.rooms.iter().enumerate())
                        .filter(|(_, r)| r.name == o.room)
                        .filter_map(|(i, _)| g.room(x.level, i))
                        .any(deep)
                })
        })
    }
}

/// A layout's own checks: every link stop lies in a room of its level, every way in's
/// doorstep (or grille) lies in its room, every door joins two rooms of its level and a
/// one-way lock opens free from one of them, each level's doors open onto one stratum at
/// most, its locks' numbers are its own, and every room is reached from the ways in by
/// someone holding every key.
pub fn validate_layout(layout: &VenueLayout) -> Result<(), String> {
    for link in &layout.links {
        let name = link.name.as_deref().unwrap_or("link");
        for s in &link.stops {
            let on = FloorPlan::label(s.level);
            let x = (layout.level(s.level))
                .ok_or_else(|| format!("the {name} stops on {on}, which it doesn't have"))?;
            let tiles = link.tiles(s.level);
            if !(x.rooms.iter()).any(|r| tiles.iter().all(|p| r.rect.contains(*p))) {
                return Err(format!(
                    "{on}: the {name}'s stop at {:?} lies in no room",
                    s.at
                ));
            }
        }
    }
    for w in &layout.ways_in {
        let on = FloorPlan::label(w.level);
        let x = (layout.level(w.level))
            .ok_or_else(|| format!("a way in comes in on {on}, which it doesn't have"))?;
        let room = (x.rooms.get(w.room))
            .ok_or_else(|| format!("{on}: the way in at {:?} names no room", w.at))?;
        let (what, at) = match w.kind {
            WayInKind::Door { .. } => ("doorstep", doorstep_of(x.size, w.at)),
            WayInKind::Crawl { .. } => ("grille", w.at),
        };
        if !room.rect.contains(at) {
            return Err(format!(
                "{on}: the way in at {:?} has its {what} at {at:?} outside its {}",
                w.at, room.name
            ));
        }
    }
    for x in &layout.levels {
        let on = FloorPlan::label(x.level);
        for d in &x.doors {
            let (Some(a), Some(b)) = (x.rooms.get(d.rooms.0), x.rooms.get(d.rooms.1)) else {
                return Err(format!("{on}: the door at {:?} joins no room", d.at));
            };
            let free = d.lock.or(d.fit).and_then(|l| l.free_from);
            if let Some(p) = free
                && !a.rect.contains(p)
                && !b.rect.contains(p)
            {
                return Err(format!(
                    "{on}: the one-way lock at {:?} opens free from {p:?}, in neither of its rooms",
                    d.at
                ));
            }
        }
        let strata: BTreeSet<StratumKind> = x.exits.iter().map(|e| e.stratum).collect();
        if strata.len() > 1 {
            return Err(format!(
                "{on}: its doors open onto {} strata: {strata:?}",
                strata.len()
            ));
        }
        let ways = (layout.ways_in.iter())
            .filter(|w| w.level == x.level)
            .filter_map(|w| w.lock);
        let mut numbers = BTreeSet::new();
        for l in x.doors.iter().filter_map(|d| d.fit.or(d.lock)).chain(ways) {
            if !numbers.insert(l.number) {
                return Err(format!(
                    "{on}: two locks take the number {}, one at {:?}",
                    l.number, l.at
                ));
            }
        }
    }
    let g = VenueGraph::of(layout, &[], None);
    let deepest = u8::try_from(layout.zones.len().saturating_sub(1)).unwrap_or(u8::MAX);
    let keys = Keys {
        card: deepest,
        talked: deepest,
        codes: (g.edges.iter())
            .filter_map(|e| e.lock.and_then(|l| l.code))
            .collect(),
    };
    let reached = g.reach(&g.starts, Ways::of(&EVERY), keys).nodes;
    for (n, at) in g.nodes.iter().enumerate() {
        if let NodeAt::Room { level, room } = *at
            && !reached[n]
        {
            let r = &layout.level(level).expect("a node's level").rooms[room];
            return Err(format!(
                "{}: nobody reaches the {} at {:?}",
                FloorPlan::label(level),
                r.name,
                r.rect
            ));
        }
    }
    Ok(())
}

/// The layout's checks and every level drawn and checked.
pub fn validate_venue(layout: &VenueLayout) -> Result<(), String> {
    validate_layout(layout)?;
    let plan = FloorPlan::of_layout(layout.clone());
    for x in &layout.levels {
        let on = FloorPlan::label(x.level);
        let floor = draw_level(layout, x.level).ok_or_else(|| format!("{on} draws no floor"))?;
        validate_floor(&plan, &floor).map_err(|e| format!("{on}: {e}"))?;
    }
    Ok(())
}

/// One number for a layout and its drawn levels (tiles, rooms, exits, stations, locks; no light).
pub fn venue_fingerprint(layout: &VenueLayout) -> u64 {
    use std::fmt::Write;
    let mut text = format!("{layout:?}\n");
    for x in &layout.levels {
        let Some(f) = draw_level(layout, x.level) else {
            continue;
        };
        for (_, t) in f.tiles.iter() {
            let _ = writeln!(text, "{:?} {:?}", t.terrain, t.feature);
        }
        for r in &f.rooms {
            let _ = writeln!(text, "{r:?}");
        }
        for e in &f.exits {
            let _ = writeln!(text, "{e:?}");
        }
        for s in &f.stations {
            let _ = writeln!(text, "{s:?}");
        }
        for k in &f.locks {
            let _ = writeln!(text, "{k:?}");
        }
    }
    text.as_bytes().chunks(8).fold(0, |h, chunk| {
        let mut word = [0u8; 8];
        word[..chunk.len()].copy_from_slice(chunk);
        hash64(h, u64::from_le_bytes(word))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Pos;
    use crate::interior::venue::testutil::test_type;
    use crate::interior::venue::{VenueFrame, VenuePack, WayInKind, lay_out};

    fn laid(seed: u64) -> VenueLayout {
        let t = test_type();
        lay_out(&t, VenueFrame::own(&t), seed).unwrap()
    }

    #[test]
    fn validate_layout_finds_a_room_nobody_reaches() {
        let mut l = laid(2);
        validate_layout(&l).unwrap();
        let lv = l.levels.iter_mut().find(|x| x.rooms.len() > 3).unwrap();
        let cut = lv.doors.iter().position(|d| !d.border).unwrap();
        let lonely = lv.doors[cut].rooms.1;
        lv.doors
            .retain(|d| d.rooms.0 != lonely && d.rooms.1 != lonely);
        assert!(validate_layout(&l).unwrap_err().contains("reach"));
    }

    #[test]
    fn validate_layout_finds_stops_ways_in_strata_and_numbers_astray() {
        let l = laid(2);
        let bad = |f: &dyn Fn(&mut VenueLayout)| {
            let mut m = l.clone();
            f(&mut m);
            validate_layout(&m).unwrap_err()
        };
        // A stop on the frame wall stands in no room.
        let err = bad(&|m| m.links[0].stops[0].at = Pos::new(0, 0));
        assert!(err.contains("stop"), "{err}");
        // A door's way in into a room that doesn't hold its doorstep.
        let err = bad(&|m| {
            let w = (m.ways_in.iter_mut())
                .find(|w| matches!(w.kind, WayInKind::Door { .. }))
                .unwrap();
            let x = m.levels.iter().find(|x| x.level == w.level).unwrap();
            w.room = (w.room + 1) % x.rooms.len();
        });
        assert!(err.contains("doorstep"), "{err}");
        // A level whose doors open onto two strata.
        let err = bad(&|m| {
            let x = m.levels.iter_mut().find(|x| x.exits.len() > 1).unwrap();
            let other = x.exits[0].stratum;
            x.exits[1].stratum = if other == crate::world::StratumKind::Street {
                crate::world::StratumKind::Skyways
            } else {
                crate::world::StratumKind::Street
            };
        });
        assert!(err.contains("strata"), "{err}");
        // Two locks on a level with one number.
        let err = bad(&|m| {
            let x = (m.levels.iter_mut())
                .find(|x| x.doors.iter().filter(|d| d.fit.is_some()).count() > 1)
                .unwrap();
            let mut fits = x.doors.iter_mut().filter_map(|d| d.fit.as_mut());
            let first = fits.next().unwrap().number;
            fits.next().unwrap().number = first;
        });
        assert!(err.contains("number"), "{err}");
        // A way in's lock numbers with the level's own.
        let err = bad(&|m| {
            let w = m.ways_in.iter_mut().find(|w| w.lock.is_some()).unwrap();
            let x = m.levels.iter().find(|x| x.level == w.level).unwrap();
            let taken = x.doors.iter().find_map(|d| d.fit).unwrap().number;
            w.lock.as_mut().unwrap().number = taken;
        });
        assert!(err.contains("number"), "{err}");
        // A door joining a room its level doesn't have.
        let err = bad(&|m| m.levels[0].doors[0].rooms.1 = 999);
        assert!(err.contains("joins no room"), "{err}");
        // A one-way lock free from a tile in neither of its rooms.
        let err = bad(&|m| {
            let d = (m.levels.iter_mut().flat_map(|x| x.doors.iter_mut()))
                .find(|d| d.fit.is_some_and(|f| f.free_from.is_some()))
                .unwrap();
            for l in d.fit.iter_mut().chain(d.lock.iter_mut()) {
                l.free_from = Some(Pos::new(0, 0));
            }
        });
        assert!(err.contains("one-way"), "{err}");
    }

    #[test]
    fn candidates_are_the_doors_with_a_fit_and_no_lock() {
        let l = laid(0);
        let c = l.candidates();
        assert!(!c.is_empty());
        for x in &l.levels {
            for (k, d) in x.doors.iter().enumerate() {
                let want = d.fit.is_some() && d.lock.is_none();
                assert_eq!(
                    c.contains(&(x.level, k)),
                    want,
                    "level {} door {k}",
                    x.level
                );
            }
        }
    }

    #[test]
    fn holds_wants_its_floors_ways_in_and_depth() {
        let t = test_type();
        let l = laid(0);
        let small = VenuePack::builtin().size(crate::interior::venue::MissionSize::Small);
        assert!(l.holds(&t, small));
        let mut more = small.clone();
        more.floors.0 = l.levels.len() as u8 + 1;
        assert!(!l.holds(&t, &more), "too few floors");
        let mut more = small.clone();
        more.ways_in = l.ways_in.len() as u8 + 1;
        assert!(!l.holds(&t, &more), "too few ways in");
        let mut more = small.clone();
        more.locks.0 = 30;
        assert!(!l.holds(&t, &more), "too shallow");
        let mut none = t.clone();
        none.objectives.clear();
        assert!(!l.holds(&none, small), "no objective");
    }

    #[test]
    fn a_venue_validates_every_level_drawn() {
        for seed in 0..5 {
            validate_venue(&laid(seed)).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        }
        let mut l = laid(0);
        // A link landing that the floor can't stand on: the frame wall.
        let x = l.levels[0].level;
        let k = l.links.iter_mut().find(|k| k.stop(x).is_some()).unwrap();
        let s = k.stops.iter_mut().find(|s| s.level == x).unwrap();
        s.at = Pos::new(0, 1);
        assert!(validate_venue(&l).is_err());
    }

    #[test]
    fn a_fingerprint_is_its_layout() {
        let (a, b) = (laid(0), laid(1));
        assert_eq!(venue_fingerprint(&a), venue_fingerprint(&a.clone()));
        assert_ne!(venue_fingerprint(&a), venue_fingerprint(&b));
    }
}
