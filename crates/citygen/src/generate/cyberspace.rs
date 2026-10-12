//! Stage 12: cyberspace — the Net stratum as a grid with constructs, hubs, lanes and ports.

use super::{GenCtx, StageRunner, StepOut};
use crate::geom::{Pos, Rect};
use crate::ids::{BuildingId, DistrictId};
use crate::net::{NodeKind, SubnetOwner};
use crate::params::GenError;
use crate::rng::tile_hash;
use crate::tile::{Feature, Terrain, Tile};
use crate::world::{Stratum, StratumKind};
use std::collections::VecDeque;

const K: StratumKind = StratumKind::Net;
const GRID_LIGHT: u8 = 200;
const HUB_RADIUS: i32 = 2;

enum Work {
    Begin,
    Construct(usize),
    Gate(usize),
    Hub(usize),
}

#[derive(Default)]
pub(crate) struct CyberspaceStage {
    queue: VecDeque<Work>,
    total: usize,
    /// Gate tile per building index.
    pub(crate) gates: Vec<Pos>,
    /// Backbone anchor per district index.
    pub(crate) backbones: Vec<Option<Pos>>,
}

/// Sets terrain, owner and light of one Net tile; out-of-bounds is a no-op.
pub(crate) fn set(ctx: &mut GenCtx, p: Pos, t: Terrain, b: Option<BuildingId>, light: u8) {
    if let Some(tile) = ctx.city.tile_mut(K, p) {
        tile.terrain = t;
        tile.building = b;
    }
    if let Some(st) = ctx.city.stratum_mut(K) {
        st.light.set(p, light);
    }
}

fn is_corner(r: Rect, p: Pos) -> bool {
    (p.x == r.x || p.x == r.right() - 1) && (p.y == r.y || p.y == r.bottom() - 1)
}

/// The tile just outside a footprint border tile, away from the centre.
fn outward(r: Rect, p: Pos) -> Pos {
    if p.y == r.y {
        p.offset(0, -1)
    } else if p.y == r.bottom() - 1 {
        p.offset(0, 1)
    } else if p.x == r.x {
        p.offset(-1, 0)
    } else {
        p.offset(1, 0)
    }
}

fn backbone_anchor(ctx: &GenCtx, d: DistrictId) -> Option<Pos> {
    ctx.city
        .net
        .nodes
        .iter()
        .find(|n| {
            n.kind == NodeKind::Backbone
                && ctx.city.net.subnet(n.subnet).owner == SubnetOwner::District(d)
        })
        .map(|n| n.anchor.1)
}

impl CyberspaceStage {
    fn begin_net(ctx: &mut GenCtx) {
        let size = ctx.size();
        ctx.city.strata.retain(|s| s.kind != K);
        let mut st = Stratum::new(K, size);
        for y in 0..size.h as i32 {
            for x in 0..size.w as i32 {
                let p = Pos::new(x, y);
                let district = ctx
                    .city
                    .tile(StratumKind::Street, p)
                    .and_then(|t| t.district);
                let lattice = u8::from(x % 4 == 0 && y % 2 == 0);
                let variant = (tile_hash(ctx.seed, K, p) & 0xFE) | lattice;
                st.tiles.set(
                    p,
                    Tile {
                        terrain: Terrain::NetGrid,
                        feature: None,
                        district,
                        building: None,
                        variant,
                    },
                );
                st.light.set(p, GRID_LIGHT);
            }
        }
        ctx.city.strata.push(st);
    }

    fn construct(&mut self, ctx: &mut GenCtx, i: usize) {
        let b = ctx.city.buildings[i].clone();
        let security = ctx
            .city
            .net
            .subnets
            .iter()
            .find(|s| s.owner == SubnetOwner::Building(b.id))
            .map(|s| s.security)
            .unwrap_or(0);
        let light = 96u8.saturating_add((security as u32 * 5 / 8) as u8);
        for p in b.footprint.iter() {
            set(ctx, p, Terrain::NetWall, Some(b.id), light);
        }
        for p in b.footprint.inset(1).iter() {
            set(ctx, p, Terrain::NetFloor, Some(b.id), light);
        }
    }

    /// Picks the gate once every construct stands, so it never opens onto another wall.
    fn gate(&mut self, ctx: &mut GenCtx, i: usize) {
        let b = ctx.city.buildings[i].clone();
        let fp = b.footprint;
        let target = self.backbones[b.district.index()].unwrap_or(fp.center());
        let open = |ctx: &GenCtx, p: Pos| {
            ctx.city
                .tile(K, outward(fp, p))
                .is_some_and(|t| t.terrain == Terrain::NetGrid)
        };
        let candidates: Vec<Pos> = fp.border().filter(|p| !is_corner(fp, *p)).collect();
        let gate = candidates
            .iter()
            .copied()
            .filter(|p| open(ctx, *p))
            .min_by_key(|p| (p.dist2(target), p.y, p.x))
            .or_else(|| {
                candidates
                    .iter()
                    .copied()
                    .min_by_key(|p| (p.dist2(target), p.y, p.x))
            })
            .unwrap_or(fp.center());
        set(ctx, gate, Terrain::NetGate, Some(b.id), 255);
        self.gates[i] = gate;
    }

