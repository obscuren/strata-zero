//! Venues: buildings laid out from a type in `flavor/sites.ron` (a hotel, an office, a
//! metro station), with zones, the locks between them and the links between levels.
//! The Rust never names a type: a new type is a change to the data.

mod check;
mod draw;
mod frame;
mod graph;
mod layout;
mod levels;
mod links;
mod rooms;
mod types;
mod ways;
mod zones;

pub use check::*;
pub use draw::draw_level;
pub use frame::*;
pub use graph::*;
pub use layout::*;
pub use types::*;

#[cfg(test)]
mod testutil;
