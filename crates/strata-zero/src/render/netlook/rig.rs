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
/// MASTIFF's ground, a broken plate of its coat, its leash's tint and dashed edge, the
/// tiles it bites.
const ICE_BG: Rgb = Rgb(0x2a, 0x07, 0x10);
const BROKEN: Rgb = Rgb(0x8a, 0x22, 0x36);
const TINT: Rgb = Rgb(0x16, 0x08, 0x0c);
const LEASH: Rgb = Rgb(0x6a, 0x1a, 0x28);
const BITE: Rgb = Rgb(0xff, 0x40, 0x60);
const BITE_BG: Rgb = Rgb(0x4a, 0x0c, 0x18);
/// MASTIFF's coat, a plate a cell round its core, in `mastiff::PLATE_AT`'s order.
const COAT: [char; 8] = ['▛', '▀', '▜', '▐', '▟', '▄', '▙', '▌'];
/// What MASTIFF leaves where its core broke, by its 3x3's top-left.
const GRIT: [(i32, i32, char); 7] = [
    (0, 0, '▝'),
    (2, 0, '░'),
    (1, 1, '·'),
    (0, 2, '▖'),
    (2, 2, '▗'),
    (3, 1, '·'),
    (-1, 1, '·'),
];

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
    /// The seat whose beam is out at MASTIFF: its turn in the fight.
    pub beam: Option<usize>,
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
    /// A black chip with MASTIFF asleep round it.
    pub coiled: bool,
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
    mastiff(&mut c, view);
    c.put(view.you, '@', YOU, you_glow(room, view.you), true);
}

/// The ground under you at `you` in the room: the bite's red on one of MASTIFF's marks,
/// else the Net's glow. The map draws your `@` with it too.
pub fn you_glow(room: &RigRoom, you: Pos) -> Rgb {
    let bitten = (room.mastiff.as_ref())
        .filter(|m| m.core > 0)
        .and_then(|m| m.bite)
        .is_some_and(|b| b.contains(you));
    if bitten { BITE_BG } else { YOU_GLOW }
}

/// MASTIFF, as the mock's round 3 draws it. Asleep: coiled round each black chip, its
/// leash tinted round the slot. Awake: its leash tinted with its open sides dashed, a
/// chain down from its slot, its coat round its core (a broken plate dim grit), the
/// tiles it bites next turn and the beam of the seat on its turn. Broken: grit.
fn mastiff(c: &mut Canvas, view: &RigView) {
    let room = view.room;
    for (i, (r, slot)) in room.slots.iter().zip(&view.slots).enumerate() {
        if !slot.as_ref().is_some_and(|s| s.coiled) {
            continue;
        }
        if let Some(leash) = crate::rigroom::mastiff::leash_of(room, i) {
            tint(c, room, leash);
        }
        coat(
            c,
            Pos::new(r.x + r.w / 2 - 1, r.y + r.h / 2 - 1),
            &[true; 8],
        );
    }
    let Some(m) = room.mastiff.as_ref() else {
        return;
    };
    if m.core == 0 {
        for (dx, dy, ch) in GRIT {
            let p = m.pos.offset(dx, dy);
            if floor(room, p) && c.blank(p) {
                c.put(p, ch, BROKEN, LEVEL, false);
            }
        }
        return;
    }
    tint(c, room, m.leash);
    leash_edge(c, room, m.leash);
    // The chain to the slot it came off, while it stands under it.
    if let Some(r) = room.slots.get(m.slot) {
        let x = r.x + r.w / 2;
        if (m.pos.x..m.pos.x + 3).contains(&x) {
            for y in r.bottom() + 2..m.pos.y {
                c.put(Pos::new(x, y), '┇', FW, TINT, true);
            }
        }
    }
    if let Some(bite) = m.bite {
        for p in bite.iter().filter(|p| floor(room, *p)) {
            c.put(p, '×', BITE, BITE_BG, false);
        }
    }
    coat(c, m.pos, &m.plates);
    if let Some(n) = view.beam {
        beam(c, room, m, n);
    }
}

