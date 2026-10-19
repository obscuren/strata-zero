# WORKBENCH as a Drawing Set Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The WORKBENCH tab becomes a set of engineering drawings, one per recipe: the
output's blueprint exploded into its parts (balloons, a parts list, a title block)
when its drawing declares parts, a process diagram otherwise; space builds, with a
closing animation. The Ratchet SMG is the first assembly, from five new components.

**Architecture:** The game crate gains `Recipe.steps`, `Action::Build` and the
content. The UI's `Art` seam gains parts (`PartPlace`, `parts`, `draw_part`), which
the Ratchet's look answers by splitting its drawing into five. A new
`screens/workbench.rs` holds the sheet's state, drawing and keys; the deck screen
only routes the WORKBENCH tab to it.

**Tech Stack:** Rust 2024, ratatui (TestBackend in tests), serde + RON content,
`unicode-width` in tests.

**Spec:** `docs/superpowers/specs/2026-10-19-workbench-design.md`. Layout reference:
`docs/superpowers/specs/2026-10-19-workbench-mock/wb.py` (`concept_a`,
`concept_a_process`, `sheet`, `parts_list`, `title_block`); render it with the
command in the spec's header to see every cell. Where this plan gives a column or
row it is at 100×30, where the deck's body is `Rect(1, 5, 98, 22)`.

## Global Constraints

- Everything fits 100×30 exactly as the mock; every glyph single width
  (`unicode-width` 1); every colour `Color::Rgb` (or `Reset`).
- Colours from `ui/chrome.rs`: `CYAN`, `MAGENTA` (every warning), `DIM`, `TEXT`,
  `RULE`, `BAR_BG`, `PANEL_BG`. Local constants in `screens/workbench.rs`:
  `FAINTER = #5c6673` (zone marks, centre lines, the header in the rule, the DWG
  line, process lines), `GREEN = #5ac88c`, `WARN_BG = #2a0622`.
- Nothing in `screens/workbench.rs` names an item, recipe or skill: wording comes
  from names, kinds, slots, counts. The Ratchet gets its sheet only because its
  look declares parts.
- `screens/deck.rs` changes only to route the tab and to drop the old recipe rows
  and card; the GEAR & INVENTORY rework on `worktree-gear-inventory` is reshaping
  that file. Do not touch `ui/chrome.rs`.
- Comments and doc comments read like the surrounding code: plain sentences, no
  task numbers.
- Run tests with `cargo test -q` at the workspace root; `cargo clippy --workspace
  --all-targets -- -D warnings` and `cargo fmt --check` stay clean after each task.
- Never build `target/release/strata` (Jeffrey may be playing it). Shots use
  `--target-dir target/smoke`.
- Commit after each task with a conventional message ending in:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  ```

## Review Focus

- **Flipping to a recipe with fewer inputs** keeps a part index past its list:
  flipping resets the part to 0, and a restored state past the counts clamps (Task 3).
- **A recipe with more inputs than rows** (5 at 100×30) scrolls the parts list to
  keep the cursor's row in view, and its process diagram falls back to one-line
  entries; nothing panics (Task 3).
- **A recipe whose output has no item or no look** draws a process sheet; one whose
  look has parts but whose exploded drawing doesn't fit the field draws the item
  whole (Tasks 3 and 4).
- **Space pressed again during the animation** ends it and builds nothing (Task 5).
- **A long handle** (16 characters, the creation cap) clips the DWG line at the
  sheet's inner edge, and at 120×40 the title block moves with the right edge
  (Task 3).

---

### Task 1: Building in the game

**Files:**
- Modify: `crates/game/src/model.rs` (`Recipe`)
- Modify: `crates/game/src/content.rs` (`ContentError`, `validate`, tests)
- Modify: `crates/game/src/action.rs` (`Action`, `apply`, tests)
- Modify: `crates/game/src/character.rs` (`debug_runner`, tests)
- Modify: `flavor/content.ron` (five items, one recipe, steps)
- Modify: `crates/strata-zero/src/actions.rs` (`handles_items`, its test list)

**Interfaces:**
- Produces: `Recipe.steps: Vec<String>` (`#[serde(default)]`);
  `Action::Build(RecipeId)`; its messages `built `<name>`` (one) and
  `built <n>× `<name>`` (more), refusals `unknown recipe <id>`, `no workbench`,
  `needs <Skill name>`, `needs <n> more <Item name>`;
  `ContentError::RecipeInvalid { id: String, why: String }` (`"recipe `{id}`: {why}"`).
  Item ids `ratchet_receiver`, `ratchet_stock`, `ratchet_barrel`, `ratchet_mag`,
  `ratchet_trigger`; recipe id `ratchet_build`.

