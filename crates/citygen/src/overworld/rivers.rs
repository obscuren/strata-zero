//! Stage 3: rivers from the ridges down to the sea (or a lake inland), and lakes.

use super::field::{poly_dist, s, sdist};
use super::{Ctx, Ground, Water, link_toward};
use crate::geom::Pos;
use crate::params::GenError;
use crate::pathing::astar;
use rand::Rng;
use rand_chacha::ChaCha8Rng;

pub(crate) fn run(ctx: &mut Ctx) -> Result<(), GenError> {
    let mut rng = ctx.rng(2);
    let count = ctx
        .params
        .knobs
        .rivers
        .unwrap_or_else(|| rng.random_range(1..=3))
        .max(1);
    // An inland world drains into a lake made for it.
    if ctx.sea_sides.is_empty() {
        let at = low_spot(ctx, &mut rng);
        lake(ctx, &mut rng, at, (4.0, 6.0), (3.0, 4.0));
    }
    let mut springs: Vec<Pos> = Vec::new();
    for i in 0..count {
        // A spring whose water finds no way down gives way to another.
        for _attempt in 0..6 {
            // The first river starts far up the range from the bay it runs to.
            let bay = ctx
                .bays
                .iter()
                .max_by(|a, b| a.depth.total_cmp(&b.depth))
                .map(|b| b.tip)
                .filter(|_| i == 0);
            let Some(spring) = spring(ctx, &mut rng, &springs, bay) else {
                break;
            };
            springs.push(spring);
            let Some(target) = target(ctx, &mut rng, i, spring) else {
                continue;
            };
            if let Some(path) = flow(ctx, spring, target) {
                mark(ctx, path);
                break;
            }
        }
    }
    for _ in 0..rng.random_range(0..=2) {
        let on_river = rng.random_bool(0.5) && !ctx.rivers.is_empty();
        let at = if on_river {
            let r = &ctx.rivers[rng.random_range(0..ctx.rivers.len())];
            let low: Vec<Pos> = r[r.len() / 3..]
                .iter()
                .copied()
                .filter(|p| ctx.tile(*p).elevation < 110)
                .collect();
            if low.is_empty() {
                continue;
            }
            low[rng.random_range(0..low.len())]
        } else {
            low_spot(ctx, &mut rng)
        };
        lake(ctx, &mut rng, at, (3.0, 5.0), (2.0, 4.0));
    }
    Ok(())
}

/// A low tile well inside the map: the lowest of a few dozen tries.
fn low_spot(ctx: &Ctx, rng: &mut ChaCha8Rng) -> Pos {
    let (w, h) = (ctx.size.w as i32, ctx.size.h as i32);
    let mut best: Option<Pos> = None;
    for _ in 0..60 {
        let p = Pos::new(
            rng.random_range(w / 5..w - w / 5),
            rng.random_range(h / 5..h - h / 5),
        );
        let t = ctx.tile(p);
        if t.water != Water::None || t.ground != Ground::Flats {
            continue;
        }
        if best.is_none_or(|b| t.elevation < ctx.tile(b).elevation) {
            best = Some(p);
        }
    }
    best.unwrap_or(Pos::new(w / 2, h / 2))
}

/// An irregular lake round `at`, radii in s-units; never over the sea.
fn lake(ctx: &mut Ctx, rng: &mut ChaCha8Rng, at: Pos, rx: (f32, f32), ry: (f32, f32)) {
    let (rx, ry) = (rng.random_range(rx.0..rx.1), rng.random_range(ry.0..ry.1));
    let shape = ctx.fbm(1102 + rng.random_range(0..1000));
    let (cx, cy) = s(at);
    for p in ctx.all() {
        let (x, y) = s(p);
        let d = ((x - cx) / rx).hypot((y - cy) / ry) + 0.9 * (shape.at(x / 2.0, y / 2.0, 2) - 0.5);
        if d < 1.0 && ctx.tile(p).water != Water::Sea && ctx.tile(p).ground != Ground::Peak {
            let t = ctx.tile_mut(p);
            t.water = Water::Lake;
            t.river = 0;
        }
    }
}

