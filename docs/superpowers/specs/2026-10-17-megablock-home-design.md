# The megablock and the player's home: design

Date: 2026-10-17. Status: draft, awaiting Jeffrey's approval (look-down decided: option A, plaza from any height).
Builds on: `2026-10-15-interiors-design.md` (floors behind doors, lifts,
stairs, locked flats) and `2026-10-12-cyberspace-design.md` (jacking in).
Riff notes this came out of: `docs/ideas/2026-10-17-terminals-access-apartment.md`.

## Intent

The player has a home, and a new game starts there. Every game used to begin
on a random street corner. Now it begins in the player's own flat, high up in
a huge H10-style megabuilding (Cyberpunk 2077's Megabuilding H10 is the
reference). The flat is a real, multi-room apartment with a desk and a
terminal of the player's own. Step out of the front door and you're on a
walkway ringing an open atrium, with floor after floor of other people's flats
around you and shops at the bottom.

This spec is about the **place**. It has to exist, look right and be walkable,
and the home rig has to jack in. Nothing in the flat has gameplay yet beyond
that. The terminal, journal and backdoor ideas in the riff notes build on this
later.

## Goals

- **Megablocks are rare and big:** one or two per city, each a single large
  building, H10-style inside.
- **The outside fits in.** On the street map a megablock is a big building
  among buildings and doesn't look out of place.
- **Inside is an atrium building:**
  - An open atrium through the levels, with walkways and railings ringing it.
  - Many locked flats off the walkways.
  - Lifts and stairs between levels.
  - The ground level is the busy part: lobby and main entrance, shops, a
    diner or noodle bar.
- **The look-down:** from a walkway you can see down into the atrium. It's a
  dim, static view of the ground plaza at the bottom (floor, stalls, tables),
  dimmer the higher you are but never black. No people are drawn down there.
- **The player's flat** is "layout A" (below). Flat → walkway → atrium is one
  map with no switch. Going to another level (lift or stairs) or out to the
  street switches the map instantly, as it does in every building today.
- **New games start in the flat.** That includes NEW GAME (DEBUG) with
  D'SHAWN and LOAD GAME, since the player respawns.
- **The home rig:** a terminal on the desk in the flat. Walk up and jack in;
  your body stays at home. Jack out and you're back in the flat.

## Non-goals

- Using the bed, shower, storage, kitchen or couch. They're furniture you
  walk around.
- Access Terminal (the typed session), the journal, credentials, keypads,
  hardware frames, backdoors, remote jack-in through backdoors, traces to
  your home. All of those are later specs, in roughly the order the riff
  notes give.
- Upgrading the rig or the flat.
- A first-minute story, tutorial or game loop in the flat.
- Live levels below the atrium (people moving down there).
- Walking from the street into the atrium without a map switch.
- Owning, renting or entering other flats.

## Anti-goals

These would count as failures even if everything technically works:

- A megablock that sticks out on the street map: odd shapes, odd colours, a
  plaza ring nobody else has.
- Any transition, flash or pause between the flat and the walkway.
- A look-down that flickers, shows people, or makes the walkway hard to read.
  The current level must always be the brightest thing on screen.
- Breaking existing saves or the floor rules that keep interiors valid
  (connected floor, sealed private rooms, every exit reaches the shafts).

## What exists today (for the builder)

- **Megablocks are common.** The `Megablocks` theme (`citygen/src/theme.rs`
  about :87) mixes `(Megablock,6),(Apartment,3),(Shop,1)` on 40×20 superblocks
  with 18×10 minimum lots, giving roughly 5–15 Megablocks per city, each
  18–36 × 10–20 tiles. They're always Tall: 14–30 floors, roof on Heights,
  50% chance of a basement.
- **A floor** is the footprint scaled ×3 (`interior/plan.rs`). Lift and stair
  shafts sit at the centre on every floor. Megablock floors are Lobby (level
  0), Corridor (upper levels) and Basement (-1). Corridor floors are a 2-wide
  spine with locked `Flat` rooms on both sides, each holding only a bed.
- **Floors** are generated on demand from `(city seed, building, level)`,
  cached for the session, and never saved. `validate_floor`
  (`interior/mod.rs`) holds the floor rules.
- **Only the city is saved.** The player always respawns via
  `MapScreen::enter_walk` → `Player::spawn` (a street connector).
- **Jacking in** works only from Street terminals with a `Jack` connector.
  `walker::terminal_in_reach` returns `None` on any `Place::Floor`, and a test
  asserts it. `jack_in` keeps the body as `(Place, Pos)`, so a body on a floor
  already works mechanically. `jack_out` leaves the floor FOV refresh to the
  caller.
- **Rendering:** `apply_vis(.., Remembered)` (`render/lighting.rs`) is the
  dim static look. `Overlay` (`render/viewport.rs`) draws over cells by
  position. Nothing yet draws one map beneath another.

## The design

### 1. Rarity and size: city generation

