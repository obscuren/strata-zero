//! What an alarm sends a calm guard to do (`foes::Call`): run to where you were placed,
//! then sweep the rooms near where it got to, and go back to its round.

use super::round::{self, Step};
use super::{
    GUARD_RUN_TICKS, GUARD_STEP_TICKS, SWEEP_PAUSE_TICKS, SWEEP_ROOMS, SWEEP_STEP_TICKS, links,
};
use crate::app::AppState;
use crate::foes::{Call, FoeMode};
use crate::render::map::TileMap;
use citygen::{BuildingId, Place, Pos};

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

/// How many calm guards from other floors answer an alarm with a spot.
const FROM_OTHER_FLOORS: usize = 2;

/// Building `b`'s alarm has just gone off: its calm standing guards answer, while you or
/// your body are in it. With a fresh placing (`placed`) on `level` at `at`, that floor's
/// guards hunt there at once in a fight on it, else run there calm (`Call::Answer`), and
/// the `FROM_OTHER_FLOORS` nearest by the fewest flights their card rides come from other
/// floors. With none, every one sweeps the rooms near it.
pub(crate) fn answer(st: &mut AppState, b: BuildingId) {
    if !crate::venues::inside(st, b) {
        return;
    }
    let calm: Vec<usize> = (st.foes.iter().enumerate())
        .filter(|(_, f)| f.standing() && !f.hunting())
        .filter(|(_, f)| f.post.as_ref().is_some_and(|p| p.building == b))
        .filter(|(_, f)| matches!(f.place, Place::Floor { .. }))
        .map(|(i, _)| i)
        .collect();
    let now = st.clock_tick();
    let Some((level, at)) = super::placed(st, b) else {
        let mut any = false;
        for i in calm {
            let here = st.foes[i].pos;
            sweep(st, i, here);
            any |= set_off(st, i, now);
        }
        if any {
            st.warn("guards sweep the floors");
        }
        return;
    };
    let floor = Place::Floor { building: b, level };
    let fight = (st.net.engaged.as_ref()).is_some_and(|e| e.place == floor);
    let plan = crate::interiors::plan_now(st, b);
    let mut here = Vec::new();
    let mut others = Vec::new();
    for i in calm {
        let f = &st.foes[i];
        if f.place == floor {
            here.push(i);
        } else if let (Some(plan), Some(post), Place::Floor { level: l, .. }) =
            (plan.as_deref(), f.post.as_ref(), f.place)
            && let Some(d) = links::hops_to(post, plan, level).get(&l)
        {
            others.push((*d, i));
        }
    }
    drop(plan);
    others.sort_unstable();
    let call = Call::Answer { level, at };
    let answering = (others.into_iter().take(FROM_OTHER_FLOORS)).map(|(_, i)| i);
    let mut any = false;
    for i in here.iter().copied().filter(|_| fight) {
        st.foes[i].mode = FoeMode::Hunting { seen: at, lost: 0 };
        any = true;
    }
    for i in here.into_iter().filter(|_| !fight).chain(answering) {
        if let Some(post) = st.foes[i].post.as_mut() {
            post.call = Some(call.clone());
        }
        any |= set_off(st, i, now);
    }
    if any {
        st.warn("guards close in");
    }
}

/// The calm guards on `place`, a floor with a fight on, that answered a spot on it join
/// the fight: they hunt toward the spot, so the fight seats them as it does its floor's
/// own (`net::seat_foes`).
pub(crate) fn join_fight(st: &mut AppState, place: Place) {
    let Place::Floor { level, .. } = place else {
        return;
    };
    for foe in (st.foes.iter_mut()).filter(|f| f.place == place && f.standing() && !f.hunting()) {
        let Some(post) = foe.post.as_mut() else {
            continue;
        };
        if let Some(Call::Answer { level: there, at }) = post.call
            && there == level
        {
            post.call = None;
            foe.mode = FoeMode::Hunting { seen: at, lost: 0 };
        }
    }
}

