//! An atrium floor: bands of flats on all four sides, a walkway ring inside them,
//! the railing and the open drop in the middle.

use super::Builder;
use super::home::HOME_DOOR;
use crate::geom::{Pos, Rect};
use crate::interior::floor::{Role, Room, RoomKind};
use crate::interior::plan::{BAND, WALK};
use crate::tile::{Feature, Furniture, Terrain};
use rand::Rng;

/// The home's run of the bottom band, its end walls included.
const HOME_RUN: i32 = 37;

/// A run of flats along one side: tiles `lo..=hi` along it, `c0..=c1` across, the
/// flat wall on line `wall` with the walkway on line `out`, which runs along
/// `dlo..=dhi`.
#[derive(Clone, Copy)]
struct Band {
    horizontal: bool,
    lo: i32,
    hi: i32,
    c0: i32,
    c1: i32,
    wall: i32,
    out: i32,
    dlo: i32,
    dhi: i32,
}

impl Band {
    fn at(&self, along: i32, across: i32) -> Pos {
        if self.horizontal {
            Pos::new(along, across)
        } else {
            Pos::new(across, along)
        }
    }

    /// The line `depth` tiles in from the band's back, the outer wall.
    fn depth(&self, depth: i32) -> i32 {
        if self.wall > self.c1 {
            self.c0 + depth
        } else {
            self.c1 - depth
        }
    }

    /// The room of tiles `a..end` along the band.
    fn rect(&self, a: i32, end: i32) -> Rect {
        let (span, depth) = (end - a, self.c1 - self.c0 + 1);
        if self.horizontal {
            Rect::new(a, self.c0, span, depth)
        } else {
            Rect::new(self.c0, a, depth, span)
        }
    }
}

/// The lift lobby's columns, in the middle of the top band.
fn lift_lobby(w: i32) -> (i32, i32) {
    (w / 2 - 3, w / 2 + 2)
}

