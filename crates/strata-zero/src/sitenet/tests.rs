use super::testutil::*;
use super::*;
use citygen::{NodeKind, Place, Pos, StratumKind};
use game::{HackTarget, Op};

#[test]
fn a_sites_security_is_its_live_copys_then_its_subnets() {
    let (st, b) = office_copy(190);
    assert!(is_site(&st, b));
    assert_eq!(security(&st, b), 190);
    assert_eq!(
        crate::owners::security(&st, crate::owners::Owner::Building(b)),
        190
    );
    let (st, b) = office_plain();
    assert_eq!(
        security(&st, b),
        crate::render::netlook::security_of(st.city.as_ref().unwrap(), b).unwrap()
    );
}

#[test]
fn a_building_not_planned_yet_is_a_site_when_its_layout_lays() {
    let st = crate::jobs::testutil::state_home();
    let city = st.city.as_ref().unwrap();
    let pack = citygen::venue::VenuePack::builtin();
    let mut sites = 0;
    let unplanned = (city.buildings.iter())
        .filter(|x| x.kind.is_closed() && st.interiors.plan_of(x.id).is_none());
    for x in unplanned {
        let lays = pack.for_kind(x.kind).is_some()
            && citygen::FloorPlan::new(city, x.id).is_some_and(|p| p.venue.is_some());
        assert_eq!(is_site(&st, x.id), lays, "{:?} {:?}", x.kind, x.id);
        assert_eq!(is_site(&st, x.id), lays, "asked again");
        assert!(st.interiors.plan_of(x.id).is_none(), "asking plans nothing");
        sites += usize::from(lays);
    }
    assert!(sites > 0, "a hotel or an office in the city");
}

#[test]
fn a_classic_plan_is_no_site() {
    let (mut st, b) = office_plain();
    let city = st.city.clone().unwrap();
    st.interiors
        .put_plan(citygen::FloorPlan::classic(&city, b).unwrap());
    assert!(!is_site(&st, b));
    assert!(net_of(&st, b).is_none());
    assert!(ensure_construct(&mut st, b).is_err());
}

#[test]
fn the_construct_is_made_once_over_an_old_one() {
    let (mut st, b) = office_copy(210);
    let city = st.city.clone().unwrap();
    // Today's construct, as an old save holds it.
    assert!(st.constructs.ensure(&city, b).is_some_and(|k| !k.is_site()));
    st.net.ice.insert(b, vec![]);
    let now = st.net.now(&st);
    st.net.land(
        b,
        HackTarget::Guard {
            building: b,
            index: 0,
        },
        game::Target::Guard(citygen::IceGrade::White),
        Op::Crack,
        now,
    );
    ensure_construct(&mut st, b).unwrap();
    assert!(st.constructs.site(b).is_some());
    assert!(!st.net.ice.contains_key(&b) && !st.net.hacks.iter().any(|h| h.building == b));
    let made = st.constructs.site(b).unwrap().clone();
    ensure_construct(&mut st, b).unwrap();
    assert_eq!(st.constructs.site(b), Some(&made), "made once");
}

#[test]
fn a_copy_beginning_or_ending_drops_the_sites_net() {
    let (mut st, b) = office_copy(190);
    ensure_construct(&mut st, b).unwrap();
    let door = st
        .constructs
        .site(b)
        .unwrap()
        .devices
        .iter()
        .find(|d| d.kind == NodeKind::DoorLock)
        .unwrap()
        .node;
    hack(&mut st, door, NodeKind::DoorLock, Op::Crack);
    crate::venues::end(&mut st, b).unwrap();
    assert!(st.constructs.get(b).is_none());
    assert!(
        !st.net
            .hacks
            .iter()
            .any(|h| h.target == HackTarget::Site(door))
    );
}

#[test]
fn a_copy_waits_while_you_are_on_its_net() {
    let (mut st, b) = office_copy(190);
    ensure_construct(&mut st, b).unwrap();
    // On a floor of it, jacked in: the body there, your Net self on the grid.
    let floor_pos = st.interiors.plan_of(b).unwrap().anchor(0);
    let mut p = crate::walker::Player::at(Place::Stratum(StratumKind::Net), Pos::new(0, 0));
    p.body = Some((
        Place::Floor {
            building: b,
            level: 0,
        },
        floor_pos,
    ));
    st.player = Some(p);
    crate::venues::end(&mut st, b).unwrap();
    assert!(st.interiors.copy(b).is_some(), "it waits for the body");
    assert!(
        st.constructs.site(b).is_some(),
        "nothing is dropped under you"
    );
}
