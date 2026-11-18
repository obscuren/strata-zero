//! The calm round: a guard walks its patrol out and back, then its stop, on the clock.

use super::links;
use super::{GUARD_PAUSE_TICKS, GUARD_STEP_TICKS};
use crate::app::AppState;
use crate::foes::{self, Foe, Post};
use citygen::{BuildingId, Place, Pos};

/// A guard's round as goals, each a level and a tile: its patrol out on `home`, back the
/// way it came (the ends left out), then its stop on another level if it has one. Its
/// round's `leg` indexes these, and wraps to 0, its post.
pub fn goals(post: &Post, home: i32) -> Vec<(i32, Pos)> {
    let n = post.patrol.len();
    let out = post.patrol.iter().map(|p| (home, *p));
    let back = (1..n.saturating_sub(1))
        .rev()
        .map(|k| (home, post.patrol[k]));
    out.chain(back).chain(post.stop).collect()
}

/// The level a guard's round walks: its post's, else the level it stands on.
pub(crate) fn home(foe: &Foe) -> i32 {
    let here = match foe.place {
        Place::Floor { level, .. } => level,
        _ => 0,
    };
    foe.post.as_ref().and_then(|p| p.home).unwrap_or(here)
}

/// The tiles foe `i` doesn't step onto, on the place it stands: yours, your body's,
/// and every other standing foe's.
pub(crate) fn avoid(st: &AppState, i: usize) -> Vec<Pos> {
    let place = st.foes[i].place;
    let you = st.player.as_ref().into_iter().flat_map(|p| {
        let body = p.body.filter(|(at, _)| *at == place).map(|(_, pos)| pos);
        (p.place == place).then_some(p.pos).into_iter().chain(body)
    });
    let others = (st.foes.iter().enumerate())
        .filter(|(j, f)| *j != i && f.standing() && f.place == place)
        .map(|(_, f)| f.pos);
    you.chain(others).collect()
}

/// One calm action of guard `i` at clock tick `now`. Waiting for a lift, it rides once
/// the lift answers. Standing on its goal, it takes the next leg and pauses; else it steps
/// a tile toward the goal round the tiles held, or along the way that ignores them when
/// there's none round (waiting while that tile is held), and a step that lands on the goal
/// takes the next leg and pauses at once. A goal on another level it walks to by the link
/// `links::link_to` picks, and takes that link standing on its stop, a hop at a time when
/// no link goes there in one. A goal (or a link)
/// with no way at all is skipped. Off a floor, or with no map, it waits a step. A saved
/// link that no longer stops on its level is let go first (`links::forget_stale`).
pub(crate) fn round_step(st: &mut AppState, i: usize, now: u64) {
    links::forget_stale(st, i);
    let foe = &st.foes[i];
    let (Some(post), Place::Floor { building, level }) = (foe.post.as_ref(), foe.place) else {
        return wait(st, i, now + GUARD_STEP_TICKS);
    };
    if let Some((link, to)) = post.round.lift {
        return links::wait_for_lift(st, i, link, to, now);
    }
    let goals = goals(post, home(foe));
    let Some(&(goal_level, goal)) = goals.get(post.round.leg % goals.len().max(1)) else {
        return wait(st, i, now + GUARD_STEP_TICKS);
    };
    let n = goals.len();
    let from = foe.pos;
    if goal_level != level {
        // The link is found on its own floor's map: drawn, though it hasn't stepped yet.
        if let Some(city) = st.city.as_ref() {
            st.interiors.ensure(city, building, level);
        }
        return match links::link_to(st, i, goal_level) {
            None => next_leg(st, i, n, now + GUARD_STEP_TICKS),
            Some((link, at, hop)) if from == at => links::take(st, i, link, hop, now),
            Some((_, at, _)) => match step(st, i, building, level, at) {
                Step::NoWay => next_leg(st, i, n, now + GUARD_STEP_TICKS),
                Step::NoMap | Step::Held | Step::To(_) => wait(st, i, now + GUARD_STEP_TICKS),
            },
        };
    }
    if from == goal {
        return next_leg(st, i, n, now + GUARD_PAUSE_TICKS);
    }
    match step(st, i, building, level, goal) {
        Step::NoWay => next_leg(st, i, n, now + GUARD_STEP_TICKS),
        Step::To(to) if to == goal => next_leg(st, i, n, now + GUARD_PAUSE_TICKS),
        Step::NoMap | Step::Held | Step::To(_) => wait(st, i, now + GUARD_STEP_TICKS),
    }
}

