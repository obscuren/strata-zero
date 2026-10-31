# Story missions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Authored story missions built from data blocks (triggers, effects, casting),
shown through a modal HOLO feed and a non-modal HOLO call card with a glitching face,
with the first chain, the drop, replacing the welcome favour at new game.

**Architecture:** A new `crates/strata-zero/src/missions/` module owns the content
(`flavor/missions.ron`), the casting of roles onto sim people, the per-mission runtime
(one waiting step each), bodies, and the HOLO's drawing. The game reports two events
(`NewGame`, `Talked`) and the explore update calls `missions::tick` for conditions; the
explore screen pushes queued feeds and hosts the call card. The sim learns dead and
renamed people.

**Tech Stack:** Rust 2024 workspace, ratatui + crossterm, serde + ron, the existing
`sim`, `citygen`, `game` crates.

**Spec:** `docs/superpowers/specs/2026-10-31-story-missions-design.md` (read it whole
before any task; section numbers below refer to it). Mock pages and their generators:
`docs/superpowers/specs/2026-10-31-story-missions-mock/` (`gen3.py` holds the face and
the five glitch recipes as Python).

## Global Constraints

- Every cargo command builds into `--target-dir target/smoke`, never `target/`.
- Both front ends: before a task's commit, `cargo fmt --all --check`, `cargo clippy
  --workspace --all-targets --target-dir target/smoke -- -D warnings`, and `cargo test
  -p strata-zero --target-dir target/smoke` (plus `-p sim` for Task 1). Task 10 runs the
  full matrix with `--features strata-zero/window` too.
- Draw only through ratatui; exact `Color::Rgb`; glyphs only box/block characters and
  ones the game already draws (`● ○ ▸ “ ” ▌ · ×`). `every_drawn_character_has_a_glyph`
  (window/font.rs) must stay green.
- No mission id, step id or role id (`the_drop`, `welcome`, `dez`, `marlo` …) in Rust
  outside `#[cfg(test)]` code.
- `st.story(..)` is the existing story-log helper; the module and state are `missions`.
- Colours: magenta `ui::chrome::MAGENTA` (255, 0, 200), cyan `CYAN` (0, 229, 255),
  `DIM`, `FAINTER`, `TEXT`, `PANEL_BG` (4, 6, 10); bodies `FOE_DOWN` (120, 30, 30).
