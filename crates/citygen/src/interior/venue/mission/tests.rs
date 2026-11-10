use super::kept::must_levels;
use super::*;
use crate::interior::floor::doorstep_of;
use crate::interior::venue::testutil::test_type;
use crate::interior::venue::{
    GraphLock, Keys, MissionSize, NodeAt, VenueFrame, VenueGraph, VenuePack, WayInKind, Ways,
    draw_copy_level, lay_out,
};
use crate::interior::{FloorPlan, validate_floor};
use crate::tile::{Feature, Terrain};
use std::collections::BTreeSet;
use std::sync::OnceLock;

const SIZES: [MissionSize; 3] = [MissionSize::Small, MissionSize::Medium, MissionSize::Large];

/// One layer laid in a test.
struct Laid {
    what: String,
    layout: VenueLayout,
    rule: SizeRule,
    layer: MissionLayer,
}

/// The test type over its own frame, seeds 0..20, laid at each size and security 0 and
/// 255 for each objective that lies deep enough for the size (one at least), once for
/// every test.
fn layers() -> &'static [Laid] {
    static LAYERS: OnceLock<Vec<Laid>> = OnceLock::new();
    LAYERS.get_or_init(|| {
        let t = test_type();
        let pack = VenuePack::builtin();
        let mut v = Vec::new();
        for seed in 0..20 {
            let layout = lay_out(&t, VenueFrame::own(&t), seed).unwrap();
            for size in SIZES {
                let rule = pack.size(size);
                for security in [0, 255] {
                    let mut errs = Vec::new();
                    for objective in 0..t.objectives.len() {
                        let inputs = MissionInputs {
                            size,
                            objective,
                            security,
                            seed,
                        };
                        let what =
                            format!("seed {seed} {size:?} objective {objective} at {security}");
                        match MissionLayer::lay(&layout, &t, rule, inputs) {
                            Ok(layer) => v.push(Laid {
                                what,
                                layout: layout.clone(),
                                rule: rule.clone(),
                                layer,
                            }),
                            Err(e) => errs.push(format!("{what}: {e}")),
                        }
                    }
                    assert!(errs.len() < t.objectives.len(), "{errs:?}");
                }
            }
        }
        v
    })
}

/// The graph node of the room holding `at` on `level`.
fn node(g: &VenueGraph, l: &VenueLayout, level: i32, at: Pos) -> usize {
    let room = room_holding(l, level, at).expect("a room holds it");
    g.room(level, room).expect("a kept room")
}

/// The levels of `l` with a door to the city.
fn door_levels(l: &VenueLayout) -> Vec<i32> {
    let mut v: Vec<i32> = (l.levels.iter())
        .filter(|x| !x.exits.is_empty())
        .map(|x| x.level)
        .collect();
    v.sort_by_key(|&level| (level.abs(), level));
    v
}

#[test]
fn kept_floors_follow_the_order() {
    for x in layers() {
        let (l, kept, what) = (&x.layout, &x.layer.kept, &x.what);
        let objective = x.layer.objective.level;
        assert!(
            kept.contains(&0) && kept.contains(&objective),
            "{what}: {kept:?}"
        );
        let (least, most) = x.rule.floors;
        assert!(
            (usize::from(least)..=usize::from(most)).contains(&kept.len()),
            "{what}: {kept:?}"
        );
        assert!(kept.windows(2).all(|w| w[0] < w[1]), "{what}: lowest first");
        assert!(kept.iter().all(|k| l.level(*k).is_some()), "{what}");
        let must = must_levels(l, objective).unwrap();
        assert!(must.iter().all(|m| kept.contains(m)), "{what}: {must:?}");
        assert!(joined(l, kept), "{what}: {kept:?} aren't joined");
        // A kept level joins the others when the rest without it aren't joined.
        let joins = |k: i32| {
            let rest: Vec<i32> = kept.iter().copied().filter(|&j| j != k).collect();
            !joined(l, &rest)
        };
        // Door levels nearest G first: while one is left out, every kept level past
        // the ones the objective needs is a door level nearer G, or joins the rest.
        let doors: Vec<i32> = (door_levels(l).into_iter())
            .filter(|d| !must.contains(d))
            .collect();
        if let Some(i) = doors.iter().position(|d| !kept.contains(d)) {
            for &k in kept.iter().filter(|k| !must.contains(k)) {
                assert!(
                    doors[..i].contains(&k) || joins(k),
                    "{what}: {k} is kept while the door level {} isn't: {kept:?}",
                    doors[i]
                );
            }
        }
    }
}

