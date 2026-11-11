use super::*;
use crate::construct::{Construct, Guard};
use crate::geom::{Grid, Rect};
use crate::ids::NodeId;
use crate::interior::venue::testutil;
use crate::interior::venue::{
    MissionInputs, MissionSize, VenuePack, Ways, draw_copy_level, line_switches, terminals,
};
use crate::interior::{LinkKind, RoomKind};
use crate::tile::{Feature, IceGrade, Terrain, Tile};
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

/// The site's construct of a plan, from its Net at security `s`.
fn make(c: &City, p: &FloorPlan, t: &VenueType, layer: Option<&MissionLayer>, s: u8) -> Construct {
    site_construct(c, &SiteNet::of(c, p, Some(t), layer, s).unwrap(), p, layer)
}

#[test]
fn a_site_construct_is_its_floors_tile_for_tile() {
    for (c, p, t, layer) in copies(190) {
        let k = make(c, &p, t, Some(&layer), 190);
        validate_site_construct(&k).unwrap();
        assert_eq!(
            k.levels.iter().map(|l| l.level).collect::<Vec<_>>(),
            p.levels.iter().rev().copied().collect::<Vec<_>>(),
            "the highest on top"
        );
        for l in &k.levels {
            let floor = draw_level(p.venue.as_deref().unwrap(), l.level).unwrap();
            for (fp, ft) in floor.tiles.iter() {
                let q = k.on_level(l.level, fp).unwrap();
                let nt = k.tiles[q];
                match ft.terrain {
                    _ if q == k.entry => assert_eq!(nt.terrain, Terrain::NetGate),
                    Terrain::Wall | Terrain::Window => {
                        assert_eq!(nt.terrain, Terrain::NetWall, "{} {fp:?}", l.level)
                    }
                    Terrain::Floor => assert!(
                        matches!(nt.terrain, Terrain::NetFloor | Terrain::NetGate),
                        "{fp:?}"
                    ),
                    _ => {}
                }
            }
        }
    }
}

#[test]
fn a_door_with_a_net_lock_is_its_node_and_one_without_is_wall() {
    for (c, p, t, layer) in copies(190) {
        let k = make(c, &p, t, Some(&layer), 190);
        let v = p.venue.as_deref().unwrap();
        for &level in &p.levels {
            let floor = draw_copy_level(v, &layer, level).unwrap();
            for lock in &floor.locks {
                let q = k.on_level(level, lock.at).unwrap();
                if !lock.ways.has(Ways::NET) {
                    assert_eq!(k.tiles[q].terrain, Terrain::NetWall);
                } else if v.gate_line_at(level, lock.at).is_none() {
                    assert_eq!(k.device_at(q).map(|d| d.kind), Some(NodeKind::DoorLock));
                }
            }
        }
    }
}

#[test]
fn the_ice_follows_the_security_table() {
    use crate::tile::IceGrade::*;
    let (c, p, t, layer) = copies(190).remove(0);
    let grades = |s: u8| -> Vec<(i32, IceGrade, bool)> {
        let k = make(c, &p, t, Some(&layer), s);
        k.guards
            .iter()
            .map(|g| {
                (
                    k.level_at(g.post).unwrap().level,
                    g.grade,
                    g.beat.is_empty(),
                )
            })
            .collect()
    };
    assert!(grades(99).is_empty());
    assert_eq!(grades(100), vec![(0, White, true)]);
    let s160 = grades(160);
    let others: Vec<i32> = (p.levels.iter().copied().filter(|l| *l != 0))
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, l)| l)
        .collect();
    assert_eq!(
        (s160.iter())
            .filter(|g| g.1 == White && !g.2)
            .map(|g| g.0)
            .collect::<BTreeSet<_>>(),
        others.into_iter().collect()
    );
    let s200 = grades(200);
    assert!(
        p.levels
            .iter()
            .all(|l| s200.iter().any(|g| g.0 == *l && g.1 == Grey && !g.2))
    );
    assert!(s200.contains(&(0, White, true)));
    let s230 = grades(230);
    assert_eq!(
        s230.iter().filter(|g| g.1 == Black).count(),
        usize::from(
            SiteNet::of(c, &p, Some(t), Some(&layer), 230)
                .unwrap()
                .nodes
                .iter()
                .any(|n| n.kind == NodeKind::DataVault)
        )
    );
}

