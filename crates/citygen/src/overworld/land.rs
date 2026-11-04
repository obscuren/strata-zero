//! Stages 1 and 2: the sea and its coast, ridges and elevation.

use super::field::{Fbm, distance_field, poly_dist, s};
use super::{Ctx, Ground, Water};
use crate::geom::{Dir, Pos};
use crate::params::GenError;
use rand::Rng;
use std::f32::consts::PI;

/// A bay cut into the coast: its side, where along it, how deep and wide (s-units), and
/// its innermost sea tile.
#[derive(Clone, Debug)]
pub(crate) struct Bay {
    pub side: Dir,
    pub t: f32,
    pub depth: f32,
    pub width: f32,
    pub tip: Pos,
}

/// Where `p` sits against a side of the map: along it (`t`), in from it (`d`), and the
/// side's length and depth (all s-space).
fn against(side: Dir, p: (f32, f32), sw: f32, sh: f32) -> (f32, f32, f32, f32) {
    match side {
        Dir::N => (p.0, p.1, sw, sh),
        Dir::S => (p.0, sh - 1.0 - p.1, sw, sh),
        Dir::W => (p.1, p.0, sh, sw),
        _ => (p.1, sw - 1.0 - p.0, sh, sw),
    }
}

fn turn(d: Dir, cw: bool) -> Dir {
    match (d, cw) {
        (Dir::N, true) | (Dir::S, false) => Dir::E,
        (Dir::E, true) | (Dir::W, false) => Dir::S,
        (Dir::S, true) | (Dir::N, false) => Dir::W,
        _ => Dir::N,
    }
}

struct Coast {
    side: Dir,
    base: f32,
    slope: f32,
    noise: Fbm,
}

pub(crate) fn run(ctx: &mut Ctx) -> Result<(), GenError> {
    sea(ctx);
    ridges(ctx);
    elevation(ctx);
    Ok(())
}

fn sea(ctx: &mut Ctx) {
    let mut rng = ctx.rng(0);
    let count = ctx.params.knobs.sea_sides.unwrap_or_else(|| {
        let r: f32 = rng.random();
        if r < 0.1 {
            0
        } else if r < 0.75 {
            1
        } else {
            2
        }
    });
    let first = Dir::CARDINAL[rng.random_range(0..4)];
    let mut sides = Vec::new();
    if count >= 1 {
        sides.push(first);
    }
    if count >= 2 {
        sides.push(turn(first, rng.random_bool(0.5)));
    }
    let (sw, sh) = (ctx.size.w as f32, 2.0 * ctx.size.h as f32);
    let coasts: Vec<Coast> = sides
        .iter()
        .enumerate()
        .map(|(i, side)| Coast {
            side: *side,
            base: if sides.len() > 1 {
                rng.random_range(0.14..0.2)
            } else {
                rng.random_range(0.16..0.24)
            },
            slope: rng.random_range(-0.08..0.08),
            noise: ctx.fbm(1001 + i as u32),
        })
        .collect();
    let mut bays = Vec::new();
    if !coasts.is_empty() {
        for _ in 0..rng.random_range(1..=3) {
            let side = coasts[rng.random_range(0..coasts.len())].side;
            let (_, _, len, _) = against(side, (0.0, 0.0), sw, sh);
            bays.push(Bay {
                side,
                t: rng.random_range(0.15..0.85) * len,
                depth: rng.random_range(6.0..14.0),
                width: rng.random_range(5.0..8.0),
                tip: Pos::new(0, 0),
            });
        }
    }
    let jitter = ctx.fbm(1002);
    for p in ctx.all() {
        let sp = s(p);
        let j = 4.0 * (jitter.at(sp.0 / 5.0, sp.1 / 5.0, 3) - 0.5);
        let wet = coasts.iter().any(|c| {
            let (t, d, len, extent) = against(c.side, sp, sw, sh);
            let mut depth = c.base * extent
                + 0.045 * extent * (2.0 * c.noise.at(t / 16.0, 0.5, 3) - 1.0)
                + c.slope * (t - len / 2.0);
            for b in bays.iter().filter(|b| b.side == c.side) {
                // A rounded inlet: full depth across its middle, closing in at the mouth.
                let u = ((t - b.t) / (1.6 * b.width)).abs();
                depth += b.depth * (1.0 - u * u).max(0.0).powf(0.6);
            }
            d < depth + j
        });
        if wet {
            ctx.tile_mut(p).water = Water::Sea;
        }
    }
    for b in &mut bays {
        let mut best: Option<(f32, Pos)> = None;
        for p in ctx.all() {
            if ctx.tile(p).water != Water::Sea {
                continue;
            }
            let (t, d, _, _) = against(b.side, s(p), sw, sh);
            if (t - b.t).abs() <= b.width / 2.0 && best.is_none_or(|(bd, _)| d > bd) {
                best = Some((d, p));
            }
        }
        if let Some((_, p)) = best {
            b.tip = p;
        }
    }
    bays.retain(|b| ctx.tile(b.tip).water == Water::Sea);
    ctx.sea_sides = sides;
    ctx.bays = bays;
}

