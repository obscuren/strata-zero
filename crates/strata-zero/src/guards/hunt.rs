//! The hunt off a fight: a guard follows you through the floors its card rides, walks
//! to where it last had you, and loses you at the street door.

use super::round::{self, Step};
use super::{GUARD_LIFT_TICKS, GUARD_RUN_TICKS, GUARD_STEP_TICKS, HEAR_RADIUS, links};
use crate::app::AppState;
use crate::foes::FoeMode;
use citygen::{BuildingId, LinkKind, Place};

/// You took link `link` off level `level` of building `b` to level `to`: every standing
/// guard of that floor that hunts you follows (`post.follow`), setting off at once.
pub(crate) fn you_left(st: &mut AppState, b: BuildingId, level: i32, link: usize, to: i32) {
    let now = st.clock_tick();
    let floor = Place::Floor { building: b, level };
    for i in crate::foes::hunting_on(st, floor) {
        if let Some(post) = st.foes[i].post.as_mut() {
            post.follow = Some((link, to));
            post.round.next = post.round.next.min(now);
        }
    }
}

/// A shot or a blast at `at` on `place`: on a floor, every standing calm guard there
/// within `HEAR_RADIUS` of it (walls or not) hunts toward `toward`. It heard you; it
/// hasn't seen you, so it has nothing to radio yet.
pub fn hear(st: &mut AppState, place: Place, at: citygen::Pos, toward: citygen::Pos) {
    if !matches!(place, Place::Floor { .. }) {
        return;
    }
    for foe in st.foes.iter_mut() {
        let calm = foe.mode == FoeMode::Idle;
        let near = foe.place == place && foe.pos.chebyshev(at) <= HEAR_RADIUS;
        if foe.post.is_some() && foe.standing() && calm && near {
            foe.mode = FoeMode::Hunting {
                seen: toward,
                lost: 0,
            };
        }
    }
}

/// One action of hunting guard `i` off a fight, at clock tick `now`; it acts again
/// `GUARD_RUN_TICKS` on. Following you, it runs to the stop of the link you took and
/// takes it when its card rides it there (a lift once it has waited for it), hunting
/// from where it lands; when its card doesn't, it loses you there. Else it runs to where
/// it last had you, and there, or with no way there, it loses you.
pub(crate) fn hunt_step(st: &mut AppState, i: usize, now: u64) {
    links::forget_stale(st, i);
    let foe = &st.foes[i];
    let FoeMode::Hunting { seen, .. } = foe.mode else {
        return;
    };
    let (Some(post), Place::Floor { building, level }) = (foe.post.as_ref(), foe.place) else {
        return lose_you(st, i, now);
    };
    let from = foe.pos;
    let Some((link, to)) = post.follow else {
        if from == seen || held_by_another(st, i, seen) {
            return lose_you(st, i, now);
        }
        return run_to(st, i, building, level, seen, now);
    };
    let Some(plan) = crate::interiors::plan_now(st, building) else {
        return lose_you(st, i, now);
    };
    let Some(at) = (plan.links.get(link)).and_then(|l| Some(l.stop(level)?.at)) else {
        return lose_you(st, i, now);
    };
    if from != at {
        drop(plan);
        return run_to(st, i, building, level, at, now);
    }
    let lift = plan.links[link].kind == LinkKind::Lift;
    let rides = links::hops(st, post, &plan, link, level).contains(&to);
    let waited = post.round.lift == Some((link, to));
    drop(plan);
    if !rides {
        return lose_you(st, i, now);
    }
    if lift && !waited {
        if let Some(post) = st.foes[i].post.as_mut() {
            post.round.lift = Some((link, to));
            post.round.next = now + GUARD_LIFT_TICKS;
        }
        return;
    }
    if links::arrive(st, i, link, to) {
        let foe = &mut st.foes[i];
        foe.mode = FoeMode::Hunting {
            seen: foe.pos,
            lost: 0,
        };
        if let Some(post) = foe.post.as_mut() {
            post.follow = None;
        }
    }
    round::wait(st, i, now + GUARD_RUN_TICKS);
}

/// Guard `i` runs a tile toward `target` on its floor; with no way there it loses you.
fn run_to(
    st: &mut AppState,
    i: usize,
    building: BuildingId,
    level: i32,
    target: citygen::Pos,
    now: u64,
) {
    match round::step(st, i, building, level, target) {
        Step::NoWay => lose_you(st, i, now),
        Step::NoMap | Step::Held | Step::To(_) => round::wait(st, i, now + GUARD_RUN_TICKS),
    }
}

/// Whether guard `i` stands beside `at` and someone else holds it (you, your body, another
/// foe): it gets no nearer where it last had you, and gives up as if there.
fn held_by_another(st: &AppState, i: usize, at: citygen::Pos) -> bool {
    st.foes[i].pos.chebyshev(at) <= 1 && round::avoid(st, i).contains(&at)
}

