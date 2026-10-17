# Megablock and Home Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make megablocks rare whole-block H10-style atrium buildings, give the player a hand-shaped flat in one of them where every new game starts, let the flat's rig jack in, and show the ground plaza dimly through the atrium from any height.

**Architecture:** citygen picks at most one whole superblock per Megablocks district as a megablock in the Lots stage. `FloorPlan` gains an atrium rect and a home level. New layouts lay out atrium upper floors, the ground plaza and the home flat, and `interior::home(city)` names the flat. strata-zero computes a `HomeRig` (the flat plus its Net landing) on `enter_city`, spawns the player there, lets `terminal_in_reach` and `jack_in` accept the rig, and draws an `AtriumOverlay` that renders level 0 under `Terrain::Atrium` cells.

**Tech Stack:** Rust 2024, ratatui 0.29, rand_chacha. Crates `citygen`, `sim`, `strata-zero`.

**Spec:** docs/superpowers/specs/2026-10-17-megablock-home-design.md

## Global Constraints

- Every commit passes `cargo clippy --all-targets -- -D warnings` and `cargo test --workspace`. Run `cargo fmt --all` before each commit.
- New enum variants go at the **end** of `Terrain`, `Furniture` and `RoomKind`. `FORMAT_VERSION` stays 3, and old saves must load.
- Same seed, same city, same floors, same home. Interiors are never saved.
- One megablock per `Megablocks` district, at most two per city. A megablock takes its whole block.
- An **atrium plan** is a Megablock plan whose floor is at least 39 wide and 27 high (`w >= 39 && h >= 27`, footprint at least 13×9). Every other Megablock keeps Lobby/Corridor floors. (Ruling during execution: medium cities often only have thin blocks such as 19×9.)
- No transition between the flat and the walkway. They're one floor map.
- The look-down shows terrain and furniture only. No people, no animation. The current level is always brighter.
- Smoke builds use `--target-dir target/smoke` and run `target/smoke/release/strata`. Never rebuild `target/release/strata`.
- Test fixture: `GenParams { seed: 6, size: Custom { w: 120, h: 60 }, districts: 8 }` (its biggest Megablocks block is about 23×19). Medium seeds 8, 11 and 42 have blocks up to 40×20.
- Commit attribution lines as in the session's system reminder.

## Review Focus

1. **A city with no atrium megablock** (an old save with only small Megablocks, or a 4-district city). The player spawns on the street as before, with no panic, and small Megablocks keep corridor floors. Task 6 test, plus Task 2's 4-district sweep.
2. **Riding the lift from the lobby straight to a high atrium level the player has never seen.** That level is generated, level 0 is generated with it, and the look-down draws immediately. Task 6 `ensure` test and Task 9 render test.
3. **Being dumped by ICE while jacked in from home.** The body is back in the flat, with the flat's FOV refreshed, not a blank map. Task 8 test.
4. **The home megablock on alert.** The home rig still jacks in, while a street terminal against the same building is still refused. Task 8 test.
5. **The home level at the edges of the range:** a 14-floor megablock, or one whose bridge falls in the upper third. The home is never on the bridge level, the top level, or below level 2. Task 5 test across the fixture and the medium seeds.

## File Structure

- `crates/citygen/src/tile.rs`: `Terrain::Atrium`, `Terrain::Railing`, `Furniture::Couch`, `Furniture::Shower`, and their rules.
- `crates/citygen/src/interior/floor.rs`: `RoomKind::Home`, `RoomKind::Shop`.
- `crates/citygen/src/generate/lots.rs`: whole-block megablocks.
- `crates/sim/src/people.rs`: housing capacity by size.
- `crates/citygen/src/interior/plan.rs`: `atrium`, `home_level`, shaft override.
- `crates/citygen/src/interior/layouts.rs`: dispatch to the new builders, plus `mod atrium; mod home;`.
- `crates/citygen/src/interior/layouts/atrium.rs` (new): upper atrium floors and the ground plaza.
- `crates/citygen/src/interior/layouts/home.rs` (new): stamps layout A.
- `crates/citygen/src/interior/home.rs` (new): `Home`, `home(city)`.
- `crates/strata-zero/src/home.rs` (new): `HomeRig`, `HomeRig::find`.
- `crates/strata-zero/src/app.rs`, `interiors.rs`, `walker/mod.rs`, `net.rs`, `screens/explore.rs`, `screens/popup.rs`.
- `crates/strata-zero/src/render/tileset.rs`, `render/atrium.rs` (new), `render/mod.rs`.
- `README.md`.

