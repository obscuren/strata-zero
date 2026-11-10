//! A venue's layout: its levels, laid out stage by stage from a type, a frame and a seed.

use super::{Spine, VenueFrame, VenueType, levels, zones};
use crate::geom::{Pos, Rect, Size};
use crate::interior::{DoorLock, Role, RoomKind};
use crate::rng::hash64;
use crate::tile::Furniture;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

/// The first number a level's locks take, clear of every room and flat number.
pub const LOCK_NUMBERS: u16 = 60_000;

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
    /// How its corridor runs: its role's, with `Any` picked by the level's seed.
    pub spine: Spine,
    /// The level cut front to back, one strip per band of its role, in the role's order.
    pub bands: Vec<BandArea>,
    /// The spine rooms first, one per band in the bands' order: the band's piece of the
    /// corridor, or on a `Hall` spine the whole band, which the rooms stage gives its
    /// first listed room.
    pub rooms: Vec<PlannedRoom>,
    pub doors: Vec<PlannedDoor>,
    /// The solid block a `Ring` spine runs round, inside its band's spine room: no room,
    /// door, spot or landing uses it, and drawing walls it.
    pub core: Option<Rect>,
    /// Tiles that take no wall, furniture or spot: doorsteps, landings and beside them.
    pub reserved: Vec<Pos>,
}

/// A band's strip of a level. A band behind another holds the wall between them as its
/// first row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BandArea {
    pub zone: u8,
    pub rect: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedDoor {
    pub at: Pos,
    /// The two rooms it joins, by index in the level's rooms.
    pub rooms: (usize, usize),
    /// Locked in the plain building: the type's `always` borders.
    pub lock: Option<DoorLock>,
    /// The lock a mission can fit here (a candidate door), None for a door it can't.
    pub fit: Option<DoorLock>,
    /// Joins two zones.
    pub border: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedRoom {
    pub rect: Rect,
    pub kind: RoomKind,
    pub name: String,
    pub zone: u8,
    pub number: u16,
    pub private: bool,
    /// From the room's def.
    pub furniture: Vec<Furniture>,
    pub stations: Vec<Role>,
    /// Interior tiles the mission layer may use.
    pub spots: Vec<Pos>,
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
    zones::lay(t, &mut layout, &mut rng(zones::STAGE))?;
    Ok(layout)
}

impl VenueLayout {
    pub fn level(&self, level: i32) -> Option<&LevelLayout> {
        self.levels.iter().find(|l| l.level == level)
    }
}
