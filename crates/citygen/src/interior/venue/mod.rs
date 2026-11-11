//! Venues: buildings laid out from a type in `flavor/sites.ron`, one for each of some kinds
//! of building, with zones, the locks between them and the links between levels.
//! The Rust never names a type: a new type is a change to the data.

mod check;
mod devices;
mod draw;
mod frame;
mod graph;
mod layout;
mod levels;
mod links;
mod mission;
mod rooms;
mod types;
mod ways;
mod zones;

pub use check::*;
pub use devices::*;
pub use draw::{draw_copy_level, draw_level};
pub use frame::*;
pub use graph::*;
pub use layout::*;
pub use mission::{
    CardSpot, CodeSpot, Device, DeviceKind, GuardPost, MissionInputs, MissionLayer, Placed,
};
pub use types::*;

#[cfg(test)]
pub(crate) mod testutil;
