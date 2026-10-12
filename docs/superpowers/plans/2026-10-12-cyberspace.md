# Cyberspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the reserved `Net` stratum a real, walkable cyberspace generated from the city, and let the player jack in at street terminals and back out, leaving the body at the port.

**Architecture:** A twelfth generator stage (`Cyberspace`) builds a fifth tile map from the finished city and Net graph: constructs on building footprints, hubs around district backbones, A*-routed lanes for inter-construct links, nodes as tile features, and `Jack` connectors at street terminals. The `cyberpunk` crate gets a Net tileset, a `body` on the player with `jack_in`/`jack_out`, Explore access to the fifth stratum, and a free-moving Explore cursor. Save format bumps to version 2.

**Tech Stack:** Rust 2024, ratatui 0.29, crossterm 0.28, serde + postcard, existing `citygen` pathing (`astar`), existing `walker::fov`.

**Spec:** `docs/superpowers/specs/2026-10-12-cyberspace-design.md`

## Global Constraints

- Same seed and parameters produce a byte-identical city, Net stratum included (seed sweeps in `crates/citygen/tests/seeds.rs` stay green).
- Every glyph is exactly one terminal cell wide (`style_glyphs_are_single_width` in `tileset.rs` covers `ALL_TERRAIN` and `ALL_FEATURES`).
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` green after every task.
- No new crates. `citygen` keeps zero terminal dependencies.
- Medium city generates in under two seconds (existing test); large city target under 0.3 s release.
- Keys: jack in/out on **Enter** in Walk mode (`j` is "move south"). Explore `[` `]` range over all five strata.

## Review Focus

1. A private port (lobby terminal inside a building): the avatar emerges inside the construct and must be able to walk out through the gate. Test in Task 4 (`every_jack_reaches_the_open_grid`).
2. A `Jack` must not count as a district's public connector, or a district with no stairs passes validation. Test in Task 5 (`jack_is_not_a_public_connector_for_districts`).
3. A hub ring or a neighbouring construct must never seal a construct gate or stand on a port tile. Test in Task 4 (`gates_open_onto_walkable_tiles`) and the Net reachability check in Task 5.
4. On a Net tile that holds a Jack, Walk `<`/`>` must not move the avatar onto the unwalkable terminal tile on Street. Test in Task 7 (the `use_connector` asserts in `jacking_in_leaves_the_body_at_the_port`).
5. A version 1 `.city` file must still list in the load screen and refuse to load with a message naming its seed, never a panic. Test in Task 10 (`version_one_files_list_but_refuse_to_load`).

---

## File structure

| File | Responsibility |
|---|---|
| `crates/citygen/src/tile.rs` | Net terrains, `Feature::Node`, `Terrain::label`, walkability, sight |
| `crates/citygen/src/world.rs` | `StratumKind::ALL` |
| `crates/citygen/src/net.rs` | `Node.cyber`, `Net::node_in_cyber` |
| `crates/citygen/src/generate/net.rs` | sets `cyber = anchor.1` on node creation |
| `crates/citygen/src/generate/mod.rs` | `Stage::Cyberspace`, runner wiring |
| `crates/citygen/src/generate/cyberspace.rs` | **new**: the stage (grid, constructs, gates, hubs, nodes, lanes, ports) |
| `crates/citygen/src/validate.rs` | `CyberAnchor`, `JackUnreachable`, Net reachability, Jack excluded from district public check |
| `crates/cyberpunk/src/render/tileset.rs` | Net glyphs, colours, `connects`, legend, `node_glyph` |
| `crates/cyberpunk/src/render/overlay.rs` | `node_under` on the Net, links in cyberspace, shared `node_glyph` |
| `crates/cyberpunk/src/walker/mod.rs` | `body`, `jack_in`, `jack_out`, `jacked_in`, five memory grids, Net radius |
| `crates/cyberpunk/src/screens/explore.rs` | free cursor, five strata, Enter to jack, avatar and body drawing, hints |
| `crates/cyberpunk/src/screens/help.rs` | key rows |
| `crates/cyberpunk/src/screens/generating.rs` | short label for the new stage |
| `crates/cyberpunk/src/ui/panel.rs` | selector over `ALL`, `Terrain::label`, node feature label |
| `crates/cyberpunk/src/save.rs` | version 2, version 1 refusal |
| `README.md` | keys, strata table, cyberspace paragraph |

---

### Task 1: Net tiles in the model and the tileset

**Files:**
- Modify: `crates/citygen/src/tile.rs`
- Modify: `crates/citygen/src/world.rs` (after `PHYSICAL`, line 21)
- Modify: `crates/cyberpunk/src/render/tileset.rs`
- Test: `crates/citygen/src/tile.rs` (tests module), `crates/cyberpunk/src/render/tileset.rs` (tests module)

**Interfaces:**
- Consumes: `citygen::net::NodeKind` (existing, derives `Hash + Eq`), `crate::render::overlay::SIGNAL: Rgb`.
- Produces: `Terrain::{NetGrid, NetLane, NetWall, NetFloor, NetGate}`, `Terrain::label(self) -> &'static str`, `Terrain::is_net(self) -> bool`, `Feature::Node(NodeKind)`, `ALL_TERRAIN: [Terrain; 25]`, `ALL_FEATURES: [Feature; 16]`, `StratumKind::ALL: [StratumKind; 5]`, `tileset::node_glyph(kind: NodeKind, style: &StylePack) -> char`.

- [ ] **Step 1: Write the failing model test**

In `crates/citygen/src/tile.rs` tests module, add:

```rust
    #[test]
    fn net_tiles_walk_and_block_like_the_spec_says() {
        assert!(Tile::new(Terrain::NetGrid).is_walkable());
        assert!(Tile::new(Terrain::NetLane).is_walkable());
        assert!(Tile::new(Terrain::NetFloor).is_walkable());
        assert!(Tile::new(Terrain::NetGate).is_walkable());
        assert!(!Tile::new(Terrain::NetWall).is_walkable());
        assert!(Tile::new(Terrain::NetWall).blocks_sight());
        assert!(!Tile::new(Terrain::NetGate).blocks_sight());
        let mut t = Tile::new(Terrain::NetGrid);
        t.feature = Some(Feature::Node(crate::net::NodeKind::Ice));
        assert!(t.is_walkable(), "nodes never block walking in this spec");
        assert_eq!(Terrain::NetWall.label(), "construct wall");
        assert_eq!(Terrain::Road.label(), "road");
        assert!(Terrain::NetLane.is_net() && !Terrain::Road.is_net());
        assert_eq!(ALL_TERRAIN.len(), 25);
        assert_eq!(ALL_FEATURES.len(), 16);
        assert_eq!(crate::world::StratumKind::ALL.len(), 5);
        assert_eq!(crate::world::StratumKind::ALL[4], crate::world::StratumKind::Net);
    }
```

Also change the existing `walkability_rules` asserts `assert_eq!(ALL_TERRAIN.len(), 20);` → `25` and `ALL_FEATURES.len(), 15` → `16`.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p citygen net_tiles_walk -- --nocapture`
Expected: compile error, `no variant named NetGrid`.

- [ ] **Step 3: Implement the model**

In `crates/citygen/src/tile.rs`:

```rust
use crate::net::NodeKind;
```

Extend the enum and constant:

```rust
    Grate,
    NetGrid,
    NetLane,
    NetWall,
    NetFloor,
    NetGate,
}

pub const ALL_TERRAIN: [Terrain; 25] = [
    /* existing 20 entries unchanged */
    Terrain::NetGrid,
    Terrain::NetLane,
    Terrain::NetWall,
    Terrain::NetFloor,
    Terrain::NetGate,
];

impl Terrain {
    /// Human label for the inspector and legend.
    pub fn label(self) -> &'static str {
        use Terrain::*;
        match self {
            Void => "void",
            Water => "water",
            Sewage => "sewage",
            Road => "road",
            Sidewalk => "sidewalk",
            Alley => "alley",
            Plaza => "plaza",
            Floor => "floor",
            Wall => "wall",
            Door => "door",
            Window => "window",
            Rooftop => "rooftop",
            Walkway => "walkway",
            Bridge => "bridge",
            Tunnel => "tunnel",
            Rail => "rail",
            Stair => "stair",
            Lift => "lift",
            Ladder => "ladder",
            Grate => "grate",
            NetGrid => "grid",
            NetLane => "lane",
            NetWall => "construct wall",
            NetFloor => "construct floor",
            NetGate => "gate",
        }
    }

    /// Cyberspace-only terrain.
    pub fn is_net(self) -> bool {
        use Terrain::*;
        matches!(self, NetGrid | NetLane | NetWall | NetFloor | NetGate)
    }
}
```

Feature:

```rust
    Cable,
    /// A Net node standing on this cyberspace tile.
    Node(NodeKind),
}

pub const ALL_FEATURES: [Feature; 16] = [
    /* existing 15 */
    Feature::Node(NodeKind::Server),
];
```

Walkability: add `| NetGrid | NetLane | NetFloor | NetGate` to the `terrain_ok` list. Sight: `matches!(self.terrain, Terrain::Wall | Terrain::Door | Terrain::NetWall)`.

In `crates/citygen/src/world.rs` after `PHYSICAL`:

```rust
    /// Every stratum a finished city has, bottom first, Net last.
    pub const ALL: [StratumKind; 5] = [
        StratumKind::Undercity,
        StratumKind::Street,
        StratumKind::Skyways,
        StratumKind::Heights,
        StratumKind::Net,
    ];
