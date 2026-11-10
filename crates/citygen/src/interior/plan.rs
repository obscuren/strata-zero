//! The plan of a closed building's floors: how many, which are special, where the
//! lift bank and the stairwell stand on every one of them.

use crate::clinic::{Front, clinic_front};
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
/// An atrium floor's flat band: the outer wall and seven rows of flats, the flat
/// wall on row `BAND`.
pub const BAND: i32 = 8;
/// The walkway between the flat wall and the railing.
pub const WALK: i32 = 2;
/// The smallest Megablock floor that rings an atrium (a 13x9 footprint).
const ATRIUM_W: i32 = 39;
const ATRIUM_H: i32 = 27;

/// What a floor is laid out as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    /// An upper floor of an atrium megablock: flats around a walkway around the drop.
    Atrium,
    /// The ground floor of an atrium megablock.
    Plaza,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(from = "FloorPlanRead")]
pub struct FloorPlan {
    pub building: BuildingId,
    pub kind: BuildingKind,
    pub theme: Theme,
    pub footprint: Rect,
    pub size: Size,
    /// The building's floors, the lowest first: from -1 with a basement, else from 0.
    pub levels: Vec<i32>,
    /// The floor the bridge door opens from.
    pub bridge: Option<i32>,
    /// A roof door on the top floor.
    pub roof: bool,
    /// The lift bank: this tile and the one east of it, on every floor.
    pub lift: Option<Pos>,
    /// The stairwell, on every floor.
    pub stair: Pos,
    /// The railing ring around an atrium megablock's open drop, the same on every
    /// floor above the ground; `None` for every other building.
    pub atrium: Option<Rect>,
    /// The floor with the player's flat: only on the city's first atrium megablock.
    pub home_level: Option<i32>,
    /// A ripperdoc's clinic: its front and how its grid is turned; `None` for every
    /// other building.
    pub clinic: Option<ClinicPlan>,
}

/// A `FloorPlan` as a save holds it. Saves written before a building's levels were a
/// list (`runs/fixtures/run-v1*.run`) name its lowest and top floors instead.
#[derive(serde::Deserialize)]
struct FloorPlanRead {
    building: BuildingId,
    kind: BuildingKind,
    theme: Theme,
    footprint: Rect,
    size: Size,
    #[serde(default)]
    levels: Vec<i32>,
    #[serde(default)]
    lowest: Option<i32>,
    #[serde(default)]
    top: Option<i32>,
    bridge: Option<i32>,
    roof: bool,
    lift: Option<Pos>,
    stair: Pos,
    atrium: Option<Rect>,
    home_level: Option<i32>,
    clinic: Option<ClinicPlan>,
}

impl From<FloorPlanRead> for FloorPlan {
    fn from(r: FloorPlanRead) -> FloorPlan {
        let levels = if r.levels.is_empty() {
            let lowest = r.lowest.unwrap_or(0);
            (lowest..=r.top.unwrap_or(lowest)).collect()
        } else {
            r.levels
        };
        FloorPlan {
            building: r.building,
            kind: r.kind,
            theme: r.theme,
            footprint: r.footprint,
            size: r.size,
            levels,
            bridge: r.bridge,
            roof: r.roof,
            lift: r.lift,
            stair: r.stair,
            atrium: r.atrium,
            home_level: r.home_level,
            clinic: r.clinic,
        }
    }
}

/// How a grid is laid on a floor: mirrored left to right first, then turned clockwise
/// a quarter at a time. A grid's bottom row ends up on the south (0 quarters), west
/// (1), north (2) or east (3) edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Turn {
    pub quarters: u8,
    pub mirror: bool,
}

/// A clinic's floors: the front's grid, turned so its street side faces the main door.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ClinicPlan {
    pub front: Front,
    pub turn: Turn,
    /// The main street door on the city map.
    pub door: Pos,
    /// Its exit on the street floor: the grid's front door, turned.
    pub entrance: Pos,
}

/// A `w` by `h` grid's size once turned.
pub fn turn_size(w: i32, h: i32, t: Turn) -> (i32, i32) {
    if t.quarters % 2 == 1 { (h, w) } else { (w, h) }
}

/// Where a tile of a `w` by `h` grid lands once turned.
pub fn turn_pos(p: Pos, w: i32, h: i32, t: Turn) -> Pos {
    let mut q = if t.mirror {
        Pos::new(w - 1 - p.x, p.y)
    } else {
        p
    };
    let (mut cw, mut ch) = (w, h);
    for _ in 0..t.quarters % 4 {
        q = Pos::new(ch - 1 - q.y, q.x);
        (cw, ch) = (ch, cw);
    }
    let _ = cw;
    q
}

/// A clinic's plan: the front, the turn that faces its street side to the storefront
/// door, mirrored on the seed.
fn clinic_plan(city: &City, id: BuildingId, fp: Rect) -> Option<ClinicPlan> {
    let front = clinic_front(city, id)?;
    // The storefront: the door the city dresses as the main one, else the first.
    let door = crate::generate::exterior::main_door(city, city.building(id)).or_else(|| {
        city.portals(id)
            .into_iter()
            .find(|(k, _)| *k == StratumKind::Street)
            .map(|(_, p)| p)
    })?;
    let quarters = match super::layouts::edge(fp, door) {
        3 => 0,
        0 => 1,
        2 => 2,
        _ => 3,
    };
    let turn = Turn {
        quarters,
        mirror: hash64(city.seed, 0xC11D ^ id.index() as u64) & 1 == 1,
    };
    let street = super::layouts::clinics::grid(front, 0);
    let (w, h) = super::layouts::clinics::dims(street);
    let e = super::layouts::clinics::find(street, b'E')?;
    Some(ClinicPlan {
        front,
        turn,
        door,
        entrance: turn_pos(e, w, h, turn),
    })
}

