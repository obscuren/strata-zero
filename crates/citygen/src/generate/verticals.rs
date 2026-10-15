//! Stage 6: Undercity, Skyways and Heights derived from the street-level plan.

use super::{GenCtx, Stage, StageRunner, StepOut};
use crate::geom::{Pos, Rect};
use crate::params::GenError;
use crate::rng::chance;
use crate::tile::Terrain;
use crate::world::{StratumKind, Theme};
use noise::{NoiseFn, OpenSimplex};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::collections::VecDeque;

enum Work {
    Arterial(usize),
    Block(usize),
    Rail,
    Building(usize),
    Walkways(usize),
    Platforms(usize),
    Bridges(usize),
}

#[derive(Default)]
pub(crate) struct VerticalsStage {
    queue: VecDeque<Work>,
    total: usize,
    rng: Option<ChaCha8Rng>,
}

/// Chebyshev gap between two rects (0 when touching or overlapping).
fn rect_gap(a: Rect, b: Rect) -> i32 {
    let dx = (b.x - a.right()).max(a.x - b.right()).max(0);
    let dy = (b.y - a.bottom()).max(a.y - b.bottom()).max(0);
    dx.max(dy)
}

/// Overlapping range of two half-open intervals.
fn overlap(a0: i32, a1: i32, b0: i32, b1: i32) -> Option<(i32, i32)> {
    let lo = a0.max(b0);
    let hi = a1.min(b1);
    (hi > lo).then_some((lo, hi))
}

fn paint_if_void(ctx: &mut GenCtx, k: StratumKind, p: Pos, t: Terrain) {
    if ctx.is_land(p) && ctx.terrain(k, p) == Terrain::Void {
        ctx.set_terrain(k, p, t);
    }
}

impl VerticalsStage {
    fn arterial(&self, ctx: &mut GenCtx, i: usize) {
        let path = ctx.city.arterials[i].clone();
        for (step, p) in path.iter().enumerate() {
            let channel = Rect::new(p.x, p.y, 2, 2);
            let stamp = Rect::new(p.x - 1, p.y - 1, 4, 4);
            for q in stamp.iter() {
                if !ctx.is_land(q) {
                    continue;
                }
                if channel.contains(q) {
                    let t = if step % 12 == 0 {
                        Terrain::Grate
                    } else {
                        Terrain::Sewage
                    };
                    let cur = ctx.terrain(StratumKind::Undercity, q);
                    if cur != Terrain::Grate {
                        ctx.set_terrain(StratumKind::Undercity, q, t);
                    }
                } else {
                    paint_if_void(ctx, StratumKind::Undercity, q, Terrain::Tunnel);
                }
            }
        }
    }

    fn block(&mut self, ctx: &mut GenCtx, i: usize) {
        let rng = self.rng.as_mut().expect("begin sets rng");
        if !chance(rng, 1.0 / 3.0) {
            return;
        }
        let block = ctx.scratch.blocks[i];
        for p in block.rect.grow(2).iter() {
            let street = matches!(
                ctx.terrain(StratumKind::Street, p),
                Terrain::Road | Terrain::Alley | Terrain::Sidewalk
            );
            if street {
                paint_if_void(ctx, StratumKind::Undercity, p, Terrain::Tunnel);
            }
        }
    }

    fn rail(&mut self, ctx: &mut GenCtx) {
        let size = ctx.size();
        let rng = self.rng.as_mut().expect("begin sets rng");
        let h = size.h as i32;
        let y0 = h / 2 + rng.random_range(-(h / 8)..=(h / 8));
        let noise = OpenSimplex::new((ctx.seed ^ 0x8A11) as u32);
        let mut prev_y = y0;
        for x in 0..size.w as i32 {
            let y = (y0 as f64 + noise.get([x as f64 * 0.02, 0.5]) * 4.0).round() as i32;
            let (lo, hi) = (prev_y.min(y), prev_y.max(y));
            for yy in lo..=hi {
                let p = Pos::new(x, yy);
                if ctx.is_land(p) {
                    ctx.set_terrain(StratumKind::Undercity, p, Terrain::Rail);
                    paint_if_void(
                        ctx,
                        StratumKind::Undercity,
                        p.offset(0, -1),
                        Terrain::Tunnel,
                    );
                    paint_if_void(ctx, StratumKind::Undercity, p.offset(0, 1), Terrain::Tunnel);
                }
            }
            prev_y = y;
        }
    }

