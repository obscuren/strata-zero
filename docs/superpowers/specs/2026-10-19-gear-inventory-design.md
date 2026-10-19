# GEAR & INVENTORY: design

Date: 2026-10-19. Status: designed with Jeffrey in the visual companion (terminal-
accurate mockups at 100×30 over the game's own frame, palette and blueprint
drawings). He picked the rack and the icon filter: "Rack and icons, perfect and
EXACTLY what I mean". Worktree `.claude/worktrees/gear-inventory`, branch
`worktree-gear-inventory`, off local main a6f20df (the SMG sockets merge).
Builds on: `2026-10-18-cyberdeck-tab-design.md` (the blueprint window),
`2026-10-19-item-sockets-design.md` (copies, marks, the `Art` seam, the card),
`2026-10-19-smg-sockets-design.md` (item looks, the Ratchet).
Layout reference: `2026-10-19-gear-inventory-mock/` beside this file, the code that
drew the approved screens (`gen.py`, run as its docstring says). It is the reference
for every cell, colour and glyph below.

## Intent

GEAR and INVENTORY are two tabs that show two halves of one thing: what you wear and
what you carry. Moving a piece from one to the other means switching tabs, and the
only place an item is drawn is the blueprint window, one Inspect away. One tab puts
the gear, the pack and the selected item on one screen, with a small version of the
item's drawing next to its card.

## Decided with Jeffrey

- **One tab, `GEAR & INVENTORY`**, in place of GEAR and INVENTORY. The deck has eight
  tabs: SKILLS, STATS, GEAR & INVENTORY, WORKBENCH, JOBS, JOURNAL, MAP, TALENTS.
- **Three panels.** A (top left): the gear. B (top right): the pack. C (across the
  bottom): the selected item.
