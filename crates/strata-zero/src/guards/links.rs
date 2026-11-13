//! Taking the stairs and the lifts: which links a guard's card rides, the one it walks
//! to, taking it, and landing beside whoever holds the other end.

use super::{GUARD_LIFT_TICKS, GUARD_STEP_TICKS};
use crate::app::AppState;
use crate::foes::{self, Post};
use crate::render::map::{MapRef, TileMap};
use citygen::pathing::astar;
use citygen::{FloorPlan, LinkKind, Place, Pos};

/// The levels a guard of `post` reaches by link `link` of `plan` from level `from` in one
/// go: none by a crawl; a lift's rides only while it answers and to stops its card opens;
/// a stair's, an escalator's or a ladder's next stop up and down, which ask no card.
pub fn hops(st: &AppState, post: &Post, plan: &FloorPlan, link: usize, from: i32) -> Vec<i32> {
    let hops = plan.hops(link, from);
    if plan.links.get(link).map(|l| l.kind) != Some(LinkKind::Lift) {
        return hops;
    }
    if !crate::sitenet::lift_answers(st, post.building, link) {
        return Vec::new();
    }
    (hops.into_iter())
        .filter(|to| crate::venues::ride_card(plan, link, *to) <= post.zone)
        .collect()
}

/// The link guard `i` takes to level `to`, and its stop tile on the guard's level: the
/// one it came by (`round.via`) while that still goes there, so it comes back the way it
/// went; else of the links one hop there, the nearest by its walk (ties: link order).
pub(crate) fn link_to(st: &AppState, i: usize, to: i32) -> Option<(usize, Pos)> {
    let foe = st.foes.get(i)?;
    let post = foe.post.as_ref()?;
    let Place::Floor { building, level } = foe.place else {
        return None;
    };
    let plan = crate::interiors::plan_now(st, building)?;
    let stop = |k: usize| Some((k, plan.links.get(k)?.stop(level)?.at));
    let goes = |k: &usize| hops(st, post, &plan, *k, level).contains(&to);
    if let Some(way) = post.round.via.filter(goes).and_then(stop) {
        return Some(way);
    }
    let map = foes::map_of(st, foe.place)?;
    (0..plan.links.len())
        .filter(goes)
        .filter_map(stop)
        .filter_map(|(k, at)| Some((walk(&map, post, foe.pos, at)?, k, at)))
        .min_by_key(|(steps, k, _)| (*steps, *k))
        .map(|(_, k, at)| (k, at))
}

/// The steps of a guard's walk from `from` to `to` on `map`, its card's doors open:
/// None when there's no way.
fn walk(map: &MapRef, post: &Post, from: Pos, to: Pos) -> Option<usize> {
    if from == to {
        return Some(0);
    }
    let path = astar(map.size(), from, to, |p, n| {
        foes::can_step(map, Some(post), p, n).then_some(1)
    })?;
    Some(path.len())
}

/// Guard `i`, on link `link`'s stop tile, takes it to level `to` at clock tick `now`: a
/// stair, an escalator or a ladder at once (waiting a step when the landing's held); a
/// lift once it has waited for it. Taking it away from its round's level marks the way
/// back (`round.via`); coming home clears it.
pub(crate) fn take(st: &mut AppState, i: usize, link: usize, to: i32, now: u64) {
    let Place::Floor { building, level } = st.foes[i].place else {
        return;
    };
    let lift = (crate::interiors::plan_now(st, building))
        .is_some_and(|plan| plan.links.get(link).map(|l| l.kind) == Some(LinkKind::Lift));
    let Some(post) = st.foes[i].post.as_mut() else {
        return;
    };
    let home = *post.home.get_or_insert(level);
    post.round.via = (to != home).then_some(link);
    if lift {
        post.round.lift = Some((link, to));
        post.round.next = now + GUARD_LIFT_TICKS;
        return;
    }
    arrive(st, i, link, to);
    if let Some(post) = st.foes[i].post.as_mut() {
        post.round.next = now + GUARD_STEP_TICKS;
    }
}