#[test]
fn patrols_walk_their_own_level() {
    for (c, p, t, layer) in copies(210) {
        let k = make(c, &p, t, Some(&layer), 210);
        for g in &k.guards {
            let band = k.level_at(g.post).unwrap().area;
            assert!(k.device_at(g.post).is_none(), "no piece posts on a node");
            for w in g.beat.windows(2) {
                assert_eq!(w[0].manhattan(w[1]), 1);
            }
            // The terrain, not `is_walkable`: the piece's own ICE stands on its first tile.
            assert!(
                g.beat
                    .iter()
                    .all(|q| band.contains(*q) && k.tiles[*q].terrain == Terrain::NetFloor)
            );
        }
    }
}

#[test]
fn no_open_tile_touches_another_level() {
    for (c, p, t) in plains() {
        let net = SiteNet::of(c, &p, Some(t), None, 120).unwrap();
        let k = site_construct(c, &net, &p, None);
        for (q, tile) in k
            .tiles
            .iter()
            .filter(|(_, x)| x.terrain != Terrain::NetWall)
        {
            let mine = k.level_at(q).map(|l| l.level);
            assert!(mine.is_some(), "{q:?} stands in no level");
            assert!(
                q.neighbors8().all(|n| k
                    .tiles
                    .get(n)
                    .is_none_or(|x| x.terrain == Terrain::NetWall)
                    || k.level_at(n).map(|l| l.level) == mine),
                "{tile:?}"
            );
        }
    }
}

#[test]
fn the_gate_is_lock_zero_on_gs_wall() {
    for (c, p, t) in plains() {
        let net = SiteNet::of(c, &p, Some(t), None, 120).unwrap();
        let k = site_construct(c, &net, &p, None);
        assert_eq!(k.tiles[k.entry].terrain, Terrain::NetGate);
        assert_eq!(
            k.device_at(k.entry).map(|d| d.node),
            Some(SiteNode::firewall(p.building, 0))
        );
        assert_eq!(k.level_at(k.entry).map(|l| l.level), Some(0));
    }
}

#[test]
fn ice_stepping_off_a_node_puts_the_node_back() {
    let (c, p, t, layer) = copies(210).remove(0);
    let mut k = make(c, &p, t, Some(&layer), 210);
    let d = k
        .devices
        .iter()
        .find(|d| d.kind == NodeKind::DoorLock)
        .unwrap()
        .clone();
    let from = k.guards[0].pos;
    k.move_guard(0, d.at);
    k.move_guard(0, from);
    assert_eq!(
        k.tiles[d.at].feature,
        Some(Feature::Node(NodeKind::DoorLock))
    );
}

#[test]
fn an_old_construct_reads_with_no_levels() {
    #[derive(serde::Serialize)]
    struct Old<'a> {
        building: BuildingId,
        security: u8,
        size: Size,
        tiles: &'a Grid<Tile>,
        light: &'a Grid<u8>,
        entry: Pos,
        rooms: Vec<Rect>,
        nodes: Vec<(NodeId, Pos)>,
        guards: Vec<Guard>,
    }
    let k = crate::construct::lot_construct();
    let old = Old {
        building: k.building,
        security: k.security,
        size: k.size,
        tiles: &k.tiles,
        light: &k.light,
        entry: k.entry,
        rooms: k.rooms.clone(),
        nodes: k.nodes.clone(),
        guards: k.guards.clone(),
    };
    let back: Construct = rmp_serde::from_slice(&rmp_serde::to_vec_named(&old).unwrap()).unwrap();
    assert!(!back.is_site() && back.devices.is_empty());
    assert_eq!(back, k);
}

#[test]
fn whites_patrol_every_other_level_but_g_and_g_keeps_its_post() {
    let mut patrols = 0;
    for (c, p, t, layer) in copies(190) {
        let k = make(c, &p, t, Some(&layer), 190);
        let others: BTreeSet<i32> = (p.levels.iter().copied().filter(|l| *l != 0))
            .enumerate()
            .filter(|(i, _)| i % 2 == 1)
            .map(|(_, l)| l)
            .collect();
        let walking: BTreeSet<i32> = (k.guards.iter())
            .filter(|g| g.grade == IceGrade::White && g.beat.len() >= 2)
            .map(|g| k.level_at(g.post).unwrap().level)
            .collect();
        assert_eq!(walking, others, "{} {:?}", t.id, p.levels);
        let posted: Vec<i32> = (k.guards.iter())
            .filter(|g| g.beat.is_empty())
            .map(|g| k.level_at(g.post).unwrap().level)
            .collect();
        assert_eq!(posted, vec![0], "only the white at G's hub keeps its post");
        patrols += others.len();
    }
    assert!(patrols > 0, "no copy keeps two levels besides G");
}

