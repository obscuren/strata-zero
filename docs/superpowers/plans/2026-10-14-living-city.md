# Living City Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `sim` crate that populates the city with people who have professions, homes, workplaces, routines and cars, stepped in game minutes, drawn and talked to from the app; the random traffic is gone.

**Architecture:** `crates/sim` depends on `citygen` and `game`. `sim::content` loads `flavor/people.ron`. `sim::places` derives doors, kerbs, beats, corners, turf and lots from the city once. `sim::paths` wraps A* with pedestrian and car cost functions and a per-frame queue. `sim::world` holds people, cars, households, the minute step, `visible`, `person_at`, `card`. The app owns `AppState.world`, feeds it the clock, draws movers through `render/people.rs`, blocks moves, opens the talk card, extends the inspector and the console.

**Tech Stack:** Rust 2024, ratatui 0.29, serde + ron, rand_chacha for determinism (already a citygen dependency).

**Spec:** `docs/superpowers/specs/2026-10-14-living-city-design.md`

## Global Constraints

- Speeds: walking 30 tiles a game minute, driving 90. 90 ticks a game minute (`hud::clock_at`).
- Pedestrian terrain: Sidewalk, Plaza, Alley, Walkway, Bridge, Stair, Door at cost 1; Road at cost 3. Cars: Road and Bridge at cost 1, nothing else.
- Paths are computed once per trip, at most four per frame; a trip has not departed until its path exists.
- Determinism: everything derives from `city.seed` through `ChaCha8Rng`; the same city gives the same people.
- Version one lives on `StratumKind::Street`.
- Copy in `flavor/people.ron` is in voice; labels lower case.
- Every commit carries the two attribution trailers; `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` clean before each commit; the whole workspace suite green.

## Review Focus

1. A city whose buildings offer no home at all (no Apartment, Megablock or Hotel outside the Strip): seeding must not panic and must still produce people, housed in whatever housing exists or marked homeless with a street spot (Task 3 test).
2. A destination door with no reachable kerb or no path at all (an island block): the trip is dropped and the person stays put, never stuck in `Travelling` forever (Task 4 test).
3. Two cars wanting the same kerb tile: the second parks on the next free kerb tile; cars never share a tile at any minute (Task 4 test).
4. The clock wrapping past midnight and the schedule's day boundary: a 22–06 shift departs at 22:00 and comes home at 06:00 the next day, every day (Task 3 and Task 4 tests).
5. The player standing on a door tile or in a doorway an NPC needs: the NPC waits, then routes around or gives up after a minute, never overlaps the player (Task 5 test).

## File Structure

- `crates/sim/Cargo.toml`, `crates/sim/src/lib.rs`
- `crates/sim/src/content.rs` — `People`, `Profession`, `Hours`, `Leisure`, `Tint`, loading and validation.
- `crates/sim/src/places.rs` — doors, kerbs, lots, beats, corners, turf, muscle posts.
- `crates/sim/src/paths.rs` — cost functions, `walk_path`, `drive_path`, the pending queue.
- `crates/sim/src/people.rs` — `Person`, `Work`, `Segment`, `Goal`, schedules, households, seeding.
- `crates/sim/src/world.rs` — `World`, the step, trips and legs, encounters, `visible`, `card`.
- `flavor/people.ron`, `flavor/names.ron` (people section), `crates/citygen/src/flavor.rs` (the field).
- App: `crates/strata-zero/src/render/people.rs` (new), `render/traffic.rs` (deleted), `render/mod.rs`, `app.rs`, `screens/explore.rs`, `screens/popup.rs`, `screens/console.rs`, `ui/panel.rs`, `walker/mod.rs`, `README.md`.

---

### Task 1: The crate, the content and the names

**Files:**
- Create: `crates/sim/Cargo.toml`, `crates/sim/src/lib.rs`, `crates/sim/src/content.rs`, `flavor/people.ron`
- Modify: `Cargo.toml` (workspace member), `flavor/names.ron` (`people` section), `crates/citygen/src/flavor.rs` (`pub people: Vec<String>` with `#[serde(default)]`)

