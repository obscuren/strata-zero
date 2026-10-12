# City Generator — Design Spec

Date: 2026-10-12
Status: approved in brainstorming, ready for implementation plan

## 1. Purpose

A terminal game in a full cyberpunk setting, built from scratch in this repo.
The first deliverable is the **city generator and its interactive app**: a
seeded, deterministic generator that produces a multi-level cyberpunk city as
walkable tile maps, wrapped in a full-screen terminal application with animated
tiles and animated menus, rich UTF-8 glyphs, and a controllable `@` to prove the
city is playable as a roguelike space.

### Success criteria

Run the binary, land in an animated title menu, configure a seed and
parameters, watch the city grow stage by stage, pan across four strata that read
as cyberpunk at a glance, toggle a digital overlay showing the Net, then walk an
`@` from a sewer tunnel up a lift to a corp rooftop. Save the city, quit, reload
it: same city. Same seed and parameters: byte-identical city, every time.

### Out of scope for this deliverable

- Combat, inventory, NPCs with logic, dialogue, quests.
- The cyberspace stratum as a playable space, ICE behavior, hacking actions,
  netrunning rules. The Net *graph* and its *overlay* are in scope; playing from
  inside the Net is a later spec.
- Pedestrian simulation. Traffic is in scope as a purely visual overlay.
- Game saves. Only the city is persisted.
- Mouse input, 256-color design target, Windows testing (crossterm keeps the
  door open; macOS is the development target).

## 2. Decisions

| Topic | Decision |
|---|---|
| Game type | Roguelike / RPG on foot: tile-level walkable maps with interiors |
| Vertical model | Named strata, each a full tile map: Undercity, Street, Skyways, Heights. Open-ended list; a `Net` stratum is reserved for later |
| Stack | Rust (edition 2024, stable toolchain) + ratatui + crossterm |
| First cut | Generate, watch, explore, walk (`@` with FOV, collision, stairs and lifts), save and load |
| Fiction | Shared cyberpunk canon (Night City, Blade Runner, the Sprawl). Night City's district model. Original names by default, kept in a swappable flavor data file |
| Generation approach | Top-down hierarchical plan (districts → roads → blocks → lots → buildings → verticals → interiors → connectivity → dressing → naming → net), with noise fields used only for wealth/decay modulation in dressing |
| Default size | 240 × 120 tiles per stratum (reads roughly square in 1:2 terminal cells) |
| Districts | 6–8 per city by default, each with a theme driving building mix, street style, palette and signage |
| Determinism | Same seed + params ⇒ byte-identical city |
| Terminal target | Modern truecolor terminals (iTerm, Ghostty, kitty, WezTerm). Must not crash on 256 colors |

## 3. Architecture

### Workspace

```
cyberpunk/
  Cargo.toml                 workspace, resolver 3, edition 2024
  rust-toolchain.toml        stable
  crates/
    citygen/                 lib: world model + generator + net graph
                             deps: rand, rand_chacha, noise, serde, thiserror
                             NO terminal dependencies, ever
    city/                    bin: the app and CLI
                             deps: ratatui, crossterm, citygen, serde, postcard, anyhow, clap
      src/main.rs            CLI parse → run app or headless `gen`
      src/app.rs             App state, screen state machine, frame loop
      src/screens/           title, configure, generating, explore, walk, files
      src/render/            tileset, lighting, weather, traffic, overlay (net), viewport, minimap
      src/anim/              timeline, easing, effects (glitch, typewriter, flicker, slide)
      src/ui/                menu, form, panel, status bar widgets built on anim
      src/walker/            player, fov, movement
  flavor/
    names.ron                embedded by citygen: syllables, corp names, landmark templates
    style.ron                embedded by city: theme palettes, glyph sets
  docs/superpowers/specs/
  cities/                    runtime save directory (gitignored)
```

The future game links `citygen` directly and lifts `render`, `anim`, `ui` and
`walker` out of `city` into crates when it needs them. They are modules with
clean boundaries now, crates later.

### Boundaries

- `citygen` knows nothing about colors, glyphs or terminals. It produces data.
- `render` is a set of pure functions from world data + render context to
  styled cells. It is testable with ratatui's `TestBackend`.
