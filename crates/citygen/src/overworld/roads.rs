//! Stage 6: highways. The cities as a graph (a spanning tree by distance, a short link
//! or two for loops, two or three roads off the map's edge), each link pathfound across
//! the land so it bends round hills and water.

use super::field::{length, s, sdist};
use super::{Ctx, Ground, LINK_E, LINK_N, LINK_S, LINK_W, Route, Water, link_toward};
use crate::geom::{Dir, Pos};
use crate::params::GenError;
use rand::Rng;

pub(crate) fn run(ctx: &mut Ctx) -> Result<(), GenError> {
    let mut rng = ctx.rng(5);
    let n = ctx.cities.len();
    if n == 0 {
        return Ok(());
    }
    let centres: Vec<Pos> = ctx.cities.iter().map(|c| c.centre).collect();
    let mut pairs: Vec<(f32, usize, usize)> = (0..n)
        .flat_map(|a| (a + 1..n).map(move |b| (a, b)))
        .map(|(a, b)| (sdist(centres[a], centres[b]), a, b))
        .collect();
    pairs.sort_by(|x, y| x.0.total_cmp(&y.0));
    // Kruskal: the shortest links that join two parts, skipping any with no way through.
    let mut part: Vec<usize> = (0..n).collect();
    fn root(part: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while part[r] != r {
            r = part[r];
        }
        part[i] = r;
        r
    }
    let mut tree_longest = 0.0f32;
    let mut spare = Vec::new();
    for &(d, a, b) in &pairs {
        let (ra, rb) = (root(&mut part, a), root(&mut part, b));
        if ra == rb {
            spare.push((d, a, b));
            continue;
        }
        if let Some(path) = road(ctx, centres[a], centres[b]) {
            part[ra] = rb;
            tree_longest = tree_longest.max(d);
            lay(ctx, path, None);
        }
    }
    let loops = rng.random_range(1..=2);
    for &(d, a, b) in spare
        .iter()
        .filter(|(d, _, _)| *d < 1.4 * tree_longest)
        .take(loops)
    {
        let _ = d;
        if let Some(path) = road(ctx, centres[a], centres[b]) {
            lay(ctx, path, None);
        }
    }
    exits(ctx, &mut rng, &centres);
    chain(ctx);
    Ok(())
}

/// What a turn costs a highway: about four tiles of flats, so it runs straight until the
/// land makes it bend.
const TURN: u32 = 40;

/// A highway from `a` to `b`, or `None` where the land gives no way through: the
/// cheapest way across the land, paying for every turn (an A* over tile and heading),
/// so it runs in long straights rather than a staircase.
fn road(ctx: &Ctx, a: Pos, b: Pos) -> Option<Vec<Pos>> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let bends = ctx.fbm(1301);
    let step = |to: Pos| -> Option<u32> {
        let t = ctx.tile(to);
        if t.water == Water::Sea || t.ground == Ground::Peak {
            return None;
        }
        if t.road != 0 {
            return Some(4);
        }
        let ground = match t.ground {
            Ground::Flats | Ground::Salt => 10.0,
            Ground::Scrub => 12.0,
            Ground::Fields => 14.0,
            Ground::Spill => 120.0,
            Ground::Hills => 30.0,
            Ground::Mountains => 90.0,
            Ground::Peak => unreachable!(),
        };
        let water = match t.water {
            Water::River => 70.0,
            Water::Lake => 500.0,
            _ => 0.0,
        };
        let (x, y) = s(to);
        Some((ground + water + 26.0 * bends.at(x / 7.0, y / 7.0, 3)) as u32)
    };
    const DIRS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
    let (w, h) = (ctx.size.w as i32, ctx.size.h as i32);
    let index = |p: Pos, d: usize| ((p.y * w + p.x) as usize) * 4 + d;
    let mut best = vec![u32::MAX; (w * h) as usize * 4];
    let mut came: Vec<Option<(Pos, usize)>> = vec![None; (w * h) as usize * 4];
    let mut heap = BinaryHeap::new();
    // Leaving the start, any heading is free.
    for d in 0..4 {
        best[index(a, d)] = 0;
        heap.push(Reverse((a.manhattan(b) as u32, 0u32, a.y, a.x, d)));
    }
    while let Some(Reverse((_, g, y, x, d))) = heap.pop() {
        let p = Pos::new(x, y);
        if g > best[index(p, d)] {
            continue;
        }
        if p == b {
            let mut path = vec![p];
            let mut at = (p, d);
            while let Some(prev) = came[index(at.0, at.1)] {
                if prev.0 != at.0 {
                    path.push(prev.0);
                }
                at = prev;
            }
            path.reverse();
            return Some(path);
        }
        for (nd, (dx, dy)) in DIRS.iter().enumerate() {
            let q = Pos::new(x + dx, y + dy);
            if !ctx.in_bounds(q) {
                continue;
            }
            let Some(c) = step(q) else { continue };
            let turn = if nd == d || p == a { 0 } else { TURN };
            let ng = g + c + turn;
            if ng < best[index(q, nd)] {
                best[index(q, nd)] = ng;
                came[index(q, nd)] = Some((p, d));
                heap.push(Reverse((ng + q.manhattan(b) as u32, ng, q.y, q.x, nd)));
            }
        }
    }
    None
}

