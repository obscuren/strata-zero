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

### Task 10: The trunks run on into a city's lanes

Added after Task 9's shots: on a city's Net the world's trunks stop where the city's own grid starts (Route 5 ends about 10 cells into Sable Bay's map, about 35 cells of dark grid short of its first lane), against "the Net is joined along the roads by its trunks". Ruling: the city's Net changes (pre-1.0 world tuning is allowed); the streets, the open Net past the outline and every construct stay as they are.

**Files:**
- Modify: `crates/citygen/src/generate/cyberspace.rs` (the Net stage on a site), `crates/strata-zero/src/outside/here.rs`, `crates/strata-zero/src/render/minimap.rs`

**Interfaces:**
- Produces: on a sited city's Net, every trunk (a `NetLane` run copied from the world's open Net) that reaches into the map is joined by `NetLane` cells to the city's own lanes: from the trunk's last cell to the nearest city lane, along the street's road under it where there is one, never through a construct (`NetWall`, `NetFloor`, `NetGate`) or a node; integer pathing only; deterministic.
- On another city's Net the minimap's caption names that city as HERE does; on a cell a settlement owns, HERE names the settlement, not the trunk (trunk names only off every settlement's cells).

- [ ] **Step 1: Write the failing tests:** `every_trunk_runs_on_into_the_citys_lanes` (citygen; seed 3's world, every settlement: each trunk entering the map is lane-connected to the city's lanes); `joining_the_trunks_touches_no_construct` (every construct cell equals the city built before the change's rule, by comparing with the lanes rule off); the stream's seam test stays green; `here_names_the_city_on_its_lanes` and `the_minimap_names_another_citys_net` (strata-zero).
- [ ] **Step 2:** Run them — FAIL.
- [ ] **Step 3:** Implement; re-pin any fingerprint the change moves, saying which.
- [ ] **Step 4:** Run them — PASS; the five checks; rerun `streamed_world_shots` and look at `streamed_net_edge_wide` and `streamed_net_next_city`.
- [ ] **Step 5:** Commit `citygen: the Net's trunks run on into a city's lanes`.

### Task 11: The Net hands over with your Net self

From Jeff's first play (spec, "After the first play"): another city's Net reads as dead and the cord stays behind on the Net you came from.

**Files:** `crates/strata-zero/src/outside/stream.rs` (`hand_over` for the Net self), `crates/strata-zero/src/outside/mod.rs` (`enter` keeps the body where it is, in the new frame), `crates/strata-zero/src/screens/explore.rs`, `crates/strata-zero/src/net.rs` (the cord; the far-city refusals go), `docs/saving.md`

**Interfaces:**
- `hand_over` also runs jacked in: when the Net self stands on a Net cell another settlement owns, that settlement becomes the live city through `enter` (refused by `can_cross`'s rules: a fight, a call, a scene), the Net self on the same world cell, the body on its own world cell in the new frame (it stands in the city you left, which is held, not live); the screen shifts as on foot.
- Jacking out with the body on another settlement's cells hands that settlement back (the on-foot handover does it once you're on foot).
- `too far from your body to touch it` goes wherever the Net self stands on the live city's cells; it stays only for acting at range on a node or gate another city owns.
- The cord runs from the Net self back to the body across areas and handovers, drawn over every area it crosses (no part left behind on the Net you came from).
- A run saved jacked in after a Net handover reopens there (the body in the city it stands in, rebuilt if it isn't home).

- [ ] Tests: `walking_the_net_into_another_city_hands_it_over`; `its_ice_and_nodes_answer_after_the_handover`; `the_body_stays_in_the_city_you_left`; `jacking_out_hands_the_bodys_city_back`; `a_fight_holds_your_net_self_at_the_line`; `the_cord_follows_you_into_the_next_citys_net` (drawn cells along the whole way); `a_run_saved_after_a_net_handover_reopens_there`.
- [ ] Implement; five checks; commit `outside: the Net hands the run over with your Net self`.

### Task 12: Neighbours alive in view

**Files:** `crates/strata-zero/src/outside/stream.rs`, `crates/strata-zero/src/screens/explore.rs` (draw other areas' people and packets), `crates/strata-zero/src/render/people.rs`, `crates/strata-zero/src/render/netlook/packets.rs`

**Interfaces:**
- Each held city or town (kept `Stay`, streamed build with seeded people, the home city set aside) whose map meets the view runs its people's day on the clock (`sim::World::advance`, no story, no cameras, no foes) and draws them on the cells it owns, through the same people overlay as the live city, in its own frame. Cities out of view hold still.
- Packets run on every area's lanes in view (each held city's own traffic in its frame) and along the open Net's trunks in view, with the same look as the live city's.
- Cost: the release draw-cost test (`streamed_draw_cost`) gains a case beside a neighbour with its people and packets; a frame stays under ~4 ms at 200×60.
- [ ] Tests: `a_neighbours_people_walk_while_in_view`; `a_neighbour_out_of_view_holds_still`; `the_open_nets_trunks_carry_packets`; `a_neighbours_lanes_carry_packets`; handing over keeps its people where they were drawn.
- [ ] Implement; five checks; shots; commit `outside: the cities next door are alive`.

### Task 13: One light

**Files:** found by the test.

- [ ] Test `a_city_looks_the_same_from_outside_and_inside`: for every settlement of seed 3's world, at noon, dusk and midnight, draw a patch of its cells from the ground beside it (held), then hand over and draw the same world cells at the same minute (live, with the same sight): every cell's glyph, foreground and background equal, people aside. Also the home city set aside vs live.
- [ ] Fix every difference at its cause (sight, light, lamps, the minute, the theme); five checks; shots by night beside and inside a neighbour; commit `render: a city looks the same from outside and inside`.

### Task 14: One set of rules everywhere

From Jeff's play: after travelling to another city (↵ down from the overworld into it, or walking in), terminals (and likely other objects) have no collision; the overworld has no dev commands; the overworld's hint line ("arrows walk · enter goes down …") should go. Principle: no discrepancy between areas or layers — the open ground, the live city, a held city, the Net, however you got there (walked in, ↵ down, Net handover, a reopened run).

**Files:** found by the tests; `crates/strata-zero/src/screens/outside.rs` (hints, dev keys), `crates/strata-zero/src/screens/explore.rs`, `crates/strata-zero/src/outside/`

- [ ] Test `every_object_blocks_the_same_everywhere`: for each way into a city (walked in across the line, ↵ down from the overworld onto its tile, a Net handover then jack out, a run saved and reopened there) and for the open ground and a held city seen from outside: stepping into each blocking kind present (wall, terminal, vending machine, furniture, a construct's wall on the Net, a lamp post or any feature the city blocks on) is refused exactly as in the home city at start; walking onto open cells moves you. Fix every difference at its cause.
- [ ] Test `the_overworld_takes_dev_commands`: with dev unlocked, the console (and every dev key the map screen has that makes sense without a street: the console, the inspector of the tile, teleport/jump) opens on the overworld and the Net overworld; locked, they don't.
- [ ] The overworld's and the Net overworld's hint lines go entirely (the `?` help stays); test that the drawn screen holds no hint line.
- [ ] Five checks; commit `outside: one set of rules on every map`.

### Task 15: The overworld in motion

Jeff picked round 2 of `docs/superpowers/specs/2026-11-05-overworld-motion-mock/` (spec, "After the first play"). Build it in the real overworld exactly as `round2.html` shows it, with its rates (the page's notes table lists every layer's rate and what the Rust side needs).

**Files:** `crates/strata-zero/src/render/overworld.rs` (the look "A" cells take the tick and the hour), `crates/strata-zero/src/screens/outside.rs` (pass the tick and minute of day), new modules under `render/` if a layer is big enough to deserve one.

**Interfaces:**
- The walked overworld's cells are a function of (world, tile, tick, minute of day): neon (each district's tiles breathe on a 600–1080-tick period rippled across the district in 5 brightness steps; half the signs stutter once every 60–90 s for 0.4 s; windows change every 60 s; tower beacons blink 6 ticks in 180); weather (no rain; swell, river glints, wind over fields, cloud shadows a tile at a time; a mountain gust every 40–70 s, ~8 s across the range in a 5-tile band: mountains whiten, hills take dust, a lake ripples); beacons (relay blink every 2 s with a halo after dark, the checkpoint's red cross every 12 ticks and its road's red glow, the other place beacons as in round 2); nothing on the roads. All of it from the tick and hashes of the cell and the world's seed (deterministic, no RNG state, nothing saved). The hour drives day/dusk/night as in the mock. 1 tick = 1/30 s (the app's frame tick).
- The Net overworld is unchanged.
- Cost: a frame of the overworld at 200×60 stays under ~2 ms in release (add a case to `streamed_draw_cost` or a sibling).

- [ ] Tests: `the_overworld_moves_with_the_tick` (two ticks apart, some cells differ; the same tick twice, identical); `a_districts_breath_is_slow` (no cell's neon step changes more than once in 300 ticks); `no_light_on_the_roads` (road cells never take a beacon/lamp colour); `the_gust_comes_and_goes` (over 70 s of ticks a gust crosses the mountains, and most ticks have none); `the_net_overworld_is_still` (identical across ticks); `the_overworld_reads_the_hour` (day and night differ).
- [ ] Shots: the overworld at day, dusk and night, and mid-gust; compare with `round2.html` side by side and match it.
- [ ] Five checks; commit `render: the overworld in motion`.

### Task 16: Solid signs, round night sight, city lights from afar

Jeff's decisions after Tasks 13–14 (2026-11-05): billboards and road signs block you; the night sight is round like the day's; lit things are seen from farther where nothing blocks the view.

**Files:** `crates/citygen/src/tile.rs` (`Tile::is_walkable`: `Feature::Sign` blocks), `crates/citygen/src/sight.rs` (the dark radius round: `dx² + dy² <= r²`, as `fov.rs` casts), `crates/citygen/src/lighting/tuning.rs` (`LIT_RADIUS`), `crates/strata-zero/src/walker/mod.rs` (`update_fov`'s cast), and whatever the change breaks.

- [ ] Signs: every `Feature::Sign` cell (billboard caps and letters, truck-stop road signs, holo ads) blocks walking for the player, NPCs and pathing alike. Check every generator stage that places signs still leaves the city connected (the validators and the connectivity tests across the usual seed ranges); where a sign would cut a way through, the generator moves or drops it (deterministic). Test: `a_sign_blocks_you` (street and open ground) and the existing connectivity tests green.
- [ ] Night sight: the dark radius is a circle in cells (`sight::lit_or_near` and `sight::sees`), matching the day and the FOV cast. Test: `the_night_sight_is_round` (at 22:00 on open ground, a cell at (r, 0) is seen and one at (r-1, r-1) beyond the circle isn't); NPC sight tests updated to the round rule.
- [ ] Far lights: a lit cell (one the light rule says is lit enough to be seen past the dark radius) is seen as far as the view reaches, up to `LIT_RADIUS` raised to 96 cells, where the line of sight is clear. Test: `a_city_glows_from_afar` (at night on open ground 60–90 cells from a neighbour, its lit cells are visible and its dark cells aren't); the release draw/step cost stays in budget (measure a step's sight update at the new radius and report it).
- [ ] Shots at night beside a neighbour from 30, 60 and 90 cells; five checks; commit `sight: solid signs, a round night, city lights from afar`.

### Task 17: Every city lives

Found in Task 12: seed 3's corporate neighbour has 116 people, 101 working there with no home in it and 15 standing at spots; the sim gives them no trips all day, so the city stands still from outside and inside — Jeff's "the city feels abandoned". Ruling (mine, Jeff can overturn): people who work in a city but have no home in it commute — they live off the map, come in along a road where it crosses the city's edge in the morning, walk to work, and walk back out and leave in the evening (and the same for any other trip whose end has no place in the city).

**Files:** the `sim` crate (seeding and trips), `crates/strata-zero` where people are drawn/kept, found by the tests.

- [ ] First, a table in the report: for every settlement of seed 3's world, people, with a home in the city, without, trips in a day today.
- [ ] Tests: `a_city_without_homes_still_has_its_commute` (seed 3's corporate neighbour: commuters appear at road entries in the morning, reach their work, and leave in the evening; the day has trips); `commuters_come_in_where_a_road_crosses_the_edge`; a city with homes behaves exactly as before (its trips unchanged, by comparing a day's trips before/after on the home city).
- [ ] Deterministic; the run-on and catch-up (`World::advance`) handle commuters; saved runs with people from before open (docs/saving.md rules for any saved type that changes); five checks; shots by day in that city; commit `sim: commuters for cities without homes`.
