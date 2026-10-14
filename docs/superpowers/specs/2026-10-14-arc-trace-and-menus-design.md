# Arc Trace and menu fixes design

One round with two halves: a second deck ability with its own effect shapes,
and a set of interface fixes Jeffrey listed after playing.

## Arc Trace

**Content.** A new active ability granted by the pawnshop deck:

```ron
Ability(id: "arc_trace", name: "Arc Trace", cooldown: 450, kind: Active(Trace(radius: 20)),
        fx: [Bolt(colour: (255, 240, 150), ticks: 12), Mark(colour: (255, 240, 150), ticks: 600)],
        desc: "The deck dumps a charge into the local grid. It arcs along whatever will carry it, lights every node it finds, and leaves them burning on your map for a while."),
```

`Active::Trace { radius }` joins `Scan` and `Ping`; it has no game effect of
its own. `pawnshop_deck` grants `["netscan", "arc_trace"]`. The deck's
ability text reads `trace: arcs to every node within {radius} tiles`.

**Targets.** When a Trace ability fires, the app collects the targets: on the
Net, every node whose `cyber` tile is within the radius (chebyshev) of the
player; elsewhere, every node anchored on the player's stratum within the
radius. `FxInstance` gains `targets: Vec<Pos>`, empty for other abilities.

**Shapes.** Two new `Fx` shapes:

- `Bolt { colour, ticks }`: from the origin to every target, a line that
  grows over `ticks` and snakes. The base path is a Bresenham line; every
  point gets a lateral jitter of -1, 0 or 1 cells from a hash of the point's
  index, the target and `tick / 2`, so the line flickers between frames. At
  `elapsed` the first `len * elapsed / ticks` points of each line draw: a
  glyph by the step's direction (`─` `│` `╱` `╲`), one point in five a `*`,
  foreground the colour at full or 60% by hash, background the colour at
  20%. The line ignores walls and sight: it draws over unseen tiles.
- `Mark { colour, ticks }`: every target tile draws `◉` in the colour pulsed
  with `pulse(tick, 20)` on a background of the colour at 20%, DIM cleared,
  whatever the tile's visibility, for `ticks`.

Both only draw on the instance's stratum. `until` is `started + ticks` for
both.

## Menus loop

Every vertical list wraps at both ends: the title and pause menus
(`ui::menu::Menu`), the deck's rows (skipping rows without a detail as now,
then wrapping), the popup rows (skipping disabled rows, then wrapping), the
load list and the settings rows.

## Title and pause

Title: `NEW GAME`, `LOAD GAME`, `QUIT`. New game generates a city with the
current parameters at once (no configure screen; it stays reachable with `g`
in the dev camera) and drops the player onto the street walking. Load game
opens the load list as today. After any generation or load the map screen
starts in Walk mode, dev mode or not; `c` still opens the dev camera.

Pause: `RESUME`, `SAVE GAME`, `LOAD GAME`, `SETTINGS`, `QUIT TO TITLE`,
`QUIT GAME`, with today's behaviour behind each.

## The deck

Tabs, in order, keys `1` to `9`: SKILLS (what ABILITIES showed), ATTRIBUTES
(what SKILLS showed), GEAR, INVENTORY, WORKBENCH, CYBERDECK, QUESTS, JOURNAL,
MAP.

- CYBERDECK lists the deck in the Deck slot as its first row (name; detail:
  description, perks, weight and value) and then one row per ability it
  grants, titled by name with the ability's description and cooldown as the
  detail. With no deck equipped a single dimmed row `no deck jacked`.
- JOURNAL is a placeholder: one dimmed row `nothing written yet`.

The deck remembers where it was: the app state keeps the last tab and the
selected row per tab; opening the deck restores them and every change writes
them back.

## Dev tabs

`[F5]Inspect [F6]Legend` moves from the band to the last row of the right
column, with one empty row above it; the band row it leaves stays empty.
Outside dev mode the column keeps the row.

## Testing

- game: `Trace` parses, the deck grants both abilities, `Fire(arc_trace)`
  has no effects.
- app/actions: firing arc_trace pushes a Bolt and a Mark with the same
  non-empty targets on the Street; on the Net the targets are cyber tiles.
- render/fx: a Bolt at half its ticks draws its glyphs about halfway to a
  target and nothing beyond; a Mark draws `◉` on an unseen target tile; both
  stay on their stratum.
- menus: wrap tests for Menu, Deck rows, Popup rows, Files, Settings.
- title: three items; new game switches to Generating; the map screen enters
  Walk mode on its first update after generation.
- deck: tab order and keys; CYBERDECK rows with and without a deck; JOURNAL
  row; reopening restores the tab and row.
- explore: the dev tab row sits on the last row of the right column in dev
  mode and the band row 3 is blank.
- smoke: tab keys updated; `3` opens GEAR.