/// What a guard's step toward a tile came to.
pub(super) enum Step {
    /// Its floor has no map: it waits.
    NoMap,
    /// No way there at all.
    NoWay,
    /// The way's next tile is held: it waits, never sharing.
    Held,
    /// It stepped onto this tile.
    To(Pos),
}

/// Guard `i`, on level `level` of `building`, steps a tile toward `target` round the
/// tiles held, else along the way that ignores them when that tile is free. Its floor is
/// drawn when needed.
pub(super) fn step(
    st: &mut AppState,
    i: usize,
    building: BuildingId,
    level: i32,
    target: Pos,
) -> Step {
    if let Some(city) = st.city.as_ref() {
        st.interiors.ensure(city, building, level);
    }
    let from = st.foes[i].pos;
    let held = avoid(st, i);
    let next = {
        let Some(map) = foes::map_of(st, st.foes[i].place) else {
            return Step::NoMap;
        };
        let post = st.foes[i].post.as_ref();
        foes::step_toward(&map, from, target, &held, post)
            .or_else(|| foes::step_toward(&map, from, target, &[], post))
    };
    match next {
        None => Step::NoWay,
        Some(to) if held.contains(&to) => Step::Held,
        Some(to) => {
            st.foes[i].pos = to;
            Step::To(to)
        }
    }
}

/// Guard `i` takes the next of its `n` legs and acts again at `next`.
fn next_leg(st: &mut AppState, i: usize, n: usize, next: u64) {
    if let Some(post) = st.foes[i].post.as_mut() {
        post.round.leg = (post.round.leg + 1) % n.max(1);
        post.round.next = next;
    }
}

