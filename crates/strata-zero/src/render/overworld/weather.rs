//! The weather on the overworld, round 2's, without rain: swells roll in toward the
//! coast and break on the shore, a lake shivers, glints run down the rivers, the wind
//! combs the fields and cloud shadows drift over it all a tile at a time; at dusk and at
//! night the sea glints, amber or the moon's grey. Every 40 to 70 s a gust runs through
//! each range of hills and mountains, west to east: the mountains whiten, the hills take
//! dust, a lake ripples.

use super::Kind;
use super::live::{Hour, hash2, hash3, key, unit};
use crate::render::{Cell, Rgb};
use citygen::overworld::{Ground, Overworld, Water, WorldTile};
use citygen::{Dir, Grid, Pos};
use std::collections::VecDeque;

/// A swell rolls a tile a second toward the coast; one in six stretches of three carries one.
const SWELL_TICKS: u64 = 30;
const SWELL_CHANCE: f32 = 0.17;
/// The sea glints after dark: a 1 in 40 chance a tile every 7 ticks.
const SEA_GLINT_TICKS: u64 = 7;
const SEA_GLINT_CHANCE: f32 = 0.025;
/// A lake shivers: 3 tiles in 10 every 20 ticks.
const LAKE_TICKS: u64 = 20;
const LAKE_CHANCE: f32 = 0.3;
/// Glints run down a river a tile per 10 ticks, 3 slots in 10 carrying one.
const RIVER_TICKS: u64 = 10;
const RIVER_GLINT: f32 = 0.3;
/// The wind over the fields: a band 3 tiles wide every 34, a tile per 6 ticks.
const FIELD_TICKS: u64 = 6;
const FIELD_EVERY: i64 = 34;
const FIELD_BAND: i64 = 3;
/// Cloud shadows: a tile east per 45 ticks, a tile north per 300, each row and column
/// stepping on its own tick.
const CLOUD_EAST: u64 = 45;
const CLOUD_NORTH: u64 = 300;
/// A range's gusts: one in each stretch of 1650 ticks, starting up to 450 into it, so
/// 1200 to 2100 ticks (40 to 70 s) apart.
const GUST_EVERY: u64 = 1650;
const GUST_JITTER: u64 = 450;
/// The gust's front: a tile per 3 ticks, leaning 0.6 of a tile a row, its band five
/// tiles deep and fading back from the front.
const GUST_TICKS: f32 = 3.0;
const GUST_TILT: f32 = 0.6;
const GUST_BAND: [f32; 5] = [1.0, 0.75, 0.5, 0.3, 0.15];
/// Hills and mountains this near each other are one range.
const GUST_REACH: i32 = 2;
/// A sea tile's bits in `Weather::sea`: its way to land (an index of `Dir::CARDINAL`),
/// and whether it is on the shore; `NOT_SEA` off it.
const SHORE: u8 = 4;
const NOT_SEA: u8 = 0xFF;

pub(super) struct Weather {
    /// Each sea tile's way to the nearest land and whether it is on the shore.
    sea: Grid<u8>,
    /// Each river tile's river and how far down it, from its spring.
    river: Grid<Option<(u16, u32)>>,
    /// Each tile of a range: which, and how far back from where its gust's front starts.
    gust: Grid<Option<(u16, f32)>>,
    /// Each range's gust: how long its front takes to cross, in ticks.
    spans: Vec<u64>,
    /// Hash keys off the world's seed, one for each thing decided.
    lanes: u64,
    swells: u64,
    sea_glint: u64,
    lake: u64,
    lake_lanes: u64,
    glints: u64,
    gusts: u64,
    cloud_rows: u64,
    cloud_cols: u64,
    cloud_big: u64,
    cloud_small: u64,
}

