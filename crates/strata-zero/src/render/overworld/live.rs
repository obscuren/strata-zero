//! The overworld on foot in motion, as round 2 of `2026-11-05-overworld-motion-mock/`
//! draws it: the atlas look under the clock's light, with the cities' neon (`neon`), the
//! weather (`weather`) and the places' beacons (`beacons`) moving on the frame tick; no
//! traffic or lamps on the roads. A cell is a function of the world, the tile, the tick
//! and the minute of the day: the layers decide from hashes of the cell and the world's
//! seed, keep nothing and save nothing. The tables they read are built once for a world.

use super::{Kind, base_cell, beacons, neon, weather};
use crate::render::style::StylePack;
use crate::render::{Cell, Rgb};
use citygen::Pos;
use citygen::StratumKind;
use citygen::lighting::{daylight, lamps_on, sky};
use citygen::overworld::Overworld;
use citygen::rng::hash64;

/// The moon's cast on the ground and water at night.
const MOON: Rgb = Rgb(64, 84, 128);

/// The hour as the layers read it for how strong things are: day, dusk (or dawn), night.
/// What is lit comes on with the city's lamps (`lighting::lamps_on`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hour {
    Day,
    Dusk,
    Night,
}

impl Hour {
    /// Full sun is day, none is night, and the ramps between are dusk.
    pub(crate) fn of(minute: u16) -> Hour {
        let dl = daylight(minute);
        if dl >= 1.0 {
            Hour::Day
        } else if dl > 0.0 {
            Hour::Dusk
        } else {
            Hour::Night
        }
    }

    /// The one of `[day, dusk, night]` for this hour.
    pub(crate) fn pick<T: Copy>(self, by: [T; 3]) -> T {
        by[self as usize]
    }

    /// Dusk or night: the lights are on.
    pub(crate) fn dark(self) -> bool {
        self != Hour::Day
    }
}

/// The world on foot in motion, built once for a world.
pub struct LiveAtlas {
    neon: neon::Neon,
    weather: weather::Weather,
    beacons: beacons::Beacons,
}

impl LiveAtlas {
    pub fn new(world: &Overworld) -> LiveAtlas {
        LiveAtlas {
            neon: neon::Neon::new(world),
            weather: weather::Weather::new(world),
            beacons: beacons::Beacons::new(world),
        }
    }

    /// Tile `p` at frame tick `tick` and `minute` of the day.
    pub fn cell(
        &self,
        world: &Overworld,
        p: Pos,
        style: &StylePack,
        tick: u64,
        minute: u16,
    ) -> Cell {
        let Some((base, kind)) = base_cell(world, p, style) else {
            return super::atlas_cell(world, p, style);
        };
        let t = world.tile(p).expect("a tile with a cell");
        let (hour, lamps) = (Hour::of(minute), lamps_on(minute));
        let mut c = lit(base, kind, minute);
        self.weather.apply(p, t, kind, tick, hour, &mut c);
        self.neon
            .apply(world, p, t, kind, base.ch, style, tick, hour, lamps, &mut c);
        self.beacons.apply(world, p, tick, lamps, &mut c);
        c
    }

    /// How strong the gust is on tile `p` at `tick`, if its band is over it: for shots
    /// to catch one.
    #[cfg(test)]
    pub(crate) fn gust(&self, p: Pos, tick: u64) -> Option<f32> {
        self.weather.gust(p, tick)
    }
}

/// `c` of kind `kind` under the clock's light at `minute`: by day as it is, dimmer toward
/// night by kind (the ground most, the places least), the ground and water taking the
/// dusk's cast or the moon's.
pub(super) fn lit(mut c: Cell, kind: Kind, minute: u16) -> Cell {
    let dl = daylight(minute);
    if dl >= 1.0 {
        return c;
    }
    let (k0, k1) = match kind {
        Kind::Ground => (0.4, 0.6),
        Kind::Water => (0.55, 0.45),
        Kind::Urban => (0.72, 0.28),
        Kind::Road => (0.5, 0.5),
        Kind::CityRoad => (0.82, 0.18),
        Kind::Place => (0.85, 0.15),
    };
    let f = k0 + k1 * dl;
    c.fg = c.fg.scale(f);
    if matches!(kind, Kind::Ground | Kind::Water) {
        c.fg = match sky(StratumKind::Street, minute) {
            Some(((r, g, b), w)) => c.fg.mix(Rgb(r, g, b).scale(f), w),
            None => c.fg.mix(MOON, 0.10),
        };
        if kind == Kind::Water || c.bg != super::ATLAS_BG {
            c.bg = c.bg.scale(0.7 + 0.3 * dl);
        }
    }
    c
}

