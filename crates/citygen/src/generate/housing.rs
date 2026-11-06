//! A city on its site houses who works in it. Its districts are its colours on the
//! overworld, and docks, industry and a corp core build no homes of their own, so a
//! city of those alone had nowhere for its people to live. And people live only where
//! they can walk to work, so each stretch of street a walk covers needs its own.
//!
//! Once the lots are laid, every stretch of a city or town on its site where people
//! work has beds for at least half the people it employs (its buildings' staff and its
//! streets' at their most), in two buildings or more once it employs `TOWN` or more.
//! Homes go in one house at a time until it does: flats on an empty lot of the stretch
//! first (a square or a car park, so no job goes), then a building on it turned over,
//! those that fit their district first (a garage or a shop; on the docks and in
//! industry, a warehouse or a works as its workers' flats; in a corp core, an office or
//! a tower as a hotel of company rooms), in the city before its outskirts, each as far
//! from the homes already there as one can be. Nobody lives on the Strip, so a city
//! all strip has no homes. The home city and lone cities are left as they are.

use super::{GenCtx, LotUse};
use crate::geom::{Grid, Pos, Rect};
use crate::ids::{BuildingId, DistrictId};
use crate::tile::{Feature, Terrain, Tile};
use crate::world::{Building, BuildingKind, City, StratumKind, Theme};
use rand_chacha::ChaCha8Rng;
use std::cmp::Reverse;
use std::collections::VecDeque;

/// The people a city employs from which it has two housing buildings or more.
pub(crate) const TOWN: usize = 40;

/// The most people a building of `kind` keeps at work over a day: what the people
/// pack (`flavor/people.ron`) staffs it with at full density, every shift, at the top
/// of each range (a warehouse's line workers three shifts of four, an office's drones
/// and its door), and a megablock's gangers on the street round it.
pub fn staff(kind: BuildingKind) -> usize {
    use BuildingKind as B;
    match kind {
        B::Depot => 15,
        B::Office | B::CorpTower | B::Factory | B::Warehouse => 12,
        B::Megablock => 9,
        B::Club => 5,
        B::Bar => 3,
        B::Shop | B::Market | B::Pawn | B::NoodleBar | B::Temple => 2,
        B::Clinic => 1,
        _ => 0,
    }
}

/// The most people a district of `theme` keeps at work on its streets: the strip's
/// sex workers on their beat and its two fixers; a night market's, whose alleys are
/// the beat where there is no strip, and its fixer.
fn street(theme: Theme) -> usize {
    match theme {
        Theme::NeonStrip => 10,
        Theme::NightMarket => 9,
        _ => 0,
    }
}

/// How many a building houses, as the people's sim counts its room: an apartment
/// house's flats, a megablock's by its size, a hotel's rooms; none on the Strip, whose
/// rooms are let by the hour, nor in any other kind.
pub fn beds(city: &crate::world::City, b: &Building) -> usize {
    if city
        .district_opt(b.district)
        .is_some_and(|d| d.theme == Theme::NeonStrip)
    {
        return 0;
    }
    match b.kind {
        BuildingKind::Apartment => 8,
        BuildingKind::Megablock => (b.footprint.w as usize * b.footprint.h as usize / 6).max(30),
        BuildingKind::Hotel => 6,
        _ => 0,
    }
}

/// Which buildings may be turned over to homes, the first to go first: none with a job
/// (a garage, an arcade), then a shop's few, then the docks' and industry's works, then
/// an office or a tower, and last a bar or a noodle bar. A clinic stays the doctor's.
fn order(kind: BuildingKind) -> Option<u8> {
    use BuildingKind as B;
    match kind {
        B::Garage | B::Arcade => Some(0),
        B::Shop | B::Pawn | B::Market => Some(1),
        B::Warehouse | B::Factory | B::Depot => Some(2),
        B::Office | B::CorpTower => Some(3),
        B::Bar | B::NoodleBar | B::Temple | B::Parking => Some(4),
        _ => None,
    }
}

/// What building `b` is turned over to: flats, or a tower too tall for flats (up to
/// the heights) a hotel of company rooms, as a corp core has.
fn homes_in(b: &Building) -> BuildingKind {
    if b.span.contains(StratumKind::Heights) {
        BuildingKind::Hotel
    } else {
        BuildingKind::Apartment
    }
}

/// Whether a walker can stand on `t` as the lots stand: the street's pavements, roads,
/// squares and bridges, as the people's sim walks them (`sim::walk_cost`; a sign stands
/// in the way). Doors and stairs come later and only join a building to its street.
fn walked(t: &Tile) -> bool {
    !matches!(t.feature, Some(Feature::Sign(_)))
        && matches!(
            t.terrain,
            Terrain::Sidewalk
                | Terrain::Plaza
                | Terrain::Alley
                | Terrain::Walkway
                | Terrain::Bridge
                | Terrain::Stair
                | Terrain::Door
                | Terrain::Road
        )
}

