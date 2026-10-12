//! Stage 10: names for the city, districts and notable buildings.

use super::{GenCtx, Stage, StageRunner, StepOut};
use crate::params::GenError;
use rand_chacha::ChaCha8Rng;

#[derive(Default)]
pub(crate) struct NamingStage {
    next: usize,
    rng: Option<ChaCha8Rng>,
}

impl StageRunner for NamingStage {
    fn begin(&mut self, ctx: &mut GenCtx) -> Result<(), GenError> {
        let mut rng = ctx.rng(Stage::Naming);
        ctx.city.name = ctx.flavor.city_name(&mut rng);
        self.rng = Some(rng);
        self.next = 0;
        Ok(())
    }

    fn step(&mut self, ctx: &mut GenCtx) -> Result<StepOut, GenError> {
        let n = ctx.city.districts.len();
        if self.next >= n {
            return Ok(StepOut::Finished);
        }
        let rng = self.rng.as_mut().expect("begin sets rng");
        let d = self.next;
        let theme = ctx.city.districts[d].theme;
        let district_name = ctx.flavor.district_name(theme, rng);
        ctx.city.districts[d].name = district_name.clone();

        let id = ctx.city.districts[d].id;
        let mut corp_names: Vec<String> = Vec::new();
        for i in 0..ctx.city.buildings.len() {
            if ctx.city.buildings[i].district != id {
                continue;
            }
            let kind = ctx.city.buildings[i].kind;
            let name = if kind.is_corp() {
                let corp = ctx.flavor.corp_name(rng);
                corp_names.push(corp.clone());
                Some(corp)
            } else {
                ctx.flavor.shop_name(kind, rng)
            };
            ctx.city.buildings[i].name = name;
        }

        // Two landmarks per district: the largest footprints get grand names.
        let mut candidates: Vec<usize> = (0..ctx.city.buildings.len())
            .filter(|i| ctx.city.buildings[*i].district == id)
            .collect();
        candidates.sort_by_key(|i| std::cmp::Reverse(ctx.city.buildings[*i].footprint.area()));
        for i in candidates.into_iter().take(2) {
            let corp = corp_names
                .first()
                .cloned()
                .unwrap_or_else(|| ctx.flavor.corp_name(rng));
            let name = ctx.flavor.landmark_name(&corp, &district_name, rng);
            ctx.city.buildings[i].name = Some(name);
        }

        self.next += 1;
        Ok(StepOut::Continue(self.next as f32 / n as f32))
    }
}

#[cfg(test)]
mod tests {
    use crate::generate::testutil::small;
    use crate::params::WaterMode;

    #[test]
    fn names_are_filled_and_deterministic() {
        let a = small(91, WaterMode::Bay, 7);
        let b = small(91, WaterMode::Bay, 7);
        assert!(!a.name.is_empty());
        assert_eq!(a.name, b.name);
        assert!(a.districts.iter().all(|d| !d.name.is_empty()));
        let named = a.buildings.iter().filter(|b| b.name.is_some()).count();
        assert!(named >= a.buildings.iter().filter(|b| b.kind.is_corp()).count());
        let landmarks = a
            .buildings
            .iter()
            .filter(|b| {
                b.name.as_deref().is_some_and(|n| {
                    n.contains("Tower")
                        || n.contains("Terminal")
                        || n.contains("Arcology")
                        || n.contains("Plaza")
                        || n.contains("Station")
                })
            })
            .count();
        assert!(landmarks >= a.districts.len(), "landmarks {landmarks}");
    }
}
