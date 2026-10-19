//! What any item's drawing shares with the blueprint window: where its sockets sit
//! and how a pointer moves between places on it.

use game::SocketKind;
use ratatui::layout::Rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

/// A socket on a drawing: the rectangle the pointer rests on, in drawing coordinates
/// (x may fall left of the drawing), its name and what it takes.
#[allow(dead_code)] // until the modding mode reads it
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SocketPlace {
    /// Its place in the item's sockets.
    pub index: usize,
    pub x: i32,
    pub y: i32,
    pub w: u16,
    pub h: u16,
    pub name: &'static str,
    pub kind: SocketKind,
}

/// The place nearest `from` in `dir` that `allow` lets the pointer rest on, or
/// `from` when there is none. Distances run in square units (a cell is twice as
/// tall as wide). A place in the 45 degree cone wins by `primary + perp`; only when
/// that cone holds none does a wider one (`perp <= 2 * primary`) count, by
/// `primary + 2 * perp`. Ties go to the earlier place.
pub fn step<P: Copy + PartialEq>(
    parts: &[(P, Rect)],
    from: P,
    dir: Dir,
    allow: impl Fn(P) -> bool,
) -> P {
    let centre = |r: &Rect| {
        (
            f32::from(r.x) + f32::from(r.width) / 2.0,
            2.0 * (f32::from(r.y) + f32::from(r.height) / 2.0),
        )
    };
    let Some((_, here)) = parts.iter().find(|(p, _)| *p == from) else {
        return from;
    };
    let (fx, fy) = centre(here);
    let mut tight: Option<(f32, P)> = None;
    let mut wide: Option<(f32, P)> = None;
    for (p, r) in parts {
        if *p == from || !allow(*p) {
            continue;
        }
        let (cx, cy) = centre(r);
        let (dx, dy) = (cx - fx, cy - fy);
        let (primary, perp) = match dir {
            Dir::Right => (dx, dy.abs()),
            Dir::Left => (-dx, dy.abs()),
            Dir::Down => (dy, dx.abs()),
            Dir::Up => (-dy, dx.abs()),
        };
        if primary <= 0.0 {
            continue;
        }
        let (tier, score) = if perp <= primary {
            (&mut tight, primary + perp)
        } else if perp <= 2.0 * primary {
            (&mut wide, primary + 2.0 * perp)
        } else {
            continue;
        };
        if tier.is_none_or(|(best, _)| score < best) {
            *tier = Some((score, *p));
        }
    }
    tight.or(wide).map_or(from, |(_, p)| p)
}