/// A layer's own hash key off the world's seed.
pub(super) fn key(world: &Overworld, salt: u64) -> u64 {
    hash64(world.seed, salt)
}

/// A stable hash of two numbers under `key`.
pub(super) fn hash2(key: u64, a: i64, b: i64) -> u64 {
    hash64(key, ((a as u32 as u64) << 32) | b as u32 as u64)
}

/// A stable hash of three numbers under `key`.
pub(super) fn hash3(key: u64, a: i64, b: i64, c: i64) -> u64 {
    hash64(hash2(key, a, b), c as u64)
}

/// A hash as a number in 0..1.
pub(super) fn unit(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// `v` rounded to `n` steps.
pub(super) fn quant(v: f32, n: f32) -> f32 {
    (v * n).round() / n
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use citygen::overworld::WorldParams;

    /// Seed 3's world, as a run makes it.
    pub(in crate::render::overworld) fn world() -> &'static Overworld {
        static WORLD: std::sync::OnceLock<Overworld> = std::sync::OnceLock::new();
        WORLD.get_or_init(|| {
            Overworld::generate(WorldParams {
                seed: 3,
                ..Default::default()
            })
            .unwrap()
        })
    }

    pub(in crate::render::overworld) fn atlas() -> &'static LiveAtlas {
        static ATLAS: std::sync::OnceLock<LiveAtlas> = std::sync::OnceLock::new();
        ATLAS.get_or_init(|| LiveAtlas::new(world()))
    }

    #[test]
    fn by_day_the_light_leaves_the_look_as_it_was() {
        let w = world();
        let style = StylePack::load().unwrap();
        for (p, _) in w.tiles.iter().step_by(7) {
            let (c, kind) = base_cell(w, p, &style).unwrap();
            assert_eq!(lit(c, kind, 13 * 60), c, "{p:?}");
            if kind == Kind::Ground && c.ch != ' ' {
                assert_ne!(lit(c, kind, 22 * 60).fg, c.fg, "{p:?} dims at night");
            }
        }
        assert_eq!(Hour::of(13 * 60), Hour::Day);
        assert_eq!(Hour::of(18 * 60 + 20), Hour::Dusk);
        assert_eq!(Hour::of(22 * 60), Hour::Night);
    }

    #[test]
    fn no_light_on_the_roads() {
        // round 2: no traffic and no lamps on the highways, and no place's light on one
        // beside it; only a checkpoint lights its own road. A road takes the hour's light
        // and the clouds' shade, never anything brighter or another glyph.
        use citygen::overworld::PoiKind;
        let w = world();
        let style = StylePack::load().unwrap();
        let near = |p: Pos, r: i32, f: &dyn Fn(Pos) -> bool| {
            (-r..=r).any(|dy| (-r..=r).any(|dx| f(p.offset(dx, dy))))
        };
        let urban = |q: Pos| w.tile(q).is_some_and(|t| t.urban.is_some());
        let checkpoint = |q: Pos| {
            w.poi_at(q)
                .is_some_and(|poi| poi.kind == PoiKind::Checkpoint)
        };
        let ringed = |q: Pos| {
            w.poi_at(q).is_some_and(|poi| {
                matches!(
                    poi.kind,
                    PoiKind::Relay | PoiKind::TruckStop | PoiKind::Camp
                )
            })
        };
        let (mut roads, mut by_a_place) = (0, 0);
        for (p, t) in w.tiles.iter() {
            if t.road == 0 || w.poi_at(p).is_some() || near(p, 2, &checkpoint) {
                continue;
            }
            // the skyline's spill tints the ground round a city, the roads with it
            if t.urban.is_none() && near(p, 3, &urban) {
                continue;
            }
            let (base, kind) = base_cell(w, p, &style).unwrap();
            let beside = near(p, 1, &ringed);
            roads += 1;
            by_a_place += beside as u32;
            for minute in [13 * 60, 18 * 60 + 20, 22 * 60] {
                let lit = lit(base, kind, minute);
                for tick in (0..420).step_by(if beside { 1 } else { 41 }) {
                    let c = atlas().cell(w, p, &style, tick, minute);
                    assert_eq!((c.ch, c.bold), (lit.ch, lit.bold), "{p:?} at {tick}");
                    let under = |a: Rgb, b: Rgb| a.0 <= b.0 && a.1 <= b.1 && a.2 <= b.2;
                    assert!(
                        under(c.fg, lit.fg) && under(c.bg, lit.bg),
                        "{p:?} at {tick}, {minute}: {c:?} lit past {lit:?}"
                    );
                }
            }
        }
        assert!(roads > 500, "{roads} road tiles");
        assert!(by_a_place > 0, "a road beside a place's light");
    }

    #[test]
    fn the_lights_keep_the_lamps_hours() {
        // the overworld's lights come on with the city's lamps (`lighting::lamps_on`,
        // 18:00 to 05:59): the windows, the night signs, the skyline's spill and the
        // places' after-dark beacons; the daylight keeps only how strong things are
        use crate::render::overworld::ATLAS_BG;
        use citygen::Theme;
        use citygen::overworld::{Ground, PoiKind};
        let (w, a) = (world(), atlas());
        let style = StylePack::load().unwrap();
        const T: u64 = 6000;
        let quiet = |p: Pos, tick: u64| a.weather.shade(p, tick) == 0.0;
        let cell = |p: Pos, tick: u64, minute: u16| a.cell(w, p, &style, tick, minute);
        let base = |p: Pos| base_cell(w, p, &style).unwrap();
        // the night signs: more of the cities' tiles a sign than by day
        let urban: Vec<Pos> = w
            .tiles
            .iter()
            .filter(|(_, t)| t.urban.is_some())
            .map(|(p, _)| p)
            .collect();
        let signs = |minute: u16| {
            urban
                .iter()
                .filter(|p| cell(**p, T, minute).ch == '▪')
                .count()
        };
        let by_day = signs(13 * 60);
        // the spill: plain ground round a city, under no cloud, off its own background
        let near_city =
            |p: Pos| (-3..=3).any(|dy| (-3..=3).any(|dx| urban.contains(&p.offset(dx, dy))));
        let ground: Vec<Pos> = w
            .tiles
            .iter()
            .filter(|(p, t)| {
                t.urban.is_none()
                    && t.road == 0
                    && w.poi_at(*p).is_none()
                    && !matches!(t.ground, Ground::Hills | Ground::Mountains | Ground::Peak)
                    && base(*p).1 == Kind::Ground
                    && base(*p).0.bg == ATLAS_BG
                    && quiet(*p, T)
            })
            .map(|(p, _)| p)
            .filter(|p| near_city(*p))
            .collect();
        let spilt = |minute: u16| {
            ground
                .iter()
                .filter(|p| cell(**p, T, minute).bg != ATLAS_BG)
                .count()
        };
        // the windows: a block's light changes with its 60 s slot; in districts whose
        // breath fits a slot (600 and 900 ticks) nothing else on it does, out of a cloud
        let blocks: Vec<Pos> = urban
            .iter()
            .copied()
            .filter(|p| {
                let t = w.tile(*p).unwrap();
                let (c, kind) = base(*p);
                matches!(t.urban.unwrap().theme, Theme::NeonStrip | Theme::OldTown)
                    && kind == Kind::Urban
                    && matches!(c.ch, '█' | '▓' | '▒')
                    && quiet(*p, T)
                    && quiet(*p, T + 1800)
            })
            .collect();
        let windows = |minute: u16| {
            blocks
                .iter()
                .filter(|p| {
                    let (now, slot_on) = (cell(**p, T, minute), cell(**p, T + 1800, minute));
                    now.ch != '▪' && slot_on.ch != '▪' && now != slot_on
                })
                .count()
        };
        // the places' after-dark beacons
        let lit_places = w
            .pois
            .iter()
            .filter(|poi| {
                matches!(
                    poi.kind,
                    PoiKind::TruckStop | PoiKind::Checkpoint | PoiKind::Camp
                )
            })
            .count();
        let after_dark = |minute: u16| {
            w.pois
                .iter()
                .filter(|poi| {
                    let c = cell(poi.at, T, minute);
                    match poi.kind {
                        PoiKind::TruckStop => c.bg == Rgb(56, 38, 6),
                        PoiKind::Checkpoint => {
                            c.fg == Rgb(255, 40, 72) || c.fg == Rgb(255, 170, 185)
                        }
                        PoiKind::Camp => c.bg != base(poi.at).0.bg,
                        _ => false,
                    }
                })
                .count()
        };
        assert!(!ground.is_empty() && blocks.len() > 20 && lit_places > 3);
        for (minute, on) in [
            (17 * 60 + 30, false),
            (18 * 60, true),
            (5 * 60 + 59, true),
            (6 * 60, false),
        ] {
            let at = format!("{:02}:{:02}", minute / 60, minute % 60);
            assert_eq!(signs(minute) > by_day, on, "night signs at {at}");
            assert_eq!(spilt(minute) > 0, on, "the spill at {at}");
            assert_eq!(windows(minute) > 0, on, "windows at {at}");
            let lit = if on { lit_places } else { 0 };
            assert_eq!(after_dark(minute), lit, "after-dark beacons at {at}");
        }
    }
}
