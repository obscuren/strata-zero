# Jobs: generated work from the fixers — design

Date: 2026-10-18. Status: approved in chat by Jeffrey (shape: a job board plus an
authored chain, cut into two specs with the board first; jobs given in person
with the brief mailed after; all four kinds with the kind enum left open for
eliminate and pick-up; deadlines and a standing per fixer). Spec, plan and
build run without check-ins in the worktree `jobs`; merging is his call.
Builds on: `2026-10-17-credentials-journal-design.md` (homes, codes, the
journal's mails), `2026-10-17-access-terminal-design.md` (the first mail),
`2026-10-18-helix-os-design.md` (the rig's mail as files),
`2026-10-18-backdoors-design.md` (planting, lifetimes, the fixer gone quiet),
`2026-10-15-constructs-and-programs-design.md` (copy, the vault),
`2026-10-14-living-city-design.md` (people, professions, schedules),
`2026-10-18-interaction-design.md` (the Space window and Talk).
Spec 2, the authored chain, follows this one and is built on its engine.

## Intent

The city is lived in but asks nothing of you. This slice gives the player
**work**: a fixer on a corner offers a job rolled from the city around them,
the brief lands on the rig, the job happens in the places and systems already
built (a locked flat, a construct's server, a host's `authorized_keys`, a
named person at their station), and it ends back at the fixer, who pays. Jobs
have a clock and the fixer has a memory: finish them and better work opens,
blow them and the number goes dead. Today's five content quests, three of them
fakes seeded at creation, go; the welcome favour becomes the first job of this
system so the first mail already walks you into the loop.

## Words

- **Job**: one piece of work from one fixer, with a kind, a target in the
  city, objectives, pay and a deadline. Runtime data, never content.
- **Fixer**: a person of the sim with profession `fixer`. They hold a corner
  18:00–04:00 and drink in a bar after; that is where you find them.
- **Standing**: a fixer's opinion of you, an integer per fixer. It gates what
  they offer and whether they talk to you at all.
- **Offer**: the job a fixer has for you right now, before you take it. One
  at a time per fixer; rolled once and kept until taken or replaced.
- **Hand-in**: the Talk row that ends a job at its fixer and pays.
- **Brief**: the job's text, in the fixer's voice: the offer window shows it,
  the mail repeats it.

## Goals

- A `jobs` module owns jobs, fixers' standings and job generation; the rest
  of the app reports events to it and reads it for display.
- Four kinds in this slice: **Fetch** (a shard on the floor of a locked
  flat), **Copy** (a server or a vault in the Net), **Plant** (a backdoor on
  a named host, standing at hand-in), **Deliver** (a sealed package into a
  named person's hands). The kind is an open enum.
- Every city has at least one fixer.
- Talk on a fixer opens their rows: the line, the offer, the hand-in, the
  drop. Taking a job mails the brief to the rig. Handing in pays credits in
  person.
- Every job has a deadline in game hours that the QUEST pane shows; past it
  the job fails and the fixer remembers.
- Standing: +1 per job handed in, −1 per drop, −2 per failure. Fetch and
  Deliver from the start, Copy from 2, Plant from 4, harder targets as it
  rises; at −3 and below the fixer refuses you.
- Generation is seeded: the same city, fixer and counter give the same job.
- The welcome favour is the first Fetch job, already taken, with the hand-in
  at the fixer's corner; `Journal.favour` and the `first_favour` content quest
  go.
- The QUEST pane and the deck's QUESTS tab read jobs. `Character.quests`,
  `QuestLog`, `Creation.quests` and the five content quests go; the `Quest`
  content type stays, its list empty, for spec 2.
- All text that is voice lives in `flavor/jobs.ron`.

## Non-goals

- Saving jobs or standings to disk (nothing of the character persists yet).
- Anything to spend credits on. Pay is a number on the HUD for now.
- Eliminate, pick-up, escort or any kind beyond the four; the enum is open
  for them and nothing else is prepared.
- Dialogue beyond rows: no branching, no persuasion, no haggling (the
  `haggle` talent flag stays unread).
- A fixer who moves the target, counter-ops, rival runners, or a job that
  changes the sim's people beyond the recipient taking a package.
- Reputation across fixers, factions, or a street-wide wanted level.
- The authored chain (spec 2).
- Changing the Space window's fixed verbs. Job rows ride on `t` Talk.

## 1. The model: `jobs.rs`

A new module `crates/strata-zero/src/jobs.rs`, `st.jobs: Jobs` on the app
state, session-only, reset by `enter_city`.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JobId(pub u32);

pub enum Kind {
    /// A paydata shard lies on `tile` of flat `number` on `level` of `building`.
    Fetch { building: BuildingId, level: i32, number: u16, tile: Pos },
    /// Copy the building's server, or its vault when `vault`.
    Copy { building: BuildingId, vault: bool },
    /// A backdoor on the building's host, still standing when you hand in.
    Plant { building: BuildingId },
    /// Put `item` into this person's hands.
    Deliver { to: PersonId, item: ItemId },
}
pub enum State { Offered, Taken, Done, Failed, Dropped }
pub struct Objective { pub text: String, pub done: bool }
pub struct Job {
    pub id: JobId,
    pub fixer: PersonId,
    pub kind: Kind,
    pub title: String,
    pub brief: Vec<String>,        // the fixer's words; the mail body repeats them
    pub objectives: Vec<Objective>, // the last one is always the hand-in
    pub pay: u32,
    pub hours: u64,                 // the window, in game hours
    pub offered: u64,               // game minute
    pub taken: Option<u64>,         // game minute
    pub deadline: Option<u64>,      // taken + hours × 60
    pub state: State,
}
pub struct Fixer { pub standing: i32, pub rolled: u32 }   // rolled: the generation counter
#[derive(Default)]
pub struct Jobs {
    pub list: Vec<Job>,
    pub fixers: BTreeMap<PersonId, Fixer>,
    next: u32,
    last_minute: u64,
}
pub const REFUSE_AT: i32 = -3;
pub const COPY_AT: i32 = 2;
pub const PLANT_AT: i32 = 4;
pub const VAULT_AT: i32 = 6;
pub const GAIN_DONE: i32 = 1;
pub const LOSS_DROP: i32 = -1;
pub const LOSS_LATE: i32 = -2;
pub const HOURS_FETCH: u64 = 12;
pub const HOURS_COPY: u64 = 24;
pub const HOURS_PLANT: u64 = 24;
pub const HOURS_DELIVER: u64 = 8;
pub const PAY_FETCH: u32 = 200;
pub const PAY_DELIVER_BASE: u32 = 100;     // + manhattan distance / 4
pub const PAY_COPY_PER_SECURITY: u32 = 3;  // × security; × 5 for a vault
pub const PAY_VAULT_PER_SECURITY: u32 = 5;
pub const PAY_PLANT_BASE: u32 = 400;       // + 2 × security
pub const PAY_PLANT_PER_SECURITY: u32 = 2;
pub const FETCH_RANGE: i32 = 80;           // manhattan from the fixer's spot to the door
pub const DELIVER_MIN_RANGE: i32 = 30;     // a package never crosses the street
pub const WARN_MINUTES: u64 = 60;          // the pane turns the clock red under this
```

**Invariants.** A fixer has at most one job in `Offered` and at most one in
`Taken` at a time. A job's `objectives` always ends with the hand-in. `Done`,
`Failed` and `Dropped` are final.

**API** (pure over the state it is given; the city, the sim and the clock come
from the caller):

- `standing(fixer) -> i32`: 0 for a fixer never met.
- `offered(fixer) -> Option<&Job>`, `taken(fixer) -> Option<&Job>`,
  `active() -> Option<&Job>`: the oldest `Taken` job, the pane's.
- `allowed(standing) -> Vec<KindTag>` where `KindTag` is the kind without its
  target: `Fetch, Deliver` always; `Copy` at `COPY_AT`; `Plant` at
  `PLANT_AT`. Vaults are a Copy at `VAULT_AT`.
- `take(id, now)`: `Offered → Taken`, sets `taken` and `deadline`.
- `drop(id)`: `Taken → Dropped`, standing `LOSS_DROP`.
- `hand_in(id) -> u32`: `Taken → Done` with every objective ticked, standing
  `GAIN_DONE`, returns the pay. The caller checks `ready(id)` first.
- `ready(&self, id, backdoors) -> Result<(), String>`: every objective but
  the hand-in is done; for Plant the backdoor on the building is standing
  (`backdoors.held(b)`), else `Err("the backdoor burned · plant it again")`;
  for Fetch and Copy the pack holds the item (checked by the caller against
  the character: `paydata`, or `vault_data` for a vault), else `Err("the
  shard is not on you")`.
- `on(&mut self, ev: Event) -> Vec<Change>` with

  ```rust
  pub enum Event {
      Taken { place: Place, pos: Pos },           // something left the ground here
      Copied { building: BuildingId, vault: bool },
      Planted { building: BuildingId },
      Handed { job: JobId },                      // the package reached its person
  }
  pub enum Change { Ticked { job: JobId, objective: usize } }
  ```

  `Taken` ticks a Fetch whose flat floor and tile match (the caller sends it
  only when no paydata is left on the tile); `Copied` ticks a Copy on that
  building with the same `vault`; `Planted` ticks a Plant on that building;
  `Handed` ticks the Deliver's first objective. A job ticks once; a second
  event for a ticked objective changes nothing.
- `minute(now) -> Vec<Late>` (`pub struct Late { pub job: JobId }`) steps
  once per game minute: every `Taken` job with `deadline < now` becomes
  `Failed`, standing `LOSS_LATE`, and is reported. Idempotent for the same
  minute.
- `left(job, now) -> Option<u64>`: minutes to the deadline, saturating.

## 2. Fixers in the city

`sim::people::seed_population` seeds fixers on the corners of every Neon Strip
(two) and Night Market (one). When no district yields one, it seeds one on
the corners of the district with the most corner tiles (ties to the lowest
district index), and when no district has corners at all, on its beats. A
city therefore always has a fixer; the test `every_city_has_a_fixer` runs it
over the seeds the sim already sweeps.

A fixer's day is the one the sim gives them: the corner from 18:00 to 04:00,
then a bar for ninety to a hundred and fifty minutes, then home. The brief
says where to find them: `holds the corner on <District> from 18:00`. The
sim gains two readers the briefs and rows need:

- `World::work_hours(id) -> Option<(u16, u16)>`: the person's own span,
  from `shift_start` and the profession's spans (the one that starts at
  `shift_start`, else the first).
- `World::spot_of(id) -> Option<Pos>`: `Work::Spot(pos)`.

The first job's fixer is the one whose spot is nearest the player's
megablock door, as `nearest_fixer` picks today, by `PersonId` now rather than
by name.

## 3. The offer window

Talk on a fixer no longer shows the one-line card. `interact::gather` asks
`jobs::rows(st, person)` for the rows of anyone whose profession is `fixer`,
and the Space window about them carries, in this order:

```
x  Look
t  Talk
w  Work: <title>                    ¤200 · 12h       (an Offered job)
h  Hand in: <title>                 ¤200             (a Taken job that is ready)
d  Drop: <title>                                     (a Taken job)
```

- `t Talk` shows the profession line as a card, as for anyone.
- `w Work` opens the offer in place of the window (`ScreenCmd::Replace`): a
  popup headed by the job's title, subtitled `pay ¤200 · 12 hours · <kind>`,
  the brief as a body, then `y Take the job` and `n Not now`. `Popup` gains an
  optional **body**: wrapped lines between the subtitle and the rows, so a
  window can carry a paragraph and still fire rows. Taking it runs
  `Jobs::take`, logs `` `<title>` · taken · <fixer>: "Mail's on your rig." ``
  and pushes a `Mail` from the fixer (first name), subject the title in lower
  case, body the brief plus the where-to-find line and `by <hh:mm>` of the
  deadline. Not now keeps the offer for next time.
- When the fixer has no `Offered` job and no `Taken` one, `rows` rolls one
  (§5) before building the window, so the row is there the first time you
  talk. When generation finds nothing in the city, the row is
  `Choice::disabled('w', "Work")` with the tail `· nothing right now`.
- `h Hand in` appears only when `ready` says so; a Taken job that is not
  ready shows the row disabled with `ready`'s reason as its tail. Firing it
  takes the item from the pack (Fetch: one `paydata`; Copy: one `paydata`,
  or one `vault_data` for a vault), runs `hand_in`, adds the pay to
  `character.credits` and logs `` `<title>` · handed in · ¤200 · <fixer>: "<handed line>" ``.
- `d Drop` opens a confirm (`y Drop it`, `n Keep it`) in place of the
  window. Dropping logs `` `<title>` · dropped · <fixer>: "<dropped line>" `` and
  a Deliver's package, if still carried, stays in the pack.
- Standing at or under `REFUSE_AT`: no `w`, `h` or `d` rows; `t Talk` shows
  a refusal line from `jobs.ron` instead of the profession line, and no job
  is rolled.
- While `st.backdoors.fixer_quiet(now)` holds after a raid, every fixer
  treats you as refused for the day, as the first mail already does.

Picks added to `screens/popup.rs`: `Offer(JobId)`, `Take(JobId)`,
`HandIn(JobId)`, `DropAsk(JobId)`, `Drop(JobId)`, `Hand(JobId)`.

**Deliver's other end.** Talk on the person a Taken Deliver names carries
`h Hand over: Sealed package` (disabled with `· not on you` when the pack
has none). Firing it takes the item, sends `Event::Handed`, logs
`` `<name>` takes the package · "<receiver line>" · <fixer> owes you `` and
leaves the hand-in at the fixer as the last objective.

## 4. The clock and the pane

`jobs::tick(st)` runs beside `net::tick_backdoors` in the explore update.
A `Late` event logs `` `<title>` · too late · <fixer> will hear of it `` as a
warning and mails `You had a window. It closed.` from the fixer.

The QUEST pane shows `Jobs::active()`:

```
 THE DROP · Dez · 3h 12m
 » Take the shard from 1104 on 11F of Hollis Court
 ▰▱ 1/2
```

The clock is the time left to the deadline (`3h 12m`, `48m` under an hour,
`late` at zero before the minute tick fails it), drawn in the warn colour under
`WARN_MINUTES`. With no Taken job the pane reads ` no job on · find a fixer`.

The deck's QUESTS tab lists jobs, Taken first (oldest first), then Offered
(dim, with the fixer's name), then the finished ones with `✓` Done, `✗`
Failed and `–` Dropped. The detail reads the title, `from <fixer> · <district>
· ¤pay · <left or the outcome>`, the brief, and the objectives as `☑`/`☐`.
The dev console learns `job [fetch|copy|plant|deliver]` (rolls and takes
that kind from the nearest fixer, bypassing standing), `standing <n>` (sets
it with every fixer) and `jobs` (lists them).

## 5. Generation

`generate(st, fixer, kind) -> Option<Job>` is seeded: every roll uses
`hash64(seed, fixer.0 as u64 ^ (counter as u64) << 32 ^ salt)`, with the
fixer's `rolled` counter bumped once per offer, so a fixer's sequence of
offers is the city's. `offer(st, fixer)` picks the kind from
`allowed(standing)` by one roll, tries `generate` for it, and falls through
the other allowed kinds in order when the city has no target for the first.
Objective texts and the where-to-find line come from the templates in §6.

**Fetch.** Candidates: closed buildings whose nearest door lies within
`FETCH_RANGE` of the fixer's spot and that have either a `Server` node
(a server copy teaches the codes) or homes with a lost card (any building
`prepare` leaves a card in; in practice every lived-in building). Pick one by
roll; `prepare` it; take its lived-in flats (`Homes::flats`) less the player's
own; none, try the next candidate, up to four; pick a flat by roll; lay one
`paydata` on the first free walkable tile of its room (as the favour does)
and remember the tile. Pay `PAY_FETCH`. Objectives: `Take the shard from
<number> on <floor> of <Building>`, hand-in. The brief names the flat, the
building and how the codes can be had in one line (`the lobby desk keeps a
card`, `the server knows the code`), never the code itself: that is the job.

**Copy.** Candidates: buildings with a `Server` node (`access::scannable` over
every district), filtered by security band: standing under `PLANT_AT`:
`security < ICE_THRESHOLD`; under `VAULT_AT`: `< GREY_THRESHOLD`; from
`VAULT_AT`: any, and a roll of one in three picks a vault among the buildings
with a `DataVault` node. Pick by roll. Pay `security × PAY_COPY_PER_SECURITY`
(`PAY_VAULT_PER_SECURITY` for a vault). Objectives: `Copy <hostname>'s
server` (or `vault`), hand-in.

**Plant.** Candidates as Copy's server list in the same bands, less any
building where a backdoor already stands or is patched. Pay `PAY_PLANT_BASE +
security × PAY_PLANT_PER_SECURITY`. Objectives: `Plant a backdoor on
<hostname>`, `Report to <fixer> while it stands`.

**Deliver.** Candidates: people who are not fixers, not of the player's
household, with `Work::Building(b)` on a closed building or `Work::Spot`,
whose work lies at least `DELIVER_MIN_RANGE` from the fixer's spot. Pick by
roll. The player gets one `sealed_package` on taking the job, a new content
item (`kind: Component, weight: 0.5, value: 0`, `desc: "Taped shut. Heavier
than it looks. Not yours to open."`). Pay `PAY_DELIVER_BASE + distance / 4`.
Objectives: `Hand the package to <Name> · <place> · <hh:mm>–<hh:mm>`, hand-in.
The brief names the person, their profession and where they work.

The first job (`jobs::first_favour`, replacing `credentials::first_mail`) is
a Fetch in the player's own megablock from the nearest fixer, taken at
`now`, with the mail as today plus the where-to-find line; its brief still
carries the door code, as the welcome does. Nothing else about the welcome
changes: the shard, the code in the journal, the fixer's name in later mails.

## 6. Flavor: `flavor/jobs.ron`

Parsed with `include_str!` and validated at load like `people.ron`:

```ron
Jobs(
    titles: [
        (Fetch,   ["The {building} shard", "A flat on {floor}", "Somebody's quarter"]),
        (Copy,    ["{host}, copied", "What {host} keeps", "A quiet read"]),
        (Plant,   ["A door into {host}", "Leave it open", "{host}, later"]),
        (Deliver, ["A parcel for {name}", "Across town", "No questions"]),
    ],
    briefs: [
        (Fetch, [["{number} on {floor} of {building}. {how}.",
                 "There's a shard on the floor with my name on it. Bring it."]]),
        (Copy, [["{host}. Security {security}. The server, nothing else.",
                "Copy it and walk. I pay for the shard, not the story."]]),
        (Plant, [["I need a way into {host} that isn't the front door.",
                 "Leave me a backdoor and come tell me while it still stands."]]),
        (Deliver, [["{name}, the {profession} at {place}. Works {hours}.",
                   "Package into their hands. Not a counter, not a colleague. Hands."]]),
    ],
    how: (card: "the lobby desk keeps a card", server: "the server knows the code"),
    where: "I hold the corner on {district} from 18:00.",
    offer:    ["Got something. Interested?", "Work found you. Want it?"],
    taken:    ["Mail's on your rig.", "Details on your rig. Don't lose them."],
    handed:   ["Good. Here.", "Clean. Here's yours."],
    dropped:  ["Fine. Someone hungrier will.", "Noted."],
    late:     ["You had a window. It closed.", "Late is the same as never."],
    refused:  ["Lose this number.", "I don't know you."],
    nothing:  ["Nothing right now. Come back.", "Dry. Try me tomorrow."],
    receiver: ["About time.", "Took you long enough.", "Not a word. Thanks."],
)
```

Slots: `{building}` the building's name or `the <kind>`, `{floor}` the
`FloorPlan` label, `{number}` the flat number, `{how}` from `how`, `{host}`
the hostname, `{security}` the number, `{name}`, `{profession}`, `{place}`
(the work building's name or `the corner`), `{hours}` as `09:00–20:00`,
`{district}` the fixer's district. Validation refuses an unknown slot in a
template, an empty list, and a kind with no title or brief. Every line is
picked by the job's roll, so the same job always reads the same.

## 7. What goes

- `Character.quests`, `QuestLog`, `Creation.quests`, the `CreationQuest` and
  `UnknownQuest` checks, and the five entries under `quests:` in
  `content.ron`. `Content.quests` and the `Quest` type stay, the list empty;
  the test that counted four quests counts jobs instead.
- `Journal.favour`, `Favour`, `FAVOUR_QUEST`, `credentials::first_mail`,
  `favour_log`, `shard_taken`; `mail_read` keeps marking the mail and stops
  ticking anything (reading the mail is no longer an objective: the pane shows
  the flat from the first minute). Readers of the favour (`access`, `hosts`,
  `shots`, `terminal`, `explore` and `bin` tests, `fixer_mail`'s sender)
  read the first job instead; `fixer_mail` names the fixer of the most
  recent job, else `Dez`.
- `actions::perform` sends `Event::Taken` through `jobs::on` instead of
  calling `shard_taken`.

## 8. Testing

Unit, in `jobs.rs`: the same seed, fixer and counter give the same job; the
kinds `allowed` at each standing and the refusal line; `take`, `drop`,
`hand_in` move the state and the standing; `ready` refuses a Plant whose
backdoor burned and a Fetch without the shard; `on` ticks each kind once
and ignores a repeat; `minute` fails a late job once, with the standing loss;
`left` and the pane's clock text (`3h 12m`, `48m`, `late`).

Integration, on `AppState` fixtures (`enter_city` on seed 8 medium, as
`state_home`): the welcome is a Taken Fetch whose shard lies on its tile
and whose mail reads as before; taking the shard ticks it and the fixer's
hand-in row pays `PAY_FETCH` and raises the standing; a Deliver taken from the
console puts a package in the pack, the recipient's window offers the
hand-over, firing it ticks the job, and the fixer pays; a Copy ticks when
`net::land` copies that server; a Plant ticks from `backdoor` in a shell and
from the Backdoor program, and `ready` goes false when the backdoor is burned
from the console; a deadline passing through `advance_minutes` fails the job,
mails the fixer's line and lowers the standing; at `REFUSE_AT` the window
holds no job rows; every city in the sim's seed sweep has a fixer. A shot in
`screens/shots.rs` draws the offer window.

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo test --workspace` green before the final report.
