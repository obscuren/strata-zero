//! Your rig's own Net: the room you land in jacking in at home (`Place::Rig`), laid out
//! from the rig as it stands each time you go in and walked as a construct is.

pub mod layout;
pub mod reader;
#[cfg(test)]
pub(crate) mod testutil;
pub mod ways;
pub use layout::{Counts, RigRoom, lay_out};

use crate::app::AppState;

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
    st.constructs.rig = st.home.map(|h| lay_out(counts(st), h.home.building));
}

/// The room as last laid out.
pub fn room(st: &AppState) -> Option<&RigRoom> {
    st.constructs.rig.as_ref()
}
