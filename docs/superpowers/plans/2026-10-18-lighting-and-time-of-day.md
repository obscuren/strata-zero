# Lighting and Time of Day Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Light decides how far you see: lamps and neon pull their pools into view across the dark, the sunlit strata get a real day with lamps off and a sky tint, and the city's people see you by the same rule, reported on the HUD as exposure.

**Architecture:** Light becomes a pure function in `citygen`: a clock ramp, an ambient table per stratum, and two emitter grids per stratum (steady fixtures and lamps) baked once with the shadowcaster so walls occlude. The map carries the minute of day, so every light read is `ambient + emitted`. One `sight` predicate in `citygen` serves both the player's FOV and the sim's walkers; the app computes `Exposure` from it each frame.

**Tech Stack:** Rust 2024 workspace (`citygen`, `sim`, `game`, `strata-zero`), ratatui, postcard saves. Run everything with `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`.

**Spec:** `docs/superpowers/specs/2026-10-18-lighting-and-time-of-day-design.md`

## Global Constraints

- Every number in the design is a named constant in `crates/citygen/src/lighting/tuning.rs` with a doc comment. No other file carries one of these literals.
- Night values equal today's: ambient Undercity 20, Street 60, Skyways 80, Heights 110, Net 0; radii Street 14, Undercity 8, Net 24, interiors 30. A city at 22:00 renders as it does now.
- Ramps: dawn 05:00 to 07:00, dusk 17:00 to 19:00. Lamps on while `daylight < 0.5`.
- `LIT_SIGHT` 40, `LIT_RADIUS` 40, `DAY_RADIUS` 60.
- Exposure bands: `dark` below 40, `dim` below 100, `lit` below 180, `bright` otherwise; zero eyes reads `unseen`.
- The Net and interiors keep the plain radius rule. Floors and constructs keep one baked `light` grid.
- Save `FORMAT_VERSION` becomes 4. Older files produce the existing "regenerate it from seed" error.
- Commit after every task with a message in the repo's style: `feat(citygen): ...`, `feat(app): ...`, lower case, a sentence.

## Review Focus

1. A map built before a world exists (title, generating screen, tests) must render at 22:00, not panic or render day. Pinned in Task 4 (`stratum_of` builds at `START_MINUTE_OF_DAY`).
2. A lamp standing on a wall-adjacent tile must light the wall and nothing behind it; a window must not block. Pinned in Task 3.
3. At noon on the Street, `update_fov` must not blow past the map edge or take noticeable time; a cast at radius 60 over a 320x160 city is bounded by `in_bounds`. Pinned in Task 6 (noon test on the small city).
4. `exposure()` off the strata (a floor, a construct, no world) returns dark and empty rather than panicking. Pinned in Task 8.
5. The HUD segment must never push the clock off the line at narrow widths; the location is cut first. Pinned in Task 8.

---

### Task 1: Move the shadowcaster into citygen

**Files:**
- Create: `crates/citygen/src/fov.rs` (moved from `crates/strata-zero/src/walker/fov.rs`)
- Modify: `crates/strata-zero/src/walker/fov.rs` (becomes a re-export)
- Modify: `crates/citygen/src/lib.rs`

**Interfaces:**
- Produces: `citygen::fov::compute(origin: Pos, radius: i32, blocks: impl Fn(Pos) -> bool, in_bounds: impl Fn(Pos) -> bool, mark: impl FnMut(Pos))`, unchanged.

- [ ] **Step 1: Move the file**

```bash
git mv crates/strata-zero/src/walker/fov.rs crates/citygen/src/fov.rs
```

- [ ] **Step 2: Fix the import inside the moved file**

In `crates/citygen/src/fov.rs` replace `use citygen::Pos;` with `use crate::geom::Pos;` (both at the top and in the `tests` module, where `use citygen::Pos;` becomes `use crate::geom::Pos;`).

- [ ] **Step 3: Export it and re-export it**

`crates/citygen/src/lib.rs`: add `pub mod fov;` after `pub mod flavor;`.

Create `crates/strata-zero/src/walker/fov.rs`:

```rust
//! The field of view lives in citygen so the generator and the sim share it.

pub use citygen::fov::*;
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --workspace fov`
Expected: `fov_symmetric_and_walls_block` passes under `citygen`; `strata-zero` compiles.

- [ ] **Step 5: Commit**

```bash
git add -A crates/citygen/src/fov.rs crates/citygen/src/lib.rs crates/strata-zero/src/walker/fov.rs
git commit -m "refactor(citygen): the shadowcaster moves into citygen"
```

---

### Task 2: The tuning block, the clock ramp, ambient and the sky

**Files:**
- Create: `crates/citygen/src/lighting/tuning.rs`
- Modify: `crates/citygen/src/lighting.rs`

**Interfaces:**
- Produces, in `citygen::lighting`:
  - `pub mod tuning` with the constants below.
  - `pub fn daylight(minute: u16) -> f32`
  - `pub fn lamps_on(minute: u16) -> bool`
  - `pub fn sunlit(k: StratumKind) -> bool`
  - `pub fn ambient(k: StratumKind, minute: u16) -> u8`
  - `pub fn sky(k: StratumKind, minute: u16) -> Option<((u8, u8, u8), f32)>`
  - `pub fn emits(terrain: Terrain, feature: Option<Feature>, k: StratumKind, lamps_on: bool) -> bool`

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module in `crates/citygen/src/lighting.rs`:

```rust
    use crate::world::StratumKind;

    #[test]
    fn the_ramp_is_night_at_ten_noon_at_twelve_and_half_at_the_edges() {
        assert_eq!(daylight(22 * 60), 0.0);
        assert_eq!(daylight(12 * 60), 1.0);
        assert!((daylight(6 * 60) - 0.5).abs() < 1e-6);
        assert!((daylight(18 * 60) - 0.5).abs() < 1e-6);
        assert!(daylight(5 * 60 + 30) > 0.0 && daylight(5 * 60 + 30) < 0.5);
        assert!(lamps_on(5 * 60 + 59) && !lamps_on(6 * 60 + 1));
        assert!(lamps_on(18 * 60 + 1) && !lamps_on(17 * 60 + 59));
    }

    #[test]
    fn ambient_at_night_is_the_old_base_and_noon_is_full_sun_above_ground() {
        for (k, base) in [
            (StratumKind::Undercity, 20),
            (StratumKind::Street, 60),
            (StratumKind::Skyways, 80),
            (StratumKind::Heights, 110),
            (StratumKind::Net, 0),
        ] {
            assert_eq!(ambient(k, 22 * 60), base, "{k:?}");
        }
        assert_eq!(ambient(StratumKind::Street, 12 * 60), 255);
        assert_eq!(ambient(StratumKind::Heights, 12 * 60), 255);
        assert_eq!(ambient(StratumKind::Undercity, 12 * 60), 20);
        assert_eq!(ambient(StratumKind::Net, 12 * 60), 0);
        assert!(sunlit(StratumKind::Street) && !sunlit(StratumKind::Undercity));
    }

    #[test]
    fn the_sky_tints_the_day_and_the_dusk_but_not_the_night_or_the_undercity() {
        assert_eq!(sky(StratumKind::Street, 22 * 60), None);
        assert_eq!(sky(StratumKind::Undercity, 12 * 60), None);
        let (day, w) = sky(StratumKind::Street, 12 * 60).unwrap();
        assert_eq!((day, w), (tuning::SKY_DAY, tuning::SKY_DAY_WEIGHT));
        let (dusk, wd) = sky(StratumKind::Street, 18 * 60).unwrap();
        assert!(dusk.0 > dusk.2, "dusk is warm: {dusk:?}");
        assert!(wd > w, "dusk weighs more than noon: {wd} vs {w}");
    }

    #[test]
    fn lamps_emit_only_while_on_and_neon_always() {
        let k = StratumKind::Street;
        assert!(emits(Terrain::Floor, Some(Feature::Lamp), k, true));
        assert!(!emits(Terrain::Floor, Some(Feature::Lamp), k, false));
        assert!(emits(Terrain::Floor, Some(Feature::Neon(crate::world::Hue::Cyan)), k, false));
        assert!(emits(Terrain::Window, None, k, false));
        assert!(!emits(Terrain::Window, None, StratumKind::Heights, false));
        assert!(!emits(Terrain::Floor, None, k, true));
    }
```

