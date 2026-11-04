//! The Net in the open: each tile's Net layer, derived from the world and the ground
//! round it, joined across tile edges as the ground is. See
//! `2026-11-04-world-tiles-design.md`, "The Net in the open".

use super::Overworld;
use super::detail::{self, TILE, centreline, roll};
use crate::geom::{Grid, Pos, Size};
use crate::net::NodeKind;
use crate::tile::{Feature, Terrain, Tile};

/// How far past the rectangle the ground and the Net are built, so every rule's reach
/// (a spur's walk, the grid's fade, a building's outline) is the same in any region.
const MARGIN: i32 = 48;

/// How far the grid fades out from signal, in screen columns (a row is two).
const FADE: f32 = 22.0;

/// The longest spur or light line, in cells.
const SPUR: i32 = 40;

/// The Net of world-wide detail rectangle `x0, y0, w, h`.
pub fn region(world: &Overworld, x0: i32, y0: i32, w: u32, h: u32) -> Grid<Tile> {
    let (bx, by) = (x0 - MARGIN, y0 - MARGIN);
    let size = Size {
        w: w + 2 * MARGIN as u32,
        h: h + 2 * MARGIN as u32,
    };
    let ground = detail::region(world, bx, by, size.w, size.h);
    let mut net = Net {
        grid: Grid::filled(size, Tile::new(Terrain::Void)),
        origin: Pos::new(bx, by),
    };
    // The trunks: every highway's centreline.
    let (tx0, ty0) = (bx.div_euclid(TILE) - 1, by.div_euclid(TILE) - 1);
    let (tx1, ty1) = (
        (bx + size.w as i32).div_euclid(TILE) + 1,
        (by + size.h as i32).div_euclid(TILE) + 1,
    );
    for ty in ty0..=ty1 {
        for tx in tx0..=tx1 {
            let Some(t) = world.tile(Pos::new(tx, ty)) else {
                continue;
            };
            if t.road != 0 {
                for c in centreline(world, tx, ty, t.road) {
                    net.lane(c);
                }
            }
        }
    }
    // The constructs: every POI room's outline, a gate where its door is, a node
    // inside the door; the spur from the gate out to a trunk.
    let room = |p: Pos| {
        ground.get(p).is_some_and(|t| {
            matches!(
                t.terrain,
                Terrain::Wall | Terrain::Window | Terrain::Door | Terrain::Floor
            )
        })
    };
    let mut gates = Vec::new();
    for (l, t) in ground.iter() {
        if !room(l) {
            continue;
        }
        let p = net.world(l);
        let rim = l.neighbors8().any(|q| !room(q));
        let terrain = match t.terrain {
            Terrain::Door => {
                gates.push(l);
                Terrain::NetGate
            }
            _ if rim => Terrain::NetWall,
            _ => Terrain::NetFloor,
        };
        net.set(p, terrain);
    }
    for &g in &gates {
        // in through the door: the node; out the other way: the spur
        let inward = g
            .neighbors4()
            .find(|q| ground.get(*q).is_some_and(|t| t.terrain == Terrain::Floor));
        let Some(inside) = inward else { continue };
        net.node(net.world(inside), NodeKind::Terminal);
        let out = (g.x - inside.x, g.y - inside.y);
        net.spur(world, net.world(g), out);
    }
    // Lone nodes: cameras, the stop's sign, a mast, each on a light line to a lane.
    for (l, t) in ground.iter() {
        let kind = match t.feature {
            Some(Feature::Camera) => NodeKind::Camera,
            Some(Feature::Sign(_))
                if !matches!(
                    ground.get(l.offset(-1, 0)).and_then(|t| t.feature),
                    Some(Feature::Sign(_))
                ) =>
            {
                NodeKind::AdBoard
            }
            Some(Feature::Pylon)
                if ground
                    .get(l.offset(1, 0))
                    .is_some_and(|t| t.feature == Some(Feature::Pylon)) =>
            {
                NodeKind::Backbone
            }
            _ => continue,
        };
        let p = net.world(l);
        net.node(p, kind);
        net.line_to_lane(world, p);
    }
    // The grid: near signal, fading out; the odd stray dot beyond.
    let lit = |q: Pos| {
        net.grid
            .get(q)
            .is_some_and(|t| t.terrain != Terrain::Void || t.feature.is_some())
    };
    let d = super::field::distance_field(size, lit, FADE + 1.0);
    for (l, dist) in d.iter() {
        if net.grid[l].terrain != Terrain::Void {
            continue;
        }
        let p = net.world(l);
        let r = roll(world.seed, 3301, p);
        let keep = if *dist <= FADE {
            let f = 1.0 - dist / FADE;
            r < 0.75 * f * f
        } else {
            r < 0.002
        };
        if keep {
            net.grid[l].terrain = Terrain::NetGrid;
        }
    }
    // The rectangle asked for.
    let mut out = Grid::filled(Size { w, h }, Tile::new(Terrain::Void));
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            out[Pos::new(x, y)] = net.grid[Pos::new(x + MARGIN, y + MARGIN)];
        }
    }
    out
}

