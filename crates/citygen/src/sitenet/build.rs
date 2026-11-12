//! A site's construct: one grid holding every kept level one under another, the highest at
//! the top, each its floor tile for tile; the nodes on their devices' tiles, the traces
//! from each to its level's hub, and the ICE the site's security sets. The rulings cited
//! are the site-net plan's, `docs/superpowers/plans/2026-11-11-site-net.md`.

use super::{SiteLevel, SiteNet, firewall};
use crate::construct::{
    BLACK_THRESHOLD, Construct, GREY_THRESHOLD, Guard, ICE_THRESHOLD, NetDevice, NetLevel,
};
use crate::geom::{Grid, Pos, Rect, Size};
use crate::interior::venue::{
    MissionLayer, VenueLayout, WayInKind, Ways, draw_copy_level, draw_level,
};
use crate::interior::{Floor, FloorPlan};
use crate::lighting::tuning::NET_LANE_LIGHT;
use crate::net::{NodeKind, SiteNode};
use crate::rng::tile_hash;
use crate::tile::{Feature, IceGrade, Terrain, Tile};
use crate::world::{City, StratumKind};
use std::cmp::Reverse;
use std::collections::{BTreeSet, VecDeque};

/// The rows of Net wall between one level's band and the next.
pub const BAND_GAP: i32 = 2;
/// Security from which a white ICE posts beside G's hub.
pub const HUB_ICE: u8 = 100;

/// The construct of a site's Net `net`, worked out from its plan (cut to a copy's levels)
/// and the copy's layer: each kept level's floor as it is drawn, its walls Net walls and
/// its floor Net floor, a door by its lock (Ruling 4); each node on its tile; the gate,
/// lock 0's tile on G's outer wall, its entry; and ICE by security (Ruling 9).
pub fn site_construct(
    city: &City,
    net: &SiteNet,
    plan: &FloorPlan,
    layer: Option<&MissionLayer>,
) -> Construct {
    let b = net.building;
    let security = net.security;
    let v = plan.venue.as_deref();
    // The kept levels that draw, the highest first; a copy's floors with its fitted locks.
    let floors: Vec<(&SiteLevel, Floor)> = (net.levels.iter().rev())
        .filter_map(|l| {
            let v = v?;
            let floor = layer
                .and_then(|layer| draw_copy_level(v, layer, l.level))
                .or_else(|| draw_level(v, l.level))?;
            Some((l, floor))
        })
        .collect();
    let w = (floors.iter().map(|(_, f)| f.tiles.size().w))
        .max()
        .unwrap_or(1);
    let heights: i32 = floors.iter().map(|(_, f)| f.tiles.size().h as i32).sum();
    let gaps = BAND_GAP * (floors.len() as i32 - 1).max(0);
    let size = Size {
        w,
        h: (heights + gaps).max(1) as u32,
    };
    let base = Tile {
        terrain: Terrain::NetWall,
        feature: None,
        district: city.building_opt(b).map(|x| x.district),
        building: Some(b),
        variant: 0,
    };
    let mut tiles = Grid::filled(size, base);
    for p in size.rect().iter() {
        tiles[p].variant = tile_hash(city.seed, StratumKind::Net, p);
    }
    let glow = 96u8.saturating_add((security as u32 * 5 / 8) as u8);
    let mut levels = Vec::new();
    let mut top = 0;
    for (l, floor) in &floors {
        let fs = floor.tiles.size();
        for (p, _) in floor.tiles.iter() {
            tiles[p.offset(0, top)].terrain = net_terrain(floor, p);
        }
        levels.push(NetLevel {
            level: l.level,
            name: l.name.clone(),
            area: Rect::new(0, top, fs.w as i32, fs.h as i32),
            hub: l.hub.offset(0, top),
            gapped: net.gap == Some(l.level),
            traces: Vec::new(),
            passed: v.and_then(|v| passed(v, net, l.level)),
        });
        top += fs.h as i32 + BAND_GAP;
    }
    let mut k = Construct {
        building: b,
        security,
        size,
        tiles,
        light: Grid::filled(size, glow),
        entry: Pos::new(0, 0),
        rooms: Vec::new(),
        nodes: Vec::new(),
        guards: Vec::new(),
        levels,
        devices: Vec::new(),
    };
    // Each node on its tile: a firewall lock on G's outer wall. One to a tile, the first
    // in the Net's order keeping it, as the Net keeps one node to a tile.
    for n in &net.nodes {
        let level = match n.node.firewall_lock() {
            Some(_) => 0,
            None => n.node.level,
        };
        let Some(at) = k.on_level(level, n.net) else {
            continue;
        };
        if k.device_at(at).is_some() {
            continue;
        }
        k.tiles[at].feature = Some(Feature::Node(n.kind));
        k.devices.push(NetDevice {
            node: n.node,
            kind: n.kind,
            at,
            name: n.name.clone(),
        });
    }
    let gate = match k.device(SiteNode::firewall(b, 0)) {
        Some(d) => d.at,
        None => lone_gate(&k, plan, &floors),
    };
    k.entry = gate;
    k.tiles[gate].terrain = Terrain::NetGate;
    k.light[gate] = NET_LANE_LIGHT;
    for i in 0..k.levels.len() {
        k.levels[i].traces = traces(&k, &k.levels[i]);
    }
    k.guards = ice(&k, &floors);
    for g in &k.guards {
        k.tiles[g.pos].feature = Some(Feature::Ice(g.grade));
    }
    k
}

