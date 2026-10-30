# Drone Chrome Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Drone cradle folds out a bay in the ledger; a drone docked there comes out with you as an entity that drifts, follows, crosses places with you and takes turns on foot.

**Architecture:** The game crate gains a socket kind (`Bay`) that needs no doc and an item kind (`Bot`) whose content describes a small vehicle; docked bots count as chrome for perks and grants but not for LOAD or EMP. The app gains `bots.rs`, an entity list synced from what is docked, moved on the world clock, seated in engagements as `Actor::Bot`, and drawn by a `BotOverlay` with the A3 rotor and wake.

**Tech Stack:** Rust 2024, ratatui, crossterm, RON content (`flavor/*.ron`), Python 3 for the card art export.

**Spec:** `docs/superpowers/specs/2026-10-30-drone-chrome-design.md`

## Global Constraints

- Build and test into `--target-dir target/smoke`, never the default `target/`.
- Both front ends: `cargo clippy --workspace --all-targets --target-dir target/smoke -- -D warnings`, the same with `--features strata-zero/window`, `cargo test --workspace --target-dir target/smoke` and the same with `--features strata-zero/window`; `cargo fmt --all --check`.
- Colours are exact `Color::Rgb`. Map glyphs: `┼` `╳` (box-drawing) and the wake's `∙` `·`.
- Wording comes from kinds: `SocketKind::Bay` labels (`bay`, `drones`, `bay`), `BotBody::word()` (`drone`, `rover`). No UI string names a specific drone.
- Bots: `LEASH = 2`; drift every 15 to 40 ticks; catch-up a step every 3 ticks; glyph pace 6 ticks (drifting, standing) and 3 (catching up, moving on turns); wake 9 ticks, `∙` for the first 3.
- Shipped drones: `scout_drone` (grants `scout`, 0.6 kg, ¤2400, light (120, 255, 120)), `mule_drone` (`Carry(8.0)`, 1.4 kg, ¤1800, (255, 200, 80)), `lamp_drone` (`Sight(3)`, 0.5 kg, ¤1500, (255, 240, 200)); all `body: Air, docks: Bay, frames: ['┼', '╳'], wake: true, reflex: 6, moves: 5`.
- Commit after each task; messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

