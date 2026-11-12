//! A site's nodes as a program meets them inside its Net: what a landing on each changes
//! and what the log says of it. The spec's "What a node does":
//! `docs/superpowers/specs/2026-11-11-site-net-design.md`.

use crate::app::AppState;
use crate::owners::Owner;
use citygen::venue::Ways;
use citygen::{BuildingId, NetNode, NodeId, NodeKind, SiteNode};
use game::{Op, Target};

/// A program landing on a site's node: what it changes, and the log's line; None leaves
/// `net::land`'s own arms to say it (a copy, a backdoor, a blind).
pub fn landed(
    st: &mut AppState,
    b: BuildingId,
    node: SiteNode,
    kind: Target,
    op: Op,
    now: u64,
) -> Option<String> {
    let Target::Node(k) = kind else {
        return None;
    };
    let name = (st.constructs.get(b))
        .and_then(|c| c.device(node))
        .map_or_else(|| k.label().to_string(), |d| d.name.clone());
    let done = op.done_label();
    // In an alarm a crack opens its lock until the alarm ends (`NetState::land`).
    let alarm = st.net.alert(b, now);
    Some(match (k, op) {
        (NodeKind::DoorLock, Op::Crack | Op::Spoof) if alarm => {
            format!("lock {done} · {name} opens through the alarm")
        }
        (NodeKind::DoorLock, Op::Crack | Op::Spoof) => {
            format!("lock {done} · {name} opens for an hour")
        }
        (NodeKind::GateSwitch, Op::Crack | Op::Spoof) if alarm => {
            format!("gate switch {done} · the {name} let you through the alarm")
        }
        (NodeKind::GateSwitch, Op::Crack | Op::Spoof) => {
            format!("gate switch {done} · the {name} let you through for an hour")
        }
        (NodeKind::LiftCtrl, Op::Spoof) => {
            format!("lift control spoofed · {name} stops at every floor")
        }
        (NodeKind::Camera | NodeKind::Terminal, Op::Crack | Op::Spoof) => {
            crate::holds::take(st, NetNode::Site(node), Owner::Building(b), now)
        }
        (NodeKind::Server, Op::Crack) => format!(
            "server cracked · root on `{}` for an hour",
            crate::net::building_name(st, b)
        ),
        (NodeKind::Turret | NodeKind::Container, Op::Crack) => format!("{name} cracked"),
        // The same lock from the grid or from inside: broken for its hour, kept in
        // `NetState.firewalls` alone.
        (NodeKind::Firewall, Op::Crack) => {
            let lock = node.firewall_lock()?;
            st.net.break_lock(b, lock, now);
            let marks = super::broken(st, b);
            let down = marks.iter().filter(|m| **m).count();
            let line = format!("firewall lock {} broken · {down}/{}", lock + 1, marks.len());
            match super::firewall_open(st, b) {
                true => format!("{line} · the gate is open"),
                false => line,
            }
        }
        _ => return None,
    })
}

/// On site `b`, a program landing on one of its nodes on the city's Net, `id`: a street
/// door's lock opens only its own door, which the line names; a lift control moves none of
/// a site's lifts, each answering its own node. None for anything else, or off a site.
pub fn city_landed(
    st: &AppState,
    b: BuildingId,
    id: NodeId,
    kind: Target,
    op: Op,
) -> Option<String> {
    if !super::is_site(st, b) {
        return None;
    }
    let name = crate::net::building_name(st, b);
    let done = op.done_label();
    match (kind, op) {
        (Target::Node(NodeKind::DoorLock), Op::Crack | Op::Spoof) => {
            let (k, door) = st.city.as_ref()?.net.nodes.get(id.index())?.anchor;
            Some(match crate::venues::locked_door_in(st, k, door) {
                Some((x, _, lock, label)) if x == b && lock.ways.has(Ways::NET) => {
                    format!("lock {done} · {label} opens for an hour")
                }
                _ => format!("lock {done} · no locked door of `{name}` behind it"),
            })
        }
        (Target::Node(NodeKind::LiftCtrl), Op::Spoof) => Some(format!(
            "lift control spoofed · `{name}`'s lifts answer only their own nodes"
        )),
        _ => None,
    }
}

/// A container's lock cracked: a live crack on its node, which an alarm drops. The Look
/// reads it; a mission's containers open with it from #96 on.
pub fn container_open(st: &AppState, node: SiteNode) -> bool {
    super::hacked(st, node, &[Op::Crack])
}