/// Lays `path` on the tiles, linked tile to tile; `out` links its last tile off the map.
fn lay(ctx: &mut Ctx, path: Vec<Pos>, out: Option<Dir>) {
    for w in path.windows(2) {
        ctx.tile_mut(w[0]).road |= link_toward(w[0], w[1]);
        ctx.tile_mut(w[1]).road |= link_toward(w[1], w[0]);
    }
    if let (Some(d), Some(last)) = (out, path.last()) {
        ctx.tile_mut(*last).road |= match d {
            Dir::N => LINK_N,
            Dir::E => LINK_E,
            Dir::S => LINK_S,
            _ => LINK_W,
        };
    }
    ctx.routes.push(Route {
        name: String::new(),
        path,
    });
}

/// Two or three roads off the edge, from the cities nearest a side that isn't sea.
fn exits(ctx: &mut Ctx, rng: &mut impl Rng, centres: &[Pos]) {
    let (w, h) = (ctx.size.w as i32, ctx.size.h as i32);
    let mut options: Vec<(f32, usize, Dir, Pos)> = Vec::new();
    for (i, c) in centres.iter().enumerate() {
        for side in Dir::CARDINAL {
            if ctx.sea_sides.contains(&side) {
                continue;
            }
            let edge = match side {
                Dir::N => Pos::new(c.x, 0),
                Dir::S => Pos::new(c.x, h - 1),
                Dir::W => Pos::new(0, c.y),
                _ => Pos::new(w - 1, c.y),
            };
            let t = ctx.tile(edge);
            if t.water == Water::Sea || t.ground == Ground::Peak {
                continue;
            }
            options.push((sdist(*c, edge), i, side, edge));
        }
    }
    options.sort_by(|a, b| a.0.total_cmp(&b.0));
    let want = rng.random_range(2..=3);
    let mut used_cities = Vec::new();
    let mut used_sides = Vec::new();
    let mut made = 0;
    for (_, i, side, edge) in options {
        if made == want || used_cities.contains(&i) || used_sides.contains(&side) {
            continue;
        }
        if let Some(path) = road(ctx, centres[i], edge) {
            lay(ctx, path, Some(side));
            used_cities.push(i);
            used_sides.push(side);
            made += 1;
        }
    }
}

/// Links that carry straight on through a city become one road: at each city, the two
/// links leaving it most nearly opposite (over 135° apart) join.
fn chain(ctx: &mut Ctx) {
    let n = ctx.routes.len();
    let mut part: Vec<usize> = (0..n).collect();
    fn root(part: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while part[r] != r {
            r = part[r];
        }
        part[i] = r;
        r
    }
    for c in ctx.cities.iter().map(|c| c.centre) {
        // Each link leaving this city, with its heading a few tiles out.
        let mut leaving: Vec<(usize, (f32, f32))> = Vec::new();
        for (i, r) in ctx.routes.iter().enumerate() {
            let along: Vec<Pos> = if r.path[0] == c {
                r.path.clone()
            } else if *r.path.last().unwrap() == c {
                r.path.iter().rev().copied().collect()
            } else {
                continue;
            };
            let ahead = along[along.len().min(10) - 1];
            let (a, b) = (s(c), s(ahead));
            let len = length(b.0 - a.0, b.1 - a.1).max(0.001);
            leaving.push((i, ((b.0 - a.0) / len, (b.1 - a.1) / len)));
        }
        let mut joined: Vec<usize> = Vec::new();
        let mut best: Vec<(f32, usize, usize)> = Vec::new();
        for (x, (i, u)) in leaving.iter().enumerate() {
            for (j, v) in &leaving[x + 1..] {
                best.push((u.0 * v.0 + u.1 * v.1, *i, *j));
            }
        }
        best.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (dot, i, j) in best {
            // cos 135° ≈ -0.707
            if dot > -0.707 || joined.contains(&i) || joined.contains(&j) {
                continue;
            }
            joined.push(i);
            joined.push(j);
            let (ri, rj) = (root(&mut part, i), root(&mut part, j));
            part[ri] = rj;
        }
    }
    let mut chains: Vec<(usize, Vec<usize>)> = Vec::new();
    for i in 0..n {
        let r = root(&mut part, i);
        match chains.iter_mut().find(|(k, _)| *k == r) {
            Some((_, v)) => v.push(i),
            None => chains.push((r, vec![i])),
        }
    }
    ctx.chains = chains.into_iter().map(|(_, v)| v).collect();
}

