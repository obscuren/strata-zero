//! A city's outskirts on its site (#20): where it thins, its last blocks are round 2's
//! lots, yards and shanties, its sidewalks gravel, its last lamps along the roads. See
//! `2026-11-04-cities-in-the-world-design.md`, "The outskirts".

use super::{Block, GenCtx, Lot, LotUse};
use crate::geom::{Pos, Rect};
use crate::rng::hash64;
use crate::tile::{Feature, Furniture, Terrain};
use crate::world::{BuildingKind, StratumKind};
use rand_chacha::ChaCha8Rng;

const K: StratumKind = StratumKind::Street;

/// What an outskirts block holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layout {
    /// A gravel lot of parked rigs, a body shop at its back.
    Lot,
    /// A fenced scrapyard of wrecks and scrap.
    Scrapyard,
    /// Shanties on the dirt round fire barrels.
    Shanties,
    /// Nothing: the world's own ground.
    Bare,
}

/// What billboards say.
const BRANDS: [&str; 8] = [
    "KIRO", "NANOFIX", "SUNWAVE", "OKAMI", "VOLTEK", "MIRA", "SODA 8", "ZENITH",
];

/// A hash of `p` for `salt`, for this city.
fn roll(seed: u64, salt: u64, p: Pos) -> u64 {
    hash64(seed ^ salt, ((p.x as u32 as u64) << 32) | p.y as u32 as u64)
}

/// Lays outskirts block `b` in place of its lots.
pub(crate) fn lay(ctx: &mut GenCtx, rng: &mut ChaCha8Rng, b: Block) {
    let r = b.rect;
    let h = roll(ctx.seed, 0x0575, Pos::new(r.x, r.y));
    let layout = match h % 100 {
        0..30 => Layout::Lot,
        30..52 => Layout::Scrapyard,
        52..86 => Layout::Shanties,
        _ => Layout::Bare,
    };
    match layout {
        Layout::Lot => lot(ctx, rng, b),
        Layout::Scrapyard => scrapyard(ctx, b),
        Layout::Shanties => shanties(ctx, b),
        Layout::Bare => bare(ctx, r),
    }
    if layout != Layout::Bare && (h >> 8).is_multiple_of(2) {
        billboard(ctx, b, h >> 16);
    }
}

/// A paved lot of parked rigs, a lamp at its corner, and a body shop at its back where
/// the block is big enough.
fn lot(ctx: &mut GenCtx, rng: &mut ChaCha8Rng, b: Block) {
    let r = b.rect;
    ctx.stratum_fill(K, r, Terrain::Plaza);
    let (sw, sh) = if r.w >= 10 && r.h >= 7 {
        (8, 5)
    } else {
        (6, 4)
    };
    let shop = (r.w >= sw + 2 && r.h >= sh + 2)
        .then(|| Rect::new(r.right() - sw, r.bottom() - sh, sw, sh));
    for y in (r.y + 1..r.bottom() - 1).step_by(3) {
        for x in r.x + 1..r.right() - 1 {
            let p = Pos::new(x, y);
            if shop.is_none_or(|s| !s.grow(1).contains(p))
                && !roll(ctx.seed, 0x1075, p).is_multiple_of(3)
            {
                ctx.set_feature(K, p, Some(Feature::Furniture(Furniture::Couch)));
            }
        }
    }
    ctx.set_feature(K, Pos::new(r.x, r.y), Some(Feature::Lamp));
    ctx.scratch.lots.push(Lot {
        rect: r,
        district: b.district,
        use_: LotUse::Parking,
    });
    if let Some(s) = shop {
        let district = ctx.city.district(b.district).clone();
        super::lots::place_building(
            ctx,
            rng,
            s,
            BuildingKind::Garage,
            district.theme.spec(),
            district.id,
        );
    }
}

