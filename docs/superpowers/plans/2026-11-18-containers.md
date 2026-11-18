# Containers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Lockers, safes, crates, medicine cabinets and desk drawers on site and building
floors that hold rolled loot, opened by the card, the Keyjack, the Net (a crack, or jacking
in at the lock) or force, with their state saved with the run (#96).

**Architecture:** A container is a piece of furniture on a tile (`Furniture::Safe` and
friends, floor-standing or set in a wall); nothing about it is stored until it is touched,
then a `ContainerState` keyed by building, level and tile holds what happened (in
`Interiors` for a plain building, in the `MissionCopy` for a live copy). Contents roll once
from `flavor/loot.ron` on a seed fixed by the site, the level and the tile. A new
`containers` module on the play side resolves a tile into a container, its lock and its
status; a panel screen in the popup's frame shows the lock or the contents, and a lock
screen hosts the Keyjack's slab for a lock with no keypad.

**Tech Stack:** Rust 2024 workspace (`citygen`, `game`, `strata-zero`), ratatui cells,
crossterm keys, serde + RON for flavor, MessagePack (named) for saves.

**Spec:** `docs/superpowers/specs/2026-11-18-containers-design.md` (mock and generator in
`docs/superpowers/specs/2026-11-18-containers-mock/`, round 2 `opt_b` for the map, round 4
`a_locked` / `a_open` for the panels). Read `CLAUDE.md`, `docs/saving.md` and
`manual/README.md` before starting.

## Global Constraints

- Build and test only from the worktree root, always with `--target-dir target/smoke`;
  never run cargo from a `src/` directory. Per-task test commands target the crate and
  module (`cargo test -p strata-zero --target-dir target/smoke containers::`).
- Saved enums grow only at their end: `Furniture` gains `Safe`, `Crate`, `Cabinet`,
  `Drawer` after `Holo`; `Furniture::Locker` keeps its place; `Feature::Container` stays
  (old saves) and draws and plays as a safe.
- Every new field in a saved type is `#[serde(default)]`; no `HashSet`, `Rc`, `Cell`,
  `Instant`, `&'static str` or ratatui `Color` in saved state; a new `AppState` field is
  named in `runs::snapshot::snapshot` and `restore`. The fixtures in
  `crates/strata-zero/src/runs/fixtures/` keep opening and are never regenerated.
- Kinds and colours, exact (`Color::Rgb` / `Rgb`):

  | Kind | Body | Detail closed | Open shape |
  |---|---|---|---|
  | Locker | `#3a4a5c` | `┃` `#9fb6cc` | `▯` |
  | Safe | `#2b2f36` | `◉` `#d8c890` (bold) | `○` |
  | Crate | `#5a4a32` | `╳` `#a89060` | `□` |
  | Cabinet | `#b9c3cc` | `╋` `#c0392b` | `┼` |
  | Drawer | `#3e4c5c` | `╍` `#a0b8d0` | `─` |

  Empty: the open shape in `#5b6b7d` on `#20262e`. On the Net and not yet opened: the
  detail in `#00e6d2`. Force bar `#ffb000`.
- Grades: Locker, Cabinet, Drawer 1; Safe and every layer spot the building's lock maker
  (`keyjack::rating`, 1 to 3). Body needed 2, 4, 6 for grades 1, 2, 3. Force time 3, 6, 10
  seconds for grades 1 to 3, cut 5% per point of Body over the need.
- Locks: Locker locked by the seed about half; Safe always; Crate never; Cabinet in a
  room of the type's `secure` zone or deeper; Drawer about a quarter; a layer spot always
  locked, always on the Net.
- Security bands: low below 86, mid below 170, high from 170.
- Panel copy (digits in counts, the game's words): `◉ SAFE`, `● NET`, `megacorp lock ·
  grade 3`, `lock · grade 1`, `j jack in`, `k Keyjack` / `2 fuses`, `f force it` /
  `Body 6`, `c card · EXECUTIVE` / `none`, `esc leave it`; open: `SAFE · OPEN`, `cash` /
  `1,250 cr`, `credchip` / `400 cr` or `LOCKED` (magenta), `Frag ×2` / `0.8 kg`,
  `t take all`, `pack 9.4/20`, `space take · esc close`. Refusals: `not now` (in a fight),
  `port locked down` (alarm), `locked · crack it at your rig` (a locked credchip), `empty`.
- Look: `safe · megacorp grade 3 · locked`, `· open`, `· empty`, `· on the Net`.
- Panel colours are `ui::chrome`'s: `CYAN`, `MAGENTA`, `DIM`, `FAINT`, `FAINTER`, `TEXT`,
  `PANEL_BG`, `BAR_BG` (they match the mock's).
- Every glyph drawn is in the window's fonts (`every_drawn_character_has_a_glyph`); no
  colour emoji; no Alt chords.
- The player's own flat keeps its lockers as furniture.
- Commits end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; the last
  task's commit ends with `Closes #96`. Committed docs record decisions, never chat.
- The five checks on both front ends pass at the end (CLAUDE.md, "Building and checking").

## Review Focus

- A wall-set container backed by another room's floor: from the far side of its wall it
  is out of reach (no subject, no panel); only the floor in front of it opens it. Test
  in Task 7.
- An old save: a copy's floor drawn with `Feature::Container` and a layer whose spots
  have no kind: each reads as a locked safe on the Net at the maker's grade, and opens.
  Tests in Tasks 3 and 6.
- Saved contents naming an item this build no longer has: the panel leaves the line out
  and taking all never panics (saving rule 6). Test in Task 6.
- The player's flat: its lockers give no subject, no look line and no state. Test in
  Task 6.
- A Keyjack lockout that has run out: the container's faults are clear again and `k`
  works, as a keypad's do. Test in Task 8.

---

## File structure

citygen:
- `crates/citygen/src/tile.rs` — `ContainerKind`, the new `Furniture` pieces,
  `Furniture::container`, `Feature::container`, `wall_front`.
- `crates/citygen/src/interior/venue/types.rs`, `layout.rs`, `rooms.rs`, `zones.rs` —
  `RoomDef.wall`, `PlannedRoom.wall`.
- `crates/citygen/src/interior/venue/draw.rs` — wall-set pieces; a copy's spots by kind.
- `crates/citygen/src/interior/venue/check.rs` — `validate_containers`.
- `crates/citygen/src/interior/venue/devices.rs` — terminals keep off containers.
- `crates/citygen/src/interior/venue/mission.rs` — `ContainerSpot`.
- `crates/citygen/src/sitenet/mod.rs` — safe and layer-spot nodes by kind.
- `flavor/sites.ron`, `crates/citygen/tests/venues.rs` (pins).

game:
- `crates/game/src/character.rs` (`ItemCopy.credits`, `give_credchip`),
  `model.rs` (`Use::Cash`), `action.rs` (`Action::UseCopy`, `over_capacity`).
- `flavor/content.ron` (two credchips), `flavor/looks.ron` (furniture lines).

strata-zero:
- `src/containers/mod.rs` — `ContainerAt`, `Container`, `Lock`, `Status`, `find`, `lock`,
  `status`, state routing, `open`, `contents`, `take`, `take_all`, `look_line`, `looks`.
- `src/containers/loot.rs` — `flavor/loot.ron`'s tables, `band`, `roll`.
- `src/containers/state.rs` — `ContainerState`, `Opened`, `Loot`.
- `src/containers/ways.rs` — card, crack, Keyjack faults and lockout, jack in.
- `src/containers/force.rs` — `need`, `ticks`, `can`, `Forcing`, `finish`.
- `src/containers/tests.rs`, `src/containers/testutil.rs`.
- `src/screens/container.rs` — `ContainerPanel` (locked and open).
- `src/screens/clip.rs` — the Keyjack's `Clip`, moved out of `keypad.rs`.
- `src/screens/lockjack.rs` — `LockJack`, the slab on a lock with no keypad.
- `src/render/containers.rs` — `ContainerLooks` and `ForceBar` overlays.
- Modified: `render/tileset.rs`, `interact.rs`, `screens/explore.rs`, `screens/keypad.rs`,
  `screens/deck.rs`, `interiors.rs`, `venues/copy.rs`, `sitenet/ways.rs`, `keyjack.rs`,
  `look.rs`, `app.rs`, `runs/snapshot.rs`, `screens/shots.rs`, `main.rs`,
  `screens/mod.rs`, `render/mod.rs`.
- `flavor/loot.ron` (new), `manual/06-the-city-and-the-world.md`,
  `manual/07-turns-and-fights.md`, `docs/saving.md`.

---

### Task 1: Container kinds, pieces and their closed look

**Files:**
- Modify: `crates/citygen/src/tile.rs` (Furniture ~L338, Feature ~L199, tests ~L660)
- Modify: `crates/strata-zero/src/render/tileset.rs` (glyph_for ~L136, feature_color ~L484, styled ~L520, tests ~L1010)
- Modify: `flavor/looks.ron` (furniture group ~L135)
- Modify: `crates/strata-zero/src/look.rs` (test list ~L1627 if it names `Container`)

**Interfaces:**
- Produces (citygen, re-exported at the crate root):
  - `#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)] pub enum ContainerKind { Locker, Safe, Crate, Cabinet, Drawer }` with `#[default] Safe`.
  - `ContainerKind::furniture(self) -> Furniture`; `ContainerKind::word(self) -> &'static str` = `"locker" | "safe" | "crate" | "cabinet" | "drawer"`; `ContainerKind::ALL: [ContainerKind; 5]`.
  - `Furniture::{Safe, Crate, Cabinet, Drawer}` appended after `Holo`; labels `"safe"`, `"crate"`, `"medicine cabinet"`, `"desk drawer"`.
  - `Furniture::container(self) -> Option<ContainerKind>` (Locker, Safe, Crate, Cabinet, Drawer).
  - `Feature::container(self) -> Option<ContainerKind>`: `Furniture(f)` → `f.container()`, `Feature::Container` → `Some(Safe)`, else None.
  - `pub fn wall_front(tiles: &Grid<Tile>, at: Pos) -> Option<Pos>`: for a `Terrain::Wall` tile, its one `Terrain::Floor` 4-neighbour; None for any other tile or with zero or two-plus floor neighbours.
- Produces (strata-zero): `render::tileset::container_body(k: ContainerKind) -> Rgb`, `container_detail(k) -> (char, Rgb)`, `container_open_shape(k) -> char`, and consts `CONTAINER_EMPTY_FG: Rgb = Rgb(0x5b,0x6b,0x7d)`, `CONTAINER_EMPTY_BG: Rgb = Rgb(0x20,0x26,0x2e)`, `CONTAINER_NET: Rgb = Rgb(0x00,0xe6,0xd2)`; Task 11 draws the states with them.

- [ ] **Step 1: Write the failing tests**

In `tile.rs` tests:

```rust
#[test]
fn each_container_kind_is_a_piece_and_back() {
    for k in ContainerKind::ALL {
        assert_eq!(k.furniture().container(), Some(k));
        assert_eq!(Feature::Furniture(k.furniture()).container(), Some(k));
    }
    assert_eq!(Feature::Container.container(), Some(ContainerKind::Safe));
    assert_eq!(Furniture::Desk.container(), None);
    assert_eq!(ContainerKind::default(), ContainerKind::Safe);
}

#[test]
fn new_furniture_keeps_the_old_indices() {
    let lead = |f: Furniture| postcard::to_stdvec(&f).unwrap()[0];
    assert_eq!(lead(Furniture::Locker), 8);
    assert_eq!(lead(Furniture::Holo), 15);
    assert_eq!(lead(Furniture::Safe), 16);
    assert_eq!(lead(Furniture::Drawer), 19);
}

#[test]
fn a_wall_tile_has_a_front_only_with_one_floor_beside_it() {
    // a 5x3 grid: wall row y=0, floor row y=1, wall row y=2; (2,0) fronts (2,1);
    // a floor tile and a wall with floor on both sides (make (2,2) floor) have none
}
```

In `tileset.rs` tests:

```rust
#[test]
fn a_container_draws_its_detail_on_its_body() {
    // cell() of a Floor tile with Feature::Furniture(Furniture::Safe), full light:
    // symbol "◉", bold; fg/bg trace back to #d8c890 on #2b2f36 (compare with lighting off
    // the way the existing furniture test does). Same for a Wall tile with a Locker: "┃".
    // Feature::Container draws as the safe: "◉".
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p citygen --target-dir target/smoke tile::` and `cargo test -p strata-zero --target-dir target/smoke render::tileset::`
Expected: FAIL, `ContainerKind` not found.

- [ ] **Step 3: Implement**

`tile.rs`: the enum, methods and `wall_front` as above; `Furniture::label` arms. The new
furniture is not walkable (it is `Feature::Furniture(_)`). `tileset.rs`: in `glyph_for`,
each container piece draws `container_detail(k).0` and `Feature::Container` draws `◉`; in
`feature_color`, a container piece (and `Feature::Container`) takes its detail colour; in
`styled`, set `bg = container_body(k)` for a container piece before lighting (as wall
fittings set `bg`), and add `BOLD` for a safe. `Furniture::Locker` loses `▮` for `┃` on its
body everywhere, the flat's too (it is drawn as a locker; it stays furniture in play).
`looks.ron`: a line or two each for `"safe"`, `"crate"`, `"medicine cabinet"`,
`"desk drawer"` in the furniture group, in the in-world voice.

- [ ] **Step 4: Run them to see them pass**

Run the two commands of Step 2, then `cargo test -p strata-zero --target-dir target/smoke look::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/citygen/src/tile.rs crates/strata-zero/src/render/tileset.rs crates/strata-zero/src/look.rs flavor/looks.ron
git commit -m "tile: container kinds as furniture, drawn as solid bodies (#96)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Containers in venue rooms, on the floor and in the wall

**Files:**
- Modify: `crates/citygen/src/interior/venue/types.rs:297` (`RoomDef`)
- Modify: `crates/citygen/src/interior/venue/layout.rs:115` (`PlannedRoom`), `rooms.rs:223,353`, `zones.rs:252`
- Modify: `crates/citygen/src/interior/venue/draw.rs:25-61,238-299` (`draw_level`, `furnish`)
- Modify: `crates/citygen/src/interior/venue/devices.rs:86-108` (`Used::candidates`)
- Modify: `crates/citygen/src/interior/venue/check.rs:180` (`validate_venue`)
- Modify: `flavor/sites.ron` (rooms of hotel, office, metro)
- Modify: `crates/citygen/tests/venues.rs:257` (`PINNED`)

**Interfaces:**
- Consumes: `ContainerKind`, `Furniture::container`, `wall_front` (Task 1).
- Produces:
  - `RoomDef.wall: Vec<Furniture>` and `PlannedRoom.wall: Vec<Furniture>`, both `#[serde(default)]` (a layout saved before has none).
  - `pub fn validate_containers(x: &LevelLayout, floor: &Floor) -> Result<(), String>` in `check.rs`, called from `validate_venue` for every level drawn (and from `validate_copy` in Task 3).

- [ ] **Step 1: Write the failing tests** (in `draw.rs` tests, over `layouts()` and the builtin pack's types laid with `lay_out(t, VenueFrame::own(t), seed)` for seeds `0..6`)

```rust
#[test]
fn wall_pieces_stand_in_the_wall_with_their_front_free() {
    // every level of every builtin type: each tile holding a container piece on
    // Terrain::Wall has wall_front(&floor.tiles, at) == Some(front), front is walkable floor
    // with no feature, inside the room that lists it, and not within manhattan 1 of a door
    // or an exit's doorstep; no two wall pieces share a front
}

#[test]
fn a_room_holds_its_wall_pieces() {
    // the hotel's "lockers" room: at least 1 Locker on a wall tile round its rect; the
    // office's "executive office": a Safe on a wall tile round its rect
}

#[test]
fn no_container_stands_on_a_doorstep_or_a_spine() {
    // every floor container (Terrain::Floor + container piece): not 4-adjacent to a door
    // tile or an exit doorstep, and in a room whose index is >= x.bands.len()
}

#[test]
fn a_room_with_no_wall_pieces_draws_as_before() {
    // test_type.ron (lists no `wall`): venue_fingerprint of its layout for seeds 0..4
    // equals the value pinned before this change (record it in Step 2 from main)
}
```

In `check.rs` tests:

```rust
#[test]
fn validate_containers_finds_a_piece_on_a_doorstep_and_one_walled_in() {
    // draw a builtin hotel level; put a Locker on a door's neighbour: Err containing
    // "doorstep"; put a Safe on a wall tile with no floor beside it: Err containing "front"
}
```

- [ ] **Step 2: Run them to see them fail**

Before changing code, record `venue_fingerprint` for `test_type()` seeds 0..4 into the
fourth test. Run: `cargo test -p citygen --target-dir target/smoke venue::draw venue::check`
Expected: FAIL (`wall` unknown field / no wall pieces).

- [ ] **Step 3: Implement**

- `wall` on `RoomDef` and `PlannedRoom`, copied in `rooms.rs` beside `furniture`; empty
  in the generated spine rooms (`rooms.rs:223`, `zones.rs:252`).
- `draw_level`: compute `let switches = line_switches(layout, level)` (their `at` and
  `front`) before furnishing; `furnish` takes them as tiles no wall piece may use.
- `furnish`: wall pieces first. Candidates: `(wall, front)` pairs round the room's rect,
  corners left out (the `devices::ring` shape), where the wall tile is `Terrain::Wall`
  with no feature, not reserved, not a switch tile, and `wall_front` is `front`, and
  `free(b, front)` and `b.station_free(front)` hold. Start the walk at
  `hash64(hash64(floor_seed, WALL_SALT), room.number as u64) % n`, where `floor_seed` is
  `hash64(layout.seed, level)` as `draw_level` makes it (pass it to `furnish`; new
  `const WALL_SALT: u64 = 0x3A11_5E75`), never from `b.rng`, so a room with no `wall`
  draws as before; pieces sit next to each other (a bank), fronts distinct. Set
  `tiles[wall].feature`, then add each `front` to `b.clear` so floor pieces and stations
  keep off it. Floor pieces then go as today.
- `devices.rs` `Used::candidates`: drop a wall tile holding a container and any front
  that is a wall container's front (collect them from the floor with `wall_front`).
- `validate_containers`: per container tile, wall-set → `wall_front` is Some, its front
  walkable and clear of doors and doorsteps; floor-standing → not 4-adjacent to a door or
  doorstep; both → in a room off the spine. Errors name the tile.
- `sites.ron`, within the spec's guide:
  - hotel: `lockers` → `furniture: [Bench], wall: [Locker, Locker, Locker, Locker]`;
    `guest room` → `furniture: [Bed, Drawer]`; `security office` →
    `furniture: [Desk, Machine, Safe, Locker]`; `linen room` →
    `furniture: [Shelf, Shelf, Crate, Crate]`; `office` (the penthouse's) → `wall: [Safe]`.
  - office: `open plan` → `furniture: [Desk, Drawer, Desk, Drawer]`; `executive office` →
    `wall: [Safe]`; `store` → `furniture: [Shelf, Crate]`; `lift hall` → `wall: [Cabinet]`.
  - metro: `staff room` → `furniture: [Table], wall: [Locker, Locker, Locker]`;
    `cash room` → `furniture: [Safe]`; `store room` → `furniture: [Shelf, Crate, Crate]`.
- `tests/venues.rs`: run `venue_fingerprints_are_pinned`, and pin the three new values
  (the change is meant to move every type's sites).

- [ ] **Step 4: Run them to see them pass**

Run: `cargo test -p citygen --target-dir target/smoke`
Expected: PASS, the whole crate (every type still passes the checker on every size).

- [ ] **Step 5: Commit**

```bash
git add crates/citygen flavor/sites.ron
git commit -m "venue: containers in rooms, standing and set in the wall (#96)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The layer's spots by kind, and safes on the site's Net

**Files:**
- Modify: `crates/citygen/src/interior/venue/mission.rs:44-50,114-115,530-541`
- Modify: `crates/citygen/src/interior/venue/mod.rs:25` (re-export)
- Modify: `crates/citygen/src/interior/venue/draw.rs:85-86`, `check.rs:281-282`
- Modify: `crates/citygen/src/sitenet/mod.rs:262-283`
- Modify: `crates/citygen/src/interior/venue/mission/tests.rs:325,337,556,584`, `crates/citygen/src/sitenet/tests.rs`, `crates/strata-zero/src/venues/copy/tests.rs:242,574`

**Interfaces:**
- Consumes: `ContainerKind`, `wall_front`, `validate_containers` (Tasks 1, 2).
- Produces:
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)] pub struct ContainerSpot { pub level: i32, pub at: Pos, #[serde(default)] pub kind: ContainerKind }` (a saved spot with no kind reads as a safe); `MissionLayer.containers: Vec<ContainerSpot>`.
  - `fn spot_kind(layout: &VenueLayout, level: i32, at: Pos) -> ContainerKind` (private): `Safe` in a room whose zone is named `"secure"` or `"executive"`, else `Crate` in a `RoomKind::Storage` or `RoomKind::Plant` room, else `Locker`. It draws nothing from the rng, so a layer lays the same spots as before.
  - `draw_copy_level` puts `Feature::Furniture(spot.kind.furniture())` on each spot; `validate_copy` expects that and calls `validate_containers`.
  - `SiteNet::of`: a `NodeKind::Container` node for each layer spot, named `"{room} {kind.word()}"`; and on every level, one for each `Furniture::Safe` on the drawn floor (floor-standing: `net = at`; wall-set: `net = wall_front(..)`), named `"{room} safe"`. No other room container gets a node.

- [ ] **Step 1: Write the failing tests**

`mission/tests.rs`:

```rust
#[test]
fn a_spot_takes_its_kind_from_its_room() {
    // lay layers for every builtin type at security 200, seeds 0..8: every spot in a
    // "secure"/"executive" zone room is Safe; in a Storage/Plant room otherwise, Crate;
    // the rest Locker; laying the same inputs twice gives the same layer
}

#[test]
fn a_spot_saved_with_no_kind_reads_as_a_safe() {
    #[derive(serde::Serialize)]
    struct Old { level: i32, at: Pos }
    let bytes = rmp_serde::to_vec_named(&Old { level: 2, at: Pos::new(4, 5) }).unwrap();
    let back: ContainerSpot = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(back.kind, ContainerKind::Safe);
}
```

(`rmp-serde` is a dev-dependency of citygen if it isn't yet; else put this test in
`strata-zero/src/venues/copy/tests.rs`.)

`sitenet/tests.rs`:

```rust
#[test]
fn every_safe_and_every_layer_spot_is_a_container_node() {
    // a plain hotel (builtin type, own frame, seed 1): every Furniture::Safe tile on every
    // level has a Container node at that tile, its `net` the tile (floor) or its front
    // (wall); no Locker/Crate/Cabinet/Drawer tile has a node; a copy's layer spot of each
    // kind has a node named ending in its kind's word
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p citygen --target-dir target/smoke mission:: sitenet::`
Expected: FAIL, `ContainerSpot` not found.

- [ ] **Step 3: Implement** as the Interfaces say; update the tests that expected
`Feature::Container` on spots to expect the kind's furniture.

- [ ] **Step 4: Run them to see them pass**

Run: `cargo test -p citygen --target-dir target/smoke` and `cargo test -p strata-zero --target-dir target/smoke venues:: sitenet::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/citygen crates/strata-zero/src/venues crates/strata-zero/src/sitenet
git commit -m "venue: a job's containers by kind, safes on the site's Net (#96)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Credchips

**Files:**
- Modify: `crates/game/src/character.rs:38-47,1393-1406` (`ItemCopy`, `new_copy`, new `give_credchip`)
- Modify: `crates/game/src/model.rs:957,1076` (`Use`, `Item::per_copy`)
- Modify: `crates/game/src/action.rs:23-120,336,409-429` (`Action::UseCopy`, `over_capacity` pub)
- Modify: every other `ItemCopy { .. }` literal (`ground.rs`, `keyjack.rs`, `screens/keypad.rs`; `grep -rn "ItemCopy {" crates`)
- Modify: `crates/strata-zero/src/screens/deck.rs:199-240,435-460` (`marks`, `copy_popup`), `crates/strata-zero/src/actions.rs:39,214` (UseCopy beside Use)
- Modify: `flavor/content.ron` (two items, beside `bottle_caps`)

**Interfaces:**
- Produces:
  - `ItemCopy.credits: u32` `#[serde(default)]`: a credchip's sum.
  - `Use::Cash { locked: bool }`; `Item::per_copy` is also true for an item whose `on_use` is `Some(Use::Cash { .. })`.
  - `Action::UseCopy(CopyId)`: a `Use::Cash { locked: false }` copy adds its `credits` to `Character.credits` and is gone, `"cashed `Credchip` · +400 cr"`; `locked: true` refuses `"locked · crack it at your rig"`; any other item refuses as `Use` does for an unusable one. `Action::Use(id)` on a Cash item refuses `"use it from the pack"`.
  - `pub fn give_credchip(&mut self, c: &Content, item: &ItemId, credits: u32) -> CopyId` on `Character`: a new copy with `credits` set, stowed in the pack.
  - `pub fn over_capacity(ch: &Character, c: &Content) -> String` (was private).
  - `flavor/content.ron`: `credchip` ("Credchip") and `credchip_locked` ("Locked credchip"), `kind: Junk`, `weight: 0.05`, `value: 0`, `use: Some(Cash(locked: false|true))`.
  - `deck::marks` adds ` · 400 cr` (dim) for a copy with `credits > 0`; `copy_popup` offers `u Use` → `Pick::Act(Action::UseCopy(copy))` for a Cash item.

- [ ] **Step 1: Write the failing tests**

`game/src/action.rs` tests:

```rust
#[test]
fn a_credchip_cashes_by_use_and_a_locked_one_refuses() {
    let id = ch.give_credchip(&c, &ItemId::new("credchip"), 400);
    let before = ch.credits;
    let out = apply(&mut ch, &mut g, &c, at, 0, Action::UseCopy(id)).unwrap();
    assert_eq!(ch.credits, before + 400);
    assert_eq!(ch.count(&ItemId::new("credchip")), 0);
    assert!(out.message.contains("400"));
    let locked = ch.give_credchip(&c, &ItemId::new("credchip_locked"), 900);
    assert_eq!(
        apply(&mut ch, &mut g, &c, at, 0, Action::UseCopy(locked)).unwrap_err(),
        "locked · crack it at your rig"
    );
    assert_eq!(ch.credits, before + 400);
}

#[test]
fn a_copy_saved_before_credits_reads_none() {
    // rmp named round trip of a struct shaped like the old ItemCopy (id, fitted, stickers,
    // charge) into ItemCopy: credits == 0
}
```

`strata-zero/src/screens/deck.rs` tests:

```rust
#[test]
fn a_credchip_names_its_sum_and_cashes_from_the_pack() {
    // give_credchip(400); marks() text ends with " · 400 cr"; copy_popup has a 'u' row
    // whose pick is Action::UseCopy(that copy)
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p game --target-dir target/smoke action::` and `cargo test -p strata-zero --target-dir target/smoke screens::deck::`
Expected: FAIL, `give_credchip` / `UseCopy` not found.

- [ ] **Step 3: Implement** as the Interfaces say. Content tests that walk every item
(`game/src/content.rs`) must still pass with the two new items.

- [ ] **Step 4: Run them to see them pass**

Run: `cargo test -p game --target-dir target/smoke` and `cargo test -p strata-zero --target-dir target/smoke screens::deck:: actions::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/game crates/strata-zero/src/screens/deck.rs crates/strata-zero/src/actions.rs crates/strata-zero/src/keyjack.rs crates/strata-zero/src/screens/keypad.rs flavor/content.ron
git commit -m "items: credchips with a sum, cashed from the pack (#96)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The loot tables and the roll

**Files:**
- Create: `flavor/loot.ron`
- Create: `crates/strata-zero/src/containers/mod.rs` (module skeleton: `pub mod loot; pub mod state;`), `crates/strata-zero/src/containers/loot.rs`, `crates/strata-zero/src/containers/state.rs`
- Modify: `crates/strata-zero/src/main.rs` (`mod containers;`)

**Interfaces:**
- Consumes: `ContainerKind` (Task 1); `game::Content` for the check.
- Produces (`containers::state`, saved types):
  - `#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)] pub enum Loot { Cash(u32), Chip { credits: u32, locked: bool }, Item { item: ItemId, qty: u32 }, Card(u8) }` — `Card(z)` is the building's zone-`z` card; `Item` also carries a fixed item for #98.
- Produces (`containers::loot`):
  - `pub enum Band { Low, Mid, High }`; `pub fn band(security: u8) -> Band` (`< 86` Low, `< 170` Mid, else High).
  - `pub struct Tables` (deserialized from the file; schema below), `Tables::parse(src: &str) -> Result<Tables, LootError>`, `Tables::shipped() -> &'static Tables` (`OnceLock`, `include_str!("../../../../flavor/loot.ron")`), `Tables::check(&self, content: &game::Content) -> Result<(), String>`.
  - `pub struct TableKey<'a> { pub kind: ContainerKind, pub site: &'a str, pub guarded: bool, pub deeper: Option<u8> }` — `site` is a venue's `type_id` or `"plain"`; `guarded` a job's layer spot; `deeper` the zone a card line hands out (None: card lines roll nothing).
  - `pub fn roll(tables: &Tables, key: &TableKey, security: u8, seed: u64) -> Vec<Loot>`, pure.
- Schema (`flavor/loot.ron`, builder's numbers within the spec's guide):

```ron
Loot(
    // Percent a cash or credchip sum is scaled by in each band: low, mid, high.
    scale: (100, 150, 220),
    tables: [
        // site: None is the kind's fallback; guarded: a job's container.
        Table(kind: Locker, site: None, guarded: false, empty: 55, count: (1, 2), lines: [
            Line(weight: 5, what: Cash((5, 25))),
            Line(weight: 3, what: Item("soy_noodle_cup", (1, 2))),
            Line(weight: 2, what: Item("stim_patch", (1, 1))),
        ]),
        // ...
    ],
)
```

  `Line { weight: u32, #[serde(default)] bands: Vec<Band> (empty = every band), what: What }`, `What::{Cash((u32, u32)), Chip((u32, u32), u8 /* percent locked */), Item(String, (u32, u32)), Card}`. Table lookup order: `(kind, Some(site), guarded)`, `(kind, None, guarded)`, `(kind, Some(site), false)`, `(kind, None, false)`.
- The roll, on a hash stream (no new dependency): `let mut n = 0; let mut next = || { n += 1; hash64(seed, n) };` — `next() % 100 < empty` → nothing; else a count in `count`, and per line drawn: only lines whose `bands` hold the band, weighted by `weight`; a sum `lo + next() % (hi - lo + 1)` then `* scale[band] / 100`; a chip locked when `next() % 100 < percent`; a `Card` line gives `Loot::Card(deeper)` or nothing.

- [ ] **Step 1: Write the failing tests** (`containers/loot.rs` tests)

```rust
#[test]
fn the_shipped_tables_hold_up() {
    let c = game::Content::load().unwrap();
    Tables::shipped().check(&c).unwrap();
    // every ContainerKind has a plain fallback (site None, guarded false) and a guarded one
}

#[test]
fn check_finds_an_unknown_item_a_bad_range_and_a_missing_fallback() {
    // three small sources: Item("no_such_item", ..) → Err naming it; Cash((9, 3)) → Err;
    // no Crate fallback → Err naming Crate
}

#[test]
fn a_roll_is_the_same_twice_and_another_seed_rolls_otherwise() {
    let key = TableKey { kind: ContainerKind::Safe, site: "office", guarded: true, deeper: Some(3) };
    let t = Tables::shipped();
    assert_eq!(roll(t, &key, 200, 7), roll(t, &key, 200, 7));
    assert!((0..20).any(|s| roll(t, &key, 200, s) != roll(t, &key, 200, 7)));
}

#[test]
fn bands_scale_sums_and_pick_lines() {
    // a one-table source: Cash((100, 100)) in every band, scale (100, 150, 220), empty 0,
    // count (1, 1): security 10 → [Cash(100)], 100 → [Cash(150)], 200 → [Cash(220)];
    // a line with bands: [High] never rolls at security 10
}

#[test]
fn a_card_line_needs_a_deeper_zone() {
    // a table of one Card line, empty 0: deeper Some(2) → [Card(2)]; None → []
}

#[test]
fn band_edges() {
    assert_eq!(band(85), Band::Low);
    assert_eq!(band(86), Band::Mid);
    assert_eq!(band(169), Band::Mid);
    assert_eq!(band(170), Band::High);
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke containers::loot::`
Expected: FAIL, module missing.

- [ ] **Step 3: Implement** `loot.rs`, `state.rs` (the `Loot` enum only for now) and
`flavor/loot.ron` with a table per kind (plain fallback and guarded fallback, plus
site rows where the guide asks: a security locker, safes deeper in). As a guide from the
spec: lockers a little cash, noodle cups, stim patches (a `secure`-site locker adds `frag`
and a `Card` line); drawers a little cash, now and then a credchip; cabinets
`stim_patch`; crates `fuse_stock`, `copper_coil`, `scrap_servo`, `frag` in a guarded
crate; safes cash and credchips (a locked one often), and in the high band a program
(`crack_2`, `copy_2`) or hardware (`fuse_ceramic`). Plain tables lean empty (40-60%) and
cheap; guarded tables are where the money is.

- [ ] **Step 4: Run them to see them pass**

Run: `cargo test -p strata-zero --target-dir target/smoke containers::loot::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add flavor/loot.ron crates/strata-zero/src/containers crates/strata-zero/src/main.rs
git commit -m "loot: tables by kind, site and security, rolled from a seed (#96)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: A container in play: find, lock, state, open, take, save

**Files:**
- Modify: `crates/strata-zero/src/containers/mod.rs`, `containers/state.rs`
- Create: `crates/strata-zero/src/containers/tests.rs`, `crates/strata-zero/src/containers/testutil.rs`
- Modify: `crates/strata-zero/src/interiors.rs:26-45` (field + accessors), `crates/strata-zero/src/venues/copy.rs:18-33` (field + `begin`'s literal)
- Modify: `docs/saving.md` ("Sites and copies")

**Interfaces:**
- Consumes: Tasks 1, 3, 4, 5; `sitenet::{net_of, layer_of, security, container_open}`, `venues::copy_live`, `keyjack::rating`, `ui::device::{Look, Device}`, `credentials::{stock_zone_cards, zone_card_id}`.
- Produces (`containers::state`, saved):
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)] pub enum Opened { Free, Card, Keyjack, Net, Forced }` (grows at its end).
  - `#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)] pub struct ContainerState { #[serde(default)] pub opened: Option<Opened>, #[serde(default)] pub left: Option<Vec<Loot>>, #[serde(default)] pub faults: u8, #[serde(default)] pub locked_until: Option<u64> }` — `left` is what's left, rolled at opening; `faults` and `locked_until` (a game minute) are the Keyjack's.
  - `Interiors.containers: BTreeMap<(BuildingId, i32, Pos), ContainerState>` `#[serde(default)]`; `MissionCopy.containers: BTreeMap<(i32, Pos), ContainerState>` `#[serde(default)]` (goes with the copy).
- Produces (`containers`):
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub struct ContainerAt { pub building: BuildingId, pub level: i32, pub at: Pos }`.
  - `#[derive(Clone, Debug, PartialEq, Eq)] pub struct Container { pub at: ContainerAt, pub kind: ContainerKind, pub front: Option<Pos>, pub guarded: bool, pub zone: Option<u8>, pub site: String }` — `front` for a wall-set one; `guarded` a live copy's layer spot; `zone` the venue room's (the room holding `front` or `at`); `site` the venue's `type_id`, else `"plain"`.
  - `#[derive(Clone, Debug, PartialEq, Eq)] pub struct Lock { pub grade: u8, pub maker: Option<String>, pub node: Option<SiteNode> }` — `maker` for a maker-graded lock (a safe, a layer spot); `node` when its site's Net has a `Container` node on the tile.
  - `pub enum Status { Locked(Lock), Shut, Open(Vec<Loot>), Empty }` — `Shut`: no lock, never opened.
  - `pub fn find(st: &AppState, at: ContainerAt) -> Option<Container>`: the tile's `Feature::container()` on the session's floor (`interiors::floor`), never inside the player's flat (`st.home`: same building and level, `home.rect` holds the tile).
  - `pub fn lock(st: &AppState, c: &Container) -> Option<Lock>`: per the Global Constraints' lock rules; the seed rule is `hash64(seed(st, c), LOCK_SALT) % 100 < 50` (Locker) or `< 25` (Drawer); maker from `Look::pick(st, Device::Keypad(b)).name` and grade `keyjack::rating(st, b)`.
  - `pub fn seed(st: &AppState, c: &Container) -> u64` = `hash64(hash64(site_seed, CONTAINER_SALT), ((level as i64 as u64) << 32) ^ ((at.x as u16 as u64) << 16) ^ (at.y as u16 as u64))`; `site_seed` is `hash64(layout.seed, inputs.seed)` on a live copy, `layout.seed` on a plain venue, `hash64(city.seed, b.index() as u64)` on a classic building. Consts `CONTAINER_SALT = 0xC047_A1E5`, `LOCK_SALT = 0x10C4_ED00`.
  - `pub fn state(st: &AppState, at: ContainerAt) -> Option<&ContainerState>` and `pub fn state_mut(st: &mut AppState, at: ContainerAt) -> &mut ContainerState`: the live copy's map when `venues::copy_live`, else `Interiors.containers`; `state_mut` inserts a default.
  - `pub fn status(st: &AppState, c: &Container) -> Status`.
  - `pub fn contents(st: &AppState, c: &Container) -> Vec<Loot>`: `left` once opened, else the roll (pure); lines naming an item the content lacks are left out.
  - `pub fn open(st: &mut AppState, c: &Container, how: Opened)`: sets `opened` (once), rolls `left` once with `TableKey { kind, site, guarded, deeper: zone + 1 when the layout has that zone }` at `sitenet::security(st, b)`; `Forced` on a lock with a `node` calls `net::lock_down(st, b)`.
  - `pub fn take(st: &mut AppState, c: &Container, line: usize) -> Result<String, String>` and `pub fn take_all(st: &mut AppState, c: &Container) -> Result<String, String>`: cash to `character.credits`; a chip as `give_credchip("credchip" | "credchip_locked", credits)`; an item by `give_item`; a card after `stock_zone_cards` as `zone_card_id(b, z)`. A line whose weight would take the pack over `carry_cap` (not in dev) is refused with `game::over_capacity`'s text; `take_all` stops at the first refused line and says so. What's taken leaves `left`.
  - `pub fn cr(n: u32) -> String` → `"1,250 cr"`; `pub fn line_weight(st: &AppState, l: &Loot) -> f32`.
- `containers::testutil` (`#[cfg(test)]`): `pub(crate) fn plain_site_with(kind: ContainerKind) -> (AppState, Container)` (from `sitenet::testutil::office_plain` and a hotel equivalent through `venues::dev_start_with(.., false)`, the first container of that kind on a kept level, its floor drawn); `pub(crate) fn copy_with_spot(security: u8) -> (AppState, Container)` (`office_copy`, the first layer spot); `pub(crate) fn beside(st: &mut AppState, c: &Container)` (stands you on its front, or a walkable 4-neighbour).

- [ ] **Step 1: Write the failing tests** (`containers/tests.rs`)

```rust
#[test]
fn every_kind_locks_as_the_spec_says() {
    // over hotel and office sites, seeds 0..6: every Safe → Lock with maker Some and grade
    // == keyjack::rating; every Crate → None; Lockers locked between 30% and 70% of them;
    // Drawers between 10% and 40%; a Cabinet in the lift hall (staff) → None
}

#[test]
fn a_layer_spot_is_locked_at_the_makers_grade_on_the_net() {
    let (st, c) = copy_with_spot(150);
    let l = lock(&st, &c).expect("locked");
    assert!(c.guarded && l.node.is_some() && l.maker.is_some());
}

#[test]
fn the_same_container_rolls_the_same_twice_and_across_a_save() {
    // contents() equal twice; open(Free) then contents() equals the pure roll; reopen via
    // outside::tests::reopen: state(at) equal, contents equal
}

#[test]
fn a_new_copy_rolls_fresh() {
    // the same room safe tile: plain roll vs a live copy (inputs.seed 1) vs another copy
    // (inputs.seed 2) — the copies' seeds differ from each other and from the plain one
}

#[test]
fn taking_half_leaves_half() {
    // a container opened with left of 4 lines (set state_mut().left directly): take(0),
    // take(0) → 2 lines left, the right ones; credits up by the cash taken; take_all → empty
}

#[test]
fn a_take_over_the_carry_cap_is_refused() {
    // fill the pack to carry_cap - 0.1 kg; left = [Item frag ×3]; take(0) is Err with
    // "over capacity"; left unchanged; a Cash line still takes
}

#[test]
fn a_line_naming_an_item_this_build_lacks_is_left_out() {
    // left = [Item("gone_item", 1), Cash(10)]: contents() == [Cash(10)]; take_all Ok, no panic
}

#[test]
fn a_kindless_spot_on_an_old_copys_floor_is_a_safe() {
    // put Feature::Container on a layer spot's tile of the drawn copy floor
    // (interiors.put_floor): find → kind Safe, guarded; lock → maker grade, node Some
}

#[test]
fn the_flats_lockers_are_no_containers() {
    // a home city (runs::testutil::story_city + MapScreen::walking): every Locker tile in
    // home.rect → find() None; after a frame, Interiors.containers is empty
}

#[test]
fn a_plain_site_pays_less_than_a_job() {
    // for each builtin type with builds (hotel, office) and seeds 0..4: open every room
    // container of every level of the plain site at security 128 and sum Cash + unlocked
    // Chip credits: < 128 * jobs::PAY_COPY_PER_SECURITY
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke containers::tests`
Expected: FAIL, `find` not found.

- [ ] **Step 3: Implement** as the Interfaces say; add `mod tests; mod testutil;` under
`#[cfg(test)]`. In `docs/saving.md` "Sites and copies" add a bullet: what
`ContainerState` holds, where it lives (`Interiors.containers` / `MissionCopy.containers`,
both defaulting), that an untouched container has no entry and rolls from its seed, and
that a copy's go with it when it ends.

- [ ] **Step 4: Run them to see them pass**

Run: `cargo test -p strata-zero --target-dir target/smoke containers:: runs::`
Expected: PASS (the fixtures still open).

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero/src/containers crates/strata-zero/src/interiors.rs crates/strata-zero/src/venues/copy.rs docs/saving.md
git commit -m "containers: a lock by kind, contents rolled once and saved (#96)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: The panels, the card and the crack

**Files:**
- Create: `crates/strata-zero/src/screens/container.rs`, `crates/strata-zero/src/containers/ways.rs`
- Modify: `crates/strata-zero/src/screens/mod.rs`, `crates/strata-zero/src/interact.rs:22-52,137-163,165-655`, `crates/strata-zero/src/screens/explore.rs:2015-2085,2360-2370` (the three `s.window(..)` callers)

**Interfaces:**
- Consumes: Task 6's API; `credentials::zone_card_held`; `sitenet::container_open`; `ui::chrome::{frame, solid, put, CYAN, MAGENTA, DIM, FAINT, FAINTER, TEXT, PANEL_BG, BAR_BG}`; `keyjack::carried`.
- Produces:
  - `containers::ways::free_way(st: &AppState, c: &Container, lock: &Lock) -> Option<Opened>`: `Net` while `lock.node` has a live crack (`sitenet::container_open`), else `Card` when `zone_card_held(st, b) >= max(zone, 1)`, else None.
  - `containers::ways::card_zone(st: &AppState, c: &Container) -> Option<(u8, String)>`: the card a locked container takes (`max(zone, 1)`) and its zone name upper-cased; None off a venue.
  - `containers::ways::reach(c: &Container, you: Pos) -> bool`: floor-standing, `you` 4-beside `at`; wall-set, `at.chebyshev(you) <= 1 && front.chebyshev(you) <= 1` (a terminal's rule, `sitenet::ways::at_port`).
  - `interact::Subject.panel: Option<ContainerAt>`; `gather` adds a subject (header: the kind's word capitalised, `mark_tile`) for each container in `reach`.
  - `interact::Subject::open(&self, st: &AppState) -> Box<dyn Screen>`: `ContainerPanel::open(st, at)` with a `panel`, else `Box::new(self.window(st.tick))`. The explore screen's `[s] => s.window(..)`, the which-way pick and the look-cursor confirm push `s.open(st)`.
  - `screens::container::ContainerPanel::open(st: &AppState, at: ContainerAt) -> ContainerPanel` (a `Screen`). Its first `update`: gone → pop; `Status::Empty` → `st.say("empty")` and pop; `Shut` → `open(st, c, Free)`; `Locked` with a `free_way` → `open(st, c, way)`. Then it shows the locked or the open panel by `status` each frame.
  - Locked rows, each a `PanelRow { key: char, label: String, right: String, right_fg: Color, enabled: bool }`: `j jack in` (only with `lock.node`), `k Keyjack` (right: `"{n} fuses"` from `keyjack::fuses(st)`; greyed `"none"` with no fused Keyjack), `f force it` (right `"Body {need}"`; greyed below it), `c card · {ZONE}` (right `"none"`, greyed; left out off a venue). The first enabled row starts selected; arrows move; a row's letter or space/enter fires it; esc pops. In this task `j`, `k`, `f` fire `st.warn("not yet")` placeholders that Tasks 8-10 replace.
  - Open rows, lettered from `a` (skipping `t`): `cash` / `cr(n)`; `credchip` / `cr(n)` or `LOCKED` in `MAGENTA`; `{item name}` (`×n` above one) / `"{kg:.1} kg"`; `Keycard · {ZONE}` / `"0.0 kg"`. A letter or space takes that line (`take`), `t` takes all, the load `pack {w:.1}/{cap:.0}` sits right of `t take all`; esc closes; the last line taken pops.
  - `keyjack::fuses(st: &AppState) -> u32`: fuses fitted in carried Keyjacks plus loose items whose `hardware.fits == SocketKind::Fuse`.
  - Drawing, as the mock (`a_locked`, `a_open`): `solid` + `frame`, header ` {glyph} {TITLE} ` at x+2 with the glyph in its map colours (detail fg, body bg; Net cyan when on the Net and locked), the title `CYAN` bold; `● NET` in `Color::Rgb(0,230,210)` at the top rule's right end only with `lock.node`; the lock line (`"{maker} lock · grade {g}"` or `"lock · grade {g}"`) in `DIM` two rows down; rows from two rows under it, key at x+2 (`CYAN`, white when selected, `FAINTER` greyed), label at x+5, right text right-aligned at x+w-2 (`DIM`, or its own colour); the selected row on `BAR_BG`; `esc leave it` (locked) or `space take · esc close` (open) in `FAINT` two rows under the last row. Width `max(30, longest row + 8)`, centred like the popup.

- [ ] **Step 1: Write the failing tests** (`screens/container.rs` tests, using `containers::testutil`, `screens::testutil::{buffer_text, key}`, a 100x30 `TestBackend`)

```rust
#[test]
fn a_locked_safe_on_the_net_shows_the_mock() {
    // copy_with_spot(150) re-kinded to a Safe spot; beside(); open; one update
    // text has "◉ SAFE", "● NET", "lock · grade", "jack in", "Keyjack", "force it",
    // "Body ", "card · ", "esc leave it"; the selected row is "jack in"
}

#[test]
fn off_the_net_there_is_no_mark_and_no_jack_in_row() {
    // a plain hotel Locker that the seed locks: no "● NET", no "jack in"; "lock · grade 1";
    // "Keyjack" selected when a fused Keyjack is carried, else "force it"
}

#[test]
fn the_card_opens_it_with_no_locked_panel() {
    // locked room container in zone z: give zone_card_id(b, z); open + update → status is
    // Open/Empty, state.opened == Some(Opened::Card); the drawn text has " · OPEN"
}

#[test]
fn a_live_crack_opens_it_with_no_prompt() {
    // copy_with_spot: sitenet::testutil::hack(st, node, NodeKind::Container, Op::Crack);
    // open + update → opened == Some(Opened::Net)
}

#[test]
fn letters_take_lines_and_t_takes_all() {
    // set left = [Cash(1250), Chip{400,false}, Chip{900,true}, Item frag ×2]; open panel:
    // text has "1,250 cr", "400 cr", "LOCKED", "Frag ×2", "t take all", "pack ";
    // 'a' → credits +1250, 3 lines left; 't' → none left and the panel pops
}

#[test]
fn an_empty_one_says_so() {
    // opened with left = []: open + update pops and the log's last line is "empty"
}

#[test]
fn a_crate_opens_at_once() {
    // a Crate: open + update → opened == Some(Opened::Free), open rows shown
}
```

`interact.rs` tests:

```rust
#[test]
fn a_container_beside_you_is_a_subject_with_a_panel() {
    // beside(c): gather has a subject at c.at.at with panel Some(c.at), named by its word
}

#[test]
fn a_wall_container_is_out_of_reach_from_behind_its_wall() {
    // a wall-set container whose wall tile has floor on its far side (pick one, or make the
    // far tile Floor on the drawn floor and put it back): standing there, no subject for it
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke screens::container:: interact::`
Expected: FAIL, `ContainerPanel` not found.

- [ ] **Step 3: Implement** as the Interfaces say.

- [ ] **Step 4: Run them to see them pass**

Run: `cargo test -p strata-zero --target-dir target/smoke screens::container:: interact:: screens::explore::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero/src/screens crates/strata-zero/src/interact.rs crates/strata-zero/src/containers crates/strata-zero/src/keyjack.rs
git commit -m "containers: the locked and open panels, the card and the crack (#96)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: The Keyjack on a container's lock

**Files:**
- Create: `crates/strata-zero/src/screens/clip.rs`, `crates/strata-zero/src/screens/lockjack.rs`
- Modify: `crates/strata-zero/src/screens/keypad.rs:29-125,209-218,274-366,425-520`, `crates/strata-zero/src/keyjack.rs:105-152`, `crates/strata-zero/src/containers/ways.rs`, `crates/strata-zero/src/screens/container.rs` (`k` row), `crates/strata-zero/src/screens/mod.rs`

**Interfaces:**
- Consumes: Task 7's panel; `ui::slab::{draw, Panel, Shows}`; `credentials::LOCKOUT_MINUTES`.
- Produces:
  - `screens::clip::Clip` (moved from `keypad.rs`, `pub(crate)` fields as today) with `Clip::new(copy: CopyId, run: keyjack::Run, seed: u64, faults: u8, tick: u64) -> Clip` (placed on beat 0), `width`, `marker`, `hits`, `place`, `panel(&self, tick: u64, digits: &[u8]) -> Panel`, `#[cfg(test)] aim_tick(&self, now: u64, hit: bool) -> u64`, and `strike(&mut self, tick: u64) -> Strike` with `pub(crate) enum Strike { Nothing, Hit { beat: usize }, Cracked, Miss }` (judged on `tick - 1`, as today). `Ended` moves with it. `Keypad` keeps its own side effects on `Hit`/`Cracked`/`Miss`; its tests pass unchanged.
  - `keyjack::run_at(st: &AppState, rating: u8, keyjack: &ItemCopy) -> Run`; `run_for(st, b, kj)` = `run_at(st, rating(st, b), kj)`.
  - `containers::ways::{keyjack_now(st: &AppState, at: ContainerAt) -> (u8, Option<u64>), fault(st: &mut AppState, at: ContainerAt) -> u8, lock_out(st: &mut AppState, at: ContainerAt)}`: the container's faults and lockout in its `ContainerState`, read as `credentials::Journal::keypad_now` reads a pad's (an ended lockout reads as no faults); `lock_out` sets `locked_until = now + LOCKOUT_MINUTES` and keeps the faults until then.
  - `containers::ways::keyjack_seed(st: &AppState, c: &Container) -> u64` = `hash64(seed(st, c), 0x4B1A_C0DE)`; the lock's four digits `hash64(seed(st, c), 0xC0DE) % 10_000`.
  - `screens::lockjack::LockJack::open(st: &AppState, at: ContainerAt) -> Option<LockJack>` (None: no fused Keyjack, locked out, or in a fight). Clipped on at once. The face: a `14x7` box in the container's body colour with its glyph, its word upper-cased and `grade {g}`, centred; the slab hangs off it with `slab::draw(buf, face, body, &clip.panel(..))`. Space strikes; `Cracked` → `containers::open(st, c, Opened::Keyjack)` and, on the next key or after `keypad::OPEN_TICKS`, `Replace` with the open `ContainerPanel`; the last `Miss` → `lock_out`, `keyjack::blow_fuse`, warn `"The Keyjack's fuse blew. The lock is out of reach for {LOCKOUT_MINUTES}m."`, stays on FUSE BLOWN until a key, then pops; esc pulls it off (faults stay) and pops.
  - The panel's `k` row: in a fight warns `"not now"`; locked out it is greyed with right `"locked {m}m"`; else `Replace` with `LockJack`.

- [ ] **Step 1: Write the failing tests** (`screens/lockjack.rs` tests)

```rust
#[test]
fn four_hits_open_the_container() {
    // a locked Locker (seed-locked) with the kit's fused Keyjack: LockJack::open, four
    // aim+space → state.opened == Some(Opened::Keyjack); next key replaces it with the panel
}

#[test]
fn the_last_fault_burns_the_fuse_and_locks_out_only_the_keyjack() {
    // misses up to run.faults: fuse gone from the copy, keyjack_now().1 == Some(now +
    // LOCKOUT_MINUTES); LockJack::open → None; the panel's 'k' row greyed "locked 30m";
    // the 'f' row still enabled for enough Body; the card still opens it
}

#[test]
fn a_lockout_run_out_clears_the_faults() {
    // after the lockout: st.net minute moved past locked_until → keyjack_now == (0, None),
    // LockJack::open is Some again
}

#[test]
fn not_in_a_fight() {
    // st.net.engaged = Some(..) (as the keypad's engaged tests make one): the panel's 'k'
    // fires warn "not now"; LockJack::open → None
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke screens::lockjack::`
Expected: FAIL, module missing.

- [ ] **Step 3: Implement** the move of `Clip` first (run `screens::keypad::` to see it
still green), then `run_at`, the ways, `LockJack` and the `k` row.

- [ ] **Step 4: Run them to see them pass**

Run: `cargo test -p strata-zero --target-dir target/smoke screens::lockjack:: screens::keypad:: keyjack:: screens::container::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero/src/screens crates/strata-zero/src/keyjack.rs crates/strata-zero/src/containers
git commit -m "keyjack: the slab on a container's lock (#96)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Jacking in at a container

**Files:**
- Modify: `crates/strata-zero/src/sitenet/ways.rs:53-89`, `crates/strata-zero/src/containers/ways.rs`, `crates/strata-zero/src/screens/container.rs` (`j` row)

**Interfaces:**
- Consumes: `Lock.node` (Task 6); `net::{engaged_on_foot, after_jack_in}`; `st.net.cooling`, `st.net.alert`.
- Produces:
  - `sitenet::jack_in_at(st: &mut AppState, b: BuildingId, port: SiteNodeInfo) -> Result<String, String>`: refuses `"not with a gun on you"` on foot in a fight, `"deck cooling · {left} min"` while cooling, `"port locked down"` while `b` is on alert; then what `jack_in_on_floor` does after finding its port (body left standing, construct made, landing on the node). `jack_in_on_floor` = today's refusal + `port_in_reach` + `jack_in_at`.
  - `containers::ways::jack_in(st: &mut AppState, c: &Container) -> Result<String, String>`: needs `lock.node` (else `"not on the Net"`), finds its `SiteNodeInfo` in `sitenet::net_of`, calls `jack_in_at`. Container nodes are never ports: `sitenet::ways::is_port` and `floor_devices` leave them out.
  - The panel's `j` row: pops with `TransitionKind::Glitch` and logs the line on Ok, warns on Err and stays.

- [ ] **Step 1: Write the failing tests** (`containers/tests.rs`)

```rust
#[test]
fn jacking_in_at_a_container_lands_on_its_node_with_the_body_beside_it() {
    // copy_with_spot(150); beside; jack_in → Ok; player place is Place::SiteNet{b, level};
    // body == Some((Place::Floor{..}, where you stood)); the construct holds its node and you
    // stand on or beside it
}

#[test]
fn an_alarm_locks_the_port_down() {
    // net::lock_down(st, b); jack_in → Err("port locked down")
}

#[test]
fn a_container_node_is_no_port() {
    // floor_devices(st, b, level) has no NodeKind::Container; standing beside a layer spot,
    // sitenet::port_in_reach is None
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke containers::tests::jacking containers::tests::an_alarm containers::tests::a_container_node`
Expected: FAIL, `jack_in` not found.

- [ ] **Step 3: Implement** as the Interfaces say.

- [ ] **Step 4: Run them to see them pass**

Run: `cargo test -p strata-zero --target-dir target/smoke containers:: sitenet::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero/src/sitenet crates/strata-zero/src/containers crates/strata-zero/src/screens/container.rs
git commit -m "containers: jack in at a lock on the Net (#96)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Force

**Files:**
- Create: `crates/strata-zero/src/containers/force.rs`, `crates/strata-zero/src/render/containers.rs` (`ForceBar` only here)
- Modify: `crates/strata-zero/src/app.rs:67-260,~400` (`force_at`), `crates/strata-zero/src/runs/snapshot.rs:111,228` (bind it `_`), `crates/strata-zero/src/screens/explore.rs:153,3640,~4280` (the forcing and its bar), `crates/strata-zero/src/screens/container.rs` (`f` row), `crates/strata-zero/src/render/mod.rs`

**Interfaces:**
- Consumes: `Lock`, `open(.., Opened::Forced)` (alarm on a lock with a node), `ContainerPanel`.
- Produces:
  - `force::need(grade: u8) -> u8` = `2 * grade`; `force::ticks(grade: u8, body: u8) -> u64` = `(secs * FPS) * (100 - 5 * over).max(0) / 100`, at least 1, with `secs` 3, 6, 10 for grades 1, 2, 3 and `over = body - need` (0 below).
  - `force::can(st: &AppState, c: &Container) -> Result<u64, String>`: `"not now"` while `st.net.engaged` is Some; `"Body {need}"` below the need (the `f` row is greyed then anyway); Ok with the ticks.
  - `AppState.force_at: Option<ContainerAt>` — a panel's request, taken by the map on its next frame (like `look_at`); `snapshot`/`restore` bind it `_` ("UI: a panel's request, taken on the map's next frame").
  - `force::Forcing { pub at: ContainerAt, pub from: u64, pub ticks: u64, pub stand: (Place, Pos) }`, `Forcing::frac(&self, tick: u64) -> f32`, `Forcing::done(&self, tick: u64) -> bool`.
  - `force::finish(st: &mut AppState, at: ContainerAt)`: `find`, then `open(st, &c, Opened::Forced)` (the alarm on a lock with a node comes from `open`) and `st.say("the {word} gives")`.
  - `MapScreen.forcing: Option<Forcing>`: started from `st.force_at.take()` (with `can`); any key that frame (then handled as usual), the player off `stand`, or a fight starting stops it (`st.say("you let go")`), and the next try starts the bar over; done → `force::finish(st, at)` and `Push` the open `ContainerPanel`. Guards are untouched.
  - `render::containers::ForceBar { pub at: Pos, pub frac: f32, pub vis: Option<&Grid<Visibility>> }` (`Overlay`): five cells on the row above `at` (below when the row above is off the view), centred, `█` filled in `Rgb(0xff,0xb0,0x00)` and `░` empty in `FAINTER`.
  - The panel's `f` row: `can` Ok → `st.force_at = Some(at)`, pop; Err → warn.

- [ ] **Step 1: Write the failing tests** (`containers/force.rs` and `screens/explore.rs` tests)

```rust
#[test]
fn force_time_by_grade_and_body() {
    assert_eq!(need(1), 2);
    assert_eq!(need(3), 6);
    assert_eq!(ticks(1, 2), 3 * FPS);
    assert_eq!(ticks(2, 4), 6 * FPS);
    assert_eq!(ticks(3, 6), 10 * FPS);
    assert_eq!(ticks(1, 4), 3 * FPS * 90 / 100);
}

#[test]
fn below_the_body_force_is_refused() {
    // a grade-3 safe, Body 4: can() == Err("Body 6"); the panel's 'f' row is greyed
}

#[test]
fn forcing_runs_its_time_and_opens_it() {
    // Body = need; panel 'f'; map.update(st, &[]) each frame with st.tick += 1: not opened
    // before ticks frames, opened == Some(Opened::Forced) after, the open panel pushed
}

#[test]
fn a_step_or_a_key_stops_it_and_it_starts_over() {
    // start; 10 frames; a key → forcing None, nothing opened; start again → from == now
}

#[test]
fn forcing_a_lock_on_the_net_sets_off_the_alarm() {
    // copy_with_spot with enough Body: force to the end → st.net.alert(b, now)
}

#[test]
fn not_in_a_fight() {
    // engaged: the 'f' row warns "not now"; force_at stays None
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke containers::force:: screens::explore::forc`
Expected: FAIL, `force` module missing.

- [ ] **Step 3: Implement** as the Interfaces say; draw `ForceBar` in the explore screen's
overlays beside `floor_devices` while forcing.

- [ ] **Step 4: Run them to see them pass**

Run: `cargo test -p strata-zero --target-dir target/smoke containers:: screens::explore:: runs::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero/src
git commit -m "containers: force a lock with Body, on a bar over it (#96)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: The states on the map and the look

**Files:**
- Modify: `crates/strata-zero/src/render/containers.rs` (`ContainerLooks`), `crates/strata-zero/src/containers/mod.rs` (`looks`, `look_line`), `crates/strata-zero/src/screens/explore.rs:~4280` (overlay), `crates/strata-zero/src/look.rs:866-878`

**Interfaces:**
- Consumes: Task 1's colours and shapes; Task 6's `status`, `lock`.
- Produces:
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum ContainerLook { Open, Empty, Net }` in `containers` (not `Look`: `ui::device::Look` is in scope there); `containers::looks(st: &AppState, b: BuildingId, level: i32) -> Vec<(Pos, ContainerKind, ContainerLook)>`: Open for opened with something left, Empty for opened with nothing, Net for a lock with a `node` not yet opened; closed ones are left to the tileset. Only tiles with a state entry or a node are visited (scan the state map and the site's Net nodes, not the floor).
  - `render::containers::ContainerLooks { pub looks: &[(Pos, ContainerKind, ContainerLook)], pub vis: Option<&Grid<Visibility>> }`: Open → the open shape in the detail colour on the body; Empty → the open shape in `CONTAINER_EMPTY_FG` on `CONTAINER_EMPTY_BG`; Net → the detail glyph in `CONTAINER_NET` on the body; dimmed at 0.45 on remembered tiles, nothing on unseen ones (as `FloorDevices`).
  - `containers::look_line(st: &AppState, c: &Container) -> String`: `"{word} · {maker} grade {g}"` (or `"{word} · grade {g}"`, or `"{word}"` with no lock) then `· locked` / `· open` / `· empty` / `· closed` (no lock, never opened), then `· on the Net` while locked with a node. `look.rs`: a container tile's card heads with the word capitalised and `second = look_line`; the flat's lockers keep the furniture look.

- [ ] **Step 1: Write the failing tests**

`containers/tests.rs`:

```rust
#[test]
fn the_look_names_the_lock_and_the_state() {
    // a plain office's room safe: "safe · megacorp grade 3 · locked" when its maker is
    // megacorp (else "{maker} grade {g}"); a layer spot: ends "· locked · on the Net";
    // opened with lines: ends "· open"; with none: "· empty"; a crate: "crate · closed"
}

#[test]
fn looks_shows_open_empty_and_the_net() {
    // a layer spot not opened → Net; a locker opened with left → Open; with [] → Empty;
    // an untouched crate is absent
}
```

`render/containers.rs` tests:

```rust
#[test]
fn each_state_draws_as_the_key_says() {
    // a 5x1 view, one look each: Open Safe → "○" on #2b2f36; Empty Locker → "▯" in
    // #5b6b7d on #20262e; Net Safe → "◉" in #00e6d2; an unseen tile untouched
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke containers:: render::containers::`
Expected: FAIL, `looks` not found.

- [ ] **Step 3: Implement** as the Interfaces say.

- [ ] **Step 4: Run them to see them pass**

Run: `cargo test -p strata-zero --target-dir target/smoke containers:: render:: look::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero/src
git commit -m "containers: open, empty and on the Net on the map; the look (#96)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Manual, shots and the five checks

**Files:**
- Modify: `manual/06-the-city-and-the-world.md` (a `### Containers` section after `### The Keyjack`, ~L69-82), `manual/07-turns-and-fights.md` (`## Fights indoors`)
- Modify: `crates/strata-zero/src/screens/shots.rs` (new `container_shots`)

**Interfaces:**
- Consumes: everything above.
- Produces: `#[test] #[ignore] fn container_shots()` writing `container_locked_net_{w}x{h}`, `container_locked_{w}x{h}`, `container_open_{w}x{h}`, `container_lockjack_{w}x{h}`, `container_force_{w}x{h}` and `container_map_{w}x{h}` at `(120, 40)` and `(100, 30)` with `shot_stack` as `keyjack_shots` does. The map shot plants (`interiors.put_floor` on a copy of the drawn floor) one of each kind floor-standing and set in a wall, in each state (closed, open, empty, on the Net), the player beside them.

- [ ] **Step 1: Write the manual**

Chapter 06, `### Containers`: the five kinds and how each looks (closed, open, empty, a
cyan detail when its lock is on the Net); `space` beside one opens it, and the locked
panel's ways in: the card of its zone (carry it and it just opens), the Keyjack (`k`,
fuses, a burnt fuse keeps the Keyjack off it for 30 minutes), the Net (crack its node from
anywhere, or `j` to jack in at it while the building isn't on alert), and force (`f`,
Body 2, 4 or 6 by grade, longer for a stronger lock and quicker with more Body, a step or a
key lets go, a lock on the Net raises the alarm); the take panel (`a` on, `t` take all,
the pack's load); credchips (use one in the pack to cash it; a locked one waits for its
crack at your rig). Chapter 07, `## Fights indoors`: one line that forcing a lock and the
Keyjack wait until the fight is over. Only what `manual/README.md` allows (no links, no
italics, keys in `code`).

- [ ] **Step 2: Run the manual's tests**

Run: `cargo test -p strata-zero --target-dir target/smoke manual::`
Expected: PASS (the Markdown draws; the "On foot" keys are unchanged).

- [ ] **Step 3: Write `container_shots` and look at them**

Run: `SHOT_DIR=target/smoke/shots cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored container_shots`
Expected: PASS; open the `.png` files and check them against the mock's `a_locked`,
`a_open` and round 2's map (bodies, details, the `● NET` mark, right-aligned tails).

- [ ] **Step 4: The five checks, both front ends**

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --target-dir target/smoke -- -D warnings
cargo clippy --workspace --all-targets --features strata-zero/window --target-dir target/smoke -- -D warnings
cargo test --workspace --target-dir target/smoke
cargo test --workspace --features strata-zero/window --target-dir target/smoke
```

Expected: all pass, `every_drawn_character_has_a_glyph` among them (`▯ ◉ ○ □ ╳ ╋ ┼ ╍ ┃ ─
█ ░ ●` all drawn).

- [ ] **Step 5: Commit**

```bash
git add manual crates/strata-zero/src/screens/shots.rs
git commit -m "manual: containers, their ways in and force; container shots

Closes #96

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Self-review notes

- Spec coverage: kinds and looks (1, 11); venue rooms, wall and floor (2); layer spots by
  kind, the Net's nodes (3); credchips (4); loot tables and their test (5); rolled once,
  saved, copies fresh, half left, fixtures (6); panels, card, crack, `empty` (7); Keyjack
  with lockout (8); jack in, `port locked down` (9); force, Body, time, alarm, fights (10);
  `look` and map states (11); manual chapters 06 and 07, shots, both front ends (12).
  Classic buildings' lockers and clinics' lockers are containers through `find` (6), with
  `plain` tables; their layouts are untouched.
- Not built, as the spec says: hacking a locked credchip, selling, foe drops, objectives
  in containers (the `Loot::Item` line can carry one), the flat's stash.