- [ ] **Step 1: Write the failing tests**

In `action.rs` tests (use `setup()`; VEX has the workbench and Electronics, not
Scrapcraft or Medtech; add skills with `ch.skills.insert(SkillId::new(..))` and
items with `ch.give(..)`):

```rust
#[test]
fn build_spends_the_inputs_and_gives_the_output() {
    // wire_shiv: copper_coil 1 + scrap_servo 1 -> shiv_blade 1. VEX carries 2 servos.
    // give 1 copper_coil; apply Build(wire_shiv)
    // -> Ok, message == format!("built `{}`", name of shiv_blade)
    // -> count(copper_coil) == 0, count(scrap_servo) == 1, count(shiv_blade) == 1
}

#[test]
fn build_of_a_batch_names_its_count() {
    // insert medtech; give copper_coil 1; stim_batch -> message "built 2× `Stim patch`"
}

#[test]
fn build_refuses_before_changing_anything() {
    // ratchet_build without scrapcraft -> Err("needs Scrapcraft")
    // insert scrapcraft; give receiver, stock, mag, trigger (no barrel)
    //   -> Err("needs 1 more Barrel 9×19"), the four parts still there
    // RecipeId "nope" -> Err("unknown recipe nope")
    // KESSLER (no WRENCH, has no workbench) on wire_shiv -> Err("no workbench")
}

#[test]
fn build_of_an_item_with_sockets_makes_an_empty_copy() {
    // scrapcraft + all five parts; Build(ratchet_build)
    // -> one more ratchet_smg stack in the pack, with a copy whose fitted are all None
    //    and as many entries as the item has sockets; no part left; "built `Ratchet SMG`"
}
```

In `content.rs` tests: the shipped content has `ratchet_build` with skill
`scrapcraft`, inputs exactly the five ids above ×1 in that order, output
`ratchet_smg` ×1 and no steps, and each of the six older recipes has 2 to 4 steps;
`validate` refuses, as `RecipeInvalid`, a recipe with 5 steps, one with a
17-character step, and one whose input is `ratchet_smg` (held per copy).

In `character.rs` tests: `debug_runner` has Scrapcraft, one receiver, stock and
trigger, two magazines and no barrel.

In `crates/strata-zero/src/actions.rs`, add `Action::Build(RecipeId::new("wire_shiv"))`
to the list of actions the jacked-in test checks are refused with `JACKED_IN`.

- [ ] **Step 2: Run to verify they fail** — `cargo test -q -p game` → compile errors
  (no `Build`, no `steps`).

- [ ] **Step 3: Implement**

`apply`'s `Build` arm checks in the order of the refusals above, before changing
anything; the first input short names `need − have`. Then `take_from_pack` each
input, `ch.give_item(c, output, n)`. No capacity check (building is not taking).
`handles_items` gains `Action::Build(_)`. Validation runs in the existing recipe
loop. Content, in `flavor/content.ron` (items next to the other components):

| id | name | kg | ¤ | desc |
|----|------|----|---|------|
| `ratchet_receiver` | Stamped receiver | 1.2 | 300 | The Ratchet's box: stamped steel, a serial number filed to fuzz. |
| `ratchet_stock` | Wire stock | 0.5 | 80 | Bent rod on a hinge. It folds, in theory. |
| `ratchet_barrel` | Barrel 9×19 | 0.6 | 220 | A short tube with gold threads at the end, for whatever you screw on. |
| `ratchet_mag` | Magazine | 0.4 | 60 | Thirty rounds of 9×19 and a strip of label tape that says GOOD STUFF. |
| `ratchet_trigger` | Trigger group | 0.3 | 140 | Hammer, sear, spring, and a selector filed past safe. |

