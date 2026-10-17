//! Stage 5: lots and building placement.

use super::{GenCtx, Lot, LotUse, Stage, StageRunner, StepOut};
use crate::geom::{Pos, Rect};
use crate::ids::{BuildingId, DistrictId};
use crate::params::GenError;
use crate::rng::{chance, pick_weighted};
use crate::theme::{StreetStyle, ThemeSpec};
use crate::tile::Terrain;
use crate::world::{Building, BuildingKind, HeightClass, StrataSpan, StratumKind, Theme};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::collections::HashSet;

/// At most this many megablocks per city.
const MAX_MEGABLOCKS: usize = 2;
/// A megablock's block must be at least this wide and high, room for an atrium plan.
const MEGABLOCK_MIN: (i32, i32) = (16, 12);

#[derive(Default)]
pub(crate) struct LotsStage {
    next: usize,
    filled: bool,
    rng: Option<ChaCha8Rng>,
    /// Indices into `scratch.blocks` that become one whole-block megablock each.
    mega_blocks: HashSet<usize>,
}

/// Splits a run `[lo, hi)` into pieces of length in `[min, 2*min)` with jittered cuts.
fn split_run(rng: &mut ChaCha8Rng, lo: i32, hi: i32, min: i32, out: &mut Vec<(i32, i32)>) {
    let len = hi - lo;
    if len < 2 * min {
        out.push((lo, hi));
        return;
    }
    let jitter: f32 = rng.random_range(0.4..0.6);
    let s = (lo + (len as f32 * jitter) as i32).clamp(lo + min, hi - min);
    split_run(rng, lo, s, min, out);
    split_run(rng, s, hi, min, out);
}

/// Splits a block into lots: at most two back-to-back rows (or columns), each cut into
/// strips along the long axis, so every lot touches the block perimeter.
fn bsp(rng: &mut ChaCha8Rng, rect: Rect, min: (i32, i32), out: &mut Vec<Rect>) {
    let (min_w, min_h) = min;
    if rect.w >= rect.h {
        let mut rows = Vec::new();
        if rect.h >= 2 * min_h {
            let s = rect.y + rect.h / 2;
            rows.push((rect.y, s));
            rows.push((s, rect.bottom()));
        } else {
            rows.push((rect.y, rect.bottom()));
        }
        for (y0, y1) in rows {
            let mut cols = Vec::new();
            split_run(rng, rect.x, rect.right(), min_w, &mut cols);
            for (x0, x1) in cols {
                out.push(Rect::new(x0, y0, x1 - x0, y1 - y0));
            }
        }
    } else {
        let mut cols = Vec::new();
        if rect.w >= 2 * min_w {
            let s = rect.x + rect.w / 2;
            cols.push((rect.x, s));
            cols.push((s, rect.right()));
        } else {
            cols.push((rect.x, rect.right()));
        }
        for (x0, x1) in cols {
            let mut rows = Vec::new();
            split_run(rng, rect.y, rect.bottom(), min_h, &mut rows);
            for (y0, y1) in rows {
                out.push(Rect::new(x0, y0, x1 - x0, y1 - y0));
            }
        }
    }
}

fn choose_class(rng: &mut ChaCha8Rng, kind: BuildingKind, spec: &ThemeSpec) -> HeightClass {
    let allowed = kind.height_classes();
    if allowed.contains(&HeightClass::Tall) && chance(rng, spec.tall) {
        HeightClass::Tall
    } else if allowed.contains(&HeightClass::Mid) && chance(rng, spec.mid) {
        HeightClass::Mid
    } else if allowed.contains(&HeightClass::Low) {
        HeightClass::Low
    } else {
        allowed[0]
    }
}

/// Raises one building on `lot`: class, basement, walls and floor, and its strata span.
fn place_building(
    ctx: &mut GenCtx,
    rng: &mut ChaCha8Rng,
    lot: Rect,
    kind: BuildingKind,
    spec: &ThemeSpec,
    district: DistrictId,
) {
    let class = choose_class(rng, kind, spec);
    let basement = chance(rng, spec.basement);
    let mut footprint = lot;
    if matches!(spec.street_style, StreetStyle::Organic | StreetStyle::Strip) && chance(rng, 0.3) {
        footprint = match rng.random_range(0..4) {
            0 if lot.w > 4 => Rect::new(lot.x + 1, lot.y, lot.w - 1, lot.h),
            1 if lot.w > 4 => Rect::new(lot.x, lot.y, lot.w - 1, lot.h),
            2 if lot.h > 3 => Rect::new(lot.x, lot.y + 1, lot.w, lot.h - 1),
            3 if lot.h > 3 => Rect::new(lot.x, lot.y, lot.w, lot.h - 1),
            _ => lot,
        };
    }
    let id = BuildingId(ctx.city.buildings.len() as u32);
    let span = StrataSpan::from_class(class, basement);
    for p in footprint.iter() {
        let on_border = p.x == footprint.x
            || p.y == footprint.y
            || p.x == footprint.right() - 1
            || p.y == footprint.bottom() - 1;
        ctx.set_terrain(
            StratumKind::Street,
            p,
            if on_border {
                Terrain::Wall
            } else {
                Terrain::Floor
            },
        );
        for k in span.iter() {
            if let Some(t) = ctx.city.tile_mut(k, p) {
                t.building = Some(id);
            }
        }
    }
    ctx.city.buildings.push(Building {
        id,
        kind,
        district,
        footprint,
        span,
        name: None,
    });
    ctx.scratch.lots.push(Lot {
        rect: lot,
        district,
        use_: LotUse::Building {
            kind,
            class,
            basement,
        },
    });
}