(If `Hue::Cyan` does not exist, use the first variant of `Hue` in `crates/citygen/src/world.rs`.)

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p citygen lighting`
Expected: compile errors, `daylight` not found.

- [ ] **Step 3: Write the tuning block**

Create `crates/citygen/src/lighting/tuning.rs`:

```rust
//! Every number behind light and sight, in one place. Tweak here, look with the dev
//! console's `clock hh:mm` and `light`.

use crate::tile::{Feature, Terrain};
use crate::world::StratumKind;

/// Dawn ramps from 05:00 to 07:00; the midpoint 06:00 is when lamps go off.
pub const DAWN_START: u16 = 5 * 60;
pub const DAWN_END: u16 = 7 * 60;
/// Dusk ramps from 17:00 to 19:00; the midpoint 18:00 is when lamps come on.
pub const DUSK_START: u16 = 17 * 60;
pub const DUSK_END: u16 = 19 * 60;

/// Ambient light per stratum as (night, day). The Undercity and the Net have no sky.
pub const fn ambient_range(k: StratumKind) -> (u8, u8) {
    match k {
        StratumKind::Undercity => (20, 20),
        StratumKind::Street => (60, 255),
        StratumKind::Skyways => (80, 255),
        StratumKind::Heights => (110, 255),
        StratumKind::Net => (0, 0),
    }
}

/// Fixtures lit all day: (radius, strength at the centre, fading linearly).
pub fn steady_emission(terrain: Terrain, feature: Option<Feature>, k: StratumKind) -> Option<(i32, f32)> {
    match feature {
        Some(Feature::Neon(_)) => Some((4, 90.0)),
        Some(Feature::HoloAd) => Some((5, 100.0)),
        Some(Feature::Helipad) => Some((3, 60.0)),
        _ if terrain == Terrain::Window && k == StratumKind::Street => Some((2, 30.0)),
        _ => None,
    }
}

/// Fixtures that switch off by day: (radius, strength).
pub fn lamp_emission(feature: Option<Feature>) -> Option<(i32, f32)> {
    match feature {
        Some(Feature::Lamp) => Some((6, 120.0)),
        _ => None,
    }
}

/// A lamp on an interior floor.
pub const INTERIOR_LAMP: (i32, f32) = (6, 120.0);
/// Base light of an interior floor; floors have no sky.
pub const INTERIOR_BASE: u8 = 150;
/// The Net's open grid and its lanes, written straight into `steady`.
pub const NET_GRID_LIGHT: u8 = 200;
pub const NET_LANE_LIGHT: u8 = 255;

/// Emitted light (ambient excluded) at or above which a tile in line of sight is seen
/// past the dark radius. A street lamp passes within four tiles, neon within two.
pub const LIT_SIGHT: u8 = 40;
/// How far the player's cast reaches when looking for lit tiles.
pub const LIT_RADIUS: i32 = 40;
/// The dark radius at full day on a sunlit stratum: past the viewport's half-diagonal.
pub const DAY_RADIUS: i32 = 60;

/// Exposure words by the light on your tile: below each bound, in order.
pub const EXPOSURE_BANDS: [(u8, &str); 4] = [(40, "dark"), (100, "dim"), (180, "lit"), (255, "bright")];

/// The sky at full day: a pale cool cast, and how much of it.
pub const SKY_DAY: (u8, u8, u8) = (200, 215, 235);
pub const SKY_DAY_WEIGHT: f32 = 0.18;
/// The sky at the middle of dawn and dusk: amber, and how much of it at the peak.
pub const SKY_DUSK: (u8, u8, u8) = (235, 150, 80);
pub const SKY_DUSK_WEIGHT: f32 = 0.25;
```

- [ ] **Step 4: Write the functions**

In `crates/citygen/src/lighting.rs`, below the `use` lines add `pub mod tuning;` and:

```rust
use crate::world::StratumKind;
use tuning::*;

/// How much sun there is at `minute` of the day: 0 at night, 1 by day, linear ramps
/// through dawn and dusk.
pub fn daylight(minute: u16) -> f32 {
    let m = minute % (24 * 60);
    let ramp = |from: u16, to: u16| (m - from) as f32 / (to - from) as f32;
    if m < DAWN_START || m >= DUSK_END {
        0.0
    } else if m < DAWN_END {
        ramp(DAWN_START, DAWN_END)
    } else if m < DUSK_START {
        1.0
    } else {
        1.0 - ramp(DUSK_START, DUSK_END)
    }
}

/// Street lamps burn while the sun is down past the middle of the ramp.
pub fn lamps_on(minute: u16) -> bool {
    daylight(minute) < 0.5
}

/// Whether a stratum sees the sky.
pub fn sunlit(k: StratumKind) -> bool {
    let (night, day) = ambient_range(k);
    day > night
}

/// Ambient light on a stratum at `minute`, between its night and day values.
pub fn ambient(k: StratumKind, minute: u16) -> u8 {
    let (night, day) = ambient_range(k);
    (night as f32 + (day as f32 - night as f32) * daylight(minute)).round() as u8
}

/// The sky's cast on a sunlit stratum: amber through the ramps, pale by day, none at
/// night. Returns the colour and how far to mix toward it.
pub fn sky(k: StratumKind, minute: u16) -> Option<((u8, u8, u8), f32)> {
    if !sunlit(k) {
        return None;
    }
    let t = daylight(minute);
    if t <= 0.0 {
        return None;
    }
    let tent = 1.0 - (2.0 * t - 1.0).abs();
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    let colour = (mix(SKY_DUSK.0, SKY_DAY.0), mix(SKY_DUSK.1, SKY_DAY.1), mix(SKY_DUSK.2, SKY_DAY.2));
    Some((colour, SKY_DUSK_WEIGHT * tent + SKY_DAY_WEIGHT * t))
}

/// Whether a tile glows on its own right now: a steady fixture, or a lamp while on.
pub fn emits(terrain: Terrain, feature: Option<Feature>, k: StratumKind, lamps_on: bool) -> bool {
    steady_emission(terrain, feature, k).is_some() || (lamps_on && lamp_emission(feature).is_some())
}
```

Add `use crate::tile::{Feature, Terrain};` to the top-level imports (the `tests` module already imports them; keep its import).

- [ ] **Step 5: Run the tests**

Run: `cargo test -p citygen lighting`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/citygen/src/lighting.rs crates/citygen/src/lighting/tuning.rs
git commit -m "feat(citygen): the clock ramp, ambient by stratum and the sky, with every number in one tuning block"
```

---

### Task 3: Occlusion in the bake

**Files:**
- Modify: `crates/citygen/src/lighting.rs` (`compute`)
- Modify: `crates/citygen/src/generate/dressing.rs:277-281`
- Modify: `crates/citygen/src/interior/layouts.rs:19,1194-1198`

**Interfaces:**
- Produces: `lighting::compute(tiles: &Grid<Tile>, base: u8, blocks: impl Fn(Pos) -> bool, emission: impl Fn(&Tile) -> Option<(i32, f32)>) -> Grid<u8>`.

- [ ] **Step 1: Write the failing test**

In the `tests` module of `crates/citygen/src/lighting.rs`:

