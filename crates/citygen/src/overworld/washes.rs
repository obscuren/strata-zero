//! Stage 4b: dry washes. Seasonal streambeds run down from the foot of the hills across
//! the lowland, into a river, a lake, the sea or a salt pan, or peter out in the flats.
//! One level down a wash is a dry sandy bed; where a highway crosses one it rides over
//! on a culvert.

use super::field::{s, sdist};
use super::{Ctx, Ground, Water, link_toward};
use crate::geom::Pos;
use crate::params::GenError;
use rand::seq::SliceRandom;

pub(crate) fn run(ctx: &mut Ctx) -> Result<(), GenError> {
    let mut rng = ctx.rng(10);
    let dry = |c: &Ctx, p: Pos| {
        let t = c.tile(p);
        t.water == Water::None && !matches!(t.ground, Ground::Mountains | Ground::Peak)
    };
    let land = ctx.all().filter(|p| dry(ctx, *p)).count();
    let want = (land / 700).clamp(3, 14);
    let high = |c: &Ctx, p: Pos| matches!(c.tile(p).ground, Ground::Hills | Ground::Mountains);
    // Starts: lowland or hills at the foot of the high ground, clear of city cores.
    let mut starts: Vec<Pos> = ctx
        .all()
        .filter(|p| {
            dry(ctx, *p)
                && (-3..=3).any(|dx| {
                    (-2..=2).any(|dy| {
                        let q = p.offset(dx, dy);
                        ctx.in_bounds(q) && high(ctx, q)
                    })
                })
                && ctx
                    .cities
                    .iter()
                    .all(|c| sdist(*p, c.centre) > 0.8 * c.size + 4.0)
        })
        .collect();
    starts.shuffle(&mut rng);
    let meander = ctx.fbm(1501);
    let mut taken: Vec<Pos> = Vec::new();
    for start in starts {
        if ctx.washes.len() >= want {
            break;
        }
        if taken.iter().any(|q| sdist(*q, start) < 10.0) || ctx.tile(start).wash != 0 {
            continue;
        }
        let course = walk(ctx, start, &meander);
        if course.len() < 6
            || ctx.tile(course[0]).elevation <= ctx.tile(*course.last().unwrap()).elevation
        {
            continue;
        }
        for w in course.windows(2) {
            ctx.tile_mut(w[0]).wash |= link_toward(w[0], w[1]);
            ctx.tile_mut(w[1]).wash |= link_toward(w[1], w[0]);
        }
        taken.push(start);
        ctx.washes.push(course);
    }
    Ok(())
}

/// Downhill from `start`, meandering, until it meets water, a salt pan, another wash or
/// ground that no longer falls; never into the mountains.
fn walk(ctx: &Ctx, start: Pos, meander: &super::field::Fbm) -> Vec<Pos> {
    let mut course = vec![start];
    for _ in 0..40 {
        let at = *course.last().unwrap();
        let here = ctx.tile(at).elevation as f32;
        let score = |q: Pos| {
            let (x, y) = s(q);
            ctx.tile(q).elevation as f32 + 40.0 * (meander.at(x / 6.0, y / 6.0, 3) - 0.5)
        };
        let next = at
            .neighbors4()
            .filter(|q| ctx.in_bounds(*q) && !course.contains(q))
            .filter(|q| {
                let t = ctx.tile(*q);
                !matches!(t.ground, Ground::Mountains | Ground::Peak)
            })
            .min_by(|a, b| {
                score(*a)
                    .total_cmp(&score(*b))
                    .then(a.y.cmp(&b.y))
                    .then(a.x.cmp(&b.x))
            });
        let Some(q) = next else { break };
        let t = ctx.tile(q);
        if t.water != Water::None || t.wash != 0 {
            break; // it runs into the water, or into another wash
        }
        if (t.elevation as f32) > here + 6.0 {
            break; // the ground stops falling: it peters out
        }
        course.push(q);
        if t.ground == Ground::Salt {
            break; // into the salt pan
        }
    }
    course
}

#[cfg(test)]
mod tests {
    use crate::overworld::{Ground, Overworld, Water, WorldParams, WorldSize};

    #[test]
    fn washes_run_down_from_the_hills_across_dry_land() {
        let mut total = 0;
        for size in [WorldSize::Small, WorldSize::Medium, WorldSize::Large] {
            for seed in 0..8 {
                let w = Overworld::generate(WorldParams {
                    seed,
                    size,
                    ..Default::default()
                })
                .unwrap();
                total += w.washes.len();
                for wash in &w.washes {
                    assert!(
                        wash.len() >= 6,
                        "{size:?} {seed}: a wash of {} tiles",
                        wash.len()
                    );
                    assert!(
                        wash.windows(2).all(|p| p[0].manhattan(p[1]) == 1),
                        "{size:?} {seed}: broken"
                    );
                    let first = w.tile(wash[0]).unwrap();
                    let last = w.tile(*wash.last().unwrap()).unwrap();
                    assert!(
                        first.elevation > last.elevation,
                        "{size:?} {seed}: a wash runs uphill"
                    );
                    for p in wash {
                        let t = w.tile(*p).unwrap();
                        assert_eq!(
                            t.water,
                            Water::None,
                            "{size:?} {seed}: wash in water at {p:?}"
                        );
                        assert!(!matches!(t.ground, Ground::Mountains | Ground::Peak));
                        assert!(
                            t.wash != 0,
                            "{size:?} {seed}: wash tile without links at {p:?}"
                        );
                    }
                }
            }
        }
        assert!(total >= 24 * 3, "{total} washes over 24 worlds");
    }

    #[test]
    fn highways_cross_washes_rather_than_follow_them() {
        let (mut shared, mut crossing) = (0, 0);
        for seed in 0..12 {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            for (_, t) in w.tiles.iter().filter(|(_, t)| t.road != 0 && t.wash != 0) {
                if t.road & t.wash != 0 {
                    shared += 1; // the road runs the same way as the wash here
                } else {
                    crossing += 1;
                }
            }
        }
        assert!(
            shared * 4 <= crossing,
            "{shared} tiles along a wash, {crossing} across"
        );
    }

    #[test]
    fn some_highways_cross_a_wash() {
        let crossings: usize = (0..8)
            .map(|seed| {
                let w = Overworld::generate(WorldParams {
                    seed,
                    ..Default::default()
                })
                .unwrap();
                w.tiles.count(|t| t.wash != 0 && t.road != 0)
            })
            .sum();
        assert!(crossings >= 4, "{crossings} culverts over 8 worlds");
    }
}
