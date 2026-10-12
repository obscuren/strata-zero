# Cyberspace: the Net as a playable stratum

**Date:** 2026-10-12
**Status:** approved in conversation (design agreed; roaming rule ruled by Claude)
**Builds on:** `2026-10-12-city-generator-design.md` (the city, the Net graph, the
digital overlay). This spec turns the reserved `StratumKind::Net` into a real,
walkable stratum and adds jacking in and out.

## 1. Intent

The player can jack in at any street terminal. Their body stays on the physical
map at the port; their consciousness emerges in cyberspace at the same spot and
walks it. Jacking out returns them to the body.

Cyberspace is its own world with the classic look: a black void, a dotted grid
floor, and the city's subnets standing on it as geometric constructs. It keeps
the city's geography one to one, so a construct stands where its building
stands, the district hubs sit where the backbone terminals are, and the corp
core is the brightest thing on the map. The digital overlay on the physical
strata and cyberspace are the same thing seen from two sides.

Basics only. The point is to walk it and see whether it is cool.

### In scope

- `StratumKind::Net` generated as a fifth tile map, walkable, saved with the city.
- Constructs (one per building), district hubs, link lanes, gates, nodes placed
  as tile features, ICE standing at construct gates.
- `Jack` connectors at street terminals; jack in / jack out with the body left
  behind; a distinct avatar; field of view and memory in cyberspace.
- Rendering: a Net tileset, animated lanes, the inspector and legend for the Net,
  the digital overlay drawing links in cyberspace, the strata selector showing
  Net on top, Explore mode able to view the Net stratum.
- Save format version 2.
- A bounded fix shipped alongside: the Explore cursor moves freely and the camera
  follows it, so edge tiles can be inspected.

### Not in scope (later specs)

ICE behaviour, hacking or any action on a node, skill gates on jacking in, a
cyberdeck or programs, NPC netrunners, lanes-only movement, damage or being
dumped. The Undercity, Skyways and Heights terminals do not get ports yet.

## 2. Rulings taken while designing

- **Free roaming.** The grid between constructs is walkable. Lanes are the lit
  paths, not the only paths. Cost if wrong: a lanes-only rule is a one-line
  walkability change later.
- **Nodes are tile features**, not overlay glyphs, on the Net stratum. They get
  field of view, memory, minimap and inspector treatment for free. Cost if wrong:
  a second anchor per node is carried in the save.
- **Ports are connectors, not tiles.** A `Jack` connector at a street terminal's
  tile is the port on both strata. The tileset draws the Net side brighter when a
  Jack is present. Cost if wrong: none; connectors are already per-tile data.
- **Avatar glyph** is `@` in signal cyan on the Net; the body is a dim grey `@`
  on its physical stratum. Cost if wrong: one glyph constant.
- **Version 1 saves are rejected** with a message that names the seed to
  regenerate from. Cost if wrong: the user regenerates a city in under a second.

## 3. World model changes (`citygen`)

### Terrain and features

```rust
pub enum Terrain {
    /* existing 20 ... */
    NetGrid,    // open cyberspace floor; walkable
    NetLane,    // a link drawn as a lane on the grid; walkable
    NetWall,    // construct or hub wall; blocks sight; not walkable
    NetFloor,   // inside a construct; walkable
    NetGate,    // opening in a construct or hub wall; walkable
}
pub const ALL_TERRAIN: [Terrain; 25];

pub enum Feature {
    /* existing 15 ... */
    Node(NodeKind),   // a Net node standing on this cyberspace tile
}
pub const ALL_FEATURES: [Feature; 16];   // adds Feature::Node(NodeKind::Server)

impl Terrain {
    /// Human label for the inspector and legend ("construct wall", "lane", "road").
    pub fn label(self) -> &'static str;
}
```

Walkability: `NetGrid | NetLane | NetFloor | NetGate` walk; `NetWall` does not.
`Feature::Node(_)` never blocks walking in this spec. `NetWall` blocks sight;
`NetGate` does not.

### Strata

```rust
impl StratumKind {
    pub const PHYSICAL: [StratumKind; 4];                  // unchanged
    pub const ALL: [StratumKind; 5];                       // PHYSICAL + Net
}
```

A finished `City` has five strata, `Net` last. `City::empty` still creates the
four physical ones; the Cyberspace stage pushes the fifth. `Stratum::light` on
the Net encodes brightness: 200 on the grid, 255 on lanes, gates and hubs, and
`96 + security * 5 / 8` on construct walls and floors, so low-security
constructs render dim. Net tiles carry `district` (copied from the Street tile
at the same position) and `building` (on construct tiles).

### Net graph

```rust
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub anchor: (StratumKind, Pos),   // physical tile, unchanged
    pub cyber: Pos,                   // NEW: the node's tile on the Net stratum
    pub subnet: SubnetId,
    pub building: Option<BuildingId>,
}

impl Net {
    pub fn node_in_cyber(&self, p: Pos) -> Option<&Node>;   // NEW
}
```

