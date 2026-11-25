use super::*;
use crate::containers::state::Opened;
use crate::jobs::testutil::a_fixer;
use crate::jobs::{PLANT_AT, take_job};

/// A plain office away from home, a fixer who trusts you, and a small site job asking
/// `kind` on its first objective that offers it, taken: its copy live, you on the street
/// at the office's main door.
fn taken(kind: ObjectiveKind) -> (AppState, JobId, BuildingId) {
    let (mut st, b) = crate::sitenet::testutil::office_plain();
    let fixer = a_fixer(&st);
    st.jobs.fixer_mut(fixer).standing = PLANT_AT;
    let plan = crate::interiors::plan_now(&st, b).unwrap().into_owned();
    let layout = plan.venue.as_deref().unwrap();
    let pack = VenuePack::builtin();
    let t = pack.by_id(&layout.type_id).unwrap();
    for (k, def) in t.objectives.iter().enumerate() {
        if !def.kinds.contains(&kind) {
            continue;
        }
        for seed in 0..40 {
            let inputs = MissionInputs {
                size: MissionSize::Small,
                objective: k,
                security: 60,
                seed,
                kind,
            };
            let Ok(layer) = MissionLayer::lay(layout, t, pack.size(inputs.size), inputs) else {
                continue;
            };
            let item = match kind {
                ObjectiveKind::Take => def.item.as_deref().map(ItemId::new),
                ObjectiveKind::Copy => Some(ItemId::new("vault_data")),
                _ => None,
            };
            let site = Kind::Site {
                building: b,
                objective: k,
                kind,
                size: inputs.size,
                security: inputs.security,
                seed,
                item: item.clone(),
            };
            let mut job = super::super::generate::blank(fixer, site, 12, 900, 0);
            finish(&st, &mut job, def, layer.objective.level, item.as_ref(), 0);
            let id = st.jobs.push(job);
            take_job(&mut st, id).expect("it is taken");
            assert!(crate::venues::copy_live(&st, b), "its copy is live");
            return (st, id, b);
        }
    }
    panic!("no office objective lays a {kind:?}");
}

fn job(st: &AppState, id: JobId) -> &Job {
    st.jobs.get(id).unwrap()
}

fn done(st: &AppState, id: JobId) -> [bool; 3] {
    let o = &job(st, id).objectives;
    [o[0].done, o[1].done, o[2].done]
}

/// You on the objective's tile, or beside it.
fn at_objective(st: &mut AppState, b: BuildingId) -> (Place, Pos) {
    let (_, place, at, _) = objective(st, b).expect("a live objective");
    let city = st.city.clone().unwrap();
    let Place::Floor { level, .. } = place else {
        unreachable!()
    };
    st.interiors.ensure(&city, b, level).expect("its floor");
    let p = st.player.as_mut().unwrap();
    p.place = place;
    p.pos = at;
    (place, at)
}

#[test]
fn sizes_follow_standing_and_fit_the_window_and_the_pay() {
    assert_eq!(size_at(PLANT_AT), MissionSize::Small);
    assert_eq!(size_at(MEDIUM_AT), MissionSize::Medium);
    assert_eq!(size_at(LARGE_AT), MissionSize::Large);
    assert_eq!(
        [MissionSize::Small, MissionSize::Medium, MissionSize::Large].map(hours),
        [12, 18, 24]
    );
    // Above a vault copy of the same security.
    for s in [0u8, 100, 255] {
        assert!(pay(MissionSize::Small, s) > u32::from(s) * crate::jobs::PAY_VAULT_PER_SECURITY);
    }
}

#[test]
fn a_trusted_fixer_offers_a_site_job_with_three_stops() {
    let mut st = crate::jobs::testutil::state_home();
    let fixer = a_fixer(&st);
    st.jobs.fixer_mut(fixer).standing = PLANT_AT;
    assert!(Jobs::allowed_for(PLANT_AT, false).contains(&KindTag::Site));
    assert!(!Jobs::allowed_for(PLANT_AT - 1, true).contains(&KindTag::Site));
    let j = (0..20)
        .find_map(|n| crate::jobs::generate::generate(&mut st, fixer, KindTag::Site, n))
        .expect("a site job");
    let (b, inputs) = j.kind.site().unwrap();
    assert_eq!(inputs.size, MissionSize::Small);
    assert_eq!(j.objectives.len(), 3);
    assert!(j.objectives[1].text.starts_with("Get out of"));
    assert!(!j.title.is_empty() && !j.brief.is_empty());
    assert!(j.brief.iter().all(|l| !l.contains('{')), "{:?}", j.brief);
    // Without a deck, never a Copy.
    assert_ne!(inputs.kind, ObjectiveKind::Copy);
    assert!(st.interiors.copy(b).is_none(), "an offer takes no building");
}

