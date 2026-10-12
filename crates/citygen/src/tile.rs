//! Tiles: terrain, features and derived walkability.

use crate::ids::{BuildingId, DistrictId};
use serde::{Deserialize, Serialize};

/// Base terrain of a tile. Every tile has exactly one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Terrain {
    Void,
    Water,
    Sewage,
    Road,
    Sidewalk,
    Alley,
    Plaza,
    Floor,
    Wall,
    Door,
    Window,
    Rooftop,
    Walkway,
    Bridge,
    Tunnel,
    Rail,
    Stair,
    Lift,
    Ladder,
    Grate,
}

pub const ALL_TERRAIN: [Terrain; 20] = [
    Terrain::Void,
    Terrain::Water,
    Terrain::Sewage,
    Terrain::Road,
    Terrain::Sidewalk,
    Terrain::Alley,
    Terrain::Plaza,
    Terrain::Floor,
    Terrain::Wall,
    Terrain::Door,
    Terrain::Window,
    Terrain::Rooftop,
    Terrain::Walkway,
    Terrain::Bridge,
    Terrain::Tunnel,
    Terrain::Rail,
    Terrain::Stair,
    Terrain::Lift,
    Terrain::Ladder,
    Terrain::Grate,
];

/// Named neon colours; the renderer maps them to RGB.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Hue {
    Cyan,
    Magenta,
    Pink,
    Orange,
    Yellow,
    Green,
    Blue,
    Red,
    White,
}

/// Optional dressing on top of the terrain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Feature {
    Neon(Hue),
    HoloAd,
    Lamp,
    Vent,
    Puddle,
    Trash,
    Barrier,
    Stall,
    Sign(char),
    Terminal,
    Camera,
    AcUnit,
    Helipad,
    Pipe,
    Cable,
}

pub const ALL_FEATURES: [Feature; 15] = [
    Feature::Neon(Hue::Cyan),
    Feature::HoloAd,
    Feature::Lamp,
    Feature::Vent,
    Feature::Puddle,
    Feature::Trash,
    Feature::Barrier,
    Feature::Stall,
    Feature::Sign('ｱ'),
    Feature::Terminal,
    Feature::Camera,
    Feature::AcUnit,
    Feature::Helipad,
    Feature::Pipe,
    Feature::Cable,
];

/// One cell of a stratum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tile {
    pub terrain: Terrain,
    pub feature: Option<Feature>,
    pub district: Option<DistrictId>,
    pub building: Option<BuildingId>,
    /// Deterministic per-tile hash used for glyph variation and animation phase.
    pub variant: u8,
}

impl Tile {
    pub const VOID: Tile = Tile {
        terrain: Terrain::Void,
        feature: None,
        district: None,
        building: None,
        variant: 0,
    };

    pub fn new(terrain: Terrain) -> Self {
        Tile {
            terrain,
            ..Tile::VOID
        }
    }

    pub fn is_walkable(&self) -> bool {
        use Terrain::*;
        let terrain_ok = matches!(
            self.terrain,
            Road | Sidewalk
                | Alley
                | Plaza
                | Floor
                | Door
                | Rooftop
                | Walkway
                | Bridge
                | Tunnel
                | Rail
                | Stair
                | Lift
                | Ladder
                | Grate
        );
        let feature_ok = !matches!(
            self.feature,
            Some(
                Feature::Barrier
                    | Feature::Stall
                    | Feature::AcUnit
                    | Feature::Pipe
                    | Feature::Terminal
            )
        );
        terrain_ok && feature_ok
    }

    pub fn blocks_sight(&self) -> bool {
        matches!(self.terrain, Terrain::Wall | Terrain::Door)
    }

    /// Outdoor ground at street level: roads, sidewalks, alleys, plazas.
    pub fn is_outdoor_ground(&self) -> bool {
        matches!(
            self.terrain,
            Terrain::Road | Terrain::Sidewalk | Terrain::Alley | Terrain::Plaza
        )
    }

    pub fn is_building_part(&self) -> bool {
        matches!(
            self.terrain,
            Terrain::Wall | Terrain::Floor | Terrain::Door | Terrain::Window
        )
    }

    /// A vertical connector tile.
    pub fn is_connector(&self) -> bool {
        matches!(
            self.terrain,
            Terrain::Stair | Terrain::Lift | Terrain::Ladder
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walkability_rules() {
        assert!(Tile::new(Terrain::Sidewalk).is_walkable());
        assert!(!Tile::new(Terrain::Wall).is_walkable());
        assert!(!Tile::new(Terrain::Water).is_walkable());
        let mut t = Tile::new(Terrain::Road);
        t.feature = Some(Feature::Barrier);
        assert!(!t.is_walkable());
        t.feature = Some(Feature::Lamp);
        assert!(t.is_walkable());
        assert!(Tile::new(Terrain::Wall).blocks_sight());
        assert!(Tile::new(Terrain::Door).blocks_sight());
        assert!(!Tile::new(Terrain::Window).blocks_sight());
        assert!(!Tile::new(Terrain::Window).is_walkable());
        assert!(Tile::new(Terrain::Plaza).is_outdoor_ground());
        assert!(Tile::new(Terrain::Door).is_building_part());
        assert_eq!(ALL_TERRAIN.len(), 20);
        assert_eq!(ALL_FEATURES.len(), 15);
    }
}
