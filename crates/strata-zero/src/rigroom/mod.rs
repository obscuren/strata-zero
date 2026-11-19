//! Your rig's own Net, built up a task at a time.

pub mod layout;
pub mod reader;
#[allow(unused_imports)] // the next tasks use them
pub use layout::{Counts, RigRoom, lay_out};
