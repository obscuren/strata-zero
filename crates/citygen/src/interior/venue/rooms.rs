//! The rooms stage: each band filled with its type's rooms along the spine, and the
//! level's set pieces: a platform and its track along the far side, a gate line of
//! turnstiles across the first border, a ramp in a car park.
//!
//! A room is its def's width across the level and its height front to back, within the
//! def's range, or turned when only that fits; a fill room takes what the listed rooms
//! leave, never under 3 x 3. Rooms pack along the walls of the band's spine room, each
//! with its door on the wall, and the floor they leave stays wall. A stretch along a spine
//! wall with a doorstep in it or on its walls is a vestibule instead: a room of the
//! corridor out to the frame, open to the spine across the wall, so no wall stands on a
//! doorstep or beside one.

mod pieces;
#[cfg(test)]
mod tests;

use super::zones::{self, Axes};
use super::{
    LOCK_NUMBERS, LevelDef, LevelLayout, PieceDef, PieceKind, PlannedDoor, PlannedRoom, RoomDef,
    Spine, VenueLayout, VenueType, room_number,
};
use crate::geom::{Dir, Pos, Rect};
use crate::interior::{DoorLock, RoomKind};
use crate::rng::hash64;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::BTreeSet;

/// The rooms stage's rng, from the layout seed.
pub(super) const STAGE: u64 = 0x3_0035;

/// The least a room runs along its wall and out from it: a fill room's least, and the
/// sliver a room takes rather than leave.
const LEAST: i32 = 3;

/// Fills every level's bands with rooms and lays its set pieces. Each level draws from
/// its own rng, so two levels of one role differ.
pub(super) fn lay(
    t: &VenueType,
    layout: &mut VenueLayout,
    rng: &mut ChaCha8Rng,
) -> Result<(), String> {
    let front = zones::front(&layout.frame);
    let seed: u64 = rng.random();
    for x in &mut layout.levels {
        let mut rng = ChaCha8Rng::seed_from_u64(hash64(seed, x.level as i64 as u64));
        level(t, front, x, &mut rng).map_err(|e| format!("{}: level {}: {e}", t.id, x.level))?;
    }
    Ok(())
}

/// One level: a hall's bands become their rooms, the track and the gate line shape the
/// spine, the bands fill with rooms, a ramp goes in a car park, and the extra doors of a
/// border find rooms to join. Then the locks are numbered and the spots picked.
fn level(
    t: &VenueType,
    front: Dir,
    x: &mut LevelLayout,
    rng: &mut ChaCha8Rng,
) -> Result<(), String> {
    let role = t
        .role(&x.role)
        .ok_or_else(|| format!("no level role `{}`", x.role))?;
    let pieces = role
        .pieces
        .iter()
        .map(|p| {
            t.pieces
                .iter()
                .find(|d| &d.name == p)
                .ok_or_else(|| format!("no piece `{p}`"))
        })
        .collect::<Result<Vec<&PieceDef>, String>>()?;
    let mut s = Stage::new(t, role, front, x, rng);
    s.halls()?;
    let track = pieces.iter().any(|p| p.kind == PieceKind::Track) && s.track_fits();
    s.straighten(track);
    for p in &pieces {
        match p.kind {
            PieceKind::Track => s.track(p),
            PieceKind::GateLine => s.gate_line(p),
            PieceKind::Ramp => {}
        }
    }
    for i in 0..s.x.bands.len() {
        s.band(i)?;
    }
    // A ramp waits for its car park.
    for p in pieces.iter().filter(|p| p.kind == PieceKind::Ramp) {
        s.ramp(p);
    }
    s.loops();
    s.renumber();
    s.spots();
    Ok(())
}

/// Which side of a lot the spine room lies on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    /// The spine is across from it, to its right.
    Left,
    /// The spine is across from it, to its left.
    Right,
    /// The spine is in front: a bent's turn.
    Behind,
}

