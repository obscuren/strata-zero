//! Site jobs (#98): a fixer sends you into a guarded building for one thing, the
//! objective its mission copy lays: take an item from a container, copy or wipe the data
//! vault, plant a tap, or reach a room. The deed, then getting out, then the hand-in. It
//! fails late, when an alarm purges the vault before a copy, or when a Take's item is
//! lost. Spec: `docs/superpowers/specs/2026-11-25-site-objectives-design.md`.

use super::{
    Event, FAMILY_AT, INNER_AT, Job, JobId, Jobs, Kind, KindTag, LOSS_LATE, Objective, Say, State,
};
use crate::app::AppState;
use crate::containers::ContainerAt;
use citygen::rng::hash64;
use citygen::tile::wall_front;
use citygen::venue::{
    MissionInputs, MissionLayer, MissionSize, ObjectiveDef, ObjectiveKind, VenuePack, VenueType,
};
use citygen::{BuildingId, FloorPlan, Place, Pos, Terrain};
use game::ItemId;
use sim::PersonId;

/// Salt for a site job's building, objective and mission seed.
const SALT_SITE: u64 = 0x10B_0098;
/// Buildings a site job tries before giving up on the kind.
const SITE_TRIES: usize = 6;
/// Net minutes from an alarm until a vault purges itself.
pub const PURGE_MINUTES: u64 = 5;
/// The device a Plant hands you with the job.
pub const TAP: &str = "tap";

/// The size a fixer offers at this standing: small from trusted, medium from the inner
/// circle, large from family.
pub fn size_at(standing: i32) -> MissionSize {
    if standing >= FAMILY_AT {
        MissionSize::Large
    } else if standing >= INNER_AT {
        MissionSize::Medium
    } else {
        MissionSize::Small
    }
}

/// A site job's window, in game hours.
pub fn hours(size: MissionSize) -> u64 {
    match size {
        MissionSize::Small => 12,
        MissionSize::Medium => 18,
        MissionSize::Large => 24,
    }
}

/// A site job's pay: by size, plus six a point of security (a vault Copy is five).
pub fn pay(size: MissionSize, security: u8) -> u32 {
    let base = match size {
        MissionSize::Small => 600,
        MissionSize::Medium => 1200,
        MissionSize::Large => 2000,
    };
    base + 6 * u32::from(security)
}

/// The type building `b` is laid out as, and its layout's plan.
fn type_of(st: &AppState, b: BuildingId) -> Option<&'static VenueType> {
    let plan = crate::interiors::plan_now(st, b)?;
    let id = plan.venue.as_deref()?.type_id.clone();
    VenuePack::builtin().by_id(&id)
}

/// The objective a site job's inputs name on building `b`.
pub fn def(st: &AppState, b: BuildingId, inputs: &MissionInputs) -> Option<&'static ObjectiveDef> {
    type_of(st, b)?.objectives.get(inputs.objective)
}

/// The live copy on `b` and its layer, when it is the copy `inputs` lays (a site job's).
fn layer_for<'a>(
    st: &'a AppState,
    b: BuildingId,
    inputs: &MissionInputs,
) -> Option<&'a MissionLayer> {
    let copy = st.interiors.copy(b).filter(|c| c.live)?;
    (copy.inputs == *inputs).then_some(())?;
    copy.layer.as_ref()
}

/// The Taken site job on building `b`, with its copy's inputs.
pub fn taken_on(st: &AppState, b: BuildingId) -> Option<(&Job, MissionInputs)> {
    st.jobs.list.iter().find_map(|j| {
        let (building, inputs) = j.kind.site()?;
        (j.state == State::Taken && building == b).then_some((j, inputs))
    })
}

/// The objective of the live copy on `b`, when a Taken site job holds it: its kind and
/// where it stands, and whether it is met.
pub fn objective(st: &AppState, b: BuildingId) -> Option<(ObjectiveKind, Place, Pos, bool)> {
    let (j, inputs) = taken_on(st, b)?;
    let layer = layer_for(st, b, &inputs)?;
    let o = layer.objective;
    let place = Place::Floor {
        building: b,
        level: o.level,
    };
    let met = j.objectives.first().is_some_and(|o| o.done);
    Some((inputs.kind, place, o.at, met))
}

