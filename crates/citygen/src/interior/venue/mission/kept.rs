//! The levels a copy keeps, and whether its links join them.

use super::super::graph::rides;
use super::super::{SizeRule, VenueLayout};
use crate::interior::{FloorPlan, LinkKind};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Each level, and the levels a link carries you to from it: a lift, stair, escalator or
/// ladder you get on at a stop on the one and off at a stop on the other. A crawl joins
/// none, since only a crawler takes it.
fn level_links(layout: &VenueLayout) -> BTreeMap<i32, BTreeSet<i32>> {
    let mut to: BTreeMap<i32, BTreeSet<i32>> = BTreeMap::new();
    for k in layout.links.iter().filter(|k| k.kind != LinkKind::Crawl) {
        for i in 0..k.stops.len() {
            for j in 0..k.stops.len() {
                if rides(&k.stops, i, j) {
                    to.entry(k.stops[i].level)
                        .or_default()
                        .insert(k.stops[j].level);
                }
            }
        }
    }
    to
}

/// The levels on the fewest rides from any of `from` to a level `end` takes, that one
/// first and the one it set out from last; breadth first, the lower level first among
/// equals. None when no run of rides gets there.
fn path_from(
    links: &BTreeMap<i32, BTreeSet<i32>>,
    from: &BTreeSet<i32>,
    end: impl Fn(i32) -> bool,
) -> Option<Vec<i32>> {
    let mut parent: BTreeMap<i32, Option<i32>> = from.iter().map(|&l| (l, None)).collect();
    let mut queue: VecDeque<i32> = from.iter().copied().collect();
    while let Some(l) = queue.pop_front() {
        if end(l) {
            let mut path = vec![l];
            let mut at = l;
            while let Some(Some(p)) = parent.get(&at) {
                path.push(*p);
                at = *p;
            }
            return Some(path);
        }
        for &m in links.get(&l).into_iter().flatten() {
            if let std::collections::btree_map::Entry::Vacant(e) = parent.entry(m) {
                e.insert(Some(l));
                queue.push_back(m);
            }
        }
    }
    None
}

/// The levels of `kept` that rides between kept levels carry you to from G, and those
/// that carry you back to G.
fn from_and_to_g(
    links: &BTreeMap<i32, BTreeSet<i32>>,
    kept: &BTreeSet<i32>,
) -> (BTreeSet<i32>, BTreeSet<i32>) {
    let spread = |step: &dyn Fn(i32, i32) -> bool| {
        let mut seen = BTreeSet::from([0]);
        let mut queue = VecDeque::from([0]);
        while let Some(l) = queue.pop_front() {
            for &m in kept {
                if step(l, m) && seen.insert(m) {
                    queue.push_back(m);
                }
            }
        }
        seen
    };
    let rides = |a: i32, b: i32| links.get(&a).is_some_and(|t| t.contains(&b));
    (spread(&|l, m| rides(l, m)), spread(&|l, m| rides(m, l)))
}

/// Whether rides between the `kept` levels carry you from G to each of them and back.
pub(in crate::interior::venue) fn joined(layout: &VenueLayout, kept: &[i32]) -> bool {
    let kept: BTreeSet<i32> = kept.iter().copied().collect();
    if !kept.contains(&0) {
        return false;
    }
    let (from_g, to_g) = from_and_to_g(&level_links(layout), &kept);
    kept.iter().all(|k| from_g.contains(k) && to_g.contains(k))
}

/// G, the objective's level and the levels on the fewest rides from G to it and from it
/// back to G, lowest first.
pub(in crate::interior::venue) fn must_levels(
    layout: &VenueLayout,
    objective: i32,
) -> Result<Vec<i32>, String> {
    let links = level_links(layout);
    let none = || {
        format!(
            "no lift or stair runs between G and {}",
            FloorPlan::label(objective)
        )
    };
    let there = path_from(&links, &BTreeSet::from([0]), |l| l == objective).ok_or_else(none)?;
    let back = path_from(&links, &BTreeSet::from([objective]), |l| l == 0).ok_or_else(none)?;
    let mut must: Vec<i32> = there.into_iter().chain(back).collect();
    must.sort_unstable();
    must.dedup();
    Ok(must)
}

