# Lighting and time of day

Date: 2026-10-18. Status: approved design. Grows out of the riff notes in
`docs/ideas/2026-10-17-lit-sight.md`, which stay as the record of what was
considered and why.

## Goal

Darkness limits how far you see; light does not. A lamp, a neon sign or a lit
window in your line of sight is visible from far away, and so is the pool of
ground it lights. The city has a real day: the three sunlit strata brighten
from dawn, lamps switch off, sight runs to the viewport edge, and the picture
takes a sky tint. The same sight rule works from the other side, so the city's
people can see you when you stand in the light and miss you in the dark, and
the HUD tells you how exposed you are. Combat and stealth build on that later.

Decided in the brainstorm:

- All three pieces of the riff notes ship together: lit sight, occlusion and
  day and night.
- Noon on the Street is real daylight, not a grey canyon.
- Day is a sky tint on top of brightness, not a second style pack.
- NPCs get eyes now. The first consumer is an exposure readout, not behaviour.
- Every number is a named constant in one tuning block.

## 1. The light model

Everything lives in `citygen` so the generator, the app and the sim read one
definition.

### One clock ramp

`lighting::daylight(minute_of_day: u16) -> f32` is 0.0 at night, rises
linearly from `DAWN_START` (05:00) to `DAWN_END` (07:00), holds at 1.0, and
falls from `DUSK_START` (17:00) to `DUSK_END` (19:00). The midpoints are 06:00
and 18:00, matching the night window the lowlight perk and the people sim
already use.

### Ambient by stratum and minute

`lighting::ambient(kind, minute) -> u8` interpolates between a night and a day
value by `daylight`:

| Stratum   | Night | Day |
|-----------|-------|-----|
| Undercity | 20    | 20  |
| Street    | 60    | 255 |
| Skyways   | 80    | 255 |
| Heights   | 110   | 255 |
| Net       | 0     | 0   |

The night column is today's `base_light`, so a city at 22:00 looks exactly as
it does now.

### Two emitter grids per stratum

`Stratum` loses `light: Grid<u8>` and gains:

- `steady: Grid<u8>`: light from fixtures that run all day. Neon, holo ads,
  helipads, street-level windows, and the Net's grid and lane light.
- `lamps: Grid<u8>`: light from street lamps, which switch off by day.

Both hold emitted light only; base is zero. The light at a tile is

```
light(kind, p, minute) = clamp(ambient(kind, minute) + steady[p] + lamps[p] * lamps_on(minute))
```

where `lamps_on(minute)` is true while `daylight(minute) < 0.5`. Lamps snap on
at 18:00 and off at 06:00. `Stratum::light_at(p, minute)` and
`Stratum::emitted(p, minute)` (steady plus active lamps, no ambient) are the
two reads.

### Occlusion in the bake

The symmetric shadowcaster moves from `strata-zero/src/walker/fov.rs` to
`citygen/src/fov.rs` unchanged; the app module becomes a re-export.
`lighting::compute` gains a `blocks: impl Fn(Pos) -> bool` parameter and, for
each emitter, casts `fov::compute` from the emitter over its radius and adds
the linear cone only to tiles the cast marks. The blocking tile itself is
marked, so walls and facades glow; the far side of a wall does not. Windows do
not block sight, so a street window keeps lighting the pavement. The bake
runs once per stratum at generation and is deterministic.

`compute` is called twice per stratum, once with the steady emitters and once
with the lamps, both at base 0. The emitter table (`emission` in
`dressing.rs`) moves to the tuning block as `steady_emission(terrain, feature,
kind)` and `lamp_emission(feature)`.

### Interiors and constructs

Floors and constructs keep their single baked `light` grid at their own base.
They have no sky and no clock. The floor bake passes a `blocks` predicate too,
so a lamp stops at the room's walls. Construct light is written directly, as
today.

### Tuning block

