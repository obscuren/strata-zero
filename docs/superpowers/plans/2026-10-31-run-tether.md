# Run Tether Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A calm run leaves the footer and hangs off its target on the map as a live wire with a small label.

**Architecture:** The net keeps an echo of the last roll; `net::tether_view` turns the state into a `TetherView` (as `turn_view` does for the turn column); `render/tether.rs` places it (left or right of the target) and paints it as an `Overlay`. The map screen pushes the overlay before the shapes and keeps the footer's RUN row for fights only.

**Tech Stack:** Rust, ratatui cells, the existing `Overlay` trait.

**Spec:** `docs/superpowers/specs/2026-10-31-run-tether-design.md`

## Global Constraints

- Calm only: the footer keeps `RUN …` in a fight, and in calm whenever no tether is drawn.
- Box-drawing and block characters only: `━ ╌ ╺ ╸ █`.
- Wire: `clamp(4 × work, 8, 16)` cells, never below one a turn when shortened.
- Hit `ROLL_TICKS`, slip `MARK_TICKS`, landing `BEAT_TICKS` (half full, half reeling in).
- Past tense: cracked, spoofed, blinded, copied, planted, hit (attack), raised (shield, mask).
- Colours: lit `CYAN` (0,229,255), unlit (23,67,74), pulse (230,253,255) with the cell behind (127,243,255), slip `MAGENTA` (255,0,200), landing (80,230,120) on (15,58,31) at the target, label background (10,11,16), dim text (160,170,185).
- Build into `--target-dir target/smoke`; check both front ends (CLAUDE.md).

## Review Focus

- A target on the map's top row: the label's first line goes under the wire, not off the map.
- You on the target's row beside it: the wire takes the other side.
- A piece of ICE in sight on the wire's path: the wire leaves its cell alone.
- The label changing on landing (`CRACK  60%` to `CRACKED`): the side does not flip.
- A run on yourself (shield, mask): the wire leaves from `@` and `@` is not recoloured.

---

### Task 1: The op's past tense

**Files:**
- Modify: `crates/game/src/model.rs` (`impl Op`)

**Interfaces:**
- Produces: `Op::done_label(self) -> &'static str`

- [ ] **Step 1: Test** `every_op_has_a_past_tense` in model.rs's tests: `Op::Crack` "cracked", `Spoof` "spoofed", `Blind` "blinded", `Copy` "copied", `Backdoor` "planted", `Attack` "hit", `Shield` "raised", `Mask` "raised"; and every `Op::ALL` entry non-empty.
- [ ] **Step 2: Run** `cargo test -p game --target-dir target/smoke every_op_has_a_past_tense`: fails to compile.
- [ ] **Step 3: Implement** `done_label` beside `label`.
- [ ] **Step 4: Run** the test: passes.

### Task 2: The tether's view and its painting

**Files:**
- Create: `crates/strata-zero/src/render/tether.rs` (+ `pub mod tether;` in `render/mod.rs`)

**Interfaces:**
- Produces:
  - `pub enum TetherState { Running, Hit, Slip, Landed { age: u64 } }`
  - `pub struct TetherView { target: Pos, me: Pos, keep: Vec<Pos>, on_self: bool, op: String /* "CRACK" */, done: String /* "CRACKED" */, odds: u8, name: String, turns: u8 /* landed */, work: u8, phase: f32, state: TetherState }`
  - `pub fn wire_cells(work: u8) -> u16`
  - `pub struct Layout { pub left: bool, pub cells: u16, pub label_row: u16 }`
  - `pub fn layout(v: &TetherView, area: Rect, origin: Pos, on_map: impl Fn(Pos) -> bool) -> Option<Layout>`
  - `pub fn paint(v: &TetherView, lay: &Layout, area: Rect, origin: Pos, buf: &mut Buffer)`
  - `pub struct TetherOverlay<'a> { pub view: &'a TetherView, pub layout: Layout }` implementing `Overlay` (paints; the map screen computes the layout with `map.tile(p).is_some()` and hands the footer whether it fit).
  - `pub const LAND_TICKS: u64 = crate::net::BEAT_TICKS`

Layout: label width `max(op + 6, done, name)` (stable across the landing); per side the needed room is `cells + 3 + width` (cap, gap, label, a cell clear of the edge); room left is `tx − area.x`, right `area.right() − 1 − tx`; shorten `cells` to fit, down to `work`. Cost per side: cells covered by wire, cap, gap and label (both rows) with `on_map` true, plus 1000 for a wire or label cell on `me` (unless on yourself, where the wire starts beside you) or a `keep` cell. Cheaper side wins; a tie goes to more room. Label rows: line one at `ty − 1`, or `ty + 1` on the area's top row.

Paint: wire cell `i` counted from the cap; lit `i < round(turns × cells / work)`; the fresh stretch on a hit is `[round((turns−1)×cells/work), round(turns×cells/work))`; the pulse at `round(phase × (cells−1))` unless the state is not `Running`; reeling: in the second half of `LAND_TICKS` only the first `cells × (LAND_TICKS − age) / (LAND_TICKS / 2)` cells show (clamped to `cells`); `keep` cells and `me` are never written.