**Interfaces:**
- Produces: `sim::content::{People, Profession, Hours, Leisure, Tint, ContentError}`; `People::load()` (embedded `flavor/people.ron`), `People::parse(&str)`, `People::profession(&str) -> Option<&Profession>`; `citygen::FlavorPack.people`.

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct People { pub professions: Vec<Profession> }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profession {
    pub id: String,
    pub label: String,
    /// Building kinds that employ it; empty for spot and roaming jobs.
    #[serde(default)] pub works_at: Vec<BuildingKind>,
    pub hours: Hours,
    /// Staff per workplace building, inclusive; spot jobs read it per district.
    pub staff: (u8, u8),
    #[serde(default)] pub leisure: Vec<(Leisure, f32)>,
    pub lines: Vec<String>,
    #[serde(default)] pub night_line: Option<String>,
    #[serde(default)] pub tint: Option<Tint>,
    #[serde(default)] pub car_nudge: f32,
}
pub enum Hours { Day { open: u16, close: u16 }, Shifts(Vec<(u16, u16)>), Beat { from: u16, to: u16 } }
pub enum Leisure { Bar, NoodleBar, Strip }
pub enum Tint { Magenta, Amber, Red, Green }
```

Hours are minutes of the day; `close < open` wraps past midnight.

- [ ] **Step 1: Failing tests** in `content.rs`: `people_content_loads` (twelve professions, `sex_worker` is a `Beat` 21:00–05:00, `line_worker` has three shifts, `corp_drone` leisure includes `Strip` at 0.15, every profession has at least two lines, `door_muscle` has a `night_line`); `validation_catches_broken_people` (a profession with one line → `ContentError::TooFewLines`; `Day { open: 1500, .. }` → `ContentError::BadHours`; `staff: (3, 1)` → `ContentError::BadStaff`; a duplicate id → `ContentError::DuplicateId`); in `citygen::flavor` tests: the default pack has at least forty people names.
- [ ] **Step 2: Watch them fail** (`cargo test -p sim`, `cargo test -p citygen -- flavor`).
- [ ] **Step 3: Implement** `content.rs`, write `flavor/people.ron` with the twelve professions from the spec's table (lines in voice, three or more each), add `people:` handles to `names.ron` (forty-plus, in the register of VEX and KESSLER: "Juno", "Kessler", "Tamsin", "Rook", "Ngozi", "Hale", "Pike", "Mireille", "Dax", "Oyelaran"...), add `pub people: Vec<String>` to `FlavorPack`.
- [ ] **Step 4: Green, lint, commit** `feat(sim): the crate, the professions and the people names`.

---

### Task 2: Places and paths

**Files:**
- Create: `crates/sim/src/places.rs`, `crates/sim/src/paths.rs`

**Interfaces:**
- Produces:

```rust
pub struct Places {
    pub doors: HashMap<BuildingId, Vec<Pos>>,     // street doors
    pub kerbs: Vec<Pos>,                           // Road tiles beside a Sidewalk
    pub lots: Vec<(BuildingId, Vec<Pos>)>,         // Parking/Garage road-side tiles
    pub beats: HashMap<DistrictId, Vec<Pos>>,      // Strip sidewalks under Club/Bar fronts, else Night Market alleys
    pub corners: HashMap<DistrictId, Vec<Pos>>,    // sidewalk tiles at road intersections
    pub turf: HashMap<BuildingId, Vec<Pos>>,       // sidewalk ring around a Megablock
    pub posts: HashMap<BuildingId, Pos>,           // the sidewalk tile beside a building's first door
}
impl Places { pub fn derive(city: &City) -> Places; pub fn nearest_kerb(&self, city: &City, from: Pos) -> Option<Pos>; }
pub fn walk_cost(city: &City, from: Pos, to: Pos) -> Option<u32>;
pub fn drive_cost(city: &City, from: Pos, to: Pos) -> Option<u32>;
pub fn walk_path(city: &City, from: Pos, to: Pos) -> Option<Vec<Pos>>;
pub fn drive_path(city: &City, from: Pos, to: Pos) -> Option<Vec<Pos>>;
```

- [ ] **Step 1: Failing tests** on the default city (`Generator::run(GenParams::default())`): every building with a street door appears in `doors` and every listed door is a `Terrain::Door` on the footprint edge with a street tile beside it; `kerbs` are Road tiles with a Sidewalk neighbour; a Neon Strip district has beats on Sidewalk or Plaza tiles inside the district; a city generated with a flavor that has no Strip (or by filtering districts) falls back to Night Market alleys; `walk_path` between two doors uses only pedestrian terrain and crosses Road only where needed; `drive_path` between two kerbs uses only Road or Bridge; `nearest_kerb` from a door is reachable on foot.
- [ ] **Step 2: Watch them fail.**
- [ ] **Step 3: Implement.** Doors: scan each building's footprint perimeter for Door tiles with a Sidewalk/Plaza/Alley/Road 4-neighbour. Kerbs: Road tiles with a Sidewalk 4-neighbour. Beats: in `Theme::NeonStrip` districts, Sidewalk and Plaza tiles 4-adjacent to a Club or Bar footprint; fallback `Theme::NightMarket` Alley tiles. Corners: Sidewalk tiles with two or more diagonal Road neighbours. Turf: Sidewalk tiles within two tiles of a Megablock footprint. Posts: the first street-side neighbour of a building's first door. Paths: `citygen::pathing::astar` with the cost closures; `nearest_kerb` by `bfs_nearest` over pedestrian terrain to a kerb, max 200.
- [ ] **Step 4: Green, lint, commit** `feat(sim): places and paths`.

---

### Task 3: People, schedules and seeding

**Files:**
- Create: `crates/sim/src/people.rs`
- Modify: `crates/sim/src/lib.rs`

**Interfaces:**
- Produces:

```rust
pub struct PersonId(pub u32); pub struct CarId(pub u32); pub struct HouseholdId(pub u32);
pub enum Work { Building(BuildingId), Spot(Pos), Roaming }
pub enum Goal { Home, Work, Leisure(Leisure), Strip }
pub struct Segment { pub at: u16, pub goal: Goal }
pub struct Person { pub id, pub name: String, pub profession: String, pub home: Option<BuildingId>, pub work: Work,
    pub car: Option<CarId>, pub household: HouseholdId, pub regular: Option<BuildingId>, pub schedule: Vec<Segment>,
    pub whereabouts: Whereabouts, pub line: String, pub tint: Option<Tint> }
