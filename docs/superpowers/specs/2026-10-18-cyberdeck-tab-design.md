# The CYBERDECK tab as a deck: design

Date: 2026-10-18. Status: designed with Jeffrey in the visual companion (three
concepts; he chose "the slab" and trimmed it), built on `worktree-cyberdeck`. Worktree
`.claude/worktrees/cyberdeck`, branch `worktree-cyberdeck`.
Builds on: `2026-10-17-keypads-design.md` and `2026-10-17-device-themes-design.md`
(diegetic hardware drawn in cells), `2026-10-15-constructs-and-programs-design.md`
(programs, slots, loading), `2026-10-15-ice-that-hunts-design.md` (the failsafe).

## Intent

The CYBERDECK tab works and is boring: a list of rows. The terminals and the
door keypads are hardware you look at; the deck should be too. The tab draws
the jacked deck as an object on the table, seen from above: programs are
cartridges standing up out of its back edge, the failsafe is a switch under a
red guard cover, and a small amber glass on the deck reads out whatever you
point at. Programs you carry lie on the table below it. You move a pointer
between the deck's parts and act on them: pick a cart up, carry it to a slot,
drop it in; lift the cover, throw the switch.

Decks will come in different kinds later, each drawn its own way. So a deck
says which drawing it wears, and this one, the pawnshop deck's, is the first:
the **pawn** look.

## Goals

### Deck looks

- `DeckSpec` gains `look: DeckLook`, an enum with one variant today, `Pawn`.
  Missing from a content entry it defaults to `Pawn` (`#[serde(default)]`).
  `flavor/content.ron`: the pawnshop deck says `look: Pawn` explicitly.
- A look owns its drawing and the places on it: where the slots, the ROM
  strips, the failsafe, the glass and the maker plate are. The parts, the
  pointer, the keys and every rule below are shared by all looks; only the
  drawing and the positions differ. A second look later is a second drawing
  and a second set of positions, nothing else.
- The pawn drawing places up to 4 slots along the back edge (A-D, 14 columns
  each) and up to 3 ROM strips in its ROM row. Content validation refuses a
  `Pawn` deck with more slots or grants than that.

### The pawn look

A fixed drawing, 84 columns by 22 rows, centred in the tab's body (the area
between the rule under the tabs and the status strip). At the smallest window
the game allows (100×30) the body is 98×22, so the drawing fills it top to
bottom; bigger windows get more room around it. Below the minimum height the
game already refuses to draw. Reference, at the smallest window, pointing at
slot A:

```
       ┏            ┓
        ▗▄▄▄▄▄▄▄▄▄▄▖  ▗▄▄▄▄▄▄▄▄▄▄▖
          CRACK         ATTACK
          r1 · s1       r1 · s1
 ╭─────┗A ▀▀▀▀▀▀▀▀▀▀┛ B ▀▀▀▀▀▀▀▀▀▀▀───────────────────────────────────────────────╮
 │   K▒ROK▒WA KX-2  2 SLOT · R1                 ● PWR  ● LINK  ● HOT              │
 │ ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓                                     │
 │ ┃ SLOT A ▸ CRACK                         ┃     HEAT               ▞▞▞▞▞▞▞▞▞▞   │
 │ ┃ rating 1 · size 1 · noise 4 · work 3   ┃     ▮▮▮▮▮▯▯            ▞  ▲ ON  ▞   │
 │ ┃ Brute force with manners. Opens        ┃                        ▞  █     ▞   │
 │ ┃ anything that answers, loudly.         ┃      NO FUTURE   VEX   ▞    off ▞   │
 │ ┃                                        ┃                        ▞▞▞▞▞▞▞▞▞▞   │
 │ ┃ [space] eject                          ┃                         FAILSAFE    │
 │ ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛ x‿x                                 │
 │  ROM▸  NETSCAN   ARC TRACE                                                     │
 │   ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆  ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆ ▆▆     │
 │         ▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆               │
 ╰────────────────────────────────────────────────────────────────────────────────╯
  INVENTORY
    SPOOF    ▐    BLIND    ▐    MASK     ▐    BACKDOOR       ▐
    r1 · s1       r1 · s1       r1 · s1       r1 · s2
```

Row by row (rows of the drawing, 0 at the top):

- **0**: room for the pointer's top corners over the carts.
- **1-3, the carts.** A loaded program stands up out of the back edge: a grip
  row (`▗▄…▄▖` in the cart's grey), then two rows of masking tape (light tape
  colour, dark ink): the name in capitals, bold, and `r<rating> · s<size>`. A
  size-1 cart is 12 columns, a size-2 cart 26 (it spans two slots). An empty
  slot shows `empty` in dim on row 3.
- **4, the back edge.** The deck's rounded outline in rust, the bay's mouth a
  dark `▀` run under the slots, `A`, `B` (… `D`) labels in the mouth.