/// Guard `i` loses you: calm, its trail and the lift it waited on to follow you let go,
/// it walks back into its round a step on from `now`.
pub(super) fn lose_you(st: &mut AppState, i: usize, now: u64) {
    let foe = &mut st.foes[i];
    foe.mode = FoeMode::Idle;
    if let Some(post) = foe.post.as_mut() {
        if post.follow.is_some() && post.round.lift == post.follow {
            post.round.lift = None;
        }
        post.follow = None;
        post.round.next = now + GUARD_STEP_TICKS;
    }
}

#[cfg(test)]
mod tests {
    use crate::guards::GUARD_RUN_TICKS;
    use crate::guards::testutil::*;
    use citygen::Pos;

    #[test]
    fn a_hunting_guard_follows_you_up_the_stairs() {
        let mut st = ward();
        stand(&mut st, 0, Pos::new(2, 1));
        let i = guard(&mut st, 0, 0, &[Pos::new(6, 3)]);
        crate::foes::look_all(&mut st);
        crate::net::engage(&mut st, on(0), true);
        crate::interiors::climb(&mut st, true).unwrap();
        assert_eq!(st.foes[i].post.as_ref().unwrap().follow, Some((0, 1)));
        crate::net::disengage(&mut st);
        stand(&mut st, 1, Pos::new(28, 3)); // walked off out of its way
        run_until(&mut st, 400, |st| st.foes[i].place == on(1));
        assert!(
            matches!(st.foes[i].mode, crate::foes::FoeMode::Hunting { seen, .. } if seen == st.foes[i].pos)
        );
        assert_eq!(st.foes[i].post.as_ref().unwrap().follow, None);
    }

    #[test]
    fn a_guard_whose_card_wont_ride_the_lift_loses_you_there() {
        let mut st = ward();
        stand(&mut st, 1, Pos::new(28, 3)); // you rode the lift up
        let i = guard(&mut st, 0, 0, &[Pos::new(10, 3)]);
        st.foes[i].mode = crate::foes::FoeMode::Hunting {
            seen: Pos::new(12, 1),
            lost: 0,
        };
        st.foes[i].post.as_mut().unwrap().follow = Some((1, 1)); // as `you_left` marks it
        run_until(&mut st, 400, |st| !st.foes[i].hunting());
        assert_eq!(st.foes[i].place, on(0));
        assert_eq!(st.foes[i].post.as_ref().unwrap().follow, None);
    }

    #[test]
    fn out_the_door_the_guards_lose_you() {
        let mut st = ward();
        let i = guard(&mut st, 0, 0, &[Pos::new(10, 3)]);
        crate::foes::look_all(&mut st);
        let exit = *st
            .interiors
            .floor(WARD, 0)
            .unwrap()
            .1
            .exit_at(Pos::new(0, 3))
            .unwrap();
        crate::interiors::leave(&mut st, &exit).unwrap();
        crate::guards::tick(&mut st);
        assert!(!st.foes[i].hunting());
        assert_eq!(st.foes[i].place, on(0), "it never leaves the building");
    }

    #[test]
    fn a_woken_guard_off_your_floor_walks_to_the_shot_and_gives_up() {
        let mut st = ward();
        stand(&mut st, 1, Pos::new(28, 3));
        let i = guard(&mut st, 0, 0, &[Pos::new(2, 3)]);
        st.foes[i].mode = crate::foes::FoeMode::Hunting {
            seen: Pos::new(6, 3),
            lost: 0,
        };
        run(&mut st, 4 * GUARD_RUN_TICKS);
        assert_eq!(st.foes[i].pos, Pos::new(6, 3));
        run(&mut st, GUARD_RUN_TICKS);
        assert!(!st.foes[i].hunting(), "nobody there: back to its round");
    }

    #[test]
    fn a_guard_whose_card_rides_the_lift_follows_you_after_its_wait() {
        let mut st = ward();
        stand(&mut st, 1, Pos::new(28, 3));
        let i = guard(&mut st, 0, 1, &[Pos::new(12, 1)]);
        st.foes[i].mode = crate::foes::FoeMode::Hunting {
            seen: Pos::new(12, 1),
            lost: 0,
        };
        st.foes[i].post.as_mut().unwrap().follow = Some((1, 1));
        run(&mut st, 1);
        assert_eq!(st.foes[i].post.as_ref().unwrap().round.lift, Some((1, 1)));
        run(&mut st, crate::guards::GUARD_LIFT_TICKS - 1);
        assert_eq!(st.foes[i].place, on(0), "still waiting");
        run(&mut st, 1);
        assert_eq!((st.foes[i].place, st.foes[i].pos), (on(1), Pos::new(12, 1)));
        assert!(st.foes[i].hunting());
        assert_eq!(st.foes[i].post.as_ref().unwrap().follow, None);
    }

