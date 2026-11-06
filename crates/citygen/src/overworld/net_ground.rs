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
const FADE: f32 = 13.0;

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
    let mut rooms = Grid::filled(size, false);
    for (l, t) in ground.iter() {
        rooms[l] = is_room(t);
    }
    let mut net = Net {
        grid: Grid::filled(size, Tile::new(Terrain::Void)),
        origin: Pos::new(bx, by),
        rooms,
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
    let room = |p: Pos| ground.get(p).is_some_and(is_room);
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
        // a door with no floor behind it still gets its spur, out of whichever side
        // isn't the room
        let out = match inward {
            Some(inside) => {
                net.node(net.world(inside), NodeKind::Terminal);
                (g.x - inside.x, g.y - inside.y)
            }
            None => [(0, 1), (0, -1), (1, 0), (-1, 0)]
                .into_iter()
                .find(|d| !room(g.offset(d.0, d.1)))
                .unwrap_or((0, 1)),
        };
        net.spur(world, net.world(g), out);
    }
    // Lone nodes: cameras, the stop's sign, a mast, each on a light line to a lane.
    for (l, t) in ground.iter() {
        let kind = match t.feature {
            Some(Feature::Camera) => NodeKind::Camera,
            // a sign's first letter is its ad server (not a warning sign's)
            Some(Feature::Sign(c))
                if c != '!'
                    && !matches!(
                        ground.get(l.offset(-1, 0)).and_then(|t| t.feature),
                        Some(Feature::Sign(_))
                    )
                    && !matches!(
                        ground.get(l.offset(0, -1)).and_then(|t| t.feature),
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
        // round 2's lattice of dots, thinning out from signal, the odd one far off
        if p.x.rem_euclid(4) != 0 || p.y.rem_euclid(2) != 0 {
            continue;
        }
        let r = roll(world.seed, 3301, p);
        let keep = if *dist <= FADE {
            r < 1.0 - dist / FADE
        } else {
            r < 0.01
        };
        if keep {
            net.grid[l].terrain = Terrain::NetGrid;
        }
    }
    // The rectangle asked for.
    let mut out = Grid::filled(Size { w, h }, Tile::new(Terrain::Void));
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let mut t = net.grid[Pos::new(x + MARGIN, y + MARGIN)];
            t.variant = crate::rng::tile_hash(
                world.seed,
                crate::world::StratumKind::Net,
                Pos::new(x0 + x, y0 + y),
            );
            out[Pos::new(x, y)] = t;
        }
    }
    out
}

/// Whether a ground cell is part of a building (which becomes a construct).
fn is_room(t: &Tile) -> bool {
    matches!(
        t.terrain,
        Terrain::Wall | Terrain::Window | Terrain::Door | Terrain::Floor
    )
}

/// The Net being drawn, over the built rectangle.
struct Net {
    grid: Grid<Tile>,
    origin: Pos,
    /// The ground's buildings, which a spur goes round rather than through.
    rooms: Grid<bool>,
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

    fn in_room(&self, p: Pos) -> bool {
        self.rooms.get(self.local(p)).copied().unwrap_or(false)
    }

    /// A spur out of a gate along `out` to the first trunk straight ahead; if none
    /// within reach, to the nearest trunk the world's roads give, stepping sideways
    /// first where the way round a building needs it.
    fn spur(&mut self, world: &Overworld, gate: Pos, out: (i32, i32)) {
        let mut path = Vec::new();
        let mut q = gate;
        for _ in 0..SPUR {
            q = q.offset(out.0, out.1);
            if self.in_room(q) {
                break;
            }
            if on_trunk(world, q) {
                for c in path {
                    self.lane(c);
                }
                return;
            }
            path.push(q);
        }
        let Some(to) = trunk_near(world, gate) else {
            return;
        };
        let start = gate.offset(out.0, out.1);
        let side = (out.1, out.0);
        let mut fallback = None;
        for k in 0..=12 {
            for s in [2 * k, -2 * k] {
                for vertical_first in [true, false] {
                    let turn = start.offset(side.0 * s, side.1 * s);
                    let mut cells = straight(start, turn);
                    cells.extend(l_path(turn, to, vertical_first));
                    if !cells.iter().any(|c| self.in_room(*c)) {
                        for c in cells {
                            self.lane(c);
                        }
                        return;
                    }
                    fallback.get_or_insert(cells);
                }
            }
        }
        for c in fallback.unwrap_or_default() {
            self.lane(c);
        }
    }