- `anim` knows nothing about the city. It maps ticks to progress values and
  applies effects to text/cells.
- `screens` compose `render`, `ui` and `walker` and own input handling.

### Frame loop

Fixed 30 frames per second. Each frame:

1. Poll crossterm events with a timeout equal to the remaining frame budget.
2. Increment the global `tick: u64`.
3. `screen.update(&mut app, input)` — may transition to another screen.
4. `screen.draw(frame, &app)`.

All animation is a pure function of `tick` (and tile position/variant), never
of wall-clock time, so any frame can be reproduced in a test by setting `tick`.

## 4. World model (`citygen`)

All types derive `Serialize, Deserialize, Clone, Debug`; enums also `Copy,
PartialEq, Eq, Hash` where sensible.

```rust
pub struct City {
    pub seed: u64,
    pub params: GenParams,
    pub name: String,
    pub size: Size,                   // { w: u32, h: u32 }
    pub strata: Vec<Stratum>,         // physical strata, lookup by kind
    pub districts: Vec<District>,
    pub buildings: Vec<Building>,
    pub connectors: Vec<Connector>,
    pub net: Net,
}

pub enum StratumKind { Undercity, Street, Skyways, Heights, Net /* reserved */ }

pub struct Stratum {
    pub kind: StratumKind,
    pub tiles: Grid<Tile>,            // row-major, w*h
    pub light: Grid<u8>,              // 0 dark .. 255 bright, precomputed
}

pub struct Tile {
    pub terrain: Terrain,
    pub feature: Option<Feature>,
    pub district: Option<DistrictId>,
    pub building: Option<BuildingId>,
    pub variant: u8,                  // hash(seed, x, y, stratum): glyph pick + anim phase
}

pub enum Terrain {
    Void, Water, Sewage,
    Road, Sidewalk, Alley, Plaza,
    Floor, Wall, Door, Window,
    Rooftop, Walkway, Bridge, Tunnel, Rail,
    Stair, Lift, Ladder, Grate,
}

pub enum Feature {
    Neon(Hue), HoloAd, Lamp, Vent, Puddle, Trash, Barrier, Stall,
    Sign(SignGlyph), Terminal, Camera, AcUnit, Helipad, Pipe, Cable,
}

pub struct District {
    pub id: DistrictId,
    pub name: String,
    pub theme: Theme,
    pub center: Pos,
    pub wealth: f32,                  // 0..1
    pub decay: f32,                   // 0..1
    pub bounds: Rect,
}

pub enum Theme {
    CorpCore, NightMarket, Megablocks, CombatZone,
    Docks, Industrial, NeonStrip, OldTown,
}

pub struct Building {
    pub id: BuildingId,
    pub kind: BuildingKind,
    pub district: DistrictId,
    pub footprint: Rect,              // at Street level
    pub span: StrataSpan,             // lowest..=highest physical stratum occupied
    pub name: Option<String>,
}

pub enum BuildingKind {
    CorpTower, Office, Megablock, Apartment, Hotel,
    Shop, NoodleBar, Bar, Club, Arcade, Pawn, Clinic, Temple,
    Warehouse, Factory, Depot, Garage, Parking, Market,
}

pub struct Connector {
    pub kind: ConnectorKind,          // Stair, Lift, Ladder, Manhole, Jack /* reserved */
    pub pos: Pos,
    pub serves: Vec<StratumKind>,     // ordered low → high; Stair/Ladder/Manhole have exactly 2
    pub public: bool,                 // reachable from the street without entering a building
}
```

Walkability and opacity are **methods** on `Tile`, derived from terrain and
feature (`is_walkable()`, `blocks_sight()`), never stored. `variant` is
deterministic per tile so two neon signs never flicker in sync.

Glyph constraint: every tile is exactly one terminal cell. Signage uses
single-width glyphs only: half-width katakana (U+FF66–FF9D), Cyrillic, Greek,
currency and symbol blocks. No full-width CJK, no Hangul.

### Net graph

