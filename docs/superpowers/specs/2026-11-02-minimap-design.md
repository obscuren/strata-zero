# The minimap: a close up outdoors, the floor plan and the shaft indoors

Date: 2026-11-02. Mockups: `2026-11-02-minimap-mock/` (`round1.png`: Now, A, B and C on
the street, and the deck MAP with and without its seen shading; `round2.png` and the
phone shots `indoor_flat.png`, `indoor_office.png`: Now, 1, 2 and 3 indoors), with their
generators (`mm.py`, `mm_in.py`) and the frames they draw over (`shots/`, from the
throwaway dump tests in `dump_tests.diff`).

## Why

The sidebar's MAP pane (`render/minimap.rs`) squeezes the whole map into 24 of its 40
columns at one cell per 10 × 10 tiles. Each cell is the district's accent scaled by how
walkable its tiles are. There are no streets, no places, no walk, and the aspect is
squashed two to one. Jeffrey: "it just shows colors and it looks awkward and isn't
helpful at all." The deck already has an overview MAP of the whole city, so the minimap
doesn't need to be one.

On the deck MAP, the seen/unseen shading (faint where you have never been, lit where you
have, brighter where you can see now) "looks stupid and adds nothing to the map in terms
of clarity."

## What Jeffrey decided

- **Outdoors: A, the close up.** The deck MAP's own look (half-block pixels, each
  coloured by what its tiles are and their district's theme), zoomed in to 3 × 3 tiles a
  pixel and centred on you. The main view shows as four corners; your walk is in the
  tracking cyan; the target is marked. The bottom row names your district and gives the
  target's distance and bearing.
- **Indoors: 3, the floor plan and the shaft.** The floor you are on, drawn whole in the
  close up's style: rooms as blocks, corridors as the streets between them, the atrium's
  drop as a pit inside its railing, the lift `⇅`, the stairs `≡`, your home `⌂`, you
  `@`, the walk to the lift in cyan. At the right, a strip of the building's floors like
  a lift panel: yours, the one the walk wants (`⌖`), the top and the bottom, the shaft
  between yours and the wanted one lit cyan.
- **The deck MAP loses its seen/unseen shading.** Every pixel is drawn at the one level
  walked ones have today, and the header loses "lit: remembered · faint: the city grid".

## Goals

- The minimap answers "what is around me and which way do I go" outdoors, and "which
  floor am I on, where is the lift, where does the walk go" indoors, at a glance.
- It reads as the same map as the deck MAP at another zoom: the same pixel colours.
- It works on every map: each physical stratum, every floor of every building, the Net
  and constructs, at every sidebar height down to the squeezed 100 × 30 one.
- Both front ends: exact `Rgb` colours, every glyph in the window's fonts.

## Non-goals

- New controls. The minimap is not interactive; zooming and panning stay on the deck MAP.
- Fog of war anywhere. With the seen shading gone, nothing on the minimap or the deck MAP
  depends on what you have seen. `Visibility` itself (sight, the main view's remembered
  tiles) is untouched.
- Showing people, foes or loot on the minimap.

## Anti-goals

- A second overview of the city. The minimap stays a close up; the whole city is the
  deck MAP's job.
- Clutter: a marker for every place nearby. Outdoors the minimap marks only you and the
  target; indoors only you, the lift, the stairs, your home and the target.
- A map that jitters: the pixels must cover the same tiles from frame to frame however
  you move, so the ground doesn't shimmer as you walk.

## Design

### 1. The pane

The MAP pane keeps its place and size in the sidebar: its separator, `MINIMAP_H` (13)
map rows and one more, squeezed down to `MAP_MIN` (7, the separator included, so 5 map
rows) when LOG needs the rows, as today (`sidebar::budget`). Under the separator, every
row but the last is the map; the last row is the **bottom row** (§5), in place of
today's coordinate row. The map is 40 cells wide,
the whole column.

What the map shows follows the map the sidebar describes (`SidebarView::map`):

| Map | Minimap |
|---|---|
| A stratum (Undercity, Street, Skyways, Heights) | the close up (§2) |
| A floor of a building | the floor plan, with the shaft when the building has more than one floor (§3) |
| The Net grid | the close up, with the Net's tiles classed as in §4 |
| A construct | the plan of the construct, no shaft (§4) |

### 2. The close up (outdoors)

