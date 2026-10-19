# Item sockets and the modding mode: design

Date: 2026-10-19. Status: designed with Jeffrey in the visual companion (terminal-
accurate mockups over the game's own 100×30 shots), approved in chat; he asked for
spec, plan and build to run on without check-ins. Worktree `.claude/worktrees/sockets`,
branch `worktree-sockets`.
Builds on: `2026-10-18-cyberdeck-tab-design.md` (the blueprint window, the pawn look,
pointing with corner brackets, the cables), `2026-10-15-constructs-and-programs-design.md`
(programs, the trace), the jacked-in lock (`actions.rs`: `handles_items`, `jacked_in`,
`JACKED_IN`, commit 79f570e).

## Intent

Items get sockets: places on an item that hold a piece of extra hardware. On a
cyberdeck that is deck hardware; on other items, later, gear upgrades (a silencer on
an SMG). Not every item has sockets; later they will be rolled per copy. For now one
item has them: the Rebuilt pawn deck (`rebuilt_deck`), always two.

Hardware is real: it changes runs. And it is drawn: a fitted piece shows on the item
that holds it, as the piece it is, so the drawings of host and hardware must match.
Every item drawing therefore says where its sockets are and what each takes; every
piece of hardware says what it fits and how it looks there.

Fitting happens in a **modding mode** of the blueprint window, separate from the
item's own inspect mode, and generic: it works for any item with sockets and a
drawing, not just the deck. Jeffrey: "Make sure those modes are separated and not
hacked in … this should not only work for this item but for all items."

## Decided with Jeffrey

- **Per-copy items now** ("your pick"): anything that carries state of its own is
  held as a copy: which hardware sits in which socket, later its rolls. Fitted
  hardware is a copy too. Things with no state (programs, stims, junk) stay counted
  stacks. Copies never merge into a stack.
- **Hardware has real effects**, numbers are examples and later rolled:
  - **Coldfin vI**: each trace you make costs 10% less (a crack's 4 becomes 3.6).
  - **Coldfin vII** (the better one): 20% less.
  - **Sidecar co-processor**: program power +1 (+10% on the odds the RUN rows show).
  The trace keeps fractions under the hood; the meter and the HUD still read whole
  numbers.
- **Sockets on the pawn deck** are its two sides: socket L and socket R, an open
  cradle each: two steel rails standing off the side with gold contacts in the edge
  between them. Fitted hardware sits between the rails. The cable ports in the
  lower corners stay where they are and are renamed **jacks** in code and docs, so
  "socket" only ever means hardware.
- **Modding mode** opens from the blueprint window with `m`, only for items with
  sockets. In it the item's own parts (carts, glass, failsafe, ROM, WAREZ) cannot be
  pointed at; only the sockets can. The item is drawn in **grey** (Jeffrey picked
  grey over washed and blueprint-blue), the sockets and their hardware in colour.
  Space on a socket chooses what goes in from a HARDWARE tray of pack hardware that
  fits; on a filled socket the tray also offers *empty* to take the piece out, and
  choosing another piece swaps.
- **Wording is never written for one piece.** An empty socket says what it takes,
  from the socket's kind: `empty · takes deck hardware`. (Jeffrey: "We'll have more
  mods than those two so the wording should be generalized.")
- **The character pane marks sockets** after an item's name, worn or carried: `□`
  open for an empty socket, `■` closed for a filled one. **A deck also shows its
  program slots**, as darker squares, then a dot separator, then its sockets:
  `■■■■■ · ■□`.
- **Hardware has its own blueprint**, drawn large at the deck's level of detail.
  The Coldfin comes in two versions drawn differently, vI (fins) and vII (a fan over
  the fins, better); each draws differently on the deck too.
- **Meatspace only**: fitting and removing obey the jacked-in lock exactly as equip,
  drop and program slotting do.

## Goals

### Items and copies (`game`)

- `ItemKind::Hardware` (label `hardware`), drawn in lists with the glyph `≡`, grouped
  under `── hardware ──` after programs.
- `SocketKind` with one variant today, `Deck`. It reads `deck hardware` for what a
  socket takes and `deck socket` for what a piece fits. New kinds (a muzzle, a rail)
  are new variants later.
- `Item.sockets: Vec<SocketKind>` (serde default empty): the sockets a new copy of
  the item is made with. `rebuilt_deck` gets `sockets: [Deck, Deck]`.
- `Item.hardware: Option<Hardware>`, for hardware only: `Hardware { fits: SocketKind,
  mods: Vec<Mod>, look: HardwareLook }`.
- `Mod` with two variants: `TraceCut(u8)` (percent off each trace you make) and
  `Power(i8)` (added to every program turn's power). Each reads as a line:
  `trace per cast −10%`, `program power +1` (U+2212 minus).
- `HardwareLook { ColdfinI, ColdfinII, Sidecar }`: which drawings a piece wears, its
  large blueprint and how it looks in each socket kind it fits.
- `DeckLook::max_sockets()`: the pawn has room for 2.
- Content validation refuses: `Hardware` kind without `hardware` or the reverse; an
  item with sockets that has no drawing placing them (today only decks, through
  their look; more sockets than `max_sockets`).
- New content items (weights and values are calls, see the end):
  - `coldfin_1` "Coldfin vI", weight 0.4, value 800, `TraceCut(10)`, look
    `ColdfinI`. "Bolted-on fins and a copper pipe. The deck still runs hot, just
    less."
  - `coldfin_2` "Coldfin vII", weight 0.6, value 1600, `TraceCut(20)`, look
    `ColdfinII`. "The same fins with a fan screwed over them. Somebody voided a
    warranty for this."
  - `sidecar` "Sidecar co-processor", weight 0.2, value 900, `Power(1)`, look
    `Sidecar`. "A math board on gold fingers. Every turn it thinks a little ahead of
    you."
- **Copies.** An item is held per copy when it can carry state: it has sockets or it
  is hardware. A copy has a small id, unique for the character (a counter on the
  character), its item, and one entry per socket holding the fitted hardware copy or
  nothing. Everything else stays a `Stack { item, qty }`. Copies never merge; each is
  its own row everywhere. Worn items are held the same way as carried ones, so a
  worn deck keeps its hardware when you take it off and put it on again.
- **Every action that names a pack item names a copy when the item is held per
  copy** (equip, drop, use where it applies, take from the ground). The ground holds
  copies too: a dropped deck lies there with its hardware and comes back with it.
- **Fitting and removing are actions** (fit a hardware copy from the pack into socket
  *n* of a host copy, worn or in the pack; take socket *n*'s piece back to the pack;
  fitting into a filled socket swaps, the old piece to the pack). Every check is made
  before anything changes: the host and socket exist, the piece is in the pack, it
  fits the socket's kind. `handles_items` covers them, so jacked in they are refused
  with `JACKED_IN` like every other item action.
- **Weight and value** of a copy include what is fitted in it.
- **Effects come from the worn deck only.** The hardware fitted in the deck you wear
  counts; a deck in the pack does nothing. Same mods add up; the trace cut is capped
  at 50%.

### The trace with fractions

- Every trace you make yourself is cut by the worn deck's trace cut: program turns
  and their slips, the held mask's turn noise, ping, arc trace. A white ICE hit's
  heat is not your trace and is not cut. Deep dive's silent copy stays silent.
- The cut works in tenths of a point: a cast's noise times (100 − cut) percent, in
  tenths; whole points land on the construct's heat, the remainder carries to that
  construct's next trace. The alarm and dump lines, the HUD bar, the deck's TRACE
  meter and every readout keep using the whole points.
- With no hardware (cut 0) every number is exactly what it is today.

### Power

- `run::power` adds the worn deck's `Power` mods. The RUN rows' odds and every roll
  read through it, so the Sidecar shows on the odds without new UI.

### The pawn drawing's sockets

Drawing coordinates as in the cyberdeck spec (84×22, origin top-left; cells outside
0..84 are drawn as the cables are, clipped to the window). Socket *n* = 0 is L, the
left side; 1 is R, the right side.

- **The cradle** (always drawn on an item with sockets, empty or not): steel rails
  on rows 6 and 12, five cells out from the edge: L cols −4..0, R cols 83..87; `▄` on
  row 6 and `▀` on row 12 in steel `#8b8596`, the outer end in `#c8c2d2`. The
  connector in the edge on rows 8-10: L col 1 `▌`, R col 82 `▐`, gold `#c9a227` on
  well `#0c0c10`.
- **Fitted hardware** fills rows 7-11 between the rails, five columns (L −4..0, R
  83..87), drawn by the piece's look for that side; see *Art reference*.
- **A socket's rectangle** for the pointer is the rails and the connector: L
  `(−4, 6, 6, 7)`, R `(82, 6, 6, 7)`. Its brackets land one cell outside: on L at
  columns −5 and 2, which at 100×30 is the window's first inside column; it fits.
- **Inspect mode does not point at sockets.** It draws them, empty or fitted, and
  keeps every stop it has today. The maker plate's glass card, which is the item
  card, gains the socket lines (see *Cards*).
- The jacks (old "sockets" in `pawn.rs`: `SOCKET_Y`, the comments, the test names,
  the `DeckView` docs) are renamed jacks. Their drawing does not change.

### The modding mode

A mode of the blueprint window, beside inspect. The window owns it; an item only
supplies its drawing and its socket places (rectangle, side, kind). Any item with
sockets and a drawing gets the mode with no code of its own.

- **Way in and out.** In inspect, on an item whose copy has sockets, `m` switches to
  modding and the hint line says so: `↑↓←→ point · space act · m mod · esc close`.
  In modding `m` switches back. `m` does nothing while a cart is held, or on an item
  without sockets (whose hint never shows it). The window's state for each mode is
  its own; switching does not reset the other's pointer.
- **Title**: ` MODDING · <ITEM NAME IN CAPITALS> ` in modding, ` BLUEPRINT · … ` in
  inspect.
- **The drawing in grey.** The item draws at the same place as in inspect (no jump
  when switching). Every cell it paints, cables included, is turned grey except the
  socket rectangles: every fg colour, and every bg colour that is not the window's
  own panel background, becomes `v = 0.299r + 0.587g + 0.114b` scaled by 0.38, as
  `(v, v, v·1.08)`, rounded and clamped (so the rust edge and the cables turn grey
  too). Bold is dropped. The deck's glass is blank in
  modding (its background stays, no text, no pinned line, no scroll marks); WAREZ,
  the carried cart and the inspect brackets are not drawn.
- **Stops**: the sockets only, in their order; the arrows move to the nearest in that
  direction, by the same rule the deck uses. The pointer starts on the first
  socket. Brackets as in inspect, in cyan, pulsing.
- **The readout**: one line on the row above the status row, starting at the inside's
  second column, in runs:
  - empty socket: `SOCKET L` bold cyan `#00e5ff`, ` ▸ empty` white `#e8e8ec`,
    ` · takes deck hardware` dim `#a0aab9`;
  - filled socket: `SOCKET L`, ` ▸ Coldfin vII`, ` · trace per cast −20%` (each mod
    of the piece, joined with ` · `).
- **Space on a socket: choose.** The socket gets a dashed cyan outline one cell
  outside its rectangle (`┌╌┐`, `╎`, `└╌┘`) and the brackets drop into the tray. The
  hint reads `←→ hardware · space fit · esc back`. ←→ and h/l move along the
  tray; ↑↓ and j/k do nothing. The readout reads the pointed tile: `COLDFIN VII` bold cyan,
  ` ▸ socket L` white, ` · trace per cast −20%` dim; when fitting it would send a
  piece back, ` · Coldfin vI back to the pack` follows. Space fits (or swaps) and the
  brackets go back to the socket; esc goes back to the socket with nothing changed.
- **The HARDWARE tray**: a band of five rows above the readout: a bracket row, two
  tile rows, a bracket row (the fourth row of the band is the bottom bracket row,
  the readout is the fifth). `HARDWARE` bold in `#6b6575` at the drawing's column 2
  on the first tile row; tiles start 11 columns later, 3 apart. It holds the pack's
  hardware whose kind fits any socket of this item, in pack order. A tile is a
  two-cell swatch (`▀▀` in the piece's swatch colours, both rows), then one space
  and the piece's name in capitals bold on the first row, its first mod line on the
  second, padded to the longer plus one space each side, on the piece's tile colours.
  While choosing for a filled socket, the first tile is `EMPTY` / `take it out` (bg
  `#2a2830`, ink `#a0aab9`). With nothing that fits, dim `nothing in your pack fits`
  follows the label. When the tiles do not fit the band's width the row scrolls to
  keep the pointed tile in view with `‹` / `›` at the ends, as WAREZ does.
- **What happens** goes to the status row in the log's register: `Coldfin vII fitted
  · socket L`, `Coldfin vII fitted · socket L · Coldfin vI to the pack`, `Coldfin vI
  out · to the pack`. Refusals in magenta, nothing changed.
- **Keys**: the hint in modding is `↑↓←→ socket · space choose · m inspect · esc
  close`. Esc (or `q`) while choosing goes back to the socket, else closes the window.
- **Worn or carried**: modding works on any copy in GEAR or INVENTORY.
- **Jacked in**: `m` still opens modding to look; space refuses with the jacked-in
  line in the status row and nothing changes.

### The character pane

- **Marks after the name** in GEAR and INVENTORY rows, one space after the name:
  - a deck's program slots as darker squares: `■` used `#3f7c87`, `□` free
    `#4f5864` (on the selected row `#1a9aaa` / `#2a7480`). Used is the loaded
    programs' total size on the worn deck; a deck in the pack reads all free.
  - if the item also has sockets, ` · ` (`#a0aab9`, selected `#00a5b8`);
  - its sockets: `■` filled `#00e5ff`, `□` empty `#a0aab9` (selected `#00e5ff` /
    `#00a5b8`).
  So the worn rebuilt deck with Backdoor (size 2) and three size-1 programs loaded
  and a Coldfin in socket L reads `■■■■■ · ■□`; the pawnshop deck in the pack reads
  `□□`.
- **The marks are never cut**; when name and marks do not fit the list pane, the
  name is cut with `…`. At 100×30 GEAR shows `Rebuilt pawn… ■■■■■ · ■□`.
- Copies list one per row with `1×`.

### Cards

- **An item with sockets**: after its grants, one line per socket in order: filled
  `■ Coldfin vII · trace per cast −20%` in cyan (the piece's name and each mod line);
  empty `□ empty socket · deck hardware` in dim. The same card reads on the deck's
  glass from the maker plate.
- **Hardware**: second line `hardware · deck socket`; its mods as perk lines in cyan;
  then its text. A copy's lines come from the copy (ready for rolled numbers).

### Hardware blueprints

Inspect on a hardware copy opens the blueprint window on its large drawing: 52×18,
centred in the inside like the deck, hint `esc close`, no `m` (hardware has no
sockets). Each piece prints its numbers on itself (a sticker, the silkscreen), taken
from its mods. See *Art reference*.

### The debug runner

D'SHAWN's rebuilt deck is a copy with both sockets empty; his pack holds a Coldfin
vI, a Coldfin vII and a Sidecar co-processor, so modding can be tried at once.
Vex and every other start get no hardware.

## Non-goals

- Rolled sockets or rolled numbers; sockets on any item but the rebuilt deck.
- Where hardware comes from in play (shops, loot, crafting).
- Socket kinds other than `Deck`; sockets on other deck looks.
- Programs loaded per deck copy: loading stays on the character's worn deck.
- Saving characters (the game saves cities only).
- Animation beyond the pulsing brackets; mouse.
- The jack-out penalty (still owed elsewhere).

## Anti-goals

- Modding hacked into the deck's part list, or any deck-only code in the modding
  mode: if the mode needs to know it is looking at a deck, the split is wrong.
- Text that names or lists particular pieces where it should name a socket kind.
- Anything that does not fit 100×30, double-width glyphs, or colours that are not
  exact RGB.

## Architecture

The names here are suggestions; the shape is the point.

- `game::model`: `ItemKind::Hardware`, `SocketKind`, `Mod`, `HardwareLook`,
  `Hardware`, `Item.sockets`, `Item.hardware`, `DeckLook::max_sockets`. Validation
  in `game::content`.
- `game::character`: the copy type (`ItemCopy { id, item, fitted }` with `CopyId`)
  and where copies are held (pack and worn slots), the copy counter, `give` making
  copies for stateful items, `equipped(slot) -> Option<&ItemId>` kept for its many
  readers; weight and value through copies; `deck_mods` / `trace_cut` / `deck_power`
  from the worn deck. `game::ground` holds copies beside stacks.
- `game::action`: fit and unfit actions, copy-aware equip/drop/take; refusals before
  any change.
- `game::run`: `power` adds `deck_power`; `Trace` carries tenths. `strata-zero::net`:
  the player's own trace goes through one path that applies the cut
  (`noise_on`), ICE's through one that does not.
- `ui/blueprint` (or wherever the window's drawing lives): an item **art** seam the
  window calls: its size, draw it (inspect or grey), its socket places (rect, side,
  kind). The pawn look implements it for decks (the cradles, the fitted pieces by
  their look, the jacks renamed); hardware looks implement it for their large
  drawings (no sockets). `ui/hardware` (new): each `HardwareLook`'s large drawing,
  its side drawing for a deck socket (defined for one side, as the *Art reference*
  gives it, and mirrored for the other), its tray tile colours.
- `screens/blueprint.rs`: `Blueprint { mode: Inspect | Mod }` with the inspect state
  (today's `DeckTab` for decks, nothing for others) and a modding bench (pointer,
  choosing, tray scroll), generic over the art seam. The grey toning is a buffer pass
  over the art's area minus the socket rectangles.
- `screens/deck.rs`: rows and actions on copies; the marks; the cards' socket and
  hardware lines; the hardware group and glyph (`render/glyphs.rs`).
- Copies also reach the ground's readers (`look.rs`, `interact.rs`, `render/items.rs`,
  the sidebar), `jobs/ui.rs` (`remove_from_pack`), the console's give and drop, the
  GEAR equip picker and the tests that write `character.equipment` directly. Each
  copy is its own row (`1×`) wherever items are listed.
- `actions.rs`: `handles_items` covers fit and unfit.
- Content: the three pieces, the deck's sockets, D'SHAWN's kit.
- Shots: ignored tests drawing modding mode, the hardware blueprints and GEAR /
  INVENTORY with marks at 100×30, as the deck's shots do.
- README: the modding mode, hardware, the marks.

## Testing

- **Content**: the rebuilt deck has two `Deck` sockets; validation refuses hardware
  without `hardware`, `hardware` on a non-hardware item, a pawn deck with three
  sockets, sockets on an item with no drawing for them.
- **Copies**: giving a stateful item makes copies with fresh ids and empty sockets;
  stateless items still stack; two rebuilt decks are two rows; equip and unequip
  keep the fitted hardware; drop and take keep it; weight includes fitted pieces.
- **Fit and unfit**: fitting moves the piece from the pack into the socket; into a
  filled socket swaps and the old piece is in the pack; removing returns it; wrong
  kind, missing socket, missing piece each refuse with nothing changed; works on a
  carried deck; jacked in every fit or unfit is refused with `JACKED_IN` and the
  character is unchanged.
- **Effects**: no hardware, the trace climbs exactly as today; with a Coldfin vI a
  crack turn (noise 4) adds 3.6: heat +3 then the carried tenths land on a later
  turn (36 + 36 = 72 tenths over two turns: +3, +4); vII cuts 20%; two pieces add up;
  the cut caps at 50%; a white ICE hit's +5 is not cut; deep dive stays 0; a deck in
  the pack does nothing; the Sidecar adds 1 to `power` and the chance moves by 10.
- **Pawn drawing**: an item with sockets draws both cradles (rails, gold connector);
  a fitted Coldfin vI/vII/Sidecar draws its side art on the right side and mirrored
  on the left; the jacks still draw where they did; at 100×30 nothing is clipped and
  the L brackets fall inside the frame.
- **Modding mode**: `m` on the rebuilt deck switches the title and hint; `m` on the
  pawnshop deck and on hardware does nothing and the hint never shows it; stops are
  the sockets only; space opens the tray with only fitting pieces; ←→ moves along it;
  space fits and the status says so; esc while choosing changes nothing; on a filled
  socket the first tile is EMPTY and choosing it takes the piece out; swap sends the
  old piece back; the drawing outside the sockets is grey (a known cell's colour
  checked against the formula) and the glass shows no text; jacked in, space warns
  and nothing changes; carried deck works.
- **Pane**: GEAR row of the worn rebuilt deck ends in `■■■■■ · ■□` with the name cut
  at 100×30; pawnshop deck in the pack `□□`; a hardware row under `── hardware ──`
  with `≡`; the deck card lists `■ …` and `□ empty socket · deck hardware`; a hardware
  card reads `hardware · deck socket` and its mod line.
- **Hardware blueprints**: each look draws without clipping at 100×30, prints its
  numbers, hint `esc close`.

## Calls made in writing this (for Jeffrey to check)

- Item weights and values: vI 0.4 kg ¤800, vII 0.6 kg ¤1600, Sidecar 0.2 kg ¤900.
- Same mods add up; the trace cut caps at 50%.
- "Each cast" is every trace you make yourself (program turns and slips, the mask's
  held noise, ping, arc trace); ICE hits are not cut.
- Fitted hardware counts toward weight and value; a dropped item takes its hardware.
- Modding works on carried items too (unlike programs, which need the worn deck).
- Jacked in, modding mode opens to look; changes are refused.
- Hardware is held per copy even before rolls, so rolls slot in later.
- The vII's side art is a fan housing with two small fans; the fin version is vI's.

## Art reference

Colours not listed are the pawn look's (`pawn.rs`). Half-block cells are given as
`ch fg/bg`. The mockups' Python is in this session's scratchpad (`mock/sockets.py`,
`mock/hardware.py`, `mock/modmode.py`, `mock/lists.py`); these tables carry what the
build needs.

Colours: steel `#8b8596`, steel hi `#c8c2d2`, gold `#c9a227`, gold hi `#e8c54a`,
well `#0c0c10`, aluminium `#b4bec8`, aluminium mid `#7d8893`, aluminium dark
`#2b3138`, aluminium shade `#5d6772`, fin end `#dfe6ec`, copper `#d07a3a`, frost
`#8fd8ff`, ink `#1a1612` (hardware stickers `#1a1e24` on the tray), fan frame
`#3a424b`, blade `#8a96a2`, blade hi `#c3ccd5`, gap `#0e1114`, hub `#5d6772`, void
red `#b0203c`, PCB `#1e5a34`, PCB dark `#123b22`, silk `#d8f0de`, trace line
`#3f8f5c`, chip `#141218`, chip text `#cfd6dd`, chip dim `#6b7480`, silver `#9aa4ae`,
LED `#50e678`, tan `#c8b48a`, hazard `#ffcc00`, black `#000000`.

### Side drawings (deck socket), left side; mirror for the right

Columns from the outer end (−4) to the inner one (0), rows 7-11. Mirroring swaps the
column order and `▌`↔`▐`.

- **Coldfin vI**: cols −4..−1 every row `▀` aluminium/aluminium dark (a fin above a
  gap); row 9 `▀` aluminium/copper (the pipe). Col 0 rows 7-11 `█` aluminium mid,
  row 9 `█` copper (the base plate).
- **Coldfin vII**: cols −4..−1 as half-block pixels, 4 wide × 10 high, rows top to
  bottom: `F F F F`, `F B G F`, `F H H F`, `F G B F`, `F F F F`, `F F F F`, `F B G F`,
  `F H H F`, `F G B F`, `F F F F` (F fan frame, B blade, H blade hi, G gap): two fans
  in one housing. Col 0 as vI's base plate.
- **Sidecar** (drawn for the right side; mirror for the left): cols 83..87 rows 7-11
  PCB. Col 83 rows 8-10 `▌` gold/PCB (fingers). Cols 84-86: row 7 `▄` silver/PCB,
  rows 8-9 chip, row 10 `▀` silver/PCB (the chip's legs); (84, 8) `◦` chip text.
  (87, 7) `◦` PCB dark; (87, 8) and (87, 9) `┊` trace line; (84, 11) `▪` tan; (85, 11)
  `╌` trace line; (87, 11) `●` LED.

### Large drawings, 52×18

- **Coldfin vI**: base plate aluminium mid over cols 4-49, rows 1-16, with `▄`/`▀`
  edges on rows 0 and 17. Clamp cols 0-3 rows 5-12 aluminium shade (`▄` row 4, `▀`
  row 13), gold contacts col 0 rows 6-11 `▌` gold/aluminium dark, `◦` steel hi at
  (2, 6) and (2, 11). Fins cols 8-47 rows 2-15 `▀` aluminium/aluminium dark, col 47
  `▀` fin end/aluminium dark. Two copper pipes on rows 4/7 and 10/13: on those rows
  `▀` aluminium/copper from col 8 to 45 and solid copper cols 4-7; col 45 between
  them `▀` aluminium/copper. Screws `•` aluminium dark at (5,1), (48,1), (5,16),
  (48,16). (8, 1) `▲ AIRFLOW` aluminium dark on aluminium mid. (9, 16) ` COLDFIN vI `
  bold ink on aluminium mid; (31, 16) ` TRACE −10% ` bold ink on frost (the number
  from the piece's mod).
- **Coldfin vII**: the same plate, clamp, fins and screws; one copper U pipe on rows
  3 and 14 (`▀` aluminium/copper cols 8-46, solid copper 4-7) with its bend col 46
  rows 3-14 `▐` copper. A fan over the fins at (22, 5): 16×16 half-block pixels: for
  each pixel, `c = 7.5`, `R = 7.4`, `r` the distance from the centre, `a` its angle:
  `r > R` fan frame; `r > R − 0.9` gap; `r < 1.0` blade hi; `r < 2.2` hub; else
  `t = (a·5/2π + r·0.11) mod 1`, blade hi under 0.12, blade under 0.5, gap above;
  steel hi screws at pixels (1,1), (14,1), (1,14), (14,14). (8,16) ` COLDFIN vII ` and
  (36,16) ` TRACE −20% ` as vI's; (41,1) `VOID` bold `#e8e8ec` on void red; (8,1)
  `▲ AIRFLOW`.
- **Sidecar co-processor**: PCB cols 2-51 rows 1-16, `▄`/`▀` edges rows 0 and 17.
  Gold fingers cols 0-1 rows 3-14 `▀` gold/PCB dark (pads with gaps), cols 0-1 rows 2
  and 15 PCB. Mounting holes `◦` gold hi at (4,1), (49,1), (4,16), (49,16). Traces `─`
  trace line rows 4, 6, 8, 10, 12 cols 2-13, `┐` (14,4), `┘` (14,12). Chip 16×7 at
  (17,5) chip colour; legs `▄` silver on row 4 and `▀` on row 12 every other column
  from 18; `▐` silver col 16 and `▌` col 33 rows 5-11; `◦` chip text at (18,5);
  `SDC-7` bold chip text at (21,7); `x4 · 0617` chip dim at (20,9). Crystal can cols
  38-42 rows 4-5 silver with `▗▖▝▘` corners and `▄`/`▀` edges. Caps `▪` tan at (37,8),
  (39,8), (41,8), (37,10), (39,10); `▪` black at (44,8), (44,10), (46,9). `─` trace
  line row 12 cols 35-46, `╴` at (34,12). `●` LED at (48,12), `ACT` silk at (46,13).
  `SIDECAR rev.B` bold silk at (17,14), `· POWER +1` silk at (31,14) (the number from
  the mod). ` QC ` bold black on hazard at (44,2).

### Tray tiles

| piece | tile bg | ink | swatch fg / bg |
|---|---|---|---|
| Coldfin vI, vII | `#b4bec8` | `#1a1e24` | `#b4bec8` / `#2b3138` |
| Sidecar | `#1e5a34` | `#d8f0de` | `#c9a227` / `#1e5a34` |
| EMPTY | `#2a2830` | `#a0aab9` | `#4a4652` / `#2a2830` |
