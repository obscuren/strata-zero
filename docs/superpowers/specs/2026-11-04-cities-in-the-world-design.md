# Cities grown into the world

Date: 2026-11-04
Status: agreed in brainstorming; plan after save the run (#2) lands
Builds on: `2026-11-04-world-generator-design.md` (slice 1, built),
`2026-11-04-world-tiles-design.md` (open ground and the edge contract)
Mocks: `2026-11-04-overworld-mock/round2.html` and `round3.html` ("Where Saintburg ends")

## Why

Today a city is a rectangle, with a bay on one edge or a river across it, and nothing
past its edge. Jeffrey wants "a more natural border instead of a rect … how a city is
expanded in certain areas and some not … like a real city", and the city sits in a
world: "if you check the overworld map, it had a river, a bay area.. and those contours
should also be present in the detail map." The overworld already grows each city into
its shape (slice 1). This slice makes the city generator build the city that shape
describes, with the world's coast, river and hills, and its edge giving way to the open
ground round it. Round 2's outskirts ("those city outskirts are beautiful") are the
picture of the edge.

## Goals

- A city's detail map covers its block of overworld tiles and takes from the world:
  its outline, the coast and bay, the rivers through it, lakes, hills, dead ground, and
  the highways that reach it.
- The towers stop where the shape says; past them the outskirts (lots, yards,
  scrapyards, shanties, billboards, the last lamps) thin into open ground, and the
  block's edge meets the neighbouring open tiles by the edge contract.
- The Net does the same: districts' constructs and tint inside, fading through the
  outskirts, the arterials' lanes running on as the highways' trunks.
- Every city in the world is generated this way, each from the world's seed, each
  different in size and shape.

## Non-goals

- Lore between cities, their stories, fixers and jobs (later; other cities are "just
  another city" for now).
- Running more than one city's people at once (the off-screen world is its own spec).
- Changing what is inside a city: buildings, interiors, the strata, the Net's nodes all
  stay the city generator's as they are.

## Anti-goals

- A rectangle with a ragged fringe: the shape comes from the world, the outline is the
  city's.
- A bay or river that doesn't match the overworld's.
- A hard line where the city's map ends.

## Decided (Jeffrey)

- The city's detail map follows the overworld's contours: its coast, bay, river and
  hills are the world's.
- The outskirts as round 2 drew them, on the street and in the Net.
- Other cities are generated like today's city; lore links them later.
- "For our largest city it should use the large city gen": the home city's map is the
  large generator's size (320×160), the others smaller, each by its own size.
- A lone city (dev, the `gen` CLI, tests) is a one-city world, which is easy for testing.

## The city's map

### Its block and size

A city's block is the bounding box of its own tiles on the overworld (not its outlying
towns'), plus a margin of two tiles all round for the outskirts to fade out in. Its
detail map is that block × 8 detail tiles, so every city's map has its own size. At
slice 1's sizes the home city's block already comes to about the large generator's size
(measured on the sheet: 248–400 × 128–208, about 304×176 on average), which is what
Jeffrey asked for; the middling cities come out between the small and medium sizes.
*(My call: the overworld's sizes stay as they are; the time to generate the home city
is about today's large city's.)* A block larger than the generator's limit (1024×512) is
clamped by shrinking the city on the overworld, never by cutting the map.

**Outlying towns** get small maps of their own, built the same way from their own tiles
(at the smallest size the generator allows, 80×40, or their block if bigger), named for
their parent city's district or the road *(my call)*.

**One owner per tile.** A detail tile belongs to at most one city's map. Where two
cities' blocks (or a city's and a town's) would overlap, the smaller one's block stops
halfway between them, and whatever of it lies beyond is the other's or the open
ground's *(my call)*. A POI that falls inside a city's block is laid out by the city's
map, in its outskirts.

### What it takes from the world

- **The mask:** every detail tile gets the city's density from its overworld tile,
  smoothed across tile lines (bilinear between tile centres, plus noise), so the outline
  runs through tiles, not along their edges.
- **Water:** the coast and bay from the sea tiles (the edge contract's coast points on
  the block edge, a noisy shoreline between), rivers along the world's river courses at
  detail scale (the contract's crossing points and widths), lakes from lake tiles.
  This replaces `WaterMode` for a city in a world; a bay and a river together is normal.
- **Ground:** hills and mountains as the open tiles draw them; the city thins on slopes
  (its density already does) and streets bend round them.
- **Dead ground:** where the spill or salt reaches the block, it is the open ground's,
  and nothing is built on it.
- **Highways:** every route crossing the block edge enters at its contract crossing and
  becomes an arterial to the core; the arterials between districts follow, as now.

### Inside the outline

Districts are seeded inside the mask, their themes guided by the overworld's district
colours so the two levels agree. Streets, blocks and lots are laid only where the
density is above a threshold; building heights and kinds follow the density (towers at
the core, low-rise toward the edge). Everything else the city generator does is
unchanged.

### The outskirts

Where the density is low (a belt of a few blocks inside the outline), the generator lays
round 2's outskirts instead of full blocks: parking lots, a body shop or two, fenced
scrapyards, shanties round fire barrels, billboards facing the city, the last lamps
along the avenues. Sidewalks end and the arterials run on as highways with gravel
shoulders. Past the outline, the block is open ground drawn as the open tiles are, so
its edge meets them seamlessly.

### The Net

Districts' Net as now inside the outline. Through the outskirts the district tint and
the grid fade (round 2); small buildings get small constructs, shanties none (one may
splice a stolen line off a trunk); the arterials' lanes run on as the highways' trunks
at the block edge.

### Cities in a world

Every city on the overworld is generated this way from the world seed and its index.
The home city is built when the run starts; another city is built the first time the
player comes near it (on the overworld or on foot), behind a short generation screen if
it takes noticeable time *(my call)*, and kept from then on (saved with the run).
Only the city the player is in runs its people *(my call; the catch-up when you return
is the off-screen world spec)*.

### A lone city

`GenParams` and `strata gen` keep working: they build a one-city world around the city,
sized to it, with its water mode turned into the world's plan (a bay is a sea side with
a bay; a river is a river through the block). The city then comes out of the same path
as every other.

## Constraints

- Deterministic and portable (slice 1's rules); the city fingerprints are re-pinned once,
  on purpose, when this lands, and say so in the commit.
- Generation time for a large city stays what it is today, or close.
- Glyphs and colours as the street's.

## My calls (Jeffrey can change any)

- The block margin is two tiles; towns have their own small maps, outside the block.
- The overworld's city sizes stay as slice 1 has them (the home block is already about
  the large generator's size).
- One owner per detail tile; overlapping blocks stop halfway; POIs in a block are the
  city's.
- The mask is smoothed bilinear-plus-noise between tile centres.
- Another city is built on first approach, behind a short screen if needed.
- Only the city you're in runs its people.

## Left to the builder

How the city generator's stages take the world (a `CitySite` input in place of the
rectangle and `WaterMode`), the density thresholds for blocks and the outskirts belt,
outskirts layouts, how arterials meet the highway crossings, how the lone city builds
its one-city world.
