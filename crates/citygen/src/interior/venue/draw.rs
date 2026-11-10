//! A venue's level drawn as a floor: wall wherever no room stands, round each walled room
//! and over a ring's core, its doors, landings, ways in and set pieces, then each room
//! furnished and the floor lit. There is no pass that cuts doors to join it up: the layout
//! is joined by construction, and the checker proves it.

use super::{LevelLayout, PieceKind, PlacedPiece, PlannedRoom, VenueLayout, WayInKind};
use crate::geom::{Dir, Pos, Rect};
use crate::interior::floor::{Role, Room, RoomKind, doorstep_of};
use crate::interior::layouts::{Builder, walled};
use crate::interior::{Floor, FloorPlan, LinkKind};
use crate::rng::hash64;
use crate::tile::{Feature, Furniture, Terrain};
use rand::Rng;

/// Level `level` of `layout` as a floor; None for a level it doesn't have. Its rng and
/// its fittings' wear come from the layout's seed and the level, its lights' moods from
/// the layout's seed: it needs no city.
pub fn draw_level(layout: &VenueLayout, level: i32) -> Option<Floor> {
    let x = layout.level(level)?;
    let plan = FloorPlan::of_venue(layout);
    let seed = hash64(layout.seed, level as i64 as u64);
    let mut b = Builder::new(None, layout.seed, &plan, level, seed);
    b.rooms = rooms(x);
    walls(&mut b, x);
    // Below the street no side faces open ground.
    if level >= 0 {
        let faces: Vec<u8> = layout.frame.faces.iter().map(|d| edge(*d)).collect();
        b.windows(&faces);
    }
    doors(&mut b, x);
    landings(&mut b, layout);
    ways_in(&mut b, layout);
    exits(&mut b, x);
    b.reserved.extend(x.reserved.iter().copied());
    b.clear
        .extend(x.rooms.iter().flat_map(|r| r.spots.iter().copied()));
    for piece in &x.pieces {
        lay_piece(&mut b, x, piece);
    }
    for (i, r) in x.rooms.iter().enumerate() {
        if r.furniture.is_empty() && r.stations.is_empty() {
            let Room {
                rect, kind, door, ..
            } = b.rooms[i];
            b.furnish_room(rect, kind, door);
        } else {
            furnish(&mut b, r);
        }
    }
    b.light();
    Some(b.finish())
}

/// A frame side as `layouts::edge` numbers it.
fn edge(d: Dir) -> u8 {
    match d {
        Dir::W => 0,
        Dir::E => 1,
        Dir::N => 2,
        _ => 3,
    }
}

/// Wall inside the frame wall wherever no room stands, round each room of a kind with
/// walls of its own (its doors go in after), and over a ring's core.
fn walls(b: &mut Builder, x: &LevelLayout) {
    let inner = b.inner();
    for p in inner.iter() {
        b.tiles[p].terrain = Terrain::Wall;
    }
    for p in (b.rooms.iter().flat_map(|r| r.rect.iter())).filter(|p| inner.contains(*p)) {
        b.tiles[p].terrain = Terrain::Floor;
    }
    let rings: Vec<Pos> = (b.rooms.iter().filter(|r| walled(r)))
        .flat_map(|r| r.rect.grow(1).border())
        .filter(|p| inner.contains(*p))
        .collect();
    for p in rings
        .into_iter()
        .chain(x.core.iter().flat_map(|c| c.iter()))
    {
        b.tiles[p].terrain = Terrain::Wall;
    }
}

/// A door on each planned door but an open join (a lock's door is a door too), the tiles
/// beside every one kept clear, and the planned doors' locks.
fn doors(b: &mut Builder, x: &LevelLayout) {
    for d in &x.doors {
        if !d.open {
            b.tiles[d.at].terrain = Terrain::Door;
            b.reserved.insert(d.at);
        }
        b.clear.extend(d.at.neighbors4());
    }
    b.locks.extend(x.doors.iter().filter_map(|d| d.lock));
}