Recipe `ratchet_build` "Ratchet SMG", desc "Five parts and a pin punch. It only goes
together one way." Steps: `wire_shiv` ["wrap the coil", "pull it taut"],
`hardhat_visor` ["scrape the glass", "wire it in"], `caps_fence` ["count the caps",
"haggle", "take the wire"], `stim_batch` ["boil", "filter", "pray"], `optics_tune`
["re-seat lenses", "kill boot logo"], `servo_rebuild` ["strip", "rewind", "seat"].
`debug_runner` inserts `scrapcraft` and gives the parts with `give_item`.

- [ ] **Step 4: Run** `cargo test -q && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check` → all pass.

- [ ] **Step 5: Commit** `feat(game): build a recipe; the Ratchet's five parts`

---

### Task 2: Drawings declare their parts

**Files:**
- Modify: `crates/strata-zero/src/ui/art.rs` (`PartPlace`, `Art::parts`, `Art::draw_part`)
- Modify: `crates/strata-zero/src/ui/looks/mod.rs` (`ItemArt`, tests)
- Modify: `crates/strata-zero/src/ui/looks/ratchet.rs` (split the drawing, `PARTS`)

**Interfaces:**
- Consumes: Task 1's item ids and `ratchet_build`.
- Produces:
  ```rust
  /// A part on a drawing, in drawing coordinates: what it is, where it sits whole,
  /// how far it moves on an exploded drawing, and where its balloon goes.
  pub struct PartPlace {
      pub index: usize,          // its place in the look's parts
      pub item: &'static str,    // the part item's id
      pub x: i32, pub y: i32, pub w: u16, pub h: u16,
      pub apart: (i16, i16),
      /// The balloon's left cell, from the drawing's origin, moving with the part;
      /// the leader runs `dir` from the balloon's edge for `len` cells.
      pub balloon: (i32, i32),
      pub leader: (Dir, u8),
  }
  // on trait Art, both with defaults:
  fn parts(&self) -> Vec<PartPlace> { Vec::new() }
  fn draw_part(&self, part: usize, origin: (u16, u16), clip: Rect, buf: &mut Buffer) {}
  // ratchet.rs
  pub fn draw_part(p: &mut Pen, part: usize);
  pub const PARTS: [..; 5];  // the table below
  ```
  `ItemArt::parts` maps its look's table (only `ItemLook::Ratchet` has one);
  `ItemArt::draw_part` draws that part alone and never fitted hardware.

- [ ] **Step 1: Pin today's drawing** — add `the_whole_gun_is_unchanged` to
  `looks/mod.rs` tests: draw `ItemArt { look: Ratchet, fitted: vec![None, None] }`
  into an empty 88×15 buffer and compare a hash of every cell's
  `(symbol, fg, bg, modifier)` (std `DefaultHasher`) with a constant. Run it once on
  the current code, put the printed value in the constant, and see it pass before
  any refactor.

- [ ] **Step 2: Write the failing tests** in `looks/mod.rs`:
  - `parts_drawn_alone_add_up_to_the_whole_gun`: each part drawn at offset zero into
    its own cleared buffer; every cell the whole drawing writes is written by
    exactly one part, with the same symbol, colours and modifier; no part writes a
    cell the whole drawing leaves empty.
  - `a_part_place_is_its_drawing_s_bounding_box`: for each part, `(x, y, w, h)` is
    exactly the bounding box of the cells `draw_part` writes; no two rects overlap.
  - `every_part_is_an_input_of_the_recipe_for_its_item`: with `Content::load()`, for
    every item whose look has parts, some recipe outputs it and lists every part's
    `item` among its inputs.
  - the existing single-width/exact-colour test also runs over each part alone.

