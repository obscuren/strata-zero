//! An atrium floor: bands of flats on all four sides, a walkway ring inside them,
//! the railing and the open drop in the middle.

use super::Builder;
use crate::geom::{Pos, Rect};
use crate::interior::floor::{Room, RoomKind};
use crate::interior::plan::{BAND, WALK};
use crate::tile::Terrain;
use rand::Rng;

/// A run of flats along one side: tiles `lo..=hi` along it, `c0..=c1` across, the
/// flat wall on line `wall` with the walkway on line `out`, which runs along
/// `dlo..=dhi`.
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
        let (lx0, lx1) = (w / 2 - 3, w / 2 + 2);
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
        // Clockwise from the top left.
        let bands = [
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
        ];
        let mut number = 0u16;
        for (i, band) in bands.iter().enumerate() {
            let laundry = i == 3 && self.level.rem_euclid(3) == 0;
            self.flats(band, laundry, &mut number);
        }
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
