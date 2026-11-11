//! Jobs: the work the fixers hand out, their deadlines, and what each fixer thinks
//! of you. Runtime data, never content; the rest of the app reports events here.

pub mod card;
pub mod flavor;
pub mod generate;
pub mod ui;

pub use flavor::*;
pub use generate::*;

use crate::app::AppState;
use citygen::{BuildingId, Place, Pos};
use game::ItemId;
use sim::PersonId;
use std::collections::BTreeMap;

/// Standing at or under which a fixer refuses you.
pub const REFUSE_AT: i32 = -3;
/// Standing from which a fixer offers Copy work.
pub const COPY_AT: i32 = 2;
/// Standing from which a fixer offers Plant work.
pub const PLANT_AT: i32 = 4;
/// Standing from which a Copy may be a vault.
pub const VAULT_AT: i32 = 6;
pub const GAIN_DONE: i32 = 1;
pub const LOSS_DROP: i32 = -1;
pub const LOSS_LATE: i32 = -2;
pub const HOURS_FETCH: u64 = 12;
pub const HOURS_COPY: u64 = 24;
pub const HOURS_PLANT: u64 = 24;
pub const HOURS_DELIVER: u64 = 8;
pub const PAY_FETCH: u32 = 200;
/// Plus the manhattan distance over four.
pub const PAY_DELIVER_BASE: u32 = 100;
/// Times the security; `PAY_VAULT_PER_SECURITY` for a vault.
pub const PAY_COPY_PER_SECURITY: u32 = 3;
pub const PAY_VAULT_PER_SECURITY: u32 = 5;
/// Plus `PAY_PLANT_PER_SECURITY` times the security.
pub const PAY_PLANT_BASE: u32 = 400;
pub const PAY_PLANT_PER_SECURITY: u32 = 2;
/// Manhattan tiles from the fixer's spot to a Fetch's door.
pub const FETCH_RANGE: i32 = 80;
/// A package never crosses the street.
pub const DELIVER_MIN_RANGE: i32 = 30;
/// Minutes of their shift inside a Deliver's window, at least, for someone who comes in
/// to work from out of town: once they walk out they're out of reach until their next
/// shift (someone with a home in the city is still at home). No fewer than the walk to
/// them at a walker's pace takes, too; an hour covers the width of the largest city.
pub const DELIVER_ON_SHIFT: u64 = 60;
/// The pane turns the clock red under this many minutes.
pub const WARN_MINUTES: u64 = 60;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct JobId(pub u32);

/// What a job is about, with its target in the city.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Kind {
    /// A paydata shard lies on `tile` of flat `number` on `level` of `building`.
    Fetch {
        building: BuildingId,
        level: i32,
        number: u16,
        tile: Pos,
    },
    /// Copy the building's server, or its vault when `vault`.
    Copy { building: BuildingId, vault: bool },
    /// A backdoor on the building's host, still standing when you hand in.
    Plant { building: BuildingId },
    /// Put `item` into this person's hands.
    Deliver { to: PersonId, item: ItemId },
}

