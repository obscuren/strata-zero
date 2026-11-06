//! Stages 7 and 8: cities grow out from their centres, along their roads and shores,
//! not up the hills or into dead ground; outlying towns where a highway leaves; and the
//! districts' colours for the overworld.

use super::field::{distance_field, roll, s, sdist};
use super::{Ctx, Ground, Urban, Water};
use crate::geom::{Grid, Pos};
use crate::params::GenError;
use crate::world::Theme;

const TOWN_SIZE: f32 = 2.6;
const TOWN_DENSITY: f32 = 0.55;

struct Fields {
    road: Grid<f32>,
    water: Grid<f32>,
}

pub(crate) fn run(ctx: &mut Ctx) -> Result<(), GenError> {
    let fields = Fields {
        road: distance_field(ctx.size, |p| ctx.tile(p).road != 0, 8.0),
        water: distance_field(
            ctx.size,
            |p| matches!(ctx.tile(p).water, Water::Sea | Water::Lake),
            8.0,
        ),
    };
    let mut held: Grid<Option<(u8, f32)>> = Grid::filled(ctx.size, None);
    for i in 0..ctx.cities.len() {
        let c = ctx.cities[i].clone();
        grow(ctx, &fields, &mut held, i as u8, c.centre, c.size, 1.0);
    }
    towns(ctx, &fields, &mut held);
    for p in ctx.all() {
        if let Some((city, density)) = held[p] {
            ctx.tile_mut(p).urban = Some(Urban {
                city,
                density: (density.clamp(0.0, 1.0) * 255.0) as u8,
                theme: Theme::OldTown,
            });
        }
    }
    districts(ctx, &fields);
    Ok(())
}

/// One city's (or town's) growth from `centre`, merged into `held` where it is denser.
fn grow(
    ctx: &Ctx,
    f: &Fields,
    held: &mut Grid<Option<(u8, f32)>>,
    city: u8,
    centre: Pos,
    size: f32,
    dense: f32,
) {
    let wobble = ctx.fbm(1400 + city as u32 * 7 + (centre.x as u32 & 0xff));
    let reach = 3.0 * size;
    let (rx, ry) = (reach.ceil() as i32, (reach / 2.0).ceil() as i32);
    let mut want: Grid<Option<f32>> = Grid::filled(ctx.size, None);
    for dy in -ry..=ry {
        for dx in -rx..=rx {
            let p = centre.offset(dx, dy);
            if !ctx.in_bounds(p) {
                continue;
            }
            let t = ctx.tile(p);
            if matches!(t.water, Water::Sea | Water::Lake)
                || matches!(t.ground, Ground::Peak | Ground::Spill | Ground::Salt)
            {
                continue;
            }
            let d = sdist(p, centre);
            let (x, y) = s(p);
            let mut score = size * (0.7 + 0.6 * wobble.at(x / 5.0, y / 5.0, 3)) - d;
            let dr = f.road[p];
            if dr <= 3.0 && d < 2.7 * size {
                score += (3.2 - dr) * 1.5 * (1.15 - d / (2.7 * size));
            }
            let dw = f.water[p];
            if dw <= 2.0 {
                score += 2.5 - dw;
            }
            score -= match t.ground {
                Ground::Hills => 4.0,
                Ground::Mountains => 12.0,
                _ => 0.0,
            };
            if score > 0.0 {
                want[p] = Some((score / size).min(1.0) * dense);
            }
        }
    }
    if want[centre].is_none() {
        want[centre] = Some(dense);
    }
    // Only what holds together with the centre (eight ways; a river is no break).
    let mut kept = vec![centre];
    let mut seen: Grid<bool> = Grid::filled(ctx.size, false);
    seen[centre] = true;
    let mut i = 0;
    while i < kept.len() {
        let p = kept[i];
        i += 1;
        for q in p.neighbors8() {
            if ctx.in_bounds(q) && !seen[q] && want[q].is_some() {
                seen[q] = true;
                kept.push(q);
            }
        }
    }
    for p in kept {
        let v = want[p].unwrap_or(dense);
        if held[p].is_none_or(|(_, old)| old < v) {
            held[p] = Some((city, v));
        }
    }
}

