//! The walker: the player's `@`, movement, connectors and field of view.

pub mod fov;

use crate::home::HomeRig;
pub use crate::render::Visibility;
use crate::render::map::{MapRef, TileMap};
use citygen::lighting::tuning::{DAY_RADIUS, LIT_RADIUS};
use citygen::rng::hash64;
use citygen::{City, ConnectorKind, Dir, Grid, Place, Pos, Size, StratumKind, Theme, sight};
use std::collections::HashMap;

/// How far you see in the dark, per place. By day on a sunlit stratum the radius
/// grows toward `DAY_RADIUS`; lit tiles past it are seen regardless.
pub const RADIUS_STREET: i32 = 14;
pub const RADIUS_UNDER: i32 = 8;
pub const RADIUS_NET: i32 = 24;
/// Indoors the walls do the limiting: a lit floor is seen across.
pub const RADIUS_INSIDE: i32 = 30;

#[derive(Clone, Debug)]
pub struct Player {
    pub pos: Pos,
    /// The map the player stands on: a stratum, or a floor inside a building.
    pub place: Place,
    /// Where the body waits while jacked in. None when not jacked in.
    pub body: Option<(Place, Pos)>,
    /// What has been seen, per place.
    memory: HashMap<Place, Grid<Visibility>>,
    /// What `vis` returns for a place never seen.
    unseen: Grid<Visibility>,
}

impl Player {
    /// Drops the player on a public connector at street level, outside the corp core.
    pub fn spawn(city: &City, seed: u64) -> Player {
        let street = StratumKind::Street;
        let is_core = |p: Pos| {
            city.tile(street, p)
                .and_then(|t| t.district)
                .and_then(|d| city.district_opt(d))
                .is_some_and(|d| d.theme == Theme::CorpCore)
        };
        let public: Vec<Pos> = city
            .connectors
            .iter()
            .filter(|c| c.public && c.serves.contains(&street) && city.walkable(street, c.pos))
            .map(|c| c.pos)
            .collect();
        let preferred: Vec<Pos> = public.iter().copied().filter(|p| !is_core(*p)).collect();
        let pool = if preferred.is_empty() {
            &public
        } else {
            &preferred
        };
        let pos = if pool.is_empty() {
            city.stratum(street)
                .and_then(|st| {
                    st.tiles
                        .iter()
                        .find(|(_, t)| t.is_walkable())
                        .map(|(p, _)| p)
                })
                .unwrap_or(city.size.center())
        } else {
            pool[(hash64(seed, 0x5A17) % pool.len() as u64) as usize]
        };
        let mut player = Player::at(Place::Stratum(street), pos);
        player.update_fov(&MapRef::stratum_of(city, street), 0);
        player
    }

    /// A player standing at a tile of a place, having seen nothing yet.
    pub fn at(place: Place, pos: Pos) -> Player {
        Player {
            pos,
            place,
            body: None,
            memory: HashMap::new(),
            unseen: Grid::filled(Size { w: 0, h: 0 }, Visibility::Unseen),
        }
    }

    /// The dark radius at night for the current place.
    pub fn radius(&self) -> i32 {
        match self.place {
            Place::Stratum(StratumKind::Undercity) => RADIUS_UNDER,
            Place::Stratum(StratumKind::Net) => RADIUS_NET,
            Place::Stratum(_) => RADIUS_STREET,
            Place::Floor { .. } | Place::Construct(_) => RADIUS_INSIDE,
        }
    }

    /// The dark radius with `daylight` (0..=1) on it: night on strata with no sky.
    pub fn radius_at(&self, daylight: f32) -> i32 {
        let night = self.radius();
        match self.place {
            Place::Stratum(k) if citygen::lighting::sunlit(k) => {
                night + ((DAY_RADIUS - night) as f32 * daylight).round() as i32
            }
            _ => night,
        }
    }

    pub fn jacked_in(&self) -> bool {
        self.body.is_some()
    }

