//! Story missions: authored chains of steps in `flavor/missions.ron`, cast onto the
//! city's people and moved on by what the game reports and what the engine sees.
//!
//! Owns the runtime (`Missions`, `Run`, `begin`, `send`, `tick`, `end`), the book's
//! types (`content`), casting (`cast`), what a step does (`effects`), and what the
//! player sees: the HOLO feed and call (`feed`, `call`, `face`), the scenes played on
//! the map (`scene`, with `marks` and `visitors`) and the JOBS lines (`ui`).
//! No mission, step or role id appears in Rust outside tests; the book names them.
//!
//! Adding a block (spec `docs/superpowers/specs/2026-10-31-story-missions-design.md`, 15):
//! - A trigger: a variant on `Trigger` (and `Where` for a place), its arm in
//!   `Trigger::fires`, its names in `Book::validate`; an event also gets a variant on
//!   `Event` and one `send` where the game does it. Test with a small book.
//! - An effect: a variant on `Effect`, its arm in `effects::apply`, its names in
//!   `Book::validate`. Test it.
//! - A pick: a variant on `Pick` and its arm in `cast::pick`.

pub mod bubbles;
pub mod call;
pub mod cast;
pub mod content;
pub mod effects;
pub mod face;
pub mod feed;
pub mod film;
pub mod marks;
pub mod overlay;
pub mod scene;
pub mod ui;
pub mod visitors;

pub use content::*;

use crate::app::AppState;
use crate::render::Visibility;
use citygen::{Place, Pos, StratumKind};
use game::ItemId;
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
    /// The authored people standing on the map, in the order they arrived.
    pub visitors: Vec<visitors::Visitor>,
    /// How bright your flat is; `None` is normal.
    pub lights: Option<f32>,
    /// Scenes queued by `Effect::Scene`: mission, step, scene id.
    pub scenes: VecDeque<(usize, usize, String)>,
    /// The scene playing.
    pub scene: Option<scene::SceneRun>,
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
            state: if st
                .book
                .needs_cast(m)
                .iter()
                .all(|r| st.missions.cast.contains_key(r))
            {
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
            .filter(|r| !st.book.is_visitor(r) && !st.missions.cast.contains_key(*r))
            .cloned()
            .collect();
        for role in uncast {
            st.warn(format!("story · no {role} in this city"));
        }
    }
    // The story restarts, so its items do: take back what an earlier start gave.
    let given: Vec<ItemId> = st.book.gives().into_iter().map(ItemId::new).collect();
    for id in given {
        let held = st.character.count(&id);
        game::remove_from_pack(&mut st.character, &id, held);
    }
    send(st, Event::NewGame);
}

/// Reports an event: every mission waiting on it moves on.
pub fn send(st: &mut AppState, ev: Event) {
    fire(st, Some(&ev));
}

/// The story's beat in the explore update: the conditions checked, the scene played,
/// the call's ring timed.
pub fn tick(st: &mut AppState) {
    fire(st, None);
    scene::tick(st);
    call::tick(st);
}

/// The feed, call or scene that step `step` of mission `mission` opened has ended.
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
            run_step(st, m);
            any = true;
        }
        ev = None;
        if !any {
            break;
        }
    }
}

/// Runs the effects of the step mission `m` waits on and moves it to the next.
fn run_step(st: &mut AppState, m: usize) {
    let Some(step) = waiting(st, m) else {
        return;
    };
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
}

