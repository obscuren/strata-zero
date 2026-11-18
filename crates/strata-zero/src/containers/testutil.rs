//! Sites with a container of a kind to test on, and standing beside one.

use super::{Container, ContainerAt, find};
use crate::app::AppState;
use citygen::tile::ContainerKind;
use citygen::venue::{MissionInputs, MissionSize, VenuePack};
use citygen::{BuildingId, BuildingKind, Place, Pos};

/// A plain (no job) hotel or office laid on a building away from home at `seed`.
pub(crate) fn plain_site(kind: BuildingKind, seed: u64) -> (AppState, BuildingId) {
    let mut st = crate::sitenet::testutil::runner_home();
    let t = VenuePack::builtin()
        .for_kind(kind)
        .expect("a type builds it");
    let inputs = MissionInputs {
        size: MissionSize::Medium,
        objective: 0,
        security: 128,
        seed,
    };
    let b = crate::venues::dev_start_with(&mut st, &t.id, inputs, false).unwrap();
    (st, b)
}

/// Every container on every kept level of building `b`, its floors drawn.
pub(crate) fn all_in(st: &mut AppState, b: BuildingId) -> Vec<Container> {
    let city = st.city.clone().expect("a city");
    let levels: Vec<i32> = st
        .interiors
        .plan(&city, b)
        .map(|p| p.levels().collect())
        .unwrap_or_default();
    let mut found = Vec::new();
    for level in levels {
        let Some(floor) = st.interiors.ensure(&city, b, level) else {
            continue;
        };
        let spots: Vec<Pos> = (floor.tiles.iter())
            .filter(|(_, t)| t.feature.is_some_and(|f| f.container().is_some()))
            .map(|(p, _)| p)
            .collect();
        for at in spots {
            let at = ContainerAt {
                building: b,
                level,
                at,
            };
            found.extend(find(st, at));
        }
    }
    found
}

/// A plain site (an office or a hotel, at the first seed that has one) with a container of
/// `kind` on a kept level, its floor drawn.
pub(crate) fn plain_site_with(kind: ContainerKind) -> (AppState, Container) {
    for seed in 0..12 {
        for what in [BuildingKind::Office, BuildingKind::Hotel] {
            let (mut st, b) = plain_site(what, seed);
            if let Some(c) = all_in(&mut st, b).into_iter().find(|c| c.kind == kind) {
                return (st, c);
            }
        }
    }
    panic!("no site with a {kind:?}");
}

/// The DEVELOPER start's office copy at `security`, and its first layer spot.
pub(crate) fn copy_with_spot(security: u8) -> (AppState, Container) {
    let (mut st, b) = crate::sitenet::testutil::office_copy(security);
    let spot = *crate::sitenet::layer_of(&st, b)
        .and_then(|l| l.containers.first())
        .expect("a layer spot");
    let city = st.city.clone().unwrap();
    st.interiors
        .ensure(&city, b, spot.level)
        .expect("its floor");
    let c = find(
        &st,
        ContainerAt {
            building: b,
            level: spot.level,
            at: spot.at,
        },
    )
    .expect("a container there");
    (st, c)
}

/// Stands you on its front, or a walkable tile beside it.
pub(crate) fn beside(st: &mut AppState, c: &Container) {
    let (b, level) = (c.at.building, c.at.level);
    let city = st.city.clone().unwrap();
    st.interiors.ensure(&city, b, level).expect("its floor");
    let (_, floor) = st.interiors.floor(b, level).expect("its floor");
    let at = c.front.or_else(|| {
        c.at.at
            .neighbors4()
            .find(|p| floor.tiles.get(*p).is_some_and(|t| t.is_walkable()))
    });
    let p = st.player.as_mut().expect("you");
    p.place = Place::Floor { building: b, level };
    p.pos = at.expect("a tile beside it");
}