/// The levels a copy keeps: G, the objective's level and those on the fewest rides there
/// and back; then, up to a count the rng draws in the size's range, the other levels with
/// a door to the city, nearest G first, then the level beside a kept one nearest the
/// objective's; then more door levels, and levels a crawl comes in on, until the ways in
/// are enough; then any level rides need to carry you from G to each kept level and back.
pub(super) fn keep(
    layout: &VenueLayout,
    rule: &SizeRule,
    objective: i32,
    rng: &mut ChaCha8Rng,
) -> Result<Vec<i32>, String> {
    let size = rule.size;
    let most = usize::from(rule.floors.1);
    let mut kept: BTreeSet<i32> = must_levels(layout, objective)?.into_iter().collect();
    if kept.len() > most {
        return Err(format!(
            "a {size:?} copy keeps {most} floors, and its objective needs {}",
            kept.len()
        ));
    }
    let by_g = |mut levels: Vec<i32>| {
        levels.sort_by_key(|&l| (l.abs(), l));
        levels
    };
    let doors = by_g(
        (layout.levels.iter())
            .filter(|x| !x.exits.is_empty())
            .map(|x| x.level)
            .collect(),
    );
    let least = rule.floors.0;
    let n = usize::from(rng.random_range(least..=rule.floors.1.max(least)));
    let n = n.min(layout.levels.len());
    while kept.len() < n {
        let beside = (layout.levels.iter())
            .map(|x| x.level)
            .filter(|l| !kept.contains(l))
            .filter(|l| kept.contains(&(l - 1)) || kept.contains(&(l + 1)))
            .min_by_key(|&l| ((l - objective).abs(), l));
        let Some(next) = doors.iter().copied().find(|d| !kept.contains(d)).or(beside) else {
            break;
        };
        kept.insert(next);
    }
    let ways = |kept: &BTreeSet<i32>| {
        (layout.ways_in.iter())
            .filter(|w| kept.contains(&w.level))
            .count()
    };
    let crawls = by_g((layout.ways_in.iter()).map(|w| w.level).collect());
    while ways(&kept) < usize::from(rule.ways_in) && kept.len() < most {
        let Some(next) = (doors.iter().chain(&crawls))
            .copied()
            .find(|l| !kept.contains(l))
        else {
            break;
        };
        kept.insert(next);
    }
    join(layout, &mut kept)?;
    if kept.len() > most {
        return Err(format!(
            "a {size:?} copy keeps {most} floors, and joining them needs {}",
            kept.len()
        ));
    }
    if kept.len() < usize::from(rule.floors.0) {
        return Err(format!("too few floors for {size:?}"));
    }
    if ways(&kept) < usize::from(rule.ways_in) {
        return Err(format!("too few ways in for {size:?}"));
    }
    Ok(kept.into_iter().collect())
}

/// Keeps the levels on the fewest rides from those G gets to, to a kept level it doesn't,
/// and from a kept level that doesn't get back to G, to one that does, until every kept
/// level is got to from G and back. Each run passes a level not kept, so this ends.
fn join(layout: &VenueLayout, kept: &mut BTreeSet<i32>) -> Result<(), String> {
    let links = level_links(layout);
    loop {
        let (from_g, to_g) = from_and_to_g(&links, kept);
        let path = if let Some(&k) = kept.iter().find(|k| !from_g.contains(k)) {
            path_from(&links, &from_g, |l| l == k)
        } else if let Some(&k) = kept.iter().find(|k| !to_g.contains(k)) {
            path_from(&links, &BTreeSet::from([k]), |l| to_g.contains(&l))
        } else {
            return Ok(());
        };
        kept.extend(path.ok_or("no lift or stair joins the kept floors")?);
    }
}
