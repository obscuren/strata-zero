# GEAR & INVENTORY Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The deck's GEAR and INVENTORY tabs become one tab, GEAR & INVENTORY: the
rack of worn slots (A), the pack with an icon filter (B), and the selected item's
card beside its drawing at half size (C).

**Architecture:** A generic half-size preview of any `Art` (draw full size
offscreen, read back as half-cell pixels, shrink 2×2 by commonest colour, fold with
the existing `Pixels` canvas). The blueprint window's choice of drawing moves into one
function the preview shares. The deck's list, popups and cards come out of `Deck`'s
methods as free functions. A new view, `screens/kit.rs`, owns the tab's own state,
keys and drawing, as TALENTS owns its own. Then `Deck` delegates to it and the old
tabs go.

**Tech Stack:** Rust 2024, ratatui (TestBackend in tests), crossterm keys.

**Spec:** `docs/superpowers/specs/2026-10-19-gear-inventory-design.md`. Layout
reference: `docs/superpowers/specs/2026-10-19-gear-inventory-mock/` (`gen.py`, run as
its docstring says). It is the reference for every cell, colour and glyph.

## Global Constraints

- Everything fits 100×30. Every glyph is single width. Every colour is `Color::Rgb`
  (or `Reset`).
- Nothing in `screens/kit.rs` or `ui/preview.rs` names an item, a slot or a kind by
  hand. Slots come from `Slot::ALL`, kinds from `ItemKind::ALL` / `kind_icon` /
  `ItemKind::label`, and drawings come through `Art`.
- Colours: `CYAN`, `DIM`, `TEXT`, `RULE`, `BAR_BG`, `PANEL_BG` from `ui/chrome.rs`;
  the cursor's lit colour is `deck::lit(tick)`; the new `FAINTER` = `#5c6673` (Task 6).
- Comments and doc comments read like the surrounding code: plain sentences, no task
  numbers.
- Run tests with `cargo test -q` at the workspace root. `cargo clippy --workspace
  --all-targets -- -D warnings` and `cargo fmt --check` stay clean after every task
  (they are clean at a6f20df; 890 tests pass there).
- Never build `target/release/strata`. Shots and release builds use
  `--target-dir target/smoke`.
- Commit after each task with a conventional message ending in:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  ```

## Review Focus

- **The pack empties completely** (everything dropped, or a fresh character with
  nothing). Expect: the list is empty, C reads `nothing here` with an empty viewer,
  space does nothing, and no key panics. Tested in Task 5
  (`an_empty_pack_selects_nothing`).
- **The last item of the filtered kind leaves** (dropped, equipped or fitted while
  filtered to it). Expect: the filter stays, the list is empty, and `]` steps on.
  Tested in Task 5 (`a_filter_left_empty_stays_until_stepped`).
- **Equipping from a pack row removes that row.** Expect: the cursor lands on a
  selectable row and C follows it. Tested in Task 5
  (`a_row_that_leaves_moves_the_cursor_to_a_row`).
- **Odd widths and the minimum height.** Expect: tiles split evenly, nothing is
  written outside the body, and the longest chip row fits. Tested in Task 6
  (`nothing_spills_outside_the_body`).
- **A drawing larger than the viewer.** Expect: it is clipped to the viewer and
  nothing is written outside it. Tested in Task 1
  (`a_drawing_bigger_than_its_room_folds_clipped`).

---

### Task 1: Half-size previews of any drawing

**Files:**
- Create: `crates/strata-zero/src/ui/preview.rs`
- Modify: `crates/strata-zero/src/ui/mod.rs` (add `pub mod preview;`),
  `crates/strata-zero/src/ui/art.rs` (trait `Art`),
  `crates/strata-zero/src/ui/pixels.rs` (`Pixels`)

**Interfaces:**
- Produces:
  - `Art::extent(&self) -> (i32, i32, u16, u16)`: x, y, width, height from the
    origin. The default is `(0, 0, w, h)` from `size()`. Doc: most drawings stay
    inside their size, and one that reaches past it says so here.
  - `Pixels::cells(&self) -> (u16, u16)`: width and rows in cells (`h / 2`).
  - `pub(crate) fn half(art: &dyn Art) -> Pixels` in `ui/preview.rs`.

- [ ] **Step 1: Write the failing tests** in `ui/preview.rs` `mod tests`.
  - A test-only `struct Cells { size: (u16, u16), extent: (i32, i32, u16, u16),
    cells: Vec<(i32, i32, char, Color, Color)> }` implements `Art`. Its `draw` writes
    each cell at `origin + (x, y)` with fg and bg, through `Pen::cell` so the clip
    holds. It has no sockets.
  - Colours: RED `#ff0000`, BLUE `#0000ff`, GREEN `#00ff00`, YELLOW `#ffff00`,
    WHITE `#ffffff`, DARK `#101010`, BRIGHT `#f0f0f0`.
  - `half_keeps_the_commonest_colour_and_drops_thin_blocks`: a 4×2-cell art. Row 0
    has `(0,0,'▀',RED,BLUE)`, `(1,0,' ',RED,RED)`, `(2,0,'X',WHITE,GREEN)` and
    `(3,0,'▄',YELLOW,PANEL_BG)`. Row 1 has `(0,1,'▄',RED,PANEL_BG)` (cell (1,1) left
    unwritten), `(2,1,'▀',DARK,BRIGHT)` and `(3,1,'▀',DARK,BRIGHT)`.
    Assert `half(&art).cells() == (2, 1)`. Fold it into a 2×1 `Buffer` through a
    `Pen` at (0,0) and check:
    - cell (0,0) is `▀` with fg RED. The block reads RED BLUE RED RED.
    - cell (1,0) is `▀` with fg GREEN and bg BRIGHT. The top-right block is GREEN,
      GREEN (the text cell reads as its bg), empty and YELLOW, so GREEN. The
      bottom-right block ties 2–2 and goes to the brighter colour.
    - The bottom-left block has one coloured pixel, so it is empty. That is why
      cell (0,0) is `▀`, not a full cell.
  - `half_reads_what_reaches_past_the_size`: `size (2,1)`, `extent (-2,0,4,1)`.
    Cells `(-2,0,' ',RED,RED)`, `(-1,0,' ',RED,RED)`, `(0,0,' ',BLUE,BLUE)` and
    `(1,0,' ',BLUE,BLUE)`. Assert `cells() == (2,1)`. Folded, cell (0,0) is `▀` with
    fg RED and cell (1,0) is `▀` with fg BLUE.
  - `odd_sizes_pad_with_empty`: a 3×1 art of `' '` on RED. Assert `cells() == (2,1)`.
    Both cells fold to `▀` with fg RED: the padded block has two coloured pixels of
    four.
  - `a_drawing_bigger_than_its_room_folds_clipped`: a 200×60 art of `' '` on RED,
    halved and folded through a `Pen` whose clip is `Rect::new(10,5,20,6)`, into a
    60×40 buffer. Every cell outside the clip is unchanged (`Cell::default()`), and
    every cell inside is RED.
  - In `ui/art.rs` tests, `extent_defaults_to_the_size`: a `HardwareArt` for
    `HardwareLook::Redeye` has `extent() == (0, 0, 52, 18)`.

