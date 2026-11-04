# Saving the run Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A run is saved as a snapshot of the whole game, reopened from CONTINUE or LOAD GAME exactly as it was left, and kept in a HALL OF FAME once it flatlines.

**Architecture:** Every run type derives serde. `runs::snapshot` destructures `AppState` field by field (no `..`) into a borrowed `SnapshotRef` that is written as gzipped MessagePack with named fields, under a small header the lists read. `runs::restore` destructures the other way and rebuilds what is derived. Story positions are written by mission and step id through a book in scope, so a run survives an edited `missions.ron`. New screens: the runs list (LOAD GAME and HALL OF FAME in one screen), and changes to the title, the pause menu, the map's status row, the broadcast and the flatline.

**Tech Stack:** Rust 2024, serde, `rmp-serde` 1 (`to_vec_named`), `flate2` 1 (gzip, `Compression::fast()`), `rmpv` 1 (dev only, for comparing), ratatui 0.29, crossterm 0.28.

**Spec:** `docs/superpowers/specs/2026-11-04-save-the-run-design.md` (mock: `docs/superpowers/specs/2026-11-04-save-the-run-mock/`, open `index.html` after `python3 -I gen.py shots index.html`).

## Global Constraints

- Build only into `--target-dir target/smoke` (relative to this worktree), never `target/`.
- The five checks, both front ends, all green before a task is done:
  `cargo fmt --all --check`;
  `cargo clippy --workspace --all-targets --target-dir target/smoke -- -D warnings`;
  `cargo clippy --workspace --all-targets --features strata-zero/window --target-dir target/smoke -- -D warnings`;
  `cargo test --workspace --target-dir target/smoke`;
  `cargo test --workspace --features strata-zero/window --target-dir target/smoke`.
- The worktree's shell refuses compound commands (`&&`, `;`, pipes into git, `$VAR`): one plain command per call.
- `AppState::new()` has saving **off** (`st.saves == None`); only `cli::start_state` turns it on with the runs folder. No test writes into the crate's working directory; tests that save use a temp dir under `std::env::temp_dir()` with a unique name and remove it.
- The run file: magic `SZRN`, `u16` LE format version `1`, `u32` LE header length, the header (MessagePack, named), the body (gzip of MessagePack, named). Written to `<id>.run.tmp` and renamed over `<id>.run`. Files live in `runs/` (`runs::default_dir()`), beside `cities/`.
- Never compare saved bytes for equality (HashMaps serialize in any order); compare through `runs::testutil::same` (normalized `rmpv` values, Task 2).
- Copy is the spec's, verbatim. Authored copy (the taunts) lives in `flavor/`, not in Rust: no authored names or lines in non-test Rust.
- Keys: space confirms, esc backs out, ↑↓ and `j`/`k` move. Never enter for a new confirm.
- Colours, exact `Color::Rgb`: CYAN (0,229,255), PICK (0,204,184), MAGENTA (255,0,200), RED (255,79,106), PINK (248,147,147), TEXT (205,212,222), DIM (160,170,185), FAINT (100,110,125), EDGE (70,90,110). Reuse the screen's own constants where the same colour exists.
- No new symbols beyond ones already drawn (`▸ · ¤ ─ │ ╭ ╮ ╰ ╯ ↑↓`): `every_drawn_character_has_a_glyph` must stay green.
- Commits end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. A run saved inside a floor (the flat) or a construct reopens on that floor or construct on its first frame, not the street (restore brings `interiors`/`constructs` back, Task 3 test `reopens_on_the_floor_it_was_saved_on`).
2. A run reopened with mail already announced doesn't announce it again, and a queued HOLO feed opens once (Task 3 test `mail_and_feeds_come_back_as_they_were`).
3. A scene in flight blocks the save; the save happens on the map once the scene is over (Task 5 test `a_scene_in_flight_waits_for_the_save`).
4. A truncated or foreign `.run` file is listed as unreadable and never panics; a folder that can't be written logs `couldn't save · …` and keeps the old file (Task 3 `a_broken_file_lists_as_unreadable`, Task 5 `a_failed_save_says_so_and_keeps_the_old_one`).
5. A perma death run can't be switched off on reopening (Task 6 `a_reopened_run_keeps_its_flags`).

---

### Task 1: The world and the character can be written

**Files:**
- Modify: `Cargo.toml` (workspace: `rand_chacha = { version = "0.9", features = ["serde"] }`; add `rmpv = "1"`), `crates/sim/Cargo.toml`, `crates/game/Cargo.toml`, `crates/citygen/Cargo.toml` (dev-deps `rmp-serde.workspace = true`)
- Modify: `crates/citygen/src/construct.rs` (`Construct`, `Guard`), `crates/citygen/src/interior/floor.rs` (`RoomKind, Role, Station, Room, Exit, Floor`), `crates/citygen/src/interior/plan.rs` (`FloorPlan, Turn, ClinicPlan`), `crates/citygen/src/interior/home.rs` (`Home`)
- Modify: `crates/game/src/character.rs` (`CopyId, ItemCopy, Stack, Character`), `crates/game/src/ground.rs` (`Ground`), `crates/game/src/run.rs` (`Target, HackTarget, Run`)
- Modify: `crates/sim/src/people.rs` (`PersonId, CarId, HouseholdId, Work, Goal, Segment, Activity, Dest, Leg, Trip, Whereabouts, Person, Car`), `crates/sim/src/world.rs` (`TripRequest, Displaced, Date, World`)

**Interfaces:**
- Produces: `Serialize + Deserialize` on every type above. `sim::World::rebuild(&mut self, city: &citygen::City, people: &sim::People)`: refills the fields skipped on save (`places` = `Places::derive(city)`, `labels`, `night_lines` as `World::seed` builds them). Those three are `#[serde(skip)]` (give their types `Default`).