- **5, the maker plate and lights.** `K▒ROK▒WA KX-2` bold on a lighter body
  patch (the `▒` are scratches), then `<slots> SLOT · R<rating>` dim. Lights:
  `● PWR` green; `● LINK` cyan while jacked in, dark teal otherwise; `● HOT`
  orange.
- **6-13, the glass.** 40×6 inside a heavy dark bezel: amber text on dark
  amber glass, alternate rows a shade darker (scanlines). See *The glass*.
  The yellow `x‿x` sticker (black ink, bold) sits just right of the bezel's
  bottom-right corner.
- **7-8, HEAT.** `HEAT` bold in the plate grey, then a 7-cell gauge: lit cells
  `▮` orange, the rest `▯` dark brown. See *Heat*.
- **10, stickers.** `NO FUTURE` bold white on magenta.
- **10 or 12, the tag.** The player's handle in capitals, bold cyan, as if
  sprayed on. It goes beside the sticker when it fits before the failsafe
  frame, else on row 12 under the sticker, cut to fit.
- **7-12, the failsafe.** A 10×5 frame of hazard stripes (`▞` yellow on black)
  around a 8×3 well. Cover shut: the well is dark red, the lever `▲` over `█`
  with `ON` bold white and `off` dull. `FAILSAFE` bold under the frame on row
  12. Cover up: the cover stands on row 6 above the frame as a red `▄` run, the
  well is black and the switch bare. See *The failsafe*.
- **14, ROM.** `ROM▸` dim, then each ability the deck grants as a strip:
  black text on dark amber, ` NETSCAN `, ` ARC TRACE `.
- **15-16, the keyboard.** One row of `▆▆` keycaps in the body's key grey with
  one missing (`··`) and one magenta replacement, then a space bar.
- **17**: the deck's bottom edge.
- **18-20, INVENTORY.** `INVENTORY` bold dim, then the programs in the pack and
  not loaded, as carts lying flat: two rows each, darker tape, name and
  `r · s`, 12 columns for size 1 and 18 for size 2, two columns apart. When
  they do not fit the 84 columns the row scrolls to keep the pointed cart in
  view, with `‹` / `›` at the ends when more lie off either side. Only
  programs are shown; the inventory tab still lists everything.
- **21**: room for the pointer's bottom corners under the inventory.

Colours as constants in the look's module, from the mockup: body `#24212a`,
lighter body `#322e3a`, bezel `#141218`, rust edge `#a0583a`, cart `#3b3842`,
tape `#d9c9a0`, loose tape `#8a8270`, ink `#1a1612`, glass `#140e06` and
`#100b04`, amber `#ffb000`, dim amber `#8c6014`, hazard `#ffcc00`, cover
`#5a0c14`, hot `#ff6a00`, keys `#4c4855`. Cyan and magenta are the chrome's.

### Pointing

- **Corner brackets.** The pointed part is framed by four cyan heavy corners
  (`┏ ┓ ┗ ┛`) one cell outside its rectangle, pulsing like the selected rows on
  other tabs. The part itself is drawn as it is. (Jeffrey chose brackets over
  lighting the part up.)
- **What can be pointed at:** in the bay, each loaded cart (one stop whatever
  its size; the brackets frame the whole cart) and the first empty slot if
  there is one (the only empty slot a program can land in, since the bay
  packs left; further empty slots are drawn but are not stops). Then the
  maker plate, each ROM strip, the failsafe, and each cart in INVENTORY.
  Stickers, lights, HEAT and the keyboard are decoration. (Builder-check
  call: bay stops are landing places, so pointing and landing never disagree.)
