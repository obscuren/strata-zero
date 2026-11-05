//! The cities' neon on the overworld, round 2's: each district breathes in its colour on
//! its own slow beat (20 to 36 s a breath, in five steps each way), each tile up to 2 s
//! off it so a step ripples across the district; half the signs stutter about once a
//! minute; after dark the windows change about once a minute, more signs light and the
//! skyline spills onto the ground and water round it; the towers' beacons blink red
//! every 6 s.

use super::live::{Hour, hash2, hash3, key, unit};
use super::{Kind, roll};
use crate::anim::effects::pulse;
use crate::render::style::StylePack;
use crate::render::{Cell, Rgb};
use citygen::overworld::{Overworld, Water, WorldTile};
use citygen::{Grid, Pos, Theme};

/// A window keeps its light for 60 s, the tiles staggered, lit 4 times in 5.
const WINDOW_SLOT: u64 = 1800;
const WINDOW_LIT: f32 = 0.8;
/// A sign that stutters does so once every 60 to 90 s, for 0.4 s.
const SIGN_CYCLE: u64 = 1800;
const SIGN_SPREAD: u64 = 900;
const SIGN_STUTTER: u64 = 12;
/// A tower's beacon: on 6 ticks in every 180.
const TOWER_EVERY: u64 = 180;
const TOWER_ON: u64 = 6;
const TOWER: Rgb = Rgb(255, 40, 60);
/// Water by a city catches its light: a 1 in 25 chance a second.
const GLINT_EVERY: u64 = 30;
const GLINT_CHANCE: f32 = 0.04;
/// How far a city's light spills onto the ground round it, in tiles.
const SPILL: i32 = 3;
/// How far apart the towers stand, in tiles.
const TOWERS_APART: i32 = 5;
/// A tile is up to this many ticks off its district's beat.
const RIPPLE: u64 = 60;

/// A district's breath, in ticks.
fn period(theme: Theme) -> u64 {
    match theme {
        Theme::CorpCore => 720,
        Theme::NeonStrip => 600,
        Theme::NightMarket => 660,
        Theme::Megablocks => 960,
        Theme::Industrial => 1080,
        Theme::Docks => 840,
        Theme::OldTown => 900,
        Theme::CombatZone => 780,
    }
}

/// Where in its breath a district starts, in ticks.
fn phase(theme: Theme) -> u64 {
    match theme {
        Theme::CorpCore => 0,
        Theme::NeonStrip => 230,
        Theme::NightMarket => 460,
        Theme::Megablocks => 690,
        Theme::Industrial => 920,
        Theme::Docks => 1150,
        Theme::OldTown => 1380,
        Theme::CombatZone => 1610,
    }
}

pub(super) struct Neon {
    /// Each tile off the cities within `SPILL` of one: how far, and the nearest city
    /// tile's district.
    near: Grid<Option<(u8, Theme)>>,
    /// The towers: each city's three densest tiles and each town's one, spaced apart.
    towers: Grid<bool>,
    /// Hash keys off the world's seed, one for each thing decided.
    ripple: u64,
    windows: u64,
    lit: u64,
    signs: u64,
    tower: u64,
    glint: u64,
}

impl Neon {
    pub(super) fn new(world: &Overworld) -> Neon {
        Neon {
            near: near_cities(world),
            towers: towers(world),
            ripple: key(world, 0x4E_07),
            windows: key(world, 0x4E_05),
            lit: key(world, 0x4E_17),
            signs: key(world, 0x4E_09),
            tower: key(world, 0x4E_03),
            glint: key(world, 0x4E_33),
        }
    }