- [ ] **Step 1: Write the failing tests**
  - `sim::world::tests::a_world_comes_back_and_goes_on_the_same`: seed the seed-8 medium city's world, advance 3,000 ticks, write it with `rmp_serde::to_vec_named`, read it back, `rebuild`, then advance both the original and the copy 3,000 more ticks; every person's `whereabouts` and `minute()` are equal.
  - `game::character::tests::a_character_comes_back`: a character with stacks, a worn item and stickers, written and read back, `==` the original.
  - `game::ground::tests::the_ground_comes_back`: two piles on two places, round trip, same stacks on the same tiles.
  - `citygen::construct::tests::a_construct_comes_back_with_its_guards`: move a guard, round trip, the guard's pos and standing are the moved ones.
- [ ] **Step 2: Run them to see them fail** — `cargo test --workspace --target-dir target/smoke comes_back` → compile errors (no derives).
- [ ] **Step 3: Derive, skip and rebuild** as listed. Fields of private types derive in their module. No `#[serde(skip)]` beyond `World.places/labels/night_lines`.
- [ ] **Step 4: Run the tests** — same command → PASS. Then the five checks.
- [ ] **Step 5: Commit** — `save: the world and the character can be written`.

### Task 2: The run's own state can be written

**Files:**
- Modify (derives; private types derive in their module): `walker/mod.rs` (`Player`; `unseen` is `#[serde(skip)]`, recreated as it is today on first use), `net.rs` (`NetState, Hack, Engagement, Actor, Phase, Ice, IceMode, Roll, RollEcho`; the cfg(test) `force_roll` is `#[serde(skip)]`), `backdoors.rs` (8 types), `jobs/mod.rs` (8), `credentials.rs` (`Journal, Code, Source, Mail, KeypadState`), `helix/{mod,shell,vfs}.rs` (10), `missions/{mod,visitors,news,daze}.rs` (`Missions, TrayOffer, Tab, Run, RunState, Body, Left, FeedView, Call, Visitor, NewsRun, Daze`), `messages.rs` (`MessageLog, Entry, Kind`), `foes.rs` (`Foe, FoeMode`), `bots.rs` (`BotOut`), `blast.rs` (`Live`), `constructs.rs` (`Constructs`), `interiors.rs` (`Interiors, Homes`), `track/mod.rs` (`Source, ListingId, Track`), `app.rs` (`FxInstance`), `mail_mark.rs` (`MailPill`), `clinic.rs` (`Chair`)
- Create: `crates/strata-zero/src/runs/mod.rs` (module root, `pub mod testutil` under cfg(test)), `crates/strata-zero/src/runs/testutil.rs`
- Modify: `crates/strata-zero/Cargo.toml` (dev-dep `rmpv.workspace = true`), `main.rs` (`mod runs;`)

