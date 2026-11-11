//! The set pieces of fixed shape: a platform and its track along the far side, a gate
//! line across the first border, a ramp in a car park.

use super::{Stage, moved, plain};
use crate::geom::Pos;
use crate::interior::RoomKind;
use crate::interior::venue::{PieceDef, PlacedPiece, PlannedDoor, Spine};
use rand::Rng;

/// A platform's rows, and its track's behind it against the far wall.
const PLATFORM: i32 = 3;
const TRACK: i32 = 2;
/// The most doors a gate line takes.
const GATES: i32 = 6;

impl Stage<'_> {
    /// Whether the last band holds a platform and its track at the far side, behind a
    /// wall on no doorstep's row, with a row of its spine left in front.
    pub(super) fn track_fits(&self) -> bool {
        let depth = self.axes.depth();
        let (u, e) = self.rows[self.bands() - 1];
        let wall = depth - PLATFORM - TRACK - 1;
        e == depth && wall > u && !self.reserved.iter().any(|p| self.axes.deep(*p) == wall)
    }

    /// Runs the last band's spine straight to the far side when a track needs it there,
    /// or when a bent's turn would stand a wall on a doorstep.
    pub(super) fn straighten(&mut self, track: bool) {
        let last = self.bands() - 1;
        let ring = self.ring_in(last);
        let bent = self.x.spine == Spine::Bent;
        let (band, turn) = (self.x.bands[last].rect, self.x.rooms[last].rect);
        let touched = bent
            && self
                .reserved
                .iter()
                .any(|p| band.contains(*p) && !turn.contains(*p));
        if (track && (bent || ring)) || touched {
            self.x.rooms[last].rect = self.axes.rect(self.rows[last], self.axes.line());
            if ring {
                self.x.core = None;
            }
            self.x.spine = Spine::Straight;
        }
    }

    /// A platform 3 deep along the far side, the track 2 deep behind it against the far
    /// wall, both the level's width; the last band's spine room ends at the platform's
    /// wall, with a door onto it, and the track opens off the platform's edge.
    pub(super) fn track(&mut self, p: &PieceDef) {
        if !self.track_fits() {
            return;
        }
        let last = self.bands() - 1;
        let (depth, width) = (self.axes.depth(), self.axes.width());
        let (a, c) = self.axes.span(self.x.rooms[last].rect);
        if a.1 != depth {
            return;
        }
        let rail = depth - TRACK;
        let wall = rail - PLATFORM - 1;
        self.x.rooms[last].rect = self.axes.rect((a.0, wall), c);
        self.rows[last].1 = wall;
        let zone = self.x.bands[last].zone;
        let edge = self.axes.rect((wall + 1, rail), (0, width));
        let platform = self.push(plain(edge, RoomKind::Platform, zone));
        let rails = self.axes.rect((rail, depth), (0, width));
        let track = self.push(plain(rails, RoomKind::Track, zone));
        let col = self.axes.line().0.clamp(c.0, c.1 - 1);
        self.join(self.axes.pos(wall, col), (last, platform), false);
        self.join(self.axes.pos(rail, col), (platform, track), true);
        self.x.pieces.push(PlacedPiece {
            name: p.name.clone(),
            kind: p.kind,
            rect: rails,
            ends: vec![self.axes.pos(rail, -1), self.axes.pos(rail, width)],
        });
    }

    /// The spine's door on the first cut becomes a line of doors a tile apart, as many as
    /// fit up to six, each with the border's lock and its own number. A straight piece of
    /// spine on either side widens to the line; a hall, a ring's lane and a bent's turn
    /// keep their own tiles, and the line stays within them.
    pub(super) fn gate_line(&mut self, p: &PieceDef) {
        if self.bands() < 2 {
            return;
        }
        let cut = self.rows[1].0 - 1;
        let Some(k) = (0..self.x.doors.len())
            .find(|&k| !self.loops[k] && self.axes.deep(self.x.doors[k].at) == cut)
        else {
            return;
        };
        let line = self.axes.line();
        let (mut lo, mut hi) = (0, self.axes.width());
        for i in [0, 1] {
            if !self.widens(i) {
                let (_, c) = self.axes.span(self.x.rooms[i].rect);
                (lo, hi) = (lo.max(c.0), hi.min(c.1));
            }
        }
        if !(lo..hi).contains(&line.0) {
            return;
        }
        let count = ((hi - lo + 1) / 2).min(GATES);
        let len = 2 * count - 1;
        let start = (line.0 + 1 - count).clamp(lo.max(line.0 + 1 - len), (hi - len).min(line.0));
        let span = (start, start + len);
        for i in [0, 1] {
            if self.widens(i) {
                let (a, c) = self.axes.span(self.x.rooms[i].rect);
                let c = (c.0.min(span.0), c.1.max(span.1));
                self.x.rooms[i].rect = self.axes.rect(a, c);
            }
        }
        let old = self.x.doors[k];
        let gates: Vec<PlannedDoor> = (0..count)
            .map(|j| moved(self.axes, &old, start + 2 * j, old.rooms))
            .collect();
        self.x.doors.splice(k..=k, gates);
        self.loops
            .splice(k..=k, std::iter::repeat_n(false, count as usize));
        self.x.pieces.push(PlacedPiece {
            name: p.name.clone(),
            kind: p.kind,
            rect: self.axes.rect((cut, cut + 1), span),
            ends: Vec::new(),
        });
    }

    /// A ramp: a strip 2 along the frame wall and 4 out from it in a car park on the wall,
    /// else 4 along and 2 out, clear of doorsteps and doors. None without a car park on
    /// the frame wall with room for one.
    pub(super) fn ramp(&mut self, p: &PieceDef) {
        let (depth, width) = (self.axes.depth(), self.axes.width());
        let near = self.near_doors();
        let taken = |q: Pos| {
            self.reserved.contains(&q)
                || near.contains(&q)
                || self.x.pieces.iter().any(|x| x.rect.contains(q))
        };
        for (l, o) in [(2, 4), (4, 2)] {
            let mut strips = Vec::new();
            for r in self.x.rooms.iter().filter(|r| r.kind == RoomKind::CarPark) {
                let (ra, rc) = self.axes.span(r.rect);
                // Each of its sides on the frame wall: tiles along it, rows out from it.
                let walls = [
                    (ra.0 == 0, rc, ra.1 - ra.0),
                    (ra.1 == depth, rc, ra.1 - ra.0),
                    (rc.0 == 0, ra, rc.1 - rc.0),
                    (rc.1 == width, ra, rc.1 - rc.0),
                ];
                for (side, (on, along, deep)) in walls.into_iter().enumerate() {
                    if !on || deep <= o {
                        continue;
                    }
                    for s in along.0..=along.1 - l {
                        let (a, c) = match side {
                            0 => ((ra.0, ra.0 + o), (s, s + l)),
                            1 => ((ra.1 - o, ra.1), (s, s + l)),
                            2 => ((s, s + l), (rc.0, rc.0 + o)),
                            _ => ((s, s + l), (rc.1 - o, rc.1)),
                        };
                        let strip = self.axes.rect(a, c);
                        if strip.iter().all(|q| !taken(q)) {
                            strips.push(strip);
                        }
                    }
                }
            }
            if !strips.is_empty() {
                let rect = strips[self.rng.random_range(0..strips.len())];
                self.x.pieces.push(PlacedPiece {
                    name: p.name.clone(),
                    kind: p.kind,
                    rect,
                    ends: Vec::new(),
                });
                return;
            }
        }
    }
}