- [ ] **Step 3: Run to verify they fail** — `cargo test -q -p strata-zero looks::` →
  compile errors (no `parts`, `draw_part`).

- [ ] **Step 4: Implement** — split `ratchet::draw` into one function per part, each
  painting its own pixels on the canvas it is given and putting its own text: the
  stock (butt pad, rods, mount), the receiver (box, port, wear marks, knob, rail,
  and the RATCHET / 9×19 / SN / selector stamping), the barrel (barrel and gold
  threads), the magazine (grip, magazine, tape, base plate, GOOD STUFF label), the
  trigger group (guard and trigger). `draw` paints all five on one canvas, folds
  once and puts all text, so its output is unchanged; `draw_part` paints one on a
  fresh canvas, folds and puts that part's text. A pixel on a boundary goes to
  whichever part reads right. The table, balloons and leaders from the mock
  (rects are then whatever Step 2's bounding-box test finds; these are the expected
  values to within a cell):

  | # | item | rect about | apart | balloon | leader |
  |---|------|------------|-------|---------|--------|
  | 0 | `ratchet_receiver` | x 21-59, y 3-8 | (0, 0) | (24, 2) | (Down, 1) |
  | 1 | `ratchet_stock` | x 2-24, y 3-10 | (−5, 0) | (8, 2) | (Down, 1) |
  | 2 | `ratchet_barrel` | x 60-69, y 5-7 | (5, 0) | (64, 2) | (Down, 1) |
  | 3 | `ratchet_mag` | x 35-51, y 9-14 | (0, 1) | (28, 12) | (Right, 3) |
  | 4 | `ratchet_trigger` | x 49-58, y 9-12 | (0, 1) | (63, 11) | (Left, 2) |

  The table's order is the look's part order; the sheet matches parts to recipe
  inputs by `item`, not by order. The spec gave a part only a balloon side; the
  balloon's cell and leader are art data here, as the spec left exact cells to the
  builder, so each look places them where its drawing has room.

- [ ] **Step 5: Run** `cargo test -q && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check` → all pass, including every existing blueprint test unchanged.

- [ ] **Step 6: Commit** `feat(app): drawings declare their parts; the Ratchet in five`

---

### Task 3: The drawing set on the WORKBENCH tab

**Files:**
- Create: `crates/strata-zero/src/screens/workbench.rs`
- Modify: `crates/strata-zero/src/screens/mod.rs` (`pub mod workbench;`)
- Modify: `crates/strata-zero/src/screens/deck.rs` (routing; remove `workbench_rows`,
  `Detail::Recipe` and its card; rewrite the workbench half of
  `workbench_and_skills_show_what_you_can_and_cannot_do`)

**Interfaces:**
- Consumes: Task 1's `Recipe.steps`; `render::glyphs::kind_icon`; `ItemKind::label`,
  `Slot::label`; `Character::count`, `has_skill`.
- Produces:
  ```rust
  #[derive(Clone, Debug, Default, PartialEq)]
  pub struct Workbench { drawing: usize, part: usize /* Task 5 adds the animation */ }
  impl Workbench {
      pub fn pack(&self) -> usize;              // drawing << 16 | part
      pub fn unpack(v: usize) -> Workbench;
      pub fn draw(&self, st: &AppState, body: Rect, buf: &mut Buffer);
      /// Takes the sheet's keys; false for any other, which the deck then handles.
      pub fn key(&mut self, st: &mut AppState, code: KeyCode) -> bool;
      pub fn hints(&self, st: &AppState) -> &'static str;
  }
  /// Recipe indices in sheet order: stably by the skill's place in `content.skills`.
  pub(crate) fn drawings(c: &Content) -> Vec<usize>;
  ```
  Deck: a `workbench: Workbench` field; `restore` sets it from
  `st.deck_rows[Tab::Workbench.index()]`, `update` writes `pack()` there after the
  tab's own row; when the tab is WORKBENCH and `has_workbench`, `draw` calls
  `workbench.draw(st, body, buf)` (as TALENTS calls its view), the hint line is
  `workbench.hints(st)`, and `handle` offers each key to `workbench.key` first,
  skipping the rest of the match when it is taken. The locked view stays as today.

