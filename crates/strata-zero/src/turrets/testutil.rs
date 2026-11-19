//! Turrets for tests.

#![allow(dead_code)]

use super::{ARM_MID, Turret};
use crate::app::AppState;
use crate::guards::testutil::{WARD, on};
use citygen::{BuildingId, Dir, Feature, Pos, SiteNode};

/// A turret on the ward's floor `level` at `at`, facing `facing`: its tile blocks.
pub(crate) fn turret_in_ward(st: &mut AppState, level: i32, at: Pos, facing: Dir) -> usize {
    let mut f = st.interiors.floor(WARD, level).unwrap().1.clone();
    let mut tile = *f.tiles.get(at).unwrap();
    tile.feature = Some(Feature::Turret);
    f.tiles.set(at, tile);
    st.interiors.put_floor(WARD, f);
    let node = SiteNode {
        building: WARD,
        level,
        at,
    };
    st.foes.push(super::foe(
        on(level),
        at,
        Turret::new(node, facing, ARM_MID),
    ));
    st.foes.len() - 1
}

/// The lot of `foes::testutil::lot(at)` with its goon a turret there, you skilled with
/// handguns.
pub(crate) fn lot_turret(at: Pos, facing: Dir) -> AppState {
    let mut st = crate::foes::testutil::lot(at);
    let place = st.foes[0].place;
    let node = SiteNode {
        building: BuildingId(0),
        level: 0,
        at,
    };
    st.foes[0] = super::foe(place, at, Turret::new(node, facing, ARM_MID));
    st.character.skills.insert(game::SkillId::new("handguns"));
    st
}
