# The world one level down: open ground, its Net, and POIs

Date: 2026-11-04
Status: agreed in brainstorming; plan after save the run (#2) lands
Builds on: `2026-11-04-world-generator-design.md` (slice 1, built: `citygen::overworld`)
Mocks: `2026-11-04-overworld-mock/round3.html` (the agreed look)

## Why

Every overworld tile zooms down to a walkable detail map, and walking off a detail
map's edge brings you into the next tile's. Jeffrey: "Yes, in to the next one and they
need to properly connect." That holds for the ground and for the Net. Out in the open
there is little to do in the Net, "maybe there's a camera hanging somewhere that you
can connect to, or an Outpost or other kind of POI, but a barren net must also be
properly represented in the same way." POIs are where the open ground gets interesting:
round 2's Kilo-9 truck stop is the picture of it.

This spec covers the detail maps of every tile that isn't a city's: their ground, the
roads, rivers and coast that run through them, the POIs on them, their Net, and how they
join. Cities grown into the world are `2026-11-04-cities-in-the-world-design.md`; the
overworld screen and moving between the levels are
`2026-11-04-overworld-screen-design.md`.

## Goals

- Every open tile has a detail map built from the overworld plan and the seed: the same
  map every time, on every platform.
- Neighbouring tiles join with no seam: ground texture, roads, rivers, the coast and the
  Net carry straight on across every edge, whichever side was built first.
- POIs are places one level down, each kind with its own layout; buildings at a POI can
  be entered like the city's.
- The Net covers the open ground as round 2 drew it: a grid that fades away from
  signal, trunks along the highways, small constructs at POIs, lone nodes on spurs.

## Non-goals

- People out in the open (POI staff, nomads, traffic as people): later, with the
  off-screen world spec. Traffic stays the visual overlay it is in the city.
- Vehicles, fuel, food, encounters.
- Undercity, Skyways and Heights outside cities.
- Anything in the Net beyond what's there: no new verbs, programs or ICE for the open.

## Anti-goals

- Visible tile seams: a road that jogs, a river that ends, noise that changes at a line.
- Open ground that reads as a busy texture: Jeffrey picked the sparse ground "because it
  reads less busy", with a little relief.
- A barren Net drawn as nothing: empty Net is drawn, faintly.

## Decided (Jeffrey)

- **Scale: one overworld tile is 8×8 detail tiles.** A detail tile is the street's
  tile, so today's 240×120 city is 30×15 overworld tiles, the size cities are on the
  sheet; on foot the next city is a city-width or two away.
- Walking off a detail map's edge enters the next tile, properly connected.
- Ground look: round 2's sparse ground with a little relief (look "T" in `ow2.py`).
- Empty Net: the fading grid (round 2's B), "pretty much the way we have it".
- The Net connects across tiles like the ground.
- Everything comes from the run's seed; the same world on every platform (players will
  share seeds).
- Tiles are built when first needed and kept from then on, and saved with the run (the
  world is part of save the run's snapshot).

## The detail world

### Coordinates

Detail positions are world-wide: overworld tile `(tx, ty)` covers detail tiles
`8tx..8tx+8` by `8ty..8ty+8`. Everything that must agree across an edge (noise, edge
crossings, hashes) is computed from world-wide detail coordinates and the seed, never
from a tile's local ones, so two neighbours can't disagree.

### What a tile is built from

Its overworld tile (water, ground, elevation, road and river links, a POI) and its
eight neighbours' (for blending at the edges), plus the seed. Nothing else, so a tile
can be built in any order.

### The edge contract

Each shared edge between two tiles has one set of crossings, derived from the plan and
a hash of the seed and the edge, so both sides compute the same:

- **Road:** where an overworld road link crosses the edge, the road crosses at **the
  middle of the edge, always**, as two lanes with a gravel shoulder each side (round
  2's Route 9). So a road that runs straight on the overworld is dead straight one level
  down, tile after tile, and so is its Net lane: nothing to steer round in a car, no
  jog in the trunk. Jeffrey: winding straight roads are "really awkward" for cars and
  look "really odd" in the Net.
- **River:** where a river link crosses, at a hashed position near the middle and at
  its width there (see Rivers below), so both banks meet across the edge.
- **Coast and lake shores:** where the shoreline meets the edge, the land/water point
  along it.
- **Elevation:** the tile corners' heights, shared by the four tiles at each corner, so
  rises blend.
- **Ground cover:** noise in world-wide coordinates, so texture runs straight on.

Inside the tile, a road runs straight between crossings on opposite edges; where the
overworld road turns, it bends **once**, as a clean corner (a short curve of road
tiles) in the turning tile, never as a jog; a junction meets at the tile's centre.
Roads are straight lines with corners, like the city's streets. A river runs between
its crossings with a meander; a shore as a natural line between its edge points (Water
below).

### Water

Jeffrey: lakes "cut of harshly", and a river you can see from the overworld shouldn't
be a one-tile creek one level down.

- **Shores:** the shoreline is curved by noise at a few scales (coves, points, a spit
  now and then), not a blend of the overworld's tile shapes. Between water and land a
  **shore band**: a tile or two of shallows (lighter water), then the shore itself:
  mud and reeds at a lake, sand or rocks at the sea, the spill's poison edge at the
  spill.
- **Depth:** water darkens away from the shore, so a lake reads as a bowl and the sea
  deepens off the coast.
- **Rivers are wide:** about 3 detail tiles near the spring, widening downstream to 6
  or 7 near the mouth, nearly the whole tile; banks of mud or reeds, shallows along the
  edges, depth down the middle. A bridge is a real span of road over open water (with
  rails); fords only in the narrow upper reaches. A wide river is a barrier on foot,
  which gives its bridges a point.

### Ground, one level down

Each overworld ground kind has its detail look (round 2's palettes and the street's):

| Overworld | One level down |
|---|---|
| Flats | dark earth, a dot of dust here and there, a few low rises in `░`, the odd rock |
| Scrub | the same with scrub `"` `,` in patches |
| Fields | rows of crops along the field's axis, a track between them |
| Salt | pale crust, `░` and `·`, flat |
| Spill | poison-green seep, pools `≈`, nothing grows, a fence or warning sign at its edge |
| Hills | rises and rock outcrops, scrub in the folds; walkable |
| Mountains | rock walls (impassable) with passes and gullies (walkable); a road through mountains keeps its way open |
| Peak | rock, impassable |
| Sea | water, impassable; a beach or rocks at the shore |
| Lake | water, impassable; reeds at the shore |
| River | wide water with banks and shallows (see Water); crossed by bridges where roads cross it, fords only upstream |
| Dry wash | a dry sandy bed, 3 detail tiles wide (4 in its lower half), walkable, thicker scrub along its banks; where a highway crosses, a culvert (see Dry washes) |

Dressing is sparse: power lines along some roads (pylons), rocks, wrecked cars near
roads, trash near anything built. At night only POIs and the roads near cities are lit.

### Dry washes

A wash runs between its crossings like a river (a hashed crossing near each edge's
middle, so both tiles agree, with a meander), but dry: a bed of sand and gravel, banks
with more scrub than the ground round them, here and there a puddle after rain. Where a
highway crosses a wash it doesn't bridge it: the road runs straight over on its
embankment, both lanes unbroken, and a culvert carries the bed underneath, its two ends
grates on the shoulders either side (round 2's Kilo-9 had one). Jeffrey: "it adds great
variation and it sells what you came up with earlier."

### POIs, one level down

A POI sits on its overworld tile and may spill into neighbouring tiles (a truck stop is
about 36×15 detail tiles, so it spans several). Each kind has a layout generator with
variation; buildings are entered through the city's interior generator (a store is a
shop, a booth is an office):

| Kind | One level down |
|---|---|
| Truck stop | forecourt with pumps under a canopy, a store, a back lot of parked rigs behind a fence, lamps, a camera, a roadside sign (round 2) |
| Checkpoint | a gate across the road with a barrier, booths both sides, floodlights, cameras, a fence running off into the ground |
| Relay mast | a fenced mast on high ground, an equipment cabinet, a service track down to the nearest road |
| Nomad camp | vehicles drawn up in a ring, tents, fire barrels, no lamps |
| Wreck | a crashed AV, tanker, rig or drone and its debris field |
| Dead arcology | the base of a sealed megastructure: walls, a closed gate, rubble; not enterable in this slice |
| Solar array | rows of panels, a fence, a maintenance shed |
| Water plant | tanks, pipes, a pump house, a fence, by its lake or river |
| Test range | a fenced range on or by dead ground, craters, warning signs, a watchtower |
| Rig | an offshore platform on the sea; reachable only from a pier on the coast tile nearest it |

### Kept tiles

A tile is built the first time it comes within reach of the view (the tiles the screen
can show, plus a ring around them) and kept from then on, with whatever the player
changed. Tiles never reached are never stored: the seed rebuilds them identically. The
kept tiles are part of the run's saved world (save the run, #2).

### The Net in the open

Each tile has its Net layer, built with its ground and joined across edges the same way:

- **Trunks:** every highway carries a backbone lane (round 2: `═` in the lane colours)
  along its road, with packets on it; trunk crossings are the road crossings.
- **Constructs:** each POI's buildings that have systems get a small construct (round
  2's Kilo-9), with its nodes and a gate on the wall facing the road; a spur lane runs
  straight from the gate to the trunk, so **every POI on the Net is wired to it**
  (round 3's Kilo-9 is the reference; Jeffrey: "that was perfect").
- **Lone nodes:** cameras on poles by checkpoints and stops hang off their POI's spur on
  a short light line; ad servers in billboards and a relay mast's node each have a spur
  of their own to the trunk.
- **The grid:** drawn only near signal (lanes, constructs, nodes), fading to void a few
  tiles out, with the odd stray dot beyond (round 2's B).
- **Barren but walkable:** the Net is drawn everywhere in the open and joins across
  edges. How far a runner may walk it from the body is decided in
  `2026-11-05-streamed-world-design.md`: no reach limit, the void walkable too.

## Constraints

- Deterministic and portable: the slice-1 rules (IEEE basics only, no platform maths),
  with world-wide coordinates for everything shared; pinned fingerprints for a few tiles.
- A tile builds in well under a millisecond on average; a POI's tiles in a few.
- Glyphs and colours from the street's palettes and round 2's; every glyph has a font in
  the window.
- Shared code stays portable; both front ends checked.

## My calls (Jeffrey can change any)

- Rivers widen downstream from 3 to 6–7 detail tiles; fords only upstream.
- Mountains have walls and passes; peaks, sea and lakes are impassable on foot.
- POI layouts are generators per kind with variation, not fixed templates.
- The rig is reached from a pier on the coast.
- The dead arcology isn't enterable yet.
- The open Net's reach from the body: none, decided in
  `2026-11-05-streamed-world-design.md`.

## Left to the builder

How tiles are stored and streamed, the exact hash and noise functions within the
portability rules, layout details per POI kind, the dressing densities (tune on shots),
which interior generator kinds stand in for POI buildings.
