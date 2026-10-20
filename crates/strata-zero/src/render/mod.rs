//! Rendering: colours, cells, the render context and the tileset.

pub mod atrium;
pub mod fx;
pub mod ghost;
pub mod glyphs;
pub mod items;
pub mod lighting;
pub mod map;
pub mod mark;
pub mod memmap;
pub mod minimap;
pub mod overlay;
pub mod path;
pub mod people;
pub mod style;
pub mod tileset;
pub mod turns;
pub mod viewport;
pub mod weather;

use citygen::{BuildingKind, ConnectorKind, StratumKind, Theme};
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize};
use style::StylePack;

/// A truecolor value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn to_color(self) -> Color {
        Color::Rgb(self.0, self.1, self.2)
    }

    /// Multiplies every channel, saturating at 255.
    pub fn scale(self, f: f32) -> Rgb {
        let s = |c: u8| ((c as f32) * f).round().clamp(0.0, 255.0) as u8;
        Rgb(s(self.0), s(self.1), s(self.2))
    }

    /// Linear blend toward `other` by `t` in 0..=1.
    pub fn mix(self, other: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let m = |a: u8, b: u8| ((a as f32) + ((b as f32) - (a as f32)) * t).round() as u8;
        Rgb(m(self.0, other.0), m(self.1, other.1), m(self.2, other.2))
    }

    /// Adds a signed offset per channel, clamped.
    pub fn tint(self, dr: i16, dg: i16, db: i16) -> Rgb {
        let t = |c: u8, d: i16| (c as i16 + d).clamp(0, 255) as u8;
        Rgb(t(self.0, dr), t(self.1, dg), t(self.2, db))
    }
}

/// One rendered terminal cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Rgb,
    pub bg: Rgb,
    pub bold: bool,
    pub dim: bool,
}

impl Cell {
    pub fn write(&self, buf: &mut Buffer, x: u16, y: u16) {
        if x < buf.area.x || y < buf.area.y || x >= buf.area.right() || y >= buf.area.bottom() {
            return;
        }
        let mut style = Style::default()
            .fg(self.fg.to_color())
            .bg(self.bg.to_color());
        if self.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        if self.dim {
            style = style.add_modifier(Modifier::DIM);
        }
        buf[(x, y)].set_char(self.ch).set_style(style);
    }
}

/// What the walker knows about a tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    Unseen,
    Remembered,
    Visible,
}

/// A connector under the tile and which way it leads from the current stratum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnDir {
    pub kind: ConnectorKind,
    pub up: bool,
    pub down: bool,
}

/// Everything `tileset::cell` needs besides the tile itself.
pub struct RenderCtx<'a> {
    pub style: &'a StylePack,
    pub theme: Option<Theme>,
    pub stratum: StratumKind,
    pub light: u8,
    pub tick: u64,
    pub vis: Visibility,
    pub connector: Option<ConnDir>,
    pub kind: Option<BuildingKind>,
    /// Bits N=1, E=2, S=4, W=8: neighbours that connect to this tile (see `tileset::connects`).
    pub wall_mask: u8,
    /// Whether street lamps burn; a lamp that is off is lit like any other tile.
    pub lamps_on: bool,
    /// The sky's cast and its weight, on a sunlit stratum by day.
    pub sky: Option<(Rgb, f32)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_math() {
        assert_eq!(Rgb(100, 200, 50).scale(0.5), Rgb(50, 100, 25));
        assert_eq!(Rgb(100, 100, 100).scale(10.0), Rgb(255, 255, 255));
        assert_eq!(Rgb(0, 0, 0).mix(Rgb(100, 200, 50), 0.5), Rgb(50, 100, 25));
        assert_eq!(Rgb(10, 10, 10).tint(-20, 5, 300), Rgb(0, 15, 255));
    }
}
