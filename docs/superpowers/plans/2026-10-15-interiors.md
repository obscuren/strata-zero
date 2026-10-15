# Interiors Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close every building but the stalls and small shops on the city maps, and open each closed one as a separate interior map of real floors with lifts, stairs and exits, with the simulation's occupants standing at their stations inside.

**Architecture:** citygen gains a `Mass` terrain and per-stratum doors for closed buildings, a `Place` (a stratum or a building floor), and an `interior` module that plans a building's floors and generates one floor at a time, deterministically. The app gets a `TileMap` trait that a city stratum and a floor both implement, so the viewport, overlays, memory, minimap, field of view, walker and inspector work on either; the explore screen holds a `Place`, moves between maps on door tiles, lifts and stairs, and places the sim's occupants at stations when the player is inside.

**Tech Stack:** Rust 2024, ratatui 0.29, serde/ron, rand_chacha. Crates `citygen`, `game`, `sim`, `strata-zero`.

**Spec:** docs/superpowers/specs/2026-10-15-interiors-design.md

## Global Constraints

- Every commit passes `cargo clippy --all-targets -- -D warnings` and `cargo test --workspace`; `cargo fmt --all` before each commit.
- Closed kinds: everything but `NoodleBar`, `Market`, `Shop`, `Pawn`, `Arcade`, `Parking`.
- A floor is the footprint ×3, at least 12×9. Private rooms are locked.
- Same seed, same city, same interiors. Interiors are never saved.
- Test fixture city: `GenParams { seed: 6, size: Custom { w: 120, h: 60 }, districts: 8 }` (has every theme) and the `small()` test cities in citygen.
- Commit attribution lines as in the session's system reminder.

## Review Focus

1. A closed building whose street doors face only an alley or plaza (no sidewalk): the main door is still placed, the exit's landing is that outdoor tile, and entering works from it (Task 1 door test, Task 5 entering test on such a building if the fixture has one, else a hand-built city).
2. A footprint at the minimum (4×3 → 12×9 floor): the lobby still fits a reception counter, the lift bank, the stairwell and two exits without furniture on a doorstep (Task 2 test on the smallest footprint in the fixture).
3. Entering from a roof, bridge or basement door while the player stands on Heights, Skyways or the Undercity: the player lands on the right floor, leaving puts them back on that stratum at the landing, and sight memory is kept per place (Task 5 test for each exit kind).
4. A FLOORS pick for a floor not yet generated while occupants are assigned to it: the floor is generated before placement, never a panic or an empty floor (Task 6 test picks a high floor straight from the lobby).
5. Jacking in while inside: no terminal is in reach, the message says so, and the body place round-trips when a jack exists on the Street (Task 5 test).

## File Structure

- `crates/citygen/src/tile.rs` — `Terrain::Mass`, `Feature::Furniture(Furniture)`, walkability and sight rules.
- `crates/citygen/src/world.rs` — `BuildingKind::is_closed`, `City::portals`, `Place`.
- `crates/citygen/src/generate/interiors.rs` — closed footprints: mass, doors per stratum, no shaft or lobby terminal.
- `crates/citygen/src/generate/connectivity.rs`, `validate.rs` — portals in reachability.
- `crates/citygen/src/lighting.rs` (new) — `compute(tiles, base, emission)`; the Dressing stage calls it.
- `crates/citygen/src/interior/mod.rs`, `plan.rs`, `floor.rs`, `layouts.rs` (new) — plans, floors, layouts, validation.
- `crates/game/src/action.rs`, `ground.rs` — `At { place, pos }`, piles keyed by place.
- `crates/strata-zero/src/render/map.rs` (new) — `TileMap`, `StratumMap`, `FloorMap`, `MapRef`.
- `crates/strata-zero/src/render/{viewport,items,fx,overlay,weather,people,memmap,minimap,tileset}.rs` — take a `TileMap` and a `Place`; `Mass` and furniture glyphs.
- `crates/strata-zero/src/walker/mod.rs` — `Player.place`, memory per place, `try_move`/`update_fov` over a `TileMap`.
- `crates/strata-zero/src/interiors.rs` (new) — the cache, entering/leaving helpers, placement of occupants.
- `crates/strata-zero/src/screens/explore.rs`, `popup.rs`, `console.rs`, `ui/hud.rs`, `ui/panel.rs`, `ui/sidebar.rs`, `app.rs`, `actions.rs`.
- `crates/sim/src/world.rs` — `World::inside`.
- `README.md`.

---

### Task 1: Closed buildings on the city maps

**Files:**
- Modify: `crates/citygen/src/tile.rs`, `crates/citygen/src/world.rs`, `crates/citygen/src/generate/interiors.rs`, `crates/citygen/src/generate/connectivity.rs`, `crates/citygen/src/generate/dressing.rs`, `crates/citygen/src/validate.rs`
- Modify: `crates/strata-zero/src/render/tileset.rs`, `crates/strata-zero/src/ui/panel.rs`

**Interfaces:**
- Produces: `Terrain::Mass` (not walkable, blocks sight, building part); `BuildingKind::is_closed(self) -> bool`; `City::portals(&self, b: BuildingId) -> Vec<(StratumKind, Pos)>` (door tiles of a closed building on every stratum, the roof door included); `Building::main_door(&self, city) -> Option<Pos>` is not needed — the sim's `Places` reads Door tiles as today.

- [ ] **Step 1: Failing tests in `generate/interiors.rs`**

Replace the two existing tests with these (the old ones assume rooms in every building):