/// A floor tile's terrain on the Net (Ruling 4): walls are Net wall and the floor, its
/// landings, stairs, escalators, ladders and track are Net floor; a door to the city is
/// Net wall (its lock's node stands on it when the lock opens to the Net); a door whose
/// lock doesn't open to the Net is Net wall; any other door, a turnstile too, Net floor.
fn net_terrain(floor: &Floor, p: Pos) -> Terrain {
    if floor.exit_at(p).is_some() {
        return Terrain::NetWall;
    }
    if floor.lock_at(p).is_some_and(|l| !l.ways.has(Ways::NET)) {
        return Terrain::NetWall;
    }
    match floor.tiles[p].terrain {
        Terrain::Wall | Terrain::Window => Terrain::NetWall,
        t if Tile::new(t).is_walkable() => Terrain::NetFloor,
        _ => Terrain::NetWall,
    }
}

/// The levels of the layout the copy drops between `level` and the next kept level below
/// it, lowest and highest; None when it drops none, or none is kept below.
fn passed(v: &VenueLayout, net: &SiteNet, level: i32) -> Option<(i32, i32)> {
    let below = (net.levels.iter().map(|l| l.level))
        .filter(|l| *l < level)
        .max()?;
    let between = || (v.levels.iter().map(|x| x.level)).filter(move |l| below < *l && *l < level);
    Some((between().min()?, between().max()?))
}

/// The gate inside with no lock on the city's Net to put it at (a building with no gate
/// there): where lock 0's would stand by the level's first door to the city, on G's outer
/// wall, else on the lowest level's.
fn lone_gate(k: &Construct, plan: &FloorPlan, floors: &[(&SiteLevel, Floor)]) -> Pos {
    let found = (floors.iter().find(|(l, _)| l.level == 0)).or(floors.last());
    let Some((l, floor)) = found else {
        return Pos::new(0, 0);
    };
    let door = (plan.venue.iter().flat_map(|v| v.ways_in.iter()))
        .filter(|w| w.level == l.level)
        .find_map(|w| match w.kind {
            WayInKind::Door { door, .. } => Some(door),
            WayInKind::Crawl { .. } => None,
        })
        .unwrap_or(plan.footprint.center());
    let at = (firewall::inside(plan.footprint, floor, &[door]).first())
        .copied()
        .unwrap_or(Pos::new(0, floor.tiles.size().h as i32 / 2));
    k.on_level(l.level, at).unwrap_or(Pos::new(0, 0))
}

/// A breadth-first search from one tile over the tiles `open` takes, inside an area.
struct Search {
    area: Rect,
    start: Pos,
    /// Each tile's steps from `start`, `u32::MAX` unreached.
    dist: Vec<u32>,
    came: Vec<Option<Pos>>,
}

