//! Walking a site's Net: a door's node holds you until it's cracked, but from its free
//! side; a turnstile until its line's switch is held; and from a hub, `<` and `>` ride the
//! riser to the next kept level on it, calm, never across the air gap. Spec
//! `docs/superpowers/specs/2026-11-11-site-net-design.md`.

use crate::app::AppState;
use crate::venues::Meets;
use citygen::venue::VenueLayout;
use citygen::{BuildingId, Construct, FloorPlan, NodeKind, Place, Pos};

/// A step on a site's Net from `here` onto `target`: a door node holds you until it's
/// cracked, but from its free side; a turnstile until its switch is held.
pub fn step_on_net(st: &AppState, here: Pos, target: Pos) -> Meets {
    let Some(Place::SiteNet { building: b, .. }) = st.player.as_ref().map(|p| p.place) else {
        return Meets::Nothing;
    };
    let Some(k) = st.constructs.site(b) else {
        return Meets::Nothing;
    };
    if !k.tiles.get(target).is_some_and(|t| t.is_walkable()) {
        return Meets::Nothing;
    }
    let (Some((level, at)), Some((_, from))) = (k.to_floor(target), k.to_floor(here)) else {
        return Meets::Nothing;
    };
    let door = k.device_at(target).filter(|d| d.kind == NodeKind::DoorLock);
    let Some(plan) = crate::interiors::plan_now(st, b) else {
        return Meets::Nothing;
    };
    let Some(v) = plan.venue.as_deref() else {
        return Meets::Nothing;
    };
    let line = v.gate_line_at(level, at);
    if door.is_none() && line.is_none() {
        return Meets::Nothing;
    }
    // Only a door's node or a turnstile draws the floor, for its lock.
    let floor = crate::interiors::floor_now(st, &plan, level);
    let lock = floor.as_deref().and_then(|f| f.lock_at(at));
    let opens = |lock: &citygen::DoorLock| {
        lock.free_from == Some(from) || super::lock_open(st, b, level, lock)
    };
    let (through, name) = match (door, lock, line) {
        (Some(d), Some(lock), _) => (opens(lock), d.name.clone()),
        (Some(d), None, _) => (keypad_opens(st, b, v, level, at, from), d.name.clone()),
        (None, Some(lock), Some(piece)) => {
            let name = (v.level(level)).map_or_else(String::new, |x| x.pieces[piece].name.clone());
            (opens(lock), name)
        }
        _ => return Meets::Nothing,
    };
    if through {
        return Meets::Through(None);
    }
    let state = if st.net.alert(b, st.net.now(st)) {
        "locked down"
    } else {
        "locked"
    };
    Meets::Held(format!("{state} · {name}"))
}

/// A private room's door whose only lock is its keypad: open from inside the room, else
/// to the Net's hack on the room's door (`room_open`), as the floor opens it.
fn keypad_opens(
    st: &AppState,
    b: BuildingId,
    v: &VenueLayout,
    level: i32,
    at: Pos,
    from: Pos,
) -> bool {
    let Some(x) = v.level(level) else {
        return false;
    };
    let Some(d) = x.doors.iter().find(|d| d.at == at) else {
        return false;
    };
    let private = || {
        [d.rooms.0, d.rooms.1]
            .into_iter()
            .filter_map(|r| x.rooms.get(r))
            .filter(|r| r.private)
    };
    private().any(|r| r.rect.contains(from))
        || private().any(|r| super::room_open(st, b, level, r.number))
}

/// `<` (`up` false) or `>` on a hub: along the riser to the next kept level on it.
pub fn riser(st: &mut AppState, up: bool) -> Result<String, String> {
    let (place, pos) = (st.player.as_ref())
        .map(|p| (p.place, p.pos))
        .ok_or("nobody to ride it")?;
    let Place::SiteNet { building: b, level } = place else {
        return Err("no riser here".to_string());
    };
    let k = st.constructs.site(b).ok_or("no riser here")?;
    let here = k.net_level(level).ok_or("no riser here")?;
    if pos != here.hub {
        return Err("stand on the hub to ride the riser".to_string());
    }
    if st.net.engaged.is_some() {
        return Err("not in a fight".to_string());
    }
    if here.gapped {
        return Err("AIR GAP · no riser here".to_string());
    }
    let on = k.levels.iter().filter(|l| !l.gapped);
    let next = if up {
        on.filter(|l| l.level > level).min_by_key(|l| l.level)
    } else {
        on.filter(|l| l.level < level).max_by_key(|l| l.level)
    }
    .ok_or("the riser ends here")?;
    let (to, line) = (
        next.level,
        format!(
            "Entering `{}` · {}",
            FloorPlan::label(next.level),
            next.name
        ),
    );
    land_on_hub(st, b, to)?;
    Ok(line)
}

