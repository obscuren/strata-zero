//! Stage 10: names for the cities, the roads, the regions and the POIs, all from the
//! flavor pack.

use super::field::{distance_field, sdist};
use super::{Ctx, Ground, Poi, Region, RegionKind, Water};
use crate::geom::{Dir, Pos};
use crate::params::GenError;
use rand::Rng;
use rand::seq::SliceRandom;

const ROUTE_NUMBERS: [u32; 12] = [1, 3, 5, 7, 9, 11, 12, 14, 17, 22, 26, 31];

pub(crate) fn run(ctx: &mut Ctx) -> Result<(), GenError> {
    let mut rng = ctx.rng(8);
    cities(ctx, &mut rng);
    routes(ctx, &mut rng);
    regions(ctx, &mut rng);
    pois(ctx, &mut rng);
    Ok(())
}

fn cities(ctx: &mut Ctx, rng: &mut impl Rng) {
    let mut taken: Vec<String> = Vec::new();
    for i in 0..ctx.cities.len() {
        let mut name = ctx.flavor.city_name(rng);
        for _ in 0..50 {
            if !taken.contains(&name) {
                break;
            }
            name = ctx.flavor.city_name(rng);
        }
        if taken.contains(&name) {
            name = format!("{name} {}", i + 1);
        }
        taken.push(name.clone());
        ctx.cities[i].name = name;
    }
}

fn routes(ctx: &mut Ctx, rng: &mut impl Rng) {
    let sea = distance_field(ctx.size, |p| ctx.tile(p).water == Water::Sea, 4.0);
    let mut numbers = ROUTE_NUMBERS.to_vec();
    numbers.shuffle(rng);
    let mut coast_used = false;
    for (k, chain) in ctx.chains.clone().iter().enumerate() {
        let tiles: Vec<Pos> = chain
            .iter()
            .flat_map(|i| ctx.routes[*i].path.iter().copied())
            .collect();
        let by_sea = tiles.iter().filter(|p| sea[**p] <= 3.0).count();
        let name = if !coast_used && by_sea * 2 > tiles.len() {
            coast_used = true;
            "Coast Road".to_string()
        } else {
            match numbers.pop() {
                Some(n) => format!("Route {n}"),
                None => format!("Route {}", 40 + k),
            }
        };
        for i in chain {
            ctx.routes[*i].name = name.clone();
        }
    }
}

/// The nearest tile to `at` (within `reach` s-units) that `inside` accepts.
fn nearest_inside(ctx: &Ctx, at: Pos, reach: f32, inside: impl Fn(Pos) -> bool) -> Option<Pos> {
    if ctx.in_bounds(at) && inside(at) {
        return Some(at);
    }
    let r = reach.ceil() as i32;
    let mut best: Option<(f32, Pos)> = None;
    for dy in -(r / 2 + 1)..=(r / 2 + 1) {
        for dx in -r..=r {
            let p = at.offset(dx, dy);
            if !ctx.in_bounds(p) || !inside(p) {
                continue;
            }
            let d = sdist(p, at);
            if d <= reach && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, p));
            }
        }
    }
    best.map(|(_, p)| p)
}