    /// A light line from a lone node to the nearest lane straight off it (within
    /// reach), else to the nearest trunk.
    fn line_to_lane(&mut self, world: &Overworld, p: Pos) {
        let best = [(0, -1), (1, 0), (0, 1), (-1, 0)]
            .into_iter()
            .filter_map(|d: (i32, i32)| {
                (1..=SPUR)
                    .find(|k| on_trunk(world, p.offset(d.0 * k, d.1 * k)))
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
                    for c in l_path(p, to, true) {
                        self.lane(c);
                    }
                }
            }
        }
    }
}

/// Whether `q` lies on a highway's trunk: its tile's road centreline (world data only,
/// so every spur and line chooses the same whatever region is being built).
fn on_trunk(world: &Overworld, q: Pos) -> bool {
    let t = Pos::new(q.x.div_euclid(TILE), q.y.div_euclid(TILE));
    world
        .tile(t)
        .filter(|x| x.road != 0)
        .is_some_and(|x| centreline(world, t.x, t.y, x.road).contains(&q))
}

/// The nearest trunk cell to `p` by the world's roads: the centreline cell of the nearest
/// road tile within ten tiles.
fn trunk_near(world: &Overworld, p: Pos) -> Option<Pos> {
    let t = Pos::new(p.x.div_euclid(TILE), p.y.div_euclid(TILE));
    (0..=10).find_map(|r| {
        (-r..=r)
            .flat_map(|dy| (-r..=r).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy): &(i32, i32)| dx.abs().max(dy.abs()) == r)
            .map(|(dx, dy)| t.offset(dx, dy))
            .find(|n| world.tile(*n).is_some_and(|x| x.road != 0))
            .map(|n| Pos::new(TILE * n.x + 3, TILE * n.y + 3))
    })
}

/// Cells from `a` to `b` in two straight legs: across to `b`'s row first, or along to
/// `b`'s column first.
fn l_path(a: Pos, b: Pos, vertical_first: bool) -> Vec<Pos> {
    let corner = if vertical_first {
        Pos::new(a.x, b.y)
    } else {
        Pos::new(b.x, a.y)
    };
    let mut out = straight(a, corner);
    out.extend(straight(corner, b));
    out.push(b);
    out
}

