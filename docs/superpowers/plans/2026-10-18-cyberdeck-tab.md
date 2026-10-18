# The CYBERDECK tab as a deck: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the CYBERDECK tab's list with a drawn deck (the pawn look): carts in a bay, a glass that reads out the pointed part, a guarded failsafe, HEAT, and the programs in inventory, driven by corner brackets and pick-up/carry/drop.

**Architecture:** The rules crate learns a deck's look (`DeckLook::Pawn`) and a slot-at-position action that plans before it changes anything. `ui/cyberdeck/` draws a `DeckView` snapshot through the look (geometry, navigation, brackets, the pawn drawing) and knows nothing of `AppState`. `screens/cyberdeck.rs` owns the tab's state (pointed part, held program, cover, glass scroll), builds the `DeckView`, the glass text and the heat, and handles the tab's keys; `screens/deck.rs` delegates to it as it does for TALENTS.

**Tech Stack:** Rust workspace, ratatui buffers, serde/RON content, `cargo test`.

**Spec:** `docs/superpowers/specs/2026-10-18-cyberdeck-tab-design.md` (read it first: the pawn drawing row by row, colours, glass rules, slotting, failsafe, heat, keys).

## Global Constraints

- The pawn drawing is fixed at 84×22 and must fit the deck body at the minimum terminal 100×30 (body 98×22); centred in bigger bodies.
- Colours (pawn module constants): body `#24212a`, lighter body `#322e3a`, bezel `#141218`, rust edge `#a0583a`, cart `#3b3842`, tape `#d9c9a0`, loose tape `#8a8270`, ink `#1a1612`, glass `#140e06` / `#100b04`, amber `#ffb000`, dim amber `#8c6014`, hazard `#ffcc00`, cover `#5a0c14`, hot `#ff6a00`, keys `#4c4855`; cyan and magenta from `ui::chrome`.
- Maker plate text: ` K▒ROK▒WA KX-2 `. Sticker: ` NO FUTURE ` (bold white on magenta), ` x‿x ` (bold black on hazard yellow).
- Strip copy: "failsafe off · black ICE can kill you" (warn), "failsafe on", "`Spoof` slotted · `Attack` to inventory", "`Spoof` loaded · 2/2 slots", "no deck jacked".
- Glass pinned rows: `[space] eject`, `[space] pick up`, `[space] lift the cover`, `[space] throw · esc shut`, `[space] throw it back`, `[space] swap · esc put back`, `[space] slot · esc put back`, `burned in · can't be pulled`, `empty · bring a program from inventory`.
- Hints: `↑↓←→ point · space act · tab tabs · esc close`; holding: `↑↓←→ carry · space drop · esc put back`.
- No change to what programs, the failsafe or heat do, or to when loading is allowed. No mouse, no animation beyond pulsing brackets and the glass's type-on.
- Every glyph drawn must be single width (`deck_glyphs_are_single_width` style); `cargo clippy --workspace --all-targets -- -D warnings` stays clean.

## Review Focus

- Inventory with more programs than fit 84 columns: the row scrolls to keep the pointed cart visible and shows `‹` / `›` (test in Task 5).
- The lists change under the tab (a program dropped, the deck unequipped, the tab reopened later): a stale `Part` clamps to a valid one, never panics (tests in Tasks 5 and 6).
- A long handle (`D'SHAWN`, or 12 letters): the tag never draws into the failsafe frame (test in Task 4).
- Odd decks: no grants (no ROM parts, `ROM▸` alone), one slot, four slots (tests in Tasks 3 and 4).
- A big terminal (200×60): the drawing is centred in the body and nothing lands outside it (test in Task 6).

---

### Task 1: Deck looks in the rules crate

**Files:**
- Modify: `crates/game/src/model.rs:175-180` (DeckSpec)
- Modify: `crates/game/src/content.rs` (validation in the `(i.kind, i.deck)` match near line 170; the test literal near line 1010)
- Modify: `flavor/content.ron:26`
- Test: `crates/game/src/content.rs` tests module

**Interfaces:**
- Produces: `game::DeckLook` (`#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)] pub enum DeckLook { #[default] Pawn }`), `DeckLook::max_slots(self) -> u8` (Pawn 4), `DeckLook::max_grants(self) -> usize` (Pawn 3); `DeckSpec { slots: u8, rating: u8, #[serde(default)] look: DeckLook }`.

