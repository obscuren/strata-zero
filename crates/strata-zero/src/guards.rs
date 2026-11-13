//! Guards in a mission's copy of a venue: what only they do beside every other foe.

mod doors;
mod hunt;
mod links;
mod round;

use crate::app::AppState;
use citygen::BuildingId;

pub use doors::opens;
pub use hunt::hear;
pub(crate) use hunt::you_left;
pub(crate) use links::{forget_stale, free_landing};

/// A calm guard's step: an amble, three times your walk.
pub const GUARD_STEP_TICKS: u64 = 3 * crate::net::WALK_TICKS;
/// A hunting guard's step off a fight: your walk.
pub const GUARD_RUN_TICKS: u64 = crate::net::WALK_TICKS;
/// How long a guard stands at each spot of its round: three seconds at 30 fps.
pub const GUARD_PAUSE_TICKS: u64 = 90;
/// How long a guard waits for a lift.
pub const GUARD_LIFT_TICKS: u64 = 60;
/// How far a shot wakes the calm guards of its floor, walls or not.
pub const HEAR_RADIUS: i32 = 12;

/// The guards' step on the clock. Every building with a posted foe, in id order: while
/// you (or your body) are in it, each standing guard of it whose time has come acts, but
/// one on the floor of a fight, which the turns move: a calm one walks its round, a
/// hunting one hunts (`hunt::hunt_step`). Out of it, its guards lose you and hold.
pub fn tick(st: &mut AppState) {
    let mut buildings: Vec<BuildingId> = (st.foes.iter())
        .filter_map(|f| f.post.as_ref().map(|p| p.building))
        .collect();
    buildings.sort();
    buildings.dedup();
    let now = st.clock_tick();
    for b in buildings {
        let ours = |f: &crate::foes::Foe| f.post.as_ref().is_some_and(|p| p.building == b);
        if !crate::venues::inside(st, b) {
            for i in 0..st.foes.len() {
                if ours(&st.foes[i]) && st.foes[i].hunting() {
                    hunt::lose_you(st, i, now);
                }
            }
            continue;
        }
        for i in 0..st.foes.len() {
            let f = &st.foes[i];
            let due = ours(f) && f.post.as_ref().is_some_and(|p| now >= p.round.next);
            let held = (st.net.engaged.as_ref()).is_some_and(|e| e.place == f.place);
            if !due || !f.standing() || held {
                continue;
            }
            if f.hunting() {
                hunt::hunt_step(st, i, now);
            } else {
                round::round_step(st, i, now);
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod testutil;