```rust
#[cfg(test)]
mod tests {
    use crate::generate::testutil::small;
    use crate::params::WaterMode;
    use crate::pathing::components;
    use crate::tile::Terrain;
    use crate::world::{BuildingKind, StratumKind};

    #[test]
    fn closed_buildings_are_mass_behind_their_facade_and_open_ones_keep_rooms() {
        let c = small(61, WaterMode::Bay, 7);
        let st = c.stratum(StratumKind::Street).unwrap();
        for b in &c.buildings {
            let doors: Vec<_> = b.footprint.border().filter(|p| st.tiles[*p].terrain == Terrain::Door).collect();
            assert!(!doors.is_empty(), "{:?} has no door", b.id);
            if b.kind.is_closed() {
                for p in b.footprint.inset(1).iter() {
                    assert_eq!(st.tiles[p].terrain, Terrain::Mass, "{:?} {p:?}", b.id);
                    assert_eq!(st.tiles[p].building, Some(b.id));
                }
                for k in b.span.iter().filter(|k| *k != StratumKind::Street && *k != StratumKind::Heights) {
                    let s = c.stratum(k).unwrap();
                    assert!(b.footprint.inset(1).iter().all(|p| s.tiles[p].terrain == Terrain::Mass), "{:?} on {k:?}", b.id);
                }
                if b.span.contains(StratumKind::Heights) {
                    let h = c.stratum(StratumKind::Heights).unwrap();
                    let roof_doors = b.footprint.iter().filter(|p| h.tiles[*p].terrain == Terrain::Door).count();
                    assert_eq!(roof_doors, 1, "{:?} roof door", b.id);
                    assert!(b.footprint.inset(1).iter().any(|p| h.tiles[p].terrain == Terrain::Rooftop));
                }
                assert!(!c.connectors.iter().any(|k| b.footprint.contains(k.pos) && !k.public), "{:?} keeps no shaft", b.id);
            } else {
                let comps = components(st.tiles.size(), |p| b.footprint.contains(p) && st.tiles[p].is_walkable());
                assert_eq!(comps.len(), 1, "{:?} interior split", b.id);
            }
        }
    }

    #[test]
    fn closed_buildings_get_a_back_door_where_another_side_faces_the_street() {
        let c = small(61, WaterMode::None, 6);
        let st = c.stratum(StratumKind::Street).unwrap();
        let mut two_sided = 0;
        for b in c.buildings.iter().filter(|b| b.kind.is_closed()) {
            let sides: std::collections::HashSet<u8> = b.footprint.border()
                .filter(|p| st.tiles[*p].terrain == Terrain::Door)
                .map(|p| if p.x == b.footprint.x { 0 } else if p.x == b.footprint.right() - 1 { 1 } else if p.y == b.footprint.y { 2 } else { 3 })
                .collect();
            let outdoor_sides: std::collections::HashSet<u8> = b.footprint.border()
                .filter(|p| !super::is_corner(b.footprint, *p))
                .filter(|p| st.tiles.get(super::outward(b.footprint, *p)).is_some_and(|t| t.is_outdoor_ground()))
                .map(|p| if p.x == b.footprint.x { 0 } else if p.x == b.footprint.right() - 1 { 1 } else if p.y == b.footprint.y { 2 } else { 3 })
                .collect();
            if outdoor_sides.len() >= 2 {
                assert!(sides.len() >= 2, "{:?} has {} outdoor sides but doors on {:?}", b.id, outdoor_sides.len(), sides);
                two_sided += 1;
            }
        }
        assert!(two_sided > 0);
    }

    #[test]
    fn bridge_and_basement_doors_face_what_they_open_onto() {
        let c = small(61, WaterMode::Bay, 7);
        for b in c.buildings.iter().filter(|b| b.kind.is_closed()) {
            for (k, ok) in [
                (StratumKind::Skyways, &[Terrain::Walkway, Terrain::Bridge][..]),
                (StratumKind::Undercity, &[Terrain::Tunnel, Terrain::Rail, Terrain::Grate][..]),
            ] {
                if !b.span.contains(k) { continue; }
                let s = c.stratum(k).unwrap();
                for p in b.footprint.border().filter(|p| s.tiles[*p].terrain == Terrain::Door) {
                    let o = super::outward(b.footprint, p);
                    assert!(s.tiles.get(o).is_some_and(|t| ok.contains(&t.terrain)), "{:?} {k:?} door at {p:?} faces {:?}", b.id, s.tiles.get(o).map(|t| t.terrain));
                }
            }
        }
        assert!(c.buildings.iter().any(|b| b.kind == BuildingKind::CorpTower && b.kind.is_closed()));
    }

    #[test]
    fn portals_list_every_door_of_a_closed_building() {
        let c = small(61, WaterMode::Bay, 7);
        let tall = c.buildings.iter().find(|b| b.kind.is_closed() && b.span.contains(StratumKind::Heights)).expect("a tall closed building");
        let portals = c.portals(tall.id);
        assert!(portals.iter().any(|(k, _)| *k == StratumKind::Street));
        assert!(portals.iter().any(|(k, _)| *k == StratumKind::Heights));
        for (k, p) in &portals {
            assert_eq!(c.tile(*k, *p).unwrap().terrain, Terrain::Door);
            assert!(c.walkable(*k, *p));
        }
        let open = c.buildings.iter().find(|b| !b.kind.is_closed()).unwrap();
        assert!(c.portals(open.id).is_empty());
    }
}
```

- [ ] **Step 2: Run them**

Run: `cargo test -p citygen interiors::`
Expected: compile errors on `is_closed`, `Terrain::Mass`, `portals`.

- [ ] **Step 3: Implement**

