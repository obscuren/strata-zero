//! The ways in that cost nothing at the panel: a card of the right zone, a live crack on
//! its node; and where you stand to reach a container at all.

use super::state::Opened;
use super::{Container, Lock, layout_of};
use crate::app::AppState;
use citygen::Pos;

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