- Commit messages: imperative, prefixed like the repo (`missions: …`, `sim: …`), ending
  with the line `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Match the surrounding code: doc comments on pub items in the repo's terse voice, no
  new dependencies.

## Review Focus

1. **A city that cannot cast a role** (no noodle cook anywhere, no fixer near): the
   drop must not panic or show a half-filled feed; its run is `Uncast` and nothing
   shows. Test in Task 3: `a_role_nobody_can_play_leaves_the_mission_uncast`.
2. **Killing someone mid-trip or indoors:** the body lies at the work tile and the sim
   never shows them again, an hour later included. Test in Task 1:
   `a_dead_person_stays_gone_an_hour_later`.
3. **Two presentations at once:** a second call while one is up queues and rings after;
   a feed opened while a call is up does not end the call. Test in Task 7:
   `a_second_call_waits_for_the_first`.
4. **The 100×30 terminal:** the call card's text wraps inside a 59-wide map and the
   feed keeps its `space` row on screen. Tests in Tasks 6 and 7 at 100×30.
5. **A second city in one session** (load game, new game again): `enter_city` resets
   the story and the next walk begins it again with a fresh cast. Test in Task 3:
   `a_new_city_begins_a_new_story`.

---

### Task 1: Dead and renamed people in the sim

**Files:**
- Modify: `crates/sim/src/world.rs` (the `World` struct, `step`, `position`, `inside`,
  `person_at`, `visible`, `refresh_open`, tests)

**Interfaces:**
- Produces: `World::kill(&mut self, id: PersonId)`, `World::is_dead(&self, id:
  PersonId) -> bool`, `World::rename(&mut self, id: PersonId, name: &str)`. A dead
  person stays in `people()` (ids are indices).

- [ ] **Step 1: Write the failing tests** in `world.rs`'s tests module:
  - `a_dead_person_is_gone_from_every_lookup`: seed a world (as the module's other
    tests do), pick a person standing on the street (`pos_of` is `Some`), `kill` them;
    assert `is_dead`, `pos_of(id) == None`, `person_at(that pos) != Some(id)`, no
    `visible(rect around it)` entry for them, and that for every building `inside(b)`
    has no occupant with their id.
  - `a_dead_person_stays_gone_an_hour_later`: kill a person who is `Travelling` (find
    one), `advance` 60 minutes; assert `pos_of == None` and they are in no `inside`.
  - `rename_renames`: `rename(id, "Dez")`; `people()[id].name == "Dez"`.
- [ ] **Step 2: Run** `cargo test -p sim --target-dir target/smoke dead_person` and
  `rename_renames`; expect compile errors (no `kill`).
- [ ] **Step 3: Implement.** A `dead: BTreeSet<PersonId>` on `World`; `kill` inserts and
  drops any pending trip or displacement for them; `step`'s per-person loops, chats and
  Strip dates skip the dead; `position` returns `None`; `inside`, `person_at`,
  `visible` and `refresh_open` leave them out.
- [ ] **Step 4: Run** `cargo test -p sim --target-dir target/smoke`; all pass.
- [ ] **Step 5: Commit** `sim: dead and renamed people`.

### Task 2: The book — `flavor/missions.ron`, its types and validation

**Files:**
- Create: `crates/strata-zero/src/missions/mod.rs` (for now only `pub mod content;` and
  re-exports), `crates/strata-zero/src/missions/content.rs`, `flavor/missions.ron`
- Modify: `crates/strata-zero/src/main.rs` (or wherever modules are declared: add `mod
  missions;`), `crates/strata-zero/src/app.rs` (`pub book: missions::Book`, loaded in
  `AppState::new` with `.expect("the missions pack is valid")`)

**Interfaces:**
- Produces (all `pub`, serde `Deserialize`, `Clone, Debug, PartialEq`):
  ```rust
  #[serde(rename = "Story")]
  pub struct Book { pub cast: BTreeMap<String, Role>, pub faces: BTreeMap<String, Face>, pub missions: Vec<Mission> }
  pub struct Role { pub name: String, pub title: String, pub pick: Pick, pub face: Option<String> }
  pub enum Pick { NearestFixer, Worker { professions: Vec<String>, at: Vec<citygen::BuildingKind>, range: (i32, i32) } }
  pub struct Face { pub rows: Vec<String>, pub eye: (u8, u8, u8) }
  pub struct Mission { pub id: String, pub title: String, pub from: String, pub steps: Vec<Step> }
  pub struct Step { pub id: String, pub when: Trigger, pub then: Vec<Effect> }
  pub enum Trigger { NewGame, Ended(String), Sees { role: String, within: i32 }, After { minutes: u64 },
                     At(Where), Talk(String), MissionDone { mission: String, minutes: u64 } }
  pub enum Where { Home, Work { role: String, within: i32 } }
  pub enum Effect { Feed { from: String, text: Vec<String>, close: String }, Call { from: String, lines: Vec<String> },
                    Give { item: String, count: u32 }, Take { item: String, count: u32 },
                    Dead { role: String, look: String }, Objective(String),
                    Mail { from: String, subject: String, body: Vec<String> }, Log(String) }
  pub enum BookError { Parse(String), Empty(String), UnknownRole { place: String, role: String },
                       UnknownSlot { place: String, slot: String }, UnknownStep { mission: String, step: String },
                       NotAPresentation { mission: String, step: String }, UnknownMission(String),
                       DuplicateStep { mission: String, step: String }, BadFace { face: String, why: String },
                       NoSilhouette }   // thiserror, like jobs::FlavorError
  impl Book {
      pub fn load() -> Result<Book, BookError>;            // the shipped file
      pub fn parse(src: &str) -> Result<Book, BookError>;  // any text; validates
      pub fn validate(&self) -> Result<(), BookError>;
      pub fn items(&self) -> Vec<&str>;                    // every item Give/Take names
      pub fn face_of(&self, role: &str) -> &Face;          // the role's face or `silhouette`
  }
  impl Mission { pub fn roles(&self) -> BTreeSet<String>; } // every role it names: from, triggers, effects, slots
  pub const SLOT_FIELDS: [&str; 4] = ["title", "place", "district", "hours"];
  pub fn slots(text: &str) -> Vec<(String, Option<String>)>; // `{marlo.place}` → ("marlo", Some("place"))
  ```
- Validation rules are spec §1's list. A face: rows all 16 chars, even count, only
  `.dmlhbeX`. `Ended(step)` must name an **earlier** step of the same mission whose
  effects include a `Feed` or `Call`.

- [ ] **Step 1: Write `flavor/missions.ron`** exactly as spec §1 and §11 lay out: the
  header comment, `dez` (NearestFixer, face `hustler`), `marlo` (Worker noodle_cook, at
  `[]`, range `(30, 140)`, face `None`), faces `hustler` (the 18 rows of `FACE` in
  `2026-10-31-story-missions-mock/gen.py`, eye `(255, 0, 200)`) and `silhouette` (18
  rows, 16 wide: a head-and-shoulders outline in `d` and `m` only, no `e`/`X`/`h`), and
  the mission `the_drop` verbatim from §11.
- [ ] **Step 2: Write the failing tests** in `content.rs`:
  - `the_shipped_book_parses_and_validates`: `Book::load().unwrap()`; one mission
    `the_drop` with six steps; `cast` has `dez` and `marlo`.
  - `every_item_the_book_names_exists`: with `game::Content::load()` (as other tests
    get content), every `items()` entry resolves.
  - `slots_split_role_and_field`: `slots("to {marlo}, at {marlo.place}")` ==
    `[("marlo", None), ("marlo", Some("place"))]`.
  - `the_drop_needs_dez_and_marlo`: `roles()` == `{"dez", "marlo"}`.
  - One refusal test each, mutating a parsed copy: unknown role in a `Feed.from`
    (`UnknownRole`), `{nobody}` in a `Log` (`UnknownRole`), `{marlo.shoe}`
    (`UnknownSlot`), `Ended("body")` on a step after `body` that opens nothing
    (`NotAPresentation`), `Ended("later")` naming a later step (`UnknownStep`),
    `MissionDone(mission: "nope")` (`UnknownMission`), a duplicate step id
    (`DuplicateStep`), a 15-wide face row (`BadFace`), removing `silhouette`
    (`NoSilhouette`).
- [ ] **Step 3: Run** `cargo test -p strata-zero --target-dir target/smoke missions::`;
  expect failures.
- [ ] **Step 4: Implement** `content.rs` and the `AppState.book` field.
- [ ] **Step 5: Run** the tests; pass. Run clippy.
- [ ] **Step 6: Commit** `missions: the book, its cast, faces and the drop`.

### Task 3: Casting and the runtime (no drawing yet)

**Files:**
- Modify: `crates/strata-zero/src/missions/mod.rs`; create
  `crates/strata-zero/src/missions/cast.rs`, `crates/strata-zero/src/missions/effects.rs`
- Modify: `crates/strata-zero/src/app.rs` (`pub missions: missions::Missions`, default;
  reset in `enter_city`/`enter_city_at`), `crates/strata-zero/src/screens/mod.rs`
  (`explore_screen`: build the `MapScreen::walking`, then `crate::missions::begin(st)`,
  then box it), `crates/strata-zero/src/screens/explore.rs` (the update:
  `crate::missions::tick(st)` right after `crate::jobs::tick(st)`),
  `flavor/content.ron` (the item `taped_package`, spec §12, verbatim) and
  `flavor/missions.ron` (`Give(item: "taped_package", count: 1)` in place of
  `sealed_package`), `crates/strata-zero/src/missions/content.rs` (`Mission::roles` must
  include a `Talk(role)` trigger's role; Task 2's review found it missing), and remove
  Task 2's staged `#![allow(dead_code, unused_imports)]` (missions/mod.rs) and
  `#[allow(dead_code)]` (app.rs `book`) once the runtime uses them, keeping a narrow
  allow only where a later task's item is still unused, with a comment naming that task,
  `crates/strata-zero/src/credentials.rs` (`move_in` stops calling
  `jobs::first_favour`), `crates/strata-zero/src/jobs/generate.rs` (`first_favour`,
  `first_fetch` → `#[cfg(test)]`), `crates/strata-zero/src/jobs/mod.rs`
  (`testutil::state_home` calls `crate::jobs::first_favour(&mut st)` after `move_in`)
