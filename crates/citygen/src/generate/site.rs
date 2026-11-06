//! A city grown from its place in the world: its water, outline and highways from the
//! world, the open ground past its outline. See `2026-11-04-cities-in-the-world-design.md`.

use crate::geom::{Grid, Pos, Size};
use crate::overworld::field::Fbm;
use crate::overworld::{Overworld, detail};
use crate::params::{CitySite, GenParams, SizePreset};
use crate::tile::{Terrain, Tile};

/// Where the city's outline runs: halfway between its own overworld tiles and the
/// others round them, so the city builds over its whole footprint on the overworld.
pub const OUTLINE: f32 = 0.5;

/// How dense the city is where its outskirts begin: below it, inside the outline, its
/// last blocks are round 2's outskirts (lots, yards, shanties) and not full blocks.
pub const OUTSKIRTS: f32 = 0.08;

/// The world's part in a city being generated: its block's ground, how much each cell
/// is the city's (its own tiles blended across tile lines, with noise: the outline runs
/// at [`OUTLINE`]), and how dense the city is there (its tiles' density, blended).
pub(crate) struct Site {
    pub world: Overworld,
    /// The block's corner, in world-wide detail cells.
    pub origin: Pos,
    pub ground: Grid<Tile>,
    pub outline: Grid<f32>,
    pub density: Grid<f32>,
}

/// `base` set to build world `world`'s city `city` on its block: its size the block's
/// in detail cells, its site the world and the block's corner.
pub fn sited(world: &Overworld, city: u8, base: GenParams) -> GenParams {
    let own = world.city_tiles(city).len();
    on_block(world, city, None, world.city_block(city), own, base)
}

/// `base` set to build town `town` of world `world`'s city `city` on a small map of its
/// own; none when the town has grown into its city.
pub fn sited_town(world: &Overworld, city: u8, town: u8, base: GenParams) -> Option<GenParams> {
    let b = world.town_block(city, town)?;
    let own = world.town_tiles(city, town).len();
    Some(on_block(world, city, Some(town), b, own, base))
}

/// Overworld tiles of a city to one of its districts: a small city has fewer, a town of
/// a few tiles one.
const TILES_A_DISTRICT: usize = 50;

/// `base` on block `b` of the world: its size, its site, and its districts by its `own`
/// tiles (never more than `base` asks for).
fn on_block(
    world: &Overworld,
    city: u8,
    town: Option<u8>,
    b: crate::geom::Rect,
    own: usize,
    base: GenParams,
) -> GenParams {
    let t = detail::TILE;
    let (w, h) = ((t * b.w) as u32, (t * b.h) as u32);
    let fits = ((w as u64 * h as u64) / 800).min(12) as usize;
    let districts = (own / TILES_A_DISTRICT).clamp(1, fits.max(1));
    GenParams {
        size: SizePreset::Custom { w, h },
        districts: (districts as u8).min(base.districts),
        site: Some(CitySite {
            world: world.params.clone(),
            city,
            origin: Pos::new(t * b.x, t * b.y),
            town,
        }),
        ..base
    }
}

/// The site's ground and outline, built from its world.
pub(crate) fn prepare(site: &CitySite, size: Size) -> Option<Site> {
    let world = Overworld::generate(site.world.clone()).ok()?;
    let o = site.origin;
    let ground = detail::region(&world, o.x, o.y, size.w, size.h);
    let edge = Fbm::new(world.seed, 4101);
    let mut outline = Grid::filled(size, 0.0f32);
    let mut density = Grid::filled(size, 0.0f32);
    let mut own = Grid::filled(world.size, false);
    let tiles = match site.town {
        Some(t) => world.town_tiles(site.city, t),
        None => world.city_tiles(site.city),
    };
    for t in tiles {
        own[t] = true;
    }
    for y in 0..size.h as i32 {
        for x in 0..size.w as i32 {
            let (wx, wy) = ((o.x + x) as f32, (o.y + y) as f32);
            let mine = detail::blend_tiles(wx, wy, |t| {
                if own.get(t).copied().unwrap_or(false) {
                    1.0
                } else {
                    0.0
                }
            });
            // the outline runs through tiles, not along them: a wobble a block or two
            // across, and a finer one
            outline[Pos::new(x, y)] = mine
                + 0.22 * detail::wave(&edge, wx / 24.0, wy / 12.0)
                + 0.08 * detail::wave(&edge, wx / 7.0 + 31.0, wy / 3.5);
            density[Pos::new(x, y)] = detail::blend_tiles(wx, wy, |t| {
                match (own.get(t), world.tile(t).and_then(|w| w.urban)) {
                    (Some(true), Some(u)) => u.density as f32 / 255.0,
                    _ => 0.0,
                }
            });
        }
    }
    Some(Site {
        world,
        origin: o,
        ground,
        outline,
        density,
    })
}

