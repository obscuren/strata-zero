//! Jobs on the rig: HELIX programs that work on game time while you're out. `sniff`
//! reads door codes off a Net backdoor, `sift` digs through a paydata shard, `keepalive`
//! holds a backdoor's life still and `rent` sells the spare cycles.

use crate::app::AppState;
use citygen::BuildingId;
use game::ItemId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobKind {
    Sniff(BuildingId),
    Sift { shard: ItemId },
    Keepalive(BuildingId),
    Rent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub n: u32,
    pub kind: JobKind,
    pub started: u64,
    pub last: u64,
    pub progress: u64,
    pub found: Vec<String>,
    /// Rent: what it has paid since the report was last emptied.
    #[serde(default)]
    pub paid: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RigJobs {
    pub running: Vec<Job>,
    pub heat: BTreeMap<BuildingId, u32>,
    pub report: Vec<String>,
    /// How much of the report `ps` has shown: the lines past it are new to you. The
    /// morning mail empties the report and sets it back to 0.
    pub seen: usize,
    pub programs: BTreeSet<String>,
    pub worked_since_boot: u32,
    pub warned: BTreeSet<BuildingId>,
    pub next: u32,
    pub mailed_day: u64,
    pub stashes: Vec<Stash>,
    counted: BTreeSet<u32>,
    cooled: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stash {
    pub building: BuildingId,
    pub level: i32,
    pub number: u16,
    pub tile: citygen::Pos,
    pub item: ItemId,
}

impl Default for RigJobs {
    fn default() -> Self {
        RigJobs {
            running: Vec::new(),
            heat: BTreeMap::new(),
            report: Vec::new(),
            seen: 0,
            programs: ["sift", "rent"].iter().map(|s| s.to_string()).collect(),
            worked_since_boot: 0,
            warned: BTreeSet::new(),
            next: 0,
            mailed_day: 0,
            stashes: Vec::new(),
            counted: BTreeSet::new(),
            cooled: 0,
        }
    }
}

/// Hours of work a job does, at the rig's speed ×1.
pub const SNIFF_HOURS: u64 = 3;
pub const SIFT_HOURS: u64 = 6;
pub const SIFT_VAULT_HOURS: u64 = 10;
/// Credits an hour of rent pays.
pub const RENT_PER_HOUR: u32 = 6;
/// Noise a clock hour on the building, in quarter points, however fast the rig.
pub const SNIFF_NOISE: u32 = 4;
pub const KEEPALIVE_NOISE: u32 = 1;
/// The heat that gets a mention in the morning, and the heat that gets the hole patched.
pub const HEAT_WARN: u32 = 18 * 4;
pub const HEAT_PATCH: u32 = 36 * 4;
/// What a building's heat falls an hour while no job is on it.
pub const COOL: u32 = 4;

const WARNING: &str = "their auth.log is getting long. somebody reads those.";

/// What a sifted shard can leave lying in a flat.
const STASHES: [&str; 6] = [
    "ratchet_receiver",
    "ratchet_barrel",
    "ratchet_trigger",
    "scrappers_roll",
    "burner_schematic",
    "scrap_servo",
];

impl JobKind {
    /// The program's name, as `run` takes it.
    pub fn name(&self) -> &'static str {
        match self {
            JobKind::Sniff(_) => "sniff",
            JobKind::Sift { .. } => "sift",
            JobKind::Keepalive(_) => "keepalive",
            JobKind::Rent => "rent",
        }
    }

    /// The backdoor's building a job works on.
    pub fn line(&self) -> Option<BuildingId> {
        match self {
            JobKind::Sniff(b) | JobKind::Keepalive(b) => Some(*b),
            _ => None,
        }
    }
}

impl RigJobs {
    /// The boot says how many jobs did work since the last boot, and starts counting
    /// again.
    pub fn boot(&mut self) -> u32 {
        let n = self.worked_since_boot;
        self.worked_since_boot = 0;
        self.counted.clear();
        n
    }
}

/// What a job works on, as `ps` and the report name it: the host, the shard, the
/// spare cycles.
pub fn target(st: &AppState, kind: &JobKind) -> String {
    match kind {
        JobKind::Sniff(b) | JobKind::Keepalive(b) => crate::helix::hosts::building_hostname(st, *b),
        JobKind::Sift { shard } => st
            .content
            .item(shard)
            .map_or_else(|| shard.0.clone(), |i| i.name.to_lowercase()),
        JobKind::Rent => "spare cycles".to_string(),
    }
}

/// `sniff · hollis-court`: how the report heads a job's lines.
pub fn head(st: &AppState, kind: &JobKind) -> String {
    format!("{} · {}", kind.name(), target(st, kind))
}

fn seed(st: &AppState) -> u64 {
    st.city.as_ref().map_or(0, |c| c.seed)
}

/// A pick seeded from the run, the job and its hour.
fn roll(seed: u64, salt: u64, n: u32, hour: u64) -> u64 {
    citygen::rng::hash64(seed ^ salt, ((n as u64) << 32) | hour)
}

fn stash_roll(seed: u64, n: u32, hour: u64) -> bool {
    roll(seed, 0x57A5, n, hour).is_multiple_of(3)
}

/// Game minutes in one of a job's hours: a keepalive holds by the clock, the rest
/// work faster on a faster rig.
fn hour_of(kind: &JobKind, speed: u16) -> u64 {
    match kind {
        JobKind::Keepalive(_) => 60,
        _ => (6000 / speed.max(1) as u64).max(1),
    }
}

fn sift_hours(shard: &ItemId) -> u64 {
    if shard.0 == "vault_data" {
        SIFT_VAULT_HOURS
    } else {
        SIFT_HOURS
    }
}

/// A sift's work at `now` on a rig of `speed`: the game minutes it has left and the
/// minutes it takes in all. None for any other job.
pub fn sift_left(job: &Job, speed: u16, now: u64) -> Option<(u64, u64)> {
    let JobKind::Sift { shard } = &job.kind else {
        return None;
    };
    let hour = hour_of(&job.kind, speed);
    let all = sift_hours(shard) * hour;
    let done = job.progress * hour + now.saturating_sub(job.last).min(hour);
    Some((all.saturating_sub(done), all))
}

/// Starts a job in a free seat; its number.
pub fn start(st: &mut AppState, kind: JobKind) -> Result<u32, String> {
    let now = st.net.now(st);
    advance(st, now);
    let what = kind.name();
    if st.backdoors.seized(now) {
        return Err("run: the rig is seized".to_string());
    }
    if !st.backdoors.jobs.programs.contains(what) {
        return Err("run: no such job · ls /opt/jobs".to_string());
    }
    match &kind {
        JobKind::Sniff(b) | JobKind::Keepalive(b) => {
            let host = target(st, &kind);
            let Some(d) = st.backdoors.held(*b) else {
                return Err(format!("{what}: no backdoor on {host}"));
            };
            if matches!(d.kind, crate::backdoors::Kind::Key { .. }) {
                return Err(format!("{what}: a key won't hold that long"));
            }
            if st.backdoors.jobs.running.iter().any(|j| j.kind == kind) {
                return Err(format!("{what}: already on {host}"));
            }
        }
        JobKind::Sift { shard } => {
            if shard.0.ends_with("_sifted") {
                return Err("sift: already sifted".to_string());
            }
            if !matches!(shard.0.as_str(), "paydata" | "vault_data") {
                return Err("sift: only a paydata shard or a vault dump".to_string());
            }
            if st.character.count(shard) == 0 {
                let name = st.content.item(shard).map_or("shard", |i| i.name.as_str());
                return Err(format!("sift: no {name} in your pack"));
            }
        }
        JobKind::Rent => {}
    }
    let seats = crate::rig::of(st).seats as usize;
    if seats == 0 {
        return Err("run: no core in the rig".to_string());
    }
    if st.backdoors.jobs.running.len() >= seats {
        return Err("run: every seat is busy · kill a job, or fit a bigger core".to_string());
    }
    if let JobKind::Sift { shard } = &kind {
        game::remove_from_pack(&mut st.character, shard, 1);
    }
    let jobs = &mut st.backdoors.jobs;
    if jobs.running.is_empty() && jobs.heat.is_empty() {
        jobs.cooled = now - now % 60;
    }
    jobs.next += 1;
    let n = jobs.next;
    jobs.running.push(Job {
        n,
        kind,
        started: now,
        last: now,
        progress: 0,
        found: Vec::new(),
        paid: 0,
    });
    Ok(n)
}

/// Stops job `n`: `killed 2 · sift · paydata shard`. A shard being sifted comes back
/// as it went in.
pub fn kill(st: &mut AppState, n: u32) -> Result<String, String> {
    let now = st.net.now(st);
    advance(st, now);
    let mut jobs = std::mem::take(&mut st.backdoors.jobs);
    let out = match jobs.running.iter().position(|j| j.n == n) {
        None => Err(format!("kill: no job {n}")),
        Some(i) => {
            let job = stop(st, &mut jobs, i, now, "killed", true);
            let mut line = format!("killed {n} · {}", head(st, &job.kind));
            if matches!(job.kind, JobKind::Sift { .. }) {
                line.push_str(" · the shard is back in your pack");
            }
            Ok(line)
        }
    };
    st.backdoors.jobs = jobs;
    out
}

/// Kills the newest jobs over the rig's seats; a line for each.
pub fn trim_to_seats(st: &mut AppState) -> Vec<String> {
    let seats = crate::rig::of(st).seats as usize;
    let now = st.net.now(st);
    advance(st, now);
    let mut jobs = std::mem::take(&mut st.backdoors.jobs);
    let mut out = Vec::new();
    while jobs.running.len() > seats {
        let Some(i) = (0..jobs.running.len()).max_by_key(|i| jobs.running[*i].n) else {
            break;
        };
        let job = stop(st, &mut jobs, i, now, "killed · no seat for it", true);
        out.push(format!(
            "job {} killed · {} · no seat for it",
            job.n,
            head(st, &job.kind)
        ));
    }
    st.backdoors.jobs = jobs;
    out
}

/// Every job ends at once, saying `why` (a seized rig).
pub fn kill_all(st: &mut AppState, why: &str) {
    let now = st.net.now(st);
    kill_where(st, why, now, |_| true);
}

/// The raid at minute `at`: the jobs work up to it, then a seized rig loses every job
/// and a rig that wiped itself the jobs on its lines (its backdoors all burned).
pub fn raid(st: &mut AppState, at: u64, wiped: bool) {
    if wiped {
        kill_where(st, "the rig wiped itself", at, |k| k.line().is_some());
    } else {
        kill_where(st, "the rig was seized", at, |_| true);
    }
}

/// The jobs `which` picks work up to `now`, then end, saying `why`.
fn kill_where(st: &mut AppState, why: &str, now: u64, which: impl Fn(&JobKind) -> bool) {
    advance(st, now);
    if st.backdoors.jobs.running.is_empty() {
        return;
    }
    let mut jobs = std::mem::take(&mut st.backdoors.jobs);
    let mut i = 0;
    while i < jobs.running.len() {
        if which(&jobs.running[i].kind) {
            stop(st, &mut jobs, i, now, why, true);
        } else {
            i += 1;
        }
    }
    st.backdoors.jobs = jobs;
}

/// Takes job `i` off the rig at `now`, its last line `line` in the report: a keepalive
/// lets its backdoor's life run again; with `give_back`, a shard comes back unsifted.
fn stop(
    st: &mut AppState,
    jobs: &mut RigJobs,
    i: usize,
    now: u64,
    line: &str,
    give_back: bool,
) -> Job {
    let job = jobs.running.remove(i);
    match &job.kind {
        JobKind::Keepalive(b) => st.backdoors.hold_still(*b, now.saturating_sub(job.last)),
        JobKind::Sift { shard } if give_back => {
            st.character.give_item(&st.content, shard.clone(), 1);
        }
        _ => {}
    }
    jobs.report
        .push(format!("{} · {line}", head(st, &job.kind)));
    job
}

/// Every job on the backdoor on `b` ends, saying `line`.
fn end_on(st: &mut AppState, jobs: &mut RigJobs, b: BuildingId, now: u64, line: &str) {
    let mut i = 0;
    while i < jobs.running.len() {
        if jobs.running[i].kind.line() == Some(b) {
            stop(st, jobs, i, now, line, false);
        } else {
            i += 1;
        }
    }
}

/// The jobs work up to `now`, an hour at a time from where each was last advanced, and
/// on the clock's hours the buildings with jobs on them get noisier and the rest cool. Every hour is played in
/// order whatever the gaps between calls, so a long catch-up comes out as ticking
/// through does.
pub fn advance(st: &mut AppState, now: u64) {
    let jobs = &mut st.backdoors.jobs;
    if jobs.running.is_empty() && jobs.heat.is_empty() {
        jobs.cooled = jobs.cooled.max(now - now % 60);
        let m = morning_of(now);
        if m > jobs.mailed_day {
            let mut jobs = std::mem::take(&mut st.backdoors.jobs);
            morning_mail(st, &mut jobs, m);
            st.backdoors.jobs = jobs;
        }
        return;
    }
    let speed = crate::rig::of(st).speed;
    let mut jobs = std::mem::take(&mut st.backdoors.jobs);
    loop {
        let next = (jobs.running.iter().enumerate())
            .map(|(i, j)| (j.last + hour_of(&j.kind, speed), j.n, i))
            .min();
        let cool = jobs.cooled + 60;
        // the next 06:00; what happens at that minute is in the mail
        let morning = jobs.mailed_day + 1;
        let six = morning_at(morning);
        match next {
            Some((t, _, i)) if t <= now && t <= cool && t <= six => work(st, &mut jobs, i, t),
            _ if cool <= now && cool <= six => clock_hour(st, &mut jobs, cool),
            _ if six <= now => morning_mail(st, &mut jobs, morning),
            _ => break,
        }
    }
    st.backdoors.jobs = jobs;
}

/// How long a job has been up, in game minutes: `7h 12m`, `3d 4h`.
pub fn up_label(minutes: u64) -> String {
    match (minutes / 1440, minutes / 60 % 24, minutes % 60) {
        (0, 0, m) => format!("{m}m"),
        (0, h, 0) => format!("{h}h"),
        (0, h, m) => format!("{h}h {m}m"),
        (d, h, _) => format!("{d}d {h}h"),
    }
}

/// The 06:00 that `now` is on or past, counted from the first day's; 0 before the
/// first.
fn morning_of(now: u64) -> u64 {
    let since = crate::ui::hud::START_MINUTE_OF_DAY as u64 + now;
    since.saturating_sub(6 * 60) / (24 * 60)
}

/// The game minute of the `morning`th 06:00.
fn morning_at(morning: u64) -> u64 {
    morning * 24 * 60 + 6 * 60 - crate::ui::hud::START_MINUTE_OF_DAY as u64
}

/// The `morning`th 06:00: the report, if it has lines, goes to the journal as a mail
/// from cron, grouped by job, and is emptied. The jobs are advanced in order up to the
/// minute, so a rig left for days mails each morning's own lines as ticking through would.
fn morning_mail(st: &mut AppState, jobs: &mut RigJobs, morning: u64) {
    jobs.mailed_day = morning;
    if jobs.report.is_empty() {
        return;
    }
    let at = morning_at(morning);
    let groups = grouped(&jobs.report);
    let mut body = Vec::new();
    for (head, lines) in &groups {
        if !body.is_empty() {
            body.push(String::new());
        }
        let running = jobs
            .running
            .iter()
            .find(|j| self::head(st, &j.kind) == *head);
        body.push(match running {
            Some(j) => format!(
                "{head} · {}, still running",
                up_label(at.saturating_sub(j.started))
            ),
            None => head.clone(),
        });
        body.extend(
            lines
                .iter()
                .filter(|l| !l.is_empty())
                .map(|l| format!("└ {l}")),
        );
    }
    let n = groups.len();
    st.journal.mails.push(crate::credentials::Mail {
        from: "cron".to_string(),
        subject: format!("overnight · {n} {}", if n == 1 { "job" } else { "jobs" }),
        body,
        read: false,
        at,
        job: None,
        story: None,
    });
    jobs.report.clear();
    jobs.seen = 0;
}

/// Report lines read `kind · target · what`; a job's lines go together, under the
/// job's `kind · target`.
fn grouped(lines: &[String]) -> Vec<(String, Vec<String>)> {
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for line in lines {
        let mut parts = line.splitn(3, " · ");
        let head = match (parts.next(), parts.next()) {
            (Some(a), Some(b)) => format!("{a} · {b}"),
            (Some(a), None) => a.to_string(),
            _ => continue,
        };
        let rest = parts.next().unwrap_or("").to_string();
        match groups.iter_mut().find(|(h, _)| *h == head) {
            Some((_, lines)) => lines.push(rest),
            None => groups.push((head, vec![rest])),
        }
    }
    groups
}

/// The report's lines `ps` hasn't shown yet, a job's together as the mail has them
/// (`sift · paydata shard`, then `└ code 4471 · …`); they count as seen now.
pub fn unseen(jobs: &mut RigJobs) -> Vec<String> {
    let from = jobs.seen.min(jobs.report.len());
    jobs.seen = jobs.report.len();
    let mut out = Vec::new();
    for (head, lines) in grouped(&jobs.report[from..]) {
        out.push(head);
        out.extend(
            lines
                .iter()
                .filter(|l| !l.is_empty())
                .map(|l| format!("└ {l}")),
        );
    }
    out
}

/// An hour of the clock at `at`: each building with jobs on it takes their noise (the
/// warning at 18, the hole patched at 36), and the rest cool.
fn clock_hour(st: &mut AppState, jobs: &mut RigJobs, at: u64) {
    use crate::backdoors::Why;
    jobs.cooled = at;
    let mut noise: BTreeMap<BuildingId, u32> = BTreeMap::new();
    for j in &jobs.running {
        match j.kind {
            JobKind::Sniff(b) => *noise.entry(b).or_default() += SNIFF_NOISE,
            JobKind::Keepalive(b) => *noise.entry(b).or_default() += KEEPALIVE_NOISE,
            _ => {}
        }
    }
    for (b, h) in jobs.heat.iter_mut() {
        if !noise.contains_key(b) {
            *h = h.saturating_sub(COOL);
        }
    }
    jobs.heat.retain(|_, h| *h > 0);
    let heat = &jobs.heat;
    jobs.warned.retain(|b| heat.contains_key(b));
    for (b, n) in noise {
        let heat = jobs.heat.entry(b).or_insert(0);
        *heat += n;
        let heat = *heat;
        let Some(kind) = (jobs.running.iter())
            .find(|j| j.kind.line() == Some(b))
            .map(|j| j.kind.clone())
        else {
            continue;
        };
        if heat >= HEAT_WARN && jobs.warned.insert(b) {
            jobs.report.push(format!("{} · {WARNING}", head(st, &kind)));
        }
        if heat >= HEAT_PATCH {
            st.backdoors.burn(b, Why::Noticed, at);
            jobs.heat.remove(&b);
            jobs.warned.remove(&b);
            end_on(st, jobs, b, at, "the admins found it · the hole is patched");
        }
    }
}

/// Job `i` works its next hour, which ends at `t`.
fn work(st: &mut AppState, jobs: &mut RigJobs, i: usize, t: u64) {
    let job = &mut jobs.running[i];
    let from = job.last;
    job.last = t;
    job.progress += 1;
    let (n, kind, hour) = (job.n, job.kind.clone(), job.progress);
    if jobs.counted.insert(n) {
        jobs.worked_since_boot += 1;
    }
    match kind {
        JobKind::Rent => pay(st, jobs, i),
        JobKind::Sift { shard } => {
            if hour >= sift_hours(&shard) {
                sifted(st, jobs, i, t, &shard);
            }
        }
        JobKind::Sniff(b) | JobKind::Keepalive(b) => on_line(st, jobs, i, b, t, t - from),
    }
}

/// An hour of rent: the credits paid, and the job's line in the report says the total
/// since the report was last emptied.
fn pay(st: &mut AppState, jobs: &mut RigJobs, i: usize) {
    let c = &mut st.character.credits;
    *c = c.saturating_add(RENT_PER_HOUR);
    let head = head(st, &jobs.running[i].kind);
    let job = &mut jobs.running[i];
    let line = |paid: u32| format!("{head} · ¤{paid} paid");
    let was = line(job.paid);
    match jobs.report.iter_mut().find(|l| **l == was) {
        Some(l) => {
            job.paid += RENT_PER_HOUR;
            *l = line(job.paid);
        }
        None => {
            job.paid = RENT_PER_HOUR;
            jobs.report.push(line(job.paid));
        }
    }
}

/// An hour of a job on the backdoor on `b`, `minutes` long: a keepalive's hold, the
/// line checked, and a sniff's find.
fn on_line(st: &mut AppState, jobs: &mut RigJobs, i: usize, b: BuildingId, t: u64, minutes: u64) {
    use crate::backdoors::{State, Why};
    let Some(state) = st.backdoors.held(b).map(|d| d.state) else {
        end_on(st, jobs, b, t, "the backdoor is gone");
        return;
    };
    if state == State::Honeypot {
        stop(st, jobs, i, t, "nothing on the line but bait", false);
        return;
    }
    let kind = jobs.running[i].kind.clone();
    let n = jobs.running[i].n;
    let sniff = matches!(kind, JobKind::Sniff(_));
    // A keepalive holds its own line first: the life it had at its start minute.
    if !sniff {
        st.backdoors.hold_still(b, minutes);
    }
    let end = st.backdoors.held(b).map_or(0, |d| d.real_end());
    let held = (jobs.running.iter()).any(|j| j.n != n && j.kind == JobKind::Keepalive(b));
    if !held && t >= end {
        st.backdoors.burn(b, Why::Decay, end);
        end_on(st, jobs, b, t, "the line went dead");
        return;
    }
    if !sniff {
        return;
    }
    if unknown_flats(st, b).is_empty() {
        stop(st, jobs, i, t, "nothing left to find", false);
        return;
    }
    let hour = jobs.running[i].progress;
    if !hour.is_multiple_of(SNIFF_HOURS) {
        return;
    }
    let flats = unknown_flats(st, b);
    let seed = seed(st);
    let (level, number) = flats[(roll(seed, 0x5_1FF, n, hour) % flats.len() as u64) as usize];
    let code = learn(st, b, level, number, crate::credentials::Source::Sniffed, t);
    jobs.running[i].found.push(format!("code {code:04}"));
    jobs.report.push(format!(
        "{} · code {code:04} · {number}, {} · in your journal",
        head(st, &kind),
        citygen::FloorPlan::label(level)
    ));
    if flats.len() == 1 {
        stop(st, jobs, i, t, "nothing left to find", false);
    }
}

/// The flats of `b` whose codes you don't know, your own left out.
fn unknown_flats(st: &mut AppState, b: BuildingId) -> Vec<(i32, u16)> {
    crate::credentials::prepare(st, b);
    let own = st
        .home
        .map(|h| (h.home.building, h.home.level, h.home.number));
    let Some(homes) = st.interiors.homes_of(b) else {
        return Vec::new();
    };
    homes
        .all_flats()
        .filter(|&(l, n)| own != Some((b, l, n)) && st.journal.code_for(b, l, n).is_none())
        .collect()
}

/// A flat's code into the journal; the code.
fn learn(
    st: &mut AppState,
    b: BuildingId,
    level: i32,
    number: u16,
    source: crate::credentials::Source,
    at: u64,
) -> u16 {
    let code = citygen::rng::door_code(seed(st), b, level, number);
    let household = (st.interiors.homes_of(b)).and_then(|h| h.household_of(level, number));
    st.journal.learn(crate::credentials::Code {
        building: b,
        level,
        number,
        household,
        code,
        source,
        noted_by: None,
        at,
    });
    code
}

/// A sift's last hour: the codes it found, a stash one time in three, and the shard
/// back in the pack, sifted.
fn sifted(st: &mut AppState, jobs: &mut RigJobs, i: usize, t: u64, shard: &ItemId) {
    let (n, hour, kind) = {
        let j = &jobs.running[i];
        (j.n, j.progress, j.kind.clone())
    };
    let seed = seed(st);
    let want = if shard.0 == "vault_data" { 4 } else { 2 };
    let head = head(st, &kind);
    match lived_in_flats(st, seed, n, hour, want) {
        None => jobs
            .report
            .push(format!("{head} · nothing in it worth a door")),
        Some((b, flats)) => {
            let building = crate::net::building_name(st, b);
            for &(level, number) in &flats {
                let code = learn(st, b, level, number, crate::credentials::Source::Sifted, t);
                jobs.running[i].found.push(format!("code {code:04}"));
                jobs.report.push(format!(
                    "{head} · code {code:04} · {number}, {} of {building} · in your journal",
                    citygen::FloorPlan::label(level)
                ));
            }
            if stash_roll(seed, n, hour) {
                let item = ItemId::new(STASHES[(roll(seed, 0x17E4, n, hour) % 6) as usize]);
                let first = (roll(seed, 0xF1A7, n, hour) % flats.len() as u64) as usize;
                let laid = (0..flats.len())
                    .map(|k| flats[(first + k) % flats.len()])
                    .find_map(|(level, number)| {
                        crate::jobs::generate::lay_in_flat(st, b, level, number, &item)
                            .map(|tile| (level, number, tile))
                    });
                if let Some((level, number, tile)) = laid {
                    let name = st
                        .content
                        .item(&item)
                        .map_or("something", |x| x.name.as_str());
                    jobs.running[i].found.push(format!("stash {number}"));
                    jobs.report.push(format!(
                        "{head} · stash · {} in {number}, {} of {building} · on your map",
                        name.to_lowercase(),
                        citygen::FloorPlan::label(level)
                    ));
                    jobs.stashes.push(Stash {
                        building: b,
                        level,
                        number,
                        tile,
                        item,
                    });
                }
            }
        }
    }
    let back = ItemId::new(&format!("{}_sifted", shard.0));
    st.character.give_item(&st.content, back, 1);
    stop(
        st,
        jobs,
        i,
        t,
        "done · the shard is back in your pack, sifted",
        false,
    );
}

/// Up to `want` lived-in flats whose codes you don't know, in one block the seed picks
/// (the next with any when it has none).
fn lived_in_flats(
    st: &mut AppState,
    seed: u64,
    n: u32,
    hour: u64,
    want: usize,
) -> Option<(BuildingId, Vec<(i32, u16)>)> {
    use citygen::BuildingKind;
    let blocks: Vec<BuildingId> = (st.city.as_ref()?.buildings.iter())
        .filter(|b| matches!(b.kind, BuildingKind::Apartment | BuildingKind::Megablock))
        .map(|b| b.id)
        .collect();
    if blocks.is_empty() {
        return None;
    }
    let own = st
        .home
        .map(|h| (h.home.building, h.home.level, h.home.number));
    let first = (roll(seed, 0xB10C, n, hour) % blocks.len() as u64) as usize;
    for k in 0..blocks.len() {
        let b = blocks[(first + k) % blocks.len()];
        crate::credentials::prepare(st, b);
        let Some(homes) = st.interiors.homes_of(b) else {
            continue;
        };
        let mut open: Vec<(i32, u16)> = homes
            .flats()
            .filter(|&(l, f)| own != Some((b, l, f)) && st.journal.code_for(b, l, f).is_none())
            .collect();
        if open.is_empty() {
            continue;
        }
        let mut picked = Vec::new();
        for j in 0..want.min(open.len()) {
            let at = roll(seed, 0xF1A7 ^ j as u64, n, hour) % open.len() as u64;
            picked.push(open.remove(at as usize));
        }
        return Some((b, picked));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backdoors::{State, Why};
    use crate::credentials::{Code, Source};
    use crate::jobs::testutil::state_home;
    use citygen::{BuildingKind, Place, StratumKind};
    use game::Stack;

    const H: u64 = 60;

    fn seed(st: &AppState) -> u64 {
        st.city.as_ref().map_or(0, |c| c.seed)
    }

    fn install(st: &mut AppState) {
        let p = &mut st.backdoors.jobs.programs;
        p.insert("sniff".to_string());
        p.insert("keepalive".to_string());
    }

    /// A lived-in block other than home with at least `flats` flats, the `skip`th.
    fn block(st: &mut AppState, flats: usize, skip: usize) -> BuildingId {
        let home = st.home.unwrap().home.building;
        let ids: Vec<BuildingId> = st
            .city
            .as_ref()
            .unwrap()
            .buildings
            .iter()
            .filter(|b| matches!(b.kind, BuildingKind::Apartment | BuildingKind::Megablock))
            .map(|b| b.id)
            .filter(|b| *b != home)
            .collect();
        let mut found = 0;
        for b in ids {
            crate::credentials::prepare(st, b);
            let fits = st
                .interiors
                .homes_of(b)
                .is_some_and(|h| !h.is_empty() && h.all_flats().count() >= flats);
            if fits {
                if found == skip {
                    return b;
                }
                found += 1;
            }
        }
        panic!("no block with {flats} flats");
    }

    /// A Net backdoor on a block of at least `flats` flats; the minute it went in.
    fn planted(st: &mut AppState, flats: usize, skip: usize) -> (BuildingId, u64) {
        let b = block(st, flats, skip);
        let now = st.net.now(st);
        let seed = seed(st);
        st.backdoors.plant_net(b, 1, seed, now, 3);
        (b, now)
    }

    fn door(st: &mut AppState, b: BuildingId) -> &mut crate::backdoors::Backdoor {
        st.backdoors
            .list
            .iter_mut()
            .find(|d| d.building == b)
            .unwrap()
    }

    fn codes(st: &AppState, source: Source) -> Vec<&Code> {
        (st.journal.codes().iter())
            .filter(|c| c.source == source)
            .collect()
    }

    fn reported(st: &AppState, text: &str) -> usize {
        // the report, and what the morning mail has already carried off
        (st.backdoors.jobs.report.iter())
            .chain(
                st.journal
                    .mails
                    .iter()
                    .filter(|m| m.from == "cron")
                    .flat_map(|m| m.body.iter()),
            )
            .filter(|l| l.contains(text))
            .count()
    }

    fn host(st: &AppState, b: BuildingId) -> String {
        crate::helix::hosts::building_hostname(st, b)
    }

    fn fit(st: &mut AppState, bay: usize, sled: &str) {
        let c = st.content.clone();
        st.character.rig_copy_mut(&c).fitted[bay] = Some(Stack {
            item: ItemId::new(sled),
            qty: 1,
            copy: None,
        });
    }

    /// The Lotus tower with a Tinman and a Q-Stack: five seats at ×1.5.
    fn lotus(st: &mut AppState) {
        let c = st.content.clone();
        crate::rig::fit_new_rack(&mut st.character, &c, "rack_lotus");
        fit(st, 0, "sled_tinman");
        fit(st, 1, "sled_qstack");
        assert_eq!(crate::rig::of(st).seats, 5);
    }

    #[test]
    fn sniff_learns_a_flat_every_three_hours() {
        let mut st = state_home();
        install(&mut st);
        let (b, t0) = planted(&mut st, 8, 0);
        start(&mut st, JobKind::Sniff(b)).unwrap();
        advance(&mut st, t0 + 2 * H);
        assert!(codes(&st, Source::Sniffed).is_empty());
        advance(&mut st, t0 + 3 * H);
        assert_eq!(codes(&st, Source::Sniffed).len(), 1);
        advance(&mut st, t0 + 9 * H);
        let got = codes(&st, Source::Sniffed);
        assert_eq!(got.len(), 3);
        assert!(got.iter().all(|c| c.building == b));
        assert_eq!(st.backdoors.jobs.running.len(), 1, "still sniffing");
        assert_eq!(st.backdoors.jobs.running[0].found.len(), 3);
    }

    #[test]
    fn sniff_ends_with_nothing_left() {
        let mut st = state_home();
        install(&mut st);
        let (b, t0) = planted(&mut st, 2, 0);
        let seed = seed(&st);
        let flats: Vec<(i32, u16)> = st.interiors.homes_of(b).unwrap().all_flats().collect();
        // Every flat but the last is known already.
        for &(level, number) in &flats[..flats.len() - 1] {
            st.journal.learn(Code {
                building: b,
                level,
                number,
                household: None,
                code: citygen::rng::door_code(seed, b, level, number),
                source: Source::Mail,
                noted_by: None,
                at: t0,
            });
        }
        start(&mut st, JobKind::Sniff(b)).unwrap();
        advance(&mut st, t0 + 3 * H);
        assert_eq!(codes(&st, Source::Sniffed).len(), 1);
        assert!(st.backdoors.jobs.running.is_empty(), "it ends");
        let line = format!("sniff · {} · nothing left to find", host(&st, b));
        assert_eq!(reported(&st, &line), 1, "{:?}", st.backdoors.jobs.report);
    }

    #[test]
    fn sift_gives_codes_and_returns_a_sifted_shard() {
        let mut st = state_home();
        let paydata = ItemId::new("paydata");
        st.character.give(paydata.clone(), 1);
        let t0 = st.net.now(&st);
        start(
            &mut st,
            JobKind::Sift {
                shard: paydata.clone(),
            },
        )
        .unwrap();
        assert_eq!(st.character.count(&paydata), 0, "the shard is in the rig");
        advance(&mut st, t0 + 5 * H);
        assert!(codes(&st, Source::Sifted).is_empty());
        advance(&mut st, t0 + 6 * H);
        assert_eq!(codes(&st, Source::Sifted).len(), 2);
        assert_eq!(st.character.count(&ItemId::new("paydata_sifted")), 1);
        assert!(st.backdoors.jobs.running.is_empty());
        assert_eq!(
            start(
                &mut st,
                JobKind::Sift {
                    shard: ItemId::new("paydata_sifted")
                }
            ),
            Err("sift: already sifted".to_string())
        );

        let vault = ItemId::new("vault_data");
        st.character.give(vault.clone(), 1);
        // The clock never ran: this one starts at t0 too, and takes ten hours.
        start(
            &mut st,
            JobKind::Sift {
                shard: vault.clone(),
            },
        )
        .unwrap();
        advance(&mut st, t0 + 9 * H);
        assert_eq!(codes(&st, Source::Sifted).len(), 2);
        advance(&mut st, t0 + 10 * H);
        assert_eq!(codes(&st, Source::Sifted).len(), 6);
        assert_eq!(st.character.count(&ItemId::new("vault_data_sifted")), 1);
    }

    #[test]
    fn a_stash_one_time_in_three() {
        let mut st = state_home();
        let seed = seed(&st);
        let hits = (1..=300)
            .filter(|n| stash_roll(seed, *n, SIFT_HOURS))
            .count();
        assert!((70..=130).contains(&hits), "{hits} of 300");
        for id in STASHES {
            let item = st.content.item(&ItemId::new(id)).expect(id);
            assert!(!item.per_copy(), "{id} lies on the floor as a stack");
        }

        let n = (1..).find(|n| stash_roll(seed, *n, SIFT_HOURS)).unwrap();
        st.backdoors.jobs.next = n - 1;
        let paydata = ItemId::new("paydata");
        st.character.give(paydata.clone(), 1);
        let t0 = st.net.now(&st);
        assert_eq!(start(&mut st, JobKind::Sift { shard: paydata }), Ok(n));
        advance(&mut st, t0 + 6 * H);
        let s = st.backdoors.jobs.stashes.last().expect("a stash").clone();
        let place = Place::Floor {
            building: s.building,
            level: s.level,
        };
        assert!(st.ground.at(place, s.tile).iter().any(|x| x.item == s.item));
        assert!(
            st.journal.code_for(s.building, s.level, s.number).is_some(),
            "its code known"
        );
        let marked = [
            StratumKind::Street,
            StratumKind::Heights,
            StratumKind::Undercity,
        ]
        .into_iter()
        .flat_map(|k| crate::track::listings::listings(&st, k))
        .any(|l| l.name.starts_with("stash ·") && l.building == Some(s.building));
        assert!(marked, "a stash on the MAP");
        assert_eq!(reported(&st, "on your map"), 1);
        // Taken off the floor, the mark goes.
        st.ground.take(place, s.tile, &s.item, 1);
        let marked = [
            StratumKind::Street,
            StratumKind::Heights,
            StratumKind::Undercity,
        ]
        .into_iter()
        .flat_map(|k| crate::track::listings::listings(&st, k))
        .any(|l| l.name.starts_with("stash ·"));
        assert!(!marked);
    }

    #[test]
    fn keepalive_freezes_the_life() {
        let mut st = state_home();
        install(&mut st);
        let (b, t0) = planted(&mut st, 1, 0);
        let health = door(&mut st, b).health(t0);
        let n = start(&mut st, JobKind::Keepalive(b)).unwrap();
        advance(&mut st, t0 + 48 * H);
        let d = door(&mut st, b).clone();
        assert_eq!(d.planted, t0 + 48 * H, "48 hours given back");
        assert_eq!(d.health(t0 + 48 * H), health);
        assert!(st.backdoors.held(b).is_some());
        kill(&mut st, n).unwrap();
        advance(&mut st, t0 + 58 * H);
        assert_eq!(door(&mut st, b).planted, t0 + 48 * H, "it runs down again");
    }

    #[test]
    fn a_keepalive_holds_a_line_with_minutes_left() {
        let mut st = state_home();
        install(&mut st);
        let (b, t0) = planted(&mut st, 1, 0);
        let d = door(&mut st, b);
        d.nominal = 30;
        d.factor = 100;
        start(&mut st, JobKind::Keepalive(b)).unwrap();
        advance(&mut st, t0 + 48 * H);
        assert!(st.backdoors.held(b).is_some(), "still standing");
        assert_eq!(st.backdoors.jobs.running.len(), 1);
        assert_eq!(
            door(&mut st, b).real_end(),
            t0 + 48 * H + 30,
            "30 minutes left"
        );
    }

    #[test]
    fn a_faster_core_is_no_noisier() {
        let mut st = state_home();
        install(&mut st);
        fit(&mut st, 0, "sled_qstack");
        let (b, t0) = planted(&mut st, 14, 0);
        start(&mut st, JobKind::Sniff(b)).unwrap();
        advance(&mut st, t0 + 12 * H);
        assert_eq!(
            st.backdoors.jobs.heat.get(&b),
            Some(&(12 * 4)),
            "a point a clock hour"
        );
        assert_eq!(
            codes(&st, Source::Sniffed).len(),
            6,
            "but it finds half again as fast"
        );
    }

    #[test]
    fn a_raid_in_a_long_catch_up_stops_the_jobs_there() {
        let mut st = state_home();
        fit(&mut st, 0, "sled_tinman");
        let paydata = ItemId::new("paydata");
        st.character.give(paydata.clone(), 1);
        let c0 = st.character.credits;
        let t0 = st.net.now(&st);
        start(&mut st, JobKind::Rent).unwrap();
        start(
            &mut st,
            JobKind::Sift {
                shard: paydata.clone(),
            },
        )
        .unwrap();
        // A trace with nothing between them and the door, there in four hours.
        st.backdoors.start_trace(t0, 0);
        st.backdoors.trace.as_mut().unwrap().moves_at = t0 + 4 * H;
        let (city, world) = (st.city.as_ref().unwrap(), st.world.as_mut().unwrap());
        world.catch_up(city, world.tick() + 30 * H * sim::TICKS_PER_MINUTE);
        crate::net::tick_backdoors(&mut st);
        assert!(!st.backdoors.seized(st.net.now(&st)), "and the rig is back");
        assert!(st.backdoors.jobs.running.is_empty());
        // Four hours of rent before the door went in, and the sift two short.
        assert_eq!(st.character.credits, c0 + 4 * RENT_PER_HOUR);
        assert_eq!(st.character.count(&paydata), 1, "the shard back, unsifted");
        assert_eq!(st.character.count(&ItemId::new("paydata_sifted")), 0);
        assert_eq!(reported(&st, "the rig was seized"), 2);
    }

    fn cron_mails(st: &AppState) -> Vec<&crate::credentials::Mail> {
        st.journal
            .mails
            .iter()
            .filter(|m| m.from == "cron")
            .collect()
    }

    /// Minutes from `now` to the next `hh:00` on the clock.
    fn until(now: u64, hh: u64) -> u64 {
        let day = 24 * 60;
        let at = (crate::ui::hud::START_MINUTE_OF_DAY as u64 + now) % day;
        (hh * 60 + day - at) % day
    }

    #[test]
    fn the_cron_mail_lands_at_six() {
        let mut st = state_home();
        install(&mut st);
        fit(&mut st, 0, "sled_tinman");
        let t0 = st.net.now(&st);
        let (b, _) = planted(&mut st, 8, 0);
        start(&mut st, JobKind::Rent).unwrap();
        start(&mut st, JobKind::Sniff(b)).unwrap();
        let two = t0 + until(t0, 2);
        let six = t0 + until(t0, 6);
        assert!(two < six);
        advance(&mut st, two);
        assert!(!st.backdoors.jobs.report.is_empty(), "the night's lines");
        assert!(cron_mails(&st).is_empty(), "nothing before 06:00");
        advance(&mut st, six - 1);
        assert!(cron_mails(&st).is_empty());
        advance(&mut st, six);
        let mails = cron_mails(&st);
        assert_eq!(mails.len(), 1);
        let m = mails[0];
        assert_eq!(m.subject, "overnight · 2 jobs");
        assert_eq!(
            (m.read, m.at, m.job.is_none(), m.story.is_none()),
            (false, six, true, true)
        );
        let body = m.body.join("\n");
        assert!(body.contains("rent · spare cycles · "), "{body}");
        assert!(body.contains("still running"), "{body}");
        assert!(body.contains("\n└ ¤"), "{body}");
        assert!(st.backdoors.jobs.report.is_empty(), "the report is emptied");
        // a second morning with nothing new says nothing; with something, one more
        advance(&mut st, six + 60);
        advance(&mut st, six + 24 * 60);
        assert_eq!(cron_mails(&st).len(), 2, "rent paid on");
        advance(&mut st, six + 24 * 60 + 90);
        assert_eq!(cron_mails(&st).len(), 2, "one a morning");
    }

    #[test]
    fn no_report_no_mail() {
        let mut st = state_home();
        let t0 = st.net.now(&st);
        advance(&mut st, t0 + until(t0, 6) + 24 * 60);
        assert!(cron_mails(&st).is_empty());
    }

    #[test]
    fn away_lands_on_return() {
        let mut st = state_home();
        start(&mut st, JobKind::Rent).unwrap();
        let (city, world) = (st.city.as_ref().unwrap(), st.world.as_mut().unwrap());
        world.catch_up(city, world.tick() + 48 * H * sim::TICKS_PER_MINUTE);
        crate::net::tick_backdoors(&mut st);
        // two mornings went by: one mail for each, as ticking through would have sent
        let at: Vec<u64> = cron_mails(&st).iter().map(|m| m.at).collect();
        assert_eq!(at, [morning_at(1), morning_at(2)]);
        crate::net::tick_backdoors(&mut st);
        assert_eq!(cron_mails(&st).len(), 2);
    }

    #[test]
    fn rent_pays_by_the_hour() {
        let mut st = state_home();
        let c0 = st.character.credits;
        let t0 = st.net.now(&st);
        start(&mut st, JobKind::Rent).unwrap();
        advance(&mut st, t0 + 4 * H);
        assert_eq!(st.character.credits, c0 + 24);
        assert_eq!(reported(&st, "rent · spare cycles · ¤24 paid"), 1);

        let mut st = state_home();
        fit(&mut st, 0, "sled_qstack");
        let c0 = st.character.credits;
        let t0 = st.net.now(&st);
        start(&mut st, JobKind::Rent).unwrap();
        advance(&mut st, t0 + 4 * H);
        assert_eq!(st.character.credits, c0 + 36, "half again on a Q-Stack");
    }

    #[test]
    fn noise_patches_at_36() {
        let mut st = state_home();
        install(&mut st);
        let (b, t0) = planted(&mut st, 14, 0);
        start(&mut st, JobKind::Sniff(b)).unwrap();
        let warning = "their auth.log is getting long. somebody reads those.";
        advance(&mut st, t0 + 17 * H);
        assert_eq!(reported(&st, warning), 0);
        advance(&mut st, t0 + 18 * H);
        assert_eq!(reported(&st, warning), 1);
        advance(&mut st, t0 + 35 * H);
        assert!(st.backdoors.held(b).is_some());
        assert_eq!(st.backdoors.jobs.heat.get(&b), Some(&(35 * 4)));
        advance(&mut st, t0 + 36 * H);
        assert!(matches!(
            door(&mut st, b).state,
            State::Burned {
                why: Why::Noticed,
                ..
            }
        ));
        assert!(st.backdoors.jobs.running.is_empty(), "the job ends");
        assert_eq!(reported(&st, "the admins found it"), 1);
        assert_eq!(reported(&st, warning), 1, "warned once");
        assert!(
            st.backdoors.can_plant(b, t0 + 37 * H, 3).is_err(),
            "the hole is patched"
        );
    }

    #[test]
    fn heat_cools_with_no_job_on_it() {
        let mut st = state_home();
        install(&mut st);
        let (b, t0) = planted(&mut st, 8, 0);
        let n = start(&mut st, JobKind::Sniff(b)).unwrap();
        advance(&mut st, t0 + 4 * H);
        assert_eq!(st.backdoors.jobs.heat.get(&b), Some(&16));
        kill(&mut st, n).unwrap();
        advance(&mut st, t0 + 9 * H);
        assert_eq!(st.backdoors.jobs.heat.get(&b), None, "cooled off");
    }

    #[test]
    fn a_dead_line_ends_the_job() {
        let mut st = state_home();
        install(&mut st);
        let (b, t0) = planted(&mut st, 8, 0);
        let d = door(&mut st, b);
        d.nominal = 2 * H;
        d.factor = 100;
        start(&mut st, JobKind::Sniff(b)).unwrap();
        advance(&mut st, t0 + 3 * H);
        assert!(matches!(
            door(&mut st, b).state,
            State::Burned {
                why: Why::Decay,
                ..
            }
        ));
        assert!(st.backdoors.jobs.running.is_empty());
        let line = format!("sniff · {} · the line went dead", host(&st, b));
        assert_eq!(reported(&st, &line), 1, "{:?}", st.backdoors.jobs.report);
    }

    #[test]
    fn bait_ends_at_the_first_hour() {
        let mut st = state_home();
        install(&mut st);
        let (b, t0) = planted(&mut st, 8, 0);
        door(&mut st, b).state = State::Honeypot;
        start(&mut st, JobKind::Sniff(b)).unwrap();
        advance(&mut st, t0 + H);
        assert!(st.backdoors.jobs.running.is_empty());
        assert_eq!(reported(&st, "nothing on the line but bait"), 1);
        assert_eq!(
            door(&mut st, b).state,
            State::Honeypot,
            "the honeypot stays set"
        );
        assert!(codes(&st, Source::Sniffed).is_empty());
    }

    /// Four jobs on a Lotus: a sniff, a sift, a keepalive and rent.
    fn busy() -> (AppState, u64) {
        let mut st = state_home();
        install(&mut st);
        lotus(&mut st);
        let (b1, t0) = planted(&mut st, 14, 0);
        let (b2, _) = planted(&mut st, 1, 1);
        let paydata = ItemId::new("paydata");
        st.character.give(paydata.clone(), 1);
        start(&mut st, JobKind::Sniff(b1)).unwrap();
        start(&mut st, JobKind::Sift { shard: paydata }).unwrap();
        start(&mut st, JobKind::Keepalive(b2)).unwrap();
        start(&mut st, JobKind::Rent).unwrap();
        (st, t0)
    }

    fn same(a: &AppState, b: &AppState) {
        assert_eq!(a.journal.codes(), b.journal.codes());
        assert_eq!(a.backdoors.jobs, b.backdoors.jobs);
        assert_eq!(a.backdoors.list, b.backdoors.list);
        assert_eq!(a.character.credits, b.character.credits);
        assert_eq!(a.character.inventory, b.character.inventory);
    }

    #[test]
    fn catch_up_matches_ticking_through() {
        let (mut once, t0) = busy();
        advance(&mut once, t0 + 30 * H);
        assert!(!once.backdoors.jobs.report.is_empty() || !cron_mails(&once).is_empty());
        assert!(!codes(&once, Source::Sniffed).is_empty());

        let (mut hourly, _) = busy();
        for h in 1..=30 {
            advance(&mut hourly, t0 + h * H);
        }
        same(&once, &hourly);

        let (mut minutes, _) = busy();
        let mut t = t0;
        while t < t0 + 30 * H {
            t = (t + 7).min(t0 + 30 * H);
            advance(&mut minutes, t);
        }
        same(&once, &minutes);
    }

    #[test]
    fn losing_seats_kills_the_newest_jobs() {
        let mut st = state_home();
        lotus(&mut st);
        let ns: Vec<u32> = (0..4)
            .map(|_| start(&mut st, JobKind::Rent).unwrap())
            .collect();
        let c = st.content.clone();
        st.character.rig_copy_mut(&c).fitted[1] = None;
        let killed = trim_to_seats(&mut st);
        assert_eq!(killed.len(), 2, "{killed:?}");
        let left: Vec<u32> = st.backdoors.jobs.running.iter().map(|j| j.n).collect();
        assert_eq!(left, ns[..2].to_vec());
        assert!(trim_to_seats(&mut st).is_empty());
    }

    #[test]
    fn a_seized_rig_kills_every_job() {
        let mut st = state_home();
        lotus(&mut st);
        let paydata = ItemId::new("paydata");
        st.character.give(paydata.clone(), 1);
        start(&mut st, JobKind::Rent).unwrap();
        start(&mut st, JobKind::Sift { shard: paydata }).unwrap();
        let now = st.net.now(&st);
        st.backdoors.rig_seized_until = Some(now + 24 * H);
        crate::net::tick_backdoors(&mut st);
        assert!(st.backdoors.jobs.running.is_empty());
        assert_eq!(reported(&st, "the rig was seized"), 2);
        assert_eq!(
            start(&mut st, JobKind::Rent),
            Err("run: the rig is seized".to_string())
        );
    }

    #[test]
    fn a_wiped_rig_keeps_the_jobs_off_its_lines() {
        let mut st = state_home();
        install(&mut st);
        lotus(&mut st);
        let (b, _) = planted(&mut st, 8, 0);
        start(&mut st, JobKind::Sniff(b)).unwrap();
        let rent = start(&mut st, JobKind::Rent).unwrap();
        let now = st.net.now(&st);
        raid(&mut st, now, true);
        let left: Vec<u32> = st.backdoors.jobs.running.iter().map(|j| j.n).collect();
        assert_eq!(left, vec![rent]);
        assert_eq!(reported(&st, "the rig wiped itself"), 1);
    }

    #[test]
    fn killing_a_sift_gives_the_shard_back() {
        let mut st = state_home();
        let paydata = ItemId::new("paydata");
        st.character.give(paydata.clone(), 1);
        let n = start(
            &mut st,
            JobKind::Sift {
                shard: paydata.clone(),
            },
        )
        .unwrap();
        assert_eq!(
            kill(&mut st, n),
            Ok(format!(
                "killed {n} · sift · paydata shard · the shard is back in your pack"
            ))
        );
        assert_eq!(st.character.count(&paydata), 1);
        assert_eq!(kill(&mut st, n), Err(format!("kill: no job {n}")));
    }

    #[test]
    fn no_core_no_run() {
        let mut st = state_home();
        let c = st.content.clone();
        st.character.rig_copy_mut(&c).fitted[0] = None;
        assert_eq!(crate::rig::of(&st).seats, 0);
        assert_eq!(
            start(&mut st, JobKind::Rent),
            Err("run: no core in the rig".to_string())
        );
    }

    #[test]
    fn start_refuses() {
        let mut st = state_home();
        start(&mut st, JobKind::Rent).unwrap();
        assert_eq!(
            start(&mut st, JobKind::Rent),
            Err("run: every seat is busy · kill a job, or fit a bigger core".to_string())
        );
        st.backdoors.jobs.running.clear();
        let b = block(&mut st, 1, 0);
        let now = st.net.now(&st);
        st.backdoors.plant_key(b, "ops", 0, seed(&st), now);
        assert_eq!(
            start(&mut st, JobKind::Sniff(b)),
            Err("run: no such job · ls /opt/jobs".to_string()),
            "sniff comes on a shard"
        );
        install(&mut st);
        assert_eq!(
            start(&mut st, JobKind::Sniff(b)),
            Err("sniff: a key won't hold that long".to_string())
        );
        assert_eq!(
            start(
                &mut st,
                JobKind::Sift {
                    shard: ItemId::new("paydata")
                }
            ),
            Err("sift: no Paydata shard in your pack".to_string())
        );
    }
}
