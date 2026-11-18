//! The ways in that cost nothing at the panel: a card of the right zone, a live crack on
//! its node; and where you stand to reach a container at all.

use super::state::Opened;
use super::{Container, ContainerAt, Lock, layout_of, seed, state, state_mut};
use crate::app::AppState;
use citygen::Pos;
use citygen::rng::hash64;

/// The way in that needs no choice: a live crack on its node, else a card of its zone or
/// deeper.
pub fn free_way(st: &AppState, c: &Container, lock: &Lock) -> Option<Opened> {
    if lock
        .node
        .is_some_and(|n| crate::sitenet::container_open(st, n))
    {
        return Some(Opened::Net);
    }
    let need = card_need(c);
    (crate::credentials::zone_card_held(st, c.at.building)? >= need).then_some(Opened::Card)
}

/// The zone a locked container's card is of: its room's, at least 1.
fn card_need(c: &Container) -> u8 {
    c.zone.unwrap_or(0).max(1)
}

/// The card a locked container takes (its zone, at least 1) and that zone's name in
/// capitals; `None` off a venue.
pub fn card_zone(st: &AppState, c: &Container) -> Option<(u8, String)> {
    let need = card_need(c);
    Some((need, zone_name(st, c.at.building, need)?))
}

/// Zone `zone` of building `b`'s venue, named in capitals.
pub fn zone_name(st: &AppState, b: citygen::BuildingId, zone: u8) -> Option<String> {
    let name = layout_of(st, b)?.zones.get(usize::from(zone))?;
    Some(name.to_uppercase())
}

/// Whether you, at `you`, are at it: a floor-standing one 4-beside you, a wall-set one
/// beside its wall tile and the tile in front of it, never through its wall from behind
/// (a terminal's rule, `sitenet::ways::at_port`).
pub fn reach(c: &Container, you: Pos) -> bool {
    match c.front {
        None => c.at.at.neighbors4().any(|n| n == you),
        Some(front) => c.at.at.chebyshev(you) <= 1 && front.chebyshev(you) <= 1,
    }
}

/// The faults a Keyjack has run up on a container's lock and the minute its lockout
/// ends, as at the game minute now: one whose lockout has ended starts over.
pub fn keyjack_now(st: &AppState, at: ContainerAt) -> (u8, Option<u64>) {
    let now = st.net.now(st);
    let Some(s) = state(st, at) else {
        return (0, None);
    };
    match s.locked_until {
        Some(until) if until <= now => (0, None),
        until => (s.faults, until),
    }
}

/// One more fault on the lock, kept until it opens or a lockout ends. The faults now.
pub fn fault(st: &mut AppState, at: ContainerAt) -> u8 {
    let (faults, until) = keyjack_now(st, at);
    let faults = faults.saturating_add(1);
    let s = state_mut(st, at);
    s.faults = faults;
    s.locked_until = until;
    faults
}

/// The Keyjack is locked out of this lock for `LOCKOUT_MINUTES`; its faults stay until
/// then.
pub fn lock_out(st: &mut AppState, at: ContainerAt) {
    let (faults, _) = keyjack_now(st, at);
    let until = st.net.now(st) + crate::credentials::LOCKOUT_MINUTES;
    let s = state_mut(st, at);
    s.faults = faults;
    s.locked_until = Some(until);
}

/// The seed a Keyjack's zones sit by on this container's lock.
pub fn keyjack_seed(st: &AppState, c: &Container) -> u64 {
    hash64(seed(st, c), 0x4B1A_C0DE)
}

/// The lock's four digits, as a number: the code a Keyjack keys in a beat at a time.
pub fn lock_code(st: &AppState, c: &Container) -> u16 {
    (hash64(seed(st, c), 0xC0DE) % 10_000) as u16
}
