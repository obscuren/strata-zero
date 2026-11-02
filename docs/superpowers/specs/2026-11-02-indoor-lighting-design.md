# Indoor lighting: fittings that suit the room, in the room's own tone

Date: 2026-11-02. Mockups: `2026-11-02-indoor-lighting-mock/` (`round1.html`: Now, A,
B and C over six interiors; `round2.html`: Now, A + B, A, B), with their generator.

## Why

Outside, the sodium street lamps are right and stay as they are. Indoors every room gets
the same lamp: `Feature::Lamp`, drawn `●`, looked at as "Street lamp" ("Sodium light and a
hum you stop hearing"), planted on a fixed six-tile grid (`layouts.rs` `lamps()`) whatever
the room, on a floor already lit to 150 of 255 (`INTERIOR_BASE`). A flat, a bar, a
corridor and a surgery read alike, and lamps sit in the middle of beds and walkways.

## What Jeffrey decided

- **Two kinds of fitting, both kept.** Ceiling fittings (mock A: tubes, panels, bulbs,
  pendants over tables, the surgical lamp, high-bays) and wall fittings (mock B: strips,
  sconces, a neon strip behind a bar's counter). A room is lit by ceiling fittings, by
  wall fittings, or by both.
- **Every room has a tone:** a colour and a level by what lights it, "how real lighting
  works": cold green-white fluorescent, blue-white LED, warm tungsten, hard surgical
  white, sodium in halls, neon and blacklight in bars and clubs.
- **A room's mood picks ceiling, walls or both**, and the mood is set when the city is
  generated and kept: one dive bar is always dark, another down the street always lit.
  A bar on walls alone is dark and cosy (neon behind the counter, sconces, the tables in
  shadow); the same bar on both is livelier (pendants over the tables too).
- The street keeps its sodium lamps; interiors no longer use them.

## Goals

- No interior floor has a `Feature::Lamp`. Every interior's light comes from its rooms'
  tones and fittings.
- A room's kind reads from its colour: a corridor, an office, a flat, a surgery and a bar
  look different at a glance.
- Two places of the same kind can look different (their moods), and a place looks the
  same on every visit and every regeneration from the seed.
- Fittings never take a walkable tile from anyone: ceiling fittings are walked under, wall
  fittings sit in walls, and none sits on furniture, a door, an exit or a doorstep.
- The rules work for every room kind and building kind: what a room gets follows from its
  tone and what stands in it (tables, a counter, a bed, an operating chair), never from a
  named building. Look text comes from the fitting's kind and wear.
- Both front ends: exact `Rgb` colours; every glyph in the window's fonts.

## Non-goals

- Light switches, shooting out a fitting, light-based stealth indoors. Indoor exposure
  stays as it is (`exposure::of` reads only the strata) and indoor sight stays the plain
  radius (`update_fov`); interior light stays cosmetic.
- Moods that change with the hour or the story. Interior lights burn all day, as today.
  The story's flat dimming (`missions.lights`) keeps working on top.
- The streets, the Net and constructs.

## Anti-goals

- Clutter: ceiling and wall fittings everywhere. A room takes one mood; "both" is a
  mood some rooms allow, not the default.
- A map too dark to read: the floor never drops below the minimum `lit()` keeps today.
- Fittings drawn in the same box-drawing lines as walls (they read as walls).

## Design

### Data (citygen)

- `Feature::Fitting(Fitting)`, appended last in `Feature` so old saves keep their indices.
  `Fitting { kind: FittingKind, tone: Tone, wear: Wear }`, three small `Copy` enums.
- `FittingKind`:
  - ceiling, on a walkable floor tile and walkable: `Tube`, `Panel`, `Bulb`, `HighBay`;
  - wall, on a `Wall` tile: `Strip`, `Sconce`, `NeonStrip`;
  - `Stage`, on each tile of a club's stage (today a block of `Lamp`s), walkable.
- `Tone`: `Fluorescent`, `Led`, `Tungsten`, `Surgical`, `Sodium`, `Neon(Hue)`,
  `Blacklight`. Each has a light colour in `lighting::tuning` (the mock's):
  fluorescent (185, 232, 222), LED (175, 205, 255), tungsten (255, 168, 88), surgical
  (228, 242, 255), sodium (255, 150, 55), blacklight (175, 95, 255), and a colour per
  neon hue (magenta (255, 60, 175), cyan (0, 225, 210), the rest from the hue).
- `Wear`: `Ok`, `Dead`, `Stutter`.

### Each room's light (tuning)

`lighting::tuning` gets one table from `RoomKind` (with a building-kind override) to a
tone, a level (the ambient the tone gives the room, before mood) and the moods it allows
with weights. Mood is `Ceiling`, `Walls` or `Both`. The table, as drawn in the mock:

| Room | Tone | Level | Moods |
|---|---|---|---|
| Corridor | fluorescent | 92 | ceiling 3, walls 1 |
| Laundry, Lockers, Toilets, Storage | fluorescent | 92 | ceiling |
| Waiting | fluorescent | 100 | ceiling 2, both 1 |
| Lobby | LED | 120 | walls 1, both 1 |
| Shop | LED | 120 | walls 1, both 2 |
| OpenPlan, Office | LED | 108 | ceiling 3, both 1 |
| Flat, Home, HotelRoom, Suite | tungsten | 60 | walls 3, ceiling 1 |
| BackRoom | tungsten | 55 | ceiling 3, walls 1 |
| Bar, Vip | tungsten | 50 | walls 1, both 1 |
| Shrine | tungsten | 50 | walls |
| Surgery | surgical | 128 | both |
| Hall, Bays | sodium | 78 | ceiling 3, both 1 |
| Stage | neon | 60 | (its own: stage fittings) |

Building-kind overrides: in a `Club`, Bar and Vip take blacklight; in a `Temple`, Hall
takes tungsten with walls only (sconces read as candles). Floor tiles in no room take the
Corridor row.

The mood scales the level: walls ×0.75 (the cosy room), ceiling ×1.0, both ×1.15.

**Which mood:** picked by weight from `hash(city seed, building, room kind, room number)`.
Public rooms have number 0, so every corridor or bar of one building shares a mood; flats
and hotel rooms carry their numbers, so each home has its own. Interiors are never saved
(`interiors.rs`), and the seed brings the same mood back.

### Placing fittings

After `connect()` (so no door is cut through a fitting later) and before `finish()`,
`Builder::light()` replaces `lamps()`. Fittings go **per part**: a room's tiles split into
the pieces walls and doors cut them into (a flat's bedroom, hall, shower room and living
room are four parts of one `Home`), and each part takes the rules below, by its room's
tone and mood. A **bar counter** is a counter with a `Role::Bar` station behind it (staff);
a kitchen counter is not. Nothing goes on a reserved or clear tile, furniture, a door, a
window or the player's spawn; a wall tile already holding a fitting is skipped by the next
part (the first part to claim a shared wall keeps it).

**Ceiling** (mood ceiling or both):
- Fluorescent, LED, sodium, surgical parts: tubes lying along the rows (cells are tall),
  one cell in a part up to 4 wide and two cells otherwise; every 6 columns from the
  part's left + 2 (a part up to 7 wide: one, centred), every 4 rows from its top + 2 (a
  part up to 4 high: its middle row); a blocked spot tries the next row down, else is
  skipped. Kind by tone: fluorescent `Tube`, LED and surgical `Panel`, sodium `HighBay`.
- Tungsten and blacklight parts: a part with a bar counter gets a pendant over each
  table and a glow over the counter (light only: the lit table is what you see); else
  a part with a bed gets one `Bulb` on the free tile nearest the bed; else one `Bulb` at
  the free tile nearest the part's middle.
- A part with an operating chair gets the surgical lamp over it (light only).

**Walls** (mood walls or both), on the part's own wall tiles:
- Fluorescent, LED, sodium, surgical parts: `Strip`s in runs of three with three dark
  between, counted along the wall; a surgical part's walls lit whole.
- Tungsten and blacklight parts: a part with a bar counter gets a `NeonStrip` in every
  wall tile within 2 of the counter (the backbar; magenta, cyan in a club) and a `Sconce`
  every 5 tiles along its other walls; else a part with a bed gets one `Sconce` in the
  wall tile nearest the bed; else a `Sconce` every 5 tiles along its walls (a temple's
  hall reads as candles; a living room gets two or three).

**A wall fitting lights its own side only.** Its light is emitted from the floor tile in
front of it, on the part's side, not from the wall tile; the wall tile still glows (it is
drawn lit, and walls take the light that hits them). A sconce by a bed never lights the
flat next door, and a corridor's strips never light the flats along it. The builder keeps
the emitters in a list as it places fittings (wall fittings at their inward tile,
pendants, counter glows and the surgical lamp at their furniture), so `finish()` lights
from that list plus every `Terminal`, not from tile features.

**Stage:** every stage tile a `Stage` fitting, rows alternating magenta and cyan.

**Wear:** fluorescent and sodium fittings only (LED and surgical don't fail visibly). A
tube or strip run is one fitting for wear: about one in nine dead and one in nine
stuttering, from a hash of the floor seed and the fitting's first tile.

**Clinics:** the templates' `*` marks become plain floor; the rules above light clinics
like every other floor.

### Light

- `lighting::compute_coloured(size, base, blocks, emitters)`: `base` gives each tile its
  ambient as an RGB (tone colour × level × mood ÷ 255); `emitters` is a list of
  `(pos, radius, strength, colour)`. The same cone and wall rule as `compute`: walls
  take the light that hits them. Returns each tile's light as `(r, g, b)`, clamped.
- A wall, door or window tile's ambient is the brightest (per channel) of its floor
  neighbours' ambients, so a bar's frame walls take the bar's warm tone.
- Emitters (radius, strength), from the mock: tube or panel 5, 200 shared over its
  cells; stuttering tube 3, 60; dead nothing; high-bay 6, 220; bulb 5, 190; pendant 3,
  150; counter glow 2, 55; surgical lamp 3, 210; strip 3, 105 (surgical 130); one sconce
  5, 190; a bar's sconces 3, 95; neon strip 4, 110; stage 3, 60; a `Terminal` (the rig,
  any screen) 3, 90 in screen cyan.
- `Floor` gains `glow: Grid<(u8, u8, u8)>`, the light's colour on each tile.
  `Floor::light` stays, as the brightest channel of `glow`, so everything that reads a
  number (the dev light overlay, weather, marks) keeps working.
- `INTERIOR_BASE` and `INTERIOR_LAMP` go.

### Drawing (strata-zero)

- `TileMap::glow(p) -> Option<Rgb>`: a floor's tile light colour; `None` on strata and
  constructs, which keep today's grey path. `RenderCtx` gains `glow: Option<Rgb>`, set
  where it is built (viewport, atrium, marks).
- `cell()` with a glow `L`: each channel of the background is
  `c × (0.22 + 0.78 × L/255) + L × 0.10`, and of a foreground that doesn't emit
  `c × (0.30 + 0.70 × L/255) + L × 0.16`. Without a glow, `lit()` as today. The sky and
  remembered/unseen handling stay as they are.
- Fittings:
  - `Tube`, `Panel`, `HighBay`: `▬` in the fitting's colour (a pale tint of the tone:
    fluorescent `#eafffa`, LED `#e8f0ff`, sodium `#ffc070`); `Bulb`: `•` in `#ffd9a0`,
    bold. They emit (foreground not lit).
  - Wall fittings: the wall's own glyph (the wall mask is terrain-based and keeps
    working) in the fitting's colour, bold, on its colour × 0.22.
  - `Stage`: the floor's `·` over a 0.45 wash of its colour.
  - `Dead`: `▬` (or the wall glyph) in `#3e4750`, not emitting. `Stutter`: the colour ×
    the neon `flicker(tick, variant)`; its pool stays steady and dim.
