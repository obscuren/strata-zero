# The living city

People live in Strata Zero. Every NPC has a name, a profession of their own, a
home, a place of work and a routine that moves them through the day: on foot,
or by a car they own and park. Shops open when their keeper arrives. The Neon
Strip fills after dark and empties by noon. The random traffic goes; every car
on the road belongs to someone going somewhere. Some of them talk.

The simulation is its own crate, `sim`, stepping in game minutes. It simulates
everyone, always, but spends nothing on anyone the player cannot see: paths are
computed once per trip, and a position out of view is derived on demand.

## Brief

What Jeffrey asked for, in order:

- NPCs that are alive, with light relations to each other (do not overdo it).
- Professions of their own, distinct from the player's categories.
- A home building, a workplace, a routine: shop owners open their shop in the
  morning if it is a day shop, sex workers hold a zone, and so on.
- Proper travel: on foot, or by car if they own one, from a parked spot or a
  garage later; into the car, drive to an appropriate place, out and walk.
- Activities at the destination. Some can be talked to now. Shops sell later.
- A sex worker profession with its own district.
- The random cars go.
- Flows matter most: the city as a system. Individuals and encounters are also
  in this spec, in that order.
- Simulate everyone, coarse far away, fine in view.
- Buildings are open on the map today; later, entering one opens a separate
  interior map, multi-level, with exits on other sides and floors and sky
  bridges. The simulation must survive that.

Assumptions carried in: the walker's player and the NPCs share the city's
tiles; cars drive on roads only and are solid to walkers; version one lives on
the Street stratum; saves still hold the city only.

## Voice

Every profession name, activity line and talk line in `flavor/people.ron` reads
like the rest of the content: second person or plain street, short, no
rulebook words. Profession labels are lower case.

## Time and the step

