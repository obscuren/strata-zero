//! What every venue test module shares.

use super::{PlannedRoom, VenueFrame, VenueLayout, VenueType, levels, links, rooms, ways, zones};
use crate::rng::hash64;
use crate::world::City;
use crate::{GenParams, Generator, SizePreset};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// The test-only type, which exercises every piece of the schema.
pub(crate) fn test_type() -> VenueType {
    VenueType::parse(include_str!("test_type.ron")).unwrap()
}

/// `t` over its own frame from `seed`, laid as `lay_out` lays it, but with every room on
/// `level` that `shut` picks made private before the links, so no landing stands in it.
pub(crate) fn crowded(
    t: &VenueType,
    seed: u64,
    level: i32,
    shut: impl Fn(&PlannedRoom) -> bool,
) -> Result<VenueLayout, String> {
    let rng = |stage: u64| ChaCha8Rng::seed_from_u64(hash64(seed, stage));
    let mut l = VenueLayout {
        type_id: t.id.clone(),
        type_name: t.name.clone(),
        seed,
        frame: VenueFrame::own(t),
        zones: t.zones.clone(),
        levels: Vec::new(),
        links: Vec::new(),
        ways_in: Vec::new(),
        barred: Vec::new(),
    };
    levels::place(t, &mut l, &mut rng(levels::STAGE))?;
    zones::lay(t, &mut l, &mut rng(zones::STAGE))?;
    rooms::lay(t, &mut l, &mut rng(rooms::STAGE))?;
    let x = l.levels.iter_mut().find(|x| x.level == level).unwrap();
    for r in x.rooms.iter_mut().filter(|r| shut(r)) {
        r.private = true;
    }
    links::lay(t, &mut l, &mut rng(links::STAGE))?;
    ways::lay(t, &mut l, &mut rng(ways::STAGE))?;
    Ok(l)
}

/// A small city: seed 6, 120 x 60, eight districts.
pub(crate) fn city() -> City {
    Generator::run(GenParams {
        seed: 6,
        size: SizePreset::Custom { w: 120, h: 60 },
        districts: 8,
        ..Default::default()
    })
    .unwrap()
}

/// A medium city of seven districts.
pub(crate) fn medium(seed: u64) -> City {
    Generator::run(GenParams {
        seed,
        size: SizePreset::Medium,
        districts: 7,
        ..Default::default()
    })
    .unwrap()
}

/// The cities venue tests lay buildings from: `city()`, `medium(8)` and `medium(11)`.
pub(crate) fn cities() -> Vec<City> {
    vec![city(), medium(8), medium(11)]
}