`tile.rs`: add `Mass` after `Window` in `Terrain` and in `ALL_TERRAIN` (count 26); `blocks_sight` includes `Mass`; `is_building_part` includes `Mass`; `is_walkable` unchanged (Mass is not listed). Add

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Furniture { Desk, Counter, Table, Stool, Shelf, Machine, Bed, Shrine, Locker }
```
and `Feature::Furniture(Furniture)`; `ALL_FEATURES` gains one entry (`Feature::Furniture(Furniture::Desk)`); the `is_walkable` feature list gains `Some(Feature::Furniture(_))`. Give `Terrain::Mass` a `label()` "building" and `Feature::Furniture` labels per piece if `Feature` has labels.

`world.rs`:
```rust
impl BuildingKind {
    /// Closed buildings are solid on the city maps and open as interiors.
    pub fn is_closed(self) -> bool {
        !matches!(self, BuildingKind::NoodleBar | BuildingKind::Market | BuildingKind::Shop | BuildingKind::Pawn | BuildingKind::Arcade | BuildingKind::Parking)
    }
}
impl City {
    /// A closed building's doors on every stratum: what its interior joins.
    pub fn portals(&self, b: BuildingId) -> Vec<(StratumKind, Pos)> {
        let Some(b) = self.building_opt(b) else { return Vec::new() };
        if !b.kind.is_closed() { return Vec::new(); }
        let mut out = Vec::new();
        for k in b.span.iter() {
            let tiles: Box<dyn Iterator<Item = Pos>> = if k == StratumKind::Heights { Box::new(b.footprint.iter()) } else { Box::new(b.footprint.border()) };
            for p in tiles {
                if self.tile(k, p).is_some_and(|t| t.terrain == Terrain::Door) { out.push((k, p)); }
            }
        }
        out
    }
}
```

`generate/interiors.rs` `furnish`: branch on `b.kind.is_closed()`:
```rust
if b.kind.is_closed() {
    for k in b.span.iter() {
        if k == StratumKind::Heights {
            continue; // the roof stays
        }
        for p in fp.inset(1).iter() {
            ctx.set_terrain(k, p, Terrain::Mass);
            ctx.set_feature(k, p, None);
        }
        match k {
            StratumKind::Street => {
                let main = Self::exterior_door(ctx, rng, k, fp, |t| matches!(t, Terrain::Road | Terrain::Sidewalk | Terrain::Alley | Terrain::Plaza), Some(Terrain::Sidewalk));
                Self::back_door(ctx, rng, fp, main);
                Self::windows(ctx, fp);
            }
            StratumKind::Skyways => { Self::exterior_door(ctx, rng, k, fp, |t| matches!(t, Terrain::Walkway | Terrain::Bridge), None); }
            StratumKind::Undercity => { Self::exterior_door(ctx, rng, k, fp, |t| matches!(t, Terrain::Tunnel | Terrain::Rail | Terrain::Grate), None); }
            _ => {}
        }
    }
    if b.span.contains(StratumKind::Heights) {
        Self::roof_door(ctx, &b);
    }
    return;
}
```
`exterior_door` must not turn the lobby tile into Floor for closed buildings: give it a `carve_lobby: bool` parameter (true for open buildings) and skip the `lobby` Floor write when false; return the door tile instead of the lobby for closed ones (change the return to `(door, lobby)`). `back_door(ctx, rng, fp, main_door)`: candidates = border tiles that are Wall, not corners, whose outward tile is outdoor ground, on a different side from `main_door` (side = which edge); pick one with the rng; set `Door`. `roof_door(ctx, b)`: the Heights tile at the main street door's position if it is Rooftop, else the Rooftop tile of the footprint nearest the centre; set `Terrain::Door`, feature None. Walls of the Heights footprint: the roof today has no Wall perimeter (verticals paints Rooftop over the whole footprint), fine.

The shaft: `if b.span.len() >= 2 && !b.kind.is_closed() { Self::shaft(ctx, &b); }`. Lobbies: only push to `ctx.scratch.lobbies` for open buildings (the dressing terminal follows automatically). Keep `partition`, `ensure_connected`, `shaft` for open buildings.

`connectivity.rs` `reachable_from_street`: after building `by_pos` for connectors, build `portal_links: HashMap<Pos, Vec<(StratumKind, Pos)>>`: for every closed building, every portal maps to all the building's portals. In the BFS, when popping `(k, p)`, also push every `(s, q)` in `portal_links[&p]` with `city.walkable(s, q)` and not yet reached (check that the popped `(k, p)` is itself one of that building's portals on `k`: key the map by `(StratumKind, Pos)`). `validate.rs` reachability uses `reachable_from_street` already; nothing else changes there. The dressing test `count(Terminal) >= buildings.len() / 2` becomes `>= open buildings / 2 + street terminals > 0`: compute `open = c.buildings.iter().filter(|b| !b.kind.is_closed()).count()` and assert `>= open / 2`.

`strata-zero/render/tileset.rs`: `Terrain::Mass => pick(&g.mass, variant as u64)` in `glyph_for`; colours `Terrain::Mass => (ts.wall.scale(0.45), bg0)`; `connects` unchanged (Mass does not join walls). `ui/panel.rs`: when `t.terrain == Terrain::Mass` the building line reads `"{name} · closed"`.

- [ ] **Step 4: Run the citygen suite, then the workspace**

Run: `cargo test -p citygen`
Expected: all green, including `verticals::skyways_and_heights_follow_spans` (its Heights assertion allows Rooftop | Lift | Stair: add `Terrain::Door`), `connectivity` tests, `validate` tests, the seeds sweep is ignored by default. Then `cargo test --workspace`: the app's tests still pass (sim's `Places::derive` reads Door tiles; the fixture now has more doors). If `sim::places::tests::doors_sit_on_footprint_edges_with_the_street_outside` or the people tests fail on counts, read the failure and adjust the expectation, not the code.

- [ ] **Step 5: Commit**

`git add -A crates && git commit -m "feat(citygen): closed buildings are mass behind their facades, with doors on every stratum they span"`

---

### Task 2: Floor plans and the stacked floors

**Files:**
- Create: `crates/citygen/src/lighting.rs`, `crates/citygen/src/interior/mod.rs`, `crates/citygen/src/interior/plan.rs`, `crates/citygen/src/interior/floor.rs`, `crates/citygen/src/interior/layouts.rs`
- Modify: `crates/citygen/src/lib.rs` (`pub mod interior; pub mod lighting; pub use interior::*;`), `crates/citygen/src/world.rs` (`Place`), `crates/citygen/src/generate/dressing.rs` (use `lighting::compute`)

**Interfaces:**
- Produces:
```rust
pub enum Place { Stratum(StratumKind), Floor { building: BuildingId, level: i32 } }
impl Place { pub fn stratum(self) -> Option<StratumKind>; pub fn is_street(self) -> bool; }
pub fn lighting::compute(tiles: &Grid<Tile>, base: u8, emission: impl Fn(&Tile) -> Option<(i32, f32)>) -> Grid<u8>;
pub struct FloorPlan { building, kind, size, lowest, top, bridge, roof, lift: Option<Pos>, stair: Pos }
impl FloorPlan { pub fn new(city: &City, b: BuildingId) -> Option<FloorPlan>; pub fn levels(&self) -> impl Iterator<Item = i32>; pub fn label(level: i32) -> String; pub fn floor_type(&self, level: i32) -> FloorType; pub fn work_floors(&self) -> Vec<i32>; }
pub enum FloorType { Lobby, Corridor, Suite, Bar, Vip, Clinic, Temple, Hall, Factory, Garage, Basement }
pub struct Floor { level, tiles: Grid<Tile>, light: Grid<u8>, rooms: Vec<Room>, exits: Vec<Exit>, stations: Vec<Station> }
impl Floor { pub fn exit_at(&self, p: Pos) -> Option<&Exit>; pub fn room_at(&self, p: Pos) -> Option<&Room>; pub fn door_of(&self, p: Pos) -> Option<&Room> /* the room whose door is p */; }
pub fn generate_floor(city: &City, plan: &FloorPlan, level: i32) -> Floor;
pub fn validate_floor(plan: &FloorPlan, floor: &Floor) -> Result<(), String>;
pub struct Exit { at: Pos, stratum: StratumKind, landing: Pos, door: Pos }
pub struct Room { rect: Rect, kind: RoomKind, door: Option<Pos>, private: bool, number: u16 }
pub struct Station { pos: Pos, role: Role }
```
  This task implements `FloorType::{Lobby, Corridor, Suite, Basement}` fully and returns an empty hall with the lift, stair and exits for the other types (Task 3 fills them).

- [ ] **Step 1: Failing tests in `interior/mod.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildingKind, GenParams, Generator, HeightClass, SizePreset, StratumKind, Terrain, Feature};
    use crate::pathing::components;

    fn city() -> crate::City {
        Generator::run(GenParams { seed: 6, size: SizePreset::Custom { w: 120, h: 60 }, districts: 8, ..Default::default() }).unwrap()
    }

    #[test]
    fn plans_follow_height_class_span_and_kind() {
        let c = city();
        let mut seen_tall = false;
        for b in c.buildings.iter().filter(|b| b.kind.is_closed()) {
            let plan = FloorPlan::new(&c, b.id).expect("a plan for a closed building");
            assert_eq!(plan.size.w as i32, (b.footprint.w * 3).max(12));
            assert_eq!(plan.size.h as i32, (b.footprint.h * 3).max(9));
            assert_eq!(plan.lowest, if b.span.contains(StratumKind::Undercity) { -1 } else { 0 });
            match b.span.hi {
                StratumKind::Street => assert!(plan.top <= 2),
                StratumKind::Skyways => assert!((3..=9).contains(&plan.top)),
                StratumKind::Heights => { assert!(plan.top >= 8); assert!(plan.roof); seen_tall = true; }
                _ => unreachable!(),
            }
            let has_bridge_door = c.portals(b.id).iter().any(|(k, _)| *k == StratumKind::Skyways);
            assert_eq!(plan.bridge.is_some(), has_bridge_door);
            if let Some(f) = plan.bridge { assert!(f >= 1 && f <= plan.top); }
            assert_eq!(plan.lift.is_some(), plan.top - plan.lowest + 1 >= 3);
            assert_eq!(plan.levels().count() as i32, plan.top - plan.lowest + 1);
        }
        assert!(seen_tall);
        let open = c.buildings.iter().find(|b| !b.kind.is_closed()).unwrap();
        assert!(FloorPlan::new(&c, open.id).is_none());
        assert_eq!(FloorPlan::label(-1), "B1");
        assert_eq!(FloorPlan::label(0), "L");
        assert_eq!(FloorPlan::label(12), "12F");
    }

    #[test]
    fn a_tower_lobby_has_a_counter_lifts_stairs_and_its_street_exits() {
        let c = city();
        let b = c.buildings.iter().find(|b| b.kind == BuildingKind::CorpTower).unwrap();
        let plan = FloorPlan::new(&c, b.id).unwrap();
        let lobby = generate_floor(&c, &plan, 0);
        validate_floor(&plan, &lobby).unwrap();
        assert!(lobby.rooms.iter().any(|r| r.kind == RoomKind::Lobby));
        assert!(lobby.stations.iter().any(|s| s.role == Role::Reception));
        assert!(lobby.tiles.iter().any(|(_, t)| t.feature == Some(Feature::Furniture(Furniture::Counter))));
        let lifts = lobby.tiles.count(|t| t.terrain == Terrain::Lift);
        assert!(lifts >= 2, "a lift bank");
        assert_eq!(lobby.tiles.count(|t| t.terrain == Terrain::Stair), 1);
        let street_doors: Vec<_> = c.portals(b.id).into_iter().filter(|(k, _)| *k == StratumKind::Street).collect();
        assert_eq!(lobby.exits.len(), street_doors.len());
        for e in &lobby.exits {
            assert!(street_doors.iter().any(|(_, d)| *d == e.door));
            assert_eq!(lobby.tiles[e.at].terrain, Terrain::Door);
            assert!(c.tile(StratumKind::Street, e.landing).unwrap().is_outdoor_ground());
            // The door is where the street door is, scaled.
            let fp = b.footprint;
            let dx = (e.door.x - fp.x) * 3 + 1; let dy = (e.door.y - fp.y) * 3 + 1;
            assert!(e.at.x == dx.clamp(0, plan.size.w as i32 - 1) || e.at.y == dy.clamp(0, plan.size.h as i32 - 1));
        }
        // Light is lit: ceiling lamps.
        assert!(lobby.light.as_slice().iter().any(|l| *l > 100));
    }

    #[test]
    fn corridor_floors_have_locked_numbered_flats_and_the_bridge_floor_its_door() {
        let c = city();
        let b = c.buildings.iter().find(|b| b.kind == BuildingKind::Megablock && b.span.contains(StratumKind::Skyways)).unwrap();
        let plan = FloorPlan::new(&c, b.id).unwrap();
        let f = generate_floor(&c, &plan, 2);
        validate_floor(&plan, &f).unwrap();
        let flats: Vec<&Room> = f.rooms.iter().filter(|r| r.kind == RoomKind::Flat).collect();
        assert!(flats.len() >= 2);
        for r in &flats {
            assert!(r.private && r.door.is_some());
            assert_eq!(f.tiles[r.door.unwrap()].terrain, Terrain::Door);
            assert!(r.number >= 201 && r.number < 300, "{}", r.number);
        }
        assert!(f.rooms.iter().any(|r| r.kind == RoomKind::Corridor));
        assert!(f.exits.is_empty());
        if let Some(bf) = plan.bridge {
            let bridge = generate_floor(&c, &plan, bf);
            validate_floor(&plan, &bridge).unwrap();
            assert_eq!(bridge.exits.len(), 1);
            assert_eq!(bridge.exits[0].stratum, StratumKind::Skyways);
            assert!(matches!(c.tile(StratumKind::Skyways, bridge.exits[0].landing).unwrap().terrain, Terrain::Walkway | Terrain::Bridge));
        }
    }

    #[test]
    fn the_top_floor_opens_onto_the_roof_and_the_basement_onto_the_tunnel() {
        let c = city();
        let b = c.buildings.iter().find(|b| b.kind.is_closed() && b.span.contains(StratumKind::Heights) && b.span.contains(StratumKind::Undercity)).expect("a tower with a basement");
        let plan = FloorPlan::new(&c, b.id).unwrap();
        let top = generate_floor(&c, &plan, plan.top);
        validate_floor(&plan, &top).unwrap();
        let roof = top.exits.iter().find(|e| e.stratum == StratumKind::Heights).expect("roof door");
        assert_eq!(c.tile(StratumKind::Heights, roof.landing).unwrap().terrain, Terrain::Door);
        assert_eq!(top.tiles[roof.at].terrain, Terrain::Door);
        let cellar = generate_floor(&c, &plan, -1);
        validate_floor(&plan, &cellar).unwrap();
        if c.portals(b.id).iter().any(|(k, _)| *k == StratumKind::Undercity) {
            let hatch = cellar.exits.iter().find(|e| e.stratum == StratumKind::Undercity).expect("basement door");
            assert!(c.walkable(StratumKind::Undercity, hatch.landing));
        }
        assert!(cellar.rooms.iter().any(|r| r.kind == RoomKind::Storage));
    }

    #[test]
    fn every_floor_of_the_fixture_validates_and_repeats() {
        let c = city();
        for b in c.buildings.iter().filter(|b| b.kind.is_closed()) {
            let plan = FloorPlan::new(&c, b.id).unwrap();
            for level in [plan.lowest, 0, plan.top, plan.bridge.unwrap_or(0)] {
                let a = generate_floor(&c, &plan, level);
                validate_floor(&plan, &a).unwrap_or_else(|e| panic!("{:?} {level}: {e}", b.id));
                let again = generate_floor(&c, &plan, level);
                assert_eq!(a.tiles.as_slice(), again.tiles.as_slice());
                let walk = components(a.tiles.size(), |p| a.tiles[p].is_walkable());
                assert_eq!(walk.len(), 1, "{:?} {level} is one piece", b.id);
            }
        }
    }

    #[test]
    fn the_smallest_footprint_still_fits_a_lobby() {
        let c = city();
        let b = c.buildings.iter().filter(|b| b.kind.is_closed()).min_by_key(|b| b.footprint.area()).unwrap();
        let plan = FloorPlan::new(&c, b.id).unwrap();
        let f = generate_floor(&c, &plan, 0);
        validate_floor(&plan, &f).unwrap();
        assert!(f.exits.len() >= 1);
        for e in &f.exits {
            let step = f.doorstep(e);
            assert!(f.tiles[step].is_walkable());
        }
    }
}
```

- [ ] **Step 2: Run them** — `cargo test -p citygen interior::` — Expected: compile errors (module missing).

- [ ] **Step 3: Implement**

`world.rs`:
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Place { Stratum(StratumKind), Floor { building: BuildingId, level: i32 } }
impl Place {
    pub fn stratum(self) -> Option<StratumKind> { match self { Place::Stratum(k) => Some(k), _ => None } }
    pub fn is_street(self) -> bool { self == Place::Stratum(StratumKind::Street) }
}
```