- [ ] **Step 2: Run to verify they fail.** `cargo test -q -p strata-zero preview::`
  → FAIL (no `preview` module, no `extent`).

- [ ] **Step 3: Implement.** `extent` is a default method on `Art`. `Pixels::cells`
  is a plain accessor. `half` in `ui/preview.rs`, with this module doc: "An item's
  drawing at half size: drawn full size offscreen, read back as half-cell pixels,
  each two by two block taken to its commonest colour":
  - `(ex, ey, ew, eh) = art.extent()`. Make a `Buffer` of `ew × eh` with every
    cell's bg `PANEL_BG`. Draw with `art.draw(((-ex) as u16, (-ey) as u16), area,
    &mut buf)`.
  - Read each cell as (top, bottom):
    - `▀` → (fg, bg)
    - `▄` → (bg, fg)
    - `█` → (fg, fg)
    - `' '` → (bg, bg)
    - any other glyph → (bg, bg)

    A pixel that is `PANEL_BG` or `Color::Reset` is empty.
  - Pad the width to even. Each 2×2 block is empty if fewer than two of its pixels
    have a colour. Otherwise it takes the colour seen most often. Ties go to the
    larger `r+g+b`, and a remaining tie to the first seen in row-major order.
  - Return `Pixels::new(ceil(ew/2), ceil(eh/2))`, filled with the shrunk pixels.
    The shrunk height is `eh` pixels; pad it to even with empty pixels.

- [ ] **Step 4: Run.** `cargo test -q && cargo clippy --workspace --all-targets --
  -D warnings && cargo fmt --check` → all pass.

- [ ] **Step 5: Commit.** `feat(app): any drawing at half size, for previews`

### Task 2: The drawing a preview shows

**Files:**
- Modify: `crates/strata-zero/src/ui/cyberdeck/mod.rs`,
  `crates/strata-zero/src/screens/cyberdeck.rs` (`DeckArt`),
  `crates/strata-zero/src/screens/blueprint.rs` (`Blueprint::open`, `Inspect`)

**Interfaces:**
- Consumes: `Art::extent`, `half` (Task 1).
- Produces:
  - `pub fn extent(view: &DeckView) -> (i32, i32, u16, u16)` in `ui/cyberdeck/mod.rs`.
    For the pawn look, a view with sockets returns `(-4, 0, pawn::W + 8, pawn::H)`:
    the cradles at x −4..0 and 83..87. Without sockets it returns
    `(0, 0, pawn::W, pawn::H)`. `DeckArt::extent` returns this.
  - `pub fn unplugged(self) -> DeckArt` on `DeckArt`: `view.plugged` and
    `view.link` set false, so neither cable is drawn.
  - `pub fn preview_art(st: &AppState, item: &ItemId, equipped: bool, copy:
    Option<CopyId>) -> Option<Box<dyn Art>>` in `screens/blueprint.rs`: the
    drawing the window would show for the same arguments, with the deck unplugged.
    `None` when the item has no drawing. Pull the choice in `Blueprint::open` into
    `fn inspect_for(st, &item, equipped, copy) -> Inspect` and
    `fn host_for(st, &item, equipped, copy) -> Option<Host>`, used by both.

- [ ] **Step 1: Write the failing tests** in `screens/blueprint.rs` tests. Use the
  existing `state()`, with `st.character = Character::debug_runner(&st.content)`
  and the `pack_copy` helper.
  - `the_preview_draws_what_the_window_draws`:
    - Fit `hushcan` (socket 0) and `redeye` (socket 1) into `Host::Worn(Slot::Primary)`
      with `Action::Fit`.
    - `preview_art(&st, &ratchet_smg, true, worn copy)` has `extent()
      == (0,0,88,15)` and `half(..).cells() == (44, 8)`.
    - For the `coldfin_1` copy in the pack (`equipped false`): extent
      `(0,0,52,18)` and cells `(26, 9)`.
    - For `pawnshop_deck` (no sockets, `equipped false`, no copy): extent
      `(0,0,84,22)` and cells `(42, 11)`.
    - For `scrap_servo`: `None`.
  - `the_decks_preview_takes_its_cradles_and_not_its_cables`:
    - For the worn `rebuilt_deck` with its worn copy, `preview_art` has extent
      `(-4, 0, 92, 22)` and cells `(46, 11)`.
    - Draw it into a 140×30 buffer at origin (30,4) with the clip set to the whole
      buffer. Every cell with x < 26 or x > 117 is still `Cell::default()`.
    - Drawn the same way, `DeckTab::for_deck(st.tick, rebuilt, true, copy,
      &st).art(&st)` writes at least one cell with x < 26 (the player's cable). This
      proves the test can fail.

- [ ] **Step 2: Run to verify they fail.** `cargo test -q -p strata-zero blueprint::`
  → FAIL (no `preview_art`).

- [ ] **Step 3: Implement** `extent`, `DeckArt::extent`, `unplugged`, `inspect_for`,
  `host_for` and `preview_art`, as the Interfaces say. `Blueprint::open` keeps its
  behaviour: every existing blueprint test passes unchanged.

- [ ] **Step 4: Run.** `cargo test -q && cargo clippy --workspace --all-targets --
  -D warnings && cargo fmt --check` → all pass.

- [ ] **Step 5: Commit.** `feat(app): the preview draws what the blueprint draws, the deck's cradles included`

