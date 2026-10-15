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
const MIN_W: i32 = 14;
const MAX_W: i32 = 40;
const MIN_H: i32 = 11;
const MAX_H: i32 = 24;
const LANE_LIGHT: u8 = 255;
const CONSTRUCT_SALT: u64 = 0xC0DE_5EED;

/// A piece of ICE standing on a construct tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Guard {
    pub pos: Pos,
    pub grade: IceGrade,
}

/// The inside of a construct.
#[derive(Clone, Debug, PartialEq)]
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
        self.guards
            .iter()
            .position(|g| g.pos == p && matches!(self.tiles[p].feature, Some(Feature::Ice(_))))
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

    /// Lifts or restores a guard: its tile's feature, and with it its walkability.
    pub fn set_guard(&mut self, index: usize, standing: bool) {
        let Some(g) = self.guards.get(index).copied() else {
            return;
        };
        self.tiles[g.pos].feature = standing.then_some(Feature::Ice(g.grade));
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
            light[p] = LANE_LIGHT;
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
    // Guards.
    let mut guards = Vec::new();
    if security >= ICE_THRESHOLD {
        let grade = if security >= GREY_THRESHOLD {
            IceGrade::Grey
        } else {
            IceGrade::White
        };
        guards.push(Guard {
            pos: Pos::new(2, sy),
            grade,
        });
        if has_vault && security >= GREY_THRESHOLD {
            guards.push(Guard {
                pos: end_door,
                grade: IceGrade::Grey,
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
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
                        assert_eq!(k.guards[1].grade, IceGrade::Grey);
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
