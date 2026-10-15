//! Floor layouts: the frame every floor shares, then one builder per floor type.

use super::floor::{Exit, Floor, Role, Room, RoomKind, Station};
use super::plan::{FloorPlan, FloorType, SCALE};
use crate::geom::{Pos, Rect};
use crate::lighting;
use crate::pathing::{bfs_nearest, components};
use crate::rng::hash64;
use crate::tile::{Feature, Furniture, Terrain, Tile};
use crate::world::{City, StratumKind};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashSet;

/// Base light of a floor; ceiling lamps add to it.
const BASE_LIGHT: u8 = 90;

pub(super) struct Builder<'a> {
    city: &'a City,
    plan: &'a FloorPlan,
    level: i32,
    tiles: crate::geom::Grid<Tile>,
    rooms: Vec<Room>,
    stations: Vec<Station>,
    exits: Vec<Exit>,
    rng: ChaCha8Rng,
    /// Tiles that stay clear: doorsteps, the lift bank, the stairwell, exits.
    reserved: HashSet<Pos>,
}

/// Which edge of a rect a border tile sits on.
fn edge(r: Rect, p: Pos) -> u8 {
    if p.x == r.x {
        0
    } else if p.x == r.right() - 1 {
        1
    } else if p.y == r.y {
        2
    } else {
        3
    }
}

fn is_corner(r: Rect, p: Pos) -> bool {
    (p.x == r.x || p.x == r.right() - 1) && (p.y == r.y || p.y == r.bottom() - 1)
}

/// The neighbour of a footprint border tile that lies outside it.
fn outward(r: Rect, p: Pos) -> Pos {
    if p.x == r.x {
        p.offset(-1, 0)
    } else if p.x == r.right() - 1 {
        p.offset(1, 0)
    } else if p.y == r.y {
        p.offset(0, -1)
    } else {
        p.offset(0, 1)
    }
}

/// Where a city-map door on the footprint border lands on the floor's border.
pub(super) fn exit_tile(plan: &FloorPlan, door: Pos) -> Pos {
    let fp = plan.footprint;
    let (w, h) = (plan.size.w as i32, plan.size.h as i32);
    let sx = ((door.x - fp.x) * SCALE + 1).clamp(1, w - 2);
    let sy = ((door.y - fp.y) * SCALE + 1).clamp(1, h - 2);
    match edge(fp, door) {
        0 => Pos::new(0, sy),
        1 => Pos::new(w - 1, sy),
        2 => Pos::new(sx, 0),
        _ => Pos::new(sx, h - 1),
    }
}

impl<'a> Builder<'a> {
    fn new(city: &'a City, plan: &'a FloorPlan, level: i32) -> Self {
        let seed = hash64(
            hash64(city.seed, plan.building.index() as u64),
            level as i64 as u64,
        );
        let mut tiles = crate::geom::Grid::filled(plan.size, Tile::new(Terrain::Floor));
        let frame = plan.size.rect();
        for p in frame.border() {
            tiles[p].terrain = Terrain::Wall;
        }
        let mut b = Builder {
            city,
            plan,
            level,
            tiles,
            rooms: Vec::new(),
            stations: Vec::new(),
            exits: Vec::new(),
            rng: ChaCha8Rng::seed_from_u64(seed),
            reserved: HashSet::new(),
        };
        b.windows();
        b
    }

    fn w(&self) -> i32 {
        self.plan.size.w as i32
    }

    fn h(&self) -> i32 {
        self.plan.size.h as i32
    }

    fn frame(&self) -> Rect {
        self.plan.size.rect()
    }

    fn inner(&self) -> Rect {
        self.frame().inset(1)
    }

    /// Windows every other tile on the sides that face the street outside.
    fn windows(&mut self) {
        let fp = self.plan.footprint;
        let street = StratumKind::Street;
        let facing: Vec<u8> = fp
            .border()
            .filter(|p| !is_corner(fp, *p))
            .filter(|p| {
                self.city
                    .tile(street, outward(fp, *p))
                    .is_some_and(|t| t.is_outdoor_ground())
            })
            .map(|p| edge(fp, p))
            .collect();
        let frame = self.frame();
        for p in frame.border() {
            if is_corner(frame, p) || !facing.contains(&edge(frame, p)) {
                continue;
            }
            if (p.x + p.y) % 2 == 0 {
                self.tiles[p].terrain = Terrain::Window;
            }
        }
    }

