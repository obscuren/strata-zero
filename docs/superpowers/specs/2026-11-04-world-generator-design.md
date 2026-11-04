# World generator, slice 1: the overworld and a browser sheet

Date: 2026-11-04
Status: agreed in brainstorming; ready for a plan

## Why

The city has always been the first slice of a bigger world. Jeffrey wants a world of
many tiles: an overworld you move across, where every tile zooms one level down onto
its own detail map, and walking off a detail map's edge brings you into the next tile.
Cities sit in that world as places that grew, with a natural edge (out along the
highways and the shore, stopped by hills and dead ground), not as rectangles.

The look was agreed in three rounds of mockups in
`docs/superpowers/specs/2026-11-04-overworld-mock/` (`round3.html` holds the picks).
That mock world was laid out by hand: I placed the coast, the bay, the ridge, the
river, the cities, the routes and every POI, and noise and simple rules did the rest.
Jeffrey loves the result and wants to make sure a generator can make maps with the
same hand-tailored look **before** anything else is built on it. So this slice is the
generator alone, and a way to look at many of its worlds at once in the browser and
tune it.

The rest of the overworld (the screen, the detail tiles and their edges, cities grown
into the world, the Net across it, the deck MAP's world zoom, saving the world) gets
its own spec once the generator is tuned. Building that also waits for save the run
(#2), which persists the whole world. This slice touches neither.

## Goals

- `citygen` generates an **overworld** from a seed: the sea and its coast with bays,
  mountain ridges with hills at their foot, rivers that run from the ridges to the sea,
  a lake or two, dead ground (a toxic spill, a salt flat), fields along the rivers,
  cities sited where cities get built and grown into their shapes, outlying towns,
  highways between the cities and off the map's edge, POIs out in the open, and names
  for the cities, routes, regions and POIs.
- Worlds read like the round 1 mock: composed, not noise. A big home city on the water,
  smaller cities upriver and inland, roads that bend round hills, a river you can
  follow from the mountains to the bay.
- **The sheet:** `strata world` writes one HTML page with a dozen seeds' worlds drawn in
  the atlas look (round 1's A), each with its seed and a few numbers, to open in the
  browser, compare and tune.
- Same seed and parameters, byte-identical world.

## Non-goals (later slices)

- The overworld screen, moving on it, going down and back up.
- Detail maps of open ground and their edges, the city generator reading the world,
  the outskirts. (Agreed already: a city's detail map takes its coast, bay, river and
  hills from the world, a bay and a river together, so `WaterMode` and the rectangle go
  for a city in a world; a lone city for dev and the CLI is a one-city world.)
- The Net over the world (round 1's D), the deck MAP's zoom 4 (round 1's C).
- Saving the world, multiple live cities, travel time.

## Anti-goals

- Noise blobs: coasts, cities or dead zones that look like thresholded noise and
  nothing else.
- Cities as circles or squares; roads as straight lines or grid staircases.
- Anything named after a real place or company, or after Cyberpunk 2020 / RTG / CDPR
  proper nouns (no "Badlands", no "Night City"); names come from the flavor pack.
- A generator that only looks good at one seed. The sheet exists to catch that.

## What Jeffrey decided

- One overworld tile is one cell on screen (the atlas look, A); every tile will zoom down
  to a detail map, cities cover many tiles. Everything comes from the run's seed.
- The look is round 3's: the atlas glyphs and colours for this sheet.
- Other cities are generated like the city we have now; lore links them later.
- The Net covers the whole world (later slice), mostly as dead grid in the open.
- Build and tune the generator first, looked at as a browser sheet.

## The world

### Scale

- A tile is drawn as one terminal cell, so on screen it is twice as tall as it is wide.
  **Shapes are measured on screen**: distances count a row as two columns, so cities,
  lakes and bays come out round on screen, not tall. (The city generator does the same:
  its 240×120 reads square.)
- Default world: **160×52 tiles**, the round 1 mock's world. Presets small 120×40,
  medium 160×52, large 220×72, or custom.
- The home city covers about 30×15 tiles, the mock's size. *(My call: that is today's
  240×120 city at 8×8 detail tiles a tile; the detail scale is decided in the next
  slice, nothing here depends on it.)*

### Parameters

`WorldParams { seed, size, cities, flavor }` plus the tuning knobs the builder finds
it needs, with defaults (`cities` 5, range 3..=8). The knobs live in the params so a
test run can change one without code.

### Stages, in order

Each stage is deterministic from the seed and the stages before it (a ChaCha stream
per stage, like the city generator's `stage_rng`).

1. **Sea and coast.** The sea takes one side of the map, sometimes two adjacent sides,
   occasionally none (an inland world: then lakes carry the water rules). The shoreline
   is a noisy line, with 1 to 3 bays cut into it, each a rounded inlet a few tiles to a
   dozen deep. Most of the map is land (roughly 70 to 85%). Small islands are allowed
   off the coast, not required.
2. **Ridges and elevation.** One or two mountain ridges as polylines (a wandering walk
   with momentum, so they curve instead of zig-zag), mostly inland and away from the
   coast. Elevation is low base noise rising a little inland, plus a falloff from each
   ridge with its own noise. Elevation bands give **hills**, **mountains** and
   **peaks**; everything else is lowland for the next stages.
3. **Rivers.** 1 to 3 rivers, each from a spring high on a ridge down to the sea (or a
   lake on an inland world), following the fall of the land with a meander, never
   uphill for long. Rivers may join; one river reaches the sea **at a bay** when the
   map has a sea. Paths are four-connected, so they draw as unbroken lines. A river
   crossing a basin may fill it as a **lake**; 0 to 2 lakes in all, irregular, small.
4. **Ground.** The lowland's cover from moisture noise and the rivers: **dust flats**
   (most of it), **scrub**, and **fields** in strips along the rivers near where cities
   will be. Then the **dead ground**: one **toxic spill** and one **salt flat**, each an
   irregular patch of a few to a dozen tiles across, placed in open land after the city
   sites are known (stage 5 runs its siting first): far enough off a site not to cover
   it, near enough to one city that it can shape that city's edge.
5. **City sites.** Every land tile gets a site score: a river mouth on a bay highest,
   then the sea coast, a river bank, a lake shore, the foot of the hills (near hills,
   not on them), flat ground; never on water, hills, peaks or dead ground. Sites keep a
   minimum spacing so cities don't merge. The best water site is the **home city**, the
   biggest; then cities of middling size up the river and inland, and one small town at
   what will be a junction. City size is a radius on screen: home about 14, others 6
   to 9, the town 4 or 5 (the mock's numbers).
6. **Highways.** The cities as a graph: a minimum spanning tree plus one or two short
   extra links for loops, and two or three **exits** off the map's edge from the cities
   nearest it, so the world goes on. Each link is pathfound (four-connected) across the
   land: cheap on flats, dearer on scrub and fields, much dearer on hills, very dear on
   mountains, never on peaks or the sea, a bridge allowed over a river at a price,
   reuse of an existing road cheap, plus a little noise so the roads bend naturally.
   Routes are named: numbered "Route N", or a coast road where it mostly follows the
   shore; a route keeps its name across the cities it runs through.
7. **Growth.** Each city grows out from its centre by the mock's rule. A tile's score:
   the city's size (with noise) less its distance from the centre; plus a bonus within
   a few tiles of a highway, fading out toward about 2.7 times the size, so the city
   reaches out along its roads; plus a bonus near the shore; less a penalty on hills
   (mountains much more). The sea, lakes, peaks and dead ground never grow. **A river
   runs through a city**: its tiles grow like land and stay river too (a tile can be
   both), so a city on a river spans both banks, as Lowshore does in the mock and as the
   city's detail map will show it. Only the part connected to the centre is kept,
   counting river tiles as connected. Density is the score over the size, 1 at the core
   fading to the edge. Then **outlying towns**: where a highway leaves a city of size 8
   or more, about 2.1 times the size out, a small knot of sprawl (low density), spaced
   apart.
8. **Districts, for colour.** Each city gets a handful of district seeds; the core is
   CorpCore, tiles on the shore lean Docks, the outer ring Megablocks and OldTown, the
   rest from the city's themes. This only colours the overworld; the city generator
   still zones the real districts later.
9. **POIs.** Out in the open, never on a city, water (except offshore) or a peak, kept
   apart, about one per 500 to 700 open tiles. Kinds and where they go:

   | Kind | Glyph | Goes |
   |---|---|---|
   | truck stop | `⌂` | on a highway, about midway between two cities |
   | checkpoint | `╪` | on a highway, a little outside a city |
   | relay mast | `⬡` | on high ground (hills) |
   | nomad camp | `△` | on flats far from any road |
   | wreck | `×` | anywhere open, near a road or not |
   | dead arcology | `▣` | on the coast or flats, away from cities |
   | solar array | `▦` | on flat open ground |
   | water plant | `◎` | on a lake shore or river bank |
   | test range | `◈` | in or beside dead ground |
   | rig | `◆` | offshore, in the sea near the coast |

   Not every kind appears in every world; a world of the default size has most of them.
10. **Names.** Cities from the flavor pack's city parts, unique. Regions get names from
    new flavor parts per kind: the mountain ranges, each bay, the spill, the salt flat,
    the big flats, a river valley. Each region has a label point inside it with room for
    the name. POIs from new flavor parts per kind (a truck stop's code name, a
    checkpoint's corp, a relay's number). All new names go in `flavor/names.ron`.

### What it hands over

An `Overworld` in `citygen` (no terminal dependencies), serde like the rest of the
model so a later slice can save it:

- per tile: water (sea, river, lake, none), ground (flats, scrub, fields, salt, spill,
  hills, mountains, peak), elevation, and the city it belongs to with its density and
  district theme, if any (independent of the water: a river tile can be city);
- the cities: name, centre, size, home or not, their towns;
- the routes: name and four-connected path;
- the rivers: four-connected paths;
- the POIs: kind, name, tile;
- the regions: kind, name, label point.

The exact types are the builder's.

## The sheet

`strata world [--seed N | --seeds A..B] [--size …] [--cities N] [--out sheet.html]`

- Default seeds 0..12, default out `world-sheet.html` in the working folder.
- One page, a section per seed: the seed and parameters, the city names with their
  sizes, counts (land %, routes, POIs by kind), then the **whole world drawn in the atlas
  look**, one cell a tile, Fira Code from `fonts/` so the cells line up as in the window.
- The atlas look is round 1's A, as in `ow.py`'s `atlas_cell`: dust `·`, scrub `"` `,`,
  fields `▬`, hills `∩`, mountains `▲` (peaks brighter), the spill in poison green,
  salt `░`, the sea `~` in deep blue, lakes `≈`, rivers as thin blue box lines,
  highways as amber double box lines (brighter in a city), cities as `█▓▒░` shaded by
  density in their districts' colours, POIs in their glyphs and colours.
- The atlas drawing lives in `strata-zero`'s render code as a function from the world
  and a tile to a cell, so the overworld screen reuses it in the next slice; the sheet
  only writes those cells out as HTML.

A test run is: `strata world --seeds 0..12`, open the page, compare with the mock, tell
me what's off, I tune the rules or the defaults, run it again.

## Constraints

- `citygen` stays free of terminal code. The sheet is a CLI path in `strata-zero`, the
  same binary, no new dependency it doesn't need.
- Deterministic: same params, byte-identical `Overworld`.
- A medium world generates in well under a second in a release build.
- Every glyph is one cell wide and has a font in the window (the box and block ranges
  always do; the others are checked against Fira Code and the fallbacks).
- Check both front ends as `CLAUDE.md` says; build into `target/smoke`.
- Doesn't touch `AppState`, `Runner`, saving or any screen, so it can't collide with the
  save the run work.

## Tests

- Determinism: the same params twice give equal worlds.
- Over a spread of seeds and every size preset:
  - there is a home city, on the water when the map has a sea;
  - every city is reachable from every other by the routes;
  - every route and river path is four-connected, routes never touch the sea, rivers
    end in the sea or a lake;
  - city tiles are never sea, lake, peak or dead ground (river tiles may be city), and
    each city's tiles connect to its centre (its towns to its roads);
  - the home city spans its river where one runs through it;
  - POIs sit on ground their kind allows; names are unique where they should be;
  - region labels fall inside their regions.
- The atlas cell function draws every kind of tile without a glyph outside the window's
  fonts.

## Left to the builder

The types, how stages split into code, the noise functions, the exact thresholds and
weights (start from `ow.py`'s numbers and tune on the sheet), how the HTML is written,
whether the sheet embeds the font or links to it.
