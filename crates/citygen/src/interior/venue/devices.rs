//! A site's devices on its floors: the terminals set in room walls and the switches of the
//! gate lines, placed by a pass of their own that reads the layout and changes nothing in it.
//! Rust names no room or piece here: the rooms a type gives terminals are data
//! (`VenueType.terminals`), the rest is by rule.

use super::{LevelLayout, PieceKind, VenueLayout, VenueType, WayInKind};
use crate::geom::{Pos, Rect};
use crate::interior::Floor;
use crate::rng::hash64;
use crate::tile::Terrain;
use crate::world::StratumKind;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::BTreeSet;

/// The terminal pass's salt, mixed into the layout seed.
pub const TERMINAL_SALT: u64 = 0x7E57_1A11;

/// A terminal set in a room's wall: the wall tile, and the floor tile in front of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terminal {
    pub level: i32,
    pub room: usize,
    pub at: Pos,
    pub front: Pos,
    pub public: bool,
}

/// A line of turnstiles' switch: set in the wall at an end of the line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineSwitch {
    pub level: i32,
    pub piece: usize,
    pub at: Pos,
    pub front: Pos,
}

/// The tiles a device's floor tile may not be: reserved, a room's spot, a door or its
/// doorstep, a station.
fn unfree(x: &LevelLayout, floor: &Floor) -> BTreeSet<Pos> {
    let mut v = held(x, floor);
    v.extend(x.reserved.iter().copied());
    v
}

/// `unfree` but for the reserved tiles: a room's spot, a door or its doorstep, a station.
/// A site's hub and server stand off these (`sitenet`).
pub(crate) fn held(x: &LevelLayout, floor: &Floor) -> BTreeSet<Pos> {
    let mut v: BTreeSet<Pos> = x
        .rooms
        .iter()
        .flat_map(|r| r.spots.iter().copied())
        .collect();
    for d in x.doors.iter().filter(|d| !d.open) {
        v.insert(d.at);
        v.extend(d.at.neighbors4());
    }
    v.extend(floor.stations.iter().map(|s| s.pos));
    v
}

/// Whether `p` is a free floor tile.
pub(crate) fn free(floor: &Floor, unfree: &BTreeSet<Pos>, p: Pos) -> bool {
    !unfree.contains(&p)
        && floor
            .tiles
            .get(p)
            .is_some_and(|t| t.terrain == Terrain::Floor && t.feature.is_none())
}

/// The wall tiles of the ring just outside `r`, corners excluded, each with its inside
/// neighbour.
fn ring(r: Rect) -> Vec<(Pos, Pos)> {
    let mut v = Vec::new();
    for x in r.x..r.right() {
        v.push((Pos::new(x, r.y - 1), Pos::new(x, r.y)));
        v.push((Pos::new(x, r.bottom()), Pos::new(x, r.bottom() - 1)));
    }
    for y in r.y..r.bottom() {
        v.push((Pos::new(r.x - 1, y), Pos::new(r.x, y)));
        v.push((Pos::new(r.right(), y), Pos::new(r.right() - 1, y)));
    }
    v
}

/// What the pass has used on a level: wall tiles and fronts.
struct Used {
    unfree: BTreeSet<Pos>,
    taken: BTreeSet<Pos>,
}

impl Used {
    /// The wall tiles round `rect` a terminal can be set in, sorted by `(y, x)`.
    fn candidates(&self, floor: &Floor, rect: Rect) -> Vec<(Pos, Pos)> {
        let mut v: Vec<(Pos, Pos)> = ring(rect)
            .into_iter()
            .filter(|&(at, front)| {
                !self.taken.contains(&at)
                    && !self.taken.contains(&front)
                    && floor
                        .tiles
                        .get(at)
                        .is_some_and(|t| t.terrain == Terrain::Wall)
                    && free(floor, &self.unfree, front)
            })
            .collect();
        v.sort_by_key(|&(at, _)| (at.y, at.x));
        v
    }
}

/// The room's own pick: one of `candidates`, drawn from the room's rng.
fn pick(layout: &VenueLayout, level: i32, room: usize, candidates: &[(Pos, Pos)]) -> (Pos, Pos) {
    let seed = hash64(
        hash64(hash64(layout.seed, TERMINAL_SALT), level as i64 as u64),
        room as u64,
    );
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    candidates[rng.random_range(0..candidates.len())]
}

/// The distance from `p` to the rect `r`.
fn rect_dist(r: Rect, p: Pos) -> i32 {
    let dx = (r.x - p.x).max(p.x - (r.right() - 1)).max(0);
    let dy = (r.y - p.y).max(p.y - (r.bottom() - 1)).max(0);
    dx + dy
}

