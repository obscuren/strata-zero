//! The ripperdocs' floors: one hand-laid grid per front (the laundromat's two, street and
//! cellar), drawn with the street side at the bottom. The clinic's floor is its grid,
//! turned to face the building's main door (`FloorPlan::clinic`).

use super::Builder;
use crate::clinic::Front;
use crate::geom::{Pos, Rect};
use crate::interior::floor::{Role, Room, RoomKind, Station};
use crate::interior::plan::turn_pos;
use crate::tile::{Feature, Furniture, Terrain};
use std::collections::HashSet;

// `#` wall, `w` window, `E` the front door, `+` a door, `%` a locked door (its room is
// private), `.` floor, `*` a lamp, `g` a drain, `v` `^` the stairs down and up, `X` the operating chair, `d` the doc's spot
// beside it; `K` counter, `S` stool, `H` shelf, `M` machine, `B` bed, `L` locker,
// `C` couch, `O` table, `D` desk, `N` bench, `V` vending machine.

/// Behind the noodle bar: the counter and stools on the street, the kitchen and its
/// locked pantry behind, the back room past it.
const NOODLE_BAR: [&str; 10] = [
    "###############################",
    "#L..D.....L..#H.M.M.M..#LL.HH.#",
    "#.....Xd.....#.........#......#",
    "#.*...g......+.......*.###%####",
    "#L...........#................#",
    "##############KKKKKKKKKKKKKKK.#",
    "#.......*.....S.S.S.S.S.S.S...#",
    "#.............................#",
    "#..O.S....O.S.....O.S.........#",
    "#w###w####w####E####w####w#####",
];

/// The laundromat: washers on the street floor and the stair at the back.
const LAUNDROMAT: [&str; 9] = [
    "########################",
    "#M.M.M.M.M.M..#H.......#",
    "#.............#....v...#",
    "#.N...N...*...+........#",
    "#.............#H.......#",
    "#M.M.M.M.M.M..#H.......#",
    "#.............##########",
    "#....O....N............#",
    "#w#w####E###w###########",
];

/// Its cellar: a locked stockroom of racked salvage, the surgery with a generator and
/// cots.
const CELLAR: [&str; 9] = [
    "########################",
    "#HHHH.....#M...M.......#",
    "#.........#........^...#",
    "#..*......%.....X......#",
    "#HHHH.....#.....dg...B.#",
    "#.........#..D.......B.#",
    "#HHHH.....#....*.......#",
    "#.........#.........L..#",
    "########################",
];

/// The shuttered dentist: the waiting room, the surgery, a locked recovery room at the
/// back.
const DENTIST: [&str; 7] = [
    "##################################",
    "#C.C.......D...#.LL.L..#B........#",
    "#..............#.......#.......*.#",
    "#..V.....*.....+...X...%.........#",
    "#C.C...........#...dgD.#B........#",
    "#..............#...*...#.........#",
    "#w####w####E######################",
];

/// The tattoo parlour: flash on the walls, benches, the till; the back room behind a
/// locker wall, a locked stock cupboard past it.
const TATTOO: [&str; 10] = [
    "##########################",
    "#H.H.H.H.#L..M....D.#.LL.#",
    "#........#..........#....#",
    "#.N.S..N.#.*....X...%....#",
    "#.S....S.+......dg..#....#",
    "#........#....L.L.L.#....#",
    "#..KKK...#################",
    "#..S.....*...............#",
    "#........................#",
    "#w###w####E####w####w#####",
];

/// The butcher: the counter and hooks out front, a locked meat locker, the chair in
/// the cold room among the freezers.
const BUTCHER: [&str; 10] = [
    "##########################",
    "#L.L.L.L.L.#H.H.H.H.#.LL.#",
    "#..........#........#....#",
    "#....X.....+...*....#....#",
    "#....dg..D.#........##%###",
    "#..*.......#.KKKKKKKKK...#",
    "#L.L.L.L.L.#.............#",
    "############.....V....*..#",
    "#........................#",
    "#w###w####w####E####w#####",
];