**Interfaces:**
- Produces: `runs::testutil::same<T: Serialize>(a: &T, b: &T) -> bool` — both written with `to_vec_named`, read as `rmpv::Value`, maps sorted by key bytes recursively, compared. `runs::testutil::round<T: Serialize + DeserializeOwned>(t: &T) -> T`.
- Shapes fixed here: `missions::Left.heading` becomes `String`. `Missions.scene` and `Missions.asides` are `#[serde(skip)]` (a scene in flight is never saved, Task 5; asides last seconds and carry a ratatui `Color`). `Track` writes only `source` and `label`; `walk` and `pulses` are `#[serde(skip)]` and `track::refollow(st: &mut AppState)` rebuilds them (the same work `track::tick` does when the walk is missing).

- [ ] **Step 1: Write the failing tests** in `runs/testutil.rs` tests:
  - `every_run_part_comes_back`: from `seren_has_written()`-like state (copy the setup from `screens/shots.rs`, it is private), assert `same(&x, &round(&x))` for `player`, `net`, `backdoors`, `journal`, `helix`, `jobs`, `missions`, `log`, `interiors`, `constructs`, `ground`, `character`.
  - `the_net_comes_back_jacked_in`: from the `on_the_open_net` setup, `player` (with `body`), `net` and `constructs` come back `same`.
  - `a_tracked_walk_comes_back`: from `tracked_city()`, round-trip `track`, call `track::refollow`, the track's target and next floor are the original's.
- [ ] **Step 2: Run them to see them fail** — `cargo test -p strata-zero --target-dir target/smoke runs::` → compile errors.
- [ ] **Step 3: Derive and reshape** as listed.
- [ ] **Step 4: Run the tests** → PASS; the five checks.
- [ ] **Step 5: Commit** — `save: the run's state can be written`.

### Task 3: The run file: snapshot, restore, header, list

**Files:**
- Create: `crates/strata-zero/src/runs/snapshot.rs`, `crates/strata-zero/src/runs/file.rs`, `crates/strata-zero/src/runs/describe.rs`
- Modify: `app.rs` (new fields), `cli.rs` (`start_state` turns saving on), `credentials.rs` (run-made items), `runs/mod.rs`

**Interfaces:**
- New `AppState` fields:
  - `saves: Option<PathBuf>` — `None` in `new()`; `start_state` sets `Some(runs::default_dir())`.
  - `run: Option<runs::RunMeta>` — `RunMeta { id: RunId, started: u64 /* st.tick */, born: u64 /* world minutes since the city's start */ }`; saved.
  - `save_due: bool`, `last_save: Option<(u64 /* tick */, SystemTime)>` — not saved.
