# The streamed world: one street, one Net, no edges

Closes the question in issue #27 (how far the Net reaches) and replaces the stopgaps the
world build left: the city's edge, the screen swap on crossing it, the Net's 3×3 leash
and the look-only Net overworld.

## Why

The world build left two maps on two screens. The city is one fixed map whose camera
stops at its edge; the open ground is streamed tile by tile on a screen of its own.
Crossing the edge swaps screens with a glitch, a city's streets only appear once you are
in it, and the Net is cut short round the body. Playing it, that reads as a border and a
teleport. The world should be one place you walk: what's ahead streams in before you
see it, a city is one more area of the world, and the Net is the same.

Nothing about generation has to change. Since #19 a city's map copies the world's own
ground and Net past its outline, so the cells on either side of its edge are already the
same cells, and a city builds in a few hundredths of a second in a release build. The
work is in how the game holds and shows the map.

## Goals

- Walk out of a city onto the open ground and into the next city or town with no screen
  change, no transition and no edge.
- Areas load on demand: the open ground's tiles as today; a city or town is built in the
  background once its block comes near the view, and kept for the session.
- One play screen at street level everywhere: the city screen, with its HUD, deck and
  keys. The outside screen keeps only the overworld and the Net overworld.
- The Net the same way: the city's Net runs off its edge into the open Net (the void and
  the trunk lines along the roads) and on into the next city's Net. No reach limit. The
  body stays where you jacked in; J returns you to it.
- The Net overworld is walked like the overworld; ↵ drops you onto the Net there.

## Non-goals

- Two cities live at once: breaking into a node of a city other than your body's, or a
  neighbour's people moving before you walk in. That is the next job.
- The world going on while you're away (#26).
- Riding the open Net's trunks: rides stay on a city's own lanes.
- Any change to what the generators make.

## Anti-goals

- A loading pause or a stutter when you cross into a city.
- A seam at a city's edge you can see: in the look, the light or what you can walk on.
- The home city as a special case: every city and town streams in the same way.

## Decided (Jeff)

- Everything streams in on demand: the open ground, cities, towns and the Net.
- A city is walked into, never teleported into.
- The Net's void is freely walkable for now (something may later make wandering into
  it costly). The Net is joined along the roads by its trunks.
- From the walkthrough of 2026-11-05: the street is one map made of areas; one city is
  live at a time; a neighbour's people appear when you walk in; in a city that isn't
  your body's the Net can be walked and looked at, not broken into, in this build; W up
  to the overworld and ↵ down stay; saved runs keep opening.

## How it works

### The frame

Every city's and town's map sits at an origin in world detail cells: the origin of its
site, or, for a city from before the world, the corner of the block `Outside::block`
gives it, times `TILE`. A world cell is the origin plus the local cell.

The city you are in, the *live* city, stays the frame of the game's systems. Your
position stays in its local cells and may now lie outside its map: on the open ground,
or over another city. Being *on the ground* means standing on the street off the cells
the live city owns (below). Everything the live city holds (its people, interiors, constructs,
jobs, foes, the Net's state) keeps working in its own cells unchanged.

### Areas and the stream

An area is something that covers part of the world at street level:

- **The open ground's tiles**, two layers: the street's (`detail::region`) and the Net's
  (`net_ground::region`), built 8×8 cells at a time as today, a ring ahead of what you
  can see, forgotten far behind.
- **A city or town**: its whole map, street and Net strata, from the city generator.

The stream holds the open ground's tiles and the cities and towns being built. It is not
saved; everything in it is made again from the world.

The open ground lights by the street's rule: ambient as the street's at the hour, and
lamps casting light the way a city's lamps do (their emission, stopped by what blocks
it). The same light falls on both sides of a city's edge.

A city or town starts building on a background thread once its block comes within
`REACH` cells of the view's rectangle, and is kept for the session once built (as the
built cities are today). The same thread seeds its people for the hour the build was
asked for, as walking in seeds them today; walking in runs them on to the hour you
arrive. The home city is always held, live or set aside. Stepping into a city whose
build hasn't finished builds it on the spot.

### Who owns a cell

Maps overlap: every city's and town's map is its block (its own tiles, two more all
round), so a town's map reaches into its city's and the city's over the town. Each map
holds only its own buildings; under the other's tiles it has the world's bare ground.
So every cell has exactly one owner, decided per overworld tile from the world alone
(built or not):

- A tile no map covers is the open ground's.
- A tile one map covers is that settlement's.
- A tile several maps cover belongs to the one whose own tiles (`city_tiles`,
  `town_tiles`) lie nearest it; a tie goes to the city before a town, then to the lower
  index.
- A city from before the world owns its whole block.

### One map to walk, see and draw

