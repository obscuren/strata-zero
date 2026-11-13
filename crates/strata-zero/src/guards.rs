//! Guards in a mission's copy of a venue: what only they do beside every other foe.

mod doors;

pub use doors::opens;

#[cfg(test)]
pub(crate) mod testutil;
