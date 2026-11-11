use super::*;
use citygen::venue::{MissionSize, VenuePack};

/// New games' cities, as NEW GAME (DEBUG) builds them, from fixed seeds.
fn new_game_cities() -> Vec<citygen::City> {
    let params = AppState::new().params;
    [11, 12]
        .into_iter()
        .map(|seed| crate::screens::build_home(params.clone().with_seed(seed)).unwrap())
        .collect()
}

#[test]
fn an_open_building_and_a_held_one_stay_as_they_are() {
    let mut st = AppState::new();
    st.enter_city(new_game_cities().remove(0));
    let city = st.city.clone().unwrap();
    let id = &VenuePack::builtin().types[0].id;
    let inputs = MissionInputs {
        size: MissionSize::Small,
        objective: 0,
        security: 128,
        seed: 0,
    };
    let open = (city.buildings.iter())
        .find(|x| !x.kind.is_closed())
        .unwrap()
        .id;
    assert_eq!(
        dev_site(&mut st, open, id, inputs).unwrap_err(),
        "an open building has no floors"
    );
    assert_eq!(
        dev_site(&mut st, open, "nowhere", inputs).unwrap_err(),
        "no site type nowhere"
    );
    // A building a job holds keeps its plan.
    let b = dev_start(&mut st, id, MissionSize::Small, true).unwrap();
    let plan = st.interiors.plan_of(b).cloned();
    assert_eq!(
        dev_site(&mut st, b, id, inputs).unwrap_err(),
        "a job is already on there"
    );
    assert_eq!(st.interiors.plan_of(b).cloned(), plan);
}

#[test]
fn a_dev_start_finds_a_site_for_every_type_and_size() {
    for city in new_game_cities() {
        for t in &VenuePack::builtin().types {
            for size in MissionSize::ALL {
                let mut st = AppState::new();
                st.enter_city(city.clone());
                let b = dev_start(&mut st, &t.id, size, true)
                    .unwrap_or_else(|e| panic!("{} {}: {e}", t.id, size.label()));
                let v = st.interiors.plan_of(b).unwrap().venue.as_deref().unwrap();
                assert_eq!(v.type_id, t.id);
                assert_ne!(Some(b), st.home.map(|h| h.home.building), "away from home");
                let copy = st.interiors.copy(b).unwrap();
                assert!(copy.live && copy.inputs.size == size);
                let p = st.player.as_ref().unwrap();
                let at = game::At {
                    place: p.place,
                    pos: p.pos,
                };
                assert_eq!(at.place, Place::Stratum(StratumKind::Street));
                assert_eq!(building_at_door(&st, at), Some(b), "at its door");
            }
        }
    }
}

#[test]
fn a_mission_that_wont_lay_tries_other_seeds_and_objectives() {
    let city = new_game_cities().remove(0);
    let pack = VenuePack::builtin();
    let mut st = AppState::new();
    st.enter_city(city);
    let t = &pack.types[0];
    let b = dev_start(&mut st, &t.id, MissionSize::Small, false).unwrap();
    let plan = st.interiors.plan_of(b).unwrap();
    let layout = plan.venue.as_deref().unwrap().clone();
    let rule = pack.size(MissionSize::Small);
    let lays = |i: MissionInputs| MissionLayer::lay(&layout, t, rule, i).is_ok();
    // An objective no lay finds: past the type's last.
    let asked = MissionInputs {
        size: MissionSize::Small,
        objective: t.objectives.len(),
        security: 128,
        seed: 3,
    };
    let found = laid_inputs(&layout, t, asked).expect("another objective lays");
    assert!(found.objective < t.objectives.len() && lays(found));
    // Inputs that lay come back as they are.
    assert_eq!(laid_inputs(&layout, t, found), Some(found));
}

#[test]
fn a_site_net_start_keeps_to_the_server_room() {
    let mut st = AppState::new();
    st.enter_city(new_game_cities().remove(0));
    let b = dev_site_net(&mut st, crate::venues::SITE_NET_SECURITY).unwrap();
    let t = (VenuePack::builtin().for_kind(citygen::BuildingKind::Office)).unwrap();
    let v = st.interiors.plan_of(b).unwrap().venue.as_deref().unwrap();
    assert_eq!(v.type_id, t.id, "the office type");
    let copy = st.interiors.copy(b).unwrap();
    assert!(copy.live && copy.inputs.size == MissionSize::Medium);
    assert_eq!(copy.inputs.security, crate::venues::SITE_NET_SECURITY);
    let server = |o: &citygen::venue::ObjectiveDef| {
        t.room(&o.room)
            .is_some_and(|r| r.kind == citygen::RoomKind::Server)
    };
    let first = t.objectives.iter().position(server);
    assert_eq!(Some(copy.inputs.objective), first, "its first server room");
    let p = st.player.as_ref().unwrap();
    let at = game::At {
        place: p.place,
        pos: p.pos,
    };
    assert_eq!(at.place, Place::Stratum(StratumKind::Street));
    assert_eq!(building_at_door(&st, at), Some(b), "at its door");
}
