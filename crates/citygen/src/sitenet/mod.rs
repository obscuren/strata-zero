//! A site's own Net, worked out from its plan (and a copy's mission layer) whenever it is
//! needed, never by chance: its nodes, the riser its hubs stand beside, the server, the
//! data vault, an air gap, and the firewall on the city's Net that fronts it. Everything
//! here is worked out from the city and the site's plan as they are, so a city from an old
//! save gets it too, and the same plan always gives the same Net.

mod build;
pub mod firewall;
mod riser;

pub use build::*;

use crate::geom::{Pos, Size};
use crate::ids::BuildingId;
use crate::interior::venue::{
    DeviceKind, LevelLayout, MissionLayer, PlannedRoom, VenueLayout, VenueType, Ways, draw_level,
    line_switches, room_label, terminals,
};
use crate::interior::{DoorLock, FloorPlan, LinkKind, RoomKind};
use crate::net::{NodeKind, SiteNode};
use crate::world::City;
use std::collections::BTreeSet;

/// A copy at this security or more cuts its objective's level off: an air gap.
pub const GAP_SECURITY: u8 = 160;
/// The plain building keeps a data vault in its first server room from this security.
pub const VAULT_SECURITY: u8 = 230;

/// A level's nodes by kind, in the Net's order.
pub const KIND_ORDER: [NodeKind; 11] = [
    NodeKind::Hub,
    NodeKind::Server,
    NodeKind::DataVault,
    NodeKind::Firewall,
    NodeKind::Terminal,
    NodeKind::GateSwitch,
    NodeKind::DoorLock,
    NodeKind::LiftCtrl,
    NodeKind::Camera,
    NodeKind::Turret,
    NodeKind::Container,
];

/// A site's Net: a level for each level the plan keeps, each with its hub; the nodes of
/// every device on them; the riser joining the hubs; and the firewall's locks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteNet {
    pub building: BuildingId,
    pub security: u8,
    /// The kept levels, lowest first.
    pub levels: Vec<SiteLevel>,
    /// Every node: the firewall's locks in lock order, then by level, kind
    /// (`KIND_ORDER`) and tile (y, x).
    pub nodes: Vec<SiteNodeInfo>,
    /// The link the riser follows, by index into the plan's links.
    pub riser: Option<usize>,
    /// The level an air gap cuts off.
    pub gap: Option<i32>,
    /// Each lock: its tile on the city's Net, its tile on G's outer wall. The gate first.
    pub firewall: Vec<(Pos, Pos)>,
}

/// A level of a site's Net.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteLevel {
    pub level: i32,
    /// The level's role: "guest floor".
    pub name: String,
    /// The floor's size.
    pub size: Size,
    /// The riser's shaft on this level (a lift bank's first tile); with no riser, the
    /// floor's centre.
    pub shaft: Pos,
    pub hub: Pos,
}

/// A node of a site's Net.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteNodeInfo {
    pub node: SiteNode,
    pub kind: NodeKind,
    /// Its tile on the Net, the level's frame: its device's own tile, but the floor tile in
    /// front of a device set in a wall, and a firewall lock's tile on G's outer wall.
    pub net: Pos,
    pub name: String,
}

/// The level a copy's air gap cuts off at `security`: its objective's at `GAP_SECURITY`
/// or more, never G.
fn gap_at(layer: Option<&MissionLayer>, security: u8) -> Option<i32> {
    let level = layer?.objective.level;
    (security >= GAP_SECURITY && level != 0).then_some(level)
}

/// A copy's level an air gap cuts off: its objective's at `GAP_SECURITY` or more, never G.
pub fn gapped_level(layer: Option<&MissionLayer>) -> Option<i32> {
    gap_at(layer, layer?.inputs.security)
}

/// The room on `x` holding `at`: the one it is a spot of, else whose floor holds it.
fn room_at(x: &LevelLayout, at: Pos) -> Option<&PlannedRoom> {
    (x.rooms.iter().find(|r| r.spots.contains(&at)))
        .or_else(|| x.rooms.iter().find(|r| r.rect.contains(at)))
}