```rust
pub struct Net {
    pub subnets: Vec<Subnet>,
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,             // (NodeId, NodeId), undirected
}
pub struct Subnet { pub id: SubnetId, pub owner: SubnetOwner, pub security: u8 }
pub enum SubnetOwner { Building(BuildingId), District(DistrictId), City }
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub anchor: (StratumKind, Pos),
    pub subnet: SubnetId,
    pub building: Option<BuildingId>,
}
pub enum NodeKind {
    Terminal, Camera, DoorLock, LiftCtrl, AdBoard, LampGrid,
    Server, DataVault, Ice, Backbone,
}
```

The Net lives beside the tile grid, not inside it. Tiles are looked up by anchor
when rendering the overlay.

## 5. Generation pipeline (`citygen`)

### API

```rust
pub struct GenParams {
    pub seed: u64,
    pub size: SizePreset,             // Small 160×80, Medium 240×120, Large 320×160, Custom{w,h}
    pub districts: u8,                // 4..=12, default 7
    pub density: f32,                 // 0..1, default 0.6
    pub wealth_skew: f32,             // -1..1, default 0.0
    pub decay: f32,                   // 0..1, default 0.4
    pub water: WaterMode,             // None, Bay, River; default Bay
    pub flavor: String,               // flavor pack name, default "default"
}
impl GenParams { pub fn validate(&self) -> Result<(), GenError>; }

pub struct Generator { /* partial city + stage cursor */ }
impl Generator {
    pub fn new(params: GenParams) -> Result<Self, GenError>;
    pub fn step(&mut self) -> Progress;        // Running { stage, pct } | Done
    pub fn city(&self) -> &City;               // partial while running
    pub fn finish(self) -> Result<City, GenError>;  // runs City::validate()
    pub fn run(params: GenParams) -> Result<City, GenError>; // convenience
}
pub enum Stage {
    Terrain, Districts, Arterials, Streets, Lots, Verticals,
    Interiors, Connectivity, Dressing, Naming, Net,
}
```

Each stage is chunked into small steps (one road, one block, one building per
step). The UI calls `step()` several times per frame and draws `city()` in
between: that is the entire "watch it grow" animation. Headless callers loop to
completion. No threads, no snapshots.

### Determinism

One `ChaCha8Rng` seeded from `params.seed`. Each stage derives its own sub-RNG
from `seed ^ STAGE_CONST[stage]` so a change to one stage does not reshuffle
earlier ones. Noise fields (`noise` crate, OpenSimplex) take the same seed.
Iteration order over collections is always index order, never hash order.

### Stages

1. **Terrain.** Choose water per `WaterMode`: Bay carves a noisy coastline along
   one random edge covering 10–20% of the map; River cuts a meandering channel
   6–10 tiles wide; None leaves all land. Water is `Terrain::Water` on Street,
   `Void` above, `Sewage` channels later in Undercity.
2. **Districts.** Place `params.districts` seeds with Poisson-style minimum
   spacing on land. Grow Voronoi regions. Assign themes by rule, in order: CorpCore to the
   seed nearest the land centroid; Docks to a seed touching water (if any);
   Industrial adjacent to Docks (or edge if no water); CombatZone to the seed
   farthest from CorpCore; remaining seeds draw from the mix
   {Megablocks, NightMarket, NeonStrip, OldTown} without repeating until
   exhausted. Wealth = theme base + `wealth_skew` + low-frequency noise, clamped.
   Decay = theme base + `params.decay` + noise, clamped.
3. **Arterials.** Minimum spanning tree over district centers plus
   `max(2, districts/3)` extra edges forming loops. Route each edge with A* on a
   cost grid: water impassable, turns cost extra, existing roads cheap. Arterials
   are 4 tiles of `Road` with 1 tile of `Sidewalk` each side.
4. **Streets.** Per district, subdivide its region into blocks in its style:
   - CorpCore: orthogonal grid, blocks ~24×12, streets 2 wide + sidewalks.
   - NightMarket, OldTown: recursive split with jitter, alleys 1 wide, dead ends allowed.
   - Megablocks: 2–4 enormous blocks per district.
   - Docks, Industrial: long parallel rows, access roads every 20–30 tiles.
   - CombatZone: grid with 30% of streets removed and `Barrier` features at cuts.
   - NeonStrip: one wide main strip with tight side alleys.
