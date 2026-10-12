# City Generator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A seeded, deterministic multi-stratum cyberpunk city generator (`citygen` crate) and a ratatui app (`city` crate) that configures, animates generation, explores, walks, and saves the city, with a Net graph and digital overlay.

**Architecture:** Cargo workspace with two crates. `citygen` is pure data + algorithms: a resumable `Generator` runs eleven chunked stages over a `City` of `Stratum` tile grids plus districts, buildings, connectors and a `Net` graph. `city` renders tiles through a pure `cell()` tileset function with an overlay stack, drives screens from a 30 fps tick loop, and owns the walker, menus, animation primitives and persistence.

**Tech Stack:** Rust edition 2024 (stable), ratatui, crossterm, rand + rand_chacha, noise, serde, postcard, serde_json, ron, thiserror, anyhow, clap, unicode-width (dev).

**Spec:** `docs/superpowers/specs/2026-10-12-city-generator-design.md`

## Global Constraints

- Rust edition 2024, stable toolchain pinned in `rust-toolchain.toml`.
- `citygen` must never depend on ratatui, crossterm or any terminal crate.
- Same `GenParams` (including seed) ⇒ byte-identical `City`. Iterate collections in index order only; never iterate a `HashMap`/`HashSet` to make decisions.
- Every tile renders as exactly one terminal cell: every glyph in `flavor/style.ron` has display width 1 (no full-width CJK, no Hangul).
- Fixed 30 frames per second; all animation is a function of `tick: u64`, never wall-clock.
- Minimum terminal 100×30; smaller shows a centered resize message, never panics.
- Size presets: Small 160×80, Medium 240×120, Large 320×160. Districts 4..=12, default 7.
- Save format: magic `CITY`, `u16` version, postcard. Extension `.city`, directory `./cities/`.
- Gate before every commit: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Commit messages end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Review Focus

1. **Too many districts for a small map** (Small + 12 districts, or `Custom{w:80,h:40}` + 8): `GenParams::validate()` must reject with `InvalidParams { field: "districts" }` instead of the generator panicking or looping forever in seed placement. Pinned in Task 4.
2. **`WaterMode::None` with the Docks rule**: theme assignment must skip Docks cleanly when no seed touches water, and Industrial must still be placed. Pinned in Task 7.
3. **Tiny or shrinking terminal** (1×1, 99×29, resize mid-session): every screen's draw must render the resize message and not index outside the buffer. Pinned in Task 18 (app) and Task 21 (viewport clipping at 1×1).
4. **Corrupt or truncated `.city` file** (wrong magic, future version, truncated body): `load_city` returns a descriptive error, never panics. Pinned in Task 18.
5. **Double-width glyph sneaking into the style pack**: a test asserts every glyph in every glyph set and every terrain/feature mapping has `UnicodeWidthStr::width == 1`. Pinned in Task 20.

---

## File structure

```
Cargo.toml                         workspace
rust-toolchain.toml
flavor/names.ron                   citygen flavor pack (names, signage glyphs)
flavor/style.ron                   city style pack (palettes, glyph sets)
crates/citygen/
  Cargo.toml
  src/lib.rs                       module tree + re-exports
  src/geom.rs                      Pos, Size, Rect, Dir, Grid<T>
  src/ids.rs                       DistrictId, BuildingId, SubnetId, NodeId
  src/tile.rs                      Terrain, Feature, Hue, Tile (+ walkability)
  src/world.rs                     StratumKind, Stratum, Theme, District, BuildingKind, StrataSpan, Building, ConnectorKind, Connector, City
  src/net.rs                       Net, Subnet, SubnetOwner, Node, NodeKind, Link
  src/params.rs                    SizePreset, WaterMode, GenParams, GenError
  src/rng.rs                       stage_rng, tile_hash, hash64
  src/flavor.rs                    FlavorPack (ron), name generators
  src/theme.rs                     ThemeSpec table, StreetStyle, HeightClass rules
  src/pathing.rs                   flood_fill, astar, bresenham (shared)
  src/gen/mod.rs                   Stage, Progress, Generator, GenCtx, Scratch, StageRunner
  src/gen/terrain.rs .. net.rs     one file per stage
  src/validate.rs                  Violation, City::validate
  tests/seeds.rs                   determinism + invariants over seed sets + timing
crates/city/
  Cargo.toml
  src/main.rs                      clap entry
  src/cli.rs                       `gen` subcommand
  src/save.rs                      .city format, list_cities
  src/term.rs                      TerminalGuard + panic hook
  src/app.rs                       AppState, Screen trait, ScreenCmd, run loop
  src/anim/mod.rs                  Easing, Tween, Timeline
  src/anim/effects.rs              glitch, typewriter, flicker, slide
  src/render/mod.rs                Rgb, Cell, RenderCtx, Visibility re-export
  src/render/style.rs              StylePack (ron)
  src/render/tileset.rs            cell()
  src/render/lighting.rs           lit(), flicker factor
  src/render/weather.rs            rain/fog overlay
  src/render/traffic.rs            Traffic state + overlay
  src/render/overlay.rs            digital (Net) overlay
  src/render/viewport.rs           Camera, MapView widget
  src/render/minimap.rs            Minimap widget
  src/ui/mod.rs
  src/ui/logo.rs                   block-letter font + Logo widget
  src/ui/menu.rs                   animated Menu
  src/ui/form.rs                   Form + Field
  src/ui/panel.rs                  inspector, legend, strata selector, status bar
  src/ui/transition.rs             screen transitions
  src/walker/mod.rs                Player, movement, connectors
  src/walker/fov.rs                symmetric shadowcasting
  src/screens/mod.rs
  src/screens/title.rs
  src/screens/configure.rs
  src/screens/generating.rs
  src/screens/explore.rs           Explore + Walk modes over one MapScreen
  src/screens/files.rs
  src/screens/help.rs
```

---

### Task 1: Workspace scaffold

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `crates/citygen/Cargo.toml`, `crates/citygen/src/lib.rs`, `crates/city/Cargo.toml`, `crates/city/src/main.rs`

**Interfaces:**
- Produces: workspace that builds, both crates empty but linked (`city` depends on `citygen`).

- [ ] **Step 1: Write workspace manifests**

`Cargo.toml`:
```toml
[workspace]
resolver = "3"
members = ["crates/citygen", "crates/city"]

[workspace.package]
edition = "2024"
version = "0.1.0"
license = "MIT"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
rand = "0.9"
rand_chacha = "0.9"
noise = "0.9"
thiserror = "2"
anyhow = "1"
ron = "0.10"
postcard = { version = "1", features = ["use-std"] }
serde_json = "1"
ratatui = "0.29"
crossterm = "0.28"
clap = { version = "4", features = ["derive"] }
unicode-width = "0.2"

[profile.dev]
opt-level = 1

[profile.release]
lto = "thin"
```

`rust-toolchain.toml`:
```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

`crates/citygen/Cargo.toml`:
```toml
[package]
name = "citygen"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
rand.workspace = true
rand_chacha.workspace = true
noise.workspace = true
thiserror.workspace = true
ron.workspace = true
```

`crates/city/Cargo.toml`:
```toml
[package]
name = "city"
edition.workspace = true
version.workspace = true
license.workspace = true

[[bin]]
name = "city"
path = "src/main.rs"

[dependencies]
citygen = { path = "../citygen" }
serde.workspace = true
postcard.workspace = true
serde_json.workspace = true
anyhow.workspace = true
ratatui.workspace = true
crossterm.workspace = true
clap.workspace = true
ron.workspace = true

[dev-dependencies]
unicode-width.workspace = true
```

- [ ] **Step 2: Minimal sources**

`crates/citygen/src/lib.rs`: `//! World model and generator for the cyberpunk city.` (empty otherwise).
`crates/city/src/main.rs`: `fn main() { println!("city"); }`

- [ ] **Step 3: Build and run the gate**

Run: `cargo build --workspace && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: success. If a workspace.dependencies version does not resolve, bump to the latest published major and note it in the commit.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock rust-toolchain.toml crates
git commit -m "chore: cargo workspace with citygen and city crates"
```

---

### Task 2: Geometry and grid

**Files:**
- Create: `crates/citygen/src/geom.rs`, `crates/citygen/src/ids.rs`
- Modify: `crates/citygen/src/lib.rs`

**Interfaces:**
- Produces:
  - `Pos { x: i32, y: i32 }` with `new`, `offset(dx,dy)`, `step(Dir)`, `manhattan`, `chebyshev`, `dist2 -> i64`, `neighbors4()`, `neighbors8()`.
  - `Size { w: u32, h: u32 }` with `area() -> usize`, `contains(Pos)`, `index(Pos) -> usize`, `center() -> Pos`.
  - `Rect { x, y, w, h }` (w,h ≥ 0; right/bottom exclusive) with `new`, `right`, `bottom`, `contains`, `intersects`, `intersection -> Option<Rect>`, `inset(n)`, `center`, `area`, `iter()`, `border()`, `clamp_to(Rect) -> Option<Rect>`, `is_empty`.
  - `Dir` 8 variants, `Dir::CARDINAL`, `Dir::ALL`, `delta()`, `opposite()`.
  - `Grid<T>`: `filled(size, v)`, `size()`, `get`, `get_mut`, `set -> bool`, `in_bounds`, `iter() -> (Pos,&T)`, `fill_rect`, `as_slice`, `Index<Pos>`/`IndexMut<Pos>`, `map(|&T| -> U) -> Grid<U>`, `count(pred)`.
  - `DistrictId(u32)`, `BuildingId(u32)`, `SubnetId(u32)`, `NodeId(u32)` each with `index() -> usize`.

- [ ] **Step 1: Write failing tests** (in `geom.rs` `#[cfg(test)] mod tests`)

```rust
#[test]
fn rect_iter_and_border() {
    let r = Rect::new(2, 3, 3, 2);
    assert_eq!(r.area(), 6);
    assert_eq!(r.iter().count(), 6);
    assert_eq!(r.border().count(), 6); // 3x2: all cells are border
    assert!(r.contains(Pos::new(4, 4)));
    assert!(!r.contains(Pos::new(5, 4)));
    assert_eq!(r.inset(1), Rect::new(3, 4, 1, 0));
}
#[test]
fn rect_intersection_and_clamp() {
    let a = Rect::new(0, 0, 10, 10);
    let b = Rect::new(5, 5, 10, 10);
    assert_eq!(a.intersection(b), Some(Rect::new(5, 5, 5, 5)));
    assert_eq!(a.intersection(Rect::new(20, 20, 1, 1)), None);
    assert_eq!(b.clamp_to(a), Some(Rect::new(5, 5, 5, 5)));
}
#[test]
fn grid_set_get_oob() {
    let mut g = Grid::filled(Size { w: 4, h: 3 }, 0u8);
    assert!(g.set(Pos::new(3, 2), 7));
    assert!(!g.set(Pos::new(4, 2), 7));
    assert_eq!(g.get(Pos::new(3, 2)), Some(&7));
    assert_eq!(g.get(Pos::new(-1, 0)), None);
    assert_eq!(g.iter().count(), 12);
    g.fill_rect(Rect::new(0, 0, 2, 2), 9);
    assert_eq!(g.count(|&v| v == 9), 4);
}
#[test]
fn dir_delta_opposite() {
    assert_eq!(Dir::N.delta(), (0, -1));
    assert_eq!(Dir::SE.opposite(), Dir::NW);
    assert_eq!(Pos::new(1, 1).step(Dir::W), Pos::new(0, 1));
    assert_eq!(Pos::new(0, 0).chebyshev(Pos::new(3, -2)), 3);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p citygen`  → compile error (types missing).

- [ ] **Step 3: Implement**

```rust
// geom.rs
use serde::{Deserialize, Serialize};
use std::ops::{Index, IndexMut};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Pos { pub x: i32, pub y: i32 }
impl Pos {
    pub const fn new(x: i32, y: i32) -> Self { Self { x, y } }
    pub fn offset(self, dx: i32, dy: i32) -> Self { Self::new(self.x + dx, self.y + dy) }
    pub fn step(self, d: Dir) -> Self { let (dx, dy) = d.delta(); self.offset(dx, dy) }
    pub fn manhattan(self, o: Pos) -> i32 { (self.x - o.x).abs() + (self.y - o.y).abs() }
    pub fn chebyshev(self, o: Pos) -> i32 { (self.x - o.x).abs().max((self.y - o.y).abs()) }
    pub fn dist2(self, o: Pos) -> i64 { let dx = (self.x - o.x) as i64; let dy = (self.y - o.y) as i64; dx * dx + dy * dy }
    pub fn neighbors4(self) -> impl Iterator<Item = Pos> { Dir::CARDINAL.into_iter().map(move |d| self.step(d)) }
    pub fn neighbors8(self) -> impl Iterator<Item = Pos> { Dir::ALL.into_iter().map(move |d| self.step(d)) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Size { pub w: u32, pub h: u32 }
impl Size {
    pub fn area(self) -> usize { self.w as usize * self.h as usize }
    pub fn contains(self, p: Pos) -> bool { p.x >= 0 && p.y >= 0 && (p.x as u32) < self.w && (p.y as u32) < self.h }
    pub fn index(self, p: Pos) -> usize { p.y as usize * self.w as usize + p.x as usize }
    pub fn center(self) -> Pos { Pos::new(self.w as i32 / 2, self.h as i32 / 2) }
    pub fn rect(self) -> Rect { Rect::new(0, 0, self.w as i32, self.h as i32) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rect { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }
impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self { Self { x, y, w: if w < 0 { 0 } else { w }, h: if h < 0 { 0 } else { h } } }
    pub fn right(self) -> i32 { self.x + self.w }
    pub fn bottom(self) -> i32 { self.y + self.h }
    pub fn is_empty(self) -> bool { self.w == 0 || self.h == 0 }
    pub fn area(self) -> usize { (self.w as usize) * (self.h as usize) }
    pub fn contains(self, p: Pos) -> bool { p.x >= self.x && p.y >= self.y && p.x < self.right() && p.y < self.bottom() }
    pub fn intersects(self, o: Rect) -> bool { self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom() }
    pub fn intersection(self, o: Rect) -> Option<Rect> {
        let x = self.x.max(o.x); let y = self.y.max(o.y);
        let r = self.right().min(o.right()); let b = self.bottom().min(o.bottom());
        if r > x && b > y { Some(Rect::new(x, y, r - x, b - y)) } else { None }
    }
    pub fn clamp_to(self, bounds: Rect) -> Option<Rect> { self.intersection(bounds) }
    pub fn inset(self, n: i32) -> Rect { Rect::new(self.x + n, self.y + n, self.w - 2 * n, self.h - 2 * n) }
    pub fn center(self) -> Pos { Pos::new(self.x + self.w / 2, self.y + self.h / 2) }
    pub fn iter(self) -> impl Iterator<Item = Pos> {
        (self.y..self.bottom()).flat_map(move |y| (self.x..self.right()).map(move |x| Pos::new(x, y)))
    }
    pub fn border(self) -> impl Iterator<Item = Pos> {
        self.iter().filter(move |p| p.x == self.x || p.y == self.y || p.x == self.right() - 1 || p.y == self.bottom() - 1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Dir { N, NE, E, SE, S, SW, W, NW }
impl Dir {
    pub const CARDINAL: [Dir; 4] = [Dir::N, Dir::E, Dir::S, Dir::W];
    pub const ALL: [Dir; 8] = [Dir::N, Dir::NE, Dir::E, Dir::SE, Dir::S, Dir::SW, Dir::W, Dir::NW];
    pub fn delta(self) -> (i32, i32) {
        match self { Dir::N => (0, -1), Dir::NE => (1, -1), Dir::E => (1, 0), Dir::SE => (1, 1),
                     Dir::S => (0, 1), Dir::SW => (-1, 1), Dir::W => (-1, 0), Dir::NW => (-1, -1) }
    }
    pub fn opposite(self) -> Dir {
        match self { Dir::N => Dir::S, Dir::NE => Dir::SW, Dir::E => Dir::W, Dir::SE => Dir::NW,
                     Dir::S => Dir::N, Dir::SW => Dir::NE, Dir::W => Dir::E, Dir::NW => Dir::SE }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grid<T> { size: Size, data: Vec<T> }
impl<T: Clone> Grid<T> {
    pub fn filled(size: Size, v: T) -> Self { Self { size, data: vec![v; size.area()] } }
    pub fn size(&self) -> Size { self.size }
    pub fn in_bounds(&self, p: Pos) -> bool { self.size.contains(p) }
    pub fn get(&self, p: Pos) -> Option<&T> { self.in_bounds(p).then(|| &self.data[self.size.index(p)]) }
    pub fn get_mut(&mut self, p: Pos) -> Option<&mut T> { if self.in_bounds(p) { let i = self.size.index(p); Some(&mut self.data[i]) } else { None } }
    pub fn set(&mut self, p: Pos, v: T) -> bool { match self.get_mut(p) { Some(slot) => { *slot = v; true } None => false } }
    pub fn iter(&self) -> impl Iterator<Item = (Pos, &T)> {
        let w = self.size.w as i32;
        self.data.iter().enumerate().map(move |(i, t)| (Pos::new(i as i32 % w, i as i32 / w), t))
    }
    pub fn fill_rect(&mut self, r: Rect, v: T) { for p in r.iter() { self.set(p, v.clone()); } }
    pub fn as_slice(&self) -> &[T] { &self.data }
    pub fn map<U: Clone>(&self, f: impl Fn(&T) -> U) -> Grid<U> { Grid { size: self.size, data: self.data.iter().map(f).collect() } }
    pub fn count(&self, pred: impl Fn(&T) -> bool) -> usize { self.data.iter().filter(|t| pred(t)).count() }
}
impl<T: Clone> Index<Pos> for Grid<T> { type Output = T; fn index(&self, p: Pos) -> &T { self.get(p).expect("grid index out of bounds") } }
impl<T: Clone> IndexMut<Pos> for Grid<T> { fn index_mut(&mut self, p: Pos) -> &mut T { self.get_mut(p).expect("grid index out of bounds") } }
```

