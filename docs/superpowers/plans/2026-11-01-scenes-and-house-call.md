# Scenes and the house call Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A scene engine for story missions (beats with timing in data, marks, visitors) with two looks, film and bubbles, and the house call that ends the drop.

**Architecture:** Scenes are data in `flavor/missions.ron` beside the missions, validated at load. A `Scene` effect queues one; `missions/scene.rs` runs it on the clock tick, takes the keys while it runs and hands the step `Ended` when it is over. The explore screen draws the film instead of its frame, or the bubbles over it. Visitors are story-placed people kept in `Missions`, drawn, looked at and blocking.

**Tech Stack:** Rust, ratatui, crossterm `KeyEvent`, serde + RON, the existing `missions` module.

**Spec:** `docs/superpowers/specs/2026-11-01-scenes-and-house-call-design.md` (read its "Calls I made" and builder rulings first; section numbers below are the spec's).

## Global Constraints

- Every timing of a scene is data: no tick, second or characters-per-second value for a scene in Rust. Seconds in data become clock ticks as `max(1, round(s * 30.0))`.
- Scenes run on the clock tick (`st.clock_tick()`), never the frame tick.
- No mission, step, role or scene id appears in non-test Rust.
- Colours are exact `Color::Rgb`. Draw only through ratatui; read keys only as crossterm `KeyEvent`s.
- New glyphs are limited to `▾ ▴ ╭ ╮ ╰ ╯ ▌` (and `·`, `@`, box-drawing already in use); `every_drawn_character_has_a_glyph` (`src/window/font.rs`) must pass.
- Fits 100×30 (`MIN_W`/`MIN_H`) and 120×40.
- Build only with `--target-dir target/smoke` (in this worktree), never the default `target/`.
- Each task ends with the five checks of `CLAUDE.md` green (fmt; clippy without and with `--features strata-zero/window`; tests without and with it), then one commit, message `scenes: <what>` (or `story:` for content), ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Match the surrounding code: its comment density, naming, test style (`#[cfg(test)] mod tests` at the file's end).

## Review Focus

1. **A scene started away from home** (`story scene house_call` on the street, or a city with no couch): marks that don't resolve must never stall it; actors put on goals; the scene ends. Test: Task 4 `a_scene_off_home_never_stalls`.
2. **100×30**: a three-line subtitle and a bubble on a 59×20 map must stay inside the frame. Tests: Task 5 `film_fits_100x30`, Task 6 `bubbles_fit_100x30`.
3. **Keys during a scene**: arrows, `wasd`, Tab, digits, `q` must not move, open the deck or pause; the HOLO call's `space` must not steal a scene's `space`. Test: Task 4 `keys_do_nothing_but_space_enter_esc`.
4. **A new city or a load mid-scene** (`enter_city` resets `Missions`): no scene, visitors or dim lights survive. Test: Task 2 `a_new_city_forgets_visitors_and_lights` and Task 4 `a_new_city_ends_the_scene`.
5. **Things queued behind a scene**: a feed queued during a scene opens after it; a call rings after it. Test: Task 4 `feeds_and_calls_wait_for_the_scene`.

---

### Task 1: Scene content types and validation

**Files:**
- Modify: `crates/strata-zero/src/missions/content.rs`
- Test: same file's `tests`

**Interfaces:**
- Produces (all `pub`, `Clone, Debug, Deserialize`, `PartialEq` where the existing types have it):
  - `enum Look { Film, Bubbles }`
  - `struct Pace { #[serde(rename = "type")] pub chars: f32, pub walk: f32, pub hold: Option<f32> }`
  - `struct Scene { pub title: String, pub look: Look, pub pace: Pace, pub beats: Vec<Beat> }`
  - `enum Mark { You, Role(String), HomeDoor, HomeOutside, HomeInside(u32), HomeRig, HomeBed, HomeFurniture(citygen::Furniture), Beside(Box<Mark>) }`
  - `enum PadCode { Journal, Code(u16) }`, `enum PadResult { Denied, Open }`
  - `enum Beat { Say { who: String, text: String, #[serde(default, rename = "as")] label: Option<String>, #[serde(default)] at: Option<Mark>, #[serde(default)] hold: Option<f32> }, Stage(String), Walk { who: String, to: Mark, #[serde(default)] pace: Option<f32>, #[serde(default = "yes")] wait: bool }, Wait(f32), Keypad { door: Mark, code: PadCode, result: PadResult, keys: f32, hold: f32 }, Door { at: Mark, open: bool }, Camera(Mark), Arrive { role: String, at: Mark }, Leave(String), Lights(f32) }`
  - `Pick::Visitor`; `Role` gains `#[serde(default)] tint: Option<(u8, u8, u8)>` and `#[serde(default)] look: Option<String>`
  - `Effect::Scene(String)`, `Effect::Arrive { role: String, at: Mark }`, `Effect::Leave(String)`, `Effect::Lights(f32)`
  - `Where::HomeDoor`
  - `Book` gains `#[serde(default)] pub scenes: BTreeMap<String, Scene>`
  - `Book::is_visitor(&self, role: &str) -> bool`
  - `Book::needs_cast(&self, m: &Mission) -> BTreeSet<String>`: `m.roles()` less visitors and `"you"`
  - `pub const YOU: &str = "you";`
  - `BookError` gains `UnknownScene { place: String, scene: String }`, `NotAVisitor { place: String, role: String }`, `BadNumber { place: String, what: String }`, `ReservedRole(String)`, `EmptyScene(String)`
- Consumes: the existing `Book::validate`, `BookError`, slot checking.

- [ ] **Step 1: Write the failing tests** in `content.rs` tests, each parsing a small RON book (reuse the module's existing test helper that builds a book from a string, or add `fn book(ron: &str) -> Result<Book, BookError>`):
  - `scenes_parse_with_every_beat`: a book with one scene using all ten beats parses and validates; `book.scenes["s"].beats.len() == 10`; a `Walk` without `wait` has `wait == true`; `Say` with `as: Some("A voice")` has `label == Some("A voice".into())`.
  - `an_unknown_scene_is_refused`: `Scene("nope")` → `Err(BookError::UnknownScene { .. })`.
  - `a_visitor_takes_no_place_slot`: a visitor role and `Log("{castellan.place}")` → `Err(BookError::UnknownSlot { .. })`.
  - `no_role_is_called_you`: a cast key `"you"` → `Err(BookError::ReservedRole(_))`.
  - `walks_and_arrivals_need_visitors`: `Arrive(role: "marlo", ..)` where marlo is a `Worker` → `Err(BookError::NotAVisitor { .. })`; `Walk(who: "you", ..)` is fine.
  - `numbers_must_be_positive`: `Pace(type: 0, ..)`, `Wait(-1.0)`, `Lights(1.5)` and `HomeInside(0)` each → `Err(BookError::BadNumber { .. })`.
  - `an_empty_scene_is_refused`: `beats: []` → `Err(BookError::EmptyScene(_))`.
  - `ended_may_name_a_scene_step`: `Step(id: "a", .., then: [Scene("s")])` then `Ended("a")` validates.
  - `you_is_a_slot`: `Log("{you}")` validates.
  - `visitors_need_no_cast`: `book.needs_cast(&mission)` leaves out a visitor `from` role and `you`.
  - `the_shipped_book_still_validates` (exists; must stay green).

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke missions::content`
Expected: compile errors (new types missing).

- [ ] **Step 3: Implement** the types and the checks in `Book::validate` (§1 "Validation"). `Mission::roles()` must leave out `YOU`. Text slot checking accepts `{you}`; for visitor roles only `{role}` and `{role.title}`. `Ended(step)` accepts a step whose effects include `Effect::Scene`. A number check names its place like the other errors (`"scene house_call beat 3"`, `"mission x step y"`).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p strata-zero --target-dir target/smoke missions::content`
Expected: PASS. Then the five checks.

- [ ] **Step 5: Commit** `scenes: content types, marks and validation`.

---

### Task 2: Casting, visitors, marks, lights and `HomeDoor`

**Files:**
- Create: `crates/strata-zero/src/missions/marks.rs`, `crates/strata-zero/src/missions/visitors.rs`
- Modify: `missions/mod.rs` (state, `begin`, `Trigger::fires` for `At(HomeDoor)`), `missions/cast.rs`, `missions/effects.rs`, `track/mod.rs` (`resolve_story`, its test), `screens/explore.rs` (draw visitors and lights, `bump` blocking), `look.rs` (visitor card)
- Test: each file's `tests`

**Interfaces:**
- Consumes (Task 1): `Mark`, `Pick::Visitor`, `Role.tint/look`, `Effect::{Arrive, Leave, Lights}`, `Where::HomeDoor`, `Book::needs_cast`, `Book::is_visitor`.
- Produces:
  - `Missions` gains `pub visitors: Vec<visitors::Visitor>`, `pub lights: Option<f32>` (`None` = normal), `pub scenes: VecDeque<(usize, usize, String)>` (queued by `Effect::Scene`; consumed in Task 4).
  - `visitors::Visitor { pub role: String, pub place: Place, pub pos: Pos }`
  - `visitors::arrive(st: &mut AppState, role: &str, at: (Place, Pos))` (moves an existing one), `visitors::leave(st: &mut AppState, role: &str)`, `visitors::at(st: &AppState, place: Place, pos: Pos) -> Option<&Visitor>`
  - `visitors::draw(buf: &mut Buffer, area: Rect, origin: Pos, st: &AppState)`: lights first (cells of the flat's rect, walls included, scaled by `lights`), then each visitor `@` (role tint or `DIM`, bold) on `Visible` tiles of the player's place
  - `visitors::colour(book: &Book, role: &str) -> Color` (tint or `DIM`; Tasks 5–6 use it for speakers)
  - `marks::resolve(st: &AppState, mark: &Mark, walker: Option<&str>) -> Option<(Place, Pos)>` (§1 "Marks"; `walker` is the actor id `Beside` measures from, else the player)
  - `marks::flat_rect(st: &AppState) -> Option<(Place, citygen::Rect)>`: the flat's rect grown by one (its walls)
- Casting: `cast::cast` skips `Pick::Visitor` roles; its roll seeds `hash64(seed, id_seed(id))` with `fn id_seed(id: &str) -> u64` folding the id's bytes through `hash64`. `begin` marks a run `Uncast` only for roles in `book.needs_cast(m)`.

- [ ] **Step 1: Write the failing tests**
  - `marks.rs`: on `state_home`-like setup (seed 8, `SizePreset::Medium`, 7 districts, `MapScreen::walking`; reuse a test helper if one exists in `jobs::testutil` or `missions` tests): `home_marks_resolve`: `HomeDoor`, `HomeOutside`, `HomeInside(1)`, `HomeInside(2)`, `HomeRig`, `HomeBed`, `HomeFurniture(Couch)` are all `Some`, on the home floor; `HomeOutside` is walkable and outside `home.rect`; `HomeInside(2)` is two tiles from the door; the couch tile's furniture is `Couch`; `beside_you_is_free_and_next_to_you`: `Beside(You)` is 1 tile from the player and not the player's tile; `no_home_no_marks`: with `st.home = None` every `Home*` mark is `None`.
  - `visitors.rs`: `arrive_and_leave` (two visitors, `at` finds them, `leave` removes one); `arrive_twice_moves`; `a_visitor_blocks_the_way` (a visitor east of the player; `handle_walk` east doesn't move the player; the log's last line contains the visitor's name and `in the way`); `a_new_city_forgets_visitors_and_lights` (after `st.enter_city(..)` both are empty/None).
  - `mod.rs` tests: `at_home_door_fires_outside_the_door` (a step `At(HomeDoor)` fires with the player on `HomeOutside`, not on `HomeInside(1)`); `a_visitor_from_is_never_uncast` (a book whose mission is `from` a visitor runs `Open` after `NewGame`).
  - `cast.rs`: `adding_a_visitor_recasts_nobody` (cast a book; add a visitor role; cast again; every non-visitor role maps to the same person).
  - `track/mod.rs`: the existing test that found `Where::Home` in the shipped book looks for `Where::HomeDoor` once Task 7 lands; in this task add `home_door_tracks_the_door` on a test book: a run waiting on `At(HomeDoor)` resolves to a spot on the home door, radius 1.
  - `look.rs`: `look_reads_a_visitor` (the card shows the name and the role's `look`).
  - `effects.rs`: `lights_dim_the_flat` (after `Effect::Lights(0.5)` a cell of the flat drawn by `visitors::draw` has half the colour it had at 1.0; a cell outside the flat is unchanged).

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke missions:: track:: look::`
Expected: compile errors.

- [ ] **Step 3: Implement.** Marks per §1's table (`HomeRig`: the walkable neighbour of `home.rig` nearest the door; `HomeFurniture(f)`: scan the flat's rect for that furniture, nearest the door, ties top then left; `HomeInside(n)`: step from the door into the rect, stopping at the last walkable tile). Effects `Arrive` (resolve then `visitors::arrive`, dev-mode warn `scene · <mark> not found` when unresolved), `Leave`, `Lights(1.0)` sets `None`, else `Some(level)`; `Effect::Scene(id)` pushes `(mission, step, id)` onto `missions.scenes`. Draw `visitors::draw` in `MapScreen::draw` right after the map view and before the player's `@`. `bump` refuses a step onto a visitor with the log line. Look shows a visitor before people. The track's `HomeDoor` arm targets `Target::Spot` on the door with radius 1.

- [ ] **Step 4: Run the tests**, then the five checks. Tests that assumed the old Marlo (re-cast by the id seed) are updated to the new cast, never by bending the rule; note each one changed in the report.

- [ ] **Step 5: Commit** `scenes: visitors, marks, lights and the home door`.

---

### Task 3: The scripted keypad

**Files:**
- Modify: `crates/strata-zero/src/screens/keypad.rs`
- Test: its `tests`

**Interfaces:**
- Produces: `Keypad::scripted(st: &AppState, building: BuildingId, level: i32, number: u16) -> Keypad`; `Keypad::show_digit(&mut self, d: u8)` (ignored past four); `Keypad::show_verdict(&mut self, open: bool, tick: u64)`. A scripted pad (a `scripted: bool` field) never calls `credentials`, never pops itself, and keeps its verdict's text (`OPEN` / `DENIED`) on the display for as long as it is drawn; the flash's glitch runs `DENIED_TICKS`/`OPEN_TICKS` frames as now.

- [ ] **Step 1: Write the failing tests**: `scripted_digits_show` (four `show_digit`s, the drawn display has the four digits); `scripted_verdict_touches_nothing` (after `show_verdict(false, t)` the journal has no new code, the lockout counter for that flat is unchanged, the log has no new line); `scripted_denied_holds` (drawn at `t + 100` frame ticks the display still reads `DENIED`). Use the file's existing `draw(k, st) -> String` test helper.
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke keypad::` → compile errors.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests, then the five checks.
- [ ] **Step 5: Commit** `scenes: a keypad a scene can type on`.

---

### Task 4: The scene runtime and its hooks

**Files:**
- Create: `crates/strata-zero/src/missions/scene.rs`
- Modify: `missions/mod.rs` (`tick` calls `scene::tick`; `pub mod scene`), `missions/call.rs` (`quiet` is also true while a scene runs; a scene starting sends an up call to voicemail), `screens/explore.rs` (keys, camera, feeds), `screens/console.rs` (`story` lists scenes; `story scene <id>`; `story skip`)
- Test: `scene.rs` tests

**Interfaces:**
- Consumes: Task 1 types; Task 2 `marks::resolve`, `visitors::{arrive, leave}`, `missions.scenes`, `missions.lights`; Task 3 `Keypad::{scripted, show_digit, show_verdict}`; existing `missions::end(st, mission, step)`, `call::quiet`, `st.story(..)` log helper, `Player::update_fov`, `citygen::pathing::astar`.
- Produces:
  - `scene::SceneRun` (§2 fields; `pub` read access for drawing): `look() -> Look`, `title() -> &str`, `stage() -> Option<&str>`, `line() -> Option<&Line>`, `pad() -> Option<&Keypad>`, `doors() -> &[(Place, Pos)]`
  - `scene::Line { pub label: String, pub who: String, pub hidden: bool, pub text: String, pub anchor: Option<(Place, Pos)>, pub began: u64, pub chars_per_tick: f32 }` with `fn shown(&self, now: u64) -> &str` and `fn typing(&self, now: u64) -> bool`
  - `Missions` gains `pub scene: Option<scene::SceneRun>`
  - `scene::running(st: &AppState) -> Option<&SceneRun>`
  - `scene::key(st: &mut AppState, k: &KeyEvent) -> bool` (true for every key while a scene runs)
  - `scene::tick(st: &mut AppState)` (start a queued scene when allowed; step walks; end beats; chain instant beats; end the scene)
  - `scene::skip(st: &mut AppState)`
  - `scene::camera(st: &AppState) -> Option<(Place, Pos)>`
  - `scene::play(st: &mut AppState, id: &str)` (console: queue with no mission step; ending it ends nothing)
- Explore hooks: in the per-key loop, after the console's early return and before `missions::call::key`: `if crate::missions::scene::key(st, k) { continue; }`; the camera follows `scene::camera(st)` when `Some` (with `Camera::target`), else the player as now; the queued-feed push at the top of `update` waits while `scene::running(st).is_some()`; starting a scene clears an aimed walk in flight.

- [ ] **Step 1: Write the failing tests** (build a small book in the test with one scene and a mission whose step plays it; helper `fn run_ticks(st, n)` advancing `st`'s clock by `n` and calling `missions::tick`):
  - `a_held_line_moves_on_by_itself`: `Say(.., hold: Some(1.0))` with `Pace(type: 30, ..)` and a 3-char text: on beat after `3 + 30` ticks, not before.
  - `a_line_without_hold_waits_for_space`: `hold: None` everywhere: still on it after 10 000 ticks; one `space` after typing moves on.
  - `space_finishes_typing_first`: mid-type `space` → `shown == text`, same beat; second `space` → next beat.
  - `a_walk_takes_its_pace`: `Walk(you, HomeInside(2))` from `HomeOutside` with `walk: 0.5` (15 ticks a tile): the player arrives after `path_len × 15` ticks; the player's FOV shows the flat's interior after.
  - `space_ends_a_walk_at_its_goal`.
  - `the_keypad_types_and_touches_nothing`: `Keypad(HomeDoor, Journal, Denied, keys: 0.1, hold: 0.5)`: digits appear one every 3 ticks; the beat ends after `4×3 + 15`; the journal and lockout are untouched.
  - `esc_skips_to_the_end_state`: a scene with two lines, a walk of a visitor, a `Leave` and `Lights(1.0)`: one `esc` → no scene, the visitor gone, lights `None`, both lines in the log once each, the step `Ended` (its `Ended` trigger fires).
  - `keys_do_nothing_but_space_enter_esc` (Review Focus 3): during a line, arrows, `w`, Tab, `1`, `q` leave the player's tile, `st.deck`/screen stack and the beat unchanged; `scene::key` returns true for each.
  - `a_scene_waits_out_a_fight`: with `st.net.engaged = Some(..)` (use the existing test way to engage) a queued scene doesn't start; it starts once disengaged. `a_fight_mid_scene_skips_it`.
  - `a_scene_off_home_never_stalls` (Review Focus 1): `st.home = None`, a scene of `Walk(you, HomeInside(2))`, `Arrive(seren, HomeRig)`, `Say(seren, ..)`, with holds: it ends within the sum of its holds plus typing, no panic.
  - `feeds_and_calls_wait_for_the_scene` (Review Focus 5): a `Feed` effect and a `Call` effect fired while a scene runs: no screen pushed, no call ringing until the scene ends; then they come.
  - `a_new_city_ends_the_scene` (Review Focus 4).
  - `lines_are_logged_as_they_start`: `` `Castellan` · “…” ``, `as` label used, your handle for `you`; stage lines not logged.
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke missions::scene` → compile errors.
- [ ] **Step 3: Implement** per spec §2, §3, §9, §10. Beat behaviour lives in three functions per §14's recipe: `start(st, run, beat)`, `ended(st, run, beat, now) -> bool`, `finish(st, run, beat)` (the end state for `space` and skip). Path: `citygen::pathing::astar` over the place's `TileMap` walkability with start and goal allowed, tiles under other actors blocked; none → put the actor on the goal. A walk to another place moves the actor there at once.
- [ ] **Step 4: Run** the tests, then the five checks.
- [ ] **Step 5: Commit** `scenes: the runtime, keys and camera`.

---

### Task 5: Film, and the scene's overlays

**Files:**
- Create: `crates/strata-zero/src/missions/film.rs`, `crates/strata-zero/src/missions/overlay.rs`
- Modify: `screens/explore.rs` (`MapScreen::draw`), `ui/dim.rs` (`wash_rect`)
- Test: `film.rs` tests

**Interfaces:**
- Consumes: Task 4 `scene::running`, `SceneRun` accessors, `Line::{shown, typing}`; Task 2 `visitors::colour`, `visitors::draw`; existing `layout(area, show_route) -> Regions`, `tile_to_screen`.
- Produces:
  - `ui::dim::wash_rect(buf: &mut Buffer, rect: Rect)`
  - `overlay::draw(buf: &mut Buffer, map: Rect, origin: Pos, st: &AppState)`: open doors (§8) and the speaker's marker `▾` (speaker colour; `DIM` when `hidden`)
  - `overlay::speaker(st: &AppState, line: &Line) -> (String, Color)`: the label upper case and its colour (`as` → `DIM`; `you` → white; a role → `visitors::colour`)
  - `film::map_rect(frame: Rect, map: Rect) -> Rect` (the map centred: `x = (frame.w − map.w) / 2`)
  - `film::draw(buf: &mut Buffer, frame: Rect, map: Rect, st: &AppState)`: blacks out everything outside `map`, then the top bar, the subtitle, the hint (§5)
- Explore: when `scene::running(st)` is a film, `MapScreen::draw` lays out as usual, moves the map rect with `film::map_rect`, draws the map view, visitors, the player and `overlay::draw` into it, skips the band, side panel, route row, footer, toasts and call card, then calls `film::draw`; a shown pad: `wash_rect` on the map, then the pad's `Screen::draw`. The console, if open, draws after as now.

- [ ] **Step 1: Write the failing tests** (draw a `MapScreen` with a running film scene into a `TestBackend` with `shots::draw_stack` or the file's own helper):
  - `film_hides_the_hud` at 120×40: rows 0–4 hold no HUD text (`HP`, `FOCUS` absent), row 1 starts with the title upper case at the map's left edge, the stage line is centred on row 3, the subtitle starts on row 36 with the speaker's label, the hint ends at column `map.right() − 1` on row 39.
  - `film_fits_100x30` (Review Focus 2): a 140-character line wraps to at most three lines on rows 26–28, every character inside the frame, the hint on row 29.
  - `the_marker_sits_over_the_speaker`: a visitor in view speaking → `▾` in their tint on the cell above them.
  - `a_hidden_speaker_reads_dim`: `as: Some("A voice")` → label `A VOICE` in `DIM`.
  - `an_open_door_lets_light_in`: `Door(HomeDoor, open: true)` → the door's cell is `·` in amber.
  - `wash_rect_only_washes_inside` (in `ui/dim.rs`).
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke film` → compile errors.
- [ ] **Step 3: Implement** per §5 and §8.
- [ ] **Step 4: Run** the tests, then the five checks.
- [ ] **Step 5: Commit** `scenes: the film`.

---

### Task 6: Bubbles

**Files:**
- Create: `crates/strata-zero/src/missions/bubbles.rs`
- Modify: `screens/explore.rs` (`MapScreen::draw`)
- Test: `bubbles.rs` tests

**Interfaces:**
- Consumes: Task 5 `overlay::{draw, speaker}`, `wash_rect`; Task 4 accessors.
- Produces:
  - `bubbles::place(anchor: Option<(u16, u16)>, map: Rect, w: u16, h: u16, avoid: &[(u16, u16)]) -> (Rect, Option<(u16, u16, char)>)`: §6 placement; the tail cell and glyph, `None` in the corner fallback
  - `bubbles::draw(buf: &mut Buffer, r: &Regions, origin: Pos, st: &AppState)`: washes the band and side panel, the footer rows, the stage line, the pad if shown, the bubble (§6). `Regions` (explore.rs) becomes `pub(crate)` with its fields, if it isn't.
- Explore: when `scene::running(st)` is bubbles, `MapScreen::draw` draws as usual plus `overlay::draw`, then `bubbles::draw` before the console.

- [ ] **Step 1: Write the failing tests**:
  - `place_prefers_above`: anchor mid-map → the rect's bottom is `sy − 2`, tail `(sx, sy − 1, '▾')`.
  - `place_goes_below_near_the_top`: anchor on the map's first row → top is `sy + 2`, tail `▴`.
  - `place_never_covers_the_player`: avoid contains a cell inside the above-rect → it goes below.
  - `place_falls_back_to_the_corner`: no anchor → bottom-left, no tail.
  - `bubbles_fit_100x30` (Review Focus 2): a 100-character line at 100×30 → the bubble inside the 59×20 map.
  - `bubbles_dim_the_hud_and_say_the_keys`: the footer's last row starts `space next`, the first `cinematic · <title>`.
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke bubbles` → compile errors.
- [ ] **Step 3: Implement** per §6.
- [ ] **Step 4: Run** the tests, then the five checks.
- [ ] **Step 5: Commit** `scenes: the bubbles`.

---

### Task 7: The house call (content) and Mezz

**Files:**
- Modify: `flavor/missions.ron`, `flavor/content.ron` (the taped package's text), `crates/strata-zero/src/credentials.rs` (`fixer_mail`'s fallback name), every test or string that says "Dez" (`grep -rn Dez crates flavor`), `track/mod.rs` (its shipped-book test now finds `Where::HomeDoor`)
- Test: `missions/mod.rs` tests (story), existing tests updated

**Interfaces:**
- Consumes: everything above. Content exactly as spec §11 and §12 (cast `mezz`, `marlo`, `castellan`, `seren`; `the_drop` ends `At(HomeDoor)` with its `Log`; mission `house_call`; scenes `house_call` and `seren_aside`). The RON comment at the file's top gains the `{you}` slot and a short note on `scenes`.

- [ ] **Step 1: Write the failing tests** (`missions/mod.rs` tests, on the shipped book and the seed-8 city):
  - `the_drop_ends_at_your_door`: after the drop's `run` step, standing on `HomeOutside` → `the_drop` is `Done`; `house_call` is `Open`; `missions.scene` is the `house_call` film (after one tick); Castellan is on the couch tile, Seren next to the rig; `missions.lights == Some(0.45)`.
  - `skipping_the_house_call`: `esc` → `seren_aside` runs (bubbles); `esc` → no scene, no visitors, `lights == None`, the objective is `Sleep · Castellan will be in touch`; two game minutes on, a mail from `Mezz` with subject `lay low`; the log holds every line of both scenes once.
  - `playing_the_house_call_through`: tick the film to its end with no keys (all lines have holds) → the player stands on `HomeInside(2)`, Castellan has left, the door is drawn closed, Seren stands next to the player.
  - `mezz_not_dez`: no string `Dez` in the shipped book, the fixer's sim name is `Mezz`.
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke missions::` → failures (content not there yet).
- [ ] **Step 3: Write the content** and the rename.
- [ ] **Step 4: Run** the tests, then the five checks.
- [ ] **Step 5: Commit** `story: the house call, and Dez is Mezz`.

---

### Task 8: Shots

**Files:**
- Modify: `crates/strata-zero/src/screens/shots.rs`
- Test: the shots themselves (ignored), plus `every_drawn_character_has_a_glyph`

**Interfaces:**
- Consumes: Task 7's content; `state_home`, `shot_stack`/`write_svg`, `draw_stack`.
- Produces: ignored tests `scene_film_shots` (writes `scene_film_keypad`, `scene_film_door`, `scene_film_couch`, `scene_film_seren` at 120×40 and `scene_film_couch_100x30`) and `scene_bubbles_shots` (`scene_bubbles_seren`, `scene_bubbles_seren_100x30`). Each drives the shipped story to the drop's end at your door, then ticks the scene to the beat named (a helper `fn scene_at(st, map, beat: usize)` ticking until `run.beat == beat` and the line fully typed).

- [ ] **Step 1: Write the shot tests.**
- [ ] **Step 2: Run** `SHOT_DIR=<scratch dir> cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored scene_` and look at the PNGs: the title, the stage line, the subtitle and the hint where §5 puts them; the bubble beside Seren, never on the player.
- [ ] **Step 3: Run** `cargo test -p strata-zero --features window --target-dir target/smoke every_drawn_character_has_a_glyph` → PASS.
- [ ] **Step 4: Run** the five checks.
- [ ] **Step 5: Commit** `scenes: shots of the film and the bubbles`.