The live city's street and Net carry on past its edge. A cell is the cell of the area
that owns it: the live city's map, another held city's or town's map, else the stream
(also for a settlement that owns the cell but isn't built yet). Walking, sight and light
see one map across every edge. A cell that isn't built yet is drawn blank and can't be
walked into.

The screen draws by area: the stream's layer under the whole view, each other held city
on the cells it owns, then the live city on the cells it owns, with everything on it
(people, foes, items, every overlay). Each area is drawn by the renderers a city is
drawn by today (the street's tiles, the Net's look) with its own city's buildings and
districts, so no id of one city is ever read against another.

### Sight and memory

On the live city's map, what you've seen is kept as today. Off it, the world's seen
record (`Outside.seen`, already saved) keeps it, with what is in sight this moment.
Walking into a city, what you saw of it from outside is remembered on its map.

One sight rule at street level everywhere: the street's (the dark radius, growing by
day, lit cells seen further). The outside screen's own far-sight rule and its moonlight
floor go. On the Net everything is in sight, past the edge too.

### Walking out and in

- The live city's edge is the edge of the cells it owns. On foot past it: refused for
  the reasons leaving is refused today (the lot has no world; not in a fight; not with
  a call on the line; not during a scene), with the same lines. Jacked in is no longer
  one of them: your Net self walks off freely.
- A step onto a cell another city or town owns makes it the live city, as walking in
  does today (the one you leave set aside, or kept as built), on the same cell with the
  same camera and no transition. The log says `into {name}`.
- Coming home is the same step the other way.
- The live city stays live while you're on the ground, so looking back you see its
  people. Off its cells its story and its cameras hold still, as when you're away today.

### What stays with a city

- Foes stay on their city's map: they don't follow you out. A fight holds you in.
- Items lie only on a city's map. Off the live city's cells there's nothing to put one
  on (`nowhere to leave it out here`).
- Bots out come with you: they follow you on the ground as on the street, and walk
  into the next city with you on a handover.

### The Net

- Jacked in, in a city or at a port on the open ground, your Net self walks the live
  city's Net, off its edge into the open Net and on into other cities' Nets.
- The void is walkable on the Net, everywhere: past a city's outline and in the open.
  A construct's walls are not.
- Your body stays where you jacked in, and J always puts you back in it.
- In a city other than the live one you can walk its Net and see its nodes; a node or a
  gate there won't answer (`too far from your body to touch it`).
- On the open ground, space by a terminal or a vending machine jacks you in, as the
  outside screen did, at the nearest Net cell with signal.
- A run saved jacked in on the open ground comes back jacked in, as one saved in a city
  does.

### Up and down

- W on the street, in a city or on the ground, goes up to the overworld tile you're on.
  ↵ comes down on that tile: into its city or town, or onto its ground. Both land on the
  city screen.
- W jacked in shows the Net overworld round your Net self's tile. The arrows walk it,
  every tile inside the world, each step costing a tile's walk. ↵ drops your Net self
  onto that tile's Net: its trunk nearest the middle, else the nearest Net cell you can
  stand on. J jacks out.

### The screen on the ground

The same HUD and keys as in a city. Off every city's map, HERE shows what the outside
screen showed: the place and the road, then `open ground · ground · hour`, and NEAR
with the mark and the nearest places. Jacked in, HERE says which trunk, city or dead
grid you're over. The minimap shows the street round you across edges.

### Saving

- A new save on the ground is the street with the player off the live city's map.
  `At::Ground` stays readable: an old save with it opens on the street at that cell.
  The fixtures in `runs/fixtures/` keep opening.
- The stream and builds in flight are not saved. Neither is sight past the edge; the
  world's seen record is.

## After the first play (2026-11-05)

Jeff played the streamed world: the neighbouring cities read as abandoned (no people),
their Nets as dead (no packets, no ICE, a cord left behind on the Net you came from),
some cities' light as overblown and stitched on, and the overworld as static. So:

- **The live city follows your Net self too.** Jacked in, stepping onto Net cells
  another city owns hands the run over to it, as walking in does on foot: its Net is
  live (its ICE, its nodes, its packets, its jobs). Your body stays where you jacked in,
  in the city you left, which holds still while you're away from it; jacking out puts
  you back in your body and hands that city back. The crossing is refused for the same
  reasons as on foot. Replaces my call 2.
- **Neighbours are alive in view.** A held city or town whose map is in view runs its
  people (their day only: no story, no cameras) and draws them; its lanes carry
  packets, and so do the open Net's trunks in view. Replaces my calls 1 and 8.
- **One light.** A city's cells look the same drawn from outside as from inside, at the
  same hour, people aside. Its tiles, light, lamps, sky and sight already did; the dev
  light view (`light`) greys every area by its own light, not only the live city's (so
  the live city no longer stands out pale and cut off at its outline); and a city held
  as you left it keeps the clock: its lights out, holds and hijacks run out as they
  would inside, and its hijacked ads glow from outside too.