/// The city's stretches of street: each cell a walker can stand on numbered (from 1) by
/// the stretch a walk from it covers, 0 elsewhere. A lobe of the city joined to the rest
/// only over the world's ground (a field, reeds, a sandbank) is a stretch of its own:
/// its people can't walk to work from the rest of town.
fn stretches(city: &City) -> Grid<u32> {
    let mut grid = Grid::filled(city.size, 0u32);
    let Some(street) = city.stratum(StratumKind::Street) else {
        return grid;
    };
    let mut n = 0;
    for (p, t) in street.tiles.iter() {
        if grid[p] != 0 || !walked(t) {
            continue;
        }
        n += 1;
        grid[p] = n;
        let mut queue = VecDeque::from([p]);
        while let Some(q) = queue.pop_front() {
            for r in q.neighbors4() {
                if grid.get(r) == Some(&0) && street.tiles.get(r).is_some_and(walked) {
                    grid[r] = n;
                    queue.push_back(r);
                }
            }
        }
    }
    grid
}

/// The stretch of street `r` (a building's footprint, or a lot) opens onto: of the cells
/// in it and round it a walker stands on, the stretch most are on (the first on a tie);
/// 0 when none.
fn stretch_of(grid: &Grid<u32>, r: Rect) -> u32 {
    let mut on: Vec<(u32, usize)> = Vec::new();
    for p in r.grow(1).iter() {
        let s = grid.get(p).copied().unwrap_or(0);
        if s == 0 {
            continue;
        }
        match on.iter_mut().find(|(t, _)| *t == s) {
            Some((_, n)) => *n += 1,
            None => on.push((s, 1)),
        }
    }
    on.iter()
        .max_by_key(|(s, n)| (*n, Reverse(*s)))
        .map_or(0, |(s, _)| *s)
}

/// Whether a house on `r` has somewhere for its street door: a side of it (not a
/// corner) facing a road, a pavement, an alley or a square. A lot can back onto ground
/// the city leaves to the world.
fn fronts_the_street(city: &City, r: Rect) -> bool {
    r.border().any(|p| {
        let corner = (p.x == r.x || p.x == r.right() - 1) && (p.y == r.y || p.y == r.bottom() - 1);
        !corner
            && city
                .tile(StratumKind::Street, super::outward(r, p))
                .is_some_and(|t| t.is_outdoor_ground())
    })
}

/// Where a city's next flats go: up on an empty lot, or into a building turned over.
#[derive(Clone, Copy)]
enum Flats {
    Lot(usize),
    Building(BuildingId),
}

/// One stretch of street's jobs, beds and housing buildings, and whether it has run
/// out of places for more.
#[derive(Clone, Default)]
struct Stretch {
    jobs: usize,
    beds: usize,
    homes: Vec<usize>,
    full: bool,
    /// The middle of its cells, and how many.
    middle: (i64, i64, i64),
}

impl Stretch {
    /// Whether it needs more homes: beds for half its jobs, in two buildings once it
    /// has `TOWN` jobs or more, one at least for any job at all.
    fn short(&self) -> bool {
        let wanted = if self.jobs >= TOWN { 2 } else { 1 };
        self.jobs > 0 && !self.full && (2 * self.beds < self.jobs || self.homes.len() < wanted)
    }
}