- [ ] **Step 1: Write the failing tests** in `content.rs` tests:
  - `deck_look_defaults_to_pawn`: parse a minimal content string (or the default content with the pawnshop deck's `look: Pawn` removed via `str::replace`) and assert `item("pawnshop_deck").deck.unwrap().look == DeckLook::Pawn`.
  - `pawn_decks_have_room_for_four_slots_and_three_grants`: clone `Content::load()`, set the pawnshop deck's `slots = 5`, `validate()` is `Err(ContentError::DeckInvalid { .. })`; reset, push a fourth grant (any ability id), `validate()` is `Err(DeckInvalid)`; with `slots = 4` and 3 grants it is `Ok`.
- [ ] **Step 2: Run** `cargo test -p game deck_look pawn_decks` — fails to compile (no `DeckLook`).
- [ ] **Step 3: Implement** `DeckLook` and the field in `model.rs`; in `validate()` add, for a `Deck` item with `Some(d)`: `d.slots > d.look.max_slots()` → `DeckInvalid { why: "a pawn deck has room for 4 slots" }`, `i.grants.len() > d.look.max_grants()` → `DeckInvalid { why: "a pawn deck has room for 3 ROM abilities" }` (word the look by a `DeckLook::name(self) -> &'static str`, Pawn → "pawn"). Add `look: Pawn` to the pawnshop deck in `content.ron`: `deck: Some(DeckSpec(slots: 2, rating: 1, look: Pawn))`. Fix the literal at `content.rs:1010` with `look: DeckLook::Pawn`.
- [ ] **Step 4: Run** `cargo test -p game` — all pass.
- [ ] **Step 5: Commit** `feat(game): a deck says which look it wears, the pawn look first`

### Task 2: Slotting a program at a position

**Files:**
- Modify: `crates/game/src/character.rs` (after `unload`, ~line 205)
- Modify: `crates/game/src/action.rs:34-36` (enum) and `:243-244` (apply)
- Test: `crates/game/src/character.rs` tests (or `action.rs` tests, next to the existing load/unload test near line 600)

**Interfaces:**
- Consumes: `Character::can_load`'s refusals and order, `Character::deck`, `loaded_size`.
- Produces:
  - `#[derive(Clone, Debug, PartialEq, Eq)] pub struct SlotPlan { pub loaded: Vec<ItemId>, pub at: usize, pub displaced: Vec<ItemId> }` (`loaded` = the order after; `at` = the slotted program's index in it; `displaced` = what goes back to the pack, in removal order).
  - `Character::plan_slot(&self, c: &Content, id: &ItemId, at: usize) -> Result<SlotPlan, String>` (pure).
  - `Character::slot_at(&mut self, c: &Content, id: &ItemId, at: usize) -> Result<String, String>`.
  - `Action::Slot { id: ItemId, at: usize }`.

- [ ] **Step 1: Write the failing tests** (Vex's kit: `loaded = [crack, copy]`, attack in the pack, deck 2 slots; give `spoof`/`backdoor` with `ch.give(id, 1)`):
  - `slot_at_the_end_loads`: unload copy, `slot_at(spoof, 1)` → `loaded == [crack, spoof]`, message "`Spoof` loaded · 2/2 slots".
  - `slot_onto_a_program_swaps_in_place`: `slot_at(spoof, 0)` → `loaded == [spoof, copy]`, message "`Spoof` slotted · `Crack` to inventory".
  - `a_size_two_program_over_two_sends_both_back`: `plan_slot(backdoor, 0)` → `loaded == [backdoor]`, `at == 0`, `displaced == [crack, copy]`; message after `slot_at` "`Backdoor` slotted · `Crack`, `Copy` to inventory".
  - `a_size_two_program_appended_displaces_the_one_before`: unload copy, `plan_slot(backdoor, 1)` → `loaded == [backdoor]`, `displaced == [crack]`.
  - `plan_order_after_then_before`: on a 3-slot deck (push a clone of the pawnshop deck with `slots: 3` into the content and equip it) with `loaded = [crack, copy, attack]`, `plan_slot(backdoor, 1)` → `displaced == [copy, attack]`, `loaded == [crack, backdoor]`.
  - `slot_refusals_change_nothing`: each leaves `loaded` and `inventory` equal to before: not a program (`copper_coil`) "that is not a program"; not carried "not in your pack"; already loaded "already loaded"; `at = 3` with 2 loaded "no such slot"; a size-3 program (clone `backdoor` with `size: 3` into the content) on the 2-slot deck "too big for this deck: needs 3 of 2 slots"; no deck equipped "no deck jacked".
  - `apply_slot_action`: `apply(.., Action::Slot { id: spoof, at: 0 })` returns `Ok` with the swap message.
- [ ] **Step 2: Run** `cargo test -p game slot` — fails to compile.
- [ ] **Step 3: Implement** `plan_slot`: the `can_load` checks except "no room" (same messages, same order), then `at <= loaded.len()` else "no such slot", then `size > deck.slots` → "too big for this deck: needs {size} of {slots} slots". Build: clone `loaded`; if `at < len` remove index `at` into `displaced`; insert `id` at `at`; while the total size is over `deck.slots`, remove the program right after `id` if there is one, else the one right before it (then `at -= 1`), pushing each to `displaced`. `slot_at` applies `plan.loaded` and words the message: no displaced → the same text `load` returns; else "`{name}` slotted · {list} to inventory" with each displaced name in backticks joined by ", ". Wire `Action::Slot` in `apply` like `Load`.
- [ ] **Step 4: Run** `cargo test -p game` — all pass.
- [ ] **Step 5: Commit** `feat(game): slot a program at a position, sending back what it covers`

### Task 3: The deck's geometry and pointing

**Files:**
- Create: `crates/strata-zero/src/ui/cyberdeck/mod.rs`
- Create: `crates/strata-zero/src/ui/cyberdeck/pawn.rs` (layout only in this task: `W`, `H`, `parts`)
- Modify: `crates/strata-zero/src/ui/mod.rs` (`pub mod cyberdeck;`)
- Test: both new files' `#[cfg(test)]` modules

**Interfaces:**
- Consumes: `game::DeckLook`.
- Produces (all `pub` in `ui::cyberdeck`):
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Part { Slot(usize), Plate, Rom(usize), Failsafe, Inv(usize) }` — `Slot(k)` is bay stop *k*: loaded program *k*, or the first empty slot at `k == bay.len()`. `Part::pack(self) -> usize` / `Part::unpack(usize) -> Part`: Slot k → k, Plate → 100, Rom k → 200 + k, Failsafe → 300, Inv k → 1000 + k.
  - `pub enum Dir { Up, Down, Left, Right }`, `pub enum Switch { On, Lifted, Off }`, `pub enum GlassTone { Head, Body, Dim, Warn }` (all `Clone, Copy, Debug, PartialEq, Eq`).
  - `#[derive(Clone, Debug, PartialEq)] pub struct Cart { pub name: String, pub rating: u8, pub size: u8 }`.
  - `#[derive(Clone, Debug, PartialEq)] pub struct Held { pub from: usize, pub cart: Cart, pub start: u8, pub letter: char }` — `from` = its inventory index, `start` = the physical slot (0-based) it lands at once packed.
  - `#[derive(Clone, Debug, PartialEq)] pub struct DeckView { pub look: Option<DeckLook>, pub slots: u8, pub rating: u8, pub bay: Vec<Cart>, pub roms: Vec<String>, pub inventory: Vec<Cart>, pub inv_offset: usize, pub point: Part, pub held: Option<Held>, pub switch: Switch, pub heat: Option<u8>, pub link: bool, pub handle: String, pub glass: Vec<(String, GlassTone)>, pub pinned: (String, GlassTone), pub more_above: bool, pub more_below: bool, pub typed: usize, pub tick: u64 }` — `look: None` = no deck; `glass` = the rows to show (already wrapped and scrolled, at most `GLASS_ROWS`); `typed` = glass characters typed so far (`usize::MAX` = all).
  - `pub const GLASS_TEXT_W: usize = 38;` `pub const GLASS_ROWS: usize = 5;`
  - `pub fn size(look: Option<DeckLook>) -> (u16, u16)` — Pawn `(84, 22)`; None `(84, 12)` (glass + inventory).
  - `pub fn parts(view: &DeckView) -> Vec<(Part, Rect)>` — rects relative to the drawing's origin; dispatches to `pawn::parts`; None → only `Inv` parts.
  - `pub fn step(parts: &[(Part, Rect)], from: Part, dir: Dir, allow: impl Fn(Part) -> bool) -> Part`.
  - `pawn::W = 84`, `pawn::H = 22`, `pawn::parts(view: &DeckView) -> Vec<(Part, Rect)>`.

- [ ] **Step 1: Write the failing tests.**
  - `pawn.rs` `parts_sit_where_the_spec_draws_them` with the kit view (bay `CRACK r1 s1`, `COPY r1 s1`; 2 slots; roms `["NETSCAN", "ARC TRACE"]`; inventory `ATTACK r1 s1`): `Slot(0)` = `Rect::new(8, 1, 12, 3)`, `Slot(1)` = `(22, 1, 12, 3)`, no `Slot(2)` (bay full), `Plate` = `(4, 5, 15, 1)`, `Rom(0)` = `(9, 14, 9, 1)`, `Rom(1)` = `(19, 14, 11, 1)`, `Failsafe` = `(69, 7, 10, 5)`, `Inv(0)` = `(2, 19, 12, 2)`. With bay `[BACKDOOR r1 s2]` on 2 slots: `Slot(0)` = `(8, 1, 26, 3)`, no `Slot(1)`. With bay `[CRACK]`: `Slot(1)` = `(22, 1, 12, 3)` (the first empty slot). With roms empty: no `Rom` parts. With `slots: 4` and bay `[CRACK]`: `Slot(1)` is the only empty stop.
  - `pawn.rs` `inventory_carts_lie_two_apart`: inventory `SPOOF s1, BACKDOOR s2, MASK s1` → `Inv(0)` x 2 w 12, `Inv(1)` x 16 w 18, `Inv(2)` x 36 w 12; with `inv_offset: 1`, `Inv(0)` is absent and `Inv(1)` starts at x 2 (the row scrolls; a `‹` cell is reserved by the drawing in Task 4 at x 0).
  - `mod.rs` `pointing_walks_the_pawn_layout` (kit view, 4 inventory carts `SPOOF, BLIND, MASK, BACKDOOR` for the last two asserts): `step(Slot(0), Right) == Slot(1)`; `step(Slot(0), Down) == Plate`; `step(Plate, Down) == Rom(0)`; `step(Rom(0), Down) == Inv(0)`; `step(Rom(1), Right) == Failsafe`; `step(Slot(1), Right) == Failsafe`; `step(Slot(0), Left) == Slot(0)`; `step(Slot(0), Up) == Slot(0)`; `step(Inv(0), Up) == Rom(0)`; `step(Failsafe, Down) == Inv(3)`; with `allow = |p| matches!(p, Part::Slot(_))`, `step(Slot(0), Down) == Slot(0)`.
  - `mod.rs` `parts_pack_and_unpack`: every variant round-trips.
- [ ] **Step 2: Run** `cargo test -p strata-zero cyberdeck` — fails to compile.
- [ ] **Step 3: Implement.** `pawn::parts` from the spec's rows: carts at `x = 8 + 14 * start_slot`, `y = 1`, `w = 14 * size - 2`, `h = 3`; plate `(4, 5, 15, 1)`; ROM strips from x 9, each `name.len() + 2` wide, one apart, row 14; failsafe `(69, 7, 10, 5)`; inventory row 19, carts 12 / 18 wide, 2 apart from x 2, starting at `inv_offset`, omitted once a cart would end past x 81. `step` — centres in square units (`cx = x + w/2`, `cy = 2 * (y + h/2)`, as floats); a candidate lies in `dir` when its primary offset is > 0; among `allow`ed candidates inside the 45° cone (`perp <= primary`) take the least `primary + perp`; if the cone is empty, the least `primary + 2 * perp` in the half-plane; none → `from`; ties → earlier in `parts` order.
- [ ] **Step 4: Run** `cargo test -p strata-zero cyberdeck` — pass.
- [ ] **Step 5: Commit** `feat(app): the deck's parts and pointing between them`

### Task 4: Drawing the pawn deck

**Files:**
- Modify: `crates/strata-zero/src/ui/cyberdeck/pawn.rs` (drawing)
- Modify: `crates/strata-zero/src/ui/cyberdeck/mod.rs` (`draw`, `brackets`, the no-deck drawing)
- Test: both files' test modules

**Interfaces:**
- Consumes: Task 3's types and `parts`.
- Produces: `pub fn draw(view: &DeckView, area: Rect, buf: &mut Buffer)` (centres `size(view.look)` in `area`, draws the look or the no-deck glass + inventory, then the brackets on `view.point`'s rect); `pub fn brackets(buf: &mut Buffer, r: Rect, tick: u64)` (cyan `┏ ┓ ┗ ┛` one cell outside `r`, brightness `0.7 + 0.3 * pulse(tick, 30)` as the selected rows do); `pawn::draw(view: &DeckView, ox: u16, oy: u16, buf: &mut Buffer)`.

- [ ] **Step 1: Write the failing tests** (render into a `Buffer::empty(Rect::new(0, 0, 98, 22))` with the kit view pointing at `Slot(0)`, glass `[("SLOT A ▸ CRACK", Head), ("rating 1 · size 1 · noise 4 · work 3", Dim)]`, pinned `("[space] eject", Body)`, handle `VEX`, heat `Some(60)`, switch `On`; read rows as strings):
  - `the_pawn_deck_draws_its_parts`: text contains `K▒ROK▒WA KX-2`, `2 SLOT · R1`, `CRACK`, `COPY`, `r1 · s1`, `FAILSAFE`, `ON`, `NETSCAN`, `ARC TRACE`, `INVENTORY`, `ATTACK`, `HEAT`, `NO FUTURE`, `x‿x`, `VEX`, `SLOT A ▸ CRACK`, `[space] eject`; the four bracket corners sit at `(ox+7, oy+0)`, `(ox+20, oy+0)`, `(ox+7, oy+4)`, `(ox+20, oy+4)` with fg cyan-ish (r == 0).
  - `heat_lights_a_cell_per_seventh_begun`: heat 60 → 5 `▮` and 2 `▯` on row 8; `None` → 0 `▮`, 7 `▯`; 100 → 7 `▮`.
  - `the_failsafe_draws_three_ways`: `On` → the well cells' bg is cover `#5a0c14` and `ON` is drawn; `Lifted` → row 6 holds 8 `▄` in red over x 70..78 and the well bg is not the cover colour; `Off` → `OFF` drawn and its fg red.
  - `link_lights_when_jacked`: `link: true` → the `●` before `LINK` is cyan; false → not.
  - `a_long_handle_moves_under_the_sticker`: handle `D'SHAWN` → `D'SHAWN` on row 12 from x 50, and no cell of the failsafe frame `(69..79, 7..12)` changed compared with the `VEX` render; handle of 20 letters is cut before x 68.
  - `odd_decks_draw`: roms empty → row 14 holds `ROM▸` and no strip; bay `[BACKDOOR s2]` → one 26-wide cart; slots 4, bay `[CRACK]` → `A`, `B`, `C`, `D` in the mouth and `empty` once.
  - `holding_draws_the_outline_where_it_lands`: held `{ from: 0, cart: SPOOF s1, start: 1, letter: 'B' }`, point `Slot(1)` → `SPOOF ▸ B` on row 2 within the dashed outline (`┌┄`, `└┄`), and the inventory cart at `Inv(0)` drawn as a dim dashed outline.
  - `inventory_overflow_shows_arrows`: 8 size-1 carts, `inv_offset: 2` → `‹` at x 0 of row 19 and `›` at x 83 (when carts remain to the right).
  - `no_deck_draws_the_glass_and_inventory`: `look: None`, glass `[("no deck jacked", Dim)]` → text contains `no deck jacked` and `INVENTORY`, not `FAILSAFE`.
  - `draw_centres_in_a_bigger_body`: area 120×30 → the drawing's top-left at `((120-84)/2, (30-22)/2)`; area 98×22 → at `(7, 0)`; nothing written outside `area`.
  - `glass_scroll_markers`: `more_below: true` → `▼` at the glass's right edge on its last text row; `more_above: true` → `▲` on its first.
  - `typed_cuts_the_glass`: `typed: 4` → the glass shows `SLOT` and not `SLOT A`.
- [ ] **Step 2: Run** `cargo test -p strata-zero cyberdeck` — the new tests fail.
- [ ] **Step 3: Implement** the drawing to the spec's rows and the Global Constraints' colours (glass: bezel at `(3, 6, 42, 8)` heavy box, text from `(5, 7)`, 6 rows: `glass` rows then blank rows then `pinned` on row 12; scanlines darken alternate rows; tones: Head bold amber, Body amber, Dim dim amber, Warn `#ff4050` bold). Use `ui::chrome::put` with explicit bg so the panel bg never leaks. The no-deck drawing: the glass (same bezel and size) at the top of its 84×12 box, INVENTORY at rows 9-11. Keep every drawn glyph single width.
- [ ] **Step 4: Run** `cargo test -p strata-zero cyberdeck` — pass.
- [ ] **Step 5: Commit** `feat(app): the pawn deck, drawn`

### Task 5: The tab's state, glass and keys

**Files:**
- Create: `crates/strata-zero/src/screens/cyberdeck.rs`
- Modify: `crates/strata-zero/src/screens/mod.rs` (`pub mod cyberdeck;`)
- Modify: `crates/strata-zero/src/screens/deck.rs` — make `program_card` `pub(crate)`; extract the `Detail::Ability(i)` arm into `pub(crate) fn ability_card(st: &AppState, i: usize) -> Vec<Line<'static>>` and call it from the arm; move the failsafe card text into `pub(crate) const FAILSAFE_TEXT: &str` and `pub(crate) const FAILSAFE_WARN: &str` used by the arm.
- Test: `screens/cyberdeck.rs` tests

**Interfaces:**
- Consumes: Task 2 (`plan_slot`, `Action::Slot`), Task 3/4 (`DeckView`, `Part`, `Dir`, `Switch`, `GlassTone`, `Cart`, `Held`, `parts`, `step`, `GLASS_TEXT_W`, `GLASS_ROWS`), `crate::actions::{perform, report}`, `ui::sidebar::wrap_text`, `Deck::{program_card, item_card, ability_card}`.
- Produces:
  - `pub struct DeckTab { pub point: Part, pub held: Option<ItemId>, pub cover_up: bool, pub scroll: usize, pub changed: u64 }`
  - `DeckTab::new(tick: u64) -> Self` (point `Slot(0)`), `DeckTab::restore(packed: usize, st: &AppState) -> Self` (unpack, then clamp), `DeckTab::pack(&self) -> usize`.
  - `DeckTab::key(&mut self, st: &mut AppState, code: KeyCode) -> Option<ScreenCmd>` — `Some(ScreenCmd::Stay)` when the tab used the key, `Some(Pop { fx: Glitch })` for esc/q with nothing to undo, `None` for keys the deck handles (tab, backtab, digits).
  - `DeckTab::view(&self, st: &AppState) -> DeckView`, `DeckTab::hint(&self) -> &'static str`.
  - `pub(crate) fn inventory(st: &AppState) -> Vec<ItemId>` — programs in the pack in pack order, one per id; with a known deck, those not loaded; with none, all.
  - `pub(crate) fn heat(st: &AppState) -> Option<u8>` — jacked in `Place::Construct(b)` → `Some(trace heat or 0)`; jacked elsewhere → `st.net.nearest_trace(city, pos).map(|(_, h)| h)`; not jacked → `None` (mirror `screens/explore.rs:318-362`).
  - `pub(crate) fn glass(st: &AppState, tab: &DeckTab) -> (Vec<(String, GlassTone)>, (String, GlassTone))` — the full, unwrapped body lines and the pinned row.

- [ ] **Step 1: Write the failing tests** (`state()` as in `deck.rs` tests: seed 6 city, Vex's kit: loaded Crack, Copy; Attack in the pack):
  - `glass_reads_the_pointed_program`: point `Slot(0)` → head `SLOT A ▸ CRACK`, next line `rating 1 · size 1 · noise 4 · work 3`, then the description, and somewhere below `crack · any node or ICE` and `your power 19 vs 13 mid-city`; pinned `[space] eject`.
  - `glass_reads_the_failsafe_three_ways`: point `Failsafe` → head `FAILSAFE ▸ ON`, body contains "pulls you out at a quarter of your hp", pinned `[space] lift the cover`; `cover_up` → head `FAILSAFE ▸ ON · cover up`, body "Throw it and black ICE can kill you.", pinned `[space] throw · esc shut`; failsafe off → head `FAILSAFE ▸ OFF` with tone `Warn`, pinned `[space] throw it back`.
  - `glass_reads_rom_and_plate`: `Rom(0)` → head `ROM ▸ NETSCAN`, body contains `within 12 tiles` and `cooldown 10s`, pinned `burned in · can't be pulled`; with talents `wide_band` 2 and `ice_pick` 2 → `cooldown 8s` and `18 tiles`; `Plate` → head `PAWNSHOP DECK`, body contains the deck's description, pinned `2/2 slots · rating 1`.
  - `glass_for_inventory_says_what_it_needs`: give backdoor; point at its `Inv` → body contains `needs 2 slots · 0 free`, pinned `[space] pick up`; on a 1-slot deck (clone with `slots: 1`) → pinned `too big for this deck` and space warns "too big for this deck: needs 2 of 1 slots" and holds nothing.
  - `space_ejects_and_the_bay_packs_left`: point `Slot(0)`, space → `loaded == [copy]`, strip "Crack unloaded" (as `last()` reads it), `inventory(st)` contains crack; point `Slot(1)` (now the empty stop) → pinned `empty · bring a program from inventory`, space changes nothing.
  - `pick_up_carry_drop_swaps`: point the `Inv` of attack, space → `held == Some(attack)`, point jumps to `Slot(0)` (bay full); right → `Slot(1)`; down stays `Slot(1)` (only bay stops while holding); glass head `ATTACK ⇄ COPY`, body "Attack goes in slot B, Copy back to inventory.", pinned `[space] swap · esc put back`; space → `loaded == [crack, attack]`, `held == None`, strip "Attack slotted · Copy to inventory", point `Slot(1)`.
  - `esc_puts_back_then_closes`: holding → esc → `held == None`, `loaded` unchanged, returns `Some(Stay)`; esc again → `Some(Pop { .. })`.
  - `the_failsafe_takes_two_presses_to_turn_off`: point `Failsafe`, space → `cover_up`, `st.net.failsafe` still true; esc → `cover_up` false, returns `Some(Stay)`; space, space → failsafe false, strip warn "failsafe off · black ICE can kill you", view switch `Off`; space → failsafe true, `cover_up` false, strip "failsafe on"; lifting then pressing left (moving away) shuts the cover.
  - `view_reports_heat_link_and_switch`: not jacked → `heat None`, `link false`; jacked into a construct whose trace holds 60 → `heat Some(60)`, `link true` (build the player as `net.rs` tests do when they jack in).
  - `pgdn_scrolls_the_glass`: point `Rom(1)` (Arc Trace's long text): `view.more_below`; PgDn → `scroll == 1`, `view.more_above`; moving the point resets `scroll` to 0.
  - `inventory_scrolls_with_the_point`: give 8 more programs (copies of the content's programs, distinct ids pushed into `st.content.items`), point the last `Inv` → `view.inv_offset > 0` and that part is in `parts(&view)`.
  - `a_stale_point_clamps`: `DeckTab::restore(Part::Inv(9).pack(), &st)` → `point` is a part of the current view; unequip the deck → `restore(Slot(0).pack())` gives `Inv(0)`.
  - `no_deck_refuses_to_pick_up`: unequip the deck; space on `Inv(0)` warns "no deck jacked", `held == None`.
- [ ] **Step 2: Run** `cargo test -p strata-zero screens::cyberdeck` — fails to compile.
- [ ] **Step 3: Implement.** Glass bodies per the spec's *The glass* and the pinned copy in Global Constraints. `Rom(k)` reads `ability_card` at the index of the deck item's k-th grant within `st.character.abilities(&st.content)`, found as `cyberdeck_rows` finds it today, its first line replaced by the `ROM ▸ NAME` head; `view.roms` are those abilities' names in capitals. Program order: stats line, description, blank, `<op> · <targets>`, power line, `loaded` when loaded (take these from `program_card`'s lines by position, so they stay one source). Holding over a stop: run `plan_slot` to word it (one displaced: "X goes in slot L, Y back to inventory."; several: "Y and Z back to inventory."; none: head `SPOOF ▸ B`, body "Spoof goes in slot B.", pinned `[space] slot · esc put back`); the landing `start` is the summed size of `plan.loaded[..plan.at]`, the letter `'A' + start`. Wrap with `wrap_text(_, GLASS_TEXT_W)`, show `GLASS_ROWS` rows from `scroll`, set `more_above`/`more_below`. `typed` = `((tick - changed) * 3)` chars capped at the total (the deck's reveal speed is `REVEAL_TICKS` 20 for a card; match its feel). Bay stops and `allow` while holding: `|p| matches!(p, Part::Slot(_))`. Moving the point sets `changed = st.tick`, `scroll = 0`, and shuts a lifted cover when leaving `Failsafe` with the failsafe on. Clamp: if `point` is not among `parts(&view)` take `Slot(0)` if present, else `Inv(0)`, else `Plate`. `inv_offset`: the smallest offset that keeps the pointed (or held-from) cart fully inside x ≤ 81.
- [ ] **Step 4: Run** `cargo test -p strata-zero screens::cyberdeck` — pass.
- [ ] **Step 5: Commit** `feat(app): the cyberdeck tab's pointer, glass, slotting and failsafe`

### Task 6: The deck screen draws the deck; docs and shots

**Files:**
- Modify: `crates/strata-zero/src/screens/deck.rs` (struct field, `restore`, `rows`, `handle`, `update`, `draw`, hints; remove `cyberdeck_rows`, `Detail::Program`, `Detail::Failsafe` and their arms; rewrite the CYBERDECK tests)
- Modify: `crates/strata-zero/src/screens/shots.rs` (an ignored shot)
- Modify: `README.md:343-348` (the Deck keys and the CYBERDECK description)
- Modify: `docs/superpowers/specs/2026-10-18-cyberdeck-tab-design.md` (Status line: built)

**Interfaces:**
- Consumes: `screens::cyberdeck::DeckTab` (Task 5), `ui::cyberdeck::draw` (Task 4).
- Produces: nothing new.

- [ ] **Step 1: Rewrite the failing tests** in `deck.rs`, keeping what each checked, through the drawn tab (`draw(&d, &st, 120, 35)` text and `d.cyber` state):
  - `cyberdeck_tab_handles_odd_decks`: `ghost_deck` → draws `no deck jacked`; `bare_deck` (no grants) → draws `ROM▸` and no `NETSCAN`; `quiet_deck` (grants Street sense) → pointing `Rom(0)` the glass reads `ROM ▸ STREET SENSE` and `passive`.
  - `abilities_show_on_the_board_and_the_cyberdeck`: second half points `Rom(0)` and finds `from Pawnshop deck`, `within 12 tiles`, `cooldown 10s` in the draw.
  - `cyberdeck_detail_reads_the_sharpened_ability`: same through `Rom(0)`: `cooldown 8s`, `18 tiles`.
  - `cyberdeck_tab_loads_and_unloads_programs`: the draw shows `2 SLOT · R1`, `CRACK`, `COPY`, `INVENTORY`, `ATTACK`; space on `Slot(1)` → "Copy unloaded"; pick up copy and drop on the empty stop → "Copy loaded · 2/2 slots"; give spoof, pick up, drop on `Slot(0)` → "Spoof slotted · Crack to inventory"; dropping crack from the pack (the `game::apply(.. Action::Drop ..)` part) → `CRACK` gone from the draw; the inventory tab still marks Copy `loaded`.
  - `the_cyberdeck_tab_toggles_the_failsafe`: through the keys: point `Failsafe` (move with arrows from `Slot(0)`: right, right), space, space → off with the warning; space → on.
  - `tabs_are_in_the_new_order_with_two_new_ones` (line ~2338): keep the tab order checks; replace the `PAWNSHOP DECK` text check with `K▒ROK▒WA KX-2`.
  - New `cyberdeck_tab_owns_the_arrows`: on CYBERDECK, `Left` from `Slot(0)` keeps `d.tab == Tab::Cyberdeck`; `Tab` moves to QUESTS; `6` comes back; the hint line reads `↑↓←→ point · space act · tab tabs · esc close`, and while holding `↑↓←→ carry · space drop · esc put back`.
  - New `cyberdeck_fits_the_minimum_terminal`: at 100×30 the draw has the frame intact (rows 1..29 start `│` and end `│`) and contains `INVENTORY` and `FAILSAFE`; at 200×60 the drawing is centred (the `K▒ROK▒WA` row's first `╭` at column `(198-84)/2 + 1 + 1`, i.e. the body's x plus the centring offset plus the slab's x 1) and nothing but the chrome is drawn outside the body.
  - New `cyberdeck_remembers_its_part`: point `Failsafe`, close, `Deck::new(5).restore(&st)` on CYBERDECK has `cyber.point == Part::Failsafe`.
- [ ] **Step 2: Run** `cargo test -p strata-zero deck::` — the rewritten tests fail.
- [ ] **Step 3: Implement.** `Deck` gets `pub(crate) cyber: DeckTab` (new in `Deck::new`; `restore` sets it from `st.deck_rows[Tab::Cyberdeck.index()]`). `rows()` returns empty for `Tab::Cyberdeck` (join the MAP/TALENTS arm). In `handle`, before the shared match: on CYBERDECK, `if let Some(cmd) = self.cyber.key(st, k.code) { match cmd { ScreenCmd::Stay => continue, other => return other } }`. In `update`, store `self.cyber.pack()` for CYBERDECK instead of `self.row`. In `draw`, CYBERDECK draws `ui::cyberdeck::draw(&self.cyber.view(st), body, buf)` like MAP/TALENTS (no list, no divider); its hint is `self.cyber.hint()`. Delete `cyberdeck_rows`, `Detail::Program`, `Detail::Failsafe` and their `detail()`/`handle()` arms once nothing uses them; keep `program_card`, `ability_card`, the failsafe consts. Add `the_cyberdeck_tab` to `shots.rs`: `state_home()`, a `Deck` on CYBERDECK, written at 100×30 and 120×40 (extend `shot` with a size parameter or add a sized variant). README: the CYBERDECK line becomes "cyberdeck (the jacked deck drawn as hardware: arrows point at a slot, a ROM strip, the failsafe or a program in inventory and the deck's glass reads it out; `space` picks a program up and drops it in a slot, ejects one, or lifts the failsafe's cover and throws it; `tab` switches tabs here)". Spec status: "built on `worktree-cyberdeck`".
- [ ] **Step 4: Run** `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` — all pass, clippy clean. Run `SHOT_DIR=<scratch dir> cargo test -p strata-zero shots:: -- --ignored` and look at the SVGs.
- [ ] **Step 5: Commit** `feat(app): the CYBERDECK tab is the deck now`

### Task 7: A five-slot pawn deck for D'SHAWN (added at Jeffrey's request after Task 6)

Jeffrey: "create another cyberdeck, same chrome, with 5 slots. give that by default to d'shawn dev character and have it equipped".

**Files:**
- Modify: `crates/game/src/model.rs` (`DeckLook::max_slots`: Pawn 4 → 5) and the validation wording in `crates/game/src/content.rs` ("a pawn deck has room for 5 slots"), with Task 1's test moved to 6 rejected / 5 accepted
- Modify: `flavor/content.ron` (new item after `pawnshop_deck`)
- Modify: `crates/game/src/character.rs` (`debug_runner`)
- Modify: `crates/strata-zero/src/ui/cyberdeck/pawn.rs` (the bay mouth and its letters span every slot, A-E)
- Modify: `crates/strata-zero/src/screens/shots.rs` (a D'SHAWN shot)
- Modify: `docs/superpowers/specs/2026-10-18-cyberdeck-tab-design.md` ("up to 4 slots (A-D)" → "up to 5 slots (A-E)"; the validation line; a line naming the rebuilt deck)

**Interfaces:**
- Consumes: Tasks 1-6 as built.
- Produces: content item `rebuilt_deck`.

- [ ] **Step 1: Write the failing tests.**
  - `content.rs`: `pawn_decks_have_room_for_five_slots_and_three_grants` (rename of Task 1's test): 6 slots → `DeckInvalid`; 5 slots with 3 grants → `Ok`; 4 grants → `DeckInvalid`.
  - `character.rs`: `debug_runner_jacks_the_rebuilt_deck`: `Character::debug_runner(&c)` has `equipment[Slot::Deck] == rebuilt_deck`, `count(pawnshop_deck) == 1` (in the pack), `loaded == [crack, copy, attack, backdoor]`, `loaded_size == 5`, and `deck(c).unwrap().1 == DeckSpec { slots: 5, rating: 3, look: Pawn }`.
  - `pawn.rs`: `five_slots_fill_the_back_edge`: slots 5 with five size-1 carts → `Slot(4)` = `Rect::new(64, 1, 12, 3)`, row 4 shows `A`, `B`, `C`, `D`, `E` in the mouth at x 8, 22, 36, 50, 64 (one right of each slot's `x - 1` label start, as A and B are drawn today), the mouth's `▀` run covers x 7..76, and nothing in rows 1-4 is drawn past x 77; with one cart, `empty` four times.
- [ ] **Step 2: Run** `cargo test -p game` and `cargo test -p strata-zero cyberdeck` — the new tests fail.
- [ ] **Step 3: Implement.** Content: `Item(id: "rebuilt_deck", name: "Rebuilt pawn deck", kind: Deck, slot: Some(Deck), weight: 1.8, value: 6000, perks: [Stat(Net, 2)], grants: ["netscan", "arc_trace"], deck: Some(DeckSpec(slots: 5, rating: 3, look: Pawn)), desc: "The pawnshop shell, gutted and rebuilt by someone who knew what they were doing. Five slots, and it still runs hot.")`. `debug_runner`: put `rebuilt_deck` in the deck slot, give the replaced `pawnshop_deck` back to the pack, then load every program in the pack that fits, in pack order, as `from_template` does (after the `backdoor` give). The pawn bay mouth and letters run over `view.slots` slots. Shots: an ignored `the_cyberdeck_tab_for_dshawn` writing `cyberdeck_dshawn_120x40.svg` with `Character::debug_runner`.
- [ ] **Step 4: Run** `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`; render the shots and look at `cyberdeck_dshawn_120x40.svg`.
- [ ] **Step 5: Commit** `feat(app): a rebuilt five-slot pawn deck, jacked on D'SHAWN`

**Task 7 also (Jeffrey, same evening):** the row under the deck is labelled `WAREZ`, not `INVENTORY` (the label only; the glass and strip copy keep "back to inventory" / "to inventory", where the program really goes). Change the label in `pawn.rs` and every test that reads it (`ui/cyberdeck`, `screens/cyberdeck.rs`, `screens/deck.rs`: assertions on `INVENTORY` in the CYBERDECK draw become `WAREZ`; the INVENTORY *tab* label stays). README's CYBERDECK line says "a program in your warez" if it names the row.

### Task 8: The deck is locked while jacked in (added at Jeffrey's request after Task 7)

Jeffrey: "lock up the deck when we are in the net. We should not be able to switch in/out programs while we are in net space. interacting with the deck is meatspace only". Spec: section "Jacked in: the deck is locked".

**Files:**
- Modify: `crates/strata-zero/src/screens/cyberdeck.rs` (key handling, the glass's pinned row)
- Modify: `crates/strata-zero/src/screens/deck.rs` (one draw test)
- Modify: `README.md` (the CYBERDECK line)

**Interfaces:**
- Consumes: Tasks 1-7 as built; the jacked-in test the view already uses for `link` (`st.player` is jacked in, `walker::Player::jacked_in`).
- Produces: nothing new.

- [ ] **Step 1: Write the failing tests** in `screens/cyberdeck.rs` (jack the player in as `view_reports_heat_link_and_switch` does):
  - `jacked_in_the_deck_is_locked`: point `Slot(0)` → pinned `locked · jacked in` (tone `Warn`), the glass head and body unchanged from jacked out; space → `loaded` unchanged, strip warn "the deck is locked while you're jacked in" (`last_is_warning`); enter does the same; point a WAREZ cart (give spoof), space → `held == None`; point `Failsafe`, space → `cover_up == false` and `st.net.failsafe` unchanged; `Right` from `Slot(0)` still reaches `Slot(1)`; PgDn on `Rom(1)` still scrolls; esc returns `Some(Pop { .. })`.
  - `jacking_out_unlocks_the_deck`: jacked in, then jacked out (`body = None` as the net tests do): space on `Slot(0)` ejects as before and the pinned row is `[space] eject` again.
  - `screens/deck.rs` `cyberdeck_locks_in_the_net`: jacked in, the CYBERDECK draw (after the glass has typed, `st.tick` ≥ 20 past the change) contains `locked · jacked in`.
- [ ] **Step 2: Run** `cargo test -p strata-zero cyberdeck` — the new tests fail.
- [ ] **Step 3: Implement** a private `fn locked(st: &AppState) -> bool` (the same test as `link`); in `key()`, before acting: space/enter while locked → `st.warn("the deck is locked while you're jacked in")`, `Some(Stay)`, nothing else; pointing, PgUp/PgDn, esc/q and the tab keys are untouched. The glass's pinned row is `("locked · jacked in", GlassTone::Warn)` for every part while locked. README's CYBERDECK line gains "; the deck is locked while you're jacked in".
- [ ] **Step 4: Run** `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] **Step 5: Commit** `feat(app): the deck is locked while you're jacked in`
