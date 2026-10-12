//! Generation parameters.

use crate::geom::Size;
use serde::{Deserialize, Serialize};

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
        }
    }
}
