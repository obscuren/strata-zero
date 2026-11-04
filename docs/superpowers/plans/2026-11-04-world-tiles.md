# Open Ground One Level Down Implementation Plan (#16)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `citygen` builds the walkable detail map of any open overworld tile (8×8 detail tiles) from the world plan and the seed, joined seamlessly to its neighbours, and `strata world --detail` draws a stretch of it in the game's tileset to check by eye.

**Architecture:** A new `citygen::overworld::detail` module turns overworld tiles into `Grid<Tile>` patches of the existing street `Tile` type (so walking, sight and the renderer work unchanged), using only world-wide detail coordinates and edge hashes so a tile built alone equals the same tile built in a region. New ground terrains are appended to `Terrain`; the tileset draws them in round 2's "T" look. The sheet gains a detail mode that paints a region through `render::tileset::cell`.

**Tech Stack:** Rust 2024, `citygen::overworld::field` (portable maths, `Fbm`, `distance_field`), the existing tile and tileset code, clap.

**Spec:** `docs/superpowers/specs/2026-11-04-world-tiles-design.md` (with `2026-11-04-world-generator-design.md` for the overworld it reads). Round 4 (`2026-11-04-overworld-mock/round4.html`, `ow4.py`) is the reference drawing and its numbers are the starting point.

