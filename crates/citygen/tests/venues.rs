//! Every venue type laid out over many seeds, in its own frame and in the city's
//! buildings, through the checker; and one site per type pinned.

use citygen::venue::{
    VenueFrame, VenuePack, VenueType, lay_out, validate_venue, venue_fingerprint,
};
use citygen::{BuildingId, City, GenParams, Generator, SizePreset};

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

#[test]
fn every_type_passes_the_checker() {
    let pack = VenuePack::builtin();
    let cities = cities();
    for t in types() {
        for seed in 0..SEEDS {
            let l = lay_out(&t, VenueFrame::own(&t), seed)
                .unwrap_or_else(|e| panic!("{} own {seed}: {e}", t.id));
            validate_venue(&l).unwrap_or_else(|e| panic!("{} own {seed}: {e}", t.id));
            for rule in &pack.sizes {
                assert!(l.holds(&t, rule), "{} own {seed} {:?}", t.id, rule.size);
            }
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
        }
    }
}

/// One site per type, so all three platforms agree. Update a number only for a change
/// meant to move that type's sites; a new type in sites.ron adds its number here.
#[test]
fn venue_fingerprints_are_pinned() {
    const PINNED: &[u64] = &[
        12979167704675157047,
        12500839520295178152,
        6713321246441217806,
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