#[test]
fn a_door_to_the_city_is_net_wall_and_the_ways_between_levels_net_floor() {
    for (c, p, t) in plains() {
        let k = make(c, &p, t, None, 120);
        for &level in &p.levels {
            let floor = draw_level(p.venue.as_deref().unwrap(), level).unwrap();
            for e in &floor.exits {
                let q = k.on_level(level, e.at).unwrap();
                assert_eq!(k.tiles[q].terrain, Terrain::NetWall, "{} {level}", t.id);
            }
            for (fp, ft) in floor.tiles.iter() {
                if matches!(
                    ft.terrain,
                    Terrain::Lift
                        | Terrain::Stair
                        | Terrain::Escalator
                        | Terrain::Ladder
                        | Terrain::Track
                ) {
                    let q = k.on_level(level, fp).unwrap();
                    assert_eq!(k.tiles[q].terrain, Terrain::NetFloor, "{} {fp:?}", t.id);
                }
            }
        }
    }
}

#[test]
fn every_site_construct_keeps_its_rules_and_comes_out_the_same() {
    for s in [99, 100, 160, 200, 230] {
        for (c, p, t) in plains() {
            let net = SiteNet::of(c, &p, Some(t), None, s).unwrap();
            let k = site_construct(c, &net, &p, None);
            validate_site_construct(&k).unwrap_or_else(|e| panic!("{} at {s}: {e}", t.id));
            assert_eq!(k, site_construct(c, &net, &p, None), "{} at {s}", t.id);
        }
        for (c, p, t, layer) in copies(s) {
            let k = make(c, &p, t, Some(&layer), s);
            validate_site_construct(&k).unwrap_or_else(|e| panic!("{} copy at {s}: {e}", t.id));
            assert_eq!(k, make(c, &p, t, Some(&layer), s), "{} copy at {s}", t.id);
        }
    }
}

#[test]
fn a_site_with_no_gate_on_the_citys_net_still_has_its_gate_inside() {
    for (c, p, t) in plains() {
        let mut net = SiteNet::of(c, &p, Some(t), None, 120).unwrap();
        net.firewall.clear();
        net.nodes.retain(|n| n.kind != NodeKind::Firewall);
        let k = site_construct(c, &net, &p, None);
        validate_site_construct(&k).unwrap_or_else(|e| panic!("{}: {e}", t.id));
        assert!(k.device_at(k.entry).is_none());
        let (level, at) = k.to_floor(k.entry).unwrap();
        let area = k.net_level(0).unwrap().area;
        assert_eq!(level, 0);
        assert!(at.x == 0 || at.y == 0 || at.x == area.w - 1 || at.y == area.h - 1);
    }
}

#[test]
fn each_level_knows_its_fold_the_lifts_through_its_slab_and_its_turnstiles() {
    let (mut lifts, mut turnstiles) = (0, 0);
    for (c, p, t, layer) in copies(190) {
        let net = SiteNet::of(c, &p, Some(t), Some(&layer), 190).unwrap();
        let v = p.venue.as_deref().unwrap();
        for (i, l) in net.levels.iter().enumerate() {
            let x = v.level(l.level).unwrap();
            assert_eq!(l.fold, fold_row(x));
            assert!(
                (1..l.size.h as i32 - 1).contains(&l.fold),
                "{} {}",
                t.id,
                l.level
            );
            // Each lift through its slab stops here and on the next kept level below.
            let below = i.checked_sub(1).map(|j| net.levels[j].level);
            for &lx in &l.lifts {
                assert!(p.links.iter().any(|k| {
                    k.kind == LinkKind::Lift
                        && (k.stops.iter()).any(|s| s.level == l.level && s.at.x == lx)
                        && (k.stops.iter()).any(|s| Some(s.level) == below)
                }));
            }
            // Each turnstile stands in a gate line, with that line's switch.
            let switches = line_switches(v, l.level);
            for &(at, switch) in &l.turnstiles {
                let piece = v.gate_line_at(l.level, at).expect("in a gate line");
                let want = switches.iter().find(|s| s.piece == piece).map(|s| s.at);
                assert_eq!(switch, want, "{} {} {at:?}", t.id, l.level);
            }
            lifts += l.lifts.len();
            turnstiles += l.turnstiles.len();
        }
    }
    assert!(
        lifts > 0 && turnstiles > 0,
        "lifts {lifts} turnstiles {turnstiles}"
    );
}
