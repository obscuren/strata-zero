# WORKBENCH as a drawing set: design

Date: 2026-10-19. Status: designed with Jeffrey in the visual companion (terminal-
accurate mockups over the game's own 100×30 shots). He picked the drawing sheet out
of three concepts ("A is the one, holy shit thats good") and approved the process
sheet for builds without a drawing ("Absolutely perfect"). He approved the whole
design in chat ("Nothing is missing. it's perfect") and asked for plan and build to
run on without check-ins. Worktree `.claude/worktrees/workbench`, branch
`worktree-workbench`.
Builds on: `2026-10-19-item-sockets-design.md` and `2026-10-19-smg-sockets-design.md`
(the `Art` seam, looks, sockets as places a drawing declares, the jacked-in lock),
`2026-10-18-cyberdeck-tab-design.md` (the blueprint window, corner brackets).
Layout reference: `2026-10-19-workbench-mock/` (`wb.py`; render with
`SHOT_DIR=… cargo test -p strata-zero shots:: -- --ignored`, then
`SHOT_DIR=… python3 wb.py out.html a_sheet a_process a_process_servo`). Columns and
rows in this document are terminal cells at 100×30.

## Intent

Today WORKBENCH is a flat list of recipes with `▪`/`▫` marks and a text card, and
nothing can be built. The rest of the game has grown real drawings (the blueprint
window, the deck, the Ratchet, its hardware); the workbench should match them.

Jeffrey wants the workbench to be about "assembling items from pieces like a true
engineer". Each build becomes an **engineering drawing**: the item's own blueprint
**exploded** into its parts, with numbered balloons, a **parts list** and a **title
block**. You walk the parts, see what you have and what you're missing, and once
everything is on hand you assemble it: the parts slide together and the item drops
into your pack.

A build whose result has no drawing with parts (most recipes today: stims, fenced
caps, a shiv) gets the same sheet with a **process diagram** where the exploded view
would be: the parts flow into a box naming the skill and its steps, then out to the
result.

## Decided with Jeffrey

- **Concept A, the drawing sheet**, over the bench mat (B) and the register plus an
  assembly window (C).
- **The process sheet** for builds without a drawing with parts. No recipe leaves
  the workbench.
- **Parts are real items.** For now they come only from the debug pack: "Content
  will be added later on when we got the components in place that make up the
  game." World sources (shops, loot) are not part of this work.
- Everything in the playback, approved as a whole ("Nothing is missing"):
  - **Keys** as on TALENTS: ←→ flips drawings, ↑↓ walks the parts list, tab and
    shift-tab switch tabs, 0-9 jump, esc closes.
  - **Every known recipe is a drawing**, ordered by skill.
  - **Without the skill** the title block says so in magenta, and space refuses
    with the reason.
  - **Assembling**: space works once every part is on hand and the skill is yours.
    The parts slide together along their centre lines in about half a second, the
    item closes up whole, the parts are spent, and the item goes into the pack with
    a log line. An item with sockets comes out with every socket empty.
  - **The Ratchet build** is the first assembly: five new components, needs
    Scrapcraft (WRENCH's second skill), and D'SHAWN carries the set minus the barrel.
  - **Generic**: the drawing declares its parts (where each sits, which way it comes
    apart) the same way it declares sockets. The sheet reads only that, so any item
    can get parts later by declaring them.
  - **Size**: exactly the mock at 100×30. On bigger terminals the sheet grows and the
    drawing stays centred.
  - **Code**: the sheet lives in its own module with a small hook into the deck
    screen, so it merges cleanly next to the GEAR & INVENTORY rework in
    `worktree-gear-inventory`.

## Goals

### Game (`game` crate)

- **`Recipe.steps: Vec<String>`** (serde default empty): the process's steps, in
  order, each a few words. Content validation refuses more than 4 steps or a step
  longer than 16 characters.
- **`Action::Build(RecipeId)`**: builds one batch of a recipe. Every check is made
  before anything changes, each refusal a plain reason:
  - the recipe exists;
  - the character has the workbench (`has_workbench`);
  - the character has the recipe's skill: `needs <Skill name>`;
  - every input is in the pack in its quantity: `needs <n> more <input name>` for
    the first one short.
  Then the inputs are taken from the pack and the output is given with `give_item`,
  so an item held per copy (one with sockets) comes out as a new copy with every
  socket empty. The message is `built <name>` for one, `built <n>× <name>` for more.
- `handles_items` covers `Build`, so jacked in it is refused with `JACKED_IN` like
  every other item action.
- Content validation refuses a recipe whose input is held per copy (has sockets or
  is hardware): inputs are counted items. Recipes keep `can_craft`; the workbench
  reads it.
- **New items**, all `Component`. Weights and values are my calls; the names are the
  placeholders Jeffrey saw:
  - `ratchet_receiver` "Stamped receiver", 1.2 kg, ¤ 300. "The Ratchet's box:
    stamped steel, a serial number filed to fuzz."
  - `ratchet_stock` "Wire stock", 0.5 kg, ¤ 80. "Bent rod on a hinge. It folds, in
    theory."
  - `ratchet_barrel` "Barrel 9×19", 0.6 kg, ¤ 220. "A short tube with gold threads
    at the end, for whatever you screw on."
  - `ratchet_mag` "Magazine", 0.4 kg, ¤ 60. "Thirty rounds of 9×19 and a strip of
    label tape that says GOOD STUFF."
  - `ratchet_trigger` "Trigger group", 0.3 kg, ¤ 140. "Hammer, sear, spring, and a
    selector filed past safe."
- **New recipe** `ratchet_build` "Ratchet SMG", skill `scrapcraft`, inputs the five
  parts ×1 each in the order above, output `ratchet_smg` ×1, no steps, desc "Five
  parts and a pin punch. It only goes together one way."
- **Steps for the six recipes there today** (my calls, from their descriptions):
  - `wire_shiv`: "wrap the coil", "pull it taut"
  - `hardhat_visor`: "scrape the glass", "wire it in"
  - `caps_fence`: "count the caps", "haggle", "take the wire"
  - `stim_batch`: "boil", "filter", "pray"
  - `optics_tune`: "re-seat lenses", "kill boot logo"
  - `servo_rebuild`: "strip", "rewind", "seat"
- **D'SHAWN** (`debug_runner`) gets Scrapcraft and one each of the receiver, stock
  and trigger group, plus two magazines, and no barrel.

### The drawing's parts (`strata-zero`, `ui/art.rs`, `ui/looks`)

- **`PartPlace`**, a sibling of `SocketPlace`, in drawing coordinates:
  - `item`: the id of the part item it shows (`"ratchet_receiver"`);
  - `x, y, w, h`: the rectangle the part covers when the item is whole (for the
    pointer's brackets and the missing outline);
  - `apart: (i16, i16)`: how far it moves, in cells, on the exploded drawing;
  - `balloon: Dir`: which side of the moved part its balloon goes.
- **`Art::parts(&self) -> Vec<PartPlace>`**, default empty, and
  **`Art::draw_part(&self, index, origin: (i32, i32), clip, buf)`**, which draws that
  part alone with its own origin. The whole drawing (`Art::draw`) stays exactly as
  it is today.
- `ItemArt` answers both from its look; the Ratchet's live in `looks/ratchet.rs`
  next to `PLACES`. Today only `ItemLook::Ratchet` has parts. Decks and hardware
  keep the defaults (no parts), so their recipes, if any, get the process sheet.
- **The Ratchet's five parts.** The reference cut, in drawing coordinates (cells,
  origin at the drawing's top-left, inclusive ranges), with the mock's offsets:

  | # | item | covers (x, y) | apart | balloon |
  |---|------|---------------|-------|---------|
  | 1 | `ratchet_receiver` | x 21-59, y 3-8, with the rail and the knob | (0, 0) | up |
  | 2 | `ratchet_stock` | x 0-20, y 3-10, the butt pad, rods and mount | (−5, 0) | up |
  | 3 | `ratchet_barrel` | x 60-70, y 3-8, the barrel and its gold threads | (5, 0) | up |
  | 4 | `ratchet_mag` | x 34-48, y 9-14, the grip, magazine and GOOD STUFF tape | (0, 1) | left |
  | 5 | `ratchet_trigger` | x 49-60, y 9-13, the trigger and its guard | (0, 1) | right |

  The builder splits `ratchet::draw` into one drawing per part along these lines,
  not the rectangles cell by cell: each part's pixels and text go to that part, and
  a part drawn at offset zero lands exactly where the whole drawing has it. Where a
  pixel sits on a boundary (the stock's rods reach x 24, where the receiver starts)
  it goes to whichever part reads right. The table's ranges can shift by a cell or
  two to follow the drawing.
- Tests: every part place's item exists and is an input of a recipe whose output
  wears that look; part rectangles of one look don't overlap; every part drawn at
  offset zero over a cleared buffer, all together, equals `draw` cell for cell;
  single-width glyphs and exact colours, as the gun's drawing test checks today.

### The sheet (`strata-zero`, a new module, e.g. `screens/workbench.rs`)

One state struct the deck holds, the way it holds the TALENTS view: the drawing
shown, the part selected, and the assembling animation if one is running. It draws
the WORKBENCH tab's body and takes its keys. The deck screen only routes to it and
keeps the locked view it has today (no WRENCH: `▣ unlocks the workbench`).

**Drawings.** Every recipe in content, stably sorted by its skill's place in
`content.skills`, ties in content order. The deck remembers the drawing and the part
across closing, as it remembers tabs. ←→ wraps around.

**Layout at 100×30.** The deck's body is rows 5-26 and columns 1-98; row 27 is
the status strip and row 28 the hints, as on every tab. The approved mock drew the
sheet down to row 27, over the strip, so the sheet built here is one row shorter:
the parts list's header rides the rule, as GEAR's weight rides its rule (my call;
nothing else moves). `wb.py`'s `concept_a`, `concept_a_process`, `sheet`,
`parts_list` and `title_block` are the cell-exact reference with that change made.

- **The sheet**: a `PANEL_BG` field with a thin `RULE` border, columns 2-97, rows
  5-26. Zone marks like a drawing's: `┬` ticks on the top edge every 12 columns with
  `1 2 3 …` in `FAINT` between them, `├` ticks on the left edge every 4 rows with
  `A B C D` between them, down to the rule over the parts list.
- **The field**, rows 6-19: the exploded drawing or the process diagram.
- **A rule** on row 20, `├`/`┤` at the ends, `┬` at column 61, with the parts
  list's header set into it in `FAINT`: ` # ` at column 5, ` PART ` at 8, ` QTY ` at
  31, ` ON HAND ` at 37. Below it, columns 3-60 hold the parts list and columns
  62-96 the title block, a `│` at column 61 down to the bottom edge (`┴`).
- **Parts list**: one row per recipe input from row 21, five rows at 100×30: the
  cursor `▸`, the number, the item name, the quantity needed (column 33), the
  quantity in the pack (column 39), `■` (cyan) when enough or `□` (magenta) with
  `not in pack` when short (column 45, 48). A short row's text is magenta. The
  selected row has the `BAR_BG` bar and bold cyan text. More inputs than rows
  scroll.
- **Title block**, rows 21-25, each line clipped at column 96:
  1. the recipe name upper-cased, cyan bold, and `◂ i/n ▸` (DIM) at column 87;
  2. for an assembly, the output's kind, slot and weight (`weapon · primary · 3.2
     kg`); for a process, `makes <n>× <name> · <kind>`;
  3. `needs <skill> · yours` (DIM) or `needs <skill>` (magenta);
  4. `every part on hand` (green, `#5ac88c`), or `<n> more <name>` (magenta) when
     only one input is short, or `<k> of <m> parts on hand` (magenta);
  5. `DWG i/n · assembly · drawn <HANDLE>` or `… · process · …`, FAINT. (The mock
     first said `REV B`; a revision letter means nothing in the game yet, so the
     sheet says which kind of drawing it is.)
- **Hints**: `←→ drawings · ↑↓ parts · space assemble · tab tabs · esc close`, with
  `space build` on a process sheet.

**The exploded drawing** (when the output's drawing has parts):

- Each part drawn with `draw_part` at its home plus `apart`, the whole exploded
  bounding box centred in the field. Sockets are drawn empty: the output a build
  makes has nothing fitted.
- **Centre lines**: in each gap a part opened, a run of `╌` (horizontal moves) or
  `╎` (vertical moves) in `FAINT`, through the part's middle row or column.
- **Balloons**: ` n ` (three cells), bold, on the part's `balloon` side, one cell
  clear of the moved part, with a short leader (`│` or `─`, one to three cells) to
  it. A part on hand: cyan on `BAR_BG`. A part short: magenta on a dark magenta
  (`#2a0622`). The selected part's balloon is inverted (dark text on cyan or
  magenta).
- **A part short** is drawn as a ghost (the modding mode's grey, at about two
  thirds of its brightness, text cells blanked), inside a magenta `╌╎` dashed box
  around its rectangle, with `none on hand` under it.
- **The selected part** has the pulsing cyan corner brackets (`art::brackets`), one
  cell clear of the box.
- If the exploded drawing doesn't fit the field, it is drawn whole instead (centred,
  clipped); at 100×30 the Ratchet fits exploded.

**The process diagram** (when the output has no drawing with parts; `wb.py`'s
`concept_a_process` is the reference):

- The inputs as boxes stacked down the left of the field (3 rows each, 26 wide,
  the stack centred): the kind's icon, the name, `×n` at the right; a thin `RULE`
  box when on hand, a dashed magenta box with magenta text when short; the selected
  input's box in cyan. Each box has its balloon to its left. With more inputs than
  fit as boxes, they become one-line entries.
- Lines `─` from each box to a junction, `┐ │ ┘` down it, `├────▶` into the process.
- **The process**: a double-line `═║` box (DIM) with the skill's name upper-cased,
  cyan bold, set into its top edge, and the steps numbered inside (`1 boil`). Its
  width follows the longest step.
- `────▶` out to **the result**: a cyan box with the kind's icon and `<n>× <name>`,
  bold.
- A recipe with no steps shows the skill box empty.

**Assembling.** Space on a sheet performs `Action::Build`. A refusal goes to the
status strip and log as a warning (the action's reason). On success:

- **Exploded**: the parts close in over 15 ticks (half a second at 30 fps), each
  part's offset easing from `apart` to zero; then the item is drawn whole with a
  cyan `ASSEMBLED` stamp (and the outline brackets) for 30 ticks; then the sheet
  shows the new state (counts updated, parts now short drawn as ghosts). The
  animation draws the parts as they were when space was pressed.
- **Process**: the lines and the process box light cyan from the inputs to the
  result over 15 ticks, the result box holds a `BUILT` stamp for 30 ticks.
- Any key ends the animation early (and is not otherwise taken).
- The action's message goes to the status strip and log as other actions' do.

**Bigger terminals**: the sheet takes the whole body; the parts list and title block
keep their rows at the foot (the title block keeps its 35 columns at the right); the
field takes the rest, the drawing centred in it.

**Colours.** The mock's `FAINT` (`#5c6673`) is dimmer than the game's `FAINT`
(`#78828f`): it is the shade the GEAR & INVENTORY rework adds to `chrome.rs` as
`FAINTER`. To keep clear of that file, the sheet defines the shade as its own
constant for the zone marks, centre lines, the header in the rule and the DWG line,
and the two can be folded together after both merge (my call).

### Shots and docs

- A shot for each: `workbench_ratchet_100x30` (D'SHAWN, barrel missing, the barrel
  selected), `workbench_ratchet_whole_100x30` (a barrel given, ready),
  `workbench_stim_100x30` (a process sheet; D'SHAWN has no Medtech, so its skill
  line reads `needs Medtech` in magenta, unlike the mock), and the 120×40 Ratchet
  sheet. The Ratchet comes last in skill order, so its sheet reads `7/7`.
- README: the workbench paragraph rewritten for the drawing set.

## Non-goals

- Taking items apart into their parts.
- Where parts come from in the world (shops, loot, job rewards).
- Drawings or parts for any item but the Ratchet.
- Finding or learning recipes: every recipe in content stays known.
- Building from per-copy inputs (a deck into a rebuilt deck); validation refuses
  them for now.
- Building more than one batch per key press.

## Anti-goals

- Anything item-specific in the sheet: no `if ratchet`. The Ratchet gets its sheet
  only because its look declares parts.
- Wording written for one item: the title block, the status lines and the refusals
  come from kinds, names and counts.
- Touching the deck screen beyond the routing: the GEAR & INVENTORY rework in
  `worktree-gear-inventory` is reshaping `deck.rs`, so the workbench's code lives in
  its own module and the old `workbench_rows` and recipe card go away with as small
  a diff there as can be.

## Left to the builder

- How `ratchet::draw` is split into parts (one function per part, each drawing its
  own pixels and text, the whole drawing all of them), as long as offset zero is
  exact.
- The exact balloon and leader cells beyond the rules above; the mock's placements
  are the reference for the Ratchet.
- Whether the sheet's state lives in a new `screens/workbench.rs` or under `ui/`.
- Easing curve for the closing animation.
