//! Stage 9: features, light maps and per-tile variants.

use super::{GenCtx, Stage, StageRunner, StepOut, exterior};
use crate::generate::connectivity::reachable_from_street;
use crate::geom::{Pos, Rect};
use crate::params::GenError;
use crate::pathing::components;
use crate::rng::{chance, tile_hash};
use crate::tile::{Feature, Terrain, Tile};
use crate::world::{BuildingKind, StratumKind, Theme};
use rand::seq::IndexedRandom;
use rand_chacha::ChaCha8Rng;
use std::collections::VecDeque;

enum Work {
    Facade(usize),
    Streets(usize),
    Exterior(usize),
    Verify,
    Lights(StratumKind),
    Variants(StratumKind),
}

#[derive(Default)]
pub(crate) struct DressingStage {
    queue: VecDeque<Work>,
    total: usize,
    rng: Option<ChaCha8Rng>,
}

/// Base light per stratum before any source is added.
fn base_light(k: StratumKind) -> u8 {
    match k {
        StratumKind::Undercity => 20,
        StratumKind::Street => 60,
        StratumKind::Skyways => 80,
        StratumKind::Heights => 110,
        StratumKind::Net => 0,
    }
}

/// Radius and strength of a light-emitting tile.
fn emission(terrain: Terrain, feature: Option<Feature>, k: StratumKind) -> Option<(i32, f32)> {
    match feature {
        Some(Feature::Lamp) => Some((6, 120.0)),
        Some(Feature::Neon(_)) => Some((4, 90.0)),
        Some(Feature::HoloAd) => Some((5, 100.0)),
        Some(Feature::Helipad) => Some((3, 60.0)),
        _ if terrain == Terrain::Window && k == StratumKind::Street => Some((2, 30.0)),
        _ => None,
    }
}

/// True when the tile may take a blocking feature without pinching a corridor.
pub(crate) fn open_area(ctx: &GenCtx, k: StratumKind, p: Pos) -> bool {
    let t = ctx.tile(k, p);
    let Some(t) = t else { return false };
    if !t.is_walkable() || t.is_connector() || t.terrain == Terrain::Door {
        return false;
    }
    if ctx.city.connectors_at(p).next().is_some() {
        return false;
    }
    p.neighbors8().filter(|n| ctx.city.walkable(k, *n)).count() >= 6
}

/// A feature may go here if the tile is bare and not a door or connector.
pub(crate) fn bare(ctx: &GenCtx, k: StratumKind, p: Pos) -> bool {
    ctx.tile(k, p).is_some_and(|t| {
        t.feature.is_none()
            && !t.is_connector()
            && t.terrain != Terrain::Door
            && ctx.city.connectors_at(p).next().is_none()
    })
}

impl DressingStage {
    fn facade(&mut self, ctx: &mut GenCtx, i: usize) {
        let b = ctx.city.buildings[i].clone();
        let district = ctx.city.district(b.district).clone();
        let spec = district.theme.spec();
        let rng = self.rng.as_mut().expect("begin sets rng");
        let fp = b.footprint;
        let k = StratumKind::Street;

        let facing_street: Vec<Pos> = fp
            .border()
            .filter(|p| ctx.terrain(k, *p) == Terrain::Wall)
            .filter(|p| {
                p.neighbors4().any(|n| {
                    !fp.contains(n) && ctx.tile(k, n).is_some_and(|t| t.is_outdoor_ground())
                })
            })
            .collect();
        for p in &facing_street {
            let feature = if chance(rng, spec.neon * (0.5 + district.wealth)) {
                Some(Feature::Neon(
                    *spec.hues.choose(rng).expect("hues non-empty"),
                ))
            } else if chance(rng, spec.holo) {
                Some(Feature::HoloAd)
            } else if chance(rng, district.wealth * 0.3) {
                Some(Feature::Camera)
            } else {
                None
            };
            if feature.is_some() {
                ctx.set_feature(k, *p, feature);
            }
        }

        // Shop sign beside the street door.
        if !b.kind.is_corp() {
            let door = fp.border().find(|p| {
                ctx.terrain(k, *p) == Terrain::Door
                    && p.neighbors4().any(|n| {
                        !fp.contains(n) && ctx.tile(k, n).is_some_and(|t| t.is_outdoor_ground())
                    })
            });
            if let Some(door) = door {
                let beside = door
                    .neighbors4()
                    .find(|n| fp.border().any(|q| q == *n) && ctx.terrain(k, *n) == Terrain::Wall);
                if let Some(s) = beside {
                    let glyph = ctx.flavor.sign_glyph(rng);
                    ctx.set_feature(k, s, Some(Feature::Sign(glyph)));
                }
            }
        }

        // Lobby terminal, only where it cannot split the interior.
        if let Some((_, lobby)) = ctx
            .scratch
            .lobbies
            .iter()
            .find(|(id, _)| *id == b.id)
            .copied()
        {
            let candidates: Vec<Pos> = lobby
                .neighbors4()
                .filter(|n| fp.inset(1).contains(*n))
                .filter(|n| ctx.terrain(k, *n) == Terrain::Floor && bare(ctx, k, *n))
                .collect();
            for c in candidates {
                ctx.set_feature(k, c, Some(Feature::Terminal));
                let st = ctx.city.stratum(k).expect("street");
                let comps = components(ctx.size(), |p| fp.contains(p) && st.tiles[p].is_walkable());
                if comps.len() == 1 {
                    break;
                }
                ctx.set_feature(k, c, None);
            }
        }

        // Rooftop gear.
        if b.span.contains(StratumKind::Heights) {
            let h = StratumKind::Heights;
            if fp.w >= 9 && fp.h >= 7 {
                let c = fp.center();
                for p in Rect::new(c.x - 1, c.y - 1, 3, 3).iter() {
                    if ctx.terrain(h, p) == Terrain::Rooftop && bare(ctx, h, p) {
                        ctx.set_feature(h, p, Some(Feature::Helipad));
                    }
                }
            }
            for p in fp.inset(1).iter() {
                if ctx.terrain(h, p) == Terrain::Rooftop
                    && bare(ctx, h, p)
                    && open_area(ctx, h, p)
                    && chance(rng, 0.08)
                {
                    ctx.set_feature(h, p, Some(Feature::AcUnit));
                }
            }
        }
    }

