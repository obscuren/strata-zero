# The Net, upgraded: no light rule, the relief look, lanes that carry you

Date: 2026-11-03. Part one of two. Mockups: `2026-11-03-net-look-mock/` (`round1.html`:
now, A signal, B relief, C wire map, and eight ideas to tick; B was chosen.
`cord.html`: the cord. `round2.html`: inside a construct in B, and a lane ride). Part two,
what hacking a node means, is still being brainstormed in
`docs/ideas/2026-11-03-net-nodes.md` and is not built here.

## Why

The Net today is a copy of the street layer: the same footprints, and the same light
rule (a sight radius, walls casting shadow, everything out of sight fading to
memory). Jeffrey: "neat, but I feel we can do better", and first of all "we should not
have the light rule applied there". From the outside nothing tells you what a construct
is or how hard it is; the grid between constructs is a slow walk; on the Net the
sidebar's minimap still shows the street and HERE says `nothing here`.

## What Jeffrey decided

- **No light rule on the Net, inside constructs too.** No sight radius, no shadow
  cast, no fading to memory: on the open grid and inside a construct you see
  everything on screen.
- **The look is B, "relief".** The void is dark and the Net lights itself: the grid
  shows only near data, lanes are beams, constructs rise in steps by security, the
  black-ICE towers glow red. Inside, the construct's body takes its security's colour,
  rooms are carved out of it and lit by what they hold, and the stretch of spine each
  piece of ICE watches is tinted.
- **The construct card**: HERE reads the construct you are at.
- **A minimap of the Net** instead of the street under it.
- **Ride the lanes**: lanes are the fast way across; the open grid stays a walk.
- **Traffic**: packets run the lanes (a look for now; intercepting them is part two).
- **The cord** stays "for now": "it might be messy so it's something that should be
  easily disabled/removed."
- **Terminal nodes in their own colour**, "so they are distinguished from the other
  nodes without looking".
- **The frame stays.** Header, sidebar and footer keep their places; only what they
  say on the Net changes. (A separate jacked-in frame was turned down on 2026-10-16.)

## Goals

- On the Net stratum and in every construct, every tile is visible all the time; the
  map draws nothing as remembered.
- The open Net and construct insides look like the B mockups, at 120x40 and at the
  minimum terminal (100x30), in the terminal and in the window.
- A player can read a construct's security tier from its outline alone, and its name
  from the map when it is large enough to hold one.
- Crossing the open Net along lanes takes a keypress per stretch, not per tile.
- The cord can be switched off in Settings and removed by deleting one module and one
  call.

## Non-goals

- What a hack does to a node (part two): no new programs, verbs or effects here.
- Intercepting packets; packets carry nothing.
- New ICE behaviour; ICE sees exactly as today (`net::sees`).
- C, the wire map; scaling the Net's tiles; a new frame.
- The street, floors and the lot keep their light rule.

## Anti-goals

- The Net must not get harder to read than today: no colour carries meaning on its
  own where a glyph carried it before (nodes keep their glyphs; tiers keep distinct
  wall glyphs as well as colours).
- No symbol a font lacks (`every_drawn_character_has_a_glyph` must stay green).
- No named ANSI colours; everything new is exact `Rgb`.

## The rules

### Sight

- Standing on the Net stratum or in a construct, every tile of that map counts as
  visible. Nothing on the Net is ever drawn as remembered.
- What leans on sight keeps working, with everything in view: the ICE lines in HERE
  list every standing piece; the inspector sees all tiles. What a program or the fire
  cursor can reach was "in your sight within range"; on the Net it is now "within
  range with no construct wall between" (`line_clear_in`), so a piece behind a wall
  stays out of reach. ICE's own sight (`net::sees`) is unchanged.
- **Ping** is unchanged: in a construct it still glows every node for a while (and
  warms the trace), which still points them out; on the street it is as it was.
- **Signal ghost** felt awake ICE through walls, which everyone now sees. It now marks
  where each awake piece steps next, on the map. (Call made, see below.)
- Low-light only ever widened a sight radius; on the Net it no longer has one to
  widen and stays a street talent.

### Security tiers

One tier per construct, from its subnet's security, using the construct thresholds
where they exist:

| Tier | Security | Wall set | Colour | Steps |
|---|---|---|---|---|
| open | below 100 | light, dashed straights `┄ ┆` | signal, dim (63,168,143) | 1 |
| guarded | 100 to 159 | light | signal (0,255,200) | 2 |
| ICE | 160 to 229 (white and grey ICE) | heavy | ice blue (159,230,255) | 3 |
| black | 230 and up | double | red (255,40,72) | 4 |

