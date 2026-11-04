//! The world beyond the city: the overworld, where you are on it, and what you have seen
//! of the open ground. The overworld comes from the city's seed and is rebuilt on load;
//! the home city sits on a block of its tiles (until the city grows into the world, the
//! whole block is the city). See `2026-11-04-overworld-screen-design.md`.

pub mod ground;
pub mod moves;

use crate::app::AppState;
use citygen::overworld::{Overworld, WorldParams};
use citygen::{City, Pos, Rect, StratumKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Where you are: in the city, on an overworld tile, or on a detail cell of the open
/// ground (world-wide detail coordinates).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum At {
    #[default]
    City,
    Overworld(Pos),
    Ground(Pos),
}

/// The world beyond the city, as the run holds it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Outside {
    pub params: WorldParams,
    pub at: At,
    /// What you have seen of the open ground: per chunk of 8×8 detail cells (not per
    /// overworld tile, whose size may change), a bit a cell (`y * 8 + x`).
    pub seen: BTreeMap<Pos, u64>,
    /// The overworld, made from `params`; rebuilt on load (`after_restore`).
    #[serde(skip)]
    pub world: Option<Overworld>,
    /// Jacked in on the open ground: where you are on its Net, your body on the ground
    /// cell `at` names. Not saved: a run reopened out here wakes on the body.
    #[serde(skip)]
    pub net: Option<Pos>,
}

impl Outside {
    /// The world round `city`, made from its seed, with you in the city.
    pub fn new(city: &City) -> anyhow::Result<Outside> {
        // a city on its site stands in that world; one from before, in its seed's
        let params = match &city.params.site {
            Some(site) => site.world.clone(),
            None => world_params(city.seed),
        };
        let world = Overworld::generate(params.clone())?;
        Ok(Outside {
            params,
            at: At::City,
            seen: BTreeMap::new(),
            world: Some(world),
            net: None,
        })
    }

    pub fn world(&self) -> &Overworld {
        self.world
            .as_ref()
            .expect("the overworld is made with the run and on load")
    }

    /// Makes the overworld again from its params (after a load).
    pub fn rebuild(&mut self) {
        if self.world.is_none() {
            self.world = Overworld::generate(self.params.clone()).ok();
        }
    }

    /// The overworld tiles the city's map covers: for a city on its site, the block it
    /// was built on; for one from before, the city's size in tiles, centred on the home
    /// city's own tiles, inside the world.
    pub fn block(&self, city: &City) -> Rect {
        if let Some(site) = &city.params.site {
            return Rect::new(
                site.origin.x.div_euclid(TILE),
                site.origin.y.div_euclid(TILE),
                city.size.w as i32 / TILE,
                city.size.h as i32 / TILE,
            );
        }
        let w = self.world();
        let (bw, bh) = (
            city.size.w.div_ceil(TILE as u32) as i32,
            city.size.h.div_ceil(TILE as u32) as i32,
        );
        let home = w.cities.iter().position(|c| c.home).unwrap_or(0) as u8;
        let mine: Vec<Pos> = w
            .tiles
            .iter()
            .filter(|(_, t)| t.urban.is_some_and(|u| u.city == home))
            .map(|(p, _)| p)
            .collect();
        let centre = match (
            mine.iter().map(|p| p.x).min(),
            mine.iter().map(|p| p.x).max(),
            mine.iter().map(|p| p.y).min(),
            mine.iter().map(|p| p.y).max(),
        ) {
            (Some(x0), Some(x1), Some(y0), Some(y1)) => Pos::new((x0 + x1) / 2, (y0 + y1) / 2),
            _ => w.home().centre,
        };
        let fit =
            |c: i32, len: i32, world: u32| (c - len / 2).clamp(0, (world as i32 - len).max(0));
        Rect::new(
            fit(centre.x, bw, w.size.w),
            fit(centre.y, bh, w.size.h),
            bw,
            bh,
        )
    }

    /// The overworld tile a cell of the city's street lies in.
    pub fn tile_of(&self, city: &City, p: Pos) -> Pos {
        let b = self.block(city);
        Pos::new(
            b.x + p.x.div_euclid(TILE).clamp(0, b.w - 1),
            b.y + p.y.div_euclid(TILE).clamp(0, b.h - 1),
        )
    }

    /// Where you stand in the city coming in on block tile `tile`: the outdoor street
    /// cell nearest the tile's middle.
    pub fn street_of(&self, city: &City, tile: Pos) -> Pos {
        let b = self.block(city);
        street_near(
            city,
            Pos::new(
                TILE * (tile.x - b.x) + TILE / 2,
                TILE * (tile.y - b.y) + TILE / 2,
            ),
        )
    }

