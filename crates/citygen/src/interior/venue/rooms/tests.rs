use super::super::zones::{Axes, front};
use crate::geom::{Pos, Rect};
use crate::interior::RoomKind;
use crate::interior::venue::testutil::{cities, test_type};
use crate::interior::venue::{
    LevelLayout, PieceKind, RoomDef, Spine, VenueFrame, VenueLayout, VenuePack, VenueType, lay_out,
    room_label, room_number,
};
use std::collections::BTreeSet;

fn layouts(t: &VenueType) -> Vec<(u64, VenueLayout)> {
    (0..20)
        .map(|s| (s, lay_out(t, VenueFrame::own(t), s).unwrap()))
        .collect()
}

fn axes(l: &VenueLayout, x: &LevelLayout) -> Axes {
    Axes::new(x.size, front(&l.frame))
}

/// The band whose rect holds all of `r`.
fn band_of(x: &LevelLayout, r: Rect) -> Option<usize> {
    x.bands
        .iter()
        .position(|b| b.rect.intersection(r) == Some(r))
}

/// The name of the role whose pieces name one of `kind`.
fn role_with(t: &VenueType, kind: PieceKind) -> String {
    let piece = &t.pieces.iter().find(|p| p.kind == kind).unwrap().name;
    let l = &t.levels;
    let mut roles = l
        .below
        .iter()
        .chain([&l.ground])
        .chain(&l.middle)
        .chain(&l.top);
    roles
        .find(|d| d.pieces.contains(piece))
        .unwrap()
        .name
        .clone()
}

/// The type with an own frame wide and deep enough for every room it lists.
fn roomy(t: &VenueType) -> VenueType {
    let mut t = t.clone();
    t.frame.footprint = (20, 12);
    t
}

/// The floor beside band `b`'s spine room, (rows, tiles across) a lot: each side of a
/// straight piece, a widened one or a ring; off a bent's turn and behind it.
fn lots(ax: Axes, x: &LevelLayout, b: usize) -> Vec<(i32, i32)> {
    let ((a0, a1), (c0, c1)) = ax.span(x.rooms[b].rect);
    let width = ax.width();
    if x.spine == Spine::Bent && b == x.bands.len() - 1 {
        let e = ax.span(x.bands[b].rect).0.1;
        vec![
            (e - a0, c0 - 1),
            (e - a0, width - c1 - 1),
            (e - a1 - 1, c1 - c0),
        ]
    } else {
        vec![(a1 - a0, c0 - 1), (a1 - a0, width - c1 - 1)]
    }
}

/// Whether a lot of (rows, tiles across) holds the smallest of `d`, either way round.
fn holds(lot: (i32, i32), d: &RoomDef) -> bool {
    let (w, h) = d.size.0;
    (w <= lot.1 && h <= lot.0) || (h <= lot.1 && w <= lot.0)
}

/// Every builtin type and the test type, over its own frame and every building of its
/// kinds in the test cities, seeds 0..3.
fn sweep() -> Vec<(String, VenueLayout)> {
    let cities = cities();
    let mut v = Vec::new();
    for t in VenuePack::builtin()
        .types
        .iter()
        .cloned()
        .chain([test_type()])
    {
        let buildings = cities.iter().flat_map(|c| {
            c.buildings
                .iter()
                .filter(|b| t.builds.contains(&b.kind))
                .filter_map(|b| VenueFrame::of_building(c, b.id))
        });
        for f in std::iter::once(VenueFrame::own(&t)).chain(buildings) {
            for seed in 0..3 {
                let what = format!("{} in {:?} {:?}, seed {seed}", t.id, f.kind, f.footprint);
                let l = lay_out(&t, f.clone(), seed).unwrap_or_else(|e| panic!("{what}: {e}"));
                v.push((what, l));
            }
        }
    }
    v
}

#[test]
fn basement_rooms_number_apart_from_the_lobby() {
    assert_eq!(room_number(0, 4), 4);
    assert_eq!(room_number(12, 3), 1203);
    assert_eq!(room_number(-1, 4), 9104);
    assert_eq!(room_number(-5, 12), 9512);
    assert_eq!(room_label(1203), "1203");
    assert_eq!(room_label(9104), "B1-04");
}

