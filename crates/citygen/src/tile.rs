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
    // Open ground one level down from the overworld. Appended, so saves keep the
    // indices of everything above.
    /// Bare earth.
    Dirt,
    /// Earth with scrub.
    Scrub,
    /// Rows of crops in a field.
    Crops,
    /// A salt pan's crust.
    Salt,
    /// The toxic spill's seep.
    Spill,
    /// A road's shoulder.
    Gravel,
    /// A beach.
    Sand,
    /// Reeds and mud at a lake's or river's edge.
    Reeds,
    /// A rock outcrop: you see past it, you do not walk through it.
    Rock,
    /// A mountain's rock wall.
    Cliff,
    /// Water at the edge, too deep to wade but lighter than the deep.
    Shallows,
    /// A river's shallow crossing, walkable.
    Ford,
    /// A low rise in the ground.
    Rise,
    /// A dry wash's bed.
    Wash,
}

pub const ALL_TERRAIN: [Terrain; 42] = [
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
    Terrain::Dirt,
    Terrain::Scrub,
    Terrain::Crops,
    Terrain::Salt,
    Terrain::Spill,
    Terrain::Gravel,
    Terrain::Sand,
    Terrain::Reeds,
    Terrain::Rock,
    Terrain::Cliff,
    Terrain::Shallows,
    Terrain::Ford,
    Terrain::Rise,
    Terrain::Wash,
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
            Dirt => "dirt",
            Scrub => "scrub",
            Crops => "crops",
            Salt => "salt",
            Spill => "spill",
            Gravel => "gravel",
            Sand => "sand",
            Reeds => "reeds",
            Rock => "rock",
            Cliff => "cliff",
            Shallows => "shallows",
            Ford => "ford",
            Rise => "rise",
            Wash => "dry wash",
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
    /// A Net node standing on this cyberspace tile.
    Node(NodeKind),
    /// A piece of furniture on an interior floor; nobody walks through it.
    Furniture(Furniture),
    /// A piece of ICE standing in a construct; nobody walks through it while it stands.
    Ice(IceGrade),
    /// A shopfront awning over the pavement; walkable.
    Awning,
    /// A light fitting indoors. Appended, so old saves keep their indices.
    Fitting(Fitting),
    /// A power line's pylon out in the open; nobody walks through it.
    Pylon,
    /// A power line overhead between pylons; `true` where it runs north-south.
    PowerLine(bool),
    /// A fire burning in an oil drum, out in a city's outskirts; nobody walks through it.
    FireBarrel,
}

/// A light fitting on an interior floor: hung from the ceiling (walked under), set in a
/// wall, or a stage's lights.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fitting {
    pub kind: FittingKind,
    pub tone: Tone,
    pub wear: Wear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FittingKind {
    Tube,
    Panel,
    Bulb,
    HighBay,
    Strip,
    Sconce,
    NeonStrip,
    Stage,
}

impl FittingKind {
    /// Set in a wall tile rather than hung over a floor tile.
    pub fn on_wall(self) -> bool {
        matches!(
            self,
            FittingKind::Strip | FittingKind::Sconce | FittingKind::NeonStrip
        )
    }

    /// The kinds that go dead or stutter with age.
    pub fn wears(self) -> bool {
        matches!(
            self,
            FittingKind::Tube | FittingKind::HighBay | FittingKind::Strip
        )
    }
}

/// What lights a room, and so its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tone {
    Fluorescent,
    Led,
    Tungsten,
    Surgical,
    Sodium,
    Neon(Hue),
    Blacklight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Wear {
    Ok,
    Dead,
    Stutter,
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
    /// A ripperdoc's operating chair. Appended, so old saves keep their indices.
    Chair,
    /// The flat's holo set, which carries the morning news. After `Chair`, so old saves
    /// keep their indices.
    Holo,
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
            Furniture::Chair => "operating chair",
            Furniture::Holo => "holo set",
        }
    }
}