```

- [ ] **Step 4: Run the citygen tests**

Run: `cargo test -p citygen`
Expected: PASS (the `cyberpunk` crate does not compile yet; that is the next step). If any `match` on `Terrain` in `citygen` is non-exhaustive, add a `_ => {}`-free explicit arm that treats the Net terrains like `Void`.

- [ ] **Step 5: Write the failing tileset test**

In `crates/cyberpunk/src/render/tileset.rs` tests module:

```rust
    #[test]
    fn net_tiles_render_in_the_signal_palette() {
        let s = StylePack::load().unwrap();
        let ctx = |stratum| RenderCtx {
            style: &s,
            theme: Some(Theme::CorpCore),
            stratum,
            light: 255,
            tick: 0,
            vis: Visibility::Visible,
            connector: None,
            kind: None,
            wall_mask: MASK_E | MASK_W,
        };
        let wall = cell(&Tile::new(Terrain::NetWall), &ctx(StratumKind::Net));
        assert_eq!(wall.ch, '━');
        assert!(wall.fg.2 > wall.fg.0, "wall is cyan-ish: {:?}", wall.fg);
        let lane = cell(&Tile::new(Terrain::NetLane), &ctx(StratumKind::Net));
        assert_eq!(lane.ch, '═');
        let mut grid = Tile::new(Terrain::NetGrid);
        grid.variant = 1;
        assert_eq!(cell(&grid, &ctx(StratumKind::Net)).ch, '·');
        grid.variant = 2;
        assert_eq!(cell(&grid, &ctx(StratumKind::Net)).ch, ' ');
        assert_eq!(cell(&Tile::new(Terrain::NetGate), &ctx(StratumKind::Net)).ch, '▯');
        let mut node = Tile::new(Terrain::NetGrid);
        node.feature = Some(Feature::Node(NodeKind::Server));
        assert_eq!(cell(&node, &ctx(StratumKind::Net)).ch, node_glyph(NodeKind::Server, &s));
        assert_eq!(node_glyph(NodeKind::Server, &s), '◆');
        assert_eq!(node_glyph(NodeKind::Ice, &s), '◈');
        assert!(connects(Terrain::NetWall, Terrain::NetGate));
        assert!(connects(Terrain::NetLane, Terrain::NetGate));
        assert!(!connects(Terrain::NetLane, Terrain::NetWall));
        let legend = legend_entries(StratumKind::Net);
        assert!(legend.iter().any(|(_, l)| *l == "construct wall"));
        assert!(!legend.iter().any(|(_, l)| *l == "stairs"));
    }
```

Add `NodeKind` to the test module's `use citygen::{...}`.

- [ ] **Step 6: Run it to verify it fails**

Run: `cargo test -p cyberpunk net_tiles_render`
Expected: compile error (non-exhaustive `match` on `Terrain` in `glyph_for`/`terrain_colors`, missing `node_glyph`).

- [ ] **Step 7: Implement the tileset**

In `crates/cyberpunk/src/render/tileset.rs`:

```rust
use crate::render::overlay::SIGNAL;
use citygen::{BuildingKind, ConnectorKind, Feature, NodeKind, StratumKind, Terrain, Tile};

/// Glyph for a Net node kind from the style's `net` set.
pub fn node_glyph(kind: NodeKind, style: &StylePack) -> char {
    let g = &style.glyphs.net;
    let i = match kind {
        NodeKind::Terminal => 0,
        NodeKind::Server => 1,
        NodeKind::Backbone => 2,
        NodeKind::DataVault => 3,
        NodeKind::Ice => 5,
        _ => 4,
    };
    g[i % g.len()]
}
```

`connects`:

```rust
pub fn connects(a: Terrain, b: Terrain) -> bool {
    use Terrain::*;
    let wallish = |t| matches!(t, Wall | Door | Window);
    let spanish = |t| matches!(t, Walkway | Bridge);
    let netwall = |t| matches!(t, NetWall | NetGate);
    let lane = |t| matches!(t, NetLane | NetGate);
    (wallish(a) && wallish(b))
        || (spanish(a) && (spanish(b) || matches!(b, Door | Floor | Stair | Lift | Ladder)))
        || (netwall(a) && netwall(b))
        || (lane(a) && lane(b))
}
```

`glyph_for` feature arm: `Feature::Node(k) => node_glyph(k, style),`. Terrain arms:

```rust
        Terrain::NetGrid => {
            if variant & 1 == 1 {
                '·'
            } else {
                ' '
            }
        }
        Terrain::NetLane => DOUBLE[(mask & 15) as usize],
        Terrain::NetWall => {
            let set = if kind.is_some_and(|k| k.is_corp()) {
                &DOUBLE
            } else {
                &HEAVY
            };
            set[(mask & 15) as usize]
        }
        Terrain::NetFloor => ' ',
        Terrain::NetGate => '▯',
```

(`DOUBLE[0]` is `═`, so an unconnected lane already draws as `═`.)

`terrain_colors` gains `tile: &Tile, tick: u64` parameters (call site in `cell`: `terrain_colors(tile.terrain, tile, ctx, tick)`) and arms:

```rust
        Terrain::NetGrid => (ts.accent.mix(SIGNAL, 0.5).scale(0.45), Rgb(3, 6, 10)),
        Terrain::NetLane => (
            SIGNAL.scale(0.6 + 0.4 * pulse(tick + tile.variant as u64, 20)),
            Rgb(3, 6, 10),
        ),
        Terrain::NetWall => (SIGNAL, Rgb(3, 6, 10)),
        Terrain::NetFloor => (Rgb(10, 40, 44), Rgb(6, 18, 22)),
        Terrain::NetGate => (Rgb(200, 255, 245), Rgb(6, 18, 22)),
```

`feature_color`: `Feature::Node(_) => Rgb(240, 255, 250).scale(0.7 + 0.3 * pulse(tick + tile.variant as u64, 30)),`.

In `cell`: add `| Feature::Node(_)` to the `emits` list, and make the bold rule:

```rust
    let jack = ctx
        .connector
        .is_some_and(|c| c.kind == ConnectorKind::Jack);
    let bold = tile.is_connector()
        || matches!(tile.feature, Some(Feature::Sign(_) | Feature::Neon(_)))
        || (matches!(tile.feature, Some(Feature::Node(_))) && jack);
```

`legend_entries`: replace the `StratumKind::Net` arm with

```rust
        StratumKind::Net => vec![
            ('·', "grid"),
            ('═', "lane"),
            ('┃', "construct wall"),
            ('▯', "gate"),
            ('◇', "terminal / port"),
            ('◆', "server"),
            ('⬡', "backbone"),
            ('⌗', "data vault"),
            ('◈', "ice"),
            ('▪', "sensor"),
        ],
```

and guard the common rows: `if k != StratumKind::Net { out.extend(common); }`.

- [ ] **Step 8: Run the whole suite**

Run: `cargo test --workspace`
Expected: PASS. Any other non-exhaustive `match` on `Terrain` or `Feature` the compiler reports (weather, minimap, dressing) gets an explicit arm that treats Net terrain as not-outdoor, not-building, not-water.

- [ ] **Step 9: Commit**

```bash
git add crates/citygen/src/tile.rs crates/citygen/src/world.rs crates/cyberpunk/src/render/tileset.rs
git commit -m "feat(net): cyberspace terrains, node features and their tileset"
```

---

### Task 2: Nodes know their cyberspace tile

**Files:**
- Modify: `crates/citygen/src/net.rs`
- Modify: `crates/citygen/src/generate/net.rs` (`add_node`, line ~44)
- Test: `crates/citygen/src/net.rs` (new tests module)

**Interfaces:**
- Produces: `Node.cyber: Pos`, `Net::node_in_cyber(&self, p: Pos) -> Option<&Node>`.

- [ ] **Step 1: Write the failing test**

Append to `crates/citygen/src/net.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nodes_are_found_by_cyber_tile() {
        let mut net = Net::default();
        net.subnets.push(Subnet {
            id: SubnetId(0),
            owner: SubnetOwner::City,
            security: 10,
        });
        net.nodes.push(Node {
            id: NodeId(0),
            kind: NodeKind::Server,
            anchor: (StratumKind::Street, Pos::new(5, 5)),
            cyber: Pos::new(7, 5),
            subnet: SubnetId(0),
            building: None,
        });
        assert_eq!(net.node_in_cyber(Pos::new(7, 5)).map(|n| n.id), Some(NodeId(0)));
        assert_eq!(net.node_in_cyber(Pos::new(5, 5)), None);
        assert!(net.node_at(StratumKind::Street, Pos::new(5, 5)).is_some());
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p citygen nodes_are_found_by_cyber_tile`
Expected: compile error, `struct Node has no field named cyber`.

- [ ] **Step 3: Implement**

`crates/citygen/src/net.rs`:

```rust
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub anchor: (StratumKind, Pos),
    /// The node's tile on the Net stratum.
    pub cyber: Pos,
    pub subnet: SubnetId,
    pub building: Option<BuildingId>,
}

