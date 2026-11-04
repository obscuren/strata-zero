# The overworld screen Implementation Plan (#22)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The overworld as a level of play: up from the open street onto the atlas, walk it tile by tile on the city's clock, go down onto any tile's open ground and walk across tiles, step back into the home city, all saved with the run.

**Architecture:** `crate::outside` holds the world beyond the city on `AppState.outside`: the world's params, where you are (`At`), what you have seen; the generated `Overworld` is rebuilt from the params on load. `screens::outside::OutsideScreen` is the play screen for the overworld and the open ground (the same HUD bands, panel and footer as the map, drawn by the same `hud` pieces). The map switches to it on `W`, and it switches back to the map through `MapScreen::resumed`, so the city's heavy draw never runs underneath. The home city is today's generator (NEW GAME at the large size), laid on a block of overworld tiles centred on the world's home city until #19 grows it into the world.

**Tech Stack:** Rust 2024, ratatui, crossterm, serde/MessagePack saves, `citygen::overworld` (+ `detail`).

**Spec:** `docs/superpowers/specs/2026-11-04-overworld-screen-design.md` (and `2026-11-04-world-tiles-design.md` for the ground one level down). Out of this plan: jacked in (#23), the deck MAP's world zoom (#24), cities grown into the world (#19), POIs one level down (#17), the world while you're away (#26).

## Global Constraints

- CLAUDE.md's two front ends: draw through ratatui only, read crossterm `KeyEvent`s, no Alt chords, exact `Color::Rgb`, glyphs from box/block ranges or ones already drawn; check both builds; the screen fits 100×30.
- docs/saving.md: a new `AppState` field is named in `snapshot`/`restore`; the snapshot's new field is `#[serde(default)]` (the v1 fixtures must keep opening; never regenerate them); a skipped cache is rebuilt in `after_restore`.
- Everything from the seed: the world is `WorldParams { seed: city.seed, ..Default::default() }`; the same on every platform.
- Keys: `W` goes up (from the open street, and from the open ground to the overworld); `↵` goes down on the overworld; walking keys as the map's (`dir_for`); `Esc` the pause menu; `?` help.
- One clock (spec): the outside runs `clock::run(st, Some(1), PATHS_PER_FRAME)` every frame as the map does; a detail step costs `sim::WALK_TICKS` (9); an overworld step costs `8 × WALK_TICKS` times the ground's factor: flats, scrub, fields, salt, spill 1; hills 2; mountains 4; a river tile with no road ×2 more (wading). Peaks, the sea and lakes can't be entered.
- Build into `--target-dir target/smoke`.

## Review Focus

- A run saved on the overworld or the open ground reopens there, not in the city; an old save (no world) opens in the city and builds its world on the first `W`. Tests in Task 1 and Task 5.
- Going down where the tile's middle is water or cliff lands on the nearest walkable cell, never in water; a tile with nothing walkable refuses with a log line. Test in Task 2.
- A home block clamped against the world's edge still maps city ↔ tiles both ways, and entering it from any side lands on a walkable outdoor street cell. Test in Task 1 and Task 2.
- The outside screen at 100×30 and at 120×40 draws without panics and keeps `@` in view. Test in Task 4.
- Walking the open ground across many tiles re-centres the cached region without moving you on screen or losing what you have seen. Test in Task 3.

---

### Task 1: The world beyond the city, saved with the run

**Files:**
- Create: `crates/strata-zero/src/outside/mod.rs`
- Modify: `crates/strata-zero/src/main.rs` (or `lib.rs`: `mod outside;`), `crates/strata-zero/src/app.rs` (field, `new`, `enter_city_at`), `crates/strata-zero/src/runs/snapshot.rs`

**Interfaces:**
- Produces:
  - `pub enum At { City, Overworld(Pos), Ground(Pos) }` (overworld tile; world-wide detail cell), `Serialize, Deserialize, Clone, Copy, PartialEq, Debug`, `Default = City`.
  - `pub struct Outside { pub params: WorldParams, pub at: At, pub seen: BTreeMap<Pos, u64>, #[serde(skip)] pub world: Option<Overworld> }` — `seen`: per overworld tile, a bit per detail cell (`y * 8 + x`).
  - `impl Outside { pub fn new(city: &City) -> anyhow::Result<Outside>; pub fn world(&self) -> &Overworld; pub fn block(&self, city: &City) -> TileRect; pub fn tile_of(&self, city: &City, p: Pos) -> Pos; pub fn street_of(&self, city: &City, tile: Pos) -> Pos; pub fn rebuild(&mut self) }`
  - `pub fn ensure(st: &mut AppState) -> Option<&mut Outside>` — builds it from the city's seed on first need.
  - `AppState.outside: Option<outside::Outside>`; NEW GAME's home city at `SizePreset::Large` (`AppState::new()`'s `params.size`).

