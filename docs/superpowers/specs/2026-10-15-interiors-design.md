# Interiors — design

Date: 2026-10-15. Status: approved in chat.

## Goal

Most buildings close. Their footprints become solid on the city maps, and a
door opens a separate interior map: real floors reached by lift and stair, with
exits onto the street, onto the Skyways over a bridge, onto the roof and down
into the Undercity. Stalls and small shops stay walk-in on the street as they
are today. People the simulation marks as inside a building are standing at
their stations when the player walks in.

## Decisions taken with Jeffrey

- **Real floor counts.** A corp tower has thirty-odd floors; a floor is
  generated the first time the lift stops there. The floors with outside exits
  are the ones that line up with the strata.
- **Open kinds.** NoodleBar, Market, Shop, Pawn and Arcade stay open on the
  street map (and Parking, which is a lot). Every other kind closes: CorpTower,
  Office, Megablock, Apartment, Hotel, Bar, Club, Clinic, Temple, Warehouse,
  Factory, Depot, Garage.
- **Scale.** A floor is the footprint scaled three times: each footprint tile is
  a 3×3 block inside, so doors line up with their street tiles. Minimum floor
  12×9.
- **Private rooms are locked.** Corridors, lobbies, bars, halls and shop floors
  are public. A flat, a hotel room and an office behind a corridor are locked;
  bumping the door says whose it is. Lockpicks and invitations come later.

Assumptions carried in: interiors are generated deterministically from the
city seed, the building and the floor, so saves stay city-only and nothing is
stored; doors remain the simulation's interface, as the living-city spec
promised; nobody walks indoors in this version; the clock runs while the
player walks, inside or out.

## Outside: closed buildings on the city maps

`BuildingKind::is_closed()` says which kinds close. For a closed building the
generator's Interiors stage no longer carves rooms. Instead:

- A new terrain, `Mass`, fills the inside of the footprint on every stratum the
  building spans except Heights. `Mass` is not walkable, blocks sight and
  counts as building part. The perimeter stays `Wall`, so facades keep their
  box-drawing, windows, neon, holo-ads and signs exactly as today.
- Doors. Street: the main door as today (preferring a sidewalk outside) and,
  where another side of the footprint faces outdoor ground, a second door on
  that side: the back door. Skyways: where a walkway or bridge touches the
  footprint, one door on that side: the bridge door. Undercity: where a tunnel,
  rail or grate touches the footprint, one door: the basement door. Heights: a
  tall building's roof stays `Rooftop` and walkable, with one `Door` tile on
  the roof above the main street door: the roof door, the stair-house seen from
  above.
- The in-footprint stair or lift connector of a closed building is not
  registered; its lobby terminal is not placed. Open buildings keep all of
  today's behaviour, connectors and terminals included.
- Reachability. `City::portals(building)` lists a closed building's door tiles
  by stratum (street doors, bridge door, roof door, basement door).
  `reachable_from_street` treats the portals of one building as mutually
  connected, standing in for the interior it cannot see, so a roof reached only
  through its tower counts as reached. The validator's door check and the
  connectivity repair use the same function. The small-component rule keeps
  its building exception.
- The tileset draws `Mass` as the style's mass glyphs in the theme's wall
  colour, dimmed, on the background: a dark bulk behind a lit facade. The
  inspector reads a `Mass` tile as the building, closed.

## Inside: the interior model

A new module `citygen::interior`.

