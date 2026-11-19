//! Your rig's room (`Place::Rig`), as round 5 of the mock draws it
//! (`docs/superpowers/specs/2026-11-18-rig-construct-mock/gen.py`, `look_a`): the
//! firewall's ring round the floor, PORTS with a door per line, the hall with the
//! reader's slots and the LINE trunk, SEATS with a box per core seat and the terminal,
//! and a dotted line from each job on a backdoor to its door. Drawn from
//! `rigroom::view`, the rig as it stands this frame.

use super::inside::LATTICE;
use super::packets::PACKET_TICKS;
use super::{AMBER, BLACK_RED, ICE_BLUE, ROOM, VOID, WHITE, YOU, YOU_GLOW, put};
use crate::render::Rgb;
use crate::render::viewport::{screen_to_tile, tile_to_screen};
use crate::rigroom::RigRoom;
use citygen::{Pos, Rect as TileRect, Terrain};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::collections::HashMap;

/// The firewall's ring.
const FW: Rgb = BLACK_RED;
const FW_BG: Rgb = Rgb(0x1f, 0x07, 0x0c);
/// The floor, its double wall, the dividers between its rooms.
const LEVEL: Rgb = Rgb(0x01, 0x19, 0x1d);
const WALL: Rgb = Rgb(0x00, 0xb7, 0xcc);
const WALL_BG: Rgb = Rgb(0x01, 0x18, 0x1c);
const DIVIDER: Rgb = Rgb(0x01, 0x70, 0x5e);
/// The rig's kinds, as the desk tints its sleds.
const CORE: Rgb = ROOM;
const RELAY: Rgb = Rgb(0x50, 0xe6, 0x78);
const LINE: Rgb = AMBER;
/// Words, as the panel beside the map sets them.
const TEXT: Rgb = Rgb(0xcd, 0xd4, 0xde);
const DIM: Rgb = Rgb(0x8a, 0x93, 0xa3);
const FAINT: Rgb = Rgb(0x4a, 0x52, 0x5e);
const LABEL: Rgb = Rgb(0xa0, 0xaa, 0xb9);
/// A slot's box and a free seat's; an empty slot's; the rule between doors.
const BOX: Rgb = Rgb(0x0b, 0x55, 0x60);
const EMPTY_BOX: Rgb = Rgb(0x0b, 0x3a, 0x40);
const RULE: Rgb = Rgb(0x0b, 0x3a, 0x35);
/// A terminal you hold, as the grid grounds it (`holds::HELD_BG`).
const HELD: Rgb = Rgb(0x0f, 0x3a, 0x1f);

const LIGHT: [char; 6] = ['┌', '┐', '└', '┘', '─', '│'];
const DOUBLE: [char; 6] = ['╔', '╗', '╚', '╝', '═', '║'];
const ROUND: [char; 6] = ['╭', '╮', '╰', '╯', '─', '│'];

/// The arms a line's cell joins.
const N: u8 = 1;
const E: u8 = 2;
const S: u8 = 4;
const W: u8 = 8;
/// A job's line by the arms each cell joins: dotted straights, light corners.
const DOTTED: [char; 16] = [
    '┈', '┊', '┈', '└', '┊', '┊', '┌', '├', '┈', '┘', '┈', '┴', '┐', '┤', '┬', '┼',
];

/// The room and what its parts hold, read off the rig (`rigroom::view`).
pub struct RigView<'a> {
    pub room: &'a RigRoom,
    /// A door per line, top to bottom.
    pub doors: Vec<DoorView>,
    /// A slot per reader slot, left to right; none for an empty one.
    pub slots: Vec<Option<SlotView>>,
    /// A seat per core seat, top to bottom; none for a free one.
    pub seats: Vec<Option<SeatView>>,
    pub you: Pos,
    /// The room's tile at the area's top-left: the map's camera.
    pub origin: Pos,
    /// The clock the Net's animations run on (`st.tick`).
    pub tick: u64,
    /// The firewall sled's word (`sandbag`) and its tier, 0 for none.
    pub firewall: (String, u8),
    /// Under PORTS: the relay and its lines (`Switchback · 5`).
    pub relay: String,
    /// Under SEATS: the core and its seats (`Tinman 2X · 2`).
    pub core: String,
    /// Beside LINE: the line sled and its hold (`Longwire · 5d`).
    pub line: String,
}

