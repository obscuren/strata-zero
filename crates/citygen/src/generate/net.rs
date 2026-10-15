//! Stage 11: the Net graph — subnets, nodes anchored to physical tiles, links.

use super::{GenCtx, StageRunner, StepOut};
use crate::geom::Pos;
use crate::ids::{BuildingId, DistrictId, NodeId, SubnetId};
use crate::net::{Link, Node, NodeKind, Subnet, SubnetOwner};
use crate::params::GenError;
use crate::tile::{Feature, Terrain};
use crate::world::{BuildingKind, StratumKind, Theme};
use std::collections::VecDeque;

enum Work {
    Building(usize),
    District(usize),
    Core,
}

#[derive(Default)]
pub(crate) struct NetStage {
    queue: VecDeque<Work>,
    total: usize,
    district_subnet: Vec<SubnetId>,
    city_subnet: Option<SubnetId>,
    /// Server node per building, by building index.
    servers: Vec<Option<NodeId>>,
    backbones: Vec<Option<NodeId>>,
}

const ICE_THRESHOLD: u8 = 160;
const CORE_SECURITY: u8 = 230;

impl NetStage {
    fn add_subnet(ctx: &mut GenCtx, owner: SubnetOwner, security: u8) -> SubnetId {
        let id = SubnetId(ctx.city.net.subnets.len() as u32);
        ctx.city.net.subnets.push(Subnet {
            id,
            owner,
            security,
        });
        id
    }

    fn add_node(
        ctx: &mut GenCtx,
        kind: NodeKind,
        anchor: (StratumKind, Pos),
        subnet: SubnetId,
        building: Option<BuildingId>,
    ) -> NodeId {
        let id = NodeId(ctx.city.net.nodes.len() as u32);
        ctx.city.net.nodes.push(Node {
            id,
            kind,
            anchor,
            cyber: anchor.1,
            subnet,
            building,
        });
        id
    }

    fn link(ctx: &mut GenCtx, a: NodeId, b: NodeId) {
        if a != b {
            ctx.city.net.links.push(Link(a, b));
        }
    }

    fn building(&mut self, ctx: &mut GenCtx, i: usize) {
        let b = ctx.city.buildings[i].clone();
        let district = ctx.city.district(b.district).clone();
        let base = ctx
            .city
            .net
            .subnet(self.district_subnet[b.district.index()])
            .security as f32;
        let security =
            (base + district.wealth * 40.0 - district.decay * 20.0).clamp(0.0, 255.0) as u8;
        let subnet = Self::add_subnet(ctx, SubnetOwner::Building(b.id), security);
        let k = StratumKind::Street;
        let fp = b.footprint;

        let lobby = ctx
            .scratch
            .lobbies
            .iter()
            .find(|(id, _)| *id == b.id)
            .map(|(_, p)| *p)
            .unwrap_or(fp.center());
        let mut floors: Vec<Pos> = fp
            .inset(1)
            .iter()
            .filter(|p| matches!(ctx.terrain(k, *p), Terrain::Floor | Terrain::Mass))
            .collect();
        floors.sort_by_key(|p| std::cmp::Reverse(p.dist2(lobby)));
        let server_pos = floors
            .first()
            .copied()
            .or_else(|| fp.inset(1).iter().find(|p| ctx.city.walkable(k, *p)))
            .unwrap_or(fp.center());
        let server = Self::add_node(ctx, NodeKind::Server, (k, server_pos), subnet, Some(b.id));
        self.servers[i] = Some(server);

        if matches!(b.kind, BuildingKind::CorpTower | BuildingKind::Clinic) {
            let vault_pos = floors.get(1).copied().unwrap_or(server_pos);
            let vault =
                Self::add_node(ctx, NodeKind::DataVault, (k, vault_pos), subnet, Some(b.id));
            Self::link(ctx, vault, server);
        }
        if security >= ICE_THRESHOLD {
            let ice = Self::add_node(ctx, NodeKind::Ice, (k, server_pos), subnet, Some(b.id));
            Self::link(ctx, ice, server);
        }

        let mut peripherals: Vec<(NodeKind, Pos)> = Vec::new();
        for p in fp.iter() {
            let Some(t) = ctx.tile(k, p) else { continue };
            let on_border = fp.border().any(|q| q == p);
            match (t.terrain, t.feature) {
                (_, Some(Feature::Terminal)) => peripherals.push((NodeKind::Terminal, p)),
                (_, Some(Feature::Camera)) => peripherals.push((NodeKind::Camera, p)),
                (_, Some(Feature::HoloAd)) => peripherals.push((NodeKind::AdBoard, p)),
                (Terrain::Door, _) if on_border => peripherals.push((NodeKind::DoorLock, p)),
                (Terrain::Lift, _) => peripherals.push((NodeKind::LiftCtrl, p)),
                _ => {}
            }
        }
        for (kind, p) in peripherals {
            let n = Self::add_node(ctx, kind, (k, p), subnet, Some(b.id));
            Self::link(ctx, n, server);
        }
    }