impl Net {
    pub fn node_in_cyber(&self, p: Pos) -> Option<&Node> {
        self.nodes.iter().find(|n| n.cyber == p)
    }
}
```

`crates/citygen/src/generate/net.rs` `add_node`: `cyber: anchor.1,` in the `Node { .. }` literal.

- [ ] **Step 4: Run the suite**

Run: `cargo test --workspace`
Expected: PASS. Any test that builds a `Node` literal (search `Node {` under `crates/`) gets `cyber: <its anchor pos>`.

- [ ] **Step 5: Commit**

```bash
git add crates/citygen/src/net.rs crates/citygen/src/generate/net.rs
git commit -m "feat(net): nodes carry their cyberspace tile"
```

---

### Task 3: Stage 12 — grid, constructs, gates, hubs

**Files:**
- Create: `crates/citygen/src/generate/cyberspace.rs`
- Modify: `crates/citygen/src/generate/mod.rs` (module list, `Stage`, `ALL`, `label`, `runner_for`, doc comment)
- Modify: `crates/cyberpunk/src/screens/generating.rs:32-45` (`short`)
- Modify: `crates/cyberpunk/src/save.rs:96` (`strata_ok` uses `StratumKind::ALL`)
- Test: `crates/citygen/src/generate/cyberspace.rs` (tests module)

**Interfaces:**
- Consumes: `GenCtx`, `StageRunner`, `StepOut` (generate/mod.rs), `Stratum::new`, `tile_hash(seed, k, p) -> u8`, `Rect::{border, inset, iter, right, bottom, center}`, `Pos::dist2`, `SubnetOwner`, `NodeKind`.
- Produces: `Stage::Cyberspace`; `CyberspaceStage` with `gates: Vec<Pos>` (per building index) and `backbones: Vec<Option<Pos>>` (per district index) used by Task 4; helper `set(ctx, p, terrain, building, light)`.

- [ ] **Step 1: Write the failing test**

Create `crates/citygen/src/generate/cyberspace.rs` with the module doc and a tests module only:

```rust
//! Stage 12: cyberspace — the Net stratum as a grid with constructs, hubs, lanes and ports.

#[cfg(test)]
mod tests {
    use crate::generate::Generator;
    use crate::geom::Pos;
    use crate::params::{GenParams, SizePreset};
    use crate::tile::Terrain;
    use crate::world::StratumKind;

    pub(crate) fn city() -> crate::world::City {
        Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn net_stratum_is_a_grid_with_constructs_on_every_footprint() {
        let c = city();
        let net = c.stratum(StratumKind::Net).expect("fifth stratum");
        assert_eq!(c.strata.len(), 5);
        assert_eq!(net.tiles.size(), c.size);
        // Grid tiles mirror the street's districts and carry the lattice bit.
        let lattice = Pos::new(4, 2);
        let t = net.tiles[lattice];
        assert_eq!(t.terrain, Terrain::NetGrid);
        assert_eq!(t.variant & 1, 1);
        assert_eq!(net.tiles[Pos::new(5, 2)].variant & 1, 0);
        assert_eq!(t.district, c.tile(StratumKind::Street, lattice).unwrap().district);
        for b in &c.buildings {
            let fp = b.footprint;
            let gates = fp
                .border()
                .filter(|p| net.tiles[*p].terrain == Terrain::NetGate)
                .count();
            assert_eq!(gates, 1, "building {:?} has {gates} gates", b.id);
            for p in fp.border() {
                let t = net.tiles[p];
                assert!(
                    matches!(t.terrain, Terrain::NetWall | Terrain::NetGate),
                    "{:?} border {p:?} is {:?}",
                    b.id,
                    t.terrain
                );
                assert_eq!(t.building, Some(b.id));
            }
            for p in fp.inset(1).iter() {
                assert_eq!(net.tiles[p].terrain, Terrain::NetFloor);
            }
            let light = net.light[fp.center()];
            assert!(light >= 96, "construct light {light}");
        }
        assert!(net.tiles.count(|t| t.terrain == Terrain::NetGrid) > c.size.area() / 4);
    }

    #[test]
    fn hubs_ring_every_backbone_that_stands_on_the_grid() {
        let c = city();
        let net = c.stratum(StratumKind::Net).unwrap();
        let mut hubs = 0;
        for n in c.net.nodes.iter().filter(|n| n.kind == crate::net::NodeKind::Backbone) {
            let a = n.anchor.1;
            if net.tiles[a].building.is_some() {
                continue;
            }
            hubs += 1;
            let ring = crate::geom::Rect::new(a.x - 2, a.y - 2, 5, 5);
            let walls = ring
                .border()
                .filter(|p| net.tiles.get(*p).is_some_and(|t| t.terrain == Terrain::NetWall))
                .count();
            let gates = ring
                .border()
                .filter(|p| net.tiles.get(*p).is_some_and(|t| t.terrain == Terrain::NetGate))
                .count();
            assert!(walls >= 8, "hub at {a:?} has only {walls} wall tiles");
            assert!(gates >= 2, "hub at {a:?} has only {gates} gates");
        }
        assert!(hubs >= 1);
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p citygen cyberspace::`
Expected: compile error, `unresolved module cyberspace` (add `pub mod cyberspace;` to `generate/mod.rs` first, then the test fails with `expected fifth stratum`).

- [ ] **Step 3: Wire the stage**

`crates/citygen/src/generate/mod.rs`:

- Doc comment: `//! The resumable generator: twelve chunked stages over a growing \`City\`.`
- `pub mod cyberspace;` in the module list (alphabetical: after `connectivity`).
- `Stage` gains `Cyberspace,` after `Net`; `ALL: [Stage; 12]` adds `Stage::Cyberspace`; `label`: `Stage::Cyberspace => "Compiling cyberspace",`; `runner_for`: `Stage::Cyberspace => Box::new(cyberspace::CyberspaceStage::default()),`.

`crates/cyberpunk/src/screens/generating.rs` `short`: `Stage::Cyberspace => "Cyber",`.

`crates/cyberpunk/src/save.rs`: `let strata_ok = city.strata.len() == citygen::StratumKind::ALL.len() && ...`.

- [ ] **Step 4: Implement grid, constructs, gates, hubs**

Above the tests module in `cyberspace.rs`:

```rust
use super::{GenCtx, StageRunner, StepOut};
use crate::geom::{Pos, Rect};
use crate::ids::{BuildingId, DistrictId};
use crate::net::{NodeKind, SubnetOwner};
use crate::params::GenError;
use crate::rng::tile_hash;
use crate::tile::{Feature, Terrain, Tile};
use crate::world::{Stratum, StratumKind};
use std::collections::VecDeque;

const K: StratumKind = StratumKind::Net;
const GRID_LIGHT: u8 = 200;
const HUB_RADIUS: i32 = 2;

enum Work {
    Begin,
    Construct(usize),
    Gate(usize),
    Hub(usize),
}

#[derive(Default)]
pub(crate) struct CyberspaceStage {
    queue: VecDeque<Work>,
    total: usize,
    /// Gate tile per building index.
    pub(crate) gates: Vec<Pos>,
    /// Backbone anchor per district index.
    pub(crate) backbones: Vec<Option<Pos>>,
}

/// Sets terrain, owner and light of one Net tile; out-of-bounds is a no-op.
pub(crate) fn set(ctx: &mut GenCtx, p: Pos, t: Terrain, b: Option<BuildingId>, light: u8) {
    if let Some(tile) = ctx.city.tile_mut(K, p) {
        tile.terrain = t;
        tile.building = b;
    }
    if let Some(st) = ctx.city.stratum_mut(K) {
        st.light.set(p, light);
    }
}

fn is_corner(r: Rect, p: Pos) -> bool {
    (p.x == r.x || p.x == r.right() - 1) && (p.y == r.y || p.y == r.bottom() - 1)
}

/// The tile just outside a footprint border tile, away from the centre.
fn outward(r: Rect, p: Pos) -> Pos {
    if p.y == r.y {
        p.offset(0, -1)
    } else if p.y == r.bottom() - 1 {
        p.offset(0, 1)
    } else if p.x == r.x {
        p.offset(-1, 0)
    } else {
        p.offset(1, 0)
    }
}

fn backbone_anchor(ctx: &GenCtx, d: DistrictId) -> Option<Pos> {
    ctx.city
        .net
        .nodes
        .iter()
        .find(|n| {
            n.kind == NodeKind::Backbone
                && ctx.city.net.subnet(n.subnet).owner == SubnetOwner::District(d)
        })
        .map(|n| n.anchor.1)
}

impl CyberspaceStage {
    fn begin_net(ctx: &mut GenCtx) {
        let size = ctx.size();
        ctx.city.strata.retain(|s| s.kind != K);
        let mut st = Stratum::new(K, size);
        for y in 0..size.h as i32 {
            for x in 0..size.w as i32 {
                let p = Pos::new(x, y);
                let district = ctx.city.tile(StratumKind::Street, p).and_then(|t| t.district);
                let lattice = u8::from(x % 4 == 0 && y % 2 == 0);
                let variant = (tile_hash(ctx.seed, K, p) & 0xFE) | lattice;
                st.tiles.set(
                    p,
                    Tile {
                        terrain: Terrain::NetGrid,
                        feature: None,
                        district,
                        building: None,
                        variant,
                    },
                );
                st.light.set(p, GRID_LIGHT);
            }
        }
        ctx.city.strata.push(st);
    }

    fn construct(&mut self, ctx: &mut GenCtx, i: usize) {
        let b = ctx.city.buildings[i].clone();
        let security = ctx
            .city
            .net
            .subnets
            .iter()
            .find(|s| s.owner == SubnetOwner::Building(b.id))
            .map(|s| s.security)
            .unwrap_or(0);
        let light = 96u8.saturating_add((security as u32 * 5 / 8) as u8);
        for p in b.footprint.iter() {
            set(ctx, p, Terrain::NetWall, Some(b.id), light);
        }
        for p in b.footprint.inset(1).iter() {
            set(ctx, p, Terrain::NetFloor, Some(b.id), light);
        }
    }

    /// Picks the gate once every construct stands, so it never opens onto another wall.
    fn gate(&mut self, ctx: &mut GenCtx, i: usize) {
        let b = ctx.city.buildings[i].clone();
        let fp = b.footprint;
        let target = self.backbones[b.district.index()].unwrap_or(fp.center());
        let open = |ctx: &GenCtx, p: Pos| {
            ctx.city
                .tile(K, outward(fp, p))
                .is_some_and(|t| t.terrain == Terrain::NetGrid)
        };
        let candidates: Vec<Pos> = fp.border().filter(|p| !is_corner(fp, *p)).collect();
        let gate = candidates
            .iter()
            .copied()
            .filter(|p| open(ctx, *p))
            .min_by_key(|p| (p.dist2(target), p.y, p.x))
            .or_else(|| {
                candidates
                    .iter()
                    .copied()
                    .min_by_key(|p| (p.dist2(target), p.y, p.x))
            })
            .unwrap_or(fp.center());
        set(ctx, gate, Terrain::NetGate, Some(b.id), 255);
        self.gates[i] = gate;
    }

    fn hub(&mut self, ctx: &mut GenCtx, d: usize) {
        let Some(c) = self.backbones[d] else { return };
        if ctx.city.tile(K, c).is_none_or(|t| t.building.is_some()) {
            return;
        }
        let r = Rect::new(
            c.x - HUB_RADIUS,
            c.y - HUB_RADIUS,
            2 * HUB_RADIUS + 1,
            2 * HUB_RADIUS + 1,
        );
        let keep_open = |ctx: &GenCtx, p: Pos| {
            // Never wall a port, and never seal a construct gate.
            ctx.city
                .tile(StratumKind::Street, p)
                .is_some_and(|t| t.feature == Some(Feature::Terminal))
                || p
                    .neighbors4()
                    .any(|q| ctx.terrain(K, q) == Terrain::NetGate)
        };
        for p in r.border() {
            if ctx.terrain(K, p) == Terrain::NetGrid && !keep_open(ctx, p) {
                set(ctx, p, Terrain::NetWall, None, 255);
            }
        }
        for p in [
            Pos::new(c.x, r.y),
            Pos::new(r.right() - 1, c.y),
            Pos::new(c.x, r.bottom() - 1),
            Pos::new(r.x, c.y),
        ] {
            if ctx.terrain(K, p) == Terrain::NetWall
                && ctx.city.tile(K, p).is_some_and(|t| t.building.is_none())
            {
                set(ctx, p, Terrain::NetGate, None, 255);
            }
        }
    }
}

impl StageRunner for CyberspaceStage {
    fn begin(&mut self, ctx: &mut GenCtx) -> Result<(), GenError> {
        let nb = ctx.city.buildings.len();
        let nd = ctx.city.districts.len();
        self.gates = vec![Pos::new(0, 0); nb];
        self.backbones = (0..nd)
            .map(|d| backbone_anchor(ctx, DistrictId(d as u32)))
            .collect();
        self.queue.clear();
        self.queue.push_back(Work::Begin);
        self.queue.extend((0..nb).map(Work::Construct));
        self.queue.extend((0..nb).map(Work::Gate));
        self.queue.extend((0..nd).map(Work::Hub));
        self.total = self.queue.len().max(1);
        Ok(())
    }

    fn step(&mut self, ctx: &mut GenCtx) -> Result<StepOut, GenError> {
        let Some(work) = self.queue.pop_front() else {
            return Ok(StepOut::Finished);
        };
        match work {
            Work::Begin => Self::begin_net(ctx),
            Work::Construct(i) => self.construct(ctx, i),
            Work::Gate(i) => self.gate(ctx, i),
            Work::Hub(d) => self.hub(ctx, d),
        }
        let done = self.total - self.queue.len();
        Ok(StepOut::Continue(done as f32 / self.total as f32))
    }
}
```

Check `Rect::inset(1)` on a footprint narrower than 3: read `geom.rs:120`; if `inset` can produce a negative width whose `iter()` misbehaves, guard with `if fp.w > 2 && fp.h > 2`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p citygen cyberspace::`
Expected: PASS both tests.

- [ ] **Step 6: Run the whole suite**

Run: `cargo test --workspace`
Expected: PASS. The generating-screen test iterates `Stage::ALL` and the save round trip now expects five strata.

- [ ] **Step 7: Commit**

```bash
git add crates/citygen/src/generate/cyberspace.rs crates/citygen/src/generate/mod.rs crates/cyberpunk/src/screens/generating.rs crates/cyberpunk/src/save.rs
git commit -m "feat(citygen): cyberspace stage with grid, constructs, gates and hubs"
```

---

### Task 4: Stage 12 — nodes, lanes, ports

**Files:**
- Modify: `crates/citygen/src/generate/cyberspace.rs`
- Test: same file

**Interfaces:**
- Consumes: `pathing::astar(size, start, goal, cost: impl Fn(Pos, Pos) -> Option<u32>) -> Option<Vec<Pos>>` (4-neighbour steps; confirm in `pathing.rs:58`), `Connector`, `ConnectorKind::Jack`, `Net::node_at`.
- Produces: `Feature::Node(kind)` on Net tiles with `node.cyber` matching; `Terrain::NetLane` tiles; `Jack` connectors `serves: [Street, Net]`.

- [ ] **Step 1: Write the failing tests**

Add to the tests module:

```rust
    #[test]
    fn every_node_stands_on_its_feature_and_ice_guards_the_gate() {
        let c = city();
        let net = c.stratum(StratumKind::Net).unwrap();
        for n in &c.net.nodes {
            assert_eq!(
                net.tiles[n.cyber].feature,
                Some(crate::tile::Feature::Node(n.kind)),
                "node {:?} ({:?}) at {:?}",
                n.id,
                n.kind,
                n.cyber
            );
        }
        let mut ice = 0;
        for n in c.net.nodes.iter().filter(|n| n.kind == crate::net::NodeKind::Ice) {
            let Some(b) = n.building else { continue };
            ice += 1;
            let fp = c.building(b).footprint;
            let gate = fp
                .border()
                .find(|p| net.tiles[*p].terrain == Terrain::NetGate)
                .unwrap();
            assert!(
                n.cyber.chebyshev(gate) <= 1,
                "ICE {:?} at {:?} is not at gate {gate:?}",
                n.id,
                n.cyber
            );
        }
        assert!(ice >= 1, "fixture has no ICE");
    }

    #[test]
    fn lanes_link_every_construct_gate_to_its_backbone() {
        let c = city();
        let net = c.stratum(StratumKind::Net).unwrap();
        assert!(net.tiles.count(|t| t.terrain == Terrain::NetLane) > 20);
        for b in &c.buildings {
            let gate = b
                .footprint
                .border()
                .find(|p| net.tiles[*p].terrain == Terrain::NetGate)
                .unwrap();
            let backbone = c
                .net
                .nodes
                .iter()
                .find(|n| {
                    n.kind == crate::net::NodeKind::Backbone
                        && c.net.subnet(n.subnet).owner
                            == crate::net::SubnetOwner::District(b.district)
                })
                .unwrap();
            let reach = crate::pathing::flood_fill(c.size, gate, |p| {
                net.tiles.get(p).is_some_and(|t| t.is_walkable())
            });
            assert!(
                reach.contains(&backbone.cyber),
                "gate {gate:?} of {:?} cannot reach backbone {:?}",
                b.id,
                backbone.cyber
            );
        }
    }

    #[test]
    fn street_terminals_with_nodes_become_jacks() {
        let c = city();
        let street = StratumKind::Street;
        let jacks: Vec<_> = c
            .connectors
            .iter()
            .filter(|k| k.kind == crate::world::ConnectorKind::Jack)
            .collect();
        assert!(jacks.len() >= 3, "only {} jacks", jacks.len());
        for j in &jacks {
            assert_eq!(j.serves, vec![street, StratumKind::Net]);
            let t = c.tile(street, j.pos).unwrap();
            assert_eq!(t.feature, Some(crate::tile::Feature::Terminal));
            assert_eq!(j.public, t.building.is_none());
            assert!(c.walkable(StratumKind::Net, j.pos), "port {:?} not walkable", j.pos);
            assert!(j.pos.neighbors8().any(|n| c.walkable(street, n)));
        }
        for (p, t) in c.stratum(street).unwrap().tiles.iter() {
            if t.feature == Some(crate::tile::Feature::Terminal)
                && c.net.node_at(street, p).is_some()
            {
                assert!(jacks.iter().any(|j| j.pos == p), "terminal {p:?} has no jack");
            }
        }
        assert!(jacks.iter().any(|j| !j.public), "fixture has no private port");
    }

    #[test]
    fn every_jack_reaches_the_open_grid() {
        let c = city();
        let net = c.stratum(StratumKind::Net).unwrap();
        for j in c
            .connectors
            .iter()
            .filter(|k| k.kind == crate::world::ConnectorKind::Jack)
        {
            let reach = crate::pathing::flood_fill(c.size, j.pos, |p| {
                net.tiles.get(p).is_some_and(|t| t.is_walkable())
            });
            assert!(
                reach.iter().any(|p| net.tiles[*p].terrain == Terrain::NetGrid
                    && net.tiles[*p].building.is_none()),
                "port {:?} is sealed in",
                j.pos
            );
        }
    }

    #[test]
    fn gates_open_onto_walkable_tiles() {
        for seed in [6u64, 11, 29] {
            let c = Generator::run(GenParams {
                seed,
                size: SizePreset::Custom { w: 80, h: 40 },
                districts: 4,
                ..Default::default()
            })
            .unwrap();
            let net = c.stratum(StratumKind::Net).unwrap();
            for b in &c.buildings {
                let fp = b.footprint;
                let gate = fp
                    .border()
                    .find(|p| net.tiles[*p].terrain == Terrain::NetGate)
                    .unwrap();
                assert!(
                    gate.neighbors4().any(|q| {
                        !fp.contains(q) && net.tiles.get(q).is_some_and(|t| t.is_walkable())
                    }),
                    "seed {seed}: gate {gate:?} of {:?} is sealed",
                    b.id
                );
            }
        }
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p citygen cyberspace::`
Expected: `every_node_stands_on_its_feature...` FAILS (`feature None`), `lanes_link...` FAILS (`count > 20`), `street_terminals...` FAILS (`only 0 jacks`), `every_jack_reaches...` passes vacuously (no jacks yet; it bites once Ports exist), `gates_open_onto_walkable_tiles` PASSES already (gate choice was Task 3).

- [ ] **Step 3: Implement nodes, lanes and ports**

Extend `Work`:

```rust
enum Work {
    Begin,
    Construct(usize),
    Gate(usize),
    Hub(usize),
    Nodes,
    Lane(usize),
    Ports,
}
```

Add constants and imports:

```rust
use crate::net::Node;
use crate::pathing::astar;
use crate::world::{Connector, ConnectorKind};

const NODE_SEARCH: i32 = 6;
const LANE_LIGHT: u8 = 255;
```

Methods on `CyberspaceStage`:

```rust
    /// Nearest walkable Net tile without a feature, scanning square rings in a fixed order.
    fn nearest_free(ctx: &GenCtx, c: Pos) -> Option<Pos> {
        for r in 1..=NODE_SEARCH {
            let ring = Rect::new(c.x - r, c.y - r, 2 * r + 1, 2 * r + 1);
            let hit = ring.border().find(|p| {
                ctx.city
                    .tile(K, *p)
                    .is_some_and(|t| t.is_walkable() && t.feature.is_none())
            });
            if hit.is_some() {
                return hit;
            }
        }
        None
    }

    /// ICE first so it always takes the gate; then everything else in id order.
    fn nodes(&mut self, ctx: &mut GenCtx) {
        let mut order: Vec<usize> = (0..ctx.city.net.nodes.len()).collect();
        order.sort_by_key(|i| (ctx.city.net.nodes[*i].kind != NodeKind::Ice, *i));
        for i in order {
            let node = ctx.city.net.nodes[i].clone();
            let mut t = node.anchor.1;
            if node.kind == NodeKind::Ice
                && let Some(b) = node.building
            {
                t = self.gates[b.index()];
            }
            let taken = ctx
                .city
                .tile(K, t)
                .is_none_or(|x| matches!(x.feature, Some(Feature::Node(_))));
            if taken {
                t = Self::nearest_free(ctx, t).unwrap_or(t);
            }
            ctx.set_feature(K, t, Some(Feature::Node(node.kind)));
            ctx.city.net.nodes[i].cyber = t;
        }
    }

    fn lane_end(&self, n: &Node) -> Pos {
        match n.building {
            Some(b) => self.gates[b.index()],
            None => n.cyber,
        }
    }

    fn lane(&mut self, ctx: &mut GenCtx, li: usize) {
        let link = ctx.city.net.links[li];
        let a = ctx.city.net.node(link.0).clone();
        let b = ctx.city.net.node(link.1).clone();
        if a.building.is_some() && a.building == b.building {
            return;
        }
        let (start, goal) = (self.lane_end(&a), self.lane_end(&b));
        if start == goal {
            return;
        }
        let size = ctx.size();
        let path = {
            let city = &ctx.city;
            astar(size, start, goal, |_, q| {
                let t = city.tile(K, q)?;
                if q == start || q == goal {
                    return Some(1);
                }
                match t.terrain {
                    Terrain::NetLane | Terrain::NetGate => Some(1),
                    Terrain::NetGrid => Some(if matches!(t.feature, Some(Feature::Node(_))) {
                        1
                    } else {
                        3
                    }),
                    _ => None,
                }
            })
        };
        let Some(path) = path else { return };
        for p in path {
            if ctx.terrain(K, p) == Terrain::NetGrid {
                ctx.set_terrain(K, p, Terrain::NetLane);
                if let Some(st) = ctx.city.stratum_mut(K) {
                    st.light.set(p, LANE_LIGHT);
                }
            }
        }
    }

    fn ports(ctx: &mut GenCtx) {
        let street = StratumKind::Street;
        let mut jacks = Vec::new();
        if let Some(st) = ctx.city.stratum(street) {
            for (p, t) in st.tiles.iter() {
                if t.feature == Some(Feature::Terminal) && ctx.city.net.node_at(street, p).is_some()
                {
                    jacks.push(Connector {
                        kind: ConnectorKind::Jack,
                        pos: p,
                        serves: vec![street, K],
                        public: t.building.is_none(),
                    });
                }
            }
        }
        ctx.city.connectors.retain(|c| c.kind != ConnectorKind::Jack);
        ctx.city.connectors.extend(jacks);
    }
```

`begin` queues, after the hubs: `self.queue.push_back(Work::Nodes); self.queue.extend((0..ctx.city.net.links.len()).map(Work::Lane)); self.queue.push_back(Work::Ports);`. `step` dispatches `Work::Nodes => self.nodes(ctx)`, `Work::Lane(i) => self.lane(ctx, i)`, `Work::Ports => Self::ports(ctx)`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p citygen cyberspace::`
Expected: PASS all. If `lanes_link_every_construct_gate_to_its_backbone` fails because a backbone hub is sealed, the `keep_open` rule in `hub` is wrong; fix the rule, not the test.

- [ ] **Step 5: Run the seed sweeps and timing**

Run: `cargo test -p citygen --test seeds` then `cargo build --release -p cyberpunk && time ./target/release/cyberpunk gen --seed 1 --size large --out /tmp/l.city`
Expected: sweeps PASS (determinism + `validate`); large generation well under 0.3 s. If over budget, restrict `astar` in `lane` to the bounding box of both districts grown by 8 (pass a smaller `size` by translating coordinates is not available; instead return `None` from the cost closure for tiles outside that box).

- [ ] **Step 6: Commit**

```bash
git add crates/citygen/src/generate/cyberspace.rs
git commit -m "feat(citygen): cyberspace nodes, lanes and jack ports"
```

---

### Task 5: Validation of cyberspace

**Files:**
- Modify: `crates/citygen/src/validate.rs`
- Test: same file

**Interfaces:**
- Produces: `Violation::CyberAnchor(NodeId)`, `Violation::JackUnreachable { pos: Pos }`; `StratumUnreachable` may now name `Net`.

- [ ] **Step 1: Write the failing tests**

In the tests module of `validate.rs`:

```rust
    #[test]
    fn detects_moved_cyber_nodes_and_walled_in_jacks() {
        let mut c = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        assert!(c.validate().is_ok());
        let moved = c.net.nodes[0].cyber;
        c.net.nodes[0].cyber = Pos::new(moved.x, (moved.y + 1) % c.size.h as i32);
        let v = c.validate().unwrap_err();
        assert!(v.iter().any(|v| matches!(v, Violation::CyberAnchor(NodeId(0)))), "{v:?}");
        c.net.nodes[0].cyber = moved;

        let jack = c
            .connectors
            .iter()
            .find(|k| k.kind == ConnectorKind::Jack)
            .unwrap()
            .pos;
        for n in jack.neighbors8() {
            if let Some(t) = c.tile_mut(StratumKind::Street, n) {
                t.terrain = Terrain::Wall;
            }
        }
        let v = c.validate().unwrap_err();
        assert!(
            v.iter().any(|v| matches!(v, Violation::JackUnreachable { pos } if *pos == jack)),
            "{v:?}"
        );
    }

    #[test]
    fn jack_is_not_a_public_connector_for_districts() {
        let mut c = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        // Strip every non-jack public connector from district 0: it must now be flagged.
        let d = DistrictId(0);
        let keep: Vec<bool> = c
            .connectors
            .iter()
            .map(|k| k.kind == ConnectorKind::Jack || !k.public || c_district(&c, k.pos) != Some(d))
            .collect();
        let mut i = 0;
        c.connectors.retain(|_| {
            let k = keep[i];
            i += 1;
            k
        });
        let v = c.validate().unwrap_err();
        assert!(v.iter().any(|v| matches!(v, Violation::DistrictEmpty(x) if *x == d)), "{v:?}");
    }

    fn c_district(c: &City, p: Pos) -> Option<DistrictId> {
        c.tile(StratumKind::Street, p).and_then(|t| t.district)
    }

    #[test]
    fn net_reachability_is_checked_from_the_jacks() {
        let mut c = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        // Wall off the whole grid except the ports: almost nothing is reachable.
        let jacks: Vec<Pos> = c
            .connectors
            .iter()
            .filter(|k| k.kind == ConnectorKind::Jack)
            .map(|k| k.pos)
            .collect();
        let size = c.size;
        let net = c.stratum_mut(StratumKind::Net).unwrap();
        for y in 0..size.h as i32 {
            for x in 0..size.w as i32 {
                let p = Pos::new(x, y);
                if let Some(t) = net.tiles.get_mut(p)
                    && t.terrain == Terrain::NetGrid
                    && !jacks.contains(&p)
                {
                    t.terrain = Terrain::NetWall;
                }
            }
        }
        let v = c.validate().unwrap_err();
        assert!(
            v.iter().any(|v| matches!(v, Violation::StratumUnreachable { stratum: StratumKind::Net, .. })),
            "{v:?}"
        );
    }
```

The test module's imports gain `Generator`, `NodeId`, `ConnectorKind` (already imported), `Violation`.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p citygen validate::`
Expected: compile error, `no variant CyberAnchor`.

- [ ] **Step 3: Implement**

`validate.rs`: enum gains

```rust
    CyberAnchor(NodeId),
    JackUnreachable { pos: Pos },
```

Display arms:

```rust
            Violation::CyberAnchor(n) => write!(f, "net node {n:?} is not on its cyberspace tile"),
            Violation::JackUnreachable { pos } => {
                write!(f, "jack at {pos:?} has no walkable neighbour or a dead port")
            }
```

Connector loop becomes:

```rust
        for c in &self.connectors {
            if c.kind == ConnectorKind::Jack {
                let street_ok = self
                    .tile(StratumKind::Street, c.pos)
                    .is_some_and(|t| t.feature == Some(Feature::Terminal))
                    && c.pos.neighbors8().any(|n| self.walkable(StratumKind::Street, n));
                let net_ok = self.walkable(StratumKind::Net, c.pos);
                if !street_ok || !net_ok {
                    out.push(Violation::JackUnreachable { pos: c.pos });
                }
                continue;
            }
            for k in &c.serves { /* unchanged */ }
        }
```

After the physical reachability loop:

```rust
        if let Some(net) = self.stratum(StratumKind::Net) {
            let jacks: Vec<Pos> = self
                .connectors
                .iter()
                .filter(|c| c.kind == ConnectorKind::Jack)
                .map(|c| c.pos)
                .collect();
            let walkable = net.tiles.count(|t| t.is_walkable());
            if !jacks.is_empty() && walkable > 0 {
                let mut seen = Grid::filled(self.size, false);
                let mut queue: VecDeque<Pos> = VecDeque::new();
                for p in jacks {
                    if net.tiles.get(p).is_some_and(|t| t.is_walkable()) && !seen[p] {
                        seen.set(p, true);
                        queue.push_back(p);
                    }
                }
                while let Some(p) = queue.pop_front() {
                    for n in p.neighbors4() {
                        if net.tiles.get(n).is_some_and(|t| t.is_walkable()) && !seen[n] {
                            seen.set(n, true);
                            queue.push_back(n);
                        }
                    }
                }
                let reached = seen.count(|b| *b);
                if (reached as f32) < REACH_TARGET * walkable as f32 {
                    out.push(Violation::StratumUnreachable {
                        stratum: StratumKind::Net,
                        reached,
                        walkable,
                    });
                }
            }
        }
```

Cyber anchors, after the existing `NetAnchor` loop:

```rust
        for n in &self.net.nodes {
            let ok = self
                .tile(StratumKind::Net, n.cyber)
                .is_some_and(|t| t.feature == Some(Feature::Node(n.kind)));
            if !ok {
                out.push(Violation::CyberAnchor(n.id));
            }
        }
```

Only when the Net stratum exists: wrap in `if self.stratum(StratumKind::Net).is_some()` so `bare_city` fixtures without a Net stay valid.

District check: `c.public && c.kind != ConnectorKind::Jack && ...`.

Imports: `use crate::geom::{Grid, Pos}; use crate::world::{City, ConnectorKind, StratumKind}; use std::collections::VecDeque;`.

- [ ] **Step 4: Run the suite**

Run: `cargo test --workspace`
Expected: PASS, including the seed sweeps (which call `validate`).

- [ ] **Step 5: Commit**

```bash
git add crates/citygen/src/validate.rs
git commit -m "feat(citygen): validate cyberspace anchors, jacks and reachability"
```

---

### Task 6: Digital overlay in cyberspace

**Files:**
- Modify: `crates/cyberpunk/src/render/overlay.rs`
- Test: same file

**Interfaces:**
- Consumes: `tileset::node_glyph`, `Net::node_in_cyber`.
- Produces: `Digital::node_under(city, stratum, p)` returns the node by `cyber` when `stratum == Net`.

- [ ] **Step 1: Write the failing test**

Add to the overlay tests module:

```rust
    #[test]
    fn overlay_in_cyberspace_finds_nodes_by_cyber_tile_and_draws_links() {
        let c = Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Custom { w: 80, h: 40 },
            districts: 4,
            ..Default::default()
        })
        .unwrap();
        let s = StylePack::load().unwrap();
        let server = c
            .net
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Server && n.building.is_some())
            .unwrap();
        assert_eq!(
            Digital::node_under(&c, StratumKind::Net, server.cyber),
            Some(server.id)
        );
        let backbone = c
            .net
            .links
            .iter()
            .find_map(|l| {
                let (a, b) = (c.net.node(l.0), c.net.node(l.1));
                if a.id == server.id && b.kind == NodeKind::Backbone {
                    Some(b)
                } else if b.id == server.id && a.kind == NodeKind::Backbone {
                    Some(a)
                } else {
                    None
                }
            })
            .unwrap();
        let mut t = Terminal::new(TestBackend::new(80, 40)).unwrap();
        t.draw(|f| {
            MapView {
                city: &c,
                stratum: StratumKind::Net,
                origin: Pos::new(0, 0),
                tick: 0,
                style: &s,
                vis: None,
                overlays: &[&Digital {
                    enabled: true,
                    focus: Some(server.id),
                }],
                cursor: None,
            }
            .render(f.area(), f.buffer_mut())
        })
        .unwrap();
        // The focused link is drawn as dots between the two cyber tiles.
        let dots = citygen::pathing::bresenham(server.cyber, backbone.cyber)
            .into_iter()
            .filter(|p| c.net.node_in_cyber(*p).is_none())
            .filter(|p| t.backend().buffer()[(p.x as u16, p.y as u16)].symbol() == "·")
            .count();
        assert!(dots >= 1, "no link dots drawn");
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p cyberpunk overlay_in_cyberspace`
Expected: FAIL on `node_under` (returns `None` for a cyber tile) or on `dots >= 1`.

- [ ] **Step 3: Implement**

`overlay.rs`:

```rust
use crate::render::tileset::node_glyph;

impl Digital {
    pub fn node_under(city: &City, stratum: StratumKind, p: Pos) -> Option<NodeId> {
        if stratum == StratumKind::Net {
            city.net.node_in_cyber(p).map(|n| n.id)
        } else {
            city.net.node_at(stratum, p).map(|n| n.id)
        }
    }
}
```

Delete `glyph_index`; the node-glyph line becomes `let g = node_glyph(n.kind, style);`.

In `draw`, right after the `enabled` check:

```rust
        let focus_subnet = self.focus.map(|id| city.net.node(id).subnet);
        if stratum == StratumKind::Net {
            // Nodes are tiles here; only the focused subnet's links are drawn.
            let Some(sn) = focus_subnet else { return };
            for link in &city.net.links {
                let a = city.net.node(link.0);
                let b = city.net.node(link.1);
                if a.subnet != sn && b.subnet != sn {
                    continue;
                }
                for p in bresenham(a.cyber, b.cyber) {
                    if city.net.node_in_cyber(p).is_some() {
                        continue;
                    }
                    if let Some((sx, sy)) = tile_to_screen(area, origin, p) {
                        buf[(sx, sy)]
                            .set_char('·')
                            .set_fg(SIGNAL.scale(0.8).to_color());
                    }
                }
            }
            return;
        }
```

The physical-side code below stays as it is (minus the removed `glyph_index`).

- [ ] **Step 4: Run the suite**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cyberpunk/src/render/overlay.rs
git commit -m "feat(render): digital overlay works inside cyberspace"
```

---

### Task 7: The player jacks in and out

**Files:**
- Modify: `crates/cyberpunk/src/walker/mod.rs`
- Test: same file

**Interfaces:**
- Produces: `Player.body: Option<(StratumKind, Pos)>`, `Player::jack_in(&mut self, city: &City) -> Result<Pos, &'static str>`, `Player::jack_out(&mut self, city: &City) -> Result<(), &'static str>`, `Player::jacked_in(&self) -> bool`, `RADIUS_NET: i32 = 24`.

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn jacking_in_leaves_the_body_at_the_port() {
        let c = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        let jack = c
            .connectors
            .iter()
            .find(|k| k.kind == citygen::ConnectorKind::Jack && k.public)
            .unwrap();
        let stand = jack
            .pos
            .neighbors8()
            .find(|n| c.walkable(StratumKind::Street, *n))
            .unwrap();
        let mut p = Player::spawn(&c, 1);
        p.pos = stand;
        p.stratum = StratumKind::Street;
        assert!(!p.jacked_in());
        assert_eq!(p.jack_in(&c), Ok(jack.pos));
        assert!(p.jacked_in());
        assert_eq!(p.stratum, StratumKind::Net);
        assert_eq!(p.pos, jack.pos);
        assert_eq!(p.body, Some((StratumKind::Street, stand)));
        assert_eq!(p.radius(), RADIUS_NET);
        assert!(p.vis().count(|v| *v == Visibility::Visible) > 10);
        assert_eq!(p.jack_in(&c), Err("already jacked in"));
        assert_eq!(p.use_connector(&c, false), None, "jacks are not stairs");
        assert_eq!(p.use_connector(&c, true), None);
        assert_eq!(p.jack_out(&c), Ok(()));
        assert_eq!((p.stratum, p.pos, p.body), (StratumKind::Street, stand, None));
        assert_eq!(p.jack_out(&c), Err("not jacked in"));
        // Far from any terminal.
        let far = c
            .stratum(StratumKind::Street)
            .unwrap()
            .tiles
            .iter()
            .find(|(q, t)| {
                t.is_walkable()
                    && std::iter::once(*q)
                        .chain(q.neighbors8())
                        .all(|n| c.connectors_at(n).all(|k| k.kind != citygen::ConnectorKind::Jack))
            })
            .map(|(q, _)| q)
            .unwrap();
        p.pos = far;
        assert_eq!(p.jack_in(&c), Err("no terminal within reach"));
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p cyberpunk jacking_in_leaves`
Expected: compile error, `no method named jack_in`.

- [ ] **Step 3: Implement**

```rust
pub const RADIUS_NET: i32 = 24;

#[derive(Clone, Debug)]
pub struct Player {
    pub pos: Pos,
    pub stratum: StratumKind,
    /// Where the body waits while jacked in: (stratum, tile). None when not jacked in.
    pub body: Option<(StratumKind, Pos)>,
    /// What has been seen on each stratum, indexed by `StratumKind::index`.
    memory: Vec<Grid<Visibility>>,
}
```

`spawn`: `body: None,` and `memory: StratumKind::ALL.iter().map(|_| Grid::filled(city.size, Visibility::Unseen)).collect()`.

`radius`:

```rust
    pub fn radius(&self) -> i32 {
        match self.stratum {
            StratumKind::Undercity => RADIUS_UNDER,
            StratumKind::Net => RADIUS_NET,
            _ => RADIUS_STREET,
        }
    }
```

`use_connector`: add `.filter(|c| c.kind != ConnectorKind::Jack)` before `filter_map`. Import `ConnectorKind`.

New methods:

```rust
    pub fn jacked_in(&self) -> bool {
        self.body.is_some()
    }

    /// Jacks in at a Jack connector on this tile or an 8-neighbour of it, leaving the body here.
    pub fn jack_in(&mut self, city: &City) -> Result<Pos, &'static str> {
        if self.jacked_in() {
            return Err("already jacked in");
        }
        let here = self.pos;
        let k = self.stratum;
        let port = std::iter::once(here)
            .chain(here.neighbors8())
            .find_map(|p| {
                city.connectors_at(p)
                    .find(|c| c.kind == ConnectorKind::Jack && c.serves.contains(&k))
                    .map(|c| c.pos)
            })
            .ok_or("no terminal within reach")?;
        if !city.walkable(StratumKind::Net, port) {
            return Err("the port is dead");
        }
        self.body = Some((k, here));
        self.stratum = StratumKind::Net;
        self.pos = port;
        self.update_fov(city);
        Ok(port)
    }

    /// Returns to the body.
    pub fn jack_out(&mut self, city: &City) -> Result<(), &'static str> {
        let (k, p) = self.body.take().ok_or("not jacked in")?;
        self.stratum = k;
        self.pos = p;
        self.update_fov(city);
        Ok(())
    }
```

- [ ] **Step 4: Run the suite**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cyberpunk/src/walker/mod.rs
git commit -m "feat(walker): jack in and out, body left at the port"
```

---

### Task 8: Free Explore cursor with a following camera

**Files:**
- Modify: `crates/cyberpunk/src/screens/explore.rs`
- Test: same file

**Interfaces:**
- Consumes: `Camera::follow(p, view_w, view_h)`, `Camera::clamp`, `Camera::origin`.
- Produces: `MapScreen.cursor: Pos` (Explore-mode inspector position).

- [ ] **Step 1: Write the failing test and fix the old one**

Add:

```rust
    #[test]
    fn explore_cursor_reaches_the_map_corners_without_showing_void() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        s.update(&mut st, &[]);
        let map = s.last_map.get();
        for _ in 0..40 {
            s.update(&mut st, &[key(KeyCode::Char('H')), key(KeyCode::Char('K'))]);
        }
        assert_eq!(s.cursor, Pos::new(0, 0));
        for _ in 0..30 {
            s.update(&mut st, &[]);
        }
        let o = s.cam.origin(map.width, map.height);
        assert!(o.x >= 0 && o.y >= 0, "origin {o:?}");
        for _ in 0..40 {
            s.update(&mut st, &[key(KeyCode::Char('L')), key(KeyCode::Char('J'))]);
        }
        assert_eq!(s.cursor, Pos::new(119, 59));
        for _ in 0..30 {
            s.update(&mut st, &[]);
        }
        let o = s.cam.origin(map.width, map.height);
        assert!(o.x + map.width as i32 <= 120 && o.y + map.height as i32 <= 60, "origin {o:?}");
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("119,59"), "{text}");
    }
```

In `explore_pans_and_switches_to_other_screens`, replace the camera assertion:

```rust
        let before = s.cursor;
        s.update(&mut st, &[key(KeyCode::Right), key(KeyCode::Down)]);
        assert_eq!(s.cursor, Pos::new(before.x + 1, before.y + 1));
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p cyberpunk explore_cursor_reaches`
Expected: compile error, `no field cursor`.

- [ ] **Step 3: Implement**

`MapScreen` gains `pub cursor: Pos` (init `Pos::new(0, 0)` in `explore`). In `update`, where `cam_ready` is set:

```rust
        if !self.cam_ready {
            self.cam.snap(size.center());
            self.cursor = size.center();
            self.cam_ready = true;
        }
```

`cursor()`:

```rust
    fn cursor(&self, st: &AppState) -> Pos {
        match (self.mode, &st.player) {
            (Mode::Walk, Some(p)) => p.pos,
            _ => self.cursor,
        }
    }
```

`handle_explore` pan arm:

```rust
            {
                let (dx, dy) = dir_for(code).expect("checked").delta();
                let size = st.city.as_ref().map(|c| c.size)?;
                self.cursor = Pos::new(
                    (self.cursor.x + dx * step).clamp(0, size.w as i32 - 1),
                    (self.cursor.y + dy * step).clamp(0, size.h as i32 - 1),
                );
            }
```

In `update`, the follow block becomes:

```rust
        let map = self.last_map.get();
        match (self.mode, &st.player) {
            (Mode::Walk, Some(p)) => self.cam.follow(p.pos, map.width, map.height),
            _ => self.cam.follow(self.cursor, map.width, map.height),
        }
```

`handle_walk` `Tab`: `KeyCode::Tab => { self.cursor = player.pos; self.mode = Mode::Explore; }`.

- [ ] **Step 4: Run the suite**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cyberpunk/src/screens/explore.rs
git commit -m "fix(explore): cursor moves freely and the camera follows it"
```

---

### Task 9: Cyberspace on screen — fifth stratum, Enter to jack, avatar and body

**Files:**
- Modify: `crates/cyberpunk/src/screens/explore.rs`
- Modify: `crates/cyberpunk/src/screens/help.rs:11-31`
- Modify: `crates/cyberpunk/src/ui/panel.rs` (`inspector_lines`, `strata_selector`)
- Test: `crates/cyberpunk/src/screens/explore.rs`

**Interfaces:**
- Consumes: `Player::{jack_in, jack_out, jacked_in, body}`, `Terrain::label`, `StratumKind::ALL`, `Digital::node_under`.

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn enter_jacks_in_and_the_net_is_the_fifth_stratum() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        for _ in 0..4 {
            s.update(&mut st, &[key(KeyCode::Char(']'))]);
        }
        assert_eq!(s.stratum, StratumKind::Net);
        s.update(&mut st, &[key(KeyCode::Char(']'))]);
        assert_eq!(s.stratum, StratumKind::Net);
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("▌ Net"), "{text}");
        assert!(text.contains('═') || text.contains('┃') || text.contains('━'), "{text}");

        // Walk up to a public port and jack in.
        s.update(&mut st, &[key(KeyCode::Char('['))]);
        s.update(&mut st, &[key(KeyCode::Char('w'))]);
        let city = st.city.as_ref().unwrap();
        let jack = city
            .connectors
            .iter()
            .find(|k| k.kind == citygen::ConnectorKind::Jack && k.public)
            .unwrap()
            .pos;
        let stand = jack
            .neighbors8()
            .find(|n| city.walkable(StratumKind::Street, *n))
            .unwrap();
        {
            let p = st.player.as_mut().unwrap();
            p.pos = stand;
            p.stratum = StratumKind::Street;
        }
        s.update(&mut st, &[key(KeyCode::Enter)]);
        let p = st.player.as_ref().unwrap();
        assert!(p.jacked_in());
        assert_eq!(s.stratum, StratumKind::Net);
        assert_eq!(p.pos, jack);
        st.tick += 500;
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("jack out"), "{text}");
        assert!(text.contains("jacked in"), "{text}");
        // The body shows on the Street while the avatar is in the Net.
        s.mode = Mode::Explore;
        s.stratum = StratumKind::Street;
        s.cursor = stand;
        s.cam.snap(stand);
        t.draw(|f| s.draw(&st, f)).unwrap();
        let map = s.last_map.get();
        let origin = s.cam.origin(map.width, map.height);
        let (sx, sy) = tile_to_screen(map, origin, stand).unwrap();
        assert_eq!(t.backend().buffer()[(sx, sy)].symbol(), "@");
        s.mode = Mode::Walk;
        s.stratum = StratumKind::Net;
        s.update(&mut st, &[key(KeyCode::Enter)]);
        let p = st.player.as_ref().unwrap();
        assert!(!p.jacked_in());
        assert_eq!((p.stratum, p.pos), (StratumKind::Street, stand));
        assert_eq!(s.stratum, StratumKind::Street);
        s.update(&mut st, &[key(KeyCode::Char('?'))]);
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert!(buffer_text(t.backend().buffer()).contains("jack in"));
    }
