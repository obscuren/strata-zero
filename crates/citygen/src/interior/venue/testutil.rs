//! What every venue test module shares.

use super::VenueType;

/// The test-only type, which exercises every piece of the schema.
pub(crate) fn test_type() -> VenueType {
    VenueType::parse(include_str!("test_type.ron")).unwrap()
}
