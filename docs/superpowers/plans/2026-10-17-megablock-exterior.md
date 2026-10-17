# The Megablock's Outside Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Dress the outside of every megablock with a forecourt at the main door, the building's name over it, a service side with a working back door, and street spill-over (a stall and parked bikes).

**Architecture:**
- A new citygen module, `generate/exterior.rs`, does the dressing as one more work item of the Dressing stage. It runs before Verify and Lights, so the new blockers get checked and the lamps light up.
- It uses its own RNG seeded from the city seed and the building id, so the Dressing stream stays untouched.
- The sign needs the final building name, so it is stamped at the end of the Naming stage. That step uses no RNG.
- The service door is a real `Terrain::Door` on the footprint border. Interior exits come from `City::portals`, so the plaza floor gains the exit by itself. Task 4 proves it validates.

**Tech Stack:** Rust 2024 workspace, rand_chacha, ratatui 0.29.

**Spec:** docs/superpowers/specs/2026-10-17-megablock-exterior-design.md

## Global Constraints

- Only megablocks (`BuildingKind::Megablock`) are dressed. Seeds without a megablock generate exactly as before.
- Generation stays deterministic. The pass never draws from `ctx.rng(Stage::Dressing)` or `ctx.rng(Stage::Naming)`. It uses `ChaCha8Rng::seed_from_u64(hash64(ctx.seed ^ EXTERIOR_SALT, b.id.0 as u64))` (or the crate's equivalent hash helper), with `const EXTERIOR_SALT: u64 = 0xE7E_0F_B10C`.
- The block must not stand out. No new emissive feature: lamps are the existing `Feature::Lamp`, and the sign is the existing `Feature::Sign(char)`.
- No road tile is ever repaved or blocked. The forecourt turns only `Terrain::Sidewalk` into `Terrain::Plaza`.
- Nothing sits on a door's landing tile, and every outdoor tile that was walkable and connected before the pass stays connected after it.
- New enum variants go at the end of their enums, with legend entries, matching how the Task 1 home work added `Couch` and `Shower`.
- Saves stay FORMAT_VERSION 3. No migration.
- Medium seed 8 keeps its home at 18F, flat 1814.

## Review Focus

1. **A megablock on a corner, or with a short or absent street on the far side.** The service door goes on a flank, or nowhere, and never cuts a wall that faces the void. Pinned by Task 2's test over every medium fixture seed.
2. **A sidewalk too narrow for 7×3.** The forecourt shrinks without touching the road, and the canopy and lamps still appear. Pinned by Task 2's test on all megablocks across 40 medium seeds.
3. **A name longer than the façade.** The sign is cut to fit and never wraps round a corner or covers the door. Pinned by Task 3's test.
4. **A plaza floor with two street exits.** It passes `validate_floor` and the shopfronts leave both exits open. Pinned by Task 4's test.
5. **The sim with a two-door megablock.** Seeding and stepping still run, with people entering by either door. Pinned by Task 4's sim smoke test.

---

### Task 1: New pieces for the street

**Files:**
- Modify: `crates/citygen/src/tile.rs` (enums, `is_walkable`, `ALL_FEATURES`)
- Modify: `crates/strata-zero/src/render/tileset.rs` (glyphs, colours, legend)

**Interfaces:**
- Produces:
  - `Feature::Awning`: walkable, does not block sight, label "awning", glyph `▔`, colour the theme's accent dimmed.
  - `Furniture::Bench`: label "bench", glyph `╥`.
  - `Furniture::Vending`: label "vending machine", glyph `▣`.
  - `Furniture::Bike`: label "bike", glyph `ꝺ`. If that glyph renders double-width in the terminal tests, use `ф`.
  - All three furniture pieces block movement, as all `Furniture` does today.

- [ ] **Step 1: Write the failing test** in `crates/citygen/src/tile.rs` tests:

```rust
#[test]
fn street_pieces_walk_and_block_as_intended() {
    let mut t = Tile::new(Terrain::Plaza);
    t.feature = Some(Feature::Awning);
    assert!(t.is_walkable() && !t.blocks_sight());
    for f in [Furniture::Bench, Furniture::Vending, Furniture::Bike] {
        t.feature = Some(Feature::Furniture(f));
        assert!(!t.is_walkable());
    }
    assert_eq!(Furniture::Vending.label(), "vending machine");
    assert!(ALL_FEATURES.contains(&Feature::Awning));
}
```

- [ ] **Step 2:** Run `cargo test -p citygen street_pieces`. Expected: it fails to compile.
- [ ] **Step 3:** Add the variants, labels and `ALL_FEATURES` entries. Add the glyphs and legend entries in `tileset.rs`, following `Couch`/`Shower`. Fix any exhaustive matches the compiler flags.
- [ ] **Step 4:** Run `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`. Expected: all green. This includes the tileset legend/ALL tests that iterate `ALL_FEATURES`.
- [ ] **Step 5:** Commit: `feat(citygen): awnings, benches, vending machines and bikes`.

### Task 2: Dress the megablock's outside

**Files:**
- Create: `crates/citygen/src/generate/exterior.rs`
- Modify: `crates/citygen/src/generate/mod.rs` (declare the module)
- Modify: `crates/citygen/src/generate/dressing.rs` (a `Work::Exterior(i)` per megablock, queued after the facades and streets, before `Verify`)

**Interfaces:**
- Consumes: Task 1's variants.
- Produces, used by Task 3:
  - `pub(crate) fn main_door(city: &City, b: &Building) -> Option<Pos>`: among the building's Street-stratum door tiles on `b.footprint.border()` whose outward neighbour is outdoor ground, the one nearest the middle of its side. Ties go to the lowest `(y, x)`.
  - `pub(crate) fn outward(fp: Rect, p: Pos) -> Pos`: the tile just outside a border tile. Move the existing private helper out of `generate/interiors.rs` into a shared place if that is cleaner.
  - `pub(crate) fn dress(ctx: &mut GenCtx, i: usize)`: does nothing unless `buildings[i].kind == Megablock`.

What `dress` places. Every piece is placed only on a tile that is `bare` (no feature, not a door, not a connector) and is never a door's landing tile:
1. **Forecourt.** Rows 1–3 outward from the main door's side, columns door±3 along it. Each such tile that is `Sidewalk` becomes `Plaza`, and the first gap that isn't sidewalk ends that row. If the main door has no outdoor landing, nothing is placed.
2. **Canopy.** `Awning` on the landing and on the two forecourt tiles beside it along the façade.
3. **Lamps.** `Lamp` on the forecourt tiles at door±3 in row 1 when they're Plaza, otherwise at door±2.
4. **Vending machines.** Two `Vending`, at door±2 in row 1, against the façade.
5. **Bench.** One `Bench` at door±3 in row 2, on the side the RNG picks.
6. **Terminal.** One street `Terminal` at door∓3 in row 2, on the other side. It becomes a Net node and a jack through the existing Net and Cyberspace stages, because they read `Feature::Terminal` on street tiles.
7. **Service door.** On the side opposite the main door, at the border tile nearest that side's middle whose outward neighbour is outdoor ground. If there is none there, try the two flanks, the one nearer the far end first. If none of the three has one, there is no service door. `set_terrain(Door)`.
8. **Service dressing.**
   - Two or three `Trash` on sidewalk tiles beside the service landing, along the façade.
   - Two `Barrier`s marking a bay 2 tiles to one side of the landing.
   - One `Lamp` beside the door.
9. **Stall.** One `Stall` on a bare sidewalk tile within 6 tiles of the forecourt edge, not on the forecourt.
10. **Bikes.** A row of 3–5 `Bike` on sidewalk tiles 4-adjacent to a `Road`, along the main door's side, beyond the forecourt. Skip any tile `open_area` rejects.

The existing `verify` pass removes any blocker that cuts something off. The test below checks that it never had to remove anything on the door paths.

- [ ] **Step 1: Write the failing tests** in `exterior.rs`:

```rust
#[test]
fn every_megablock_gets_a_forecourt_a_canopy_and_lamps() {
    // Medium seeds 1, 2, 8, 11 (the plan's fixtures) plus seeds 1..=40.
    for seed in fixture_seeds() {
        let c = medium(seed);
        for b in c.buildings.iter().filter(|b| b.kind == BuildingKind::Megablock) {
            let door = main_door(&c, b).expect("a megablock has a street door");
            let land = outward(b.footprint, door);
            let t = c.tile(StratumKind::Street, land).unwrap();
            assert_eq!(t.feature, Some(Feature::Awning), "seed {seed}");
            assert!(t.is_walkable());
            let lamps = b.footprint.grow(4).iter()
                .filter(|p| c.tile(StratumKind::Street, *p)
                    .is_some_and(|t| t.feature == Some(Feature::Lamp)))
                .count();
            assert!(lamps >= 2, "seed {seed}");
        }
    }
}

#[test]
fn the_dressing_never_touches_roads_or_cuts_anything_off() {
    // Generate each seed twice, with the Exterior work skipped and with it
    // (a test-only GenParams flag or cfg hook). Then: no Road tile changed,
    // and every outdoor tile reachable before is reachable after.
}

#[test]
fn the_service_door_is_a_real_street_exit() {
    for seed in fixture_seeds() {
        let c = medium(seed);
        for b in c.buildings.iter().filter(|b| b.kind == BuildingKind::Megablock) {
            let doors = c.portals(b.id).into_iter()
                .filter(|(k, _)| *k == StratumKind::Street).count();
            assert!(doors >= 2, "seed {seed}: megablock {} has {doors}", b.id.0);
        }
    }
}

#[test]
fn cities_without_a_megablock_are_unchanged() {
    // Large seed 6 (120×60) has no megablock: its City serializes to the
    // same bytes with the Exterior work skipped and with it.
}
```

- [ ] **Step 2:** Run `cargo test -p citygen exterior`. Expected: it fails.
- [ ] **Step 3:** Implement `main_door`, `outward` and `dress`, and add the `Work::Exterior` queueing. The test-only skip switch is the implementer's choice of mechanism, provided it doesn't exist in release builds' public API.
- [ ] **Step 4:** If any fixture assertion elsewhere moved, fix it. Expected movers: Net/jack counts for megablock cities, and the home landing beside the city gate. Medium seed 8's home must stay at 18F, flat 1814; if it moved, something drew from a shared stream, so fix the cause, not the test. Then run `cargo test --workspace` and clippy.
- [ ] **Step 5:** Commit: `feat(citygen): a forecourt, a service door and a street around each megablock`.

### Task 3: The name over the entrance

**Files:**
- Modify: `crates/citygen/src/generate/naming.rs`
- Modify: `crates/citygen/src/generate/exterior.rs` (the sign function and its test)

**Interfaces:**
- Consumes: `main_door`, `outward` (Task 2).
- Produces: `pub(crate) fn sign(ctx: &mut GenCtx, i: usize)`. Naming calls it for each megablock when its last district is done, just before returning `StepOut::Finished`. It uses no RNG.

Rules:
- Take the building's final `name` and uppercase it.
- Lay it along the main door's side, centred on the door. Letter `k` goes on border tile `start + k` along that side, and the door's own tile is skipped (the name continues on the other side of the door).
- Only `Wall` tiles take a letter. A `Window` or corner ends the run on that side. Spaces stay plain wall but use up their slot.
- Cut the name to the slots available, and overwrite any `Neon`/`Camera` the facade pass put there.
- A building with no name gets no sign.

- [ ] **Step 1: Write the failing test** in `exterior.rs`:

```rust
#[test]
fn the_name_reads_over_the_main_door() {
    for seed in fixture_seeds() {
        let c = medium(seed);
        for b in c.buildings.iter().filter(|b| b.kind == BuildingKind::Megablock) {
            let name = b.name.clone().expect("megablocks are named").to_uppercase();
            let door = main_door(&c, b).unwrap();
            let letters: String = side_tiles(b.footprint, door)  // in reading order
                .filter_map(|p| match c.tile(StratumKind::Street, p)?.feature {
                    Some(Feature::Sign(ch)) => Some(ch),
                    _ => None,
                })
                .collect();
            let squeezed: String = name.chars().filter(|c| *c != ' ').collect();
            assert!(letters.len() >= 3, "seed {seed}");
            assert!(squeezed.starts_with(&letters) || squeezed.contains(&letters),
                "seed {seed}: {letters} vs {name}");
            assert!(c.tile(StratumKind::Street, door).unwrap().terrain == Terrain::Door);
        }
    }
}
```

`side_tiles` is a test helper: the border tiles of the door's side in reading order (left to right, or top to bottom).

- [ ] **Step 2:** Run `cargo test -p citygen the_name_reads`. Expected: it fails.
- [ ] **Step 3:** Implement `sign` and call it from Naming.
- [ ] **Step 4:** Run `cargo test --workspace` and clippy.
- [ ] **Step 5:** Commit: `feat(citygen): the megablock's name over its door`.

### Task 4: Two ways in, inside and in the sim

**Files:**
- Modify: `crates/citygen/src/interior/mod.rs` (tests), and `crates/citygen/src/interior/layouts/atrium.rs` only if a test fails
- Modify: `crates/sim/src/world.rs` (test)

Ruling carried from planning: the "small service corridor" inside is the open span the shopfronts already keep clear beside every exit ("a span by an exit ... stays open plaza"). The plaza is one walkable piece joined to the lift lobby, so no walled corridor is built. If the clear span turns out narrower than 3 tiles at the service exit, widen it to 3 in `shopfronts`.

- [ ] **Step 1: Write the tests.**

```rust
// interior/mod.rs
#[test]
fn megablock_ground_floors_validate_with_the_service_exit() {
    for seed in [1u64, 2, 8, 11] {
        let c = medium(seed);
        for b in c.buildings.iter().filter(|b| b.kind == BuildingKind::Megablock) {
            let plan = FloorPlan::new(&c, b.id).unwrap();
            let ground = layouts::build(&c, &plan, 0);
            assert!(ground.exits.len() >= 2, "seed {seed}");
            validate_floor(&plan, &ground).unwrap();
            for e in &ground.exits {
                let step = ground.doorstep(e);
                let open = step.neighbors4().filter(|n| ground.tiles.get(*n)
                    .is_some_and(|t| t.is_walkable())).count();
                assert!(open >= 2, "seed {seed}: exit {:?} is pinched", e.at);
            }
        }
    }
}

// sim/world.rs
#[test]
fn a_two_door_megablock_seeds_and_steps() {
    // Medium seed 8: build the World, step a full day, no panic,
    // and at least one resident goes Inside the megablock.
}
```

Use the real names of the floor-building and validation entry points in `interior/`. The calls above stand in for them.

- [ ] **Step 2:** Run `cargo test -p citygen megablock_ground_floors` and `cargo test -p sim two_door`. If either fails, fix `atrium.rs` (or the sim) minimally.
- [ ] **Step 3:** Run `cargo test --workspace` and clippy.
- [ ] **Step 4:** Commit: `test: megablocks with a service door, inside and in the sim`.

### Task 5: Smoke and README

**Files:**
- Modify: `README.md` (one or two lines under the megablock section)
- Smoke driver (outside the repo): `/private/tmp/claude-501/-Users-jeffrey-dev-cyberpunk/smoke/drive21.py`, copied from `drive20.py`

- [ ] **Step 1:** Run `cargo build --release --target-dir target/smoke -p strata-zero`. Never use `target/release`, and never touch a running `target/release/strata`, which is Jeffrey's.
- [ ] **Step 2:** Drive medium seed 8 from the flat to the street by the main door and screenshot the forecourt. Check:
  - the canopy is on the landing;
  - the name letters show on the façade;
  - looking at a sign tile shows the building's name.
- [ ] **Step 3:** Walk round to the service side, screenshot it, step through the back door onto the plaza floor, and walk to the lift.
- [ ] **Step 4:** Jack in at the forecourt terminal and back out.
- [ ] **Step 5:** Judge the anti-goal from the shots: the block doesn't stand out more than the shops around it.
- [ ] **Step 6:** Update the README and commit `docs: the megablock's outside`. Report bugs; don't fix them.
