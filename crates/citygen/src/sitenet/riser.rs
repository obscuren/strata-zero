//! The riser a site's Net runs up, its shaft on each level, each level's hub beside it and
//! the server beside G's hub.

use crate::geom::Pos;
use crate::interior::venue::{LevelLayout, free, held};
use crate::interior::{Floor, FloorPlan, LinkKind};
use crate::tile::Terrain;
use std::cmp::Reverse;
use std::collections::BTreeSet;

/// The link the riser follows, by index into the plan's links: the lift that stops on the
/// most of the plan's levels, else the link of any kind that does; the first in link order
/// on a tie. None with no link stopping on any of them.
pub(crate) fn riser_link(plan: &FloorPlan) -> Option<usize> {
    let serves = |i: usize| {
        (plan.links[i].stops.iter())
            .filter(|s| plan.has_level(s.level))
            .count()
    };
    let most = |lifts: bool| {
        (0..plan.links.len())
            .filter(|&i| !lifts || plan.links[i].kind == LinkKind::Lift)
            .filter(|&i| serves(i) > 0)
            .max_by_key(|&i| (serves(i), Reverse(i)))
    };
    most(true).or_else(|| most(false))
}

/// The riser's shaft on `level`: its landing there (both tiles of a lift's bank, one of
/// anything else), else its landing on the nearest level it stops at, the lower on a tie.
pub(crate) fn shaft(plan: &FloorPlan, link: usize, level: i32) -> Vec<Pos> {
    let k = &plan.links[link];
    let Some(stop) = (k.stops.iter()).min_by_key(|s| ((s.level - level).abs(), s.level)) else {
        return Vec::new();
    };
    if k.kind == LinkKind::Lift {
        vec![stop.at, stop.at.offset(1, 0)]
    } else {
        vec![stop.at]
    }
}

/// The tiles a hub or the server may stand on: free floor by the devices' rule (no spot,
/// door or doorstep, station or reserved tile), and none of `taken` (the devices' fronts,
/// the hub), so no two nodes share a tile on the Net. The floor beside a landing is
/// reserved to keep the way on and off it clear; a hub or the server stands there all the
/// same, as each stands on the Net and leaves the floor as it is.
struct Ground<'a> {
    floor: &'a Floor,
    unfree: BTreeSet<Pos>,
}

impl<'a> Ground<'a> {
    fn new(floor: &'a Floor, x: &LevelLayout, taken: &BTreeSet<Pos>) -> Ground<'a> {
        let landing = |p: Pos| {
            floor.tiles.get(p).is_some_and(|t| {
                matches!(
                    t.terrain,
                    Terrain::Lift | Terrain::Stair | Terrain::Escalator | Terrain::Ladder
                )
            })
        };
        let mut unfree = held(x, floor);
        unfree.extend((x.reserved.iter().copied()).filter(|p| !p.neighbors4().any(landing)));
        unfree.extend(floor.exits.iter().map(|e| floor.doorstep(e)));
        unfree.extend(taken);
        Ground { floor, unfree }
    }

    fn free(&self, p: Pos) -> bool {
        free(self.floor, &self.unfree, p)
    }

    /// The free tile nearest `to` (manhattan, then y, x).
    fn nearest(&self, to: Pos) -> Option<Pos> {
        (self.floor.tiles.iter())
            .map(|(p, _)| p)
            .filter(|p| self.free(*p))
            .min_by_key(|p| (p.manhattan(to), p.y, p.x))
    }
}

/// A level's hub beside the riser's shaft `bank`: the first free tile of west of the bank,
/// east of its last tile, under it, over it; else the free tile nearest the shaft. With no
/// riser, `bank` is the one tile the hub stands nearest. `taken` are tiles of other nodes.
pub(crate) fn hub_tile(floor: &Floor, x: &LevelLayout, bank: &[Pos], taken: &BTreeSet<Pos>) -> Pos {
    let ground = Ground::new(floor, x, taken);
    let (Some(&first), Some(&last)) = (bank.first(), bank.last()) else {
        return floor.tiles.size().center();
    };
    let mut beside = [first.offset(-1, 0), last.offset(1, 0)]
        .into_iter()
        .chain(bank.iter().map(|p| p.offset(0, 1)))
        .chain(bank.iter().map(|p| p.offset(0, -1)));
    (beside.find(|p| ground.free(*p)))
        .or_else(|| ground.nearest(first))
        .unwrap_or(first)
}

/// The server beside G's hub: the first free tile 4-beside it, then 8-beside, in (y, x)
/// order; else the free tile nearest it. `taken` are tiles of other nodes, the hub's too.
pub(crate) fn server_tile(floor: &Floor, x: &LevelLayout, hub: Pos, taken: &BTreeSet<Pos>) -> Pos {
    let mut taken = taken.clone();
    taken.insert(hub);
    let ground = Ground::new(floor, x, &taken);
    let sorted = |v: Vec<Pos>| {
        let mut v = v;
        v.sort_by_key(|p| (p.y, p.x));
        v
    };
    let four = sorted(hub.neighbors4().collect());
    let eight = sorted(hub.neighbors8().collect());
    (four.into_iter().chain(eight).find(|p| ground.free(*p)))
        .or_else(|| ground.nearest(hub))
        .unwrap_or(hub)
}
