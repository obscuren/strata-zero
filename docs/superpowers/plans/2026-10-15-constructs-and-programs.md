# Constructs and Programs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The first playable netrunning run: constructs with an inside, programs loaded into deck slots, hacks resolved in turns against a trace that alarms and dumps, and hacks that change the real building.

**Architecture:** `game` gets the program and deck item specs, the loaded list, the run maths and the trace (terminal-free). `citygen` gets `Feature::Ice`, `Place::Construct` and a `construct` module that generates and validates a construct's inside. The app gets a `Constructs` cache and `MapRef::Construct`, gate entry and exit, a `NetState` (traces, hacks, backdoors, cooling, the running program) driven by turns and the world clock, the RUN popup, the effects on floors and lifts, the lockdown, the sidebar meter, the cyberdeck tab's loading, console commands, docs.

**Tech Stack:** Rust 2024, ratatui 0.29, ron.

**Spec:** docs/superpowers/specs/2026-10-15-constructs-and-programs-design.md

## Global Constraints

- Every commit passes `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`.
- Constants exactly as the spec: `ALARM_HEAT 60`, `DUMP_HEAT 100`, `ALERT_MINUTES 60`, `HACK_MINUTES 60`, `COOLING_MINUTES 10`, `ICE_THRESHOLD 160`, grey from 200, construct size footprint×2 clamped 14×9..40×24, `difficulty = security / 8`, `power = 2·Net + 3 (skill) + 2·rating + 2 (flag)`, roll 1..=10, success when `roll + power > difficulty`.
- Nothing about the Net's state or constructs is saved; `FORMAT_VERSION` only bumps if a saved type changes (adding `Feature::Ice` at the end of the enum does not change existing cities' bytes; verify with the save round-trip test).
- Commit attribution lines as in the session's system reminder.

## Review Focus

1. A program dropped from the pack while loaded: the loaded list no longer names it and the cyberdeck tab does not list it (Task 1 + Task 5 tests).
2. The alarm firing while a copy run is mid-way: the run is not cleared (only moves, dumps and jack-outs clear it), but every landed lock hack on that building expires, and a second alarm cannot fire while alert (Task 4 test).
3. Leaving a construct, walking to another building and coming back after the alert ended: the heat is zero, the guards stand again, the gate opens (Task 4 test).
4. A construct whose building has no door locks, cameras or vault (a bar): the inside still has the server, validates, and `copy` on the server works (Task 2 + Task 4 tests).
5. Jacking out while a run is in flight, then jacking in again: no run, no heat lost, the construct remembers its heat (Task 4 test).

## File Structure

- `crates/game/src/model.rs` — `ItemKind::Program`, `Program`, `Op`, `DeckSpec`, item fields.
- `crates/game/src/content.rs` — validation of program and deck items.
- `crates/game/src/character.rs` — `loaded`, `deck`, `loaded_size`, `can_load`, `load`, `unload`, `from_build` loading the kit, dropping unloads.
- `crates/game/src/action.rs` — `Action::Load`, `Action::Unload`.
- `crates/game/src/run.rs` (new) — `Target`, `HackTarget`, `applies`, `skill_for`, `power`, `difficulty`, `succeeds`, `Run`, `Trace`, `TraceEvent`, the constants.
- `flavor/content.ron` — the programs, the deck spec, paydata items, kits.
- `crates/citygen/src/tile.rs` — `Feature::Ice(IceGrade)`, walkability.
- `crates/citygen/src/world.rs` — `Place::Construct(BuildingId)`.
- `crates/citygen/src/construct.rs` (new) — `IceGrade`, `Guard`, `Construct`, `generate_construct`, `validate_construct`.
- `crates/strata-zero/src/render/map.rs` — `ConstructMap`, `MapRef::Construct`.
- `crates/strata-zero/src/constructs.rs` (new) — `Constructs` cache, `enter_construct`, `leave_construct`, guard standing.
- `crates/strata-zero/src/net.rs` (new) — `NetState`, `Hack`, the turn step, effects queries, cooling, lockdown queries.
- `crates/strata-zero/src/screens/explore.rs` — gate entry/exit, the RUN popup, turn stepping, port refusals, locked rooms through `NetState`, dump handling.
- `crates/strata-zero/src/interiors.rs` — `ride` refusing on lockdown.
- `crates/strata-zero/src/ui/sidebar.rs`, `ui/hud.rs`, `render/tileset.rs`, `render/glyphs.rs` — meter, ALERT, location, ICE glyph, blind cameras.
- `crates/strata-zero/src/screens/deck.rs` — cyberdeck tab loading, inventory marker.
- `crates/strata-zero/src/screens/console.rs` — `program`, `heat`, `alarm`.
- `README.md`.

