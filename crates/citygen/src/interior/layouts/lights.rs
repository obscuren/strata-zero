//! Interior light: fittings placed per walled part of each room, by the room's tone and
//! its place's mood, and the emitters the floor is lit from.

use super::Builder;
use crate::geom::{Grid, Pos};
use crate::interior::floor::{Role, RoomKind};
use crate::lighting::tuning::{self, tone_rgb};
use crate::lighting::{Emitter, Mood, room_mood};
use crate::rng::hash64;
use crate::tile::{Feature, Fitting, FittingKind, Furniture, Hue, Terrain, Tone, Wear};
use crate::world::BuildingKind;
use std::collections::{HashSet, VecDeque};

/// A piece of one room that walls and doors cut off from the rest of it (a flat's
/// bedroom, its hall), or of the floor outside every room.
struct Part {
    room: Option<usize>,
    tiles: Vec<Pos>,
}

/// A wall tile beside a part, with the part's tile in front of it.
#[derive(Clone, Copy)]
struct Side {
    wall: Pos,
    inward: Pos,
}

impl Side {
    /// The wall's position counted along its own line.
    fn run(self) -> i32 {
        if self.inward.x == self.wall.x {
            self.wall.x
        } else {
            self.wall.y
        }
    }

    /// Which line the wall lies on, for grouping a strip's runs.
    fn line(self) -> (i32, i32, i32) {
        let (dx, dy) = (self.inward.x - self.wall.x, self.inward.y - self.wall.y);
        if dx == 0 {
            (dx, dy, self.wall.y)
        } else {
            (dx, dy, self.wall.x)
        }
    }
}

/// A part's tiles count as floor for light: the floor, grates and every link's landing.
fn part_terrain(t: Terrain) -> bool {
    matches!(
        t,
        Terrain::Floor
            | Terrain::Grate
            | Terrain::Stair
            | Terrain::Lift
            | Terrain::Ladder
            | Terrain::Escalator
    )
}

fn fitting(kind: FittingKind, tone: Tone, wear: Wear) -> Feature {
    Feature::Fitting(Fitting { kind, tone, wear })
}

fn emitter(pos: Pos, (radius, strength): (i32, f32), colour: (u8, u8, u8)) -> Emitter {
    Emitter {
        pos,
        radius,
        strength,
        colour,
    }
}

/// Where a part's tubes would hang, first tile of each, rows then columns: a part up to
/// 7 wide gets one column with the tube centred (two cells from 5 wide), a wider one a
/// column every 6 from its left + 2; a part up to 4 high gets its middle row, a higher one
/// a row every 4 from its top + 2. Only spots inside the part.
fn tube_spots(tiles: &[Pos]) -> Vec<Pos> {
    let (Some(x0), Some(x1)) = (
        tiles.iter().map(|p| p.x).min(),
        tiles.iter().map(|p| p.x).max(),
    ) else {
        return Vec::new();
    };
    let y0 = tiles.iter().map(|p| p.y).min().unwrap_or(0);
    let y1 = tiles.iter().map(|p| p.y).max().unwrap_or(0);
    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
    let long = if w >= 5 { 2 } else { 1 };
    let cols: Vec<i32> = if w <= 7 {
        vec![x0 + (w - long) / 2]
    } else {
        (x0 + 2..x1).step_by(6).collect()
    };
    let rows: Vec<i32> = if h <= 4 {
        vec![y0 + h / 2]
    } else {
        (y0 + 2..y1).step_by(4).collect()
    };
    let set: HashSet<Pos> = tiles.iter().copied().collect();
    rows.iter()
        .flat_map(|&y| cols.iter().map(move |&x| Pos::new(x, y)))
        .filter(|p| set.contains(p))
        .collect()
}

/// The first tile of every tube a part hangs: each free `tube_spots` spot, then every
/// free tile more than a grid step from all of those, in reading order.
fn tube_plan(tiles: &[Pos], free: impl Fn(Pos) -> bool) -> Vec<Pos> {
    let mut placed: Vec<Pos> = tube_spots(tiles).into_iter().filter(|p| free(*p)).collect();
    let mut rest = tiles.to_vec();
    rest.sort_by_key(|p| (p.y, p.x));
    for p in rest {
        if free(p) && !near_a_tube(&placed, p) {
            placed.push(p);
        }
    }
    placed
}

