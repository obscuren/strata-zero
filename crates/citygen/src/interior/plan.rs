//! The plan of a closed building's floors: how many, which open onto which stratum,
//! and the links between them (lifts, stairs, escalators, ladders and crawls).

use crate::clinic::{Front, clinic_front};
use crate::geom::{Pos, Rect, Size};
use crate::ids::BuildingId;
use crate::rng::hash64;
use crate::tile::Terrain;
use crate::world::Theme;
use crate::world::{Building, BuildingKind, City, StratumKind};
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
    /// Which stratum each level with doors opens onto: the tunnel from -1, the street
    /// from 0, the bridge floor onto the skyways, the top floor onto the roof. Each floor
    /// listed has a door there.
    pub doors: Vec<LevelDoor>,
    /// How you move between levels.
    pub links: Vec<LevelLink>,
    /// The railing ring around an atrium megablock's open drop, the same on every
    /// floor above the ground; `None` for every other building.
    pub atrium: Option<Rect>,
    /// The floor with the player's flat: only on the city's first atrium megablock.
    pub home_level: Option<i32>,
    /// A ripperdoc's clinic: its front and how its grid is turned; `None` for every
    /// other building.
    pub clinic: Option<ClinicPlan>,
}

/// A level whose doors open onto a stratum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LevelDoor {
    pub level: i32,
    pub stratum: StratumKind,
}

/// What carries you between levels.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum LinkKind {
    Lift,
    Stair,
    Escalator,
    Ladder,
    Crawl,
}

impl LinkKind {
    /// The terrain of its landings: a crawl comes up a ladder.
    pub fn terrain(self) -> Terrain {
        match self {
            LinkKind::Lift => Terrain::Lift,
            LinkKind::Stair => Terrain::Stair,
            LinkKind::Escalator => Terrain::Escalator,
            LinkKind::Ladder | LinkKind::Crawl => Terrain::Ladder,
        }
    }
}

fn yes() -> bool {
    true
}

/// Where a link stops on one level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LinkStop {
    pub level: i32,
    /// The landing; a lift's bank is this tile and the one east of it.
    pub at: Pos,
    /// The zone card a lift asks for to stop here; 0 asks none.
    #[serde(default)]
    pub card: u8,
    /// You can get on here, and get off here.
    #[serde(default = "yes")]
    pub on: bool,
    #[serde(default = "yes")]
    pub off: bool,
}

/// A lift, a stair, an escalator, a ladder or a crawl, and where it stops.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LevelLink {
    pub kind: LinkKind,
    /// Lowest first.
    pub stops: Vec<LinkStop>,
    #[serde(default)]
    pub name: Option<String>,
}

impl LevelLink {
    /// Its stop on `level`.
    pub fn stop(&self, level: i32) -> Option<&LinkStop> {
        self.stops.iter().find(|s| s.level == level)
    }

    /// Its landing tiles on `level`: both tiles of a lift's bank, one of anything else.
    pub fn tiles(&self, level: i32) -> Vec<Pos> {
        match self.stop(level) {
            Some(s) if self.kind == LinkKind::Lift => vec![s.at, s.at.offset(1, 0)],
            Some(s) => vec![s.at],
            None => Vec::new(),
        }
    }
}

/// Today's links: the lift bank, then the stairwell, each stopping on every level.
fn classic_links(levels: &[i32], lift: Option<Pos>, stair: Option<Pos>) -> Vec<LevelLink> {
    let link = |kind, at: Pos| LevelLink {
        kind,
        stops: levels
            .iter()
            .map(|&level| LinkStop {
                level,
                at,
                card: 0,
                on: true,
                off: true,
            })
            .collect(),
        name: None,
    };
    lift.map(|at| link(LinkKind::Lift, at))
        .into_iter()
        .chain(stair.map(|at| link(LinkKind::Stair, at)))
        .collect()
}