/// A door in PORTS' outer wall.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DoorView {
    /// The backdoor's building, or `free line`.
    pub name: String,
    /// Its time left on the nominal clock (`2d left`); empty on a free line.
    pub left: String,
    /// The job that holds its clock still (`keepalive`), said in place of its time left.
    pub held_by: Option<&'static str>,
    /// A backdoor stands on it: lit, with its lane out through the firewall.
    pub lit: bool,
}

/// What a reader slot holds.
#[derive(Clone, Debug, PartialEq)]
pub struct SlotView {
    pub glyph: char,
    pub colour: Color,
    /// Under the box: what it is, in a word (`Credchip`).
    pub name: String,
    /// Under that: what summoning it does (`¤340`, `2 locks`).
    pub tail: String,
}

/// A job in a core seat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeatView {
    pub job: String,
    /// What it works on (`Paydata shard`, `Kiroflux Mart`).
    pub on: String,
    /// How it's going (`▰▰▰▱▱ 4h`, `noise 3 of 36`, `¤6/h`).
    pub state: String,
    /// The door of the backdoor it works on.
    pub door: Option<usize>,
}

/// The room in `area`: the Net's void round it, the ring, the floor and its rooms, the
/// jobs' lines over them and you.
pub fn draw(buf: &mut Buffer, area: Rect, view: &RigView) {
    let mut c = Canvas {
        buf,
        area,
        origin: view.origin,
    };
    for sy in area.y..area.bottom() {
        for sx in area.x..area.right() {
            let p = screen_to_tile(area, view.origin, (sx, sy));
            let dot = p.x.rem_euclid(4) == 1 && p.y.rem_euclid(2) == 0;
            let ch = if dot { '·' } else { ' ' };
            put(c.buf, sx, sy, ch, LATTICE, VOID, false);
        }
    }
    let room = view.room;
    if let Some(ring) = room.ring {
        c.fill(ring, FW_BG);
        c.boxed(ring, DOUBLE, FW, FW_BG, true);
        let (word, tier) = &view.firewall;
        let blocks: String = (0..3).map(|n| if n < *tier { '▮' } else { '▯' }).collect();
        let title = format!(" FIREWALL · {word} {blocks} ");
        let x = ring.x + (ring.w - title.chars().count() as i32) / 2;
        c.text(Pos::new(x, ring.y), &title, FW, FW_BG, true);
    }
    let f = room.floor;
    c.fill(f, LEVEL);
    c.boxed(f, DOUBLE, WALL, WALL_BG, false);
    for x in [room.hall.x, room.seats_room.x] {
        c.put(Pos::new(x, f.y), '╤', WALL, WALL_BG, false);
        c.put(Pos::new(x, f.bottom() - 1), '╧', WALL, WALL_BG, false);
        for y in f.y + 1..f.bottom() - 1 {
            if wall(room, Pos::new(x, y)) {
                c.put(Pos::new(x, y), '│', DIVIDER, LEVEL, false);
            }
        }
    }
    ports(&mut c, view);
    hall(&mut c, view);
    seats(&mut c, view);
    jobs_lines(&mut c, view);
    c.put(view.you, '@', YOU, YOU_GLOW, true);
}