/// Guard `i` acts again at `next`.
pub(super) fn wait(st: &mut AppState, i: usize, next: u64) {
    if let Some(post) = st.foes[i].post.as_mut() {
        post.round.next = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guards::testutil::*;
    use crate::guards::{GUARD_PAUSE_TICKS, GUARD_STEP_TICKS};
    use citygen::{Place, Pos};

    #[test]
    fn goals_walk_the_patrol_out_and_back_then_the_stop() {
        let (a, b, c, d, s) = (
            Pos::new(1, 1),
            Pos::new(2, 1),
            Pos::new(3, 1),
            Pos::new(4, 1),
            Pos::new(9, 9),
        );
        let mut p = post(0, &[a, b, c, d]);
        assert_eq!(
            goals(&p, 0),
            vec![(0, a), (0, b), (0, c), (0, d), (0, c), (0, b)]
        );
        p.stop = Some((1, s));
        assert_eq!(goals(&p, 0).last(), Some(&(1, s)));
        assert_eq!(goals(&post(0, &[a]), 0), vec![(0, a)]);
    }

    #[test]
    fn a_guard_walks_its_round_a_step_every_twelve_ticks_and_pauses() {
        let mut st = ward();
        stand(&mut st, 1, Pos::new(20, 3)); // in the building, out of its sight
        let i = guard(&mut st, 0, 0, &[Pos::new(2, 3), Pos::new(10, 3)]);
        run(&mut st, 1); // at its post: the pause
        run(&mut st, GUARD_PAUSE_TICKS);
        assert_eq!(st.foes[i].pos, Pos::new(3, 3));
        run(&mut st, GUARD_STEP_TICKS - 1);
        assert_eq!(st.foes[i].pos, Pos::new(3, 3), "not yet");
        run(&mut st, 1);
        assert_eq!(st.foes[i].pos, Pos::new(4, 3));
        run(&mut st, 6 * GUARD_STEP_TICKS);
        assert_eq!(st.foes[i].pos, Pos::new(10, 3), "there");
        run(&mut st, GUARD_PAUSE_TICKS);
        assert_eq!(
            st.foes[i].pos,
            Pos::new(9, 3),
            "paused, and on its way back"
        );
    }

    #[test]
    fn the_clock_walks_a_calm_guard_off_its_post() {
        let mut st = ward();
        stand(&mut st, 1, Pos::new(20, 3));
        let i = guard(&mut st, 0, 0, &[Pos::new(2, 3), Pos::new(10, 3)]);
        for _ in 0..(1 + GUARD_PAUSE_TICKS + GUARD_STEP_TICKS) {
            st.tick += 1; // the app's frame; the living city's, when there is one, runs below
            crate::clock::run(&mut st, Some(1), 0);
        }
        assert_ne!(st.foes[i].pos, Pos::new(2, 3), "off its post");
    }

    #[test]
    fn a_rest_walks_a_calm_guard_as_far_as_the_frames_would() {
        let walked = |rest: bool| {
            let mut st = ward();
            stand(&mut st, 0, Pos::new(28, 3)); // in the private room, out of sight
            let i = guard(&mut st, 1, 0, &[Pos::new(1, 3), Pos::new(30, 3)]);
            let (minutes, frame) = (3, sim::TICKS_PER_MINUTE);
            if rest {
                // A rest runs the clock a game minute at a time.
                for _ in 0..minutes {
                    st.tick += frame;
                    crate::clock::run(&mut st, Some(frame), crate::clock::PATHS_PER_FRAME);
                }
            } else {
                for _ in 0..minutes * frame {
                    st.tick += 1;
                    crate::clock::run(&mut st, Some(1), crate::clock::PATHS_PER_FRAME);
                }
            }
            st.foes[i].pos
        };
        let (awake, asleep) = (walked(false), walked(true));
        assert!(awake.x > 10, "it walks in play: {awake:?}");
        assert_eq!(asleep, awake);
    }

    #[test]
    fn guards_hold_while_you_are_out_of_the_building() {
        let mut st = ward();
        let i = guard(&mut st, 0, 0, &[Pos::new(2, 3), Pos::new(10, 3)]);
        st.player.as_mut().unwrap().place = Place::Stratum(citygen::StratumKind::Street);
        run(&mut st, 400);
        assert_eq!(st.foes[i].pos, Pos::new(2, 3));
    }

    #[test]
    fn a_calm_guard_holds_in_a_fight_on_its_floor_and_walks_in_one_elsewhere() {
        let mut st = ward();
        stand(&mut st, 1, Pos::new(20, 3));
        let i = guard(&mut st, 0, 0, &[Pos::new(2, 3), Pos::new(10, 3)]);
        crate::net::engage(&mut st, on(0), true); // a fight on its floor (as when you were there)
        run(&mut st, 400);
        assert_eq!(st.foes[i].pos, Pos::new(2, 3));
        crate::net::disengage(&mut st);
        crate::net::engage(&mut st, on(1), true);
        run(&mut st, GUARD_PAUSE_TICKS + GUARD_STEP_TICKS + 1);
        assert_ne!(st.foes[i].pos, Pos::new(2, 3));
    }

    #[test]
    fn an_unreachable_spot_is_skipped_and_nothing_reachable_stands() {
        let mut st = ward();
        stand(&mut st, 1, Pos::new(20, 3));
        let i = guard(
            &mut st,
            0,
            0,
            &[Pos::new(2, 3), Pos::new(20, 3), Pos::new(6, 3)],
        );
        run(&mut st, 1 + GUARD_PAUSE_TICKS + 2 * GUARD_STEP_TICKS);
        assert_eq!(st.foes[i].pos.y, 3);
        assert!(
            st.foes[i].pos.x > 2 && st.foes[i].pos.x <= 6,
            "on to (6,3), past the east hall"
        );
        let j = guard(&mut st, 0, 0, &[Pos::new(20, 4)]);
        st.foes[j].pos = Pos::new(10, 5);
        run(&mut st, 300);
        assert_eq!(st.foes[j].pos, Pos::new(10, 5), "nothing it can reach");
    }

    #[test]
    fn a_guard_waits_for_you_to_clear_its_way() {
        let mut st = ward();
        stand(&mut st, 0, Pos::new(15, 3)); // in the only doorway west
        let i = guard(&mut st, 0, 1, &[Pos::new(18, 3), Pos::new(10, 3)]);
        st.foes[i].post.as_mut().unwrap().round.leg = 1; // on its way west
        run(&mut st, 6 * GUARD_STEP_TICKS);
        assert_eq!(
            st.foes[i].pos,
            Pos::new(16, 3),
            "it waits at the door, never shares"
        );
        stand(&mut st, 0, Pos::new(12, 5));
        run(&mut st, 3 * GUARD_STEP_TICKS);
        assert!(st.foes[i].pos.x < 15, "through once you're clear");
    }

    #[test]
    fn a_guard_on_a_floor_not_drawn_yet_walks() {
        let (mut st, b) = crate::sitenet::testutil::office_copy(crate::venues::SITE_NET_SECURITY);
        let i = (st.foes.iter())
            .position(|f| {
                f.post
                    .as_ref()
                    .is_some_and(|p| p.building == b && p.patrol.len() > 1)
                    && matches!(f.place, Place::Floor { level, .. } if level != 0)
            })
            .expect("a walking guard off L");
        let Place::Floor { level, .. } = st.foes[i].place else {
            unreachable!()
        };
        st.interiors.drop_floors(b);
        let city = st.city.clone().unwrap();
        let floor = st.interiors.ensure(&city, b, 0).unwrap();
        let door = floor.doorstep(&floor.exits[0]);
        crate::sitenet::testutil::stand_on_floor(&mut st, b, 0, door); // you on L, inside
        let from = st.foes[i].pos;
        run(&mut st, 1 + GUARD_PAUSE_TICKS + GUARD_STEP_TICKS);
        assert!(st.interiors.floor(b, level).is_some(), "drawn for it");
        assert_ne!(st.foes[i].pos, from);
    }

    #[test]
    fn a_guard_that_stands_its_post_on_a_floor_not_drawn_yet_walks_to_its_stop() {
        let (mut st, b) = crate::sitenet::testutil::office_copy(crate::venues::SITE_NET_SECURITY);
        let i = (st.foes.iter())
            .position(|f| {
                f.post.as_ref().is_some_and(|p| p.building == b)
                    && matches!(f.place, Place::Floor { level, .. } if level != 0)
            })
            .expect("a guard off L");
        let Place::Floor { level, .. } = st.foes[i].place else {
            unreachable!()
        };
        // Its stop: the far end of the first link off its level its card rides.
        let plan = crate::interiors::plan_now(&st, b).unwrap().into_owned();
        let mut it = st.foes[i].clone();
        let post = it.post.as_mut().unwrap();
        let (to, at) = (0..plan.links.len())
            .find_map(|k| {
                let to = *links::hops(post, &plan, k, level).first()?;
                Some((to, plan.links[k].stop(to)?.at))
            })
            .expect("a link off its level");
        // One spot and the stop, its round from the start.
        post.patrol = vec![it.pos];
        post.stop = Some((to, at));
        post.round = Default::default();
        // Alone: no other guard's walk draws its floor for it.
        no_guards(&mut st, b);
        st.foes.push(it);
        let i = st.foes.len() - 1;
        st.interiors.drop_floors(b);
        let city = st.city.clone().unwrap();
        let floor = st.interiors.ensure(&city, b, 0).unwrap();
        let door = floor.doorstep(&floor.exits[0]);
        crate::sitenet::testutil::stand_on_floor(&mut st, b, 0, door); // you on L, inside
        assert!(
            st.interiors.floor(b, level).is_none(),
            "its own floor not drawn"
        );
        let stop = Place::Floor {
            building: b,
            level: to,
        };
        run_until(&mut st, 5000, |st| {
            (st.foes[i].place, st.foes[i].pos) == (stop, at)
        });
    }
}
