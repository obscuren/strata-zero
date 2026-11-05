# The Streamed World Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One street and one Net across the world: the live city's map carries on past its edge into the streamed open ground and the other cities, a city builds in the background before you reach it and is walked into with no screen change, and the Net is walked the same way with no reach limit; the Net overworld is walked.

**Architecture:** The live city stays the frame of every system (positions in its local cells, now allowed off its map). A stream on `Outside` (not saved) holds the open ground's tiles, two layers, and the cities being built. The street and Net stratum maps get a `Beyond` (the stream plus the other held cities, each at its world origin) that walking, sight and light read through; the city screen draws by area (stream, other cities clipped, the live city clipped, overlays). Crossing into another city's map hands the run over to it in place. The outside screen keeps only the overworld and the Net overworld.

**Tech Stack:** Rust workspace (`citygen`, `game`, `sim`, `strata-zero`), ratatui/crossterm, std threads for background builds.

**Spec:** `docs/superpowers/specs/2026-11-05-streamed-world-design.md`

## Global Constraints

- Build only with `--target-dir target/smoke`; never the default `target/` (Jeff plays `target/release/strata` from this worktree).
- The five checks, all with `--target-dir target/smoke`: `cargo fmt --all --check`; clippy `--workspace --all-targets -- -D warnings`, plain and with `--features strata-zero/window`; `cargo test --workspace`, plain and with `--features strata-zero/window`.
- Nothing generated changes: worlds and cities come out cell for cell as before; no new floating-point maths in `citygen` generation.
- `docs/saving.md`: a new `AppState` field is named in `runs::snapshot::snapshot` and `restore`; new fields in saved types `#[serde(default)]`; the fixtures in `runs/fixtures/` keep opening and are never regenerated.
- `REACH` = 16 cells (two overworld tiles) past the view. A Net overworld step costs `TILE as u64 * sim::WALK_TICKS`.
- In-world lines, exactly: `into {name}` on a handover (as `outside::enter` says today); `too far from your body to touch it` at a node or gate of a city that isn't the live one. Every existing refusal line keeps its words.
- Draw through ratatui only; exact `Color::Rgb`; no new symbols outside box/block characters unless a carried font has them.
- Commits end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; the commit that finishes issue #27 ends with `Closes #27`. Committed docs record decisions, not chat.

## Review Focus