/// The container a Take keeps its item in on `b`'s live copy: on the objective's tile, or
/// the wall-set one the objective's tile stands in front of.
pub fn take_container(st: &AppState, b: BuildingId) -> Option<ContainerAt> {
    let (kind, place, at, _) = objective(st, b)?;
    if kind != ObjectiveKind::Take {
        return None;
    }
    take_container_at(st, place, at)
}

fn take_container_at(st: &AppState, place: Place, at: Pos) -> Option<ContainerAt> {
    let Place::Floor { building, level } = place else {
        return None;
    };
    let (_, floor) = st.interiors.floor(building, level)?;
    let holds = |p: Pos| floor.tiles.get(p).and_then(|t| t.feature?.container());
    let tile = if holds(at).is_some() {
        at
    } else {
        at.neighbors4().find(|&p| {
            holds(p).is_some()
                && floor
                    .tiles
                    .get(p)
                    .is_some_and(|t| t.terrain == Terrain::Wall)
                && wall_front(&floor.tiles, p) == Some(at)
        })?
    };
    Some(ContainerAt {
        building,
        level,
        at: tile,
    })
}

/// The item a Take's container holds, when `c` is the container of a Taken Take.
pub fn take_item(st: &AppState, c: ContainerAt) -> Option<ItemId> {
    (take_container(st, c.building) == Some(c)).then_some(())?;
    let (j, _) = taken_on(st, c.building)?;
    match &j.kind {
        Kind::Site { item, .. } => item.clone(),
        _ => None,
    }
}

/// Whether the data vault on `b`'s copy is wiped, by an alarm's purge or by you.
pub fn wiped(st: &AppState, b: BuildingId) -> bool {
    st.interiors.copy(b).is_some_and(|c| c.live && c.wiped)
}

/// A site's vault meets a Copy program on the Net: on a Wipe job it wipes the vault and
/// meets the objective; on a vault already wiped there is nothing to copy. Its line, or
/// None to let the copy land as any vault copy does.
pub fn vault_copied(st: &mut AppState, b: BuildingId) -> Option<String> {
    if wiped(st, b) {
        return Some("the vault is empty · wiped".to_string());
    }
    let (kind, ..) = objective(st, b)?;
    if kind != ObjectiveKind::Wipe {
        return None;
    }
    wipe(st, b);
    Some("vault wiped · nothing left to sell".to_string())
}

/// The vault on `b` is wiped, and the job's objective met.
fn wipe(st: &mut AppState, b: BuildingId) {
    if let Some(c) = st.interiors.copy_mut(b) {
        c.wiped = true;
    }
    super::send(st, Event::Met { building: b });
}

/// Rolls a site job from this fixer: a hotel or office of the city that isn't yours and
/// holds no copy, an objective of its type and a kind of it (Copy only with a deck), at the
/// size the standing gives. None when no building lays one.
pub fn generate(st: &mut AppState, fixer: PersonId, counter: u32, now: u64) -> Option<Job> {
    let seed = st.city.as_ref()?.seed;
    let r = super::roll(seed, fixer, counter, SALT_SITE);
    let r_text = super::roll(seed, fixer, counter, super::generate::SALT_TEXT);
    let standing = st.jobs.standing(fixer);
    let size = size_at(standing);
    let deck = super::has_deck(st);
    let home = st.home.map(|h| h.home.building);
    let city = st.city.as_ref()?;
    let pack = VenuePack::builtin();
    let held: Vec<BuildingId> = (st.jobs.list.iter())
        .filter(|j| matches!(j.state, State::Offered | State::Taken))
        .filter_map(|j| j.kind.site().map(|(b, _)| b))
        .collect();
    let mut cands: Vec<BuildingId> = (city.buildings.iter())
        .filter(|x| pack.for_kind(x.kind).is_some())
        .map(|x| x.id)
        .filter(|b| Some(*b) != home && !held.contains(b) && st.interiors.copy(*b).is_none())
        .collect();
    if cands.is_empty() {
        return None;
    }
    cands.sort_by_key(|b| b.index());
    let start = (r % cands.len() as u64) as usize;
    cands.rotate_left(start);
    for (i, b) in cands.into_iter().take(SITE_TRIES).enumerate() {
        let r_b = hash64(r, i as u64);
        if let Some(job) = lay_one(st, fixer, b, size, deck, r_b, r_text, now) {
            return Some(job);
        }
    }
    None
}

