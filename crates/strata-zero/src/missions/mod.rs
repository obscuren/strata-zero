//! Story missions: authored chains of steps in `flavor/missions.ron`, cast onto the
//! city's people and moved on by what the game reports and what the engine sees.
//!
//! Owns the runtime (`Missions`, `Run`, `begin`, `send`, `tick`, `end`), the book's
//! types (`content`), casting (`cast`), what a step does (`effects`), and what the
//! player sees: the HOLO feed and call (`feed`, `call`, `face`), the scenes played on
//! the map (`scene`, with `marks` and `visitors`), the news put out (`news`) and its
//! broadcast on the holo set (`broadcast`), the bubbles that leave you the keys
//! (`asides`), the daze (`daze`), the JOBS lines (`ui`) and an item's readings and
//! their card (`reading`).
//! No mission, step or role id appears in Rust outside tests; the book names them.
//!
//! Adding a block (spec `docs/superpowers/specs/2026-10-31-story-missions-design.md`, 15):
//! - A trigger: a variant on `Trigger` (and `Where` for a place), its arm in
//!   `Trigger::fires`, its names in `Book::validate`; an event also gets a variant on
//!   `Event` and one `send` where the game does it. Test with a small book.
//! - An effect: a variant on `Effect`, its arm in `effects::apply`, its names in
//!   `Book::validate`. Test it.
//! - A pick: a variant on `Pick` and its arm in `cast::pick`.
//! - A need: a variant on `Need` and its arm in `holds`.
//! - A reading's row: a variant on `Row`, its texts in `Reading::texts` and its lines
//!   in `reading::lines`.

pub mod asides;
pub mod broadcast;
pub mod bubbles;
pub mod call;
pub mod cast;
pub mod content;
pub mod daze;
pub mod effects;
pub mod face;
pub mod feed;
pub mod film;
pub mod marks;
pub mod news;
pub mod overlay;
pub mod reading;
pub mod scene;
pub mod ui;
pub mod visitors;

pub use content::*;
pub use reading::{item_desc, reading};

use crate::app::AppState;
use crate::render::Visibility;
use crate::runs::ids::StepRef;
use citygen::{Place, Pos, StratumKind};
use game::{ItemId, SkillId, Slot};
use sim::PersonId;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The session's story: who plays each role, where each mission stands, the bodies
/// it left and what the HOLO has waiting. Reset with the city.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Missions {
    pub cast: BTreeMap<String, PersonId>,
    /// One per mission, in book order; saved by mission id.
    #[serde(with = "crate::runs::ids::runs")]
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
    /// Scenes queued by `Effect::Scene`: the step that queued it (none from the
    /// console), scene id.
    pub scenes: VecDeque<(Option<StepRef>, String)>,
    /// The scene playing.
    // Never saved: a run is written only when no scene is in flight (it holds cloned content).
    #[serde(skip)]
    pub scene: Option<scene::SceneRun>,
    /// The news item put out last, fresh or not.
    pub news: Option<news::NewsRun>,
    /// Steps whose news item a newer one replaced before they ended. The next news tick
    /// ends them.
    pub news_ends: Vec<StepRef>,
    /// The daze running, if one is.
    pub daze: Option<daze::Daze>,
    /// The bubbles up over the map that leave you the keys, oldest first.
    // Never saved: asides last seconds, and carry a ratatui colour.
    #[serde(skip)]
    pub asides: Vec<asides::Aside>,
    /// The items the story has unsealed, by id: `Need::Unsealed`.
    pub unsealed: BTreeSet<String>,
    /// The items whose telling reading has been shown, by id: `Need::Read`.
    pub read: BTreeSet<String>,
    /// Roles `Release` let go, waiting for you to leave their work building.
    pub releasing: BTreeSet<String>,
    /// The lines the story left under a tile's Look, one a tile.
    pub notes: Vec<(Place, Pos, String)>,
    /// The tray held out, until a piece is taken from it.
    pub tray: Option<TrayOffer>,
    /// The piece taken from a tray, on someone's tab.
    pub tab: Option<Tab>,
}

impl Missions {
    /// Drops the pieces `c` lacks from the tray held out and the tab, as a run saved by
    /// a build that had them comes back in one that doesn't; a tray left empty goes. A
    /// line for each. (`Left.worn` stays: it is what you wore then, only ever compared,
    /// and named through a lookup that falls back.)
    pub fn forget_unknown(&mut self, c: &game::Content) -> Vec<String> {
        let mut lost = Vec::new();
        let mut known = |id: &ItemId| {
            let known = c.item(id).is_some();
            if !known {
                lost.push(format!("an item ({id})"));
            }
            known
        };
        if let Some(t) = self.tray.as_mut() {
            t.items.retain(&mut known);
            if t.items.is_empty() {
                self.tray = None;
            }
        }
        if self.tab.as_ref().is_some_and(|t| !known(&t.item)) {
            self.tab = None;
        }
        lost
    }
}

/// A tray of pieces held out by a role (doctor spec §8): one to take, on a tab.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TrayOffer {
    /// The step that held it out.
    pub story: StepRef,
    /// The role holding it out.
    pub from: String,
    /// The role whose tab pays.
    pub tab: String,
    pub title: String,
    pub line: String,
    /// The set that fitted who you were when it was held out, kept.
    pub items: Vec<ItemId>,
    /// It has been put up over the map; esc leaves it waiting, talking opens it again.
    pub shown: bool,
}

/// A piece on someone's tab: in its doc's chair it goes in free, once.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Tab {
    pub doc: PersonId,
    pub item: ItemId,
    /// The name of whoever pays.
    pub payer: String,
    pub spent: bool,
}

/// Where one mission stands.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// The game minute its first step fired: when the mission began.
    pub began: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Body {
    pub person: PersonId,
    pub name: String,
    pub place: Place,
    pub pos: Pos,
    /// What look mode says of it.
    pub look: String,
    /// You have been within `news::NEAR` tiles of it on its place.
    pub near: bool,
    /// When you walked away from it, once: recorded by the news tick.
    pub left: Option<Left>,
}

/// You walking away from a body (morning news spec §7): the first time you were more
/// than `news::AWAY` tiles from it after having been near.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Left {
    /// The clock: hour, minute.
    pub at: (u8, u8),
    /// The eight-point heading from the body to you: `north`, `north-east` …
    pub heading: String,
    /// What you wore on your torso then: the jacket the camera saw (doctor spec §7).
    pub worn: Option<ItemId>,
}

/// A HOLO feed waiting to be shown, its text filled.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FeedView {
    /// The step that opened it; none from the console.
    pub story: Option<StepRef>,
    /// The role it comes from.
    pub from: String,
    pub text: Vec<String>,
    /// The label after `space`.
    pub close: String,
}

/// A HOLO call, ringing or talking, its lines filled.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Call {
    /// The step that rang it; none from the console.
    pub story: Option<StepRef>,
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
        .map(|m| fresh_run(&st.book, &st.missions.cast, m, now))
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
    let given = st.book.gives(&st.content);
    for id in given {
        let held = st.character.count(&id);
        game::remove_from_pack(&mut st.character, &id, held);
    }
    send(st, Event::NewGame);
}

/// Mission `m`'s run as the story begins at game minute `now`: waiting on its first
/// step, or `Uncast` when a role it needs has nobody in `cast`.
pub fn fresh_run(book: &Book, cast: &BTreeMap<String, PersonId>, m: &Mission, now: u64) -> Run {
    Run {
        at: 0,
        since: now,
        state: if book.needs_cast(m).iter().all(|r| cast.contains_key(r)) {
            RunState::Waiting
        } else {
            RunState::Uncast
        },
        objectives: Vec::new(),
        ended: BTreeSet::new(),
        began: None,
    }
}

/// Whether the story has unsealed the item, so the pack may open it.
pub fn unsealed(st: &AppState, id: &ItemId) -> bool {
    st.missions.unsealed.contains(&id.0)
}

/// The person holding out the tray, while one waits.
pub fn tray_from(st: &AppState) -> Option<PersonId> {
    st.missions
        .tray
        .as_ref()
        .and_then(|t| person_of(st, &t.from))
}

/// Takes the tray's `i`th piece into your pack, on the tab of its doc, the one holding
/// it out; the rest are put away. Nothing without a tray or its doc.
pub fn take_from_tray(st: &mut AppState, i: usize) {
    let Some(tray) = st.missions.tray.as_ref() else {
        return;
    };
    let (Some(item), Some(doc)) = (tray.items.get(i).cloned(), person_of(st, &tray.from)) else {
        return;
    };
    let payer = role_name(st, &tray.tab);
    let name = st
        .content
        .item(&item)
        .map_or_else(|| item.to_string(), |it| it.name.clone());
    st.character.give_item(&st.content, item.clone(), 1);
    st.missions.tab = Some(Tab {
        doc,
        item,
        payer,
        spent: false,
    });
    st.missions.tray = None;
    st.story(format!("`{name}` ×1 · in your pack"));
}

/// The tab that pays for this piece in the chair you sit in: not yet spent, on that
/// piece, with that chair's doc.
pub fn tab_for<'a>(st: &'a AppState, item: &ItemId) -> Option<&'a Tab> {
    st.missions
        .tab
        .as_ref()
        .filter(|t| !t.spent && &t.item == item && st.chair.is_some_and(|c| c.doc == t.doc))
}

/// The tab dev free chrome spends on this piece: in a chair, the one `tab_for` finds;
/// in none, the unspent tab on that piece.
pub fn free_tab_for<'a>(st: &'a AppState, item: &ItemId) -> Option<&'a Tab> {
    match st.chair {
        Some(_) => tab_for(st, item),
        None => st
            .missions
            .tab
            .as_ref()
            .filter(|t| !t.spent && &t.item == item),
    }
}

/// Reports an event: every mission waiting on it moves on.
pub fn send(st: &mut AppState, ev: Event) {
    fire(st, Some(&ev));
}

