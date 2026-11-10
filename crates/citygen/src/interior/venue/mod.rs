//! Venues: buildings laid out from a type in `flavor/sites.ron` (a hotel, an office, a
//! metro station), with zones, the locks between them and the links between levels.
//! The Rust never names a type: a new type is a change to the data.

mod draw;
mod frame;
mod layout;
mod levels;
mod links;
mod rooms;
mod types;
mod ways;
mod zones;

pub use draw::draw_level;
pub use frame::*;
pub use layout::*;
pub use types::*;

#[cfg(test)]
mod testutil;