fn regions(ctx: &mut Ctx, rng: &mut impl Rng) {
    let mut lists: Vec<(RegionKind, Vec<String>)> = ctx.flavor.world.regions.clone();
    for (_, names) in &mut lists {
        names.shuffle(rng);
    }
    let mut name = |kind: RegionKind| -> String {
        lists
            .iter_mut()
            .find(|(k, _)| *k == kind)
            .and_then(|(_, names)| names.pop())
            .unwrap_or_else(|| format!("{kind:?}"))
    };
    let mut out: Vec<Region> = Vec::new();
    let high = |c: &Ctx, p: Pos| {
        matches!(
            c.tile(p).ground,
            Ground::Hills | Ground::Mountains | Ground::Peak
        )
    };
    for ridge in ctx.ridges.clone() {
        let (x, y) = ridge[ridge.len() / 2];
        let at = Pos::new(x as i32, (y / 2.0) as i32);
        if let Some(at) = nearest_inside(ctx, at, 8.0, |p| high(ctx, p)) {
            out.push(Region {
                kind: RegionKind::Range,
                name: name(RegionKind::Range),
                at,
            });
        }
    }
    for bay in ctx.bays.clone() {
        // Half way out from the bay's tip toward the open sea.
        let out_dir = match bay.side {
            Dir::N => (0, -1),
            Dir::S => (0, 1),
            Dir::W => (-1, 0),
            _ => (1, 0),
        };
        let half = (bay.depth / 2.0) as i32;
        let at = bay
            .tip
            .offset(out_dir.0 * half, out_dir.1 * (half / 2).max(1));
        let sea = |p: Pos| ctx.tile(p).water == Water::Sea;
        if let Some(at) = nearest_inside(ctx, at, 6.0, sea).or(Some(bay.tip).filter(|p| sea(*p))) {
            out.push(Region {
                kind: RegionKind::Bay,
                name: name(RegionKind::Bay),
                at,
            });
        }
    }
    for (kind, ground, at) in [
        (RegionKind::Spill, Ground::Spill, ctx.spill),
        (RegionKind::Salt, Ground::Salt, ctx.salt),
    ] {
        if let Some(at) =
            at.and_then(|a| nearest_inside(ctx, a, 12.0, |p| ctx.tile(p).ground == ground))
        {
            out.push(Region {
                kind,
                name: name(kind),
                at,
            });
        }
    }
    // The flats: the open tile farthest from roads, cities and water.
    let road = distance_field(ctx.size, |p| ctx.tile(p).road != 0, 40.0);
    let town = distance_field(ctx.size, |p| ctx.tile(p).urban.is_some(), 40.0);
    let wet = distance_field(ctx.size, |p| ctx.tile(p).water != Water::None, 40.0);
    if let Some(at) = ctx
        .all()
        .filter(|p| ctx.tile(*p).ground == Ground::Flats && ctx.tile(*p).water == Water::None)
        .max_by(|a, b| {
            let f = |p: &Pos| road[*p].min(town[*p]).min(wet[*p]);
            f(a).total_cmp(&f(b))
                .then(b.y.cmp(&a.y))
                .then(b.x.cmp(&a.x))
        })
    {
        out.push(Region {
            kind: RegionKind::Flats,
            name: name(RegionKind::Flats),
            at,
        });
    }
    if let Some(river) = ctx.rivers.iter().max_by_key(|r| r.len()) {
        let at = river[river.len() / 2];
        if ctx.tile(at).water == Water::River {
            out.push(Region {
                kind: RegionKind::Valley,
                name: name(RegionKind::Valley),
                at,
            });
        }
    }
    ctx.regions = out;
}

fn pois(ctx: &mut Ctx, rng: &mut impl Rng) {
    let parts = ctx.flavor.world.pois.clone();
    let mut taken: Vec<String> = Vec::new();
    let pois: Vec<Poi> = ctx.pois.clone();
    for (i, poi) in pois.iter().enumerate() {
        let pick = |rng: &mut dyn rand::RngCore| -> String {
            match parts.iter().find(|(k, _)| *k == poi.kind) {
                Some((_, np)) if !np.prefixes.is_empty() && !np.suffixes.is_empty() => format!(
                    "{}{}",
                    np.prefixes[rng.random_range(0..np.prefixes.len())],
                    np.suffixes[rng.random_range(0..np.suffixes.len())]
                ),
                _ => poi.kind.label().to_string(),
            }
        };
        let mut name = pick(rng);
        for _ in 0..10 {
            if !taken.contains(&name) {
                break;
            }
            name = pick(rng);
        }
        taken.push(name.clone());
        ctx.pois[i].name = name;
    }
}

#[cfg(test)]
mod tests {
    use crate::overworld::{Ground, Overworld, RegionKind, Water, WorldParams};

    #[test]
    fn everything_has_a_name_and_labels_sit_in_their_regions() {
        for seed in 0..20 {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            let mut cities: Vec<_> = w.cities.iter().map(|c| c.name.clone()).collect();
            cities.sort();
            cities.dedup();
            assert_eq!(cities.len(), w.cities.len(), "seed {seed}");
            assert!(w.cities.iter().all(|c| !c.name.is_empty()));
            assert!(w.routes.iter().all(|r| !r.name.is_empty()));
            assert!(w.pois.iter().all(|p| !p.name.is_empty()));
            assert!(
                w.regions.iter().any(|r| r.kind == RegionKind::Range),
                "seed {seed}"
            );
            for r in &w.regions {
                let t = w.tile(r.at).unwrap();
                let inside = match r.kind {
                    RegionKind::Range => {
                        matches!(t.ground, Ground::Hills | Ground::Mountains | Ground::Peak)
                    }
                    RegionKind::Bay => t.water == Water::Sea,
                    RegionKind::Spill => t.ground == Ground::Spill,
                    RegionKind::Salt => t.ground == Ground::Salt,
                    RegionKind::Flats => t.ground == Ground::Flats,
                    RegionKind::Valley => {
                        t.water == Water::River
                            || matches!(t.ground, Ground::Fields | Ground::Scrub)
                    }
                };
                assert!(inside, "seed {seed}: {} at {:?}", r.name, r.at);
            }
        }
    }
}