```rust
    #[test]
    fn a_wall_takes_the_light_and_shades_what_stands_behind_it() {
        let mut tiles = Grid::filled(Size { w: 20, h: 10 }, Tile::new(Terrain::Floor));
        tiles[Pos::new(10, 5)].feature = Some(Feature::Lamp);
        for y in 0..10 {
            tiles[Pos::new(12, y)].terrain = Terrain::Wall;
        }
        tiles[Pos::new(12, 5)].terrain = Terrain::Window;
        let light = compute(&tiles, 0, |p| tiles[p].blocks_sight(), |t| match t.feature {
            Some(Feature::Lamp) => Some((6, 120.0)),
            _ => None,
        });
        assert!(light[Pos::new(12, 3)] > 0, "the wall glows");
        assert_eq!(light[Pos::new(13, 3)], 0, "nothing behind the wall");
        assert!(light[Pos::new(13, 5)] > 0, "the window lets it through");
    }
```

Update the existing lamp test's `compute(&tiles, 40, |t| ...)` call to `compute(&tiles, 40, |_| false, |t| ...)`.

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p citygen lighting`
Expected: compile error, `compute` takes 3 arguments.

- [ ] **Step 3: Cast from each emitter**

Replace `compute` in `crates/citygen/src/lighting.rs`:

```rust
/// Adds every emitter's cone of `strength` over `radius` tiles to `base`, clamped to
/// 0..=255, reaching only the tiles the emitter can see through `blocks`. A blocking
/// tile takes the light that hits it, so walls and facades glow. `emission` gives
/// (radius, strength) for a tile that glows.
pub fn compute(
    tiles: &Grid<Tile>,
    base: u8,
    blocks: impl Fn(Pos) -> bool,
    emission: impl Fn(&Tile) -> Option<(i32, f32)>,
) -> Grid<u8> {
    let size = tiles.size();
    let mut light = vec![base as f32; size.area()];
    for (p, t) in tiles.iter() {
        let Some((r, strength)) = emission(t) else {
            continue;
        };
        crate::fov::compute(p, r, &blocks, |q| size.contains(q), |q| {
            let d = (p.dist2(q) as f32).sqrt();
            if d <= r as f32 {
                light[size.index(q)] += strength * (1.0 - d / r as f32);
            }
        });
    }
    let mut out = Grid::filled(size, 0u8);
    for (slot, v) in out.as_mut_slice().iter_mut().zip(light) {
        *slot = v.clamp(0.0, 255.0) as u8;
    }
    out
}
```

Add `use crate::geom::Pos;` at the top (drop `Rect` if it becomes unused).

- [ ] **Step 4: Pass `blocks` from both callers**

`crates/citygen/src/generate/dressing.rs:280`:

```rust
        st.light = crate::lighting::compute(
            &st.tiles,
            base,
            |p| st.tiles.get(p).is_none_or(|t| t.blocks_sight()),
            |t| emission(t.terrain, t.feature, k),
        );
```

(The borrow of `st.tiles` inside the closure while assigning `st.light`: bind `let tiles = &st.tiles;` first, compute into a local, then assign.)

`crates/citygen/src/interior/layouts.rs:1195`:

```rust
        let tiles = &self.tiles;
        let light = lighting::compute(
            tiles,
            lighting::tuning::INTERIOR_BASE,
            |p| tiles.get(p).is_none_or(|t| t.blocks_sight()),
            |t| match t.feature {
                Some(Feature::Lamp) => Some(lighting::tuning::INTERIOR_LAMP),
                _ => None,
            },
        );
```

Delete `const BASE_LIGHT: u8 = 150;` at `layouts.rs:19` and fix any other use of it with `lighting::tuning::INTERIOR_BASE`.

- [ ] **Step 5: Run the citygen tests**

Run: `cargo test -p citygen`
Expected: all pass, including `dressing_places_features_and_lights` and the interior lobby light test, since lamps still light their own tile and the floor around them.

- [ ] **Step 6: Commit**

```bash
git add crates/citygen/src/lighting.rs crates/citygen/src/generate/dressing.rs crates/citygen/src/interior/layouts.rs
git commit -m "feat(citygen): light stops at walls; the bake casts from each emitter"
```

---

### Task 4: Two emitter grids per stratum, and the map knows the minute

**Files:**
- Modify: `crates/citygen/src/world.rs:88-105` (`Stratum`)
- Modify: `crates/citygen/src/generate/dressing.rs:31-52,277-281,353-364`
- Modify: `crates/citygen/src/generate/cyberspace.rs:15-18,44-53,137,338,461`
- Modify: `crates/citygen/src/construct.rs:25,147-155`
- Modify: `crates/strata-zero/src/save.rs:12-14,108-115`
- Modify: `crates/strata-zero/src/render/map.rs:34-38,103-115,179-184`
- Modify: `crates/strata-zero/src/render/weather.rs:77`
- Modify: `crates/strata-zero/src/ui/hud.rs:20-30`
- Modify: `crates/strata-zero/src/app.rs:158-161`
- Modify: every `for_place(` caller (list below)

**Interfaces:**
- Produces:
  - `citygen::Stratum { kind, tiles, steady: Grid<u8>, lamps: Grid<u8> }` with `fn emitted(&self, p: Pos, minute: u16) -> u8` and `fn light_at(&self, p: Pos, minute: u16) -> u8`.
  - `MapRef::stratum_at(city, kind, minute: u16)`; `MapRef::stratum_of(city, kind)` builds at `START_MINUTE_OF_DAY`; `MapRef::for_place(city, interiors, constructs, place, minute: u16)`.
  - `StratumMap { city, stratum, minute }`.
  - `AppState::minute_of_day(&self) -> u16`; `hud::minute_at(tick: u64) -> u16`.

- [ ] **Step 1: Write the failing citygen test**

In `crates/citygen/src/generate/dressing.rs` tests, replace the two `light` assertions in `dressing_places_features_and_lights` with:

```rust
        assert!(st.lamps.as_slice().iter().any(|&l| l > 100), "lamps are baked");
        assert!(st.steady.as_slice().iter().any(|&l| l > 60), "neon is baked");
        let lamp = st
            .tiles
            .iter()
            .find(|(_, t)| t.feature == Some(Feature::Lamp))
            .map(|(p, _)| p)
            .unwrap();
        assert!(st.light_at(lamp, 22 * 60) > 150);
        assert_eq!(st.light_at(lamp, 12 * 60), 255, "noon on the street is full sun");
        assert_eq!(st.emitted(lamp, 12 * 60), st.steady[lamp], "the lamp is off by day");
        let under = c.stratum(StratumKind::Undercity).unwrap();
        let dark = under
            .tiles
            .iter()
            .filter(|(p, _)| under.light_at(*p, 22 * 60) < 40)
            .count();
        assert!(dark > 100);
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p citygen dressing`
Expected: compile error, no field `lamps`.

- [ ] **Step 3: Split the stratum**

`crates/citygen/src/world.rs`:

```rust
/// One full tile map plus its baked emitters; ambient comes from the clock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stratum {
    pub kind: StratumKind,
    pub tiles: Grid<Tile>,
    /// Light from fixtures that run all day: neon, holo ads, helipads, windows, the
    /// Net's grid. 0 dark .. 255 bright, no ambient.
    pub steady: Grid<u8>,
    /// Light from street lamps, which switch off by day. No ambient.
    pub lamps: Grid<u8>,
}

impl Stratum {
    pub fn new(kind: StratumKind, size: Size) -> Self {
        Stratum {
            kind,
            tiles: Grid::filled(size, Tile::VOID),
            steady: Grid::filled(size, 0),
            lamps: Grid::filled(size, 0),
        }
    }

    /// Light the tile throws on its own at `minute`: steady fixtures plus lamps while on.
    pub fn emitted(&self, p: Pos, minute: u16) -> u8 {
        let steady = self.steady.get(p).copied().unwrap_or(0);
        let lamps = if crate::lighting::lamps_on(minute) {
            self.lamps.get(p).copied().unwrap_or(0)
        } else {
            0
        };
        steady.saturating_add(lamps)
    }

