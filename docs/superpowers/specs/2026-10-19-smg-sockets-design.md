# Sockets on the Ratchet SMG: design

Date: 2026-10-19. Status: designed with Jeffrey in the visual companion (terminal-
accurate mockups over the game's own 100×30 blueprint window), approved in chat ("Its
perfect. Including the scope", then "both good, write the spec and build it"); spec,
plan and build run on without check-ins. Worktree `.claude/worktrees/smg-sockets`,
branch `worktree-smg-sockets`, off local main 6b259f2.
Builds on: `2026-10-19-item-sockets-design.md` (copies, `Fit`/`Unfit`, the `Art`
seam, the modding mode, the marks, the cards), `2026-10-18-cyberdeck-tab-design.md`
(the blueprint window).
Art reference: `2026-10-19-smg-sockets-art.py` beside this file, the code that drew
the approved mockups. It is the truth for every pixel, colour and glyph below.

## Intent

The socket system was built for one item, the Rebuilt pawn deck. A second item with
sockets proves it is generic, or shows where it is not. The Ratchet SMG (`ratchet_smg`,
Primary slot) gets a blueprint drawing at the deck's level of detail and two sockets
of new kinds, a **muzzle** and a **rail**, drawn where those parts sit on the gun. Two
pieces of weapon hardware go in them, each with its own large blueprint and a drawing
in its socket that matches the gun. D'SHAWN carries both so modding can be tried at
once.

Jeffrey's bar from the item-sockets spec stands: the modding mode stays generic and
separate; "if the bench needs to know it's a gun, the seam is wrong"; wording comes
from socket kinds, never from piece names.

## Decided with Jeffrey

- **Effects while there is no street combat**: "Nothing. Just give it Cool +1 for
  now. We will add realistic effects later." The suppressor gives **Cool +1**. The
  sight gives **Reflex +1** (proposed as the parallel; Jeffrey approved the mockups
  that showed it).
- **The gun** (approved "perfect"): side view, muzzle right. Stamped gunmetal
  receiver with a seam to a darker lower half, ejection port, rivets, charging knob
  top rear, wire stock extended left with a rubber butt pad, polymer grip with a red
  grip wrap, trigger and guard in front of the grip, the magazine's base plate under
  it with a tape label ` GOOD STUFF ` (from the item text: "jams on cheap ammo, sings
  on the good stuff"). Stamped `RATCHET`, `9×19`, `SN ░░░░` (serial filed off), and the
  filed-down selector `S ▬ ∞`. 88×15 cells.
- **Two sockets.** `MUZZLE`: the barrel's threads, in gold like every connector (the
  deck's are gold too). `RAIL`: an aluminium rail with teeth on top of the receiver.
  An empty socket is its connector alone, as on the deck.
- **The pieces** (approved "both good"):
  - **Hushcan** (muzzle): a fat matte-black can, knurled collar against the threads,
    turned rings, rounded end, `HUSH` etched. In the socket it carries the bore line
    on to the right.
  - **Redeye sight** (rail): a reflex sight clamped on the rail, body and a taller
    hood at the front, a red front glass, a turret knob and a red emitter dot.
- **How a piece draws in its socket**: each socket kind has one footprint, the same
  on every host that places a socket of that kind; a piece is drawn once for the kind
  it fits. Muzzle: 18×5 cells running away from the threads. Rail: 24×3 cells above
  the rail's teeth. The host's drawing only says where each socket sits and what kind
  it is. (Shown to and approved by Jeffrey with the mockups.)
- **Large blueprints, each its own size**: Hushcan 72×14, a side view with the lower
  half cut away (collar with the gold female thread, blast chamber, baffle stack,
  `▲ THREAD` and `▲ BAFFLE STACK` captions, sticker `COOL +1`). Redeye 52×18, on a
  stub of rail for scale (clamp and cross-bolt knob, battery cap, windage turret,
  `REDEYE`, `2 MOA`, `+ −` buttons, emitter `●`, red-coated glass, sticker
  `REFLEX +1`).
- **The tray, while choosing, offers only pieces of the chosen socket's kind.** Today
  it offers hardware that fits *any* socket of the item, so choosing for MUZZLE
  would offer the sight and the fit would be refused. The fix is in the bench and
  holds for every item (shown in the mockups; Jeffrey approved them).

## Goals

### Socket kinds and mods (`game::model`)

- `SocketKind` gains `Muzzle` and `Rail`. `takes()`: `muzzle hardware`, `rail
  hardware`. `fits()`: `muzzle socket`, `rail socket`.
- `Mod` gains `Stat(Stat, i8)`: added to the stat while the piece is fitted in a worn
  item. `line()`: the stat's label and the signed number, `cool +1`, `reflex −1`
  (U+2212 for a minus, as the other mod text). `sticker()`: `COOL +1`.
- `HardwareLook` gains `Hushcan` and `Redeye`. A look is drawn for one socket kind:
  `HardwareLook::kind()` (Coldfins and Sidecar `Deck`, Hushcan `Muzzle`, Redeye
  `Rail`).
- **An item's drawing**: `Item.look: Option<ItemLook>` (serde default none), for
  items that are drawn but are neither decks (their look is `DeckSpec.look`) nor
  hardware (`Hardware.look`). `ItemLook::Ratchet` today.
- **A drawing says where its sockets are and what each takes**: `DeckLook::sockets()`
  and `ItemLook::sockets()` each return the drawing's socket places as `(name,
  SocketKind)` in order. Pawn: `[("L", Deck), ("R", Deck)]` (replaces
  `socket_names`). Ratchet: `[("MUZZLE", Muzzle), ("RAIL", Rail)]`.

### Content and validation (`game::content`)

- `ratchet_smg`: `look: Some(Ratchet)`, `sockets: [Muzzle, Rail]`. Its weight, value
  and text stay.
- New items (weights and values are calls, see the end):
  - `hushcan` "Hushcan", Hardware, weight 0.5, value 700,
    `Hardware(fits: Muzzle, mods: [Stat(Cool, 1)], look: Hushcan)`. "A baffle can
    that threads onto a muzzle. The shot still happens; the street just takes longer
    to turn round."
  - `redeye` "Redeye sight", Hardware, weight 0.2, value 550,
    `Hardware(fits: Rail, mods: [Stat(Reflex, 1)], look: Redeye)`. "A red dot on
    coated glass. You stop aiming and start pointing."
- Validation, all generic:
  - An item's sockets must be placed by its drawing, kind by kind: the drawing is
    the deck's look, else `look`; socket *k* of the item must be the drawing's place
    *k*'s kind, and there may be no more sockets than places. No drawing, no sockets.
    (Replaces today's count-only check.)
  - Hardware's `fits` must be its look's `kind()` (a look has no drawing for any
    other socket).
  - `look` on a deck or on hardware is refused: they have their own.

### Effects (`game::character`)

- **Hardware fitted in anything worn counts**, from every slot; a copy in the pack
  counts for nothing. `deck_mods` becomes `worn_mods` (every worn copy's fitted
  pieces' mods, in slot order). `trace_cut` and `deck_power` read it unchanged: only
  deck hardware carries those mods, so every deck number is what it is today.
- `Character::stat` adds `Mod::Stat` from `worn_mods` to the perks' bonus, inside the
  same 0..10 clamp. So D'SHAWN's Cool reads one higher on STATS (`5 · 6 with gear`)
  with a Hushcan on the Ratchet he wears.

### The Ratchet's drawing (`strata-zero`)

- An `Art` for items drawn by an `ItemLook` (size, draw, sockets), the Ratchet's
  drawing the first. Size 88×15, origin centred as the trait's default: at 100×30 the
  gun stands on rows 6-20 above the tray band (rows 21-25), at columns 6-93.
- Its socket places, drawing coordinates, in order:
  - `MUZZLE` `(66, 4, 22, 5)`: the gold threads (cols 66-69) and the muzzle
    footprint (cols 70-87, rows 4-8).
  - `RAIL` `(34, 0, 24, 4)`: the rail footprint (cols 34-57, rows 0-2) and the rail
    (row 3).
  The brackets fall one cell outside: at 100×30 the MUZZLE's right brackets land on
  column 94, inside the frame (inside columns 3-96); the RAIL's top ones on row 5.
- The connectors are always drawn, fitted or not: the threads and the rail. A fitted
  piece draws over its footprint by its look.
- **Pieces in sockets**: `ui::hardware` draws each look in its kind's footprint from
  the footprint's top-left: muzzle footprint at the place's `(x + 4, y)`, 18×5; rail
  footprint at the place's `(x, y)`, 24×3. A muzzle piece is drawn for a muzzle that
  points right; nothing else is needed today (no gun points left). The deck's side
  drawings stay as they are.
- Pixel art: the gun and both pieces are half-block pixel drawings (two pixels a
  cell). The build folds pixels into cells by the reference's rule (equal halves a
  blank cell on that colour, one empty half a half block over the panel, else `▀`
  top on bottom), then writes the text and glyphs. A small pixel canvas helper in the
  UI is the builder's call; the cells it writes must match the reference.
- Inspect on the Ratchet (GEAR or INVENTORY) opens the blueprint window on the
  drawing with the fitted pieces, hint `m mod · esc close`; no stops, nothing to
  point at, esc or q closes.

### The blueprint window and the bench

- `Blueprint::open`: a deck draws as its deck look, hardware as its large drawing,
  an item with a `look` as that look's drawing, anything else MISSING BLUEPRINT. The
  item-look case is one generic arm, no gun names in the window.
- `m` opens modding on the Ratchet exactly as on the deck (the copy has sockets, the
  item has a drawing, there is a host). Title ` MODDING · RATCHET SMG `.
- **The tray filter.** Not choosing: the pack's hardware that fits any socket of the
  item (unchanged). While choosing for a socket: only the pack's hardware whose
  `fits` is that socket's kind; EMPTY first on a filled socket as today. With nothing
  of that kind in the pack, space on the socket opens nothing (as an empty tray does
  today) and the tray row reads `nothing in your pack fits`.
- The readout and status read as today, from the socket's name and kind and the
  piece's name and mods: `SOCKET MUZZLE ▸ empty · takes muzzle hardware`, `HUSHCAN ▸
  socket MUZZLE · cool +1`, `Hushcan fitted · socket MUZZLE`.

### Hardware blueprints

- `HardwareArt::size` comes from the look: Coldfins and Sidecar 52×18 as today,
  Hushcan 72×14, Redeye 52×18. Each prints its first mod's sticker from the piece's
  mods (`COOL +1`, `REFLEX +1`).
- Tray tiles (swatch fg / swatch bg, tile bg, ink):
  - Hushcan: `#3a3f47` / `#1c1f24`, `#3a3f47`, `#e8ecf0`.
  - Redeye: `#ff2a3c` / `#15171b`, `#5a0c14`, `#ffd8dc`.

### The pane and cards

Nothing new to build; they are generic and the tests prove it on the gun: GEAR shows
`Ratchet SMG □□` (no slots, no dot), `■□` with a Hushcan fitted; the gun's card lists
`□ empty socket · muzzle hardware` / `■ Hushcan · cool +1`; a Hushcan's card reads
`hardware · muzzle socket` and `cool +1`.

### The debug runner and starts

- D'SHAWN wears his Ratchet as a copy with both sockets empty (Vex's kit already
  wears it; a per-copy item is worn as a copy). His pack adds a Hushcan and a Redeye
  sight after the deck hardware.
