//! The daze (morning news spec §8): a few seconds where the map swims, then settles.

use crate::app::AppState;

/// Frame ticks a second.
const FRAMES_PER_SECOND: f32 = 30.0;

/// A daze running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Daze {
    /// The frame tick it began.
    pub start: u64,
    /// The frame ticks it lasts.
    pub ticks: u64,
}

/// Starts a daze of `seconds`, in place of any running.
pub fn start(st: &mut AppState, seconds: f32) {
    st.missions.daze = Some(Daze {
        start: st.tick,
        ticks: (seconds * FRAMES_PER_SECOND).round() as u64,
    });
}
