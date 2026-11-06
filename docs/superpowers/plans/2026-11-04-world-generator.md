# World Generator, Slice 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `citygen` generates a seeded overworld that reads like the round 1 mock, and `strata world` writes a browser sheet of many seeds in the atlas look to tune it on.

**Architecture:** A new `citygen::overworld` module runs ten deterministic stages over a tile grid (one tile is one cell on screen) and hands over an `Overworld` value. `strata-zero` gets an atlas renderer (`render/overworld.rs`, world + tile to `Cell`) that the overworld screen will reuse, and a sheet writer plus a `world` subcommand that writes those cells as HTML.

**Tech Stack:** Rust 2024, `noise` (OpenSimplex) and `rand_chacha` as the city generator uses them, `citygen::pathing::astar`, serde, clap.

**Spec:** `docs/superpowers/specs/2026-11-04-world-generator-design.md`; numbers start from `docs/superpowers/specs/2026-11-04-overworld-mock/ow.py`.

## Global Constraints

- `citygen` stays free of terminal code; no new crate dependencies anywhere.
- Same params, byte-identical `Overworld` (no `HashMap` iteration in generation; per-stage RNG via `rng::stage_rng`).
- Shapes are measured on screen: s-space is `(x, 2y)`; every distance and every noise sample uses s-space.
- Default world 160×52 tiles; presets small 120×40, medium 160×52, large 220×72; `cities` default 5, range 3..=8.
- Home city size 14, others 6..=9, the town 4.5 (s-space radius).
- Names from `flavor/names.ron` only; nothing named after real places, Cyberpunk 2020, RTG or CDPR (no "Badlands").
- Every glyph one cell wide and drawn by the window's fonts (`every_drawn_character_has_a_glyph`).
- Build into `--target-dir target/smoke`; check both front ends (CLAUDE.md's five commands).
- Medium world generates in well under a second in release.

## Review Focus

- Tiny worlds and crowded ones (custom 60×24 with 8 cities) must not panic or spin: spacing relaxes, then fewer cities (never fewer than 3). Test in Task 4.
- An inland world (no sea) must still work: rivers end in a lake, the home city sits on a lake or river. Test in Task 3 and Task 4 with the `sea_sides` knob forced to 0.
- A city cut off by sea or peaks must not break the routes: if a link can't be pathfound the generator retries a different link, and every city stays reachable. Test over seeds 0..40 in Task 5.
- Determinism must hold across runs and stages: compare postcard bytes of two runs, for every preset. Test in Task 1 (kept passing by every later task).
- `strata world --seeds` with a bad range (`5..2`, `abc`) gives a clean error, not a panic. Test in Task 10.

---

### Task 1: The overworld model and a generator skeleton

**Files:**
- Create: `crates/citygen/src/overworld/mod.rs`
- Create: `crates/citygen/src/overworld/field.rs`
- Modify: `crates/citygen/src/lib.rs` (add `pub mod overworld;` and `pub use overworld::{Overworld, WorldParams, WorldSize};`)

**Interfaces:**
- Produces (all `#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]`, enums also `Copy, Eq, Hash`):
  - `WorldSize { Small, Medium, Large, Custom { w: u32, h: u32 } }` with `fn size(self) -> Size` (120×40, 160×52, 220×72) and `fn label(self) -> &'static str`.
  - `WorldParams { seed: u64, size: WorldSize, cities: u8, flavor: String, knobs: Knobs }`, `Default` (seed 0, Medium, 5, "default"), `fn validate(&self) -> Result<(), GenError>` (w 60..=512, h 24..=256, cities 3..=8).
  - `Knobs { sea_sides: Option<u8>, ridges: Option<u8>, rivers: Option<u8> }`, all `None` by default (a `Some` forces that count; tests and tuning use them).
  - `Water { None, Sea, River, Lake }`, `Ground { Flats, Scrub, Fields, Salt, Spill, Hills, Mountains, Peak }`.
  - `Urban { city: u8, density: u8, theme: Theme }` (density 0..=255).
  - `WorldTile { water: Water, ground: Ground, elevation: u8, road: u8, river: u8, urban: Option<Urban> }`; `road` and `river` are link masks, `LINK_N = 1, LINK_E = 2, LINK_S = 4, LINK_W = 8`, nonzero means present.
  - `WorldCity { name: String, centre: Pos, size: f32, home: bool, towns: Vec<Pos> }`.
  - `Route { name: String, path: Vec<Pos> }`.
  - `PoiKind { TruckStop, Checkpoint, Relay, Camp, Wreck, Arcology, Solar, WaterPlant, TestRange, Rig }` with `const ALL: [PoiKind; 10]` and `fn label(self) -> &'static str`.
  - `Poi { kind: PoiKind, name: String, at: Pos }`.
  - `RegionKind { Range, Bay, Spill, Salt, Flats, Valley }`; `Region { kind: RegionKind, name: String, at: Pos }`.
  - `Overworld { seed, params, size: Size, tiles: Grid<WorldTile>, cities: Vec<WorldCity>, routes: Vec<Route>, rivers: Vec<Vec<Pos>>, pois: Vec<Poi>, regions: Vec<Region> }` with `fn generate(params: WorldParams) -> Result<Overworld, GenError>`, `fn tile(&self, p: Pos) -> Option<&WorldTile>`, `fn poi_at(&self, p: Pos) -> Option<&Poi>`, `fn home(&self) -> &WorldCity`.
  - `field.rs` (pub(crate)): `fn s(p: Pos) -> (f32, f32)` = `(x, 2y)`; `fn sdist(a: Pos, b: Pos) -> f32`; `struct Fbm` built from `(seed, salt)` over `noise::OpenSimplex`, `fn at(&self, x: f32, y: f32, octaves: u32) -> f32` in 0..=1; `fn seg_dist(p, a, b) -> f32` and `fn poly_dist(p, &[(f32, f32)]) -> f32` in s-space.
  - Inside `mod.rs`: a `pub(crate) struct Ctx` carrying params, size, the growing tile grid and each stage's scratch (sea sides, bays, ridges, sites); `generate` runs the stages in this order, each a `pub(crate) fn run(ctx: &mut Ctx) -> Result<(), GenError>` in its own file: `land` (sea, coast, ridges, elevation), `rivers`, `sites`, `ground`, `roads`, `growth`, `pois`, `names`. Each stage takes its RNG from `stage_rng(seed, 100 + n)`.

- [ ] **Step 1: Write the failing tests** in `overworld/mod.rs`:

```rust
#[test]
fn same_params_same_world_bytes() {
    for size in [WorldSize::Small, WorldSize::Medium, WorldSize::Large] {
        let p = WorldParams { seed: 7, size, ..Default::default() };
        let a = postcard::to_allocvec(&Overworld::generate(p.clone()).unwrap()).unwrap();
        let b = postcard::to_allocvec(&Overworld::generate(p).unwrap()).unwrap();
        assert_eq!(a, b, "{size:?}");
    }
}

#[test]
fn bad_params_are_refused() {
    let bad = |p: WorldParams| assert!(Overworld::generate(p).is_err());
    bad(WorldParams { cities: 2, ..Default::default() });
    bad(WorldParams { cities: 9, ..Default::default() });
    bad(WorldParams { size: WorldSize::Custom { w: 40, h: 52 }, ..Default::default() });
}
```

- [ ] **Step 2: Run** `cargo test -p citygen --target-dir target/smoke overworld` — expect FAIL (no module).
- [ ] **Step 3: Implement** the types, `validate`, `field.rs`, `Ctx`, and `generate` with every stage an empty `run` returning `Ok(())` (tiles start as `Water::None`, `Ground::Flats`).
- [ ] **Step 4: Run** the same command — expect PASS.
- [ ] **Step 5: Commit** `citygen: the overworld model and an empty generator`.

### Task 2: Sea, coast, ridges and elevation (`land`)

**Files:**
- Create: `crates/citygen/src/overworld/land.rs`
- Test: `land.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `Ctx`, `Fbm`, `s`, `poly_dist` (Task 1).
- Produces on `Ctx`: `sea_sides: Vec<Dir>`, `bays: Vec<Bay>` where `pub(crate) struct Bay { side: Dir, t: f32, depth: f32, width: f32, tip: Pos }` (`tip` the innermost sea tile), `ridges: Vec<Vec<(f32, f32)>>` (s-space polylines); tiles get `Water::Sea`, `elevation` and the bands `Hills`, `Mountains`, `Peak`.

Rules (values from `ow.py`):
- Sea sides: none 10%, one side 65%, two adjacent sides 25% (`knobs.sea_sides` forces the count). Per side the coast depth along the side is `0.25 × extent + 0.045 × extent × (2·fbm(t/16) − 1) + Σ bays`, a bay adding `depth × exp(−((t − t_b)/width)²)` with depth 6..16 and width 5..8 (s-units); 1..=3 bays across the sea sides. Each tile also gets `4 × (fbm(s/5) − 0.5)` of jitter.
- Ridges: 1 (60%) or 2 (`knobs.ridges` forces). Start on land at least 0.3 of the map's smaller extent from the sea, heading roughly along the coast (±40°), segments of 10..16 s-units turning ≤25° each, 50..90 s-units long, clipped to the map.
- Elevation: `0.32·fbm(s/26) + 0.08·inland + 0.78·m·(0.55 + 0.45·fbm(s/7))`, `m = max(0, 1 − poly_dist/11)`, `inland` 0 at the sea rising to 1 far from it. Bands: > 0.74 Peak, > 0.58 Mountains, > 0.47 Hills. Stored as `(e × 255) as u8`; sea tiles 0.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn most_of_the_map_is_land_and_the_sea_has_bays() {
    for seed in 0..20 {
        let w = Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap();
        let sea = w.tiles.count(|t| t.water == Water::Sea) as f32 / w.size.area() as f32;
        assert!((0.0..=0.4).contains(&sea), "seed {seed}: sea {sea}");
    }
    let w = gen_with(Knobs { sea_sides: Some(1), ..Default::default() }, 3);
    assert!(w.tiles.count(|t| t.water == Water::Sea) > 0);
}

#[test]
fn ridges_make_hills_mountains_and_peaks() {
    for seed in 0..10 {
        let w = Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap();
        assert!(w.tiles.count(|t| t.ground == Ground::Mountains) > 20, "seed {seed}");
        assert!(w.tiles.count(|t| t.ground == Ground::Hills) > 40, "seed {seed}");
    }
}

#[test]
fn an_inland_world_has_no_sea() {
    let w = gen_with(Knobs { sea_sides: Some(0), ..Default::default() }, 5);
    assert_eq!(w.tiles.count(|t| t.water == Water::Sea), 0);
}
```

(`gen_with(knobs, seed) -> Overworld` is a test helper in `mod.rs` tests, shared by later tasks.)

- [ ] **Step 2: Run** `cargo test -p citygen --target-dir target/smoke overworld::land` — expect FAIL.
- [ ] **Step 3: Implement** `land::run`.
- [ ] **Step 4: Run** — expect PASS, and Task 1's tests still pass.
- [ ] **Step 5: Commit** `citygen: the overworld's sea, bays, ridges and elevation`.

### Task 3: Rivers and lakes (`rivers`)

**Files:**
- Create: `crates/citygen/src/overworld/rivers.rs`

**Interfaces:**
- Consumes: `Ctx.bays`, `Ctx.ridges`, tiles from Task 2, `pathing::astar`.
- Produces: `Overworld.rivers` (four-connected paths, spring first), tiles `Water::River` with `river` link masks (the mouth tile also links toward the sea), `Water::Lake` patches; `Ctx.mouths: Vec<Pos>`.

Rules:
- Count 1..=3 (`knobs.rivers` forces). Spring: a Mountains or Hills tile within 8 s-units of a ridge.
- The first river ends at the deepest bay's `tip`; others end at the nearest sea tile, or (50%) join an existing river at its nearest tile. Inland world: the target is a lake made for it (below).
- Path: `astar` four-connected from spring to target, cost `1 + 30·e + 4·fbm(s/9)`, peaks and sea impassable (the target tile passable). That follows the valleys down and bends naturally.
- Lakes: 0..=2, irregular (`hypot(dx/rx, dy/ry) + 0.9·fbm(s/2) < 1.25`, rx 3..5, ry 2..4 s-units), on low land, never on the sea; half the time on a river's path (the lake tiles replace the river there). On an inland world the first river's target is a lake, always made.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn rivers_run_unbroken_from_the_hills_to_the_water() {
    for seed in 0..20 {
        let w = Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap();
        assert!(!w.rivers.is_empty(), "seed {seed}");
        for r in &w.rivers {
            assert!(r.windows(2).all(|p| p[0].manhattan(p[1]) == 1), "seed {seed}: broken");
            let spring = w.tile(r[0]).unwrap();
            assert!(spring.elevation as f32 / 255.0 > 0.4, "seed {seed}: low spring");
            let end = *r.last().unwrap();
            let ends_wet = end.neighbors4().chain([end]).any(|q| {
                w.tile(q).is_some_and(|t| matches!(t.water, Water::Sea | Water::Lake))
                    || w.rivers.iter().any(|o| !std::ptr::eq(o, r) && o.contains(&q))
            });
            assert!(ends_wet, "seed {seed}: river ends dry at {end:?}");
        }
    }
}

#[test]
fn an_inland_world_drains_into_a_lake() {
    let w = gen_with(Knobs { sea_sides: Some(0), ..Default::default() }, 5);
    assert!(w.tiles.count(|t| t.water == Water::Lake) > 0);
}
```

- [ ] **Step 2: Run** `cargo test -p citygen --target-dir target/smoke overworld::rivers` — expect FAIL.
- [ ] **Step 3: Implement** `rivers::run`.
- [ ] **Step 4: Run** — expect PASS.
- [ ] **Step 5: Commit** `citygen: rivers from the ridges to the sea, and lakes`.

### Task 4: City sites, ground cover and dead ground (`sites`, `ground`)

**Files:**
- Create: `crates/citygen/src/overworld/sites.rs`, `crates/citygen/src/overworld/ground.rs`

**Interfaces:**
- Consumes: tiles, `Ctx.mouths`, `Ctx.bays`.
- Produces: `Overworld.cities` with `centre`, `size`, `home` (names empty until Task 8); tiles' `Ground::{Scrub, Fields, Salt, Spill}`.

Rules:
- Site score per land tile (never water, Hills, Mountains, Peak): `2·flatness` (1 − elevation range within 3 tiles) `+ 5` within 3 s-units of a river mouth `+ 3` within 2 of the sea `+ 2` beside a river `+ 1.5` on a lake shore `+ 1` with hills within 6 but none within 2, `− 3` within 6 of the map's edge.
- Home city: the best site, size 14. Then `cities − 2` cities of size 6..=9, then one town of size 4.5 at the best remaining site within 15 s-units of the midpoint of two chosen cities. Minimum spacing 38 s-units (2.4 × the larger size for the town); when they don't fit, spacing ×0.85 and retry, down to 12; then fewer cities, never fewer than 3.
- Cover: moisture `fbm(s/18)`; within 3 s-units of a river with moisture > 0.42: Fields when `fbm(s/3) > 0.45` and within 3 × size of a city, else Scrub; moisture > 0.6 Scrub; else Flats.
- Spill: one patch (`hypot(dx/8, dy/5) + 0.8·fbm(s/4) < 1.3`), centred on flats between size + 6 and size + 16 s-units from a city (the home city 60%), never on water or a centre. Salt: one patch (`hypot(dx/11, dy/5.5) + 0.7·fbm(s/5) < 1.25`) on flats at least 25 s-units from every city.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn the_home_city_is_the_biggest_and_on_the_water() {
    for seed in 0..20 {
        let w = Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap();
        let home = w.home();
        assert!(w.cities.iter().all(|c| c.size <= home.size), "seed {seed}");
        let wet = (-4..=4).flat_map(|dx| (-2..=2).map(move |dy| (dx, dy))).any(|(dx, dy)| {
            w.tile(home.centre.offset(dx, dy)).is_some_and(|t| t.water != Water::None)
        });
        assert!(wet, "seed {seed}: home city dry");
    }
}

#[test]
fn crowded_and_tiny_worlds_still_generate() {
    for seed in 0..10 {
        let w = Overworld::generate(WorldParams {
            seed,
            size: WorldSize::Custom { w: 60, h: 24 },
            cities: 8,
            ..Default::default()
        })
        .unwrap();
        assert!(w.cities.len() >= 3, "seed {seed}");
    }
}

#[test]
fn each_world_has_one_spill_and_one_salt_flat() {
    for seed in 0..10 {
        let w = Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap();
        assert!(w.tiles.count(|t| t.ground == Ground::Spill) > 10, "seed {seed}");
        assert!(w.tiles.count(|t| t.ground == Ground::Salt) > 10, "seed {seed}");
        assert!(w.tiles.count(|t| t.ground != Ground::Flats && t.water != Water::None
            && matches!(t.ground, Ground::Spill | Ground::Salt)) == 0);
    }
}
```

The inland case: `gen_with(Knobs { sea_sides: Some(0), .. }, 5)` keeps the home test's "on the water" (a lake or river).

- [ ] **Step 2: Run** `cargo test -p citygen --target-dir target/smoke overworld::sites overworld::ground` — expect FAIL.
- [ ] **Step 3: Implement** `sites::run` then `ground::run`.
- [ ] **Step 4: Run** — expect PASS.
- [ ] **Step 5: Commit** `citygen: city sites, ground cover and the dead ground`.

### Task 5: Highways (`roads`)

**Files:**
- Create: `crates/citygen/src/overworld/roads.rs`

**Interfaces:**
- Consumes: cities (Task 4), tiles.
- Produces: `Overworld.routes` (one `Route` per link, names `""` until Task 8), `Ctx.chains: Vec<Vec<usize>>` (route indices that form one named road), tiles' `road` link masks (an exit's last tile also links off the map).