The tracked building (the job's construct when you track a job, or any building you
track) draws in amber (255,176,0) in place of its tier colour, glyphs unchanged.
District hubs keep double walls in white.

### The open grid

- **Void**: near black (1,2,4). The lattice dots show only within two tiles of a
  lane, a construct or a hub, brighter at one tile than at two; beyond that the void
  is empty. A faint glow (signal at 9% at one tile, 3% at two) lies under the dots.
- **Lanes**: double lines as today, signal mixed a quarter toward white, on a bloom of
  signal at 22%.
- **Constructs, seen from outside**: the outer wall in the tier's set and colour;
  inside the footprint, concentric rings, one per step of the tier, as many as fit
  (a ring needs at least 3 by 1 inside it), each a light box-drawing ring brighter than
  the one outside it, each band's floor a little brighter; the plate inside the last
  ring brightest. The name sits centred on the plate, bold, in caps, wrapped to the
  plate's width and cut with `…`, kept off node tiles. Blocks of flats read `FLATS`;
  your home block reads `YOUR BLOCK`.
- **Gates** `▯` in white on a glow of the tier colour.
- **Hubs**: double walls in white, `⬡`, and faint rings of dots in the void around them
  (at about 5, 9 and 13 tiles, measured with a cell twice as tall as it is wide).
- **Nodes** keep today's glyphs. Server, vault and hub in white; an ICE node in the
  colour of the piece at that gate (white or grey; gates never hold black).
  **Terminals** (`◇`) in their own colour everywhere on the Net, open grid and
  constructs: violet-blue (140,150,255). Other nodes (locks, cameras, ad boards, lamp
  grids, lift controls) in their wall's colour mixed halfway to white.
- **Packets**: a white bold head on a signal glow, a two-tile tail fading behind it,
  moving one tile every few ticks. A packet leaves a node and runs the lane to another
  node that lane joins it to (through junctions, never past another node), then picks
  the next at random, not straight back while it has a choice; packets cross now and
  then. One node in two round the view sends one. A look only. (Changed 2026-11-04 from
  one packet per straight run, which ran node to junction: Jeffrey found that awkward.)
- **You**: `@` in signal, bold.

### Inside a construct

- **The body** (wall tiles not touching anything open): `░` in the tier colour at
  half, on the tier colour at 7% over the void.
- **The outer border** in the tier's set and colour; **inner edges** (wall tiles
  touching a room, the spine or a door, diagonals included) as light lines in the tier
  colour at 75%, joined along the edge.
- **Rooms**: floor lit cyan at 10%; the vault room amber at 16%. Nodes as on the open
  grid; server and vault bold, the vault `⌗` amber.
- **The spine** as a lane; **doors** `▯` white.
- **ICE** `◈` (today `▣`; `◈` is ICE everywhere on the Net now, and HERE's ICE lines
  say `◈` too): white ICE near white (240,244,248), grey ICE magenta (255,0,200), as
  today, black ICE red; each on a glow of its colour.
- **The watch**: each standing piece tints the spine tiles it would see you on (its
  row or column, within its sight, a clear line, by today's `sees` rule without the
  mask), in the piece's colour: 25% for white, 30% for grey, 45% for black. A tile two
  pieces watch takes the darkest piece's tint (black over grey over white).
- **The tracked building is not amber inside**: the body keeps its tier colour; the
  vault keeps the amber.
- **The void around** as on the open grid.

### The sidebar on the Net

- **Header, second line**: today it reads the light and the eyes (`dark · unseen`).
  On the Net it reads the place and the eyes: `grid` on the open Net, `construct`
  inside, then `unseen`, or `hunted` while ICE hunts you.
- **HERE, the construct card.** On the open Net: the construct whose gate is nearest
  within 12 tiles (the one the footer's TRACE already names), or `open grid` when there
  is none in reach. Inside a construct: that construct, above today's TRACE and ICE
  lines. Three lines on the open Net, the first two inside:
  - its name in caps, bold, in the tier colour (amber when tracked, open Net only),
    then `· kind` and, on the open Net, an arrow toward its gate;
  - `SEC n · ` and the ICE it keeps: `no ICE`, `gate white`, `gate grey`, and when it
    has a vault guard ` · vault grey` or ` · vault black`; with no ICE, the tier word
    (`open`, `guarded`) instead;
  - `NODES` and a count per node glyph (`▪ 5  ◆ 1  ◇ 1`), then ` · alarm` while its
    alarm is up, ` · backdoor` while you hold one there, ` · job` when tracked for a
    job.
  The node or ICE under the focus keeps its line after the card. On the Net HERE may
  take up to 10 rows (7 elsewhere); the alarm inside reads in the TRACE lines
  (`ALERT · n min`) as today.
- **The minimap**: on the open Net, 3 by 3 tiles to a half-block pixel, centred on you:
  construct walls in the tier colour, their floors dark in it, lanes signal, hubs
  white, you white, your port green, the tracked building amber, the void dark with the
  district's accent at 12%. Inside a construct, the construct's plan at a tile a pixel,
  centred: body and edges in the tier colour (dark and less dark), spine signal, the
  watched spine tinted, rooms cyan, nodes white, the vault amber, ICE in its pieces'
  colours, you white. The caption under it names the district (open Net) or the construct.

### Lanes carry you

- On the open Net, a step that runs along a lane (from a lane tile, toward a lane tile
  in the same direction) does not stop there: you ride on, a tile at a time, until the
  next stop. Riding is the **Data surfer** talent's (ICE tree, the 12-point tier, under
  Trace eater; 2026-11-04): without it a lane is walked a tile at a time. **Slipstream**
  below it gives each tile ridden a 5% chance that every trace on you sheds a tenth of
  its heat.