/// A front's grids: the street floor first, then the cellar.
pub(crate) fn grids(front: Front) -> &'static [&'static [&'static str]] {
    match front {
        Front::NoodleBar => &[&NOODLE_BAR],
        Front::Laundromat => &[&LAUNDROMAT, &CELLAR],
        Front::Dentist => &[&DENTIST],
        Front::Tattoo => &[&TATTOO],
        Front::Butcher => &[&BUTCHER],
    }
}

/// The grid for a level: the street floor at 0, the cellar below.
pub(crate) fn grid(front: Front, level: i32) -> &'static [&'static str] {
    let all = grids(front);
    all[if level < 0 { 1 } else { 0 }.min(all.len() - 1)]
}

/// A grid's width and height.
pub(crate) fn dims(grid: &[&str]) -> (i32, i32) {
    (grid[0].len() as i32, grid.len() as i32)
}

/// Where a mark is in a grid, unturned.
pub(crate) fn find(grid: &[&str], mark: u8) -> Option<Pos> {
    grid.iter().enumerate().find_map(|(j, row)| {
        row.bytes()
            .position(|c| c == mark)
            .map(|i| Pos::new(i as i32, j as i32))
    })
}

/// The tiles of a grid's locked rooms, unturned: the regions behind a `%` door that
/// hold no chair, no stairs and no way in from the front door.
pub(crate) fn locked_cells(grid: &[&str]) -> HashSet<Pos> {
    let (w, h) = dims(grid);
    let at = |p: Pos| grid[p.y as usize].as_bytes()[p.x as usize];
    let inside = |p: Pos| p.x > 0 && p.y > 0 && p.x < w - 1 && p.y < h - 1;
    let room = |p: Pos| inside(p) && !b"#w+%E".contains(&at(p));
    let front = find(grid, b'E').map(|e| e.offset(0, -1));
    let mut out = HashSet::new();
    let mut seen = HashSet::new();
    for j in 0..h {
        for i in 0..w {
            let p = Pos::new(i, j);
            if at(p) != b'%' {
                continue;
            }
            for start in p.neighbors4() {
                if !room(start) || seen.contains(&start) {
                    continue;
                }
                let mut region = vec![start];
                let mut todo = vec![start];
                seen.insert(start);
                while let Some(q) = todo.pop() {
                    for n in q.neighbors4() {
                        if room(n) && seen.insert(n) {
                            region.push(n);
                            todo.push(n);
                        }
                    }
                }
                let open = region
                    .iter()
                    .any(|q| b"Xv^".contains(&at(*q)) || Some(*q) == front);
                if !open {
                    out.extend(region);
                }
            }
        }
    }
    out
}

/// The tiles of a grid's front room, unturned: the region the front door steps into.
pub(crate) fn front_cells(grid: &[&str]) -> HashSet<Pos> {
    let (w, h) = dims(grid);
    let at = |p: Pos| grid[p.y as usize].as_bytes()[p.x as usize];
    let room =
        |p: Pos| p.x > 0 && p.y > 0 && p.x < w - 1 && p.y < h - 1 && !b"#w+%E".contains(&at(p));
    let Some(start) = find(grid, b'E').map(|e| e.offset(0, -1)) else {
        return HashSet::new();
    };
    let mut seen = HashSet::from([start]);
    let mut todo = vec![start];
    while let Some(q) = todo.pop() {
        for n in q.neighbors4() {
            if room(n) && seen.insert(n) {
                todo.push(n);
            }
        }
    }
    seen
}

fn furniture(mark: u8) -> Option<Furniture> {
    Some(match mark {
        b'X' => Furniture::Chair,
        b'K' => Furniture::Counter,
        b'S' => Furniture::Stool,
        b'H' => Furniture::Shelf,
        b'M' => Furniture::Machine,
        b'B' => Furniture::Bed,
        b'L' => Furniture::Locker,
        b'C' => Furniture::Couch,
        b'O' => Furniture::Table,
        b'D' => Furniture::Desk,
        b'N' => Furniture::Bench,
        b'V' => Furniture::Vending,
        _ => return None,
    })
}