impl Kind {
    pub fn tag(&self) -> KindTag {
        match self {
            Kind::Fetch { .. } => KindTag::Fetch,
            Kind::Copy { .. } => KindTag::Copy,
            Kind::Plant { .. } => KindTag::Plant,
            Kind::Deliver { .. } => KindTag::Deliver,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum State {
    Offered,
    Taken,
    Done,
    Failed,
    Dropped,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Objective {
    pub text: String,
    pub done: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Job {
    pub id: JobId,
    pub fixer: PersonId,
    pub kind: Kind,
    pub title: String,
    /// The fixer's words; the mail body repeats them.
    pub brief: Vec<String>,
    /// The last one is always the hand-in.
    pub objectives: Vec<Objective>,
    pub pay: u32,
    /// The window, in game hours.
    pub hours: u64,
    /// Game minutes.
    pub offered: u64,
    pub taken: Option<u64>,
    /// `taken + hours × 60`.
    pub deadline: Option<u64>,
    pub state: State,
    /// When it ended, done, late or dropped.
    pub closed: Option<Closed>,
}

/// How a job ended: the game minute, and the fixer's standing just after.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Closed {
    pub at: u64,
    pub standing: i32,
}

/// What a fixer thinks of you, and how many offers they have rolled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Fixer {
    pub standing: i32,
    pub rolled: u32,
}

/// A job that ran out of time this minute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Late {
    pub job: JobId,
}

/// Something the world did that a job may be waiting for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// Something left the ground here, and no paydata is left on the tile.
    Taken {
        place: Place,
        pos: Pos,
    },
    Copied {
        building: BuildingId,
        vault: bool,
    },
    Planted {
        building: BuildingId,
    },
    /// The package reached its person.
    Handed {
        job: JobId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Ticked { job: JobId, objective: usize },
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Jobs {
    pub list: Vec<Job>,
    pub fixers: BTreeMap<PersonId, Fixer>,
    next: u32,
    last_minute: Option<u64>,
}

/// The item a hand-in takes from the pack, when the kind brings one back.
pub fn handin_item(kind: &Kind) -> Option<ItemId> {
    match kind {
        Kind::Fetch { .. } | Kind::Copy { vault: false, .. } => Some(ItemId::new("paydata")),
        Kind::Copy { vault: true, .. } => Some(ItemId::new("vault_data")),
        Kind::Plant { .. } | Kind::Deliver { .. } => None,
    }
}

/// Minutes left as the pane says them.
pub fn clock_text(left: u64) -> String {
    match left {
        0 => "late".to_string(),
        m if m < 60 => format!("{m}m"),
        m => format!("{}h {}m", m / 60, m % 60),
    }
}

/// What a fixer thinks of you, in a word.
pub fn standing_word(standing: i32) -> &'static str {
    match standing {
        s if s <= REFUSE_AT => "refuses you",
        s if s < 0 => "cold",
        s if s < COPY_AT => "new",
        s if s < PLANT_AT => "known",
        s if s < VAULT_AT => "trusted",
        _ => "inner circle",
    }
}

/// Work done in the Net, which a runner needs a deck in the slot for.
pub fn is_net_work(kind: KindTag) -> bool {
    matches!(kind, KindTag::Copy | KindTag::Plant)
}

/// Whether the runner has a deck in the DECK slot (one in the pack doesn't count).
pub fn has_deck(st: &crate::app::AppState) -> bool {
    st.character.deck(&st.content).is_some()
}

/// The standing from which a fixer offers this work.
pub fn opens_at(kind: KindTag, vault: bool) -> i32 {
    match kind {
        KindTag::Fetch | KindTag::Deliver => REFUSE_AT + 1,
        KindTag::Copy if vault => VAULT_AT,
        KindTag::Copy => COPY_AT,
        KindTag::Plant => PLANT_AT,
    }
}

/// The minute of the day at game minute `m`.
pub fn day_minute(m: u64) -> u16 {
    crate::ui::hud::minute_at(m * sim::TICKS_PER_MINUTE)
}

/// Whether minute of the day `t` falls in the shift `(from, to)`; a shift whose end is
/// before its start wraps past midnight.
pub fn in_shift(t: u16, (from, to): (u16, u16)) -> bool {
    if from <= to {
        from <= t && t < to
    } else {
        t >= from || t < to
    }
}

/// Game minutes from `now` until the shift next starts or ends.
pub fn shift_turn(now: u64, shift: (u16, u16)) -> u64 {
    let on = in_shift(day_minute(now), shift);
    (1..=24 * 60)
        .find(|d| in_shift(day_minute(now + d), shift) != on)
        .unwrap_or(24 * 60)
}

/// Where the fixer is at the end of a job's window: on their corner, or off it. A fixer
/// off the corner still takes the hand-in wherever you find them; the window only
/// says where to look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    /// On the corner at the deadline.
    Through,
    /// Off the corner from this game minute, before the deadline, and not back in time.
    Leaves(u64),
    /// Off the corner from now until after the deadline.
    Off,
}

pub fn corner(now: u64, deadline: u64, shift: (u16, u16)) -> Corner {
    if in_shift(day_minute(deadline), shift) {
        return Corner::Through;
    }
    match (now..deadline)
        .rev()
        .find(|m| in_shift(day_minute(*m), shift))
    {
        Some(m) => Corner::Leaves(m + 1),
        None => Corner::Off,
    }
}

/// A fixer's hours on their corner.
pub fn shift_of(st: &AppState, fixer: PersonId) -> Option<(u16, u16)> {
    st.world.as_ref()?.work_hours(fixer)
}

impl Jobs {
    /// Adds a job under the next id and returns it.
    pub fn push(&mut self, mut job: Job) -> JobId {
        job.id = JobId(self.next);
        self.next += 1;
        let id = job.id;
        self.list.push(job);
        id
    }

    pub fn get(&self, id: JobId) -> Option<&Job> {
        self.list.iter().find(|j| j.id == id)
    }

    fn get_mut(&mut self, id: JobId) -> Option<&mut Job> {
        self.list.iter_mut().find(|j| j.id == id)
    }

    /// Zero for a fixer never met.
    pub fn standing(&self, fixer: PersonId) -> i32 {
        self.fixers.get(&fixer).map_or(0, |f| f.standing)
    }

    pub fn fixer_mut(&mut self, fixer: PersonId) -> &mut Fixer {
        self.fixers.entry(fixer).or_default()
    }

    pub fn offered(&self, fixer: PersonId) -> Option<&Job> {
        self.list
            .iter()
            .find(|j| j.fixer == fixer && j.state == State::Offered)
    }

    pub fn taken(&self, fixer: PersonId) -> Option<&Job> {
        self.list
            .iter()
            .find(|j| j.fixer == fixer && j.state == State::Taken)
    }

    /// The oldest job in hand: the pane's.
    pub fn active(&self) -> Option<&Job> {
        self.list.iter().find(|j| j.state == State::Taken)
    }

    /// The kinds a fixer offers at this standing; none when they refuse you.
    pub fn allowed(standing: i32) -> Vec<KindTag> {
        if standing <= REFUSE_AT {
            return Vec::new();
        }
        let mut out = vec![KindTag::Fetch, KindTag::Deliver];
        if standing >= COPY_AT {
            out.push(KindTag::Copy);
        }
        if standing >= PLANT_AT {
            out.push(KindTag::Plant);
        }
        out
    }

    /// The kinds a fixer offers a runner: Copy (vaults too) and Plant are Net work and
    /// need a deck in the DECK slot; Fetch and Deliver are for every build.
    pub fn allowed_for(standing: i32, deck: bool) -> Vec<KindTag> {
        let mut out = Self::allowed(standing);
        if !deck {
            out.retain(|k| !is_net_work(*k));
        }
        out
    }

