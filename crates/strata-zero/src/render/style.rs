//! The style pack: palettes and glyph sets loaded from `flavor/style.ron`.

use crate::render::Rgb;
use anyhow::{Context, Result};
use citygen::{Hue, StratumKind, Theme};
use serde::{Deserialize, Serialize};

const DEFAULT_STYLE: &str = include_str!("../../../../flavor/style.ron");

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThemeStyle {
    pub accent: Rgb,
    pub accent2: Rgb,
    pub wall: Rgb,
    pub floor: Rgb,
    pub road: Rgb,
    pub sidewalk: Rgb,
    pub roof: Rgb,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StratumStyle {
    pub tint: (i16, i16, i16),
    pub fog: bool,
    /// 0 none, 1 light, 2 medium, 3 heavy.
    pub rain: u8,
}

/// How a device looks and speaks: a keypad or a terminal in a theme.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeviceStyle {
    /// Who built it: CIVITAS, HALCYON, LOTUS, ZENBO.
    pub maker: String,
    /// The keypad's model, after the maker's name.
    pub keypad: String,
    /// The terminal's model, after the maker's name.
    pub terminal: String,
    /// The HELIX/OS version this hardware runs.
    pub os_version: String,
    /// The badge's letters alternate the district's two accents.
    #[serde(default)]
    pub alternate: bool,
    pub body: Rgb,
    /// The edge; `None` takes the district's accent.
    pub edge: Option<Rgb>,
    pub glass: Rgb,
    /// Live text on the glass; `None` is the game's cyan.
    pub text: Option<Rgb>,
    pub text_dim: Rgb,
    pub keys_lit: bool,
    pub scanlines: bool,
    /// The edge has gaps.
    pub cracked: bool,
    /// Dead cells drawn on the unit.
    pub dead: u8,
    /// Ticks between flickers of the glass; 0 never.
    pub flicker: u64,
    /// Characters a second a reply types.
    pub cps: u64,
    /// The boot, `{PLACE}` for the district or building.
    pub boot: Vec<String>,
    /// A first line on refusals.
    pub denied: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlyphSets {
    pub holo: Vec<char>,
    pub rain: Vec<char>,
    pub water: Vec<char>,
    pub steam: Vec<char>,
    pub mass: Vec<char>,
    pub net: Vec<char>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StylePack {
    pub background: Rgb,
    pub neutral: ThemeStyle,
    pub themes: Vec<(Theme, ThemeStyle)>,
    pub strata: Vec<(StratumKind, StratumStyle)>,
    pub hues: Vec<(Hue, Rgb)>,
    pub glyphs: GlyphSets,
    /// Device themes by name: municipal, megacorp, neon, street.
    #[serde(default)]
    pub devices: Vec<(String, DeviceStyle)>,
}

impl StylePack {
    /// Loads the embedded default pack.
    pub fn load() -> Result<StylePack> {
        let pack: StylePack = ron::from_str(DEFAULT_STYLE).context("parsing flavor/style.ron")?;
        for set in [
            &pack.glyphs.holo,
            &pack.glyphs.rain,
            &pack.glyphs.water,
            &pack.glyphs.steam,
            &pack.glyphs.mass,
            &pack.glyphs.net,
        ] {
            anyhow::ensure!(!set.is_empty(), "style.ron: a glyph set is empty");
        }
        Ok(pack)
    }

    /// A device theme by name.
    pub fn device(&self, name: &str) -> Option<&DeviceStyle> {
        self.devices.iter().find(|(n, _)| n == name).map(|(_, d)| d)
    }

    pub fn theme(&self, t: Theme) -> &ThemeStyle {
        self.themes
            .iter()
            .find(|(k, _)| *k == t)
            .map(|(_, s)| s)
            .unwrap_or(&self.neutral)
    }

    pub fn theme_or_neutral(&self, t: Option<Theme>) -> &ThemeStyle {
        match t {
            Some(t) => self.theme(t),
            None => &self.neutral,
        }
    }

    pub fn stratum(&self, k: StratumKind) -> &StratumStyle {
        static FALLBACK: StratumStyle = StratumStyle {
            tint: (0, 0, 0),
            fog: false,
            rain: 0,
        };
        self.strata
            .iter()
            .find(|(s, _)| *s == k)
            .map(|(_, s)| s)
            .unwrap_or(&FALLBACK)
    }

    pub fn hue(&self, h: Hue) -> Rgb {
        self.hues
            .iter()
            .find(|(k, _)| *k == h)
            .map(|(_, c)| *c)
            .unwrap_or(Rgb(255, 255, 255))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{Hue, StratumKind, Theme};

    #[test]
    fn style_pack_loads_and_covers_every_theme() {
        let s = StylePack::load().unwrap();
        for t in Theme::ALL {
            let _ = s.theme(t);
        }
        for k in StratumKind::PHYSICAL {
            let _ = s.stratum(k);
        }
        assert_ne!(s.hue(Hue::Magenta), s.hue(Hue::Cyan));
        assert!(s.glyphs.holo.len() >= 8 && s.glyphs.rain.len() >= 4);
        assert_eq!(s.stratum(StratumKind::Undercity).rain, 0);
        assert!(s.stratum(StratumKind::Heights).rain >= 3);
    }
}
