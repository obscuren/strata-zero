//! Stage 3: arterial roads between district centres.

use super::{GenCtx, StageRunner, StepOut};
use crate::geom::{Pos, Rect};
use crate::params::GenError;
use crate::pathing::astar;
use crate::tile::Terrain;
use crate::world::StratumKind;

#[derive(Default)]
pub(crate) struct ArterialsStage {
    edges: Vec<(Pos, Pos)>,
    next: usize,
}

/// Minimum spanning tree (Prim) plus the shortest `extra` non-tree edges for loops.
fn plan_edges(centers: &[Pos], extra: usize) -> Vec<(Pos, Pos)> {
    let n = centers.len();
    if n < 2 {
        return Vec::new();
    }
    let mut in_tree = vec![false; n];
    let mut edges = Vec::new();
    in_tree[0] = true;
    for _ in 1..n {
        let mut best: Option<(i64, usize, usize)> = None;
        for a in 0..n {
            if !in_tree[a] {
                continue;
            }
            for b in 0..n {
                if in_tree[b] {
                    continue;
                }
                let d = centers[a].dist2(centers[b]);
                if best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, a, b));
                }
            }
        }
        let (_, a, b) = best.expect("some vertex remains");
        in_tree[b] = true;
        edges.push((a, b));
    }
    let mut candidates: Vec<(i64, usize, usize)> = Vec::new();
    for a in 0..n {
        for b in (a + 1)..n {
            let used = edges
                .iter()
                .any(|(x, y)| (*x == a && *y == b) || (*x == b && *y == a));
            if !used {
                candidates.push((centers[a].dist2(centers[b]), a, b));
            }
        }
    }
    candidates.sort();
    edges.extend(candidates.iter().take(extra).map(|(_, a, b)| (*a, *b)));
    edges
        .into_iter()
        .map(|(a, b)| (centers[a], centers[b]))
        .collect()
}

impl ArterialsStage {
    fn route(&self, ctx: &GenCtx, a: Pos, b: Pos) -> Option<Vec<Pos>> {
        let land = &ctx.scratch.land;
        let arterial = &ctx.scratch.arterial;
        astar(ctx.size(), a, b, |_, to| {
            if !land[to] {
                Some(40)
            } else if arterial[to] {
                Some(10)
            } else {
                Some(11)
            }
        })
    }

    fn paint(&self, ctx: &mut GenCtx, path: &[Pos]) {
        for p in path {
            for q in Rect::new(p.x - 1, p.y - 1, 4, 4).iter() {
                if !ctx.city.size.contains(q) {
                    continue;
                }
                let t = if ctx.is_land(q) {
                    Terrain::Road
                } else {
                    Terrain::Bridge
                };
                ctx.set_terrain(StratumKind::Street, q, t);
                ctx.scratch.arterial.set(q, true);
                ctx.scratch.road.set(q, true);
            }
        }
        for p in path {
            for q in Rect::new(p.x - 2, p.y - 2, 6, 6).border() {
                if ctx.is_land(q) && ctx.terrain(StratumKind::Street, q) == Terrain::Void {
                    ctx.set_terrain(StratumKind::Street, q, Terrain::Sidewalk);
                }
            }
        }
    }
}

impl StageRunner for ArterialsStage {
    fn begin(&mut self, ctx: &mut GenCtx) -> Result<(), GenError> {
        let centers: Vec<Pos> = ctx.city.districts.iter().map(|d| d.center).collect();
        let extra = (centers.len() / 3).max(2);
        self.edges = plan_edges(&centers, extra);
        self.next = 0;
        Ok(())
    }

    fn step(&mut self, ctx: &mut GenCtx) -> Result<StepOut, GenError> {
        if self.next >= self.edges.len() {
            return Ok(StepOut::Finished);
        }
        let (a, b) = self.edges[self.next];
        self.next += 1;
        if let Some(path) = self.route(ctx, a, b) {
            self.paint(ctx, &path);
            ctx.city.arterials.push(path);
        }
        Ok(StepOut::Continue(
            self.next as f32 / self.edges.len() as f32,
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::generate::testutil::small;
    use crate::geom::{Grid, Rect};
    use crate::params::WaterMode;
    use crate::pathing::components;
    use crate::tile::Terrain;
    use crate::world::StratumKind;

    #[test]
    fn arterials_connect_all_districts_and_avoid_water() {
        let c = small(21, WaterMode::Bay, 7);
        assert!(c.arterials.len() >= 6);
        let st = c.stratum(StratumKind::Street).unwrap();
        for path in &c.arterials {
            for p in path {
                assert_ne!(st.tiles[*p].terrain, Terrain::Water);
            }
        }
        for d in &c.districts {
            let near = Rect::new(d.center.x - 3, d.center.y - 3, 7, 7)
                .iter()
                .any(|p| st.tiles.get(p).is_some_and(|t| t.terrain == Terrain::Road));
            assert!(near, "district {:?} centre not on road", d.id);
        }
        // The arterial stamps themselves form one connected network.
        let mut mask = Grid::filled(st.tiles.size(), false);
        for path in &c.arterials {
            for p in path {
                mask.fill_rect(Rect::new(p.x - 1, p.y - 1, 4, 4), true);
            }
        }
        let comps = components(st.tiles.size(), |p| {
            mask[p] && matches!(st.tiles[p].terrain, Terrain::Road | Terrain::Bridge)
        });
        assert_eq!(comps.len(), 1);
    }

    #[test]
    fn arterial_width_is_four_with_sidewalks() {
        let c = small(21, WaterMode::None, 5);
        let st = c.stratum(StratumKind::Street).unwrap();
        let path = &c.arterials[0];
        let mid = path[path.len() / 2];
        let roads = Rect::new(mid.x - 3, mid.y - 3, 7, 7)
            .iter()
            .filter(|p| st.tiles.get(*p).is_some_and(|t| t.terrain == Terrain::Road))
            .count();
        assert!(
            roads >= 16,
            "expected a 4-wide road around {mid:?}, got {roads}"
        );
        assert!(st.tiles.count(|t| t.terrain == Terrain::Sidewalk) > 0);
    }
}
