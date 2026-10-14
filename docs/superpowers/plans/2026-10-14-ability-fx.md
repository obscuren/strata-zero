# Ability Effects Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Data-driven visual effects for active abilities: a glitch kick, an expanding wavefront that paints a scan's reveal outward, and the ping glow, with Netscan and Ping wired up.

**Architecture:** `game` gains the `Fx` vocabulary on `Ability`; firing pushes `FxInstance`s into `AppState::fx`; a new `render/fx.rs` overlay draws them; the action bar flashes the fired slot. The old `Ping`/`PingOverlay` path becomes the `Glow` shape.

**Tech Stack:** Rust 2024, ratatui, ron.

**Spec:** `docs/superpowers/specs/2026-10-14-ability-fx-design.md`

## Global Constraints

- Shapes exactly: `Kick(u64)`, `Wave { colour: (u8,u8,u8), ticks: u64 }`, `Glow { colour: (u8,u8,u8), ticks: u64 }`; `Active::Ping { radius }` only; `Effect::Ping` removed.
- Netscan fx `[Kick(4), Wave(cyan (0,229,255), 18)]`; Ping fx `[Wave(magenta (255,0,200), 24), Glow(magenta, 180)]`.
- Wave front `r - 1 < d <= r`, tail `r - 4 < d <= r - 1`, tail allowance 3 ticks, sparks one cell in six from `ｦ ｱ ﾂ ﾈ ╳ ≋`; hidden revealed tiles beyond the front draw as unseen (space on black).
- Slot inverts for 6 ticks after firing.
- Attribution lines on every commit; fmt and clippy `-D warnings` clean per commit.

## Review Focus

1. Firing netscan while jacked in (Net stratum): the wave must draw on the Net and nothing on the Street (Task 3 `fx_draws_only_on_its_stratum`).
2. A wave at the map edge: tiles outside the city are never indexed (Task 3 `wave_at_the_map_edge_is_safe`).
3. Two netscans in a row: the second's `revealed` is empty, so nothing is hidden twice (Task 2 `second_reveal_is_empty`).
4. A terminal narrower than the wave: cells off screen are skipped (Task 3, covered by `wave_at_the_map_edge_is_safe` at a small area).
5. The ping glow after its ticks: pruned and not drawn (Task 2 `prune_fx_drops_ended`).

---

### Task 1: The vocabulary in the game crate

**Files:** `crates/game/src/model.rs`, `crates/game/src/content.rs`, `crates/game/src/action.rs`, `flavor/content.ron`, `crates/cyberpunk/src/screens/deck.rs:633` (ping text).

- [ ] Tests (content.rs): netscan `fx == [Kick(4), Wave{colour:(0,229,255),ticks:18}]`, ping `fx` as spec and `kind == Active(Ping{radius:24})`; a copy of the content with `fx` on `lowlight` fails with `ContentError::FxOnPassive`. (action.rs): `Fire(ping)` → `out.effects.is_empty()`.
- [ ] Run → RED (no field `fx`).
- [ ] Implement: `Fx` enum (Clone, Debug, PartialEq, Serialize, Deserialize); `Ability.fx` with `#[serde(default)]`; `Active::Ping { radius }`; `Effect::Ping` removed and `apply` maps `Ping` to no effect; validation; content.ron; deck text `ping: lights terminals and stairs within {radius} tiles`.
- [ ] Run `cargo test -p game` → GREEN; commit `feat(game): ability fx vocabulary`.

### Task 2: Instances in the app state

**Files:** `crates/cyberpunk/src/walker/mod.rs` (`reveal -> Vec<Pos>`), `crates/cyberpunk/src/app.rs` (`FxInstance`, `fx`, `prune_fx`, remove `Ping`), `crates/cyberpunk/src/actions.rs`, `crates/cyberpunk/src/render/items.rs` (remove `PingOverlay` + tests), `crates/cyberpunk/src/screens/explore.rs` (remove the ping overlay use).

- [ ] Tests: walker `reveal_returns_newly_revealed_then_nothing`; actions `netscan_pushes_kick_and_wave_with_revealed`, `ping_pushes_wave_and_glow`; app `prune_fx_drops_ended` and `until` per shape.
- [ ] RED → implement → GREEN; commit `feat(app): ability fx instances`.

### Task 3: The overlay

**Files:** create `crates/cyberpunk/src/render/fx.rs`; `render/mod.rs`; `screens/explore.rs` (push `FxOverlay`, call `prune_fx` in update).

- [ ] Tests: `wave_front_colours_and_hides_unreached_tiles`, `kick_scatters_then_stops`, `glow_marks_connector_out_of_sight`, `fx_draws_only_on_its_stratum`, `wave_at_the_map_edge_is_safe`; explore `firing_a_slot_leaves_fx_that_prune`.
- [ ] RED → implement → GREEN; commit `feat(render): ability fx overlay`.

### Task 4: Slot flash, smoke, screenshot

**Files:** `crates/cyberpunk/src/ui/hud.rs` (`Slot.flash`), `screens/explore.rs` (`slots`).

- [ ] Test `fired_slot_draws_inverted`; RED → implement → GREEN; commit `feat(hud): fired slot flashes`.
- [ ] Smoke `drive8.py`: after `1`, snapshot at +2 ticks via `capture6.py` (`fx_wave`); send.
