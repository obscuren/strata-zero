//! Stage 7: rooms, doors, windows and internal shafts.

use super::{GenCtx, Stage, StageRunner, StepOut};
use crate::geom::{Pos, Rect};
use crate::params::GenError;
use crate::pathing::{bfs_nearest, components};
use crate::tile::Terrain;
use crate::world::{Building, Connector, ConnectorKind, StratumKind};
use rand::Rng;
use rand::seq::IndexedRandom;
use rand_chacha::ChaCha8Rng;

#[derive(Default)]
pub(crate) struct InteriorsStage {
    next: usize,
    rng: Option<ChaCha8Rng>,
}

fn is_corner(r: Rect, p: Pos) -> bool {
    (p.x == r.x || p.x == r.right() - 1) && (p.y == r.y || p.y == r.bottom() - 1)
}

/// The neighbour of a border tile that lies inside the footprint.
fn inward(r: Rect, p: Pos) -> Pos {
    if p.x == r.x {
        p.offset(1, 0)
    } else if p.x == r.right() - 1 {
        p.offset(-1, 0)
    } else if p.y == r.y {
        p.offset(0, 1)
    } else {
        p.offset(0, -1)
    }
}

fn outward(r: Rect, p: Pos) -> Pos {
    let i = inward(r, p);
    Pos::new(2 * p.x - i.x, 2 * p.y - i.y)
}

impl InteriorsStage {
    /// Recursive partitions: a wall line with one door, on floor tiles only.
    fn partition(ctx: &mut GenCtx, rng: &mut ChaCha8Rng, k: StratumKind, inner: Rect, depth: u8) {
        let can_v = inner.w >= 7;
        let can_h = inner.h >= 7;
        if depth == 0 || (!can_v && !can_h) {
            return;
        }
        let vertical = if can_v && can_h {
            inner.w >= inner.h
        } else {
            can_v
        };
        if vertical {
            let s = rng.random_range(inner.x + 3..=inner.right() - 4);
            for y in inner.y..inner.bottom() {
                let p = Pos::new(s, y);
                if ctx.terrain(k, p) == Terrain::Floor {
                    ctx.set_terrain(k, p, Terrain::Wall);
                }
            }
            let door = Pos::new(s, rng.random_range(inner.y..inner.bottom()));
            if ctx.terrain(k, door) == Terrain::Wall {
                ctx.set_terrain(k, door, Terrain::Door);
            }
            Self::partition(
                ctx,
                rng,
                k,
                Rect::new(inner.x, inner.y, s - inner.x, inner.h),
                depth - 1,
            );
            Self::partition(
                ctx,
                rng,
                k,
                Rect::new(s + 1, inner.y, inner.right() - s - 1, inner.h),
                depth - 1,
            );
        } else {
            let s = rng.random_range(inner.y + 3..=inner.bottom() - 4);
            for x in inner.x..inner.right() {
                let p = Pos::new(x, s);
                if ctx.terrain(k, p) == Terrain::Floor {
                    ctx.set_terrain(k, p, Terrain::Wall);
                }
            }
            let door = Pos::new(rng.random_range(inner.x..inner.right()), s);
            if ctx.terrain(k, door) == Terrain::Wall {
                ctx.set_terrain(k, door, Terrain::Door);
            }
            Self::partition(
                ctx,
                rng,
                k,
                Rect::new(inner.x, inner.y, inner.w, s - inner.y),
                depth - 1,
            );
            Self::partition(
                ctx,
                rng,
                k,
                Rect::new(inner.x, s + 1, inner.w, inner.bottom() - s - 1),
                depth - 1,
            );
        }
    }

    /// Guarantees every walkable tile inside the footprint reaches every other by carving
    /// doors through inner walls (never the perimeter).
    fn ensure_connected(ctx: &mut GenCtx, k: StratumKind, fp: Rect) {
        let size = ctx.size();
        let inner = fp.inset(1);
        for _ in 0..8 {
            let comps = components(size, |p| fp.contains(p) && ctx.city.walkable(k, p));
            if comps.len() <= 1 {
                return;
            }
            let main = &comps[0];
            let mut in_main = vec![false; size.area()];
            for p in main {
                in_main[size.index(*p)] = true;
            }
            let start = comps[1][0];
            let path = bfs_nearest(
                size,
                start,
                |p| {
                    inner.contains(p)
                        && (ctx.city.walkable(k, p) || ctx.terrain(k, p) == Terrain::Wall)
                },
                |p| in_main[size.index(p)],
                16,
            );
            match path {
                Some(path) => {
                    for p in path {
                        if ctx.terrain(k, p) == Terrain::Wall && inner.contains(p) {
                            ctx.set_terrain(k, p, Terrain::Door);
                        }
                    }
                }
                None => return,
            }
        }
    }