    /// The port of a terminal on this tile or an 8-neighbour that serves this stratum.
    /// Inside, only the rig at home reaches the Net: its port is the landing outside.
    pub fn terminal_in_reach(&self, city: &City, home: Option<&HomeRig>) -> Option<Pos> {
        if let Place::Floor { .. } = self.place {
            let h = home.filter(|h| h.place() == self.place)?;
            return (self.pos.chebyshev(h.home.rig) <= 1).then_some(h.landing);
        }
        let k = self.place.stratum()?;
        std::iter::once(self.pos)
            .chain(self.pos.neighbors8())
            .find_map(|p| {
                city.connectors_at(p)
                    .find(|c| c.kind == ConnectorKind::Jack && c.serves.contains(&k))
                    .map(|c| c.pos)
            })
    }

    /// Jacks in at a Jack connector, or the home rig, on this tile or an 8-neighbour of
    /// it, leaving the body here.
    pub fn jack_in(
        &mut self,
        city: &City,
        home: Option<&HomeRig>,
        sight_bonus: i32,
    ) -> Result<Pos, &'static str> {
        if self.jacked_in() {
            return Err("already jacked in");
        }
        let port = self
            .terminal_in_reach(city, home)
            .ok_or("no terminal within reach")?;
        if !city.walkable(StratumKind::Net, port) {
            return Err("the port is dead");
        }
        self.body = Some((self.place, self.pos));
        self.place = Place::Stratum(StratumKind::Net);
        self.pos = port;
        self.update_fov(&MapRef::stratum_of(city, StratumKind::Net), sight_bonus);
        Ok(port)
    }

    /// Returns to the body. A body left on a stratum sees again at once, at `minute`
    /// of the day; one left on a floor is refreshed by the caller, who holds the floor.
    pub fn jack_out(
        &mut self,
        city: &City,
        sight_bonus: i32,
        minute: u16,
    ) -> Result<(), &'static str> {
        let (place, p) = self.body.take().ok_or("not jacked in")?;
        self.place = place;
        self.pos = p;
        if let Some(k) = place.stratum() {
            self.update_fov(&MapRef::stratum_at(city, k, minute), sight_bonus);
        }
        Ok(())
    }

    /// Steps one tile if the target is walkable.
    pub fn try_move(&mut self, map: &dyn TileMap, dir: Dir) -> bool {
        let target = self.pos.step(dir);
        if map.walkable(target) {
            self.pos = target;
            true
        } else {
            false
        }
    }

    /// Takes the connector under the player one stratum up or down, if it serves that way.
    pub fn use_connector(&mut self, city: &City, up: bool) -> Option<StratumKind> {
        let from = self.place.stratum()?;
        let dest = city
            .connectors_at(self.pos)
            .filter(|c| c.kind != ConnectorKind::Jack)
            .filter_map(|c| c.serves_from(from))
            .find_map(|(below, above)| if up { above } else { below })?;
        if !city.walkable(dest, self.pos) {
            return None;
        }
        self.place = Place::Stratum(dest);
        Some(dest)
    }

    fn memory_for(&mut self, size: Size) -> &mut Grid<Visibility> {
        let grid = self
            .memory
            .entry(self.place)
            .or_insert_with(|| Grid::filled(size, Visibility::Unseen));
        if grid.size() != size {
            *grid = Grid::filled(size, Visibility::Unseen);
        }
        grid
    }

    /// Recomputes what the player sees on the current map, demoting old sight to memory.
    /// `sight_bonus` is the character's equipment bonus to the dark radius. On a
    /// physical stratum the cast reaches `LIT_RADIUS` and keeps lit tiles past the dark
    /// radius; the Net and interiors see the plain radius.
    pub fn update_fov(&mut self, map: &dyn TileMap, sight_bonus: i32) {
        let dark = (self.radius_at(map.daylight()) + sight_bonus).max(1);
        let lit = map.lit_sight();
        let cast = if lit { dark.max(LIT_RADIUS) } else { dark };
        let size = map.size();
        let pos = self.pos;
        let grid = self.memory_for(size);
        for v in grid.as_mut_slice().iter_mut() {
            if *v == Visibility::Visible {
                *v = Visibility::Remembered;
            }
        }
        fov::compute(
            pos,
            cast,
            |p| map.tile(p).is_none_or(|t| t.blocks_sight()),
            |p| size.contains(p),
            |p| {
                if !lit || sight::lit_or_near(pos, p, dark, |q| map.emitted(q), |q| map.emits(q)) {
                    grid.set(p, Visibility::Visible);
                }
            },
        );
    }

    /// Paints every tile within `radius` on the current map into memory, walls or not;
    /// tiles in sight stay visible.
    pub fn reveal(&mut self, map: &dyn TileMap, radius: i32) -> Vec<Pos> {
        let size = map.size();
        let pos = self.pos;
        let grid = self.memory_for(size);
        let mut newly = Vec::new();
        for y in (pos.y - radius)..=(pos.y + radius) {
            for x in (pos.x - radius)..=(pos.x + radius) {
                let p = Pos::new(x, y);
                if size.contains(p) && grid.get(p) == Some(&Visibility::Unseen) {
                    grid.set(p, Visibility::Remembered);
                    newly.push(p);
                }
            }
        }
        newly
    }

    /// Visibility grid for the current place.
    pub fn vis(&self) -> &Grid<Visibility> {
        self.memory.get(&self.place).unwrap_or(&self.unseen)
    }

    /// Sets what is known of one tile of the current place.
    #[cfg(test)]
    pub fn set_vis(&mut self, p: Pos, v: Visibility) {
        if let Some(g) = self.memory.get_mut(&self.place) {
            g.set(p, v);
        }
    }

    /// What has been seen of another place, if anything.
    #[cfg(test)]
    pub fn vis_at(&self, place: Place) -> Option<&Grid<Visibility>> {
        self.memory.get(&place)
    }

    /// Paints tiles of any place into memory, as a plan does; what is in sight stays
    /// visible.
    pub fn remember_place(&mut self, place: Place, size: Size, tiles: impl Iterator<Item = Pos>) {
        let grid = self
            .memory
            .entry(place)
            .or_insert_with(|| Grid::filled(size, Visibility::Unseen));
        if grid.size() != size {
            *grid = Grid::filled(size, Visibility::Unseen);
        }
        for p in tiles {
            if grid.get(p) == Some(&Visibility::Unseen) {
                grid.set(p, Visibility::Remembered);
            }
        }
    }
}

