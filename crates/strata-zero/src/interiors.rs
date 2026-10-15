//! The interiors this session has walked into: plans and floors, generated on the
//! first visit and kept for the session. Never saved; the seed brings them back.

use citygen::{BuildingId, City, Floor, FloorPlan, generate_floor};
use std::collections::HashMap;
use std::collections::hash_map::Entry;

// The doors that walk into these floors come with the next task; until then only the
// renderer reads what is here.
#[allow(dead_code)]
#[derive(Default)]
pub struct Interiors {
    plans: HashMap<BuildingId, FloorPlan>,
    floors: HashMap<(BuildingId, i32), Floor>,
}

#[allow(dead_code)]
impl Interiors {
    /// The plan of a closed building, made on first ask.
    pub fn plan(&mut self, city: &City, b: BuildingId) -> Option<&FloorPlan> {
        match self.plans.entry(b) {
            Entry::Occupied(e) => Some(e.into_mut()),
            Entry::Vacant(e) => Some(e.insert(FloorPlan::new(city, b)?)),
        }
    }

    pub fn plan_of(&self, b: BuildingId) -> Option<&FloorPlan> {
        self.plans.get(&b)
    }

    /// A floor, generated if this is its first visit. `None` for an open building or
    /// a level the plan does not have.
    pub fn ensure(&mut self, city: &City, b: BuildingId, level: i32) -> Option<&Floor> {
        let plan = self.plan(city, b)?.clone();
        if level < plan.lowest || level > plan.top {
            return None;
        }
        Some(
            self.floors
                .entry((b, level))
                .or_insert_with(|| generate_floor(city, &plan, level)),
        )
    }

    /// A floor already generated, with its plan.
    pub fn floor(&self, b: BuildingId, level: i32) -> Option<(&FloorPlan, &Floor)> {
        Some((self.plans.get(&b)?, self.floors.get(&(b, level))?))
    }

    pub fn generated(&self) -> usize {
        self.floors.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{BuildingKind, GenParams, Generator, SizePreset};

    #[test]
    fn floors_are_made_once_and_only_inside_the_plan() {
        let c = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 8,
            ..Default::default()
        })
        .unwrap();
        let tower = c
            .buildings
            .iter()
            .find(|b| b.kind == BuildingKind::CorpTower)
            .unwrap();
        let shop = c.buildings.iter().find(|b| !b.kind.is_closed()).unwrap();
        let mut i = Interiors::default();
        assert!(i.plan(&c, shop.id).is_none());
        assert!(i.ensure(&c, shop.id, 0).is_none());
        let top = i.plan(&c, tower.id).unwrap().top;
        assert!(i.ensure(&c, tower.id, 0).is_some());
        assert!(i.ensure(&c, tower.id, 0).is_some());
        assert_eq!(i.generated(), 1);
        assert!(i.ensure(&c, tower.id, top + 1).is_none());
        assert!(i.ensure(&c, tower.id, top).is_some());
        assert_eq!(i.generated(), 2);
        assert!(i.floor(tower.id, 1).is_none());
        assert!(i.floor(tower.id, top).is_some());
    }
}