**Starts after:** save the run (#2) has merged and this branch is rebased onto `main` (re-run every check after the rebase before Task 1).

**Not in this plan:** POI layouts (#17), the open Net (#18), cities in the world (#19), the overworld screen and walking between tiles in the game (#22), keeping and saving tiles (#25). A tile that is a city's or a POI's is built here as plain ground; those slices draw over it.

## Global Constraints

- One overworld tile = 8×8 detail tiles; detail coordinates are world-wide: tile `(tx, ty)` covers `8tx..8tx+8 × 8ty..8ty+8`.
- Everything shared across an edge comes from world-wide coordinates, the seed and the edge (never a tile's local state): `ground(tx, ty)` built alone equals the same cells of a `region` built over it.
- Portable: IEEE-exact maths only (`field::length`, `sin`, `cos`, `pow_five_quarters`, sqrt); no `hypot`, `powf`, `sin`, `cos`, `exp`, `ln` from std. Pinned fingerprints for detail tiles.
- New `Terrain` and `Feature` variants are **appended** after the last existing variant (postcard saves index enums by position); `ALL_TERRAIN` grows to match.
- Roads cross every tile edge at the middle: two lanes on detail rows 3 and 4 (horizontal) or columns 3 and 4 (vertical) of the edge; a road bends only once, at the tile's middle, as a clean corner; junctions meet at the middle.
- Rivers 3 detail tiles wide at the spring, widening to 7 at the mouth; banks of reeds; shallows at the water's edge; fords only where a river is 3 wide.
- Every glyph one cell wide and drawn by the window's fonts; exact `Rgb` colours.
- No terminal code in `citygen`; both front ends checked (CLAUDE.md's five commands); build into `target/smoke`.
- A tile builds in well under a millisecond on average in release.

## Review Focus

- A road or river that crosses a tile edge must meet itself on the other side for every pair of neighbours, including tiles on the map's edge and corners (the edge has no neighbour: a road there runs off the map straight). Test in Task 4 and Task 5 over whole regions.
- Water must never touch dry land directly: shallows or a shore band always sit between, also where a river meets the sea or a lake, and where the sea fills a whole tile. Test in Task 3.
- A bridge must stay walkable end to end where a road crosses a 7-wide river near its mouth, and never leave a gap of water in the road. Test in Task 5.
- Tiles at the overworld's outer edge and corners (no neighbour to blend with) must build without panicking and look like their neighbours inside. Test in Task 2.
- Identical output on every platform: pinned fingerprints of several detail tiles (coast, river mouth, mountains, junction, salt flat). Test in Task 6.

---

### Task 1: Ground terrains and how they look

**Files:**
- Modify: `crates/citygen/src/tile.rs` (append terrains, a feature; walkability, sight, labels, `ALL_TERRAIN`)
- Modify: `crates/strata-zero/src/render/tileset.rs` (`glyph_for`, `terrain_colors`, `feature_color`)
- Modify: any other exhaustive `match` on `Terrain` the compiler points at (`render/mappix.rs`, `screens/explore.rs`, `render/atrium.rs`, `interior/…`)

**Interfaces:**
- Produces, appended to `Terrain` after `Railing`: `Dirt, Scrub, Crops, Salt, Spill, Gravel, Sand, Reeds, Rock, Cliff, Shallows, Ford, Rise`. `Feature::Pylon` appended after `Fitting`.
- Walkable: `Dirt, Scrub, Crops, Salt, Spill, Gravel, Sand, Reeds, Ford, Rise` (and the existing `Road`, `Bridge`). Not walkable: `Rock, Cliff, Shallows` (and `Water`). Blocks sight: `Cliff` only. `Feature::Pylon` blocks movement, not sight (like `Furniture`).
- Labels: "dirt", "scrub", "crops", "salt", "spill", "gravel", "sand", "reeds", "rock", "cliff", "shallows", "ford", "rise".
- Glyphs and colours, round 2's T look (fg, bg; the renderer's light scales them as for streets):
  - Dirt: `·` rarely (variant % 16 == 0), else blank; (67,60,48) on (13,11,9). Low rises
    (`░`) come from the generator's relief (Task 2), not the glyph
  - Scrub: `"` or `,` (variant % 3 == 0 / == 1), else blank; (74,90,48) on (13,12,9)
  - Crops: `▬` on rows where variant is even, else blank; (58,106,48) on (11,12,8)
  - Salt: `░` or `·`; (90,88,82) on (18,18,16)
  - Spill: `≈`, `·` or blank; (74,138,34) on (6,12,4)
  - Gravel: `∙` or blank; (74,69,60) on (20,19,16)
  - Sand: `░`, `·` or blank; (138,122,88) on (28,24,18)
  - Reeds: `"`, `·` or blank; (94,112,64) on (15,18,11)
  - Rock: `▪` or `■`; (94,88,80) on (13,11,9)
  - Cliff: `▓` or `█`; (52,49,44) on (12,11,10)
  - Shallows: `~`, `≈` or blank; (106,166,216) on (18,60,94)
  - Ford: `·` or blank; (90,140,190) on (14,40,64)
  - Rise: `░` mostly, `·` now and then; (70,60,44) on (20,17,13)
  - Pylon: `╫`, (106,110,118)

- [ ] **Step 1: Write the failing tests** in `tile.rs` and `tileset.rs`:

```rust
// tile.rs
#[test]
fn open_ground_walks_and_sees_as_it_should() {
    use Terrain::*;
    for t in [Dirt, Scrub, Crops, Salt, Spill, Gravel, Sand, Reeds, Ford, Rise] {
        assert!(Tile::new(t).is_walkable(), "{t:?}");
        assert!(!Tile::new(t).blocks_sight(), "{t:?}");
    }
    for t in [Rock, Cliff, Shallows] {
        assert!(!Tile::new(t).is_walkable(), "{t:?}");
    }
    assert!(Tile::new(Cliff).blocks_sight());
    assert!(!Tile::new(Rock).blocks_sight());
    let mut pylon = Tile::new(Dirt);
    pylon.feature = Some(Feature::Pylon);
    assert!(!pylon.is_walkable() && !pylon.blocks_sight());
    assert_eq!(ALL_TERRAIN.len(), 41);
    assert_eq!(*ALL_TERRAIN.last().unwrap(), Rise);
}
```

```rust
// tileset.rs (next to the existing per-terrain test that walks ALL_TERRAIN)
#[test]
fn open_ground_draws_in_one_cell() {
    let style = StylePack::load().unwrap();
    for t in [Terrain::Dirt, Terrain::Scrub, Terrain::Crops, Terrain::Salt, Terrain::Spill,
              Terrain::Gravel, Terrain::Sand, Terrain::Reeds, Terrain::Rock, Terrain::Cliff,
              Terrain::Shallows, Terrain::Ford, Terrain::Rise] {
        for variant in 0..=255u8 {
            let mut tile = Tile::new(t);
            tile.variant = variant;
            let c = cell(&tile, &street_ctx(&style));
            assert_eq!(unicode_width::UnicodeWidthChar::width(c.ch), Some(1), "{t:?}");
        }
    }
}
```

(`street_ctx` is the test helper the tileset tests already use for a Street `RenderCtx`; if it doesn't exist, make one in the test module.)

- [ ] **Step 2: Run** `cargo test -p citygen --target-dir target/smoke open_ground_walks` — expect FAIL (no such variants).
- [ ] **Step 3: Implement** the variants (appended), rules, labels, `ALL_TERRAIN`, then the tileset arms; fix every exhaustive match the compiler flags (mappix: Rock and Cliff count as solid; Shallows as water).
- [ ] **Step 4: Run** both tests, `cargo test --workspace --target-dir target/smoke`, and `cargo test -p strata-zero --features window --target-dir target/smoke every_drawn_character_has_a_glyph` — expect PASS.
- [ ] **Step 5: Commit** `tiles: ground terrains for the open world`.

### Task 2: The detail module, coordinates and the ground cover

**Files:**
- Create: `crates/citygen/src/overworld/detail.rs` (`pub mod detail;` in `overworld/mod.rs`)

**Interfaces:**
- Produces:
  - `pub const TILE: i32 = 8;`
  - `pub fn ground(world: &Overworld, tx: i32, ty: i32) -> Grid<Tile>`: the 8×8 patch of overworld tile `(tx, ty)`, Street-level `Tile`s with `variant` from `rng::tile_hash(world.seed, StratumKind::Street, detail_pos)`.
  - `pub fn region(world: &Overworld, x0: i32, y0: i32, w: u32, h: u32) -> Grid<Tile>`: any rectangle of detail coordinates (may extend past the world: out-of-world cells are `Terrain::Void`), built from the same per-cell rules, so it equals the stitched `ground` patches.
  - Internal: `fn blend(world, x, y, f: impl Fn(&WorldTile) -> f32) -> f32` (bilinear between tile centres; a missing tile counts as its nearest in-world tile, so the outer edge blends with itself); `fn jitter_tile(world, x, y) -> (i32, i32)` (the overworld tile whose ground a detail cell takes, displaced by world-wide noise up to ±3 columns, ±1.5 rows, so ground kinds meet in curves).
- Relief: world-wide noise (scale 8×4) over the blended elevation turns Dirt into
  `Terrain::Rise` where it is above 0.66 (round 2's T look: low rises in `░`).
- Ground cover by the jittered tile's `Ground`: Flats → Dirt; Scrub → Scrub; Fields → Crops; Salt → Salt; Spill → Spill; Hills → Dirt with Rock where a hash < 0.12; Mountains → Cliff where world-wide noise (scale 4×2) ≥ 0.45 else Dirt (passes); Peak → Cliff.

- [ ] **Step 1: Write the failing tests** in `detail.rs`:

```rust
#[test]
fn a_tile_alone_equals_the_same_tile_in_a_region() {
    let w = Overworld::generate(WorldParams { seed: 0, ..Default::default() }).unwrap();
    let r = region(&w, 8 * 40, 8 * 20, 8 * 6, 8 * 4);
    for ty in 20..24 {
        for tx in 40..46 {
            let g = ground(&w, tx, ty);
            for y in 0..8 {
                for x in 0..8 {
                    let a = g[Pos::new(x, y)];
                    let b = r[Pos::new(8 * (tx - 40) + x, 8 * (ty - 20) + y)];
                    assert_eq!((a.terrain, a.feature, a.variant), (b.terrain, b.feature, b.variant), "tile {tx},{ty} cell {x},{y}");
                }
            }
        }
    }
}

#[test]
fn ground_follows_its_overworld_tile() {
    for seed in 0..5 {
        let w = Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap();
        for (p, t) in w.tiles.iter() {
            if t.water != Water::None || t.road != 0 || t.urban.is_some() {
                continue;
            }
            let want = match t.ground {
                Ground::Flats | Ground::Hills => Terrain::Dirt, // or Rise, or Rock on hills
                Ground::Scrub => Terrain::Scrub,
                Ground::Fields => Terrain::Crops,
                Ground::Salt => Terrain::Salt,
                Ground::Spill => Terrain::Spill,
                Ground::Mountains | Ground::Peak => Terrain::Cliff,
            };
            // the centre of the patch is the tile's own ground (edges may blend)
            let c = ground(&w, p.x, p.y)[Pos::new(4, 4)].terrain;
            assert!(c == want || matches!(c, Terrain::Rock | Terrain::Dirt | Terrain::Rise), "seed {seed} {p:?}: {c:?} for {:?}", t.ground);
        }
    }
}

#[test]
fn the_world_edge_and_corners_build() {
    let w = Overworld::generate(WorldParams { seed: 3, size: WorldSize::Small, ..Default::default() }).unwrap();
    let (mw, mh) = (w.size.w as i32, w.size.h as i32);
    for (tx, ty) in [(0, 0), (mw - 1, 0), (0, mh - 1), (mw - 1, mh - 1), (mw / 2, 0)] {
        let g = ground(&w, tx, ty);
        assert_eq!(g.size(), Size { w: 8, h: 8 });
    }
    let r = region(&w, -8, -8, 24, 24);
    assert_eq!(r[Pos::new(0, 0)].terrain, Terrain::Void, "outside the world is void");
}
```

- [ ] **Step 2: Run** `cargo test -p citygen --target-dir target/smoke overworld::detail` — expect FAIL.
- [ ] **Step 3: Implement** `ground`, `region`, `blend`, `jitter_tile`, the cover rules.
- [ ] **Step 4: Run** — expect PASS.
- [ ] **Step 5: Commit** `tiles: open ground one level down, cover from the overworld`.

### Task 3: Shores: the sea and lakes

**Files:**
- Modify: `crates/citygen/src/overworld/detail.rs`

**Interfaces:**
- Consumes: `blend`, `field::Fbm`.
- Produces: in `ground`/`region`: `Terrain::Water` for deep water, `Terrain::Shallows` for water within 2 (columns, rows count double) of land, the shore band on land within 1.5 of water: `Sand` beside the sea, `Reeds` beside a lake or river; the spill keeps its own ground at its shore.
- Shoreline: water where `blend(sea) + shore(x, y) > 0.5` (and the same for lakes), `shore = 0.34·(n(x/16, y/8) − 0.5) + 0.22·(n(x/6, y/3) − 0.5) + 0.10·(n(x/2.5, y/1.25) − 0.5)` with world-wide noise (ow4.py's numbers).
- Distances to land and water for the bands are computed over the cell's neighbourhood in world-wide coordinates (a fixed radius, so a tile alone still matches the region).

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn water_never_touches_dry_land() {
    for seed in 0..6 {
        let w = Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap();
        let r = region(&w, 0, 0, 8 * w.size.w, 8 * w.size.h);
        for (p, t) in r.iter() {
            if t.terrain != Terrain::Water {
                continue;
            }
            for q in p.neighbors8() {
                if let Some(n) = r.get(q) {
                    assert!(
                        matches!(n.terrain, Terrain::Water | Terrain::Shallows | Terrain::Void | Terrain::Bridge | Terrain::Railing),
                        "seed {seed}: deep water at {p:?} next to {:?}", n.terrain
                    );
                }
            }
        }
    }
}

#[test]
fn a_sea_tile_is_water_and_its_shore_is_sand() {
    let w = Overworld::generate(WorldParams { seed: 0, ..Default::default() }).unwrap();
    let (p, _) = w.tiles.iter().find(|(p, t)| {
        t.water == Water::Sea && p.neighbors8().all(|q| w.tile(q).is_some_and(|n| n.water == Water::Sea))
    }).unwrap();
    assert!(ground(&w, p.x, p.y).iter().all(|(_, t)| matches!(t.terrain, Terrain::Water | Terrain::Shallows)));
    let r = region(&w, 0, 0, 8 * w.size.w, 8 * w.size.h);
    assert!(r.count(|t| t.terrain == Terrain::Sand) > 50, "a coast has a beach");
}
```

- [ ] **Step 2: Run** `cargo test -p citygen --target-dir target/smoke overworld::detail` — expect the two new tests to FAIL.
- [ ] **Step 3: Implement** the shoreline, the shallows and the shore band.
- [ ] **Step 4: Run** — expect PASS (Task 2's tests too).
- [ ] **Step 5: Commit** `tiles: natural shores, shallows and beaches`.

### Task 4: Rivers, wide and joined

**Files:**
- Modify: `crates/citygen/src/overworld/detail.rs`

**Interfaces:**
- Produces: `pub fn river_width(world: &Overworld, p: Pos) -> u8` (3 at a river's spring tile to 7 at its last tile, by position along `world.rivers`; 0 off a river); `fn crossing(world, tx, ty, side: Dir, road: bool) -> Pos` (a road always at the edge's middle, a river at `2 + hash(seed, edge) % 4` along it; the same from both tiles: W and N edges delegate to the neighbour's E and S); `fn centreline(world, tx, ty, mask: u8, road: bool) -> Vec<Pos>` (straight crossing to crossing for a pass-through, else each crossing to the tile's middle, four-connected). River cells: a square brush of the width round each centreline cell, `Water`; its edges become `Shallows` and its banks `Reeds` by Task 3's bands; where the width is 3, a few hashed centreline cells (1 in 6) are `Ford`.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn rivers_run_unbroken_across_tiles() {
    for seed in 0..6 {
        let w = Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap();
        let r = region(&w, 0, 0, 8 * w.size.w, 8 * w.size.h);
        for river in &w.rivers {
            // the water in each step of the course reaches the next step's patch
            for pair in river.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let cells_a: Vec<Pos> = (0..8).flat_map(|y| (0..8).map(move |x| Pos::new(8 * a.x + x, 8 * a.y + y))).collect();
                let near_b = |p: Pos| p.x >= 8 * b.x - 1 && p.x <= 8 * b.x + 8 && p.y >= 8 * b.y - 1 && p.y <= 8 * b.y + 8;
                let joined = cells_a.iter().any(|p| {
                    r.get(*p).is_some_and(|t| matches!(t.terrain, Terrain::Water | Terrain::Shallows | Terrain::Ford | Terrain::Bridge))
                        && near_b(*p)
                });
                assert!(joined, "seed {seed}: river breaks between {a:?} and {b:?}");
            }
        }
    }
}

#[test]
fn rivers_widen_toward_the_mouth() {
    let w = Overworld::generate(WorldParams { seed: 0, ..Default::default() }).unwrap();
    let river = w.rivers.iter().max_by_key(|r| r.len()).unwrap();
    assert_eq!(river_width(&w, river[0]), 3);
    assert_eq!(river_width(&w, *river.last().unwrap()), 7);
}
```

- [ ] **Step 2: Run** — expect FAIL.
- [ ] **Step 3: Implement** `river_width`, `crossing`, `centreline`, the brush, the fords.
- [ ] **Step 4: Run** — expect PASS.
- [ ] **Step 5: Commit** `tiles: rivers, wide, banked and joined across tiles`.

### Task 5: Highways: straight, cornered, bridged

**Files:**
- Modify: `crates/citygen/src/overworld/detail.rs`

**Interfaces:**
- Consumes: `crossing(…, road: true)`, `centreline(…, road: true)`.
- Produces: `Terrain::Road` two cells wide along each road centreline (the cell and the one right and/or below it, so horizontal runs fill rows 3–4 and vertical runs columns 3–4 of the tile); `Terrain::Gravel` shoulders on dry land beside it; over water, `Terrain::Bridge` with `Terrain::Railing` along both sides; a road that leaves the world runs straight to the world's edge.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn a_straight_highway_is_straight_one_level_down() {
    let w = Overworld::generate(WorldParams { seed: 0, ..Default::default() }).unwrap();
    // a run of tiles linked east-west and nothing else
    let run: Vec<Pos> = w.tiles.iter()
        .filter(|(_, t)| t.road == LINK_E | LINK_W && t.water == Water::None)
        .map(|(p, _)| p).collect();
    assert!(run.len() > 10);
    for p in run {
        let g = ground(&w, p.x, p.y);
        for x in 0..8 {
            assert_eq!(g[Pos::new(x, 3)].terrain, Terrain::Road, "{p:?} col {x}");
            assert_eq!(g[Pos::new(x, 4)].terrain, Terrain::Road, "{p:?} col {x}");
            assert_ne!(g[Pos::new(x, 2)].terrain, Terrain::Road);
            assert_ne!(g[Pos::new(x, 5)].terrain, Terrain::Road);
        }
    }
}

#[test]
fn every_road_meets_itself_across_every_edge() {
    for seed in 0..6 {
        let w = Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap();
        for (p, t) in w.tiles.iter() {
            if t.road & LINK_E != 0 && w.tile(p.offset(1, 0)).is_some() {
                let (a, b) = (ground(&w, p.x, p.y), ground(&w, p.x + 1, p.y));
                for y in [3, 4] {
                    let ok = |t: Terrain| matches!(t, Terrain::Road | Terrain::Bridge);
                    assert!(ok(a[Pos::new(7, y)].terrain) && ok(b[Pos::new(0, y)].terrain), "seed {seed} {p:?} row {y}");
                }
            }
            if t.road & LINK_S != 0 && w.tile(p.offset(0, 1)).is_some() {
                let (a, b) = (ground(&w, p.x, p.y), ground(&w, p.x, p.y + 1));
                for x in [3, 4] {
                    let ok = |t: Terrain| matches!(t, Terrain::Road | Terrain::Bridge);
                    assert!(ok(a[Pos::new(x, 7)].terrain) && ok(b[Pos::new(x, 0)].terrain), "seed {seed} {p:?} col {x}");
                }
            }
        }
    }
}

#[test]
fn a_bridge_carries_the_road_across_the_whole_river() {
    // Where a highway crosses a river (7 wide near the mouth), both lanes stay road or
    // bridge the whole way: no gap of water, no shallows in the road.
    for seed in 0..10 {
        let w = Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap();
        for (p, t) in w.tiles.iter().filter(|(_, t)| t.road != 0 && t.water == Water::River) {
            let g = ground(&w, p.x, p.y);
            let lanes: Vec<Pos> = match t.road {
                m if m == LINK_E | LINK_W => (0..8).flat_map(|x| [Pos::new(x, 3), Pos::new(x, 4)]).collect(),
                m if m == LINK_N | LINK_S => (0..8).flat_map(|y| [Pos::new(3, y), Pos::new(4, y)]).collect(),
                _ => continue,
            };
            for c in lanes {
                assert!(matches!(g[c].terrain, Terrain::Road | Terrain::Bridge), "seed {seed} {p:?} {c:?}: {:?}", g[c].terrain);
            }
            assert!(g.count(|c| c.terrain == Terrain::Railing) >= 2, "seed {seed} {p:?}: a bridge has rails");
        }
    }
}

#[test]
fn a_road_off_the_map_runs_to_the_edge() {
    for seed in 0..10 {
        let w = Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap();
        let (mw, mh) = (w.size.w as i32, w.size.h as i32);
        for (p, t) in w.tiles.iter() {
            let g = ground(&w, p.x, p.y);
            let road = |x: i32, y: i32| matches!(g[Pos::new(x, y)].terrain, Terrain::Road | Terrain::Bridge);
            if p.x == mw - 1 && t.road & LINK_E != 0 { assert!(road(7, 3) && road(7, 4), "seed {seed} {p:?}"); }
            if p.x == 0 && t.road & LINK_W != 0 { assert!(road(0, 3) && road(0, 4), "seed {seed} {p:?}"); }
            if p.y == mh - 1 && t.road & LINK_S != 0 { assert!(road(3, 7) && road(4, 7), "seed {seed} {p:?}"); }
            if p.y == 0 && t.road & LINK_N != 0 { assert!(road(3, 0) && road(4, 0), "seed {seed} {p:?}"); }
        }
    }
}
```

- [ ] **Step 2: Run** — expect FAIL.
- [ ] **Step 3: Implement** the road brush, shoulders, bridges and rails, the edge-of-world runs.
- [ ] **Step 4: Run** — expect PASS.
- [ ] **Step 5: Commit** `tiles: highways straight across tiles, cornered, bridged`.

### Task 6: Dressing, fingerprints, speed

**Files:**
- Modify: `crates/citygen/src/overworld/detail.rs`

**Interfaces:**
- Produces: sparse dressing by world-wide hash: `Feature::Pylon` every 16 detail tiles along a line 6 cells to one side of highways that the route's hash picks (a third of routes), with `Feature::Cable` on the cells between; `Feature::Trash` and wrecked cars (`Feature::Barrier`) near roads at low density; nothing on water, roads or bridges. `pub fn fingerprint(world, tx, ty) -> u64` for the tests (`#[cfg(test)]`).

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn detail_fingerprints_are_pinned() {
    // A shared seed must give the same ground on every platform; CI runs this on
    // macOS, Linux and Windows. Update only for a change meant to move the ground.
    let w = Overworld::generate(WorldParams { seed: 0, ..Default::default() }).unwrap();
    let pick = |f: &dyn Fn(&WorldTile) -> bool| w.tiles.iter().find(|(_, t)| f(t)).map(|(p, _)| p).unwrap();
    let tiles = [
        ("coast", pick(&|t| t.water == Water::Sea)),
        ("river", pick(&|t| t.water == Water::River && t.road == 0)),
        ("bridge", pick(&|t| t.water == Water::River && t.road != 0)),
        ("mountains", pick(&|t| t.ground == Ground::Mountains)),
        ("salt", pick(&|t| t.ground == Ground::Salt)),
    ];
    let got: Vec<(&str, u64)> = tiles.iter().map(|(n, p)| (*n, fingerprint(&w, p.x, p.y))).collect();
    assert_eq!(got, vec![("coast", 0), ("river", 0), ("bridge", 0), ("mountains", 0), ("salt", 0)]);
}

#[test]
fn tiles_build_fast() {
    let w = Overworld::generate(WorldParams { seed: 1, ..Default::default() }).unwrap();
    let start = std::time::Instant::now();
    for ty in 0..w.size.h as i32 {
        for tx in 0..w.size.w as i32 {
            let _ = ground(&w, tx, ty);
        }
    }
    let per = start.elapsed() / (w.size.w * w.size.h);
    assert!(per < std::time::Duration::from_millis(3), "{per:?} a tile (debug build)");
}
```

- [ ] **Step 2: Run** — expect FAIL (fingerprints 0, dressing missing).
- [ ] **Step 3: Implement** the dressing and `fingerprint`; run once, then pin the five values printed by the failure.
- [ ] **Step 4: Run** — expect PASS, and the whole crate's tests.
- [ ] **Step 5: Commit** `tiles: dressing, and the ground's fingerprints pinned`.

### Task 7: Seeing it: `strata world --detail`

**Files:**
- Modify: `crates/strata-zero/src/cli.rs` (`--detail TX,TY`, `--span WxH` on `World`)
- Modify: `crates/strata-zero/src/sheet.rs` (`detail_sheet`)
- Modify: `docs/development.md`

**Interfaces:**
- Consumes: `detail::region`, `render::tileset::cell`, `render::overworld::atlas_cell`.
- Produces: `pub fn detail_sheet(world: &Overworld, centre: Pos, span: (u32, u32), style: &StylePack) -> String`: the region of `span` overworld tiles (default 10×4, one screen) round `centre`, drawn cell by cell through `tileset::cell` with a Street `RenderCtx` at full light, water shaded by depth (bg darkened by the distance to land, up to 6), beside the overworld round it in the atlas look with the region framed; `pub fn parse_tile(s: &str) -> Result<Pos>`.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn a_tile_parses() {
    assert_eq!(parse_tile("50,36").unwrap(), Pos::new(50, 36));
    assert!(parse_tile("50").is_err() && parse_tile("a,b").is_err());
}

#[test]
fn the_detail_sheet_draws_the_ground() {
    let style = StylePack::load().unwrap();
    let w = Overworld::generate(WorldParams::default()).unwrap();
    let html = detail_sheet(&w, Pos::new(50, 26), (4, 2), &style);
    assert!(html.starts_with("<!doctype html>") && html.contains("@font-face"));
    assert!(html.matches("<div class=r>").count() >= 16 + 13, "detail rows and the overworld beside");
}
```

- [ ] **Step 2: Run** — expect FAIL.
- [ ] **Step 3: Implement** the flags (`--detail` needs a single seed: refuse it with `--seeds`), `detail_sheet`, the docs line.
- [ ] **Step 4: Run** the tests, then `cargo run -p strata-zero --release --target-dir target/smoke -- world --seed 0 --detail 108,5 --out target/smoke/detail.html` and open it: a truck stop's tile (as plain ground for now), its highway straight with a corner, the river wide.
- [ ] **Step 5: Commit** `cli: strata world --detail draws the ground one level down`.

### Task 8: Checks and a look

- [ ] **Step 1:** CLAUDE.md's five checks — all clean.
- [ ] **Step 2:** Detail sheets for seed 0 at the round 4 spots (the truck stop, the bridge, the lake, the city's edge) next to `round4.html`; note differences that matter for #17–#19.
- [ ] **Step 3: Commit** any tuning with the fingerprints re-pinned, and put the sheets' paths in the report.
