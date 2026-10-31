# Tracking and the deck's MAP

Status: agreed with Jeffrey on 2026-10-31 through two rounds of mockups; he said "Build
it". This document is what gets built.

Mockups and their generators: `2026-10-31-job-tracking-mock/` beside this file.
- `round1.html` (`gen.py`): three looks for the tracked walk. He picked **C, floor glow**,
  and asked to keep the page for inspiration.
- `map1.html` (`gen2.py`): three deck MAP layouts. He picked **B, directory**.
- `map2.html` (`gen3.py`): B combined with C's exploration, and a pane that folds. This
  is the agreed layout; its three frames are the reference for §7.

## Why

Jobs and the story send you across the city, and nothing shows you the way: the JOBS
pane names a place, and you find it yourself. The deck's MAP tab is a coarse block of
colour that only shows what you have already walked. Jeffrey wants:

- a **track**: one target at a time, the walk to it drawn over the street, animated,
  "visible to the player", tracing towards the target;
- tracking from the JOBS tab, and from a better MAP where you filter what is shown
  (ripperdocs, fixers…), pick something and track it;
- great care that a track never lingers: it clears when you arrive, when its job ends,
  when something fails.

## What Jeffrey decided

- "From the jobs we should be able to 'track', this overlays a path towards the target
  (or suggested target area). This is a walkable line from our current position to the
  target area. It needs to be a pulsating A* (or similar) line that is visible to the
  player and 'traces' towards the target with an animation (brighter color, trailing,
  etc.)"
- The look: **C, floor glow** (round 1). The walk's tiles glow from below, a bright band
  slides along them from you to the target with a trail, rings ripple out of the target
  area. No glyphs are drawn; everything on the street keeps its own.
- "This should be usable for other things as well like selecting a target area on your
  map."
- "I want to improve the map from the cyberdeck in one go too. We should be able to
  filter on things that can be highlighted on the map. Like a ripperdoc for example.
  Selecting it allows us to track it as well and we should see it. From the jobs menu
  you can also track."
- "Each new 'track' overrides the previous."
- "We need extra care: when you reach your destination the tracking should be cleared.
  When something fails it should clear. etc."
- The map: B's list of places on the right, "easily scroll through", with C's
  exploration (a free cursor over the map). "B's right pane should be foldable to the
  right with a key. When folded an indication marker on the right it's folded."
- Round 2 (`map2.html`) as drawn: "Build it".

## Calls I made (Jeffrey: check these)

1. **The keys on MAP have no modes.** Arrows and `hjkl` move the cursor, `[ ]` step the
   places list, `< >` pick the kind, `t` (or space) tracks, `+ −` zoom, `f` folds. The
   deck keeps `q e`, the digits, tab and esc, so the kinds are on `< >`, not on digits
   as in round 1's B.
2. **One kind at a time**, as GEAR & INVENTORY filters: `all`, then each kind. Home, your
   jobs and the story, and the tracked place always show, whatever the kind. Round 1's B
   toggled several kinds at once.
3. **Every place shows**, including where you have never been: the deck pulls a city
   listing. The map itself stays faint where you have not walked.
4. **Ripperdocs show through their fronts**: a clinic is listed as a ripperdoc under its
   front's name.
5. **A job track follows the objective it was made for.** It clears when you reach that
   objective's place or when the objective is ticked; it does not move on to the next
   objective (the hand-in) by itself. A story track follows its step and clears when the
   step moves on: the step's own trigger is its arrival (you are home when `At(Home)`
   fires in your flat, not in the lobby), so a story track has no arrival of its own.
6. **"No way there" refuses, it does not clear.** `t` on something no walk reaches says
   so and leaves the old track. A track whose walk breaks later (you went where there is
   no way back) stays, says "no way from here", and draws nothing until a way opens.
7. **The walk crosses places a leg at a time.** On the street the walk goes to the
   building's door; inside, to the lift (or the stairs when lifts are locked down), then
   to the target. Between strata, to a connector. Each leg is drawn in the place you are
   in, and the pane says what comes next ("door, then 6F").
8. **A person target inside a building** is that building until you are inside it; you
   arrive on entering. Inside, you find them yourself.
9. **The map shows your stratum**, or the Street while you are inside a building or
   jacked in. Inside a building the walk on the map starts from that building's way out.
