# Sites Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One generator lays out every hotel and office (and, from the console, metro stations) from `flavor/sites.ron`, and a mission's copy of a site holds its guards, devices and objective until the mission ends (#34).

**Architecture:**
- `citygen::interior::venue` makes a layout from a type (data), a frame (a building's, or the type's own) and a seed, in the spec's stages: frame, levels by role, zones and locks, rooms and set pieces, links, ways in. It draws one level of a layout as a `Floor`, checks a layout through a graph of its rooms (`validate_venue`), and lays a mission layer on it.
- `FloorPlan` becomes the one description of any building's levels: a list of levels, the stratum each level's doors open onto, and links between levels. Today's buildings keep today's floors (pinned in Task 1). A venue's plan carries its layout (`FloorPlan.venue`), so the layout is saved with the plan.
- `strata_zero::venues` keeps mission copies in `Interiors.copies`: `begin`/`end`, flagged and barred doors, the copy's floors drawn with the layer, guards as inert foes. Copies travel in `Stay`/`StayParts` because `Interiors` does.

**Tech Stack:** Rust workspace; `crates/citygen` (no terminal dependencies) and `crates/strata-zero` (ratatui, crossterm); RON (`ron` 0.10), serde, `rand_chacha::ChaCha8Rng`; `postcard` and `rmp-serde` in tests.

**Spec:** `docs/superpowers/specs/2026-11-10-site-generator-design.md`; its mocks and their Python sources are in `docs/superpowers/specs/2026-11-10-site-generator-mock/` (`hotel.py` + `levels.py` are the Pale Orchid, `metro.py` is Cinder Cross).

## Global Constraints

- **No type's name in Rust.** The spec: "The Rust never names a type, so a new type is a file change", and "A type's name in the Rust code" is a failure. Rust code and Rust tests never spell a type's id or name (hotel, office, metro station, the test type's id). They reach types through `VenuePack::for_kind(BuildingKind)`, `VenuePack::by_id(&str)` with an id read from data or typed at the console, or by iterating `VenuePack::builtin().types`. A test name may name a `BuildingKind` (an enum that predates types, as in `hotels_and_offices_are_venues_and_towers_are_not`) but never a type id.
- **Same seed, same site, every platform.** "The same inputs give the same site on every platform." "Generation follows the world's rule: no libm `powf`, `sin`, `cos` or `hypot`." In `venue` generation: integer arithmetic only (shares are percent), every rng is a `ChaCha8Rng` seeded from `citygen::rng::hash64`, and no `HashMap`/`HashSet` is iterated (use `Vec`, `BTreeMap`, `BTreeSet`; a `HashSet` used only for `contains`, as `layouts::Builder` does, is fine).
- **Saving** (CLAUDE.md, `docs/saving.md`): "Saved types derive `Serialize` and `Deserialize`. No `HashSet` (use `BTreeSet`), `Rc`, `Cell`, `Instant` or `&'static str` in them." "A new field in a saved type needs `#[serde(default)]`, and renaming or reshaping one needs a reader for its old shape (as `net::EchoRead` does); the fixtures in `runs/fixtures/` must keep opening. Never regenerate them to pass a test." This plan adds no `AppState` field. Enums that saves hold (`Terrain`, `Feature`, `RoomKind`) grow only at their end: "New generic kinds join the enum at its end."
- **Drawing** (CLAUDE.md): "Draw everything through ratatui." "Prefer exact `Color::Rgb` colours." "A new symbol that no font has fails `every_drawn_character_has_a_glyph`." Glyphs this plan adds: `↑` `↓` (escalator), `╪` (track), `▓` (shutter), `◆` (turret), `□` (container), `◎` (objective), `▮` (barred door). That test lives in `window/font.rs` and runs only with the window feature, so each task that adds a glyph also runs `cargo test -p strata-zero --features window --target-dir target/smoke every_drawn_character` before its commit. If one fails the glyph test on macOS, use one the game already draws instead and say so in the commit.
- **The word "site".** "`game::model::Site` is a place on the body for chrome, and `citygen::generate::site` (`CitySite`) is a city's place in the world." The code says *venue*: `citygen::interior::venue`, `strata_zero::venues`, `VenueLayout`, and the spec's `validate_site` is `validate_venue`. citygen also exports `Link` (a Net link) and `Guard` (construct ICE), so this plan's names are `LevelLink` and `GuardPost`. The data file keeps the spec's name, `flavor/sites.ron`, and the console command is `site`.
- **The size table lives in `flavor/sites.ron`, not in code:** Small 2–3 floors kept, 2 locks to the objective, at least 2 ways in, 6–10 guards; Medium 4–6, 3, 3, 12–20; Large 6–9, 4–5, 4, 25–40. Security is "0–255, the scale a subnet's `security` uses" (`u8`).
- **IP:** "inspired by" Cyberpunk 2020 only; no Cyberpunk 2077, CDPR or R. Talsorian proper nouns in data, copy or docs.
- **Committed docs** record decisions, not chat quotes. A person named in them is "Jeff".
- **Builds** use `--target-dir target/smoke` inside this worktree (its own `target/smoke`), never `target/` and never the main checkout's target dir. A task that touches `crates/strata-zero` also runs `cargo check --workspace --all-targets --features strata-zero/window --target-dir target/smoke`, since shared code can break the window build while plain tests pass.
- **Dev mode:** "`AppState::new()` has dev mode off and locked." A test that runs the console calls `st.unlock_dev()` first.
- **Commits:** an area prefix as `git log` uses (`citygen:`, `interiors:`, `venues:`, `flavor:`, `console:`, `shots:`, `manual:`, `docs:`); every message ends with the line `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; the last task's commit also says `Closes #34`.

## Choices this plan makes

Where the spec leaves a choice to the builder, or is silent, this plan decides:

1. **Names:** see "The word site" above. A mission's size is `MissionSize` (`citygen::Size` is a width and height).
2. **The plan is saved, not remade:** the layout rides in the building's `FloorPlan` (`venue: Option<Box<VenueLayout>>`, saved with `Interiors.plans`), so floors saved before a generator change and floors drawn after it agree.
3. **Copies live in `Interiors.copies`** (`#[serde(default)]`), so `Stay` and `StayParts` carry them through their existing `interiors` field and need no new field.
4. **Card readers are locks on doors** (`Floor.locks`), drawn as the door in the reader colour. "No device stands on a wall or in a doorway" applies to cameras, turrets, containers and the objective, which stand on floor spots.
5. **A mission fits the border's own lock** on a candidate door: a card reader on a card border, a keypad on a keypad border. Candidate doors are zone-border doors only, never the doors of private rooms (those already carry the keypad of their number, and no edge carries two locks); a layout with too few border doors holds fewer sizes.
6. **Crawls:** a crawl's grille is its landing tile (a `Ladder` tile, or a `Floor` tile for a crawl way in, with `Feature::Vent`). Crossing a crawl link or a crawl way in is one lock. In play the grille wants Tech ≥ `crawl_tech` (6, in `sites.ron`).
7. **Numbers:** a room is `level * 100 + n` at or above the street and `9000 + 100 * depth + n` below it, shown `B1-04`; zone locks are numbered `60000 + i` per level, so a lock's code keys `(building, level, number)` like a flat's.
8. **Doors:** a flagged door is the door glyph in `MAGENTA_RGB`; a barred door is `▮` in a dim grey. Their Look: `Flagged door` / `{type name} · a job inside`, and `Barred door` / `shut for the job`.
9. **A stair between kept floors that aren't neighbours** is drawn as any stair; its rows and Look name the next kept level.
10. **Console:** `site <type id> [small|medium|large] [objective id] [security 0-255] [seed]` and `site end`; `site` alone lists the type ids.
11. **Seeds:** the checker runs 50 per frame. If the test runs over 60 s in a debug build, it runs 20 and the commit says so.
12. **Shots:** one SVG per type and size with every kept level's map side by side.
13. **Codes found in the site** are a sticky note under a desk spot's Look; typed at the keypad, the journal learns them as it does today.
14. **Kept floors:** G, the objective's level and the levels the copy's links need to join them are always kept (a size that can't hold them is refused); the other door levels fill the count the mission seed draws, nearest G first, then levels next to the objective's.
15. **Frame for the console and shots:** the building's frame as the city made it; its top is raised to the type's own frame top (`VenueFrame::raised`) only when the building can't hold the asked size (Task 17's `dev_site`).
16. **Deep levels larger than the frame** grow east and south; the frame's area sits at the origin, so links line up.
17. **Landings:** lifts are two tiles wide as today; stairs, escalators, ladders and crawls one tile. Escalator stops sit 3–5 tiles apart.
18. **Talk in the checker:** a person of zone z hands over card z (a guard's post zone in a copy; a staff station's room zone in the plain building).
19. **Ways out** include crawl ways for the way of playing that crawls.
20. **Track:** a side platform along a level's far edge. The track (`Terrain::Track`) is walkable, because Jeff ruled that the player can walk on the rails. It ends at shutters at both ends, in the plain station and the copy alike, and the copy's Look says sealed. The track belongs to its platform's zone, so it opens no way around a lock. Trains on it come with the metro project.

## Review Focus

1. **An old save's hotels.** A hotel whose plan a save holds (visited before this change) keeps its floors and today's builder for floors not yet made; one never visited becomes a venue on first need, in the same run (Task 12, `an_old_saves_hotel_keeps_the_classic_builder`).
2. **`site` at the player's own building, or from inside the building.** Expect a refusal and nothing changed: the home's floors, rig and flat stay as they are (Task 17, `site_refuses_home_and_inside`).
3. **A track aimed at a floor the copy doesn't keep** (a person or spot on a dropped level). Expect the leg to say there's no way, no panic, and the route back after `site end` (Task 15, `a_track_to_a_dropped_floor_finds_no_way`).
4. **Guards standing in the way.** Inert foes block their tile. Expect no post or patrol tile on a link landing, a doorway, a doorstep or an exit (Task 14, `no_post_blocks_a_landing_doorway_or_doorstep`).
5. **Zone cards after a reopen and a trip.** Expect a zone card in the pack to still open its doors after the run is saved and reopened, and after walking to another city and back (Task 13, `zone_cards_survive_a_reopen_and_a_trip`).

## Files

