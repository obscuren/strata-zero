//! The overworld: a world of tiles, one cell each on screen, that cities sit in.
//!
//! Ten stages run in order over one tile grid, each deterministic from the seed and the
//! stages before it. Shapes are measured on screen: a tile is twice as tall as it is
//! wide, so every distance and noise sample works in s-space, `(x, 2y)` (see `field`).

pub(crate) mod field;
mod ground;
mod growth;
mod land;
mod names;
mod pois;
mod rivers;
mod roads;
mod sites;

use crate::flavor::FlavorPack;
use crate::geom::{Dir, Grid, Pos, Size};
use crate::params::GenError;
use crate::rng::stage_rng;
use crate::world::Theme;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

/// The world's size in tiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorldSize {
    Small,
    Medium,
    Large,
    Custom { w: u32, h: u32 },
}

impl WorldSize {
    pub fn size(self) -> Size {
        match self {
            WorldSize::Small => Size { w: 120, h: 40 },
            WorldSize::Medium => Size { w: 160, h: 52 },
            WorldSize::Large => Size { w: 220, h: 72 },
            WorldSize::Custom { w, h } => Size { w, h },
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            WorldSize::Small => "small",
            WorldSize::Medium => "medium",
            WorldSize::Large => "large",
            WorldSize::Custom { .. } => "custom",
        }
    }
}

/// Counts a test run can force; `None` lets the seed choose.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Knobs {
    pub sea_sides: Option<u8>,
    pub ridges: Option<u8>,
    pub rivers: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldParams {
    pub seed: u64,
    pub size: WorldSize,
    pub cities: u8,
    pub flavor: String,
    #[serde(default)]
    pub knobs: Knobs,
}

impl Default for WorldParams {
    fn default() -> Self {
        WorldParams {
            seed: 0,
            size: WorldSize::Medium,
            cities: 5,
            flavor: "default".into(),
            knobs: Knobs::default(),
        }
    }
}