impl Weather {
    pub(super) fn new(world: &Overworld) -> Weather {
        let mut river = Grid::filled(world.size, None);
        for (r, course) in world.rivers.iter().enumerate() {
            for (k, p) in course.iter().enumerate() {
                if world.tile(*p).is_some() && river[*p].is_none() {
                    river[*p] = Some((r as u16, k as u32));
                }
            }
        }
        let (gust, spans) = ranges(world);
        Weather {
            sea: seas(world),
            river,
            gust,
            spans,
            lanes: key(world, 0x5E_30),
            swells: key(world, 0x5E_77),
            sea_glint: key(world, 0x5E_01),
            lake: key(world, 0x5E_02),
            lake_lanes: key(world, 0x5E_22),
            glints: key(world, 0x5E_404),
            gusts: key(world, 0x5E_55),
            cloud_rows: key(world, 0x5E_45),
            cloud_cols: key(world, 0x5E_46),
            cloud_big: key(world, 0x5E_91),
            cloud_small: key(world, 0x5E_92),
        }
    }

    /// The weather over tile `p` (`t`, drawn as `kind`) at `tick`.
    pub(super) fn apply(
        &self,
        p: Pos,
        t: &WorldTile,
        kind: Kind,
        tick: u64,
        hour: Hour,
        c: &mut Cell,
    ) {
        let (x, y) = (p.x as i64, p.y as i64);
        let night = hour == Hour::Night;
        if kind == Kind::Water {
            match t.water {
                Water::Sea => {
                    let bits = self.sea[p];
                    let dir = bits & 3;
                    if bits & SHORE != 0 {
                        // the shore breaks as a swell comes in a tile out to sea
                        let out = p.step(Dir::CARDINAL[dir as usize].opposite());
                        if self.swell(out, dir, tick) {
                            c.ch = '≈';
                            c.fg = c.fg.mix(Rgb(150, 200, 240), 0.5);
                        }
                    } else {
                        c.ch = if self.swell(p, dir, tick) { '~' } else { ' ' };
                        let slot = (tick / SEA_GLINT_TICKS) as i64;
                        if hour.dark() && unit(hash3(self.sea_glint, x, y, slot)) < SEA_GLINT_CHANCE
                        {
                            c.ch = '~';
                            c.fg = if night {
                                Rgb(120, 140, 180)
                            } else {
                                Rgb(200, 130, 70)
                            };
                        }
                    }
                }
                Water::Lake => {
                    let off = hash2(self.lake_lanes, x, y) % LAKE_TICKS;
                    let slot = ((tick + off) / LAKE_TICKS) as i64;
                    if unit(hash3(self.lake, x, y, slot)) < LAKE_CHANCE {
                        c.ch = '~';
                        c.fg = c.fg.mix(Rgb(120, 170, 220), 0.3);
                    }
                }
                Water::River => {
                    if let Some((r, k)) = self.river[p] {
                        let at = (tick / RIVER_TICKS) as i64 - k as i64;
                        let glint =
                            |slot: i64| unit(hash2(self.glints, slot, r as i64)) < RIVER_GLINT;
                        if glint(at) {
                            c.fg = if night {
                                Rgb(120, 170, 220)
                            } else {
                                Rgb(170, 220, 255)
                            };
                            c.bold = true;
                        } else if glint(at - 1) {
                            let to = if night {
                                Rgb(100, 150, 200)
                            } else {
                                Rgb(120, 190, 245)
                            };
                            c.fg = c.fg.mix(to, 0.6);
                        }
                    }
                }
                Water::None => {}
            }
        }
        // the wind through the fields
        if kind == Kind::Ground
            && c.ch == '▬'
            && (x + y.div_euclid(2) - (tick / FIELD_TICKS) as i64).rem_euclid(FIELD_EVERY)
                < FIELD_BAND
        {
            let to = if night {
                Rgb(80, 110, 70)
            } else {
                Rgb(150, 200, 110)
            };
            c.fg = c.fg.mix(to, 0.35);
        }
        // now and then a gust through the hills and mountains
        if matches!(kind, Kind::Ground | Kind::Water)
            && t.water != Water::River
            && let Some(s) = self.gust(p, tick)
        {
            let k = hour.pick([1.0, 0.85, 0.7]);
            if t.water == Water::Lake {
                c.ch = '~';
                c.fg = c.fg.mix(Rgb(150, 200, 240).scale(k), 0.5 * s);
            } else if matches!(t.ground, Ground::Mountains | Ground::Peak) {
                c.fg = c.fg.mix(Rgb(235, 240, 250).scale(k), 0.5 * s);
                c.bg = c.bg.mix(Rgb(60, 66, 80).scale(k), 0.25 * s);
            } else if t.ground == Ground::Hills {
                c.fg = c.fg.mix(Rgb(205, 175, 125).scale(k), 0.45 * s);
                c.bg = c.bg.mix(Rgb(44, 36, 22).scale(k), 0.25 * s);
            }
        }
        // cloud shadows, over all but the places
        if kind != Kind::Place {
            let shade = self.shade(p, tick);
            if shade > 0.0 {
                let k = if night { shade * 0.6 } else { shade };
                c.fg = c.fg.scale(1.0 - k);
                c.bg = c.bg.scale(1.0 - k * 0.5);
            }
        }
    }

