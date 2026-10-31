//! Story missions: authored chains of steps in `flavor/missions.ron`, cast onto the
//! city's people and moved on by what the game reports and what the engine sees.

pub mod call;
pub mod cast;
pub mod content;
pub mod effects;
pub mod face;
pub mod feed;

pub use content::*;

use crate::app::AppState;
use crate::render::Visibility;
use citygen::{Place, Pos, StratumKind};
use sim::PersonId;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The session's story: who plays each role, where each mission stands, the bodies
/// it left and what the HOLO has waiting. Reset with the city.
#[derive(Clone, Debug, Default)]
pub struct Missions {
    pub cast: BTreeMap<String, PersonId>,
    /// One per mission, in book order.
    pub runs: Vec<Run>,
    pub bodies: Vec<Body>,
    /// Feeds opened, not yet shown.
    pub feeds: VecDeque<FeedView>,
    /// The call that is up.
    pub call: Option<Call>,
    /// Calls waiting their turn behind it.
    pub calls: VecDeque<Call>,
    /// The story has begun in this city.
    pub begun: bool,
}

/// Where one mission stands.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// The step it waits on.
    pub at: usize,
    /// The game minute that step was armed.
    pub since: u64,
    pub state: RunState,
    /// Each `Objective` pushes one; the last is current.
    pub objectives: Vec<String>,
    /// Steps whose feed or call has ended.
    pub ended: BTreeSet<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunState {
    /// Nothing fired yet; not shown.
    Waiting,
    Open,
    /// The last step fired at this game minute.
    Done {
        at: u64,
    },
    /// A role it needs has nobody in this city.
    Uncast,
}

/// A person the story killed, lying at their station.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub person: PersonId,
    pub name: String,
    pub place: Place,
    pub pos: Pos,
    /// What look mode says of it.
    pub look: String,
}

/// A HOLO feed waiting to be shown, its text filled.
#[derive(Clone, Debug, PartialEq)]
pub struct FeedView {
    pub mission: usize,
    pub step: usize,
    /// The role it comes from.
    pub from: String,
    pub text: Vec<String>,
    /// The label after `space`.
    pub close: String,
}

/// A HOLO call, ringing or talking, its lines filled.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub mission: usize,
    pub step: usize,
    /// The role calling.
    pub from: String,
    pub lines: Vec<String>,
    /// The clock tick it began to ring.
    pub rang: u64,
    /// The clock tick it was answered.
    pub answered: Option<u64>,
    /// The line being said.
    pub line: usize,
    /// The clock tick that line began.
    pub shown: u64,
    /// `space` finished typing the line: it shows whole.
    pub full: bool,
}

/// What the game reports to the story; everything else it checks each tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    NewGame,
    Talked(PersonId),
}

/// Passes of firing at most per event or tick: a step already true when it is armed
/// fires at once, and a book that loops cannot hang the game.
const PASSES: usize = 16;

/// Casts the roles and starts the story: once per city, and only in a city with a
/// home. A mission that needs a role nobody plays stays `Uncast`.
pub fn begin(st: &mut AppState) {
    if st.missions.begun || st.home.is_none() || st.world.is_none() {
        return;
    }
    st.missions.begun = true;
    cast::cast(st);
    let now = st.net.now(st);
    let runs: Vec<Run> = st
        .book
        .missions
        .iter()
        .map(|m| Run {
            at: 0,
            since: now,
            state: if m.roles().iter().all(|r| st.missions.cast.contains_key(r)) {
                RunState::Waiting
            } else {
                RunState::Uncast
            },
            objectives: Vec::new(),
            ended: BTreeSet::new(),
        })
        .collect();
    st.missions.runs = runs;
    if st.dev {
        let uncast: Vec<String> = st
            .book
            .cast
            .keys()
            .filter(|r| !st.missions.cast.contains_key(*r))
            .cloned()
            .collect();
        for role in uncast {
            st.warn(format!("story · no {role} in this city"));
        }
    }
    send(st, Event::NewGame);
}

/// Reports an event: every mission waiting on it moves on.
pub fn send(st: &mut AppState, ev: Event) {
    fire(st, Some(&ev));
}

/// The story's beat in the explore update: the conditions checked, the call's ring
/// timed.
pub fn tick(st: &mut AppState) {
    fire(st, None);
    call::tick(st);
}