---

### Task 1: New terrain, furniture and room kinds

**Files:**
- Modify: `crates/citygen/src/tile.rs`, `crates/citygen/src/interior/floor.rs`, `crates/strata-zero/src/render/tileset.rs`

**Interfaces:**
- Produces:
  - `Terrain::Atrium` (label "atrium") and `Terrain::Railing` (label "railing"). Both are not walkable, don't block sight, and aren't building parts. `ALL_TERRAIN` becomes `[Terrain; 28]`.
  - `Furniture::Couch` ("couch", glyph `▬`) and `Furniture::Shower` ("shower", glyph `≋`).
  - `RoomKind::Home` ("home") and `RoomKind::Shop` ("shop"). Home is walled; Shop is not (add it to `walled()`'s open list).
  - Glyphs: Atrium `' '` (the overlay draws it), Railing a thin line that joins neighbouring railings through `connects()`, in the wall colour.

- [ ] **Step 1: Failing tests in `tile.rs`**

```rust
#[test]
fn atrium_and_railing_are_open_to_sight_but_not_to_feet() {
    for t in [Terrain::Atrium, Terrain::Railing] {
        let tile = Tile::new(t);
        assert!(!tile.is_walkable());
        assert!(!tile.blocks_sight());
        assert!(!tile.is_building_part());
    }
    assert_eq!(ALL_TERRAIN.len(), 28);
    assert_eq!(*ALL_TERRAIN.last().unwrap(), Terrain::Railing);
    assert_eq!(Furniture::Couch.label(), "couch");
    assert_eq!(Furniture::Shower.label(), "shower");
}
```

In `tileset.rs`: `glyph_for(Terrain::Floor, Some(Feature::Furniture(Furniture::Couch)), ..) == '▬'`, same for Shower → `'≋'`, and a Railing tile with east and west railing neighbours draws a horizontal line glyph.

- [ ] **Step 2: Run** `cargo test -p citygen atrium_and_railing && cargo test -p strata-zero tileset`. Expected: FAIL, the variants don't exist.
- [ ] **Step 3: Add the variants at the end of each enum.** Fill the labels, `walled`, `glyph_for`, `terrain_colors`, `feature_color` and `legend_entries` (Railing and both furniture in the floor legend).
- [ ] **Step 4: Run** `cargo test --workspace`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(citygen): atrium, railing, couch, shower, home and shop rooms`

### Task 2: Rare, whole-block megablocks

**Files:**
- Modify: `crates/citygen/src/generate/lots.rs`, `crates/sim/src/people.rs`
- Modify (fixtures): `crates/citygen/src/interior/mod.rs` (~:268), `crates/strata-zero/src/screens/explore.rs` (~:3922, :4067, :4124), `crates/sim/src/world.rs` (~:1812)

**Interfaces:**
- Produces: after the Lots stage, each `Megablocks` district (in district-id order, at most two per city) has at most one `BuildingKind::Megablock`. Its footprint is the rect of that district's largest block (by area) whose rect is at least 16 wide and 12 high; failing that, its largest block at least 13 wide and 9 high (so its plan is always an atrium plan). If no block qualifies, the district has none. Every other lot in a Megablocks district draws from the theme mix with Megablock removed. No other theme produces Megablocks.
- Produces: `sim::people::housing_capacity(b: &Building) -> usize`. Megablock gives `(fp.w * fp.h / 6).max(30)`; Apartment, Hotel and other kinds stay the same. The call site passes the building.

- [ ] **Step 1: Failing tests in `lots.rs`**

```rust
#[test]
fn megablocks_are_rare_and_take_a_whole_block() {
    for (seed, size, districts) in [(6, SizePreset::Custom { w: 120, h: 60 }, 8), (8, SizePreset::Medium, 7), (11, SizePreset::Medium, 7), (42, SizePreset::Medium, 7)] {
        let c = Generator::run(GenParams { seed, size, districts, ..Default::default() }).unwrap();
        let megas: Vec<_> = c.buildings.iter().filter(|b| b.kind == BuildingKind::Megablock).collect();
        assert!((1..=2).contains(&megas.len()), "seed {seed}: {} megablocks", megas.len());
        for m in &megas {
            assert_eq!(c.district(m.district).theme, Theme::Megablocks);
            assert!(m.footprint.w >= 13 && m.footprint.h >= 9);
            // No other building shares its block: nothing of another building touches the footprint.
            assert!(c.buildings.iter().filter(|b| b.id != m.id).all(|b| b.footprint.grow(1).intersection(m.footprint).is_none()));
        }
    }
}

#[test]
fn a_city_too_small_for_a_megablocks_district_has_no_megablock() {
    let c = Generator::run(GenParams { seed: 3, size: SizePreset::Small, districts: 4, ..Default::default() }).unwrap();
    if !c.districts.iter().any(|d| d.theme == Theme::Megablocks) {
        assert!(c.buildings.iter().all(|b| b.kind != BuildingKind::Megablock));
    }
}
```

In `sim/people.rs`: a Megablock with a 20×18 footprint holds 60, and one with 6×5 holds 30.

- [ ] **Step 2: Run** `cargo test -p citygen megablocks && cargo test -p sim housing`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - In `LotsStage::begin`, after the rng, compute `mega_blocks: HashSet<usize>`. For each Megablocks district in id order, take the index into `ctx.scratch.blocks` of its largest qualifying block, stopping at two. Store it on the stage.
  - In `place_block`, a chosen block skips `bsp`. Its whole `block.rect` becomes one lot of kind Megablock, using the same class, basement, wall/floor and span code as any lot. Factor that code into a `place_building(ctx, rng, rect, kind, spec)` helper used by both paths.
  - Otherwise, `kind` is picked from the mix with Megablock filtered out (re-pick on Megablock, or filter the slice, but keep it deterministic).
  - The capacity change goes in `sim`.
- [ ] **Step 4: Fix the fixtures** that pick "the first Megablock". Use the megablock the city now has. Where a test needs a residential building with a Skyways bridge, an `Apartment` or the megablock works, whichever the fixture holds. Keep each test's intent.
- [ ] **Step 5: Run** `cargo test --workspace`, including `citygen/tests/seeds.rs` (4–12 districts). Expected: PASS.
- [ ] **Step 6: Commit** `feat(citygen): one whole-block megablock per Megablocks district`

### Task 3: The atrium plan and upper floors

**Files:**
- Modify: `crates/citygen/src/interior/plan.rs`, `crates/citygen/src/interior/layouts.rs`
- Create: `crates/citygen/src/interior/layouts/atrium.rs` (declared as `mod atrium;` inside `layouts.rs`, so it sees `Builder`'s private fields)

**Interfaces:**
- Produces:
  - `FloorPlan.atrium: Option<Rect>`: the railing ring's outer rect, `Some` only for an atrium plan (see Global Constraints). The ring is `Railing`, and `atrium.inset(1)` is `Atrium` on every level ≥ 1.
  - `pub const BAND: i32 = 8`: the flat band, the outer wall plus 7 rows of flat (flat interior rows 1..=7, the flat wall at row 8).
  - `pub const WALK: i32 = 2`: the walkway's width.
  - So `atrium = size.rect().inset(BAND + 1 + WALK)`, an inset of 11.
  - `FloorType::Atrium` (upper levels of an atrium plan) and `FloorType::Plaza` (level 0 of an atrium plan). Basement and roof levels are unchanged. An atrium plan's level 0 is `Plaza`, not `Lobby`. Its levels ≥ 1 are `Atrium`, including the bridge and top levels.
  - `corridor_floors()` includes `Atrium` levels.
  - In an atrium plan the shafts move out of the centre: `lift = Some(Pos::new(w/2 - 1, BAND - 3))` and `stair = Pos::new(w/2 - 1, BAND - 2)`. They sit in a lift lobby that's the middle slot of the top flat band, 6 wide, open to the walkway and never private. This holds on every level, Plaza and Basement included.

- [ ] **Step 1: Failing tests in `interior/mod.rs`**

```rust
fn atrium_plans(c: &City) -> Vec<FloorPlan> {
    c.buildings.iter().filter_map(|b| FloorPlan::new(c, b.id)).filter(|p| p.atrium.is_some()).collect()
}

#[test]
fn atrium_floors_validate_and_share_one_atrium() {
    let c = city();
    let plans = atrium_plans(&c);
    assert!(!plans.is_empty());
    for plan in plans {
        let a = plan.atrium.unwrap();
        assert!(!a.contains(plan.stair) && plan.lift_bank().iter().all(|p| !a.contains(*p)));
        for level in plan.levels() {
            let f = generate_floor(&c, &plan, level);
            validate_floor(&plan, &f).unwrap_or_else(|e| panic!("level {level}: {e}"));
            if level >= 1 {
                for p in a.border() { assert_eq!(f.tiles[p].terrain, Terrain::Railing, "{p:?}"); }
                for p in a.inset(1).iter() { assert_eq!(f.tiles[p].terrain, Terrain::Atrium); }
                let flats = f.rooms.iter().filter(|r| r.kind == RoomKind::Flat && r.private).count();
                assert!(flats >= 8, "level {level} has {flats} flats");
            }
        }
    }
}

#[test]
fn small_megablocks_keep_corridor_floors() {
    // A hand-shrunk copy: a Megablock under the threshold plans no atrium.
    let mut c = city();
    let b = c.buildings.iter().position(|b| b.kind == BuildingKind::Megablock).unwrap();
    c.buildings[b].footprint.w = 10;
    c.buildings[b].footprint.h = 8;
    let plan = FloorPlan::new(&c, c.buildings[b].id).unwrap();
    assert!(plan.atrium.is_none());
    assert_eq!(plan.floor_type(1), FloorType::Corridor);
}
```

Also extend the medium-seed sweep in `citygen/tests/seeds.rs` so it validates every level of every atrium plan for seeds 8, 11 and 42.

- [ ] **Step 2: Run** `cargo test -p citygen atrium small_megablocks`. Expected: FAIL.
- [ ] **Step 3: Implement the plan fields** in `FloorPlan::new` and `floor_type`.
- [ ] **Step 4: Implement `Builder::atrium_floor(&mut self)`** in `layouts/atrium.rs`.
  - Railing ring and atrium tiles.
  - A walkway ring of `WALK` between railing and flat wall (a `RoomKind::Corridor` room, not private).
  - Flat walls at inset `BAND` on all four sides.
  - Bands on all four sides, cut into private numbered `Flat` rooms 5–8 long, with doors on the walkway side, furnished by `furnish_room`. The top/bottom bands own the corners.
  - The 6-wide lift-lobby slot.
  - Exit doorsteps on this level (bridge, roof) get vestibules exactly as `corridor()` makes them.
  - A laundry every third level stays.
  - Numbering `level*100 + n`.
  - Dispatch `FloorType::Atrium` in `generate`.
- [ ] **Step 5: Run** `cargo test -p citygen`. Expected: PASS.
- [ ] **Step 6: Commit** `feat(citygen): atrium floors ring an open atrium with flats`

### Task 4: The ground plaza

**Files:**
- Modify: `crates/citygen/src/interior/layouts/atrium.rs`, `crates/citygen/src/interior/layouts.rs`

**Interfaces:**
- Produces: `Builder::plaza_floor(&mut self)` for `FloorType::Plaza`:
  - The atrium area (`atrium` including its ring) is open walkable floor: a `RoomKind::Lobby` room covering the whole inner floor, as `lobby()` does.
  - Reception counter facing the main door, as `lobby()` does (reuse it).
  - Around the edge, in the band where flats would be: one diner (`RoomKind::Bar`, a `Counter` run with a `Role::Bar` station behind it, two or more `Table`s with `Role::Table` stations) and at least two `RoomKind::Shop` rooms (a `Counter` with a `Role::Reception` station behind it). All are open to the plaza (doorways, not private).
  - On the plaza under `atrium.inset(1)`: stalls and tables spaced so it stays one walkable piece. Use `Feature::Stall`, `Furniture::Table` and `Furniture::Stool`, with at least one piece per 40 plaza tiles. Nothing on doorsteps, shafts or stations.

- [ ] **Step 1: Failing test in `interior/mod.rs`**

```rust
#[test]
fn the_plaza_has_a_diner_shops_and_something_to_see_from_above() {
    let c = city();
    for plan in atrium_plans(&c) {
        let f = generate_floor(&c, &plan, 0);
        validate_floor(&plan, &f).unwrap();
        assert!(f.rooms.iter().any(|r| r.kind == RoomKind::Bar));
        assert!(f.rooms.iter().filter(|r| r.kind == RoomKind::Shop).count() >= 2);
        assert!(f.stations.iter().any(|s| s.role == Role::Bar));
        assert!(f.stations.iter().any(|s| s.role == Role::Reception));
        let under = plan.atrium.unwrap().inset(1);
        let pieces = under.iter().filter(|p| f.tiles[*p].feature.is_some_and(|x| matches!(x, Feature::Stall | Feature::Furniture(_)))).count();
        assert!(pieces as i32 >= under.w * under.h / 40, "{pieces} pieces under the atrium");
        assert!(under.iter().all(|p| f.tiles[p].terrain != Terrain::Atrium));
    }
}
```

- [ ] **Step 2: Run** it. Expected: FAIL (Plaza is not dispatched).
- [ ] **Step 3: Implement `plaza_floor`** and dispatch it.
- [ ] **Step 4: Run** `cargo test -p citygen`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(citygen): the megablock's ground plaza with a diner and shops`

### Task 5: The home flat

**Files:**
- Modify: `crates/citygen/src/interior/plan.rs`, `crates/citygen/src/interior/layouts/atrium.rs`, `crates/citygen/src/interior/mod.rs`, `crates/citygen/src/lib.rs` (re-export)
- Create: `crates/citygen/src/interior/layouts/home.rs`, `crates/citygen/src/interior/home.rs`

**Interfaces:**
- Produces:
  - `FloorPlan.home_level: Option<i32>`. It's `Some` only on the **home building**: the atrium plan of the lowest-id Megablock that qualifies. The level is picked with the plan rng from `max(2, (top*2)/3) ..= top-1`, never `bridge`. If that range is empty after removing the bridge, use `None`.
  - On that level, `atrium_floor` reserves a 37-wide run of the **bottom** flat band, walls included, before cutting flats. It's centred, and the band is at least 37 long since atrium plans are at least 39 wide. The lift lobby is always in the top band. `Builder::home_flat(&mut self, rect: Rect, door: Pos)` stamps layout A into that run's interior (35×7):
    - A front door on the walkway wall.
    - A living room and kitchen along the window side with `Couch`, `Table` and `Counter`.
    - A walled bedroom with `Bed`, `Desk` and a `Feature::Terminal` on the desk row.
    - A hall to a walled shower room (`Shower`) and a walled storage room (`Shelf`, `Locker`).
    - Inner doors are plain `Door` tiles.
    - One `Room { kind: RoomKind::Home, private: true, door: Some(front door), number: level*100 + n }` covering the whole interior. The inner walls sit inside its rect, so the sealed-ring rule holds.
    - No stations inside.
  - `pub struct Home { pub building: BuildingId, pub level: i32, pub number: u16, pub rect: Rect, pub door: Pos, pub rig: Pos, pub bed_side: Pos }`:
    - `rig` is the Terminal tile.
    - `bed_side` is a walkable tile 4-adjacent to the bed.
  - `pub fn home(city: &City) -> Option<Home>` in `interior/home.rs`. It builds the plan and generates the home level to read these off, and is re-exported from the crate root.

- [ ] **Step 1: Failing tests in `interior/home.rs`**

```rust
#[test]
fn the_home_is_a_whole_flat_high_in_the_megablock() {
    for c in [fixture(), medium(8), medium(11), medium(42)] {
        let h = home(&c).expect("a home");
        let plan = FloorPlan::new(&c, h.building).unwrap();
        assert_eq!(plan.home_level, Some(h.level));
        assert!(h.level >= 2 && h.level < plan.top && Some(h.level) != plan.bridge);
        let f = generate_floor(&c, &plan, h.level);
        validate_floor(&plan, &f).unwrap();
        let room = f.rooms.iter().find(|r| r.kind == RoomKind::Home).unwrap();
        assert!(room.private && room.door == Some(h.door) && room.rect == h.rect);
        let has = |x: Furniture| h.rect.iter().any(|p| f.tiles[p].feature == Some(Feature::Furniture(x)));
        for x in [Furniture::Bed, Furniture::Desk, Furniture::Couch, Furniture::Shower, Furniture::Counter, Furniture::Shelf] { assert!(has(x), "{x:?}"); }
        assert_eq!(f.tiles[h.rig].feature, Some(Feature::Terminal));
        // The living room has windows: the outer wall along the home is windowed.
        assert!((h.rect.x..h.rect.right()).any(|x| f.tiles[Pos::new(x, h.rect.bottom())].terrain == Terrain::Window));
        assert!(f.tiles[h.bed_side].is_walkable() && h.rect.contains(h.bed_side));
        assert!(f.rooms.iter().filter(|r| r.kind == RoomKind::Home).count() == 1);
        assert_eq!(home(&c), Some(h)); // deterministic
    }
}
```

Plus: `Home` derives `Clone, Copy, Debug, PartialEq, Eq`. Task 3's `atrium_floors_validate_and_share_one_atrium` relaxes to `flats >= 4` on `plan.home_level`, since the home takes most of the bottom band there. And sim `Placement` never assigns an owner to the home's door: the existing filter on `RoomKind::Flat` already excludes it, so assert it in `strata-zero/src/interiors.rs` tests.

- [ ] **Step 2: Run** `cargo test -p citygen home`. Expected: FAIL.
- [ ] **Step 3: Implement** `home_level`, the reserved run, `home_flat` and `home()`.
- [ ] **Step 4: Run** `cargo test --workspace`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(citygen): the player's flat in the megablock`

### Task 6: Start at home

**Files:**
- Create: `crates/strata-zero/src/home.rs`
- Modify: `crates/strata-zero/src/main.rs` or `lib.rs` (`mod home;`), `app.rs`, `interiors.rs`, `walker/mod.rs`, `screens/explore.rs`

**Interfaces:**
- Consumes: `citygen::home`, `citygen::Home`, `crate::constructs::city_gate`.
- Produces:
  - `pub struct HomeRig { pub home: citygen::Home, pub landing: Pos }`.
  - `HomeRig::find(city: &City) -> Option<HomeRig>`. `landing` is the first walkable `StratumKind::Net` tile outside the building's footprint that's 4-adjacent to `city_gate(city, building)`.
  - `AppState.home: Option<HomeRig>`, set in `enter_city`.
  - `Player::at(place: Place, pos: Pos) -> Player` (empty memory); `Player::spawn` uses it.
  - `Interiors::ensure` generates level 0 first for an atrium plan whenever it generates a level ≥ 1.
  - `MapScreen::enter_walk`: with no player and `st.home` set, it ensures the home floor and spawns `Player::at(Place::Floor { building, level }, home.bed_side)` with FOV from `MapRef::for_place`. Otherwise it uses today's street spawn.

- [ ] **Step 1: Failing tests**
  - In `interiors.rs`:

    ```rust
    #[test]
    fn an_atrium_level_brings_the_plaza_with_it() {
        let (city, mut ints) = (fixture_city(), Interiors::default());
        let h = citygen::home(&city).unwrap();
        ints.ensure(&city, h.building, h.level).unwrap();
        assert!(ints.floor(h.building, 0).is_some());
    }
    ```

  - In `screens/explore.rs`:

    ```rust
    #[test]
    fn a_new_game_wakes_up_at_home() {
        let mut st = state8();
        let s = MapScreen::walking(&mut st);
        let h = st.home.unwrap();
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.place, Place::Floor { building: h.home.building, level: h.home.level });
        assert_eq!(p.pos, h.home.bed_side);
        assert_eq!(s.place, p.place);
        assert!(p.vis(p.place, h.home.rig) == Visibility::Visible);
    }

    #[test]
    fn a_city_without_a_home_starts_on_the_street() {
        let mut st = state8();
        let mut city = st.city.take().unwrap();
        for b in city.buildings.iter_mut().filter(|b| b.kind == BuildingKind::Megablock) { b.kind = BuildingKind::Apartment; }
        st.enter_city(city);
        assert!(st.home.is_none());
        MapScreen::walking(&mut st);
        assert!(matches!(st.player.as_ref().unwrap().place, Place::Stratum(StratumKind::Street)));
    }
    ```

  - In `title.rs`: extend `new_game_debug_starts_dshawn_maxed_out_in_a_fresh_city` to walk in and assert the place is a floor when `st.home` is `Some`.
- [ ] **Step 2: Run** `cargo test -p strata-zero home`. Expected: FAIL.
- [ ] **Step 3: Implement** the items listed under Produces. The visibility accessor in the test is whatever `Player` exposes today; use it.
- [ ] **Step 4: Run** `cargo test --workspace`. Expected: PASS. Fix existing tests that assumed a street spawn by teleporting the player to the street where they need it (`Player::at`).
- [ ] **Step 5: Commit** `feat(app): every new game starts at home`

### Task 7: The home door

**Files:**
- Modify: `crates/strata-zero/src/screens/explore.rs` (`doorway`)

**Interfaces:**
- Produces: the private-room lock check in `doorway` skips `RoomKind::Home`. Stepping onto the home's front door from outside logs `home · flat {number}` (info, not a warning). Walking through any home inner door logs nothing.

- [ ] **Step 1: Failing test:** from the walkway tile outside `home.door`, walking onto it moves the player onto the door with no `locked` warning and logs `home · flat {n}`. A neighbouring `Flat`'s door still warns `locked ·`.
- [ ] **Step 2: Run** it. Expected: FAIL (locked).
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -p strata-zero`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(app): the home door opens for you`

### Task 8: Jack in from the rig

**Files:**
- Modify: `crates/strata-zero/src/walker/mod.rs`, `net.rs`, `screens/explore.rs`, `screens/popup.rs`

**Interfaces:**
- Consumes: `HomeRig`.
- Produces:
  - `Player::terminal_in_reach(&self, city: &City, home: Option<&HomeRig>) -> Option<Pos>`. On a stratum it behaves as today. On `Place::Floor` equal to the home's, with `home.rig` on this tile or an 8-neighbour, it returns `Some(home.landing)`. Otherwise `None`.
  - `Player::jack_in(&mut self, city: &City, home: Option<&HomeRig>, sight_bonus: i32) -> Result<Pos, &'static str>`, with the body kept as `(Place::Floor{..}, pos)`.
  - `crate::net::jack_out(st: &mut AppState) -> Result<(), &'static str>`: `Player::jack_out` plus the FOV refresh through `MapRef::for_place` for any body place. `dump_for`, the explore Enter key and `WorldAct::JackOut` all use it.
  - `jack_in_refusal` skips "port locked down" when the player stands on the home floor. Cooling still applies.
  - The jack-in message from home reads `jacked in from home`.

- [ ] **Step 1: Failing tests**
  - `walker`: replace the "nothing reaches the Net from inside a building" assertion. Inside a non-home floor, `jack_in` is `Err("no terminal within reach")`. Beside the home rig it's `Ok(landing)`, with `body == Some((home place, pos))`.
  - `explore`: a new game, step to `bed_side`'s nearest tile within reach of the rig, then Enter → place `Stratum(Net)` at `landing`, log `jacked in from home`. Enter again → back on the home floor at the same tile, and the rig tile is `Visible`.
  - `net`: with `st.net` alert set on the home building, `jack_in_refusal(st)` is `None` beside the rig. A street Jack whose port lies within `footprint.grow(1)` of that building is refused with `port locked down` (find one in the medium seeds or skip if none exists).
  - `net`: `dump_for` from a home jack-in puts the player on the home floor with the rig `Visible`.
- [ ] **Step 2: Run** `cargo test -p strata-zero jack`. Expected: FAIL.
- [ ] **Step 3: Implement**, updating every caller listed by `grep -n "terminal_in_reach\|jack_in(\|jack_out(" crates/strata-zero/src`.
- [ ] **Step 4: Run** `cargo test --workspace`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(app): jack in from the rig at home`

### Task 9: The look-down

**Files:**
- Create: `crates/strata-zero/src/render/atrium.rs`
- Modify: `crates/strata-zero/src/render/mod.rs`, `screens/explore.rs` (overlay list, ~:1922)

**Interfaces:**
- Produces:
  - `pub fn look_down_scale(level: i32) -> f32`: `0.35` at level 1, falling by `0.02` per level, never below `0.12`.
  - `pub struct AtriumOverlay<'a> { pub ground: &'a citygen::Floor, pub level: i32, pub vis: &'a Grid<Visibility> }` implements `Overlay`. For every on-screen cell whose current-map tile is `Terrain::Atrium` and whose `vis` isn't `Unseen`, it draws `cell(ground tile at p, ctx)` with `tick 0` and `light` from `ground.light`. It then scales fg and bg by `look_down_scale(level)` and sets `dim`.
  - Explore pushes it first in the overlay list when the place is `Floor { building, level }` with `level >= 1` and the plan has an `atrium`.

- [ ] **Step 1: Failing tests**
  - In `atrium.rs`: `look_down_scale(1) == 0.35`, `look_down_scale(5) < look_down_scale(2)`, `look_down_scale(60) == 0.12`.
  - A render test: build the home level and level 0 of the fixture and render a `MapView` with the overlay at an origin showing an atrium cell above a plaza `Table`, all visible. The buffer cell shows `○`, its fg is dimmer than the same table drawn at level 0, and an `Unseen` atrium cell is blank black.
  - A screen test: a new game, then teleport to a walkway tile beside the railing. Rendering shows no person glyph on any atrium cell.
- [ ] **Step 2: Run** `cargo test -p strata-zero atrium`. Expected: FAIL.
- [ ] **Step 3: Implement** and wire it in.
- [ ] **Step 4: Run** `cargo test --workspace`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(app): see the plaza through the atrium`

### Task 10: Smoke and docs

**Files:**
- Modify: `README.md` (where a new game starts; megablocks; the home rig)
- Scratch: a pty smoke driver in the session's scratchpad, following the earlier drivers (console `tp`, `gen --json`)

- [ ] **Step 1: Build** `cargo build --release --target-dir target/smoke -p strata-zero`.
- [ ] **Step 2: Drive NEW GAME (DEBUG).** Screenshot the flat on wake-up. Walk out of the front door to the walkway (no transition), then to the railing (screenshot the look-down). Ride the lift to L and screenshot the plaza. Leave by the main door to the street, re-enter, ride back, walk home, jack in at the rig (screenshot the Net landing), jack out (screenshot the flat).
- [ ] **Step 3: Check each screenshot** against the spec's anti-goals: no transition flat → walkway, the look-down is dim and never brighter than the walkway, no people under the atrium, and the exterior on the street doesn't stand out.
- [ ] **Step 4: Update the README** and commit `docs: the megablock and your home`.
