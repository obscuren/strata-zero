use super::loot::{TableKey, Tables};
use super::state::{Loot, Opened};
use super::testutil::*;
use super::*;
use citygen::tile::ContainerKind;
use citygen::{BuildingKind, Feature, Furniture};
use game::ItemId;

fn frag() -> ItemId {
    ItemId::new("frag")
}

/// An opened container holding `left`.
fn holding(st: &mut AppState, c: &Container, left: Vec<Loot>) {
    let s = state_mut(st, c.at);
    s.opened = Some(Opened::Free);
    s.left = Some(left);
}

#[test]
fn every_kind_locks_as_the_spec_says() {
    let mut locker = (0usize, 0usize, ContainerKind::Locker);
    let mut drawer = (0usize, 0usize, ContainerKind::Drawer);
    let (mut safes, mut crates, mut open_cabinets, mut secure_cabinets) = (0, 0, 0, 0);
    for seed in 0..6 {
        for what in [BuildingKind::Hotel, BuildingKind::Office] {
            let (mut st, b) = plain_site(what, seed);
            let secure = layout_of(&st, b)
                .and_then(|v| v.zones.iter().position(|z| z == "secure"))
                .map(|z| z as u8);
            for c in all_in(&mut st, b) {
                let l = lock(&st, &c);
                match c.kind {
                    ContainerKind::Safe => {
                        let l = l.clone().expect("a safe is locked");
                        assert!(l.maker.is_some());
                        assert_eq!(l.grade, crate::keyjack::rating(&st, b));
                        safes += 1;
                    }
                    ContainerKind::Crate => {
                        assert!(l.is_none());
                        crates += 1;
                    }
                    ContainerKind::Locker => {
                        locker.0 += 1;
                        locker.1 += usize::from(l.is_some());
                    }
                    ContainerKind::Drawer => {
                        drawer.0 += 1;
                        drawer.1 += usize::from(l.is_some());
                    }
                    ContainerKind::Cabinet => {
                        let deep = matches!((secure, c.zone), (Some(s), Some(z)) if z >= s);
                        assert_eq!(
                            l.is_some(),
                            deep,
                            "a cabinet is locked in `secure` and deeper"
                        );
                        if deep {
                            secure_cabinets += 1;
                        } else {
                            open_cabinets += 1;
                        }
                    }
                }
                if let Some(l) = l.filter(|_| c.kind != ContainerKind::Safe) {
                    assert_eq!((l.grade, l.maker), (1, None));
                }
            }
        }
    }
    assert!(safes > 0 && crates > 0 && open_cabinets > 0);
    let _ = secure_cabinets;
    for (n, locked, kind) in [locker, drawer] {
        assert!(n >= 10, "{kind:?}: only {n} to judge by");
        let share = locked as f32 / n as f32;
        let (lo, hi) = match kind {
            ContainerKind::Locker => (0.3, 0.7),
            _ => (0.1, 0.4),
        };
        assert!(
            (lo..=hi).contains(&share),
            "{kind:?}: {locked} of {n} locked"
        );
    }
}

#[test]
fn a_layer_spot_is_locked_at_the_makers_grade_on_the_net() {
    let (st, c) = copy_with_spot(150);
    let l = lock(&st, &c).expect("locked");
    assert!(c.guarded && l.node.is_some() && l.maker.is_some());
    assert_eq!(l.grade, crate::keyjack::rating(&st, c.at.building));
}

#[test]
fn the_same_container_rolls_the_same_twice_and_across_a_save() {
    let (mut st, c) = plain_site_with(ContainerKind::Safe);
    let pure = contents(&st, &c);
    assert_eq!(pure, contents(&st, &c));
    open(&mut st, &c, Opened::Free);
    assert_eq!(contents(&st, &c), pure);
    assert_eq!(state(&st, c.at).unwrap().opened, Some(Opened::Free));
    let before = state(&st, c.at).cloned();
    let back = crate::outside::tests::reopen(st, "container-reopen");
    assert_eq!(state(&back, c.at).cloned(), before);
    let again = find(&back, c.at).expect("still there");
    assert_eq!(contents(&back, &again), pure);
}