/// A `FloorPlan` as a save holds it. Saves written before a building's levels were a
/// list (`runs/fixtures/run-v1*.run`) name its lowest and top floors instead; saves
/// written before its levels named their doors name its bridge floor and whether it
/// has a roof door; and saves written before its links name its lift bank and its
/// stairwell, both on every floor.
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
    #[serde(default)]
    doors: Vec<LevelDoor>,
    #[serde(default)]
    bridge: Option<i32>,
    #[serde(default)]
    roof: bool,
    #[serde(default)]
    links: Vec<LevelLink>,
    #[serde(default)]
    lift: Option<Pos>,
    #[serde(default)]
    stair: Option<Pos>,
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
        let doors = if r.doors.is_empty() {
            // These saves don't say whether the city put a door by the basement: it opens
            // onto the tunnel, as they read it then.
            let lowest = levels.first().copied().unwrap_or(0);
            let top = levels.last().copied().unwrap_or(0);
            classic_doors(
                (lowest == -1).then_some(-1),
                r.bridge,
                r.roof.then_some(top),
            )
        } else {
            r.doors
        };
        // An old one-floor plan names a stairwell that led nowhere and was never drawn.
        let links = if r.links.is_empty() {
            classic_links(&levels, r.lift, r.stair.filter(|_| levels.len() >= 2))
        } else {
            r.links
        };
        FloorPlan {
            building: r.building,
            kind: r.kind,
            theme: r.theme,
            footprint: r.footprint,
            size: r.size,
            levels,
            doors,
            links,
            atrium: r.atrium,
            home_level: r.home_level,
            clinic: r.clinic,
        }
    }
}