/// A spring high on a ridge, away from the springs before it; with `far_from`, one of
/// the farthest from there.
fn spring(ctx: &Ctx, rng: &mut ChaCha8Rng, taken: &[Pos], far_from: Option<Pos>) -> Option<Pos> {
    let mut high: Vec<Pos> = ctx
        .all()
        .filter(|p| {
            let t = ctx.tile(*p);
            t.water == Water::None
                && matches!(t.ground, Ground::Hills | Ground::Mountains)
                && ctx.ridges.iter().any(|r| poly_dist(s(*p), r) < 8.0)
                && taken.iter().all(|q| sdist(*p, *q) > 20.0)
        })
        .collect();
    if high.is_empty() {
        // No ridge to start from: the highest hill anywhere, if there is one.
        return ctx
            .all()
            .filter(|p| {
                let t = ctx.tile(*p);
                t.water == Water::None && matches!(t.ground, Ground::Hills | Ground::Mountains)
            })
            .filter(|p| taken.iter().all(|q| sdist(*p, *q) > 20.0))
            .max_by_key(|p| ctx.tile(*p).elevation);
    }
    if let Some(from) = far_from {
        high.sort_by(|a, b| sdist(*b, from).total_cmp(&sdist(*a, from)));
        high.truncate(high.len().div_ceil(4));
    }
    Some(high[rng.random_range(0..high.len())])
}

/// Where river `i` from `spring` runs to: the first to the deepest bay, the rest to the
/// nearest sea or lake, or half the time into a river already there.
fn target(ctx: &Ctx, rng: &mut ChaCha8Rng, i: u8, spring: Pos) -> Option<Pos> {
    let nearest = |f: &dyn Fn(Pos) -> bool| {
        ctx.all()
            .filter(|p| f(*p))
            .min_by(|a, b| sdist(*a, spring).total_cmp(&sdist(*b, spring)))
    };
    if i == 0
        && let Some(bay) = ctx.bays.iter().max_by(|a, b| a.depth.total_cmp(&b.depth))
    {
        return Some(bay.tip);
    }
    if i > 0 && rng.random_bool(0.5) {
        let on: Vec<Pos> = ctx.rivers.iter().flatten().copied().collect();
        if let Some(p) = on
            .iter()
            .copied()
            .min_by(|a, b| sdist(*a, spring).total_cmp(&sdist(*b, spring)))
        {
            return Some(p);
        }
    }
    nearest(&|p| matches!(ctx.tile(p).water, Water::Sea | Water::Lake))
}

/// The river's course: a route down the valleys from `spring` to `target` (pathfound,
/// kept off the map's edge), drawn as a smooth meandering line through it, cut where it
/// first meets water.
fn flow(ctx: &Ctx, spring: Pos, target: Pos) -> Option<Vec<Pos>> {
    let valley = ctx.fbm(1101);
    let (w, h) = (ctx.size.w as i32, ctx.size.h as i32);
    let route = astar(ctx.size, spring, target, |_, to| {
        if to == target {
            return Some(10);
        }
        let t = ctx.tile(to);
        if t.water != Water::None || t.ground == Ground::Peak {
            return None;
        }
        let (x, y) = s(to);
        let e = t.elevation as f32 / 255.0;
        let edge = to.x.min(w - 1 - to.x).min(2 * to.y.min(h - 1 - to.y));
        let near_edge = if edge < 6 { 30.0 * (6 - edge) as f32 } else { 0.0 };
        Some((10.0 + 300.0 * e + 120.0 * valley.at(x / 12.0, y / 12.0, 3) + near_edge) as u32)
    })?;
    let line = smooth(ctx, &route);
    // The swing must not move the spring.
    let mut course: Vec<Pos> = vec![spring];
    // Adds a tile; coming back over the line cuts the loop out.
    let add = |course: &mut Vec<Pos>, q: Pos| {
        if let Some(i) = course.iter().position(|c| *c == q) {
            course.truncate(i + 1);
        } else {
            course.push(q);
        }
    };
    for p in line {
        if course.last() == Some(&p) {
            continue;
        }
        // Keep it four-connected: step across first, then down.
        while let Some(&last) = course.last() {
            if last.manhattan(p) <= 1 {
                break;
            }
            let step = if last.x != p.x {
                Pos::new(last.x + (p.x - last.x).signum(), last.y)
            } else {
                Pos::new(last.x, last.y + (p.y - last.y).signum())
            };
            add(&mut course, step);
            if ctx.tile(step).water != Water::None {
                break;
            }
        }
        if course.last().is_some_and(|l| ctx.tile(*l).water != Water::None) && course.len() > 1 {
            break;
        }
        add(&mut course, p);
        let t = ctx.tile(p);
        if course.len() > 1 && (t.water != Water::None || p == target) {
            break;
        }
    }
    let end = course
        .iter()
        .position(|p| ctx.tile(*p).water != Water::None)
        .map(|i| i + 1)
        .unwrap_or(course.len());
    course.truncate(end);
    (course.len() >= 2).then_some(course)
}

