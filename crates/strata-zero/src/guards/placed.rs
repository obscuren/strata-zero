//! Where a building last placed you: the floor and the tile a guard, a floor camera or
//! your found body put you on, and the Net minute it did. An alarm sends its guards there
//! (`call::answer`) while the placing is fresh.

use crate::app::AppState;
use citygen::{BuildingId, Place, Pos};

/// How many Net minutes a placing counts for: older, an alarm has no spot to send its
/// guards to.
pub const PLACED_MINUTES: u64 = 5;

/// Where a building last placed you: on `level` at `at`, at Net minute `minute`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Placed {
    pub level: i32,
    pub at: Pos,
    pub minute: u64,
}

/// Building `b` places you on `level` at `at`, now.
pub(crate) fn place_you(st: &mut AppState, b: BuildingId, level: i32, at: Pos) {
    let minute = st.net.now(st);
    st.net.placed.insert(b, Placed { level, at, minute });
}

/// Building `b` places you where you stand, when that's one of its floors.
pub(crate) fn place_you_here(st: &mut AppState, b: BuildingId) {
    let here = st.player.as_ref().map(|p| (p.place, p.pos));
    if let Some((Place::Floor { building, level }, at)) = here
        && building == b
    {
        place_you(st, b, level, at);
    }
}

/// Where building `b` last placed you, while that counts: its level and tile.
pub(crate) fn placed(st: &AppState, b: BuildingId) -> Option<(i32, Pos)> {
    let p = st.net.placed.get(&b)?;
    (st.net.now(st) <= p.minute + PLACED_MINUTES).then_some((p.level, p.at))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guards::testutil::*;
    use citygen::Pos;

    #[test]
    fn a_guard_that_sees_you_places_you() {
        let mut st = ward();
        guard(&mut st, 0, 0, &[Pos::new(10, 3)]);
        assert_eq!(placed(&st, WARD), None);
        assert!(crate::foes::look_all(&mut st), "you stand in the lit hall");
        assert_eq!(placed(&st, WARD), Some((0, Pos::new(6, 3))));
    }

    #[test]
    fn a_placing_goes_stale_after_five_minutes() {
        let mut st = ward();
        place_you(&mut st, WARD, 1, Pos::new(20, 3));
        for _ in 0..PLACED_MINUTES {
            crate::sitenet::testutil::next_minute(&mut st);
        }
        assert_eq!(placed(&st, WARD), Some((1, Pos::new(20, 3))));
        crate::sitenet::testutil::next_minute(&mut st);
        assert_eq!(placed(&st, WARD), None);
    }

    #[test]
    fn a_floor_camera_places_you() {
        use crate::sitenet::testutil::{in_a_floor_cameras_cone, next_minute, office_copy};
        let (mut st, b) = office_copy(190);
        in_a_floor_cameras_cone(&mut st, b);
        let p = st.player.as_ref().unwrap();
        let Place::Floor { level, .. } = p.place else {
            panic!("on a floor");
        };
        let you = (level, p.pos);
        next_minute(&mut st);
        crate::cameras::tick(&mut st);
        assert_eq!(placed(&st, b), Some(you));
    }

    #[test]
    fn finding_your_body_places_you() {
        use crate::sitenet::testutil::{jacked_in_at_the_kiosk, office_copy};
        let (mut st, b) = office_copy(190);
        jacked_in_at_the_kiosk(&mut st, b);
        let (place, at) = st.player.as_ref().unwrap().body.unwrap();
        let Place::Floor { level, .. } = place else {
            panic!("the body lies on a floor");
        };
        guard_by_the_body(&mut st, b);
        crate::guards::tick(&mut st, 1);
        assert_eq!(placed(&st, b), Some((level, at)));
    }
}