```

Note on the `"jacked in"` assertion: the status set by the key is transient; the test advances `tick` by 500 before drawing, so the assertion needs the badge in the hints line, not the status. The hints while jacked in must contain both `jack out` and `jacked in` (see Step 3).

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p cyberpunk enter_jacks_in`
Expected: FAIL at `assert_eq!(s.stratum, StratumKind::Net)` (still clamped to Heights).

- [ ] **Step 3: Implement**

`explore.rs`:

```rust
    fn change_stratum(&mut self, st: &AppState, delta: i32) {
        let max = st.city.as_ref().map(|c| c.strata.len() as i32 - 1).unwrap_or(3);
        let i = (self.stratum.index() as i32 + delta).clamp(0, max);
        if let Some(k) = StratumKind::from_index(i as usize) {
            self.stratum = k;
        }
    }
```

(call sites pass `st`.)

`handle_walk` gains, before the `code =>` arm:

```rust
            KeyCode::Enter => {
                let result = if player.jacked_in() {
                    player.jack_out(city).map(|_| None)
                } else {
                    player.jack_in(city).map(Some)
                };
                match result {
                    Ok(Some(port)) => {
                        self.stratum = StratumKind::Net;
                        self.cam.snap(port);
                        let where_ = city
                            .tile(StratumKind::Street, port)
                            .and_then(|t| t.building)
                            .and_then(|b| city.building_opt(b))
                            .and_then(|b| b.name.clone())
                            .or_else(|| {
                                city.tile(StratumKind::Street, port)
                                    .and_then(|t| t.district)
                                    .and_then(|d| city.district_opt(d))
                                    .map(|d| d.name.clone())
                            })
                            .unwrap_or_else(|| "the Net".to_string());
                        st.set_status(format!("jacked in at {where_}"), 90);
                    }
                    Ok(None) => {
                        self.stratum = player.stratum;
                        self.cam.snap(player.pos);
                        st.set_status("jacked out", 60);
                    }
                    Err(e) => st.set_status(e, 60),
                }
            }
```

