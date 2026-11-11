//! The ways into a site's Net besides its firewall: jacking in on a floor beside a terminal
//! or a turnstile line's switch, your body left standing there; and a backdoor on the
//! site, landing you on G's hub. Spec `docs/superpowers/specs/2026-11-11-site-net-design.md`,
//! "Ways in"; Ruling 19.

use crate::app::AppState;
use citygen::sitenet::SiteNodeInfo;
use citygen::{BuildingId, FloorPlan, NodeKind, Place, Pos, SiteNode};

/// Whether a node of a site's Net is a way in on its floor: a terminal or a gate switch.
/// A camera is out of reach on the ceiling.
pub(super) fn is_port(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Terminal | NodeKind::GateSwitch)
}

/// The terminals and switches drawn on a site's floor, by their wall tiles.
pub fn floor_devices(st: &AppState, b: BuildingId, level: i32) -> Vec<(Pos, NodeKind)> {
    let Some(net) = super::net_of(st, b) else {
        return Vec::new();
    };
    (net.nodes.iter())
        .filter(|n| n.node.level == level && is_port(n.kind))
        .map(|n| (n.node.at, n.kind))
        .collect()
}

/// The terminal or gate switch your body stands beside on a site's floor (Chebyshev 1 of
/// its wall tile, Ruling 19), as the site's Net has it: its building and its node.
pub(crate) fn port_in_reach(st: &AppState) -> Option<(BuildingId, SiteNodeInfo)> {
    let p = st.player.as_ref()?;
    let Place::Floor { building: b, level } = p.place else {
        return None;
    };
    let net = super::net_of(st, b)?;
    let port = (net.nodes.into_iter())
        .find(|n| n.node.level == level && is_port(n.kind) && n.node.at.chebyshev(p.pos) <= 1)?;
    Some((b, port))
}

/// The terminal or gate switch your body stands beside on a site's floor: its building,
/// level, node and name.
pub fn device_in_reach(st: &AppState) -> Option<(BuildingId, i32, SiteNode, String)> {
    port_in_reach(st).map(|(b, n)| (b, n.node.level, n.node, n.name))
}

/// Jacks in at the terminal or gate switch beside you on a site's floor, unless the port
/// refuses (`net::jack_in_refusal`: an alarm locks them): your body stays where it stands,
/// you on that level of its Net on the device's node (beside it when ICE stands there),
/// its construct made the first time.
pub fn jack_in_on_floor(st: &mut AppState) -> Result<String, String> {
    if let Some(why) = crate::net::jack_in_refusal(st) {
        return Err(why);
    }
    let (b, port) = port_in_reach(st).ok_or("no terminal within reach")?;
    let (from, body) = (st.player.as_ref())
        .filter(|p| !p.jacked_in())
        .map(|p| (p.place, p.pos))
        .ok_or("already jacked in")?;
    super::ensure_construct(st, b)?;
    let level = port.node.level;
    let k = st
        .constructs
        .site(b)
        .ok_or("that building has no Net of its own")?;
    let node = k.device(port.node).ok_or("the port is dead")?.at;
    let at = super::walk::landing(k, level, node).ok_or("ICE holds the port")?;
    if let Some(p) = st.player.as_mut() {
        p.body = Some((from, body));
    }
    // The first beat falls a full beat after you land.
    st.net.beat = st.clock_tick();
    crate::constructs::land(st, Place::SiteNet { building: b, level }, at);
    crate::net::after_jack_in(st, from, None);
    Ok(format!(
        "jacked in · {} · {}",
        port.name,
        FloorPlan::label(level)
    ))
}

/// G's level of site `b`'s Net (the level its gate stands on) and where you land on its
/// hub, its construct made the first time: what a backdoor or a hop settles before the
/// key is used.
pub fn gs_hub(st: &mut AppState, b: BuildingId) -> Result<(i32, Pos), String> {
    super::ensure_construct(st, b)?;
    let k = (st.constructs.site(b)).ok_or("that building has no Net of its own")?;
    let level = k.level_at(k.entry).ok_or("the gate is gone")?.level;
    Ok((level, super::walk::hub_landing(k, level)?))
}

/// You onto G's hub of site `b`, from wherever (through the firewall, a backdoor's
/// landing, a hop), beside it when ICE stands there: G's level.
pub fn land_on_gs_hub(st: &mut AppState, b: BuildingId) -> Result<i32, String> {
    let (level, _) = gs_hub(st, b)?;
    // The first beat falls a full beat after you land.
    st.net.beat = st.clock_tick();
    super::walk::land_on_hub(st, b, level)?;
    Ok(level)
}

#[cfg(test)]
mod tests {
    use super::super::testutil::*;
    use super::super::*;
    use crate::screens::popup::{Pick, WorldAct};
    use citygen::venue::{PieceKind, line_switches};
    use citygen::{NodeKind, Place};