/// A site job on `b`, the first (objective, kind) from the roll's that lays.
#[allow(clippy::too_many_arguments)]
fn lay_one(
    st: &AppState,
    fixer: PersonId,
    b: BuildingId,
    size: MissionSize,
    deck: bool,
    r: u64,
    r_text: u64,
    now: u64,
) -> Option<Job> {
    let plan = crate::interiors::plan_now(st, b)?;
    let layout = plan.venue.as_deref()?;
    let pack = VenuePack::builtin();
    let t = pack.by_id(&layout.type_id)?;
    let mut asks: Vec<(usize, ObjectiveKind)> = (t.objectives.iter().enumerate())
        .flat_map(|(k, o)| o.kinds.iter().map(move |&kind| (k, kind)))
        .filter(|&(_, kind)| deck || kind != ObjectiveKind::Copy)
        .collect();
    if asks.is_empty() {
        return None;
    }
    let start = (r % asks.len() as u64) as usize;
    asks.rotate_left(start);
    let security = crate::access::security_of(st, b);
    for (objective, kind) in asks {
        let inputs = MissionInputs {
            size,
            objective,
            security,
            seed: hash64(r, SALT_SITE),
            kind,
        };
        let Ok(layer) = MissionLayer::lay(layout, t, pack.size(size), inputs) else {
            continue;
        };
        let def = &t.objectives[objective];
        let item = match kind {
            ObjectiveKind::Take => def.item.as_deref().map(ItemId::new),
            ObjectiveKind::Copy => Some(ItemId::new("vault_data")),
            _ => None,
        };
        if kind == ObjectiveKind::Take && item.as_ref().is_none_or(|i| st.content.item(i).is_none())
        {
            continue;
        }
        let k = Kind::Site {
            building: b,
            objective,
            kind,
            size,
            security,
            seed: inputs.seed,
            item: item.clone(),
        };
        let mut job = super::generate::blank(fixer, k, hours(size), pay(size, security), now);
        finish(
            st,
            &mut job,
            def,
            layer.objective.level,
            item.as_ref(),
            r_text,
        );
        return Some(job);
    }
    None
}

/// The item's name as the pack says it, lower case.
fn item_name(st: &AppState, item: &ItemId) -> String {
    st.content
        .item(item)
        .map_or_else(|| item.0.clone(), |i| i.name.to_lowercase())
}

/// The deed, as the job's first objective says it, and the brief's task.
fn deed(
    st: &AppState,
    kind: ObjectiveKind,
    def: &ObjectiveDef,
    item: Option<&ItemId>,
) -> (String, String) {
    let at = &def.name;
    let thing = item.map(|i| item_name(st, i)).unwrap_or_default();
    match kind {
        ObjectiveKind::Take => (
            format!("Take the {thing} from {at}"),
            format!("The {thing} is kept there. Bring it to me."),
        ),
        ObjectiveKind::Copy => (
            format!("Copy the vault in {at}"),
            "Copy the vault. If they sound the alarm, it purges itself, so be quick.".to_string(),
        ),
        ObjectiveKind::Wipe => (
            format!("Wipe the vault in {at}"),
            "Wipe the vault. A deck does it quietly; a crowbar does it loudly.".to_string(),
        ),
        ObjectiveKind::Plant => (
            format!("Plant the tap in {at}"),
            "Press the tap I'm giving you to the rack and leave it there.".to_string(),
        ),
        ObjectiveKind::Reach => (
            format!("Get into {at}"),
            "I need eyes in that room. Stand in it.".to_string(),
        ),
    }
}