/// The terminals of `level`, drawn as `floor`: the kiosk on G, a desk terminal in each room
/// `t` names, and with `gapped` the gapped level's terminal. Reads, changes nothing.
pub fn terminals(
    layout: &VenueLayout,
    t: Option<&VenueType>,
    level: i32,
    floor: &Floor,
    gapped: bool,
) -> Vec<Terminal> {
    let Some(x) = layout.level(level) else {
        return Vec::new();
    };
    let mut used = Used {
        unfree: unfree(x, floor),
        taken: BTreeSet::new(),
    };
    for s in line_switches(layout, level) {
        used.taken.insert(s.at);
        used.taken.insert(s.front);
    }
    let mut out: Vec<Terminal> = Vec::new();
    let mut put = |used: &mut Used, room: usize, (at, front): (Pos, Pos), public: bool| {
        used.taken.insert(at);
        used.taken.insert(front);
        out.push(Terminal {
            level,
            room,
            at,
            front,
            public,
        });
    };
    let rooms_of_zone = |zone: u8| -> Vec<usize> {
        (0..x.rooms.len())
            .filter(|&i| x.rooms[i].zone == zone)
            .collect()
    };

    // The kiosk and the gapped level's terminal always exist: the ruled room, the next
    // by the same rule, then any wall of the zone facing free floor.
    let mut must: Vec<(Vec<usize>, u8, bool)> = Vec::new();
    if level == 0 {
        let street = layout
            .ways_in
            .iter()
            .filter(|w| w.level == 0)
            .find(|w| {
                matches!(
                    w.kind,
                    WayInKind::Door {
                        stratum: StratumKind::Street,
                        ..
                    }
                )
            })
            .map(|w| w.room);
        let mut order: Vec<usize> = street.into_iter().collect();
        order.extend(rooms_of_zone(0).into_iter().filter(|i| Some(*i) != street));
        must.push((order, 0, true));
    }
    if gapped && let Some(low) = x.rooms.iter().map(|r| r.zone).min() {
        let landings: Vec<Pos> = layout.links.iter().flat_map(|l| l.tiles(level)).collect();
        let near = |i: usize| {
            landings
                .iter()
                .map(|p| rect_dist(x.rooms[i].rect, *p))
                .min()
                .unwrap_or(0)
        };
        let mut order = rooms_of_zone(low);
        order.sort_by_key(|&i| (near(i), i));
        must.push((order, low, false));
    }
    let mut placed: BTreeSet<usize> = BTreeSet::new();
    for (order, zone, public) in must {
        // A room already holding a terminal is passed by: each room has one.
        let found = order.iter().copied().find_map(|i| {
            if placed.contains(&i) {
                return None;
            }
            let c = used.candidates(floor, x.rooms[i].rect);
            (!c.is_empty()).then(|| (i, pick(layout, level, i, &c)))
        });
        // Then any wall tile of the zone facing free floor and, on a level with none (a
        // corridor of two tiles with landings on the rest), of the next zone in.
        let top = x.rooms.iter().map(|r| r.zone).max().unwrap_or(zone);
        let found = found.or_else(|| {
            (zone..=top).find_map(|z| {
                let mut walls: Vec<(Pos, Pos, usize)> = Vec::new();
                for i in rooms_of_zone(z) {
                    walls.extend(
                        used.candidates(floor, x.rooms[i].rect)
                            .into_iter()
                            .map(|(a, f)| (a, f, i)),
                    );
                }
                walls.sort_by_key(|&(a, _, i)| (a.y, a.x, i));
                walls.first().map(|&(a, f, i)| (i, (a, f)))
            })
        });
        if let Some((room, w)) = found {
            placed.insert(room);
            put(&mut used, room, w, public);
        }
    }

    if let Some(t) = t {
        for i in 0..x.rooms.len() {
            if placed.contains(&i) || !t.terminals.contains(&x.rooms[i].name) {
                continue;
            }
            let c = used.candidates(floor, x.rooms[i].rect);
            if !c.is_empty() {
                let w = pick(layout, level, i, &c);
                put(&mut used, i, w, false);
            }
        }
    }
    out.sort_by_key(|d| d.room);
    out
}