### Task 3: The card, compacted

**Files:**
- Modify: `crates/strata-zero/src/screens/deck.rs` (`item_card_of`, `perk_lines`,
  tests)

**Interfaces:**
- Produces: `item_card_of` line 2 becomes `"{kind}{place} · {weight:.1} kg · ¤ {value}"`
  (dim), and the separate weight line goes. `perk_lines` puts every granted
  ability on one cyan line, `"grants {a} · {b}"`, after the perk lines. Nothing
  else changes.

- [ ] **Step 1: Write the failing test and update the old assertions.**
  - New `the_card_shares_its_lines`, on `dshawn()` (Ratchet sockets empty):
    - The worn Ratchet's `Deck::item_card_of(&st, &ratchet, worn_copy)` gives
      line 1 `"weapon · primary · 3.2 kg · ¤ 900"`.
    - The worn rebuilt deck's card has exactly one line starting `"grants "`, and
      it equals `"grants Netscan · Arc Trace"`.
    - The `coldfin_1` copy's card gives line 1 `"hardware · deck socket · 0.4 kg · ¤ 800"`.
  - Update existing tests:
    - `cards_list_sockets_and_mods`: `text[1] == "hardware · deck socket · 0.4 kg · ¤ 800"`.
    - `the_gear_row_and_card_show_the_guns_sockets`: replace
      `starts_with("3.7 kg")` with `ends_with("3.7 kg · ¤ 1600")`.
    - `the_hardware_card_names_its_socket_and_stat`:
      `text[1] == "hardware · muzzle socket · 0.5 kg · ¤ 700"`.
    - `deck_tabs_and_rows_render_from_content`: `cyberware · eyes` still matches,
      as a substring.

- [ ] **Step 2: Run to verify.** `cargo test -q -p strata-zero deck::` → the new
  test and the three updated ones FAIL.

- [ ] **Step 3: Implement** the two changes.

- [ ] **Step 4: Run.** The full check passes. The blueprint and cyberdeck tests
  pass unchanged: the plate readout reads the same card.

- [ ] **Step 5: Commit.** `feat(app): the item card shares its meta and grants lines`

### Task 4: The deck's list, popups and cards as free functions

A refactor with no behaviour change, so the new view can use them.

**Files:**
- Modify: `crates/strata-zero/src/screens/deck.rs`

**Interfaces:**
- Produces, all `pub(crate)` in `screens/deck.rs`:
  - `fn cut(name: &str, room: usize) -> String` and `fn lit(tick: u64) -> Color`:
    the existing ones, now visible.
  - `fn item_row(st: &AppState, room: usize, lead: String, name: &str, marks:
    Vec<Span<'static>>, selected: bool, detail: Detail) -> Row`: today's
    `Deck::item_row`, with `room` passed in.
  - `fn slot_popup(st: &mut AppState, slot: Slot, anchor: (u16, u16)) ->
    Option<Popup>`, `fn item_popup(st: &mut AppState, id: &ItemId, anchor: (u16,
    u16)) -> Option<Popup>` and `fn copy_popup(st: &mut AppState, copy: CopyId,
    anchor: (u16, u16)) -> Option<Popup>`: the three arms of `Deck::interact`.
    `Deck::interact` calls them.
  - `fn card(st: &AppState, detail: &Detail) -> Vec<Line<'static>>`: the match in
    `Deck::detail`. `Deck::detail` becomes the row lookup plus `card`.
  - `fn draw_rows(buf: &mut Buffer, list: Rect, rows: &[Row], first: usize, cursor:
    usize, focused: bool, tick: u64)`: the row loop in `Deck::draw`. With `focused`
    true it is exactly today's drawing. With `focused` false the cursor's row gets
    `▸ ` in `DIM`, with no bar and its text in its own tone. SKILLS' board lines go
    into `Row.spans` in `board_list`, so `draw_rows` needs no board.