/// A site job's words and objectives: the deed, getting out, the hand-in.
fn finish(
    st: &AppState,
    job: &mut Job,
    def: &ObjectiveDef,
    level: i32,
    item: Option<&ItemId>,
    r_text: u64,
) {
    let Kind::Site {
        building,
        kind,
        security,
        ..
    } = job.kind
    else {
        return;
    };
    let name = crate::net::building_name(st, building);
    let floor = FloorPlan::label(level);
    let (text, task) = deed(st, kind, def, item);
    let slots = [
        ("building", name.clone()),
        ("floor", floor.clone()),
        ("security", security.to_string()),
        ("objective", capital(&def.name)),
        ("task", task),
    ];
    job.title = st.flavor.title(KindTag::Site, r_text, &slots);
    job.brief = st.flavor.brief(KindTag::Site, r_text, &slots);
    job.objectives = vec![
        Objective {
            text: format!("{text} · {name} · {floor}"),
            done: false,
        },
        Objective {
            text: format!("Get out of {name}"),
            done: false,
        },
        Objective {
            text: super::generate::handin_text(st, job.fixer),
            done: false,
        },
    ];
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// Taking a site job: its copy begins on the building, and a Plant hands you the tap.
/// Err, and nothing taken, when the building can't take the copy now.
pub fn begin(st: &mut AppState, kind: &Kind) -> Result<(), String> {
    let Some((b, inputs)) = kind.site() else {
        return Ok(());
    };
    crate::venues::begin(st, b, inputs)?;
    if inputs.kind == ObjectiveKind::Plant {
        st.character.give(ItemId::new(TAP), 1);
    }
    Ok(())
}

impl Jobs {
    /// `Taken → Failed` at minute `now`, the standing down as for a late job.
    pub fn fail(&mut self, id: JobId, now: u64) -> bool {
        let Some(j) = self.list.iter_mut().find(|j| j.id == id) else {
            return false;
        };
        if j.state != State::Taken {
            return false;
        }
        j.state = State::Failed;
        let fixer = j.fixer;
        self.close(id, fixer, LOSS_LATE, now);
        true
    }
}

/// A site job fails: the log, and the fixer's mail.
fn fail(st: &mut AppState, id: JobId, why: &str) {
    let now = st.net.now(st);
    let Some(j) = st.jobs.get(id).cloned() else {
        return;
    };
    if !st.jobs.fail(id, now) {
        return;
    }
    let fixer = super::ui::fixer_name(st, j.fixer);
    st.warn(format!("`{}` · failed · {why}", j.title));
    let line = st.flavor.line(Say::Late, id.0 as u64).to_string();
    st.journal.mails.push(crate::credentials::Mail {
        from: fixer,
        subject: format!("{} · failed", j.title.to_lowercase()),
        body: vec![format!("{}.", capital(why)), line],
        read: false,
        at: now,
        job: Some(id),
        story: None,
    });
}

/// Whether you carry what the job's hand-in takes, or its sifted shard.
fn carries_handin(st: &AppState, kind: &Kind) -> bool {
    let Some(item) = super::handin_item(kind) else {
        return true;
    };
    st.character.count(&item) > 0
        || super::handin_sifted(kind).is_some_and(|s| st.character.count(&s) > 0)
}

/// On the clock: each Taken site job's objective met (standing on a Reach's spot, a Take's
/// item out of its container), its vault purged in an alarm, you out of the building with
/// it done, its item lost; and the copy of every site job closed let go.
pub fn tick(st: &mut AppState) {
    let now = st.net.now(st);
    let jobs: Vec<Job> = (st.jobs.list.iter())
        .filter(|j| j.kind.site().is_some())
        .cloned()
        .collect();
    for j in jobs {
        let Some((b, inputs)) = j.kind.site() else {
            continue;
        };
        if j.state != State::Taken {
            let ours = st
                .interiors
                .copy(b)
                .is_some_and(|c| c.inputs == inputs && !c.ending);
            if ours {
                let _ = crate::venues::end(st, b);
            }
            continue;
        }
        let Some(layer) = layer_for(st, b, &inputs) else {
            continue;
        };
        let o = layer.objective;
        let met = j.objectives.first().is_some_and(|x| x.done);
        if !met {
            let here = st.player.as_ref().map(|p| (p.place, p.pos));
            let place = Place::Floor {
                building: b,
                level: o.level,
            };
            let reached = inputs.kind == ObjectiveKind::Reach && here == Some((place, o.at));
            let taken = inputs.kind == ObjectiveKind::Take && item_taken(st, b, &j.kind);
            if reached || taken {
                super::send(st, Event::Met { building: b });
            }
            if inputs.kind.on_the_vault() {
                purge(st, j.id, b, inputs.kind, now);
            }
            continue;
        }
        if !j.objectives.get(1).is_some_and(|x| x.done)
            && !crate::venues::inside(st, b)
            && carries_handin(st, &j.kind)
        {
            super::send(st, Event::Out { building: b });
        }
        if let Kind::Site {
            kind: ObjectiveKind::Take,
            item: Some(item),
            ..
        } = &j.kind
            && st.character.count(item) == 0
            && !st.ground.holds(item)
        {
            fail(st, j.id, &format!("the {} is lost", item_name(st, item)));
        }
    }
}

/// Whether a Take's item has left its container: it was opened and holds it no more.
fn item_taken(st: &AppState, b: BuildingId, kind: &Kind) -> bool {
    let Kind::Site {
        item: Some(item), ..
    } = kind
    else {
        return false;
    };
    let Some(at) = take_container(st, b) else {
        return false;
    };
    let opened = crate::containers::state(st, at).is_some_and(|s| s.opened.is_some());
    let Some(c) = crate::containers::find(st, at) else {
        return false;
    };
    opened
        && !crate::containers::contents(st, &c)
            .iter()
            .any(|l| matches!(l, crate::containers::state::Loot::Item { item: i, .. } if i == item))
}

/// A vault's purge: an alarm on the building starts it, and `PURGE_MINUTES` later the
/// vault wipes itself. An uncopied Copy fails; a Wipe is done, at half its pay.
fn purge(st: &mut AppState, id: JobId, b: BuildingId, kind: ObjectiveKind, now: u64) {
    let Some(copy) = st.interiors.copy(b) else {
        return;
    };
    match copy.purge {
        None if st.net.alert(b, now) => {
            if let Some(c) = st.interiors.copy_mut(b) {
                c.purge = Some(now + PURGE_MINUTES);
            }
            let name = crate::net::building_name(st, b);
            st.warn(format!(
                "`{name}`'s vault purges in {PURGE_MINUTES} minutes"
            ));
        }
        Some(at) if now >= at && !copy.wiped => {
            if let Some(c) = st.interiors.copy_mut(b) {
                c.wiped = true;
            }
            match kind {
                ObjectiveKind::Copy => fail(st, id, "the vault wiped itself in the alarm"),
                _ => {
                    if let Some(j) = st.jobs.list.iter_mut().find(|j| j.id == id) {
                        j.pay /= 2;
                    }
                    st.warn("the vault wiped itself in the alarm · half pay for a loud job");
                    super::send(st, Event::Met { building: b });
                }
            }
        }
        _ => {}
    }
}

/// Minutes left on `b`'s vault purge, while one runs and the vault stands.
pub fn purge_left(st: &AppState, b: BuildingId) -> Option<u64> {
    let copy = st.interiors.copy(b).filter(|c| c.live && !c.wiped)?;
    let at = copy.purge?;
    Some(at.saturating_sub(st.net.now(st)))
}

/// What working the objective by hand is: planting the tap, or forcing the rack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Work {
    Tap,
    Rack,
}