citygen (`crates/citygen/src/interior/`):
- `plan.rs`: `FloorPlan` as levels, doors and links; `LevelDoor`, `LinkKind`, `LinkStop`, `LevelLink`, `CrawlWay`; the reader for old saves (`FloorPlanRead`); `FloorPlan::classic` (today's builder) and `FloorPlan::of_layout`.
- `floor.rs`: `DoorLock`, `Floor.locks`, `Room.name`, `Room.zone`, new `RoomKind`s.
- `layouts.rs`: today's builder, reading links; `Builder` usable without a city for venue levels.
- `venue/mod.rs` (new): module doc, re-exports, `#[cfg(test)] mod testutil;`.
- `venue/types.rs` (new): the pack: `VenuePack`, `VenueType` and the data types; `check`.
- `venue/frame.rs` (new): `VenueFrame`.
- `venue/layout.rs` (new): `VenueLayout`, `LevelLayout`, `lay_out`, numbers.
- `venue/levels.rs`, `venue/zones.rs`, `venue/rooms.rs`, `venue/links.rs`, `venue/ways.rs` (new): stages 2–6.
- `venue/draw.rs` (new): a level as a `Floor`.
- `venue/graph.rs` (new): the room graph and its searches.
- `venue/check.rs` (new): `validate_layout`, `validate_venue`, `validate_copy`, `holds`, `venue_fingerprint`.
- `venue/mission.rs` (new): `MissionInputs`, `MissionLayer`.
- `venue/test_type.ron` (new): the test-only type.
- `venue/testutil.rs` (new, `#[cfg(test)]`, `pub(crate)`): `test_type`, `city`, `medium`, `cities`, the helpers every venue test module shares (Task 4 creates it, Task 5 adds the cities).
- `crates/citygen/tests/venues.rs` (new): the checker over every type.
- `flavor/sites.ron` (new): the types.

strata-zero (`crates/strata-zero/src/`): `interiors.rs`, `credentials.rs`, `interact.rs`, `look.rs`, `foes.rs`, `clock.rs`, `venues.rs` (new), `render/map.rs`, `render/minimap.rs`, `render/tileset.rs`, `render/doors.rs` (new), `screens/explore.rs`, `screens/popup.rs`, `screens/console.rs`, `screens/shots.rs`, `track/route.rs`, `track/listings.rs`, `ui/panel.rs`, `render/atrium.rs`; `flavor/looks.ron`.

Docs: `docs/saving.md`, `docs/development.md`, `manual/06-the-city-and-the-world.md`.

Test commands used throughout:
- `cargo test -p citygen --target-dir target/smoke <filter>`
- `cargo test -p citygen --target-dir target/smoke --test venues`
- `cargo test -p strata-zero --target-dir target/smoke <filter>`
- green at a task's end: `cargo test --workspace --target-dir target/smoke`

---

### Task 1: Pin today's floors; a building's levels are a list

**Files:**
- Modify: `crates/citygen/src/interior/plan.rs` (fields `lowest`/`top` → `levels`; `FloorPlanRead`; `classic`)
- Modify: `crates/citygen/src/interior/mod.rs` (`validate_floor`; tests: the pin, the old-plan reader, every `plan.lowest`/`plan.top`)
- Modify: `crates/citygen/src/interior/layouts.rs:347-367` (`shafts`), `layouts/lights.rs` tests, `interior/home.rs` test
- Modify: `crates/strata-zero/src/interiors.rs` (`ensure`, `ride`, `climb`, `floor_rows`, `floor_now`; test helper `put_plan`), `interact.rs:517-537`, `look.rs:221-238`, `render/map.rs:430-441`, `render/minimap.rs` (`shaft_rows`, `floor_plan`), `render/atrium.rs`, `ui/panel.rs:95-99`, `track/route.rs:405-420`, `screens/explore.rs` tests, `screens/shots.rs:2357`

**Interfaces:**
- Produces:
  ```rust
  // citygen::interior::plan
  pub struct FloorPlan { /* ... */ pub levels: Vec<i32>, /* lowest first; replaces lowest and top */ }
  impl FloorPlan {
      /// Today's builder's plan. `new` calls it until Task 12 dispatches to venues.
      pub fn classic(city: &City, id: BuildingId) -> Option<FloorPlan>;
      pub fn levels(&self) -> impl Iterator<Item = i32> + '_;
      pub fn lowest(&self) -> i32;
      pub fn top(&self) -> i32;
      pub fn has_level(&self, level: i32) -> bool;
      pub fn floors(&self) -> i32; // levels.len()
      /// The next level in the list above (`up`) or below `level`.
      pub fn next_level(&self, level: i32, up: bool) -> Option<i32>;
  }
  // test-only, citygen::interior
  pub(crate) fn floor_fingerprint(f: &Floor) -> u64;
  // strata_zero::render::minimap
  pub fn shaft_rows(levels: &[i32], you: i32, wanted: Option<i32>, rows: usize) -> Vec<Option<i32>>;
  // strata_zero::interiors (test-only until Task 15)
  #[cfg(test)] pub fn put_plan(&mut self, plan: FloorPlan); // replaces the plan, drops its cached floors
  ```

- [ ] **Step 1: Write the pin on the untouched tree**

In `interior/mod.rs`'s tests add `floor_fingerprint` (it mixes, with `rng::hash64`, the `Debug` text of each tile's terrain and feature, of each room's rect, kind, door, private and number, and of each exit and station; light and glow are left out) and a `medium(seed)` helper (`SizePreset::Medium`, `districts: 7`).

```rust
/// Today's floors as one number per city: every closed building's lowest, ground,
/// bridge and top floors. The level, door and link refactors (Tasks 1-3) keep them.
#[test]
fn classic_floors_are_pinned() {
    const PINNED: [u64; 2] = [0, 0];
    for (i, c) in [city(), medium(8)].iter().enumerate() {
        let mut h = 0;
        for b in c.buildings.iter().filter(|b| b.kind.is_closed()) {
            let plan = FloorPlan::new(c, b.id).unwrap();
            let mut levels = vec![plan.lowest, 0, plan.top, plan.bridge.unwrap_or(0)];
            levels.sort();
            levels.dedup();
            for l in levels {
                h = crate::rng::hash64(h, floor_fingerprint(&generate_floor(c, &plan, l)));
            }
        }
        assert_eq!(h, PINNED[i], "city {i}");
    }
}
```

- [ ] **Step 2: Record the numbers and commit the pin**

Run: `cargo test -p citygen --target-dir target/smoke classic_floors_are_pinned`
Expected: FAIL, `left` is the city's number. Put both `left` values into `PINNED`, run again: PASS.

```bash
git add crates/citygen/src/interior/mod.rs
git commit -F - <<'EOF'
citygen: pin today's floors

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

- [ ] **Step 3: Write the failing tests**

In `interior/mod.rs` tests, an old plan's shape (the fields saves wrote before this branch) and its reader:

```rust
#[derive(serde::Serialize)]
struct OldPlan {
    building: BuildingId, kind: BuildingKind, theme: Theme, footprint: Rect, size: Size,
    lowest: i32, top: i32, bridge: Option<i32>, roof: bool, lift: Option<Pos>, stair: Pos,
    atrium: Option<Rect>, home_level: Option<i32>, clinic: Option<ClinicPlan>,
}
// OldPlan::of(&FloorPlan) fills the old fields from the plan's accessors.

#[test]
fn an_old_plan_reads_back_the_same() {
    let c = city();
    for b in c.buildings.iter().filter(|b| b.kind.is_closed()) {
        let plan = FloorPlan::classic(&c, b.id).unwrap();
        let bytes = rmp_serde::to_vec_named(&OldPlan::of(&plan)).unwrap();
        let back: FloorPlan = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(back, plan, "{:?}", b.id);
        let again: FloorPlan = rmp_serde::from_slice(&rmp_serde::to_vec_named(&plan).unwrap()).unwrap();
        assert_eq!(again, plan);
    }
}

#[test]
fn a_level_list_steps_over_its_gaps() {
    let c = city();
    let b = c.buildings.iter().find(|b| b.kind == BuildingKind::CorpTower).unwrap();
    let mut plan = FloorPlan::classic(&c, b.id).unwrap();
    plan.levels.retain(|l| ![2, 3].contains(l));
    assert_eq!(plan.next_level(1, true), Some(4));
    assert_eq!(plan.next_level(4, false), Some(1));
    assert_eq!(plan.next_level(plan.top(), true), None);
    assert!(!plan.has_level(2) && plan.has_level(4));
    assert_eq!(plan.floors(), plan.levels.len() as i32);
}
```

In `render/minimap.rs` tests, `shaft_rows_choose` takes the list (`&(-1..=27).collect::<Vec<_>>()` and so on, same expectations), and:

```rust
#[test]
fn shaft_rows_follow_the_level_list() {
    let f = |l: i32| Some(l);
    // -3 and -1 are neighbours in the list: no gap between them.
    assert_eq!(shaft_rows(&[-3, -1, 0, 1, 2], 0, None, 12), vec![f(2), f(1), f(0), f(-1), f(-3)]);
    // Today's trim order: the end gaps go first, so three rows keep 5, the gap below it and 0.
    assert_eq!(shaft_rows(&[0, 1, 5, 9], 5, Some(0), 3), vec![f(5), None, f(0)]);
}
```

In `interiors.rs` tests (a CorpTower of `home_city()`, its plan given 2F and 3F removed through `put_plan`, the player on the stair on 1F):

```rust
#[test]
fn levels_with_a_gap_climb_and_ride_over_it() {
    // ...
    assert_eq!(climb(&mut st, true).unwrap(), "Entering `4F`");
    assert!(st.interiors.ensure(&city, b, 2).is_none());
    assert!(floor_rows(st.interiors.plan_of(b).unwrap(), 0).iter().all(|(l, _)| ![2, 3].contains(l)));
}
```

- [ ] **Step 4: Run them to make sure they fail**

Run: `cargo test -p citygen --target-dir target/smoke interior::`
Expected: FAIL to compile (`classic`, `levels`, `next_level` are not defined).

- [ ] **Step 5: Implement the level list**

- `FloorPlan` loses `lowest` and `top` and gains `levels: Vec<i32>`; rename `FloorPlan::new`'s body to `classic` and have `new` call it. `levels` is `(lowest..=top).collect()`.
- Deserialize `FloorPlan` through `#[serde(from = "FloorPlanRead")]`: `FloorPlanRead` (private, `Deserialize` only) has every field of the new shape with `#[serde(default)]` on the new ones, and the old ones as `#[serde(default)] lowest: Option<i32>, top: Option<i32>`; `From<FloorPlanRead>` fills an empty `levels` from `lowest..=top`. Serialize writes the new shape only. Tasks 2 and 3 add to the same reader.
- Every `plan.lowest`/`plan.top`/`lowest..=top` reader moves to the methods; bounds checks become `has_level`; the stair rows, `climb`, the map's up/down flags (`render/map.rs` `connector`) and the Look use `next_level`; `floor_rows` lists `plan.levels()`; `shaft_rows` takes the list, and a gap row marks list entries skipped (index distance > 1), with a gap before the first row when the top isn't shown and after the last when the lowest isn't.
- In the pin, `plan.lowest`/`plan.top` become `plan.lowest()`/`plan.top()` and `FloorPlan::new` becomes `FloorPlan::classic`; the numbers stay.

- [ ] **Step 6: Run the tests to make sure they pass**

Run: `cargo test -p citygen --target-dir target/smoke interior:: && cargo test -p strata-zero --target-dir target/smoke levels_with_a_gap shaft_rows`
Expected: PASS, `classic_floors_are_pinned` included.
Run: `cargo test --workspace --target-dir target/smoke && cargo check --workspace --all-targets --features strata-zero/window --target-dir target/smoke`
Expected: PASS (the fixtures open: their plans read through `FloorPlanRead`).

- [ ] **Step 7: Commit**

```bash
git add crates/citygen crates/strata-zero
git commit -F - <<'EOF'
citygen: a building's levels are a list, not lowest to top

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 2: Each level says which stratum its doors open onto

**Files:**
- Modify: `crates/citygen/src/interior/plan.rs` (fields `bridge`/`roof` → `doors`; `exit_stratum` → `door_stratum`; reader)
- Modify: `crates/citygen/src/interior/layouts.rs:209-286` (`exits`), `interior/home.rs`, `interior/mod.rs` tests
- Modify: `crates/strata-zero/src/interiors.rs` (delete `level_for`; `enter`, `floor_rows` marks), `track/route.rs`, `track/listings.rs`, `track/mod.rs`, `screens/explore.rs` tests

**Interfaces:**
- Consumes: Task 1's `FloorPlan.levels` and `FloorPlanRead`.
- Produces:
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
  pub struct LevelDoor { pub level: i32, pub stratum: StratumKind }
  pub struct FloorPlan { /* ... */ pub doors: Vec<LevelDoor>, /* replaces bridge and roof */ }
  impl FloorPlan {
      /// The stratum a level's doors open onto: the first entry for the level.
      pub fn door_stratum(&self, level: i32) -> Option<StratumKind>; // was exit_stratum
      /// The level a city-map door on `k` opens onto: the first entry for `k`.
      pub fn door_level(&self, k: StratumKind) -> Option<i32>;       // was interiors::level_for
  }
  ```

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn each_level_says_which_stratum_its_doors_open_onto() {
    let c = city();
    for b in c.buildings.iter().filter(|b| b.kind.is_closed()) {
        let plan = FloorPlan::classic(&c, b.id).unwrap();
        assert_eq!(plan.door_stratum(0), Some(StratumKind::Street));
        assert_eq!(plan.door_level(StratumKind::Street), Some(0));
        assert_eq!(plan.door_level(StratumKind::Undercity), (plan.lowest() == -1).then_some(-1));
        let skyways = c.portals(b.id).iter().any(|(k, _)| *k == StratumKind::Skyways);
        assert_eq!(plan.door_level(StratumKind::Skyways).is_some(), skyways);
        assert_eq!(
            plan.door_level(StratumKind::Heights),
            (b.span.hi == StratumKind::Heights).then_some(plan.top())
        );
        assert_eq!(plan.door_level(StratumKind::Net), None);
    }
}
```

`OldPlan::of` now fills `bridge` from `door_level(Skyways)` and `roof` from `door_level(Heights).is_some()`; `an_old_plan_reads_back_the_same` covers the reader.

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test -p citygen --target-dir target/smoke each_level_says`
Expected: FAIL to compile (`door_stratum`, `door_level`).

- [ ] **Step 3: Implement the doors**

`classic` builds `doors` in today's `exit_stratum` order: `(-1, Undercity)` when the lowest level is −1, `(0, Street)`, `(bridge, Skyways)` when there is a bridge, `(top, Heights)` when there is a roof. `FloorPlanRead` takes old `bridge: Option<i32>` and `roof: bool` (`#[serde(default)]`) and fills an empty `doors` the same way. `interiors::level_for` goes; its callers use `door_level`. `floor_rows` marks `bridge` and `roof` from `door_stratum`. `classic` keeps a local `bridge` for `home_level`. Task 1's pin (`classic_floors_are_pinned`) now reads `plan.door_level(StratumKind::Skyways).unwrap_or(0)` where it read `plan.bridge.unwrap_or(0)`; its pinned numbers stay, since this task deletes the field.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p citygen --target-dir target/smoke interior::`
Expected: PASS, the pin unchanged.
Run: `cargo test --workspace --target-dir target/smoke && cargo check --workspace --all-targets --features strata-zero/window --target-dir target/smoke`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/citygen crates/strata-zero
git commit -F - <<'EOF'
citygen: each level says which stratum its doors open onto

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 3: Every building moves between levels by links

**Files:**
- Modify: `crates/citygen/src/interior/plan.rs` (fields `lift`/`stair` → `links`; the link types; reader)
- Modify: `crates/citygen/src/tile.rs` (`Terrain::Escalator` at the end, walkable, in `ALL_TERRAIN`)
- Modify: `crates/citygen/src/interior/layouts.rs` (`shafts` draws every link stop; `connect` anchors on `plan.anchor`; roof door beside `stair_on`), `layouts/atrium.rs`, `interior/mod.rs` (`validate_floor`, tests)
- Modify: `crates/strata-zero/src/interiors.rs` (`ride`, `climb`, `floor_rows`, `Placement` anchor), `interact.rs` (`ride_rows`, stair rows for every link kind), `screens/popup.rs`, `render/map.rs` (`connector`), `render/minimap.rs` (a mark per link), `render/tileset.rs` (escalator glyph), `look.rs` + `flavor/looks.ron` (stairwell, lift, escalator, ladder fixtures), `track/route.rs` (`to_level`), tests in `screens/explore.rs`, `track/`, `render/minimap.rs`

**Interfaces:**
- Consumes: Tasks 1–2.
- Produces:
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
  pub enum LinkKind { Lift, Stair, Escalator, Ladder, Crawl }
  impl LinkKind { pub fn terrain(self) -> Terrain; } // Lift, Stair, Escalator, Ladder, Ladder

  #[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
  pub struct LinkStop {
      pub level: i32,
      /// The landing; a lift's bank is this tile and the one east of it.
      pub at: Pos,
      /// The zone card a lift asks for to stop here; 0 asks none.
      #[serde(default)] pub card: u8,
      /// You can get on here, and get off here.
      #[serde(default = "yes")] pub on: bool,
      #[serde(default = "yes")] pub off: bool,
  }

  #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
  pub struct LevelLink {
      pub kind: LinkKind,
      /// Lowest first.
      pub stops: Vec<LinkStop>,
      #[serde(default)] pub name: Option<String>,
  }
  impl LevelLink {
      pub fn stop(&self, level: i32) -> Option<&LinkStop>;
      pub fn tiles(&self, level: i32) -> Vec<Pos>;
  }

  pub struct FloorPlan { /* ... */ pub links: Vec<LevelLink>, /* replaces lift and stair */ }
  impl FloorPlan {
      /// The link with a landing tile at `p` on `level`.
      pub fn link_at(&self, level: i32, p: Pos) -> Option<usize>;
      /// Every link's landing tiles on a level, in link order.
      pub fn landings(&self, level: i32) -> Vec<Pos>;
      /// From `level` on link `link`: the next stop up or down you can get off at;
      /// None when you can't get on here. Stops are read through `plan.levels`: a stop whose
      /// level the plan lacks is skipped.
      pub fn next_stop(&self, link: usize, level: i32, up: bool) -> Option<i32>;
      /// The levels a lift takes you to from `from`: its stops you can get off at,
      /// `from` left out; empty when you can't get on at `from`. Stops on levels the plan
      /// lacks are left out.
      pub fn rides(&self, link: usize, from: i32) -> Vec<i32>;
      pub fn lift_on(&self, level: i32) -> Option<Pos>;  // the first Lift's landing
      pub fn stair_on(&self, level: i32) -> Option<Pos>; // the first Stair's landing
      /// Where a floor's people and its connect pass gather: the first landing, else
      /// `(w / 2 - 1, h / 2)`, where the stairwell stood in a one-floor building.
      pub fn anchor(&self, level: i32) -> Pos;
  }
  // strata_zero::interiors
  pub fn floor_rows(plan: &FloorPlan, link: usize, here: i32) -> Vec<(i32, String)>;
  pub fn ride(st: &mut AppState, b: BuildingId, level: i32) -> Result<String, String>; // the lift under you
  pub fn climb(st: &mut AppState, up: bool) -> Result<String, String>;                  // the link under you
  ```

- [ ] **Step 1: Write the failing tests**

In `interior/mod.rs`:

```rust
#[test]
fn classic_plans_have_one_lift_and_one_stair() {
    let c = city();
    for b in c.buildings.iter().filter(|b| b.kind.is_closed()) {
        let plan = FloorPlan::classic(&c, b.id).unwrap();
        let lifts: Vec<_> = plan.links.iter().filter(|l| l.kind == LinkKind::Lift).collect();
        let stairs: Vec<_> = plan.links.iter().filter(|l| l.kind == LinkKind::Stair).collect();
        assert_eq!(lifts.len(), usize::from(plan.floors() >= 3));
        assert_eq!(stairs.len(), usize::from(plan.floors() >= 2));
        for l in lifts.iter().chain(&stairs) {
            assert_eq!(l.stops.iter().map(|s| s.level).collect::<Vec<_>>(), plan.levels);
            assert!(l.stops.iter().all(|s| s.at == l.stops[0].at && s.card == 0 && s.on && s.off));
        }
        if let Some(l) = lifts.first() {
            assert_eq!(plan.links[0].kind, LinkKind::Lift, "the lift comes first");
            assert_eq!(l.tiles(0), vec![l.stops[0].at, l.stops[0].at.offset(1, 0)]);
        }
    }
}

#[test]
fn next_stop_and_rides_follow_on_and_off() {
    let mut plan = FloorPlan::classic(&city(), /* a 5-floor building */).unwrap();
    let p = Pos::new(3, 3);
    let stop = |level, on, off| LinkStop { level, at: p, card: 0, on, off };
    plan.links = vec![
        // An up escalator.
        LevelLink { kind: LinkKind::Escalator, stops: vec![stop(0, true, false), stop(1, false, true)], name: None },
        // A fire stair you leave only at its ends.
        LevelLink { kind: LinkKind::Stair, stops: (0..=4).map(|l| stop(l, true, l == 0 || l == 4)).collect(), name: None },
        // A lift that passes 1F and 3F.
        LevelLink { kind: LinkKind::Lift, stops: [0, 2, 4].into_iter().map(|l| stop(l, true, true)).collect(), name: None },
    ];
    assert_eq!(plan.next_stop(0, 0, true), Some(1));
    assert_eq!(plan.next_stop(0, 1, false), None);
    assert_eq!(plan.next_stop(1, 2, false), Some(0));
    assert_eq!(plan.next_stop(1, 2, true), Some(4));
    assert_eq!(plan.rides(2, 0), vec![2, 4]);
    assert_eq!(plan.rides(2, 1), Vec::<i32>::new());
}
```

Links are always read through `plan.levels`, so Task 1's `levels_with_a_gap_climb_and_ride_over_it` (stops left on 2F and 3F in the links, those levels cut from `levels` only) still passes; this test pins the rule:

```rust
#[test]
fn links_skip_a_stop_on_a_level_the_plan_lacks() {
    // The classic lift and stair of a 5-floor building with 2 and 3 cut from plan.levels:
    // next_stop(stair, 1, true) == Some(4); next_stop(stair, 4, false) == Some(1);
    // rides(lift, 0) holds neither 2 nor 3.
}
```

In `interiors.rs` tests (a CorpTower of `home_city()`; links edited with `put_plan`, its floors generated by `ensure`):

```rust
#[test]
fn an_escalator_and_a_ladder_climb_like_stairs() {
    // An up escalator from p0 on L to p1 on 1F (three tiles apart), a ladder at q between 1F and 2F.
    // On p0: climb(up) lands on p1 on 1F. On p1: climb(down) is "nothing leads down from here".
    // On q on 1F: climb(up) lands on q on 2F; climb(down) back on 1F.
    assert_eq!(st.interiors.floor(b, 0).unwrap().1.tiles[p0].terrain, Terrain::Escalator);
}

#[test]
fn the_lift_rides_only_where_it_stops() {
    // The lift's stops cut to L, 2F and the top floor; the player on its bank on L.
    // ride_rows has three rows, the top floor, 2F and L (L disabled: you're on it), and no other;
    // ride(st, b, 1) is Err("the lift doesn't stop there");
    // ride(st, b, 2) lands on the bank on 2F.
}
```

In `track/route.rs` tests: `the_track_takes_a_lift_that_stops_there` (a target on 2F of a tower whose only lift skips 2F: the leg goes to the stairs, `Through::Stairs { up: true, to: 2 }`).

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p citygen --target-dir target/smoke links next_stop classic_plans`
Expected: FAIL to compile (`LinkKind`, `LevelLink`, `links`).

- [ ] **Step 3: Implement the links**

- `classic` makes the lift link (every level at today's lift position, when there are three floors or more) then the stair link (every level at today's stair position, when there are two or more). `FloorPlanRead` takes old `lift: Option<Pos>` and `stair: Option<Pos>` and builds the same two links when `links` is empty.
- `shafts` draws every stop on the level with `kind.terrain()` (both tiles of a lift); the roof door stands east of `stair_on(level)`; `connect` and `Placement` anchor on `anchor(level)`; `validate_floor` checks each stop's tiles carry its kind's terrain and that every exit reaches one of `landings(level)`.
- Link stops are read through `plan.levels`: `next_stop`, `rides` and `floor_rows` skip a stop whose level the plan lacks, so a plan cut from `levels` alone and a plan cut by Task 15's `kept` agree. `floor_rows` takes the link now, so Task 1's `levels_with_a_gap…` test calls `floor_rows(plan, 0, 0)` (link 0, from level 0).
- `climb` uses the link under the player and lands on that link's stop on the next level (`next_stop`); `ride` uses the lift under the player, refuses a level not in `rides` with `"the lift doesn't stop there"`, and lands on its stop. Errors keep today's words: `"nothing leads up from here"`, `"no lift here"`, `"lifts locked down"`.
- Rows: a Stair, Escalator, Ladder or Crawl landing offers `>`/`<` to `next_stop`; a Lift landing offers `ride_rows` from `floor_rows(plan, link, here)`: `rides` and your own stop (greyed), top first, marked as today. The map's `connector` reports a Lift as `ConnectorKind::Lift`, Stair and Escalator as `Stair`, Ladder and Crawl as `Ladder`, with `up`/`down` from `next_stop`. The escalator glyph is `↑` up only, `↓` down only, `⇅` both, in the stair's colours.
- `to_level` picks, among links with a stop on this level you can get on at: a lift (when lifts answer) whose `rides` holds the target level; else a Stair, Escalator or Ladder whose `next_stop` moves toward it; else a lift to the level nearest it; the shortest walk wins among equals. With today's one lift and one stair this is today's choice.
- `OldPlan::of` fills `lift` from the Lift link and `stair` from `stair_on(lowest)`, else `anchor(0)`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p citygen --target-dir target/smoke interior::`
Expected: PASS, `classic_floors_are_pinned` unchanged.
Run: `cargo test --workspace --target-dir target/smoke && cargo check --workspace --all-targets --features strata-zero/window --target-dir target/smoke`
Expected: PASS.
Run: `cargo test -p strata-zero --features window --target-dir target/smoke every_drawn_character`
Expected: PASS (`↑` and `↓` are new; `⇅` is already drawn).

- [ ] **Step 5: Commit**

```bash
git add crates/citygen crates/strata-zero flavor/looks.ron
git commit -F - <<'EOF'
citygen: every building moves between levels by links

Today's buildings get one lift and one stair as links, so their floors don't change.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 4: Venue types as data

**Files:**
- Create: `crates/citygen/src/interior/venue/mod.rs`, `venue/types.rs`, `venue/test_type.ron`, `venue/testutil.rs`, `flavor/sites.ron`
- Modify: `crates/citygen/src/interior/mod.rs` (`pub mod venue;`), `interior/floor.rs` (`RoomKind` grows), `interior/layouts.rs` (`walled`), `lighting/tuning.rs` (`room_light` for the new kinds), `crates/strata-zero/src/look.rs` + `flavor/looks.ron` only if a test there wants a Look line per room kind

**Interfaces:**
- Consumes: `LinkKind` (Task 3), `BuildingKind`, `StratumKind`, `Dir`, `Furniture`, `Role`, `RoomKind`.
- Produces (all in `citygen::interior::venue`, reachable as `citygen::venue::…`; every pack struct is `#[serde(deny_unknown_fields)]`):
  ```rust
  pub enum MissionSize { Small, Medium, Large }                  // Ord
  pub enum Means { Card, Code, Keyjack, Net, Force, Talk, Crawl } // in the data
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
  pub struct Ways(pub u8);                                       // a set of Means, in layouts
  impl Ways {
      pub const CARD: Ways; pub const CODE: Ways; pub const KEYJACK: Ways; pub const NET: Ways;
      pub const FORCE: Ways; pub const TALK: Ways; pub const CRAWL: Ways;
      pub fn of(means: &[Means]) -> Ways;
      pub fn has(self, w: Ways) -> bool; // any bit of w
  }                                       // + BitOr
  pub enum LockKind { Card, Keypad, Grille }
  pub enum PieceKind { Ramp, Track, GateLine }
  pub enum Spine { Any, Straight, Bent, Ring, Hall } // Default: Any
  pub enum LinkDir { Both, Up, Down }   // Default: Both
  pub enum Alight { All, Ends }         // Default: All
  pub enum Serves { All, Roles(Vec<String>), Levels(Vec<i32>) }

  pub struct VenuePack { pub crawl_tech: u8, pub sizes: Vec<SizeRule>, pub types: Vec<VenueType> }
  pub struct SizeRule { pub size: MissionSize, pub floors: (u8, u8), pub locks: (u8, u8), pub ways_in: u8, pub guards: (u16, u16) }
  pub struct VenueType {
      pub id: String, pub name: String,
      pub builds: Vec<BuildingKind>,
      pub frame: OwnFrame,
      /// Public first; a zone's rank is its index.
      pub zones: Vec<String>,
      pub borders: Vec<BorderDef>,
      pub levels: LevelRoles,
      pub rooms: Vec<RoomDef>,
      #[serde(default)] pub pieces: Vec<PieceDef>,
      pub links: Vec<LinkDef>,
      #[serde(default)] pub crawls_in: Vec<CrawlInDef>,
      pub objectives: Vec<ObjectiveDef>,
      pub security: Vec<SecurityDef>,
  }
  pub struct OwnFrame { pub kind: BuildingKind, pub footprint: (i32, i32), pub top: i32,
      pub undercity: bool, pub roof: bool, #[serde(default)] pub bridge: Option<i32>, pub doors: Vec<OwnDoor> }
  pub struct OwnDoor { pub stratum: StratumKind, pub side: Dir, pub at: i32 } // footprint tiles along that side
  /// The lock between two zones (outer, inner) and the ways past it.
  pub struct BorderDef { pub zones: (String, String), pub lock: LockKind,
      #[serde(default)] pub always: bool, #[serde(default)] pub one_way: bool, pub ways: Vec<Means> }
  pub struct LevelRoles { #[serde(default)] pub below: Vec<LevelDef>, pub ground: LevelDef,
      #[serde(default)] pub middle: Option<LevelDef>, #[serde(default)] pub top: Vec<LevelDef> }
  pub struct LevelDef { pub name: String, pub bands: Vec<BandDef>, #[serde(default)] pub pieces: Vec<String>,
      /// Percent of the frame's floor, width and height; over 100 only below -1.
      #[serde(default = "full")] pub scale: (u8, u8), #[serde(default)] pub spine: Spine }
  /// A strip of the level across its depth, front to back, in one zone.
  pub struct BandDef { pub zone: String, pub share: u8, pub rooms: Vec<String>,
      #[serde(default)] pub fill: Option<String>, #[serde(default = "one")] pub doors: u8 }
  pub struct RoomDef { pub name: String, pub kind: RoomKind, pub size: ((i32, i32), (i32, i32)),
      #[serde(default)] pub private: bool, #[serde(default)] pub furniture: Vec<Furniture>,
      #[serde(default)] pub stations: Vec<Role> }
  pub struct PieceDef { pub name: String, pub kind: PieceKind }
  pub struct LinkDef { pub name: String, pub kind: LinkKind, pub zone: String, pub serves: Serves,
      #[serde(default)] pub cards: Vec<(String, String)>, // (level role, zone): a lift's card there
      #[serde(default)] pub dir: LinkDir, #[serde(default)] pub alight: Alight }
  pub struct CrawlInDef { pub stratum: StratumKind, pub zone: String }
  pub struct ObjectiveDef { pub id: String, pub name: String, pub room: String, #[serde(default)] pub level: Option<String> }
  pub struct SecurityDef { pub size: MissionSize, pub guards: Vec<(String, u8)>, // zone weights
      pub cameras: (u8, u8), pub turrets: (u8, u8), #[serde(default)] pub turret_zones: Vec<String>,
      pub containers: (u8, u8) }

  impl VenuePack {
      pub fn parse(text: &str) -> Result<VenuePack, String>; // parses and checks every type
      pub fn builtin() -> &'static VenuePack;               // flavor/sites.ron, parsed once (OnceLock)
      pub fn by_id(&self, id: &str) -> Option<&VenueType>;
      pub fn for_kind(&self, kind: BuildingKind) -> Option<&VenueType>;
      pub fn size(&self, size: MissionSize) -> &SizeRule;
  }
  impl VenueType {
      pub fn parse(text: &str) -> Result<VenueType, String>;
      pub fn check(&self, sizes: &[SizeRule]) -> Result<(), String>;
      pub fn zone(&self, name: &str) -> Option<u8>;
      pub fn room(&self, name: &str) -> Option<&RoomDef>;
      pub fn border(&self, outer: u8, inner: u8) -> Option<&BorderDef>;
  }
  // RoomKind gains, at its end: Platform, Track, Plant, Server, Kitchen, CarPark
  // (labels "platform", "track", "plant room", "server room", "kitchen", "car park";
  // Platform, Track and CarPark aren't walled).
  ```

- [ ] **Step 1: Write the failing tests**

In `venue/types.rs`:

```rust
fn sizes() -> &'static [SizeRule] { &VenuePack::builtin().sizes }
// test_type() is `venue::testutil`'s (see below).

#[test]
fn the_builtin_pack_loads_with_the_size_table() {
    let p = VenuePack::builtin();
    assert!(p.types.len() >= 3);
    assert_eq!(p.crawl_tech, 6);
    let r = |s| { let r = p.size(s); (r.floors, r.locks, r.ways_in, r.guards) };
    assert_eq!(r(MissionSize::Small), ((2, 3), (2, 2), 2, (6, 10)));
    assert_eq!(r(MissionSize::Medium), ((4, 6), (3, 3), 3, (12, 20)));
    assert_eq!(r(MissionSize::Large), ((6, 9), (4, 5), 4, (25, 40)));
    for t in &p.types { t.check(&p.sizes).unwrap_or_else(|e| panic!("{}: {e}", t.id)); }
}

#[test]
fn a_building_kind_has_at_most_one_type() {
    let p = VenuePack::builtin();
    for t in &p.types { for k in &t.builds {
        assert_eq!(p.types.iter().filter(|u| u.builds.contains(k)).count(), 1, "{k:?}");
    } }
    assert!(p.for_kind(BuildingKind::CorpTower).is_none(), "towers stay on today's builder");
}

#[test]
fn the_test_type_loads() { test_type().check(sizes()).unwrap(); }

#[test]
fn check_finds_what_is_wrong() {
    let bad = |f: &dyn Fn(&mut VenueType)| { let mut t = test_type(); f(&mut t); t.check(sizes()).unwrap_err() };
    assert!(bad(&|t| t.borders[0].zones.1 = "nowhere".into()).contains("nowhere"));
    assert!(bad(&|t| t.borders[0].ways.truncate(1)).contains("one way past"));
    assert!(bad(&|t| t.borders[0].lock = LockKind::Grille).contains("grille"));
    assert!(bad(&|t| t.objectives[0].room = "nowhere".into()).contains("nowhere"));
    assert!(bad(&|t| t.levels.ground.scale = (150, 100)).contains("scale"));
    assert!(bad(&|t| t.security.truncate(1)).contains("security"));
}
```

`check`'s rules, each an `Err` naming what it found: at least two zones; every zone, room, piece and role a type names exists; every border has two ways or more (`"one way past"`), and its ways fit its lock (a `Card` lock lists `Card` and neither `Code` nor `Keyjack`; a `Keypad` lock lists `Code`; `Grille` is never a border's); every link serves at least two levels of the type's own frame; every objective's room is in some level's bands (and on its `level` role when it names one); `scale` is 100 for the ground, middle and top roles and the first below role (−1), and 100–200 below that; shares of a level sum to 100; `security` covers every size. (A level need not have a link's zone: Task 8 lands the link in the next deeper zone the level has.)