    fn streets(&mut self, ctx: &mut GenCtx, d: usize) {
        let district = ctx.city.districts[d].clone();
        let spec = district.theme.spec();
        let rng = self.rng.as_mut().expect("begin sets rng");
        for p in district.bounds.iter() {
            if !ctx.in_district(p, district.id) {
                continue;
            }
            let decay = ctx.scratch.decay.get(p).copied().unwrap_or(0.5);
            // Street level.
            let k = StratumKind::Street;
            if bare(ctx, k, p) {
                let terrain = ctx.terrain(k, p);
                let open = open_area(ctx, k, p);
                let feature = match terrain {
                    Terrain::Sidewalk if (p.x * 7 + p.y * 13) % spec.lamp_spacing == 0 => {
                        Some(Feature::Lamp)
                    }
                    Terrain::Road
                        if district.theme == Theme::CombatZone && open && chance(rng, 0.03) =>
                    {
                        Some(Feature::Barrier)
                    }
                    Terrain::Plaza
                        if district.theme == Theme::NightMarket && open && chance(rng, 0.3) =>
                    {
                        Some(Feature::Stall)
                    }
                    Terrain::Sidewalk if open && chance(rng, 0.01) => Some(Feature::Terminal),
                    Terrain::Road | Terrain::Sidewalk | Terrain::Alley
                        if chance(rng, decay * 0.08) =>
                    {
                        Some(Feature::Puddle)
                    }
                    Terrain::Alley | Terrain::Sidewalk if chance(rng, decay * 0.1) => {
                        Some(Feature::Trash)
                    }
                    Terrain::Floor if chance(rng, decay * 0.03) => Some(Feature::Vent),
                    _ => None,
                };
                if feature.is_some() {
                    ctx.set_feature(k, p, feature);
                }
            }
            // Undercity: pipes along tunnel edges, the odd work lamp.
            let k = StratumKind::Undercity;
            if bare(ctx, k, p) && ctx.terrain(k, p) == Terrain::Tunnel {
                let edge = p.neighbors4().any(|n| ctx.terrain(k, n) == Terrain::Void);
                if edge && open_area(ctx, k, p) && chance(rng, 0.15) {
                    ctx.set_feature(k, p, Some(Feature::Pipe));
                } else if chance(rng, 0.02) {
                    ctx.set_feature(k, p, Some(Feature::Lamp));
                }
            }
            // Skyways: cables and walkway lamps.
            let k = StratumKind::Skyways;
            if bare(ctx, k, p) && ctx.terrain(k, p) == Terrain::Walkway {
                if chance(rng, 0.1) {
                    ctx.set_feature(k, p, Some(Feature::Cable));
                } else if chance(rng, 0.03) {
                    ctx.set_feature(k, p, Some(Feature::Lamp));
                }
            }
        }
    }

    /// Blocking features must never cut tiles off from the street: any blocker beside an
    /// unreached walkable tile is removed, repeatedly, until nothing is cut off.
    fn verify(ctx: &mut GenCtx) {
        for _ in 0..6 {
            let reach = reachable_from_street(&ctx.city);
            let mut to_clear: Vec<(StratumKind, Pos)> = Vec::new();
            for k in StratumKind::PHYSICAL {
                let st = ctx.city.stratum(k).expect("stratum");
                for (p, t) in st.tiles.iter() {
                    if !t.is_walkable() || reach[k.index()][p] {
                        continue;
                    }
                    for n in p.neighbors4() {
                        let blocker = st.tiles.get(n).is_some_and(|u| {
                            u.feature.is_some()
                                && !u.is_walkable()
                                && Tile::new(u.terrain).is_walkable()
                        });
                        if blocker {
                            to_clear.push((k, n));
                        }
                    }
                }
            }
            if to_clear.is_empty() {
                return;
            }
            for (k, p) in to_clear {
                ctx.set_feature(k, p, None);
            }
        }
    }