`lighting.rs`: move the body of `DressingStage::lights` into `pub fn compute(tiles: &Grid<Tile>, base: u8, emission: impl Fn(&Tile) -> Option<(i32, f32)>) -> Grid<u8>`; the stage calls it with `|t| emission(t.terrain, t.feature, k)`.

`interior/plan.rs`: `FloorPlan::new`: `None` for open kinds; `size = Size { w: (fp.w*3).max(12), h: (fp.h*3).max(9) }`; rng = `ChaCha8Rng::seed_from_u64(hash64(city.seed, b.id.index() as u64 ^ 0x1F00))`; floors per class/kind as the spec's table (`b.span.hi` gives the class); `lowest = -1` if `span.lo == Undercity`; `bridge = portals has a Skyways door → Some(3.min(top))`; `roof = span.hi == Heights`; `lift = (floors >= 3).then(|| Pos::new(w/2 - 1, h - 3))` (the bank is `lift` and `lift + (1,0)`); `stair = Pos::new(w - 3, h - 3)` (for 2+ floors; a 1-floor building has a stair tile nowhere: still set, unused — validation skips the stair when `top == lowest`). `floor_type(level)`: Basement if level == -1; for CorpTower/Office/Megablock/Apartment/Hotel: Lobby at 0, Suite at top when roof && kind is CorpTower|Office, else Corridor; Bar: Bar; Club: Bar at 0, Vip at 1; Clinic/Temple/Hall(Warehouse,Depot)/Factory/Garage: their own. `work_floors()`: the Corridor and Suite levels for offices/towers, `[0]` for everything else. `label(level)`: `-1 → "B1"`, `0 → "L"`, `n → "{n}F"`.

