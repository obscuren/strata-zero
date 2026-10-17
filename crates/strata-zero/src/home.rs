//! The player's flat: where every new game wakes up, and the rig it jacks in from.

use crate::constructs::{Constructs, city_gate};
use crate::interiors::Interiors;
use crate::render::map::MapRef;
use crate::walker::Player;
use citygen::{City, Home, Place, Pos, StratumKind};

/// The home with the Net tile its rig puts the player on: beside the megablock's gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HomeRig {
    pub home: Home,
    pub landing: Pos,
}

impl HomeRig {
    /// The city's home, if it has one and its megablock's gate has a walkable Net tile
    /// beside it, outside the footprint.
    pub fn find(city: &City) -> Option<HomeRig> {
        let home = citygen::home(city)?;
        let fp = city.building_opt(home.building)?.footprint;
        let gate = city_gate(city, home.building)?;
        let landing = gate
            .neighbors4()
            .find(|p| !fp.contains(*p) && city.walkable(StratumKind::Net, *p))?;
        Some(HomeRig { home, landing })
    }

    /// The player waking up beside the bed, the flat seen around them; the floor is
    /// generated if need be.
    pub fn wake(
        &self,
        city: &City,
        interiors: &mut Interiors,
        constructs: &Constructs,
        sight_bonus: i32,
    ) -> Option<Player> {
        let (building, level) = (self.home.building, self.home.level);
        interiors.ensure(city, building, level)?;
        let place = Place::Floor { building, level };
        let map = MapRef::for_place(city, interiors, constructs, place)?;
        let mut p = Player::at(place, self.home.bed_side);
        p.update_fov(&map, sight_bonus);
        Some(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{GenParams, Generator, SizePreset};

    #[test]
    fn the_home_rig_lands_beside_the_megablocks_gate() {
        let city = Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap();
        let rig = HomeRig::find(&city).expect("a home with a rig");
        let gate = city_gate(&city, rig.home.building).unwrap();
        let fp = city.building(rig.home.building).footprint;
        assert!(gate.neighbors4().any(|p| p == rig.landing));
        assert!(!fp.contains(rig.landing));
        assert!(city.walkable(StratumKind::Net, rig.landing));
    }
}