/// Guard `i`, given a call, sets off at once rather than finishing its pause: whether it
/// has one.
fn set_off(st: &mut AppState, i: usize, now: u64) -> bool {
    let Some(post) = st.foes[i].post.as_mut().filter(|p| p.call.is_some()) else {
        return false;
    };
    post.round.next = post.round.next.min(now);
    true
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

    /// A calm guard of the ward that can't see you where you stand on L at (6,3): on L in
    /// the dark east hall beyond the card lock, else in the middle of its floor.
    fn calm_guard(st: &mut AppState, level: i32, zone: u8) -> usize {
        let at = if level == 0 {
            Pos::new(20, 3)
        } else {
            Pos::new(24, 3)
        };
        guard(st, level, zone.max(u8::from(level == 0)), &[at])
    }

    #[test]
    fn a_radio_call_in_a_fight_brings_the_floor_in() {
        let mut st = ward();
        let seer = guard(&mut st, 0, 0, &[Pos::new(10, 3)]);
        let far = calm_guard(&mut st, 0, 1);
        assert!(crate::foes::look_all(&mut st));
        assert!(st.foes[seer].hunting() && !st.foes[far].hunting());
        crate::net::engage(&mut st, on(0), true);
        crate::net::lock_down(&mut st, WARD);
        assert_eq!(
            st.foes[far].mode,
            crate::foes::FoeMode::Hunting {
                seen: Pos::new(6, 3),
                lost: 0
            }
        );
        crate::net::seat_foes(&mut st);
        let order = &st.net.engaged.as_ref().unwrap().order;
        assert!(order.contains(&crate::net::Actor::Foe(far)), "{order:?}");
        assert!(logged(&st, "guards close in"));
    }

    #[test]
    fn a_guard_answering_into_a_fight_joins_it() {
        let mut st = ward();
        // The fight is in the dark east hall, out of sight of the stair's landing.
        stand(&mut st, 0, Pos::new(18, 3));
        let seer = guard(&mut st, 0, 1, &[Pos::new(20, 3)]);
        let up = calm_guard(&mut st, 1, 0);
        assert!(crate::foes::look_all(&mut st));
        crate::net::engage(&mut st, on(0), true);
        crate::net::lock_down(&mut st, WARD);
        assert!(matches!(
            call_of(&st, up),
            Some(Call::Answer { level: 0, .. })
        ));
        run_until(&mut st, 2000, |st| st.foes[up].place == on(0));
        crate::net::seat_foes(&mut st);
        assert!(st.foes[up].hunting(), "it came for the fight");
        let order = &st.net.engaged.as_ref().unwrap().order;
        assert!(order.contains(&crate::net::Actor::Foe(up)), "{order:?}");
        assert!(st.foes[seer].hunting());
    }

    #[test]
    fn a_camera_alarm_sends_the_floor_calm() {
        let mut st = ward();
        let far = calm_guard(&mut st, 0, 1);
        crate::guards::place_you(&mut st, WARD, 0, Pos::new(6, 3));
        crate::net::lock_down(&mut st, WARD);
        assert_eq!(
            call_of(&st, far),
            Some(Call::Answer {
                level: 0,
                at: Pos::new(6, 3)
            })
        );
        assert!(!st.foes[far].hunting());
        assert!(st.net.engaged.is_none());
    }

    #[test]
    fn two_guards_answer_from_other_floors_nearest_first() {
        let mut st = ward3();
        let near_a = calm_guard(&mut st, 1, 0);
        let near_b = guard(&mut st, 1, 0, &[Pos::new(20, 3)]);
        let far = calm_guard(&mut st, 2, 0);
        crate::guards::place_you(&mut st, WARD, 0, Pos::new(6, 3));
        crate::net::lock_down(&mut st, WARD);
        let answer = Some(Call::Answer {
            level: 0,
            at: Pos::new(6, 3),
        });
        assert_eq!(call_of(&st, near_a), answer);
        assert_eq!(call_of(&st, near_b), answer);
        assert_eq!(call_of(&st, far), None, "3F is two flights off");
    }

    #[test]
    fn a_net_alarm_with_no_spot_sweeps_every_floor() {
        let mut st = ward();
        let down = guard(&mut st, 0, 2, &[Pos::new(3, 3)]);
        let up = calm_guard(&mut st, 1, 0);
        crate::net::lock_down(&mut st, WARD);
        assert!(matches!(
            call_of(&st, down),
            Some(Call::Sweep { level: 0, .. })
        ));
        assert!(matches!(
            call_of(&st, up),
            Some(Call::Sweep { level: 1, .. })
        ));
        assert!(logged(&st, "guards sweep the floors"));
    }

    #[test]
    fn a_repeat_alarm_answers_nothing() {
        let mut st = ward();
        let i = calm_guard(&mut st, 1, 0);
        crate::net::lock_down(&mut st, WARD);
        assert!(call_of(&st, i).is_some());
        st.foes[i].post.as_mut().unwrap().call = None;
        crate::net::lock_down(&mut st, WARD);
        assert_eq!(call_of(&st, i), None);
    }

    #[test]
    fn an_alarm_while_youre_out_leaves_no_calls() {
        let mut st = ward();
        let i = calm_guard(&mut st, 1, 0);
        let exit = *st
            .interiors
            .floor(WARD, 0)
            .unwrap()
            .1
            .exit_at(Pos::new(0, 3))
            .unwrap();
        crate::interiors::leave(&mut st, &exit).unwrap();
        crate::net::lock_down(&mut st, WARD);
        assert_eq!(call_of(&st, i), None);
    }

    #[test]
    fn heat_over_the_alarm_line_answers() {
        let mut st = ward();
        let far = calm_guard(&mut st, 0, 1);
        crate::guards::place_you(&mut st, WARD, 0, Pos::new(6, 3));
        crate::net::ice_heat(&mut st, WARD, game::ALARM_HEAT);
        assert!(st.net.alert(WARD, st.net.now(&st)));
        assert!(matches!(call_of(&st, far), Some(Call::Answer { .. })));
    }
}