impl Search {
    /// From `start` (taken whether `open` takes it or not), 4-steps in `Dir::CARDINAL`
    /// order, so the same grid always gives the same paths.
    fn new(area: Rect, start: Pos, open: impl Fn(Pos) -> bool) -> Search {
        let mut s = Search {
            area,
            start,
            dist: vec![u32::MAX; area.area()],
            came: vec![None; area.area()],
        };
        let Some(i) = s.index(start) else {
            return s;
        };
        s.dist[i] = 0;
        let mut queue = VecDeque::from([start]);
        while let Some(p) = queue.pop_front() {
            let d = s.dist[s.index(p).expect("queued in the area")];
            for n in p.neighbors4() {
                let Some(j) = s.index(n) else {
                    continue;
                };
                if s.dist[j] == u32::MAX && open(n) {
                    s.dist[j] = d + 1;
                    s.came[j] = Some(p);
                    queue.push_back(n);
                }
            }
        }
        s
    }

    fn index(&self, p: Pos) -> Option<usize> {
        let a = self.area;
        a.contains(p)
            .then(|| ((p.y - a.y) * a.w + (p.x - a.x)) as usize)
    }

    fn dist(&self, p: Pos) -> Option<u32> {
        self.index(p)
            .map(|i| self.dist[i])
            .filter(|d| *d != u32::MAX)
    }

    fn reached(&self) -> impl Iterator<Item = (Pos, u32)> + '_ {
        (self.area.iter()).filter_map(|p| self.dist(p).map(|d| (p, d)))
    }

    /// The shortest path from the start to `to`, both ends; empty when it isn't reached.
    fn path_to(&self, to: Pos) -> Vec<Pos> {
        if self.dist(to).is_none() {
            return Vec::new();
        }
        let mut path = vec![to];
        let mut at = to;
        while at != self.start {
            at = self.came[self.index(at).expect("reached")].expect("a step back");
            path.push(at);
        }
        path.reverse();
        path
    }
}

/// A level's traces: for each of its devices but the hub, the shortest 4-step path over
/// the level's open tiles (doors among them) from the device to the hub, both ends; a
/// device in the wall from the open tile beside it nearest the hub. Their union, in
/// (y, x) order.
fn traces(k: &Construct, l: &NetLevel) -> Vec<Pos> {
    let open = |p: Pos| k.tiles[p].terrain != Terrain::NetWall;
    let from_hub = Search::new(l.area, l.hub, open);
    let mut out: BTreeSet<(i32, i32)> = BTreeSet::new();
    for d in (k.devices.iter()).filter(|d| l.area.contains(d.at) && d.kind != NodeKind::Hub) {
        let via = match from_hub.dist(d.at) {
            Some(_) => Some(d.at),
            None => (d.at.neighbors4())
                .filter_map(|q| from_hub.dist(q).map(|n| (n, q.y, q.x)))
                .min()
                .map(|(_, y, x)| Pos::new(x, y)),
        };
        let Some(via) = via else {
            continue;
        };
        out.insert((d.at.y, d.at.x));
        out.extend(from_hub.path_to(via).into_iter().map(|p| (p.y, p.x)));
    }
    out.into_iter().map(|(y, x)| Pos::new(x, y)).collect()
}

/// The free tile nearest `at` on level `l` by steps over its Net floor (ties: y, x): Net
/// floor with no node on it and none of `taken`.
fn beside(k: &Construct, l: &NetLevel, at: Pos, taken: &BTreeSet<Pos>) -> Option<Pos> {
    let free = |p: Pos| k.tiles[p].terrain == Terrain::NetFloor && !taken.contains(&p);
    let s = Search::new(l.area, at, free);
    (s.reached())
        .filter(|(p, _)| *p != at && k.device_at(*p).is_none())
        .min_by_key(|(p, d)| (*d, p.y, p.x))
        .map(|(p, _)| p)
}

/// A patrol's beat on level `l` from `start` (Ruling 9): through the tile nearest the
/// centre of each of the level's rooms that isn't private, in room order, by shortest
/// 4-step paths over its Net floor clear of `taken` (the pieces that keep their posts);
/// with fewer than 2 such rooms to reach, to its farthest such tile.
fn beat(k: &Construct, l: &NetLevel, floor: &Floor, start: Pos, taken: &BTreeSet<Pos>) -> Vec<Pos> {
    let open = |p: Pos| k.tiles[p].terrain == Terrain::NetFloor && !taken.contains(&p);
    let from = Search::new(l.area, start, open);
    let origin = |p: Pos| p.offset(l.area.x, l.area.y);
    let mut stops: Vec<Pos> = (floor.rooms.iter().filter(|r| !r.private))
        .filter_map(|r| {
            let centre = origin(r.rect.center());
            (r.rect.iter().map(origin))
                .filter(|p| from.dist(*p).is_some())
                .min_by_key(|p| (p.manhattan(centre), p.y, p.x))
        })
        .collect();
    if stops.len() < 2 {
        stops = (from.reached())
            .max_by_key(|(p, d)| (*d, Reverse(p.y), Reverse(p.x)))
            .map(|(p, _)| p)
            .into_iter()
            .collect();
    }
    let mut out = vec![start];
    let mut at = start;
    for stop in stops {
        if stop == at {
            continue;
        }
        let leg = Search::new(l.area, at, open).path_to(stop);
        out.extend(leg.into_iter().skip(1));
        at = stop;
    }
    out
}