#[test]
fn locks_raise_the_depth_to_the_size() {
    for x in layers() {
        let (l, layer, what) = (&x.layout, &x.layer, &x.what);
        let g = VenueGraph::copy(l, layer);
        let goal = g.goal.unwrap_or_else(|| panic!("{what}: no goal"));
        let d = g
            .depth(goal)
            .unwrap_or_else(|| panic!("{what}: nothing reaches it"));
        assert!(d >= u32::from(x.rule.locks.0), "{what}: depth {d}");
        let candidates = l.candidates();
        let mut seen = BTreeSet::new();
        for f in &layer.fitted {
            assert!(candidates.contains(f), "{what}: {f:?} is no candidate");
            assert!(
                layer.kept.contains(&f.0),
                "{what}: {f:?} on a level not kept"
            );
            assert!(seen.insert(*f), "{what}: {f:?} fitted twice");
            // A reader goes on a zone's border, never on a private room's door.
            let lv = l.level(f.0).unwrap();
            let d = &lv.doors[f.1];
            assert!(d.border && !d.open, "{what}: {f:?} is no border door");
            let (a, b) = d.rooms;
            assert!(
                !lv.rooms[a].private && !lv.rooms[b].private,
                "{what}: {f:?} is a private room's door"
            );
        }
    }
}

#[test]
fn every_card_and_code_lies_before_its_lock() {
    let base = Ways::default();
    let mut keys = 0;
    for x in layers() {
        let (l, layer, what) = (&x.layout, &x.layer, &x.what);
        let g = VenueGraph::copy(l, layer);
        let goal = g.goal.unwrap();
        assert_eq!(g.cards.len(), layer.cards.len(), "{what}");
        assert_eq!(g.codes.len(), layer.codes.len(), "{what}");
        // Reached by the base way holding the keys left before it, and where it can be
        // carried back from: to a way in, or (a key for the way out) to the objective.
        let lies_before = |h: &VenueGraph, n: usize| {
            let r = h.reach(&h.starts, base, Keys::default());
            let back = |to: &[usize]| h.returns(to, base, &r.keys)[n];
            r.nodes[n] && (back(&h.starts) || back(&[goal]))
        };
        for (i, c) in layer.cards.iter().enumerate() {
            let mut h = g.clone();
            h.cards.truncate(i);
            let n = node(&g, l, c.level, c.at);
            assert!(lies_before(&h, n), "{what}: card {i} {c:?}");
            assert!(c.zone > 0, "{what}: a card of the public zone");
        }
        for (i, c) in layer.codes.iter().enumerate() {
            let mut h = g.clone();
            h.codes.truncate(i);
            let n = node(&g, l, c.level, c.at);
            assert!(lies_before(&h, n), "{what}: code {i} {c:?}");
        }
        keys += layer.cards.len() + layer.codes.len();
        let held = g
            .route_in(base)
            .unwrap_or_else(|| panic!("{what}: no way in"));
        assert!(g.route_out(base, held), "{what}: no way out");
    }
    assert!(keys > 0, "no layer left a key");
}