impl Builder<'_> {
    /// Flats in bands on all four sides, a walkway of `WALK` inside the flat walls,
    /// the railing around the drop; the shafts in a lift lobby in the middle of the
    /// top band.
    pub(super) fn atrium_floor(&mut self) {
        let (w, h) = (self.w(), self.h());
        let atrium = self.plan.atrium.expect("an atrium plan");
        let drop = atrium.inset(1);
        for p in atrium.iter() {
            self.tiles[p].terrain = if !drop.contains(p) {
                Terrain::Railing
            } else {
                Terrain::Atrium
            };
            self.tiles[p].feature = None;
        }
        // The flat walls: the top and bottom ones run the whole width, so those bands
        // own the corners.
        let (top, bottom) = (BAND, h - 1 - BAND);
        let (left, right) = (BAND, w - 1 - BAND);
        self.wall_line(Pos::new(1, top), Pos::new(w - 2, top));
        self.wall_line(Pos::new(1, bottom), Pos::new(w - 2, bottom));
        self.wall_line(Pos::new(left, top), Pos::new(left, bottom));
        self.wall_line(Pos::new(right, top), Pos::new(right, bottom));
        let (run_w, run_h) = (w - 2 * (BAND + 1), h - 2 * (BAND + 1 + WALK));
        for rect in [
            Rect::new(BAND + 1, BAND + 1, run_w, WALK),
            Rect::new(BAND + 1, bottom - WALK, run_w, WALK),
            Rect::new(BAND + 1, BAND + 1 + WALK, WALK, run_h),
            Rect::new(right - WALK, BAND + 1 + WALK, WALK, run_h),
        ] {
            self.rooms.push(Room {
                rect,
                kind: RoomKind::Corridor,
                door: None,
                private: false,
                number: 0,
            });
        }
        // The lift lobby: six wide in the middle of the top band, open to the walkway.
        let (lx0, lx1) = lift_lobby(w);
        self.wall_line(Pos::new(lx0 - 1, 1), Pos::new(lx0 - 1, top - 1));
        self.wall_line(Pos::new(lx1 + 1, 1), Pos::new(lx1 + 1, top - 1));
        for x in lx0..=lx1 {
            let p = Pos::new(x, top);
            if self.tiles[p].terrain == Terrain::Wall {
                self.tiles[p].terrain = Terrain::Floor;
            }
        }
        self.rooms.push(Room {
            rect: Rect::new(lx0, 1, lx1 - lx0 + 1, top),
            kind: RoomKind::Corridor,
            door: None,
            private: false,
            number: 0,
        });
        let bands = self.bands();
        let mut number = 0u16;
        for (i, band) in bands.into_iter().enumerate() {
            let laundry = i == 3 && self.level.rem_euclid(3) == 0;
            if i == 3 && self.plan.home_level == Some(self.level) {
                self.home_band(band, laundry, &mut number);
            } else {
                self.flats(&band, laundry, &mut number);
            }
        }
    }

    /// The bottom band of the home floor: the home in a centred run of `HOME_RUN`,
    /// walls included, flats either side; a side too short to reach the walkway is
    /// solid wall.
    fn home_band(&mut self, band: Band, laundry: bool, number: &mut u16) {
        let x0 = (self.w() - HOME_RUN) / 2;
        let x1 = x0 + HOME_RUN - 1;
        let left = Band { hi: x0 - 1, ..band };
        let right = Band { lo: x1 + 1, ..band };
        self.side_flats(&left, false, number);
        for x in [x0, x1] {
            self.wall_line(Pos::new(x, band.c0), Pos::new(x, band.c1));
        }
        *number += 1;
        let rect = Rect::new(x0 + 1, band.c0, HOME_RUN - 2, band.c1 - band.c0 + 1);
        let door = Pos::new(rect.x + HOME_DOOR, band.wall);
        self.home_flat(rect, door);
        self.rooms.push(Room {
            rect,
            kind: RoomKind::Home,
            door: Some(door),
            private: true,
            number: (self.level.max(0) as u16) * 100 + *number,
        });
        self.side_flats(&right, laundry, number);
    }

    /// Flats on one side of the home, or solid wall where no door could reach the
    /// walkway.
    fn side_flats(&mut self, band: &Band, laundry: bool, number: &mut u16) {
        if band.lo > band.hi {
            return;
        }
        if band.hi < band.dlo || band.lo > band.dhi {
            let r = band.rect(band.lo, band.hi + 1);
            self.wall_line(Pos::new(r.x, r.y), Pos::new(r.right() - 1, r.bottom() - 1));
            return;
        }
        self.flats(band, laundry, number);
    }

    /// The ground floor under the atrium: an open plaza with the reception inside
    /// the main door, stalls and tables under the drop, and a diner and shops in the
    /// bands where the flats are upstairs, open to the plaza.
    pub(super) fn plaza_floor(&mut self) {
        self.lobby();
        let mut diner = false;
        for mut band in self.bands() {
            // The side bands keep clear of the top and bottom ones' corners, so no
            // corner is walled in.
            if !band.horizontal {
                band.lo += WALK;
                band.hi -= WALK;
            }
            self.shopfronts(&band, &mut diner);
        }
        let under = self.plan.atrium.expect("an atrium plan").inset(1);
        // One piece every four tiles on every third row, so the plaza stays one
        // piece around them.
        let mut stall = true;
        let mut y = under.y + 1;
        while y < under.bottom() - 1 {
            let mut x = under.x + 1;
            while x < under.right() - 1 {
                let p = Pos::new(x, y);
                if stall && self.open(p) {
                    self.tiles[p].feature = Some(Feature::Stall);
                } else if !stall && self.put(p, Furniture::Table) {
                    self.put(p.offset(1, 0), Furniture::Stool);
                    self.station(p.offset(0, 1), Role::Table);
                }
                stall = !stall;
                x += 4;
            }
            y += 3;
        }
    }

    /// Cuts a band into a diner (the first that fits) and shops of 5–7 along, each
    /// walled from the next with a wide shopfront onto the plaza; a span by an exit,
    /// the reception or another shopfront stays open plaza.
    fn shopfronts(&mut self, band: &Band, diner: &mut bool) {
        let inner = self.inner();
        let mut prev_end = None;
        let mut a = band.lo;
        while a <= band.hi {
            let span = if *diner {
                self.rng.random_range(5..=7)
            } else {
                10
            };
            let end = a + span;
            if end > band.hi + 1 {
                break;
            }
            // The room, its front and end walls (the start wall unless the last shop
            // drew it) must be bare floor nobody needs, the line beyond the front not
            // walled off, and nothing standing or reserved a tile beyond the walls,
            // so a way stays open past the reception and the exits.
            let shared = prev_end == Some(a - 1);
            let first = if shared { a } else { a - 1 };
            let rect = band.rect(a, end);
            let walls = (band.c0..=band.c1)
                .flat_map(|c| {
                    let start = (!shared).then(|| band.at(a - 1, c));
                    start.into_iter().chain([band.at(end, c)])
                })
                .chain((first..=end).map(|m| band.at(m, band.wall)));
            let bare = |p: Pos| {
                !inner.contains(p)
                    || (self.tiles[p].terrain == Terrain::Floor && !self.clear.contains(&p))
            };
            let untouched = |p: Pos| {
                !inner.contains(p)
                    || (self.tiles[p].feature.is_none()
                        && !self.reserved.contains(&p)
                        && !self.stations.iter().any(|s| s.pos == p))
            };
            let fits = rect.iter().chain(walls).all(bare)
                && rect.grow(2).iter().all(untouched)
                && (a..end).all(|m| {
                    let p = band.at(m, band.out);
                    self.tiles[p].terrain == Terrain::Floor && self.tiles[p].feature.is_none()
                });
            if !fits {
                a += 1;
                continue;
            }
            // The end walls reach the front line; the front is open between them.
            for m in [a - 1, end] {
                self.wall_line(band.at(m, band.c0), band.at(m, band.wall));
                self.wall_line(band.at(m, band.wall), band.at(m, band.c1));
            }
            for m in a..end {
                self.clear.insert(band.at(m, band.wall));
                self.clear.insert(band.at(m, band.out));
            }
            // A counter across the back with the staff behind it, a way round at
            // each end.
            let (counter, behind) = (band.depth(1), band.depth(0));
            for m in a + 1..end - 1 {
                self.put(band.at(m, counter), Furniture::Counter);
            }
            let kind = if *diner {
                self.station(band.at(a + span / 2, behind), Role::Reception);
                RoomKind::Shop
            } else {
                self.station(band.at(a + span / 2, behind), Role::Bar);
                // Tables with a stool down the middle, a seat in front of each.
                let mut m = a + 1;
                while m + 1 < end - 1 {
                    if self.put(band.at(m, band.depth(4)), Furniture::Table) {
                        self.put(band.at(m + 1, band.depth(4)), Furniture::Stool);
                        self.station(band.at(m, band.depth(5)), Role::Table);
                    }
                    m += 4;
                }
                *diner = true;
                RoomKind::Bar
            };
            self.rooms.push(Room {
                rect,
                kind,
                door: None,
                private: false,
                number: 0,
            });
            prev_end = Some(end);
            a = end + 1;
        }
    }

    /// The five bands of flats, clockwise from the top left: the top band either
    /// side of the lift lobby, the right, the bottom and the left; the top and the
    /// bottom bands own the corners.
    fn bands(&self) -> [Band; 5] {
        let (w, h) = (self.w(), self.h());
        let (top, bottom) = (BAND, h - 1 - BAND);
        let (left, right) = (BAND, w - 1 - BAND);
        let (lx0, lx1) = lift_lobby(w);
        // A band: along `lo..=hi`, across `c0..=c1`, its wall and the walkway beyond.
        let band = |horizontal, lo, hi, c0, c1, wall, out| Band {
            horizontal,
            lo,
            hi,
            c0,
            c1,
            wall,
            out,
            dlo: BAND + 1,
            dhi: if horizontal {
                w - 2 - BAND
            } else {
                h - 2 - BAND
            },
        };
        [
            band(true, 1, lx0 - 2, 1, top - 1, top, top + 1),
            band(true, lx1 + 2, w - 2, 1, top - 1, top, top + 1),
            band(
                false,
                top + 1,
                bottom - 1,
                right + 1,
                w - 2,
                right,
                right - 1,
            ),
            band(true, 1, w - 2, bottom + 1, h - 2, bottom, bottom - 1),
            band(false, top + 1, bottom - 1, 1, left - 1, left, left + 1),
        ]
    }

    /// Cuts a band into flats of 5–8 along, each with its door on the walkway; a
    /// span with an exit's doorstep in it or on its walls is an open vestibule.
    fn flats(&mut self, band: &Band, laundry: bool, number: &mut u16) {
        let doorsteps = self.doorsteps.clone();
        let on_wall = |m: i32| (band.c0..=band.c1).any(|c| doorsteps.contains(&band.at(m, c)));
        let mut a = band.lo;
        while a <= band.hi {
            let rest = band.hi + 1 - a;
            let mut span = self.rng.random_range(5..=8);
            // The corner room reaches two tiles along the walkway, for its door.
            if a <= band.dlo {
                span = span.max(band.dlo + 2 - a);
            }
            // No sliver at the end: the last room takes it.
            if rest - span < 4 {
                span = rest;
            }
            // The next room must start where the walkway still runs, two tiles
            // before its end; else this one takes the rest.
            if span < rest && a + span + 1 > band.dhi - 1 {
                let fit = band.dhi - 2 - a;
                span = if fit >= 3 && !on_wall(a + fit) {
                    fit
                } else {
                    rest
                };
            }
            // No end wall through a doorstep: the room grows past it.
            while span < rest && on_wall(a + span) {
                span += 1;
            }
            if span < rest && (rest - span < 4 || a + span + 1 > band.dhi - 1) {
                span = rest;
            }
            if span < 3 {
                break;
            }
            let end = a + span;
            let rect = band.rect(a, end);
            if end <= band.hi {
                self.wall_line(band.at(end, band.c0), band.at(end, band.c1));
            }
            let facing = (a..end).filter(|m| (band.dlo..=band.dhi).contains(m));
            if rect.grow(1).iter().any(|p| doorsteps.contains(&p)) {
                for m in facing {
                    let p = band.at(m, band.wall);
                    if self.tiles[p].terrain == Terrain::Wall {
                        self.tiles[p].terrain = Terrain::Floor;
                    }
                }
                self.rooms.push(Room {
                    rect,
                    kind: RoomKind::Corridor,
                    door: None,
                    private: false,
                    number: 0,
                });
                a = end + 1;
                continue;
            }
            let (kind, private) = if laundry && end > band.hi {
                (RoomKind::Laundry, false)
            } else {
                (RoomKind::Flat, true)
            };
            // The door nearest the room's middle whose both sides are open floor.
            let mid = a + span / 2;
            let inside = if band.wall < band.out {
                band.wall - 1
            } else {
                band.wall + 1
            };
            let mut along: Vec<i32> = facing.collect();
            along.sort_by_key(|m| (m - mid).abs());
            let door = along
                .into_iter()
                .map(|m| {
                    (
                        band.at(m, band.wall),
                        band.at(m, inside),
                        band.at(m, band.out),
                    )
                })
                .find(|(d, i, o)| {
                    self.tiles[*d].terrain == Terrain::Wall
                        && self.tiles[*i].terrain == Terrain::Floor
                        && self.tiles[*o].terrain == Terrain::Floor
                })
                .map(|(d, _, _)| d)
                .filter(|d| self.door(*d));
            *number += 1;
            let room_number = if kind == RoomKind::Flat {
                (self.level.max(0) as u16) * 100 + *number
            } else {
                0
            };
            self.furnish_room(rect, kind, door);
            self.rooms.push(Room {
                rect,
                kind,
                door,
                private: private && door.is_some(),
                number: room_number,
            });
            a = end + 1;
        }
    }
}