/// The switch of each gate line on `level`, in the level's piece order.
pub fn line_switches(layout: &VenueLayout, level: i32) -> Vec<LineSwitch> {
    let Some(x) = layout.level(level) else {
        return Vec::new();
    };
    let Some(floor) = super::draw_level(layout, level) else {
        return Vec::new();
    };
    let unfree = unfree(x, &floor);
    let mut out = Vec::new();
    for (piece, g) in x.pieces.iter().enumerate() {
        if g.kind != PieceKind::GateLine {
            continue;
        }
        let mut doors: Vec<_> = x.doors.iter().filter(|d| g.rect.contains(d.at)).collect();
        doors.sort_by_key(|d| (d.at.y, d.at.x));
        let (Some(first), Some(last)) = (doors.first(), doors.last()) else {
            continue;
        };
        let horizontal = g.rect.w >= g.rect.h;
        let step = |p: Pos, n: i32| {
            if horizontal {
                p.offset(n, 0)
            } else {
                p.offset(0, n)
            }
        };
        let across = |p: Pos, n: i32| {
            if horizontal {
                p.offset(0, n)
            } else {
                p.offset(n, 0)
            }
        };
        let (r0, r1) = first.rooms;
        let near = if x.rooms[r0].zone <= x.rooms[r1].zone {
            r0
        } else {
            r1
        };
        let try_at = |at: Pos| -> Option<LineSwitch> {
            if floor.tiles.get(at)?.terrain != Terrain::Wall {
                return None;
            }
            [across(at, -1), across(at, 1)]
                .into_iter()
                .find(|f| x.rooms[near].rect.contains(*f) && free(&floor, &unfree, *f))
                .map(|front| LineSwitch {
                    level,
                    piece,
                    at,
                    front,
                })
        };
        // Past an end of the line; where a room's side walls stand there (the line spans
        // its room), in the wall between the end turnstile and the next.
        let found = try_at(step(first.at, -1))
            .or_else(|| try_at(step(last.at, 1)))
            .or_else(|| try_at(step(first.at, 1)))
            .or_else(|| try_at(step(last.at, -1)));
        if let Some(s) = found {
            out.push(s);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interior::FloorPlan;
    use crate::interior::venue::testutil::{self, cities};
    use crate::interior::venue::{
        PlannedDoor, VenueFrame, VenuePack, draw_level, lay_out, venue_fingerprint,
    };

    /// Every builtin type laid on each closed building of the test cities whose kind it
    /// builds, and each type on its own frame.
    fn sites() -> Vec<(VenueLayout, &'static VenueType)> {
        let pack = VenuePack::builtin();
        let mut v = Vec::new();
        for t in &pack.types {
            for seed in 0..3 {
                v.push((lay_out(t, VenueFrame::own(t), seed).unwrap(), t));
            }
        }
        for c in cities() {
            for b in c.buildings.iter().filter(|b| b.kind.is_closed()) {
                let Some(t) = pack.for_kind(b.kind) else {
                    continue;
                };
                if let Some(plan) = FloorPlan::new(&c, b.id)
                    && let Some(l) = plan.venue
                {
                    v.push((*l, t));
                }
            }
        }
        v
    }

    #[test]
    fn placing_devices_changes_no_door_lock_or_code() {
        let mut placed = 0;
        for (layout, t) in sites() {
            let before = venue_fingerprint(&layout);
            let frozen = layout.clone();
            for x in &layout.levels {
                let floor = draw_level(&layout, x.level).unwrap();
                let spots: Vec<Pos> = x.rooms.iter().flat_map(|r| r.spots.clone()).collect();
                let found = terminals(&layout, Some(t), x.level, &floor, true);
                let walls = (found.iter().map(|d| (d.at, d.front)))
                    .chain((line_switches(&layout, x.level).into_iter()).map(|s| (s.at, s.front)));
                for (at, front) in walls {
                    placed += 1;
                    assert_eq!(
                        floor.tiles[at].terrain,
                        Terrain::Wall,
                        "{} {} {at:?}",
                        t.id,
                        x.level
                    );
                    assert_eq!(at.manhattan(front), 1);
                    assert!(
                        floor.tiles[front].terrain == Terrain::Floor
                            && floor.tiles[front].feature.is_none()
                    );
                    assert!(!x.reserved.contains(&front) && !spots.contains(&front));
                    assert!(!x.doors.iter().any(|d| d.at == at));
                }
                // The floor, its locks and its doors are as the layout alone drew them.
                assert_eq!(draw_level(&layout, x.level).unwrap(), floor);
            }
            assert_eq!(layout, frozen);
            assert_eq!(venue_fingerprint(&layout), before);
        }
        assert!(placed > 0, "the pass placed nothing");
    }

    #[test]
    fn every_site_has_a_public_terminal_at_g() {
        for (layout, t) in sites() {
            let g = draw_level(&layout, 0).unwrap();
            let x = layout.level(0).unwrap();
            let kiosks: Vec<Terminal> = terminals(&layout, Some(t), 0, &g, false)
                .into_iter()
                .filter(|d| d.public)
                .collect();
            assert_eq!(kiosks.len(), 1, "{}", t.id);
            assert_eq!(x.rooms[kiosks[0].room].zone, 0);
        }
    }

    #[test]
    fn every_air_gap_has_a_terminal_inside_it() {
        for (layout, t) in sites() {
            for x in layout.levels.iter().filter(|x| x.level != 0) {
                let floor = draw_level(&layout, x.level).unwrap();
                let lowest = x.rooms.iter().map(|r| r.zone).min().unwrap();
                let gap = terminals(&layout, Some(t), x.level, &floor, true);
                assert!(!gap.is_empty(), "{} {}", t.id, x.level);
                // Inside the lowest zone, unless it has no wall facing free floor at all.
                let spots: Vec<Pos> = x.rooms.iter().flat_map(|r| r.spots.clone()).collect();
                let open = x.rooms.iter().filter(|r| r.zone == lowest).any(|r| {
                    ring(r.rect).into_iter().any(|(at, front)| {
                        let tile = |p| floor.tiles.get(p).unwrap();
                        tile(at).terrain == Terrain::Wall
                            && tile(front).terrain == Terrain::Floor
                            && tile(front).feature.is_none()
                            && !x.reserved.contains(&front)
                            && !spots.contains(&front)
                    })
                });
                if open {
                    assert!(
                        gap.iter().any(|d| x.rooms[d.room].zone == lowest),
                        "{} {}",
                        t.id,
                        x.level
                    );
                }
            }
        }
    }

    #[test]
    fn devices_are_the_same_every_time() {
        for (layout, t) in sites() {
            for x in &layout.levels {
                let f = draw_level(&layout, x.level).unwrap();
                assert_eq!(
                    terminals(&layout, Some(t), x.level, &f, true),
                    terminals(&layout, Some(t), x.level, &f, true)
                );
                assert_eq!(
                    line_switches(&layout, x.level),
                    line_switches(&layout, x.level)
                );
            }
        }
    }

    #[test]
    fn a_switch_stands_at_an_end_of_its_line_on_the_near_side() {
        let mut lines_seen = 0;
        for (layout, _) in sites() {
            for x in &layout.levels {
                let lines: Vec<usize> = (0..x.pieces.len())
                    .filter(|&i| x.pieces[i].kind == PieceKind::GateLine)
                    .collect();
                let switches = line_switches(&layout, x.level);
                assert_eq!(switches.len(), lines.len());
                for s in switches {
                    lines_seen += 1;
                    let rect = x.pieces[s.piece].rect;
                    let mut doors: Vec<&PlannedDoor> =
                        x.doors.iter().filter(|d| rect.contains(d.at)).collect();
                    doors.sort_by_key(|d| (d.at.y, d.at.x));
                    assert!(!doors.iter().any(|d| d.at == s.at), "not on a turnstile");
                    let ends = [doors[0].at, doors[doors.len() - 1].at];
                    assert!(
                        ends.iter().any(|d| d.manhattan(s.at) == 1),
                        "{:?} at an end of {ends:?}",
                        s.at
                    );
                    let (r0, r1) = doors[0].rooms;
                    let near = if x.rooms[r0].zone <= x.rooms[r1].zone {
                        r0
                    } else {
                        r1
                    };
                    assert!(x.rooms[near].rect.contains(s.front));
                    assert_eq!(layout.gate_line_at(x.level, doors[0].at), Some(s.piece));
                }
            }
        }
        assert!(lines_seen > 0);
    }

    #[test]
    fn a_room_door_is_the_private_rooms_own() {
        for (layout, _) in sites() {
            for x in &layout.levels {
                for (i, r) in x.rooms.iter().enumerate().filter(|(_, r)| r.private) {
                    let d = layout.room_door(x.level, r.number);
                    let mut doors = x.doors.iter().filter(|d| !d.open);
                    assert_eq!(d.is_some(), doors.any(|d| d.rooms.0 == i || d.rooms.1 == i));
                }
                assert_eq!(layout.room_door(x.level, u16::MAX), None);
            }
        }
    }

    #[test]
    fn a_type_naming_no_room_for_a_terminal_is_refused() {
        let mut t = testutil::test_type();
        t.terminals.push("no such room".to_string());
        assert!(
            t.check(&VenuePack::builtin().sizes)
                .unwrap_err()
                .contains("terminal")
        );
    }
}
