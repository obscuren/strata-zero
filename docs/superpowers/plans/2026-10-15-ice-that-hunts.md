# ICE That Hunts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** ICE that patrols, sees, hunts and hits by grade; attack, shield and mask programs; black ICE behind a failsafe and the game's first death screen.

**Architecture:** `citygen` grows the guard (post, position, beat, black grade) and the construct's `move_guard`. `game` grows the ops and the deck target. The app's `net.rs` gains per-piece state and `ice_turn`, run after every player turn; hits route through the existing trace, dump and character; a `death` flag switches the explore screen to a new FLATLINED screen. The cyberdeck tab gets the failsafe row; the console gets `wake`, `ice`, `failsafe`.

**Tech Stack:** Rust 2024, ratatui 0.29, ron.

**Spec:** docs/superpowers/specs/2026-10-15-ice-that-hunts-design.md

## Global Constraints

- Every commit: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`.
- Numbers exactly as the spec: integrity 3/6/10, sight 4/6/8, black from security 230, white heat 5, grey focus 10, black hp 6 + (security − 230)/5, rating off grey and black hits (min 1), failsafe at hp ≤ hp_max/4 with 30 minutes cooling, MASK_TURNS 6, LOST_TURNS 3, shield charges at most 2, attack damage 2 × rating (+1 with the black_ice flag).
- Commit attribution lines as in the session's system reminder.

## Review Focus

1. A piece hunting the player who steps into a room: the piece reaches the last seen tile, loses sight (rooms are not seen into), counts three turns, and walks back to its beat; it does not path into the room (Task 1 test).
2. A hit on the turn the player jacks out or is dumped: no second hit, no hit on a body outside the Net, no panic from a missing construct (Task 2 test).
3. Black ICE with the failsafe on and HP already under a quarter: the first hit dumps rather than kills (Task 3 test).
4. A broken piece's tile is walkable, and when its window ends it stands again at its current position with full integrity, not at its post (Task 2 test).
5. Two pieces hunting the same tile never share a tile; one waits (Task 1 test).

## File Structure

- `crates/citygen/src/tile.rs` — `IceGrade::Black`, `integrity()`, `sight()`.
- `crates/citygen/src/construct.rs` — `Guard { post, pos, grade, beat }`, generation, `move_guard`, validation.
- `crates/game/src/model.rs` — `Op::{Attack, Shield, Mask}`.
- `crates/game/src/run.rs` — `Target::Deck`, `HackTarget::Deck`, `applies`, `skill_for`.
- `flavor/content.ron` — attack, shield, mask; the netrunner kit.
- `crates/strata-zero/src/net.rs` — `Ice`, `IceMode`, `ice_turn`, hits, attack landing, shield, mask, failsafe, death.
- `crates/strata-zero/src/screens/flatlined.rs` (new) — the death screen.
- `crates/strata-zero/src/screens/explore.rs` — `ice_turn` after turns, the death switch, RUN rows for Deck programs.
- `crates/strata-zero/src/ui/sidebar.rs`, `render/tileset.rs` — ICE lines, colours.
- `crates/strata-zero/src/screens/deck.rs` — failsafe row.
- `crates/strata-zero/src/screens/console.rs` — `wake`, `ice`, `failsafe`.
- `README.md`.

---

### Task 1: Pieces that patrol, see and hunt

**Files:** tile.rs, construct.rs, net.rs, explore.rs (call `ice_turn` where `turn_ends` was), sidebar.rs, tileset.rs.

**Interfaces (Produces):**
```rust
impl IceGrade { pub fn integrity(self) -> u8; pub fn sight(self) -> i32; }
pub struct Guard { pub post: Pos, pub pos: Pos, pub grade: IceGrade, pub beat: Vec<Pos> }
impl Construct { pub fn move_guard(&mut self, index: usize, to: Pos); }
pub enum IceMode { Asleep, Awake { seen: Pos, lost: u8 } }
pub struct Ice { pub mode: IceMode, pub integrity: u8, pub step: usize, pub forward: bool }
impl NetState { pub ice: BTreeMap<BuildingId, Vec<Ice>>; pub mask: u8; pub fn ice_of(&mut self, k: &Construct) -> &mut Vec<Ice>; pub fn wake_all(&mut self, b: BuildingId, seen: Pos); }
pub fn sees(k: &Construct, from: Pos, grade: IceGrade, player: Pos, masked: bool) -> bool;
pub fn ice_turn(st: &mut AppState);   // sight, act, hits (hits land in Task 2; here a hit logs and does nothing else)
```
- [ ] **Step 1: Failing tests** (construct.rs: grades incl. black at 230 and 255, beats are walkable spine, gate beat ends at the middle and the vault beat starts there, `move_guard` moves the feature; net.rs/explore.rs: an asleep grey piece steps along its beat each wait and turns at the end; `sees` true on the spine within 6, false at 7, false in a room, true at 3 and false at 4 when masked; a seen player wakes it and it closes one tile a wait; stepping into a room and waiting three times puts it back asleep and walking home (Review Focus 1); two pieces hunting never share a tile (Review Focus 5); the alarm wakes both; HERE reads `grey ICE · hunting`).
- [ ] **Step 2: Run** — failures.
- [ ] **Step 3: Implement.** Replace `turn_ends` with `ice_turn` in `handle_walk` and `Pick::Continue`.
- [ ] **Step 4: Run** the workspace — green (the old awake-guard dump test changes to expect a hit log; the hit's effect comes in Task 2).
- [ ] **Step 5: Commit** `feat: ICE patrols the spine, sees along it and hunts`

---

### Task 2: Combat: hits by grade, attack, shield

**Files:** model.rs, run.rs, content.ron, net.rs, explore.rs (RUN rows for Deck targets: `shield · 100% · 1 turn · noise 1`), deck.rs program card targets text.

- [ ] **Step 1: Failing tests** (game: `applies(Attack, Guard)`, `applies(Shield, Deck)`, `!applies(Attack, Node)`, `skill_for`; app: white hit adds 5 heat; grey hit takes 10 − rating focus and dumps at zero with `grey ICE drained you`; a shield charge absorbs one hit; attack on a grey piece with forced rolls takes 2 a turn and breaks it at 0 with `grey ICE broken`, the tile walkable, `sync_guards` keeping it down, and after the window it stands where it was with 6/6 (Review Focus 4); a hit on the turn the player jacks out does nothing (Review Focus 2)).
- [ ] **Step 2: Run** — failures.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the workspace — green.
- [ ] **Step 5: Commit** `feat: ICE hits by grade; attack and shield`

---

### Task 3: Black ICE, the failsafe, FLATLINED

**Files:** net.rs, flatlined.rs (new), screens/mod.rs, explore.rs, deck.rs (failsafe row), app.rs if the state needs it.

- [ ] **Step 1: Failing tests**: black hit takes 6 + (security − 230)/5 − rating; failsafe on at hp ≤ max/4 dumps hard with 30 minutes cooling and keeps the hp (Review Focus 3: already under a quarter → dump, not death); failsafe off → hp 0 → `net.death` set and the explore update returns `Switch` to a screen whose text contains `FLATLINED` and the cause; any key on it switches to the title; the cyberdeck row toggles with space and reads both states.
- [ ] **Step 2: Run** — failures.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the workspace — green.
- [ ] **Step 5: Commit** `feat: black ICE, the failsafe and FLATLINED`

---

### Task 4: Mask, console, docs, smoke, screenshots

- [ ] **Step 1: Failing tests**: mask landing sets 6 turns, halves sight, adds one heat a turn and ends; console `wake` wakes all, `ice` lists `0 grey asleep 6/6 at 2,5`, `failsafe off` toggles.
- [ ] **Step 2–4:** implement, README (ICE grades, hunting, combat, the failsafe, death), smoke `drive15.py` (Hextek Holdings: enter, wait until the grey gate guard sees and hits, shield absorbs, attack breaks it; console `failsafe off`, black vault guard → FLATLINED → title), screenshots of a hunt and FLATLINED.
- [ ] **Step 5: Commit** `feat: mask; docs for ICE that hunts`.

## Self-review

Spec coverage: ICE model and patrols (1), sight and stealth (1, 4), combat and programs (2), black/failsafe/death (3), reading it (1, 3), console (4), docs (4). Review Focus pinned in 1, 2, 3, 2, 1. Names consistent: `Guard.post/pos/beat`, `move_guard`, `Ice`, `IceMode`, `ice_turn`, `sees`, `wake_all`, `Target::Deck`, `HackTarget::Deck`, `net.death`, `Flatlined`.
