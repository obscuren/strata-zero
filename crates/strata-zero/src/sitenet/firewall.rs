//! A site's firewall: its locks on the city's Net, each broken for an hour by a crack from
//! the grid or from inside (`NetState.firewalls` holds them), the gate crossed either way
//! once every lock is broken, and an alarm closing them all. Spec
//! `docs/superpowers/specs/2026-11-11-site-net-design.md`, Rulings 11, 12, 21 and 26.

use crate::app::AppState;
use citygen::sitenet::firewall::{lock_count, outside};
use citygen::{BuildingId, FloorPlan, Place, Pos, StratumKind};

/// Site `b`'s firewall locks on the city's Net, the gate first (Ruling 11); empty for a
/// building that is no site or has no gate there.
pub fn lock_spots(st: &AppState, b: BuildingId) -> Vec<Pos> {
    match st.city.as_ref() {
        Some(city) if city.building_opt(b).is_some() && super::is_site(st, b) => {
            outside(city, b, lock_count(super::security(st, b)))
        }
        _ => Vec::new(),
    }
}

/// Per lock of site `b`'s firewall, in lock order, whether it stands broken now: as many
/// as its security sets, whether or not each found a tile, so a firewall short of tiles
/// (or with none) never reads open.
pub fn broken(st: &AppState, b: BuildingId) -> Vec<bool> {
    let down = st.net.broken_locks(b, st.net.now(st));
    (0..lock_count(super::security(st, b)))
        .map(|i| down.contains(&(i as u8)))
        .collect()
}

/// Whether every lock of site `b`'s firewall stands broken.
pub fn firewall_open(st: &AppState, b: BuildingId) -> bool {
    let marks = broken(st, b);
    !marks.is_empty() && marks.iter().all(|m| *m)
}

/// The gate refusing while a lock is whole: `FIREWALL · 3 locks · 1/3 broken`.
pub fn refusal(st: &AppState, b: BuildingId) -> String {
    let marks = broken(st, b);
    let (n, down) = (marks.len(), marks.iter().filter(|m| **m).count());
    let locks = if n == 1 { "lock" } else { "locks" };
    format!("FIREWALL · {n} {locks} · {down}/{n} broken")
}

/// Why site `b`'s gate won't let you across now, either way: a lock still whole (an
/// alarm closes them all, so this is what one says too), else the alarm itself.
fn shut(st: &AppState, b: BuildingId) -> Option<String> {
    if !firewall_open(st, b) {
        return Some(refusal(st, b));
    }
    st.net.alert(b, st.net.now(st)).then(|| {
        format!(
            "gate closed · `{}` on alert",
            crate::net::building_name(st, b)
        )
    })
}

/// Through the gate from the grid: onto the hub of the level the gate stands on (G's,
/// beside the server), its construct made the first time.
pub fn through(st: &mut AppState, b: BuildingId) -> Result<String, String> {
    if let Some(why) = shut(st, b) {
        return Err(why);
    }
    super::ensure_construct(st, b)?;
    let k = st
        .constructs
        .site(b)
        .ok_or("that building has no Net of its own")?;
    let level = k.level_at(k.entry).ok_or("the gate is gone")?.level;
    super::walk::land_on_hub(st, b, level)?;
    Ok(format!(
        "Entering `{}` · {} hub",
        crate::net::building_name(st, b),
        FloorPlan::label(level)
    ))
}

/// Out through the gate inside (you stand on its construct's entry), onto the city's
/// gate tile.
pub fn out(st: &mut AppState) -> Result<String, String> {
    let (place, pos) = (st.player.as_ref())
        .map(|p| (p.place, p.pos))
        .ok_or("no body")?;
    let Place::SiteNet { building: b, .. } = place else {
        return Err("not on a site's Net".to_string());
    };
    let k = st.constructs.site(b).ok_or("not on a site's Net")?;
    if pos != k.entry {
        return Err("stand on the gate to go out".to_string());
    }
    if let Some(why) = shut(st, b) {
        return Err(why);
    }
    let city = st.city.as_ref().ok_or("no city")?;
    let gate = crate::constructs::city_gate(city, b).ok_or("the gate is gone")?;
    crate::constructs::land(st, Place::Stratum(StratumKind::Net), gate);
    Ok("Entering the `GRID`".to_string())
}

