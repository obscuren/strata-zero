//! Venues: buildings laid out from a type in `flavor/sites.ron` (a hotel, an office, a
//! metro station), with zones, the locks between them and the links between levels.
//! The Rust never names a type: a new type is a change to the data.

mod types;

pub use types::*;

#[cfg(test)]
mod testutil;