    #[test]
    fn a_trail_on_a_link_the_plan_lacks_is_let_go_on_its_next_step() {
        let mut st = ward();
        stand(&mut st, 1, Pos::new(28, 3));
        let i = guard(&mut st, 0, 0, &[Pos::new(2, 3)]);
        st.foes[i].mode = crate::foes::FoeMode::Hunting {
            seen: Pos::new(6, 3),
            lost: 0,
        };
        st.foes[i].post.as_mut().unwrap().follow = Some((99, 1)); // a save from another build
        run(&mut st, 1);
        assert_eq!(st.foes[i].post.as_ref().unwrap().follow, None);
        assert!(st.foes[i].hunting(), "on to where it had you");
        assert_eq!(st.foes[i].pos, Pos::new(3, 3));
    }

    #[test]
    fn a_guard_beside_where_it_had_you_with_someone_there_gives_up() {
        let mut st = ward();
        stand(&mut st, 1, Pos::new(28, 3));
        let i = guard(&mut st, 0, 0, &[Pos::new(5, 3)]);
        st.foes[i].mode = crate::foes::FoeMode::Hunting {
            seen: Pos::new(6, 3),
            lost: 0,
        };
        guard(&mut st, 0, 0, &[Pos::new(6, 3)]); // standing its post there
        run(&mut st, 1);
        assert!(!st.foes[i].hunting());
        assert_eq!(st.foes[i].pos, Pos::new(5, 3));
    }

    #[test]
    fn a_shot_on_a_floor_wakes_the_calm_guards_within_twelve() {
        let mut st = ward();
        st.character.skills.insert(game::SkillId::new("handguns"));
        let target = goon(&mut st, 0, Pos::new(10, 3));
        let near = guard(&mut st, 0, 1, &[Pos::new(18, 3)]); // 12, behind the wall
        let far = guard(&mut st, 0, 1, &[Pos::new(19, 3)]); // 13
        let above = guard(&mut st, 1, 0, &[Pos::new(6, 3)]);
        crate::net::engage(&mut st, on(0), true);
        crate::net::stay(&mut st);
        crate::combat::fire(&mut st, target).unwrap();
        assert_eq!(
            st.foes[near].mode,
            crate::foes::FoeMode::Hunting {
                seen: Pos::new(6, 3),
                lost: 0
            }
        );
        assert!(
            !st.foes[near].post.as_ref().unwrap().spotted,
            "heard, not seen"
        );
        assert!(!st.foes[far].hunting() && !st.foes[above].hunting());
    }

    #[test]
    fn a_guards_shot_wakes_the_guards_near_it() {
        let mut st = ward();
        let shooter = guard(&mut st, 0, 0, &[Pos::new(10, 3)]);
        let near = guard(&mut st, 0, 1, &[Pos::new(22, 3)]); // 12 from the shooter
        crate::combat::fire_at_you(&mut st, shooter);
        assert_eq!(
            st.foes[near].mode,
            crate::foes::FoeMode::Hunting {
                seen: Pos::new(10, 3),
                lost: 0
            }
        );
    }

    #[test]
    fn a_blast_on_a_floor_is_heard_within_twelve() {
        let mut st = ward();
        let near = guard(&mut st, 0, 2, &[Pos::new(25, 3)]); // 12 from (13,3)
        let far = guard(&mut st, 0, 2, &[Pos::new(26, 3)]);
        let satchel = st
            .content
            .item(&game::ItemId::new("satchel"))
            .unwrap()
            .clone();
        let (_, _, blast) = satchel.throw().unwrap();
        st.live.push(crate::blast::Live {
            place: on(0),
            pos: Pos::new(13, 3),
            blast,
            fx: Vec::new(),
            name: satchel.name.clone(),
            turns: 2,
            lands: 0,
            wakes: Some(Pos::new(6, 3)),
        });
        crate::blast::land(&mut st);
        assert_eq!(
            st.foes[near].mode,
            crate::foes::FoeMode::Hunting {
                seen: Pos::new(6, 3),
                lost: 0
            }
        );
        assert!(!st.foes[far].hunting());
    }

    #[test]
    fn a_blast_going_off_on_a_floor_turns_the_guards_in_earshot_toward_you() {
        let mut st = ward();
        let near = guard(&mut st, 0, 2, &[Pos::new(25, 3)]); // 12 from (13,3)
        let far = guard(&mut st, 0, 2, &[Pos::new(26, 3)]);
        let satchel = st
            .content
            .item(&game::ItemId::new("satchel"))
            .unwrap()
            .clone();
        let (_, _, blast) = satchel.throw().unwrap();
        let now = st.tick;
        crate::blast::detonate(
            &mut st,
            on(0),
            Pos::new(13, 3),
            blast,
            &[],
            &satchel.name,
            now,
            None,
        );
        assert_eq!(
            st.foes[near].mode,
            crate::foes::FoeMode::Hunting {
                seen: Pos::new(6, 3),
                lost: 0
            }
        );
        assert!(!st.foes[far].hunting());
    }
}
