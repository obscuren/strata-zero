# The Overworld Jacked In Implementation Plan (#23)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Jacked in on a street-level Net (the city's, or the open Net out on a detail map), the up key shows the Net overworld round you in round 1's D look; ↵ (or Esc) goes back down to where you were on the Net; it can be looked at, not walked, until the Net's reach is decided (#27).

**Architecture:** `render::overworld::NetAtlas` draws a world tile jacked in (trunks along the highways in heavy lines with packets moving on them, the cities as node fields in their districts' colours, each POI a node by its tier with a dashed spur to its trunk, the dead grid between). `OutsideScreen` gains a look-only Net-overworld mode (`net_up: Option<NetUp>`), opened by `W` while jacked in on the open ground, and pushed as its own screen from the city's map by `W` while jacked in there; ↵/Esc/W close it, the walking keys say why they don't move, `J` jacks out. The panel's HERE says the Net and the trunk you're on; the minimap pane shows the same look.

**Spec:** `docs/superpowers/specs/2026-11-04-overworld-screen-design.md` ("Jacked in"); mock `docs/superpowers/specs/2026-11-04-overworld-mock/round1.html` (D), `ow.py` `draw_net`.

## Global Constraints

- Glyphs the window draws (the glyph test); exact colours; fits 100×30.
- Look-only: nothing moves you on the Net overworld; the clock doesn't run while you look.
- Jacking out always puts you back with your body.

## Review Focus

- `W` jacked in on the open ground opens it over your body's tile; ↵ comes back to the same Net cell. Test in Task 1.
- `W` jacked in on the city's Net opens it over the city's tile; ↵/Esc returns to the city's Net, still jacked in. Test in Task 1.
- The walking keys don't move you and say why. Test in Task 1.
- `J` from it jacks out onto the body. Test in Task 1.
- At 100×30 it draws without overflow. Test in Task 1.

---

### Task 1: The Net overworld

**Files:** `crates/strata-zero/src/render/overworld.rs` (`NetAtlas`, `HEAVY`), `crates/strata-zero/src/screens/outside.rs` (the mode), `crates/strata-zero/src/screens/explore.rs` (`W` jacked in pushes it).

- [ ] Tests: `the_net_over_the_world_is_trunks_nodes_and_dead_grid`; `w_jacked_in_out_here_looks_up_at_the_net`; `w_jacked_in_in_the_city_looks_up_at_the_net`; `the_net_overworld_is_look_only`; `j_from_the_net_overworld_jacks_out`; `the_net_overworld_fits_the_smallest_terminal`.
- [ ] Implement; commit `outside: the overworld jacked in (#23)`.

### Task 2: Shots, checks, review

- [ ] Shots: the Net overworld from the open ground and from the city; the five checks; a fresh reviewer; one fix pass whose commit `Closes #23`.