/// Dev console: runs the step mission `m` waits on as if its trigger fired, then lets
/// whatever that makes true fire. False when nothing waits.
pub fn force(st: &mut AppState, m: usize) -> bool {
    if waiting(st, m).is_none() {
        return false;
    }
    run_step(st, m);
    fire(st, None);
    true
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
        // Home: in the flat, or walking up to its door (the walkway one tile from it, the
        // doorway itself).
        Where::HomeDoor => st.home.is_some_and(|rig| {
            p.place == rig.place()
                && (rig.home.rect.contains(p.pos) || p.pos.manhattan(rig.home.door) <= 1)
        }),
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

/// What one slot says: the player's handle for `{you}`, the role's name, or one of
/// `SLOT_FIELDS`.
fn slot_text(st: &AppState, role: &str, field: Option<&str>) -> Option<String> {
    if role == YOU && field.is_none() {
        return Some(st.character.handle.clone());
    }
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
pub(crate) mod tests {
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
                    "mezz": Role(name: "Mezz", title: "fixer", pick: NearestFixer, face: None),
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

    /// The welcome closed and the body seen, a minute on: Mezz's call is up.
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
                Step(id: "hello", when: Talk("mezz"), then: [Log("hi")]),
            ])"#,
        );
        assert!(b.missions[0].roles().contains("mezz"));
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
    fn begin_casts_mezz_and_marlo_and_queues_the_welcome() {
        let st = begun();
        let mezz = person_of(&st, "mezz").expect("mezz is cast");
        let marlo = marlo(&st);
        assert_ne!(mezz, marlo);
        let door = door(&st);
        assert_eq!(Some(mezz), crate::jobs::nearest_fixer(&st, door));
        let people = st.world.as_ref().unwrap().people();
        assert_eq!(people[mezz.0 as usize].name, "Mezz");
        assert_eq!(people[marlo.0 as usize].name, "Marlo");
        assert_eq!(people[marlo.0 as usize].profession, "noodle_cook");
        let d = cast::work_tile(&st, marlo).unwrap().manhattan(door);
        assert!((30..=140).contains(&d), "Marlo works {d} from home");
        assert_eq!(st.missions.feeds.len(), 1);
        let feed = &st.missions.feeds[0];
        assert_eq!(
            (feed.mission, feed.step, feed.from.as_str()),
            (0, 0, "mezz")
        );
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
    fn a_second_city_in_a_session_gives_one_package() {
        let mut st = begun();
        end(&mut st, 0, 0);
        assert_eq!(st.character.count(&ItemId::new("taped_package")), 1);
        // Loading a city keeps the character and restarts the story.
        let city = Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap();
        st.enter_city(city);
        begin(&mut st);
        assert!(st.missions.begun);
        assert_eq!(st.character.count(&ItemId::new("taped_package")), 0);
        end(&mut st, 0, 0);
        assert_eq!(st.character.count(&ItemId::new("taped_package")), 1);
    }

    #[test]
    fn a_roamer_who_dies_still_leaves_a_body() {
        let mut st = begun();
        let (roamer, home) = {
            let world = st.world.as_ref().unwrap();
            let p = world
                .people()
                .iter()
                .find(|p| p.work == sim::Work::Roaming && p.home.is_some() && !world.is_dead(p.id))
                .expect("someone roams");
            (p.id, p.home.unwrap())
        };
        assert_eq!(cast::work_tile(&st, roamer), None);
        st.missions.cast.insert("marlo".into(), roamer);
        end(&mut st, 0, 0);
        assert!(st.world.as_ref().unwrap().is_dead(roamer));
        let body = st.missions.bodies.last().expect("a body");
        assert_eq!(body.person, roamer);
        let city = st.city.as_ref().unwrap();
        assert!(
            city.walkable(StratumKind::Street, body.pos),
            "{:?}",
            body.pos
        );
        assert!(
            !city.building(home).footprint.contains(body.pos),
            "{:?} is inside home",
            body.pos
        );
    }

    #[test]
    fn seeing_the_body_then_a_minute_rings_mezz() {
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
        let call = st.missions.call.as_ref().expect("Mezz rings");
        assert_eq!(
            (call.mission, call.step, call.from.as_str()),
            (0, 3, "mezz")
        );
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
    fn ending_the_call_sets_get_home_and_your_door_closes_it() {
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
        let (place, pos) = marks::resolve(&st, &Mark::HomeOutside, None).unwrap();
        st.player = Some(Player::at(place, pos));
        tick(&mut st);
        assert!(matches!(st.missions.runs[0].state, RunState::Done { .. }));
        assert!(st.log.iter().any(|e| e.text == "The drop · home"));
        // The mail waits for the house call.
        assert!(st.journal.mails.iter().all(|m| m.subject != "lay low"));
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
            r#"Mission(id: "first", title: "First", from: "mezz", steps: [
                Step(id: "start", when: NewGame, then: [Objective("Meet {marlo}")]),
            ]),
            Mission(id: "second", title: "Second", from: "mezz", steps: [
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
            r#"Mission(id: "visit", title: "Visit", from: "mezz", steps: [
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
            r#"Mission(id: "hand", title: "Hand", from: "mezz", steps: [
                Step(id: "get", when: NewGame, then: [Give(item: "taped_package", count: 2)]),
                Step(id: "give", when: Talk("marlo"), then: [Take(item: "taped_package", count: 1)]),
            ])"#,
        );
        begin(&mut st);
        let id = ItemId::new("taped_package");
        assert_eq!(st.character.count(&id), 2);
        let mezz = person_of(&st, "mezz").unwrap();
        let marlo = marlo(&st);
        assert!(listens(&st, marlo) && !listens(&st, mezz));
        send(&mut st, Event::Talked(mezz));
        assert_eq!(st.character.count(&id), 2);
        send(&mut st, Event::Talked(marlo));
        assert_eq!(st.character.count(&id), 1);
        assert!(matches!(st.missions.runs[0].state, RunState::Done { .. }));
        assert!(!listens(&st, marlo));
    }

    #[test]
    fn fill_fills_every_slot() {
        let st = begun();
        let mezz = person_of(&st, "mezz").unwrap();
        let hours = crate::jobs::hours_text(st.world.as_ref().unwrap().work_hours(mezz).unwrap());
        assert_eq!(fill(&st, "{mezz} the {mezz.title}"), "Mezz the fixer");
        assert_eq!(fill(&st, "at {mezz.place}"), "at the corner");
        assert_eq!(fill(&st, "{mezz.hours}"), hours);
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

    /// The shipped cast's picks and a visitor, with these missions.
    fn book_with_visitor(missions: &str) -> Book {
        Book::parse(&format!(
            r#"Story(
                cast: {{
                    "mezz": Role(name: "Mezz", title: "fixer", pick: NearestFixer, face: None),
                    "seren": Role(name: "Seren", title: "doc", pick: Visitor, face: None),
                }},
                faces: {{
                    "silhouette": Face(rows: ["......dd........", "....dmmmmd......"], eye: (255, 255, 255)),
                }},
                missions: [{missions}],
            )"#
        ))
        .unwrap()
    }

    #[test]
    fn at_home_door_fires_at_the_door_and_inside() {
        // Whether `At(HomeDoor)` fires with you standing on the mark.
        let knocks = |mark: Option<Mark>| {
            let mut st = state_home();
            st.book = book(
                r#"Mission(id: "door", title: "Door", from: "mezz", steps: [
                    Step(id: "go", when: NewGame, then: [Objective("Go home")]),
                    Step(id: "knock", when: At(HomeDoor), then: [Log("knock")]),
                ])"#,
            );
            begin(&mut st);
            match mark {
                Some(mark) => {
                    let (place, pos) = marks::resolve(&st, &mark, None).unwrap();
                    st.player = Some(Player::at(place, pos));
                }
                None => {
                    let below = door(&st);
                    on_street(&mut st, below);
                }
            }
            tick(&mut st);
            let done = matches!(st.missions.runs[0].state, RunState::Done { .. });
            assert_eq!(done, st.log.iter().any(|e| e.text == "knock"));
            done
        };
        // Home is home: the walkway at the door, the doorway, anywhere in the flat.
        assert!(knocks(Some(Mark::HomeOutside)));
        assert!(knocks(Some(Mark::HomeDoor)));
        assert!(knocks(Some(Mark::HomeInside(1))));
        assert!(knocks(Some(Mark::HomeBed)));
        // On the street below is not.
        assert!(!knocks(None));
    }

    #[test]
    fn arriving_you_drops_an_aimed_walk() {
        let mut st = state_home();
        st.book = book(
            r#"Mission(id: "cut", title: "Cut", from: "mezz", steps: [
                Step(id: "go", when: NewGame, then: [Objective("Go home")]),
                Step(id: "out", when: At(Home), then: [Arrive(role: "you", at: HomeOutside)]),
            ])"#,
        );
        begin(&mut st);
        let mut map = crate::screens::explore::MapScreen::walking(&mut st);
        let (place, pos) = marks::resolve(&st, &Mark::HomeInside(1), None).unwrap();
        st.player = Some(Player::at(place, pos));
        // A fight on foot, where an aimed walk outlasts the frame.
        crate::net::engage(&mut st, place, true);
        map.walking = Some(crate::screens::explore::Walk {
            path: vec![pos.offset(0, 1)],
            next_at: u64::MAX,
        });
        crate::app::Screen::update(&mut map, &mut st, &[]);
        let out = marks::resolve(&st, &Mark::HomeOutside, None).unwrap();
        let me = st.player.as_ref().unwrap();
        assert_eq!((me.place, me.pos), out);
        assert!(map.walking.is_none(), "no walk goes on from where you were");
    }

    #[test]
    fn a_visitor_from_is_never_uncast() {
        let mut st = state_home();
        st.book = book_with_visitor(
            r#"Mission(id: "call", title: "Call", from: "seren", steps: [
                Step(id: "go", when: NewGame, then: [Objective("Wait")]),
                Step(id: "later", when: After(minutes: 600), then: [Log("later")]),
            ])"#,
        );
        begin(&mut st);
        assert!(!st.missions.cast.contains_key("seren"));
        assert_eq!(st.missions.runs[0].state, RunState::Open);
    }

    #[test]
    fn you_fills_with_the_handle() {
        let mut st = state_home();
        st.character.handle = "D'SHAWN".into();
        assert_eq!(fill(&st, "hello {you}, {you}"), "hello D'SHAWN, D'SHAWN");
        // Only the plain slot: `{you.title}` stays as written.
        assert_eq!(fill(&st, "{you.title}"), "{you.title}");
    }

    /// The drop run through its last step, you standing at your door, the house call's
    /// film queued.
    pub(crate) fn at_your_door() -> AppState {
        let mut st = begun();
        st.character.handle = "VEX".into();
        to_the_call(&mut st);
        end(&mut st, 0, 3);
        let (place, pos) = marks::resolve(&st, &Mark::HomeOutside, None).unwrap();
        st.player = Some(Player::at(place, pos));
        tick(&mut st);
        st
    }

    fn visitor_at(st: &AppState, mark: &Mark) -> Option<String> {
        let (place, pos) = marks::resolve(st, mark, None).unwrap();
        visitors::at(st, place, pos).map(|v| v.role.clone())
    }

    /// The house call's flat as the film opens: dim, Castellan on the couch, Seren at
    /// your rig.
    fn the_stage_is_set(st: &AppState) {
        assert_eq!(scene::running(st).map(|r| r.id()), Some("house_call"));
        assert_eq!(
            visitor_at(st, &Mark::HomeFurniture(citygen::Furniture::Couch)).as_deref(),
            Some("castellan")
        );
        let (place, pos) = marks::resolve(st, &Mark::HomeRig, None).unwrap();
        let seren = st
            .missions
            .visitors
            .iter()
            .find(|v| v.role == "seren")
            .expect("seren is in");
        assert_eq!(seren.place, place);
        assert!(seren.pos.manhattan(pos) <= 1, "{:?} by {pos:?}", seren.pos);
        assert_eq!(st.missions.lights, Some(0.45));
    }

    #[test]
    fn the_drop_ends_at_your_door() {
        let mut st = at_your_door();
        assert!(matches!(st.missions.runs[0].state, RunState::Done { .. }));
        assert_eq!(st.missions.runs[1].state, RunState::Open);
        tick(&mut st);
        the_stage_is_set(&st);
    }

    #[test]
    fn the_house_call_played_alone_sets_its_own_stage() {
        // As the console plays it: no mission step has put anyone in the flat.
        let mut st = begun();
        scene::play(&mut st, "house_call");
        tick(&mut st);
        the_stage_is_set(&st);
    }

    /// Mezz's call over with you already standing on `mark`, home before he told you to
    /// go: the drop's last steps run as the call ends.
    fn home_early(mark: &Mark) -> AppState {
        let mut st = begun();
        st.character.handle = "VEX".into();
        to_the_call(&mut st);
        let (place, pos) = marks::resolve(&st, mark, None).unwrap();
        st.player = Some(Player::at(place, pos));
        scene::tests::see(&mut st);
        end(&mut st, 0, 3);
        st
    }

    #[test]
    fn home_early_the_film_opens_at_your_door() {
        for mark in [Mark::HomeInside(1), Mark::HomeDoor, Mark::HomeBed] {
            let mut st = home_early(&mark);
            assert!(
                matches!(st.missions.runs[0].state, RunState::Done { .. }),
                "{mark:?}: the drop ends at once"
            );
            tick(&mut st);
            assert_eq!(
                scene::running(&st).map(|r| r.id()),
                Some("house_call"),
                "{mark:?}"
            );
            // Cut to the walkway outside, seeing from it.
            let out = marks::resolve(&st, &Mark::HomeOutside, None).unwrap();
            let me = st.player.as_ref().unwrap();
            assert_eq!((me.place, me.pos), out, "{mark:?}");
            assert_eq!(me.vis().get(out.1), Some(&Visibility::Visible));
            assert_eq!(
                visitor_at(&st, &Mark::HomeFurniture(citygen::Furniture::Couch)).as_deref(),
                Some("castellan"),
                "{mark:?}"
            );
            let (_, rig) = marks::resolve(&st, &Mark::HomeRig, None).unwrap();
            let seren = st.missions.visitors.iter().find(|v| v.role == "seren");
            assert!(
                seren.is_some_and(|s| s.pos.chebyshev(rig) <= 1 && s.pos != me.pos),
                "{mark:?}: {seren:?} by {rig:?}"
            );
        }
    }

    #[test]
    fn home_early_on_the_rig_side_seren_still_takes_the_rig() {
        // The film puts you on the walkway before anyone comes in, so the tile you
        // held is hers again.
        let mut st = home_early(&Mark::HomeRig);
        tick(&mut st);
        the_stage_is_set(&st);
        let seren = st
            .missions
            .visitors
            .iter()
            .find(|v| v.role == "seren")
            .expect("seren is in");
        let map = crate::render::map::MapRef::for_place(
            st.city.as_ref().unwrap(),
            &st.interiors,
            &st.constructs,
            seren.place,
            st.minute_of_day(),
        )
        .unwrap();
        assert!(crate::render::map::TileMap::walkable(&map, seren.pos));
    }

    #[test]
    fn skipping_the_house_call() {
        let mut st = at_your_door();
        tick(&mut st);
        assert!(scene::tests::press(&mut st, crossterm::event::KeyCode::Esc));
        tick(&mut st);
        assert_eq!(scene::running(&st).map(|r| r.id()), Some("seren_aside"));
        assert!(scene::tests::press(&mut st, crossterm::event::KeyCode::Esc));
        assert!(scene::running(&st).is_none());
        assert!(st.missions.visitors.is_empty());
        assert_eq!(st.missions.lights, None);
        assert_eq!(
            st.missions.runs[1].objectives.last().map(String::as_str),
            Some("Sleep · Castellan will be in touch")
        );
        assert!(st.journal.mails.iter().all(|m| m.subject != "lay low"));
        advance(&mut st, 2);
        tick(&mut st);
        let mail = st.journal.mails.last().unwrap();
        assert_eq!(
            (mail.from.as_str(), mail.subject.as_str()),
            ("Mezz", "lay low")
        );
        assert_eq!(mail.body.len(), 3, "{:?}", mail.body);
        assert_eq!(mail.body[0], "You made it home. Good.");
        // Every line of both scenes is in the log, once.
        for id in ["house_call", "seren_aside"] {
            for beat in &st.book.scenes[id].beats {
                let Beat::Say { text, .. } = beat else {
                    continue;
                };
                let text = fill(&st, text);
                let n = st.log.iter().filter(|e| e.text.contains(&text)).count();
                assert_eq!(n, 1, "{text}");
            }
        }
    }

    #[test]
    fn playing_the_house_call_through() {
        // At your door, the film queued and not yet begun.
        let mut st = home_early(&Mark::HomeOutside);
        assert!(scene::running(&st).is_none());
        // A wait after the film's last beat, so the run is seen with the door shut.
        let film = st.book.scenes.get_mut("house_call").unwrap();
        film.beats.push(Beat::Wait(1.0));
        let last = film.beats.len() - 1;
        let door = marks::resolve(&st, &Mark::HomeDoor, None).unwrap();
        let (mut opened, mut shut) = (false, false);
        for _ in 0..20_000 {
            match scene::running(&st) {
                Some(r) if r.id() == "seren_aside" => break,
                Some(r) if r.id() == "house_call" => {
                    opened |= r.doors() == [door];
                    shut |= r.beat() == last && r.doors().is_empty();
                }
                _ => {}
            }
            let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) else {
                panic!("no world");
            };
            world.advance(city, world.tick() + 1);
            tick(&mut st);
        }
        assert_eq!(scene::running(&st).map(|r| r.id()), Some("seren_aside"));
        let (place, pos) = marks::resolve(&st, &Mark::HomeInside(2), None).unwrap();
        let me = st.player.as_ref().unwrap();
        assert_eq!((me.place, me.pos), (place, pos));
        assert!(st.missions.visitors.iter().all(|v| v.role != "castellan"));
        let seren = st
            .missions
            .visitors
            .iter()
            .find(|v| v.role == "seren")
            .unwrap();
        assert_eq!(seren.place, place);
        assert!(seren.pos.manhattan(pos) <= 1, "{:?} by {pos:?}", seren.pos);
        assert!(opened, "the door opens");
        assert!(shut, "and closes behind Castellan");
    }

    #[test]
    fn mezz_not_dez() {
        let st = begun();
        let book = format!("{:?}", st.book);
        assert!(!book.contains(concat!("D", "ez")) && !book.contains(concat!("d", "ez")));
        let fixer = person_of(&st, "mezz").expect("mezz is cast");
        assert_eq!(
            st.world.as_ref().unwrap().people()[fixer.0 as usize].name,
            "Mezz"
        );
    }
}