    /// The light on a tile at `minute`: ambient for the stratum plus what it emits.
    pub fn light_at(&self, p: Pos, minute: u16) -> u8 {
        crate::lighting::ambient(self.kind, minute).saturating_add(self.emitted(p, minute))
    }
}
```

- [ ] **Step 4: Bake twice in dressing**

In `crates/citygen/src/generate/dressing.rs` delete `base_light` and `emission` (lines 31-52) and rewrite `lights`:

```rust
    fn lights(ctx: &mut GenCtx, k: StratumKind) {
        use crate::lighting::{compute, tuning};
        let st = ctx.city.stratum_mut(k).expect("stratum");
        let tiles = &st.tiles;
        let blocks = |p: Pos| tiles.get(p).is_none_or(|t| t.blocks_sight());
        let steady = compute(tiles, 0, blocks, |t| tuning::steady_emission(t.terrain, t.feature, k));
        let lamps = compute(tiles, 0, blocks, |t| tuning::lamp_emission(t.feature));
        st.steady = steady;
        st.lamps = lamps;
    }
```

Update the module doc at line 1 to say "features, emitter grids and per-tile variants".

- [ ] **Step 5: The Net writes steady**

`crates/citygen/src/generate/cyberspace.rs`: delete `const GRID_LIGHT` and `const LANE_LIGHT` (lines 15-18); replace their uses with `crate::lighting::tuning::NET_GRID_LIGHT` and `NET_LANE_LIGHT`; change every `st.light.set(` to `st.steady.set(` (lines 51, 137, 338); line 461 becomes `let light = net.steady[fp.center()];`.

`crates/citygen/src/construct.rs:25`: replace `const LANE_LIGHT: u8 = 255;` with a use of `crate::lighting::tuning::NET_LANE_LIGHT` at line 155. Constructs keep their own `light: Grid<u8>`.

- [ ] **Step 6: Run the citygen tests**

Run: `cargo test -p citygen`
Expected: all pass. Fix any remaining `.light` reads on a `Stratum` the compiler finds.

- [ ] **Step 7: The app reads the split**

`crates/strata-zero/src/ui/hud.rs`: next to `clock_at` add

```rust
/// The minute of the day at `tick`, from the 22:00 start.
pub fn minute_at(tick: u64) -> u16 {
    ((START_MINUTE + tick / TICKS_PER_MINUTE) % (24 * 60)) as u16
}
```

and make `clock_at` use it: `let m = minute_at(tick) as u64;`.

`crates/strata-zero/src/app.rs` after `clock`:

```rust
    /// The minute of the day: the living city's clock, or the tick's before there is one.
    pub fn minute_of_day(&self) -> u16 {
        self.world
            .as_ref()
            .map_or_else(|| crate::ui::hud::minute_at(self.tick), |w| w.minute_of_day())
    }
```

`crates/strata-zero/src/render/map.rs`:

```rust
#[derive(Clone, Copy)]
pub struct StratumMap<'a> {
    pub city: &'a City,
    pub stratum: StratumKind,
    /// The minute of the day the stratum is seen at; ambient and lamps follow it.
    pub minute: u16,
}
```

```rust
    /// A stratum at the night the game starts, for tests and screens before a world exists.
    pub fn stratum_of(city: &'a City, stratum: StratumKind) -> Self {
        Self::stratum_at(city, stratum, crate::ui::hud::START_MINUTE_OF_DAY)
    }

    /// A stratum at `minute` of the day.
    pub fn stratum_at(city: &'a City, stratum: StratumKind, minute: u16) -> Self {
        MapRef::Stratum(StratumMap { city, stratum, minute })
    }

    /// The map at a place at `minute` of the day, if its floor or construct has been generated.
    pub fn for_place(
        city: &'a City,
        interiors: &'a Interiors,
        constructs: &'a Constructs,
        place: Place,
        minute: u16,
    ) -> Option<Self> {
        match place {
            Place::Stratum(k) => Some(MapRef::stratum_at(city, k, minute)),
            // the Floor and Construct arms unchanged
```

`StratumMap::light`:

```rust
    fn light(&self, p: Pos) -> u8 {
        self.city
            .stratum(self.stratum)
            .map_or(0, |s| s.light_at(p, self.minute))
    }
```

`crates/strata-zero/src/render/weather.rs:77`: `let light = map.light(p);` (the overlay already has `map: &MapRef`; drop the `st` lookup if it was only for light).

`crates/strata-zero/src/save.rs`: `FORMAT_VERSION = 4`, `READABLE_VERSIONS: [u16; 4] = [1, 2, 3, 4]`, and the size check becomes

```rust
            .all(|s| {
                s.tiles.size() == city.size
                    && s.steady.size() == city.size
                    && s.lamps.size() == city.size
            });
```

- [ ] **Step 8: Pass the minute at every `for_place` call**

Each call gains a last argument. Where an `AppState` (`st`) is in scope use `st.minute_of_day()`; in `map.rs:439` (a test) use `crate::ui::hud::START_MINUTE_OF_DAY`. Callers: `actions.rs:47,57`, `interiors.rs:219`, `home.rs:48`, `net.rs:955`, `interact.rs:104`, `constructs.rs:64`, `ui/sidebar.rs:599`, `render/map.rs:439`, `screens/console.rs:46,379`, `screens/deck.rs:1226`, `screens/explore.rs:585,1457,1694`. Where a function takes `city, interiors, constructs` but no `st` (e.g. `explore.rs:1694`), add a `minute: u16` parameter and thread it from the caller.

- [ ] **Step 9: Add a save test for the version**

In `crates/strata-zero/src/save.rs` tests, add to `roundtrip_and_header` after the roundtrip assert:

```rust
        let st = back.stratum(citygen::StratumKind::Street).unwrap();
        assert_eq!(st.lamps.size(), back.size);
        assert_eq!(st.steady.size(), back.size);
        // A version 3 file is listed but refused.
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[4..6].copy_from_slice(&3u16.to_le_bytes());
        let old = dir.join("old.city");
        std::fs::write(&old, &bytes).unwrap();
        let err = load_city(&old).unwrap_err().to_string();
        assert!(err.contains("older version"), "{err}");
```

- [ ] **Step 10: Run the whole workspace**

Run: `cargo test --workspace`
Expected: all pass. The `memmap` test may still pass since sight has not changed yet.

- [ ] **Step 11: Commit**

```bash
git add -A crates
git commit -m "feat(citygen): a stratum bakes its steady fixtures and its lamps apart, and the map reads them by the minute"
```

---

### Task 5: The sight predicate and `sees`

**Files:**
- Create: `crates/citygen/src/sight.rs`
- Modify: `crates/citygen/src/lib.rs`

**Interfaces:**
- Produces:
  - `citygen::sight::lit_or_near(origin: Pos, p: Pos, dark_radius: i32, emitted: impl Fn(Pos) -> u8, is_emitter: impl Fn(Pos) -> bool) -> bool`
  - `citygen::sight::sees(from: Pos, to: Pos, dark_radius: i32, blocks: impl Fn(Pos) -> bool, emitted: impl Fn(Pos) -> u8, is_emitter: impl Fn(Pos) -> bool) -> bool`

- [ ] **Step 1: Write the failing tests**

Create `crates/citygen/src/sight.rs` with the tests module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting::tuning::LIT_SIGHT;

    #[test]
    fn near_lit_or_a_fixture_each_count_and_dark_far_does_not() {
        let o = Pos::new(0, 0);
        let dark = |_: Pos| 0u8;
        let none = |_: Pos| false;
        assert!(lit_or_near(o, Pos::new(3, 3), 5, dark, none));
        assert!(!lit_or_near(o, Pos::new(9, 9), 5, dark, none));
        assert!(lit_or_near(o, Pos::new(9, 9), 5, |_| LIT_SIGHT, none));
        assert!(!lit_or_near(o, Pos::new(9, 9), 5, |_| LIT_SIGHT - 1, none));
        assert!(lit_or_near(o, Pos::new(9, 9), 5, dark, |p| p == Pos::new(9, 9)));
    }

    #[test]
    fn the_dark_sees_the_lamp_and_the_lamp_does_not_see_the_dark() {
        let lamp = Pos::new(10, 0);
        let shadow = Pos::new(0, 0);
        let emitted = |p: Pos| if p.chebyshev(lamp) <= 2 { 100 } else { 0 };
        let open = |_: Pos| false;
        let none = |_: Pos| false;
        assert!(sees(shadow, lamp, 3, open, emitted, none));
        assert!(!sees(lamp, shadow, 3, open, emitted, none));
        let wall = |p: Pos| p.x == 5;
        assert!(!sees(shadow, lamp, 3, wall, emitted, none), "a wall between");
        assert!(sees(shadow, Pos::new(2, 0), 3, wall, emitted, none), "near, this side");
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Add `pub mod sight;` to `crates/citygen/src/lib.rs`. Run: `cargo test -p citygen sight`
Expected: compile error, `lit_or_near` not found.

- [ ] **Step 3: Write the predicate**

Above the tests in `crates/citygen/src/sight.rs`:

```rust
//! Whether a tile in line of sight is seen: near enough to make out in the dark, or lit.
//! The same rule serves the player's eyes and anyone looking back.

use crate::geom::Pos;
use crate::lighting::tuning::LIT_SIGHT;
use crate::pathing::bresenham;

/// A tile already in line of sight from `origin` counts as seen when it lies within
/// `dark_radius`, throws `LIT_SIGHT` or more of its own light, or is itself a fixture.
pub fn lit_or_near(
    origin: Pos,
    p: Pos,
    dark_radius: i32,
    emitted: impl Fn(Pos) -> u8,
    is_emitter: impl Fn(Pos) -> bool,
) -> bool {
    origin.chebyshev(p) <= dark_radius || emitted(p) >= LIT_SIGHT || is_emitter(p)
}

/// Whether someone at `from` sees the tile `to`: a clear line between them, and `to`
/// near or lit. Not symmetric: the dark sees the lamp, the lamp does not see the dark.
pub fn sees(
    from: Pos,
    to: Pos,
    dark_radius: i32,
    blocks: impl Fn(Pos) -> bool,
    emitted: impl Fn(Pos) -> u8,
    is_emitter: impl Fn(Pos) -> bool,
) -> bool {
    if from == to {
        return true;
    }
    let clear = bresenham(from, to)
        .into_iter()
        .filter(|p| *p != from && *p != to)
        .all(|p| !blocks(p));
    clear && lit_or_near(from, to, dark_radius, emitted, is_emitter)
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p citygen sight`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/citygen/src/sight.rs crates/citygen/src/lib.rs
git commit -m "feat(citygen): one sight rule, near or lit, from either side"
```

---

### Task 6: Lit sight and the day radius for the player

**Files:**
- Modify: `crates/strata-zero/src/render/map.rs` (`TileMap` trait, `StratumMap` impl)
- Modify: `crates/strata-zero/src/walker/mod.rs:12-16,81-88,184-205` and tests
- Modify: `crates/strata-zero/src/render/memmap.rs:110`
- Modify: `crates/strata-zero/src/app.rs:163-177`

**Interfaces:**
- Consumes: `citygen::sight::lit_or_near`, `citygen::lighting::{daylight, lamps_on, sunlit, emits, tuning::{LIT_RADIUS, DAY_RADIUS}}`.
- Produces:
  - `TileMap` gains `fn emitted(&self, p: Pos) -> u8 { 0 }`, `fn emits(&self, p: Pos) -> bool { false }`, `fn daylight(&self) -> f32 { 0.0 }`, `fn lamps_on(&self) -> bool { true }`, `fn lit_sight(&self) -> bool { false }`.
  - `Player::radius_at(&self, daylight: f32) -> i32`. `Player::radius()` stays as the night value.

- [ ] **Step 1: Write the failing tests**

In `crates/strata-zero/src/walker/mod.rs` tests (the helpers `city()` and `street(&c)` exist there):

```rust
    #[test]
    fn a_lamp_pulls_its_pool_into_view_past_the_dark_radius() {
        let c = city();
        let st = c.stratum(StratumKind::Street).unwrap();
        let mut p = Player::spawn(&c, 1);
        p.update_fov(&street(&c), 0);
        let far_lit = p
            .vis()
            .iter()
            .filter(|(q, v)| **v == Visibility::Visible && q.chebyshev(p.pos) > RADIUS_STREET)
            .filter(|(q, _)| st.emitted(*q, 22 * 60) >= citygen::lighting::tuning::LIT_SIGHT)
            .count();
        assert!(far_lit > 0, "something lit beyond {RADIUS_STREET} tiles is seen");
        let far_dark = p
            .vis()
            .iter()
            .filter(|(q, v)| **v == Visibility::Visible && q.chebyshev(p.pos) > RADIUS_STREET)
            .filter(|(q, _)| st.emitted(*q, 22 * 60) == 0 && !street(&c).emits(*q))
            .count();
        assert_eq!(far_dark, 0, "nothing dark beyond the radius is seen");
    }

    #[test]
    fn noon_on_the_street_sees_to_the_horizon_and_the_net_does_not_care() {
        let c = city();
        let mut p = Player::spawn(&c, 1);
        let noon = MapRef::stratum_at(&c, StratumKind::Street, 12 * 60);
        assert_eq!(p.radius_at(noon.daylight()), citygen::lighting::tuning::DAY_RADIUS);
        p.update_fov(&noon, 0);
        let day = p.vis().count(|v| *v == Visibility::Visible);
        p.update_fov(&street(&c), 0);
        let night = p.vis().count(|v| *v == Visibility::Visible);
        assert!(day > 2 * night, "{day} by day vs {night} at night");
        p.place = Place::Stratum(StratumKind::Net);
        assert_eq!(p.radius_at(1.0), RADIUS_NET);
        p.place = Place::Stratum(StratumKind::Undercity);
        assert_eq!(p.radius_at(1.0), RADIUS_UNDER);
    }
```

In `crates/strata-zero/src/render/memmap.rs:110` change `lit < 60 * 30 / 4` to `lit < 60 * 30 / 2`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero walker`
Expected: compile errors, `radius_at`, `emits`, `daylight` not found.

- [ ] **Step 3: Grow the trait and the stratum impl**

`crates/strata-zero/src/render/map.rs` `TileMap`:

```rust
    /// Light the tile throws on its own, ambient excluded; 0 off the strata.
    fn emitted(&self, _p: Pos) -> u8 {
        0
    }
    /// Whether the tile is a fixture glowing right now.
    fn emits(&self, _p: Pos) -> bool {
        false
    }
    /// How much sun: 0 at night, 1 by day; 0 wherever there is no sky.
    fn daylight(&self) -> f32 {
        0.0
    }
    /// Whether street lamps burn; always, where there is no clock.
    fn lamps_on(&self) -> bool {
        true
    }
    /// Whether light past the dark radius is seen here: the physical strata only.
    fn lit_sight(&self) -> bool {
        false
    }
```

`impl TileMap for StratumMap`:

```rust
    fn emitted(&self, p: Pos) -> u8 {
        self.city
            .stratum(self.stratum)
            .map_or(0, |s| s.emitted(p, self.minute))
    }

    fn emits(&self, p: Pos) -> bool {
        self.tile(p)
            .is_some_and(|t| citygen::lighting::emits(t.terrain, t.feature, self.stratum, self.lamps_on()))
    }

    fn daylight(&self) -> f32 {
        if citygen::lighting::sunlit(self.stratum) {
            citygen::lighting::daylight(self.minute)
        } else {
            0.0
        }
    }

    fn lamps_on(&self) -> bool {
        citygen::lighting::lamps_on(self.minute)
    }

    fn lit_sight(&self) -> bool {
        self.stratum != StratumKind::Net
    }
```

Forward all five in `impl TileMap for MapRef` as `light` is forwarded (floors and constructs fall to the defaults).

- [ ] **Step 4: The player's radius and cast**

`crates/strata-zero/src/walker/mod.rs`:

```rust
/// How far you see in the dark, per place. By day on a sunlit stratum the radius
/// grows toward `DAY_RADIUS`; lit tiles past it are seen regardless.
pub const RADIUS_STREET: i32 = 14;
pub const RADIUS_UNDER: i32 = 8;
pub const RADIUS_NET: i32 = 24;
/// Indoors the walls do the limiting: a lit floor is seen across.
pub const RADIUS_INSIDE: i32 = 30;
```

```rust
    /// The dark radius at night for the current place.
    pub fn radius(&self) -> i32 { /* unchanged */ }

    /// The dark radius with `daylight` (0..=1) on it: night on strata with no sky.
    pub fn radius_at(&self, daylight: f32) -> i32 {
        let night = self.radius();
        match self.place {
            Place::Stratum(k) if citygen::lighting::sunlit(k) => {
                night + ((DAY_RADIUS - night) as f32 * daylight).round() as i32
            }
            _ => night,
        }
    }
```

```rust
    /// Recomputes what the player sees on the current map, demoting old sight to memory.
    /// `sight_bonus` is the character's equipment bonus to the dark radius. On a
    /// physical stratum the cast reaches `LIT_RADIUS` and keeps lit tiles past the dark
    /// radius; the Net and interiors see the plain radius.
    pub fn update_fov(&mut self, map: &dyn TileMap, sight_bonus: i32) {
        let dark = (self.radius_at(map.daylight()) + sight_bonus).max(1);
        let lit = map.lit_sight();
        let cast = if lit { dark.max(LIT_RADIUS) } else { dark };
        let size = map.size();
        let pos = self.pos;
        let grid = self.memory_for(size);
        for v in grid.as_mut_slice().iter_mut() {
            if *v == Visibility::Visible {
                *v = Visibility::Remembered;
            }
        }
        fov::compute(
            pos,
            cast,
            |p| map.tile(p).is_none_or(|t| t.blocks_sight()),
            |p| size.contains(p),
            |p| {
                if !lit || sight::lit_or_near(pos, p, dark, |q| map.emitted(q), |q| map.emits(q)) {
                    grid.set(p, Visibility::Visible);
                }
            },
        );
    }
```

with `use citygen::lighting::tuning::{DAY_RADIUS, LIT_RADIUS};` and `use citygen::sight;` at the top.

`crates/strata-zero/src/app.rs:168`: replace `let night = !(6..18).contains(&hour);` with `let night = citygen::lighting::lamps_on(self.minute_of_day());` and drop the `clock()` call if unused.

- [ ] **Step 5: Run the workspace tests**

Run: `cargo test --workspace`
Expected: all pass. If `memmap` still trips, print `lit` and pick the next bound that holds with margin; do not drop the upper bound.

- [ ] **Step 6: Commit**

```bash
git add crates/strata-zero
git commit -m "feat(app): light is seen across the dark, and the day opens the street to the horizon"
```

---

### Task 7: The sky tint and lamps off by day in the renderer

**Files:**
- Modify: `crates/strata-zero/src/render/mod.rs:100-112` (`RenderCtx`)
- Modify: `crates/strata-zero/src/render/tileset.rs:415-427` and tests
- Modify: `crates/strata-zero/src/render/viewport.rs:175,197`, `atrium.rs:96,116,239`, `mark.rs:46`

**Interfaces:**
- Produces: `RenderCtx { .., lamps_on: bool, sky: Option<(Rgb, f32)> }`.

- [ ] **Step 1: Write the failing test**

In `crates/strata-zero/src/render/tileset.rs` tests, next to `cell_mapping_is_pure_and_lit`:

```rust
    #[test]
    fn a_lamp_glows_at_night_and_is_furniture_by_day_and_the_sky_tints() {
        let s = StylePack::load().unwrap();
        let ctx = |lamps_on, sky| RenderCtx {
            style: &s,
            theme: Some(Theme::CorpCore),
            stratum: StratumKind::Street,
            light: 40,
            tick: 0,
            vis: Visibility::Visible,
            connector: None,
            kind: None,
            wall_mask: 0,
            lamps_on,
            sky,
        };
        let mut lamp = Tile::new(Terrain::Sidewalk);
        lamp.feature = Some(Feature::Lamp);
        let night = cell(&lamp, &ctx(true, None));
        let day = cell(&lamp, &ctx(false, None));
        assert_ne!(night.fg, day.fg, "a lit lamp keeps its colour; an unlit one is scaled");
        let floor = Tile::new(Terrain::Sidewalk);
        let plain = cell(&floor, &ctx(true, None));
        let tinted = cell(&floor, &ctx(true, Some((Rgb(255, 0, 0), 0.5))));
        assert!(tinted.bg.0 > plain.bg.0, "the sky pulls toward red");
    }
```

(If `Terrain::Sidewalk` is not a variant, use `Terrain::Floor`.)

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p strata-zero tileset`
Expected: compile error, no field `lamps_on`.

- [ ] **Step 3: Extend the context and the lighting step**

`crates/strata-zero/src/render/mod.rs` `RenderCtx`: add

```rust
    /// Whether street lamps burn; a lamp that is off is lit like any other tile.
    pub lamps_on: bool,
    /// The sky's cast and its weight, on a sunlit stratum by day.
    pub sky: Option<(Rgb, f32)>,
```

`crates/strata-zero/src/render/tileset.rs:415-427`:

```rust
    let emits = match tile.feature {
        Some(Feature::Lamp) => ctx.lamps_on,
        Some(
            Feature::Neon(_) | Feature::HoloAd | Feature::Terminal | Feature::Camera | Feature::Node(_),
        ) => true,
        _ => false,
    };
    let mut fg = if emits { fg } else { lit(fg, ctx.light) };
    let mut bg = lit(bg, ctx.light);
    if let Some((sky, w)) = ctx.sky {
        if !emits {
            fg = fg.mix(sky, w);
        }
        bg = bg.mix(sky, w);
    }
```

Every `RenderCtx { .. }` literal gains `lamps_on: map.lamps_on()` and `sky: sky_of(map)` where a map is at hand, and `lamps_on: true, sky: None` in the atrium (floors have no sky) and in tests. Add to `render/lighting.rs`:

```rust
/// The sky's cast on this map right now, as the tileset mixes it.
pub fn sky_of(map: &dyn TileMap) -> Option<(Rgb, f32)> {
    map.sky().map(|((r, g, b), w)| (Rgb(r, g, b), w))
}
```

and to `TileMap` a `fn sky(&self) -> Option<((u8, u8, u8), f32)> { None }` that `StratumMap` implements as `citygen::lighting::sky(self.stratum, self.minute)` and `MapRef` forwards.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p strata-zero render`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero/src/render
git commit -m "feat(app): the sky tints the day, and a lamp that is off is just a post"
```

---

### Task 8: Exposure and the HUD segment

**Files:**
- Create: `crates/strata-zero/src/exposure.rs`
- Modify: `crates/strata-zero/src/main.rs` (or `lib.rs`, wherever modules are declared) to add `mod exposure;`
- Modify: `crates/strata-zero/src/ui/hud.rs:173-184` and its test at 677-688
- Modify: `crates/strata-zero/src/screens/explore.rs:2215`

**Interfaces:**
- Consumes: `citygen::sight::sees`, `sim::World::visible(Rect) -> Vec<Mover>`, `MapRef::stratum_at`.
- Produces:
  - `exposure::Exposure { light: u8, eyes: Vec<sim::PersonId> }`
  - `exposure::of(st: &AppState) -> Exposure`
  - `exposure::band(light: u8) -> &'static str`
  - `exposure::text(e: &Exposure) -> String`
  - `hud::location_line(location: &str, exposure: &str, clock: (u8, u8), width: u16)`

- [ ] **Step 1: Write the failing tests**

`crates/strata-zero/src/exposure.rs` tests module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_and_words() {
        assert_eq!(band(0), "dark");
        assert_eq!(band(39), "dark");
        assert_eq!(band(40), "dim");
        assert_eq!(band(100), "lit");
        assert_eq!(band(180), "bright");
        assert_eq!(band(255), "bright");
        let none = Exposure { light: 70, eyes: vec![] };
        assert_eq!(text(&none), "dim · unseen");
        let two = Exposure { light: 200, eyes: vec![sim::PersonId(1), sim::PersonId(2)] };
        assert_eq!(text(&two), "bright · 2 eyes");
        let one = Exposure { light: 10, eyes: vec![sim::PersonId(1)] };
        assert_eq!(text(&one), "dark · 1 eye");
    }

    #[test]
    fn exposure_reads_the_tile_light_and_only_walkers_who_can_see_you() {
        let mut st = crate::app::AppState::test_city(5);
        let e = of(&st);
        let p = st.player.as_ref().unwrap();
        let map = crate::render::map::MapRef::stratum_at(
            st.city.as_ref().unwrap(),
            citygen::StratumKind::Street,
            st.minute_of_day(),
        );
        assert_eq!(e.light, map.light(p.pos));
        let world = st.world.as_ref().unwrap();
        for id in &e.eyes {
            let m = world
                .visible(citygen::Rect::new(p.pos.x - 60, p.pos.y - 60, 121, 121))
                .into_iter()
                .find(|m| matches!(m.kind, sim::MoverKind::Walker { id: w, .. } if w == *id))
                .expect("an eye is a walker on the stratum");
            assert!(m.pos.chebyshev(p.pos) <= 60);
        }
        // Off the strata there is nothing to see you.
        st.player.as_mut().unwrap().place = citygen::Place::Floor { building: citygen::BuildingId(0), level: 0 };
        let inside = of(&st);
        assert_eq!((inside.light, inside.eyes.len()), (0, 0));
    }
}
```

Look in `crates/strata-zero/src/app.rs` tests for the existing fixture that builds an `AppState` with a city, a player and a world (search `fn test_city` or how `screens/console.rs` tests build their state) and use that name instead of `test_city` if it differs. If none builds a world, construct one with `sim::World::seed(city, &st.people, &sim::SimParams::default(), START_MINUTE_OF_DAY, 0)` and assign `st.world`.

`crates/strata-zero/src/ui/hud.rs` test `location_line_keeps_the_clock` becomes:

```rust
    #[test]
    fn location_line_keeps_the_clock_and_the_exposure() {
        let s = location_line("Night Market · Street", "dim · 2 eyes", (22, 41), 52).to_string();
        assert_eq!(s.chars().count(), 52, "{s:?}");
        assert!(
            s.starts_with(" Night Market · Street") && s.ends_with("dim · 2 eyes  ☾ 22:41"),
            "{s:?}"
        );
        let long = location_line(&"B".repeat(60), "bright · unseen", (9, 5), 40).to_string();
        assert_eq!(long.chars().count(), 40, "{long:?}");
        assert!(long.ends_with("… bright · unseen  ☀ 09:05"), "{long:?}");
        let bare = location_line("Home", "", (9, 5), 20).to_string();
        assert!(bare.ends_with("☀ 09:05") && !bare.contains("  ☀"), "{bare:?}");
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero exposure hud`
Expected: compile errors.

- [ ] **Step 3: Write exposure**

`crates/strata-zero/src/exposure.rs`:

```rust
//! How exposed you are: the light on your tile, and who has you in sight. Stealth
//! and combat read this; today it is a readout.

use crate::app::AppState;
use crate::render::map::{MapRef, TileMap};
use citygen::lighting::tuning::{DAY_RADIUS, EXPOSURE_BANDS};
use citygen::{Place, Rect, sight};
use sim::{MoverKind, PersonId};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Exposure {
    /// The light on your tile, ambient included.
    pub light: u8,
    /// Everyone on the stratum who can see you.
    pub eyes: Vec<PersonId>,
}

/// The exposure right now: on a stratum, from the walkers around you; dark and
/// unseen anywhere else.
pub fn of(st: &AppState) -> Exposure {
    let (Some(city), Some(player)) = (st.city.as_ref(), st.player.as_ref()) else {
        return Exposure::default();
    };
    let Place::Stratum(k) = player.place else {
        return Exposure::default();
    };
    let map = MapRef::stratum_at(city, k, st.minute_of_day());
    let light = map.light(player.pos);
    let Some(world) = st.world.as_ref() else {
        return Exposure { light, eyes: vec![] };
    };
    let radius = player.radius_at(map.daylight());
    let bubble = Rect::new(player.pos.x - DAY_RADIUS, player.pos.y - DAY_RADIUS, 2 * DAY_RADIUS + 1, 2 * DAY_RADIUS + 1);
    let eyes = world
        .visible(bubble)
        .into_iter()
        .filter_map(|m| match m.kind {
            MoverKind::Walker { id, .. } => Some((id, m.pos)),
            _ => None,
        })
        .filter(|(_, from)| {
            sight::sees(
                *from,
                player.pos,
                radius,
                |p| map.tile(p).is_none_or(|t| t.blocks_sight()),
                |p| map.emitted(p),
                |p| map.emits(p),
            )
        })
        .map(|(id, _)| id)
        .collect();
    Exposure { light, eyes }
}

/// The word for a light level.
pub fn band(light: u8) -> &'static str {
    EXPOSURE_BANDS
        .iter()
        .find(|(upto, _)| light < *upto)
        .map_or(EXPOSURE_BANDS[EXPOSURE_BANDS.len() - 1].1, |(_, w)| w)
}

/// The HUD's words: `dim · 2 eyes`, `bright · unseen`.
pub fn text(e: &Exposure) -> String {
    let eyes = match e.eyes.len() {
        0 => "unseen".to_string(),
        1 => "1 eye".to_string(),
        n => format!("{n} eyes"),
    };
    format!("{} · {eyes}", band(e.light))
}
```

Note: `World::visible` only lists walkers whose `position(i)` is `(pos, None)`, meaning on the map and not inside; that is the "outside a building" rule for free. If `MoverKind`/`PersonId` are not re-exported from `sim`, import them from `sim::world::MoverKind` and `sim::people::PersonId`.

Check that `band(255)` returns `bright`: the last band's bound is 255 and `255 < 255` is false, so the `map_or` fallback supplies it.

- [ ] **Step 4: Write the HUD segment**

`crates/strata-zero/src/ui/hud.rs`:

```rust
/// The location on the left; exposure, when given, and the clock on the right. The
/// location is cut first so the right side always fits.
pub fn location_line(location: &str, exposure: &str, clock: (u8, u8), width: u16) -> Line<'static> {
    let clock_text = format!("{} {:02}:{:02}", clock_glyph(clock.0), clock.0, clock.1);
    let right = if exposure.is_empty() {
        clock_text
    } else {
        format!("{exposure}  {clock_text}")
    };
    let room = (width as usize).saturating_sub(right.chars().count() + 2);
    let loc = cut(location, room);
    let gap = (width as usize).saturating_sub(1 + loc.chars().count() + right.chars().count());
    Line::from(vec![
        Span::raw(" "),
        cyan(loc),
        Span::raw(" ".repeat(gap)),
        text(right),
    ])
}
```

`crates/strata-zero/src/screens/explore.rs:2215`: compute `let exposure = crate::exposure::text(&crate::exposure::of(st));` once per draw and pass `&exposure`. Off the strata, pass `""` so interiors show only the clock: `let exposure = if matches!(self.place, Place::Stratum(_)) { text(&of(st)) } else { String::new() };`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p strata-zero`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/strata-zero
git commit -m "feat(app): the HUD says how lit you are and how many eyes are on you"
```

---

### Task 9: The `eyes` and `light` dev overlays

**Files:**
- Modify: `crates/strata-zero/src/app.rs` (two flags)
- Modify: `crates/strata-zero/src/screens/console.rs:14-18,392-410` (two commands, help text)
- Modify: `crates/strata-zero/src/render/people.rs:30-35,80-84` and its five test constructions
- Modify: `crates/strata-zero/src/render/lighting.rs` (a `LightOverlay`)
- Modify: `crates/strata-zero/src/screens/explore.rs:2040-2050,2118`

**Interfaces:**
- Produces: `AppState { show_eyes: bool, show_light: bool }`; `PeopleOverlay { .., eyes: &'a [PersonId] }`; `render::lighting::LightOverlay { vis: Option<&Grid<Visibility>> }`.

- [ ] **Step 1: Write the failing tests**

In `crates/strata-zero/src/screens/console.rs` tests (find the existing fixture the `clock` test uses):

```rust
    #[test]
    fn eyes_and_light_toggle() {
        let mut st = state();
        let focus = At { place: citygen::Place::Stratum(citygen::StratumKind::Street), pos: citygen::Pos::new(0, 0) };
        assert!(!st.show_eyes && !st.show_light);
        assert_eq!(run(&mut st, focus, "eyes").unwrap(), "eyes on");
        assert_eq!(run(&mut st, focus, "light").unwrap(), "light on");
        assert!(st.show_eyes && st.show_light);
        assert_eq!(run(&mut st, focus, "eyes").unwrap(), "eyes off");
        assert!(!st.show_eyes);
    }
```

In `crates/strata-zero/src/render/lighting.rs` tests:

```rust
    #[test]
    fn the_light_overlay_paints_grey_by_light_and_keeps_the_glyph() {
        use citygen::{GenParams, Generator, SizePreset, StratumKind};
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        let s = crate::render::style::StylePack::load().unwrap();
        let mut params = GenParams::default();
        params.seed = 3;
        params.size = SizePreset::Small;
        let city = Generator::new(params).run().unwrap();
        let map = crate::render::map::MapRef::stratum_of(&city, StratumKind::Street);
        let area = Rect::new(0, 0, 20, 10);
        let mut buf = Buffer::empty(area);
        LightOverlay { vis: None }.draw(&map, citygen::Pos::new(10, 10), area, 0, &mut buf, &s);
        let p = citygen::Pos::new(10, 10);
        let l = map.light(p);
        assert_eq!(buf[(0, 0)].bg, Rgb(l, l, l).scale(0.6).to_color());
    }
```

(Mirror how `viewport.rs` tests build a city; copy its `GenParams` lines if the fields differ.)

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero console lighting`
Expected: compile errors.

- [ ] **Step 3: The flags and the commands**

`crates/strata-zero/src/app.rs` after `show_fps`:

```rust
    /// Dev: tint the walkers who can see you.
    pub show_eyes: bool,
    /// Dev: paint each visible tile grey by its light.
    pub show_light: bool,
```

Initialise both to `false` where `AppState` is built.

`crates/strata-zero/src/screens/console.rs`: add `"eyes", "light"` to `COMMANDS` (bump the array length) and, before the `"clear"` arm:

```rust
        "eyes" => {
            st.show_eyes = !st.show_eyes;
            Ok(format!("eyes {}", if st.show_eyes { "on" } else { "off" }))
        }
        "light" => {
            st.show_light = !st.show_light;
            Ok(format!("light {}", if st.show_light { "on" } else { "off" }))
        }
```

Append ` · eyes · light` to the help string.

- [ ] **Step 4: The two overlays**

`crates/strata-zero/src/render/people.rs`: add `pub eyes: &'a [PersonId],` to `PeopleOverlay` (import `sim::PersonId`), and in the walker arm:

```rust
                MoverKind::Walker { id, tint } => {
                    let fg = if self.eyes.contains(&id) {
                        Rgb(255, 70, 70).to_color()
                    } else {
                        tint.map(tint_colour).unwrap_or(DIM)
                    };
                    buf[(sx, sy)].set_char('@').set_fg(fg);
                }
```

Every `PeopleOverlay { .. }` construction in the people tests gains `eyes: &[]`.

`crates/strata-zero/src/render/lighting.rs`:

```rust
use crate::render::map::{MapRef, TileMap};
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, screen_to_tile};
use citygen::Pos;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// Dev: every visible tile goes grey by its light value, glyph kept, so a tuning
/// change can be seen against the map.
pub struct LightOverlay<'a> {
    pub vis: Option<&'a citygen::Grid<Visibility>>,
}

impl Overlay for LightOverlay<'_> {
    fn draw(&self, map: &MapRef, origin: Pos, area: Rect, _tick: u64, buf: &mut Buffer, _style: &StylePack) {
        for sy in area.y..area.bottom() {
            for sx in area.x..area.right() {
                let p = screen_to_tile(area, origin, sx, sy);
                if map.tile(p).is_none() {
                    continue;
                }
                if self.vis.is_some_and(|v| v.get(p) != Some(&Visibility::Visible)) {
                    continue;
                }
                let l = map.light(p);
                let grey = Rgb(l, l, l);
                let fg = if l < 128 { Rgb(230, 230, 230) } else { Rgb(20, 20, 20) };
                buf[(sx, sy)].set_bg(grey.scale(0.6).to_color()).set_fg(fg.to_color());
            }
        }
    }
}
```

(Check `screen_to_tile`'s real signature in `viewport.rs` and match it.)

`crates/strata-zero/src/screens/explore.rs`: at 2040 compute `let exposure = if st.show_eyes { crate::exposure::of(st).eyes } else { Vec::new() };` and pass `eyes: &exposure`; after `overlays.push(&fx);` add

```rust
        let light = crate::render::lighting::LightOverlay { vis };
        if st.show_light {
            overlays.push(&light);
        }
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p strata-zero`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/strata-zero
git commit -m "feat(app): dev overlays for who sees you and how lit the map is"
```

---

### Task 10: Docs, lint, and a look

**Files:**
- Modify: `README.md` (the sight paragraph near line 158 and the people paragraph near 168)
- Modify: `docs/ideas/2026-10-17-lit-sight.md` (status line)

- [ ] **Step 1: README**

In the paragraph that describes low-light and sight, add two sentences: "Darkness limits how far you see; light does not. A lamp or a neon sign in your line of sight is seen from across the dark with the pool it lights, and by day the sunlit strata open to the horizon with the lamps off." Add to the people paragraph: "They see you by the same rule: the HUD's `dim · 2 eyes` says how lit your tile is and how many have you in sight."

- [ ] **Step 2: Idea doc status**

Change the status line of `docs/ideas/2026-10-17-lit-sight.md` to: `Status: **built** as of 2026-10-18; see docs/superpowers/specs/2026-10-18-lighting-and-time-of-day-design.md.`

- [ ] **Step 3: Lint and format**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: clean, all green.

- [ ] **Step 4: Look**

Run the game with the `run` skill or `cargo run -p strata-zero`, jump the console clock to `12:00`, `06:00`, `18:30`, `22:00`, and toggle `light` and `eyes`. Confirm: lamps off at noon, the street bright and pale, amber at 18:30, lamp pools seen across the dark at 22:00, the HUD segment changing as you step under a lamp.

- [ ] **Step 5: Commit**

```bash
git add README.md docs/ideas/2026-10-17-lit-sight.md
git commit -m "docs: the README speaks lit sight and the day; the lit-sight riff is built"
```