/// A scrapyard: its fence round the block with a gate on its south side, gravel inside,
/// wrecks and scrap.
fn scrapyard(ctx: &mut GenCtx, b: Block) {
    let r = b.rect;
    let gate = r.x + r.w / 2;
    for p in r.iter() {
        let rim = p.x == r.x || p.y == r.y || p.x == r.right() - 1 || p.y == r.bottom() - 1;
        let in_gate = p.y == r.bottom() - 1 && (p.x == gate || p.x == gate - 1);
        if rim && !in_gate {
            ctx.set_terrain(K, p, Terrain::Railing);
            continue;
        }
        ctx.set_terrain(K, p, Terrain::Gravel);
        if rim {
            continue;
        }
        // a way in from the gate kept clear
        if p.x == gate || p.x == gate - 1 {
            continue;
        }
        let k = roll(ctx.seed, 0x5C8A, p) % 100;
        let f = match k {
            0..22 => Some(Feature::Trash),
            22..32 => Some(Feature::Furniture(Furniture::Couch)),
            _ => None,
        };
        ctx.set_feature(K, p, f);
    }
    ctx.scratch.lots.push(Lot {
        rect: r,
        district: b.district,
        use_: LotUse::Plaza,
    });
}

/// Shanties: dirt, up to five shacks with their doors on the dirt, fire barrels between.
fn shanties(ctx: &mut GenCtx, b: Block) {
    let r = b.rect;
    ctx.stratum_fill(K, r, Terrain::Dirt);
    let mut shacks: Vec<Rect> = Vec::new();
    for k in 0..16u64 {
        if shacks.len() == 5 {
            break;
        }
        let h = roll(ctx.seed, 0x5AA7 + k, Pos::new(r.x, r.y));
        let (w, d) = (4 + (h % 3) as i32, 3 + ((h >> 4) % 2) as i32);
        if w > r.w || d > r.h {
            continue;
        }
        let x = r.x + ((h >> 8) % (r.w - w + 1) as u64) as i32;
        let y = r.y + ((h >> 20) % (r.h - d + 1) as u64) as i32;
        let s = Rect::new(x, y, w, d);
        if shacks.iter().any(|o| o.grow(1).intersects(s)) {
            continue;
        }
        shacks.push(s);
    }
    for s in &shacks {
        for p in s.iter() {
            let rim = p.x == s.x || p.y == s.y || p.x == s.right() - 1 || p.y == s.bottom() - 1;
            ctx.set_terrain(K, p, if rim { Terrain::Wall } else { Terrain::Floor });
        }
        // the door on its south wall, onto the dirt or the alley round the block
        ctx.set_terrain(K, Pos::new(s.x + s.w / 2, s.bottom() - 1), Terrain::Door);
    }
    // one or two fires in the open beside the shacks, and the litter of living out
    let open = |ctx: &GenCtx, p: Pos| {
        ctx.terrain(K, p) == Terrain::Dirt && shacks.iter().all(|s| !s.grow(1).contains(p))
    };
    let mut by_shacks: Vec<Pos> = r
        .inset(1)
        .iter()
        .filter(|p| open(ctx, *p) && shacks.iter().any(|s| s.grow(2).contains(*p)))
        .collect();
    by_shacks.sort_by_key(|p| roll(ctx.seed, 0xF14E, *p));
    let fires = 1 + (roll(ctx.seed, 0xF14F, Pos::new(r.x, r.y)) % 2) as usize;
    for p in by_shacks.into_iter().take(fires) {
        ctx.set_feature(K, p, Some(Feature::FireBarrel));
    }
    for p in r.inset(1).iter() {
        if open(ctx, p)
            && ctx.tile(K, p).is_some_and(|t| t.feature.is_none())
            && roll(ctx.seed, 0x7A54, p) % 100 < 4
        {
            ctx.set_feature(K, p, Some(Feature::Trash));
        }
    }
    ctx.scratch.lots.push(Lot {
        rect: r,
        district: b.district,
        use_: LotUse::Plaza,
    });
}

/// Nothing: the block is left out of the city, for the world's ground to fill.
fn bare(ctx: &mut GenCtx, r: Rect) {
    for p in r.iter() {
        for k in StratumKind::PHYSICAL {
            if let Some(t) = ctx.city.tile_mut(k, p) {
                t.district = None;
            }
        }
    }
}

/// How far past a billboard's block a way round it is looked for: the street round it.
const ROUND: i32 = 3;

/// Whether a billboard standing on `shape` in block `r` (its letters and posts, all
/// solid) would part a way through: a scrapyard's gate, a gap between shacks, a pocket
/// behind its posts. A way round it is looked for in the block and the street round it,
/// four ways and eight (as the street's reach is found, and as you walk).
fn cuts_a_way(ctx: &GenCtx, r: Rect, shape: &[Pos]) -> bool {
    crate::pathing::cuts(
        r.grow(ROUND),
        |p| ctx.tile(K, p).is_some_and(|t| t.is_walkable()),
        shape,
    )
}