- A save from before this change (a Drone cradle copy with `fitted: []`, Scout granted by the cradle) loads; the bay row shows empty and docking resizes the copy. Pinned in Task 2.
- No free tile next to you (a one-tile corridor, the map's edge, a crowd of foes): the bot waits undrawn and comes out when a tile frees; never on your tile, never in a wall. Pinned in Task 5.
- Stowing or swapping a drone mid-fight, or its cradle leaving: a seat for a bot that is gone passes; no index into `bots` goes stale. Pinned in Task 6.
- The clock standing (menus, overview, the deck open): nothing moves and the wake doesn't age. Pinned in Task 5.
- The ledger's bay keys outside a chair when the sheet is otherwise locked (jacked in): still refused with `JACKED_IN`. Pinned in Task 4.

---

### Task 1: The bay, a socket that needs no doc

**Files:**
- Modify: `crates/game/src/model.rs` (`SocketKind`)
- Modify: `crates/game/src/surgery.rs` (`surgery_fee`)
- Test: same files' `tests` modules

**Interfaces:**
- Produces: `SocketKind::Bay`; `SocketKind::needs_a_doc(self) -> bool` (true for `Hand`, `Forearm` only); `label` `"bay"`, `takes` `"drones"`, `fits` `"bay"`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_bay_reads_from_its_kind() {
    assert_eq!(SocketKind::Bay.label(), "bay");
    assert_eq!(SocketKind::Bay.takes(), "drones");
    assert_eq!(SocketKind::Bay.fits(), "bay");
    assert!(!SocketKind::Bay.needs_a_doc());
    assert!(SocketKind::Hand.needs_a_doc() && SocketKind::Forearm.needs_a_doc());
    assert!(!SocketKind::Deck.needs_a_doc());
}
```

In `surgery.rs`: `docking_a_bay_is_no_surgery` — a cradle installed at `Site::Back` with a bot copy in the pack: `surgery_fee` of `Fit { host: Host::Site(Site::Back), socket: 0, piece }` and of `Unfit { host: Host::Site(Site::Back), socket: 0 }` is `None`; the existing hand-socket fee test still gives `Some`. (Needs Task 2's content; write it in Task 2 if the drones aren't in yet, and keep this step's model test here.)

- [ ] **Step 2: Run to see them fail** — `cargo test -p game --target-dir target/smoke a_bay_reads` → no variant `Bay`.
- [ ] **Step 3: Add `Bay` and `needs_a_doc`; in `surgery_fee`, `Fit`/`Unfit` on a `Host::Site` socket whose kind (from the host item's `sockets`) doesn't need a doc return `None`.** Fix every exhaustive `match` on `SocketKind` the compiler names.
- [ ] **Step 4: Run** `cargo test -p game --target-dir target/smoke` → pass.
- [ ] **Step 5: Commit** `game: a bay socket, which needs no doc`.

### Task 2: Bots in the model and the content

**Files:**
- Modify: `crates/game/src/model.rs` (`ItemKind::Bot`, `Item.bot`, `Bot`, `BotBody`, `Item::fits_socket`, `per_copy`)
- Modify: `crates/game/src/action.rs` (`Fit`)
- Modify: `crates/game/src/character.rs` (`fits_at`, `pieces`, `sources`, `Contrib::Bot`, `debug_runner`)
- Modify: `crates/game/src/content.rs` (`chrome_ok`, a new `bot_ok`, art required for bots)
- Modify: `crates/strata-zero/src/credentials.rs` (the `Item` literal gains `bot: None`)
- Modify: `flavor/content.ron` (cradle `sockets: [Bay]`, no grants, new line; three drones)
- Modify: `docs/superpowers/specs/2026-10-29-stats-chrome-art/implantart.py` (drawings: `drone_cradle` redrawn as the mount alone, `scout_drone`, `mule_drone`, `lamp_drone`), then run its `export.py` to regenerate `flavor/implant_art.ron`
- Test: the files' `tests` modules

**Interfaces:**
- Consumes: Task 1's `SocketKind::Bay`.
- Produces:
  - `pub struct Bot { pub body: BotBody, pub docks: SocketKind, pub frames: Vec<char>, pub light: (u8, u8, u8), pub wake: bool, pub reflex: u8, pub moves: u8 }` (serde, as the spec's block).
  - `pub enum BotBody { Air, Ground }`, `BotBody::word(self) -> &'static str`.
  - `Item.bot: Option<Bot>` (`#[serde(default)]`).
  - `Item::fits_socket(&self) -> Option<SocketKind>`: hardware's `fits`, socket chrome's kind, a bot's `docks`.
  - `Contrib::Bot(ItemId)`: a docked bot's part of a number.
  - `Character::docked(&self, c: &Content) -> Vec<(Site, usize, ItemId)>`: every bot fitted in installed chrome, sites and sockets in order.

- [ ] **Step 1: Write the failing tests** (`character.rs` unless named)

```rust
#[test]
fn a_docked_drone_grants_and_perks_like_chrome() {
    // cradle at Back, Scout drone fitted at socket 0
    assert!(ch.abilities(&c).iter().any(|a| a.0 == "scout"));
    // Mule drone instead: carry goes up by 8
    assert_eq!(after.carry_max(&c), before.carry_max(&c) + 8.0);
}

#[test]
fn a_docked_drone_costs_no_load_and_no_emp() {
    assert_eq!(with_drone.pieces(), cradle_only.pieces());
    assert_eq!(with_drone.chrome_load(&c), cradle_only.chrome_load(&c));
    assert_eq!(with_drone.resist(&c, Harm::Emp), cradle_only.resist(&c, Harm::Emp));
}

#[test]
fn a_docked_drone_is_named_as_a_drone_in_the_sources() {
    let s = ch.sources(&c, Num::Carry);
    assert!(s.contains(&(Contrib::Bot(ItemId::new("mule_drone")), 8)));
}

#[test]
fn docked_lists_bots_in_their_bays() {
    assert_eq!(ch.docked(&c), vec![(Site::Back, 0, ItemId::new("scout_drone"))]);
}

#[test]
fn a_cradle_from_an_old_save_takes_a_drone() {
    // cradle copy with fitted: vec![] (made before it had a bay)
    apply(Fit { host: Host::Site(Site::Back), socket: 0, piece: drone }).unwrap();
    assert_eq!(ch.chrome_copy(Site::Back).unwrap().fitted.len(), 1);
}

#[test]
fn bays_take_bots_and_nothing_else() {
    // a bot into a hand socket: Err "`Scout drone` does not fit a hand socket"
    // arc hand into the bay: Err "`Arc hand` does not fit a bay"
}

#[test]
fn pulling_the_cradle_takes_its_drone_along() {
    // Pull(Site::Back): the cradle's pack copy has the drone in fitted[0]
}

#[test]
fn the_debug_runner_docks_the_scout_drone() {
    // with the cradle installed: docked == [(Back, 0, scout_drone)];
    // the pack holds mule_drone and lamp_drone
}
```

In `content.rs`: `the_drones_load` (three bots, `body` Air, `docks` Bay, the spec's numbers; the cradle's `sockets == [Bay]`, `grants` empty) and `validation_refuses_bad_bots`, one `Err(ContentError::BotInvalid { .. })` each for: a bot with an `implant`; with `sockets`; with no frames; with `moves: 0`; a bot whose `docks` isn't `Bay`; a bay on something that is not chrome; a bot with no art.

- [ ] **Step 2: Run to see them fail** — `cargo test -p game --target-dir target/smoke` → compile errors on `Bot`, `docked`, `Contrib::Bot`.
- [ ] **Step 3: Model:** the types above; `per_copy` true for a bot; `Fit` takes `piece_item.fits_socket()` (load 0 for a bot) in place of its own `match`; `fits_at` and every other "what fits a socket" check use `fits_socket`.
- [ ] **Step 4: Counting:** `installed()` stays as it is (perks and grants reach everything through it); `pieces()` counts entries whose item has no `bot`; `sources` pushes `Contrib::Bot(id)` for a docked bot where it pushes `Contrib::Chrome(id)` for chrome.
- [ ] **Step 5: Validation:** `chrome_ok` accepts `Bay` among a piece's sockets (`Hand | Forearm | Bay`); `bot_ok(i)` holds the rules above; `ContentError::BotInvalid { id, why }`; art is required for items with an `implant` or a `bot`.
- [ ] **Step 6: Content and art:** the cradle and three drones as the spec has them (lines verbatim); draw the four pictures in `implantart.py` (the Scout drone and the mount in `docs/superpowers/specs/2026-10-30-drone-chrome-mock/gen.py` are the starting points), run `python3 export.py` there, check `board.py` shows them.
- [ ] **Step 7: Debug runner:** after its chrome is in, give one of every `ItemKind::Bot` and, if a piece with a `Bay` socket is installed, fit the first bot whose `docks` matches.
- [ ] **Step 8: Run** `cargo test --workspace --target-dir target/smoke` → pass (fix the strata-zero tests and shots that asserted the cradle grants Scout: they now dock a Scout drone).
- [ ] **Step 9: Commit** `game: bots, three drones, the cradle's bay`.

### Task 3: The source readout names drones

**Files:**
- Modify: `crates/strata-zero/src/screens/sheet/mod.rs` (`KINDS`, `kind_of`, the readout's `match`, `granted`)
- Test: `crates/strata-zero/src/screens/sheet/mod.rs` tests

**Interfaces:**
- Consumes: `Contrib::Bot(ItemId)`, `Bot.body.word()`.

- [ ] **Step 1: Failing test** `the_readout_names_a_drone_by_its_kind`: CAR selected with the Mule drone docked; the readout's sources line contains `drone +8 Mule drone`, and SGT with the Lamp drone `drone +3 Lamp drone`.
- [ ] **Step 2: Run** → fails (`Contrib::Bot` unmatched or shown as chrome).
- [ ] **Step 3: Implement:** a source kind after `chrome` whose word is the bot body's (`drone`); an ability granted by a docked bot is named with it the same way in `granted`.
- [ ] **Step 4: Run** `cargo test -p strata-zero --target-dir target/smoke sheet` → pass.
- [ ] **Step 5: Commit** `sheet: a drone's part of a number reads as the drone's`.

### Task 4: The ledger's bay

**Files:**
- Modify: `crates/strata-zero/src/screens/sheet/ledger.rs` (`act`, `pull`, `piece_card`, `socket_lines`, `fit_card`, `ledger_hints`)
- Test: `ledger.rs` tests

**Interfaces:**
- Consumes: `SocketKind::needs_a_doc`, `Item::fits_socket`, `Item.bot`, `BotBody::word`.
- Produces: `fn chair_free(st: &AppState, at: (Site, Option<usize>)) -> bool` in `ledger.rs`: the row is a socket whose kind needs no doc.

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn a_bay_docks_and_stows_anywhere() {
    // Locked (no chair, no free chrome), cursor on LedgerRow::Socket(Back, 0), Mule in the bag
    // space, space: Mule docked, Scout to the pack, last() == "`Mule drone` fitted · socket bay · `Scout drone` to the pack"
    // x: last() == "`Mule drone` out · to the pack", the bay empty, no fee asked
}

#[test]
fn a_bay_stays_locked_jacked_in() { /* jacked(): space and x on the bay → last() == Some(JACKED_IN) */ }

#[test]
fn the_cradle_itself_still_needs_the_chair() { /* Locked, cursor on Site(Back), x → last() == Some(OUTSIDE) */ }

#[test]
fn hints_on_a_bay() {
    // held: "↑↓ select · space swap · x stow in the bag · ← attributes · Q/E tabs" and "drones dock anywhere"
    // empty: contains "space dock", no "x stow"
}

#[test]
fn the_bay_row_card_is_the_drones() {
    // cursor on the bay at 100x30: the card holds "SCOUT DRONE", "docked in the Drone cradle",
    // "grants Scout", "out with you · follows on foot", and the drone's drawing above
}

#[test]
fn the_cradles_card_shows_its_bay() { /* "■ bay" then "Scout drone"; emptied: "□ bay" "empty" and "Scout drone, Mule drone, Lamp drone" faint */ }

#[test]
fn the_bay_picker_shows_what_a_drone_does() { /* picker on the bay: Mule row ends "carry +8", no "L0" */ }
```

- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke ledger` → fail.
- [ ] **Step 3: Keys:** `act` and `pull` skip the chair (`operating`) check and the fee ask when `chair_free(st, v.at)`; the `jacked_in` check stays first.
- [ ] **Step 4: Cards:** on a bot, `piece_card` draws `docked in the <host>` in place of `<place> · load · EMP`, then its effects, then `out with you · follows on foot` in FAINTER; `socket_lines` lists what fits a kind with `fits_socket` (so bots show under an empty bay); in `fit_card` a pick that is a bot shows its compact `effects` in GOOD where chrome shows `L<n>`, and its LOAD/EMP lines are left out.
- [ ] **Step 5: Hints:** on a chair-free row, `space swap` / `space dock`, `x stow in the bag` when held, and on the right `<word>s dock anywhere` from the held or fitting bot's body (`drones` for a bay with nothing to read) in place of the chair line.
- [ ] **Step 6: Run** the ledger tests → pass.
- [ ] **Step 7: Commit** `ledger: the cradle's bay, docked and swapped anywhere`.

### Task 5: Bots on the map

**Files:**
- Create: `crates/strata-zero/src/bots.rs`
- Modify: `crates/strata-zero/src/main.rs` (`mod bots;`), `crates/strata-zero/src/app.rs` (`AppState.bots`), `crates/strata-zero/src/screens/explore.rs` (call `bots::tick` each frame on the map, after the world advances)
- Test: `bots.rs` tests

**Interfaces:**
- Consumes: `Character::docked`, `Item.bot`, `MapRef::for_place`, `citygen::pathing::astar`.
- Produces:
  - `pub struct BotOut { pub item: ItemId, pub dock: (Site, usize), pub place: Place, pub pos: Pos, pub catching_up: bool, pub moved_at: u64, pub next_drift: u64, pub wake: Vec<(Pos, u64)> }`
  - `pub const LEASH: i32 = 2;`
  - `pub fn anchor(st: &AppState) -> Option<(Place, Pos)>`: your body while jacked in, else you.
  - `pub fn free(st: &AppState, place: Place, pos: Pos, me: usize) -> bool`
  - `pub fn sync(st: &mut AppState)`, `pub fn tick(st: &mut AppState)` (sync, then each bot's real-time move unless engaged on its place).
  - `pub fn bot_at(st: &AppState, place: Place, pos: Pos) -> Option<usize>`
  - `pub fn spec<'a>(st: &'a AppState, i: usize) -> Option<&'a game::Bot>`

- [ ] **Step 1: Failing tests** (on the COMBAT lot or `state_home` street, Scout drone docked)

```rust
#[test] fn a_docked_drone_comes_out_beside_you() { /* sync: one bot, chebyshev(you) == 1, free tile */ }
#[test] fn a_stowed_drone_goes() { /* Unfit the bay; sync: bots empty */ }
#[test] fn a_swapped_drone_is_the_new_one() { /* Fit mule over scout; sync: bots[0].item == mule_drone */ }
#[test] fn it_drifts_within_the_leash_and_off_your_tile() {
    // 2000 ticks of tick() with you standing: every frame chebyshev <= LEASH, pos != yours, walkable
}
#[test] fn it_never_drifts_through_a_door() {
    // you beside a door on a floor: 2000 ticks, never on a door tile, always a clear line to you
}
#[test] fn it_steps_off_a_foe() { /* a foe placed on its tile: within a few ticks it is elsewhere and free */ }
#[test] fn it_catches_up() { /* move you 6 tiles east; within 3 * 6 + 3 ticks it is within LEASH again */ }
#[test] fn it_crosses_places_with_you() { /* you onto a floor: after sync its place is yours, it is beside you */ }
#[test] fn jacked_in_it_stays_by_your_body() { /* body set, you in the Net: place == body's place, within LEASH of the body */ }
#[test] fn nothing_moves_while_the_clock_stands() { /* tick() 100 times with clock_tick unchanged: pos and wake unchanged */ }
#[test] fn with_no_free_tile_it_waits() {
    // every neighbour of yours blocked (foes or walls): bots[0] is out but not placed
    // (place None or a flag: it is never drawn or seated); free one tile → it comes out there
}
#[test] fn the_wake_holds_three_beats() { /* after a move, wake has the left tile; 10 ticks later it is gone */ }
```

- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke bots` → fail.
- [ ] **Step 3: Implement** per the spec's "Sync" and "Moving, off turns". Free tile: `MapRef::for_place(...).walkable(pos)`, not the anchor's tile, no standing foe, no other bot. Paths with `astar` over the place's `MapRef` (foes' `step_toward` works on strata only). Drift times from `hash64` of `(tick, dock)`. A bot with no free tile keeps `pos` unset: make `pos: Option<Pos>` if that is cleaner than a flag, and every reader skips it.
- [ ] **Step 4: Run** the bots tests → pass.
- [ ] **Step 5: Commit** `bots: a docked drone comes out, drifts, follows and crosses places`.

### Task 6: Bots take turns

**Files:**
- Modify: `crates/strata-zero/src/net.rs` (`Actor::Bot`, `initiative`, `pools_for`, `start_turn`, `pace`, `seat_foes`, `turn_view`, `turn_readout`)
- Modify: `crates/strata-zero/src/bots.rs` (`turn_start`, `step`)
- Modify: every `match` on `Actor` the compiler names (`combat.rs`, `foes.rs`, `explore.rs`)
- Test: `bots.rs` and `net.rs` tests

**Interfaces:**
- Consumes: Task 5's `BotOut`, `anchor`, `free`.
- Produces: `Actor::Bot(usize)` (index into `st.bots`); `bots::turn_start(st, i) -> bool` (false when the bot is gone, unplaced or on another place); `bots::step(st, i, can_move) -> net::Step`.

- [ ] **Step 1: Failing tests**

```rust
#[test] fn a_bot_takes_a_seat() { /* engage on the lot: order contains Actor::Bot(0), after a foe of equal reflex */ }
#[test] fn on_its_turn_it_comes_back_to_you() {
    // bot 5 tiles off on its turn: pace() moves it a tile each PACE_TICKS until within LEASH, then the turn ends
}
#[test] fn near_you_it_passes() { /* within LEASH: its turn ends at once */ }
#[test] fn a_gone_bot_passes_its_seat() { /* stow mid-fight: end_turn onto Actor::Bot(0) passes; no panic */ }
#[test] fn a_bot_is_never_a_target() { /* fire cursor candidates hold no bot tile; enemy_at on its tile is None */ }
#[test] fn the_turn_column_shows_the_drone() { /* turn_view chips include "Scout drone" with empty hp; readout "Scout drone moves 5" while it acts */ }
```

- [ ] **Step 2: Run** → fail.
- [ ] **Step 3: Implement** per the spec's "On turns". Bots seat on foot only; pools `(moves, 0)`; `start_turn` asks `bots::turn_start`; `pace` asks `bots::step` (`Step::Moved` while outside the leash, else `Step::Nothing`); a round's `seat_foes` also seats bots out on that place; the order skips nothing structurally, a gone bot's turn just passes. `bots::tick` doesn't move a bot whose place is on turns.
- [ ] **Step 4: Run** `cargo test -p strata-zero --target-dir target/smoke` → pass.
- [ ] **Step 5: Commit** `net: bots take a seat and a turn on foot`.

### Task 7: Drawing the drone

**Files:**
- Create: `crates/strata-zero/src/render/bots.rs` (`BotOverlay`)
- Modify: `crates/strata-zero/src/render/mod.rs` (`pub mod bots;`), `crates/strata-zero/src/screens/explore.rs` (push it after the people overlay)
- Test: `render/bots.rs` tests

**Interfaces:**
- Consumes: `BotOut`, `bots::spec`, the `Overlay` trait (`draw(&self, map, origin, area, tick, buf, style)`).
- Produces: `pub struct BotOverlay<'a> { pub bots: &'a [BotOut], pub content: &'a Content, pub vis: Option<&'a Grid<Visibility>>, pub moving: &'a [bool] }` (or reads moving from `BotOut`), and `pub fn frame(bot: &game::Bot, tick: u64, moving: bool) -> char`.

- [ ] **Step 1: Failing tests**

```rust
#[test] fn the_rotor_turns() { /* frame(scout, 0, false) == '┼', frame(scout, 6, false) == '╳', frame(scout, 3, true) == '╳' */ }
#[test] fn it_draws_in_its_light_where_you_see() { /* Visible tile: '┼' in Rgb(120,255,120), bold; Remembered tile: untouched */ }
#[test] fn a_foe_draws_over_it() { /* a standing foe on the bot's tile: the foe's glyph, not the rotor */ }
#[test] fn the_wake_fades() { /* left 1 tick ago: '∙' near the light; 5 ago: '·' dimmer; 10 ago: nothing; a tile with a foe on it: untouched */ }
#[test] fn your_at_draws_over_it() { /* the map's draw with the bot's tile and yours adjacent: '@' stays '@' */ }
```

- [ ] **Step 2: Run** → fail.
- [ ] **Step 3: Implement** per the spec's "Drawing": the wake's colour mixes the light toward the tile's background by its age over 9 ticks, drawn only on Visible tiles holding no one.
- [ ] **Step 4: Run** `cargo test -p strata-zero --target-dir target/smoke render::bots` → pass.
- [ ] **Step 5: Commit** `render: the drone's rotor and its wake`.

### Task 8: Shots, README, both front ends

**Files:**
- Modify: `crates/strata-zero/src/screens/shots.rs` (`attributes_sheet` gains the cradle, the bay and the bay's picker at 100×30 and 120×40; a new `drone_on_the_street` shot)
- Modify: `README.md` (the chrome section)

- [ ] **Step 1:** Add the shots; run `SHOT_DIR=<scratch> cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored attributes_sheet drone_on_the_street`; look at the PNGs: the bay row, the drone's card and drawing, the picker, the rotor beside the `@` with its wake.
- [ ] **Step 2:** README: the cradle's bay, drones dock and swap anywhere, `x` stows, a docked drone comes out with you.
- [ ] **Step 3:** The five checks in Global Constraints, all green.
- [ ] **Step 4:** A live look: `STRATA_WINDOW_SHOT=<scratch>/live.png cargo run -p strata-zero --features window --target-dir target/smoke -- --window --dev` on a debug start, the drone visible beside you.
- [ ] **Step 5: Commit** `docs, shots: the drone`.