    fn building(&self, ctx: &mut GenCtx, i: usize) {
        let b = ctx.city.buildings[i].clone();
        for k in b.span.iter() {
            if k == StratumKind::Street {
                continue;
            }
            for p in b.footprint.iter() {
                let t = if k == StratumKind::Heights {
                    Terrain::Rooftop
                } else if b.footprint.border().any(|q| q == p) {
                    Terrain::Wall
                } else {
                    Terrain::Floor
                };
                if k == StratumKind::Undercity {
                    // Tunnels, sewers and rail run under buildings; basements fill the rest.
                    paint_if_void(ctx, k, p, t);
                } else {
                    ctx.set_terrain(k, p, t);
                }
            }
        }
    }

    /// Paints a 2-wide L-shaped walkway between the facing edges of two footprints.
    fn connect_walkway(ctx: &mut GenCtx, a: Rect, b: Rect) -> bool {
        let k = StratumKind::Skyways;
        let horizontal_gap = b.x >= a.right() || a.x >= b.right();
        let (start, end) = if horizontal_gap {
            let (left, right) = if b.x >= a.right() { (a, b) } else { (b, a) };
            let y_a = overlap(left.y, left.bottom(), right.y, right.bottom())
                .map(|(lo, hi)| (lo + hi) / 2)
                .unwrap_or(left.center().y);
            let y_b = overlap(left.y, left.bottom(), right.y, right.bottom())
                .map(|(lo, hi)| (lo + hi) / 2)
                .unwrap_or(right.center().y);
            (Pos::new(left.right(), y_a), Pos::new(right.x - 1, y_b))
        } else {
            let (top, bottom) = if b.y >= a.bottom() { (a, b) } else { (b, a) };
            let x_a = overlap(top.x, top.right(), bottom.x, bottom.right())
                .map(|(lo, hi)| (lo + hi) / 2)
                .unwrap_or(top.center().x);
            let x_b = overlap(top.x, top.right(), bottom.x, bottom.right())
                .map(|(lo, hi)| (lo + hi) / 2)
                .unwrap_or(bottom.center().x);
            (Pos::new(x_a, top.bottom()), Pos::new(x_b, bottom.y - 1))
        };
        for first_horizontal in [horizontal_gap, !horizontal_gap] {
            let mut tiles = Vec::new();
            let corner = if first_horizontal {
                Pos::new(end.x, start.y)
            } else {
                Pos::new(start.x, end.y)
            };
            for seg in [(start, corner), (corner, end)] {
                let (p, q) = seg;
                if p.y == q.y {
                    let (x0, x1) = (p.x.min(q.x), p.x.max(q.x));
                    for x in x0..=x1 {
                        tiles.push(Pos::new(x, p.y));
                        tiles.push(Pos::new(x, p.y + 1));
                    }
                } else {
                    let (y0, y1) = (p.y.min(q.y), p.y.max(q.y));
                    for y in y0..=y1 {
                        tiles.push(Pos::new(p.x, y));
                        tiles.push(Pos::new(p.x + 1, y));
                    }
                }
            }
            let clear = tiles.iter().all(|t| {
                ctx.city.size.contains(*t)
                    && matches!(ctx.terrain(k, *t), Terrain::Void | Terrain::Walkway)
                    && !a.contains(*t)
                    && !b.contains(*t)
            });
            if clear {
                for t in tiles {
                    ctx.set_terrain(k, t, Terrain::Walkway);
                }
                return true;
            }
        }
        false
    }

