//! Screen-space geometry and noise for the overworld.
//!
//! A tile is drawn as one cell, twice as tall as it is wide, so shapes are measured in
//! s-space: `(x, 2y)`. A circle there is a circle on screen.

use crate::geom::Pos;
use crate::rng::hash64;
use noise::{NoiseFn, OpenSimplex};

/// `p` in s-space.
pub(crate) fn s(p: Pos) -> (f32, f32) {
    (p.x as f32, 2.0 * p.y as f32)
}

/// The distance between two tiles as it looks on screen.
pub(crate) fn sdist(a: Pos, b: Pos) -> f32 {
    let (ax, ay) = s(a);
    let (bx, by) = s(b);
    (ax - bx).hypot(ay - by)
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
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

/// The distance from `p` to a polyline, in s-space; infinite for an empty one.
pub(crate) fn poly_dist(p: (f32, f32), pts: &[(f32, f32)]) -> f32 {
    match pts {
        [] => f32::INFINITY,
        [a] => (p.0 - a.0).hypot(p.1 - a.1),
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
