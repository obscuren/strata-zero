//! The hunt off a fight: a guard follows you through the floors its card rides, walks
//! to where it last had you, and loses you at the street door.

use super::round::{self, Step};
use super::{GUARD_LIFT_TICKS, GUARD_RUN_TICKS, GUARD_STEP_TICKS, HEAR_RADIUS, links};
use crate::app::AppState;
use crate::foes::{FoeMode, Post};
use citygen::{BuildingId, Place};

/// You took link `link` off level `level` of building `b` to level `to`: every standing
/// guard of that floor that hunts you follows (`post.follow`), setting off at once.
/// Cloaked, nobody saw you go: they walk to where they last had you instead.
pub(crate) fn you_left(st: &mut AppState, b: BuildingId, level: i32, link: usize, to: i32) {
    if crate::cloak::hidden(st) {
        return;
    }
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
/// `GUARD_RUN_TICKS` on, or `GUARD_LIFT_TICKS` on when it starts waiting for a lift.
/// Following you, it runs to the stop of the link you took and takes it when its card
/// rides it there (a lift once it has waited for it), hunting from where it lands; when
/// its card doesn't, it loses you there. Else it runs to where it last had you, and
/// there, or with no way there, it loses you. Held up `net::LOST_TURNS` steps running,
/// either way, it loses you too.
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
    let lift = links::is_lift(&plan, link);
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
/// Its steps held up in a row count in its hunt's `lost`, and at `net::LOST_TURNS` it
/// loses you: a way blocked by someone who won't move is no way.
fn run_to(
    st: &mut AppState,
    i: usize,
    building: BuildingId,
    level: i32,
    target: citygen::Pos,
    now: u64,
) {
    let step = round::step(st, i, building, level, target);
    let FoeMode::Hunting { seen, lost } = st.foes[i].mode else {
        return;
    };
    let lost = match step {
        Step::NoWay => return lose_you(st, i, now),
        Step::Held => lost + 1,
        Step::To(_) => 0,
        Step::NoMap => lost,
    };
    if lost >= crate::net::LOST_TURNS {
        return lose_you(st, i, now);
    }
    st.foes[i].mode = FoeMode::Hunting { seen, lost };
    round::wait(st, i, now + GUARD_RUN_TICKS);
}

/// Whether guard `i` stands beside `at` and someone else holds it (you, your body, another
/// foe): it gets no nearer where it last had you, and gives up as if there.
fn held_by_another(st: &AppState, i: usize, at: citygen::Pos) -> bool {
    st.foes[i].pos.chebyshev(at) <= 1 && round::avoid(st, i).contains(&at)
}

/// While you're jacked in with your body on a floor, the first standing guard of that
/// floor's building there that sees the body finds it: it radios it in (the alarm, unless
/// the site is already on alert), you're dumped back into it, and it hunts from the body.
pub(crate) fn body_check(st: &mut AppState) {
    let Some((place, at)) = st.player.as_ref().and_then(|p| p.body) else {
        return;
    };
    let Place::Floor { building: b, .. } = place else {
        return;
    };
    let Some(map) = crate::foes::map_of(st, place) else {
        return;
    };
    let ours = |f: &crate::foes::Foe| f.post.as_ref().is_some_and(|p| p.building == b);
    let found = (st.foes.iter())
        .position(|f| f.place == place && ours(f) && crate::foes::sees(&map, f, at));
    let Some(i) = found else {
        return;
    };
    if let Some(post) = st.foes[i].post.as_mut() {
        post.radioed = true;
        release_follow(post);
    }
    if !st.net.alert(b, st.net.now(st)) {
        crate::net::lock_down(st, b);
    }
    crate::net::dump(st, "a guard found you · jacked out");
    st.foes[i].mode = FoeMode::Hunting { seen: at, lost: 0 };
}

/// Lets a trail go: the follow, and the lift it waited on to follow it.
pub(crate) fn release_follow(post: &mut Post) {
    if post.follow.is_some() && post.round.lift == post.follow {
        post.round.lift = None;
    }
    post.follow = None;
}

/// A downed guard of a zone above public leaves its zone's card where it fell, and the
/// copy lists it among the cards lying about, so the job's end takes it too.
pub(crate) fn drop_card(st: &mut AppState, i: usize) {
    let foe = &st.foes[i];
    let Some(post) = foe.post.as_ref().filter(|p| p.zone > 0) else {
        return;
    };
    let (place, at, b) = (foe.place, foe.pos, post.building);
    let id = crate::credentials::zone_card_id(b, post.zone);
    st.ground.add(place, at, id.clone(), 1);
    if let Some(left) = st.interiors.left_mut(b) {
        left.push((place, at, id));
    }
}

/// Guard `i` loses you: calm, its trail and the lift it waited on to follow you let go,
/// it walks back into its round a step on from `now`.
pub(super) fn lose_you(st: &mut AppState, i: usize, now: u64) {
    let foe = &mut st.foes[i];
    foe.mode = FoeMode::Idle;
    if let Some(post) = foe.post.as_mut() {
        release_follow(post);
        post.round.next = now + GUARD_STEP_TICKS;
    }
}

#[cfg(test)]
mod tests {
    use crate::app::AppState;
    use crate::guards::GUARD_RUN_TICKS;
    use crate::guards::testutil::*;
    use crate::render::map::TileMap;
    use citygen::{BuildingId, Place, Pos};

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
    fn a_cloaked_runner_isnt_seen_leaving_a_floor() {
        let mut st = ward();
        stand(&mut st, 0, Pos::new(2, 1));
        let i = guard(&mut st, 0, 0, &[Pos::new(6, 3)]);
        assert!(crate::foes::look_all(&mut st), "it has you");
        crate::cloak::testutil::stealthy(&mut st);
        st.character.focus = 10;
        crate::cloak::testutil::fire(&mut st, "cloak").unwrap();
        crate::interiors::climb(&mut st, true).unwrap();
        assert!(st.foes[i].hunting());
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
        st.foes[i].post.as_mut().unwrap().follow = Some((0, 1));
        let exit = *st
            .interiors
            .floor(WARD, 0)
            .unwrap()
            .1
            .exit_at(Pos::new(0, 3))
            .unwrap();
        crate::interiors::leave(&mut st, &exit).unwrap();
        crate::guards::tick(&mut st, 1);
        assert!(!st.foes[i].hunting());
        assert_eq!(st.foes[i].place, on(0), "it never leaves the building");
        assert_eq!(st.foes[i].post.as_ref().unwrap().follow, None);
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
    fn a_strike_on_a_floor_is_not_heard_but_a_shot_is() {
        let mut st = ward();
        st.character.skills.insert(game::SkillId::new("handguns"));
        st.character.give(game::ItemId::new("neural_port"), 1);
        st.character
            .chrome
            .insert(game::Site::Neural, game::ItemId::new("neural_port"));
        let target = goon(&mut st, 0, Pos::new(10, 3));
        let near = guard(&mut st, 0, 1, &[Pos::new(18, 3)]); // 12, behind the wall
        crate::net::engage(&mut st, on(0), true);
        crate::net::stay(&mut st);
        crate::combat::strike(&mut st, &game::AbilityId::new("static"), target).unwrap();
        assert!(!st.foes[near].hunting(), "a strike is no gunshot");
        st.net.engaged.as_mut().unwrap().ap = crate::net::BASE_AP;
        crate::combat::fire(&mut st, target).unwrap();
        assert!(st.foes[near].hunting(), "a gunshot carries");
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

    #[test]
    fn a_hunter_held_up_in_a_one_wide_way_gives_up() {
        let mut st = ward();
        stand(&mut st, 1, Pos::new(28, 3));
        let i = guard(&mut st, 0, 1, &[Pos::new(18, 3)]);
        st.foes[i].mode = crate::foes::FoeMode::Hunting {
            seen: Pos::new(6, 3),
            lost: 0,
        };
        guard(&mut st, 0, 1, &[Pos::new(15, 3)]); // standing its post in the card door
        run_until(&mut st, 400, |st| !st.foes[i].hunting());
        assert_eq!(st.foes[i].pos, Pos::new(16, 3), "it never got by");
    }

    #[test]
    fn a_follower_whose_stair_is_held_gives_up() {
        let mut st = ward();
        stand(&mut st, 1, Pos::new(28, 3)); // you took the stair up
        let i = guard(&mut st, 0, 0, &[Pos::new(6, 3)]);
        st.foes[i].mode = crate::foes::FoeMode::Hunting {
            seen: Pos::new(2, 1),
            lost: 0,
        };
        st.foes[i].post.as_mut().unwrap().follow = Some((0, 1));
        guard(&mut st, 0, 0, &[Pos::new(2, 1)]); // standing its post on the stair
        run_until(&mut st, 400, |st| !st.foes[i].hunting());
        assert_eq!(st.foes[i].place, on(0));
        assert_eq!(st.foes[i].post.as_ref().unwrap().follow, None);
    }

    #[test]
    fn a_lift_that_stops_answering_while_a_guard_waits_to_follow_turns_it_calm() {
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
        crate::net::lock_down(&mut st, WARD);
        run_until(&mut st, 2 * crate::guards::GUARD_LIFT_TICKS, |st| {
            !st.foes[i].hunting()
        });
        let post = st.foes[i].post.as_ref().unwrap();
        assert_eq!((post.round.lift, post.follow), (None, None));
        assert_eq!(st.foes[i].place, on(0));
    }

    #[test]
    fn a_guard_waiting_on_the_lift_to_follow_that_sees_you_lets_the_lift_go() {
        let mut st = ward();
        let i = guard(&mut st, 0, 1, &[Pos::new(12, 1)]);
        st.foes[i].mode = crate::foes::FoeMode::Hunting {
            seen: Pos::new(12, 1),
            lost: 0,
        };
        let post = st.foes[i].post.as_mut().unwrap();
        post.follow = Some((1, 1));
        post.round.lift = Some((1, 1));
        assert!(crate::foes::look(&mut st, i), "you stand in the lit hall");
        let post = st.foes[i].post.as_ref().unwrap();
        assert_eq!((post.round.lift, post.follow), (None, None));
    }

    /// Site `b`'s first guard moved onto a tile of the body's floor two tiles from it with a
    /// clear line, lit or not (2 is within the dark reach), its round held so it stands
    /// there; its index.
    fn guard_by_the_body(st: &mut AppState, b: BuildingId) -> usize {
        nth_guard_by_the_body(st, b, 0)
    }

    /// The `n`th guard of building `b` (index order), set two tiles from the body in its
    /// sight, on a tile no foe holds.
    fn nth_guard_by_the_body(st: &mut AppState, b: BuildingId, n: usize) -> usize {
        let (place, at) = st.player.as_ref().unwrap().body.unwrap();
        let map = crate::foes::map_of(st, place).unwrap();
        let spot = (at.x - 2..=at.x + 2)
            .flat_map(|x| (at.y - 2..=at.y + 2).map(move |y| Pos::new(x, y)))
            .filter(|p| {
                p.chebyshev(at) == 2 && map.walkable(*p) && crate::foes::line_clear(&map, *p, at)
            })
            .find(|p| crate::foes::foe_at(st, place, *p).is_none())
            .expect("a tile beside the body");
        let i = st
            .foes
            .iter()
            .enumerate()
            .filter(|(_, f)| f.post.as_ref().is_some_and(|p| p.building == b))
            .nth(n)
            .map(|(i, _)| i)
            .unwrap();
        st.foes[i].place = place;
        st.foes[i].pos = spot;
        st.foes[i].post.as_mut().unwrap().round.next = u64::MAX;
        i
    }

    #[test]
    fn a_guard_that_finds_your_body_dumps_you_and_raises_the_alarm() {
        use crate::sitenet::testutil::{jacked_in_at_the_kiosk, office_copy};
        let (mut st, b) = office_copy(190);
        jacked_in_at_the_kiosk(&mut st, b);
        let (_, at) = st.player.as_ref().unwrap().body.unwrap();
        let i = guard_by_the_body(&mut st, b);
        let second = nth_guard_by_the_body(&mut st, b, 1);
        assert!(second > i);
        crate::guards::tick(&mut st, 1);
        assert!(!st.player.as_ref().unwrap().jacked_in());
        assert!(
            !st.foes[second].post.as_ref().unwrap().radioed,
            "only the first finds it"
        );
        assert_eq!(st.foes[second].mode, crate::foes::FoeMode::Idle);
        assert!(
            st.log
                .iter()
                .any(|e| e.text == "DUMPED · a guard found you · jacked out")
        );
        assert!(st.net.alert(b, st.net.now(&st)));
        assert_eq!(st.character.focus, 0);
        assert_eq!(
            st.foes[i].mode,
            crate::foes::FoeMode::Hunting { seen: at, lost: 0 }
        );
        assert!(st.foes[i].post.as_ref().unwrap().radioed);
    }

    #[test]
    fn the_body_panel_follows_a_guard_as_it_walks() {
        use crate::foes::{Foe, Post};
        use crate::sitenet::testutil::{jacked_in_at_the_kiosk, office_copy};
        let (mut st, b) = office_copy(190);
        jacked_in_at_the_kiosk(&mut st, b);
        let (place, at) = st.player.as_ref().unwrap().body.unwrap();
        let Place::Floor { level, .. } = place else {
            panic!("a body on a floor")
        };
        // One guard, walking between the tile of the body's floor farthest from it and a
        // tile beside that: far past its sight.
        let (far, next) = {
            let (_, floor) = st.interiors.floor(b, level).unwrap();
            let open = |p: Pos| {
                floor
                    .tiles
                    .get(p)
                    .is_some_and(|t| t.terrain == citygen::Terrain::Floor)
            };
            let far = (floor.tiles.iter())
                .map(|(p, _)| p)
                .filter(|p| open(*p))
                .max_by_key(|p| (p.chebyshev(at), p.y, p.x))
                .unwrap();
            let next = far
                .neighbors4()
                .filter(|p| open(*p))
                .min_by_key(|p| p.chebyshev(at))
                .unwrap();
            (far, next)
        };
        no_guards(&mut st, b);
        st.foes.push(Foe {
            name: "guard".to_string(),
            glyph: '@',
            post: Some(Post {
                home: Some(level),
                ..Post::new(b, 1, vec![far, next])
            }),
            ..Foe::goon(place, far)
        });
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..400 {
            advance(&mut st, 1);
            crate::guards::tick(&mut st, 1);
            let d = st.foes[st.foes.len() - 1].pos.chebyshev(at);
            let lines = crate::sitenet::body_lines(&st);
            assert!(lines[1].contains(&format!("nearest {d} ")), "{lines:?}");
            seen.insert(d);
        }
        assert!(
            st.player.as_ref().unwrap().jacked_in(),
            "never near enough to find you"
        );
        assert!(seen.len() > 1, "the line followed it");
    }

    /// Site `b`'s first guard of a zone above public, and its zone.
    fn a_carded_guard(st: &AppState, b: BuildingId) -> (usize, u8) {
        (st.foes.iter().enumerate())
            .find_map(|(i, f)| {
                f.post
                    .as_ref()
                    .filter(|p| p.building == b && p.zone > 0)
                    .map(|p| (i, p.zone))
            })
            .expect("a carded guard")
    }

    #[test]
    fn a_downed_guard_leaves_its_zone_card() {
        let (mut st, b) = crate::sitenet::testutil::office_copy(190);
        let (i, zone) = a_carded_guard(&st, b);
        let (place, at) = (st.foes[i].place, st.foes[i].pos);
        crate::combat::wound_foe(&mut st, i, 255);
        let id = crate::credentials::zone_card_id(b, zone);
        assert!(st.ground.at(place, at).iter().any(|s| s.item == id));
        assert!(
            st.interiors
                .copy(b)
                .unwrap()
                .left
                .contains(&(place, at, id))
        );
    }

    #[test]
    fn a_public_guard_drops_nothing() {
        let (mut st, b) = crate::sitenet::testutil::office_copy(190);
        let (i, _) = a_carded_guard(&st, b);
        st.foes[i].post.as_mut().unwrap().zone = 0;
        let (place, at) = (st.foes[i].place, st.foes[i].pos);
        let left = st.interiors.copy(b).unwrap().left.len();
        crate::combat::wound_foe(&mut st, i, 255);
        assert!(st.ground.at(place, at).is_empty());
        assert_eq!(st.interiors.copy(b).unwrap().left.len(), left);
    }

    #[test]
    fn the_jobs_end_takes_a_dropped_card_and_a_picked_up_guard_card_stays_in_your_pack() {
        let (mut st, b) = crate::sitenet::testutil::office_copy(190);
        let (i, zone) = a_carded_guard(&st, b);
        let (place, at) = (st.foes[i].place, st.foes[i].pos);
        crate::combat::wound_foe(&mut st, i, 255);
        let id = crate::credentials::zone_card_id(b, zone);
        let mut other = (st.foes.iter().enumerate()).filter(|(j, f)| {
            *j != i
                && f.post
                    .as_ref()
                    .is_some_and(|p| p.building == b && p.zone > 0)
        });
        let (j, _) = other.next().expect("a second carded guard");
        let (jp, ja, jz) = (
            st.foes[j].place,
            st.foes[j].pos,
            st.foes[j].post.as_ref().unwrap().zone,
        );
        crate::combat::wound_foe(&mut st, j, 255);
        let jid = crate::credentials::zone_card_id(b, jz);
        assert_eq!(st.ground.take(jp, ja, &jid, 1), 1);
        st.character.give(jid.clone(), 1); // picked up
        crate::venues::end(&mut st, b).unwrap(); // you're outside: it ends now
        assert!(
            st.ground.at(place, at).iter().all(|s| s.item != id),
            "the one left lying goes"
        );
        assert_eq!(st.character.count(&jid), 1, "the one you took stays");
        assert!(
            !st.foes
                .iter()
                .any(|f| f.post.as_ref().is_some_and(|p| p.building == b))
        );
    }
}
