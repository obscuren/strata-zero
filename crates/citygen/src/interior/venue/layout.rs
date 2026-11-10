//! A venue's layout: its levels, laid out stage by stage from a type, a frame and a seed.

use super::{Spine, VenueFrame, VenueType, levels};
use crate::geom::{Pos, Size};
use crate::rng::hash64;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VenueLayout {
    pub type_id: String,
    pub type_name: String,
    pub seed: u64,
    pub frame: VenueFrame,
    /// The type's zone names by rank.
    pub zones: Vec<String>,
    /// Lowest first.
    pub levels: Vec<LevelLayout>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelLayout {
    pub level: i32,
    /// The type's role name: "guest floor".
    pub role: String,
    pub size: Size,
    /// How its corridor runs, as its role gives it.
    pub spine: Spine,
    /// Tiles that take no wall, furniture or spot: doorsteps, landings and beside them.
    pub reserved: Vec<Pos>,
}

/// Lays out a venue of type `t` in `frame` from `seed`, a stage at a time: each stage
/// draws from its own rng, seeded from `seed` and the stage.
pub fn lay_out(t: &VenueType, frame: VenueFrame, seed: u64) -> Result<VenueLayout, String> {
    let rng = |stage: u64| ChaCha8Rng::seed_from_u64(hash64(seed, stage));
    let mut layout = VenueLayout {
        type_id: t.id.clone(),
        type_name: t.name.clone(),
        seed,
        frame,
        zones: t.zones.clone(),
        levels: Vec::new(),
    };
    levels::place(t, &mut layout, &mut rng(levels::STAGE))?;
    Ok(layout)
}

impl VenueLayout {
    pub fn level(&self, level: i32) -> Option<&LevelLayout> {
        self.levels.iter().find(|l| l.level == level)
    }
}
