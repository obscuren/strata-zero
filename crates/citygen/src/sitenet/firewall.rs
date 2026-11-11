//! The firewall's locks: how many a site has, where they stand on the city's Net map, and
//! where each stands inside, on G's outer wall.

use crate::geom::{Pos, Rect};
use crate::ids::BuildingId;
use crate::interior::Floor;
use crate::tile::{Feature, Terrain};
use crate::world::{City, StratumKind};
use std::collections::BTreeSet;

/// 1 under 100, 2 to 159, 3 to 229, 4 from 230.
pub fn lock_count(security: u8) -> usize {
    match security {
        0..=99 => 1,
        100..=159 => 2,
        160..=229 => 3,
        _ => 4,
    }
}

/// The locks of building `b` on the city's Net, the gate first. Worked out from the Net
/// map as it is: a city from an old save gets them too. Empty when `b` has no gate.
pub fn outside(city: &City, b: BuildingId, count: usize) -> Vec<Pos> {
    let fp = city.building(b).footprint;
    let Some(gate) = fp.border().find(|p| {
        city.tile(StratumKind::Net, *p)
            .is_some_and(|t| t.terrain == Terrain::NetGate && t.building == Some(b))
    }) else {
        return Vec::new();
    };
    let open = |p: Pos| {
        city.tile(StratumKind::Net, p).is_some_and(|t| {
            t.terrain == Terrain::NetGrid && !matches!(t.feature, Some(Feature::Node(_)))
        })
    };
    spread(fp, gate, open, count)
}

fn is_corner(fp: Rect, p: Pos) -> bool {
    (p.x == fp.x || p.x == fp.right() - 1) && (p.y == fp.y || p.y == fp.bottom() - 1)
}

/// The candidate farthest (Chebyshev) from the locks already placed; the first of `from`
/// on a tie.
fn farthest(from: &[Pos], placed: &[Pos]) -> Option<Pos> {
    let mut best: Option<(i32, Pos)> = None;
    for &c in from {
        let d = placed.iter().map(|p| p.chebyshev(c)).min().unwrap_or(0);
        if best.is_none_or(|(bd, _)| d > bd) {
            best = Some((d, c));
        }
    }
    best.map(|(_, p)| p)
}

/// Adds to `placed` from `pool`, farthest first, until `count` stand.
fn fill(pool: &mut Vec<Pos>, placed: &mut Vec<Pos>, count: usize) {
    while placed.len() < count {
        let Some(p) = farthest(pool, placed) else {
            break;
        };
        pool.retain(|q| *q != p);
        placed.push(p);
    }
}

/// `outside`'s rule on a footprint, its gate and which tiles are open grid: after the
/// gate, each lock stands on the open tile beside the edge farthest from the locks placed;
/// with none left, on the footprint's own border (corners spared) the same way.
pub(crate) fn spread(fp: Rect, gate: Pos, open: impl Fn(Pos) -> bool, count: usize) -> Vec<Pos> {
    let edge: Vec<Pos> = fp.border().filter(|p| !is_corner(fp, *p)).collect();
    let mut beside: Vec<Pos> = edge
        .iter()
        .flat_map(|p| p.neighbors4())
        .filter(|p| !fp.contains(*p) && open(*p))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    beside.sort_by_key(|p| (p.y, p.x));
    let mut placed = vec![gate];
    fill(&mut beside, &mut placed, count);
    let mut rest: Vec<Pos> = edge.into_iter().filter(|p| *p != gate).collect();
    rest.sort_by_key(|p| (p.y, p.x));
    fill(&mut rest, &mut placed, count);
    placed
}