#[cfg(test)]
mod tests {
    use crate::geom::Pos;
    use crate::overworld::{Knobs, Overworld, Water, WorldParams, WorldSize};

    fn worlds() -> Vec<(String, Overworld)> {
        let mut out = Vec::new();
        let mut add =
            |what: String, p: WorldParams| out.push((what, Overworld::generate(p).unwrap()));
        for seed in 0..40 {
            add(
                format!("seed {seed}"),
                WorldParams {
                    seed,
                    ..Default::default()
                },
            );
        }
        for size in [WorldSize::Small, WorldSize::Large] {
            for seed in 0..12 {
                add(
                    format!("{size:?} seed {seed}"),
                    WorldParams {
                        seed,
                        size,
                        ..Default::default()
                    },
                );
            }
        }
        // Once: a city on an islet or behind peaks, with no road to it.
        for seed in [132, 259, 265] {
            add(
                format!("small seed {seed}"),
                WorldParams {
                    seed,
                    size: WorldSize::Small,
                    ..Default::default()
                },
            );
        }
        for seed in [38, 1, 2, 3, 4, 5] {
            add(
                format!("two seas seed {seed}"),
                WorldParams {
                    seed,
                    knobs: Knobs {
                        sea_sides: Some(2),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );
        }
        out
    }

    #[test]
    fn every_city_is_reachable_by_road() {
        for (seed, w) in worlds() {
            let mut reached = vec![false; w.cities.len()];
            reached[0] = true;
            let at = |p: Pos| w.cities.iter().position(|c| c.centre == p);
            loop {
                let before = reached.iter().filter(|r| **r).count();
                for r in &w.routes {
                    let (a, b) = (at(r.path[0]), at(*r.path.last().unwrap()));
                    if let (Some(a), Some(b)) = (a, b)
                        && (reached[a] || reached[b])
                    {
                        reached[a] = true;
                        reached[b] = true;
                    }
                }
                if reached.iter().filter(|r| **r).count() == before {
                    break;
                }
            }
            assert!(reached.iter().all(|r| *r), "seed {seed}: {reached:?}");
        }
    }

    #[test]
    fn roads_are_unbroken_and_stay_off_the_sea() {
        for seed in 0..20 {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            for r in &w.routes {
                assert!(
                    r.path.windows(2).all(|p| p[0].manhattan(p[1]) == 1),
                    "seed {seed}"
                );
                assert!(
                    r.path
                        .iter()
                        .all(|p| w.tile(*p).unwrap().water != Water::Sea)
                );
                assert!(r.path.iter().all(|p| w.tile(*p).unwrap().road != 0));
            }
            let exits = w.routes.iter().filter(|r| {
                let e = *r.path.last().unwrap();
                e.x == 0 || e.y == 0 || e.x == w.size.w as i32 - 1 || e.y == w.size.h as i32 - 1
            });
            assert!(exits.count() >= 1, "seed {seed}: the world doesn't go on");
        }
    }
}

#[cfg(test)]
mod straightness {
    use crate::overworld::{Overworld, WorldParams};

    /// Tiles per straight run, over all routes of seeds 0..20.
    fn tiles_per_turn() -> f32 {
        let (mut tiles, mut turns) = (0usize, 0usize);
        for seed in 0..20 {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            for r in &w.routes {
                tiles += r.path.len();
                for k in 2..r.path.len() {
                    let (a, b, c) = (r.path[k - 2], r.path[k - 1], r.path[k]);
                    if (b.x - a.x, b.y - a.y) != (c.x - b.x, c.y - b.y) {
                        turns += 1;
                    }
                }
            }
        }
        tiles as f32 / turns.max(1) as f32
    }

    #[test]
    fn highways_run_long_and_straight() {
        // One level down every turn is a corner: a highway turns only where the land
        // asks, not in a staircase every tile or two.
        let t = tiles_per_turn();
        assert!(t >= 9.0, "a turn every {t:.1} tiles");
    }
}