The Net stage sets `cyber = anchor.1` when it creates a node; the Cyberspace
stage moves nodes that need moving (see 4).

### Connectors

`ConnectorKind::Jack` is used: `Connector { kind: Jack, pos: <terminal tile>,
serves: [Street, Net], public: <street tile has no building> }`. A Jack's Street
tile is the terminal itself (not walkable); it is used from an adjacent tile.

## 4. Generation: stage 12, `Cyberspace`

`Stage::Cyberspace` runs after `Net`. Label "Compiling cyberspace"; short label
"Cyber". `Stage::ALL` has 12 entries. The stage is resumable like the others;
its work queue is `Begin`, one `Construct` per building, one `Hub` per district,
`Nodes`, one `Lane` per inter-construct link, `Ports`.

1. **Begin.** Push `Stratum::new(Net, size)`. Every tile: `NetGrid`, `district`
   from the Street tile, `light` 200, `variant` with bit 0 set on the lattice
   positions `x % 4 == 0 && y % 2 == 0` and the other bits from `tile_hash`.
2. **Construct** (per building `b`). Footprint border → `NetWall`, inset → `NetFloor`
   (a footprint too small to inset is all wall), `building = b.id`, light from
   the building subnet's security. The **gate** is the border tile that is not a
   corner and is nearest to the district backbone's anchor; it becomes `NetGate`
   with light 255. The stage remembers `gate[b]`.
3. **Hub** (per district). Around the backbone anchor `c`, the border of
   `Rect(c.x-2, c.y-2, 5, 5)` becomes `NetWall` (light 255) where it is still
   `NetGrid`, and its four edge midpoints become `NetGate`. If `c` lies inside a
   building footprint, no hub: the construct is the hub.
4. **Nodes** (in id order). Target `t = anchor.1`, except ICE: `t = gate[building]`.
   If the Net tile at `t` already holds a `Feature::Node`, `t` becomes the nearest
   walkable Net tile without a node within Chebyshev distance 6, scanned in a
   fixed order. Then `node.cyber = t` and the tile's feature is `Node(kind)`. A
   node whose target is a `NetWall` stays on the wall (a sensor in the wall).
5. **Lane** (per link whose endpoints are not inside the same building). Start is
   `gate[a.building]` if `a` is in a building, else `a.cyber`; likewise the goal.
   A* over Net tiles; step cost into `NetLane | NetGate` 1, `NetGrid` 3, tiles with
   a `Node` feature 1, `NetWall | NetFloor` impassable unless it is the start or
   goal. Every `NetGrid` tile on the path becomes `NetLane` (light 255); features
   and gates are kept. If A* fails the lane is skipped.
6. **Ports.** For every Street tile with `Feature::Terminal` that holds a Net node
   (`net.node_at(Street, p)` is `Some`), push a `Jack` connector as in section 3.

## 5. Validation additions

- `Violation::CyberAnchor(NodeId)`: the Net tile at `node.cyber` must exist and
  carry `Feature::Node(node.kind)`.
- `Violation::JackUnreachable { pos }`: a Jack's Street tile must have
  `Feature::Terminal` and at least one walkable 8-neighbour on Street; its Net
  tile must be walkable. The existing `ConnectorNotWalkable` check skips Jacks.
- `StratumUnreachable { stratum: Net, .. }`: at least `REACH_TARGET` of the Net's
  walkable tiles must be reachable (4-neighbour flood) from the set of Jack
  tiles. Skipped when the city has no Jacks.
- `reachable_from_street` keeps ignoring `Net` (it already does).
- The save loader requires `strata.len() == StratumKind::ALL.len()`.

## 6. The player (`cyberpunk::walker`)

```rust
pub struct Player {
    pub pos: Pos,
    pub stratum: StratumKind,
    /// Where the body waits while jacked in: (stratum, tile). None when not jacked in.
    pub body: Option<(StratumKind, Pos)>,
    memory: Vec<Grid<Visibility>>,        // one per StratumKind::ALL
}

impl Player {
    /// Jacks in at a Jack connector on the player's tile or an 8-neighbour of it.
    /// Leaves the body behind and moves to the port's Net tile. Err if none in reach
    /// or already jacked in.
    pub fn jack_in(&mut self, city: &City) -> Result<Pos, &'static str>;
    /// Returns to the body. Err if not jacked in.
    pub fn jack_out(&mut self) -> Result<(), &'static str>;
    pub fn jacked_in(&self) -> bool;
}
```

`use_connector` ignores `Jack` connectors. FOV radius on the Net is 24; `NetWall`
blocks sight. Memory has five grids indexed by `StratumKind::index()`.

## 7. Screens and rendering (`cyberpunk`)

### Keys

- **Walk** `j`: jack in at an adjacent terminal, or jack out. Status: "jacked in
  at <building or district name>", "jacked out", "no terminal within reach".
  Hints line while jacked in: `hjkl move · j jack out · tab camera · d net · ? keys`.