- Every start whose kit holds the Ratchet now holds a copy of it with empty sockets;
  nobody else gets weapon hardware.

## Non-goals

- Weapon effects beyond the stat (noise, recoil, accuracy): no street combat yet.
- More guns, more pieces, rolled sockets or numbers, where hardware comes from.
- A muzzle or rail drawn pointing any other way than this gun's.
- Moving the deck's sockets onto the kind footprints: the deck's side drawings and
  rails stay as built.

## Anti-goals

- Anything in the bench or the blueprint window that names the gun, a socket kind or
  a piece.
- Wording written for a piece instead of read from a kind or a mod.
- Anything that does not fit 100×30, double-width glyphs, or colours that are not
  exact RGB.

## Testing

- **Model and content**: `takes`/`fits` for the new kinds; `Mod::Stat` line and
  sticker, both signs; `HardwareLook::kind`; the Ratchet loads with its look and two
  sockets, the pieces with their numbers. Validation refuses: sockets of the wrong
  kind for the drawing's place, more sockets than places, sockets with no drawing,
  hardware whose `fits` is not its look's kind, a `look` on a deck or on hardware.
- **Effects**: Cool is unchanged with the Hushcan in the pack; +1 with it on the worn
  Ratchet; +0 with it on a Ratchet in the pack; the clamp at 10 holds; Reflex +1 with
  the Redeye; trace cut and deck power are unchanged by weapon hardware, and every
  existing deck-mod test still passes.