/// A site's ICE as its card and a scan word it: the strongest piece on G's level, the
/// piece beside the data vault, and the strongest anywhere on its Net.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SiteIce {
    pub g: Option<IceGrade>,
    pub vault: Option<IceGrade>,
    pub top: Option<IceGrade>,
}

/// What `ice` posts at `security`, before a construct is made: on G's level the white
/// beside its hub from `HUB_ICE`, the grey patrolling it from `GREY_THRESHOLD`; beside the
/// data vault, on a site with one, the black from `BLACK_THRESHOLD`. The whites patrolling
/// other levels from `ICE_THRESHOLD` are never stronger than G's.
pub fn table_ice(security: u8, vault: bool) -> SiteIce {
    let g = if security >= GREY_THRESHOLD {
        Some(IceGrade::Grey)
    } else if security >= HUB_ICE {
        Some(IceGrade::White)
    } else {
        None
    };
    let vault = (vault && security >= BLACK_THRESHOLD).then_some(IceGrade::Black);
    SiteIce {
        g,
        vault,
        top: vault.or(g),
    }
}

/// How hard a grade bites, for picking the strongest.
fn rank(g: IceGrade) -> u8 {
    match g {
        IceGrade::White => 0,
        IceGrade::Grey => 1,
        IceGrade::Black => 2,
    }
}

/// The strongest ICE posted on `level` of site construct `k`, lifted or standing.
pub fn ice_on(k: &Construct, level: i32) -> Option<IceGrade> {
    (k.guards.iter())
        .filter(|g| !g.inert && k.level_at(g.post).is_some_and(|l| l.level == level))
        .map(|g| g.grade)
        .max_by_key(|g| rank(*g))
}

/// The ICE site construct `k` posts, read off its pieces (`table_ice` before it's made).
pub fn posted_ice(k: &Construct) -> SiteIce {
    let strongest = |near: Option<Pos>| {
        (k.guards.iter())
            .filter(|g| !g.inert && near.is_none_or(|at| g.post.chebyshev(at) <= 1))
            .map(|g| g.grade)
            .max_by_key(|g| rank(*g))
    };
    let vault = (k.devices.iter()).find(|d| d.kind == NodeKind::DataVault);
    SiteIce {
        g: ice_on(k, 0),
        vault: vault.and_then(|d| strongest(Some(d.at))),
        top: strongest(None),
    }
}

/// The site's ICE by its security (Ruling 9): from `HUB_ICE` a white posted beside G's
/// hub; from `ICE_THRESHOLD` a white patrolling each kept level but G at an odd place in
/// the list lowest first; from `GREY_THRESHOLD` a grey patrolling every kept level
/// instead; from `BLACK_THRESHOLD` a black posted beside the data vault. The white at G
/// first, then the patrols lowest level first, then the black. Only the posted pieces
/// keep their posts, and the patrols walk round them.
fn ice(k: &Construct, floors: &[(&SiteLevel, Floor)]) -> Vec<Guard> {
    let s = k.security;
    let mut taken: BTreeSet<Pos> = BTreeSet::new();
    let piece = |at: Pos, grade: IceGrade, beat: Vec<Pos>| Guard {
        post: at,
        pos: at,
        grade,
        beat,
        standing: true,
        inert: false,
    };
    let white = (k.net_level(0).filter(|_| s >= HUB_ICE)).and_then(|g| beside(k, g, g.hub, &taken));
    taken.extend(white);
    let vault = (k.devices.iter()).find(|d| d.kind == NodeKind::DataVault && s >= BLACK_THRESHOLD);
    let black = vault.and_then(|d| beside(k, k.level_at(d.at)?, d.at, &taken));
    taken.extend(black);
    // The kept levels, lowest first.
    let kept: Vec<i32> = k.levels.iter().rev().map(|l| l.level).collect();
    let patrols: Vec<(i32, IceGrade)> = if s >= GREY_THRESHOLD {
        kept.iter().map(|&l| (l, IceGrade::Grey)).collect()
    } else if s >= ICE_THRESHOLD {
        (kept.iter().copied().filter(|l| *l != 0))
            .enumerate()
            .filter(|(i, _)| i % 2 == 1)
            .map(|(_, l)| (l, IceGrade::White))
            .collect()
    } else {
        Vec::new()
    };
    let mut guards: Vec<Guard> = white
        .map(|p| piece(p, IceGrade::White, Vec::new()))
        .into_iter()
        .collect();
    for (level, grade) in patrols {
        let (Some(l), Some((_, floor))) = (
            k.net_level(level),
            floors.iter().find(|(f, _)| f.level == level),
        ) else {
            continue;
        };
        let Some(start) = beside(k, l, l.hub, &taken) else {
            continue;
        };
        let walk = beat(k, l, floor, start, &taken);
        taken.insert(start);
        guards.push(piece(start, grade, walk));
    }
    guards.extend(black.map(|p| piece(p, IceGrade::Black, Vec::new())));
    guards
}