    /// Opens a door on a border wall facing `outside_ok` terrain; returns the lobby tile.
    fn exterior_door(
        ctx: &mut GenCtx,
        rng: &mut ChaCha8Rng,
        k: StratumKind,
        fp: Rect,
        outside_ok: impl Fn(Terrain) -> bool,
        prefer: Option<Terrain>,
    ) -> Option<Pos> {
        let candidates: Vec<Pos> = fp
            .border()
            .filter(|p| !is_corner(fp, *p) && ctx.terrain(k, *p) == Terrain::Wall)
            .filter(|p| {
                let o = outward(fp, *p);
                ctx.city.size.contains(o) && outside_ok(ctx.terrain(k, o))
            })
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let preferred: Vec<Pos> = match prefer {
            Some(t) => candidates
                .iter()
                .copied()
                .filter(|p| ctx.terrain(k, outward(fp, *p)) == t)
                .collect(),
            None => Vec::new(),
        };
        let pool = if preferred.is_empty() {
            &candidates
        } else {
            &preferred
        };
        let door = *pool.choose(rng).expect("non-empty pool");
        ctx.set_terrain(k, door, Terrain::Door);
        let lobby = inward(fp, door);
        if !ctx.city.walkable(k, lobby) {
            ctx.set_terrain(k, lobby, Terrain::Floor);
        }
        Some(lobby)
    }

    fn windows(ctx: &mut GenCtx, fp: Rect) {
        let k = StratumKind::Street;
        let spots: Vec<Pos> = fp
            .border()
            .filter(|p| !is_corner(fp, *p) && ctx.terrain(k, *p) == Terrain::Wall)
            .filter(|p| (p.x + p.y) % 2 == 0)
            .filter(|p| {
                let o = outward(fp, *p);
                ctx.tile(k, o).is_some_and(|t| t.is_outdoor_ground())
            })
            .collect();
        for p in spots {
            ctx.set_terrain(k, p, Terrain::Window);
        }
    }

    /// Places the internal stair or lift and registers its connector.
    fn shaft(ctx: &mut GenCtx, b: &Building) {
        let fp = b.footprint;
        let inner = fp.inset(1);
        let strata: Vec<StratumKind> = b.span.iter().collect();
        let open_everywhere = |ctx: &GenCtx, p: Pos| {
            strata.iter().all(|k| {
                let t = ctx.terrain(*k, p);
                t != Terrain::Wall && t != Terrain::Void && ctx.city.walkable(*k, p)
            })
        };
        let centre = fp.center();
        let mut best = inner
            .iter()
            .filter(|p| open_everywhere(ctx, *p))
            .min_by_key(|p| p.dist2(centre));
        if best.is_none() {
            let c = if inner.is_empty() {
                centre
            } else {
                inner.center()
            };
            for k in &strata {
                if *k != StratumKind::Heights {
                    ctx.set_terrain(*k, c, Terrain::Floor);
                    for n in c.neighbors4() {
                        if inner.contains(n) && ctx.terrain(*k, n) == Terrain::Wall {
                            ctx.set_terrain(*k, n, Terrain::Floor);
                        }
                    }
                }
            }
            best = Some(c);
        }
        let pos = best.expect("set above");
        let kind = if strata.len() == 2 {
            ConnectorKind::Stair
        } else {
            ConnectorKind::Lift
        };
        let terrain = match kind {
            ConnectorKind::Stair => Terrain::Stair,
            _ => Terrain::Lift,
        };
        for k in &strata {
            ctx.set_terrain(*k, pos, terrain);
            ctx.set_feature(*k, pos, None);
        }
        ctx.city.connectors.push(Connector {
            kind,
            pos,
            serves: strata,
            public: false,
        });
    }