/// The doors of a building: the tunnel from the `tunnel` floor (its basement), the street
/// from the ground floor, the skyways from the `bridge` floor and the roof from the
/// `roof` floor.
fn classic_doors(tunnel: Option<i32>, bridge: Option<i32>, roof: Option<i32>) -> Vec<LevelDoor> {
    let door = |level, stratum| LevelDoor { level, stratum };
    let mut doors: Vec<LevelDoor> = (tunnel.map(|l| door(l, StratumKind::Undercity)))
        .into_iter()
        .collect();
    doors.push(door(0, StratumKind::Street));
    doors.extend(bridge.map(|l| door(l, StratumKind::Skyways)));
    doors.extend(roof.map(|l| door(l, StratumKind::Heights)));
    doors
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

/// A floor over footprint `fp`: the footprint x 3, at least `MIN_W` x `MIN_H`.
pub(in crate::interior) fn floor_size(fp: Rect) -> Size {
    Size {
        w: (fp.w * SCALE).max(MIN_W) as u32,
        h: (fp.h * SCALE).max(MIN_H) as u32,
    }
}

/// The floor a building's Skyways doors open from: the third, or the top when it is
/// lower, never the ground.
pub(in crate::interior) fn bridge_level(top: i32) -> i32 {
    3.min(top).max(1)
}

/// A building's plan rng: today's builder draws its floor count from it first.
fn plan_rng(city: &City, id: BuildingId) -> ChaCha8Rng {
    ChaCha8Rng::seed_from_u64(hash64(city.seed, 0x1F00 ^ id.index() as u64))
}

/// A closed building's lowest and top level, as today's builder makes them: the top
/// from the floor count its plan's rng draws first, the lowest -1 when the building
/// reaches the Undercity or is a clinic with a cellar.
pub(crate) fn storeys(city: &City, id: BuildingId) -> (i32, i32) {
    let b = city.building(id);
    let cellar = clinic_plan(city, id, b.footprint).is_some_and(|c| c.front.floors() == 2);
    draw_storeys(b, cellar, &mut plan_rng(city, id))
}

/// `storeys` from a plan rng that has drawn nothing yet.
fn draw_storeys(b: &Building, cellar: bool, rng: &mut ChaCha8Rng) -> (i32, i32) {
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
    let lowest = if b.span.lo == StratumKind::Undercity || cellar {
        -1
    } else {
        0
    };
    (lowest, floors - 1)
}

impl FloorPlan {
    /// The plan of a closed building; `None` for an open one.
    pub fn new(city: &City, id: BuildingId) -> Option<FloorPlan> {
        FloorPlan::classic(city, id)
    }

    /// Today's builder's plan: the one `new` gives every closed building.
    pub fn classic(city: &City, id: BuildingId) -> Option<FloorPlan> {
        let b = city.building_opt(id)?;
        if !b.kind.is_closed() {
            return None;
        }
        let mut rng = plan_rng(city, id);
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
        let cellar = clinic.is_some_and(|c| c.front.floors() == 2);
        let (lowest, top) = draw_storeys(b, cellar, &mut rng);
        // A basement opens onto the tunnel, and a floor onto the skyways, only where the
        // city put a door to it.
        let has_door = |k: StratumKind| city.portals(id).iter().any(|(s, _)| *s == k);
        let tunnel = (lowest == -1 && has_door(StratumKind::Undercity)).then_some(-1);
        let bridge = has_door(StratumKind::Skyways).then(|| bridge_level(top));
        let roof = (b.span.hi == StratumKind::Heights).then_some(top);
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
        let levels: Vec<i32> = (lowest..=top).collect();
        Some(FloorPlan {
            building: id,
            kind: b.kind,
            theme: city.district(b.district).theme,
            footprint: fp,
            size,
            doors: classic_doors(tunnel, bridge, roof),
            links: classic_links(&levels, lift, (total >= 2).then_some(stair)),
            levels,
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
                } else if self.door_stratum(level) == Some(StratumKind::Heights)
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

    /// The stratum a level's doors open onto: the first entry for the level.
    pub fn door_stratum(&self, level: i32) -> Option<StratumKind> {
        self.doors
            .iter()
            .find(|d| d.level == level)
            .map(|d| d.stratum)
    }

    /// The level a city-map door on `k` opens onto: the first entry for `k`.
    pub fn door_level(&self, k: StratumKind) -> Option<i32> {
        self.doors.iter().find(|d| d.stratum == k).map(|d| d.level)
    }

    /// The link with a landing tile at `p` on `level`.
    pub fn link_at(&self, level: i32, p: Pos) -> Option<usize> {
        self.links.iter().position(|l| l.tiles(level).contains(&p))
    }

    /// Every link's landing tiles on a level, in link order.
    pub fn landings(&self, level: i32) -> Vec<Pos> {
        self.links.iter().flat_map(|l| l.tiles(level)).collect()
    }

    /// The stops of link `link` you can get off at, on the plan's levels, `from` left
    /// out; none when you can't get on at `from`.
    fn alights(&self, link: usize, from: i32) -> impl Iterator<Item = i32> + '_ {
        let link = self.links.get(link);
        let boards = link
            .and_then(|l| l.stop(from))
            .is_some_and(|s| s.on && self.has_level(from));
        link.filter(|_| boards)
            .into_iter()
            .flat_map(|l| l.stops.iter())
            .filter(move |s| s.off && s.level != from && self.has_level(s.level))
            .map(|s| s.level)
    }

    /// From `level` on link `link`: the next stop up or down you can get off at;
    /// None when you can't get on here. Stops are read through `levels`: a stop whose
    /// level the plan lacks is skipped.
    pub fn next_stop(&self, link: usize, level: i32, up: bool) -> Option<i32> {
        let stops = self.alights(link, level);
        if up {
            stops.filter(|l| *l > level).min()
        } else {
            stops.filter(|l| *l < level).max()
        }
    }

    /// The levels a lift takes you to from `from`: its stops you can get off at,
    /// `from` left out; empty when you can't get on at `from`. Stops on levels the plan
    /// lacks are left out.
    pub fn rides(&self, link: usize, from: i32) -> Vec<i32> {
        self.alights(link, from).collect()
    }

    /// The first lift's landing on `level`: the west tile of its bank.
    pub fn lift_on(&self, level: i32) -> Option<Pos> {
        self.landing_of(LinkKind::Lift, level)
    }

    /// The first stair's landing on `level`.
    pub fn stair_on(&self, level: i32) -> Option<Pos> {
        self.landing_of(LinkKind::Stair, level)
    }

    fn landing_of(&self, kind: LinkKind, level: i32) -> Option<Pos> {
        self.links
            .iter()
            .filter(|l| l.kind == kind)
            .find_map(|l| l.stop(level))
            .map(|s| s.at)
    }

    /// Where a floor's people and its connect pass gather: the first landing, else
    /// `(w / 2 - 1, h / 2)`, where the stairwell stood in a one-floor building.
    pub fn anchor(&self, level: i32) -> Pos {
        self.landings(level)
            .first()
            .copied()
            .unwrap_or(Pos::new(self.size.w as i32 / 2 - 1, self.size.h as i32 / 2))
    }
}