/// PORTS: a door per line, a lit one named with its clock and its lane out through the
/// firewall to the edge of the view, a free one dark.
fn ports(c: &mut Canvas, view: &RigView) {
    let room = view.room;
    let r = room.ports;
    c.text(Pos::new(r.x + 2, r.y + 1), "PORTS", RELAY, LEVEL, true);
    c.text(Pos::new(r.x + 2, r.y + 2), &view.relay, DIM, LEVEL, false);
    let last = room.doors.len().saturating_sub(1);
    for (i, (door, d)) in room.doors.iter().zip(&view.doors).enumerate() {
        let (glyph, name) = if d.lit { (RELAY, TEXT) } else { (FAINT, FAINT) };
        c.put(*door, '▯', glyph, WALL_BG, true);
        c.text(Pos::new(r.x + 2, door.y), &d.name, name, LEVEL, false);
        let under = Pos::new(r.x + 2, door.y + 1);
        match d.held_by {
            Some(job) if d.lit => c.text(under, &format!("held · {job}"), CORE, LEVEL, false),
            _ => c.text(under, &d.left, DIM, LEVEL, false),
        }
        if d.lit {
            for x in c.origin.x..door.x {
                let p = Pos::new(x, door.y);
                match room.ring {
                    Some(r) if x == r.x => c.put(p, '╬', FW, FW_BG, true),
                    Some(r) if r.contains(p) => c.put(p, '═', RELAY, FW_BG, false),
                    _ => c.put(p, '═', RELAY, VOID, false),
                }
            }
        }
        if i < last {
            for x in r.x + 1..r.right() {
                c.put(Pos::new(x, door.y + 2), '┄', RULE, LEVEL, false);
            }
        }
    }
}

/// The hall: the reader's slots under its title, the LINE trunk at its foot and down
/// through the firewall to the grid.
fn hall(c: &mut Canvas, view: &RigView) {
    let room = view.room;
    let f = room.floor;
    let h = room.hall;
    let n = room.slots.len();
    c.text(Pos::new(h.x + 3, f.y + 1), "READER", AMBER, LEVEL, true);
    let count = format!("· {n} {}", if n == 1 { "slot" } else { "slots" });
    c.text(Pos::new(h.x + 10, f.y + 1), &count, DIM, LEVEL, false);
    for (r, slot) in room.slots.iter().zip(&view.slots) {
        match slot {
            Some(s) => {
                c.boxed(*r, LIGHT, BOX, LEVEL, false);
                let middle = Pos::new(r.x + r.w / 2, r.y + r.h / 2);
                c.put(middle, s.glyph, rgb(s.colour), LEVEL, true);
                let tail = if s.glyph == '◆' { FW } else { DIM };
                c.under(*r, 0, &s.name, TEXT);
                c.under(*r, 1, &s.tail, tail);
            }
            None => {
                c.boxed(*r, LIGHT, EMPTY_BOX, LEVEL, false);
                c.under(*r, 0, "free", FAINT);
            }
        }
    }
    let foot = f.bottom() - 2;
    c.text(Pos::new(h.x + 2, foot), "LINE", LINE, LEVEL, true);
    c.text(Pos::new(h.x + 7, foot), &view.line, DIM, LEVEL, false);
    let t = room.trunk;
    c.put(t, '▯', ICE_BLUE, WALL_BG, true);
    let last = room.construct.size.h as i32 - 1;
    for y in t.y + 1..=last {
        let p = Pos::new(t.x, y);
        match room.ring {
            Some(r) if y == r.bottom() - 1 => c.put(p, '╬', FW, FW_BG, true),
            Some(r) if r.contains(p) => c.put(p, '║', ICE_BLUE, FW_BG, false),
            _ => c.put(p, '║', ICE_BLUE, VOID, false),
        }
    }
    let out = Pos::new(t.x + 2, last);
    c.text(out, "▸ the grid · your block", ICE_BLUE, VOID, false);
}

/// SEATS: a box per core seat, a job's lit with what it works on and how it's going, a
/// free one dim; the terminal under them.
fn seats(c: &mut Canvas, view: &RigView) {
    let room = view.room;
    let f = room.floor;
    let x = room.seats_room.x + 2;
    c.text(Pos::new(x, f.y + 1), "SEATS", CORE, LEVEL, true);
    c.text(Pos::new(x, f.y + 2), &view.core, DIM, LEVEL, false);
    for (r, seat) in room.seats.iter().zip(&view.seats) {
        let at = |dy: i32| Pos::new(r.x + 2, r.y + dy);
        match seat {
            Some(s) => {
                c.boxed(*r, ROUND, CORE, LEVEL, false);
                c.text(at(1), &format!("◉ {}", s.job), CORE, LEVEL, true);
                c.text(at(2), &s.on, TEXT, LEVEL, false);
                c.text(at(3), &s.state, DIM, LEVEL, false);
            }
            None => {
                c.boxed(
                    TileRect::new(r.x, r.y + 1, r.w, r.h - 2),
                    ROUND,
                    BOX,
                    LEVEL,
                    false,
                );
                c.text(at(2), "free seat", FAINT, LEVEL, false);
            }
        }
    }
    let t = room.terminal;
    c.put(t, '◇', RELAY, HELD, true);
    c.text(t.offset(2, 0), "TERMINAL", RELAY, LEVEL, true);
    c.text(t.offset(2, 1), "[r] shell", LABEL, LEVEL, false);
}