    /// `Offered → Taken`, the clock started.
    pub fn take(&mut self, id: JobId, now: u64) -> bool {
        let Some(j) = self.get_mut(id) else {
            return false;
        };
        if j.state != State::Offered {
            return false;
        }
        j.state = State::Taken;
        j.taken = Some(now);
        j.deadline = Some(now + j.hours * 60);
        true
    }

    /// `Taken → Dropped` at minute `now`, the standing down.
    pub fn drop_job(&mut self, id: JobId, now: u64) -> bool {
        let Some(j) = self.get_mut(id) else {
            return false;
        };
        if j.state != State::Taken {
            return false;
        }
        j.state = State::Dropped;
        let fixer = j.fixer;
        self.close(id, fixer, LOSS_DROP, now);
        true
    }

    /// Ends a Taken job at minute `now`: every objective ticked, the standing up, the
    /// pay returned. The caller checks `ready` first.
    pub fn hand_in(&mut self, id: JobId, now: u64) -> Option<u32> {
        let j = self.get_mut(id)?;
        if j.state != State::Taken {
            return None;
        }
        j.state = State::Done;
        for o in &mut j.objectives {
            o.done = true;
        }
        let (fixer, pay) = (j.fixer, j.pay);
        self.close(id, fixer, GAIN_DONE, now);
        Some(pay)
    }

    /// Moves the fixer's standing by `delta` and stamps the job closed.
    fn close(&mut self, id: JobId, fixer: PersonId, delta: i32, now: u64) {
        let f = self.fixer_mut(fixer);
        f.standing += delta;
        let standing = f.standing;
        if let Some(j) = self.get_mut(id) {
            j.closed = Some(Closed { at: now, standing });
        }
    }

    /// Whether the hand-in can happen now, else why not, in the fixer's words.
    pub fn ready(
        &self,
        id: JobId,
        backdoor_stands: &dyn Fn(BuildingId) -> bool,
        carries: &dyn Fn(&ItemId) -> bool,
    ) -> Result<(), String> {
        let j = self.get(id).ok_or_else(|| "no such job".to_string())?;
        if j.state != State::Taken {
            return Err("not in hand".to_string());
        }
        let n = j.objectives.len();
        if j.objectives
            .iter()
            .take(n.saturating_sub(1))
            .any(|o| !o.done)
        {
            return Err(match j.kind {
                Kind::Fetch { .. } | Kind::Copy { .. } => "the shard is not yet yours",
                Kind::Plant { .. } => "the backdoor is not planted yet",
                Kind::Deliver { .. } => "the package is not delivered",
            }
            .to_string());
        }
        if let Kind::Plant { building } = j.kind
            && !backdoor_stands(building)
        {
            return Err("the backdoor burned · plant it again".to_string());
        }
        if let Some(item) = handin_item(&j.kind)
            && !carries(&item)
        {
            return Err("the shard is not on you".to_string());
        }
        Ok(())
    }

    /// Ticks the first objective of every Taken job the event answers, once.
    pub fn on(&mut self, ev: Event) -> Vec<Change> {
        let mut out = Vec::new();
        for j in self.list.iter_mut().filter(|j| j.state == State::Taken) {
            let hit = match (&j.kind, &ev) {
                (
                    Kind::Fetch {
                        building,
                        level,
                        tile,
                        ..
                    },
                    Event::Taken { place, pos },
                ) => {
                    *place
                        == Place::Floor {
                            building: *building,
                            level: *level,
                        }
                        && pos == tile
                }
                (
                    Kind::Copy { building, vault },
                    Event::Copied {
                        building: b,
                        vault: v,
                    },
                ) => building == b && vault == v,
                (Kind::Plant { building }, Event::Planted { building: b }) => building == b,
                (Kind::Deliver { .. }, Event::Handed { job }) => *job == j.id,
                _ => false,
            };
            if hit
                && let Some(o) = j.objectives.first_mut()
                && !o.done
            {
                o.done = true;
                out.push(Change::Ticked {
                    job: j.id,
                    objective: 0,
                });
            }
        }
        out
    }

    /// The last game minute `minute` ran on.
    #[cfg(test)]
    pub(crate) fn last_minute(&self) -> Option<u64> {
        self.last_minute
    }

    /// Once per game minute: jobs past their deadline fail and the fixer remembers.
    pub fn minute(&mut self, now: u64) -> Vec<Late> {
        if self.last_minute.is_some_and(|m| now <= m) {
            return Vec::new();
        }
        self.last_minute = Some(now);
        let mut late = Vec::new();
        let mut losses = Vec::new();
        for j in self.list.iter_mut().filter(|j| j.state == State::Taken) {
            if j.deadline.is_some_and(|d| d < now) {
                j.state = State::Failed;
                late.push(Late { job: j.id });
                losses.push((j.id, j.fixer));
            }
        }
        for (id, f) in losses {
            self.close(id, f, LOSS_LATE, now);
        }
        late
    }