impl SiteNet {
    /// The Net of a venue's plan (cut to a copy's levels) with the copy's layer, at
    /// `security`, which sets the firewall's locks, the data vault and the air gap; None
    /// for a plan with no venue.
    pub fn of(
        city: &City,
        plan: &FloorPlan,
        t: Option<&VenueType>,
        layer: Option<&MissionLayer>,
        security: u8,
    ) -> Option<SiteNet> {
        let v = plan.venue.as_deref()?;
        let building = plan.building;
        let gap = gap_at(layer, security).filter(|l| plan.has_level(*l));
        let riser = riser::riser_link(plan);
        let mut net = SiteNet {
            building,
            security,
            levels: Vec::new(),
            nodes: Vec::new(),
            riser,
            gap,
            firewall: Vec::new(),
        };
        let mut nodes: Vec<SiteNodeInfo> = Vec::new();
        let mut push = |level: i32, at: Pos, kind: NodeKind, net: Pos, name: String| {
            nodes.push(SiteNodeInfo {
                node: SiteNode {
                    building,
                    level,
                    at,
                },
                kind,
                net,
                name,
            });
        };
        for level in plan.levels() {
            let (Some(x), Some(floor)) = (v.level(level), draw_level(v, level)) else {
                continue;
            };
            let found = terminals(v, t, level, &floor, gap == Some(level));
            let switches = line_switches(v, level);
            let fronts: BTreeSet<Pos> = (found.iter().map(|d| d.front))
                .chain(switches.iter().map(|s| s.front))
                .collect();
            let bank = match riser {
                Some(r) => riser::shaft(plan, r, level),
                None => vec![floor.tiles.size().center()],
            };
            let hub = riser::hub_tile(&floor, x, &bank, &fronts);
            let label = FloorPlan::label(level);
            push(level, hub, NodeKind::Hub, hub, format!("{label} hub"));
            if level == 0 {
                let server = riser::server_tile(&floor, x, hub, &fronts);
                push(level, server, NodeKind::Server, server, "server".into());
                if v.frame.building == Some(building) && city.building_opt(building).is_some() {
                    let out = firewall::outside(city, building, firewall::lock_count(security));
                    let inside = firewall::inside(plan.footprint, &floor, &out);
                    net.firewall = out.into_iter().zip(inside).collect();
                }
            }
            for d in &found {
                let what = if d.public { "kiosk" } else { "terminal" };
                let room = &x.rooms[d.room].name;
                push(
                    level,
                    d.at,
                    NodeKind::Terminal,
                    d.front,
                    format!("{room} {what}"),
                );
            }
            for s in &switches {
                let name = x.pieces[s.piece].name.clone();
                push(level, s.at, NodeKind::GateSwitch, s.front, name);
            }
            for (at, name) in door_locks(v, x, layer) {
                push(level, at, NodeKind::DoorLock, at, name);
            }
            net.levels.push(SiteLevel {
                level,
                name: x.role.clone(),
                size: floor.tiles.size(),
                shaft: bank.first().copied().unwrap_or(hub),
                hub,
            });
        }
        for (i, &(_, inside)) in net.firewall.iter().enumerate() {
            let lock = SiteNode::firewall(building, i as u8);
            push(
                lock.level,
                lock.at,
                NodeKind::Firewall,
                inside,
                format!("firewall lock {}", i + 1),
            );
        }
        // A lift's node stands at its stop on the lowest level it serves on the riser.
        for k in plan.links.iter().filter(|k| k.kind == LinkKind::Lift) {
            if let Some(s) = k.stops.iter().find(|s| net.on_riser(s.level)) {
                let name = k.name.clone().unwrap_or_else(|| "lift".into());
                push(s.level, s.at, NodeKind::LiftCtrl, s.at, name);
            }
        }
        if let Some((level, at)) = vault(v, plan, layer, security) {
            push(level, at, NodeKind::DataVault, at, "data vault".into());
        }
        if let Some(layer) = layer {
            let devices = (layer.devices.iter()).map(|d| {
                let kind = match d.kind {
                    DeviceKind::Camera => NodeKind::Camera,
                    DeviceKind::Turret => NodeKind::Turret,
                };
                (d.level, d.at, kind)
            });
            let containers =
                (layer.containers.iter()).map(|c| (c.level, c.at, NodeKind::Container));
            for (level, at, kind) in devices.chain(containers) {
                let Some(x) = v.level(level).filter(|_| plan.has_level(level)) else {
                    continue;
                };
                let what = match kind {
                    NodeKind::Container => "safe",
                    other => other.label(),
                };
                let name = match room_at(x, at) {
                    Some(room) => format!("{} {what}", room.name),
                    None => what.to_string(),
                };
                push(level, at, kind, at, name);
            }
        }
        let rank = |k: NodeKind| {
            KIND_ORDER
                .iter()
                .position(|o| *o == k)
                .unwrap_or(usize::MAX)
        };
        nodes.sort_by_key(|n| (n.node.level, rank(n.kind), n.node.at.y, n.node.at.x));
        // One node to a tile (`door_locks` keeps one to a door): should two devices ever
        // share one, the first in the Net's order keeps it.
        let mut seen: BTreeSet<SiteNode> = BTreeSet::new();
        nodes.retain(|n| seen.insert(n.node));
        net.nodes = nodes;
        Some(net)
    }