/// Floor beside a band's spine room, packed with rooms along the wall between them.
#[derive(Clone, Debug)]
struct Lot {
    side: Side,
    /// Its rows deep and tiles across.
    a: (i32, i32),
    c: (i32, i32),
    /// The stretch along the wall with the spine room beyond it: all of it, or the end of
    /// a bent's turn.
    touch: (i32, i32),
    /// Counted from its far end: the end at a border's extra door.
    rev: bool,
    /// Its first room stands at a border's extra door, so it waits for one anyone may
    /// enter.
    waits: bool,
    /// The stretches along the wall still free.
    free: Vec<(i32, i32)>,
}

impl Lot {
    fn new(side: Side, a: (i32, i32), c: (i32, i32), touch: Option<(i32, i32)>) -> Lot {
        let mut lot = Lot {
            side,
            a,
            c,
            touch: (0, 0),
            rev: false,
            waits: false,
            free: Vec::new(),
        };
        lot.touch = touch.unwrap_or((0, lot.len()));
        lot
    }

    /// How far it runs along the wall.
    fn len(&self) -> i32 {
        match self.side {
            Side::Behind => self.c.1 - self.c.0,
            _ => self.a.1 - self.a.0,
        }
    }

    /// How far it runs out from the wall.
    fn out(&self) -> i32 {
        match self.side {
            Side::Behind => self.a.1 - self.a.0,
            _ => self.c.1 - self.c.0,
        }
    }

    /// Only part of its wall has the spine beyond it: it holds one room, at that end.
    fn single(&self) -> bool {
        self.touch != (0, self.len())
    }

    /// The rows deep and tiles across of the tile `s` along it and `t` out from the wall,
    /// the wall at `t` -1 and the spine room beyond it at -2.
    fn at(&self, s: i32, t: i32) -> (i32, i32) {
        let s = if self.rev { self.len() - 1 - s } else { s };
        match self.side {
            Side::Left => (self.a.0 + s, self.c.1 - 1 - t),
            Side::Right => (self.a.0 + s, self.c.0 + t),
            Side::Behind => (self.a.0 + t, self.c.0 + s),
        }
    }

    /// The rows and tiles across of `s` along it and `t` out.
    fn span(&self, s: (i32, i32), t: (i32, i32)) -> ((i32, i32), (i32, i32)) {
        let (p, q) = (self.at(s.0, t.0), self.at(s.1 - 1, t.1 - 1));
        (
            (p.0.min(q.0), p.0.max(q.0) + 1),
            (p.1.min(q.1), p.1.max(q.1) + 1),
        )
    }

    fn touches(&self, s: i32) -> bool {
        let s = if self.rev { self.len() - 1 - s } else { s };
        (self.touch.0..self.touch.1).contains(&s)
    }

    /// A def's least and most along the wall, then out from it, its own way round: its
    /// height along a side's wall, which runs front to back, and its width along a bent's
    /// turn.
    fn ranges(&self, d: &RoomDef) -> ((i32, i32), (i32, i32)) {
        let ((w0, h0), (w1, h1)) = d.size;
        match self.side {
            Side::Behind => ((w0, w1), (h0, h1)),
            _ => ((h0, h1), (w0, w1)),
        }
    }

    fn room_left(&self) -> i32 {
        self.free.iter().map(|f| f.1 - f.0).sum()
    }
}

/// Where a room goes in a lot: which free stretch, how far along and out it runs, and how
/// far along the wall its door stands.
struct Fit {
    stretch: usize,
    along: i32,
    out: i32,
    door: i32,
}

/// A room no def gives: a vestibule, a platform, a track.
fn plain(rect: Rect, kind: RoomKind, zone: u8) -> PlannedRoom {
    PlannedRoom {
        rect,
        kind,
        name: kind.label().to_string(),
        zone,
        number: 0,
        private: false,
        furniture: Vec::new(),
        stations: Vec::new(),
        spots: Vec::new(),
    }
}

