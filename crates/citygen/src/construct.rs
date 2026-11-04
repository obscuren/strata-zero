//! A construct's inside: the Net map behind a building's gate, generated on demand
//! from the city seed and the building. Rooms off a spine hold the building's
//! nodes; ICE stands at the gate and at the vault; the vault is deepest.

use crate::geom::{Grid, Pos, Rect, Size};
use crate::ids::{BuildingId, NodeId};
use crate::net::{NodeKind, SubnetOwner};
use crate::pathing::flood_fill;
use crate::rng::{hash64, tile_hash};
use crate::tile::{Feature, IceGrade, Terrain, Tile};
use crate::world::{City, StratumKind};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Security from which a construct has ICE at its gate.
pub const ICE_THRESHOLD: u8 = 160;
/// Security from which the ICE is grey and the vault has its own guard.
pub const GREY_THRESHOLD: u8 = 200;
/// Security from which the vault's guard is black.
pub const BLACK_THRESHOLD: u8 = 230;
const MIN_W: i32 = 14;
const MAX_W: i32 = 40;
const MIN_H: i32 = 11;
const MAX_H: i32 = 24;
const CONSTRUCT_SALT: u64 = 0xC0DE_5EED;

/// A piece of ICE: where it was made, where it stands now (the tile carrying
/// `Feature::Ice`), and the spine it walks when asleep (empty for white).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Guard {
    pub post: Pos,
    pub pos: Pos,
    pub grade: IceGrade,
    pub beat: Vec<Pos>,
    /// Standing, or lifted by a crack or a break.
    pub standing: bool,
    /// A dummy: it never wakes, hits or breaks; a target to practise on.
    pub inert: bool,
}

impl Guard {
    /// `grey`, `black`; `dummy` for an inert piece.
    pub fn label(&self) -> &'static str {
        if self.inert {
            "dummy"
        } else {
            self.grade.label()
        }
    }
}

/// The lot's construct, for practice: one open floor the size of the lot (no lane
/// grid) with a grey ICE at the far end and a dummy near the gate, nothing else.
pub fn lot_construct() -> Construct {
    let size = Size { w: 41, h: 19 };
    let wall = Tile {
        terrain: Terrain::NetWall,
        feature: None,
        district: None,
        building: None,
        variant: 0,
    };
    let mut tiles = Grid::filled(size, wall);
    let floor = Tile {
        terrain: Terrain::NetFloor,
        ..wall
    };
    tiles.fill_rect(Rect::new(1, 1, 39, 17), floor);
    let entry = Pos::new(0, 9);
    tiles[entry] = Tile {
        terrain: Terrain::NetGate,
        ..wall
    };
    let guards = vec![
        Guard {
            post: Pos::new(38, 9),
            pos: Pos::new(38, 9),
            grade: IceGrade::Grey,
            beat: Vec::new(),
            standing: true,
            inert: false,
        },
        Guard {
            post: Pos::new(6, 3),
            pos: Pos::new(6, 3),
            grade: IceGrade::Grey,
            beat: Vec::new(),
            standing: true,
            inert: true,
        },
    ];
    for g in &guards {
        tiles[g.pos].feature = Some(Feature::Ice(g.grade));
    }
    Construct {
        building: BuildingId(0),
        security: 200,
        size,
        tiles,
        light: Grid::filled(size, 255),
        entry,
        rooms: Vec::new(),
        nodes: Vec::new(),
        guards,
    }
}

/// The inside of a construct.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Construct {
    pub building: BuildingId,
    pub security: u8,
    pub size: Size,
    pub tiles: Grid<Tile>,
    pub light: Grid<u8>,
    /// The gate on the border: in from the city's gate tile, out to it.
    pub entry: Pos,
    /// In order along the spine; the last holds the vault when there is one.
    pub rooms: Vec<Rect>,
    pub nodes: Vec<(NodeId, Pos)>,
    /// The gate guard first, the vault guard second.
    pub guards: Vec<Guard>,
}

impl Construct {
    pub fn node_at(&self, p: Pos) -> Option<NodeId> {
        self.nodes.iter().find(|(_, q)| *q == p).map(|(id, _)| *id)
    }

    /// The index of the guard standing on a tile; a lifted guard is not there.
    pub fn guard_at(&self, p: Pos) -> Option<usize> {
        self.guards.iter().position(|g| g.pos == p && g.standing)
    }