- `runs::RunId(pub u64)`, `Display` as 16 lowercase hex. `RunId::fresh(tick: u64) -> RunId` (SystemTime nanos hashed with the tick, as `Title::fresh_params` does).
- `runs::default_dir() -> PathBuf` = `"runs"`.
- `runs::RunHeader { id, handle, profession /* display name */, level: u8, city, seed: u64, day: u32, minute: u16, place: String, mission: Option<String>, objective: Option<String>, perma: bool, credits: u32, chrome: Vec<String> /* names */, talents: u32, played: u64 /* seconds */, saved: u64 /* unix secs */, death: Option<Death> }`; `runs::Death { cause: String, day: u32, minute: u16, at: u64 /* unix secs */, lasted: u64 /* city minutes */ }`.
- `runs::snapshot::SnapshotRef<'a>` (Serialize, `&'a` fields) and `Snapshot` (Deserialize, owned): the same field names. `snapshot(st: &AppState) -> SnapshotRef<'_>` and `restore(st: &mut AppState, s: Snapshot)` each destructure `AppState` exhaustively, no `..`; every field left out is bound to `_` with a comment saying why (UI, setting, dev, content, never `Some` at a save). Saved: the spec §1 list plus `params, chair, fx, mail_seen, mail_pill, run, interiors, constructs, track`, and `keycards: Vec<game::Item>` (the run-made `keycard:*` items of `st.content`). `rest` is left out (never `Some` at a save), `home` is rebuilt.
- `runs::snapshot::after_restore(st: &mut AppState)`: `world.rebuild(city, &st.people)`, `home = HomeRig::find(city)`, `track::refollow`, keycards put back into `st.content` (`credentials::put_back(&mut Content, Vec<Item>)`, after `credentials::run_items(&Content) -> Vec<Item>` on save).
- `runs::file::write(dir: &Path, st: &AppState) -> anyhow::Result<PathBuf>` (header + body, `.tmp` then rename; sets nothing on `st`), `runs::file::read(path: &Path) -> anyhow::Result<(RunHeader, Snapshot)>`, `runs::file::header(path: &Path) -> anyhow::Result<RunHeader>` (reads no body), `runs::file::rewrite_header(path, &RunHeader) -> anyhow::Result<()>` (body bytes kept).
- `runs::RunInfo { path: PathBuf, header: Result<RunHeader, String> }`; `runs::list(dir: &Path) -> Vec<RunInfo>` (every `*.run`, an unreadable one carries `Err(reason)`; a newer format version reads `Err("this build can't open it")`); `runs::living(dir) -> Vec<RunInfo>` (no death, newest `saved` first, unreadable last); `runs::dead(dir) -> Vec<RunInfo>` (newest `death.at` first).
- `runs::describe::header(st: &AppState, death: Option<Death>) -> RunHeader`; helpers used by the screens: `place_line(st) -> String` (`{building} {level}F · at home` in the flat, `{building} {level}F` on a floor, the district's name on the street, `on the Net` jacked in), `story(st) -> (Option<String>, Option<String>)` (title and objective of the first mission in book order that is under way), `played(secs) -> String` (`3h 12m`, `40m`), `lasted(minutes) -> String` (`3 days`, `19 hours`, `40 min`), `credits(n) -> String` (`¤ 1,240`), `saved_at(unix, now) -> String` (`today 10:44` on the same UTC date, else `YYYY-MM-DD HH:MM`, as `files::fmt_time`), `first_part(objective) -> &str` (up to the first ` · `).

- [ ] **Step 1: Write the failing tests** (`runs/file.rs` tests, temp dirs):
  - `a_run_comes_back_whole`: from the morning-after state, `write` then `read` then `restore` into a fresh `AppState::new()` then `after_restore`; `same(&snapshot(&a), &snapshot(&b))`.
  - `reopens_on_the_floor_it_was_saved_on`: saved standing in the flat; after restore, `MapScreen::walking(&mut st)` draws the flat's floor (the drawn text contains the bed's glyph row the original draws, compare `draw_stack` text of original and restored).
  - `mail_and_feeds_come_back_as_they_were`: saved with three mails announced and the welcome feed queued; after restore `st.mail_seen == 3`, and the first map update opens the feed once.
  - `a_broken_file_lists_as_unreadable`: a file cut in half and a file of random bytes named `*.run` list with `Err`, nothing panics; a header with version 2 lists `Err("this build can't open it")`.
  - `the_header_reads_without_the_body`: `header(path)` on a file whose body bytes are garbage still returns the header.
  - `describe` unit tests for `played`, `lasted`, `credits`, `saved_at`, `first_part` with the spec's values (`3h 12m`, `3 days`, `19 hours`, `40 min`, `¤ 1,240`, `Bring it to Coz`).
  - `a_new_state_saves_nowhere`: `AppState::new().saves == None`.
  - Print the file size of the morning-after save (`eprintln!`), and assert it is under 4 MB.
