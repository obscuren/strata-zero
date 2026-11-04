# The Net in the Open Implementation Plan (#18)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The open ground has its Net layer, joined across tile edges as the ground is: trunks along the highways, every POI's buildings a small construct wired to its trunk by a spur from its gate, lone nodes on light lines, and the grid fading out from signal; a runner jacks in at a POI's terminal and walks it within reach of the body.

**Architecture:** `citygen::overworld::net_ground::region(world, x0, y0, w, h)` derives the Net from world data and the ground region round the rectangle (built with a margin): trunk cells from the road centrelines, constructs from the POI rooms found in the ground, spurs and light lines from the POI's road frame and track (world data only), the grid from a distance to signal within the margin. `outside::Outside` gains `net: Option<Pos>` (where you are on the Net; your body stays on the ground cell), and the outside screen draws the Net layer while jacked in.

**Spec:** `docs/superpowers/specs/2026-11-04-world-tiles-design.md`, "The Net in the open"; round 2 (`ow2.py`) and round 3's Kilo-9 are the look.

## Global Constraints

- Deterministic: a tile's Net built alone equals the same tile in any region; world-wide hashes only.
- The city's Net terrains and features only: `NetLane`, `NetGrid`, `NetWall`, `NetFloor`, `NetGate`, `Void`; `Feature::Node(NodeKind)`.
- Walkable only within the body's tile and the eight round it (the spec's stopgap until the reach question, #27).
- Jacking in needs what the city's does (a deck); out here at a POI terminal, a cabinet or a pump.

## Review Focus

- Every construct's gate joins a trunk over lanes. Test in Task 1.
- A trunk runs unbroken along a highway across tile edges. Test in Task 1.
- Far from any road or POI the Net is near-empty (a stray dot at most). Test in Task 1.
- Jacked in, you can't walk past the body's reach, and jacking out puts you on your body. Test in Task 2.
- Saving while jacked in out here reopens jacked in, or safely on the body. Test in Task 2.

---

### Task 1: The layer

**Files:** Create `crates/citygen/src/overworld/net_ground.rs`; `detail.rs` exposes `centreline` (pub(crate)); `poi_ground.rs` exposes the road frame's spur (gate → trunk) and the relay's track.

- [ ] Tests: `a_trunk_runs_unbroken_along_a_highway`, `every_gate_is_wired_to_a_trunk`, `the_barren_net_is_near_empty`, `a_net_tile_alone_equals_its_region`, `the_grid_fades_from_signal` (grid density within 4 cells of a lane above 3 times that 20 cells off).
- [ ] Implement; commit `tiles: the Net in the open (#18)`.

### Task 2: Jacked in out there

**Files:** `crates/strata-zero/src/outside/mod.rs` (`net`), `ground.rs` (the Net view cache), `screens/outside.rs`.

- [ ] Tests: `space_by_a_terminal_jacks_in` (without a deck it says why), `the_net_out_here_ends_at_the_bodys_reach`, `jacking_out_wakes_on_the_body`, `a_run_saved_jacked_in_reopens_on_its_body` (the open Net isn't saved: you wake on the body).
- [ ] Implement; commit `outside: jack in on the open ground (#18)`.

### Task 3: Shots, checks, review

- [ ] Shots of the truck stop's Net and a barren stretch; the five checks; a fresh reviewer; one fix pass whose commit `Closes #18`.
