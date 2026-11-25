//! Forcing a lock by Body: a bar fills over the container while you stand there, longer
//! for a better lock, shorter for more Body. The map runs it (`screens::explore`).

use super::{Container, ContainerAt, Status, find, open, state::Opened, status};
use crate::app::{AppState, FPS};
use citygen::{Place, Pos};
use game::Attribute;

/// The Body a lock of `grade` asks.
pub fn need(grade: u8) -> u8 {
    2 * grade
}

/// How long forcing a lock of `grade` takes with `body`, in ticks: 3, 6 or 10 s by grade,
/// 5% off for each point of Body over the need.
pub fn ticks(grade: u8, body: u8) -> u64 {
    let secs = match grade {
        0 | 1 => 3,
        2 => 6,
        _ => 10,
    };
    let over = body.saturating_sub(need(grade)) as u64;
    (secs * FPS * 100u64.saturating_sub(5 * over) / 100).max(1)
}

/// The ticks forcing `c` takes you, or why not.
pub fn can(st: &AppState, c: &Container) -> Result<u64, String> {
    let Status::Locked(lock) = status(st, c) else {
        return Err("not locked".to_string());
    };
    if st.net.engaged.is_some() {
        return Err("not now".to_string());
    }
    let body = st.character.attribute(&st.content, Attribute::Body);
    if body < need(lock.grade) {
        return Err(format!("Body {}", need(lock.grade)));
    }
    Ok(ticks(lock.grade, body))
}

/// A lock being forced: from when, for how long, and where you stand doing it.
#[derive(Clone, Debug, PartialEq)]
pub struct Forcing {
    pub at: ContainerAt,
    pub from: u64,
    pub ticks: u64,
    pub stand: (Place, Pos),
    /// A site job's objective worked by hand at `at` instead of a lock (#98).
    pub work: Option<crate::jobs::site::Work>,
}

impl Forcing {
    /// How far along it is, 0 to 1.
    pub fn frac(&self, tick: u64) -> f32 {
        (tick.saturating_sub(self.from) as f32 / self.ticks as f32).clamp(0.0, 1.0)
    }

    pub fn done(&self, tick: u64) -> bool {
        tick.saturating_sub(self.from) >= self.ticks
    }
}

/// The lock gives: it opens by force (setting off the alarm if it was on the Net).
pub fn finish(st: &mut AppState, at: ContainerAt) {
    let Some(c) = find(st, at) else { return };
    open(st, &c, Opened::Forced);
    st.say(format!("the {} gives", c.kind.word()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::containers::testutil::*;

    #[test]
    fn force_time_by_grade_and_body() {
        assert_eq!(need(1), 2);
        assert_eq!(need(3), 6);
        assert_eq!(ticks(1, 2), 3 * FPS);
        assert_eq!(ticks(2, 4), 6 * FPS);
        assert_eq!(ticks(3, 6), 10 * FPS);
        assert_eq!(ticks(1, 4), 3 * FPS * 90 / 100);
        assert_eq!(ticks(1, 255), 1);
    }

    #[test]
    fn below_the_body_force_is_refused() {
        let (mut st, c) = copy_with_spot(255);
        beside(&mut st, &c);
        let Status::Locked(lock) = status(&st, &c) else {
            panic!("locked")
        };
        st.character.attributes.body = 0;
        assert_eq!(can(&st, &c), Err(format!("Body {}", need(lock.grade))));
    }
}