`interior/floor.rs`: the structs; `Floor::doorstep(&self, e: &Exit) -> Pos` (the inward neighbour of `e.at`); `exit_at`, `room_at`, `door_of`.

`interior/layouts.rs`: a `Builder { tiles: Grid<Tile>, rooms, stations, rng, plan, level }` with helpers: `fill(rect, terrain)`, `wall_rect(rect)`, `door(pos)`, `furnish(pos, Furniture)` (only on a Floor tile that is not a doorstep, lift, stair or exit), `station(pos, role)`, `corridor_floor(builder, kind)`, `lobby(builder)`, `suite(builder)`, `basement(builder)`, `hall(builder)` (empty: Task 3). Common frame: outer `Wall` perimeter with `Window` every other tile on sides that face outdoors on the city map (the street sides), the rest Floor; then the layout; then `lift bank` (two `Lift` tiles at `plan.lift`, Floor around them) and `stair` (`Terrain::Stair`), then exits: for each portal of the floor's stratum (street doors for level 0, the bridge door for `plan.bridge`, the roof door for `plan.top` when `plan.roof`, the basement door for level -1): map the city door `d` to the border tile `at` (edge from `d`'s side, offset `((d.x-fp.x)*3+1, (d.y-fp.y)*3+1)` clamped to the interior of the edge), set `Door`, ensure the doorstep and a straight carved path of Floor from the doorstep toward the lift (or the room centre) so the exit joins the floor; `landing = outward(fp, d)` for street/bridge/basement; for the roof door `landing = d` itself (the Heights door tile) and `at` is the tile beside the stairwell. Lamps: `Feature::Lamp` on Floor tiles at `(x % 6 == 3, y % 6 == 3)`. Light via `lighting::compute(&tiles, 90, |t| match t.feature { Some(Feature::Lamp) => Some((6, 120.0)), _ => None })`. Finally `ensure_connected` (carve doors through inner walls between walkable components, like the generator's).

Corridor floor: the corridor is the two rows at `h/2 - 1 ..= h/2` (long axis horizontal; transpose when `h > w`), rooms every `rng 5..=8` tiles on both sides, each a `Rect` from the outer wall to the corridor wall with a `Door` on the corridor wall at the room's middle; room kinds per building kind (Office: alternate `Office` private / `OpenPlan` public with desk rows every 3 columns, Desk stations beside each desk; Megablock/Apartment: `Flat` private numbered `level*100 + index`, with a `Bed`; every third floor the last room is a `Laundry` public with Machines; Hotel: `HotelRoom` private numbered, with a Bed). The lift bank and stair sit on the corridor side (plan.lift is placed on the corridor row; if the plan's lift is not on the corridor, carve Floor from it to the corridor).

Lobby: an open hall; the reception counter: three `Counter` tiles two rows inward from the main door's edge, centred on the door, with a Reception station behind (the tile farther from the door); the lift bank and stair; the back door exit.

Suite: open plan desks on one half and one locked `Suite` office (a Rect of a third of the floor) with a Desk; the roof exit's `at` beside the stair.

Basement: `Storage` hall with `Shelf` rows every 4 columns (Shelf stations), the lift bank, stair and the basement exit.

`validate_floor`: as the spec's list; walkable one component including lift/stair/doorsteps; exits are Door tiles on the border with walkable doorsteps; private rooms have a door; no furniture on doorsteps, lifts, stairs, exits; room rects inside the floor.

- [ ] **Step 4: Run** `cargo test -p citygen` — Expected: green. Then the workspace.

- [ ] **Step 5: Commit** `feat(citygen): floor plans and the stacked floors`

---

### Task 3: The single-floor kinds

**Files:** `crates/citygen/src/interior/layouts.rs`, tests in `interior/mod.rs`.

**Interfaces:** Consumes Task 2's `Builder`. Produces nothing new; `FloorType::{Bar, Vip, Clinic, Temple, Hall, Factory, Garage}` are laid out and stationed.