/// Where a billboard may stand: a cell off the centre of its side (-1, 0 or 1), and its
/// posts a cell in from its ends or under them.
#[derive(Clone, Copy)]
struct Spot {
    shift: i32,
    under_caps: bool,
}

/// A billboard along the block's north or south side, whichever faces the denser city
/// (the other if that one is built on, or a billboard there would cut a way through; the
/// next brand, of another length, if neither side takes it): a brand between two caps,
/// on two posts behind, centred. One that would cut a way through wherever it fits so
/// stands a cell off the centre, or on posts under its caps; nowhere at all if none of
/// those will do. It stands in the way, as its posts do.
fn billboard(ctx: &mut GenCtx, b: Block, h: u64) {
    let r = b.rect;
    let Some(site) = ctx.scratch.site.as_ref() else {
        return;
    };
    let at = |p: Pos| site.density.get(p).copied().unwrap_or(0.0);
    let c = Pos::new(r.x + r.w / 2, r.y + r.h / 2);
    let north = (r.y, 1);
    let south = (r.bottom() - 1, -1);
    let sides = if at(c.offset(0, -r.h / 2 - 10)) >= at(c.offset(0, r.h / 2 + 10)) {
        [north, south]
    } else {
        [south, north]
    };
    let built = |ctx: &GenCtx, p: Pos| {
        matches!(
            ctx.terrain(K, p),
            Terrain::Wall | Terrain::Door | Terrain::Floor | Terrain::Window
        ) || ctx.tile(K, p).is_some_and(|t| t.building.is_some())
    };
    let brands: Vec<&str> = (0..BRANDS.len())
        .map(|k| BRANDS[(h as usize + k) % BRANDS.len()])
        .collect();
    let centred = [Spot {
        shift: 0,
        under_caps: false,
    }];
    let off = [(-1, false), (1, false), (0, true), (-1, true), (1, true)]
        .map(|(shift, under_caps)| Spot { shift, under_caps });
    let mut refused = 0;
    for (pass, spots) in [&centred[..], &off[..]].into_iter().enumerate() {
        // off the centre only to move one that would cut a way through
        if pass > 0 && refused == 0 {
            break;
        }
        for brand in &brands {
            let text: Vec<char> = std::iter::once('▣')
                .chain(brand.chars())
                .chain(std::iter::once('▣'))
                .collect();
            let n = text.len() as i32;
            if n + 2 > r.w {
                continue;
            }
            for (y, inward) in sides {
                for spot in spots {
                    let x0 = r.x + (r.w - n) / 2 + spot.shift;
                    if x0 < r.x + 1 || x0 + n > r.right() - 1 {
                        continue;
                    }
                    let cells: Vec<Pos> = (0..n).map(|k| Pos::new(x0 + k, y)).collect();
                    let inset = if spot.under_caps { 0 } else { 1 };
                    let posts = [
                        Pos::new(x0 + inset, y + inward),
                        Pos::new(x0 + n - 1 - inset, y + inward),
                    ];
                    let shape: Vec<Pos> = cells.iter().chain(&posts).copied().collect();
                    if shape.iter().any(|p| built(ctx, *p)) {
                        continue;
                    }
                    if cuts_a_way(ctx, r, &shape) {
                        refused += 1;
                        continue;
                    }
                    for (p, ch) in cells.iter().zip(&text) {
                        ctx.set_feature(K, *p, Some(Feature::Sign(*ch)));
                    }
                    for p in posts {
                        ctx.set_feature(K, p, Some(Feature::Pipe));
                    }
                    return;
                }
            }
        }
    }
}

