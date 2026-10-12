//! Stage 2: district seeds, Voronoi regions, theme assignment, wealth and decay fields.

use super::{GenCtx, Stage, StageRunner, StepOut};
use crate::geom::{Pos, Rect};
use crate::ids::DistrictId;
use crate::params::GenError;
use crate::tile::Terrain;
use crate::world::{District, StratumKind, Theme};
use noise::{NoiseFn, OpenSimplex};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

#[derive(Default)]
pub(crate) struct DistrictsStage {
    seeds: Vec<Pos>,
    centroid: Pos,
    row: i32,
    finalized: bool,
    noise: Option<OpenSimplex>,
}

const FILLER_THEMES: [Theme; 4] = [
    Theme::Megablocks,
    Theme::NightMarket,
    Theme::NeonStrip,
    Theme::OldTown,
];

impl DistrictsStage {
    /// Poisson-style rejection sampling over land tiles; relaxes spacing until `n` seeds fit.
    fn place_seeds(rng: &mut ChaCha8Rng, land: &[Pos], n: usize) -> Vec<Pos> {
        let mut spacing = (land.len() as f32 / n as f32).sqrt() * 0.7;
        let mut seeds: Vec<Pos> = Vec::with_capacity(n);
        while seeds.len() < n {
            let mut attempts = 0;
            while seeds.len() < n && attempts < 4000 {
                attempts += 1;
                let cand = land[rng.random_range(0..land.len())];
                let ok = seeds.iter().all(|s| s.chebyshev(cand) as f32 >= spacing);
                if ok {
                    seeds.push(cand);
                }
            }
            spacing *= 0.8;
            if spacing < 1.0 && seeds.len() < n {
                // Degenerate map: fall back to any distinct land tiles.
                for p in land {
                    if seeds.len() == n {
                        break;
                    }
                    if !seeds.contains(p) {
                        seeds.push(*p);
                    }
                }
            }
        }
        seeds
    }

    fn nearest_seed(&self, p: Pos) -> DistrictId {
        let mut best = 0;
        let mut best_d = i64::MAX;
        for (i, s) in self.seeds.iter().enumerate() {
            let d = s.dist2(p);
            if d < best_d {
                best_d = d;
                best = i;
            }
        }
        DistrictId(best as u32)
    }

    fn assign_themes(&self, ctx: &GenCtx) -> Vec<Theme> {
        let n = self.seeds.len();
        let mut themes: Vec<Option<Theme>> = vec![None; n];
        let st = ctx
            .city
            .stratum(StratumKind::Street)
            .expect("street stratum");

        let core = (0..n)
            .min_by_key(|i| self.seeds[*i].dist2(self.centroid))
            .expect("at least one seed");
        themes[core] = Some(Theme::CorpCore);

        // Combat zone: farthest from the core.
        let combat = (0..n)
            .filter(|i| themes[*i].is_none())
            .max_by_key(|i| self.seeds[*i].dist2(self.seeds[core]))
            .expect("at least two seeds");
        themes[combat] = Some(Theme::CombatZone);

        // Docks: the remaining district with the most water-adjacent tiles.
        let mut water_adj = vec![0usize; n];
        for (p, t) in st.tiles.iter() {
            if let Some(d) = t.district {
                let touches = p
                    .neighbors4()
                    .any(|q| st.tiles.get(q).is_some_and(|u| u.terrain == Terrain::Water));
                if touches {
                    water_adj[d.index()] += 1;
                }
            }
        }
        let docks = (0..n)
            .filter(|i| themes[*i].is_none() && water_adj[*i] > 0)
            .max_by_key(|i| water_adj[*i]);
        if let Some(d) = docks {
            themes[d] = Some(Theme::Docks);
        }

        // Industrial: next to the docks, or nearest an edge when there is no water.
        let size = ctx.size();
        let edge_dist = |p: Pos| {
            p.x.min(p.y)
                .min(size.w as i32 - 1 - p.x)
                .min(size.h as i32 - 1 - p.y)
        };
        let industrial = (0..n)
            .filter(|i| themes[*i].is_none())
            .min_by_key(|i| match docks {
                Some(d) => self.seeds[*i].dist2(self.seeds[d]),
                None => edge_dist(self.seeds[*i]) as i64,
            });
        if let Some(i) = industrial {
            themes[i] = Some(Theme::Industrial);
        }

        let mut filler = FILLER_THEMES.iter().cycle();
        themes
            .into_iter()
            .map(|t| t.unwrap_or_else(|| *filler.next().expect("cycle never ends")))
            .collect()
    }

