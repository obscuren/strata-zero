//! Stage 4: ground cover from moisture and the rivers, and the dead ground: a toxic
//! spill near enough to a city to shape its edge, a salt flat out in the open.

use super::field::{Fbm, distance_field, length, s, sdist};
use super::{Ctx, Ground, Water};
use crate::geom::Pos;
use crate::params::GenError;
use rand::Rng;

pub(crate) fn run(ctx: &mut Ctx) -> Result<(), GenError> {
    let mut rng = ctx.rng(3);
    cover(ctx);
    let spill_near = if rng.random_bool(0.6) || ctx.cities.len() < 2 {
        0
    } else {
        rng.random_range(1..ctx.cities.len())
    };
    if let Some(c) = ctx.cities.get(spill_near).cloned() {
        let spots = open_spots(ctx, |p| {
            let d = sdist(p, c.centre);
            d >= c.size + 6.0 && d <= c.size + 16.0
        });
        if !spots.is_empty() {
            let at = spots[rng.random_range(0..spots.len())];
            patch(ctx, at, (8.0, 5.0), 0.8, 4.0, 1.3, Ground::Spill, 1203);
            ctx.spill = Some(at);
        }
    }
    for far in [25.0, 15.0, 8.0] {
        let spill = ctx.spill;
        let cities = ctx.cities.clone();
        let spots = open_spots(ctx, |p| {
            cities.iter().all(|c| sdist(p, c.centre) >= far)
                && spill.is_none_or(|q| sdist(p, q) >= 20.0)
        });
        if !spots.is_empty() {
            let at = spots[rng.random_range(0..spots.len())];
            patch(ctx, at, (11.0, 5.5), 0.7, 5.0, 1.25, Ground::Salt, 1204);
            ctx.salt = Some(at);
            break;
        }
    }
    Ok(())
}

fn cover(ctx: &mut Ctx) {
    let moist = ctx.fbm(1201);
    let fields = ctx.fbm(1202);
    let river = distance_field(ctx.size, |p| ctx.tile(p).water == Water::River, 8.0);
    let cities = ctx.cities.clone();
    for p in ctx.all() {
        let t = ctx.tile(p);
        if t.water != Water::None || t.ground != Ground::Flats {
            continue;
        }
        let (x, y) = s(p);
        let m = moist.at(x / 18.0, y / 18.0, 3);
        let near_city = cities.iter().any(|c| sdist(p, c.centre) < 3.0 * c.size);
        let ground = if river[p] <= 3.0 && m > 0.42 {
            if fields.at(x / 3.0, y / 3.0, 2) > 0.45 && near_city {
                Ground::Fields
            } else {
                Ground::Scrub
            }
        } else if m > 0.6 {
            Ground::Scrub
        } else {
            Ground::Flats
        };
        ctx.tile_mut(p).ground = ground;
    }
}

/// Open lowland tiles with open lowland round them, where `ok` holds.
fn open_spots(ctx: &Ctx, ok: impl Fn(Pos) -> bool) -> Vec<Pos> {
    let lowland = |p: Pos| {
        ctx.in_bounds(p)
            && ctx.tile(p).water == Water::None
            && matches!(
                ctx.tile(p).ground,
                Ground::Flats | Ground::Scrub | Ground::Fields
            )
    };
    let centres: Vec<Pos> = ctx.cities.iter().map(|c| c.centre).collect();
    ctx.all()
        .filter(|p| {
            (-3..=3).all(|dx| (-1..=1).all(|dy| lowland(p.offset(dx, dy))))
                && !centres.iter().any(|c| sdist(*p, *c) < 4.0)
                && ok(*p)
        })
        .collect()
}

/// An irregular patch of `ground` round `at` (radii in s-units), on lowland only and
/// never over a city's centre.
#[allow(clippy::too_many_arguments)]
fn patch(
    ctx: &mut Ctx,
    at: Pos,
    (rx, ry): (f32, f32),
    wobble: f32,
    scale: f32,
    reach: f32,
    ground: Ground,
    salt: u32,
) {
    let shape: Fbm = ctx.fbm(salt);
    let (cx, cy) = s(at);
    let centres: Vec<Pos> = ctx.cities.iter().map(|c| c.centre).collect();
    for p in ctx.all() {
        let (x, y) = s(p);
        let d = length((x - cx) / rx, (y - cy) / ry) + wobble * shape.at(x / scale, y / scale, 3);
        let t = ctx.tile(p);
        if d < reach
            && t.water == Water::None
            && matches!(t.ground, Ground::Flats | Ground::Scrub | Ground::Fields)
            && !centres.iter().any(|c| sdist(p, *c) < 3.0)
        {
            ctx.tile_mut(p).ground = ground;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::overworld::{Ground, Overworld, Water, WorldParams};

    #[test]
    fn each_world_has_one_spill_and_one_salt_flat() {
        for seed in 0..10 {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            assert!(
                w.tiles.count(|t| t.ground == Ground::Spill) > 10,
                "seed {seed}"
            );
            assert!(
                w.tiles.count(|t| t.ground == Ground::Salt) > 10,
                "seed {seed}"
            );
            let dead_water = w.tiles.count(|t| {
                matches!(t.ground, Ground::Spill | Ground::Salt) && t.water != Water::None
            });
            assert_eq!(dead_water, 0, "seed {seed}: dead ground on water");
        }
    }
}