1. The player's position off the live city's map reaching code that reads the city at that position (grid indexing, `city.building(id)`, pathing): a panic or a wrong answer. Task 5's long-walk test pins it.
2. A building or district id of one city read against another city (themes, wall styles, the Net look's constructs): a wrong colour or a panic. Task 2's routing test and Task 3's area drawing pin it.
3. A seam you can see at a city's edge: light, glyphs or walkability changing across the edge, day and night. Task 1's seam tests and Task 9's shots pin it.
4. Runs saved on the ground, jacked in on the ground, in another city, or from before this change (`At::Ground`, the fixtures) reopening where they were. Tasks 3 and 4 pin it.
5. Background builds: a build that finishes after you've walked away, the same city requested twice, a build that fails, and tests and shots needing builds finished. Task 6 pins it.

---

### Task 1: The stream, its two layers and the ground's light

**Files:**
- Create: `crates/strata-zero/src/outside/stream.rs`
- Modify: `crates/strata-zero/src/outside/mod.rs` (`Outside.stream`, `Outside::origin`), `crates/strata-zero/src/outside/ground.rs` (`GroundView` keeps a lamp layer for the street layer), `crates/strata-zero/src/render/tileset.rs` (a sign with no district in the roadside orange, moved from `screens/outside.rs::open_cell`)

**Interfaces:**
- Produces:
  - `Outside::origin(&self, city: &City) -> Pos`: `TILE * block.x`, `TILE * block.y` of `self.block(city)`.
  - `pub struct Stream` (in `outside::stream`) with the street layer and the Net layer (`GroundView`s) and, from Task 6, the builds. `Outside` gains `#[serde(skip)] pub stream: Stream`; `Outside` stays `Clone` and `Debug` (a clone's stream starts empty; `Debug` prints the number of tiles held).
  - `Stream::cover(&mut self, world: &Overworld, area: Rect)`: both layers cover `area` (world cells) plus the `AHEAD` ring, forgetting far tiles as `GroundView::cover` does.
  - `pub struct GroundMap<'a> { pub stream: &'a Stream, pub layer: StratumKind, pub minute: u16 }` implementing `TileMap` in world cells: `tile` from the layer; `light` = `citygen::lighting::ambient(Street, minute)` + the lamp layer while lamps are on (street layer), the Net's light for the Net layer as a city's Net stratum gives it; `emitted`/`emits` from the lamp layer; `daylight`, `lamps_on`, `lit_sight`, `sky` as a `StratumMap` of the same stratum gives them; `theme`, `kind`, `connector` `None`.
  - The lamp layer: `citygen::lighting::compute` with `citygen::lighting::tuning::lamp_emission` over each batch the view builds, with a margin of one tile on every side, cropped, so a tile's light is the same whatever batch built it.
  - `Outside::owner(&self, tile: Pos) -> Option<Settlement>`: who owns an overworld tile, from the world alone, computed once per world and kept (`#[serde(skip)]`, rebuilt with the world): no map covers it → `None`; one map → that settlement; several → the one whose own tiles (`Overworld::city_tiles`, `town_tiles`) lie nearest (squared tile distance), ties to the city before a town, then the lower city index, then the lower town index. "Map" is the home city's `block(city)` and every other settlement's block from `others()`; a home city from before the world owns its whole block.

- [ ] **Step 1: Write the failing tests** (in `outside/stream.rs`):
  - `every_tile_has_one_owner`: on seed 3's world, every tile covered by two blocks is owned by the settlement with the nearest own tile; every own tile of a settlement is owned by it; a tile no block covers has no owner.
  - `a_town_owns_its_own_tiles_inside_its_citys_block`: a town whose tiles lie inside its city's block owns them.
  - `the_ground_past_a_city_is_the_citys_own_ground`: seed 3's sited home city (the `sited()` helper of `outside/mod.rs` tests); for every cell `p` of the city's map within 2 cells of its edge whose street tile has no district, the stream's street tile at `origin + p` has the same `terrain` and `feature`; the same for the Net layer against the city's Net stratum where the Net tile has no district.
  - `a_ground_lamp_lights_as_the_citys_lamps_do`: at a minute when lamps are on, for every cell of that city's map at least 10 cells from its edge with no district cell within 8 cells (out of reach of the city's own lamps), `GroundMap{layer: Street}.emitted(origin + p)` equals the city's street `emitted(p)` within 1.
  - If the seam test finds cells where the city's open ground differs from the world's, don't change generation: report where and what differs (counts, terrains, examples).
  - `a_tile_lights_the_same_whatever_built_it`: cover a 3×3-tile area, record the lamp light of its middle tile; a fresh stream covering a 7×7 area round it gives the same values.
  - `the_stream_forgets_far_tiles`: cover areas walking 400 tiles away; the tiles held stay under the cap.
- [ ] **Step 2:** Run `cargo test -p strata-zero --target-dir target/smoke outside::stream` — FAIL (no module).
- [ ] **Step 3:** Implement the interfaces above. The roadside-orange rule moves into the tile renderer so the city's own open ground and the stream draw signs alike; `screens/outside.rs::open_cell` calls it.
- [ ] **Step 4:** Run the same command — PASS; run `cargo test -p strata-zero --target-dir target/smoke outside` — PASS.
- [ ] **Step 5:** Commit `outside: the stream and the ground's light (#27)`.

### Task 2: The map past the edge

**Files:**
- Modify: `crates/strata-zero/src/render/map.rs` (`StratumMap.beyond`, `MapRef::beyond`, `TileMap::reaches`), `crates/strata-zero/src/outside/stream.rs` (`Beyond`), `crates/strata-zero/src/walker/mod.rs` (sight past the edge), `crates/strata-zero/src/walker/fov.rs` if bounds are checked there

**Interfaces:**
- Consumes: `Stream`, `GroundMap`, `Outside::origin` (Task 1).
- Produces:
  - `pub struct Beyond<'a>`: the live city's origin, the other held cities with their origins (the home city in `st.away` when you're elsewhere, every city in `Outside.built`), and `&'a Stream`. `Beyond::of(st: &'a AppState) -> Option<Beyond<'a>>`: `None` when there is no world (no `outside`, the lot).
  - `StratumMap.beyond: Option<&'a Beyond<'a>>` (`None` everywhere it's built today); `MapRef::beyond(self, b: &'a Beyond<'a>) -> MapRef<'a>` sets it on a `Stratum` map of the street or the Net, leaves floors, constructs and the undercity as they are.
  - Every per-cell method of `StratumMap` with a beyond answers from the area that owns `origin + p` (`Outside::owner` of its tile): the live city's own map when it owns it; another held city's stratum at the cell less that city's origin when that city owns it and is held; else the stream through `GroundMap` (open ground, or an owner not built yet). Ids on a tile of another area are only ever read against that area's city. `Beyond` carries what it needs to answer ownership (a reference to `Outside`, or the owner table).
  - `Beyond::live_owns(&self, p: Pos) -> bool` (local cell): whether the live city owns it.
  - `TileMap::reaches(&self, p: Pos) -> bool`, default `self.size().contains(p)`; a map with a beyond reaches every cell whose tile is built.
  - On the Net, the map's `walkable(p)` is also true on `Terrain::Void`, on and off the live map.
  - `Player::update_fov` casts over `reaches`; cells off the map in sight are kept in a not-saved list, `Player::beyond_sight(&self) -> &[Pos]`, cleared each update. `outside::stream::remember_sight(st: &mut AppState)` records them (as world cells) into `Outside.seen`.
- [ ] **Step 1: Write the failing tests:**
  - `the_street_walks_off_the_city_and_back` (walker): with the beyond, a player on an edge street cell steps off the map onto the ground and back with `try_move`; without it the same step is refused.
  - `sight_crosses_the_edge`: by day from a street cell beside the edge, `beyond_sight()` holds ground cells; after `remember_sight` they are remembered in `Outside.seen`.
  - `the_nets_void_is_walked_and_walls_are_not`: on the Net map with the beyond, a `Void` cell of the city's Net past its outline and a `Void` cell of the stream are walkable; a `NetWall` isn't.
  - `an_unbuilt_cell_cannot_be_walked`: a cell whose stream tile isn't covered is neither walkable nor reached.
  - `another_citys_cells_answer_from_that_city`: with another city held in `Outside.built` (build it the way `outside::enter` does), for a cell it owns with a building `b`, `map.kind(p)` is that city's `building(b).kind` and `map.theme(p)` its district's theme.
  - `a_cell_of_the_live_map_another_settlement_owns_answers_from_it`: a cell inside the live city's map owned by a town (held) answers from the town's map; with the town not held, from the stream.
- [ ] **Step 2:** Run `cargo test -p strata-zero --target-dir target/smoke beyond` (name the tests so the filter catches them) — FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the filter — PASS; `cargo test -p strata-zero --target-dir target/smoke` — PASS.
- [ ] **Step 5:** Commit `render: the street and the Net past the city's edge (#27)`.

### Task 3: The city screen walks and draws past the edge

**Files:**
- Modify: `crates/strata-zero/src/screens/explore.rs` (draw by area, camera, steps, `off_the_edge`), `crates/strata-zero/src/render/viewport.rs` (a tile pass any `TileMap` can draw, visibility off the live map), `crates/strata-zero/src/render/netlook/grid.rs` (draw an area's Net with its own city, or none for the open Net; draw only the cells its map holds), `crates/strata-zero/src/outside/mod.rs` (`away`, `your_tile`, `tile_of`, `can_leave`, `reopened`, `At::Ground` converted), `crates/strata-zero/src/runs/describe.rs`, `crates/strata-zero/src/runs/snapshot.rs` if restore needs it

**Interfaces:**
- Consumes: `Beyond`, `MapRef::beyond`, `GroundMap`, `Stream::cover`, `remember_sight` (Tasks 1–2).
- Produces:
  - The city screen tends the stream every update: `Stream::cover` over the view's rectangle in world cells (and over your sight), before any step is checked.
  - Drawing on the street and the Net with a world: the stream's layer under the whole map area (`GroundMap` through the tile pass; the open Net through the Net look with no city), then each other held city on the cells it owns (its own `StratumMap`; its own Net look), then the live city and its overlays as today, the live map's tile pass and Net look drawing only the cells it owns. Visibility off the live city's cells: in sight now, else remembered in `Outside.seen`, else unseen; the Net is all in sight.
  - The camera isn't clamped on the street or the Net when there is a world; floors, constructs and the lot clamp as today.
  - The live city's edge is the edge of the cells it owns. A step across it is a step on the composite map: on foot it's refused by `can_leave`'s rules other than "jack out first" (the lot, "not in a fight", a call, a scene; lines unchanged); the Net self steps across freely. `leave_by` and the switch to the outside screen go. `can_leave`'s "only from the open street" holds off the live city's cells.
  - `outside::away(st)`: on the overworld, or standing (the body, when jacked in) off the live city's cells. `tile_of` and `your_tile` no longer clamp to the block.
  - `At::Ground(c)` is no longer written. Restoring a run with it: `At::City`, the player on the street at `c - origin` (body none); a cell that can't be stood on any more lands back in the city with today's line.
  - `describe`: a run whose player stands off the live map says `open ground`.
- [ ] **Step 1: Write the failing tests** (explore or outside tests; use seed 3's sited home city):
  - `walking_off_the_city_stays_on_the_map_screen`: arrows from an edge street cell off the map return `ScreenCmd::Stay`; the player is off the map; the screen is still the map screen.
  - `the_ground_draws_past_the_edge`: with the player off the map by day, the drawn buffer's cells over the ground show the stream's glyphs (not the blank background).
  - `the_camera_follows_past_the_edge`: ten steps off the map, the player is drawn in the middle third of the view.
  - `a_call_still_holds_you_at_the_edge`: with a call on the line, the step off the map is refused with `not with a call on the line`; in a fight, with `not in a fight`.
  - `the_net_self_walks_off_the_citys_net`: jacked in at a city port, walking to the Net's edge and on leaves the Net self off the map, the body where it was.
  - `the_story_holds_still_off_the_map`: off the map a ringing call's `rang` follows the clock (as `clock.rs` does away).
  - `an_old_save_on_the_ground_opens_on_the_street`: a state with `At::Ground(c)` round-tripped through the snapshot opens with `At::City` and the player at `c - origin`.
  - `the_fixtures_still_open` stays green (existing).
- [ ] **Step 2:** Run those tests — FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run them — PASS; full `cargo test -p strata-zero --target-dir target/smoke` — PASS (update tests whose expectation was the screen switch, keeping what they check).
- [ ] **Step 5:** Commit `explore: the street and the Net carry on past the city's edge (#27)`.

### Task 4: The ground on the city screen; the outside screen keeps the overworld

**Files:**
- Modify: `crates/strata-zero/src/screens/outside.rs` (ground and open-Net modes go: `draw_ground`, `ground_step`'s callers, `jack_in`, `net_step`, `draw_net`, `in_reach`), `crates/strata-zero/src/outside/moves.rs` (`down` onto the ground lands on the city screen), `crates/strata-zero/src/outside/ground.rs`, `crates/strata-zero/src/screens/explore.rs` (space at a ground port, W off the map), `crates/strata-zero/src/ui/sidebar.rs` (HERE/NEAR off the map), create `crates/strata-zero/src/outside/here.rs` (the place lines moved from `screens/outside.rs`), `crates/strata-zero/src/render/minimap.rs` (the close-up outdoors through the beyond), `crates/strata-zero/src/screens/help.rs`, `crates/strata-zero/src/screens/shots.rs` (the ground shots on the map screen)

**Interfaces:**
- Consumes: Tasks 1–3.
- Produces:
  - ↵ on the overworld onto open ground: `At::City`, the player on the street at the landing cell less the live origin, the city screen (`resume_screen`). W anywhere on the street outdoors (on or off the map) goes up to the tile you stand on.
  - Space off the live map, not jacked in, with a `Feature::Terminal` or a vending machine on your cell or beside it (any area's street, through the beyond): jack in as the city does (`body` set, the Net self on the nearest Net cell with signal within 8, not `Void`), with `no port in reach`, `deck cooling · {left} min` and `no signal here` as today. J jacks out to the body.
  - `outside::here::here_lines(st) -> Vec<Line>`, `near_lines(st, n) -> Vec<Line>`, `net_here_lines(st) -> Vec<Line>` (the outside screen's, reading your tile from the player's world cell); HERE in the sidebar uses them when you stand off the live map, and the Net ones jacked in off it.
  - The minimap's close-up outdoors draws through the beyond.
  - Bots out follow you on the ground as on the street (their way found on the map with the beyond); they're drawn wherever they are.
  - Dropping an item off the live city's cells is refused: `nowhere to leave it out here`.
  - `KEYS · OUTSIDE` lists only the overworld's keys; the map screen's help says W goes up from the street or the ground.
- [ ] **Step 1: Write the failing tests:**
  - `enter_on_the_overworld_lands_on_the_city_screen` (outside): ↵ on an open tile returns a switch to the map screen; the player stands off the live map on a walkable cell of that tile or its ring.
  - `w_off_the_map_goes_up`: from the ground, W puts you on the overworld tile of your world cell.
  - `space_by_a_ground_terminal_jacks_in`: the `by_a_pump` set-up, moved to the map screen; space jacks in, the body stays.
  - `jacking_out_on_the_ground_wakes_on_the_body`.
  - `a_run_saved_jacked_in_on_the_ground_reopens_jacked_in`.
  - `here_off_the_map_names_the_place_and_the_road`: the drawn sidebar holds `open ground ·` and the tile's region or road.
  - `the_minimap_shows_the_ground_past_the_edge`.
  - `a_bot_follows_you_onto_the_ground`: with a bot out, ten steps onto the ground, the bot is within its leash of you.
  - `nothing_is_dropped_on_the_open_ground`: dropping off the live city's cells says `nowhere to leave it out here` and keeps the item.
- [ ] **Step 2:** Run them — FAIL.
- [ ] **Step 3:** Implement; retire the outside screen's tests of the removed modes, keeping each behaviour they checked that still exists, rewritten for the map screen.
- [ ] **Step 4:** Run them — PASS; full `cargo test -p strata-zero --target-dir target/smoke` and with `--features strata-zero/window` — PASS.
- [ ] **Step 5:** Commit `outside: the open ground is walked on the city screen (#27)`.

### Task 5: Off the map everywhere

**Files:**
- Test: `crates/strata-zero/src/screens/explore.rs` (or a new `screens/far.rs` test module)
- Modify: whatever the test breaks.

**Interfaces:**
- Consumes: Tasks 1–4.
- Produces: no new interface; every system that reads the city at the player's (or the body's, or the Net self's) position copes with a position off the live map.
- [ ] **Step 1: Write the failing test** `a_long_walk_out_and_back_never_breaks`: from seed 3's home city, walk 120 steps out across the ground in a seeded wander that keeps to walkable cells, then back; at every step draw the map screen at 80×24, 120×40 and 200×60; at steps 0, 40, 80, 120 open and close the deck (Tab, each tab), MAP at every zoom, the sheet, the pause menu, look mode, the inspector (dev unlocked), and press every key the map screen's help lists; then jack in at a ground port and do the same on the Net, 120 steps out across the void. Nothing panics; the player ends where the walk says.
- [ ] **Step 2:** Run it — note every failure.
- [ ] **Step 3:** Fix each at its cause (read the city only through `Option`s or the beyond; off the map a system finds nothing of the city).
- [ ] **Step 4:** Run it — PASS; full suite plain and with the window feature — PASS.
- [ ] **Step 5:** Commit `explore: off the city's map, nothing breaks (#27)`.

### Task 6: Cities stream in

**Files:**
- Modify: `crates/strata-zero/src/outside/stream.rs` (builds), `crates/strata-zero/src/outside/mod.rs` (`enter` takes a finished build or builds on the spot; the build's params made in one place), `crates/strata-zero/src/screens/explore.rs` (tend the builds each update)

**Interfaces:**
- Consumes: `Stream`, `Beyond` (other held cities drawn and walked from Task 3).
- Produces:
  - `pub fn build_params(o: &Outside, s: Settlement, base: &GenParams) -> Option<GenParams>` (what `enter` computes today).
  - `Stream::tend(&mut self, …)` (called from `outside::stream::tend(st: &mut AppState, view: Rect)`): for every settlement other than the live one and the held home city whose map rectangle (its block in cells) comes within `REACH` cells of `view`, not built and not building, start a build on a background thread (`Generator::run` on the params made on the main thread), the same thread then seeding its people as `Stay::fresh` does for the clock tick and minute of day when the build was asked for (handed a clone of `st.people`); collect each finished city into `Outside.built` (where `Beyond` already finds other cities) and its seeded people into the stream, keyed by settlement; a failed build is remembered and not retried this session.
  - `Stream::finish(&mut self, …)` (and `outside::stream::finish(st)`): waits for every build in flight and collects them (tests, shots).
  - `enter` uses the held city, else a finished build (its seeded people run on to now with `sim::World::advance`, not seeded again), else builds and seeds on the spot.
  - If `sim::People` or `sim::World` can't cross a thread, report it (DONE_WITH_CONCERNS) and seed on the frame at the handover as today.
- [ ] **Step 1: Write the failing tests:**
  - `a_city_near_the_view_builds_in_the_background`: on the ground within `REACH` of another city's block, `tend` then `finish`: `Outside.built` holds it.
  - `far_cities_are_not_built`: a view more than `REACH` from every other block builds nothing.
  - `a_streamed_city_is_the_city_built_on_the_spot`: its street and Net strata equal a city built synchronously from `build_params`.
  - `a_streamed_citys_people_are_ready_when_you_walk_in`: entering a city whose build finished in the background doesn't seed its people again (its world's tick is run on, the people are those the thread seeded).
  - `the_same_city_is_never_built_twice`: tending repeatedly while a build is in flight starts one build.
  - `the_neighbour_draws_before_you_walk_in`: standing beside another city's block after `finish`, the drawn map shows that city's buildings past its edge.
- [ ] **Step 2:** Run them — FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run them — PASS; full suite — PASS.
- [ ] **Step 5:** Commit `outside: cities build in the background as you near them`.

### Task 7: The handover

**Files:**
- Modify: `crates/strata-zero/src/outside/mod.rs` (`enter` keeps your cell), `crates/strata-zero/src/outside/stream.rs` (`hand_over`), `crates/strata-zero/src/screens/explore.rs` (after a step; the camera, cursor and any held positions move with the frame), `crates/strata-zero/src/walker/mod.rs` (memory seeded from `Outside.seen`), `crates/strata-zero/src/interact.rs` / the Net's gate and node handling (the line at a non-live city)

**Interfaces:**
- Consumes: Tasks 1–6.
- Produces:
  - `outside::stream::hand_over(st: &mut AppState) -> Option<Pos>`: when the body stands on the street on a cell another settlement owns (not jacked in), it becomes the live city through `enter`, the player on the same world cell in its frame, its street memory remembering what `Outside.seen` holds of its map, sight recomputed; bots out come along (taken from the city you leave, set beside you in the new frame); returns the shift (old origin − new origin) for the screen.
  - The map screen calls it after every step on foot and shifts its camera, cursor, aim and looking positions by the shift, with no transition.
  - Jacked in, no handover: the Net self walks other cities' Nets. Bumping a gate or acting on a node of a city other than the live one: `too far from your body to touch it`, no move.
- [ ] **Step 1: Write the failing tests:**
  - `walking_into_another_city_hands_over_on_the_same_cell`: walk from the ground into a neighbour's map; `Outside.here` names it; the player's world cell is the cell walked onto; the log holds `into {name}`.
  - `the_camera_does_not_move_on_a_handover`: the screen origin of the player's cell is the same before and after.
  - `what_you_saw_from_outside_is_remembered_inside`: cells of the neighbour seen from the ground are remembered on its street after the handover.
  - `walking_home_hands_back`: walking back into the home city restores its story and its player memory.
  - `the_net_self_walks_another_citys_net_without_a_handover`.
  - `another_citys_gate_is_too_far_to_touch`.
  - `a_town_hands_over_too`: walking from a city's cells onto its town's own tiles inside the city's block hands over at the ownership line, and the cell you stand on is the town's.
  - `your_bots_come_into_the_next_city`.
- [ ] **Step 2:** Run them — FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run them — PASS; full suite plain and with the window feature — PASS.
- [ ] **Step 5:** Commit `outside: walking into a city hands the run over in place`.

### Task 8: The Net overworld is walked

**Files:**
- Modify: `crates/strata-zero/src/screens/outside.rs` (`net_up_key`, the panel's Net lines, the hints), `crates/strata-zero/src/outside/moves.rs` (`net_world_step`, `net_down`), `crates/strata-zero/src/screens/help.rs`, `docs/superpowers/specs/2026-11-04-overworld-screen-design.md` and `2026-11-04-world-tiles-design.md` (the open question points to the new spec), `docs/saving.md` (the stream, `At::Ground`, positions off the map)

**Interfaces:**
- Consumes: Tasks 1–7.
- Produces:
  - `moves::net_world_step(st, d) -> Moved`: onto the next tile inside the world, any tile, costing `TILE as u64 * sim::WALK_TICKS`; `the world ends here` past the edge.
  - `moves::net_down(st, tile) -> Result<(), &'static str>`: the Net self onto that tile's Net: the `NetLane` cell nearest the tile's middle, else the nearest walkable Net cell (any area, through the beyond, the stream covered first), in the live city's frame; the map screen shows it, the camera on the Net self.
  - The Net overworld's panel and hints no longer say look-only: `arrows walk · enter goes down · J jack out · esc menu · ? keys`.
- [ ] **Step 1: Write the failing tests** (replace `the_net_overworld_is_look_only`):
  - `the_net_overworld_walks_tile_by_tile`.
  - `a_net_overworld_step_costs_a_tiles_walk`.
  - `enter_on_the_net_overworld_lands_on_that_tiles_net`: the Net self's world cell lies in the tile or its ring; the body hasn't moved.
  - `landing_prefers_the_trunk`: on a tile with a trunk, the Net self lands on a `NetLane`.
  - `the_net_overworlds_help_says_it_walks`.
- [ ] **Step 2:** Run them — FAIL.
- [ ] **Step 3:** Implement; update the docs named above.
- [ ] **Step 4:** Run them — PASS; full suite plain and with the window feature — PASS.
- [ ] **Step 5:** Commit `outside: the Net overworld is walked (#27)` ending `Closes #27`.

### Task 9: Shots and the checks

**Files:**
- Modify: `crates/strata-zero/src/screens/shots.rs`

- [ ] **Step 1:** Add `streamed_world_shots` (ignored, written by `SHOT_DIR`): the map screen at a city's edge walking out by day and by night, standing on the ground with the city behind, beside a neighbour city before walking in, jacked in on the Net past the city's edge and in the void, the Net overworld walked; replace the old ground and open-Net shots that the outside screen drew.
- [ ] **Step 2:** Run `SHOT_DIR=<scratch> cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored streamed_world_shots` and look at every PNG: no seam at the edge, the light continuous, the neighbour's buildings in place.
- [ ] **Step 3:** The five checks; `cargo check -p strata-zero --features window --target x86_64-pc-windows-msvc --target-dir target/smoke` and `x86_64-unknown-linux-gnu` if the targets are installed.
- [ ] **Step 4:** Commit `shots: the streamed world`.