#[test]
fn a_new_copy_rolls_fresh() {
    let (mut st, c) = plain_site_with(ContainerKind::Safe);
    let b = c.at.building;
    let plain = seed(&st, &c);
    let inputs = |seed| citygen::venue::MissionInputs {
        size: citygen::venue::MissionSize::Medium,
        objective: 0,
        security: 128,
        seed,
    };
    crate::venues::begin(&mut st, b, inputs(1)).expect("a job");
    assert!(crate::venues::copy_live(&st, b));
    let first = seed(&st, &c);
    st.interiors.copy_mut(b).unwrap().inputs.seed = 2;
    let second = seed(&st, &c);
    assert!(plain != first && plain != second && first != second);
}

#[test]
fn taking_half_leaves_half() {
    let (mut st, c) = plain_site_with(ContainerKind::Safe);
    let items = st.character.count(&frag());
    let credits = st.character.credits;
    let keep = vec![
        Loot::Item {
            item: frag(),
            qty: 2,
        },
        Loot::Cash(50),
    ];
    let mut left = vec![
        Loot::Cash(100),
        Loot::Chip {
            credits: 400,
            locked: false,
        },
    ];
    left.extend(keep.clone());
    holding(&mut st, &c, left);
    assert_eq!(take(&mut st, &c, 0).unwrap(), "took 100 cr");
    assert!(take(&mut st, &c, 0).unwrap().contains("credchip"));
    assert_eq!(st.character.credits, credits + 100);
    assert_eq!(state(&st, c.at).unwrap().left.as_ref(), Some(&keep));
    assert_eq!(contents(&st, &c), keep);
    take_all(&mut st, &c).unwrap();
    assert_eq!(st.character.credits, credits + 150);
    assert_eq!(st.character.count(&frag()), items + 2);
    assert_eq!(status(&st, &c), Status::Empty);
}

#[test]
fn a_take_over_the_carry_cap_is_refused() {
    let (mut st, c) = plain_site_with(ContainerKind::Safe);
    st.character.dev = false;
    st.character.inventory.clear();
    let cap = st.character.carry_cap(&st.content);
    let w = st.content.item(&frag()).unwrap().weight;
    assert!(st.character.weight(&st.content) < cap);
    while st.character.weight(&st.content) + w <= cap {
        st.character.give(frag(), 1);
    }
    let left = vec![
        Loot::Item {
            item: frag(),
            qty: 3,
        },
        Loot::Cash(10),
    ];
    holding(&mut st, &c, left.clone());
    let err = take(&mut st, &c, 0).unwrap_err();
    assert!(err.contains("over capacity"), "{err}");
    assert_eq!(state(&st, c.at).unwrap().left.as_ref(), Some(&left));
    let credits = st.character.credits;
    take(&mut st, &c, 1).unwrap();
    assert_eq!(st.character.credits, credits + 10);
    let said = take_all(&mut st, &c).unwrap_err();
    assert!(said.contains("over capacity"), "{said}");
}

#[test]
fn a_line_naming_an_item_this_build_lacks_is_left_out() {
    let (mut st, c) = plain_site_with(ContainerKind::Safe);
    let credits = st.character.credits;
    holding(
        &mut st,
        &c,
        vec![
            Loot::Item {
                item: ItemId::new("gone_item"),
                qty: 1,
            },
            Loot::Cash(10),
        ],
    );
    assert_eq!(contents(&st, &c), vec![Loot::Cash(10)]);
    assert_eq!(status(&st, &c), Status::Open(vec![Loot::Cash(10)]));
    take_all(&mut st, &c).unwrap();
    assert_eq!(st.character.credits, credits + 10);
    assert_eq!(status(&st, &c), Status::Empty);
}

#[test]
fn a_kindless_spot_on_an_old_copys_floor_is_a_safe() {
    let (mut st, c) = copy_with_spot(150);
    let (b, level, at) = (c.at.building, c.at.level, c.at.at);
    let mut floor = st.interiors.floor(b, level).unwrap().1.clone();
    floor.tiles.get_mut(at).unwrap().feature = Some(Feature::Container);
    st.interiors.put_floor(b, floor);
    let old = find(&st, c.at).expect("a container");
    assert_eq!(old.kind, ContainerKind::Safe);
    assert!(old.guarded);
    let l = lock(&st, &old).expect("locked");
    assert_eq!(l.grade, crate::keyjack::rating(&st, b));
    assert!(l.maker.is_some() && l.node.is_some());
}