impl Builder<'_> {
    /// Stamps the clinic's grid for this level over the floor, turned as the plan says:
    /// walls (the frame's too), windows, doors, furniture, lamps, the drain and the
    /// doc's spot. The exits' doors and doorsteps and the stairwell, set already, stay.
    pub(super) fn clinic_floor(&mut self) {
        let Some(plan) = self.plan.clinic else {
            return;
        };
        let rows = grid(plan.front, self.level);
        let (w, h) = dims(rows);
        for (j, row) in rows.iter().enumerate() {
            for (i, mark) in row.bytes().enumerate() {
                let p = turn_pos(Pos::new(i as i32, j as i32), w, h, plan.turn);
                if self.reserved.contains(&p) {
                    continue;
                }
                let t = &mut self.tiles[p];
                t.terrain = match mark {
                    b'#' => Terrain::Wall,
                    b'w' => Terrain::Window,
                    b'E' | b'+' | b'%' => Terrain::Door,
                    b'g' => Terrain::Grate,
                    _ => Terrain::Floor,
                };
                // A `*` was a lamp; the floor's fittings light it now.
                t.feature = furniture(mark).map(Feature::Furniture);
                if mark == b'd' {
                    self.stations.push(Station {
                        pos: p,
                        role: Role::Surgery,
                    });
                }
            }
        }
        let locked: Vec<Pos> = rows
            .iter()
            .enumerate()
            .flat_map(|(j, row)| {
                row.bytes()
                    .enumerate()
                    .filter(|(_, m)| *m == b'%')
                    .map(move |(i, _)| Pos::new(i as i32, j as i32))
            })
            .map(|q| turn_pos(q, w, h, plan.turn))
            .collect();
        self.clinic_rooms(&locked);
    }

    /// The floor's rooms: each walled region inside the frame, the chair's the surgery
    /// (listed first, then the locked ones), the one by the front door the waiting
    /// room, the rest back rooms;
    /// a room behind a `locked` door is private, that door its own.
    fn clinic_rooms(&mut self, locked: &[Pos]) {
        let inner = self.inner();
        let open = |b: &Self, p: Pos| {
            inner.contains(p)
                && !matches!(
                    b.tiles[p].terrain,
                    Terrain::Wall | Terrain::Window | Terrain::Door | Terrain::Mass
                )
        };
        let front_step = self.doorsteps.first().copied();
        let mut seen: HashSet<Pos> = HashSet::new();
        let mut rooms = Vec::new();
        for start in inner.iter() {
            if seen.contains(&start) || !open(self, start) {
                continue;
            }
            let mut todo = vec![start];
            let mut region = Vec::new();
            seen.insert(start);
            while let Some(p) = todo.pop() {
                region.push(p);
                for n in p.neighbors4() {
                    if !seen.contains(&n) && open(self, n) {
                        seen.insert(n);
                        todo.push(n);
                    }
                }
            }
            let (x0, y0) = (
                region.iter().map(|p| p.x).min().unwrap(),
                region.iter().map(|p| p.y).min().unwrap(),
            );
            let (x1, y1) = (
                region.iter().map(|p| p.x).max().unwrap(),
                region.iter().map(|p| p.y).max().unwrap(),
            );
            let chair = region
                .iter()
                .any(|p| self.tiles[*p].feature == Some(Feature::Furniture(Furniture::Chair)));
            let kind = if chair {
                RoomKind::Surgery
            } else if front_step.is_some_and(|s| region.contains(&s)) {
                RoomKind::Waiting
            } else {
                RoomKind::BackRoom
            };
            // A locked door has two sides: the room locked away is the one with no
            // chair, no stairs and no way in from the street.
            let open_side = chair
                || kind == RoomKind::Waiting
                || region
                    .iter()
                    .any(|p| self.tiles[*p].terrain == Terrain::Stair);
            let door = locked
                .iter()
                .copied()
                .filter(|_| !open_side)
                .find(|d| d.neighbors4().any(|n| region.contains(&n)));
            rooms.push(Room {
                rect: Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1),
                kind,
                door,
                private: door.is_some(),
                number: 0,
            });
        }
        // The surgery first, then the locked rooms: `room_at` takes the first rect that
        // holds a tile, and the front room's wraps around both.
        rooms.sort_by_key(|r| (r.kind != RoomKind::Surgery, !r.private));
        self.rooms.extend(rooms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interior::plan::{Turn, turn_size};

    const WALK: &[u8] = b".dEg+v^*%";

    #[test]
    fn layouts_are_sound() {
        for front in Front::ALL {
            let all = grids(front);
            assert_eq!(all.len(), front.floors(), "{front:?}");
            let (w, h) = dims(all[0]);
            let mut chairs = 0;
            for (level, rows) in all.iter().enumerate() {
                assert_eq!(dims(rows), (w, h), "{front:?} floor {level}");
                for (j, row) in rows.iter().enumerate() {
                    assert_eq!(row.len() as i32, w, "{front:?} row {j}");
                    for (i, c) in row.bytes().enumerate() {
                        let (i, j) = (i as i32, j as i32);
                        let rim = i == 0 || j == 0 || i == w - 1 || j == h - 1;
                        let corner = (i == 0 || i == w - 1) && (j == 0 || j == h - 1);
                        if rim {
                            assert!(b"#wE".contains(&c), "{front:?} frame ({i},{j})");
                        }
                        if corner {
                            assert_eq!(c, b'#', "{front:?} corner ({i},{j})");
                        }
                        if c == b'E' {
                            assert!(level == 0 && j == h - 1, "{front:?}: E off the street");
                        }
                        if c == b'X' {
                            chairs += 1;
                            let p = Pos::new(i, j);
                            assert!(
                                p.neighbors4()
                                    .any(|n| rows[n.y as usize].as_bytes()[n.x as usize] == b'd'),
                                "{front:?}: no d beside X"
                            );
                        }
                    }
                }
                // Everything walkable is reached: from E, or on the cellar from its stair.
                let start = find(rows, if level == 0 { b'E' } else { b'^' }).unwrap();
                let walk = |p: Pos| {
                    p.x >= 0
                        && p.y >= 0
                        && p.x < w
                        && p.y < h
                        && WALK.contains(&rows[p.y as usize].as_bytes()[p.x as usize])
                };
                let mut seen = HashSet::from([start]);
                let mut todo = vec![start];
                while let Some(p) = todo.pop() {
                    for n in p.neighbors4() {
                        if walk(n) && seen.insert(n) {
                            todo.push(n);
                        }
                    }
                }
                for j in 0..h {
                    for i in 0..w {
                        let p = Pos::new(i, j);
                        assert!(
                            !walk(p) || seen.contains(&p),
                            "{front:?} {level}: {p:?} cut off"
                        );
                    }
                }
            }
            assert_eq!(chairs, 1, "{front:?}");
            assert_eq!(
                all[0].iter().map(|r| r.matches('E').count()).sum::<usize>(),
                1
            );
            if all.len() == 2 {
                assert_eq!(find(all[0], b'v'), find(all[1], b'^'), "{front:?} stairs");
                assert!(find(all[0], b'v').is_some());
            }
        }
    }

    #[test]
    fn every_clinic_has_a_locked_room() {
        for front in Front::ALL {
            let n: usize = grids(front).iter().map(|g| locked_cells(g).len()).sum();
            assert!(n > 0, "{front:?}");
            // The chair's room is never locked away.
            for g in grids(front) {
                if let Some(x) = find(g, b'X') {
                    assert!(!locked_cells(g).contains(&x), "{front:?}");
                }
            }
        }
    }

    #[test]
    fn turns_round_trip() {
        let (w, h) = (31, 10);
        for quarters in 0..4 {
            for mirror in [false, true] {
                let t = Turn { quarters, mirror };
                let (tw, th) = turn_size(w, h, t);
                let corners = [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)];
                let mut got: Vec<(i32, i32)> = corners
                    .iter()
                    .map(|&(x, y)| {
                        let p = turn_pos(Pos::new(x, y), w, h, t);
                        (p.x, p.y)
                    })
                    .collect();
                got.sort();
                let mut want = vec![(0, 0), (tw - 1, 0), (0, th - 1), (tw - 1, th - 1)];
                want.sort();
                assert_eq!(got, want, "{t:?}");
                // The street side (the bottom row) faces south, west, north, east.
                let mid = turn_pos(Pos::new(w / 2, h - 1), w, h, t);
                let on = match quarters {
                    0 => mid.y == th - 1,
                    1 => mid.x == 0,
                    2 => mid.y == 0,
                    _ => mid.x == tw - 1,
                };
                assert!(on, "{t:?}: {mid:?} in {tw}x{th}");
            }
        }
    }
}
