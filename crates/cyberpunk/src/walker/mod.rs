//! The walker: the player's `@`, movement, connectors and field of view.

pub mod fov;

pub use crate::render::Visibility;
use citygen::rng::hash64;
use citygen::{City, Dir, Grid, Pos, StratumKind, Theme};

pub const RADIUS_STREET: i32 = 14;
pub const RADIUS_UNDER: i32 = 8;

#[derive(Clone, Debug)]
pub struct Player {
    pub pos: Pos,
    pub stratum: StratumKind,
    /// What has been seen on each physical stratum.
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
            memory: StratumKind::PHYSICAL
                .iter()
                .map(|_| Grid::filled(city.size, Visibility::Unseen))
                .collect(),
        };
        player.update_fov(city);
        player
    }

    pub fn radius(&self) -> i32 {
        if self.stratum == StratumKind::Undercity {
            RADIUS_UNDER
        } else {
            RADIUS_STREET
        }
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
}
