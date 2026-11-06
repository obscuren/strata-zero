# Streamed world build report (#27 and the first play's fixes)

This report records the calls made while building the streamed world (each with what it
costs if it was the wrong call) and the review findings left for later. It is kept from
the build ledger, which is scratch.

- Spec: `docs/superpowers/specs/2026-11-05-streamed-world-design.md`
- Plan: `docs/superpowers/plans/2026-11-05-streamed-world.md`
- Range: c4f1e12..8cce355 (54 commits). Not merged, not pushed. Main has moved on since
  this branch was cut, so rebase onto it before merging.

## What was built

Tasks 1 to 9 were the plan:
- the stream and the ground's light;
- `Beyond`;
- the city screen past its edge;
- the ground on the city screen;
- off-the-map sweeps;
- background builds;
- the handover;
- the Net overworld walked tile by tile (Closes #27);
- shots.

Tasks 10 to 17 came from the first play:
- trunks joining the cities' lanes;
- the Net self's handover;
- one light;
- neighbouring cities alive in view;
- one set of rules and one name;
- the overworld in motion (round 2 of the motion mock);
- solid signs, round night sight and far lights;
- commuters for cities without homes.

Every task had its own review and a fix round where one was needed. After that came a
whole-branch review by three reviewers, one per slice (the stream core, the screens and
the sim with citygen). Their findings went into one fix pass of three waves (21 commits),
and that fix pass was re-reviewed clean.

Checks on 8cce355:
- fmt and both clippys are clean.
- Tests pass, plain and with the window feature (strata-zero 2151 / 2215).
- The window build checks for Windows and Linux.

## Worth a look when playing

- **Collision with terminals.** The "no collision on terminal objects" report was never
  reproduced as an area bug. The likely cause was the walkable `▣` caps of billboards
  and road signs, and signs are now solid. The matrix test (`screens/everywhere.rs`)
  covers 26 situations, including towns, home set aside, kept cities and the Net after a
  handover. Recheck it on a fresh build.
- **Night sight** is a circle counted in cells, as day sight has always been. Cells are
  about twice as tall as wide, so on screen it is a tall oval.
- **Cities by day on the overworld** can look dim under cloud. Round 2's cloud shade
  falls on city tiles too; about a third of a city sits under cloud at a time.
- **A new game's first night** shows about 35 people on the street where it showed about
  226 before. The 226 were people with no home standing at their work's door all night.
  Those people now commute. Nightmouth, with no bar, empties after the evening commute.

## Calls made

### Before the tasks
- **Ownership.** Each overworld tile has one owner (nearest own tiles, city before town),
  and the handover happens at that line. Cost if wrong: a different split line.
- **"Not in a fight" still refuses leaving,** and foes don't follow you off the map.
  Cost if wrong: fights end at the edge instead.
- **Bots come with you** on the ground and across a handover. Items can't be dropped off
  the live city's cells. Cost if wrong: a world layer of changes later.
- **People in new cities** are seeded on the build thread. Home's catch-up stays on the
  frame until #26. Cost if wrong: a hitch coming home after long trips.
- **Superseded:** "Kept cities for the session, not saved". See the final fix pass.

### Task 1: the stream and the ground's light
- The street layer bakes steady light (windows) as well as lamps, and emits by the city's
  rule. A layer with lamps only failed the seam test. Cost if wrong: none visible, one
  more layer.
- The lamp test takes only cells with no district within 8 and at least 10 cells from the
  map edge, since the stream can't know a city's own lamps. Cost if wrong: a weaker test.
- Where a city's open ground differs from the world's, the implementer reports it and
  generation doesn't change. Cost if wrong: a seam left visible.
- The town-in-block test runs on seed 4, because seed 3 has none. Cost if wrong: none.

### Task 3: the city screen past its edge
- The world exists before the city screen's first frame (a new game, or an old save
  without one), so the street past the edge draws from the start. The load and broadcast
  screens hide the hitch. Cost if wrong: a longer new-game load.
- Rain falls past the edge, which serves the no-seam goal. Cost if wrong: none.
- `At::Ground` is no longer written on leaving and is converted on restore. Cost if
  wrong: none.

