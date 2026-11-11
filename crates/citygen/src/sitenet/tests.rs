use super::*;
use crate::interior::venue::testutil;
use crate::interior::venue::{
    MissionInputs, MissionSize, VenuePack, Ways, draw_copy_level, line_switches, terminals,
};
use crate::interior::{LinkKind, RoomKind};
use std::collections::BTreeSet;
use std::sync::OnceLock;

/// The test cities, made once for every test.
fn cities() -> &'static [City] {
    static CITIES: OnceLock<Vec<City>> = OnceLock::new();
    CITIES.get_or_init(testutil::cities)
}

/// Every builtin type laid on each building of the test cities whose kind it builds: the
/// plain building's plan.
fn plains() -> Vec<(&'static City, FloorPlan, &'static VenueType)> {
    let pack = VenuePack::builtin();
    let mut v = Vec::new();
    for c in cities() {
        for b in c.buildings.iter().filter(|b| b.kind.is_closed()) {
            let Some(t) = pack.for_kind(b.kind) else {
                continue;
            };
            if let Some(p) = FloorPlan::new(c, b.id).filter(|p| p.venue.is_some()) {
                v.push((c, p, t));
            }
        }
    }
    assert!(!v.is_empty(), "no venue in the test cities");
    v
}

/// Each plain building with a Small copy laid at `security`: the first objective and seed
/// in 0..8 that lays, the plan kept to it.
fn copies(security: u8) -> Vec<(&'static City, FloorPlan, &'static VenueType, MissionLayer)> {
    let rule = VenuePack::builtin().size(MissionSize::Small);
    let v: Vec<_> = (plains().into_iter())
        .filter_map(|(c, p, t)| {
            let layout = p.venue.as_deref()?;
            let layer = (0..t.objectives.len()).find_map(|objective| {
                (0..8).find_map(|seed| {
                    let inputs = MissionInputs {
                        size: MissionSize::Small,
                        objective,
                        security,
                        seed,
                    };
                    MissionLayer::lay(layout, t, rule, inputs).ok()
                })
            })?;
            Some((c, p.kept(&layer.kept), t, layer))
        })
        .collect();
    assert!(!v.is_empty(), "no copy lays");
    v
}

#[test]
fn the_same_plan_gives_the_same_net() {
    for (c, p, t, layer) in copies(190) {
        assert_eq!(
            SiteNet::of(c, &p, Some(t), Some(&layer), 190),
            SiteNet::of(c, &p, Some(t), Some(&layer), 190)
        );
    }
}

#[test]
fn a_plan_with_no_venue_has_no_site_net() {
    let (c, p, t) = plains().remove(0);
    let classic = FloorPlan::classic(c, p.building).unwrap();
    assert_eq!(SiteNet::of(c, &classic, Some(t), None, 120), None);
}

#[test]
fn every_net_lock_lift_and_terminal_has_a_node() {
    for (c, p, t, layer) in copies(190) {
        let net = SiteNet::of(c, &p, Some(t), Some(&layer), 190).unwrap();
        let v = p.venue.as_deref().unwrap();
        let has = |kind, level, at| {
            net.nodes
                .iter()
                .any(|n| n.kind == kind && n.node.level == level && n.node.at == at)
        };
        for &level in &p.levels {
            let floor = draw_copy_level(v, &layer, level).unwrap();
            for lock in floor
                .locks
                .iter()
                .filter(|l| l.ways.has(Ways::NET) && v.gate_line_at(level, l.at).is_none())
            {
                assert!(
                    has(NodeKind::DoorLock, level, lock.at),
                    "{} {level} {:?}",
                    t.id,
                    lock.at
                );
            }
            for r in floor.rooms.iter().filter(|r| r.private && r.number > 0) {
                assert!(
                    has(NodeKind::DoorLock, level, r.door.unwrap()),
                    "keypad {}",
                    r.number
                );
            }
            for d in terminals(v, Some(t), level, &floor, net.gap == Some(level)) {
                assert!(has(NodeKind::Terminal, level, d.at));
            }
            for s in line_switches(v, level) {
                assert!(has(NodeKind::GateSwitch, level, s.at));
            }
        }
        let lifts = p.links.iter().filter(|l| l.kind == LinkKind::Lift).count();
        assert_eq!(
            net.nodes
                .iter()
                .filter(|n| n.kind == NodeKind::LiftCtrl)
                .count(),
            lifts
        );
        assert_eq!(
            net.nodes
                .iter()
                .filter(|n| n.kind == NodeKind::Firewall)
                .count(),
            firewall::lock_count(190)
        );
    }
}

#[test]
fn every_hub_stands_beside_the_risers_shaft() {
    for (c, p, t) in plains() {
        let net = SiteNet::of(c, &p, Some(t), None, 120).unwrap();
        let Some(r) = net.riser else { continue };
        for l in &net.levels {
            if let Some(stop) = p.links[r].stop(l.level) {
                let bank = [stop.at, stop.at.offset(1, 0)];
                assert!(
                    bank.iter().any(|b| b.chebyshev(l.hub) <= 1),
                    "{} {} hub {:?} bank {:?}",
                    t.id,
                    l.level,
                    l.hub,
                    stop.at
                );
            }
            assert!(
                net.nodes
                    .iter()
                    .any(|n| n.kind == NodeKind::Hub && n.node.level == l.level && n.net == l.hub)
            );
        }
    }
}

#[test]
fn the_server_stands_beside_gs_hub() {
    for (c, p, t) in plains() {
        let net = SiteNet::of(c, &p, Some(t), None, 120).unwrap();
        let server = net
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Server)
            .unwrap();
        assert_eq!(server.node.level, 0);
        assert!(server.net.chebyshev(net.level(0).unwrap().hub) <= 1);
    }
}