    fn lights(ctx: &mut GenCtx, k: StratumKind) {
        let base = base_light(k);
        let st = ctx.city.stratum_mut(k).expect("stratum");
        let tiles = &st.tiles;
        let light = crate::lighting::compute(
            tiles,
            base,
            |p| tiles.get(p).is_none_or(|t| t.blocks_sight()),
            |t| emission(t.terrain, t.feature, k),
        );
        st.light = light;
    }

    fn variants(ctx: &mut GenCtx, k: StratumKind) {
        let seed = ctx.seed;
        let size = ctx.size();
        let st = ctx.city.stratum_mut(k).expect("stratum");
        for (i, t) in st.tiles.as_mut_slice().iter_mut().enumerate() {
            let p = Pos::new(i as i32 % size.w as i32, i as i32 / size.w as i32);
            t.variant = tile_hash(seed, k, p);
        }
    }
}

impl StageRunner for DressingStage {
    fn begin(&mut self, ctx: &mut GenCtx) -> Result<(), GenError> {
        self.rng = Some(ctx.rng(Stage::Dressing));
        self.queue.clear();
        self.queue
            .extend((0..ctx.city.buildings.len()).map(Work::Facade));
        self.queue
            .extend((0..ctx.city.districts.len()).map(Work::Streets));
        // Megablocks get their outside after the street pass, before Verify checks it.
        let megablocks = (0..ctx.city.buildings.len())
            .filter(|i| ctx.city.buildings[*i].kind == BuildingKind::Megablock);
        self.queue.extend(megablocks.map(Work::Exterior));
        self.queue.push_back(Work::Verify);
        self.queue
            .extend(StratumKind::PHYSICAL.into_iter().map(Work::Lights));
        self.queue
            .extend(StratumKind::PHYSICAL.into_iter().map(Work::Variants));
        self.total = self.queue.len().max(1);
        Ok(())
    }

    fn step(&mut self, ctx: &mut GenCtx) -> Result<StepOut, GenError> {
        let Some(work) = self.queue.pop_front() else {
            return Ok(StepOut::Finished);
        };
        match work {
            Work::Facade(i) => self.facade(ctx, i),
            Work::Streets(d) => self.streets(ctx, d),
            Work::Exterior(i) => exterior::dress(ctx, i),
            Work::Verify => Self::verify(ctx),
            Work::Lights(k) => Self::lights(ctx, k),
            Work::Variants(k) => Self::variants(ctx, k),
        }
        let done = self.total - self.queue.len();
        Ok(StepOut::Continue(done as f32 / self.total as f32))
    }
}

#[cfg(test)]
mod tests {
    use crate::generate::testutil::small;
    use crate::params::WaterMode;
    use crate::tile::{Feature, Terrain};
    use crate::world::StratumKind;

    #[test]
    fn dressing_places_features_and_lights() {
        let c = small(81, WaterMode::Bay, 7);
        let st = c.stratum(StratumKind::Street).unwrap();
        let count = |f: fn(&Feature) -> bool| {
            st.tiles
                .iter()
                .filter(|(_, t)| t.feature.as_ref().is_some_and(f))
                .count()
        };
        assert!(count(|f| matches!(f, Feature::Neon(_))) > 30);
        assert!(count(|f| matches!(f, Feature::Lamp)) > 30);
        assert!(count(|f| matches!(f, Feature::Sign(_))) > 5);
        let open = c.buildings.iter().filter(|b| !b.kind.is_closed()).count();
        assert!(count(|f| matches!(f, Feature::Terminal)) >= open / 2);
        assert!(st.light.as_slice().iter().any(|&l| l > 150));
        assert!(
            c.stratum(StratumKind::Undercity)
                .unwrap()
                .light
                .as_slice()
                .iter()
                .filter(|&&l| l < 40)
                .count()
                > 100
        );
        for k in c
            .connectors
            .iter()
            .filter(|k| k.kind != crate::world::ConnectorKind::Jack)
        {
            for s in &k.serves {
                let t = c.tile(*s, k.pos).unwrap();
                assert!(t.is_walkable(), "{k:?} blocked on {s:?}");
            }
        }
        assert!(
            st.tiles
                .iter()
                .all(|(_, t)| t.terrain != Terrain::Door || t.feature.is_none())
        );
        let hi = c.stratum(StratumKind::Heights).unwrap();
        assert!(
            hi.tiles
                .iter()
                .any(|(_, t)| t.feature == Some(Feature::AcUnit))
        );
    }

    #[test]
    fn variants_are_set_everywhere() {
        let c = small(81, WaterMode::None, 5);
        let st = c.stratum(StratumKind::Street).unwrap();
        let distinct: std::collections::HashSet<u8> =
            st.tiles.as_slice().iter().map(|t| t.variant).collect();
        assert!(distinct.len() > 100);
        let uc = c.stratum(StratumKind::Undercity).unwrap();
        assert!(uc.tiles.as_slice().iter().any(|t| t.variant != 0));
    }
}
