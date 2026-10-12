//! Stage 5: lots and building placement.

use super::{GenCtx, Lot, LotUse, Stage, StageRunner, StepOut};
use crate::geom::{Pos, Rect};
use crate::ids::BuildingId;
use crate::params::GenError;
use crate::rng::{chance, pick_weighted};
use crate::theme::{StreetStyle, ThemeSpec};
use crate::tile::Terrain;
use crate::world::{Building, BuildingKind, HeightClass, StrataSpan, StratumKind};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

#[derive(Default)]
pub(crate) struct LotsStage {
    next: usize,
    filled: bool,
    rng: Option<ChaCha8Rng>,
}

/// Splits a block into lots no smaller than `min` and no larger than twice that.
fn bsp(rng: &mut ChaCha8Rng, rect: Rect, min: (i32, i32), out: &mut Vec<Rect>) {
    let (min_w, min_h) = min;
    let split_w = rect.w > 2 * min_w;
    let split_h = rect.h > 2 * min_h;
    if !split_w && !split_h {
        out.push(rect);
        return;
    }
    let vertical = if split_w && split_h {
        rect.w as f32 / min_w as f32 >= rect.h as f32 / min_h as f32
    } else {
        split_w
    };
    let jitter: f32 = rng.random_range(0.4..0.6);
    if vertical {
        let s =
            (rect.x + (rect.w as f32 * jitter) as i32).clamp(rect.x + min_w, rect.right() - min_w);
        bsp(rng, Rect::new(rect.x, rect.y, s - rect.x, rect.h), min, out);
        bsp(
            rng,
            Rect::new(s, rect.y, rect.right() - s, rect.h),
            min,
            out,
        );
    } else {
        let s =
            (rect.y + (rect.h as f32 * jitter) as i32).clamp(rect.y + min_h, rect.bottom() - min_h);
        bsp(rng, Rect::new(rect.x, rect.y, rect.w, s - rect.y), min, out);
        bsp(
            rng,
            Rect::new(rect.x, s, rect.w, rect.bottom() - s),
            min,
            out,
        );
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

impl LotsStage {
    fn place_block(&mut self, ctx: &mut GenCtx, idx: usize) {
        let block = ctx.scratch.blocks[idx];
        let district = ctx.city.district(block.district).clone();
        let spec = district.theme.spec();
        let density = ctx.params().density;
        let rng = self.rng.as_mut().expect("begin sets rng");

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
            let plaza_p = spec.plaza * (1.3 - density);
            if chance(rng, plaza_p) {
                let parking = matches!(
                    district.theme,
                    crate::world::Theme::CorpCore | crate::world::Theme::Industrial
                ) && rng.random_bool(0.5);
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
            let kind = *pick_weighted(rng, spec.mix);
            let class = choose_class(rng, kind, spec);
            let basement = chance(rng, spec.basement);
            let mut footprint = lot;
            if matches!(spec.street_style, StreetStyle::Organic | StreetStyle::Strip)
                && chance(rng, 0.3)
            {
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
                district: district.id,
                footprint,
                span,
                name: None,
            });
            ctx.scratch.lots.push(Lot {
                rect: lot,
                district: district.id,
                use_: LotUse::Building {
                    kind,
                    class,
                    basement,
                },
            });
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
    use crate::world::{StratumKind, Theme};

    #[test]
    fn buildings_fit_blocks_and_do_not_overlap() {
        let c = small(41, WaterMode::Bay, 7);
        assert!(
            c.buildings.len() > 40,
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
}
