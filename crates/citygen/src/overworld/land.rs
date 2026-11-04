//! Stages 1 and 2: the sea and its coast, ridges and elevation.

use super::Ctx;
use crate::geom::{Dir, Pos};
use crate::params::GenError;

/// A bay cut into the coast: its side, where along it, how deep and wide (s-units), and
/// its innermost sea tile.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct Bay {
    pub side: Dir,
    pub t: f32,
    pub depth: f32,
    pub width: f32,
    pub tip: Pos,
}

pub(crate) fn run(_ctx: &mut Ctx) -> Result<(), GenError> {
    Ok(())
}