/// A job's line through the room: its cells from its seat's box to its door, each with
/// the arms it joins, ending at the `◂` after the door's name.
struct JobLine {
    cells: Vec<(Pos, u8)>,
    tip: Pos,
}

/// The arm of `from` that points at its neighbour `to`.
fn arm(from: Pos, to: Pos) -> u8 {
    match (to.x - from.x, to.y - from.y) {
        (0, -1) => N,
        (1, 0) => E,
        (0, 1) => S,
        _ => W,
    }
}

/// The cells along the corners `pts`, in order, each with the arms it joins.
fn trace(pts: &[Pos]) -> Vec<(Pos, u8)> {
    let mut cells = vec![(pts[0], 0)];
    for leg in pts.windows(2) {
        let (mut p, to) = (leg[0], leg[1]);
        while p != to {
            let q = Pos::new(p.x + (to.x - p.x).signum(), p.y + (to.y - p.y).signum());
            if let Some(last) = cells.last_mut() {
                last.1 |= arm(p, q);
            }
            cells.push((q, arm(q, p)));
            p = q;
        }
    }
    cells
}

/// The lines from the seats with a job on a backdoor to that backdoor's door, as the
/// mock runs keepalive's: out of the seat's box into the column inside SEATS' divider,
/// along a free row of the hall (over the reader for a door beside its top half, under it
/// for the rest), down the hall's first column to the door's row and through PORTS'
/// divider to the door. Each takes the first way that runs along no other's cells.
fn lines(view: &RigView) -> Vec<JobLine> {
    let room = view.room;
    let f = room.floor;
    let (seat_col, hall_col) = (room.seats_room.x + 1, room.hall.x + 1);
    // The rows a line may cross the hall in: over the reader, and under it to the LINE.
    let (top, open): (Vec<i32>, Vec<i32>) = match room.slots.first() {
        Some(s) => (
            (f.y + 2..s.y).rev().collect(),
            (s.bottom() + 2..f.bottom() - 2).collect(),
        ),
        None => (Vec::new(), (f.y + 2..f.bottom() - 2).collect()),
    };
    let mut taken: HashMap<Pos, u8> = HashMap::new();
    let mut out = Vec::new();
    for (bx, seat) in room.seats.iter().zip(&view.seats) {
        let Some(i) = seat.as_ref().and_then(|s| s.door) else {
            continue;
        };
        let (Some(door), Some(dv)) = (room.doors.get(i), view.doors.get(i)) else {
            continue;
        };
        let tip_x = (f.x + 3 + dv.name.chars().count() as i32).min(room.hall.x - 1);
        let tip = Pos::new(tip_x, door.y);
        let mid = bx.y + bx.h / 2;
        let mut entries: Vec<i32> = (bx.y + 1..bx.bottom() - 1).collect();
        entries.sort_by_key(|e| (e - mid).abs());
        let cost = |r: &i32| {
            let e = entries.iter().map(|e| (e - r).abs()).min().unwrap_or(0);
            e + (r - door.y).abs()
        };
        let mut under = open.clone();
        under.sort_by_key(cost);
        let over = room.slots.first().is_some_and(|s| door.y <= s.y + s.h / 2);
        let rows: Vec<i32> = match over {
            true => top.iter().chain(&under).copied().collect(),
            false => under.iter().chain(&top).copied().collect(),
        };
        let way = |r: i32, e: i32| JobLine {
            cells: trace(&[
                Pos::new(bx.x, e),
                Pos::new(seat_col, e),
                Pos::new(seat_col, r),
                Pos::new(hall_col, r),
                Pos::new(hall_col, door.y),
                tip,
            ]),
            tip,
        };
        let clear =
            |l: &JobLine| (l.cells.iter()).all(|(p, a)| taken.get(p).is_none_or(|t| t & a == 0));
        let Some(line) = (rows.iter())
            .flat_map(|r| entries.iter().map(move |e| (*r, *e)))
            .map(|(r, e)| way(r, e))
            .find(clear)
            .or_else(|| Some(way(*rows.first()?, *entries.first()?)))
        else {
            continue;
        };
        for (p, a) in &line.cells {
            *taken.entry(*p).or_default() |= a;
        }
        out.push(line);
    }
    out
}