    /// The tile's feature follows whoever stands on it.
    fn refresh_ice(&mut self, p: Pos) {
        if !self.tiles.in_bounds(p) {
            return;
        }
        let standing = self.guards.iter().find(|g| g.pos == p && g.standing);
        self.tiles[p].feature = standing.map(|g| Feature::Ice(g.grade));
    }

    pub fn vault_room(&self) -> Option<Rect> {
        let last = *self.rooms.last()?;
        self.nodes
            .iter()
            .any(|(_, p)| {
                last.contains(*p)
                    && matches!(
                        self.tiles[*p].feature,
                        Some(Feature::Node(NodeKind::DataVault))
                    )
            })
            .then_some(last)
    }

    /// Lifts or restores a guard: its tile's feature, and with it its walkability. A
    /// guard never stands up under another standing guard.
    pub fn set_guard(&mut self, index: usize, standing: bool) {
        let Some(pos) = self.guards.get(index).map(|g| g.pos) else {
            return;
        };
        if standing && self.guard_at(pos).is_some_and(|i| i != index) {
            return;
        }
        self.guards[index].standing = standing;
        self.refresh_ice(pos);
    }

    /// Walks a guard to a tile: the feature travels with it.
    pub fn move_guard(&mut self, index: usize, to: Pos) {
        let Some(from) = self.guards.get(index).map(|g| g.pos) else {
            return;
        };
        if !self.tiles.in_bounds(to) || from == to {
            return;
        }
        self.guards[index].pos = to;
        self.refresh_ice(from);
        self.refresh_ice(to);
    }
}