/// Gives a city on its site the homes its people need (see the module). The home city
/// is left as it is: its people, the story's, keep their commute, and it has its
/// blocks and its flats.
pub(crate) fn ensure(ctx: &mut GenCtx, rng: &mut ChaCha8Rng) {
    let (Some(site), Some(at)) = (ctx.scratch.site.as_ref(), ctx.params().site.as_ref()) else {
        return;
    };
    let home = site
        .world
        .cities
        .get(at.city as usize)
        .is_some_and(|c| c.home);
    if home && at.town.is_none() {
        return;
    }
    let city = &ctx.city;
    let grid = stretches(city);
    let mut on: Vec<u32> = city
        .buildings
        .iter()
        .map(|b| stretch_of(&grid, b.footprint))
        .collect();
    let count = grid.iter().map(|(_, s)| *s as usize).max().unwrap_or(0) + 1;
    let mut all = vec![Stretch::default(); count];
    for (i, b) in city.buildings.iter().enumerate() {
        let s = &mut all[on[i] as usize];
        s.jobs += staff(b.kind);
        if beds(city, b) > 0 {
            s.beds += beds(city, b);
            s.homes.push(i);
        }
    }
    // a district's street work on the stretch most of its street is
    for d in &city.districts {
        let mut by: Vec<usize> = vec![0; count];
        for (p, s) in grid.iter() {
            if *s != 0 && city.tile(StratumKind::Street, p).and_then(|t| t.district) == Some(d.id) {
                by[*s as usize] += 1;
            }
        }
        if let Some(s) = (1..count).max_by_key(|s| (by[*s], Reverse(*s))) {
            all[s].jobs += street(d.theme);
        }
    }
    all[0] = Stretch::default();
    for (p, s) in grid.iter() {
        let m = &mut all[*s as usize].middle;
        *m = (m.0 + p.x as i64, m.1 + p.y as i64, m.2 + 1);
    }
    let middle = |s: &Stretch| {
        let (x, y, n) = s.middle;
        Pos::new((x / n.max(1)) as i32, (y / n.max(1)) as i32)
    };
    // the first stretch short of homes, until none is or none can be helped
    while let Some(s) = (1..count).find(|s| all[*s].short()) {
        let Some(next) = next(ctx, &grid, &on, s as u32, &all[s].homes) else {
            // nowhere on it for homes (all strip, say): those it can't house look to
            // the nearest stretch that can, which a bridge the street's repairs lay
            // later may join to it
            all[s].full = true;
            let short = all[s].jobs.saturating_sub(2 * all[s].beds);
            let at = middle(&all[s]);
            let near = (1..count)
                .filter(|t| !all[*t].full && all[*t].middle.2 > 0)
                .min_by_key(|t| (middle(&all[*t]).manhattan(at), *t));
            if let Some(t) = near {
                all[t].jobs += short;
            }
            continue;
        };
        let id = match next {
            Flats::Lot(i) => {
                let lot = ctx.scratch.lots.remove(i);
                let spec = ctx.city.district(lot.district).theme.spec();
                let id = ctx.city.buildings.len();
                super::lots::place_building(
                    ctx,
                    rng,
                    lot.rect,
                    BuildingKind::Apartment,
                    spec,
                    lot.district,
                );
                on.push(s as u32);
                id
            }
            Flats::Building(id) => {
                let b = &mut ctx.city.buildings[id.index()];
                let to = homes_in(b);
                all[s].jobs -= staff(b.kind);
                b.kind = to;
                let corner = Pos::new(b.footprint.x, b.footprint.y);
                for lot in &mut ctx.scratch.lots {
                    if let LotUse::Building { kind, .. } = &mut lot.use_
                        && lot.rect.contains(corner)
                    {
                        *kind = to;
                    }
                }
                id.index()
            }
        };
        all[s].beds += beds(&ctx.city, &ctx.city.buildings[id]);
        all[s].homes.push(id);
    }
}

/// Where stretch `s`'s next flats go, off the Strip: an empty lot of the city's on it (a
/// square, then a car park), else a building on it that may become flats (`order`), in
/// the city before its outskirts; of those first in line, the one farthest from the
/// `homes` there.
fn next(ctx: &GenCtx, grid: &Grid<u32>, on: &[u32], s: u32, homes: &[usize]) -> Option<Flats> {
    let (city, site) = (&ctx.city, ctx.scratch.site.as_ref()?);
    let off_strip = |d: DistrictId| {
        city.district_opt(d)
            .is_some_and(|d| d.theme != Theme::NeonStrip)
    };
    let far = |c: Pos| {
        homes
            .iter()
            .map(|h| city.buildings[*h].footprint.center().manhattan(c))
            .min()
            .unwrap_or(0)
    };
    let lots = ctx
        .scratch
        .lots
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            l.rect.w >= 5
                && l.rect.h >= 4
                && off_strip(l.district)
                && !site.outskirts(l.rect.center())
                && stretch_of(grid, l.rect) == s
                && fronts_the_street(city, l.rect)
        })
        .filter_map(|(i, l)| {
            let car_park = match l.use_ {
                LotUse::Plaza => 0,
                LotUse::Parking => 1,
                LotUse::Building { .. } => return None,
            };
            Some((
                (0, car_park, Reverse(far(l.rect.center())), i),
                Flats::Lot(i),
            ))
        });
    let buildings = city
        .buildings
        .iter()
        .zip(on)
        .filter(|(b, on)| {
            **on == s
                && b.footprint.w >= 5
                && b.footprint.h >= 4
                && off_strip(b.district)
                && fronts_the_street(city, b.footprint)
        })
        .filter_map(|(b, _)| {
            let o = order(b.kind)?;
            let c = b.footprint.center();
            let edge = 1 + u8::from(site.outskirts(c));
            Some((
                (edge, o, Reverse(far(c)), b.id.index()),
                Flats::Building(b.id),
            ))
        });
    lots.chain(buildings)
        .min_by_key(|(key, _)| *key)
        .map(|(_, next)| next)
}