- [ ] **Step 2: Run them to see them fail** — `cargo test -p strata-zero --target-dir target/smoke runs::` → FAIL.
- [ ] **Step 3: Implement** as in Interfaces.
- [ ] **Step 4: Run the tests** → PASS; the five checks.
- [ ] **Step 5: Commit** — `save: the run file, its snapshot and its header`.

### Task 4: Story positions by id, and ids this build no longer has

**Files:**
- Create: `crates/strata-zero/src/runs/ids.rs`, `crates/strata-zero/src/runs/fixtures/run-v1.run`
- Modify: `missions/mod.rs`, `missions/call.rs`, `missions/feed.rs`, `missions/news.rs`, `missions/scene.rs`, `credentials.rs` (`Mail.story`), `track/mod.rs` (`Source::Story`), `runs/file.rs`, `runs/snapshot.rs`, `crates/game/src/character.rs`, `crates/game/src/ground.rs`

**Interfaces:**
- `runs::ids::StepRef { pub mission: usize, pub step: usize }` (Copy, Eq, Ord, Hash). Every `(mission, step)` pair the run keeps becomes a `StepRef`: `Call`, `FeedView`, `NewsRun`, `TrayOffer`, the entries of `Missions.scenes` and `Missions.news_ends`, `credentials::Mail.story: Option<StepRef>`, `track::Source::Story(StepRef)`. Its serde writes `{ mission: "<mission id>", step: "<step id>" }` through the book in scope.
- `runs::ids::with_book<R>(book: &missions::Book, f: impl FnOnce() -> R) -> R`: puts the book in a thread-local for the duration; `file::write` and `file::read` call through it with `st.book` (for `read`, a `&Book` argument: `read(path, book)`). Without a book in scope a `StepRef` fails to serialize with an error, never a panic.
- `Missions.runs` is written as a map from mission id to its run, and `Run.at` / `Run.ended` as step ids (`at` past the last step writes `"end"`), through a `#[serde(with = "runs::ids::runs")]` on the field.
- On read, against this build's book: a mission id it lacks is dropped; a book mission with no saved run gets the run `missions::begin` would make; a `StepRef` whose mission or step is gone drops the thing holding it (the call, the feed, the scene, the mail's story link); a run whose `at` step is gone is set to its end (done). Each drop adds a line to the returned `Vec<String>` of losses.
- `game::Character::forget_unknown(&mut self, c: &Content) -> Vec<String>` and `game::Ground::forget_unknown(&mut self, c: &Content) -> Vec<String>`: drop every item, chrome, talent, skill and sticker id `c` lacks; return the ids.
- `runs::file::read(path, book) -> Result<(RunHeader, Snapshot, Vec<String>)>`; `after_restore` runs `forget_unknown` and logs every loss as `lost in the update · {what}` (a warning line).

- [ ] **Step 1: Write the failing tests** (`runs/ids.rs` tests):
  - `story_positions_are_written_by_id`: save the morning-after state; the body read as `rmpv::Value` holds the string `"morning_after"` and the current step's id, and no `runs` array in book order.
  - `a_step_added_before_where_you_are_doesnt_rewind`: save at the doctor's `tray` step; insert a new step at the front of `doctor` in a cloned book; read with that book: `doctor`'s `at` points at `tray`'s new index, every ended step is still ended by id.
  - `a_mission_gone_from_the_book_is_dropped_and_said`: remove `ledger` from a cloned book; read: no panic, losses mention it.
  - `an_item_gone_from_the_content_is_dropped_and_said`: a character carrying an item id the content lacks, saved, restored: the item is gone, the log has `lost in the update · …`, opening the deck's kit draws without panic.
  - `the_first_saved_run_still_opens`: `include_bytes!("fixtures/run-v1.run")` written to a temp dir, `read` with the current book succeeds and `restore` runs. The fixture is written once by the ignored test `write_run_fixture` (a SMALL seed-8 city, the morning-after state) and committed; never regenerate it to make this test pass: a field added later gets `#[serde(default)]` instead.