    /// Minutes to the deadline, saturating.
    pub fn left(job: &Job, now: u64) -> Option<u64> {
        job.deadline.map(|d| d.saturating_sub(now))
    }
}

/// Logs what an event ticked.
pub fn report(st: &mut AppState, changes: Vec<Change>) {
    for Change::Ticked { job, objective } in changes {
        if let Some(j) = st.jobs.get(job) {
            let line = format!("`{}` · {}", j.title, j.objectives[objective].text);
            st.story(line);
        }
    }
}

/// Tells the jobs what the world did, and logs what it ticked.
pub fn send(st: &mut AppState, ev: Event) {
    let changes = st.jobs.on(ev);
    report(st, changes);
}

/// The job clock: late jobs fail, the fixer writes.
pub fn tick(st: &mut AppState) {
    let now = st.net.now(st);
    for Late { job } in st.jobs.minute(now) {
        let Some(j) = st.jobs.get(job).cloned() else {
            continue;
        };
        let fixer = ui::fixer_name(st, j.fixer);
        st.warn(format!(
            "`{}` · too late · {fixer} will hear of it",
            j.title
        ));
        let line = st.flavor.line(Say::Late, job.0 as u64).to_string();
        st.journal.mails.push(crate::credentials::Mail {
            from: fixer,
            subject: format!("{} · late", j.title.to_lowercase()),
            body: vec![line],
            read: false,
            at: now,
            job: Some(job),
            story: None,
        });
    }
}

/// Work the city already holds against a job taken just now: a shard fetched, a
/// backdoor planted or a copy made while it was only on offer counts.
fn credit_done_work(st: &mut AppState, id: JobId) {
    let Some(j) = st.jobs.get(id).cloned() else {
        return;
    };
    let now = st.net.now(st);
    match j.kind {
        Kind::Fetch {
            building,
            level,
            tile,
            ..
        } => {
            let place = Place::Floor { building, level };
            let left = st
                .ground
                .at(place, tile)
                .iter()
                .any(|s| s.item == ItemId::new("paydata"));
            if !left {
                send(st, Event::Taken { place, pos: tile });
            }
        }
        Kind::Plant { building } => {
            if st.backdoors.held(building).is_some() {
                send(st, Event::Planted { building });
            }
        }
        Kind::Copy { building, vault } => {
            let kind = if vault {
                citygen::net::NodeKind::DataVault
            } else {
                citygen::net::NodeKind::Server
            };
            let copied = st.city.as_ref().is_some_and(|c| {
                c.net.nodes.iter().any(|n| {
                    n.building == Some(building)
                        && n.kind == kind
                        && st.net.copied(game::HackTarget::Node(n.id), now)
                })
            });
            if copied {
                send(st, Event::Copied { building, vault });
            }
        }
        Kind::Deliver { .. } => {}
    }
}

/// Takes an offered job: the state, the mail, a Deliver's package. Logs the line.
pub fn take_job(st: &mut AppState, id: JobId) -> Result<String, String> {
    let now = st.net.now(st);
    let Some(j) = st.jobs.get(id).cloned() else {
        return Err("no such job".into());
    };
    if !st.jobs.take(id, now) {
        return Err("not on offer".into());
    }
    credit_done_work(st, id);
    if let Kind::Deliver { item, .. } = &j.kind {
        st.character.give(item.clone(), 1);
    }
    let fixer = ui::fixer_name(st, j.fixer);
    let mut body = j.brief.clone();
    body.push(generate::where_line(st, j.fixer));
    let by = crate::ui::hud::minute_at((now + j.hours * 60) * sim::TICKS_PER_MINUTE);
    body.push(format!("By {:02}:{:02}.", by / 60, by % 60));
    body.push(fixer.clone());
    st.journal.mails.push(crate::credentials::Mail {
        from: fixer.clone(),
        subject: j.title.to_lowercase(),
        body,
        read: false,
        at: now,
        job: Some(id),
        story: None,
    });
    let said = st.flavor.line(Say::Taken, id.0 as u64).to_string();
    let line = format!("`{}` · taken · {fixer}: \u{201c}{said}\u{201d}", j.title);
    st.story(line.clone());
    Ok(line)
}

#[cfg(test)]
pub(crate) mod testutil {
    use super::{HOURS_FETCH, Job, JobId, Kind, Objective, PAY_FETCH, State};
    use crate::app::AppState;
    use citygen::{BuildingId, GenParams, Generator, Pos, SizePreset};

    /// Seed 8, medium, seven districts: a megablock home, a fixer, the welcome favour.
    pub fn state_home() -> AppState {
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
        let home = st.home.unwrap().home;
        crate::credentials::prepare(&mut st, home.building);
        crate::credentials::move_in(&mut st);
        crate::jobs::first_favour(&mut st);
        st
    }

    pub fn a_fixer(st: &AppState) -> sim::PersonId {
        st.world
            .as_ref()
            .unwrap()
            .people()
            .iter()
            .find(|p| p.profession == "fixer")
            .unwrap()
            .id
    }

