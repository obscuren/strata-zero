//! The city's clock as one step: the explore frame runs it each frame, and a rest runs
//! it a game minute at a time.

use crate::app::AppState;
use citygen::Place;

/// How many requested paths turn into trips per tick of the clock run: a frame's worth on
/// the street, and 360 a game minute however the clock runs.
pub const PATHS_PER_FRAME: usize = 4;

/// One step of the city's clock. With `Some(n)` the world advances `n` ticks and turns up
/// to `paths` requests a tick into trips; a step over a minute (an overworld step over
/// rough ground) is caught up as a city you come back to is (`sim::World::catch_up`), so
/// every trip leaves on its minute and nothing is left queued. With `None` it holds (the
/// free camera). Either way the bots, the Net's minute, the backdoors, the jobs, the
/// missions' copies and the story take their step.
pub fn run(st: &mut AppState, world_ticks: Option<u64>, paths: usize) {
    if let (Some(n), Some(city), Some(world)) = (world_ticks, st.city.as_ref(), st.world.as_mut()) {
        if n > sim::TICKS_PER_MINUTE {
            world.catch_up(city, world.tick() + n);
        } else {
            world.advance(city, world.tick() + n);
            world.path_some(city, paths.saturating_mul(n as usize));
        }
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
    crate::grid::tick(st);
    crate::jobs::tick(st);
    // A mission's copy taken or let go once you're out of its building.
    crate::venues::tick(st);
    // Out in the world the city holds still for you: its cameras look at no one, and
    // its story waits (a call keeps ringing from the start when you're back).
    if crate::outside::away(st) {
        let now = st.clock_tick();
        if let Some(c) = st.missions.call.as_mut().filter(|c| c.answered.is_none()) {
            c.rang = now;
        }
        return;
    }
    crate::cameras::tick(st);
    // in another city of the world the home city's story waits for you
    if !crate::outside::away_from_home(st) {
        crate::missions::tick(st);
    }
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
    fn an_hour_of_overworld_steps_leaves_the_city_nothing_queued() {
        // a flat tile's walk at a time (72 ticks), the live city run on as the overworld
        // runs it
        let mut st = crate::outside::tests::home_run();
        // seven in the morning, the commute starting, nothing queued
        let ahead = (7 * 60 + 1440 - st.minute_of_day() as u64) % 1440;
        let (city, w) = (st.city.as_ref().unwrap(), st.world.as_mut().unwrap());
        w.place_at(city, w.tick() + ahead * sim::TICKS_PER_MINUTE);
        for _ in 0..50 {
            run(&mut st, Some(72), PATHS_PER_FRAME);
        }
        assert_eq!(st.world.as_ref().unwrap().pending(), 0);
        // and a mountain river's walk (576 ticks) at a time
        for _ in 0..7 {
            run(&mut st, Some(576), PATHS_PER_FRAME);
        }
        assert_eq!(st.world.as_ref().unwrap().pending(), 0);
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