    /// The doors of this floor, from the city map's doors on the floor's stratum.
    fn exits(&mut self) {
        let Some(stratum) = self.plan.exit_stratum(self.level) else {
            return;
        };
        let fp = self.plan.footprint;
        let doors: Vec<Pos> = self
            .city
            .portals(self.plan.building)
            .into_iter()
            .filter(|(k, _)| *k == stratum)
            .map(|(_, p)| p)
            .collect();
        for door in doors {
            let (at, landing) = if stratum == StratumKind::Heights {
                // The roof door: a stair-house door beside the stairwell, landing on
                // the roof's door tile.
                (self.plan.stair.offset(1, 0), door)
            } else {
                (exit_tile(self.plan, door), outward(fp, door))
            };
            self.tiles[at].terrain = Terrain::Door;
            self.tiles[at].feature = None;
            let exit = Exit {
                at,
                stratum,
                landing,
                door,
            };
            let probe = Floor {
                level: self.level,
                tiles: self.tiles.clone(),
                light: crate::geom::Grid::filled(self.plan.size, 0),
                rooms: Vec::new(),
                exits: Vec::new(),
                stations: Vec::new(),
            };
            let step = probe.doorstep(&exit);
            self.tiles[step].terrain = Terrain::Floor;
            self.tiles[step].feature = None;
            self.reserved.insert(at);
            self.reserved.insert(step);
            for n in step.neighbors4() {
                if self.inner().contains(n) {
                    self.reserved.insert(n);
                }
            }
            self.exits.push(exit);
        }
    }

    /// The lift bank and the stairwell, where the plan puts them on every floor.
    fn shafts(&mut self) {
        let up = self.level < self.plan.top;
        let down = self.level > self.plan.lowest;
        for p in self.plan.lift_bank() {
            self.tiles[p].terrain = Terrain::Lift;
            self.tiles[p].feature = None;
            self.reserved.insert(p);
            for n in p.neighbors4() {
                self.reserved.insert(n);
            }
        }
        if up || down {
            let s = self.plan.stair;
            self.tiles[s].terrain = Terrain::Stair;
            self.tiles[s].feature = None;
            self.reserved.insert(s);
            for n in s.neighbors4() {
                self.reserved.insert(n);
            }
        }
    }

    fn put(&mut self, p: Pos, piece: Furniture) -> bool {
        if !self.inner().contains(p)
            || self.reserved.contains(&p)
            || self.tiles[p].terrain != Terrain::Floor
            || self.tiles[p].feature.is_some()
        {
            return false;
        }
        self.tiles[p].feature = Some(Feature::Furniture(piece));
        true
    }

    fn station(&mut self, p: Pos, role: Role) {
        if self.inner().contains(p)
            && self.tiles[p].terrain == Terrain::Floor
            && self.tiles[p].feature.is_none()
            && !self.stations.iter().any(|s| s.pos == p)
        {
            self.stations.push(Station { pos: p, role });
        }
    }

    fn wall_line(&mut self, from: Pos, to: Pos) {
        let r = Rect::new(
            from.x.min(to.x),
            from.y.min(to.y),
            (from.x - to.x).abs() + 1,
            (from.y - to.y).abs() + 1,
        );
        for p in r.iter() {
            if self.inner().contains(p)
                && !self.reserved.contains(&p)
                && self.tiles[p].terrain == Terrain::Floor
            {
                self.tiles[p].terrain = Terrain::Wall;
                self.tiles[p].feature = None;
            }
        }
    }

    fn door(&mut self, p: Pos) -> bool {
        if self.inner().contains(p) && self.tiles[p].terrain == Terrain::Wall {
            self.tiles[p].terrain = Terrain::Door;
            self.reserved.insert(p);
            true
        } else {
            false
        }
    }

    // ---- floor types ------------------------------------------------------