Rules:
- Links: Prim's MST over city centres by `sdist`, plus the shortest one or two non-tree links shorter than 1.4 × the longest tree link, plus 2..=3 exits: from the cities nearest a map edge that isn't a sea side, straight out to the nearest edge tile on land.
- Each link pathfound four-connected with `astar`, cost Flats 1, Scrub 1.2, Fields 1.4, Salt 1, Spill 12, Hills 3, Mountains 9, a river 7 (a bridge), a lake 50, plus `0.8·fbm(s/6)`; Peak and Sea impassable; a tile already on a road 0.35. If a link finds no path, drop it and take the next-shortest non-tree link that reconnects the graph.
- Chains: at each city, pair the links whose directions are most opposed (angle over 135°) into one chain; a chain is one named road.
- Masks: for each path, consecutive tiles link both ways (parallel roads never join).

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn every_city_is_reachable_by_road() {
    for seed in 0..40 {
        let w = Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap();
        let mut reached = vec![false; w.cities.len()];
        reached[0] = true;
        let at = |p: Pos| w.cities.iter().position(|c| c.centre == p);
        loop {
            let before = reached.iter().filter(|r| **r).count();
            for r in &w.routes {
                let (a, b) = (at(r.path[0]), at(*r.path.last().unwrap()));
                if let (Some(a), Some(b)) = (a, b) {
                    if reached[a] || reached[b] { reached[a] = true; reached[b] = true; }
                }
            }
            if reached.iter().filter(|r| **r).count() == before { break; }
        }
        assert!(reached.iter().all(|r| *r), "seed {seed}: {reached:?}");
    }
}