/// Guard `i` waits on lift `link` for level `to`, due at clock tick `now`: it rides once
/// the lift answers it and the landing has room, else waits another step. A lift that no
/// longer goes there from the guard's level (a save from another build) is let go.
pub(crate) fn wait_for_lift(st: &mut AppState, i: usize, link: usize, to: i32, now: u64) {
    let rides = match st.foes[i].place {
        Place::Floor { building, level } => crate::interiors::plan_now(st, building).map(|plan| {
            let post = st.foes[i].post.as_ref();
            (
                plan.hops(link, level).contains(&to),
                post.is_some_and(|p| hops(st, p, &plan, link, level).contains(&to)),
            )
        }),
        _ => None,
    };
    let Some(post) = st.foes[i].post.as_mut() else {
        return;
    };
    post.round.next = now + GUARD_STEP_TICKS;
    match rides {
        Some((true, true)) => {
            arrive(st, i, link, to);
        }
        Some((true, false)) => {}
        _ => post.round.lift = None,
    }
}

/// Guard `i` comes off link `link` on level `to`: on the link's stop there, or beside it
/// when that's held, its lift wait over. False when every tile there is held: it waits.
pub(crate) fn arrive(st: &mut AppState, i: usize, link: usize, to: i32) -> bool {
    let Place::Floor { building, .. } = st.foes[i].place else {
        return false;
    };
    if let Some(city) = st.city.as_ref() {
        st.interiors.ensure(city, building, to);
    }
    let place = Place::Floor {
        building,
        level: to,
    };
    let at = (crate::interiors::plan_now(st, building))
        .and_then(|plan| Some(plan.links.get(link)?.stop(to)?.at));
    let Some(land) = at.and_then(|at| free_landing(st, place, at)) else {
        return false;
    };
    let foe = &mut st.foes[i];
    foe.place = place;
    foe.pos = land;
    if let Some(post) = foe.post.as_mut() {
        post.round.lift = None;
    }
    true
}