    /// An open hall with a reception counter inside the main door, the shafts at
    /// the back.
    fn lobby(&mut self) {
        let frame = self.frame();
        self.rooms.push(Room {
            rect: self.inner(),
            kind: RoomKind::Lobby,
            door: None,
            private: false,
            number: 0,
        });
        let Some(main) = self.exits.first().copied() else {
            return;
        };
        // Three counter tiles three tiles in from the door's edge, centred on it; the
        // receptionist stands behind.
        let (counter, behind): (Vec<Pos>, Pos) = match edge(frame, main.at) {
            0 => {
                let x = 3;
                (
                    (-1..=1).map(|d| Pos::new(x, main.at.y + d)).collect(),
                    Pos::new(x + 1, main.at.y),
                )
            }
            1 => {
                let x = self.w() - 4;
                (
                    (-1..=1).map(|d| Pos::new(x, main.at.y + d)).collect(),
                    Pos::new(x - 1, main.at.y),
                )
            }
            2 => {
                let y = 3;
                (
                    (-1..=1).map(|d| Pos::new(main.at.x + d, y)).collect(),
                    Pos::new(main.at.x, y + 1),
                )
            }
            _ => {
                let y = self.h() - 4;
                (
                    (-1..=1).map(|d| Pos::new(main.at.x + d, y)).collect(),
                    Pos::new(main.at.x, y - 1),
                )
            }
        };
        let mut placed = 0;
        for p in counter {
            if self.put(p, Furniture::Counter) {
                placed += 1;
            }
        }
        if placed > 0 {
            self.station(behind, Role::Reception);
        }
    }

    /// A two-wide corridor along the long axis, rooms on both sides.
    fn corridor(&mut self, kind: FloorType) {
        let (w, h) = (self.w(), self.h());
        let horizontal = w >= h;
        let along = if horizontal { w } else { h };
        let across = if horizontal { h } else { w };
        let at = |a: i32, c: i32| {
            if horizontal {
                Pos::new(a, c)
            } else {
                Pos::new(c, a)
            }
        };
        let c0 = across / 2 - 1;
        let c1 = across / 2;
        // The corridor itself.
        let corridor = if horizontal {
            Rect::new(1, c0, w - 2, 2)
        } else {
            Rect::new(c0, 1, 2, h - 2)
        };
        self.rooms.push(Room {
            rect: corridor,
            kind: RoomKind::Corridor,
            door: None,
            private: false,
            number: 0,
        });
        // Walls along both sides of the corridor.
        self.wall_line(at(1, c0 - 1), at(along - 2, c0 - 1));
        self.wall_line(at(1, c1 + 1), at(along - 2, c1 + 1));
        let mut number = 0u16;
        let laundry_floor = self.level.rem_euclid(3) == 0;
        for side in 0..2 {
            let (lo, hi) = if side == 0 {
                (1, c0 - 2)
            } else {
                (c1 + 2, across - 2)
            };
            if hi < lo + 1 {
                continue;
            }
            let mut a = 1;
            let mut index = 0;
            while a <= along - 2 {
                let span = self.rng.random_range(5..=8).min(along - 1 - a);
                if span < 3 {
                    break;
                }
                let rect = if horizontal {
                    Rect::new(a, lo, span, hi - lo + 1)
                } else {
                    Rect::new(lo, a, hi - lo + 1, span)
                };
                index += 1;
                let last = a + span >= along - 2;
                let (room_kind, private) = match kind {
                    FloorType::Suite => (RoomKind::OpenPlan, false),
                    FloorType::Vip => (RoomKind::Vip, false),
                    _ => match self.plan.kind {
                        crate::world::BuildingKind::CorpTower
                        | crate::world::BuildingKind::Office => {
                            if index % 2 == 1 {
                                (RoomKind::Office, true)
                            } else {
                                (RoomKind::OpenPlan, false)
                            }
                        }
                        crate::world::BuildingKind::Hotel => (RoomKind::HotelRoom, true),
                        _ => {
                            if laundry_floor && last && side == 1 {
                                (RoomKind::Laundry, false)
                            } else {
                                (RoomKind::Flat, true)
                            }
                        }
                    },
                };
                // The wall at the room's far end, then its door onto the corridor.
                let end = a + span;
                if end <= along - 2 {
                    self.wall_line(at(end, lo), at(end, hi));
                }
                // The door at the room's middle, or the nearest wall tile along it: the
                // shafts' surroundings stay open floor.
                let wall_c = if side == 0 { c0 - 1 } else { c1 + 1 };
                let mid = at(a + span / 2, wall_c);
                let door = std::iter::once(mid)
                    .chain((a..end.min(along - 1)).map(|m| at(m, wall_c)))
                    .find(|p| self.tiles[*p].terrain == Terrain::Wall)
                    .filter(|p| self.door(*p));
                let private = private && door.is_some();
                number += 1;
                let room_number = if matches!(room_kind, RoomKind::Flat | RoomKind::HotelRoom) {
                    (self.level.max(0) as u16) * 100 + number
                } else {
                    0
                };
                self.furnish_room(rect, room_kind, door);
                self.rooms.push(Room {
                    rect,
                    kind: room_kind,
                    door,
                    private,
                    number: room_number,
                });
                a = end + 1;
            }
        }
    }