/// MASTIFF's 3x3 at `at`: its coat's whole plates red, the broken ones dim grit, its core
/// white.
fn coat(c: &mut Canvas, at: Pos, plates: &[bool; 8]) {
    for (i, (dx, dy)) in crate::rigroom::mastiff::PLATE_AT.iter().enumerate() {
        let p = at.offset(*dx, *dy);
        match plates[i] {
            true => c.put(p, COAT[i], FW, ICE_BG, true),
            false => c.put(p, '░', BROKEN, ICE_BG, false),
        }
    }
    c.put(at.offset(1, 1), '◆', WHITE, ICE_BG, true);
}

/// The leash's red over the floor in `r` and what stands on it, not the dividers' walls.
fn tint(c: &mut Canvas, room: &RigRoom, r: TileRect) {
    for p in r.iter() {
        let divider = (p.x == room.hall.x || p.x == room.seats_room.x) && wall(room, p);
        if !divider {
            c.retint(p, LEVEL, TINT);
        }
    }
}

/// The leash's dashed edge, as the mock draws it: on each blank floor cell of `r` whose
/// west, east or south neighbour off the leash is the hall (its floor, a doorway, the
/// next slot's box), a side's dash, or a corner where the south meets a side. Against a
/// divider's wall it stays undashed, and its north runs up to the reader undashed.
fn leash_edge(c: &mut Canvas, room: &RigRoom, r: TileRect) {
    let open =
        |p: Pos| !r.contains(p) && (floor(room, p) || room.slots.iter().any(|b| b.contains(p)));
    for p in r.iter().filter(|p| floor(room, *p)) {
        if !c.blank(p) {
            continue;
        }
        let [e, s, w] = [(1, 0), (0, 1), (-1, 0)].map(|(dx, dy)| open(p.offset(dx, dy)));
        let ch = match (w, e, s) {
            (false, false, false) => continue,
            (true, _, true) => '└',
            (false, true, true) => '┘',
            (_, _, true) => '┄',
            _ => '┆',
        };
        c.put(p, ch, LEASH, TINT, false);
    }
}

/// Seat `n`'s beam at MASTIFF: out of its box's west side, through the SEATS wall along
/// the box's middle row, turning up or down to MASTIFF's nearest row, to its tip by
/// MASTIFF's east side.
fn beam(c: &mut Canvas, room: &RigRoom, m: &crate::rigroom::mastiff::Mastiff, n: usize) {
    let Some(s) = room.seats.get(n) else {
        return;
    };
    let row = s.y + s.h / 2;
    let to = row.clamp(m.pos.y, m.pos.y + 2);
    let tip = Pos::new(m.pos.x + 3, to);
    c.over(Pos::new(s.x, row), '┤', CORE, false);
    if to == row {
        for x in tip.x + 1..s.x {
            c.over(Pos::new(x, row), '━', CORE, false);
        }
    } else {
        // The turn, a column east of the tip.
        let x = tip.x + 1;
        for bx in x + 1..s.x {
            c.over(Pos::new(bx, row), '━', CORE, false);
        }
        let (from_row, to_row) = if to < row {
            ('┗', '┓')
        } else {
            ('┏', '┛')
        };
        c.over(Pos::new(x, row), from_row, CORE, false);
        for y in to.min(row) + 1..to.max(row) {
            c.over(Pos::new(x, y), '┃', CORE, false);
        }
        c.over(Pos::new(x, to), to_row, CORE, false);
    }
    c.over(tip, '◂', CORE, true);
}