/// Seconds the tap takes to plant.
pub const TAP_SECONDS: u64 = 4;

/// Working the objective at `at` by hand: what, and how long it takes you in ticks, or why
/// not. A Plant wants the tap carried; a Wipe's rack wants Body for its grade.
pub fn can_work(st: &AppState, at: ContainerAt) -> Result<(Work, u64), String> {
    let (kind, place, spot, met) = objective(st, at.building).ok_or("no job here")?;
    if place
        != (Place::Floor {
            building: at.building,
            level: at.level,
        })
        || spot != at.at
    {
        return Err("no job here".to_string());
    }
    if met {
        return Err("done already".to_string());
    }
    if st.net.engaged.is_some() {
        return Err("not now".to_string());
    }
    match kind {
        ObjectiveKind::Plant => {
            if st.character.count(&ItemId::new(TAP)) == 0 {
                return Err("no tap on you".to_string());
            }
            Ok((Work::Tap, TAP_SECONDS * crate::app::FPS))
        }
        ObjectiveKind::Wipe => {
            if wiped(st, at.building) {
                return Err("wiped already".to_string());
            }
            let grade = rack_grade(st, at.building);
            let body = st.character.attribute(&st.content, game::Attribute::Body);
            let need = crate::containers::force::need(grade);
            if body < need {
                return Err(format!("Body {need}"));
            }
            Ok((Work::Rack, crate::containers::force::ticks(grade, body)))
        }
        _ => Err("nothing to do by hand".to_string()),
    }
}

/// A rack's grade, as a guarded container's: the building's lock maker's.
pub fn rack_grade(st: &AppState, b: BuildingId) -> u8 {
    crate::keyjack::rating(st, b).clamp(1, 3)
}

