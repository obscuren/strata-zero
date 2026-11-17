//! Guards in a mission's copy of a venue: what only they do beside every other foe.

mod call;
mod doors;
mod hunt;
mod links;
mod placed;
mod round;

use crate::app::AppState;
use citygen::BuildingId;

pub(crate) use call::answer;
pub use doors::opens;
pub use hunt::hear;
pub(crate) use hunt::{drop_card, lose_you, release_follow, you_left};
pub(crate) use links::{forget_stale, free_landing};
pub use placed::Placed;
pub(crate) use placed::{place_you, place_you_here, placed};

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
/// A sweeping guard's step: between a run and an amble.
pub const SWEEP_STEP_TICKS: u64 = 2 * crate::net::WALK_TICKS;
/// How long a sweeping guard stands at each room it checks: a second at 30 fps.
pub const SWEEP_PAUSE_TICKS: u64 = 30;
/// How many rooms a sweep checks.
pub const SWEEP_ROOMS: usize = 3;

/// The most a guard does in one step of the clock: a rest's game minute is a few steps of
/// a walk, and this bounds a step that ran long.
const MOST_ACTIONS: usize = 64;

/// The guards' step on a clock that moved `ticks` ticks: a frame's 1, a rest's game
/// minute. Every building with a posted foe, in id order: while you (or your body) are in
/// it, each standing guard of it acts as often as its time comes in those ticks (at most
/// `MOST_ACTIONS`), each action at the tick it fell due, the step's first tick at the
/// earliest, so a minute's rest walks it as far as a minute of frames. One on the floor of
/// a fight, which the turns move, holds. A hunting one hunts (`hunt::hunt_step`), a calm
/// one the alarm sent somewhere goes (`call::call_step`), any other walks its round. Out of it, its guards lose you and hold. Then a guard that sees
/// the body you left jacked in finds it (`hunt::body_check`).
pub fn tick(st: &mut AppState, ticks: u64) {
    let mut buildings: Vec<BuildingId> = (st.foes.iter())
        .filter_map(|f| f.post.as_ref().map(|p| p.building))
        .collect();
    buildings.sort();
    buildings.dedup();
    let now = st.clock_tick();
    let first = now.saturating_sub(ticks.max(1) - 1);
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
            for _ in 0..MOST_ACTIONS {
                let f = &st.foes[i];
                let Some(due) = (f.post.as_ref()).filter(|_| ours(f)).map(|p| p.round.next) else {
                    break;
                };
                let held = (st.net.engaged.as_ref()).is_some_and(|e| e.place == f.place);
                if now < due || !f.standing() || held {
                    break;
                }
                let at = due.max(first);
                if f.hunting() {
                    hunt::hunt_step(st, i, at);
                } else if f.post.as_ref().is_some_and(|p| p.call.is_some()) {
                    call::call_step(st, i, at);
                } else {
                    round::round_step(st, i, at);
                }
            }
        }
    }
    hunt::body_check(st);
}

#[cfg(test)]
pub(crate) mod testutil;