/// Each lock's tile on G's outer wall, in `outside`'s order; the first is the gate inside.
/// A lock's tile has an open floor tile beside it, inside, and is no corner, door or exit.
pub fn inside(footprint: Rect, g: &Floor, outside: &[Pos]) -> Vec<Pos> {
    let (w, h) = (g.tiles.size().w as i32, g.tiles.size().h as i32);
    let fits = |p: Pos| {
        let frame = p.x == 0 || p.y == 0 || p.x == w - 1 || p.y == h - 1;
        let corner = (p.x == 0 || p.x == w - 1) && (p.y == 0 || p.y == h - 1);
        frame
            && !corner
            && g.tiles[p].terrain != Terrain::Door
            && !g.exits.iter().any(|e| e.at == p)
            && p.neighbors4().any(|q| {
                q.x > 0
                    && q.y > 0
                    && q.x < w - 1
                    && q.y < h - 1
                    && g.tiles[q].terrain == Terrain::Floor
                    && g.tiles[q].feature.is_none()
            })
    };
    // In (y, x) order, so the first of the least distance wins a tie.
    let mut cands: Vec<Pos> = (0..h)
        .flat_map(|y| (0..w).map(move |x| Pos::new(x, y)))
        .filter(|p| fits(*p))
        .collect();
    let mut taken = Vec::new();
    for o in outside {
        let want = Pos::new(
            (o.x - footprint.x) * w / footprint.w.max(1),
            (o.y - footprint.y) * h / footprint.h.max(1),
        );
        let Some(best) = cands.iter().copied().min_by_key(|p| p.manhattan(want)) else {
            break;
        };
        cands.retain(|q| *q != best);
        taken.push(best);
    }
    taken
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interior::FloorPlan;
    use crate::interior::venue::{VenuePack, draw_level, testutil};
    use crate::world::BuildingKind;

    #[test]
    fn the_lock_count_follows_security() {
        for (s, n) in [
            (0, 1),
            (99, 1),
            (100, 2),
            (159, 2),
            (160, 3),
            (229, 3),
            (230, 4),
            (255, 4),
        ] {
            assert_eq!(lock_count(s), n, "{s}");
        }
    }

    #[test]
    fn the_locks_stand_against_the_edge_the_gate_first() {
        let mut seen = 0;
        for c in testutil::cities() {
            for b in c.buildings.iter().filter(|b| b.kind.is_closed()) {
                let fp = b.footprint;
                let Some(gate) = fp.border().find(|p| {
                    c.tile(StratumKind::Net, *p)
                        .is_some_and(|t| t.terrain == Terrain::NetGate && t.building == Some(b.id))
                }) else {
                    continue;
                };
                seen += 1;
                for n in 1..=4 {
                    let spots = outside(&c, b.id, n);
                    assert_eq!(spots.len(), n);
                    assert_eq!(spots[0], gate);
                    assert_eq!(spots.iter().collect::<BTreeSet<_>>().len(), n, "distinct");
                    for p in &spots[1..] {
                        let edge = !fp.contains(*p) && fp.border().any(|q| q.manhattan(*p) == 1);
                        assert!(edge || fp.border().any(|q| q == *p), "{p:?}");
                    }
                    assert_eq!(outside(&c, b.id, n), spots);
                }
            }
        }
        assert!(seen > 0);
    }

    #[test]
    fn a_packed_building_still_gets_every_lock() {
        let fp = Rect::new(10, 10, 6, 4);
        let spots = spread(fp, Pos::new(12, 10), |_| false, 4);
        assert_eq!(spots.len(), 4);
        assert_eq!(spots.iter().collect::<BTreeSet<_>>().len(), 4);
        assert!(spots.iter().all(|p| fp.border().any(|q| q == *p)));
        assert!(
            spots
                .iter()
                .all(|p| ![(10, 10), (15, 10), (10, 13), (15, 13)].contains(&(p.x, p.y))),
            "no corner"
        );
    }

    #[test]
    fn the_locks_spread_round_the_building() {
        let fp = Rect::new(10, 10, 8, 6);
        let spots = spread(fp, Pos::new(13, 10), |p| !fp.contains(p), 2);
        assert!(
            spots[1].y > 15,
            "the second lock goes to the far side: {spots:?}"
        );
    }

    #[test]
    fn inside_each_lock_stands_on_gs_outer_wall() {
        let pack = VenuePack::builtin();
        let office = pack.for_kind(BuildingKind::Office).unwrap().id.clone();
        let mut seen = 0;
        for c in testutil::cities() {
            for b in c.buildings.iter().filter(|b| b.kind.is_closed()) {
                if pack.for_kind(b.kind).is_none_or(|t| t.id != office) {
                    continue;
                }
                let Some(plan) = FloorPlan::new(&c, b.id) else {
                    continue;
                };
                let Some(venue) = plan.venue.as_deref() else {
                    continue;
                };
                if outside(&c, b.id, 1).is_empty() {
                    continue;
                }
                seen += 1;
                let g = draw_level(venue, 0).unwrap();
                let (w, h) = (g.tiles.size().w as i32, g.tiles.size().h as i32);
                let out = outside(&c, b.id, 4);
                let ins = inside(plan.footprint, &g, &out);
                assert_eq!(ins.len(), 4);
                assert_eq!(ins.iter().collect::<BTreeSet<_>>().len(), 4);
                for p in &ins {
                    let frame = p.x == 0 || p.y == 0 || p.x == w - 1 || p.y == h - 1;
                    let corner = (p.x == 0 || p.x == w - 1) && (p.y == 0 || p.y == h - 1);
                    assert!(
                        frame && !corner && g.tiles[*p].terrain != Terrain::Door,
                        "{p:?}"
                    );
                    assert!(
                        p.neighbors4().any(|q| q.x > 0
                            && q.y > 0
                            && q.x < w - 1
                            && q.y < h - 1
                            && g.tiles[q].terrain == Terrain::Floor),
                        "open floor beside {p:?}"
                    );
                }
            }
        }
        assert!(seen > 0, "no office in the test cities");
    }
}