/// Each link's landing on this level as its kind's terrain, both tiles of a lift's bank,
/// a crawl's with its grille.
fn landings(b: &mut Builder, layout: &VenueLayout) {
    for k in &layout.links {
        for p in k.tiles(b.level) {
            b.tiles[p].terrain = k.kind.terrain();
            b.tiles[p].feature = (k.kind == LinkKind::Crawl).then_some(Feature::Vent);
            b.reserved.insert(p);
            b.clear.extend(p.neighbors4());
        }
    }
}

/// A grille on each crawl in to this level, and the locks of its doors in.
fn ways_in(b: &mut Builder, layout: &VenueLayout) {
    for w in layout.ways_in.iter().filter(|w| w.level == b.level) {
        if let WayInKind::Crawl { .. } = w.kind {
            b.tiles[w.at].feature = Some(Feature::Vent);
            b.reserved.insert(w.at);
            b.clear.extend(w.at.neighbors4());
        }
        b.locks.extend(w.lock);
    }
}

/// The level's doors to the city, and their doorsteps kept clear.
fn exits(b: &mut Builder, x: &LevelLayout) {
    for e in &x.exits {
        b.tiles[e.at].terrain = Terrain::Door;
        b.tiles[e.at].feature = None;
        let step = doorstep_of(b.size, e.at);
        b.reserved.extend([e.at, step]);
        b.clear.extend(step.neighbors4());
        b.doorsteps.push(step);
        b.exits.push(*e);
    }
}

/// A set piece: a track's rails and the shutters at its ends, a ramp's rails along its
/// long sides; a gate line is its doors. Nothing stands on a piece.
fn lay_piece(b: &mut Builder, x: &LevelLayout, piece: &PlacedPiece) {
    let r = piece.rect;
    match piece.kind {
        PieceKind::Track => {
            for p in r.iter() {
                b.tiles[p].terrain = Terrain::Track;
                b.tiles[p].feature = None;
            }
            for &end in &piece.ends {
                b.tiles[end].terrain = Terrain::Wall;
                b.tiles[end].feature = Some(Feature::Shutter);
            }
        }
        PieceKind::Ramp => {
            let park = (x.rooms.iter())
                .find(|room| room.kind == RoomKind::CarPark && room.rect.contains(r.center()))
                .map(|room| room.rect);
            let sides: Vec<Pos> = if r.w < r.h {
                (r.y..r.bottom())
                    .flat_map(|y| [Pos::new(r.x - 1, y), Pos::new(r.right(), y)])
                    .collect()
            } else {
                (r.x..r.right())
                    .flat_map(|x| [Pos::new(x, r.y - 1), Pos::new(x, r.bottom())])
                    .collect()
            };
            for p in sides {
                if park.is_some_and(|k| k.contains(p)) && b.open(p) && keeps_way(b, p) {
                    b.tiles[p].feature = Some(Feature::Barrier);
                }
            }
        }
        PieceKind::GateLine => {}
    }
    b.clear.extend(r.iter());
}

/// The planned rooms, named and zoned. A room past the spine with one door has it as its
/// door; a spine room is the way through, with none.
fn rooms(x: &LevelLayout) -> Vec<Room> {
    let spine = x.bands.len();
    (x.rooms.iter().enumerate())
        .map(|(i, r)| {
            let doors: Vec<Pos> = (x.doors.iter())
                .filter(|d| !d.open && (d.rooms.0 == i || d.rooms.1 == i))
                .map(|d| d.at)
                .collect();
            let door = (i >= spine && doors.len() == 1).then(|| doors[0]);
            Room {
                name: Some(r.name.clone()),
                zone: r.zone,
                ..Room::new(r.rect, r.kind, door, r.private, r.number)
            }
        })
        .collect()
}

