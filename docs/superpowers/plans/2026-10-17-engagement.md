# Engagement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Net runs on a beat until ICE wakes, then on turns with MOVE and AP, an aimed path, animated walking and bolts; a four-row footer carries it.

**Architecture:** `net.rs` splits `ice_turn` into look/step, adds the beat and the `Engagement` state machine with `spend_ap`/`spend_move`; `explore.rs` drives the beat, the ICE pace and the player's walk from `update`, routes keys through the budgets, and owns the aim cursor; `hud.rs` builds the four footer rows; `render/path.rs` draws the aim; bolts ride the existing `Fx` system.

**Tech Stack:** Rust 2024, ratatui 0.29.

**Spec:** docs/superpowers/specs/2026-10-17-engagement-design.md

## Global Constraints

- Every commit: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`.
- Numbers as the spec: `BEAT_TICKS` 45, `PACE_TICKS` 8, `WALK_TICKS` 4, `ROLL_TICKS` 12, `BASE_MOVE` 4, `BASE_AP` 2, ICE budgets white 0/1, grey 2/1, black 3/2, initiative black 3 grey 2 white 1, footer four rows, map `h-10`.
- Commit attribution lines as in the session's system reminder.

## Review Focus

1. A beat that fires while a popup was just closed: at most one beat fires per update even if several `BEAT_TICKS` passed (no burst of rolls after a long menu) (Task 2 test).
2. The player walks an aimed path and a piece wakes mid-walk: the walk stops at the tile reached, the engagement starts with the moves left as spent, no double step (Task 4 test).
3. A dump or FLATLINED during an ICE turn: the engagement ends, no further actor acts, the pace timer does not touch a player who is on the street (Task 3 test).
4. `e` pressed during an ICE turn's animation: refused (`still hunted` or `not your turn`), nothing changes (Task 3 test).
5. The footer at 100 columns engaged with three pieces in sight and a long order: every row is one line, cut, never wrapping into the next (Task 1 test).

## File Structure

- `crates/strata-zero/src/ui/hud.rs` — `Footer` input struct and `footer_rows(&Footer, width) -> [Line; 4]`; `layout` in explore gains `footer: [Rect; 4]`.
- `crates/strata-zero/src/net.rs` — `BEAT_TICKS`, `beat`, `Engagement`, `Actor`, `engage`, `disengage`, `spend_ap`, `spend_move`, `ice_look`, `ice_step`, `ice_pace`, `initiative`.
- `crates/strata-zero/src/screens/explore.rs` — the beat and pace in `update`, `e`, `.`, budgets on keys, `aim: Option<Pos>`, `walking: Option<Walk>`, the path overlay push.
- `crates/strata-zero/src/render/path.rs` (new) — `PathOverlay`.
- `crates/strata-zero/src/actions.rs`, `net.rs` — bolts on rolls and hits.
- `README.md`, `screens/help.rs`.

---

### Task 1: The four-row footer

**Interfaces (Produces):**
```rust
pub struct Footer<'a> { pub place: String, pub jacked: bool, pub camera: bool, pub overburdened: bool, pub trace: Option<String>, pub hunted: bool, pub turn: Option<String>, pub run: Option<String>, pub ice: Vec<String>, pub slots_label: &'a str, pub slots: &'a [Slot], pub hollow: bool, pub budgets: Option<(u8, u8, u8, u8)> }
pub fn footer_rows(f: &Footer, width: u16) -> [Line<'static>; 4];
struct Regions { ..., footer: [Rect; 4] }   // explore
```
- [ ] **Step 1: Failing tests** (hud.rs, explore.rs): `footer_rows` on foot: row 1 `on foot · Night Market` and `[?]` at the right end and no `HERE`, rows 2–3 blank, row 4 `ABILITIES  [1] netscan ●`; jacked calm with a run: row 1 `⚡ …  TRACE …`, row 3 starts `RUN  crack · door lock` and holds `next roll`, row 4 `PROGRAMS`; engaged: row 3 `TURN 3  YOU › grey ICE › black ICE`, row 4 ends `MOVE ▰▰▰▱ 3/4    AP ●● 2/2`, hollow keys `○` during an ICE turn, `HUNTED` in row 1; at width 100 with three pieces every row is one line (Review Focus 5); `layout(100×30)`: `bottom_rule.y == 25`, `footer[0].y == 26`, map height 20; the explore screen draws the rows and no `[space]` hint anywhere; the console line takes row 4; help lists `space`, `enter`, `v`, `m`, `tab`, `e`, `.`.
- [ ] **Step 2: Run** — failures.
- [ ] **Step 3: Implement.** The old `footer_status`/`footer_abilities`/`*_VERBS` go.
- [ ] **Step 4: Run** the workspace — green (update every test that read the old strip).
- [ ] **Step 5: Commit** `feat(app): a four-row footer with the fight in it; hints move to help`

---

### Task 2: The beat

**Interfaces (Produces):**
```rust
pub const BEAT_TICKS: u64 = 45;
impl NetState { pub beat: u64; /* tick of the last beat */ }
pub enum Step { Moved(Pos), Hit, Nothing }
pub fn ice_look(st: &mut AppState, b: BuildingId, i: usize);        // sight → mode, stun countdown
pub fn ice_step(st: &mut AppState, b: BuildingId, i: usize) -> Step; // one patrol/hunt step or a hit
pub fn beat(st: &mut AppState);                                      // step_run + one look+step per piece
pub fn beat_bar(st: &AppState, cells: usize) -> String;              // ▓▓░░ toward the next beat
```
- [ ] **Step 1: Failing tests**: in a construct, `update` with `st.tick += BEAT_TICKS` rolls the run once and patrols asleep ICE one tile; two beats' worth of ticks fire one beat (Review Focus 1); no beat on the street or the grid; `.` fires a beat at once; a popup frame (no update) fires none; a beat that wakes a piece sets `net.engaged` (Task 3 wires the rest; here assert the wake); `beat_bar` fills with the phase; `ice_turn` behaviour is preserved by `beat` (the spec-2 tests still pass).
- [ ] **Step 2–5:** implement, gate, commit `feat(app): programs run on a beat in a calm construct`.

---

### Task 3: Turns

**Interfaces (Produces):**
```rust
pub enum Actor { You, Ice(usize) }
pub struct Engagement { pub round: u32, pub order: Vec<Actor>, pub turn: usize, pub moves: u8, pub ap: u8, pub manual: bool, pub acting_since: u64 }
pub const BASE_MOVE: u8 = 4; pub const BASE_AP: u8 = 2; pub const PACE_TICKS: u64 = 8;
impl NetState { pub engaged: Option<Engagement>; }
pub fn engage(st: &mut AppState, b: BuildingId, manual: bool);
pub fn disengage(st: &mut AppState);
pub fn initiative(st: &AppState, b: BuildingId) -> Vec<Actor>;
pub fn budgets(grade: IceGrade) -> (u8, u8);
pub fn spend_ap(st: &mut AppState) -> Result<(), String>;      // Err("no AP left") / not your turn
pub fn spend_move(st: &mut AppState) -> Result<(), String>;
pub fn end_turn(st: &mut AppState);                            // next actor; rounds; disengage when calm
pub fn ice_pace(st: &mut AppState);                            // one ICE sub-action per PACE_TICKS, from update
pub fn your_turn(st: &AppState) -> bool;
```
- [ ] **Step 1: Failing tests**: a wake during a beat engages with the order by Net then grade (black, grey) and ties to you; budgets by grade; your keys spend: a step costs a move, a program turn an AP, the fifth step says `no moves left`, the third AP `no AP left`; `.` ends your turn; the ICE turn steps one tile per `PACE_TICKS` toward you and hits per AP beside you, black twice; a stunned piece's turn is skipped and the stun falls; a round with nothing awake disengages unless manual; `e` engages by hand in calm and is refused while hunted and during an ICE turn (Review Focus 4); jack out disengages; a dump during an ICE turn disengages and no actor touches the player after (Review Focus 3); the run keeps `done` across engage/disengage.
- [ ] **Step 2–5:** implement, gate, commit `feat(app): ICE that wakes puts the construct on turns`.

---

### Task 4: Aim, path and the walk

**Interfaces (Produces):**
```rust
pub struct PathOverlay<'a> { pub path: &'a [Pos], pub lit: usize, pub reachable: bool }
pub const WALK_TICKS: u64 = 4;
struct Walk { path: Vec<Pos>, next_at: u64 }   // explore
pub fn path_to(k: &Construct, from: Pos, to: Pos) -> Option<Vec<Pos>>;   // net.rs, A* over walkable tiles minus standing pieces
```
- [ ] **Step 1: Failing tests**: engaged, `l` moves the aim one tile and the overlay draws `·` on the path's lit tiles and `×` at the end; a tile beyond the moves is dim; an unreachable tile draws the cursor red; `enter` starts the walk: after `WALK_TICKS` the player is one tile along, keys are ignored until the walk ends, moves spent per tile; a walk out of the target's reach aborts the run; `esc` drops the aim; a wake mid-walk stops the walk at the tile reached (Review Focus 2); in calm the aim and walk work and cost nothing.
- [ ] **Step 2–5:** implement, gate, commit `feat(app): aim a path and walk it`.

---

### Task 5: Bolts, docs, smoke

- [ ] **Step 1: Failing tests**: a program turn pushes a `Fx::Bolt` from the player to the target with `ROLL_TICKS`; a slip pushes a magenta `Mark`; an attack turn a white bolt; a hit a bolt from the piece to the player, red for black; a shield absorbing a cyan mark; the beat's roll bolts too.
- [ ] **Step 2–5:** implement, README (the beat, turns, budgets, aim, `e`, the footer), `drive19.py` (calm crack landing on beats; a wake; a turn with an aimed walk; a hit; `.`; `e` refused then allowed), screenshots of the three footer states; commit `feat: bolts for every roll and hit; docs for engagements`.

## Self-review

Spec coverage: footer (1), beat (2), turns and `e` (3), aim/walk (4), bolts and docs (5). Review Focus pinned in 2, 4, 3, 3, 1. Names consistent: `Footer`, `footer_rows`, `BEAT_TICKS`, `beat`, `ice_look`, `ice_step`, `Engagement`, `Actor`, `engage`, `disengage`, `spend_ap`, `spend_move`, `end_turn`, `ice_pace`, `PathOverlay`, `Walk`, `path_to`.