---

### Task 1: Programs, decks, loading, the run maths and the trace (`game`)

**Files:** model.rs, content.rs, character.rs, action.rs, run.rs (new), lib.rs, flavor/content.ron; app sites that match on `ItemKind` exhaustively (deck.rs `kind_icon`, glyphs) get the new arm.

**Interfaces (Produces):**
```rust
pub enum ItemKind { ..., Program }           // label "program"
pub enum Op { Crack, Spoof, Blind, Copy, Backdoor }
impl Op { pub fn label(self) -> &'static str }   // "crack" ...
pub struct Program { pub op: Op, pub rating: u8, pub size: u8, pub noise: u8, pub work: u8 }
pub struct DeckSpec { pub slots: u8, pub rating: u8 }
pub struct Item { ..., #[serde(default)] pub program: Option<Program>, #[serde(default)] pub deck: Option<DeckSpec> }
impl Character {
    pub loaded: Vec<ItemId>;
    pub fn deck(&self, c: &Content) -> Option<(&Item, DeckSpec)>;
    pub fn loaded_size(&self, c: &Content) -> u8;
    pub fn can_load(&self, c: &Content, id: &ItemId) -> Result<(), String>;
    pub fn load(&mut self, c: &Content, id: &ItemId) -> Result<String, String>;    // "Crack loaded · 2/2 slots"
    pub fn unload(&mut self, c: &Content, id: &ItemId) -> Result<String, String>;  // "Crack unloaded"
    pub fn loaded_programs<'a>(&'a self, c: &'a Content) -> impl Iterator<Item = &'a Item>;
}
pub enum Action { ..., Load(ItemId), Unload(ItemId) }
// game::run
pub const ALARM_HEAT: u8 = 60; pub const DUMP_HEAT: u8 = 100;
pub const ALERT_MINUTES: u64 = 60; pub const HACK_MINUTES: u64 = 60; pub const COOLING_MINUTES: u64 = 10;
pub enum IceGrade { White, Grey }            // lives in citygen (Task 2); game re-uses citygen::IceGrade — define it in citygen first in THIS task (tile.rs, no feature yet) so run.rs compiles
pub enum Target { Node(citygen::NodeKind), Guard(citygen::IceGrade) }
pub enum HackTarget { Node(citygen::NodeId), Guard { building: citygen::BuildingId, index: usize } }
pub fn applies(op: Op, target: Target) -> bool;
pub fn skill_for(op: Op, target: Target) -> SkillId;
pub fn power(ch: &Character, c: &Content, p: &Program, target: Target) -> i32;
pub fn difficulty(security: u8) -> i32;
pub fn succeeds(roll: u8, power: i32, difficulty: i32) -> bool;
pub struct Run { pub program: ItemId, pub target: HackTarget, pub done: u8, pub work: u8 }
pub enum TraceEvent { None, Alarm, Dump }
pub struct Trace { pub heat: u8, pub alert_until: Option<u64> }
impl Trace {
    pub fn add(&mut self, noise: u8, now: u64) -> TraceEvent;   // now in game minutes
    pub fn alert(&self, now: u64) -> bool;
    pub fn cool(&mut self, now: u64, backdoor: bool);           // one game minute
}
```
`now` is a game minute (`clock_tick / TICKS_PER_MINUTE`).