#[test]
fn roads_are_unbroken_and_stay_off_the_sea() {
    for seed in 0..20 {
        let w = Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap();
        for r in &w.routes {
            assert!(r.path.windows(2).all(|p| p[0].manhattan(p[1]) == 1), "seed {seed}");
            assert!(r.path.iter().all(|p| w.tile(*p).unwrap().water != Water::Sea));
            assert!(r.path.iter().all(|p| w.tile(*p).unwrap().road != 0));
        }
        let exits = w.routes.iter().filter(|r| {
            let e = *r.path.last().unwrap();
            e.x == 0 || e.y == 0 || e.x == w.size.w as i32 - 1 || e.y == w.size.h as i32 - 1
        });
        assert!(exits.count() >= 1, "seed {seed}: the world doesn't go on");
    }
}
```

- [ ] **Step 2: Run** `cargo test -p citygen --target-dir target/smoke overworld::roads` — expect FAIL.
- [ ] **Step 3: Implement** `roads::run`.
- [ ] **Step 4: Run** — expect PASS.
- [ ] **Step 5: Commit** `citygen: highways between the cities and off the map`.

### Task 6: Cities grow, towns, district colours (`growth`)

**Files:**
- Create: `crates/citygen/src/overworld/growth.rs`

**Interfaces:**
- Consumes: cities, routes, tiles.
- Produces: tiles' `urban`, `WorldCity.towns`.

Rules (the mock's `grow`):
- Road distance field: s-space distance to the nearest road tile (a row step counts 2), capped at 8; water distance the same to sea and lake tiles.
- For a city of size S at c, each tile within 3S s-units that isn't Sea, Lake, Peak, Spill or Salt: `score = S·(0.7 + 0.6·fbm(s/5)) − d`; `+ (3.2 − dr)·1.5·(1.15 − d/(2.7S))` when `dr ≤ 3` and `d < 2.7S`; `+ (2.5 − dw)` when `dw ≤ 2`; `− 4` on Hills, `− 12` on Mountains. Positive scores are candidates with density `min(1, score/S) × dense`. River tiles are candidates like land and stay river.
- Keep the candidates eight-connected to the centre (river tiles count). A tile two cities want goes to the denser.
- Towns: along each route out of a city of size ≥ 8, the first tile where `|d − 2.1S| < 0.5` and the tile hash < 0.55, at least 8 s-units from other towns: grow it with S = 2.6, `dense` 0.55, as the parent city's.
- District colour: seeds per city `clamp(round(S / 2.4), 1, 6)`, spaced over 0.55S; the centre's seed CorpCore, only within 0.4S of the centre; the others rotate NightMarket, Megablocks, NeonStrip, Docks, Industrial, OldTown, CombatZone by city; beyond 1.3S Megablocks or OldTown by a coarse hash; tiles beside the sea or a lake Docks 70%.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn cities_grow_on_land_and_hold_together() {
    for seed in 0..20 {
        let w = Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap();
        for (p, t) in w.tiles.iter() {
            if t.urban.is_some() {
                assert!(!matches!(t.water, Water::Sea | Water::Lake), "seed {seed} {p:?}");
                assert!(!matches!(t.ground, Ground::Peak | Ground::Spill | Ground::Salt));
            }
        }
        for (i, c) in w.cities.iter().enumerate() {
            let mine = |p: Pos| w.tile(p).and_then(|t| t.urban).is_some_and(|u| u.city as usize == i);
            let core = crate::pathing::flood_fill(w.size, c.centre, |p| mine(p));
            let all = w.tiles.iter().filter(|(p, _)| mine(*p)).count();
            let towns: usize = c.towns.iter()
                .map(|t| crate::pathing::flood_fill(w.size, *t, |p| mine(p)).len()).sum();
            assert!(core.len() + towns >= all * 9 / 10, "seed {seed}: {} scattered", c.name);
        }
    }
}

#[test]
fn the_home_city_is_big_and_spans_its_river() {
    for seed in 0..20 {
        let w = Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap();
        let tiles = w.tiles.count(|t| t.urban.is_some_and(|u| u.city == 0));
        assert!(tiles > 200, "seed {seed}: home only {tiles} tiles");
        let river_in_city = w.tiles.count(|t| t.water == Water::River && t.urban.is_some_and(|u| u.city == 0));
        if river_in_city > 0 {
            // tiles of the city on both sides: the city reaches past its river
            assert!(river_in_city >= 2, "seed {seed}");
        }
    }
}
```

