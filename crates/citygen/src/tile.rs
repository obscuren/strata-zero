//! Tiles: terrain, features and derived walkability.

use crate::ids::{BuildingId, DistrictId};
use crate::net::NodeKind;
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
    /// The solid bulk of a closed building behind its facade.
    Mass,
    Rooftop,
    Walkway,
    Bridge,
    Tunnel,
    Rail,
    Stair,
    Lift,
    Ladder,
    Grate,
    NetGrid,
    NetLane,
    NetWall,
    NetFloor,
    NetGate,
    /// The open drop of a megablock atrium; the overlay draws it.
    Atrium,
    /// The edge of an atrium floor: you see past it, you do not walk through it.
    Railing,
}

pub const ALL_TERRAIN: [Terrain; 28] = [
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
    Terrain::Mass,
    Terrain::Rooftop,
    Terrain::Walkway,
    Terrain::Bridge,
    Terrain::Tunnel,
    Terrain::Rail,
    Terrain::Stair,
    Terrain::Lift,
    Terrain::Ladder,
    Terrain::Grate,
    Terrain::NetGrid,
    Terrain::NetLane,
    Terrain::NetWall,
    Terrain::NetFloor,
    Terrain::NetGate,
    Terrain::Atrium,
    Terrain::Railing,
];

impl Terrain {
    /// Human label for the inspector and legend.
    pub fn label(self) -> &'static str {
        use Terrain::*;
        match self {
            Void => "void",
            Water => "water",
            Sewage => "sewage",
            Road => "road",
            Sidewalk => "sidewalk",
            Alley => "alley",
            Plaza => "plaza",
            Floor => "floor",
            Wall => "wall",
            Door => "door",
            Window => "window",
            Mass => "building",
            Rooftop => "rooftop",
            Walkway => "walkway",
            Bridge => "bridge",
            Tunnel => "tunnel",
            Rail => "rail",
            Stair => "stair",
            Lift => "lift",
            Ladder => "ladder",
            Grate => "grate",
            NetGrid => "grid",
            NetLane => "lane",
            NetWall => "construct wall",
            NetFloor => "construct floor",
            NetGate => "gate",
            Atrium => "atrium",
            Railing => "railing",
        }
    }

    /// Cyberspace-only terrain.
    pub fn is_net(self) -> bool {
        use Terrain::*;
        matches!(self, NetGrid | NetLane | NetWall | NetFloor | NetGate)
    }
}

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
    /// A shopfront awning over the pavement; walkable, see-through.
    Awning,
    /// A Net node standing on this cyberspace tile.
    Node(NodeKind),
    /// A piece of furniture on an interior floor; nobody walks through it.
    Furniture(Furniture),
    /// A piece of ICE standing in a construct; nobody walks through it while it stands.
    Ice(IceGrade),
}

/// How hard a piece of ICE bites: white traces and alarms, grey dumps you.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IceGrade {
    White,
    Grey,
    Black,
}

impl IceGrade {
    pub fn label(self) -> &'static str {
        match self {
            IceGrade::White => "white",
            IceGrade::Grey => "grey",
            IceGrade::Black => "black",
        }
    }

    /// Hits it takes to break.
    pub fn integrity(self) -> u8 {
        match self {
            IceGrade::White => 3,
            IceGrade::Grey => 6,
            IceGrade::Black => 10,
        }
    }

    /// How far along the spine it sees, in tiles.
    pub fn sight(self) -> i32 {
        match self {
            IceGrade::White => 4,
            IceGrade::Grey => 6,
            IceGrade::Black => 8,
        }
    }
}

/// What stands in a room. A station is the floor tile beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Furniture {
    Desk,
    Counter,
    Table,
    Stool,
    Shelf,
    Machine,
    Bed,
    Shrine,
    Locker,
    Couch,
    Shower,
    Bench,
    Vending,
    Bike,
}

impl Furniture {
    pub fn label(self) -> &'static str {
        match self {
            Furniture::Desk => "desk",
            Furniture::Counter => "counter",
            Furniture::Table => "table",
            Furniture::Stool => "stool",
            Furniture::Shelf => "shelving",
            Furniture::Machine => "machine",
            Furniture::Bed => "bed",
            Furniture::Shrine => "shrine",
            Furniture::Locker => "lockers",
            Furniture::Couch => "couch",
            Furniture::Shower => "shower",
            Furniture::Bench => "bench",
            Furniture::Vending => "vending machine",
            Furniture::Bike => "bike",
        }
    }
}