`st.set_status` needs the player borrow released first: bind `let player = st.player.as_mut()?;` is already there; restructure the arm to compute `result`, `port`, and the message while `player` is borrowed, then drop it before `st.set_status` (take `let msg: String = ...;` then `st.set_status(msg, 90)` after the match). The compiler will insist; follow it.

Hints:

```rust
        let hints = match (self.mode, st.player.as_ref().is_some_and(|p| p.jacked_in())) {
            (Mode::Explore, _) => "hjkl pan · [] level · w walk · d net · e legend · s save · ? keys",
            (Mode::Walk, true) => "⚡ jacked in · hjkl move · enter jack out · tab camera · d net · ? keys",
            (Mode::Walk, false) => "hjkl move · <> stairs · enter jack in · tab camera · ? keys",
        };
```

Avatar and body drawing replaces the `@` block:

```rust
        if let Some(p) = &st.player {
            if let Some((bk, bp)) = p.body
                && bk == self.stratum
                && let Some((sx, sy)) = tile_to_screen(map, origin, bp)
            {
                f.buffer_mut()[(sx, sy)].set_char('@').set_style(
                    Style::default()
                        .fg(Color::Rgb(110, 120, 130))
                        .add_modifier(Modifier::DIM),
                );
            }
            if p.stratum == self.stratum
                && let Some((sx, sy)) = tile_to_screen(map, origin, p.pos)
            {
                let fg = if p.stratum == StratumKind::Net {
                    Color::Rgb(0, 255, 200)
                } else {
                    Color::Rgb(255, 255, 255)
                };
                f.buffer_mut()[(sx, sy)]
                    .set_char('@')
                    .set_style(Style::default().fg(fg).add_modifier(Modifier::BOLD));
            }
        }
```