pub const ALL_FEATURES: [Feature; 25] = [
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
    Feature::Node(NodeKind::Server),
    Feature::Furniture(Furniture::Desk),
    Feature::Ice(IceGrade::White),
    Feature::Awning,
    Feature::Fitting(Fitting {
        kind: FittingKind::Tube,
        tone: Tone::Fluorescent,
        wear: Wear::Ok,
    }),
    Feature::Fitting(Fitting {
        kind: FittingKind::Bulb,
        tone: Tone::Tungsten,
        wear: Wear::Ok,
    }),
    Feature::Pylon,
    Feature::PowerLine(false),
    Feature::PowerLine(true),
    Feature::FireBarrel,
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
                | Dirt
                | Scrub
                | Crops
                | Salt
                | Spill
                | Gravel
                | Sand
                | Reeds
                | Ford
                | Rise
                | Wash
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
                    | Feature::Pylon
                    | Feature::FireBarrel
            )
        );
        terrain_ok && feature_ok
    }

    pub fn blocks_sight(&self) -> bool {
        matches!(
            self.terrain,
            Terrain::Wall | Terrain::Door | Terrain::Mass | Terrain::NetWall | Terrain::Cliff
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
    #[test]
    fn open_ground_walks_and_sees_as_it_should() {
        use Terrain::*;
        for t in [
            Dirt, Scrub, Crops, Salt, Spill, Gravel, Sand, Reeds, Ford, Rise, Wash,
        ] {
            assert!(Tile::new(t).is_walkable(), "{t:?}");
            assert!(!Tile::new(t).blocks_sight(), "{t:?}");
        }
        for t in [Rock, Cliff, Shallows] {
            assert!(!Tile::new(t).is_walkable(), "{t:?}");
        }
        assert!(Tile::new(Cliff).blocks_sight());
        assert!(!Tile::new(Rock).blocks_sight());
        let mut pylon = Tile::new(Dirt);
        pylon.feature = Some(Feature::Pylon);
        assert!(!pylon.is_walkable() && !pylon.blocks_sight());
        let mut line = Tile::new(Dirt);
        line.feature = Some(Feature::PowerLine(true));
        assert!(line.is_walkable() && !line.blocks_sight());
        assert_eq!(ALL_TERRAIN.len(), 42);
        assert_eq!(*ALL_TERRAIN.last().unwrap(), Wash);
        assert_eq!(*ALL_FEATURES.last().unwrap(), Feature::FireBarrel);
        let mut fire = Tile::new(Dirt);
        fire.feature = Some(Feature::FireBarrel);
        assert!(!fire.is_walkable() && !fire.blocks_sight());
    }

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
        assert_eq!(ALL_TERRAIN.len(), 42);
        assert_eq!(ALL_FEATURES.len(), 25);
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
        assert_eq!(ALL_TERRAIN.len(), 42);
        assert_eq!(
            ALL_TERRAIN[27],
            Terrain::Railing,
            "appended, so saves keep indices"
        );
        assert_eq!(Furniture::Couch.label(), "couch");
        assert_eq!(Furniture::Shower.label(), "shower");
    }

    #[test]
    fn street_pieces_walk_and_block_as_intended() {
        let mut t = Tile::new(Terrain::Plaza);
        t.feature = Some(Feature::Awning);
        assert!(t.is_walkable());
        for f in [Furniture::Bench, Furniture::Vending, Furniture::Bike] {
            t.feature = Some(Feature::Furniture(f));
            assert!(!t.is_walkable());
        }
        assert_eq!(Furniture::Vending.label(), "vending machine");
        assert!(ALL_FEATURES.contains(&Feature::Awning));
    }

    #[test]
    fn feature_save_encoding_keeps_its_old_indices() {
        // Postcard writes the variant index first; saves written before the
        // street pieces existed depend on these staying put.
        let lead = |f: Feature| postcard::to_stdvec(&f).unwrap()[0];
        assert_eq!(lead(Feature::Node(NodeKind::Server)), 15);
        assert_eq!(lead(Feature::Furniture(Furniture::Desk)), 16);
        assert_eq!(lead(Feature::Ice(IceGrade::White)), 17);
        assert_eq!(lead(Feature::Awning), 18);
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
        assert_eq!(ALL_TERRAIN.len(), 42);
        assert_eq!(ALL_FEATURES.len(), 25);
        assert_eq!(crate::world::StratumKind::ALL.len(), 5);
        assert_eq!(
            crate::world::StratumKind::ALL[4],
            crate::world::StratumKind::Net
        );
    }

    #[test]
    fn fittings_are_walked_under_or_sit_in_walls() {
        let f = |kind| {
            Feature::Fitting(Fitting {
                kind,
                tone: Tone::Fluorescent,
                wear: Wear::Ok,
            })
        };
        let mut t = Tile::new(Terrain::Floor);
        for k in [
            FittingKind::Tube,
            FittingKind::Panel,
            FittingKind::Bulb,
            FittingKind::HighBay,
            FittingKind::Stage,
        ] {
            t.feature = Some(f(k));
            assert!(t.is_walkable(), "{k:?}");
            assert!(!k.on_wall());
        }
        for k in [
            FittingKind::Strip,
            FittingKind::Sconce,
            FittingKind::NeonStrip,
        ] {
            assert!(k.on_wall(), "{k:?}");
        }
        assert!(FittingKind::Tube.wears() && !FittingKind::Panel.wears());
    }
}
