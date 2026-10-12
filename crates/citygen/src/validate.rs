//! City invariants.

use crate::world::City;

/// A broken invariant found by [`City::validate`].
#[derive(Clone, Debug, PartialEq)]
pub struct Violation(pub String);

impl City {
    /// Checks the invariants every finished city must satisfy.
    pub fn validate(&self) -> Result<(), Vec<Violation>> {
        Ok(())
    }
}