/// Whether `p` lies within one grid step (6 columns, 4 rows) of a tube already hung, so
/// the filling pass leaves it to that tube.
fn near_a_tube(tubes: &[Pos], p: Pos) -> bool {
    tubes
        .iter()
        .any(|q| (q.x - p.x).abs() <= 6 && (q.y - p.y).abs() <= 4)
}

fn centroid(tiles: &[Pos]) -> (f32, f32) {
    let n = tiles.len().max(1) as f32;
    let x = tiles.iter().map(|p| p.x as f32).sum::<f32>() / n;
    let y = tiles.iter().map(|p| p.y as f32).sum::<f32>() / n;
    (x, y)
}

fn dist2(p: Pos, (x, y): (f32, f32)) -> f32 {
    (p.x as f32 - x).powi(2) + (p.y as f32 - y).powi(2)
}

impl Builder<'_> {
    /// Places every part's fittings by its room's tone and mood, and sets the ambient
    /// and the emitters `finish` lights the floor from.
    pub(super) fn light(&mut self) {
        let parts = self.parts();
        let corridor = self.ambient_rgb(None, 0);
        let mut in_part = Grid::filled(self.plan.size, false);
        for part in &parts {
            let kind = part.room.map(|i| self.rooms[i].kind);
            let number = part.room.map_or(0, |i| self.rooms[i].number);
            let colour = self.ambient_rgb(kind, number);
            for p in &part.tiles {
                self.ambient[*p] = colour;
                in_part[*p] = true;
            }
            self.light_part(part, kind, number);
        }
        // Walls, doors, windows and the rest take their brightest floor neighbour's
        // ambient, else the corridor's.
        let size = self.plan.size;
        for y in 0..size.h as i32 {
            for x in 0..size.w as i32 {
                let p = Pos::new(x, y);
                if in_part[p] {
                    continue;
                }
                let mut best: Option<[f32; 3]> = None;
                for n in p.neighbors8().filter(|n| size.contains(*n) && in_part[*n]) {
                    let a = self.ambient[n];
                    best = Some(match best {
                        Some(b) => [b[0].max(a[0]), b[1].max(a[1]), b[2].max(a[2])],
                        None => a,
                    });
                }
                self.ambient[p] = best.unwrap_or(corridor);
            }
        }
        for (p, t) in self.tiles.iter() {
            if t.feature == Some(Feature::Terminal) {
                self.emitters
                    .push(emitter(p, tuning::SCREEN_GLOW, tuning::SCREEN));
            }
        }
    }

    /// The room that owns each tile: the smallest whose rect holds it.
    fn owners(&self) -> Grid<Option<usize>> {
        let mut out = Grid::filled(self.plan.size, None);
        let mut by_area: Vec<usize> = (0..self.rooms.len()).collect();
        by_area.sort_by_key(|&i| std::cmp::Reverse(self.rooms[i].rect.area()));
        for i in by_area {
            for p in self.rooms[i].rect.iter() {
                if self.plan.size.contains(p) {
                    out[p] = Some(i);
                }
            }
        }
        out
    }

    /// The floor cut into parts: one room's floor tiles joined across open floor only.
    fn parts(&self) -> Vec<Part> {
        let owners = self.owners();
        let size = self.plan.size;
        let mut seen = Grid::filled(size, false);
        let mut out = Vec::new();
        for y in 0..size.h as i32 {
            for x in 0..size.w as i32 {
                let start = Pos::new(x, y);
                if seen[start] || !part_terrain(self.tiles[start].terrain) {
                    continue;
                }
                let room = owners[start];
                let mut tiles = Vec::new();
                let mut queue = VecDeque::from([start]);
                seen[start] = true;
                while let Some(p) = queue.pop_front() {
                    tiles.push(p);
                    for n in p.neighbors4() {
                        if size.contains(n)
                            && !seen[n]
                            && owners[n] == room
                            && part_terrain(self.tiles[n].terrain)
                        {
                            seen[n] = true;
                            queue.push_back(n);
                        }
                    }
                }
                out.push(Part { room, tiles });
            }
        }
        out
    }

    fn room_mood(&self, kind: Option<RoomKind>, number: u16) -> (tuning::RoomLight, Mood) {
        let light = tuning::room_light(kind, self.plan.kind);
        let mood = room_mood(
            self.city.seed,
            self.plan.building,
            kind,
            number,
            light.moods,
        );
        (light, mood)
    }

    fn ambient_rgb(&self, kind: Option<RoomKind>, number: u16) -> [f32; 3] {
        let (light, mood) = self.room_mood(kind, number);
        let k = light.level as f32 * tuning::mood_factor(mood) / 255.0;
        let (r, g, b) = tone_rgb(light.tone);
        [r as f32 * k, g as f32 * k, b as f32 * k]
    }

    /// A tile a ceiling fitting can hang over: open floor that nobody stands on.
    fn hangs_free(&self, p: Pos) -> bool {
        self.open(p) && !self.stations.iter().any(|s| s.pos == p)
    }

    /// The bare wall tiles beside a part, in reading order.
    fn sides(&self, part: &Part) -> Vec<Side> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for &p in &part.tiles {
            for wall in p.neighbors4() {
                if self.plan.size.contains(wall)
                    && self.tiles[wall].terrain == Terrain::Wall
                    && self.tiles[wall].feature.is_none()
                    && seen.insert(wall)
                {
                    out.push(Side { wall, inward: p });
                }
            }
        }
        out.sort_by_key(|s| (s.wall.y, s.wall.x));
        out
    }

    /// Wear by a hash of the floor seed and `key`, for the tones that wear.
    fn wear(&self, kind: FittingKind, tone: Tone, key: u64) -> Wear {
        if !kind.wears() || !matches!(tone, Tone::Fluorescent | Tone::Sodium) {
            return Wear::Ok;
        }
        match hash64(self.seed, key) % tuning::WEAR_IN {
            0 => Wear::Dead,
            1 => Wear::Stutter,
            _ => Wear::Ok,
        }
    }

    fn light_part(&mut self, part: &Part, kind: Option<RoomKind>, number: u16) {
        if part.tiles.len() < 2 && kind != Some(RoomKind::Stage) {
            return;
        }
        let (light, mood) = self.room_mood(kind, number);
        let tone = light.tone;
        let has = |b: &Self, f: Furniture| -> Vec<Pos> {
            part.tiles
                .iter()
                .copied()
                .filter(|p| b.tiles[*p].feature == Some(Feature::Furniture(f)))
                .collect()
        };
        let tables = has(self, Furniture::Table);
        let beds = has(self, Furniture::Bed);
        let chairs = has(self, Furniture::Chair);
        // A bar: a counter with staff behind it. Its whole counter takes the glow and
        // the neon behind it; a kitchen counter makes no bar.
        let counter = has(self, Furniture::Counter);
        let bar = counter.iter().any(|c| {
            c.neighbors4().any(|n| {
                self.stations
                    .iter()
                    .any(|s| s.pos == n && s.role == Role::Bar)
            })
        });
        let counters: Vec<Pos> = if bar { counter } else { Vec::new() };
        let before = self.emitters.len();
        if kind == Some(RoomKind::Stage) {
            for &p in &part.tiles {
                if let Some(Feature::Fitting(f)) = self.tiles[p].feature
                    && (p.x + p.y) % 3 == 0
                {
                    self.emitters
                        .push(emitter(p, tuning::STAGE, tone_rgb(f.tone)));
                }
            }
            return;
        }
        let cold = matches!(
            tone,
            Tone::Fluorescent | Tone::Led | Tone::Sodium | Tone::Surgical
        );
        if mood.ceiling() {
            if cold {
                self.tubes(part, tone);
            } else if !counters.is_empty() {
                let colour = tone_rgb(tone);
                for &t in &tables {
                    self.emitters.push(emitter(t, tuning::PENDANT, colour));
                }
                for &c in &counters {
                    self.emitters.push(emitter(c, tuning::COUNTER_GLOW, colour));
                }
            } else {
                let at = if beds.is_empty() {
                    centroid(&part.tiles)
                } else {
                    centroid(&beds)
                };
                self.bulb(part, at, tone);
            }
            for &c in &chairs {
                self.emitters
                    .push(emitter(c, tuning::SURGICAL_LAMP, tone_rgb(Tone::Surgical)));
            }
        }
        if mood.walls() {
            if cold {
                self.strips(part, tone);
            } else if !counters.is_empty() {
                self.backbar(part, &counters);
                self.sconce_row(part, tone);
            } else if !beds.is_empty() {
                self.sconce(part, centroid(&beds), tone);
            } else if !self.sconce_row(part, tone) {
                self.sconce(part, centroid(&part.tiles), tone);
            }
        }
        // A part of some size the rules left dark gets one fitting of its tone, where
        // its mood would put one first.
        if self.emitters.len() == before && part.tiles.len() >= 4 {
            let mid = centroid(&part.tiles);
            let walls_first = mood.walls();
            let first = if walls_first {
                self.one_on_wall(part, mid, tone)
            } else {
                self.one_overhead(part, mid, tone)
            };
            if !first && walls_first {
                self.one_overhead(part, mid, tone);
            } else if !first {
                self.one_on_wall(part, mid, tone);
            }
        }
    }

    /// Tubes along the rows: on `tube_spots` first, then wherever the part still lies
    /// more than a grid step from a tube. Two cells where the row runs 5
    /// wide or more.
    fn tubes(&mut self, part: &Part, tone: Tone) {
        let kind = match tone {
            Tone::Fluorescent => FittingKind::Tube,
            Tone::Sodium => FittingKind::HighBay,
            _ => FittingKind::Panel,
        };
        let set: HashSet<Pos> = part.tiles.iter().copied().collect();
        for p in tube_plan(&part.tiles, |p| self.hangs_free(p)) {
            let run = (0..)
                .map(|i| p.offset(i, 0))
                .take_while(|q| set.contains(q))
                .count()
                + (1..)
                    .map(|i| p.offset(-i, 0))
                    .take_while(|q| set.contains(q))
                    .count();
            let right = p.offset(1, 0);
            let cells = if run >= 5 && set.contains(&right) && self.hangs_free(right) {
                vec![p, right]
            } else {
                vec![p]
            };
            let size = self.plan.size;
            let wear = self.wear(kind, tone, size.index(p) as u64);
            let (base, colour) = match (kind, wear) {
                (FittingKind::HighBay, _) => (tuning::HIGH_BAY, tone_rgb(tone)),
                (_, Wear::Stutter) => (tuning::TUBE_STUTTER, tone_rgb(tone)),
                _ => (tuning::TUBE, tone_rgb(tone)),
            };
            let share = (base.0, base.1 / cells.len() as f32);
            for q in cells {
                self.tiles[q].feature = Some(fitting(kind, tone, wear));
                if wear != Wear::Dead {
                    self.emitters.push(emitter(q, share, colour));
                }
            }
        }
    }

    /// The tone's own single fitting overhead, on the free tile nearest `at`: a bulb
    /// in a warm room, one cell of tube, panel or high-bay in a cold one.
    fn one_overhead(&mut self, part: &Part, at: (f32, f32), tone: Tone) -> bool {
        let kind = match tone {
            Tone::Fluorescent => FittingKind::Tube,
            Tone::Sodium => FittingKind::HighBay,
            Tone::Led | Tone::Surgical => FittingKind::Panel,
            _ => return self.bulb(part, at, tone),
        };
        let Some(p) = part
            .tiles
            .iter()
            .copied()
            .filter(|p| self.hangs_free(*p))
            .min_by(|a, b| dist2(*a, at).total_cmp(&dist2(*b, at)))
        else {
            return false;
        };
        let wear = self.wear(kind, tone, self.plan.size.index(p) as u64);
        self.tiles[p].feature = Some(fitting(kind, tone, wear));
        if wear != Wear::Dead {
            let light = if wear == Wear::Stutter {
                tuning::TUBE_STUTTER
            } else {
                tuning::TUBE
            };
            self.emitters.push(emitter(p, light, tone_rgb(tone)));
        }
        true
    }

    /// The tone's own single wall fitting nearest `at`: a sconce in a warm room, a strip
    /// in a cold one.
    fn one_on_wall(&mut self, part: &Part, at: (f32, f32), tone: Tone) -> bool {
        if !matches!(
            tone,
            Tone::Fluorescent | Tone::Led | Tone::Sodium | Tone::Surgical
        ) {
            return self.sconce(part, at, tone);
        }
        let Some(side) = self
            .sides(part)
            .into_iter()
            .min_by(|a, b| dist2(a.wall, at).total_cmp(&dist2(b.wall, at)))
        else {
            return false;
        };
        self.wall_fitting(side, FittingKind::Strip, tone, Wear::Ok, tuning::STRIP);
        true
    }

    /// One bulb on the free tile nearest `at`; false when there is none.
    fn bulb(&mut self, part: &Part, at: (f32, f32), tone: Tone) -> bool {
        let Some(p) = part
            .tiles
            .iter()
            .copied()
            .filter(|p| self.hangs_free(*p))
            .min_by(|a, b| dist2(*a, at).total_cmp(&dist2(*b, at)))
        else {
            return false;
        };
        self.tiles[p].feature = Some(fitting(FittingKind::Bulb, tone, Wear::Ok));
        self.emitters.push(emitter(p, tuning::BULB, tone_rgb(tone)));
        true
    }

    /// Puts a wall fitting in `side` and its light on the tile in front of it.
    fn wall_fitting(
        &mut self,
        side: Side,
        kind: FittingKind,
        tone: Tone,
        wear: Wear,
        light: (i32, f32),
    ) {
        self.tiles[side.wall].feature = Some(fitting(kind, tone, wear));
        if wear != Wear::Dead {
            let light = if wear == Wear::Stutter {
                tuning::TUBE_STUTTER
            } else {
                light
            };
            self.emitters
                .push(emitter(side.inward, light, tone_rgb(tone)));
        }
    }

    /// Strips in runs of three, three dark between; a surgery's walls lit whole.
    fn strips(&mut self, part: &Part, tone: Tone) {
        let whole = tone == Tone::Surgical;
        let light = if whole {
            tuning::STRIP_SURGICAL
        } else {
            tuning::STRIP
        };
        for side in self.sides(part) {
            if !whole && (side.run() / 3) % 2 != 0 {
                continue;
            }
            let (dx, dy, line) = side.line();
            let key = hash64(
                hash64((dx + 2) as u64 * 4 + (dy + 2) as u64, line as u64),
                (side.run() / 3) as u64,
            );
            let wear = self.wear(FittingKind::Strip, tone, key);
            self.wall_fitting(side, FittingKind::Strip, tone, wear, light);
        }
    }

    /// Neon in every wall tile within two of a bar counter: magenta, cyan in a club.
    fn backbar(&mut self, part: &Part, counters: &[Pos]) {
        let hue = if self.plan.kind == BuildingKind::Club {
            Hue::Cyan
        } else {
            Hue::Magenta
        };
        for side in self.sides(part) {
            if counters.iter().any(|c| c.manhattan(side.wall) <= 2) {
                self.wall_fitting(
                    side,
                    FittingKind::NeonStrip,
                    Tone::Neon(hue),
                    Wear::Ok,
                    tuning::NEON_STRIP,
                );
            }
        }
    }

    /// A sconce every five tiles along the part's walls; false when none fits.
    fn sconce_row(&mut self, part: &Part, tone: Tone) -> bool {
        let mut any = false;
        for side in self.sides(part) {
            if side.run().rem_euclid(5) == 2 {
                self.wall_fitting(
                    side,
                    FittingKind::Sconce,
                    tone,
                    Wear::Ok,
                    tuning::SCONCE_ROW,
                );
                any = true;
            }
        }
        any
    }

    /// One sconce in the wall tile nearest `at`; false when the part has no bare wall.
    fn sconce(&mut self, part: &Part, at: (f32, f32), tone: Tone) -> bool {
        let Some(side) = self
            .sides(part)
            .into_iter()
            .min_by(|a, b| dist2(a.wall, at).total_cmp(&dist2(b.wall, at)))
        else {
            return false;
        };
        self.wall_fitting(side, FittingKind::Sconce, tone, Wear::Ok, tuning::SCONCE);
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::geom::Pos;
    use crate::interior::{Floor, FloorPlan, RoomKind, generate_floor, home, validate_floor};
    use crate::tile::{Feature, Fitting, FittingKind, Furniture, Terrain, Tone};
    use crate::world::{BuildingKind, City};
    use crate::{GenParams, Generator, SizePreset};
    use std::collections::{BTreeSet, HashSet};
    use std::sync::OnceLock;

    fn rect(x0: i32, y0: i32, w: i32, h: i32) -> Vec<Pos> {
        (y0..y0 + h)
            .flat_map(|y| (x0..x0 + w).map(move |x| Pos::new(x, y)))
            .collect()
    }

    #[test]
    fn a_tile_one_grid_step_from_a_tube_needs_no_tube_of_its_own() {
        use super::near_a_tube;
        let tubes = [Pos::new(10, 10)];
        assert!(near_a_tube(&tubes, Pos::new(16, 14)), "a grid step away");
        assert!(near_a_tube(&tubes, Pos::new(4, 6)));
        assert!(
            !near_a_tube(&tubes, Pos::new(17, 10)),
            "past the next column"
        );
        assert!(!near_a_tube(&tubes, Pos::new(10, 15)), "past the next row");
    }

    #[test]
    fn an_open_room_hangs_a_tube_on_every_grid_spot_and_fills_no_more() {
        use super::{tube_plan, tube_spots};
        let room = rect(0, 0, 20, 10);
        assert_eq!(tube_plan(&room, |_| true), tube_spots(&room));
        assert_eq!(tube_spots(&room).len(), 6);
    }

    #[test]
    fn tubes_sit_in_the_middle_of_a_narrow_part() {
        use super::tube_spots;
        assert_eq!(tube_spots(&rect(4, 6, 3, 3)), vec![Pos::new(5, 7)]);
        // Seven wide or less: one column, the two-cell tube centred.
        assert_eq!(tube_spots(&rect(0, 0, 7, 2)), vec![Pos::new(2, 1)]);
        // Wide and low: the middle row, every six columns from the corner + 2.
        assert_eq!(
            tube_spots(&rect(10, 3, 14, 3)),
            vec![Pos::new(12, 4), Pos::new(18, 4)]
        );
        // A long, narrow corridor: one column, every four rows from the top + 2.
        assert_eq!(
            tube_spots(&rect(0, 0, 2, 12)),
            vec![Pos::new(0, 2), Pos::new(0, 6), Pos::new(0, 10)]
        );
    }

    /// The interior tests' small city and three Medium ones (seed 8 is the shots').
    fn cities() -> &'static [City] {
        static CITIES: OnceLock<Vec<City>> = OnceLock::new();
        CITIES.get_or_init(|| {
            let small = Generator::run(GenParams {
                seed: 6,
                size: SizePreset::Custom { w: 120, h: 60 },
                districts: 8,
                ..Default::default()
            })
            .unwrap();
            let medium = [8u64, 9, 11].into_iter().map(|seed| {
                Generator::run(GenParams {
                    seed,
                    size: SizePreset::Medium,
                    districts: 7,
                    ..Default::default()
                })
                .unwrap()
            });
            std::iter::once(small).chain(medium).collect()
        })
    }

    /// Every building of `kind` in the cities, with its plan.
    fn plans(kind: BuildingKind) -> Vec<(&'static City, FloorPlan)> {
        cities()
            .iter()
            .flat_map(|c| {
                c.buildings
                    .iter()
                    .filter(move |b| b.kind == kind)
                    .filter_map(move |b| FloorPlan::new(c, b.id).map(|p| (c, p)))
            })
            .collect()
    }

    fn fittings(f: &Floor) -> Vec<(Pos, Fitting)> {
        f.tiles
            .iter()
            .filter_map(|(p, t)| match t.feature {
                Some(Feature::Fitting(x)) => Some((p, x)),
                _ => None,
            })
            .collect()
    }

    /// The first building of every closed kind, at its lowest, ground and top levels.
    fn sample_floors() -> Vec<(BuildingKind, FloorPlan, Floor)> {
        use BuildingKind::*;
        let kinds = [
            CorpTower, Office, Megablock, Apartment, Hotel, Bar, Club, Arcade, Pawn, Clinic,
            Temple, Warehouse, Factory, Depot, Garage, Parking,
        ];
        let mut out = Vec::new();
        for kind in kinds {
            let Some((city, plan)) = plans(kind).into_iter().next() else {
                continue;
            };
            let levels: BTreeSet<i32> = [plan.lowest(), 0, plan.top()]
                .into_iter()
                .filter(|l| plan.has_level(*l))
                .collect();
            for level in levels {
                let f = generate_floor(city, &plan, level);
                out.push((kind, plan.clone(), f));
            }
        }
        out
    }

    #[test]
    fn no_interior_has_a_street_lamp() {
        let floors = sample_floors();
        assert!(floors.len() > 20, "{} floors", floors.len());
        for (kind, plan, f) in &floors {
            let at = format!("{kind:?} {}", f.level);
            assert!(
                f.tiles
                    .iter()
                    .all(|(_, t)| t.feature != Some(Feature::Lamp)),
                "{at}: a street lamp"
            );
            let fs = fittings(f);
            assert!(!fs.is_empty(), "{at}: no fittings");
            for (p, x) in fs {
                let t = &f.tiles[p];
                if x.kind.on_wall() {
                    assert_eq!(t.terrain, Terrain::Wall, "{at}: {x:?} at {p:?}");
                } else {
                    assert_eq!(t.terrain, Terrain::Floor, "{at}: {x:?} at {p:?}");
                    assert!(t.is_walkable(), "{at}: {x:?} at {p:?}");
                }
            }
            validate_floor(plan, f).unwrap_or_else(|e| panic!("{at}: {e}"));
        }
    }

    #[test]
    fn fittings_keep_off_doorsteps_and_doors() {
        for (kind, _, f) in sample_floors() {
            let steps: HashSet<Pos> = f.exits.iter().map(|e| f.doorstep(e)).collect();
            for (p, x) in fittings(&f) {
                assert!(!steps.contains(&p), "{kind:?}: {x:?} on a doorstep {p:?}");
                assert!(
                    !matches!(f.tiles[p].terrain, Terrain::Door | Terrain::Window),
                    "{kind:?}: {x:?} in a door or window at {p:?}"
                );
            }
        }
    }

    #[test]
    fn wall_fittings_light_their_own_side() {
        for (city, plan) in plans(BuildingKind::Hotel).into_iter().take(2) {
            for level in [0, 1.min(plan.top())] {
                let b = super::super::built(city, &plan, level);
                for e in &b.emitters {
                    assert!(
                        !b.tiles[e.pos].blocks_sight(),
                        "light from inside a wall at {:?}",
                        e.pos
                    );
                }
                for (p, t) in b.tiles.iter() {
                    if let Some(Feature::Fitting(x)) = t.feature
                        && x.kind.on_wall()
                        && x.wear != crate::tile::Wear::Dead
                    {
                        assert!(
                            b.emitters.iter().any(|e| e.pos.manhattan(p) == 1),
                            "{x:?} at {p:?} throws no light"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_place_keeps_its_mood_on_every_floor_and_every_visit() {
        let (city, plan) = plans(BuildingKind::Hotel)
            .into_iter()
            .find(|(_, p)| p.top() >= 2)
            .expect("a hotel of three floors");
        let corridor = |level| -> BTreeSet<String> {
            fittings(&generate_floor(city, &plan, level))
                .into_iter()
                .filter(|(_, x)| x.tone == Tone::Fluorescent)
                .map(|(_, x)| format!("{:?}", x.kind))
                .collect()
        };
        assert!(!corridor(1).is_empty());
        assert_eq!(corridor(1), corridor(2));
        assert_eq!(
            generate_floor(city, &plan, 1),
            generate_floor(city, &plan, 1)
        );
    }

    #[test]
    fn bars_come_dark_and_lit() {
        let (mut dark, mut lit) = (0, 0);
        for (city, plan) in plans(BuildingKind::Bar) {
            let f = generate_floor(city, &plan, 0);
            assert!(
                fittings(&f)
                    .iter()
                    .any(|(_, x)| x.kind == FittingKind::NeonStrip),
                "a bar without its backbar"
            );
            let red = |want: bool| -> f32 {
                let v: Vec<f32> = f
                    .tiles
                    .iter()
                    .filter(|(p, t)| {
                        let table = t.feature == Some(Feature::Furniture(Furniture::Table));
                        let bare = t.terrain == Terrain::Floor && t.feature.is_none();
                        (if want { table } else { bare })
                            && f.room_at(*p).is_some_and(|r| r.kind == RoomKind::Bar)
                    })
                    .map(|(p, _)| f.glow[p].0 as f32)
                    .collect();
                v.iter().sum::<f32>() / v.len().max(1) as f32
            };
            if red(true) > red(false) + 40.0 {
                lit += 1;
            } else {
                dark += 1;
            }
        }
        assert!(dark > 0 && lit > 0, "dark {dark}, lit {lit}");
    }

    #[test]
    fn the_flat_is_not_a_bar() {
        let city = &cities()[1];
        let h = home(city).expect("the start flat");
        let plan = FloorPlan::new(city, h.building).unwrap();
        let f = generate_floor(city, &plan, h.level);
        let ring = h.rect.grow(1);
        let near: Vec<(Pos, Fitting)> = fittings(&f)
            .into_iter()
            .filter(|(p, _)| ring.contains(*p))
            .collect();
        assert!(
            near.iter().all(|(_, x)| x.kind != FittingKind::NeonStrip),
            "{near:?}"
        );
        // Every walled part of the flat of four tiles or more has a fitting in it or
        // in a wall beside it.
        let part_tile = |p: Pos| h.rect.contains(p) && f.tiles[p].terrain == Terrain::Floor;
        let parts = crate::pathing::components(f.tiles.size(), part_tile);
        for part in parts.iter().filter(|c| c.len() >= 4) {
            let lit = part.iter().any(|p| {
                std::iter::once(*p)
                    .chain(p.neighbors4())
                    .any(|q| near.iter().any(|(n, _)| *n == q))
            });
            assert!(lit, "a dark part of the flat at {:?}", part[0]);
        }
    }

    #[test]
    fn halls_get_more_than_one_fitting() {
        let (city, plan) = plans(BuildingKind::Warehouse)
            .into_iter()
            .next()
            .expect("a warehouse");
        assert!(fittings(&generate_floor(city, &plan, 0)).len() >= 2);
        let (city, plan) = plans(BuildingKind::Bar).into_iter().next().unwrap();
        let mut temple = plan.clone();
        temple.kind = BuildingKind::Temple;
        let f = generate_floor(city, &temple, 0);
        let hall = fittings(&f)
            .into_iter()
            .filter(|(_, x)| x.kind == FittingKind::Sconce)
            .count();
        assert!(hall >= 2, "{hall} sconces in the temple");
    }

    #[test]
    fn the_chair_is_the_surgerys_brightest_tile() {
        let mut seen = 0;
        let clinics = cities().iter().flat_map(|c| {
            c.buildings
                .iter()
                .filter(move |b| crate::clinic::clinic_front(c, b.id).is_some())
                .filter_map(move |b| FloorPlan::new(c, b.id).map(|p| (c, p)))
        });
        for (city, plan) in clinics.take(4) {
            for level in plan.levels() {
                let f = generate_floor(city, &plan, level);
                let Some(chair) = f.chair() else {
                    continue;
                };
                seen += 1;
                let room = f
                    .rooms
                    .iter()
                    .find(|r| r.kind == RoomKind::Surgery && r.rect.contains(chair))
                    .expect("the chair's surgery");
                for p in room.rect.iter() {
                    assert!(f.light[chair] >= f.light[p], "{p:?} outshines the chair");
                }
            }
        }
        assert!(seen > 0, "no operating chair found");
    }

    #[test]
    fn corridors_are_cooler_than_flats() {
        let (city, plan) = plans(BuildingKind::Megablock)
            .into_iter()
            .find(|(_, p)| p.top() >= 3)
            .expect("a megablock");
        let f = generate_floor(city, &plan, 3);
        let mean = |kind: RoomKind| -> f32 {
            let v: Vec<f32> = f
                .tiles
                .iter()
                .filter(|(p, t)| {
                    t.terrain == Terrain::Floor && f.room_at(*p).is_some_and(|r| r.kind == kind)
                })
                .map(|(p, _)| f.glow[p].2 as f32 - f.glow[p].0 as f32)
                .collect();
            assert!(!v.is_empty(), "no {kind:?}");
            v.iter().sum::<f32>() / v.len() as f32
        };
        assert!(mean(RoomKind::Corridor) > mean(RoomKind::Flat));
    }
}