#[cfg(test)]
mod tests {
    use super::super::testutil::*;
    use super::super::*;
    use citygen::{NodeKind, Place, SiteNode};
    use game::{HackTarget, ItemId, Op, Target};

    #[test]
    fn the_nodes_beside_you_are_targets() {
        let (mut st, b) = office_copy(190);
        on_hub(&mut st, b, 0);
        let pos = st.player.as_ref().unwrap().pos;
        let hub = st.constructs.site(b).unwrap().device_at(pos).unwrap().node;
        let targets = crate::net::targets_near(&st, b, pos);
        assert!(targets.iter().any(|(t, _)| *t == HackTarget::Site(hub)));
        let server = device(&st, b, NodeKind::Server);
        assert!(
            targets
                .iter()
                .any(|(t, w)| *t == HackTarget::Site(server.node) && w.contains("server"))
        );
        assert_eq!(
            crate::net::target_kind(&st, b, HackTarget::Site(hub)),
            Some(Target::Node(NodeKind::Hub))
        );
        assert!(
            crate::net::start_run(&mut st, ItemId::new("crack"), HackTarget::Site(hub))
                .unwrap_err()
                .contains("does nothing")
        );
    }

    #[test]
    fn cracking_a_door_node_from_inside_opens_that_door_on_the_floor() {
        let (mut st, b) = office_copy(190);
        let (level, door) = beside_a_door_node(&mut st, b);
        let lines = crate::net::tests::crack_to_landing(&mut st, HackTarget::Site(door.node));
        assert!(
            lines.last().unwrap().contains("opens for an hour"),
            "{lines:?}"
        );
        let lock = *crate::interiors::floor_now(&st, st.interiors.plan_of(b).unwrap(), level)
            .unwrap()
            .lock_at(door.node.at)
            .unwrap();
        assert!(lock_open(&st, b, level, &lock));
    }

    #[test]
    fn a_container_cracks_and_its_lock_stays_open() {
        let (mut st, b) = office_copy(230);
        let c = beside_device(&mut st, b, NodeKind::Container);
        let place = st.player.as_ref().unwrap().place;
        let look = |st: &AppState| look_second(st, place, c.at);
        assert_eq!(look(&st).as_deref(), Some("container lock · locked"));
        let line = crate::net::tests::crack_to_landing(&mut st, HackTarget::Site(c.node));
        assert!(line.last().unwrap().ends_with("cracked"), "{line:?}");
        assert!(container_open(&st, c.node));
        assert_eq!(look(&st).as_deref(), Some("container lock · cracked"));
        crate::net::lock_down(&mut st, b);
        assert!(!container_open(&st, c.node), "an alarm drops it");
        assert_eq!(look(&st).as_deref(), Some("container lock · locked"));
    }