- [ ] **Step 1: Write the failing test.** `draw_rows_marks_an_unfocused_cursor_dim`
  uses three rows: a header, then two items with `Tone::Normal`. Draw them into a
  30×5 buffer with `list = Rect(0,0,30,3)` and cursor 1.
  - With `focused = true`: cell (1,1) is `▸` and has bg `BAR_BG`.
  - With `focused = false`: cell (1,1) is `▸` with fg `DIM`, and bg isn't `BAR_BG`.

- [ ] **Step 2: Run to verify it fails.** No `draw_rows` → compile FAIL.

- [ ] **Step 3: Move the code** into the functions above, keeping behaviour.

- [ ] **Step 4: Run.** The full check passes with every existing test unchanged.

- [ ] **Step 5: Commit.** `refactor(app): the deck's rows, popups and cards as free functions`

### Task 5: The kit: state, layout, keys, filter, popups

**Files:**
- Create: `crates/strata-zero/src/screens/kit.rs`
- Modify: `crates/strata-zero/src/screens/mod.rs` (`pub mod kit;`)

**Interfaces:**
- Consumes: `item_row`, `cut`, `slot_popup`, `item_popup`, `copy_popup`, `Row`,
  `Detail` and `Tone` (Task 4), and `deck::marks`.
- Produces, in `screens/kit.rs`. Module doc: "GEAR & INVENTORY: the rack of what you
  wear, the pack of what you carry, and the item under the cursor, its card beside
  its drawing at half size."
  - `#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub enum Pane { #[default]
    Rack, Pack }`
  - `#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct KitState { pub
    pane: Pane, pub tile: usize /* index into Slot::ALL */, pub row: usize /* the
    pack's row, headers counted */, pub filter: Option<ItemKind> /* None is ALL */ }`
  - `pub(crate) struct Layout { pub rack: Rect, pub divider: u16, pub pack: Rect, pub
    rule: u16, pub card: Rect, pub view: Rect }` and `pub(crate) fn layout(body: Rect)
    -> Layout`. Consts: `C_ROWS: u16 = 12`, `CARD_W: u16 = 41`, `RACK_ROWS: u16 = 9`.
    - `top = body.height.saturating_sub(C_ROWS + 1)`
    - `a_w = (body.width - 1) / 2`
    - `rack = (body.x, body.y, a_w, top)`
    - `divider = body.x + a_w`
    - `pack = (divider + 1, body.y, body.right() - divider - 1, top)`
    - `rule = body.y + top`
    - `card = (body.x + 2, rule + 1, CARD_W, C_ROWS)`
    - `view = (body.x + 45, rule + 1, body.right() - 1 - (body.x + 45), C_ROWS)`
  - `pub(crate) fn tile_rect(rack: Rect, i: usize) -> Rect`: x is `rack.x + (i % 3)
    * (rack.width / 3)`, y is `rack.y + (i / 3) * 3`, width `rack.width / 3`,
    height 3.
  - `pub struct Kit { pub state: KitState, pub changed: u64, seg_from: Pane, seg:
    Tween, room: Cell<usize>, body: Cell<Rect>, first: Cell<usize> }`. `changed` is
    the tick of the last selection change, for the card's typewriter. `seg` is the
    rule segment's slide, `Tween::new(tick, 6, Easing::OutCubic)` on every pane
    change.
  - `pub fn new(tick: u64) -> Kit`
  - `pub(crate) fn pack_rows(&self, st: &AppState) -> Vec<Row>`: today's
    `inventory_rows`, filtered. Under ALL, every kind with stacks gets its `── kind ──`
    header. Under a kind, only that kind's rows, with no header. A row's marks are
    lit when it is the cursor's row and `pane == Pack`. Names are cut to
    `room.get()`.
  - `pub(crate) fn selected(&self, st: &AppState) -> Option<Detail>`: the rack gives
    `Detail::Slot(Slot::ALL[tile])`. The pack gives that row's detail.
  - `pub(crate) fn key(&mut self, st: &AppState, code: KeyCode, tick: u64) -> bool`:
    true when the key is the kit's. Sets `changed = tick` whenever the selection
    moves or the pane changes.
    - On the rack:
      - `Up`/`k`: −3 when `tile >= 3`.
      - `Down`/`j`: +3 when `tile < 6`.
      - `Left`/`h`: −1 when `tile % 3 > 0`.
      - `Right`/`l`: +1 when `tile % 3 < 2`, else the pane becomes `Pack`.
    - On the pack:
      - `Up`/`Down`/`k`/`j` move over selectable rows and wrap, as `Deck::move_row`
        does.
      - `Left`/`h`: the pane becomes `Rack`, with `tile` unchanged.
      - `Right`/`l` is handled but does nothing.
    - `[` and `]` step the filter back and forward (either pane): through `None`
      then `ItemKind::ALL` in order, wrapping, stopping only at `None` or a kind
      some stack in the pack has. Then `row` goes to the first selectable row.
  - `pub(crate) fn reselect(&mut self, st: &AppState)`: clamps `tile` below 9. Puts
    `row` on a selectable row when it is past the end or on a header: the first
    selectable row, or 0 when there is none.
  - `pub(crate) fn interact(&self, st: &mut AppState) -> Option<Popup>`. Anchors
    come from `layout(self.body.get())`. On the rack: `slot_popup` at
    `(tile.right() + 1, tile.y + 1)`. On the pack: `item_popup` or `copy_popup` at
    `(pack.x + 2, pack.y + 1 + (row - first) + 1)`. `None` with no selection.
  - `pub(crate) fn hints(&self) -> &'static str`:
    - rack: `"←↑↓→ rack, then over to the pack · [ ] filter · space act · tab tabs · esc close"`
    - pack: `"←→ gear/pack · ↑↓ select · [ ] filter · space act · tab tabs · esc close"`