    /// The neon over tile `p` (`t`, drawn as `kind`, `base` its glyph by day) at `tick`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply(
        &self,
        world: &Overworld,
        p: Pos,
        t: &WorldTile,
        kind: Kind,
        base: char,
        style: &StylePack,
        tick: u64,
        hour: Hour,
        c: &mut Cell,
    ) {
        let accent = |theme: Theme| style.theme(theme).accent;
        if let (Some(u), Kind::Urban) = (t.urban, kind) {
            let v = u.density as f32 / 255.0;
            let acc = accent(u.theme);
            let g =
                hour.pick([0.10, 0.18, 0.24]) * (0.35 + 0.65 * v) * self.breath(u.theme, p, tick);
            c.fg = c.fg.mix(acc, g);
            if hour.dark() {
                c.bg = c.bg.mix(acc, g * 0.3);
            }
            if hour.dark() && matches!(c.ch, '█' | '▓' | '▒') {
                let slot = (tick + hash2(self.windows, p.x as i64, p.y as i64) % WINDOW_SLOT)
                    / WINDOW_SLOT;
                let on = unit(hash3(self.lit, p.x as i64, p.y as i64, slot as i64)) < WINDOW_LIT;
                c.fg = if on {
                    c.fg.mix(acc.mix(Rgb(255, 230, 180), 0.5), 0.16)
                } else {
                    c.fg.scale(0.82)
                };
            }
            let r = roll(world, p);
            if base == '▪' || (hour.dark() && v > 0.3 && r > 0.86 && r <= 0.92) {
                let h = hash2(self.signs, p.x as i64, p.y as i64);
                let cycle = SIGN_CYCLE + (h >> 1) % SIGN_SPREAD;
                let at = (tick + (h >> 8) % cycle) % cycle;
                let stutters = h & 1 == 0 && at < SIGN_STUTTER;
                let on = !stutters || !((tick >> 1) + h % 3).is_multiple_of(3);
                c.ch = '▪';
                c.fg = match (on, hour) {
                    (false, _) => acc.scale(0.22),
                    (true, Hour::Day) => acc.scale(0.7),
                    (true, _) => acc,
                };
                if on && hour.dark() {
                    c.bg = c.bg.mix(acc, 0.16);
                }
            }
        } else if hour.dark() {
            let near = self.near[p];
            if let Some((d, theme)) = near {
                let k = hour.pick([0.0, 0.07, 0.13])
                    * [1.0, 0.55, 0.27][d as usize - 1]
                    * (0.8 + 0.2 * self.breath(theme, p, tick));
                c.bg = c.bg.mix(accent(theme), k);
            }
            let wet = t.water != Water::None
                && !matches!(kind, Kind::Road | Kind::CityRoad | Kind::Place);
            let by = near.map(|(_, theme)| theme).or(t.urban.map(|u| u.theme));
            if let Some(theme) = by.filter(|_| wet) {
                let slot = (tick / GLINT_EVERY) as i64;
                if unit(hash3(self.glint, p.x as i64, p.y as i64, slot)) < GLINT_CHANCE {
                    if t.water != Water::River {
                        c.ch = '~';
                    }
                    c.fg = accent(theme)
                        .mix(Rgb(255, 255, 255), 0.15)
                        .scale(hour.pick([0.0, 0.55, 0.75]));
                }
            }
        }
        if self.towers[p]
            && (tick + hash2(self.tower, p.x as i64, p.y as i64) % TOWER_EVERY) % TOWER_EVERY
                < TOWER_ON
        {
            c.ch = '▪';
            c.fg = TOWER;
            c.bold = true;
            if hour.dark() {
                c.bg = c.bg.mix(TOWER, 0.22);
            }
        }
    }

    /// The step of its district's breath tile `p` is on at `tick`, 0 to 5.
    pub(super) fn step(&self, theme: Theme, p: Pos, tick: u64) -> u8 {
        let off = hash2(self.ripple, p.x as i64, p.y as i64) % RIPPLE;
        (pulse(tick + phase(theme) + off, period(theme)) * 5.0).round() as u8
    }

    /// How bright its district's breath has tile `p` at `tick`, 0 to 1 in five steps.
    fn breath(&self, theme: Theme, p: Pos, tick: u64) -> f32 {
        self.step(theme, p, tick) as f32 / 5.0
    }
}

