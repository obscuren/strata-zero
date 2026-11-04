# City Outskirts Implementation Plan (#20)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Where a city on its site thins out, its last blocks are round 2's outskirts (parking lots with a body shop, fenced scrapyards, shanties round fire barrels, billboards facing the city, the last lamps) and its sidewalks give way to gravel; in the Net the district tint and the grid fade through them, the shanties are off the grid but one splices a stolen line, and past the outline the city's Net is the open Net, its trunks meeting the city's grid.

**Architecture:** `generate::site::Site` gets its density back (the overworld's urban density for the city, blended between tile centres). A block of a city on its site whose middle is below `OUTSKIRTS` (0.22) is an outskirts block: the Lots stage lays one of four layouts on it from the block's hash (lot, scrapyard, shanties, left to the ground) instead of its lots, and may put a billboard on the side facing the denser city. After the lots, the belt's sidewalks and alleys become gravel, and lamps are spaced along its roads. The Cyberspace stage, for a city on its site, fades the grid's dots and tint in the belt by the density, takes the open Net (`overworld::net_ground::region`) for every cell past the outline, and lays one stolen line from a shanty to the nearest lane. Lone cities are unchanged.

**Tech Stack:** Rust; citygen's stages; the street's terrains and features (`Feature::FireBarrel` new, appended).

**Spec:** `docs/superpowers/specs/2026-11-04-cities-in-the-world-design.md` ("The outskirts", "The Net"); round 2 (`ow2.py` `city_edge`, `net_edge`) is the look.

## Global Constraints

- Deterministic and portable; layouts from world-wide hashes of the block's corner; lone-city fingerprints (`crates/citygen/tests/seeds.rs`) unchanged.
- Glyphs the window draws; a new feature is appended to `Feature` and `ALL_FEATURES` (saves keep opening).
- A city on its site still builds over 300 worlds' home cities, with its home and its story cast (#19's tests stay green).

## Review Focus

- The belt is a belt: outskirts blocks only where the city thins (never in the core), and the core keeps full blocks. Test in Task 1.
- A body shop or shanty is reachable on foot (its door opens on walkable ground). Test in Task 1.
- The Net stays whole: every jack still reaches the city's grid and the reach check holds for every home city. Test in Task 2.
- Past the outline the city's Net is the open Net, cell for cell. Test in Task 2.
- The highways' trunks meet the city's grid at the outline. Test in Task 2.

---

### Task 1: The outskirts on the street

**Files:** `crates/citygen/src/generate/site.rs` (density, `OUTSKIRTS`, `Site::outskirts(p)`), new `crates/citygen/src/generate/outskirts.rs` (the four layouts, billboards, the belt's gravel and lamps), `lots.rs` (an outskirts block goes to it), `tile.rs` (`Feature::FireBarrel`), `crates/strata-zero/src/render/tileset.rs` (glyph `•`, orange, flickering; lit like a lamp at night).

- [ ] Tests: `outskirts_lie_where_the_city_thins` (every outskirts block's middle is below `OUTSKIRTS`; the blocks of the densest district have none); `the_outskirts_have_their_kinds` (over the home cities of seeds 3, 8 and 41 together: a body shop, a fenced scrapyard, a shanty with a fire barrel by it, a billboard); `an_outskirts_door_is_reached_on_foot` (every shanty and body-shop door has a walkable cell outside it reached from the main street); `the_belt_has_no_sidewalks` (no Sidewalk cell where the density is below `OUTSKIRTS` - 0.05); `a_lone_city_has_no_outskirts` (fingerprints).
- [ ] Implement; commit `citygen: the outskirts where a city thins (#20)`.

### Task 2: The outskirts in the Net

**Files:** `crates/citygen/src/generate/cyberspace.rs` (a city on its site: fade, open Net past the outline, the stolen line), `connectivity.rs`/`validate.rs` (a sited city's Net reach counts the city's own cells).

- [ ] Tests: `the_grid_fades_through_the_outskirts` (lattice dots per cell in the belt under half the core's); `past_the_outline_the_net_is_the_open_nets` (cells outside the outline equal `net_ground::region` of the block); `a_trunk_meets_the_citys_grid` (a NetLane trunk cell next to a walkable city Net cell inside the outline, for every highway into the block); `a_shanty_splices_a_line` (one NetLane run from a shanty cell to a lane); `every_jack_still_reaches_the_grid` (the home cities of seeds 0..8 build and validate).
- [ ] Implement; commit `cyberspace: the outskirts in the Net (#20)`.

### Task 3: Shots, checks, review

- [ ] Shots: the edge on the street by day and night, the same edge in the Net; the five checks; a fresh reviewer; one fix pass whose commit `Closes #20`.