- **Copies**: Vex's start and D'SHAWN wear the Ratchet as a copy with two empty
  sockets; D'SHAWN's pack holds a Hushcan and a Redeye sight.
- **Fit and unfit on the gun**: the Hushcan fits MUZZLE and is refused on RAIL; the
  Redeye the reverse; jacked in, both refuse; carried and worn both work.
- **Drawing**: the bare gun draws its gold threads and the rail at their cells and no
  piece; a fitted Hushcan and Redeye draw in their footprints (checked against the
  reference at known cells: the `HUSH` text, the red lens column, the emitter `●`);
  every glyph single width and every colour exact RGB; at 100×30 nothing is clipped,
  the gun clears the tray band and every bracket lands inside the frame.
- **Window and bench**: Inspect on the Ratchet shows its drawing and `m mod · esc
  close`; `m` switches to modding and the title says so; the stops are MUZZLE then
  RAIL and the arrows move between them; the readout on an empty MUZZLE reads `takes
  muzzle hardware`; choosing for MUZZLE offers only the Hushcan and for RAIL only the
  Redeye; fitting each and the status line; on the deck the tray still offers its
  three pieces (and no weapon hardware); the grey covers the gun but the socket
  rectangles.
- **Large blueprints**: the Hushcan draws 72×14 and the Redeye 52×18 without clipping
  at 100×30, each prints its sticker from its mod.
- **Pane and cards**: as listed under *The pane and cards*.
- **Shots**: ignored shot tests for the Ratchet's blueprint (bare and fitted), its
  modding (pointer, tray, fitted) and both large blueprints, at 100×30 and 120×40.

## Calls made in writing this (for Jeffrey to check)

- Item weights and values: Hushcan 0.5 kg ¤700, Redeye sight 0.2 kg ¤550.
- The pieces' item texts (above).
- `worn_mods` counts hardware in every worn item; the old name `deck_mods` goes.
- Socket names `MUZZLE` and `RAIL` in capitals, as the deck's `L` and `R` are.
- `Item.look` / `ItemLook` as the name for the drawing of an item that is neither a
  deck nor hardware.
- The Redeye's large blueprint is 52×18, the deck hardware's size; the Hushcan's
  72×14.
