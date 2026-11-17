//! What an alarm sends a calm guard to do (`foes::Call`): run to where you were placed,
//! then sweep the rooms near where it got to, and go back to its round.

use super::round::{self, Step};
use super::{
    GUARD_RUN_TICKS, GUARD_STEP_TICKS, SWEEP_PAUSE_TICKS, SWEEP_ROOMS, SWEEP_STEP_TICKS, links,
};
use crate::app::AppState;
use crate::foes::Call;
use crate::render::map::TileMap;
use citygen::{Place, Pos};

/// The stops of a sweep by guard `i` of its own floor near `near`: of the rooms whose zone
/// its card opens, nearest first by their centre (the one holding `near` too), each one's
/// walkable tile nearest its centre that the guard can walk to; a stop on its own tile is
/// left out. At most `SWEEP_ROOMS`.
pub(crate) fn sweep_stops(st: &mut AppState, i: usize, near: Pos) -> Vec<Pos> {
    let (place, from) = (st.foes[i].place, st.foes[i].pos);
    let Place::Floor { building, level } = place else {
        return Vec::new();
    };
    if let Some(city) = st.city.as_ref() {
        st.interiors.ensure(city, building, level);
    }
    let (Some(post), Some(map), Some((_, floor))) = (
        st.foes[i].post.as_ref(),
        crate::foes::map_of(st, place),
        st.interiors.floor(building, level),
    ) else {
        return Vec::new();
    };
    let mut rooms: Vec<_> = (floor.rooms.iter())
        .filter(|r| r.zone <= post.zone)
        .collect();
    rooms.sort_by_key(|r| near.chebyshev(r.rect.center()));
    rooms
        .into_iter()
        .filter_map(|r| {
            let c = r.rect.center();
            let tile = (r.rect.iter())
                .filter(|p| map.walkable(*p))
                .min_by_key(|p| (p.chebyshev(c), p.y, p.x))?;
            (tile != from && links::walk(&map, post, from, tile).is_some()).then_some(tile)
        })
        .take(SWEEP_ROOMS)
        .collect()
}

/// Guard `i` sweeps its floor near `near` (`sweep_stops`); with nothing to check it has no
/// call and walks its round.
pub(crate) fn sweep(st: &mut AppState, i: usize, near: Pos) {
    let Place::Floor { level, .. } = st.foes[i].place else {
        return;
    };
    let stops = sweep_stops(st, i, near);
    if let Some(post) = st.foes[i].post.as_mut() {
        post.call = (!stops.is_empty()).then_some(Call::Sweep { level, stops });
    }
}