    fn district(&mut self, ctx: &mut GenCtx, d: usize) {
        let district = ctx.city.districts[d].clone();
        let subnet = self.district_subnet[d];
        let k = StratumKind::Street;
        let in_district = |ctx: &GenCtx, p: Pos| ctx.in_district(p, district.id);

        // Street-side terminals (outside buildings) and the first lamp.
        let mut street_terminals = Vec::new();
        let mut first_lamp = None;
        for p in district.bounds.iter() {
            if !in_district(ctx, p) {
                continue;
            }
            let Some(t) = ctx.tile(k, p) else { continue };
            if t.building.is_none() && t.feature == Some(Feature::Terminal) {
                street_terminals.push(p);
            }
            if first_lamp.is_none() && t.feature == Some(Feature::Lamp) {
                first_lamp = Some(p);
            }
        }

        // A hub rings its anchor two tiles out, so prefer a terminal clear of the edge.
        let size = ctx.size();
        let clear =
            |p: &Pos| p.x >= 2 && p.y >= 2 && p.x < size.w as i32 - 2 && p.y < size.h as i32 - 2;
        let backbone_anchor = street_terminals
            .iter()
            .copied()
            .find(clear)
            .or_else(|| street_terminals.first().copied())
            .or_else(|| {
                ctx.city
                    .buildings
                    .iter()
                    .enumerate()
                    .find(|(_, b)| b.district == district.id)
                    .and_then(|(i, _)| self.servers[i])
                    .map(|n| ctx.city.net.node(n).anchor.1)
            })
            .unwrap_or(district.center);
        let backbone = Self::add_node(ctx, NodeKind::Backbone, (k, backbone_anchor), subnet, None);
        self.backbones[d] = Some(backbone);

        if let Some(p) = first_lamp {
            let grid = Self::add_node(ctx, NodeKind::LampGrid, (k, p), subnet, None);
            Self::link(ctx, grid, backbone);
        }
        for p in street_terminals {
            if p == backbone_anchor {
                continue;
            }
            let n = Self::add_node(ctx, NodeKind::Terminal, (k, p), subnet, None);
            Self::link(ctx, n, backbone);
        }
        for i in 0..ctx.city.buildings.len() {
            if ctx.city.buildings[i].district == district.id
                && let Some(server) = self.servers[i]
            {
                Self::link(ctx, server, backbone);
            }
        }
    }

    fn core(&mut self, ctx: &mut GenCtx) {
        let subnet = Self::add_subnet(ctx, SubnetOwner::City, CORE_SECURITY);
        self.city_subnet = Some(subnet);
        let k = StratumKind::Street;
        let host = ctx
            .city
            .buildings
            .iter()
            .enumerate()
            .filter(|(_, b)| ctx.city.district(b.district).theme == Theme::CorpCore)
            .max_by_key(|(_, b)| b.footprint.area())
            .or_else(|| {
                ctx.city
                    .buildings
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, b)| b.footprint.area())
            })
            .map(|(i, b)| (i, b.footprint));
        let Some((host_idx, fp)) = host else { return };
        let anchor = fp
            .inset(1)
            .iter()
            .filter(|p| matches!(ctx.terrain(k, *p), Terrain::Floor | Terrain::Mass))
            .min_by_key(|p| p.dist2(fp.center()))
            .unwrap_or(fp.center());
        let building = Some(ctx.city.buildings[host_idx].id);
        let server = Self::add_node(ctx, NodeKind::Server, (k, anchor), subnet, building);
        let ice = Self::add_node(ctx, NodeKind::Ice, (k, anchor), subnet, building);
        Self::link(ctx, ice, server);
        for d in 0..self.backbones.len() {
            if let Some(b) = self.backbones[d] {
                Self::link(ctx, b, server);
            }
        }
    }
}

