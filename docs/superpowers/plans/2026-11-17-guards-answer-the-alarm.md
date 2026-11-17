# Guards answer the alarm Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A site's alarm sends its guards to where you were last placed (or to sweep their
floors), a guard that loses you on alert sweeps nearby rooms, and the trace stops cooling
under a camera.

**Architecture:** A saved `NetState.placed` remembers where each building last saw you. A
fresh alarm (`net::alarm_goes_off`) calls `guards::answer`, which hands calm guards a saved
`Post.call` (`Answer` or `Sweep`). `guards::tick` runs a new `call::call_step` for a calm
guard with a call. `NetState::minute` skips cooling for buildings a camera watches.

**Tech Stack:** Rust, serde (saved state), the crate's existing test fixtures (`guards::testutil` ward, `sitenet::testutil::office_copy`).

**Spec:** `docs/superpowers/specs/2026-11-17-guards-answer-the-alarm-design.md`

## Global Constraints

- Build into `--target-dir target/smoke`, never `target/`.
- New saved fields carry `#[serde(default)]`; no `HashSet`, `Rc`, `Cell`, `Instant`, `&'static str` in saved types; `runs/fixtures/` are never regenerated.
- Constants: `PLACED_MINUTES` = 5, `SWEEP_ROOMS` = 3, `SWEEP_STEP_TICKS` = 2 × `WALK_TICKS`, `SWEEP_PAUSE_TICKS` = 30; answering moves at `GUARD_RUN_TICKS`; two guards from other floors.
- Log copy, exact: `guards close in`, `guards sweep the floors`. Footer words: `answering`, `sweeping`.
- Every commit ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; the last one ends with `Closes #102` above it.
- The five checks of CLAUDE.md pass at the end on both front ends.

## Review Focus

1. A repeat `lock_down` while already on alert (HELIX `burn` twice) answers nothing the second time: pinned in Task 4.
2. An alarm raised while you're out of the building (rig at home) leaves no calls: pinned in Task 4.
3. A guard mid-call that spots you fights as usual and, once it loses you, never resumes the stale call: pinned in Task 5.
4. An alert that ends while a guard is mid-sweep sends it back to its round: pinned in Task 3.
5. A save from before (fixtures) opens with no calls and no placings: covered by the existing fixture tests, run in Task 7.

---

### Task 1: Lifts carry guards in a lockdown

**Files:**
- Modify: `crates/strata-zero/src/guards/links.rs` (`hops`)
- Test: same file's tests

**Interfaces:**
- Produces: `links::hops` no longer consults `sitenet::lift_answers`.

- [ ] **Step 1: Write the failing test** `on_alert_a_lift_still_carries_a_guard`: on `ward()`, a zone-1 guard on L, `crate::net::lock_down(&mut st, WARD)`; assert `hops(&st, post, &plan, 1, 0) == vec![1]` (link 1 is the lift; its 2F stop asks card 1).
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke on_alert_a_lift_still_carries` → FAIL (empty).
- [ ] **Step 3:** Drop the `lift_answers` check from `hops`; fix its doc comment ("a lift's rides to stops its card opens, alarm or not: a lockdown shuts lifts to intruders").
- [ ] **Step 4: Run** the test and `cargo test -p strata-zero --target-dir target/smoke guards::` → PASS (fix any test that asserted the old refusal by changing its expectation, saying why in a comment).
- [ ] **Step 5: Commit** `guards: lifts carry guards in a lockdown (#102)`.

### Task 2: Where the building last placed you

**Files:**
- Modify: `crates/strata-zero/src/net.rs` (`NetState`, its `new`/`Default`), `foes.rs` (`look`), `cameras.rs` (`tick`), `guards/hunt.rs` (`body_check`)
- Test: `net.rs` / `foes.rs` / `cameras.rs` / `hunt.rs` tests

**Interfaces:**
- Produces:
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)] pub struct Placed { pub level: i32, pub at: Pos, pub minute: u64 }` in `net.rs`
  - `NetState.placed: BTreeMap<BuildingId, Placed>` with `#[serde(default)]`
  - `pub const PLACED_MINUTES: u64 = 5;`
  - `pub(crate) fn place_you(st: &mut AppState, b: BuildingId, level: i32, at: Pos)` (stamps `st.net.now(st)`)
  - `pub(crate) fn placed(st: &AppState, b: BuildingId) -> Option<(i32, Pos)>` (None once older than `PLACED_MINUTES`)

