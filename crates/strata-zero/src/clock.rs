//! The city's clock as one step: the explore frame runs it each frame, and a rest runs
//! it a game minute at a time.

use crate::app::AppState;
use citygen::Place;

/// How many requested paths turn into trips in one step.
pub const PATHS_PER_FRAME: usize = 4;

/// One step of the city's clock. With `Some(n)` the world advances `n` ticks and turns up
/// to `paths` requests into trips; with `None` it holds (the free camera). Either way the
/// bots, the Net's minute, the backdoors, the jobs and the story take their step.
pub fn run(st: &mut AppState, world_ticks: Option<u64>, paths: usize) {
    if let (Some(n), Some(city), Some(world)) = (world_ticks, st.city.as_ref(), st.world.as_mut()) {
        world.advance(city, world.tick() + n);
        world.path_some(city, paths);
    }
    // The bots out with you move on the same clock.
    crate::bots::tick(st);
    // The Net's clock: traces cool, hacks expire, alerts end.
    let now = st.net.now(st);
    let inside = match st.player.as_ref().map(|p| p.place) {
        Some(Place::Construct(b)) => Some(b),
        _ => None,
    };
    let roots = st.backdoors.roots();
    st.net.minute(now, inside, &mut st.constructs, &roots);
    crate::net::tick_backdoors(st);
    crate::holds::tick(st);
    crate::cameras::tick(st);
    crate::grid::tick(st);
    crate::jobs::tick(st);
    crate::missions::tick(st);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::testutil::state_home;
    use crate::missions::Book;

    fn book(missions: &str) -> Book {
        Book::parse(&format!(
            r#"Story(
                cast: {{
                    "mezz": Role(name: "Mezz", title: "fixer", pick: NearestFixer, face: None),
                }},
                faces: {{
                    "silhouette": Face(rows: ["......dd........", "....dmmmmd......"], eye: (255, 255, 255)),
                }},
                missions: [{missions}],
            )"#
        ))
        .unwrap()
    }

    fn timed() -> AppState {
        let mut st = state_home();
        st.book = book(
            r#"Mission(id: "wait", title: "Wait", from: "mezz", steps: [
                Step(id: "start", when: NewGame, then: [Objective("Wait")]),
                Step(id: "later", when: After(minutes: 1), then: [Log("a minute")]),
            ])"#,
        );
        crate::missions::begin(&mut st);
        st
    }

    #[test]
    fn run_moves_the_world_and_the_story() {
        let mut st = timed();
        let from = st.clock_tick();
        run(&mut st, Some(sim::TICKS_PER_MINUTE), PATHS_PER_FRAME);
        run(&mut st, Some(sim::TICKS_PER_MINUTE), PATHS_PER_FRAME);
        assert_eq!(st.clock_tick(), from + 2 * sim::TICKS_PER_MINUTE);
        assert_eq!(st.log.iter().filter(|e| e.text == "a minute").count(), 1);
    }

    #[test]
    fn none_holds_the_world() {
        let mut st = timed();
        let from = st.clock_tick();
        run(&mut st, None, PATHS_PER_FRAME);
        assert_eq!(st.clock_tick(), from);
    }
}