impl StageRunner for NetStage {
    fn begin(&mut self, ctx: &mut GenCtx) -> Result<(), GenError> {
        ctx.city.net = Default::default();
        self.district_subnet.clear();
        for d in 0..ctx.city.districts.len() {
            let district = &ctx.city.districts[d];
            let security = (district.theme.spec().security as f32 + district.wealth * 30.0)
                .clamp(0.0, 255.0) as u8;
            let id = DistrictId(d as u32);
            let sn = Self::add_subnet(ctx, SubnetOwner::District(id), security);
            self.district_subnet.push(sn);
        }
        self.servers = vec![None; ctx.city.buildings.len()];
        self.backbones = vec![None; ctx.city.districts.len()];
        self.queue.clear();
        self.queue
            .extend((0..ctx.city.buildings.len()).map(Work::Building));
        self.queue
            .extend((0..ctx.city.districts.len()).map(Work::District));
        self.queue.push_back(Work::Core);
        self.total = self.queue.len().max(1);
        Ok(())
    }

    fn step(&mut self, ctx: &mut GenCtx) -> Result<StepOut, GenError> {
        let Some(work) = self.queue.pop_front() else {
            return Ok(StepOut::Finished);
        };
        match work {
            Work::Building(i) => self.building(ctx, i),
            Work::District(d) => self.district(ctx, d),
            Work::Core => self.core(ctx),
        }
        let done = self.total - self.queue.len();
        Ok(StepOut::Continue(done as f32 / self.total as f32))
    }
}

#[cfg(test)]
mod tests {
    use crate::generate::testutil::small;
    use crate::net::{Link, NodeKind, SubnetOwner};
    use crate::params::WaterMode;
    use crate::tile::{Feature, Terrain};

    #[test]
    fn net_graph_is_consistent() {
        let c = small(101, WaterMode::Bay, 7);
        assert_eq!(
            c.net
                .subnets
                .iter()
                .filter(|s| matches!(s.owner, SubnetOwner::Building(_)))
                .count(),
            c.buildings.len()
        );
        assert_eq!(
            c.net
                .subnets
                .iter()
                .filter(|s| matches!(s.owner, SubnetOwner::District(_)))
                .count(),
            c.districts.len()
        );
        assert_eq!(
            c.net
                .subnets
                .iter()
                .filter(|s| s.owner == SubnetOwner::City)
                .count(),
            1
        );
        assert!(!c.net.nodes.is_empty());
        for (i, n) in c.net.nodes.iter().enumerate() {
            assert_eq!(n.id.index(), i);
            let t = c.tile(n.anchor.0, n.anchor.1).expect("anchor in bounds");
            match n.kind {
                NodeKind::Terminal => assert_eq!(t.feature, Some(Feature::Terminal)),
                NodeKind::Camera => assert_eq!(t.feature, Some(Feature::Camera)),
                NodeKind::DoorLock => assert_eq!(t.terrain, Terrain::Door),
                NodeKind::LiftCtrl => assert_eq!(t.terrain, Terrain::Lift),
                NodeKind::AdBoard => assert_eq!(t.feature, Some(Feature::HoloAd)),
                NodeKind::Server | NodeKind::DataVault | NodeKind::Ice => {
                    assert!(matches!(
                        t.terrain,
                        Terrain::Floor | Terrain::Mass | Terrain::Lift | Terrain::Stair
                    ))
                }
                NodeKind::LampGrid => assert_eq!(t.feature, Some(Feature::Lamp)),
                NodeKind::Backbone => {
                    assert!(
                        t.feature == Some(Feature::Terminal)
                            || matches!(t.terrain, Terrain::Floor | Terrain::Mass)
                    )
                }
            }
            assert!(c.net.subnets.iter().any(|s| s.id == n.subnet));
        }
        for Link(a, b) in &c.net.links {
            assert!(a.index() < c.net.nodes.len() && b.index() < c.net.nodes.len());
        }
        let backbones: Vec<_> = c
            .net
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Backbone)
            .map(|n| n.id)
            .collect();
        assert_eq!(backbones.len(), c.districts.len());
        for s in c
            .net
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Server && n.building.is_some())
        {
            assert!(
                c.net
                    .links
                    .iter()
                    .any(|Link(a, b)| (*a == s.id && backbones.contains(b))
                        || (*b == s.id && backbones.contains(a))),
                "{:?} not linked to a backbone",
                s.id
            );
        }
        assert!(c.net.nodes.iter().any(|n| n.kind == NodeKind::Ice));
        assert!(c.net.nodes.iter().any(|n| n.kind == NodeKind::DoorLock));
    }
}