- [ ] **Step 2: Run them to see them fail** → FAIL.
- [ ] **Step 3: Implement** as in Interfaces; `write_run_fixture` writes `crates/strata-zero/src/runs/fixtures/run-v1.run` (run it once: `cargo test -p strata-zero --target-dir target/smoke -- --ignored write_run_fixture`).
- [ ] **Step 4: Run the tests** → PASS; the five checks.
- [ ] **Step 5: Commit** — `save: story positions by id, losses said`.

### Task 5: When it saves

**Files:**
- Create: `crates/strata-zero/src/runs/when.rs`
- Modify: `screens/explore.rs` (frame hook, `s` key, the status row's mark), `screens/pause.rs`, `screens/broadcast.rs` (`jack_in` NewGame), `rest.rs` (`wake`), `missions/mod.rs` (`run_step`), `window/mod.rs` (close, Cmd+Q), `screens/title.rs` and `screens/mod.rs` (dev starts: no `run` on the lot)

**Interfaces:**
- `runs::when::can_save(st: &AppState) -> bool`: `st.saves` is `Some`, `st.run` is `Some`, a city that isn't the lot (`save::refusal` is `None`), `st.player` is `Some`, `st.net.death` is `None`, `st.rest` is `None`, `st.missions.scene` is `None`.
- `runs::when::save_now(st: &mut AppState) -> bool`: if `can_save`, `file::write` to `st.saves`; on success sets `st.last_save = Some((st.tick, SystemTime::now()))`, clears `save_due`, returns true; on error logs a warning `couldn't save · {reason}` and returns false (the old file stands).
- `runs::when::frame(st: &mut AppState)`: called at the end of `MapScreen::update`; saves when `save_due` or `st.tick - last_save tick >= 9000` (and `can_save`).
- `runs::when::begin_run(st: &mut AppState)`: `st.run = Some(RunMeta { id: RunId::fresh(st.tick), started: st.tick, born: <world minutes now> })`, then `save_now`. Called in `jack_in`'s NewGame arm after the map screen is built, and by the dev starts that enter a real city (not the lot).
- Due points: `rest::wake` and `missions::run_step` set `st.save_due = true`.
- Pause: SAVE GAME → `save_now` (no name form); LOAD GAME → `save_now` then push `RunsScreen::load` (Task 6; until then keep `Files::load`); QUIT TO TITLE and QUIT GAME → `save_now` first. Under the rows, FAINT: `last saved · just now` (< 60 s), `· 1 min ago`, `· N min ago`, `· N h ago` (≥ 60 min), from `last_save`'s wall time; no line when `None`. In dev mode the pause menu adds SAVE CITY (today's `Files::save`) after SAVE GAME.
- The map's `s` key saves the run (`save_now`) instead of opening the city form.
- The mark: on the map's bottom status row, after the place text, ` :: ` (FAINT) and `SAVED` (PICK, bold), while `st.tick - last_save tick < 90`.
- Window: `CloseRequested` and `Shortcut::Quit` call `runs::when::save_now(&mut runner.st)` before `el.exit()`.

