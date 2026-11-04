//! A city grown from its place in the world: its water, outline and highways from the
//! world, the open ground past its outline. See `2026-11-04-cities-in-the-world-design.md`.

use crate::geom::{Grid, Pos, Size};
use crate::overworld::field::Fbm;
use crate::overworld::{Overworld, detail};
use crate::params::{CitySite, GenParams, SizePreset};
use crate::tile::{Terrain, Tile};

/// How dense the city must be at a cell for it to build there: its outline.
pub const OUTLINE: f32 = 0.3;

/// The world's part in a city being generated: its block's ground and how dense the
/// city is at each cell.
pub(crate) struct Site {
    pub ground: Grid<Tile>,
    pub density: Grid<f32>,
}

/// `base` set to build world `world`'s city `city` on its block: its size the block's
/// in detail cells, its site the world and the block's corner.
pub fn sited(world: &Overworld, city: u8, base: GenParams) -> GenParams {
    let b = world.city_block(city);
    let t = detail::TILE;
    GenParams {
        size: SizePreset::Custom {
            w: (t * b.w) as u32,
            h: (t * b.h) as u32,
        },
        site: Some(CitySite {
            world: world.params.clone(),
            city,
            origin: Pos::new(t * b.x, t * b.y),
        }),
        ..base
    }
}

/// The site's ground and density, built from its world.
pub(crate) fn prepare(site: &CitySite, size: Size) -> Option<Site> {
    let world = Overworld::generate(site.world.clone()).ok()?;
    let o = site.origin;
    let ground = detail::region(&world, o.x, o.y, size.w, size.h);
    let edge = Fbm::new(world.seed, 4101);
    let mut density = Grid::filled(size, 0.0f32);
    for y in 0..size.h as i32 {
        for x in 0..size.w as i32 {
            let (wx, wy) = ((o.x + x) as f32, (o.y + y) as f32);
            let d = detail::blend(&world, wx, wy, |t| {
                t.urban
                    .filter(|u| u.city == site.city)
                    .map_or(0.0, |u| u.density as f32 / 255.0)
            });
            // the outline runs through tiles, not along them
            density[Pos::new(x, y)] = d + 0.12 * detail::wave(&edge, wx / 7.0, wy / 3.5);
        }
    }
    Some(Site { ground, density })
}

impl Site {
    /// Whether the city builds at `p`: dry, and inside its outline.
    pub fn builds(&self, p: Pos) -> bool {
        !self.wet(p) && self.density.get(p).is_some_and(|d| *d >= OUTLINE)
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
        let near = self.density.get(p).is_some_and(|d| *d >= OUTLINE - 0.15);
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
}