### Task 4: the ground on the city screen
- A ground jack-in shows the cord and the port mark. Cost if wrong: one more fix.
- Off the live cells, space with no port in reach does what space does in the city.
  Cost if wrong: a line differs.
- The top band off the live cells names the place, as HERE does. Cost if wrong: a label.
- "not with a gun on you" holds at a ground port, the same rule as city ports. Cost if
  wrong: none.
- Resuming any run outside the lot makes its world on load, dev starts included. Cost if
  wrong: slower dev starts.
- Off the live cells, a blocked step says the ground's lines. Existing refusal lines keep
  their words. Cost if wrong: lines.
- `still_stands` un-jacks through `net::jack_out`'s pieces (run, mask, route).
- A landing (↵ from the overworld) on a cell a settlement owns goes into that settlement.

### Task 5: off-the-map sweeps
- Tracking from off the live map refuses ("no way there from here"), and there are no
  floor pulses on the ground. Cost if wrong: tracking across the world later.
- A grid tracer stops at the live map's edge, because the Net's grid is the live city's.
  Cost if wrong: none.
- The overworld panel's dim lines take `chrome::DIM`. Cost if wrong: a colour.

### Task 6: background builds
- A people-world streamed in and older than a day is dropped and seeded on the spot, so
  walking in doesn't stutter. Cost if wrong: fresh people instead of run-on ones after a
  day.
- **Superseded** in the final fix pass (a build panic now warns): "a panicking build
  re-panics on the main thread when collected".

### Task 7: the handover
- Entering from the overworld goes by owner, not by block. Cost if wrong: entering the
  wrong one of two overlapping settlements.
- Each city's street memory is written into `Outside.seen` on leaving, so saves grow a
  few KB per city. Cost if wrong: save size.
- Every settlement you enter keeps its whole `Stay` for the session, so crossings along a
  town line don't drop items or foes. Cost if wrong: memory for a few cities' state.
- Three review minors were re-graded Important and fixed:
  - space beside a foreign node hid the live popup;
  - live nodes weren't filtered by owner;
  - the once-a-frame handover skipped `held_in`.
- Jacked in, space always opens the live city's window (Jack out is always offered). The
  foreign-node line comes from the look cursor and the program keys. Cost if wrong: one
  more key press.
- A kept city runs its people on to now when you come back. Cost if wrong: a catch-up
  hitch after a long trip.

### Task 8: the Net overworld walked
- `net_world_step` takes the screen's tile. Cost if wrong: none.
- Walking or dropping on the Net overworld is refused in a fight. Cost if wrong: one more
  refusal.
- A landing never puts you on a NetFloor or NetGate. Cost if wrong: none.
- The cord is walked up to 64 cells apart and runs straight beyond that. Cost if wrong: a
  straight cord far out.

### Task 9: shots
- Task 10 was added: trunks join a city's lanes in citygen's Net stage. This is a
  generation change, for cities' Nets only. Cost if wrong: city Nets differ from before.
- The road colour change at a city's outline (where the district theme starts) is the
  city's look, not a seam. Cost if wrong: a look tweak.

### Task 10: trunks join the lanes
- HERE names the owning settlement on the open Net inside its block, and trunk names
  appear only off every block. Cost if wrong: a label.

### Task 11: the Net self's handover
- The live city follows the Net self, and the neighbours in view run. This replaces the
  spec's first calls 1, 2 and 8. Cost if wrong: CPU for the neighbours' sims.
- A Net crossing is refused while a program run is in flight ("finish the run first").
  Cost if wrong: one more refusal line.
- A body at the home rig is placed at the rig's landing in the new frame, while home
  keeps the real body. Cost if wrong: a one-frame street position at jack-out.

### Task 14: one set of rules, one name
- Refusal lines go by what's on the tile, everywhere. Cost if wrong: one line to flip.
- A sited city's name is the world's name. Cost if wrong: a generated name changes.
- `W` back down from the Net overworld is not a drop. Cost if wrong: none.
- Towns are "a town off {city}" everywhere. Cost if wrong: towns get proper names later.

