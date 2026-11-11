//! Every venue type laid out over many seeds, in its own frame and in the city's
//! buildings, through the checker; what every builtin type offers and every objective
//! holds; and one site per type pinned.

use citygen::venue::{
    MissionInputs, MissionLayer, NodeAt, PieceKind, VenueFrame, VenueGraph, VenueLayout, VenuePack,
    VenueType, lay_out, validate_copy, validate_venue, venue_fingerprint,
};
use citygen::{BuildingId, City, FloorPlan, GenParams, Generator, LinkKind, Pos, SizePreset};
use std::collections::BTreeSet;

const SEEDS: u64 = 50;

fn types() -> Vec<VenueType> {
    let mut v = VenuePack::builtin().types.clone();
    v.push(VenueType::parse(include_str!("../src/interior/venue/test_type.ron")).unwrap());
    v
}

/// A city of `districts` districts from `seed` at `size`.
fn city(seed: u64, size: SizePreset, districts: u8) -> City {
    Generator::run(GenParams {
        seed,
        size,
        districts,
        ..Default::default()
    })
    .unwrap()
}

/// The cities the sweep lays buildings from: a small one, and two medium ones.
fn cities() -> [City; 3] {
    [
        city(6, SizePreset::Custom { w: 120, h: 60 }, 8),
        city(8, SizePreset::Medium, 7),
        city(11, SizePreset::Medium, 7),
    ]
}

/// The `n` closed buildings of `c` with the largest footprint (by area, then id).
fn largest_closed(c: &City, n: usize) -> Vec<BuildingId> {
    let mut v: Vec<(i32, BuildingId)> = (c.buildings.iter())
        .filter(|b| b.kind.is_closed())
        .map(|b| (b.footprint.w * b.footprint.h, b.id))
        .collect();
    v.sort_by_key(|&(area, id)| (std::cmp::Reverse(area), id));
    v.into_iter().take(n).map(|(_, id)| id).collect()
}

/// A mission's copy of `l` at every size it holds, from mission seed `seed` at security
/// `seed * 37 % 256`: for the objective the seed picks, then each other. An objective
/// that doesn't lay (one too shallow for the size, say) is passed by, but one at least
/// lays; every copy laid passes the copy's checks.
fn copies(t: &VenueType, l: &VenueLayout, seed: u64, what: &str) {
    let pack = VenuePack::builtin();
    let n = t.objectives.len();
    for rule in pack.sizes.iter().filter(|rule| l.holds(t, rule)) {
        let size = rule.size;
        let mut laid = 0;
        let mut errs = Vec::new();
        for k in 0..n {
            let inputs = MissionInputs {
                size,
                objective: (seed as usize + k) % n,
                security: (seed * 37 % 256) as u8,
                seed,
            };
            match MissionLayer::lay(l, t, rule, inputs) {
                Ok(layer) => {
                    validate_copy(l, &layer, rule)
                        .unwrap_or_else(|e| panic!("{} {seed} {:?}: {e}", t.id, size));
                    laid += 1;
                }
                Err(e) => errs.push(e),
            }
        }
        assert!(
            laid > 0,
            "{} {what} {seed} {size:?}: no objective lays: {errs:?}",
            t.id
        );
    }
}

/// Every cut between two bands holds its band's `doors`: a door on a cut stands on the
/// back band's first row, beside the band in front, and a gate line's row of doors is
/// one.
fn cuts_keep_their_doors(t: &VenueType, l: &VenueLayout, what: &str) {
    for x in &l.levels {
        let def = t.role(&x.role).unwrap();
        for (i, band) in def.bands.iter().enumerate().skip(1) {
            let (front, back) = (x.bands[i - 1].rect, x.bands[i].rect);
            let on: Vec<Pos> = (x.doors.iter().map(|d| d.at))
                .filter(|p| back.contains(*p) && p.neighbors4().any(|q| front.contains(q)))
                .collect();
            let gate = |p: &Pos| {
                (x.pieces.iter()).any(|g| g.kind == PieceKind::GateLine && g.rect.contains(*p))
            };
            let doors = on.iter().filter(|p| !gate(p)).count() + usize::from(on.iter().any(gate));
            assert_eq!(
                doors,
                usize::from(band.doors.max(1)),
                "{} {what}: {} cut {i}, doors at {on:?}",
                t.id,
                FloorPlan::label(x.level)
            );
        }
    }
}

