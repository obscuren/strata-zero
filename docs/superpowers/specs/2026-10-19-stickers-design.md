# Stickers: design

Date: 2026-10-19. Status: designed with Jeffrey in the visual companion (terminal-
accurate mockups over the game's own 100×30 blueprint window). Worktree
`.claude/worktrees/stickers`, branch `worktree-stickers`, off local main a6f20df.
Builds on: `2026-10-19-item-sockets-design.md` (copies, `Host`, the `Art` seam, the
modding mode, `JACKED_IN`), `2026-10-19-smg-sockets-design.md` (`ItemLook`, `ItemArt`,
`ui/pixels.rs`), `2026-10-18-cyberdeck-tab-design.md` (the blueprint window, the pawn
look, the handle tag).
Art reference: `2026-10-19-stickers-art.py` beside this file, the code that drew the
approved mockups. It is the truth for every sticker's pixels, glyphs and colours, the
turn tables, the sticker areas and the tray's layout. Run it to print every sticker at
every turn.

## Intent

Stickers are cosmetic decals the player finds and sticks on their gear. Nothing is
gained: no stat, no value, no effect anywhere. It is customisation, and the fun of it
is the same as with real vinyl: you choose where it goes, you can turn it, and once
it is down it stays until you tear it off.

Any item that can be worn and has a drawing takes stickers (today the two pawn decks
and the Ratchet SMG). Stickers are items of their own, each with a small graphic,
carried in the pack. A third mode in the blueprint window, beside inspect and
modding, sticks them on. The pawn deck's two painted stickers, NO FUTURE and x‿x,
become real stickers on every pawn deck's copy.

Jeffrey's standing bar from the sockets work holds here: the sticker mode is a
separate mode with its own state, generic over the `Art` seam ("this should not only
work for this item but for all items"); each drawing defines where stickers may go;
wording comes from kinds, never from the current examples.

## Decided with Jeffrey

- **Turning lettering (option A, "stacked letters").** Picked over "lettering stays
  level" and "true size, unreadable sideways". A lettered sticker turned a quarter
  stands up: one cell of its text a row, letters upright, reading top to bottom at
  90° and bottom to top at 270°. A half turn reads upside down with turned letters.
  Symbols with a true turned form use it (x‿x → x(x → x⁀x → x)x). Stood up, a word
  is twice as long as it is level, so it only fits where the drawing has the height.
  Pictures (half-block pixels, which are square) turn exactly, width and height
  swapped.
- **The test pile** ("a whole bunch … cyberpunk all the way! … Maybe add orodyne in
  there as well"; the pile was then approved: "Perfect"): 15 lettered and 14 pictures,
  listed below and drawn in the art reference. ORODYNE stays as drawn (Jeffrey was
  told it is CD Projekt's and R. Talsorian's trademark and the repo is going public;
  the in-world ARASU·TEK is in the pile too).
- **Where stickers come from**: proposed (on the ground in districts' hangouts, a
  fixer's tip on a job) and accepted as the direction, then: "We can later determine
  how we will distribute them … Add those stickers to D'SHAWN as well for testing."
  This build places none in the world; D'SHAWN carries the pile and the console's
  `spawn` hands them out.
- **Peeling destroys**: "destroyed. i agree". While a sticker is being placed it can
  be moved and turned freely; once space sticks it, it stays until peeled, and
  peeling tears it: it is gone, not back in the pack.
- **What a sticker may cover** (approved: "No LETS GOOO" to "what would you change"):
  - Working parts stay clear: anything the inspect pointer stops on (carts, the maker
    plate, ROM strips, the failsafe), the readouts (the glass, TRACE, the lights),
    sockets and what is fitted in them, jacks and cables. On the gun, the barrel, the
    wire stock, the trigger guard and the ejection port.
  - The rest of the body takes stickers, the deck's keys and the gun's stamping
    included. Each drawing defines its area.
  - A sticker lies wholly on the area: it never hangs off an edge or over a part.
  - Stickers overlap, newest on top.
  - The handle tag is paint: it keeps a fixed spot of its own (where it is today, no
    longer tied to NO FUTURE) and stickers may cover it.
- **The mode's look** (option A, "the tray rises", picked over modding-style two-row
  tiles): the drawing greys outside its sticker area as modding greys all but the
  sockets; stickers stay in colour. While you choose, the tray rises over the
  drawing's foot and draws every sticker at its real size with its count under it;
  pick one and the tray sinks to the readout and the sticker appears on the item to
  place. The placing and peeling screens were shown and not changed.
- **Factory stickers** (proposed in the same message as the tray, not objected to):
  every pawn deck, the pawnshop deck and the rebuilt one, is made with NO FUTURE and
  x‿x stuck where the paint is today; they peel like any other.
- **Per copy** (same): every item that takes stickers is held per copy, the pawnshop
  deck included.
- **Meatspace only, like modding** (the brief).
- **Keys** (the brief): `s` opens the mode; in it the arrows move the sticker within
  the area, `r` turns it 90° clockwise, space sticks it, esc backs out.

## Goals

### Sticker items (`game::model`, `game::content`)

- `ItemKind::Sticker` (label `sticker`), added to `ItemKind::ALL`.
- `Item.sticker: Option<StickerArt>` (serde default), the graphic:
  - `StickerArt::Lettered { text: String, ink: (u8, u8, u8), ground: (u8, u8, u8),
    bold: bool }`. The text carries its own margin (a space at each end), as
    ` NO FUTURE ` does today.
  - `StickerArt::Picture { rows: Vec<String>, colours: BTreeMap<char, (u8, u8, u8)> }`,
    one char per pixel, `.` empty. Die-cut edges are baked into the rows (the art
    reference's `diecut` produced them; content holds its output).
- `Item.stickers: Vec<Stuck>` (serde default): the stickers a new copy is made with,
  like `sockets`. `Stuck { sticker: ItemId, x: u16, y: u16, turn: Turn }` in drawing
  cells, `x, y` the sticker's top-left cell as turned. `Turn` is `Level`, `Quarter`
  (90° clockwise), `Half`, `ThreeQuarter`, with `cw()` and serde default `Level`.
- `Item::takes_stickers()`: `slot.is_some()` and the item has a drawing (a deck look
  or an item look). Today: `pawnshop_deck`, `rebuilt_deck`, `ratchet_smg`.
- `Item::per_copy()` gains `|| self.takes_stickers()`.
- The 29 stickers in `flavor/content.ron`, ids, names and descriptions as in the art
  reference (`sticker_no_future` "No Future", `sticker_dead_face` "Dead face", …),
  each `kind: Sticker`, no slot, weight 0.0, value 5. The two pawn decks get
  `stickers: [(sticker: "sticker_no_future", x: 50, y: 10), (sticker:
  "sticker_dead_face", x: 45, y: 13)]`.
- Validation (`Content::validate`):
  - `kind: Sticker` exactly when `sticker` is set; a sticker has no slot, sockets,
    hardware, deck, program or stickers of its own.
  - Lettered: 1 to 18 chars. Picture: 1 to 14 pixels wide, 1 to 12 tall, rows of one
    length, every non-`.` char in `colours`, at least one pixel set.
  - `stickers` only on an item that `takes_stickers`, each naming a sticker item.
  - Single-width glyphs are checked where `unicode-width` lives (a `strata-zero` test
    over every lettered sticker at every turn), not in `game`.

### Copies carry stickers (`game::character`)

- `ItemCopy.stickers: Vec<Stuck>`, oldest first, drawn in that order (newest on top).
- `new_copy` copies the item's factory `stickers` in.
- Stuck stickers add no weight and no value (`fitted_weight`/`fitted_value` unchanged).
- Copies keep their stickers through wear, take off, drop, take and the ground, as
  they keep fitted pieces. The character is not saved in `.city` files, so there is no
  format change.

### Stick and Peel (`game::action`, `strata-zero::actions`)

- `Action::Stick { host: Host, sticker: ItemId, x: u16, y: u16, turn: Turn }`: takes
  one of the sticker from a pack stack without a copy and pushes a `Stuck` onto the
  host copy. Refusals, all before any change: the host has no copy or its item does
  not take stickers ("takes no stickers"), the sticker is not in the pack ("not in
  your pack"), the item is not a sticker ("`X` is not a sticker"). Message:
  "`No Future` stuck".
- `Action::Peel { host: Host, index: usize }`: removes that stuck sticker from the
  host copy; it is destroyed. Refusal: "no such sticker". Message: "`No Future`
  peeled · it tore".
- `game` stays free of geometry: `Stick` does not check the area. The window only
  ever sends spots where the sticker fits, and a drawing clips anything that lands
  outside it (a sticker stuck before a drawing changed).
- Both are in `handles_items`, so both are refused jacked in with `JACKED_IN`.

### Turning (`strata-zero::ui::stickers`, new)

- `cells(art: &StickerArt, turn: Turn) -> Vec<(u16, u16, Cell)>`, the art
  reference's `Lettered.cells` and `Picture.cells` exactly: a lettered sticker's text
  level, reversed and half-turned, or stacked three wide at a quarter (top to bottom
  at 90°, bottom to top at 270°, the middle cell the glyph upright or quarter-turned,
  ground either side); a picture's grid turned, folded as `ui::pixels` folds (an
  empty half lets the cell under it show).
- `TURN180` and `TURN90`, the art reference's tables. A glyph with no entry turns to
  itself. A test asserts every glyph in both tables is one cell wide.
- `size(art, turn)` and `draw(stuck, origin, pen)`, which every drawing calls to put
  its copy's stickers on.

### The `Art` seam (`ui::art`)

- `fn sticker_area(&self) -> Option<StickerArea>`, default `None`: the drawing cells
  where stickers may lie; `None` for a drawing that takes none (hardware).
  `StickerArea` is a cell mask over the drawing, built from kept rects less dropped
  rects, with `fits(art, turn, x, y)`: every cell the sticker writes lies on it.
- The stuck stickers are part of the drawing: `DeckArt` (through `DeckView`) and
  `ItemArt` carry their copy's stickers resolved to their art, and their `draw` puts
  them on with `ui::stickers::draw`, so inspect, modding and the sticker mode all show
  them.

### The pawn deck (`ui/cyberdeck/pawn.rs`)

- The painted ` NO FUTURE ` and ` x‿x ` go; the copy's stickers are drawn after the
  keyboard and before the inventory row, landing and cables. The tag is drawn before
  them, so a sticker covers it.
- The tag keeps today's places (beside NO FUTURE's factory spot at (63, 10) when the
  handle fits before the failsafe frame, else under it at (50, 12), cut to fit); its
  doc no longer speaks of "the sticker".
- `sticker_area(view)`: the art reference's `PAWN_KEEP` less `PAWN_DROP`, the ROM row
  dropped from x 4 to the end of the last `Part::Rom` rect `parts(view)` gives.

### The Ratchet (`ui/looks/ratchet.rs`, `ui/looks/mod.rs`)

- `AREA`: `RATCHET_KEEP` less `RATCHET_DROP`. `ItemArt` returns it per look and
  draws its stickers after the fitted pieces.

### Sticker mode (`strata-zero::screens::stickers`, new, beside `bench.rs`)

Its own state (`StickerBench`), kept across mode switches like the bench's. It knows
the item only through its `Art` and the host's copy: no deck or gun code.

**Opening.** `s` in inspect opens it when the item's `Art` has a sticker area and the
window has a host copy; `s` again goes back to inspect. `m` in the sticker mode goes
to modding when the item is moddable, and `s` in modding comes here. While a sticker
is held, a peel is armed or modding's tray is open, the mode keys go to the mode
instead. Inspect's hint gains `s stickers` after `m mod` when the mode is open to it.
The title reads ` STICKERS · NAME `.

**The pointer** is in one of three places: the tray, on a held sticker (placing), or
on a stuck sticker. It opens in the tray, or on the stuck stickers when the pack holds
none, or on nothing when there are neither.

**The tray** (the art reference's `tray_rows` and layout notes):
- The pack's sticker stacks in pack order, one tile each. The band spans the
  drawing's columns at the body's foot, as modding's does, and is as tall as the
  tallest sticker in the tray plus four rows: a bracket row, the art, the count row,
  a bracket row, the readout. It is laid over the drawing.
- `STICKERS` (bold, `#6b6575`) at the drawing's column 2 on the first art row; tiles
  from column 13, three apart, each as wide as its sticker (at least its count), the
  art level and bottom-aligned on the count row, `×N` (`#6b6575`) under it. It
  scrolls as WAREZ and HARDWARE do, just far enough to show the pointed tile, with
  `‹` at column 11 and `›` at the drawing's last column (bold `#8b8596`).
- Pulsing corner brackets (`ui::art::brackets`) one cell outside the pointed tile's
  art-and-count box.
- An empty pack: `STICKERS` and `no stickers in your pack` (dim) in a five-row band.
- Keys: ←→ and h/l along the tray; space picks the pointed sticker (refused jacked
  in with `JACKED_IN` in the log); ↑ or k goes to the stuck sticker nearest the
  pointed tile, as `ui::art::step` measures (when there is one); esc or q closes.
- Readout: `NO FUTURE` (the name in capitals, bold cyan) ` ▸ 2 in your pack` (white)
  ` · lettered` or ` · picture` (dim).
- Hint: `←→ sticker · space pick · ↑ stuck · s inspect · esc close`, `↑ stuck` only
  when something is stuck.

**Placing.** Picking puts the sticker, level, on the fitting spot nearest the
drawing's centre (square units: a row counts twice a column) and the band sinks to
the readout row alone. If it fits nowhere level, it tries the other turns in order;
if it fits nowhere at all, nothing is picked and the log says "`Scan me` doesn't fit
on the `Ratchet SMG`". The held sticker is drawn over the drawing and its stickers,
with pulsing brackets one cell outside it.
- Arrows and hjkl move it a cell that way; when it does not fit there, it keeps going
  that way to the first spot where it does, and stays put when there is none.
- `r` turns it 90° clockwise about its centre and lands it on the fitting spot nearest
  that centre; a turn at which it fits nowhere is skipped; when no other turn fits, it
  does nothing.
- Space sticks it (`Action::Stick`, the result in the log); the pointer goes back to
  the tray on the same tile (or the last one, when that stack ran out), or to the new
  sticker when the pack has no stickers left.
- Esc or q puts it back: nothing changes, the pointer is on its tile again.
- Readout: `ORODYNE ▸ level` or ` ▸ turned 90°` · `space sticks it` · `it tears if
  peeled`. Hint: `↑↓←→ move · r turn · space stick · esc back`.

**Stuck stickers and peeling.** On the stuck stickers the band is the readout row
alone, so nothing on the drawing is covered.
- Arrows and hjkl step between the stuck stickers' rects with `ui::art::step`; ↓ or j
  with nothing below goes to the tray when the pack has stickers.
- Pulsing brackets one cell outside the pointed sticker's rect, overlapped or not.
- Space arms a peel: the readout reads `SAKURA ▸ space again peels it` · `it tears` ·
  `any other key keeps it`. A second space peels it (`Action::Peel`), and the pointer
  goes to the nearest remaining stuck sticker, else the tray. Any other key disarms
  and then does what it does; esc only disarms. Arming is refused jacked in.
- Readout unarmed: `SAKURA ▸ stuck` · `picture`. Hint: `↑↓←→ sticker · space peel ·
  ↓ tray · s inspect · esc close`, `↓ tray` only when the pack has stickers.

**Drawing.** The item where inspect draws it (`Art::origin`, so switching never moves
it), then every body cell outside the sticker area greyed with `ui::art::grey` (cables
and all, to the body's edges; bold dropped), as the art reference's mockups show; the
stuck stickers lie on the area and so keep their colour. Then the held sticker, the
band, the brackets and the readout on the body's last row (`bench::readout`'s
style, shared). It fits the minimum terminal: the tallest test sticker is 6 rows, a
10-row band, inside the 24-row body at 100×30.

### A sticker's own blueprint

Inspect on a sticker in the pack opens the window on the sticker drawn level at its
real size, centred, instead of MISSING BLUEPRINT. It has no sockets and no sticker
area, so neither `m` nor `s` is offered.

### D'SHAWN, kits and the console

- `Character::debug_runner` gives two of each of the 29 stickers (`give`, plain
  stacks). His worn rebuilt deck and the pawnshop deck he carries come with their
  factory stickers through `new_copy`.
- Kits hand out the pawnshop deck through `give_item`, which now makes a copy with its
  factory stickers.
- The console's `spawn <item> [qty]` hands out stickers like any item and tab
  completion finds them already; nothing changes there (`give` stays credits).

### README

The blueprint section of the README gets a paragraph on stickers: `s` in a drawn,
worn item's blueprint, the tray, moving and turning, sticking, peeling tears it,
meatspace only. The help overlay has no blueprint keys today (not even `m`), so it is
left alone; the window's hint row names every key.

## Non-goals

- Placing stickers in the world: no ground drops, no fixer tips, no shops. That is a
  later spec ("we can later determine how we will distribute them").
- Peeling back into the pack, or a careful peel that sometimes saves it.
- Stickers on hardware, programs, or items without a drawing; drawings for more
  items.
- Any effect: stats, value, weight, street cred, NPC reactions.
- Mirroring, free angles, scaling, custom stickers or an editor.
- A sticker line on the GEAR or INVENTORY cards or pane marks.

## Anti-goals

- Deck- or gun-specific code in the sticker mode. If the mode needs to know what the
  item is, the seam is wrong.
- Wording written for the current examples rather than from kinds and names.
- A sticker over a working part, a readout, a socket or the edge of the drawing.
- A double-width glyph anywhere a sticker draws, at any turn.
- Anything that breaks the 100×30 fit.
- Stickers changing anything but how the item looks.

## Testing

Unit tests in each crate, and screen tests over a `TestBackend` at 100×30 and 120×40:

- Content: the 29 stickers load with their art; validation refuses a sticker with a
  slot, a lettered text over 18 chars, a picture row of the wrong length, an unknown
  pixel char, factory stickers on an item that takes none or naming a non-sticker.
- Model: `takes_stickers` and `per_copy` for the three drawn items and none else;
  `new_copy` carries the factory stickers; `give_item` on the pawnshop deck makes a
  copy with them; D'SHAWN has two of each sticker.
- Actions: `Stick` takes one from the stack and pushes onto the copy, worn or in the
  pack; refusals leave everything unchanged; `Peel` removes and destroys; both refused
  jacked in; messages as written.
- Turning: every lettered sticker at every turn draws one-cell-wide glyphs in exact
  RGB; quarter turns stack three wide with the text top to bottom (90°) and bottom to
  top (270°); a half turn reverses and turns the glyphs (` NO FUTURE ` →
  ` Ǝꓤ∩⊥∩Ⅎ ON `, ` x‿x ` → ` x⁀x `); a picture turned four times is itself and a
  quarter turn swaps its size; every glyph in both tables is single width.
- Areas: the pawn deck's area excludes every part `parts(view)` gives, the glass,
  TRACE, the lights and the failsafe, and holds the keys; the Ratchet's excludes the
  ejection port, the barrel and the sockets' rects; the factory stickers fit the pawn
  area for both decks (and for D'SHAWN's long handle).
- Drawing: the pawn deck draws its copy's stickers where the paint was (the strings
  `NO FUTURE` and `x‿x` still show); a deck copy with them peeled shows neither; a
  sticker drawn over the tag hides it; newer stickers draw over older ones; the
  Ratchet draws a stuck sticker.
- The mode: `s` opens and closes it only for drawn, worn items with a copy (not
  hardware, not MISSING BLUEPRINT, not a sticker's own blueprint); `m`/`s` switch
  directly; the title and hints; the tray band's height, tiles, counts, scrolling and
  brackets at 100×30; pick puts the sticker on the nearest fitting spot; arrows skip
  unfit spots and stop at the area's edge; `r` turns about the centre, nudges into
  fit, skips turns that fit nowhere; space sticks with the log line and the pointer
  back on the tile; esc backs out with nothing changed; a sticker that fits nowhere
  says so; ↑ and ↓ between the tray and the stuck stickers; peeling takes two spaces
  and any other key keeps the sticker; picking and arming refused jacked in; cells
  outside the area greyed and stuck stickers in colour; the drawing never moves
  between modes.
- Shots: `shots.rs` gains ignored shot tests for the sticker mode (tray, placing,
  turned, peeling) on D'SHAWN's deck and the Ratchet, and the deck's blueprint with
  stickers, at 100×30 and 120×40.

## Left to the builder

- `StickerArea`'s representation (a bit mask, rect lists), how `ui::stickers` caches
  turned cells, and where the shared readout helper lives.
- How "nearest fitting spot" is searched (a scan of the area is fine; the areas are
  small).
- The order of the new fields in `Item` and `ItemCopy`, and serde defaults.
- Splitting `screens/stickers.rs` if it grows past the size of `bench.rs`.

## Calls made in writing this (for Jeffrey to check)

- The 29 stickers' ids, names and one-line descriptions (in the art reference); each
  weighs nothing and is worth 5.
- D'SHAWN carries two of each, so sticking and peeling can be tried twice.
- The tray hint says `↑ stuck`, not "on the deck": wording from kinds.
- Picking starts the sticker on the fitting spot nearest the drawing's centre; arrows
  skip over spots it doesn't fit; `r` skips turns at which it fits nowhere.
- Peeling is armed by the first space and done by the second, as the failsafe's two
  presses; any other key keeps the sticker.
- `game` doesn't check the area on `Stick`; the window only offers spots that fit.
- Inspect on a sticker draws it at its real size instead of MISSING BLUEPRINT.
- A lettered sticker is at most 18 cells, a picture at most 14×12 pixels (6 rows).
- The log lines "`No Future` stuck" and "`No Future` peeled · it tore", in the Fit
  messages' style.
- No blueprint keys in the help overlay; the README and the hint row carry them, as
  for modding.