- [ ] **Step 1: Write the failing tests**
  - `foes`: `a_guard_that_sees_you_places_you` — ward, guard on L sees you at (6,3) via `look_all`; `assert_eq!(crate::net::placed(&st, WARD), Some((0, Pos::new(6, 3))))`.
  - `cameras`: `a_floor_camera_places_you` — `office_copy(190)`, `in_a_floor_cameras_cone`, `next_minute`, `tick`; `placed(&st, b)` is `Some((level, you))`.
  - `net`: `a_placing_goes_stale_after_five_minutes` — `place_you` then advance the Net clock 5 minutes → still Some; 6 → None.
  - `hunt`: in the existing body-found test (or a new `finding_your_body_places_you`), `placed` is the body's level and tile.
- [ ] **Step 2: Run** them → FAIL (no `placed`).
- [ ] **Step 3: Implement** the type, field, constant and two functions; call `place_you` in `foes::look` when a posted guard on a `Place::Floor` sees you, in `cameras::tick` for each building a camera on a `Place::Floor` sees you (street cameras don't), and in `body_check` before `lock_down`.
- [ ] **Step 4: Run** the tests → PASS.
- [ ] **Step 5: Commit** `net: the building remembers where it last placed you (#102)`.

### Task 3: Calls and the call step

**Files:**
- Create: `crates/strata-zero/src/guards/call.rs`
- Modify: `foes.rs` (`Post.call`, `Post::new`, `lines`), `guards.rs` (`mod call`, consts, `tick` dispatch), `guards/hunt.rs` (`lose_you` clears `call`)
- Test: `guards/call.rs` tests, `foes.rs` `lines` test

**Interfaces:**
- Produces:
  - in `foes.rs`: `#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)] pub enum Call { Answer { level: i32, at: Pos }, Sweep { level: i32, stops: Vec<Pos> } }`; `Post.call: Option<Call>` `#[serde(default)]`
  - in `guards.rs`: `pub const SWEEP_STEP_TICKS: u64 = 2 * crate::net::WALK_TICKS; pub const SWEEP_PAUSE_TICKS: u64 = 30; pub const SWEEP_ROOMS: usize = 3;`
  - in `guards/call.rs`:
    - `pub(crate) fn sweep_stops(st: &mut AppState, i: usize, near: Pos) -> Vec<Pos>` (guard `i`'s own floor)
    - `pub(crate) fn sweep(st: &mut AppState, i: usize, near: Pos)` (sets `Call::Sweep` on the guard's level with `sweep_stops`, or no call when empty)
    - `pub(crate) fn call_step(st: &mut AppState, i: usize, now: u64)`

- [ ] **Step 1: Write the failing tests** in `guards/call.rs` (ward fixture; put the guard on alert with `crate::net::lock_down(&mut st, WARD)` and you on 2F out of the way unless said):
  - `sweep_stops_take_the_rooms_its_card_opens_nearest_first`: zone-2 guard on L at (3,3); `sweep_stops(&mut st, i, Pos::new(3,3))` has three stops; the first is in the west hall `Rect(1,1,14,5)`, the second in the east corridor, the third in the office; none equals (3,3).
  - `a_zone_0_guard_sweeps_only_the_hall`: zone-0 guard at (3,3) → one stop, in the west hall.
  - `a_sweeping_guard_checks_its_stops_then_walks_its_round`: zone-2 guard, `sweep(&mut st, i, its pos)`; `run_until` the call is None; record that it stood on each stop (check positions each tick); afterwards its mode is Idle and `round_step` runs (it heads back to its patrol).
  - `an_answering_guard_rides_up_runs_to_the_spot_and_sweeps`: you on 2F at (28,3) unseen behind the cloak (`cloak::testutil::stealthy` + `fire("cloak")`) or out of sight; zone-1 guard on L; `call = Some(Call::Answer { level: 1, at: Pos::new(20,3) })`; `run_until` it's on `on(1)` and at (20,3); then its call is `Some(Call::Sweep { level: 1, .. })`.
  - `a_call_ends_with_its_alert`: guard sweeping; clear the alert (`st.net.trace(WARD).alert_until = None`, heat 0); one `call_step` → call None.
  - `foes::lines_read_answering_and_sweeping`: a calm posted guard with each call reads `▣ guard · answering · …` / `▣ guard · sweeping · …`.
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke guards::call` → FAIL (no module).
- [ ] **Step 3: Implement.**
  - `sweep_stops`: rooms of the guard's floor (`st.interiors.floor(b, level)`) with `room.zone <= post.zone` and, for a room with a door, `guards::opens` for that door; sort by `near.chebyshev(rect centre)` then room order; per room the walkable tile nearest the centre the guard can reach (`links`'s `walk`-style A* with `foes::can_step`, made `pub(super)` or duplicated small); drop a stop equal to the guard's tile; take `SWEEP_ROOMS`.
  - `call_step` (guard calm, has a call): building not on alert (`st.net.alert(b, st.net.now(st))` false) → `call = None`, `round::wait(now + GUARD_STEP_TICKS)`. A `round.lift` wait → `links::wait_for_lift`. `Answer` off `level`: `links::link_to(st, i, level)`; None → call None; at the stop → `links::take`; else `round::step` toward it, waiting `GUARD_RUN_TICKS`. `Answer` on `level`: at `at`, `NoWay`, or `Held` beside `at` (chebyshev ≤ 1) → `sweep(st, i, at)`; `Held` elsewhere or `NoMap` waits; `To` waits `GUARD_RUN_TICKS`. `Sweep`: off `level` → None; empty → None and back to round; on the first stop → pop it, wait `SWEEP_PAUSE_TICKS`; else step: `NoWay` pops, `Held`/`NoMap` waits, `To` waits `SWEEP_STEP_TICKS`.
  - `guards::tick`: `if f.hunting() { hunt_step } else if has_call { call::call_step } else { round_step }`.
  - `lose_you` sets `post.call = None`.
  - `foes::lines`: Idle with `Some(Call::Answer{..})` → `answering`, `Some(Call::Sweep{..})` → `sweeping`.
- [ ] **Step 4: Run** the tests → PASS; also `cargo test -p strata-zero --target-dir target/smoke guards::` and `foes::`.
- [ ] **Step 5: Commit** `guards: calls to answer and to sweep (#102)`.

### Task 4: The answer

**Files:**
- Modify: `net.rs` (`alarm_goes_off`, `heat_on`, `lock_down`), `helix/bin.rs` (key plant's `Alarm`), `guards.rs` (`pub(crate) use call::answer`), `guards/call.rs` (`answer`)
- Test: `guards/call.rs` tests

**Interfaces:**
- Consumes: Task 2's `net::placed`, Task 3's `Call`, `sweep`.
- Produces: `pub fn alarm_goes_off(st: &mut AppState, b: BuildingId) -> String` in `net.rs`; `pub(crate) fn answer(st: &mut AppState, b: BuildingId)` in `guards/call.rs`.

- [ ] **Step 1: Write the failing tests** (ward3 for three levels; guards on several floors):
  - `a_radio_call_in_a_fight_brings_the_floor_in`: you on L seen by guard A (fight on L via `look_all` + `engage`), calm guard B on L far off; `lock_down(WARD)` → B is `Hunting { seen: (6,3) }`; `seat_foes` seats B; log has `guards close in`.
  - `a_camera_alarm_sends_the_floor_calm`: no fight; `place_you(WARD, 0, (6,3))`; `lock_down` → calm guard on L has `Call::Answer { level: 0, at: (6,3) }`, mode Idle; no fight engaged.
  - `two_guards_answer_from_other_floors_nearest_first`: ward3, you placed on L; calm guards on 2F (two) and 3F (one) → the two on 2F answer (fewer hops), the 3F one keeps no call.
  - `a_net_alarm_with_no_spot_sweeps_every_floor`: no placing; guards on L and 2F → each has `Call::Sweep` on its own level; log `guards sweep the floors`.
  - `a_repeat_alarm_answers_nothing`: `lock_down` twice; clear the calls between; after the second, no guard has a call.
  - `an_alarm_while_youre_out_leaves_no_calls`: you on the street (`leave`), `lock_down` → no calls.
  - `heat_over_the_line_answers`: `crate::net::noise_on(&mut st, WARD, 60)` with you inside and a placing → the floor's guard answers.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement.** `alarm_goes_off` = `raise_alarm` then `guards::answer`, returning the line. `lock_down`: read `was = st.net.alert(b, now)` first; call `alarm_goes_off` when `!was`, else `raise_alarm`. `heat_on`'s `Alarm` and `!was_alert` `Dump` call `alarm_goes_off`. HELIX's plant `Alarm` likewise. `answer`: return unless `venues::inside(st, b)`; calm standing guards of `b`; with `placed` → on the spot's level: engaged there (`st.net.engaged.place == Floor{b, level}`) → `Hunting{seen: at, lost: 0}`, else `Call::Answer`; other levels: rank by fewest hops (reuse `links::hops_to`, made `pub(super)`) then index, skipping unreachable, take 2, `Call::Answer`; warn `guards close in` if any. Without → each `sweep(st, i, its pos)`; warn `guards sweep the floors` if any got stops.
- [ ] **Step 4: Run** the new tests, then `cargo test -p strata-zero --target-dir target/smoke net:: helix:: sitenet:: venues::` → PASS.
- [ ] **Step 5: Commit** `guards: the alarm brings them, once, while you're in (#102)`.

### Task 5: Losing you on alert sweeps

**Files:**
- Modify: `guards/hunt.rs` (`lost_you`, `hunt_step`/`run_to` give-ups), `foes.rs` (`step`'s `LOST_TURNS`), `guards.rs` (export)
- Test: `guards/hunt.rs` tests, `cloak/vanish.rs` tests

**Interfaces:**
- Produces: `pub(crate) fn lost_you(st: &mut AppState, i: usize, now: u64)`: `lose_you`, then on a posted guard on a floor of a building on alert, `call::sweep(st, i, its pos)`.

- [ ] **Step 1: Write the failing tests:**
  - `on_alert_a_guard_that_loses_you_sweeps`: ward, alert on, guard hunting toward (6,3) with you gone up to 2F unseen; run until it's calm → its call is `Call::Sweep { level: 0, .. }`.
  - `off_alert_it_just_walks_its_round`: same with no alert → call None.
  - `in_a_fight_a_guard_lost_on_alert_sweeps`: engaged on L, alert on, cloaked you; step the guard's turns until calm (`foes::step` `LOST_TURNS`) → `Call::Sweep`.
  - `a_guard_mid_call_that_spots_you_never_resumes_it`: guard with `Call::Answer` sees you (`look_all`), then loses you → call is a fresh `Sweep` near where it lost you, not the old `Answer`.
  - `vanish`: `vanish_leaves_no_sweeps` — alert on, fight, Vanish → no guard has a call.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement** `lost_you`; use it in `hunt_step` where it reaches `seen`/`seen` held, in `run_to`'s `NoWay` and `LOST_TURNS`; in `foes::step`'s `LOST_TURNS` branch for a foe with a post (others stay plain Idle). Leave `lose_you` for Vanish, leaving the building, and a card that won't ride.
- [ ] **Step 4: Run** `cargo test -p strata-zero --target-dir target/smoke guards:: foes:: cloak::` → PASS.
- [ ] **Step 5: Commit** `guards: on alert, a guard that loses you sweeps (#102)`.

### Task 6: Ghost under a camera

**Files:**
- Modify: `net.rs` (`NetState::minute` gains `watched: &BTreeSet<BuildingId>`; every caller), `clock.rs` (collect watched), `cameras.rs` (`pub fn watching(st) -> BTreeSet<BuildingId>`)
- Test: `cameras.rs` tests, `net.rs` minute tests

**Interfaces:**
- Produces: `cameras::watching(st: &AppState) -> BTreeSet<BuildingId>` (the buildings of `seeing(st)`); `NetState::minute(&mut self, now, inside, constructs, roots, watched)`.

- [ ] **Step 1: Write the failing tests:**
  - `cameras`: `under_a_camera_the_trace_doesnt_cool` — `office_copy(190)`, no Ghost, heat set to 10, in the cone; run one `crate::clock::run` minute (or call `minute` with `watching(&st)` then `tick`) → heat 12. `ghost_halves_what_a_camera_costs` — with Ghost, the same → 11.
  - `net`: `a_watched_trace_doesnt_cool_that_minute` — `minute(now, None, …, &BTreeSet::from([b]))` leaves heat; empty set cools by 1; an alert ending this minute still ends when watched.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement.** In `minute`, skip `t.cool(m, backdoor)` when `m == now && watched.contains(b) && !ending`. In `clock::run`, `let watched = if now > st.net.last_minute && !crate::outside::away(st) { crate::cameras::watching(st) } else { BTreeSet::new() };` (make `last_minute` readable if private). Update other `minute` callers (tests, `when.rs`, catch-up) with an empty set.
- [ ] **Step 4: Run** `cargo test -p strata-zero --target-dir target/smoke cameras:: net:: clock::` → PASS.
- [ ] **Step 5: Commit** `net: a trace doesn't cool under a camera, so Ghost halves it (#102)`.

### Task 7: Manual and the checks

**Files:**
- Modify: `manual/07-turns-and-fights.md` ("Fights indoors", after the radio paragraph; the Vanish paragraph), `manual/08-the-net.md` (the camera-heat line at ~95)

- [ ] **Step 1:** Write the 07 paragraph per the spec's "The manual" (alarm brings the floor, two more, sweeping rooms, a Net alarm sweeps floors and can find your body, lifts carry guards, footer `answering`/`sweeping`) and add to Vanish "they go back to their rounds". In 08: "While a camera sees you, the trace doesn't cool."
- [ ] **Step 2: Run** the five checks of CLAUDE.md. All green.
- [ ] **Step 3: Commit** `manual: guards answer the alarm; the trace under a camera` with `Closes #102`.