5. **Lots.** Split each block into lots along the long axis (BSP) with minimum
   lot size from theme. Each lot draws a `BuildingKind` from the theme's
   weighted mix and a height class (Low: Street only; Mid: Street+Skyways;
   Tall: Street..Heights). CorpCore and Megablocks also get basements
   (Undercity). `density` scales the fraction of lots that become Plaza or
   Parking instead of buildings. Footprints are written as `Wall` perimeter +
   `Floor` interior on Street.
6. **Verticals.**
   - Undercity: `Tunnel` under every arterial and ⅓ of streets; `Sewage`
     channels under arterials with `Grate` crossings; maintenance chambers under
     buildings with basements; one `Rail` line across the map.
   - Skyways: `Walkway` between buildings with span ≥ Skyways when within 12
     tiles; `Walkway` platforms over intersections in CorpCore and NeonStrip;
     Mid/Tall buildings get `Wall`/`Floor` footprints at this stratum.
   - Heights: `Rooftop` over Tall buildings with `Helipad`/`AcUnit` features;
     `Bridge` between Tall buildings within 8 tiles; everything else `Void`.
7. **Interiors.** For each building and each stratum it spans: BSP rooms within
   the footprint (min room 3×3), corridors joining rooms, a `Door` on a
   street-facing wall at Street level, `Window` on street-facing walls, internal
   `Stair` (span 2) or `Lift` shaft (span ≥ 3) linking every stratum spanned.
8. **Connectivity.** Flood fill walkable tiles per stratum. Components smaller
   than 1% of walkables are filled `Void` (Undercity/Skyways/Heights) or joined
   (Street). Remaining disconnected components are joined by carving the
   shortest `Door`/`Tunnel`/`Walkway` path to the main component. Then build the
   connector graph across strata; add public connectors (street stairwells to
   Skyways, `Manhole` to Undercity, lift kiosks, fire ladders) in each district
   until every stratum is reachable from Street. At most 3 repair rounds, then
   `GenError::Unconnected { stratum }`.
9. **Dressing.** `Neon(hue)` and `HoloAd` on street-facing facades, density by
   theme and wealth; `Lamp` along sidewalks every 8–12 tiles; `Vent` (steam),
   `Puddle`, `Trash` scaled by decay; `Barrier` in CombatZone; `Stall` in
   NightMarket plazas; `Terminal` in lobbies and on some sidewalks; `Camera` on
   walls facing streets/corridors scaled by wealth; `Pipe`/`Cable` in Undercity;
   `Sign` glyphs above shop doors. Compute `light` per stratum: lamps and neon
   radiate with falloff, sky exposure adds base light (Heights > Street >
   Skyways > Undercity). Assign `variant` for every tile.
10. **Naming.** City name, district names (theme syllable tables), corp names
    for CorpTower/Office/Megablock, landmark templates for a few notable
    buildings per district. Flavor pack from `flavor/names.ron`.
11. **Net.** One `Subnet` per building (owner Building, security from theme +
    wealth), one per district (owner District, `Backbone` node at a Terminal in
    the district), one city core (owner City) in CorpCore. Nodes from existing
    features: `Terminal`→Terminal, `Camera`→Camera, `Door`→DoorLock,
    `Lift`→LiftCtrl, `HoloAd`→AdBoard, one `LampGrid` per district, one
    `Server` per building in an interior room, `DataVault` in CorpTower/Clinic,
    `Ice` anchored at servers with security ≥ 160. Links: node → its building's
    Server; Server → district Backbone; Backbone → city core.

### Validation

`City::validate() -> Result<(), Vec<Violation>>` checks:

- every `Connector` endpoint tile is walkable on every stratum it serves;
- every `Building` has at least one `Door` on Street;
- every stratum is reachable from Street via connectors;
- every tile's `district`/`building` ids exist;
- no two building footprints overlap;
- every Net node anchors to a tile of the expected terrain/feature;
- every district has ≥ 1 building and ≥ 1 public connector.

## 6. Rendering and animation (`city/src/render`)

### Tileset

```rust
pub fn cell(tile: &Tile, ctx: &RenderCtx) -> Cell
// RenderCtx: theme palette, light value, tick, pos, visibility, stratum kind
```

