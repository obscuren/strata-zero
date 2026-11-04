# Cities Grown into the World Implementation Plan (#19, with #20's outskirts)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A city's map is its block of the world: the world's coast, bay, rivers and lakes run through it, its outline is the overworld's shape, the highways come in as its roads, and past the outline the open ground meets the neighbouring tiles seamlessly.

**Architecture:** `GenParams` gains `site: Option<CitySite { world: WorldParams, city: u8 }>` (`#[serde(default)]`). With a site the city generator rebuilds the overworld (deterministic, ~100 ms), takes the city's block (its own tiles' bounding box plus two tiles all round) as its size, and its terrain stage reads the block's detail ground (`detail::region`, POI-free where the city stands): water, shallows and fords become the city's water; land is marked buildable only inside the outline (the overworld's urban density, blended between tile centres plus noise, above a threshold); the world's highways through the block are painted as `Road` before the arterials, so the stages that paint only `Void` grow the city round them. A last step fills every Street cell still `Void` with the block's open ground. `Outside::block` reads the same block, so city cells and world cells line up exactly.

**Spec:** `docs/superpowers/specs/2026-11-04-cities-in-the-world-design.md`.

## Global Constraints

- Deterministic and portable; the city fingerprints (`crates/citygen/tests/seeds.rs`) stay as they are for a city without a site (a lone city is unchanged in this slice; #21 turns it into a one-city world).
- NEW GAME builds the home city with its site; the map is its block × 8 (about the large generator's size), clamped to the generator's limit.
- Old saves (a city without a site) keep today's behaviour: the centred block.
- Both front ends checked; a large home city still generates in about today's time.

## Review Focus

- The city's edge cells equal the open ground's cells across the block's edge (walk off the city's map and the ground goes on). Test in Task 2.
- No building stands in the world's water. Test in Task 1.
- Every highway through the block reaches the city's roads. Test in Task 1.
- `Outside::block` and the generator agree on the block for every seed tried. Test in Task 2.
- A home city on a coast has its sea where the world has it. Test in Task 1.

---

### Task 1: The generator takes its site

- [ ] Tests: `a_sited_city_takes_its_blocks_size`, `the_worlds_water_runs_through_the_city`, `no_building_stands_in_water`, `the_highways_come_in_as_roads`, `the_city_stops_at_its_outline` (cells of the block's margin tiles carry no buildings), `a_city_without_a_site_is_unchanged` (fingerprints).
- [ ] Implement `CitySite`, `site_block`, the terrain stage's site path, the open-ground fill; commit `citygen: a city grown from its place in the world (#19)`.

### Task 2: The game builds the home city on its site

- [ ] Tests: `new_game_builds_the_home_city_on_its_site`, `the_block_is_the_generators`, `walking_off_the_city_the_ground_goes_on` (the city's street cell at its west edge row y and the world's ground cell beside it are both walkable and the same terrain kind), `an_old_save_keeps_its_block`.
- [ ] Implement in `app.rs` (NEW GAME's params), `outside/mod.rs` (`block` from the site when the city has one), `screens/title.rs`/`create` (the site on fresh params); commit `outside: the home city stands on its place in the world (#19)`.

### Task 3: Shots, checks, review

- [ ] Shots: the city's edge on the street, the overworld over the home city, the coast through the city; the five checks; a fresh reviewer; one fix pass whose commit `Closes #19`. #20 (the outskirts' belt of lots, yards, shanties and billboards, and the Net fading through it) stays open: this slice only stops the city at its outline and meets the open ground.
