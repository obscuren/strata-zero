# The overworld: moving across the world, going down, and the deck's world map

Date: 2026-11-04
Status: agreed in brainstorming; plan after save the run (#2) lands
Builds on: `2026-11-04-world-generator-design.md` (built), `2026-11-04-world-tiles-design.md`,
`2026-11-04-cities-in-the-world-design.md`
Mocks: `2026-11-04-overworld-mock/round1.html` (A, C, D), `round3.html` (the picks,
including the deck MAP's zoom 4)

## Why

The city was the first slice of a much larger world. Jeffrey: "multiple cities in a
giant world … an over world … Hitting a city tile instantly zooms in to the level we
have now. The overworld will have POIs which we can also zoom in to. Any tile can be
zoomed in to and you will end up on the detail view of that part of the map." For now
the player can't do anything on the overworld "other than go down one level deep";
roads, water and POIs must read from above. This spec is the overworld as a place to
be: its screen, moving on it, going down and up, the same world jacked in, and the
deck's MAP zoomed out to the world.

## Goals

- The overworld is a level of play in the play screen (HUD, panel and all), drawn in
  the atlas look, one cell a tile.
- Move across it; step onto a city and you are in the city where you stepped; press ↵
  on any other tile and you are on its detail map; go back up from the open street.
- Jacked in, the overworld shows its Net (round 1's D), and going down lands on the
  tile's Net.
- The deck's MAP has a fourth zoom, the world, drawn as the survey chart (round 1's C)
  with its places pane.
- The overworld, where you are on it, and every kept tile are saved with the run.

## Non-goals

- Encounters, vehicles, fuel, food, weather on the overworld.
- Travel in the Net across the world, and how far the Net reaches from your body (open
  question below).
- People moving between cities.

## Anti-goals

- A fast-travel menu: the overworld is a map you walk, tile by tile.
- A second UI language: it is the play screen, with the same panel and keys.

## Decided (Jeffrey)

- The overworld look is round 1's A; jacked in it is round 1's D; the survey chart (C)
  is the deck MAP's world map, a zoom further out than today's whole-city view.
- Stepping onto a city tile goes straight into the city at that spot; any tile can be
  gone down into.
- Going down from the Net overworld lands on that tile's Net.
- One overworld tile is 8×8 detail tiles.
- Everything from the seed, the same on every platform.

## The overworld screen

### What it shows

The play screen as it is, with the map area drawing the overworld in the atlas look
(`render::overworld::atlas_cell`), centred on you, `@` on your tile.

- **Panel, HERE:** the region and ground you're on and the route if one runs here
  ("Dust Flats · Route 9", "open ground · clear night").
- **Panel, the minimap pane:** the whole world, four tiles a pixel, the view framed
  (round 1). On the detail map of an open tile, the same pane shows the overworld round
  you in A's look (round 2), since the ground near you says little.
- **Panel, NEAR:** the nearest POIs and cities with distance and bearing.
- **Bottom line:** "on foot · overworld", with the hint for going down.

### Moving

The walking keys move one tile. A step costs the time it takes to walk across a tile
on the detail map (8 detail tiles) on that ground *(my call: one clock, no fast travel)*;
hills cost more; mountains slow you to a crawl; peaks, the sea and lakes can't be
entered; a river can be crossed where a road bridges it, or waded at a time cost
*(my call)*. The world's clock runs as you go.

### Going down and back up

- **Down:** ↵ on any tile puts you on its detail map: on its road if it has one, else
  at the walkable spot nearest the tile's centre. Stepping onto a city tile goes down
  into the city at once, on the street at that spot (Jeffrey's words).
- **Up:** from the open street (the Street stratum, outdoors, not inside a building), a
  key takes you up to the overworld onto the tile you're on. The key is the builder's
  pick from the free ones (`<` and `>` are the strata's), shown in the hint line; not
  Alt.
- Walking off a detail map's edge needs no key: you're in the next tile.

### Jacked in

You jack in where there is a port, as today: a terminal in reach on a detail map (POIs
have them out in the open: the truck stop's store, the checkpoint's booths). The
overworld has no ports, so you can't jack in there *(my call)*. Jacked in on a detail
map, the up key shows the Net overworld round you, round 1's D: cities as node fields,
trunks along the highways with packets, dead grid between, lone nodes on spurs; ↵ goes
back down onto the tile's Net. Jacking out always puts you back with your body on its
detail map, as in the city.

**Open question (later):** how far the Net may take you from your body across tiles,
and whether riding a trunk is travel. Until it's decided, the Net overworld can be
looked at but not walked, and the street-level Net is walkable only within the body's
place (see the tiles spec) *(my call)*.

## The deck MAP's world zoom

MAP's zoom steps go one further: after zoom 3 (the whole city) comes zoom 4, the world,
drawn as round 1's survey chart: contour dots up the hills, the coast hatched, cities as
tinted ground with their names, highways with route shields, every POI and region named
(round 3). The cursor moves over tiles; PLACES lists the world's cities, POIs and
regions by distance, with kinds to filter; a place's card shows its kind, what's there
and the way to it; `t` tracks it (its bearing and distance in the panel; the floor's
pulse when you're on the detail map near it). Out in the open, MAP opens at zoom 4.

## Saving

The run's snapshot (save the run, #2) holds the world: the overworld (or just its seed
and params, rebuilt on load, since it is deterministic), where you are on it, every
city built so far, and every kept tile with its changes. Loading puts you back on the
overworld, a tile, or a city, as you left it.

## The world while you're away

Only the place you're in runs. When you come back to a city after time away, its people
must not look frozen. Jeffrey: maybe a super high speed-up so the world appears not to
have stood still. The direction agreed: a short absence replays the missed time fast; a
long one puts everyone where their day says they'd be now. **This gets its own spec
later; nothing is built for it here.**

## Constraints

- The play screen's rules (CLAUDE.md): draw through ratatui, crossterm keys, no Alt,
  exact colours, glyphs the window's fonts draw; both front ends checked; fits the
  smallest terminal (100×30).
- Deterministic and portable.

## My calls (Jeffrey can change any)

- One clock: an overworld step costs the walk across a tile; no fast travel.
- Rivers are crossed at bridges or waded slowly; mountains are slow, peaks closed.
- No jacking in on the overworld (it has no ports); the Net overworld is reached by going
  up from a street-level Net, look-only until the reach question is decided.
- MAP opens at zoom 4 out in the open; tracking a world place shows bearing and distance.
- The overworld itself is saved as its seed and params and rebuilt on load.

## Left to the builder

The up key, how the overworld fits the screen stack and `Place`, the HERE lines'
wording (in the game's voice), the time costs per ground, the generation screen's
world stages on NEW GAME, the cursor and card wording at zoom 4.