    /// What a room of a kind holds, and the stations beside it.
    fn furnish_room(&mut self, rect: Rect, kind: RoomKind, door: Option<Pos>) {
        let centre = rect.center();
        match kind {
            RoomKind::Office | RoomKind::Suite => {
                if self.put(centre, Furniture::Desk) {
                    self.station(centre.offset(0, 1), Role::Desk);
                }
            }
            RoomKind::OpenPlan => {
                let mut y = rect.y + 1;
                while y < rect.bottom() - 1 {
                    let mut x = rect.x + 1;
                    while x < rect.right() - 1 {
                        let p = Pos::new(x, y);
                        if Some(p) != door && self.put(p, Furniture::Desk) {
                            self.station(p.offset(0, 1), Role::Desk);
                        }
                        x += 3;
                    }
                    y += 3;
                }
            }
            RoomKind::Flat | RoomKind::HotelRoom => {
                let corner = Pos::new(rect.x, rect.y);
                if !self.put(corner, Furniture::Bed) {
                    self.put(centre, Furniture::Bed);
                }
            }
            RoomKind::Laundry => {
                let mut x = rect.x;
                while x < rect.right() {
                    self.put(Pos::new(x, rect.y), Furniture::Machine);
                    x += 2;
                }
            }
            RoomKind::Vip => {
                if self.put(centre, Furniture::Table) {
                    self.station(centre.offset(0, 1), Role::Table);
                    self.station(centre.offset(0, -1), Role::Table);
                }
            }
            RoomKind::Lockers => {
                for x in rect.x..rect.right() {
                    self.put(Pos::new(x, rect.y), Furniture::Locker);
                }
            }
            RoomKind::Storage | RoomKind::Hall => {
                let mut y = rect.y + 1;
                while y < rect.bottom() - 1 {
                    // Shelving with an aisle through it every six tiles.
                    for x in rect.x + 1..rect.right() - 1 {
                        if (x - rect.x) % 6 == 3 {
                            continue;
                        }
                        self.put(Pos::new(x, y), Furniture::Shelf);
                    }
                    self.station(Pos::new(rect.x, y), Role::Shelf);
                    self.station(Pos::new(rect.right() - 1, y), Role::Shelf);
                    y += 4;
                }
            }
            _ => {}
        }
    }

    /// Open plan on most of the floor, one locked office at the far end, the roof
    /// door beside the stairwell.
    fn suite(&mut self) {
        let inner = self.inner();
        let (w, h) = (self.w(), self.h());
        let horizontal = w >= h;
        let third = if horizontal { w / 3 } else { h / 3 };
        let (open, office) = if horizontal {
            (
                Rect::new(1, 1, w - 2 - third, h - 2),
                Rect::new(w - 1 - third, 1, third, h - 2),
            )
        } else {
            (
                Rect::new(1, 1, w - 2, h - 2 - third),
                Rect::new(1, h - 1 - third, w - 2, third),
            )
        };
        let _ = inner;
        // The office wall and its door.
        let (from, to, door) = if horizontal {
            (
                Pos::new(office.x - 1, 1),
                Pos::new(office.x - 1, h - 2),
                Pos::new(office.x - 1, h / 2),
            )
        } else {
            (
                Pos::new(1, office.y - 1),
                Pos::new(w - 2, office.y - 1),
                Pos::new(w / 2, office.y - 1),
            )
        };
        self.wall_line(from, to);
        let door = if self.door(door) { Some(door) } else { None };
        self.furnish_room(open, RoomKind::OpenPlan, None);
        self.rooms.push(Room {
            rect: open,
            kind: RoomKind::OpenPlan,
            door: None,
            private: false,
            number: 0,
        });
        self.furnish_room(office, RoomKind::Suite, door);
        self.rooms.push(Room {
            rect: office,
            kind: RoomKind::Suite,
            door,
            private: true,
            number: 0,
        });
    }