- [ ] **Step 1: Failing tests** (`content.rs`, `character.rs`, `action.rs`, `run.rs`):
  - `programs_and_decks_validate`: content loads; `crack`, `copy`, `spoof`, `blind`, `backdoor`, `crack_2`, `copy_2` are `Program` items with specs; `pawnshop_deck.deck == Some(DeckSpec{slots:2, rating:1})`; a program with a slot fails validation (message contains the id and "slot"); a program without `program` fails; a deck without `deck` fails; `size: 0` fails.
  - `the_kit_loads_into_the_deck`: VEX has `loaded == [crack, copy]`, `loaded_size == 2`; KESSLER (no deck) has an empty loaded list.
  - `loading_follows_the_rules_in_order`: VEX `can_load(spoof)` → "not in your pack"; give spoof → "no room: 2 of 2 slots"; `unload(copy)` → "Copy unloaded"; `load(spoof)` → "Spoof loaded · 2/2 slots"; `load(spoof)` again → "already loaded"; `load(hardhat)` → "that is not a program"; KESSLER `can_load(crack)` → "no deck jacked"; dropping `crack` with `Action::Drop` unloads it.
  - `the_run_maths`: `applies(Crack, Node(Camera))`, `applies(Crack, Guard(White))`, `!applies(Blind, Node(DoorLock))`, `applies(Copy, Node(Server))`, `applies(Backdoor, Node(Server))`, `!applies(Backdoor, Node(DataVault))`; `skill_for(Crack, Guard(_)) == ice_breaker`, `skill_for(Copy, _) == vault_run`, `skill_for(Spoof, _) == hacking`; VEX power with crack vs a lock = 2·6 + 3 + 2 = 17; with `ice_pick` rank 1 forced, 19; KESSLER (Net 3, no skill) with crack = 8; `difficulty(110) == 13`, `difficulty(200) == 25`; `succeeds(4, 17, 13)` true, `succeeds(1, 8, 13)` false, `succeeds(6, 8, 13)` true.
  - `the_trace_alarms_dumps_and_cools`: fresh trace, `add(30, 0)` → None, heat 30; `add(30, 0)` → Alarm, alert until 60; `add(30, 1)` → None (already alert); `add(10, 1)` → Dump at 100; `cool(2, false)` heat 99; alert at 59 true, at 60 false and after `cool(60, false)` heat is 0; a backdoor cools by two.
  - `load_and_unload_actions`: `apply(Action::Unload(copy))` message "Copy unloaded"; `apply(Action::Load(copy))` message "Copy loaded · 2/2 slots".
- [ ] **Step 2: Run** `cargo test -p game` — compile errors.
- [ ] **Step 3: Implement** model, content items and validation (`ContentError::ProgramInvalid { id, why }`, `DeckInvalid { id, why }`), character, actions, `run.rs`, `citygen::IceGrade` in `tile.rs` (`Feature::Ice` itself comes in Task 2, the enum first), kits in content.ron (netrunner: `("crack", 1), ("copy", 1)`; VEX kit the same; fixer: `("spoof", 1)`), `ItemKind::Program` arms in the app.
- [ ] **Step 4: Run** `cargo test --workspace` — green.
- [ ] **Step 5: Commit** `feat(game): programs in deck slots, the run maths and the trace`

---

### Task 2: Constructs (`citygen`)

**Files:** tile.rs (`Feature::Ice(IceGrade)`, `ALL_FEATURES` 18, `is_walkable` false with a standing guard feature), world.rs (`Place::Construct(BuildingId)`, `stratum()` None, `is_street()` false), construct.rs (new), lib.rs; app sites matching `Place` and `Feature` exhaustively get arms (hud::location: `"{name} · construct"`; MapRef::for_place returns None for Construct until Task 3; tileset: `▣` magenta for Grey, white for White; sidebar inspector "ICE · grey").