The sim keeps its own clock in game minutes, fed from the app's tick through
the HUD's clock (90 ticks a minute, starting at 22:00). `World::advance(city,
minute)` runs one `step` per elapsed minute. A real-time clock now and a turn
clock later drive it the same way.

Whereabouts is one of three states:

- `Inside(building)`: at a door, not on a tile.
- `At { pos, activity }`: standing on a street tile, doing something.
- `Travelling(trip)`: on a path from A to B with a departure minute.

Speeds in tiles per game minute: walking 10 (1 tile per 9 ticks), driving 30
(1 tile per 3 ticks). Revised down from 30 and 90 while building: at the faster
pace a sixty-tile commute took six real seconds and the streets stood empty
between departure bursts. A traveller's position is derived from the departure minute
and the speed, so an NPC out of sight costs nothing per tick and one who walks
into view mid-trip is exactly where the path says. Nobody teleports.

A path is computed once, when the trip starts, on the street tiles of the
Street stratum. Departures go into a queue pathed at most four per frame, so a
big map never hitches. Until its path exists a trip has not departed.

The world is seeded at game start in the state the schedules imply for that
minute: the night crowd out, day workers home, trips in progress.

A step: depart everyone whose schedule says so; arrive everyone whose trip is
done and put them inside or at their spot; tick each spot's activity; apply the
five encounter rules. Nothing else happens per minute.

## People

### Professions

| id | label | works at | hours | after work |
|---|---|---|---|---|
| shopkeeper | shopkeeper | Shop, Market, Pawn | 09–20; Pawn 12–02 | home, sometimes a noodle bar |
| noodle_cook | noodle cook | NoodleBar | 17–03 | home |
| barkeep | barkeep | Bar, Club | 20–05 | home |
| corp_drone | corp drone | Office, CorpTower | 08–18 | bar or club 40 percent, the Strip 15 percent |
| line_worker | line worker | Factory, Warehouse, Depot | shifts 06–14, 14–22, 22–06 | noodle bar, the Strip 10 percent, then home |
| courier | courier | Depot | 08–20, a chain of trips between workplace doors | home |
| ripperdoc | ripperdoc | Clinic | 10–22 | home |
| sex_worker | sex worker | the Strip's beat | 21–05 | home |
| fixer | fixer | a corner in the Strip or the Night Market, then a bar | 18–04 | home |
| door_muscle | door muscle | the door of a CorpTower or Office by day, a Club by night | shifts, standing outside | home |
| monk | monk | Temple | 06–19 | home |
| ganger | ganger | a Megablock's turf, roaming its block | 20–04, the Strip 20 percent | home |

Every start and end carries a jitter of up to twenty minutes per person,
fixed at seeding, so nobody moves in lockstep and the same person keeps the
same habits.

### Population

Staff per workplace kind: Shop 1 to 2, Market 2 to 3, Pawn 1, NoodleBar 2,
Bar 2 to 3, Club 3 to 4, Office 6 to 10, CorpTower 12 to 20, Factory 8 to
12, Warehouse 4 to 6, Depot 4 to 6 (half couriers), Clinic 2 to 3, Temple 1
to 2. Each Neon Strip district gets 4 to 8 sex workers and 2 fixers; each
Night Market district 1 fixer; each Megablock 3 to 5 gangers; each CorpTower,
Office and Club one door muscle per shift. `SimParams.density` (default 1.0)
scales every count. The default city lands near a hundred and fifty people.

Homes come from Apartment, Megablock and Hotel outside the Strip, in the
workplace's district or a neighbouring one, chosen by distance with corp
drones allowed the furthest. The Strip's hotels are short-stay stock, not
homes. Cities without a Strip put sex workers on the Night Market's alleys.

### Relations

Three kinds and no more. A *household* of two or three sharing a home, who
leave together when their shifts match. *Colleagues* at the same workplace.
A *regular* bar, chosen in the home district, that leisure trips return to.

### Identity

A handle from the names file's new `people` section, the profession, home,
workplace, a car or none, household, regular, the schedule, and one talk line
drawn at seeding from the profession's pool.

## Streets and cars

*Walking.* Pedestrian terrain: Sidewalk, Plaza, Alley, Walkway, Bridge, Stair
and Door at cost 1, Road at cost 3. NPCs may share a tile with each other (the
top one draws) but never with the player or a car. An NPC whose next tile
holds the player waits a tick, then steps around. The player cannot walk into
an NPC.

*Doors.* A building's street doors are its Door tiles on the footprint's edge
with a street tile beside them. A trip ends at the nearest one; the NPC steps
inside and disappears, and reappears on a door when leaving. A building is
*open* while someone works inside within its hours; its doors glow at night
while open. Interiors as separate maps will keep this contract: doors are the
interface, floors are not.

*Spots.* Standing jobs happen on street tiles: the sex worker's beat, the
fixer's corner, the muscle beside the door, the ganger's turf. At a spot an
NPC shuffles one tile every minute or two. The courier's day is a chain of
short trips between workplace doors.

*Cars.* One owner each. Ownership follows the home district's wealth with the
profession as a nudge: most corp drones, few gangers, `SimParams.car_share`
(default 0.25) overall. A car starts parked at the kerb nearest the owner's
home, a Road tile beside a Sidewalk, or in a Parking lot within twenty tiles.
A trip longer than sixty tiles by an owner with a car has three legs: walk to
the car, drive on Road and Bridge tiles at 1 tile per tick, park at the kerb
nearest the destination door, walk the rest. The car stays where it was left.
Cars never share a tile: one waits behind another. No lights, lanes or
accidents.

*What goes.* `render/traffic.rs` is deleted. A `render/people.rs` overlay
draws the sim's walkers and cars inside the bubble, cars with the old
overlay's `▬` glyph and headlights at night.

*Strata.* Version one lives on the Street stratum. Skyways and the Undercity
wait for interiors and connectors.

## Encounters and talking

Five rules between NPCs, in the minute step:

1. Colleagues on the same shift depart within minutes of each other.
2. Regulars go back to the same bar.
3. Two NPCs sharing a spot or a door turn to each other for a minute now and
   then, drawn as `…` between them.
4. An open noodle bar at peak hours grows a queue at its door: arrivals stand
   in line a minute before going in.
5. The Strip: a client and a sex worker walk into a hotel together and come
   out twenty to forty minutes later.

With the player: walking into an NPC talks to them; `space` on the player's
tile lists adjacent people with a talk verb. The card: name, profession, what
they are doing right now in voice ("heading home from the office", "working
the Strip", "minding the shop till eight"), their line, a relation hint when
there is one. Verbs: `t talk` now, `s shop` reserved. Door muscle at a club
door at night has its own line. No other profession-specific reactions.

## Rendering

NPCs are `@` in the dim text colour; the player's bold white `@` stays
unmistakable. Four tints: sex workers magenta, fixers amber, door muscle red,
gangers green. Cars `▬` in their hue with headlights at night. Open doors glow
at night. The dev inspector shows a person's name, profession, state and
destination for whatever is under the cursor.

## The crate

`crates/sim`, depending on `citygen` and `game`.

```rust
pub struct SimParams { pub density: f32, pub car_share: f32 }
pub struct People { pub professions: Vec<Profession> }   // flavor/people.ron
pub struct Profession { id, label, works_at: Vec<BuildingKind>, hours: Hours, staff: (u8, u8),
    leisure: Vec<(Leisure, f32)>, lines: Vec<String>, night_line: Option<String>, tint: Option<Tint> }
