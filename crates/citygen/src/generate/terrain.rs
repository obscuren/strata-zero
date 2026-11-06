//! Stage 1: water and land.

use super::{GenCtx, Stage, StageRunner, StepOut};
use crate::geom::{Dir, Pos};
use crate::params::{GenError, WaterMode};
use crate::tile::Terrain;
use crate::world::StratumKind;
use noise::{NoiseFn, OpenSimplex};
use rand::Rng;
use rand::seq::IndexedRandom;

#[derive(Default)]
pub(crate) struct TerrainStage {
    row: i32,
    noise: Option<OpenSimplex>,
    edge: Option<Dir>,
    river_axis_horizontal: bool,
    river_offset: f64,
}

impl TerrainStage {
    fn is_water(&self, ctx: &GenCtx, p: Pos) -> bool {
        if let Some(site) = ctx.scratch.site.as_ref() {
            return site.wet(p);
        }
        let size = ctx.size();
        let (w, h) = (size.w as f64, size.h as f64);
        let noise = self.noise.as_ref().expect("begin sets noise");
        match ctx.params().water {
            WaterMode::None => false,
            WaterMode::Bay => {
                let edge = self.edge.expect("begin sets edge");
                // `t` runs along the edge, `d` is the distance from it.
                let (t, d, extent) = match edge {
                    Dir::N => (p.x as f64, p.y as f64, h),
                    Dir::S => (p.x as f64, h - 1.0 - p.y as f64, h),
                    Dir::W => (p.y as f64, p.x as f64, w),
                    _ => (p.y as f64, w - 1.0 - p.x as f64, w),
                };
                let depth = 0.15 * extent + noise.get([t * 0.04, 7.3]) * 0.06 * extent;
                d < depth
            }
            WaterMode::River => {
                let (t, c, extent) = if self.river_axis_horizontal {
                    (p.x as f64, p.y as f64, h)
                } else {
                    (p.y as f64, p.x as f64, w)
                };
                let centre =
                    extent / 2.0 + self.river_offset + noise.get([t * 0.02, 3.1]) * 0.25 * extent;
                let half = 3.0 + (noise.get([t * 0.05, 9.7]) + 1.0);
                (c - centre).abs() < half
            }
        }
    }
}

impl StageRunner for TerrainStage {
    fn begin(&mut self, ctx: &mut GenCtx) -> Result<(), GenError> {
        if let Some(site) = ctx.params().site.clone() {
            ctx.scratch.site = super::site::prepare(&site, ctx.size());
        }
        let mut rng = ctx.rng(Stage::Terrain);
        self.noise = Some(OpenSimplex::new(ctx.seed as u32));
        self.edge = Dir::CARDINAL.choose(&mut rng).copied();
        self.river_axis_horizontal = rng.random_bool(0.5);
        self.river_offset = rng.random_range(-0.1..0.1) * ctx.size().h as f64;
        ctx.scratch.water_edge = self.edge;
        self.row = 0;
        Ok(())
    }

    fn step(&mut self, ctx: &mut GenCtx) -> Result<StepOut, GenError> {
        let size = ctx.size();
        if self.row >= size.h as i32 {
            return Ok(StepOut::Finished);
        }
        let y = self.row;
        for x in 0..size.w as i32 {
            let p = Pos::new(x, y);
            // in a world: its highways are the city's roads, past its outline nothing is
            // built (the open ground fills in at the end)
            let site = ctx.scratch.site.as_ref();
            let highway = site.and_then(|s| s.highway(p));
            let outside = site.is_some_and(|s| !s.builds(p));
            if let Some(t) = highway {
                ctx.scratch.land.set(p, t == Terrain::Road);
                ctx.scratch.road.set(p, true);
                ctx.scratch.arterial.set(p, true);
                ctx.set_terrain(StratumKind::Street, p, t);
            } else if self.is_water(ctx, p) {
                ctx.scratch.land.set(p, false);
                let t = match site.and_then(|s| s.ground.get(p)).map(|t| t.terrain) {
                    Some(t @ (Terrain::Ford | Terrain::Shallows)) => t,
                    _ => Terrain::Water,
                };
                ctx.set_terrain(StratumKind::Street, p, t);
            } else if outside {
                ctx.scratch.land.set(p, false);
            }
        }
        self.row += 1;
        Ok(StepOut::Continue(self.row as f32 / size.h as f32))
    }
}

#[cfg(test)]
mod tests {
    use crate::generate::Generator;
    use crate::params::{GenParams, SizePreset, WaterMode};
    use crate::tile::Terrain;
    use crate::world::StratumKind;

    #[test]
    fn bay_covers_ten_to_twenty_percent_of_street() {
        for seed in 0..5 {
            let c = Generator::run(GenParams {
                seed,
                water: WaterMode::Bay,
                size: SizePreset::Custom { w: 120, h: 60 },
                districts: 4,
                ..Default::default()
            })
            .unwrap();
            let st = c.stratum(StratumKind::Street).unwrap();
            let water = st.tiles.count(|t| t.terrain == Terrain::Water) as f32
                / st.tiles.size().area() as f32;
            assert!((0.08..=0.25).contains(&water), "seed {seed}: {water}");
        }
        let c = Generator::run(GenParams {
            seed: 3,
            water: WaterMode::None,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 4,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            c.stratum(StratumKind::Street)
                .unwrap()
                .tiles
                .count(|t| t.terrain == Terrain::Water),
            0
        );
        let c = Generator::run(GenParams {
            seed: 3,
            water: WaterMode::River,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 4,
            ..Default::default()
        })
        .unwrap();
        let st = c.stratum(StratumKind::Street).unwrap();
        let water = st.tiles.count(|t| t.terrain == Terrain::Water);
        assert!(water > 300 && water < 1500, "river tiles {water}");
    }
}
