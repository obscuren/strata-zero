//! Generation parameters.

use crate::geom::Size;
use crate::validate::Violation;
use crate::world::StratumKind;
use serde::{Deserialize, Serialize};

/// Everything that can go wrong while generating a city.
#[derive(Debug, thiserror::Error)]
pub enum GenError {
    #[error("invalid parameter `{field}`: {reason}")]
    InvalidParams { field: &'static str, reason: String },
    #[error("could not connect stratum {stratum:?} after repair rounds")]
    Unconnected { stratum: StratumKind },
    #[error("flavor pack error: {0}")]
    Flavor(String),
    #[error("generated city violates invariants: {0:?}")]
    Invalid(Vec<Violation>),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SizePreset {
    Small,
    Medium,
    Large,
    Custom { w: u32, h: u32 },
}

impl SizePreset {
    pub fn size(self) -> Size {
        match self {
            SizePreset::Small => Size { w: 160, h: 80 },
            SizePreset::Medium => Size { w: 240, h: 120 },
            SizePreset::Large => Size { w: 320, h: 160 },
            SizePreset::Custom { w, h } => Size { w, h },
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SizePreset::Small => "small",
            SizePreset::Medium => "medium",
            SizePreset::Large => "large",
            SizePreset::Custom { .. } => "custom",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaterMode {
    None,
    Bay,
    River,
}

impl WaterMode {
    pub fn label(self) -> &'static str {
        match self {
            WaterMode::None => "none",
            WaterMode::Bay => "bay",
            WaterMode::River => "river",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GenParams {
    pub seed: u64,
    pub size: SizePreset,
    pub districts: u8,
    pub density: f32,
    pub wealth_skew: f32,
    pub decay: f32,
    pub water: WaterMode,
    pub flavor: String,
    /// Where in a world the city stands; none for a lone city. With a site the city's
    /// water, outline and highways are the world's, and `water` is not used.
    #[serde(default)]
    pub site: Option<CitySite>,
}

/// A city's place in a world: the world it is in, which of its cities it is, and the
/// world-wide detail cell its map's top-left corner lies on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CitySite {
    pub world: crate::overworld::WorldParams,
    pub city: u8,
    pub origin: crate::geom::Pos,
    /// One of the city's outlying towns, on a map of its own, rather than the city.
    #[serde(default)]
    pub town: Option<u8>,
}

impl Default for GenParams {
    fn default() -> Self {
        GenParams {
            seed: 0,
            size: SizePreset::Medium,
            districts: 7,
            density: 0.6,
            wealth_skew: 0.0,
            decay: 0.4,
            water: WaterMode::Bay,
            flavor: "default".into(),
            site: None,
        }
    }
}

impl GenParams {
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// The fewest districts the city may have: one on its site in a world (a small
    /// city's or a town's, by its tiles); four for a lone city.
    pub fn least_districts(&self) -> u8 {
        if self.site.is_some() { 1 } else { 4 }
    }

    pub fn validate(&self) -> Result<(), GenError> {
        let bad = |field: &'static str, reason: String| GenError::InvalidParams { field, reason };
        let Size { w, h } = self.size.size();
        if !(80..=1024).contains(&w) || !(40..=512).contains(&h) {
            return Err(bad(
                "size",
                format!("{w}x{h} is outside 80..=1024 by 40..=512"),
            ));
        }
        let least = self.least_districts();
        if !(least..=12).contains(&self.districts) {
            return Err(bad(
                "districts",
                format!("{} is outside {least}..=12", self.districts),
            ));
        }
        if (w as u64) * (h as u64) < self.districts as u64 * 800 {
            return Err(bad(
                "districts",
                format!(
                    "too many districts for map size ({} districts need at least {} tiles, map has {})",
                    self.districts,
                    self.districts as u64 * 800,
                    w as u64 * h as u64
                ),
            ));
        }
        if !(0.0..=1.0).contains(&self.density) {
            return Err(bad("density", format!("{} is outside 0..=1", self.density)));
        }
        if !(-1.0..=1.0).contains(&self.wealth_skew) {
            return Err(bad(
                "wealth_skew",
                format!("{} is outside -1..=1", self.wealth_skew),
            ));
        }
        if !(0.0..=1.0).contains(&self.decay) {
            return Err(bad("decay", format!("{} is outside 0..=1", self.decay)));
        }
        if self.flavor.trim().is_empty() {
            return Err(bad("flavor", "flavor pack name is empty".into()));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_bad_params() {
        let bad = GenParams {
            districts: 13,
            ..Default::default()
        };
        assert!(matches!(
            bad.validate(),
            Err(GenError::InvalidParams {
                field: "districts",
                ..
            })
        ));
        let bad = GenParams {
            size: SizePreset::Custom { w: 80, h: 40 },
            districts: 8,
            ..Default::default()
        };
        assert!(matches!(
            bad.validate(),
            Err(GenError::InvalidParams {
                field: "districts",
                ..
            })
        ));
        let bad = GenParams {
            size: SizePreset::Custom { w: 40, h: 40 },
            ..Default::default()
        };
        assert!(matches!(
            bad.validate(),
            Err(GenError::InvalidParams { field: "size", .. })
        ));
        let bad = GenParams {
            density: 1.5,
            ..Default::default()
        };
        assert!(matches!(
            bad.validate(),
            Err(GenError::InvalidParams {
                field: "density",
                ..
            })
        ));
        assert!(GenParams::default().validate().is_ok());
        assert!(
            GenParams {
                size: SizePreset::Small,
                districts: 12,
                ..Default::default()
            }
            .validate()
            .is_ok()
        );
        assert_eq!(GenParams::default().with_seed(9).seed, 9);
    }
}