    pub fn node(&self, n: SiteNode) -> Option<&SiteNodeInfo> {
        self.nodes.iter().find(|i| i.node == n)
    }

    pub fn level(&self, level: i32) -> Option<&SiteLevel> {
        self.levels.iter().find(|l| l.level == level)
    }

    /// Whether `level` is a kept level whose hub hangs off the riser: every one but the
    /// gapped level.
    pub fn on_riser(&self, level: i32) -> bool {
        self.level(level).is_some() && self.gap != Some(level)
    }

    /// From `from`, `to` can be reached on the Net: the same level, or both on the riser.
    pub fn reaches(&self, from: i32, to: i32) -> bool {
        (from == to && self.level(from).is_some()) || (self.on_riser(from) && self.on_riser(to))
    }
}

/// The door locks of level `x` the Net opens, each by its door tile and name, as the venue
/// graph holds them: a door's lock (locked in the plain building, or fitted by the copy),
/// else on a private room's door its keypad; then the ways in with a lock. Each the Net
/// opens and no gate line holds (its switch works the line); one to a tile.
fn door_locks(
    v: &VenueLayout,
    x: &LevelLayout,
    layer: Option<&MissionLayer>,
) -> Vec<(Pos, String)> {
    let level = x.level;
    let fitted = |k: usize| layer.is_some_and(|l| l.fitted.contains(&(level, k)));
    let lock = |l: DoorLock| {
        let zone = v.zones.get(usize::from(l.zone));
        let zone = zone.map_or("locked", String::as_str);
        (l.at, l.ways, format!("{zone} door"))
    };
    let mut found: Vec<(Pos, Ways, String)> = Vec::new();
    for (k, d) in x.doors.iter().enumerate() {
        if let Some(l) = d.lock.or(d.fit.filter(|_| fitted(k))) {
            found.push(lock(l));
            continue;
        }
        // A private room's keypad: the Net opens it, as it does a code or a keyjack.
        let private = [d.rooms.0, d.rooms.1]
            .into_iter()
            .find(|&r| x.rooms[r].private);
        if let Some(r) = private.map(|r| &x.rooms[r]).filter(|_| !d.open) {
            let name = match r.number {
                0 => r.name.clone(),
                n => format!("{} {}", r.name, room_label(n)),
            };
            found.push((d.at, Ways::NET, name));
        }
    }
    let ways_in = v.ways_in.iter().filter(|w| w.level == level);
    found.extend(ways_in.filter_map(|w| w.lock).map(lock));
    let mut out: Vec<(Pos, String)> = Vec::new();
    for (at, ways, name) in found {
        if ways.has(Ways::NET)
            && v.gate_line_at(level, at).is_none()
            && !out.iter().any(|o| o.0 == at)
        {
            out.push((at, name));
        }
    }
    out
}

/// The data vault: a copy's objective spot when the objective stands in a server room; in
/// the plain building at `VAULT_SECURITY` or more, the first server room of its levels
/// (lowest first, rooms in order) at its first spot, else its centre.
fn vault(
    v: &VenueLayout,
    plan: &FloorPlan,
    layer: Option<&MissionLayer>,
    security: u8,
) -> Option<(i32, Pos)> {
    if let Some(layer) = layer {
        let o = layer.objective;
        let x = v.level(o.level).filter(|_| plan.has_level(o.level))?;
        return (room_at(x, o.at)?.kind == RoomKind::Server).then_some((o.level, o.at));
    }
    if security < VAULT_SECURITY {
        return None;
    }
    plan.levels().filter_map(|l| v.level(l)).find_map(|x| {
        let r = x.rooms.iter().find(|r| r.kind == RoomKind::Server)?;
        Some((x.level, r.spots.first().copied().unwrap_or(r.rect.center())))
    })
}

#[cfg(test)]
mod tests;