    fn furnish(&mut self, ctx: &mut GenCtx, i: usize) {
        let b = ctx.city.buildings[i].clone();
        let rng = self.rng.as_mut().expect("begin sets rng");
        let fp = b.footprint;
        for k in b.span.iter() {
            if k == StratumKind::Heights {
                continue;
            }
            Self::partition(ctx, rng, k, fp.inset(1), 4);
            Self::ensure_connected(ctx, k, fp);
            match k {
                StratumKind::Street => {
                    let lobby = Self::exterior_door(
                        ctx,
                        rng,
                        k,
                        fp,
                        |t| {
                            matches!(
                                t,
                                Terrain::Road | Terrain::Sidewalk | Terrain::Alley | Terrain::Plaza
                            )
                        },
                        Some(Terrain::Sidewalk),
                    );
                    if let Some(l) = lobby {
                        ctx.scratch.lobbies.push((b.id, l));
                    }
                    Self::windows(ctx, fp);
                }
                StratumKind::Skyways => {
                    Self::exterior_door(
                        ctx,
                        rng,
                        k,
                        fp,
                        |t| matches!(t, Terrain::Walkway | Terrain::Bridge),
                        None,
                    );
                }
                StratumKind::Undercity => {
                    Self::exterior_door(
                        ctx,
                        rng,
                        k,
                        fp,
                        |t| matches!(t, Terrain::Tunnel | Terrain::Rail | Terrain::Grate),
                        None,
                    );
                }
                _ => {}
            }
        }
        if b.span.len() >= 2 {
            Self::shaft(ctx, &b);
        }
    }
}

impl StageRunner for InteriorsStage {
    fn begin(&mut self, ctx: &mut GenCtx) -> Result<(), GenError> {
        self.rng = Some(ctx.rng(Stage::Interiors));
        self.next = 0;
        ctx.scratch.lobbies.clear();
        Ok(())
    }

    fn step(&mut self, ctx: &mut GenCtx) -> Result<StepOut, GenError> {
        let n = ctx.city.buildings.len();
        if self.next >= n {
            return Ok(StepOut::Finished);
        }
        self.furnish(ctx, self.next);
        self.next += 1;
        Ok(StepOut::Continue(self.next as f32 / n as f32))
    }
}

#[cfg(test)]
mod tests {
    use crate::generate::testutil::small;
    use crate::params::WaterMode;
    use crate::pathing::components;
    use crate::tile::Terrain;
    use crate::world::StratumKind;

    #[test]
    fn every_building_has_a_street_door_and_internal_connectivity() {
        let c = small(61, WaterMode::Bay, 7);
        let st = c.stratum(StratumKind::Street).unwrap();
        for b in &c.buildings {
            let doors: Vec<_> = b
                .footprint
                .border()
                .filter(|p| st.tiles[*p].terrain == Terrain::Door)
                .collect();
            assert!(!doors.is_empty(), "{:?} has no door", b.id);
            assert!(doors.iter().any(|d| {
                d.neighbors4()
                    .any(|n| st.tiles.get(n).is_some_and(|t| t.is_outdoor_ground()))
            }));
            let inside = b.footprint;
            let comps = components(st.tiles.size(), |p| {
                inside.contains(p) && st.tiles[p].is_walkable()
            });
            assert_eq!(comps.len(), 1, "{:?} interior split", b.id);
            if b.span.len() >= 2 {
                let conn = c
                    .connectors
                    .iter()
                    .find(|k| inside.contains(k.pos) && !k.public)
                    .expect("internal connector");
                for s in b.span.iter() {
                    assert!(conn.serves.contains(&s));
                    assert!(c.walkable(s, conn.pos), "{:?} {s:?} {:?}", b.id, conn.pos);
                }
            }
        }
    }

    #[test]
    fn rooms_respect_minimum_size_and_big_buildings_get_walls() {
        let c = small(61, WaterMode::None, 6);
        let st = c.stratum(StratumKind::Street).unwrap();
        let big = c
            .buildings
            .iter()
            .find(|b| b.footprint.w >= 12 && b.footprint.h >= 9)
            .expect("a big building");
        let walls_inside = big
            .footprint
            .inset(1)
            .iter()
            .filter(|p| st.tiles[*p].terrain == Terrain::Wall)
            .count();
        assert!(walls_inside > 0, "big building should have internal walls");
        assert!(st.tiles.count(|t| t.terrain == Terrain::Window) > 20);
    }
}
