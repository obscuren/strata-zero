//! The room's layout, a pure function of the rig's counts: the firewall's ring, the
//! floor's double wall, PORTS on the west, the hall with the reader's slots, SEATS on
//! the east, the terminal and the trunk's end. Later tasks walk it, draw it and fill it.

use citygen::{BuildingId, Construct, Grid, Pos, Rect, Size, Terrain, Tile};

/// What the room is laid out for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counts {
    /// Doors in PORTS' outer wall.
    pub lines: usize,
    pub seats: u8,
    /// The reader's slots.
    pub slots: usize,
    /// The firewall's strength; 0 has no ring.
    pub firewall: u8,
}

#[derive(Clone, Debug)]
pub struct RigRoom {
    /// Hand-built like `lot_construct`: NetWall, NetFloor, NetGate for the doors and
    /// the trunk's end.
    pub construct: Construct,
    /// The firewall ring's outer edge; None at firewall 0.
    pub ring: Option<Rect>,
    /// The floor's double wall.
    pub floor: Rect,
    pub ports: Rect,
    pub hall: Rect,
    pub seats_room: Rect,
    /// A door per line in PORTS' outer wall, top to bottom.
    pub doors: Vec<Pos>,
    /// The backdoor each door was laid out for, as the room was built (`rigroom::build`);
    /// `None` for a free line. `rigroom::door_lines` keeps only those still standing.
    pub door_for: Vec<Option<BuildingId>>,
    /// A 7x5 box per reader slot, left to right.
    pub slots: Vec<Rect>,
    /// A box per core seat, top to bottom.
    pub seats: Vec<Rect>,
    pub terminal: Pos,
    /// The trunk's end, in the floor's bottom wall.
    pub trunk: Pos,
    /// Where you land: in the hall, under the slots.
    pub land: Pos,
}

/// Rows past which doors stand 3 apart instead of 4.
const MAX_DOOR_ROWS: usize = 40; // the door rows 4 apart may take before they go 3 apart
/// PORTS up to and with the hall's west divider: 18 columns inside, as the mock has it.
const PORTS_W: i32 = 19;
const SEATS_W: i32 = 20;
const SLOT_W: i32 = 7;
const SLOT_H: i32 = 5;
const SEAT_W: i32 = 16;
const SEAT_H: i32 = 5;

fn tile(terrain: Terrain) -> Tile {
    Tile::new(terrain)
}

/// The tile under a box's middle, where you stand at it.
#[allow(dead_code)] // the reader's summon stands you here (#104's next tasks)
pub fn front(r: Rect) -> Pos {
    Pos::new(r.x + r.w / 2, r.bottom())
}

/// Within one step of the box.
#[allow(dead_code)] // the reader's summon asks it (#104's next tasks)
pub fn at_box(here: Pos, r: Rect) -> bool {
    here.x >= r.x - 1 && here.x <= r.right() && here.y >= r.y - 1 && here.y <= r.bottom()
}

