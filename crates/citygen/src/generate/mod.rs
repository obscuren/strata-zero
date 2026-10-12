//! The resumable generator: eleven chunked stages over a growing `City`.

pub mod arterials;
pub mod districts;
pub mod streets;
pub mod terrain;

use crate::flavor::FlavorPack;
use crate::geom::{Grid, Pos, Rect, Size};
use crate::ids::{BuildingId, DistrictId};
use crate::params::{GenError, GenParams};
use crate::rng::stage_rng;
use crate::tile::{Feature, Terrain, Tile};
use crate::world::{BuildingKind, City, HeightClass, StratumKind};
use rand_chacha::ChaCha8Rng;

/// Generation stages, in execution order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    Terrain,
    Districts,
    Arterials,
    Streets,
    Lots,
    Verticals,
    Interiors,
    Connectivity,
    Dressing,
    Naming,
    Net,
}

impl Stage {
    pub const ALL: [Stage; 11] = [
        Stage::Terrain,
        Stage::Districts,
        Stage::Arterials,
        Stage::Streets,
        Stage::Lots,
        Stage::Verticals,
        Stage::Interiors,
        Stage::Connectivity,
        Stage::Dressing,
        Stage::Naming,
        Stage::Net,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Stage::Terrain => "Shaping terrain",
            Stage::Districts => "Zoning districts",
            Stage::Arterials => "Routing arterials",
            Stage::Streets => "Carving streets",
            Stage::Lots => "Raising buildings",
            Stage::Verticals => "Stacking strata",
            Stage::Interiors => "Fitting interiors",
            Stage::Connectivity => "Linking levels",
            Stage::Dressing => "Hanging neon",
            Stage::Naming => "Naming places",
            Stage::Net => "Wiring the Net",
        }
    }

    pub fn index(self) -> u8 {
        Stage::ALL.iter().position(|s| *s == self).unwrap_or(0) as u8
    }
}

/// What `Generator::step` reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Progress {
    Running { stage: Stage, pct: f32 },
    Done,
}

/// Output of one `StageRunner::step`.
pub(crate) enum StepOut {
    Continue(f32),
    Finished,
}

/// One stage, run as many small steps.
pub(crate) trait StageRunner {
    fn begin(&mut self, ctx: &mut GenCtx) -> Result<(), GenError>;
    fn step(&mut self, ctx: &mut GenCtx) -> Result<StepOut, GenError>;
}