#[test]
fn rooms_fill_their_band_and_keep_its_zone() {
    let t = test_type();
    let (mut loops, mut sized) = (0, 0);
    for (seed, l) in layouts(&t) {
        for x in &l.levels {
            let what = format!("seed {seed} level {}", x.level);
            let def = t.role(&x.role).unwrap();
            let ax = axes(&l, x);
            let n = x.bands.len();
            let joined = |a: usize, b: usize| {
                x.doors
                    .iter()
                    .any(|d| d.rooms == (a, b) || d.rooms == (b, a))
            };
            for (i, r) in x.rooms.iter().enumerate() {
                let b = band_of(x, r.rect)
                    .unwrap_or_else(|| panic!("{what}: room {i} at {:?} in no band", r.rect));
                assert_eq!(r.zone, x.bands[b].zone, "{what}: room {i}");
                for (j, o) in x.rooms.iter().enumerate().skip(i + 1) {
                    assert!(!r.rect.intersects(o.rect), "{what}: rooms {i} and {j}");
                }
                if i < n {
                    assert_eq!(b, i, "{what}: spine room {i}");
                    continue;
                }
                // A track is joined to its platform, every other room to its band's
                // spine room.
                if r.kind == RoomKind::Track {
                    let platform = x.rooms.iter().position(|o| o.kind == RoomKind::Platform);
                    assert!(joined(platform.unwrap(), i), "{what}: the track");
                } else {
                    assert!(joined(b, i), "{what}: room {i} ({}) has no door", r.name);
                }
                let band = &def.bands[b];
                let (rows, across) = ax.span(r.rect);
                let (w, h) = (across.1 - across.0, rows.1 - rows.0);
                if band.rooms.contains(&r.name) {
                    // Its def's way round, or turned.
                    let d = t.room(&r.name).unwrap();
                    let ((w0, h0), (w1, h1)) = d.size;
                    let within = |w: i32, h: i32| (w0..=w1).contains(&w) && (h0..=h1).contains(&h);
                    assert!(
                        within(w, h) || within(h, w),
                        "{what}: {} is {w} x {h}, its range {:?}",
                        r.name,
                        d.size
                    );
                    sized += 1;
                } else if band.fill.as_ref() == Some(&r.name) {
                    assert!(w >= 3 && h >= 3, "{what}: a fill {} is {w} x {h}", r.name);
                }
            }
            // A door stands on the wall between its two rooms; an opening is a tile of
            // its second room, beside its first.
            for d in &x.doors {
                let (r0, r1) = (x.rooms[d.rooms.0].rect, x.rooms[d.rooms.1].rect);
                let beside = |r: Rect| d.at.neighbors4().any(|p| r.contains(p));
                if d.open {
                    assert!(r1.contains(d.at) && beside(r0), "{what}: {:?}", d.at);
                } else {
                    assert!(x.rooms.iter().all(|r| !r.rect.contains(d.at)), "{what}");
                    assert!(beside(r0) && beside(r1), "{what}: the door at {:?}", d.at);
                }
            }
            // Every border door joins a room of each of its two bands, across its cut.
            let mut first = BTreeSet::new();
            for d in x.doors.iter().filter(|d| d.border) {
                let (cut, c) = (ax.deep(d.at), ax.across(d.at));
                let j = x
                    .bands
                    .iter()
                    .position(|b| ax.span(b.rect).0.0 == cut)
                    .unwrap_or_else(|| panic!("{what}: the door at {:?} is on no cut", d.at));
                let (f, k) = (ax.pos(cut - 1, c), ax.pos(cut + 1, c));
                let (rf, rb) = (&x.rooms[d.rooms.0], &x.rooms[d.rooms.1]);
                assert!(
                    rf.rect.contains(f) && rb.rect.contains(k),
                    "{what}: {:?}",
                    d.at
                );
                assert_eq!(band_of(x, rf.rect), Some(j - 1), "{what}: {:?}", d.at);
                assert_eq!(band_of(x, rb.rect), Some(j), "{what}: {:?}", d.at);
                loops += usize::from(!first.insert(cut));
            }
        }
    }
    assert!(loops > 0, "a band with doors: 2 has its loop door");
    assert!(sized > 0);
}

#[test]
fn private_rooms_and_offices_take_the_levels_numbers() {
    let t = test_type();
    let mut numbered = 0;
    for (seed, l) in layouts(&t) {
        for x in &l.levels {
            let mut n = 0;
            for r in &x.rooms {
                let want = t.room(&r.name).is_some_and(|d| {
                    d.private
                        || matches!(
                            d.kind,
                            RoomKind::HotelRoom | RoomKind::Flat | RoomKind::Office
                        )
                });
                if want {
                    n += 1;
                    assert_eq!(r.number, room_number(x.level, n), "seed {seed}");
                } else {
                    assert_eq!(r.number, 0, "seed {seed}: {}", r.name);
                }
            }
            numbered += n;
        }
    }
    assert!(numbered > 0);
}

