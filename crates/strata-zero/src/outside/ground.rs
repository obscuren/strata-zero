//! The open ground round you: a region of the detail world kept round where you stand,
//! walked a cell at a time on the city's clock, seen as far as the light lets you, and
//! remembered.

use super::moves::Moved;
use super::{At, Outside, TILE};
use crate::app::AppState;
use citygen::overworld::{Overworld, detail};
use citygen::{Dir, Grid, Pos, Rect, Terrain, Tile};
use std::collections::{BTreeSet, HashMap};

/// A ring of tiles built beyond what's on screen and in sight, so the ground is there
/// before you see it.
pub const AHEAD: i32 = 1;

/// The open ground round you, tile by tile: each overworld tile's cells built once and
/// kept while near (a tile built alone is the same tile in any region, so they fit);
/// not saved, made again from the world.
#[derive(Default)]
pub struct GroundView {
    tiles: HashMap<Pos, Grid<Tile>>,
    /// The Net's layer rather than the ground's.
    net: bool,
}

impl GroundView {
    /// The ground round `at`: what you can see from there and a little more.
    #[cfg(test)]
    pub fn round(world: &Overworld, at: Pos) -> GroundView {
        let mut v = GroundView::default();
        v.cover(world, sight_area(at));
        v
    }

    /// The open Net round you, kept the same way.
    pub fn net() -> GroundView {
        GroundView {
            net: true,
            ..Default::default()
        }
    }

    /// Builds every tile `area` (world-wide cells) touches that isn't built yet, and a
    /// ring past it, all in one region; forgets tiles far from it.
    pub fn cover(&mut self, world: &Overworld, area: Rect) {
        let (ww, wh) = (world.size.w as i32, world.size.h as i32);
        let t0 = Pos::new(
            (area.x.div_euclid(TILE) - AHEAD).max(0),
            (area.y.div_euclid(TILE) - AHEAD).max(0),
        );
        let t1 = Pos::new(
            ((area.x + area.w).div_euclid(TILE) + AHEAD).min(ww - 1),
            ((area.y + area.h).div_euclid(TILE) + AHEAD).min(wh - 1),
        );
        let missing: Vec<Pos> = (t0.y..=t1.y)
            .flat_map(|y| (t0.x..=t1.x).map(move |x| Pos::new(x, y)))
            .filter(|t| !self.tiles.contains_key(t))
            .collect();
        if let (Some(lo), Some(hi)) = (
            missing
                .iter()
                .copied()
                .reduce(|a, b| Pos::new(a.x.min(b.x), a.y.min(b.y))),
            missing
                .iter()
                .copied()
                .reduce(|a, b| Pos::new(a.x.max(b.x), a.y.max(b.y))),
        ) {
            let (w, h) = ((hi.x - lo.x + 1) as u32, (hi.y - lo.y + 1) as u32);
            let (x, y, w, h) = (TILE * lo.x, TILE * lo.y, w * TILE as u32, h * TILE as u32);
            let g = if self.net {
                citygen::overworld::net_ground::region(world, x, y, w, h)
            } else {
                detail::region(world, x, y, w, h)
            };
            for t in missing {
                let mut tile = Grid::filled(
                    citygen::Size {
                        w: TILE as u32,
                        h: TILE as u32,
                    },
                    Tile::new(Terrain::Void),
                );
                for y in 0..TILE {
                    for x in 0..TILE {
                        let at = Pos::new(TILE * (t.x - lo.x) + x, TILE * (t.y - lo.y) + y);
                        tile[Pos::new(x, y)] = g[at];
                    }
                }
                self.tiles.insert(t, tile);
            }
        }
        // keep what's near: a few tiles past the area, the rest forgotten
        if self.tiles.len() > 2048 {
            let keep = 4;
            self.tiles.retain(|t, _| {
                t.x >= t0.x - keep && t.x <= t1.x + keep && t.y >= t0.y - keep && t.y <= t1.y + keep
            });
        }
    }

    /// A lamp stood on `p` (tests).
    #[cfg(test)]
    pub fn light(&mut self, p: Pos) {
        let t = Pos::new(p.x.div_euclid(TILE), p.y.div_euclid(TILE));
        if let Some(g) = self.tiles.get_mut(&t)
            && let Some(c) = g.get_mut(Pos::new(p.x.rem_euclid(TILE), p.y.rem_euclid(TILE)))
        {
            c.feature = Some(citygen::Feature::Lamp);
        }
    }