/// The log line for a jack-in from a body at `from`: home, or a named port.
pub fn jacked_in_line(city: &City, from: Place, port: Pos) -> String {
    match from {
        Place::Floor { .. } => "jacked in from home".to_string(),
        _ => format!("jacked in at `{}`", port_name(city, port)),
    }
}

/// What a port is called in the status line: its building, else its district.
pub fn port_name(city: &City, port: Pos) -> String {
    let tile = city.tile(StratumKind::Street, port);
    tile.and_then(|t| t.building)
        .and_then(|b| city.building_opt(b))
        .and_then(|b| b.name.clone())
        .or_else(|| {
            tile.and_then(|t| t.district)
                .and_then(|d| city.district_opt(d))
                .map(|d| d.name.clone())
        })
        .unwrap_or_else(|| "the Net".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{Dir, GenParams, Generator, Pos, SizePreset, StratumKind};

    fn city() -> City {
        Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap()
    }

    fn street(c: &City) -> MapRef<'_> {
        MapRef::stratum_of(c, StratumKind::Street)
    }

    #[test]
    fn player_moves_and_uses_connectors() {
        let c = city();
        let mut p = Player::spawn(&c, 1);
        assert_eq!(p.place, Place::Stratum(StratumKind::Street));
        assert!(c.walkable(StratumKind::Street, p.pos));
        let conn = c
            .connectors
            .iter()
            .find(|k| k.public && k.serves.contains(&StratumKind::Undercity))
            .unwrap();
        p.pos = conn.pos;
        p.place = Place::Stratum(StratumKind::Street);
        assert_eq!(p.use_connector(&c, false), Some(StratumKind::Undercity));
        assert_eq!(p.use_connector(&c, false), None);
        assert_eq!(p.use_connector(&c, true), Some(StratumKind::Street));
        p.update_fov(&street(&c), 0);
        assert_eq!(p.vis()[p.pos], Visibility::Visible);
        assert!(p.vis().count(|v| *v == Visibility::Visible) > 10);
        let moved = Dir::ALL.iter().any(|d| {
            let mut q = p.clone();
            q.try_move(&street(&c), *d)
        });
        assert!(moved);
        let before = p.pos;
        for d in Dir::ALL {
            if p.try_move(&street(&c), d) {
                break;
            }
        }
        assert_ne!(p.pos, before);
        p.update_fov(&street(&c), 0);
        assert!(
            p.vis().count(|v| *v == Visibility::Remembered)
                <= p.vis().count(|v| *v != Visibility::Unseen)
        );
        let mut q = p.clone();
        q.pos = Pos::new(-1, -1);
        assert!(!q.try_move(&street(&c), Dir::N) || c.walkable(StratumKind::Street, q.pos));
        assert_eq!(p.radius(), 14);
        p.place = Place::Stratum(StratumKind::Undercity);
        assert_eq!(p.radius(), 8);
        // A place never seen reads as unseen, and a floor has its own memory.
        p.place = Place::Floor {
            building: citygen::BuildingId(0),
            level: 0,
        };
        assert_eq!(p.radius(), RADIUS_INSIDE);
        assert_eq!(p.vis().count(|v| *v != Visibility::Unseen), 0);
        assert!(
            p.use_connector(&c, true).is_none(),
            "no stairs between strata from a floor"
        );
    }

    #[test]
    fn jacking_in_leaves_the_body_at_the_port() {
        let c = city();
        let jack = c
            .connectors
            .iter()
            .find(|k| k.kind == citygen::ConnectorKind::Jack && k.public)
            .unwrap();
        let stand = jack
            .pos
            .neighbors8()
            .find(|n| c.walkable(StratumKind::Street, *n))
            .unwrap();
        let mut p = Player::spawn(&c, 1);
        p.pos = stand;
        p.place = Place::Stratum(StratumKind::Street);
        assert!(!p.jacked_in());
        assert_eq!(p.jack_in(&c, None, 0), Ok(jack.pos));
        assert!(p.jacked_in());
        assert_eq!(p.place, Place::Stratum(StratumKind::Net));
        assert_eq!(p.pos, jack.pos);
        assert_eq!(p.body, Some((Place::Stratum(StratumKind::Street), stand)));
        assert_eq!(p.radius(), RADIUS_NET);
        assert!(p.vis().count(|v| *v == Visibility::Visible) > 10);
        assert_eq!(p.jack_in(&c, None, 0), Err("already jacked in"));
        assert_eq!(p.use_connector(&c, false), None, "jacks are not stairs");
        assert_eq!(p.use_connector(&c, true), None);
        assert_eq!(p.jack_out(&c, 0, 22 * 60), Ok(()));
        assert_eq!(
            (p.place, p.pos, p.body),
            (Place::Stratum(StratumKind::Street), stand, None)
        );
        assert_eq!(p.jack_out(&c, 0, 22 * 60), Err("not jacked in"));
        // The body left on the street at noon sees the noon street.
        let night = p.vis().count(|v| *v == Visibility::Visible);
        assert!(p.jack_in(&c, None, 0).is_ok());
        assert_eq!(p.jack_out(&c, 0, 12 * 60), Ok(()));
        let day = p.vis().count(|v| *v == Visibility::Visible);
        assert!(day > night, "{day} by day vs {night} at night");
        // Far from any terminal.
        let far = c
            .stratum(StratumKind::Street)
            .unwrap()
            .tiles
            .iter()
            .find(|(q, t)| {
                t.is_walkable()
                    && std::iter::once(*q).chain(q.neighbors8()).all(|n| {
                        c.connectors_at(n)
                            .all(|k| k.kind != citygen::ConnectorKind::Jack)
                    })
            })
            .map(|(q, _)| q)
            .unwrap();
        p.pos = far;
        assert_eq!(p.jack_in(&c, None, 0), Err("no terminal within reach"));
        // A floor that is not home reaches nothing, a street Jack beside it or not.
        p.pos = stand;
        p.place = Place::Floor {
            building: citygen::BuildingId(0),
            level: 0,
        };
        assert_eq!(p.jack_in(&c, None, 0), Err("no terminal within reach"));
    }

    #[test]
    fn the_home_rig_jacks_in_and_the_body_stays_home() {
        let c = Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap();
        let rig = crate::home::HomeRig::find(&c).expect("a home");
        let home = Place::Floor {
            building: rig.home.building,
            level: rig.home.level,
        };
        let beside = rig.home.rig.neighbors8().next().unwrap();
        // Another floor of the same megablock reaches nothing.
        let mut p = Player::at(
            Place::Floor {
                building: rig.home.building,
                level: rig.home.level + 1,
            },
            beside,
        );
        assert_eq!(p.terminal_in_reach(&c, Some(&rig)), None);
        assert_eq!(
            p.jack_in(&c, Some(&rig), 0),
            Err("no terminal within reach")
        );
        // Home, but out of reach of the rig.
        p.place = home;
        p.pos = Pos::new(rig.home.rig.x + 2, rig.home.rig.y);
        assert_eq!(
            p.jack_in(&c, Some(&rig), 0),
            Err("no terminal within reach")
        );
        // Beside it.
        p.pos = beside;
        assert_eq!(p.terminal_in_reach(&c, None), None, "no home, no rig");
        assert_eq!(p.terminal_in_reach(&c, Some(&rig)), Some(rig.landing));
        assert_eq!(p.jack_in(&c, Some(&rig), 0), Ok(rig.landing));
        assert_eq!(p.place, Place::Stratum(StratumKind::Net));
        assert_eq!(p.pos, rig.landing);
        assert_eq!(p.body, Some((home, beside)));
        assert_eq!(p.jack_out(&c, 0, 22 * 60), Ok(()));
        assert_eq!((p.place, p.pos, p.body), (home, beside, None));
    }

    #[test]
    fn sight_bonus_widens_the_view() {
        let c = city();
        let mut p = Player::spawn(&c, 1);
        p.update_fov(&street(&c), 0);
        let base = p.vis().count(|v| *v == Visibility::Visible);
        p.update_fov(&street(&c), 10);
        let wide = p.vis().count(|v| *v == Visibility::Visible);
        assert!(wide > base, "{wide} vs {base}");
    }

    #[test]
    fn a_lamp_pulls_its_pool_into_view_past_the_dark_radius() {
        let c = city();
        let st = c.stratum(StratumKind::Street).unwrap();
        let mut p = Player::spawn(&c, 1);
        p.update_fov(&street(&c), 0);
        let far_lit = p
            .vis()
            .iter()
            .filter(|(q, v)| **v == Visibility::Visible && q.chebyshev(p.pos) > RADIUS_STREET)
            .filter(|(q, _)| st.emitted(*q, 22 * 60) >= citygen::lighting::tuning::LIT_SIGHT)
            .count();
        assert!(
            far_lit > 0,
            "something lit beyond {RADIUS_STREET} tiles is seen"
        );
        let far_dark = p
            .vis()
            .iter()
            .filter(|(q, v)| **v == Visibility::Visible && q.chebyshev(p.pos) > RADIUS_STREET)
            .filter(|(q, _)| st.emitted(*q, 22 * 60) == 0 && !street(&c).emits(*q))
            .count();
        assert_eq!(far_dark, 0, "nothing dark beyond the radius is seen");
    }

    #[test]
    fn noon_on_the_street_sees_to_the_horizon_and_the_net_does_not_care() {
        let c = city();
        let mut p = Player::spawn(&c, 1);
        let noon = MapRef::stratum_at(&c, StratumKind::Street, 12 * 60);
        assert_eq!(
            p.radius_at(noon.daylight()),
            citygen::lighting::tuning::DAY_RADIUS
        );
        p.update_fov(&noon, 0);
        let day = p.vis().count(|v| *v == Visibility::Visible);
        p.update_fov(&street(&c), 0);
        let night = p.vis().count(|v| *v == Visibility::Visible);
        assert!(day > 2 * night, "{day} by day vs {night} at night");
        p.place = Place::Stratum(StratumKind::Net);
        assert_eq!(p.radius_at(1.0), RADIUS_NET);
        p.place = Place::Stratum(StratumKind::Undercity);
        assert_eq!(p.radius_at(1.0), RADIUS_UNDER);
    }

    #[test]
    fn reveal_returns_newly_revealed_then_nothing() {
        let c = city();
        let mut p = Player::spawn(&c, 1);
        p.update_fov(&street(&c), 0);
        let first = p.reveal(&street(&c), 6);
        assert!(!first.is_empty());
        assert!(first.iter().all(|q| q.chebyshev(p.pos) <= 6));
        assert!(
            p.reveal(&street(&c), 6).is_empty(),
            "a second reveal of the same square finds nothing new"
        );
        assert!(p.vis().count(|v| *v == Visibility::Remembered) >= first.len());
    }
}
