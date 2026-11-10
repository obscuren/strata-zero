//! The rooms stage: each band filled with its type's rooms along the spine, and the
//! level's set pieces: a platform and its track along the far side, a gate line of
//! turnstiles across the first border, a ramp in a car park.
//!
//! A room is its def's width across the level and its height front to back, within the
//! def's range; a fill room takes what the listed rooms leave, never under 3 x 3. Rooms
//! pack along the walls of the band's spine room, each with its door on the wall, and the
//! floor they leave stays wall. A stretch along a spine wall with a doorstep in it or on
//! its walls is a vestibule instead: a room of the corridor out to the frame, open to the
//! spine across the wall, so no wall stands on a doorstep or beside one.

use super::zones::{self, Axes};
use super::{
    LOCK_NUMBERS, LevelDef, LevelLayout, PieceDef, PieceKind, PlacedPiece, PlannedDoor,
    PlannedRoom, RoomDef, Spine, VenueLayout, VenueType, room_number,
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
/// A platform's rows, and its track's behind it against the far wall.
const PLATFORM: i32 = 3;
const TRACK: i32 = 2;
/// The most doors a gate line takes.
const GATES: i32 = 6;

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

    /// A def's least and most along the wall, then out from it: its height along a
    /// side's wall, which runs front to back, and its width along a bent's turn.
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

    /// Whether the last band holds a platform and its track at the far side, behind a
    /// wall on no doorstep's row, with a row of its spine left in front.
    fn track_fits(&self) -> bool {
        let depth = self.axes.depth();
        let (u, e) = self.rows[self.bands() - 1];
        let wall = depth - PLATFORM - TRACK - 1;
        e == depth && wall > u && !self.reserved.iter().any(|p| self.axes.deep(*p) == wall)
    }

    /// Runs the last band's spine straight to the far side when a track needs it there,
    /// or when a bent's turn would stand a wall on a doorstep.
    fn straighten(&mut self, track: bool) {
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
    fn track(&mut self, p: &PieceDef) {
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
    fn gate_line(&mut self, p: &PieceDef) {
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

    /// Where a room of `d` fits in `lot`: in the first free stretch long enough, as far
    /// out as the lot and its range allow (a fill room all the way out), and along as the
    /// rng draws within its range, taking the rest of the stretch rather than leave a
    /// sliver when its range allows. A lot off a bent's turn takes one room at the turn.
    fn fit(&mut self, i: usize, lot: &Lot, d: &RoomDef, fill: bool) -> Option<Fit> {
        let ((a0, a1), (o0, o1)) = lot.ranges(d);
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

    /// A ramp: a strip 2 along the frame wall and 4 out from it in a car park on the wall,
    /// else 4 along and 2 out, clear of doorsteps and doors. None without a car park on
    /// the frame wall with room for one.
    fn ramp(&mut self, p: &PieceDef) {
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
        let near = self.near_doors();
        let core = self.x.core;
        let pieces: Vec<Rect> = self.x.pieces.iter().map(|p| p.rect).collect();
        let reserved = &self.reserved;
        let free = |p: Pos| {
            !reserved.contains(&p)
                && !near.contains(&p)
                && !core.is_some_and(|c| c.contains(p))
                && !pieces.iter().any(|r| r.contains(p))
        };
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
            r.spots = tiles.into_iter().filter(|p| free(*p)).take(4).collect();
            if r.spots.is_empty() {
                let centre = inner.center();
                r.spots.extend(
                    inner
                        .iter()
                        .filter(|p| free(*p))
                        .min_by_key(|p| (p.manhattan(centre), p.y, p.x)),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::zones::{Axes, front};
    use crate::geom::{Pos, Rect};
    use crate::interior::RoomKind;
    use crate::interior::venue::testutil::{cities, test_type};
    use crate::interior::venue::{
        LevelLayout, PieceKind, Spine, VenueFrame, VenueLayout, VenuePack, VenueType, lay_out,
        room_label, room_number,
    };
    use std::collections::BTreeSet;

    fn layouts(t: &VenueType) -> Vec<(u64, VenueLayout)> {
        (0..20)
            .map(|s| (s, lay_out(t, VenueFrame::own(t), s).unwrap()))
            .collect()
    }

    fn axes(l: &VenueLayout, x: &LevelLayout) -> Axes {
        Axes::new(x.size, front(&l.frame))
    }

    /// The band whose rect holds all of `r`.
    fn band_of(x: &LevelLayout, r: Rect) -> Option<usize> {
        x.bands
            .iter()
            .position(|b| b.rect.intersection(r) == Some(r))
    }

    /// The name of the role whose pieces name one of `kind`.
    fn role_with(t: &VenueType, kind: PieceKind) -> String {
        let piece = &t.pieces.iter().find(|p| p.kind == kind).unwrap().name;
        let l = &t.levels;
        let mut roles = l
            .below
            .iter()
            .chain([&l.ground])
            .chain(&l.middle)
            .chain(&l.top);
        roles
            .find(|d| d.pieces.contains(piece))
            .unwrap()
            .name
            .clone()
    }

    /// The type with an own frame wide and deep enough for every room it lists.
    fn roomy(t: &VenueType) -> VenueType {
        let mut t = t.clone();
        t.frame.footprint = (20, 12);
        t
    }

    /// Every builtin type and the test type, over its own frame and every building of its
    /// kinds in the test cities, seeds 0..3.
    fn sweep() -> Vec<(String, VenueLayout)> {
        let cities = cities();
        let mut v = Vec::new();
        for t in VenuePack::builtin()
            .types
            .iter()
            .cloned()
            .chain([test_type()])
        {
            let buildings = cities.iter().flat_map(|c| {
                c.buildings
                    .iter()
                    .filter(|b| t.builds.contains(&b.kind))
                    .filter_map(|b| VenueFrame::of_building(c, b.id))
            });
            for f in std::iter::once(VenueFrame::own(&t)).chain(buildings) {
                for seed in 0..3 {
                    let what = format!("{} in {:?} {:?}, seed {seed}", t.id, f.kind, f.footprint);
                    let l = lay_out(&t, f.clone(), seed).unwrap_or_else(|e| panic!("{what}: {e}"));
                    v.push((what, l));
                }
            }
        }
        v
    }

    #[test]
    fn basement_rooms_number_apart_from_the_lobby() {
        assert_eq!(room_number(0, 4), 4);
        assert_eq!(room_number(12, 3), 1203);
        assert_eq!(room_number(-1, 4), 9104);
        assert_eq!(room_number(-5, 12), 9512);
        assert_eq!(room_label(1203), "1203");
        assert_eq!(room_label(9104), "B1-04");
    }

    #[test]
    fn rooms_fill_their_band_and_keep_its_zone() {
        let t = test_type();
        let (mut loops, mut sized) = (0, 0);
        for (seed, l) in layouts(&t) {
            for x in &l.levels {
                let what = format!("seed {seed} level {}", x.level);
                let def = t.role(&x.role).unwrap();
                let ax = axes(&l, x);
                let n = x.bands.len();
                let joined = |a: usize, b: usize| {
                    x.doors
                        .iter()
                        .any(|d| d.rooms == (a, b) || d.rooms == (b, a))
                };
                for (i, r) in x.rooms.iter().enumerate() {
                    let b = band_of(x, r.rect)
                        .unwrap_or_else(|| panic!("{what}: room {i} at {:?} in no band", r.rect));
                    assert_eq!(r.zone, x.bands[b].zone, "{what}: room {i}");
                    for (j, o) in x.rooms.iter().enumerate().skip(i + 1) {
                        assert!(!r.rect.intersects(o.rect), "{what}: rooms {i} and {j}");
                    }
                    if i < n {
                        assert_eq!(b, i, "{what}: spine room {i}");
                        continue;
                    }
                    // A track is joined to its platform, every other room to its band's
                    // spine room.
                    if r.kind == RoomKind::Track {
                        let platform = x.rooms.iter().position(|o| o.kind == RoomKind::Platform);
                        assert!(joined(platform.unwrap(), i), "{what}: the track");
                    } else {
                        assert!(joined(b, i), "{what}: room {i} ({}) has no door", r.name);
                    }
                    let band = &def.bands[b];
                    let (rows, across) = ax.span(r.rect);
                    let (w, h) = (across.1 - across.0, rows.1 - rows.0);
                    if band.rooms.contains(&r.name) {
                        let d = t.room(&r.name).unwrap();
                        let ((w0, h0), (w1, h1)) = d.size;
                        assert!(
                            (w0..=w1).contains(&w) && (h0..=h1).contains(&h),
                            "{what}: {} is {w} x {h}, its range {:?}",
                            r.name,
                            d.size
                        );
                        sized += 1;
                    } else if band.fill.as_ref() == Some(&r.name) {
                        assert!(w >= 3 && h >= 3, "{what}: a fill {} is {w} x {h}", r.name);
                    }
                }
                // A door stands on the wall between its two rooms; an opening is a tile of
                // its second room, beside its first.
                for d in &x.doors {
                    let (r0, r1) = (x.rooms[d.rooms.0].rect, x.rooms[d.rooms.1].rect);
                    let beside = |r: Rect| d.at.neighbors4().any(|p| r.contains(p));
                    if d.open {
                        assert!(r1.contains(d.at) && beside(r0), "{what}: {:?}", d.at);
                    } else {
                        assert!(x.rooms.iter().all(|r| !r.rect.contains(d.at)), "{what}");
                        assert!(beside(r0) && beside(r1), "{what}: the door at {:?}", d.at);
                    }
                }
                // Every border door joins a room of each of its two bands, across its cut.
                let mut first = BTreeSet::new();
                for d in x.doors.iter().filter(|d| d.border) {
                    let (cut, c) = (ax.deep(d.at), ax.across(d.at));
                    let j = x
                        .bands
                        .iter()
                        .position(|b| ax.span(b.rect).0.0 == cut)
                        .unwrap_or_else(|| panic!("{what}: the door at {:?} is on no cut", d.at));
                    let (f, k) = (ax.pos(cut - 1, c), ax.pos(cut + 1, c));
                    let (rf, rb) = (&x.rooms[d.rooms.0], &x.rooms[d.rooms.1]);
                    assert!(
                        rf.rect.contains(f) && rb.rect.contains(k),
                        "{what}: {:?}",
                        d.at
                    );
                    assert_eq!(band_of(x, rf.rect), Some(j - 1), "{what}: {:?}", d.at);
                    assert_eq!(band_of(x, rb.rect), Some(j), "{what}: {:?}", d.at);
                    loops += usize::from(!first.insert(cut));
                }
            }
        }
        assert!(loops > 0, "a band with doors: 2 has its loop door");
        assert!(sized > 0);
    }

    #[test]
    fn private_rooms_and_offices_take_the_levels_numbers() {
        let t = test_type();
        let mut numbered = 0;
        for (seed, l) in layouts(&t) {
            for x in &l.levels {
                let mut n = 0;
                for r in &x.rooms {
                    let want = t.room(&r.name).is_some_and(|d| {
                        d.private
                            || matches!(
                                d.kind,
                                RoomKind::HotelRoom | RoomKind::Flat | RoomKind::Office
                            )
                    });
                    if want {
                        n += 1;
                        assert_eq!(r.number, room_number(x.level, n), "seed {seed}");
                    } else {
                        assert_eq!(r.number, 0, "seed {seed}: {}", r.name);
                    }
                }
                numbered += n;
            }
        }
        assert!(numbered > 0);
    }

    #[test]
    fn listed_rooms_come_before_the_fill() {
        let t = test_type();
        for (t, every) in [(t.clone(), false), (roomy(&t), true)] {
            for (seed, l) in layouts(&t) {
                for x in &l.levels {
                    let what = format!("seed {seed} level {}", x.level);
                    let def = t.role(&x.role).unwrap();
                    let n = x.bands.len();
                    for (b, band) in def.bands.iter().enumerate() {
                        let fill = band.fill.as_ref();
                        if x.spine == Spine::Hall {
                            // A hall band is one room of its first listed def.
                            let first = band.rooms.first().or(fill).unwrap();
                            assert_eq!(&x.rooms[b].name, first, "{what}: band {b}");
                        }
                        let names: Vec<&String> = x
                            .rooms
                            .iter()
                            .skip(n)
                            .filter(|r| band_of(x, r.rect) == Some(b))
                            .map(|r| &r.name)
                            .filter(|m| band.rooms.contains(m) || fill == Some(*m))
                            .collect();
                        let split = names
                            .iter()
                            .position(|m| !band.rooms.contains(m))
                            .unwrap_or(names.len());
                        let (listed, rest) = names.split_at(split);
                        assert!(rest.iter().all(|m| fill == Some(*m)), "{what}: {names:?}");
                        // The listed rooms that fit, in list order, each once.
                        let mut order = band.rooms.iter();
                        for m in listed {
                            assert!(order.any(|o| o == *m), "{what}: band {b} {names:?}");
                        }
                        if every && x.spine != Spine::Hall {
                            assert_eq!(listed.len(), band.rooms.len(), "{what}: band {b}");
                        }
                        if x.spine == Spine::Hall {
                            assert!(names.is_empty(), "{what}: band {b} {names:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn doorsteps_landings_and_spots_stay_clear() {
        let mut spots = 0;
        for (what, l) in sweep() {
            for x in &l.levels {
                let what = format!("{what}, level {}", x.level);
                let reserved: BTreeSet<Pos> = x.reserved.iter().copied().collect();
                // A tile no room holds is wall.
                for p in &reserved {
                    assert!(
                        x.rooms.iter().any(|r| r.rect.contains(*p)),
                        "{what}: a wall on {p:?}"
                    );
                    assert!(
                        !x.doors.iter().any(|d| !d.open && d.at == *p),
                        "{what}: a door on {p:?}"
                    );
                }
                let near: BTreeSet<Pos> = x
                    .doors
                    .iter()
                    .flat_map(|d| std::iter::once(d.at).chain(d.at.neighbors4()))
                    .collect();
                let taken = |p: Pos| {
                    reserved.contains(&p)
                        || near.contains(&p)
                        || x.core.is_some_and(|c| c.contains(p))
                        || x.pieces.iter().any(|q| q.rect.contains(p))
                };
                for r in &x.rooms {
                    assert!(r.spots.len() <= 4, "{what}");
                    for s in &r.spots {
                        assert!(
                            r.rect.inset(1).contains(*s),
                            "{what}: {s:?} in {:?}",
                            r.rect
                        );
                        assert!(!taken(*s), "{what}: the spot {s:?} isn't free");
                    }
                    spots += r.spots.len();
                    let inner = r.rect.inset(1);
                    if r.rect.w >= 3 && r.rect.h >= 3 && inner.iter().any(|p| !taken(p)) {
                        assert!(!r.spots.is_empty(), "{what}: {} has no spot", r.name);
                    }
                }
            }
        }
        assert!(spots > 0);
    }

    #[test]
    fn a_track_runs_the_far_side_and_ends_in_shutters() {
        let t = test_type();
        let role = role_with(&t, PieceKind::Track);
        for (seed, l) in layouts(&t) {
            let x = l.levels.iter().find(|x| x.role == role).unwrap();
            let what = format!("seed {seed} level {}", x.level);
            let ax = axes(&l, x);
            let (depth, width) = (ax.depth(), ax.width());
            let tracks: Vec<_> = x
                .pieces
                .iter()
                .filter(|p| p.kind == PieceKind::Track)
                .collect();
            assert_eq!(tracks.len(), 1, "{what}");
            let room = |k: RoomKind| x.rooms.iter().position(|r| r.kind == k).unwrap();
            let (platform, track) = (room(RoomKind::Platform), room(RoomKind::Track));
            // Both span the inner width: the platform 3 deep, then the track 2 deep against
            // the back wall.
            let span = |i: usize| ax.span(x.rooms[i].rect);
            assert_eq!(
                span(platform),
                ((depth - 5, depth - 2), (0, width)),
                "{what}"
            );
            assert_eq!(span(track), ((depth - 2, depth), (0, width)), "{what}");
            assert_eq!(tracks[0].rect, x.rooms[track].rect, "{what}");
            let back = x.bands.last().unwrap().zone;
            assert_eq!((x.rooms[platform].zone, x.rooms[track].zone), (back, back));
            // Its ends are the frame wall's tiles at each end of the track.
            let frame = x.size.rect();
            let ends = &tracks[0].ends;
            assert_eq!(ends, &vec![ax.pos(depth - 2, -1), ax.pos(depth - 2, width)]);
            for e in ends {
                assert!(frame.contains(*e) && !frame.inset(1).contains(*e), "{what}");
                assert!(e.neighbors4().any(|p| x.rooms[track].rect.contains(p)));
            }
            // The platform opens off the spine; the track off the platform's edge.
            let n = x.bands.len();
            let to = |a: usize, b: usize| x.doors.iter().find(|d| d.rooms == (a, b));
            assert!(to(n - 1, platform).is_some_and(|d| !d.open), "{what}");
            assert!(to(platform, track).is_some_and(|d| d.open), "{what}");
        }
    }

    #[test]
    fn a_gate_line_turns_its_border_into_turnstiles() {
        let t = test_type();
        let role = role_with(&t, PieceKind::GateLine);
        let mut spines = BTreeSet::new();
        for (seed, l) in layouts(&t) {
            let x = l.levels.iter().find(|x| x.role == role).unwrap();
            let what = format!("seed {seed} level {}", x.level);
            let ax = axes(&l, x);
            let cut = ax.span(x.bands[1].rect).0.0;
            let (zf, zb) = (x.bands[0].zone, x.bands[1].zone);
            let b = t.border(zf, zb).unwrap();
            let gate: Vec<_> = x.doors.iter().filter(|d| ax.deep(d.at) == cut).collect();
            assert!(
                (3..=6).contains(&gate.len()),
                "{what}: {} doors",
                gate.len()
            );
            // One line, a tile apart, where the spine's single door was.
            let cols: Vec<i32> = gate.iter().map(|d| ax.across(d.at)).collect();
            assert!(
                cols.windows(2).all(|w| w[1] - w[0] == 2),
                "{what}: {cols:?}"
            );
            let line = ax.line();
            assert!(
                cols[0] <= line.0 && line.0 <= cols[cols.len() - 1],
                "{what}"
            );
            for d in &gate {
                let fit = d.fit.unwrap();
                assert!(d.border && !d.open, "{what}");
                assert_eq!(
                    (fit.at, fit.kind, fit.zone),
                    (d.at, b.lock, zf.max(zb)),
                    "{what}"
                );
                assert_eq!(d.lock, b.always.then_some(fit), "{what}");
                assert_eq!(d.rooms, (0, 1), "{what}");
            }
            let pieces: Vec<_> = x
                .pieces
                .iter()
                .filter(|p| p.kind == PieceKind::GateLine)
                .collect();
            assert_eq!(pieces.len(), 1, "{what}");
            assert!(gate.iter().all(|d| pieces[0].rect.contains(d.at)), "{what}");
            spines.insert(format!("{:?}", x.spine));
        }
        assert!(spines.len() >= 2, "{spines:?}");
    }

    #[test]
    fn a_ramp_is_two_by_four_against_a_wall() {
        let t = test_type();
        let role = role_with(&t, PieceKind::Ramp);
        let mut ramps = 0;
        for (seed, l) in layouts(&t) {
            let x = l.levels.iter().find(|x| x.role == role).unwrap();
            let what = format!("seed {seed} level {}", x.level);
            let inner = x.size.rect().inset(1);
            for p in x.pieces.iter().filter(|p| p.kind == PieceKind::Ramp) {
                ramps += 1;
                let r = p.rect;
                assert!(matches!((r.w, r.h), (2, 4) | (4, 2)), "{what}: {r:?}");
                let walls = [
                    r.x == inner.x,
                    r.y == inner.y,
                    r.right() == inner.right(),
                    r.bottom() == inner.bottom(),
                ];
                assert!(walls.contains(&true), "{what}: {r:?} off the frame wall");
                assert!(
                    x.rooms
                        .iter()
                        .any(|c| c.kind == RoomKind::CarPark && c.rect.intersection(r) == Some(r)),
                    "{what}: {r:?} in no car park"
                );
                assert!(r.iter().all(|q| !x.reserved.contains(&q)), "{what}");
            }
        }
        assert!(ramps > 0);
    }

    #[test]
    fn middle_floors_differ() {
        let t = test_type();
        let middle = &t.levels.middle.as_ref().unwrap().name;
        let top = t.levels.top.len() as i32 + 6;
        for seed in 0..10 {
            let l = lay_out(&t, VenueFrame::own(&t).raised(top), seed).unwrap();
            let levels: Vec<&LevelLayout> = l.levels.iter().filter(|x| &x.role == middle).collect();
            let floors: Vec<Vec<Rect>> = levels
                .iter()
                .map(|x| x.rooms.iter().map(|r| r.rect).collect())
                .collect();
            assert_eq!(floors.len(), 6);
            for (k, w) in floors.windows(2).enumerate() {
                // A hall's rooms are its bands, which only the zones stage's cuts move: two
                // halls cut alike are the one floor that repeats.
                let halls = levels[k].spine == Spine::Hall && levels[k + 1].spine == Spine::Hall;
                if !halls {
                    assert_ne!(w[0], w[1], "seed {seed}: neighbouring floors alike");
                }
            }
            let mut kinds: Vec<&Vec<Rect>> = Vec::new();
            for f in &floors {
                if !kinds.contains(&f) {
                    kinds.push(f);
                }
            }
            assert!(
                kinds.len() >= 3,
                "seed {seed}: {} kinds of floor",
                kinds.len()
            );
        }
    }

    #[test]
    fn a_thin_band_holds_only_what_fits() {
        // Shallow real buildings thin a band to as little as 2 rows. The layout still lays,
        // and the band keeps only the listed rooms that fit it, in list order.
        let mut thin = 0;
        let pack = VenuePack::builtin();
        for (what, l) in sweep() {
            let t = pack.by_id(&l.type_id).cloned().unwrap_or_else(test_type);
            for x in l.levels.iter().filter(|x| x.spine != Spine::Hall) {
                let ax = axes(&l, x);
                let what = format!("{what}, level {}", x.level);
                let def = t.role(&x.role).unwrap();
                for (b, band) in x.bands.iter().enumerate() {
                    let (rows, _) = ax.span(band.rect);
                    let floor = rows.1 - rows.0 - i32::from(b > 0);
                    let listed = &def.bands[b].rooms;
                    let tall: Vec<&String> = listed
                        .iter()
                        .filter(|m| t.room(m).unwrap().size.0.1 > floor)
                        .collect();
                    if floor > 2 || tall.is_empty() {
                        continue;
                    }
                    thin += 1;
                    let placed: Vec<&String> = x
                        .rooms
                        .iter()
                        .skip(x.bands.len())
                        .filter(|r| band_of(x, r.rect) == Some(b) && listed.contains(&r.name))
                        .map(|r| &r.name)
                        .collect();
                    // Those too tall for it are left out; the rest keep their list order.
                    assert!(
                        tall.iter().all(|m| !placed.contains(m)),
                        "{what}: {placed:?}"
                    );
                    let mut order = listed.iter();
                    for m in &placed {
                        assert!(order.any(|o| o == *m), "{what}: {placed:?}");
                    }
                }
            }
        }
        assert!(thin > 0, "no band thinned under its listed rooms");
    }
}