/// A knot of sprawl where a highway leaves a big city, about 2.1 times its size out.
fn towns(ctx: &mut Ctx, f: &Fields, held: &mut Grid<Option<(u8, f32)>>) {
    let mut placed: Vec<Pos> = Vec::new();
    let routes = ctx.routes.clone();
    for (ci, c) in ctx.cities.clone().iter().enumerate() {
        if c.size < 8.0 {
            continue;
        }
        for r in &routes {
            let along: Vec<Pos> = if r.path[0] == c.centre {
                r.path.clone()
            } else if *r.path.last().unwrap() == c.centre {
                r.path.iter().rev().copied().collect()
            } else {
                continue;
            };
            let Some(&at) = along.iter().find(|p| sdist(**p, c.centre) >= 2.1 * c.size) else {
                continue;
            };
            let t = ctx.tile(at);
            if roll(ctx.seed(), 0x70E, at) >= 0.55
                || matches!(t.water, Water::Sea | Water::Lake)
                || matches!(
                    t.ground,
                    Ground::Peak | Ground::Spill | Ground::Salt | Ground::Mountains
                )
                || placed.iter().any(|q| sdist(*q, at) < 8.0)
                || ctx
                    .cities
                    .iter()
                    .any(|o| sdist(o.centre, at) < o.size + TOWN_SIZE)
            {
                continue;
            }
            grow(ctx, f, held, ci as u8, at, TOWN_SIZE, TOWN_DENSITY);
            placed.push(at);
            ctx.cities[ci].towns.push(at);
        }
    }
}

const ROTATION: [Theme; 7] = [
    Theme::NightMarket,
    Theme::Megablocks,
    Theme::NeonStrip,
    Theme::Docks,
    Theme::Industrial,
    Theme::OldTown,
    Theme::CombatZone,
];

/// What the home city's story needs on its streets, first need first: a night market
/// (the cook the first job goes to, and a fixer), the strip (the fixers who give the
/// jobs) and the blocks (the runner's flat).
const STORY: [Theme; 3] = [Theme::NightMarket, Theme::NeonStrip, Theme::Megablocks];

/// The fewest tiles of the home city a district the story needs comes to: past a stray
/// tile or two, a quarter with streets and buildings of its own.
const STORY_TILES: usize = 24;

/// Where a tile's colour comes from: one of its city's district seeds, or none (the
/// outer ring's housing and the shore's docks, whatever the seed).
#[derive(Clone, Copy, PartialEq)]
enum Source {
    Seed(usize),
    Ring(Theme),
    Shore,
}

/// The districts' colours: a corp core, themes round it, the outer ring housing, the
/// shore docks. The home city's seeds give the story what it needs.
fn districts(ctx: &mut Ctx, f: &Fields) {
    let seed = ctx.seed();
    for (ci, c) in ctx.cities.clone().iter().enumerate() {
        let mine: Vec<Pos> = ctx
            .all()
            .filter(|p| ctx.tile(*p).urban.is_some_and(|u| u.city as usize == ci))
            .collect();
        let want = ((c.size / 2.4).round() as usize).clamp(1, 6);
        let mut order = mine.clone();
        order.sort_by(|a, b| {
            roll(seed, 0xD15 + ci as u64, *a).total_cmp(&roll(seed, 0xD15 + ci as u64, *b))
        });
        let mut seeds = vec![c.centre];
        for p in order {
            if seeds.len() >= want {
                break;
            }
            if seeds.iter().all(|q| sdist(p, *q) > 0.55 * c.size) {
                seeds.push(p);
            }
        }
        // the core is seed 0's (and only seed 0's); the rest take the rotation in turn
        let mut themes: Vec<Theme> = (0..seeds.len())
            .map(|j| match j {
                0 => Theme::CorpCore,
                j => ROTATION[(j - 1 + ci) % ROTATION.len()],
            })
            .collect();
        let from = |p: Pos| {
            let d = sdist(p, c.centre);
            let mut by_distance: Vec<usize> = (0..seeds.len()).collect();
            by_distance.sort_by(|a, b| sdist(p, seeds[*a]).total_cmp(&sdist(p, seeds[*b])));
            let mut from = Source::Seed(by_distance[0]);
            // The core is only the core: past it, the next district out.
            if from == Source::Seed(0) && d > 0.4 * c.size {
                from = by_distance
                    .get(1)
                    .map_or(Source::Ring(Theme::OldTown), |j| Source::Seed(*j));
            }
            if d > 1.3 * c.size {
                let coarse = Pos::new(p.x / 4, p.y / 2);
                from = Source::Ring(if roll(seed, 0x0A7, coarse) < 0.5 {
                    Theme::Megablocks
                } else {
                    Theme::OldTown
                });
            }
            if f.water[p] <= 1.5 && from != Source::Seed(0) && roll(seed, 0xD0C, p) < 0.7 {
                from = Source::Shore;
            }
            from
        };
        let froms: Vec<(Pos, Source)> = mine.iter().map(|p| (*p, from(*p))).collect();
        if c.home {
            story(&mut themes, &froms);
        }
        for (p, from) in froms {
            let theme = match from {
                Source::Seed(j) => themes[j],
                Source::Ring(theme) => theme,
                Source::Shore => Theme::Docks,
            };
            if let Some(u) = ctx.tile_mut(p).urban.as_mut() {
                u.theme = theme;
            }
        }
    }
}

