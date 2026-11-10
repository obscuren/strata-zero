//! What every venue test module shares.

use super::VenueType;
use crate::world::City;
use crate::{GenParams, Generator, SizePreset};

/// The test-only type, which exercises every piece of the schema.
pub(crate) fn test_type() -> VenueType {
    VenueType::parse(include_str!("test_type.ron")).unwrap()
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