- **Stops**: a junction (a lane tile with a lane or gate beside it off the ride's
  line), a hub's gate, a node, the lane's end, the tile before anything that blocks,
  and the lane tile before a construct's gate: a ride never enters a construct; the
  next step does, as today. A ride follows the lane round a bend where it does not
  branch.
- **Any key stops a ride** at the tile you have reached, and does nothing else.
- **Speed**: a tile a tick; nothing else is spent (the open Net runs in real time, a
  walked tile costs only its keypress, so the gain is one keypress per stretch). Behind you a short streak of the lane
  lit white, fading over four tiles; the stretch ahead to the stop lit and the stop
  marked.
- Not inside constructs (the spine is walked) and not while engaged.
- A ride is a move: it aborts a run in flight as any move does.

### The cord

- **Only on the open Net**; inside a construct there is no cord.
- **On a rig run** (a route exists): a line from you back to where the run entered the
  open Net: the last relay you landed at, or your rig's port. It follows the shortest
  way along the Net from there to you, lanes preferred (a step on a lane costs 1, on
  open grid 3), worked out when you move. Lane tiles on it keep their glyphs and turn
  white over a white glow; open-grid tiles draw it as light box lines.
- **Hops**: between relays, and from the first relay to your rig's port, a dotted line
  of `·` in cyan, straight. Relays `▣` cyan, your rig's port `▣` green
  (124,255,154).
- **A trace walking home**: the relays it has passed and the stretch from you to the
  relay it sits on turn magenta; the relay it sits on draws on a magenta glow.
- **From a street terminal** (no route): the cord leads back to the port you jacked in
  at, green.
- **Off screen**: lines run to the edge of the map; nothing more.
- **On and off**: a Settings row `Cord on the Net` (on by default) hides it. The cord
  lives in its own render module with one call from the map draw; removing it is that
  module, that call and that setting.

## Calls made

- **Riding has no key of its own.** The mock said Shift+direction, but `J` (Shift+j) is
  already jack out, so Shift can't ride south. A step along a lane carries you instead,
  and any key stops it.
- **Tier boundaries.** ICE (160), grey (200) and black (230) are the construct
  thresholds; the split between open and guarded at 100 is new, for the look only.
- **Signal ghost** gets a new effect so it doesn't die: each awake piece's next step is
  marked on the map with a small `·` in its colour. Its description stays (it fits).
- **The cord is computed, not recorded**: the shortest way from the entry point to
  you, so it never loops and nothing is saved.
- **Terminal colour** violet-blue (140,150,255): not signal, not ICE, not amber, not
  magenta.
- **The construct card** on the open Net picks the nearest gate within 12, matching
  the footer's TRACE.
- **The header's eyes word** on the Net is `unseen` or `hunted`.
- **Riding never enters a construct**: a ride stops before a construct's gate.
- **ICE keeps today's colours** (white near white, grey magenta, black red) so white and
  grey stay apart; the glyph inside becomes `◈`, as on the open Net.
- **HERE grows to 10 rows on the Net**; the card drops to two lines inside.
- **The fire cursor and program reach use a clear line**, not the old sight, on the Net.

## What's left to the builder

- Where the visibility switch lives (the walker, the map, or both) and how memory is
  kept out of the Net.
- How the renderer is split: the Net's look may replace the Net arms of the tileset
  or sit as its own pass over the stratum and construct views.
- Packet timing and how runs are found each frame (cache by camera if needed).
- The exact streak and stop marks of a ride, within the mock's spirit.
- Tests: keep the existing Net tests green, add tests for every rule above; add
  shots (`net_open`, `net_inside`, `net_ride`, `net_cord`) to `screens/shots.rs`.