/// The work is done: the tap planted, or the rack forced (which sounds the alarm).
pub fn finish_work(st: &mut AppState, at: ContainerAt, work: Work) {
    let b = at.building;
    match work {
        Work::Tap => {
            game::remove_from_pack(&mut st.character, &ItemId::new(TAP), 1);
            st.say("the tap is planted");
            super::send(st, Event::Met { building: b });
        }
        Work::Rack => {
            st.say("the rack gives · the vault is wiped");
            wipe(st, b);
            crate::net::lock_down(st, b);
        }
    }
}

/// The interact row on the objective beside you or under you, when it is worked by hand.
pub fn work_row(st: &AppState) -> Option<(Pos, ContainerAt, &'static str)> {
    let p = st.player.as_ref()?;
    let Place::Floor { building, level } = p.place else {
        return None;
    };
    let (kind, place, spot, met) = objective(st, building)?;
    if met || place != p.place || spot.chebyshev(p.pos) > 1 {
        return None;
    }
    let verb = match kind {
        ObjectiveKind::Plant => "Plant the tap",
        ObjectiveKind::Wipe if !wiped(st, building) => "Force the rack",
        _ => return None,
    };
    Some((
        spot,
        ContainerAt {
            building,
            level,
            at: spot,
        },
        verb,
    ))
}

/// A site job's stop on its card: what and where, the verb to do, the verb once done.
pub fn stop_words(st: &AppState, job: &Job) -> (String, &'static str, &'static str) {
    let Some((b, inputs)) = job.kind.site() else {
        return (String::new(), "", "");
    };
    let name = crate::net::building_name(st, b);
    let at = def(st, b, &inputs).map_or_else(|| "the objective".to_string(), |d| d.name.clone());
    let floor = layer_for(st, b, &inputs)
        .map(|l| format!(" · {}", FloorPlan::label(l.objective.level)))
        .unwrap_or_default();
    let thing = match &job.kind {
        Kind::Site { item: Some(i), .. } if inputs.kind == ObjectiveKind::Take => {
            format!("the {} · ", item_name(st, i))
        }
        _ => String::new(),
    };
    let (todo, did) = match inputs.kind {
        ObjectiveKind::Take => ("take it", "taken"),
        ObjectiveKind::Copy => ("copy the vault", "vault copied"),
        ObjectiveKind::Wipe => ("wipe the vault", "vault wiped"),
        ObjectiveKind::Plant => ("plant the tap", "tap planted"),
        ObjectiveKind::Reach => ("get in", "reached"),
    };
    (format!("{thing}{at}{floor} · {name}"), todo, did)
}

/// A site job's size.
pub fn size_of(job: &Job) -> MissionSize {
    job.kind.site().map_or(MissionSize::Small, |(_, i)| i.size)
}

/// A site job's security.
pub fn security_of_job(job: &Job) -> u8 {
    job.kind.site().map_or(0, |(_, i)| i.security)
}

/// What the Look says of a site job's objective on tile `pos` of `place`: where it is and
/// what the job asks, `done` once met. A Take's container says it too.
pub fn look_line(st: &AppState, place: Place, pos: Pos) -> Option<String> {
    let Place::Floor { building, level } = place else {
        return None;
    };
    let (kind, at_place, at, met) = objective(st, building)?;
    let here = (at_place == place && at == pos)
        || take_container(st, building)
            == Some(ContainerAt {
                building,
                level,
                at: pos,
            });
    if !here {
        return None;
    }
    let (j, inputs) = taken_on(st, building)?;
    let name =
        def(st, building, &inputs).map_or_else(|| "the objective".into(), |d| d.name.clone());
    let ask = match (kind, &j.kind) {
        (ObjectiveKind::Take, Kind::Site { item: Some(i), .. }) => {
            format!("take the {}", item_name(st, i))
        }
        (ObjectiveKind::Copy, _) => "copy the vault".to_string(),
        (ObjectiveKind::Wipe, _) => "wipe the vault".to_string(),
        (ObjectiveKind::Plant, _) => "plant the tap".to_string(),
        _ => "get in".to_string(),
    };
    Some(match met {
        true => format!("{name} · your job · done"),
        false => format!("{name} · your job: {ask}"),
    })
}

#[cfg(test)]
mod tests;