// Scratch types are populated by stages that land in later tasks; the allows go when they do.
/// A rectangular block between streets, owned by one district.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct Block {
    pub rect: Rect,
    pub district: DistrictId,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(crate) enum LotUse {
    Building {
        kind: BuildingKind,
        class: HeightClass,
        basement: bool,
    },
    Plaza,
    Parking,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct Lot {
    pub rect: Rect,
    pub district: DistrictId,
    pub use_: LotUse,
}

/// Intermediate state shared between stages; never serialized.
#[allow(dead_code)]
pub(crate) struct Scratch {
    pub land: Grid<bool>,
    pub wealth: Grid<f32>,
    pub decay: Grid<f32>,
    pub arterial: Grid<bool>,
    pub road: Grid<bool>,
    pub blocks: Vec<Block>,
    pub lots: Vec<Lot>,
    pub lobbies: Vec<(BuildingId, Pos)>,
    pub water_edge: Option<crate::geom::Dir>,
}

impl Scratch {
    fn new(size: Size) -> Self {
        Scratch {
            land: Grid::filled(size, true),
            wealth: Grid::filled(size, 0.5),
            decay: Grid::filled(size, 0.5),
            arterial: Grid::filled(size, false),
            road: Grid::filled(size, false),
            blocks: Vec::new(),
            lots: Vec::new(),
            lobbies: Vec::new(),
            water_edge: None,
        }
    }
}

/// Everything a stage can see and mutate.
#[allow(dead_code)]
pub(crate) struct GenCtx {
    pub city: City,
    pub flavor: FlavorPack,
    pub seed: u64,
    pub scratch: Scratch,
}

#[allow(dead_code)]
impl GenCtx {
    pub fn rng(&self, stage: Stage) -> ChaCha8Rng {
        stage_rng(self.seed, stage.index())
    }

    pub fn size(&self) -> Size {
        self.city.size
    }

    pub fn params(&self) -> &GenParams {
        &self.city.params
    }

    pub fn terrain(&self, k: StratumKind, p: Pos) -> Terrain {
        self.city
            .tile(k, p)
            .map(|t| t.terrain)
            .unwrap_or(Terrain::Void)
    }

    pub fn set_terrain(&mut self, k: StratumKind, p: Pos, t: Terrain) -> bool {
        match self.city.tile_mut(k, p) {
            Some(tile) => {
                tile.terrain = t;
                true
            }
            None => false,
        }
    }

    pub fn set_feature(&mut self, k: StratumKind, p: Pos, f: Option<Feature>) -> bool {
        match self.city.tile_mut(k, p) {
            Some(tile) => {
                tile.feature = f;
                true
            }
            None => false,
        }
    }

    pub fn tile(&self, k: StratumKind, p: Pos) -> Option<&Tile> {
        self.city.tile(k, p)
    }

    pub fn in_district(&self, p: Pos, id: DistrictId) -> bool {
        self.city
            .tile(StratumKind::Street, p)
            .is_some_and(|t| t.district == Some(id))
    }

    pub fn is_land(&self, p: Pos) -> bool {
        self.scratch.land.get(p).copied().unwrap_or(false)
    }
}

/// A stage that does nothing; placeholder until the real stage lands.
struct NoopStage;

impl StageRunner for NoopStage {
    fn begin(&mut self, _ctx: &mut GenCtx) -> Result<(), GenError> {
        Ok(())
    }
    fn step(&mut self, _ctx: &mut GenCtx) -> Result<StepOut, GenError> {
        Ok(StepOut::Finished)
    }
}

pub(crate) fn runner_for(stage: Stage) -> Box<dyn StageRunner> {
    match stage {
        Stage::Terrain => Box::new(terrain::TerrainStage::default()),
        Stage::Districts => Box::new(districts::DistrictsStage::default()),
        Stage::Arterials => Box::new(arterials::ArterialsStage::default()),
        Stage::Streets => Box::new(streets::StreetsStage::default()),
        _ => Box::new(NoopStage),
    }
}

/// Drives the stages one small step at a time so a UI can draw between steps.
pub struct Generator {
    ctx: GenCtx,
    stage_idx: usize,
    runner: Option<Box<dyn StageRunner>>,
    error: Option<GenError>,
    done: bool,
}

impl Generator {
    pub fn new(params: GenParams) -> Result<Self, GenError> {
        params.validate()?;
        let flavor = FlavorPack::load(&params.flavor)?;
        let city = City::empty(params.clone());
        let scratch = Scratch::new(city.size);
        Ok(Generator {
            ctx: GenCtx {
                seed: params.seed,
                city,
                flavor,
                scratch,
            },
            stage_idx: 0,
            runner: None,
            error: None,
            done: false,
        })
    }

    /// Runs one small unit of work.
    pub fn step(&mut self) -> Progress {
        if self.done || self.error.is_some() {
            return Progress::Done;
        }
        let stage = Stage::ALL[self.stage_idx];
        if self.runner.is_none() {
            let mut r = runner_for(stage);
            if let Err(e) = r.begin(&mut self.ctx) {
                self.error = Some(e);
                return Progress::Done;
            }
            self.runner = Some(r);
        }
        let out = self
            .runner
            .as_mut()
            .expect("runner set above")
            .step(&mut self.ctx);
        match out {
            Ok(StepOut::Continue(pct)) => Progress::Running {
                stage,
                pct: pct.clamp(0.0, 1.0),
            },
            Ok(StepOut::Finished) => {
                self.runner = None;
                self.stage_idx += 1;
                if self.stage_idx == Stage::ALL.len() {
                    self.done = true;
                    Progress::Done
                } else {
                    Progress::Running {
                        stage: Stage::ALL[self.stage_idx],
                        pct: 0.0,
                    }
                }
            }
            Err(e) => {
                self.error = Some(e);
                Progress::Done
            }
        }
    }

    /// The city so far; partial while running.
    pub fn city(&self) -> &City {
        &self.ctx.city
    }

    pub fn error(&self) -> Option<&GenError> {
        self.error.as_ref()
    }

    pub fn is_done(&self) -> bool {
        self.done || self.error.is_some()
    }

    /// Validates and returns the finished city.
    pub fn finish(self) -> Result<City, GenError> {
        if let Some(e) = self.error {
            return Err(e);
        }
        self.ctx.city.validate().map_err(GenError::Invalid)?;
        Ok(self.ctx.city)
    }

    /// Runs every stage to completion.
    pub fn run(params: GenParams) -> Result<City, GenError> {
        let mut g = Generator::new(params)?;
        while let Progress::Running { .. } = g.step() {}
        g.finish()
    }

    /// Test helper: run until `stage` has finished and hand back the context.
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn run_until(params: GenParams, stage: Stage) -> GenCtx {
        let mut g = Generator::new(params).expect("valid params");
        loop {
            match g.step() {
                Progress::Running { stage: s, .. } if s.index() > stage.index() => break,
                Progress::Running { .. } => {}
                Progress::Done => break,
            }
        }
        assert!(g.error.is_none(), "{:?}", g.error);
        g.ctx
    }
}

/// Shared fixtures for stage tests.
#[cfg(test)]
pub(crate) mod testutil {
    use super::Generator;
    use crate::params::{GenParams, SizePreset, WaterMode};
    use crate::world::City;

    /// A 120x60 city with the given seed, water mode and district count.
    pub fn small(seed: u64, water: WaterMode, districts: u8) -> City {
        Generator::run(GenParams {
            seed,
            water,
            districts,
            size: SizePreset::Custom { w: 120, h: 60 },
            ..Default::default()
        })
        .unwrap_or_else(|e| panic!("seed {seed}: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{GenParams, SizePreset};

    #[test]
    fn generator_walks_all_stages_in_order() {
        let mut g = Generator::new(GenParams {
            seed: 1,
            size: SizePreset::Custom { w: 80, h: 40 },
            districts: 4,
            ..Default::default()
        })
        .unwrap();
        let mut seen = vec![];
        while let Progress::Running { stage, pct } = g.step() {
            assert!((0.0..=1.0).contains(&pct));
            if seen.last() != Some(&stage) {
                seen.push(stage);
            }
        }
        assert_eq!(seen, Stage::ALL.to_vec());
        assert!(g.error().is_none());
        assert!(g.finish().is_ok());
    }

    #[test]
    fn generator_rejects_invalid_params() {
        assert!(
            Generator::new(GenParams {
                districts: 99,
                ..Default::default()
            })
            .is_err()
        );
    }
}
