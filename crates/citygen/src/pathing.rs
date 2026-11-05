//! Grid search helpers shared by generation stages: flood fill, components, A*, BFS, Bresenham.

use crate::geom::{Dir, Grid, Pos, Rect, Size};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

/// All positions 4-connected to `start` through `passable`. Empty if `start` is not passable.
pub fn flood_fill(size: Size, start: Pos, passable: impl Fn(Pos) -> bool) -> Vec<Pos> {
    let mut seen = Grid::filled(size, false);
    let mut out = Vec::new();
    if !size.contains(start) || !passable(start) {
        return out;
    }
    let mut queue = VecDeque::from([start]);
    seen.set(start, true);
    while let Some(p) = queue.pop_front() {
        out.push(p);
        for n in p.neighbors4() {
            if size.contains(n) && !seen[n] && passable(n) {
                seen.set(n, true);
                queue.push_back(n);
            }
        }
    }
    out
}

/// Every 4-connected component of passable tiles, largest first (ties: earliest first cell).
pub fn components(size: Size, passable: impl Fn(Pos) -> bool) -> Vec<Vec<Pos>> {
    let mut seen = Grid::filled(size, false);
    let mut comps: Vec<Vec<Pos>> = Vec::new();
    for y in 0..size.h as i32 {
        for x in 0..size.w as i32 {
            let start = Pos::new(x, y);
            if seen[start] || !passable(start) {
                continue;
            }
            let mut comp = Vec::new();
            let mut queue = VecDeque::from([start]);
            seen.set(start, true);
            while let Some(p) = queue.pop_front() {
                comp.push(p);
                for n in p.neighbors4() {
                    if size.contains(n) && !seen[n] && passable(n) {
                        seen.set(n, true);
                        queue.push_back(n);
                    }
                }
            }
            comps.push(comp);
        }
    }
    comps.sort_by_key(|c| Reverse(c.len()));
    comps
}

/// 4-neighbour A*. `cost(from, to)` returns `None` for impassable steps. Path includes both ends.
pub fn astar(
    size: Size,
    start: Pos,
    goal: Pos,
    cost: impl Fn(Pos, Pos) -> Option<u32>,
) -> Option<Vec<Pos>> {
    if !size.contains(start) || !size.contains(goal) {
        return None;
    }
    let mut g = Grid::filled(size, u32::MAX);
    let mut came: Grid<Option<Pos>> = Grid::filled(size, None);
    let mut heap = BinaryHeap::new();
    let mut counter = 0u32;
    g.set(start, 0);
    heap.push(Reverse((start.manhattan(goal) as u32, counter, start)));
    while let Some(Reverse((_, _, p))) = heap.pop() {
        if p == goal {
            let mut path = vec![p];
            let mut cur = p;
            while let Some(prev) = came[cur] {
                path.push(prev);
                cur = prev;
            }
            path.reverse();
            return Some(path);
        }
        let gp = g[p];
        for n in p.neighbors4() {
            if !size.contains(n) {
                continue;
            }
            let Some(c) = cost(p, n) else { continue };
            let ng = gp.saturating_add(c);
            if ng < g[n] {
                g.set(n, ng);
                came.set(n, Some(p));
                counter += 1;
                heap.push(Reverse((ng + n.manhattan(goal) as u32, counter, n)));
            }
        }
    }
    None
}

/// Cheapest 4-path from `start` to the nearest `target` tile by `cost` (Dijkstra; of
/// equal costs, the first reached). `cost(from, to)` returns `None` for impassable steps,
/// a target's own step included. Path includes both ends; `start` is never its target.
pub fn cheapest_to(
    size: Size,
    start: Pos,
    cost: impl Fn(Pos, Pos) -> Option<u32>,
    target: impl Fn(Pos) -> bool,
) -> Option<Vec<Pos>> {
    if !size.contains(start) {
        return None;
    }
    let mut g = Grid::filled(size, u32::MAX);
    let mut came: Grid<Option<Pos>> = Grid::filled(size, None);
    let mut heap = BinaryHeap::new();
    let mut counter = 0u32;
    g.set(start, 0);
    heap.push(Reverse((0u32, counter, start)));
    while let Some(Reverse((d, _, p))) = heap.pop() {
        if d > g[p] {
            continue;
        }
        if p != start && target(p) {
            let mut path = vec![p];
            let mut cur = p;
            while let Some(prev) = came[cur] {
                path.push(prev);
                cur = prev;
            }
            path.reverse();
            return Some(path);
        }
        for n in p.neighbors4() {
            if !size.contains(n) {
                continue;
            }
            let Some(c) = cost(p, n) else { continue };
            let nd = d.saturating_add(c);
            if nd < g[n] {
                g.set(n, nd);
                came.set(n, Some(p));
                counter += 1;
                heap.push(Reverse((nd, counter, n)));
            }
        }
    }
    None
}

