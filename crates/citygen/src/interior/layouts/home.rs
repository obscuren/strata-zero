//! The player's flat ("layout A"): a bedroom with the rig by the window, a living
//! room and kitchen, a short hall to the shower room and the storage room.

use super::Builder;
use crate::geom::{Pos, Rect};
use crate::tile::{Feature, Furniture, Terrain};

/// The front door's column in the flat, over the living room.
pub(super) const HOME_DOOR: i32 = 17;

/// The flat's 35x7 interior, the walkway side on top, the windows below: `#` wall,
/// `+` door, `W` shower, `H` shelf, `L` locker, `B` bed, `D` desk, `T` the rig,
/// `C` couch, `V` the holo set, `O` table, `S` stool, `K` counter.
const LAYOUT: [&str; 7] = [
    "W..#HHHH.LLL#......................",
    "...#........#......................",
    "#+#####+#####......................",
    "............+...........SOS.......K",
    "#########+###...V.................K",
    "BB..........#.....................K",
    "BB....DT....#..CCCC.........KKKKKKK",
];

impl Builder<'_> {
    /// Stamps the flat into `rect`, its front door at `door` on the walkway wall,
    /// and windows the outer wall along it.
    pub(super) fn home_flat(&mut self, rect: Rect, door: Pos) {
        for (j, row) in LAYOUT.iter().enumerate() {
            for (i, c) in row.chars().enumerate() {
                let p = Pos::new(rect.x + i as i32, rect.y + j as i32);
                let piece = match c {
                    '#' | '+' => {
                        self.wall_line(p, p);
                        if c == '+' {
                            self.door(p);
                        }
                        continue;
                    }
                    'T' => {
                        if self.open(p) {
                            self.tiles[p].feature = Some(Feature::Terminal);
                        }
                        continue;
                    }
                    'W' => Furniture::Shower,
                    'H' => Furniture::Shelf,
                    'L' => Furniture::Locker,
                    'B' => Furniture::Bed,
                    'D' => Furniture::Desk,
                    'C' => Furniture::Couch,
                    'V' => Furniture::Holo,
                    'O' => Furniture::Table,
                    'S' => Furniture::Stool,
                    'K' => Furniture::Counter,
                    _ => continue,
                };
                self.put(p, piece);
            }
        }
        self.door(door);
        self.clear.insert(door.offset(0, 1));
        let y = rect.bottom();
        for x in rect.x..rect.right() {
            let p = Pos::new(x, y);
            if (x + y) % 2 == 0 && self.tiles[p].terrain == Terrain::Wall {
                self.tiles[p].terrain = Terrain::Window;
            }
        }
    }
}