/// The construct of a building with a subnet; `None` for one without.
pub fn generate_construct(city: &City, b: BuildingId) -> Option<Construct> {
    let building = city.building_opt(b)?;
    let security = city
        .net
        .subnets
        .iter()
        .find(|s| s.owner == SubnetOwner::Building(b))
        .map(|s| s.security)?;
    let fp = building.footprint;
    let (long, short) = (fp.w.max(fp.h), fp.w.min(fp.h));
    let (w, h) = (
        (long * 2).clamp(MIN_W, MAX_W),
        (short * 2).clamp(MIN_H, MAX_H),
    );
    let size = Size {
        w: w as u32,
        h: h as u32,
    };
    let mut rng =
        ChaCha8Rng::seed_from_u64(hash64(hash64(city.seed, b.index() as u64), CONSTRUCT_SALT));
    let glow = 96u8.saturating_add((security as u32 * 5 / 8) as u8);
    let base = Tile {
        terrain: Terrain::NetWall,
        feature: None,
        district: Some(building.district),
        building: Some(b),
        variant: 0,
    };
    let mut tiles = Grid::filled(size, base);
    let mut light = Grid::filled(size, glow);
    for p in Rect::new(0, 0, w, h).iter() {
        tiles[p].variant = tile_hash(city.seed, StratumKind::Net, p);
    }
    let sy = h / 2;
    let carve = |tiles: &mut Grid<Tile>, light: &mut Grid<u8>, p: Pos, terrain: Terrain| {
        tiles[p].terrain = terrain;
        if matches!(terrain, Terrain::NetLane | Terrain::NetGate) {
            light[p] = crate::lighting::tuning::NET_LANE_LIGHT;
        }
    };
    // The end room closes the spine; side rooms open off it on both sides.
    // Narrow enough that at least one side room fits before it.
    let end_w = rng.random_range(3..=6).min(w - 11).max(3);
    let end = Rect::new(w - 1 - end_w, sy - 1, end_w, 3);
    let end_door = Pos::new(end.x - 1, sy);
    let entry = Pos::new(0, sy);
    carve(&mut tiles, &mut light, entry, Terrain::NetGate);
    for x in 1..end.x - 1 {
        carve(&mut tiles, &mut light, Pos::new(x, sy), Terrain::NetLane);
    }
    carve(&mut tiles, &mut light, end_door, Terrain::NetGate);
    for p in end.iter() {
        carve(&mut tiles, &mut light, p, Terrain::NetFloor);
    }
    let mut rooms: Vec<Rect> = Vec::new();
    let mut x = 2;
    let mut top = rng.random_bool(0.5);
    let last_x = end.x - 3;
    while x + 4 <= last_x {
        let rw = rng.random_range(4..=7).min(last_x - x);
        let depth_room = if top { sy - 2 } else { h - 3 - sy };
        let rd = rng.random_range(3..=5).min(depth_room);
        let r = if top {
            Rect::new(x, sy - 1 - rd, rw, rd)
        } else {
            Rect::new(x, sy + 2, rw, rd)
        };
        for p in r.iter() {
            carve(&mut tiles, &mut light, p, Terrain::NetFloor);
        }
        let door = Pos::new(x + rw / 2, if top { sy - 1 } else { sy + 1 });
        carve(&mut tiles, &mut light, door, Terrain::NetGate);
        rooms.push(r);
        x += rw + 1 + rng.random_range(0..=2);
        top = !top;
    }
    rooms.push(end);
    // Nodes: peripherals through the side rooms, the server before the vault.
    let mut by_kind: Vec<(NodeId, NodeKind)> = city
        .net
        .nodes
        .iter()
        .filter(|n| n.building == Some(b) && n.kind != NodeKind::Ice)
        .map(|n| (n.id, n.kind))
        .collect();
    let rank = |k: NodeKind| match k {
        NodeKind::DoorLock => 0,
        NodeKind::LiftCtrl => 1,
        NodeKind::Camera => 2,
        NodeKind::Terminal => 3,
        NodeKind::AdBoard => 4,
        NodeKind::LampGrid | NodeKind::Backbone => 5,
        NodeKind::Server => 6,
        NodeKind::DataVault => 7,
        NodeKind::Ice => 8,
    };
    by_kind.sort_by_key(|(id, k)| (rank(*k), id.index()));
    let has_vault = by_kind.iter().any(|(_, k)| *k == NodeKind::DataVault);
    let side_rooms = rooms.len() - 1;
    let mut free: Vec<Vec<Pos>> = rooms
        .iter()
        .map(|r| {
            let mut v: Vec<Pos> = r.iter().collect();
            v.sort_by_key(|p| (p.y, p.x));
            v
        })
        .collect();
    let mut nodes: Vec<(NodeId, Pos)> = Vec::new();
    let take = |free: &mut Vec<Vec<Pos>>, room: usize, rng: &mut ChaCha8Rng| -> Option<Pos> {
        let slots = &mut free[room];
        if slots.is_empty() {
            return None;
        }
        let i = rng.random_range(0..slots.len());
        Some(slots.swap_remove(i))
    };
    let peripheral_rooms: Vec<usize> = match (side_rooms, has_vault) {
        (0, _) => vec![rooms.len() - 1],
        (1, true) => vec![0],
        (n, true) => (0..n - 1).collect(),
        (n, false) => (0..n).collect(),
    };
    let mut next_room = 0usize;
    for (id, kind) in by_kind {
        let room = match kind {
            NodeKind::DataVault => rooms.len() - 1,
            NodeKind::Server => {
                if has_vault {
                    side_rooms.saturating_sub(1)
                } else {
                    rooms.len() - 1
                }
            }
            _ => {
                let r = peripheral_rooms[next_room % peripheral_rooms.len()];
                next_room += 1;
                r
            }
        };
        let mut tries = 0;
        let pos = loop {
            if let Some(p) = take(&mut free, (room + tries) % rooms.len(), &mut rng) {
                break p;
            }
            tries += 1;
            if tries >= rooms.len() {
                break Pos::new(end.x, sy);
            }
        };
        tiles[pos].feature = Some(Feature::Node(kind));
        nodes.push((id, pos));
    }
    // Guards: the gate guard walks the near half of the spine, the vault guard the
    // far half; they meet in the middle. White keeps its post.
    let mut guards = Vec::new();
    if security >= ICE_THRESHOLD {
        let spine: Vec<Pos> = (2..=end_door.x).map(|x| Pos::new(x, sy)).collect();
        let mid = spine.len() / 2;
        let grade = if security >= GREY_THRESHOLD {
            IceGrade::Grey
        } else {
            IceGrade::White
        };
        let post = Pos::new(2, sy);
        guards.push(Guard {
            post,
            pos: post,
            grade,
            beat: if grade == IceGrade::White {
                Vec::new()
            } else {
                spine[..=mid].to_vec()
            },
            standing: true,
            inert: false,
        });
        if has_vault && security >= GREY_THRESHOLD {
            let grade = if security >= BLACK_THRESHOLD {
                IceGrade::Black
            } else {
                IceGrade::Grey
            };
            guards.push(Guard {
                post: end_door,
                pos: end_door,
                grade,
                beat: spine[mid..].to_vec(),
                standing: true,
                inert: false,
            });
        }
    }
    for g in &guards {
        tiles[g.pos].feature = Some(Feature::Ice(g.grade));
    }
    Some(Construct {
        building: b,
        security,
        size,
        tiles,
        light,
        entry,
        rooms,
        nodes,
        guards,
    })
}