- [ ] **Step 1: Tests** in tether.rs:
  - `the_wire_is_four_cells_a_turn_between_eight_and_sixteen`: `wire_cells(1)=8, (2)=8, (3)=12, (4)=16, (5)=16`.
  - `it_reaches_off_the_construct`: area 80×26 at (0,0), origin (0,0), `on_map = |p| p.x >= 30`, target (33,10), me (33,9) → `left`, `cells == 12`, `label_row == 9`.
  - `it_takes_the_side_away_from_you`: `on_map = |_| false`, target (40,10), me (39,10) → right.
  - `it_leaves_ice_alone_and_keeps_its_side_on_landing`: a keep cell at (37,10), target (40,10), me (40,9), `on_map` false → right; the same view with `Landed { age: 0 }` gives the same layout.
  - `it_shortens_to_fit_and_gives_up_when_it_cannot`: area 33 wide, target x=17, work 3 → left, `cells` 3; area 20 wide, target x=10 → `None`.
  - `on_the_top_row_the_label_goes_under`: target y = area.y → `label_row == area.y + 1`.
  - `a_run_mid_way_paints_its_wire_label_and_pulse`: work 3, turns 1, phase 1.0, left layout: row has `CRACK  60%` and `door lock`, one `╺`, four `━` in cyan, seven `╌`, one `█`; the target keeps its glyph in cyan bold.
  - `a_hit_flashes_its_stretch_and_the_target`: turns 2, `Hit`: cells 4..8 fg white, target bg cyan, no `█`.
  - `a_slip_turns_the_unlit_wire_magenta`: `Slip`: every `╌` magenta, no `█`.
  - `a_landing_goes_green_then_reels_in`: `Landed { age: 0 }`: `CRACKED`, twelve green `━`; `age: LAND_TICKS * 3 / 4` → six `━`; `age: LAND_TICKS - 1` → at most one.
  - `on_yourself_the_at_sign_is_left_alone`: `on_self`, target = me: the `@` cell unchanged.
- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke render::tether`: fails to compile.
- [ ] **Step 3: Implement** tether.rs.
- [ ] **Step 4: Run** the tests: pass.

### Task 3: The roll's echo and the view from the state

**Files:**
- Modify: `crates/strata-zero/src/net.rs` (NetState, `step_run`, `run_label`, `run_readout`, `beat_bar`)

**Interfaces:**
- Consumes: `Op::done_label`, `TetherView`, `TetherState`, `LAND_TICKS`.
- Produces:
  - `pub enum Roll { Hit, Slip, Land }`, `pub struct RollEcho { pub at: u64, pub roll: Roll, pub building: BuildingId, pub run: Run }`, `NetState.echo: Option<RollEcho>` (set in `step_run` for every non-attack roll that does not dump; `run` as it stands after the roll, `done == work` on a landing; `at` is `st.tick`).
  - `pub fn tether_view(st: &AppState, place: Place) -> Option<TetherView>`: none unless the player stands in `place`, a construct, calm; from the live run (not a guard), state `Hit`/`Slip` while an echo of the same program and target is younger than `ROLL_TICKS`/`MARK_TICKS`; else from a `Land` echo of this construct younger than `LAND_TICKS`.
  - `pub fn beat_phase(st: &AppState) -> f32` replacing `beat_bar`.
  - `run_readout` without the calm tail; `target_label` and `run_odds` split out of `run_label`/`run_readout` and shared.

- [ ] **Step 1: Tests** in explore.rs's tests (they have the construct helpers):
  - `a_calm_run_hangs_its_tether_off_the_lock`: `lock_run`, `start_run(crack, lock)`, `.` → `tether_view` some, target the lock, `turns` 1, `Running`; drawn 120×40: contains `CRACK` and `door lock`, not `RUN  crack`.
  - `a_landed_run_holds_its_tether_for_a_beat`: three rolls on `force_roll 10` → `st.net.run` none, view `Landed`, label `CRACKED`; after `LAND_TICKS` more ticks, none.
  - `a_slip_shows_on_the_tether`: `force_roll 1` → view `Slip`.
  - In `the_run_keeps_its_progress_across_calm_and_turns`: after `e`, the view is none and the footer reads `RUN  crack · door lock`.
  - `a_calm_run_with_no_tether_stays_in_the_footer`: a calm run on a guard → no view, footer `RUN  crack · grey ICE  ▰▱▱ 1/3`.
- [ ] **Step 2: Run** them: fail.
- [ ] **Step 3: Implement** the echo, `tether_view`, `beat_phase`; trim `run_readout`.
- [ ] **Step 4: Run** them: pass (the drawing ones wait for Task 4).

### Task 4: The map draws it; the footer keeps RUN for fights

**Files:**
- Modify: `crates/strata-zero/src/screens/explore.rs` (draw: overlay before `FxOverlay`; footer `run` only when engaged; tests at the old RUN assertions), `crates/strata-zero/src/ui/hud.rs` (the `run` field's doc and its test string)

- [ ] **Step 1:** Update the tests that read RUN in calm: `a_run_in_flight_shows_its_turn_hint_and_a_continue_row` and `the_strip_reads_the_run_the_heat_and_the_hunt` look for the label on the map instead; the four-rows test expects row 38 blank in calm; `the_beat_bar_fills_toward_the_next_roll` becomes `the_pulse_rides_toward_the_next_roll` on `beat_phase` (0.0, then 0.5 half a beat on) and the drawn `█`.
- [ ] **Step 2: Implement** the wiring.
- [ ] **Step 3: Run** `cargo test --workspace --target-dir target/smoke`: pass.

### Task 5: Shots

**Files:**
- Modify: `crates/strata-zero/src/screens/shots.rs`

- [ ] **Step 1:** `run_tether` (ignored): the Medium city, the largest corp tower with a door lock that fits 34×18, every tile remembered, beside the lock, a crack one turn in; writes `run_tether`, `run_tether_hit` and `run_tether_landed` at 120×36 and `run_tether_small` at 100×30.
- [ ] **Step 2: Run** with `SHOT_DIR` and the window feature; look at the PNGs.

### Task 6: Both front ends

- [ ] Run the five checks from CLAUDE.md; commit.