/// Door `d` moved along its cut to `c` tiles across and joined to `rooms`, its lock with
/// it.
fn moved(axes: Axes, d: &PlannedDoor, c: i32, rooms: (usize, usize)) -> PlannedDoor {
    let at = axes.pos(axes.deep(d.at), c);
    let fit = d.fit.map(|f| DoorLock {
        at,
        free_from: f.free_from.map(|q| axes.pos(axes.deep(q), c)),
        ..f
    });
    PlannedDoor {
        at,
        rooms,
        lock: d.lock.and(fit),
        fit,
        ..*d
    }
}

/// One level's rooms stage in progress.
struct Stage<'a> {
    t: &'a VenueType,
    role: &'a LevelDef,
    axes: Axes,
    x: &'a mut LevelLayout,
    rng: &'a mut ChaCha8Rng,
    reserved: BTreeSet<Pos>,
    /// Each band's floor rows: behind its front wall, and in front of a platform's.
    rows: Vec<(i32, i32)>,
    /// Beside each door: whether it is one of a border's extra doors, which find their
    /// rooms once the rooms are in.
    loops: Vec<bool>,
    /// The room numbers given on the level so far.
    numbered: u16,
}

impl<'a> Stage<'a> {
    fn new(
        t: &'a VenueType,
        role: &'a LevelDef,
        front: Dir,
        x: &'a mut LevelLayout,
        rng: &'a mut ChaCha8Rng,
    ) -> Stage<'a> {
        let axes = Axes::new(x.size, front);
        let rows = x
            .bands
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let (a, _) = axes.span(b.rect);
                (a.0 + i32::from(i > 0), a.1)
            })
            .collect();
        // The zones stage puts a cut's spine door first, then its extra doors.
        let mut cuts = BTreeSet::new();
        let loops = x
            .doors
            .iter()
            .map(|d| !cuts.insert(axes.deep(d.at)))
            .collect();
        x.pieces.clear();
        Stage {
            t,
            role,
            axes,
            reserved: x.reserved.iter().copied().collect(),
            x,
            rng,
            rows,
            loops,
            numbered: 0,
        }
    }

    fn bands(&self) -> usize {
        self.x.bands.len()
    }

    fn def(&self, name: &str) -> Result<&'a RoomDef, String> {
        self.t.room(name).ok_or_else(|| format!("no room `{name}`"))
    }

    fn pos(&self, (a, c): (i32, i32)) -> Pos {
        self.axes.pos(a, c)
    }

    fn push(&mut self, r: PlannedRoom) -> usize {
        self.x.rooms.push(r);
        self.x.rooms.len() - 1
    }

    fn join(&mut self, at: Pos, rooms: (usize, usize), open: bool) {
        self.x.doors.push(PlannedDoor {
            at,
            rooms,
            lock: None,
            fit: None,
            border: false,
            open,
        });
        self.loops.push(false);
    }

    /// A room of def `d`. A private def's room, a hotel room, a flat and an office take
    /// the level's next number.
    fn of_def(&mut self, rect: Rect, d: &RoomDef, zone: u8) -> PlannedRoom {
        let numbered = d.private
            || matches!(
                d.kind,
                RoomKind::HotelRoom | RoomKind::Flat | RoomKind::Office
            );
        let number = if numbered {
            self.numbered += 1;
            room_number(self.x.level, self.numbered)
        } else {
            0
        };
        PlannedRoom {
            rect,
            kind: d.kind,
            name: d.name.clone(),
            zone,
            number,
            private: d.private,
            furniture: d.furniture.clone(),
            stations: d.stations.clone(),
            spots: Vec::new(),
        }
    }

    fn ring_in(&self, i: usize) -> bool {
        self.x
            .core
            .is_some_and(|c| self.x.rooms[i].rect.intersects(c))
    }

    /// Whether band `i`'s spine room is a straight piece of corridor, which a gate line
    /// can widen.
    fn widens(&self, i: usize) -> bool {
        self.x.spine != Spine::Hall
            && !self.ring_in(i)
            && !(self.x.spine == Spine::Bent && i == self.bands() - 1)
    }

    fn room_at(&self, p: Pos) -> Option<usize> {
        self.x.rooms.iter().position(|r| r.rect.contains(p))
    }

    /// Door tiles and the tiles beside them.
    fn near_doors(&self) -> BTreeSet<Pos> {
        self.x
            .doors
            .iter()
            .flat_map(|d| std::iter::once(d.at).chain(d.at.neighbors4()))
            .collect()
    }

    /// On a hall, each band is one room of its first listed def (its fill when it lists
    /// none). It is the band's way through, so it is never locked.
    fn halls(&mut self) -> Result<(), String> {
        if self.x.spine != Spine::Hall {
            return Ok(());
        }
        for i in 0..self.bands() {
            let band = &self.role.bands[i];
            let Some(name) = band.rooms.first().or(band.fill.as_ref()) else {
                continue;
            };
            let d = self.def(name)?;
            let (rect, zone) = (self.x.rooms[i].rect, self.x.rooms[i].zone);
            let mut room = self.of_def(rect, d, zone);
            room.private = false;
            self.x.rooms[i] = room;
        }
        Ok(())
    }

    /// The lots beside band `i`'s spine room: one each side of a straight piece or a ring;
    /// beside a bent's turn, one off its end and one behind it.
    fn lots(&self, i: usize) -> Vec<Lot> {
        let (u, e) = self.rows[i];
        let width = self.axes.width();
        let (a, c) = self.axes.span(self.x.rooms[i].rect);
        let mut v = Vec::new();
        if e <= u || self.x.spine == Spine::Hall {
            return v;
        }
        // A bent's turn runs from the spine to one side: the lot off its end touches it
        // only beside the turn's rows. A wall on a side with no floor beyond it is a lot
        // nothing out from its wall, so a doorstep on that wall still opens it.
        let turn = self.x.spine == Spine::Bent && i == self.bands() - 1;
        let touch = turn.then_some((0, a.1 - u));
        if c.0 > 0 {
            v.push(Lot::new(Side::Left, (u, e), (0, c.0 - 1), touch));
        }
        if c.1 < width {
            v.push(Lot::new(Side::Right, (u, e), (c.1 + 1, width), touch));
        }
        if turn && a.1 + 1 < e {
            v.push(Lot::new(Side::Behind, (a.1 + 1, e), c, None));
        }
        v
    }

    /// Marks the lots at a border's extra door: the lot on its side in the band behind
    /// waits at its front, and the one in the band in front is packed from its back.
    fn mark_loops(&self, i: usize, lots: &mut [Lot]) {
        let line = self.axes.line();
        let (front, back) = (self.rows[i].0 - 1, self.rows[i].1);
        let doors: Vec<(i32, Side)> = (0..self.x.doors.len())
            .filter(|&k| self.loops[k])
            .map(|k| {
                let at = self.x.doors[k].at;
                let side = if self.axes.across(at) < line.0 {
                    Side::Left
                } else {
                    Side::Right
                };
                (self.axes.deep(at), side)
            })
            .collect();
        for (row, side) in &doors {
            for lot in lots.iter_mut().filter(|l| l.side == *side) {
                if i > 0 && *row == front {
                    lot.waits = true;
                }
            }
        }
        for (row, side) in &doors {
            for lot in lots.iter_mut().filter(|l| l.side == *side) {
                if i + 1 < self.bands() && *row == back && !lot.waits && !lot.single() {
                    (lot.waits, lot.rev) = (true, true);
                }
            }
        }
    }

    /// The vestibules of a lot: each stretch along it with a doorstep, or beside one, in
    /// it or on its walls, grown over any stretch too short for a room, becomes an open
    /// room of the corridor running out to the frame and over the spine's wall. What's
    /// left between them is free for rooms, behind a wall each side.
    fn vestibules(&mut self, i: usize, lot: &mut Lot) {
        let (len, out) = (lot.len(), lot.out());
        let blocked = |s: i32| (-1..out).any(|t| self.reserved.contains(&self.pos(lot.at(s, t))));
        let mut runs: Vec<(i32, i32)> = Vec::new();
        for s in (0..len).filter(|s| blocked(*s)) {
            match runs.last_mut() {
                // Two walls and less than a room between them: one vestibule.
                Some(r) if s - r.1 - 2 < LEAST => r.1 = s + 1,
                _ => runs.push((s, s + 1)),
            }
        }
        if let Some(f) = runs.first_mut()
            && f.0 - 1 < LEAST
        {
            f.0 = 0;
        }
        if let Some(l) = runs.last_mut()
            && len - l.1 - 1 < LEAST
        {
            l.1 = len;
        }
        let mut next = 0;
        for &(v0, v1) in &runs {
            if v0 - 1 > next {
                lot.free.push((next, v0 - 1));
            }
            next = v1 + 1;
        }
        if next < len {
            lot.free.push((next, len));
        }
        let zone = self.x.bands[i].zone;
        for (v0, v1) in runs {
            let (a, c) = lot.span((v0, v1), (-1, out));
            let room = self.push(plain(self.axes.rect(a, c), RoomKind::Corridor, zone));
            let at = self.pos(lot.at((v0 + v1) / 2, -1));
            self.join(at, (i, room), true);
        }
        // A vestibule at the end the extra door is at lets anyone in already.
        if lot.free.first().is_none_or(|f| f.0 > 0) {
            lot.waits = false;
        }
    }

    /// Band `i`'s rooms: its listed rooms in order, each in the first lot it fits, then
    /// its fill room until no lot holds another. A hall's bands are already rooms.
    fn band(&mut self, i: usize) -> Result<(), String> {
        let band = &self.role.bands[i];
        let listed = band
            .rooms
            .iter()
            .map(|m| self.def(m))
            .collect::<Result<Vec<_>, String>>()?;
        let fill = band.fill.as_ref().map(|m| self.def(m)).transpose()?;
        let mut lots = self.lots(i);
        // Lots with as much room left take rooms in an order the rng draws, so a band's
        // rooms don't always start on one side.
        for k in (1..lots.len()).rev() {
            let j = self.rng.random_range(0..=k as i32) as usize;
            lots.swap(k, j);
        }
        self.mark_loops(i, &mut lots);
        for lot in &mut lots {
            self.vestibules(i, lot);
        }
        for (k, d) in listed.iter().enumerate() {
            let open =
                listed[k + 1..].iter().any(|d| !d.private) || fill.is_some_and(|f| !f.private);
            self.place(i, &mut lots, d, false, open);
        }
        if let Some(f) = fill {
            while self.place(i, &mut lots, f, true, !f.private) {}
        }
        Ok(())
    }

    /// Places a room of `d` in the lots of band `i`: a lot waiting at an extra door first
    /// when anyone may enter it, then the lot with the most room left. A private room
    /// leaves a waiting lot to a room still to come that anyone may enter. False when it
    /// fits nowhere.
    fn place(&mut self, i: usize, lots: &mut [Lot], d: &RoomDef, fill: bool, open: bool) -> bool {
        let mut order: Vec<usize> = (0..lots.len()).collect();
        order.sort_by_key(|&k| (!(lots[k].waits && !d.private), -lots[k].room_left()));
        for k in order {
            if lots[k].waits && d.private && open {
                continue;
            }
            let Some(f) = self.fit(i, &lots[k], d, fill) else {
                continue;
            };
            let lot = &mut lots[k];
            let s = lot.free[f.stretch].0;
            let (a, c) = lot.span((s, s + f.along), (0, f.out));
            let zone = self.x.bands[i].zone;
            let room = self.of_def(self.axes.rect(a, c), d, zone);
            let room = self.push(room);
            let at = self.pos(lot.at(f.door, -1));
            self.join(at, (i, room), false);
            lot.free[f.stretch].0 += f.along + 1;
            if lot.free[f.stretch].0 >= lot.free[f.stretch].1 {
                lot.free.remove(f.stretch);
            }
            if lot.single() {
                lot.free.clear();
            }
            lot.waits = false;
            return true;
        }
        false
    }

    /// Where a room of `d` fits in `lot`: its def's way round, else turned.
    fn fit(&mut self, i: usize, lot: &Lot, d: &RoomDef, fill: bool) -> Option<Fit> {
        let (along, out) = lot.ranges(d);
        self.fit_ranges(i, lot, (along, out), fill)
            .or_else(|| self.fit_ranges(i, lot, (out, along), fill))
    }

    /// Where a room runs `along` the lot's wall and `out` from it (each least and most):
    /// in the first free stretch long enough, as far out as the lot and its range allow
    /// (a fill room all the way out), and along as the rng draws within its range, taking
    /// the rest of the stretch rather than leave a sliver when its range allows. A lot off
    /// a bent's turn takes one room at the turn.
    fn fit_ranges(
        &mut self,
        i: usize,
        lot: &Lot,
        ((a0, a1), (o0, o1)): ((i32, i32), (i32, i32)),
        fill: bool,
    ) -> Option<Fit> {
        let out = if fill { lot.out() } else { lot.out().min(o1) };
        if out < if fill { LEAST } else { o0 } {
            return None;
        }
        let (least, most) = if fill {
            (a0.max(LEAST), a1.max(a0.max(LEAST)))
        } else {
            (a0, a1)
        };
        for (k, &(f0, f1)) in lot.free.iter().enumerate() {
            if lot.single() && f0 != 0 {
                return None;
            }
            let room = f1 - f0;
            let along = if fill && (lot.single() || room < least) {
                room
            } else if room < least {
                continue;
            } else {
                let along = self.rng.random_range(least..=most).min(room);
                if room - along - 1 < LEAST && (fill || room <= most) {
                    room
                } else {
                    along
                }
            };
            if along < LEAST.min(least) {
                continue;
            }
            if let Some(door) = self.door(i, lot, (f0, f0 + along)) {
                return Some(Fit {
                    stretch: k,
                    along,
                    out,
                    door,
                });
            }
        }
        None
    }

    /// The door of a room over `s` along a lot's wall: the middle tile, else the one
    /// nearest it, with the spine room beyond, nothing reserved on it or inside, and no
    /// door beside it.
    fn door(&self, i: usize, lot: &Lot, s: (i32, i32)) -> Option<i32> {
        let spine = self.x.rooms[i].rect;
        let mid = s.0 + (s.1 - s.0) / 2;
        let mut along: Vec<i32> = (s.0..s.1).collect();
        along.sort_by_key(|m| ((m - mid).abs(), *m));
        along.into_iter().find(|&m| {
            let (at, inside, beyond) = (
                self.pos(lot.at(m, -1)),
                self.pos(lot.at(m, 0)),
                self.pos(lot.at(m, -2)),
            );
            lot.touches(m)
                && spine.contains(beyond)
                && !self.x.core.is_some_and(|c| c.contains(beyond))
                && !self.reserved.contains(&at)
                && !self.reserved.contains(&inside)
                && !self.x.doors.iter().any(|d| d.at.manhattan(at) <= 1)
        })
    }

    /// Each extra door of a border moves along its cut to the nearest tile with a room
    /// anyone may enter on either side, and no door beside it; one with none goes.
    fn loops(&mut self) {
        let width = self.axes.width();
        let mut gone = Vec::new();
        for k in 0..self.x.doors.len() {
            if !self.loops[k] {
                continue;
            }
            let d = self.x.doors[k];
            let (cut, want) = (self.axes.deep(d.at), self.axes.across(d.at));
            let pick = (0..width)
                .filter_map(|c| {
                    let at = self.axes.pos(cut, c);
                    let crowded = self
                        .x
                        .doors
                        .iter()
                        .enumerate()
                        .any(|(j, o)| j != k && o.at.manhattan(at) <= 1);
                    if crowded || self.x.pieces.iter().any(|p| p.rect.contains(at)) {
                        return None;
                    }
                    let f = self.room_at(self.axes.pos(cut - 1, c))?;
                    let b = self.room_at(self.axes.pos(cut + 1, c))?;
                    let open = !self.x.rooms[f].private && !self.x.rooms[b].private;
                    open.then_some((c, (f, b)))
                })
                .min_by_key(|(c, _)| ((c - want).abs(), *c));
            match pick {
                Some((c, rooms)) => self.x.doors[k] = moved(self.axes, &d, c, rooms),
                None => gone.push(k),
            }
        }
        for k in gone.into_iter().rev() {
            self.x.doors.remove(k);
            self.loops.remove(k);
        }
    }

    /// The level's locks numbered afresh in door order, as the zones stage numbered them.
    fn renumber(&mut self) {
        let mut number = LOCK_NUMBERS;
        for d in &mut self.x.doors {
            if let Some(f) = &mut d.fit {
                f.number = number;
                number += 1;
                d.lock = d.lock.map(|_| *f);
            }
        }
    }

    /// Up to four spots a room: the corners of its floor inset by 1, the centre standing
    /// in for one that isn't free, none reserved, by a door, in the core or on a piece.
    /// A room with none of those free takes the free tile nearest its centre.
    fn spots(&mut self) {
        let taken = Taken::of(self.x);
        for r in &mut self.x.rooms {
            let inner = r.rect.inset(1);
            r.spots.clear();
            if inner.is_empty() {
                continue;
            }
            let (x0, y0) = (inner.x, inner.y);
            let (x1, y1) = (inner.right() - 1, inner.bottom() - 1);
            let mut tiles = Vec::new();
            for p in [
                Pos::new(x0, y0),
                Pos::new(x1, y0),
                Pos::new(x0, y1),
                Pos::new(x1, y1),
                inner.center(),
            ] {
                if !tiles.contains(&p) {
                    tiles.push(p);
                }
            }
            r.spots = tiles
                .into_iter()
                .filter(|p| taken.free(*p))
                .take(4)
                .collect();
            if r.spots.is_empty() {
                r.spots.extend(taken.nearest(r.rect));
            }
        }
    }
}