### Task 12: neighbours alive
- The live city's people and lit doors draw only on the cells it owns. Cost if wrong: a
  person at the line hidden.
- Open-Net packets run relay to relay (lane ends, junctions, every 24 columns and 12
  rows) and end in a lane glow. Cost if wrong: a look tweak.
- Task 17 was added for cities without homes. Cost if wrong: another way to populate
  such cities.

### Task 15: the overworld in motion
- The breath keeps round 2's rates: a step every 2 to 3.6 s. The plan's "no step change
  more than once in 300 ticks" contradicted those rates. Cost if wrong: slower periods,
  one knob.
- Gusts run per mountain range, starting from the range's west end, on one world
  schedule, 40 to 70 s apart. Swells roll toward the nearest land. Cost if wrong: a look
  tweak.
- Place rings skip road tiles. Only a checkpoint's own road glow stays. Cost if wrong: a
  ring tweak.
- Route shields are not built. Cost if wrong: a small task of its own.
- Clouds keep round 2's depth on city tiles. Cost if wrong: less shade on cities, one
  knob.
- The night spill tints the background of open roads within 3 tiles of a city, as in
  round 2. Cost if wrong: skip roads in the spill.
- Windows, night signs, the spill and the beacons' after-dark modes come on with the
  city's lamps (18:00 to 05:59). The hour only sets strength. Cost if wrong: none (one
  rule with the city).
- Sea glints follow the hour (sun and moon, not lamps). Cost if wrong: one word.
- Task 15 ran alongside Task 16's review, since they share no files. Cost if wrong: a
  merge.

### Task 16: signs, sight, lights
- The night's dark radius is a circle in cells, as day sight and the field of view are.
  Cost if wrong: a screen-round shape changes the metric for day, night and NPC sight.

### Task 17: every city lives
- A spot worker with no home keeps to their spot off shift. No cast role or fixer ever
  goes off the map.
- Commuters get a lunch trip (a third of them, 12:00 to 13:30) and an after-work drink (a
  third of them, leaving between 21:00 and 01:00). Both are picked by a hash of the
  person's id, so the residents' days are unchanged.
- A neighbour within `CATCH_UP` of the clock runs every frame. Only large catch-ups and
  reseeds take turns, one a frame.
- Cost if wrong: the shares and times are knobs. Giving residents lunch would mean
  re-pinning their days.

### Final fix pass
- **Kept cities are saved.** Each one is stored as its changes against the world's
  rebuild. A save with home and three kept cities is 1.01× a home-only save; saving them
  whole would be 1.64×. They are rebuilt on reopen, about 120 ms each, side by side. Cost
  if wrong: reopen time grows with the cities visited.
- **A build that panics** is marked failed, refused and warned. It re-panics only in
  tests. Cost if wrong: a missing city instead of a crash.
- **A chase holds you at a town line.** `held_in` uses `runs::when::fighting`. Cost if
  wrong: none.
- **Commuters' ways:** a person commutes only by a way whose sim walk reaches both their
  work's doors. With no such way, they stay in town. Anyone off the map with no road out
  comes back the next minute. Cost if wrong: fewer commuters in some outskirts cities.
- **Deliveries to commuters:** a Deliver needs max(60 min, the walk at sim pace) of the
  recipient's shift inside the window. Cost if wrong: one constant.
- **Dressing's verify** takes a sign down only when nothing else seals the pocket and
  only if that opens ground the street reaches. This moved 7 cells in 5 of 1,100
  settlements; the other option moved 450,393. Cost if wrong: a pocket sealed by signs
  alone stays sealed beside unreachable ground.
- **POI signs** are decided once per world and cached per POI. Cost if wrong: none (the
  same signs; the cost fell).
- The fix pass ran as three waves in temporary worktrees, rebased onto the branch. Cost
  if wrong: a merge.
- Two behaviours the review set aside stand:
  - The deck MAP's zooms 1 to 3 don't draw streamed ground; the spec asks only the
    minimap to cross edges.
  - The live city's people standing on cells it doesn't own stay unseen (Task 12's
    rule).

  Cost if wrong: a zoom draws more; a person at the line is hidden.