pub const ALL_FEATURES: [Feature; 19] = [
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
    Feature::Awning,
    Feature::Node(NodeKind::Server),
    Feature::Furniture(Furniture::Desk),
    Feature::Ice(IceGrade::White),
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
                | NetGrid
                | NetLane
                | NetFloor
                | NetGate
        );
        let feature_ok = !matches!(
            self.feature,
            Some(
                Feature::Barrier
                    | Feature::Stall
                    | Feature::AcUnit
                    | Feature::Pipe
                    | Feature::Terminal
                    | Feature::Furniture(_)
                    | Feature::Ice(_)
            )
        );
        terrain_ok && feature_ok
    }

    pub fn blocks_sight(&self) -> bool {
        matches!(
            self.terrain,
            Terrain::Wall | Terrain::Door | Terrain::Mass | Terrain::NetWall
        )
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
            Terrain::Wall | Terrain::Floor | Terrain::Door | Terrain::Window | Terrain::Mass
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
        assert_eq!(ALL_TERRAIN.len(), 28);
        assert_eq!(ALL_FEATURES.len(), 19);
        assert!(!Tile::new(Terrain::Mass).is_walkable());
        assert!(Tile::new(Terrain::Mass).blocks_sight());
        assert!(Tile::new(Terrain::Mass).is_building_part());
        assert_eq!(Terrain::Mass.label(), "building");
        t.feature = Some(Feature::Furniture(Furniture::Desk));
        assert!(!t.is_walkable(), "nobody walks through a desk");
        assert_eq!(Furniture::Shelf.label(), "shelving");
    }

    #[test]
    fn atrium_and_railing_are_open_to_sight_but_not_to_feet() {
        for t in [Terrain::Atrium, Terrain::Railing] {
            let tile = Tile::new(t);
            assert!(!tile.is_walkable());
            assert!(!tile.blocks_sight());
            assert!(!tile.is_building_part());
        }
        assert_eq!(ALL_TERRAIN.len(), 28);
        assert_eq!(*ALL_TERRAIN.last().unwrap(), Terrain::Railing);
        assert_eq!(Furniture::Couch.label(), "couch");
        assert_eq!(Furniture::Shower.label(), "shower");
    }

    #[test]
    fn street_pieces_walk_and_block_as_intended() {
        let mut t = Tile::new(Terrain::Plaza);
        t.feature = Some(Feature::Awning);
        assert!(t.is_walkable() && !t.blocks_sight());
        for f in [Furniture::Bench, Furniture::Vending, Furniture::Bike] {
            t.feature = Some(Feature::Furniture(f));
            assert!(!t.is_walkable());
        }
        assert_eq!(Furniture::Vending.label(), "vending machine");
        assert!(ALL_FEATURES.contains(&Feature::Awning));
    }

    #[test]
    fn net_tiles_walk_and_block_like_the_spec_says() {
        assert!(Tile::new(Terrain::NetGrid).is_walkable());
        assert!(Tile::new(Terrain::NetLane).is_walkable());
        assert!(Tile::new(Terrain::NetFloor).is_walkable());
        assert!(Tile::new(Terrain::NetGate).is_walkable());
        assert!(!Tile::new(Terrain::NetWall).is_walkable());
        assert!(Tile::new(Terrain::NetWall).blocks_sight());
        assert!(!Tile::new(Terrain::NetGate).blocks_sight());
        let mut t = Tile::new(Terrain::NetGrid);
        t.feature = Some(Feature::Node(crate::net::NodeKind::Ice));
        assert!(t.is_walkable(), "nodes never block walking in this spec");
        assert_eq!(Terrain::NetWall.label(), "construct wall");
        assert_eq!(Terrain::Road.label(), "road");
        assert!(Terrain::NetLane.is_net() && !Terrain::Road.is_net());
        assert_eq!(ALL_TERRAIN.len(), 28);
        assert_eq!(ALL_FEATURES.len(), 19);
        assert_eq!(crate::world::StratumKind::ALL.len(), 5);
        assert_eq!(
            crate::world::StratumKind::ALL[4],
            crate::world::StratumKind::Net
        );
    }
}
