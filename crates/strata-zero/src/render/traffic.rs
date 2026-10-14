//! Ambient traffic on the arterials: purely visual, no simulation beyond position.

use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use citygen::rng::hash64;
use citygen::{City, Dir, Hue, Pos, Rect as TileRect, StratumKind, Terrain};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::collections::HashSet;

pub const MAX_CARS: usize = 24;
const MAX_AGE: u32 = 600;

#[derive(Clone, Copy, Debug)]
pub struct Car {
    pub pos: Pos,
    pub dir: Dir,
    pub hue: Hue,
    pub age: u32,
}

#[derive(Default)]
pub struct Traffic {
    pub cars: Vec<Car>,
    arterial: HashSet<Pos>,
    arterial_key: Option<(u64, u32, u32)>,
}

impl Traffic {
    fn ensure_arterials(&mut self, city: &City) {
        let key = (city.seed, city.size.w, city.size.h);
        if self.arterial_key == Some(key) {
            return;
        }
        self.arterial.clear();
        for path in &city.arterials {
            for p in path {
                for q in TileRect::new(p.x - 1, p.y - 1, 4, 4).iter() {
                    self.arterial.insert(q);
                }
            }
        }
        self.arterial_key = Some(key);
        self.cars.clear();
    }

    fn drivable(&self, city: &City, p: Pos) -> bool {
        self.arterial.contains(&p)
            && city
                .tile(StratumKind::Street, p)
                .is_some_and(|t| t.terrain == Terrain::Road && t.feature.is_none())
    }

    /// Advances cars and spawns new ones at the edge of `view` (in tiles). Only the Street
    /// has traffic.
    pub fn update(&mut self, city: &City, stratum: StratumKind, view: TileRect, tick: u64) {
        if stratum != StratumKind::Street {
            return;
        }
        self.ensure_arterials(city);
        if tick.is_multiple_of(2) {
            let cars = std::mem::take(&mut self.cars);
            let mut kept = Vec::with_capacity(cars.len());
            for mut car in cars {
                car.age += 1;
                if car.age > MAX_AGE {
                    continue;
                }
                let ahead = car.pos.step(car.dir);
                if self.drivable(city, ahead) {
                    car.pos = ahead;
                    kept.push(car);
                    continue;
                }
                let turns = [turn_left(car.dir), turn_right(car.dir), car.dir.opposite()];
                if let Some(d) = turns
                    .into_iter()
                    .find(|d| self.drivable(city, car.pos.step(*d)))
                {
                    car.dir = d;
                    car.pos = car.pos.step(d);
                    kept.push(car);
                }
            }
            self.cars = kept;
        }
        if tick.is_multiple_of(10) && self.cars.len() < MAX_CARS {
            let edge: Vec<Pos> = view
                .border()
                .filter(|p| self.drivable(city, *p))
                .filter(|p| self.cars.iter().all(|c| c.pos != *p))
                .collect();
            let h = hash64(tick, city.seed);
            // No arterial crosses the view edge: pick a drivable tile inside instead.
            let inside = (0..64).find_map(|i| {
                let hh = hash64(h, i);
                let p = Pos::new(
                    view.x + (hh % view.w.max(1) as u64) as i32,
                    view.y + ((hh >> 20) % view.h.max(1) as u64) as i32,
                );
                (self.drivable(city, p) && self.cars.iter().all(|c| c.pos != p)).then_some(p)
            });
            let pick = if edge.is_empty() {
                inside
            } else {
                Some(edge[(h % edge.len() as u64) as usize])
            };
            if let Some(pos) = pick {
                let order = [Dir::N, Dir::E, Dir::S, Dir::W];
                let start = ((h >> 8) % 4) as usize;
                let dir = (0..4)
                    .map(|i| order[(start + i) % 4])
                    .find(|d| self.drivable(city, pos.step(*d)));
                if let Some(dir) = dir {
                    let hues = [Hue::White, Hue::Yellow, Hue::Red, Hue::Cyan];
                    self.cars.push(Car {
                        pos,
                        dir,
                        hue: hues[((h >> 16) % 4) as usize],
                        age: 0,
                    });
                }
            }
        }
    }
}

fn turn_left(d: Dir) -> Dir {
    match d {
        Dir::N => Dir::W,
        Dir::W => Dir::S,
        Dir::S => Dir::E,
        Dir::E => Dir::N,
        other => other,
    }
}

fn turn_right(d: Dir) -> Dir {
    match d {
        Dir::N => Dir::E,
        Dir::E => Dir::S,
        Dir::S => Dir::W,
        Dir::W => Dir::N,
        other => other,
    }
}

impl Overlay for Traffic {
    fn draw(
        &self,
        _city: &City,
        stratum: StratumKind,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        style: &StylePack,
    ) {
        if stratum != StratumKind::Street {
            return;
        }
        for car in &self.cars {
            let color = style.hue(car.hue);
            if let Some((sx, sy)) = tile_to_screen(area, origin, car.pos.step(car.dir)) {
                let c = &mut buf[(sx, sy)];
                let bg = match c.bg {
                    ratatui::style::Color::Rgb(r, g, b) => crate::render::Rgb(r, g, b),
                    _ => crate::render::Rgb(0, 0, 0),
                };
                c.set_bg(bg.mix(color, 0.3).to_color());
            }
            if let Some((sx, sy)) = tile_to_screen(area, origin, car.pos) {
                buf[(sx, sy)].set_char('▬').set_fg(color.to_color());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{GenParams, Generator, Rect, SizePreset, StratumKind, Terrain};

    #[test]
    fn traffic_stays_on_roads() {
        let c = Generator::run(GenParams {
            seed: 3,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        let mut tr = Traffic::default();
        for tick in 0..400 {
            tr.update(&c, StratumKind::Street, Rect::new(0, 0, 120, 60), tick);
        }
        assert!(!tr.cars.is_empty());
        assert!(tr.cars.len() <= MAX_CARS);
        for car in &tr.cars {
            assert_eq!(
                c.tile(StratumKind::Street, car.pos).unwrap().terrain,
                Terrain::Road
            );
        }
        tr.update(&c, StratumKind::Skyways, Rect::new(0, 0, 120, 60), 401);
    }
}