```rust
pub struct FloorPlan {
    pub building: BuildingId,
    pub kind: BuildingKind,
    pub size: Size,          // footprint × 3, at least 12×9
    pub lowest: i32,         // -1 with a basement, else 0
    pub top: i32,            // the highest floor
    pub bridge: Option<i32>, // the floor the bridge door opens from
    pub roof: bool,          // a roof door on the top floor
    pub lift: Option<Pos>,   // the lift bank, the same tile on every floor
    pub stair: Pos,          // the stairwell, the same tile on every floor
}
pub struct Floor {
    pub level: i32,
    pub tiles: Grid<Tile>,
    pub light: Grid<u8>,
    pub rooms: Vec<Room>,
    pub exits: Vec<Exit>,
    pub stations: Vec<Station>,
}
pub struct Exit { pub at: Pos, pub stratum: StratumKind, pub landing: Pos, pub door: Pos }
pub struct Room { pub rect: Rect, pub kind: RoomKind, pub door: Option<Pos>, pub private: bool, pub number: u16 }
pub enum RoomKind { Lobby, Corridor, OpenPlan, Office, Flat, HotelRoom, Suite, Bar, Stage, Vip,
    Toilets, BackRoom, Waiting, Surgery, Hall, Shrine, Laundry, Lockers, Storage, Bays }
pub struct Station { pub pos: Pos, pub role: Role }
pub enum Role { Reception, Desk, Bar, Table, Machine, Surgery, Shrine, Shelf, Bay }
pub enum Furniture { Desk, Counter, Table, Stool, Shelf, Machine, Bed, Shrine, Locker }
```

`Feature::Furniture(Furniture)` is a new feature; furniture is not walkable. A
station is the floor tile beside the furniture where someone stands. Floors
use the existing terrains: `Floor`, `Wall`, `Door`, `Window` on the outer
wall, `Lift`, `Stair`.

`FloorPlan::new(city, building)` is deterministic. Floor counts by height
class and kind: low buildings 1 floor (Bar, Clinic, Temple, Warehouse, Depot,
Factory, Garage), 2 (Club), 2–3 (Office, Apartment); mid 4–9; tall CorpTower
18–48, Office 12–24, Megablock 14–30, Hotel 10–24, Apartment 8–14. The
basement is level −1 when the building spans the Undercity. The bridge floor
is 3 for buildings with a bridge door (or the top floor if lower). The roof
door is on the top floor of tall buildings. Buildings of three or more floors
have a lift bank; every building of two or more floors has a stairwell.

`generate_floor(city, plan, level)` builds one floor from
`hash(hash(seed, building), level)`. Floor types:

- **Lobby** (level 0 of towers, offices, megablocks, apartments, hotels): an
  open hall; a reception counter near the main door with a Reception station
  behind it; the lift bank and stairwell at the back; the back door if the map
  has one.
- **Corridor floor** (every other floor of those kinds, and the bridge floor):
  a two-wide corridor along the long axis, rooms on both sides every 5–8
  tiles, each with a door onto the corridor; the lift bank and stairwell open
  onto the corridor. Offices: locked `Office` rooms with a desk alternating
  with public `OpenPlan` rooms of desk rows (Desk stations). Megablocks and
  apartments: locked `Flat`s numbered `{level}{index:02}` and a `Laundry` every
  few floors. Hotels: locked `HotelRoom`s.
- **Suite** (top floor of tall towers and offices): open plan with a large
  locked office, and the roof door at the stairwell.
- **Bar** (Bar, Club level 0): a bar counter along one wall with Bar stations
  behind it, tables with stools (Table stations), toilets, a locked back room.
  A club adds a `Stage` and a level 1 `Vip` corridor floor whose rooms are
  public with tables.
- **Clinic**: a waiting room with stools, a `Surgery` with the Surgery station,
  a locked back room. **Temple**: a hall with the shrine at the far end (Shrine
  station) and a locked side room.
- **Hall** (Warehouse, Depot): shelving rows with Shelf stations; the back door
  is the dock. **Factory**: machine rows with Machine stations, a `Lockers`
  room, a locked office. **Garage**: bays (Bay stations) and a locked office.
- **Basement** (level −1): a `Storage` hall with shelves, the lift bank and
  stairwell, the basement door.

Exits. A city-map door on the footprint border maps to the floor's border at
three times its offset plus one, clamped to the edge; the floor tile there is a
`Door`, the tile inward of it is the doorstep, and the exit lands on the
outdoor tile in front of the city-map door. Street doors are exits of level 0,
the bridge door of the bridge floor, the roof door of the top floor (landing on
the roof door tile on Heights), the basement door of level −1 (landing on the
tunnel tile in front of it).