## Left for later

### Look and the screens
- Look reads a neighbouring city's walker as the ground under them.
- The dev inspector (F5) and free-camera location read the live map on other areas'
  cells.
- `draw_beyond` passes over the whole view per held city. `grid::draw` allocates per
  area per frame, and `Beyond::of` allocates several times a frame. All within budget.
- Wall, bridge and rail glyphs don't join across an ownership line.
- The minimap draws the open ground in one grey, without picking out roads.
- The cord is straight past 64 cells and doesn't cross the void inside a city.
- A gate one step across the line says it is too far.
- Dev light view: people and items on other areas' cells grey only on the live city.
  `relight_city` allocates per held city per tick.
- `everywhere.rs` demands rows only on places that produced any.
- `LiveAtlas::cell` takes the world separately from the one it was built from.
  `Neon::apply` takes 9 arguments. Every CorpCore district breathes in step (as in the
  mock).

### The stream and saves
- **A generation-changing update drops a kept city** that builds differently, with one
  line: "lost in the update · what you left in {name}". Its keycards, jobs and journal go
  with it, and a Deliver's sealed package stays in the pack. A run saved inside a
  non-home city records its map from the save, so after such an update the loss lands
  one reopen later.
- The kept-city fingerprint hashes `Feature`, `Hue` and `StratumKind` through
  `derive(Hash)`. Those enums must only grow at the end.
- A reopen whose world can't be made leaves a ground save off the map.
- `built`, `kept`, `builds.people` and `Nets.cities` are never pruned. They are bounded
  by the world.
- `remake` spawns one thread per kept city, uncapped (about 13 at most).
- Concurrent background builds are uncapped (typically 1 to 3). The error and
  spawn-failure paths are untested. A failed build is retried once when you walk in.
- Without a panic hook (the window build outside Windows), a build panic prints twice.
- Home's backdoor trace doesn't advance while you're in another city.
- Flat bots show at the landing while away.
- The long walk could hand over if its wander reaches a settlement's cell.
- The handover tests check the camera shift only (cursor, aim and path are not checked).
- Runs saved before c24004a keep their old city names until rebuilt.
- Old saves keep pre-change billboards, which are now solid. A pocket could seal with
  the player inside.

### The sim
- Residents going out of town for something the city lacks pick from all ways. One
  whose way doesn't walk home could strand (none seen).
- `track::follow` keeps the edge as last seen for someone who walks out while tracked.
- A lunch break with nowhere to go stays inside as Visiting.
- Couriers with no home can also stay for a drink.
- The dev clock-set command reseeds the world and drops the in-town mark until a reopen.
- A freshly seeded city sits below its steady street count until its minutes run.
- No fallback anchor when a reopened map has no ways in.
- The residents' day pin hashes Debug output.

### Citygen
- Two POIs' signs together could seal a verge pocket (none seen in 30 worlds).
- NPCs walk through Pipe posts. This predates the branch.
- Verify still strips non-sign blockers beside ground the street never reaches.
- In the trunk joins:
  - a join may run beside a laneless gate, or double a lane at the seam;
  - the case with no lane in reach is untested.

### Code tidiness
- `net_light` repeats citygen's match, including a literal 96.
- `GroundMap::light` takes any layer.
- `owner()` returns None before the owners are built.
- `GroundMap::size()` is 0×0 before the first cover.
- `Beyond.stream` duplicates `outside.stream`.
- `can_cross`'s lot branch is dead.
- `Beyond::of` gating is written three ways.
- `live_owns` repeats `owner_of`'s arithmetic.
- The jack-connector search is written three times.
- The call set-up is written three times.
- `walk_in` and `due` could live in `commute.rs`.
- Duplicated commute test helpers.
- Weak or hand-kept tests:
  - the void test's NetWall check;
  - `here_off_the_map` off a route;
  - the no-port space test;
  - the long walk's key list;
  - the name test's guard;
  - `a_run_saved_after_a_drop`'s Net check.
- `streamed_draw_cost` times drawing only.