/// The route as a smooth line of tiles: every sixth step as a control point, rounded
/// twice (Chaikin), swung to either side by noise, sampled densely.
fn smooth(ctx: &Ctx, route: &[Pos]) -> Vec<Pos> {
    let mut pts: Vec<(f32, f32)> = route.iter().step_by(6).map(|p| s(*p)).collect();
    let end = s(*route.last().unwrap());
    if pts.last() != Some(&end) {
        pts.push(end);
    }
    for _ in 0..2 {
        let mut next = vec![pts[0]];
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            next.push((0.75 * a.0 + 0.25 * b.0, 0.75 * a.1 + 0.25 * b.1));
            next.push((0.25 * a.0 + 0.75 * b.0, 0.25 * a.1 + 0.75 * b.1));
        }
        next.push(*pts.last().unwrap());
        pts = next;
    }
    let swing = ctx.fbm(1103);
    let mut out = Vec::new();
    let mut along = 0.0f32;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = (b.0 - a.0).hypot(b.1 - a.1).max(0.001);
        let (nx, ny) = (-(b.1 - a.1) / len, (b.0 - a.0) / len);
        let n = (len * 3.0).ceil() as usize;
        for k in 0..n {
            let t = k as f32 / n as f32;
            let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            let off = 4.5 * (2.0 * swing.at(along / 14.0, 0.5, 2) - 1.0);
            let (x, y) = (x + nx * off, y + ny * off);
            let p = Pos::new(x.round() as i32, (y / 2.0).round() as i32);
            if ctx.in_bounds(p) {
                out.push(p);
            }
            along += len / n as f32;
        }
    }
    out.push(*route.last().unwrap());
    out
}

/// Lays the course on the tiles: river where there's land, linked tile to tile, the
/// last land tile linked on into the water it meets.
fn mark(ctx: &mut Ctx, mut course: Vec<Pos>) {
    let into = course
        .last()
        .copied()
        .filter(|p| ctx.tile(*p).water != Water::None || ctx.rivers.iter().flatten().any(|q| q == p));
    if into.is_some() {
        course.pop();
    }
    if course.len() < 2 {
        return;
    }
    for w in course.windows(2) {
        ctx.tile_mut(w[0]).river |= link_toward(w[0], w[1]);
        ctx.tile_mut(w[1]).river |= link_toward(w[1], w[0]);
    }
    for p in &course {
        let t = ctx.tile_mut(*p);
        t.water = Water::River;
        // A river through a peak has cut a gorge.
        if t.ground == Ground::Peak {
            t.ground = Ground::Mountains;
        }
    }
    let last = *course.last().unwrap();
    if let Some(q) = into {
        ctx.tile_mut(last).river |= link_toward(last, q);
        if ctx.tile(q).water == Water::River {
            ctx.tile_mut(q).river |= link_toward(q, last);
        }
        if ctx.tile(q).water == Water::Sea {
            ctx.mouths.push(last);
        }
    }
    ctx.rivers.push(course);
}

#[cfg(test)]
mod tests {
    use crate::overworld::{Knobs, Overworld, Water, WorldParams, gen_with};

    #[test]
    fn rivers_run_unbroken_from_the_hills_to_the_water() {
        for seed in 0..20 {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            assert!(!w.rivers.is_empty(), "seed {seed}");
            for r in &w.rivers {
                assert!(
                    r.windows(2).all(|p| p[0].manhattan(p[1]) == 1),
                    "seed {seed}: broken"
                );
                let spring = w.tile(r[0]).unwrap();
                assert!(
                    spring.elevation as f32 / 255.0 > 0.4,
                    "seed {seed}: low spring"
                );
                let end = *r.last().unwrap();
                let ends_wet = end.neighbors4().chain([end]).any(|q| {
                    w.tile(q)
                        .is_some_and(|t| matches!(t.water, Water::Sea | Water::Lake))
                        || w.rivers.iter().any(|o| !std::ptr::eq(o, r) && o.contains(&q))
                });
                assert!(ends_wet, "seed {seed}: river ends dry at {end:?}");
            }
        }
    }

    #[test]
    fn an_inland_world_drains_into_a_lake() {
        let w = gen_with(
            Knobs {
                sea_sides: Some(0),
                ..Default::default()
            },
            5,
        );
        assert!(w.tiles.count(|t| t.water == Water::Lake) > 0);
    }
}