    #[test]
    fn a_held_site_terminal_opens_a_shell_on_its_host() {
        let (mut st, b) = office_copy(190);
        let t = beside_device(&mut st, b, NodeKind::Terminal);
        crate::net::tests::crack_to_landing(&mut st, HackTarget::Site(t.node));
        match crate::holds::use_verb(&mut st, 'r').unwrap() {
            crate::holds::VerbDone::Shell(host, _) => {
                assert_eq!(host, crate::helix::shell::HostId::Building(b))
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_cracked_site_server_is_root() {
        let (mut st, b) = office_copy(190);
        let s = beside_device(&mut st, b, NodeKind::Server);
        crate::net::tests::crack_to_landing(&mut st, HackTarget::Site(s.node));
        assert!(st.net.server_cracked(b, st.net.now(&st)));
        assert_eq!(crate::access::tier_on(&st, b), crate::access::Tier::Admin);
    }

    #[test]
    fn a_held_site_camera_offers_its_verbs_where_it_stands_on_its_level() {
        let (mut st, b) = office_copy(190);
        let cam = beside_device(&mut st, b, NodeKind::Camera);
        let lines = crate::net::tests::crack_to_landing(&mut st, HackTarget::Site(cam.node));
        assert!(
            lines.last().unwrap().starts_with("camera held"),
            "{lines:?}"
        );
        let node = citygen::NetNode::Site(cam.node);
        let place = st.player.as_ref().unwrap().place;
        assert_eq!(crate::holds::node_pos(&st, place, node), Some(cam.at));
        assert_eq!(
            crate::holds::verbs_of(&st, node),
            vec![crate::holds::Verb::Tap, crate::holds::Verb::Loop]
        );
        assert_eq!(crate::holds::nearest(&st).map(|(n, _)| n), Some(node));
        // Off its level its verbs don't reach.
        let other = (st.constructs.site(b).unwrap().levels.iter())
            .map(|l| l.level)
            .find(|l| *l != cam.node.level)
            .unwrap();
        let other = Place::SiteNet {
            building: b,
            level: other,
        };
        assert_eq!(crate::holds::node_pos(&st, other, node), None);
    }

    #[test]
    fn the_look_reads_a_site_nodes_name_kind_and_state() {
        let (mut st, b) = office_copy(190);
        let (_, door) = beside_a_door_node(&mut st, b);
        let place = st.player.as_ref().unwrap().place;
        let card = |st: &AppState| {
            let map = crate::render::map::MapRef::for_place(
                st.city.as_ref().unwrap(),
                &st.interiors,
                &st.constructs,
                place,
                0,
            )
            .unwrap();
            crate::look::look_card(st, &map, place, door.at, &|_, _| None, &[])
        };
        let c = card(&st);
        let name = c.header.clone().unwrap();
        assert_eq!(name.to_lowercase(), door.name.to_lowercase(), "{c:?}");
        assert_eq!(c.second.as_deref(), Some("door lock · locked"), "{c:?}");
        hack(&mut st, door.node, NodeKind::DoorLock, Op::Crack);
        assert_eq!(card(&st).second.as_deref(), Some("door lock · cracked"));
    }

    /// The Look card's second line for the tile `at` of `place`.
    fn look_second(st: &AppState, place: Place, at: citygen::Pos) -> Option<String> {
        let map = crate::render::map::MapRef::for_place(
            st.city.as_ref().unwrap(),
            &st.interiors,
            &st.constructs,
            place,
            0,
        )
        .unwrap();
        crate::look::look_card(st, &map, place, at, &|_, _| None, &[]).second
    }

    #[test]
    fn the_look_reads_a_door_node_as_a_step_meets_it() {
        let (mut st, b, level, way) = a_locked_street_way_in();
        on_hub(&mut st, b, level);
        let place = st.player.as_ref().unwrap().place;
        let lock = way.lock.unwrap();
        let k = st.constructs.site(b).unwrap();
        let door = k.device(SiteNode {
            building: b,
            level,
            at: lock.at,
        });
        let door = door.expect("the way in's node").clone();
        let second = |st: &AppState| look_second(st, place, door.at);
        assert_eq!(second(&st).as_deref(), Some("door lock · locked"));
        // Its street door's city lock cracked opens it: it reads open.
        let citygen::venue::WayInKind::Door { stratum, door: at } = way.kind else {
            unreachable!()
        };
        let id = (st.city.as_ref().unwrap().net.node_at(stratum, at))
            .unwrap()
            .id;
        let now = st.net.now(&st);
        st.net.land(
            b,
            HackTarget::Node(id),
            Target::Node(NodeKind::DoorLock),
            Op::Crack,
            now,
        );
        assert!(lock_open(&st, b, level, &lock));
        assert_eq!(second(&st).as_deref(), Some("door lock · open"));
        // An alarm locks it down, the city's crack dropped; a crack on its own node landed
        // since opens it again, as a step finds it.
        crate::net::lock_down(&mut st, b);
        assert_eq!(second(&st).as_deref(), Some("door lock · locked down"));
        assert!(!lock_open(&st, b, level, &lock));
        hack(&mut st, door.node, NodeKind::DoorLock, Op::Crack);
        assert_eq!(second(&st).as_deref(), Some("door lock · cracked"));
        assert!(lock_open(&st, b, level, &lock));
    }

    #[test]
    fn on_a_site_a_street_doors_city_lock_names_the_one_door_it_opens() {
        let (st, b, _, way) = a_locked_street_way_in();
        let citygen::venue::WayInKind::Door { stratum, door } = way.kind else {
            unreachable!()
        };
        let id = (st.city.as_ref().unwrap().net.node_at(stratum, door))
            .unwrap()
            .id;
        let plan = st.interiors.plan_of(b).unwrap();
        let label = crate::venues::lock_label(plan, &way.lock.unwrap());
        let line = city_landed(&st, b, id, Target::Node(NodeKind::DoorLock), Op::Crack).unwrap();
        assert_eq!(line, format!("lock cracked · {label} opens for an hour"));
        let line = city_landed(&st, b, id, Target::Node(NodeKind::LiftCtrl), Op::Spoof).unwrap();
        assert!(!line.contains("the lifts answer you"), "{line}");
        assert!(line.contains("own node"), "{line}");
    }
}