    /// The JOBS mock's six jobs in place of the favour, each fixer in turn giving one:
    /// a fetch and a delivery in hand (the package delivered), a copy on offer, a
    /// plant done, a fetch late, a delivery dropped. Every fixer at standing 6.
    pub fn six_jobs(st: &mut AppState) -> Vec<JobId> {
        use super::{KindTag, generate};
        st.jobs.list.clear();
        let fixers: Vec<sim::PersonId> = st
            .world
            .as_ref()
            .unwrap()
            .people()
            .iter()
            .filter(|p| p.profession == "fixer")
            .map(|p| p.id)
            .collect();
        let kinds = [
            KindTag::Fetch,
            KindTag::Deliver,
            KindTag::Copy,
            KindTag::Plant,
            KindTag::Fetch,
            KindTag::Deliver,
        ];
        let mut ids = Vec::new();
        for (i, k) in kinds.into_iter().enumerate() {
            let job = fixers
                .iter()
                .cycle()
                .skip(i)
                .take(fixers.len())
                .find_map(|f| {
                    st.jobs.fixer_mut(*f).standing = 6;
                    (0..40).find_map(|n| generate(st, *f, k, n + i as u32))
                })
                .expect("a job of each kind");
            ids.push(st.jobs.push(job));
        }
        for (i, id) in ids.iter().enumerate() {
            if i != 2 {
                crate::jobs::take_job(st, *id).unwrap();
            }
        }
        st.jobs
            .list
            .iter_mut()
            .find(|j| j.id == ids[1])
            .unwrap()
            .objectives[0]
            .done = true;
        st.jobs.hand_in(ids[3], 0);
        // Late on its own: the minute would fail the fetch in hand with it.
        let fixer = st.jobs.get(ids[4]).unwrap().fixer;
        st.jobs.fixer_mut(fixer).standing += super::LOSS_LATE;
        let standing = st.jobs.standing(fixer);
        let j = st.jobs.list.iter_mut().find(|j| j.id == ids[4]).unwrap();
        j.state = State::Failed;
        j.closed = Some(super::Closed {
            at: j.deadline.unwrap() + 1,
            standing,
        });
        st.jobs.drop_job(ids[5], 0);
        ids
    }