    #[test]
    fn jacking_in_at_the_kiosk_lands_on_its_node() {
        let (mut st, b) = office_copy(190);
        let kiosk = stand_beside_the_kiosk(&mut st, b);
        let body = st.player.as_ref().unwrap().pos;
        let line = crate::net::jack_in_here(&mut st).unwrap();
        let p = st.player.as_ref().unwrap();
        assert_eq!(
            p.place,
            Place::SiteNet {
                building: b,
                level: 0
            }
        );
        let k = st.constructs.site(b).unwrap();
        assert_eq!(k.device_at(p.pos).map(|d| d.node), Some(kiosk));
        assert_eq!(
            p.body,
            Some((
                Place::Floor {
                    building: b,
                    level: 0
                },
                body
            ))
        );
        let name = &k.device(kiosk).unwrap().name;
        assert_eq!(line, format!("jacked in · {name} · L"));
        assert!(
            st.backdoors.route.is_none(),
            "a floor's port has no route home"
        );
        crate::net::jack_out(&mut st).unwrap();
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.pos, body);
        assert_eq!(
            p.place,
            Place::Floor {
                building: b,
                level: 0
            }
        );
    }

    #[test]
    fn a_switch_is_a_way_in_and_a_camera_is_not() {
        let (mut st, b) = office_copy(190);
        let switch = stand_beside_the_switch(&mut st, b);
        let (building, level, node, name) = device_in_reach(&st).expect("the switch in reach");
        assert_eq!((building, level, node), (b, switch.level, switch));
        // It goes by its line's name, a gate line of the plan's (the names are the data's).
        let plan = crate::interiors::plan_now(&st, b).unwrap();
        let v = plan.venue.as_deref().unwrap();
        let s = (line_switches(v, level).into_iter())
            .find(|s| s.at == switch.at)
            .unwrap();
        let line = &v.level(level).unwrap().pieces[s.piece];
        assert_eq!(
            (line.kind, line.name.as_str()),
            (PieceKind::GateLine, &*name)
        );
        stand_beside_a_camera(&mut st, b);
        assert!(device_in_reach(&st).is_none());
    }

    #[test]
    fn an_alarm_locks_the_floors_ports() {
        let (mut st, b) = office_copy(190);
        stand_beside_the_kiosk(&mut st, b);
        assert_eq!(crate::net::jack_in_refusal(&st), None);
        crate::net::lock_down(&mut st, b);
        assert_eq!(
            crate::net::jack_in_here(&mut st).unwrap_err(),
            "port locked down"
        );
        assert_eq!(jack_in_on_floor(&mut st).unwrap_err(), "port locked down");
        assert!(!st.player.as_ref().unwrap().jacked_in());
    }

    #[test]
    fn a_backdoor_on_a_sites_server_lands_on_gs_hub() {
        let (mut st, b) = office_copy(190);
        let now = st.net.now(&st);
        st.backdoors
            .plant_net(b, 99, st.city.as_ref().unwrap().seed, now);
        stand_at_the_rig(&mut st);
        let line = crate::net::land_at(&mut st, b).unwrap();
        let p = st.player.as_ref().unwrap();
        assert_eq!(
            p.place,
            Place::SiteNet {
                building: b,
                level: 0
            }
        );
        let k = st.constructs.site(b).unwrap();
        assert_eq!(k.device_at(p.pos).map(|d| d.kind), Some(NodeKind::Hub));
        assert_eq!(
            p.body.map(|(place, _)| place),
            Some(st.home.unwrap().place())
        );
        let name = crate::net::building_name(&st, b);
        assert_eq!(
            line,
            format!("jacked in · `{name}`'s L hub · via your backdoor")
        );
        assert_eq!(st.backdoors.route.as_ref().unwrap().relays, vec![b]);
    }

    #[test]
    fn a_hop_onto_a_sites_backdoor_lands_on_gs_hub() {
        let (mut st, b) = office_copy(190);
        let now = st.net.now(&st);
        st.backdoors
            .plant_net(b, 99, st.city.as_ref().unwrap().seed, now);
        stand_at_the_rig(&mut st);
        crate::net::jack_in_here(&mut st).unwrap();
        let line = crate::net::hop(&mut st, b).unwrap();
        let p = st.player.as_ref().unwrap();
        assert_eq!(
            p.place,
            Place::SiteNet {
                building: b,
                level: 0
            }
        );
        let k = st.constructs.site(b).unwrap();
        assert_eq!(k.device_at(p.pos).map(|d| d.kind), Some(NodeKind::Hub));
        let name = crate::net::building_name(&st, b);
        assert_eq!(line, format!("hop · `{name}`'s L hub"));
        assert_eq!(st.backdoors.route.as_ref().unwrap().relays, vec![b]);
    }

    #[test]
    fn a_sites_floor_shows_its_terminals_and_switches_on_their_wall_tiles() {
        let (mut st, b) = office_copy(190);
        let kiosk = stand_beside_the_kiosk(&mut st, b);
        let switch = stand_beside_the_switch(&mut st, b);
        let on = |level: i32| floor_devices(&st, b, level);
        assert!(on(0).contains(&(kiosk.at, NodeKind::Terminal)));
        assert!(on(switch.level).contains(&(switch.at, NodeKind::GateSwitch)));
        let plan = crate::interiors::plan_now(&st, b).unwrap();
        for level in plan.levels() {
            let floor = crate::interiors::floor_now(&st, &plan, level).unwrap();
            for (at, kind) in on(level) {
                assert!(matches!(kind, NodeKind::Terminal | NodeKind::GateSwitch));
                assert!(!floor.tiles[at].is_walkable(), "{at:?} on {level}: a wall");
            }
        }
    }

    #[test]
    fn beside_the_kiosk_the_floor_offers_jack_in_under_its_name() {
        let (mut st, b) = office_copy(190);
        let kiosk = stand_beside_the_kiosk(&mut st, b);
        let (.., name) = device_in_reach(&st).unwrap();
        let subjects = crate::interact::gather(&st, &|_, _| None);
        let s = (subjects.iter())
            .find(|s| s.pos == kiosk.at)
            .expect("the kiosk's window");
        assert_eq!(s.name(), crate::ui::hud::capitalised(&name));
        assert_eq!(s.choices.len(), 1, "{:?}", s.choices);
        let row = &s.choices[0];
        assert_eq!((row.key, row.label.as_str()), ('j', "Jack in"));
        assert!(matches!(row.pick, Pick::World(WorldAct::JackIn)));
    }
}