/// The feed or call that step `step` of mission `mission` opened has ended.
pub fn end(st: &mut AppState, mission: usize, step: usize) {
    if let Some(run) = st.missions.runs.get_mut(mission) {
        run.ended.insert(step);
    }
    fire(st, None);
}

/// Whether a mission waits on talking to this person.
pub fn listens(st: &AppState, who: PersonId) -> bool {
    (0..st.missions.runs.len()).any(|m| {
        matches!(waiting(st, m).map(|s| &s.when),
            Some(Trigger::Talk(role)) if person_of(st, role) == Some(who))
    })
}

/// The step mission `m` waits on, while it is live.
fn waiting(st: &AppState, m: usize) -> Option<&Step> {
    let run = st.missions.runs.get(m)?;
    if matches!(run.state, RunState::Done { .. } | RunState::Uncast) {
        return None;
    }
    st.book.missions.get(m)?.steps.get(run.at)
}

/// Runs every live mission whose waiting step's trigger holds, pass after pass until
/// none does. The event counts in the first pass only: one event moves a mission one
/// step.
fn fire(st: &mut AppState, ev: Option<&Event>) {
    let mut ev = ev;
    for _ in 0..PASSES {
        let mut any = false;
        for m in 0..st.missions.runs.len() {
            let Some(step) = waiting(st, m) else {
                continue;
            };
            if !step.when.fires(st, m, ev) {
                continue;
            }
            let then = step.then.clone();
            let at = st.missions.runs[m].at;
            for e in &then {
                effects::apply(st, m, at, e);
            }
            let now = st.net.now(st);
            let last = st.book.missions[m].steps.len();
            let run = &mut st.missions.runs[m];
            run.at = at + 1;
            run.since = now;
            run.state = if run.at >= last {
                RunState::Done { at: now }
            } else {
                RunState::Open
            };
            any = true;
        }
        ev = None;
        if !any {
            break;
        }
    }
}

impl Trigger {
    /// Whether the trigger holds for mission `m` now, or on `ev`.
    pub fn fires(&self, st: &AppState, m: usize, ev: Option<&Event>) -> bool {
        let Some(run) = st.missions.runs.get(m) else {
            return false;
        };
        let now = st.net.now(st);
        match self {
            Trigger::NewGame => ev == Some(&Event::NewGame),
            Trigger::Ended(step) => st.book.missions[m]
                .steps
                .iter()
                .position(|s| &s.id == step)
                .is_some_and(|i| run.ended.contains(&i)),
            Trigger::Sees { role, within } => sees(st, role, *within),
            Trigger::After { minutes } => now.saturating_sub(run.since) >= *minutes,
            Trigger::At(place) => at(st, place),
            Trigger::Talk(role) => {
                matches!(ev, Some(Event::Talked(who)) if person_of(st, role) == Some(*who))
            }
            Trigger::MissionDone { mission, minutes } => st
                .book
                .missions
                .iter()
                .position(|o| &o.id == mission)
                .and_then(|i| st.missions.runs.get(i))
                .is_some_and(|r| {
                    matches!(r.state, RunState::Done { at: done } if now.saturating_sub(done) >= *minutes)
                }),
        }
    }
}

/// The role's person is in the player's sight within `within` tiles: their body, or
/// alive, where they walk the street.
fn sees(st: &AppState, role: &str, within: i32) -> bool {
    let Some(p) = st.player.as_ref() else {
        return false;
    };
    let Some(id) = person_of(st, role) else {
        return false;
    };
    let seen = match st.missions.bodies.iter().find(|b| b.person == id) {
        Some(b) => Some((b.place, b.pos)),
        None => st
            .world
            .as_ref()
            .and_then(|w| w.pos_of(id))
            .map(|pos| (Place::Stratum(StratumKind::Street), pos)),
    };
    seen.is_some_and(|(place, pos)| {
        p.place == place
            && p.vis().get(pos) == Some(&Visibility::Visible)
            && p.pos.manhattan(pos) <= within
    })
}

/// The player is at the place: inside their flat, or on the street by a role's work.
fn at(st: &AppState, place: &Where) -> bool {
    let Some(p) = st.player.as_ref() else {
        return false;
    };
    match place {
        Where::Home => st
            .home
            .is_some_and(|rig| p.place == rig.place() && rig.home.rect.contains(p.pos)),
        Where::Work { role, within } => {
            p.place == Place::Stratum(StratumKind::Street)
                && person_of(st, role)
                    .and_then(|id| cast::work_tile(st, id))
                    .is_some_and(|t| p.pos.manhattan(t) <= *within)
        }
    }
}

