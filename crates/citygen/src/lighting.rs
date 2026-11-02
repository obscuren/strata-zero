//! Light from emitting tiles, as a pure function over a grid.

pub mod tuning;

use crate::geom::{Grid, Pos, Size};
use crate::tile::{Feature, Terrain, Tile};
use crate::world::StratumKind;
use tuning::*;

/// How much sun there is at `minute` of the day: 0 at night, 1 by day, linear ramps
/// through dawn and dusk.
pub fn daylight(minute: u16) -> f32 {
    let m = minute % (24 * 60);
    let ramp = |from: u16, to: u16| (m - from) as f32 / (to - from) as f32;
    if !(DAWN_START..DUSK_END).contains(&m) {
        0.0
    } else if m < DAWN_END {
        ramp(DAWN_START, DAWN_END)
    } else if m < DUSK_START {
        1.0
    } else {
        1.0 - ramp(DUSK_START, DUSK_END)
    }
}

/// Street lamps burn from the middle of dusk to the middle of dawn: on at 18:00,
/// off at 06:00, the night window the rest of the game keeps.
pub fn lamps_on(minute: u16) -> bool {
    let m = minute % (24 * 60);
    !(DAWN_MID..DUSK_MID).contains(&m)
}

/// Whether the lamps burn on a stratum at `minute`: by the clock under the sky, always
/// where there is none.
pub fn lamps_on_at(k: StratumKind, minute: u16) -> bool {
    !sunlit(k) || lamps_on(minute)
}

/// Whether a stratum sees the sky.
pub fn sunlit(k: StratumKind) -> bool {
    let (night, day) = ambient_range(k);
    day > night
}

/// Ambient light on a stratum at `minute`, between its night and day values.
pub fn ambient(k: StratumKind, minute: u16) -> u8 {
    let (night, day) = ambient_range(k);
    (night as f32 + (day as f32 - night as f32) * daylight(minute)).round() as u8
}

/// The sky's cast on a sunlit stratum: amber through the ramps, pale by day, none at
/// night. Returns the colour and how far to mix toward it.
pub fn sky(k: StratumKind, minute: u16) -> Option<((u8, u8, u8), f32)> {
    if !sunlit(k) {
        return None;
    }
    let t = daylight(minute);
    if t <= 0.0 {
        return None;
    }
    let tent = 1.0 - (2.0 * t - 1.0).abs();
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    let colour = (
        mix(SKY_DUSK.0, SKY_DAY.0),
        mix(SKY_DUSK.1, SKY_DAY.1),
        mix(SKY_DUSK.2, SKY_DAY.2),
    );
    Some((colour, SKY_DUSK_WEIGHT * tent + SKY_DAY_WEIGHT * t))
}

/// Whether a tile glows on its own right now: a steady fixture, or a lamp while on.
pub fn emits(terrain: Terrain, feature: Option<Feature>, k: StratumKind, lamps_on: bool) -> bool {
    steady_emission(terrain, feature, k).is_some() || (lamps_on && lamp_emission(feature).is_some())
}

/// Adds every emitter's cone of `strength` over `radius` tiles to `base`, clamped to
/// 0..=255, reaching only the tiles the emitter can see through `blocks`. A blocking
/// tile takes the light that hits it, so walls and facades glow. `emission` gives
/// (radius, strength) for a tile that glows.
pub fn compute(
    tiles: &Grid<Tile>,
    base: u8,
    blocks: impl Fn(Pos) -> bool,
    emission: impl Fn(&Tile) -> Option<(i32, f32)>,
) -> Grid<u8> {
    let size = tiles.size();
    let mut light = vec![base as f32; size.area()];
    for (p, t) in tiles.iter() {
        let Some((r, strength)) = emission(t) else {
            continue;
        };
        crate::fov::compute(
            p,
            r,
            &blocks,
            |q| size.contains(q),
            |q| {
                let d = (p.dist2(q) as f32).sqrt();
                if d <= r as f32 {
                    light[size.index(q)] += strength * (1.0 - d / r as f32);
                }
            },
        );
    }
    let mut out = Grid::filled(size, 0u8);
    for (slot, v) in out.as_mut_slice().iter_mut().zip(light) {
        *slot = v.clamp(0.0, 255.0) as u8;
    }
    out
}