#[test]
fn guards_number_with_size_and_security() {
    let t = test_type();
    for x in layers() {
        let (l, layer, what) = (&x.layout, &x.layer, &x.what);
        let (least, most) = x.rule.guards;
        let want = match layer.inputs.security {
            0 => least,
            255 => most,
            s => panic!("{what}: security {s}"),
        };
        assert_eq!(layer.guards.len(), usize::from(want), "{what}");
        let security = (t.security.iter())
            .find(|s| s.size == layer.inputs.size)
            .unwrap();
        let mut posts = BTreeSet::new();
        for p in &layer.guards {
            assert!(layer.kept.contains(&p.level), "{what}: {p:?}");
            let room = room_holding(l, p.level, p.at).unwrap();
            let zone = l.level(p.level).unwrap().rooms[room].zone;
            assert_eq!(zone, p.zone, "{what}: {p:?}");
            let name = &t.zones[usize::from(zone)];
            assert!(
                security.guards.iter().any(|(z, w)| z == name && *w > 0),
                "{what}: a guard in `{name}`"
            );
            assert!(posts.insert((p.level, p.at)), "{what}: two guards at {p:?}");
        }
    }
}

#[test]
fn no_post_blocks_a_landing_doorway_or_doorstep() {
    for x in layers() {
        let (l, layer, what) = (&x.layout, &x.layer, &x.what);
        for p in &layer.guards {
            assert_eq!(p.patrol.first(), Some(&p.at), "{what}: {p:?}");
            assert!((1..=4).contains(&p.patrol.len()), "{what}: {p:?}");
            let lv = l.level(p.level).unwrap();
            let landings: Vec<Pos> = l.links.iter().flat_map(|k| k.tiles(p.level)).collect();
            for &at in &p.patrol {
                let room = (lv.rooms.iter()).position(|r| r.spots.contains(&at));
                let room = room.unwrap_or_else(|| panic!("{what}: {at:?} is no spot"));
                assert_eq!(lv.rooms[room].zone, p.zone, "{what}: {at:?}");
                assert!(!landings.contains(&at), "{what}: {at:?} on a landing");
                assert!(!lv.doors.iter().any(|d| d.at == at), "{what}: {at:?}");
                for e in &lv.exits {
                    assert_ne!(e.at, at, "{what}");
                    assert_ne!(doorstep_of(lv.size, e.at), at, "{what}: a doorstep");
                }
                assert!(!lv.reserved.contains(&at), "{what}: {at:?} is reserved");
            }
        }
    }
}

#[test]
fn devices_stand_on_floor_spots() {
    let (mut turrets, mut containers) = (0, 0);
    for x in layers().iter().step_by(3) {
        let (l, layer, what) = (&x.layout, &x.layer, &x.what);
        let plan = FloorPlan::of_layout(l.clone());
        for lv in &l.levels {
            let Some(f) = draw_copy_level(l, layer, lv.level) else {
                assert!(
                    !layer.kept.contains(&lv.level),
                    "{what}: {} draws",
                    lv.level
                );
                continue;
            };
            assert!(layer.kept.contains(&lv.level), "{what}");
            let on = format!("{what} level {}", lv.level);
            validate_floor(&plan, &f).unwrap_or_else(|e| panic!("{on}: {e}"));
            let mut want: Vec<(Pos, Feature)> = Vec::new();
            for d in layer.devices.iter().filter(|d| d.level == lv.level) {
                want.push((
                    d.at,
                    match d.kind {
                        DeviceKind::Camera => Feature::Camera,
                        DeviceKind::Turret => Feature::Turret,
                    },
                ));
            }
            for c in layer.containers.iter().filter(|c| c.level == lv.level) {
                want.push((c.at, Feature::Container));
            }
            if layer.objective.level == lv.level {
                want.push((layer.objective.at, Feature::Objective));
            }
            let steps: Vec<Pos> = f.exits.iter().map(|e| f.doorstep(e)).collect();
            for &(at, feature) in &want {
                let t = f.tiles[at];
                assert_eq!(t.terrain, Terrain::Floor, "{on}: {feature:?} at {at:?}");
                assert_eq!(t.feature, Some(feature), "{on}: {at:?}");
                assert!(!steps.contains(&at), "{on}: {feature:?} on a doorstep");
                turrets += usize::from(feature == Feature::Turret);
                containers += usize::from(feature == Feature::Container);
            }
            let laid = |p: Pos, feature: Feature| want.contains(&(p, feature));
            for (p, t) in f.tiles.iter() {
                if let Some(
                    feature @ (Feature::Camera
                    | Feature::Turret
                    | Feature::Container
                    | Feature::Objective),
                ) = t.feature
                {
                    assert!(laid(p, feature), "{on}: a stray {feature:?} at {p:?}");
                }
            }
            for &(_, k) in layer.fitted.iter().filter(|f| f.0 == lv.level) {
                let fit = lv.doors[k].fit.unwrap();
                assert!(f.locks.contains(&fit), "{on}: door {k}'s lock isn't fitted");
            }
        }
    }
    assert!(
        turrets > 0 && containers > 0,
        "{turrets} turrets, {containers} containers"
    );
}