/// For each tile off the cities, the nearest city tile within `SPILL` (the first of the
/// nearest, reading the rows round it top to bottom) and its district.
fn near_cities(world: &Overworld) -> Grid<Option<(u8, Theme)>> {
    let side = 2 * SPILL + 1;
    // the best (distance, place in the reading) and its district, stamped from each city tile
    let mut best: Grid<Option<(i32, i32, Theme)>> = Grid::filled(world.size, None);
    for (u, t) in world.tiles.iter() {
        let Some(urban) = t.urban else { continue };
        for oy in -SPILL..=SPILL {
            for ox in -SPILL..=SPILL {
                let p = u.offset(ox, oy);
                if world.tile(p).is_none_or(|t| t.urban.is_some()) {
                    continue;
                }
                let d = ox.abs().max(oy.abs());
                let read = (SPILL - oy) * side + (SPILL - ox);
                if best[p].is_none_or(|(bd, br, _)| (d, read) < (bd, br)) {
                    best[p] = Some((d, read, urban.theme));
                }
            }
        }
    }
    best.map(|b| b.map(|(d, _, theme)| (d as u8, theme)))
}

/// The towers: each city's three densest tiles off its roads and water and each town's
/// one, the denser first, each `TOWERS_APART` from any other.
fn towers(world: &Overworld) -> Grid<bool> {
    let mut picked: Vec<Pos> = Vec::new();
    let mut settlements = Vec::new();
    for (i, city) in world.cities.iter().enumerate() {
        settlements.push((world.city_tiles(i as u8), 3));
        for t in 0..city.towns.len() {
            settlements.push((world.town_tiles(i as u8, t as u8), 1));
        }
    }
    for (tiles, want) in settlements {
        let mut cand: Vec<(Pos, u8, f32)> = tiles
            .into_iter()
            .filter_map(|p| {
                let t = world.tile(p)?;
                let u = t.urban?;
                (t.road == 0 && t.water == Water::None).then(|| (p, u.density, roll(world, p)))
            })
            .collect();
        cand.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.total_cmp(&b.2)));
        let mut mine = 0;
        for (p, _, _) in cand {
            if mine == want {
                break;
            }
            if picked.iter().any(|q| q.chebyshev(p) < TOWERS_APART) {
                continue;
            }
            picked.push(p);
            mine += 1;
        }
    }
    let mut out = Grid::filled(world.size, false);
    for p in picked {
        out[p] = true;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::overworld::live::tests::world;

    #[test]
    fn a_districts_breath_is_slow() {
        // round 2: a district breathes once in 20 to 36 s in five steps each way, so a
        // tile's step changes every 2 to 3.6 s on the mean, and never twice in a second
        let w = world();
        let neon = Neon::new(w);
        const TICKS: u64 = 4320;
        let mut tiles = 0;
        for (p, t) in w.tiles.iter().step_by(3) {
            let Some(u) = t.urban else { continue };
            tiles += 1;
            let mut last = neon.step(u.theme, p, 0);
            let mut changes = Vec::new();
            for tick in 1..=TICKS {
                let s = neon.step(u.theme, p, tick);
                if s != last {
                    changes.push(tick);
                    last = s;
                }
            }
            assert!(
                changes.windows(2).all(|k| k[1] - k[0] >= 30),
                "{p:?} {:?}: {changes:?}",
                u.theme
            );
            let mean = TICKS as f32 / changes.len().max(1) as f32;
            assert!(
                (55.0..=115.0).contains(&mean),
                "{p:?} {:?}: a step every {mean} ticks",
                u.theme
            );
        }
        assert!(tiles > 100, "{tiles} city tiles");
    }

    #[test]
    fn the_towers_stand_on_each_settlements_densest_tiles() {
        let w = world();
        let neon = Neon::new(w);
        let towers: Vec<Pos> = neon
            .towers
            .iter()
            .filter(|(_, on)| **on)
            .map(|(p, _)| p)
            .collect();
        assert!(towers.len() >= w.cities.len(), "{towers:?}");
        for (i, p) in towers.iter().enumerate() {
            let t = w.tile(*p).unwrap();
            assert!(t.urban.is_some() && t.road == 0 && t.water == Water::None);
            assert!(
                towers[i + 1..]
                    .iter()
                    .all(|q| q.chebyshev(*p) >= TOWERS_APART),
                "{p:?} spaced"
            );
        }
    }
}