/// Shortest 4-path from `start` through `passable` tiles to the nearest `target` tile,
/// at most `max` steps long. The target tile itself need not be passable.
pub fn bfs_nearest(
    size: Size,
    start: Pos,
    passable: impl Fn(Pos) -> bool,
    target: impl Fn(Pos) -> bool,
    max: usize,
) -> Option<Vec<Pos>> {
    if !size.contains(start) {
        return None;
    }
    let mut came: Grid<Option<Pos>> = Grid::filled(size, None);
    let mut dist = Grid::filled(size, usize::MAX);
    let mut queue = VecDeque::from([start]);
    dist.set(start, 0);
    while let Some(p) = queue.pop_front() {
        let d = dist[p];
        if target(p) && p != start {
            let mut path = vec![p];
            let mut cur = p;
            while let Some(prev) = came[cur] {
                path.push(prev);
                cur = prev;
            }
            path.reverse();
            return Some(path);
        }
        if d >= max {
            continue;
        }
        for n in p.neighbors4() {
            if size.contains(n) && dist[n] == usize::MAX && (passable(n) || target(n)) {
                dist.set(n, d + 1);
                came.set(n, Some(p));
                queue.push_back(n);
            }
        }
    }
    None
}

/// Whether making the cells of `solid` stand in the way parts walkable cells of `window`
/// that were joined within it, four ways or eight. When it doesn't, no way anywhere is
/// cut: a way that crossed `solid` came into the window and went out again, and a way
/// round is left inside it. `solid` lies inside the window, off its rim.
pub fn cuts(window: Rect, walkable: impl Fn(Pos) -> bool, solid: &[Pos]) -> bool {
    let size = Size {
        w: window.w as u32,
        h: window.h as u32,
    };
    let at = |l: Pos| l.offset(window.x, window.y);
    let mut open = Grid::filled(size, false);
    for y in 0..window.h {
        for x in 0..window.w {
            let l = Pos::new(x, y);
            open.set(l, walkable(at(l)));
        }
    }
    let mut kept = open.clone();
    for p in solid {
        kept.set(Pos::new(p.x - window.x, p.y - window.y), false);
    }
    [false, true].into_iter().any(|eight| {
        let (before, n) = labels(&open, eight);
        let (after, _) = labels(&kept, eight);
        // each part before lies in one part after, or none
        let mut into: Vec<Option<u32>> = vec![None; n as usize];
        before.iter().zip(after.iter()).any(|((_, b), (_, a))| {
            let (Some(b), Some(a)) = (*b, *a) else {
                return false;
            };
            *into[b as usize].get_or_insert(a) != a
        })
    })
}

/// The part of `open` each open cell is in, joined four ways or `eight`, and how many
/// parts.
fn labels(open: &Grid<bool>, eight: bool) -> (Grid<Option<u32>>, u32) {
    let size = open.size();
    let mut part: Grid<Option<u32>> = Grid::filled(size, None);
    let mut n = 0;
    let mut stack = Vec::new();
    for (start, _) in open.iter().filter(|(_, o)| **o) {
        if part[start].is_some() {
            continue;
        }
        part.set(start, Some(n));
        stack.push(start);
        while let Some(p) = stack.pop() {
            let ways: &[Dir] = if eight { &Dir::ALL } else { &Dir::CARDINAL };
            for q in ways.iter().map(|d| p.step(*d)) {
                if open.get(q).copied().unwrap_or(false) && part[q].is_none() {
                    part.set(q, Some(n));
                    stack.push(q);
                }
            }
        }
        n += 1;
    }
    (part, n)
}