`ids.rs`:
```rust
use serde::{Deserialize, Serialize};
macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct $name(pub u32);
        impl $name { pub fn index(self) -> usize { self.0 as usize } }
    };
}
id_type!(DistrictId); id_type!(BuildingId); id_type!(SubnetId); id_type!(NodeId);
```

`lib.rs`: `pub mod geom; pub mod ids; pub use geom::{Dir, Grid, Pos, Rect, Size}; pub use ids::*;`

- [ ] **Step 4: Run tests** → `cargo test -p citygen` PASS.
- [ ] **Step 5: Commit** `git commit -am "feat(citygen): geometry primitives and Grid"` (after `git add`).

---

### Task 3: Tiles, world and net types

**Files:**
- Create: `crates/citygen/src/tile.rs`, `crates/citygen/src/world.rs`, `crates/citygen/src/net.rs`
- Modify: `crates/citygen/src/lib.rs`

**Interfaces:**
- Produces (all `Serialize, Deserialize, Clone, Debug`; enums `Copy, PartialEq, Eq, Hash`):
  - `Terrain` (20 variants per spec), `Hue { Cyan, Magenta, Pink, Orange, Yellow, Green, Blue, Red, White }`, `Feature` (per spec, `Sign(char)`).
  - `Tile { terrain, feature: Option<Feature>, district: Option<DistrictId>, building: Option<BuildingId>, variant: u8 }`, `Tile::VOID`, `Tile::new(Terrain)`, `is_walkable()`, `blocks_sight()`, `is_outdoor_ground()` (Road|Sidewalk|Alley|Plaza), `is_building_part()` (Wall|Floor|Door|Window).
  - `StratumKind { Undercity, Street, Skyways, Heights, Net }`, `StratumKind::PHYSICAL: [StratumKind;4]`, `label()`, `index() -> usize` (physical order), `from_index`.
  - `Stratum { kind, tiles: Grid<Tile>, light: Grid<u8> }`, `Stratum::new(kind, size)`.
  - `Theme` (8), `Theme::ALL`, `label()`.
  - `District { id, name, theme, center, wealth, decay, bounds }`.
  - `BuildingKind` (19 per spec), `HeightClass { Low, Mid, Tall }`.
  - `StrataSpan { lo, hi }` with `contains(k)`, `iter()`, `len()`, `StrataSpan::from_class(class, basement: bool)`.
  - `Building { id, kind, district, footprint, span, name: Option<String> }`.
  - `ConnectorKind { Stair, Lift, Ladder, Manhole, Jack }`, `Connector { kind, pos, serves: Vec<StratumKind>, public: bool }`, `Connector::other(from) -> Option<StratumKind>` (next/prev not needed; `serves_from(k) -> Option<(Option<StratumKind>, Option<StratumKind>)>` returns (below, above) neighbors in `serves`).
  - `City { seed, params: GenParams, name, size, strata: Vec<Stratum>, districts, buildings, connectors, arterials: Vec<Vec<Pos>>, net: Net }` with `empty(params)`, `stratum(k) -> Option<&Stratum>`, `stratum_mut`, `tile(k, p) -> Option<&Tile>`, `tile_mut`, `connectors_at(p) -> impl Iterator<Item=&Connector>`, `district(id) -> &District`, `building(id) -> &Building`, `walkable(k, p) -> bool`.
  - `Net { subnets, nodes, links: Vec<Link> }`, `Link(pub NodeId, pub NodeId)`, `Subnet { id, owner, security }`, `SubnetOwner`, `Node { id, kind, anchor: (StratumKind, Pos), subnet, building }`, `NodeKind` (10), `Net::nodes_on(k) -> impl Iterator<Item=&Node>`, `Net::node_at(k, p) -> Option<&Node>`.
- Note: `GenParams` is defined in Task 4; `City` references it, so Task 3 includes a minimal `params.rs` with the struct and `Default` only (Task 4 adds validation and `GenError`).

- [ ] **Step 1: Tests**

```rust
// tile.rs tests
#[test]
fn walkability_rules() {
    assert!(Tile::new(Terrain::Sidewalk).is_walkable());
    assert!(!Tile::new(Terrain::Wall).is_walkable());
    assert!(!Tile::new(Terrain::Water).is_walkable());
    let mut t = Tile::new(Terrain::Road);
    t.feature = Some(Feature::Barrier);
    assert!(!t.is_walkable());
    t.feature = Some(Feature::Lamp);
    assert!(t.is_walkable());
    assert!(Tile::new(Terrain::Wall).blocks_sight());
    assert!(Tile::new(Terrain::Door).blocks_sight());
    assert!(!Tile::new(Terrain::Window).blocks_sight());
    assert!(!Tile::new(Terrain::Window).is_walkable());
}
// world.rs tests
#[test]
fn span_iter_and_class() {
    let s = StrataSpan::from_class(HeightClass::Tall, true);
    assert_eq!(s.iter().collect::<Vec<_>>(), StratumKind::PHYSICAL.to_vec());
    let s = StrataSpan::from_class(HeightClass::Mid, false);
    assert_eq!(s.iter().collect::<Vec<_>>(), vec![StratumKind::Street, StratumKind::Skyways]);
    assert!(s.contains(StratumKind::Skyways));
    assert!(!s.contains(StratumKind::Heights));
}
#[test]
fn city_empty_has_four_void_strata() {
    let c = City::empty(GenParams { size: SizePreset::Custom { w: 10, h: 5 }, ..GenParams::default() });
    assert_eq!(c.strata.len(), 4);
    assert_eq!(c.stratum(StratumKind::Heights).unwrap().tiles.size(), Size { w: 10, h: 5 });
    assert_eq!(c.tile(StratumKind::Street, Pos::new(3, 3)).unwrap().terrain, Terrain::Void);
    assert!(c.tile(StratumKind::Street, Pos::new(10, 3)).is_none());
}
#[test]
fn connector_neighbors() {
    let c = Connector { kind: ConnectorKind::Lift, pos: Pos::new(0, 0), serves: vec![StratumKind::Street, StratumKind::Skyways, StratumKind::Heights], public: false };
    assert_eq!(c.serves_from(StratumKind::Skyways), Some((Some(StratumKind::Street), Some(StratumKind::Heights))));
    assert_eq!(c.serves_from(StratumKind::Heights), Some((Some(StratumKind::Skyways), None)));
    assert_eq!(c.serves_from(StratumKind::Undercity), None);
}
```

- [ ] **Step 2: Run** → compile failure.

- [ ] **Step 3: Implement**

Walkability:
```rust
pub fn is_walkable(&self) -> bool {
    use Terrain::*;
    let terrain_ok = matches!(self.terrain, Road | Sidewalk | Alley | Plaza | Floor | Door | Rooftop | Walkway | Bridge | Tunnel | Rail | Stair | Lift | Ladder | Grate);
    let feature_ok = !matches!(self.feature, Some(Feature::Barrier | Feature::Stall | Feature::AcUnit | Feature::Pipe | Feature::Terminal));
    terrain_ok && feature_ok
}
pub fn blocks_sight(&self) -> bool { matches!(self.terrain, Terrain::Wall | Terrain::Door) }
```
`StratumKind::index`: Undercity 0, Street 1, Skyways 2, Heights 3, Net 4. `StrataSpan::from_class`: Low → Street..=Street, Mid → Street..=Skyways, Tall → Street..=Heights; `basement` lowers `lo` to Undercity. `City::empty` builds four `Stratum::new` for `PHYSICAL`, `arterials: vec![]`, `net: Net::default()`, `name: String::new()`, `seed: params.seed`, `size: params.size.size()`.

Minimal `params.rs` for this task:
```rust
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SizePreset { Small, Medium, Large, Custom { w: u32, h: u32 } }
impl SizePreset {
    pub fn size(self) -> Size { match self { Self::Small => Size { w: 160, h: 80 }, Self::Medium => Size { w: 240, h: 120 }, Self::Large => Size { w: 320, h: 160 }, Self::Custom { w, h } => Size { w, h } } }
    pub fn label(self) -> &'static str { match self { Self::Small => "small", Self::Medium => "medium", Self::Large => "large", Self::Custom { .. } => "custom" } }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaterMode { None, Bay, River }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GenParams { pub seed: u64, pub size: SizePreset, pub districts: u8, pub density: f32, pub wealth_skew: f32, pub decay: f32, pub water: WaterMode, pub flavor: String }
impl Default for GenParams { fn default() -> Self { Self { seed: 0, size: SizePreset::Medium, districts: 7, density: 0.6, wealth_skew: 0.0, decay: 0.4, water: WaterMode::Bay, flavor: "default".into() } } }
```

- [ ] **Step 4: Run tests** → PASS. Run clippy gate.
- [ ] **Step 5: Commit** `feat(citygen): tile, world and net data model`.

---

### Task 4: Params validation, errors, RNG and hashing

**Files:**
- Modify: `crates/citygen/src/params.rs`
- Create: `crates/citygen/src/rng.rs`
- Modify: `crates/citygen/src/lib.rs`

**Interfaces:**
- Produces:
  - `GenError` (thiserror): `InvalidParams { field: &'static str, reason: String }`, `Unconnected { stratum: StratumKind }`, `Flavor(String)`, `Invalid(Vec<Violation>)` (Violation defined in Task 17; until then declare `pub struct Violation(pub String)` in `validate.rs` now, Task 17 extends it to an enum).
  - `GenParams::validate(&self) -> Result<(), GenError>`; `GenParams::with_seed(self, u64) -> Self`.
  - `rng::stage_rng(seed: u64, stage: u8) -> ChaCha8Rng`, `rng::hash64(a: u64, b: u64) -> u64` (splitmix-style), `rng::tile_hash(seed, k: StratumKind, p: Pos) -> u8`, `rng::chance(rng, p: f32) -> bool`, `rng::pick_weighted<'a, T>(rng, &'a [(T, u32)]) -> &'a T`.

- [ ] **Step 1: Tests**

```rust
#[test]
fn validate_rejects_bad_params() {
    let bad = GenParams { districts: 13, ..Default::default() };
    assert!(matches!(bad.validate(), Err(GenError::InvalidParams { field: "districts", .. })));
    let bad = GenParams { size: SizePreset::Custom { w: 80, h: 40 }, districts: 8, ..Default::default() };
    assert!(matches!(bad.validate(), Err(GenError::InvalidParams { field: "districts", .. })));
    let bad = GenParams { size: SizePreset::Custom { w: 40, h: 40 }, ..Default::default() };
    assert!(matches!(bad.validate(), Err(GenError::InvalidParams { field: "size", .. })));
    let bad = GenParams { density: 1.5, ..Default::default() };
    assert!(matches!(bad.validate(), Err(GenError::InvalidParams { field: "density", .. })));
    assert!(GenParams::default().validate().is_ok());
    assert!(GenParams { size: SizePreset::Small, districts: 12, ..Default::default() }.validate().is_ok());
}
#[test]
fn rng_is_deterministic_and_stage_isolated() {
    use rand::Rng;
    let a: u64 = stage_rng(42, 3).random(); let b: u64 = stage_rng(42, 3).random();
    let c: u64 = stage_rng(42, 4).random();
    assert_eq!(a, b); assert_ne!(a, c);
    assert_eq!(tile_hash(1, StratumKind::Street, Pos::new(5, 6)), tile_hash(1, StratumKind::Street, Pos::new(5, 6)));
    assert_ne!(tile_hash(1, StratumKind::Street, Pos::new(5, 6)), tile_hash(1, StratumKind::Skyways, Pos::new(5, 6)));
}
```

- [ ] **Step 2: Run** → fail.

- [ ] **Step 3: Implement**

Validation rules: `w` in 80..=1024 and `h` in 40..=512 else field "size"; districts 4..=12 else "districts"; `w*h >= districts*1000` else "districts" with reason "too many districts for map size"; density 0..=1 "density"; wealth_skew -1..=1 "wealth_skew"; decay 0..=1 "decay"; flavor non-empty "flavor".

```rust
// rng.rs
pub fn hash64(mut a: u64, b: u64) -> u64 {
    a ^= b.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    a = (a ^ (a >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    a = (a ^ (a >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    a ^ (a >> 31)
}
pub fn stage_rng(seed: u64, stage: u8) -> ChaCha8Rng { ChaCha8Rng::seed_from_u64(hash64(seed, 0xC17_u64 + stage as u64 * 0x1000)) }
pub fn tile_hash(seed: u64, k: StratumKind, p: Pos) -> u8 {
    (hash64(hash64(seed, k.index() as u64), ((p.x as u32 as u64) << 32) | p.y as u32 as u64) >> 56) as u8
}
pub fn chance(rng: &mut impl Rng, p: f32) -> bool { p > 0.0 && rng.random::<f32>() < p }
pub fn pick_weighted<'a, T>(rng: &mut impl Rng, items: &'a [(T, u32)]) -> &'a T {
    let total: u32 = items.iter().map(|(_, w)| *w).sum();
    let mut roll = rng.random_range(0..total.max(1));
    for (item, w) in items { if roll < *w { return item; } roll -= *w; }
    &items[items.len() - 1].0
}
```

- [ ] **Step 4: Run tests** → PASS; gate.
- [ ] **Step 5: Commit** `feat(citygen): params validation, GenError, seeded rng helpers`.

---

### Task 5: Flavor pack and theme table

**Files:**
- Create: `flavor/names.ron`, `crates/citygen/src/flavor.rs`, `crates/citygen/src/theme.rs`
- Modify: `crates/citygen/src/lib.rs`

**Interfaces:**
- Produces:
  - `FlavorPack` (serde, from RON): `city: NameParts { prefixes, suffixes }`, `districts: Vec<(Theme, NameParts)>` (vector, not map, for ordering), `corp: CorpParts { first: Vec<String>, second: Vec<String>, suffix: Vec<String> }`, `shops: Vec<(BuildingKind, Vec<String>)>` templates with `{syl}` placeholder, `landmarks: Vec<String>` templates with `{corp}` and `{district}`, `signage: Vec<char>`.
  - `FlavorPack::load(name: &str) -> Result<FlavorPack, GenError>` ("default" is `include_str!("../../../flavor/names.ron")`; anything else → `GenError::Flavor`).
  - `FlavorPack::city_name(&self, rng) -> String`, `district_name(theme, rng)`, `corp_name(rng)`, `shop_name(kind, rng) -> Option<String>`, `landmark_name(corp: &str, district: &str, rng)`, `sign_glyph(rng) -> char`.
  - `theme.rs`: `StreetStyle { Grid, Organic, Superblock, Rows, Shattered, Strip }`, `ThemeSpec { base_wealth, base_decay, street_style, block: (i32,i32), min_lot: (i32,i32), mix: &'static [(BuildingKind, u32)], tall: f32, mid: f32, basement: f32, plaza: f32, neon: f32, holo: f32, lamp_spacing: i32, security: u8, hues: &'static [Hue] }`, `Theme::spec(self) -> &'static ThemeSpec`, `BuildingKind::height_classes(self) -> &'static [HeightClass]`, `BuildingKind::label`, `BuildingKind::is_corp(self) -> bool` (CorpTower|Office|Megablock).

- [ ] **Step 1: Tests**

```rust
#[test]
fn default_flavor_loads_and_names_are_deterministic() {
    let f = FlavorPack::load("default").unwrap();
    assert!(FlavorPack::load("nope").is_err());
    let mut a = stage_rng(7, 9); let mut b = stage_rng(7, 9);
    assert_eq!(f.city_name(&mut a), f.city_name(&mut b));
    for t in Theme::ALL { assert!(!f.district_name(t, &mut a).is_empty()); }
    assert!(f.shop_name(BuildingKind::NoodleBar, &mut a).is_some());
    assert!(f.shop_name(BuildingKind::CorpTower, &mut a).is_none());
    assert!(f.signage.iter().all(|c| unicode_single_width(*c)));
}
#[test]
fn theme_mix_weights_nonzero() {
    for t in Theme::ALL { let s = t.spec(); assert!(s.mix.iter().map(|(_, w)| *w).sum::<u32>() > 0); assert!(s.min_lot.0 >= 4 && s.min_lot.1 >= 3); }
    assert_eq!(BuildingKind::CorpTower.height_classes(), &[HeightClass::Tall]);
}
```
`unicode_single_width` in the test: `c as u32 >= 0x20 && !(0x1100..=0x115F).contains(&(c as u32)) && !(0x2E80..=0xA4CF).contains(&(c as u32)) && !(0xAC00..=0xD7A3).contains(&(c as u32)) && !(0xF900..=0xFAFF).contains(&(c as u32)) && !(0xFE30..=0xFE4F).contains(&(c as u32)) && !(0xFF00..=0xFF60).contains(&(c as u32)) && !(0xFFE0..=0xFFE6).contains(&(c as u32))` (half-width katakana U+FF66..FF9D passes). The real width check with `unicode-width` lives in the `city` crate (Task 20).