- **How many:** one or two megablocks per city. My call: one per
  `Megablocks` district, at most two. With the default 7 districts there's
  always a Megablocks district (it's the first filler theme), so normal play
  always has one. Cities too small to get a filler district (about four
  districts) have none, and the player starts on the street as before. Theme
  assignment isn't changed.
- **How big:** a megablock takes a **whole superblock** (about 40×20 city
  tiles), so its floors come out around 120×60. My call, from "I want big".
  If a whole block is impractical somewhere, the largest lot in the district
  is the fallback.
- **The rest of the district:** its other lots draw from the mix with
  Megablock removed (Apartment and Shop), so the district still reads as dense
  housing.
- **Hook:** a pass at the end of the Lots stage on the stage RNG, so it stays
  deterministic. Exact placement is the builder's call.
- **From the street:** it's an ordinary closed building with the usual
  wall/floor footprint, doors, Tall span and roof. Nothing special is drawn
  outside.
- **Knock-on effects to handle:**
  - Housing capacity. A megablock holds 30 people today (`sim/people.rs`). It
    should scale so the city's housed population stays roughly what it is.
    Builder's call on the formula.
  - Ganger turf rings, which form around megablocks. Fewer megablocks means
    fewer rings. Accept that, or let the builder put turf rings around the
    district's apartments if gangers vanish.
  - Tests that pick "the first Megablock" (interior/mod.rs :268,
    explore.rs :3922/:4067/:4124, sim/world.rs :1812) need new fixtures.

### 2. The atrium floors: interior generation

A megablock's levels get atrium layouts instead of Corridor/Lobby, **but only
when the building is big enough** for an atrium, a walkway ring and the home
flat. Builder's call on the threshold, somewhere near a whole superblock.
Smaller Megablocks keep today's Corridor/Lobby floors. They only show up in
saves from before this change, since new cities don't make them. My call: it
keeps old saves playable without fitting atriums into 54×30 floors. The shape
of every upper level:

```
████████████████████████████████████████████████
█ flat █ flat █ flat █ flat █ flat █ flat █ flat ░   ← flats on the outside,
█      █      █      █      █      █      █      ░     windows on the outer wall
███+██████+██████+██████+██████+██████+██████+███
█  walkway                                      █
█  ┌──────────────────────────────────────┐ ≡ ⇅ █   ← lifts and stairs at
█  │ . . . . . . atrium . . . . . . . . . │     █     the atrium's edge
█  │ . (dim view of the level below) . .  │     █
█  └──────────────────────────────────────┘     █   ← railing
█  walkway                                      █
███+██████+██████+██████+██████+██████+██████+███
█ flat █ flat █ flat █ flat █ flat █ flat █ flat ░
████████████████████████████████████████████████
```

**Upper levels**
- A central rectangular atrium, ringed by a railing, then a walkway, then
  flats facing the walkway with their backs to the outer walls (which get
  windows, as today).
- The flat band between walkway and outer wall should be a believable flat
  depth. If the floor is too deep for one band, a second ring corridor or
  cross-corridors are the builder's call.
- Flats are locked, numbered and owned exactly as today. A laundry every few
  levels can stay.
- **Shafts:** lifts and stairs move from the floor centre (where the atrium
  is) to the atrium's edge, at the same tile on every level as the rules
  require. This is a plan-level override for megablocks only.

**Ground level (0)**
- The atrium floor is walkable ground: a plaza.
- The lobby and reception face the main street door.
- A few shopfronts and a diner or noodle bar sit around the plaza, with
  `Counter`, `Table` and `Bar` stations so the sim's workers and visitors
  stand there, as in bars today.
- The stretch of the plaza seen from above should have something on it
  (stalls, tables, planters), so the look-down shows life without people.
  Exact furnishing is the builder's call.

**Other levels**
- Basement (-1) and the roof work as today.
- The Skyways bridge level is an atrium level with its bridge exit.

**New terrain, appended at the end of `Terrain` for save compatibility**
- `Atrium`: the open drop. Not walkable, doesn't block sight. Drawn as the
  look-down (§3).
- `Railing`: the edge. Not walkable, doesn't block sight. A thin glyph in the
  building's wall colour.

**Validation:** atrium floors must pass `validate_floor`. Atrium and railing
count as non-walkable; the walkway ring must keep the floor one connected
piece; every exit must still reach the shafts.

### 3. The look-down

Decided with Jeffrey: **you see the ground plaza from any height.**

- Because the atrium is the same rectangle on every upper level, the only
  thing straight below an `Atrium` cell is the ground plaza (level 0). On any
  upper level, an `Atrium` cell at p draws level 0's tile at p (plaza floor,
  stalls, tables, furniture), dimmed.
- **Dimming by height:** level 1 is about as dim as remembered terrain
  (`apply_vis(.., Remembered)`). It gets dimmer the higher you go, down to a
  floor that's still legible, never black. The exact curve is the builder's
  call. The current level must stay clearly brighter than the look-down.
- **Static:** terrain and furniture only. No people, no animation, no
  lighting flicker.
- **Visibility:** only for atrium cells the player currently sees or
  remembers on the current level. Unseen atrium stays unseen.
- **Cost:** only level 0 has to be generated (through the existing cache)
  when the player arrives on an upper level of an atrium megablock.