- [ ] **Step 1: Failing tests**

```rust
    #[test]
    fn bars_clubs_clinics_temples_halls_factories_and_garages_have_their_rooms() {
        let c = city();
        let expect = |kind: BuildingKind, rooms: &[RoomKind], roles: &[Role]| {
            let b = c.buildings.iter().find(|b| b.kind == kind).unwrap_or_else(|| panic!("a {kind:?}"));
            let plan = FloorPlan::new(&c, b.id).unwrap();
            let f = generate_floor(&c, &plan, 0);
            validate_floor(&plan, &f).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
            for r in rooms { assert!(f.rooms.iter().any(|x| x.kind == *r), "{kind:?} lacks {r:?}"); }
            for r in roles { assert!(f.stations.iter().any(|s| s.role == *r), "{kind:?} lacks {r:?}"); }
            assert!(f.rooms.iter().any(|r| r.private), "{kind:?} has a locked room");
            f
        };
        expect(BuildingKind::Bar, &[RoomKind::Bar, RoomKind::Toilets, RoomKind::BackRoom], &[Role::Bar, Role::Table]);
        let club = expect(BuildingKind::Club, &[RoomKind::Bar, RoomKind::Stage], &[Role::Bar, Role::Table]);
        let plan = FloorPlan::new(&c, c.buildings.iter().find(|b| b.kind == BuildingKind::Club).unwrap().id).unwrap();
        assert_eq!(plan.top, 1);
        let vip = generate_floor(&c, &plan, 1);
        validate_floor(&plan, &vip).unwrap();
        assert!(vip.rooms.iter().any(|r| r.kind == RoomKind::Vip && !r.private));
        let _ = club;
        expect(BuildingKind::Clinic, &[RoomKind::Waiting, RoomKind::Surgery], &[Role::Surgery]);
        expect(BuildingKind::Temple, &[RoomKind::Hall, RoomKind::Shrine], &[Role::Shrine]);
        expect(BuildingKind::Warehouse, &[RoomKind::Hall], &[Role::Shelf]);
        expect(BuildingKind::Factory, &[RoomKind::Hall, RoomKind::Lockers], &[Role::Machine]);
        expect(BuildingKind::Garage, &[RoomKind::Bays], &[Role::Bay]);
    }
```
If the fixture lacks a kind (Depot, Garage), generate a second city (`seed: 8, Medium`) in the test for the missing ones.

- [ ] **Step 2: Run** — Expected: the empty-hall fallback has no such rooms: assertions fail.

- [ ] **Step 3: Implement** the layouts: Bar (counter along the wall opposite the main door, 60% of its length, Bar stations behind; tables 2×1 `Table` with `Stool`s around and a Table station beside each, spaced ≥ 3; `Toilets` 3×3 locked? no: public, `BackRoom` private 4×3 in a corner; Club: a `Stage` 5×3 room (open rect marked, no walls) at the far end), Vip (a corridor floor with public `Vip` rooms with a Table each), Clinic (`Waiting` with stools in rows, `Surgery` with a `Bed` and the Surgery station, `BackRoom` private), Temple (`Hall` the whole floor with the `Shrine` furniture at the far wall and the station before it, a private side room), Hall (shelf rows the length of the hall every 4 columns, Shelf stations at row ends; Warehouse/Depot the back door is the dock), Factory (machine rows, Machine stations beside each, `Lockers` room with Lockers, a locked office with a Desk), Garage (`Bays`: 3-wide bays separated by short walls along one side, Bay stations; a locked office).

- [ ] **Step 4: Run** `cargo test -p citygen` and the workspace. Expected: green.

- [ ] **Step 5: Commit** `feat(citygen): bars, clubs, clinics, temples, halls, factories and garages`

---

### Task 4: One renderer, two kinds of map

**Files:**
- Create: `crates/strata-zero/src/render/map.rs`
- Modify: `crates/game/src/action.rs` (`At { place: Place, pos }`), `crates/game/src/ground.rs` (keys `(Place, Pos)`), `crates/strata-zero/src/render/{mod,viewport,items,fx,overlay,weather,people,memmap,minimap,tileset}.rs`, `walker/mod.rs`, `ui/{hud,panel,sidebar}.rs`, `screens/{explore,popup,console}.rs`, `app.rs`, `actions.rs`

**Interfaces:**
- Produces:
```rust
pub trait TileMap {
    fn size(&self) -> Size;
    fn tile(&self, p: Pos) -> Option<&Tile>;
    fn light(&self, p: Pos) -> u8;
    fn theme(&self, p: Pos) -> Option<Theme>;
    fn kind(&self, p: Pos) -> Option<BuildingKind>;
    fn connector(&self, p: Pos) -> Option<ConnDir>;
    fn walkable(&self, p: Pos) -> bool { self.tile(p).is_some_and(|t| t.is_walkable()) }
}
pub struct StratumMap<'a> { pub city: &'a City, pub stratum: StratumKind }
pub struct FloorMap<'a> { pub city: &'a City, pub plan: &'a FloorPlan, pub floor: &'a Floor }
pub enum MapRef<'a> { Stratum(StratumMap<'a>), Floor(FloorMap<'a>) }   // impl TileMap by delegation
impl<'a> MapRef<'a> { pub fn place(&self) -> Place; }
```
  `MapView { map: &dyn TileMap, place: Place, origin, tick, style, vis, overlays, cursor }`; `Overlay::draw(&self, map: &dyn TileMap, place: Place, origin, area, tick, buf, style)`; `MemoryMap { map, vis, player, style }`; `Minimap { map, view, player, style }`; `Player { pos, place: Place, body: Option<(Place, Pos)>, memory: HashMap<Place, Grid<Visibility>> }` with `try_move(&mut self, map: &dyn TileMap, dir) -> bool`, `update_fov(&mut self, map: &dyn TileMap, bonus)`, `reveal(map, radius)`, `vis(&self) -> &Grid<Visibility>` (the current place's; an unknown place gets an Unseen grid of the map's size on first `update_fov`), `use_connector(city, up)` (city strata only), `jack_in/jack_out` (body is `(Place, Pos)`); `FxInstance.place: Place`; `game::At { place, pos }`; `hud::location(city, interiors: &Interiors, place, pos) -> String`; `inspector_lines(city, map: &dyn TileMap, place, pos, node, items, content, person)`.
- `crate::interiors::Interiors` (new, in this task as the cache only): `HashMap<BuildingId, (FloorPlan, HashMap<i32, Floor>)>` with `plan(&mut self, city, b) -> Option<&FloorPlan>`, `ensure(&mut self, city, b, level) -> Option<&Floor>`, `floor(&self, b, level) -> Option<(&FloorPlan, &Floor)>`; `AppState.interiors: Interiors`; `MapRef::for_place(st: &AppState, place) -> Option<MapRef>`.