/// A room furnished from its def: each piece of its furniture along its walls, round from
/// a tile the rng picks with a tile between pieces, then a station for each of its roles
/// beside a piece nobody stands at yet, the role's own kind first (a desk's for a desk
/// station); with no piece a station fits beside, the free tile nearest the room's middle.
fn furnish(b: &mut Builder, r: &PlannedRoom) {
    let rect = r.rect;
    let mut along: Vec<Pos> = round(rect)
        .into_iter()
        .filter(|p| {
            p.neighbors4().any(|n| {
                !rect.contains(n)
                    && b.size.contains(n)
                    && matches!(b.tiles[n].terrain, Terrain::Wall | Terrain::Window)
            })
        })
        .collect();
    let mut placed: Vec<(Pos, Furniture)> = Vec::new();
    if !along.is_empty() {
        let start = b.rng.random_range(0..along.len());
        along.rotate_left(start);
        let mut from = 0;
        for &piece in &r.furniture {
            let n = along.len();
            let Some(k) = (from..n).chain(0..from).find(|&k| fits(b, along[k])) else {
                continue;
            };
            b.put(along[k], piece);
            placed.push((along[k], piece));
            from = (k + 2) % n;
        }
    }
    let mut staffed = vec![false; placed.len()];
    for &role in &r.stations {
        let own = |j: usize| Some(placed[j].1) == piece_of(role);
        let mut order: Vec<usize> = (0..placed.len()).collect();
        order.sort_by_key(|&j| (staffed[j], !own(j)));
        let by_piece = order
            .into_iter()
            .find_map(|j| beside(b, rect, placed[j].0).map(|p| (j, p)));
        match by_piece {
            Some((j, p)) => {
                b.station(p, role);
                staffed[j] = true;
            }
            None => {
                let middle = rect.center();
                let free = (rect.iter())
                    .filter(|p| b.open(*p) && b.station_free(*p))
                    .min_by_key(|p| (p.manhattan(middle), p.y, p.x));
                if let Some(p) = free {
                    b.station(p, role);
                }
            }
        }
    }
}

/// The furniture a role's station stands beside.
fn piece_of(role: Role) -> Option<Furniture> {
    match role {
        Role::Reception | Role::Bar => Some(Furniture::Counter),
        Role::Desk => Some(Furniture::Desk),
        Role::Table => Some(Furniture::Table),
        Role::Machine => Some(Furniture::Machine),
        Role::Surgery => Some(Furniture::Chair),
        Role::Shrine => Some(Furniture::Shrine),
        Role::Shelf => Some(Furniture::Shelf),
        Role::Bay => None,
    }
}

/// The free tile beside a piece at `at` in `rect`, nearest the room's middle.
fn beside(b: &Builder, rect: Rect, at: Pos) -> Option<Pos> {
    let middle = rect.center();
    at.neighbors4()
        .filter(|n| rect.contains(*n) && b.open(*n) && b.station_free(*n))
        .min_by_key(|n| (n.manhattan(middle), n.y, n.x))
}

/// Whether a piece of furniture may stand at `p`: free floor nobody stands on, where it
/// cuts no way.
fn fits(b: &Builder, p: Pos) -> bool {
    b.open(p) && b.station_free(p) && keeps_way(b, p)
}

/// Whether a tile at `p` that stops feet leaves the walkable tiles beside it joined round
/// it: they lie in one run round its eight neighbours, so no way through `p` is lost.
fn keeps_way(b: &Builder, p: Pos) -> bool {
    use Dir::*;
    let walk = [N, NE, E, SE, S, SW, W, NW]
        .map(|d| p.step(d))
        .map(|q| b.size.contains(q) && b.tiles[q].is_walkable());
    if walk.iter().all(|w| *w) {
        return true;
    }
    // Each run of walkable tiles round the ring, counted where it holds a side tile
    // (N, E, S or W, the even places): those are the ones `p` joins.
    let runs = (0..8)
        .filter(|&i| walk[i] && !walk[(i + 7) % 8])
        .filter(|&i| {
            (0..8)
                .map(|k| (i + k) % 8)
                .take_while(|&j| walk[j])
                .any(|j| j % 2 == 0)
        })
        .count();
    runs <= 1
}

