# Your rig's own Net (#104) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Jacking in at the home rig lands you in your rig's own Net: a room built from
the rig whose doors are your backdoors, whose reader summons what you put in it (cash,
rig programs, sift, readings), where a locked credchip is cracked, a black one is fought
for against MASTIFF, and the rig's shell opens with `r`.

**Architecture:** A new `citygen::Place::Rig`. The room (`rigroom::RigRoom`) is laid out
from the rig's counts on entry, kept unsaved in `Constructs.rig`, and walked as a
hand-built `citygen::Construct` (as `lot_construct` is) through `MapRef::Construct` with a
`rig` flag. It has its own renderer (`render/netlook/rig.rs`), panel, interactions and,
for MASTIFF, two new turn actors (`Actor::Mastiff`, `Actor::Seat(u8)`).

**Tech Stack:** Rust, ratatui cells, crossterm keys; content in `flavor/*.ron`.

**Spec:** `docs/superpowers/specs/2026-11-18-rig-construct-design.md` (read it with this
plan; the mock it names, `2026-11-18-rig-construct-mock/gen.py`, is the picture of every
frame).

## Global Constraints

- Build only into `--target-dir target/smoke` (never `target/`).
- Every task ends green on: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --target-dir target/smoke -- -D warnings`; the same with `--features strata-zero/window`; `cargo test --workspace --target-dir target/smoke`; the same with `--features strata-zero/window`. Run the window ones at least on tasks that touch drawing or `Place`.
- Draw only through ratatui; read keys as crossterm `KeyEvent`s; no Alt+key features.
- Exact `Color::Rgb` colours. Any new symbol must pass `every_drawn_character_has_a_glyph`.
- A new `AppState` field is named in `runs::snapshot::{snapshot, restore}`; a new field in a saved type has `#[serde(default)]`; `runs/fixtures/` keep opening and are never regenerated.
- Commit messages: the decision, no chat; subject tagged `(#104)`; end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. The last task's commit adds `Closes #104` before the trailer.
- Counts in UI are digits (`2 locks`, `1/3`), never words.
- In-world copy is the world's voice (log lines, labels), no narrator prose.
- Colours: CORE `#00e5ff`, RELAY `#50e678`, LINE/AMBER `#ffb000`, firewall/RED `#ff2848`, MAGENTA `#ff00c8`, black-ICE HUD red `#ff2828`, TERMINAL green held-node ground `#0f3a1f`, MASTIFF ground `#2a0710`, broken plate `#8a2236`, leash tint `#16080c`, leash edge `#6a1a28`, bite `#ff4060` on `#4a0c18` (all from the mock's `gen.py`).

## Review Focus

1. A backdoor that burns, springs or runs out while you're in the room: its door goes dark at once, and stepping onto it refuses with the reason instead of landing you (Task 5 test `a_burned_backdoor_door_refuses`).
2. A Hydra relay (8 lines) and a Lotus with two Q-Stacks (6+ seats): every door, seat and the terminal still get their own tile and the room stays walkable from the landing to each (Task 3 test `the_room_fits_eight_lines_and_six_seats`).
3. A run saved mid-fight with MASTIFF: it reopens in the room with MASTIFF asleep and whole and not on turns (Task 11 test `a_restored_fight_finds_mastiff_asleep`).
4. A rack swapped for one with fewer reader slots while the reader is full: nothing is lost, the extras land in the pack with their sums (Task 1 test `fewer_reader_slots_send_the_extras_to_the_pack`).
5. No deck, or no Crack / Attack loaded: cracking, attacking and the seats refuse or sit out without spending the chip or a turn's AP (Task 9 test `crack_without_a_deck_refuses_and_keeps_the_chip`, Task 11 test `with_no_attack_loaded_the_seats_sit_out`).

---

### Task 1: The reader, the black credchip and crack counts in the model

**Files:**
- Modify: `crates/game/src/model.rs` (`FrameSpec`), `crates/game/src/character.rs` (`ItemCopy`, `Character`, `fit_frame`/`set_frame`)
- Modify: `flavor/content.ron` (racks' `frame:` gain `reader:`; new item `credchip_black`)
- Modify: `crates/strata-zero/src/containers/loot.rs` (`What::BlackChip`), `flavor/loot.ron`
- Test: in-file `#[cfg(test)]` modules of the files above (`game/src/character.rs`, `containers/loot.rs`)

**Interfaces:**
- Produces: `FrameSpec.reader: u8` (`#[serde(default)]`); `ItemCopy.cracked: u8` (`#[serde(default)]`); `Character.reader: Vec<Option<Stack>>` (`#[serde(default)]`); `Character::reader_slots(&self, c: &Content) -> usize` (the fitted rack's `reader`, stock rack when none); `Character::put_in_reader(&mut self, c: &Content, slot: usize, copy: CopyId) -> Result<(), String>` and `put_item_in_reader(&mut self, c, slot, item: &ItemId)` for copy-less stacks (one of the pack's stack, removed from the pack); `Character::pull_from_reader(&mut self, slot: usize) -> Result<Stack, String>` (into the pack, returns what moved); `loot::What::BlackChip((u32, u32))`.
- Item `credchip_black`: `name: "Black credchip"`, `kind: Junk`, `weight: 0.05`, `value: 0`, `use: Some(Cash(locked: true))`, `desc: "A bearer chip with black ICE coiled round its sum. Somebody's rig can break it."`

- [ ] **Step 1: Write the failing tests**
  - `racks_carry_their_reader_slots`: reader of `rack_zenbo` 1, `rack_lotus` 2, `rack_civitas` 3, `rack_halcyon` 4; a fresh Vex's `reader_slots` is 1.
  - `the_reader_keeps_a_chips_sum`: give a `credchip` of 400, put it in slot 0 → pack count 0, `reader[0]` holds it with `credits == 400`; pull it → back in the pack with 400.
  - `a_full_slot_refuses`: putting into an occupied slot errs and leaves both items where they were.
  - `fewer_reader_slots_send_the_extras_to_the_pack`: on a Halcyon with chips of 100/200/300/400 in slots 0-3, set the frame to `rack_civitas` → `reader.len() == 3`, the chip of 400 is in the pack with its sum; then to `rack_zenbo` → slots 1-2's chips in the pack too.
  - `a_black_chip_refuses_in_the_pack`: `Action::UseCopy` on a `credchip_black` errs with `locked · crack it at your rig` and keeps it.
  - (loot.rs) `black_chips_roll_from_their_line`: a table whose only line is `BlackChip((2000, 6000))` yields one `credchip_black` copy with credits in `2000..=6000` scaled by the band; the content check accepts the line.

- [ ] **Step 2: Run them to see them fail**
  Run: `cargo test -p game --target-dir target/smoke reader` and `cargo test -p strata-zero --target-dir target/smoke black_chips_roll`
  Expected: compile errors (no `reader`, no `BlackChip`).

- [ ] **Step 3: Implement**
  - The four racks' `FrameSpec` get `reader: 1 | 2 | 3 | 4` as above.
  - `Character.reader` grows to `reader_slots` lazily (a `None` per missing slot) when a slot is first used; a frame change truncates it, sending each `Some` past the new count to the pack (`inventory.push`), as the bays' extras go.
  - `BlackChip` rolls a `credchip_black` copy through `give_credchip`'s path with the band's scale; add `Line(weight: 1, what: BlackChip((2000, 6000)))` to the guarded `Safe` tables for `office` and `hotel` (create those `(site, guarded: true)` safe tables only if they don't exist; else add the line to them).

- [ ] **Step 4: Run the tests** — same commands; expected PASS. Then the five checks.

- [ ] **Step 5: Commit** — `git commit -m "rig: reader slots by rack, the black credchip, crack counts on a copy (#104)"`

### Task 2: What the reader takes, and the desk's READER rows

**Files:**
- Create: `crates/strata-zero/src/rigroom/mod.rs` (module root, `pub mod reader;` for now), `crates/strata-zero/src/rigroom/reader.rs`
- Modify: `crates/strata-zero/src/main.rs` or `lib.rs` (wherever modules are declared: `mod rigroom;`), `crates/strata-zero/src/screens/desk.rs`
- Test: `rigroom/reader.rs` and `screens/desk.rs` test modules

**Interfaces:**
- Consumes: Task 1's reader API.
- Produces:
  ```rust
  pub enum Holds { Chip, Locked, Black, Program(&'static str), Data, Story }
  pub fn holds(st: &AppState, item: &ItemId) -> Option<Holds>; // None: the reader won't take it
  pub fn glyph(h: Holds) -> (char, Color);   // ¤ amber, ▣ #9fe6ff, ◆ #ff2848, program CORE, data/story: builder's pick
  pub fn summon_word(st: &AppState, stack: &Stack) -> String; // "+¤340", "install keepalive", "sift · 6h", "read", "2 locks", "MASTIFF"
  ```
  `Program` is `"sniff"` for `prog_sniff`, `"keepalive"` for `prog_keepalive`; `Data` is `paydata`/`vault_data` (not `_sifted`); `Story` is any item with a `ReadAt::Rig` reading in `st.book.readings`.

- [ ] **Step 1: Write the failing tests**
  - `the_reader_takes_its_kinds`: `holds` is `Some(Chip)` for `credchip`, `Some(Locked)` for `credchip_locked`, `Some(Black)` for `credchip_black`, `Some(Program("keepalive"))` for `prog_keepalive`, `Some(Data)` for `paydata` and `vault_data`, `None` for `paydata_sifted`, `stim_patch` and `sled_tinman`; `Some(Story)` for `ledger_shard` (the book has its rig reading).
  - Desk: `the_reader_rows_follow_the_bays`: on a Civitas the desk has `1 + 4 + 3` rows; the first reader row's label reads `READER 1`.
  - Desk: `space_on_a_reader_slot_lists_what_it_takes`: with a credchip, a stim patch and a ledger shard in the pack, the list on `READER 1` has the chip and the shard, not the patch.
  - Desk: `putting_and_pulling_takes_no_time`: pick the chip → it's in `reader[0]`, the clock didn't move; `x` on that row → back in the pack, clock unmoved.

- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke reader` → FAIL (missing module/rows).

- [ ] **Step 3: Implement** the rows after the bays in `Desk::rows` (`frame + bays + reader_slots`), the list (`fits_here` returns the pack's takeable stacks for a reader row, a new `Pick::Reader { item, copy: Option<CopyId> }`), `space` puts (no `run_clock`), `x` pulls; the side card for a reader row names what's in it and `summon_word`. The status line adds nothing new.

- [ ] **Step 4: Run** the tests → PASS; the five checks.

- [ ] **Step 5: Commit** — `"desk: the reader's slots, filled and emptied at the desk (#104)"`

### Task 3: The room's layout

**Files:**
- Create: `crates/strata-zero/src/rigroom/layout.rs`
- Modify: `rigroom/mod.rs` (`pub mod layout; pub use layout::{RigRoom, Counts, lay_out};`)

**Interfaces:**
- Produces:
  ```rust
  pub struct Counts { pub lines: usize, pub seats: u8, pub slots: usize, pub firewall: u8 }
  pub struct RigRoom {
      pub construct: citygen::Construct, // hand-built like lot_construct: NetWall, NetFloor, NetGate for the doors and the trunk's end
      pub ring: Option<Rect>,            // the firewall ring's outer edge; None at firewall 0
      pub floor: Rect,                   // the floor's double wall
      pub ports: Rect, pub hall: Rect, pub seats_room: Rect,
      pub doors: Vec<Pos>,               // a door per line in PORTS' outer wall, top to bottom
      pub slots: Vec<Rect>,              // a 7x5 box per reader slot, left to right
      pub seats: Vec<Rect>,              // a box per core seat, top to bottom
      pub terminal: Pos,
      pub trunk: Pos,                    // the trunk's end, in the floor's bottom wall
      pub land: Pos,                     // where you land: in the hall, under the slots
  }
  pub fn lay_out(c: Counts, building: BuildingId) -> RigRoom;
  pub fn front(r: Rect) -> Pos; // the tile under a box's middle, where you stand at it
  pub fn at_box(here: Pos, r: Rect) -> bool; // within one step of the box
  ```
  Geometry from the mock (`gen.py look_a`): firewall ring at the outer edge, floor two cells in, PORTS 19 wide on the west, hall 27 wide, SEATS 18 wide on the east; doors 4 rows apart starting 5 rows below the floor's top (3 apart if `5 + 4 × lines` won't fit the height); seat boxes 5 rows tall from 5 rows below the top, a row apart; the terminal 3 rows under the last seat; the reader's boxes from 4 rows below the top, 9 apart; height `max(30, 5 + 4 × lines + 2, 5 + 6 × seats + 6)`. Walls, slot boxes, seat boxes and the terminal block; doors and the trunk's end are `NetGate`.

- [ ] **Step 1: Write the failing tests** (`rigroom::layout::tests`)
  - `the_civitas_room_is_the_mocks`: `lay_out(Counts{lines:5, seats:2, slots:3, firewall:1}, b)` → 5 doors in PORTS' outer wall at `floor.y + 5, 9, 13, 17, 21`, 3 slots, 2 seats, `ring.is_some()`.
  - `no_firewall_no_ring`: firewall 0 → `ring == None`.
  - `the_room_fits_eight_lines_and_six_seats`: lines 8, seats 6, slots 4 → 8 distinct doors, 6 seats, 4 slots, terminal on its own tile, all inside `floor`.
  - `every_part_is_a_walk_from_the_landing`: for each of the above counts, a flood fill over walkable tiles from `land` reaches every door, `front(slot)`, a walkable tile next to each seat box, a walkable tile next to the terminal, and `trunk`.
  - `boxes_block_doors_step`: each slot/seat tile and the terminal are not walkable; each door and the trunk are `NetGate`.

- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke rigroom::layout` → FAIL.
- [ ] **Step 3: Implement** `lay_out` and the helpers.
- [ ] **Step 4: Run** → PASS; the five checks.
- [ ] **Step 5: Commit** — `"rigroom: the room laid out from the rig's counts (#104)"`

### Task 4: `Place::Rig`, jacking in at home, the trunk, saves

**Files:**
- Modify: `crates/citygen/src/world.rs` (`Place::Rig`, `stratum()`/`construct()` → `None`), every `match` on `Place` the compiler flags (the survey list in the spec's "Left to the builder"), `crates/strata-zero/src/constructs.rs` (`#[serde(skip)] pub rig: Option<RigRoom>`), `crates/strata-zero/src/render/map.rs` (`ConstructMap.rig: bool`; `for_place(Place::Rig)`; `place()` → `Place::Rig` when `rig`), `walker/mod.rs` (`sees_all` for `Rig`), `net.rs` (`jack_in_here` at home, `wants_landing_list` false at home, `port_of`), `rigroom/mod.rs`, `runs/snapshot.rs` (`after_restore` rebuilds the room), `runs/describe.rs`, `ui/hud.rs::location`
- Create: `crates/strata-zero/src/rigroom/ways.rs`

**Interfaces:**
- Consumes: Task 3 (`lay_out`, `RigRoom`).
- Note: tests elsewhere that call `net::jack_in_here` at home to stand on the grid now land in the room; change them to jack in and take the trunk (`ways::to_the_grid`), keeping what they test.
- Produces: `rigroom::counts(st: &AppState) -> Counts` (`lines` is `max(rig lines, standing backdoors)`); `rigroom::build(st: &mut AppState)` (lays out into `st.constructs.rig`, keeping an awake MASTIFF out: always rebuilds asleep); `rigroom::room(st: &AppState) -> Option<&RigRoom>`; `rigroom::ways::enter(st: &mut AppState) -> Result<String, String>` (from the rig: body set, `Place::Rig` at `land`, `after_jack_in(st, from, None)`); `rigroom::ways::to_the_grid(st: &mut AppState) -> Result<String, String>` (onto `HomeRig.landing` on `Place::Stratum(Net)`, FOV refreshed). `rigroom::ways::step(st: &mut AppState, to: Pos) -> Option<Result<String,String>>` (called from `explore.rs`'s `door_ahead` path for `Place::Rig`: the trunk now; doors in Task 5).
- Log and labels: `jacked in · your rig`; HUD place line and footer row 1 `zenbo · your rig`; describe `in your rig's Net`.

- [ ] **Step 1: Write the failing tests**
  - `jacking_in_at_home_lands_in_the_room`: `state_home()` with a backdoor held; `net::jack_in_here` → `player.place == Place::Rig`, `pos == room.land`, `body` is the flat, `wants_landing_list(st) == false`.
  - `the_trunk_lands_on_your_block`: from the room, `ways::step` onto `trunk` → `Place::Stratum(Net)` at `HomeRig.landing`, still jacked in.
  - `jacking_out_of_the_room_is_home`: `net::jack_out` from `Place::Rig` → back on the flat at the rig.
  - `a_save_inside_reopens_inside`: save with `runs::testutil::Dir`, reopen → `Place::Rig`, `constructs.rig.is_some()`, the player on a walkable tile.
  - `the_rig_is_fully_seen`: `walker` sees every room tile without walking.
  - `the_place_line_names_your_rig`: `ui::hud::location` → `zenbo · your rig`.

- [ ] **Step 2: Run** → FAIL (no variant).
- [ ] **Step 3: Implement.** `Place::Rig` arms: in turn code treat it like a construct place for engagement (`scene_of` → a new `Scene::Rig`, or `None` where scenes only serve buildings — pick per site, documented in the commit), building-keyed lookups skip it. Street terminals keep the landing list.
- [ ] **Step 4: Run** the tests and all five checks (both front ends: this touches shared code).
- [ ] **Step 5: Commit** — `"net: jacking in at home lands in your rig's own Net (#104)"`

### Task 5: Doors out through your backdoors

**Files:**
- Modify: `crates/strata-zero/src/net.rs` (split `land_at` into its checks-and-landing part usable while already jacked in), `rigroom/ways.rs`, `rigroom/mod.rs`

**Interfaces:**
- Consumes: Task 4's `ways::step`.
- Produces: `rigroom::door_lines(st: &AppState) -> Vec<Option<BuildingId>>` (one per door: `Some(b)` for a standing backdoor, in `route`'s listing order, held ones first; `None` for a free line); `net::leave_by_backdoor(st: &mut AppState, b: BuildingId) -> Result<String, String>` (what `land_at` does minus `Player::jack_in`; the route's first relay is `b`).

- [ ] **Step 1: Write the failing tests**
  - `a_door_leaves_for_its_buildings_gate`: a Net backdoor on a plain building → stepping onto its door lands on `Place::Stratum(Net)` at `constructs::city_gate(city, b)`; the route's relays are `[b]`; the backdoor's life dropped by `COST_LAND_NET` (or `COST_LAND` for a key).
  - `a_sites_door_lands_on_its_l_hub`: a backdoor on a site → `Place::SiteNet` on L's hub.
  - `a_burned_backdoor_door_refuses`: burn the backdoor after entering; `door_lines` shows `None`… the door is dark and stepping on it leaves you in the room with a log line; a honeypot springs as `land_at` springs it.
  - `a_free_line_is_a_wall`: stepping onto a free line's door doesn't move you.

- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS; five checks.
- [ ] **Step 5: Commit** — `"rigroom: backdoors are doors out of the room (#104)"`

### Task 6: Drawing the room

**Files:**
- Create: `crates/strata-zero/src/render/netlook/rig.rs`
- Modify: `render/netlook/mod.rs` (dispatch when `ConstructMap.rig`), `screens/explore.rs` (overlay pushed for `Place::Rig`; `cam_bounds` for the room), `ui/hud.rs` footer row 1 for the room, `screens/shots.rs` (shot `rig_room`)

**Interfaces:**
- Consumes: `RigRoom`, `door_lines`, `st.backdoors.jobs.running`, `st.character.reader`, `reader::glyph`.
- Produces: `pub fn draw(buf: &mut Buffer, area: Rect, view: &RigView)` with `pub struct RigView<'a> { room: &'a RigRoom, doors: Vec<DoorView>, slots: Vec<Option<SlotView>>, seats: Vec<Option<SeatView>>, you: Pos, origin: Pos }` built by `rigroom::view(st) -> RigView` (in `rigroom/mod.rs`); `DoorView { name: String, left: String, held_by: Option<&'static str> }`; `SlotView { glyph: char, colour: Color, name: String, tail: String }`; `SeatView { job: String, on: String, state: String, door: Option<usize> }`.
- Pictures, from `gen.py look_a` round 5 frame 1: the firewall ring (double, `#ff2848` on `#1f070c`, title `FIREWALL · <tier> ▮▯▯`), the floor's double wall (`#00b7cc` on `#01181c`), the room titles (`PORTS` RELAY bold + relay name dim, `READER · N slots` AMBER bold, `SEATS` CORE bold + core name), held doors `▯` RELAY bold with lanes `═` out through `╬` in the ring, free doors `▯` `#4a525e`, the slot boxes (`#0b5560`), seat boxes (round corners, CORE when busy, `#0b5560` free), lines seat→door (`┈ ┊ ┌ ┐ └ ┘ ┼ ◂`, a `●` pulse moving one cell a beat), `held · keepalive`, the terminal node (`◇` RELAY bold on `#0f3a1f`, `TERMINAL` RELAY bold, `[r] shell` `#a0aab9`), the LINE label and trunk (`▯` `#9fe6ff`, `║`, `▸ the grid · your block`). The reading card in the room sits over the map pane.

- [ ] **Step 1: Write the failing tests**
  - `the_room_draws_its_parts`: drawing a Civitas room with 3 held of 5 lines, sift + keepalive running, a chip in slot 0 → the buffer text contains `FIREWALL · sandbag`, `PORTS`, `Kiroflux`-style names of the held buildings, `free line` twice, `READER · 3 slots`, `SEATS`, `◉ sift`, `◉ keepalive`, `held · keepalive`, `TERMINAL`, `[r] shell`, `▸ the grid · your block`.
  - `a_killed_job_leaves_its_seat_and_line`: kill keepalive → its seat reads `free seat`, its door shows its time left again, no `◂` in the PORTS wall.
  - The shot `rig_room` (ignored, `SHOT_DIR`), and the existing `every_drawn_character_has_a_glyph` covering it (add the room to the screens it walks).

- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS; five checks including the window ones; write the shot and look at it against the mock's round 5 frame 1.
- [ ] **Step 5: Commit** — `"netlook: your rig's room drawn from the rig (#104)"`

### Task 7: The rig's panel

**Files:**
- Create: `crates/strata-zero/src/rigroom/panel.rs`
- Modify: `screens/explore.rs` (the HERE card for `Place::Rig`; the sidebar's here cap as a site's)

**Interfaces:**
- Produces: `pub fn panel_lines(st: &AppState) -> Vec<Line<'static>>`; `pub fn here_lines(st: &AppState) -> Vec<Line<'static>>` (what's under you; Tasks 8-11 add to it).
- Rows, from the mock's panel(): `ZENBO · your rig`; the rack name and bay count (`Civitas cabinet · 4 bays`); `FIREWALL ▮▯▯ sandbag`; rule; `LINES 3/5 · hold 5 days` and a row per line (`net · 2d`, `site · 4d`… as the backdoor's kind and life, `key · 6h`, `net · held` CORE under keepalive, `free line` dim) then `▸ the grid  your block`; rule; `READER 2/3` and a row per slot (glyph, name, `summon_word`; `· free slot` faint); rule; `SEATS 2/2 · Tinman 2X` and a row per job (`◉ sift · Paydata shard  4h`); rule; here lines; `BODY at home` at the foot.

- [ ] **Step 1: Failing test** `the_panel_reads_the_rig`: on the Task 6 state the lines' text contains each row above with the state's values; a free line row is there twice; `BODY at home` last.
- [ ] **Steps 2-4:** run → FAIL, implement, run → PASS; five checks.
- [ ] **Step 5: Commit** — `"rigroom: the rig's panel beside its room (#104)"`

### Task 8: Summon

**Files:**
- Create: `crates/strata-zero/src/rigroom/summon.rs`
- Modify: `screens/explore.rs` (`space` in `Place::Rig` → `rigroom::interact`), `screens/popup.rs` (new picks), `missions/reading.rs` (a card placed over a given rect)

**Interfaces:**
- Consumes: `reader::holds`, `layout::at_box`, `rig::jobs::{start, JobKind}`, `missions::reading::ReadCard`.
- Produces: `pub fn interact(st: &mut AppState) -> Option<Popup>` (the nearest of: a slot's box within a step, the terminal (Task 10), nothing); `Pick::Summon(usize)`, `Pick::RunOn { prog: &'static str, b: BuildingId }`; `pub fn summon(st: &mut AppState, slot: usize) -> Result<Summoned, String>` with `pub enum Summoned { Cash(u32), Installed(&'static str, bool /* asks for a host */), Sifting(u32 /* job n */), Read(ItemId) }`; `ReadCard::open_in(st, item, at, area: Rect)` (or a field) so the room's card centres on the map pane.
- Copy: popup head is the thing's name; rows `Summon · +¤340`, `Install keepalive` / `Run keepalive`, `Sift it · 6h` (`· 10h` vault; greyed `every seat is busy`), the reading's verb, `x Leave it`. Log: `summoned ¤340 · you carry ¤660`, `INSTALLED keepalive FROM SHARD`, run's own lines (`2 · keepalive on <host> · its hold stays full while it runs`). The host picker lists standing Net backdoors only.

- [ ] **Step 1: Failing tests** (`rigroom::summon::tests`)
  - `summoning_a_chip_cashes_it`: chip of 340 in slot 0, credits 320 → `Cash(340)`, credits 660, `reader[0] == None`, a number fx pushed.
  - `summoning_a_program_installs_it_and_offers_a_host`: `prog_keepalive` in slot 1, a free seat, one Net and one key backdoor → `Installed("keepalive", true)`, the shard gone, `programs` has it; the host popup lists the Net backdoor only; picking it starts `Keepalive(b)`.
  - `an_installed_program_runs_without_spending_the_shard`: already installed → row `Run keepalive`, the shard stays.
  - `summoning_paydata_sifts_it`: `paydata` in slot 2, a free seat → a `Sift` job, `reader[2] == None`; with every seat busy the row is refused and the shard stays.
  - `summoning_the_ledger_opens_its_reading`: the HOLO card opens, `missions.read` has `ledger_shard`; closing the card puts the shard in the pack and empties its slot (`character.count(ledger_shard) == 1`).
  - `space_away_from_the_reader_does_nothing`.
- [ ] **Steps 2-4:** run → FAIL, implement, run → PASS; five checks.
- [ ] **Step 5: Commit** — `"rigroom: summon what's in the reader (#104)"`

### Task 9: Cracking a locked credchip

**Files:**
- Create: `crates/strata-zero/src/rigroom/crack.rs`
- Modify: `rigroom/summon.rs` (the locked chip's row), `rigroom/panel.rs` (here lines), `screens/popup.rs` (`Pick::CrackChip(usize)`)

**Interfaces:**
- Produces: `pub fn locks(credits: u32) -> u8` (`< 500` → 1, `< 2000` → 2, else 3); `pub const LOCK_SECURITY: [u8; 3] = [100, 140, 180]`; `pub fn crack_chance(st: &AppState, slot: usize) -> Result<u8, String>` (`game::chance(game::power(ch, c, crack, Target::Node(NodeKind::Container)), game::difficulty(LOCK_SECURITY[locks - 1]))`); `pub fn crack(st: &mut AppState, slot: usize) -> Result<String, String>` (one minute of game time; a roll via `combat::roll`; a hit raises `copy.cracked`; at `cracked == locks` the stack becomes `credchip` with the same copy).
- Copy: row `Crack a lock · 58%`; log `lock 1 of 2 cracked`, `the lock holds`, last: `the chip opens · ¤1,200`; here lines `▣ Locked credchip · ¤1,200 inside` / `2 locks · crack 58% · space`.

- [ ] **Step 1: Failing tests**
  - `locks_come_from_the_sum`: 499 → 1, 500 → 2, 1999 → 2, 2000 → 3.
  - `a_hit_cracks_a_lock_and_the_last_opens_it`: with a forced roll (the test util combat uses), a 1,200 chip takes two hits and then is a `credchip` of 1,200 in its slot; the clock moved a minute per try.
  - `a_miss_holds`: a forced low roll leaves `cracked` unchanged, says `the lock holds`.
  - `crack_without_a_deck_refuses_and_keeps_the_chip`: no deck / no Crack loaded → `Err`, the chip unchanged, no time spent.
  - `cracking_raises_no_trace`: `st.net` heat and trace unchanged after a crack.
- [ ] **Steps 2-4:** run → FAIL, implement, run → PASS; five checks.
- [ ] **Step 5: Commit** — `"rigroom: crack a locked credchip, a lock at a time (#104)"`

### Task 10: The rig's terminal in the room

**Files:**
- Modify: `screens/explore.rs` (`r` in `Place::Rig` opens the rig's shell; `space` near the terminal via `rigroom::interact`), `screens/terminal.rs` (`jack_labels`: `ZENBO` for `HostId::Rig` when the player is in `Place::Rig`; bottom edge `esc · back to your rig` there), `rigroom/panel.rs` (here lines at the terminal)

**Interfaces:**
- Produces: `rigroom::open_shell(st: &mut AppState) -> Box<dyn Screen>` = `Terminal::open(st, HostId::Rig, (Place::Rig, room.terminal))`.
- Copy: here lines `◇ TERMINAL · your rig's shell` / `r opens it from anywhere here` / `yours: no crack, no hold to run out`.

- [ ] **Step 1: Failing tests**
  - `r_opens_the_rigs_shell_anywhere_in_the_room`: from `land` (far from the terminal) `r` pushes a `Terminal` whose drawn text contains `◇ JACK · ZENBO`, `esc · back to your rig` and the rig's prompt.
  - `space_at_the_terminal_opens_it_too`.
  - `the_shell_installs_from_the_pack_as_the_desk_does`: a `prog_sniff` in the pack → the boot says `INSTALLED sniff FROM SHARD`.
  - `run_and_kill_show_in_the_room`: run `run rent` in the shell, close it, draw the room → a seat reads `◉ rent`; `kill 1` → `free seat`.
- [ ] **Steps 2-4:** run → FAIL, implement, run → PASS; five checks.
- [ ] **Step 5: Commit** — `"rigroom: the rig's shell from anywhere in its room (#104)"`

### Task 11: MASTIFF on turns

**Files:**
- Create: `crates/strata-zero/src/rigroom/mastiff.rs`
- Modify: `rigroom/layout.rs` (`RigRoom` gains `pub mastiff: Option<Mastiff>`, `None` from `lay_out`), `net.rs` (`Actor::Mastiff`, `Actor::Seat(u8)`; `initiative`, `pools_for`, `pace`/`end_turn` dispatch to `mastiff::turn` and `mastiff::seat_turn`; `hunting` true while MASTIFF is awake; `dump`/`jack_out` call `mastiff::reset`), `combat.rs` (`attack_mastiff`), `screens/explore.rs` (the Attack key on MASTIFF in `Place::Rig`), `rigroom/summon.rs` (`Wake MASTIFF` row), `rigroom/mod.rs` (`build` drops any MASTIFF)

**Interfaces:**
- Produces:
  ```rust
  pub struct Mastiff { pub slot: usize, pub pos: Pos /* top-left of 3x3 */, pub plates: [bool; 8] /* true = whole, COAT order: ▛ ▀ ▜ ▐ ▟ ▄ ▙ ▌ */,
                       pub core: u8 /* hits left, starts 2 */, pub bite: Option<Rect>, pub leash: Rect }
  pub const MASTIFF_SECURITY: u8 = 180;
  pub const BITE: u32 = 7;
  pub fn wake(st: &mut AppState, slot: usize) -> Result<(), String>;   // engages Place::Rig: order You, Seat(i) per free seat, Mastiff
  pub fn turn(st: &mut AppState);                                      // two moves toward you in the leash, then mark or bite
  pub fn seat_turn(st: &mut AppState, seat: u8);                       // the seat's attack at chance × speed / 100, capped 95
  pub fn hit(st: &mut AppState, from: Pos) -> String;                  // breaks the whole plate nearest `from`, else a core hit
  pub fn end_of_your_turn(st: &mut AppState);                          // off the leash: one plate back (max 8)
  pub fn reset(st: &mut AppState);                                     // asleep and whole: room.mastiff = None, disengage
  pub fn attack_chance(st: &AppState) -> Result<u8, String>;           // chance(power(attack, Guard(Black)), difficulty(MASTIFF_SECURITY))
  ```
  The leash is the slot's box grown by 6 each way, clipped to the hall. A bite is the 3×3 beside MASTIFF on the side facing you; biting you takes `BITE` from your HP as black ICE does (Shield soaks it, the failsafe dumps you first, with the failsafe off it can kill: death line `MASTIFF · in your own rig`). Your Attack and the seats reach MASTIFF only while you stand on the leash; off it they refuse (`off the leash · out of reach`) and sit out. The core's second hit: grit (kept on the room until it's rebuilt), `disengage`, the stack becomes `credchip` with its sum. No heat, no trace, no tracer anywhere in this.

- [ ] **Step 1: Failing tests** (`rigroom::mastiff::tests`)
  - `waking_puts_the_room_on_turns`: a black chip of 4,800 in slot 2, a free seat → `engaged` with order `[You, Seat(1), Mastiff]` (the free seat's index), MASTIFF on the tile in front of the slot.
  - `it_marks_then_bites`: its first turn sets `bite`, the second hits a player standing in it (HP drops by the black ICE damage) and clears it; a player off the marks is untouched.
  - `shield_soaks_the_bite`.
  - `plates_break_from_your_side_then_the_core`: hits from the west break west plates first; after eight, two core hits win: the chip is a `credchip` of 4,800, not engaged.
  - `off_the_leash_a_plate_grows_back`: two plates broken, end your turn off the leash → seven whole.
  - `off_the_leash_nobody_hits_it`: standing off the leash, your Attack refuses without spending AP and the seat's turn does nothing.
  - `a_free_seat_attacks_and_a_busy_one_sits_out`: one seat busy with sift, one free → order has one `Seat`; the seat's forced hit breaks a plate.
  - `with_no_attack_loaded_the_seats_sit_out`: no `Seat` in the order; your Attack key refuses without spending AP.
  - `losing_resets_it`: dumped mid-fight → next wake has 8 plates and core 2.
  - `with_the_failsafe_off_it_can_kill`: failsafe off, HP at 5, a bite → `st.net.death` set.
  - `a_restored_fight_finds_mastiff_asleep`: save mid-fight, reopen → `Place::Rig`, not engaged (`after_restore` ends it), `room.mastiff == None`, the chip still black.
  - `no_trace_inside_your_firewall`: after a full fight, heat and trace unchanged, no tracer.
- [ ] **Steps 2-4:** run → FAIL, implement, run → PASS; five checks.
- [ ] **Step 5: Commit** — `"rigroom: MASTIFF, its coat, its leash and its bite, and a seat beside you (#104)"`

### Task 12: Drawing the fight

**Files:**
- Modify: `render/netlook/rig.rs` (asleep coil, leash tint and edge, awake 3×3 with broken plates, bite marks, the seat's beam on its turn, grit), `ui/hud.rs` footer (turn order labels `seat 2`, `MASTIFF`; the ICE list entry `▣ MASTIFF · black ICE · coat 5/8`), `rigroom/panel.rs` (fight here lines), `screens/shots.rs` (shots `rig_locked_chip`, `rig_mastiff`, `rig_mastiff_after`, `rig_summon`, `rig_shell`)

**Interfaces:**
- Consumes: Task 11's `Mastiff`.
- Copy (from the mock's round 3): prompt `MASTIFF bites the × tiles next turn` with keys `move off them · 3 shield · 2 attack`; here lines `◆ MASTIFF · black ICE`, `COAT ▮▮▮▮▮▯▯▯ 5/8`, `CORE sealed under the coat` / `CORE 2 hits`, `BITE next turn · the × tiles`, `LEASH the red ground`; the asleep slot's tail `MASTIFF` red, awake `awake`.

- [ ] **Step 1: Failing tests**
  - `the_fight_draws_its_board`: after three west hits and a mark, the drawn text has `░` three times in MASTIFF's cells, `×` nine times, the footer has `ENGAGED`, `seat 2`, `MASTIFF · black ICE · coat 5/8`.
  - `asleep_it_coils_round_the_chip`: `▛▀▜` above the slot glyph row in red.
  - The shots above through `every_drawn_character_has_a_glyph`.
- [ ] **Steps 2-4:** run → FAIL, implement, run → PASS; write the shots and compare with the mock's frames; five checks.
- [ ] **Step 5: Commit** — `"netlook: MASTIFF's fight drawn as the mock has it (#104)"`

### Task 13: The manual

**Files:**
- Modify: `manual/08-the-net.md` (Jacking in; Backdoors; a new section `## Your rig's Net` before `## A building's own Net`), `manual/09-the-rig-and-helix.md` (the desk's READER rows; `r` from the room; reader slots per rack in the Racks table), the chapter that says a locked credchip can't be cracked yet (grep `crack it at your rig`).

- [ ] **Step 1:** Write the sections from the spec's Decided list in the manual's voice (second person, tables for keys and marks, digits for counts). Marks table: `▯` a backdoor's door (dim: a free line), `¤ ▣ ◆` and the shard glyphs, `◉` a job in a seat, `◇` your rig's terminal, `×` a tile MASTIFF bites next turn.
- [ ] **Step 2:** Run `cargo test -p strata-zero --target-dir target/smoke manual` (the booklet's Markdown checks and the Controls chapter's key check) → PASS; then the five checks.
- [ ] **Step 3: Commit** — `"manual: your rig's own Net (#104)"` with `Closes #104` before the trailer.