    /// Shelving rows, the shafts and the basement door.
    fn basement(&mut self) {
        let inner = self.inner();
        self.furnish_room(inner, RoomKind::Storage, None);
        self.rooms.push(Room {
            rect: inner,
            kind: RoomKind::Storage,
            door: None,
            private: false,
            number: 0,
        });
    }

    /// Whether every tile of a rect is bare floor nobody has claimed.
    fn free(&self, r: Rect) -> bool {
        r.iter().all(|p| {
            self.frame().contains(p)
                && !self.reserved.contains(&p)
                && (self.tiles[p].terrain == Terrain::Floor
                    || (self.frame().border().any(|q| q == p)))
                && self.tiles[p].feature.is_none()
        })
    }

    /// A walled room of `w`×`h` (walls included) in the first free corner, with a
    /// door on a side facing the hall. Returns the room's interior.
    fn corner_room(&mut self, w: i32, h: i32, kind: RoomKind, private: bool) -> Option<Room> {
        let (fw, fh) = (self.w(), self.h());
        let corners = [
            Rect::new(0, 0, w, h),
            Rect::new(fw - w, 0, w, h),
            Rect::new(0, fh - h, w, h),
            Rect::new(fw - w, fh - h, w, h),
        ];
        let rect = corners.into_iter().find(|r| self.free(*r))?;
        for p in rect.border() {
            if self.inner().contains(p) && self.tiles[p].terrain == Terrain::Floor {
                self.tiles[p].terrain = Terrain::Wall;
            }
        }
        // The door: on the inner side wall facing the hall's centre, at its middle.
        let centre = self.frame().center();
        let inside = rect.inset(1);
        let east_west = if centre.x > rect.center().x {
            Pos::new(rect.right() - 1, inside.y + inside.h / 2)
        } else {
            Pos::new(rect.x, inside.y + inside.h / 2)
        };
        let north_south = if centre.y > rect.center().y {
            Pos::new(inside.x + inside.w / 2, rect.bottom() - 1)
        } else {
            Pos::new(inside.x + inside.w / 2, rect.y)
        };
        let door = [east_west, north_south]
            .into_iter()
            .find(|d| self.inner().contains(*d) && self.tiles[*d].terrain == Terrain::Wall)
            .filter(|d| self.door(*d));
        self.furnish_room(inside, kind, door);
        let room = Room {
            rect: inside,
            kind,
            door,
            private: private && door.is_some(),
            number: 0,
        };
        self.rooms.push(room.clone());
        Some(room)
    }

    /// The edge the main door is on, else the south.
    fn door_edge(&self) -> u8 {
        self.exits
            .first()
            .map(|e| edge(self.frame(), e.at))
            .unwrap_or(3)
    }

    /// A position by (along the wall opposite the door, in from that wall).
    fn opposite(&self, along: i32, depth: i32) -> Pos {
        let (w, h) = (self.w(), self.h());
        match self.door_edge() {
            0 => Pos::new(w - 1 - depth, along),
            1 => Pos::new(depth, along),
            2 => Pos::new(along, h - 1 - depth),
            _ => Pos::new(along, depth),
        }
    }

    /// How long the wall opposite the door is.
    fn opposite_len(&self) -> i32 {
        match self.door_edge() {
            0 | 1 => self.h(),
            _ => self.w(),
        }
    }

    /// Tables with a stool on a loose grid over the open floor, a station at each.
    fn tables(&mut self) {
        let inner = self.inner();
        let mut y = inner.y + 1;
        while y < inner.bottom() - 1 {
            let mut x = inner.x + 1;
            while x < inner.right() - 1 {
                let p = Pos::new(x, y);
                if self.free(Rect::new(x, y, 2, 2)) && self.put(p, Furniture::Table) {
                    self.put(p.offset(1, 0), Furniture::Stool);
                    self.station(p.offset(0, 1), Role::Table);
                }
                x += 4;
            }
            y += 4;
        }
        // A tight floor: the first free pair anywhere.
        if !self.stations.iter().any(|s| s.role == Role::Table) {
            for p in inner.iter() {
                if self.free(Rect::new(p.x, p.y, 2, 2)) && self.put(p, Furniture::Table) {
                    self.put(p.offset(1, 0), Furniture::Stool);
                    self.station(p.offset(0, 1), Role::Table);
                    break;
                }
            }
        }
    }