- [ ] **Step 1: Write the failing tests** in `kit.rs` `mod tests`.
  - The state helper: `AppState::new()` with `st.character =
    Character::debug_runner(&st.content)`.
  - `layout_at_the_smallest_and_a_common_terminal`:
    - `layout(Rect::new(1,5,98,22))` gives rack `(1,5,48,9)`, divider 49, pack
      `(50,5,49,9)`, rule 14, card `(3,15,41,12)` and view `(46,15,52,12)`.
    - `layout(Rect::new(1,10,118,27))` gives rack `(1,10,58,14)`, divider 59, pack
      `(60,10,59,14)`, rule 24, card `(3,25,41,12)` and view `(46,25,72,12)`.
    - Odd width: `layout(Rect::new(1,5,99,22))` gives divider 50 and pack
      `(51,5,49,9)`.
    - `tile_rect((1,5,48,9), 8) == (33,11,16,3)` and
      `tile_rect((1,10,58,14), 4) == (20,13,19,3)`.
  - `the_rack_moves_by_tiles_and_crosses_to_the_pack`, starting at tile 0, pane
    Rack:
    - Right → 1. Down → 4. Down → 7. Down → 7 (it stops).
    - Left, Left → 6. Left → 6. Up ×3 → 0. `l`, `l` → 2.
    - Right → pane Pack, tile still 2. Left → pane Rack, tile 2. `j` → 5.
    - Every arrow returns true. `changed` is set on a move and left alone on a stop.
  - `the_pack_wraps_and_left_returns_to_the_tile`, in pane Pack, after `reselect`:
    - `row` is the first selectable row.
    - Up → the last selectable row. Down → the first again.
    - Left → Rack.
  - `the_filter_steps_through_the_kinds_carried`:
    - Clear the pack, then `give_item` one `stim_patch` and one `hushcan`.
    - `]` → `Some(Consumable)`. `]` → `Some(Hardware)`. `]` → `None`.
    - `[` → `Some(Hardware)`.
    - Under `Some(Hardware)`, `pack_rows` texts are `["≡  1× Hushcan"]`.
    - Under `None`, the texts are `["── consumable ──", "◍  1× Stim patch", "── hardware ──",
      "≡  1× Hushcan"]`.
    - Works with the pane on the Rack, which stays Rack.
  - `selected_follows_the_pane`:
    - Tile 6 → `Some(Detail::Slot(Slot::Primary))`.
    - Pane Pack on the row whose text ends `Hushcan` → `Some(Detail::Copy(_))`.
  - `space_on_a_tile_or_a_row_opens_its_popup`:
    - On the rack at tile 6, the popup's labels contain `"Unequip"` and `"Inspect"`.
    - Pane Pack on `Stim patch`: the labels are `["Inspect", "Use", "Drop one"]`.
  - `an_empty_pack_selects_nothing` (Review Focus):
    - Clear the pack, pane Pack.
    - `pack_rows` is empty, `selected` is `None` and `interact` is `None`.
    - Down, Up, `]`, `[` don't panic.
  - `a_filter_left_empty_stays_until_stepped` (Review Focus):
    - Filter `Some(Hardware)`, then remove every hardware stack.
    - `reselect` → filter still `Some(Hardware)` and `pack_rows` empty.
    - `]` → `None`.
  - `a_row_that_leaves_moves_the_cursor_to_a_row` (Review Focus):
    - Pane Pack on the `Pawnshop deck` row.
    - `perform(st, Action::Equip(pawnshop_deck))`, then `reselect`.
    - `pack_rows[row].detail` is `Some` and `selected` is `Some`.

- [ ] **Step 2: Run to verify they fail.** `cargo test -q -p strata-zero kit::` →
  compile FAIL (no `kit`).

- [ ] **Step 3: Implement** `kit.rs` as the Interfaces say.

- [ ] **Step 4: Run.** The full check passes.

- [ ] **Step 5: Commit.** `feat(app): the kit view's state, keys, filter and popups`

### Task 6: The kit drawn

**Files:**
- Modify: `crates/strata-zero/src/screens/kit.rs`, `crates/strata-zero/src/ui/chrome.rs`
  (add `pub(crate) const FAINTER: Color = Color::Rgb(0x5c, 0x66, 0x73);` with a doc
  line: "Darker than FAINT: what is there but empty or out of reach").

**Interfaces:**
- Consumes: `layout`, `tile_rect`, `Kit` (Task 5), `draw_rows`, `card` and `lit`
  (Task 4), `preview_art` (Task 2), `half` and `Pixels::fold`/`cells` (Task 1),
  `chrome::{solid, put, put_paragraph, reveal, dim}`, `kind_icon` and
  `deck::REVEAL_TICKS`.