/// The cells from `a` up to (not including) `b`, which share a row or a column.
fn straight(a: Pos, b: Pos) -> Vec<Pos> {
    let mut out = Vec::new();
    let mut q = a;
    while q != b {
        out.push(q);
        q = q.offset((b.x - q.x).signum(), (b.y - q.y).signum());
    }
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
        let straight = |p: Pos| {
            (-2..=2).all(|dx| {
                w.tile(p.offset(dx, 0))
                    .is_some_and(|t| t.road == LINK_E | LINK_W && t.water == Water::None)
            })
        };
        let t = w
            .tiles
            .iter()
            .map(|(p, _)| p)
            .filter(|p| straight(*p))
            .nth(2)
            .unwrap();
        let g = region(
            &w,
            TILE * (t.x - 2),
            TILE * (t.y - 3),
            (TILE * 5) as u32,
            (TILE * 7) as u32,
        );
        let grid = |rows: std::ops::Range<i32>| {
            rows.flat_map(|y| (0..TILE * 5).map(move |x| Pos::new(x, y)))
                .filter(|p| g[*p].terrain == Terrain::NetGrid)
                .count()
        };
        let road_row = 3 * TILE + 3;
        let near = grid(road_row - 4..road_row - 1);
        let far = grid(0..3);
        assert!(near >= 3 * far.max(1), "near {near}, far {far}");
    }

    #[test]
    fn every_kinds_net_is_the_same_built_alone() {
        let mut seen = Vec::new();
        for seed in 0..20 {
            let w = small(seed);
            for poi in &w.pois {
                if seen.contains(&poi.kind) && !(seed == 7 && poi.at == Pos::new(68, 6)) {
                    continue;
                }
                seen.push(poi.kind);
                let o = Pos::new(TILE * (poi.at.x - 1), TILE * (poi.at.y - 1));
                let r = region(&w, o.x, o.y, (TILE * 3) as u32, (TILE * 3) as u32);
                for dy in 0..3 {
                    for dx in 0..3 {
                        let (tx, ty) = (poi.at.x - 1 + dx, poi.at.y - 1 + dy);
                        let g = region(&w, TILE * tx, TILE * ty, TILE as u32, TILE as u32);
                        for (c, a) in g.iter() {
                            let b = r[Pos::new(TILE * dx + c.x, TILE * dy + c.y)];
                            assert_eq!(
                                (a.terrain, a.feature),
                                (b.terrain, b.feature),
                                "seed {seed} {:?} at {:?}: {tx},{ty} {c:?}",
                                poi.kind,
                                poi.at
                            );
                        }
                    }
                }
            }
        }
        assert!(seen.len() >= 8, "{seen:?}");
    }

    #[test]
    fn every_kinds_gates_are_wired() {
        // every gate on or beside its POI's tile joins a trunk over lanes (the region
        // reaches past the furthest trunk a spur can make for)
        const R: i32 = 13;
        let mut wired = 0;
        for seed in 0..8 {
            let w = small(seed);
            for poi in w.pois.iter().filter(|p| p.kind != PoiKind::Rig) {
                let o = Pos::new(TILE * (poi.at.x - R), TILE * (poi.at.y - R));
                let span = (TILE * (2 * R + 1)) as u32;
                let g = region(&w, o.x, o.y, span, span);
                let near = |l: Pos| {
                    (l.x.div_euclid(TILE) - R).abs() <= 1 && (l.y.div_euclid(TILE) - R).abs() <= 1
                };
                let gates = g
                    .iter()
                    .filter(|(l, t)| t.terrain == Terrain::NetGate && near(*l))
                    .map(|(l, _)| l)
                    .collect::<Vec<_>>();
                for s in gates {
                    let at = |l: Pos| Pos::new(o.x + l.x, o.y + l.y);
                    if trunk_near(&w, at(s)).is_none() {
                        continue;
                    }
                    let mut seen = Grid::filled(g.size(), false);
                    let mut stack = vec![s];
                    let mut trunk = false;
                    while let Some(p) = stack.pop() {
                        trunk |= on_trunk(&w, at(p));
                        for q in p.neighbors4() {
                            if g.get(q).is_some_and(lane) && !seen[q] {
                                seen[q] = true;
                                stack.push(q);
                            }
                        }
                    }
                    assert!(
                        trunk,
                        "seed {seed} {:?} at {:?}: gate {s:?} reaches no trunk",
                        poi.kind, poi.at
                    );
                    wired += 1;
                }
            }
        }
        assert!(wired >= 20, "{wired} gates");
    }

    #[test]
    fn the_grid_is_a_lattice_that_shows() {
        let w = small(0);
        let t = w
            .tiles
            .iter()
            .filter(|(_, t)| t.road == LINK_E | LINK_W && t.water == Water::None)
            .map(|(p, _)| p)
            .nth(5)
            .unwrap();
        let o = Pos::new(TILE * t.x, TILE * (t.y - 2));
        let g = region(&w, o.x, o.y, TILE as u32, (TILE * 5) as u32);
        let grid: Vec<(Pos, Tile)> = g
            .iter()
            .filter(|(_, t)| t.terrain == Terrain::NetGrid)
            .map(|(p, t)| (Pos::new(o.x + p.x, o.y + p.y), *t))
            .collect();
        assert!(grid.len() >= 4, "{} grid dots", grid.len());
        assert!(
            grid.iter()
                .all(|(p, _)| p.x.rem_euclid(4) == 0 && p.y.rem_euclid(2) == 0)
        );
        let variants: std::collections::BTreeSet<u8> = g.iter().map(|(_, t)| t.variant).collect();
        assert!(
            variants.len() > 4,
            "cells carry their variant: {variants:?}"
        );
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