- **The arrow keys move between gear and pack.**
- **A is the rack** (picked over a ledger with each item's effects and a paper doll):
  the nine slots as tiles, three by three, in `Slot::ALL` order (HEAD EYES NEURAL /
  ARMS TORSO LEGS / PRIMARY SECONDARY DECK).
- **B has a filter row at the top for the item kinds the pack already groups by**
  (`ItemKind`). The **icons** row was picked over a two-row row of names.
- **C is the card the detail pane shows now, plus the item's drawing.**

## Panel A: the rack

- Each tile is 3 rows tall and a third of A's width (16 columns at 100×30). The
  **top edge** carries the slot's name (`┌ PRIMARY ──────┐`). **Inside**: the item's
  name, cut with `…` to fit, or `— empty —` in faint. The **bottom edge** carries the
  item's marks (program slots, the dot, sockets: the existing `marks`), set in from
  the left with a space either side (`└ ■■■■■ · ■□ ──┘`). An item with no marks
  leaves the edge plain.
- Edges are drawn in the rule colour. The selected tile: while the rack has focus,
  its edge is cyan, its label lit and bold, and its inner row on the cursor bar
  colour with the name lit and bold. When the pack has focus, the selected tile's
  edge goes dim and nothing else is lit.
- The rack is 9 rows tall at every terminal size. It sits at the top of A.
- **Keys on the rack**: `←↑↓→` (and `h j k l`) move between tiles and stop at the
  edges, no wrapping. `→` on the right column crosses to the pack. `←` on the left
  column does nothing.
- **space** on a tile opens the slot popup GEAR opens now (equip a fitting piece from
  the pack, Unequip, Inspect for the worn copy), unchanged, anchored beside the tile.

## Panel B: the pack

- **The filter row** is B's first row: ` ALL ` then one chip per `ItemKind` in
  `ItemKind::ALL` order, each the kind's glyph from `kind_icon` (`/ ▣ ◈ ⌗ ◍ ▪ · ⌁ ≡`).
  The active chip is bracketed like the active tab and names itself: `▌ALL▐`,
  `▌≡ HARDWARE▐` (glyph, space, the kind's label uppercased). The active chip is lit
  and bold while the pack has focus, dim when it does not. Other chips are dim, or
  faint when the pack holds none of that kind.
- **`[` and `]`** step the filter back and forward through ALL and the kinds the pack
  holds, wrapping. They work from either panel and do not move the focus.
- **The list** below the row is today's INVENTORY list (glyph, count, name, marks; a
  copy is a row of its own). Under ALL the kind headers (`── deck ──`) stay as now.
  Under one kind, the list shows only that kind and has no header.
- **The pack's weight and value**, now a footer under the list, move into the rule
  above C at its right end under B: ` 15.3 kg · ¤ 16909 `, dim.
- **Keys on the pack**: `↑↓` (and `j k`) move the rows, wrapping as now. `←` crosses
  back to the rack, onto the tile it was on. `→` does nothing.
- **space** on a row opens the item or copy verbs INVENTORY opens now (Inspect,
  Equip, Use, Drop one, Drop all, or the copy's Inspect, Equip, Drop), unchanged.

## Focus

- One panel has the focus. It shows the cursor bar on its selection, and a **cyan
  segment** (`━`) runs under it along the rule above C, the way the magenta segment
  marks the active tab. The segment slides between panels like the tab segment
  (same 6-tick tween and easing).
- The other panel keeps its selection, shown dim: a dim `▸` in the pack, a dim edge
  on the rack's tile.
- **C shows the selection of the focused panel.** Each panel keeps its own cursor.

## Panel C: the selected item

- **Left: the card**, 41 columns wide, starting 2 in from the frame. It is
  `item_card_of` as now, with two changes so it fits 12 rows: the kind (and slot or
  socket) and the weight and value share the second line
  (`weapon · primary · 3.9 kg · ¤ 2150`), and the abilities an item grants share one
  line (`grants Netscan · Arc trace`). The change is to the shared card, so the
  journal's keycard card and the deck blueprint's plate readout on its glass
  (`screens/cyberdeck.rs`, `Part::Plate`) read the same; on that five-row glass the
  shorter card scrolls less. *(My call.)* It types out on a selection change as the
  detail pane does now (`REVEAL_TICKS`).
- **An empty slot** shows the card it shows now (`— empty —` / `nothing in the head
  slot`) and an empty viewer.
- **Right: the viewer.** From column 46 to the frame, solid panel colour, corner
  ticks `┌ ┐ └ ┘` in the rule colour. The item's drawing is drawn at **half size**,
  centred. `space inspect` sits in faint at the bottom right, inside the bottom edge.
  space → Inspect opens the full-size blueprint as now.
- **No drawing yet**: a dashed box, 21×5, in faint (`╌ ╎` with `┌ ┐ └ ┘` corners),
  centred in the viewer. The kind's glyph sits in dim bold at its centre on the second
  row, and `MISSING BLUEPRINT` in faint on the fourth, the words the blueprint window
  uses.

### The half-size drawing

Generic over the `Art` trait, so every item with a drawing gets a preview, now and
later. Nothing is special-cased per item or per kind.

- **Which drawing.** The one the blueprint window would show for the same item, copy
  and worn state: the deck's art, hardware's large art, an item's look with its
  fitted pieces. Pull that choice out of `Blueprint::open` / `Inspect::art` into one
  function both use. The preview draws `Art::draw` (the item as it stands: no
  pointer, no readout).
- **All of the item, none of its cables.** Some drawings reach past `Art::size()`:
  the pawn deck's side cradles, with any hardware fitted in them, sit at x −4..0 and
  83..87 (`cradles` in `ui/cyberdeck/pawn.rs`). The preview includes that overhang,
  so a fitted Coldfin shows. `Art` gets a way to report a drawing's full extent,
  `size()` by default, and the deck reports its cradles. The player and Net cables
  are not drawn in the preview: they join the item to you or a terminal and are not
  part of it. *(My call.)*
- **How.** Draw the art at full size into an offscreen buffer filled with the panel
  colour. Read each cell back as two pixels, top then bottom: `▀` is fg over bg,
  `▄` bg over fg, `█` fg over fg, a space is bg over bg, and **any other glyph is
  its bg over its bg** (text drops out and keeps its backing). A pixel in the panel
  colour is empty. Shrink by two each way: each 2×2 block is empty if fewer than two
  of its pixels have a colour, else takes its **commonest** colour, ties going to
  the brighter (sum of RGB). Pad odd edges with empty pixels. Fold the result back
  into cells the way the drawings fold: equal halves are a blank cell in that colour,
  one empty half is a half block on the panel colour, else `▀` with the top as fg and
  the bottom as bg.
- **Sizes today**: the deck 92×22 with its cradles → 46×11, the Ratchet 88×15 → 44×8, the Hushcan
  72×14 → 36×7, the other hardware 52×18 → 26×9. A future drawing taller than
  the viewer is clipped to it, centred.

## The layout

- **At 100×30** (the reference screens): the deck's body is rows 5..26. A and B take
  rows 5..13 (the rack's nine). The rule above C is row 14. C is rows 15..26 (twelve).
  A is columns 1..48, the divider `║` is column 49, B is columns 50..98. The rule
  above C is `═` in the rule colour with `╩` where the divider meets it.
- **Larger terminals**: C stays 12 rows. The top panels take every row above it. The
  rack stays 9 rows at the top of A, and B's list gets the extra rows. A and B split
  the width evenly (the divider at the middle), and tiles widen to a third of A.
  *(My call; see below.)*
- The status strip and the hints stay where they are.
- **Faint** in this spec is the mock's `#5c6673`, darker than `chrome::FAINT`
  (`#78828f`): empty tiles, chips for kinds the pack holds none of, `space inspect`,
  the MISSING BLUEPRINT box. It joins the palette as its own constant, because the
  approved screens used it. *(My call.)*

## Tabs and keys

- `Tab::ALL` has eight entries. `1`–`8` jump to them by position, and `0` is the
  last tab (TALENTS) as now. `9` does nothing.
- On this tab `←→` belong to the panels, so tabs switch with `tab` / `shift-tab` and
  the digits, as on TALENTS. `h`/`l` follow the arrows.
- **Hints**: rack focused
  `←↑↓→ rack, then over to the pack · [ ] filter · space act · tab tabs · esc close`;
  pack focused `←→ gear/pack · ↑↓ select · [ ] filter · space act · tab tabs · esc close`.
- The deck reopens where it was closed: this tab remembers the focus, the rack's
  tile, the pack's row and the filter, cleared with the rest of the deck's memory
  on a city switch.

## Unchanged

The popups and their verbs, the blueprint window and modding, the marks, the
equip/use/drop rules, jacked-in hands-off, the slot popup's copy labels, every other
tab.

## Goals, non-goals, anti-goals

- **Goals**: gear and pack on one screen. Every item's drawing visible without
  opening a window. Built so a new slot, kind or drawing needs no change here.
- **Non-goals**: effect totals or per-slot effects on the rack (the ledger was not
  picked). A paper doll. Sorting, search, a comparison view. New drawings for items
  that have none. Changes to the blueprint window beyond the card's two compactions
  on the deck's glass.
- **Anti-goals**: nothing in this tab knows a specific item or kind by name (the
  preview takes any `Art`, the chips come from `ItemKind::ALL`, the tiles from
  `Slot::ALL`). The panels don't change size or move as the selection changes. Don't
  fold the merged tab into the shared list/detail drawing as extra branches: it is
  its own view with its own state, as MAP and TALENTS are.

## Docs and shots

- README: the deck line (`1`-`8`, `0` talents), the TALENTS line (tab `8` or `0`),
  the "Inspect it in GEAR" line, and the "In GEAR and INVENTORY" marks paragraph.
  Describe the tab, the rack, the filter and the preview.
- Shots (`screens/shots.rs`): replace `gear_and_inventory_marks` with this tab at
  100×30 and 120×40, rack focused on the Ratchet with both sockets filled, and the
  pack focused on HARDWARE with the Hushcan selected.

## My calls (Jeffrey to check)

- **Larger terminals** give the extra rows to the pack's list, and the rack stays 9
  rows. The alternative was a taller C with the same half-size drawing in more space.
- **The rack doesn't wrap** at its edges. The pack's list still wraps, as lists do now.
- **`←` from the pack** returns to the tile the rack was on, not the nearest one.
- **`[ ]` skip the kinds the pack holds none of.** If the active kind empties, it
  stays selected (an empty list) until you step away.
- **The card's two compactions** (one meta line, one grants line) apply everywhere
  the card shows, the deck blueprint's glass included.
- **The deck's preview includes its side cradles** and any hardware fitted in them
  (46×11), and no cables.
- **A darker faint**, as in the approved screens, joins the palette.
- **The preview uses the commonest-colour shrink**, which kept edges crisper than
  averaging in the mockups.
- **The focus segment slides** like the tab segment.

## Left to the builder

Names (the `Tab` variant, the view's type, the preview function), where the preview
lives (beside `Art` in `ui/art.rs` or its own module), the popup anchor arithmetic,
and how the remembered state is stored on `AppState`.
