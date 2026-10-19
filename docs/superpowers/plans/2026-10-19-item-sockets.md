# Item sockets and the modding mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Sockets on items (the rebuilt deck's two), per-copy items, three pieces of deck hardware with real effects on runs, and a generic modding mode in the blueprint window to fit them.

**Architecture:** `game` learns hardware, sockets and mods as content, holds stateful items as copies (a `Stack` carrying an `ItemCopy`), fits and unfits by action, and applies the worn deck's mods to the trace and to power. `strata-zero` draws the hardware (`ui/hardware.rs`), the pawn deck's cradles, marks and cards in the pane, and splits the blueprint window into an inspect mode (the item's own view) and a modding mode that works over a small `Art` seam any item drawing implements.

**Tech Stack:** Rust 2024, ratatui, serde/RON content, `cargo test`, `cargo clippy`.

**Spec:** `docs/superpowers/specs/2026-10-19-item-sockets-design.md` (read it with this plan; its *Art reference* holds every drawing's cells and colours).

## Global Constraints

- Fits the 100×30 minimum terminal; the blueprint window there is 96×28.
- Single-width glyphs only; every colour an exact `Color::Rgb`.
- Wording never names particular pieces where it names a socket kind: an empty socket reads `empty · takes deck hardware`.
- Mod lines: `trace per cast −10%`, `program power +1` (U+2212 minus; `+` for positive power).
- Meatspace only: every fit or unfit is refused jacked in with `actions::JACKED_IN` and nothing changes.
- Modding mode must not know it is looking at a deck: it only uses the `Art` seam.
- Comments and doc lines in the code's existing register: short, plain, saying why.
- Every task ends with `cargo test --workspace -q` green and `cargo clippy --workspace --all-targets -- -D warnings` clean, run from the worktree (its own `target/`).

## Review Focus

- Two copies of the same item (two rebuilt decks, only one modded): every row, Inspect, Equip and Drop must act on the copy the row shows. Test in Task 6.
- A per-copy item that arrived without a copy (plain `give`, a test writing `equipment` directly, the console): it lists, shows empty sockets and can be fitted (the copy is made on first need). Test in Task 3.
- Hardware fitted, then the deck taken off and put back on: effects only while worn, the hardware never lost. Test in Task 4.
- Counting: a fitted piece is not in the pack (`count` excludes it), so jobs, recipes and the tray never see it. Test in Task 3.
- The modding mode at 120×40: the drawing sits exactly where inspect draws it, the tray band at the window's bottom. Test in Task 8.

---

### Task 1: Hardware, sockets and mods in content

**Files:**
- Modify: `crates/game/src/model.rs`, `crates/game/src/content.rs`, `flavor/content.ron`, `crates/strata-zero/src/render/glyphs.rs` (and any other exhaustive `match` on `ItemKind` the compiler names)

**Interfaces:**
- Produces (all in `game::model`, re-exported at the crate root as today):
  - `ItemKind::Hardware`, last in `ItemKind::ALL`, label `"hardware"`; glyph `'≡'`.
  - `enum SocketKind { Deck }` (serde, Copy, Eq, Hash) with `fn takes(self) -> &'static str` = `"deck hardware"` and `fn fits(self) -> &'static str` = `"deck socket"`.
  - `enum Mod { TraceCut(u8), Power(i8) }` (serde, Copy, Eq) with `fn line(&self) -> String` (`"trace per cast −10%"`, `"program power +1"`) and `fn sticker(&self) -> String` (`"TRACE −10%"`, `"POWER +1"`).
  - `enum HardwareLook { ColdfinI, ColdfinII, Sidecar }` (serde, Copy, Eq, Hash).
  - `struct Hardware { pub fits: SocketKind, pub mods: Vec<Mod>, pub look: HardwareLook }` (serde).
  - `Item.sockets: Vec<SocketKind>` and `Item.hardware: Option<Hardware>`, both `#[serde(default)]`.
  - `Item::per_copy(&self) -> bool`: has sockets or is hardware.
  - `DeckLook::socket_names(self) -> &'static [&'static str]`: Pawn → `&["L", "R"]` (its length is the look's room for sockets).
  - `ContentError::SocketsInvalid { id: String, why: String }` and `ContentError::HardwareInvalid { id: String, why: String }`, shaped like `DeckInvalid`.

- [ ] **Step 1: Write the failing tests** in `content.rs`'s test module:
  - `hardware_and_sockets_load`: `Content::load()`; `rebuilt_deck.sockets == vec![SocketKind::Deck, SocketKind::Deck]`; `pawnshop_deck.sockets.is_empty()`; `coldfin_1` is `ItemKind::Hardware`, name `"Coldfin vI"`, weight 0.4, value 800, hardware `fits: Deck, mods: [TraceCut(10)], look: ColdfinI`; `coldfin_2` "Coldfin vII", 0.6, 1600, `[TraceCut(20)]`, `ColdfinII`; `sidecar` "Sidecar co-processor", 0.2, 900, `[Power(1)]`, `Sidecar`; `per_copy` true for `rebuilt_deck` and `coldfin_1`, false for `pawnshop_deck` and `crack`.
  - `mod_lines_read_as_the_card_does`: `Mod::TraceCut(10).line() == "trace per cast −10%"`, `Mod::Power(1).line() == "program power +1"`, `Mod::TraceCut(20).sticker() == "TRACE −20%"`, `Mod::Power(1).sticker() == "POWER +1"`; `SocketKind::Deck.takes() == "deck hardware"`, `.fits() == "deck socket"`.
  - `validation_refuses_bad_hardware_and_sockets`: on a loaded `Content`, each mutation makes `validate()` return the named error: a `Hardware`-kind item with `hardware: None` → `HardwareInvalid`; `hardware: Some(..)` on `crack` → `HardwareInvalid`; hardware with empty `mods` → `HardwareInvalid`; `rebuilt_deck.sockets` of three `Deck` → `SocketsInvalid`; `sockets: [Deck]` on `salvage_jacket` (no drawing places sockets) → `SocketsInvalid`.

- [ ] **Step 2: Run them to see them fail** — `cargo test -p game -q hardware_and_sockets_load mod_lines validation_refuses_bad` → compile errors (unknown types).

- [ ] **Step 3: Implement** the types above, the validation (in `validate`'s item loop beside the deck checks: sockets only on `ItemKind::Deck` items with a `deck`, at most `look.socket_names().len()`; `kind == Hardware` iff `hardware.is_some()`; hardware mods non-empty), the glyph, and the content: the three items as the spec's *Items and copies* lists them (ids, names, weights, values, mods, looks, the exact `desc` text), placed after the decks, and `sockets: [Deck, Deck]` on `rebuilt_deck`.

- [ ] **Step 4: Run** `cargo test --workspace -q` → all pass; `cargo clippy --workspace --all-targets -- -D warnings` → clean.

- [ ] **Step 5: Commit** `feat(game): hardware, sockets and mods as content`.

---

### Task 2: Items held per copy

**Files:**
- Modify: `crates/game/src/character.rs`, `crates/game/src/action.rs`, `crates/game/src/ground.rs`; every `Stack { item, qty }` literal the compiler finds (`strata-zero` tests included), `crates/strata-zero/src/screens/console.rs` (spawn uses `give_item`)

**Interfaces:**
- Consumes: Task 1's `Item::per_copy`, `Item.sockets`.
- Produces (in `game::character` unless said):
  - `struct CopyId(pub u32)` (Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord).
  - `struct ItemCopy { pub id: CopyId, pub fitted: Vec<Option<Stack>> }` (Clone, Debug, PartialEq): one entry per socket; a fitted entry is a `Stack` of qty 1 with its own copy.
  - `Stack { pub item: ItemId, pub qty: u32, pub copy: Option<ItemCopy> }`; `Stack::new(item, qty) -> Stack` (no copy); `Stack::copy_id(&self) -> Option<CopyId>`; `Stack::weight(&self, c: &Content) -> f32` and `Stack::value(&self, c: &Content) -> u32`, both including fitted pieces. A stack with a copy has qty 1 and never merges.
  - `Character.worn: BTreeMap<Slot, ItemCopy>`: the copy of a worn per-copy item (absent for stateless worn items); `Character.next_copy: u32`.
  - `Character::new_copy(&mut self, c: &Content, item: &ItemId) -> ItemCopy`: next id, `fitted` = one `None` per `item.sockets`.
  - `Character::give_item(&mut self, c: &Content, item: ItemId, qty: u32)`: per-copy items as `qty` stacks of one with fresh copies; anything else as `give`.
  - `Character::give` keeps its signature and only merges into stacks without a copy.
  - `Character::copy_in_pack(&self, id: CopyId) -> Option<&Stack>`; `Character::worn_copy(&self, slot: Slot) -> Option<&ItemCopy>`; `Character::is_worn_copy(&self, id: CopyId) -> bool`.
  - `game::action::take_from_pack(ch, item: &ItemId, qty: u32) -> Vec<Stack>` (removes and returns, splitting stateless stacks; programs leave `loaded` as `remove_from_pack` does today); `remove_from_pack` keeps its signature on top of it.
  - `Action::EquipCopy(CopyId)`, `Action::DropCopy(CopyId)`, `Action::TakeCopy(CopyId)`.
  - `Ground::put(&mut self, place, p, stack: Stack)` (merges only copy-less stacks of the same item); `Ground::add` stays as `put(Stack::new(..))`; `Ground::take` takes from copy-less stacks only; `Ground::take_copy(&mut self, place, p, id: CopyId) -> Option<Stack>`; `Ground::take_all(&mut self, place, p) -> Vec<Stack>`.

Behaviour: `Equip(id)` takes the first stack of `id` in pack order (a copy if it has one) and, with a copy, moves it to `worn[slot]`; the item it replaces comes back with its worn copy as its own stack (or merged by `give` when stateless). `EquipCopy` does the same for that copy. `Unequip` brings the worn copy back with its fitted pieces. `Drop { item, qty }` drops what `take_from_pack` returns, each copy its own ground stack; `DropCopy` drops that one. `Take` takes copy-less stacks only; `TakeCopy` takes that copy; `TakeAll` takes every stack as it lies. `weight`/`value` sum `Stack::weight`/`value` over the pack plus worn items (a worn copy's fitted pieces included). `from_build` gives kits through `give_item` and wears a per-copy kit item with a new copy; `debug_runner` wears `rebuilt_deck` with a new copy (both sockets empty) and gives `coldfin_1`, `coldfin_2`, `sidecar` through `give_item`. The console's spawn uses `give_item`.

- [ ] **Step 1: Write the failing tests** (`character.rs`, `action.rs`, `ground.rs` test modules):
  - `per_copy_items_never_stack`: `give_item(rebuilt_deck, 2)` makes two stacks of qty 1 with distinct ids and `fitted == [None, None]`; `give_item(crack, 2)` one stack of 2; `give(crack, 1)` after it makes 3; `count(rebuilt_deck) == 2`.
  - `wearing_a_copy_keeps_its_state`: put `Some(coldfin_2 copy stack)` into socket 0 of a pack deck copy by hand; `EquipCopy(that id)` → `worn[Deck].fitted[0]` holds it and the pack has no rebuilt deck; `Unequip(Deck)` → the pack's stack has that copy id and the piece still fitted; `weight` includes 1.8 + 0.6.
  - `equip_by_id_takes_the_first_copy`: two deck copies, `Equip(rebuilt_deck)` wears the first in pack order; the old worn deck returns.
  - `drop_and_take_keep_copies`: `DropCopy(id)` puts the copy (with its fitted piece) on the ground; `TakeAll` returns it intact; `DropCopy` of an unknown id refuses and changes nothing; `Take { item: rebuilt_deck, qty: 1 }` on a tile holding only a copy refuses (`no Rebuilt pawn deck here`); `TakeCopy(id)` takes it.
  - `ground_put_merges_only_plain_stacks`: two copies of the same item stay two stacks; two `add(crack, 1)` merge.
  - `debug_runner_carries_hardware`: D'SHAWN wears a rebuilt deck copy with two empty sockets; `count(coldfin_1) == count(coldfin_2) == count(sidecar) == 1`, each its own copy.

- [ ] **Step 2: Run** `cargo test -p game -q` → fail to compile (new names).

- [ ] **Step 3: Implement** the interfaces and behaviour above. Add `copy: None` (or `Stack::new`) to every `Stack` literal the compiler names across the workspace; keep `equipment` and `equipped()` as they are.

- [ ] **Step 4: Run** `cargo test --workspace -q` and clippy → green.

- [ ] **Step 5: Commit** `feat(game): items with state of their own are held per copy`.

---

### Task 3: Fitting and removing hardware

**Files:**
- Modify: `crates/game/src/action.rs`, `crates/game/src/character.rs`, `crates/strata-zero/src/actions.rs`

**Interfaces:**
- Consumes: Task 1 (`SocketKind`, `Hardware`, `DeckLook::socket_names`), Task 2 (copies).
- Produces:
  - `enum Host { Worn(Slot), Pack(CopyId) }` (Clone, Copy, Debug, PartialEq, Eq) in `game::action`.
  - `Action::Fit { host: Host, socket: usize, piece: CopyId }` and `Action::Unfit { host: Host, socket: usize }`.
  - `Character::host_item(&self, host: Host) -> Option<&ItemId>`; `Character::host_copy(&self, host: Host) -> Option<&ItemCopy>`; `Character::host_copy_mut(&mut self, c: &Content, host: Host) -> Option<&mut ItemCopy>`, which makes the copy on first need for a per-copy item that has none (worn without an entry in `worn`, or a copy-less pack stack, which is split to one).
  - `game::action::socket_name(c: &Content, item: &ItemId, socket: usize) -> String`: the deck look's name (`"L"`, `"R"`), else `socket + 1`.
  - `strata-zero::actions::handles_items` matches `Fit` and `Unfit`.

Messages (the log's register, names in backticks): fit `` `Coldfin vII` fitted · socket L ``; swap `` `Coldfin vII` fitted · socket L · `Coldfin vI` to the pack ``; unfit `` `Coldfin vI` out · to the pack ``. Refusals, all before any change: `no such socket` (host missing or socket out of range), `` `X` is not in your pack `` (piece id not a pack copy), `` `X` does not fit a deck socket `` / `` `X` is not hardware `` (kind mismatch: `` `X` does not fit `` followed by the socket kind's `fits()`), `socket L is empty` (unfit of an empty socket).

- [ ] **Step 1: Write the failing tests** (`action.rs`):
  - `fit_moves_the_piece_into_the_socket`: D'SHAWN; `Fit { host: Worn(Deck), socket: 0, piece: coldfin_2's id }` → message `` `Coldfin vII` fitted · socket L ``; `worn_copy(Deck).fitted[0]` is that copy; `count(coldfin_2) == 0` (Review Focus: fitted pieces are not in the pack).
  - `fitting_a_filled_socket_swaps`: then `Fit` socket 0 with `coldfin_1` → message ends `` · `Coldfin vII` to the pack ``; `count(coldfin_2) == 1` with its original copy id.
  - `unfit_returns_the_piece`: `Unfit { Worn(Deck), 0 }` → `` `Coldfin vI` out · to the pack ``; socket empty; `Unfit` again → `Err("socket L is empty")`.
  - `fit_refusals_change_nothing`: socket 2, an unknown piece id, a crack id (not a copy), fitting into the worn `salvage_jacket` (no sockets) → each `Err`, character unchanged (`assert_eq!` against a clone).
  - `fit_into_a_carried_deck`: `give_item(rebuilt_deck, 1)`, fit into `Host::Pack(id)` → that stack's copy holds the piece.
  - `a_deck_worn_without_a_copy_takes_hardware`: a fresh Vex with `equipment.insert(Deck, rebuilt_deck)` and `worn` empty; `Fit` socket 1 works and `worn[Deck]` now exists with `fitted[1]` set (Review Focus).
  - In `strata-zero/src/actions.rs`'s `jacked_in_the_gear_and_the_pack_stay_put`: add `Action::Fit { .. }` and `Action::Unfit { .. }` to the refused list.

- [ ] **Step 2: Run** `cargo test -q -p game fit_ unfit_ a_deck_worn` → compile errors.

- [ ] **Step 3: Implement** `Host`, the two actions in `apply` (checks first, then the move), the host helpers, `socket_name`, and `handles_items`.

- [ ] **Step 4: Run** the workspace tests and clippy → green.

- [ ] **Step 5: Commit** `feat(game): fit and unfit hardware, refused jacked in`.

---

### Task 4: The worn deck's hardware changes runs

**Files:**
- Modify: `crates/game/src/character.rs`, `crates/game/src/run.rs`, `crates/strata-zero/src/net.rs`, `crates/strata-zero/src/helix/bin.rs` (the two `add_heat` calls pass a cut of 0)

**Interfaces:**
- Consumes: Task 2/3 (worn copies).
- Produces:
  - `Character::deck_mods(&self, c: &Content) -> Vec<Mod>`: the mods of the hardware fitted in `worn[Deck]` (content mods today).
  - `Character::trace_cut(&self, c: &Content) -> u8`: sum of `TraceCut`, capped at 50.
  - `Character::deck_power(&self, c: &Content) -> i32`: sum of `Power`.
  - `run::power` adds `ch.deck_power(c)`.
  - `Trace { heat, alert_until, #[serde(default)] carry: u8 }` (tenths, always < 10); `Trace::add_cut(&mut self, noise: u8, cut: u8, now: u64) -> TraceEvent`; `Trace::add(noise, now)` = `add_cut(noise, 0, now)`.
  - `NetState::add_heat(&mut self, b, noise: u8, cut: u8, now: u64, null_route: bool)`.
  - `net::noise_on(st, b, noise) -> bool` applies `st.character.trace_cut(..)`; new `net::ice_heat(st, b, noise) -> bool` is the same with no cut; the white ICE hit uses `ice_heat`. Ping, arc trace, the mask's held noise and program turns keep calling `noise_on`.

`add_cut`: tenths = `noise as u32 * (100 - cut) as u32 / 10 + carry`; heat gains `tenths / 10` (through today's add logic, alarm and dump included), `carry = tenths % 10`.

- [ ] **Step 1: Write the failing tests**:
  - `run.rs` `trace_carries_tenths`: `add_cut(4, 10, 0)` twice → heat 3 then 7 (36 + 36 tenths), carry 2; `add_cut(4, 0, 0)` on a fresh trace → heat 4, carry 0; a carry does not stop `add` from reaching the dump line exactly as today.
  - `character.rs` `deck_mods_come_from_the_worn_deck`: D'SHAWN with `coldfin_2` fitted → `trace_cut == 20`; plus `coldfin_1` in socket 1 → 30; with two `coldfin_2` (one `give_item`ed) → 40; `deck_power` 0, and with `sidecar` fitted → 1; `Unequip(Deck)` → `trace_cut == 0` and `deck_power == 0`; equip it back → 20 again (Review Focus). A fake `TraceCut(40)` twice (content mutated in the test) caps at 50.
  - `run.rs` `the_sidecar_adds_power`: `power` with `sidecar` fitted is one more than without; `chance` moves by 10 where it is not already 0 or 100.
  - `net.rs` `hardware_cuts_your_trace_not_ice`: in a construct (follow an existing net test's setup), D'SHAWN with `coldfin_1` fitted; `noise_on(st, b, 4)` twice → heat 7; `ice_heat(st, b, 5)` → heat 12; without hardware `noise_on(st, b, 4)` → 4 more exactly.

- [ ] **Step 2: Run** them → fail.

- [ ] **Step 3: Implement** the interfaces above.

- [ ] **Step 4: Run** the workspace tests and clippy → green (every existing trace test still passes unchanged: cut 0 is today's arithmetic).

- [ ] **Step 5: Commit** `feat(net): the worn deck's hardware cuts your trace and adds power`.

---

### Task 5: The hardware's drawings

**Files:**
- Create: `crates/strata-zero/src/ui/hardware.rs` (register in `ui/mod.rs`)
- Modify: `crates/strata-zero/src/ui/cyberdeck/mod.rs` (only if `Pen` needs a `pub(crate)` constructor or method)

**Interfaces:**
- Consumes: `game::{HardwareLook, Mod}`, `crate::ui::cyberdeck::Pen`.
- Produces:
  - `pub const W: u16 = 52; pub const H: u16 = 18;`
  - `pub enum Side { Left, Right }`
  - `pub fn draw_large(look: HardwareLook, mods: &[Mod], p: &mut Pen)`: the 52×18 drawing at the pen's origin; stickers and silkscreen read the first mod's `sticker()` (`TRACE −10%`, `POWER +1`; the Sidecar's silkscreen is `· ` + sticker).
  - `pub fn draw_side(look: HardwareLook, side: Side, p: &mut Pen, x: i32, y: i32)`: the 5×5 drawing for a deck socket with its top-left at (x, y) in pen coordinates; each look is defined for one side (Coldfins left, Sidecar right) and mirrored for the other (columns reversed, `▌`↔`▐`).
  - `pub struct Tile { pub bg: Color, pub ink: Color, pub swatch: (Color, Color) }`; `pub fn tile(look: HardwareLook) -> Tile`; `pub const EMPTY_TILE: Tile`.

All cells and colours are the spec's *Art reference* (side drawings, large drawings, tray tiles); the vII fan is the pixel rule given there, painted as `▀` with the top pixel as fg and the bottom as bg.

- [ ] **Step 1: Write the failing tests** (in `hardware.rs`, drawing into a `Buffer` through a `Pen` at origin (0,0) or (10,0) for side drawings):
  - `coldfin_large_prints_its_number`: vI with `[TraceCut(10)]`: cell (0,6) is `▌` gold `#c9a227`; row 16 from col 31 reads ` TRACE −10% ` on frost `#8fd8ff`; row 16 from col 9 reads ` COLDFIN vI `. vII with `[TraceCut(20)]` reads ` TRACE −20% ` at (36,16) and `VOID` at (41,1); its fan's centre pixel (fan origin (22,5), pixel (7,7) → cell (29,8) fg or bg) is blade hi `#c3ccd5`.
  - `sidecar_large`: `SDC-7` at (21,7) on chip `#141218`; ` QC ` at (44,2) on hazard `#ffcc00`; `· POWER +1` at (31,14).
  - `side_drawings_mirror`: Coldfin vI left at (−4,7) (pen origin 10): cell (10+0, 9) is `█` copper `#d07a3a`; right at (83,7) on a wider buffer: the base plate is at col 83 and the fins at 84..87. Sidecar right at (83,7): (83,8) `▌` gold; left at (−4,7): (0,8) `▐` gold. vII left: (−3,8) top pixel is blade `#8a96a2`.
  - `tiles`: `tile(ColdfinI).bg == Rgb(0xb4,0xbe,0xc8)`, `tile(Sidecar).ink == Rgb(0xd8,0xf0,0xde)`, `EMPTY_TILE.bg == Rgb(0x2a,0x28,0x30)`.

- [ ] **Step 2: Run** `cargo test -p strata-zero -q ui::hardware` → fail.

- [ ] **Step 3: Implement** the module.

- [ ] **Step 4: Run** the tests and clippy → green.

- [ ] **Step 5: Commit** `feat(ui): the hardware drawn large and in a deck's socket`.

---

### Task 6: The pane shows copies, slots, sockets and hardware

**Files:**
- Modify: `crates/strata-zero/src/screens/deck.rs`, `crates/strata-zero/src/screens/popup.rs`, `crates/strata-zero/src/screens/blueprint.rs` (the new `open` parameter only), `crates/strata-zero/src/interact.rs`, `crates/strata-zero/src/screens/cyberdeck.rs` (the `item_card` caller)

**Interfaces:**
- Consumes: Tasks 1-3.
- Produces:
  - `Detail::Copy(CopyId)` for an INVENTORY row holding a copy.
  - `Pick::Blueprint { item: ItemId, equipped: bool, copy: Option<CopyId> }`; `Blueprint::open(st, item, equipped, copy: Option<CopyId>)` (stores it; Task 7 uses it).
  - `Deck::item_card_of(st, id: &ItemId, copy: Option<&ItemCopy>) -> Vec<Line<'static>>`; `item_card(st, id)` = `item_card_of(st, id, None)`. With a copy, `equipped` shows only when `is_worn_copy(copy.id)`.
  - `fn marks(st: &AppState, item: &Item, copy: Option<&ItemCopy>, worn: bool, selected: bool) -> Vec<Span<'static>>` in `deck.rs`.

Rows: GEAR rows and INVENTORY rows of decks and of items with sockets get spans: the name, one space, then the marks (spec *The character pane*, exact colours there; selected-row colours when the row is the cursor's). Used slots: `loaded_size` on the worn deck, all free for a deck in the pack. The name is cut with `…` so name + space + marks fit the list pane (`list_w`); the marks never are. INVENTORY lists `ItemKind::Hardware` under `── hardware ──` after programs with `≡`; every copy is its own `1×` row with `Detail::Copy`. Cards: an item with sockets lists, after its grants, `■ <Name> · <mod lines joined ' · '>` (cyan) or `□ empty socket · <kind.takes()>` (dim) per socket, from the copy (empty sockets from content when there is no copy); hardware cards read `hardware · <fits()>` on the second line and each `mod.line()` as a cyan perk line. Popups: the GEAR picker equips copies with `EquipCopy`; a `Detail::Copy` row offers Inspect (`Pick::Blueprint { copy: Some(id), equipped: false }`), Equip (`EquipCopy`) when it has a slot, Drop (`DropCopy`), no Drop all; GEAR's Inspect passes the worn copy's id. The ground's Take rows (`interact.rs`) use `TakeCopy` for a copy.

- [ ] **Step 1: Write the failing tests** (`deck.rs` tests; render at 100×30 as its existing tests do):
  - `gear_marks_slots_and_sockets`: D'SHAWN with `coldfin_2` fitted in socket 0 and the debug loadout: the DECK row reads `Rebuilt pawn… ■■■■■ · ■□` within the list pane; the first `■` is `Rgb(0x1a,0x9a,0xaa)` on the selected row (cursor on DECK) and `Rgb(0x3f,0x7c,0x87)` when another row is selected; the socket `■` is `#00e5ff`, the `□` `#a0aab9` unselected.
  - `inventory_marks_and_hardware`: the pack's `Pawnshop deck □□` (no dot), a second rebuilt deck `Rebuilt pawn deck □□□□□ · □□`, a `── hardware ──` header followed by `≡  1× Coldfin vI`, `≡  1× Coldfin vII`, `≡  1× Sidecar co-processor`.
  - `two_copies_act_on_their_own_row` (Review Focus): two rebuilt decks in the pack, the second with `sidecar` fitted; the second row's marks end `■□`, the first's `□□`; its popup's Inspect is `Pick::Blueprint { copy: Some(second id), .. }` and Drop is `DropCopy(second id)`.
  - `cards_list_sockets_and_mods`: the worn deck's card has `■ Coldfin vII · trace per cast −20%` and `□ empty socket · deck hardware`; the `coldfin_1` card's second line is `hardware · deck socket` and it has `trace per cast −10%`.
  - In `interact.rs` tests: a dropped deck copy on the tile gives a Take row with `TakeCopy(id)`.

- [ ] **Step 2: Run** → fail.

- [ ] **Step 3: Implement** the rows, marks, cards, popups and Take rows.

- [ ] **Step 4: Run** the workspace tests and clippy → green.

- [ ] **Step 5: Commit** `feat(app): the pane marks slots and sockets and lists copies and hardware`.

---

### Task 7: The pawn deck's sockets

**Files:**
- Create: `crates/strata-zero/src/ui/art.rs` (register in `ui/mod.rs`)
- Modify: `crates/strata-zero/src/ui/cyberdeck/mod.rs`, `crates/strata-zero/src/ui/cyberdeck/pawn.rs`, `crates/strata-zero/src/screens/cyberdeck.rs`, `crates/strata-zero/src/screens/blueprint.rs`

**Interfaces:**
- Consumes: Task 5 (`draw_side`, `Side`), Task 6 (`Blueprint::open`'s copy).
- Produces:
  - `ui::art::SocketPlace { pub index: usize, pub x: i32, pub y: i32, pub w: u16, pub h: u16, pub name: &'static str, pub kind: SocketKind }` (drawing coordinates; x may be negative).
  - `ui::art::step<P: Copy + PartialEq>(parts: &[(P, Rect)], from: P, dir: Dir, allow: impl Fn(P) -> bool) -> P`: today's `ui::cyberdeck::step`, moved and made generic (`Dir` moves with it; `ui::cyberdeck` re-exports or its callers update).
  - `DeckView.sockets: Vec<Option<HardwareLook>>` (one per socket, fitted look or none) and `DeckView.bare: bool`.
  - `ui::cyberdeck::sockets(view: &DeckView) -> Vec<SocketPlace>`: Pawn → L `(−4, 6, 6, 7)`, R `(82, 6, 6, 7)`, names from `DeckLook::socket_names`.
  - `ui::cyberdeck::origin(look: DeckLook, area: Rect) -> (u16, u16)`: where `draw` puts the drawing (today's centring), shared by both modes.
  - `DeckTab` gains `copy: Option<CopyId>` (from `Blueprint::open`) and fills `view.sockets` from the worn copy (equipped) or the pack copy; the plate's glass reads `Deck::item_card_of(st, deck, that copy)`.

Pawn: for each socket draw the cradle and, when fitted, `draw_side(look, Left/Right, p, −4 | 83, 7)` (spec *The pawn drawing's sockets*: rails rows 6 and 12, connector rows 8-10 in the edge). Inspect never points at sockets. `bare` draws no glass text, pinned line or scroll marks (glass background stays), no WAREZ row, held cart or landing outline, and `draw` skips the brackets. The cable ports become jacks: `SOCKET_Y` → `JACK_Y`, comments, `DeckView` docs and test names (`sockets_always_show_and_cables_follow_the_deck` → `jacks_always_show_…`); their drawing does not change.

- [ ] **Step 1: Write the failing tests** (`pawn.rs`, `ui/cyberdeck/mod.rs`, `screens/blueprint.rs` tests):
  - `cradles_draw_on_a_deck_with_sockets`: `kit_view()` with `sockets: vec![None, None]`: drawing (−4..=0, 6) is `▄` steel `#8b8596`, (−4, 6) `#c8c2d2`; (1, 9) `▌` gold on well; (82, 9) `▐`; with `sockets: vec![]` none of those cells change from today's.
  - `fitted_hardware_draws_in_its_socket`: `sockets: vec![Some(ColdfinI), Some(Sidecar)]`: (0, 9) is copper `█`, (83, 8) `▌` gold.
  - `jacks_still_draw_where_they_did` (renamed existing test, same assertions).
  - `bare_draws_no_glass_text_warez_or_brackets`: `bare: true` with glass rows set → no glass text in rows 7-12, no `WAREZ`, no `┏` anywhere.
  - `socket_places_and_generic_step`: `sockets(&view)` gives L and R rects as above; `step` over two `(usize, Rect)` places moves Right from 0 to 1 and Left back.
  - In `blueprint.rs` `the_worn_deck_shows_its_cradles`: D'SHAWN's worn deck blueprint at 100×30 (window at x 2, drawing origin x 8): screen (4, oy + 6) is `▄` (socket L's outer rail end, drawing x −4, one column right of the window's first inside column 3) and screen (95, oy + 6) is `▄` (socket R's, drawing x 87); the plate's glass card (point at the plate) lists `□ empty socket · deck hardware` twice.

- [ ] **Step 2: Run** → fail.

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run** the workspace tests and clippy → green.

- [ ] **Step 5: Commit** `feat(app): the pawn deck's sockets, with the cable ports renamed jacks`.

---

### Task 8: The blueprint window's two modes

**Files:**
- Modify: `crates/strata-zero/src/ui/art.rs`, `crates/strata-zero/src/screens/blueprint.rs`, `crates/strata-zero/src/screens/cyberdeck.rs`

**Interfaces:**
- Consumes: Tasks 5-7.
- Produces:
  - `ui::art::Art` trait: `fn size(&self) -> (u16, u16)`; `fn origin(&self, body: Rect) -> (u16, u16)`; `fn draw(&self, origin: (u16, u16), clip: Rect, buf: &mut Buffer)` (the item as it stands, no mode's pointer or readout); `fn sockets(&self) -> Vec<SocketPlace>`.
  - `ui::art::grey(c: Color, panel: Color) -> Color` per the spec's formula (fg always; for bg, the caller passes `panel` to leave the panel colour alone) and `ui::art::grey_area(buf, area: Rect, keep: &[Rect], panel: Color)` (drops bold).
  - `DeckArt` (in `screens/cyberdeck.rs`): built by `DeckTab::art(&self, st) -> DeckArt` from a `bare` view; implements `Art` through `ui::cyberdeck::{origin, draw, sockets}`.
  - `HardwareArt { look: HardwareLook, mods: Vec<Mod> }` (in `ui/hardware.rs`): `Art` with no sockets, centred.
  - `Blueprint { title_name: String, inspect: Inspect, mode: Mode, host: Option<Host>, item: ItemId }` with `enum Inspect { Deck(DeckTab), Hardware(HardwareArt), Missing }` and `enum Mode { Inspect, Mod(Bench) }`; `Bench { point: usize, choosing: Option<Choosing>, offset: usize }` (`Choosing` arrives in Task 9).
  - `DeckTab::hint(&self) -> (&'static str, &'static str)`: the keys and the esc part (`("↑↓←→ point · space act", "esc close")`, holding `("↑↓←→ carry · space drop", "esc put back")`); `DeckTab::holding(&self) -> bool`.

Behaviour: Inspect on hardware draws `HardwareArt` centred (hint `esc close`, no `m`). `m` in inspect switches to modding when the item has sockets (`host` copy's or content's) and the deck is not holding a cart; `m` in modding switches back; each mode keeps its own pointer. Inspect hint: keys, ` · m mod` when allowed, ` · `, esc part. Title ` MODDING · <NAME> ` / ` BLUEPRINT · <NAME> `. Modding draws: the art at `art.origin(body)` clipped to the body, `grey_area` over the body minus the tray band (the five rows above the status row, from the art's left edge rightwards — Task 9 fills it) keeping the socket rectangles (screen coordinates: origin + place), brackets on the pointed socket (`brackets` from `ui::cyberdeck`, made `pub(crate)`), the readout on the row above the status row (spec *The readout*), hint `↑↓←→ socket · space choose · m inspect · esc close`. Arrows and hjkl move between sockets with `ui::art::step`; esc/q close.

- [ ] **Step 1: Write the failing tests** (`blueprint.rs`):
  - `hardware_has_its_own_blueprint`: Inspect `coldfin_2` from the pack → ` BLUEPRINT · COLDFIN VII ` title, `TRACE −20%` on screen, hint `esc close`; `m` does nothing.
  - `m_switches_modes_on_an_item_with_sockets`: D'SHAWN's deck: inspect hint contains `space act · m mod · esc close`; `m` → title ` MODDING · REBUILT PAWN DECK `, hint `↑↓←→ socket · space choose · m inspect · esc close`, readout `SOCKET L ▸ empty · takes deck hardware`; `→` → readout `SOCKET R ▸ empty …`; `m` → back to inspect with the inspect pointer where it was. The pawnshop deck: hint has no `m mod` and `m` changes nothing.
  - `modding_greys_the_item_but_not_its_sockets`: in modding, the maker plate's `K` cell fg equals `grey(Rgb(0x8b,0x85,0x96))` and is not bold; the rail cell at socket L (screen coordinates from the origin) keeps `#8b8596`; the glass shows no `SLOT A`.
  - `modding_keeps_the_drawing_in_place` (Review Focus): at 120×40 and 100×30, the cell of the maker plate's `K` is at the same screen position in both modes.
  - `holding_a_cart_blocks_m`: pick a WAREZ cart up (as the existing slotting test does), press `m` → still inspect.

- [ ] **Step 2: Run** → fail.

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run** the workspace tests and clippy → green.

- [ ] **Step 5: Commit** `feat(app): the blueprint window's modding mode, in grey`.

---

### Task 9: The hardware tray and fitting

**Files:**
- Modify: `crates/strata-zero/src/screens/blueprint.rs` (and a new `crates/strata-zero/src/screens/bench.rs` if `blueprint.rs` grows past ~900 lines: the modding bench's state, keys and drawing)

**Interfaces:**
- Consumes: Task 3 (`Action::Fit`, `Action::Unfit`, `Host`), Task 5 (`tile`, `EMPTY_TILE`), Task 8 (`Bench`).
- Produces: `Choosing { socket: usize, tile: usize }`; the tray (spec *The HARDWARE tray*): the pack's hardware copies whose `fits` matches any socket's kind, pack order, `EMPTY` first while choosing for a filled socket.

Keys while choosing: ←→ and h/l move along the tiles (↑↓ j/k nothing), space performs `Fit { host, socket, piece }` (or `Unfit` for EMPTY) through `actions::perform` and reports with `actions::report`, then returns to the socket; esc/q returns to the socket, nothing changed. Space on a socket while jacked in warns `actions::JACKED_IN` and does not open the tray; with an empty tray it opens nothing and the readout's tray row reads `nothing in your pack fits`. Drawing: the dashed cyan outline one cell outside the chosen socket (`┌ ╌ ┐ ╎ └ ╌ ┘`), brackets on the pointed tile, the readout for the tile (`COLDFIN VII ▸ socket L · trace per cast −20%`, plus ` · Coldfin vI back to the pack` when it would swap; `EMPTY ▸ socket L · Coldfin vII back to the pack`), hint `←→ hardware · space fit · esc back`. Tiles scroll as WAREZ does, with `‹`/`›` when cut.

- [ ] **Step 1: Write the failing tests** (`blueprint.rs`):
  - `choosing_fits_the_pointed_piece`: D'SHAWN, `m`, space → the tray shows `HARDWARE`, `COLDFIN VI`, `COLDFIN VII`, `SIDECAR CO-PROCESSOR`; `→` → readout `COLDFIN VII ▸ socket L · trace per cast −20%`; space → status `Coldfin vII fitted · socket L` (log text without the backtick marks), `worn_copy(Deck).fitted[0]` is `coldfin_2`, the readout `SOCKET L ▸ Coldfin vII · trace per cast −20%`, the tray no longer holds it.
  - `a_filled_socket_offers_empty_and_swaps`: space on socket L again → first tile `EMPTY`; readout `EMPTY ▸ socket L · Coldfin vII back to the pack`; `→` to `COLDFIN VI`, readout ends `· Coldfin vII back to the pack`; space swaps; then space, EMPTY, space → socket empty, both Coldfins in the pack.
  - `esc_while_choosing_changes_nothing`: space, `→`, esc → character unchanged, pointer on socket L, hint back to the socket hint.
  - `jacked_in_modding_only_looks`: jacked in (as the existing lock test sets up), `m` works, space → status `JACKED_IN` as a warning, no tray, character unchanged.
  - `an_empty_tray_says_so`: Vex wearing a rebuilt deck (given via `give_item` and `EquipCopy`) with no hardware: space → `nothing in your pack fits` on screen, still on the socket.
  - `a_carried_deck_can_be_modded`: Inspect a pack deck copy, `m`, fit `sidecar` → that pack copy holds it, the worn deck does not.

- [ ] **Step 2: Run** → fail.

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run** the workspace tests and clippy → green.

- [ ] **Step 5: Commit** `feat(app): the hardware tray: fit, swap and take out`.

---

### Task 10: Empty sockets, picker marks, shots, README

**Files:**
- Modify: `crates/strata-zero/src/ui/cyberdeck/pawn.rs` (and its tests), `crates/strata-zero/src/screens/blueprint.rs` (tests touching the rails), `crates/strata-zero/src/screens/deck.rs` (GEAR picker labels), `crates/strata-zero/src/screens/shots.rs`, `README.md`

Added after Tasks 1-9 were built:
- **Empty sockets are the connector alone** (Jeffrey, on seeing it: "drop those [prongs] … just the socket is enough"; spec *The pawn drawing's sockets* updated): the pawn draws the gold connector for every socket, and the steel rails (rows 6 and 12) only round a fitted piece. The socket rectangles (`sockets()`) do not change, so modding's brackets still frame where a piece goes.
- **The GEAR picker shows a copy's marks** (controller ruling after Task 6): each picker row reads the item name, one space, then the same marks the INVENTORY row shows for that copy (`marks(..)`, plain colours), so two copies of one deck read apart.

- [ ] **Step 0a: Write the failing tests**: in `pawn.rs`, `empty_sockets_draw_only_the_connector`: `sockets: vec![None, None]` → drawing cells (−4..=0, 6) and (−4..=0, 12) and (83..=87, 6/12) are unchanged from a deck without sockets (no `▄`/`▀` steel), (1, 9) is still `▌` gold on well; `sockets: vec![Some(ColdfinI), None]` → (−4, 6) is `▄` `#c8c2d2`, (−3..=0, 6) `▄` `#8b8596`, and the R side has no rails. Update `cradles_draw_on_a_deck_with_sockets` and `the_worn_deck_shows_its_cradles` (screens/blueprint.rs) to the new rule (D'SHAWN's empty deck: no rail at screen (4, oy+6) or (95, oy+6); the gold connectors still there). In `deck.rs`, `gear_picker_reads_copies_apart`: D'SHAWN with a second rebuilt deck in the pack holding `sidecar` in socket 0 → the DECK slot's picker rows read `Rebuilt pawn deck □□□□□ · □□` and `Rebuilt pawn deck □□□□□ · ■□` and equip their own copy ids. Run them → fail.

- [ ] **Step 0b: Implement** both changes; run the workspace tests and clippy → green; commit `feat(app): empty sockets show only their connector; the picker reads copies apart`.

- [ ] **Step 1: Add ignored shots** in `shots.rs` beside `dshawns_decks_on_their_blueprints`: `modding_the_rebuilt_deck` (D'SHAWN, `m` on the worn deck, 100×30 and 120×40; one with the tray open), `hardware_blueprints` (each of the three pieces at 100×30), `gear_and_inventory_marks` (GEAR and INVENTORY tabs at 100×30 with a Coldfin vII fitted). Run `SHOT_DIR=<scratch dir> cargo test -p strata-zero -q shots:: -- --ignored` and look at the SVGs: nothing clipped at 100×30, the drawings match the spec's art reference.

- [ ] **Step 2: README**: in the blueprint paragraph (around "Inspect on an item opens its blueprint"), add the modding mode (`m`, grey, sockets only, the HARDWARE tray, fit/swap/take out, meatspace only), the hardware (Coldfin vI/vII, Sidecar: what each does), and the pane's marks (`■■■■■ · ■□`: slots, then sockets). Keep the README's register.

- [ ] **Step 3: Run** `cargo fmt --all --check`, `cargo test --workspace -q`, `cargo clippy --workspace --all-targets -- -D warnings` → all clean.

- [ ] **Step 4: Commit** `docs: modding mode and hardware in the README; shots`.
