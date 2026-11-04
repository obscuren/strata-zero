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
mod washes;

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
            return Err(bad(
                "size",
                format!("{w}x{h} is outside 60..=512 by 24..=256"),
            ));
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
    /// A dry wash's link mask (`LINK_*`), nonzero where one runs.
    pub wash: u8,
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
    /// Each river's course, spring first. A lake made after a river can cover a stretch
    /// of it; the course keeps those tiles so it stays unbroken, and the tile's `water`
    /// says what is there.
    pub rivers: Vec<Vec<Pos>>,
    /// Each dry wash's course, its high end first.
    pub washes: Vec<Vec<Pos>>,
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
        washes::run(&mut ctx)?;
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

    /// The home city. `generate` never hands over a world without one (it refuses a
    /// world with no room for three cities).
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
    pub washes: Vec<Vec<Pos>>,
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
            washes: Vec::new(),
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
            washes: self.washes,
            pois: self.pois,
            regions: self.regions,
        }
    }
}

/// The world as plain characters, for a look from a test (`--nocapture`).
#[cfg(test)]
pub(crate) fn sketch(w: &Overworld) -> String {
    let mut out = String::new();
    for y in 0..w.size.h as i32 {
        for x in 0..w.size.w as i32 {
            let p = Pos::new(x, y);
            let t = w.tile(p).unwrap();
            let ch = if let Some(c) = w.cities.iter().find(|c| c.centre == p) {
                if c.home { '@' } else { 'C' }
            } else if w.poi_at(p).is_some() {
                '*'
            } else if t.road != 0 {
                '='
            } else if t.water == Water::River {
                '-'
            } else if let Some(u) = t.urban {
                if u.density > 128 { '#' } else { '+' }
            } else {
                match (t.water, t.ground) {
                    (Water::Sea, _) => '~',
                    (Water::Lake, _) => 'o',
                    (_, Ground::Peak) => 'A',
                    (_, Ground::Mountains) => '^',
                    (_, Ground::Hills) => 'n',
                    (_, Ground::Spill) => 'x',
                    (_, Ground::Salt) => ':',
                    (_, Ground::Fields) => '_',
                    (_, Ground::Scrub) => '"',
                    _ => '.',
                }
            };
            out.push(ch);
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
#[test]
#[ignore]
fn sketch_a_few() {
    for seed in 0..4 {
        let w = Overworld::generate(WorldParams {
            seed,
            ..Default::default()
        })
        .unwrap();
        println!("seed {seed}\n{}", sketch(&w));
    }
}

/// The invariants that once broke on a few seeds in a few hundred, over 500 seeds at
/// two presets. Slow; run it after a tuning pass:
/// `cargo test -p citygen --release overworld::sweep -- --ignored`.
#[cfg(test)]
#[test]
#[ignore]
fn sweep() {
    let mut bad = Vec::new();
    for size in [WorldSize::Small, WorldSize::Medium] {
        for seed in 0..500 {
            let w = Overworld::generate(WorldParams {
                seed,
                size,
                ..Default::default()
            })
            .unwrap();
            if w.rivers.is_empty() {
                bad.push(format!("{size:?} {seed}: no river"));
            }
            if w.cities.len() < 3 {
                bad.push(format!("{size:?} {seed}: {} cities", w.cities.len()));
            }
            let mut reached = vec![false; w.cities.len()];
            reached[0] = true;
            let at = |p: Pos| w.cities.iter().position(|c| c.centre == p);
            loop {
                let before = reached.iter().filter(|r| **r).count();
                for r in &w.routes {
                    if let (Some(a), Some(b)) = (at(r.path[0]), at(*r.path.last().unwrap()))
                        && (reached[a] || reached[b])
                    {
                        reached[a] = true;
                        reached[b] = true;
                    }
                }
                if reached.iter().filter(|r| **r).count() == before {
                    break;
                }
            }
            if !reached.iter().all(|r| *r) {
                bad.push(format!("{size:?} {seed}: a city with no road"));
            }
        }
    }
    assert!(bad.is_empty(), "{} bad worlds: {bad:?}", bad.len());
}

/// What a world is, as one number: every tile, every path, every name. A shared seed
/// must give these exact worlds on every platform; CI runs this on macOS, Linux and
/// Windows. A change to the generator that moves them changes every seed players have
/// shared: update the numbers only for a change meant to do that.
#[cfg(test)]
pub(crate) fn fingerprint(w: &Overworld) -> u64 {
    use crate::rng::hash64;
    let mut h = hash64(w.seed, w.size.w as u64 * 1000 + w.size.h as u64);
    let mut mix = |v: u64| h = hash64(h, v);
    for (_, t) in w.tiles.iter() {
        mix(t.water as u64
            | (t.ground as u64) << 4
            | (t.elevation as u64) << 8
            | (t.road as u64) << 16
            | (t.river as u64) << 20
            | (t.wash as u64) << 56
            | t.urban.map_or(0, |u| {
                1 | (u.city as u64) << 1 | (u.density as u64) << 9 | (u.theme as u64) << 17
            }) << 24);
    }
    let mut text = |s: &str| {
        for b in s.bytes() {
            mix(b as u64);
        }
    };
    for c in &w.cities {
        text(&c.name);
    }
    for r in &w.routes {
        text(&r.name);
    }
    for p in &w.pois {
        text(&p.name);
    }
    for r in &w.regions {
        text(&r.name);
    }
    for c in &w.cities {
        mix((c.centre.x as u64) << 32 | c.centre.y as u32 as u64);
        mix(c.size.to_bits() as u64);
    }
    for path in w
        .routes
        .iter()
        .map(|r| &r.path)
        .chain(&w.rivers)
        .chain(&w.washes)
    {
        for p in path {
            mix((p.x as u64) << 32 | p.y as u32 as u64);
        }
    }
    for p in &w.pois {
        mix((p.at.x as u64) << 32 | p.at.y as u32 as u64);
    }
    h
}

#[cfg(test)]
#[test]
fn fingerprints_are_pinned() {
    let cases: [(&str, WorldParams, u64); 6] = [
        (
            "medium 0",
            WorldParams {
                seed: 0,
                ..Default::default()
            },
            9659042430022885391,
        ),
        (
            "medium 7",
            WorldParams {
                seed: 7,
                ..Default::default()
            },
            13821703677101156480,
        ),
        (
            "small 3",
            WorldParams {
                seed: 3,
                size: WorldSize::Small,
                ..Default::default()
            },
            18208813476832896280,
        ),
        (
            "large 11",
            WorldParams {
                seed: 11,
                size: WorldSize::Large,
                ..Default::default()
            },
            2213363804552518395,
        ),
        (
            "two seas 5",
            WorldParams {
                seed: 5,
                knobs: Knobs {
                    sea_sides: Some(2),
                    ..Default::default()
                },
                ..Default::default()
            },
            1856414223241012311,
        ),
        (
            "inland 2",
            WorldParams {
                seed: 2,
                knobs: Knobs {
                    sea_sides: Some(0),
                    ..Default::default()
                },
                ..Default::default()
            },
            16183058830807330739,
        ),
    ];
    let got: Vec<(&str, u64)> = cases
        .iter()
        .map(|(what, p, _)| (*what, fingerprint(&Overworld::generate(p.clone()).unwrap())))
        .collect();
    let want: Vec<(&str, u64)> = cases.iter().map(|(what, _, f)| (*what, *f)).collect();
    assert_eq!(got, want, "a seed no longer gives the world it did");
}

/// A default world with `knobs` forced, for tests.
#[cfg(test)]
pub(crate) fn gen_with(knobs: Knobs, seed: u64) -> Overworld {
    Overworld::generate(WorldParams {
        seed,
        knobs,
        ..Default::default()
    })
    .unwrap()
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
