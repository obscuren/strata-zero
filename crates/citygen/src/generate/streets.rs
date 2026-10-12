//! Stage 4: per-district street subdivision and block extraction.

use super::{Block, GenCtx, Stage, StageRunner, StepOut};
use crate::geom::{Pos, Rect};
use crate::ids::DistrictId;
use crate::params::GenError;
use crate::rng::chance;
use crate::theme::StreetStyle;
use crate::tile::{Feature, Terrain};
use crate::world::StratumKind;
use rand::Rng;
use rand_chacha::ChaCha8Rng;

#[derive(Default)]
pub(crate) struct StreetsStage {
    next: usize,
    rng: Option<ChaCha8Rng>,
}

/// Paints `terrain` at `p` only on untouched land of this district.
fn paint_cell(ctx: &mut GenCtx, p: Pos, d: DistrictId, terrain: Terrain) {
    if ctx.is_land(p)
        && ctx.in_district(p, d)
        && ctx.terrain(StratumKind::Street, p) == Terrain::Void
    {
        ctx.set_terrain(StratumKind::Street, p, terrain);
        if terrain != Terrain::Sidewalk {
            ctx.scratch.road.set(p, true);
        }
    }
}

/// A straight street segment to paint.
#[derive(Clone, Copy)]
struct Line {
    terrain: Terrain,
    width: i32,
    sidewalk: bool,
}

impl Line {
    const fn road(width: i32, sidewalk: bool) -> Self {
        Line {
            terrain: Terrain::Road,
            width,
            sidewalk,
        }
    }
    const ALLEY: Line = Line {
        terrain: Terrain::Alley,
        width: 1,
        sidewalk: false,
    };
}

/// A vertical street starting at column `x`, rows `y0..y1`.
fn paint_vline(ctx: &mut GenCtx, x: i32, y0: i32, y1: i32, line: Line, d: DistrictId) {
    for y in y0..y1 {
        for dx in 0..line.width {
            paint_cell(ctx, Pos::new(x + dx, y), d, line.terrain);
        }
        if line.sidewalk {
            paint_cell(ctx, Pos::new(x - 1, y), d, Terrain::Sidewalk);
            paint_cell(ctx, Pos::new(x + line.width, y), d, Terrain::Sidewalk);
        }
    }
}

/// A horizontal street starting at row `y`, columns `x0..x1`.
fn paint_hline(ctx: &mut GenCtx, y: i32, x0: i32, x1: i32, line: Line, d: DistrictId) {
    for x in x0..x1 {
        for dy in 0..line.width {
            paint_cell(ctx, Pos::new(x, y + dy), d, line.terrain);
        }
        if line.sidewalk {
            paint_cell(ctx, Pos::new(x, y - 1), d, Terrain::Sidewalk);
            paint_cell(ctx, Pos::new(x, y + line.width), d, Terrain::Sidewalk);
        }
    }
}

/// Positions of parallel street lines across `[lo, hi)` with the given pitch and a random offset.
fn line_positions(rng: &mut ChaCha8Rng, lo: i32, hi: i32, pitch: i32) -> Vec<i32> {
    let offset = rng.random_range(0..pitch.max(1));
    let mut out = Vec::new();
    let mut x = lo + offset;
    while x < hi {
        out.push(x);
        x += pitch;
    }
    out
}

/// Blocks between consecutive line positions; `gap` is the street width plus sidewalks.
fn cells_between(lines: &[i32], lo: i32, hi: i32, gap: i32, pad: i32) -> Vec<(i32, i32)> {
    let mut edges = vec![lo - gap];
    edges.extend_from_slice(lines);
    edges.push(hi);
    edges
        .windows(2)
        .map(|w| (w[0] + gap + pad, w[1] - pad))
        .filter(|(a, b)| b > a)
        .collect()
}

struct Carver<'a> {
    ctx: &'a mut GenCtx,
    rng: &'a mut ChaCha8Rng,
    d: DistrictId,
    bounds: Rect,
    candidates: Vec<Rect>,
}

