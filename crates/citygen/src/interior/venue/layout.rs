//! A venue's layout: its levels, laid out stage by stage from a type, a frame and a seed.

use super::{PieceKind, Spine, VenueFrame, VenueType, levels, links, rooms, ways, zones};
use crate::geom::{Pos, Rect, Size};
use crate::interior::{DoorLock, Exit, LevelLink, Role, RoomKind};
use crate::rng::hash64;
use crate::tile::Furniture;
use crate::world::StratumKind;
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
    /// The links between its levels, in the type's order, each named from its def.
    pub links: Vec<LevelLink>,
    /// The ways in from the city: an exit for each frame door on a level it has, in the
    /// frame's order, then a grille for each of the type's crawls in that reaches a level
    /// it has, in the type's order.
    pub ways_in: Vec<WayIn>,
    /// The frame's doors on levels it doesn't have, shut: (stratum, city-map door).
    pub barred: Vec<(StratumKind, Pos)>,
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
    /// corridor, or on a `Hall` spine the whole band as its first listed room. A tile
    /// inside the frame wall that no room holds and no door stands on is wall.
    pub rooms: Vec<PlannedRoom>,
    pub doors: Vec<PlannedDoor>,
    pub pieces: Vec<PlacedPiece>,
    /// The solid block a `Ring` spine runs round, inside its band's spine room: no room,
    /// door, spot or landing uses it, and drawing walls it.
    pub core: Option<Rect>,
    /// Tiles that take no wall, furniture or spot: doorsteps, landings, the roof door, crawl
    /// grilles and beside them.
    pub reserved: Vec<Pos>,
    /// Its doors to the city, in the frame's order: each in the frame wall, but the roof
    /// door, which stands on the floor east of a landing, its doorstep.
    pub exits: Vec<Exit>,
}

/// A way in from the city: a door, or a crawl's grille.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WayIn {
    pub level: i32,
    /// The room it comes into, by index in the level's rooms: the one holding a door's
    /// doorstep, or the grille.
    pub room: usize,
    /// The floor tile: an exit's door, or a crawl's grille.
    pub at: Pos,
    pub kind: WayInKind,
    /// The lock on the door itself, at `at`, where it comes into a zone above public
    /// (but the main street door): the type's border into that zone from the next
    /// shallower zone with one, locked in the plain building too, and one-way, opening
    /// from its doorstep without a key. It numbers on from the level's zone locks. None
    /// for a door into public, and for a crawl, whose grille is its own lock.
    pub lock: Option<DoorLock>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WayInKind {
    /// The level's exit through the city-map door `door` onto `stratum`.
    Door { stratum: StratumKind, door: Pos },
    /// A crawl in from the city tile `outside` on `stratum`. Its grille is a lock only a
    /// crawler passes.
    Crawl { stratum: StratumKind, outside: Pos },
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
    /// No door stands here: the two rooms touch, open to each other along their edge (a
    /// vestibule on the spine, a platform's edge), and `at` is a tile of the second.
    pub open: bool,
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

/// A set piece where it stands on its level.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacedPiece {
    pub name: String,
    pub kind: PieceKind,
    /// A track's rails, a gate line's row of doors, a ramp's strip.
    pub rect: Rect,
    /// A track's two end tiles on the frame wall: its shutters.
    pub ends: Vec<Pos>,
}

/// Where the rooms below the street number from: B1's are 9101 on.
const BELOW: u16 = 9000;

/// A room's number: level x 100 + n at or above the street; 9000 + 100 x depth + n below
/// it, so a basement's rooms never take the lobby's numbers (on levels from -9 to 89).
pub fn room_number(level: i32, n: u16) -> u16 {
    if level >= 0 {
        level as u16 * 100 + n
    } else {
        BELOW + level.unsigned_abs() as u16 * 100 + n
    }
}

/// "1204", or "B1-04" below the street.
pub fn room_label(number: u16) -> String {
    if number > BELOW {
        let below = number - BELOW;
        format!("B{}-{:02}", below / 100, below % 100)
    } else {
        number.to_string()
    }
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
        links: Vec::new(),
        ways_in: Vec::new(),
        barred: Vec::new(),
    };
    levels::place(t, &mut layout, &mut rng(levels::STAGE))?;
    zones::lay(t, &mut layout, &mut rng(zones::STAGE))?;
    rooms::lay(t, &mut layout, &mut rng(rooms::STAGE))?;
    links::lay(t, &mut layout, &mut rng(links::STAGE))?;
    ways::lay(t, &mut layout, &mut rng(ways::STAGE))?;
    Ok(layout)
}

impl VenueLayout {
    pub fn level(&self, level: i32) -> Option<&LevelLayout> {
        self.levels.iter().find(|l| l.level == level)
    }

    /// The door of the private room numbered `number` on `level`.
    pub fn room_door(&self, level: i32, number: u16) -> Option<Pos> {
        let x = self.level(level)?;
        let i = x
            .rooms
            .iter()
            .position(|r| r.private && r.number == number)?;
        (x.doors.iter())
            .find(|d| !d.open && (d.rooms.0 == i || d.rooms.1 == i))
            .map(|d| d.at)
    }

    /// The gate line (index into the level's pieces) whose rect holds `p`.
    pub fn gate_line_at(&self, level: i32, p: Pos) -> Option<usize> {
        (self.level(level)?.pieces.iter())
            .position(|g| g.kind == PieceKind::GateLine && g.rect.contains(p))
    }
}