- [ ] **Step 2: Run** → fail.

- [ ] **Step 3: Implement**

`flavor/names.ron` (abbreviated here; the real file has ≥ 12 entries per list):
```ron
FlavorPack(
  city: NameParts(prefixes: ["Neo", "New", "Night", "Grey", "Ash", "Iron", "Hollow", "Saint", "Port", "North", "Low", "Vanta"],
                  suffixes: ["haven", "port", "gate", "spire", "fall", "reach", "burg", "side", "mouth", "wick", "crest", "shore"]),
  districts: [
    (CorpCore, NameParts(prefixes: ["Apex", "Meridian", "Zenith", "Crown", "Axis", "Prime", "Nexus", "Vector", "Summit", "Core", "Helix", "Aurum"], suffixes: [" Plaza", " Center", " Heights", " District", " Row", " Quarter", " Terrace", " Exchange", " Circle", " Mile", " Spire", " Square"])),
    (NightMarket, NameParts(prefixes: ["Kowloon", "Lantern", "Ginza", "Saigon", "Jade", "Paper", "Smoke", "Bazaar", "Spice", "Dragon", "Lotus", "Silk"], suffixes: [" Market", " Alley", " Walk", " Ward", " Lanes", " Steps", " Street", " Yard", " Court", " Strip", " Row", " Block"])),
    // ... Megablocks, CombatZone, Docks, Industrial, NeonStrip, OldTown
  ],
  corp: CorpParts(first: ["Arasu", "Kiro", "Nov", "Hex", "Mira", "Tess", "Oro", "Vel", "Kai", "Zen", "Dai", "Sol"],
                  second: ["tek", "dyne", "corp", "soft", "gen", "ware", "sys", "line", "med", "sec", "net", "bio"],
                  suffix: ["", " Industries", " Group", " Holdings", " Systems", " Biotech", " Dynamics", " Securities", " Logistics", " Media", " Capital", " Labs"]),
  shops: [
    (NoodleBar, ["{syl} Noodles", "Ramen {syl}", "{syl} Kitchen", "Hot {syl}", "{syl} Bowl"]),
    (Bar, ["The {syl}", "{syl} Lounge", "Bar {syl}", "{syl} & Static", "Dive {syl}"]),
    (Club, ["Club {syl}", "{syl} Afterdark", "Velvet {syl}", "{syl} Pulse"]),
    (Clinic, ["{syl} Ripperdoc", "Clinic {syl}", "{syl} Chrome", "Doc {syl}"]),
    (Shop, ["{syl} Goods", "{syl} Mart", "24h {syl}", "{syl} Supply"]),
    (Pawn, ["{syl} Pawn", "{syl} & Sons", "Cash {syl}"]),
    (Arcade, ["{syl} Arcade", "Neon {syl}", "{syl} Braindance"]),
    (Hotel, ["Hotel {syl}", "{syl} Capsule", "The {syl} Inn"]),
    (Temple, ["{syl} Temple", "Shrine of {syl}", "{syl} Mission"]),
  ],
  landmarks: ["{corp} Tower", "{district} Terminal", "The {district} Arcology", "{corp} Plaza", "Old {district} Station"],
  signage: ['ｦ','ｧ','ｨ','ｩ','ｪ','ｫ','ｬ','ｭ','ｮ','ｯ','ｰ','ｱ','ｲ','ｳ','ｴ','ｵ','ｶ','ｷ','ｸ','ｹ','ｺ','ｻ','ｼ','ｽ','ｾ','ｿ','ﾀ','ﾁ','ﾂ','ﾃ','ﾄ','ﾅ','ﾆ','ﾇ','ﾈ','ﾉ','ﾊ','ﾋ','ﾌ','ﾍ','ﾎ','ﾏ','ﾐ','ﾑ','ﾒ','ﾓ','ﾔ','ﾕ','ﾖ','ﾗ','ﾘ','ﾙ','ﾚ','ﾛ','ﾜ','ﾝ','Ж','Ф','Ц','Ш','Я','Δ','Σ','Ψ','Ω','¥','$','€','§','¤'],
)
```
Name generation: `city_name` = random prefix + random suffix; `district_name` = prefix + suffix from the theme entry; `corp_name` = first + second + suffix (capitalized); `shop_name` replaces `{syl}` with a two-syllable word built from `corp.first` lowercased + `corp.second`, then capitalizes; `landmark_name` substitutes placeholders.

`theme.rs` spec table (one `static` per theme; values below are the ones to use):

| Theme | wealth | decay | style | block | min_lot | tall | mid | basement | plaza | neon | holo | lamp | security | hues |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| CorpCore | .9 | .1 | Grid | (24,12) | (10,7) | .7 | .3 | .6 | .25 | .3 | .5 | 8 | 200 | Cyan,White,Blue |
| NightMarket | .4 | .5 | Organic | (16,9) | (5,4) | .05 | .3 | .1 | .3 | .8 | .3 | 12 | 60 | Magenta,Orange,Yellow |
| Megablocks | .3 | .6 | Superblock | (40,20) | (18,10) | .9 | .1 | .5 | .1 | .2 | .2 | 10 | 110 | Yellow,White,Orange |
| CombatZone | .1 | .9 | Shattered | (16,10) | (5,4) | .05 | .2 | .1 | .15 | .3 | .05 | 16 | 20 | Red,Yellow,Orange |
| Docks | .3 | .6 | Rows | (30,14) | (12,8) | .0 | .2 | .2 | .1 | .2 | .05 | 14 | 70 | Cyan,Green,Orange |
| Industrial | .25 | .7 | Rows | (28,14) | (12,8) | .05 | .3 | .4 | .05 | .15 | .05 | 14 | 90 | Yellow,Green,White |
| NeonStrip | .6 | .3 | Strip | (18,10) | (6,4) | .2 | .5 | .2 | .2 | .95 | .6 | 6 | 120 | Pink,Blue,Magenta |
| OldTown | .5 | .4 | Organic | (14,9) | (5,4) | .0 | .25 | .15 | .25 | .4 | .1 | 12 | 80 | Orange,Green,White |

Mixes (kind, weight): CorpCore {CorpTower 5, Office 4, Hotel 1, Parking 1}; NightMarket {Shop 4, NoodleBar 4, Bar 2, Market 2, Clinic 1, Pawn 1, Apartment 2}; Megablocks {Megablock 6, Apartment 3, Shop 1}; CombatZone {Apartment 3, Shop 2, Bar 2, Pawn 2, Clinic 1, Garage 1}; Docks {Warehouse 6, Depot 2, Bar 1, Garage 1}; Industrial {Factory 5, Warehouse 3, Depot 2, Garage 1}; NeonStrip {Club 4, Bar 3, Arcade 3, Hotel 2, Shop 1}; OldTown {Apartment 4, Shop 2, Temple 1, NoodleBar 2, Bar 1, Clinic 1}.

Height classes: CorpTower [Tall]; Megablock [Tall]; Office [Mid, Tall]; Hotel [Mid, Tall]; Apartment [Mid, Low]; Factory [Low, Mid]; Parking [Low, Mid]; everything else [Low]. Selection: pick from allowed list using theme `tall`/`mid` chances where applicable, else Low.

- [ ] **Step 4: Run tests** → PASS; gate.
- [ ] **Step 5: Commit** `feat(citygen): flavor pack and theme table`.

---

### Task 6: Pathing helpers and generator skeleton with Terrain stage

**Files:**
- Create: `crates/citygen/src/pathing.rs`, `crates/citygen/src/gen/mod.rs`, `crates/citygen/src/gen/terrain.rs`
- Modify: `crates/citygen/src/lib.rs`

**Interfaces:**
- Produces:
  - `pathing::flood_fill(size: Size, start: Pos, passable: impl Fn(Pos)->bool) -> Vec<Pos>` (4-connected).
  - `pathing::components(size, passable) -> Vec<Vec<Pos>>` (all 4-connected components, largest first, deterministic order by first cell index).
  - `pathing::astar(size, start, goal, cost: impl Fn(Pos, Pos)->Option<u32>) -> Option<Vec<Pos>>` (4-neighbour; `cost(from, to)` returns `None` for impassable; includes start and goal).
  - `pathing::bfs_nearest(size, start, passable: Fn(Pos)->bool, target: Fn(Pos)->bool, max: usize) -> Option<Vec<Pos>>` (shortest 4-path through passable to the nearest target; path includes start and the target).
  - `pathing::bresenham(a: Pos, b: Pos) -> Vec<Pos>`.
  - `gen::Stage` (11 variants, `Stage::ALL`, `label()`, `index() -> u8`), `gen::Progress { Running { stage: Stage, pct: f32 }, Done }`.
  - `gen::Generator::{new(params) -> Result<Self, GenError>, step(&mut self) -> Progress, city(&self) -> &City, finish(self) -> Result<City, GenError>, run(params) -> Result<City, GenError>, error(&self) -> Option<&GenError>}`.
  - `pub(crate) struct GenCtx { city: City, flavor: FlavorPack, seed: u64, scratch: Scratch }`, `pub(crate) struct Scratch { land: Grid<bool>, wealth: Grid<f32>, decay: Grid<f32>, arterial: Grid<bool>, road: Grid<bool>, blocks: Vec<Block>, lots: Vec<Lot>, water_edge: Option<Dir> }`, `Block { rect: Rect, district: DistrictId }`, `Lot { rect, district, use_: LotUse }`, `LotUse { Building { kind, class, basement }, Plaza, Parking }`.
  - `pub(crate) trait StageRunner { fn begin(&mut self, ctx: &mut GenCtx) -> Result<(), GenError>; fn step(&mut self, ctx: &mut GenCtx) -> Result<StepOut, GenError>; }`, `StepOut { Continue(f32), Finished }`.
  - `pub(crate) fn runner_for(stage: Stage) -> Box<dyn StageRunner>` (stages not yet implemented return a `NoopStage` that finishes immediately; each later task replaces its entry).
  - `GenCtx` helpers: `rng(&self, stage) -> ChaCha8Rng`, `size()`, `set_terrain(k, p, Terrain)`, `terrain(k, p) -> Terrain` (Void if OOB), `set_feature`, `in_district(p, id) -> bool`.

- [ ] **Step 1: Tests**

```rust
// pathing.rs
#[test]
fn astar_routes_around_block() {
    let size = Size { w: 5, h: 5 };
    let wall = |p: Pos| p.x == 2 && p.y != 4;
    let path = astar(size, Pos::new(0, 0), Pos::new(4, 0), |_, to| if wall(to) { None } else { Some(1) }).unwrap();
    assert_eq!(path[0], Pos::new(0, 0)); assert_eq!(*path.last().unwrap(), Pos::new(4, 0));
    assert!(path.iter().all(|p| !wall(*p)));
    assert_eq!(path.len(), 13);
}
#[test]
fn components_are_largest_first() {
    let size = Size { w: 6, h: 1 };
    let pass = |p: Pos| p.x != 2; // {0,1} and {3,4,5}
    let c = components(size, pass);
    assert_eq!(c.len(), 2); assert_eq!(c[0].len(), 3); assert_eq!(c[1].len(), 2);
}
#[test]
fn bfs_nearest_finds_target() {
    let size = Size { w: 10, h: 1 };
    let p = bfs_nearest(size, Pos::new(0, 0), |_| true, |p| p.x == 7, 20).unwrap();
    assert_eq!(p.len(), 8);
    assert!(bfs_nearest(size, Pos::new(0, 0), |_| true, |p| p.x == 7, 3).is_none());
}
// gen/mod.rs
#[test]
fn generator_walks_all_stages_in_order() {
    let mut g = Generator::new(GenParams { seed: 1, size: SizePreset::Custom { w: 80, h: 40 }, districts: 4, ..Default::default() }).unwrap();
    let mut seen = vec![];
    loop { match g.step() { Progress::Running { stage, pct } => { assert!((0.0..=1.0).contains(&pct)); if seen.last() != Some(&stage) { seen.push(stage); } } Progress::Done => break } }
    assert_eq!(seen, Stage::ALL.to_vec());
}
// gen/terrain.rs
#[test]
fn bay_covers_ten_to_twenty_percent_of_street() {
    for seed in 0..5 {
        let c = Generator::run(GenParams { seed, water: WaterMode::Bay, size: SizePreset::Custom { w: 120, h: 60 }, districts: 4, ..Default::default() }).unwrap();
        let st = c.stratum(StratumKind::Street).unwrap();
        let water = st.tiles.count(|t| t.terrain == Terrain::Water) as f32 / st.tiles.size().area() as f32;
        assert!((0.08..=0.25).contains(&water), "seed {seed}: {water}");
    }
    let c = Generator::run(GenParams { seed: 3, water: WaterMode::None, size: SizePreset::Custom { w: 120, h: 60 }, districts: 4, ..Default::default() }).unwrap();
    assert_eq!(c.stratum(StratumKind::Street).unwrap().tiles.count(|t| t.terrain == Terrain::Water), 0);
}
```
Until later stages exist, `Generator::run` passes because `finish()` only validates when all stages are real; implement `finish()` as: if `error` → Err; else `City::validate` (Task 17 adds it; until then `validate` returns Ok). Note the terrain test calls `Generator::run` with the Noop stages; it stays green through later tasks.

- [ ] **Step 2: Run** → fail.

- [ ] **Step 3: Implement**

A* uses `BinaryHeap<Reverse<(u32, u32, Pos)>>` keyed by `f = g + manhattan`, with a tie-break counter for determinism, `came_from: Grid<Option<Pos>>`, `g: Grid<u32>` initialised to `u32::MAX`. Flood fill and BFS use `VecDeque` and a `Grid<bool>` visited.

Generator step machinery:
```rust
pub struct Generator { ctx: GenCtx, stage_idx: usize, runner: Option<Box<dyn StageRunner>>, error: Option<GenError>, done: bool }
impl Generator {
    pub fn new(params: GenParams) -> Result<Self, GenError> {
        params.validate()?;
        let flavor = FlavorPack::load(&params.flavor)?;
        let city = City::empty(params.clone());
        let size = city.size;
        let scratch = Scratch::new(size);
        Ok(Self { ctx: GenCtx { seed: params.seed, city, flavor, scratch }, stage_idx: 0, runner: None, error: None, done: false })
    }
    pub fn step(&mut self) -> Progress {
        if self.done || self.error.is_some() { return Progress::Done; }
        let stage = Stage::ALL[self.stage_idx];
        if self.runner.is_none() {
            let mut r = runner_for(stage);
            if let Err(e) = r.begin(&mut self.ctx) { self.error = Some(e); return Progress::Done; }
            self.runner = Some(r);
        }
        match self.runner.as_mut().unwrap().step(&mut self.ctx) {
            Ok(StepOut::Continue(pct)) => Progress::Running { stage, pct: pct.clamp(0.0, 1.0) },
            Ok(StepOut::Finished) => {
                self.runner = None; self.stage_idx += 1;
                if self.stage_idx == Stage::ALL.len() { self.done = true; Progress::Done } else { Progress::Running { stage: Stage::ALL[self.stage_idx], pct: 0.0 } }
            }
            Err(e) => { self.error = Some(e); Progress::Done }
        }
    }
    pub fn finish(self) -> Result<City, GenError> {
        if let Some(e) = self.error { return Err(e); }
        self.ctx.city.validate().map_err(GenError::Invalid)?;
        Ok(self.ctx.city)
    }
    pub fn run(params: GenParams) -> Result<City, GenError> { let mut g = Self::new(params)?; while let Progress::Running { .. } = g.step() {} g.finish() }
}
```

Terrain stage (`terrain.rs`): `begin` chooses the water edge with the stage RNG (`Bay`: random `Dir::CARDINAL`; `River`: random axis). Work is per row: `step` processes one row `y`, sets `land[p]` and `Water` on Street for water tiles, `pct = y / h`. Bay: depth at coordinate `t` along the edge = `base + noise`, with `base = 0.15 * extent` and `noise` from OpenSimplex at `t * 0.04` scaled by `0.06 * extent`; tiles within that depth from the edge are water. River: centre line `c(t) = mid + noise(t*0.02) * 0.25 * extent`, half-width `3..=5` from noise; tiles within half-width of `c(t)` are water. After the last row, if water fraction < 0.08 for Bay, widen by 1 tile along the whole edge until ≥ 0.10 (bounded loop of at most 10 widenings). Non-water Street tiles stay `Void` for now (later stages paint them); `land` is true for them.

- [ ] **Step 4: Run tests** → PASS; gate.
- [ ] **Step 5: Commit** `feat(citygen): generator state machine, pathing helpers, terrain stage`.

---

### Task 7: Districts stage

**Files:**
- Create: `crates/citygen/src/gen/districts.rs`
- Modify: `crates/citygen/src/gen/mod.rs` (`runner_for`)

**Interfaces:**
- Consumes: `Scratch.land`, `GenCtx.rng(Stage::Districts)`, `Theme::spec`.
- Produces: `City.districts` filled (ids 0..n, `bounds`, `center`, `theme`, `wealth`, `decay`, empty `name`), `tile.district` set on all four strata for land tiles, `Scratch.wealth`/`Scratch.decay` per-tile fields.