- [ ] **Step 1: Write the failing tests** (temp dir as `st.saves`):
  - `a_new_run_saves_as_it_starts`: NEW GAME through `jack_in`: one `*.run` in the dir, its header's handle the build's.
  - `waking_and_a_story_step_save`: `rest::wake` then a map frame → the file's `saved` stamp moves; same after a `run_step`.
  - `five_minutes_of_play_save`: 9,000 frames on the map with nothing else happening → saved once.
  - `a_scene_in_flight_waits_for_the_save`: `save_due` with `missions.scene` running: no write; after the scene ends and a map frame, written.
  - `a_failed_save_says_so_and_keeps_the_old_one`: `st.saves` pointing at a file (not a dir): `save_now` is false, the log's last line starts `couldn't save · `.
  - `the_lot_never_saves`: the dev COMBAT start: `st.run == None`, nothing written.
  - `pause_quits_save_first` and `pause_shows_when_it_last_saved` (`last saved · 2 min ago` with `last_save` two minutes back).
  - `the_mark_shows_for_three_seconds`: after a save the status row reads `:: SAVED`; 90 ticks later it doesn't.
- [ ] **Step 2: Run them to see them fail** → FAIL.
- [ ] **Step 3: Implement** as in Interfaces.
- [ ] **Step 4: Run the tests** → PASS; the five checks.
- [ ] **Step 5: Commit** — `save: the run saves itself`.

### Task 6: Opening a run: CONTINUE, LOAD GAME, the broadcast

**Files:**
- Create: `crates/strata-zero/src/screens/runs.rs`
- Modify: `screens/title.rs`, `screens/broadcast.rs`, `screens/mod.rs`, `screens/explore.rs` (`MapScreen::resumed`), `screens/pause.rs` (LOAD GAME → the runs screen), `broadcasts.rs`, `flavor/broadcasts.ron`, `screens/shots.rs`

**Interfaces:**
- `runs::open(st: &mut AppState, path: &Path) -> anyhow::Result<()>`: `file::read` with `st.book`, `restore`, `after_restore`, log the losses; sets `last_save` to the header's time.
- `screens::runs::RunsScreen::load(st: &AppState) -> Self` and `::hall(st: &AppState) -> Self` (Task 7 draws the hall); lists from `st.saves` (empty when `None`). LOAD GAME per spec §5: the files panel retitled, header row, rows, the `PERMA` tag, the card, keys `↑↓ select · space continue · esc back`, `No runs yet.`, unreadable rows dim with their reason and space does nothing on them. Space on a row: `runs::open`, then push `Broadcast::resume(st)`.
- `Broadcast::resume(st: &AppState) -> Self`: `Entry::Resume`, the LoadGame pick, the run's handle and flags, no `p`/`t` switches (their hint lines not drawn). `jack_in`'s Resume arm: switch to `screens::resume_screen(st)` = `MapScreen::resumed(st)`, which is `walking` with `last_tick = st.tick` (no `begin`, no `enter_city`).
- `Broadcast.during: Option<String>` (`#[serde(default)]` in the RON), checked at load against the book's mission ids (`BroadcastError::UnknownMission(String)`; `Broadcasts::load` takes the book's ids). `Broadcasts::pick_for(&self, when: When, seed: u64, under_way: impl Fn(&str) -> bool) -> &Broadcast`: among `when`'s, those whose `during` is under way if any, else those without `during`. `courier` gets `during: Some("morning_after")`. Under way = the mission's run has begun and isn't done.
- Title rows: `Continue` (only with a living run), `NewGame`, the dev starts, `Load`, `LoadCity` (dev only: today's `Files::load`), `Hall`, `Quit`; the cursor starts on CONTINUE when present. While CONTINUE is selected, two rows under the menu, centred: `{HANDLE}` (CYAN bold) ` · {PROFESSION} · {city} · day {d} {hh:mm} · ` (FAINT) `{first_part(objective)}` (DIM). CONTINUE opens the newest living run as LOAD GAME's space would.

- [ ] **Step 1: Write the failing tests** (temp dir):
  - `continue_opens_the_newest_run`: two runs saved, CONTINUE on the title restores the newer; after the broadcast's space the map's player stands where it was saved.
  - `a_reopened_run_keeps_its_flags`: a perma run reopened: `p` on its broadcast changes nothing; `st.perma_death` stays true; the broadcast text has no switch hints.
  - `load_game_lists_living_runs_only`: one living, one flatlined (header with a death): one row; unreadable file: a dim row, space on it does nothing.
  - `the_courier_airs_only_in_the_morning_after`: `pick_for(LoadGame, ..)` returns `courier` while `morning_after` is under way, never otherwise.
  - `no_continue_without_a_run`: `st.saves` empty dir → rows NEW GAME, LOAD GAME, HALL OF FAME, QUIT.
  - Shots `save_title_shots`, `save_load_shots` (120×40) beside the existing ones.
- [ ] **Step 2: Run them to see them fail** → FAIL.
- [ ] **Step 3: Implement** as in Interfaces.
- [ ] **Step 4: Run the tests** → PASS; the five checks.
- [ ] **Step 5: Commit** — `save: CONTINUE and LOAD GAME open a run`.

### Task 7: The flatline and the HALL OF FAME

**Files:**
- Create: `flavor/flatline.ron`
- Modify: `screens/flatlined.rs`, `screens/explore.rs` (the flatline switch), `screens/runs.rs` (the hall), `screens/title.rs` (HALL OF FAME row), `runs/when.rs`, `screens/shots.rs`

**Interfaces:**
- `flavor/flatline.ron`: `(taunts: [...])` with the spec's six lines, in order; loaded like the other flavor files, checked non-empty.
- `runs::when::flatline(st: &mut AppState, cause: &str) -> Option<RunHeader>`: perma death on: writes the run once more (allowed though `net.death` is set), its header with `death: Some(Death { cause, day, minute, at: now, lasted })`, then `st.run = None`; returns the header. Perma death off: writes nothing, returns the run's current header from disk (`file::header`), or `None`.
- `Flatlined::new(cause: String, handle: &str, level: u8, tick: u64, end: End)` where `End::Perma { taunt: String }` (the taunt picked by `hash64(run id, 0xF1A7)` over the list) or `End::Reload { path: PathBuf, line: String /* "day 2 07:40 · at home" */ }` or `End::Title` (no save to go back to).
  - Perma: the logo, the cause, `{HANDLE} · level {n}`, then the taunt (PINK) on the row the old `any key · title` used, `any key · title` two rows lower.
  - Reload: `reload last save point · {line}` (DIM), two rows lower `space · load last save     esc · title` (FAINT). Space: `runs::open(path)` then switch straight to `MapScreen::resumed` (no broadcast). Esc: the title.
  - The resets the screen makes today stay for the paths to the title.
- HALL OF FAME per spec §6 in `RunsScreen::hall`: header RUNNER, BUILD, LVL, CITY, LASTED, THE END; rows from `runs::dead`; the card (`FLATLINED` RED bold; cause, `day {d} {hh:mm}`, date PINK; `{city} · lasted {lasted} · played {played}`; `got as far as  {mission} · {first_part(objective)}`; `left behind    {credits} · {chrome names} · {n} talents`); keys `↑↓ select · esc back`; empty `Nobody you ran has flatlined. Yet.`; space does nothing.

- [ ] **Step 1: Write the failing tests** (temp dir):
  - `a_perma_flatline_goes_to_the_hall`: perma run, black ICE kills: the file stays, its header has the death; `runs::living` is empty, `runs::dead` has it; the screen shows one of the taunts and no `HALL OF FAME`.
  - `a_flatline_without_perma_reloads`: perma off, saved at the flat, then killed on the Net: the screen reads `reload last save point · day 1 22:01 · at home` (the saved header's values); space puts the player back in the flat with the saved credits.
  - `the_hall_lists_the_dead_newest_first` and `the_hall_is_empty_at_first` (`Nobody you ran has flatlined. Yet.`).
  - `the_taunts_load`: six lines, the first `somebody's already wearing your jacket.`
  - Shots `save_hall_shots`, `save_flatline_shots` (both ends), `save_pause_shots` (with the line and the mark).
- [ ] **Step 2: Run them to see them fail** → FAIL.
- [ ] **Step 3: Implement** as in Interfaces.
- [ ] **Step 4: Run the tests** → PASS; the five checks; render the shots (`SHOT_DIR=<scratch> cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored save_`) and look at them against the mock.
- [ ] **Step 5: Commit** — `save: the flatline and the HALL OF FAME`.