    /// Where you stand in the city walking in at world-wide cell `cell` of its block:
    /// the outdoor street cell nearest the city's cell there.
    pub fn street_at(&self, city: &City, cell: Pos) -> Pos {
        let b = self.block(city);
        street_near(city, Pos::new(cell.x - TILE * b.x, cell.y - TILE * b.y))
    }

    /// Whether world-wide detail cell `cell` lies in the city's block.
    pub fn in_city(&self, city: &City, cell: Pos) -> bool {
        self.block(city)
            .contains(Pos::new(cell.x.div_euclid(TILE), cell.y.div_euclid(TILE)))
    }
}

/// The outdoor street cell of `city` nearest `cell` (clamped into the city).
fn street_near(city: &City, cell: Pos) -> Pos {
    let (cw, ch) = (city.size.w as i32, city.size.h as i32);
    let from = Pos::new(cell.x.clamp(0, cw - 1), cell.y.clamp(0, ch - 1));
    nearest(from, cw.max(ch), |p| {
        city.tile(StratumKind::Street, p)
            .is_some_and(|t| t.is_walkable() && t.building.is_none())
    })
    .unwrap_or(from)
}

/// A run's world for seed `seed`: the default world, its size kept in tiles rather than
/// by the preset's name, so a run keeps its world when the presets are retuned.
pub fn world_params(seed: u64) -> WorldParams {
    let d = WorldParams::default();
    let citygen::Size { w, h } = d.size.size();
    WorldParams {
        seed,
        size: citygen::overworld::WorldSize::Custom { w, h },
        ..d
    }
}

/// Detail cells to an overworld tile, each way.
pub const TILE: i32 = citygen::overworld::detail::TILE;

/// The cell nearest `from` (by rings, then row order) where `ok` holds, out to `reach`.
pub fn nearest(from: Pos, reach: i32, ok: impl Fn(Pos) -> bool) -> Option<Pos> {
    (0..=reach).find_map(|r| {
        (-r..=r)
            .flat_map(|dy| (-r..=r).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy)| dx.abs().max(dy.abs()) == r)
            .map(|(dx, dy)| from.offset(dx, dy))
            .find(|p| ok(*p))
    })
}

/// You are out of the city, on the overworld or the open ground: the city holds still
/// for you (its story waits, its cameras look at no one), your body isn't on its street.
pub fn away(st: &AppState) -> bool {
    st.outside.as_ref().is_some_and(|o| o.at != At::City)
}

/// After a run is opened: the overworld made again, and you back in the city if what
/// you stood on is no longer there (the world under a saved run can change between
/// builds until seeds are fixed for 1.0).
pub fn reopened(st: &mut AppState) {
    let Some(o) = st.outside.as_mut() else {
        return;
    };
    o.rebuild();
    let Some(w) = o.world.as_ref() else {
        st.outside = None;
        st.warn("the world out there couldn't be made again · back in the city");
        return;
    };
    let stands = match o.at {
        At::City => true,
        At::Overworld(t) => moves::step_cost(w, t).is_some(),
        At::Ground(c) => {
            let (cw, ch) = (TILE * w.size.w as i32, TILE * w.size.h as i32);
            c.x >= 0
                && c.y >= 0
                && c.x < cw
                && c.y < ch
                && citygen::overworld::detail::region(w, c.x, c.y, 1, 1)[Pos::new(0, 0)]
                    .is_walkable()
        }
    };
    if !stands {
        o.at = At::City;
        st.warn("the world out there has changed since this run was saved · back in the city");
    }
}

/// Whether you can leave the city for the world now: on foot on its open street, in a
/// city (not the lot), not jacked in, not in a fight, with no call or scene under way.
pub fn can_leave(st: &AppState) -> Result<(), &'static str> {
    let (Some(city), Some(p)) = (st.city.as_ref(), st.player.as_ref()) else {
        return Err("nowhere to go");
    };
    if city.is_lot() {
        return Err("there is no world past the lot");
    }
    if p.jacked_in() {
        return Err("jack out first");
    }
    if st.net.engaged.is_some() {
        return Err("not in a fight");
    }
    let m = &st.missions;
    if m.call.is_some() || !m.calls.is_empty() {
        return Err("not with a call on the line");
    }
    if m.scene.is_some() || !m.scenes.is_empty() {
        return Err("not now");
    }
    let outdoors = p.place.is_street()
        && city
            .tile(StratumKind::Street, p.pos)
            .is_some_and(|t| t.building.is_none());
    if !outdoors {
        return Err("only from the open street");
    }
    Ok(())
}