    /// "The drop", in hand, one of three objectives done; and "First jack", done.
    /// For layout tests that want the pane and the tab filled.
    pub fn sample_jobs(st: &mut AppState) -> JobId {
        let job = |title: &str, texts: &[&str]| Job {
            id: JobId(0),
            fixer: sim::PersonId(0),
            kind: Kind::Fetch {
                building: BuildingId(0),
                level: 1,
                number: 101,
                tile: Pos::new(1, 1),
            },
            title: title.to_string(),
            brief: vec!["Bring it.".to_string()],
            objectives: texts
                .iter()
                .map(|t| Objective {
                    text: t.to_string(),
                    done: false,
                })
                .collect(),
            pay: PAY_FETCH,
            hours: HOURS_FETCH,
            offered: 0,
            taken: None,
            deadline: None,
            state: State::Offered,
            closed: None,
        };
        let drop = st.jobs.push(job(
            "The drop",
            &[
                "Pick up the package at the noodle stall",
                "Cross the corp core without a scan",
                "Hand it to the docks fixer",
            ],
        ));
        st.jobs.take(drop, 0);
        st.jobs.list.last_mut().unwrap().objectives[0].done = true;
        let jack = st.jobs.push(job("First jack", &["Jack in", "Jack out"]));
        st.jobs.take(jack, 0);
        st.jobs.hand_in(jack, 0);
        drop
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{BuildingId, Place, Pos, StratumKind};
    use game::ItemId;
    use sim::PersonId;

    fn fetch(fixer: u32) -> Job {
        Job {
            id: JobId(0),
            fixer: PersonId(fixer),
            kind: Kind::Fetch {
                building: BuildingId(3),
                level: 11,
                number: 1104,
                tile: Pos::new(5, 5),
            },
            title: "The shard".into(),
            brief: vec!["Bring it.".into()],
            objectives: vec![
                Objective {
                    text: "Take the shard".into(),
                    done: false,
                },
                Objective {
                    text: "Bring it to Mezz".into(),
                    done: false,
                },
            ],
            pay: PAY_FETCH,
            hours: HOURS_FETCH,
            offered: 0,
            taken: None,
            deadline: None,
            state: State::Offered,
            closed: None,
        }
    }

    fn flat() -> Place {
        Place::Floor {
            building: BuildingId(3),
            level: 11,
        }
    }

    #[test]
    fn allowed_kinds_open_with_standing() {
        assert_eq!(Jobs::allowed(0), vec![KindTag::Fetch, KindTag::Deliver]);
        assert_eq!(
            Jobs::allowed(COPY_AT),
            vec![KindTag::Fetch, KindTag::Deliver, KindTag::Copy]
        );
        assert_eq!(
            Jobs::allowed(PLANT_AT),
            vec![
                KindTag::Fetch,
                KindTag::Deliver,
                KindTag::Copy,
                KindTag::Plant
            ]
        );
        assert!(Jobs::allowed(REFUSE_AT).is_empty());
    }

    #[test]
    fn take_drop_and_hand_in_move_the_state_and_the_standing() {
        let mut jobs = Jobs::default();
        let id = jobs.push(fetch(7));
        assert_eq!(jobs.offered(PersonId(7)).map(|j| j.id), Some(id));
        assert!(jobs.take(id, 100));
        let j = jobs.get(id).unwrap();
        assert_eq!(
            (j.state, j.taken, j.deadline),
            (State::Taken, Some(100), Some(100 + HOURS_FETCH * 60))
        );
        assert_eq!(jobs.taken(PersonId(7)).map(|j| j.id), Some(id));
        assert_eq!(jobs.active().map(|j| j.id), Some(id));
        assert!(!jobs.take(id, 100), "taken twice is refused");
        assert!(jobs.drop_job(id, 0));
        assert_eq!(jobs.standing(PersonId(7)), LOSS_DROP);
        let id2 = jobs.push(fetch(7));
        jobs.take(id2, 200);
        jobs.on(Event::Taken {
            place: flat(),
            pos: Pos::new(5, 5),
        });
        assert_eq!(jobs.hand_in(id2, 0), Some(PAY_FETCH));
        assert_eq!(jobs.standing(PersonId(7)), LOSS_DROP + GAIN_DONE);
        assert!(jobs.get(id2).unwrap().objectives.iter().all(|o| o.done));
        assert!(jobs.active().is_none());
    }

    #[test]
    fn ready_wants_the_objectives_the_item_and_a_standing_backdoor() {
        let mut jobs = Jobs::default();
        let id = jobs.push(fetch(1));
        jobs.take(id, 0);
        let no = |_: &ItemId| false;
        let yes = |_: &ItemId| true;
        assert_eq!(
            jobs.ready(id, &|_| true, &yes),
            Err("the shard is not yet yours".to_string())
        );
        jobs.on(Event::Taken {
            place: flat(),
            pos: Pos::new(5, 5),
        });
        assert_eq!(
            jobs.ready(id, &|_| true, &no),
            Err("the shard is not on you".to_string())
        );
        assert_eq!(jobs.ready(id, &|_| true, &yes), Ok(()));
        let mut plant = fetch(1);
        plant.kind = Kind::Plant {
            building: BuildingId(9),
        };
        plant.objectives = vec![
            Objective {
                text: "Plant".into(),
                done: false,
            },
            Objective {
                text: "Report".into(),
                done: false,
            },
        ];
        let p = jobs.push(plant);
        jobs.take(p, 0);
        jobs.on(Event::Planted {
            building: BuildingId(9),
        });
        assert_eq!(
            jobs.ready(p, &|_| false, &yes),
            Err("the backdoor burned · plant it again".to_string())
        );
        assert_eq!(jobs.ready(p, &|b| b == BuildingId(9), &yes), Ok(()));
    }

    #[test]
    fn events_tick_each_kind_once() {
        let mut jobs = Jobs::default();
        let f = jobs.push(fetch(1));
        jobs.take(f, 0);
        let wrong = jobs.on(Event::Taken {
            place: Place::Stratum(StratumKind::Street),
            pos: Pos::new(5, 5),
        });
        assert!(wrong.is_empty());
        let right = jobs.on(Event::Taken {
            place: flat(),
            pos: Pos::new(5, 5),
        });
        assert_eq!(
            right,
            vec![Change::Ticked {
                job: f,
                objective: 0
            }]
        );
        assert!(
            jobs.on(Event::Taken {
                place: flat(),
                pos: Pos::new(5, 5),
            })
            .is_empty()
        );
        let mut copy = fetch(2);
        copy.kind = Kind::Copy {
            building: BuildingId(4),
            vault: true,
        };
        let c = jobs.push(copy);
        jobs.take(c, 0);
        assert!(
            jobs.on(Event::Copied {
                building: BuildingId(4),
                vault: false
            })
            .is_empty(),
            "a server copy is not the vault"
        );
        assert_eq!(
            jobs.on(Event::Copied {
                building: BuildingId(4),
                vault: true
            })
            .len(),
            1
        );
        let mut del = fetch(3);
        del.kind = Kind::Deliver {
            to: PersonId(40),
            item: ItemId::new("sealed_package"),
        };
        let d = jobs.push(del);
        jobs.take(d, 0);
        assert_eq!(jobs.on(Event::Handed { job: d }).len(), 1);
        assert!(jobs.on(Event::Handed { job: d }).is_empty());
        let offered = jobs.push(fetch(5));
        assert!(
            jobs.on(Event::Taken {
                place: flat(),
                pos: Pos::new(5, 5),
            })
            .is_empty(),
            "an Offered job never ticks: {offered:?}"
        );
    }

    #[test]
    fn minute_fails_a_late_job_once() {
        let mut jobs = Jobs::default();
        let id = jobs.push(fetch(1));
        jobs.take(id, 0);
        let end = HOURS_FETCH * 60;
        assert!(jobs.minute(end).is_empty(), "the last minute still counts");
        assert_eq!(Jobs::left(jobs.get(id).unwrap(), end), Some(0));
        assert_eq!(jobs.minute(end + 1).len(), 1);
        assert_eq!(jobs.get(id).unwrap().state, State::Failed);
        assert_eq!(jobs.standing(PersonId(1)), LOSS_LATE);
        assert!(jobs.minute(end + 1).is_empty());
        assert!(jobs.minute(end + 500).is_empty());
    }

    #[test]
    fn clock_words() {
        assert_eq!(clock_text(192), "3h 12m");
        assert_eq!(clock_text(60), "1h 0m");
        assert_eq!(clock_text(48), "48m");
        assert_eq!(clock_text(0), "late");
    }

    #[test]
    fn hand_in_items_by_kind() {
        assert_eq!(
            handin_item(&Kind::Copy {
                building: BuildingId(1),
                vault: true
            }),
            Some(ItemId::new("vault_data"))
        );
        assert_eq!(
            handin_item(&Kind::Copy {
                building: BuildingId(1),
                vault: false
            }),
            Some(ItemId::new("paydata"))
        );
        assert_eq!(
            handin_item(&Kind::Plant {
                building: BuildingId(1)
            }),
            None
        );
    }

    use super::testutil::{a_fixer, state_home};

    #[test]
    fn taking_the_welcome_shard_ticks_the_job() {
        let mut st = state_home();
        let (b, level, _, tile) = crate::jobs::first_fetch(&st).unwrap();
        let id = st.jobs.active().unwrap().id;
        // Stand on the shard and take it the way the Space window does.
        let city = st.city.clone().unwrap();
        st.interiors.ensure(&city, b, level);
        let mut p = st
            .player
            .take()
            .unwrap_or_else(|| crate::walker::Player::spawn(&city, 1));
        p.place = Place::Floor { building: b, level };
        p.pos = tile;
        st.player = Some(p);
        crate::actions::perform(&mut st, game::Action::TakeAll).unwrap();
        assert!(st.jobs.get(id).unwrap().objectives[0].done);
        assert_eq!(st.character.count(&ItemId::new("paydata")), 1);
        assert!(
            st.log
                .latest()
                .is_some_and(|e| e.text.contains("Take the shard")),
            "{:?}",
            st.log.latest().map(|e| e.text.clone())
        );
    }

    #[test]
    fn a_server_copy_and_a_backdoor_tick_their_jobs() {
        let mut st = state_home();
        let fx = a_fixer(&st);
        st.jobs.fixer_mut(fx).standing = PLANT_AT;
        let copy = crate::jobs::generate(&mut st, fx, KindTag::Copy, 0).unwrap();
        let Kind::Copy { building: cb, .. } = copy.kind else {
            panic!()
        };
        let cid = st.jobs.push(copy);
        st.jobs.take(cid, 0);
        let plant = crate::jobs::generate(&mut st, fx, KindTag::Plant, 1).unwrap();
        let Kind::Plant { building: pb } = plant.kind else {
            panic!()
        };
        let pid = st.jobs.push(plant);
        st.jobs.take(pid, 0);
        crate::jobs::send(
            &mut st,
            Event::Copied {
                building: cb,
                vault: false,
            },
        );
        assert!(st.jobs.get(cid).unwrap().objectives[0].done);
        let seed = st.city.as_ref().unwrap().seed;
        st.backdoors.plant_net(pb, 1, seed, 0);
        crate::jobs::send(&mut st, Event::Planted { building: pb });
        assert!(st.jobs.get(pid).unwrap().objectives[0].done);
        let stands = |b: BuildingId| st.backdoors.standing().any(|d| d.building == b);
        assert_eq!(st.jobs.ready(pid, &stands, &|_| true), Ok(()));
        st.backdoors.burn(pb, crate::backdoors::Why::You, 1);
        let stands = |b: BuildingId| st.backdoors.standing().any(|d| d.building == b);
        assert!(st.jobs.ready(pid, &stands, &|_| true).is_err());
    }

    #[test]
    fn a_deadline_passing_mails_once() {
        let mut st = state_home();
        let id = st.jobs.active().unwrap().id;
        let deadline = st.jobs.get(id).unwrap().deadline.unwrap();
        let mails = st.journal.mails.len();
        // Jump the world's clock past the deadline in one step, as a long menu would.
        let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) else {
            panic!()
        };
        world.advance(city, (deadline + 5) * sim::TICKS_PER_MINUTE);
        crate::jobs::tick(&mut st);
        crate::jobs::tick(&mut st);
        assert_eq!(st.jobs.get(id).unwrap().state, State::Failed);
        assert_eq!(st.journal.mails.len(), mails + 1);
        let last = st.journal.mails.last().unwrap();
        assert!(
            last.body[0].contains("window") || last.body[0].contains("Late"),
            "{:?}",
            last.body
        );
        assert_eq!(st.jobs.standing(st.jobs.get(id).unwrap().fixer), LOSS_LATE);
        assert!(st.log.latest().is_some_and(|e| e.text.contains("too late")));
        assert_eq!(last.job, Some(id), "the late mail names its job");
        assert_eq!(
            st.jobs.get(id).unwrap().closed.map(|c| c.standing),
            Some(LOSS_LATE)
        );
    }