impl Site {
    /// Whether the city builds at `p`: dry, and inside its outline.
    pub fn builds(&self, p: Pos) -> bool {
        !self.wet(p) && self.outline.get(p).is_some_and(|d| *d >= OUTLINE)
    }

    /// Whether `p` is in the city's outskirts: inside its outline where it is thin.
    pub fn outskirts(&self, p: Pos) -> bool {
        self.builds(p) && self.density.get(p).is_some_and(|d| *d < OUTSKIRTS)
    }

    /// Whether the world has water at `p`.
    pub fn wet(&self, p: Pos) -> bool {
        self.ground.get(p).is_some_and(|t| {
            matches!(
                t.terrain,
                Terrain::Water | Terrain::Shallows | Terrain::Ford
            )
        })
    }

    /// The world's highway at `p`, as the city's road (a road, or a bridge over water),
    /// where the city reaches (a little past its outline, so the roads meet it).
    pub fn highway(&self, p: Pos) -> Option<Terrain> {
        let near = self.outline.get(p).is_some_and(|d| *d >= OUTLINE - 0.2);
        self.ground
            .get(p)
            .map(|t| t.terrain)
            .filter(|t| near && matches!(t, Terrain::Road | Terrain::Bridge))
    }
}

/// Past the outline (every Street cell the city left empty), the open ground.
pub(crate) fn fill(city: &mut crate::world::City, site: &Site) {
    let Some(street) = city.stratum_mut(crate::world::StratumKind::Street) else {
        return;
    };
    let size = street.tiles.size();
    for y in 0..size.h as i32 {
        for x in 0..size.w as i32 {
            let p = Pos::new(x, y);
            let (Some(t), Some(g)) = (street.tiles.get_mut(p), site.ground.get(p)) else {
                continue;
            };
            if t.terrain == Terrain::Void && t.building.is_none() {
                t.terrain = g.terrain;
                t.feature = g.feature;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::Generator;
    use crate::geom::Pos;
    use crate::overworld::WorldParams;
    use crate::tile::Terrain;
    use crate::world::{City, StratumKind};
    use std::sync::OnceLock;

    /// World `seed`'s home city built on its block.
    fn home_of(seed: u64) -> (Overworld, City) {
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
        (w, c)
    }

    /// The world's ground under city `c`'s block.
    fn ground_of(w: &Overworld, c: &City) -> crate::geom::Grid<Tile> {
        let o = c.params.site.as_ref().unwrap().origin;
        detail::region(w, o.x, o.y, c.size.w, c.size.h)
    }

    #[test]
    fn a_city_bridges_only_water() {
        // its arterials and its repairs cross the world's water on bridges, never its
        // dry ground (0's and 48's arterials ran bridges over dirt)
        for seed in [0u64, 48, 8] {
            let (w, c) = home_of(seed);
            let g = ground_of(&w, &c);
            let dry = c
                .stratum(StratumKind::Street)
                .unwrap()
                .tiles
                .iter()
                .filter(|(p, t)| {
                    t.terrain == Terrain::Bridge
                        && !matches!(
                            g[*p].terrain,
                            Terrain::Water | Terrain::Shallows | Terrain::Ford | Terrain::Bridge
                        )
                })
                .count();
            assert_eq!(dry, 0, "seed {seed}: {dry} bridge cells on dry ground");
        }
    }

    #[test]
    fn a_citys_edge_is_the_worlds_ground() {
        // every cell round the block's edge is the world's own (7's and 45's repairs ran
        // bridges along the edge; every coast's shallows were deepened)
        for seed in [3u64, 7, 45, 12] {
            let (w, c) = home_of(seed);
            let g = ground_of(&w, &c);
            let st = &c.stratum(StratumKind::Street).unwrap().tiles;
            let (cw, ch) = (c.size.w as i32, c.size.h as i32);
            let off: Vec<Pos> = (0..ch)
                .flat_map(|y| [Pos::new(0, y), Pos::new(cw - 1, y)])
                .chain((0..cw).flat_map(|x| [Pos::new(x, 0), Pos::new(x, ch - 1)]))
                .filter(|p| st[*p].terrain != g[*p].terrain)
                .collect();
            assert!(
                off.is_empty(),
                "seed {seed}: {} edge cells differ, first {:?}: {:?} vs {:?}",
                off.len(),
                off[0],
                st[off[0]].terrain,
                g[off[0]].terrain
            );
        }
    }

    #[test]
    fn every_home_city_has_a_clinic() {
        // the story's doctor is the ripperdoc nearest home: a home city has a clinic for
        // her (125's had none)
        for seed in [125u64, 8] {
            let (_, c) = home_of(seed);
            assert!(
                c.buildings
                    .iter()
                    .any(|b| b.kind == crate::world::BuildingKind::Clinic),
                "seed {seed}: no clinic"
            );
        }
    }

    /// Seed 8's world and its home city built on its block, once for every test.
    fn home() -> &'static (Overworld, City) {
        static HOME: OnceLock<(Overworld, City)> = OnceLock::new();
        HOME.get_or_init(|| {
            let w = Overworld::generate(WorldParams {
                seed: 8,
                ..Default::default()
            })
            .unwrap();
            let i = w.cities.iter().position(|c| c.home).unwrap() as u8;
            let p = sited(
                &w,
                i,
                GenParams {
                    seed: 8,
                    ..Default::default()
                },
            );
            (w, Generator::run(p).unwrap())
        })
    }

    fn ground_at(w: &Overworld, c: &City, p: Pos) -> Tile {
        let o = c.params.site.as_ref().unwrap().origin;
        crate::overworld::detail::region(w, o.x + p.x, o.y + p.y, 1, 1)[Pos::new(0, 0)]
    }

    #[test]
    fn a_sited_city_takes_its_blocks_size() {
        let (w, c) = home();
        let i = w.cities.iter().position(|c| c.home).unwrap() as u8;
        let b = w.city_block(i);
        assert_eq!((c.size.w as i32, c.size.h as i32), (8 * b.w, 8 * b.h));
        assert_eq!(
            c.params.site.as_ref().unwrap().origin,
            Pos::new(8 * b.x, 8 * b.y)
        );
    }

    #[test]
    fn the_worlds_water_runs_through_the_city() {
        let (w, c) = home();
        let (mut wet, mut kept) = (0, 0);
        for y in (0..c.size.h as i32).step_by(3) {
            for x in (0..c.size.w as i32).step_by(3) {
                let p = Pos::new(x, y);
                if ground_at(w, c, p).terrain == Terrain::Water {
                    wet += 1;
                    let t = c.tile(StratumKind::Street, p).unwrap().terrain;
                    kept += matches!(t, Terrain::Water | Terrain::Bridge) as i32;
                }
            }
        }
        assert!(wet > 50, "{wet} water cells sampled");
        assert!(kept * 100 >= wet * 95, "{kept} of {wet}");
    }

    #[test]
    fn no_building_stands_in_water() {
        let (w, c) = home();
        for y in (0..c.size.h as i32).step_by(2) {
            for x in (0..c.size.w as i32).step_by(2) {
                let p = Pos::new(x, y);
                if c.tile(StratumKind::Street, p).unwrap().building.is_some() {
                    let g = ground_at(w, c, p).terrain;
                    assert!(!matches!(g, Terrain::Water | Terrain::Shallows), "{p:?}");
                }
            }
        }
    }

    #[test]
    fn the_highways_come_in_as_roads() {
        let (w, c) = home();
        let (mut road, mut kept) = (0, 0);
        for y in 0..c.size.h as i32 {
            for x in 0..c.size.w as i32 {
                let p = Pos::new(x, y);
                if ground_at(w, c, p).terrain == Terrain::Road {
                    road += 1;
                    let t = c.tile(StratumKind::Street, p).unwrap();
                    kept += (matches!(t.terrain, Terrain::Road | Terrain::Bridge)
                        && t.building.is_none()) as i32;
                }
            }
        }
        assert!(road > 100, "{road}");
        assert!(
            kept * 100 >= road * 90,
            "{kept} of {road} highway cells are the city's roads"
        );
    }

    #[test]
    fn the_city_stops_at_its_outline() {
        let (_, c) = home();
        let (w, h) = (c.size.w as i32, c.size.h as i32);
        for y in 0..h {
            for x in 0..w {
                let edge = x < 12 || y < 12 || x >= w - 12 || y >= h - 12;
                if edge {
                    let t = c.tile(StratumKind::Street, Pos::new(x, y)).unwrap();
                    assert!(t.building.is_none(), "a building in the margin at {x},{y}");
                }
            }
        }
        // and past the outline it is the open ground
        let open = c.stratum(StratumKind::Street).unwrap().tiles.count(|t| {
            matches!(
                t.terrain,
                Terrain::Dirt | Terrain::Scrub | Terrain::Crops | Terrain::Rise
            )
        });
        assert!(open > 2000, "{open} cells of open ground");
    }

    #[test]
    fn a_home_city_builds_over_its_footprint() {
        // the city builds over its own tiles on the overworld, not only its densest part
        let (w, c) = home();
        let i = w.cities.iter().position(|c| c.home).unwrap() as u8;
        let own = w.city_tiles(i).len();
        let land = c
            .stratum(StratumKind::Street)
            .unwrap()
            .tiles
            .count(|t| t.district.is_some());
        let cells = own * (detail::TILE * detail::TILE) as usize;
        assert!(
            land * 10 >= cells * 7,
            "{land} cells built of {cells} its tiles hold"
        );
        assert!(c.buildings.len() >= 120, "{} buildings", c.buildings.len());
    }

    #[test]
    fn every_home_city_has_the_runners_home() {
        // the home is high in a megablock round an atrium: a home city whose Megablocks
        // district is too broken up for one still has one somewhere (8's is)
        for seed in [8u64, 3, 41] {
            let w = Overworld::generate(crate::overworld::WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            let home = w.cities.iter().position(|c| c.home).unwrap() as u8;
            let p = sited(
                &w,
                home,
                crate::GenParams {
                    seed,
                    ..Default::default()
                },
            );
            let c = crate::Generator::run(p).unwrap();
            assert!(crate::interior::home(&c).is_some(), "seed {seed}: no home");
        }
    }

    #[test]
    fn every_home_city_builds_on_its_site() {
        // home cities each cut by their own water, all of them connected (69's is cut in
        // two by a river and bridged)
        for seed in (0..8u64).chain([69]) {
            let w = Overworld::generate(crate::overworld::WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            let home = w.cities.iter().position(|c| c.home).unwrap() as u8;
            let p = sited(
                &w,
                home,
                crate::GenParams {
                    seed,
                    ..Default::default()
                },
            );
            let built = crate::Generator::run(p);
            assert!(built.is_ok(), "seed {seed}: {:?}", built.err());
        }
    }

    #[test]
    fn a_town_builds_on_a_small_map_of_its_own() {
        // the first town that hasn't grown into its city, over a few worlds
        let found = (0..12u64).find_map(|seed| {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            (0..w.cities.len() as u8).find_map(|i| {
                (0..w.cities[i as usize].towns.len() as u8).find_map(|t| {
                    let p = sited_town(
                        &w,
                        i,
                        t,
                        GenParams {
                            seed,
                            ..Default::default()
                        },
                    )?;
                    Some((w.clone(), i, t, p))
                })
            })
        });
        let (w, i, t, p) = found.expect("a town of its own in twelve worlds");
        let b = w.town_block(i, t).unwrap();
        assert!(b.contains(w.cities[i as usize].towns[t as usize]));
        assert!(
            !b.contains(w.cities[i as usize].centre),
            "not its city's centre"
        );
        assert_eq!(p.site.as_ref().unwrap().town, Some(t));
        let c = Generator::run(p).unwrap();
        assert!(c.size.w >= 80 && c.size.h >= 40, "{:?}", c.size);
        assert!(!c.buildings.is_empty());
    }

    #[test]
    fn every_city_in_a_world_builds_on_its_site() {
        let w = Overworld::generate(WorldParams {
            seed: 8,
            ..Default::default()
        })
        .unwrap();
        for i in 0..w.cities.len() as u8 {
            let p = sited(
                &w,
                i,
                GenParams {
                    seed: 8,
                    ..Default::default()
                },
            );
            let built = Generator::run(p);
            assert!(
                built.is_ok(),
                "{}: {:?}",
                w.cities[i as usize].name,
                built.err()
            );
        }
    }

    #[test]
    fn a_city_site_from_before_reads_without_a_town() {
        let w = Overworld::generate(WorldParams {
            seed: 8,
            ..Default::default()
        })
        .unwrap();
        let site = sited(&w, 0, GenParams::default()).site.unwrap();
        let text = ron::to_string(&site).unwrap();
        let old = text.replace(",town:None", "");
        assert!(!old.contains("town"), "{old}");
        let back: CitySite = ron::from_str(&old).unwrap();
        assert_eq!(back.town, None);
        assert_eq!(back, site);
    }

    #[test]
    #[ignore]
    fn every_home_city_of_400_worlds_builds() {
        // a sweep (release: cargo test -p citygen --lib --release -- --ignored
        // every_home_city_of_400): the generator's own rate is under 1 in 100
        let mut bad = Vec::new();
        for seed in 0..400u64 {
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
            if let Err(e) = Generator::run(p) {
                bad.push((seed, format!("{e}")));
            }
        }
        eprintln!("{} of 400 failed: {bad:?}", bad.len());
        assert!(bad.len() <= 4, "{} of 400 failed: {bad:?}", bad.len());
    }

    #[test]
    #[ignore]
    fn every_settlement_of_30_worlds_builds() {
        // a sweep (release): every city and town of 30 worlds on its site
        let (mut built, mut bad) = (0, Vec::new());
        for seed in 0..30u64 {
            let w = Overworld::generate(WorldParams {
                seed,
                ..Default::default()
            })
            .unwrap();
            let base = GenParams {
                seed,
                ..Default::default()
            };
            for i in 0..w.cities.len() as u8 {
                let mut all = vec![(None, Some(sited(&w, i, base.clone())))];
                for t in 0..w.cities[i as usize].towns.len() as u8 {
                    all.push((Some(t), sited_town(&w, i, t, base.clone())));
                }
                for (t, p) in all {
                    let Some(p) = p else { continue };
                    match Generator::run(p) {
                        Ok(_) => built += 1,
                        Err(e) => bad.push((seed, i, t, format!("{e}"))),
                    }
                }
            }
        }
        eprintln!("{built} built, {} failed: {bad:?}", bad.len());
        assert!(
            bad.len() * 50 <= built,
            "{} failed of {}",
            bad.len(),
            built + bad.len()
        );
    }
}