impl LotsStage {
    fn place_block(&mut self, ctx: &mut GenCtx, idx: usize) {
        let block = ctx.scratch.blocks[idx];
        let district = ctx.city.district(block.district).clone();
        let spec = district.theme.spec();
        let density = ctx.params().density;
        let rng = self.rng.as_mut().expect("begin sets rng");

        if self.mega_blocks.contains(&idx) {
            place_building(
                ctx,
                rng,
                block.rect,
                BuildingKind::Megablock,
                spec,
                district.id,
            );
            return;
        }

        // Megablocks are landmarks chosen whole-block in `begin`, never drawn per lot.
        let mix: Vec<(BuildingKind, u32)> = spec
            .mix
            .iter()
            .copied()
            .filter(|(k, _)| *k != BuildingKind::Megablock)
            .collect();
        let mut lots = Vec::new();
        bsp(rng, block.rect, spec.min_lot, &mut lots);
        for lot in lots {
            if lot.w < 4 || lot.h < 3 {
                ctx.stratum_fill(StratumKind::Street, lot, Terrain::Plaza);
                ctx.scratch.lots.push(Lot {
                    rect: lot,
                    district: district.id,
                    use_: LotUse::Plaza,
                });
                continue;
            }
            // A district's first lot is always a building, so no district ends up empty.
            let has_building = ctx.city.buildings.iter().any(|b| b.district == district.id);
            let plaza_p = spec.plaza * (1.3 - density);
            if has_building && chance(rng, plaza_p) {
                let parking = matches!(district.theme, Theme::CorpCore | Theme::Industrial)
                    && rng.random_bool(0.5);
                ctx.stratum_fill(StratumKind::Street, lot, Terrain::Plaza);
                ctx.scratch.lots.push(Lot {
                    rect: lot,
                    district: district.id,
                    use_: if parking {
                        LotUse::Parking
                    } else {
                        LotUse::Plaza
                    },
                });
                continue;
            }
            let kind = *pick_weighted(rng, &mix);
            place_building(ctx, rng, lot, kind, spec, district.id);
        }
    }

    /// Any zoned land still untouched becomes a back alley so nothing is unreachable void.
    fn fill_leftovers(ctx: &mut GenCtx) {
        let size = ctx.size();
        for y in 0..size.h as i32 {
            for x in 0..size.w as i32 {
                let p = Pos::new(x, y);
                let Some(t) = ctx.city.tile(StratumKind::Street, p) else {
                    continue;
                };
                if t.district.is_some() && t.terrain == Terrain::Void {
                    ctx.set_terrain(StratumKind::Street, p, Terrain::Alley);
                }
            }
        }
    }
}

impl GenCtx {
    /// Fills a rect with `terrain` on one stratum (no district check).
    pub fn stratum_fill(&mut self, k: StratumKind, r: Rect, terrain: Terrain) {
        for p in r.iter() {
            self.set_terrain(k, p, terrain);
        }
    }
}

impl StageRunner for LotsStage {
    fn begin(&mut self, ctx: &mut GenCtx) -> Result<(), GenError> {
        self.rng = Some(ctx.rng(Stage::Lots));
        // Each Megablocks district gets its largest big-enough block as one megablock.
        self.mega_blocks.clear();
        for d in ctx
            .city
            .districts
            .iter()
            .filter(|d| d.theme == Theme::Megablocks)
        {
            if self.mega_blocks.len() == MAX_MEGABLOCKS {
                break;
            }
            let best = ctx
                .scratch
                .blocks
                .iter()
                .enumerate()
                .filter(|(_, b)| b.district == d.id)
                .filter(|(_, b)| b.rect.w >= MEGABLOCK_MIN.0 && b.rect.h >= MEGABLOCK_MIN.1)
                .max_by_key(|(i, b)| (b.rect.area(), std::cmp::Reverse(*i)))
                .map(|(i, _)| i);
            self.mega_blocks.extend(best);
        }
        self.next = 0;
        self.filled = false;
        ctx.scratch.lots.clear();
        Ok(())
    }

    fn step(&mut self, ctx: &mut GenCtx) -> Result<StepOut, GenError> {
        let n = ctx.scratch.blocks.len();
        if self.next < n {
            self.place_block(ctx, self.next);
            self.next += 1;
            return Ok(StepOut::Continue(0.95 * self.next as f32 / n as f32));
        }
        if !self.filled {
            Self::fill_leftovers(ctx);
            self.filled = true;
            return Ok(StepOut::Continue(1.0));
        }
        Ok(StepOut::Finished)
    }
}