    fn finalize(&mut self, ctx: &mut GenCtx) {
        let n = self.seeds.len();
        let size = ctx.size();
        let noise = self.noise.as_ref().expect("begin sets noise");
        let params = ctx.params().clone();

        let mut bounds: Vec<Option<Rect>> = vec![None; n];
        {
            let st = ctx
                .city
                .stratum(StratumKind::Street)
                .expect("street stratum");
            for (p, t) in st.tiles.iter() {
                if let Some(d) = t.district {
                    let b = &mut bounds[d.index()];
                    *b = Some(match *b {
                        None => Rect::new(p.x, p.y, 1, 1),
                        Some(r) => {
                            let x0 = r.x.min(p.x);
                            let y0 = r.y.min(p.y);
                            let x1 = r.right().max(p.x + 1);
                            let y1 = r.bottom().max(p.y + 1);
                            Rect::new(x0, y0, x1 - x0, y1 - y0)
                        }
                    });
                }
            }
        }
        let themes = self.assign_themes(ctx);

        let clamp = |v: f32| v.clamp(0.0, 1.0);
        let mut districts = Vec::with_capacity(n);
        for i in 0..n {
            let theme = themes[i];
            let spec = theme.spec();
            let c = self.seeds[i];
            let nz = |salt: f64| noise.get([c.x as f64 * 0.01, c.y as f64 * 0.01, salt]) as f32;
            let wealth = clamp(spec.base_wealth + params.wealth_skew * 0.3 + nz(1.0) * 0.15);
            let decay = clamp(spec.base_decay + (params.decay - 0.4) * 0.5 + nz(2.0) * 0.15);
            districts.push(District {
                id: DistrictId(i as u32),
                name: String::new(),
                theme,
                center: c,
                wealth,
                decay,
                bounds: bounds[i].unwrap_or(Rect::new(c.x, c.y, 1, 1)),
            });
        }

        // Per-tile fields modulate the district value with fine noise.
        for y in 0..size.h as i32 {
            for x in 0..size.w as i32 {
                let p = Pos::new(x, y);
                let Some(d) = ctx
                    .city
                    .tile(StratumKind::Street, p)
                    .and_then(|t| t.district)
                else {
                    continue;
                };
                let dist = &districts[d.index()];
                let n1 = noise.get([x as f64 * 0.03, y as f64 * 0.03, 5.0]) as f32;
                let n2 = noise.get([x as f64 * 0.03, y as f64 * 0.03, 6.0]) as f32;
                ctx.scratch.wealth.set(p, clamp(dist.wealth + n1 * 0.1));
                ctx.scratch.decay.set(p, clamp(dist.decay + n2 * 0.1));
            }
        }
        ctx.city.districts = districts;
    }
}

impl StageRunner for DistrictsStage {
    fn begin(&mut self, ctx: &mut GenCtx) -> Result<(), GenError> {
        let mut rng = ctx.rng(Stage::Districts);
        self.noise = Some(OpenSimplex::new((ctx.seed ^ 0xD15) as u32));
        let land: Vec<Pos> = ctx
            .scratch
            .land
            .iter()
            .filter(|(_, l)| **l)
            .map(|(p, _)| p)
            .collect();
        if land.is_empty() {
            return Err(GenError::InvalidParams {
                field: "water",
                reason: "no land left to build on".into(),
            });
        }
        let (sx, sy) = land.iter().fold((0i64, 0i64), |(ax, ay), p| {
            (ax + p.x as i64, ay + p.y as i64)
        });
        self.centroid = Pos::new(
            (sx / land.len() as i64) as i32,
            (sy / land.len() as i64) as i32,
        );
        self.seeds = Self::place_seeds(&mut rng, &land, ctx.params().districts as usize);
        self.row = 0;
        self.finalized = false;
        Ok(())
    }

