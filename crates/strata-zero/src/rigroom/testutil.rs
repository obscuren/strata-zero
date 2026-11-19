//! The tests' rig at home: seed 8's flat, you beside the rig, and the room jacked into.

use crate::app::AppState;
use citygen::{BuildingId, GenParams, Generator, SizePreset};

/// Seed 8, medium, seven districts: you woken in the flat and stepped beside the rig.
pub(crate) fn at_the_rig() -> AppState {
    let mut st = AppState::new();
    st.enter_city(
        Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap(),
    );
    let rig = st.home.expect("a home");
    let city = st.city.as_ref().unwrap();
    let mut p = (rig.wake(city, &mut st.interiors, &st.constructs, 0)).expect("the flat");
    let (_, floor) = (st.interiors.floor(rig.home.building, rig.home.level)).unwrap();
    p.pos = (rig.home.rig.neighbors8())
        .filter(|q| rig.home.rect.contains(*q) && floor.tiles[*q].is_walkable())
        .min_by_key(|q| q.chebyshev(rig.home.bed_side))
        .expect("a tile beside the rig");
    st.player = Some(p);
    st
}

/// A Net backdoor held on the `n`th plain building (not a site) with a gate on the
/// grid, not the home's.
pub(crate) fn hold_a_backdoor(st: &mut AppState, n: usize) -> BuildingId {
    let city = st.city.as_ref().unwrap();
    let home = st.home.unwrap().home.building;
    let b = (city.buildings.iter().map(|b| b.id))
        .filter(|b| *b != home && crate::constructs::city_gate(city, *b).is_some())
        .filter(|b| !crate::sitenet::is_site(st, *b))
        .nth(n)
        .expect("a building with a gate");
    let seed = city.seed;
    st.backdoors.plant_net(b, 1, seed, 0, 3);
    b
}

/// At the rig and jacked in: in the room, on the tile you land on.
pub(crate) fn in_the_room() -> AppState {
    let mut st = at_the_rig();
    crate::net::jack_in_here(&mut st).expect("jacked in");
    st
}