- **Furniture in strong light takes the light's colour:** where a furniture tile's glow
  has a channel of 180 or more, its foreground mixes 70% toward the glow scaled up to a
  brightest channel of 255. A table under a pendant shows amber (violet in a club), the
  operating chair under the surgical lamp white, with no marker in the data.
- The story's flat dimming, the dev light overlay and `apply_vis` are unchanged.

### Look

- Names from kind and wear: "Fluorescent tube", "LED panel", "Bulb", "High-bay lamp",
  "Light strip", "Sconce", "Neon strip", "Stage lights"; a dead one "Dead fluorescent
  tube", a stuttering one "Flickering fluorescent tube" (and so on for strips and
  high-bays).
- `flavor/looks.ron` `feature` keys: `tube`, `panel`, `bulb`, `high_bay`, `strip`,
  `sconce`, `neon_strip`, `stage_lights`, and `_dead` / `_flicker` variants for the kinds
  that wear; a missing variant falls back to the kind's key.
- Fittings are not added to the look cursor's "worth a look" stops (a floor of tubes
  would bury the people and doors); the cursor still names one it rests on.

### Shots

An ignored shot test, `indoor_lighting`, writes the six mock interiors (the flat, the
clinic, a bar, a club, an office floor, a hotel floor), map only, for comparison with the
mock.