(`flood_fill` passes eight-connected? It is four-connected in `pathing.rs`: use it for the test as written; if the generator's eight-connected keep leaves diagonal-only joints, the 90% bound allows them.)

- [ ] **Step 2: Run** `cargo test -p citygen --target-dir target/smoke overworld::growth` — expect FAIL.
- [ ] **Step 3: Implement** `growth::run`.
- [ ] **Step 4: Run** — expect PASS.
- [ ] **Step 5: Commit** `citygen: cities grow along their roads and shores`.

### Task 7: POIs (`pois`)

**Files:**
- Create: `crates/citygen/src/overworld/pois.rs`

**Interfaces:**
- Consumes: tiles, cities, routes.
- Produces: `Overworld.pois` (names empty until Task 8).

Rules: target count `clamp(open_tiles / 600, 6, 16)`; never on a city tile or within 3 s-units of one, a peak, or water (a rig only on the sea); at least 6 s-units apart. Kinds in this order, each up to its cap, while the target isn't met:

| Kind | Cap | Where |
|---|---|---|
| TruckStop | 3 | beside (not on) a route longer than 30 tiles, near its middle |
| Checkpoint | 2 | on a route, 3 tiles past the edge of a city of size ≥ 8 |
| Relay | 2 | on Hills |
| Camp | 2 | on Flats or Scrub, ≥ 10 s-units from any road |
| Wreck | 2 | any open land |
| Arcology | 1 | on Flats within 6 of the sea, ≥ 15 from any city |
| Solar | 1 | on Flats |
| WaterPlant | 1 | beside a lake or river, outside cities |
| TestRange | 1 | on or beside Spill or Salt |
| Rig | 1 | on the sea within 6 s-units of land |

- [ ] **Step 1: Write the failing test:**

```rust
#[test]
fn pois_sit_where_their_kind_goes() {
    for seed in 0..20 {
        let w = Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap();
        assert!(w.pois.len() >= 6, "seed {seed}: {}", w.pois.len());
        for p in &w.pois {
            let t = w.tile(p.at).unwrap();
            assert!(t.urban.is_none() && t.ground != Ground::Peak, "seed {seed} {p:?}");
            match p.kind {
                PoiKind::Rig => assert_eq!(t.water, Water::Sea),
                PoiKind::Checkpoint => assert!(t.road != 0),
                PoiKind::Relay => assert_eq!(t.ground, Ground::Hills),
                _ => assert_ne!(t.water, Water::Sea),
            }
        }
        for (i, a) in w.pois.iter().enumerate() {
            for b in &w.pois[i + 1..] {
                assert!(field::sdist(a.at, b.at) >= 6.0, "seed {seed}: crowded");
            }
        }
    }
}
```

- [ ] **Step 2: Run** `cargo test -p citygen --target-dir target/smoke overworld::pois` — expect FAIL.
- [ ] **Step 3: Implement** `pois::run`.
- [ ] **Step 4: Run** — expect PASS.
- [ ] **Step 5: Commit** `citygen: POIs out in the open`.

### Task 8: Names (`names`, flavor)

**Files:**
- Create: `crates/citygen/src/overworld/names.rs`
- Modify: `crates/citygen/src/flavor.rs` (a `#[serde(default)] pub world: WorldNames` field on `FlavorPack`)
- Modify: `flavor/names.ron` (a `world: WorldNames(...)` section)

**Interfaces:**
- Produces: `pub struct WorldNames { regions: Vec<(RegionKind, Vec<String>)>, pois: Vec<(PoiKind, NameParts)> }` (`Default` empty); every `WorldCity.name`, `Route.name`, `Poi.name`; `Overworld.regions`.

Rules:
- Cities: `FlavorPack::city_name`, retried until unique.
- Routes: each chain gets "Route N", N drawn without repeats from 1, 3, 5, 7, 9, 11, 12, 14, 17, 22, 26, 31; a chain with more than half its tiles within 3 s-units of the sea is "Coast Road" (the first such chain only).
- Regions: one Range per ridge (label at the ridge's middle, on a Hills/Mountains/Peak tile), one Bay per bay (label at the bay's centre, a sea tile), the Spill, the Salt flat (label at each patch's centre tile), one Flats (the flats tile farthest from roads, cities and water, a Flats tile), one Valley (the middle tile of the longest river, a river or Fields/Scrub tile beside it). Names drawn without repeats from the kind's list.
- POIs: `prefix + suffix` from the kind's `NameParts`; Relay and Rig suffixes are numbers.
- `names.ron` lists (original, no real or RTG/CDPR names):
  - Range: "the Teeth", "the Spine", "the Ribs", "the Knuckles", "the Saw", "the Combs", "Rust Ridge", "Glass Ridge", "the Molars"
  - Bay: "Sable Bay", "Oil Bay", "Grey Bay", "Rust Bay", "Slack Bay", "the Sump"
  - Spill: "the Spill", "the Seep", "Low Spill", "the Slick", "Bleed Field"
  - Salt: "Brine Flats", "the Crust", "White Pan", "Salt Hollow"
  - Flats: "Dust Flats", "the Scour", "Ash Flats", "the Pan", "Grit Flats", "the Long Dry"
  - Valley: "the Silt", "the Wash", "Mud Run", "the Bottoms", "Reed Run"
  - TruckStop: prefixes "Kilo-", "Juliet-", "Tango-", "Echo-", "Mike-", "Oscar-"; suffixes "4 truck stop", "7 truck stop", "9 truck stop", "12 truck stop"
  - Checkpoint: prefixes "Halcor", "Velk", "Orrin", "Tesk", "Marrow"; suffixes " checkpoint"
  - Relay: prefixes "relay mast R"; suffixes "2", "4", "7", "9", "11"
  - Camp: prefixes "nomad camp · "; suffixes "the Drift", "Tin Camp", "the Caravan", "Wheelhouse"
  - Wreck: prefixes ""; suffixes "AV wreck", "tanker wreck", "rig wreck", "downed drone"
  - Arcology: prefixes "Carrow", "Velde", "Hask", "Orrin"; suffixes " arcology"
  - Solar: prefixes "Sunfield", "Glare", "Brightline", "Heliot"; suffixes " array"
  - WaterPlant: prefixes "Brine", "Clearwell", "Deepdraw"; suffixes " water plant"
  - TestRange: prefixes "Velk", "Halcor", "Orrin"; suffixes " test range"
  - Rig: prefixes "Rig "; suffixes "2", "4", "6", "8"

- [ ] **Step 1: Write the failing test:**

```rust
#[test]
fn everything_has_a_name_and_labels_sit_in_their_regions() {
    for seed in 0..20 {
        let w = Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap();
        let mut cities: Vec<_> = w.cities.iter().map(|c| c.name.clone()).collect();
        cities.sort();
        cities.dedup();
        assert_eq!(cities.len(), w.cities.len(), "seed {seed}");
        assert!(w.cities.iter().all(|c| !c.name.is_empty()));
        assert!(w.routes.iter().all(|r| !r.name.is_empty()));
        assert!(w.pois.iter().all(|p| !p.name.is_empty()));
        for r in &w.regions {
            let t = w.tile(r.at).unwrap();
            let inside = match r.kind {
                RegionKind::Range => matches!(t.ground, Ground::Hills | Ground::Mountains | Ground::Peak),
                RegionKind::Bay => t.water == Water::Sea,
                RegionKind::Spill => t.ground == Ground::Spill,
                RegionKind::Salt => t.ground == Ground::Salt,
                RegionKind::Flats => t.ground == Ground::Flats,
                RegionKind::Valley => t.water == Water::River || matches!(t.ground, Ground::Fields | Ground::Scrub),
            };
            assert!(inside, "seed {seed}: {} at {:?}", r.name, r.at);
        }
    }
}
```

- [ ] **Step 2: Run** `cargo test -p citygen --target-dir target/smoke overworld::names` — expect FAIL.
- [ ] **Step 3: Implement** the flavor section and `names::run`.
- [ ] **Step 4: Run** `cargo test -p citygen --target-dir target/smoke` — expect PASS (the whole crate: the flavor change must not break the city).
- [ ] **Step 5: Commit** `citygen: names for the world's cities, roads, regions and POIs`.

### Task 9: The atlas look (`render/overworld.rs`)

**Files:**
- Create: `crates/strata-zero/src/render/overworld.rs`
- Modify: `crates/strata-zero/src/render/mod.rs` (`pub mod overworld;`)

**Interfaces:**
- Consumes: `citygen::overworld::*`, `StylePack::theme_or_neutral(Some(theme)).accent`.
- Produces: `pub fn atlas_cell(world: &Overworld, p: Pos, style: &StylePack) -> Cell` and `pub fn poi_glyph(kind: PoiKind) -> (char, Rgb)`.

The look is `ow.py`'s `atlas_cell`, in this priority: POI, road, river, city, lake, sea, ground. Background `ATLAS_BG = Rgb(4, 6, 10)`. A per-tile `r` in 0..1 from `rng::hash64(world.seed, packed position)`.
- POI glyphs: TruckStop `⌂` (255,176,0), Checkpoint `╪` (255,40,72), Relay `⬡` (0,255,200), Camp `△` (255,128,64), Wreck `×` (255,40,72), Arcology `▣` (168,144,216), Solar `▦` (122,176,224), WaterPlant `◎` (0,229,255), TestRange `◈` (255,0,200), Rig `◆` (200,160,80); bold, on the tile's own background.
- Road: double box glyph from `road` (`═║╔╗╚╝╠╣╦╩╬`), fg (200,160,80), in a city (240,200,112) bold on (26,20,8), over water on (10,26,46).
- River: round box glyph from `river` (`─│╭╮╰╯├┤┬┴┼`), (58,138,216) on (6,18,31).
- City: density v and accent a: v > 0.75 `█`/`▓`, > 0.45 `▓`/`▒`, > 0.22 `▒`/`░`, else `░`/`▫`/`·`; fg `mix(mix((42,46,56), (154,160,174), v), a, 0.22 + 0.38v)`; `▪` in the accent where v > 0.45 and r > 0.92; bg `mix(ATLAS_BG, a, 0.03 + 0.06v)`.
- Lake `≈` (42,90,138) on (10,26,46). Sea beside land `~` (42,90,138) on (10,26,46); open sea `~` (26,58,92) when r < 0.12 else blank, on (5,12,24).
- Ground: Peak `▲` (200,204,216); Mountains `▲` (120,124,138); Hills `∩` (r < 0.7) or `·`, (122,102,72); Spill `≈`/`·`/blank (r < 0.18 / 0.6), (74,138,34) or (154,255,90) when r > 0.9, on (6,12,4); Salt `░`/`·` (90,88,82); Fields `▬` (58,106,48); Scrub `"`/`,`/blank (74,90,52); Flats `·` (r < 0.3) `.` (r < 0.45) or blank, (74,66,52).

- [ ] **Step 1: Write the failing test** in `render/overworld.rs`:

```rust
#[test]
fn every_kind_of_tile_draws_one_cell() {
    let w = Overworld::generate(WorldParams { seed: 3, ..Default::default() }).unwrap();
    let style = StylePack::load().unwrap();
    let mut kinds = std::collections::BTreeSet::new();
    for (p, t) in w.tiles.iter() {
        let c = atlas_cell(&w, p, &style);
        assert_eq!(unicode_width::UnicodeWidthChar::width(c.ch), Some(1), "{:?} {p:?}", c.ch);
        kinds.insert(format!("{:?}{:?}{}{}", t.water, t.ground, t.road != 0, t.urban.is_some()));
    }
    assert!(kinds.len() > 12, "a world has many kinds of tile: {kinds:?}");
    for poi in &w.pois {
        assert_eq!(atlas_cell(&w, poi.at, &style).ch, poi_glyph(poi.kind).0);
    }
}
```

- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke render::overworld` — expect FAIL.
- [ ] **Step 3: Implement** `atlas_cell` and `poi_glyph`.
- [ ] **Step 4: Run** it, then `cargo test -p strata-zero --features window --target-dir target/smoke every_drawn_character_has_a_glyph` — expect both PASS.
- [ ] **Step 5: Commit** `render: the overworld's atlas look`.

### Task 10: The sheet and `strata world`

**Files:**
- Create: `crates/strata-zero/src/sheet.rs` (and `mod sheet;` in `main.rs`)
- Modify: `crates/strata-zero/src/cli.rs` (a `World` subcommand and its dispatch)

**Interfaces:**
- Consumes: `atlas_cell`, `Overworld::generate`.
- Produces: `pub fn world_sheet(worlds: &[(Overworld, std::time::Duration)], style: &StylePack) -> String`; `pub fn parse_seeds(s: &str) -> anyhow::Result<std::ops::Range<u64>>` in `cli.rs`; `Cmd::World { seed: Option<u64>, seeds: Option<String>, size: String /* "medium" */, cities: Option<u8>, out: PathBuf /* "world-sheet.html" */ }`.

The page: dark (`#0d1117`), Fira Code Regular and Bold embedded as base64 `@font-face` (`include_bytes!("../fonts/…")`, a small base64 encoder in `sheet.rs`), cells as rows of spans run-length by style, cell width `0.6em`, row height `1.25em`. Per world: a heading `seed N · 160×52 · 5 cities · 12 ms`, a line of city names with sizes (the home city first, marked home), a line of counts (land %, routes with names, POIs by kind, regions with names), then the whole world. `size` takes the presets' labels or `WxH` (reuse `parse_size`'s shape: write `parse_world_size`). The command prints the path and one line per seed.

- [ ] **Step 1: Write the failing tests** in `cli.rs` and `sheet.rs`:

```rust
#[test]
fn seeds_parse_as_ranges() {
    assert_eq!(parse_seeds("0..12").unwrap(), 0..12);
    assert_eq!(parse_seeds("7").unwrap(), 7..8);
    assert!(parse_seeds("5..2").is_err());
    assert!(parse_seeds("abc").is_err());
}

#[test]
fn the_sheet_has_a_section_per_world() {
    let style = StylePack::load().unwrap();
    let worlds: Vec<_> = (0..2)
        .map(|seed| (Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap(),
                     std::time::Duration::from_millis(5)))
        .collect();
    let html = world_sheet(&worlds, &style);
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("seed 0 ·") && html.contains("seed 1 ·"));
    assert!(html.contains(&worlds[1].0.home().name));
    assert!(html.contains("@font-face"));
}
```

- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke seeds_parse the_sheet_has` — expect FAIL.
- [ ] **Step 3: Implement** `sheet.rs`, `parse_seeds`, `parse_world_size`, the subcommand.
- [ ] **Step 4: Run** the tests — PASS; then `cargo run -p strata-zero --release --target-dir target/smoke -- world --seeds 0..12 --out target/smoke/world-sheet.html` prints twelve lines and writes the page.
- [ ] **Step 5: Commit** `cli: strata world writes a sheet of overworlds`.

### Task 11: First test run, tuning, both front ends

**Files:**
- Modify: whichever stage files the tuning touches; `docs/development.md` (a line for `strata world` under the CLI section).

- [ ] **Step 1:** Write the sheet for seeds 0..12 and open it in the browser pane next to `round1.html`'s A frame.
- [ ] **Step 2:** For each seed, check against the mock: a coast with bays, a ridge with hills at its foot, a river from the ridge into a bay through the home city, cities grown along roads and shores (not circles), roads that bend, outlying towns, dead ground near a city's edge, POIs spread out. Note what's off across seeds, not on one.
- [ ] **Step 3:** Tune the numbers in the stages (keep the tests passing), rewrite the sheet, repeat until most seeds read like the mock.
- [ ] **Step 4:** Run CLAUDE.md's five checks (fmt, both clippys, both test runs) — all clean.
- [ ] **Step 5: Commit** `overworld: first tuning pass on the sheet`, and put the sheet's path in the final report.