impl Carver<'_> {
    fn grid(&mut self, bw: i32, bh: i32, sw: i32, sidewalk: bool) -> (Vec<i32>, Vec<i32>) {
        let pad = if sidewalk { 1 } else { 0 };
        let b = self.bounds;
        let xs = line_positions(self.rng, b.x, b.right(), bw + sw + 2 * pad);
        let ys = line_positions(self.rng, b.y, b.bottom(), bh + sw + 2 * pad);
        for x in &xs {
            paint_vline(
                self.ctx,
                *x,
                b.y,
                b.bottom(),
                Line::road(sw, sidewalk),
                self.d,
            );
        }
        for y in &ys {
            paint_hline(
                self.ctx,
                *y,
                b.x,
                b.right(),
                Line::road(sw, sidewalk),
                self.d,
            );
        }
        for (x0, x1) in cells_between(&xs, b.x, b.right(), sw, pad) {
            for (y0, y1) in cells_between(&ys, b.y, b.bottom(), sw, pad) {
                self.candidates.push(Rect::new(x0, y0, x1 - x0, y1 - y0));
            }
        }
        (xs, ys)
    }

    fn organic(&mut self, rect: Rect, max_w: i32, max_h: i32, depth: u8) {
        const MIN_W: i32 = 8;
        const MIN_H: i32 = 6;
        let split_w = rect.w > max_w && rect.w > 2 * MIN_W;
        let split_h = rect.h > max_h && rect.h > 2 * MIN_H;
        if depth == 0 || (!split_w && !split_h) {
            self.candidates.push(rect);
            return;
        }
        let vertical = if split_w && split_h {
            rect.w >= rect.h
        } else {
            split_w
        };
        let jitter = self.rng.random_range(0.35..0.65);
        if vertical {
            let s = (rect.x + (rect.w as f32 * jitter) as i32)
                .clamp(rect.x + MIN_W, rect.right() - MIN_W - 1);
            paint_vline(self.ctx, s, rect.y, rect.bottom(), Line::ALLEY, self.d);
            self.organic(
                Rect::new(rect.x, rect.y, s - rect.x, rect.h),
                max_w,
                max_h,
                depth - 1,
            );
            self.organic(
                Rect::new(s + 1, rect.y, rect.right() - s - 1, rect.h),
                max_w,
                max_h,
                depth - 1,
            );
        } else {
            let s = (rect.y + (rect.h as f32 * jitter) as i32)
                .clamp(rect.y + MIN_H, rect.bottom() - MIN_H - 1);
            paint_hline(self.ctx, s, rect.x, rect.right(), Line::ALLEY, self.d);
            self.organic(
                Rect::new(rect.x, rect.y, rect.w, s - rect.y),
                max_w,
                max_h,
                depth - 1,
            );
            self.organic(
                Rect::new(rect.x, s + 1, rect.w, rect.bottom() - s - 1),
                max_w,
                max_h,
                depth - 1,
            );
        }
    }

    fn rows(&mut self, row_h: i32, access_pitch: i32) {
        let b = self.bounds;
        let ys = line_positions(self.rng, b.y, b.bottom(), row_h + 4);
        let xs = line_positions(self.rng, b.x, b.right(), access_pitch);
        for y in &ys {
            paint_hline(self.ctx, *y, b.x, b.right(), Line::road(2, true), self.d);
        }
        for x in &xs {
            paint_vline(self.ctx, *x, b.y, b.bottom(), Line::road(2, true), self.d);
        }
        for (x0, x1) in cells_between(&xs, b.x, b.right(), 2, 1) {
            for (y0, y1) in cells_between(&ys, b.y, b.bottom(), 2, 1) {
                self.candidates.push(Rect::new(x0, y0, x1 - x0, y1 - y0));
            }
        }
    }

    fn shattered(&mut self) {
        let (xs, ys) = self.grid(16, 10, 2, false);
        let b = self.bounds;
        // Cut 30% of the vertical segments between consecutive horizontal streets.
        let mut ybounds = vec![b.y - 2];
        ybounds.extend_from_slice(&ys);
        ybounds.push(b.bottom());
        for x in &xs {
            for w in ybounds.windows(2) {
                if !chance(self.rng, 0.3) {
                    continue;
                }
                let (y0, y1) = (w[0] + 2, w[1]);
                if y1 - y0 < 3 {
                    continue;
                }
                let mut cut = Vec::new();
                for y in y0..y1 {
                    for dx in 0..2 {
                        let p = Pos::new(x + dx, y);
                        let mine = self.ctx.in_district(p, self.d)
                            && self.ctx.terrain(StratumKind::Street, p) == Terrain::Road
                            && !self.ctx.scratch.arterial[p];
                        if mine {
                            self.ctx.set_terrain(StratumKind::Street, p, Terrain::Alley);
                            cut.push(p);
                        }
                    }
                }
                if let (Some(first), Some(last)) = (cut.first().copied(), cut.last().copied()) {
                    self.ctx
                        .set_feature(StratumKind::Street, first, Some(Feature::Barrier));
                    self.ctx
                        .set_feature(StratumKind::Street, last, Some(Feature::Barrier));
                }
            }
        }
    }

    fn strip(&mut self) {
        let b = self.bounds;
        let center = self.ctx.city.district(self.d).center;
        let horizontal = b.w >= b.h;
        if horizontal {
            let y = center.y - 1;
            paint_hline(self.ctx, y, b.x, b.right(), Line::road(3, true), self.d);
            let xs = line_positions(self.rng, b.x, b.right(), 10);
            for x in &xs {
                paint_vline(self.ctx, *x, b.y, b.bottom(), Line::ALLEY, self.d);
            }
            for (x0, x1) in cells_between(&xs, b.x, b.right(), 1, 0) {
                self.candidates
                    .push(Rect::new(x0, b.y, x1 - x0, y - 1 - b.y));
                self.candidates
                    .push(Rect::new(x0, y + 4, x1 - x0, b.bottom() - (y + 4)));
            }
        } else {
            let x = center.x - 1;
            paint_vline(self.ctx, x, b.y, b.bottom(), Line::road(3, true), self.d);
            let ys = line_positions(self.rng, b.y, b.bottom(), 10);
            for y in &ys {
                paint_hline(self.ctx, *y, b.x, b.right(), Line::ALLEY, self.d);
            }
            for (y0, y1) in cells_between(&ys, b.y, b.bottom(), 1, 0) {
                self.candidates
                    .push(Rect::new(b.x, y0, x - 1 - b.x, y1 - y0));
                self.candidates
                    .push(Rect::new(x + 4, y0, b.right() - (x + 4), y1 - y0));
            }
        }
    }

    fn is_bad(&self, p: Pos) -> bool {
        !self.ctx.is_land(p)
            || !self.ctx.in_district(p, self.d)
            || self.ctx.terrain(StratumKind::Street, p) != Terrain::Void
    }

    /// Drops edge rows/columns that are entirely unusable.
    fn trim(&self, mut r: Rect) -> Rect {
        loop {
            if r.w < 1 || r.h < 1 {
                return r;
            }
            if (r.x..r.right()).all(|x| self.is_bad(Pos::new(x, r.y))) {
                r = Rect::new(r.x, r.y + 1, r.w, r.h - 1);
            } else if (r.x..r.right()).all(|x| self.is_bad(Pos::new(x, r.bottom() - 1))) {
                r = Rect::new(r.x, r.y, r.w, r.h - 1);
            } else if (r.y..r.bottom()).all(|y| self.is_bad(Pos::new(r.x, y))) {
                r = Rect::new(r.x + 1, r.y, r.w - 1, r.h);
            } else if (r.y..r.bottom()).all(|y| self.is_bad(Pos::new(r.right() - 1, y))) {
                r = Rect::new(r.x, r.y, r.w - 1, r.h);
            } else {
                return r;
            }
        }
    }

    /// Greedy shrink from the dirtiest side until clean or too small.
    fn shrink(&self, mut r: Rect) -> Option<Rect> {
        for _ in 0..12 {
            if r.w < 5 || r.h < 4 {
                return None;
            }
            let top = (r.x..r.right())
                .filter(|x| self.is_bad(Pos::new(*x, r.y)))
                .count();
            let bottom = (r.x..r.right())
                .filter(|x| self.is_bad(Pos::new(*x, r.bottom() - 1)))
                .count();
            let left = (r.y..r.bottom())
                .filter(|y| self.is_bad(Pos::new(r.x, *y)))
                .count();
            let right = (r.y..r.bottom())
                .filter(|y| self.is_bad(Pos::new(r.right() - 1, *y)))
                .count();
            if top + bottom + left + right == 0 {
                match r.inset(1).iter().find(|p| self.is_bad(*p)) {
                    None => return Some(r),
                    Some(p) => {
                        if (p.x - r.x) * r.h < (p.y - r.y) * r.w {
                            r = Rect::new(p.x + 1, r.y, r.right() - p.x - 1, r.h);
                        } else {
                            r = Rect::new(r.x, p.y + 1, r.w, r.bottom() - p.y - 1);
                        }
                        continue;
                    }
                }
            }
            let fr = |n: usize, len: i32| n as f32 / len as f32;
            let (t, b, l, rr) = (fr(top, r.w), fr(bottom, r.w), fr(left, r.h), fr(right, r.h));
            let max = t.max(b).max(l).max(rr);
            if max == t {
                r = Rect::new(r.x, r.y + 1, r.w, r.h - 1);
            } else if max == b {
                r = Rect::new(r.x, r.y, r.w, r.h - 1);
            } else if max == l {
                r = Rect::new(r.x + 1, r.y, r.w - 1, r.h);
            } else {
                r = Rect::new(r.x, r.y, r.w - 1, r.h);
            }
        }
        None
    }

    /// Turns one candidate into clean blocks: trims dead edges, and when the rest is still
    /// dirty, splits it along the longer axis (painting an alley on the cut) and recurses.
    fn fit_blocks(&mut self, r: Rect, depth: u8, out: &mut Vec<Rect>) {
        let r = self.trim(r);
        if r.w < 5 || r.h < 4 {
            return;
        }
        if !r.iter().any(|p| self.is_bad(p)) {
            out.push(r);
            return;
        }
        let can_split_w = r.w >= 11;
        let can_split_h = r.h >= 9;
        if depth > 0 && (can_split_w || can_split_h) {
            let vertical = if can_split_w && can_split_h {
                r.w >= r.h
            } else {
                can_split_w
            };
            if vertical {
                let s = r.x + r.w / 2;
                paint_vline(self.ctx, s, r.y, r.bottom(), Line::ALLEY, self.d);
                self.fit_blocks(Rect::new(r.x, r.y, s - r.x, r.h), depth - 1, out);
                self.fit_blocks(
                    Rect::new(s + 1, r.y, r.right() - s - 1, r.h),
                    depth - 1,
                    out,
                );
            } else {
                let s = r.y + r.h / 2;
                paint_hline(self.ctx, s, r.x, r.right(), Line::ALLEY, self.d);
                self.fit_blocks(Rect::new(r.x, r.y, r.w, s - r.y), depth - 1, out);
                self.fit_blocks(
                    Rect::new(r.x, s + 1, r.w, r.bottom() - s - 1),
                    depth - 1,
                    out,
                );
            }
            return;
        }
        if let Some(fitted) = self.shrink(r) {
            out.push(fitted);
        }
    }
}