/// A floor tile of the room: what you and MASTIFF walk.
fn floor(room: &RigRoom, p: Pos) -> bool {
    (room.construct.tiles.get(p)).is_some_and(|t| t.is_walkable())
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
            // A free seat's box as far as it blocks, dim.
            None => {
                c.boxed(*r, ROUND, BOX, LEVEL, false);
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

    /// `ch` at `p` over the ground already there.
    fn over(&mut self, p: Pos, ch: char, fg: Rgb, bold: bool) {
        if let Some((sx, sy)) = tile_to_screen(self.area, self.origin, p) {
            let bg = match self.buf[(sx, sy)].bg {
                Color::Rgb(r, g, b) => Rgb(r, g, b),
                _ => LEVEL,
            };
            put(self.buf, sx, sy, ch, fg, bg, bold);
        }
    }

    /// Nothing drawn at `p` but its ground.
    fn blank(&self, p: Pos) -> bool {
        tile_to_screen(self.area, self.origin, p)
            .is_some_and(|(sx, sy)| self.buf[(sx, sy)].symbol() == " ")
    }

    /// `p`'s ground from `from` to `to`, what stands on it kept.
    fn retint(&mut self, p: Pos, from: Rgb, to: Rgb) {
        if let Some((sx, sy)) = tile_to_screen(self.area, self.origin, p) {
            let cell = &mut self.buf[(sx, sy)];
            if cell.bg == from.to_color() {
                cell.set_bg(to.to_color());
            }
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
    use crate::rigroom::mastiff::{self, Mastiff};
    use crate::rigroom::testutil::{BLACK_SLOT, a_black_chip, the_mocks_rig};

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

    #[test]
    fn a_free_seat_is_drawn_as_far_as_it_blocks() {
        let (mut st, _) = the_mocks_rig();
        let n = (st.backdoors.jobs.running.iter())
            .find(|j| j.kind.name() == "keepalive")
            .map(|j| j.n)
            .unwrap();
        crate::rig::jobs::kill(&mut st, n).unwrap();
        let room = room_of(&st);
        let free = room.seats[1];
        let (buf, origin) = paint(&st);
        // Its box's corners on the seat's own corners: no wall drawn as floor.
        let corner = |p: Pos| {
            let (x, y) = at(origin, p);
            buf[(x, y)].symbol().to_string()
        };
        assert_eq!(corner(Pos::new(free.x, free.y)), "╭", "{}", text(&buf));
        assert_eq!(corner(Pos::new(free.right() - 1, free.y)), "╮");
        assert_eq!(corner(Pos::new(free.x, free.bottom() - 1)), "╰");
        assert_eq!(corner(Pos::new(free.right() - 1, free.bottom() - 1)), "╯");
        for p in free.iter() {
            assert!(
                !room.construct.tiles[p].is_walkable(),
                "the seat's tiles block"
            );
        }
    }

    /// Where tile `p` lands in the pane painted from `origin`.
    fn at(origin: Pos, p: Pos) -> (u16, u16) {
        ((p.x - origin.x) as u16, (p.y - origin.y) as u16)
    }

    fn room_of(st: &AppState) -> RigRoom {
        crate::rigroom::room(st).expect("the room").clone()
    }

    fn mastiff_of(st: &AppState) -> Mastiff {
        room_of(st).mastiff.expect("MASTIFF out")
    }

    /// The three rows of MASTIFF's 3x3 as drawn, top to bottom.
    fn body(buf: &Buffer, origin: Pos, m: &Mastiff) -> String {
        (0..3)
            .map(|dy| {
                let (x, y) = at(origin, m.pos.offset(0, dy));
                row(buf, y, x..x + 3)
            })
            .collect()
    }

    const RED: Color = Color::Rgb(0xff, 0x28, 0x48);

    #[test]
    fn asleep_it_coils_round_the_chip() {
        let st = a_black_chip();
        let (buf, origin) = paint(&st);
        let slot = room_of(&st).slots[BLACK_SLOT];
        let (x, y) = at(origin, Pos::new(slot.x + slot.w / 2, slot.y + slot.h / 2));
        assert_eq!(row(&buf, y - 1, x - 1..x + 2), "▛▀▜", "{}", text(&buf));
        assert_eq!(row(&buf, y, x - 1..x + 2), "▌◆▐");
        assert_eq!(row(&buf, y + 1, x - 1..x + 2), "▙▄▟");
        for dx in 0..3 {
            assert_eq!(buf[(x - 1 + dx, y - 1)].fg, RED);
        }
        assert_eq!(
            buf[(x, y)].fg,
            Color::Rgb(0xe6, 0xf0, 0xf5),
            "the core white"
        );
        // Its tail under the box names it, in red; the ground round it is tinted.
        let (tx, ty) = at(origin, Pos::new(slot.x, slot.bottom() + 1));
        assert_eq!(row(&buf, ty, tx..tx + 7).trim(), "MASTIFF");
        assert_eq!(buf[(x, ty)].fg, RED);
        let (gx, gy) = at(origin, Pos::new(slot.x - 1, slot.bottom() + 3));
        assert_eq!(
            buf[(gx, gy)].bg,
            Color::Rgb(0x16, 0x08, 0x0c),
            "its leash tinted"
        );
    }

    /// MASTIFF moved to `to`, as its steps would take it; you beside it on the west.
    fn walked_to(st: &mut AppState, to: Pos) {
        let room = st.constructs.rig.as_mut().unwrap();
        room.mastiff.as_mut().expect("MASTIFF out").pos = to;
        st.player.as_mut().unwrap().pos = to.offset(-1, 1);
    }

    #[test]
    fn the_fight_draws_its_board() {
        let mut st = a_black_chip();
        mastiff::wake(&mut st, BLACK_SLOT).unwrap();
        // Off its slot, where the mock's turn 4 has it.
        let to = mastiff_of(&st).pos.offset(0, 3);
        walked_to(&mut st, to);
        let west = Pos::new(to.x - 3, to.y + 1);
        for _ in 0..3 {
            mastiff::hit(&mut st, west);
        }
        mastiff::turn(&mut st);
        let m = mastiff_of(&st);
        assert_eq!(m.pos, to, "beside you it doesn't step");
        let bite = m.bite.expect("a bite marked");
        // You step off the marks, still on the leash.
        st.player.as_mut().unwrap().pos = Pos::new(bite.x - 1, bite.y + 1);
        assert!(m.leash.contains(Pos::new(bite.x - 1, bite.y + 1)));
        let (buf, origin) = paint(&st);
        let drawn = body(&buf, origin, &m);
        assert_eq!(drawn.matches('░').count(), 3, "{drawn}");
        assert_eq!(drawn, "░▀▜░◆▐░▄▟", "the west side's plates broken");
        let (x, y) = at(origin, m.pos);
        assert_eq!(
            buf[(x, y)].fg,
            Color::Rgb(0x8a, 0x22, 0x36),
            "a broken plate dim"
        );
        assert_eq!(buf[(x + 1, y)].fg, RED);
        let text = text(&buf);
        assert_eq!(text.matches('×').count(), 9, "{text}");
        assert!(text.contains("awake"), "the slot's tail: {text}");
        assert!(
            text.contains("◉ attack"),
            "a free seat in the fight: {text}"
        );
        // The footer reads the fight.
        let map = crate::screens::explore::MapScreen::walking(&mut st);
        let screen = crate::screens::testutil::buffer_text(&crate::screens::shots::draw_stack(
            &st,
            &[&map],
            (120, 40),
        ));
        for want in ["ENGAGED", "seat 2", "MASTIFF · black ICE · coat 5/8"] {
            assert!(screen.contains(want), "{want}\n{screen}");
        }
    }

    #[test]
    fn a_seats_beam_draws_on_its_turn() {
        let mut st = a_black_chip();
        mastiff::wake(&mut st, BLACK_SLOT).unwrap();
        let seat = room_of(&st).seats[1];
        let (_, origin) = paint(&st);
        let (x, y) = at(origin, Pos::new(seat.x, seat.y + seat.h / 2));
        let (buf, _) = paint(&st);
        assert_ne!(row(&buf, y, x..x + 1), "┤", "not on your turn");
        let e = st.net.engaged.as_mut().unwrap();
        e.turn = (e.order.iter())
            .position(|a| *a == crate::net::Actor::Seat(1))
            .expect("seat 2 in the fight");
        let (buf, _) = paint(&st);
        assert_eq!(row(&buf, y, x..x + 1), "┤", "{}", text(&buf));
        let m = mastiff_of(&st);
        let line = row(&buf, y, 0..x);
        assert!(line.trim_end().ends_with('━'), "{line}");
        assert!(text(&buf).contains('◂'), "its tip at MASTIFF");
        let (mx, _) = at(origin, Pos::new(m.pos.x + 3, m.pos.y));
        let tip = (0..buf.area.height).find(|ty| row(&buf, *ty, mx..mx + 1) == "◂");
        assert!(tip.is_some(), "the tip beside MASTIFF's east side");
    }

    #[test]
    fn its_grit_stays_where_it_broke() {
        let mut st = a_black_chip();
        mastiff::wake(&mut st, BLACK_SLOT).unwrap();
        let to = mastiff_of(&st).pos.offset(0, 3);
        walked_to(&mut st, to);
        let m = mastiff_of(&st);
        let west = Pos::new(m.pos.x - 3, m.pos.y + 1);
        for _ in 0..10 {
            mastiff::hit(&mut st, west);
        }
        assert!(st.net.engaged.is_none(), "the fight is won");
        st.player.as_mut().unwrap().pos = Pos::new(m.pos.x - 3, m.pos.y);
        let (buf, origin) = paint(&st);
        let drawn = body(&buf, origin, &m);
        for grit in ['▝', '░', '▖', '▗', '·'] {
            assert!(drawn.contains(grit), "{grit} in {drawn}");
        }
        assert!(!drawn.contains('◆'), "{drawn}");
        let text = text(&buf);
        assert!(!text.contains('×') && !text.contains('┆'), "{text}");
        let slot = room_of(&st).slots[BLACK_SLOT];
        let (x, y) = at(origin, Pos::new(slot.x + slot.w / 2, slot.y + slot.h / 2));
        assert_eq!(row(&buf, y, x..x + 1), "¤", "a plain credchip now");
        let (tx, ty) = at(origin, Pos::new(slot.x, slot.bottom() + 1));
        assert_eq!(row(&buf, ty, tx..tx + 7).trim(), "¤4,800");
    }

    #[test]
    fn the_leash_is_dashed_as_the_mock_draws_it() {
        let mut st = a_black_chip();
        mastiff::wake(&mut st, BLACK_SLOT).unwrap();
        let r = room_of(&st);
        let leash = mastiff_of(&st).leash;
        let (buf, origin) = paint(&st);
        let cell = |p: Pos| {
            let (x, y) = at(origin, p);
            (buf[(x, y)].symbol().to_string(), buf[(x, y)].bg)
        };
        let tint = Color::Rgb(0x16, 0x08, 0x0c);
        let (x0, y1) = (leash.x, leash.bottom() - 1);
        // The west side dashed from the top, the south along the bottom.
        for y in leash.y..y1 {
            assert_eq!(cell(Pos::new(x0, y)), ("┆".into(), tint), "west at {y}");
        }
        assert_eq!(cell(Pos::new(x0, y1)).0, "└");
        for x in x0 + 1..leash.right() {
            assert_eq!(cell(Pos::new(x, y1)).0, "┄", "south at {x}");
        }
        // The north runs up to the reader undashed; the READER title stays untinted.
        assert_eq!(cell(Pos::new(x0 + 1, leash.y)), (" ".into(), tint));
        for x in r.hall.x + 1..r.hall.right() {
            assert_ne!(cell(Pos::new(x, r.floor.y + 1)).1, tint, "title row at {x}");
        }
        // The next slot's box is off the leash.
        let next = r.slots[BLACK_SLOT - 1];
        for p in next.iter() {
            assert_ne!(cell(p).1, tint, "{p:?}");
        }
    }

    #[test]
    fn a_leash_through_the_divider_tints_its_doorway_not_its_wall() {
        let mut st = a_black_chip();
        st.character.reader.swap(0, BLACK_SLOT);
        let r = room_of(&st);
        st.player.as_mut().unwrap().pos = crate::rigroom::layout::front(r.slots[0]);
        mastiff::wake(&mut st, 0).unwrap();
        let m = mastiff_of(&st);
        assert_eq!(m.leash.x, r.hall.x, "the leash takes in the west divider");
        let (buf, origin) = paint(&st);
        let tint = Color::Rgb(0x16, 0x08, 0x0c);
        let floor = |p: Pos| r.construct.tiles.get(p).is_some_and(|t| t.is_walkable());
        let mut doorway = 0;
        for y in m.leash.y + 1..m.leash.bottom() {
            let p = Pos::new(r.hall.x, y);
            let (sx, sy) = at(origin, p);
            let cell = &buf[(sx, sy)];
            if floor(p) {
                doorway += 1;
                assert_eq!((cell.symbol(), cell.bg), ("┆", tint), "{p:?}");
            } else {
                assert_eq!(cell.symbol(), "│", "{p:?}");
                assert_ne!(cell.bg, tint, "no tint on the divider's wall: {p:?}");
            }
        }
        assert!(doorway > 0, "the doorway is on the leash");
        // Its open south side dashed.
        let (sx, sy) = at(origin, Pos::new(m.leash.x + 3, m.leash.bottom() - 1));
        assert_eq!(row(&buf, sy, sx..sx + 1), "┄", "{}", text(&buf));
    }
}