Node focus for the inspector: `let focus = if self.digital || self.stratum == StratumKind::Net { Digital::node_under(city, self.stratum, cursor) } else { None };`.

`help.rs`: EXPLORE row `("[ ]", "stratum down / up (Net on top)")`; WALK gains `("enter", "jack in at an adjacent terminal / jack out")` after the `< >` row.

`panel.rs`: `strata_selector` iterates `StratumKind::ALL` (doc comment: "Every stratum, top first"); `inspector_lines` uses `t.terrain.label()` and

```rust
    match t.feature {
        Some(Feature::Node(k)) => out.push(kv("feature", format!("node · {}", k.label()))),
        Some(f) => out.push(kv("feature", format!("{f:?}").to_lowercase())),
        None => {}
    }
```

(import `citygen::Feature`).

- [ ] **Step 4: Run the suite**

Run: `cargo test --workspace`
Expected: PASS. The existing `explore_and_walk_round_trip` still expects `[` from Street twice to land on Undercity and stay there.

- [ ] **Step 5: Commit**

```bash
git add crates/cyberpunk/src/screens/explore.rs crates/cyberpunk/src/screens/help.rs crates/cyberpunk/src/ui/panel.rs
git commit -m "feat(screens): walk cyberspace, jack in on enter, body left behind"
```