impl WorldParams {
    pub fn validate(&self) -> Result<(), GenError> {
        let bad = |field: &'static str, reason: String| GenError::InvalidParams { field, reason };
        let Size { w, h } = self.size.size();
        if !(60..=512).contains(&w) || !(24..=256).contains(&h) {
            return Err(bad("size", format!("{w}x{h} is outside 60..=512 by 24..=256")));
        }
        if !(3..=8).contains(&self.cities) {
            return Err(bad("cities", format!("{} is outside 3..=8", self.cities)));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Water {
    #[default]
    None,
    Sea,
    River,
    Lake,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ground {
    #[default]
    Flats,
    Scrub,
    Fields,
    Salt,
    Spill,
    Hills,
    Mountains,
    Peak,
}

/// A city's hold on a tile: whose, how dense (0..=255), and the district colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Urban {
    pub city: u8,
    pub density: u8,
    pub theme: Theme,
}

pub const LINK_N: u8 = 1;
pub const LINK_E: u8 = 2;
pub const LINK_S: u8 = 4;
pub const LINK_W: u8 = 8;

/// The link bit from `a` toward the four-neighbour `b`.
pub fn link_toward(a: Pos, b: Pos) -> u8 {
    match (b.x - a.x, b.y - a.y) {
        (0, -1) => LINK_N,
        (1, 0) => LINK_E,
        (0, 1) => LINK_S,
        (-1, 0) => LINK_W,
        _ => 0,
    }
}

/// One tile of the world. `road` and `river` are link masks (`LINK_*`), nonzero where
/// one runs; a river tile can be a city's too.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorldTile {
    pub water: Water,
    pub ground: Ground,
    pub elevation: u8,
    pub road: u8,
    pub river: u8,
    pub urban: Option<Urban>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldCity {
    pub name: String,
    pub centre: Pos,
    /// Radius on screen, in s-space columns.
    pub size: f32,
    pub home: bool,
    pub towns: Vec<Pos>,
}

/// One highway link; links of one named road share its name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Route {
    pub name: String,
    pub path: Vec<Pos>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PoiKind {
    TruckStop,
    Checkpoint,
    Relay,
    Camp,
    Wreck,
    Arcology,
    Solar,
    WaterPlant,
    TestRange,
    Rig,
}

impl PoiKind {
    pub const ALL: [PoiKind; 10] = [
        PoiKind::TruckStop,
        PoiKind::Checkpoint,
        PoiKind::Relay,
        PoiKind::Camp,
        PoiKind::Wreck,
        PoiKind::Arcology,
        PoiKind::Solar,
        PoiKind::WaterPlant,
        PoiKind::TestRange,
        PoiKind::Rig,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PoiKind::TruckStop => "truck stop",
            PoiKind::Checkpoint => "checkpoint",
            PoiKind::Relay => "relay mast",
            PoiKind::Camp => "nomad camp",
            PoiKind::Wreck => "wreck",
            PoiKind::Arcology => "dead arcology",
            PoiKind::Solar => "solar array",
            PoiKind::WaterPlant => "water plant",
            PoiKind::TestRange => "test range",
            PoiKind::Rig => "rig",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Poi {
    pub kind: PoiKind,
    pub name: String,
    pub at: Pos,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RegionKind {
    Range,
    Bay,
    Spill,
    Salt,
    Flats,
    Valley,
}

/// A named stretch of the world and the tile its label sits on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    pub kind: RegionKind,
    pub name: String,
    pub at: Pos,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Overworld {
    pub seed: u64,
    pub params: WorldParams,
    pub size: Size,
    pub tiles: Grid<WorldTile>,
    pub cities: Vec<WorldCity>,
    pub routes: Vec<Route>,
    pub rivers: Vec<Vec<Pos>>,
    pub pois: Vec<Poi>,
    pub regions: Vec<Region>,
}

impl Overworld {
    pub fn generate(params: WorldParams) -> Result<Overworld, GenError> {
        params.validate()?;
        let flavor = FlavorPack::load(&params.flavor)?;
        let mut ctx = Ctx::new(params, flavor);
        land::run(&mut ctx)?;
        rivers::run(&mut ctx)?;
        sites::run(&mut ctx)?;
        ground::run(&mut ctx)?;
        roads::run(&mut ctx)?;
        growth::run(&mut ctx)?;
        pois::run(&mut ctx)?;
        names::run(&mut ctx)?;
        Ok(ctx.finish())
    }

    pub fn tile(&self, p: Pos) -> Option<&WorldTile> {
        self.tiles.get(p)
    }

    pub fn poi_at(&self, p: Pos) -> Option<&Poi> {
        self.pois.iter().find(|poi| poi.at == p)
    }

    pub fn home(&self) -> &WorldCity {
        self.cities
            .iter()
            .find(|c| c.home)
            .expect("a world has a home city")
    }
}

/// The world as the stages build it, and what they hand each other.
pub(crate) struct Ctx {
    pub params: WorldParams,
    pub size: Size,
    pub flavor: FlavorPack,
    pub tiles: Grid<WorldTile>,
    pub sea_sides: Vec<Dir>,
    pub bays: Vec<land::Bay>,
    pub ridges: Vec<Vec<(f32, f32)>>,
    pub rivers: Vec<Vec<Pos>>,
    pub mouths: Vec<Pos>,
    pub cities: Vec<WorldCity>,
    pub spill: Option<Pos>,
    pub salt: Option<Pos>,
    pub routes: Vec<Route>,
    pub chains: Vec<Vec<usize>>,
    pub pois: Vec<Poi>,
    pub regions: Vec<Region>,
}

impl Ctx {
    fn new(params: WorldParams, flavor: FlavorPack) -> Ctx {
        let size = params.size.size();
        Ctx {
            params,
            size,
            flavor,
            tiles: Grid::filled(size, WorldTile::default()),
            sea_sides: Vec::new(),
            bays: Vec::new(),
            ridges: Vec::new(),
            rivers: Vec::new(),
            mouths: Vec::new(),
            cities: Vec::new(),
            spill: None,
            salt: None,
            routes: Vec::new(),
            chains: Vec::new(),
            pois: Vec::new(),
            regions: Vec::new(),
        }
    }

    pub fn seed(&self) -> u64 {
        self.params.seed
    }

    /// The RNG private to stage `n`.
    pub fn rng(&self, n: u8) -> ChaCha8Rng {
        stage_rng(self.params.seed, 100 + n)
    }

    /// Fractal noise private to `salt`.
    pub fn fbm(&self, salt: u32) -> field::Fbm {
        field::Fbm::new(self.params.seed, salt)
    }

    pub fn tile(&self, p: Pos) -> &WorldTile {
        &self.tiles[p]
    }

    pub fn tile_mut(&mut self, p: Pos) -> &mut WorldTile {
        &mut self.tiles[p]
    }

    pub fn in_bounds(&self, p: Pos) -> bool {
        self.size.contains(p)
    }

    /// Every tile, row by row.
    pub fn all(&self) -> impl Iterator<Item = Pos> + use<> {
        let Size { w, h } = self.size;
        (0..h as i32).flat_map(move |y| (0..w as i32).map(move |x| Pos::new(x, y)))
    }

    fn finish(self) -> Overworld {
        Overworld {
            seed: self.params.seed,
            size: self.size,
            params: self.params,
            tiles: self.tiles,
            cities: self.cities,
            routes: self.routes,
            rivers: self.rivers,
            pois: self.pois,
            regions: self.regions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_params_same_world_bytes() {
        for size in [WorldSize::Small, WorldSize::Medium, WorldSize::Large] {
            let p = WorldParams {
                seed: 7,
                size,
                ..Default::default()
            };
            let a = postcard::to_allocvec(&Overworld::generate(p.clone()).unwrap()).unwrap();
            let b = postcard::to_allocvec(&Overworld::generate(p).unwrap()).unwrap();
            assert_eq!(a, b, "{size:?}");
        }
    }

    #[test]
    fn bad_params_are_refused() {
        let bad = |p: WorldParams| assert!(Overworld::generate(p).is_err());
        bad(WorldParams {
            cities: 2,
            ..Default::default()
        });
        bad(WorldParams {
            cities: 9,
            ..Default::default()
        });
        bad(WorldParams {
            size: WorldSize::Custom { w: 40, h: 52 },
            ..Default::default()
        });
    }
}