#[test]
fn the_flats_lockers_are_no_containers() {
    let mut st = AppState::new();
    st.enter_city(crate::runs::testutil::story_city());
    st.player = Some(crate::walker::Player::spawn(st.city.as_ref().unwrap(), 1));
    let mut map = crate::screens::explore::MapScreen::walking(&mut st);
    let home = st.home.as_ref().expect("a home").home;
    let city = st.city.clone().unwrap();
    let floor = st
        .interiors
        .ensure(&city, home.building, home.level)
        .expect("the flat's floor")
        .clone();
    let lockers: Vec<_> = floor
        .tiles
        .iter()
        .filter(|(p, t)| {
            home.rect.contains(*p) && t.feature == Some(Feature::Furniture(Furniture::Locker))
        })
        .map(|(p, _)| p)
        .collect();
    assert!(!lockers.is_empty(), "the flat has lockers");
    let at = |at| ContainerAt {
        building: home.building,
        level: home.level,
        at,
    };
    for p in &lockers {
        assert!(find(&st, at(*p)).is_none());
    }
    crate::app::Screen::update(&mut map, &mut st, &[]);
    for p in &lockers {
        assert!(find(&st, at(*p)).is_none() && state(&st, at(*p)).is_none());
    }
}

#[test]
fn sums_are_written_with_thousands() {
    assert_eq!(cr(0), "0 cr");
    assert_eq!(cr(950), "950 cr");
    assert_eq!(cr(1250), "1,250 cr");
    assert_eq!(cr(1_250_000), "1,250,000 cr");
}

#[test]
fn a_plain_site_pays_less_than_a_job() {
    let tables = Tables::shipped();
    for what in [BuildingKind::Hotel, BuildingKind::Office] {
        for seed in 0..4 {
            let (mut st, b) = plain_site(what, seed);
            let mut paid = 0u32;
            for c in all_in(&mut st, b) {
                let key = TableKey {
                    kind: c.kind,
                    site: &c.site,
                    guarded: false,
                    deeper: None,
                };
                for l in loot::roll(tables, &key, 128, super::seed(&st, &c)) {
                    paid += match l {
                        Loot::Cash(n) => n,
                        Loot::Chip {
                            credits,
                            locked: false,
                        } => credits,
                        _ => 0,
                    };
                }
            }
            let job = 128 * crate::jobs::PAY_COPY_PER_SECURITY;
            assert!(
                paid < job,
                "{what:?} seed {seed}: {paid} cr against a job's {job}"
            );
        }
    }
}

#[test]
fn jacking_in_at_a_container_lands_on_its_node_with_the_body_beside_it() {
    let (mut st, c) = copy_with_spot(150);
    beside(&mut st, &c);
    let b = c.at.building;
    let body = st.player.as_ref().unwrap().pos;
    let node = lock(&st, &c).expect("locked").node.expect("on the net");
    ways::jack_in(&mut st, &c).expect("jacks in");
    let p = st.player.as_ref().unwrap();
    assert_eq!(
        p.place,
        citygen::Place::SiteNet {
            building: b,
            level: c.at.level
        }
    );
    assert_eq!(
        p.body,
        Some((
            citygen::Place::Floor {
                building: b,
                level: c.at.level
            },
            body
        ))
    );
    let k = st.constructs.site(b).unwrap();
    let d = k.device(node).expect("the construct holds its node");
    assert!(d.at.chebyshev(p.pos) <= 1, "on or beside its node");
}

#[test]
fn an_alarm_locks_the_port_down() {
    let (mut st, c) = copy_with_spot(150);
    beside(&mut st, &c);
    crate::net::lock_down(&mut st, c.at.building);
    assert_eq!(
        ways::jack_in(&mut st, &c),
        Err("port locked down".to_string())
    );
}

#[test]
fn a_container_node_is_no_port() {
    let (mut st, c) = copy_with_spot(150);
    let (b, level) = (c.at.building, c.at.level);
    assert!(
        crate::sitenet::floor_devices(&st, b, level)
            .iter()
            .all(|(_, k)| *k != citygen::NodeKind::Container)
    );
    beside(&mut st, &c);
    assert!(crate::sitenet::port_in_reach(&st).is_none());
}

#[test]
fn a_plain_container_is_not_on_the_net() {
    let (mut st, c) = plain_site_with(ContainerKind::Locker);
    beside(&mut st, &c);
    assert_eq!(
        ways::jack_in(&mut st, &c),
        Err("not on the Net".to_string())
    );
}
