# Every City in the World Implementation Plan (#21)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every city on the overworld can be walked into: the home city is built at the start, any other city (and any outlying town, on a small map of its own) the first time you step onto its block, each on its site from the world's seed; while you're in another city the home city waits, set aside with the run, and comes back as you left it.

**Architecture:** A settlement is a world city or one of its towns (`outside::Settlement { city: u8, town: Option<u8> }`). citygen gains the town's tiles and block (`Overworld::town_tiles`, `town_block`) and `CitySite.town` (`#[serde(default)]`), so `sited_town` builds a town as `sited` builds a city. `Outside` keeps which settlement the loaded city is (`here`), the home city's state while you're elsewhere (`away: Option<Box<Stay>>`, saved), and the settlements' blocks (a skipped cache). A `Stay` is everything the run holds of the city it's in (the city, its people, interiors, constructs, the Net's state, the ground's items, foes, bots, the journal, jobs, the story, the track, the player); stepping onto another settlement's block on the overworld or on foot takes the current one's `Stay` out (kept only for the home city), builds the other on its site and puts a fresh `Stay` in; stepping onto the home city's block puts its `Stay` back. Away from home the story waits (the clock skips missions and cameras, as it does on the overworld).

**Tech Stack:** Rust; citygen's site and world; the outside module and the clock.

**Spec:** `docs/superpowers/specs/2026-11-04-cities-in-the-world-design.md` ("Cities in a world", "A lone city"); `2026-11-04-overworld-screen-design.md` ("Stepping onto a city tile goes straight into the city").

## Global Constraints

- Deterministic: a settlement's map is the same every time it's built from the same world.
- Only the city you're in runs its people; the home city's are set aside as they were.
- Old saves (no `here`, no `away`) open as before; new fields `#[serde(default)]`.
- Both front ends; the five checks.

## Review Focus

- Coming home after a visit, the story, the flat, jobs and tracking are as they were. Test in Task 2.
- A run saved in another city reopens there, and coming home still works. Test in Task 2.
- While away the clock never runs the story against the other city's people. Test in Task 2.
- A town or city whose map fails to build leaves you on the open ground with a line why, never stuck. Test in Task 2.
- Stepping between two overlapping blocks never loses the player. Test in Task 2.

---

### Task 1: Towns and every city on their sites

**Files:** `crates/citygen/src/overworld/mod.rs` (`town_tiles`, `town_block`), `params.rs` (`CitySite.town`), `generate/site.rs` (`sited_town`, own tiles by settlement).

- [ ] Tests: `a_town_has_a_small_map_of_its_own` (its block holds the town and not its city's centre; its map is at least 80×40; it builds); `every_city_in_a_world_builds_on_its_site` (seed 8's nine cities); `a_town_site_reads_back_without_town` (a `CitySite` saved before has `town: None`).
- [ ] Implement; commit `citygen: towns on maps of their own (#21)`.

### Task 2: Walking into another city

**Files:** `crates/strata-zero/src/outside/mod.rs` (`Settlement`, `Stay`, `here`, `away`, the blocks), `outside/moves.rs` (stepping onto a block), `clock.rs` (`away` includes another city), `runs/snapshot.rs` (`after_restore` rebuilds a stashed stay's caches).

- [ ] Tests: `stepping_onto_another_city_goes_into_it`; `coming_home_puts_the_home_city_back` (story runs, home, jobs and track equal before and after); `the_story_waits_in_another_city`; `a_run_saved_in_another_city_reopens_there`; `a_settlement_that_wont_build_leaves_you_outside`; `a_town_is_walked_into`.
- [ ] Implement; commit `outside: walk into every city in the world (#21)`.

### Task 3: Shots, checks, review

- [ ] Shots: another city's street, a town; the five checks; a fresh reviewer; one fix pass whose commit `Closes #21`.
