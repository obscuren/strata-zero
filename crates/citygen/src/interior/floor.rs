//! One floor of an interior: tiles, light, rooms, exits and the stations people
//! stand at.

use crate::geom::{Grid, Pos, Rect};
use crate::tile::Tile;
use crate::world::StratumKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RoomKind {
    Lobby,
    Corridor,
    OpenPlan,
    Office,
    Flat,
    HotelRoom,
    Suite,
    Bar,
    Stage,
    Vip,
    Toilets,
    BackRoom,
    Waiting,
    Surgery,
    Hall,
    Shrine,
    Laundry,
    Lockers,
    Storage,
    Bays,
    Home,
    Shop,
    Platform,
    Track,
    Plant,
    Server,
    Kitchen,
    CarPark,
}

impl RoomKind {
    pub fn label(self) -> &'static str {
        match self {
            RoomKind::Lobby => "lobby",
            RoomKind::Corridor => "corridor",
            RoomKind::OpenPlan => "open plan",
            RoomKind::Office => "office",
            RoomKind::Flat => "flat",
            RoomKind::HotelRoom => "room",
            RoomKind::Suite => "suite",
            RoomKind::Bar => "bar",
            RoomKind::Stage => "stage",
            RoomKind::Vip => "vip room",
            RoomKind::Toilets => "toilets",
            RoomKind::BackRoom => "back room",
            RoomKind::Waiting => "waiting room",
            RoomKind::Surgery => "surgery",
            RoomKind::Hall => "hall",
            RoomKind::Shrine => "shrine",
            RoomKind::Laundry => "laundry",
            RoomKind::Lockers => "lockers",
            RoomKind::Storage => "storage",
            RoomKind::Bays => "bays",
            RoomKind::Home => "home",
            RoomKind::Shop => "shop",
            RoomKind::Platform => "platform",
            RoomKind::Track => "track",
            RoomKind::Plant => "plant room",
            RoomKind::Server => "server room",
            RoomKind::Kitchen => "kitchen",
            RoomKind::CarPark => "car park",
        }
    }
}

/// Where someone at work or at leisure stands: the floor tile beside the furniture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Role {
    Reception,
    Desk,
    Bar,
    Table,
    Machine,
    Surgery,
    Shrine,
    Shelf,
    Bay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Station {
    pub pos: Pos,
    pub role: Role,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Room {
    pub rect: Rect,
    pub kind: RoomKind,
    /// The door onto the corridor or hall, if the room is walled.
    pub door: Option<Pos>,
    /// Locked: a flat, a hotel room, an office, a back room.
    pub private: bool,
    /// A flat or room number, 0 when the room has none.
    pub number: u16,
}

/// A door tile on the floor that leads outside.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Exit {
    /// The `Door` tile on this floor.
    pub at: Pos,
    /// The stratum it opens onto.
    pub stratum: StratumKind,
    /// The city-map tile the player lands on when leaving.
    pub landing: Pos,
    /// The city-map door tile that enters here.
    pub door: Pos,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Floor {
    pub level: i32,
    pub tiles: Grid<Tile>,
    /// The brightest channel of `glow`: the light as one number.
    pub light: Grid<u8>,
    /// The light's colour on each tile, each channel 0..=255.
    pub glow: Grid<(u8, u8, u8)>,
    pub rooms: Vec<Room>,
    pub exits: Vec<Exit>,
    pub stations: Vec<Station>,
}

impl Floor {
    /// The tile just inside an exit's door.
    pub fn doorstep(&self, e: &Exit) -> Pos {
        let size = self.tiles.size();
        let (w, h) = (size.w as i32, size.h as i32);
        let p = e.at;
        if p.x == 0 {
            p.offset(1, 0)
        } else if p.x == w - 1 {
            p.offset(-1, 0)
        } else if p.y == 0 {
            p.offset(0, 1)
        } else if p.y == h - 1 {
            p.offset(0, -1)
        } else {
            // An inner door, as the roof door beside the stairwell: the tile west of it.
            p.offset(-1, 0)
        }
    }

    pub fn exit_at(&self, p: Pos) -> Option<&Exit> {
        self.exits.iter().find(|e| e.at == p)
    }

    pub fn room_at(&self, p: Pos) -> Option<&Room> {
        self.rooms
            .iter()
            .filter(|r| r.kind != RoomKind::Corridor && r.kind != RoomKind::Hall)
            .find(|r| r.rect.contains(p))
            .or_else(|| self.rooms.iter().find(|r| r.rect.contains(p)))
    }

    /// The room whose door is `p`.
    pub fn door_of(&self, p: Pos) -> Option<&Room> {
        self.rooms.iter().find(|r| r.door == Some(p))
    }

    /// The operating chair on a ripperdoc's floor.
    pub fn chair(&self) -> Option<Pos> {
        self.tiles
            .iter()
            .find(|(_, t)| {
                t.feature
                    == Some(crate::tile::Feature::Furniture(
                        crate::tile::Furniture::Chair,
                    ))
            })
            .map(|(p, _)| p)
    }

    pub fn station_at(&self, p: Pos) -> Option<&Station> {
        self.stations.iter().find(|s| s.pos == p)
    }
}