pub enum Hours { Day { open: u16, close: u16 }, Shifts(Vec<(u16, u16)>), Beat { from: u16, to: u16 } }
pub struct World { people: Vec<Person>, cars: Vec<Car>, households, spots, pending: VecDeque<TripRequest>, minute: u64, open: BTreeSet<BuildingId> }
pub struct Person { id, name, profession, home: BuildingId, work: Work, car: Option<CarId>, household: HouseholdId,
    regular: Option<BuildingId>, schedule: Vec<Segment>, whereabouts: Whereabouts, line: String }
pub enum Work { Building(BuildingId), Spot(Pos), Roaming }
pub struct Segment { pub at: u16, pub goal: Goal }        // minute of day, Home | Work | Leisure(kind) | Strip
pub enum Whereabouts { Inside(BuildingId), At { pos: Pos, activity: Activity }, Travelling(Trip) }
pub struct Trip { from: Pos, to: Pos, depart: u64, legs: Vec<Leg>, goal: Goal }   // Leg { path: Vec<Pos>, by_car: Option<CarId> }
pub struct Car { id, owner: PersonId, pos: Pos, hue: Hue, parked: bool }
pub struct Mover { pub pos: Pos, pub kind: MoverKind /* Walker(PersonId, tint) | Car(CarId, hue, heading) | Chat(Pos) */ }
pub struct Card { pub name, pub profession, pub doing, pub line, pub relation: Option<String> }

impl World {
    pub fn seed(city: &City, people: &People, params: &SimParams, minute: u64) -> World;
    pub fn advance(&mut self, city: &City, minute: u64);
    pub fn path_some(&mut self, city: &City, budget: usize);   // called once per frame
    pub fn visible(&self, bubble: Rect, tick: u64) -> Vec<Mover>;
    pub fn person_at(&self, pos: Pos, tick: u64) -> Option<PersonId>;
    pub fn car_at(&self, pos: Pos, tick: u64) -> Option<CarId>;
    pub fn card(&self, id: PersonId, city: &City) -> Card;
    pub fn is_open(&self, building: BuildingId) -> bool;
    pub fn minute(&self) -> u64;
}
```

`sim::places` derives from the city, once at seeding: street doors per
building, kerbs, the Strip's beats, corners, turf, parking lots. `sim::paths`
wraps the generator's A* with the two cost functions. Everything is
deterministic from the city's seed.

Validation at load: every building kind a profession names exists; every
profession has at least two lines; hours are within a day; staff ranges are
ordered. The names file gains `people: Vec<String>`.

## App seams

- `AppState.world: Option<sim::World>`, seeded in `enter_city` from the city
  and the clock. A loaded city re-seeds its people at the clock's time.
- The explore screen calls `advance` with the clock's minute each frame, then
  `path_some(city, 4)`, then draws `visible` for the viewport plus a margin of
  twenty tiles through `render/people.rs`.
- The walker's move consults `person_at` and `car_at`; a move into a person
  opens the talk card as a popup; `space` lists adjacent people with `t talk`.
- The inspector reads `card` for the cursor tile. The dev console gains
  `who`, which reports how many people are out, inside, and driving.
- Doors glow when `is_open`.

## Cost

A step touches every person once with no pathing. Paths happen on departure,
four per frame at most. Between steps only movers in the bubble are derived.
Nothing scales with map size but the paths, and those are bounded.

## Testing

Sim crate, headless, on the default city: a whole day runs in milliseconds.
A shopkeeper is inside the shop at noon and at home at three; the Strip has
sex workers at their beats at 23:00 and none at noon; a long-commute car
owner's trip has a drive leg and the car ends at the destination kerb;
colleagues on a shift arrive within ten minutes; every walk leg lies on
pedestrian terrain and every drive leg on Road or Bridge; no two cars share a
tile at any minute of the day; the same seed gives the same world; a city
without a Strip has sex workers in the Night Market; population scales with
density; `visible` returns only movers in the rect, at positions on their
paths; `person_at` finds a walker in view.

App: a walker draws as `@` in the dim colour and the player stays bold; a
bump opens the card with name, profession and line; the move is blocked; the
inspector reads a person; `who` counts; the traffic overlay is gone. A smoke
run opens a city, waits a minute, and checks `who` reports people out.

## Out of scope

Combat, pickpocketing, following reactions, shops selling, dialogue trees,
saving the world's state, interiors, garages, Skyways and Undercity life,
weather and events, NPCs on other strata, relations beyond the three.

## Revisions

- 2026-10-14, after the build (user): walking into someone swaps places with
  them instead of opening their card; they drift back once the player moves
  on. `space` keeps the talk list.
- 2026-10-14, after the build (user): the world keeps its own clock, which
  runs only while the player walks; the free camera and every menu freeze
  the city. The HUD clock and the log stamps read that clock.
- 2026-10-14, after the build (user): the inspector's overview of a person
  adds what comes next (an arrival, or the next thing on the schedule),
  where they live and work, and where their car is.