**Interfaces (Produces):**
```rust
pub enum IceGrade { White, Grey }   // Copy, Eq, Hash, Serialize
pub struct Guard { pub pos: Pos, pub grade: IceGrade }
pub struct Construct { pub building: BuildingId, pub security: u8, pub size: Size, pub tiles: Grid<Tile>, pub light: Grid<u8>, pub entry: Pos, pub rooms: Vec<Rect>, pub nodes: Vec<(NodeId, Pos)>, pub guards: Vec<Guard> }
impl Construct {
    pub fn node_at(&self, p: Pos) -> Option<NodeId>;
    pub fn guard_at(&self, p: Pos) -> Option<usize>;
    pub fn vault_room(&self) -> Option<Rect>;
    /// Lifts or restores a guard: its tile's feature and walkability.
    pub fn set_guard(&mut self, index: usize, standing: bool);
}
pub fn generate_construct(city: &City, b: BuildingId) -> Option<Construct>;
pub fn validate_construct(c: &Construct) -> Result<(), String>;
```
- [ ] **Step 1: Failing tests** (`construct.rs`): for the 250-seed sweep's smaller fixture (the test city used in `generate/cyberspace.rs` tests), every building with a subnet generates a construct that validates; the construct holds every non-ICE node of the building; a CorpTower with security ≥ 200 has a Grey gate guard and a Grey vault guard, a building between 160 and 199 a White gate guard and no vault guard, below 160 none; the vault's room is the last room and the server's the one before; the same seed gives an equal construct; `set_guard(i, false)` makes the tile walkable and removes the feature, `set_guard(i, true)` restores; `Feature::Ice(Grey)` makes a tile unwalkable; `Place::Construct(b).stratum().is_none()`; a city save round-trips unchanged (existing test still passes).
- [ ] **Step 2: Run** — failures.
- [ ] **Step 3: Implement** generation: size, the spine (two-wide `NetLane`, from the entry's short side), rooms every 5–7 tiles alternating sides (`NetWall` walls, `NetFloor` inside, one `NetGate` doorway on the spine), the far room closing the spine; node order door locks, lift controls, cameras, terminals, ad boards, server, vault; guards by security; light with `lighting::compute` (base 90, lanes and gates 255 as the Net stage); validation; the app arms.
- [ ] **Step 4: Run** the workspace — green.
- [ ] **Step 5: Commit** `feat(citygen): constructs have an inside`

---

### Task 3: Into the construct (app map, cache, gate, location)

**Files:** render/map.rs (`ConstructMap { city, construct }`, `MapRef::Construct`, `for_place` takes `constructs: &Constructs`), constructs.rs (new), walker (sight radius for `Place::Construct` = `RADIUS_INSIDE`), explore.rs (`doorway` on the Net: stepping onto a `NetGate` of a building from the open grid enters; from `entry` outward leaves; jack out works inside), hud::location, app.rs (`AppState.constructs`).

**Interfaces (Produces):**
```rust
pub struct Constructs { ... }
impl Constructs {
    pub fn ensure(&mut self, city: &City, b: BuildingId) -> Option<&Construct>;
    pub fn get(&self, b: BuildingId) -> Option<&Construct>;
    pub fn get_mut(&mut self, b: BuildingId) -> Option<&mut Construct>;
}
pub fn enter_construct(st: &mut AppState, b: BuildingId) -> Result<String, String>;   // "into Hextek Holdings"; Err "gate closed · Hextek Holdings on alert" comes in Task 4
pub fn leave_construct(st: &mut AppState) -> Result<String, String>;                // "out onto the grid"
```
- [ ] **Step 1: Failing tests** (explore.rs): jack in at a port, teleport (console `tp`) next to a construct's gate on the Net, step onto the gate → `player.place == Place::Construct(b)`, `pos == entry`, the log `into <name>`, the location line `<name> · construct`; step outward → back on the Net at the gate tile, log `out onto the grid`; inside, `enter` jacks out to the body; the construct's tiles draw (the ICE glyph `▣` appears when the building has a guard); a construct map is generated once (`constructs.generated() == 1` after re-entry).
- [ ] **Step 2: Run** — failures.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the workspace — green.
- [ ] **Step 5: Commit** `feat(app): through the gate into a construct`

---

### Task 4: The run: programs against nodes, the trace, the alarm, the dump, the lockdown

**Files:** net.rs (new), explore.rs (RUN popup, turn stepping on `.`, moves abort, guard adjacency dump, port refusals, locked rooms via `NetState`), interiors.rs (`ride` refuses on lockdown unless spoofed), popup.rs (`Pick::Run(ItemId, HackTarget)`, `Pick::Continue`), sidebar.rs (TRACE meter and ALERT in HERE), hud.rs (`· ALERT` in the location line), tileset.rs (blind camera dim), sidebar inspector (hacked states), console.rs (`program`, `heat`, `alarm`), app.rs (`AppState.net`).

**Interfaces (Produces):**
```rust
pub struct Hack { pub building: BuildingId, pub target: HackTarget, pub op: Op, pub until: Option<u64> }   // minutes; None forever
pub struct NetState { pub traces: BTreeMap<BuildingId, Trace>, pub hacks: Vec<Hack>, pub backdoors: BTreeSet<BuildingId>, pub cooling_until: Option<u64>, pub run: Option<Run>, last_minute: u64 }
impl NetState {
    pub fn trace(&mut self, b: BuildingId) -> &mut Trace;
    pub fn alert(&self, b: BuildingId, now: u64) -> bool;
    pub fn unlocked(&self, b: BuildingId, now: u64) -> bool;          // a Crack/Spoof on a DoorLock node of b that has not expired, and b not alert
    pub fn lifts_answer(&self, b: BuildingId, now: u64) -> bool;      // not alert, or a Spoof on LiftCtrl landed after the alarm
    pub fn blinded(&self, node: NodeId, now: u64) -> bool;
    pub fn copied(&self, node: NodeId, now: u64) -> bool;
    pub fn cooling(&self, now: u64) -> Option<u64>;                   // minutes left
    /// Called with the game minute; cools every trace the player is not inside, expires hacks, ends alerts.
    pub fn minute(&mut self, now: u64, inside: Option<BuildingId>, constructs: &mut Constructs);
}
/// One turn of the running program. Returns the log line; applies effects, heat, alarm, dump.
pub fn step_run(st: &mut AppState) -> Option<String>;
pub fn start_run(st: &mut AppState, program: ItemId, target: HackTarget) -> Result<String, String>;
pub fn dump(st: &mut AppState, why: &str);
```
Roll: `(hash64(st.clock_tick(), target_seed) % 10 + 1) as u8` where `target_seed` is the node id or `1000 + guard index`.

- [ ] **Step 1: Failing tests** (explore.rs, net.rs, interiors.rs, console.rs): the RUN popup on a lock lists `crack · 17 vs N · 3 turns · noise 4` and not blind; picking it, three `.` turns with forced-success rolls (dev `heat` and a test hook that sets `st.net.force_roll = Some(10)` under `cfg(test)`) land `lock cracked`; the building's private room now admits the player on its floor (`unlocked`); `heat` rose by 12; a move aborts (`run aborted`); console `alarm` → `ALARM · <name> locks down`, the room refuses again, `ride` → `lifts locked down`, the location line ends `· ALERT`, the gate refuses `gate closed`; `heat 100` → `DUMPED`, player back in the body, `cooling(now)` 10, jack in → `deck cooling · 10 min`; a port adjacent to an alert building → `port locked down`; copy on the vault with forced success gives one `vault_data` and a second copy is dim `copied already`; backdoor on the server: the gate guard is lifted and heat cools by two per minute; ending a turn beside a standing guard while alert dumps; Review Focus 2, 3, 5 as written; `minute()` after 60 minutes resets heat and restores guards; the sidebar HERE shows `TRACE` with the heat and `ALERT · 48 min`.
- [ ] **Step 2: Run** — failures.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the workspace — green.
- [ ] **Step 5: Commit** `feat(app): the run — programs, the trace, the alarm, the dump`

---

### Task 5: The cyberdeck tab loads programs; the inventory marks them

**Files:** deck.rs.

- [ ] **Step 1: Failing tests**: the CYBERDECK tab for VEX reads `Pawnshop deck · 2/2 slots · rating 1`, `── loaded ──`, `◆ Crack · size 1`, `◆ Copy · size 1`, `── programs ──` with netscan and arc trace; `space` on Copy → `Copy unloaded`, the row moves under `── in the pack ──` as `◇ Copy · size 1`; `space` again loads; give spoof → it lists in the pack; with the deck full `space` on spoof → `no room: 2 of 2 slots` as a warning; the detail of Crack reads `crack · any node or ICE`, `rating 1 · size 1 · noise 4 · work 3`, `your power 17 vs 13 mid-city`; the INVENTORY tab groups `── program ──` and marks `loaded`; Review Focus 1.
- [ ] **Step 2: Run** — failures.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the workspace — green.
- [ ] **Step 5: Commit** `feat(app): the cyberdeck loads and unloads programs`

---

### Task 6: Docs, smoke, screenshots

- [ ] README: the Net paragraph (constructs, gates, the RUN popup, the trace, the alarm, the dump), the cyberdeck tab, the console commands.
- [ ] Smoke `drive14.py`: VEX, jack in, `tp` to a gate, enter, RUN crack on the gate guard or a lock, `.`×3, see the log, console `alarm`, see ALERT, `heat 100`, DUMPED, quit.
- [ ] Screenshots: a construct inside with the trace meter; the RUN popup.
- [ ] Commit `docs: constructs and programs`.

## Self-review

Spec coverage: content (1), rules (1), constructs (2), moving (3), the run and effects and lockdown (4), the cyberdeck tab (5), console (4), docs (6). Review Focus lines have tests in 1+5, 4, 4, 2+4, 4. Names match: `Constructs::ensure/get/get_mut`, `NetState::{unlocked, lifts_answer, blinded, copied, cooling, minute}`, `start_run`, `step_run`, `dump`, `HackTarget`, `Trace::{add, alert, cool}`.