/// Integer line from `a` to `b`, both inclusive.
pub fn bresenham(a: Pos, b: Pos) -> Vec<Pos> {
    let mut out = Vec::new();
    let (mut x, mut y) = (a.x, a.y);
    let dx = (b.x - a.x).abs();
    let dy = -(b.y - a.y).abs();
    let sx = if a.x < b.x { 1 } else { -1 };
    let sy = if a.y < b.y { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        out.push(Pos::new(x, y));
        if x == b.x && y == b.y {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Pos, Size};

    #[test]
    fn astar_routes_around_block() {
        let size = Size { w: 5, h: 5 };
        let wall = |p: Pos| p.x == 2 && p.y != 4;
        let path = astar(size, Pos::new(0, 0), Pos::new(4, 0), |_, to| {
            if wall(to) { None } else { Some(1) }
        })
        .unwrap();
        assert_eq!(path[0], Pos::new(0, 0));
        assert_eq!(*path.last().unwrap(), Pos::new(4, 0));
        assert!(path.iter().all(|p| !wall(*p)));
        assert_eq!(path.len(), 13);
    }

    #[test]
    fn components_are_largest_first() {
        let size = Size { w: 6, h: 1 };
        let pass = |p: Pos| p.x != 2;
        let c = components(size, pass);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].len(), 3);
        assert_eq!(c[1].len(), 2);
        assert_eq!(flood_fill(size, Pos::new(0, 0), pass).len(), 2);
    }

    #[test]
    fn bfs_nearest_finds_target() {
        let size = Size { w: 10, h: 1 };
        let p = bfs_nearest(size, Pos::new(0, 0), |_| true, |p| p.x == 7, 20).unwrap();
        assert_eq!(p.len(), 8);
        assert!(bfs_nearest(size, Pos::new(0, 0), |_| true, |p| p.x == 7, 3).is_none());
    }

    #[test]
    fn cheapest_to_takes_the_cheap_way_to_the_nearest_target() {
        // two targets: (6, 0) straight along row 0, (0, 3) down column 0; row 0 costs 3
        // a step past x = 0, the rest 1, so the way down is the cheaper
        let size = Size { w: 8, h: 5 };
        let cost = |_: Pos, to: Pos| Some(if to.y == 0 { 3 } else { 1 });
        let target = |p: Pos| p == Pos::new(6, 0) || p == Pos::new(0, 3);
        let path = cheapest_to(size, Pos::new(0, 0), cost, target).unwrap();
        assert_eq!(
            path,
            vec![
                Pos::new(0, 0),
                Pos::new(0, 1),
                Pos::new(0, 2),
                Pos::new(0, 3)
            ]
        );
        // the start is never its own target, and a walled-off target is not found
        let wall = |_: Pos, to: Pos| (to.x < 3).then_some(1);
        assert_eq!(
            cheapest_to(size, Pos::new(0, 0), wall, |p| p.x == 0).map(|p| p.len()),
            Some(2)
        );
        assert!(cheapest_to(size, Pos::new(0, 0), wall, |p| p.x == 6).is_none());
    }

    #[test]
    fn a_post_in_the_open_cuts_nothing_and_a_bar_across_a_lane_does() {
        // a lane two rows wide between walls, x 0..12
        let lane = |p: Pos| (3..=4).contains(&p.y);
        let window = Rect::new(0, 0, 12, 8);
        assert!(!cuts(window, |_| true, &[Pos::new(5, 3), Pos::new(5, 4)]));
        assert!(!cuts(window, lane, &[Pos::new(5, 3)]), "one cell: round it");
        assert!(cuts(window, lane, &[Pos::new(5, 3), Pos::new(5, 4)]));
        // a pocket a sign shuts away is cut too
        let pocket = |p: Pos| lane(p) || p == Pos::new(6, 2);
        assert!(cuts(window, pocket, &[Pos::new(6, 3)]));
        // joined only corner to corner: a step is a step, eight ways
        let corner = |p: Pos| matches!((p.x, p.y), (2, 2) | (3, 3) | (4, 4));
        assert!(cuts(window, corner, &[Pos::new(3, 3)]));
        // a cell no longer walkable that nothing went through
        assert!(!cuts(window, lane, &[Pos::new(5, 6)]));
    }

    #[test]
    fn bresenham_endpoints() {
        let line = bresenham(Pos::new(0, 0), Pos::new(3, 1));
        assert_eq!(line.first(), Some(&Pos::new(0, 0)));
        assert_eq!(line.last(), Some(&Pos::new(3, 1)));
        assert_eq!(line.len(), 4);
    }
}
