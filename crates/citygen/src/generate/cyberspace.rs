//! Stage 12: cyberspace — the Net stratum as a grid with constructs, hubs, lanes and ports.

use super::{GenCtx, StageRunner, StepOut};
use crate::geom::{Grid, Pos, Rect, Size};
use crate::ids::{BuildingId, DistrictId};
use crate::net::{Node, NodeKind, SubnetOwner};
use crate::params::GenError;
use crate::pathing::{astar, components};
use crate::rng::tile_hash;
use crate::tile::{Feature, Terrain, Tile};
use crate::world::{Connector, ConnectorKind, Stratum, StratumKind};
use std::collections::VecDeque;

const K: StratumKind = StratumKind::Net;
const HUB_RADIUS: i32 = 2;
const NODE_SEARCH: i32 = 6;
/// How far a lane may stray from the box spanned by its endpoints.
const LANE_MARGIN: i32 = 16;

enum Work {
    Begin,
    Construct(usize),
    Gate(usize),
    Hub(usize),
    Nodes,
    Lane(usize),
    Site,
    Ports,
}

#[derive(Default)]
pub(crate) struct CyberspaceStage {
    queue: VecDeque<Work>,
    total: usize,
    /// Gate tile per building index.
    pub(crate) gates: Vec<Pos>,
    /// Backbone anchor per district index.
    pub(crate) backbones: Vec<Option<Pos>>,
    /// The main open-grid component once every construct stands; gates prefer to open onto it.
    open: Option<Grid<bool>>,
}

/// How far from anything a city on its site built its own grid runs past its outline
/// (so a corner of it joined only round the outside stays joined).
const FRINGE: i32 = 6;

/// A city on its site: the cells of its Net that are the open Net's, past its outline
/// and more than [`FRINGE`] steps from any of its land or buildings.
pub(crate) fn open_net_mask(street: &Grid<Tile>, site: &crate::generate::site::Site) -> Grid<bool> {
    let size = street.size();
    let mut dist = Grid::filled(size, i32::MAX);
    let mut queue = VecDeque::new();
    for (p, t) in street.iter() {
        if t.district.is_some() || t.building.is_some() {
            dist[p] = 0;
            queue.push_back(p);
        }
    }
    while let Some(p) = queue.pop_front() {
        let d = dist[p];
        if d >= FRINGE {
            continue;
        }
        for q in p.neighbors4() {
            if size.contains(q) && dist[q] > d + 1 {
                dist[q] = d + 1;
                queue.push_back(q);
            }
        }
    }
    let mut mask = Grid::filled(size, false);
    for (p, _) in street.iter() {
        mask[p] = site.outline[p] < crate::generate::site::OUTLINE && dist[p] > FRINGE;
    }
    mask
}

