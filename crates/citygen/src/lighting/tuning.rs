//! Every number behind light and sight, in one place. Tweak here, look with the dev
//! console's `clock hh:mm` and `light`.

use crate::tile::{Feature, Hue, Terrain, Tone};
use crate::world::StratumKind;

/// Dawn ramps from 05:00 to 07:00; the midpoint 06:00 is when lamps go off.
pub const DAWN_START: u16 = 5 * 60;
pub const DAWN_END: u16 = 7 * 60;
/// Dusk ramps from 17:00 to 19:00; the midpoint 18:00 is when lamps come on.
pub const DUSK_START: u16 = 17 * 60;
pub const DUSK_END: u16 = 19 * 60;
/// The lamp switch: off at the middle of dawn, on at the middle of dusk.
pub const DAWN_MID: u16 = (DAWN_START + DAWN_END) / 2;
pub const DUSK_MID: u16 = (DUSK_START + DUSK_END) / 2;

/// Ambient light per stratum as (night, day). The Undercity and the Net have no sky.
pub const fn ambient_range(k: StratumKind) -> (u8, u8) {
    match k {
        StratumKind::Undercity => (20, 20),
        StratumKind::Street => (60, 255),
        StratumKind::Skyways => (80, 255),
        StratumKind::Heights => (110, 255),
        StratumKind::Net => (0, 0),
    }
}

/// Fixtures lit all day: (radius, strength at the centre, fading linearly).
pub fn steady_emission(
    terrain: Terrain,
    feature: Option<Feature>,
    k: StratumKind,
) -> Option<(i32, f32)> {
    match feature {
        Some(Feature::Neon(_)) => Some((4, 90.0)),
        Some(Feature::HoloAd) => Some((5, 100.0)),
        Some(Feature::Helipad) => Some((3, 60.0)),
        _ if terrain == Terrain::Window && k == StratumKind::Street => Some((2, 30.0)),
        _ => None,
    }
}

/// Fixtures that switch off by day: (radius, strength).
pub fn lamp_emission(feature: Option<Feature>) -> Option<(i32, f32)> {
    match feature {
        Some(Feature::Lamp) => Some((6, 120.0)),
        _ => None,
    }
}

/// A lamp on an interior floor.
pub const INTERIOR_LAMP: (i32, f32) = (6, 120.0);
/// Base light of an interior floor; floors have no sky.
pub const INTERIOR_BASE: u8 = 150;
/// The Net's open grid and its lanes, written straight into `steady`.
pub const NET_GRID_LIGHT: u8 = 200;
pub const NET_LANE_LIGHT: u8 = 255;

/// Emitted light (ambient excluded) at or above which a tile in line of sight is seen
/// past the dark radius. A street lamp passes within four tiles, neon within two.
pub const LIT_SIGHT: u8 = 40;
/// How far the player's cast reaches when looking for lit tiles.
pub const LIT_RADIUS: i32 = 40;
/// The dark radius at full day on a sunlit stratum: past the viewport's half-diagonal.
pub const DAY_RADIUS: i32 = 60;

/// Exposure words by the light on your tile: below each bound, in order; the last
/// word also covers its bound.
pub const EXPOSURE_BANDS: [(u8, &str); 4] =
    [(40, "dark"), (100, "dim"), (180, "lit"), (255, "bright")];

/// The sky at full day: a pale cool cast, and how much of it.
pub const SKY_DAY: (u8, u8, u8) = (200, 215, 235);
pub const SKY_DAY_WEIGHT: f32 = 0.18;
/// The sky at the middle of dawn and dusk: amber, and how much of it at the peak.
pub const SKY_DUSK: (u8, u8, u8) = (235, 150, 80);
pub const SKY_DUSK_WEIGHT: f32 = 0.25;

/// The colour a tone's light throws.
pub const fn tone_rgb(t: Tone) -> (u8, u8, u8) {
    match t {
        Tone::Fluorescent => (185, 232, 222),
        Tone::Led => (175, 205, 255),
        Tone::Tungsten => (255, 168, 88),
        Tone::Surgical => (228, 242, 255),
        Tone::Sodium => (255, 150, 55),
        Tone::Blacklight => (175, 95, 255),
        Tone::Neon(h) => match h {
            Hue::Cyan => (0, 225, 210),
            Hue::Magenta => (255, 60, 175),
            Hue::Pink => (255, 110, 200),
            Hue::Orange => (255, 140, 40),
            Hue::Yellow => (255, 220, 60),
            Hue::Green => (60, 255, 120),
            Hue::Blue => (60, 120, 255),
            Hue::Red => (255, 50, 60),
            Hue::White => (240, 240, 255),
        },
    }
}

/// A rig's or a terminal's screen glow.
pub const SCREEN: (u8, u8, u8) = (0, 225, 210);