/// The story's beat in the explore update: the held let go once you are out, the
/// conditions checked, the scene played, the call's ring timed.
pub fn tick(st: &mut AppState) {
    effects::release_waiting(st);
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

/// `end` for what a step or the console put out: the console's ends no step, but the
/// story still moves on whatever now holds.
pub fn end_of(st: &mut AppState, story: Option<StepRef>) {
    match story {
        Some(r) => end(st, r.mission, r.step),
        None => fire(st, None),
    }
}

/// Something changed off the map's frame (a Look that told, an action in the deck):
/// every mission whose step now holds moves on, before the map's next frame.
pub fn settle(st: &mut AppState) {
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

/// Runs the effects of the step mission `m` waits on and moves it to the next; the run
/// is saved on the map's next frame that can take it.
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
    if at == 0 {
        run.began = Some(now);
    }
    run.at = at + 1;
    run.since = now;
    run.state = if run.at >= last {
        RunState::Done { at: now }
    } else {
        RunState::Open
    };
    st.save_due = true;
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
            Trigger::Holds(need) => holds(st, Some(m), need),
            // The tab with the role's person as its doc is spent, and you are up out of
            // the chair.
            Trigger::Chromed(role) => {
                st.chair.is_none()
                    && st
                        .missions
                        .tab
                        .as_ref()
                        .is_some_and(|t| t.spent && person_of(st, role) == Some(t.doc))
            }
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

/// The player is at the place: inside their flat, on the street by a role's work, or
/// beside the role's person with them in sight.
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
        // On the street or a floor, wherever the role's person is drawn, and seen: never
        // through a wall. Inside a building you are not in, they are nowhere near you,
        // so their floor is never worked out.
        Where::By { role, within } => person_of(st, role)
            .filter(|id| !inside_elsewhere(st, p.place, *id))
            .and_then(|id| crate::interiors::drawn_at(st, id))
            .is_some_and(|(place, pos)| {
                p.place == place
                    && p.vis().get(pos) == Some(&Visibility::Visible)
                    && p.pos.manhattan(pos) <= *within
            }),
    }
}

/// The person is inside a building and off the street, and `here` is none of its
/// floors: wherever its floors draw them, it isn't here.
fn inside_elsewhere(st: &AppState, here: Place, id: PersonId) -> bool {
    let Some(world) = st.world.as_ref() else {
        return false;
    };
    let inside = match world.people().get(id.0 as usize).map(|q| &q.whereabouts) {
        Some(sim::Whereabouts::Inside(b)) => *b,
        _ => return false,
    };
    world.pos_of(id).is_none()
        && !matches!(here, Place::Floor { building, .. } if building == inside)
}

/// Whether a need holds now (doctor spec §2), asked for mission `mission`: `Late`
/// counts from when that mission began, so asked for none it never holds.
pub fn holds(st: &AppState, mission: Option<usize>, need: &Need) -> bool {
    let torso = || st.character.equipped(Slot::Torso);
    match need {
        Need::Skill(id) => st.character.has_skill(&SkillId::new(id)),
        Need::Read(item) => st.missions.read.contains(item),
        Need::Carries(item) => st.character.count(&ItemId::new(item)) > 0,
        Need::Unsealed(item) => st.missions.unsealed.contains(item),
        Need::Changed => news::jacket_seen(st).is_some_and(|j| torso() != Some(&j)),
        Need::Unchanged => news::jacket_seen(st).is_some_and(|j| torso() == Some(&j)),
        Need::Late(minute) => mission
            .and_then(|m| st.missions.runs.get(m))
            .and_then(|r| r.began)
            .is_some_and(|began| st.net.now(st) > first_at(st, began, *minute)),
        Need::Not(need) => !holds(st, mission, need),
        Need::All(needs) => needs.iter().all(|n| holds(st, mission, n)),
    }
}

/// The first game minute at or after `from` whose clock shows `minute` of the day.
fn first_at(st: &AppState, from: u64, minute: u16) -> u64 {
    let day = u64::from(sim::DAY);
    // The clock at `from`, counted back from the clock now.
    let back = st.net.now(st).saturating_sub(from) % day;
    let then = (u64::from(st.minute_of_day()) + day - back) % day;
    from + (u64::from(minute) + day - then) % day
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
    match field {
        "title" => return Some(r.title.clone()),
        "spaced" => return Some(spaced(&r.name)),
        _ => {}
    }
    let who = person_of(st, role)?;
    match field {
        "place" => Some(cast::place_of(st, who)),
        "district" => Some(cast::district_of(st, who)),
        "hours" => Some(cast::hours_of(st, who)),
        _ => None,
    }
}