- [ ] **Step 1: Write the failing tests** (in `outside/mod.rs`):
  - `the_world_comes_from_the_citys_seed`: `Outside::new(&city)` twice → equal `world().tiles`; `params.seed == city.seed`.
  - `the_home_block_is_the_city_at_its_size`: for a medium city (240×120) the block is 30×15 tiles, centred on `world().home().centre` (±1), inside the world; for a block that would cross the edge (move the home centre to tile (2, 2) in a test copy) it is clamped inside, same size.
  - `city_cells_and_tiles_map_both_ways`: for every corner of the block and its middle, `tile_of(street_of(t)) == t`; `street_of` lands on a walkable outdoor Street cell of the city.
  - `ensure_builds_once`: `ensure(st)` on a state with a city creates it; the second call returns the same (pointer to `world().tiles` unchanged, `at` kept).
  - `the_world_comes_back_from_a_save`: with `at = Overworld((40, 20))` and a `seen` entry, `runs::testutil::round` of the snapshot restores `at`, `seen` and `params`, and after `after_restore` `world()` is present and equal to the original.
  - `a_new_city_forgets_the_world`: `enter_city` sets `outside` to `None`.
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke outside::` — FAIL (no module).
- [ ] **Step 3: Implement.** `block`: `w = ceil(city.w / 8)`, `h = ceil(city.h / 8)`, centred on the home centre, clamped into the world. `street_of(tile)`: the city cell `8 * (tile - block.origin) + (4, 4)`, clamped into the city, then the nearest walkable outdoor Street cell by a ring search (`is_walkable` and no building). `rebuild`: regenerate `world` from `params`. `after_restore` calls `rebuild` on `st.outside`. `SnapshotRef`/`Snapshot` gain `outside` (`#[serde(default)]` on `Snapshot`'s).
- [ ] **Step 4: Run** the tests and `cargo test -p strata-zero --target-dir target/smoke runs::` — PASS (the fixtures still open).
- [ ] **Step 5: Commit** `outside: the world beyond the city, saved with the run (#22)`.

### Task 2: Walking the overworld, going down and coming up

**Files:**
- Create: `crates/strata-zero/src/outside/moves.rs`

**Interfaces:**
- Consumes: Task 1's `Outside`, `ensure`.
- Produces:
  - `pub enum Moved { Stepped, Blocked(&'static str), City(Pos) }` — `City(street cell)` when the step enters the home block.
  - `pub fn step_cost(world: &Overworld, to: Pos) -> Option<u64>` (ticks; `None` closed), per the Global Constraints.
  - `pub fn world_step(st: &mut AppState, d: Dir) -> Moved` — moves `At::Overworld`, runs the clock by the cost.
  - `pub fn down(st: &mut AppState) -> Result<Pos, &'static str>` — `At::Overworld(t)` → `At::Ground(cell)`: a road cell nearest the middle if the tile has a road, else the walkable cell nearest `(8t + 4)`; never water.
  - `pub fn up(st: &mut AppState)` — from `At::Ground(c)` to `At::Overworld(c / 8)`; from `At::City` to the tile of the player's street cell.
  - `pub fn into_city(st: &mut AppState, cell: Pos)` — puts `st.player` on Street at `cell`, `At::City`.