/// Out of the city by its edge: from street cell `from` a step `d` off the map, onto
/// the open ground beside the block, where there is ground to stand on.
pub fn leave_by(st: &mut AppState, from: Pos, d: citygen::Dir) -> Result<(), &'static str> {
    can_leave(st)?;
    let city = st.city.clone().ok_or("nowhere to go")?;
    let o = ensure(st).ok_or("there is no world out here")?;
    let b = o.block(&city);
    let to = from.step(d);
    let cell = Pos::new(TILE * b.x + to.x, TILE * b.y + to.y);
    let w = o.world();
    if cell.x < 0
        || cell.y < 0
        || cell.x >= TILE * w.size.w as i32
        || cell.y >= TILE * w.size.h as i32
    {
        return Err("the world ends here");
    }
    let ground = citygen::overworld::detail::region(w, cell.x, cell.y, 1, 1)[Pos::new(0, 0)];
    if !ground.is_walkable() {
        return Err("no way out there");
    }
    o.at = At::Ground(cell);
    Ok(())
}

/// The run's world beyond the city, made from the city's seed the first time it is
/// needed (an old save, a test or a dev start has none yet).
pub fn ensure(st: &mut AppState) -> Option<&mut Outside> {
    if st.outside.is_none() {
        let city = st.city.as_ref()?;
        st.outside = Some(Outside::new(city).ok()?);
    }
    st.outside.as_mut()
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{GenParams, Generator, SizePreset};

    fn medium() -> City {
        Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap()
    }

    /// Seed 3's home city built on its place in its world, once for every test.
    fn sited() -> &'static City {
        static CITY: std::sync::OnceLock<City> = std::sync::OnceLock::new();
        CITY.get_or_init(|| {
            crate::screens::build_home(GenParams {
                seed: 3,
                ..Default::default()
            })
            .unwrap()
        })
    }

    #[test]
    fn the_block_is_the_generators() {
        let city = sited();
        let o = Outside::new(city).unwrap();
        let site = city.params.site.as_ref().unwrap();
        assert_eq!(o.params, site.world, "the city's own world");
        assert_eq!(o.block(city), o.world().city_block(site.city));
    }

    #[test]
    fn walking_off_the_city_the_ground_goes_on() {
        // the cells along the city's edges are the open ground's own: walk off its map
        // and the ground goes on as it was
        let city = sited();
        let o = Outside::new(city).unwrap();
        let b = o.block(city);
        let (w, h) = (city.size.w as i32, city.size.h as i32);
        let ground = citygen::overworld::detail::region(
            o.world(),
            TILE * b.x,
            TILE * b.y,
            w as u32,
            h as u32,
        );
        let edge = (0..h)
            .flat_map(|y| [Pos::new(0, y), Pos::new(w - 1, y)])
            .chain((0..w).flat_map(|x| [Pos::new(x, 0), Pos::new(x, h - 1)]))
            .collect::<Vec<_>>();
        let same = edge
            .iter()
            .filter(|p| city.tile(StratumKind::Street, **p).unwrap().terrain == ground[**p].terrain)
            .count();
        assert!(
            same * 10 >= edge.len() * 9,
            "{same} of {} edge cells are the ground's",
            edge.len()
        );
    }

    #[test]
    fn an_old_save_keeps_its_block() {
        // a city from before cities stood in the world has no site: its world comes
        // from its seed and its block is centred on the home city, as it was
        let city = medium();
        assert!(city.params.site.is_none());
        let o = Outside::new(&city).unwrap();
        assert_eq!(o.params, world_params(city.seed));
        let b = o.block(&city);
        assert_eq!((b.w, b.h), (30, 15));
    }

    #[test]
    fn the_world_comes_from_the_citys_seed() {
        let city = medium();
        let (a, b) = (Outside::new(&city).unwrap(), Outside::new(&city).unwrap());
        assert_eq!(a.params.seed, city.seed);
        assert_eq!(a.world().tiles, b.world().tiles);
        assert_eq!(a.at, At::City);
    }

    #[test]
    fn the_home_block_is_the_city_at_its_size() {
        let city = medium();
        let o = Outside::new(&city).unwrap();
        let b = o.block(&city);
        assert_eq!((b.w, b.h), (30, 15));
        let w = o.world();
        assert!(b.x >= 0 && b.y >= 0);
        assert!(b.x + b.w <= w.size.w as i32 && b.y + b.h <= w.size.h as i32);
        // over the home city's own tiles
        let home = w.cities.iter().position(|c| c.home).unwrap() as u8;
        let mine = w
            .tiles
            .iter()
            .filter(|(_, t)| t.urban.is_some_and(|u| u.city == home))
            .count();
        let inside = w
            .tiles
            .iter()
            .filter(|(p, t)| b.contains(*p) && t.urban.is_some_and(|u| u.city == home))
            .count();
        assert!(
            inside * 10 >= mine * 8,
            "{inside} of {mine} home tiles in the block"
        );
    }

    #[test]
    fn a_block_against_the_worlds_edge_stays_inside() {
        let city = medium();
        let mut o = Outside::new(&city).unwrap();
        let w = o.world.as_mut().unwrap();
        let all: Vec<Pos> = w.tiles.iter().map(|(p, _)| p).collect();
        for p in all {
            w.tiles[p].urban = None;
        }
        let home = w.cities.iter().position(|c| c.home).unwrap() as u8;
        w.tiles[Pos::new(1, 1)].urban = Some(citygen::overworld::Urban {
            city: home,
            density: 200,
            theme: citygen::Theme::CorpCore,
        });
        let b = o.block(&city);
        assert_eq!((b.x, b.y, b.w, b.h), (0, 0, 30, 15));
    }

    #[test]
    fn city_cells_and_tiles_map_both_ways() {
        let city = medium();
        let o = Outside::new(&city).unwrap();
        let b = o.block(&city);
        for t in [
            Pos::new(b.x, b.y),
            Pos::new(b.x + b.w - 1, b.y),
            Pos::new(b.x, b.y + b.h - 1),
            Pos::new(b.x + b.w - 1, b.y + b.h - 1),
            Pos::new(b.x + b.w / 2, b.y + b.h / 2),
        ] {
            let cell = o.street_of(&city, t);
            let tile = city.tile(StratumKind::Street, cell).unwrap();
            assert!(
                tile.is_walkable() && tile.building.is_none(),
                "{t:?} → {cell:?}"
            );
        }
        // in the middle of the city a street is never far; at a corner it may be
        // across the bay
        let mid = Pos::new(b.x + b.w / 2, b.y + b.h / 2);
        let back = o.tile_of(&city, o.street_of(&city, mid));
        assert!(
            (back.x - mid.x).abs() <= 1 && (back.y - mid.y).abs() <= 1,
            "{back:?}"
        );
        for p in [Pos::new(0, 0), Pos::new(239, 119), Pos::new(100, 50)] {
            let t = o.tile_of(&city, p);
            assert!(b.contains(t), "{p:?} → {t:?}");
            assert_eq!(t, Pos::new(b.x + p.x / 8, b.y + p.y / 8));
        }
    }

    #[test]
    fn ensure_builds_once() {
        let mut st = AppState::new();
        assert!(ensure(&mut st).is_none(), "no city, no world");
        st.enter_city(medium());
        ensure(&mut st).unwrap().at = At::Overworld(Pos::new(3, 4));
        let o = ensure(&mut st).unwrap();
        assert_eq!(o.at, At::Overworld(Pos::new(3, 4)));
    }

    #[test]
    fn the_world_comes_back_from_a_save() {
        let mut st = AppState::new();
        st.enter_city(medium());
        let o = ensure(&mut st).unwrap();
        o.at = At::Overworld(Pos::new(40, 20));
        o.seen.insert(Pos::new(41, 20), 0xF0F0);
        let tiles = o.world().tiles.clone();
        let dir = crate::runs::testutil::Dir::new("outside");
        let st = crate::runs::testutil::running(st);
        let path = crate::runs::file::write(&dir.0, &st).unwrap();
        let mut back = AppState::new();
        let (_, snap, lost) = crate::runs::file::read(&path, &back.book).unwrap();
        crate::runs::snapshot::restore(&mut back, snap);
        crate::runs::snapshot::after_restore(&mut back, lost);
        let o = back.outside.as_ref().unwrap();
        assert_eq!(o.at, At::Overworld(Pos::new(40, 20)));
        assert_eq!(o.seen.get(&Pos::new(41, 20)), Some(&0xF0F0));
        assert_eq!(o.world().tiles, tiles);
    }

    #[test]
    fn a_run_keeps_its_worlds_size_in_tiles() {
        let o = Outside::new(&medium()).unwrap();
        let citygen::Size { w, h } = WorldParams::default().size.size();
        assert_eq!(
            o.params.size,
            citygen::overworld::WorldSize::Custom { w, h }
        );
        assert_eq!((o.world().size.w, o.world().size.h), (w, h));
    }

    #[test]
    fn a_new_city_forgets_the_world() {
        let mut st = AppState::new();
        st.enter_city(medium());
        ensure(&mut st);
        st.enter_city(medium());
        assert!(st.outside.is_none());
    }

    #[test]
    fn a_new_game_builds_the_large_city() {
        assert_eq!(AppState::new().params.size, SizePreset::Large);
    }
}