- **The overworld moves** (Jeff picked round 2 of `2026-11-05-overworld-motion-mock/`):
  slow neon in the cities (a district breathes every 20–36 s, rippling across it; a
  sign stutters about once a minute; tower beacons blink every 6 s); weather without
  rain (swell on the shore, glints down the river, wind over the fields, cloud shadows
  drifting a tile at a time, and now and then a gust through the mountains, every
  40–70 s for about 8 s); beacons at places (the relay's green blink with a halo after
  dark, the checkpoint's red cross, and the rest of round 2's), no lights on the roads.
  No traffic. The Net overworld stays as it is: its calm suits it. The overworld's
  hint line goes. Rates and the Rust notes are in `round2.html`.
- **One set of rules everywhere.** The open ground, the live city, a city held but not
  live and the Net behave the same however you got there (walked in, ↵ down, your Net
  self walked in and jacked out, a run reopened there). A refused step says why by
  what's there, whoever's cell it is: water, a climb and the world's end in the outside
  screen's words; a wall, or anything standing in the way, nothing, as in the city. On
  the Net the status bar, HERE and the minimap name a cell by who owns its tile (the
  city or town, else the trunk, else the dead grid); on foot the minimap and the run's
  card name a city's or town's street as HERE does. A city or town built on its site is
  called what its world calls it (citygen names it so), so the status bar and the band
  name it as HERE and the overworld do. A drop from the Net overworld is
  refused for what refuses walking across. The overworlds take the map's dev keys that
  need no street (`` ` ``, the console with `tp` over the world, F5's inspector of the
  tile) and carry no line of keys: `?` lists them.
- **Signs stand in the way; the night is round; cities glow from afar** (Jeff, after
  playing the above). A sign on the ground (a billboard's caps and letters, a truck
  stop's road sign, a test range's warnings) is solid for you, a city's walkers and
  every path. Where one would cut a way through (a scrapyard's gate, a pocket by the
  road), the generator stands it at its next spot or leaves it out, the same in every
  build. The dark radius at night is a circle, as the day's sight and the field of view
  are, for you and for anyone looking back. A lit cell is seen wherever nothing stands
  between, up to 96 cells: a neighbour's lamps show from the open ground well before
  you reach it.

## Constraints

- Seeds: nothing generated changes but what solid signs change. Worlds and cities come
  out cell for cell as before, but:
  - a sign is moved or left out where it would cut a way through (above);
  - a sign on the ground isn't open ground to the street's dressing, so a roll it made
    beside where one now stands isn't made and the rolls after it fall differently:
    some stalls, trash, puddles and terminals stand elsewhere (592 cells in 44 of 550
    settlements of 40 worlds);
  - the dressing, clearing what seals a pocket, takes a sign down last: only where
    nothing else seals it and the street would reach the pocket with every sign down
    (7 cells in 5 of 1,100 settlements of 80 worlds, each a billboard's letter or cap,
    or a world's warning past a city's outline, kept standing).
- No stutter: cities build, and their people are seeded, off the main thread; the
  frame never waits for one except when you step into a city before its build has
  finished. Coming home after a long trip still runs the home city's people on to the
  hour on the frame, as today, until #26 gives it a fast catch-up.
- Per frame, work is bounded by the view; the stream forgets what's far behind.
- Both front ends; `docs/saving.md`'s rules for anything that lives through play.

## My calls (Jeff can change any)

1. The live city is the last one you stood in. Neighbours draw their streets and lights
   without people until you walk in.
2. A non-live city's Net is walk-and-look in this build, with the line above.
3. One sight and light rule, the street's, everywhere at street level.
4. `REACH` is two overworld tiles (16 cells) past the view.
5. Riding a trunk isn't travel in this build; the Net overworld is the fast way across
   the world (the second question of #27).
6. A step on the Net overworld costs a tile's walk (`TILE × WALK_TICKS`); every tile in
   the world can be walked.
7. Tracking's floor pulses lead within the live city's map; off it, the mark's way
   shows in NEAR.
8. The Net's packets run on the live city's lanes only.
9. In-world lines: `into {name}` on crossing (as today), `too far from your body to touch
   it` at a non-live city's node, `nowhere to leave it out here` dropping off the live
   city's cells.
10. Who owns a cell: the nearest own tiles, city before town (above). The handover is
    at that line, not at the edge of a map.
11. A fight still holds you in the city; foes don't follow you out.
12. Bots out come with you across the ground and into the next city; items don't lie
    on the open ground.
13. A new city's people are seeded on its build thread; the home city's catch-up after
    a long trip stays on the frame until #26.

## Left to the builder

How the street map learns about the areas past its edge (a field on the stratum map or a
wrapper), how the map view takes visibility for cells off the live map, how the screen
notices a handover to move its camera with it, and the threading of the builds.