    fn walkways(&self, ctx: &mut GenCtx, i: usize) {
        let a = ctx.city.buildings[i].clone();
        if !a.span.contains(StratumKind::Skyways) {
            return;
        }
        let mut partners: Vec<(i32, usize)> = ctx
            .city
            .buildings
            .iter()
            .enumerate()
            .skip(i + 1)
            .filter(|(_, b)| b.span.contains(StratumKind::Skyways))
            .map(|(j, b)| (rect_gap(a.footprint, b.footprint), j))
            .filter(|(gap, _)| *gap >= 1 && *gap <= 12)
            .collect();
        partners.sort();
        for (_, j) in partners.into_iter().take(2) {
            let b = ctx.city.buildings[j].footprint;
            Self::connect_walkway(ctx, a.footprint, b);
        }
    }

    fn platforms(&self, ctx: &mut GenCtx, d: usize) {
        let district = ctx.city.districts[d].clone();
        if !matches!(district.theme, Theme::CorpCore | Theme::NeonStrip) {
            return;
        }
        let mut spots = Vec::new();
        for p in district.bounds.iter() {
            if !ctx.in_district(p, district.id)
                || ctx.terrain(StratumKind::Street, p) != Terrain::Road
            {
                continue;
            }
            let roads = p
                .neighbors4()
                .filter(|q| ctx.terrain(StratumKind::Street, *q) == Terrain::Road)
                .count();
            if roads >= 3 && (p.x + p.y) % 7 == 0 {
                spots.push(p);
            }
        }
        for p in spots {
            for q in Rect::new(p.x - 2, p.y - 2, 5, 5).iter() {
                paint_if_void(ctx, StratumKind::Skyways, q, Terrain::Walkway);
            }
        }
    }

    fn bridges(&self, ctx: &mut GenCtx, i: usize) {
        let a = ctx.city.buildings[i].clone();
        if !a.span.contains(StratumKind::Heights) {
            return;
        }
        let k = StratumKind::Heights;
        let partner = ctx
            .city
            .buildings
            .iter()
            .enumerate()
            .skip(i + 1)
            .filter(|(_, b)| b.span.contains(StratumKind::Heights))
            .map(|(j, b)| (rect_gap(a.footprint, b.footprint), j))
            .filter(|(gap, _)| *gap >= 1 && *gap <= 8)
            .min();
        let Some((_, j)) = partner else { return };
        let b = ctx.city.buildings[j].footprint;
        let fa = a.footprint;
        let mut tiles = Vec::new();
        if let Some((lo, hi)) = overlap(fa.y, fa.bottom(), b.y, b.bottom()) {
            let y = (lo + hi) / 2;
            let (x0, x1) = if b.x >= fa.right() {
                (fa.right(), b.x)
            } else {
                (b.right(), fa.x)
            };
            tiles.extend((x0..x1).map(|x| Pos::new(x, y)));
        } else if let Some((lo, hi)) = overlap(fa.x, fa.right(), b.x, b.right()) {
            let x = (lo + hi) / 2;
            let (y0, y1) = if b.y >= fa.bottom() {
                (fa.bottom(), b.y)
            } else {
                (b.bottom(), fa.y)
            };
            tiles.extend((y0..y1).map(|y| Pos::new(x, y)));
        }
        if !tiles.is_empty() && tiles.iter().all(|t| ctx.terrain(k, *t) == Terrain::Void) {
            for t in tiles {
                ctx.set_terrain(k, t, Terrain::Bridge);
            }
        }
    }
}