/// The rules every construct keeps: the entry is a gate on the border; every node
/// and every room is reachable from the entry with the guards lifted; nodes do not
/// share tiles; guards stand where one could walk.
pub fn validate_construct(c: &Construct) -> Result<(), String> {
    let size = c.tiles.size();
    if size != c.size {
        return Err(format!(
            "tiles are {:?}, the construct says {:?}",
            size, c.size
        ));
    }
    let (w, h) = (size.w as i32, size.h as i32);
    let on_border = c.entry.x == 0 || c.entry.y == 0 || c.entry.x == w - 1 || c.entry.y == h - 1;
    if !on_border || c.tiles[c.entry].terrain != Terrain::NetGate {
        return Err(format!("entry {:?} is not a gate on the border", c.entry));
    }
    let passable = |p: Pos| {
        let t = &c.tiles[p];
        let mut lifted = *t;
        if matches!(t.feature, Some(Feature::Ice(_))) {
            lifted.feature = None;
        }
        lifted.is_walkable()
    };
    let reach = flood_fill(size, c.entry, passable);
    let reached = |p: Pos| reach.contains(&p);
    for (id, p) in &c.nodes {
        if !reached(*p) {
            return Err(format!("node {:?} at {p:?} is unreachable", id));
        }
        if c.nodes.iter().filter(|(_, q)| q == p).count() > 1 {
            return Err(format!("two nodes at {p:?}"));
        }
    }
    for (i, r) in c.rooms.iter().enumerate() {
        if let Some(p) = r.iter().find(|p| !reached(*p)) {
            return Err(format!("room {i} tile {p:?} is unreachable"));
        }
    }
    for g in &c.guards {
        let mut t = c.tiles[g.pos];
        t.feature = None;
        if !t.is_walkable() {
            return Err(format!("guard at {:?} stands in a wall", g.pos));
        }
        for p in &g.beat {
            if !matches!(c.tiles[*p].terrain, Terrain::NetLane | Terrain::NetGate) {
                return Err(format!("beat tile {p:?} is not spine"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_construct_comes_back_with_its_guards() {
        use super::*;
        let mut k = lot_construct();
        k.guards[0].pos = Pos::new(30, 9);
        k.guards[0].standing = false;
        let bytes = rmp_serde::to_vec_named(&k).unwrap();
        let back: Construct = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(back.guards[0].pos, Pos::new(30, 9));
        assert!(!back.guards[0].standing);
        assert_eq!(back.guards, k.guards);
        assert_eq!(back, k);
    }

    #[test]
    fn the_lot_construct_is_a_lane_with_a_grey_ice_and_a_dummy() {
        use super::*;
        let k = lot_construct();
        assert!(
            validate_construct(&k).is_ok(),
            "{:?}",
            validate_construct(&k)
        );
        assert_eq!(k.entry, Pos::new(0, 9));
        assert_eq!(k.tiles[k.entry].terrain, Terrain::NetGate);
        assert_eq!(k.tiles[Pos::new(20, 9)].terrain, Terrain::NetFloor);
        assert_eq!(k.tiles[Pos::new(40, 9)].terrain, Terrain::NetWall);
        assert_eq!(k.guards.len(), 2);
        assert_eq!(
            (k.guards[0].pos, k.guards[0].inert, k.guards[0].label()),
            (Pos::new(38, 9), false, "grey")
        );
        assert_eq!(
            (k.guards[1].pos, k.guards[1].inert, k.guards[1].label()),
            (Pos::new(6, 3), true, "dummy")
        );
        assert_eq!(k.guard_at(Pos::new(6, 3)), Some(1));
        assert!(k.nodes.is_empty() && k.rooms.is_empty());
        assert_eq!(k.security, 200);
    }

    use super::*;
    use crate::generate::cyberspace::tests::city;
    use crate::geom::Pos;
    use crate::net::{NodeKind, SubnetOwner};
    use crate::tile::{Feature, IceGrade, Terrain};
    use crate::world::Place;

    fn security_of(c: &crate::world::City, b: crate::ids::BuildingId) -> u8 {
        c.net
            .subnets
            .iter()
            .find(|s| s.owner == SubnetOwner::Building(b))
            .map(|s| s.security)
            .unwrap()
    }

    #[test]
    fn every_building_with_a_subnet_gets_a_valid_construct_holding_its_nodes() {
        let c = city();
        let mut made = 0;
        for b in &c.buildings {
            let Some(k) = generate_construct(&c, b.id) else {
                continue;
            };
            made += 1;
            validate_construct(&k).unwrap_or_else(|e| panic!("{:?}: {e}", b.id));
            assert_eq!(k.building, b.id);
            assert_eq!(k.security, security_of(&c, b.id));
            let expected: Vec<_> = c
                .net
                .nodes
                .iter()
                .filter(|n| n.building == Some(b.id) && n.kind != NodeKind::Ice)
                .map(|n| n.id)
                .collect();
            let mut placed: Vec<_> = k.nodes.iter().map(|(id, _)| *id).collect();
            placed.sort_by_key(|id| id.index());
            let mut expected = expected;
            expected.sort_by_key(|id| id.index());
            assert_eq!(placed, expected, "{:?}", b.id);
            for (id, p) in &k.nodes {
                assert_eq!(
                    k.tiles[*p].feature,
                    Some(Feature::Node(c.net.node(*id).kind))
                );
                assert_eq!(k.node_at(*p), Some(*id));
            }
            assert!((14..=40).contains(&k.size.w) && (11..=24).contains(&k.size.h));
            // The vault is in the last room, the server in the one before.
            let room_of = |p: crate::geom::Pos| k.rooms.iter().position(|r| r.contains(p));
            let vault = k
                .nodes
                .iter()
                .find(|(id, _)| c.net.node(*id).kind == NodeKind::DataVault);
            let server = k
                .nodes
                .iter()
                .find(|(id, _)| c.net.node(*id).kind == NodeKind::Server)
                .expect("a server");
            if let Some((_, vp)) = vault {
                assert_eq!(room_of(*vp), Some(k.rooms.len() - 1), "{:?}", b.id);
                assert_eq!(k.vault_room(), k.rooms.last().copied());
                assert_eq!(room_of(server.1), Some(k.rooms.len() - 2), "{:?}", b.id);
            } else {
                assert_eq!(room_of(server.1), Some(k.rooms.len() - 1), "{:?}", b.id);
                assert_eq!(k.vault_room(), None);
            }
            // Guards by security.
            let gate_guard = k.guards.first();
            match k.security {
                s if s >= 200 => {
                    assert_eq!(
                        gate_guard.map(|g| g.grade),
                        Some(IceGrade::Grey),
                        "{:?}",
                        b.id
                    );
                    if vault.is_some() {
                        assert_eq!(k.guards.len(), 2);
                        let want = if s >= 230 {
                            IceGrade::Black
                        } else {
                            IceGrade::Grey
                        };
                        assert_eq!(k.guards[1].grade, want, "{:?} at {s}", b.id);
                        assert_eq!(k.tiles[k.guards[1].pos].terrain, Terrain::NetGate);
                    }
                }
                s if s >= 160 => {
                    assert_eq!(
                        gate_guard.map(|g| g.grade),
                        Some(IceGrade::White),
                        "{:?}",
                        b.id
                    );
                    assert_eq!(k.guards.len(), 1);
                }
                _ => assert!(k.guards.is_empty(), "{:?}", b.id),
            }
            for g in &k.guards {
                assert_eq!(k.tiles[g.pos].feature, Some(Feature::Ice(g.grade)));
                assert!(!k.tiles[g.pos].is_walkable());
                assert_eq!(g.pos, g.post);
                // A beat is walkable spine; white has none.
                assert_eq!(g.beat.is_empty(), g.grade == IceGrade::White, "{:?}", b.id);
                for p in &g.beat {
                    assert!(
                        matches!(k.tiles[*p].terrain, Terrain::NetLane | Terrain::NetGate),
                        "{:?} beat tile {p:?} is {:?}",
                        b.id,
                        k.tiles[*p].terrain
                    );
                }
            }
            if k.guards.len() == 2 {
                let (gate, vault_g) = (&k.guards[0], &k.guards[1]);
                let mid = gate.beat.last().unwrap();
                assert_eq!(gate.beat.first(), Some(&gate.post));
                assert_eq!(
                    vault_g.beat.first(),
                    Some(mid),
                    "the halves meet in the middle"
                );
                assert_eq!(vault_g.beat.last(), Some(&vault_g.post));
                assert!(gate.beat.len() >= 2 && vault_g.beat.len() >= 2);
            }
            assert_eq!(k.tiles[k.entry].terrain, Terrain::NetGate);
            assert_eq!(generate_construct(&c, b.id).unwrap(), k, "deterministic");
        }
        assert!(made >= 10, "{made} constructs");
        assert!(c.buildings.iter().any(|b| security_of(&c, b.id) >= 200));
        assert!(c.buildings.iter().any(|b| security_of(&c, b.id) < 160));
    }

    #[test]
    fn a_white_ice_construct_has_one_guard_at_the_gate() {
        let mut c = city();
        let b = c.buildings[0].id;
        let sn = c
            .net
            .subnets
            .iter_mut()
            .find(|s| s.owner == SubnetOwner::Building(b))
            .unwrap();
        sn.security = 170;
        let k = generate_construct(&c, b).unwrap();
        validate_construct(&k).unwrap();
        assert_eq!(k.guards.len(), 1);
        assert_eq!(k.guards[0].grade, IceGrade::White);
        assert_eq!(k.guards[0].pos, Pos::new(2, k.entry.y));
        sn_security(&mut c, b, 150);
        assert!(generate_construct(&c, b).unwrap().guards.is_empty());
    }

    fn sn_security(c: &mut crate::world::City, b: crate::ids::BuildingId, s: u8) {
        c.net
            .subnets
            .iter_mut()
            .find(|sn| sn.owner == SubnetOwner::Building(b))
            .unwrap()
            .security = s;
    }

    #[test]
    fn black_ice_guards_the_vault_from_230() {
        let mut c = city();
        let b = c
            .buildings
            .iter()
            .find(|b| {
                c.net
                    .nodes
                    .iter()
                    .any(|n| n.building == Some(b.id) && n.kind == NodeKind::DataVault)
            })
            .unwrap()
            .id;
        sn_security(&mut c, b, 229);
        let k = generate_construct(&c, b).unwrap();
        assert_eq!(
            k.guards.iter().map(|g| g.grade).collect::<Vec<_>>(),
            vec![IceGrade::Grey, IceGrade::Grey]
        );
        sn_security(&mut c, b, 230);
        let k = generate_construct(&c, b).unwrap();
        assert_eq!(
            k.guards.iter().map(|g| g.grade).collect::<Vec<_>>(),
            vec![IceGrade::Grey, IceGrade::Black]
        );
        validate_construct(&k).unwrap();
        assert_eq!(IceGrade::Black.integrity(), 10);
        assert_eq!(IceGrade::Grey.sight(), 6);
        assert_eq!(IceGrade::White.sight(), 4);
    }

    #[test]
    fn a_guard_moves_with_its_feature() {
        let mut c = city();
        let b = c.buildings[0].id;
        sn_security(&mut c, b, 200);
        let mut k = generate_construct(&c, b).unwrap();
        let from = k.guards[0].pos;
        let to = k.guards[0].beat[1];
        k.move_guard(0, to);
        assert_eq!(k.guards[0].pos, to);
        assert_eq!(k.guards[0].post, from);
        assert!(k.tiles[from].feature.is_none() && k.tiles[from].is_walkable());
        assert_eq!(k.tiles[to].feature, Some(Feature::Ice(IceGrade::Grey)));
        assert_eq!(k.guard_at(to), Some(0));
        assert_eq!(k.guard_at(from), None);
        // Lifting and standing again happen where the piece is.
        k.set_guard(0, false);
        assert!(k.tiles[to].is_walkable());
        k.set_guard(0, true);
        assert_eq!(k.guard_at(to), Some(0));
    }

    #[test]
    fn guards_lift_and_stand_again() {
        let c = city();
        let b = c
            .buildings
            .iter()
            .find(|b| security_of(&c, b.id) >= 160)
            .unwrap()
            .id;
        let mut k = generate_construct(&c, b).unwrap();
        let p = k.guards[0].pos;
        assert_eq!(k.guard_at(p), Some(0));
        k.set_guard(0, false);
        assert!(k.tiles[p].is_walkable() && k.tiles[p].feature.is_none());
        assert_eq!(k.guard_at(p), None, "a lifted guard is not there");
        k.set_guard(0, true);
        assert!(!k.tiles[p].is_walkable());
        assert_eq!(k.guard_at(p), Some(0));
    }

    #[test]
    fn a_construct_place_is_no_stratum() {
        let b = crate::ids::BuildingId(3);
        assert_eq!(Place::Construct(b).stratum(), None);
        assert!(!Place::Construct(b).is_street());
    }
}