    #[test]
    fn console_rolls_a_job_and_sets_the_standing() {
        let mut st = state_home();
        let at = game::At {
            place: Place::Stratum(StratumKind::Street),
            pos: Pos::new(0, 0),
        };
        let out = crate::screens::console::run(&mut st, at, "job deliver").unwrap();
        assert!(out.contains("deliver"), "{out}");
        assert_eq!(st.character.count(&ItemId::new("sealed_package")), 1);
        assert!(
            st.jobs
                .list
                .iter()
                .any(|j| matches!(j.kind, Kind::Deliver { .. }) && j.state == State::Taken)
        );
        assert!(
            st.journal
                .mails
                .last()
                .unwrap()
                .body
                .iter()
                .any(|l| l.starts_with("By "))
        );
        let out = crate::screens::console::run(&mut st, at, "standing 4").unwrap();
        assert!(out.contains('4'), "{out}");
        assert_eq!(st.jobs.standing(a_fixer(&st)), 4);
        let out = crate::screens::console::run(&mut st, at, "jobs").unwrap();
        assert!(out.contains("A favour"), "{out}");
    }

    #[test]
    fn the_mail_says_the_deadline_on_the_clock() {
        let mut st = state_home();
        let fx = a_fixer(&st);
        let welcome = st.jobs.active().unwrap().id;
        st.jobs.drop_job(welcome, 0);
        st.jobs.fixer_mut(fx).standing = 0;
        let id = crate::jobs::offer(&mut st, fx).unwrap();
        let hours = st.jobs.get(id).unwrap().hours;
        let (h, m) = st.clock();
        let by = (h as u64 * 60 + m as u64 + hours * 60) % (24 * 60);
        crate::jobs::take_job(&mut st, id).unwrap();
        let line = format!("By {:02}:{:02}.", by / 60, by % 60);
        let body = &st.journal.mails.last().unwrap().body;
        assert!(body.contains(&line), "wanted {line:?} in {body:?}");
        assert_eq!(st.journal.mails.last().unwrap().job, Some(id));
    }

