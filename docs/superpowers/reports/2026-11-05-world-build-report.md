# World generation build report (#15–#25)

The calls made while building the world issues (each with what it costs if it was the wrong call), and the review findings left for later. Kept from the build ledgers, which are scratch.

## #16 Open ground one level down

**Calls made**

- a build covers its rectangle plus an 8-tile margin, then crops — makes 'a tile alone equals the region' true by construction for every later rule whose reach is under 8 — cost if wrong: a rule reaching farther would need a wider margin
- the whole-world water test runs seeds 0..3, not 0..6 — a whole world's detail in a debug build takes about a second a seed — cost if wrong: fewer seeds checked (the sweep can widen it)
- the shore test counts a ford as water — a ford is a shallow bar across the river, and the river runs deep on either side of it — cost if wrong: none
- the river-continuity test checks water on both sides of each shared edge (rather than "near b's patch") and runs seeds 0..3 — a sharper check, and whole-world builds are slow in debug — cost if wrong: fewer seeds
- found in Task 6 — three tile tests pinned the old counts (28 terrains, 21 features) and failed from Task 1 on; Task 1's run filtered them out and Tasks 1-4's ledger lines quote the doc-test line ("0 passed"). Counts moved to 42/24, Railing's index check kept as ALL_TERRAIN[27]; task-done now runs --lib — cost if wrong: none, the workspace suite is green
- the line between pylons is a new appended Feature::PowerLine(bool) (┄ east-west, ┆ north-south, the mock's dark grey), not Feature::Cable — Cable has one glyph (╌) that reads wrong on a north-south run — cost if wrong: one more feature variant
- the power line keeps 3 rows off an east-west road and 6 columns off a north-south one, mitred at corners, pylons at corners and every other tile — "6 cells" read as the same distance on screen, cells being twice as tall — cost if wrong: a constant
- added power_lines_run_beside_some_highways and nothing_stands_on_water_or_the_road — the brief named no test for the dressing itself — cost if wrong: none
- the first look at the sheet showed rivers and washes as boxes (a square brush on Task 4's L-shaped line); reworked in 16e9039 as a cubic through the crossings, leaving each edge square (tiles still join), a world-wide meander and ragged banks, the channel a disc in cells as round 4 drew it (an east-west river is as many rows deep as its width); test river_banks_curve_rather_than_step pins it — cost if wrong: river geometry differs from Task 4's brief, fingerprints re-pinned
- ground_follows_its_overworld_tile skips tiles beside a river or wash — a 7-wide banked river can't stay inside an 8-cell tile, so its banks reach the next tile — cost if wrong: the cover rule is checked on fewer tiles
- the long straight reach on seed 0 row 19 came from nine offset-3 crossings in a row by chance (checked the edge hash by hand); the noise field sits at 0.5 ± 0.09, so a `wave` helper stretches it to -1..1 for the meander and banks — cost if wrong: none
- `Closes #15` rides on this commit — the generator's slice 1 was finished and tuned before this plan, and none of its commits closed it (Jeff asked for closing keywords mid-run) — cost if wrong: none
- the sea and lake blend is read through a warp (±5 columns, ±3.5 rows) on top of the stretched shore noise — the blend's slope (an eighth a cell) makes noise alone move a coast about one cell; more noise would leave ponds inland — cost if wrong: coasts move up to half a tile from the overworld's line
- the cover jitter is ±4 columns and ±2 rows (the plan said up to ±3 and ±1.5) — at ±3 the seams still sat within a cell of the tile lines two times in three — cost if wrong: a constant
- ground_follows_its_overworld_tile checks that at least 24 of a tile's 64 cells are its own ground (Dirt only for flats, hills and mountain passes; reviewer minor 4) and skips tiles beside water as well as rivers and washes — with the jitter a single middle cell is no fair probe, and a whole tile can lean a few cells — cost if wrong: a looser check on the cover rule
- a_straight_highway_is_straight_one_level_down takes Bridge lanes as well as Road — a lake shore can now reach a highway, which bridges it and stays straight — cost if wrong: none
- fords are one per third of the narrow (width-3) river tiles, at a point the tile's hash picks, the whole cross-section there (within the channel's radius + 1.5) — per-cell rolls on a whole-width ford would make every narrow tile fordable — cost if wrong: fewer fords upstream
- declined-to-judge lines — water depth in play, the minimap's classes for Shallows/Rock/Cliff, look names for Pylon/PowerLine, is_outdoor_ground on open terrain: all #22's renderer and panel, weighed there; litter density, the Void variant hash, exact river widths stand — cost if wrong: a look detail
- this workspace stays until the run's one final report collects its rulings (the run spans several plans) — cost if wrong: none, it is git-ignored

**Left for later**

- relief ignores elevation (rises only from noise)
- a tributary ends 7 wide into a narrower main river (width by position along its own course)
- the tile-alone test covers 24 tiles of seed 0 only (the reviewer's exhaustive probe found 0 mismatches)
- the detail sheet passes wall_mask 0, so an east-west bridge draws as ╏ (#22's ground view computes the mask)
- grates line a road wherever a wash runs beside it
- a power line can sit on a neighbouring road's shoulder
- detail::roll duplicates field::roll
- docs/development.md's world commands build into the default target/release
- the spec's ground details not built: crops in rows with a track, scrub in patches and folds, the spill's fence or warning sign
- style: a `use` mid test module in detail.rs; the new tile.rs test above `use super::*;`

## #22 The overworld screen (and #15)

**Calls made**

- the outside is a screen switched to (not pushed over the map) and back through MapScreen::resumed — the map's draw is heavy and a pushed screen keeps the ones below drawing — cost if wrong: a little state the map screen keeps (look mode) resets on return
- the world's generation screen stages on NEW GAME (spec: left to the builder) are not built — the world builds on first need in about 50 ms (release) — cost if wrong: a short stall on the first W
- the block is centred on the home city's own tiles' bounding box, not WorldCity.centre — the large map then lines up with what the atlas draws — cost if wrong: the block shifts a few tiles
- city_cells_and_tiles_map_both_ways checks street_of lands on a walkable outdoor street everywhere, and lands near only at the block's middle — at a corner the nearest street can be across the bay — cost if wrong: none
- `down` returns Result<Moved, _> (not Result<Pos, _>) — ↵ on a block tile goes into the city, which needs Moved::City — cost if wrong: none
- a step enters the city only when it crosses into the block from outside; inside the block the overworld moves you, ↵ goes down into the city there — else going up from the city's middle leaves every step re-entering it — cost if wrong: one rule in world_step
- the closed-tile test looks for any closed neighbour (sea, lake, peak) — seed 8's world is landlocked (100% land), a valid generator outcome — cost if wrong: none
- walking into the block from the open ground checks only that you land on a walkable outdoor street — until #19 the city's edge is not the world's (seed 8's city has a bay where its world has land), so the nearest street can be 35 cells in — cost if wrong: a jump on entering until #19
- the region re-centres within KEEP columns of its side edges and KEEP/2 rows of its top and bottom (rows are twice as tall) — cost if wrong: a constant
- the screen and its tests were written in one go and run once (7 passing) — it is wiring of existing pieces (layout, hud rows, atlas_cell, tileset::cell, the log pane); the tests drive real keys through it — cost if wrong: a test that never failed (they check the at-state each key leaves, which the stub could not)
- help gains draw_keys(f, title, rows) and the map's slots come from a free ability_slots(st), both shared with the map; sidebar::draw_log and explore::dir_for are pub(crate) — cost if wrong: none
- the overworld camera is held inside the world (centred where the world is smaller than the view); the ground's isn't — the world's edge out there is the world ending — cost if wrong: a look detail
- on the open ground at night sight is RADIUS_UNDER + 2 (no lamps out there), by day RADIUS_STREET — cost if wrong: a constant
- the street-edge test searches all four sides of the city for a street cell with walkable ground past it — seed 8's city has bay and buildings along its west column — cost if wrong: none
- leaving is refused on the lot, jacked in, in a fight (net.engaged) and off the open street, with a log line each; a step off the edge onto water says "no way out there" — cost if wrong: a refusal wording
- the jacked-in W test was left out — W from the Net and the Net overworld are #23 — cost if wrong: none
- by day the open ground is seen out to RADIUS_OPEN (40) — the street's 14 left a small blob in a dark field with nothing built to block sight — cost if wrong: a constant
- the open ground's light never drops below MOONLIGHT (110) — the street's night ambient (60) assumes lamps, and there are none out here — cost if wrong: a constant
- a bridge's deck and rails choose their glyph by where they run on (east-west only with both east and west neighbours, or one with none north or south) — a deck two wide has its twin beside it, which drew a north-south bridge sideways; also changes a city walkway corner's glyph — cost if wrong: a glyph
- the look fixes (moonlight, sight by day, here in NEAR, the averaged world pane) were made with the shots, tests written with them (a_wide_bridge_runs_the_way_it_goes and by_day_the_open_ground_is_seen_far ran red first; the_city_you_stand_in_is_here did not) — cost if wrong: one test that never failed
- `Closes #22` waits for the fix pass after the fresh review, not this commit — cost if wrong: none
- the shots commit as PNGs only (the SVGs duplicate them); the 900 KB detail sheets stay untracked — cost if wrong: none
- declined-to-judge lines — the city not drawn from the ground and the entry jump (#19), jacked-in W (#23), MAP world zoom (#24), world stages on NEW GAME (18.5 ms: moot), look-mode lost on a switch, a drone left in the city on W (stays; #19/#21 decide), NEW GAME at Large never played through the story (the design call stands; the story tests run on Medium seed 8) — cost if wrong: a drone that should follow you
- ranges grow by the scale's three quarters (2-3 a world) — in proportion gave 3-4 crossing in lattices — cost if wrong: a constant. Ruling: the detail tests run on the small world (160×52) — the rules don't depend on size, and the default world made the suite four times slower — cost if wrong: bigger worlds' detail only checked by the pinned fingerprints and the sweep. Ruling: down searches the tile's ring for dry ground when the tile itself is all water — cost if wrong: you land a few cells into the next tile.
- a run saves its world's size in tiles (WorldSize::Custom) and the seen mask is keyed by fixed 8×8 chunks — Jeff may retune the world's size or the cells per tile later; neither then moves old runs' saves — cost if wrong: none
- the outside's 's' checks save::refusal as the map's does (found in the fix pass) — cost if wrong: none

**Left for later**

- NEW GAME at Large: saves 1.38 MB / ~27 ms (×1.55 of Medium); saving.md item 12 and §3 after_restore list need updating
- load robustness — rebuild() swallows a generation error then world() panics; `at` unchecked against a regenerated world (can come back in water): fall back to At::City with a log line
- the overworld's ground costs (hills ×2, mountains ×4, wading) disagree with the ground (every cell WALK_TICKS, rivers a barrier)
- world pane has no view frame; NEAR lists 4 not 5; no 100×30 ground shots
- sight updates only on a step (standing from day into night keeps day sight)
- tests — a_closed_step_says_why has an always-true assert; the overworld draw test accepts any name; clamped-block entering untested; down's refusal log untested
- W on the overworld and enter on the ground do nothing silently; space could go down

## #17 POIs one level down

**Calls made**

- POI buildings are single rooms laid in the ground (walls, windows, door, floor, furniture), walked into on the ground itself — the spec's "entered through the city's interior generator" needs the open ground to hold city buildings, which comes with cities in the world (#19/#21) — cost if wrong: no upper floors or generated interiors in POIs yet
- a road POI faces its own tile's road, else the nearest neighbour's (a stop sits beside the highway, its tile often roadless) — cost if wrong: none
- a truck stop whose store would meet water or a road tries the other side, then slides up to 32 cells along the road; a stop with nowhere dry is left out — cost if wrong: a rare missing stop
- the north-south frame is tested on its geometry (Frame::beside_road) — no small world of seeds 0..20 put a stop on a north-south road — cost if wrong: the rotated layout's look only checked on shots
- ground_follows_its_overworld_tile skips tiles within 5×3 tiles of a POI, and nothing_stands_on_water_or_the_road lets a checkpoint's gate stand on its road — cost if wrong: none
- an off-road POI whose tile carries a road centres beside it (6 rows off an east-west road, 12 columns off a north-south one) — a wreck on a highway tile had its hull refused — cost if wrong: a POI a little off its tile's middle
- a relay's track runs only to a road it reaches without crossing a river or lake tile; the test asks for a track on at least 60% of relays by a road (seeds 0..30) — a river between mast and road is the world's, not a fault — cost if wrong: a mast with no track
- nothing POI-made stands on a cliff (open() refuses Cliff) — cost if wrong: none
- the POIs use only existing terrains and features (rigs as Couch furniture, tents as Stall, panels as AcUnit, fires as Lamp) — new glyphs would need fonts and a save-index append — cost if wrong: a look detail
- the reviewer found north-south stops in seeds 12, 14, 16, 20 (the Task 1 ruling said none in 0..20) — my check read the stop's own tile, which never carries a road; the frame test stands and a real-world test now covers them — cost if wrong: none
- declined lines — single rooms for the interior generator (product call, #19/#21), camp fires as lamps (look), the round 2 mock fidelity (Jeff to judge on shots), other platforms (shared code only) — cost if wrong: a look detail

**Left for later**

- litter treats POI gravel as a shoulder (barriers in solar pens, lots)
- the east-west checkpoint gate stands a column off its fence; on corner tiles on dirt
- room() and block() don't refuse cliffs (a water plant's tanks on cliffs)
- late dressing (trash rolls) can replace pumps, lamps, vehicles
- an arcology meeting water or a road has no fallback (rubble only)
- look — the sign below its posts on south and west sides; the canopy one row between pumps and store; plaza ▫ reads like pumps
- no rig shot (seed 8's world has none); no pinned fingerprint over a POI tile

## #18 The Net in the open

**Calls made**

- the Net is derived from the ground region built with a 48-cell margin (constructs from POI rooms, gates at doors, a node inside the door); spurs walk straight out of the gate to the first lane within 40, else an L-path to the nearest road tile's centreline (world data) — every decision local or from the world, so tiles alone equal regions — cost if wrong: a tile's Net costs a ground build of (8+96)² cells
- every POI room is a construct (the spec: buildings "that have systems"); lone nodes at cameras, a sign run's first letter (ad board) and a mast's pylon pair (backbone) — cost if wrong: a shed with a construct
- out here you jack in beside a cabinet (Terminal) or a pump, refused only while the deck cools — the city's jack-in has no deck rule either (any runner jacks in at a port); no traces or ICE out here yet — cost if wrong: a non-runner can look at the open Net
- Outside.net is skipped by the save (not a cache: a run reopened out here wakes on the body, as the plan says) — cost if wrong: a jacked-in save comes back jacked out
- the reach test sets the Net position at the edge of reach and steps out — walking east from a pump hits walls first — cost if wrong: none
- every_kinds_gates_are_wired asks each gate within a tile of its POI to reach a trunk cell over lanes, in a region ±13 tiles (the furthest trunk a spur makes for is 10 tiles) — the first form (60 lane cells inside ±8×6 tiles) failed a neighbouring POI's spur that runs off the test region — cost if wrong: a gate wired to a stub elsewhere would pass if the stub touched a trunk
- the_grid_fades_from_signal measures over five tiles of straight road, not one — the lattice keeps 1 cell in 8, so one tile gave 1 dot near and 0 far — cost if wrong: none

**Left for later**

- lone nodes overfire (every camera gets a light line; the sign's '!' warnings no longer do)
- a POI tile's Net costs about 4 ms (the ground with a 48-cell margin)
- Outside.net is #[serde(skip)] though not a cache (saving.md rule 4); a reopened run wakes on the body, as the plan says
- ports at the store and the booths are only the pumps and cabinets; no port at a store's counter
- Net::lane refuses to cross a construct, so a trunk through a construct breaks (none seen; constructs stand off the road)
- the relay's cabinet has no test of its own

## #19 Cities grown into the world

**Calls made**

- the city's block is its own tiles' bounding box plus 2 tiles, clamped to the world and to the generator's 1024×512 cells (Overworld::city_block) — cost if wrong: none
- the outline is density ≥ 0.3 (blended overworld density for the city ± 0.12 noise); highways are painted as roads where density ≥ 0.15 so they meet the city — cost if wrong: two constants
- POIs inside a city's block are laid by the ground and survive only where the city leaves cells empty — the spec says a POI in a block is the city's, laid in its outskirts — cost if wrong: a POI cut by the city's edge
- a sited city's open ground is filled in before the linking stage (and again at the end for slivers it voids) — 3 of 8 worlds' home cities failed as Unconnected with a lobe of outskirts ~190 cells off over unbuilt ground; reached on foot in play — cost if wrong: later stages see ground terrain where they saw Void (dressing only dresses city terrain)
- the outline is the city's own overworld tiles (ownership blended between tile centres, ±0.22 wobble at 24×12 cells and ±0.08 at 7×3.5, at 0.5), not density ≥ 0.3 — the density outline built only the densest half (40–80 buildings vs ~400 in today's large city); now 160–200 — cost if wrong: the outline constant and wobble
- a sited city's reach counts only the city's own cells (open-ground terrains are walked over, not counted) in both the linking check and validate — with the ground filled first, ground cut off by water failed 17 of 100 cities — cost if wrong: an unreachable patch of open ground inside a block (reached round the outside in play)
- a sited city's street repair may carve a bridge over water and reaches 160 cells — seed 69's city is cut in two by a river — cost if wrong: a footbridge where a planner wouldn't put one
- the home city stays the overworld's size (growing the starting city is Jeff's for later) — it comes to about half today's large city's land and buildings — cost if wrong: a smaller first city than today's until the overworld's home city grows
- a sited city with no megablock raises its largest block of 13×9 or more as one (lone cities unchanged) — the runner's home needs an atrium megablock and seed 8's Megablocks district had no block that big — cost if wrong: a megablock outside its district
- build_home passes over a world whose home city fails or has no flat, to hash64(seed, k) for k in 1..8 — seed 60's city is too small for a megablock; a run without a home has no story — cost if wrong: a seed typed for a world opens the next in its line
- a city with one fixer keeps Coz's ledger unplayed (the shipped design, the_doctor_plays_with_one_fixer) — I tried two fixers everywhere and reverted it — cost if wrong: some home cities lose the ledger thread
- a_new_game_passes_over_a_world_whose_city_has_no_home refuses seed 60 by a fit check (build_home_where) — with towns out, seed 60's city has a flat now — cost if wrong: none

**Left for later**

- prepare() returning None builds a lone city that still carries its site (Outside then lines up the wrong block); should be a GenError
- build_home's ? on Overworld::generate stops the line at the first failure instead of trying the next seed
- a new game builds the world three times (~350 ms on the frame thread); passing the built world through would save ~200 ms
- the open-ground fill doesn't copy `variant`, so the ground's glyph scatter in the block differs from the same cells seen from outside
- at night the city's open margin is darker than the open ground (drawn at least at MOONLIGHT); walking off, the ground brightens
- about 15% of home cities have one fixer, so Coz's ledger goes unplayed (the shipped fallback)
- the dev free camera's `r` rerolls the seed but keeps the old site, and doesn't check for a flat

## #20 City outskirts

**Calls made**

- OUTSKIRTS is 0.08, not the plan's 0.22 — at 0.22 the belt took ~45% of the buildings (seed 8: 162 → 89), at 0.14 ~30%; at 0.08 a belt of a few blocks (median 161 buildings over 120 new games) — cost if wrong: one constant
- a block is outskirts when its middle is thin and no corner is over OUTSKIRTS+0.1, and every district keeps its largest block whole — a big block up the slope laid shanties at density 0.4, and an all-outskirts district failed DistrictEmpty — cost if wrong: a few thin blocks laid as full blocks
- the belt keeps the pavement next to buildings, and the body shop's lot is paved (Plaza) — doors open only onto outdoor ground (road, sidewalk, alley, plaza), and gravel all round left four buildings without doors — cost if wrong: a strip of sidewalk at the last buildings
- shanties are rooms in the street itself (no building, so no construct, no interior) and the billboard is Sign letters between ▣ caps on two Pipe posts along the block's north or south side — the spec: shanties are off the grid; no new kinds — cost if wrong: shanties can't be entered as interiors
- the belt's fade keeps the grid walkable (only its lattice dots and district tint thin out, by density/0.2) — voiding it would cut constructs off the jacks — cost if wrong: the belt's grid walks like the core's
- a sited city's Net reach counts cells with a district (the open Net past the outline and the faded grid are walked, not counted) — open-Net grid dots are isolated walkable cells — cost if wrong: an unreached patch of the open Net inside the block
- the open Net covers only cells past the outline and more than 6 steps (FRINGE) from the city's land or buildings; the fringe is the city's grid, faded — a corner joined only round the outside stays joined — cost if wrong: a strip of city grid past the outline where the overworld Net has void (the seam is at the block edge, unaffected)
- the belt's fade drops a cell's dot and its district tint together (one roll) — so a district-less grid cell never shows a dot the city's look hides — cost if wrong: none

**Left for later**

- look mode has no name for the fire barrel (reads as the ground) and calls parked rigs and wrecks couches (Furniture::Couch)
- the trunk test checks one trunk meets the grid, not every highway; the shanty-door reach test doesn't cover body-shop doors (fine in the reviewer's probe); every_jack_still_reaches_the_grid isn't a test of its own (validate covers it)
- a billboard post can stand in front of a shanty's door in a reachable layout (never seen over 394 cities)
- the generator's own GateSealed rate (hub rings sealing a gate's pocket, ~0.75%) predates #20; a new game passes over such a world

## #21 Every city in the world

**Calls made**

- a sited settlement's districts are its own tiles / 50 (1 at least, never more than asked), and a sited city's least is 1 (a lone city's 4) — the smallest city of a world failed DistrictEmpty with 4 in 13 of 30 worlds — cost if wrong: small cities of one or two quarters
- a one-district town takes the overworld's district theme for its tile (OldTown without one) — theme assignment needed a core and a combat zone — cost if wrong: a town's look
- the home city's state is a Stay in a new saved AppState field (away), not in Outside — several of its types aren't Clone/Debug, which Outside derives — cost if wrong: none
- another city's people, interiors and the rest start fresh each time you come in (only its map is kept, for the session); only the home city's state is kept with the run — the spec: "only the city you're in runs its people", the catch-up is the off-screen world spec (#26) — cost if wrong: a dropped item in another city is gone when you come back
- another city's GenParams are the run's (density, decay, flavor) with a seed from the world seed and the settlement; no generation screen (a city builds in ~250 ms) — cost if wrong: a short hitch stepping in
- the existing tests that picked "open ground outside the home block" now pick ground off every settlement's block (in_any_block) — another city's block is walked into — cost if wrong: none
- coming home, the home city's people are run on to the run's hour (sim advance, as asleep) — its clock stopped while it was set aside and the run's clock is the loaded city's — cost if wrong: a long time away is a long catch-up on the frame thread (#26 decides the world while you're away)
- a lone city (no site: dev starts, tests, old saves, `strata gen`) keeps working as before, its world made round it from its seed with its block centred, and the world's other cities can be walked into from it; it isn't rebuilt through a one-city world — that would re-pin every lone-city fingerprint and move every test city — cost if wrong: a lone city's edges don't meet its world's ground
- re-graded minor 7 (deck and MAP picks into the city you left) to important and fixed — the_deck_forgets_the_city_you_left RED → GREEN — a stale building id could reach a lookup in the other city — cost if wrong: none

**Left for later**

- the run's card (LOAD GAME, CONTINUE) shows no story while away (describe::story reads st.missions)
- a place that won't build is retried (and its line logged) on every step inside its block
- a failed world rebuild on reopen sets outside to None while away holds home; home is lost for that run
- the district count can drift down across new games in one session (sited home params feed fresh_params)
- naming — outside::away (off the map), st.away (home set aside), away_from_home; the blocks OnceCell is a lazily rebuilt skipped cache

## #23 The overworld jacked in

**Calls made**

- the Net overworld is a look-only mode of the outside screen (net_up), pushed as its own screen from the city's map by W on the city's Net (not from inside a construct) — the spec: "from a street-level Net" — cost if wrong: W inside a construct still says nothing new
- the clock holds while you look up — looking costs no time, as the deck doesn't — cost if wrong: none
- POI tiers on the Net overworld: relays and water plants open (signal), checkpoints and test ranges black (red), the rest grey; camps none — round 1's notes per POI — cost if wrong: colours
- re-graded minor 2 (look mode's help lists the outside's keys) to important and fixed — the_net_overworlds_help_is_its_own RED → GREEN — wrong help on screen — cost if wrong: none
- re-graded minor 3 (the city's help doesn't say W on the Net) to important and fixed in help.rs — the key can't be found otherwise — cost if wrong: none
- J from the Net overworld jacks out at once from the city too (the outside screen's J never asks; the city's J asks) — cost if wrong: a jack-out from the city without its confirm

**Left for later**

- look mode swallows s (save) and Tab with no message
- the MAP pane shows the overworld's atlas look, not the Net's, while you look up
- W from the city's Net is a free pause mid-fight on your turn or mid-run (everything holds while you look)

## #24 The deck MAP's world zoom

**Calls made**

- a world place is marked (Outside.mark, saved), not tracked by the city's track (its walks, legs and pulses are the city's); its way shows in the tracking line and NEAR, and it clears on its tile; no floor pulse near it on the detail map — cost if wrong: no pulse leading you the last steps
- zoom 4 is one tile a cell round its own cursor (not the whole world fitted), panned by the cursor — the world is bigger than the deck's map area — cost if wrong: you scroll to see far places (PLACES lists them all)
- PLACES is nearest you first; the card's way is the straight bearing and the nearest highway — cost if wrong: none
- re-graded minor 5 (a job's last pane row lost to the mark's line) to important and fixed — a_mark_leads_the_pane_without_costing_a_job_row RED → GREEN — cost if wrong: none
- re-graded minor 8 (the city's MAP opening on the world after coming back in) to important and fixed — coming_into_a_city_the_map_is_the_citys RED → GREEN — cost if wrong: none

**Left for later**

- AtlasState::leave doesn't clear the world cursor (zoom 4 reopens where it was)
- a - then + round trip loses the city cursor (the world draw settles cursor None)
- [ ] stick on the first of two places sharing a tile; with nothing selected ] goes to the place nearest you, not the cursor
- bearing(0,0) says NW and "1 tiles" reads wrong
- standing_on_the_mark_clears_it asserts inside an if
- the world hint always says f fold and folded there's no strip; the title's region is the nearest however far; the card's route is the nearest however far
- two region labels can overlap on the chart ("SSable Bay")

## #25 Save the world with the run

**Calls made**

- the open ground's kept tiles are the seen memory (Outside.seen) and the seed's rebuild; there's no layer of changes because nothing the player does changes the open ground yet — the spec's "every kept tile with its changes" — cost if wrong: the first thing that changes the ground needs the layer
- "every city built so far" is Outside.visited (which cities and towns), their maps made again from the world when you go back; only the city you're in and the home set aside are saved whole — the maps are the same every time; their state isn't kept (#21's ruling) — cost if wrong: none for the map; a visited city's items

**Left for later**

- no fixture holds an Outside or a Stay yet; write a world fixture (away in a town, a mark, visited cities) once this ships
- Outside.blocks is a std::cell::OnceCell in a saved type (skipped; the doc now allows caches rebuilt when first needed)
- a_run_reopens_where_it_was_wherever_that_was skips the ground case if the cell is unwalkable, has a leftover binding, and doesn't check the town's here
- the world card's "you've been" has no test, matches by name, and towns never show (PLACES lists no towns)