    /// The lamps on the built ground within `r` columns of `at` (and `r / 2` rows).
    pub fn lamps_near(&self, at: Pos, r: i32) -> Vec<Pos> {
        (at.y - r / 2..=at.y + r / 2)
            .flat_map(|y| (at.x - r..=at.x + r).map(move |x| Pos::new(x, y)))
            .filter(|p| {
                self.tile(*p)
                    .is_some_and(|t| t.feature == Some(citygen::Feature::Lamp))
            })
            .collect()
    }

    /// The tile at world-wide cell `p`, if it is built.
    pub fn tile(&self, p: Pos) -> Option<&Tile> {
        let t = Pos::new(p.x.div_euclid(TILE), p.y.div_euclid(TILE));
        self.tiles
            .get(&t)?
            .get(Pos::new(p.x.rem_euclid(TILE), p.y.rem_euclid(TILE)))
    }
}

/// The cells you might see from `at`: out to the open ground's day sight each way.
pub fn sight_area(at: Pos) -> Rect {
    Rect::new(
        at.x - RADIUS_OPEN,
        at.y - RADIUS_OPEN,
        2 * RADIUS_OPEN + 1,
        2 * RADIUS_OPEN + 1,
    )
}

/// One step on the open ground: onto the next cell on the city's clock, or into the city
/// when the cell is in its block.
pub fn ground_step(st: &mut AppState, view: &mut GroundView, d: Dir) -> Moved {
    let (Some(o), Some(_)) = (st.outside.as_ref(), st.city.as_ref()) else {
        return Moved::Blocked("there is no world out here");
    };
    let At::Ground(from) = o.at else {
        return Moved::Blocked("not on the open ground");
    };
    let to = from.step(d);
    let tile = Pos::new(to.x.div_euclid(TILE), to.y.div_euclid(TILE));
    if let Some(s) = crate::outside::settlement_at(st, tile) {
        match crate::outside::enter(st, s) {
            Ok(()) => {
                if let (Some(o), Some(city)) = (st.outside.as_ref(), st.city.as_ref()) {
                    return Moved::City(o.street_at(city, to));
                }
            }
            Err(why) => st.warn(why),
        }
    }
    let Some(t) = view.tile(to) else {
        return Moved::Blocked("the world ends here");
    };
    if !t.is_walkable() {
        return Moved::Blocked(match t.terrain {
            Terrain::Water | Terrain::Shallows => "too deep to wade",
            Terrain::Cliff | Terrain::Rock => "too steep to climb",
            Terrain::Void => "the world ends here",
            _ => "something in the way",
        });
    }
    if let Some(o) = st.outside.as_mut() {
        o.at = At::Ground(to);
        view.cover(o.world(), sight_area(to));
    }
    crate::clock::run(st, Some(sim::WALK_TICKS), crate::clock::PATHS_PER_FRAME);
    crate::outside::reached_mark(st);
    Moved::Stepped
}

/// How far you see out in the open by day, with nothing built to stand in the way.
pub const RADIUS_OPEN: i32 = 40;

/// How far a lamp's light reaches, in columns (half as many rows).
pub const LAMP_REACH: f32 = 8.0;

/// The light the lamps throw on `p`: bright beside one, gone `LAMP_REACH` columns off,
/// measured as it looks on screen (a row is two columns).
pub fn lamp_light(lamps: &[Pos], p: Pos) -> u8 {
    lamps
        .iter()
        .map(|l| {
            let (dx, dy) = ((p.x - l.x) as f32, 2.0 * (p.y - l.y) as f32);
            let d = (dx * dx + dy * dy).sqrt();
            (230.0 * (1.0 - d / LAMP_REACH)).max(0.0) as u8
        })
        .max()
        .unwrap_or(0)
}

/// What you see from `at` at `minute`: far across open ground by day, a little way at
/// night, between the two at dusk and dawn; and by night, ground a lamp lights as far
/// as you could see by day, where nothing stands between.
pub fn sight(view: &GroundView, at: Pos, minute: u16) -> BTreeSet<Pos> {
    let night = crate::walker::RADIUS_UNDER + 2;
    let day = citygen::lighting::daylight(minute);
    let radius = night + ((RADIUS_OPEN - night) as f32 * day).round() as i32;
    let lamps = view.lamps_near(at, RADIUS_OPEN + LAMP_REACH as i32);
    let r2 = radius * radius;
    let mut seen = BTreeSet::new();
    crate::walker::fov::compute(
        at,
        RADIUS_OPEN,
        |p| view.tile(p).is_none_or(|t| t.blocks_sight()),
        |p| view.tile(p).is_some(),
        |p| {
            let (dx, dy) = (p.x - at.x, p.y - at.y);
            if dx * dx + dy * dy <= r2 || lamp_light(&lamps, p) >= 60 {
                seen.insert(p);
            }
        },
    );
    seen
}

