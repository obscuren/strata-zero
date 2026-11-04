//! Packets: the traffic on the open Net's lanes, a look only. On every long straight
//! run of lane a packet runs, a white head with a fading tail, wrapping at the run's
//! end; runs are found whole, so a packet does not jump as the camera moves.

use citygen::{Pos, Rect};
use std::collections::{HashMap, HashSet};

/// The shortest run that carries a packet.
pub const MIN_RUN: usize = 8;
/// Ticks a packet takes per tile.
pub const PACKET_TICKS: u64 = 6;
/// How far a run is followed past the view to find its ends.
const REACH: i32 = 256;

/// A straight stretch of lane with no lane beside it across the run: a crossing or a
/// branch ends it.
fn straight(lane: &impl Fn(Pos) -> bool, p: Pos, horizontal: bool) -> bool {
    let (a, b) = if horizontal {
        (p.offset(0, -1), p.offset(0, 1))
    } else {
        (p.offset(-1, 0), p.offset(1, 0))
    };
    lane(p) && !lane(a) && !lane(b)
}

/// The straight runs of `MIN_RUN` or more tiles that touch `view`, each in order from
/// its top or left end.
pub fn runs(lane: impl Fn(Pos) -> bool, view: Rect) -> Vec<Vec<Pos>> {
    let mut out = Vec::new();
    for horizontal in [true, false] {
        let back = if horizontal { (-1, 0) } else { (0, -1) };
        let mut seen = HashSet::new();
        for p in view.iter() {
            if !straight(&lane, p, horizontal) {
                continue;
            }
            let mut start = p;
            for _ in 0..REACH {
                let q = start.offset(back.0, back.1);
                if !straight(&lane, q, horizontal) {
                    break;
                }
                start = q;
            }
            if !seen.insert(start) {
                continue;
            }
            let mut run = vec![start];
            let mut at = start;
            for _ in 0..REACH {
                let q = at.offset(-back.0, -back.1);
                if !straight(&lane, q, horizontal) {
                    break;
                }
                run.push(q);
                at = q;
            }
            if run.len() >= MIN_RUN {
                out.push(run);
            }
        }
    }
    out
}

/// Where a run's packet is at `tick`: an index into the run. Each run starts its
/// packet at its own offset and runs one way or the other, both from its first tile.
pub fn head(run: &[Pos], tick: u64) -> usize {
    let n = run.len();
    let first = run[0];
    let seed = (first.x as i64 * 7 + first.y as i64 * 13).unsigned_abs() as usize;
    let k = (tick / PACKET_TICKS) as usize + seed;
    if (first.x + first.y).rem_euclid(2) == 0 {
        k % n
    } else {
        n - 1 - k % n
    }
}

/// Each lane tile a packet lights, with how much: the head 0.6, its tail 0.3 and 0.13
/// behind it; `true` for the head.
pub fn glows(lane: impl Fn(Pos) -> bool, view: Rect, tick: u64) -> HashMap<Pos, (f32, bool)> {
    let mut out = HashMap::new();
    for run in runs(&lane, view) {
        let at = head(&run, tick);
        let forward = (run[0].x + run[0].y).rem_euclid(2) == 0;
        for (k, glow) in [(0usize, 0.6f32), (1, 0.3), (2, 0.13)] {
            let i = if forward {
                at.checked_sub(k)
            } else {
                Some(at + k).filter(|i| *i < run.len())
            };
            if let Some(i) = i {
                out.insert(run[i], (glow, k == 0));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lanes(tiles: &[(i32, i32)]) -> impl Fn(Pos) -> bool + '_ {
        move |p| tiles.contains(&(p.x, p.y))
    }

    #[test]
    fn runs_are_long_straight_lanes() {
        // A row of 10 at y 2, a row of 7 at y 6, a column of 12 at x 20 crossing
        // a row at y 10 (x 15..=25) in its middle.
        let mut t: Vec<(i32, i32)> = (0..10).map(|x| (x, 2)).collect();
        t.extend((0..7).map(|x| (x, 6)));
        t.extend((4..16).map(|y| (20, y)));
        t.extend((15..=25).filter(|x| *x != 20).map(|x| (x, 10)));
        let found = runs(lanes(&t), Rect::new(0, 0, 30, 20));
        let lens: Vec<usize> = found.iter().map(|r| r.len()).collect();
        assert!(
            found
                .iter()
                .any(|r| r[0] == Pos::new(0, 2) && r.len() == 10),
            "{lens:?}"
        );
        assert!(!found.iter().any(|r| r[0].y == 6), "seven is too short");
        // The crossing splits the column (4..=9 above: 6 tiles; 11..=15 below: 5), and the
        // row's halves are short too.
        assert!(
            !found.iter().any(|r| r[0].x == 20 || r[0].y == 10),
            "{found:?}"
        );
        // A run reaching past the view is found whole.
        let wide = runs(lanes(&t), Rect::new(5, 0, 2, 4));
        assert_eq!(wide.len(), 1);
        assert_eq!(wide[0].len(), 10);
    }

    #[test]
    fn packets_move_with_the_tick_and_wrap() {
        let run: Vec<Pos> = (0..10).map(|x| Pos::new(x, 2)).collect();
        let a = head(&run, 0);
        assert_eq!(head(&run, PACKET_TICKS - 1), a, "stable inside a step");
        let b = head(&run, PACKET_TICKS);
        assert_eq!(
            (a + 1) % 10,
            b,
            "one tile a step, forward on an even first tile"
        );
        let odd: Vec<Pos> = (0..10).map(|x| Pos::new(x, 3)).collect();
        let c = head(&odd, 0);
        assert_eq!((head(&odd, PACKET_TICKS) + 1) % 10, c, "back on an odd one");
        assert!((0..100).all(|t| head(&run, t * PACKET_TICKS) < 10));
    }

    #[test]
    fn a_packet_has_a_head_and_a_tail() {
        let t: Vec<(i32, i32)> = (0..12).map(|x| (x, 0)).collect();
        let g = glows(lanes(&t), Rect::new(0, 0, 12, 1), 0);
        assert_eq!(g.values().filter(|(_, h)| *h).count(), 1);
        assert!(
            (1..=3).contains(&g.len()),
            "the tail ends where the run does: {g:?}"
        );
    }
}
