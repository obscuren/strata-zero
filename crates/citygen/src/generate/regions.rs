//! A city on its site takes its districts from the overworld: each run of its own tiles
//! of one district colour is one district of that theme, so the street, the maps and the
//! overworld's neon agree. A run too small to hold a block or two (a stray tile of
//! docks on the shore) goes to the neighbour it shares the most edge with, and so, once
//! the lots are laid, does a district its streets left no room for a building in.
//! District edges run along tile edges.
//!
//! Everything here counts in whole numbers, so a seed gives the same districts on every
//! platform.

use super::GenCtx;
use super::site::Site;
use crate::geom::{Grid, Pos, Rect, Size};
use crate::ids::DistrictId;
use crate::overworld::detail::TILE;
use crate::world::{StratumKind, Theme};
use std::cmp::Reverse;
use std::collections::VecDeque;

/// The fewest land cells a run of one colour needs to be a district of its own: fewer,
/// and it goes to a neighbour (as a lone city's smallest districts merge, at 600).
pub(crate) const SLIVER: usize = 600;

/// The most districts a city has (as `GenParams` allows): past it, the smallest runs go
/// to their neighbours too.
pub(crate) const MOST: usize = 12;

/// A sited city's districts: which one each overworld tile of its block is in, and each
/// one's theme and centre.
pub(crate) struct Regions {
    /// The block's top-left overworld tile.
    corner: Pos,
    /// Each overworld tile of the block (from its corner): the district it is in. A tile
    /// that isn't the city's own is in the district of the nearest that is.
    of: Grid<u32>,
    pub themes: Vec<Theme>,
    /// A land cell of each district, as near the middle of its land as one is.
    pub centres: Vec<Pos>,
}

impl Regions {
    /// The district cell `p` of the city's map is in.
    pub fn at(&self, origin: Pos, p: Pos) -> u32 {
        let t = tile_of(origin, p);
        self.of
            .get(Pos::new(t.x - self.corner.x, t.y - self.corner.y))
            .copied()
            .unwrap_or(0)
    }
}

/// The overworld tile world-wide detail cell `origin + p` lies on.
fn tile_of(origin: Pos, p: Pos) -> Pos {
    Pos::new(
        (origin.x + p.x).div_euclid(TILE),
        (origin.y + p.y).div_euclid(TILE),
    )
}

/// One run of tiles of one colour, as it grows by merging.
struct Run {
    theme: Theme,
    tiles: Vec<Pos>,
    /// The city's land cells on its tiles.
    land: usize,
    /// The run it went to, once merged.
    into: Option<usize>,
}

