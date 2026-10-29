//! Ripperdoc clinics hide behind a front: the business the street sees. The front is a
//! function of the city's seed and the building, so its name and its floor agree
//! without storing anything.

use crate::ids::BuildingId;
use crate::rng::hash64;
use crate::world::{BuildingKind, City, StratumKind};
use serde::{Deserialize, Serialize};

/// What a clinic looks like from the street. Each has its own hand-laid floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Front {
    NoodleBar,
    /// The clinic is in its cellar.
    Laundromat,
    Dentist,
    Tattoo,
    Butcher,
}

impl Front {
    pub const ALL: [Front; 5] = [
        Front::NoodleBar,
        Front::Laundromat,
        Front::Dentist,
        Front::Tattoo,
        Front::Butcher,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Front::NoodleBar => "noodle bar",
            Front::Laundromat => "laundromat",
            Front::Dentist => "dentist",
            Front::Tattoo => "tattoo parlour",
            Front::Butcher => "butcher",
        }
    }

    /// Its floors: the laundromat's street floor and the cellar under it, else one.
    pub fn floors(self) -> usize {
        match self {
            Front::Laundromat => 2,
            _ => 1,
        }
    }
}

/// The front for a building: a basement makes it the laundromat, else one of the five
/// by the seed.
fn front_for(seed: u64, id: BuildingId, basement: bool) -> Front {
    if basement {
        return Front::Laundromat;
    }
    let n = Front::ALL.len() as u64;
    Front::ALL[(hash64(seed, 0xC11C ^ id.index() as u64) % n) as usize]
}

/// A clinic's front; `None` for every other building.
pub fn clinic_front(city: &City, id: BuildingId) -> Option<Front> {
    let b = city.building_opt(id)?;
    (b.kind == BuildingKind::Clinic)
        .then(|| front_for(city.seed, id, b.span.lo == StratumKind::Undercity))
}

/// What the game calls a building's kind to the player: a clinic is its front.
pub fn kind_label(city: &City, id: BuildingId) -> &'static str {
    match clinic_front(city, id) {
        Some(front) => front.label(),
        None => city
            .building_opt(id)
            .map_or("building", |b| b.kind.label()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GenParams, Generator, SizePreset};

    fn medium(seed: u64) -> City {
        Generator::run(GenParams {
            seed,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn every_clinic_has_a_front_and_nothing_else_does() {
        for c in [medium(1), medium(8)] {
            for b in &c.buildings {
                assert_eq!(
                    clinic_front(&c, b.id).is_some(),
                    b.kind == BuildingKind::Clinic,
                    "{:?}",
                    b.kind
                );
            }
        }
    }

    #[test]
    fn a_basement_clinic_is_a_laundromat() {
        let mut c = medium(8);
        let b = c
            .buildings
            .iter()
            .find(|b| b.kind == BuildingKind::Clinic)
            .unwrap()
            .id;
        c.buildings[b.index()].span.lo = StratumKind::Undercity;
        assert_eq!(clinic_front(&c, b), Some(Front::Laundromat));
    }

    #[test]
    fn the_front_is_stable() {
        let (a, b) = (medium(11), medium(11));
        for x in a.buildings.iter().filter(|x| x.kind == BuildingKind::Clinic) {
            assert_eq!(clinic_front(&a, x.id), clinic_front(&a, x.id));
            assert_eq!(clinic_front(&a, x.id), clinic_front(&b, x.id));
        }
    }

    #[test]
    fn fronts_cover_all_five() {
        let seen: std::collections::HashSet<Front> =
            (0..400).map(|i| front_for(8, BuildingId(i), false)).collect();
        assert_eq!(seen.len(), Front::ALL.len());
    }

    #[test]
    fn kind_label_names_the_front() {
        let c = medium(2);
        for b in &c.buildings {
            let want = match clinic_front(&c, b.id) {
                Some(f) => f.label(),
                None => b.kind.label(),
            };
            assert_eq!(kind_label(&c, b.id), want);
        }
        assert!(
            c.buildings
                .iter()
                .any(|b| b.kind == BuildingKind::Clinic && kind_label(&c, b.id) != "clinic")
        );
    }
}