/// The Net being drawn, over the built rectangle.
struct Net {
    grid: Grid<Tile>,
    origin: Pos,
}

impl Net {
    fn world(&self, l: Pos) -> Pos {
        Pos::new(l.x + self.origin.x, l.y + self.origin.y)
    }

    fn local(&self, p: Pos) -> Pos {
        Pos::new(p.x - self.origin.x, p.y - self.origin.y)
    }

    fn get(&self, p: Pos) -> Option<&Tile> {
        self.grid.get(self.local(p))
    }

    fn set(&mut self, p: Pos, t: Terrain) {
        let l = self.local(p);
        if let Some(c) = self.grid.get_mut(l) {
            c.terrain = t;
        }
    }

    /// A lane on `p`, unless a construct's wall or gate is there.
    fn lane(&mut self, p: Pos) {
        if self.get(p).is_some_and(|t| {
            !matches!(
                t.terrain,
                Terrain::NetWall | Terrain::NetGate | Terrain::NetFloor
            )
        }) {
            self.set(p, Terrain::NetLane);
        }
    }

    fn node(&mut self, p: Pos, kind: NodeKind) {
        let l = self.local(p);
        if let Some(c) = self.grid.get_mut(l) {
            if c.terrain == Terrain::Void || c.terrain == Terrain::NetGrid {
                c.terrain = Terrain::NetFloor;
            }
            c.feature = Some(Feature::Node(kind));
        }
    }

    fn is_lane(&self, p: Pos) -> bool {
        self.get(p).is_some_and(|t| t.terrain == Terrain::NetLane)
    }

    /// A spur out of a gate along `out` to the first lane; if none within reach, to the
    /// nearest trunk the world's roads give.
    fn spur(&mut self, world: &Overworld, gate: Pos, out: (i32, i32)) {
        let mut path = Vec::new();
        let mut q = gate;
        for _ in 0..SPUR {
            q = q.offset(out.0, out.1);
            if self.is_lane(q) {
                for c in path {
                    self.lane(c);
                }
                return;
            }
            path.push(q);
        }
        if let Some(to) = trunk_near(world, gate) {
            for c in l_path(gate.offset(out.0, out.1), to) {
                self.lane(c);
            }
        }
    }

    /// A light line from a lone node to the nearest lane straight off it (within
    /// reach), else to the nearest trunk.
    fn line_to_lane(&mut self, world: &Overworld, p: Pos) {
        let best = [(0, -1), (1, 0), (0, 1), (-1, 0)]
            .into_iter()
            .filter_map(|d: (i32, i32)| {
                (1..=SPUR)
                    .find(|k| self.is_lane(p.offset(d.0 * k, d.1 * k)))
                    .map(|k| (k, d))
            })
            .min_by_key(|(k, _)| *k);
        match best {
            Some((k, d)) => {
                for i in 1..k {
                    self.lane(p.offset(d.0 * i, d.1 * i));
                }
            }
            None => {
                if let Some(to) = trunk_near(world, p) {
                    for c in l_path(p, to) {
                        self.lane(c);
                    }
                }
            }
        }
    }
}

/// The nearest trunk cell to `p` by the world's roads: the centreline cell of the nearest
/// road tile within three tiles.
fn trunk_near(world: &Overworld, p: Pos) -> Option<Pos> {
    let t = Pos::new(p.x.div_euclid(TILE), p.y.div_euclid(TILE));
    (0..=3).find_map(|r| {
        (-r..=r)
            .flat_map(|dy| (-r..=r).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy): &(i32, i32)| dx.abs().max(dy.abs()) == r)
            .map(|(dx, dy)| t.offset(dx, dy))
            .find(|n| world.tile(*n).is_some_and(|x| x.road != 0))
            .map(|n| Pos::new(TILE * n.x + 3, TILE * n.y + 3))
    })
}