/// Marks `seen` as seen.
pub fn remember(o: &mut Outside, seen: &BTreeSet<Pos>) {
    for p in seen {
        let (chunk, bit) = cell_bit(*p);
        *o.seen.entry(chunk).or_insert(0) |= 1 << bit;
    }
}

/// Whether world-wide cell `p` has been seen.
pub fn remembered(o: &Outside, p: Pos) -> bool {
    let (chunk, bit) = cell_bit(p);
    o.seen.get(&chunk).is_some_and(|m| m & (1 << bit) != 0)
}

/// What you've seen is kept in chunks of 8×8 cells, a bit a cell: fixed, not the
/// overworld tile's size, so the tile's size can change without the saves changing.
const CHUNK: i32 = 8;

/// The chunk of cell `p` and its bit in that chunk's mask.
fn cell_bit(p: Pos) -> (Pos, u32) {
    (
        Pos::new(p.x.div_euclid(CHUNK), p.y.div_euclid(CHUNK)),
        (p.y.rem_euclid(CHUNK) * CHUNK + p.x.rem_euclid(CHUNK)) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outside::moves::tests::on_the_world;
    use citygen::StratumKind;
    use citygen::overworld::{LINK_E, LINK_W, Water};

    fn outside(st: &AppState) -> &Outside {
        st.outside.as_ref().unwrap()
    }

    fn stand(st: &mut AppState, c: Pos) -> GroundView {
        let o = st.outside.as_mut().unwrap();
        o.at = At::Ground(c);
        GroundView::round(o.world(), c)
    }

    fn at(st: &AppState) -> Pos {
        match outside(st).at {
            At::Ground(c) => c,
            other => panic!("{other:?}"),
        }
    }

    /// A highway tile linked east-west outside the block, with its east neighbour too.
    fn highway(st: &AppState) -> Pos {
        let o = outside(st);
        let b = o.block(st.city.as_ref().unwrap());
        let w = o.world();
        let ew = |p: Pos| {
            !b.contains(p)
                && !crate::outside::in_any_block(st, p)
                && w.tile(p)
                    .is_some_and(|t| t.road == LINK_E | LINK_W && t.water == Water::None)
        };
        w.tiles
            .iter()
            .map(|(p, _)| p)
            .find(|p| {
                ew(*p)
                    && ew(p.offset(1, 0))
                    && (2..10).all(|k| !crate::outside::in_any_block(st, p.offset(k, 0)))
            })
            .expect("a highway")
    }

    #[test]
    fn walking_east_crosses_into_the_next_tile() {
        let mut st = on_the_world();
        let t = highway(&st);
        let mut view = stand(&mut st, Pos::new(TILE * t.x + 7, TILE * t.y + 3));
        let before = st.world.as_ref().unwrap().tick();
        assert_eq!(ground_step(&mut st, &mut view, Dir::E), Moved::Stepped);
        assert_eq!(at(&st), Pos::new(TILE * (t.x + 1), TILE * t.y + 3));
        assert_eq!(st.world.as_ref().unwrap().tick() - before, sim::WALK_TICKS);
    }

    #[test]
    fn water_stops_you() {
        let mut st = on_the_world();
        let w = outside(&st).world().clone();
        let t = w
            .tiles
            .iter()
            .find(|(_, t)| t.water == Water::River && t.road == 0)
            .map(|(p, _)| p)
            .unwrap();
        let view = GroundView::round(&w, Pos::new(TILE * t.x + 4, TILE * t.y + 4));
        let (from, d) = (TILE * (t.y - 1)..TILE * (t.y + 2))
            .flat_map(|y| (TILE * (t.x - 1)..TILE * (t.x + 2)).map(move |x| Pos::new(x, y)))
            .flat_map(|p| [Dir::N, Dir::E, Dir::S, Dir::W].map(|d| (p, d)))
            .find(|(p, d)| {
                view.tile(*p).is_some_and(|c| c.is_walkable())
                    && view
                        .tile(p.step(*d))
                        .is_some_and(|c| matches!(c.terrain, Terrain::Water | Terrain::Shallows))
            })
            .unwrap();
        let mut view = stand(&mut st, from);
        assert!(matches!(
            ground_step(&mut st, &mut view, d),
            Moved::Blocked(_)
        ));
        assert_eq!(at(&st), from);
    }

    #[test]
    fn the_ground_stays_right_as_you_walk() {
        let mut st = on_the_world();
        // a highway whose row is open ground for 64 cells east, off every city's block
        let start = {
            let o = outside(&st);
            let w = o.world();
            w.tiles
                .iter()
                .filter(|(p, t)| {
                    t.road == LINK_E | LINK_W
                        && t.water == Water::None
                        && (0..9).all(|k| !crate::outside::in_any_block(&st, p.offset(k, 0)))
                })
                .map(|(p, _)| Pos::new(TILE * p.x, TILE * p.y + 3))
                .find(|c| {
                    detail::region(w, c.x, c.y, 64, 1)
                        .iter()
                        .all(|(_, t)| t.is_walkable())
                })
                .expect("an open stretch of highway")
        };
        let mut view = stand(&mut st, start);
        let w = outside(&st).world().clone();
        let mut steps = 0;
        while steps < 200 && ground_step(&mut st, &mut view, Dir::E) == Moved::Stepped {
            steps += 1;
            let c = at(&st);
            let want = detail::region(&w, c.x, c.y, 1, 1)[Pos::new(0, 0)];
            let got = view.tile(c).unwrap();
            assert_eq!(
                (got.terrain, got.variant),
                (want.terrain, want.variant),
                "{c:?}"
            );
        }
        assert!(steps >= 60, "{steps} steps along the highway");
        assert_eq!(at(&st), start.offset(steps, 0));
        // and what's ahead is built before you get there
        assert!(view.tile(at(&st).offset(RADIUS_OPEN, 0)).is_some() || steps < 60);
    }

    #[test]
    fn what_you_saw_stays_seen() {
        let mut st = on_the_world();
        let t = highway(&st);
        let here = Pos::new(TILE * t.x + 4, TILE * t.y + 3);
        let mut view = stand(&mut st, here);
        let seen = sight(&view, here, 12 * 60);
        assert!(seen.contains(&here) && seen.len() > 100, "{}", seen.len());
        assert!(seen.len() > sight(&view, here, 0).len(), "less at night");
        remember(st.outside.as_mut().unwrap(), &seen);
        for _ in 0..50 {
            ground_step(&mut st, &mut view, Dir::E);
        }
        for p in &seen {
            assert!(remembered(outside(&st), *p), "{p:?}");
        }
        assert!(!remembered(outside(&st), here.offset(0, 60)));
    }

    #[test]
    fn by_day_the_open_ground_is_seen_far() {
        let mut st = on_the_world();
        let t = highway(&st);
        let here = Pos::new(TILE * t.x + 4, TILE * t.y + 3);
        let view = stand(&mut st, here);
        // along the highway's lane, flat and open
        let far = here.offset(30, 0);
        assert!(
            sight(&view, here, 12 * 60).contains(&far),
            "30 cells down the road by day"
        );
        assert!(!sight(&view, here, 0).contains(&far), "not at night");
    }

    #[test]
    fn a_lamp_lights_the_ground_round_it() {
        let lamps = [Pos::new(100, 50)];
        let near = lamp_light(&lamps, Pos::new(102, 50));
        let far = lamp_light(&lamps, Pos::new(112, 50));
        assert!(near > 150 && far == 0, "{near} {far}");
        // a row is two columns on screen: two rows off is as dark as four columns
        assert_eq!(
            lamp_light(&lamps, Pos::new(100, 52)),
            lamp_light(&lamps, Pos::new(104, 50))
        );
    }

    #[test]
    fn lit_ground_is_seen_from_afar() {
        let mut st = on_the_world();
        let t = highway(&st);
        let here = Pos::new(TILE * t.x + 4, TILE * t.y + 3);
        let mut view = stand(&mut st, here);
        // a lamp down the road, well past night sight
        let far = here.offset(24, 0);
        view.light(far);
        let night = sight(&view, here, 0);
        assert!(night.contains(&far), "the lamp is seen");
        assert!(
            night.contains(&far.offset(-2, 0)),
            "and the ground it lights"
        );
        assert!(
            !night.contains(&here.offset(16, -6)),
            "not the dark between"
        );
    }

    #[test]
    fn the_city_block_takes_you_in() {
        let mut st = on_the_world();
        let o = outside(&st);
        let city = st.city.clone().unwrap();
        let b = o.block(&city);
        let w = o.world().clone();
        // a walkable cell just west of the block with the block's cell beside it
        let x = TILE * b.x - 1;
        let c = (TILE * b.y..TILE * (b.y + b.h))
            .map(|y| Pos::new(x, y))
            .find(|p| detail::region(&w, p.x, p.y, 1, 1)[Pos::new(0, 0)].is_walkable())
            .expect("a way in from the west");
        let mut view = stand(&mut st, c);
        let Moved::City(cell) = ground_step(&mut st, &mut view, Dir::E) else {
            panic!("into the city")
        };
        let t = city.tile(StratumKind::Street, cell).unwrap();
        assert!(t.is_walkable() && t.building.is_none());
        // until the city grows into the world (#19) its edge is not the world's: the
        // nearest street may be some way in
    }
}