/// Where you land on tile `at` of level `level` of a site's construct `k`: on it, or with
/// ICE on it, the free tile beside it on its level, the nearest first.
pub(super) fn landing(k: &Construct, level: i32, at: Pos) -> Option<Pos> {
    let free = |p: Pos| {
        k.tiles.get(p).is_some_and(|t| t.is_walkable())
            && k.guard_at(p).is_none()
            && k.level_at(p).is_some_and(|l| l.level == level)
    };
    std::iter::once(at)
        .chain(at.neighbors8())
        .filter(|p| free(*p))
        .min_by_key(|p| p.manhattan(at))
}

/// Where you land on level `level`'s hub of site construct `k` (`landing`).
pub(super) fn hub_landing(k: &Construct, level: i32) -> Result<Pos, String> {
    let hub = k.net_level(level).ok_or("no hub on that level")?.hub;
    landing(k, level, hub).ok_or_else(|| "ICE holds the hub".to_string())
}

/// You onto level `level`'s hub of site `b`'s Net; with ICE on it, onto the free tile
/// beside it, the nearest first. The riser lands you so, and onto G's hub the firewall's
/// gate, a backdoor and a hop (`land_on_gs_hub`).
pub(super) fn land_on_hub(st: &mut AppState, b: BuildingId, level: i32) -> Result<(), String> {
    let k = st.constructs.site(b).ok_or("no Net here")?;
    let at = hub_landing(k, level)?;
    crate::constructs::land(st, Place::SiteNet { building: b, level }, at);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::testutil::*;
    use super::super::*;
    use crate::venues::Meets;
    use citygen::venue::line_switches;
    use citygen::{NodeKind, Place, Pos, SiteNode};
    use game::Op;

    #[test]
    fn a_door_node_stops_you_until_cracked_but_not_from_its_free_side() {
        let (mut st, b) = office_copy(190);
        let (locked_side, free_side, door) = across_a_door_node(&mut st, b);
        assert!(
            matches!(step_on_net(&st, locked_side, door.at), Meets::Held(w) if w.contains(&door.name))
        );
        assert!(matches!(
            step_on_net(&st, free_side, door.at),
            Meets::Through(_)
        ));
        hack(&mut st, door.node, NodeKind::DoorLock, Op::Crack);
        assert!(matches!(
            step_on_net(&st, locked_side, door.at),
            Meets::Through(_)
        ));
    }

    #[test]
    fn in_an_alarm_a_door_node_is_locked_down() {
        let (mut st, b) = office_copy(190);
        let (locked_side, _, door) = across_a_door_node(&mut st, b);
        hack(&mut st, door.node, NodeKind::DoorLock, Op::Crack);
        crate::net::lock_down(&mut st, b);
        assert!(matches!(
            step_on_net(&st, locked_side, door.at),
            Meets::Held(w) if w.starts_with("locked down")
        ));
    }

    #[test]
    fn a_turnstile_waits_for_its_lines_switch_but_not_from_its_free_side() {
        let (mut st, b) = office_plain();
        crate::sitenet::ensure_construct(&mut st, b).unwrap();
        let plan = st.interiors.plan_of(b).unwrap().clone();
        let v = plan.venue.as_deref().unwrap();
        let k = st.constructs.site(b).unwrap();
        let open = |p: Pos| k.tiles.get(p).is_some_and(|t| t.is_walkable());
        // A turnstile with a tile to step from on each side, and its line's switch.
        let (level, at, locked, free, switch) = plan
            .levels()
            .filter(|l| k.net_level(*l).is_some_and(|n| !n.gapped))
            .find_map(|level| {
                let floor = crate::interiors::floor_now(&st, &plan, level)?;
                floor.locks.iter().find_map(|l| {
                    let piece = v.gate_line_at(level, l.at)?;
                    let free = k.on_level(level, l.free_from?).filter(|p| open(*p))?;
                    let locked = (l.at.neighbors4())
                        .filter(|n| Some(*n) != l.free_from)
                        .filter_map(|n| k.on_level(level, n))
                        .find(|p| open(*p))?;
                    let switch =
                        (line_switches(v, level).into_iter()).find(|s| s.piece == piece)?;
                    Some((level, k.on_level(level, l.at)?, locked, free, switch))
                })
            })
            .expect("a turnstile on the Net with both sides open");
        on_hub(&mut st, b, level);
        assert!(matches!(step_on_net(&st, locked, at), Meets::Held(_)));
        assert!(matches!(step_on_net(&st, free, at), Meets::Through(_)));
        let node = SiteNode {
            building: b,
            level,
            at: switch.at,
        };
        hack(&mut st, node, NodeKind::GateSwitch, Op::Crack);
        assert!(matches!(step_on_net(&st, locked, at), Meets::Through(_)));
    }

    #[test]
    fn the_riser_takes_you_to_the_next_kept_levels_hub_and_not_across_the_gap() {
        let (mut st, b) = office_copy(190);
        let net = net_of(&st, b).unwrap();
        on_hub(&mut st, b, 0);
        let up = net
            .levels
            .iter()
            .map(|l| l.level)
            .find(|l| *l > 0 && net.on_riser(*l))
            .unwrap();
        assert!(
            riser(&mut st, true)
                .unwrap()
                .contains(&citygen::FloorPlan::label(up))
        );
        assert_eq!(
            st.player.as_ref().unwrap().place,
            Place::SiteNet {
                building: b,
                level: up
            }
        );
        let gap = net.gap.unwrap();
        on_hub(&mut st, b, gap);
        assert!(riser(&mut st, true).unwrap_err().contains("AIR GAP"));
        assert!(riser(&mut st, false).unwrap_err().contains("AIR GAP"));
        on_hub(&mut st, b, 0);
        let below = riser(&mut st, false);
        assert!(
            below.is_err()
                || st.player.as_ref().unwrap().place
                    != (Place::SiteNet {
                        building: b,
                        level: gap
                    }),
            "never onto the gapped level"
        );
    }

    #[test]
    fn the_riser_asks_for_the_hub_and_calm() {
        let (mut st, b) = office_copy(190);
        on_hub(&mut st, b, 0);
        let hub = st.player.as_ref().unwrap().pos;
        let off = {
            let k = st.constructs.site(b).unwrap();
            (hub.neighbors8())
                .find(|p| k.tiles.get(*p).is_some_and(|t| t.is_walkable()))
                .unwrap()
        };
        st.player.as_mut().unwrap().pos = off;
        assert_eq!(
            riser(&mut st, true).unwrap_err(),
            "stand on the hub to ride the riser"
        );
        st.player.as_mut().unwrap().pos = hub;
        let place = st.player.as_ref().unwrap().place;
        crate::net::engage(&mut st, place, true);
        assert_eq!(riser(&mut st, true).unwrap_err(), "not in a fight");
    }

    #[test]
    fn arriving_at_a_hub_under_ice_stands_beside_it() {
        let (mut st, b) = office_copy(160);
        on_hub(&mut st, b, 0);
        let hub0 = st.player.as_ref().unwrap().pos;
        // The white at G's hub stands beside it: put it on the hub and ride back down.
        let i = {
            let k = st.constructs.site(b).unwrap();
            (0..k.guards.len())
                .find(|&i| k.level_at(k.guards[i].pos).unwrap().level == 0)
                .unwrap()
        };
        riser(&mut st, true).unwrap();
        st.constructs.get_mut(b).unwrap().move_guard(i, hub0);
        riser(&mut st, false).unwrap();
        let me = st.player.as_ref().unwrap().pos;
        assert!(me != hub0 && me.chebyshev(hub0) <= 1);
    }
}