impl StreetsStage {
    fn carve_district(&mut self, ctx: &mut GenCtx, idx: usize) {
        let district = ctx.city.districts[idx].clone();
        let spec = district.theme.spec();
        let rng = self.rng.as_mut().expect("begin sets rng");
        let mut carver = Carver {
            ctx,
            rng,
            d: district.id,
            bounds: district.bounds,
            candidates: Vec::new(),
        };
        match spec.street_style {
            StreetStyle::Grid => {
                carver.grid(spec.block.0, spec.block.1, 2, true);
            }
            StreetStyle::Superblock => {
                carver.grid(spec.block.0, spec.block.1, 2, true);
            }
            StreetStyle::Organic => {
                let (max_w, max_h) = (spec.block.0 * 3 / 2, spec.block.1 * 3 / 2);
                let b = carver.bounds;
                carver.organic(b, max_w, max_h, 7);
            }
            StreetStyle::Rows => carver.rows(14, 24),
            StreetStyle::Shattered => carver.shattered(),
            StreetStyle::Strip => carver.strip(),
        }
        let candidates = std::mem::take(&mut carver.candidates);
        let mut fitted = Vec::new();
        for c in candidates {
            let Some(c) = c.clamp_to(carver.bounds) else {
                continue;
            };
            carver.fit_blocks(c, 4, &mut fitted);
        }
        let mut blocks: Vec<Block> = Vec::new();
        for rect in fitted {
            if blocks.iter().all(|b| !b.rect.intersects(rect)) {
                blocks.push(Block {
                    rect,
                    district: district.id,
                });
            }
        }
        carver.ctx.scratch.blocks.extend(blocks);
    }
}