Pure function: glyph from terrain+feature+variant, color from theme palette,
light applied, animation phase from tick+variant. Tested by exact glyph/color
assertions.

### Glyph vocabulary (single width)

| Thing | Glyphs |
|---|---|
| Walls | light `─│┌┐└┘├┤┬┴┼` shops; heavy `━┃┏┓┗┛` megablocks; double `═║╔╗╚╝` corp |
| Mass / rooftop | `█▓▒░▀▄` by height class |
| Stairs / lifts / ladders / manholes | `▲▼` `⇅≡` `╫` `◍` |
| Water / sewage | `≈~∼` cycled |
| Rain | braille `⠁⠂⠄⡀⢀⠈⠐⠠` |
| Lamps | `●○◉`; vents `☰`, steam `°∘○` |
| Signage | half-width katakana, Cyrillic, Greek, `¥$€` |
| Rail / grate / cable / barrier / stall / holo ad | `╬` `▦▩` `╌` `▬` `▤` `▣` |
| Net overlay nodes | `◇◆⬡⌗` |

### Palette

Dark night base (`#0a0b10` background family). Theme accents:

| Theme | Accent pair |
|---|---|
| CorpCore | cold cyan, steel white |
| NightMarket | magenta, orange |
| Megablocks | concrete grey, amber |
| CombatZone | red, sodium yellow |
| Docks | teal, rust |
| Industrial | sulfur, sick green |
| NeonStrip | hot pink, electric blue |
| OldTown | warm paper, jade |

Stratum tint: Undercity toward green-black, Heights toward blue with more
light. Palettes live in `flavor/style.ron`.

### Overlay stack

Rendering a frame of the map is: base tiles → lighting → overlays in order:

1. **Tile phase** (inside `cell()`): neon flicker schedule from variant, holo ad
   glyph cycle, water ripple, vent puff, lift indicator, rail blink.
2. **Lighting**: static light map × slow flicker factor near neon. Undercity
   dark with pools of light; Heights brightest.
3. **Weather**: rain streaks falling one cell per tick with wind drift; density
   by stratum (none Undercity, light Street, medium Skyways, heavy Heights);
   thin fog overlay in Undercity.
4. **Traffic**: `▬` cars with headlight tint moving along arterials; spawned and
   despawned at viewport edges; position + direction only, no simulation.
5. **Digital overlay** (toggle `d`): Net nodes drawn as wireframe glyphs in
   signal cyan over their anchor tile; building subnet outline faint; when the
   inspector points at a node, its subnet's nodes brighten and links draw as
   Bresenham lines.
6. **Walker**: FOV shading (Visible normal, Remembered dimmed and unanimated,
   Unseen black) and the `@`.

### Viewport and panels

- Map viewport fills the left. Camera in tile coords, clipped to the stratum,
  eased toward its target over a few frames.
- Right panel (toggle `i`): tile inspector (coords, stratum, terrain, feature,
  district, building name, Net node if any), legend for the current stratum
  (toggle `l`), stratum selector showing all strata with the current one
  highlighted.
- Minimap (toggle `m`), bottom right: 1 cell per 4×4 tiles, colored by
  district, viewport rectangle and `@` marked.
- Status bar: city name, seed, stratum, FPS, key hints.
- Minimum terminal 100×30; below that, draw a centered resize message.

## 7. App flow (`city/src/screens`, `ui`, `anim`)

```
Title ──▶ Configure ──▶ Generating ──▶ Explore ◀──▶ Walk
  │                                      │
  └──▶ Files (load) ─────────────────────┘  (save from Explore/Walk)
```

- **Title.** Game name in block-element letters with neon flicker and a
  scanline sweep. Menu: New City, Load City, Quit. Items slide in from the left
  with typewriter reveal; selected item has pulsing `▸` and accent color.
- **Configure.** Form over `GenParams`: seed (random / typed), size preset,
  districts, density, wealth skew, decay, water, flavor. Sliders as `━━━◉━━━`.
  Generate button. Esc back.
- **Generating.** Street stratum grows in the viewport. Right panel: eleven
  stages as a checklist, active one with progress bar
  `[▸] Routing arterials  ▓▓▓▓▓░░░ 62%`. Space pause, `+`/`-` steps per
  frame (1..64, default 8), Enter skip to end. On `GenError`, show the error
  and offer `r` reseed. Glitch wipe into Explore.