- **Explore** `[` `]` now range over all five strata; `]` from Heights shows the Net.
- Help overlay (Walk) gains the `j` row. README documents the key and the stratum.

### Explore cursor (the bounded fix)

`MapScreen` keeps its own `cursor: Pos` for Explore mode. Pan keys move the
cursor (shift: 8 tiles); it is clamped to the map. The camera follows the cursor
with `Camera::follow`, as it follows the player in Walk mode, and stays clamped
so no void shows past the map edge. Leaving Walk mode puts the cursor on the
player. Every tile of the map can be put under the cursor.

### Drawing the Net stratum

Tileset (`render::tileset`):

| Tile | Glyph | Colour |
|---|---|---|
| `NetGrid` | `·` where `variant & 1 == 1`, else space | fg district accent mixed half with signal cyan, scaled 0.45; bg (3, 6, 10) |
| `NetLane` | double-line box set by mask (`═ ║ ╔ …`), `═` when unconnected | fg signal cyan scaled by `0.6 + 0.4 * pulse(tick + variant, 20)` |
| `NetWall` | heavy box set by mask; corp buildings use the double set (`wall_set`) | fg signal cyan, darkened by `light` (security) |
| `NetGate` | `▯` | fg (200, 255, 245) |
| `NetFloor` | space | bg (6, 18, 22) |
| `Feature::Node(kind)` | `tileset::node_glyph(kind)`: ◇ terminal, ◆ server, ⬡ backbone, ⌗ data vault, ◈ ice, ▪ others | fg (240, 255, 250) pulsing; emits (never darkened); bold when a Jack connector is present |

`connects`: `NetWall` with `NetWall | NetGate`; `NetLane` with `NetLane | NetGate`.
The digital overlay reuses `node_glyph` for its physical-side glyphs.

Explore and Walk draw the avatar as `@` bold signal cyan when the player's
stratum is the Net, white otherwise; when `player.body` is on the viewed
stratum, a dim grey `@` marks the body. Weather and traffic stay off the Net
(rain density 0 in the style pack already).

The digital overlay on the Net stratum draws only the focused subnet's links,
as dotted lines between `cyber` positions. `Digital::node_under` looks up
`node_in_cyber` when the stratum is the Net.

Panel: `strata_selector` lists `StratumKind::ALL` reversed (Net on top). The
inspector uses `Terrain::label`, shows the net block for the node under the
cursor on the Net, and lists the Jack like any connector ("jack  Street · Net").
Legend for the Net: grid, lane, construct wall, gate, terminal/port, server,
backbone, data vault, ice, sensor.

Generating screen: the checklist gains "Cyber"; the growing map keeps showing
the Street stratum.

## 8. Save format

`FORMAT_VERSION = 2`. `split` accepts versions 1 and 2 so the file list can still
show version 1 cities; `load_city` rejects version 1 with
`"<path> was saved by an older version; regenerate it from seed <seed>"`.

## 9. Testing

`citygen`:
- Walkability and sight for the five Net terrains; `ALL_TERRAIN` is 25, `ALL_FEATURES` 16.
- On a generated 120x60 city: the Net stratum exists at city size; every building's
  footprint border is `NetWall` or `NetGate` with exactly one gate on the border
  and its inset is `NetFloor`; grid tiles carry the Street district.
- Every node's `cyber` tile holds `Feature::Node(kind)`; every ICE node stands on
  its building's gate.
- Every Street terminal tile with a node has a Jack serving `[Street, Net]`;
  every Jack has a walkable Street neighbour and a walkable Net tile.
- At least one `NetLane` tile exists and the gate of every building reaches its
  district backbone over walkable Net tiles.
- `validate` reports `CyberAnchor` for a node moved off its feature and
  `JackUnreachable` for a Jack walled in on Street.
- The seed sweeps (determinism + validate) keep passing; medium under two seconds.

`cyberpunk`:
- Every new glyph is single width; Net cells are pure in their inputs.
- `jack_in` next to a terminal: stratum Net, `body` set, `pos` is the port's tile,
  FOV marks more than ten tiles; `jack_in` far from a terminal is `Err`;
  `jack_out` restores stratum and position; `use_connector` on a Jack tile is `None`.
- Explore `]` four times reaches the Net and the panel shows "Net"; the rendered
  Net stratum contains a lane or wall glyph.
- Walk `j` beside a terminal jacks in (screen-level test), the hints mention
  "jack out", and `j` again jacks out.
- The Explore cursor reaches `(0, 0)` and `(w-1, h-1)` while the camera origin
  never goes negative or past the map.
- Save round trip at version 2; a version 1 file is listed but refused with a
  message that contains its seed.

## 10. Performance

Lanes add one A* per inter-construct link (about one per building plus one per
district). Budget: a large city still generates in under 0.3 s in release; the
existing two-second medium test stands as the gate. If the budget is missed,
lanes fall back to routing only within the district's bounds.