#[test]
fn a_reach_is_met_on_its_spot_then_out_then_handed_in() {
    let (mut st, id, b) = taken(ObjectiveKind::Reach);
    let outside = st.player.as_ref().map(|p| (p.place, p.pos)).unwrap();
    tick(&mut st);
    assert_eq!(done(&st, id), [false, false, false]);
    at_objective(&mut st, b);
    tick(&mut st);
    assert_eq!(done(&st, id), [true, false, false]);
    // Still inside: not out yet.
    tick(&mut st);
    assert!(!done(&st, id)[1]);
    let p = st.player.as_mut().unwrap();
    (p.place, p.pos) = outside;
    tick(&mut st);
    assert_eq!(done(&st, id), [true, true, false]);
    assert!(st.jobs.ready(id, &|_| false, &|_| false).is_ok());
}

#[test]
fn a_take_holds_its_item_alone_and_is_met_once_it_is_in_the_pack() {
    let (mut st, id, b) = taken(ObjectiveKind::Take);
    let item = match &job(&st, id).kind {
        Kind::Site { item, .. } => item.clone().expect("an item"),
        _ => unreachable!(),
    };
    at_objective(&mut st, b);
    let at = take_container(&st, b).expect("its container");
    let c = crate::containers::find(&st, at).expect("a container there");
    assert!(c.guarded, "the job's container is guarded");
    assert!(crate::containers::lock(&st, &c).is_some(), "and locked");
    assert_eq!(
        crate::containers::contents(&st, &c),
        vec![crate::containers::state::Loot::Item {
            item: item.clone(),
            qty: 1
        }]
    );
    crate::containers::open(&mut st, &c, Opened::Keyjack);
    tick(&mut st);
    assert!(!done(&st, id)[0], "opened, not taken");
    crate::containers::take_all(&mut st, &c).unwrap();
    assert_eq!(st.character.count(&item), 1);
    tick(&mut st);
    assert!(done(&st, id)[0]);
    // Lost: out of the pack and lying nowhere.
    game::remove_from_pack(&mut st.character, &item, 1);
    tick(&mut st);
    assert_eq!(job(&st, id).state, State::Failed);
}

#[test]
fn a_take_out_of_the_building_without_its_item_is_not_out() {
    let (mut st, id, b) = taken(ObjectiveKind::Take);
    let outside = st.player.as_ref().map(|p| (p.place, p.pos)).unwrap();
    let (place, at) = at_objective(&mut st, b);
    let c = crate::containers::find(&st, take_container(&st, b).unwrap()).unwrap();
    crate::containers::open(&mut st, &c, Opened::Keyjack);
    crate::containers::take_all(&mut st, &c).unwrap();
    tick(&mut st);
    let item = crate::jobs::handin_item(&job(&st, id).kind).unwrap();
    // Dropped on the floor: not lost, and not carried out.
    game::remove_from_pack(&mut st.character, &item, 1);
    st.ground.add(place, at, item.clone(), 1);
    let p = st.player.as_mut().unwrap();
    (p.place, p.pos) = outside;
    tick(&mut st);
    assert_eq!(job(&st, id).state, State::Taken);
    assert_eq!(done(&st, id), [true, false, false]);
}

#[test]
fn a_copy_is_met_by_the_vault_copy() {
    let (mut st, id, b) = taken(ObjectiveKind::Copy);
    crate::jobs::send(
        &mut st,
        Event::Copied {
            building: b,
            vault: true,
        },
    );
    assert!(done(&st, id)[0]);
}

#[test]
fn an_alarm_purges_the_vault_and_fails_an_uncopied_copy() {
    let (mut st, id, b) = taken(ObjectiveKind::Copy);
    crate::net::lock_down(&mut st, b);
    tick(&mut st);
    assert_eq!(purge_left(&st, b), Some(PURGE_MINUTES));
    for _ in 0..PURGE_MINUTES {
        crate::sitenet::testutil::next_minute(&mut st);
    }
    tick(&mut st);
    assert!(wiped(&st, b));
    assert_eq!(job(&st, id).state, State::Failed);
    // Nothing left to copy.
    assert_eq!(
        vault_copied(&mut st, b).as_deref(),
        Some("the vault is empty · wiped")
    );
}

