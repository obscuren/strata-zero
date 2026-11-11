use super::testutil::*;
use super::*;
use crate::render::map::MapRef;
use citygen::{NodeKind, Place, Pos, StratumKind, Terrain};
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

#[test]
fn a_site_place_maps_to_its_construct_and_scene() {
    let (mut st, b) = office_copy(190);
    on_hub(&mut st, b, 0);
    let place = st.player.as_ref().unwrap().place;
    assert_eq!(
        place,
        Place::SiteNet {
            building: b,
            level: 0
        }
    );
    assert_eq!(
        crate::net::scene_of(place),
        Some(crate::net::Scene::Inside(b))
    );
    let city = st.city.as_ref().unwrap();
    let map = MapRef::for_place(city, &st.interiors, &st.constructs, place, 0).unwrap();
    assert_eq!(map.place(), place);
    assert!(st.player.as_ref().unwrap().sees_all());
}

#[test]
fn ice_on_a_sites_floor_sees_you_in_line() {
    let (mut st, b) = office_copy(210);
    on_hub(&mut st, b, 0);
    let k = st.constructs.site(b).unwrap();
    let g = (k.guards.iter())
        .find(|g| k.level_at(g.pos).unwrap().level == 0)
        .unwrap();
    let open = [(1, 0), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .map(|(dx, dy)| g.pos.offset(dx, dy))
        .find(|q| k.tiles[*q].terrain == Terrain::NetFloor && k.tiles[*q].feature.is_none())
        .expect("an open tile beside the piece");
    assert!(crate::net::sees(k, g.pos, g.grade, open, false));
    let walled = (k.tiles.iter())
        .find(|(q, t)| {
            t.terrain == Terrain::NetFloor
                && q.y == g.pos.y
                && !crate::net::line_clear_in(k, g.pos, *q)
        })
        .map(|(q, _)| q);
    if let Some(q) = walled {
        assert!(!crate::net::sees(k, g.pos, g.grade, q, false));
    }
}

#[test]
fn only_your_levels_ice_takes_turns() {
    let (mut st, b) = office_copy(210);
    on_hub(&mut st, b, 0);
    let place = st.player.as_ref().unwrap().place;
    let k = st.constructs.site(b).unwrap();
    for a in crate::net::initiative(&st, place) {
        if let crate::net::Actor::Ice(i) = a {
            assert_eq!(k.level_at(k.guards[i].pos).unwrap().level, 0);
        }
    }
}

#[test]
fn a_copy_waits_while_you_stand_on_its_net() {
    let (mut st, b) = office_copy(190);
    on_hub(&mut st, b, 0);
    crate::venues::end(&mut st, b).unwrap();
    assert!(st.interiors.copy(b).is_some() && st.constructs.site(b).is_some());
}

#[test]
fn forgetting_a_sites_net_drops_a_run_on_its_node() {
    let (mut st, b) = office_copy(190);
    ensure_construct(&mut st, b).unwrap();
    let door = (st.constructs.site(b).unwrap().devices.iter())
        .find(|d| d.kind == NodeKind::DoorLock)
        .unwrap()
        .node;
    let run = |target| game::Run {
        program: game::ItemId::new("crack"),
        target,
        done: 1,
        work: 3,
    };
    st.net.run = Some(run(HackTarget::Deck));
    forget(&mut st, b);
    assert_eq!(st.net.run, Some(run(HackTarget::Deck)), "not the site's");
    st.net.run = Some(run(HackTarget::Site(door)));
    forget(&mut st, b);
    assert_eq!(st.net.run, None);
}

#[test]
fn a_lost_copy_leaves_the_net_you_stand_on() {
    let (mut st, b) = office_copy(190);
    on_hub(&mut st, b, 0);
    let made = st.constructs.site(b).unwrap().clone();
    // As a save from before copies kept their layer reads.
    st.interiors.copy_mut(b).unwrap().layer = None;
    crate::venues::tick(&mut st);
    assert!(st.interiors.copy(b).is_none(), "lost at once");
    assert_eq!(
        st.constructs.site(b),
        Some(&made),
        "nothing dropped under you"
    );
    let place = st.player.as_ref().unwrap().place;
    let city = st.city.as_ref().unwrap();
    assert!(MapRef::for_place(city, &st.interiors, &st.constructs, place, 0).is_some());
}

#[test]
fn arc_trace_ping_and_the_ice_lines_keep_to_your_level() {
    let (mut st, b) = office_copy(210);
    on_hub(&mut st, b, 0);
    let k = st.constructs.site(b).unwrap().clone();
    let level = |p: Pos| k.level_at(p).unwrap().level;
    st.net.ice_of(&k);
    let fire = |st: &mut crate::app::AppState, id: &str| {
        crate::actions::perform(st, game::Action::Fire(game::AbilityId::new(id)))
    };
    // Arc Trace reaches the band of another level, a few rows off: it stuns only yours.
    let radius = match st
        .character
        .ability(&st.content, &game::AbilityId::new("arc_trace"))
    {
        Some(game::Ability {
            kind: game::AbilityKind::Active(game::Active::Trace { radius }),
            ..
        }) => radius,
        other => panic!("the deck's arc trace: {other:?}"),
    };
    // You at the open tile of your level nearest a piece of another level, in its reach.
    let (at, away) = (k.tiles.iter())
        .filter(|(p, t)| t.terrain == Terrain::NetFloor && t.feature.is_none() && level(*p) == 0)
        .flat_map(|(p, _)| {
            (k.guards.iter().enumerate())
                .filter(move |(_, g)| level(g.pos) != 0 && g.pos.chebyshev(p) <= radius)
                .map(move |(i, g)| (g.pos.chebyshev(p), p, i))
        })
        .min()
        .map(|(_, p, i)| (p, i))
        .expect("a piece of another level in the arc's reach");
    st.player.as_mut().unwrap().pos = at;
    fire(&mut st, "arc_trace").unwrap();
    assert_eq!(st.net.ice[&b][away].stunned, 0, "another level's piece");
    let stunned: Vec<usize> = (0..k.guards.len())
        .filter(|&i| st.net.ice[&b][i].stunned > 0)
        .collect();
    assert!(!stunned.is_empty(), "your level's ICE in reach");
    assert!(stunned.iter().all(|&i| level(k.guards[i].pos) == 0));
    // The HERE lines list your level's pieces.
    let ours = (k.guards.iter())
        .filter(|g| k.guard_at(g.pos).is_some() && level(g.pos) == 0)
        .count();
    assert_eq!(
        crate::net::ice_lines(
            &st,
            Place::SiteNet {
                building: b,
                level: 0
            },
            |_| true
        )
        .len(),
        ours
    );
    // A ping lights your level's nodes, every one of them.
    st.character
        .talents
        .insert(game::TalentId::new("handshake"), 1);
    st.fx.clear();
    fire(&mut st, "ping").unwrap();
    let glow = (st.fx.iter())
        .find(|f| matches!(f.fx, game::Fx::Glow { .. }))
        .expect("the ping's glow");
    let mut lit = glow.targets.clone();
    lit.sort_by_key(|p| (p.y, p.x));
    let mut nodes: Vec<Pos> = (k.devices.iter())
        .map(|d| d.at)
        .filter(|p| level(*p) == 0)
        .collect();
    nodes.sort_by_key(|p| (p.y, p.x));
    assert!(!nodes.is_empty());
    assert_eq!(lit, nodes);
}

#[test]
fn a_sites_net_is_worked_out_once_and_kept() {
    let (st, b) = office_copy(190);
    let first = net_of(&st, b).unwrap();
    let again = net_of(&st, b).unwrap();
    assert!(std::sync::Arc::ptr_eq(&first, &again), "the one kept");
}

#[test]
fn a_copy_starting_or_ending_drops_the_kept_net() {
    let (mut st, b) = office_copy(190);
    let copy = st.interiors.copy(b).unwrap().clone();
    assert_eq!(net_of(&st, b).unwrap().security, 190, "the copy's, kept");
    crate::venues::end(&mut st, b).unwrap();
    let plain = net_of(&st, b).unwrap();
    assert_ne!(plain.security, 190, "the plain building's own");
    assert_eq!(plain.gap, None);
    crate::venues::begin(&mut st, b, copy.inputs).unwrap();
    assert!(
        crate::venues::copy_live(&st, b),
        "taken at once: you're out"
    );
    let net = net_of(&st, b).unwrap();
    assert_eq!(net.security, 190, "the copy's again");
    assert_eq!(
        net.gap,
        citygen::sitenet::gapped_level(copy.layer.as_ref()),
        "its air gap"
    );
}

#[test]
fn forgetting_a_sites_net_drops_the_kept_one() {
    let (mut st, b) = office_copy(190);
    let kept = net_of(&st, b).unwrap();
    forget(&mut st, b);
    let again = net_of(&st, b).unwrap();
    assert!(!std::sync::Arc::ptr_eq(&kept, &again), "worked out again");
    assert_eq!(*kept, *again, "from the same plan and copy");
}