    /// Whether a swell rolling `dir` (toward land, an index of `Dir::CARDINAL`) is on
    /// `p` at `tick`: crests lying along the rows, two tiles in each three, each row (or
    /// column of three) stepping on its own tick, so the sea never moves all at once.
    fn swell(&self, p: Pos, dir: u8, tick: u64) -> bool {
        let (x, y) = (p.x as i64, p.y as i64);
        // rolling east or west each row is a lane, north or south each column of three
        let by_rows = dir % 2 == 1;
        let (along, across, toward) = match Dir::CARDINAL[dir as usize] {
            Dir::E => (x, y, 1),
            Dir::W => (x, y, -1),
            Dir::S => (y, x, 1),
            _ => (y, x, -1),
        };
        let lane = if by_rows {
            across
        } else {
            across.div_euclid(3)
        };
        let off = hash2(self.lanes, lane, dir as i64) % SWELL_TICKS;
        let u = toward * along - ((tick + off) / SWELL_TICKS) as i64;
        let (crest, cell) = if by_rows {
            (hash3(self.swells, u.div_euclid(3), across, dir as i64), u)
        } else {
            (hash3(self.swells, lane, u, dir as i64), across)
        };
        cell.rem_euclid(3) != 2 && unit(crest) < SWELL_CHANCE
    }

    /// How strong the gust is on tile `p` at `tick`, if its band is over it.
    pub(super) fn gust(&self, p: Pos, tick: u64) -> Option<f32> {
        let (r, u) = (*self.gust.get(p)?)?;
        let d = self.front(r, tick)? - u;
        (0.0..GUST_BAND.len() as f32)
            .contains(&d)
            .then(|| GUST_BAND[d as usize])
    }

    /// How far range `r`'s gust has come at `tick`, in tiles from its west end, while
    /// one is crossing it.
    fn front(&self, r: u16, tick: u64) -> Option<f32> {
        let span = self.spans[r as usize];
        let now = tick / GUST_EVERY;
        (0..=span / GUST_EVERY + 1)
            .filter_map(|back| now.checked_sub(back))
            .find_map(|k| {
                let start = k * GUST_EVERY + hash2(self.gusts, r as i64, k as i64) % GUST_JITTER;
                (start <= tick && tick < start + span).then(|| (tick - start) as f32 / GUST_TICKS)
            })
    }

    /// How deep the cloud's shadow over tile `p` is at `tick`: 0, 0.25 or 0.45.
    pub(super) fn shade(&self, p: Pos, tick: u64) -> f32 {
        let (x, y) = (p.x as i64, p.y as i64);
        let east = (tick + hash2(self.cloud_rows, y, 0) % CLOUD_EAST) / CLOUD_EAST;
        let north = (tick + hash2(self.cloud_cols, x, 0) % CLOUD_NORTH) / CLOUD_NORTH;
        let (cx, cy) = ((x - east as i64) as f32, (y + north as i64) as f32);
        let s = 0.65 * noise(cx / 10.0, cy / 4.5, self.cloud_big)
            + 0.35 * noise(cx / 4.5, cy / 2.2, self.cloud_small);
        if s > 0.66 {
            0.45
        } else if s > 0.58 {
            0.25
        } else {
            0.0
        }
    }

    /// Each range's tiles (the hills, mountains and lakes a gust runs through together).
    #[cfg(test)]
    pub(super) fn ranges(&self) -> Vec<Vec<Pos>> {
        let mut out = vec![Vec::new(); self.spans.len()];
        for (p, g) in self.gust.iter() {
            if let Some((r, _)) = g {
                out[*r as usize].push(p);
            }
        }
        out
    }
}

