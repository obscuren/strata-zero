//! The walker: the player's `@`, movement, connectors and field of view.

pub mod fov;

pub use crate::render::Visibility;
use citygen::rng::hash64;
use citygen::{City, ConnectorKind, Dir, Grid, Pos, StratumKind, Theme};

pub const RADIUS_STREET: i32 = 14;
pub const RADIUS_UNDER: i32 = 8;
pub const RADIUS_NET: i32 = 24;

#[derive(Clone, Debug)]
pub struct Player {
    pub pos: Pos,
    pub stratum: StratumKind,
    /// Where the body waits while jacked in: (stratum, tile). None when not jacked in.
    #[allow(dead_code)] // read by the map screen from task 9
    pub body: Option<(StratumKind, Pos)>,
    /// What has been seen on each stratum, indexed by `StratumKind::index`.
    memory: Vec<Grid<Visibility>>,
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
        let mut player = Player {
            pos,
            stratum: street,
            body: None,
            memory: StratumKind::ALL
                .iter()
                .map(|_| Grid::filled(city.size, Visibility::Unseen))
                .collect(),
        };
        player.update_fov(city);
        player
    }

    pub fn radius(&self) -> i32 {
        match self.stratum {
            StratumKind::Undercity => RADIUS_UNDER,
            StratumKind::Net => RADIUS_NET,
            _ => RADIUS_STREET,
        }
    }

    #[allow(dead_code)] // wired by the map screen from task 9
    pub fn jacked_in(&self) -> bool {
        self.body.is_some()
    }

    /// Jacks in at a Jack connector on this tile or an 8-neighbour of it, leaving the body here.
    #[allow(dead_code)] // wired by the map screen from task 9
    pub fn jack_in(&mut self, city: &City) -> Result<Pos, &'static str> {
        if self.jacked_in() {
            return Err("already jacked in");
        }
        let here = self.pos;
        let k = self.stratum;
        let port = std::iter::once(here)
            .chain(here.neighbors8())
            .find_map(|p| {
                city.connectors_at(p)
                    .find(|c| c.kind == ConnectorKind::Jack && c.serves.contains(&k))
                    .map(|c| c.pos)
            })
            .ok_or("no terminal within reach")?;
        if !city.walkable(StratumKind::Net, port) {
            return Err("the port is dead");
        }
        self.body = Some((k, here));
        self.stratum = StratumKind::Net;
        self.pos = port;
        self.update_fov(city);
        Ok(port)
    }

    /// Returns to the body.
    #[allow(dead_code)] // wired by the map screen from task 9
    pub fn jack_out(&mut self, city: &City) -> Result<(), &'static str> {
        let (k, p) = self.body.take().ok_or("not jacked in")?;
        self.stratum = k;
        self.pos = p;
        self.update_fov(city);
        Ok(())
    }

    /// Steps one tile if the target is walkable.
    pub fn try_move(&mut self, city: &City, dir: Dir) -> bool {
        let target = self.pos.step(dir);
        if city.walkable(self.stratum, target) {
            self.pos = target;
            true
        } else {
            false
        }
    }

    /// Takes the connector under the player one stratum up or down, if it serves that way.
    pub fn use_connector(&mut self, city: &City, up: bool) -> Option<StratumKind> {
        let dest = city
            .connectors_at(self.pos)
            .filter(|c| c.kind != ConnectorKind::Jack)
            .filter_map(|c| c.serves_from(self.stratum))
            .find_map(|(below, above)| if up { above } else { below })?;
        if !city.walkable(dest, self.pos) {
            return None;
        }
        self.stratum = dest;
        Some(dest)
    }

    /// Recomputes what the player sees on the current stratum, demoting old sight to memory.
    pub fn update_fov(&mut self, city: &City) {
        let radius = self.radius();
        let Some(st) = city.stratum(self.stratum) else {
            return;
        };
        let grid = &mut self.memory[self.stratum.index()];
        for v in grid.as_mut_slice().iter_mut() {
            if *v == Visibility::Visible {
                *v = Visibility::Remembered;
            }
        }
        let size = city.size;
        fov::compute(
            self.pos,
            radius,
            |p| st.tiles.get(p).is_none_or(|t| t.blocks_sight()),
            |p| size.contains(p),
            |p| {
                grid.set(p, Visibility::Visible);
            },
        );
    }

    /// Visibility grid for the current stratum.
    pub fn vis(&self) -> &Grid<Visibility> {
        &self.memory[self.stratum.index()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{Dir, GenParams, Generator, Pos, SizePreset, StratumKind};

    #[test]
    fn player_moves_and_uses_connectors() {
        let c = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        let mut p = Player::spawn(&c, 1);
        assert_eq!(p.stratum, StratumKind::Street);
        assert!(c.walkable(p.stratum, p.pos));
        let conn = c
            .connectors
            .iter()
            .find(|k| k.public && k.serves.contains(&StratumKind::Undercity))
            .unwrap();
        p.pos = conn.pos;
        p.stratum = StratumKind::Street;
        assert_eq!(p.use_connector(&c, false), Some(StratumKind::Undercity));
        assert_eq!(p.use_connector(&c, false), None);
        assert_eq!(p.use_connector(&c, true), Some(StratumKind::Street));
        p.update_fov(&c);
        assert_eq!(p.vis()[p.pos], Visibility::Visible);
        assert!(p.vis().count(|v| *v == Visibility::Visible) > 10);
        let moved = Dir::ALL.iter().any(|d| {
            let mut q = p.clone();
            q.try_move(&c, *d)
        });
        assert!(moved);
        let before = p.pos;
        for d in Dir::ALL {
            if p.try_move(&c, d) {
                break;
            }
        }
        assert_ne!(p.pos, before);
        p.update_fov(&c);
        assert!(
            p.vis().count(|v| *v == Visibility::Remembered)
                <= p.vis().count(|v| *v != Visibility::Unseen)
        );
        let mut q = p.clone();
        q.pos = Pos::new(-1, -1);
        assert!(!q.try_move(&c, Dir::N) || c.walkable(q.stratum, q.pos));
        assert_eq!(p.radius(), 14);
        p.stratum = StratumKind::Undercity;
        assert_eq!(p.radius(), 8);
    }

    #[test]
    fn jacking_in_leaves_the_body_at_the_port() {
        let c = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
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
        p.stratum = StratumKind::Street;
        assert!(!p.jacked_in());
        assert_eq!(p.jack_in(&c), Ok(jack.pos));
        assert!(p.jacked_in());
        assert_eq!(p.stratum, StratumKind::Net);
        assert_eq!(p.pos, jack.pos);
        assert_eq!(p.body, Some((StratumKind::Street, stand)));
        assert_eq!(p.radius(), RADIUS_NET);
        assert!(p.vis().count(|v| *v == Visibility::Visible) > 10);
        assert_eq!(p.jack_in(&c), Err("already jacked in"));
        assert_eq!(p.use_connector(&c, false), None, "jacks are not stairs");
        assert_eq!(p.use_connector(&c, true), None);
        assert_eq!(p.jack_out(&c), Ok(()));
        assert_eq!(
            (p.stratum, p.pos, p.body),
            (StratumKind::Street, stand, None)
        );
        assert_eq!(p.jack_out(&c), Err("not jacked in"));
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
        assert_eq!(p.jack_in(&c), Err("no terminal within reach"));
    }
}
