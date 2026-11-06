//! The living city: people with professions, homes, workplaces, routines and cars,
//! stepped in game minutes.

pub mod commute;
pub mod content;
pub mod paths;
pub mod people;
pub mod places;
pub mod world;

pub use content::*;
pub use paths::*;
pub use people::*;
pub use places::*;
pub use world::*;