#[test]
fn every_type_passes_the_checker() {
    let pack = VenuePack::builtin();
    let cities = cities();
    for t in types() {
        for seed in 0..SEEDS {
            let l = lay_out(&t, VenueFrame::own(&t), seed)
                .unwrap_or_else(|e| panic!("{} own {seed}: {e}", t.id));
            validate_venue(&l).unwrap_or_else(|e| panic!("{} own {seed}: {e}", t.id));
            cuts_keep_their_doors(&t, &l, &format!("own {seed}"));
            for rule in &pack.sizes {
                assert!(l.holds(&t, rule), "{} own {seed} {:?}", t.id, rule.size);
            }
            copies(&t, &l, seed, "own");
        }
        // The buildings of its kinds; a type no kind builds stands in the
        // five largest closed buildings of each city, raised to its own top here so every size is held
        // (the console raises only when a building can't hold the size).
        let frames: Vec<VenueFrame> = if t.builds.is_empty() {
            (cities.iter())
                .flat_map(|c| {
                    (largest_closed(c, 5).into_iter())
                        .filter_map(|b| VenueFrame::of_building(c, b))
                        .map(|f| {
                            let top = t.frame.top.max(f.storeys.1);
                            f.raised(top)
                        })
                })
                .collect()
        } else {
            (cities.iter())
                .flat_map(|c| {
                    (c.buildings.iter())
                        .filter(|b| t.builds.contains(&b.kind))
                        .filter_map(|b| VenueFrame::of_building(c, b.id))
                })
                .collect()
        };
        for seed in 0..if frames.is_empty() { 0 } else { SEEDS } {
            let f = frames[seed as usize % frames.len()].clone();
            let l =
                lay_out(&t, f, seed).unwrap_or_else(|e| panic!("{} building {seed}: {e}", t.id));
            validate_venue(&l).unwrap_or_else(|e| panic!("{} building {seed}: {e}", t.id));
            cuts_keep_their_doors(&t, &l, &format!("building {seed}"));
            copies(&t, &l, seed, "building");
        }
    }
}

/// Every objective of every type holds some size on the type's own frame, at every seed:
/// a mission for it lays and its copy passes the checks.
#[test]
fn every_objective_holds_a_size() {
    let pack = VenuePack::builtin();
    for t in types() {
        for seed in 0..SEEDS {
            let l = lay_out(&t, VenueFrame::own(&t), seed).unwrap();
            for (k, o) in t.objectives.iter().enumerate() {
                let holds = pack.sizes.iter().any(|rule| {
                    let inputs = MissionInputs {
                        size: rule.size,
                        objective: k,
                        security: u8::MAX,
                        seed,
                    };
                    MissionLayer::lay(&l, &t, rule, inputs)
                        .is_ok_and(|layer| validate_copy(&l, &layer, rule).is_ok())
                });
                assert!(holds, "{} seed {seed}: {} holds no size", t.id, o.id);
            }
        }
    }
}

/// Getting out is free: no room of any venue in the test cities, laid out as the city lays
/// it, leaves you with no way out to the city but a key (a crack lapses; a perma-death
/// run can't wait).
#[test]
fn nobody_is_shut_in_anywhere_in_the_test_cities() {
    let mut venues = 0;
    for c in cities() {
        for b in &c.buildings {
            let Some(plan) = FloorPlan::new(&c, b.id) else {
                continue;
            };
            let Some(l) = plan.venue.as_deref() else {
                continue;
            };
            venues += 1;
            let g = VenueGraph::of(l, &[], None);
            let shut: Vec<String> = (g.shut_in().iter())
                .map(|&n| match g.nodes[n] {
                    NodeAt::Room { level, room } => {
                        let r = &l.level(level).unwrap().rooms[room];
                        format!("{} {} at {:?}", FloorPlan::label(level), r.name, r.rect)
                    }
                    at => format!("{at:?}"),
                })
                .collect();
            assert!(
                shut.is_empty(),
                "{} {:?} in city {}: {} rooms shut in: {shut:?}",
                l.type_id,
                b.id,
                c.seed,
                shut.len()
            );
        }
    }
    assert!(venues > 10, "{venues} venues");
}

/// What every builtin type offers, said without naming one.
#[test]
fn every_builtin_type_is_a_whole_site() {
    let pack = VenuePack::builtin();
    for t in &pack.types {
        assert!(t.zones.len() >= 4, "{}", t.id);
        assert!(t.objectives.len() >= 3, "{}", t.id);
        let kinds: BTreeSet<_> = t.links.iter().map(|l| l.kind).collect();
        assert!(
            kinds.len() >= 3,
            "{}: lifts, stairs and a crawl at least",
            t.id
        );
        assert!(
            t.links.iter().any(|l| l.kind == LinkKind::Crawl) || !t.crawls_in.is_empty(),
            "{}",
            t.id
        );
        let own = lay_out(t, VenueFrame::own(t), 0).unwrap();
        assert!(own.levels.len() >= 6, "{}", t.id);
        if t.builds.is_empty() {
            // A type no building kind builds stands below the street.
            assert!(
                own.levels.iter().filter(|l| l.level < 0).count() >= 5,
                "{}",
                t.id
            );
        }
    }
}

/// One site per type, so all three platforms agree. Update a number only for a change
/// meant to move that type's sites; a new type in sites.ron adds its number here.
#[test]
fn venue_fingerprints_are_pinned() {
    const PINNED: &[u64] = &[
        10811627059381793165,
        13561429448275412134,
        3395241277663048963,
    ];
    let pack = VenuePack::builtin();
    assert_eq!(
        PINNED.len(),
        pack.types.len(),
        "one fingerprint per type, in sites.ron's order"
    );
    for (t, want) in pack.types.iter().zip(PINNED) {
        let l = lay_out(t, VenueFrame::own(t), 0x5A17).unwrap();
        assert_eq!(venue_fingerprint(&l), *want, "{}", t.id);
    }
}