/// One action of calm guard `i` with a call, at clock tick `now`. Its building off alert,
/// the call ends. Waiting for a lift, it rides once the lift answers. Answering on another
/// level it heads there as its round heads for a stop (`links::link_to`) at a run; with no
/// way there the call ends. On the level it runs to the spot, and there (or beside it,
/// held, or with no way to it) it sweeps it. Sweeping, it walks to each stop in turn and
/// pauses on it; a stop with no way to it is dropped; out of stops the call ends.
pub(crate) fn call_step(st: &mut AppState, i: usize, now: u64) {
    links::forget_stale(st, i);
    let foe = &st.foes[i];
    let from = foe.pos;
    let (Some(post), Place::Floor { building, level }) = (foe.post.as_ref(), foe.place) else {
        return round::wait(st, i, now + GUARD_STEP_TICKS);
    };
    if !st.net.alert(building, st.net.now(st)) {
        return end(st, i, now);
    }
    if let Some((link, to)) = post.round.lift {
        return links::wait_for_lift(st, i, link, to, now);
    }
    let Some(call) = post.call.clone() else {
        return round::wait(st, i, now + GUARD_STEP_TICKS);
    };
    match call {
        Call::Answer { level: there, .. } if there != level => {
            if let Some(city) = st.city.as_ref() {
                st.interiors.ensure(city, building, level);
            }
            match links::link_to(st, i, there) {
                None => end(st, i, now),
                Some((link, at, hop)) if from == at => links::take(st, i, link, hop, now),
                Some((_, at, _)) => match round::step(st, i, building, level, at) {
                    Step::NoWay => end(st, i, now),
                    _ => round::wait(st, i, now + GUARD_RUN_TICKS),
                },
            }
        }
        Call::Answer { at, .. } => {
            if from == at {
                return sweep_from(st, i, at, now);
            }
            match round::step(st, i, building, level, at) {
                Step::NoWay => sweep_from(st, i, at, now),
                Step::Held if from.chebyshev(at) <= 1 => sweep_from(st, i, at, now),
                Step::To(to) if to == at => sweep_from(st, i, at, now),
                _ => round::wait(st, i, now + GUARD_RUN_TICKS),
            }
        }
        Call::Sweep { level: there, .. } if there != level => end(st, i, now),
        Call::Sweep { stops, .. } => {
            let Some(&next) = stops.first() else {
                return end(st, i, now);
            };
            if from == next {
                return checked(st, i, now + SWEEP_PAUSE_TICKS);
            }
            match round::step(st, i, building, level, next) {
                Step::NoWay => checked(st, i, now + SWEEP_STEP_TICKS),
                Step::To(to) if to == next => checked(st, i, now + SWEEP_PAUSE_TICKS),
                _ => round::wait(st, i, now + SWEEP_STEP_TICKS),
            }
        }
    }
}

/// Guard `i`'s call ends: it walks back into its round a step on from `now`.
fn end(st: &mut AppState, i: usize, now: u64) {
    if let Some(post) = st.foes[i].post.as_mut() {
        post.call = None;
    }
    round::wait(st, i, now + GUARD_STEP_TICKS);
}

/// Guard `i` got to the spot `at` it answered: it sweeps the rooms near it.
fn sweep_from(st: &mut AppState, i: usize, at: Pos, now: u64) {
    sweep(st, i, at);
    round::wait(st, i, now + SWEEP_PAUSE_TICKS);
}

