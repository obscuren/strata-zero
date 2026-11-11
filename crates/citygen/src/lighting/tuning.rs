//! Every number behind light and sight, in one place. Tweak here, look with the dev
//! console's `clock hh:mm` and `light`.

use super::Mood;
use crate::interior::RoomKind;
use crate::tile::{Feature, Hue, Terrain, Tone};
use crate::world::BuildingKind;
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
        // a fire in a drum warms the dirt round a shanty
        Some(Feature::FireBarrel) => Some((4, 70.0)),
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

/// The Net's open grid and its lanes, written straight into `steady`.
pub const NET_GRID_LIGHT: u8 = 200;
pub const NET_LANE_LIGHT: u8 = 255;

/// Emitted light (ambient excluded) at or above which a tile in line of sight is seen
/// past the dark radius. A street lamp passes within four tiles, neon within two.
pub const LIT_SIGHT: u8 = 40;
/// How far the player's cast reaches when looking for lit tiles: a city's lamps are
/// seen from the open ground well before you reach it, where nothing stands between.
pub const LIT_RADIUS: i32 = 96;
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

/// What lights a room: its tone, the ambient that tone gives it before mood, and the
/// moods it can take with their weights.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomLight {
    pub tone: Tone,
    pub level: u8,
    pub moods: &'static [(Mood, u32)],
}

/// The light of a room of `room` kind (`None`: floor in no room, lit as a corridor)
/// in a building of `building` kind.
pub fn room_light(room: Option<RoomKind>, building: BuildingKind) -> RoomLight {
    use Mood::*;
    use RoomKind as R;
    let light = |tone, level, moods| RoomLight { tone, level, moods };
    match (room, building) {
        (Some(R::Bar | R::Vip), BuildingKind::Club) => {
            light(Tone::Blacklight, 50, &[(Walls, 1), (Both, 1)])
        }
        (Some(R::Hall), BuildingKind::Temple) => light(Tone::Tungsten, 50, &[(Walls, 1)]),
        (
            Some(
                R::Laundry
                | R::Lockers
                | R::Toilets
                | R::Storage
                | R::Plant
                | R::Server
                | R::Kitchen,
            ),
            _,
        ) => light(Tone::Fluorescent, 92, &[(Ceiling, 1)]),
        (Some(R::Waiting), _) => light(Tone::Fluorescent, 100, &[(Ceiling, 2), (Both, 1)]),
        (Some(R::Lobby), _) => light(Tone::Led, 120, &[(Walls, 1), (Both, 1)]),
        (Some(R::Shop), _) => light(Tone::Led, 120, &[(Walls, 1), (Both, 2)]),
        (Some(R::OpenPlan | R::Office), _) => light(Tone::Led, 108, &[(Ceiling, 3), (Both, 1)]),
        (Some(R::Flat | R::Home | R::HotelRoom | R::Suite), _) => {
            light(Tone::Tungsten, 60, &[(Walls, 3), (Ceiling, 1)])
        }
        (Some(R::BackRoom), _) => light(Tone::Tungsten, 55, &[(Ceiling, 3), (Walls, 1)]),
        (Some(R::Bar | R::Vip), _) => light(Tone::Tungsten, 50, &[(Walls, 1), (Both, 1)]),
        (Some(R::Shrine), _) => light(Tone::Tungsten, 50, &[(Walls, 1)]),
        (Some(R::Surgery), _) => light(Tone::Surgical, 128, &[(Both, 1)]),
        (Some(R::Hall | R::Bays | R::Platform | R::Track | R::CarPark), _) => {
            light(Tone::Sodium, 78, &[(Ceiling, 3), (Both, 1)])
        }
        (Some(R::Stage), _) => light(Tone::Neon(Hue::Magenta), 60, &[(Ceiling, 1)]),
        (Some(R::Corridor) | None, _) => light(Tone::Fluorescent, 92, &[(Ceiling, 3), (Walls, 1)]),
    }
}

/// How a mood scales a room's ambient: walls alone are the cosy room, both the lively.
pub fn mood_factor(m: Mood) -> f32 {
    match m {
        Mood::Walls => 0.75,
        Mood::Ceiling => 1.0,
        Mood::Both => 1.15,
    }
}

/// Fittings' light, as (radius, strength at the centre): a tube's or panel's and a
/// high-bay's strength is shared over its cells.
pub const TUBE: (i32, f32) = (5, 200.0);
pub const TUBE_STUTTER: (i32, f32) = (3, 60.0);
pub const HIGH_BAY: (i32, f32) = (6, 220.0);
pub const BULB: (i32, f32) = (5, 190.0);
pub const PENDANT: (i32, f32) = (3, 150.0);
pub const COUNTER_GLOW: (i32, f32) = (2, 55.0);
pub const SURGICAL_LAMP: (i32, f32) = (3, 210.0);
pub const STRIP: (i32, f32) = (3, 105.0);
pub const STRIP_SURGICAL: (i32, f32) = (3, 130.0);
/// The one sconce by a bed.
pub const SCONCE: (i32, f32) = (5, 190.0);
/// Sconces in a row along a room's walls.
pub const SCONCE_ROW: (i32, f32) = (3, 95.0);
pub const NEON_STRIP: (i32, f32) = (4, 110.0);
pub const STAGE: (i32, f32) = (3, 60.0);
pub const SCREEN_GLOW: (i32, f32) = (3, 90.0);
/// One in this many wearing fittings is dead, and one more stutters.
pub const WEAR_IN: u64 = 9;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fire_barrel_lights_the_dirt_round_it() {
        let e = steady_emission(
            Terrain::Dirt,
            Some(Feature::FireBarrel),
            StratumKind::Street,
        );
        assert!(e.is_some_and(|(r, s)| r >= 3 && s > 40.0));
    }
}