/// A light on an interior floor: its cone's radius and strength at the centre, and
/// the colour it throws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emitter {
    pub pos: Pos,
    pub radius: i32,
    pub strength: f32,
    pub colour: (u8, u8, u8),
}

/// Coloured light: each tile's ambient from `base`, plus every emitter's cone in its
/// colour, reaching only the tiles it can see through `blocks`. As in `compute`, a
/// blocking tile takes the light that hits it. Each channel is clamped to 0..=255.
pub fn compute_coloured(
    size: Size,
    base: impl Fn(Pos) -> [f32; 3],
    blocks: impl Fn(Pos) -> bool,
    emitters: &[Emitter],
) -> Grid<(u8, u8, u8)> {
    let mut light: Vec<[f32; 3]> = (0..size.area())
        .map(|i| {
            let w = size.w as usize;
            base(Pos::new((i % w) as i32, (i / w) as i32))
        })
        .collect();
    for e in emitters {
        let colour = [e.colour.0, e.colour.1, e.colour.2].map(|c| c as f32 / 255.0);
        crate::fov::compute(
            e.pos,
            e.radius,
            &blocks,
            |q| size.contains(q),
            |q| {
                let d = (e.pos.dist2(q) as f32).sqrt();
                if d <= e.radius as f32 {
                    let k = e.strength * (1.0 - d / e.radius as f32);
                    for (v, c) in light[size.index(q)].iter_mut().zip(colour) {
                        *v += k * c;
                    }
                }
            },
        );
    }
    let mut out = Grid::filled(size, (0u8, 0u8, 0u8));
    for (slot, [r, g, b]) in out.as_mut_slice().iter_mut().zip(light) {
        let c = |v: f32| v.round().clamp(0.0, 255.0) as u8;
        *slot = (c(r), c(g), c(b));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Pos, Size};
    use crate::tile::{Feature, Terrain};

    #[test]
    fn a_lamp_lights_its_surroundings_and_fades_with_distance() {
        let mut tiles = Grid::filled(Size { w: 20, h: 10 }, Tile::new(Terrain::Floor));
        tiles[Pos::new(10, 5)].feature = Some(Feature::Lamp);
        let light = compute(
            &tiles,
            40,
            |_| false,
            |t| match t.feature {
                Some(Feature::Lamp) => Some((4, 120.0)),
                _ => None,
            },
        );
        assert_eq!(light[Pos::new(0, 0)], 40);
        assert!(light[Pos::new(10, 5)] > light[Pos::new(12, 5)]);
        assert!(light[Pos::new(12, 5)] > light[Pos::new(14, 5)]);
        assert_eq!(light[Pos::new(15, 5)], 40);
    }

    use crate::tile::Hue;
    use crate::world::StratumKind;

    #[test]
    fn a_wall_takes_the_light_and_shades_what_stands_behind_it() {
        let mut tiles = Grid::filled(Size { w: 20, h: 10 }, Tile::new(Terrain::Floor));
        tiles[Pos::new(10, 5)].feature = Some(Feature::Lamp);
        for y in 0..10 {
            tiles[Pos::new(12, y)].terrain = Terrain::Wall;
        }
        tiles[Pos::new(12, 5)].terrain = Terrain::Window;
        let light = compute(
            &tiles,
            0,
            |p| tiles[p].blocks_sight(),
            |t| match t.feature {
                Some(Feature::Lamp) => Some((6, 120.0)),
                _ => None,
            },
        );
        assert!(light[Pos::new(12, 3)] > 0, "the wall glows");
        assert_eq!(light[Pos::new(13, 3)], 0, "nothing behind the wall");
        assert!(light[Pos::new(13, 5)] > 0, "the window lets it through");
    }

    #[test]
    fn coloured_light_carries_its_colour_and_stops_at_walls() {
        let size = Size { w: 20, h: 10 };
        let walls: Vec<Pos> = (0..10).map(|y| Pos::new(12, y)).collect();
        let e = Emitter {
            pos: Pos::new(10, 5),
            radius: 6,
            strength: 200.0,
            colour: (255, 168, 88),
        };
        let g = compute_coloured(size, |_| [10.0, 10.0, 10.0], |p| walls.contains(&p), &[e]);
        let near = g[Pos::new(10, 5)];
        assert!(near.0 > near.2, "warm: {near:?}");
        assert_eq!(g[Pos::new(0, 0)], (10, 10, 10));
        assert!(g[Pos::new(12, 5)].0 > 10, "the wall takes the light");
        assert_eq!(g[Pos::new(13, 5)], (10, 10, 10), "nothing behind the wall");
    }

    #[test]
    fn the_ramp_is_night_at_ten_noon_at_twelve_and_half_at_the_edges() {
        assert_eq!(daylight(22 * 60), 0.0);
        assert_eq!(daylight(12 * 60), 1.0);
        assert!((daylight(6 * 60) - 0.5).abs() < 1e-6);
        assert!((daylight(18 * 60) - 0.5).abs() < 1e-6);
        assert!(daylight(5 * 60 + 30) > 0.0 && daylight(5 * 60 + 30) < 0.5);
        assert!(lamps_on(5 * 60 + 59) && !lamps_on(6 * 60));
        assert!(lamps_on(18 * 60) && !lamps_on(17 * 60 + 59));
    }

    #[test]
    fn lamps_burn_all_day_where_there_is_no_sky() {
        assert!(!lamps_on_at(StratumKind::Street, 12 * 60));
        assert!(lamps_on_at(StratumKind::Street, 22 * 60));
        assert!(lamps_on_at(StratumKind::Undercity, 12 * 60));
        assert!(lamps_on_at(StratumKind::Net, 12 * 60));
    }

    #[test]
    fn ambient_at_night_is_the_old_base_and_noon_is_full_sun_above_ground() {
        for (k, base) in [
            (StratumKind::Undercity, 20),
            (StratumKind::Street, 60),
            (StratumKind::Skyways, 80),
            (StratumKind::Heights, 110),
            (StratumKind::Net, 0),
        ] {
            assert_eq!(ambient(k, 22 * 60), base, "{k:?}");
        }
        assert_eq!(ambient(StratumKind::Street, 12 * 60), 255);
        assert_eq!(ambient(StratumKind::Heights, 12 * 60), 255);
        assert_eq!(ambient(StratumKind::Undercity, 12 * 60), 20);
        assert_eq!(ambient(StratumKind::Net, 12 * 60), 0);
        assert!(sunlit(StratumKind::Street) && !sunlit(StratumKind::Undercity));
    }

    #[test]
    fn the_sky_tints_the_day_and_the_dusk_but_not_the_night_or_the_undercity() {
        assert_eq!(sky(StratumKind::Street, 22 * 60), None);
        assert_eq!(sky(StratumKind::Undercity, 12 * 60), None);
        let (day, w) = sky(StratumKind::Street, 12 * 60).unwrap();
        assert_eq!((day, w), (tuning::SKY_DAY, tuning::SKY_DAY_WEIGHT));
        let (dusk, wd) = sky(StratumKind::Street, 18 * 60).unwrap();
        assert!(dusk.0 > dusk.2, "dusk is warm: {dusk:?}");
        assert!(wd > w, "dusk weighs more than noon: {wd} vs {w}");
    }

    #[test]
    fn lamps_emit_only_while_on_and_neon_always() {
        let k = StratumKind::Street;
        assert!(emits(Terrain::Floor, Some(Feature::Lamp), k, true));
        assert!(!emits(Terrain::Floor, Some(Feature::Lamp), k, false));
        assert!(emits(
            Terrain::Floor,
            Some(Feature::Neon(Hue::Cyan)),
            k,
            false
        ));
        assert!(emits(Terrain::Window, None, k, false));
        assert!(!emits(Terrain::Window, None, StratumKind::Heights, false));
        assert!(!emits(Terrain::Floor, None, k, true));
    }
}