impl StageRunner for VerticalsStage {
    fn begin(&mut self, ctx: &mut GenCtx) -> Result<(), GenError> {
        self.rng = Some(ctx.rng(Stage::Verticals));
        self.queue.clear();
        self.queue
            .extend((0..ctx.city.arterials.len()).map(Work::Arterial));
        self.queue
            .extend((0..ctx.scratch.blocks.len()).map(Work::Block));
        self.queue.push_back(Work::Rail);
        let n = ctx.city.buildings.len();
        self.queue.extend((0..n).map(Work::Building));
        self.queue.extend((0..n).map(Work::Walkways));
        self.queue
            .extend((0..ctx.city.districts.len()).map(Work::Platforms));
        self.queue.extend((0..n).map(Work::Bridges));
        self.total = self.queue.len().max(1);
        Ok(())
    }

    fn step(&mut self, ctx: &mut GenCtx) -> Result<StepOut, GenError> {
        let Some(work) = self.queue.pop_front() else {
            return Ok(StepOut::Finished);
        };
        match work {
            Work::Arterial(i) => self.arterial(ctx, i),
            Work::Block(i) => self.block(ctx, i),
            Work::Rail => self.rail(ctx),
            Work::Building(i) => self.building(ctx, i),
            Work::Walkways(i) => self.walkways(ctx, i),
            Work::Platforms(d) => self.platforms(ctx, d),
            Work::Bridges(i) => self.bridges(ctx, i),
        }
        let done = self.total - self.queue.len();
        Ok(StepOut::Continue(done as f32 / self.total as f32))
    }
}

#[cfg(test)]
mod tests {
    use crate::generate::testutil::small;
    use crate::params::WaterMode;
    use crate::tile::Terrain;
    use crate::world::StratumKind;

    #[test]
    fn undercity_has_tunnels_under_arterials_and_a_rail() {
        let c = small(51, WaterMode::Bay, 7);
        let uc = c.stratum(StratumKind::Undercity).unwrap();
        for path in &c.arterials {
            for p in path {
                assert!(
                    matches!(
                        uc.tiles[*p].terrain,
                        Terrain::Tunnel | Terrain::Sewage | Terrain::Grate | Terrain::Rail
                    ),
                    "{p:?} is {:?}",
                    uc.tiles[*p].terrain
                );
            }
        }
        let rail = uc.tiles.count(|t| t.terrain == Terrain::Rail);
        assert!(
            rail >= (uc.tiles.size().w as f32 * 0.7) as usize,
            "rail {rail}"
        );
        assert!(uc.tiles.count(|t| t.terrain == Terrain::Sewage) > 0);
        assert!(uc.tiles.count(|t| t.terrain == Terrain::Grate) > 0);
    }

    #[test]
    fn skyways_and_heights_follow_spans() {
        let c = small(51, WaterMode::None, 7);
        let sk = c.stratum(StratumKind::Skyways).unwrap();
        let hi = c.stratum(StratumKind::Heights).unwrap();
        let uc = c.stratum(StratumKind::Undercity).unwrap();
        for b in &c.buildings {
            let p = b.footprint.center();
            if b.span.contains(StratumKind::Skyways) {
                assert!(sk.tiles[p].is_building_part() || sk.tiles[p].is_connector());
            } else {
                // Air above a low roof may carry an elevated walkway.
                assert!(matches!(
                    sk.tiles[p].terrain,
                    Terrain::Void | Terrain::Walkway
                ));
            }
            if b.span.contains(StratumKind::Heights) {
                assert!(matches!(
                    hi.tiles[p].terrain,
                    Terrain::Rooftop | Terrain::Lift | Terrain::Stair | Terrain::Door
                ));
            } else {
                assert!(matches!(
                    hi.tiles[p].terrain,
                    Terrain::Void | Terrain::Bridge
                ));
            }
            if b.span.contains(StratumKind::Undercity) {
                // Basement, or a tunnel/rail that already ran under the building.
                assert_ne!(uc.tiles[p].terrain, Terrain::Void);
            }
        }
        assert!(sk.tiles.count(|t| t.terrain == Terrain::Walkway) > 50);
    }
}
