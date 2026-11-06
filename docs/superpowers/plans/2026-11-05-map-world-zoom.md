# The Deck MAP's World Zoom Implementation Plan (#24)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** MAP's zoom goes one step further than the whole city: zoom 4, the world, drawn as round 1's survey chart, with a cursor over its tiles, PLACES listing the world's cities, places and regions by distance (kinds to filter), a card for the place under the cursor, and `t` to track it (its bearing and distance in the panel). Out in the open, MAP opens at zoom 4.

**Architecture:** `atlas::Zoom` gains `World`; past `Fit`, `-` goes to it when the run has a world. `screens/atlas/world.rs` owns zoom 4: its state (`AtlasState.world`: cursor tile, kind filter, scroll), its draw (the chart one tile a cell round the cursor, labels for cities, places, regions and route shields, `@` where you are; the PLACES pane; the card) and its keys (move, `[ ]` step the list, `< >` kinds, `t` track, `+` back to the city's zooms). `render::overworld::chart_cell` is the survey look. A tracked world place is `outside::Outside.mark` (saved; `#[serde(default)]`), not a city track (whose walks and pulses are the city's): its bearing and distance show in the outside screen's NEAR and in the city's tracking line; you're there when you stand on its tile.

**Spec:** `docs/superpowers/specs/2026-11-04-overworld-screen-design.md` ("The deck MAP's world zoom"); mock `round3.html` section 4 (`ow3.py` `deck_world`), round 1's C (`ow.py` `draw_chart`).

## Global Constraints

- The city's zooms and their keys unchanged; zoom 4 only where there is a world (not the lot).
- Fits 100×30 (the pane drops before the map does).
- Glyphs the window draws; exact colours.

## Review Focus

- `-` from the whole city goes to the world; `+` comes back to the city's whole view. Test in Task 1.
- Out in the open, MAP opens at zoom 4 on your tile. Test in Task 1.
- PLACES is nearest first and the kinds filter it. Test in Task 1.
- `t` marks the place; its bearing and distance show; standing on it clears it. Test in Task 1.
- A run saved with a mark keeps it. Test in Task 1.

---

### Task 1: Zoom 4

**Files:** `crates/strata-zero/src/render/overworld.rs` (`chart_cell`), new `crates/strata-zero/src/screens/atlas/world.rs`, `screens/atlas/mod.rs` (`Zoom::World`, the branch), `screens/atlas/keys.rs` (the branch), `outside/mod.rs` (`Mark`, `Outside.mark`), `screens/outside.rs` (NEAR shows the mark; MAP opens at zoom 4), `track/ui.rs` or the sidebar (the mark's line in the city).

- [ ] Tests: `minus_past_the_whole_city_is_the_world`; `out_in_the_open_map_opens_on_the_world`; `places_are_nearest_first_and_kinds_filter_them`; `t_marks_a_world_place_and_its_way_shows`; `standing_on_the_mark_clears_it`; `a_mark_is_kept_with_the_run`; `the_world_zoom_fits_the_smallest_terminal`; `every_world_tile_charts_one_cell`.
- [ ] Implement; commit `atlas: the world zoom (#24)`.

### Task 2: Shots, checks, review

- [ ] Shots: zoom 4 from the city and from the open ground, with a card; the five checks; a fresh reviewer; one fix pass whose commit `Closes #24`.
