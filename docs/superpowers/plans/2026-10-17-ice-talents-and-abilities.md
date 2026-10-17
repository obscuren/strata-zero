# ICE Talents And Abilities Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deep dive, Null route and The deep end do what they say; Ping, Arc trace, Signal ghost and Low-light work on the Net.

**Architecture:** `net.rs` gains `add_heat` (one door for every heat source, with the null route), the deep-end hit, the stun on `Ice`, and the ghost rule in `ice_lines`. `actions.rs` gives Ping and Arc trace a construct branch. `render/fx.rs` glows targets. `app.rs` owns `sight_bonus`.

**Tech Stack:** Rust 2024, ratatui 0.29.

**Spec:** docs/superpowers/specs/2026-10-17-ice-talents-and-abilities-design.md

## Global Constraints

- Every commit: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`.
- Numbers as the spec: null route to `ALARM_HEAT`, deep end halves and eighths with `DEEP_END_COOLING` 10, `PING_NOISE` 2, `TRACE_NOISE` 5, `STUN_TURNS` 2, low-light +2 from 18:00 to 05:59.
- Commit attribution lines as in the session's system reminder.

## Review Focus

1. Null route when the dumping turn is the one that also crosses the alarm line (heat 58 + 6): the alarm trips, the heat lands on 60, no dump, one warning (Task 1 test).
2. A stunned piece that the alarm wakes: it wakes but still does not act until the stun ends (Task 2 test).
3. Arc trace fired on the grid or the street: unchanged behaviour, no stun, no heat (Task 2 test).
4. Deep end with the failsafe off: HP can still reach zero and FLATLINE; the halving never saves a zero (Task 1 test).
5. Low-light at the day boundary: 05:59 counts, 06:00 does not; 18:00 counts (Task 3 test).

## File Structure

- `crates/game/src/character.rs` — `has_ability`.
- `crates/strata-zero/src/net.rs` — `add_heat`, `null_routed`, deep end in `hit`, `Ice.stunned`, stun in `ice_turn`, ghost in `ice_lines`, `stun_pieces`, `construct_nodes`.
- `crates/strata-zero/src/actions.rs` — Ping and Arc trace in a construct.
- `crates/strata-zero/src/render/fx.rs` — glow targets; `render/net_overlay.rs` — ghost pieces.
- `crates/strata-zero/src/app.rs` — `sight_bonus`; callers in interiors, constructs, net, actions, popup, console, explore.
- `README.md`.

---

### Task 1: Deep dive, Null route, The deep end

**Interfaces (Produces):**
```rust
pub const DEEP_END_COOLING: u64 = 10;
impl NetState { pub null_routed: BTreeSet<BuildingId>; pub fn add_heat(&mut self, b: BuildingId, noise: u8, now: u64, null_route: bool) -> (TraceEvent, bool /* routed */); }
```
- [ ] **Step 1: Failing tests** (explore.rs): a vault copy with `deep_dive` lands with heat unchanged on successes and `noise + 2` on a slip; without the flag heat rises by `noise`; null route at 98 → heat 60, warning, `null_routed` holds the building, run goes on; at 58 with noise 6 the alarm trips and no dump (Review Focus 1); the second dump is real; a white hit at 96 routes; a held mask at 99 routes; deep end: the black hit halves (security 255, armour 1: 10 → 5), failsafe at hp ≤ max/8 with `DEEP_END_COOLING`, failsafe off and hp reaching 0 still FLATLINES (Review Focus 4).
- [ ] **Step 2: Run** — failures.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the workspace — green.
- [ ] **Step 5: Commit** `feat: deep dive, null route and the deep end do what they say`

---

### Task 2: Ping and Arc trace inside a construct; Signal ghost

**Interfaces (Produces):**
```rust
pub const PING_NOISE: u8 = 2; pub const TRACE_NOISE: u8 = 5; pub const STUN_TURNS: u8 = 2;
pub struct Ice { ..., pub stunned: u8 }
pub fn construct_nodes(st: &AppState, b: BuildingId) -> Vec<Pos>;
pub fn stun_pieces(st: &mut AppState, b: BuildingId, origin: Pos, radius: i32) -> Vec<Pos>;   // the stunned pieces' tiles
impl Character { pub fn has_ability(&self, c: &Content, id: &AbilityId) -> bool }
pub struct NetOverlay { ..., pub ghost: bool }
```
- [ ] **Step 1: Failing tests**: ping in a construct → `Ping: N nodes answer`, heat +2, an fx with every node as target; on the street unchanged (no heat, connectors glow); arc trace in a construct → pieces within radius `stunned == 2`, `Arc Trace: 1 pieces stunned`/`1 piece`, heat +5, a stunned adjacent awake piece does not hit for two turns and hits on the third; the alarm wakes a stunned piece but it still does not act until the stun ends (Review Focus 2); arc trace on the grid: no stun, no heat (Review Focus 3); HERE reads `· stunned 2`; signal ghost: an awake piece out of sight is listed by `ice_lines` and drawn by the overlay, an asleep one is not, and a character without the ability sees neither.
- [ ] **Step 2–5:** implement, gate, commit `feat: ping and arc trace work inside a construct; signal ghost feels awake ICE`.

---

### Task 3: Low-light, docs, smoke

- [ ] **Step 1: Failing tests**: `AppState::sight_bonus()` with the optics on: base + 2 at 22:00, 18:00 and 05:59; base at 06:00 and 12:00 (Review Focus 5); without the optics base.
- [ ] **Step 2–5:** implement (`st.sight_bonus()` at every refresh), README (the three talents, the abilities on the Net, the passives), smoke `drive18.py` (ping and arc trace inside Hextek, a null-routed dump via the console), commit `feat(app): low-light sees at night; docs for the ICE tree`.

## Self-review

Spec coverage: talents (1), abilities and ghost (2), low-light and docs (3). Review Focus pinned in 1, 2, 2, 1, 3. Names: `add_heat`, `null_routed`, `DEEP_END_COOLING`, `PING_NOISE`, `TRACE_NOISE`, `STUN_TURNS`, `stunned`, `construct_nodes`, `stun_pieces`, `has_ability`, `ghost`, `sight_bonus`.
