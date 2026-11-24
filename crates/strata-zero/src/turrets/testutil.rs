//! Turrets for tests.

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

/// The security at which the office copy lays a turret: the site-net start's 190 lays
/// none (checked every 5 up to 255), the top of the scale lays one.
pub(crate) const TURRET_SECURITY: u8 = 255;

/// The high-security office copy with every kept floor drawn, and the index of its first
/// turret foe.
pub(crate) fn turret_copy() -> (AppState, BuildingId, usize) {
    let (mut st, b) = crate::sitenet::testutil::office_copy(TURRET_SECURITY);
    let city = st.city.clone().expect("a city");
    let kept: Vec<i32> = st.interiors.plan_of(b).expect("a plan").levels().collect();
    for level in kept {
        st.interiors.ensure(&city, b, level);
    }
    let i = st
        .foes
        .iter()
        .position(|f| f.turret.is_some())
        .expect("a turret");
    (st, b, i)
}

/// Turret `i`'s node cracked from the site's Net, as `sitenet::testutil::loop_from_the_net`
/// does a camera's: your body left where you stand, you landed beside its node on its
/// level of the site's Net, the crack run to its landing. You stay jacked in.
pub(crate) fn hold_from_the_net(st: &mut AppState, i: usize) {
    let n = st.foes[i].turret.as_ref().expect("a turret").node;
    let (place, here) = st.player.as_ref().map(|p| (p.place, p.pos)).expect("you");
    crate::sitenet::ensure_construct(st, n.building).unwrap();
    let at = (st.constructs.site(n.building).unwrap().device(n))
        .expect("its node")
        .at;
    st.player.as_mut().unwrap().body = Some((place, here));
    let level = citygen::Place::SiteNet {
        building: n.building,
        level: n.level,
    };
    crate::constructs::land(st, level, at);
    crate::sitenet::testutil::walk_beside(st, at);
    st.net.beat = st.clock_tick();
    crate::net::tests::crack_to_landing(st, game::HackTarget::Site(n));
}