/// The rules a site's construct keeps: its tiles are its size; its entry is a gate; no two
/// nodes share a tile; every node can be reached from its level's hub with the pieces
/// lifted (one in the wall from a tile beside it); every piece stands where it could walk
/// on its own level, on no node and no other piece; every beat steps 4-ways on its level.
pub fn validate_site_construct(k: &Construct) -> Result<(), String> {
    if k.tiles.size() != k.size {
        return Err(format!(
            "tiles are {:?}, the construct says {:?}",
            k.tiles.size(),
            k.size
        ));
    }
    if k.tiles
        .get(k.entry)
        .is_none_or(|t| t.terrain != Terrain::NetGate)
    {
        return Err(format!("entry {:?} is not a gate", k.entry));
    }
    let lifted = |p: Pos| {
        k.tiles.get(p).is_some_and(|t| {
            let mut t = *t;
            if matches!(t.feature, Some(Feature::Ice(_))) {
                t.feature = None;
            }
            t.is_walkable()
        })
    };
    for l in &k.levels {
        if l.area.intersection(k.size.rect()) != Some(l.area) || !l.area.contains(l.hub) {
            return Err(format!("level {} stands out of the grid", l.level));
        }
    }
    // Each level's tiles reached from its hub, in the levels' order.
    let reach: Vec<Search> = (k.levels.iter())
        .map(|l| Search::new(l.area, l.hub, lifted))
        .collect();
    for (i, d) in k.devices.iter().enumerate() {
        if k.devices[..i].iter().any(|e| e.at == d.at) {
            return Err(format!("two nodes at {:?}", d.at));
        }
        let Some(reach) = (k.levels.iter().position(|l| l.area.contains(d.at))).map(|j| &reach[j])
        else {
            return Err(format!("{} at {:?} stands in no level", d.name, d.at));
        };
        if reach.dist(d.at).is_none() && !d.at.neighbors4().any(|q| reach.dist(q).is_some()) {
            return Err(format!(
                "{} at {:?} can't be reached from its hub",
                d.name, d.at
            ));
        }
    }
    for (i, g) in k.guards.iter().enumerate() {
        let Some(l) = k.level_at(g.post) else {
            return Err(format!("a piece posted at {:?} stands in no level", g.post));
        };
        if !l.area.contains(g.pos) || !lifted(g.pos) {
            return Err(format!(
                "a piece at {:?} stands off its level's floor",
                g.pos
            ));
        }
        if k.device_at(g.pos).is_some() {
            return Err(format!("a piece stands on a node at {:?}", g.pos));
        }
        if k.guards[..i].iter().any(|h| h.pos == g.pos) {
            return Err(format!("two pieces at {:?}", g.pos));
        }
        if g.beat.windows(2).any(|w| w[0].manhattan(w[1]) != 1) {
            return Err(format!("the beat from {:?} jumps", g.post));
        }
        if let Some(p) = g.beat.iter().find(|p| !l.area.contains(**p)) {
            return Err(format!(
                "the beat from {:?} leaves its level at {p:?}",
                g.post
            ));
        }
    }
    Ok(())
}