    /// The bar: a counter along the wall opposite the door with the staff behind it,
    /// tables and stools, toilets, a locked back room; a club adds a stage.
    fn bar(&mut self, club: bool) {
        self.rooms.push(Room {
            rect: self.inner(),
            kind: RoomKind::Bar,
            door: None,
            private: false,
            number: 0,
        });
        let len = self.opposite_len();
        // A club's stage first: five along, three deep, as close to the wall opposite
        // the door as a free block allows.
        if club {
            let mut spot = None;
            'search: for depth in 1..=(self.h().max(self.w()) - 4) {
                for along in (2..=len - 7).rev() {
                    let tiles: Vec<Pos> = (along..along + 5)
                        .flat_map(|a| (depth..depth + 3).map(move |d| (a, d)))
                        .map(|(a, d)| self.opposite(a, d))
                        .collect();
                    let xs = tiles.iter().map(|p| p.x).min().unwrap_or(1);
                    let ys = tiles.iter().map(|p| p.y).min().unwrap_or(1);
                    let xe = tiles.iter().map(|p| p.x).max().unwrap_or(1);
                    let ye = tiles.iter().map(|p| p.y).max().unwrap_or(1);
                    let rect = Rect::new(xs, ys, xe - xs + 1, ye - ys + 1);
                    if self.inner().contains(Pos::new(xs, ys))
                        && self.inner().contains(Pos::new(xe, ye))
                        && self.free(rect)
                    {
                        spot = Some(rect);
                        break 'search;
                    }
                }
            }
            if let Some(rect) = spot {
                for p in rect.iter() {
                    self.reserved.insert(p);
                    self.tiles[p].feature = Some(Feature::Lamp);
                }
                self.rooms.push(Room {
                    rect,
                    kind: RoomKind::Stage,
                    door: None,
                    private: false,
                    number: 0,
                });
            }
        }
        // The counter: three fifths of the wall; it breaks around anything in its way.
        let (from, to) = (2, 2 + (len - 4) * 3 / 5);
        for a in from..=to {
            let p = self.opposite(a, 2);
            if self.put(p, Furniture::Counter) && (a - from) % 3 == 0 {
                let behind = self.opposite(a, 1);
                self.station(behind, Role::Bar);
            }
        }
        self.corner_room(4, 4, RoomKind::Toilets, false);
        self.corner_room(5, 4, RoomKind::BackRoom, true);
        self.tables();
    }

    /// A waiting room by the door, the surgery beyond a wall, a locked back room.
    fn clinic(&mut self) {
        let (w, h) = (self.w(), self.h());
        let edge = self.door_edge();
        // The dividing wall runs parallel to the door's wall, halfway in.
        let (waiting, surgery, wall_from, wall_to, door) = match edge {
            0 => (
                Rect::new(1, 1, w / 2 - 1, h - 2),
                Rect::new(w / 2 + 1, 1, w - 2 - w / 2, h - 2),
                Pos::new(w / 2, 1),
                Pos::new(w / 2, h - 2),
                Pos::new(w / 2, h / 2),
            ),
            1 => (
                Rect::new(w / 2 + 1, 1, w - 2 - w / 2, h - 2),
                Rect::new(1, 1, w / 2 - 1, h - 2),
                Pos::new(w / 2, 1),
                Pos::new(w / 2, h - 2),
                Pos::new(w / 2, h / 2),
            ),
            2 => (
                Rect::new(1, 1, w - 2, h / 2 - 1),
                Rect::new(1, h / 2 + 1, w - 2, h - 2 - h / 2),
                Pos::new(1, h / 2),
                Pos::new(w - 2, h / 2),
                Pos::new(w / 2 + 2, h / 2),
            ),
            _ => (
                Rect::new(1, h / 2 + 1, w - 2, h - 2 - h / 2),
                Rect::new(1, 1, w - 2, h / 2 - 1),
                Pos::new(1, h / 2),
                Pos::new(w - 2, h / 2),
                Pos::new(w / 2 + 2, h / 2),
            ),
        };
        self.wall_line(wall_from, wall_to);
        let door = std::iter::once(door)
            .chain(
                Rect::new(
                    wall_from.x.min(wall_to.x),
                    wall_from.y.min(wall_to.y),
                    (wall_from.x - wall_to.x).abs() + 1,
                    (wall_from.y - wall_to.y).abs() + 1,
                )
                .iter(),
            )
            .find(|p| self.tiles[*p].terrain == Terrain::Wall)
            .filter(|p| self.door(*p));
        // Stools in rows along the waiting room's far wall.
        let mut y = waiting.y + 1;
        while y < waiting.bottom() {
            let mut x = waiting.x + 1;
            while x < waiting.right() - 1 {
                self.put(Pos::new(x, y), Furniture::Stool);
                x += 2;
            }
            y += 3;
        }
        self.rooms.push(Room {
            rect: waiting,
            kind: RoomKind::Waiting,
            door: None,
            private: false,
            number: 0,
        });
        let bed = surgery.center();
        if self.put(bed, Furniture::Bed) {
            self.station(bed.offset(1, 0), Role::Surgery);
            self.station(bed.offset(-1, 0), Role::Surgery);
        }
        self.rooms.push(Room {
            rect: surgery,
            kind: RoomKind::Surgery,
            door,
            private: false,
            number: 0,
        });
        self.corner_room(4, 4, RoomKind::BackRoom, true);
    }

    /// A hall with the shrine at the far wall and a locked side room.
    fn temple(&mut self) {
        self.rooms.push(Room {
            rect: self.inner(),
            kind: RoomKind::Hall,
            door: None,
            private: false,
            number: 0,
        });
        self.corner_room(4, 4, RoomKind::BackRoom, true);
        // The shrine on the wall opposite the door, as near its middle as a free
        // span allows (the other door's doorstep may sit there).
        let len = self.opposite_len();
        let mid = len / 2;
        let spot = (0..len)
            .map(|k| {
                if k % 2 == 0 {
                    mid + k / 2
                } else {
                    mid - (k + 1) / 2
                }
            })
            .filter(|a| *a >= 2 && *a <= len - 3)
            .find(|a| {
                [
                    self.opposite(a - 1, 1),
                    self.opposite(*a, 1),
                    self.opposite(a + 1, 1),
                    self.opposite(*a, 2),
                ]
                .into_iter()
                .all(|p| self.free(Rect::new(p.x, p.y, 1, 1)))
            });
        if let Some(a) = spot {
            let shrine = self.opposite(a, 1);
            let before = self.opposite(a, 2);
            self.put(shrine, Furniture::Shrine);
            self.put(self.opposite(a - 1, 1), Furniture::Shrine);
            self.put(self.opposite(a + 1, 1), Furniture::Shrine);
            self.station(before, Role::Shrine);
            let rect = Rect::new(shrine.x.min(before.x) - 1, shrine.y.min(before.y) - 1, 3, 3);
            if let Some(r) = rect.intersection(self.inner()) {
                self.rooms.push(Room {
                    rect: r,
                    kind: RoomKind::Shrine,
                    door: None,
                    private: false,
                    number: 0,
                });
            }
        }
    }

    /// Shelving rows and a locked office: a warehouse or a depot.
    fn hall(&mut self) {
        let inner = self.inner();
        self.corner_room(5, 4, RoomKind::Office, true);
        self.furnish_room(inner, RoomKind::Hall, None);
        self.rooms.push(Room {
            rect: inner,
            kind: RoomKind::Hall,
            door: None,
            private: false,
            number: 0,
        });
    }

    /// Machine rows with a station at each, a lockers room, a locked office.
    fn factory(&mut self) {
        let inner = self.inner();
        self.corner_room(5, 4, RoomKind::Office, true);
        self.corner_room(5, 4, RoomKind::Lockers, false);
        let mut y = inner.y + 2;
        while y < inner.bottom() - 2 {
            let mut x = inner.x + 1;
            while x < inner.right() - 1 {
                let p = Pos::new(x, y);
                if self.free(Rect::new(x, y, 1, 2)) && self.put(p, Furniture::Machine) {
                    self.station(p.offset(0, 1), Role::Machine);
                }
                x += 2;
            }
            y += 4;
        }
        self.rooms.push(Room {
            rect: inner,
            kind: RoomKind::Hall,
            door: None,
            private: false,
            number: 0,
        });
    }

    /// Bays three wide along the wall opposite the door, a locked office.
    fn garage(&mut self) {
        let inner = self.inner();
        self.corner_room(5, 4, RoomKind::Office, true);
        let len = self.opposite_len();
        let mut a = 2;
        while a + 3 <= len - 2 {
            // A stub wall three deep marks the bay's side; the station is its mouth.
            for d in 1..=3 {
                let p = self.opposite(a, d);
                if self.inner().contains(p)
                    && !self.reserved.contains(&p)
                    && self.tiles[p].terrain == Terrain::Floor
                {
                    self.tiles[p].terrain = Terrain::Wall;
                }
            }
            let mouth = self.opposite(a + 2, 4);
            self.station(mouth, Role::Bay);
            a += 4;
        }
        self.rooms.push(Room {
            rect: inner,
            kind: RoomKind::Bays,
            door: None,
            private: false,
            number: 0,
        });
    }

    // ---- finishing --------------------------------------------------------

    fn lamps(&mut self) {
        let inner = self.inner();
        for p in inner.iter() {
            if p.x % 6 == 3
                && p.y % 6 == 3
                && self.tiles[p].terrain == Terrain::Floor
                && self.tiles[p].feature.is_none()
            {
                self.tiles[p].feature = Some(Feature::Lamp);
            }
        }
    }

    /// Every walkable tile reaches every other: doors through inner walls where two
    /// pieces meet.
    fn connect(&mut self) {
        let size = self.plan.size;
        let inner = self.inner();
        for _ in 0..16 {
            let comps = components(size, |p| self.tiles[p].is_walkable());
            if comps.len() <= 1 {
                return;
            }
            // The main piece is the one with the shafts, else the largest.
            let anchor = self.plan.lift.unwrap_or(self.plan.stair);
            let main_idx = comps.iter().position(|c| c.contains(&anchor)).unwrap_or(0);
            let mut in_main = crate::geom::Grid::filled(size, false);
            for p in &comps[main_idx] {
                in_main.set(*p, true);
            }
            let other = (0..comps.len())
                .find(|i| *i != main_idx)
                .expect("two pieces");
            let start = comps[other][0];
            let path = bfs_nearest(
                size,
                start,
                |p| {
                    inner.contains(p)
                        && (self.tiles[p].is_walkable()
                            || self.tiles[p].terrain == Terrain::Wall
                            || matches!(self.tiles[p].feature, Some(Feature::Furniture(_))))
                },
                |p| in_main[p],
                48,
            );
            match path {
                Some(path) => {
                    for p in path {
                        if !inner.contains(p) {
                            continue;
                        }
                        if self.tiles[p].terrain == Terrain::Wall {
                            self.tiles[p].terrain = Terrain::Door;
                        } else if matches!(self.tiles[p].feature, Some(Feature::Furniture(_))) {
                            self.tiles[p].feature = None;
                            self.stations.retain(|s| s.pos != p);
                        }
                    }
                }
                None => return,
            }
        }
    }

    fn finish(self) -> Floor {
        let light = lighting::compute(&self.tiles, BASE_LIGHT, |t| match t.feature {
            Some(Feature::Lamp) => Some((6, 120.0)),
            _ => None,
        });
        let mut stations = self.stations;
        stations.retain(|s| self.tiles[s.pos].is_walkable());
        Floor {
            level: self.level,
            tiles: self.tiles,
            light,
            rooms: self.rooms,
            exits: self.exits,
            stations,
        }
    }
}

pub(super) fn generate(city: &City, plan: &FloorPlan, level: i32) -> Floor {
    let mut b = Builder::new(city, plan, level);
    b.exits();
    b.shafts();
    match plan.floor_type(level) {
        FloorType::Lobby => b.lobby(),
        FloorType::Corridor => b.corridor(FloorType::Corridor),
        FloorType::Suite => b.suite(),
        FloorType::Basement => b.basement(),
        FloorType::Vip => b.corridor(FloorType::Vip),
        FloorType::Bar => b.bar(plan.kind == crate::world::BuildingKind::Club),
        FloorType::Clinic => b.clinic(),
        FloorType::Temple => b.temple(),
        FloorType::Factory => b.factory(),
        FloorType::Garage => b.garage(),
        FloorType::Hall => b.hall(),
    }
    b.lamps();
    b.connect();
    b.finish()
}