    fn step(&mut self, ctx: &mut GenCtx) -> Result<StepOut, GenError> {
        let size = ctx.size();
        let h = size.h as i32;
        if self.row < h {
            let y = self.row;
            for x in 0..size.w as i32 {
                let p = Pos::new(x, y);
                if !ctx.is_land(p) {
                    continue;
                }
                let d = self.nearest_seed(p);
                for k in StratumKind::PHYSICAL {
                    if let Some(t) = ctx.city.tile_mut(k, p) {
                        t.district = Some(d);
                    }
                }
            }
            self.row += 1;
            return Ok(StepOut::Continue(0.9 * self.row as f32 / h as f32));
        }
        if !self.finalized {
            self.finalize(ctx);
            self.finalized = true;
            return Ok(StepOut::Continue(1.0));
        }
        Ok(StepOut::Finished)
    }
}

#[cfg(test)]
mod tests {
    use crate::generate::testutil::small;
    use crate::params::WaterMode;
    use crate::tile::Terrain;
    use crate::world::{StratumKind, Theme};

    #[test]
    fn every_land_tile_has_a_district_and_counts_match() {
        let c = small(5, WaterMode::Bay, 6);
        assert_eq!(c.districts.len(), 6);
        let st = c.stratum(StratumKind::Street).unwrap();
        for (_, t) in st.tiles.iter() {
            if t.terrain != Terrain::Water {
                assert!(t.district.is_some());
            } else {
                assert!(t.district.is_none());
            }
        }
        let mut ids: Vec<_> = c.districts.iter().map(|d| d.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), 6);
        for (i, d) in c.districts.iter().enumerate() {
            assert_eq!(d.id.index(), i);
            assert!(d.bounds.contains(d.center));
            assert_eq!(st.tiles[d.center].district, Some(d.id));
        }
        // other strata carry the same district ids
        let sk = c.stratum(StratumKind::Skyways).unwrap();
        for (p, t) in st.tiles.iter() {
            assert_eq!(sk.tiles[p].district, t.district);
        }
    }

    #[test]
    fn theme_rules_hold() {
        let c = small(11, WaterMode::Bay, 7);
        let themes: Vec<_> = c.districts.iter().map(|d| d.theme).collect();
        assert_eq!(themes.iter().filter(|t| **t == Theme::CorpCore).count(), 1);
        assert_eq!(themes.iter().filter(|t| **t == Theme::Docks).count(), 1);
        assert!(themes.contains(&Theme::Industrial) && themes.contains(&Theme::CombatZone));
        let core = c
            .districts
            .iter()
            .find(|d| d.theme == Theme::CorpCore)
            .unwrap();
        let cz = c
            .districts
            .iter()
            .find(|d| d.theme == Theme::CombatZone)
            .unwrap();
        let far = c
            .districts
            .iter()
            .filter(|d| d.theme != Theme::CorpCore)
            .map(|d| d.center.dist2(core.center))
            .max()
            .unwrap();
        assert_eq!(cz.center.dist2(core.center), far);
    }

    #[test]
    fn no_water_means_no_docks_but_industrial_present() {
        let c = small(2, WaterMode::None, 5);
        assert!(c.districts.iter().all(|d| d.theme != Theme::Docks));
        assert!(c.districts.iter().any(|d| d.theme == Theme::Industrial));
    }

    #[test]
    fn wealth_and_decay_are_clamped() {
        let c = small(9, WaterMode::River, 8);
        for d in &c.districts {
            assert!((0.0..=1.0).contains(&d.wealth));
            assert!((0.0..=1.0).contains(&d.decay));
        }
    }
}
