//! Grid search helpers shared by generation stages: flood fill, components, A*, BFS, Bresenham.

use crate::geom::{Grid, Pos, Size};
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
    fn bresenham_endpoints() {
        let line = bresenham(Pos::new(0, 0), Pos::new(3, 1));
        assert_eq!(line.first(), Some(&Pos::new(0, 0)));
        assert_eq!(line.last(), Some(&Pos::new(3, 1)));
        assert_eq!(line.len(), 4);
    }
}
