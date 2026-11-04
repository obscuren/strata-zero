//! The open ground round you: a region of the detail world kept round where you stand,
//! walked a cell at a time on the city's clock, seen as far as the light lets you, and
//! remembered.

use super::moves::Moved;
use super::{At, Outside, TILE};
use crate::app::AppState;
use citygen::overworld::{Overworld, detail};
use citygen::{Dir, Grid, Pos, Terrain, Tile};
use std::collections::BTreeSet;

/// The region kept round you, in detail cells.
pub const SPAN: (u32, u32) = (16 * TILE as u32, 8 * TILE as u32);

/// How near its edge you may come before the region is made again round you.
pub const KEEP: i32 = 24;

/// A region of the open ground; not saved (made again from the world).
pub struct GroundView {
    /// The world-wide cell of the region's top-left.
    pub origin: Pos,
    pub tiles: Grid<Tile>,
}

impl GroundView {
    /// The region centred on `at`.
    pub fn round(world: &Overworld, at: Pos) -> GroundView {
        let origin = Pos::new(at.x - SPAN.0 as i32 / 2, at.y - SPAN.1 as i32 / 2);
        GroundView {
            origin,
            tiles: detail::region(world, origin.x, origin.y, SPAN.0, SPAN.1),
        }
    }

    /// Makes the region again round `at` when `at` comes within `KEEP` of its edge;
    /// whether it did.
    pub fn keep(&mut self, world: &Overworld, at: Pos) -> bool {
        let (x, y) = (at.x - self.origin.x, at.y - self.origin.y);
        let (w, h) = (SPAN.0 as i32, SPAN.1 as i32);
        let near = x < KEEP || y < KEEP / 2 || x >= w - KEEP || y >= h - KEEP / 2;
        if near {
            *self = GroundView::round(world, at);
        }
        near
    }

    /// The tile at world-wide cell `p`, if the region holds it.
    pub fn tile(&self, p: Pos) -> Option<&Tile> {
        self.tiles
            .get(Pos::new(p.x - self.origin.x, p.y - self.origin.y))
    }
}

/// One step on the open ground: onto the next cell on the city's clock, or into the city
/// when the cell is in its block.
pub fn ground_step(st: &mut AppState, view: &mut GroundView, d: Dir) -> Moved {
    let (Some(o), Some(city)) = (st.outside.as_ref(), st.city.as_ref()) else {
        return Moved::Blocked("there is no world out here");
    };
    let At::Ground(from) = o.at else {
        return Moved::Blocked("not on the open ground");
    };
    let to = from.step(d);
    if o.in_city(city, to) {
        return Moved::City(o.street_at(city, to));
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
        view.keep(o.world(), to);
    }
    crate::clock::run(st, Some(sim::WALK_TICKS), crate::clock::PATHS_PER_FRAME);
    Moved::Stepped
}

/// What you see from `at` at `minute`: as far as the street by day, less at night.
pub fn sight(view: &GroundView, at: Pos, minute: u16) -> BTreeSet<Pos> {
    let radius = if citygen::lighting::daylight(minute) > 0.25 {
        crate::walker::RADIUS_STREET
    } else {
        crate::walker::RADIUS_UNDER + 2
    };
    let mut seen = BTreeSet::new();
    crate::walker::fov::compute(
        at,
        radius,
        |p| view.tile(p).is_none_or(|t| t.blocks_sight()),
        |p| view.tile(p).is_some(),
        |p| {
            seen.insert(p);
        },
    );
    seen
}

/// Marks `seen` as seen.
pub fn remember(o: &mut Outside, seen: &BTreeSet<Pos>) {
    for p in seen {
        let (tile, bit) = cell_bit(*p);
        *o.seen.entry(tile).or_insert(0) |= 1 << bit;
    }
}

/// Whether world-wide cell `p` has been seen.
pub fn remembered(o: &Outside, p: Pos) -> bool {
    let (tile, bit) = cell_bit(p);
    o.seen.get(&tile).is_some_and(|m| m & (1 << bit) != 0)
}

/// The overworld tile of cell `p` and its bit in that tile's mask.
fn cell_bit(p: Pos) -> (Pos, u32) {
    (
        Pos::new(p.x.div_euclid(TILE), p.y.div_euclid(TILE)),
        (p.y.rem_euclid(TILE) * TILE + p.x.rem_euclid(TILE)) as u32,
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
                && w.tile(p)
                    .is_some_and(|t| t.road == LINK_E | LINK_W && t.water == Water::None)
        };
        w.tiles
            .iter()
            .map(|(p, _)| p)
            .find(|p| ew(*p) && ew(p.offset(1, 0)))
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
        let (from, d) = view
            .tiles
            .iter()
            .map(|(p, _)| Pos::new(view.origin.x + p.x, view.origin.y + p.y))
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
    fn the_region_recentres_without_moving_you() {
        let mut st = on_the_world();
        let t = highway(&st);
        let start = Pos::new(TILE * t.x, TILE * t.y + 3);
        let mut view = stand(&mut st, start);
        let first = view.origin;
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
        assert_ne!(view.origin, first, "the region moved with you");
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