- [ ] **Step 1: Tests**

```rust
fn small(seed: u64, water: WaterMode, districts: u8) -> City {
    Generator::run(GenParams { seed, water, districts, size: SizePreset::Custom { w: 120, h: 60 }, ..Default::default() }).unwrap()
}
#[test]
fn every_land_tile_has_a_district_and_counts_match() {
    let c = small(5, WaterMode::Bay, 6);
    assert_eq!(c.districts.len(), 6);
    let st = c.stratum(StratumKind::Street).unwrap();
    for (_, t) in st.tiles.iter() { if t.terrain != Terrain::Water { assert!(t.district.is_some()); } else { assert!(t.district.is_none()); } }
    let mut ids: Vec<_> = c.districts.iter().map(|d| d.id).collect(); ids.dedup(); assert_eq!(ids.len(), 6);
}
#[test]
fn theme_rules_hold() {
    let c = small(11, WaterMode::Bay, 7);
    let themes: Vec<_> = c.districts.iter().map(|d| d.theme).collect();
    assert_eq!(themes.iter().filter(|t| **t == Theme::CorpCore).count(), 1);
    assert_eq!(themes.iter().filter(|t| **t == Theme::Docks).count(), 1);
    assert!(themes.contains(&Theme::Industrial) && themes.contains(&Theme::CombatZone));
    let core = c.districts.iter().find(|d| d.theme == Theme::CorpCore).unwrap();
    let cz = c.districts.iter().find(|d| d.theme == Theme::CombatZone).unwrap();
    let far = c.districts.iter().filter(|d| d.theme != Theme::CorpCore).map(|d| d.center.dist2(core.center)).max().unwrap();
    assert_eq!(cz.center.dist2(core.center), far);
}
#[test]
fn no_water_means_no_docks_but_industrial_present() {
    let c = small(2, WaterMode::None, 5);
    assert!(c.districts.iter().all(|d| d.theme != Theme::Docks));
    assert!(c.districts.iter().any(|d| d.theme == Theme::Industrial));
}
#[test]
fn wealth_and_decay_are_clamped() {
    let c = small(9, WaterMode::River, 8);
    for d in &c.districts { assert!((0.0..=1.0).contains(&d.wealth)); assert!((0.0..=1.0).contains(&d.decay)); }
}
```

- [ ] **Step 2: Run** → fail.

- [ ] **Step 3: Implement**

`begin`: collect land positions in index order; `n = params.districts`; `spacing = (land.len() as f32 / n as f32).sqrt() * 0.7`; place seeds by rejection: up to 4000 attempts drawing a random land index, accept if `chebyshev ≥ spacing` to all accepted; if fewer than n after attempts, multiply spacing by 0.8 and continue (loop ends because spacing → 0 accepts everything). Queue rows for Voronoi assignment.

`step` (one row per step): for each land `p` in the row, `nearest = argmin dist2(seed_i)` (ties → lower index); write `district_of`. After last row: compute `bounds` (min/max per district), `center = seed`; assign themes:
```rust
let centroid = mean of land positions;
let core = argmin dist2(seed, centroid);                 // CorpCore
let touches_water = |i| district i has a land tile with a 4-neighbour that is Water;
let docks = seeds (≠core) with touches_water, pick the one with most water-adjacent tiles; // Option
let industrial = if let Some(d) = docks { argmin over others of bounds-centre distance to d } else { the seed nearest a map edge (≠core) };
let combat = argmax dist2(seed, core) among unassigned;
remaining in index order draw from [Megablocks, NightMarket, NeonStrip, OldTown] cycling.
```
Then `wealth = clamp(spec.base_wealth + params.wealth_skew*0.3 + noise2d(center*0.01)*0.15)`, `decay = clamp(spec.base_decay + (params.decay-0.4)*0.5 + noise*0.15)`. Per-tile fields: `wealth[p] = district.wealth + noise(p*0.03)*0.1`, same for decay. Write `tile.district` for all four strata (land tiles only).

- [ ] **Step 4: Run tests** → PASS; gate.
- [ ] **Step 5: Commit** `feat(citygen): districts stage with themed Voronoi regions`.

---

### Task 8: Arterials stage

**Files:**
- Create: `crates/citygen/src/gen/arterials.rs`
- Modify: `gen/mod.rs`

**Interfaces:**
- Consumes: `City.districts`, `Scratch.land`, `pathing::astar`.
- Produces: `City.arterials: Vec<Vec<Pos>>` (centre paths), Street `Road` 4 wide + `Sidewalk` ring, `Scratch.arterial` and `Scratch.road` masks.

- [ ] **Step 1: Tests**

```rust
#[test]
fn arterials_connect_all_districts_and_avoid_water() {
    let c = small(21, WaterMode::Bay, 7);
    assert!(c.arterials.len() >= 6);
    let st = c.stratum(StratumKind::Street).unwrap();
    for path in &c.arterials { for p in path { assert_ne!(st.tiles[*p].terrain, Terrain::Water); } }
    // every district centre is within 3 tiles of a road
    for d in &c.districts {
        let near = Rect::new(d.center.x - 3, d.center.y - 3, 7, 7).iter().any(|p| st.tiles.get(p).is_some_and(|t| t.terrain == Terrain::Road));
        assert!(near, "district {:?} centre not on road", d.id);
    }
    // roads form one 4-connected component
    let comps = components(st.tiles.size(), |p| st.tiles[p].terrain == Terrain::Road);
    assert_eq!(comps.len(), 1);
}
#[test]
fn arterial_width_is_four_with_sidewalks() {
    let c = small(21, WaterMode::None, 5);
    let st = c.stratum(StratumKind::Street).unwrap();
    let path = &c.arterials[0];
    let mid = path[path.len() / 2];
    let roads = Rect::new(mid.x - 3, mid.y - 3, 7, 7).iter().filter(|p| st.tiles[*p].terrain == Terrain::Road).count();
    assert!(roads >= 16, "expected a 4-wide road around {mid:?}, got {roads}");
    assert!(st.tiles.count(|t| t.terrain == Terrain::Sidewalk) > 0);
}
```

- [ ] **Step 2: Run** → fail.

- [ ] **Step 3: Implement**