- [ ] **Step 1: Write the failing tests:**
  - `a_step_costs_the_walk_across_a_tile`: on flats a step advances the world by `8 * WALK_TICKS`; on hills by twice that.
  - `the_sea_and_peaks_are_closed`: a step toward a sea tile returns `Blocked` and `at` is unchanged.
  - `stepping_onto_the_block_goes_into_the_city`: from the tile west of the block's middle row, `Dir::E` returns `City(cell)` with a walkable street `cell`; `into_city` leaves `at == City` and the player on Street there.
  - `down_lands_on_the_road_or_dry_ground`: on a road tile, `down` lands on `Road`; on a river tile with no road, on a walkable non-water cell; for every tile of seed 0's world that is land, `down` succeeds (sampled every 7th tile).
  - `up_from_the_ground_is_the_tile_you_are_on`.
- [ ] **Step 2: Run** — FAIL.
- [ ] **Step 3: Implement** with `detail::ground` for the tile (a ring search outward from the middle over `region` of the tile plus its neighbours for the nearest walkable cell).
- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit** `outside: walking the overworld, down onto a tile and back up (#22)`.

### Task 3: The open ground: a region round you, sight and memory

**Files:**
- Create: `crates/strata-zero/src/outside/ground.rs`

**Interfaces:**
- Produces:
  - `pub struct GroundView { pub origin: Pos, pub tiles: Grid<Tile> }` (a `region` of detail cells; not saved).
  - `pub const SPAN: (u32, u32) = (16 * 8, 8 * 8)` (tiles of the region), `pub const KEEP: i32 = 24` (re-centre when you come within this of its edge).
  - `impl GroundView { pub fn round(world: &Overworld, at: Pos) -> GroundView; pub fn keep(&mut self, world: &Overworld, at: Pos) -> bool; pub fn tile(&self, p: Pos) -> Option<&Tile> }` (world-wide `p`).
  - `pub fn ground_step(st: &mut AppState, view: &mut GroundView, d: Dir) -> Moved` — walkable check, the clock by `WALK_TICKS`, `City(cell)` when the cell is in the home block.
  - `pub fn sight(view: &GroundView, at: Pos, minute: u16) -> BTreeSet<Pos>` — `fov::compute` with `RADIUS_STREET` (at night `RADIUS_UNDER + 2`), `blocks_sight`.
  - `pub fn remember(o: &mut Outside, seen: &BTreeSet<Pos>)`, `pub fn remembered(o: &Outside, p: Pos) -> bool`.
- [ ] **Step 1: Write the failing tests:**
  - `walking_east_crosses_into_the_next_tile`: from the last column of a road tile's lane, `Dir::E` moves to the next tile's first column.
  - `water_stops_you`: a step into `Water` is `Blocked`.
  - `the_region_recentres_without_moving_you`: walk 200 cells east along a straight highway; `view.tile(at)` always equals `detail::ground` of that tile at that cell, and `at` advanced 200.
  - `what_you_saw_stays_seen`: after `remember`, a cell seen stays `remembered` after walking away and after a save round trip.
  - `the_city_block_takes_you_in`: a ground step into a cell of the home block returns `City`.
- [ ] **Step 2: Run** — FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit** `outside: the open ground round you, seen as you walk (#22)`.

### Task 4: The outside screen

**Files:**
- Create: `crates/strata-zero/src/screens/outside.rs`
- Modify: `crates/strata-zero/src/screens/mod.rs` (`pub mod outside;`), `crates/strata-zero/src/screens/explore.rs` (make `layout` `pub(crate)`)