10. **Arrival radius:** a person, 1 tile; a building, its door or anywhere inside; a spot
    picked on the map, the tiles one map pixel covers at that zoom (at least 1); a story
    step at a workplace, the step's own `within`.
11. **The default kind is `all`, the default zoom 2**, and both, with the fold, last the
    session (as the deck's other tabs remember their rows).
12. **No persistence.** Like the story, a track lives for the session.
13. **A `◆` on the map is its job or its story**, not a place: `t` on it makes the same
    source as `t` on the JOBS tab, shows the same track state and clears the same way.
    A job whose target is a listed building (a Plant on a clinic) is two rows, the `◆`
    and the `+`; they share one marker cell, where the `◆` wins.

After the builder check (settled without asking):
- "Get home" leads to the flat: `At(Home)` resolves to a spot inside your flat on its
  floor, through the lift, and clears when the step fires (call 5).
- The door rule (§2) holds on strata, and for a floor's exits; the doors of flats and
  rooms inside a floor are walkable, or a Fetch behind one could not be reached.
- Between strata the leg picks the connector that leads on to the target, not merely
  the nearest (Skyways and Heights are pieces with a link each).
- A walk-in building (no door, no floors) is arrived at in or beside its footprint.
- On the Net grid, as in a construct, there is no leg: `jacked in`.
- The edge tag writes glyphs, so it is drawn after the overlay, not by it; the overlay
  itself only sets backgrounds.
- `Player::vis_at` becomes public, `interiors::level_for` `pub(crate)`.

## Words

- **Track**: what you follow; at most one. `AppState.track`.
- **Source**: what made it: a job, a story mission, a place from the map, or a spot.
- **Target**: where the source is now, resolved every tick: a spot in a place, a
  building, or a person.
- **Leg**: the walk in the place you are in, from you to the target or to the next way
  through (a door, a lift, the stairs, a connector).
- **Place**: in the code `citygen::Place` (a stratum, a building's floor, a construct).
  On the map, "a place" is a listed spot: a ripperdoc, a bar, a fixer. The code calls
  those **listings** to keep the two apart.

## Goals

- `t` on a job, the story, a listing or a bare spot of the map tracks it; `t` on the
  tracked thing stops it; a new track replaces the old one.
- The walk to the target glows on the street, in the floor glow Jeffrey picked, and
  rings ripple at the target; the walk follows you through doors, lifts and stairs.
- When the target is off the screen, a tag at the edge where the walk leaves names it
  and how far.
- The JOBS pane says what is tracked and how far; the JOBS tab's detail says it too.
- A track clears on arrival, when its job is done, failed, dropped or late, when its
  story step moves on, when its person dies, on a new city or a load, and when you
  flatline. Every clear says why in the log.
- MAP shows the city in half-block pixels, lit where you remember it and faint where you
  don't; markers for the shown listings; a free cursor; a places pane nearest-first that
  folds away; zoom; the tracked walk glowing on it.
- Everything fits 100×30 and draws the same in the window.

## Non-goals

- Following a track automatically (auto-walk). You walk; the track shows the way.
- A track that moves on to a job's next objective by itself.
- Saving tracks or the map's settings to disk.
- Routing through the Net, or into a construct.
- Showing people inside buildings on the map, or finding a person on a floor for you.
- Searching listings by typed text.
- Several tracks at once, or waypoints.

## Anti-goals

- A track that lingers after its reason is gone: a stale glow is worse than none.
- Drawing over the street: the glow is background only. No glyph on the street is ever
  replaced by the track.
- Hardcoding a kind of target or a job kind in the drawing or the routing: sources
  resolve to the same three target shapes, and everything downstream works on those.
- A new MAP that needs window-only code: it is ratatui cells like every screen.

## 1. The track (`crates/strata-zero/src/track/mod.rs`)

```rust
pub struct Track {
    pub source: Source,
    /// What the pane and the log call it: "Miracorp Tattoo", "The drop", "the spot".
    pub label: String,
}

pub enum Source {
    /// A taken job's objective `objective` (its index when tracked).
    Job { job: JobId, objective: usize },
    /// A story mission's waiting step `step` (its index when tracked).
    Story { mission: usize, step: usize },
    /// A listing from the map.
    Listing(ListingId),
    /// A tile picked on the map (or from the console), arrived within `radius`.
    Spot { place: Place, pos: Pos, radius: i32 },
}

pub enum Target {
    Spot { place: Place, pos: Pos, radius: i32 },
    Building(BuildingId),
    Person(PersonId),
}
```

- `track::set(st, source) -> Result<String, String>` resolves the source, checks a leg
  exists from where you stand, and replaces `st.track`. It refuses with the reason
  ("nothing to walk to yet", "no way there from here", "take it first") and leaves the
  old track. The message goes to the log: `⌖ tracking Miracorp Tattoo`.
- `track::stop(st)`: `⌖ stopped tracking Miracorp Tattoo`.
- `track::toggle(st, source)`: stops when `source` is the tracked one, else sets.
- `track::resolve(st, &Source) -> Resolved` where `Resolved` is `Target(Target)`,
  `Gone(reason)` (the source ended) or `Nowhere` (a story step with no place).
- `track::tick(st)` runs in the explore screen's update after `missions::tick`: resolve,
  clear when `Gone`, clear on arrival (§3), refresh the leg when you or the target moved
  (§2). It does nothing while no track is set.
- `AppState::enter_city_at` (new city, load) and flatlining set `st.track = None`.

How each source resolves:

| Source | Target |
|---|---|
| `Job`, objective 0, Fetch | `Spot { Floor{building, level}, tile, radius 1 }` |
| `Job`, objective 0, Copy or Plant | `Building(building)` |
| `Job`, objective 0, Deliver | `Person(to)` |
| `Job`, objective 1 (the hand-in) | `Person(fixer)` |
| `Story`, waiting on `Sees { role }` or `Talk(role)` | `Person(the role's person)` |
| `Story`, waiting on `At(Home)` | `Spot { your flat's floor, a walkable tile inside the flat, radius 0 }` |
| `Story`, waiting on `At(Work { role, within })` | `Spot { Street, work tile, radius within }` |
| `Story`, any other trigger | `Nowhere` |
| `Listing` of a building or a fixer | its building, or its person |
| `Listing` of a job or the story (`◆`) | never made: a `◆` makes `Job` or `Story` (call 13) |
| `Spot` | itself |

Your flat: `st.home`'s rig gives its place (`Floor { building, level }`) and its rect;
the tile is the walkable tile inside the rect nearest the flat's door.

A `Person` resolves further each tick:
- standing somewhere on a stratum: that tile, radius 1;
- inside building `b`: `Building(b)`;
- dead with a story body: the body's tile, radius 1; dead without one: `Gone("{name} is
  gone")`.

## 2. Legs: the walk across places (`track/route.rs`)

`route::leg(st, target) -> Option<Leg>`:

```rust
pub struct Leg {
    pub place: Place,
    /// The tiles after yours, in walking order, ending on `goal`.
    pub path: Vec<Pos>,
    pub goal: Pos,
    pub through: Through,
}
pub enum Through { Arrive, Door, Lift(i32), Stairs { up: bool }, Exit, Connector(StratumKind) }
```

From where you stand (`player.place`, `player.pos`):

- **Same place as the target**: A* to the target tile, or to the walkable tile beside it
  nearest you when the tile is not walkable. `Arrive`.
- **On a stratum, the target in building `b` or on its floor**: A* to `b`'s door on this
  stratum (`city.portals(b)`), or, for a walk-in kind with no door, to the walkable
  tile beside its footprint nearest you. `Door`.
- **On a floor of `b`, the target on another floor of `b`**: to the lift bank
  (`plan.lift`) when the lifts answer (`st.net.lifts_answer`), else to the stairwell
  (`plan.stair`). `Lift(level)` or `Stairs { up }`.
- **On a floor of `b`, the target outside `b`**: to this floor's exit onto the target's
  stratum (`floor.exits`, the door tile `at`), `Exit`; when this floor has none, to the
  lift or the stairs towards the floor that does (`level_for`).
- **On stratum `k`, the target on stratum `j ≠ k`** (or in a building reached from
  `j`): to the nearest connector on `k` whose `serves` holds the next stratum towards
  `j`. `Connector(next)`.
- **In a construct, or on the Net grid**: no leg; nothing draws; the pane says
  "jacked in".

A* is `citygen::pathing::astar` over the place's walkable tiles (`TileMap::walkable` of
its `MapRef`), unit cost, four ways. **On a stratum, door tiles are walls except the
leg's goal**, so a walk never runs through another building's door; **on a floor, its
exits' door tiles are walls except the goal**, and every other door (flats, rooms) is
walkable. People and foes are not obstacles.

**Between strata**, the connector is the one that leads on: with `j` the next stratum
towards the target, among the connectors on `k` serving `j`, the one with the least
walk to it on `k` plus walk from it on `j` to the target (or to the target's building's
door on `j`), by one BFS on each stratum; when the target is further than `j`, the
nearest one on `k` from which `j` has a connector onward.

The leg is cached in the track with the inputs it was made from (place, your tile, the
goal) and remade when any changes, at most once a tick.

Distance shown anywhere is the leg's length in tiles, as `m`. When more legs follow, the
pane adds what comes next: `door, then 6F`, `lift to 6F`, `stairs up`, `out to the
street`, `up to the Street`.

## 3. Arriving and clearing

| When | What happens | Log |
|---|---|---|
| `Spot`: same place, within `radius` (Chebyshev) | clear | `⌖ there · {label}` |
| `Building(b)`: inside `b` on any floor, or on or beside its door; a walk-in kind, in or beside its footprint | clear | `⌖ there · {label}` |
| `Person` on a stratum: same place, within 1 | clear | `⌖ there · {label}` |
| `Person` inside `b`: you enter `b` | clear | `⌖ {name} is in here` |
| `Job`: the job is not Taken any more (done, failed, late, dropped) | clear | `⌖ {label} · done with` |
| `Job`: objective `objective` is ticked | clear | `⌖ {label} · done with` |
| `Story`: the run is not Open, or its step is not `step` any more | clear | `⌖ {label} · done with` |
| `Story`: reaching its target | nothing: the step's trigger decides (call 5) | none |
| a person gone (dead, no body) | clear | `⌖ {label} · {name} is gone` |
| `t` on another thing | replace | `⌖ tracking {new}` |
| `t` on the tracked thing | stop | `⌖ stopped tracking {label}` |
| a new city, a load, a flatline | clear | none |
| no leg from here | keep; pane says `no way from here` | none |

Arrival is checked every tick, so walking into the radius clears the track at once,
before the next step's own effects (a story step's `Sees` may end the step on the same
tick: either clear is right, and only one log line is written).