---

### Task 10: Save format version 2

**Files:**
- Modify: `crates/cyberpunk/src/save.rs`
- Test: same file

**Interfaces:**
- Produces: `FORMAT_VERSION = 2`; `split` returns `(Header, u16, &[u8])`; `load_city` refuses version 1 naming the seed.

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn version_one_files_list_but_refuse_to_load() {
        let dir = std::env::temp_dir().join(format!("cyberpunk-v1-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let header = postcard::to_allocvec(&Header {
            name: "Old Town".into(),
            seed: 42,
            w: 80,
            h: 40,
        })
        .unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&(header.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(&[0u8; 8]);
        let path = dir.join("old.city");
        std::fs::write(&path, &bytes).unwrap();
        let listed = list_cities(&dir).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].seed, 42);
        let err = load_city(&path).unwrap_err().to_string();
        assert!(err.contains("older version") && err.contains("seed 42"), "{err}");
        assert_eq!(FORMAT_VERSION, 2);
        std::fs::remove_dir_all(&dir).ok();
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p cyberpunk version_one_files`
Expected: FAIL: `listed.len()` is 0 (split rejects version 1) or `FORMAT_VERSION == 2` fails.

- [ ] **Step 3: Implement**

```rust
pub const FORMAT_VERSION: u16 = 2;
/// Versions whose header this build can read (for listing); only FORMAT_VERSION loads.
const READABLE_VERSIONS: [u16; 2] = [1, 2];