- **Moving.** Arrows and `hjkl` move to the nearest part in that direction
  (by the parts' centres, preferring the most in line), so any look's layout
  navigates without a hand-written map. Nothing in that direction: stay put.
- The pointed part is remembered per session the way other tabs remember
  their row (`st.deck_rows`), falling back to slot A, then to the first cart
  in inventory.

### The glass

- **It reads out whatever the brackets are on** (Jeffrey: "when we have
  something highlighted, display the information in the screen of the
  cyberdeck"). There is no detail pane on this tab; the glass is it.
- First row bold: what it is (`SLOT A ▸ CRACK`, `FAILSAFE ▸ ON`, `ROM ▸
  NETSCAN`, `BACKDOOR`, the deck's name for the plate). Then the same facts
  and text today's detail cards give: `program_card` for a program, the
  failsafe text, the ability card for a ROM strip (sharpened abilities read
  as sharpened, as today), the item card for the plate. Text wraps to the
  glass's width.
- **What comes first is what fits.** For a program, the glass leads as the
  mockup does: one stats line (`rating 1 · size 1 · noise 4 · work 3`), then
  the description; the rest of today's card (what it targets, your power
  against the odds, `loaded`) follows below it, reached with PgDn. Other
  parts keep their card's order. So the card text is shared, but a program
  gets a glass order of its own. (Builder-check call: the full card would
  push every description below the fold.)
- **The last row is pinned to the key that acts**, e.g. `[space] eject`,
  `[space] pick up`, `[space] lift the cover`, `[space] swap · esc put back`.
  Parts with no action pin a plain line instead (`burned in · can't be
  pulled` for ROM, `empty · bring a program from inventory` for an empty slot
  with nothing held).
- When the text does not fit above the pinned row, PgUp/PgDn scroll it, with
  `▲` / `▼` at the glass's right edge when there is more, as the terminal
  does. Pointing at another part starts it at the top.
- The text types on as the detail pane does today (the reveal), so moving the
  brackets feels like the deck answering.

### Slotting

The bay packs left: loaded programs fill the slots from A in their loaded
order, which is also the order of the `1`-`5` program keys. Ejecting one slides
the rest left. Empty slots are always on the right.

- **Eject.** Space on a loaded cart unloads it; it joins INVENTORY.
- **Pick up.** Space on a cart in INVENTORY picks it up. The brackets jump to
  the first empty slot, else the first loaded cart. Its place in INVENTORY
  shows a dim dashed outline. While holding, the arrows move between the bay's
  stops only.
- **Where it lands.** A bay stop is a position in the loaded order: loaded
  cart *k* is position *k*, the first empty slot is position `loaded.len()`.
  Dropping at position *k* on a loaded cart takes that cart out and puts the
  held one in its place; dropping on the empty slot appends. If the deck is
  then over its slots, carts after the new one go back to INVENTORY, nearest
  first, then carts before it, nearest first, until it fits. The others keep
  their order. Every check is made before anything changes.
- **Carry.** The held cart's dashed cyan outline is drawn where it will land
  once packed (so a size-2 cart dropped on the empty B with CRACK in A shows
  over A-B, CRACK displaced), labelled with the slot letter it lands at:
  `SPOOF ▸ B`. The glass says what happens: `SPOOF ⇄ ATTACK`, "Spoof goes in
  slot B, Attack back to inventory.", pinned `[space] swap · esc put back`
  (or `[space] slot` when nothing is displaced).
- **Drop.** Space does it. The status strip says what happened, in the log's
  register: "`Spoof` slotted · `Attack` to inventory", or "`Spoof` loaded ·
  2/2 slots" as today when nothing was displaced.
- **Refusals happen before anything moves.** If the drop cannot be done (the
  deck is too small for the cart even with everything displaced), nothing
  changes, the strip says why in magenta, and the cart stays held. Pointing
  at a cart in INVENTORY that can never fit says so on the glass before it is
  picked up ("needs 2 slots · this deck has 1"); one that fits only by
  displacing says what it needs ("needs 2 slots · eject one").
- **Put back.** Esc while holding puts it back where it lay; nothing changes.
- Loading rules are today's (`can_load`): the deck reports refusals the same
  way. Nothing new about when you may load (jacked in or not stays as it is).

### The failsafe

`st.net.failsafe` and what it does are unchanged. The tab only makes turning
it off take two presses.

- **On, cover shut** (the default). Glass: `FAILSAFE ▸ ON` and today's text,
  pinned `[space] lift the cover`.
- **Space lifts the cover.** The cover stands up above the frame, the switch
  is bare and still on. Glass: `FAILSAFE ▸ ON · cover up`, "Throw it and
  black ICE can kill you.", pinned `[space] throw · esc shut`. Esc, or moving
  the brackets away, shuts the cover again. Lifting changes nothing in the
  game; it is the tab's state, not saved.
- **Space again throws it off.** The lever drops (`▼` red, `OFF` bold red,
  `on` dull), the cover stays up as a reminder, the strip warns "failsafe off
  · black ICE can kill you" as today. Glass: `FAILSAFE ▸ OFF` in red, pinned
  `[space] throw it back`.
- **Space when off** throws it back on and shuts the cover in one press; the
  strip says "failsafe on".
- Opening the tab with the failsafe off draws it off with the cover up.

### Heat

The gauge reads the trace the HUD would show you: inside a construct, that
construct's heat; on the grid, the nearest gate's trace within reach
(`NetState::nearest_trace`); anywhere else, cold (no cells lit). Seven cells
over 0 to `DUMP_HEAT`, a cell lit for each seventh begun. Cells at or past
`ALARM_HEAT` may be drawn a hotter shade; builder's call.

### Keys on this tab

As the TALENTS tab does, the arrows belong to the tab:

- `↑↓←→` / `hjkl` point (between slots only while holding).
- `space` / `enter` act on the pointed part.
- `esc` puts a held cart back, else shuts a lifted cover, else closes the
  deck as on every tab.
- `tab` / `shift-tab` switch tabs; `0`-`9` jump, as today.
- `PgUp` / `PgDn` scroll the glass.
- The hint line: `↑↓←→ point · space act · tab tabs · esc close`, and while
  holding `↑↓←→ carry · space drop · esc put back`.

### No deck

With no deck equipped (or one the content does not know) the tab shows one
line, `No CYBERDECK equipped`, centred in the tab's body in the chrome's dim
colour, and nothing else: no deck, no glass, no INVENTORY, no brackets.
Nothing can be pointed at; arrows and space do nothing; esc closes the deck and
`tab` / digits switch tabs as usual. (Jeffrey, after the build: "When no
cyberdeck is installed we should simply display a message … The chrome,
display, programs etc should be hidden.")

## Non-goals

- Other deck looks. Only `Pawn` is drawn; the type exists so the next one has
  a place to go.
- Animation beyond the pulsing brackets and the glass's type-on: no cart
  sliding into the bay, no cover swinging.
- Mouse. The deck screen has none today.
- Any change to what programs, the failsafe or heat do, or to when loading is
  allowed.
- Upgrading or modding a deck, stickers you choose, a deck's own name.
- A jack-out penalty (still owed elsewhere).

## Architecture

- `game::model`: `DeckLook { Pawn }` (serde, `Default` = `Pawn`); `DeckSpec {
  slots, rating, #[serde(default)] look }`. Validation: a `Pawn` deck has at
  most 4 slots and grants at most 3 abilities.
- `game::character` / `game::action`: `Action::Slot { id, at }` over
  `Character::slot_at(c, id, at)`, where `at` is a position in `loaded` (see
  *Where it lands*), returning what it displaced, every check made before any
  change. `Load` and `Unload` stay for the console and anything else that
  uses them.
- `ui/cyberdeck/` (new): the shared part. A `Part` enum (`Slot(u8)`,
  `Plate`, `Rom(u8)`, `Failsafe`, `Inv(usize)`), the nearest-in-direction
  move over a look's part rectangles, the glass's text for a part (built
  from the existing card functions, moved or shared from `screens/deck.rs`),
  the bracket drawing.
- `ui/cyberdeck/pawn.rs` (new): the pawn drawing and its part rectangles,
  its colours, the fixed size.
- `screens/deck.rs`: the CYBERDECK tab draws through the look, as MAP and
  TALENTS draw their own bodies, and handles its keys before the shared ones
  (arrows, space, esc, PgUp/PgDn) as TALENTS does. Tab state on `Deck`: the
  pointed part, the held program, cover up, the glass's scroll. The list rows
  for the tab (`cyberdeck_rows`) go.
- Shots: an ignored test in `screens/shots.rs` drawing the tab at 100×30 and
  at 120×40, so the look can be checked as an SVG like the devices.

## Testing

- `model`: a deck entry without `look` reads as `Pawn`; validation refuses a
  `Pawn` deck with 5 slots or 4 grants.
- `character` / `action`: slotting at `loaded.len()` loads; at a loaded
  program's position swaps and keeps the position (the slotted program is at
  that index of `loaded`); a size-2 program at position 0 over two size-1
  programs sends both back; a size-2 program appended after one size-1
  program displaces it; a refusal leaves `loaded` and the pack exactly as
  they were.
- Pointing: from slot A, right reaches slot B, down reaches the plate or the
  nearest part below, and so on through the pawn layout; nothing to the left
  of slot A keeps it on A; a loaded size-2 cart is one stop; empty slots past
  the first are not stops; while holding only bay stops are reachable.
- Glass: pointing at a program shows its name and stats and pins `[space]
  eject`; at the failsafe pins `[space] lift the cover`; at a ROM strip shows
  the ability (sharpened when sharpened); long text shows `▼` and PgDn
  scrolls.
- Slotting through the tab: pick up SPOOF, move to B, space: `loaded` is
  CRACK, SPOOF and ATTACK is in INVENTORY; esc while holding changes
  nothing; BACKDOOR on a 2-slot deck displaces both.
- Failsafe: one space leaves `st.net.failsafe` on with the cover up; a second
  turns it off with the warning; esc between them shuts the cover; space when
  off turns it on in one press.
- Heat: in a construct with heat 60 the gauge lights 5 cells; off the Net it
  lights none.
- Draw: at 100×30 the drawing fits the body with nothing clipped; it shows
  the plate, the carts' names, `FAILSAFE`, `INVENTORY` and the handle; at
  120×40 it is centred; no deck draws `No CYBERDECK equipped` and no deck
  parts, glass or INVENTORY.
- Existing deck tests that read the CYBERDECK list rows are rewritten against
  the new tab, keeping what they checked (odd decks, loading and unloading,
  the failsafe toggle, the sharpened ability).