## 4. The floor glow on the street (`track/overlay.rs`)

A `TrackOverlay` in the explore screen's overlay stack, drawn after the others. It only
calls `set_bg`, so every glyph stays.

- **The walk**: each tile of `leg.path` in view, at `g` tiles from the goal along the
  path (`g = n − 1 − i` for index `i`, `n = path.len()`):
  - base `f = 0.22 + 0.06·sin(2π·t/60)`;
  - **pulses**, Jeffrey's ruling after playing it (2026-10-31): "when you move the line
    trace get redrawn and act weirdly. It should be drawn independently", and the next
    pulse must not wait for the last to arrive — "a 3s timer for firing":
    - a pulse fires every 90 ticks (3 s), the first the moment the leg is drawn, from
      your end of the walk: it records the tick it fired, `t₀`, and the walk's length
      then, `L₀ = n`;
    - its head lies `h = L₀ − 0.9·(t − t₀)` tiles from the goal: it is pinned to the
      walk, not to you, so walking along the path or the path being remade does not
      move it;
    - it dies when `h < −12` (its tail past the goal); several can be in flight on a
      long walk;
    - the tail trails behind the head, away from the goal: with `d = g − h`, when
      `0 ≤ d < 12`, `f = max(f, 0.85·(1 − d/12))`; the brightest pulse wins;
    - pulses reset when the leg moves to another place (a door, a lift, a connector);
  - the cell's background becomes `mix(its background, CYAN (0,229,255), f)`.