- **Explore.** Free camera. Keys: arrows/hjkl pan, Shift fast pan, `[`/`]`
  stratum, `i` inspector, `m` minimap, `l` legend, `d` digital overlay, `w`
  Walk, `s` save, `r` regenerate with fresh seed, `g` Configure, `?` help,
  `q` Title.
- **Walk.** Camera follows `@`. hjkl/yubn/arrows move, `<`/`>` use connector,
  `.` wait, Tab back to Explore leaving `@` in place. Panels and `d` as above.
- **Files.** List `./cities/*.city` with name, seed, size, date. Enter loads,
  Esc back. Save prompts for a name, default the city name.

**Transitions**: every screen change is a 150–250 ms effect (glitch wipe of
shuffled cells, or vertical slide); any key skips it. Panels fade in by
sweeping their border.

**`anim`**: `Timeline` of tweens (start tick, duration, easing); `Effects`
providing glitch, typewriter, flicker, slide as functions of progress ∈ [0,1].

## 8. Walker (`city/src/walker`)

```rust
pub struct Player { pub pos: Pos, pub stratum: StratumKind, pub memory: HashMap<StratumKind, Grid<Visibility>> }
pub enum Visibility { Unseen, Remembered, Visible }
```

- Eight-direction movement; a move succeeds if the target `is_walkable()`.
  Doors are passable and render open while occupied.
- `<`/`>` on Stair/Ladder/Manhole: move to the other stratum. On Lift: step
  through `serves` in order; inspector shows the lift panel with current floor.
- FOV: symmetric shadowcasting. Radius 14 on Street/Skyways/Heights, 8 in
  Undercity. Memory per stratum.
- Spawn on first entering Walk: a random public connector on Street in a
  non-CorpCore district. `@` persists across Explore/Walk until regeneration.
- Camera follows with a deadzone of ⅓ viewport, eased.

## 9. Persistence and CLI (`city`)

- File: magic `CITY` (4 bytes), `u16` format version, postcard body of `City`.
  Extension `.city`, directory `./cities/`.
- Unknown/newer version ⇒ clear error, not a panic.
- CLI: `city` (interactive) and `city gen --seed N --size small|medium|large
  --out file.city [--json]`. `--json` writes serde_json instead. Nothing else.
- Player position is not saved.

## 10. Error handling

- `citygen`: `thiserror`, `GenError { InvalidParams { field, reason },
  Unconnected { stratum }, Flavor(String) }`. Never panics on valid params.
- `city`: `anyhow`. A terminal guard restores screen, raw mode and cursor on
  exit, error and panic (panic hook). Resize below 100×30 shows a message.

## 11. Testing

- `citygen` property-style tests over fixed seeds: 50 Small, 10 Medium, 3 Large
  in the default run; an `#[ignore]` extended test runs 200 seeds. Each seed:
  byte-identical output on two runs, `validate()` passes, Street reaches every
  stratum, every district has a building and a public connector, every Net node
  anchors correctly, no footprint overlap. One timing test: Medium under 2 s in
  debug.
- Per-stage unit tests on small hand-built cities: arterial router never
  crosses water; BSP never emits a room under 3×3; connectivity repair joins two
  halves; theme assignment rules hold.
- `city` via `TestBackend`: tileset exact glyph/color at fixed ticks; viewport
  clipping at all four edges; title menu at tick 0 and after slide-in; file
  header round-trip.
- FOV: symmetry, walls block, radius respected, Undercity radius.
- Gate: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`.

## 12. Dependencies

ratatui, crossterm, rand, rand_chacha, noise, serde (derive), postcard,
serde_json, thiserror, anyhow, clap (derive), ron. Latest stable majors at
implementation time.

## 13. Future work (not this deliverable)

- `StratumKind::Net` generated as a projection of the physical city; `Jack`
  connectors from terminals; ICE behavior; netrunning actions.
- NPCs and pedestrians, combat, inventory, factions per district.
- Extracting `render`, `anim`, `ui`, `walker` into crates for the game.