- [ ] **Step 1: Failing tests** — in `render/map.rs`:

```rust
    #[test]
    fn a_floor_renders_like_a_stratum_and_the_walker_walks_it() {
        let c = city(); // the fixture
        let b = c.buildings.iter().find(|b| b.kind == BuildingKind::CorpTower).unwrap();
        let mut interiors = Interiors::default();
        let (plan, floor) = { interiors.ensure(&c, b.id, 0).unwrap(); interiors.floor(b.id, 0).unwrap() };
        let map = MapRef::Floor(FloorMap { city: &c, plan, floor });
        assert_eq!(map.size(), plan.size);
        assert!(map.light(floor.stations[0].pos) > 0);
        assert_eq!(map.theme(Pos::new(1, 1)), Some(c.district(b.district).theme));
        let lift = floor.tiles.iter().find(|(_, t)| t.terrain == Terrain::Lift).unwrap().0;
        assert!(map.connector(lift).is_some_and(|d| d.kind == ConnectorKind::Lift && d.up));
        let s = StylePack::load().unwrap();
        let mut t = Terminal::new(TestBackend::new(80, 30)).unwrap();
        t.draw(|f| MapView { map: &map, place: Place::Floor { building: b.id, level: 0 }, origin: Pos::new(0, 0), tick: 1, style: &s, vis: None, overlays: &[], cursor: None }.render(f.area(), f.buffer_mut())).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains('▯') && text.contains('≣') || text.contains('▭'), "doors and furniture drawn: {text}");
        let mut p = Player::spawn(&c, 1);
        p.place = Place::Floor { building: b.id, level: 0 };
        p.pos = floor.doorstep(&floor.exits[0]);
        p.update_fov(&map, 0);
        assert_eq!(p.vis().get(p.pos), Some(&Visibility::Visible));
        let before = p.pos;
        let moved = citygen::Dir::ALL.iter().any(|d| p.try_move(&map, *d));
        assert!(moved && p.pos != before);
        // A furniture tile is not walkable; mass is not walkable on the street.
        let desk = floor.tiles.iter().find(|(_, t)| matches!(t.feature, Some(Feature::Furniture(_)))).unwrap().0;
        assert!(!map.walkable(desk));
        let street = MapRef::Stratum(StratumMap { city: &c, stratum: StratumKind::Street });
        let mass = b.footprint.inset(1).iter().next().unwrap();
        assert!(!street.walkable(mass));
    }
```
and in `game/src/ground.rs`: `piles are kept per place` (add on `Place::Floor { .. }` and on `Place::Stratum(Street)` at the same pos, read back separately).

- [ ] **Step 2: Run** — compile errors.

- [ ] **Step 3: Implement** the trait, the two maps, `MapRef`, then thread `TileMap`/`Place` through every file listed, replacing `(city, stratum)` pairs. `FloorMap::theme` is the building's district theme everywhere; `kind` the building's kind; `connector` reads Lift/Stair tiles against the plan (`up` when `level < plan.top`, `down` when `level > plan.lowest`); `light` from `floor.light`. The tileset: furniture glyphs `Desk '▭'`, `Counter '═'` (no mask), `Table '○'`, `Stool '·'`, `Shelf '≣'`, `Machine '⚙'`, `Bed '▤'`, `Shrine '♦'`, `Locker '▮'`, in `ts.accent2` for counter/shrine and `ts.wall.scale(1.2)` for the rest; `Terrain::Lift` and `Stair` draw as today with the connector arrows. `StratumMap::connector` builds the `ConnDir` from `city.connectors_at` exactly as `MapView` does today. Keep `PeopleOverlay` drawing only when `place.is_street()`. The explore screen: `self.place: Place` replaces `self.stratum`; Explore-mode `[`/`]` browse strata only when `place` is a stratum (on a floor they do nothing); `MapRef::for_place` in `draw` and in `update` where the walker moves. The sim blocker: `place.is_street()`. Jacking: `Player::jack_in` keeps working from the Street. `hud::location`: `Place::Floor` → `"{building name} · {FloorPlan::label(level)}"`. The inspector: on a floor, lines `room` (kind label, `locked` when private, the number) instead of district/theme/wealth. Existing explore tests use `s.stratum`: rename to `s.place` with `Place::Stratum(..)`.

- [ ] **Step 4: Run** `cargo test --workspace` — Expected: green (the refactor must keep every existing test passing; where a test asserts `p.stratum`, it now asserts `p.place`).

- [ ] **Step 5: Commit** `refactor(app): one renderer over a stratum or a floor, and places everywhere`

---

### Task 5: Doors, lifts and stairs

**Files:** `crates/strata-zero/src/interiors.rs`, `screens/explore.rs`, `screens/popup.rs`, `ui/hud.rs`, `ui/panel.rs`, `walker/mod.rs`.