/// Guard `i` is done with its sweep's first stop; it acts again at `next`.
fn checked(st: &mut AppState, i: usize, next: u64) {
    if let Some(Call::Sweep { stops, .. }) =
        (st.foes[i].post.as_mut()).and_then(|p| p.call.as_mut())
        && !stops.is_empty()
    {
        stops.remove(0);
    }
    round::wait(st, i, next);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guards::testutil::*;
    use citygen::{Pos, Rect};

    const HALL: Rect = Rect::new(1, 1, 14, 5);
    const CORRIDOR: Rect = Rect::new(16, 1, 8, 5);
    const OFFICE: Rect = Rect::new(25, 1, 6, 5);

    /// The ward on alert, you up on 2F in its far corner, out of every guard's way.
    fn on_alert() -> AppState {
        let mut st = ward();
        crate::net::lock_down(&mut st, WARD);
        stand(&mut st, 1, Pos::new(30, 5));
        st
    }

    fn call_of(st: &AppState, i: usize) -> Option<Call> {
        st.foes[i].post.as_ref().unwrap().call.clone()
    }

    #[test]
    fn sweep_stops_take_the_rooms_its_card_opens_nearest_first() {
        let mut st = on_alert();
        let i = guard(&mut st, 0, 2, &[Pos::new(3, 3)]);
        let stops = sweep_stops(&mut st, i, Pos::new(3, 3));
        assert_eq!(stops.len(), 3, "{stops:?}");
        assert!(HALL.contains(stops[0]), "{stops:?}");
        assert!(CORRIDOR.contains(stops[1]), "{stops:?}");
        assert!(OFFICE.contains(stops[2]), "{stops:?}");
        assert!(!stops.contains(&Pos::new(3, 3)));
    }

    #[test]
    fn a_zone_0_guard_sweeps_only_the_hall() {
        let mut st = on_alert();
        let i = guard(&mut st, 0, 0, &[Pos::new(3, 3)]);
        let stops = sweep_stops(&mut st, i, Pos::new(3, 3));
        assert_eq!(stops.len(), 1, "{stops:?}");
        assert!(HALL.contains(stops[0]));
    }

    #[test]
    fn a_sweeping_guard_checks_its_stops_then_walks_its_round() {
        let mut st = on_alert();
        let i = guard(&mut st, 0, 2, &[Pos::new(3, 3)]);
        sweep(&mut st, i, Pos::new(3, 3));
        let Some(Call::Sweep { level: 0, stops }) = call_of(&st, i) else {
            panic!("a sweep of L: {:?}", call_of(&st, i));
        };
        let mut stood = Vec::new();
        for _ in 0..3000 {
            run(&mut st, 1);
            stood.push(st.foes[i].pos);
            if call_of(&st, i).is_none() {
                break;
            }
        }
        assert_eq!(call_of(&st, i), None, "the sweep ends");
        assert!(stops.iter().all(|s| stood.contains(s)), "{stops:?}");
        assert!(!st.foes[i].hunting());
        run_until(&mut st, 3000, |st| st.foes[i].pos == Pos::new(3, 3));
    }

    #[test]
    fn an_answering_guard_rides_up_runs_to_the_spot_and_sweeps() {
        let mut st = on_alert();
        let i = guard(&mut st, 0, 1, &[Pos::new(10, 3)]);
        st.foes[i].post.as_mut().unwrap().call = Some(Call::Answer {
            level: 1,
            at: Pos::new(20, 3),
        });
        run_until(&mut st, 3000, |st| {
            matches!(
                st.foes[i].post.as_ref().unwrap().call,
                Some(Call::Sweep { level: 1, .. })
            )
        });
        assert_eq!(st.foes[i].place, on(1));
        assert_eq!(st.foes[i].pos, Pos::new(20, 3));
        assert!(!st.foes[i].hunting(), "it never saw you");
    }

    #[test]
    fn an_answer_runs_at_a_run() {
        let mut st = on_alert();
        let i = guard(&mut st, 0, 0, &[Pos::new(2, 3)]);
        st.foes[i].post.as_mut().unwrap().call = Some(Call::Answer {
            level: 0,
            at: Pos::new(10, 3),
        });
        run(&mut st, 4 * crate::guards::GUARD_RUN_TICKS);
        assert_eq!(st.foes[i].pos, Pos::new(6, 3));
    }

    #[test]
    fn a_call_ends_with_its_alert() {
        let mut st = on_alert();
        let i = guard(&mut st, 0, 2, &[Pos::new(3, 3)]);
        sweep(&mut st, i, Pos::new(3, 3));
        assert!(call_of(&st, i).is_some());
        let t = st.net.trace(WARD);
        t.alert_until = None;
        t.heat = 0;
        run(&mut st, 1);
        assert_eq!(call_of(&st, i), None);
    }

    #[test]
    fn the_footer_reads_answering_and_sweeping() {
        let mut st = ward();
        let i = guard(&mut st, 0, 0, &[Pos::new(10, 3)]);
        st.foes[i].post.as_mut().unwrap().call = Some(Call::Answer {
            level: 0,
            at: Pos::new(20, 3),
        });
        assert_eq!(
            crate::foes::lines(&st, |_| true),
            vec!["▣ guard · answering · 30/30 · 4 tiles"]
        );
        st.foes[i].post.as_mut().unwrap().call = Some(Call::Sweep {
            level: 0,
            stops: vec![Pos::new(8, 3)],
        });
        assert_eq!(
            crate::foes::lines(&st, |_| true),
            vec!["▣ guard · sweeping · 30/30 · 4 tiles"]
        );
    }
}
