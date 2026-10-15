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

    /// Opens a door on a border wall facing `outside_ok` terrain; returns the door and
    /// the lobby tile inward of it, carved to Floor when `carve_lobby`.
    fn exterior_door(
        ctx: &mut GenCtx,
        rng: &mut ChaCha8Rng,
        k: StratumKind,
        fp: Rect,
        outside_ok: impl Fn(Terrain) -> bool,
        prefer: Option<Terrain>,
        carve_lobby: bool,
    ) -> Option<(Pos, Pos)> {
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
        if carve_lobby && !ctx.city.walkable(k, lobby) {
            ctx.set_terrain(k, lobby, Terrain::Floor);
        }
        Some((door, lobby))
    }

    /// Which edge of the footprint a border tile sits on.
    fn edge(fp: Rect, p: Pos) -> u8 {
        if p.x == fp.x {
            0
        } else if p.x == fp.right() - 1 {
            1
        } else if p.y == fp.y {
            2
        } else {
            3
        }
    }

    /// A second street door on another side of a closed building, where that side
    /// faces outdoor ground.
    fn back_door(ctx: &mut GenCtx, rng: &mut ChaCha8Rng, fp: Rect, main: Pos) {
        let k = StratumKind::Street;
        let candidates: Vec<Pos> = fp
            .border()
            .filter(|p| !is_corner(fp, *p) && ctx.terrain(k, *p) == Terrain::Wall)
            .filter(|p| Self::edge(fp, *p) != Self::edge(fp, main))
            .filter(|p| {
                let o = outward(fp, *p);
                ctx.tile(k, o).is_some_and(|t| t.is_outdoor_ground())
            })
            .collect();
        if let Some(door) = candidates.choose(rng) {
            ctx.set_terrain(k, *door, Terrain::Door);
        }
    }

    /// The roof door of a tall closed building: the stair-house above the main
    /// street door, or the roof tile nearest the centre.
    fn roof_door(ctx: &mut GenCtx, fp: Rect, main: Option<Pos>) {
        let k = StratumKind::Heights;
        let above = main.filter(|p| ctx.terrain(k, *p) == Terrain::Rooftop);
        let spot = above.or_else(|| {
            let centre = fp.center();
            fp.iter()
                .filter(|p| ctx.terrain(k, *p) == Terrain::Rooftop)
                .min_by_key(|p| p.dist2(centre))
        });
        if let Some(p) = spot {
            ctx.set_terrain(k, p, Terrain::Door);
            ctx.set_feature(k, p, None);
        }
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

    /// A closed building: mass behind the facade on every stratum but the roof, a
    /// door on each stratum that has something to open onto, a back door where
    /// another side faces the street, and the roof door of a tall one.
    fn close(ctx: &mut GenCtx, rng: &mut ChaCha8Rng, b: &Building) {
        let fp = b.footprint;
        let street_ok = |t: Terrain| {
            matches!(
                t,
                Terrain::Road | Terrain::Sidewalk | Terrain::Alley | Terrain::Plaza
            )
        };
        let mut main = None;
        for k in b.span.iter() {
            if k == StratumKind::Heights {
                continue;
            }
            for p in fp.inset(1).iter() {
                ctx.set_terrain(k, p, Terrain::Mass);
                ctx.set_feature(k, p, None);
            }
            match k {
                StratumKind::Street => {
                    main = Self::exterior_door(
                        ctx,
                        rng,
                        k,
                        fp,
                        street_ok,
                        Some(Terrain::Sidewalk),
                        false,
                    )
                    .map(|(door, _)| door);
                    if let Some(door) = main {
                        Self::back_door(ctx, rng, fp, door);
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
                        false,
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
                        false,
                    );
                }
                _ => {}
            }
        }
        if b.span.contains(StratumKind::Heights) {
            Self::roof_door(ctx, fp, main);
        }
    }

    fn furnish(&mut self, ctx: &mut GenCtx, i: usize) {
        let b = ctx.city.buildings[i].clone();
        let rng = self.rng.as_mut().expect("begin sets rng");
        if b.kind.is_closed() {
            Self::close(ctx, rng, &b);
            return;
        }
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
                        true,
                    );
                    if let Some((_, l)) = lobby {
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
                        true,
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
                        true,
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
    use crate::world::{BuildingKind, StratumKind};
    use std::collections::HashSet;

    fn side(b: &crate::world::Building, p: crate::geom::Pos) -> u8 {
        if p.x == b.footprint.x {
            0
        } else if p.x == b.footprint.right() - 1 {
            1
        } else if p.y == b.footprint.y {
            2
        } else {
            3
        }
    }

    #[test]
    fn closed_buildings_are_mass_behind_their_facade_and_open_ones_keep_rooms() {
        let c = small(61, WaterMode::Bay, 7);
        let st = c.stratum(StratumKind::Street).unwrap();
        let mut closed = 0;
        for b in &c.buildings {
            let doors: Vec<_> = b
                .footprint
                .border()
                .filter(|p| st.tiles[*p].terrain == Terrain::Door)
                .collect();
            assert!(!doors.is_empty(), "{:?} has no door", b.id);
            if b.kind.is_closed() {
                closed += 1;
                for p in b.footprint.inset(1).iter() {
                    assert_eq!(st.tiles[p].terrain, Terrain::Mass, "{:?} {p:?}", b.id);
                    assert_eq!(st.tiles[p].building, Some(b.id));
                }
                for k in b
                    .span
                    .iter()
                    .filter(|k| *k != StratumKind::Street && *k != StratumKind::Heights)
                {
                    let s = c.stratum(k).unwrap();
                    assert!(
                        b.footprint
                            .inset(1)
                            .iter()
                            .all(|p| s.tiles[p].terrain == Terrain::Mass),
                        "{:?} on {k:?}",
                        b.id
                    );
                }
                if b.span.contains(StratumKind::Heights) {
                    let h = c.stratum(StratumKind::Heights).unwrap();
                    let roof_doors = b
                        .footprint
                        .iter()
                        .filter(|p| h.tiles[*p].terrain == Terrain::Door)
                        .count();
                    assert_eq!(roof_doors, 1, "{:?} roof door", b.id);
                    assert!(
                        b.footprint
                            .iter()
                            .any(|p| h.tiles[p].terrain == Terrain::Rooftop)
                    );
                }
                assert!(
                    !c.connectors
                        .iter()
                        .any(|k| b.footprint.contains(k.pos) && !k.public),
                    "{:?} keeps no shaft",
                    b.id
                );
            } else {
                let comps = components(st.tiles.size(), |p| {
                    b.footprint.contains(p) && st.tiles[p].is_walkable()
                });
                assert_eq!(comps.len(), 1, "{:?} interior split", b.id);
            }
        }
        assert!(closed > 0);
    }

    #[test]
    fn closed_buildings_get_a_back_door_where_another_side_faces_the_street() {
        let c = small(61, WaterMode::None, 6);
        let st = c.stratum(StratumKind::Street).unwrap();
        let mut two_sided = 0;
        for b in c.buildings.iter().filter(|b| b.kind.is_closed()) {
            let sides: HashSet<u8> = b
                .footprint
                .border()
                .filter(|p| st.tiles[*p].terrain == Terrain::Door)
                .map(|p| side(b, p))
                .collect();
            let outdoor_sides: HashSet<u8> = b
                .footprint
                .border()
                .filter(|p| !super::is_corner(b.footprint, *p))
                .filter(|p| {
                    st.tiles
                        .get(super::outward(b.footprint, *p))
                        .is_some_and(|t| t.is_outdoor_ground())
                })
                .map(|p| side(b, p))
                .collect();
            if outdoor_sides.len() >= 2 {
                assert!(
                    sides.len() >= 2,
                    "{:?} has {} outdoor sides but doors on {:?}",
                    b.id,
                    outdoor_sides.len(),
                    sides
                );
                two_sided += 1;
            }
        }
        assert!(two_sided > 0);
    }

    #[test]
    fn bridge_and_basement_doors_face_what_they_open_onto() {
        let c = small(61, WaterMode::Bay, 7);
        for b in c.buildings.iter().filter(|b| b.kind.is_closed()) {
            for (k, ok) in [
                (
                    StratumKind::Skyways,
                    &[Terrain::Walkway, Terrain::Bridge][..],
                ),
                (
                    StratumKind::Undercity,
                    &[Terrain::Tunnel, Terrain::Rail, Terrain::Grate][..],
                ),
            ] {
                if !b.span.contains(k) {
                    continue;
                }
                let s = c.stratum(k).unwrap();
                for p in b
                    .footprint
                    .border()
                    .filter(|p| s.tiles[*p].terrain == Terrain::Door)
                {
                    let o = super::outward(b.footprint, p);
                    // A public connector may have landed on the tile outside since.
                    assert!(
                        s.tiles
                            .get(o)
                            .is_some_and(|t| ok.contains(&t.terrain) || t.is_connector()),
                        "{:?} {k:?} door at {p:?} faces {:?}",
                        b.id,
                        s.tiles.get(o).map(|t| t.terrain)
                    );
                }
            }
        }
        assert!(
            c.buildings
                .iter()
                .any(|b| b.kind == BuildingKind::CorpTower && b.kind.is_closed())
        );
    }

    #[test]
    fn portals_list_every_door_of_a_closed_building() {
        let c = small(61, WaterMode::Bay, 7);
        let tall = c
            .buildings
            .iter()
            .find(|b| b.kind.is_closed() && b.span.contains(StratumKind::Heights))
            .expect("a tall closed building");
        let portals = c.portals(tall.id);
        assert!(portals.iter().any(|(k, _)| *k == StratumKind::Street));
        assert!(portals.iter().any(|(k, _)| *k == StratumKind::Heights));
        for (k, p) in &portals {
            assert_eq!(c.tile(*k, *p).unwrap().terrain, Terrain::Door);
            assert!(c.walkable(*k, *p));
        }
        let open = c.buildings.iter().find(|b| !b.kind.is_closed()).unwrap();
        assert!(c.portals(open.id).is_empty());
    }
}