- **Content:** the plaza area under the atrium is what gets seen from every
  level, so it must be furnished (§2).
- **Implementation:** an overlay or a `MapView` extension. Builder's call.

### 4. The player's flat (layout A)

```
 ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░    ← window wall (outer wall)
█████████████████████████████████████
█ ▤▤   ▭▣▭  █  ▬▬▬▬▬▬          ═══ █
█ ▤▤        █              ○   ═   █
█           +  ▬               ═   █
█████+████████                     █
█ ≋  █  ≣≣≣ █                      █
█    + ≣  ▮ +                      █
███████████████████+████████████████
                walkway
```

- **Rooms:**
  - Front door from the walkway into a big open living room and kitchen
    along the window wall: couch, table, kitchen counter.
  - Bedroom at the back: bed, plus the desk with the terminal (`▣`) beside
    it.
  - A short hall to a shower room and a storage room (shelving, lockers).
- **Shape:** hand-shaped, not random. The exact tile layout is the builder's
  call within this shape: about 35×9 inside, window wall on the outside,
  front door on the walkway. It replaces two to four ordinary flat slots on
  its level.
- **Where:** in the city's (first) megablock, on an upper level. My call:
  roughly the upper third, never the bridge or the top floor. Picked
  deterministically from the city seed, in an outer-wall flat band so the
  living room has windows.
- **New furniture, appended at the end of `Furniture`:**
  - `Couch` (`▬`)
  - `Shower` (`≋`)
  - Existing pieces for the rest: `Bed`, `Desk`, `Counter`, `Table`, `Shelf`,
    `Locker`.
  - The terminal is `Feature::Terminal`.
- **Locks:** the flat is the player's. Its front door opens for the player
  and stays locked to nobody else (no NPCs walk indoors today anyway). Inner
  doors are ordinary open doors. Bumping it from the walkway says something
  like "home · flat 1712". How to model ownership and the inner rooms so
  `validate_floor`'s sealed-private-room rule still holds (for example, one
  private room with inner partitions, or a dedicated `RoomKind::Home`) is the
  builder's call.
- **Nobody else's:** the sim doesn't assign any resident to the player's
  flat.

### 5. Starting in the flat

- NEW GAME, NEW GAME (DEBUG) and LOAD GAME all put the player in the flat,
  standing beside the bed. My call. This happens in place of
  `Player::spawn`'s street pick in `MapScreen::enter_walk`:
  1. Find the home building and level.
  2. `ensure` the floor, and level 0 for the look-down.
  3. `Place::Floor`.
  4. Set FOV as `interiors::land` does.
- **Fallback:** if a city has no atrium megablock (an old save with only
  small Megablocks, or a city too small for a Megablocks district), spawn on
  the street as today. Old saves must load and play.

### 6. The home rig: jack in

- Standing within reach of the home terminal, the world popup and Enter offer
  **jack in**, exactly as at a street terminal.
- **Only the home rig** gets a port. Other indoor terminals, if any, still
  don't jack in.
- **Landing:** on the Net stratum, at a walkable tile just outside the
  megablock's own construct gate. My call. It matches how street jacks land
  on the open grid, and stepping into your own building's subnet stays a
  choice.
- **Body:** stays in the flat (`(Place::Floor, pos)`). Jack out (popup, Enter
  or being dumped) returns the player to the flat with the floor's FOV
  refreshed. Fix the callers that skip that refresh today.
- **Refusals:** deck cooling applies as usual. My call: the "port locked
  down" refusal (building on alert) does **not** apply to the home rig. It's
  your private line, and the alert is on the building's subnet, not your
  flat.
- The test asserting "nothing inside a building reaches the Net" changes to
  "nothing inside a building except the home rig".

## Calls I made (Jeffrey, check these)

1. One megablock per Megablocks district, between one and two per city.
2. A megablock fills a whole superblock (floors around 120×60).
3. Other Megablock lots become Apartments and Shops.
4. Housing capacity scales so the population stays roughly the same.
5. The flat sits in the upper third, outer wall, picked from the seed.
6. You start beside the bed.
7. Jacking in from home lands outside your building's construct gate.
8. The home rig ignores building-alert lockdown.
9. Couch and Shower are added as furniture. Kitchen, storage and desk use
   existing pieces.

## Testing

- **citygen:**
  - Every city has one or two megablocks, each a whole block where possible.
  - Generation stays deterministic.
  - Every atrium floor in the seed sweep validates.
  - Atrium and railing never split a floor.
  - Shafts line up across levels.
  - The home flat exists, has its rooms, the terminal and windows, and is
    reachable from the lifts.
- **strata-zero:**
  - New game and debug start on `Place::Floor` inside the home.
  - Load game respawns there.
  - The look-down draws the ground plaza dimmed by height (never black) and draws no people.
  - Jack in from the rig lands outside the construct gate, and jack out
    returns to the flat with FOV refreshed.
  - The rig ignores alert lockdown.
  - Old saves (format 3) still load.
- **Smoke:** drive a new game, screenshot the flat, walk to the walkway (no
  switch), look over the atrium, ride a lift, step outside, jack in from the
  rig and jack out.