`citygen/src/lighting/tuning.rs` holds every constant in this design with a
doc comment each: the ambient table, the four ramp minutes, the emitter radii
and strengths, `LIT_SIGHT`, `LIT_RADIUS`, `DAY_RADIUS`, the exposure bands,
and the sky colours and weights. No other file carries one of these literals.
The dev console's existing `clock hh:mm` command is how any hour is reached
for a look.

## 2. The sight rule

### One predicate

`citygen::sight::lit_or_near(origin, p, dark_radius, emitted, is_emitter) -> bool`
says a tile already known to be in line of sight counts as seen when any of:

1. `origin.chebyshev(p) <= dark_radius`;
2. `emitted(p) >= LIT_SIGHT` (40): the tile glows on its own, ambient excluded;
3. `is_emitter(p)`: the fixture itself, so a window emitting 30 still shows
   across a dark plaza.

A street lamp (120 over 6 tiles) passes clause 2 within four tiles; neon (90
over 4) within two; a holo ad (100 over 5) within three.

### Dark radius by daylight

`walker::Player::radius(minute)` keeps the night constants (Street 14,
Undercity 8, Net 24, interiors 30) and, on the three sunlit strata,
interpolates toward `DAY_RADIUS` (60) by `daylight`. The constants keep their
names; their doc comment says "in the dark". The Undercity, the Net and
interiors never change.

### The player

`Player::update_fov(map, sight_bonus, minute)` casts over
`max(dark_radius + sight_bonus, LIT_RADIUS)` with `LIT_RADIUS` 40, and the
mark closure applies the predicate with `dark_radius + sight_bonus`. Perks add
to the dark radius as today. The lowlight ability keeps its two tiles.

### Exemptions

The Net keeps the plain radius rule: its grid light is 200 everywhere and the
lit clause would make sight unlimited. Interiors keep the plain radius rule:
base 150 already sees across a room. `TileMap` gains `fn lit_sight(&self) ->
bool`, true only for the four physical strata, so `update_fov` need not match
on the place.

## 3. Perception and the exposure readout

### `sees`

`citygen::sight::sees(from, to, dark_radius, blocks, emitted, is_emitter) -> bool`
walks a Bresenham line from `from` to `to` (exclusive of both ends) and fails
on any blocking tile, then applies `lit_or_near` to `to`. One line walk per
observer; for a single target tile the answer matches a shadowcast. The rule
is not symmetric by design: someone under a lamp is seen from the dark and
does not see the dark.

### Who looks

Every walker the sim places on the player's stratum, outside a building, with
the stratum's dark radius for the minute and no perks. Cars and chat markers
do not look. The sim stays ignorant of the player; the app lists walkers via
`World::visible` over a square of `DAY_RADIUS` around the player.

### Exposure

```rust
pub struct Exposure { pub light: u8, pub eyes: Vec<PersonId> }
impl AppState { pub fn exposure(&self) -> Exposure }
```

Computed on demand each frame on a stratum. `light` is `light_at` on the
player's tile, ambient included. `eyes` is every walker for whom `sees` is
true. Off the strata it is `{ light: 0, eyes: [] }`.

### The HUD

`hud::location_line` gains a third segment between the location and the
clock: a light word and the eye count. The word comes from
`lighting::tuning::EXPOSURE_BANDS`: `dark` below 40, `dim` below 100, `lit`
below 180, `bright` otherwise. The count reads `unseen` at zero and `N eyes`
otherwise, so a line reads `dim · 2 eyes` or `bright · unseen`. Plain text,
no colour, cut before the location when the width is short.

### Dev overlays

Two console commands toggle fields on `AppState`:

- `eyes`: the people overlay draws walkers who can see you with a red tint.
- `light`: the viewport replaces each visible tile's colours with a grey from
  black to white by its light value, glyph kept, so a tuning change can be
  seen against the map.

## 4. Rendering

### The map knows the minute

`StratumMap` gains `minute: u16`. `TileMap::light(p)` on a stratum returns
`light_at(p, minute)`. `MapRef::stratum_of(city, kind)` stays and builds at
`START_MINUTE_OF_DAY` (22:00); it is for tests and for screens before a world
exists. Live code uses `MapRef::stratum_at(city, kind, minute)` and
`MapRef::for_place(.., minute)`, with the minute from
`AppState::minute_of_day()`, which reads the sim world's clock or derives it
from the tick when there is no world yet.