#[test]
fn turrets_and_containers_stand_clear_of_every_way() {
    let mut stood = 0;
    for x in layers() {
        let (l, layer, what) = (&x.layout, &x.layer, &x.what);
        let blockers: Vec<(i32, Pos)> = (layer.devices.iter())
            .filter(|d| d.kind == DeviceKind::Turret)
            .map(|d| (d.level, d.at))
            .chain(layer.containers.iter().map(|c| (c.level, c.at)))
            .collect();
        let posts: Vec<(i32, Pos)> = layer.guards.iter().map(|g| (g.level, g.at)).collect();
        for &(level, at) in &blockers {
            stood += 1;
            let lv = l.level(level).unwrap();
            let room = room_holding(l, level, at).unwrap();
            let r = &lv.rooms[room];
            assert!(r.spots.contains(&at), "{what}: {at:?} is no spot");
            for q in at.neighbors8() {
                let on = format!("{what}: beside {at:?} at {q:?}");
                assert!(r.rect.contains(q), "{on}: off its room");
                assert!(!lv.core.is_some_and(|c| c.contains(q)), "{on}: the core");
                assert!(!lv.doors.iter().any(|d| d.at == q), "{on}: a door");
                assert!(!lv.reserved.contains(&q), "{on}: reserved");
                assert!(
                    !lv.pieces.iter().any(|p| p.rect.grow(1).contains(q)),
                    "{on}"
                );
                assert!(
                    !blockers.contains(&(level, q)),
                    "{on}: another stands there"
                );
                assert!(!posts.contains(&(level, q)), "{on}: a guard stands there");
            }
        }
    }
    assert!(stood > 0);
}

