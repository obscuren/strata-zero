# Chrome on GEAR & INVENTORY (#103)

Status: picked with Jeff, 2026-11-16 (concept C, "a tile per site"). The look is the mock beside this
file, `2026-11-16-chrome-view-mock/` (round 2 for the body, round 3 for the right pane; run
`gen.py` as its docstring says). Follows the
body of parts (`2026-11-16-equipment-design.md`), which recorded which part each chrome site
belongs to.

## Why

Chrome lives on ATTRIBUTES, as a ledger beside the attributes; what you wear lives on GEAR &
INVENTORY. Jeff wants everything on the body in one place: GEAR & INVENTORY shows the body two
ways, what you wear and hold (the meat) and the chrome in it, and one key switches between them.

## Today

- ATTRIBUTES (`screens/sheet/`) has two panes: on the left the attributes, secondary numbers,
  resistances and the readout of the number under the cursor; on the right the chrome ledger
  (`sheet/ledger.rs`): the 11 sites (`game::Site`) in groups, a row under a piece for each of its
  sockets, the LOAD gauge on the header, and the card of the row (with the implant's drawing at
  120×40). `→` past the last tile crosses to the ledger.
- The ledger is also where chrome changes. Space on a row opens FITS (what in the pack fits there,
  with the load after and the doc's fee) inside the card; space fits the pick; `x` pulls (in a
  doc's chair the first `x` asks, for the fee). `operating()` decides: in a doc's chair, with dev
  free chrome, or locked (`chrome goes in and out in a ripperdoc's chair`). A drone cradle's bay
  docks and stows anywhere, not jacked in. The tab's free piece, the fee line, over-budget in
  magenta: all in the ledger and its tests.
- Sitting in the chair opens the deck on ATTRIBUTES, on the ledger (`Deck::chair`).
- Sockets on chrome: an arm's `Hand` and `Forearm`, a cradle's `Bay`.
- None of this state is saved (`deck_sheet`, the kit state are UI).

## Goals

- GEAR & INVENTORY shows the body as MEAT (as now) or as CHROME. `c` switches.
- CHROME draws the chrome as a body of its own: a tile per site where it sits, and a tile for
  each socket of what's installed, joined by lines (concept C).
- Everything the ledger does moves there unchanged: the card, FITS, fitting, pulling, fees, the
  tab's free piece, the bay anywhere, LOAD.
- The ripperdoc's chair opens GEAR & INVENTORY on CHROME.
- ATTRIBUTES keeps the attributes and the numbers that come from them, and nothing else.
- The manual follows (05 the deck, 10 chrome and drones).

## Non-goals

- New chrome, new rules for fitting, fees or load.
- Chrome that adds a body part (a third arm). The layout keeps free places for it; nothing adds
  one yet.
- Changing the MEAT view.

## Anti-goals

- Two places that change chrome. After this, only CHROME on GEAR does; ATTRIBUTES has no ledger.
- Losing anything the ledger shows: every site, every socket, what each holds, LOAD, the card's
  sockets and their candidates, the fee line.
- Slot names special-cased in code: tiles come from `Site::ALL` and the sockets of what's
  installed, placed by a table.

## Approaches

Three were drawn (round 1 of the mock):

- **A, sites on the parts:** the MEAT tiles, holding the sites of each part. Lines up with MEAT
  tile for tile; parts with several sites list them in a cramped tile.
- **B, the ledger beside the body:** the body as a map of pips, today's ledger on the right. Least
  new, but the body does little.
- **C, a tile per site:** the chrome as its own body. Every piece shown in full, as clothes are on
  MEAT; the two views don't line up tile for tile. **Jeff picked C.**

## CHROME

### The tiles

A tile per site, 11×5 like MEAT's, named for the site (`NEURAL`, `L ARM`, ...), and a tile per
socket of what's installed, named for the socket in capitals (`HAND`, `FOREARM`, `BAY`). Placed
on the body's left area (the same area MEAT uses), offsets from its origin:

| Tile | Place (x, y) | Joined to |
|---|---|---|
| EYES | 12, 0 | NEURAL |
| NEURAL | 24, 0 | EYES, EARS; the trunk down |
| EARS | 36, 0 | NEURAL, JAW |
| JAW | 48, 0 | EARS |
| L ARM's HAND | 0, 6 | L ARM |
| L ARM | 12, 6 | ORGANS; its FOREARM below |
| ORGANS | 24, 6 | L ARM, R ARM; the trunk |
| R ARM | 36, 6 | ORGANS; its FOREARM below |
| R ARM's HAND | 48, 6 | R ARM |
| L ARM's FOREARM | 12, 12 | L ARM |
| SPINE | 24, 12 | the trunk |
| R ARM's FOREARM | 36, 12 | R ARM |
| BACK's BAY | 0, 18 | BACK |
| BACK | 12, 18 | the trunk's branch |
| LEGS | 24, 18 | the trunk |
| SKIN | 36, 18 | the trunk's branch |

- The trunk runs down the middle (the column under NEURAL's centre) from NEURAL through ORGANS and
  SPINE to LEGS. Under SPINE it branches: left to BACK, right to SKIN, down to LEGS
  (`┬ ┼ ┴`, `┌ ┐` as the mock).
- Free places for later: (48, 12), (48, 18) and the column at x = 0 above BAY.
- A socket's tile shows only while its host holds a piece with that socket, with its line; a plain
  meat arm has no HAND or FOREARM tile, no cradle no BAY. A socket's place comes from its kind
  and its host (an arm's hand left or right of the arm, its forearm under it, a bay left of the
  back). A piece with two sockets of one kind is not in the content; if one comes, the builder
  gives the second a free place.
- In a tile: the piece's name on two lines (wrapped and cut as MEAT does), and dim under it what
  it gives: the first perk short (`NET +1`, `HP +10`), or the first ability it grants when it
  has no perk (`Bite`); an arm adds its sockets' pips (`■` fitted, `□` empty); a cradle says
  `1 bay ■`; a socket's piece its first perk or ability (`Jolt`), a docked drone `docked`.
- An empty site or socket shows a dim `·`, as an empty MEAT part does.
- The cursor's tile is lit as on MEAT.

### The header and the right

- The header row: `MEAT ▌CHROME▐  c switch` on the left (on MEAT, `▌MEAT▐ CHROME  c switch`).
- The right pane is split by a horizontal divider, as MEAT's is between the pack and the card
  (`╠═══`, joined to the vertical one). Over it, LOAD (Jeff, round 3 of the mock): `LOAD`, the
  gauge across the pane's width and `19/24` at its right (over budget in magenta, as the
  ledger's), and a dim line under it with what's left and how many pieces are in you. Under it,
  the card, or FITS in its place.
- The ledger's mark of whether chrome can change comes along unchanged, on CHROME only: the
  header's `· CHROMING UP` in amber in a doc's chair, the ` FREE ` badge with dev free chrome,
  nothing when locked (on the header row beside CHROME), and the divider between the body and the card
  turning amber in the chair or with free chrome, as the ledger's does. MEAT's divider never
  changes. `the_mark_is_on_the_header_line` moves with its asserts.
- Under the divider, no pack: ATTRIBUTES' card of the tile (`piece_card`, `fit_card`), with the
  implant's drawing on top where there is room, as the ledger shows it now. FITS opens in its
  place; LOAD stays over the divider.
- The hint row is the ledger's, as today: out of the chair its faint tail `chrome goes in and
  out in a ripperdoc's chair`, in the chair `esc get out of the chair` and the fee in amber.
  Nothing moves to the status strip. (Round 2's chair mock shows a right-aligned chair line and
  no amber mark; both are the mock's, not the spec's.)

### Keys

- `c` switches MEAT and CHROME, on either view, from anywhere on the tab (the pack too: it lands
  on the body). The view is remembered while the game runs, like the cursor; it is not saved.
- On CHROME: the arrows move tile to tile (each tile's neighbour in each direction is part of the
  layout table); there is no pack to cross to. `space`, `x`, the picker's `↑↓`, `space` and `esc`
  do what they do on the ledger now, with the same rules (`operating`, jacked in, the bay
  anywhere, the fee asked first). The hint row says what the ledger's hint says now, with `c
  meat/chrome` in it.
- The chair: `Deck::chair` opens GEAR & INVENTORY on CHROME, the cursor on NEURAL. `esc` still
  gets out of the chair.

### Sizes

- At the full size (the body's area at least 113×27, as MEAT): the tiles above.
- Below it, compact, in the left half, as MEAT goes compact: every site and every socket stays a
  cursor stop and shows what's in it; tiles may shrink (4 tall, a one-line name) and socket tiles
  may draw as one-line rows under their host. The exact compact layout is the builder's within
  that rule; 100×30 must show all of it. The card's drawing waits for room, as now.

## ATTRIBUTES

- One pane: the attributes, the secondary numbers, the resistances and the readout. With the
  ledger gone the readout moves to the right half, full height, beside the tiles (my call), so
  the tab's width isn't wasted. `→` no longer crosses anywhere; the hint loses it.
- The SECONDARY header's `from gear & chrome` stays.

## The code

- The ledger's rules and drawing move from `sheet/ledger.rs` to the kit (a `kit/chrome.rs` or as
  the builder sees fit): the cursor's stop (a site, or a socket of a site: today's `LedgerRow`
  without its group headers), `fitting`, the picker, `install`, `pull`, the fee and load lines,
  `piece_card`, `fit_card`, the LOAD header. ATTRIBUTES loses `SheetPane::Ledger` and its
  state's `row` and `picker`.
- The ledger's tests move with it and keep their asserts (hints in and out of the chair, the bay
  anywhere and jacked in, the tab's free piece, the fee line, over budget, `x` asks first, the
  picker not retyping the card, pull and reinstall keeps pieces).
- Shots: `attributes_sheet`'s ledger and picker shots become CHROME shots on GEAR (the same
  loadout, both sizes, the picker on a site, on the forearm and on the bay, the swap picker); a
  CHROME shot with meat arms and no cradle; ATTRIBUTES' own shots at both sizes.

## The manual

- 05 the deck: ATTRIBUTES loses its chrome line; GEAR & INVENTORY gets the CHROME view (`c`, the
  tiles, socket tiles, LOAD, the card, that changes happen in the chair).
- 10 chrome and drones: the chair opens GEAR & INVENTORY on CHROME; "the ledger" becomes the
  CHROME view.
- 02 your runner's tab table, if it says chrome is on ATTRIBUTES.

## Left to the builder

- Where the moved code lives and what its types are called.
- The compact layout within the rule above, and the neighbour table.
- Free places for a second socket of a kind, if content ever adds one.

## Calls made

1. Concept C, as round 2 of the mock lays it out (Jeff's pick; the layout table is mine).
2. A socket gets a tile only while what holds it is installed.
3. `c` is the key, on both views; the view isn't saved.
4. CHROME's right half is LOAD over a divider (Jeff's) and the card under it, no pack. The dim
   line under the gauge (what's left, pieces in you) is mine.
5. The chair opens GEAR & INVENTORY on CHROME.
6. ATTRIBUTES' readout moves into the right half the ledger leaves.
7. The chair and free-chrome marks, the amber divider and the hint row come over from the
   ledger as they are; no new chair line.