    fn hub(&mut self, ctx: &mut GenCtx, d: usize) {
        let Some(c) = self.backbones[d] else { return };
        if ctx.city.tile(K, c).is_none_or(|t| t.building.is_some()) {
            return;
        }
        let r = Rect::new(
            c.x - HUB_RADIUS,
            c.y - HUB_RADIUS,
            2 * HUB_RADIUS + 1,
            2 * HUB_RADIUS + 1,
        );
        let keep_open = |ctx: &GenCtx, p: Pos| {
            // Never wall a port, and never seal a construct gate.
            ctx.city
                .tile(StratumKind::Street, p)
                .is_some_and(|t| t.feature == Some(Feature::Terminal))
                || p.neighbors4()
                    .any(|q| ctx.terrain(K, q) == Terrain::NetGate)
        };
        for p in r.border() {
            if ctx.terrain(K, p) == Terrain::NetGrid && !keep_open(ctx, p) {
                set(ctx, p, Terrain::NetWall, None, 255);
            }
        }
        for p in [
            Pos::new(c.x, r.y),
            Pos::new(r.right() - 1, c.y),
            Pos::new(c.x, r.bottom() - 1),
            Pos::new(r.x, c.y),
        ] {
            if ctx.terrain(K, p) == Terrain::NetWall
                && ctx.city.tile(K, p).is_some_and(|t| t.building.is_none())
            {
                set(ctx, p, Terrain::NetGate, None, 255);
            }
        }
    }
}

impl StageRunner for CyberspaceStage {
    fn begin(&mut self, ctx: &mut GenCtx) -> Result<(), GenError> {
        let nb = ctx.city.buildings.len();
        let nd = ctx.city.districts.len();
        self.gates = vec![Pos::new(0, 0); nb];
        self.backbones = (0..nd)
            .map(|d| backbone_anchor(ctx, DistrictId(d as u32)))
            .collect();
        self.queue.clear();
        self.queue.push_back(Work::Begin);
        self.queue.extend((0..nb).map(Work::Construct));
        self.queue.extend((0..nb).map(Work::Gate));
        self.queue.extend((0..nd).map(Work::Hub));
        self.total = self.queue.len().max(1);
        Ok(())
    }

    fn step(&mut self, ctx: &mut GenCtx) -> Result<StepOut, GenError> {
        let Some(work) = self.queue.pop_front() else {
            return Ok(StepOut::Finished);
        };
        match work {
            Work::Begin => Self::begin_net(ctx),
            Work::Construct(i) => self.construct(ctx, i),
            Work::Gate(i) => self.gate(ctx, i),
            Work::Hub(d) => self.hub(ctx, d),
        }
        let done = self.total - self.queue.len();
        Ok(StepOut::Continue(done as f32 / self.total as f32))
    }
}

#[cfg(test)]
mod tests {
    use crate::generate::Generator;
    use crate::geom::Pos;
    use crate::params::{GenParams, SizePreset};
    use crate::tile::Terrain;
    use crate::world::StratumKind;

    pub(crate) fn city() -> crate::world::City {
        Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn net_stratum_is_a_grid_with_constructs_on_every_footprint() {
        let c = city();
        let net = c.stratum(StratumKind::Net).expect("fifth stratum");
        assert_eq!(c.strata.len(), 5);
        assert_eq!(net.tiles.size(), c.size);
        // Grid tiles mirror the street's districts and carry the lattice bit.
        let lattice = Pos::new(4, 2);
        let t = net.tiles[lattice];
        assert_eq!(t.terrain, Terrain::NetGrid);
        assert_eq!(t.variant & 1, 1);
        assert_eq!(net.tiles[Pos::new(5, 2)].variant & 1, 0);
        assert_eq!(
            t.district,
            c.tile(StratumKind::Street, lattice).unwrap().district
        );
        for b in &c.buildings {
            let fp = b.footprint;
            let gates = fp
                .border()
                .filter(|p| net.tiles[*p].terrain == Terrain::NetGate)
                .count();
            assert_eq!(gates, 1, "building {:?} has {gates} gates", b.id);
            for p in fp.border() {
                let t = net.tiles[p];
                assert!(
                    matches!(t.terrain, Terrain::NetWall | Terrain::NetGate),
                    "{:?} border {p:?} is {:?}",
                    b.id,
                    t.terrain
                );
                assert_eq!(t.building, Some(b.id));
            }
            for p in fp.inset(1).iter() {
                assert_eq!(net.tiles[p].terrain, Terrain::NetFloor);
            }
            let light = net.light[fp.center()];
            assert!(light >= 96, "construct light {light}");
        }
        assert!(net.tiles.count(|t| t.terrain == Terrain::NetGrid) > c.size.area() / 4);
    }

    #[test]
    fn hubs_ring_every_backbone_that_stands_on_the_grid() {
        let c = city();
        let net = c.stratum(StratumKind::Net).unwrap();
        let mut hubs = 0;
        for n in c
            .net
            .nodes
            .iter()
            .filter(|n| n.kind == crate::net::NodeKind::Backbone)
        {
            let a = n.anchor.1;
            if net.tiles[a].building.is_some() {
                continue;
            }
            hubs += 1;
            let ring = crate::geom::Rect::new(a.x - 2, a.y - 2, 5, 5);
            let walls = ring
                .border()
                .filter(|p| {
                    net.tiles
                        .get(*p)
                        .is_some_and(|t| t.terrain == Terrain::NetWall)
                })
                .count();
            let gates = ring
                .border()
                .filter(|p| {
                    net.tiles
                        .get(*p)
                        .is_some_and(|t| t.terrain == Terrain::NetGate)
                })
                .count();
            assert!(walls >= 8, "hub at {a:?} has only {walls} wall tiles");
            assert!(gates >= 2, "hub at {a:?} has only {gates} gates");
        }
        assert!(hubs >= 1);
    }
}
