//! The open ground, a layer of the stream (`stream::Stream`): the detail world built
//! tile by tile round the view and kept while near, lit as a city's street is lit; and
//! what you've seen of it, remembered with the run.

use super::{Outside, TILE};
use citygen::overworld::{Overworld, detail};
use citygen::{Grid, Pos, Rect, StratumKind, Tile};
use std::collections::HashMap;

/// A ring of tiles built beyond what's on screen and in sight, so the ground is there
/// before you see it.
pub const AHEAD: i32 = 1;

/// The most tiles a view holds before it forgets the far ones.
pub const CAP: usize = 2048;

/// The open ground round you, tile by tile: each overworld tile's cells built once and
/// kept while near (a tile built alone is the same tile in any region, so they fit);
/// not saved, made again from the world.
#[derive(Default)]
pub struct GroundView {
    tiles: HashMap<Pos, Built>,
    /// The Net's layer rather than the ground's.
    net: bool,
}

/// One overworld tile as a view keeps it.
struct Built {
    cells: Grid<Tile>,
    /// The light it throws on its own, as a city's street bakes it; none on the Net.
    light: Option<Light>,
}

/// A street tile's own light, ambient excluded: from what burns all day (a building's
/// windows), and from its lamps, which burn by night.
struct Light {
    steady: Grid<u8>,
    lamps: Grid<u8>,
}

impl GroundView {
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
            // the street is built a tile wider all round and lit whole as a city's street
            // is lit, then cropped: a light past the batch's edge reaches into it as it
            // would in any other batch
            let m = if self.net { 0 } else { TILE };
            let g = if self.net {
                citygen::overworld::net_ground::region(world, x, y, w, h)
            } else {
                detail::region(world, x - m, y - m, w + 2 * m as u32, h + 2 * m as u32)
            };
            let light = (!self.net).then(|| {
                use citygen::lighting::{compute, tuning};
                let blocks = |p: Pos| g.get(p).is_none_or(|t| t.blocks_sight());
                Light {
                    steady: compute(&g, 0, blocks, |t| {
                        tuning::steady_emission(t.terrain, t.feature, StratumKind::Street)
                    }),
                    lamps: compute(&g, 0, blocks, |t| tuning::lamp_emission(t.feature)),
                }
            });
            for t in missing {
                let from = Pos::new(m + TILE * (t.x - lo.x), m + TILE * (t.y - lo.y));
                let built = Built {
                    cells: cut(&g, from),
                    light: light.as_ref().map(|l| Light {
                        steady: cut(&l.steady, from),
                        lamps: cut(&l.lamps, from),
                    }),
                };
                self.tiles.insert(t, built);
            }
        }
        // keep what's near: a few tiles past the area, the rest forgotten
        if self.tiles.len() > CAP {
            let keep = 4;
            self.tiles.retain(|t, _| {
                t.x >= t0.x - keep && t.x <= t1.x + keep && t.y >= t0.y - keep && t.y <= t1.y + keep
            });
        }
    }

    /// The tile at world-wide cell `p`, if it is built.
    pub fn tile(&self, p: Pos) -> Option<&Tile> {
        let t = Pos::new(p.x.div_euclid(TILE), p.y.div_euclid(TILE));
        self.tiles
            .get(&t)?
            .cells
            .get(Pos::new(p.x.rem_euclid(TILE), p.y.rem_euclid(TILE)))
    }

    /// The light world-wide cell `p` throws on its own, ambient excluded: what burns all
    /// day, and the lamps while `lamps_on`, as a city's street gives it
    /// (`Stratum::emitted`). 0 where it isn't built, and on the Net.
    pub fn emitted(&self, p: Pos, lamps_on: bool) -> u8 {
        let t = Pos::new(p.x.div_euclid(TILE), p.y.div_euclid(TILE));
        let at = Pos::new(p.x.rem_euclid(TILE), p.y.rem_euclid(TILE));
        let Some(l) = self.tiles.get(&t).and_then(|b| b.light.as_ref()) else {
            return 0;
        };
        let lamps = if lamps_on { l.lamps[at] } else { 0 };
        l.steady[at].saturating_add(lamps)
    }

    /// How many overworld tiles the view holds.
    pub fn held(&self) -> usize {
        self.tiles.len()
    }
}

/// The overworld tile's worth of `g` from `from` on.
fn cut<T: Copy>(g: &Grid<T>, from: Pos) -> Grid<T> {
    let size = citygen::Size {
        w: TILE as u32,
        h: TILE as u32,
    };
    let mut out = Grid::filled(size, g[from]);
    for y in 0..TILE {
        for x in 0..TILE {
            out[Pos::new(x, y)] = g[from.offset(x, y)];
        }
    }
    out
}

/// Marks world-wide cell `p` as seen.
pub fn remember_cell(o: &mut Outside, p: Pos) {
    let (chunk, bit) = cell_bit(p);
    *o.seen.entry(chunk).or_insert(0) |= 1 << bit;
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