/// After every block: the belt's sidewalks and alleys gravel shoulders and tracks (but
/// the pavement at its last buildings' doors), and the last lamps along its roads.
pub(crate) fn belt(ctx: &mut GenCtx) {
    let Some(site) = ctx.scratch.site.as_ref() else {
        return;
    };
    let size = ctx.size();
    let mut gravel = Vec::new();
    let mut lamps = Vec::new();
    for y in 0..size.h as i32 {
        for x in 0..size.w as i32 {
            let p = Pos::new(x, y);
            if !site.outskirts(p) {
                continue;
            }
            let t = ctx.terrain(K, p);
            let at_a_building = p
                .neighbors8()
                .any(|q| ctx.tile(K, q).is_some_and(|u| u.building.is_some()));
            if matches!(t, Terrain::Sidewalk | Terrain::Alley) && !at_a_building {
                gravel.push(p);
                let by_road = p.neighbors4().any(|q| ctx.terrain(K, q) == Terrain::Road);
                if by_road && (x * 7 + y * 13).rem_euclid(29) == 0 && site.density[p] > 0.06 {
                    lamps.push(p);
                }
            }
        }
    }
    for p in gravel {
        ctx.set_terrain(K, p, Terrain::Gravel);
    }
    for p in lamps {
        if ctx.tile(K, p).is_some_and(|t| t.feature.is_none()) {
            ctx.set_feature(K, p, Some(Feature::Lamp));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::generate::Generator;
    use crate::generate::connectivity::reachable_from_street;
    use crate::generate::site::{OUTSKIRTS, Site, prepare, sited};
    use crate::geom::Pos;
    use crate::overworld::{Overworld, WorldParams};
    use crate::tile::{Feature, Terrain};
    use crate::world::{BuildingKind, City, StratumKind};
    use crate::{GenParams, SizePreset};
    use std::sync::OnceLock;

    /// The home cities of three worlds, each with its site, built once.
    fn homes() -> &'static Vec<(u64, City, Site)> {
        static HOMES: OnceLock<Vec<(u64, City, Site)>> = OnceLock::new();
        HOMES.get_or_init(|| {
            [3u64, 8, 41]
                .into_iter()
                .map(|seed| {
                    let w = Overworld::generate(WorldParams {
                        seed,
                        ..Default::default()
                    })
                    .unwrap();
                    let i = w.cities.iter().position(|c| c.home).unwrap() as u8;
                    let p = sited(
                        &w,
                        i,
                        GenParams {
                            seed,
                            ..Default::default()
                        },
                    );
                    let c = Generator::run(p).unwrap();
                    let site = prepare(c.params.site.as_ref().unwrap(), c.size).unwrap();
                    (seed, c, site)
                })
                .collect()
        })
    }

    fn street(c: &City) -> &crate::geom::Grid<crate::tile::Tile> {
        &c.stratum(StratumKind::Street).unwrap().tiles
    }

    fn middle(r: crate::geom::Rect) -> Pos {
        Pos::new(r.x + r.w / 2, r.y + r.h / 2)
    }

    #[test]
    fn outskirts_lie_where_the_city_thins() {
        // a yard's fence, a fire, a shanty: only where the city is thin (a big block
        // runs a little up the slope; past the outline a POI's rooms are the world's);
        // its densest part keeps its full blocks
        for (seed, c, site) in homes() {
            for (p, t) in street(c).iter() {
                let outskirt = t.feature == Some(Feature::FireBarrel)
                    || (t.terrain == Terrain::Railing && site.builds(p))
                    || (t.terrain == Terrain::Door && t.building.is_none() && site.builds(p));
                if outskirt {
                    assert!(
                        site.density[p] < OUTSKIRTS + 0.12,
                        "seed {seed}: {:?} at {p:?} where the density is {}",
                        t.terrain,
                        site.density[p]
                    );
                }
            }
            let core = c
                .buildings
                .iter()
                .filter(|b| site.density[middle(b.footprint)] > 0.6)
                .count();
            // (the core is the overworld's corp core, its towers on big lots)
            assert!(core >= 12, "seed {seed}: {core} buildings in the core");
        }
    }

    #[test]
    fn the_outskirts_have_their_kinds() {
        // over three home cities: a body shop, a fenced scrapyard, a shanty with a fire
        // by it, a billboard facing the city
        let (mut shops, mut fence, mut fires, mut shanties, mut boards) = (0, 0, 0, 0, 0);
        for (_, c, site) in homes() {
            shops += c
                .buildings
                .iter()
                .filter(|b| {
                    let m = middle(b.footprint);
                    b.kind == BuildingKind::Garage
                        && site.builds(m)
                        && site.density[m] < OUTSKIRTS + 0.1
                })
                .count();
            for (p, t) in street(c).iter() {
                fence += (t.terrain == Terrain::Railing && site.builds(p)) as usize;
                fires += (t.feature == Some(Feature::FireBarrel)) as usize;
                shanties += (t.terrain == Terrain::Door && t.building.is_none()) as usize;
                boards += (t.feature == Some(Feature::Sign('▣'))) as usize;
            }
        }
        assert!(shops >= 1, "{shops} body shops");
        assert!(fence >= 20, "{fence} cells of yard fence");
        assert!(fires >= 2, "{fires} fires");
        assert!(shanties >= 3, "{shanties} shanties");
        assert!(boards >= 2, "{boards} billboard caps");
    }

    /// Every billboard of `c`'s street: its letters and caps on the ground (not a
    /// building's wall) where the city builds (past its outline a sign is a POI's), and
    /// the posts behind them (a cell in from its ends, or under them).
    fn billboards(c: &City, site: &Site) -> Vec<Vec<Pos>> {
        let st = street(c);
        let ground_sign = |p: Pos| {
            st.get(p).is_some_and(|t| {
                matches!(t.feature, Some(Feature::Sign(_)))
                    && !matches!(t.terrain, Terrain::Wall | Terrain::Window | Terrain::Mass)
                    && site.builds(p)
            })
        };
        let pipe = |p: Pos| st.get(p).is_some_and(|t| t.feature == Some(Feature::Pipe));
        st.iter()
            .map(|(p, _)| p)
            .filter(|p| ground_sign(*p) && !ground_sign(p.offset(-1, 0)))
            .map(|p| {
                let n = (0..).take_while(|k| ground_sign(p.offset(*k, 0))).count() as i32;
                let mut shape: Vec<Pos> = (0..n).map(|k| p.offset(k, 0)).collect();
                for x in [p.x, p.x + 1, p.x + n - 2, p.x + n - 1] {
                    for dy in [-1, 1] {
                        let q = Pos::new(x, p.y + dy);
                        if pipe(q) {
                            shape.push(q);
                        }
                    }
                }
                shape
            })
            .collect()
    }

    #[test]
    fn a_billboard_never_cuts_a_way_through() {
        // where it stands, every way through the street that was there without it is
        // still there (four ways and eight); and it stands whole, a brand between its
        // caps (nothing after it took a letter down)
        use crate::pathing::cuts;
        let mut boards = 0;
        for (seed, c, site) in homes() {
            let st = street(c);
            for shape in billboards(c, site) {
                let text: String = shape
                    .iter()
                    .filter_map(|p| match st[*p].feature {
                        Some(Feature::Sign(ch)) => Some(ch),
                        _ => None,
                    })
                    .collect();
                let brand = text.strip_prefix('▣').and_then(|t| t.strip_suffix('▣'));
                assert!(
                    brand.is_some_and(|b| super::BRANDS.contains(&b)),
                    "seed {seed}: the billboard at {:?} reads {text}",
                    shape[0]
                );
                let window = c.size.rect().grow(1);
                let without = |q: Pos| {
                    st.get(q).is_some_and(|t| {
                        let mut t = *t;
                        if shape.contains(&q) {
                            t.feature = None;
                        }
                        t.is_walkable()
                    })
                };
                assert!(
                    !cuts(window, without, &shape),
                    "seed {seed}: the billboard at {:?} cuts a way through",
                    shape[0]
                );
                boards += 1;
            }
        }
        assert!(boards >= 2, "{boards} billboards");
    }

    /// The cities and towns of world `seed` as a game builds them on their sites
    /// (`screens::build_home`, `outside::build_params`): the home city on the world's own
    /// seed, the rest each on a seed of its own. Each named as the world names it.
    fn settlements_of(seed: u64) -> Vec<(String, GenParams)> {
        use crate::generate::site::sited_town;
        use crate::overworld::WorldSize;
        use crate::rng::hash64;
        let d = WorldParams::default();
        let crate::geom::Size { w, h } = d.size.size();
        let size = WorldSize::Custom { w, h };
        let world = Overworld::generate(WorldParams { seed, size, ..d }).unwrap();
        let base = GenParams {
            size: SizePreset::Large,
            ..Default::default()
        };
        let home = world.cities.iter().position(|c| c.home).unwrap_or(0) as u8;
        let mut out = vec![(
            world.settlement_name(home, None),
            sited(&world, home, base.clone().with_seed(seed)),
        )];
        for (i, c) in world.cities.iter().enumerate() {
            let i = i as u8;
            let own = |t: Option<u8>| {
                let at = (u64::from(i) << 8) | t.map_or(0, |t| u64::from(t) + 1);
                base.clone()
                    .with_seed(hash64(world.seed ^ 0xC17E, at) % 1_000_000_007)
            };
            if i != home {
                out.push((c.name.clone(), sited(&world, i, own(None))));
            }
            for t in 0..c.towns.len() as u8 {
                if let Some(p) = sited_town(&world, i, t, own(Some(t))) {
                    out.push((format!("{}'s town {t}", c.name), p));
                }
            }
        }
        out
    }

    /// The signs `c` has lost: each billboard of its street that doesn't read a brand
    /// between its caps, and each sign of the world's ground past its outline it didn't
    /// keep.
    fn lost_signs(c: &City, site: &Site) -> Vec<String> {
        let st = street(c);
        let mut lost = Vec::new();
        for shape in billboards(c, site) {
            let text: String = shape
                .iter()
                .filter_map(|p| match st[*p].feature {
                    Some(Feature::Sign(ch)) => Some(ch),
                    _ => None,
                })
                .collect();
            let brand = text.strip_prefix('▣').and_then(|t| t.strip_suffix('▣'));
            if !brand.is_some_and(|b| super::BRANDS.contains(&b)) {
                lost.push(format!("the billboard at {:?} reads {text}", shape[0]));
            }
        }
        for (p, t) in site.ground.iter() {
            if !site.builds(p)
                && matches!(t.feature, Some(Feature::Sign(_)))
                && st[p].feature != t.feature
            {
                lost.push(format!("the world's {:?} at {p:?} is gone", t.feature));
            }
        }
        lost
    }

    #[test]
    fn a_billboard_stands_whole() {
        // the cities where the review found a billboard with letters or a cap taken down
        // by the street's dressing (and 4's Emberfall, as 4's Porthaven has none since
        // its districts are its colours): each reads its brand between its caps
        for (seed, name) in [
            (4, "Emberfall"),
            (25, "Chromehaven"),
            (33, "Greyside"),
            (38, "Hollowmouth"),
        ] {
            let (_, params) = settlements_of(seed)
                .into_iter()
                .find(|(n, _)| n == name)
                .unwrap();
            let c = Generator::run(params).unwrap();
            let site = prepare(c.params.site.as_ref().unwrap(), c.size).unwrap();
            let boards = billboards(&c, &site).len();
            assert!(boards >= 1, "seed {seed} {name}: no billboard");
            let lost = lost_signs(&c, &site);
            assert!(lost.is_empty(), "seed {seed} {name}: {lost:?}");
        }
    }

    #[test]
    fn a_worlds_sign_past_the_outline_stays() {
        // a test range's warnings in the world's ground past seed 49 Neoreach's outline,
        // beside ground its street never reaches: the dressing leaves them standing
        let (_, params) = settlements_of(49)
            .into_iter()
            .find(|(n, _)| n == "Neoreach")
            .unwrap();
        let c = Generator::run(params).unwrap();
        let site = prepare(c.params.site.as_ref().unwrap(), c.size).unwrap();
        let lost = lost_signs(&c, &site);
        assert!(lost.is_empty(), "seed 49 Neoreach: {lost:?}");
    }

    #[test]
    #[ignore = "every city and town of 80 worlds: minutes in the debug profile"]
    fn every_sign_stands_whole() {
        let (mut boards, mut lost) = (0, 0);
        for seed in 0..80 {
            for (name, params) in settlements_of(seed) {
                let Ok(c) = Generator::run(params) else {
                    continue;
                };
                let site = prepare(c.params.site.as_ref().unwrap(), c.size).unwrap();
                boards += billboards(&c, &site).len();
                for l in lost_signs(&c, &site) {
                    println!("seed {seed} {name}: {l}");
                    lost += 1;
                }
            }
        }
        println!("{lost} lost of {boards} billboards and the world's signs");
        assert_eq!(lost, 0);
    }

    #[test]
    fn an_outskirts_door_is_reached_on_foot() {
        for (seed, c, _) in homes() {
            let reach = reachable_from_street(c);
            for (p, t) in street(c).iter() {
                if t.terrain == Terrain::Door && t.building.is_none() {
                    assert!(
                        reach[StratumKind::Street.index()][p],
                        "seed {seed}: a shanty's door at {p:?} is cut off"
                    );
                }
            }
        }
    }

    #[test]
    fn the_belt_has_no_sidewalks() {
        for (seed, c, site) in homes() {
            let st = street(c);
            let walks = st
                .iter()
                .filter(|(p, t)| {
                    t.terrain == Terrain::Sidewalk
                        && site.builds(*p)
                        && site.density[*p] < OUTSKIRTS - 0.05
                        && !p
                            .neighbors8()
                            .any(|q| st.get(q).is_some_and(|u| u.building.is_some()))
                })
                .map(|(p, _)| p)
                .collect::<Vec<Pos>>();
            assert!(
                walks.is_empty(),
                "seed {seed}: sidewalk in the belt at {:?}",
                walks.first()
            );
        }
    }

    fn net(c: &City) -> &crate::geom::Grid<crate::tile::Tile> {
        &c.stratum(StratumKind::Net).unwrap().tiles
    }

    #[test]
    fn the_grid_fades_through_the_outskirts() {
        // the grid's dots and the districts' tint thin out where the city does
        for (seed, c, site) in homes() {
            let dots = |thin: bool| {
                let cells: Vec<_> = net(c)
                    .iter()
                    .filter(|(p, t)| {
                        t.terrain == Terrain::NetGrid
                            && site.builds(*p)
                            && p.x % 4 == 0
                            && p.y % 2 == 0
                            && (site.density[*p] < OUTSKIRTS) == thin
                            && (thin || site.density[*p] > 0.5)
                    })
                    .map(|(_, t)| t)
                    .collect();
                let lit = cells.iter().filter(|t| t.variant & 1 == 1).count();
                lit as f32 / cells.len().max(1) as f32
            };
            let (belt, core) = (dots(true), dots(false));
            assert!(
                belt * 2.0 < core,
                "seed {seed}: dots {belt} in the belt, {core} in the core"
            );
        }
    }

    #[test]
    fn past_the_outline_the_net_is_the_open_nets() {
        for (seed, c, site) in homes() {
            let o = site.origin;
            let open =
                crate::overworld::net_ground::region(&site.world, o.x, o.y, c.size.w, c.size.h);
            let mask = crate::generate::cyberspace::open_net_mask(street(c), site);
            assert!(mask.count(|m| *m) > 1000, "seed {seed}: little open Net");
            let mut off = 0;
            for (p, t) in net(c).iter() {
                if mask[p] && (t.terrain, t.feature) != (open[p].terrain, open[p].feature) {
                    off += 1;
                }
            }
            assert_eq!(
                off, 0,
                "seed {seed}: {off} cells past the outline aren't the open Net's"
            );
        }
    }

    #[test]
    fn a_trunk_meets_the_citys_grid() {
        // the open Net's trunk along a highway runs into the city's own walkable Net
        for (seed, c, site) in homes() {
            let n = net(c);
            let mask = crate::generate::cyberspace::open_net_mask(street(c), site);
            let met = n.iter().any(|(p, t)| {
                t.terrain == Terrain::NetLane
                    && mask[p]
                    && p.neighbors4().any(|q| {
                        mask.get(q).is_some_and(|m| !m) && n.get(q).is_some_and(|u| u.is_walkable())
                    })
            });
            assert!(met, "seed {seed}: no trunk meets the city's grid");
        }
    }

    #[test]
    fn a_shanty_splices_a_line() {
        // the shanties are off the grid, but in every city that has them one has a
        // stolen line off a lane
        for (seed, c, _) in homes() {
            let doors: Vec<Pos> = street(c)
                .iter()
                .filter(|(_, t)| t.terrain == Terrain::Door && t.building.is_none())
                .map(|(p, _)| p)
                .collect();
            if doors.is_empty() {
                continue;
            }
            let spliced = doors.iter().any(|p| {
                net(c)
                    .get(*p)
                    .is_some_and(|u| u.terrain == Terrain::NetLane)
            });
            assert!(spliced, "seed {seed}: no shanty taps a line");
        }
    }

    #[test]
    fn the_open_net_doesnt_cut_the_citys_net() {
        // past the outline the open Net's void must not cut a corner of the city's grid
        // off its hub (109's, 145's and 165's home cities sealed a construct's gate)
        for seed in [109u64, 145, 165] {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            let i = w.cities.iter().position(|c| c.home).unwrap() as u8;
            let p = sited(
                &w,
                i,
                GenParams {
                    seed,
                    ..Default::default()
                },
            );
            let built = Generator::run(p);
            assert!(built.is_ok(), "seed {seed}: {:?}", built.err());
        }
    }

    #[test]
    fn a_lone_city_has_no_outskirts() {
        let c = Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Medium,
            ..Default::default()
        })
        .unwrap();
        assert!(street(&c).iter().all(|(_, t)| {
            t.feature != Some(Feature::FireBarrel) && t.terrain != Terrain::Railing
        }));
    }
}