## Calls made in the writing (for Jeffrey to check)

- The bar treatment (neon backbar, pendants) keys on a **staffed** counter, so a flat's
  kitchen counter doesn't make it a bar.
- Rooms are lit **per walled part**, so every part of a flat or a clinic gets light.
- Wall fittings light **their own side** only; a shared wall goes to the first part.
- A tungsten part with no bed and no bar counter gets **sconces every 5** (the temple).
- Furniture in strong light **takes the light's colour** (amber tables, white chair).
- Wear only on fluorescent and sodium fittings; flats and hotel rooms pick their mood
  **per room**, public rooms per building.
- Fittings are left out of the look cursor's stops.

## Testing

- citygen: no interior floor of the seed-8 Medium city has a `Lamp`; every ceiling and
  stage fitting is on a walkable tile and leaves it walkable; every wall fitting is on a
  `Wall`; none is on a reserved or clear tile, furniture, a door or a window; the same
  building gives the same moods and fittings on every regeneration and on every floor;
  across the city's bars both moods occur; in a bar lit on both, every table tile is
  brighter than the bar's ambient; a surgery's chair tile is its brightest floor tile;
  a corridor's light is cooler than a flat's (blue over red), a flat's warmer; a wall
  fitting's light stays on its side (the tile behind its wall gets none of it); the
  start flat has no `NeonStrip` and every walled part of it has a fitting or a lit
  neighbour fitting.
- strata-zero: a warm glow makes a floor cell's background redder than bluer; a dead
  fitting is grey and doesn't emit; a wall fitting keeps the wall's glyph; every fitting
  glyph is single width (`style_glyphs_are_single_width` via `ALL_FEATURES`) and in the
  window's fonts (`every_drawn_character_has_a_glyph`); look names and flavour exist for
  every fitting kind.
- Both front ends per CLAUDE.md (fmt, clippy and tests with and without `window`).