/// How far inland each tile is, 0 at the sea to 1 far from it (0.5 everywhere inland).
fn inland(ctx: &Ctx) -> super::Grid<f32> {
    let reach = 0.6 * (ctx.size.w as f32).max(2.0 * ctx.size.h as f32);
    if ctx.sea_sides.is_empty() {
        return super::Grid::filled(ctx.size, 0.5);
    }
    let d = distance_field(ctx.size, |p| ctx.tile(p).water == Water::Sea, reach);
    d.map(|v| (v / reach).min(1.0))
}

fn ridges(ctx: &mut Ctx) {
    let mut rng = ctx.rng(1);
    let count = ctx
        .params
        .knobs
        .ridges
        .unwrap_or_else(|| if rng.random_bool(0.6) { 1 } else { 2 });
    let far = inland(ctx);
    let (sw, sh) = (ctx.size.w as f32, 2.0 * ctx.size.h as f32);
    let min_in = 0.3 * sw.min(sh) / (0.6 * sw.max(sh));
    let land: Vec<Pos> = ctx
        .all()
        .filter(|p| ctx.tile(*p).water != Water::Sea)
        .collect();
    if land.is_empty() {
        return;
    }
    for _ in 0..count {
        // A short walk (it ran into the sea or the edge) starts again elsewhere.
        for _attempt in 0..12 {
            let pts = walk(ctx, &mut rng, &land, &far, min_in, (sw, sh));
            let length: f32 = pts
                .windows(2)
                .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
                .sum();
            if length >= 30.0 {
                ctx.ridges.push(pts);
                break;
            }
        }
    }
    ctx.ridges.sort_by_key(|r| std::cmp::Reverse(r.len()));
}

/// One ridge: a wandering walk with momentum from a start well inside the land, roughly
/// along the coast, turning away from the sea and the map's edge.
fn walk(
    ctx: &Ctx,
    rng: &mut impl Rng,
    land: &[Pos],
    far: &super::Grid<f32>,
    min_in: f32,
    (sw, sh): (f32, f32),
) -> Vec<(f32, f32)> {
    let mut start = land[rng.random_range(0..land.len())];
    for _ in 0..200 {
        let p = land[rng.random_range(0..land.len())];
        let (px, py) = s(p);
        let inside = px > 0.2 * sw && px < 0.8 * sw && py > 0.2 * sh && py < 0.8 * sh;
        if inside && far[p] >= min_in && (ctx.sea_sides.is_empty() || far[p] < 0.95) {
            start = p;
            break;
        }
    }
    let along = |side: Dir| match side {
        Dir::N | Dir::S => 0.0,
        _ => PI / 2.0,
    };
    let mut heading = match ctx.sea_sides.first() {
        Some(side) => along(*side) + if rng.random_bool(0.5) { PI } else { 0.0 },
        None => rng.random_range(0.0..2.0 * PI),
    } + rng.random_range(-0.7..0.7);
    let mut at = s(start);
    let mut pts = vec![at];
    let mut length = 0.0;
    let want = rng.random_range(40.0..75.0);
    let margin = 8.0;
    let open = |q: (f32, f32)| {
        q.0 >= margin
            && q.1 >= margin
            && q.0 < sw - margin
            && q.1 < sh - margin
            && ctx.tile(Pos::new(q.0 as i32, (q.1 / 2.0) as i32)).water != Water::Sea
    };
    while length < want {
        let step = rng.random_range(10.0..16.0);
        heading += rng.random_range(-0.44..0.44);
        let mut next = None;
        for turn in [0.0, 0.5, -0.5, 1.0, -1.0] {
            let h = heading + turn;
            let q = (at.0 + h.cos() * step, at.1 + h.sin() * step);
            if open(q) {
                heading = h;
                next = Some(q);
                break;
            }
        }
        let Some(q) = next else { break };
        pts.push(q);
        at = q;
        length += step;
    }
    pts
}

