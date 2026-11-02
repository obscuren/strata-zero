# Indoor Lighting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Interiors lose the street lamp; each room is lit in its own tone by ceiling
fittings, wall fittings or both, as its place's seeded mood picks.

**Architecture:** citygen gains a `Feature::Fitting`, a coloured light pass and a room
light table; the floor builder places fittings per walled part after `connect()` and
lights the floor from the emitters it placed. `Floor` carries a per-tile light colour
(`glow`); strata-zero's cell renderer uses it when present and draws the fittings.

**Tech Stack:** Rust workspace (citygen, strata-zero), ratatui cells, serde, RON content.

**Spec:** `docs/superpowers/specs/2026-11-02-indoor-lighting-design.md`

## Global Constraints

- `Feature::Fitting` goes last in `Feature` ("Last, so old saves keep their indices").
- Exact `Rgb` colours only; every new glyph (`▬`, `•`) must pass
  `every_drawn_character_has_a_glyph` and `style_glyphs_are_single_width`.
- Build into `--target-dir target/smoke` (this checkout's own); never `target/`.
- Check both front ends: fmt, clippy `-D warnings` and tests with and without
  `--features strata-zero/window` (CLAUDE.md).
- Rules come from tone and contents (tables, a staffed counter, a bed, an operating
  chair), never from a named building; look words from the fitting's kind and wear.
- Interior light stays cosmetic: `exposure::of` and `update_fov` are not touched.

## Review Focus

1. A floor whose only room is huge and furniture-free (a warehouse hall, a temple):
   still lit, not a single dim fitting. Pinned in Task 4 (`halls_get_more_than_one_fitting`).
2. A wall shared by two rooms: the fitting lights its own side only, and the other room
   doesn't also claim it. Pinned in Task 4 (`wall_fittings_light_their_own_side`).
3. Clinics (template floors with their own `*` marks): lit by the rules, no `Lamp`
   left. Pinned in Task 4 (`no_interior_has_a_street_lamp`, which walks every building
   kind including clinics).
4. Remembered tiles and the story's flat dimming over coloured light: unchanged
   behaviour. Pinned in Task 5 (`remembered_glow_tiles_still_dim`).
5. Strata and constructs: no glow, the grey `lit()` path byte-for-byte as before.
   Pinned in Task 5 (`strata_cells_are_unchanged_without_glow`).

---

### Task 1: The fitting as tile data

**Files:**
- Modify: `crates/citygen/src/tile.rs` (enums, `Feature`, `ALL_FEATURES`, test)
- Modify: `crates/citygen/src/lighting/tuning.rs` (`tone_rgb`)

**Interfaces:**
- Produces (all `Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize`, exported
  from the crate root through `pub use tile::*`):
  - `pub enum FittingKind { Tube, Panel, Bulb, HighBay, Strip, Sconce, NeonStrip, Stage }`
    with `pub fn on_wall(self) -> bool` (Strip, Sconce, NeonStrip) and
    `pub fn wears(self) -> bool` (Tube, HighBay, Strip).
  - `pub enum Tone { Fluorescent, Led, Tungsten, Surgical, Sodium, Neon(Hue), Blacklight }`
  - `pub enum Wear { Ok, Dead, Stutter }`
  - `pub struct Fitting { pub kind: FittingKind, pub tone: Tone, pub wear: Wear }`
  - `Feature::Fitting(Fitting)` (last variant).
  - `lighting::tuning::tone_rgb(t: Tone) -> (u8, u8, u8)` and
    `lighting::tuning::SCREEN: (u8, u8, u8) = (0, 225, 210)`.

- [ ] **Step 1: Write the failing test** in `tile.rs` tests:

```rust
#[test]
fn fittings_are_walked_under_or_sit_in_walls() {
    let f = |kind| Feature::Fitting(Fitting { kind, tone: Tone::Fluorescent, wear: Wear::Ok });
    let mut t = Tile::new(Terrain::Floor);
    for k in [FittingKind::Tube, FittingKind::Panel, FittingKind::Bulb, FittingKind::HighBay, FittingKind::Stage] {
        t.feature = Some(f(k));
        assert!(t.is_walkable(), "{k:?}");
        assert!(!k.on_wall());
    }
    for k in [FittingKind::Strip, FittingKind::Sconce, FittingKind::NeonStrip] {
        assert!(k.on_wall(), "{k:?}");
    }
    assert!(FittingKind::Tube.wears() && !FittingKind::Panel.wears());
}
```
and in `walkability_rules` change `assert_eq!(ALL_FEATURES.len(), 19)` to `21`.

- [ ] **Step 2: Run** `cargo test -p citygen --target-dir target/smoke fittings_are_walked` — FAIL (no `Fitting`).
- [ ] **Step 3: Implement** the types and `Feature::Fitting`; append to `ALL_FEATURES` a
  fluorescent `Tube` and a tungsten `Bulb` (`[Feature; 21]`). `tone_rgb` values:
  Fluorescent (185, 232, 222), Led (175, 205, 255), Tungsten (255, 168, 88), Surgical
  (228, 242, 255), Sodium (255, 150, 55), Blacklight (175, 95, 255); Neon by hue: Cyan
  (0, 225, 210), Magenta (255, 60, 175), Pink (255, 110, 200), Orange (255, 140, 40),
  Yellow (255, 220, 60), Green (60, 255, 120), Blue (60, 120, 255), Red (255, 50, 60),
  White (240, 240, 255). Fix any exhaustive `match` on `Feature` the compiler flags
  (strata-zero's `glyph_for`/`feature_color` get a placeholder arm here, replaced in Task 5).
- [ ] **Step 4: Run** `cargo test -p citygen --target-dir target/smoke` — PASS;
  `cargo check --workspace --target-dir target/smoke` — clean.
- [ ] **Step 5: Commit** `citygen: fittings as tile features`.

### Task 2: Coloured light

**Files:**
- Modify: `crates/citygen/src/lighting.rs`

**Interfaces:**
- Produces:
  - `pub struct Emitter { pub pos: Pos, pub radius: i32, pub strength: f32, pub colour: (u8, u8, u8) }`
  - `pub fn compute_coloured(size: Size, base: impl Fn(Pos) -> [f32; 3], blocks: impl Fn(Pos) -> bool, emitters: &[Emitter]) -> Grid<(u8, u8, u8)>`
    Same cone as `compute` (`crate::fov::compute`, falloff `strength * (1 - d / r)`,
    blocking tiles take the light), each channel `+= falloff * colour / 255`, clamped 0..=255.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn coloured_light_carries_its_colour_and_stops_at_walls() {
    let size = Size { w: 20, h: 10 };
    let walls: Vec<Pos> = (0..10).map(|y| Pos::new(12, y)).collect();
    let e = Emitter { pos: Pos::new(10, 5), radius: 6, strength: 200.0, colour: (255, 168, 88) };
    let g = compute_coloured(size, |_| [10.0, 10.0, 10.0], |p| walls.contains(&p), &[e]);
    let near = g[Pos::new(10, 5)];
    assert!(near.0 > near.2, "warm: {near:?}");
    assert_eq!(g[Pos::new(0, 0)], (10, 10, 10));
    assert!(g[Pos::new(12, 5)].0 > 10, "the wall takes the light");
    assert_eq!(g[Pos::new(13, 5)], (10, 10, 10), "nothing behind the wall");
}
```

- [ ] **Step 2: Run** `cargo test -p citygen --target-dir target/smoke coloured_light` — FAIL.
- [ ] **Step 3: Implement** `Emitter` and `compute_coloured` beside `compute`.
- [ ] **Step 4: Run** the test — PASS.
- [ ] **Step 5: Commit** `citygen: coloured light from a list of emitters`.

### Task 3: Each room's tone and mood

**Files:**
- Modify: `crates/citygen/src/lighting.rs` (`Mood`, `room_mood`)
- Modify: `crates/citygen/src/lighting/tuning.rs` (table and numbers)

**Interfaces:**
- Consumes: `Tone` (Task 1), `RoomKind`, `BuildingKind`, `BuildingId`, `rng::hash64`.
- Produces:
  - `pub enum Mood { Ceiling, Walls, Both }` (in `lighting`), with
    `pub fn ceiling(self) -> bool`, `pub fn walls(self) -> bool`.
  - `pub struct RoomLight { pub tone: Tone, pub level: u8, pub moods: &'static [(Mood, u32)] }`
  - `tuning::room_light(room: Option<RoomKind>, building: BuildingKind) -> RoomLight`
    (the spec's table; `None` and unlisted kinds take the Corridor row; Club: Bar and
    Vip take `Blacklight`; Temple: Hall takes Tungsten, level 50, `[(Walls, 1)]`).
  - `tuning::mood_factor(m: Mood) -> f32` (Walls 0.75, Ceiling 1.0, Both 1.15).
  - `lighting::room_mood(seed: u64, building: BuildingId, room: Option<RoomKind>, number: u16, moods: &[(Mood, u32)]) -> Mood`
    — weighted pick from `hash64(hash64(hash64(seed, building.index() as u64), kind index or 255), number)`.
  - Emitter numbers as `(i32, f32)` consts in tuning: `TUBE (5, 200.0)`,
    `TUBE_STUTTER (3, 60.0)`, `HIGH_BAY (6, 220.0)`, `BULB (5, 190.0)`,
    `PENDANT (3, 150.0)`, `COUNTER_GLOW (2, 55.0)`, `SURGICAL_LAMP (3, 210.0)`,
    `STRIP (3, 105.0)`, `STRIP_SURGICAL (3, 130.0)`, `SCONCE (5, 190.0)`,
    `SCONCE_ROW (3, 95.0)`, `NEON_STRIP (4, 110.0)`, `STAGE (3, 60.0)`, `SCREEN_GLOW (3, 90.0)`;
    wear `WEAR_IN: u64 = 9` (hash % 9: 0 dead, 1 stutter).
  - Remove `INTERIOR_BASE` and `INTERIOR_LAMP` in Task 4 when their last user goes.

- [ ] **Step 1: Write the failing tests** in `lighting.rs` tests:

```rust
#[test]
fn a_mood_is_the_same_for_the_same_place_and_varies_across_places() {
    let moods = tuning::room_light(Some(RoomKind::Bar), BuildingKind::Bar).moods;
    let pick = |b: usize| room_mood(8, BuildingId::new(b), Some(RoomKind::Bar), 0, moods);
    assert_eq!(pick(3), pick(3));
    let seen: std::collections::HashSet<_> = (0..40).map(pick).collect();
    assert!(seen.contains(&Mood::Walls) && seen.contains(&Mood::Both));
    assert!(!seen.contains(&Mood::Ceiling), "a bar never goes ceiling-only");
}

#[test]
fn a_club_bar_is_blacklight_and_a_temple_hall_candlelit() {
    assert_eq!(tuning::room_light(Some(RoomKind::Bar), BuildingKind::Club).tone, Tone::Blacklight);
    let hall = tuning::room_light(Some(RoomKind::Hall), BuildingKind::Temple);
    assert_eq!((hall.tone, hall.moods), (Tone::Tungsten, &[(Mood::Walls, 1)][..]));
    assert_eq!(tuning::room_light(Some(RoomKind::Hall), BuildingKind::Warehouse).tone, Tone::Sodium);
    assert_eq!(tuning::room_light(None, BuildingKind::Hotel).tone, Tone::Fluorescent);
}
```
(Use the crate's real `BuildingId` constructor; adjust the call if it is not `new`.)

- [ ] **Step 2: Run** `cargo test -p citygen --target-dir target/smoke mood` — FAIL.
- [ ] **Step 3: Implement** the table, `mood_factor`, `room_mood`, the consts.
- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit** `citygen: a tone and a mood per room`.

### Task 4: Placing fittings and lighting the floor

**Files:**
- Create: `crates/citygen/src/interior/layouts/lights.rs` (`impl Builder<'_> { pub(super) fn light(&mut self) }` and its tests)
- Modify: `crates/citygen/src/interior/layouts.rs` (Builder gains `seed: u64` and
  `emitters: Vec<lighting::Emitter>`; `mod lights;`; delete `lamps()`; stage tiles get
  `Stage` fittings; `generate_floor` calls `b.light()` after `b.connect()` for every
  floor; `finish()` lights from the emitters; the exits probe `Floor` gets a `glow`)
- Modify: `crates/citygen/src/interior/layouts/clinics.rs` (`b'*'` → no feature)
- Modify: `crates/citygen/src/interior/floor.rs` (`pub glow: Grid<(u8, u8, u8)>` on `Floor`)
- Modify: `crates/citygen/src/interior/mod.rs` (ASCII dump: a non-wall fitting `*`, wall
  fittings stay `#`)
- Modify: `crates/citygen/src/lighting/tuning.rs` (drop `INTERIOR_BASE`, `INTERIOR_LAMP`)

**Interfaces:**
- Consumes: Tasks 1-3.
- Produces: `Floor::glow`; `Floor::light[p]` = the brightest channel of `glow[p]`.

How `light()` works (the spec's "Placing fittings", in this order):
1. **Owner** of each tile: the room with the smallest rect area containing it, else `None`.
2. **Parts:** `crate::pathing::components` over tiles whose terrain is `Floor`, `Grate` or a
   connector, joined only within one owner (doors, walls, windows, atriums and railings
   cut parts). Iterate parts in order of their first tile, so placement is deterministic.
3. Per part: `room_light(owner kind, plan.kind)`, `room_mood(city.seed, plan.building,
   kind, room.number, moods)` (number 0 for public rooms and for `None`).
   Contents: `bar counter` = a `Counter` tile with a 4-neighbour `Role::Bar` station;
   tables, beds, operating chair by `Furniture`.
4. **Free** for a ceiling fitting: `self.open(p)` and not a station.
   **Part walls:** 4-neighbours of the part's tiles with terrain `Wall` and no feature,
   each mapped to its inward part tile; sorted by `(y, x)`; `run` = the wall tile's x when
   the inward tile is above or below it, else its y.
5. Ceiling and wall rules, stage, wear (`hash64(self.seed, first tile index) % WEAR_IN`
   per tube; per strip `(line, run / 3)` group), emitters exactly as the spec lists,
   wall-fitting emitters at the inward tile, plus `SCREEN_GLOW` at every `Terminal`.
6. `finish()`: ambient per part tile = `tone_rgb(tone) × level × mood_factor ÷ 255`;
   non-part tiles take the per-channel max of their 8-neighbour part tiles' ambient, else
   the `None` (corridor) ambient; `glow = compute_coloured(size, base, blocks_sight, &emitters)`.

- [ ] **Step 1: Write the failing tests** in `lights.rs` (a `city()` like
  `interior/mod.rs` tests' seed-6 120x60 city, plus a `find(kind)` that returns the first
  building of a kind with a plan; for the start flat use the Medium seed-8 city and
  `HomeRig`'s building and level via `crate::interior::home` helpers, or the `Home` room):

```rust
#[test]
fn no_interior_has_a_street_lamp() // every closed building kind present in the city,
    // every level: no Feature::Lamp; ceiling and Stage fittings on walkable Floor tiles
    // (and is_walkable()); wall fittings on Wall tiles; validate_floor still Ok.

#[test]
fn fittings_keep_off_doorsteps_and_furniture() // no fitting on an exit's doorstep,
    // on a door or window, or on a tile with furniture.

#[test]
fn wall_fittings_light_their_own_side() // for every wall fitting on a hotel floor:
    // the tile on the far side of its wall (the inward tile mirrored) gets no more
    // light than its own part's ambient plus other emitters it can see — test as:
    // regenerate with only that fitting's emitter and check the far tile's glow equals
    // its ambient. Expose `pub(super) fn ambient_of(&self, p) -> [f32; 3]` for it.

#[test]
fn a_place_keeps_its_mood_on_every_floor_and_every_visit() // hotel levels 1 and 2:
    // the corridor fittings' kinds as a set are equal; generate_floor twice: equal Floors.

#[test]
fn bars_come_dark_and_lit() // across the city's Bar buildings (seeds 6 and 8 if one
    // city has too few): some ground floor has NeonStrip and no lit tables, another has
    // NeonStrip and every table tile's glow red channel > the bar's ambient red.

#[test]
fn the_flat_is_not_a_bar() // the start flat (Medium seed 8): no NeonStrip in its Home
    // room; every part of the Home with 4+ tiles has a fitting in or on its walls.

#[test]
fn halls_get_more_than_one_fitting() // a Warehouse ground floor and the Temple laid on a
    // bar's plan (as `interior/mod.rs` does): 2+ fittings in or on the hall.

#[test]
fn surgery_chair_is_the_brightest_and_corridors_are_cooler_than_flats() // clinic:
    // the chair tile's light >= every other surgery floor tile's; a megablock floor:
    // mean corridor glow b/r > mean flat glow b/r.
```

- [ ] **Step 2: Run** `cargo test -p citygen --target-dir target/smoke lights::` — FAIL.
- [ ] **Step 3: Implement** `light()`, the Builder fields, `finish()`, stage, clinics, the
  dump, `Floor::glow`; remove `lamps()`, `INTERIOR_BASE`, `INTERIOR_LAMP`.
- [ ] **Step 4: Run** `cargo test -p citygen --target-dir target/smoke` — all PASS (fix
  older interior tests only where they asserted the removed lamp grid).
- [ ] **Step 5: Commit** `citygen: interiors lit by fittings in the room's tone`.

### Task 5: Drawing coloured light and fittings

**Files:**
- Modify: `crates/strata-zero/src/render/mod.rs` (`RenderCtx.glow: Option<Rgb>`)
- Modify: `crates/strata-zero/src/render/map.rs` (`TileMap::glow`, `FloorMap::glow`, `MapRef::glow`)
- Modify: `crates/strata-zero/src/render/viewport.rs`, `render/atrium.rs`, `render/mark.rs` (pass the glow)
- Modify: `crates/strata-zero/src/render/tileset.rs` (glyphs, colours, `cell()`, tests)
- Modify: `crates/strata-zero/src/render/lighting.rs` (`lit_by(c: Rgb, glow: Rgb, fg: bool) -> Rgb`)

**Interfaces:**
- Consumes: `Floor::glow`, `Fitting` (Tasks 1, 4), `anim::effects::flicker`.
- Produces: `TileMap::glow(&self, p: Pos) -> Option<Rgb>` (default `None`).

Values (spec "Drawing"): background `c × (0.22 + 0.78·L/255) + L × 0.10`; non-emitting
foreground `c × (0.30 + 0.70·L/255) + L × 0.16`; fitting colour = `tone_rgb` mixed toward
white by 0.6 (Neon and Blacklight by 0.2), the bulb `#ffd9a0`; `▬` for Tube, Panel,
HighBay; `•` for Bulb (bold); wall fittings and Stage keep the terrain's glyph; wall
fitting background = colour × 0.22; Stage background = `bg.mix(colour × 0.35, 0.45)`;
Dead `#3e4750`, not emitting; Stutter = colour × `flicker(tick, variant, true)`; furniture
whose glow has a channel ≥ 180: foreground `mix(glow scaled to max 255, 0.7)`.

- [ ] **Step 1: Write the failing tests** in `tileset.rs` tests:

```rust
#[test]
fn a_warm_glow_warms_the_floor_and_a_cold_one_cools_it() // a Floor tile, glow
    // Some(Rgb(200,130,70)) → bg.0 > bg.2; glow Some(Rgb(140,180,170)) → bg.2 >= bg.0.

#[test]
fn strata_cells_are_unchanged_without_glow() // glow None: cell equals today's
    // expected values for a wall and a lit lamp (copy from cell_mapping_is_pure_and_lit).

#[test]
fn fittings_draw_as_tubes_bulbs_and_lit_walls() // Tube → '▬', Bulb → '•',
    // Strip on a Wall keeps glyph_for(Wall, None, ..) and its fg is the fitting colour;
    // Dead tube fg == Rgb(0x3e,0x47,0x50) and its bg is lit like the floor.

#[test]
fn furniture_in_strong_light_takes_its_colour() // Table with glow Rgb(230,160,90):
    // fg.0 > fg.2 + 60; with glow Rgb(90,60,40): fg unchanged by the rule.

#[test]
fn remembered_glow_tiles_still_dim() // apply_vis(cell(..glow..), Remembered) scales as before.
```

- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke tileset::` — FAIL.
- [ ] **Step 3: Implement**; every `RenderCtx { .. }` literal gets `glow` (`None` in tests
  and strata; `map.glow(p)` in viewport and marks; the ground floor's glow in atrium).
- [ ] **Step 4: Run** `cargo test -p strata-zero --target-dir target/smoke` — PASS.
- [ ] **Step 5: Commit** `render: coloured interior light and its fittings`.

### Task 6: Look names and flavour

**Files:**
- Modify: `crates/strata-zero/src/look.rs` (`feature_name`)
- Modify: `flavor/looks.ron` (`feature` keys)

**Interfaces:**
- Produces: `feature_name(Feature::Fitting(f))` → names and keys from the spec ("Fluorescent
  tube" `tube`, "LED panel" `panel`, "Bulb" `bulb`, "High-bay lamp" `high_bay`, "Light
  strip" `strip`, "Sconce" `sconce`, "Neon strip" `neon_strip`, "Stage lights"
  `stage_lights`; for `wears()` kinds: "Dead …" `<key>_dead`, "Flickering …" `<key>_flicker`).
  Every key gets two lines in `looks.ron`, so no fallback is needed.

- [ ] **Step 1: Write the failing test** in `look.rs` tests:

```rust
#[test]
fn every_fitting_has_a_name_and_flavour() // for every FittingKind × Wear (Ok only for
    // kinds that don't wear): feature_name is Some, the key has lines in Looks::load().
```

- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke look::` — FAIL.
- [ ] **Step 3: Implement** the names and write the flavour (voice of `looks.ron`: one or
  two short sentences, no numbers, never "You see").
- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit** `look: names and flavour for indoor fittings`.

### Task 7: Shots and both front ends

**Files:**
- Modify: `crates/strata-zero/src/screens/shots.rs` (ignored `indoor_lighting`)

- [ ] **Step 1: Add** `indoor_lighting`: the six mock interiors (start flat on 18F via
  `state_home`; `clinic::testutil::inside()`; the first Bar and Club at level 0, Office
  and Hotel at level 2 with the player on the floor's middle walkable tile), each
  `write_svg(dir, "indoor_<name>", st, &draw_stack(st, &[&map], (120, 40)), false)`.
- [ ] **Step 2: Run** `SHOT_DIR=<scratch> cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored indoor_lighting`
  and compare each PNG with the mock's A + B frames.
- [ ] **Step 3: Run the checks** (all must be clean):

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --target-dir target/smoke -- -D warnings
cargo clippy --workspace --all-targets --features strata-zero/window --target-dir target/smoke -- -D warnings
cargo test --workspace --target-dir target/smoke
cargo test --workspace --features strata-zero/window --target-dir target/smoke
```

- [ ] **Step 4: Commit** `shots: interiors under their own light`.