- Fix every test or shot that assumed the welcome favour after `MapScreen::walking`
  (spec §12: `screens/shots.rs` keypad and `offer_stack`, `screens/explore.rs`'s
  "Subject: a favour" test, `interact.rs`'s welcome drop): call
  `crate::jobs::first_favour(&mut st)` where the test needs the favour, or drop the
  assumption.

**Interfaces:**
- Consumes: Task 1 (`kill`, `is_dead`, `rename`), Task 2 (the book types).
- Produces (in `missions/mod.rs` unless noted):
  ```rust
  #[derive(Default)] pub struct Missions { pub cast: BTreeMap<String, PersonId>, pub runs: Vec<Run>,
      pub bodies: Vec<Body>, pub feeds: VecDeque<FeedView>, pub call: Option<Call>,
      pub calls: VecDeque<Call>, pub begun: bool }
  pub struct Run { pub at: usize, pub since: u64, pub state: RunState, pub objectives: Vec<String>, pub ended: BTreeSet<usize> }
  pub enum RunState { Waiting, Open, Done { at: u64 }, Uncast }
  pub struct Body { pub person: PersonId, pub name: String, pub place: Place, pub pos: Pos, pub look: String }
  pub struct FeedView { pub mission: usize, pub step: usize, pub from: String, pub text: Vec<String>, pub close: String }
  pub struct Call { pub mission: usize, pub step: usize, pub from: String, pub lines: Vec<String>,
                    pub rang: u64, pub answered: Option<u64>, pub line: usize, pub shown: u64 }
  pub enum Event { NewGame, Talked(PersonId) }
  pub fn begin(st: &mut AppState);                       // once per city, needs st.home
  pub fn send(st: &mut AppState, ev: Event);
  pub fn tick(st: &mut AppState);                        // fire(st, None); Task 7 adds the call's tick here
  pub fn end(st: &mut AppState, mission: usize, step: usize); // a feed/call of that step ended: mark, then fire
  pub fn listens(st: &AppState, who: PersonId) -> bool;  // a run waits on Talk for who's role
  pub fn fill(st: &AppState, text: &str) -> String;      // spec §5 slots
  pub fn person_of(st: &AppState, role: &str) -> Option<PersonId>;
  pub fn role_name(st: &AppState, role: &str) -> String; // the book's name for the role
  // cast.rs
  pub fn cast(st: &mut AppState);
  pub fn work_tile(st: &AppState, id: PersonId) -> Option<Pos>; // spec §6
  // effects.rs
  pub fn apply(st: &mut AppState, mission: usize, step: usize, e: &Effect);
  ```
  `Effect::Call` pushes into `calls` and, when `call` is `None`, moves the front into
  `call` with `rang = st.clock_tick()`. Task 7 replaces this with `call::ring`.
- Values: firing repeats up to 16 passes; `After`/`MissionDone` compare game minutes
  (`st.net.now(st)`); `Sees` = same place, `Visibility::Visible` in the player's `vis()`,
  manhattan ≤ `within`; `At(Home)` = player on `Place::Floor { building, level }` of
  `st.home`'s `home` and `home.rect.contains(pos)`; `At(Work)` = player on the Street
  within `within` of `work_tile`. `Give` logs `` `<item name>` ×<n> · in your pack ``.
  `Dead` with the person already dead does nothing. In dev mode, an uncast role warns
  `story · no <role> in this city`.

- [ ] **Step 1: Write the failing tests** (in `missions/mod.rs` tests, a real city via
  `jobs::testutil::state_home()` then `begin`; the welcome favour must NOT be there
  after `MapScreen::walking` — add `new_game_has_no_favour_mail`; and
  `explore_screen_begins_the_story` (a fresh city through `screens::explore_screen`:
  `begun`, one feed) while `MapScreen::walking` alone leaves `begun == false`):
  - `roles_include_a_talk_role` (a test book with `Talk("dez")` as its only mention of
    dez: `roles()` has `dez`);
  - `begin_casts_dez_and_marlo_and_queues_the_welcome`: cast has both; `person_of("dez")
    == jobs::nearest_fixer(home door)`; that person's name is "Dez"; Marlo's work tile
    is 30–140 from the home door; `feeds.len() == 1`, its `text[1]` contains "Marlo"
    and no `{`.
  - `the_same_city_casts_the_same`: two fresh states, same cast ids.
  - `closing_the_welcome_gives_the_package_and_kills_marlo`: `end(st, 0, 0)`; the pack
    has one `taped_package` and no `sealed_package`; `world.is_dead(marlo)`; one `Body` at `work_tile`;
    objective starts "Hand the package to Marlo"; run waits on step 2 (`body`).
  - `seeing_the_body_then_a_minute_rings_dez`: put the player on the Street next to
    the body with sight updated (as the shot's street test does), `tick`; run waits on
    `ring`; advance the world one game minute, `tick`; `call.is_some()`, from `dez`,
    three lines.
  - `ending_the_call_sets_get_home_and_home_closes_it`: `end(st, 0, 3)`; objective
    "Get home · keep the package"; move the player into the flat (`home.rect`, home
    floor), `tick`; run is `Done`; the last mail's subject is "lay low".
  - `a_role_nobody_can_play_leaves_the_mission_uncast` (Review Focus 1): a test book
    whose Worker wants profession `"astronaut"`; `begin`; run `Uncast`; no feed.
  - `a_new_city_begins_a_new_story` (Review Focus 5): `begin`, `enter_city` again with
    the same city, `begun == false`, `begin` again casts and queues a feed.
  - `mission_done_after_minutes_starts_the_next` and `at_work_fires_near_the_role`: a
    small test book (two missions) through `Book::parse`.
  - In `cast.rs`: `work_tile_of_a_walk_in_stall_is_a_walkable_street_tile_outside_it`.
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke missions::`;
  expect failures.
- [ ] **Step 3: Implement** `Missions`, `begin`, `send`, `tick`, firing and every
  trigger arm (spec §2–§3), `cast.rs` (spec §5, `hash64(seed, role index)` for the
  seeded pick), `effects.rs` (spec §4, `Dead` per §6). Wire `begin`/`tick` into
  explore and the state into `AppState`. Move the welcome favour out of `move_in` and
  repair the tests that leaned on it.
- [ ] **Step 4: Run** `cargo test -p strata-zero --target-dir target/smoke`; the whole
  suite passes. Clippy.
- [ ] **Step 5: Commit** `missions: casting, firing, effects; the drop replaces the favour`.

### Task 4: Bodies on the map, in look, out of the sim's rooms and jobs

**Files:**
- Modify: `crates/strata-zero/src/render/people.rs` (draw bodies with the down foes:
  `%` in `FOE_DOWN` on tiles not `Unseen`), `crates/strata-zero/src/screens/explore.rs`
  (where the people overlay is built: pass the bodies on the current place),
  `crates/strata-zero/src/look.rs` (a body reads like a down foe: name header, `%`,
  line `dead`, the body's `look` as the description, the out-of-sight case as foes),
  `crates/strata-zero/src/interiors.rs` (`Placement::new` skips the dead),
  `crates/strata-zero/src/jobs/generate.rs` (the Deliver pool skips the dead)

**Interfaces:**
- Consumes: `Missions.bodies`, `Body` (Task 3); `World::is_dead` (Task 1).

- [ ] **Step 1: Write the failing tests:** `a_body_is_drawn_once_seen` (render a map
  frame with a body on a visible tile; the cell is `%` with fg `FOE_DOWN`; on an
  `Unseen` tile nothing), `look_reads_a_body` (header "Marlo", a line "dead", the look
  text), `the_dead_get_no_deliveries` (roll a Deliver, kill the
  person it names, roll Delivers for counters 0–30: none names a dead person), `placement_leaves_out_the_dead`.
- [ ] **Step 2: Run** them; expect failures.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -p strata-zero --target-dir target/smoke`; pass.
- [ ] **Step 5: Commit** `missions: bodies on the map and in look`.

### Task 5: The face and the bracket frame

**Files:**
- Create: `crates/strata-zero/src/missions/face.rs`

**Interfaces:**
- Consumes: `content::Face` (Task 2), `ui::pixels::Pixels`, `ui::cyberdeck::Pen`.
- Produces:
  ```rust
  pub const FACE_W: u16 = 20;                       // 16 + two empty columns each side
  pub fn rows(face: &Face) -> u16;                  // face.rows.len() / 2
  pub fn burst(tick: u64, seed: u64) -> Option<u8>; // the recipe (0..5) drawn at this tick, None when calm
  pub fn pixels(face: &Face, tick: u64, seed: u64) -> Pixels; // pure
  pub fn draw(buf: &mut Buffer, x: u16, y: u16, face: &Face, tick: u64, seed: u64);
  pub fn bracket_frame(buf: &mut Buffer, area: Rect); // spec §7's frame, on PANEL_BG
  ```
- Values (spec §9): tint d 0.22 / m 0.45 / l 0.75 / h 1.0 / b 0.3 of cyan, e (232, 255,
  255), X the face's eye; scanline 60 % on pixel rows of the current phase, phase =
  `(tick / 4) % 2`; 60-tick cycles, burst start `hash64(seed, cycle) % 30`, frames
  `2 + h % 4`, each `2 + (h >> 8) % 2` ticks, recipe `((h >> 16) + frame) % 5`; the five
  recipes with rows as fractions of the face height (row r of an 18-row face is `r *
  rows / 18`); fringe magenta at 80 % left and (127, 246, 255) right; cut clamped.
  The frame: border cells `▀`/`▄`/`▌`/`▐` in magenta at 35 %, corners `▛▀▀`, `▀▀▜`,
  `▙▄▄`, `▄▄▟` and the cells right under/over them in full magenta.

- [ ] **Step 1: Write the failing tests:** `pixels_are_pure` (same args, same pixels),
  `the_scanlines_crawl` (ticks 0 and 4 differ, no burst at either for a seed where
  `burst` is None), `every_cycle_has_one_burst_of_two_to_five_frames` (for cycles 0–20,
  count ticks with `burst` Some: between 4 and 15, in one contiguous run inside the
  first 45 ticks), `a_burst_frame_differs_from_calm`, `slips_stay_on_the_canvas` (no
  recipe writes outside 20 wide), `the_frame_has_its_corners` (draw into a 30×6 buffer:
  `▛` at (0,0), `▟` at (29,5), fg magenta).
- [ ] **Step 2: Run**; fail. **Step 3: Implement.** **Step 4: Run**; pass.
- [ ] **Step 5: Commit** `missions: the HOLO face and its glitch`.

### Task 6: The HOLO feed screen

**Files:**
- Create: `crates/strata-zero/src/missions/feed.rs`
- Modify: `crates/strata-zero/src/screens/explore.rs` (at the top of the update, when
  `st.missions.feeds` has one: `pop_front` it and return `ScreenCmd::Push { to:
  Box::new(Feed::new(view, st.tick)), fx: TransitionKind::Glitch }`)

**Interfaces:**
- Consumes: `FeedView`, `missions::end`, `role_name`, `st.book` (cast titles, faces),
  `face::{draw, bracket_frame, FACE_W}`.
- Produces: `pub struct Feed`, `Feed::new(view: FeedView, tick: u64) -> Feed`, `impl
  Screen for Feed`; `pub fn layout(frame: Rect, text: &[String]) -> (Rect, Vec<String>)`
  (the box and the wrapped lines with blank rows between paragraphs).
- Values (spec §7): width `min(90, frame.width - 4)`, text column at 22, wrap at width −
  26, height `max(16, lines + 6)` capped at `frame.height - 2` (cut with `…`), centred;
  `space`/`enter`/`esc` → `missions::end(st, mission, step)` then `ScreenCmd::Pop`.

- [ ] **Step 1: Write the failing tests:** `the_feed_keeps_every_word_at_100x30_and_120x40`
  (draw the welcome over a map with `shots::draw_stack`; every word of the three
  paragraphs and "space" and "take the package" appear), `space_closes_the_feed_and_ends_its_step`
  (update with space → `Pop`; the pack has the package), `the_explore_screen_opens_a_queued_feed`
  (after `begin`, one map update returns `Push`).
- [ ] **Step 2–4:** run, implement, run.
- [ ] **Step 5: Commit** `missions: the HOLO feed`.

### Task 7: The HOLO call card

**Files:**
- Create: `crates/strata-zero/src/missions/call.rs`
- Modify: `crates/strata-zero/src/missions/effects.rs` (`Effect::Call` → `call::ring`),
  `crates/strata-zero/src/missions/mod.rs` (`tick` also runs `call::tick`),
  `crates/strata-zero/src/screens/explore.rs` (keys: before the walk keys, every key
  event goes to `call::key` first and is dropped when it returns true; draw: after the
  map overlays, `call::draw(f.buffer_mut(), r.map, st)`)

**Interfaces:**
- Consumes: `Call`, `missions::end`, `role_name`, `face`, the book.
- Produces:
  ```rust
  pub const RING_TICKS: u64 = 900;      // 30 s of clock ticks
  pub const TYPE_PER_SECOND: u64 = 60;  // characters
  pub fn ring(st: &mut AppState, call: Call);        // up now, or queued behind the one up
  pub fn tick(st: &mut AppState);                    // voicemail after RING_TICKS
  pub fn key(st: &mut AppState, k: &KeyEvent) -> bool; // true when the card took it
  pub fn draw(buf: &mut Buffer, map: Rect, player: Option<(u16, u16)>, st: &AppState); // player = its screen cell
  pub fn card_rect(map: Rect, talking: bool, player: Option<(u16, u16)>) -> Rect;
      // ringing 36×5; talking min(62, map.w−2)×13, compact ×7 (no face) when map.h < 28;
      // at (map.x+1, map.y+1), or (map.x+1, map.bottom()−1−h) when the player's cell is inside that
  pub fn quiet(st: &AppState) -> bool;  // a fight is on (st.net.engaged) or the player is on a Place::Construct
  ```
- Behaviour exactly as spec §8: while `quiet`, a queued call does not ring (its ring
  timer does not run) and a call that is up goes to voicemail at once; `key` takes
  nothing while `quiet`; keys `space`/`esc` only; each line logged as it starts
  (`` `Dez` · “…” ``); voicemail mails all lines (subject `voicemail`) and logs ``
  `Dez` · voicemail on your rig ``; any end calls `missions::end` and rings the next
  queued call.

- [ ] **Step 1: Write the failing tests:** `a_ringing_card_names_the_caller` (draw:
  "INCOMING · DEZ", "space", "answer"), `space_answers_and_types_the_first_line`
  (key space → answered; advance clock ticks; the card shows the first line; the log
  has it), `space_steps_through_and_the_last_ends_the_call` (step ended, `call` None),
  `esc_while_ringing_is_voicemail` (mail with all three lines, step ended),
  `ringing_out_is_voicemail` (advance 900 clock ticks, `tick`), `esc_while_talking_hangs_up`,
  `a_second_call_waits_for_the_first` (Review Focus 3), `the_card_fits_a_100x30_map`
  (talking card inside `r.map` at 100×30, compact: 7 rows, no face, every word of the
  current line visible), `the_card_moves_off_the_player` (player cell inside the
  top-left rect → the card sits at the bottom-left), `no_call_rings_in_a_fight` (with
  `st.net.engaged` set, `ring` leaves it queued; `tick` past 900 does not voicemail;
  clearing the fight and ticking rings it), `a_fight_sends_a_live_call_to_voicemail`,
  `space_with_a_call_up_does_not_open_the_space_window` (explore update with a call
  up and a person beside the player: no `Push`).
- [ ] **Step 2–4:** run, implement, run.
- [ ] **Step 5: Commit** `missions: the HOLO call`.

### Task 8: Talk as a trigger

**Files:**
- Modify: `crates/strata-zero/src/screens/popup.rs` (`Pick::Talk` gains `who:
  Option<sim::PersonId>`; in `fire`, when `who` is `Some` and `missions::listens`, send
  `Event::Talked(who)` and return `ScreenCmd::Pop`), `crates/strata-zero/src/interact.rs`
  and `crates/strata-zero/src/jobs/ui.rs` (pass the person to every `Pick::Talk`)

**Interfaces:**
- Consumes: `missions::{listens, send, Event}`.

- [ ] **Step 1: Write the failing test** `talking_to_a_role_fires_its_step`: a test book
  whose first step is `Talk("dez")` opening a `Feed`; `begin`; build the Space window on
  Dez (`interact::gather`), fire its `t` row: `Pop`, one feed queued. And
  `talk_is_unchanged_for_anyone_else`: the card shows as before.
- [ ] **Step 2–4:** run, implement, run.
- [ ] **Step 5: Commit** `missions: talk fires a waiting step`.

### Task 9: The JOBS pane and tab

**Files:**
- Create: `crates/strata-zero/src/missions/ui.rs`
- Modify: `crates/strata-zero/src/screens/explore.rs` and `crates/strata-zero/src/ui/sidebar.rs`
  (where `jobs: crate::jobs::ui::pane_lines(st)` is built: story lines first),
  `crates/strata-zero/src/ui/sidebar.rs` (`JOBS_CAP` 4 → 5),
  `crates/strata-zero/src/screens/deck.rs` (`Detail::Mission(usize)`; the JOBS tab's
  rows from `missions::ui::deck_rows(st)` then the jobs)

**Interfaces:**
- Produces: `pub fn pane_lines(st: &AppState) -> Vec<Line<'static>>`, `pub fn
  deck_rows(st: &AppState) -> Vec<Row>` (empty with no story; else `STORY` header, the
  runs, `JOBS` header), `pub fn detail(st: &AppState, mission: usize) -> Vec<Line<'static>>`.
- Values (spec §10): pane ` THE DROP` magenta bold, ` · Dez · story` dim; ` » <objective>`
  in `TEXT`. Tab rows `  <title>` open (Normal), `✓ <title>` done (Done). Detail: title
  magenta bold; `from <name> · story · open|done` dim; blank; objectives `☑` dim for
  earlier, `☐` for the current.

- [ ] **Step 1: Write the failing tests:** `the_pane_shows_the_drop_above_the_job`,
  `the_tab_has_story_then_jobs`, `the_tab_is_unchanged_without_a_story`,
  `the_detail_ticks_earlier_objectives`.
- [ ] **Step 2–4:** run, implement, run (existing sidebar/deck tests stay green).
- [ ] **Step 5: Commit** `missions: the story in the JOBS pane and tab`.

### Task 10: Console, shots, docs and both front ends

**Files:**
- Modify: `crates/strata-zero/src/screens/console.rs` (`story`, `story fire <mission>`,
  `story feed`, `story call`; `COMMANDS` length and `help`),
  `crates/strata-zero/src/missions/ui.rs` (`pub fn console_lines(st) -> Vec<String>`),
  `crates/strata-zero/src/screens/shots.rs` (shots `holo_feed`, `holo_call_ringing`,
  `holo_call_talking`, `holo_call_glitch`, each at 120×40 and 100×30; the street ones
  put the player beside Marlo's body), `docs/development.md` (the console's `story`
  where the console commands are listed, if they are), `README.md` only if it lists
  the opening job.

- [ ] **Step 1: Write the failing tests:** `story_lists_the_cast_and_missions`
  (`console::run(st, .., "story")` mentions `dez`, `the_drop`, `waits on`),
  `story_fire_runs_the_waiting_step`, `story_call_rings`.
- [ ] **Step 2–4:** run, implement, run.
- [ ] **Step 5: Shots.** `SHOT_DIR=<scratch>/shots cargo test -p strata-zero --features
  window --target-dir target/smoke -- --ignored holo_`; look at the PNGs (feed centred,
  face glitch frame visibly slipped, card in the map's top-left, nothing clipped at
  100×30).
- [ ] **Step 6: The full matrix** (CLAUDE.md): fmt check; clippy with and without
  `--features strata-zero/window`; `cargo test --workspace` with and without it;
  `cargo check -p strata-zero --features window --target x86_64-unknown-linux-gnu
  --target-dir target/smoke` if that target is installed. All green.
- [ ] **Step 7: Commit** `missions: console, shots and docs`.