/// The districts of a city on `site`, its map `size` cells, from its land.
pub(crate) fn regions(site: &Site, size: Size, land: &Grid<bool>) -> Regions {
    let corner = tile_of(site.origin, Pos::new(0, 0));
    let tiles = Size {
        w: (size.w as i32 + TILE - 1).div_euclid(TILE) as u32,
        h: (size.h as i32 + TILE - 1).div_euclid(TILE) as u32,
    };
    // the colour of each of the city's own tiles in the block (from its corner)
    let colour = |q: Pos| -> Option<Theme> {
        let w = Pos::new(corner.x + q.x, corner.y + q.y);
        if !site.own.get(w).copied().unwrap_or(false) {
            return None;
        }
        site.world.tile(w).and_then(|t| t.urban).map(|u| u.theme)
    };

    // the runs: tiles of one colour joined side to side, found in reading order
    let mut run_of: Grid<Option<usize>> = Grid::filled(tiles, None);
    let mut runs: Vec<Run> = Vec::new();
    for y in 0..tiles.h as i32 {
        for x in 0..tiles.w as i32 {
            let q = Pos::new(x, y);
            let Some(theme) = colour(q) else { continue };
            if run_of[q].is_some() {
                continue;
            }
            let r = runs.len();
            run_of[q] = Some(r);
            let mut found = vec![q];
            let mut i = 0;
            while i < found.len() {
                let at = found[i];
                i += 1;
                for n in at.neighbors4() {
                    if run_of.get(n).is_some_and(|o| o.is_none()) && colour(n) == Some(theme) {
                        run_of[n] = Some(r);
                        found.push(n);
                    }
                }
            }
            runs.push(Run {
                theme,
                tiles: found,
                land: 0,
                into: None,
            });
        }
    }
    for (p, l) in land.iter() {
        let t = tile_of(site.origin, p);
        let q = Pos::new(t.x - corner.x, t.y - corner.y);
        if *l && let Some(Some(r)) = run_of.get(q) {
            runs[*r].land += 1;
        }
    }

    // slivers go to their neighbours, the smallest first; then, past the most a city
    // has, the smallest of all
    loop {
        let live: Vec<usize> = (0..runs.len())
            .filter(|r| runs[*r].into.is_none())
            .collect();
        if live.len() <= 1 {
            break;
        }
        let size_of = |r: &usize| (runs[*r].land, runs[*r].tiles.len(), *r);
        let sliver = live
            .iter()
            .copied()
            .filter(|r| runs[*r].land < SLIVER)
            .min_by_key(size_of);
        let crowded = (live.len() > MOST)
            .then(|| live.iter().copied().min_by_key(size_of))
            .flatten();
        let Some(s) = sliver.or(crowded) else {
            break;
        };
        let to = neighbour(&runs, &run_of, &live, s);
        let moved = std::mem::take(&mut runs[s].tiles);
        for q in &moved {
            run_of[*q] = Some(to);
        }
        runs[to].tiles.extend(moved);
        runs[to].land += runs[s].land;
        runs[s].into = Some(to);
    }

    // each live run a district, in the order they were found
    let live: Vec<usize> = (0..runs.len())
        .filter(|r| runs[*r].into.is_none())
        .collect();
    let themes: Vec<Theme> = live.iter().map(|r| runs[*r].theme).collect();
    let mut of: Grid<Option<u32>> = Grid::filled(tiles, None);
    let mut queue: VecDeque<Pos> = VecDeque::new();
    for (q, r) in run_of.iter() {
        if let Some(r) = r {
            let d = live.iter().position(|l| l == r).unwrap_or(0) as u32;
            of[q] = Some(d);
            queue.push_back(q);
        }
    }
    // every other tile of the block: the district of the nearest own tile
    while let Some(q) = queue.pop_front() {
        let d = of[q];
        for n in q.neighbors4() {
            if of.get(n).is_some_and(|o| o.is_none()) {
                of[n] = d;
                queue.push_back(n);
            }
        }
    }
    let of = of.map(|d| d.unwrap_or(0));

    // centres: the land cell of each district nearest the middle of its land
    let n = themes.len().max(1);
    let mut sums = vec![(0i64, 0i64, 0i64); n];
    let at = |p: Pos| {
        let t = tile_of(site.origin, p);
        of.get(Pos::new(t.x - corner.x, t.y - corner.y))
            .copied()
            .unwrap_or(0) as usize
    };
    for (p, l) in land.iter() {
        if *l {
            let s = &mut sums[at(p)];
            *s = (s.0 + p.x as i64, s.1 + p.y as i64, s.2 + 1);
        }
    }
    let middle: Vec<Pos> = sums
        .iter()
        .map(|(x, y, c)| match c {
            0 => Pos::new(0, 0),
            c => Pos::new((x / c) as i32, (y / c) as i32),
        })
        .collect();
    let mut best: Vec<Option<(i64, Pos)>> = vec![None; n];
    for (p, l) in land.iter() {
        if !*l {
            continue;
        }
        let d = at(p);
        let far = p.dist2(middle[d]);
        if best[d].is_none_or(|(b, _)| far < b) {
            best[d] = Some((far, p));
        }
    }
    let centres = best
        .iter()
        .enumerate()
        .map(|(d, b)| {
            b.map(|(_, p)| p).unwrap_or_else(|| {
                // a district with no land: the middle of its first tile
                let q = of
                    .iter()
                    .find(|(_, v)| **v as usize == d)
                    .map_or(Pos::new(0, 0), |(q, _)| q);
                Pos::new(
                    (q.x * TILE + TILE / 2).min(size.w as i32 - 1),
                    (q.y * TILE + TILE / 2).min(size.h as i32 - 1),
                )
            })
        })
        .collect();
    Regions {
        corner,
        of,
        themes: if themes.is_empty() {
            vec![Theme::OldTown]
        } else {
            themes
        },
        centres,
    }
}