pub fn lay_out(c: Counts, building: BuildingId) -> RigRoom {
    let seats_n = c.seats as usize;
    let step: i32 = if 5 + 4 * c.lines + 2 <= MAX_DOOR_ROWS {
        4
    } else {
        3
    };
    let door_rows = 5 + step as usize * c.lines + 2;
    let fh = [27, door_rows, 5 + 6 * seats_n + 6]
        .into_iter()
        .max()
        .unwrap() as i32;
    // The divider, a free column, the slots 9 apart, a free column: a line runs down each
    // free column past the slots (`render::netlook::rig`).
    let hall_w = 28.max(9 * c.slots as i32 + 1);
    let fw = PORTS_W + hall_w + SEATS_W;
    let size = Size {
        w: (fw + 4) as u32,
        h: (fh + 3) as u32,
    };

    let floor = Rect::new(2, 1, fw, fh);
    let ports = Rect::new(floor.x, floor.y, PORTS_W, fh);
    let hall = Rect::new(ports.right(), floor.y, hall_w, fh);
    let seats_room = Rect::new(hall.right(), floor.y, SEATS_W, fh);

    let mut tiles = Grid::filled(size, tile(Terrain::NetWall));
    // The floor inside the double wall, then the dividers back in as walls.
    tiles.fill_rect(
        Rect::new(floor.x + 1, floor.y + 1, fw - 2, fh - 2),
        tile(Terrain::NetFloor),
    );
    // Doorways in each divider: three rows at the middle, two above the bottom wall.
    let mid = floor.y + fh / 2;
    for x in [hall.x, seats_room.x] {
        for y in floor.y + 1..floor.bottom() - 1 {
            let gap = (mid - 1..=mid + 1).contains(&y) || y >= floor.bottom() - 3;
            if !gap {
                tiles[Pos::new(x, y)] = tile(Terrain::NetWall);
            }
        }
    }

    let doors: Vec<Pos> = (0..c.lines as i32)
        .map(|i| Pos::new(floor.x, floor.y + 5 + step * i))
        .collect();
    for &d in &doors {
        tiles[d] = tile(Terrain::NetGate);
    }

    let slots: Vec<Rect> = (0..c.slots as i32)
        .map(|i| Rect::new(hall.x + 2 + 9 * i, floor.y + 4, SLOT_W, SLOT_H))
        .collect();
    let seats: Vec<Rect> = (0..c.seats as i32)
        .map(|i| Rect::new(seats_room.x + 2, floor.y + 5 + 6 * i, SEAT_W, SEAT_H))
        .collect();
    for r in slots.iter().chain(&seats) {
        tiles.fill_rect(*r, tile(Terrain::NetWall));
    }
    let terminal = Pos::new(
        seats_room.x + 3,
        seats.last().map_or(floor.y + 5, |s| s.bottom() + 3),
    );
    tiles[terminal] = tile(Terrain::NetWall);

    let trunk = Pos::new(hall.x + hall_w / 2, floor.bottom() - 1);
    tiles[trunk] = tile(Terrain::NetGate);
    let land = Pos::new(trunk.x, floor.y + 12);

    let construct = Construct {
        building,
        security: 0,
        size,
        tiles,
        light: Grid::filled(size, 255),
        entry: trunk,
        rooms: Vec::new(),
        nodes: Vec::new(),
        guards: Vec::new(),
        levels: Vec::new(),
        devices: Vec::new(),
    };
    RigRoom {
        construct,
        ring: (c.firewall > 0).then(|| Rect::new(0, 0, fw + 4, fh + 2)),
        floor,
        ports,
        hall,
        seats_room,
        door_for: vec![None; doors.len()],
        doors,
        slots,
        seats,
        terminal,
        trunk,
        land,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    const B: BuildingId = BuildingId(7);
    const CASES: [(usize, u8, usize); 2] = [(5, 2, 3), (8, 6, 4)];

    fn room(lines: usize, seats: u8, slots: usize) -> RigRoom {
        lay_out(
            Counts {
                lines,
                seats,
                slots,
                firewall: 1,
            },
            B,
        )
    }

    fn walkable(r: &RigRoom, p: Pos) -> bool {
        r.construct.tiles.get(p).is_some_and(|t| t.is_walkable())
    }

    fn reach(r: &RigRoom) -> BTreeSet<Pos> {
        let mut seen = BTreeSet::from([r.land]);
        let mut todo = vec![r.land];
        while let Some(p) = todo.pop() {
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let q = Pos::new(p.x + dx, p.y + dy);
                if walkable(r, q) && seen.insert(q) {
                    todo.push(q);
                }
            }
        }
        seen
    }

    fn beside(seen: &BTreeSet<Pos>, b: Rect) -> bool {
        b.iter().any(|p| {
            [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|(dx, dy)| seen.contains(&Pos::new(p.x + dx, p.y + dy)))
        })
    }

    #[test]
    fn the_civitas_room_is_the_mocks() {
        let r = lay_out(
            Counts {
                lines: 5,
                seats: 2,
                slots: 3,
                firewall: 1,
            },
            B,
        );
        let ys: Vec<i32> = r.doors.iter().map(|d| d.y - r.floor.y).collect();
        assert_eq!(ys, [5, 9, 13, 17, 21]);
        assert!(r.doors.iter().all(|d| d.x == r.floor.x));
        assert_eq!((r.slots.len(), r.seats.len()), (3, 2));
        assert!(r.ring.is_some());
        // The mock's columns: PORTS' divider, the slots, SEATS' divider, the trunk.
        let x = |x: i32| x - r.floor.x;
        assert_eq!(x(r.hall.x), 19);
        let slots: Vec<i32> = r.slots.iter().map(|s| x(s.x)).collect();
        assert_eq!(slots, [21, 30, 39]);
        assert_eq!(x(r.seats_room.x), 47);
        assert_eq!(r.floor.w, 67);
        assert_eq!(x(r.trunk.x), 33);
    }

    #[test]
    fn the_civitas_room_fits_the_pane() {
        let r = room(5, 2, 3);
        assert_eq!(r.floor.h, 27);
        assert_eq!(r.ring.unwrap().h, 29);
        assert!(r.construct.size.h <= 30);
        assert_eq!(r.terminal.x - r.seats_room.x, 3);
        assert_eq!(r.terminal.y - r.floor.y, 19);
        assert_eq!(r.trunk.y - r.floor.y, 26);
    }

    #[test]
    fn no_firewall_no_ring() {
        let r = lay_out(
            Counts {
                lines: 5,
                seats: 2,
                slots: 3,
                firewall: 0,
            },
            B,
        );
        assert_eq!(r.ring, None);
    }

    #[test]
    fn the_room_fits_eight_lines_and_six_seats() {
        let r = room(8, 6, 4);
        assert_eq!(r.doors.iter().collect::<BTreeSet<_>>().len(), 8);
        assert_eq!((r.seats.len(), r.slots.len()), (6, 4));
        let inside = Rect::new(r.floor.x + 1, r.floor.y + 1, r.floor.w - 2, r.floor.h - 2);
        assert!(inside.contains(r.terminal));
        for b in r.seats.iter().chain(&r.slots) {
            assert!(!b.contains(r.terminal));
            assert!(inside.contains(Pos::new(b.right() - 1, b.bottom() - 1)));
        }
        assert!(r.hall.w > 9 * 4);
    }

    #[test]
    fn every_part_is_a_walk_from_the_landing() {
        for (l, s, n) in CASES.into_iter().chain([(12, 0, 1)]) {
            let r = room(l, s, n);
            let seen = reach(&r);
            for d in &r.doors {
                assert!(seen.contains(d), "door {d:?}");
            }
            for b in &r.slots {
                assert!(seen.contains(&front(*b)), "slot {b:?}");
            }
            for b in &r.seats {
                assert!(beside(&seen, *b), "seat {b:?}");
            }
            assert!(beside(&seen, Rect::new(r.terminal.x, r.terminal.y, 1, 1)));
            assert!(seen.contains(&r.trunk));
        }
    }

    #[test]
    fn boxes_block_doors_step() {
        let r = room(5, 2, 3);
        for b in r.slots.iter().chain(&r.seats) {
            assert!(b.iter().all(|p| !walkable(&r, p)));
        }
        assert!(!walkable(&r, r.terminal));
        for p in r.doors.iter().chain([&r.trunk]) {
            assert_eq!(r.construct.tiles[*p].terrain, Terrain::NetGate);
        }
        assert!(at_box(front(r.slots[0]), r.slots[0]));
        assert!(!at_box(r.land, r.seats[0]));
    }
}
