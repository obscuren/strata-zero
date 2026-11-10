# Sites (#34)

A job or a mission happens in a site: a building of several levels, made by one
generator from a few inputs, with locks between the way in and the objective and more
than one way past each. The generator builds every hotel, office and metro station. When
a mission is in one, its doors open onto a copy of it that holds the mission's guards,
devices and objective, and keeps its state until the mission ends.

The mocks are in `2026-11-10-site-generator-mock/`. `sites.html` is made by `gen.py`
from `hotel.py`, `levels.py`, `metro.py` and `sitekit.py`. It holds a medium hotel, the
Pale Orchid, and a large metro interchange, Cinder Cross. Each mock has four routes in
and out, one per way of playing. They are the target for:

- how a site reads;
- how big it is;
- how its levels join.

The mocks are hand-drawn. The generator should make sites like them, not copy them.

## Why

- **Jobs are walks.** Today a job's goal is an item on a flat's floor or a node in the
  construct. Nothing stands between the street door and the goal.
- **Size.** The first hotel mock was too small and would be too easy. A site has to be
  big enough that getting to the objective and back out is the job. That means several
  levels, several locks, guards between floors, and a way out that is harder than the
  way in.
- **What comes after.** Act two's missions (#30), new job kinds (#40) and the fixers'
  stories (#44) all need places like this. The metro network, the project after this
  one, builds on the station type.

## Decided

Jeff settled these while the mocks were drawn.

- **One generator, types as data.** A type (hotel, office, metro station) is an entry in
  `flavor/sites.ron`. The Rust never names a type, so a new type is a file change.
- **Size makes a site deeper,** not only wider: more levels, more locks before the
  objective, more ways in, more guards.
- **Every hotel, office and station is a site.** With no mission in it, it is the plain
  building: guests, staff and passengers, and nobody looking for you. The plain building
  and a mission's copy come from the same generator and the same seed, so they have the
  same layout.
- **A mission copy for every site type.** While a mission is in a site, every door into
  the site is flagged. Going through a flagged door takes you into the copy: the same
  layout, plus the mission's guards, devices and objective.
  - A copy's only ways out are its own doors into the city. A station's tracks are
    sealed at the platform ends.
  - A copy holds only the floors that matter; the lift passes the rest.
  - A copy keeps its state. Leave halfway and come back an hour or a day later, and it
    is as you left it.
- **A station's depths.**
  - Its ground level (G) is part of the city's street.
  - Level −1 is at the Undercity's depth, with maintenance doors into the Undercity's
    tunnels.
  - Levels −2 and lower are new: nothing in the game is deeper than −1 today.
- **The first types are hotel, office and metro station.** The hospital moves to #99.
- **Today's Undercity rail stays, as a dead old line.** The metro's lines will run at −2
  and lower. That belongs to the metro project, not to this spec. The mock's −5 old line
  goes away.

## What happens today

**Floors.**
- They are made lazily, one `(building, level)` at a time, by
  `generate_floor(city, plan, level)` (`crates/citygen/src/interior/`), from a
  `FloorPlan` (`plan.rs`).
- They are cached in `Interiors.floors` (`crates/strata-zero/src/interiors.rs`) and
  saved whole (`docs/saving.md` §12).
- A floor's rng comes from the city seed, the building id and the level.
- A floor's tiles never change after it is made. Play state lives beside it: items in
  `Ground`, codes and keypads in `Journal`, building-wide locks in `NetState`.
- Entering a building runs `credentials::prepare`, which makes every corridor floor of
  the building at once to count its flats.

**How tall.**
- A hotel or an office is Mid (4–9 floors) or Tall (hotels 10–24, offices 12–24). A corp
  tower is 18–48.
- A building has at most one basement, at −1, and only when it reaches the Undercity.
- A floor is the footprint × 3, at least 12×9.
- Hotels and offices stand only in a Corp Core district, and hotels on the Neon Strip.

**What a floor holds.**

| Floor | What's on it |
|---|---|
| Basement | one storage room |
| Lobby | one room with a counter and reception |
| Corridor | a 2-wide corridor with rooms on each side: hotel rooms, or offices and open plan |
| Suite | the top floor of an office or tower |

Every hotel floor above the lobby looks the same.

**Moving between floors.**
- A building has one lift bank and one stair, at the same position on every floor.
- The lift goes to any level (the FLOORS popup). A stair moves one level.
- A building on alert stops its lifts, unless its LiftCtrl is spoofed.

**Doors to the city.**
- `exit_stratum` is fixed: −1 → Undercity, 0 → Street, the bridge level → Skyways, and
  the top floor with a roof → Heights.
- The doors on the city map are decided when the city is made (`generate/interiors.rs`,
  `close`): a main and a back street door, one Skyways door, a roof door, and one
  Undercity door onto a tunnel, the rail or a grate.

**Locks.**
- A locked room is `Room.private`, opened through `credentials::opens`: a Net crack, a
  keycard, or a code (the Keyjack cracks keypads).
- Nothing on a tile records whether a door is open.

**Nothing hostile acts on a floor.**
- `foes.rs` sees, walks and shoots on strata only (#33).
- There are no guards, patrols, turrets or containers.
- Net nodes are anchored to street tiles only (#95), and cameras face out from facades.

**Jobs and missions.**
- A Fetch job puts paydata in a lived-in flat; hotels are left out. Copy and Plant are
  construct nodes.
- The mission book reaches places only through its cast's roles.

**Code that assumes today's shape.** About 25 places in 14 files assume:
- one basement at −1;
- level 0 is the street floor;
- the levels are a contiguous `lowest..=top`;
- the lift and the stair stand in one position on every floor;
- a stair moves ±1.

They include:
- `FloorPlan.levels`, `exit_stratum` and `level_for`;
- `Interiors::ensure`, `ride` and `climb`;
- the FLOORS rows and the stair rows in `interact.rs`;
- the minimap's `shaft_rows`;
- the map's up/down flags;
- room numbers (`level.max(0) * 100 + n`).

**The word "site" is taken twice.** `game::model::Site` is a place on the body for
chrome, and `citygen::generate::site` (`CitySite`) is a city's place in the world.

## Goals

1. **One generator.** Given a type, a building or a frame, and a seed, it makes a site.
   A mission adds a size, an objective and a security level on top. The same inputs give
   the same site on every platform.
2. **Hotels and offices are sites.** In every city, a hotel or office with no mission in
   it is a plain site, and it looks and plays better than today's floors.
3. **Stations are a type.** They are generated, checked, seen in shots and opened from
   the console. Cities don't place them yet.
4. **Mission copies** follow every rule in "Decided".
5. **Links.** Every building moves between levels by links of several kinds: lifts that
   serve some levels, stairs, escalators, ladders, crawls.
6. **Checks.** A checker runs every type at every size over many seeds, and there are
   shots of each.
7. **Old saves** keep opening.

## Not in this spec

- **The metro network:**
  - placing stations in cities;
  - the lines at −2 and lower;
  - trains on a timetable;
  - walking the track;
  - lines between cities;
  - the old line's new look.
- **Making the mission layer act.** This spec places it; these tickets make it work:
  - guards acting on floors (#33);
  - the alert (#36);
  - the site's own Net (#95);
  - containers (#96);
  - turrets acting (#97);
  - objectives as play (#98).
- Jobs and missions that use sites (#30, #40, #44).
- The hospital and the other types (#99). Corp towers stay on today's floor builder until
  #99.
- Finding your way around the Undercity: a ticket of its own.

## What would make it a failure

- A type's name in the Rust code.
- A site with one way through, or a lock with one way past.
- Every floor of a site the same, as today's hotel floors are.
- A copy that leaks:
  - its guards, its alarm or its bodies showing in the city or in the plain building
    after the mission;
  - a way out of a copy to anywhere but the city at the building's own doors.
- One seed giving different sites on different platforms. Generation follows the
  world's rule: no libm `powf`, `sin`, `cos` or `hypot`.
- A save from before this change that no longer opens. The fixtures in `runs/fixtures/`
  keep opening.

## The design

### The layout and the mission layer

A site has two parts, and the split is what keeps the copy's layout the plain building's.

- **The layout** depends only on:
  - the type;
  - the frame: the footprint, the floors, and which levels have doors to which strata,
    and where;
  - the seed.

  It holds:
  - the levels, each a floor as today: tiles, rooms, exits and stations;
  - the links between levels;
  - the zones, the locks at their borders and the ways past each lock;
  - the ways in;
  - the spots where the mission layer can go.

  In a city, the frame and the seed come from the building: the city seed and the
  building id, as a floor's do today. For a shot, the checker or the console, the frame
  can come from the type's own defaults, and the seed is any number.
- **The mission layer** is laid on the layout from:
  - the mission's size;
  - its objective;
  - its security (0–255, the scale a subnet's `security` uses);
  - a mission seed.

  It decides:
  - which floors the copy keeps;
  - where the objective goes;
  - the guards, their posts and patrols;
  - the devices: cameras, turrets and card readers;
  - the containers;
  - where a lost keycard lies.

  The plain building is the layout with no mission layer.

### Size

Size is the mission's, not the building's. Size sets:

- how many floors the copy keeps;
- how many locks lie between a way in and the objective;
- how many guards there are.

**Where the locks come from.**
- The layout has the zones, and the doors between them.
- The plain building locks only the doors the type marks as always locked, such as staff
  and secure doors. So a small plain hotel isn't a fortress.
- A mission can put card readers on more of the layout's doors, up to its size's count.
  It never adds walls or doors.

**Which buildings can hold which sizes.** Each building's layout says which sizes it can
hold: enough floors, and a zone chain deep enough for the locks. A 6×4 hotel on the Neon
Strip may hold only small. A mission picks a building that holds its size. The type's
own frame holds every size.

| Size | Floors kept | Locks to the objective | Ways in, at least | Guards |
|---|---|---|---|---|
| Small | 2–3 | 2 | 2 | 6–10 |
| Medium | 4–6 | 3 | 3 | 12–20 |
| Large | 6–9 | 4–5 | 4 | 25–40 |

The numbers are first guesses, to tune by playing. They live in `flavor/sites.ron`, not
in the code.

### A type

A type says, as data:

- **Which building kinds it builds.** Hotel builds `Hotel` and office builds `Office`.
  The station builds no kind yet.
- **Its levels by role,** placed onto the building's floors:
  - fixed roles at the bottom: car park, service, concourse, platform;
  - the ground role: lobby or hall;
  - a role repeated to fill the middle: guest floor, open-plan floor;
  - fixed roles at the top: penthouse, executive floor.

  Basements below −1 are the type's own and touch no stratum.
- **Its zones** (public, guest, staff, secure, and so on), and for each lock between two
  zones, the ways past it:
  - a keycard or a code;
  - the Keyjack;
  - the Net;
  - force;
  - talk.

  Force and talk have no play yet. They are in the data and the checker so the layout is
  right when they arrive.
- **One-way locks and links.** A lock or link can open from one side only: a fire door,
  a gate that opens from the inside, an exit-only turnstile, an emergency stair that
  opens only at its ends. This is in the data now, so saved sites don't need reshaping
  when the alert (#36) makes the way out the hard part.
- **Its rooms:** a name, a zone, a generic room kind (below), sizes, furniture and
  stations. It also names set pieces with a fixed shape, like a platform with its track
  or a car park ramp.
- **Its links:** a lift for the public and one for staff, a freight lift, a public stair
  and an emergency stair, escalators, ladders, and crawls (vents and ducts). For each
  link it gives the zone and which roles it serves.
- **Where its objectives can go,** for each objective kind it offers.
- **Its mission layer per size and security:** guards per zone, device density, and the
  numbers in the size table.

Room kinds stay a Rust enum of generic kinds (hall, corridor, room, open floor,
platform, track, store, plant, office, and so on), since lighting, stations and look
text hang off them. A type names its rooms in data, so "ticket hall" is a hall that the
station type calls a ticket hall. New generic kinds join the enum at its end.

The RON follows `flavor/missions.ron`'s style: named structs, lists, and citygen enums
used directly (`BuildingKind`). The exact schema is the builder's.

### How a site is made

1. **Frame.** The footprint, the floors, and the doors to each stratum come from the
   building, or from the type's defaults.
2. **Levels.** The type's roles are placed onto the floors. Each level's shape is chosen:
   - A level at or above −1 stays within the footprint × 3. Its doors land where the
     city's doors are, as today (`exit_tile`).
   - A level below −1 can be larger, since nothing else is there.
3. **Zones.** Zones are laid out per level and across levels: public at the doors,
   secure deepest. Locks go on zone borders, and each lock takes its ways past from the
   type.
4. **Rooms.** The zones are filled with the type's rooms, corridors, set pieces and
   doors.
5. **Links.** Each link gets its kind, the levels it serves, and a position on each
   level it serves. Positions can differ from level to level: escalator landings sit a
   few tiles apart, and a duct comes out somewhere else.
6. **Ways in.**
   - The building's doors from city generation. The city map doesn't change.
   - Crawl ways within the footprint, on levels that touch a stratum: a roof grille, a
     duct from the basement into an Undercity tunnel.
7. **Mission layer** (copy only), as above.
8. **Check.** Run `validate_site`.

A site is planned whole when its building is first needed, which is cheap. Its floors
are drawn as they are asked for and cached as today. `credentials::prepare` counting
flats must not draw every floor of a large site.

### Levels, depth and links

- **Doors per level.** Each level says which stratum its doors open onto. G opens onto
  the Street. The bridge level opens onto the Skyways, and the roof onto the Heights.
  Level −1 opens onto the Undercity when the building reaches it. This replaces
  `exit_stratum`'s fixed rule and `level_for`'s mirror of it.
- **One way to move between levels, for every building.** A link has:
  - a kind;
  - a zone, so a staff lift wants a card;
  - the levels it serves;
  - its position on each of them.

  Today's buildings get their one lift (all levels) and one stair (same position) as
  links, so nothing changes for them.
- **A lift** lists only the levels it serves; in a copy, only the kept ones.
- **A stair, escalator or ladder** takes you to the next level it serves, up or down.
- **A crawl** is a link only some builds use (a Tech or body check, the builder's call).
  The checker counts it as the crawling build's way.
- **On alert,** lifts stop as today, unless they are held.
- **Room numbers** on basement levels mustn't collide with the lobby's.

### Cards and codes

Today a keycard opens one household's flat, and a code opens one room. A site needs
keys to its zones, so it adds zone cards.

- **One card per zone above public.** A zone's card opens that zone's card doors and
  lifts, and those of the zones below it, so a secure card opens staff doors too.
- **Where the cards are.** In the plain building, staff carry their zone's card. In a
  copy, guards and staff carry them. The mission layer also leaves each zone's card
  somewhere in reach before that zone's locks: a locker, a desk, a coat. This takes the
  place of today's lost card.
- **Codes.** A keypad lock has a code. It can be found in the site (a note, a terminal),
  taken from the Net, or cracked with the Keyjack. Codes keep today's key, (building,
  level, room).
- **The checker.** Its card-and-code route must find each card or code in the site
  before the lock it opens. Nobody brings keys from outside.
- **Talk.** Until talk has play (#42), the checker models it as reaching a person of that
  zone, who could open the door or hand over a card.

### The plain building

The plain building is the layout with today's population: people are placed at the
type's stations when a floor is looked at (`interiors::Placement`). Its locks are the
always-locked doors, opened by a zone card, a code, the Keyjack or the Net. Nobody is
hunting you.

### The mission copy

The copy is the building's own floors in their mission state. It is not a new kind of
place.

**Why that works.** While a mission is in a site, every door into it is flagged. So the
player can't be in the plain building and the copy at once. `Place::Floor { building,
level }` stays as it is, and everything that works on a floor already works in the copy:
drawing, light, sight, walking, items lying on it, tracking and saving.

**Starting.** A mission takes a site with `begin(building, inputs)` and lets it go with
`end(building)`. #30 and #40 will call these; in this spec the console does.
- A site holds at most one mission at a time.
- If the player is inside when a mission takes the site, the change waits until they are
  out.

**Flagged doors.** Every door of the building on the city map is flagged: street, back,
bridge, roof and Undercity. A flagged door is drawn in the mission's mark, and its look
says what it is.

**Which floors it keeps.** In this order, up to the size's count:
1. G;
2. the other levels with a door to the city;
3. the objective's level;
4. the rest, as the layout needs to join them.

A door on a level the copy doesn't keep is barred: it's drawn shut, its look says so,
and it isn't a way in. So a small copy of a tall hotel may lose its bridge and roof
doors, and keep the two street doors.

**Inside.**
- The building's floors are the copy's: only the kept floors, each with the mission
  layer on it.
- Lifts and stairs serve only the kept floors, so the lift passes the rest.
- The copy's ways out are the building's doors. A station's tracks end at shutters at the
  platform ends.

**Its state is kept.** Everything in a copy is saved as play state is today: the floors,
items, foes, journal codes and Net holds. Nothing in a copy moves while you're outside
it.

**Ending.** When the mission ends (done, failed or dropped) and the player is outside:
- the copy's guards, devices and mission things are removed;
- the floors go back to the plain building, made again from the seed;
- the doors lose their flag.

Items the player dropped in the copy stay where they lie. Codes learned in the copy
still work in the plain building, since the layout is the same.

The real building carries on while it is flagged. Its sim workers keep their day, and
they're only not drawn in the copy.

### What a copy holds in this spec

| Part | In this spec | Comes to life with |
|---|---|---|
| Guards | stand at their posts, with patrols recorded; foes can't act on floors | #33 |
| Cameras, turrets, card readers | placed and drawn on floor tiles | #95, #97 |
| Containers | placed and drawn | #96 |
| The objective | a marked spot | #98 |
| Zone cards and codes | carried, and left in reach; they open their locks | (this spec) |

So at the end of this spec a copy can be walked, but not yet fought or snuck through.

### Saving

- Floors and plans are saved whole as today. A site's plan is saved with them, or made
  again from its seed in `after_restore`: the builder picks, under saving.md rule #4.
- Which buildings hold a copy, with the copy's inputs and kept floors, is saved:
  - with `#[serde(default)]`;
  - in `Stay` and `StayParts`, so a copy in another city travels with it.
- **Old saves:** a building whose plan was saved before this change keeps its old floors
  and today's builder for its floors not yet made. Buildings first built after the
  change are sites.
- `docs/saving.md` gets a section on sites and copies.

### Checking

`validate_site` checks one site:
- Every level's floor passes the floor checks that still apply.
- Every link's landings are walkable and reachable on their level.
- In the plain building, every room is reachable from a way in by someone holding every
  key.
- In a copy, each of #34's four ways of playing has a route in and a route out:
  - **The ways:** force, the Net, the Keyjack and talk. Each is a set of means. Every way
    can use open doors and the cards and codes it finds on the way. Force adds force,
    the Net adds the Net, the Keyjack adds the Keyjack and crawls, and talk adds talk.
    The mock's four routes are the model.
  - **The routes:** each way needs a route from an open way in to the objective, and one
    from the objective out to a door into the city. They are found separately, since
    locks and links can be one-way.
- **Depth** is the fewest locks on any route from an open way in to the objective. A
  locked door counts as one, and so does a crawl's grille, which is a lock only crawlers
  pass. A crawl can stand in for a door, but no loop or crawl makes the site shallower
  than its size.
- No device stands on a wall or in a doorway, and the kept floors are joined by the
  copy's lifts and stairs.

Tests:
- One test runs every type over 50 seeds, in two frames, and fails on any error:
  - buildings of the type's kinds from a few city seeds, at every size each one says it
    can hold;
  - the type's own frame, at every size.
- A test type that lives only in a test `.ron` must generate and pass.
- A fingerprint test pins one site per type, so all three platforms agree.

### Seeing it

**Shots.** Through the shot harness (`screens/shots.rs`), as the window paints them:
- each type at each size, every kept level;
- the plain building and the copy;
- one sheet per type and size with the levels side by side, as the mock lays them out
  (the builder's call).

**Console** (dev): `site <type> [size] [objective] [security] [seed]` at a building's
door turns that building into the given site for testing, even another kind. A station
can stand in any building this way. It starts a dev mission copy and flags the doors.
`site end` ends it.

## The first three types

Each one's levels, rooms and objectives are the builder's to fill in from the mocks and
this list.

**Hotel** (`Hotel`).
- A car park below.
- A service level: kitchen, laundry, lockers, the security office.
- The lobby: reception, a bar or restaurant, the lift lobby, the back corridor.
- Guest floors to fill the middle, with rooms of a few shapes, a linen room and the
  staff stair.
- The penthouse on top, with the safe.
- Zones: public, guest, staff, secure.
- Objectives: the penthouse safe, a guest's room, the security office's server.
- The Pale Orchid mock is a medium one.

**Office** (`Office`).
- A lobby with a gate line of turnstiles and a security desk.
- Open-plan floors to fill the middle.
- An executive floor at or near the top.
- A server room, in a basement or on a plant floor.
- Zones: public, staff, secure, executive.

**Metro station** (no building kind until the metro project).
- G is the station hall. The plaza, the yard and the street around it are city ground,
  laid by the metro project.
- −1 is the concourse: the ticket hall, the gate line, the paid side and the staff block.
  Maintenance doors open into the Undercity's tunnels.
- −2 and −3 are platforms for two lines crossing.
- −4 is the service level, with Line Control.
- −5 holds drainage and a cable vault, in place of the old line, so a large station has
  six levels.
- In the plain station, the tracks at the platform ends meet a wall or a shutter until
  the metro network exists. In a copy they are sealed.
- Cinder Cross is the target. Its ways out by the tracks and the old line go, so every
  route leaves by the street doors or the −1 maintenance doors.

## Calls I made

These weren't settled with Jeff. Each is open to change.

1. The copy is the building's own floors in their mission state, not a new kind of
   place.
2. The layout depends only on the type, the frame and the seed. The mission's size,
   objective and security decide the kept floors, the objective's spot, and the guards
   and devices.
3. Every building moves between levels by links. Today's buildings get theirs with no
   visible change.
4. The city map doesn't change. A site's doors are its building's doors, and extra ways
   in are crawls within the footprint.
5. Levels at or above −1 stay within the footprint × 3. Deeper levels can be larger.
6. Guards stand but don't act until #33. Devices are placed and drawn, and act with #95
   to #97. The objective is a marked spot until #98.
7. The station's G is its hall, and −5 becomes drainage and a cable vault. Its tracks
   end at a shutter until the network exists.
8. Corp towers stay on today's builder until #99.
9. Nothing in a copy moves while you're outside it, and what you leave in a copy lies
   where you left it.
10. A mission takes a site only while you're outside it.
11. Old saves keep their old floors.
12. Room kinds stay a generic Rust enum, and types name their rooms in data.
13. Keys are zone cards, one per zone, found in the site before their locks. They take
    the place of the lost card.
14. The plain building locks only the doors the type marks. A mission adds card readers
    on the layout's doors up to its size, and a building holds only the sizes its layout
    can reach.
15. A copy keeps its door levels first. Doors on levels it doesn't keep are barred.
16. Depth counts crawl grilles as locks, so crawls stand in for doors without making the
    site shallower.
17. Locks and links can be one-way now, and the checker finds the way in and the way out
    separately.

## Left to the builder

- The names. "Site" clashes twice, see above.
- The RON schema.
- The layout algorithms, and how level shapes vary.
- How a flagged door looks, and the console's syntax.
- How a stair between two kept floors that aren't neighbours is drawn.
- Room numbers below ground.
- How many seeds the checker runs, if 50 is too slow.
- Whether the plan is saved or made again.

## What done looks like

- In a new game, a hotel or office is a site. Walk in and its levels differ from one
  another, with more than one lift or stair, and basements where the type has them.
- In dev mode, `site` makes a copy:
  - its doors are flagged;
  - the lift passes the floors it doesn't keep;
  - guards and devices stand where the mission layer put them;
  - leave and come back, and it is as you left it;
  - `site end` brings the plain building back.
- Stations, hotels and offices pass the checker at every size, and their shots are
  written.
- The five checks in CLAUDE.md pass, for both front ends.
- Old fixtures open.
- The manual's chapter on the city (`manual/06-the-city-and-the-world.md`) says what
  changed for players: lifts that serve some floors, more than one way up, basements
  below B1.
- #34 is closed by the commit that finishes it.

## After this spec

1. Guards on floors (#33), the alert (#36), the site's Net (#95), containers (#96),
   turrets (#97) and objectives (#98), which make a copy play.
2. Jobs and missions in sites (#40, #30, #44).
3. The metro network:
   - stations placed over lines at −2 and lower;
   - trains on a timetable;
   - walkable track;
   - lines between cities (they may come up to the surface at a city's edge and cross
     open ground, still to be decided);
   - the old line's new look.
4. A ticket for finding your way around the Undercity.
5. More types (#99).
