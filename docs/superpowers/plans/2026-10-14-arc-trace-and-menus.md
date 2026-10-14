# Arc Trace and Menus Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the Arc Trace ability with Bolt and Mark effect shapes, and land the listed menu, title, pause, deck and dev-tab fixes.

**Architecture:** The game crate gains `Active::Trace` and two `Fx` shapes; firing collects node targets into the instance; the fx overlay draws snaking lines and pulsing marks. Lists wrap through their own navigation helpers. The deck's tab list is reordered with two new tabs and its position lives in the app state.

**Spec:** `docs/superpowers/specs/2026-10-14-arc-trace-and-menus-design.md`

## Global Constraints
- Shapes: `Bolt { colour, ticks }`, `Mark { colour, ticks }`; `Active::Trace { radius }`; targets per spec.
- Tab order and keys exactly as the spec; deck memory in `AppState`.
- Attribution lines; fmt and clippy `-D warnings` clean per commit.

## Review Focus
1. Arc Trace fired with no node in range: no Bolt lines, no Marks, no panic (Task 2 test).
2. A target off screen: the Bolt line stops at the viewport edge without indexing outside (Task 2 test).
3. Deck memory for a tab whose rows shrank (an item dropped): the row clamps (Task 4 test).
4. Popup wrap with every row disabled: no infinite loop (Task 3 test).
5. New game when a city already exists: the old body and effects are gone (covered by `enter_city`).

### Task 1: Trace ability and content
`Active::Trace`, `Fx::Bolt`/`Fx::Mark`, content.ron (ability + grant), deck text, game tests. Commit `feat(game): arc trace`.

### Task 2: Targets and the overlay
`FxInstance.targets`; `actions::perform` collects targets; `render/fx.rs` Bolt and Mark; tests. Commit `feat(fx): bolt and mark shapes`.

### Task 3: Lists wrap
`Menu::up/down`, `Deck::move_row`, `Popup::move_sel`, `Files`, `Settings`; tests. Commit `fix(ui): lists wrap`.

### Task 4: Title, pause, walk on entry, deck tabs and memory, dev tabs
Title items and new-game flow; pause labels; `MapScreen::walking`; deck tab order, CYBERDECK and JOURNAL, `AppState::deck_tab`/`deck_rows`; dev tab row at the bottom of the column; help/README; tests. Commit `feat(ui): deck round two`.

### Task 5: Smoke and screenshots
`drive9.py` with the new tab keys; capture the bolt mid-flight; send.