- Produces: `pub(crate) fn draw(&self, st: &AppState, body: Rect, buf: &mut Buffer)`
  on `Kit`. It stores `body` and `first` for `interact`, and sets `room` to
  `pack.width - 3` before building the pack rows. Let `f` be `pane == Rack`
  for the rack's parts and `pane == Pack` for the pack's.
  - **Rack**: each `Slot::ALL[i]` at `tile_rect(layout.rack, i)`, as `gen.py`'s
    `tile` draws it:
    - **Top edge:** `"┌ " + LABEL + " " + "─"×(w − len − 4) + "┐"`.
    - **Inside:** `│`, the item's name cut to `w − 2` (or `— empty —` in
      `FAINTER`), then `│`.
    - **Bottom edge:** `"└" + "─"×(w − 2) + "┘"`, with `" " + marks + " "` written
      from `x + 2` when the worn item has marks (`deck::marks(st, item, worn_copy,
      true, sel && f)`).
    - **Colours:**
      - Edge: `RULE`, or for the selected tile `CYAN` when `f` and `DIM` when not.
      - Label: `DIM`, or `lit(tick)` and bold when selected and `f`.
      - Name: `TEXT`. Selected with `f`: `lit(tick)` bold on `BAR_BG` across the
        inner width.
  - **Pack**:
    - The chips start at `(pack.x + 1, pack.y)`: ALL is `" ALL "` or `"▌ALL▐"`.
      Each kind is `" {icon} "`, or `"▌{icon} {LABEL}▐"` when active. The active
      chip is `lit(tick)` bold when `f`, else `DIM`. Inactive chips are `DIM`, or
      `FAINTER` for a kind the pack holds none of.
    - The list is `Rect(pack.x, pack.y + 1, pack.width, pack.height − 1)`, drawn
      with `draw_rows(…, cursor = row, focused = f, tick)`. It scrolls as the deck's
      list does: `first = row.saturating_sub((h − 1) / 2).min(len.saturating_sub(h))`.
  - **Divider and rule**:
    - `║` in `RULE` from `body.y` to the rule.
    - `═` in `RULE` across the body at `rule`, with `╩` at the divider.
    - The segment: `━` in `CYAN` over the focused pane's span (rack `body.x..divider`,
      pack `divider + 1..body.right()`). Its two ends move from the old span to the
      new one by `seg.progress(tick)`.
    - Then `" {w:.1} kg · ¤ {v} "` (the pack's weight and value) in `DIM`, ending
      at `pack.right() − 2` (exclusive).
  - **Card**: `card(st, &d)` for `selected()`, else `vec![dim("nothing here")]`.
    Pass it through `reveal(lines, (tick − changed) / REVEAL_TICKS, blink)` and
    `put_paragraph` into `layout.card`.
  - **Viewer**: `solid(view)`. Corners `┌ ┐ └ ┘` go at its four corner cells in
    `RULE`. The item for the selection:
    - `Slot(s)` → the worn item, `equipped = true`, the worn copy.
    - `Item(id)` → `id`, `false`, `None`.
    - `Copy(c)` → the stack's item, `false`, `Some(c)`.

    With `preview_art` `Some(art)`: `px = half(art)`. Fold it through a `Pen` with
    `clip = view` at `(view.x + (view.w − cw)/2, view.y + (view.h − ch)/2)`
    (saturating). With `None`, draw the missing box:
    - A 21×5 box, centred. Its edges are `╌`/`╎` with `┌ ┐ └ ┘` corners, all
      `FAINTER`.
    - The kind's icon in `DIM` bold at `(box.x + 10, box.y + 1)`.
    - `MISSING BLUEPRINT` in `FAINTER` at `(box.x + 2, box.y + 3)`.

    With an item, write `space inspect` in `FAINTER` at
    `(view.right() − 2 − 13, view.bottom() − 1)`. An empty slot gets only the
    corners.

- [ ] **Step 1: Write the failing tests** in `kit.rs`.
  - Setup: D'SHAWN, a 100×30 buffer drawn with `kit.draw(&st, Rect::new(1,5,98,22),
    &mut buf)`, and `st.tick = kit.changed + REVEAL_TICKS + 40` with the segment
    finished.
  - `the_rack_draws_tiles_with_marks_in_their_edges`:
    - Fit `coldfin_2` into the worn deck's socket 0. Tile 8, pane Rack.
    - Row 11 from x 33 reads `"┌ DECK ────────┐"`. Row 12 reads `"│Rebuilt pawn… │"`
      (13 characters of name in a 14-wide row). Row 13 reads `"└─ ■■■■■ · ■□ ─┘"`.
    - fg of (33,11) is `CYAN`. bg of (34,12) is `BAR_BG`.
    - Marks' fg: (36,13) `#1a9aaa`, (42,13) `#00a5b8`, (44,13) `#00e5ff`, (45,13)
      `#00a5b8`.
    - With pane Pack: fg of (33,11) is `DIM`, bg of (34,12) isn't `BAR_BG`, (36,13)
      is `#3f7c87`, and (42,13) and (45,13) are `DIM`.
    - The HEAD tile (empty) has `—` with fg `FAINTER` at (2,6).
  - `the_pack_draws_its_chips_then_its_list`:
    - Pane Pack, filter `None`. Row 5 from x 51 starts `"▌ALL▐ /  ▣  ◈  ⌗ "`.
    - The `/` at (57,5) has fg `FAINTER`. The `⌗` at (66,5) has fg `DIM`.
    - With filter `Some(Hardware)`: row 5 contains `"▌≡ HARDWARE▐"` and starts with
      `" ALL "`. Row 6 contains `"▸ ≡  1× Coldfin vI"` with bg `BAR_BG` at (51,6).
  - `the_rule_carries_the_focus_and_the_packs_weight`:
    - (49,14) is `╩`. Pane Rack: cells 1..=48 of row 14 are `━` in `CYAN`, and
      (50,14) is `═` in `RULE`.
    - Row 14 contains `format!(" {:.1} kg · ¤ {} ", weight, value)` ending at x 96.
    - Switch to Pack by key, with `st.tick = seg start + 6`: (50,14) is `━` and
      (48,14) is `═`.
    - At `seg start + 3`, the first `━` on row 14 lies strictly between x 1 and x 50.
  - `the_card_types_beside_the_viewer`:
    - Fit `hushcan` and `redeye` on the worn Ratchet. Tile 6.
    - Row 15 from x 3 starts `"RATCHET SMG"`. Row 16 starts
      `"weapon · primary · 3.9 kg · ¤ 2150"`.
    - Corners in `RULE`: `┌` (46,15), `┐` (97,15), `└` (46,26) and `┘` (97,26).
    - Row 26 from x 83 reads `"space inspect"`.
    - Some cell in `Rect(50,17,44,8)` has fg or bg other than `PANEL_BG` and
      `Reset`.
  - `an_item_without_a_drawing_shows_the_missing_box`: tile 1 (Nightglass optics).
    - `┌` at (61,18), `┐` at (81,18), `└` at (61,22), `┘` at (81,22), all `FAINTER`.
    - `◈` at (71,19) in `DIM`.
    - `"MISSING BLUEPRINT"` from (63,21).
  - `an_empty_slot_shows_an_empty_viewer`:
    - Tile 0, with HEAD empty for D'SHAWN.
    - Row 15 starts `"— EMPTY —"`.
    - Row 26 holds no `space inspect`. The corners are there.
  - `nothing_spills_outside_the_body` (Review Focus): for the bodies
    `(1,5,98,22)`, `(1,5,99,23)` and `(1,10,118,27)`, in both panes with filter
    `Some(Hardware)`:
    - Fill the buffer with `'X'`, draw, then check every cell outside the body is
      still `'X'`.
    - The chip row fits inside the pack.