- **The ripple at the goal** when `through` is `Arrive` (MAGENTA (255,0,200)), or a
  dimmer cyan ripple at a door, lift, stairs or connector goal:
  - every map cell within 3 tiles (`dx` and `dy·1.6`, Manhattan) of the goal;
  - `r = (t/14) mod 4`; `ring = max(0, 1 − |d − 1.4r| / 1.1)·(1 − r/4)`;
  - at the goal itself `core = 0.35 + 0.2·sin(π·t/15)`;
  - `f = max(0.55·ring, core)`; the background mixes towards the colour by `f`
    (cyan ripples at half strength).
- `t` is the frame tick (`st.tick`), as the other overlays animate.
- **The edge tag** (drawn after the overlay, since it writes glyphs): when the goal is
  outside the view, at the edge cell where the path
  last is in view, one row (or column) inside: ` ▴ Miracorp Tattoo · 61 m ` (▴ ▾ ◂ ▸
  by the edge), the arrow pulsing magenta (`0.65 + 0.35·sin(2π·t/30)`), the text
  `(205,212,222)` on `(4,6,10)`, clamped inside the view. For a leg through a door or a
  lift the tag names that: ` ▸ door · Velform Group Plaza · 12 m `.

These constants are the ones `round1.html` animates (concept C) and `map2.html` reuses,
except the pulses, which replace the mock's band `(0.9·t) mod (n + 24)` (it numbered
the walk from you and waited for each pulse to arrive). The deck map's walk uses the
same pulses, each pixel lit by the tiles it holds.

