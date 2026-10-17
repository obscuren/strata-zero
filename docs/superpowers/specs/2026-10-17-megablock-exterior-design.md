# The megablock's outside

Date: 2026-10-17. Status: approved in chat ("lets go"). Follows
`2026-10-17-megablock-home-design.md` and builds on its branch.

## Intent

The megablock is the player's home building. On the street it's a big
footprint with plain walls. Jeffrey wants the outside to "deserve a little
something" when a megablock is stamped down, so the building reads as a
lived-in place from the street. It still has to fit in among the other
buildings.

## Goals (Jeffrey's picks: 1, 2, 5 and 7)

1. **A forecourt at the main door.**
   - A small paved apron of `Plaza` terrain on the sidewalk in front of the
     main entrance, about 7 wide and 3 deep.
   - A canopy over the door, two vending machines, a bench, a public street
     `Terminal` (jackable like any street terminal), and a lamp on each side
     of the door.
2. **The name over the entrance.**
   - The building's `name` is spelled in sign glyphs on the façade wall over
     the main door, centred and cut to what fits.
   - Looking at the sign reads the full name.
3. **A service side.**
   - A second street door on the side away from the main entrance.
   - Dumpsters (`Trash`), a loading bay marked with `Barrier`s, and one lamp.
   - The door works both ways. Inside, on the plaza floor (level 0), it opens
     into a small service corridor joined to the lift lobby. It is unlocked
     for now; locks and keypads come with the credentials work.
4. **Street spill-over.**
   - One food `Stall` near the forecourt.
   - A short row of parked bikes along the kerb. Bikes are new: a furniture
     or feature glyph only, with no sim behaviour and no riding.

## Non-goals

- Lit windows tied to who's home (idea 4) and shopfronts that match the plaza
  shops (idea 3) were not picked. Turf graffiti (idea 6) was not picked
  either.
- No locks, keypads or credentials on the service door.
- No vehicles beyond static bikes.

## Anti-goals

- The block must not stand out. Nothing brighter than a normal shop's neon,
  and nothing at street level that you wouldn't find elsewhere in the city.
- No broken connectivity: every street tile that was walkable stays reachable,
  the road stays clear, and the forecourt never paves over a road tile.
- Other buildings and other cities must not change. Seeds without a megablock
  generate exactly as before.

## Constraints

- Generation is deterministic from the seed. The exterior dressing must not
  shift any other stage's or pass's RNG stream. Use its own stream (for
  example hashed from the seed and the building id), so existing fixtures
  stay put: medium seed 8 still has its home at 18F, flat 1814.
- Saves hold the City (FORMAT_VERSION 3). An old save keeps its old exterior,
  and the service door only exists if the saved city has it. Interiors are
  derived from the city's street doors, so this follows on its own. No format
  bump unless the builder finds one is needed.
- Interior exits already come from the building's street door portals
  (`layouts.rs` `exits()`). The plaza floor must still pass `validate_floor`
  with the second exit: the exit reaches the shafts and the floor stays one
  walkable piece.
- It applies to every megablock (one or two per city), atrium or not. The
  service door's inside corridor is only for atrium plaza floors; a small
  megablock's ordinary lobby just gains the exit the way any building would.

## Calls made after the builder check (Claude's, not Jeffrey's)

- **When it runs:** after Naming, so the sign spells the final name, including
  any landmark renaming. The service door is added in the same pass. Interior
  floors are generated on demand from the finished city's door portals. If a
  portal or door list is cached in an earlier stage, the pass registers the
  new door there too.
- **The main door:** among the megablock's existing street doors, the one
  nearest the middle of its side. It is kept as it is, and any other existing
  doors are left alone.
- **Too little sidewalk:** the forecourt may shrink to nothing. The sign, the
  canopy and the door lamps are still placed, because they sit on or beside
  the façade.
- **Blocking:** placed pieces block movement as they do elsewhere in the city.
  None may sit on a door's landing tile or cut a door off from the road. Test:
  every outdoor tile that was walkable and connected before the pass is still
  connected after it.
- **Two street exits:** the builder adds a test that the atrium plaza floor
  validates with the service exit, across the medium fixture seeds.
- **Old saves:** no migration. A saved city without the dressing just has
  none, and looking at things works as before.

## Left to the builder

- **Picking the main door and the service side.** "Away from the main
  entrance" means the opposite side where there is a street, otherwise one of
  the two flanks, otherwise no service door.
- **Shrinking.** The forecourt shrinks when the sidewalk in front is too
  narrow, and the dressing skips any piece that doesn't fit, rather than
  overwriting roads or doors.
- **Glyphs and colours.** For the canopy, bench and bikes: reuse existing
  features where one fits (for example `Machine` for the vending machines), and
  add new variants at the ends of their enums where none does, with
  legend entries.
- **How the name sign is stored.** It can be several `Sign(char)` tiles, or a
  new feature that carries the building id. Either way, looking at it must
  read the full name.
- **Whether this is a new dressing pass or part of `Lots`/`Dressing`**,
  provided the RNG constraint holds.