- [ ] **Step 2: Run to verify they fail.** `cargo test -q -p strata-zero kit::` → FAIL
  (no `draw`, no `FAINTER`).

- [ ] **Step 3: Implement** `draw` and `FAINTER`.

- [ ] **Step 4: Run.** The full check passes.

- [ ] **Step 5: Commit.** `feat(app): the kit drawn: rack, pack, rule, card and viewer`

### Task 7: One tab in the deck

**Files:**
- Modify: `crates/strata-zero/src/screens/deck.rs` (`Tab`, `Deck`, tests),
  `crates/strata-zero/src/app.rs` (`AppState`, `enter_city`, its test)

**Interfaces:**
- Consumes: `Kit`, `KitState`, `Pane` (Tasks 5–6).
- Produces:
  - **`Tab`:**
    - `Tab::Kit` replaces `Gear` and `Inventory`, third in `Tab::ALL: [Tab; 8]`,
      labelled `"GEAR & INVENTORY"`.
    - The `Talents` doc says `8`, and `0` too.
  - **`AppState`:** `pub deck_kit: crate::screens::kit::KitState`, default, reset in
    `enter_city` with the rest of the deck's memory.
  - **`Kit::restore`:** `pub fn restore(st: &AppState) -> Kit` takes
    `st.deck_kit`, `reselect`s, and sets `changed = st.tick`.
  - **`Deck`** gets `pub(crate) kit: Kit`:
    - `new` and `restore` build it.
    - `update` writes `st.deck_kit = self.kit.state`.
    - `set_tab` into `Kit` sets `kit.changed`.
    - `rows` returns `Vec::new()` for `Kit`.
    - `interact` delegates to `kit.interact`.
    - `draw` calls `kit.draw` for the body.
    - The hint is `kit.hints()`.
  - **`handle` on `Tab::Kit`:**
    - Each key first calls `kit.reselect(st)`.
    - Arrows, `h j k l`, `[` and `]` go to `kit.key`.
    - `space` and `enter` push `kit.interact`'s popup.
    - `tab`, `shift-tab`, the digits, `esc` and `q` fall through to the deck's own
      arms.
    - The left and right arrows never switch tabs here.
  - **Removed:** `gear_rows`, `inventory_rows`, the INVENTORY footer and its
    reserved rows.

