//! Stage 5: city sites. Cities get built where water meets flat ground: a river mouth on
//! a bay first, then the coast, river banks, lake shores, the foot of the hills.

use super::field::{distance_field, roll, s, sdist};
use super::{Ctx, Ground, Water, WorldCity};
use crate::geom::Pos;
use crate::params::GenError;
use rand::Rng;

const HOME_SIZE: f32 = 16.0;
const TOWN_SIZE: f32 = 5.0;

pub(crate) fn run(ctx: &mut Ctx) -> Result<(), GenError> {
    let mut rng = ctx.rng(4);
    let scored = scores(ctx);
    let n = ctx.params.cities as usize;
    let middles: Vec<f32> = (0..n.saturating_sub(2))
        .map(|_| rng.random_range(7.0..=10.0))
        .collect();
    let wet = |p: Pos| {
        (-4..=4)
            .flat_map(|dx| (-2..=2).map(move |dy| (dx, dy)))
            .any(|(dx, dy)| {
                let q = p.offset(dx, dy);
                ctx.in_bounds(q) && ctx.tile(q).water != Water::None
            })
    };
    let Some(&(_, home)) = scored
        .iter()
        .find(|(_, p)| wet(*p))
        .or_else(|| scored.first())
    else {
        return Ok(());
    };
    // The city spans its shore: its centre sits a little inland, by the water still.
    let sea = distance_field(ctx.size, |p| ctx.tile(p).water == Water::Sea, 20.0);
    let home = ctx
        .all()
        .filter(|p| {
            let d = sdist(*p, home);
            let t = ctx.tile(*p);
            (4.0..=8.0).contains(&d)
                && t.water == Water::None
                && !matches!(t.ground, Ground::Hills | Ground::Mountains | Ground::Peak)
                && wet(*p)
        })
        .max_by(|a, b| {
            sea[*a]
                .total_cmp(&sea[*b])
                .then(b.y.cmp(&a.y))
                .then(b.x.cmp(&a.x))
        })
        .filter(|p| sea[*p] > sea[home])
        .unwrap_or(home);
    let mut spacing = 38.0f32;
    let mut cities = vec![(home, HOME_SIZE)];
    loop {
        cities.truncate(1);
        let fits = |p: Pos, size: f32, placed: &[(Pos, f32)], k: f32| {
            placed
                .iter()
                .all(|(q, qs)| sdist(p, *q) >= (spacing.max(2.4 * size.max(*qs)) * k).min(spacing))
        };
        for size in &middles {
            if let Some(&(_, p)) = scored.iter().find(|(_, p)| fits(*p, *size, &cities, 1.0)) {
                cities.push((p, *size));
            }
        }
        // The town sits between two cities, where a road will cross.
        let town_fits = |p: Pos, placed: &[(Pos, f32)]| {
            placed
                .iter()
                .all(|(q, qs)| sdist(p, *q) >= (2.4 * qs).min(spacing))
        };
        let between = |p: Pos| {
            cities.iter().enumerate().any(|(i, (a, _))| {
                cities[i + 1..].iter().any(|(b, _)| {
                    let mid = Pos::new((a.x + b.x) / 2, (a.y + b.y) / 2);
                    sdist(p, mid) <= 15.0
                })
            })
        };
        let town = scored
            .iter()
            .find(|(_, p)| between(*p) && town_fits(*p, &cities))
            .or_else(|| scored.iter().find(|(_, p)| town_fits(*p, &cities)))
            .map(|(_, p)| *p);
        if let Some(p) = town
            && n >= 2
        {
            cities.push((p, TOWN_SIZE));
        }
        if cities.len() >= n || spacing <= 12.0 {
            break;
        }
        spacing = (spacing * 0.85).max(12.0);
    }
    ctx.cities = cities
        .into_iter()
        .enumerate()
        .map(|(i, (centre, size))| WorldCity {
            name: String::new(),
            centre,
            size,
            home: i == 0,
            towns: Vec::new(),
        })
        .collect();
    Ok(())
}