#[cfg(test)]
mod tests {
    use super::super::testutil::*;
    use super::super::*;
    use citygen::{NodeKind, Place, Pos, SiteNode, StratumKind};
    use game::{HackTarget, Op};

    #[test]
    fn the_gate_refuses_while_a_lock_is_whole() {
        let (mut st, b) = office_copy(190);
        at_the_gate(&mut st, b);
        assert_eq!(
            crate::constructs::enter_construct(&mut st, b).unwrap_err(),
            "FIREWALL · 3 locks · 0/3 broken"
        );
        let now = st.net.now(&st);
        st.net.break_lock(b, 0, now);
        assert_eq!(
            crate::constructs::enter_construct(&mut st, b).unwrap_err(),
            "FIREWALL · 3 locks · 1/3 broken"
        );
    }

    #[test]
    fn with_every_lock_broken_the_gate_lands_you_on_gs_hub_beside_the_server() {
        let (mut st, b) = office_copy(190);
        at_the_gate(&mut st, b);
        let now = st.net.now(&st);
        for i in 0..3 {
            st.net.break_lock(b, i, now);
        }
        let line = crate::constructs::enter_construct(&mut st, b).unwrap();
        assert_eq!(
            line,
            format!("Entering `{}` · L hub", crate::net::building_name(&st, b))
        );
        let p = st.player.as_ref().unwrap();
        let k = st.constructs.site(b).unwrap();
        assert_eq!(
            p.place,
            Place::SiteNet {
                building: b,
                level: 0
            }
        );
        assert_eq!(k.device_at(p.pos).map(|d| d.kind), Some(NodeKind::Hub));
        assert!(
            (k.devices.iter()).any(|d| d.kind == NodeKind::Server && d.at.chebyshev(p.pos) <= 1)
        );
        assert_eq!(
            out(&mut st).unwrap_err(),
            "stand on the gate to go out",
            "only from the gate"
        );
        walk_to_the_gate_inside(&mut st, b);
        assert_eq!(out(&mut st).unwrap(), "Entering the `GRID`");
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.place, Place::Stratum(StratumKind::Net));
        let city = st.city.as_ref().unwrap();
        assert_eq!(Some(p.pos), crate::constructs::city_gate(city, b));
    }

    #[test]
    fn breaking_each_lock_from_the_grid_adds_trace() {
        let (mut st, b) = office_copy(190);
        let spots = lock_spots(&st, b);
        assert_eq!(spots.len(), 3);
        let city = st.city.as_ref().unwrap();
        assert_eq!(Some(spots[0]), crate::constructs::city_gate(city, b));
        let mut heat = 0;
        for (i, at) in spots.iter().enumerate() {
            stand_on_the_grid_beside(&mut st, *at);
            let node = SiteNode::firewall(b, i as u8);
            let here = st.player.as_ref().unwrap().pos;
            let targets = crate::net::targets_outside(&st, here);
            let name = format!(
                "firewall lock {} · {}",
                i + 1,
                crate::net::building_name(&st, b)
            );
            assert!(
                targets
                    .iter()
                    .any(|(t, w)| *t == HackTarget::Site(node) && *w == name),
                "{targets:?}"
            );
            let lines = crate::net::tests::crack_to_landing(&mut st, HackTarget::Site(node));
            let now_heat = st.net.traces.get(&b).map_or(0, |t| t.heat);
            assert!(now_heat > heat, "lock {i}");
            heat = now_heat;
            let want = format!("firewall lock {} broken · {}/3", i + 1, i + 1);
            let last = lines.last().unwrap();
            assert!(last.starts_with(&want), "{lines:?}");
            assert_eq!(last.ends_with(" · the gate is open"), i == 2, "{last}");
        }
        assert!(firewall_open(&st, b));
        assert!(
            !st.net.hacks.iter().any(|h| h.building == b),
            "a broken lock is no hack"
        );
    }

    #[test]
    fn three_locks_leave_room_under_the_alarm() {
        let (mut st, b) = office_copy(190);
        st.net.force_roll = Some(10);
        for (i, at) in lock_spots(&st, b).iter().enumerate() {
            stand_on_the_grid_beside(&mut st, *at);
            crate::net::tests::crack_to_landing(
                &mut st,
                HackTarget::Site(SiteNode::firewall(b, i as u8)),
            );
        }
        assert!(firewall_open(&st, b));
        let heat = st.net.traces[&b].heat;
        assert_eq!(heat, 36);
        assert!(heat < game::ALARM_HEAT);
        assert!(!st.net.alert(b, st.net.now(&st)));
    }

    #[test]
    fn a_crack_from_inside_counts_as_one_from_outside() {
        let (mut st, b) = office_copy(190);
        on_hub(&mut st, b, 0);
        let lock = (st.constructs.site(b).unwrap().devices.iter())
            .find(|d| d.node == SiteNode::firewall(b, 1))
            .unwrap()
            .clone();
        walk_beside(&mut st, lock.at);
        let lines = crate::net::tests::crack_to_landing(&mut st, HackTarget::Site(lock.node));
        assert_eq!(broken(&st, b), vec![false, true, false]);
        assert_eq!(
            lines.last().map(String::as_str),
            Some("firewall lock 2 broken · 1/3")
        );
        assert!(
            !(st.net.hacks.iter()).any(|h| h.target == HackTarget::Site(lock.node)),
            "the firewall is its one record"
        );
    }

    #[test]
    fn an_alarm_shuts_the_gate_behind_you_but_jacking_out_works() {
        let (mut st, b) = office_copy(190);
        at_the_gate(&mut st, b);
        let now = st.net.now(&st);
        for i in 0..3 {
            st.net.break_lock(b, i, now);
        }
        let (level, door) = a_door_node(&st, b);
        hack(&mut st, door, NodeKind::DoorLock, Op::Crack);
        crate::constructs::enter_construct(&mut st, b).unwrap();
        crate::net::lock_down(&mut st, b);
        assert_eq!(broken(&st, b), vec![false, false, false]);
        walk_to_the_gate_inside(&mut st, b);
        assert!(out(&mut st).unwrap_err().starts_with("FIREWALL · 3 locks"));
        let lock = lock_of(&st, b, level, door);
        assert!(!lock_open(&st, b, level, &lock));
        crate::net::jack_out(&mut st).unwrap();
        assert!(!st.player.as_ref().unwrap().jacked_in());
    }

    #[test]
    fn on_alert_with_the_locks_broken_the_gate_says_so() {
        let (mut st, b) = office_copy(190);
        at_the_gate(&mut st, b);
        let now = st.net.now(&st);
        st.net.trace(b).alert_until = Some(now + game::ALERT_MINUTES);
        for i in 0..3 {
            st.net.break_lock(b, i, now);
        }
        assert_eq!(
            crate::constructs::enter_construct(&mut st, b).unwrap_err(),
            format!(
                "gate closed · `{}` on alert",
                crate::net::building_name(&st, b)
            )
        );
    }

    #[test]
    fn broken_locks_close_after_the_hour() {
        let (mut st, b) = office_copy(190);
        let now = st.net.now(&st);
        st.net.break_lock(b, 0, now);
        assert_eq!(
            st.net.broken_locks(b, now + game::HACK_MINUTES - 1),
            vec![0]
        );
        assert!(st.net.broken_locks(b, now + game::HACK_MINUTES).is_empty());
        // Broken again, the lock's hour starts over; the minute drops it once closed.
        st.net.break_lock(b, 0, now + 10);
        assert_eq!(
            st.net.firewalls[&b],
            vec![(0, now + 10 + game::HACK_MINUTES)]
        );
        let roots = st.backdoors.roots();
        let end = now + 10 + game::HACK_MINUTES;
        st.net.minute(end, None, &mut st.constructs, &roots);
        assert!(!st.net.firewalls.contains_key(&b));
    }

    #[test]
    fn forgetting_a_sites_net_drops_its_broken_locks() {
        let (mut st, b) = office_copy(190);
        let now = st.net.now(&st);
        st.net.break_lock(b, 1, now);
        forget(&mut st, b);
        assert!(st.net.broken_locks(b, now).is_empty());
    }

    #[test]
    fn a_firewall_with_no_lock_placed_never_reads_open() {
        let (mut st, b) = office_copy(190);
        assert_eq!(broken(&st, b), vec![false; 3]);
        assert!(!firewall_open(&st, b));
        // A building with no gate on the city's Net places no lock: still shut.
        let city = st.city.as_mut().unwrap();
        let gate = crate::constructs::city_gate(city, b).unwrap();
        city.tile_mut(StratumKind::Net, gate).unwrap().terrain = citygen::Terrain::NetWall;
        assert!(lock_spots(&st, b).is_empty());
        assert_eq!(broken(&st, b), vec![false; 3]);
        assert!(!firewall_open(&st, b));
    }

    /// A tile of the grid beside `at`, off building `b`'s footprint, you may stand on.
    fn grid_beside(st: &AppState, b: BuildingId, at: Pos) -> Option<Pos> {
        let city = st.city.as_ref().unwrap();
        let fp = city.building(b).footprint;
        (at.neighbors8()).find(|p| city.walkable(StratumKind::Net, *p) && !fp.contains(*p))
    }

    /// Whether a program from `at` on the open Net reaches city node `id`.
    fn reaches(st: &AppState, at: Pos, id: citygen::NodeId) -> bool {
        crate::net::targets_outside(st, at)
            .iter()
            .any(|(t, _)| *t == HackTarget::Node(id))
    }

    #[test]
    fn a_sites_server_vault_and_lifts_are_inside_the_firewall() {
        let (mut st, b) = office_plain();
        let inside = |k: NodeKind| {
            matches!(
                k,
                NodeKind::Server | NodeKind::DataVault | NodeKind::LiftCtrl
            )
        };
        // A site's own server, vault and lifts: from no tile on or beside them.
        let city = st.city.clone().unwrap();
        let ours = || city.net.nodes.iter().filter(|n| n.building == Some(b));
        for n in ours().filter(|n| inside(n.kind)) {
            let mut around = std::iter::once(n.cyber).chain(n.cyber.neighbors8());
            assert!(!around.any(|at| reaches(&st, at, n.id)), "{:?}", n.kind);
        }
        // Its face's nodes (ads, street door locks, cameras, terminals) stay in reach.
        let mut face = 0;
        for n in ours().filter(|n| !inside(n.kind) && n.kind != NodeKind::Ice) {
            if let Some(at) = grid_beside(&st, b, n.cyber) {
                assert!(reaches(&st, at, n.id), "{:?} at {:?}", n.kind, n.cyber);
                face += 1;
            }
        }
        assert!(face > 0, "a node on its face beside the grid");
        // Its server stood on its face, where the open Net reaches a building's nodes: out
        // of reach on a site, in reach once the same building is no site.
        let fp = city.building(b).footprint;
        let corner = |p: &Pos| {
            (p.x == fp.x || p.x == fp.right() - 1) && (p.y == fp.y || p.y == fp.bottom() - 1)
        };
        let wall = (fp.border())
            .filter(|p| !corner(p))
            .find(|p| {
                city.tile(StratumKind::Net, *p)
                    .is_some_and(|t| t.terrain == citygen::Terrain::NetWall)
                    && !city.net.nodes.iter().any(|n| n.cyber == *p)
                    && grid_beside(&st, b, *p).is_some()
            })
            .expect("a bare wall of its face beside the grid");
        let server = (city.net.nodes.iter())
            .position(|n| n.building == Some(b) && n.kind == NodeKind::Server)
            .expect("its server");
        st.city.as_mut().unwrap().net.nodes[server].cyber = wall;
        let id = st.city.as_ref().unwrap().net.nodes[server].id;
        let at = grid_beside(&st, b, wall).unwrap();
        assert!(!reaches(&st, at, id), "a site's server stays inside");
        st.interiors
            .put_plan(citygen::FloorPlan::classic(&city, b).unwrap());
        assert!(!is_site(&st, b));
        assert!(
            reaches(&st, at, id),
            "no site: the face's server is in reach"
        );
    }
}