/// The home city's seeds' themes, so each district the story needs (`STORY`) comes to
/// `STORY_TILES` tiles or more where the seeds can give it: a need the rotation left
/// out, or left a sliver, goes to the seed with the most tiles that no need holds yet,
/// and the seed that had it (if one did) takes that seed's theme in its place.
fn story(themes: &mut [Theme], froms: &[(Pos, Source)]) {
    let mut tiles = vec![0usize; themes.len()];
    for (_, from) in froms {
        if let Source::Seed(j) = from {
            tiles[*j] += 1;
        }
    }
    let mut held: Vec<usize> = vec![0];
    for need in STORY {
        let has = (1..themes.len()).find(|j| themes[*j] == need && tiles[*j] >= STORY_TILES);
        if let Some(j) = has {
            held.push(j);
            continue;
        }
        let Some(best) = (1..themes.len())
            .filter(|j| !held.contains(j))
            .max_by_key(|j| (tiles[*j], std::cmp::Reverse(*j)))
        else {
            continue;
        };
        if let Some(old) = (1..themes.len()).find(|j| themes[*j] == need) {
            themes[old] = themes[best];
        }
        themes[best] = need;
        held.push(best);
    }
}

#[cfg(test)]
mod tests {
    use crate::geom::Pos;
    use crate::overworld::{Ground, Overworld, Water, WorldParams};
    use crate::pathing::flood_fill;

    #[test]
    fn cities_grow_on_land_and_hold_together() {
        for seed in 0..20 {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            for (p, t) in w.tiles.iter() {
                if t.urban.is_some() {
                    assert!(
                        !matches!(t.water, Water::Sea | Water::Lake),
                        "seed {seed} {p:?}"
                    );
                    assert!(!matches!(
                        t.ground,
                        Ground::Peak | Ground::Spill | Ground::Salt
                    ));
                }
            }
            for (i, c) in w.cities.iter().enumerate() {
                let mine = |p: Pos| {
                    w.tile(p)
                        .and_then(|t| t.urban)
                        .is_some_and(|u| u.city as usize == i)
                };
                let core = flood_fill(w.size, c.centre, mine);
                let all = w.tiles.iter().filter(|(p, _)| mine(*p)).count();
                let towns: usize = c
                    .towns
                    .iter()
                    .map(|t| flood_fill(w.size, *t, mine).len())
                    .sum();
                assert!(
                    core.len() + towns >= all * 9 / 10,
                    "seed {seed}: city {i} scattered ({} + {towns} of {all})",
                    core.len()
                );
            }
        }
    }

    #[test]
    fn the_home_city_has_what_the_story_needs() {
        // the home city is coloured with a night market, the strip and the blocks, each
        // a quarter of its own (2's had no market, 11's no strip, 3's no blocks)
        for seed in 0..30 {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            let home = w.cities.iter().position(|c| c.home).unwrap() as u8;
            for need in super::STORY {
                let tiles = w
                    .city_tiles(home)
                    .iter()
                    .filter(|p| w.tile(**p).and_then(|t| t.urban).map(|u| u.theme) == Some(need))
                    .count();
                assert!(
                    tiles >= super::STORY_TILES,
                    "seed {seed}: {tiles} tiles of {need:?}"
                );
            }
        }
    }

    #[test]
    fn the_home_city_is_big_and_spans_its_river() {
        for seed in 0..20 {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            let tiles = w.tiles.count(|t| t.urban.is_some_and(|u| u.city == 0));
            assert!(tiles > 200, "seed {seed}: home only {tiles} tiles");
            // Where the river runs through the city, the city is on both banks.
            let home = |p: Pos| {
                w.tile(p).is_some_and(|t| {
                    t.water != Water::River && t.urban.is_some_and(|u| u.city == 0)
                })
            };
            let river: Vec<Pos> = w
                .tiles
                .iter()
                .filter(|(_, t)| t.water == Water::River && t.urban.is_some_and(|u| u.city == 0))
                .map(|(p, _)| p)
                .collect();
            if river.len() >= 4 {
                let bank_to_bank = river.iter().any(|p| {
                    (home(p.offset(-1, 0)) && home(p.offset(1, 0)))
                        || (home(p.offset(0, -1)) && home(p.offset(0, 1)))
                });
                assert!(bank_to_bank, "seed {seed}: the city stops at its river");
            }
        }
    }
}