## 5. Where the track shows

- **The JOBS pane** (sidebar): when a track is set, its first line is
  ` ⌖ {label} · {m} m` (cyan `⌖`, dim text), with `· {next}` when more legs follow and
  `· no way from here` or `· jacked in` instead of the distance when so. It counts
  towards the pane's rows like the story's lines.
- **The deck's JOBS tab**: a job's or mission's detail gains a last line, `⌖ tracking ·
  t stops` (cyan) or `t track` (dim), and nothing for a job that is not taken. The tab's
  hint row gains `t track`.
- **The log**: the lines of §3.

## 6. Tracking from the JOBS tab

On the JOBS tab, `t` on a row:
- a taken job: `track::toggle(Source::Job { job, objective: first undone })`;
- an open story mission: `track::toggle(Source::Story { mission, step: run.at })`;
- an offered job: `take it first`; a done or failed one: nothing to track.

The deck keeps every other key as it is. `t` is not taken by the deck today.

## 7. The deck's MAP (`screens/atlas.rs`, `track/listings.rs`)

The reference is `map2.html`: frame 1 (the pane open, the cursor on a listing), frame 2
(the pane open, the cursor on open ground), frame 3 (the pane folded).

### The map

- **Which map**: your stratum; the Street while inside a building or jacked in (your `@`
  at that building's door, or at your body).
- **Pixels**: half blocks (`▀` with fg the top pixel, bg the bottom; a space when both
  are the same). One pixel covers `s × s` tiles, `s` the zoom.
- **A pixel's colour**, from the tiles it covers on that stratum:
  - its class: building when more than half are wall, mass, window, rooftop or door;
    else road when any is road; else walk when any is walkable; else water when at
    least half are water or sewage; else nothing (the deck's background);
  - its district: the commonest district among them; the district's theme
    (`style.theme_or_neutral`), neutral `(40,44,52)` without one;
  - building `theme.roof`, road `theme.road × 1.9`, walk `theme.sidewalk × 1.15`, water
    `(14,40,72)`;
  - how well you know it, the best of its tiles (`player.vis_at(stratum)`): never seen,
    the colour mixed from the background by 0.32 (0.22 for buildings), "the city grid";
    remembered, the colour; in sight now, the colour × 1.3.
- **Zoom**: `s` steps through 1, 2 and "fit" (the smallest `s` that fits the whole
  stratum in the map's cells). `+` (and `=`) zooms in, `-` out. The default is 2, or
  fit when fit is smaller. The title row says `zoom {s}`.
- **District names** at each district's centre in its accent × 0.55, on the nearest row
  (0, +1, −1, +2, −2, +3, −3) where the name covers no marker and no cursor corner; left
  out when none fits.
- **Markers**: each shown listing's glyph on the cell of its tile (a building's door,
  else its footprint's middle; a fixer's spot or their building's door), bold in its
  colour, on the cell's two pixels mixed and darkened to 0.45. Your `@` in white. When
  several share a cell: you, then the tracked one, then pinned (jobs and story, home),
  then the rest.
- **The tracked walk**: the floor glow of §4 on the map's pixels (each pixel the walk
  crosses, by its first index), and the ripple around the target pixel (`d` in pixels,
  rings `1.6r` apart, as `gen2.py`'s JS). Drawn when the map's stratum is where the walk
  is: from you, or from your building's way out when you are inside one.
- **The cursor**: four corners `┌ ┐ └ ┘` bold cyan on the diagonal cells around its
  cell, the cell's background mixed 25% with cyan. It starts on the tracked target when
  it is on this map, else on you. Arrows and `hjkl` move it a cell, `HJKL` (shift) 8
  cells. The view pans so the cursor stays 4 cells inside the map's edges, clamped to
  the stratum.

### Listings

`track/listings.rs` builds the listings of a stratum, each
`Listing { id, kind, name, what, tile, building: Option<BuildingId>, person: Option<PersonId> }`,
with `ListingId` one of `Building(b)`, `Person(p)`, `Job(JobId)`, `Story(mission)`. A
`◆` listing's `t` makes its job's or story's source (call 13); the others make
`Source::Listing`. When several listings stand on the cursor's cell, the one whose
marker shows there (the priority below) is the one under the cursor; `[ ]` still step
each.

| Kind | Glyph | Colour | From |
|---|---|---|---|
| jobs & story | `◆` | MAGENTA | taken jobs' and open missions' current targets (§1), when on this stratum |
| home | `⌂` | (124,255,154) | your home's building |
| ripperdocs | `+` | (255,79,94) | clinics (`clinic::is_clinic`), named by `clinic::name` |
| fixers | `$` | AMBER (255,200,80) | people whose profession is `fixer`, at their spot or building |
| bars & clubs | `Y` | (255,138,216) | `Bar`, `Club` |
| noodle bars | `≈` | (255,176,112) | `NoodleBar` |
| shops & markets | `▪` | (143,208,255) | `Shop`, `Market` |
| pawnshops | `¢` | (224,192,112) | `Pawn` |
| arcades | `▣` | (176,128,255) | `Arcade` |
| hotels | `H` | (192,200,216) | `Hotel` |

- Buildings count when their span holds the stratum; names as the street shows them
  (`net::building_name`, `clinic::name` for clinics). `what` is the kind and, for a
  clinic, open or closed and its hours (`clinic::is_open`, `clinic::hours_line`).
- **Distance**: the walk from you, by one BFS over the stratum from your tile (or your
  building's way out), to the listing's tile or the walkable tile beside it; listings
  no walk reaches show `—` and sort last. Ties by name.
- `ListingId` is stable for a city and a session: the building or person id, the job
  id, the mission's index.

### The pane

- Width 36 cells at 110 columns and wider, 30 below; a `│` in `(26,34,48)` on its left.
- **Header**: `PLACES ‹ {kind} ›` and `< >` faint on the right. Below, the kinds' glyphs
  in a row, lit when shown, the chosen one on `SEL (20,32,44)`, `all` last.
- **The list**: the listings of the chosen kind (and the pinned ones), nearest first:
  glyph, name (cut to fit), distance right-aligned (`61 m`, `—`). `[ ]` step the
  selection; the cursor jumps to the listing and the view pans to it. The list scrolls
  to keep the selection in view, with `▲ n more` and `▼ n more` rows.
- **The row under the cursor**: when the cursor's cell holds a listing's marker, or the
  cursor is on that listing's building, its row is selected: lit on `SEL`, and opened
  with two more lines, `what` and the track state (`⌖ tracking · t stops` cyan, or
  `t track` dim). Moving the cursor off it unselects (no row lit; `[ ]` start from the
  listing nearest the cursor).
- **The cursor on open ground** (frame 2): the pane's last three rows say `CURSOR ·
  {district} · {terrain}`, `walk {m} m · {never been | you've been here}` (or `no way
  there`), and `t tracks this spot` (`instead` when something is tracked).
- **Fold**: `f` folds the pane away; the map takes its cells. A strip of three columns
  on the right edge marks it (frame 3): `▕` in the border colour, then on `(10,16,24)`:
  `◂f` (cyan, bold) on the first row, `PLACES` down the strip zigzagging between its two
  columns, the chosen kind's glyph, and the count of listings. `f` unfolds.
- **The card**, the pane open or folded (Jeffrey after playing it, 2026-10-31: "when
  hovering over a POI in the map, I remember a popout being drawn"): the listing under
  the cursor gets a 30-wide box beside its marker, on the side with room, inside the
  map: glyph and name, `what`, `{district} · {m} m`, and the track state. Nothing
  animates under it. With the pane open its row still opens in the list too.

### Keys

| Key | Does |
|---|---|
| ← ↑ ↓ → / h j k l | move the cursor a cell |
| H J K L | move it 8 cells |
| `[` `]` | the previous / next listing |
| `<` `>` | the previous / next kind |
| `t` `T` space | track what is under the cursor (a listing, else the spot); on the tracked one, stop |
| `+` `=` / `-` | zoom in / out |
| `f` | fold / unfold the pane |

Everything else falls through to the deck (`q e`, digits, tab, esc). The hint row:
`←↑↓→ explore · [ ] places · < > kinds · t track · +− zoom · f fold · tab close`
(`f unfold` when folded).

**The spot under the cursor** is the walkable tile nearest the middle of the tiles the
cursor's cell covers, searched within 8 tiles; none: `nothing to walk to there`. Its
radius is `s` (the tiles a pixel covers), at least 1. Its label: `{district} · spot`.

The map's state (cursor, zoom, kind, fold, scroll) lives in `AppState.atlas`; the cursor
is reset when the map's stratum changes.

## 8. Dev console

`track`:
- `track`: the track, its target and its leg (`⌖ Miracorp Tattoo · Building(112) · Door ·
  61 m`), or `nothing tracked`;
- `track off`: stops it;
- `track <x> <y>`: a spot in your place, radius 1;
- `track <name>`: the listing whose name starts with it (`several: …` when more match).

## 9. Shots (`screens/shots.rs`)

At 120×40 and 100×30, from `state_home()` with the story begun and the walk from home
to Marlo's body remembered, as the mock's dump:
- `atlas_listing`: frame 1 (kind all, the cursor on the tracked clinic);
- `atlas_ground`: frame 2 (kind ripperdocs, the cursor on a Crown Circle plaza);
- `atlas_folded`: frame 3;
- `track_street_near` and `track_street_far`: the street with the walk to Marlo's body
  in view, and running off the map with its edge tag (as round 1's frames);
- `track_jobs_tab`: the JOBS tab with the story tracked.

## 10. Testing

Unit tests beside each module. They must cover:
- each source's resolution, including a person moving, inside a building, dead with and
  without a body;
- each row of §3: arrival for each target shape, each way a job ends, a story step
  moving on, replace, stop, the refusals, and a new city clearing the track;
- legs: same place; street to a door; a walk-in kind; floor to lift; lifts locked to
  stairs; floor to exit; stratum to connector; a construct; door tiles not walked
  through;
- the overlay only changes backgrounds (every glyph and foreground of the buffer is the
  same with and without it);
- MAP: pixel classes and colours; zoom levels and fit; the cursor's moves, panning and
  clamping; `[ ]` order and the jump; `< >` and the pinned kinds; the row under the
  cursor; the spot under the cursor and its refusal; fold and the strip; the card's
  side; listings and their distances, `—` sorting last;
- every new glyph has a glyph in the window (`every_drawn_character_has_a_glyph` sees
  the new shots).

Checks as CLAUDE.md: fmt, clippy and tests with and without the window feature, into
`target/smoke`.

## 11. Code layout

- `crates/strata-zero/src/track/mod.rs`: `Track`, `Source`, `Target`, `set`, `stop`,
  `toggle`, `resolve`, `tick`, arrival and clearing.
- `track/route.rs`: `Leg`, `Through`, `leg`, the A* with the door rule.
- `track/overlay.rs`: `TrackOverlay` (the street glow and ripple) and the edge tag; the
  glow's maths as plain functions the MAP reuses on pixels.
- `track/listings.rs`: kinds, `Listing`, `listings(st, stratum)`, distances.
- `screens/atlas.rs`: the MAP tab: pixels, markers, cursor, pane, strip, card, keys.
  `Deck` hands MAP its keys first, as it does GEAR & INVENTORY, and draws it in
  `draw_map`'s place. `MemoryMap` stays for whoever else uses it.
- `AppState`: `track: Option<Track>`, `atlas: AtlasState`.
- The sidebar's JOBS pane, the JOBS tab's rows and detail, the explore screen's overlay
  stack and update, the console, the shots.

## 12. What goes

- The MAP tab's old drawing (`Deck::draw_map` and its "what you remember" title) is
  replaced by `atlas`.
- Nothing else is removed.