#[test]
fn an_air_gap_cuts_off_its_whole_level() {
    for (c, p, t, layer) in copies(190) {
        let net = SiteNet::of(c, &p, Some(t), Some(&layer), 190).unwrap();
        let want = (layer.objective.level != 0).then_some(layer.objective.level);
        assert_eq!(net.gap, want);
        assert_eq!(gapped_level(Some(&layer)), want);
        if let Some(gap) = net.gap {
            for l in p.levels.iter().filter(|l| **l != gap) {
                assert!(!net.reaches(gap, *l) && !net.reaches(*l, gap));
            }
            assert!(net.reaches(gap, gap));
        }
        let joined: Vec<i32> = p.levels().filter(|l| Some(*l) != net.gap).collect();
        assert!(
            joined
                .iter()
                .all(|a| joined.iter().all(|b| net.reaches(*a, *b)))
        );
    }
    for (c, p, t, layer) in copies(159) {
        assert_eq!(
            SiteNet::of(c, &p, Some(t), Some(&layer), 159).unwrap().gap,
            None
        );
        assert_eq!(gapped_level(Some(&layer)), None);
    }
    assert_eq!(gapped_level(None), None);
}

#[test]
fn the_plain_building_has_no_gap_and_no_mission_devices() {
    for (c, p, t) in plains() {
        let net = SiteNet::of(c, &p, Some(t), None, 200).unwrap();
        assert_eq!(net.gap, None);
        assert!(!net.nodes.iter().any(|n| matches!(
            n.kind,
            NodeKind::Camera | NodeKind::Turret | NodeKind::Container
        )));
        assert!(!net.nodes.iter().any(|n| n.kind == NodeKind::DataVault));
        let at_230 = SiteNet::of(c, &p, Some(t), None, 230).unwrap();
        let first_server_room = p
            .venue
            .as_deref()
            .unwrap()
            .levels
            .iter()
            .any(|x| x.rooms.iter().any(|r| r.kind == RoomKind::Server));
        assert_eq!(
            at_230.nodes.iter().any(|n| n.kind == NodeKind::DataVault),
            first_server_room
        );
    }
}

#[test]
fn a_copys_vault_is_its_objective_in_a_server_room() {
    let mut vaults = 0;
    for (c, p, t, layer) in copies(190) {
        let net = SiteNet::of(c, &p, Some(t), Some(&layer), 190).unwrap();
        let x = p
            .venue
            .as_deref()
            .unwrap()
            .level(layer.objective.level)
            .unwrap();
        let server = x
            .rooms
            .iter()
            .any(|r| r.kind == RoomKind::Server && r.rect.contains(layer.objective.at));
        let vault = net.nodes.iter().find(|n| n.kind == NodeKind::DataVault);
        assert_eq!(
            vault.map(|n| (n.node.level, n.node.at)),
            server.then_some((layer.objective.level, layer.objective.at))
        );
        vaults += usize::from(vault.is_some());
    }
    assert!(vaults > 0, "no copy's objective stands in a server room");
}

#[test]
fn a_site_without_a_lift_still_has_a_riser() {
    for (c, mut p, t) in plains() {
        p.links.retain(|l| l.kind != LinkKind::Lift);
        let net = SiteNet::of(c, &p, Some(t), None, 120).unwrap();
        assert_eq!(net.riser.is_some(), !p.links.is_empty());
        assert!(net.levels.iter().all(|l| net.on_riser(l.level)));
        p.links.clear();
        let alone = SiteNet::of(c, &p, Some(t), None, 120).unwrap();
        assert_eq!(alone.riser, None);
        assert_eq!(
            alone.levels.len(),
            p.levels.len(),
            "every level keeps its hub"
        );
    }
}

#[test]
fn the_nodes_come_firewall_first_then_by_level_kind_and_tile() {
    let rank = |k: NodeKind| KIND_ORDER.iter().position(|o| *o == k).unwrap();
    for (c, p, t, layer) in copies(190) {
        let net = SiteNet::of(c, &p, Some(t), Some(&layer), 190).unwrap();
        let keys: Vec<_> = (net.nodes.iter())
            .map(|n| (n.node.level, rank(n.kind), n.node.at.y, n.node.at.x))
            .collect();
        assert!(keys.windows(2).all(|w| w[0] < w[1]), "{}", t.id);
        let locks: Vec<Option<u8>> = (net.nodes.iter())
            .take_while(|n| n.kind == NodeKind::Firewall)
            .map(|n| n.node.firewall_lock())
            .collect();
        let want: Vec<Option<u8>> = (0..net.firewall.len()).map(|i| Some(i as u8)).collect();
        assert_eq!(locks, want);
        for (i, n) in net.nodes.iter().take(locks.len()).enumerate() {
            assert_eq!(n.net, net.firewall[i].1);
        }
    }
}

#[test]
fn no_two_nodes_share_a_tile() {
    for (c, p, t, layer) in copies(190) {
        let net = SiteNet::of(c, &p, Some(t), Some(&layer), 190).unwrap();
        let nodes: BTreeSet<SiteNode> = net.nodes.iter().map(|n| n.node).collect();
        assert_eq!(nodes.len(), net.nodes.len(), "{} a node twice", t.id);
        let tiles: BTreeSet<(i32, Pos)> =
            (net.nodes.iter()).map(|n| (n.node.level, n.net)).collect();
        assert_eq!(tiles.len(), net.nodes.len(), "{} a Net tile twice", t.id);
        for n in &net.nodes {
            assert_eq!(net.node(n.node), Some(n));
        }
    }
}