#[test]
fn a_key_lies_where_it_can_be_carried_back() {
    let t = test_type();
    let rule = VenuePack::builtin().size(MissionSize::Small);
    let base = Ways::default();
    let mut tried = 0;
    for seed in 0..20 {
        let layout = lay_out(&t, VenueFrame::own(&t), seed).unwrap();
        let inputs = MissionInputs {
            size: MissionSize::Small,
            objective: 0,
            security: 0,
            seed,
        };
        let Ok(laid) = MissionLayer::lay(&layout, &t, rule, inputs) else {
            continue;
        };
        // Its doors fitted, and no key left yet.
        let mut layer = MissionLayer {
            cards: Vec::new(),
            codes: Vec::new(),
            ..laid
        };
        let mut g = VenueGraph::of(&layout, &layer.fitted, Some(&layer.kept));
        let goal = goal_of(&g, &layout, layer.objective).unwrap();
        g.goal = Some(goal);
        let starts = g.starts.clone();
        let used = BTreeSet::from([(layer.objective.level, layer.objective.at)]);
        let r = g.reach(&starts, base, Keys::default());
        // Every reached room of the deepest zone with a spot becomes a trap: in, and no
        // way back out.
        let room = |n: usize| match g.nodes[n] {
            NodeAt::Room { level, room } => Some((level, room)),
            NodeAt::Outside(_) => None,
        };
        let zone = |n: usize| room(n).map(|(level, i)| layout.level(level).unwrap().rooms[i].zone);
        let spotted = |n: usize| {
            room(n).is_some_and(|(level, i)| !free_spots(&layout, level, i, &used).is_empty())
        };
        let deepest = (0..g.nodes.len())
            .filter(|&n| r.nodes[n] && spotted(n))
            .filter_map(zone)
            .max();
        let traps: Vec<usize> = (0..g.nodes.len())
            .filter(|&n| r.nodes[n] && spotted(n) && zone(n) == deepest)
            .collect();
        let shut = GraphLock {
            zone: u8::MAX,
            ways: Ways::NET,
            code: None,
            free: false,
        };
        for e in g.edges.iter_mut().filter(|e| traps.contains(&e.from)) {
            e.lock = Some(shut);
        }
        let r = g.reach(&starts, base, Keys::default());
        let back = g.returns(&starts, base, &r.keys);
        let mut used = used;
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        if (layer.leave_key(&layout, &mut g, &r, &back, &[goal], &mut used, &mut rng)).is_err() {
            continue;
        }
        let left = (layer.cards.iter().map(|c| (c.level, c.at)))
            .chain(layer.codes.iter().map(|c| (c.level, c.at)))
            .next()
            .unwrap();
        let n = node(&g, &layout, left.0, left.1);
        assert!(!traps.contains(&n), "seed {seed}: a key in a trap");
        assert!(r.nodes[n] && back[n], "seed {seed}");
        tried += 1;
    }
    assert!(tried > 0, "no layout left a key past a trap");
}

#[test]
fn a_grille_needs_no_key() {
    let mut crawls = 0;
    for x in layers() {
        let (l, layer, what) = (&x.layout, &x.layer, &x.what);
        let g = VenueGraph::copy(l, layer);
        // The lowest-zone lock out of a start is a crawl in's grille.
        let grille = (g.edges.iter())
            .filter(|e| g.starts.contains(&e.from))
            .filter_map(|e| e.lock)
            .min_by_key(|k| k.zone)
            .is_some_and(|k| k.ways == Ways::CRAWL);
        let crawl_in = (l.ways_in.iter())
            .any(|w| matches!(w.kind, WayInKind::Crawl { .. }) && layer.kept.contains(&w.level));
        if !(grille && crawl_in) {
            continue;
        }
        crawls += 1;
        assert!(layer.cards.iter().all(|c| c.zone > 0), "{what}");
        // Every code opens a keypad: a lock that takes a code, or a private room's.
        for c in &layer.codes {
            let (level, number) = c.lock;
            let lv = l.level(level).unwrap();
            let pad = (lv.doors.iter())
                .filter_map(|d| d.lock.or(d.fit))
                .chain(
                    (l.ways_in.iter())
                        .filter(|w| w.level == level)
                        .filter_map(|w| w.lock),
                )
                .any(|k| k.number == number && k.ways.has(Ways::CODE))
                || lv.rooms.iter().any(|r| r.private && r.number == number);
            assert!(pad, "{what}: the code for {:?} opens nothing", c.lock);
        }
    }
    assert!(crawls > 0, "no copy kept a crawl in");
}

#[test]
fn the_same_inputs_lay_the_same_layer() {
    let t = test_type();
    for x in layers().iter().step_by(7) {
        let again = MissionLayer::lay(&x.layout, &t, &x.rule, x.layer.inputs).unwrap();
        assert_eq!(again, x.layer, "{}", x.what);
    }
    // Another mission seed lays another layer.
    let x = &layers()[0];
    let other = MissionInputs {
        seed: x.layer.inputs.seed + 100,
        ..x.layer.inputs
    };
    let differ = (0..5).any(|k| {
        let inputs = MissionInputs {
            seed: other.seed + k,
            ..other
        };
        MissionLayer::lay(&x.layout, &t, &x.rule, inputs).is_ok_and(|o| o != x.layer)
    });
    assert!(differ);
}
