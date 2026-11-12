//! Who a node answers to: a building's subnet or a district's. A run's heat goes to its
//! target's owner. Buildings keep their traces where they always have (`NetState::
//! traces`); districts keep theirs beside them (`NetState::district_traces`), and
//! every heat on either comes through `heat_on`. Spec
//! `docs/superpowers/specs/2026-11-04-net-nodes-design.md`, "Owners and traces".

use crate::app::AppState;
use citygen::{BuildingId, City, DistrictId, NodeId, SubnetOwner};

/// A building's subnet or a district's.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Owner {
    Building(BuildingId),
    District(DistrictId),
}

/// The owner of a node's subnet; none for the city's own core.
pub fn owner_of(city: &City, node: NodeId) -> Option<Owner> {
    let n = city.net.nodes.get(node.index())?;
    match city.net.subnets.get(n.subnet.index())?.owner {
        SubnetOwner::Building(b) => Some(Owner::Building(b)),
        SubnetOwner::District(d) => Some(Owner::District(d)),
        SubnetOwner::City => None,
    }
}

/// The owner's subnet security; a site's governing security (`sitenet::security`).
pub fn security(st: &AppState, o: Owner) -> u8 {
    match o {
        Owner::Building(b) if crate::sitenet::is_site(st, b) => crate::sitenet::security(st, b),
        // A construct already made carries its own (the lot's practice construct
        // among them); else the building's subnet.
        Owner::Building(b) => st
            .constructs
            .get(b)
            .map_or_else(|| crate::access::security_of(st, b), |k| k.security),
        Owner::District(d) => st
            .city
            .as_ref()
            .and_then(|c| {
                c.net
                    .subnets
                    .iter()
                    .find(|s| s.owner == SubnetOwner::District(d))
            })
            .map_or(0, |s| s.security),
    }
}

/// The owner's trace's heat.
pub fn heat(st: &AppState, o: Owner) -> u8 {
    match o {
        Owner::Building(b) => st.net.traces.get(&b).map_or(0, |t| t.heat),
        Owner::District(d) => st.net.district_traces.get(&d).map_or(0, |t| t.heat),
    }
}

/// Whether the owner's alarm is up.
pub fn alert(st: &AppState, o: Owner, now: u64) -> bool {
    match o {
        Owner::Building(b) => st.net.alert(b, now),
        Owner::District(d) => st.net.district_traces.get(&d).is_some_and(|t| t.alert(now)),
    }
}

/// What the HUD calls the owner: the building's name, or the district's with
/// `(district)`.
pub fn name(st: &AppState, o: Owner) -> String {
    let Some(city) = st.city.as_ref() else {
        return String::new();
    };
    match o {
        Owner::Building(b) => crate::net::building_name(st, b),
        Owner::District(d) => city
            .district_opt(d)
            .map_or_else(|| city.name.clone(), |x| format!("{} (district)", x.name)),
    }
}

/// Noise a turn makes on the open Net: half again what it makes behind a gate.
pub fn outside_noise(noise: u8) -> u8 {
    noise.saturating_add(noise.div_ceil(2))
}

/// What a heat did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heat {
    Quiet,
    /// The owner's alarm went up with this heat.
    Alarm,
    /// The trace found you: you were dumped.
    Dumped,
}

/// Heat on the owner's trace with everything that follows. A building's goes the way
/// it always has (`net::noise_on`): its alarm locks it down, its dump dumps you. A
/// district's alarm starts its alert and says so; its dump dumps you.
pub fn heat_on(st: &mut AppState, o: Owner, noise: u8) -> Heat {
    let now = st.net.now(st);
    let was_alert = alert(st, o, now);
    match o {
        Owner::Building(b) => {
            if crate::net::noise_on(st, b, noise) {
                return Heat::Dumped;
            }
        }
        Owner::District(d) => {
            let cut = st.character.trace_cut(&st.content);
            let event = st
                .net
                .district_traces
                .entry(d)
                .or_default()
                .add_cut(noise, cut, now);
            if event == game::TraceEvent::Dump {
                crate::net::dump(st, "the trace found you");
                return Heat::Dumped;
            }
        }
    }
    if !was_alert && alert(st, o, now) {
        if let Owner::District(_) = o {
            let line = format!("ALARM · `{}` is looking for you", name(st, o));
            st.warn(line);
        }
        return Heat::Alarm;
    }
    Heat::Quiet
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{GenParams, Generator, NodeKind, SizePreset};

    fn state() -> AppState {
        let mut st = AppState::new();
        st.enter_city(
            Generator::run(GenParams {
                seed: 6,
                size: SizePreset::Custom { w: 120, h: 60 },
                districts: 5,
                ..Default::default()
            })
            .unwrap(),
        );
        st
    }

    #[test]
    fn owner_of_reads_the_subnet() {
        let st = state();
        let city = st.city.as_ref().unwrap();
        let lamp = city
            .net
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::LampGrid)
            .unwrap();
        assert!(matches!(owner_of(city, lamp.id), Some(Owner::District(_))));
        let cam = city
            .net
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Camera)
            .unwrap();
        assert_eq!(owner_of(city, cam.id), cam.building.map(Owner::Building));
    }

    #[test]
    fn a_district_trace_heats_alarms_and_cools() {
        let mut st = state();
        let d = st.city.as_ref().unwrap().districts[0].id;
        let o = Owner::District(d);
        assert_eq!(heat_on(&mut st, o, 30), Heat::Quiet);
        assert_eq!(heat(&st, o), 30);
        assert_eq!(heat_on(&mut st, o, 35), Heat::Alarm, "past the alarm line");
        let now = st.net.now(&st);
        assert!(alert(&st, o, now));
        assert!(name(&st, o).ends_with("(district)"));
        // An hour and more on, the alert is over and the heat gone.
        let later = now + game::ALERT_MINUTES + 2;
        let roots = st.backdoors.roots();
        st.net.minute(later, None, &mut st.constructs, &roots);
        assert!(!alert(&st, o, later));
        assert_eq!(heat(&st, o), 0);
    }

    #[test]
    fn outside_is_half_again() {
        assert_eq!(outside_noise(2), 3);
        assert_eq!(outside_noise(3), 5);
        assert_eq!(outside_noise(0), 0);
    }
}