impl StageRunner for StreetsStage {
    fn begin(&mut self, ctx: &mut GenCtx) -> Result<(), GenError> {
        self.rng = Some(ctx.rng(Stage::Streets));
        self.next = 0;
        ctx.scratch.blocks.clear();
        Ok(())
    }

    fn step(&mut self, ctx: &mut GenCtx) -> Result<StepOut, GenError> {
        let n = ctx.city.districts.len();
        if self.next >= n {
            return Ok(StepOut::Finished);
        }
        self.carve_district(ctx, self.next);
        self.next += 1;
        Ok(StepOut::Continue(self.next as f32 / n as f32))
    }
}

#[cfg(test)]
mod tests {
    use crate::generate::testutil::small;
    use crate::generate::{Generator, Stage};
    use crate::params::{GenParams, SizePreset, WaterMode};
    use crate::pathing::components;
    use crate::tile::{Feature, Terrain};
    use crate::world::{StratumKind, Theme};

    #[test]
    fn blocks_are_inside_districts_and_off_roads() {
        let ctx = Generator::run_until(
            GenParams {
                seed: 31,
                water: WaterMode::Bay,
                districts: 7,
                size: SizePreset::Custom { w: 120, h: 60 },
                ..Default::default()
            },
            Stage::Streets,
        );
        let st = ctx.city.stratum(StratumKind::Street).unwrap();
        let blocks = &ctx.scratch.blocks;
        assert!(blocks.len() > 20, "only {} blocks", blocks.len());
        for b in blocks {
            assert!(b.rect.w >= 5 && b.rect.h >= 4, "{:?}", b.rect);
            for p in b.rect.iter() {
                let t = &st.tiles[p];
                assert_eq!(t.district, Some(b.district), "{p:?} in {:?}", b.rect);
                assert!(
                    !matches!(
                        t.terrain,
                        Terrain::Road | Terrain::Alley | Terrain::Water | Terrain::Sidewalk
                    ),
                    "{p:?} is {:?}",
                    t.terrain
                );
            }
        }
        for (i, a) in blocks.iter().enumerate() {
            for b in &blocks[i + 1..] {
                assert!(
                    !a.rect.intersects(b.rect),
                    "{:?} overlaps {:?}",
                    a.rect,
                    b.rect
                );
            }
        }
    }

    #[test]
    fn street_network_is_connected_per_street_level() {
        let c = small(31, WaterMode::None, 6);
        let st = c.stratum(StratumKind::Street).unwrap();
        let comps = components(st.tiles.size(), |p| {
            matches!(
                st.tiles[p].terrain,
                Terrain::Road | Terrain::Alley | Terrain::Sidewalk
            )
        });
        let total: usize = comps.iter().map(|c| c.len()).sum();
        let share = comps[0].len() as f32 / total as f32;
        assert!(share > 0.97, "main street component only {share}");
    }

    #[test]
    fn combat_zone_has_barriers_and_grid_has_none() {
        let c = small(31, WaterMode::None, 7);
        let st = c.stratum(StratumKind::Street).unwrap();
        let count = |theme: Theme| {
            st.tiles
                .iter()
                .filter(|(_, t)| {
                    t.feature == Some(Feature::Barrier)
                        && t.district.map(|d| c.district(d).theme) == Some(theme)
                })
                .count()
        };
        assert!(count(Theme::CombatZone) > 0);
        assert_eq!(count(Theme::CorpCore), 0);
    }
}