Light: a pure `citygen::lighting::compute(tiles, base, emission)` extracted
from the Dressing stage; floors get ceiling `Lamp` features every six tiles and
a base of 90.

Validation (`validate_floor`): every walkable tile reaches the lift bank or
stairwell; every exit's `at` is a `Door` on the border with a walkable
doorstep; every room with `private` has a door; furniture never stands on a
doorstep, a lift, a stair or an exit; the same seed gives the same floor.

## Moving between maps

`citygen::Place` is where a map is: `Stratum(StratumKind)` or
`Floor { building, level }`. The player's `stratum` becomes `place`, the body
left behind when jacking in is a `(Place, Pos)`, and sight memory is kept per
place. `game::At` carries a place; piles (`Ground`) and ability effects are
keyed by place. The sim's blocker is set only when the player stands on the
Street.

`AppState.interiors` caches `FloorPlan`s and generated `Floor`s per building;
`ensure(city, building, level)` generates on first visit. Interiors are not
saved.

Entering: walking onto a closed building's `Door` tile from outside it (on the
Street, the Skyways bridge door, the Heights roof door, the Undercity basement
door) puts the player on the matching exit's doorstep inside, with the camera
snapped and a log line naming the building. Leaving: walking onto an exit's
door tile inside puts the player on its landing outside. Lifts: `>`, `<` or
`space` on a lift tile opens the FLOORS popup, one row per level labelled
`B1`, `L`, `2F` … with `bridge` and `roof` marks; picking a row moves the
player to that floor's lift bank, generating the floor if needed. Stairs: `<`
and `>` on a stair tile move one floor. The HUD location reads the building
and the floor label; the inspector reads the room under the cursor.

## One renderer, two kinds of map

A `TileMap` trait in the app (`render/map.rs`): size, tile, light, theme,
building kind and connector direction at a position, and walkability. A city
stratum and a floor both implement it; `MapRef::for_place(state, place)` picks
one. `MapView`, the overlays, the memory map, the minimap, the field of view,
`Player::try_move`, the inspector and `hud::location` take a `TileMap` and a
`Place`. The explore screen keeps one `place`; in Walk mode it follows the
player, in the free camera it browses city strata as today.

## People inside

`World::inside(building)` lists the occupants: working there, living there, a
hotel guest on a date, or visiting. The app assigns each a station on the floor
being drawn, deterministically: workers by profession (barkeep → Bar, corp
drone → Reception for the first on the lobby then Desk, line worker → Machine,
ripperdoc → Surgery, monk → Shrine; the floor is the person's id modulo the
work floors), residents to flats and guests to rooms (shown as the locked door's
owner, not drawn), visitors to tables and bar stools. Extras beyond the stations
stand in the corridor. An `InteriorPeople` overlay draws them as `@` with their
tints; they are solid (no swap indoors) and `space` lists the ones beside you
with their cards. Nobody walks indoors: someone arriving or leaving while the
player is there appears or vanishes at their station.

## Out of scope

Jacking in from indoor terminals, shops selling, lockpicking, people walking
indoors, combat, saving interiors, a transition effect on the door.

## Tests

citygen: closed kinds and `Mass` on every spanned stratum but Heights, doors
on each stratum as specified, portals and reachability with closed buildings,
open kinds unchanged, validator and full-city validation on the test fixtures;
floor plans (counts, bridge, roof, basement), each floor type's rooms and
stations, exits mapping to the city-map doors, floor validation, determinism.
app: the trait over both maps, entering and leaving through every exit kind,
the FLOORS popup, stairs, memory and piles per place, the location line, the
inspector on a floor, locked doors with their owner, occupants at stations and
talking to them, the free camera unchanged. A smoke run and screenshots of a
tower lobby, a megablock corridor and a club floor.