fn elevation(ctx: &mut Ctx) {
    let far = inland(ctx);
    let base = ctx.fbm(1003);
    let detail = ctx.fbm(1004);
    let (warp_x, warp_y, girth) = (ctx.fbm(1005), ctx.fbm(1006), ctx.fbm(1007));
    for p in ctx.all() {
        if ctx.tile(p).water == Water::Sea {
            continue;
        }
        let sp = s(p);
        // The ridge line wanders and swells, so the range has spurs and saddles.
        let warped = (
            sp.0 + 10.0 * (warp_x.at(sp.0 / 14.0, sp.1 / 14.0, 3) - 0.5),
            sp.1 + 10.0 * (warp_y.at(sp.0 / 14.0, sp.1 / 14.0, 3) - 0.5),
        );
        let width = 9.5 * (0.7 + 0.6 * girth.at(sp.0 / 20.0, sp.1 / 20.0, 2));
        let m = ctx
            .ridges
            .iter()
            .map(|r| (1.0 - poly_dist(warped, r) / width).max(0.0).powf(1.2))
            .fold(0.0f32, f32::max);
        // Ridged detail: crests and gullies inside the range instead of even bands.
        let ridged = 1.0 - (2.0 * detail.at(sp.0 / 9.0, sp.1 / 9.0, 4) - 1.0).abs();
        let e = 0.4 * base.at(sp.0 / 22.0, sp.1 / 22.0, 4)
            + 0.08 * far[p]
            + 0.82 * m * (0.25 + 0.6 * ridged);
        let t = ctx.tile_mut(p);
        t.elevation = (e.clamp(0.0, 1.0) * 255.0) as u8;
        t.ground = if e > 0.76 {
            Ground::Peak
        } else if e > 0.58 {
            Ground::Mountains
        } else if e > 0.47 {
            Ground::Hills
        } else {
            Ground::Flats
        };
    }
}

#[cfg(test)]
mod tests {
    use crate::overworld::{Ground, Knobs, Overworld, Water, WorldParams, gen_with};

    #[test]
    fn most_of_the_map_is_land_and_the_sea_has_bays() {
        for seed in 0..20 {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            let sea = w.tiles.count(|t| t.water == Water::Sea) as f32 / w.size.area() as f32;
            assert!((0.0..=0.4).contains(&sea), "seed {seed}: sea {sea}");
        }
        let w = gen_with(
            Knobs {
                sea_sides: Some(1),
                ..Default::default()
            },
            3,
        );
        assert!(w.tiles.count(|t| t.water == Water::Sea) > 0);
    }

    #[test]
    fn ridges_make_hills_mountains_and_peaks() {
        for seed in 0..10 {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            assert!(
                w.tiles.count(|t| t.ground == Ground::Mountains) > 20,
                "seed {seed}"
            );
            assert!(
                w.tiles.count(|t| t.ground == Ground::Hills) > 40,
                "seed {seed}"
            );
        }
    }

    #[test]
    fn an_inland_world_has_no_sea() {
        let w = gen_with(
            Knobs {
                sea_sides: Some(0),
                ..Default::default()
            },
            5,
        );
        assert_eq!(w.tiles.count(|t| t.water == Water::Sea), 0);
    }
}