- [ ] **Step 1: Write the failing tests** in `workbench.rs` (draw a `Deck` on the tab
  at 100×30 with `TestBackend`, as `deck.rs` tests do, D'SHAWN's character):
  - `drawings_go_by_skill`: ids in order `wire_shiv, hardhat_visor, optics_tune,
    servo_rebuild, caps_fence, stim_batch, ratchet_build`.
  - `the_sheet_frames_the_body`: row 5 has `┌` at column 2 and `┐` at 97; row 26 `└`,
    `┴` at 61, `┘` at 97; row 20 has `├` at 2, ` # ` at 5, ` PART ` at 8, ` QTY ` at
    31, ` ON HAND ` at 37, `┬` at 61, `┤` at 97; `A` at (2, 7), `1` at (8, 5); row 27
    is left to the status strip (not written by the sheet).
  - `a_process_sheet` (flip to `stim_batch` with →; give 1 copper coil): title rows
    21-25 from column 63: `STIM BATCH` then `◂ 6/7 ▸` at 87; `makes 2× Stim patch ·
    consumable`; `needs Medtech` (magenta, not `· yours`); `every part on hand`;
    `DWG 6/7 · process · drawn D'SHAWN`. Parts rows 21-22: `1  Copper coil` with `1`
    at 33, the count at 39, `■` at 45. The field holds `MEDTECH`, `1 boil`, `2
    filter`, `3 pray`, `2× Stim patch`.
  - `a_short_input_is_dashed_and_named` (`servo_rebuild`; D'SHAWN has 2 servos, give
    him 2 copper coils so only the servos are short):
    its input box uses `╌`, the row reads `not in pack` at column 48, the title's
    fourth line `2 more Scrap servo`.
  - `keys_flip_and_walk`: → and ← wrap through all seven; ↓ and ↑ wrap through the
    inputs; flipping resets the part to 0; `tab` still changes tab; the hint line
    reads `←→ drawings · ↑↓ parts · space build · tab tabs · esc close` on a process
    sheet.
  - `the_sheet_reopens_where_it_was`: close and `restore` → same drawing and part;
    `unpack` of a value past the counts clamps to the last drawing and part 0.
  - `many_inputs_scroll`: a test recipe with 7 inputs (push onto `st.content.recipes`)
    → five rows shown, the cursor on input 7 keeps its row in view, the field shows
    one-line entries, no panic.
  - `odd_outputs_still_draw`: a recipe whose output id is unknown draws a process
    sheet without panicking.
  - `big_terminal_and_long_handle`: at 120×40 the title block starts at column 83
    and the sheet's right edge is 117; a 16-character handle clips the DWG line
    before the edge.

- [ ] **Step 2: Run to verify they fail** — `cargo test -q -p strata-zero workbench` → compile errors.