/// A text with its `{role}` and `{role.field}` slots filled from the cast. A slot the
/// cast cannot fill stays as written.
pub fn fill(st: &AppState, text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            break;
        };
        out.push_str(&rest[..open]);
        let slot = &after[..close];
        let (role, field) = match slot.split_once('.') {
            Some((role, field)) => (role, Some(field)),
            None => (slot, None),
        };
        match slot_text(st, role, field) {
            Some(text) => out.push_str(&text),
            None => {
                out.push('{');
                out.push_str(slot);
                out.push('}');
            }
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// What one slot says: the role's name, or one of `SLOT_FIELDS`.
fn slot_text(st: &AppState, role: &str, field: Option<&str>) -> Option<String> {
    let r = st.book.cast.get(role)?;
    let Some(field) = field else {
        return Some(r.name.clone());
    };
    if field == "title" {
        return Some(r.title.clone());
    }
    let who = person_of(st, role)?;
    match field {
        "place" => Some(cast::place_of(st, who)),
        "district" => Some(cast::district_of(st, who)),
        "hours" => Some(cast::hours_of(st, who)),
        _ => None,
    }
}

/// The person cast in a role.
pub fn person_of(st: &AppState, role: &str) -> Option<PersonId> {
    st.missions.cast.get(role).copied()
}

/// The book's name for a role.
pub fn role_name(st: &AppState, role: &str) -> String {
    st.book
        .cast
        .get(role)
        .map_or_else(|| role.to_string(), |r| r.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::testutil::state_home;
    use crate::render::map::MapRef;
    use crate::walker::Player;
    use citygen::{GenParams, Generator, SizePreset};
    use game::ItemId;

    const STREET: Place = Place::Stratum(StratumKind::Street);

    /// Seed 8 with its home moved into, the story begun.
    pub(super) fn begun() -> AppState {
        let mut st = state_home();
        begin(&mut st);
        st
    }

    /// Seed 8 put in place as a new game would, nothing walked yet.
    fn fresh() -> AppState {
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
        st
    }

    /// The home building's first door, as the jobs measure from it.
    fn door(st: &AppState) -> Pos {
        let b = st.home.unwrap().home.building;
        st.city.as_ref().unwrap().portals(b).first().unwrap().1
    }

    /// Stands the player on a street tile, having looked around.
    pub(super) fn on_street(st: &mut AppState, pos: Pos) {
        let city = st.city.as_ref().unwrap();
        let mut p = Player::at(STREET, pos);
        p.update_fov(
            &MapRef::stratum_at(city, StratumKind::Street, st.minute_of_day()),
            0,
        );
        st.player = Some(p);
    }

    /// Runs the world on `minutes` game minutes.
    fn advance(st: &mut AppState, minutes: u64) {
        if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
            world.advance(city, world.tick() + minutes * sim::TICKS_PER_MINUTE);
        }
    }

    /// A book with the shipped cast's picks and these missions.
    fn book(missions: &str) -> Book {
        Book::parse(&format!(
            r#"Story(
                cast: {{
                    "dez": Role(name: "Dez", title: "fixer", pick: NearestFixer, face: None),
                    "marlo": Role(name: "Marlo", title: "cook", face: None,
                        pick: Worker(professions: ["noodle_cook"], at: [], range: (30, 140))),
                }},
                faces: {{
                    "silhouette": Face(rows: ["......dd........", "....dmmmmd......"], eye: (255, 255, 255)),
                }},
                missions: [{missions}],
            )"#
        ))
        .unwrap()
    }

    fn marlo(st: &AppState) -> PersonId {
        person_of(st, "marlo").expect("marlo is cast")
    }

    /// The welcome closed and the body seen, a minute on: Dez's call is up.
    pub(super) fn to_the_call(st: &mut AppState) {
        end(st, 0, 0);
        let body = st.missions.bodies[0].pos;
        on_street(st, body);
        tick(st);
        advance(st, 1);
        tick(st);
    }

    #[test]
    fn roles_include_a_talk_role() {
        let b = book(
            r#"Mission(id: "chat", title: "Chat", from: "marlo", steps: [
                Step(id: "hello", when: Talk("dez"), then: [Log("hi")]),
            ])"#,
        );
        assert!(b.missions[0].roles().contains("dez"));
    }

    #[test]
    fn new_game_has_no_favour_mail() {
        let mut st = fresh();
        let _ = crate::screens::explore::MapScreen::walking(&mut st);
        assert!(st.jobs.list.is_empty(), "{:?}", st.jobs.list);
        assert!(
            st.journal.mails.iter().all(|m| m.subject != "a favour"),
            "{:?}",
            st.journal.mails
        );
    }

    #[test]
    fn explore_screen_begins_the_story() {
        let mut st = fresh();
        let _ = crate::screens::explore::MapScreen::walking(&mut st);
        assert!(!st.missions.begun);
        let mut st = fresh();
        let _ = crate::screens::explore_screen(&mut st);
        assert!(st.missions.begun);
        assert_eq!(st.missions.feeds.len(), 1);
    }

    #[test]
    fn begin_casts_dez_and_marlo_and_queues_the_welcome() {
        let st = begun();
        let dez = person_of(&st, "dez").expect("dez is cast");
        let marlo = marlo(&st);
        assert_ne!(dez, marlo);
        let door = door(&st);
        assert_eq!(Some(dez), crate::jobs::nearest_fixer(&st, door));
        let people = st.world.as_ref().unwrap().people();
        assert_eq!(people[dez.0 as usize].name, "Dez");
        assert_eq!(people[marlo.0 as usize].name, "Marlo");
        assert_eq!(people[marlo.0 as usize].profession, "noodle_cook");
        let d = cast::work_tile(&st, marlo).unwrap().manhattan(door);
        assert!((30..=140).contains(&d), "Marlo works {d} from home");
        assert_eq!(st.missions.feeds.len(), 1);
        let feed = &st.missions.feeds[0];
        assert_eq!((feed.mission, feed.step, feed.from.as_str()), (0, 0, "dez"));
        assert!(feed.text[1].contains("Marlo"), "{:?}", feed.text);
        assert!(
            !feed.text.iter().any(|t| t.contains('{')),
            "{:?}",
            feed.text
        );
        assert_eq!(feed.close, "take the package");
        assert_eq!(st.missions.runs[0].state, RunState::Open);
        assert_eq!(st.missions.runs[0].at, 1);
    }

    #[test]
    fn the_same_city_casts_the_same() {
        let (a, b) = (begun(), begun());
        assert_eq!(a.missions.cast.len(), 2);
        assert_eq!(a.missions.cast, b.missions.cast);
    }

    #[test]
    fn the_drop_casts_on_every_medium_seed() {
        for seed in 1..=16 {
            let mut st = AppState::new();
            st.enter_city(
                Generator::run(GenParams {
                    seed,
                    size: SizePreset::Medium,
                    ..Default::default()
                })
                .unwrap(),
            );
            if st.home.is_none() {
                continue;
            }
            begin(&mut st);
            // Both cast, Marlo in range or, where no cook is, the nearest past it.
            assert_eq!(st.missions.cast.len(), 2, "seed {seed}");
            assert_eq!(st.missions.runs[0].state, RunState::Open, "seed {seed}");
            assert_eq!(st.missions.feeds.len(), 1, "seed {seed}");
        }
    }

    #[test]
    fn closing_the_welcome_gives_the_package_and_kills_marlo() {
        let mut st = begun();
        let marlo = marlo(&st);
        end(&mut st, 0, 0);
        assert_eq!(st.character.count(&ItemId::new("taped_package")), 1);
        assert_eq!(st.character.count(&ItemId::new("sealed_package")), 0);
        assert!(st.world.as_ref().unwrap().is_dead(marlo));
        assert_eq!(st.missions.bodies.len(), 1);
        let body = &st.missions.bodies[0];
        assert_eq!(
            (body.person, body.name.as_str(), body.place),
            (marlo, "Marlo", STREET)
        );
        assert_eq!(Some(body.pos), cast::work_tile(&st, marlo));
        assert!(body.look.contains("apron"), "{}", body.look);
        let run = &st.missions.runs[0];
        let objective = run.objectives.last().unwrap();
        assert!(
            objective.starts_with("Hand the package to Marlo"),
            "{objective}"
        );
        assert!(!objective.contains('{'), "{objective}");
        assert_eq!(st.book.missions[0].steps[run.at].id, "body");
        assert!(
            st.log
                .iter()
                .any(|e| e.text == "Taped package ×1 · in your pack"),
            "{:?}",
            st.log.iter().map(|e| &e.text).collect::<Vec<_>>()
        );
        // Closing it again does nothing more.
        end(&mut st, 0, 0);
        assert_eq!(st.character.count(&ItemId::new("taped_package")), 1);
        assert_eq!(st.missions.bodies.len(), 1);
    }

    #[test]
    fn seeing_the_body_then_a_minute_rings_dez() {
        let mut st = begun();
        end(&mut st, 0, 0);
        // Out of sight, nothing.
        tick(&mut st);
        assert_eq!(st.book.missions[0].steps[st.missions.runs[0].at].id, "body");
        let body = st.missions.bodies[0].pos;
        let city = st.city.as_ref().unwrap();
        let beside = body
            .neighbors4()
            .find(|p| city.walkable(StratumKind::Street, *p))
            .unwrap_or(body);
        on_street(&mut st, beside);
        tick(&mut st);
        assert_eq!(st.book.missions[0].steps[st.missions.runs[0].at].id, "ring");
        assert!(st.log.iter().any(|e| e.text == "Marlo is dead"));
        assert!(st.missions.call.is_none());
        advance(&mut st, 1);
        tick(&mut st);
        let call = st.missions.call.as_ref().expect("Dez rings");
        assert_eq!((call.mission, call.step, call.from.as_str()), (0, 3, "dez"));
        assert_eq!(call.lines.len(), 3);
        assert!(
            call.lines[1].starts_with("Marlo was meant"),
            "{:?}",
            call.lines
        );
        assert_eq!(call.rang, st.clock_tick());
        assert_eq!(st.book.missions[0].steps[st.missions.runs[0].at].id, "run");
    }

    #[test]
    fn ending_the_call_sets_get_home_and_home_closes_it() {
        let mut st = begun();
        to_the_call(&mut st);
        end(&mut st, 0, 3);
        let run = &st.missions.runs[0];
        assert_eq!(
            run.objectives.last().map(String::as_str),
            Some("Get home · keep the package")
        );
        assert_eq!(st.book.missions[0].steps[run.at].id, "home");
        // Still on the street: not home.
        tick(&mut st);
        assert_eq!(st.missions.runs[0].state, RunState::Open);
        let rig = st.home.unwrap();
        let p = st.player.as_mut().unwrap();
        p.place = rig.place();
        p.pos = rig.home.bed_side;
        assert!(rig.home.rect.contains(p.pos));
        tick(&mut st);
        assert!(matches!(st.missions.runs[0].state, RunState::Done { .. }));
        let mail = st.journal.mails.last().unwrap();
        assert_eq!(
            (mail.from.as_str(), mail.subject.as_str()),
            ("Dez", "lay low")
        );
        assert_eq!(mail.body.len(), 3);
        assert!(
            st.log
                .iter()
                .any(|e| e.text == "The drop · home · Dez will call")
        );
        assert_eq!(st.character.count(&ItemId::new("taped_package")), 1);
    }

    #[test]
    fn a_role_nobody_can_play_leaves_the_mission_uncast() {
        let mut st = state_home();
        st.unlock_dev();
        st.book.cast.get_mut("marlo").unwrap().pick = Pick::Worker {
            professions: vec!["astronaut".into()],
            at: vec![],
            range: (30, 140),
        };
        begin(&mut st);
        assert!(!st.missions.cast.contains_key("marlo"));
        assert_eq!(st.missions.runs[0].state, RunState::Uncast);
        assert!(st.missions.feeds.is_empty());
        assert!(
            st.log
                .iter()
                .any(|e| e.text == "story · no marlo in this city"),
            "{:?}",
            st.log.iter().map(|e| &e.text).collect::<Vec<_>>()
        );
        end(&mut st, 0, 0);
        tick(&mut st);
        assert_eq!(st.missions.runs[0].state, RunState::Uncast);
        assert_eq!(st.character.count(&ItemId::new("taped_package")), 0);
    }

    #[test]
    fn a_new_city_begins_a_new_story() {
        let mut st = begun();
        end(&mut st, 0, 0);
        assert!(st.missions.begun && !st.missions.bodies.is_empty());
        let city = st.city.clone().unwrap();
        st.enter_city(city);
        assert!(!st.missions.begun);
        assert!(st.missions.cast.is_empty() && st.missions.runs.is_empty());
        assert!(st.missions.bodies.is_empty() && st.missions.feeds.is_empty());
        begin(&mut st);
        assert_eq!(st.missions.cast.len(), 2);
        assert_eq!(st.missions.feeds.len(), 1);
        let marlo = marlo(&st);
        assert!(!st.world.as_ref().unwrap().is_dead(marlo));
        // Once per city.
        begin(&mut st);
        assert_eq!(st.missions.feeds.len(), 1);
    }

    #[test]
    fn mission_done_after_minutes_starts_the_next() {
        let mut st = state_home();
        st.book = book(
            r#"Mission(id: "first", title: "First", from: "dez", steps: [
                Step(id: "start", when: NewGame, then: [Objective("Meet {marlo}")]),
            ]),
            Mission(id: "second", title: "Second", from: "dez", steps: [
                Step(id: "later", when: MissionDone(mission: "first", minutes: 5), then: [
                    Objective("Later"),
                ]),
                Step(id: "wait", when: After(minutes: 60), then: [Log("an hour")]),
            ])"#,
        );
        begin(&mut st);
        assert!(matches!(st.missions.runs[0].state, RunState::Done { .. }));
        assert_eq!(st.missions.runs[0].objectives, vec!["Meet Marlo"]);
        assert_eq!(st.missions.runs[1].state, RunState::Waiting);
        advance(&mut st, 4);
        tick(&mut st);
        assert_eq!(st.missions.runs[1].state, RunState::Waiting);
        advance(&mut st, 1);
        tick(&mut st);
        assert_eq!(st.missions.runs[1].state, RunState::Open);
        assert_eq!(st.missions.runs[1].objectives, vec!["Later"]);
        assert_eq!(st.missions.runs[1].at, 1);
    }

    #[test]
    fn at_work_fires_near_the_role() {
        let mut st = state_home();
        st.book = book(
            r#"Mission(id: "visit", title: "Visit", from: "dez", steps: [
                Step(id: "go", when: NewGame, then: [Objective("Find {marlo} at {marlo.place}")]),
                Step(id: "there", when: At(Work(role: "marlo", within: 3)), then: [
                    Log("at `{marlo.place}`"),
                ]),
            ])"#,
        );
        begin(&mut st);
        assert_eq!(st.missions.runs[0].at, 1);
        let tile = cast::work_tile(&st, marlo(&st)).unwrap();
        let far = door(&st);
        on_street(&mut st, far);
        tick(&mut st);
        assert_eq!(st.missions.runs[0].state, RunState::Open);
        on_street(&mut st, tile);
        tick(&mut st);
        assert!(matches!(st.missions.runs[0].state, RunState::Done { .. }));
        let place = fill(&st, "{marlo.place}");
        assert!(st.log.iter().any(|e| e.text == format!("at {place}")));
    }

    #[test]
    fn talking_to_the_role_fires_and_take_takes() {
        let mut st = state_home();
        st.book = book(
            r#"Mission(id: "hand", title: "Hand", from: "dez", steps: [
                Step(id: "get", when: NewGame, then: [Give(item: "taped_package", count: 2)]),
                Step(id: "give", when: Talk("marlo"), then: [Take(item: "taped_package", count: 1)]),
            ])"#,
        );
        begin(&mut st);
        let id = ItemId::new("taped_package");
        assert_eq!(st.character.count(&id), 2);
        let dez = person_of(&st, "dez").unwrap();
        let marlo = marlo(&st);
        assert!(listens(&st, marlo) && !listens(&st, dez));
        send(&mut st, Event::Talked(dez));
        assert_eq!(st.character.count(&id), 2);
        send(&mut st, Event::Talked(marlo));
        assert_eq!(st.character.count(&id), 1);
        assert!(matches!(st.missions.runs[0].state, RunState::Done { .. }));
        assert!(!listens(&st, marlo));
    }

    #[test]
    fn fill_fills_every_slot() {
        let st = begun();
        let dez = person_of(&st, "dez").unwrap();
        let hours = crate::jobs::hours_text(st.world.as_ref().unwrap().work_hours(dez).unwrap());
        assert_eq!(fill(&st, "{dez} the {dez.title}"), "Dez the fixer");
        assert_eq!(fill(&st, "at {dez.place}"), "at the corner");
        assert_eq!(fill(&st, "{dez.hours}"), hours);
        let district = fill(&st, "{marlo.district}");
        let tile = cast::work_tile(&st, marlo(&st)).unwrap();
        let city = st.city.as_ref().unwrap();
        let want = city
            .districts
            .iter()
            .find(|d| d.bounds.contains(tile))
            .unwrap();
        assert_eq!(district, want.name);
        assert!(!fill(&st, "{marlo.place}").contains('{'));
        assert_eq!(fill(&st, "no slots"), "no slots");
    }
}