pub struct Car { pub id: CarId, pub owner: PersonId, pub pos: Pos, pub hue: Hue, pub parked: bool }
pub struct Population { pub people: Vec<Person>, pub cars: Vec<Car> }
pub fn seed_population(city: &City, people: &People, places: &Places, params: &SimParams, rng: &mut ChaCha8Rng) -> Population;
pub fn goal_at(schedule: &[Segment], minute_of_day: u16) -> Goal;   // the last segment at or before the minute, wrapping
```

Schedules per profession: `Day { open, close }` → `[at: open-30±jitter → Work, at: close±jitter → Leisure or Home, at: leisure_end → Home]`; `Shifts` → one shift picked per person at seeding; `Beat` → `[from → Work(Spot), to → Home]`. Leisure is rolled per person at seeding from the profession's `leisure` list (the Strip roll also marks the person a client). Jitter: up to 20 minutes, from the rng, fixed per person.

- [ ] **Step 1: Failing tests**: on the default city with density 1.0 the population is between 80 and 400, every person has a workplace matching their profession's kinds or a spot, every housed person's home is Apartment, Megablock or Hotel outside the Strip; two seedings with the same seed are identical and density 2.0 roughly doubles; households share a home and have 1 to 3 members; `goal_at` on a 22–06 shift gives Work at 23:00 and at 03:00 and Home at 07:00 (Review Focus 4); colleagues (same workplace) have departure minutes within 10 of each other; a city with no housing (strip every residential kind from a generated city's buildings) still seeds with people at street spots and no panic (Review Focus 1); car share near `car_share` and every car on a kerb or lot; Strip fallback when no Neon Strip district exists.
- [ ] **Step 2: Watch them fail.**
- [ ] **Step 3: Implement.** Count staff per workplace building from the profession table, then spot jobs per district, then gangers per Megablock, scaled by density; draw names without replacement until the pool runs out, then suffix a number; assign homes by nearest housing building with capacity (Apartment 8, Megablock 30, Hotel 6, scaled), corp drones allowed any district; households: pair up to three people sharing a home with matching shifts 30 percent of the time; regulars: a Bar in the home district when one exists; cars by `car_share` × district wealth × profession nudge, parked at the nearest kerb to the home (or a lot within twenty), never two on one tile.
- [ ] **Step 4: Green, lint, commit** `feat(sim): people, schedules and seeding`.

---

### Task 4: The world and its minute

**Files:**
- Create: `crates/sim/src/world.rs`
- Modify: `crates/sim/src/lib.rs`

**Interfaces:**
- Produces `World` per the spec: `seed`, `advance`, `path_some`, `visible`, `person_at`, `car_at`, `card`, `is_open`, `minute`, `people()`, `cars()`.

Trip legs: `Leg { path: Vec<Pos>, by_car: Option<CarId> }`; a trip's position at minute `m` and tick `t` walks the legs by elapsed tiles (walk 1 tile per 3 ticks, drive 1 per tick). Arrival: the last leg done → `Inside` for a building goal, `At` for a spot. Departure: when `goal_at(schedule, minute)` changes from the person's current goal, push a `TripRequest`; `path_some` resolves requests into trips (walk, or walk-drive-walk when the owner has a car and the straight-line distance exceeds 60), dropping requests with no path (Review Focus 2). Cars: a driving car's next tile taken by another car waits (the trip's elapsed counter stalls by one tick); parking picks the nearest free kerb (Review Focus 3). Encounters: the five rules as small functions on the step. `is_open`: any worker `Inside` their workplace within hours. `visible`: movers in the rect from `Travelling` and `At` people and parked or driving cars; `…` chat markers.

- [ ] **Step 1: Failing tests** (default city, run a day from 22:00 in minute steps with `path_some(city, usize::MAX)` after each): a shopkeeper is `Inside` their shop at 12:00 and at home at 03:00; at 23:00 at least one sex worker is `At` a beat and at 12:00 none; a corp drone with a car and a long commute has a trip with a drive leg and the car ends within three tiles of a kerb nearest the office; cars never share a tile at any minute; every walk leg on pedestrian terrain, every drive leg on Road or Bridge; colleagues on a shift arrive within ten minutes; the 22–06 shift goes out at 22 and comes home at 06 on day two (Review Focus 4); a trip to an unreachable door (a door whose building sits on an island: pick one by removing paths, or construct a tiny city) is dropped and the person stays `Inside` (Review Focus 2); two cars sent to the same kerb park on different tiles (Review Focus 3); `visible` at 23:00 over the Strip returns walkers at positions on their paths or beats and nothing outside the rect; `person_at` finds one of them; `card` for a shopkeeper at noon says "minding the shop" and gives a line; `is_open` true for that shop at noon and false at 03:00; same seed same world after a day.
- [ ] **Step 2: Watch them fail.**
- [ ] **Step 3: Implement** `world.rs`.
- [ ] **Step 4: Green, lint, commit** `feat(sim): the world and its minute`.

---

### Task 5: The city on screen

**Files:**
- Create: `crates/strata-zero/src/render/people.rs`
- Delete: `crates/strata-zero/src/render/traffic.rs`
- Modify: `crates/strata-zero/src/render/mod.rs`, `app.rs` (`world`, seeding in `enter_city`, the `sim` dependency), `screens/explore.rs` (advance, path_some, overlay, blocking, bump → card, `space` lists people), `screens/popup.rs` (`Pick::Card` reuse), `walker/mod.rs` (`try_move` takes a `blocked` closure), `ui/panel.rs` (inspector), `screens/console.rs` (`who`), `crates/strata-zero/Cargo.toml`.

- [ ] **Step 1: Failing tests**: explore at 120×40 after seeding draws at least one dim `@` that is not the player within the first game hour (advance the clock, call update, draw, count `@` cells with the dim foreground); the player's `@` stays bold white; moving into a tile that `person_at` reports pushes a popup whose text has the name, profession and line (Review Focus 5 also: an NPC whose next tile is the player waits, then is elsewhere two minutes later and never on the player's tile); `space` lists `t talk` for an adjacent person; the inspector shows `name`/`profession`/`doing` for a person under the cursor; `who` reports counts; `render::traffic` no longer exists (compile); doors glow at night when open (the door cell's fg differs from a closed door's).
- [ ] **Step 2: Watch them fail.**
- [ ] **Step 3: Implement.** `AppState.world: Option<sim::World>`, seeded in `enter_city` with `sim::People::load()` and `hud::minute_at(tick)`; `explore::update` calls `world.advance(city, minute)` then `world.path_some(city, 4)`; `render/people.rs` implements `Overlay` drawing `visible(viewport grown by 20)`; `walker::Player::try_move(city, dir, blocked: impl Fn(Pos) -> bool)`; a blocked move into a person opens `Popup::verbs(name, [t talk → Pick::Card(card lines)])`; the HERE interact adds `t talk <name>` rows for adjacent people; `panel::inspector_lines` gains a `person: Option<&Card>` argument; console `who`.
- [ ] **Step 4: Green, lint, commit** `feat(app): people on the street, the traffic overlay gone`.

---

### Task 6: Docs, smoke, screenshots

- [ ] **Step 1: README**: a "People" paragraph (professions, routines, cars, talking, `who`), the deleted traffic mention, the Keys additions.
- [ ] **Step 2: Smoke `drive11.py`**: VEX through the chooser, walk, `:` `who` → expects `out`, wait a game minute, `who` again; bump search is not scriptable, so assert the `@` count on screen grows over a minute via the dump.
- [ ] **Step 3: Screenshots** of the Strip at 23:00 and a factory row at noon (use the dev console to set the clock if a `clock <hh:mm>` command exists; add one if not, minimal) and send them.
- [ ] **Step 4: Commit** `docs: the living city`.
