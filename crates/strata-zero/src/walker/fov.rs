//! Symmetric shadowcasting field of view.
//!
//! Albert Ford's algorithm: each quadrant is scanned row by row between two rational
//! slopes; a floor tile is visible exactly when its centre lies inside the view
//! segment, which makes visibility symmetric between any two floor tiles.

use citygen::Pos;

/// A rational slope `num / den` with `den > 0`.
#[derive(Clone, Copy, Debug)]
struct Frac {
    num: i64,
    den: i64,
}

impl Frac {
    const fn new(num: i64, den: i64) -> Self {
        Frac { num, den }
    }
}

#[derive(Clone, Copy)]
enum Quadrant {
    N,
    E,
    S,
    W,
}

impl Quadrant {
    fn transform(self, origin: Pos, depth: i64, col: i64) -> Pos {
        let (d, c) = (depth as i32, col as i32);
        match self {
            Quadrant::N => Pos::new(origin.x + c, origin.y - d),
            Quadrant::S => Pos::new(origin.x + c, origin.y + d),
            Quadrant::E => Pos::new(origin.x + d, origin.y + c),
            Quadrant::W => Pos::new(origin.x - d, origin.y + c),
        }
    }
}

/// `floor(depth * f + 1/2)`.
fn round_ties_up(depth: i64, f: Frac) -> i64 {
    (2 * depth * f.num + f.den).div_euclid(2 * f.den)
}

/// `ceil(depth * f - 1/2)`.
fn round_ties_down(depth: i64, f: Frac) -> i64 {
    let n = 2 * depth * f.num - f.den;
    let d = 2 * f.den;
    -((-n).div_euclid(d))
}

fn slope(depth: i64, col: i64) -> Frac {
    Frac::new(2 * col - 1, 2 * depth)
}

/// `col >= depth * start && col <= depth * end`.
fn is_symmetric(depth: i64, col: i64, start: Frac, end: Frac) -> bool {
    col * start.den >= depth * start.num && col * end.den <= depth * end.num
}

/// Marks every tile visible from `origin` within `radius`. `blocks` says whether a
/// tile stops sight; out-of-bounds tiles (per `in_bounds`) count as walls and are
/// never marked.
pub fn compute(
    origin: Pos,
    radius: i32,
    blocks: impl Fn(Pos) -> bool,
    in_bounds: impl Fn(Pos) -> bool,
    mut mark: impl FnMut(Pos),
) {
    mark(origin);
    let r2 = (radius as i64) * (radius as i64);
    for q in [Quadrant::N, Quadrant::E, Quadrant::S, Quadrant::W] {
        let mut stack: Vec<(i64, Frac, Frac)> = vec![(1, Frac::new(-1, 1), Frac::new(1, 1))];
        while let Some((depth, mut start, end)) = stack.pop() {
            if depth > radius as i64 {
                continue;
            }
            let min_col = round_ties_up(depth, start);
            let max_col = round_ties_down(depth, end);
            let mut prev: Option<bool> = None; // Some(true) = wall, Some(false) = floor
            for col in min_col..=max_col {
                let p = q.transform(origin, depth, col);
                let inside = in_bounds(p);
                let wall = !inside || blocks(p);
                let within = depth * depth + col * col <= r2;
                if inside && within && (wall || is_symmetric(depth, col, start, end)) {
                    mark(p);
                }
                if prev == Some(true) && !wall {
                    start = slope(depth, col);
                }
                if prev == Some(false) && wall {
                    stack.push((depth + 1, start, slope(depth, col)));
                }
                prev = Some(wall);
            }
            if prev == Some(false) {
                stack.push((depth + 1, start, end));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::Pos;
    use std::collections::HashSet;

    fn see(from: Pos, radius: i32) -> HashSet<Pos> {
        // 9x9 room, wall column at x=4 rows 0..=6, opening at (4,7),(4,8).
        let wall = |p: Pos| p.x == 4 && p.y <= 6;
        let bounds = |p: Pos| (0..9).contains(&p.x) && (0..9).contains(&p.y);
        let mut v = HashSet::new();
        compute(from, radius, wall, bounds, |p| {
            v.insert(p);
        });
        v
    }

    #[test]
    fn fov_symmetric_and_walls_block() {
        let a = Pos::new(1, 1);
        let b = Pos::new(7, 1);
        assert_eq!(see(a, 12).contains(&b), see(b, 12).contains(&a));
        assert!(!see(a, 12).contains(&b));
        assert!(see(a, 12).contains(&Pos::new(4, 1)));
        assert!(see(a, 12).contains(&a));
        assert!(see(Pos::new(1, 8), 12).contains(&Pos::new(7, 8)));
        assert!(!see(a, 12).contains(&Pos::new(1, 14)));
        assert!(!see(a, 2).contains(&Pos::new(1, 5)));
        assert!(see(a, 2).contains(&Pos::new(1, 3)));
        // Every floor tile on the open side is visible from the middle of the room.
        let c = Pos::new(2, 4);
        for y in 0..9 {
            for x in 0..4 {
                assert!(see(c, 12).contains(&Pos::new(x, y)), "({x},{y}) hidden");
            }
        }
        // Symmetry holds for every pair of floor tiles.
        let floors: Vec<Pos> = (0..9)
            .flat_map(|y| (0..9).map(move |x| Pos::new(x, y)))
            .filter(|p| !(p.x == 4 && p.y <= 6))
            .collect();
        for p in &floors {
            let sp = see(*p, 20);
            for q in &floors {
                assert_eq!(sp.contains(q), see(*q, 20).contains(p), "{p:?} {q:?}");
            }
        }
    }
}
