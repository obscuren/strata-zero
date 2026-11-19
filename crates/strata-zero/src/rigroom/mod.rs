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
        let standing: Vec<BuildingId> = st.backdoors.standing().map(|d| d.building).collect();
        fill(&mut room.door_for, &standing);
        room
    });
}

/// Gives each free slot, in order, a standing backdoor no slot holds yet.
fn fill(slots: &mut [Option<BuildingId>], standing: &[BuildingId]) {
    let held: Vec<BuildingId> = slots.iter().flatten().copied().collect();
    let mut spare = standing.iter().filter(|b| !held.contains(b));
    for slot in slots.iter_mut().filter(|s| s.is_none()) {
        *slot = spare.next().copied();
    }
}

/// What each door leads out to, top to bottom, read from the game as it stands: a
/// door keeps its backdoor while it stands, a burned one goes dark, and a free line
/// takes a backdoor planted since (`refresh` records it).
pub fn door_lines(st: &AppState) -> Vec<Option<BuildingId>> {
    let Some(room) = room(st) else {
        return Vec::new();
    };
    let mut slots: Vec<Option<BuildingId>> = (room.door_for.iter())
        .map(|b| b.filter(|b| st.backdoors.live(*b).is_some()))
        .collect();
    let standing: Vec<BuildingId> = st.backdoors.standing().map(|d| d.building).collect();
    fill(&mut slots, &standing);
    slots
}

/// Brings the room in step with the backdoors you hold, as it is about to be walked or
/// drawn: a backdoor planted since you came in takes a free door and keeps it; with
/// more backdoors than doors the room is laid out again with one for each, you left
/// where you stand if that is still floor, else on the landing.
pub fn refresh(st: &mut AppState) {
    let Some(doors) = room(st).map(|r| r.doors.len()) else {
        return;
    };
    let lines = door_lines(st);
    if st.backdoors.standing().count() > doors {
        let old = lines;
        build(st);
        if let Some(room) = st.constructs.rig.as_mut() {
            // The doors you knew keep their backdoors; the new ones take the rest.
            let standing: Vec<BuildingId> = st.backdoors.standing().map(|d| d.building).collect();
            let mut slots = vec![None; room.doors.len()];
            slots[..old.len()].copy_from_slice(&old);
            fill(&mut slots, &standing);
            room.door_for = slots;
        }
        keep_player(st);
    } else if let Some(room) = st.constructs.rig.as_mut() {
        room.door_for = lines;
    }
}

/// After the room is laid out again: you stay put on a floor tile of it, else you are on
/// the landing.
pub fn keep_player(st: &mut AppState) {
    let Some(land) = room(st).map(|r| r.land) else {
        return;
    };
    let pos = st.player.as_ref().map(|p| p.pos);
    let stands = room(st)
        .zip(pos)
        .is_some_and(|(r, pos)| r.construct.tiles.get(pos).is_some_and(|t| t.is_walkable()));
    if !stands {
        crate::constructs::land(st, citygen::Place::Rig, land);
    }
}

/// The room as last laid out.
pub fn room(st: &AppState) -> Option<&RigRoom> {
    st.constructs.rig.as_ref()
}