/// Sets terrain, owner and light of one Net tile; out-of-bounds is a no-op.
pub(crate) fn set(ctx: &mut GenCtx, p: Pos, t: Terrain, b: Option<BuildingId>, light: u8) {
    if let Some(tile) = ctx.city.tile_mut(K, p) {
        tile.terrain = t;
        tile.building = b;
    }
    if let Some(st) = ctx.city.stratum_mut(K) {
        st.steady.set(p, light);
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

/// A* confined to the bounding box of `start` and `goal` grown by `margin`, so each
/// search allocates a small grid instead of the whole map. A detour that leaves the
/// box is not found.
fn bounded_astar(
    size: Size,
    start: Pos,
    goal: Pos,
    margin: i32,
    cost: impl Fn(Pos, Pos) -> Option<u32>,
) -> Option<Vec<Pos>> {
    let x0 = (start.x.min(goal.x) - margin).max(0);
    let y0 = (start.y.min(goal.y) - margin).max(0);
    let x1 = (start.x.max(goal.x) + margin + 1).min(size.w as i32);
    let y1 = (start.y.max(goal.y) + margin + 1).min(size.h as i32);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let sub = Size {
        w: (x1 - x0) as u32,
        h: (y1 - y0) as u32,
    };
    let to_local = |p: Pos| Pos::new(p.x - x0, p.y - y0);
    let to_world = |p: Pos| Pos::new(p.x + x0, p.y + y0);
    astar(sub, to_local(start), to_local(goal), |a, b| {
        cost(to_world(a), to_world(b))
    })
    .map(|path| path.into_iter().map(to_world).collect())
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
                st.steady.set(p, crate::lighting::tuning::NET_GRID_LIGHT);
            }
        }
        ctx.city.strata.push(st);
        Self::on_its_site(ctx);
    }

    /// A city on its site, before anything of its own is laid on its Net: past its
    /// outline, away from what it built, the open Net (#18's), its void dark and its grid
    /// dotted; through its outskirts the grid's dots and the districts' tint thin out
    /// with the city.
    fn on_its_site(ctx: &mut GenCtx) {
        use crate::lighting::tuning::{NET_GRID_LIGHT, NET_LANE_LIGHT};
        use crate::overworld::detail::roll;
        /// How dense the city is where its grid has fully come in.
        const FADE: f32 = 0.2;
        let Some(site) = ctx.scratch.site.as_ref() else {
            return;
        };
        let size = ctx.size();
        let Some(street) = ctx.city.stratum(StratumKind::Street) else {
            return;
        };
        let mask = open_net_mask(&street.tiles, site);
        let open = crate::overworld::net_ground::region(
            &site.world,
            site.origin.x,
            site.origin.y,
            size.w,
            size.h,
        );
        let seed = ctx.seed;
        let density = site.density.clone();
        let Some(net) = ctx.city.stratum_mut(K) else {
            return;
        };
        for y in 0..size.h as i32 {
            for x in 0..size.w as i32 {
                let p = Pos::new(x, y);
                if mask[p] {
                    let mut o = open[p];
                    match o.terrain {
                        Terrain::NetGrid => o.variant |= 1,
                        Terrain::Void => o.variant &= !1,
                        _ => {}
                    }
                    let light = match o.terrain {
                        Terrain::NetLane | Terrain::NetGate => NET_LANE_LIGHT,
                        Terrain::NetGrid => NET_GRID_LIGHT,
                        Terrain::NetWall | Terrain::NetFloor => 96,
                        _ => 0,
                    };
                    net.tiles[p] = Tile {
                        district: None,
                        building: None,
                        ..o
                    };
                    net.steady.set(p, light);
                } else {
                    let f = (density[p] / FADE).clamp(0.0, 1.0);
                    if roll(seed, 0xFADE, p) > f {
                        let t = &mut net.tiles[p];
                        t.variant &= !1;
                        t.district = None;
                    }
                }
            }
        }
        ctx.scratch.open_net = Some(mask);
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

    /// Marks the largest connected region of open grid; computed once, after the constructs.
    fn ensure_open(&mut self, ctx: &GenCtx) {
        if self.open.is_some() {
            return;
        }
        let size = ctx.size();
        let comps = components(size, |p| ctx.terrain(K, p) == Terrain::NetGrid);
        let mut open = Grid::filled(size, false);
        if let Some(main) = comps.iter().max_by_key(|c| c.len()) {
            for p in main {
                open.set(*p, true);
            }
        }
        self.open = Some(open);
    }

    /// Picks the gate once every construct stands: onto the main grid if any border tile
    /// allows it, else onto any open tile, else the nearest non-corner border tile.
    fn gate(&mut self, ctx: &mut GenCtx, i: usize) {
        self.ensure_open(ctx);
        let b = ctx.city.buildings[i].clone();
        let fp = b.footprint;
        let target = self.backbones[b.district.index()].unwrap_or(fp.center());
        let candidates: Vec<Pos> = fp.border().filter(|p| !is_corner(fp, *p)).collect();
        let gate = {
            let open = self.open.as_ref().expect("set above");
            let on_main = |p: Pos| open.get(outward(fp, p)).copied().unwrap_or(false);
            let on_grid = |p: Pos| {
                ctx.city
                    .tile(K, outward(fp, p))
                    .is_some_and(|t| t.terrain == Terrain::NetGrid)
            };
            let pick = |keep: &dyn Fn(Pos) -> bool| {
                candidates
                    .iter()
                    .copied()
                    .filter(|p| keep(*p))
                    .min_by_key(|p| (p.dist2(target), p.y, p.x))
            };
            pick(&on_main)
                .or_else(|| pick(&on_grid))
                .or_else(|| pick(&|_| true))
                .unwrap_or(fp.center())
        };
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
        let open = |ctx: &GenCtx, p: Pos| {
            ctx.scratch
                .open_net
                .as_ref()
                .is_some_and(|o| o.get(p).copied().unwrap_or(false))
        };
        for p in r.border() {
            if ctx.terrain(K, p) == Terrain::NetGrid && !keep_open(ctx, p) && !open(ctx, p) {
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

    /// Nearest walkable Net tile without a feature, scanning square rings in a fixed order.
    fn nearest_free(ctx: &GenCtx, c: Pos) -> Option<Pos> {
        for r in 1..=NODE_SEARCH {
            let ring = Rect::new(c.x - r, c.y - r, 2 * r + 1, 2 * r + 1);
            let hit = ring.border().find(|p| {
                ctx.city
                    .tile(K, *p)
                    .is_some_and(|t| t.is_walkable() && t.feature.is_none())
                    && !ctx
                        .scratch
                        .open_net
                        .as_ref()
                        .is_some_and(|o| o.get(*p).copied().unwrap_or(false))
            });
            if hit.is_some() {
                return hit;
            }
        }
        None
    }

    /// ICE first so it always takes the gate; then everything else in id order.
    fn nodes(&mut self, ctx: &mut GenCtx) {
        let mut order: Vec<usize> = (0..ctx.city.net.nodes.len()).collect();
        order.sort_by_key(|i| (ctx.city.net.nodes[*i].kind != NodeKind::Ice, *i));
        for i in order {
            let node = ctx.city.net.nodes[i].clone();
            let mut t = node.anchor.1;
            if node.kind == NodeKind::Ice
                && let Some(b) = node.building
            {
                t = self.gates[b.index()];
            }
            let taken = ctx
                .city
                .tile(K, t)
                .is_none_or(|x| matches!(x.feature, Some(Feature::Node(_))));
            if taken {
                t = Self::nearest_free(ctx, t).unwrap_or(t);
            }
            ctx.set_feature(K, t, Some(Feature::Node(node.kind)));
            ctx.city.net.nodes[i].cyber = t;
        }
    }

    /// The construct a node stands in: its own building, else the one its cyber tile lies in
    /// (a district backbone anchored at a server has no building of its own).
    fn host(ctx: &GenCtx, n: &Node) -> Option<BuildingId> {
        n.building
            .or_else(|| ctx.city.tile(K, n.cyber).and_then(|t| t.building))
    }

    fn lane_end(&self, ctx: &GenCtx, n: &Node) -> Pos {
        match Self::host(ctx, n) {
            Some(b) => self.gates[b.index()],
            None => n.cyber,
        }
    }

    fn lane(&mut self, ctx: &mut GenCtx, li: usize) {
        let link = ctx.city.net.links[li];
        let a = ctx.city.net.node(link.0).clone();
        let b = ctx.city.net.node(link.1).clone();
        let (ha, hb) = (Self::host(ctx, &a), Self::host(ctx, &b));
        if ha.is_some() && ha == hb {
            return;
        }
        let (start, goal) = (self.lane_end(ctx, &a), self.lane_end(ctx, &b));
        if start == goal {
            return;
        }
        let size = ctx.size();
        let path = {
            let city = &ctx.city;
            let open = ctx.scratch.open_net.as_ref();
            bounded_astar(size, start, goal, LANE_MARGIN, |_, q| {
                let t = city.tile(K, q)?;
                if q == start || q == goal {
                    return Some(1);
                }
                if open.is_some_and(|o| o.get(q).copied().unwrap_or(false)) {
                    return None;
                }
                match t.terrain {
                    Terrain::NetLane | Terrain::NetGate => Some(1),
                    Terrain::NetGrid => Some(if matches!(t.feature, Some(Feature::Node(_))) {
                        1
                    } else {
                        3
                    }),
                    _ => None,
                }
            })
        };
        let Some(path) = path else { return };
        for p in path {
            if ctx.terrain(K, p) == Terrain::NetGrid {
                ctx.set_terrain(K, p, Terrain::NetLane);
                if let Some(st) = ctx.city.stratum_mut(K) {
                    st.steady.set(p, crate::lighting::tuning::NET_LANE_LIGHT);
                }
            }
        }
    }

    /// A city on its site: one shanty splices a stolen line straight off the nearest
    /// lane (onto the open Net's trunk at most, never across the open Net), the first
    /// door in the hash's order that reaches one.
    fn site(ctx: &mut GenCtx) {
        use crate::generate::site::OUTLINE;
        use crate::lighting::tuning::NET_LANE_LIGHT;
        use crate::overworld::detail::roll;
        let (Some(site), Some(open)) = (ctx.scratch.site.as_ref(), ctx.scratch.open_net.as_ref())
        else {
            return;
        };
        let seed = ctx.seed;
        let (Some(street), Some(net)) =
            (ctx.city.stratum(StratumKind::Street), ctx.city.stratum(K))
        else {
            return;
        };
        let mut doors: Vec<Pos> = street
            .tiles
            .iter()
            .filter(|(p, t)| {
                t.terrain == Terrain::Door && t.building.is_none() && site.outline[*p] >= OUTLINE
            })
            .map(|(p, _)| p)
            .collect();
        doors.sort_by_key(|p| (roll(seed, 0x57E1, *p) * 1e6) as u64);
        let run_from = |d: Pos| {
            [(0, 1), (0, -1), (1, 0), (-1, 0)]
                .into_iter()
                .filter_map(|(dx, dy)| {
                    let mut run = vec![d];
                    for k in 1..=80 {
                        let q = d.offset(dx * k, dy * k);
                        let t = net.tiles.get(q)?.terrain;
                        if open.get(q).copied().unwrap_or(true) {
                            // the open Net's trunk, or nothing
                            return (t == Terrain::NetLane).then_some(run);
                        }
                        match t {
                            Terrain::NetLane | Terrain::NetGate => return Some(run),
                            Terrain::NetGrid => run.push(q),
                            _ => return None,
                        }
                    }
                    None
                })
                .min_by_key(|run| run.len())
        };
        let Some(line) = doors.into_iter().find_map(run_from) else {
            return;
        };
        for p in line {
            if let Some(t) = ctx.city.tile_mut(K, p) {
                t.terrain = Terrain::NetLane;
            }
            if let Some(st) = ctx.city.stratum_mut(K) {
                st.steady.set(p, NET_LANE_LIGHT);
            }
        }
    }

    fn ports(ctx: &mut GenCtx) {
        let street = StratumKind::Street;
        let mut jacks = Vec::new();
        if let Some(st) = ctx.city.stratum(street) {
            for (p, t) in st.tiles.iter() {
                if t.feature == Some(Feature::Terminal) && ctx.city.net.node_at(street, p).is_some()
                {
                    jacks.push(Connector {
                        kind: ConnectorKind::Jack,
                        pos: p,
                        serves: vec![street, K],
                        public: t.building.is_none(),
                    });
                }
            }
        }
        ctx.city
            .connectors
            .retain(|c| c.kind != ConnectorKind::Jack);
        ctx.city.connectors.extend(jacks);
    }
}

impl StageRunner for CyberspaceStage {
    fn begin(&mut self, ctx: &mut GenCtx) -> Result<(), GenError> {
        let nb = ctx.city.buildings.len();
        let nd = ctx.city.districts.len();
        self.gates = vec![Pos::new(0, 0); nb];
        self.open = None;
        self.backbones = (0..nd)
            .map(|d| backbone_anchor(ctx, DistrictId(d as u32)))
            .collect();
        self.queue.clear();
        self.queue.push_back(Work::Begin);
        self.queue.extend((0..nb).map(Work::Construct));
        self.queue.extend((0..nb).map(Work::Gate));
        self.queue.extend((0..nd).map(Work::Hub));
        self.queue.push_back(Work::Nodes);
        self.queue
            .extend((0..ctx.city.net.links.len()).map(Work::Lane));
        self.queue.push_back(Work::Site);
        self.queue.push_back(Work::Ports);
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
            Work::Nodes => self.nodes(ctx),
            Work::Lane(i) => self.lane(ctx, i),
            Work::Site => Self::site(ctx),
            Work::Ports => Self::ports(ctx),
        }
        let done = self.total - self.queue.len();
        Ok(StepOut::Continue(done as f32 / self.total as f32))
    }
}

#[cfg(test)]
pub(crate) mod tests {
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
            let light = net.steady[fp.center()];
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

    #[test]
    fn every_node_stands_on_its_feature_and_ice_guards_the_gate() {
        let c = city();
        let net = c.stratum(StratumKind::Net).unwrap();
        for n in &c.net.nodes {
            assert_eq!(
                net.tiles[n.cyber].feature,
                Some(crate::tile::Feature::Node(n.kind)),
                "node {:?} ({:?}) at {:?}",
                n.id,
                n.kind,
                n.cyber
            );
        }
        let mut ice = 0;
        for n in c
            .net
            .nodes
            .iter()
            .filter(|n| n.kind == crate::net::NodeKind::Ice)
        {
            let Some(b) = n.building else { continue };
            ice += 1;
            let fp = c.building(b).footprint;
            let gate = fp
                .border()
                .find(|p| net.tiles[*p].terrain == Terrain::NetGate)
                .unwrap();
            assert!(
                n.cyber.chebyshev(gate) <= 1,
                "ICE {:?} at {:?} is not at gate {gate:?}",
                n.id,
                n.cyber
            );
        }
        assert!(ice >= 1, "fixture has no ICE");
    }

    #[test]
    fn lanes_link_every_construct_gate_to_its_backbone() {
        let c = city();
        let net = c.stratum(StratumKind::Net).unwrap();
        assert!(net.tiles.count(|t| t.terrain == Terrain::NetLane) > 20);
        for b in &c.buildings {
            let gate = b
                .footprint
                .border()
                .find(|p| net.tiles[*p].terrain == Terrain::NetGate)
                .unwrap();
            let backbone = c
                .net
                .nodes
                .iter()
                .find(|n| {
                    n.kind == crate::net::NodeKind::Backbone
                        && c.net.subnet(n.subnet).owner
                            == crate::net::SubnetOwner::District(b.district)
                })
                .unwrap();
            let reach = crate::pathing::flood_fill(c.size, gate, |p| {
                net.tiles.get(p).is_some_and(|t| t.is_walkable())
            });
            assert!(
                reach.contains(&backbone.cyber),
                "gate {gate:?} of {:?} cannot reach backbone {:?}",
                b.id,
                backbone.cyber
            );
        }
    }

    #[test]
    fn street_terminals_with_nodes_become_jacks() {
        let c = city();
        let street = StratumKind::Street;
        let jacks: Vec<_> = c
            .connectors
            .iter()
            .filter(|k| k.kind == crate::world::ConnectorKind::Jack)
            .collect();
        assert!(jacks.len() >= 3, "only {} jacks", jacks.len());
        for j in &jacks {
            assert_eq!(j.serves, vec![street, StratumKind::Net]);
            let t = c.tile(street, j.pos).unwrap();
            assert_eq!(t.feature, Some(crate::tile::Feature::Terminal));
            assert_eq!(j.public, t.building.is_none());
            assert!(
                c.walkable(StratumKind::Net, j.pos),
                "port {:?} not walkable",
                j.pos
            );
            assert!(j.pos.neighbors8().any(|n| c.walkable(street, n)));
        }
        for (p, t) in c.stratum(street).unwrap().tiles.iter() {
            if t.feature == Some(crate::tile::Feature::Terminal)
                && c.net.node_at(street, p).is_some()
            {
                assert!(
                    jacks.iter().any(|j| j.pos == p),
                    "terminal {p:?} has no jack"
                );
            }
        }
        assert!(
            jacks.iter().any(|j| !j.public),
            "fixture has no private port"
        );
    }

    #[test]
    fn every_jack_reaches_the_open_grid() {
        let c = city();
        let net = c.stratum(StratumKind::Net).unwrap();
        for j in c
            .connectors
            .iter()
            .filter(|k| k.kind == crate::world::ConnectorKind::Jack)
        {
            let reach = crate::pathing::flood_fill(c.size, j.pos, |p| {
                net.tiles.get(p).is_some_and(|t| t.is_walkable())
            });
            assert!(
                reach
                    .iter()
                    .any(|p| net.tiles[*p].terrain == Terrain::NetGrid
                        && net.tiles[*p].building.is_none()),
                "port {:?} is sealed in",
                j.pos
            );
        }
    }

    #[test]
    fn gates_open_onto_walkable_tiles() {
        for seed in [6u64, 11, 29] {
            let c = Generator::run(GenParams {
                seed,
                size: SizePreset::Custom { w: 80, h: 40 },
                districts: 4,
                ..Default::default()
            })
            .unwrap();
            let net = c.stratum(StratumKind::Net).unwrap();
            for b in &c.buildings {
                let fp = b.footprint;
                let gate = fp
                    .border()
                    .find(|p| net.tiles[*p].terrain == Terrain::NetGate)
                    .unwrap();
                assert!(
                    gate.neighbors4().any(|q| {
                        !fp.contains(q) && net.tiles.get(q).is_some_and(|t| t.is_walkable())
                    }),
                    "seed {seed}: gate {gate:?} of {:?} is sealed",
                    b.id
                );
            }
        }
    }

    #[test]
    fn lane_search_is_bounded_to_the_endpoints_box() {
        use crate::geom::Size;
        let size = Size { w: 200, h: 60 };
        // A wall across x = 50 whose only opening, y = 58, lies far outside the margin.
        let walled = |_: Pos, q: Pos| {
            if q.x == 50 && q.y != 58 {
                None
            } else {
                Some(1u32)
            }
        };
        let (a, b) = (Pos::new(40, 10), Pos::new(60, 10));
        assert!(
            crate::pathing::astar(size, a, b, walled).is_some(),
            "unbounded search finds the detour"
        );
        assert!(
            super::bounded_astar(size, a, b, 16, walled).is_none(),
            "bounded search gives up"
        );
        let free = |_: Pos, _: Pos| Some(1u32);
        let path = super::bounded_astar(size, a, b, 16, free).unwrap();
        assert_eq!((path[0], *path.last().unwrap()), (a, b));
    }

    fn gate_of(c: &crate::world::City, b: crate::ids::BuildingId) -> Pos {
        let net = c.stratum(StratumKind::Net).unwrap();
        c.building(b)
            .footprint
            .border()
            .find(|p| net.tiles[*p].terrain == Terrain::NetGate)
            .expect("every construct has a gate")
    }

    #[test]
    fn every_district_hub_is_lit_by_a_lane() {
        for (seed, size) in [
            (1u64, SizePreset::Small),
            (6, SizePreset::Custom { w: 120, h: 60 }),
        ] {
            let c = Generator::run(GenParams {
                seed,
                size,
                ..Default::default()
            })
            .unwrap();
            let net = c.stratum(StratumKind::Net).unwrap();
            for n in c
                .net
                .nodes
                .iter()
                .filter(|n| n.kind == crate::net::NodeKind::Backbone)
            {
                // A backbone inside a construct is reached through that construct's gate.
                let terminal = match net.tiles[n.cyber].building {
                    Some(b) => gate_of(&c, b),
                    None => n.cyber,
                };
                assert!(
                    terminal.neighbors4().any(|q| net
                        .tiles
                        .get(q)
                        .is_some_and(|t| t.terrain == Terrain::NetLane)),
                    "seed {seed}: backbone {:?} at {:?} (terminal {terminal:?}) has no lane",
                    n.id,
                    n.cyber
                );
            }
        }
    }

    #[test]
    fn gates_open_onto_the_main_grid() {
        let c = Generator::run(GenParams {
            seed: 3,
            size: SizePreset::Medium,
            ..Default::default()
        })
        .unwrap();
        let net = c.stratum(StratumKind::Net).unwrap();
        let comps = crate::pathing::components(c.size, |p| {
            net.tiles
                .get(p)
                .is_some_and(|t| t.building.is_none() && t.is_walkable())
        });
        let main: std::collections::HashSet<Pos> = comps
            .iter()
            .max_by_key(|c| c.len())
            .unwrap()
            .iter()
            .copied()
            .collect();
        for b in &c.buildings {
            let gate = gate_of(&c, b.id);
            let out = gate
                .neighbors4()
                .find(|q| !b.footprint.contains(*q))
                .unwrap();
            assert!(
                main.contains(&out),
                "gate {gate:?} of {:?} opens into a pocket at {out:?}",
                b.id
            );
        }
    }
}