/// Whether a building plans an atrium.
fn rings_an_atrium(kind: BuildingKind, size: Size) -> bool {
    kind == BuildingKind::Megablock && size.w as i32 >= ATRIUM_W && size.h as i32 >= ATRIUM_H
}

fn floor_size(fp: Rect) -> Size {
    Size {
        w: (fp.w * SCALE).max(MIN_W) as u32,
        h: (fp.h * SCALE).max(MIN_H) as u32,
    }
}

impl FloorPlan {
    /// The plan of a closed building; `None` for an open one.
    pub fn new(city: &City, id: BuildingId) -> Option<FloorPlan> {
        FloorPlan::classic(city, id)
    }

    /// Today's builder's plan. `new` calls it until Task 12 dispatches to venues.
    pub fn classic(city: &City, id: BuildingId) -> Option<FloorPlan> {
        let b = city.building_opt(id)?;
        if !b.kind.is_closed() {
            return None;
        }
        let mut rng = ChaCha8Rng::seed_from_u64(hash64(city.seed, 0x1F00 ^ id.index() as u64));
        let fp = b.footprint;
        let clinic = clinic_plan(city, id, fp);
        // A clinic's floor is its grid, turned.
        let size = match clinic {
            Some(c) => {
                let (w, h) =
                    super::layouts::clinics::dims(super::layouts::clinics::grid(c.front, 0));
                let (w, h) = turn_size(w, h, c.turn);
                Size {
                    w: w as u32,
                    h: h as u32,
                }
            }
            None => floor_size(fp),
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
        let cellar = clinic.is_some_and(|c| c.front.floors() == 2);
        let lowest = if b.span.lo == StratumKind::Undercity || cellar {
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
        let atrium = rings_an_atrium(b.kind, size).then(|| size.rect().inset(BAND + 1 + WALK));
        // The lift bank and the stairwell sit on the corridor line of a corridor
        // floor, which is open floor on every other type; an atrium floor keeps them
        // in the lift lobby in the middle of its top band of flats.
        let (lift_y, stair) = if atrium.is_some() {
            (BAND - 3, Pos::new(w / 2 - 1, BAND - 2))
        } else {
            (h / 2 - 1, Pos::new(w / 2 - 1, h / 2))
        };
        let lift = (total >= 3).then(|| Pos::new(w / 2 - 1, lift_y));
        // A clinic with a cellar has its stair where its grid puts it.
        let stair = clinic
            .filter(|_| cellar)
            .and_then(|c| {
                let street = super::layouts::clinics::grid(c.front, 0);
                let (gw, gh) = super::layouts::clinics::dims(street);
                super::layouts::clinics::find(street, b'v').map(|v| turn_pos(v, gw, gh, c.turn))
            })
            .unwrap_or(stair);
        // The home: high in the lowest-id atrium megablock, below the top floor and
        // off the bridge floor.
        let first_atrium = city
            .buildings
            .iter()
            .filter(|b| rings_an_atrium(b.kind, floor_size(b.footprint)))
            .map(|b| b.id)
            .min_by_key(|b| b.index());
        let home_level = if first_atrium == Some(id) {
            let levels: Vec<i32> = (2.max(top * 2 / 3)..top)
                .filter(|l| Some(*l) != bridge)
                .collect();
            (!levels.is_empty()).then(|| levels[rng.random_range(0..levels.len())])
        } else {
            None
        };
        Some(FloorPlan {
            building: id,
            kind: b.kind,
            theme: city.district(b.district).theme,
            footprint: fp,
            size,
            levels: (lowest..=top).collect(),
            bridge,
            roof,
            lift,
            stair,
            atrium,
            home_level,
            clinic,
        })
    }

    /// The building's floors, the lowest first.
    pub fn levels(&self) -> impl Iterator<Item = i32> + '_ {
        self.levels.iter().copied()
    }

    pub fn lowest(&self) -> i32 {
        self.levels.first().copied().unwrap_or(0)
    }

    pub fn top(&self) -> i32 {
        self.levels.last().copied().unwrap_or(0)
    }

    pub fn has_level(&self, level: i32) -> bool {
        self.levels.contains(&level)
    }

    pub fn floors(&self) -> i32 {
        self.levels.len() as i32
    }

    /// The next level in the list above (`up`) or below `level`.
    pub fn next_level(&self, level: i32, up: bool) -> Option<i32> {
        let mut levels = self.levels.iter().copied();
        if up {
            levels.find(|l| *l > level)
        } else {
            levels.rev().find(|l| *l < level)
        }
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
        // Every floor of a clinic is its grid, the laundromat's cellar too.
        if self.clinic.is_some() {
            return FloorType::Clinic;
        }
        if level < 0 {
            return FloorType::Basement;
        }
        if self.atrium.is_some() {
            return if level == 0 {
                FloorType::Plaza
            } else {
                FloorType::Atrium
            };
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
                    && level == self.top()
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
        // A clinic's doc works by the chair: the laundromat's in its cellar.
        if let Some(c) = self.clinic {
            return vec![1 - c.front.floors() as i32];
        }
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
            .filter(|l| matches!(self.floor_type(*l), FloorType::Corridor | FloorType::Atrium))
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
        } else if self.roof && level == self.top() {
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