/// The tiles round the edge of `rect`, clockwise from its north-west corner.
fn round(rect: Rect) -> Vec<Pos> {
    let (x0, y0, x1, y1) = (rect.x, rect.y, rect.right() - 1, rect.bottom() - 1);
    let mut v: Vec<Pos> = (x0..=x1).map(|x| Pos::new(x, y0)).collect();
    v.extend((y0 + 1..=y1).map(|y| Pos::new(x1, y)));
    if y1 > y0 {
        v.extend((x0..x1).rev().map(|x| Pos::new(x, y1)));
    }
    if x1 > x0 {
        v.extend((y0 + 1..y1).rev().map(|y| Pos::new(x0, y)));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interior::venue::testutil::{city, test_type};
    use crate::interior::venue::{PieceKind, VenueFrame, VenueLayout, WayInKind, lay_out};
    use crate::interior::{FloorPlan, LinkKind, RoomKind, generate_floor, validate_floor};
    use crate::tile::{Feature, Terrain};
    use crate::world::StratumKind;

    /// The test type over its own frame, seeds 0..10.
    fn layouts() -> Vec<(u64, VenueLayout)> {
        let t = test_type();
        (0..10)
            .map(|seed| (seed, lay_out(&t, VenueFrame::own(&t), seed).unwrap()))
            .collect()
    }

    #[test]
    fn every_level_draws_and_validates() {
        for (seed, layout) in layouts() {
            let plan = FloorPlan::of_layout(layout.clone());
            for l in &layout.levels {
                let f = draw_level(&layout, l.level).unwrap();
                validate_floor(&plan, &f)
                    .unwrap_or_else(|e| panic!("seed {seed} level {}: {e}", l.level));
                assert_eq!(f.tiles.size(), l.size);
                let planned: Vec<_> = (l.rooms.iter())
                    .map(|r| (r.rect, r.kind, Some(r.name.clone()), r.zone, r.number))
                    .collect();
                let drawn: Vec<_> = (f.rooms.iter())
                    .map(|r| (r.rect, r.kind, r.name.clone(), r.zone, r.number))
                    .collect();
                assert_eq!(drawn, planned);
                // The planned doors' locks, then the locks on the level's doors in.
                let ways = (layout.ways_in.iter()).filter(|w| w.level == l.level);
                let locks: Vec<_> = (l.doors.iter().filter_map(|d| d.lock))
                    .chain(ways.filter_map(|w| w.lock))
                    .collect();
                assert_eq!(f.locks, locks, "seed {seed} level {}", l.level);
                for k in &f.locks {
                    assert_eq!(f.lock_at(k.at), Some(k));
                    assert_eq!(f.tiles[k.at].terrain, Terrain::Door);
                }
                assert_eq!(f.exits, l.exits);
            }
        }
    }

    #[test]
    fn walls_stand_where_no_room_is_and_open_joins_have_no_door() {
        for (seed, layout) in layouts() {
            for l in &layout.levels {
                let f = draw_level(&layout, l.level).unwrap();
                let what = format!("seed {seed} level {}", l.level);
                let doors: Vec<_> = (l.doors.iter().filter(|d| !d.open).map(|d| d.at))
                    .chain(l.exits.iter().map(|e| e.at))
                    .collect();
                for p in l.size.rect().inset(1).iter() {
                    let held = l.rooms.iter().any(|r| r.rect.contains(p))
                        && !l.core.is_some_and(|c| c.contains(p));
                    let t = f.tiles[p].terrain;
                    if doors.contains(&p) {
                        assert_eq!(t, Terrain::Door, "{what}: {p:?}");
                    } else if held {
                        assert_ne!(t, Terrain::Wall, "{what}: {p:?} is in a room");
                    } else {
                        assert_eq!(t, Terrain::Wall, "{what}: {p:?} is in no room");
                    }
                }
                for d in l.doors.iter().filter(|d| d.open) {
                    assert_ne!(f.tiles[d.at].terrain, Terrain::Door, "{what}: {:?}", d.at);
                    assert!(f.tiles[d.at].is_walkable(), "{what}: {:?}", d.at);
                }
            }
        }
    }

    #[test]
    fn spots_stay_free_and_landings_carry_their_kind() {
        let mut crawls = 0;
        for (seed, layout) in layouts() {
            for l in &layout.levels {
                let f = draw_level(&layout, l.level).unwrap();
                let what = format!("seed {seed} level {}", l.level);
                for p in l.rooms.iter().flat_map(|r| &r.spots) {
                    let t = f.tiles[*p];
                    assert_eq!(t.terrain, Terrain::Floor, "{what}: spot {p:?}");
                    assert_eq!(t.feature, None, "{what}: spot {p:?}");
                    assert!(f.station_at(*p).is_none(), "{what}: spot {p:?}");
                }
                for k in &layout.links {
                    for p in k.tiles(l.level) {
                        assert_eq!(f.tiles[p].terrain, k.kind.terrain(), "{what}: {p:?}");
                        let vent = (k.kind == LinkKind::Crawl).then_some(Feature::Vent);
                        assert_eq!(f.tiles[p].feature, vent, "{what}: {p:?}");
                        crawls += usize::from(vent.is_some());
                    }
                }
                let ways = (layout.ways_in.iter()).filter(|w| w.level == l.level);
                for w in ways.filter(|w| matches!(w.kind, WayInKind::Crawl { .. })) {
                    let t = f.tiles[w.at];
                    assert_eq!(t.terrain, Terrain::Floor, "{what}: grille {:?}", w.at);
                    assert_eq!(t.feature, Some(Feature::Vent), "{what}: grille {:?}", w.at);
                    assert!(t.is_walkable());
                    crawls += 1;
                }
            }
        }
        assert!(crawls > 0, "no crawl came up");
    }

    #[test]
    fn the_track_is_walked_and_ends_in_shutters() {
        let mut tracks = 0;
        for (seed, layout) in layouts() {
            for l in &layout.levels {
                let f = draw_level(&layout, l.level).unwrap();
                let what = format!("seed {seed} level {}", l.level);
                for piece in l.pieces.iter().filter(|p| p.kind == PieceKind::Track) {
                    tracks += 1;
                    for p in piece.rect.iter() {
                        assert_eq!(f.tiles[p].terrain, Terrain::Track, "{what}: {p:?}");
                        assert!(f.tiles[p].is_walkable(), "{what}: {p:?}");
                        assert!(!f.tiles[p].blocks_sight(), "{what}: {p:?}");
                    }
                    assert_eq!(piece.ends.len(), 2);
                    for &end in &piece.ends {
                        assert_eq!(f.tiles[end].terrain, Terrain::Wall, "{what}: {end:?}");
                        assert_eq!(f.tiles[end].feature, Some(Feature::Shutter));
                        assert!(!f.tiles[end].is_walkable(), "{what}: {end:?}");
                    }
                }
                // Every track tile is its track room's, of its platform's zone.
                for (p, t) in f.tiles.iter() {
                    if t.terrain != Terrain::Track {
                        continue;
                    }
                    let track = (f.rooms.iter())
                        .find(|r| r.kind == RoomKind::Track && r.rect.contains(p))
                        .unwrap_or_else(|| panic!("{what}: {p:?} is in no track"));
                    let platform = (f.rooms.iter())
                        .find(|r| {
                            r.kind == RoomKind::Platform && r.rect.grow(1).intersects(track.rect)
                        })
                        .unwrap_or_else(|| panic!("{what}: the track at {p:?} has no platform"));
                    assert_eq!(track.zone, platform.zone, "{what}: {p:?}");
                }
            }
        }
        assert!(tracks > 0, "no track laid");
    }

    #[test]
    fn a_ramp_runs_between_rails() {
        let mut ramps = 0;
        for (seed, layout) in layouts() {
            for l in &layout.levels {
                let f = draw_level(&layout, l.level).unwrap();
                for piece in l.pieces.iter().filter(|p| p.kind == PieceKind::Ramp) {
                    ramps += 1;
                    let what = format!("seed {seed} level {}: {:?}", l.level, piece.rect);
                    for p in piece.rect.iter() {
                        assert!(f.tiles[p].is_walkable(), "{what}: {p:?}");
                    }
                    let rails: Vec<_> = (piece.rect.grow(1).iter())
                        .filter(|p| f.tiles[*p].feature == Some(Feature::Barrier))
                        .collect();
                    assert!(!rails.is_empty(), "{what}");
                    for p in rails {
                        let long = if piece.rect.w < piece.rect.h {
                            (p.x == piece.rect.x - 1 || p.x == piece.rect.right())
                                && (piece.rect.y..piece.rect.bottom()).contains(&p.y)
                        } else {
                            (p.y == piece.rect.y - 1 || p.y == piece.rect.bottom())
                                && (piece.rect.x..piece.rect.right()).contains(&p.x)
                        };
                        assert!(long, "{what}: a rail at {p:?}");
                    }
                }
            }
        }
        assert!(ramps > 0, "no ramp laid");
    }

    #[test]
    fn rooms_keep_their_doors_and_def_furniture() {
        for (seed, layout) in layouts() {
            for l in &layout.levels {
                let f = draw_level(&layout, l.level).unwrap();
                let what = format!("seed {seed} level {}", l.level);
                for (i, (r, planned)) in f.rooms.iter().zip(&l.rooms).enumerate() {
                    if planned.private {
                        let d = r
                            .door
                            .unwrap_or_else(|| panic!("{what}: room {i} has no door"));
                        assert!(l.doors.iter().any(|x| x.at == d && !x.open));
                    }
                    // A room past the spine with one door has it as its door.
                    let doors: Vec<_> = (l.doors.iter())
                        .filter(|d| !d.open && (d.rooms.0 == i || d.rooms.1 == i))
                        .map(|d| d.at)
                        .collect();
                    if i >= l.bands.len() && doors.len() == 1 {
                        assert_eq!(r.door, Some(doors[0]), "{what}: room {i}");
                    }
                    // Its def's furniture stands in it, by its walls.
                    let pieces: Vec<_> = (r.rect.iter())
                        .filter_map(|p| match f.tiles[p].feature {
                            Some(Feature::Furniture(x)) => Some((p, x)),
                            _ => None,
                        })
                        .collect();
                    for (p, x) in &pieces {
                        if !planned.furniture.is_empty() {
                            assert!(planned.furniture.contains(x), "{what}: {x:?} at {p:?}");
                        }
                    }
                    let walled = |p: Pos| {
                        p.neighbors4().any(|n| {
                            !r.rect.contains(n)
                                && matches!(f.tiles[n].terrain, Terrain::Wall | Terrain::Window)
                        })
                    };
                    for (p, _) in pieces.iter().filter(|_| !planned.furniture.is_empty()) {
                        assert!(walled(*p), "{what}: {p:?} stands off the walls");
                    }
                    for role in &planned.stations {
                        let n = planned.stations.iter().filter(|s| *s == role).count();
                        let drawn = (f.stations.iter())
                            .filter(|s| s.role == *role && r.rect.contains(s.pos))
                            .count();
                        assert!(drawn <= n, "{what}: {role:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_plan_carries_the_layout() {
        let t = test_type();
        let layout = lay_out(&t, VenueFrame::own(&t), 3).unwrap();
        let plan = FloorPlan::of_layout(layout.clone());
        let levels: Vec<i32> = layout.levels.iter().map(|l| l.level).collect();
        assert_eq!(plan.levels, levels);
        assert_eq!(plan.links, layout.links);
        assert_eq!(plan.size_of(-2), layout.level(-2).unwrap().size);
        assert_eq!(plan.size_of(0), layout.frame.size);
        assert_eq!(plan.size, layout.frame.size);
        assert_eq!(plan.door_level(StratumKind::Street), Some(0));
        let crawls = (layout.ways_in.iter())
            .filter(|w| matches!(w.kind, WayInKind::Crawl { .. }))
            .count();
        assert_eq!(plan.crawls.len(), crawls);
        assert!(crawls > 0);
        assert_eq!(plan.venue.as_deref(), Some(&layout));
        assert!(plan.atrium.is_none() && plan.home_level.is_none() && plan.clinic.is_none());
        let back: FloorPlan =
            rmp_serde::from_slice(&rmp_serde::to_vec_named(&plan).unwrap()).unwrap();
        assert_eq!(back, plan);
        assert_eq!(
            generate_floor(&city(), &plan, 0),
            draw_level(&layout, 0).unwrap()
        );
        assert!(draw_level(&layout, 40).is_none());
    }

    #[test]
    fn a_frame_door_on_a_level_the_layout_lacks_still_has_its_level_door() {
        let t = test_type();
        let mut frame = VenueFrame::own(&t);
        // The Skyways door on a level the type gives no role: the plan keeps its door,
        // which the layout bars.
        let short = frame.storeys.1;
        for d in frame
            .doors
            .iter_mut()
            .filter(|d| d.stratum == StratumKind::Skyways)
        {
            d.level = short + 3;
        }
        let layout = lay_out(&t, frame, 1).unwrap();
        let plan = FloorPlan::of_layout(layout.clone());
        assert_eq!(plan.door_level(StratumKind::Skyways), Some(short + 3));
        assert!(!plan.has_level(short + 3));
        assert!(
            layout
                .barred
                .iter()
                .any(|(k, _)| *k == StratumKind::Skyways)
        );
    }

    #[test]
    fn work_and_guest_floors_come_from_the_layout() {
        let t = test_type();
        let layout = lay_out(&t, VenueFrame::own(&t), 2).unwrap();
        let plan = FloorPlan::of_layout(layout.clone());
        let work = [
            Role::Desk,
            Role::Reception,
            Role::Bar,
            Role::Table,
            Role::Machine,
            Role::Shelf,
        ];
        let working = |x: &&LevelLayout| {
            (x.rooms.iter()).any(|r| r.stations.iter().any(|s| work.contains(s)))
        };
        let want: Vec<i32> = layout
            .levels
            .iter()
            .filter(working)
            .map(|x| x.level)
            .collect();
        assert!(!want.is_empty() && want.len() < layout.levels.len());
        assert_eq!(plan.work_floors(), want);
        // The test type has no guest rooms or flats.
        assert!(plan.corridor_floors().is_empty());
        // A guest room makes a guest floor; with nobody at work, the ground is.
        let mut other = layout.clone();
        other.levels[2].rooms[0].kind = RoomKind::HotelRoom;
        for r in other.levels.iter_mut().flat_map(|x| x.rooms.iter_mut()) {
            r.stations.clear();
        }
        let plan = FloorPlan::of_layout(other.clone());
        assert_eq!(plan.corridor_floors(), vec![other.levels[2].level]);
        assert_eq!(plan.work_floors(), vec![0]);
    }

    #[test]
    fn drawing_is_the_same_twice() {
        for (_, layout) in layouts().into_iter().take(3) {
            for l in &layout.levels {
                assert_eq!(draw_level(&layout, l.level), draw_level(&layout, l.level));
            }
        }
    }
}