/// The jobs' lines over the room, walls they cross drawn crossed, each with its pulse a
/// cell further along every beat of the Net's traffic.
fn jobs_lines(c: &mut Canvas, view: &RigView) {
    let room = view.room;
    let lines = lines(view);
    let mut arms: HashMap<Pos, u8> = HashMap::new();
    for line in &lines {
        for (p, a) in &line.cells {
            *arms.entry(*p).or_default() |= a;
        }
    }
    for (p, a) in arms {
        let a = if wall(room, p) { a | N | S } else { a };
        c.put(p, DOTTED[a as usize], CORE, LEVEL, false);
    }
    for (j, line) in lines.iter().enumerate() {
        c.put(line.tip, '◂', CORE, LEVEL, true);
        let path: Vec<Pos> = (line.cells.iter())
            .map(|(p, _)| *p)
            .filter(|p| *p != line.tip && !wall(room, *p))
            .collect();
        if !path.is_empty() {
            let at = (view.tick / PACKET_TICKS + 7 * j as u64) % path.len() as u64;
            c.put(path[at as usize], '●', CORE, LEVEL, true);
        }
    }
}

/// A wall of the room: its outer wall, a divider, a box.
fn wall(room: &RigRoom, p: Pos) -> bool {
    (room.construct.tiles.get(p)).is_some_and(|t| t.terrain == Terrain::NetWall)
}

/// A colour as the look takes it; a named one as white.
fn rgb(c: Color) -> Rgb {
    match c {
        Color::Rgb(r, g, b) => Rgb(r, g, b),
        _ => WHITE,
    }
}

/// The buffer seen through the camera: the room's tiles at their cells.
struct Canvas<'b> {
    buf: &'b mut Buffer,
    area: Rect,
    origin: Pos,
}