/// Once the lots are laid: a district of a city on its site that its streets left no
/// room for a building in (a small city's core, three tiles tall, where the arterials
/// all meet) goes to the neighbour it shares the most edge with, as a sliver does. Its
/// streets stay as they are, in the neighbour's colours. Lone cities are left as they
/// are.
pub(crate) fn merge_unbuilt(ctx: &mut GenCtx) {
    if ctx.scratch.site.is_none() {
        return;
    }
    loop {
        let n = ctx.city.districts.len();
        if n <= 1 {
            return;
        }
        let mut built = vec![false; n];
        for b in &ctx.city.buildings {
            built[b.district.index()] = true;
        }
        let Some(empty) = (0..n).find(|d| !built[*d]) else {
            return;
        };
        // the edge it shares with each other district, on the street
        let mut shared = vec![0usize; n];
        let st = &ctx
            .city
            .stratum(StratumKind::Street)
            .expect("street stratum")
            .tiles;
        for (p, t) in st.iter() {
            if t.district.is_some_and(|d| d.index() == empty) {
                for q in p.neighbors4() {
                    if let Some(d) = st.get(q).and_then(|u| u.district)
                        && d.index() != empty
                    {
                        shared[d.index()] += 1;
                    }
                }
            }
        }
        let centre = ctx.city.districts[empty].center;
        let to = (0..n)
            .filter(|d| *d != empty)
            .max_by_key(|d| {
                (
                    shared[*d],
                    built[*d],
                    Reverse(ctx.city.districts[*d].center.dist2(centre)),
                    Reverse(*d),
                )
            })
            .expect("another district");
        // the empty one's cells are the neighbour's; the districts after it move down one
        let to = if to > empty { to - 1 } else { to };
        let remap = |d: DistrictId| match d.index() {
            i if i == empty => DistrictId(to as u32),
            i if i > empty => DistrictId(i as u32 - 1),
            _ => d,
        };
        for k in StratumKind::PHYSICAL {
            if let Some(s) = ctx.city.stratum_mut(k) {
                for t in s.tiles.as_mut_slice() {
                    t.district = t.district.map(remap);
                }
            }
        }
        for b in &mut ctx.city.buildings {
            b.district = remap(b.district);
        }
        for b in &mut ctx.scratch.blocks {
            b.district = remap(b.district);
        }
        for l in &mut ctx.scratch.lots {
            l.district = remap(l.district);
        }
        let gone = ctx.city.districts.remove(empty);
        for (i, d) in ctx.city.districts.iter_mut().enumerate() {
            d.id = DistrictId(i as u32);
        }
        let into = &mut ctx.city.districts[to];
        let (x0, y0) = (
            into.bounds.x.min(gone.bounds.x),
            into.bounds.y.min(gone.bounds.y),
        );
        let x1 = into.bounds.right().max(gone.bounds.right());
        let y1 = into.bounds.bottom().max(gone.bounds.bottom());
        into.bounds = Rect::new(x0, y0, x1 - x0, y1 - y0);
    }
}

/// The live run that run `s` goes to: the one it shares the most tile edges with (the
/// larger, then the first found, on a tie), else the nearest.
fn neighbour(runs: &[Run], run_of: &Grid<Option<usize>>, live: &[usize], s: usize) -> usize {
    let mut shared = vec![0usize; runs.len()];
    for q in &runs[s].tiles {
        for n in q.neighbors4() {
            if let Some(Some(r)) = run_of.get(n)
                && *r != s
            {
                shared[*r] += 1;
            }
        }
    }
    let others = || live.iter().copied().filter(|r| *r != s);
    if let Some(to) = others()
        .filter(|r| shared[*r] > 0)
        .max_by_key(|r| (shared[*r], runs[*r].land, Reverse(*r)))
    {
        return to;
    }
    others()
        .min_by_key(|r| {
            let gap = runs[s]
                .tiles
                .iter()
                .flat_map(|a| runs[*r].tiles.iter().map(move |b| a.manhattan(*b)))
                .min()
                .unwrap_or(i32::MAX);
            (gap, Reverse(runs[*r].land), *r)
        })
        .unwrap_or(s)
}
