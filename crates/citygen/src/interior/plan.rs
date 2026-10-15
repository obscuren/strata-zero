//! The plan of a closed building's floors: how many, which are special, where the
//! lift bank and the stairwell stand on every one of them.

use crate::geom::{Pos, Rect, Size};
use crate::ids::BuildingId;
use crate::rng::hash64;
use crate::world::Theme;
use crate::world::{BuildingKind, City, StratumKind};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Footprint tiles to floor tiles.
pub const SCALE: i32 = 3;
pub const MIN_W: i32 = 12;
pub const MIN_H: i32 = 9;

/// What a floor is laid out as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloorType {
    Lobby,
    Corridor,
    Suite,
    Bar,
    Vip,
    Clinic,
    Temple,
    Hall,
    Factory,
    Garage,
    Basement,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FloorPlan {
    pub building: BuildingId,
    pub kind: BuildingKind,
    pub theme: Theme,
    pub footprint: Rect,
    pub size: Size,
    /// -1 with a basement, else 0.
    pub lowest: i32,
    pub top: i32,
    /// The floor the bridge door opens from.
    pub bridge: Option<i32>,
    /// A roof door on the top floor.
    pub roof: bool,
    /// The lift bank: this tile and the one east of it, on every floor.
    pub lift: Option<Pos>,
    /// The stairwell, on every floor.
    pub stair: Pos,
}

impl FloorPlan {
    /// The plan of a closed building; `None` for an open one.
    pub fn new(city: &City, id: BuildingId) -> Option<FloorPlan> {
        let b = city.building_opt(id)?;
        if !b.kind.is_closed() {
            return None;
        }
        let mut rng = ChaCha8Rng::seed_from_u64(hash64(city.seed, 0x1F00 ^ id.index() as u64));
        let fp = b.footprint;
        let size = Size {
            w: (fp.w * SCALE).max(MIN_W) as u32,
            h: (fp.h * SCALE).max(MIN_H) as u32,
        };
        let floors: i32 = match (b.span.hi, b.kind) {
            (StratumKind::Street, BuildingKind::Club) => 2,
            (StratumKind::Street, BuildingKind::Office | BuildingKind::Apartment) => {
                rng.random_range(2..=3)
            }
            (StratumKind::Street, _) => 1,
            (StratumKind::Skyways, _) => rng.random_range(4..=9),
            (_, BuildingKind::CorpTower) => rng.random_range(18..=48),
            (_, BuildingKind::Office) => rng.random_range(12..=24),
            (_, BuildingKind::Megablock) => rng.random_range(14..=30),
            (_, BuildingKind::Hotel) => rng.random_range(10..=24),
            _ => rng.random_range(8..=14),
        };
        let top = floors - 1;
        let lowest = if b.span.lo == StratumKind::Undercity {
            -1
        } else {
            0
        };
        let has_bridge = city
            .portals(id)
            .iter()
            .any(|(k, _)| *k == StratumKind::Skyways);
        let bridge = has_bridge.then(|| 3.min(top).max(1));
        let roof = b.span.hi == StratumKind::Heights;
        let total = top - lowest + 1;
        let (w, h) = (size.w as i32, size.h as i32);
        // The lift bank and the stairwell sit on the corridor line of a corridor
        // floor, which is open floor on every other type.
        let lift = (total >= 3).then(|| Pos::new(w / 2 - 1, h / 2 - 1));
        let stair = Pos::new(w / 2 - 1, h / 2);
        Some(FloorPlan {
            building: id,
            kind: b.kind,
            theme: city.district(b.district).theme,
            footprint: fp,
            size,
            lowest,
            top,
            bridge,
            roof,
            lift,
            stair,
        })
    }

    pub fn levels(&self) -> impl Iterator<Item = i32> {
        self.lowest..=self.top
    }

    pub fn floors(&self) -> i32 {
        self.top - self.lowest + 1
    }

    /// What the lift panel and the HUD call a floor.
    pub fn label(level: i32) -> String {
        match level {
            l if l < 0 => format!("B{}", -l),
            0 => "L".to_string(),
            n => format!("{n}F"),
        }
    }

    pub fn floor_type(&self, level: i32) -> FloorType {
        if level < 0 {
            return FloorType::Basement;
        }
        match self.kind {
            BuildingKind::CorpTower
            | BuildingKind::Office
            | BuildingKind::Megablock
            | BuildingKind::Apartment
            | BuildingKind::Hotel => {
                if level == 0 {
                    FloorType::Lobby
                } else if self.roof
                    && level == self.top
                    && matches!(self.kind, BuildingKind::CorpTower | BuildingKind::Office)
                {
                    FloorType::Suite
                } else {
                    FloorType::Corridor
                }
            }
            BuildingKind::Bar => FloorType::Bar,
            BuildingKind::Club => {
                if level == 0 {
                    FloorType::Bar
                } else {
                    FloorType::Vip
                }
            }
            BuildingKind::Clinic => FloorType::Clinic,
            BuildingKind::Temple => FloorType::Temple,
            BuildingKind::Factory => FloorType::Factory,
            BuildingKind::Garage => FloorType::Garage,
            _ => FloorType::Hall,
        }
    }

    /// The floors people work on: the office floors of a tower, the one floor of
    /// everything else.
    pub fn work_floors(&self) -> Vec<i32> {
        let offices: Vec<i32> = self
            .levels()
            .filter(|l| matches!(self.floor_type(*l), FloorType::Corridor | FloorType::Suite))
            .collect();
        if matches!(self.kind, BuildingKind::CorpTower | BuildingKind::Office)
            && !offices.is_empty()
        {
            offices
        } else {
            vec![0]
        }
    }

    /// The floors with flats or rooms: where residents and guests are.
    pub fn corridor_floors(&self) -> Vec<i32> {
        self.levels()
            .filter(|l| self.floor_type(*l) == FloorType::Corridor)
            .collect()
    }

    /// The stratum a floor's doors open onto, if any.
    pub fn exit_stratum(&self, level: i32) -> Option<StratumKind> {
        if level == -1 {
            Some(StratumKind::Undercity)
        } else if level == 0 {
            Some(StratumKind::Street)
        } else if self.bridge == Some(level) {
            Some(StratumKind::Skyways)
        } else if self.roof && level == self.top {
            Some(StratumKind::Heights)
        } else {
            None
        }
    }

    /// The second tile of the lift bank.
    pub fn lift_bank(&self) -> Vec<Pos> {
        match self.lift {
            Some(p) => vec![p, p.offset(1, 0)],
            None => Vec::new(),
        }
    }
}