`begin`: Prim's MST over district centres (dist2); extra edges = the `max(2, n/3)` shortest non-MST pairs. Queue edges in order. `step`: route one edge with `astar` where `cost(from,to)`: `None` if `!land[to]`; base 10; `+1` if `!arterial[to]` (prefer reuse) else `0`; turn penalty: since `cost` only sees `from,to`, bias straightness by using the A* heuristic plus `+3` when `to` changes axis relative to `from - came_from` — implement by routing on an expanded state is overkill; instead post-smooth: after A*, replace any 1-tile zigzag `(x,y)->(x+1,y)->(x+1,y+1)->(x+2,y+1)` by keeping as is (zigzags render fine as 4-wide roads). Paint: for each path point `p`, `fill_rect(Rect::new(p.x-1, p.y-1, 4, 4), Road)` on land only, `arterial[p']=true`, `road[p']=true`; then second pass per edge: ring `Rect::new(p.x-2,p.y-2,6,6).border()` → `Sidewalk` where terrain is `Void` and land. Push the path into `city.arterials`. `pct = edge_idx / edges`.

- [ ] **Step 4: Run tests** → PASS; gate.
- [ ] **Step 5: Commit** `feat(citygen): arterial road network`.

---

### Task 9: Streets stage

**Files:**
- Create: `crates/citygen/src/gen/streets.rs`
- Modify: `gen/mod.rs`

**Interfaces:**
- Consumes: districts, `Scratch.land/arterial/road`, `Theme::spec().street_style/block`.
- Produces: Street `Road`/`Alley`/`Sidewalk` tiles per district style; `Scratch.blocks: Vec<Block>` (rects fully inside the district, not overlapping streets; at least 5×4); `Barrier` features in CombatZone at cut streets; `Scratch.road` updated.

- [ ] **Step 1: Tests**

```rust
#[test]
fn blocks_are_inside_districts_and_off_roads() {
    let c = small(31, WaterMode::Bay, 7);
    let st = c.stratum(StratumKind::Street).unwrap();
    let blocks = debug_blocks(&c); // test-only helper exported under #[cfg(test)]: re-run Generator to the Streets stage and read scratch
    assert!(blocks.len() > 20);
    for b in &blocks {
        assert!(b.rect.w >= 5 && b.rect.h >= 4);
        for p in b.rect.iter() { let t = &st.tiles[p]; assert_eq!(t.district, Some(b.district)); assert!(!matches!(t.terrain, Terrain::Road | Terrain::Alley | Terrain::Water)); }
    }
}
#[test]
fn street_network_is_connected_per_street_level() {
    let c = small(31, WaterMode::None, 6);
    let st = c.stratum(StratumKind::Street).unwrap();
    let comps = components(st.tiles.size(), |p| matches!(st.tiles[p].terrain, Terrain::Road | Terrain::Alley | Terrain::Sidewalk));
    assert_eq!(comps[0].len() as f32 / comps.iter().map(|c| c.len()).sum::<usize>() as f32 > 0.97, true);
}
#[test]
fn combat_zone_has_barriers_and_grid_has_none() {
    let c = small(31, WaterMode::None, 7);
    let st = c.stratum(StratumKind::Street).unwrap();
    let count = |theme: Theme| st.tiles.iter().filter(|(_, t)| t.feature == Some(Feature::Barrier) && t.district.map(|d| c.district(d).theme) == Some(theme)).count();
    assert!(count(Theme::CombatZone) > 0);
    assert_eq!(count(Theme::CorpCore), 0);
}
```
`debug_blocks`: in `gen/mod.rs` add `#[cfg(test)] pub(crate) fn run_until(params, stage: Stage) -> GenCtx` that steps until the given stage finishes and returns the ctx.

- [ ] **Step 2: Run** → fail.

- [ ] **Step 3: Implement**

Per district (one district per `step`), on `bounds`:
- Helper `paint_line(ctx, a: Pos, b: Pos, width: i32, terrain: Terrain, sidewalk: bool, district)`: for each bresenham point stamp a `width`-wide perpendicular span; only on land tiles of this district whose terrain is `Void` (never overwrite arterial Road/Sidewalk); if `sidewalk`, add a `Sidewalk` on each side where `Void`.
- `carve_grid(rect, bw, bh, street_w, sidewalk)`: vertical lines every `bw + street_w + 2*sidewalk` and horizontal every `bh + ...` across the district bounds; leaf blocks = cells between lines.
- Styles:
  - `Grid` (CorpCore): `carve_grid(block=(24,12), street_w 2, sidewalk)`.
  - `Superblock`: `carve_grid((40,20), 2, sidewalk)`.
  - `Organic`: BSP of `bounds`: split the longer axis at `0.35..0.65` jitter while `w > 2*max_w` or `h > 2*max_h` with `max = block + block/2`, min leaf 8×6; each split paints a 1-wide `Alley` line (no sidewalks); leaf blocks = leaf rect inset by 1 on the alley side(s). 20% of leaves additionally get a dead-end alley stub of length `w/2` from one edge.
  - `Rows`: horizontal `Road` lines (2 wide + sidewalk) every 14 rows; vertical access `Road` every 24 columns.
  - `Shattered`: `carve_grid((16,10), 2, false)`, then for each horizontal/vertical street segment between intersections with probability 0.3: repaint it `Alley` and set `Feature::Barrier` on the two end tiles.
  - `Strip`: one `Road` 3 wide + sidewalks along the longer axis through `center`; perpendicular `Alley` 1 wide every 10 tiles spanning the bounds.
- Block extraction: for each candidate leaf rect, `clamp_to(bounds)`, shrink until no cell is road/water/outside district (shrink from the offending side, up to 4 times), require `w≥5,h≥4` and `≥ 85%` of cells in this district and land; push `Block`.
- Leftover `Void` land tiles inside the district bounds that are adjacent to a street stay `Void` for Task 10 to turn into `Alley`.

- [ ] **Step 4: Run tests** → PASS; gate.
- [ ] **Step 5: Commit** `feat(citygen): per-theme street subdivision and blocks`.

---

### Task 10: Lots and buildings stage

**Files:**
- Create: `crates/citygen/src/gen/lots.rs`
- Modify: `gen/mod.rs`

**Interfaces:**
- Consumes: `Scratch.blocks`, `Theme::spec().{min_lot,mix,tall,mid,basement,plaza}`, `BuildingKind::height_classes`, `params.density`.
- Produces: `Scratch.lots`, `City.buildings` (Street footprint painted `Wall` border + `Floor` interior, `tile.building` set on all strata in span... footprint painted on Street only here; Task 11 paints other strata), `Plaza`/`Parking`(as `Plaza` terrain with `Parking` lot use) tiles, leftover land `Void` → `Alley`.

- [ ] **Step 1: Tests**

```rust
#[test]
fn buildings_fit_blocks_and_do_not_overlap() {
    let c = small(41, WaterMode::Bay, 7);
    assert!(c.buildings.len() > 40);
    for (i, a) in c.buildings.iter().enumerate() {
        assert!(a.footprint.w >= 4 && a.footprint.h >= 3);
        for b in &c.buildings[i + 1..] { assert!(!a.footprint.intersects(b.footprint), "{:?} overlaps {:?}", a.id, b.id); }
        let st = c.stratum(StratumKind::Street).unwrap();
        for p in a.footprint.border() { assert_eq!(st.tiles[p].terrain, Terrain::Wall); assert_eq!(st.tiles[p].building, Some(a.id)); }
        for p in a.footprint.inset(1).iter() { assert_eq!(st.tiles[p].terrain, Terrain::Floor); }
        assert_eq!(st.tiles[a.footprint.center()].district, Some(a.district));
    }
}
#[test]
fn corp_core_has_tall_buildings_and_no_void_left_on_land() {
    let c = small(41, WaterMode::None, 7);
    let core = c.districts.iter().find(|d| d.theme == Theme::CorpCore).unwrap().id;
    assert!(c.buildings.iter().any(|b| b.district == core && b.span.hi == StratumKind::Heights));
    let st = c.stratum(StratumKind::Street).unwrap();
    assert_eq!(st.tiles.iter().filter(|(_, t)| t.terrain == Terrain::Void && t.district.is_some()).count(), 0);
}
#[test]
fn density_changes_plaza_share() {
    let lo = Generator::run(GenParams { seed: 4, density: 0.2, size: SizePreset::Custom { w: 120, h: 60 }, districts: 5, ..Default::default() }).unwrap();
    let hi = Generator::run(GenParams { seed: 4, density: 0.95, size: SizePreset::Custom { w: 120, h: 60 }, districts: 5, ..Default::default() }).unwrap();
    assert!(lo.buildings.len() < hi.buildings.len());
}
```

- [ ] **Step 2: Run** → fail.

- [ ] **Step 3: Implement**

One block per `step`. BSP the block rect into lots: split the longer axis while `w > 2*min_w` or `h > 2*min_h` at a jittered midpoint (0.4..0.6), stop when a leaf is within `[min, 2*min)`; lots with `w<4||h<3` are merged into Plaza. For each lot: `plaza_p = spec.plaza * (1.3 - density)`; if `chance(plaza_p)` → `LotUse::Plaza` (paint `Plaza`), with half of those `Parking` in CorpCore/Industrial (still `Plaza` terrain, `tile.feature` none). Else `kind = pick_weighted(mix)`, `class`: allowed = `kind.height_classes()`; if allowed contains Tall and `chance(spec.tall)` → Tall; else if contains Mid and `chance(spec.mid)` → Mid; else first allowed with preference Low. `basement = chance(spec.basement)`. Footprint = lot rect, but in Organic/Strip styles inset by 1 on a random side with p=0.3 to create gaps (gaps become `Alley`). Paint on Street: border `Wall`, inner `Floor`, set `tile.building` for all footprint tiles on **every** stratum in `span` (terrain on other strata painted in Task 11). Push `Building { id, kind, district, footprint, span: StrataSpan::from_class(class, basement), name: None }`. After the last block: every Street tile with `district.is_some()` and terrain `Void` → `Alley`.

- [ ] **Step 4: Run tests** → PASS; gate.
- [ ] **Step 5: Commit** `feat(citygen): lots and building placement`.

---

### Task 11: Verticals stage

**Files:**
- Create: `crates/citygen/src/gen/verticals.rs`
- Modify: `gen/mod.rs`

**Interfaces:**
- Consumes: `City.arterials`, `Scratch.blocks/road`, buildings with spans.
- Produces: Undercity `Tunnel`/`Sewage`/`Grate`/`Rail` + basement `Wall`/`Floor`; Skyways `Wall`/`Floor` footprints + `Walkway`; Heights `Rooftop` + `Bridge` + `Helipad`/`AcUnit` features placed later in Dressing (Task 14), so here only terrain.

- [ ] **Step 1: Tests**

```rust
#[test]
fn undercity_has_tunnels_under_arterials_and_a_rail() {
    let c = small(51, WaterMode::Bay, 7);
    let uc = c.stratum(StratumKind::Undercity).unwrap();
    for path in &c.arterials { for p in path { assert!(matches!(uc.tiles[*p].terrain, Terrain::Tunnel | Terrain::Sewage | Terrain::Grate), "{p:?}"); } }
    assert!(uc.tiles.count(|t| t.terrain == Terrain::Rail) >= 100);
    assert!(uc.tiles.count(|t| t.terrain == Terrain::Sewage) > 0);
}
#[test]
fn skyways_and_heights_follow_spans() {
    let c = small(51, WaterMode::None, 7);
    let sk = c.stratum(StratumKind::Skyways).unwrap(); let hi = c.stratum(StratumKind::Heights).unwrap();
    for b in &c.buildings {
        let p = b.footprint.center();
        if b.span.contains(StratumKind::Skyways) { assert_eq!(sk.tiles[p].terrain, Terrain::Floor); } else { assert_eq!(sk.tiles[p].terrain, Terrain::Void); }
        if b.span.contains(StratumKind::Heights) { assert_eq!(hi.tiles[p].terrain, Terrain::Rooftop); } else { assert_eq!(hi.tiles[p].terrain, Terrain::Void); }
    }
    assert!(sk.tiles.count(|t| t.terrain == Terrain::Walkway) > 50);
}
```

- [ ] **Step 2: Run** → fail.

- [ ] **Step 3: Implement**

Work items in `begin`: `[Arterials(i)...]`, `[Blocks(i)...]`, `[Rail]`, `[Buildings(i)...]`, `[Walkways(i)...]`, `[Bridges(i)...]`; one item per `step`.
- `Arterials(i)`: along `arterials[i]`, for each point `p`: `Sewage` at `p` and `p.offset(1,0)` (2 wide), `Tunnel` at the 4×4 stamp (`Rect(p.x-1,p.y-1,4,4)`) where not Sewage; every 12th path point set the two Sewage tiles to `Grate`.
- `Blocks(i)`: with p = 1/3 (stage rng), for every Street tile within `block.rect` grown by 2 that is `Road|Alley` → Undercity `Tunnel` (if `Void`).
- `Rail`: y0 = `h/2 + rng(-h/8..h/8)`; for x in 0..w: `y = y0 + (noise(x*0.02)*4) as i32`; set `Rail` at `(x,y)` and `Tunnel` at `(x,y±1)` where Void.
- `Buildings(i)`: for each stratum in `span` other than Street: paint `Wall` border / `Floor` interior (Skyways, Undercity); Heights → `Rooftop` whole footprint.
- `Walkways(i)` for building i with span ∋ Skyways: find up to 2 other such buildings with chebyshev edge distance ≤ 12 (index order, j > i); connect nearest border tiles with an L-shaped 2-wide `Walkway` path over tiles that are `Void` at Skyways (if any tile on the L is not Void/Walkway, try the other L orientation; else skip). In CorpCore/NeonStrip districts, for every Street `Road` tile that has Road in ≥ 3 cardinal directions and `(x+y)%7==0`, stamp a 5×5 `Walkway` platform at Skyways where Void.
- `Bridges(i)`: for Tall building i, nearest Tall building j (j>i) within 8 tiles with overlapping row range (or column range): paint a 1-wide `Bridge` line at Heights across the gap on the middle overlapping row/column where Void.

- [ ] **Step 4: Run tests** → PASS; gate.
- [ ] **Step 5: Commit** `feat(citygen): undercity, skyways and heights verticals`.

---

### Task 12: Interiors stage

**Files:**
- Create: `crates/citygen/src/gen/interiors.rs`
- Modify: `gen/mod.rs`

**Interfaces:**
- Consumes: buildings, strata grids.
- Produces: BSP rooms (`Wall` partitions with `Door` gaps), exterior `Door` on Street facing outdoor ground, `Window`s on street-facing walls, internal `Stair`/`Lift` with `Connector { public: false }` for spans ≥ 2, doors at Skyways/Undercity where adjacent to `Walkway`/`Tunnel`.

- [ ] **Step 1: Tests**

```rust
#[test]
fn every_building_has_a_street_door_and_internal_connectivity() {
    let c = small(61, WaterMode::Bay, 7);
    let st = c.stratum(StratumKind::Street).unwrap();
    for b in &c.buildings {
        let doors: Vec<_> = b.footprint.border().filter(|p| st.tiles[*p].terrain == Terrain::Door).collect();
        assert!(!doors.is_empty(), "{:?} has no door", b.id);
        assert!(doors.iter().any(|d| d.neighbors4().any(|n| st.tiles.get(n).is_some_and(|t| t.is_outdoor_ground()))));
        // all floor tiles inside reachable from the door
        let inside = b.footprint;
        let comps = components(st.tiles.size(), |p| inside.contains(p) && st.tiles[p].is_walkable());
        assert_eq!(comps.len(), 1, "{:?} interior split", b.id);
        if b.span.len() >= 2 {
            let conn = c.connectors.iter().find(|k| inside.contains(k.pos) && !k.public).expect("internal connector");
            for s in b.span.iter() { assert!(conn.serves.contains(&s)); assert!(c.walkable(s, conn.pos)); }
        }
    }
}
#[test]
fn rooms_respect_minimum_size() {
    let c = small(61, WaterMode::None, 6);
    let st = c.stratum(StratumKind::Street).unwrap();
    let big = c.buildings.iter().find(|b| b.footprint.w >= 12 && b.footprint.h >= 9).unwrap();
    let walls_inside = big.footprint.inset(1).iter().filter(|p| st.tiles[*p].terrain == Terrain::Wall).count();
    assert!(walls_inside > 0, "big building should have internal walls");
}
```

- [ ] **Step 2: Run** → fail.

- [ ] **Step 3: Implement**

One building per `step`. For each stratum `s` in `span` with terrain painted as Wall/Floor (skip Heights rooftops):
1. `bsp(inner = footprint.inset(1))`: if `inner.w >= 7 || inner.h >= 7` (room ≥ 3 + wall + 3): split the longer axis at a position leaving ≥ 3 on each side (jittered); paint the partition line `Wall`; put one `Door` in the partition at a random position not adjacent to the inner border; recurse on both sides with depth ≤ 4.
2. Exterior door (Street only): candidates = border tiles (not corners) with an outdoor-ground 4-neighbour outside the footprint; prefer Sidewalk; pick with rng; set `Door`. If no candidate, carve: pick the border tile nearest any outdoor ground within 3 and set the intervening tiles to `Alley`. Record the door's inside neighbour as `lobby` (used by Dressing/Net).
3. Windows (Street): border tiles (not corners/doors) adjacent to outdoor ground, every 2nd tile → `Window`.
4. Other strata doors: at Skyways/Undercity, border tiles with a `Walkway`/`Tunnel`/`Bridge` 4-neighbour outside: first one → `Door`.
5. Shaft (`span.len() >= 2`): choose `pos` = the Floor tile closest to footprint centre that is Floor on every stratum in span (Heights counts Rooftop); if none, choose the inner-rect centre and set it to Floor on all strata and its 4 inner neighbours to Floor where they are internal walls. Terrain at `pos` on every stratum in span: `Stair` if `span.len()==2` else `Lift`. Push `Connector { kind: Stair|Lift, pos, serves: span.iter().collect(), public: false }`.
Store `lobby` positions in `Scratch.lobbies: Vec<(BuildingId, Pos)>` (add the field).

- [ ] **Step 4: Run tests** → PASS; gate.
- [ ] **Step 5: Commit** `feat(citygen): building interiors, doors, windows, shafts`.

---

### Task 13: Connectivity stage

**Files:**
- Create: `crates/citygen/src/gen/connectivity.rs`
- Modify: `gen/mod.rs`

**Interfaces:**
- Consumes: all strata, connectors, districts.
- Produces: repaired strata (carved `Door`/`Tunnel`/`Walkway`/`Bridge`, voided slivers), public connectors (`Manhole`, `Stair`, `Ladder`), `GenError::Unconnected` after 3 failed rounds.
- Exposes `pub(crate) fn reachable_from_street(city: &City) -> Vec<Grid<bool>>` (one grid per physical stratum; BFS across 4-adjacency and connectors from the largest Street walkable component) — reused by `validate` (Task 17).

- [ ] **Step 1: Tests**

```rust
#[test]
fn every_stratum_is_reachable_from_street() {
    for seed in [71, 72, 73] {
        let c = small(seed, WaterMode::Bay, 7);
        let reach = reachable_from_street(&c);
        for (i, k) in StratumKind::PHYSICAL.iter().enumerate() {
            let st = c.stratum(*k).unwrap();
            let walk = st.tiles.count(|t| t.is_walkable()); let got = reach[i].count(|&b| b);
            assert!(got as f32 >= 0.95 * walk as f32, "seed {seed} {k:?}: {got}/{walk} reachable");
        }
    }
}
#[test]
fn every_district_has_a_public_connector() {
    let c = small(74, WaterMode::River, 8);
    for d in &c.districts {
        assert!(c.connectors.iter().any(|k| k.public && d.bounds.contains(k.pos) && c.tile(StratumKind::Street, k.pos).unwrap().district == Some(d.id)), "{:?}", d.id);
    }
}
#[test]
fn repair_joins_two_halves() {
    // hand-built: 20x5 Street with a wall column at x=10; a 'building' owning the wall
    let mut c = City::empty(GenParams { size: SizePreset::Custom { w: 80, h: 40 }, ..Default::default() });
    let st = c.stratum_mut(StratumKind::Street).unwrap();
    st.tiles.fill_rect(Rect::new(0, 0, 20, 5), Tile::new(Terrain::Sidewalk));
    for y in 0..5 { st.tiles.set(Pos::new(10, y), Tile::new(Terrain::Wall)); }
    join_street_components(&mut c);
    let st = c.stratum(StratumKind::Street).unwrap();
    assert_eq!(components(st.tiles.size(), |p| st.tiles[p].is_walkable()).len(), 1);
}
```

- [ ] **Step 2: Run** → fail.

- [ ] **Step 3: Implement**

`begin` queues rounds `[Round(0), Round(1), Round(2), Check]`.
`Round(r)`:
1. `join_street_components(city)`: components of walkable Street tiles; for each non-largest component, `bfs_nearest` from its first tile through tiles that are `Wall|Void|Window` (carvable) or walkable, to any tile of the main component, max 40; carve the path: Wall/Window→Door if `tile.building.is_some()` else Alley; Void→Alley. If no path, and component < 1% → set component tiles to Void (only if not inside a building).
2. `reach = reachable_from_street(city)`. For each other stratum `k`, components of (walkable && !reach): for each: try `bfs_nearest` through carvable tiles (Undercity: Void→Tunnel; Skyways: Void→Walkway; Heights: Void→Bridge; Wall→Door when in a building) to a reached tile on the same stratum, max 30; else if component < 1% and no tile has `building.is_some()` → Void.
3. Public connectors per district, in district index order, for each target stratum lacking a public connector in this district:
   - Undercity: candidate Street tiles in district with terrain `Sidewalk|Alley` whose Undercity tile is `Tunnel` → pick the first by index; set `Ladder` on both strata; push `Connector { Manhole, serves: [Undercity, Street], public: true }`. If none: pick a Sidewalk tile, carve `bfs_nearest` on Undercity from below it through Void to a Tunnel (max 30) painting `Tunnel`, then place as above.
   - Skyways: candidate Street `Sidewalk|Plaza` tile whose Skyways tile is `Walkway` → `Stair` both; else pick a Sidewalk/Plaza tile, carve Walkway path at Skyways to nearest Skyways walkable (max 30); push `Connector { Stair, serves: [Street, Skyways], public: true }`.
   - Heights: find a Tall building in the district; candidate = a Skyways walkable tile 4-adjacent to its footprint; set `Ladder` at Skyways (keep walkable) and at Heights set `Ladder` (the tile was Void); push `Connector { Ladder, serves: [Skyways, Heights], public: true }`. If no Tall building or no candidate, skip.
`Check`: `reach` again; if any stratum has walkables and < 95% reached → if rounds remain, continue, else `Err(GenError::Unconnected { stratum })`.

`reachable_from_street`: start = largest walkable Street component; BFS over `(stratum, pos)`; neighbours = 4-adjacent walkables on the same stratum + for every connector at `pos` whose `serves` contains the current stratum, the other served strata where walkable.

- [ ] **Step 4: Run tests** → PASS; gate.
- [ ] **Step 5: Commit** `feat(citygen): connectivity repair and public connectors`.

---

### Task 14: Dressing stage

**Files:**
- Create: `crates/citygen/src/gen/dressing.rs`
- Modify: `gen/mod.rs`

**Interfaces:**
- Consumes: buildings, lobbies, districts (theme spec, wealth, decay), `Scratch.decay/wealth`, `FlavorPack::sign_glyph`.
- Produces: features per spec §5.9, `Stratum.light`, `tile.variant` on all strata.

- [ ] **Step 1: Tests**

```rust
#[test]
fn dressing_places_features_and_lights() {
    let c = small(81, WaterMode::Bay, 7);
    let st = c.stratum(StratumKind::Street).unwrap();
    let count = |f: fn(&Feature) -> bool| st.tiles.iter().filter(|(_, t)| t.feature.as_ref().is_some_and(f)).count();
    assert!(count(|f| matches!(f, Feature::Neon(_))) > 30);
    assert!(count(|f| matches!(f, Feature::Lamp)) > 30);
    assert!(count(|f| matches!(f, Feature::Sign(_))) > 5);
    assert!(count(|f| matches!(f, Feature::Terminal)) >= c.buildings.len() / 2);
    assert!(st.light.as_slice().iter().any(|&l| l > 150));
    assert!(c.stratum(StratumKind::Undercity).unwrap().light.as_slice().iter().filter(|&&l| l < 40).count() > 100);
    // blocking features never sit on connectors or doors
    for k in &c.connectors { for s in &k.serves { let t = c.tile(*s, k.pos).unwrap(); assert!(t.is_walkable(), "{k:?} blocked on {s:?}"); } }
    assert!(st.tiles.iter().all(|(_, t)| t.terrain != Terrain::Door || t.feature.is_none()));
}
#[test]
fn variants_are_set_everywhere() {
    let c = small(81, WaterMode::None, 5);
    let st = c.stratum(StratumKind::Street).unwrap();
    let distinct: std::collections::HashSet<u8> = st.tiles.as_slice().iter().map(|t| t.variant).collect();
    assert!(distinct.len() > 100);
}
```

- [ ] **Step 2: Run** → fail.

- [ ] **Step 3: Implement**

Work items: `Facades(building)`, `Streets(district)`, `Strata(k)` for lights/variants.
- `Facades(b)`: theme = district theme; for Street border tiles that are `Wall` with an outdoor 4-neighbour: `chance(spec.neon * (0.5 + wealth))` → `Neon(pick hues)`; else `chance(spec.holo)` → `HoloAd`; else `chance(wealth * 0.3)` → `Camera`. Wall tile 4-adjacent to the exterior door (same facade) → `Sign(flavor.sign_glyph)` for non-corp kinds. Lobby tile → `Terminal` only if the lobby has ≥ 2 walkable 4-neighbours after placing (never block the door: require lobby ≠ only path; implement as: place Terminal at the lobby tile's inner neighbour away from the door if that tile is Floor and has ≥ 3 walkable neighbours; else skip). Heights rooftop: if `w≥9 && h≥7` set `Helipad` on the centre 3×3; `AcUnit` with p 0.08 on rooftop tiles with ≥ 6 walkable 8-neighbours.
- `Streets(d)`: for Street tiles in `bounds` with `district==d`: Sidewalk with `(x*7+y*13) % spec.lamp_spacing == 0` → `Lamp`; Road|Sidewalk|Alley: `chance(decay*0.08)` → `Puddle`; Alley|Sidewalk: `chance(decay*0.1)` → `Trash`; Floor (building) `chance(decay*0.03)` → `Vent`; CombatZone Road tiles with ≥ 6 walkable 8-neighbours `chance(0.03)` → `Barrier`; NightMarket Plaza tiles with ≥ 6 walkable 8-neighbours `chance(0.3)` → `Stall`; Sidewalk `chance(0.01)` → `Terminal` only with ≥ 6 walkable 8-neighbours. Undercity Tunnel tiles with a Void 4-neighbour: `chance(0.15)` → `Pipe`. Skyways Walkway: `chance(0.1)` → `Cable`. Never place a feature on `Door|Stair|Lift|Ladder` tiles or on connector positions.
- `Strata(k)`: `light = base[k]` (Undercity 20, Street 60, Skyways 80, Heights 110) then additive falloff: Lamp r6 +120, Neon r4 +90, HoloAd r5 +100, Window(Street) r2 +30; `add = strength * (1 - d/r)`, clamp 255. Then `variant = tile_hash(seed, k, p)` for every tile.

- [ ] **Step 4: Run tests** → PASS; gate.
- [ ] **Step 5: Commit** `feat(citygen): dressing, light maps, variants`.

---

### Task 15: Naming stage

**Files:**
- Create: `crates/citygen/src/gen/naming.rs`
- Modify: `gen/mod.rs`

**Interfaces:**
- Produces: `City.name`, `District.name`, `Building.name` for corp kinds, shops with templates, and 2 landmarks per district.

- [ ] **Step 1: Tests**

```rust
#[test]
fn names_are_filled_and_deterministic() {
    let a = small(91, WaterMode::Bay, 7); let b = small(91, WaterMode::Bay, 7);
    assert!(!a.name.is_empty()); assert_eq!(a.name, b.name);
    assert!(a.districts.iter().all(|d| !d.name.is_empty()));
    let named = a.buildings.iter().filter(|b| b.name.is_some()).count();
    assert!(named >= a.buildings.iter().filter(|b| b.kind.is_corp()).count());
    assert!(a.buildings.iter().filter(|b| b.name.as_deref().is_some_and(|n| n.contains("Tower") || n.contains("Terminal") || n.contains("Arcology") || n.contains("Plaza") || n.contains("Station"))).count() >= a.districts.len());
}
```

- [ ] **Step 2–3:** one `step` per district: name the district; for its buildings in index order: corp kinds → `corp_name`; others → `shop_name(kind)` when `Some`; then choose up to 2 unnamed-or-corp buildings (largest footprints first) and set `landmark_name(corp, district)`. First step also sets `city.name`.
- [ ] **Step 4: Run tests** → PASS; gate.
- [ ] **Step 5: Commit** `feat(citygen): naming stage`.

---

### Task 16: Net stage

**Files:**
- Create: `crates/citygen/src/gen/net.rs`
- Modify: `gen/mod.rs`

**Interfaces:**
- Produces: `City.net` per spec §5.11. Node anchors: Terminal → the `Feature::Terminal` tile; Camera → `Feature::Camera` tile; DoorLock → exterior `Door` tile (Street); LiftCtrl → `Lift` connector pos on Street; AdBoard → `HoloAd` tile; LampGrid → district's first Lamp tile; Server → a Floor tile in the building farthest from the door (Street); DataVault → another Floor tile (CorpTower/Clinic only); Ice → same tile as Server when `security ≥ 160`; Backbone → the district's first Terminal (or Server if none). City core: `Subnet { owner: City, security: 230 }` with a `Server` + `Ice` in the largest CorpCore building.

- [ ] **Step 1: Tests**

```rust
#[test]
fn net_graph_is_consistent() {
    let c = small(101, WaterMode::Bay, 7);
    assert_eq!(c.net.subnets.iter().filter(|s| matches!(s.owner, SubnetOwner::Building(_))).count(), c.buildings.len());
    assert_eq!(c.net.subnets.iter().filter(|s| matches!(s.owner, SubnetOwner::District(_))).count(), c.districts.len());
    assert_eq!(c.net.subnets.iter().filter(|s| s.owner == SubnetOwner::City).count(), 1);
    for n in &c.net.nodes {
        let t = c.tile(n.anchor.0, n.anchor.1).expect("anchor in bounds");
        match n.kind {
            NodeKind::Terminal => assert_eq!(t.feature, Some(Feature::Terminal)),
            NodeKind::Camera => assert_eq!(t.feature, Some(Feature::Camera)),
            NodeKind::DoorLock => assert_eq!(t.terrain, Terrain::Door),
            NodeKind::LiftCtrl => assert_eq!(t.terrain, Terrain::Lift),
            NodeKind::AdBoard => assert_eq!(t.feature, Some(Feature::HoloAd)),
            NodeKind::Server | NodeKind::DataVault | NodeKind::Ice => assert!(matches!(t.terrain, Terrain::Floor | Terrain::Lift | Terrain::Stair)),
            NodeKind::LampGrid => assert_eq!(t.feature, Some(Feature::Lamp)),
            NodeKind::Backbone => assert!(t.feature == Some(Feature::Terminal) || t.terrain == Terrain::Floor),
        }
        assert!(c.net.subnets.iter().any(|s| s.id == n.subnet));
    }
    for Link(a, b) in &c.net.links { assert!(a.index() < c.net.nodes.len() && b.index() < c.net.nodes.len()); }
    // every building server links to its district backbone (path length 1)
    let backbones: Vec<_> = c.net.nodes.iter().filter(|n| n.kind == NodeKind::Backbone).map(|n| n.id).collect();
    for s in c.net.nodes.iter().filter(|n| n.kind == NodeKind::Server && n.building.is_some()) {
        assert!(c.net.links.iter().any(|Link(a, b)| (*a == s.id && backbones.contains(b)) || (*b == s.id && backbones.contains(a))));
    }
}
```

- [ ] **Step 2–3:** `begin`: create district subnets (security = `spec.security` ± wealth*30) and the city subnet; one `step` per building: subnet (security = district security + wealth*40 − decay*20, clamp), nodes from features within the footprint and its exterior door/lift, links node→Server, Server→district Backbone (create the Backbone node lazily at the district's first Terminal/Server). Last step: city core and links Backbone→core Server.
- [ ] **Step 4: Run tests** → PASS; gate.
- [ ] **Step 5: Commit** `feat(citygen): net graph stage`.

---

### Task 17: Validation and seed-sweep integration tests

**Files:**
- Create: `crates/citygen/src/validate.rs` (replace the placeholder `Violation`), `crates/citygen/tests/seeds.rs`
- Modify: `gen/mod.rs` (`finish` already calls validate), `lib.rs`

**Interfaces:**
- Produces: `pub enum Violation { ConnectorNotWalkable { pos, stratum }, BuildingWithoutDoor(BuildingId), StratumUnreachable { stratum, reached, walkable }, DanglingId { pos, stratum }, FootprintOverlap(BuildingId, BuildingId), NetAnchor(NodeId), DistrictEmpty(DistrictId) }` with `Display`; `City::validate(&self) -> Result<(), Vec<Violation>>`.

- [ ] **Step 1: Tests** (`tests/seeds.rs`)

```rust
use citygen::*;
fn sweep(preset: SizePreset, seeds: std::ops::Range<u64>) {
    for seed in seeds {
        let p = GenParams { seed, size: preset, water: match seed % 3 { 0 => WaterMode::Bay, 1 => WaterMode::River, _ => WaterMode::None }, districts: 4 + (seed % 9) as u8, ..Default::default() };
        if p.validate().is_err() { continue; }
        let a = Generator::run(p.clone()).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let b = Generator::run(p).unwrap();
        assert_eq!(postcard::to_allocvec(&a).unwrap(), postcard::to_allocvec(&b).unwrap(), "seed {seed} not deterministic");
        a.validate().unwrap_or_else(|v| panic!("seed {seed}: {v:?}"));
    }
}
#[test] fn small_seeds() { sweep(SizePreset::Small, 0..50); }
#[test] fn medium_seeds() { sweep(SizePreset::Medium, 1000..1010); }
#[test] fn large_seeds() { sweep(SizePreset::Large, 2000..2003); }
#[test] #[ignore] fn extended_seeds() { sweep(SizePreset::Small, 100..250); sweep(SizePreset::Medium, 3000..3050); }
#[test] fn medium_generates_under_two_seconds() {
    let t = std::time::Instant::now();
    Generator::run(GenParams { seed: 77, ..Default::default() }).unwrap();
    assert!(t.elapsed().as_secs_f32() < 2.0, "took {:?}", t.elapsed());
}
```
Add `postcard` to `citygen` dev-dependencies.

- [ ] **Step 2–3:** implement `validate` per spec §5 using `reachable_from_street` (≥ 95% rule for `StratumUnreachable`), and run the sweep. Fix whatever stage breaks a seed; keep each fix in its stage file. If the timing test fails, profile with `cargo test --release` first and optimise the hot stage (likely light-map falloff: restrict to a bounding box per light).
- [ ] **Step 4: Run** `cargo test -p citygen` → PASS; gate.
- [ ] **Step 5: Commit** `feat(citygen): City::validate and seed sweep tests`.

---

### Task 18: App skeleton, terminal guard, save format, CLI

**Files:**
- Create: `crates/city/src/term.rs`, `crates/city/src/save.rs`, `crates/city/src/cli.rs`, `crates/city/src/app.rs`, `crates/city/src/screens/mod.rs`
- Modify: `crates/city/src/main.rs`

**Interfaces:**
- Produces:
  - `term::TerminalGuard::enter() -> anyhow::Result<(Self, Terminal<CrosstermBackend<Stdout>>)>`; `Drop` restores; `term::install_panic_hook()`.
  - `save::{FORMAT_VERSION: u16 = 1, save_city(path, &City) -> Result<()>, load_city(path) -> Result<City>, list_cities(dir) -> Result<Vec<CityFileInfo>>, CityFileInfo { path: PathBuf, name: String, seed: u64, size: Size, modified: SystemTime }, default_dir() -> PathBuf ("./cities")}`. Layout: `b"CITY"`, `u16 LE version`, `u32 LE header_len`, postcard `Header { name, seed, w, h }`, postcard `City`.
  - `app::{AppState { params: GenParams, city: Option<City>, player: Option<Player>, style: StylePack, tick: u64, status: Option<(String, u64)>, pending_save: Option<PathBuf> }, Screen trait { fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd; fn draw(&self, st: &AppState, f: &mut Frame) }, ScreenCmd { Stay, Switch { to: Box<dyn Screen>, fx: TransitionKind }, Quit }, run(initial: Box<dyn Screen>) -> Result<()>, MIN_W = 100, MIN_H = 30, draw_too_small(f) }`.
  - `cli::Cli` (clap): no subcommand → interactive; `Gen { seed: Option<u64>, size: String ("small"|"medium"|"large"|"WxH"), districts: Option<u8>, water: Option<String>, out: PathBuf, json: bool }`.
  - `screens::placeholder::Blank` screen that draws nothing (used until Task 24 provides Title).
- Note: `Player` (Task 26) and `StylePack` (Task 20) don't exist yet: in this task declare `AppState.player: Option<()>`-free by defining `pub type Player = crate::walker::Player;` later; for now omit `player` and `style` fields and add them in Tasks 20/26 (both tasks list the modification).

- [ ] **Step 1: Tests** (`save.rs`)

```rust
#[test]
fn roundtrip_and_header() {
    let c = Generator::run(GenParams { seed: 5, size: SizePreset::Custom { w: 80, h: 40 }, districts: 4, ..Default::default() }).unwrap();
    let dir = std::env::temp_dir().join(format!("city-test-{}", std::process::id())); std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("a.city");
    save_city(&path, &c).unwrap();
    let back = load_city(&path).unwrap();
    assert_eq!(postcard::to_allocvec(&c).unwrap(), postcard::to_allocvec(&back).unwrap());
    let list = list_cities(&dir).unwrap();
    assert_eq!(list.len(), 1); assert_eq!(list[0].seed, 5); assert_eq!(list[0].name, c.name);
}
#[test]
fn rejects_bad_magic_version_and_truncation() {
    let dir = std::env::temp_dir().join(format!("city-bad-{}", std::process::id())); std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("x.city"); std::fs::write(&p, b"NOPE\x01\x00").unwrap();
    assert!(load_city(&p).unwrap_err().to_string().contains("not a .city file"));
    let mut bytes = b"CITY".to_vec(); bytes.extend_from_slice(&99u16.to_le_bytes()); std::fs::write(&p, &bytes).unwrap();
    assert!(load_city(&p).unwrap_err().to_string().contains("version 99"));
    let c = Generator::run(GenParams { seed: 5, size: SizePreset::Custom { w: 80, h: 40 }, districts: 4, ..Default::default() }).unwrap();
    save_city(&p, &c).unwrap();
    let full = std::fs::read(&p).unwrap(); std::fs::write(&p, &full[..full.len() / 2]).unwrap();
    assert!(load_city(&p).is_err());
}
```
`app.rs` test: `draw_too_small` on a `TestBackend::new(1,1)` and `(99,29)` does not panic and the 99×29 buffer contains "100".

- [ ] **Step 2: Run** → fail.

- [ ] **Step 3: Implement**

`run` loop:
```rust
pub fn run(mut screen: Box<dyn Screen>, mut st: AppState) -> Result<()> {
    install_panic_hook();
    let (_guard, mut term) = TerminalGuard::enter()?;
    let frame = Duration::from_micros(33_333);
    let mut transition: Option<Transition> = None;   // Task 23 provides ui::transition
    loop {
        let start = Instant::now();
        let mut keys = Vec::new();
        while event::poll(Duration::ZERO)? { if let Event::Key(k) = event::read()? { if k.kind == KeyEventKind::Press { keys.push(k); } } }
        st.tick += 1;
        if transition.as_ref().is_some_and(|t| t.active(st.tick)) && !keys.is_empty() { transition = None; }
        match screen.update(&mut st, &keys) {
            ScreenCmd::Stay => {}
            ScreenCmd::Switch { to, fx } => { let snap = term.current_buffer_mut().clone(); screen = to; transition = Some(Transition::new(fx, snap, st.tick)); }
            ScreenCmd::Quit => break,
        }
        term.draw(|f| {
            if f.area().width < MIN_W || f.area().height < MIN_H { draw_too_small(f); return; }
            screen.draw(&st, f);
            if let Some(t) = &transition { t.apply(f, st.tick); }
        })?;
        if let Some(rem) = frame.checked_sub(start.elapsed()) { std::thread::sleep(rem); }
    }
    Ok(())
}
```
Until Task 23 exists, define `Transition` as a stub in `app.rs` with `active()` false and `apply()` no-op; Task 23 replaces it with `ui::transition::Transition` and `TransitionKind { None, Glitch, SlideUp }`.

`main.rs`: parse `Cli`; `Gen` → `cli::gen(args)` builds `GenParams`, runs `Generator::run`, writes with `save_city` or `serde_json::to_writer_pretty`, prints `name seed WxH buildings districts`. No subcommand → `app::run(Box::new(Blank), AppState::new())`.

- [ ] **Step 4: Run tests** → PASS; `cargo run -p city -- gen --seed 1 --size small --out /tmp/x.city` prints a summary; gate.
- [ ] **Step 5: Commit** `feat(city): app loop, terminal guard, save format, gen CLI`.

---

### Task 19: Animation primitives

**Files:**
- Create: `crates/city/src/anim/mod.rs`, `crates/city/src/anim/effects.rs`

**Interfaces:**
- Produces:
  - `Easing { Linear, OutCubic, InOutCubic, OutBack }` with `apply(t: f32) -> f32`.
  - `Tween { start: u64, dur: u64, easing }` with `progress(tick) -> f32` (clamped 0..1), `done(tick)`, `active(tick)`.
  - `Timeline` = `Vec<(label: &'static str, Tween)>` with `progress(label, tick) -> f32`, `all_done(tick)`.
  - `effects::typewriter(text: &str, p: f32) -> &str` (prefix by char count), `effects::slide_x(width: u16, p: f32) -> i32` (offset from −width to 0), `effects::flicker(tick, variant: u8, broken: bool) -> f32` (0..1 brightness), `effects::pulse(tick, period: u64) -> f32`, `effects::glitch_cells(area: Rect, tick: u64, p: f32) -> impl Iterator<Item=(u16,u16,char)>` (deterministic set of cells to corrupt; count ∝ `1 − p`), `effects::GLITCH_CHARS: &[char]`.

- [ ] **Step 1: Tests**

```rust
#[test] fn tween_clamps_and_eases() {
    let t = Tween { start: 10, dur: 10, easing: Easing::Linear };
    assert_eq!(t.progress(5), 0.0); assert_eq!(t.progress(15), 0.5); assert_eq!(t.progress(30), 1.0); assert!(t.done(20));
    assert!(Easing::OutCubic.apply(0.5) > 0.5);
    assert_eq!(Easing::OutBack.apply(1.0), 1.0);
}
#[test] fn typewriter_is_char_safe() {
    assert_eq!(typewriter("ｦｧｨ", 0.34), "ｦ"); assert_eq!(typewriter("abc", 1.0), "abc"); assert_eq!(typewriter("abc", 0.0), "");
}
#[test] fn flicker_is_deterministic_and_bounded() {
    for tick in 0..200 { let v = flicker(tick, 7, true); assert!((0.0..=1.0).contains(&v)); assert_eq!(v, flicker(tick, 7, true)); }
    assert!(flicker(3, 0, false) > 0.6);
}
#[test] fn glitch_density_shrinks_with_progress() {
    let a = glitch_cells(Rect::new(0, 0, 50, 10), 1, 0.1).count(); let b = glitch_cells(Rect::new(0, 0, 50, 10), 1, 0.9).count();
    assert!(a > b);
}
```

- [ ] **Step 2–3:** implement. `flicker`: `broken` → off (0.05) when `((tick/3 + variant as u64*7) % 64)` ∈ {10,11,12,13,20,21,40}; else `0.8 + 0.2*sin((tick as f32)/9.0 + variant as f32)`. `glitch_cells`: `n = (area.area() as f32 * 0.25 * (1-p)) as usize`; for i in 0..n: `h = hash64(tick, i)`, `x = h % w`, `y = (h>>16) % h_area`, `ch = GLITCH_CHARS[(h>>32) % len]`.
- [ ] **Step 4: Run tests** → PASS; gate.
- [ ] **Step 5: Commit** `feat(city): animation primitives and effects`.

---

### Task 20: Style pack, tileset and lighting

**Files:**
- Create: `flavor/style.ron`, `crates/city/src/render/mod.rs`, `render/style.rs`, `render/tileset.rs`, `render/lighting.rs`
- Modify: `crates/city/src/app.rs` (add `style: StylePack` to `AppState`)

**Interfaces:**
- Produces:
  - `Rgb(pub u8, pub u8, pub u8)` with `to_color() -> ratatui Color::Rgb`, `scale(f32)`, `mix(other, t)`, `tint(dr, dg, db)`.
  - `Cell { ch: char, fg: Rgb, bg: Rgb, bold: bool, dim: bool }` with `write(&self, buf: &mut Buffer, x: u16, y: u16)`.
  - `Visibility { Unseen, Remembered, Visible }`.
  - `StylePack` (ron): `background: Rgb`, `themes: Vec<(Theme, ThemeStyle { accent, accent2, wall, floor, road, sidewalk, roof })>`, `strata: Vec<(StratumKind, StratumStyle { tint: (i16,i16,i16), fog: bool, rain: u8 })>`, `hues: Vec<(Hue, Rgb)>`, `glyphs: GlyphSets { holo: Vec<char>, rain: Vec<char>, water: Vec<char>, steam: Vec<char>, mass: Vec<char>, net: Vec<char> }`, `StylePack::load() -> anyhow::Result<Self>` from `include_str!("../../../../flavor/style.ron")`, `theme(Theme) -> &ThemeStyle`, `stratum(StratumKind) -> &StratumStyle`, `hue(Hue) -> Rgb`.
  - `RenderCtx<'a> { style: &'a StylePack, theme: Option<Theme>, stratum: StratumKind, light: u8, tick: u64, pos: Pos, vis: Visibility, connector: Option<ConnectorKind>, kind: Option<BuildingKind>, wall_mask: u8 }`.
  - `tileset::cell(tile: &Tile, ctx: &RenderCtx) -> Cell`; `tileset::glyph_for(terrain, feature, connector, kind, variant, tick, wall_mask: u8, style) -> char` (the static part, exposed for tests and the legend; `wall_mask` bits N=1,E=2,S=4,W=8 mark neighbouring Wall|Door|Window tiles); `tileset::legend_entries(stratum) -> Vec<(char, &'static str)>`.
  - `lighting::lit(c: Rgb, light: u8) -> Rgb` (`scale(0.25 + 0.75*light/255)`), `lighting::apply_vis(cell, vis) -> Cell` (Remembered: scale 0.35, no animation; Unseen: all background).

- [ ] **Step 1: Tests**

```rust
#[test] fn style_glyphs_are_single_width() {
    use unicode_width::UnicodeWidthChar;
    let s = StylePack::load().unwrap();
    for set in [&s.glyphs.holo, &s.glyphs.rain, &s.glyphs.water, &s.glyphs.steam, &s.glyphs.mass, &s.glyphs.net] { for c in set { assert_eq!(c.width(), Some(1), "{c:?}"); } }
    for terrain in ALL_TERRAIN { for v in [0u8, 77, 200] { let c = glyph_for(terrain, None, None, None, v, 0, 0, &s); assert_eq!(c.width(), Some(1), "{terrain:?}"); } }
    for f in ALL_FEATURES { let c = glyph_for(Terrain::Floor, Some(f), None, None, 1, 0, 0, &s); assert_eq!(c.width(), Some(1), "{f:?}"); }
}
#[test] fn cell_mapping_is_pure_and_lit() {
    let s = StylePack::load().unwrap();
    let ctx = |light, tick| RenderCtx { style: &s, theme: Some(Theme::CorpCore), stratum: StratumKind::Street, light, tick, pos: Pos::new(1, 1), vis: Visibility::Visible, connector: None, kind: None, wall_mask: 0 };
    let wall = Tile::new(Terrain::Wall);
    assert_eq!(cell(&wall, &ctx(255, 0)), cell(&wall, &ctx(255, 0)));
    assert!(cell(&wall, &ctx(255, 0)).fg.0 > cell(&wall, &ctx(0, 0)).fg.0);
    let mut neon = Tile::new(Terrain::Wall); neon.feature = Some(Feature::Neon(Hue::Magenta)); neon.variant = 5; // broken sign
    let on = cell(&neon, &ctx(200, 0)); let off = cell(&neon, &ctx(200, 30));
    assert_ne!(on.fg, off.fg);
    let mut water = Tile::new(Terrain::Water);
    assert_ne!(cell(&water, &ctx(100, 0)).ch, cell(&water, &ctx(100, 6)).ch);
    water.variant = 1; let _ = water;
    let unseen = apply_vis(cell(&wall, &ctx(255, 0)), Visibility::Unseen);
    assert_eq!(unseen.ch, ' ');
}
```
Define `ALL_TERRAIN: [Terrain; 20]` and `ALL_FEATURES: [Feature; 15]` consts in `citygen::tile` (public, used by the legend too).

- [ ] **Step 2: Run** → fail.

- [ ] **Step 3: Implement**

Glyph table (terrain → char; `kind` picks wall weight: corp kinds `═║╔╗╚╝╠╣╦╩╬`, Megablock/Apartment `━┃┏┓┗┛┣┫┳┻╋`, else `─│┌┐└┘├┤┬┴┼`; wall glyph chosen from the 4-neighbour wall mask, which the viewport computes into `RenderCtx.wall_mask`):

| Terrain | glyph |
|---|---|
| Void | `' '` (Heights/Skyways: `' '` with dark bg; Undercity: `' '`) |
| Water | `style.glyphs.water[(tick/6 + x + variant) % len]` |
| Sewage | `≈` with green fg |
| Road | `' '` (dark asphalt bg); variant%16==0 → `·` lane mark |
| Sidewalk | `░` |
| Alley | `' '` slightly lighter than Road |
| Plaza | `▫` every (x+y)%3==0 else `' '` |
| Floor | `' '` floor bg; variant%23==0 → `·` |
| Wall | box drawing by mask |
| Door | `▯` (occupied → `▭`, handled by walker overlay) |
| Window | `▫` lit by light |
| Rooftop | `style.glyphs.mass[variant%len]` dim |
| Walkway | `═` / `║` by neighbour axis (mask has E|W → `═`, else `║`) |
| Bridge | `╍` / `╏` by axis |
| Tunnel | `' '` dark bg; variant%19==0 → `.` |
| Rail | `╬` blink every 20 ticks → `╫` |
| Stair | `▲` if connector serves a higher stratum than current, `▼` if lower, both → `⇅` |
| Lift | `≡` with indicator `⇅` when `(tick/10)%2==0` |
| Ladder | `╫` |
| Grate | `▦` |

Features override the glyph (and fg): Neon(h) → `▪` fg `hue(h) * flicker`; HoloAd → `holo[(tick/8+variant)%len]` fg accent2; Lamp → `●` fg yellow-white; Vent → steam if `(tick/4+variant)%10<3` → `steam[..]` else `☰`; Puddle → `~` fg bluish; Trash → `%`; Barrier → `▬` fg red; Stall → `▤` fg accent; Sign(c) → `c` fg accent2; Terminal → `▣` fg cyan; Camera → `◉` fg red (blink `(tick/15)%4==0` → `○`); AcUnit → `▩`; Helipad → `H`; Pipe → `╌`; Cable → `╌` dim.

Colors: base `fg/bg` from `ThemeStyle` (fallback neutral when `theme` is None), stratum `tint` added, then `lit(light)`. `Remembered` → `apply_vis` scales to 0.35 and replaces animated glyphs with their static variant (pass `tick = 0`).

- [ ] **Step 4: Run tests** → PASS; gate.
- [ ] **Step 5: Commit** `feat(city): style pack, tileset and lighting`.

---

### Task 21: Viewport, camera, minimap, weather, traffic

**Files:**
- Create: `render/viewport.rs`, `render/minimap.rs`, `render/weather.rs`, `render/traffic.rs`

**Interfaces:**
- Produces:
  - `Camera { x: f32, y: f32, tx: i32, ty: i32 }` with `snap(p: Pos)`, `target(p: Pos)`, `follow(p: Pos, view_w, view_h)` (deadzone ⅓), `update()` (ease 0.25 toward target per tick), `origin(view_w, view_h) -> Pos` (top-left tile), `clamp(size, view_w, view_h)`.
  - `MapView<'a> { city: &'a City, stratum: StratumKind, origin: Pos, tick: u64, style: &'a StylePack, vis: Option<&'a Grid<Visibility>>, overlays: &'a [&'a dyn Overlay], cursor: Option<Pos> }` implementing `ratatui::widgets::Widget`; `MapView::screen_to_tile(area, origin, (sx, sy)) -> Pos` and `tile_to_screen`.
  - `trait Overlay { fn draw(&self, city: &City, stratum: StratumKind, origin: Pos, area: Rect, tick: u64, buf: &mut Buffer, style: &StylePack); }`.
  - `weather::Weather` (unit struct) implementing `Overlay`: rain density from `style.stratum(k).rain` (0 none, 1 light=1/40 cells, 2 medium=1/20, 3 heavy=1/10); cell `(sx,sy)` has a drop when `hash64(tick/1 + sy as u64, (sx as i64 - (tick/2) as i64 % 3) as u64) % density_div == 0`; drop glyph from `rain` set; fg steel blue; only over tiles whose terrain is not Wall/Floor/Door (rain falls outdoors). Fog when `fog` flag: cells with `light < 40` get `░` in dim green with p = hash%6==0.
  - `traffic::Traffic { cars: Vec<Car { pos: Pos, dir: Dir, hue: Hue, age: u32 }> }` with `update(&mut self, city, stratum, origin, area, tick)` (only on Street; every 2 ticks move each car 1 tile along `Road` keeping direction, turning to a random Road neighbour at dead ends, despawn after 600 ticks or off-road; spawn up to 1 car per 10 ticks at a random arterial Road tile on the viewport edge, max 24 cars) and `impl Overlay` drawing `▬` with headlight tint on the tile ahead.
  - `Minimap<'a> { city, stratum, view: Rect (in tiles), player: Option<Pos>, style }` implementing `Widget`: 1 cell per 4×4 tiles, bg = district accent scaled by walkable fraction, `┌┐└┘│─` for the viewport rect, `@` for player.

- [ ] **Step 1: Tests**

```rust
#[test] fn mapview_clips_at_all_edges_and_tiny_areas() {
    let c = Generator::run(GenParams { seed: 3, size: SizePreset::Custom { w: 80, h: 40 }, districts: 4, ..Default::default() }).unwrap();
    let s = StylePack::load().unwrap();
    for origin in [Pos::new(-50, -50), Pos::new(70, 30), Pos::new(0, 0), Pos::new(200, 200)] {
        for (w, h) in [(1, 1), (120, 40), (10, 3)] {
            let backend = TestBackend::new(w, h); let mut t = Terminal::new(backend).unwrap();
            t.draw(|f| MapView { city: &c, stratum: StratumKind::Street, origin, tick: 5, style: &s, vis: None, overlays: &[&Weather], cursor: None }.render(f.area(), f.buffer_mut())).unwrap();
        }
    }
}
#[test] fn camera_follow_has_deadzone_and_eases() {
    let mut cam = Camera::default(); cam.snap(Pos::new(50, 50));
    cam.follow(Pos::new(52, 50), 60, 30); assert_eq!((cam.tx, cam.ty), (50, 50)); // inside deadzone
    cam.follow(Pos::new(90, 50), 60, 30); assert!(cam.tx > 50);
    let before = cam.x; cam.update(); assert!(cam.x > before && cam.x < cam.tx as f32);
}
#[test] fn traffic_stays_on_roads() {
    let c = Generator::run(GenParams { seed: 3, size: SizePreset::Custom { w: 120, h: 60 }, districts: 5, ..Default::default() }).unwrap();
    let mut tr = Traffic::default();
    for tick in 0..400 { tr.update(&c, StratumKind::Street, Pos::new(0, 0), Rect::new(0, 0, 120, 60), tick); }
    assert!(!tr.cars.is_empty());
    for car in &tr.cars { assert_eq!(c.tile(StratumKind::Street, car.pos).unwrap().terrain, Terrain::Road); }
}
```

- [ ] **Step 2–3:** implement. `MapView::render`: for each `(sx, sy)` in area: `p = origin + (sx - area.x, sy - area.y)`; tile = `city.tile(stratum, p)` else draw background; build `RenderCtx` (theme from tile.district, light from stratum.light, connector from `connectors_at`, kind from building, wall_mask from 4 neighbours' `Wall|Door|Window`); `vis` from grid when present; write cell; then `for o in overlays { o.draw(...) }`; finally if `cursor == Some(p)` invert fg/bg.
- [ ] **Step 4: Run tests** → PASS; gate.
- [ ] **Step 5: Commit** `feat(city): map viewport, camera, minimap, weather and traffic overlays`.

---

### Task 22: Digital (Net) overlay

**Files:**
- Create: `render/overlay.rs`

**Interfaces:**
- Produces: `Digital<'a> { enabled: bool, focus: Option<NodeId> }` implementing `Overlay`: for nodes anchored on the current stratum within the view, write glyph by kind (`net` glyph set index: Terminal 0 `◇`, Server 1 `◆`, Backbone 2 `⬡`, DataVault 3 `⌗`, Camera/DoorLock/LiftCtrl/AdBoard/LampGrid 4 `▪`, Ice 5 `◈`) fg signal cyan `Rgb(0,255,200)` pulsing with `pulse(tick, 40)`; building subnets: footprint border tiles get bg tinted `+0,+25,+25`; when `focus` is a node: its subnet's nodes glow white and each `Link` touching them draws `bresenham` dots `·` in cyan between anchors (skipping cells that hold a node glyph).
- Also `Digital::node_under(city, p: Pos, stratum) -> Option<NodeId>` for the inspector.

- [ ] **Step 1: Test**

```rust
#[test] fn overlay_draws_nodes_on_current_stratum_only() {
    let c = Generator::run(GenParams { seed: 8, size: SizePreset::Custom { w: 80, h: 40 }, districts: 4, ..Default::default() }).unwrap();
    let s = StylePack::load().unwrap();
    let node = c.net.nodes.iter().find(|n| n.kind == NodeKind::Server).unwrap();
    let (k, p) = node.anchor;
    let mut t = Terminal::new(TestBackend::new(80, 40)).unwrap();
    t.draw(|f| MapView { city: &c, stratum: k, origin: Pos::new(0, 0), tick: 0, style: &s, vis: None, overlays: &[&Digital { enabled: true, focus: None }], cursor: None }.render(f.area(), f.buffer_mut())).unwrap();
    assert_eq!(t.backend().buffer()[(p.x as u16, p.y as u16)].symbol(), "◆");
    let other = StratumKind::PHYSICAL.iter().copied().find(|s| *s != k).unwrap();
    t.draw(|f| MapView { city: &c, stratum: other, origin: Pos::new(0, 0), tick: 0, style: &s, vis: None, overlays: &[&Digital { enabled: true, focus: None }], cursor: None }.render(f.area(), f.buffer_mut())).unwrap();
    assert_ne!(t.backend().buffer()[(p.x as u16, p.y as u16)].symbol(), "◆");
}
```
- [ ] **Step 2–3:** implement; **Step 4:** PASS + gate; **Step 5:** commit `feat(city): digital net overlay`.

---

### Task 23: UI widgets: logo, menu, form, panels, transitions

**Files:**
- Create: `ui/mod.rs`, `ui/logo.rs`, `ui/menu.rs`, `ui/form.rs`, `ui/panel.rs`, `ui/transition.rs`
- Modify: `app.rs` (replace stub `Transition`)

**Interfaces:**
- Produces:
  - `logo::Logo { text: &str, tick, accent: Rgb }` Widget: 5-row block font (glyphs `█▀▄ ` ) for `A–Z`, `0–9`, space; each letter 6 cols; neon `flicker` per letter (variant = letter index), a scanline row (`tick/2 % (rows+10)`) brightened. `logo::width(text) -> u16`.
  - `menu::Menu { items: Vec<String>, selected: usize, opened: u64 }` with `new(items, tick)`, `up()`, `down()`, `Widget for MenuView<'a> { menu, tick, accent }`: item i slides from `−len` to 0 over 8 ticks starting at `opened + i*4` (OutCubic), typewriter in the same window, selected shows `▸ ` with fg `accent.scale(0.6 + 0.4*pulse(tick, 30))`.
  - `form::Field { Number { label, value: i64, min, max, step }, Slider { label, value: f32, min, max, step }, Choice { label, options: Vec<String>, idx }, Text { label, value: String, max }, Button { label } }`, `Form { fields, selected }` with `handle(key) -> Option<usize /*button idx pressed*/>` (Up/Down move; Left/Right adjust Number/Slider/Choice; chars/Backspace edit Text; Enter on Button returns its index), `FormView<'a> { form, tick, accent }` Widget drawing `label …… value` lines, sliders as `━━━◉━━━` 20 wide, selected line bright with `▸`.
  - `panel::{inspector_lines(city, stratum, p, net_node: Option<&Node>) -> Vec<Line>, legend(stratum, style) -> Vec<Line>, strata_selector(current, tick) -> Vec<Line>, status_bar(city: Option<&City>, stratum, fps: f32, hints: &str) -> Line, boxed(title) -> Block}`.
  - `transition::{TransitionKind { None, Glitch, SlideUp }, Transition { kind, from: Buffer, start: u64, dur: u64 (Glitch 6 ticks, SlideUp 8) }, new(kind, from, start), active(tick), apply(&self, f: &mut Frame, tick)}`: Glitch → for `glitch_cells(area, tick, p)` write chars from `GLITCH_CHARS` with cyan/magenta fg, and for `p < 0.5` copy `from` cells for rows where `hash(tick,row)%3==0`; SlideUp → draw `from` shifted up by `p*height` rows over the new frame for rows above the shift line.

- [ ] **Step 1: Tests**

```rust
#[test] fn menu_slides_in_and_selects() {
    let m = Menu::new(vec!["NEW CITY".into(), "LOAD CITY".into(), "QUIT".into()], 0);
    let mut t = Terminal::new(TestBackend::new(40, 10)).unwrap();
    t.draw(|f| MenuView { menu: &m, tick: 0, accent: Rgb(0, 255, 255) }.render(Rect::new(2, 2, 30, 6), f.buffer_mut())).unwrap();
    let row = (0..40).map(|x| t.backend().buffer()[(x, 2)].symbol().to_string()).collect::<String>();
    assert!(!row.contains("NEW CITY"));
    t.draw(|f| MenuView { menu: &m, tick: 40, accent: Rgb(0, 255, 255) }.render(Rect::new(2, 2, 30, 6), f.buffer_mut())).unwrap();
    let rows: Vec<String> = (2..5).map(|y| (0..40).map(|x| t.backend().buffer()[(x, y)].symbol().to_string()).collect()).collect();
    assert!(rows[0].contains("▸ NEW CITY")); assert!(rows[1].contains("LOAD CITY")); assert!(rows[2].contains("QUIT"));
}
#[test] fn form_adjusts_and_fires_button() {
    let mut f = Form { fields: vec![Field::Number { label: "Districts".into(), value: 7, min: 4, max: 12, step: 1 }, Field::Button { label: "GENERATE".into() }], selected: 0 };
    assert_eq!(f.handle(KeyCode::Right), None); assert!(matches!(f.fields[0], Field::Number { value: 8, .. }));
    for _ in 0..10 { f.handle(KeyCode::Right); } assert!(matches!(f.fields[0], Field::Number { value: 12, .. }));
    f.handle(KeyCode::Down); assert_eq!(f.handle(KeyCode::Enter), Some(1));
}
#[test] fn logo_width_and_render() {
    assert_eq!(logo::width("AB"), 12);
    let mut t = Terminal::new(TestBackend::new(60, 7)).unwrap();
    t.draw(|f| Logo { text: "CITY 9", tick: 3, accent: Rgb(255, 0, 200) }.render(f.area(), f.buffer_mut())).unwrap();
    assert!((0..60).any(|x| t.backend().buffer()[(x, 0)].symbol() == "█"));
}
#[test] fn glitch_transition_ends() {
    let from = Buffer::empty(Rect::new(0, 0, 20, 5));
    let tr = Transition::new(TransitionKind::Glitch, from, 10);
    assert!(tr.active(12)); assert!(!tr.active(17));
}
```
`handle` takes `KeyCode` (crossterm) for testability.

- [ ] **Step 2–3:** implement; the block font is a `const FONT: [(char, [&str; 5]); 37]` table (write all 37; e.g. `('A', ["▄▀▀▄ ", "█  █ ", "█▀▀█ ", "█  █ ", "▀  ▀ "])`, each row exactly 5 chars plus a 1-col gap = 6 cols).
- [ ] **Step 4: Run tests** → PASS; gate.
- [ ] **Step 5: Commit** `feat(city): logo, menu, form, panels and transitions`.

---

### Task 24: Title, Configure and Files screens

**Files:**
- Create: `screens/title.rs`, `screens/configure.rs`, `screens/files.rs`
- Modify: `screens/mod.rs`, `main.rs` (start on `Title`), remove `Blank`

**Interfaces:**
- Produces:
  - `Title::new(tick) -> Title` (Screen). Draw: `Logo { text: "CYBERPUNK" }` centred in the top third with accent magenta; subtitle "city generator" typewritten; `MenuView` centred below; bottom hint "↑↓ select · enter confirm · q quit". Update: Up/Down/j/k move; Enter → `NEW CITY` ⇒ `Switch { Configure::new(st.params.clone(), tick), Glitch }`, `LOAD CITY` ⇒ `Switch { Files::load(tick), SlideUp }`, `QUIT` ⇒ `Quit`; `q`/Esc ⇒ Quit.
  - `Configure::new(params, tick)` (Screen). Form fields in order: Text seed (digits only, max 20; empty ⇒ random), Choice size [small, medium, large], Number districts 4..12, Slider density 0..1 step .05, Slider wealth skew −1..1 step .1, Slider decay 0..1 step .05, Choice water [none, bay, river], Choice flavor [default], Button RANDOM SEED, Button GENERATE. Right side shows a live "preview card": chosen size in tiles, estimated buildings (`area/900 * density`), current seed. Update: `handle(key)`; RANDOM SEED fills the seed from `hash64(tick, 0xC0FFEE)`; GENERATE builds `GenParams`, runs `validate()`; on error show it in red under the form (status for 90 ticks); on success `st.params = params` and `Switch { Generating::new(params, tick)?, Glitch }` (if `Generator::new` fails, show the error instead). Esc ⇒ `Switch { Title, SlideUp }`.
  - `Files::load(tick)` and `Files::save(tick, city_name)` (Screen). Load mode: list from `save::list_cities(default_dir())` rendered as rows `name · seed · WxH · modified`, Up/Down select, Enter ⇒ `load_city` → `st.city = Some(city); st.player = None; Switch { MapScreen::explore(tick), Glitch }`, errors shown in status; Esc ⇒ Title. Save mode: a single `Field::Text` prefilled with the city name, Enter ⇒ `save_city(default_dir().join(format!("{name}.city")))` (create dir), status "saved <path>" and `Switch { MapScreen::explore(tick), None }`; Esc ⇒ back to the map.

- [ ] **Step 1: Tests**

```rust
#[test] fn title_renders_logo_and_menu_and_quits() {
    let mut st = AppState::new();
    let mut title = Title::new(0);
    let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
    st.tick = 60;
    t.draw(|f| title.draw(&st, f)).unwrap();
    let text = buffer_text(t.backend().buffer());
    assert!(text.contains("NEW CITY") && text.contains("LOAD CITY") && text.contains("QUIT"));
    assert!(matches!(title.update(&mut st, &[key(KeyCode::Char('q'))]), ScreenCmd::Quit));
    title.update(&mut st, &[key(KeyCode::Down), key(KeyCode::Down)]);
    assert!(matches!(title.update(&mut st, &[key(KeyCode::Enter)]), ScreenCmd::Quit));
}
#[test] fn configure_generate_switches_screens() {
    let mut st = AppState::new();
    let mut cfg = Configure::new(GenParams { size: SizePreset::Custom { w: 80, h: 40 }, districts: 4, ..Default::default() }, 0);
    for _ in 0..9 { cfg.update(&mut st, &[key(KeyCode::Down)]); } // to GENERATE
    assert!(matches!(cfg.update(&mut st, &[key(KeyCode::Enter)]), ScreenCmd::Switch { .. }));
}
```
Test helpers in `screens/mod.rs` under `#[cfg(test)]`: `key(code) -> KeyEvent`, `buffer_text(&Buffer) -> String`.

- [ ] **Step 2–3:** implement; **Step 4:** PASS + gate; **Step 5:** commit `feat(city): title, configure and files screens`.

---

### Task 25: Generating screen

**Files:**
- Create: `screens/generating.rs`

**Interfaces:**
- Produces: `Generating::new(params, tick) -> anyhow::Result<Generating>` (holds `Generator`, `steps_per_frame: u32 = 8`, `paused: bool`, `stage_done_tick: [Option<u64>; 11]`, `cam: Camera`, `error: Option<GenError>`). Update: if not paused and not finished, call `gen.step()` up to `steps_per_frame` times (Enter ⇒ loop until `Done`); Space toggles pause; `+`/`-` doubles/halves steps (1..=64); when `Done`: `gen.finish()` → `st.city = Some(city); st.player = None; Switch { MapScreen::explore(tick), Glitch }` or keep the error on screen with hint `r reseed · esc back`; `r` ⇒ new `Generating` with `params.seed = hash64(seed, tick)`; Esc ⇒ Configure. Draw: left `MapView` of Street (origin = camera centred on map centre, camera slowly drifts: `tx = cx + (sin(tick/90)*w/6)`), right panel 34 wide: title "GENERATING <seed>", one row per `Stage::ALL`: `[✓]` done (dim), `[▸]` active with a 16-wide progress bar `▓▓░░ 62%`, `[ ]` pending; bottom hint `space pause · +/- speed · enter skip`.

- [ ] **Step 1: Test**

```rust
#[test] fn generating_reaches_explore() {
    let mut st = AppState::new();
    let mut g = Generating::new(GenParams { seed: 2, size: SizePreset::Custom { w: 80, h: 40 }, districts: 4, ..Default::default() }, 0).unwrap();
    let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
    t.draw(|f| g.draw(&st, f)).unwrap();
    assert!(buffer_text(t.backend().buffer()).contains("Terrain"));
    let mut cmd = ScreenCmd::Stay; for _ in 0..100_000 { st.tick += 1; cmd = g.update(&mut st, &[key(KeyCode::Enter)]); if !matches!(cmd, ScreenCmd::Stay) { break; } }
    assert!(matches!(cmd, ScreenCmd::Switch { .. })); assert!(st.city.is_some());
}
```
- [ ] **Step 2–3:** implement; **Step 4:** PASS + gate; **Step 5:** commit `feat(city): animated generating screen`.

---

### Task 26: Walker: player, movement, connectors, FOV

**Files:**
- Create: `walker/mod.rs`, `walker/fov.rs`
- Modify: `app.rs` (add `player: Option<Player>` to `AppState`)

**Interfaces:**
- Produces:
  - `fov::compute(origin: Pos, radius: i32, blocks: impl Fn(Pos) -> bool, in_bounds: impl Fn(Pos) -> bool, mut mark: impl FnMut(Pos))` — symmetric shadowcasting over 4 quadrants with rational slopes (Albert Ford's algorithm): rows expand from the origin; each row scans tiles between `start_slope` and `end_slope`; a wall tile ends the current view segment, a floor after a wall starts a new one; symmetric rule: a floor tile is visible only if its centre slope lies within `[start, end]`.
  - `Player { pos: Pos, stratum: StratumKind, memory: Vec<Grid<Visibility>> (one per physical stratum), radius_street: i32 = 14, radius_under: i32 = 8 }` with `spawn(city, rng_seed: u64) -> Player` (random public connector on Street in a non-CorpCore district; fallback: first walkable Street tile), `try_move(&mut self, city, dir) -> bool`, `use_connector(&mut self, city, up: bool) -> Option<StratumKind>` (looks up `connectors_at(pos)` serving the current stratum; picks the neighbour above/below in `serves`; requires destination walkable), `update_fov(&mut self, city)` (sets previously Visible → Remembered on the current stratum, then marks Visible), `vis(&self) -> &Grid<Visibility>`.

- [ ] **Step 1: Tests**

```rust
#[test] fn fov_symmetric_and_walls_block() {
    // 9x9 room, wall column at x=4 rows 0..=6, opening at (4,7),(4,8)
    let wall = |p: Pos| p.x == 4 && p.y <= 6;
    let bounds = |p: Pos| (0..9).contains(&p.x) && (0..9).contains(&p.y);
    let see = |from: Pos| { let mut v = std::collections::HashSet::new(); compute(from, 12, wall, bounds, |p| { v.insert(p); }); v };
    let a = Pos::new(1, 1); let b = Pos::new(7, 1);
    assert_eq!(see(a).contains(&b), see(b).contains(&a)); assert!(!see(a).contains(&b));
    assert!(see(a).contains(&Pos::new(4, 1)));                    // the wall itself is visible
    assert!(see(Pos::new(1, 8)).contains(&Pos::new(7, 8)));       // through the opening
    assert!(!see(a).contains(&Pos::new(1, 1 + 13)));              // radius
}
#[test] fn player_moves_and_uses_connectors() {
    let c = Generator::run(GenParams { seed: 6, size: SizePreset::Custom { w: 120, h: 60 }, districts: 5, ..Default::default() }).unwrap();
    let mut p = Player::spawn(&c, 1);
    assert!(c.walkable(p.stratum, p.pos));
    let conn = c.connectors.iter().find(|k| k.public && k.serves.contains(&StratumKind::Undercity)).unwrap();
    p.pos = conn.pos; p.stratum = StratumKind::Street;
    assert_eq!(p.use_connector(&c, false), Some(StratumKind::Undercity));
    assert_eq!(p.use_connector(&c, false), None);
    assert_eq!(p.use_connector(&c, true), Some(StratumKind::Street));
    p.update_fov(&c); assert_eq!(p.vis()[p.pos], Visibility::Visible);
    let moved = Dir::ALL.iter().any(|d| { let mut q = p.clone(); q.try_move(&c, *d) });
    assert!(moved);
    let mut q = p.clone(); q.pos = Pos::new(-1, -1); assert!(!q.try_move(&c, Dir::N) || c.walkable(q.stratum, q.pos));
}
```
- [ ] **Step 2–3:** implement; **Step 4:** PASS + gate; **Step 5:** commit `feat(city): walker with symmetric shadowcasting FOV`.

---

### Task 27: Explore and Walk screen, help overlay, save wiring

**Files:**
- Create: `screens/explore.rs`, `screens/help.rs`
- Modify: `screens/mod.rs`

**Interfaces:**
- Produces: `MapScreen { mode: Mode { Explore, Walk }, stratum, cam: Camera, cursor: Option<Pos>, show_inspector: bool (default true), show_minimap: bool (true), show_legend: bool (false), digital: bool (false), help: bool, traffic: Traffic, fps: FpsMeter }` with `MapScreen::explore(tick)`; Screen impl:
  - Layout: map area = full width minus 34 when inspector shown; status bar bottom row; minimap 24×12 bottom-right over the map; legend replaces inspector content when toggled; help overlay centred 60×20 boxed.
  - Explore keys: arrows/hjkl pan 1 (Shift ×8), `[`/`]` stratum down/up, `i` `m` `l` `d` toggles, `w` ⇒ Walk (spawn `Player` via `Player::spawn(city, hash64(seed, tick))` if `st.player` is None; set stratum to player's), `s` ⇒ `Switch { Files::save(tick, name), None }`, `r` ⇒ `Generating::new(params.with_seed(hash64(seed, tick)))`, `g` ⇒ Configure, `?` help, `q`/Esc ⇒ Title. Cursor in Explore = map centre tile (inspector describes it).
  - Walk keys: hjkl/yubn/arrows ⇒ `try_move` then `update_fov`; `<`/`>` ⇒ `use_connector` (status "no connector here" when None); `.` wait; Tab ⇒ Explore; toggles and `s`/`?`/`q` as above. Camera `follow(player)`. Inspector shows the tile under `@`, plus the lift panel (`serves` list with `▸` on current) when standing on a connector.
  - Draw: `MapView` with `vis: Some(player.vis())` in Walk mode, overlays `[&Weather, &traffic, &Digital{..}]`, player `@` written last in bright white with `Door` under player shown `▭`; `status_bar` with hints per mode.
  - `help::draw(f, mode)`: key list for the current mode.
  - `FpsMeter { last: Instant, frames: u32, fps: f32 }` updated in `update`.

- [ ] **Step 1: Tests**

```rust
#[test] fn explore_and_walk_round_trip() {
    let mut st = AppState::new();
    st.city = Some(Generator::run(GenParams { seed: 6, size: SizePreset::Custom { w: 120, h: 60 }, districts: 5, ..Default::default() }).unwrap());
    let mut s = MapScreen::explore(0);
    let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
    t.draw(|f| s.draw(&st, f)).unwrap();
    assert!(buffer_text(t.backend().buffer()).contains("Street"));
    s.update(&mut st, &[key(KeyCode::Char(']'))]); assert_eq!(s.stratum, StratumKind::Skyways);
    s.update(&mut st, &[key(KeyCode::Char('w'))]); assert!(st.player.is_some()); assert_eq!(s.mode, Mode::Walk);
    let before = st.player.as_ref().unwrap().pos;
    for d in ['h', 'j', 'k', 'l'] { s.update(&mut st, &[key(KeyCode::Char(d))]); }
    let _ = before;
    t.draw(|f| s.draw(&st, f)).unwrap();
    assert!(buffer_text(t.backend().buffer()).contains('@'));
    s.update(&mut st, &[key(KeyCode::Tab)]); assert_eq!(s.mode, Mode::Explore);
    assert!(matches!(s.update(&mut st, &[key(KeyCode::Char('q'))]), ScreenCmd::Switch { .. }));
}
```
- [ ] **Step 2–3:** implement; **Step 4:** PASS + gate; **Step 5:** commit `feat(city): explore and walk screens with panels and help`.

---

### Task 28: README, smoke run, final gate

**Files:**
- Create: `README.md`
- Modify: none

- [ ] **Step 1:** `README.md`: what it is, `cargo run -p city`, `cargo run -p city -- gen --seed 42 --size medium --out cities/demo.city`, key reference per screen, the four strata, and a pointer to the spec and plan.
- [ ] **Step 2:** Headless smoke: `cargo run --release -p city -- gen --seed 42 --size large --out /tmp/smoke.city --json` and `cargo run --release -p city -- gen --seed 42 --size large --out /tmp/smoke.city`; both succeed; file sizes printed.
- [ ] **Step 3:** Full gate: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`.
- [ ] **Step 4:** Interactive smoke (manual, by a person in a real terminal): launch, generate medium, pan all strata, toggle `d`, enter Walk, use a manhole and a lift, save, quit, load. Record results in the final report.
- [ ] **Step 5:** Commit `docs: README and smoke-tested release candidate`.
