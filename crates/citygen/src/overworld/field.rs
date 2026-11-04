//! Screen-space geometry and noise for the overworld.
//!
//! A tile is drawn as one cell, twice as tall as it is wide, so shapes are measured in
//! s-space: `(x, 2y)`. A circle there is a circle on screen.
//!
//! Players share seeds, so a seed must give the same world on every platform. The
//! maths here uses only what IEEE 754 fixes to the bit (add, multiply, divide, `sqrt`,
//! rounding): `hypot`, `powf`, `sin` and `cos` come from each platform's own maths
//! library and may differ in the last bit, so the generator uses `length`, `sin`, `cos`
//! and the root powers below instead. `fingerprints_are_pinned` (in `mod.rs`) catches a
//! slip, on every platform CI runs.

use crate::geom::{Grid, Pos, Size};
use crate::rng::hash64;
use noise::{NoiseFn, OpenSimplex};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// √(x² + y²), the same on every platform (unlike `hypot`).
pub(crate) fn length(x: f32, y: f32) -> f32 {
    (x * x + y * y).sqrt()
}

/// sin from basic arithmetic: reduced to ±π/2, then a Taylor series to x¹⁵ in f64
/// (error under 1e-8), the same on every platform.
fn sin64(x: f64) -> f64 {
    use std::f64::consts::{FRAC_PI_2, PI, TAU};
    let mut r = x - TAU * (x / TAU).round();
    if r > FRAC_PI_2 {
        r = PI - r;
    } else if r < -FRAC_PI_2 {
        r = -PI - r;
    }
    let r2 = r * r;
    let (mut term, mut sum) = (r, r);
    for k in 1..8 {
        term *= -r2 / ((2 * k) as f64 * (2 * k + 1) as f64);
        sum += term;
    }
    sum
}

/// Portable sin.
pub(crate) fn sin(x: f32) -> f32 {
    sin64(x as f64) as f32
}

/// Portable cos.
pub(crate) fn cos(x: f32) -> f32 {
    sin64(x as f64 + std::f64::consts::FRAC_PI_2) as f32
}

/// x^1¼ from square roots.
pub(crate) fn pow_five_quarters(x: f32) -> f32 {
    x * x.sqrt().sqrt()
}

/// `p` in s-space.
pub(crate) fn s(p: Pos) -> (f32, f32) {
    (p.x as f32, 2.0 * p.y as f32)
}

/// The distance between two tiles as it looks on screen.
pub(crate) fn sdist(a: Pos, b: Pos) -> f32 {
    let (ax, ay) = s(a);
    let (bx, by) = s(b);
    length(ax - bx, ay - by)
}

/// The distance from `p` to the segment `a..b`, all in s-space.
pub(crate) fn seg_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    length(p.0 - a.0 - t * dx, p.1 - a.1 - t * dy)
}

/// The distance from `p` to a polyline, in s-space; infinite for an empty one.
pub(crate) fn poly_dist(p: (f32, f32), pts: &[(f32, f32)]) -> f32 {
    match pts {
        [] => f32::INFINITY,
        [a] => length(p.0 - a.0, p.1 - a.1),
        _ => pts
            .windows(2)
            .map(|w| seg_dist(p, w[0], w[1]))
            .fold(f32::INFINITY, f32::min),
    }
}

/// A tile's stable roll in 0..1 under `salt`.
pub(crate) fn roll(seed: u64, salt: u64, p: Pos) -> f32 {
    let packed = ((p.x as u32 as u64) << 32) | p.y as u32 as u64;
    (hash64(hash64(seed, salt), packed) >> 40) as f32 / (1u64 << 24) as f32
}

/// Fractal noise in 0..=1.
pub(crate) struct Fbm {
    simplex: OpenSimplex,
}

impl Fbm {
    pub fn new(seed: u64, salt: u32) -> Fbm {
        Fbm {
            simplex: OpenSimplex::new(hash64(seed, salt as u64) as u32),
        }
    }

    /// `octaves` of noise at `(x, y)`, each half the last's weight at twice its scale.
    pub fn at(&self, x: f32, y: f32, octaves: u32) -> f32 {
        let (mut t, mut amp, mut f, mut norm) = (0.0f64, 1.0f64, 1.0f64, 0.0f64);
        for _ in 0..octaves.max(1) {
            t += self.simplex.get([x as f64 * f, y as f64 * f]) * amp;
            norm += amp;
            amp *= 0.5;
            f *= 2.0;
        }
        ((t / norm * 0.5 + 0.5) as f32).clamp(0.0, 1.0)
    }
}

/// The on-screen distance from every tile to the nearest tile `from` accepts, out to
/// `cap` (farther tiles read `cap`): a chamfer walk, a column 1, a row 2, a diagonal √5.
pub(crate) fn distance_field(size: Size, from: impl Fn(Pos) -> bool, cap: f32) -> Grid<f32> {
    let mut d = Grid::filled(size, cap);
    // Distances in hundredths so the heap orders integers.
    let mut heap = BinaryHeap::new();
    for y in 0..size.h as i32 {
        for x in 0..size.w as i32 {
            let p = Pos::new(x, y);
            if from(p) {
                d[p] = 0.0;
                heap.push(Reverse((0u32, x, y)));
            }
        }
    }
    const STEPS: [(i32, i32, u32); 8] = [
        (1, 0, 100),
        (-1, 0, 100),
        (0, 1, 200),
        (0, -1, 200),
        (1, 1, 224),
        (1, -1, 224),
        (-1, 1, 224),
        (-1, -1, 224),
    ];
    let cap100 = (cap * 100.0) as u32;
    while let Some(Reverse((c, x, y))) = heap.pop() {
        let p = Pos::new(x, y);
        if c as f32 / 100.0 > d[p] {
            continue;
        }
        for (dx, dy, w) in STEPS {
            let q = Pos::new(x + dx, y + dy);
            let nc = c + w;
            if !size.contains(q) || nc >= cap100 {
                continue;
            }
            if (nc as f32 / 100.0) < d[q] {
                d[q] = nc as f32 / 100.0;
                heap.push(Reverse((nc, q.x, q.y)));
            }
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_trig_matches_the_platform_closely() {
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
        let mut x = -20.0f32;
        while x < 20.0 {
            assert!(
                (sin(x) - x.sin()).abs() < 1e-5,
                "sin {x}: {} vs {}",
                sin(x),
                x.sin()
            );
            assert!(
                (cos(x) - x.cos()).abs() < 1e-5,
                "cos {x}: {} vs {}",
                cos(x),
                x.cos()
            );
            x += 0.0137;
        }
    }

    #[test]
    fn length_is_the_square_root_of_the_squares() {
        assert_eq!(length(3.0, 4.0), 5.0);
        assert_eq!(length(0.0, 0.0), 0.0);
        assert_eq!(length(-6.0, 8.0), 10.0);
    }

    #[test]
    fn root_powers_are_sqrt_built() {
        assert_eq!(pow_five_quarters(0.0), 0.0);
        assert_eq!(pow_five_quarters(1.0), 1.0);
        assert_eq!(pow_five_quarters(16.0), 32.0);
    }
}
