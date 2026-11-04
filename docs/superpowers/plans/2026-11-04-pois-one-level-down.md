# POIs One Level Down Implementation Plan (#17)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every POI on the overworld is a place on its ground one level down: a truck stop with pumps under a canopy, a store you can walk into, a back lot of rigs; a checkpoint gate with booths; a fenced relay mast on its track; a camp ring; a wreck and its debris; the base of a dead arcology; rows of solar panels; a water plant's tanks and pipes; a test range's craters; a rig on the sea with its pier.

**Architecture:** A new `citygen::overworld::poi_ground` lays each POI into the detail `Patch` after the roads and before the dressing, from the POI's anchor and world-wide hashes only, so a tile built alone stays the same tile in any region. Road POIs (truck stop, checkpoint) face the road their tile carries; a layout is drawn in a road frame (along, away) that maps to cells with the screen's aspect kept (along a north-south road a unit is half a row, away from it two columns). Buildings are single rooms laid in the ground itself (walls, windows, a door, floor and furniture), so they are walked into on the ground; a building whose footprint would meet water is left out, decided from a POI-free build of its footprint (deterministic, independent of the patch). Lamps light the open ground round them at night, and lit cells in line of sight are seen from afar.

**Tech Stack:** Rust, citygen's detail patch, the street's terrains and features.

**Spec:** `docs/superpowers/specs/2026-11-04-world-tiles-design.md` ("POIs, one level down"); round 2 (`ow2.py` `kilo`) is the truck stop's reference.

## Global Constraints

- Deterministic and portable; layouts from world-wide hashes (`hash64(seed, salt)` per POI index); fingerprints re-pinned once.
- Only existing terrains and features (glyphs the window draws): `Plaza` forecourt, `Wall`/`Window`/`Door`/`Floor` rooms, `Railing` fences, `Gravel` lots and tracks, `Walkway` piers and decks, `Mass` hulls and tanks; features `Furniture(Vending)` pumps, `Awning` canopy, `Lamp`, `Camera`, `Trash`, `Vent`, `Pipe`, `AcUnit` panels, `Barrier` gates, `Furniture(Couch)` parked rigs, `Sign(char)` lettering, `Pylon` the mast, `Stall` tents, `Furniture(Counter|Shelf)` inside.
- Never over a road's lanes or a bridge; never a building cut by water.
- A tile with a POI still builds in a few milliseconds (release).

## Review Focus

- A road POI on a north-south road keeps its proportions on screen and its forecourt against the road. Test in Task 1.
- The store's door is reachable on foot from the road. Test in Task 1.
- A POI by water (water plant, a truck stop near a river) never has a building cut by water. Test in Task 2.
- The rig is reachable from the coast by its pier. Test in Task 2.
- A POI's tiles built alone equal the same tiles in a region. Test in Task 1.

---

### Task 1: The frame, the truck stop and the checkpoint

**Files:** Create `crates/citygen/src/overworld/poi_ground.rs`; modify `detail.rs` (the `pois` step, `Patch` pub(crate)), `mod.rs`.

- [ ] Tests: `a_truck_stop_has_pumps_a_store_and_a_lot` (on its tiles: ≥ 4 pumps, ≥ 6 canopy cells, a room with a door and floor, a fence, ≥ 2 lamps, a camera); `the_stores_door_is_reached_from_the_road` (a walk over walkable cells from a lane cell to the door); `a_checkpoint_gates_its_road` (barrier cells beside the lanes on its tile, booths both sides, lane cells still road); `a_poi_tile_alone_equals_its_region`; `road_pois_keep_their_shape_on_a_north_south_road` (the forecourt's width in columns over its height in rows is ≥ 1.5 either way the road runs).
- [ ] Implement; commit `tiles: the truck stop and the checkpoint one level down (#17)`.

### Task 2: The other eight

- [ ] Tests: `every_poi_lays_something_on_its_tile` (all kinds, seeds 0..4 of the small world: ≥ 12 cells changed from the bare ground on or beside the POI's tile); `no_building_is_cut_by_water` (every Wall cell of a POI room has no Water or Shallows in its 3×3); `the_rig_is_reached_by_its_pier` (a walk from dry land to the platform over walkable cells); `a_relay_mast_has_its_track_to_the_road` (Gravel from the fence to a road lane, when a road is within 3 tiles).
- [ ] Implement; commit `tiles: the other POIs one level down (#17)`.

### Task 3: Lamps at night

**Files:** `crates/strata-zero/src/outside/ground.rs`, `crates/strata-zero/src/screens/outside.rs`.

- [ ] Tests: `a_lamp_lights_the_ground_round_it` (at 22:00 a cell 2 from a lamp draws brighter than one 12 away); `lit_ground_is_seen_from_afar` (at night a lit cell 20 away in line of sight is in `sight`).
- [ ] Implement; commit `outside: lamps light the open ground at night (#17)`.

### Task 4: Shots, checks, review

- [ ] `outside_shots` adds the truck stop by day and by night, the checkpoint, the camp and the rig.
- [ ] The five checks; a fresh reviewer; one fix pass; its commit `Closes #17`.
