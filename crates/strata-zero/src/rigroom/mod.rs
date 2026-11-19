//! Your rig's own Net: the room you land in jacking in at home (`Place::Rig`), laid out
//! from the rig as it stands each time you go in and walked as a construct is.

pub mod layout;
pub mod reader;
#[cfg(test)]
pub(crate) mod testutil;
pub mod ways;
pub use layout::{Counts, RigRoom, lay_out};

use crate::app::AppState;
use citygen::BuildingId;

/// What the HUD's place line and the footer's first row read in the room.
pub const PLACE_LINE: &str = "zenbo · your rig";

/// What the room is laid out for: a door for each of the relay's lines, or for each
/// standing backdoor where you hold more (lowering the relay burns nothing); the core's
/// seats; the rack's reader slots; the firewall's tier.
pub fn counts(st: &AppState) -> Counts {
    let rig = crate::rig::of(st);
    Counts {
        lines: rig.lines.max(st.backdoors.standing().count()),
        seats: rig.seats,
        slots: st.character.reader_slots(&st.content),
        firewall: rig.firewall,
    }
}

/// Lays the room out afresh from the rig into `st.constructs.rig`: on the way in, and
/// for a run reopened inside it. None without a home.
pub fn build(st: &mut AppState) {
    st.constructs.rig = st.home.map(|h| {
        let mut room = lay_out(counts(st), h.home.building);
        // Doors take the standing backdoors in order; the lines left over stay free.
        for (slot, d) in room.door_for.iter_mut().zip(st.backdoors.standing()) {
            *slot = Some(d.building);
        }
        room
    });
}

/// What each door leads out to, top to bottom: `Some(b)` a backdoor that still stands
/// (the room's own, laid out in the order the backdoors are listed), `None` for a free
/// line, or a door whose backdoor burned since you came in (it goes dark).
pub fn door_lines(st: &AppState) -> Vec<Option<BuildingId>> {
    let Some(room) = room(st) else {
        return Vec::new();
    };
    (room.door_for.iter())
        .map(|b| b.filter(|b| st.backdoors.live(*b).is_some()))
        .collect()
}

/// The room as last laid out.
pub fn room(st: &AppState) -> Option<&RigRoom> {
    st.constructs.rig.as_ref()
}