/// What keeps a tile of a level from being a spot: reserved, a door or beside one, in
/// the core, or on a piece.
pub(super) struct Taken {
    reserved: BTreeSet<Pos>,
    near: BTreeSet<Pos>,
    core: Option<Rect>,
    pieces: Vec<Rect>,
}

impl Taken {
    pub(super) fn of(x: &LevelLayout) -> Taken {
        Taken {
            reserved: x.reserved.iter().copied().collect(),
            near: x
                .doors
                .iter()
                .flat_map(|d| std::iter::once(d.at).chain(d.at.neighbors4()))
                .collect(),
            core: x.core,
            pieces: x.pieces.iter().map(|p| p.rect).collect(),
        }
    }

    /// Whether `p` can be a spot.
    pub(super) fn free(&self, p: Pos) -> bool {
        !self.reserved.contains(&p)
            && !self.near.contains(&p)
            && !self.core.is_some_and(|c| c.contains(p))
            && !self.pieces.iter().any(|r| r.contains(p))
    }

    /// The free tile of a room's floor inset by 1 nearest its centre.
    pub(super) fn nearest(&self, room: Rect) -> Option<Pos> {
        let inner = room.inset(1);
        let centre = inner.center();
        inner
            .iter()
            .filter(|p| self.free(*p))
            .min_by_key(|p| (p.manhattan(centre), p.y, p.x))
    }
}