    #[test]
    fn standing_words() {
        let words: Vec<&str> = [-3, -1, 0, 1, 2, 5, 6, 9]
            .into_iter()
            .map(standing_word)
            .collect();
        assert_eq!(
            words,
            [
                "refuses you",
                "cold",
                "new",
                "new",
                "known",
                "trusted",
                "inner circle",
                "inner circle"
            ]
        );
    }

    #[test]
    fn opens_at_matches_allowed() {
        for n in -4..=8 {
            for k in [
                KindTag::Fetch,
                KindTag::Deliver,
                KindTag::Copy,
                KindTag::Plant,
            ] {
                assert_eq!(
                    opens_at(k, false) <= n,
                    Jobs::allowed(n).contains(&k),
                    "{k:?} at {n}"
                );
            }
        }
        assert_eq!(opens_at(KindTag::Copy, true), VAULT_AT);
    }

    #[test]
    fn in_shift_wraps() {
        let night = (18 * 60, 4 * 60);
        for t in [1080, 1439, 0, 239] {
            assert!(in_shift(t, night), "{t}");
        }
        for t in [240, 600, 1079] {
            assert!(!in_shift(t, night), "{t}");
        }
        assert!(in_shift(540, (540, 1200)));
        assert!(!in_shift(1200, (540, 1200)));
    }

    /// The first game minute from `base` on whose clock reads `hm`.
    fn at(base: u64, hm: u16) -> u64 {
        (base..base + 24 * 60)
            .find(|m| day_minute(*m) == hm)
            .unwrap()
    }

    #[test]
    fn corner_at_the_deadline() {
        let night = (18 * 60, 4 * 60);
        let taken = at(0, 22 * 60);
        let due = taken + 12 * 60;
        assert_eq!(day_minute(due), 10 * 60);
        assert_eq!(
            corner(taken + 200, due, night),
            Corner::Leaves(at(taken, 4 * 60))
        );
        assert_eq!(corner(taken, at(taken, 3 * 60), night), Corner::Through);
        assert_eq!(
            corner(at(taken, 5 * 60), at(taken, 10 * 60), night),
            Corner::Off
        );
    }

    #[test]
    fn shift_turn_counts_to_the_next_change() {
        let night = (18 * 60, 4 * 60);
        assert_eq!(shift_turn(at(0, 23 * 60 + 30), night), 270);
        assert_eq!(shift_turn(at(0, 5 * 60), night), 13 * 60);
    }

    #[test]
    fn closing_stamps_the_minute_and_the_standing() {
        let mut jobs = Jobs::default();
        let done = jobs.push(fetch(1));
        jobs.take(done, 0);
        assert_eq!(jobs.hand_in(done, 500), Some(PAY_FETCH));
        assert_eq!(
            jobs.get(done).unwrap().closed,
            Some(Closed {
                at: 500,
                standing: GAIN_DONE
            })
        );
        let dropped = jobs.push(fetch(2));
        jobs.take(dropped, 0);
        assert!(jobs.drop_job(dropped, 600));
        assert_eq!(
            jobs.get(dropped).unwrap().closed,
            Some(Closed {
                at: 600,
                standing: LOSS_DROP
            })
        );
        let late = jobs.push(fetch(3));
        jobs.take(late, 0);
        let after = HOURS_FETCH * 60 + 1;
        assert_eq!(jobs.minute(after), vec![Late { job: late }]);
        assert_eq!(
            jobs.get(late).unwrap().closed,
            Some(Closed {
                at: after,
                standing: LOSS_LATE
            })
        );
        assert_eq!(jobs.get(done).unwrap().closed.unwrap().at, 500, "kept");
    }

    #[test]
    fn an_unplanted_backdoor_says_so() {
        let mut jobs = Jobs::default();
        let mut plant = fetch(1);
        plant.kind = Kind::Plant {
            building: BuildingId(3),
        };
        let id = jobs.push(plant);
        jobs.take(id, 0);
        assert_eq!(
            jobs.ready(id, &|_| false, &|_| true),
            Err("the backdoor is not planted yet".to_string())
        );
    }

    #[test]
    fn work_done_before_taking_counts() {
        let mut st = state_home();
        let fx = a_fixer(&st);
        st.jobs.fixer_mut(fx).standing = PLANT_AT;
        // The shard fetched while the job was only on offer.
        let fetch = crate::jobs::generate(&mut st, fx, KindTag::Fetch, 0).unwrap();
        let Kind::Fetch {
            building,
            level,
            tile,
            ..
        } = fetch.kind
        else {
            panic!()
        };
        let f = st.jobs.push(fetch);
        st.ground.take(
            Place::Floor { building, level },
            tile,
            &ItemId::new("paydata"),
            1,
        );
        crate::jobs::take_job(&mut st, f).unwrap();
        assert!(
            st.jobs.get(f).unwrap().objectives[0].done,
            "the shard already taken counts"
        );
        // The backdoor planted while the job was only on offer.
        let plant = crate::jobs::generate(&mut st, fx, KindTag::Plant, 1).unwrap();
        let Kind::Plant { building: pb } = plant.kind else {
            panic!()
        };
        let p = st.jobs.push(plant);
        let seed = st.city.as_ref().unwrap().seed;
        st.backdoors.plant_net(pb, 1, seed, 0);
        st.jobs.drop_job(f, 0);
        crate::jobs::take_job(&mut st, p).unwrap();
        assert!(
            st.jobs.get(p).unwrap().objectives[0].done,
            "the standing backdoor counts"
        );
    }
}