#[test]
fn listed_rooms_come_before_the_fill() {
    let t = test_type();
    for t in [t.clone(), roomy(&t)] {
        for (seed, l) in layouts(&t) {
            for x in &l.levels {
                let what = format!("seed {seed} level {}", x.level);
                let def = t.role(&x.role).unwrap();
                let ax = axes(&l, x);
                let n = x.bands.len();
                for (b, band) in def.bands.iter().enumerate() {
                    let fill = band.fill.as_ref();
                    if x.spine == Spine::Hall {
                        // A hall band is one room of its first listed def.
                        let first = band.rooms.first().or(fill).unwrap();
                        assert_eq!(&x.rooms[b].name, first, "{what}: band {b}");
                    }
                    let names: Vec<&String> = x
                        .rooms
                        .iter()
                        .skip(n)
                        .filter(|r| band_of(x, r.rect) == Some(b))
                        .map(|r| &r.name)
                        .filter(|m| band.rooms.contains(m) || fill == Some(*m))
                        .collect();
                    let split = names
                        .iter()
                        .position(|m| !band.rooms.contains(m))
                        .unwrap_or(names.len());
                    let (listed, rest) = names.split_at(split);
                    assert!(rest.iter().all(|m| fill == Some(*m)), "{what}: {names:?}");
                    // The listed rooms that fit, in list order, each once.
                    let mut order = band.rooms.iter();
                    for m in listed {
                        assert!(order.any(|o| o == *m), "{what}: band {b} {names:?}");
                    }
                    // Every listed room is there, unless no lot of its band holds its
                    // smallest size either way round.
                    if x.spine != Spine::Hall {
                        for m in &band.rooms {
                            let d = t.room(m).unwrap();
                            let room = lots(ax, x, b).iter().any(|l| holds(*l, d));
                            assert!(
                                listed.contains(&m) || !room,
                                "{what}: band {b} has room for {m}, yet holds {names:?}"
                            );
                        }
                    }
                    if x.spine == Spine::Hall {
                        assert!(names.is_empty(), "{what}: band {b} {names:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn doorsteps_landings_and_spots_stay_clear() {
    let mut spots = 0;
    for (what, l) in sweep() {
        for x in &l.levels {
            let what = format!("{what}, level {}", x.level);
            let reserved: BTreeSet<Pos> = x.reserved.iter().copied().collect();
            // A tile no room holds is wall.
            for p in &reserved {
                assert!(
                    x.rooms.iter().any(|r| r.rect.contains(*p)),
                    "{what}: a wall on {p:?}"
                );
                assert!(
                    !x.doors.iter().any(|d| !d.open && d.at == *p),
                    "{what}: a door on {p:?}"
                );
            }
            let near: BTreeSet<Pos> = x
                .doors
                .iter()
                .flat_map(|d| std::iter::once(d.at).chain(d.at.neighbors4()))
                .collect();
            let taken = |p: Pos| {
                reserved.contains(&p)
                    || near.contains(&p)
                    || x.core.is_some_and(|c| c.contains(p))
                    || x.pieces.iter().any(|q| q.rect.contains(p))
            };
            for r in &x.rooms {
                assert!(r.spots.len() <= 4, "{what}");
                for s in &r.spots {
                    assert!(
                        r.rect.inset(1).contains(*s),
                        "{what}: {s:?} in {:?}",
                        r.rect
                    );
                    assert!(!taken(*s), "{what}: the spot {s:?} isn't free");
                }
                spots += r.spots.len();
                let inner = r.rect.inset(1);
                if r.rect.w >= 3 && r.rect.h >= 3 && inner.iter().any(|p| !taken(p)) {
                    assert!(!r.spots.is_empty(), "{what}: {} has no spot", r.name);
                }
            }
        }
    }
    assert!(spots > 0);
}

#[test]
fn a_track_runs_the_far_side_and_ends_in_shutters() {
    let t = test_type();
    let role = role_with(&t, PieceKind::Track);
    for (seed, l) in layouts(&t) {
        let x = l.levels.iter().find(|x| x.role == role).unwrap();
        let what = format!("seed {seed} level {}", x.level);
        let ax = axes(&l, x);
        let (depth, width) = (ax.depth(), ax.width());
        let tracks: Vec<_> = x
            .pieces
            .iter()
            .filter(|p| p.kind == PieceKind::Track)
            .collect();
        assert_eq!(tracks.len(), 1, "{what}");
        let room = |k: RoomKind| x.rooms.iter().position(|r| r.kind == k).unwrap();
        let (platform, track) = (room(RoomKind::Platform), room(RoomKind::Track));
        // Both span the inner width: the platform 3 deep, then the track 2 deep against
        // the back wall.
        let span = |i: usize| ax.span(x.rooms[i].rect);
        assert_eq!(
            span(platform),
            ((depth - 5, depth - 2), (0, width)),
            "{what}"
        );
        assert_eq!(span(track), ((depth - 2, depth), (0, width)), "{what}");
        assert_eq!(tracks[0].rect, x.rooms[track].rect, "{what}");
        let back = x.bands.last().unwrap().zone;
        assert_eq!((x.rooms[platform].zone, x.rooms[track].zone), (back, back));
        // Its ends are the frame wall's tiles at each end of the track.
        let frame = x.size.rect();
        let ends = &tracks[0].ends;
        assert_eq!(ends, &vec![ax.pos(depth - 2, -1), ax.pos(depth - 2, width)]);
        for e in ends {
            assert!(frame.contains(*e) && !frame.inset(1).contains(*e), "{what}");
            assert!(e.neighbors4().any(|p| x.rooms[track].rect.contains(p)));
        }
        // The platform opens off the spine; the track off the platform's edge.
        let n = x.bands.len();
        let to = |a: usize, b: usize| x.doors.iter().find(|d| d.rooms == (a, b));
        assert!(to(n - 1, platform).is_some_and(|d| !d.open), "{what}");
        assert!(to(platform, track).is_some_and(|d| d.open), "{what}");
    }
}

#[test]
fn a_gate_line_turns_its_border_into_turnstiles() {
    let t = test_type();
    let role = role_with(&t, PieceKind::GateLine);
    let mut spines = BTreeSet::new();
    for (seed, l) in layouts(&t) {
        let x = l.levels.iter().find(|x| x.role == role).unwrap();
        let what = format!("seed {seed} level {}", x.level);
        let ax = axes(&l, x);
        let cut = ax.span(x.bands[1].rect).0.0;
        let (zf, zb) = (x.bands[0].zone, x.bands[1].zone);
        let b = t.border(zf, zb).unwrap();
        let gate: Vec<_> = x.doors.iter().filter(|d| ax.deep(d.at) == cut).collect();
        assert!(
            (3..=6).contains(&gate.len()),
            "{what}: {} doors",
            gate.len()
        );
        // One line, a tile apart, where the spine's single door was.
        let cols: Vec<i32> = gate.iter().map(|d| ax.across(d.at)).collect();
        assert!(
            cols.windows(2).all(|w| w[1] - w[0] == 2),
            "{what}: {cols:?}"
        );
        let line = ax.line();
        assert!(
            cols[0] <= line.0 && line.0 <= cols[cols.len() - 1],
            "{what}"
        );
        for d in &gate {
            let fit = d.fit.unwrap();
            assert!(d.border && !d.open, "{what}");
            assert_eq!(
                (fit.at, fit.kind, fit.zone),
                (d.at, b.lock, zf.max(zb)),
                "{what}"
            );
            assert_eq!(d.lock, b.always.then_some(fit), "{what}");
            assert_eq!(d.rooms, (0, 1), "{what}");
        }
        let pieces: Vec<_> = x
            .pieces
            .iter()
            .filter(|p| p.kind == PieceKind::GateLine)
            .collect();
        assert_eq!(pieces.len(), 1, "{what}");
        assert!(gate.iter().all(|d| pieces[0].rect.contains(d.at)), "{what}");
        spines.insert(format!("{:?}", x.spine));
    }
    assert!(spines.len() >= 2, "{spines:?}");
}

#[test]
fn a_ramp_is_two_by_four_against_a_wall() {
    let t = test_type();
    let role = role_with(&t, PieceKind::Ramp);
    // The band of the ramp's level that lists a car park, and the car park.
    let (band, park) = t
        .role(&role)
        .unwrap()
        .bands
        .iter()
        .enumerate()
        .find_map(|(b, band)| {
            let park = band
                .rooms
                .iter()
                .find(|m| t.room(m).unwrap().kind == RoomKind::CarPark)?;
            Some((b, park))
        })
        .unwrap();
    let mut ramps = 0;
    for (seed, l) in layouts(&t) {
        let x = l.levels.iter().find(|x| x.role == role).unwrap();
        let what = format!("seed {seed} level {}", x.level);
        // It has a ramp, unless its car park fits no lot of its band either way round
        // (on a hall, unless the band's room is another).
        let listed = &t.role(&role).unwrap().bands[band].rooms;
        let fits = if x.spine == Spine::Hall {
            listed.first() == Some(park)
        } else {
            let d = t.room(park).unwrap();
            lots(axes(&l, x), x, band).iter().any(|lot| holds(*lot, d))
        };
        let has = x.pieces.iter().any(|p| p.kind == PieceKind::Ramp);
        assert!(has || !fits, "{what}: the car park has room, yet no ramp");
        let inner = x.size.rect().inset(1);
        for p in x.pieces.iter().filter(|p| p.kind == PieceKind::Ramp) {
            ramps += 1;
            let r = p.rect;
            assert!(matches!((r.w, r.h), (2, 4) | (4, 2)), "{what}: {r:?}");
            let walls = [
                r.x == inner.x,
                r.y == inner.y,
                r.right() == inner.right(),
                r.bottom() == inner.bottom(),
            ];
            assert!(walls.contains(&true), "{what}: {r:?} off the frame wall");
            assert!(
                x.rooms
                    .iter()
                    .any(|c| c.kind == RoomKind::CarPark && c.rect.intersection(r) == Some(r)),
                "{what}: {r:?} in no car park"
            );
            assert!(r.iter().all(|q| !x.reserved.contains(&q)), "{what}");
        }
    }
    assert!(ramps > 0);
}

#[test]
fn middle_floors_differ() {
    let t = test_type();
    let middle = &t.levels.middle.as_ref().unwrap().name;
    let top = t.levels.top.len() as i32 + 6;
    for seed in 0..10 {
        let l = lay_out(&t, VenueFrame::own(&t).raised(top), seed).unwrap();
        let floors: Vec<Vec<Rect>> = l
            .levels
            .iter()
            .filter(|x| &x.role == middle)
            .map(|x| x.rooms.iter().map(|r| r.rect).collect())
            .collect();
        assert_eq!(floors.len(), 6);
        for w in floors.windows(2) {
            assert_ne!(w[0], w[1], "seed {seed}: neighbouring floors alike");
        }
        let mut kinds: Vec<&Vec<Rect>> = Vec::new();
        for f in &floors {
            if !kinds.contains(&f) {
                kinds.push(f);
            }
        }
        assert!(
            kinds.len() >= 3,
            "seed {seed}: {} kinds of floor",
            kinds.len()
        );
    }
}

#[test]
fn a_thin_band_holds_only_what_fits() {
    // Shallow real buildings thin a band to as little as 2 rows. The layout still lays,
    // and the band keeps only the listed rooms that fit it, in list order.
    let mut thin = 0;
    let pack = VenuePack::builtin();
    for (what, l) in sweep() {
        let t = pack.by_id(&l.type_id).cloned().unwrap_or_else(test_type);
        for x in l.levels.iter().filter(|x| x.spine != Spine::Hall) {
            let ax = axes(&l, x);
            let what = format!("{what}, level {}", x.level);
            let def = t.role(&x.role).unwrap();
            for (b, band) in x.bands.iter().enumerate() {
                let (rows, _) = ax.span(band.rect);
                let floor = rows.1 - rows.0 - i32::from(b > 0);
                let listed = &def.bands[b].rooms;
                let tall: Vec<&String> = listed
                    .iter()
                    .filter(|m| {
                        let (w, h) = t.room(m).unwrap().size.0;
                        w.min(h) > floor
                    })
                    .collect();
                if floor > 2 || tall.is_empty() {
                    continue;
                }
                thin += 1;
                let placed: Vec<&String> = x
                    .rooms
                    .iter()
                    .skip(x.bands.len())
                    .filter(|r| band_of(x, r.rect) == Some(b) && listed.contains(&r.name))
                    .map(|r| &r.name)
                    .collect();
                // Those too big for it either way round are left out; the rest keep their
                // list order.
                assert!(
                    tall.iter().all(|m| !placed.contains(m)),
                    "{what}: {placed:?}"
                );
                let mut order = listed.iter();
                for m in &placed {
                    assert!(order.any(|o| o == *m), "{what}: {placed:?}");
                }
            }
        }
    }
    assert!(thin > 0, "no band thinned under its listed rooms");
}
