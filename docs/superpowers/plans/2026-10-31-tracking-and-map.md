# Tracking and the Deck's MAP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One track at a time (a job, the story, a listed place or a bare spot), its walk glowing on the street and on a new half-block city map in the deck, cleared the moment its reason is gone.

**Architecture:** A new `track` module turns a `Source` into a `Target`, a `Target` into a `Leg` (the walk in the place you are in), and clears itself on arrival or when its source ends. An overlay paints the leg's backgrounds on the street. A new `screens/atlas.rs` replaces the deck's MAP tab with a pixel map, a free cursor and a folding places pane fed by `track/listings.rs`.

**Tech Stack:** Rust, ratatui buffers, crossterm `KeyCode`s, `citygen::pathing::astar`. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-10-31-tracking-and-map-design.md` (read it; its "Calls I made" and the builder-check rulings are binding). Mock reference: `docs/superpowers/specs/2026-10-31-job-tracking-mock/map2.html` and `gen2.py`/`gen3.py`.

## Global Constraints

- Build and test into `--target-dir target/smoke`, never `target/`.
- Every task ends green on: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --target-dir target/smoke -- -D warnings`, the same with `--features strata-zero/window`, and `cargo test --workspace --target-dir target/smoke` (the window feature's tests run in Tasks 4, 7, 8 and 9 too).
- Colours: `ui::chrome::{CYAN, MAGENTA, DIM, PANEL_BG}` (and `CYAN_RGB`, `MAGENTA_RGB`, `PANEL_BG_RGB`), `ui::hud::AMBER`; new ones are exact `Color::Rgb` values from the spec.
- The street overlay only ever calls `set_bg`.
- Draw through ratatui only; read `KeyCode`s; nothing for the window alone.
- New glyphs (`◆ ⌂ ≈ ¢ ▣ ⌖ ▴ ▾ ◂ ▸ ▕`, box corners) must pass `window::font::tests::every_drawn_character_has_a_glyph`.
- Log wording exactly as spec §3; game wording terse, in-world, lower-case like the rest of the log.
- No Pondsmith/RTG/CDPR proper nouns.
- Commits: small, imperative, prefixed like the history (`track: …`, `atlas: …`, `shots: …`), each ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Match the surrounding code: doc comments short and plain, tests beside the code in `#[cfg(test)] mod tests`, test states from `crate::jobs::testutil::state_home()` (seed 8; call `MapScreen::walking(&mut st)` before moving the player) and keys from `crate::screens::testutil::key`.

## Review Focus

1. **A track whose target is in the building you are already in** (tracking the story's "Get home" from the lobby of your own megablock): the leg must go floor → lift → the flat, never out to the street. Test in Task 2.
2. **The explore screen's view place differs from yours** (the free camera on another stratum): the glow draws only where `leg.place` is the place on screen. Test in Task 4.
3. **The smallest terminal (100×30)**: pane width 30, the card and the edge tag clamped inside the map, the strip on the edge. Tests in Tasks 4 and 8, shots in Task 9.
4. **A job dropped from the JOBS tab while it is tracked**: the track clears within the same `track::tick`, with exactly one log line. Test in Task 3.
5. **The map's edges and a small stratum**: the view clamps at every edge, and a stratum smaller than the view at `fit` sits centred with the cursor still reaching every tile. Test in Task 7.

---

### Task 1: Sources and targets

**Files:**
- Create: `crates/strata-zero/src/track/mod.rs`
- Modify: `crates/strata-zero/src/main.rs` (`mod track;`), `crates/strata-zero/src/app.rs` (`pub track: Option<crate::track::Track>`, `None` in `new` and in `enter_city_at`), `crates/strata-zero/src/walker/mod.rs` (drop `#[cfg(test)]` from `vis_at`), `crates/strata-zero/src/interiors.rs` (`pub(crate) fn level_for`)

**Interfaces:**
- Produces:
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum ListingId { Building(BuildingId), Person(PersonId), Job(JobId), Story(usize) }`
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Source { Job { job: JobId, objective: usize }, Story { mission: usize, step: usize }, Listing(ListingId), Spot { place: Place, pos: Pos, radius: i32 } }`
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Target { Spot { place: Place, pos: Pos, radius: i32 }, Building(BuildingId), Person(PersonId) }`
  - `#[derive(Clone, Debug, PartialEq, Eq)] pub enum Resolved { Target(Target), Gone(String), Nowhere }`
  - `pub struct Track { pub source: Source, pub label: String }` (Task 3 adds a private leg cache; keep the struct non-exhaustive in spirit: build it only through `track::set`).
  - `pub fn resolve(st: &AppState, source: &Source) -> Resolved` — spec §1 table. `Gone` text is the reason without the label: `"done with"` for a job not Taken, an objective ticked, a story run not Open or moved past `step`; `"{name} is gone"` for a dead listing person without a body. `ListingId::Job`/`Story` → `Nowhere`.
  - `pub fn locate(st: &AppState, t: Target) -> Resolved` — a `Person` made concrete: on a stratum → `Spot { Street place they stand in, tile, radius 1 }`; inside `b` → `Building(b)`; dead with a story body → `Spot` at the body, radius 1; dead without → `Gone("{name} is gone")`. `Spot`/`Building` pass through.
  - `pub fn label(st: &AppState, source: &Source) -> String` — job title, mission title, listing name (`clinic::name` for clinics, `net::building_name` for other buildings, the person's name), or `"{district} · spot"` (`"spot"` without a district).
  - `pub(crate) fn flat_tile(st: &AppState) -> Option<(Place, Pos)>` — your flat's floor and the walkable tile inside `rig.home.rect` nearest the flat's door (spec §1).

- [ ] **Step 1: Write the failing tests** in `track/mod.rs`:
  - `job_objective_zero_resolves_by_kind` — real jobs from `jobs::generate::generate(&mut st, jobs::testutil::a_fixer(&st), KindTag::{Fetch,Copy,Plant,Deliver}, n)` (try counters until one is `Some`), pushed and taken (`st.jobs.push`, `st.jobs.take(id, 0)`); Fetch → `Spot { Floor{building, level}, tile, 1 }`, Copy/Plant → `Building(b)`, Deliver → `Person(to)`. (`jobs::testutil::sample_jobs` uses fake ids: do not use it here.)
  - `job_hand_in_resolves_to_the_fixer` — objective 1 → `Person(job.fixer)`.
  - `job_not_taken_or_objective_ticked_is_gone` — drop the job → `Gone("done with")`; a ticked objective 0 → `Gone("done with")`.
  - `story_steps_resolve_by_trigger` — with `missions::begin` and the welcome closed (`missions::end(&mut st, 0, 0)` as `shots::street_by_the_body` does): the `Sees(marlo)` step → `Person(marlo)`; force the run to the `home` step (`missions::force`) → `Spot` on your flat's floor inside the flat's rect, radius 0; the `ring` step (`After`) → `Nowhere`; a `Story` whose `step` is not `run.at` → `Gone("done with")`.
  - `listing_resolves_to_building_or_person` — `Listing(Building(b))` → `Building(b)`, `Listing(Person(p))` → `Person(p)`, `Listing(Job(_))` → `Nowhere`.
  - `locate_follows_a_person` — standing → `Spot` radius 1 at `world.pos_of`; inside → `Building(b)`; killed with a body (Marlo after the welcome) → the body's tile; killed without a body (`world.kill` on a fixer) → `Gone("{name} is gone")`.
  - `a_new_city_clears_the_track` — set `st.track = Some(..)` by hand, `enter_city_at` → `None`.
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke track::` — fails (module missing).
- [ ] **Step 3: Implement** the types and functions above. Use `st.jobs.get`, `missions::person_of`, `st.missions.bodies`, `st.world` (`pos_of`, `people()[i].whereabouts`, `is_dead`), `st.home` (`HomeRig::place()`, `.home.rect`). Story `At(Work { role, within })` → `Spot { Street, missions::cast::work_tile(st, person), within }` (make `work_tile` `pub(crate)` if it is not).
- [ ] **Step 4: Run** the tests — pass; run the Global Constraints checks.
- [ ] **Step 5: Commit** `track: sources resolve to targets`.

### Task 2: Legs across places

**Files:**
- Create: `crates/strata-zero/src/track/route.rs`
- Modify: `crates/strata-zero/src/track/mod.rs` (`pub mod route;`)

**Interfaces:**
- Consumes: `Target`, `Resolved`, `locate` (Task 1).
- Produces:
  - `#[derive(Clone, Debug, PartialEq, Eq)] pub struct Leg { pub place: Place, pub path: Vec<Pos>, pub goal: Pos, pub through: Through }` — `path` is the tiles after yours ending on `goal` (empty when you stand on it).
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Through { Arrive, Door(BuildingId), Lift(i32), Stairs { up: bool }, Exit, Connector(StratumKind) }`
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum NoLeg { JackedIn, NoWay }`
  - `pub fn leg(st: &AppState, target: Target) -> Result<Leg, NoLeg>` — `target` is concrete (`Spot` or `Building`; a `Person` is `locate`d by the caller, and `leg` returns `NoWay` for one).
  - `pub fn next_text(st: &AppState, leg: &Leg, target: Target) -> Option<String>` — `None` for `Arrive`; else `"door, then {floor label}"` (when the target is on a floor other than the door's level) or `"door"`, `"lift to {label}"`, `"stairs up"`/`"stairs down"`, `"out to the street"` (or `"out to the {stratum label lower-case}"`), `"up to the {Stratum}"`/`"down to the {Stratum}"`. Floor labels via `FloorPlan::label`.
  - `pub(crate) fn walk(map: &MapRef, from: Pos, to: Pos, doors_are_walls: bool, exits: &[Pos]) -> Option<Vec<Pos>>` — the shared A* (spec §2 door rule: on a stratum every `Door` except `to` is a wall; on a floor only the `exits` doors except `to`).
- [ ] **Step 1: Write the failing tests** in `route.rs`, each from `state_home()`:
  - `same_place_walks_to_the_tile` — a Street `Spot` 20 tiles off: `Arrive`, path ends on it, every step 4-adjacent and walkable.
  - `an_unwalkable_spot_walks_beside_it` — a `Spot` on a building's mass: the goal is the walkable tile beside it nearest you.
  - `street_to_a_building_goes_to_its_door` — `Building(clinic)` from the street: `Door(b)`, goal is its Street portal; `next_text` is `"door"`.
  - `a_walk_in_kind_goes_beside_its_footprint` — a noodle bar (no portals): goal is walkable and 4-adjacent to the footprint.
  - `a_walk_never_crosses_another_door` — no tile of a long street path is a `Door` except the goal.
  - `floor_to_another_floor_takes_the_lift` — inside the home megablock on 0F, target `Spot` on the flat's floor: `Lift(level)`, goal `plan.lift`; with the lifts locked down (`st.net` lockdown as the lift tests do) → `Stairs { up: true }`, goal `plan.stair`.
  - `home_from_the_lobby_stays_inside` (Review Focus 1) — on the megablock's 0F with `flat_tile` as target: `through` is `Lift`, never `Exit`.
  - `floor_to_the_street_takes_the_exit` — on 0F of a building, target on the Street: `Exit`, goal is an exit's `at`; on an upper floor: `Lift(0)` or `Stairs { up: false }`.
  - `a_fetch_behind_a_flat_door_is_reached` — a Fetch tile from `jobs::generate::first_fetch(&st)` (building, level, number, tile), the player landed on that floor's lift or stair tile (`interiors.ensure`, then set `place`/`pos`): `Arrive` path exists (flat doors walkable).
  - `between_strata_takes_a_connector` — standing on the Undercity at a public connector's tile with a Street target: `Connector(Street)`, the goal's connector serves both.
  - `a_construct_has_no_leg` — the player in `Place::Construct(b)` → `Err(JackedIn)`; on `Place::Stratum(Net)` → `Err(JackedIn)`.
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke track::route` — fails.
- [ ] **Step 3: Implement** spec §2 with `MapRef::for_place`, `TileMap::walkable`, `city.portals`, `interiors.plan_of`/`floor`, `level_for`, `st.net.lifts_answer(b, st.net.now(st))`, `city.connectors` (the cross-strata choice by one BFS per stratum, spec §2 "Between strata"; `citygen::pathing::bfs_nearest` or a local BFS).
- [ ] **Step 4: Run** the tests and the Global Constraints checks.
- [ ] **Step 5: Commit** `track: legs through doors, lifts and connectors`.

### Task 3: Setting, ticking and clearing

**Files:**
- Modify: `crates/strata-zero/src/track/mod.rs`, `crates/strata-zero/src/screens/explore.rs` (call `crate::track::tick(st)` right after `crate::missions::tick(st)`; clear `st.track` where the explore screen pushes `Flatlined`)

**Interfaces:**
- Consumes: Tasks 1–2.
- Produces:
  - `pub fn set(st: &mut AppState, source: Source) -> Result<String, String>` — resolves and locates; `Nowhere` → `Err("nothing to walk to yet")`; `Gone(_)` → `Err("nothing to walk to")`; `leg` `NoWay` → `Err("no way there from here")`; `JackedIn` → `Err("not from the net")`. On success replaces `st.track`, logs `⌖ tracking {label}` with `st.say` and returns the same text. An offered job is refused by the JOBS tab, not here.
  - `pub fn stop(st: &mut AppState)` — logs `⌖ stopped tracking {label}`; nothing when untracked.
  - `pub fn toggle(st: &mut AppState, source: Source) -> Result<String, String>` — `stop` when `is_tracked`, else `set`.
  - `pub fn is_tracked(st: &AppState, source: &Source) -> bool` — compares sources; a `Job` source matches any objective of the same job, a `Story` any step of the same mission.
  - `pub fn tick(st: &mut AppState)` — spec §3 in order: resolve (Gone → clear with `⌖ {label} · done with` or `⌖ {label} · {reason}`), locate, arrival (not for `Story`), then refresh the cached leg when `(player.place, player.pos, located target)` changed.
  - `pub fn leg_now(st: &AppState) -> Option<Result<&route::Leg, route::NoLeg>>` and `pub fn target_now(st: &AppState) -> Option<Target>` — what the last tick cached, for drawing.
  - `pub fn distance(st: &AppState) -> Option<usize>` — the cached leg's `path.len()`.
- [ ] **Step 1: Write the failing tests** (spec §3, one per row):
  - `set_replaces_and_logs` — set A, set B: `st.track` is B; the log's last line is `⌖ tracking {B}`.
  - `toggle_on_the_tracked_one_stops` — log `⌖ stopped tracking {label}`, `st.track` is `None`.
  - `set_refuses_and_keeps_the_old` — a `Story` on the `ring` step → `Err("nothing to walk to yet")`, the old track kept; an unreachable `Spot` (inside solid mass with no walkable neighbour reachable — use a tile walled off in the test, or a `Spot` on a floor of a building whose lift and stairs are both cut by test setup) → `Err("no way there from here")`.
  - `arriving_at_a_spot_clears` — a Street `Spot` radius 2; put the player 2 tiles off; `tick` → cleared, log `⌖ there · {label}`.
  - `arriving_at_a_building_clears` — `Building(b)`: player beside its door → cleared; another track: player on a floor of `b` → cleared.
  - `a_person_inside_clears_on_entering` — `Listing(Person(p))` with `p` inside `b`; player enters `b` (`interiors::enter`) → log `⌖ {name} is in here`.
  - `a_dropped_job_clears_once` (Review Focus 4) — track job, `st.jobs.drop_job(id)`, one `tick` → `None` and exactly one new log line `⌖ {title} · done with`; a second `tick` adds nothing.
  - `a_ticked_objective_clears` — track a Fetch, send `jobs::Event::Taken` for its tile → cleared.
  - `a_story_step_moving_on_clears_and_arrival_does_not` — track the story on the `body` step; put the player beside the body without ticking the missions → not cleared by arrival; run `missions::tick` until the step moves → `track::tick` clears with `⌖ {title} · done with`.
  - `a_dead_person_without_a_body_clears` — track a fixer listing, `world.kill` → `⌖ {name} · {name} is gone`.
  - `no_way_from_here_keeps_the_track` — make the cached leg fail after setting (move the player into a construct) → still tracked, `leg_now` is `Err(JackedIn)`.
  - `flatlining_clears` — drive the explore screen to its `Flatlined` push (as the existing flatline test does) → `st.track` is `None`.
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke track::` — new tests fail.
- [ ] **Step 3: Implement**; the cache lives in `Track` as a private field set only by `set`/`tick`.
- [ ] **Step 4: Run** the tests and the Global Constraints checks.
- [ ] **Step 5: Commit** `track: one at a time, cleared on arrival and when its reason ends`.

### Task 4: The floor glow and the edge tag

**Files:**
- Create: `crates/strata-zero/src/track/overlay.rs`
- Modify: `crates/strata-zero/src/track/mod.rs` (`pub mod overlay;`), `crates/strata-zero/src/screens/explore.rs` (push the overlay last in the stack at line ~2815–3065 when `leg.place == self.place`; draw the edge tag after the viewport renders)

**Interfaces:**
- Consumes: `leg_now`, `target_now`, `distance`, `Leg`, `Through`, `label`.
- Produces:
  - `pub fn glow(i: usize, n: usize, t: u64) -> f32` — spec §4 walk formula (base breathe, band head `0.9·t mod (n+24)`, tail 12, peak 0.85).
  - `pub fn ripple(dx: i32, dy: i32, t: u64) -> f32` — spec §4 (`d = |dx| + 1.6·|dy|`, rings, core at `d < 0.5`).
  - `pub fn mix(a: Color, b: Color, f: f32) -> Color` (or reuse an existing `Rgb` mix if `render` has one).
  - `pub struct TrackOverlay<'a> { pub leg: &'a Leg, pub arrive: bool }` implementing `render::viewport::Overlay` — `set_bg` only.
  - `pub fn draw_edge_tag(buf: &mut Buffer, area: Rect, origin: Pos, leg: &Leg, text: &str, t: u64)` — spec §4 tag, clamped inside `area`.
- [ ] **Step 1: Write the failing tests:**
  - `glow_breathes_and_the_band_runs` — `glow(i, n, t)` at the head equals 0.85 within 1e-3; far from the head lies in `[0.16, 0.28]`; the head moves forward with `t`.
  - `ripple_rings_out_and_fades` — the core pulses in `[0.15, 0.55]`; at `t` multiples where `r = 1`, a cell at `d ≈ 1.4` is brighter than one at `d = 0` minus core; beyond `d = 7` it is 0.
  - `the_overlay_only_changes_backgrounds` — render a 60×20 buffer from the street (`MapScreen::walking`, a tracked Street spot 15 tiles away) once without and once with `TrackOverlay`: every cell's symbol and fg are identical; at least one bg differs.
  - `the_glow_draws_only_in_its_place` (Review Focus 2) — move the explore screen's free camera to another stratum (`st.unlock_dev()`, as the free-camera tests do): no cell's bg changes.
  - `the_edge_tag_sits_where_the_walk_leaves` — a target 60 tiles north: the tag row is the map's second row, it contains `▴` and `· {m} m`; at 100×30 it stays inside the map rect (Review Focus 3).
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke track::overlay` — fails.
- [ ] **Step 3: Implement**; the tag text is `"{arrow} {label} · {m} m"`, or `"{arrow} {next_text} · {m} m"` when `through` is not `Arrive`.
- [ ] **Step 4: Run** the tests and the Global Constraints checks, window feature included.
- [ ] **Step 5: Commit** `track: the walk glows on the street`.

### Task 5: The JOBS pane and the JOBS tab

**Files:**
- Create: `crates/strata-zero/src/track/ui.rs`
- Modify: `crates/strata-zero/src/track/mod.rs` (`pub mod ui;`), `crates/strata-zero/src/missions/ui.rs` (`jobs_pane` prepends the track line), `crates/strata-zero/src/ui/sidebar.rs` and `crates/strata-zero/src/screens/explore.rs` (the two `story_lines:` counts become `missions::ui::story_line_count(st)`), `crates/strata-zero/src/screens/deck.rs` (JOBS: `t` key, hint row), `crates/strata-zero/src/jobs/ui.rs` and `crates/strata-zero/src/missions/ui.rs` (`detail` gains the track line)

**Interfaces:**
- Consumes: Tasks 1–4.
- Produces:
  - `pub fn pane_line(st: &AppState) -> Option<Line<'static>>` — spec §5: ` ⌖ {label} · {m} m` (+ ` · {next}`), or `· no way from here` / `· jacked in`.
  - `pub fn detail_line(st: &AppState, source: &Source) -> Line<'static>` — `⌖ tracking · t stops` (cyan) or `t track` (dim).
  - `pub fn story_line_count(st: &AppState) -> usize` in `missions::ui` — the track line (0 or 1) plus `pane_lines(st).len()`.
  - Deck JOBS: `KeyCode::Char('t' | 'T')` on a `Detail::Job(id)` row: taken → `track::toggle(Job { job, objective: first undone })`; offered → `st.warn("take it first")`; else nothing. On `Detail::Mission(m)`: open run → `toggle(Story { mission: m, step: run.at })`. Errors go to `st.warn`. JOBS hint: `Q/E tabs · ↑↓ select · 0-8 jump · space act · t track · tab close`.
- [ ] **Step 1: Write the failing tests:**
  - `the_pane_leads_with_the_track` — a tracked spot: `jobs_pane(st)[0]` reads ` ⌖ {label} · {m} m`; untracked: unchanged from today.
  - `the_pane_names_the_next_leg` — tracking a building's door target from inside another building's floor: the line contains `· out to the street`.
  - `the_story_count_includes_the_track_line` — `story_line_count` is `pane_lines().len() + 1` when tracked.
  - `t_on_a_taken_job_tracks_and_stops` — deck on JOBS, select the taken job's row, `t` → tracked; `t` → stopped.
  - `t_on_an_offered_job_says_take_it_first` — the warning, no track.
  - `t_on_the_story_tracks_its_step` — `Story { mission: 0, step: run.at }`.
  - `the_detail_says_the_track_state` — the job's card's last line is `⌖ tracking · t stops` when tracked, `t track` when not.
- [ ] **Step 2: Run** the tests — fail.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests and the Global Constraints checks.
- [ ] **Step 5: Commit** `track: the JOBS pane and tab show and set it`.

### Task 6: Listings

**Files:**
- Create: `crates/strata-zero/src/track/listings.rs`
- Modify: `crates/strata-zero/src/track/mod.rs` (`pub mod listings;`)

**Interfaces:**
- Consumes: `Source`, `ListingId`, `resolve`, `locate`, `route::walk`.
- Produces:
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Kind { Jobs, Home, Docs, Fixers, Bars, Noodle, Shops, Pawn, Arcades, Hotels }` with `pub const ALL: [Kind; 10]` (that order), `pub fn glyph(self) -> char`, `pub fn color(self) -> Color`, `pub fn label(self) -> &'static str` (`"jobs & story"`, `"home"`, `"ripperdocs"`, `"fixers"`, `"bars & clubs"`, `"noodle bars"`, `"shops & markets"`, `"pawnshops"`, `"arcades"`, `"hotels"`), `pub fn pinned(self) -> bool` (Jobs, Home). Glyphs and colours: spec §7 table.
  - `#[derive(Clone, Debug, PartialEq)] pub struct Listing { pub id: ListingId, pub kind: Kind, pub name: String, pub what: String, pub tile: Pos, pub building: Option<BuildingId>, pub person: Option<PersonId>, pub walk: Option<u32> }`
  - `pub fn listings(st: &AppState, stratum: StratumKind) -> Vec<Listing>` — spec §7 "Listings": sorted by `walk` (None last), ties by name.
  - `pub fn source_of(st: &AppState, l: &Listing) -> Source` — `Job`/`Story` sources for `◆` (first undone objective; the run's `at`), else `Source::Listing(l.id)`.
  - `pub fn walk_from(st: &AppState, stratum: StratumKind) -> Option<Pos>` — your tile on that stratum, or your building's exit landing onto it, or your body's tile while jacked in when it is there.
- [ ] **Step 1: Write the failing tests:**
  - `kinds_have_their_glyphs` — the table, and `pinned`.
  - `clinics_list_as_ripperdocs_under_their_front` — both seed-8 clinics listed `Docs`, named by `clinic::name`, `what` starting `ripperdoc`.
  - `fixers_list_at_their_spots` — the three fixers, `tile` their spot; a killed one is left out.
  - `the_job_and_the_story_are_listed_as_diamonds` — with a taken Deliver and the story on `body`: two `Kind::Jobs` listings with `ListingId::Job`/`Story`, `source_of` giving `Job`/`Story` sources.
  - `a_job_at_a_listed_building_is_two_rows` — a Plant job on a clinic: one `Docs` and one `Jobs` listing at the same tile.
  - `home_is_listed_at_its_door` — `Kind::Home` at the megablock's Street portal.
  - `listings_sort_by_walk_with_the_unreachable_last` — walks non-decreasing; a listing with `walk: None` (force one by testing a fixer whose spot is walled off, or assert the sort on a hand-made vec through a `sort_listings` helper) sorts after.
  - `walk_from_inside_uses_the_way_out` — from a floor of the home megablock, `walk_from(Street)` is the exit's landing.
- [ ] **Step 2: Run** — fail.
- [ ] **Step 3: Implement**; one BFS per call from `walk_from` over the stratum (Street size 240×120 is fine per call; the atlas calls it once per draw at most — cache in `AtlasState` in Task 8 if the profile shows it).
- [ ] **Step 4: Run** the tests and the Global Constraints checks.
- [ ] **Step 5: Commit** `track: the city's listings, nearest walk first`.

### Task 7: The atlas map

**Files:**
- Create: `crates/strata-zero/src/screens/atlas.rs`
- Modify: `crates/strata-zero/src/screens/mod.rs` (`pub mod atlas;`), `crates/strata-zero/src/app.rs` (`pub atlas: crate::screens::atlas::AtlasState`, default in `new`, reset in `enter_city_at`)

**Interfaces:**
- Consumes: `listings`, `Kind`, `Listing`, `track` state, `overlay::{glow, ripple}`.
- Produces:
  - `#[derive(Clone, Debug, Default, PartialEq)] pub struct AtlasState { pub stratum: Option<StratumKind>, pub cursor: Option<Pos>, pub origin: Pos, pub zoom: Zoom, pub kind: Option<Kind>, pub folded: bool, pub scroll: usize }` — `kind: None` is `all`; `cursor` in tiles.
  - `#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub enum Zoom { One, #[default] Two, Fit }` with `pub fn tiles(self, fit: i32) -> i32` (1, 2, `fit`; `Fit` never larger than needed, `Two` becomes `fit` when `fit < 2`).
  - `pub fn fit_scale(size: Size, cols: u16, rows: u16) -> i32` — the smallest `s` with `w/s ≤ cols` and `h/(2s) ≤ rows`.
  - `pub fn pixel(st: &AppState, stratum: StratumKind, x0: i32, y0: i32, s: i32) -> Color` — spec §7 "A pixel's colour" for the `s×s` block at tile `(x0, y0)`.
  - `pub struct MapLayout { pub area: Rect, pub s: i32, pub origin: Pos }` with `pub fn cell_of(&self, tile: Pos) -> Option<(u16, u16)>`, `pub fn tiles_of(&self, cell: (u16, u16)) -> TileRect` (the `s × 2s` tiles a cell covers).
  - `pub fn draw_map(st: &AppState, area: Rect, buf: &mut Buffer) -> MapLayout` — pixels, district names, markers (priority: you, tracked, pinned, rest; listings filtered by `st.atlas.kind`, pinned always), the cursor's corners, the tracked walk and ripple on pixels (spec §7). Starts the cursor (on the tracked target on this stratum, else on you) and clamps `origin` (cursor 4 cells inside; a stratum smaller than the area centred).
- [ ] **Step 1: Write the failing tests:**
  - `fit_scale_fits` — 240×120 into 81×26 → 3; into 66×21 → 4; a 30×10 stratum → 1.
  - `pixels_class_by_their_tiles` — a block of mass → the district's roof colour (remembered) / mixed at 0.22 (never seen) / ×1.3 (in sight); a road block → road ×1.9; water → `(14,40,72)`; void → `PANEL_BG`.
  - `the_cursor_starts_on_you_or_the_track` — no track: the cursor is your tile; tracking the clinic: the clinic's tile.
  - `the_view_clamps_and_centres` (Review Focus 5) — cursor at (0,0) → origin (0,0); at the far corner → origin `size − view`; a 30×10 lot (`City::lot`) at `Fit` → centred, and every tile maps to a cell.
  - `markers_stack_by_priority` — you over the tracked one over a pinned one over the rest, on a shared cell.
  - `the_walk_draws_on_pixels` — tracking the clinic, at least one map cell's colours differ from the untracked draw along the walk; none outside the map area.
  - `district_names_avoid_markers` — no district name cell overlaps a marker or a cursor corner.
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke screens::atlas` — fail.
- [ ] **Step 3: Implement**; the walk on pixels uses `glow`/`ripple` with `t = st.tick` and pixel coordinates (spec §7).
- [ ] **Step 4: Run** the tests and the Global Constraints checks, window feature included.
- [ ] **Step 5: Commit** `atlas: the city in half-block pixels`.

### Task 8: The atlas pane, keys and the deck

**Files:**
- Modify: `crates/strata-zero/src/screens/atlas.rs`, `crates/strata-zero/src/screens/deck.rs` (MAP takes its keys first, as `Kit` does; `atlas::draw` replaces `draw_map`; the MAP hint from `atlas::hint`; delete `Deck::draw_map`)

**Interfaces:**
- Consumes: Task 7.
- Produces:
  - `pub fn draw(st: &AppState, body: Rect, buf: &mut Buffer)` — the title row (`{CITY} · {STRATUM} · zoom {s} · lit: remembered · faint: the city grid`), the map, then the pane (or the strip and the card when folded), spec §7 "The pane".
  - `pub fn key(st: &mut AppState, code: KeyCode) -> bool` — spec §7 "Keys"; returns `false` for keys the deck keeps.
  - `pub fn hint(st: &AppState) -> &'static str` — the two hint rows of spec §7.
  - `pub fn under_cursor(st: &AppState, layout: &MapLayout) -> Option<Listing>` and `pub fn spot_under_cursor(st: &AppState, layout: &MapLayout) -> Option<Pos>` (radius = `s`, search within 8 tiles).
- [ ] **Step 1: Write the failing tests:**
  - `arrows_move_and_shift_moves_eight` — `l` moves the cursor one cell (`s` tiles), `L` eight cells; it stops at the stratum's edge.
  - `brackets_step_the_list_and_jump` — `]` from no selection picks the listing nearest the cursor, then the next by walk; the cursor lands on its tile; the scroll keeps it in view.
  - `angles_step_the_kind_and_pinned_stay` — `>` from all → `Jobs` … → `Hotels` → all; with `Docs`, the list holds the clinics plus the pinned rows.
  - `t_tracks_the_listing_or_the_spot` — the cursor on a clinic marker: `t` → `Listing(Building(b))` tracked; `t` again → stopped; the cursor on open ground: `Spot { place, pos, radius: s }`; on solid ground with nothing walkable within 8 → warning `nothing to walk to there`.
  - `t_on_a_diamond_is_the_jobs_tab_t` — the story's `◆`: the tracked source is `Story { mission: 0, step: run.at }`.
  - `zoom_steps_and_clamps` — `-` from Two → Fit; `+` from Fit → Two → One; `=` same as `+`.
  - `f_folds_and_the_strip_marks_it` — after `f`: the rightmost three columns of the body hold `▕`, `◂f`, the letters of `PLACES` and the count; `f` again → the pane is back.
  - `the_row_under_the_cursor_opens` — the selected row is on `SEL (20,32,44)` with `what` and the track state below it; off a listing, the footer says `CURSOR · …` and `t tracks this spot`.
  - `the_card_stays_inside` (Review Focus 3) — folded, cursor on a listing near the right edge at 100×30: the card's box lies inside the map area.
  - `the_pane_is_narrower_below_110_columns` — body width 98 → pane 30; 118 → 36.
  - `the_deck_hands_map_its_keys` — on the MAP tab, `q`/`e`/digits/tab still switch and close; `h` moves the cursor instead of anything else.
  - Delete the old `map_tab_shows_memory_or_no_signal` and `map_tab_ignores_row_keys` tests with `draw_map`; add `map_tab_without_a_city_says_no_signal` (`no signal — walk the city first`, kept from today).
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke screens::atlas screens::deck` — new tests fail.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests and the Global Constraints checks, window feature included.
- [ ] **Step 5: Commit** `atlas: the places pane, the keys, and the deck's MAP`.

### Task 9: Console, shots and docs

**Files:**
- Modify: `crates/strata-zero/src/screens/console.rs` (`track` in `COMMANDS` and `run`; completion `off`), `crates/strata-zero/src/screens/shots.rs` (the six shots), `docs/development.md` (a line for the `track` console command beside `story` if the doc lists console commands; the MAP tab where the doc describes the deck)

**Interfaces:**
- Consumes: everything.
- Produces: console `track` per spec §8; shots per spec §9 (helper `fn tracked_city() -> (AppState, MapScreen)`: `street_by_the_body()`, the walk from home to the body remembered as `tmp_mapdump` did — patch saved at the session scratchpad's `track/tmp-shots.patch` for reference — the player outside the home megablock's door).
- [ ] **Step 1: Write the failing tests:**
  - `track_shows_and_stops` — `track` → `nothing tracked`; `track 200 30` on the Street → tracked spot; `track` → a line starting `⌖`; `track off` → stopped.
  - `track_by_name_finds_a_listing` — `track mira` → the clinic; `track zzz` → `no listing called zzz`; an ambiguous prefix → `several: …`.
  - the shots are `#[ignore]` tests like the others; `every_drawn_character_has_a_glyph` runs on their output (run them with `SHOT_DIR` set to the folder that test reads, as for the holo shots).
- [ ] **Step 2: Run** — fail.
- [ ] **Step 3: Implement**; write the shots: `atlas_listing`, `atlas_ground`, `atlas_folded` (120×40 and 100×30), `track_street_near`, `track_street_far`, `track_jobs_tab`.
- [ ] **Step 4: Run** all five Global Constraints commands with and without the window feature, and the shots with `SHOT_DIR=<scratch> cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored atlas_ track_`; look at the `.png`s against `map2.html` and `round1.html` (concept C).
- [ ] **Step 5: Commit** `track: the console, the shots and the docs`.