- [ ] **Step 3: Implement** — the layout, relative to the body (`sheet` = body less a
  column each side, so its edges are columns 2 and 97; `rule` = `sheet.bottom() − 7`;
  the field is the rows between the top edge and the rule; the title block's text at
  `sheet.right() − 35`, its divider at `sheet.right() − 37`; the parts list from
  `x0 = sheet.x + 1`), cell by cell as `wb.py`'s `sheet`, `parts_list`,
  `title_block` and `concept_a_process`:
  - Zone marks: `┬` at `sheet.x + 12k` while `< sheet.right() − 5`, digit `k` six
    columns before each; `├` at `sheet.y + 4k` while above the rule, letter two rows
    before each.
  - Parts list columns from its left `x0`: cursor `x0+1`, number `x0+3`, name `x0+6`
    (cut with `…` before `x0+29`), need `x0+30`, have `x0+36`, mark `x0+42`,
    `not in pack` `x0+45`; the selected row's bar from `x0` to the divider. Header
    in the rule at `x0+2`, `x0+5`, `x0+28`, `x0+34`.
  - Title block lines as the spec lists, clipped one column inside the sheet's right
    edge, `◂ i/n ▸` ending four columns before it.
  - Process diagram: input boxes 26 wide, 3 rows, from `field.x + 9`, centred down
    the field with up to 3 rows between them (one-line entries when they don't fit),
    balloons four columns to their left; the junction six columns past the boxes,
    the process box seven past the junction (width from its longest line, at least
    20), the result box seven past it to two columns inside the sheet's right edge
    (column 95).
  Space does nothing yet.

- [ ] **Step 4: Run** `cargo test -q && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check` → all pass (the locked-view test unchanged).

- [ ] **Step 5: Commit** `feat(app): WORKBENCH as a set of drawings, the process sheet`

---

### Task 4: The exploded drawing

**Files:**
- Modify: `crates/strata-zero/src/screens/workbench.rs`

**Interfaces:**
- Consumes: Task 2's `Art::parts`, `Art::draw_part`, `PartPlace`; `ItemArt`;
  `art::grey` (its formula), `art::brackets`.
- Produces: a sheet whose output item has a look with parts draws exploded; the
  hint line reads `space assemble` there; the title's DWG line says `assembly`.

- [ ] **Step 1: Write the failing tests** (D'SHAWN, ← once to `ratchet_build`):
  - `the_ratchet_explodes`: title `RATCHET SMG`, `◂ 7/7 ▸`, `weapon · primary · 3.2
    kg`, `needs Scrapcraft · yours`, `1 more Barrel 9×19`, `DWG 7/7 · assembly ·
    drawn D'SHAWN`; balloons ` 1 `…` 5 ` in the field; the drawing's cells match the
    mock's placement: the receiver's top-left cell at the column and row
    `concept_a` puts it (compare one known cell of each part against the same cell
    of a whole-drawing render shifted by `apart`).
  - `a_missing_part_is_a_ghost_in_a_dashed_box`: the barrel's cells are greyed
    darker than modding's grey, a `╌`/`╎` box in magenta round its rect one column
    out, `none on hand` two rows under the rect; ↓↓ selects it and brackets sit
    round it (`┏`, `┗` …) per the rule below; its balloon is inverted.
  - `centre_lines_fill_the_gaps`: `╌` in the five columns the stock left and the
    five the barrel left, on the barrel's middle row; `╎` in the row the magazine
    and trigger left.
  - `too_big_draws_whole`: with a field too small (shrink the body in the test),
    the item draws whole and no balloon is drawn.

- [ ] **Step 2: Run to verify they fail** — `cargo test -q -p strata-zero workbench` → FAIL.

- [ ] **Step 3: Implement** — the art is `ItemArt { look, fitted: vec![None; sockets] }`
  for the output item's `look`. Inputs match parts by `item`; the balloon number is
  the input's row number; an input without a part has no balloon. The exploded box
  is the union of every moved rect and its balloon and leader cells, centred in the
  field. Each part is drawn with `draw_part` at the origin plus `apart`; a part on
  hand as drawn; a part short drawn to a scratch buffer and copied greyed by
  luminance × 0.26 (`grey`'s formula with 0.26 for 0.38, blue × 1.08), text glyphs
  blanked, then the magenta dashed box at `rect` widened a column each side, and
  `none on hand` at `(rect.x − 1, rect.bottom() + 1)`. Centre lines in the cells the
  home rect has and the moved one doesn't, on its middle row (horizontal moves) or
  column (vertical). Balloons ` n ` bold: cyan on `BAR_BG` / magenta on `WARN_BG`,
  inverted when selected; leaders `│`/`─` in the balloon's colour. The selected
  part's brackets: `art::brackets` round `rect` grown by two columns and one row
  each side.

- [ ] **Step 4: Run** `cargo test -q && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check` → all pass.

- [ ] **Step 5: Commit** `feat(app): the exploded drawing on the workbench`

---

### Task 5: Assembling

**Files:**
- Modify: `crates/strata-zero/src/screens/workbench.rs`

**Interfaces:**
- Consumes: Task 1's `Action::Build`; `actions::{perform, report}`.
- Produces: `Workbench` gains `anim: Option<(u64, usize)>` (start tick, drawing);
  `CLOSE_TICKS = 15`, `HOLD_TICKS = 30`.

- [ ] **Step 1: Write the failing tests**:
  - `space_assembles_and_the_parts_close`: give D'SHAWN a barrel, ← to the Ratchet,
    space → the log's latest is `built `Ratchet SMG``, one more Ratchet copy with
    empty sockets, no barrel; drawn at `start + 7` no balloon shows and the stock sits
    between its exploded and home columns; at `start + 20` the field shows
    `ASSEMBLED`; at `start + 46` it is gone and the parts short show `not in pack`.
  - `space_again_ends_it_and_builds_nothing`: a second space at `start + 5` ends the
    animation (the sheet is back) and the Ratchet count rises by one only; `esc` at
    `start + 5` ends it without closing the deck.
  - `a_refusal_starts_nothing`: space with the barrel missing → the log's latest is a
    warning `needs 1 more Barrel 9×19`, no animation.
  - `a_process_builds_and_lights`: Stim batch with Medtech and a coil → `BUILT` at
    `start + 20`, the result box's edges cyan, two more stim patches.

- [ ] **Step 2: Run to verify they fail** — `cargo test -q -p strata-zero workbench` → FAIL.

- [ ] **Step 3: Implement** — space or enter: if an animation is running, end it and
  take the key; otherwise `perform(st, Action::Build(id))`, `report` it, and start
  the animation on `Ok`. Every other key while it runs also just ends it (`key`
  returns true). Exploded: for `t < CLOSE_TICKS` each part at `apart × (1 − ease(t /
  CLOSE_TICKS))` rounded, all in colour, no balloons, centre lines, outlines or
  brackets; then until `CLOSE_TICKS + HOLD_TICKS` the item whole (`Art::draw`) at
  the home origin, brackets round its whole rect, and ` ASSEMBLED ` bold, `PANEL_BG`
  on `CYAN`, centred on the row under it (or the field's last row). Process: the
  three runs (lines in, the process box, the line out) turn cyan as `t` passes
  thirds of `CLOSE_TICKS`; then ` BUILT ` the same way on the result box's bottom
  edge for `HOLD_TICKS`. The animation ends by itself after that.

- [ ] **Step 4: Run** `cargo test -q && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check` → all pass.

- [ ] **Step 5: Commit** `feat(app): assembling on the workbench`

---

### Task 6: Shots and the README

**Files:**
- Modify: `crates/strata-zero/src/screens/shots.rs`
- Modify: `README.md` (the deck paragraph's workbench clause)

- [ ] **Step 1: Add shots** — an ignored `workbench_sheets` test (D'SHAWN, as the
  blueprint shots set him): `workbench_ratchet_100x30` (← to the Ratchet, ↓↓ to the
  barrel), `workbench_ratchet_120x40`, `workbench_ratchet_whole_100x30` (a barrel
  given), `workbench_stim_100x30`, each over the walking map.

- [ ] **Step 2: Render and look** —
  `SHOT_DIR=<scratch dir> cargo test -q --target-dir target/smoke -p strata-zero shots:: -- --ignored workbench`,
  turn the SVGs into PNGs (headless Chrome `--screenshot` on the `.svg`, or
  `qlmanage -t -s 1400`), and compare with `wb.py`'s `a_sheet` and `a_process`
  renders: the same cells but for the row the header moved into, `7/7`, and the
  title lines the spec lists. Fix any drift in the code, not the shot.

- [ ] **Step 3: README** — the workbench clause becomes: workbench (WRENCH unlocks it;
  every recipe is a drawing: `←→` flips them, `↑↓` walks the parts, `space` builds
  once every part is on hand; an item whose drawing has parts is drawn exploded, the
  rest as a process).

- [ ] **Step 4: Run** `cargo test -q && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check` → all pass.

- [ ] **Step 5: Commit** `docs: the workbench's drawings in the README; shots`
