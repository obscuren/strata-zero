# Wait and sleep Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Sleep in your own bed through a timeline window (that also serves Wait), with game time running fast and for real while you rest, and Castellan's morning call firing on elapsed time asleep or awake.

**Architecture:** The explore frame's clock step (world, paths, bots, the Net's minute, backdoors, jobs, the story) moves into `clock::run`, called by the frame with its ticks. A rest is state in `AppState` (`rest::Rest`), like a scene or a call: while it is set the explore frame calls `rest::frame` instead, which runs the clock a game minute at a time, four a frame, checking what wakes you after each. The choosing window is a pushed screen (`screens/rest.rs`); asleep, the explore screen draws the same window over the map with the flat dark.

**Tech Stack:** Rust, ratatui, crossterm `KeyEvent`, the `sim` world, the `missions` module, RON content.

**Spec:** `docs/superpowers/specs/2026-11-01-wait-and-sleep-design.md` (read "Calls I made" and any builder rulings first; section numbers below are the spec's).

## Global Constraints

- Time asleep is game time: the world's tick moves one game minute (`sim::TICKS_PER_MINUTE`, 90 ticks) at a time through `clock::run`; never a reseed, never a skipped or repeated minute. `rest::MINUTES_PER_FRAME` is 4.
- A rest's times are absolute game minutes (`st.net.now(st)`), never minutes of the day; labels wrap at 24 hours.
- Colours are exact `Color::Rgb`: the strip's past `Rgb(35, 38, 45)`, night `Rgb(23, 33, 58)`, day `Rgb(61, 51, 24)`, sleep `Rgb(0, 165, 184)`; `CYAN`, `DIM`, `FAINT`, `TEXT`, `PANEL_BG` from `ui::chrome`. Draw only through ratatui; read keys only as crossterm `KeyEvent`s.
- No new glyph beyond `▴ ▤ ☾ █ z Z` and the box-drawing already in use; `every_drawn_character_has_a_glyph` (`src/window/font.rs`) must pass.
- Fits 100×30 (`MIN_W`/`MIN_H`) and 120×40.
- No mission, step or role id in non-test Rust.
- Build only with `--target-dir target/smoke` (in this worktree), never the default `target/`.
- Each task ends with the five checks of `CLAUDE.md` green (fmt; clippy without and with `--features strata-zero/window`; tests without and with it), then one commit, message `rest: <what>` (or `story: <what>` for content), ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Match the surrounding code: its comment density, naming, test style (`#[cfg(test)] mod tests` at the file's end).

## Review Focus

1. **Something already waking you as you lie down** (a call ringing, a scene queued): you wake at once, no minute run. Test: Task 2 `a_call_up_as_you_lie_down_wakes_you_at_once`.
2. **Keys while resting**: arrows, `wasd`, Tab, digits, `q`, space must not move you, open anything or pause; `esc` wakes; the dev console's `:` still opens. Test: Task 2 `keys_do_nothing_but_esc`.
3. **A new city or a load while resting**: nothing of the rest survives. Test: Task 2 `a_new_city_forgets_the_rest`.
4. **The default wake time at the lamps' edges and across midnight**: 05:30 gives 07:00, 06:30 gives 14:30, 23:50 gives 07:00 the next day. Test: Task 2 `the_default_wake_time`.
5. **The window at 100×30**: inside the 59-wide map, labels not overlapping when now and the wake time are close. Test: Task 3 `the_window_fits_100x30` and `close_labels_do_not_touch`.

---

### Task 1: The city's clock as one step

**Files:**
- Create: `crates/strata-zero/src/clock.rs`
- Modify: `crates/strata-zero/src/screens/explore.rs` (the frame step around the "The city lives" comment, ~lines 2675–2700), the crate root that declares modules (`main.rs` or `lib.rs`, wherever `mod missions;` is)
- Test: `clock.rs` tests

**Interfaces:**
- Produces: `pub const PATHS_PER_FRAME: usize = 4;` and `pub fn run(st: &mut AppState, world_ticks: Option<u64>, paths: usize)`: with `Some(n)` the world advances `n` ticks and turns up to `paths` requests into trips (`world.advance(city, world.tick() + n); world.path_some(city, paths)`); with `None` the world holds (the free camera). Then, either way, as the frame does now: `crate::bots::tick`, the Net's minute (`st.net.minute(now, inside, &mut st.constructs, &roots)`), `crate::net::tick_backdoors`, `crate::jobs::tick`, `crate::missions::tick`.
- The explore frame keeps what is its own: `world.set_blocker(standing)` before the call, the `before` position captured before the call and compared after it, everything after `missions::tick`.

- [ ] **Step 1: Write the failing tests** in `clock.rs`:
  - `run_moves_the_world_and_the_story`: a home state (`crate::jobs::testutil::state_home()`) with a book whose mission's second step is `After(minutes: 1)` and logs `"a minute"` (build it the way `missions` tests build small books); `clock::run(&mut st, Some(sim::TICKS_PER_MINUTE), PATHS_PER_FRAME)` twice moves `st.clock_tick()` by 180 and the log has `"a minute"` once.
  - `none_holds_the_world`: `clock::run(&mut st, None, PATHS_PER_FRAME)` leaves `st.clock_tick()` as it was.
- [ ] **Step 2: Run them**: `cargo test -p strata-zero --target-dir target/smoke clock::` — fails to compile (no `clock`).
- [ ] **Step 3: Implement `clock::run`** by moving the code out of the explore frame unchanged in order; the frame calls `crate::clock::run(st, (self.mode == Mode::Walk).then_some(elapsed), crate::clock::PATHS_PER_FRAME)` where it ran that code.
- [ ] **Step 4: Run** the new tests and the whole suite (explore's tests must stay green: the frame behaves as before).
- [ ] **Step 5: The five checks, then commit** `rest: the city's clock as one step`.

### Task 2: Resting: time running fast, and waking

**Files:**
- Create: `crates/strata-zero/src/rest.rs`
- Modify: `crates/strata-zero/src/app.rs` (`AppState.rest`, its default, and wherever a new city resets the story's state, e.g. beside `missions` in `enter_city`), `screens/explore.rs` (keys and the frame step), the crate root (`mod rest;`)
- Test: `rest.rs` tests

**Interfaces:**
- Consumes: `clock::run` (Task 1).
- Produces (`pub`):
  - `enum Kind { Sleep, Wait }` (`Clone, Copy, Debug, PartialEq, Eq`)
  - `struct Rest { pub kind: Kind, pub from: u64, pub until: u64, hp: u32, visitors: Vec<String> }` (game minutes; `hp` and `visitors` as you lay down)
  - `enum Why { Time, Esc, Fight, Hurt, Call, Scene, Feed, Visitor }`
  - `const MINUTES_PER_FRAME: u64 = 4;` `const FRAME_BUDGET: Duration = Duration::from_millis(16);` `const NIGHT: u64 = 360;` (minutes for a full night); a minute asleep paths `clock::PATHS_PER_FRAME * sim::TICKS_PER_MINUTE as usize` (360) requests, what a minute awake allows
  - `fn now(st: &AppState) -> u64` (the game minute, `st.net.now(st)`)
  - `fn default_wake(st: &AppState, kind: Kind) -> u64`: Sleep in the lamps' hours (`citygen::lighting::lamps_on(st.minute_of_day())`) the next 07:00, by day now + 480; Wait now + 60; then on a ten (minute of the day rounded up to ten).
  - `fn clamp(st: &AppState, wake: u64) -> u64`: within `now + 10 ..= now + 1200`.
  - `fn start(st: &mut AppState, kind: Kind, until: u64)`: sets `st.rest`, then wakes at once if a cause in spec §4 (1, 3, 4, 5) already holds.
  - `fn frame(st: &mut AppState)`: `frame_within(st, FRAME_BUDGET)`.
  - `fn frame_within(st: &mut AppState, budget: Duration)`: up to `MINUTES_PER_FRAME` whole minutes, stopping after a minute once `budget` has passed since it began (always at least one): each `clock::run(st, Some(sim::TICKS_PER_MINUTE), 360)`, then the checks of spec §4 in order; the first that holds calls `wake`. Tests call it with a long budget (`Duration::from_secs(60)`) so the count is exact.
  - `fn key(st: &mut AppState, k: &KeyEvent) -> bool`: false when not resting; `esc` wakes (`Why::Esc`); every key returns true.
  - `fn wake(st: &mut AppState, why: Why)`: clears `st.rest`; Sleep heals HP and FOCUS by `ceil(max × min(1, slept / NIGHT))` each, capped (`Character::hp_max` / `focus_max` with `&st.content`); one log line (`st.say`) as spec §4 words it.
- In explore: in the key loop, after the scene's handling and before the call's, `if !console && crate::rest::key(st, k) { continue; }`; in the frame step, when `st.rest.is_some()` call `crate::rest::frame(st)` in place of `crate::clock::run(..)` (the rest of the frame runs as usual).

- [ ] **Step 1: Write the failing tests** in `rest.rs` (a home state at night; for other hours use a helper that advances the world with `clock::run` to the minute wanted, or the console's `clock` for a reseed where the test does not care about the sim):
  - `the_default_wake_time`: Sleep at 22:01 → the minute of 07:00 next day; 05:30 → 07:00 the same day; 06:30 → 14:30; 13:00 → 21:00; 23:50 → 07:00 next day; Wait at 22:01 → 23:10 (an hour on, up to a ten).
  - `the_wake_time_clamps`: `clamp` of now + 1 is now + 10; of now + 2000 is now + 1200.
  - `time_passes_four_minutes_a_frame`: start Sleep until now + 30; after one `frame_within(st, 60 s)` the clock is 4 minutes on; after 8 such frames 30 minutes on exactly (no overshoot), `st.rest` is `None`, and the log's last line is `You wake at hh:mm.` with that time.
  - `a_spent_budget_still_runs_a_minute`: `frame_within(st, Duration::ZERO)` moves the clock exactly one minute.
  - `an_after_step_fires_on_the_same_minute_asleep`: two states with a mission step `After(minutes: 30)` that logs; one runs `clock::run(st, Some(TICKS_PER_MINUTE), PATHS_PER_FRAME)` 30 times, the other rests until now + 120 and runs `frame_within(st, 60 s)` until awake; the step logged at the same game minute in both (record `st.net.now(st)` when the line appears).
  - `a_call_wakes_you_ringing`: an `After(minutes: 10)` step rings a `Call`; resting until now + 120 you wake at that minute with `st.missions.call` up and not answered, and it is not on voicemail (no mail from it).
  - `a_call_up_as_you_lie_down_wakes_you_at_once`: a call up, `start` leaves `st.rest` `None` and the clock unmoved.
  - `each_cause_wakes_you`: a fight (`st.net.engaged = Some(..)` via `crate::net::engage`), HP lowered below the start, a scene queued (`st.missions.scenes`), a feed queued, a visitor arriving in the flat (`missions::visitors::arrive`): each wakes with its own line.
  - `keys_do_nothing_but_esc`: through the explore screen (`MapScreen::walking`) while resting, `l`, `Up`, `Tab`, `1`, `q`, space change nothing (position, stack, rest); `esc` wakes; with `st.unlock_dev()`, `:` opens the console.
  - `sleep_heals_and_wait_does_not`: 6 hours from 10 HP refills to max; 3 hours from 10/45 ends at 33; FOCUS the same; Wait for 3 hours leaves both.
  - `a_new_city_forgets_the_rest`: resting, a new city (the same call the visitors test uses) leaves `st.rest` `None`.
- [ ] **Step 2: Run them**: fail to compile.
- [ ] **Step 3: Implement** `rest.rs`, `AppState.rest` (not saved; check `save.rs` writes fields explicitly and add nothing there), the reset, the explore hooks.
- [ ] **Step 4: Run** the tests and the whole suite.
- [ ] **Step 5: The five checks, then commit** `rest: time runs fast while you rest, and things wake you`.

### Task 3: The Rest window, the bed and the console's `wait`

**Files:**
- Create: `crates/strata-zero/src/screens/rest.rs`
- Modify: `rest.rs` (the window's drawing), `screens/mod.rs`, `interact.rs` (the bed's subject), `screens/popup.rs` (`Pick::Sleep`), `screens/console.rs` (`wait`)
- Test: `rest.rs` and `screens/rest.rs` tests, `interact.rs` tests, `console.rs` tests

**Interfaces:**
- Consumes: `rest::{Kind, default_wake, clamp, start, now}` (Task 2).
- Produces:
  - `rest::window_rect(map: Rect) -> Rect`: 57×12, centred across `map`, bottom row on the map's last row (clipped to `map` when it is narrower).
  - `rest::draw_window(buf: &mut Buffer, map: Rect, st: &AppState, kind: Kind, until: u64, asleep: Option<u64>)`: spec §2 (choosing, `asleep` `None`) and §3 (asleep, `Some(from)` with the time now from `st`): the frame (`chrome::frame`), the title as the popup draws a header (`▤ Sleep` / `☾ Wait`, `Asleep` / `Waiting` when asleep), the hour labels, the strip, the markers and labels, the line under it, the keys line.
  - `screens::rest::RestScreen { kind: Kind, until: u64 }` with `pub fn open(st: &AppState, kind: Kind) -> Self` and `impl Screen`: `h`/`Left` −60, `l`/`Right` +60, `H` −10, `L` +10 (all through `clamp`); `Enter` → `rest::start` and `ScreenCmd::Pop`; `Esc` → `ScreenCmd::Pop`. Draws over the washed stack with `explore::layout(f.area(), false).map`.
  - `Pick::Sleep` (popup) → `ScreenCmd::Replace { to: Box::new(RestScreen::open(st, Kind::Sleep)), fx: TransitionKind::None }`.
  - `interact::gather`: the bed subject of spec §1.
  - console `wait` (dev): pushes `RestScreen::open(st, Kind::Wait)`; in the command list, completion and `help`.

- [ ] **Step 1: Write the failing tests**:
  - `rest.rs` `the_strip_at_night`: drawn at 22:01 until 07:00 into a 120×40 buffer over the map rect: the labels row starts `20  22  00`; strip cells 0–3 past `Rgb(35, 38, 45)`, 4–21 sleep `Rgb(0, 165, 184)`, 22 day `Rgb(61, 51, 24)` (07:00), the last cells (18:00 on) night `Rgb(23, 33, 58)`; `▴` under cell 4 and cell 22; the line reads `8h 59m asleep · you wake rested`.
  - `rest.rs` `asleep_the_strip_fills`: at 03:12 of the same rest, cells from 4 up to 03:00's are `CYAN`, the rest of the sleep `Rgb(0, 165, 184)`; the line reads `5h 11m slept · 3h 48m to go`; the keys line `esc wakes you`; the title reads `Asleep`.
  - `rest.rs` `the_window_fits_100x30` and at 120×40: `window_rect` lies inside the map rect of `explore::layout`.
  - `rest.rs` `close_labels_do_not_touch`: now 22:01, wake 22:20: both labels drawn whole with a space between.
  - `screens/rest.rs` `keys_move_the_wake_time`: `l` +60, `h` −60, `L` +10, `H` −10, clamped; `Esc` pops with `st.rest` `None`; `Enter` pops with `st.rest` set to the shown `until`.
  - `interact.rs` `the_bed_offers_sleep`: standing on `home.bed_side`, `gather` has a subject named `Bed` whose choices include `Interact: Sleep`; standing on the rig's side, none named `Bed`.
  - `console.rs` `wait_opens_the_wait_window` (dev): `wait` returns a push of the window (follow how the console tests check a pushed screen, or that the result names the window).
- [ ] **Step 2: Run them**: fail.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests and the whole suite.
- [ ] **Step 5: The five checks, then commit** `rest: the timeline window, the bed and the console's wait`.

### Task 4: Asleep on the map

**Files:**
- Modify: `screens/explore.rs` (draw), `missions/visitors.rs` (the flat's lights while asleep), `rest.rs` (the z's), `screens/shots.rs` (`rest_shots`)
- Test: `rest.rs` / `explore.rs` tests; `window/font.rs` `every_drawn_character_has_a_glyph` stays green

**Interfaces:**
- Consumes: `rest::draw_window` (Task 3), `st.rest` (Task 2).
- Produces: while `st.rest` is set, the explore frame draws, after the visitors and before the HOLO call card: (Sleep only) the flat at `min(story lights, 0.4)` and the z's of spec §3; then `rest::draw_window(.., asleep: Some(rest.from))`. Waiting keeps the flat's lights and draws no z's.

- [ ] **Step 1: Write the failing tests**:
  - `asleep_the_flat_is_dark_and_you_have_zs`: resting (Sleep) beside the bed, drawn at 120×40: a floor cell of the flat is darker than drawn awake; `z` one up-right of your `@` in `Rgb(0, 165, 184)`, `Z` two up two right in `CYAN` bold; the window's title row reads `Asleep`.
  - `waiting_keeps_the_lights`: resting (Wait), the flat's floor cell as awake and no `z`.
  - `asleep_fits_100x30`: the window inside the map, the `@` not covered.
- [ ] **Step 2: Run them**: fail.
- [ ] **Step 3: Implement**, and add the ignored shot test `rest_shots` in `screens/shots.rs`: after the house call (as `scene_bubbles_shots` builds its state, both scenes skipped), beside the bed: `rest_choose` (the window pushed over the map) and `rest_asleep` (asleep at 03:12, reached with `rest::frame_within` and a long budget), each at 120×40 and 100×30.
- [ ] **Step 4: Run** the tests and the whole suite, and the shots: `SHOT_DIR=<dir> cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored rest_shots`; look at the PNGs.
- [ ] **Step 5: The five checks, then commit** `rest: asleep on the map`.

### Task 5: Castellan's morning call

**Files:**
- Modify: `flavor/missions.ron` (`house_call`, spec §6), `missions/content.rs` tests (counts that name `house_call`'s steps), `missions/mod.rs` tests
- Test: `missions/mod.rs` tests

**Interfaces:**
- Consumes: `rest::{start, frame_within, Kind}`, `clock::{run, PATHS_PER_FRAME}`, the house call helpers in `missions::tests` (`at_your_door`, `scene::tests::press`).

- [ ] **Step 1: Write the failing tests** in `missions/mod.rs`:
  - `castellan_calls_in_the_morning_awake`: after the house call (both scenes skipped) and Mezz's mail, run `clock::run(st, Some(TICKS_PER_MINUTE), PATHS_PER_FRAME)` minute by minute: the call from Castellan comes up exactly 480 game minutes after the `lay_low` step fired (its `since`), the house call still open with its "Sleep" objective until then; its first line fills `{you}`; answering through every line and hanging up leaves the house call done.
  - `castellan_wakes_you_in_the_morning`: the same, asleep from just after the mail until 20 hours on: you wake by the call (`A call wakes you at hh:mm.`) at the same game minute, the call up and ringing.
- [ ] **Step 2: Run them**: fail (no such step).
- [ ] **Step 3: Add the step** of spec §6 to `house_call` after `lay_low`; update any test that counts `house_call`'s steps.
- [ ] **Step 4: Run** the tests and the whole suite.
- [ ] **Step 5: The five checks, then commit** `story: Castellan calls in the morning`.