/// Cells from `a` to `b`, across to `b`'s row, then along it.
fn l_path(a: Pos, b: Pos) -> Vec<Pos> {
    let mut out = Vec::new();
    let mut q = a;
    while q.y != b.y {
        out.push(q);
        q.y += (b.y - q.y).signum();
    }
    while q.x != b.x {
        out.push(q);
        q.x += (b.x - q.x).signum();
    }
    out.push(q);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Pos;
    use crate::overworld::detail::TILE;
    use crate::overworld::{LINK_E, LINK_W, PoiKind, Water, WorldParams, WorldSize};
    use crate::tile::Feature;

    fn small(seed: u64) -> Overworld {
        Overworld::generate(WorldParams {
            seed,
            size: WorldSize::Small,
            ..Default::default()
        })
        .unwrap()
    }

    fn lane(t: &Tile) -> bool {
        matches!(t.terrain, Terrain::NetLane | Terrain::NetGate)
    }

    #[test]
    fn a_trunk_runs_unbroken_along_a_highway() {
        let w = small(0);
        let run: Vec<Pos> = w
            .tiles
            .iter()
            .filter(|(_, t)| t.road == LINK_E | LINK_W && t.water == Water::None)
            .map(|(p, _)| p)
            .take(20)
            .collect();
        assert!(run.len() >= 10);
        for t in run {
            let g = region(&w, TILE * t.x, TILE * t.y, TILE as u32, TILE as u32);
            for x in 0..TILE {
                assert!(lane(&g[Pos::new(x, 3)]), "{t:?} col {x}");
            }
        }
    }

    #[test]
    fn every_gate_is_wired_to_a_trunk() {
        let mut gates = 0;
        for seed in 0..4 {
            let w = small(seed);
            for poi in w.pois.iter().filter(|p| p.kind == PoiKind::TruckStop) {
                let o = Pos::new(TILE * (poi.at.x - 6), TILE * (poi.at.y - 4));
                let g = region(&w, o.x, o.y, (TILE * 13) as u32, (TILE * 9) as u32);
                let starts: Vec<Pos> = g
                    .iter()
                    .filter(|(_, t)| t.terrain == Terrain::NetGate)
                    .map(|(p, _)| p)
                    .collect();
                assert!(!starts.is_empty(), "seed {seed} {poi:?}: no gate");
                for s in starts {
                    // over lanes from the gate to a long straight run (the trunk)
                    let mut seen = Grid::filled(g.size(), false);
                    let mut stack = vec![s];
                    let mut n = 0;
                    while let Some(p) = stack.pop() {
                        n += 1;
                        for q in p.neighbors4() {
                            if g.get(q).is_some_and(lane) && !seen[q] {
                                seen[q] = true;
                                stack.push(q);
                            }
                        }
                    }
                    assert!(
                        n > 60,
                        "seed {seed} {poi:?}: the gate at {s:?} reaches {n} lane cells"
                    );
                    gates += 1;
                }
            }
        }
        assert!(gates >= 3, "{gates} gates");
    }

    #[test]
    fn the_barren_net_is_near_empty() {
        let w = small(0);
        // a tile three tiles from any road, POI or city
        let far = w
            .tiles
            .iter()
            .map(|(p, _)| p)
            .find(|p| {
                (-3..=3).all(|dy| {
                    (-3..=3).all(|dx| {
                        let q = p.offset(dx, dy);
                        w.tile(q).is_some_and(|t| t.road == 0 && t.urban.is_none())
                            && !w.pois.iter().any(|o| o.at == q)
                    })
                })
            })
            .unwrap();
        let g = region(&w, TILE * far.x, TILE * far.y, TILE as u32, TILE as u32);
        let signal = g.count(|t| t.terrain != Terrain::Void);
        assert!(signal <= 2, "{far:?}: {signal} cells of Net");
    }

    #[test]
    fn the_grid_fades_from_signal() {
        let w = small(0);
        let t = w
            .tiles
            .iter()
            .filter(|(_, t)| t.road == LINK_E | LINK_W && t.water == Water::None)
            .map(|(p, _)| p)
            .nth(5)
            .unwrap();
        let g = region(
            &w,
            TILE * t.x,
            TILE * (t.y - 3),
            TILE as u32,
            (TILE * 7) as u32,
        );
        let grid = |rows: std::ops::Range<i32>| {
            rows.flat_map(|y| (0..TILE).map(move |x| Pos::new(x, y)))
                .filter(|p| g[*p].terrain == Terrain::NetGrid)
                .count()
        };
        let road_row = 3 * TILE + 3;
        let near = grid(road_row - 4..road_row - 1);
        let far = grid(0..3);
        assert!(near >= 3 * far.max(1), "near {near}, far {far}");
    }

    #[test]
    fn a_net_tile_alone_equals_its_region() {
        let w = small(0);
        let poi = w
            .pois
            .iter()
            .find(|p| p.kind == PoiKind::TruckStop)
            .unwrap();
        let o = Pos::new(TILE * (poi.at.x - 2), TILE * (poi.at.y - 2));
        let r = region(&w, o.x, o.y, (TILE * 5) as u32, (TILE * 5) as u32);
        for dy in 0..5 {
            for dx in 0..5 {
                let (tx, ty) = (poi.at.x - 2 + dx, poi.at.y - 2 + dy);
                let g = region(&w, TILE * tx, TILE * ty, TILE as u32, TILE as u32);
                for (c, a) in g.iter() {
                    let b = r[Pos::new(TILE * dx + c.x, TILE * dy + c.y)];
                    assert_eq!(
                        (a.terrain, a.feature),
                        (b.terrain, b.feature),
                        "{tx},{ty} {c:?}"
                    );
                }
            }
        }
        let _ = Feature::Lamp;
    }
}