/// Where a body coming off a link at tile `at` of floor `place` lands: `at`, else the
/// first of its eight neighbours that is walkable, no exit, no door a lock or a private
/// room holds (a landing there would be past a door it may have no card for), and free
/// of you, your body and every standing foe. None when the floor isn't drawn or nothing
/// there is free.
pub(crate) fn free_landing(st: &AppState, place: Place, at: Pos) -> Option<Pos> {
    let Place::Floor { building, level } = place else {
        return None;
    };
    let (_, floor) = st.interiors.floor(building, level)?;
    let you = st.player.as_ref();
    let held = |p: Pos| {
        foes::foe_at(st, place, p).is_some()
            || you.is_some_and(|y| (y.place == place && y.pos == p) || y.body == Some((place, p)))
    };
    std::iter::once(at).chain(at.neighbors8()).find(|p| {
        floor.tiles.get(*p).is_some_and(|t| t.is_walkable())
            && floor.exit_at(*p).is_none()
            && floor.lock_at(*p).is_none()
            && !floor.door_of(*p).is_some_and(|r| r.private)
            && !held(*p)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guards::GUARD_LIFT_TICKS;
    use crate::guards::testutil::*;
    use citygen::Pos;

    #[test]
    fn a_guard_takes_the_stairs_to_its_stop_and_back() {
        let mut st = ward();
        stand(&mut st, 0, Pos::new(28, 3)); // in the private room, out of sight
        let i = guard(&mut st, 0, 0, &[Pos::new(4, 3)]);
        st.foes[i].post.as_mut().unwrap().stop = Some((1, Pos::new(20, 3)));
        run_until(&mut st, 2000, |st| st.foes[i].place == on(1));
        assert_eq!(st.foes[i].pos, Pos::new(2, 1), "off the stairs");
        run_until(&mut st, 2000, |st| st.foes[i].pos == Pos::new(20, 3));
        // Back the way it came: the stairs, though 2F's lift (card 0 down to L) stands nearer.
        run_until(&mut st, 3000, |st| st.foes[i].place == on(0));
        assert_eq!(st.foes[i].pos, Pos::new(2, 1));
        run_until(&mut st, 3000, |st| st.foes[i].pos == Pos::new(4, 3));
    }

    #[test]
    fn a_guard_waits_for_the_lift_its_card_rides() {
        let mut st = ward();
        stand(&mut st, 0, Pos::new(28, 3));
        let i = guard(&mut st, 0, 1, &[Pos::new(12, 3)]);
        st.foes[i].post.as_mut().unwrap().stop = Some((1, Pos::new(20, 3)));
        run_until(&mut st, 600, |st| {
            st.foes[i].post.as_ref().unwrap().round.lift.is_some()
        });
        assert_eq!((st.foes[i].place, st.foes[i].pos), (on(0), Pos::new(12, 1)));
        run(&mut st, GUARD_LIFT_TICKS);
        assert_eq!((st.foes[i].place, st.foes[i].pos), (on(1), Pos::new(12, 1)));
    }

    #[test]
    fn a_zone_0_card_never_takes_the_carded_lift_and_an_alarm_sends_a_guard_to_the_stairs() {
        let mut st = ward();
        stand(&mut st, 0, Pos::new(28, 3));
        let plan = st.interiors.plan_of(WARD).unwrap().clone();
        assert!(
            hops(&st, &post(0, &[]), &plan, 1, 0).is_empty(),
            "the lift's 2F asks zone 1"
        );
        assert_eq!(hops(&st, &post(1, &[]), &plan, 1, 0), vec![1]);
        crate::net::lock_down(&mut st, WARD);
        assert!(
            hops(&st, &post(1, &[]), &plan, 1, 0).is_empty(),
            "the lift doesn't answer"
        );
        let i = guard(&mut st, 0, 1, &[Pos::new(12, 3)]);
        assert_eq!(link_to(&st, i, 1).map(|(k, _)| k), Some(0), "the stairs");
    }

    #[test]
    fn a_lift_that_stops_answering_keeps_a_guard_waiting() {
        let mut st = ward();
        stand(&mut st, 0, Pos::new(28, 3));
        let i = guard(&mut st, 0, 1, &[Pos::new(12, 1)]);
        st.foes[i].post.as_mut().unwrap().round.lift = Some((1, 1));
        crate::net::lock_down(&mut st, WARD);
        run(&mut st, 4 * GUARD_LIFT_TICKS);
        assert_eq!(st.foes[i].place, on(0));
    }

    #[test]
    fn a_guard_never_takes_a_crawl() {
        let st = ward();
        let mut plan = st.interiors.plan_of(WARD).unwrap().clone();
        plan.links[0].kind = citygen::LinkKind::Crawl;
        assert!(hops(&st, &post(2, &[]), &plan, 0, 0).is_empty());
    }

    #[test]
    fn a_guard_lands_beside_whoever_holds_the_landing() {
        let mut st = ward();
        stand(&mut st, 1, Pos::new(2, 1)); // you on 2F's stair landing
        let i = guard(&mut st, 0, 0, &[Pos::new(2, 1)]);
        assert!(arrive(&mut st, i, 0, 1));
        assert_eq!(st.foes[i].place, on(1));
        assert_eq!(st.foes[i].pos.chebyshev(Pos::new(2, 1)), 1);
    }

    #[test]
    fn a_guard_never_lands_on_a_lock_or_a_private_door() {
        let mut st = ward();
        doors_by_the_stair(&mut st, 1);
        stand(&mut st, 1, Pos::new(2, 1)); // you on 1F's stair landing
        let i = guard(&mut st, 0, 0, &[Pos::new(2, 1)]);
        let held = goon(&mut st, 1, Pos::new(1, 1)); // and its one plain tile held
        assert!(!arrive(&mut st, i, 0, 1), "no door: it waits");
        assert_eq!((st.foes[i].place, st.foes[i].pos), (on(0), Pos::new(2, 1)));
        st.foes[held].pos = Pos::new(20, 3);
        assert!(arrive(&mut st, i, 0, 1));
        assert_eq!((st.foes[i].place, st.foes[i].pos), (on(1), Pos::new(1, 1)));
    }
}