**Interfaces:**
- Consumes Task 4.
- Produces: `interiors::enter(st, building, exit_door: (StratumKind, Pos)) -> Result<String, String>` (puts the player on the doorstep of the exit whose `door` is the tile, generating the floor), `interiors::leave(st, exit: &Exit) -> String`, `interiors::ride(st, building, level) -> Result<String, String>` (to a floor's lift bank), `interiors::climb(st, up: bool) -> Result<String, String>` (stairs); `WorldAct::Lift(i32)` in `popup.rs`; the FLOORS popup built by `MapScreen::floors_popup`.

- [ ] **Step 1: Failing tests in `screens/explore.rs`**

```rust
    fn walk_to(st: &mut AppState, s: &mut MapScreen, place: Place, pos: Pos) { let p = st.player.as_mut().unwrap(); p.place = place; p.pos = pos; s.place = place; s.update(st, &[]); }

    #[test]
    fn a_street_door_enters_the_lobby_and_its_door_leaves_again() {
        let mut st = state(); let mut s = MapScreen::explore(0); walking(&mut st, &mut s);
        let city = st.city.as_ref().unwrap();
        let b = city.buildings.iter().find(|b| b.kind == BuildingKind::CorpTower).unwrap();
        let (_, door) = city.portals(b.id).into_iter().find(|(k, _)| *k == StratumKind::Street).unwrap();
        let outside = door.neighbors4().find(|n| !b.footprint.contains(*n) && city.walkable(StratumKind::Street, *n)).unwrap();
        walk_to(&mut st, &mut s, Place::Stratum(StratumKind::Street), outside);
        let dir = dir_between(outside, door);
        s.update(&mut st, &[key(dir)]);
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.place, Place::Floor { building: b.id, level: 0 });
        let (plan, lobby) = st.interiors.floor(b.id, 0).unwrap();
        let exit = lobby.exits.iter().find(|e| e.door == door).unwrap();
        assert_eq!(p.pos, lobby.doorstep(exit));
        assert!(last(&st).unwrap().contains(&b.name.clone().unwrap()));
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.contains(&format!("· {}", FloorPlan::label(0))), "{text}");
        // Back out through the door.
        let back = dir_between(p.pos, exit.at);
        s.update(&mut st, &[key(back)]);
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.place, Place::Stratum(StratumKind::Street));
        assert_eq!(p.pos, exit.landing);
        let _ = plan;
    }

    #[test]
    fn the_lift_lists_floors_and_rides_and_stairs_climb() {
        // enter the tower as above, stand on the lift, press '>' → FLOORS popup lists "L", "2F", the top with "roof"; pick the top → place.level == plan.top, pos == plan.lift; stand on the stair, '<' → level top-1; a lift ride to a floor never generated before works (Review Focus 4).
    }

    #[test]
    fn roof_bridge_and_basement_doors_enter_and_leave_on_their_strata() {
        // for each exit kind present on a tall tower with a basement in the fixture: stand on the stratum tile beside the door, step in, assert the floor and doorstep, step out, assert the stratum and landing; sight memory for the stratum is kept (the tile stepped from stays Remembered).
    }

    #[test]
    fn jacking_in_inside_finds_no_terminal_and_the_body_place_round_trips() {
        // inside: space's rows have no "jack in"; `enter` warns "no terminal within reach". Outside beside a terminal: jack in, body place is Street, jack out returns.
    }

    #[test]
    fn locked_doors_stay_shut_and_say_so() {
        // on a corridor floor: step onto a private room's door → position unchanged, last log contains "locked".
    }
```
Write the four sketched tests out in full in the same style as the first.

- [ ] **Step 2: Run** — failures: the player walks onto the door tile and stays on the street.

- [ ] **Step 3: Implement.** In `handle_walk`'s move arm, before `try_move`: if the player is on a stratum and the target tile is a `Door` with `building = Some(b)` where `b.kind.is_closed()` and the player is outside the footprint (or on Heights on the roof: the roof door tile) → `interiors::enter`. If the player is on a floor and the target is an exit's `at` → `interiors::leave`. If the target is a private room's door (`floor.door_of(target).is_some_and(|r| r.private)`) → warn `"locked · {owner}"` (owner from Task 6; until then `"locked"`). `>`/`<` on a floor: on a `Lift` tile open the FLOORS popup (rows: every level, label, marks `bridge`/`roof`/`here`), on a `Stair` tile `climb`; `space` on a lift adds the same rows to HERE. The popup's `WorldAct::Lift(level)` calls `interiors::ride`. The camera snaps on every map change; `st.repaint = true`. `hud::location` reads the building and label; the inspector shows the room.

- [ ] **Step 4: Run** `cargo test --workspace` — green.

- [ ] **Step 5: Commit** `feat(app): doors open interiors, lifts list floors, stairs climb`

---

### Task 6: People inside

**Files:** `crates/sim/src/world.rs` (`inside`), `crates/strata-zero/src/interiors.rs` (placement), `render/people.rs` (`InteriorPeople`), `screens/explore.rs`.

**Interfaces:**
- Produces: `sim::Occupant { id: PersonId, role: OccupantRole::{Working, Living, Guest, Visiting} }`, `World::inside(&self, b: BuildingId) -> Vec<Occupant>` (sorted by id); `interiors::Placement::new(plan, floor, occupants: &[Occupant], world: &World) -> Placement { at: Vec<(Pos, PersonId)>, owners: HashMap<Pos /*room door*/, PersonId> }` deterministic; `Placement::person_at(pos)`, `Placement::owner_of(door)`; `InteriorPeople { placement, world, vis }` overlay; `MapScreen` keeps `placement: Option<Placement>` rebuilt when the player's place changes or a minute passes.

- [ ] **Step 1: Failing tests** — sim: `inside_lists_workers_residents_guests_and_visitors` (at noon a tower lists its drones as Working; at 03:00 a megablock lists Living); app: `occupants_stand_at_their_stations_and_space_talks_to_them` (enter a bar at 23:00: the barkeep draws as `@` at a Bar station; `person_at` finds them; walking into them does not move the player and does not swap; `space` lists `talk to NAME`; the card's doing reads "behind the bar"), `a_locked_flat_names_its_resident` (a megablock corridor at 03:00: stepping onto a flat door whose owner is home warns "locked · NAME's flat").

- [ ] **Step 2: Run** — failures.

- [ ] **Step 3: Implement.** `World::inside`: people whose whereabouts is `Inside(b)`: Working when `work == Building(b)`, Guest when in a date at that hotel, Living when `home == Some(b)`, else Visiting. Placement: workers → stations by profession (`barkeep` → Bar, `corp_drone` → Reception for the first on a lobby then Desk, `line_worker` → Machine, `ripperdoc` → Surgery, `monk` → Shrine, others → Table); the floor for a worker is `plan.work_floors()[hash(id) % n]`; residents → `Flat` rooms on floor `hash(id) % corridor floors`, in room order; guests → HotelRooms; visitors → Table stations. Only occupants whose floor is the drawn one are placed; the extras stand on free corridor/hall floor tiles nearest the lift. Owners: the room's door → the resident/guest. The overlay draws `@` with tints, in sight only. `person_at` inside comes from the placement; bump → blocked silently (no swap); `space` lists adjacent with `card_lines`.

- [ ] **Step 4: Run** the workspace — green.

- [ ] **Step 5: Commit** `feat: the people inside, at their stations`

---

### Task 7: Docs, smoke, screenshots

- [ ] **Step 1: README**: the opening paragraph on buildings (closed and open), Keys additions (doors, `>` `<` on lifts and stairs, FLOORS popup, locked doors), the Layout line for `interior`. A dev console `tp <x> <y>` (teleport on the current place) with a test, to drive screenshots.
- [ ] **Step 2: Smoke** `drive12.py` in the scratchpad: VEX in, `tp` beside a tower door (looked up from `gen --json`), walk in, expect the floor label in the header, `>` on the lift (walk to it with `tp`), expect "FLOORS", pick the top, expect "F", walk to the roof door, expect "Heights" in the header.
- [ ] **Step 3: Screenshots** of a tower lobby, a megablock corridor at 03:00 and a club floor at 23:00; send them.
- [ ] **Step 4: Commit** `docs: interiors`.

---

## Self-review

Spec coverage: outside (Task 1), model and generation (2, 3), moving (5), renderer (4), people (6), tests and docs (7). Review Focus lines each have a test in Tasks 1, 2, 5, 5, 5. Interfaces: `Place` is defined in Task 2 and used from Task 4 on; `FloorPlan::label` is used by the HUD in Task 5 and the test in Task 4; `Floor::doorstep` is used by Tasks 2, 4, 5; `Interiors` is created in Task 4 and used by 5 and 6.