- **Pixels.** Each cell is two pixels stacked (`▀` in the top pixel's colour on the
  bottom's), each pixel 3 × 3 tiles. The colour is the deck MAP's pixel colour for those
  tiles, without seen/unseen (§6): its class (building, road, walk, water) in its
  commonest district's theme (`roof`, `road` × 1.9, `sidewalk` × 1.15), water `WATER`,
  no district `NEUTRAL`, and the panel ground off the stratum or where nothing is.
  Exactly one function computes this colour, shared by the deck MAP and the minimap.
- **Where it looks.** Centred on you (the player's tile when the player is on this map,
  else the main view's centre, e.g. the dev free camera). The origin snaps to the grid of
  cells (x to a multiple of 3 tiles, y to a multiple of 6), as the deck MAP's pan does,
  so a cell always covers the same tiles. The centre may sit up to a cell off true. Near
  the stratum's edge the view does not clamp: what lies off the stratum is the panel
  ground.
- **The main view.** Its four corners `┌ ┐ └ ┘` sit in the cells of the main viewport's
  corner tiles (`SidebarView::view`), in light grey (230, 230, 240) on the ground under
  them. A corner that falls outside the close up is left out. There are no edges between
  the corners.
- **The walk.** When the track's leg is on this map (`leg.place == map.place()`), every
  pixel a tile of its path falls in is mixed 75 % toward the tracking cyan. The walk is
  drawn still: no pulses and no ripple (the main view has them).
- **The target.** When the tracked target stands on this map, its marker (§5, "the
  target's marker") is drawn on its cell. When it is on this map but outside the close
  up, the marker is drawn on the close up's border instead, where the line from the centre
  to the target leaves the map. When it isn't on this map, nothing is drawn; the walk and
  the bottom row show the way.
- **You.** `@` in white on your cell, drawn last so it wins its cell. Not bold: the
  bold `@` is the one in the main view (a test holds the player to one).
- A glyph over a half-block cell takes as its background the colour of the pixel its
  tile falls in, so the ground under it stays right.

### 3. The floor plan and the shaft (indoors)

**The plan.** The floor you are on, in pixels like the close up but with the floor's own
classes, in the colours of the building's district theme (`FloorPlan::theme`):

| A pixel's tiles | Colour |
|---|---|
| any railing (`Terrain::Railing`) | the rail: `roof` mixed 25 % toward (230, 230, 240) |
| half or more atrium drop (`Terrain::Atrium`) | the pit: (5, 7, 11) |
| walls: wall tiles × s ≥ the tiles counted (a third of a pixel at s = 3, half at s = 2), so a one-tile wall always shows | `PANEL_BG` mixed 30 % toward `roof` |
| otherwise, by the room most of its floor tiles lie in (`Floor::room_at`): | |
| · no room, or a corridor | `road` × 2.1 |
| · open plan | `roof` × 1.25 |
| · home (`RoomKind::Home`) | the room colour mixed 22 % toward (124, 255, 154) |
| · any other room | `roof` × 1.1 |
| no tiles (outside the floor, `Terrain::Void`) | the panel ground |

Walls are `Wall`, `Window`, `Mass` and every other terrain that is neither walkable nor
one of the rows above. A floor tile with furniture on it is floor, not wall, as on the
deck MAP's street ("a stall on a pavement is pavement").

**Its scale.** The plan is drawn whole when it fits: at 2 tiles a pixel if the floor fits
the plan's cells at 2, else at 3, centred in its area. A floor too big for 3 is drawn at 3
and follows you: centred on you and clamped to the floor, so no ground beyond the floor
shows on the side you approach. A floor is three times its street footprint, so a
megablock's 60 × 60 floor fits at 3 in a full pane and follows you in a squeezed one.

**On it**, in this order, so the later ones win a shared cell:
- the walk to the lift (or the stairs, or the door out) in the tracking cyan, as in §2;
- the stairs `≡` in `DIM` at `FloorPlan::stair`, where that tile really is a stair;
- the lift `⇅` in light grey (230, 230, 240) at `FloorPlan::lift`, when the building
  has one. The lift wins a cell it shares with the stairs, unless the walk goes to the
  stairs (no lift, or the lifts locked), when the stairs win it;
- your home `⌂` in the home green (124, 255, 154), bold, at the door of a `Home` room on
  this floor, unless you are in it;
- the target's marker, when the target is on this floor (§5);
- you, `@` in white, bold.

**The shaft.** When the building has more than one floor (`lowest < top`), the right 8
columns of the map are the shaft strip and the plan takes the other 32. A one-floor
building has no strip and the plan takes all 40.

The strip's columns, from its left: a `│` in `FAINT` all the way down, then the floor's
name right-aligned in four cells (`FloorPlan::label`: `27F`, `L`, `B1`), a space, the
marker column, a space. Its rows are floors, top floor first, with a gap row `┊` (in
`FAINT`, in the name column's last cell) wherever floors are skipped, including above the
first row shown and below the last when floors lie beyond them. The strip is centred
vertically in the map's rows.

- Your floor: the row's cells right of the `│` take the selection ground (20, 32, 44);
  the name is bold white, the marker `@` white.
- The floor the track wants (`track::floor_wanted`, when it isn't yours): the name bold
  cyan, the marker `⌖` cyan. A stairs leg carries the floor it goes to
  (`Through::Stairs { up, to }`), so the stairs want a floor as the lift does, here and
  on the lift panel.
- The floors (and gap rows) strictly between yours and the wanted one: the marker
  column is a bold cyan `┃`, the shaft lit.
- Any other floor: the name in `DIM`, the marker column a `│` in `FAINT`.

Which floors get a row, as many as fit the map's rows: yours and the wanted one always;
then the top, then the bottom; then the floors next to yours and next to the wanted one;
then those two away from yours; then every other floor, nearest yours first (the upper
on a tie). Each is added only if the strip, gap rows and all, still fits; one that
doesn't fit is skipped and the next tried. So a building whose floors all fit shows
every floor. In a full pane (13 map rows) a 27-floor megablock wanting L shows
`27F ┊ 21F 20F 19F 18F 17F 16F 15F ┊ 1F L B1`; at 12 rows the same without 15F; in a
squeezed pane (5 rows) `┊ 18F ┊ L B1`.

### 4. The Net and constructs

On the Net grid (`StratumKind::Net`), the close up of §2 with the Net's tiles classed for
it: `NetWall`, `NetGate` and `NetFloor` are building, `NetLane` is road, and the open
lattice (`NetGrid`) is the panel ground, so the lanes and the gates read as streets and blocks.
The deck MAP never shows the Net, so this classing changes nothing there.

In a construct, the plan of §3 over the construct's tiles: `NetWall` is wall, every
walkable tile floor (the corridor colour), in the colours of the building's district
theme, at 2 or 3 tiles a pixel as a floor is; no rooms, no shaft, no stairs or lift. The
target's marker and `@` are drawn as on a floor.

### 5. The bottom row

The last row of the pane, on the sidebar's ground, one cell in from each side.

**Left: where you are.**
- On a stratum: the name of the district under you (the player's tile, else the view's
  centre), in its theme's accent × 0.85. Where no district is (open water), the stratum's
  label in `DIM`.
- On a floor: the floor's name in bold white (`18F`), then ` · ` and the room you are in
  (`home 1814`, `corridor`, `open plan`) in `DIM`: the room's kind label and its number
  when it has one, as the inspector writes it. In no room, the floor's name alone.
- In a construct: `construct` in `DIM`. On the Net grid, as on any stratum, the
  district under you; `the Net` in `DIM` where there is none.

**Right: the track.** Nothing when nothing is tracked. Else, by the first that applies:
1. The track has no leg: `⌖ no way` (no way from here) or `⌖ jacked in`, the `⌖` in cyan,
   the words in `DIM`.
2. The target stands on this map: the target's marker, a space, the leg's length as
   the JOBS pane gives it (`73 m`), a space and the bearing from you to the target as one
   of `↑ ↗ → ↘ ↓ ↙ ← ↖` (north up), the number and arrow in `DIM`. No arrow when you stand
   on it.
3. Otherwise: `⌖` in cyan and what comes after the leg in `DIM` (`track::next_now`:
   `lift to L`, `stairs up`), else the leg's length (`105 m`).

The right side keeps all its cells; the left is cut to fit with `…` when the two would
meet, keeping a space between them.

**The target's marker** is what the deck MAP's listings call the target: a building's
listing glyph and colour (`listings::Kind`, your home as `⌂`), a job's or the story's
`◆` in magenta, a picked spot `⌖` in magenta. One function in `track::listings` gives it
from the track's source and target, so the deck MAP and the minimap agree.

### 6. The deck MAP without "where you have been"

- The pixel colour (§2) no longer reads `Visibility`: every pixel is its class colour,
  as a remembered one is today. Nothing brightens where you can see now. The `vis`
  argument and `vis_of` go.
- The header loses ` · lit: remembered · faint: the city grid`; it reads
  `CITY · STREET · zoom 2`.
- Nothing else on the deck MAP changes: markers, the walk's glow and pulses, the cursor,
  district names, the places pane.

### 7. Where the code goes

- **One pixel module** in `render/` holds the shared colouring: a stratum pixel from its
  tiles (§2, used by the deck MAP and the close up, §4's Net classing in it), a floor or
  construct pixel (§3), and painting pixels as `▀` cells. The deck MAP's `shade`,
  `Class`, `ground`, `NEUTRAL`, `WATER` and `paint` move there from `screens/atlas/map.rs`.
  The deck MAP keeps its layout, pan, markers, glow and names.
- **`render/minimap.rs`** is rewritten as the close up, the plan and the shaft. It draws
  from what it is handed and never reads `AppState`.
- **The sidebar** hands the minimap what it needs, worked out where `SidebarView` is
  built (`screens/explore.rs`): the leg when it is on this map, the target's tile and
  marker when it is on this map, the floor the track wants, and the bottom row's right
  side. `SidebarView::view` and `player` stay. The coordinate row goes.
- Drawing is cheap enough to do every frame: about a thousand pixels of up to nine tiles
  each. Room lookups on a floor go through a per-draw index of the floor's rooms rather
  than a scan of every room for every tile.
- Where nothing is (off the stratum, outside the floor), the minimap shows the sidebar's
  own ground (`StylePack::background`) and the deck MAP its panel colour: the shared
  pixel functions return no colour there and each map fills its own.

### 8. Tests and shots

- The close up: the `@` sits on the pane's centre cell (within a cell); a pixel's colour
  equals the shared function's for its tiles; the view's corners land on the cells of the
  view's corner tiles; the walk's pixels are mixed toward cyan; a target off the close up
  pins to the border on the side it lies; the cells cover the same tiles after a step.
- The plan: a floor that fits is drawn whole at 2 or 3; walls one tile thick show; the
  lift, stairs, home and `@` are drawn; a too-big floor follows you; a construct draws.
- The shaft: the row choice (a pure function of the building's floors, yours, the wanted
  one and the rows) for a tall building at 12 and 5 rows, a short one, a basement; you and
  the wanted floor always shown; the lit run between them; no strip in a one-floor
  building.
- The bottom row: the district's name; a floor's name and room; each of the right side's
  three cases; the left cut with `…`.
- The deck MAP: `pixels_class_by_their_tiles` without the seen levels, and the header
  tests (`atlas/mod.rs`, `deck.rs`) without the removed words.
- `explore.rs`'s coordinate-row assertion moves to the bottom row.
- Shots: the existing street and interior shots now carry the new minimap. Add one from
  the flat with a target tracked (the strip lit) and one from a one-floor place; check
  both at 120 × 40 and at 100 × 30, where the pane squeezes to 5 map rows.
- `every_drawn_character_has_a_glyph` covers the new glyphs (`⇅ ≡ ⌂ ⌖ ┊ ┃` and the
  arrows).

## Calls I made

1. **The walk is still on the minimap**: no pulses, no ripple. The main view animates the
   track; a 40-cell pane pulsing beside it would pull the eye.
2. **The close up doesn't clamp at the stratum's edge.** You stay in the centre, which is
   what "centred on you" promises; the deck MAP is for seeing the edge.
3. **The origin snaps to the cell grid**, so the ground doesn't shimmer as you walk; the
   price is the centre wandering up to a cell.
4. **Only you and the target are marked outdoors.** Places (shops, clinics) stay on the
   deck MAP, per the anti-goal.
5. **The target off the close up pins to the border** in its own marker; the bottom row's
   arrow gives the bearing.
6. **The bottom row's right side** reuses the JOBS pane's length (`track::distance`) so
   the two never disagree, and uses the leg's next step indoors ("lift to L") where the
   target is on another map.
7. **The target's marker comes from the listings' kinds**: one function so the deck MAP
   and the minimap draw the same glyph for the same thing.
8. **The coordinate row goes.** The inspector (dev) still shows the tile; the bottom row
   is worth more to a player.
9. **Indoors there are no main-view corners**: the plan is small and whole, as mocked.
10. **The Net grid and constructs**, not mocked, get the same two looks (close up and
    plan) with their tiles classed in the obvious way, so no map falls back to the old
    minimap and the old code can go.
11. **The floor plan is coloured by the building's district theme**, not a fixed indoor
    palette, so a floor looks like the block it sits in on the street.
12. **The shaft's floor choice** goes yours, wanted, top, bottom, then neighbours, skipping
    what doesn't fit, as mocked at 12 rows; at 5 rows it keeps the floors that matter.

13. **The minimap's `@` is not bold** (see §2).
14. **The stairs win a shared cell when the walk takes them** (§3); otherwise the lift
    does, as mocked.

## The builder check

A fresh reader planned from this document and asked two things; both were mine to
settle, and are settled above:

1. *When the walk takes the stairs, does the shaft still show the floor it heads for?*
   Yes: a stairs leg now carries its floor, and `floor_wanted` reads it (§3).
2. *Does the floor choice stop at "two away from yours" even when every floor fits?*
   No: after those, every other floor nearest yours, so a building that fits shows every
   floor (§3).

It also settled, as its own calls: `NetFloor` counts as building on the Net grid; the
stairs are drawn only where the floor has a stair tile; a floor too big for 3 follows you
on the same cell grid as the close up; a target off the plan pins to the border as on
the close up; the target's tile follows the deck MAP's rule (a building's door).

## Left to the builder

- The pixel module's name and the exact shape of what the sidebar hands the minimap.
- How the strip's rows are computed, as long as §3's choice order holds.
- Whether the room index is a grid built per draw or cached with the floor.
- How a test reaches each case (the shot helpers `walked_city`, `stand_on`,
  `tracked_from_the_flat` and friends in `screens/shots.rs` are there).