/// Smooth value noise on the unit lattice under `key`, in 0..1.
fn noise(x: f32, y: f32, key: u64) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (x0, y0) = (x0 as i64, y0 as i64);
    let at = |a: i64, b: i64| unit(hash2(key, a, b));
    let top = at(x0, y0) + (at(x0 + 1, y0) - at(x0, y0)) * sx;
    let low = at(x0, y0 + 1) + (at(x0 + 1, y0 + 1) - at(x0, y0 + 1)) * sx;
    top + (low - top) * sy
}

/// Each sea tile's way toward the nearest land, as an index of `Dir::CARDINAL` along
/// the longer leg to it, with `SHORE` where land touches it; `NOT_SEA` elsewhere.
fn seas(world: &Overworld) -> Grid<u8> {
    let sea = |p: Pos| world.tile(p).is_some_and(|t| t.water == Water::Sea);
    let land = |p: Pos| world.tile(p).is_some_and(|t| t.water != Water::Sea);
    // the nearest land, walked out over the sea from every shore
    let mut nearest: Grid<Option<Pos>> = Grid::filled(world.size, None);
    let mut queue = VecDeque::new();
    for (p, t) in world.tiles.iter() {
        if t.water == Water::Sea
            && let Some(q) = p.neighbors4().find(|q| land(*q))
        {
            nearest[p] = Some(q);
            queue.push_back(p);
        }
    }
    while let Some(p) = queue.pop_front() {
        for q in p.neighbors4() {
            if sea(q) && nearest[q].is_none() {
                nearest[q] = nearest[p];
                queue.push_back(q);
            }
        }
    }
    let mut out = Grid::filled(world.size, NOT_SEA);
    for (p, t) in world.tiles.iter() {
        if t.water != Water::Sea {
            continue;
        }
        let to = nearest[p].map_or(Dir::E, |l| {
            let (dx, dy) = (l.x - p.x, l.y - p.y);
            match (dx.abs() >= dy.abs(), dx > 0, dy > 0) {
                (true, true, _) => Dir::E,
                (true, false, _) => Dir::W,
                (false, _, true) => Dir::S,
                (false, _, false) => Dir::N,
            }
        });
        let dir = Dir::CARDINAL.iter().position(|d| *d == to).unwrap() as u8;
        let shore = p.neighbors8().any(land);
        out[p] = dir | if shore { SHORE } else { 0 };
    }
    out
}