/// Every tile a city could stand on, best first.
fn scores(ctx: &Ctx) -> Vec<(f32, Pos)> {
    let size = ctx.size;
    let sea = distance_field(size, |p| ctx.tile(p).water == Water::Sea, 12.0);
    let lake = distance_field(size, |p| ctx.tile(p).water == Water::Lake, 8.0);
    let river = distance_field(size, |p| ctx.tile(p).water == Water::River, 8.0);
    let mouth = distance_field(size, |p| ctx.mouths.contains(&p), 8.0);
    let high = distance_field(
        size,
        |p| {
            matches!(
                ctx.tile(p).ground,
                Ground::Hills | Ground::Mountains | Ground::Peak
            )
        },
        12.0,
    );
    let (w, h) = (size.w as i32, size.h as i32);
    let mut out: Vec<(f32, Pos)> = ctx
        .all()
        .filter(|p| {
            let t = ctx.tile(*p);
            t.water == Water::None
                && !matches!(t.ground, Ground::Hills | Ground::Mountains | Ground::Peak)
        })
        .map(|p| {
            let mut lo = 255u8;
            let mut hi = 0u8;
            for dy in -1..=1 {
                for dx in -3..=3 {
                    let q = p.offset(dx, dy);
                    if ctx.in_bounds(q) && ctx.tile(q).water != Water::Sea {
                        let e = ctx.tile(q).elevation;
                        lo = lo.min(e);
                        hi = hi.max(e);
                    }
                }
            }
            let flat = (1.0 - 4.0 * (hi.saturating_sub(lo)) as f32 / 255.0).max(0.0);
            let mut score = 2.0 * flat;
            if mouth[p] <= 3.0 {
                score += 5.0;
            }
            if sea[p] <= 2.0 {
                score += 3.0;
            }
            if river[p] <= 1.5 {
                score += 2.0;
            }
            if lake[p] <= 2.0 {
                score += 1.5;
            }
            if high[p] <= 6.0 && high[p] > 2.0 {
                score += 1.0;
            }
            let (x, y) = s(p);
            let edge = x
                .min(w as f32 - 1.0 - x)
                .min(y)
                .min(2.0 * h as f32 - 2.0 - y);
            if edge < 6.0 {
                score -= 3.0;
            }
            (score + 0.5 * roll(ctx.seed(), 0x517E, p), p)
        })
        .collect();
    out.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then(a.1.y.cmp(&b.1.y))
            .then(a.1.x.cmp(&b.1.x))
    });
    out
}

#[cfg(test)]
mod tests {
    use crate::overworld::{Knobs, Overworld, Water, WorldParams, WorldSize, gen_with};

    fn home_is_wet(w: &Overworld) -> bool {
        let home = w.home();
        (-4..=4)
            .flat_map(|dx| (-2..=2).map(move |dy| (dx, dy)))
            .any(|(dx, dy)| {
                w.tile(home.centre.offset(dx, dy))
                    .is_some_and(|t| t.water != Water::None)
            })
    }

    #[test]
    fn the_home_city_is_the_biggest_and_on_the_water() {
        for seed in 0..20 {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            let home = w.home();
            assert!(w.cities.iter().all(|c| c.size <= home.size), "seed {seed}");
            assert!(home_is_wet(&w), "seed {seed}: home city dry");
        }
        let inland = gen_with(
            Knobs {
                sea_sides: Some(0),
                ..Default::default()
            },
            5,
        );
        assert!(home_is_wet(&inland), "inland: home city dry");
    }

    #[test]
    fn crowded_and_tiny_worlds_still_generate() {
        for seed in 0..10 {
            let w = Overworld::generate(WorldParams {
                seed,
                size: WorldSize::Custom { w: 60, h: 24 },
                cities: 8,
                ..Default::default()
            })
            .unwrap();
            assert!(w.cities.len() >= 3, "seed {seed}");
        }
    }
}