impl Canvas<'_> {
    fn put(&mut self, p: Pos, ch: char, fg: Rgb, bg: Rgb, bold: bool) {
        if let Some((sx, sy)) = tile_to_screen(self.area, self.origin, p) {
            put(self.buf, sx, sy, ch, fg, bg, bold);
        }
    }

    fn text(&mut self, p: Pos, s: &str, fg: Rgb, bg: Rgb, bold: bool) {
        for (i, ch) in s.chars().enumerate() {
            self.put(p.offset(i as i32, 0), ch, fg, bg, bold);
        }
    }

    fn fill(&mut self, r: TileRect, bg: Rgb) {
        for p in r.iter() {
            self.put(p, ' ', TEXT, bg, false);
        }
    }

    /// `s` on the floor centred on `r`'s columns, `dy` rows under it.
    fn under(&mut self, r: TileRect, dy: i32, s: &str, fg: Rgb) {
        let x = r.x + (r.w - s.chars().count() as i32).div_euclid(2);
        self.text(Pos::new(x, r.bottom() + dy), s, fg, LEVEL, false);
    }

    /// `r`'s edge in `set` (corners, then the straights).
    fn boxed(&mut self, r: TileRect, set: [char; 6], fg: Rgb, bg: Rgb, bold: bool) {
        let [tl, tr, bl, br, h, v] = set;
        let (x1, y1) = (r.right() - 1, r.bottom() - 1);
        for x in r.x + 1..x1 {
            self.put(Pos::new(x, r.y), h, fg, bg, bold);
            self.put(Pos::new(x, y1), h, fg, bg, bold);
        }
        for y in r.y + 1..y1 {
            self.put(Pos::new(r.x, y), v, fg, bg, bold);
            self.put(Pos::new(x1, y), v, fg, bg, bold);
        }
        self.put(Pos::new(r.x, r.y), tl, fg, bg, bold);
        self.put(Pos::new(x1, r.y), tr, fg, bg, bold);
        self.put(Pos::new(r.x, y1), bl, fg, bg, bold);
        self.put(Pos::new(x1, y1), br, fg, bg, bold);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::rigroom::testutil::the_mocks_rig;

    /// The mock's map pane: 79 by 30.
    const PANE: Rect = Rect::new(0, 0, 79, 30);

    /// The room drawn into the mock's pane, centred as the map's camera centres it.
    fn paint(st: &AppState) -> (Buffer, Pos) {
        let mut view = crate::rigroom::view(st).expect("the room");
        let size = view.room.construct.size;
        view.origin = Pos::new(
            size.w as i32 / 2 - PANE.width as i32 / 2,
            size.h as i32 / 2 - PANE.height as i32 / 2,
        );
        let mut buf = Buffer::empty(PANE);
        draw(&mut buf, PANE, &view);
        (buf, view.origin)
    }

    fn row(buf: &Buffer, y: u16, xs: std::ops::Range<u16>) -> String {
        xs.map(|x| buf[(x, y)].symbol().to_string()).collect()
    }

    fn text(buf: &Buffer) -> String {
        (0..buf.area.height)
            .map(|y| row(buf, y, 0..buf.area.width))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn name(st: &AppState, b: citygen::BuildingId) -> String {
        crate::net::building_name(st, b).chars().take(14).collect()
    }

    #[test]
    fn the_room_draws_its_parts() {
        let (st, held) = the_mocks_rig();
        let (buf, _) = paint(&st);
        let text = text(&buf);
        for want in [
            "FIREWALL · sandbag ▮▯▯",
            "PORTS",
            "READER · 3 slots",
            "SEATS",
            "◉ sift",
            "◉ keepalive",
            "held · keepalive",
            "TERMINAL",
            "[r] shell",
            "▸ the grid · your block",
        ] {
            assert!(text.contains(want), "{want}\n{text}");
        }
        for b in held {
            assert!(text.contains(&name(&st, b)), "{}\n{text}", name(&st, b));
        }
        assert_eq!(text.matches("free line").count(), 2, "{text}");
    }

    #[test]
    fn a_killed_job_leaves_its_seat_and_line() {
        let (mut st, [kiroflux, ..]) = the_mocks_rig();
        let room = crate::rigroom::room(&st).unwrap().clone();
        let at = |origin: Pos, p: Pos| ((p.x - origin.x) as u16, (p.y - origin.y) as u16);
        let door = crate::rigroom::door_lines(&st)
            .iter()
            .position(|b| *b == Some(kiroflux))
            .map(|i| room.doors[i])
            .expect("kiroflux's door");
        // PORTS inside its walls, and the row under the door's name.
        let ports = |buf: &Buffer, origin: Pos| {
            let (x0, y0) = at(origin, Pos::new(room.floor.x + 1, room.floor.y + 1));
            let (x1, y1) = at(origin, Pos::new(room.hall.x, room.floor.bottom() - 1));
            (y0..y1)
                .map(|y| row(buf, y, x0..x1))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let under = |buf: &Buffer, origin: Pos| {
            let (x, y) = at(origin, Pos::new(room.floor.x + 2, door.y + 1));
            row(buf, y, x..x + 17)
        };
        let (buf, origin) = paint(&st);
        assert_eq!(under(&buf, origin).trim(), "held · keepalive");
        assert!(ports(&buf, origin).contains('◂'));

        let n = (st.backdoors.jobs.running.iter())
            .find(|j| j.kind.name() == "keepalive")
            .map(|j| j.n)
            .unwrap();
        crate::rig::jobs::kill(&mut st, n).unwrap();
        let (buf, origin) = paint(&st);
        let text = text(&buf);
        assert!(text.contains("free seat"), "{text}");
        assert!(!text.contains("◉ keepalive"), "{text}");
        assert!(under(&buf, origin).trim().ends_with(" left"), "{text}");
        assert!(!ports(&buf, origin).contains('◂'), "{text}");
    }
}