/// The ranges a gust runs through: the hills, mountains and lakes, those within
/// `GUST_REACH` of each other one range. For each of their tiles, which range and how
/// far it lies back from where the front starts (the range's west end, the front
/// leaning across its rows); for each range, how long the front takes to cross it.
fn ranges(world: &Overworld) -> (Grid<Option<(u16, f32)>>, Vec<u64>) {
    let windy = |p: Pos| {
        world.tile(p).is_some_and(|t| {
            t.water != Water::Sea
                && (t.water == Water::Lake
                    || matches!(t.ground, Ground::Hills | Ground::Mountains | Ground::Peak))
        })
    };
    let mut seen = Grid::filled(world.size, false);
    let mut out = Grid::filled(world.size, None);
    let mut spans = Vec::new();
    for (from, _) in world.tiles.iter() {
        if seen[from] || !windy(from) {
            continue;
        }
        seen[from] = true;
        let mut tiles = vec![from];
        let mut k = 0;
        while k < tiles.len() {
            let p = tiles[k];
            k += 1;
            for dy in -GUST_REACH..=GUST_REACH {
                for dx in -GUST_REACH..=GUST_REACH {
                    let q = p.offset(dx, dy);
                    if world.tile(q).is_some() && !seen[q] && windy(q) {
                        seen[q] = true;
                        tiles.push(q);
                    }
                }
            }
        }
        let (y0, y1) = tiles
            .iter()
            .fold((i32::MAX, i32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
        let mid = (y0 + y1) as f32 / 2.0;
        let at = |p: &Pos| p.x as f32 + GUST_TILT * (p.y as f32 - mid);
        let (lo, hi) = tiles
            .iter()
            .map(at)
            .fold((f32::MAX, f32::MIN), |(a, b), u| (a.min(u), b.max(u)));
        let r = spans.len() as u16;
        for p in &tiles {
            out[*p] = Some((r, at(p) - lo));
        }
        spans.push(((hi - lo + GUST_BAND.len() as f32) * GUST_TICKS).ceil() as u64);
    }
    (out, spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::overworld::live::tests::{atlas, world};
    use crate::render::style::StylePack;

    #[test]
    fn the_gust_comes_and_goes() {
        // round 2: every 40 to 70 s a gust runs through a range west to east, a tile per
        // 3 ticks in a 5-tile band, and the hills are quiet between
        let w = world();
        let weather = Weather::new(w);
        let ranges = weather.ranges();
        let range = ranges.iter().max_by_key(|r| r.len()).unwrap();
        assert!(
            range.len() > 100,
            "seed 3's world has a range: {}",
            range.len()
        );
        let gusty = |tick: u64| range.iter().any(|p| weather.gust(*p, tick).is_some());
        const FROM: u64 = 100_000;
        const SEVENTY: u64 = 70 * 30;
        let quiet = (FROM..FROM + SEVENTY).filter(|t| !gusty(*t)).count() as u64;
        assert!(
            quiet * 2 > SEVENTY,
            "most ticks have none: {quiet} quiet of {SEVENTY}"
        );
        let start = (FROM..FROM + SEVENTY)
            .find(|t| gusty(*t) && !gusty(t - 1))
            .expect("a gust within 70 s");
        let end = (start..).find(|t| !gusty(*t)).unwrap();
        assert!(
            (90..=900).contains(&(end - start)),
            "{} ticks across the range",
            end - start
        );
        // west to east, over every tile of the range
        let mean_x = |tick: u64| {
            let xs: Vec<i32> = range
                .iter()
                .filter(|p| weather.gust(**p, tick).is_some())
                .map(|p| p.x)
                .collect();
            xs.iter().sum::<i32>() as f32 / xs.len() as f32
        };
        assert!(mean_x(start) < mean_x(end - 1), "west to east");
        for p in range {
            let ticks = (start..end)
                .filter(|t| weather.gust(*p, *t).is_some())
                .count();
            assert!(
                (14..=16).contains(&ticks),
                "{p:?} in the band {ticks} ticks"
            );
        }
        // a mountain whitens as the band goes over it
        let style = StylePack::load().unwrap();
        let (p, at) = range
            .iter()
            .filter(|p| {
                let t = w.tile(**p).unwrap();
                matches!(t.ground, Ground::Mountains | Ground::Peak)
                    && t.road == 0
                    && t.urban.is_none()
                    && t.water == Water::None
                    && w.poi_at(**p).is_none()
            })
            .find_map(|p| {
                let at = (start..end).find(|t| weather.gust(*p, *t) == Some(1.0))?;
                (weather.shade(*p, at) == weather.shade(*p, at + 30)).then_some((*p, at))
            })
            .expect("a mountain the band goes over, under the same cloud after");
        let (gust, after) = (
            atlas().cell(w, p, &style, at, 13 * 60),
            atlas().cell(w, p, &style, at + 30, 13 * 60),
        );
        let sum = |c: Cell| c.fg.0 as u32 + c.fg.1 as u32 + c.fg.2 as u32;
        assert!(sum(gust) > sum(after), "{gust:?} whiter than {after:?}");
    }

    #[test]
    fn swells_roll_toward_the_land() {
        // a crest on a deep sea tile is a tile nearer the land 30 ticks on
        let w = world();
        let weather = Weather::new(w);
        let mut rolled = 0;
        for (p, bits) in weather.sea.iter().step_by(5) {
            if *bits == NOT_SEA || bits & SHORE != 0 {
                continue;
            }
            let dir = bits & 3;
            let ahead = p.step(Dir::CARDINAL[dir as usize]);
            for t in 9000..9000 + SWELL_TICKS {
                if weather.swell(p, dir, t) && !weather.swell(p, dir, t + 1) {
                    assert!(weather.swell(ahead, dir, t + 1), "{p:?} at {t}");
                    rolled += 1;
                }
            }
        }
        assert!(rolled > 50, "{rolled} crests rolled in");
    }
}