- [ ] **Step 1: Port the tests.**
  - Add test helpers: `on_tile(d, st, slot)` presses `3` and sets `d.kit.state`
    to pane Rack with that slot's tile. `on_pack(d, st, pick)` presses `3`, sets
    pane Pack, and sets `row` to the first position in `d.kit.pack_rows(st)` that
    `pick` takes.
  - Port with the helpers, same assertions unless noted:
    - `gear_slot_picker_equips_what_fits`
    - `inventory_verbs_drop_and_use` (re-read rows with `d.kit.pack_rows`)
    - `deck_reselects_after_a_popup_removes_the_row`
    - `inspect_opens_the_blueprint_window`: the `inspect` helper takes a
      `Detail`-or-row pick through the helpers.
    - `jacked_in_gear_and_items_only_inspect`
    - `inspect_follows_where_the_deck_lies`
    - `two_copies_act_on_their_own_row`
    - `gear_picker_reads_copies_apart`
    - `cards_list_sockets_and_mods`: cards through `card(&st, &detail)`.
    - `the_gear_row_and_card_show_the_guns_sockets`: the row becomes the PRIMARY
      tile's bottom edge, which holds `□□` and then `■□`.
    - `the_hardware_card_names_its_socket_and_stat`
    - `inventory_groups_programs_and_marks_the_loaded`
    - `skills_and_gear_show_perks`
    - `inventory_marks_and_hardware`: texts from `d.kit.pack_rows`, then drawn with
      pane Pack.
  - Rewrite these:
    - `tabs_are_in_the_new_order_with_two_new_ones` → labels `["SKILLS", "STATS",
      "GEAR & INVENTORY", "WORKBENCH", "JOBS", "JOURNAL", "MAP", "TALENTS"]`.
      - `6` is JOURNAL.
      - `8` and `0` are TALENTS.
      - `9` changes nothing.
      - `st.deck_rows.len() == 8`.
    - `deck_remembers_its_tab_and_row` → the kit remembers pane Pack, `row`, tile 4
      and filter `Some(Program)` through `Deck::new(5).restore(&st)`.
      - The pack row clamps after `inventory.truncate(1)`.
    - `deck_tabs_and_rows_render_from_content`, the GEAR/INVENTORY part:
      - Key `3` shows `▌GEAR & INVENTORY▐`, `PRIMARY`, `Ratchet SMG` and `— empty —`.
      - Right → EYES; its card shows `NIGHTGLASS OPTICS` and `cyberware · eyes`.
      - Right Right Right → the pack, with `── component ──` and `3× Copper coil`.
      - Left never leaves the tab.
      - Tab / BackTab still cycle: from SKILLS, BackTab ×2 → MAP.
    - `inventory_footer_keeps_its_own_row` → covered by Task 6's rule test: delete.
    - `list_scroll_fills_the_pane_at_the_end` → at the last pack row at 100×30, the
      pack list's 8 rows are all filled.
    - `gear_marks_slots_and_sockets` → covered by Task 6's rack test: delete.
    - `rows_wrap_around` → covered by Task 5: delete.
  - In `app.rs`, the `enter_city` test sets `st.deck_kit` to a non-default value
    and asserts it comes back `KitState::default()`.

- [ ] **Step 2: Run to verify.** `cargo test -q -p strata-zero` → the ported tests
  FAIL (no `Tab::Kit`).

- [ ] **Step 3: Implement** as the Interfaces say.

- [ ] **Step 4: Run.** The full check passes. `grep -rn "Tab::Gear\|Tab::Inventory"
  crates/` finds nothing.

- [ ] **Step 5: Commit.** `feat(app): GEAR & INVENTORY, one tab in the deck`

### Task 8: Shots and the README

**Files:**
- Modify: `crates/strata-zero/src/screens/shots.rs`, `README.md`

- [ ] **Step 1: Shots.**
  - `blueprint_shots(st, worn: bool, item, name, sizes)` replaces the tab char. It
    opens the deck on `3` and sets the kit's pane to `Rack` when worn, `Pack` when
    not. Update every caller: `'3'` → `true`, `'4'` → `false`.
  - The two modding shots press `3` as before.
  - Replace `gear_and_inventory_marks` with `gear_and_inventory`, which writes:
    - `gear_and_inventory_rack_{w}x{h}`: D'SHAWN with the Hushcan and Redeye fitted
      on the Ratchet, the rack on PRIMARY.
    - `gear_and_inventory_pack_{w}x{h}`: a fresh D'SHAWN, the pack focused, filter
      `Some(Hardware)`, the cursor on `Hushcan`.

    Both at `(100,30)` and `(120,40)`, with the tick past the typewriter and the
    jitter (as `blueprint_shots` does).

- [ ] **Step 2: Run the shots.** `SHOT_DIR=<scratch dir> cargo test -q -p
  strata-zero --target-dir target/smoke shots:: -- --ignored` → 15 or more pass and
  the four new SVGs exist.

- [ ] **Step 3: README.**
  - Line 128: "Inspect it in GEAR" → "Inspect it on GEAR & INVENTORY's rack".
  - The **Deck** paragraph:
    - `1`-`8` tabs, `0` is talents too.
    - The tab list names "gear & inventory" in place of "gear, inventory".
    - Add, in the README's voice, that this tab is three panels:
      - The rack: nine slots as tiles; arrows move between them and on to the pack.
      - The pack, with a row of kind icons on top; `[ ]` filter it, and the active
        icon names its kind.
      - The selected item's card beside its drawing at half size.
      - `space` on a tile picks what fits or unequips; on a pack row it opens the
        verbs.
      - `←→` move between gear and pack, so `tab` and `shift-tab` switch tabs here.
  - The marks sentence: "In GEAR and INVENTORY" → "On the rack and in the pack". On
    the rack, the marks sit in the tile's bottom edge.
  - Talents: "(deck tab `9` or `0`)" → "(deck tab `8` or `0`)".

- [ ] **Step 4: Run.** The full check passes.

- [ ] **Step 5: Commit.** `docs: GEAR & INVENTORY in the README; shots`