`venue/testutil.rs` (`#[cfg(test)] mod testutil;` in `venue/mod.rs`) holds the helpers every venue test module shares, so no module keeps its own copy: `pub(crate) fn test_type() -> VenueType` (parses `test_type.ron`). Task 5 adds `city()`, `medium(seed)` and `cities()`; Tasks 5–11 and 14 use it.

`venue/test_type.ron` is a whole `Type(...)` that exercises every piece of the schema: three zones with one `one_way` border and one `Keypad` border; two below roles, the deeper with `scale: (150, 120)` and a `Track` piece; a ground role with a `GateLine`; a middle role with `spine: Any`; one top role with a `Ring` spine; a `Ramp`; links of all five kinds, among them an `Up` escalator, a stair with `alight: Ends` and a lift with a card on its top stop; a crawl in from `Heights` and one from `Undercity`; two objectives; security for every size; an own frame with two street doors, a Skyways door, `roof: true` and `undercity: true`.

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p citygen --target-dir target/smoke venue::types`
Expected: FAIL to compile (module `venue` doesn't exist).

- [ ] **Step 3: Write the types, the loader and the first cut of `flavor/sites.ron`**

`builtin()` parses `include_str!("../../../../../flavor/sites.ron")` once into a `std::sync::OnceLock` and panics with the error if the built-in file doesn't load (its test catches that first). Struct names in the RON are labels; `ron` 0.10 doesn't check them (`Story(` reads as `Book`).

The file opens with a comment in `missions.ron`'s style saying what a type is, what each field does and that sizes and security are first guesses to tune by playing. The shape, as the hotel's first cut:

```ron
Sites(
    crawl_tech: 6,
    sizes: [
        Size(size: Small,  floors: (2, 3), locks: (2, 2), ways_in: 2, guards: (6, 10)),
        Size(size: Medium, floors: (4, 6), locks: (3, 3), ways_in: 3, guards: (12, 20)),
        Size(size: Large,  floors: (6, 9), locks: (4, 5), ways_in: 4, guards: (25, 40)),
    ],
    types: [
        Type(
            id: "hotel",
            name: "hotel",
            builds: [Hotel],
            frame: Frame(kind: Hotel, footprint: (15, 9), top: 6, undercity: true, roof: true, bridge: Some(3),
                doors: [Door(stratum: Street, side: S, at: 5), Door(stratum: Street, side: E, at: 4),
                        Door(stratum: Skyways, side: N, at: 7), Door(stratum: Undercity, side: W, at: 3)]),
            zones: ["public", "guest", "staff", "secure"],
            borders: [
                Border(zones: ("public", "guest"), lock: Card, ways: [Card, Net, Talk, Force]),
                Border(zones: ("public", "staff"), lock: Card, always: true, ways: [Card, Net, Talk, Force]),
                Border(zones: ("guest", "staff"), lock: Card, always: true, ways: [Card, Net, Talk, Force]),
                Border(zones: ("staff", "secure"), lock: Keypad, always: true, one_way: true, ways: [Code, Keyjack, Net, Force]),
                Border(zones: ("guest", "secure"), lock: Keypad, always: true, ways: [Code, Keyjack, Net, Force]),
            ],
            levels: Levels(
                below: [
                    Level(name: "car park", bands: [Band(zone: "public", share: 15, rooms: ["lift lobby"]),
                                                    Band(zone: "staff", share: 55, rooms: ["car park"]),
                                                    Band(zone: "secure", share: 30, rooms: ["security office"])]),
                    Level(name: "service", bands: [Band(zone: "staff", share: 100, rooms: ["kitchen", "laundry", "lockers"])]),
                ],
                ground: Level(name: "lobby", bands: [Band(zone: "public", share: 60, rooms: ["reception", "bar"]),
                                                     Band(zone: "staff", share: 40, rooms: ["back office"])]),
                middle: Some(Level(name: "guest floor", spine: Any, bands: [
                    Band(zone: "public", share: 15, rooms: ["lift lobby"]),
                    Band(zone: "guest", share: 65, rooms: [], fill: Some("guest room")),
                    Band(zone: "staff", share: 20, rooms: ["linen room"])])),
                top: [Level(name: "penthouse", bands: [Band(zone: "guest", share: 30, rooms: ["foyer"]),
                                                      Band(zone: "secure", share: 70, rooms: ["lounge", "office"])])],
            ),
            rooms: [
                Room(name: "reception", kind: Lobby, size: ((8, 6), (14, 10)), furniture: [Counter], stations: [Reception]),
                Room(name: "guest room", kind: HotelRoom, size: ((4, 4), (6, 6)), private: true),
                // ...
            ],
            links: [
                Link(name: "guest lift", kind: Lift, zone: "public", serves: Roles(["car park", "lobby", "guest floor", "penthouse"]),
                     cards: [("penthouse", "secure")]),
                Link(name: "service lift", kind: Lift, zone: "staff", serves: Roles(["car park", "service", "lobby", "guest floor"])),
                Link(name: "service stair", kind: Stair, zone: "staff", serves: All),
                Link(name: "grand stair", kind: Stair, zone: "public", serves: Levels([0, 1])),
                Link(name: "riser", kind: Crawl, zone: "staff", serves: All),
            ],
            crawls_in: [Crawl(stratum: Heights, zone: "staff"), Crawl(stratum: Undercity, zone: "staff")],
            objectives: [Objective(id: "safe", name: "the penthouse safe", room: "office", level: Some("penthouse"))],
            security: [Security(size: Small, guards: [("staff", 1), ("secure", 2)], cameras: (2, 4), turrets: (0, 0), containers: (1, 2)), /* Medium, Large */],
        ),
        // The office and the metro station, cut the same way from the spec's lists:
        // office: builds [Office]; zones public, staff, secure, executive; below [server basement];
        //   ground lobby with a "gate line" piece; middle open-plan floor; top [executive floor].
        // metro station: builds []; zones public, paid, staff, secure; ground hall;
        //   below [concourse, platform, platform, service, drainage], a Track piece on each platform
        //   level (Task 15's copy test needs one before Task 16 writes the full type); no middle, no top.
    ],
)
```

The first cut only has to pass `check`; Task 16 writes each type's full program. Give the new `RoomKind`s their labels, `walled` and `room_light` entries (`Platform`/`Track`/`CarPark` lit as `Hall`, `Plant`/`Server` fluorescent like `Storage`, `Kitchen` like `Laundry`).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p citygen --target-dir target/smoke venue::types`
Expected: PASS.
Run: `cargo test --workspace --target-dir target/smoke && cargo check --workspace --all-targets --features strata-zero/window --target-dir target/smoke`
Expected: PASS.
Run: `cargo test -p strata-zero --features window --target-dir target/smoke every_drawn_character`
Expected: PASS (`every_drawn_character_has_a_glyph` scans `flavor/sites.ron`: keep it ASCII; the test lives in `window/font.rs` and runs only with the window feature, so this command is the one that checks it).

- [ ] **Step 5: Commit**

```bash
git add crates/citygen flavor/sites.ron   # and crates/strata-zero flavor/looks.ron if a Look line was added
git commit -F - <<'EOF'
citygen: venue types are data, in flavor/sites.ron

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 5: The frame and the levels by role

**Files:**
- Create: `crates/citygen/src/interior/venue/frame.rs`, `venue/layout.rs`, `venue/levels.rs`
- Modify: `crates/citygen/src/interior/venue/testutil.rs` (Task 4's: adds `city()`, `medium(seed)`, `cities()`, with the bodies of `interior/mod.rs`'s test helpers)
- Modify: `crates/citygen/src/interior/plan.rs` (share today's storey count as `pub(crate) fn storeys(city, id) -> (i32, i32)`), `interior/layouts.rs` (`exit_tile`, `outward`, `edge` reachable from `venue` as `pub(in crate::interior)`)

**Interfaces:**
- Consumes: Task 4's types; `FloorPlan::classic`'s storey rng (same seed, same ranges).
- Produces:
  ```rust
  #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
  pub struct VenueFrame {
      pub building: Option<BuildingId>,
      pub kind: BuildingKind,
      pub theme: Theme,
      pub footprint: Rect,
      /// The footprint x 3, at least 12 x 9: every level at or above -1.
      pub size: Size,
      /// The building's lowest and top level: lowest is -1 when it reaches the Undercity.
      pub storeys: (i32, i32),
      pub doors: Vec<FrameDoor>,
      pub crawls: Vec<FrameCrawl>,
      /// The sides with windows: those facing open ground on the street.
      pub faces: Vec<Dir>,
  }
  pub struct FrameDoor { pub stratum: StratumKind, pub level: i32,
      /// The city-map door, the city tile you step out onto, and the floor tile (None: the
      /// roof door, which the layout stands beside a stair).
      pub door: Pos, pub landing: Pos, pub at: Option<Pos> }
  pub struct FrameCrawl { pub stratum: StratumKind, pub level: i32, pub outside: Pos, pub at: Pos }
  impl VenueFrame {
      pub fn of_building(city: &City, b: BuildingId) -> Option<VenueFrame>;
      pub fn own(t: &VenueType) -> VenueFrame;
      /// The frame with its top raised to `top` (the roof door moves up with it).
      pub fn raised(self, top: i32) -> VenueFrame;
  }
  /// A building's layout seed: the city seed and the building, as a floor's is.
  pub fn building_seed(city_seed: u64, b: BuildingId) -> u64;

  #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
  pub struct VenueLayout {
      pub type_id: String,
      pub type_name: String,
      pub seed: u64,
      pub frame: VenueFrame,
      /// The type's zone names by rank.
      pub zones: Vec<String>,
      /// Lowest first.
      pub levels: Vec<LevelLayout>,
      pub links: Vec<LevelLink>,      // Task 8
      pub ways_in: Vec<WayIn>,        // Task 9
      /// City doors on levels this layout doesn't have.
      pub barred: Vec<(StratumKind, Pos)>, // Task 9
  }
  #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
  pub struct LevelLayout {
      pub level: i32,
      /// The type's role name: "guest floor".
      pub role: String,
      pub size: Size,
      pub spine: Spine,                 // Task 6: never Any once laid out
      pub bands: Vec<BandArea>,         // Task 6
      pub rooms: Vec<PlannedRoom>,      // Tasks 6-7
      pub doors: Vec<PlannedDoor>,      // Tasks 6-7
      pub pieces: Vec<PlacedPiece>,     // Task 7
      pub exits: Vec<Exit>,             // Task 9
      /// Tiles that take no wall, furniture or spot: doorsteps, landings and beside them.
      pub reserved: Vec<Pos>,
  }
  pub fn lay_out(t: &VenueType, frame: VenueFrame, seed: u64) -> Result<VenueLayout, String>;
  impl VenueLayout { pub fn level(&self, level: i32) -> Option<&LevelLayout>; }
  ```
  Later tasks add their stages inside `lay_out` in order; each stage is a `pub(super) fn` in its file taking `(&VenueType, &mut VenueLayout, &mut ChaCha8Rng)`, the rng seeded `hash64(seed, STAGE)` with a constant per stage.

- [ ] **Step 1: Write the failing tests**

In `venue/frame.rs` (cities from `venue::testutil`: `cities()` is `city()`, `medium(8)` and `medium(11)`):

```rust
#[test]
fn a_buildings_frame_is_its_footprint_floors_and_doors() {
    for c in cities() {
        for b in c.buildings.iter().filter(|b| matches!(b.kind, BuildingKind::Hotel | BuildingKind::Office)) {
            let f = VenueFrame::of_building(&c, b.id).unwrap();
            let classic = FloorPlan::classic(&c, b.id).unwrap();
            assert_eq!(f.size, classic.size);
            assert_eq!(f.storeys, (classic.lowest(), classic.top()));
            for (k, door) in c.portals(b.id) {
                let d = f.doors.iter().find(|d| d.door == door && d.stratum == k).unwrap();
                assert_eq!(Some(d.level), classic.door_level(k));
                if k != StratumKind::Heights { assert!(d.at.is_some()); }
            }
            assert_eq!(f.doors.len(), c.portals(b.id).len());
        }
    }
}

#[test]
fn the_own_frame_is_the_types_frame() {
    for t in &VenuePack::builtin().types {
        let f = VenueFrame::own(t);
        assert_eq!((f.size.w as i32, f.size.h as i32), ((t.frame.footprint.0 * 3).max(12), (t.frame.footprint.1 * 3).max(9)));
        assert_eq!(f.storeys, (if t.frame.undercity { -1 } else { 0 }, t.frame.top));
        assert_eq!(f.doors.len(), t.frame.doors.len() + usize::from(t.frame.roof));
        assert!(f.building.is_none());
    }
}
```

In `venue/levels.rs` (`testutil::test_type()`, the test type of Task 4):

```rust
#[test]
fn roles_fill_the_floors() {
    let t = test_type();
    let own = VenueFrame::own(&t);
    let l = lay_out(&t, own.clone(), 1).unwrap();
    let levels: Vec<i32> = l.levels.iter().map(|x| x.level).collect();
    let below = t.levels.below.len() as i32;
    assert_eq!(levels, (-below..=own.storeys.1).collect::<Vec<_>>());
    assert_eq!(l.level(0).unwrap().role, t.levels.ground.name);
    assert_eq!(l.level(-1).unwrap().role, t.levels.below[0].name);
    assert_eq!(l.level(own.storeys.1).unwrap().role, t.levels.top.last().unwrap().name);
    assert_eq!(l.level(1).unwrap().role, t.levels.middle.as_ref().unwrap().name);
    // Too short for the middle: the top role keeps the top floor; at G only, no level above.
    let short = lay_out(&t, VenueFrame { storeys: (-1, 1), ..own.clone() }, 1).unwrap();
    assert_eq!(short.level(1).unwrap().role, t.levels.top.last().unwrap().name);
    let flat = lay_out(&t, VenueFrame { storeys: (-1, 0), ..own }, 1).unwrap();
    assert_eq!(flat.levels.last().unwrap().level, 0);
}

#[test]
fn only_levels_below_minus_one_grow() {
    let t = test_type();
    let l = lay_out(&t, VenueFrame::own(&t), 1).unwrap();
    for x in &l.levels {
        let def = /* the LevelDef of x.role */;
        let want = if x.level < -1 {
            Size { w: l.frame.size.w * def.scale.0 as u32 / 100, h: l.frame.size.h * def.scale.1 as u32 / 100 }
        } else { l.frame.size };
        assert_eq!(x.size, want, "level {}", x.level);
    }
    assert!(l.level(-2).unwrap().size.w > l.frame.size.w);
}

#[test]
fn the_same_seed_lays_the_same_levels() {
    let t = test_type();
    assert_eq!(lay_out(&t, VenueFrame::own(&t), 9), lay_out(&t, VenueFrame::own(&t), 9));
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p citygen --target-dir target/smoke venue::frame venue::levels`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the frame and the levels**

- `of_building`: `None` for an open building. `size` and `storeys` as `classic`; a door's level from today's rule (Street 0, Skyways the bridge level `3.min(top).max(1)`, Heights the top, Undercity −1); `at` is `exit_tile` for every stratum but Heights; `faces` are the sides `layouts::Builder::windows` would glaze; `crawls`: on Heights, the footprint tile nearest the footprint's centre that is walkable on the Heights map and isn't the roof door, with `at` its floor tile on the top level; on the Undercity (when the span reaches it), the walkable tunnel tile beside the footprint farthest from the Undercity door, with `at` its border tile's inner neighbour on −1.
- `own`: the footprint at `(0, 0)`; doors along their side (`at` tiles from the side's start), the roof door and the crawls placed as `of_building` would on an open roof and a tunnel along the west side; `faces` every side.
- `raised`: moves `storeys.1` and the Heights door's level up to `top`.
- `levels::place`: below roles at −1, −2, …; ground at 0; the top roles take the highest floors above 0 (all of them when there's room, else the topmost ones); the middle role fills what lies between. Sizes as the test. `spine` is the role's (Task 6 resolves `Any`).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p citygen --target-dir target/smoke venue:: && cargo test -p citygen --target-dir target/smoke interior::`
Expected: PASS (this task edits `classic`, so `classic_floors_are_pinned` runs and stays unchanged).
Run: `cargo test --workspace --target-dir target/smoke`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/citygen
git commit -F - <<'EOF'
citygen: a venue's frame and its levels by role

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 6: Zones, the spine and the locks between them

**Files:**
- Create: `crates/citygen/src/interior/venue/zones.rs`
- Modify: `crates/citygen/src/interior/floor.rs` (`DoorLock`), `venue/layout.rs` (`BandArea`, `PlannedDoor`, `PlannedRoom` first fields, `LOCK_NUMBERS`)

**Interfaces:**
- Consumes: Tasks 4–5.
- Produces:
  ```rust
  // citygen::interior::floor
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
  pub struct DoorLock {
      pub at: Pos,
      /// Keys its code with (building, level), as a flat's number does.
      pub number: u16,
      /// The card that opens it: this zone's, or a deeper one's.
      pub zone: u8,
      pub kind: LockKind,
      pub ways: Ways,
      /// The tile it opens from without a key: a one-way lock's inner side.
      pub free_from: Option<Pos>,
  }
  // citygen::interior::venue
  pub const LOCK_NUMBERS: u16 = 60_000;
  pub struct BandArea { pub zone: u8, pub rect: Rect }
  pub struct PlannedDoor {
      pub at: Pos,
      /// The two rooms it joins, by index in the level's rooms.
      pub rooms: (usize, usize),
      /// Locked in the plain building: the type's `always` borders.
      pub lock: Option<DoorLock>,
      /// The lock a mission can fit here (a candidate door), None for a door it can't.
      pub fit: Option<DoorLock>,
      /// Joins two zones.
      pub border: bool,
  }
  pub struct PlannedRoom {
      pub rect: Rect, pub kind: RoomKind, pub name: String, pub zone: u8,
      pub number: u16, pub private: bool,
      pub furniture: Vec<Furniture>, pub stations: Vec<Role>, // from the room's def (Task 7)
      /// Interior tiles the mission layer may use (Task 7).
      pub spots: Vec<Pos>,
  }
  ```

- [ ] **Step 1: Write the failing tests**

In `venue/zones.rs` (`testutil::test_type()`, own frame, seeds `0..20`):

```rust
#[test]
fn bands_run_front_to_back_in_data_order() {
    // For each level: bands.len() == the role's bands; band i's zone == t.zone(def.bands[i].zone);
    // band 0 touches the front side (the side of the frame's first Street door);
    // each cut between bands lies within 10 percentage points of its running share (the shares so far summed), the jitter the step allows each cut; bands tile the level's inner rect.
}

#[test]
fn border_doors_join_neighbouring_bands_on_the_spine() {
    // For every pair of neighbouring bands with different zones: at least `doors` of the inner
    // band's BandDef are PlannedDoors with border == true; the first joins the two bands' spine rooms
    // (a `doors: 2` loop door's rooms are set by Task 7, once the rooms exist);
    // a border def with `always` puts a lock on each (kind and ways from the def, zone = the inner
    // rank); `fit` equals that lock (or the lock it would carry, for a border not always locked).
}

#[test]
fn a_one_way_border_opens_free_from_its_inner_side() {
    // The test type's one_way border: free_from == Some(the door's neighbour inside the inner
    // band); every other lock's free_from is None.
}

#[test]
fn lock_numbers_are_their_own() {
    // Per level, every lock number is unique and >= LOCK_NUMBERS.
}

#[test]
fn middle_levels_vary_their_spine() {
    // A raised own frame with eight middle levels: across seeds 0..10, the middle levels' spines
    // take at least two shapes, and no seed gives all eight the same shape and band cuts.
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p citygen --target-dir target/smoke venue::zones`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the zones stage**

- **Front:** the side of the frame's first Street door, on every level, so zones stack the same way up the building: public at the doors, the deepest band at the back.
- **Spine:** a level's `Any` takes, by the level's rng, `Straight` (a 2-wide corridor from front to back along the middle), `Bent` (straight, then turning along the back band), `Ring` (round a central core, only when the inner rect is at least 24 x 16) or `Hall` (no corridor: each band is one open room). The spine becomes one `Corridor` room per band (its zone the band's).
- **Bands:** cut across the spine's run, front to back, by share, each cut moved by up to ±10 points of the level's depth by the rng (integer percent), never thinner than 5 tiles.
- **Border doors:** where the spine crosses from a band to the next, a door across the spine's width at the cut; `doors: 2` adds a second border door along the cut away from the spine (a loop; its `rooms` stay the first door's until Task 7 re-points them); on a `Hall` spine the door sits on the cut at the spine line. Each border door gets `fit` from `t.border(outer, inner)` (kind, ways, zone = inner rank, `free_from` the inner neighbour when `one_way`), and `lock = fit` when the border is `always`. `fit` is set on border doors only, never on a room's own door (Choice 5). Lock numbers count from `LOCK_NUMBERS` per level in door order.
- Doorsteps of the frame's doors and their 4-neighbours go into `reserved`; no cut or wall crosses them.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p citygen --target-dir target/smoke venue:: && cargo test --workspace --target-dir target/smoke`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/citygen
git commit -F - <<'EOF'
citygen: a venue's zones, its spine and the locks on their borders

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 7: Rooms and set pieces

**Files:**
- Create: `crates/citygen/src/interior/venue/rooms.rs`
- Modify: `venue/layout.rs` (`PlacedPiece`, `room_number`, `room_label`)

**Interfaces:**
- Consumes: Task 6's bands, spine rooms and border doors.
- Produces:
  ```rust
  pub struct PlacedPiece { pub name: String, pub kind: PieceKind, pub rect: Rect,
      /// A track's two end tiles on the frame wall: its shutters.
      pub ends: Vec<Pos> }
  /// A room's number: level x 100 + n at or above the street; 9000 + 100 x depth + n below it.
  pub fn room_number(level: i32, n: u16) -> u16;
  /// "1204", or "B1-04" below the street.
  pub fn room_label(number: u16) -> String;
  ```

- [ ] **Step 1: Write the failing tests**

The tests take the test type from `venue::testutil`.

```rust
#[test]
fn basement_rooms_number_apart_from_the_lobby() {
    assert_eq!(room_number(0, 4), 4);
    assert_eq!(room_number(12, 3), 1203);
    assert_eq!(room_number(-1, 4), 9104);
    assert_eq!(room_number(-5, 12), 9512);
    assert_eq!(room_label(1203), "1203");
    assert_eq!(room_label(9104), "B1-04");
}

#[test]
fn rooms_fill_their_band_and_keep_its_zone() {
    // Test type, own frame, seeds 0..20, every level: each room's rect lies inside its band's
    // rect (or its piece's) and takes the band's zone; no two rooms overlap; each room but a Hall
    // band's open room has a PlannedDoor to its band's spine room; a listed room's size is within
    // its RoomDef's range; a fill room is at least 3 x 3;
    // every border door, a `doors: 2` loop door included, joins a room of each of its two bands
    // (Task 6 left the loop door's rooms to this task).
}

#[test]
fn listed_rooms_come_before_the_fill() {
    // Every room a band lists appears on the level; fill rooms only after them.
}

#[test]
fn doorsteps_landings_and_spots_stay_clear() {
    // No room wall on a reserved tile; every spot lies in its room's rect inset by 1, is not
    // reserved and is not beside a door; a room 3 x 3 or larger has at least one spot.
}

#[test]
fn a_track_runs_the_far_side_and_ends_in_shutters() {
    // The Track piece on the test type's deep level: a Platform room 3 deep in front of a Track
    // room 2 deep, both spanning the level's inner width at the side away from the front;
    // ends == the two frame-wall tiles at the track's ends.
}

#[test]
fn a_gate_line_turns_its_border_into_turnstiles() {
    // On the test type's ground: 3..=6 border doors in one line across the first cut, each with
    // the border's lock (`fit`), and the spine's single border door gone.
}

#[test]
fn a_ramp_is_two_by_four_against_a_wall() { /* rect 2 x 4 or 4 x 2, touching the frame wall */ }

#[test]
fn middle_floors_differ() {
    // A raised own frame with six middle levels, seeds 0..10: no two neighbouring middle levels
    // have the same room rects, and at least three different room sets among the six.
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p citygen --target-dir target/smoke venue::rooms`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the rooms stage**

- **Pieces first,** each in its level's band: `Track` along the far side (a `Platform` room 3 deep, then a `Track` room 2 deep against the wall; its two wall-end tiles become `ends`); `Ramp` a 2 x 4 strip against the frame wall, in a `CarPark` room; `GateLine` replaces the spine's border door at the level's first cut by a line of 3–6 doors across the cut (as many as fit, one tile apart), each with the border's lock and its own lock number.
- **Rooms:** each band's room list in order, then its `fill` room until the band is used, packed along both sides of the spine with each room's door on the spine wall (the middle tile whose inside is free, as `corridor` does today); sizes drawn by the level's rng within the def's range, so two levels of one role differ; a span with a doorstep in it stays open to the spine. A `Hall` band is one room of its first listed def.
- **Numbers:** rooms with a private def or kind `HotelRoom`/`Flat`/`Office` get `room_number(level, n)`, n counting from 1 per level; the rest 0.
- **Spots:** up to four per room from its rect inset by 1 (the corners and the centre), never reserved or beside a door; drawing keeps them free (Task 10).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p citygen --target-dir target/smoke venue:: && cargo test --workspace --target-dir target/smoke`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/citygen
git commit -F - <<'EOF'
citygen: a venue's rooms and its set pieces

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 8: Links in the layout

**Files:**
- Create: `crates/citygen/src/interior/venue/links.rs`

**Interfaces:**
- Consumes: `LevelLink`, `LinkStop`, `LinkKind` (Task 3); `LinkDef`, `Serves`, `LinkDir`, `Alight` (Task 4); the rooms (Task 7).
- Produces: `VenueLayout.links: Vec<LevelLink>` in the type's link order, each named from its def, with landings and their 4-neighbours added to their level's `reserved`, and any mission spot those tiles cover removed from its room's `spots` (spots are never reserved, and no post lands on a landing).

- [ ] **Step 1: Write the failing tests**

The tests take the test type from `venue::testutil`.

```rust
#[test]
fn every_link_serves_its_levels_from_rooms_of_its_zone() {
    // Test type, own frame, seeds 0..20: for each LinkDef, the stops' levels are the layout's
    // levels its Serves selects, lowest first; every tile of every stop lies inside a room of the
    // stop's zone (the stop's card zone for a carded lift stop, else the def's zone; on a level
    // without that zone, the next deeper zone the level has, else its deepest), on no door,
    // reserved doorstep, spot or piece tile.
}

#[test]
fn lifts_stairs_and_crawls_stack_where_they_can() {
    // For Lift, Stair and Crawl links, at least half of each link's neighbouring stop pairs share `at`.
}

#[test]
fn an_escalator_lands_a_few_tiles_on() {
    // The test type's Up escalator: its two stops' positions are 3..=5 apart (Manhattan);
    // the lower stop on && !off, the upper !on && off.
}

#[test]
fn a_fire_stair_lets_you_off_only_at_its_ends() {
    // alight: Ends: off only on the first and last stop, on everywhere.
}

#[test]
fn a_lift_asks_for_its_card_where_the_type_says() {
    // The test type's carded lift: card == the zone's rank on its carded role's stop, 0 elsewhere.
}

#[test]
fn a_roof_level_keeps_a_stair_with_room_beside_it() {
    // When the frame has a Heights door: a Stair stop on that level whose east neighbour is free floor in a room.
}

#[test]
fn landings_leave_no_spot_reserved() {
    // After the links stage, on every level no room's spots hold a reserved tile
    // (Task 7's doorsteps_landings_and_spots_stay_clear holds on the finished layout too).
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p citygen --target-dir target/smoke venue::links`
Expected: FAIL (no links laid).

- [ ] **Step 3: Implement the links stage**

Per def, per level it serves, lowest first: a landing on free floor of a room of the stop's zone as the test defines it (a lift needs two tiles side by side), so a staff stair comes up into the penthouse's secure rooms and a carded lift stop opens into its card's zone. Lifts, stairs, ladders and crawls try the tile their previous stop used first, then the free tile nearest it; an escalator's stop goes 3–5 tiles on from the stop below along the spine. The first Stair serving a Heights-door level keeps its east neighbour free there. `on`/`off` from `dir` and `alight`; `card` from `cards`. A level that has no room of the zone left with space fails `lay_out` with `"no room for the {link} on {level}"` (the checker reports it).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p citygen --target-dir target/smoke venue:: && cargo test --workspace --target-dir target/smoke`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/citygen
git commit -F - <<'EOF'
citygen: a venue's links between its levels

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 9: Ways in

**Files:**
- Create: `crates/citygen/src/interior/venue/ways.rs`
- Modify: `venue/layout.rs` (`WayIn`, `WayInKind`)

**Interfaces:**
- Consumes: the frame's doors and crawls (Task 5), the rooms (Task 7), the links (Task 8).
- Produces:
  ```rust
  pub struct WayIn { pub level: i32, pub room: usize,
      /// The floor tile: an exit's door, or a crawl's grille.
      pub at: Pos, pub kind: WayInKind }
  pub enum WayInKind { Door { stratum: StratumKind, door: Pos }, Crawl { stratum: StratumKind, outside: Pos } }
  ```
  and `LevelLayout.exits` (`citygen::Exit`), `VenueLayout.barred`.

- [ ] **Step 1: Write the failing tests**

The tests take the test type and the cities from `venue::testutil`.

```rust
#[test]
fn every_frame_door_is_an_exit_or_barred() {
    // Test type over its own frame and over every Hotel/Office of cities 6, 8, 11: each FrameDoor is
    // either an Exit on its level (at, stratum, landing, door) with a WayIn::Door, or its
    // (stratum, door) is in `barred` because the layout lacks its level; never both.
}

#[test]
fn the_roof_door_stands_east_of_a_stair() {
    // The Heights exit's `at` is the east neighbour of a Stair stop on that level.
}

#[test]
fn crawls_in_land_in_their_zone_on_a_stratum_level() {
    // For each CrawlInDef whose stratum the frame has a crawl for: a WayIn::Crawl on that crawl's
    // level, `at` in a room of its zone, `outside` the frame crawl's tile; none for a stratum the frame lacks.
}

#[test]
fn a_type_with_no_upper_roles_bars_the_doors_above() {
    // Every builtin type with no middle and no top role, laid on the tallest Office of medium(8):
    // its levels end at 0, and the building's Skyways and Heights doors are all in `barred`.
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p citygen --target-dir target/smoke venue::ways`
Expected: FAIL.

- [ ] **Step 3: Implement the ways-in stage**

An exit per frame door on a level the layout has (the roof door's `at` east of the Stair stop), its `WayIn` room the room holding its doorstep; the rest `barred`. A crawl way in per `CrawlInDef` whose stratum the frame has a crawl for, its grille on the free tile of a room of its zone nearest the frame crawl's `at`. `lay_out` now runs all six stages.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p citygen --target-dir target/smoke venue:: && cargo test --workspace --target-dir target/smoke`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/citygen
git commit -F - <<'EOF'
citygen: a venue's ways in, its crawls and its barred doors

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 10: A venue's level drawn as a floor

**Files:**
- Create: `crates/citygen/src/interior/venue/draw.rs`
- Modify: `crates/citygen/src/interior/floor.rs` (`Floor.locks`, `Room.name`, `Room.zone`, `Room::new`, `Floor::lock_at`), `interior/plan.rs` (`sizes`, `size_of`, `crawls`, `venue`, `of_layout`), `interior/layouts.rs` + `layouts/*.rs` (`Builder.city: Option<&City>`, `Builder.size` per level, helpers `pub(in crate::interior)`; `Room` literals through `Room::new`), `interior/mod.rs` (`generate_floor` dispatch, `validate_floor` uses `size_of`), `tile.rs` (`Terrain::Track` at the end, walkable, sees through; `Feature::Shutter` at the end), `crates/strata-zero/src/render/tileset.rs` (`╪` track in `Rgb(140, 125, 95)`, `▓` shutter in `Rgb(120, 120, 128)`), `crates/strata-zero/src/look.rs` + `flavor/looks.ron` (keys `track`, `shutter`)

**Interfaces:**
- Consumes: Tasks 3–9.
- Produces:
  ```rust
  // citygen::interior::floor
  pub struct Floor { /* ... */ #[serde(default)] pub locks: Vec<DoorLock> }
  impl Floor { pub fn lock_at(&self, p: Pos) -> Option<&DoorLock>; }
  pub struct Room { /* ... */ #[serde(default)] pub name: Option<String>, #[serde(default)] pub zone: u8 }
  impl Room { pub fn new(rect: Rect, kind: RoomKind, door: Option<Pos>, private: bool, number: u16) -> Room; }
  // citygen::interior::plan
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
  pub struct CrawlWay { pub stratum: StratumKind, pub outside: Pos, pub level: i32, pub at: Pos }
  pub struct FloorPlan { /* ... */
      /// Levels drawn at another size than `size`: a venue's deep levels.
      #[serde(default)] pub sizes: Vec<(i32, Size)>,
      #[serde(default)] pub crawls: Vec<CrawlWay>,
      /// The layout a venue's floors are drawn from; None: today's builder.
      #[serde(default)] pub venue: Option<Box<VenueLayout>>,
  }
  impl FloorPlan {
      pub fn size_of(&self, level: i32) -> Size;
      pub fn of_layout(layout: VenueLayout) -> FloorPlan;
  }
  // citygen::interior::venue
  pub fn draw_level(layout: &VenueLayout, level: i32) -> Option<Floor>;
  ```
  `generate_floor(city, plan, level)` draws `plan.venue`'s level when there is one, else today's builder.

- [ ] **Step 1: Write the failing tests**

In `venue/draw.rs` (`testutil::test_type()`, own frame, seeds `0..10`):

```rust
#[test]
fn every_level_draws_and_validates() {
    let t = test_type();
    for seed in 0..10 {
        let layout = lay_out(&t, VenueFrame::own(&t), seed).unwrap();
        let plan = FloorPlan::of_layout(layout.clone());
        for l in &layout.levels {
            let f = draw_level(&layout, l.level).unwrap();
            validate_floor(&plan, &f).unwrap_or_else(|e| panic!("seed {seed} level {}: {e}", l.level));
            assert_eq!(f.tiles.size(), l.size);
            let planned: Vec<_> = l.rooms.iter().map(|r| (r.rect, r.kind, Some(r.name.clone()), r.zone, r.number)).collect();
            let drawn: Vec<_> = f.rooms.iter().map(|r| (r.rect, r.kind, r.name.clone(), r.zone, r.number)).collect();
            assert_eq!(drawn, planned);
            assert_eq!(f.locks, l.doors.iter().filter_map(|d| d.lock).collect::<Vec<_>>());
            assert_eq!(f.exits, l.exits);
        }
    }
}

#[test]
fn spots_stay_free_and_landings_carry_their_kind() {
    // Every spot is Terrain::Floor with no feature; every link stop's tiles carry kind.terrain();
    // a Crawl stop and a crawl way in carry Feature::Vent.
}

#[test]
fn the_track_is_walked_and_ends_in_shutters() {
    // Track room tiles are Terrain::Track and is_walkable(); each piece end is Wall + Feature::Shutter
    // (not walkable); every Track tile is in its Platform room's zone.
}

#[test]
fn the_plan_carries_the_layout() {
    let layout = /* test type, own frame, seed 3 */;
    let plan = FloorPlan::of_layout(layout.clone());
    assert_eq!(plan.levels, layout.levels.iter().map(|l| l.level).collect::<Vec<_>>());
    assert_eq!(plan.links, layout.links);
    assert_eq!(plan.size_of(-2), layout.level(-2).unwrap().size);
    assert_eq!(plan.door_level(StratumKind::Street), Some(0));
    assert_eq!(plan.crawls.len(), layout.ways_in.iter().filter(|w| matches!(w.kind, WayInKind::Crawl { .. })).count());
    let back: FloorPlan = rmp_serde::from_slice(&rmp_serde::to_vec_named(&plan).unwrap()).unwrap();
    assert_eq!(back, plan);
    assert_eq!(generate_floor(&city(), &plan, 0), draw_level(&layout, 0).unwrap());
}

#[test]
fn drawing_is_the_same_twice() { /* draw_level(&l, x) == draw_level(&l, x) for every level */ }
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p citygen --target-dir target/smoke venue::draw`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the drawing**

- `Builder` takes `city: Option<&'a City>` and `city_seed: u64` (lights read the seed, not the city), and a `size` for its level (every `self.plan.size` reads `self.size`); `windows` glazes a list of sides; today's builder passes the city and its sides as before, so the pin holds.
- `draw_level`: a `Builder` on `FloorPlan::of_layout`'s plan without a city, its rng seeded from the layout seed and the level; frame wall and windows on `frame.faces` (levels below the street get none); each planned room's walls (walled kinds, as `walled`), doors (`Terrain::Door`; a lock door is a door), pieces (`Track` tiles, shutters on the ends, `CarPark` ramp rails as `Feature::Barrier` along the ramp's long sides), landings (`kind.terrain()`, both lift tiles; `Feature::Vent` on crawl grilles and crawl ways in), exits; then each room furnished by its def's furniture along its walls and a station beside each piece of it for each listed role, or by `furnish_room` by kind when the def lists none, never on a spot or reserved tile; then `light` and `finish`. No `connect` pass: the layout is connected by construction and the checker proves it.
- `Floor.locks` are the planned doors' `lock`s; rooms carry `name` and `zone`.
- `FloorPlanRead` takes `sizes`, `crawls` and `venue` too, each `#[serde(default)]`. Every layout type (`VenueLayout` and all it holds) derives `Clone, Debug, PartialEq, Eq, Serialize, Deserialize`, as `FloorPlan` needs.
- `of_layout`: `building` (the frame's, else `BuildingId(0)`), `kind` and `theme` from the frame, `size` the frame's, `levels`, `doors` (a `LevelDoor` per frame door, levels the layout lacks included, so their doors read as barred), `links`, `sizes` for levels whose size differs, `crawls` from the crawl ways in, `atrium`/`home_level`/`clinic` None, `venue` the layout. `work_floors` of a venue plan are the levels with a room holding a `Desk`, `Reception`, `Bar`, `Table`, `Machine` or `Shelf` station; `corridor_floors` the levels with a `HotelRoom` or `Flat`; `floor_type` of a venue level is never asked (today's builder only).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p citygen --target-dir target/smoke interior::`
Expected: PASS, `classic_floors_are_pinned` unchanged.
Run: `cargo test --workspace --target-dir target/smoke && cargo check --workspace --all-targets --features strata-zero/window --target-dir target/smoke`
Expected: PASS.
Run: `cargo test -p strata-zero --features window --target-dir target/smoke every_drawn_character`
Expected: PASS (`╪` and `▓` are new).

- [ ] **Step 5: Commit**

```bash
git add crates/citygen crates/strata-zero flavor/looks.ron
git commit -F - <<'EOF'
citygen: a venue's level drawn as a floor, its plan carrying its layout

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 11: The checker

**Files:**
- Create: `crates/citygen/src/interior/venue/graph.rs`, `venue/check.rs`, `crates/citygen/tests/venues.rs`

**Interfaces:**
- Consumes: Tasks 4–10.
- Produces:
  ```rust
  pub enum NodeAt { Room { level: i32, room: usize }, Outside(StratumKind) }
  pub struct GraphLock { pub zone: u8, pub ways: Ways,
      /// The code that opens it: (level, number).
      pub code: Option<(i32, u16)>,
      /// This direction passes without a key: a one-way lock's free side.
      pub free: bool }
  pub struct GraphEdge { pub from: usize, pub to: usize, pub lock: Option<GraphLock> } // directed
  #[derive(Clone, Debug, Default, PartialEq, Eq)]
  pub struct Keys { pub card: u8, pub talked: u8, pub codes: BTreeSet<(i32, u16)> }
  pub struct Reach { pub nodes: Vec<bool>, pub keys: Keys }
  pub struct VenueGraph {
      pub nodes: Vec<NodeAt>, pub edges: Vec<GraphEdge>,
      /// The open ways in; `Outside` nodes are where routes in start and routes out end.
      pub starts: Vec<usize>,
      /// What lies in a room: a card of a zone, a code, a person of a zone.
      pub cards: Vec<(usize, u8)>, pub codes: Vec<(usize, (i32, u16))>, pub people: Vec<(usize, u8)>,
      pub goal: Option<usize>,
  }
  /// The four ways of playing, each a set of means on top of open doors and the cards
  /// and codes found on the way.
  pub const PLAYS: [(&str, Ways); 4]; // ("force", FORCE), ("the Net", NET), ("the Keyjack", KEYJACK | CRAWL), ("talk", TALK)
  // `BitOr` isn't const: write the bits (`Ways(KEYJACK.0 | CRAWL.0)`), or make PLAYS a `pub fn plays() -> [(&'static str, Ways); 4]`.
  impl VenueGraph {
      /// Every level's rooms, doors (a private room's door is a Keypad lock coded by its
      /// number, ways Code | Keyjack | Net | Force), links (a lift edge carries a Card | Net | Talk
      /// lock at the card of either stop; a crawl edge a Grille lock, ways CRAWL), exits and crawl
      /// ways in (a Grille lock). `fitted` adds the `fit` lock of those doors; `kept` keeps only those
      /// levels, their links' stops and their ways in.
      pub fn of(layout: &VenueLayout, fitted: &[(i32, usize)], kept: Option<&[i32]>) -> VenueGraph;
      pub fn room(&self, level: i32, room: usize) -> Option<usize>;
      pub fn reach(&self, from: &[usize], means: Ways, keys: Keys) -> Reach;
      /// A route from a way in to the goal: the keys it ends with.
      pub fn route_in(&self, means: Ways) -> Option<Keys>;
      /// A route from the goal out into the city, starting with `keys`.
      pub fn route_out(&self, means: Ways, keys: Keys) -> bool;
      /// The fewest locks on any route from a way in to `goal`, any lock passable.
      pub fn depth(&self, goal: usize) -> Option<u32>;
  }
  impl VenueLayout {
      /// Doors a mission can fit a lock to: `fit` set, `lock` not.
      pub fn candidates(&self) -> Vec<(i32, usize)>;
      /// Whether this layout can hold a mission of `size` (Task 14 makes this lay one).
      pub fn holds(&self, t: &VenueType, rule: &SizeRule) -> bool;
  }
  pub fn validate_layout(layout: &VenueLayout) -> Result<(), String>;
  /// The layout's checks and every level drawn and checked.
  pub fn validate_venue(layout: &VenueLayout) -> Result<(), String>;
  /// One number for a layout and its drawn levels (tiles, rooms, exits, stations, locks; no light).
  pub fn venue_fingerprint(layout: &VenueLayout) -> u64;
  ```

- [ ] **Step 1: Write the failing graph tests**

In `venue/graph.rs`, on small hand-built graphs (a helper builds one from a node count, edges and starts; node 0 is `Outside(Street)`):

```rust
#[test]
fn a_one_way_lock_is_free_only_from_its_side() {
    // 0 -> 1 open; 1 -> 2 locked (Card zone 2, free: false); 2 -> 1 the same lock with free: true; goal 2.
    // Every PLAY fails to reach 2 without a card; route_out from 2 reaches 0 with no keys.
}

#[test]
fn a_card_found_on_the_way_opens_its_lock() {
    // card (1, 2) in node 1: route_in(FORCE) and every other PLAY reach node 2 holding card 2.
}

#[test]
fn each_way_passes_only_its_locks() {
    // Locks 1->2 with ways NET, KEYJACK, FORCE, TALK, CRAWL in turn: only the PLAY whose set holds
    // that bit reaches 2 (the Keyjack's set passes both KEYJACK and CRAWL).
}

#[test]
fn talk_hands_over_the_card_of_who_you_reach() {
    // A person of zone 3 in node 1; a Card lock of zone 3 at 1->2: talk reaches 2, force doesn't;
    // a lock with TALK of zone 3 passes after reaching them too.
}

#[test]
fn depth_counts_locks_and_grilles_on_the_cheapest_route() {
    // Route A: two locked doors; route B: a grille then one locked door: depth == 2.
    // Locking route B's open door makes depth 2 still (A); adding a lock on A makes it 3.
}

#[test]
fn the_way_out_is_found_apart_from_the_way_in() {
    // In by a one-way-free door that is locked coming back; out only by a second way: route_in
    // true, route_out true through the second; removing the second makes route_out false.
}
```

The passing rule, for an edge under means `m` holding `k`:

```rust
fn passes(lock: Option<&GraphLock>, m: Ways, k: &Keys) -> bool {
    let Some(l) = lock else { return true };
    l.free
        || (l.ways.has(Ways::CARD) && k.card >= l.zone)
        || (l.ways.has(Ways::CODE) && l.code.is_some_and(|c| k.codes.contains(&c)))
        || (l.ways.has(Ways::TALK) && m.has(Ways::TALK) && k.talked >= l.zone)
        || Ways(l.ways.0 & m.0 & !(Ways::CARD.0 | Ways::CODE.0 | Ways::TALK.0)).0 != 0
}
```

`reach` repeats a breadth-first pass from `from` over passing edges, then picks up the cards and codes lying in reached rooms (and, under `TALK`, raises `talked` and `card` to the zone of each person reached), until a pass finds nothing new. `depth` is a 0-1 search:

```rust
// dist[start] = 0 for every start; a deque; an edge costs 1 when it has a lock that isn't
// `free`, else 0; 0-cost neighbours go to the front, 1-cost to the back; dist[goal] is the depth.
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p citygen --target-dir target/smoke venue::graph`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the graph**

As the interface says. Doors join both ways, a one-way lock with `free: true` on the edge leaving its `free_from` side; links join their stops by `on`/`off` (an edge from stop a to stop b when you can get on at a and off at b); exits join `Outside(stratum)` and the doorstep's room both ways; a crawl way in joins `Outside(stratum)` and its room both ways through a Grille lock.

- [ ] **Step 4: Write the failing checks and the sweep**

In `venue/check.rs` (`testutil::test_type()` for the test type):

```rust
#[test]
fn validate_layout_finds_a_room_nobody_reaches() {
    let mut l = /* test type, own frame, seed 2 */;
    let lv = l.levels.iter_mut().find(|x| x.rooms.len() > 3).unwrap();
    let cut = lv.doors.iter().position(|d| !d.border).unwrap();
    let lonely = lv.doors[cut].rooms.1;
    lv.doors.retain(|d| d.rooms.0 != lonely && d.rooms.1 != lonely);
    assert!(validate_layout(&l).unwrap_err().contains("reach"));
}
```

`validate_layout` checks: every room is reached from the starts by someone holding every key (`means` all bits, `card` the deepest zone, every code); every link stop lies in a room of its level; every exit's doorstep lies in its way-in room; every level has at most one door stratum; lock numbers are unique per level. `validate_venue` adds every level drawn by `draw_level` and checked by `validate_floor` against `FloorPlan::of_layout`. `holds` (until Task 14): the layout has at least `rule.floors.0` levels, at least `rule.ways_in` ways in, and some objective room of the type at depth `rule.locks.0` or more in `VenueGraph::of(layout, &layout.candidates(), None)`.

`crates/citygen/tests/venues.rs` (an integration test: it can't reach `testutil`, so it builds its own cities):

```rust
const SEEDS: u64 = 50;

fn types() -> Vec<VenueType> {
    let mut v = VenuePack::builtin().types.clone();
    v.push(VenueType::parse(include_str!("../src/interior/venue/test_type.ron")).unwrap());
    v
}

#[test]
fn every_type_passes_the_checker() {
    let pack = VenuePack::builtin();
    let cities = [/* seed 6 120x60, medium(8), medium(11) */];
    for t in types() {
        for seed in 0..SEEDS {
            let l = lay_out(&t, VenueFrame::own(&t), seed).unwrap_or_else(|e| panic!("{} own {seed}: {e}", t.id));
            validate_venue(&l).unwrap_or_else(|e| panic!("{} own {seed}: {e}", t.id));
            for rule in &pack.sizes { assert!(l.holds(&t, rule), "{} own {seed} {:?}", t.id, rule.size); }
        }
        // The buildings of its kinds; a type no kind builds stands in the
        // five largest closed buildings of each city, raised to its own top here so every size is held
        // (the console raises only when a building can't hold the size).
        let frames: Vec<VenueFrame> = if t.builds.is_empty() {
            cities.iter().flat_map(|c| largest_closed(c, 5).into_iter()
                .filter_map(|b| VenueFrame::of_building(c, b)).map(|f| f.raised(t.frame.top.max(f.storeys.1)))).collect()
        } else {
            cities.iter().flat_map(|c| c.buildings.iter()
                .filter(|b| t.builds.contains(&b.kind)).filter_map(|b| VenueFrame::of_building(c, b.id))).collect()
        };
        for seed in 0..if frames.is_empty() { 0 } else { SEEDS } {
            let f = frames[seed as usize % frames.len()].clone();
            let l = lay_out(&t, f, seed).unwrap_or_else(|e| panic!("{} building {seed}: {e}", t.id));
            validate_venue(&l).unwrap_or_else(|e| panic!("{} building {seed}: {e}", t.id));
        }
    }
}

/// One site per type, so all three platforms agree. Update a number only for a change
/// meant to move that type's sites; a new type in sites.ron adds its number here.
#[test]
fn venue_fingerprints_are_pinned() {
    const PINNED: &[u64] = &[0, 0, 0];
    let pack = VenuePack::builtin();
    assert_eq!(PINNED.len(), pack.types.len(), "one fingerprint per type, in sites.ron's order");
    for (t, want) in pack.types.iter().zip(PINNED) {
        let l = lay_out(t, VenueFrame::own(t), 0x5A17).unwrap();
        assert_eq!(venue_fingerprint(&l), *want, "{}", t.id);
    }
}
```

`largest_closed(c, n)` is a helper in the test file: the `n` closed buildings with the largest footprint (area, then id). Record `PINNED` from the first run's `left` values, as in Task 1.

- [ ] **Step 5: Run the checks**

Run: `cargo test -p citygen --target-dir target/smoke venue:: && cargo test -p citygen --target-dir target/smoke --test venues`
Expected: PASS after the fingerprints are recorded.
Run: `cargo test --workspace --target-dir target/smoke`
Expected: PASS. A failure in the sweep names the type, the frame and the seed: fix the stage, or the first cut in `sites.ron`, not the check. Note the sweep's time; over 60 s in debug, `SEEDS = 20` (say so in the commit).

- [ ] **Step 6: Commit**

```bash
git add crates/citygen flavor/sites.ron
git commit -F - <<'EOF'
citygen: the venue checker, over every type, frame and seed

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 12: Hotels and offices are venues

**Files:**
- Modify: `crates/citygen/src/interior/plan.rs` (`FloorPlan::new` dispatches), `interior/mod.rs` tests (classic-only tests call `FloorPlan::classic`), `crates/citygen/tests/seeds.rs` if it assumes classic shapes
- Modify: `crates/strata-zero/src/interiors.rs` (`ensure` without cloning the plan; `ensure_homes` from the layout), `credentials.rs` (`prepare`), and tests across `crates/strata-zero` that took a Hotel or Office for a classic floor (move them to a classic kind or to the venue's own shape)

**Interfaces:**
- Consumes: `VenuePack::for_kind`, `VenueFrame::of_building`, `building_seed`, `lay_out`, `FloorPlan::of_layout`, `draw_level` (Tasks 4–10).
- Produces: `FloorPlan::new(city, b)`: for a kind a type builds, `of_layout(lay_out(type, of_building(city, b), building_seed(city.seed, b)))`; when `lay_out` fails, `debug_assert!` and `classic`. `Interiors::ensure` borrows `self.plans` and `self.floors` apart (no plan clone). `Interiors::ensure_homes` counts a venue's `Flat` rooms from its layout without drawing a floor.

- [ ] **Step 1: Write the failing tests**

citygen (`interior/mod.rs`):

```rust
#[test]
fn hotels_and_offices_are_venues_and_towers_are_not() {
    let c = medium(8);
    let mut seen = 0;
    for b in &c.buildings {
        let Some(plan) = FloorPlan::new(&c, b.id) else { continue };
        let venue = VenuePack::builtin().for_kind(b.kind).is_some();
        assert_eq!(plan.venue.is_some(), venue, "{:?} {:?}", b.kind, b.id);
        seen += usize::from(venue);
    }
    assert!(seen > 0);
}
```

strata-zero (`interiors.rs`), with `home_city()` and the first Hotel in it:

```rust
#[test]
fn walking_into_a_hotel_finds_a_venue() {
    // enter() by its first street door: Ok; plan_of(b).venue is Some; the plan has at least two
    // links; plan.lowest() == -(below roles of its type); st.interiors.generated() == 1.
}

#[test]
fn prepare_draws_no_floor_of_a_venue() {
    // A fresh state: credentials::prepare(&mut st, hotel) leaves generated() at 0.
}

#[test]
fn an_old_saves_hotel_keeps_the_classic_builder() {
    // This test's own old shapes (citygen's test OldPlan isn't reachable from here): an OldPlan with
    // Task 1's fields, filled from FloorPlan::classic(&city, b), inside an
    // `OldInteriors { plans: HashMap<BuildingId, OldPlan>, floors: HashMap<(BuildingId, i32), Floor>,
    // homes: HashMap<BuildingId, Homes> }`, written with rmp_serde::to_vec_named and read as Interiors:
    // plan_of(b).venue is None and ensure(b, 1) equals
    // generate_floor(&city, &FloorPlan::classic(&city, b).unwrap(), 1). Another hotel, not in it:
    // plan(&city, other).venue is Some.
}

#[test]
fn workers_stand_at_a_venues_stations() {
    // A world at 10:00; Placement::new on the hotel's ground floor puts the receptionist
    // (`receptionist`) on its Reception station.
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p citygen --target-dir target/smoke hotels_and_offices && cargo test -p strata-zero --target-dir target/smoke interiors::`
Expected: FAIL (plans are classic).

- [ ] **Step 3: Implement the dispatch**

As the interface. Tests that are about today's builder (`plans_follow_height_class_span_and_kind`, `corridor_floors_have_locked_numbered_flats…`, `print_floors`, the pin) call `classic`; sweeps over "every closed building" keep `new` and must pass on venue floors too. `interior::home::home` scans every building for the home's megablock: it calls `classic` (homes are never venues), so finding the home doesn't lay out every hotel in the city. `credentials::prepare` stays as it is (`ensure_homes` no longer draws for a venue).

- [ ] **Step 4: Run the tests**

Run: `cargo test --workspace --target-dir target/smoke && cargo check --workspace --all-targets --features strata-zero/window --target-dir target/smoke`
Expected: PASS, `the_first_saved_run_still_opens` among them.

- [ ] **Step 5: Commit**

```bash
git add crates/citygen crates/strata-zero
git commit -F - <<'EOF'
interiors: every hotel and office is a venue; old saves keep their floors

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 13: Zone locks, zone cards and crawls in play

**Files:**
- Modify: `crates/strata-zero/src/credentials.rs` (zone cards, `opens_lock`, `opens_stop`, `door_label`, `How::Free`, `carries`, `prepare` stocks zone cards), `interiors.rs` (`ride` asks for a stop's card; `crawl_in`, `crawl_out`; `climb` onto a crawl wants the check), `screens/explore.rs` (stepping onto a lock door or a grille), `interact.rs` (a keypad row on a keypad lock door; crawl rows on a grille, on a crawl way in, and on the city tile outside one), `screens/popup.rs` (`WorldAct::Crawl`), `screens/keypad.rs` (`door_label`), `look.rs` + `flavor/looks.ron` (fixtures `zone_door`, `grille`, `escalator`)
- Modify: `crates/strata-zero/src/jobs/card.rs` (its exhaustive match on `Option<How>`, lines 411-418, gets a `How::Free` arm)
- Create: `crates/strata-zero/src/venues.rs` (`can_crawl`; `pub mod venues;` in `main.rs`/`lib.rs` as the crate does)

**Interfaces:**
- Consumes: `Floor::lock_at`, `DoorLock`, `LockKind`, `Ways`, `FloorPlan.crawls`, `LinkStop.card`, `VenueLayout.zones`, `VenuePack::builtin().crawl_tech`.
- Produces:
  ```rust
  // strata_zero::credentials
  /// A zone's card for a venue: `keycard:{b}:zone{z}`. Run-made, so saved with the run.
  pub fn zone_card_id(b: BuildingId, zone: u8) -> ItemId;
  pub fn parse_zone_card(id: &ItemId) -> Option<(BuildingId, u8)>;
  /// The deepest zone card for `b` in the pack.
  pub fn zone_card_held(st: &AppState, b: BuildingId) -> Option<u8>;
  /// Puts a venue's zone cards in the catalogue, once: "Keycard · {building}, {zone}".
  pub fn stock_zone_cards(st: &mut AppState, b: BuildingId);
  pub enum How { Cracked, Keycard, Code(u16), Free }
  /// Whether the player gets through a zone lock stepping from `from`, and how: free from its
  /// free side, the Net's lock crack (a lock with Net), a zone card at its zone or deeper (Card),
  /// the journal's code (Code), in that order.
  pub fn opens_lock(st: &AppState, b: BuildingId, level: i32, lock: &DoorLock, from: Pos) -> Option<How>;
  /// A lift stop's card: none asked, the Net's crack, or a zone card at `card` or deeper.
  pub fn opens_stop(st: &AppState, b: BuildingId, card: u8) -> Option<How>;
  /// "{zone} door" for a lock number; a venue room's number reads its `room_label` ("B1-04",
  /// not "flat 9104"); any other number, `flat_label`.
  pub fn door_label(st: &AppState, b: BuildingId, level: i32, number: u16) -> String;
  // strata_zero::venues
  pub fn can_crawl(st: &AppState) -> bool; // Tech >= crawl_tech
  // strata_zero::interiors
  pub fn crawl_in(st: &mut AppState, b: BuildingId, way: usize) -> Result<String, String>;
  pub fn crawl_out(st: &mut AppState) -> Result<String, String>;
  // screens::popup::WorldAct gains Crawl
  ```

- [ ] **Step 1: Write the failing tests**

`credentials.rs` tests (a state in `home_city()` standing on a hotel venue's floor beside a lock door; `put_plan` where a test needs a lock of a given kind):

```rust
#[test]
fn a_zone_card_opens_its_zone_and_those_below() {
    // With zone_card_id(b, 2) in the pack: a Card lock of zone 2 and one of zone 1 open (Keycard);
    // one of zone 3 doesn't. parse_zone_card(&zone_card_id(b, 2)) == Some((b, 2)); parse_keycard of it is None.
}

#[test]
fn a_card_lock_stays_shut_and_says_whose() {
    // Stepping onto it with nothing: the player stays put, the log warns "locked · {zone} door".
}

#[test]
fn a_keypad_lock_takes_its_code_and_the_keyjack() {
    // try_code(st, b, level, lock.number, door_code(seed, b, level, lock.number)) == Verdict::Open,
    // then opens_lock == Some(How::Code(_)); Keypad::open's label is "{zone} door".
}

#[test]
fn a_venue_room_reads_its_label() {
    // door_label for the number of a venue room below the street is "B1-04"-style (room_label), not
    // "flat 9104"; for a room at or above the street it is its number; a flat's number still reads flat_label.
}

#[test]
fn a_one_way_door_opens_from_its_free_side() {
    // opens_lock(.., lock, lock.free_from.unwrap()) == Some(How::Free); from the other side None.
}

#[test]
fn a_net_crack_opens_only_locks_the_net_reaches() { /* a lock without NET stays shut while net.unlocked(b) */ }

#[test]
fn the_lift_asks_for_its_card_at_a_carded_stop() {
    // ride to a stop with card 3: Err("the lift wants a {zone} card") without; Ok with zone_card_id(b, 3).
}

#[test]
fn a_grille_wants_tech() {
    // Tech below crawl_tech: stepping onto a crawl landing is refused ("a grille, screwed shut");
    // at crawl_tech: climb through the crawl lands on its next stop.
}

#[test]
fn crawling_in_from_the_roof_lands_inside() {
    // On the Heights tile of the plan's Heights CrawlWay: interact offers "Crawl in"; crawl_in lands
    // on the way's floor tile; crawl_out from there lands back on the roof tile.
}

#[test]
fn staff_carry_their_zones_card() {
    // A worker drawn on a staff-zone room of a venue: carries(st, id) holds zone_card_id(b, that zone).
}

#[test]
fn zone_cards_survive_a_reopen_and_a_trip() {
    // A zone card in the pack: a save and runs::open bring it back and it opens its lock; walking into
    // another city and back (outside::enter both ways) does too.
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p strata-zero --target-dir target/smoke credentials:: interiors::`
Expected: FAIL to compile.

- [ ] **Step 3: Implement**

- Stepping onto a door (`MapScreen`'s floor branch, before the private-room check): a `Floor::lock_at` lock asks `opens_lock(.., here)`: `None` warns `locked · {zone} door` (`, keypad` for a keypad lock) and the player stays; `Keycard` says `{zone} door · the keycard opens it`; `Code` says `{zone} door · you key in {c:04}`; `Cracked` and `Free` say nothing. Stepping onto a grille (a `Crawl` link's stop, or a `CrawlWay`'s `at`; an open `Ladder` link is no grille) without `can_crawl` warns `a grille, screwed shut · it takes Tech {n}` and the player stays.
- `interact`: a keypad lock door beside you has `i Interact: Keypad` (`Pick::Keypad` with the lock's number); a grille has the climb rows when you can crawl, else one disabled row naming the Tech; a crawl way in has `Crawl out`; on a stratum, the tile of a closed building's `CrawlWay.outside` (looked up through the building whose footprint holds or touches the tile, its plan made if need be) has `Crawl in`. All through `WorldAct::Crawl`.
- `ride` asks `opens_stop` for the card of the stop you board at and of the stop you ride to (the deeper of the two); refused, it says `the lift wants a {zone} card`. Lifts on alert stop as before.
- `prepare` calls `stock_zone_cards` for a venue. Zone cards are `keycard:`-prefixed, so `run_made`, `run_items`, `purge` and `put_back` carry them already.
- Look: a lock door reads `{Zone} door` / `card` or `keypad` and `locked` or `unlocked`; a grille `Grille` / `a crawl, {Tech n}`; an escalator `Escalator` / `up to {X}` or `down to {X}`.

- [ ] **Step 4: Run the tests**

Run: `cargo test --workspace --target-dir target/smoke && cargo check --workspace --all-targets --features strata-zero/window --target-dir target/smoke`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero flavor/looks.ron
git commit -F - <<'EOF'
venues: zone locks, zone cards and crawls in play

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 14: The mission layer

**Files:**
- Create: `crates/citygen/src/interior/venue/mission.rs`
- Modify: `venue/draw.rs` (`draw_copy_level`), `venue/check.rs` (`validate_copy`; `holds` lays a copy), `crates/citygen/tests/venues.rs` (copies in the sweep), `crates/citygen/src/tile.rs` (`Feature::Turret`, `Feature::Container`, `Feature::Objective` at the end; turret and container not walkable)
- Modify: `crates/strata-zero/src/foes.rs` (`Post`, `Foe.post`), `grid.rs` (its `Foe` literal, line 134, gets `post: None`), `venues.rs` (`guard_foes`), `render/tileset.rs` (`◆` turret `Rgb(255, 60, 70)`, `□` container `Rgb(150, 160, 170)`, `◎` objective `MAGENTA_RGB`; a door with a reader in `Rgb(255, 154, 60)`), `look.rs` + `flavor/looks.ron` (keys `turret`, `container`, `objective`)

**Interfaces:**
- Consumes: Tasks 4–11; `Foe::goon`'s fields.
- Produces:
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
  pub struct MissionInputs { pub size: MissionSize,
      /// Index into the type's objectives.
      pub objective: usize, pub security: u8, pub seed: u64 }
  pub struct Placed { pub level: i32, pub at: Pos }
  pub struct GuardPost { pub level: i32, pub at: Pos, pub zone: u8, pub patrol: Vec<Pos> }
  pub enum DeviceKind { Camera, Turret }
  pub struct Device { pub level: i32, pub at: Pos, pub kind: DeviceKind }
  pub struct CardSpot { pub zone: u8, pub level: i32, pub at: Pos }
  /// A code note: where it lies, and the lock it opens as (level, number).
  pub struct CodeSpot { pub level: i32, pub at: Pos, pub lock: (i32, u16) }
  #[derive(Clone, Debug, PartialEq, Eq)]
  pub struct MissionLayer {
      pub inputs: MissionInputs,
      /// Lowest first.
      pub kept: Vec<i32>,
      pub objective: Placed,
      /// Doors given their `fit` lock: (level, door index).
      pub fitted: Vec<(i32, usize)>,
      pub guards: Vec<GuardPost>, pub devices: Vec<Device>, pub containers: Vec<Placed>,
      pub cards: Vec<CardSpot>, pub codes: Vec<CodeSpot>,
  }
  impl MissionLayer {
      pub fn lay(layout: &VenueLayout, t: &VenueType, rule: &SizeRule, inputs: MissionInputs) -> Result<MissionLayer, String>;
  }
  impl VenueGraph { pub fn copy(layout: &VenueLayout, layer: &MissionLayer) -> VenueGraph; } // goal, cards, codes, guards as people
  pub fn draw_copy_level(layout: &VenueLayout, layer: &MissionLayer, level: i32) -> Option<Floor>;
  /// The copy's graph checks: the four ways in and out, depth, ways in, kept levels joined.
  pub fn validate_copy_graph(layout: &VenueLayout, layer: &MissionLayer, rule: &SizeRule) -> Result<(), String>;
  /// The graph checks, and every kept level drawn with the layer and checked.
  pub fn validate_copy(layout: &VenueLayout, layer: &MissionLayer, rule: &SizeRule) -> Result<(), String>;
  // strata_zero::foes
  /// A guard's post in a mission's copy: whose it is, its zone (the card it carries) and
  /// its patrol, recorded for when guards act on floors (#33).
  #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
  pub struct Post { pub building: BuildingId, pub zone: u8, pub patrol: Vec<Pos> }
  pub struct Foe { /* ... */ #[serde(default)] pub post: Option<Post> }
  // strata_zero::venues
  /// The layer's guards as inert foes at their posts: name "guard", glyph '@', post set.
  pub fn guard_foes(layer: &MissionLayer, b: BuildingId) -> Vec<Foe>;
  ```

- [ ] **Step 1: Write the failing tests**

In `venue/mission.rs` (`testutil::test_type()`, own frame, seeds `0..20`, each size, security `0` and `255`):

```rust
#[test]
fn kept_floors_follow_the_order() {
    // kept holds 0 and the objective's level; rule.floors.0 <= kept.len() <= rule.floors.1;
    // every other level with a door to the city is kept before any level without one, nearer G
    // first; the kept levels are joined by the links restricted to them (one component).
}

#[test]
fn locks_raise_the_depth_to_the_size() {
    // VenueGraph::copy(..).depth(goal) >= rule.locks.0; fitted only names candidate doors on kept levels.
}

#[test]
fn every_card_and_code_lies_before_its_lock() {
    // For each CardSpot of zone z: its room is reached from the starts by base means (no bits)
    // holding only the cards of the spots before it; base route_in and route_out succeed.
}

#[test]
fn guards_number_with_size_and_security() {
    // security 0: guards.len() == rule.guards.0; 255: == rule.guards.1; each on a kept level, in a
    // room of a zone the type's security weights name.
}

#[test]
fn no_post_blocks_a_landing_doorway_or_doorstep() {
    // Every post and every patrol tile is a room spot: not a landing tile, a door, a doorstep, an exit or reserved.
}

#[test]
fn devices_stand_on_floor_spots() {
    // draw_copy_level: every Camera/Turret/Container/Objective feature stands on Terrain::Floor, never
    // a wall, a door or a doorstep; every fitted door is in the floor's locks; the floor passes validate_floor.
}

#[test]
fn a_grille_needs_no_key() {
    // A layout whose lowest-zone lock out of the start is a grille (a crawl way in): `lay` is still Ok
    // and no CardSpot or CodeSpot is laid for it.
}

#[test]
fn the_same_inputs_lay_the_same_layer() { /* lay twice: equal */ }
```

In `crates/citygen/tests/venues.rs`, `every_type_passes_the_checker` adds, for each own-frame and building layout and each size it `holds`: `lay` with `objective = seed as usize % t.objectives.len()` and then each other objective, `security = (seed * 37 % 256) as u8`, `seed`; an (objective, seed) whose `lay` errs is skipped, and the test fails if a seed lays no objective at all at a size the building `holds`; every layer that lays goes to `validate_copy(..).unwrap_or_else(|e| panic!("{} {seed} {:?}: {e}", t.id, size))`.

In `strata-zero/src/venues.rs`:

```rust
#[test]
fn guards_are_inert_foes_at_their_posts() {
    // guard_foes(&layer, b): one per post; inert, standing, place Floor { building: b, level: post.level },
    // pos post.at, post == Some(Post { building: b, zone: post.zone, patrol: post.patrol.clone() }).
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p citygen --target-dir target/smoke venue::mission`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `lay`**

Its rng is `hash64(layout.seed, inputs.seed)`; it fails with a reason a test can read.

1. **Objective:** the spots of rooms named by the objective's room (on its role's levels when it names one); the rng picks one; `Err("no {room} for the objective")` when none.
2. **Kept levels** (choice 14):
   ```rust
   // must = {0, objective level} plus the levels on the shortest link path between them
   // (breadth-first over levels joined by any link's on/off stops);
   // n = rng in rule.floors; Err if must.len() > rule.floors.1;
   // then, while kept.len() < n: the next door level not kept, nearest 0 first;
   // then the level next to a kept one nearest the objective's level;
   // finally, any level a kept link needs to join the kept ones is added (Err if that passes the max).
   ```
3. **Locks to the size:** target = rng in `rule.locks`. Greedy, over candidates on kept levels:
   ```rust
   // loop: g = VenueGraph::of(layout, &fitted, Some(&kept)); d = g.depth(goal);
   // if d >= target { break }
   // walk one cheapest route (0-1 search with parents) from a start to the goal and fit the
   // candidate on it nearest the goal; none left on it: Err("too shallow for {size}").
   ```
   Any route at depth `d` holds an unfitted candidate while `d` is below the depth with every candidate fitted, so this ends once `holds` says yes.
4. **Keys:** repeat until the base way (no means) gets in and out: reach with what lies so far; among the locks on edges leaving the reached rooms whose ways include `Card` or `Code` (a grille's ways are `CRAWL`: it needs no key and is skipped, a crawl way in's included), the one of lowest zone (then lowest node) gets its card (a `Card` lock) or code (a `Code` lock) left on a free spot of the reached room of deepest zone (then nearest the lock); a lock with neither is `Err`. The same for the way out from the goal.
5. **Guards:** `rule.guards.0 + (rule.guards.1 - rule.guards.0) * security / 255`, shared out by the security def's zone weights over kept rooms of those zones, one to a spot; each patrol is 2–4 spots of rooms of its zone on its level, starting at the post.
6. **Devices and containers:** cameras and turrets from the def's ranges scaled by security the same way, cameras on spots of spine and hall rooms nearest the fitted doors, turrets only in `turret_zones`; containers in rooms above public; the objective spot drawn as `Feature::Objective`.

`holds` now returns whether `lay` with seed 0 succeeds for some objective and `validate_copy_graph` passes (it stays that: the sweep, not `holds`, skips an (objective, seed) that errs; Task 16's test still wants three objectives per type) (no floor is drawn, so a mission can ask it of many buildings). `validate_copy_graph`: the four ways (`PLAYS`) each have `route_in`, and `route_out` from the goal with the keys it ended with; depth at least `rule.locks.0`; at least `rule.ways_in` open ways in on kept levels; kept levels joined by the restricted links. `validate_copy` adds every kept level drawn by `draw_copy_level`, checked by `validate_floor` against `FloorPlan::of_layout(layout.clone())` and by the device rule.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p citygen --target-dir target/smoke venue:: && cargo test -p citygen --target-dir target/smoke --test venues`
Expected: PASS; the fingerprints stay (the layer doesn't touch the plain layout).
Run: `cargo test --workspace --target-dir target/smoke && cargo check --workspace --all-targets --features strata-zero/window --target-dir target/smoke`
Expected: PASS (fixtures: `Foe.post` defaults).
Run: `cargo test -p strata-zero --features window --target-dir target/smoke every_drawn_character`
Expected: PASS (`◆`, `□` and `◎` are new).

- [ ] **Step 5: Commit**

```bash
git add crates/citygen crates/strata-zero flavor/looks.ron flavor/sites.ron
git commit -F - <<'EOF'
citygen: a mission's layer on a venue: kept floors, locks, keys, guards and devices

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 15: The mission copy

**Files:**
- Modify: `crates/strata-zero/src/venues.rs` (`MissionCopy`, `begin`, `end`, `tick`, `door_marks`, `DoorMark`, `copy_live`, `note_at`), `interiors.rs` (`copies` field; `ensure` draws a live copy's kept levels with the layer; `enter` refuses a barred door; `drawn_at` is `None` in a live copy; `put_plan` loses `#[cfg(test)]`; `drop_floors`), `crates/citygen/src/interior/plan.rs` (`FloorPlan::kept`), `clock.rs` (`venues::tick` after `jobs::tick`), `screens/explore.rs` (the door-marks overlay; no `Placement` on a live copy's floors), `look.rs` + `flavor/looks.ron` (`flagged_door`, `barred_door`, `shutter_sealed`, a code note under a spot), `track/route.rs` (no leg to a level the plan lacks)
- Create: `crates/strata-zero/src/render/doors.rs` (`DoorMarks` overlay; `pub mod doors;` in `render/mod.rs`)

**Interfaces:**
- Consumes: `MissionLayer::lay`, `draw_copy_level`, `guard_foes`, `stock_zone_cards`, `zone_card_id`, `VenuePack::builtin().by_id(&layout.type_id)`.
- Produces:
  ```rust
  // citygen::interior::plan
  impl FloorPlan {
      /// The plan with only `kept` levels: link stops cut to them, links with fewer than two stops
      /// dropped, crawls on other levels dropped, `doors` kept whole (so their doors read barred).
      pub fn kept(&self, kept: &[i32]) -> FloorPlan;
  }
  // strata_zero::venues
  #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
  pub struct MissionCopy {
      pub inputs: MissionInputs,
      pub kept: Vec<i32>,
      /// Taken: the floors are the copy's. False while it waits for you to be out.
      pub live: bool,
      /// Asked to end: it does once you're out.
      pub ending: bool,
      /// The cards the layer left, where they were left.
      pub left: Vec<(Place, Pos, ItemId)>,
  }
  pub enum DoorMark { Flagged, Barred }
  /// Takes a site for a mission. Err when it already holds one, isn't a venue, or can't hold the size.
  pub fn begin(st: &mut AppState, b: BuildingId, inputs: MissionInputs) -> Result<String, String>;
  /// Lets the site go; it happens at once when you're out, else when you are.
  pub fn end(st: &mut AppState, b: BuildingId) -> Result<String, String>;
  /// Takes a waiting copy, or ends one, once the player is out of its building.
  pub fn tick(st: &mut AppState);
  pub fn copy_live(st: &AppState, b: BuildingId) -> bool;
  /// The city-map doors of venue buildings the interiors hold: flagged (a live copy's, on kept
  /// levels) and barred (doors on levels the plan lacks).
  pub fn door_marks(st: &AppState) -> Vec<(StratumKind, Pos, DoorMark)>;
  /// The code note on a copy's floor tile: "a sticky note: {code:04} · {door}".
  pub fn note_at(st: &AppState, place: Place, pos: Pos) -> Option<String>;
  // strata_zero::interiors
  #[serde(default)] copies: BTreeMap<BuildingId, MissionCopy>, // in Interiors
  impl Interiors {
      pub fn copy(&self, b: BuildingId) -> Option<&MissionCopy>;
      pub(crate) fn copy_mut(&mut self, b: BuildingId) -> Option<&mut MissionCopy>;
      pub(crate) fn set_copy(&mut self, b: BuildingId, copy: Option<MissionCopy>);
      pub fn put_plan(&mut self, plan: FloorPlan);
      pub(crate) fn drop_floors(&mut self, b: BuildingId);
  }
  ```

- [ ] **Step 1: Write the failing tests**

In `venues.rs` (a state in `home_city()`, a Hotel venue `b` with its street door beside the player on the street; `inputs(size)` builds `MissionInputs { size, objective: 0, security: 128, seed: 7 }`):

```rust
#[test]
fn begin_flags_every_door_and_bars_the_dropped_levels() {
    // begin(Small): live at once (the player is out); every portal of b on a kept level is
    // DoorMark::Flagged in door_marks, the others Barred; entering by a barred door is Err("barred").
}

#[test]
fn the_copy_keeps_only_its_floors() {
    // plan_of(b).levels == copy.kept; the lift's rides list only kept levels; climbing a stair from a
    // kept level lands on the next kept level it serves; ensure(b, a level not kept) is None.
}

#[test]
fn the_copy_draws_its_layer() {
    // ensure a kept level: its features hold the layer's devices, its locks the fitted doors; st.foes
    // holds guard_foes(&layer, b); a zone card lies at each CardSpot; zone cards are in the catalogue.
}

#[test]
fn a_mission_waits_while_you_are_inside() {
    // Player on b's ground floor: begin is Ok but not live, doors not flagged, the floor unchanged;
    // leave; tick: live.
}

#[test]
fn leave_and_come_back_and_it_is_as_you_left_it() {
    // In the copy, pick up a card and drop a coil; leave; tick and advance the clock 24 game hours;
    // come back: the card is in the pack, the coil where it lay, the foes and the floors as they were.
}

#[test]
fn ending_brings_the_plain_building_back() {
    // In the copy, key the right code into an always-locked keypad door. Then end while out: no foe
    // with post b; the left cards still lying are gone; the coil dropped in the copy stays where it
    // lies; ensure(b, l) == draw_level(layout, l) for a kept level; door_marks has no Flagged for b;
    // plan_of(b).levels == the layout's levels; copy(b) is None; and opens_lock on that keypad door
    // is Some(How::Code(_)): the code learned in the copy works in the plain building.
}

#[test]
fn ending_waits_until_you_are_out() { /* end while inside: still live; leave; tick: ended */ }

#[test]
fn one_mission_per_site() { /* a second begin on b is Err */ }

#[test]
fn the_real_building_carries_on_unseen() {
    // A worker inside b: drawn_at is None while live, Some after the end.
}

#[test]
fn a_copy_comes_back_whole() {
    // runs::testutil::round over a state with a live copy and the player on a kept floor:
    // runs::testutil::same on interiors, foes and ground; after restore the flags and kept plan hold.
}

#[test]
fn a_copy_travels_with_its_city() {
    // Live copy in the home city; walk into another city (outside::enter) and back: copy(b) the same,
    // foes the same. And a kept city's StayParts written and read keeps a copy in it.
}

#[test]
fn a_track_to_a_dropped_floor_finds_no_way() {
    // Track a spot on a level the copy doesn't keep: leg_now is Err(NoLeg::NoWay), no panic;
    // after end(b) and tick, leg_now is Ok.
}

#[test]
fn a_copy_seals_its_track_ends() {
    // A builtin type with a Track piece (found by iterating the pack; the first cut has one on each
    // platform level), laid on the largest closed building of home_city() other than the player's home
    // (VenueFrame::of_building(..).raised(its own top)), made the building's plan with put_plan,
    // then begin at Large (it keeps every level): the Look of a shutter tile reads
    // a `shutter_sealed` line; before begin, a `shutter` line.
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p strata-zero --target-dir target/smoke venues::`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the copy**

- `begin`: `Err` when `copy(b)` exists (`"a job is already on there"`), when the plan isn't a venue, when the type is gone from this build, or when `lay` fails (`"that building can't hold a {size} job"`). Records `MissionCopy { live: false, ending: false, kept: layer.kept, .. }` and runs `tick`.
- Taking a copy (in `tick`, player out of `b`): `put_plan(plain.kept(&kept))`, `drop_floors(b)` (copy floors are drawn on demand: `ensure`, for a building with a live copy, lays the layer again from the copy's inputs, the pack's type `by_id(&layout.type_id)` and `size(inputs.size)`, and draws with `draw_copy_level`), `st.foes.extend(guard_foes(..))`, `stock_zone_cards`, each `CardSpot` laid on the ground and recorded in `left`, `live = true`.
- Ending (in `tick`, `ending`, player out): foes with `post.building == b` removed, `left` cards still where they were taken off the ground (one each), `drop_floors(b)`, `put_plan(FloorPlan::of_layout(layout))`, the copy removed.
- "Inside" is `player.place == Place::Floor { building: b, .. }`.
- `door_marks` over the venue plans the interiors hold; `DoorMarks` draws them on strata maps: flagged doors in `MAGENTA_RGB`, barred `▮` in `Rgb(90, 90, 96)`. Look on such a door: `Flagged door` / `{type name} · a job inside`; `Barred door` / `shut for the job`.
- `note_at` reads the layer's `codes` (re-laid from the copy's inputs) for a live copy, and `look.rs` shows it under the tile's card; the code is `door_code(seed, b, level, number)`.
- `after_restore` needs nothing new: copies, foes, ground, journal and plans are saved whole. `tick` ends at once a copy whose type this build lacks (ending needs only the layout, which the plan holds) and logs `lost in the update · the job in {building}`.

- [ ] **Step 4: Run the tests**

Run: `cargo test --workspace --target-dir target/smoke && cargo check --workspace --all-targets --features strata-zero/window --target-dir target/smoke`
Expected: PASS, the fixtures among them (`Interiors.copies` defaults).
Run: `cargo test -p strata-zero --features window --target-dir target/smoke every_drawn_character`
Expected: PASS (`▮` is new).

- [ ] **Step 5: Commit**

```bash
git add crates/citygen crates/strata-zero flavor/looks.ron
git commit -F - <<'EOF'
venues: a mission's copy of a site, its doors flagged until it ends

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 16: The first three types

**Files:**
- Modify: `flavor/sites.ron`; `crates/citygen/tests/venues.rs` (re-pinned fingerprints; one more test)

**Interfaces:**
- Consumes: everything in `venue`.
- Produces: the hotel, office and metro station types per the spec's "The first three types", passing the sweep at every size on their own frames.

- [ ] **Step 1: Write the failing test**

```rust
/// What every builtin type offers, said without naming one.
#[test]
fn every_builtin_type_is_a_whole_site() {
    let pack = VenuePack::builtin();
    for t in &pack.types {
        assert!(t.zones.len() >= 4, "{}", t.id);
        assert!(t.objectives.len() >= 3, "{}", t.id);
        let kinds: BTreeSet<_> = t.links.iter().map(|l| l.kind).collect();
        assert!(kinds.len() >= 3, "{}: lifts, stairs and a crawl at least", t.id);
        assert!(t.links.iter().any(|l| l.kind == LinkKind::Crawl) || !t.crawls_in.is_empty(), "{}", t.id);
        assert!(t.borders.iter().any(|b| b.one_way), "{}: a way out harder than the way in", t.id);
        let own = lay_out(t, VenueFrame::own(t), 0).unwrap();
        assert!(own.levels.len() >= 6, "{}", t.id);
        if t.builds.is_empty() {
            // A type no building kind builds stands below the street.
            assert!(own.levels.iter().filter(|l| l.level < 0).count() >= 5, "{}", t.id);
        }
    }
}
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test -p citygen --target-dir target/smoke --test venues every_builtin_type`
Expected: FAIL on the first cut.

- [ ] **Step 3: Write the three types**

From the spec's lists and the mocks (room programs in `levels.py` and `metro.py`); room names are the game's words, plain ASCII:

- **Hotel** (`builds: [Hotel]`; zones public, guest, staff, secure): −1 car park (`CarPark` with the `Ramp` up to G, the security office with its server, `Server`), −2 service (kitchen, laundry, lockers, a store); G lobby (reception, a bar, the lift lobby; a staff band with the back corridor and the manager's office); guest floors in the middle with guest rooms of three sizes, a linen room and the staff stair; the penthouse on top (foyer, lounge, bedroom, the office with the safe). Links: guest lift (all, a secure card at the penthouse), service lift (not the penthouse), service stair, grand stair (G and 1F), a fire stair with `alight: Ends`, the riser crawl; crawls in from the roof and the Undercity. Borders: staff and secure doors `always`; secure one-way out. Objectives: the penthouse safe, a guest's room, the security office's server. Own frame about the Pale Orchid's size (mock 46 x 28 on G).
- **Office** (`builds: [Office]`; zones public, staff, secure, executive): G lobby with the `GateLine` (public to staff, one-way out) and a security desk; open-plan floors to fill the middle; the executive floor on top; a server room in a −1 basement with the plant room. Links: two public lifts behind the gate line (staff stops), a goods lift, two stairs (one a fire stair with `alight: Ends`), a duct crawl. Objectives: the server room, the executive's office, a desk on the open plan.
- **Metro station** (`builds: []`, own frame `kind: Depot`, footprint as Cinder Cross's 96 x 40 / 3; zones public, paid, staff, secure): G the station hall; −1 the concourse (ticket hall, gate line into the paid side (its (public, paid) border is `one_way`: exit-only, free from the paid side to the public), the staff block, the cash room; Undercity maintenance doors from the frame); −2 and −3 platforms (`Track` each) for two lines; −4 service with Line Control (`secure`); −5 drainage and a cable vault. Links: street stairs and hall escalators (G and −1), platform escalators (−1 to −2, −2 to −3, one `Up` and one `Down` bank), the platform lift (−1 to −3), the freight lift (G, −1, −4, a staff card), the emergency stair (G to −4, `alight: Ends`), the ventilation shaft crawl (G to −4), the old shaft ladder (−4 to −5). Objectives: Line Control's core, the cash room, the signal box.

Security for every size in each, first guesses. Run the sweep; tune shares, sizes and borders until every size passes on each own frame and on the city buildings. Then record the new `PINNED` fingerprints (this change is meant to move them).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p citygen --target-dir target/smoke --test venues && cargo test --workspace --target-dir target/smoke`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add flavor/sites.ron crates/citygen/tests/venues.rs
git commit -F - <<'EOF'
flavor: the hotel, the office and the metro station as venue types

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 17: The console's `site`, the title's DEVELOPER menu, and shots of every type

**Files:**
- Modify: `crates/strata-zero/src/screens/console.rs` (`site`: `COMMANDS` grows to 44, `help`, `complete`), `venues.rs` (`dev_site`, `building_at_door`, `dev_start`), `screens/title.rs` (the DEVELOPER row and its menu), `screens/shots.rs` (`site_shots`)

**Interfaces:**
- Consumes: `begin`, `end`, `VenueFrame::of_building`, `VenueFrame::raised`, `lay_out`, `FloorPlan::of_layout`, `put_plan`, `holds`.
- Produces:
  ```rust
  // strata_zero::venues
  /// The closed building whose city-map door is at or beside `at` on its stratum.
  pub fn building_at_door(st: &AppState, at: game::At) -> Option<BuildingId>;
  /// Dev: lays type `type_id` on `b` (the building's frame, raised to the type's own top when it
  /// can't hold `inputs.size`), makes it the building's plan and begins a mission there.
  pub fn dev_site(st: &mut AppState, b: BuildingId, type_id: &str, inputs: MissionInputs) -> Result<String, String>;
  /// Dev, from the title's DEVELOPER menu: in a state NEW GAME (DEBUG) has just made (D'SHAWN maxed,
  /// a fresh city), picks the largest-footprint closed building away from home, lays `type_id` on it
  /// at `size`, begins the mission copy when `mission` (else leaves the plain site), and stands the
  /// player on the street outside its main street door.
  pub fn dev_start(st: &mut AppState, type_id: &str, size: MissionSize, mission: bool) -> Result<BuildingId, String>;
  ```

Jeff's ask, added after the plan was written: a DEVELOPER section on the title for testing levels. In dev mode only:
- NEW GAME (DEBUG) stays where it is on the title.
- A new DEVELOPER row takes the place of NEW GAME · COMBAT and LOAD CITY, which move inside it.
- DEVELOPER opens a second menu in the title's own style; esc goes back. It holds:
  - one row per builtin site type, labelled with the type's `name` from the pack (no type named in Rust);
  - COMBAT, with its `NET · STREET` pick as today;
  - LOAD CITY.
- On a type's row, left/right picks the size (`SMALL · MEDIUM · LARGE`, drawn after the label as COMBAT draws `NET · STREET`), and up/down within the row's pick, or a second pick on the same row, chooses `PLAIN · MISSION` (the builder picks the keys; `?`/the row's hint says them).
- Enter starts NEW GAME (DEBUG)'s start, then calls `dev_start`.

- [ ] **Step 1: Write the failing tests**

In `console.rs` tests (`st.unlock_dev()`, the player on the street beside the first Office's street door of the seed-6 city):

```rust
#[test]
fn site_turns_a_building_into_a_copy() {
    // run "site {id} small" with `id` the first builtin type's id read from the pack: Ok; the building's
    // plan is that type's venue (type_id == id); its doors flagged; foes with its post present.
}

#[test]
fn site_end_brings_it_back() { /* "site end": Ok; no Flagged mark; no foe with its post */ }

#[test]
fn site_lists_types_and_completes_ids() {
    // "site" alone: Ok listing every builtin id; complete(st, "site", "") gives every id;
    // complete(st, "site {id}", "") gives small, medium, large.
}

#[test]
fn site_refuses_home_and_inside() {
    // At the home building's door: Err("not your own building"); the home's plan and floors unchanged.
    // On a floor of another building: Err("step outside first").
}

#[test]
fn site_raises_a_low_building_to_hold_the_size() {
    // A four-floor building and the type whose own frame is tallest, "site {id} large": Ok, and the
    // plan's top equals the type's own top. The same type at a size the building holds leaves the
    // top the city made.
}
```

In `title.rs` tests (update the existing row-list tests, which name COMBAT and LOAD CITY on the title):

```rust
#[test]
fn developer_holds_the_test_starts() {
    // Dev mode: the title's rows are NEW GAME (DEBUG) and DEVELOPER among the usual ones, and neither
    // "NEW GAME · COMBAT" nor "LOAD CITY" is on the title. Opening DEVELOPER shows one row per builtin
    // type (its name from the pack), then COMBAT with "NET · STREET", then LOAD CITY; esc goes back.
    // Without dev mode: no DEVELOPER row (the existing "no debug rows" test also checks "DEVELOPER").
}

#[test]
fn a_developer_site_row_starts_at_its_door() {
    // DEVELOPER, the first type's row, size picked LARGE, MISSION: enter. The run is D'SHAWN's, the
    // player stands on the street beside the main street door of a building whose plan is that type's
    // venue, the door is Flagged, and a live copy exists at Large. With PLAIN instead: the same building
    // kind of start, no live copy, the door not Flagged.
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p strata-zero --target-dir target/smoke console:: title::`
Expected: FAIL (`unknown command site`; no DEVELOPER row).

- [ ] **Step 3: Implement**

`site <type id> [small|medium|large] [objective id] [security 0-255] [seed]`: size defaults to `small`, objective to the type's first, security to 128, seed to the tick; `site end` ends the copy at the focus building, else the only live copy. Help line: `site <type> [size] [objective] [security] [seed] | end`. `dev_site` refuses the home building, a building the player is inside, and an open building.

The DEVELOPER row and menu in `title.rs` as above; the COMBAT start and LOAD CITY behave as today from inside it. `dev_start` reuses `dev_site` for the laying and the mission.

`site_shots` (`#[test] #[ignore]`, `SHOT_DIR`): in `state_home()`, the largest-footprint closed building away from home; for each builtin type: `dev_site` at Small then `site end`, writing every level of the plain site (`site_{id}_plain_{label}`, the player on the level's first landing, `MapScreen::walking`, 120 x 40); then for each size, the copy's kept levels (`site_{id}_{size}_{label}`) and one sheet (`site_{id}_{size}_sheet`: each kept level's map drawn at 96 x 36 into one wide buffer, side by side, written by `write_svg`).

- [ ] **Step 4: Run the tests and the shots**

Run: `cargo test -p strata-zero --target-dir target/smoke console:: title::`
Expected: PASS.
Run: `cargo test --workspace --target-dir target/smoke && cargo check --workspace --all-targets --features strata-zero/window --target-dir target/smoke`
Expected: PASS.
Run: `SHOT_DIR=<an empty folder of your own> cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored site_shots`
Expected: PASS, and `.svg` and `.png` for every type, size and level. Look at a few: levels differ, more than one lift or stair, guards and devices on the copies, flagged doors on the street.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero
git commit -F - <<'EOF'
console: site makes any building a venue's copy; DEVELOPER on the title; shots of every type

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 18: Docs, the manual and the checks

**Files:**
- Modify: `docs/saving.md` (a section "Sites and copies", and the `Interiors` line under "Elsewhere"), `docs/development.md` (the `flavor/` line names `sites.ron`; the title's dev starts now say NEW GAME (DEBUG) on the title and DEVELOPER holding the site starts, COMBAT and LOAD CITY; the `site` console command), `manual/06-the-city-and-the-world.md` ("Buildings" and the map marks), `crates/strata-zero/src/interiors.rs` (the module doc: "Never saved; the seed brings them back" is wrong: plans and floors are saved whole)

**Interfaces:**
- Consumes: the whole branch.

- [ ] **Step 1: Write the docs**

- `docs/saving.md` "Sites and copies": a venue's layout is saved inside its building's `FloorPlan` (`venue`), so floors drawn later agree with floors saved; a plan saved before venues reads through `FloorPlanRead` (levels, doors and links from `lowest`/`top`/`bridge`/`roof`/`lift`/`stair`) and keeps today's builder; a building first planned after it is a venue. `Interiors.copies` (`#[serde(default)]`) holds each copy's inputs, kept levels, `live`/`ending` and the cards it left; the copy's floors, its guards (`Foe.post`, `#[serde(default)]`), the items on its floors and the codes learned are play state saved as ever; the copy travels in `Stay` and `StayParts` with `interiors`; zone cards are `keycard:` run items. Nothing is rebuilt in `after_restore`.
- The manual, in its voice, for players: hotels and offices are built floor by floor now, and no two floors are the same; more than one way up (a guest lift and a staff lift, stairs, escalators); a lift may serve only some floors, and some stops want a keycard; levels below `B1`; staff doors want a zone keycard (staff carry them), a code or the Net; vents and ducts are a way through for a runner with the Tech; the new map marks (`↑` `↓` escalators, `▮` a barred door). Leave dev mode and the console out.

- [ ] **Step 2: Run the manual's tests**

Run: `cargo test -p strata-zero --target-dir target/smoke manual::`
Expected: PASS (the booklet draws the Markdown; the glyph test covers the new marks).

- [ ] **Step 3: Run the five checks for both front ends**

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --target-dir target/smoke -- -D warnings
cargo clippy --workspace --all-targets --features strata-zero/window --target-dir target/smoke -- -D warnings
cargo test --workspace --target-dir target/smoke
cargo test --workspace --features strata-zero/window --target-dir target/smoke
```

Expected: all pass. (No window code changed, so the other platforms' `cfg` checks aren't needed; CI runs them.)

- [ ] **Step 4: Commit and close #34**

```bash
git add docs manual crates/strata-zero/src/interiors.rs
git commit -F - <<'EOF'
docs: sites in the save, the manual and the dev notes

Hotels and offices are venues laid out from flavor/sites.ron; a mission's copy of a site
keeps its guards, devices and objective until it ends.

Closes #34

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```