fn split<'a>(bytes: &'a [u8], path: &Path) -> Result<(Header, u16, &'a [u8])> {
    /* magic check unchanged */
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if !READABLE_VERSIONS.contains(&version) {
        bail!(
            "{} is .city version {version}; this build reads version {FORMAT_VERSION}",
            path.display()
        );
    }
    /* header parse unchanged */
    Ok((header, version, &bytes[header_end..]))
}

pub fn load_city(path: &Path) -> Result<City> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let (header, version, body) = split(&bytes, path)?;
    if version != FORMAT_VERSION {
        bail!(
            "{} was saved by an older version ({version}); regenerate it from seed {}",
            path.display(),
            header.seed
        );
    }
    /* rest unchanged */
}
```

`list_cities`: `let Ok((header, _, _)) = split(&bytes, &path) else { continue };`.

- [ ] **Step 4: Run the suite**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cyberpunk/src/save.rs
git commit -m "feat(save): format version 2 with cyberspace; refuse v1 by seed"
```

---

### Task 11: Docs, lint, sweep, smoke

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-10-12-cyberspace-design.md` (only if a ruling changed a stated behaviour)

- [ ] **Step 1: README**

Strata table gains a first row `| Net | cyberspace: a dotted grid, constructs on every building, hubs, lanes, ICE at the gates |`. After the Net graph paragraph add:

```
Jack in at any street terminal (walk up to it and press `enter`): your body
stays at the port and you emerge in cyberspace at the same spot. `enter` again
jacks out. In Explore mode `]` past Heights shows the Net.
```

Keys: **Explore** `[` `]` → "stratum down / up (Net on top)"; **Walk** adds `enter` jack in / jack out. Layout line for `crates/cyberpunk` mentions cyberspace.

- [ ] **Step 2: Lint and full checks**

Run:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p citygen --test seeds -- --include-ignored
```

Expected: all green. Fix formatting or clippy findings in place.

- [ ] **Step 3: Timing**

Run: `cargo build --release -p cyberpunk && time ./target/release/cyberpunk gen --seed 1 --size large --out /tmp/large.city && time ./target/release/cyberpunk gen --seed 1 --size 1024x512 --out /tmp/huge.city`
Expected: large well under 0.3 s; 1024x512 under 10 s. Record both in the final report.

- [ ] **Step 4: Pty smoke**

Drive the release binary through a pseudo-terminal (reuse the session's `drive2.py` approach): title → generate → explore `]` ×4 (Net visible, selector shows Net) → `[` → `w` → navigate to a port → `enter` → hints show "jack out" → `enter` → `q`. Expected: no panic, clean exit. Keep the script in the scratchpad, not the repo.

- [ ] **Step 5: Commit**

```bash
git add README.md
git commit -m "docs: cyberspace keys and stratum"
```
