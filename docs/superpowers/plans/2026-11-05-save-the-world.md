# Save the World Implementation Plan (#25)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The run's snapshot holds the world: the overworld as its params (rebuilt on load), where you are on it, every city you've been to, the open ground you've seen, the place you marked; loading puts you back where you were, wherever that was, and nothing of the home city is lost on the way.

**Architecture:** Most of it is in place from #22, #21 and #24 (`Outside` saved with `params`, `at`, `seen`, `here`, `mark`; the home city in `st.city` or `st.away`). This slice adds `Outside.visited` (the cities and towns you've been into, `#[serde(default)]`; their maps are rebuilt from the world when you go back, the same every time), shows it on the MAP's world card, makes `outside::reopened` put the home city back when the world can't be made again while you were away, and lets the run's card (LOAD GAME, CONTINUE) read the home story set aside. The open ground's tiles are rebuilt from the seed; nothing the player does changes them yet, so there is no layer of changes to keep (the first thing that changes the ground brings one).

**Spec:** `docs/superpowers/specs/2026-11-04-overworld-screen-design.md` ("Saving"), `2026-11-04-world-tiles-design.md` ("Kept tiles"); `docs/saving.md`.

## Global Constraints

- docs/saving.md: new saved fields `#[serde(default)]`; the fixtures in `runs/fixtures/` keep opening.
- The overworld itself is never written into the save (its params are).

## Review Focus

- A run reopens where it was: on the overworld, on the open ground, jacked in out there (on the body), in the home city, in another city, in a town. Test in Task 1.
- A world that can't be made again while you were away puts you back in the home city with its story. Test in Task 1.
- The run's card shows the home story while you're away. Test in Task 1.
- The save doesn't hold the world's tiles. Test in Task 1.
- Old saves (no `visited`) open. Test in Task 1 (the fixtures).

---

### Task 1: The world kept with the run

**Files:** `crates/strata-zero/src/outside/mod.rs` (`visited`, `reopened`), `screens/atlas/world.rs` (the card says you've been), `runs/describe.rs` (the story from `away`).

- [ ] Tests: `a_run_reopens_where_it_was_wherever_that_was`; `the_cities_you_have_been_to_are_kept`; `a_world_that_wont_come_back_puts_you_home`; `the_runs_card_tells_the_home_story_while_away`; `the_save_holds_the_worlds_params_not_its_tiles`.
- [ ] Implement; commit `runs: the world kept with the run (#25)`.

### Task 2: Checks, review

- [ ] The five checks; a fresh reviewer; one fix pass whose commit `Closes #25`.