/// A name for a sign: each word's letters upper-case with a space between them, the
/// words three spaces apart.
fn spaced(name: &str) -> String {
    name.split_whitespace()
        .map(|w| {
            w.chars()
                .flat_map(char::to_uppercase)
                .map(String::from)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("   ")
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
    pub(crate) fn on_street(st: &mut AppState, pos: Pos) {
        let city = st.city.as_ref().unwrap();
        let mut p = Player::at(STREET, pos);
        p.update_fov(
            &MapRef::stratum_at(city, StratumKind::Street, st.minute_of_day()),
            0,
        );
        st.player = Some(p);
    }

    /// Stands the player on a tile of a place as it is drawn now (a floor once walked
    /// onto), having looked around.
    pub(crate) fn stand_seeing(st: &mut AppState, place: Place, pos: Pos) {
        let city = st.city.as_ref().unwrap();
        let minute = st.minute_of_day();
        let map = MapRef::for_place(city, &st.interiors, &st.constructs, place, minute)
            .expect("the place is drawn");
        let mut p = Player::at(place, pos);
        p.update_fov(&map, 0);
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
    fn a_name_slot_fills_from_the_book_when_nobody_plays_the_role() {
        let mut st = begun();
        st.book.cast.insert(
            "zed".into(),
            Role {
                name: "Zed".into(),
                title: "fixer".into(),
                pick: Pick::NearestFixer,
                face: None,
                tint: None,
                look: None,
            },
        );
        assert!(person_of(&st, "zed").is_none());
        assert_eq!(
            fill(&st, "{zed} · {zed.spaced} · {zed.title} · {zed.place}"),
            "Zed · Z E D · fixer · {zed.place}"
        );
        assert_eq!(spaced("Ab Cd"), "A B   C D");
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
            (feed.story, feed.from.as_str()),
            (Some(StepRef::new(0, 0)), "mezz")
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
        let roles: Vec<&str> = a.missions.cast.keys().map(String::as_str).collect();
        assert_eq!(roles, ["coz", "marlo", "mezz", "odile"]);
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
            assert!(
                ["mezz", "marlo"]
                    .iter()
                    .all(|r| st.missions.cast.contains_key(*r)),
                "seed {seed}: {:?}",
                st.missions.cast
            );
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
            (call.story, call.from.as_str()),
            (Some(StepRef::new(0, 3)), "mezz")
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
        let cast = st.missions.cast.clone();
        end(&mut st, 0, 0);
        assert!(st.missions.begun && !st.missions.bodies.is_empty());
        let city = st.city.clone().unwrap();
        st.enter_city(city);
        assert!(!st.missions.begun);
        assert!(st.missions.cast.is_empty() && st.missions.runs.is_empty());
        assert!(st.missions.bodies.is_empty() && st.missions.feeds.is_empty());
        begin(&mut st);
        assert_eq!(st.missions.cast, cast);
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
        let want = city.district_at(tile).unwrap();
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
        to_your_door(&mut st);
        st
    }

    /// `at_your_door` in any city whose story has begun.
    fn to_your_door(st: &mut AppState) {
        st.character.handle = "VEX".into();
        to_the_call(st);
        end(st, 0, 3);
        let (place, pos) = marks::resolve(st, &Mark::HomeOutside, None).unwrap();
        st.player = Some(Player::at(place, pos));
        tick(st);
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

    #[test]
    fn the_house_call_played_alone_ends_with_seren_s_aside() {
        let mut st = begun();
        scene::play(&mut st, "house_call");
        tick(&mut st);
        assert!(scene::tests::press(&mut st, crossterm::event::KeyCode::Esc));
        tick(&mut st);
        assert_eq!(scene::running(&st).map(|r| r.id()), Some("seren_aside"));
        assert!(scene::tests::press(&mut st, crossterm::event::KeyCode::Esc));
        tick(&mut st);
        assert!(scene::running(&st).is_none());
        assert!(st.missions.visitors.is_empty(), "Seren has gone");
        assert_eq!(st.missions.lights, None);
        // Played for no step: the house call's mission has not moved.
        assert!(st.missions.runs[1].ended.is_empty());
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

    /// The house call skipped and Mezz's mail landed, the HOLO feed read: the night
    /// ahead, with the minute the morning's eight hours count from.
    fn after_the_mail() -> (AppState, u64) {
        let mut st = at_your_door();
        let since = to_the_mail(&mut st);
        (st, since)
    }

    /// From your door, the house call skipped and Mezz's mail landed, the HOLO feed
    /// read: the minute the morning's eight hours count from.
    fn to_the_mail(st: &mut AppState) -> u64 {
        for _ in 0..2 {
            tick(st);
            scene::tests::press(st, crossterm::event::KeyCode::Esc);
        }
        st.missions.feeds.clear();
        advance(st, 2);
        tick(st);
        assert!(st.journal.mails.iter().any(|m| m.subject == "lay low"));
        assert_eq!(st.missions.runs[1].state, RunState::Open);
        let since = st.missions.runs[1].since;
        assert_eq!(
            crate::rest::now(st),
            since,
            "the mail starts the eight hours"
        );
        since
    }

    /// The house call still open on its Sleep objective, nobody calling.
    fn waiting_on_the_call(st: &AppState) {
        let run = &st.missions.runs[1];
        assert_eq!(run.state, RunState::Open);
        assert_eq!(
            run.objectives.last().map(String::as_str),
            Some("Sleep · Castellan will be in touch")
        );
        assert!(st.missions.call.is_none());
    }

    /// Answers the call through every line and hangs up.
    fn answer_through(st: &mut AppState) {
        for _ in 0..40 {
            if st.missions.call.is_none() {
                return;
            }
            let space = crate::screens::testutil::key(crossterm::event::KeyCode::Char(' '));
            assert!(call::key(st, &space));
        }
        panic!("the call never ended");
    }

    #[test]
    fn castellan_calls_in_the_morning_awake() {
        let (mut st, since) = after_the_mail();
        let mut rang = None;
        for _ in 0..600 {
            crate::clock::run(
                &mut st,
                Some(sim::TICKS_PER_MINUTE),
                crate::clock::PATHS_PER_FRAME,
            );
            if st.missions.call.is_some() {
                rang = Some(crate::rest::now(&st));
                break;
            }
            waiting_on_the_call(&st);
        }
        assert_eq!(rang, Some(since + 480));
        let call = st.missions.call.as_ref().unwrap();
        assert_eq!(call.from, "castellan");
        assert!(call.answered.is_none(), "ringing");
        assert!(
            call.lines[0].starts_with("Morning, VEX."),
            "{:?}",
            call.lines
        );
        assert!(call.lines.iter().all(|l| !l.contains('{')));
        answer_through(&mut st);
        assert!(matches!(st.missions.runs[1].state, RunState::Done { .. }));
    }

    #[test]
    fn castellan_wakes_you_in_the_morning() {
        use crate::rest::{self, Kind};
        let (mut st, since) = after_the_mail();
        rest::start(&mut st, Kind::Sleep, since + 1200);
        assert!(st.rest.is_some(), "asleep");
        for _ in 0..400 {
            if st.rest.is_none() {
                break;
            }
            rest::frame_within(&mut st, std::time::Duration::from_secs(60));
            if st.rest.is_some() {
                waiting_on_the_call(&st);
            }
        }
        assert!(st.rest.is_none(), "the call woke you");
        assert_eq!(rest::now(&st), since + 480, "the same minute as awake");
        let (h, m) = st.clock();
        let wake = format!("A call wakes you at {h:02}:{m:02}.");
        assert_eq!(
            st.log.iter().last().map(|e| e.text.as_str()),
            Some(wake.as_str())
        );
        let call = st.missions.call.as_ref().expect("the call is up");
        assert_eq!(call.from, "castellan");
        assert!(call.answered.is_none(), "and ringing");
        answer_through(&mut st);
        assert!(matches!(st.missions.runs[1].state, RunState::Done { .. }));
    }

    /// Eight hours after Mezz's mail, in the flat: Castellan's morning call ringing.
    pub(crate) fn the_morning_call() -> AppState {
        let (mut st, _) = after_the_mail();
        to_the_morning_call(&mut st);
        st
    }

    /// From Mezz's mail, eight hours on: Castellan's morning call ringing.
    fn to_the_morning_call(st: &mut AppState) {
        advance(st, 480);
        tick(st);
        let call = st.missions.call.as_ref().expect("the call is up");
        assert_eq!(call.from, "castellan");
    }

    /// The morning call answered through and the waking scene run out: the news is on.
    pub(crate) fn the_news_is_on(st: &mut AppState) {
        answer_through(st);
        tick(st);
        assert_eq!(scene::running(st).map(|r| r.id()), Some("waking"));
        scene::skip(st);
        assert!(news::current(st).is_some(), "the news is on");
    }

    /// Mezz's mails about the news.
    fn news_mails(st: &AppState) -> Vec<&crate::credentials::Mail> {
        st.journal
            .mails
            .iter()
            .filter(|m| m.subject == "the news")
            .collect()
    }

    /// Mezz's mail has come, once, unread, and the morning after waits on reading it.
    fn mezz_saw_the_news(st: &AppState) {
        let mails = news_mails(st);
        assert_eq!(mails.len(), 1, "{mails:?}");
        assert_eq!(mails[0].from, "Mezz");
        assert_eq!(
            mails[0].body,
            ["Saw the news.", "Don't go out looking like you."]
        );
        assert!(!mails[0].read);
        assert_eq!(
            mails[0].story,
            Some(StepRef::new(2, 2)),
            "the morning after's `saw`"
        );
        let run = &st.missions.runs[2];
        assert_eq!(run.state, RunState::Open);
        assert_eq!(
            run.objectives.last().map(String::as_str),
            Some("Read Mezz's mail · on your rig")
        );
    }

    /// Mezz's mail read, as the rig's `mail n` and the JOBS card read it.
    fn read_mezzs_mail(st: &mut AppState) {
        let i = st
            .journal
            .mails
            .iter()
            .position(|m| m.subject == "the news");
        crate::credentials::mail_read(st, i.expect("Mezz's mail"));
    }

    #[test]
    fn the_morning_after_asks_for_mezzs_mail() {
        let mut st = the_morning_call();
        the_news_is_on(&mut st);
        news::switch_off(&mut st);
        mezz_saw_the_news(&st);
        // Time alone moves nothing on: the doctor waits, the package stays shut.
        advance(&mut st, 30);
        tick(&mut st);
        mezz_saw_the_news(&st);
        let doctor = mission_at(&st, "doctor");
        assert_eq!(st.missions.runs[doctor].state, RunState::Waiting);
        assert!(!unsealed(&st, &ItemId::new("taped_package")));
        assert!(st.journal.mails.iter().all(|m| m.subject != "ten"));
    }

    #[test]
    fn reading_mezzs_mail_lets_seren_in() {
        let mut st = the_morning_call();
        the_news_is_on(&mut st);
        news::switch_off(&mut st);
        let (morning, doctor) = (mission_at(&st, "morning_after"), mission_at(&st, "doctor"));
        assert_eq!(st.missions.runs[doctor].state, RunState::Waiting);
        read_mezzs_mail(&mut st);
        // The package is yours to open, and Seren breaks into your set.
        assert!(unsealed(&st, &ItemId::new("taped_package")));
        let seren = st.missions.asides.last().expect("Seren on the set");
        assert!(seren.label.ends_with("▚▞▚ S"), "{}", seren.label);
        assert_eq!(seren.text, "Doctor's in by ten. Eat first.");
        assert_eq!(
            st.missions.runs[morning]
                .objectives
                .last()
                .map(String::as_str),
            Some("Open the package · it's in your pack")
        );
        assert_eq!(st.missions.runs[morning].state, RunState::Open);
        // Her mail lands with the code, and the doctor opens on it.
        let doc = person_of(&st, "odile").expect("odile is cast");
        let b = cast::work_building(&st, doc).expect("she works in her clinic");
        let code = crate::credentials::street_code(&st, b);
        let mail = st.journal.mails.iter().find(|m| m.subject == "ten");
        let mail = mail.expect("Seren's mail");
        assert_eq!(mail.from, "Seren");
        assert!(
            mail.body.iter().any(|l| l.contains(&format!("{code:04}"))),
            "{:?}",
            mail.body
        );
        assert_eq!(st.missions.runs[doctor].state, RunState::Open);
        let see = format!(
            "See the doctor · {} · {} · by ten",
            cast::place_of(&st, doc),
            cast::district_of(&st, doc)
        );
        assert_eq!(st.missions.runs[doctor].objectives.last(), Some(&see));
    }

    #[test]
    fn the_house_call_is_not_done_while_the_call_rings() {
        let st = the_morning_call();
        let run = &st.missions.runs[1];
        assert_eq!(run.state, RunState::Open);
        assert_eq!(
            run.objectives.last().map(String::as_str),
            Some("Sleep · Castellan will be in touch")
        );
        assert_eq!(st.missions.runs[2].state, RunState::Waiting);
        assert!(st.missions.scenes.is_empty(), "no scene under the call");
    }

    #[test]
    fn the_morning_after_wakes_dizzy_and_puts_out_the_news() {
        let mut st = the_morning_call();
        answer_through(&mut st);
        // Done only now the call has ended.
        assert!(matches!(st.missions.runs[1].state, RunState::Done { .. }));
        assert!(st.log.iter().any(|e| e.text == "House call · morning"));
        assert_eq!(st.missions.runs[2].state, RunState::Open);
        tick(&mut st);
        assert_eq!(scene::running(&st).map(|r| r.id()), Some("waking"));
        assert!(st.missions.daze.is_none() && news::current(&st).is_none());
        scene::skip(&mut st);
        assert!(scene::running(&st).is_none());
        assert!(
            st.log.iter().any(|e| e.text.contains("…so that happened.")),
            "{:?}",
            st.log.iter().map(|e| &e.text).collect::<Vec<_>>()
        );
        assert!(st.missions.daze.is_some(), "dizzy");
        let n = news::current(&st).expect("the news is on");
        assert!(n.on && !n.ended);
        assert_eq!(
            (n.id.as_str(), n.story),
            ("marlo", Some(StepRef::new(2, 1)))
        );
        assert_eq!(
            st.missions.runs[2].objectives.last().map(String::as_str),
            Some("Catch the news on the holo")
        );
        // In the flat, fresh: the step waits.
        news::tick(&mut st, true);
        assert!(news_mails(&st).is_empty());
        assert_eq!(st.missions.runs[2].state, RunState::Open);
    }

    #[test]
    fn switching_the_set_off_sends_mezzs_mail() {
        let mut st = the_morning_call();
        the_news_is_on(&mut st);
        news::switch_off(&mut st);
        mezz_saw_the_news(&st);
        assert!(!news::current(&st).unwrap().on);
        news::tick(&mut st, true);
        mezz_saw_the_news(&st);
    }

    #[test]
    fn leaving_the_flat_sends_it_too() {
        let mut st = the_morning_call();
        the_news_is_on(&mut st);
        news::tick(&mut st, true);
        assert!(news_mails(&st).is_empty());
        let (place, pos) = marks::resolve(&st, &Mark::HomeOutside, None).unwrap();
        st.player = Some(Player::at(place, pos));
        news::tick(&mut st, true);
        mezz_saw_the_news(&st);
        assert!(news::current(&st).unwrap().on, "the set stays on");
        news::tick(&mut st, true);
        mezz_saw_the_news(&st);
    }

    #[test]
    fn out_of_the_flat_when_it_comes_sends_it_at_once() {
        let mut st = the_morning_call();
        let below = door(&st);
        on_street(&mut st, below);
        the_news_is_on(&mut st);
        news::tick(&mut st, true);
        mezz_saw_the_news(&st);
        assert!(news::current(&st).unwrap().on, "on for when you get home");
    }

    #[test]
    fn staying_in_until_it_is_stale_sends_it_then() {
        let mut st = the_morning_call();
        the_news_is_on(&mut st);
        let fresh = news::current(&st).unwrap().fresh;
        advance(&mut st, fresh - 1);
        news::tick(&mut st, true);
        assert!(news_mails(&st).is_empty());
        advance(&mut st, 1);
        news::tick(&mut st, true);
        mezz_saw_the_news(&st);
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

    #[test]
    fn a_restart_takes_back_the_shard_too() {
        let mut st = state_home();
        let shard = ItemId::new("ledger_shard");
        assert!(st.book.gives(&st.content).contains(&shard));
        st.character.give(shard.clone(), 1);
        st.missions.unsealed.insert("taped_package".into());
        st.missions.read.insert("ledger_shard".into());
        // A new story: its record is fresh, and begin takes back what it gave.
        st.missions = Missions::default();
        assert!(st.missions.unsealed.is_empty() && st.missions.read.is_empty());
        begin(&mut st);
        assert_eq!(st.character.count(&shard), 0);
    }

    #[test]
    fn skill_carries_unsealed_and_read_hold_as_named() {
        let mut st = AppState::new();
        let skill = Need::Skill("hacking".into());
        st.character.skills.clear();
        assert!(!holds(&st, None, &skill));
        st.character.skills.insert(SkillId::new("hacking"));
        assert!(holds(&st, None, &skill));
        let package = ItemId::new("taped_package");
        let carries = Need::Carries("taped_package".into());
        assert!(!holds(&st, None, &carries));
        st.character.give(package.clone(), 1);
        assert!(holds(&st, None, &carries));
        game::remove_from_pack(&mut st.character, &package, 1);
        assert!(!holds(&st, None, &carries));
        let unsealed = Need::Unsealed("taped_package".into());
        assert!(!holds(&st, None, &unsealed));
        st.missions.unsealed.insert("taped_package".into());
        assert!(holds(&st, None, &unsealed));
        let read = Need::Read("ledger_shard".into());
        assert!(!holds(&st, None, &read));
        st.missions.read.insert("ledger_shard".into());
        assert!(holds(&st, None, &read));
        assert!(!holds(&st, None, &Need::Read("taped_package".into())));
        // A new story forgets both.
        let fresh = Missions::default();
        assert!(fresh.unsealed.is_empty() && fresh.read.is_empty());
    }

    #[test]
    fn not_and_all_combine() {
        let st = AppState::new();
        let yes = Need::All(vec![]);
        let no = Need::Not(Box::new(yes.clone()));
        assert!(holds(&st, None, &yes));
        assert!(!holds(&st, None, &no));
        assert!(!holds(&st, None, &Need::All(vec![yes.clone(), no.clone()])));
        assert!(holds(&st, None, &Need::All(vec![yes.clone(), yes.clone()])));
        let mixed = Need::All(vec![yes, no]);
        assert!(holds(&st, None, &Need::Not(Box::new(mixed))));
        // As the book writes them.
        let need: Need = ron::from_str(r#"Not(All([Skill("hacking"), Late(600)]))"#).unwrap();
        assert_eq!(
            need,
            Need::Not(Box::new(Need::All(vec![
                Need::Skill("hacking".into()),
                Need::Late(600),
            ])))
        );
    }

    /// A city-less game's clock set to `h:m` on day `day`, the game starting at 22:00 on
    /// day 1.
    fn set_clock(st: &mut AppState, day: u64, h: u64, m: u64) {
        let minute = (day - 1) * 1440 + h * 60 + m - 22 * 60;
        st.tick = minute * sim::TICKS_PER_MINUTE;
        assert_eq!(st.clock(), (h as u8, m as u8));
    }

    #[test]
    fn late_is_past_the_first_such_clock_after_the_mission_began() {
        let mut st = AppState::new();
        st.missions.runs.push(Run {
            at: 1,
            since: 0,
            state: RunState::Open,
            objectives: Vec::new(),
            ended: BTreeSet::new(),
            began: None,
        });
        let late = Need::Late(600);
        let is_late = |st: &AppState| holds(st, Some(0), &late);
        set_clock(&mut st, 2, 10, 1);
        assert!(!is_late(&st), "not begun");
        // Begun at 07:20: late after ten that morning, and from then on.
        set_clock(&mut st, 2, 7, 20);
        st.missions.runs[0].began = Some(st.net.now(&st));
        set_clock(&mut st, 2, 9, 59);
        assert!(!is_late(&st));
        set_clock(&mut st, 2, 10, 0);
        assert!(!is_late(&st), "ten is not past ten");
        set_clock(&mut st, 2, 10, 1);
        assert!(is_late(&st));
        assert!(!holds(&st, None, &late), "asked for no mission");
        set_clock(&mut st, 3, 1, 0);
        assert!(is_late(&st));
        // Begun at 11:00: the next morning's ten.
        set_clock(&mut st, 2, 11, 0);
        st.missions.runs[0].began = Some(st.net.now(&st));
        set_clock(&mut st, 2, 23, 0);
        assert!(!is_late(&st));
        set_clock(&mut st, 3, 10, 0);
        assert!(!is_late(&st));
        set_clock(&mut st, 3, 10, 1);
        assert!(is_late(&st));
        // Begun on ten: late a minute on.
        set_clock(&mut st, 2, 10, 0);
        st.missions.runs[0].began = Some(st.net.now(&st));
        assert!(!is_late(&st));
        set_clock(&mut st, 2, 10, 1);
        assert!(is_late(&st));
    }

    #[test]
    fn holds_fires_once_the_need_holds() {
        let mut st = state_home();
        st.book = book(
            r#"Mission(id: "m", title: "M", from: "mezz", steps: [
                Step(id: "has", when: Holds(Carries("ledger_shard")), then: [Log("got")]),
            ])"#,
        );
        begin(&mut st);
        tick(&mut st);
        assert_eq!(st.missions.runs[0].state, RunState::Waiting);
        st.character.give(ItemId::new("ledger_shard"), 1);
        tick(&mut st);
        assert!(matches!(st.missions.runs[0].state, RunState::Done { .. }));
    }

    #[test]
    fn holds_late_uses_its_mission() {
        let mut st = state_home();
        let half_hour = (st.minute_of_day() + 30) % sim::DAY;
        st.book = book(&format!(
            r#"Mission(id: "m", title: "M", from: "mezz", steps: [
                Step(id: "go", when: After(minutes: 0), then: [Log("go")]),
                Step(id: "late", when: Holds(Late({half_hour})), then: [Log("late")]),
            ])"#
        ));
        begin(&mut st);
        tick(&mut st);
        assert_eq!(st.missions.runs[0].began, Some(st.net.now(&st)));
        assert_eq!(st.missions.runs[0].state, RunState::Open);
        advance(&mut st, 29);
        tick(&mut st);
        assert_eq!(st.missions.runs[0].state, RunState::Open);
        advance(&mut st, 2);
        tick(&mut st);
        assert!(matches!(st.missions.runs[0].state, RunState::Done { .. }));
    }

    #[test]
    fn objective_fit_fills_its_slots() {
        let mut st = state_home();
        st.book = book(
            r#"Mission(id: "m", title: "M", from: "mezz", steps: [
                Step(id: "a", when: NewGame, then: [ObjectiveFit([
                    Fit(need: All([]), text: "see {marlo}"),
                ])]),
            ])"#,
        );
        begin(&mut st);
        tick(&mut st);
        assert_eq!(st.missions.runs[0].objectives, ["see Marlo"]);
    }

    #[test]
    fn a_later_missions_holds_step_runs_in_the_same_call() {
        let mut st = state_home();
        st.book = book(
            r#"Mission(id: "a", title: "A", from: "mezz", steps: [
                Step(id: "s", when: NewGame, then: [Unseal("x")]),
            ]),
            Mission(id: "b", title: "B", from: "mezz", steps: [
                Step(id: "w", when: Holds(Unsealed("x")), then: [Log("ok")]),
            ])"#,
        );
        begin(&mut st);
        tick(&mut st);
        assert!(matches!(st.missions.runs[0].state, RunState::Done { .. }));
        assert!(matches!(st.missions.runs[1].state, RunState::Done { .. }));
    }

    #[test]
    fn objective_fit_takes_the_first_that_holds() {
        let objective = |skills: &[&str]| {
            let mut st = state_home();
            st.book = book(
                r#"Mission(id: "m", title: "M", from: "mezz", steps: [
                    Step(id: "a", when: NewGame, then: [ObjectiveFit([
                        Fit(need: Skill("hacking"), text: "crack it"),
                        Fit(need: Skill("streetwise"), text: "look it over"),
                        Fit(need: All([]), text: "ask around"),
                    ])]),
                ])"#,
            );
            st.character.skills = skills.iter().map(|s| game::SkillId::new(s)).collect();
            begin(&mut st);
            tick(&mut st);
            st.missions.runs[0].objectives.clone()
        };
        assert_eq!(objective(&["hacking", "streetwise"]), ["crack it"]);
        assert_eq!(objective(&["streetwise"]), ["look it over"]);
        assert_eq!(objective(&[]), ["ask around"]);
    }

    #[test]
    fn the_first_step_firing_marks_when_the_mission_began() {
        let mut st = state_home();
        st.book = book(
            r#"Mission(id: "later", title: "Later", from: "mezz", steps: [
                Step(id: "wait", when: After(minutes: 30), then: [Log("on")]),
                Step(id: "more", when: After(minutes: 30), then: [Log("more")]),
            ])"#,
        );
        begin(&mut st);
        assert_eq!(st.missions.runs[0].began, None, "nothing fired");
        advance(&mut st, 30);
        tick(&mut st);
        let began = st.net.now(&st);
        assert_eq!(st.missions.runs[0].began, Some(began));
        advance(&mut st, 30);
        tick(&mut st);
        assert!(matches!(st.missions.runs[0].state, RunState::Done { .. }));
        assert_eq!(st.missions.runs[0].began, Some(began), "the first step's");
    }

    #[test]
    fn changed_and_unchanged_need_a_known_jacket() {
        let mut st = begun();
        let both = |st: &AppState| {
            (
                holds(st, None, &Need::Changed),
                holds(st, None, &Need::Unchanged),
            )
        };
        assert_eq!(both(&st), (false, false), "no news item");
        // Marlo dies and the news is about him.
        end(&mut st, 0, 0);
        news::put_out(&mut st, "marlo", None);
        let jacket = ItemId::new("salvage_jacket");
        st.character.wear(game::Slot::Torso, jacket.clone(), None);
        assert_eq!(both(&st), (false, false), "nobody saw you go");
        st.missions.bodies[0].left = Some(Left {
            at: (22, 41),
            heading: "north".to_string(),
            worn: None,
        });
        assert_eq!(both(&st), (false, false), "nothing worn then");
        st.missions.bodies[0].left.as_mut().unwrap().worn = Some(jacket.clone());
        assert_eq!(both(&st), (false, true));
        st.character.take_off(game::Slot::Torso);
        assert_eq!(both(&st), (true, false), "nothing on now");
        st.character
            .wear(game::Slot::Torso, ItemId::new("ballistic_weave"), None);
        assert_eq!(both(&st), (true, false), "another jacket");
        // Stale, the item still holds what the camera saw.
        let fresh = news::current(&st).unwrap().fresh;
        advance(&mut st, fresh);
        assert!(news::current(&st).is_none());
        assert_eq!(both(&st), (true, false));
        st.character.wear(game::Slot::Torso, jacket, None);
        assert_eq!(both(&st), (false, true));
    }

    /// A book whose one role, `doc`, is a ripperdoc, with these missions and scenes.
    pub(crate) fn doc_book(missions: &str, scenes: &str) -> Book {
        Book::parse(&format!(
            r#"Story(
                cast: {{
                    "doc": Role(name: "Doc Vance", title: "ripperdoc",
                        pick: Nearest("ripperdoc"), face: None),
                }},
                faces: {{
                    "silhouette": Face(rows: ["................", "................"], eye: (0, 0, 0)),
                }},
                missions: [{missions}],
                scenes: {{ {scenes} }},
            )"#
        ))
        .unwrap()
    }

    /// A mission that does nothing, for a book that only needs its scenes.
    pub(crate) const IDLE: &str = r#"Mission(id: "m", title: "M", from: "doc", steps: [
        Step(id: "a", when: NewGame, then: [Log("hi")]),
    ])"#;

    /// Casts `doc` on the clinic's doc at work.
    pub(crate) fn cast_doc(st: &mut AppState, b: citygen::BuildingId) -> PersonId {
        let doc = crate::clinic::doc_at_work(st, b).expect("the doc is in");
        st.missions.cast.insert("doc".into(), doc);
        doc
    }

    /// The street tile outside a building's street door.
    pub(crate) fn outside_door(st: &AppState, b: citygen::BuildingId) -> Pos {
        let c = st.city.as_ref().unwrap();
        let foot = c.building_opt(b).unwrap().footprint;
        let (_, door) = c
            .portals(b)
            .into_iter()
            .find(|(k, _)| *k == StratumKind::Street)
            .unwrap();
        door.neighbors4()
            .find(|n| !foot.contains(*n) && c.walkable(StratumKind::Street, *n))
            .unwrap()
    }

    #[test]
    fn by_holds_beside_a_doc_at_work_and_not_at_the_front_door() {
        let (mut st, b, _, _) = crate::clinic::testutil::inside();
        st.book = doc_book(IDLE, "");
        let doc = cast_doc(&mut st, b);
        let by = |within| Where::By {
            role: "doc".into(),
            within,
        };
        let (place, pos) = st.player.as_ref().map(|p| (p.place, p.pos)).unwrap();
        stand_seeing(&mut st, place, pos);
        assert!(at(&st, &by(4)), "beside the chair, beside her");
        assert!(!at(&st, &by(0)), "not on her tile");
        // At the front door on the street floor: a room or a floor away from her.
        let city = st.city.clone().unwrap();
        let entrance = st
            .interiors
            .plan(&city, b)
            .unwrap()
            .clinic
            .unwrap()
            .entrance;
        st.interiors.ensure(&city, b, 0).unwrap();
        st.player = Some(Player::at(
            Place::Floor {
                building: b,
                level: 0,
            },
            entrance,
        ));
        assert!(!at(&st, &by(4)));
        // Out on the street, beside her where she stands.
        let out = outside_door(&st, b);
        st.world.as_mut().unwrap().park(doc, out);
        on_street(&mut st, out.offset(0, 1));
        assert!(at(&st, &by(4)));
        on_street(&mut st, out.offset(5, 0));
        assert!(!at(&st, &by(4)));
        // Nobody cast, nobody beside.
        st.missions.cast.clear();
        on_street(&mut st, out.offset(0, 1));
        assert!(!at(&st, &by(4)));
    }

    #[test]
    fn by_wants_her_in_sight_not_across_a_wall() {
        use citygen::{Front, RoomKind};
        let (mut st, _, _, _) = crate::clinic::testutil::outside(crate::clinic::testutil::NIGHT);
        st.book = doc_book(IDLE, "");
        let city = st.city.clone().unwrap();
        // A front whose front room comes within four tiles of the doc's spot, a wall
        // between them.
        let b = city
            .buildings
            .iter()
            .map(|x| x.id)
            .find(|b| {
                matches!(
                    citygen::clinic_front(&city, *b),
                    Some(Front::NoodleBar | Front::Butcher | Front::Tattoo)
                ) && crate::clinic::doc_at_work(&st, *b).is_some()
            })
            .expect("a noodle bar, butcher or tattoo clinic with its doc in");
        let doc = cast_doc(&mut st, b);
        let (place, her) = crate::interiors::drawn_at(&st, doc).expect("she is drawn inside");
        let Place::Floor { level, .. } = place else {
            panic!("{place:?}")
        };
        let floor = st.interiors.ensure(&city, b, level).unwrap().clone();
        let room = |kind| floor.rooms.iter().find(|r| r.kind == kind).unwrap().rect;
        let (front, surgery) = (room(RoomKind::Waiting), room(RoomKind::Surgery));
        let across = front
            .iter()
            .find(|p| {
                !surgery.contains(*p) && floor.tiles[*p].is_walkable() && p.manhattan(her) <= 4
            })
            .expect("a front-room tile four from her");
        let by = Where::By {
            role: "doc".into(),
            within: 4,
        };
        stand_seeing(&mut st, place, across);
        assert!(!at(&st, &by), "across the wall from her");
        let beside = beside_on(&mut st, place, her);
        stand_seeing(&mut st, place, beside);
        assert!(at(&st, &by), "beside her in the surgery");
    }

    #[test]
    fn by_answers_off_her_building_without_drawing_her_floor() {
        let (mut st, b, _, _) = crate::clinic::testutil::inside();
        st.book = doc_book(IDLE, "");
        let doc = cast_doc(&mut st, b);
        let by = Where::By {
            role: "doc".into(),
            within: 4,
        };
        // What `at` says working out in full where her floor draws her.
        let full = |st: &AppState| {
            let p = st.player.as_ref().unwrap();
            crate::interiors::drawn_at(st, doc).is_some_and(|(place, pos)| {
                p.place == place
                    && p.vis().get(pos) == Some(&Visibility::Visible)
                    && p.pos.manhattan(pos) <= 4
            })
        };
        // On her floor, beside her: worked out, and beside.
        let (place, pos) = st.player.as_ref().map(|p| (p.place, p.pos)).unwrap();
        stand_seeing(&mut st, place, pos);
        assert!(!inside_elsewhere(&st, place, doc));
        assert!(at(&st, &by) && full(&st));
        // On the street outside: she is in a building you are not in.
        let out = outside_door(&st, b);
        on_street(&mut st, out);
        assert!(inside_elsewhere(&st, STREET, doc));
        assert!(!at(&st, &by) && !full(&st));
        // On another building's floor: the same.
        let (home, level) = st
            .city
            .as_ref()
            .and_then(citygen::home)
            .map(|h| (h.building, h.level))
            .unwrap();
        let elsewhere = Place::Floor {
            building: home,
            level,
        };
        assert!(inside_elsewhere(&st, elsewhere, doc));
        // Out on the street herself: worked out, and beside.
        st.world.as_mut().unwrap().park(doc, out);
        on_street(&mut st, out.offset(0, 1));
        assert!(!inside_elsewhere(&st, STREET, doc));
        assert!(at(&st, &by) && full(&st));
    }

    #[test]
    fn chromed_waits_for_spent_and_up() {
        let mut st = crate::clinic::testutil::seated();
        let b = st.chair.unwrap().building;
        st.book = doc_book(
            r#"Mission(id: "m", title: "M", from: "doc", steps: [
                Step(id: "tray", when: NewGame, then: [Tray(from: "doc", tab: "doc",
                    title: "her tray", line: "One piece.",
                    sets: [TraySet(need: All([]), items: ["neural_port"])])]),
                Step(id: "out", when: Chromed("doc"), then: [Log("out")]),
            ])"#,
            "",
        );
        begin(&mut st);
        let doc = cast_doc(&mut st, b);
        take_from_tray(&mut st, 0);
        tick(&mut st);
        assert_eq!(st.missions.runs[0].at, 1, "nothing spent yet");
        // The piece in, in her chair, on the tab: spent, but you are still sat in it.
        st.character.credits = 0;
        let put = crate::actions::perform(
            &mut st,
            game::Action::Install {
                item: ItemId::new("neural_port"),
                site: game::Site::Neural,
            },
        );
        assert!(put.is_ok(), "{put:?}");
        assert!(st.missions.tab.as_ref().unwrap().spent);
        tick(&mut st);
        assert_eq!(st.missions.runs[0].at, 1, "still in the chair");
        // Up out of the chair, but the tab another doc's: nothing.
        st.chair = None;
        st.missions.tab.as_mut().unwrap().doc = PersonId(doc.0 + 1);
        tick(&mut st);
        assert_eq!(st.missions.runs[0].at, 1, "another doc's tab");
        // Hers, spent, and you up: chromed.
        st.missions.tab.as_mut().unwrap().doc = doc;
        tick(&mut st);
        assert!(matches!(st.missions.runs[0].state, RunState::Done { .. }));
        assert!(st.log.iter().any(|e| e.text == "out"));
    }

    /// The window closed while you sat in her chair, the piece in on her tab: the run
    /// reopens with you up (the chair is the deck's, which opens anew), so the doctor's
    /// `Chromed` step fires without the deck opened and closed first.
    #[test]
    fn a_run_saved_in_the_chair_reopens_up_and_chromed_fires() {
        use crate::runs::file::{read, write};
        use crate::runs::snapshot::{after_restore, restore};
        let mut st = crate::clinic::testutil::seated();
        let b = st.chair.unwrap().building;
        st.book = doc_book(
            r#"Mission(id: "m", title: "M", from: "doc", steps: [
                Step(id: "tray", when: NewGame, then: [Tray(from: "doc", tab: "doc",
                    title: "her tray", line: "One piece.",
                    sets: [TraySet(need: All([]), items: ["neural_port"])])]),
                Step(id: "out", when: Chromed("doc"), then: [Log("out")]),
            ])"#,
            "",
        );
        begin(&mut st);
        cast_doc(&mut st, b);
        take_from_tray(&mut st, 0);
        let put = crate::actions::perform(
            &mut st,
            game::Action::Install {
                item: ItemId::new("neural_port"),
                site: game::Site::Neural,
            },
        );
        assert!(put.is_ok(), "{put:?}");
        assert!(st.missions.tab.as_ref().unwrap().spent);
        tick(&mut st);
        assert_eq!(st.missions.runs[0].at, 1, "still in the chair");
        let dir = crate::runs::testutil::Dir::new("chair");
        let st = crate::runs::testutil::running(st);
        let path = write(&dir.0, &st).unwrap();
        let mut back = AppState::new();
        back.book = st.book.clone();
        let (_, snap, lost) = read(&path, &back.book).unwrap();
        restore(&mut back, snap);
        after_restore(&mut back, lost);
        assert_eq!(back.chair, None, "up out of the chair");
        tick(&mut back);
        assert!(matches!(back.missions.runs[0].state, RunState::Done { .. }));
        assert!(back.log.iter().any(|e| e.text == "out"));
    }

    #[test]
    fn dev_free_chrome_spends_the_tab_on_its_piece() {
        let (mut st, b, level, _) = crate::clinic::testutil::inside();
        st.book = doc_book(
            r#"Mission(id: "m", title: "M", from: "doc", steps: [
                Step(id: "tray", when: NewGame, then: [Tray(from: "doc", tab: "doc",
                    title: "her tray", line: "One piece.",
                    sets: [TraySet(need: All([]), items: ["neural_port"])])]),
                Step(id: "out", when: Chromed("doc"), then: [Log("out")]),
            ])"#,
            "",
        );
        begin(&mut st);
        let doc = cast_doc(&mut st, b);
        take_from_tray(&mut st, 0);
        st.free_chrome = true;
        let spent = |st: &AppState| st.missions.tab.as_ref().unwrap().spent;
        let install = |st: &mut AppState, id: &str| {
            let item = ItemId::new(id);
            let site = site_for(st, &item);
            let put = crate::actions::perform(st, game::Action::Install { item, site });
            assert!(put.is_ok(), "{put:?}");
            site
        };
        // Another piece goes in free and leaves the tab be.
        let mesh = ItemId::new("subdermal_mesh");
        st.character.give_item(&st.content, mesh, 1);
        install(&mut st, "subdermal_mesh");
        assert!(!spent(&st));
        // In another doc's chair the tab isn't theirs to spend.
        st.chair = Some(crate::clinic::Chair {
            building: b,
            level,
            doc: PersonId(doc.0 + 1),
        });
        let site = install(&mut st, "neural_port");
        assert!(!spent(&st));
        let pulled = crate::actions::perform(&mut st, game::Action::Pull(site));
        assert!(pulled.is_ok(), "{pulled:?}");
        // Up, the tab's piece in anywhere: spent, and chromed.
        st.chair = None;
        install(&mut st, "neural_port");
        assert!(spent(&st));
        tick(&mut st);
        assert!(matches!(st.missions.runs[0].state, RunState::Done { .. }));
        assert!(st.log.iter().any(|e| e.text == "out"));
    }

    /// The index of the book's mission `id`, and of its run.
    fn mission_at(st: &AppState, id: &str) -> usize {
        st.book.missions.iter().position(|m| m.id == id).unwrap()
    }

    /// `seren_has_written` in any city with a home moved into: the story begun and run
    /// from your door to Seren's word.
    pub(crate) fn to_seren_has_written(st: &mut AppState) {
        begin(st);
        to_your_door(st);
        to_the_mail(st);
        to_the_morning_call(st);
        to_the_word(st);
    }

    /// The morning after to Seren's word: the news switched off, Mezz's mail come and
    /// read; the package unsealed, the doctor open.
    pub(crate) fn to_the_word(st: &mut AppState) {
        the_news_is_on(st);
        news::switch_off(st);
        mezz_saw_the_news(st);
        read_mezzs_mail(st);
        assert!(unsealed(st, &ItemId::new("taped_package")));
        let doctor = mission_at(st, "doctor");
        assert_eq!(st.missions.runs[doctor].state, RunState::Open);
    }

    /// The unsealed package opened as the pack opens it, and the story's next beat: the
    /// shard in your pack.
    pub(crate) fn open_the_package(st: &mut AppState) {
        let (package, shard) = (ItemId::new("taped_package"), ItemId::new("ledger_shard"));
        let opened = crate::actions::perform(st, game::Action::Open(package.clone()));
        assert!(opened.is_ok(), "{opened:?}");
        assert_eq!(
            (st.character.count(&package), st.character.count(&shard)),
            (0, 1)
        );
        tick(st);
    }

    /// A site on the body the piece fits, a free one first.
    pub(crate) fn site_for(st: &AppState, piece: &ItemId) -> game::Site {
        let fits = st.content.item(piece).and_then(|i| i.implant).unwrap().fits;
        game::Site::ALL
            .into_iter()
            .filter(|s| s.fits() == fits)
            .min_by_key(|s| st.character.chrome.contains_key(s))
            .unwrap_or_else(|| panic!("{piece} fits no site"))
    }

    /// The floor generated, a free tile beside `at` on it.
    pub(crate) fn beside_on(st: &mut AppState, place: Place, at: Pos) -> Pos {
        let Place::Floor { building, level } = place else {
            panic!("{place:?} is no floor")
        };
        st.interiors
            .ensure(st.city.as_ref().unwrap(), building, level)
            .unwrap();
        marks::beside_tile(st, place, at, None).unwrap().1
    }

    /// In at her street door on the mailed code and up beside her: her scene played out.
    /// Gives back the lines of it that were said.
    fn through_her_scene(st: &mut AppState) -> Vec<String> {
        let doc = person_of(st, "odile").expect("odile is cast");
        let b = cast::work_building(st, doc).expect("she works in her clinic");
        let code = crate::credentials::street_code(st, b);
        let out = outside_door(st, b);
        on_street(st, out);
        let city = st.city.as_ref().unwrap();
        let (_, door) = city
            .portals(b)
            .into_iter()
            .find(|(k, _)| *k == StratumKind::Street)
            .unwrap();
        let keyed = crate::interiors::enter(st, b, (StratumKind::Street, door));
        assert_eq!(keyed, Ok(format!("you key in {code:04}")));
        let (place, at) = crate::interiors::drawn_at(st, doc).expect("she is drawn inside");
        let by = beside_on(st, place, at);
        stand_seeing(st, place, by);
        tick(st);
        assert_eq!(scene::running(st).map(|r| r.id()), Some("odile"));
        scene::skip(st);
        st.book.scenes["odile"]
            .beats
            .iter()
            .filter_map(|beat| match beat {
                Beat::Say { text, .. } => Some(fill(st, text)),
                _ => None,
            })
            .filter(|t| st.log.iter().any(|e| e.text.contains(t.as_str())))
            .collect()
    }

    /// Seren's word come: plays the doctor through as the game does and gives back the
    /// lines of her scene that were said, and her tray. The package is opened when
    /// `open`, its shard read at the rig when `read`; then in at the street door on the
    /// mailed code, beside her, her scene, the tray's first piece, her chair on the tab,
    /// up, her parting words, and out on the street, where she is let go.
    fn play_the_doctor(st: &mut AppState, open: bool, read: bool) -> (Vec<String>, Vec<ItemId>) {
        use crate::credentials::{STREET_DOOR, Source, street_code};
        let doctor = mission_at(st, "doctor");
        let doc = person_of(st, "odile").expect("odile is cast");
        let b = cast::work_building(st, doc).expect("she works in her clinic");
        let held = |st: &AppState| st.world.as_ref().unwrap().is_held(doc);
        assert_eq!(
            st.world.as_ref().unwrap().people()[doc.0 as usize].name,
            "Odile Vance"
        );
        // Seren's word: the doc kept in, her clinic shut, the package yours to open.
        assert!(held(st));
        assert!(!crate::clinic::is_open(st, b));
        assert!(unsealed(st, &ItemId::new("taped_package")));
        // Her mail carries the street code, and the journal keeps it.
        let code = street_code(st, b);
        let mail = st.journal.mails.iter().find(|m| m.subject == "ten");
        let mail = mail.expect("Seren's mail");
        assert_eq!(mail.from, "Seren");
        assert_eq!(
            mail.body,
            [
                "Doctor's in by ten. Eat first.".to_string(),
                format!("Shutter's down till dark. Code's {code:04}. She knows you're coming."),
                "The package is yours now. Open it.".into(),
                "— S".into(),
            ]
        );
        let learned = st.journal.code_for(b, 0, STREET_DOOR).expect("learned");
        assert_eq!((learned.code, learned.source), (code, Source::Mail));
        let see = format!(
            "See the doctor · {} · {} · by ten",
            cast::place_of(st, doc),
            cast::district_of(st, doc)
        );
        assert_eq!(st.missions.runs[doctor].objectives.last(), Some(&see));
        // The package opens on the shard; read at the rig, it tells.
        if open {
            open_the_package(st);
        }
        if read {
            let shard = ItemId::new("ledger_shard");
            assert!(reading::ReadCard::open(st, &shard, ReadAt::Rig).is_some());
            assert!(holds(st, None, &Need::Read("ledger_shard".into())));
        }
        let said = through_her_scene(st);
        let out = outside_door(st, b);
        // Her tray: one piece, on Castellan's tab.
        let tray = st.missions.tray.clone().expect("her tray is held out");
        assert_eq!(
            (tray.from.as_str(), tray.tab.as_str()),
            ("odile", "castellan")
        );
        assert_eq!(
            st.missions.runs[doctor]
                .objectives
                .last()
                .map(String::as_str),
            Some("Sit in Odile Vance's chair · your piece is on Castellan's tab")
        );
        take_from_tray(st, 0);
        let piece = tray.items[0].clone();
        assert_eq!(st.character.count(&piece), 1);
        // Sat in her chair, it goes in free.
        let chair = Mark::Chair("odile".into());
        let (floor, seat) = marks::resolve(st, &chair, None).expect("her chair");
        let Place::Floor { building, level } = floor else {
            panic!("{floor:?}")
        };
        let by = beside_on(st, floor, seat);
        st.player = Some(Player::at(floor, by));
        assert_eq!(crate::clinic::doc_at_work(st, building), Some(doc));
        st.chair = Some(crate::clinic::Chair {
            building,
            level,
            doc,
        });
        let (credits, site) = (st.character.credits, site_for(st, &piece));
        let put = crate::actions::perform(
            st,
            game::Action::Install {
                item: piece.clone(),
                site,
            },
        );
        assert!(put.is_ok(), "{put:?}");
        assert_eq!(st.character.chrome.get(&site), Some(&piece));
        assert_eq!(st.character.credits, credits, "on the tab");
        // Up out of the chair: her parting words, the tags on her lamp.
        assert!(st.chair.take().is_some());
        tick(st);
        assert_eq!(scene::running(st).map(|r| r.id()), Some("odile_out"));
        scene::skip(st);
        assert!(
            st.missions.notes.iter().any(|(p, q, t)| (*p, *q) == (floor, seat)
                && t == "Three clinic tags hang off the lamp arm. The names are scratched off."),
            "{:?}",
            st.missions.notes
        );
        assert!(matches!(
            st.missions.runs[doctor].state,
            RunState::Done { .. }
        ));
        assert!(
            st.log
                .iter()
                .any(|e| e.text == "Doctor's in by ten · on Castellan's tab")
        );
        // Let go once you are out, never past you.
        tick(st);
        assert!(held(st), "not while you are inside");
        on_street(st, out);
        tick(st);
        assert!(!held(st));
        (said, tray.items)
    }

    #[test]
    fn the_doctor_plays_end_to_end() {
        let mut st = the_morning_call();
        to_the_word(&mut st);
        let (doctor, ledger) = (mission_at(&st, "doctor"), mission_at(&st, "ledger"));
        assert_eq!(st.missions.runs[doctor].state, RunState::Open);
        assert_eq!(st.missions.runs[ledger].state, RunState::Waiting);
        // Seren breaks into your set.
        let seren = st.missions.asides.last().expect("Seren on the set");
        assert!(seren.label.ends_with("▚▞▚ S"), "{}", seren.label);
        assert_eq!(seren.text, "Doctor's in by ten. Eat first.");
        // The camera saw you in this jacket, and you wear it still; it isn't ten yet.
        let jacket = ItemId::new("salvage_jacket");
        st.character.wear(Slot::Torso, jacket.clone(), None);
        st.missions.bodies[0].left = Some(Left {
            at: (22, 41),
            heading: "north".to_string(),
            worn: Some(jacket),
        });
        assert!(st.clock() < (10, 0), "{:?}", st.clock());
        let district = fill(&st, "{marlo.district}");
        let (said, tray) = play_the_doctor(&mut st, true, true);
        // The shard read, she names nobody; on time, she says nothing of it.
        assert_eq!(
            said,
            [
                "He sends me strays.".to_string(),
                format!("Lose that jacket before you go. It's on every screen in {district}."),
                "Castellan pays for one piece. Pick.".into(),
            ]
        );
        assert_eq!(
            tray,
            ["neural_port", "target_lens", "subdermal_mesh"].map(ItemId::new),
            "a runner's tray"
        );
        // The hook: opened, whose the shard is; read at the rig, its owner, then his line.
        let coz = person_of(&st, "coz").expect("coz is cast");
        assert_ne!(Some(coz), person_of(&st, "mezz"));
        assert_eq!(st.missions.runs[ledger].state, RunState::Open);
        assert_eq!(
            st.missions.runs[ledger].objectives,
            [
                "Find out whose shard this is · read it at your rig".to_string(),
                bring_it_to_coz(&st),
            ]
        );
        send(&mut st, Event::Talked(coz));
        assert!(matches!(
            st.missions.runs[ledger].state,
            RunState::Done { .. }
        ));
        assert!(
            st.log
                .iter()
                .any(|e| e.text == "Coz · \"The ledger? Not here. I'll find you.\"")
        );
    }

    #[test]
    fn the_doctor_names_the_fixer_to_whoever_couldnt_read_the_shard() {
        let mut st = the_morning_call();
        to_the_word(&mut st);
        // A gun hand, not a runner, the package still taped, in another jacket than the
        // camera saw, and late.
        st.character.skills = [SkillId::new("handguns")].into();
        st.character
            .wear(Slot::Torso, ItemId::new("ballistic_weave"), None);
        st.missions.bodies[0].left = Some(Left {
            at: (22, 41),
            heading: "north".to_string(),
            worn: Some(ItemId::new("salvage_jacket")),
        });
        let (h, m) = st.clock();
        let now = u64::from(h) * 60 + u64::from(m);
        advance(&mut st, (10 * 60 + 30 + 1440 - now) % 1440);
        assert_eq!(st.clock(), (10, 30));
        let (said, tray) = play_the_doctor(&mut st, false, false);
        assert_eq!(
            said,
            [
                "He sends me strays.",
                "Ten, he said. He said you'd be late, too.",
                "New jacket. Good. The last one didn't think of that.",
                "That tape's Coz's. You're walking a fixer's ledger round in daylight.",
                "Castellan pays for one piece. Pick.",
            ]
        );
        assert_eq!(
            tray,
            ["target_lens", "adrenal_pump", "hardback_brace"].map(ItemId::new),
            "a gun hand's tray"
        );
    }

    /// The ledger's objective once you know whose the shard is.
    pub(crate) fn bring_it_to_coz(st: &AppState) -> String {
        let coz = person_of(st, "coz").expect("coz is cast");
        format!(
            "Bring it to Coz · {} · {}",
            cast::place_of(st, coz),
            cast::district_of(st, coz)
        )
    }

    /// The ledger's objectives, in order.
    pub(crate) fn ledger_objectives(st: &AppState) -> Vec<String> {
        st.missions.runs[mission_at(st, "ledger")]
            .objectives
            .clone()
    }

    /// Seren's word come, with only these skills.
    pub(crate) fn the_word_with(skills: &[&str]) -> AppState {
        let mut st = the_morning_call();
        to_the_word(&mut st);
        st.character.skills = skills.iter().map(|s| SkillId::new(s)).collect();
        st
    }

    #[test]
    fn opening_the_package_opens_the_ledger() {
        for (skills, how) in [
            (&["hacking"][..], "read it at your rig"),
            (&["electronics"][..], "open the casing at your bench"),
            (&["streetwise"][..], "look it over in your pack"),
            (&[][..], "ask the doctor at {odile.place}"),
        ] {
            let mut st = the_word_with(skills);
            let (morning, ledger) = (mission_at(&st, "morning_after"), mission_at(&st, "ledger"));
            assert_eq!(st.missions.runs[ledger].state, RunState::Waiting);
            open_the_package(&mut st);
            assert!(matches!(
                st.missions.runs[morning].state,
                RunState::Done { .. }
            ));
            assert!(
                st.log
                    .iter()
                    .any(|e| e.text == "The morning after · a ledger shard, in your pack")
            );
            assert_eq!(st.missions.runs[ledger].state, RunState::Open);
            let whose = fill(&st, &format!("Find out whose shard this is · {how}"));
            assert_eq!(ledger_objectives(&st), [whose], "{skills:?}");
        }
    }

    #[test]
    fn reading_the_shard_sends_you_to_coz() {
        let shard = ItemId::new("ledger_shard");
        // Read at the rig.
        let mut st = the_word_with(&["hacking"]);
        open_the_package(&mut st);
        assert!(reading::ReadCard::open(&mut st, &shard, ReadAt::Rig).is_some());
        tick(&mut st);
        assert_eq!(ledger_objectives(&st).last(), Some(&bring_it_to_coz(&st)));
        // Looked over in the pack, its card showing the tape.
        let mut st = the_word_with(&["streetwise"]);
        open_the_package(&mut st);
        tick(&mut st);
        assert_eq!(ledger_objectives(&st).len(), 1, "not yet looked at");
        reading::look_tells(&mut st, &shard);
        tick(&mut st);
        assert_eq!(ledger_objectives(&st).last(), Some(&bring_it_to_coz(&st)));
    }

    #[test]
    fn odile_tells_who_has_not_read() {
        let named = |said: &[String], line: &str| said.iter().any(|l| l == line);
        // The shard in your pack, unread: she names him, and that is the ledger's word.
        let mut st = the_word_with(&[]);
        open_the_package(&mut st);
        let said = through_her_scene(&mut st);
        assert!(
            named(
                &said,
                "That shard's Coz's. You're walking a fixer's ledger round in daylight."
            ),
            "{said:?}"
        );
        assert_eq!(ledger_objectives(&st).last(), Some(&bring_it_to_coz(&st)));
        // Her scene first, the package still taped: told, and opened after, the ledger
        // goes straight to him.
        let mut st = the_word_with(&[]);
        let said = through_her_scene(&mut st);
        assert!(
            named(
                &said,
                "That tape's Coz's. You're walking a fixer's ledger round in daylight."
            ),
            "{said:?}"
        );
        assert!(ledger_objectives(&st).is_empty(), "no shard yet");
        open_the_package(&mut st);
        // Its first tick carries no hint she made needless.
        assert_eq!(
            ledger_objectives(&st),
            [
                "Find out whose shard this is".to_string(),
                bring_it_to_coz(&st)
            ]
        );
    }

    #[test]
    fn odile_names_him_to_the_empty_handed() {
        let mut st = the_word_with(&[]);
        let package = ItemId::new("taped_package");
        game::remove_from_pack(&mut st.character, &package, 1);
        let said = through_her_scene(&mut st);
        let third = "He told me what you're carrying. Black tape, three turns? That's Coz's.";
        assert!(said.iter().any(|l| l == third), "{said:?}");
        assert!(
            said.iter().all(|l| !l.contains("You're walking")),
            "{said:?}"
        );
        assert!(holds(&st, None, &Need::Read("ledger_shard".into())));
        // Read already, she says none of it.
        let mut st = the_word_with(&[]);
        game::remove_from_pack(&mut st.character, &package, 1);
        st.missions.read.insert("ledger_shard".into());
        let said = through_her_scene(&mut st);
        assert!(said.iter().all(|l| l != third), "{said:?}");
    }

    #[test]
    fn no_clinic_leaves_the_ledger_uncast() {
        let mut st = state_home();
        st.book.cast.get_mut("odile").unwrap().pick = Pick::Nearest("astronaut".into());
        begin(&mut st);
        let morning = mission_at(&st, "morning_after");
        let (doctor, ledger) = (mission_at(&st, "doctor"), mission_at(&st, "ledger"));
        assert!(person_of(&st, "odile").is_none());
        assert_eq!(st.missions.runs[doctor].state, RunState::Uncast);
        assert_eq!(st.missions.runs[ledger].state, RunState::Uncast);
        // The morning after plays all the same: the news, Mezz's mail, the package.
        to_your_door(&mut st);
        to_the_mail(&mut st);
        to_the_morning_call(&mut st);
        the_news_is_on(&mut st);
        news::switch_off(&mut st);
        mezz_saw_the_news(&st);
        read_mezzs_mail(&mut st);
        assert!(unsealed(&st, &ItemId::new("taped_package")));
        assert!(st.journal.mails.iter().all(|m| m.subject != "ten"));
        open_the_package(&mut st);
        assert!(matches!(
            st.missions.runs[morning].state,
            RunState::Done { .. }
        ));
        assert_eq!(st.missions.runs[doctor].state, RunState::Uncast);
        assert_eq!(st.missions.runs[ledger].state, RunState::Uncast);
    }

    #[test]
    fn the_doctor_plays_with_one_fixer() {
        let mut st = AppState::new();
        st.enter_city(
            Generator::run(GenParams {
                seed: 1,
                size: SizePreset::Medium,
                districts: 6,
                ..Default::default()
            })
            .unwrap(),
        );
        let world = st.world.as_ref().unwrap();
        let fixers = world.people().iter().filter(|p| p.profession == "fixer");
        assert_eq!(fixers.count(), 1, "a city with one fixer");
        let home = st.home.expect("a home").home.building;
        crate::credentials::prepare(&mut st, home);
        crate::credentials::move_in(&mut st);
        begin(&mut st);
        let (doctor, ledger) = (mission_at(&st, "doctor"), mission_at(&st, "ledger"));
        assert!(person_of(&st, "mezz").is_some() && person_of(&st, "coz").is_none());
        assert_eq!(st.missions.runs[doctor].state, RunState::Waiting);
        assert_eq!(st.missions.runs[ledger].state, RunState::Uncast);
        // The story to Seren's word, as anywhere.
        to_your_door(&mut st);
        to_the_mail(&mut st);
        to_the_morning_call(&mut st);
        to_the_word(&mut st);
        assert_eq!(st.missions.runs[doctor].state, RunState::Open);
        // Opened, not read: she names him though nobody plays him.
        let (said, _) = play_the_doctor(&mut st, true, false);
        let named = "That shard's Coz's. You're walking a fixer's ledger round in daylight.";
        assert!(said.iter().any(|l| l == named), "{said:?}");
        assert_eq!(st.missions.runs[ledger].state, RunState::Uncast);
        // No slot shows unfilled: the log, the mail, the objectives, the notes, the
        // shard's Look and its cards.
        let shard = ItemId::new("ledger_shard");
        let mut shown: Vec<String> = st.log.iter().map(|e| e.text.clone()).collect();
        shown.extend(st.journal.mails.iter().flat_map(|m| m.body.clone()));
        shown.extend(st.missions.runs.iter().flat_map(|r| r.objectives.clone()));
        shown.extend(st.missions.notes.iter().map(|(_, _, t)| t.clone()));
        shown.push(item_desc(&st, &shard));
        for at in [ReadAt::Rig, ReadAt::Bench] {
            let card = reading::ReadCard::open(&mut st, &shard, at).expect("a card");
            let drawn = crate::screens::shots::draw_stack(&st, &[&card], (120, 40));
            let text = crate::screens::testutil::buffer_text(&drawn);
            assert!(text.contains("Coz"), "{text}");
            shown.push(text);
        }
        for text in &shown {
            assert!(!text.contains('{'), "{text}");
        }
    }

    #[test]
    fn every_tray_piece_installs_on_bare_meat() {
        let mut st = crate::clinic::testutil::seated();
        st.free_chrome = true;
        let mut bare = st.character.clone();
        bare.chrome.clear();
        bare.chrome_copies.clear();
        let sets: Vec<TraySet> = st
            .book
            .missions
            .iter()
            .flat_map(|m| &m.steps)
            .flat_map(|s| &s.then)
            .filter_map(|e| match e {
                Effect::Tray { sets, .. } => Some(sets.clone()),
                _ => None,
            })
            .flatten()
            .collect();
        assert!(!sets.is_empty(), "the book holds out a tray");
        for piece in sets.iter().flat_map(|s| &s.items).map(|i| ItemId::new(i)) {
            let item = st.content.item(&piece).unwrap();
            assert_eq!(item.kind, game::ItemKind::Cyberware, "{piece}");
            st.character = bare.clone();
            st.character.give_item(&st.content, piece.clone(), 1);
            let site = site_for(&st, &piece);
            let put = crate::actions::perform(
                &mut st,
                game::Action::Install {
                    item: piece.clone(),
                    site,
                },
            );
            assert!(put.is_ok(), "{piece}: {put:?}");
            assert_eq!(st.character.chrome.get(&site), Some(&piece));
        }
    }

    /// A book whose `send` step mails and whose `next` step waits for that mail to end.
    const MAIL_STEPS: &str = r#"Mission(id: "post", title: "Post", from: "mezz", steps: [
        Step(id: "send", when: NewGame, then: [Mail(from: "mezz", subject: "Hi", body: ["Read me."])]),
        Step(id: "next", when: Ended("send"), then: [Log("after")]),
    ])"#;

    fn after_logged(st: &AppState) -> bool {
        st.log.iter().any(|e| e.text == "after")
    }

    /// The inbox index of the mail the first step sent.
    fn story_mail(st: &AppState) -> usize {
        st.journal
            .mails
            .iter()
            .position(|m| m.story == Some(StepRef::new(0, 0)))
            .expect("the step sent its mail")
    }

    #[test]
    fn a_story_mail_read_ends_its_step() {
        let mut st = state_home();
        st.book = book(MAIL_STEPS);
        begin(&mut st);
        let i = story_mail(&st);
        tick(&mut st);
        assert!(!after_logged(&st), "waits while the mail is unread");
        crate::credentials::mail_read(&mut st, i);
        assert!(after_logged(&st));
    }

    #[test]
    fn a_mail_read_at_the_terminal_ends_its_step() {
        let mut st = state_home();
        st.book = book(MAIL_STEPS);
        begin(&mut st);
        tick(&mut st);
        assert!(!after_logged(&st));
        let i = story_mail(&st);
        crate::access::mail_open(&mut st, i).unwrap();
        assert!(after_logged(&st));
    }

    #[test]
    fn reading_it_again_ends_nothing_twice() {
        let mut st = state_home();
        st.book = book(MAIL_STEPS);
        begin(&mut st);
        let i = story_mail(&st);
        crate::credentials::mail_read(&mut st, i);
        let lines = st.log.len();
        crate::credentials::mail_read(&mut st, i);
        assert_eq!(st.log.len(), lines);
    }

    #[test]
    fn ended_may_name_a_mail_step() {
        // `Book::parse` validates, and `book` unwraps it.
        let b = book(MAIL_STEPS);
        assert!(b.validate().is_ok());
    }
}