`TileMap` also gains `fn lamps_on(&self) -> bool`, true on a stratum when
`lighting::lamps_on(minute)`, true elsewhere.

### The sky tint

`lighting::sky(kind, minute) -> Option<((u8, u8, u8), f32)>` returns a colour
and a weight for the three sunlit strata and `None` otherwise:

- full day: `SKY_DAY` (200, 215, 235) at `SKY_DAY_WEIGHT` (0.18);
- in a ramp: `SKY_DUSK` (235, 150, 80) at `SKY_DUSK_WEIGHT` (0.25) times a
  tent over the ramp that peaks at its midpoint, blended with the day colour
  as the ramp rises;
- night: `None`.

The tileset's lighting step mixes foreground and background toward the sky
colour after the brightness scale. Emitting tiles keep their own colour.

### Lamps off are not emitters

`RenderCtx` gains `lamps_on: bool`. The tileset's "this tile emits so keep its
colour" check treats `Feature::Lamp` as an emitter only when `lamps_on`.

### Nothing else

Visibility stays a hard mask. Remembered tiles stay dim and static. A lit tile
beyond the dark radius draws as it would up close. The memory map and minimap
read visibility only.

## 5. Saves, moves and tests

### Save format

`FORMAT_VERSION` becomes 4 and joins `READABLE_VERSIONS`. `load_city` checks
that `steady` and `lamps` both match the city size. Older files produce the
existing "regenerate it from seed" error.

### Module moves

- `strata-zero/src/walker/fov.rs` becomes `pub use citygen::fov::*;` with its
  tests moved to citygen.
- `emission` and `base_light` leave `dressing.rs` for the tuning block.
- `generate/cyberspace.rs` writes `GRID_LIGHT` and `LANE_LIGHT` into `steady`.
- `AppState::sight_bonus` keeps its night test but reads `lighting::lamps_on`
  so the perk and the lamps agree.

### Performance

A shadowcast at radius 40 touches a few thousand tiles per move. Exposure
runs per frame on the walkers within `DAY_RADIUS`, each a line walk of at
most sixty steps.

### Tests

citygen:

- `daylight` is 0 at 22:00, 1 at 12:00, 0.5 at 06:00 and 18:00.
- `ambient` at night equals the old bases for every stratum; at noon the
  sunlit strata read 255 and the Undercity 20.
- `lamps_on` at 05:59 and not at 06:01, and at 18:01 and not at 17:59.
- A lamp beside a wall lights the wall and not the tile behind it.
- A generated Street stratum has nonzero `lamps` and nonzero `steady`, and
  `light_at` at 22:00 on a lamp tile exceeds the old baseline.
- `lit_or_near`: each clause passes alone; a dark far tile fails.
- `sees`: from the dark to a lamp tile is true; the reverse is false; a wall
  between fails.
- `sky` is `None` at night and on the Undercity, `Some` at noon on the Street.

strata-zero:

- `update_fov` at night pulls a lamp's pool past the radius into view and
  leaves a dark tile at the same distance `Unseen`.
- `update_fov` at noon on the Street marks a floor tile 40 away `Visible`.
- The Net and a floor are unchanged by the hour.
- The memory map test's lit-fraction bound relaxes to under half the screen.
- `exposure` counts a walker in the lamp pool and not one behind a wall;
  reports the player's tile light.
- `location_line` renders `dim · 2 eyes` and `bright · unseen` and still ends
  with the clock.
- A lamp by day is not an emitter in `tileset::cell`; by night it is.
- `save_city` then `load_city` round-trips both grids; a version 3 header is
  rejected.

## Out of scope

- Any behaviour on being seen. Stealth and combat consume `Exposure`.
- The lowlight ability as a lower threshold.
- Interiors or constructs by the hour.
- Weather by the hour.