#[test]
fn a_wipe_is_met_on_the_net_or_by_the_purge_at_half_pay() {
    let (mut st, id, b) = taken(ObjectiveKind::Wipe);
    assert_eq!(
        vault_copied(&mut st, b).as_deref(),
        Some("vault wiped · nothing left to sell")
    );
    assert!(done(&st, id)[0] && wiped(&st, b));

    let (mut st, id, b) = taken(ObjectiveKind::Wipe);
    let pay = job(&st, id).pay;
    crate::net::lock_down(&mut st, b);
    tick(&mut st);
    for _ in 0..PURGE_MINUTES {
        crate::sitenet::testutil::next_minute(&mut st);
    }
    tick(&mut st);
    assert!(done(&st, id)[0]);
    assert_eq!(job(&st, id).state, State::Taken);
    assert_eq!(job(&st, id).pay, pay / 2);
}

#[test]
fn forcing_the_rack_wipes_it_and_sounds_the_alarm() {
    let (mut st, id, b) = taken(ObjectiveKind::Wipe);
    let (place, at) = at_objective(&mut st, b);
    let Place::Floor { level, .. } = place else {
        unreachable!()
    };
    let spot = ContainerAt {
        building: b,
        level,
        at,
    };
    assert_eq!(work_row(&st).map(|r| r.2), Some("Force the rack"));
    st.character.attributes.body = 10;
    let (work, ticks) = can_work(&st, spot).expect("it can be forced");
    assert_eq!(work, Work::Rack);
    assert!(ticks > 0);
    finish_work(&mut st, spot, work);
    assert!(done(&st, id)[0] && wiped(&st, b));
    assert!(st.net.alert(b, st.net.now(&st)), "the alarm is on");
    assert!(work_row(&st).is_none());
}

#[test]
fn a_plant_hands_over_the_tap_and_is_met_once_it_is_pressed_on() {
    let (mut st, id, b) = taken(ObjectiveKind::Plant);
    let tap = ItemId::new(TAP);
    assert_eq!(st.character.count(&tap), 1, "the tap comes with the job");
    let (place, at) = at_objective(&mut st, b);
    let Place::Floor { level, .. } = place else {
        unreachable!()
    };
    let spot = ContainerAt {
        building: b,
        level,
        at,
    };
    assert_eq!(work_row(&st).map(|r| r.2), Some("Plant the tap"));
    let (work, ticks) = can_work(&st, spot).unwrap();
    assert_eq!((work, ticks), (Work::Tap, TAP_SECONDS * crate::app::FPS));
    finish_work(&mut st, spot, work);
    assert!(done(&st, id)[0]);
    assert_eq!(st.character.count(&tap), 0);
    assert!(can_work(&st, spot).is_err());
}

#[test]
fn a_closed_site_job_lets_its_copy_go() {
    let (mut st, id, b) = taken(ObjectiveKind::Reach);
    crate::jobs::ui::drop(&mut st, id).unwrap();
    tick(&mut st);
    crate::venues::tick(&mut st);
    assert!(st.interiors.copy(b).is_none(), "the copy ended");
}

#[test]
fn the_card_names_the_deed_and_then_the_way_out() {
    let (mut st, id, b) = taken(ObjectiveKind::Reach);
    let (text, todo, _) = stop_words(&st, job(&st, id));
    assert!(text.ends_with(&crate::net::building_name(&st, b)), "{text}");
    assert_eq!(todo, "get in");
    let card = |st: &AppState| -> String {
        crate::jobs::card::job_card(st, id, 60, &crate::jobs::card::Way::Unreachable)
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert!(
        card(&st).contains("small job · security 60"),
        "{}",
        card(&st)
    );
    at_objective(&mut st, b);
    tick(&mut st);
    assert!(card(&st).contains("now get out of"), "{}", card(&st));
}

#[test]
fn the_look_and_sitenet_name_the_job() {
    let (mut st, _, b) = taken(ObjectiveKind::Plant);
    let (place, at) = at_objective(&mut st, b);
    let line = look_line(&st, place, at).expect("the objective's line");
    assert!(line.ends_with("your job: plant the tap"), "{line}");
    assert!(
        crate::sitenet::panel::job_lines(&st, b)[0].starts_with("plant · "),
        "{:?}",
        crate::sitenet::panel::job_lines(&st, b)
    );
}

#[test]
fn tracking_leads_to_the_building_then_to_the_objective() {
    let (mut st, id, b) = taken(ObjectiveKind::Reach);
    let src = crate::track::Source::Job {
        job: id,
        objective: 0,
    };
    assert_eq!(
        crate::track::resolve(&st, &src),
        crate::track::Resolved::Target(crate::track::Target::Building(b))
    );
    let (place, at) = at_objective(&mut st, b);
    assert_eq!(
        crate::track::resolve(&st, &src),
        crate::track::Resolved::Target(crate::track::Target::Spot {
            place,
            pos: at,
            radius: 1
        })
    );
}