**Interfaces:**
- Consumes: Tasks 1–3; `ui::hud` (`status_line`, `bar_row`, `location_line`, `carry_row`, `rule_row`, `footer_rows`), `explore::layout`, `render::overworld::atlas_cell`, `render::tileset::cell`, `render::lighting::lit`.
- Produces: `pub struct OutsideScreen` with `pub fn new(st: &mut AppState) -> OutsideScreen` and `impl Screen`:
  - Map area: on the overworld, atlas cells one a tile centred on you with `@`; on the ground, the cached region through `tileset::cell` lit by `ambient(Street, minute)`, unseen cells blank, remembered ones dim (`apply_vis`), `@` on you.
  - Bands: the map's status line and bars; the right band's location line `overworld` / `open ground` with the clock; carry row.
  - Panel: HERE (`here_lines`: the region nearest you, the route if one runs on the tile, the ground and the hour, e.g. `Dust Flats · Route 9` / `open ground · night`), MAP (on the overworld the whole world scaled into the pane, the view framed; on the ground the overworld round you in the atlas look with `@`), NEAR (the five nearest cities and POIs: name, distance in tiles, bearing `N NE E…`), LOG (the sidebar's log pane).
  - Footer: `on foot · overworld` + `↵ down · W up …` hints; the log's newest line.
  - Keys: walking keys → `world_step`/`ground_step`; `↵` → `down`; `W` on the ground → `up`; `City(cell)` → `into_city` and `ScreenCmd::Switch` to `screens::resume_screen(st)`; `Esc` pushes the pause menu; `?` help.
  - Each update runs the clock a tick and `runs::when::frame`.
- [ ] **Step 1: Write the failing tests:**
  - `the_overworld_draws_in_the_play_screen`: 120×40 → text has `overworld`, `@`, the home city's name in NEAR, the region name in HERE.
  - `it_fits_the_smallest_terminal`: 100×30 draws, `@` present.
  - `enter_goes_down_and_w_comes_up`: `↵` → `at` is `Ground`, the text says `open ground`; `W` → `Overworld` again.
  - `walking_into_the_city_switches_to_the_map`: steps toward the block until `Switch`; `at == City`.
  - `esc_opens_the_pause_menu`.
- [ ] **Step 2: Run** — FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit** `screens: the overworld and the open ground in the play screen (#22)`.

### Task 5: Into and out of the city, and back after a reload

**Files:**
- Modify: `crates/strata-zero/src/screens/explore.rs` (the `W` key; the street's edge), `crates/strata-zero/src/screens/mod.rs` (`resume_screen`), `crates/strata-zero/src/screens/help.rs`, the map's footer hints.

**Interfaces:**
- Consumes: Tasks 1–4.
- [ ] **Step 1: Write the failing tests:**
  - `w_on_the_open_street_goes_up_to_the_world`: on Street, outdoors, walking: `W` → `Switch`; `at == Overworld(tile_of(player))`.
  - `w_indoors_or_on_the_net_says_why_not`: a floor → `Stay` and a log line; jacked in → `Stay`.
  - `the_streets_edge_leads_out`: a walking step off the street map's edge (`Dir::W` at x = 0) → `Switch` to the outside on `Ground` at the world cell beside the block; at a closed side (water) it is just blocked.
  - `a_run_saved_outside_reopens_outside`: `resume_screen` on a state with `at = Overworld` returns a screen whose draw says `overworld`; with `At::City`, the map.
  - `the_help_lists_w`.
- [ ] **Step 2: Run** — FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** — PASS, then `cargo test -p strata-zero --target-dir target/smoke` whole.
- [ ] **Step 5: Commit** `map: W goes up to the world; the street's edge leads out (#22)` with `Closes #22` once Task 6 passes (on Task 6's commit).

### Task 6: Shots and checks

**Files:**
- Modify: `crates/strata-zero/src/screens/shots.rs`

- [ ] **Step 1:** `#[ignore] fn outside_shots()`: a large seed-8 city with its world; shots `outside-overworld` (on the overworld beside the home block), `outside-ground-road` (down on a highway tile), `outside-ground-river` (down beside a bridge), at 120×40 and 100×30.
- [ ] **Step 2:** `SHOT_DIR=... cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored outside_shots` and look at the PNGs.
- [ ] **Step 3:** CLAUDE.md's five checks, all clean.
- [ ] **Step 4: Commit** `shots: the overworld and the open ground` with `Closes #22`.