#[cfg(test)]
mod tests {
    use crate::generate::Generator;
    use crate::generate::testutil::small;
    use crate::params::{GenParams, SizePreset, WaterMode};
    use crate::tile::Terrain;
    use crate::world::{BuildingKind, StratumKind, Theme};

    #[test]
    fn buildings_fit_blocks_and_do_not_overlap() {
        let c = small(41, WaterMode::Bay, 7);
        assert!(
            c.buildings.len() > 30,
            "only {} buildings",
            c.buildings.len()
        );
        let st = c.stratum(StratumKind::Street).unwrap();
        for (i, a) in c.buildings.iter().enumerate() {
            assert_eq!(a.id.index(), i);
            assert!(
                a.footprint.w >= 4 && a.footprint.h >= 3,
                "{:?}",
                a.footprint
            );
            for b in &c.buildings[i + 1..] {
                assert!(
                    !a.footprint.intersects(b.footprint),
                    "{:?} overlaps {:?}",
                    a.id,
                    b.id
                );
            }
            for p in a.footprint.border() {
                assert!(
                    matches!(
                        st.tiles[p].terrain,
                        Terrain::Wall | Terrain::Door | Terrain::Window
                    ),
                    "{p:?} is {:?}",
                    st.tiles[p].terrain
                );
                assert_eq!(st.tiles[p].building, Some(a.id));
            }
            for p in a.footprint.inset(1).iter() {
                assert!(st.tiles[p].terrain != Terrain::Void && !st.tiles[p].is_outdoor_ground());
            }
            assert_eq!(st.tiles[a.footprint.center()].district, Some(a.district));
        }
    }

    #[test]
    fn corp_core_has_tall_buildings_and_no_void_left_on_land() {
        let c = small(41, WaterMode::None, 7);
        let core = c
            .districts
            .iter()
            .find(|d| d.theme == Theme::CorpCore)
            .unwrap()
            .id;
        assert!(
            c.buildings
                .iter()
                .any(|b| b.district == core && b.span.hi == StratumKind::Heights)
        );
        let st = c.stratum(StratumKind::Street).unwrap();
        assert_eq!(
            st.tiles
                .iter()
                .filter(|(_, t)| t.terrain == Terrain::Void && t.district.is_some())
                .count(),
            0
        );
    }

    #[test]
    fn density_changes_plaza_share() {
        let run = |seed, density| {
            Generator::run(GenParams {
                seed,
                density,
                size: SizePreset::Custom { w: 120, h: 60 },
                districts: 5,
                ..Default::default()
            })
            .unwrap()
            .buildings
            .len()
        };
        let lo: usize = (4..8).map(|s| run(s, 0.2)).sum();
        let hi: usize = (4..8).map(|s| run(s, 0.95)).sum();
        assert!(
            lo < hi,
            "low density {lo} should yield fewer buildings than high {hi}"
        );
    }

    #[test]
    fn megablocks_are_rare_and_take_a_whole_block() {
        for (seed, size, districts) in [
            (6, SizePreset::Custom { w: 120, h: 60 }, 8),
            (8, SizePreset::Medium, 7),
            (11, SizePreset::Medium, 7),
            (42, SizePreset::Medium, 7),
            (1, SizePreset::Medium, 7),
            (2, SizePreset::Medium, 7),
        ] {
            let c = Generator::run(GenParams {
                seed,
                size,
                districts,
                ..Default::default()
            })
            .unwrap();
            let megas: Vec<_> = c
                .buildings
                .iter()
                .filter(|b| b.kind == BuildingKind::Megablock)
                .collect();
            // Seeds 6, 11 and 42 cut their Megablocks district into blocks too small to qualify.
            let expected = if matches!(seed, 6 | 11 | 42) {
                0..=2
            } else {
                1..=2
            };
            assert!(
                expected.contains(&megas.len()),
                "seed {seed}: {} megablocks",
                megas.len()
            );
            for m in &megas {
                assert_eq!(c.district(m.district).theme, Theme::Megablocks);
                assert!(m.footprint.w >= 16 && m.footprint.h >= 12);
                // No other building shares its block: nothing of another building touches the footprint.
                assert!(
                    c.buildings.iter().filter(|b| b.id != m.id).all(|b| b
                        .footprint
                        .grow(1)
                        .intersection(m.footprint)
                        .is_none())
                );
            }
        }
    }

    #[test]
    fn a_city_too_small_for_a_megablocks_district_has_no_megablock() {
        let c = Generator::run(GenParams {
            seed: 3,
            size: SizePreset::Small,
            districts: 4,
            ..Default::default()
        })
        .unwrap();
        if !c.districts.iter().any(|d| d.theme == Theme::Megablocks) {
            assert!(
                c.buildings
                    .iter()
                    .all(|b| b.kind != BuildingKind::Megablock)
            );
        }
    }
}
