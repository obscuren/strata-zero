//! Stage 9: POIs out in the open, each kind where it belongs.

use super::field::{distance_field, sdist};
use super::{Ctx, Ground, Poi, PoiKind, Water};
use crate::geom::Pos;
use crate::params::GenError;
use rand::seq::SliceRandom;

/// How many of each kind a world takes, at most.
fn cap(kind: PoiKind) -> usize {
    match kind {
        PoiKind::TruckStop => 3,
        PoiKind::Checkpoint | PoiKind::Relay | PoiKind::Camp | PoiKind::Wreck => 2,
        _ => 1,
    }
}

pub(crate) fn run(ctx: &mut Ctx) -> Result<(), GenError> {
    let mut rng = ctx.rng(7);
    let size = ctx.size;
    let urban = distance_field(size, |p| ctx.tile(p).urban.is_some(), 8.0);
    let road = distance_field(size, |p| ctx.tile(p).road != 0, 14.0);
    let sea = distance_field(size, |p| ctx.tile(p).water == Water::Sea, 8.0);
    let land = distance_field(size, |p| ctx.tile(p).water != Water::Sea, 8.0);
    let shore = distance_field(
        size,
        |p| matches!(ctx.tile(p).water, Water::Lake | Water::River),
        4.0,
    );
    let dead = distance_field(
        size,
        |p| matches!(ctx.tile(p).ground, Ground::Spill | Ground::Salt),
        4.0,
    );
    let open = ctx
        .all()
        .filter(|p| ctx.tile(*p).water == Water::None && ctx.tile(*p).urban.is_none())
        .count();
    let target = (open / 600).clamp(6, 16);
    let centres: Vec<Pos> = ctx.cities.iter().map(|c| c.centre).collect();
    let dry = |p: Pos| ctx.tile(p).water == Water::None;
    let lowland = |p: Pos| dry(p) && matches!(ctx.tile(p).ground, Ground::Flats | Ground::Scrub);
    let mut candidates: Vec<(PoiKind, Vec<Pos>)> = Vec::new();
    for kind in PoiKind::ALL {
        let mut spots: Vec<Pos> = match kind {
            PoiKind::TruckStop => beside_middles(ctx),
            PoiKind::Checkpoint => outside_cities(ctx),
            PoiKind::Relay => ctx
                .all()
                .filter(|p| dry(*p) && ctx.tile(*p).ground == Ground::Hills)
                .collect(),
            PoiKind::Camp => ctx
                .all()
                .filter(|p| lowland(*p) && road[*p] >= 10.0)
                .collect(),
            PoiKind::Wreck => ctx
                .all()
                .filter(|p| {
                    dry(*p) && !matches!(ctx.tile(*p).ground, Ground::Peak | Ground::Mountains)
                })
                .collect(),
            PoiKind::Arcology => ctx
                .all()
                .filter(|p| {
                    dry(*p)
                        && ctx.tile(*p).ground == Ground::Flats
                        && sea[*p] <= 6.0
                        && centres.iter().all(|c| sdist(*p, *c) >= 15.0)
                })
                .collect(),
            PoiKind::Solar => ctx
                .all()
                .filter(|p| dry(*p) && ctx.tile(*p).ground == Ground::Flats)
                .collect(),
            PoiKind::WaterPlant => ctx.all().filter(|p| dry(*p) && shore[*p] <= 1.5).collect(),
            PoiKind::TestRange => ctx.all().filter(|p| dry(*p) && dead[*p] <= 1.5).collect(),
            PoiKind::Rig => ctx
                .all()
                .filter(|p| ctx.tile(*p).water == Water::Sea && land[*p] <= 6.0 && land[*p] >= 2.0)
                .collect(),
        };
        spots.shuffle(&mut rng);
        candidates.push((kind, spots));
    }
    let mut placed: Vec<Poi> = Vec::new();
    let fits = |p: Pos, placed: &[Poi]| {
        let t = ctx.tile(p);
        t.urban.is_none()
            && urban[p] > 3.0
            && t.ground != Ground::Peak
            && placed.iter().all(|q| sdist(q.at, p) >= 6.0)
    };
    // One of each kind first, so a world shows its variety, then up to each kind's cap.
    for round in 0..3 {
        for (kind, spots) in &candidates {
            if placed.len() >= target {
                break;
            }
            let have = placed.iter().filter(|q| q.kind == *kind).count();
            if have > round || have >= cap(*kind) {
                continue;
            }
            if let Some(&at) = spots.iter().find(|p| fits(**p, &placed)) {
                placed.push(Poi {
                    kind: *kind,
                    name: String::new(),
                    at,
                });
            }
        }
    }
    ctx.pois = placed;
    Ok(())
}

/// Tiles beside (not on) the middle third of each long route, nearest the middle first.
fn beside_middles(ctx: &Ctx) -> Vec<Pos> {
    let mut out = Vec::new();
    for r in ctx.routes.iter().filter(|r| r.path.len() > 30) {
        let mid = r.path[r.path.len() / 2];
        let mut side: Vec<Pos> = r.path[r.path.len() / 3..2 * r.path.len() / 3]
            .iter()
            .flat_map(|p| p.neighbors4())
            .filter(|q| {
                ctx.in_bounds(*q) && ctx.tile(*q).road == 0 && ctx.tile(*q).water == Water::None
            })
            .collect();
        side.sort_by(|a, b| sdist(*a, mid).total_cmp(&sdist(*b, mid)));
        out.extend(side.into_iter().take(3));
    }
    out
}

/// On each route out of a big city, three tiles past its edge.
fn outside_cities(ctx: &Ctx) -> Vec<Pos> {
    let mut out = Vec::new();
    for c in ctx.cities.iter().filter(|c| c.size >= 8.0) {
        for r in &ctx.routes {
            let along: Vec<Pos> = if r.path[0] == c.centre {
                r.path.clone()
            } else if *r.path.last().unwrap() == c.centre {
                r.path.iter().rev().copied().collect()
            } else {
                continue;
            };
            if let Some(k) = along.iter().position(|p| ctx.tile(*p).urban.is_none())
                && let Some(p) = along.get(k + 3)
            {
                out.push(*p);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::overworld::field::sdist;
    use crate::overworld::{Ground, Overworld, PoiKind, Water, WorldParams};

    #[test]
    fn pois_sit_where_their_kind_goes() {
        for seed in 0..20 {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            assert!(w.pois.len() >= 6, "seed {seed}: {}", w.pois.len());
            for p in &w.pois {
                let t = w.tile(p.at).unwrap();
                assert!(
                    t.urban.is_none() && t.ground != Ground::Peak,
                    "seed {seed} {p:?}"
                );
                match p.kind {
                    PoiKind::Rig => assert_eq!(t.water, Water::Sea, "seed {seed}"),
                    PoiKind::Checkpoint => assert!(t.road != 0, "seed {seed}"),
                    PoiKind::Relay => assert_eq!(t.ground, Ground::Hills, "seed {seed}"),
                    _ => assert_ne!(t.water, Water::Sea, "seed {seed} {p:?}"),
                }
            }
            for (i, a) in w.pois.iter().enumerate() {
                for b in &w.pois[i + 1..] {
                    assert!(sdist(a.at, b.at) >= 6.0, "seed {seed}: crowded");
                }
            }
        }
    }
}
