//! Taking the stairs and the lifts: which links a guard's card rides, the one it walks
//! to, taking it, and landing beside whoever holds the other end.

use super::{GUARD_LIFT_TICKS, GUARD_STEP_TICKS};
use crate::app::AppState;
use crate::foes::{self, Post};
use crate::render::map::{MapRef, TileMap};
use citygen::pathing::astar;
use citygen::{FloorPlan, LinkKind, Place, Pos};
use std::collections::BTreeMap;

/// The levels a guard of `post` reaches by link `link` of `plan` from level `from` in one
/// go: none by a crawl; a lift's rides to stops its card opens, alarm or not (a lockdown
/// shuts the lifts to intruders, not to security); a stair's, an escalator's or a
/// ladder's next stop up and down, which ask no card.
pub fn hops(post: &Post, plan: &FloorPlan, link: usize, from: i32) -> Vec<i32> {
    let hops = plan.hops(link, from);
    if !is_lift(plan, link) {
        return hops;
    }
    (hops.into_iter())
        .filter(|to| crate::venues::ride_card(plan, link, *to) <= post.zone)
        .collect()
}

/// Whether link `link` of `plan` is a lift.
pub(crate) fn is_lift(plan: &FloorPlan, link: usize) -> bool {
    plan.links
        .get(link)
        .is_some_and(|l| l.kind == LinkKind::Lift)
}

/// The link guard `i` takes toward level `to`, its stop tile on the guard's level, and
/// the level it rides to: the one it came by (`round.via`) while that still goes there,
/// so it comes back the way it went; else of the links one hop there, the nearest by its
/// walk (ties: link order). With none (a stair goes a stop at a time), the first link of
/// the fewest hops there its card rides, the nearest of those the same way.
pub(crate) fn link_to(st: &AppState, i: usize, to: i32) -> Option<(usize, Pos, i32)> {
    let foe = st.foes.get(i)?;
    let post = foe.post.as_ref()?;
    let Place::Floor { building, level } = foe.place else {
        return None;
    };
    let plan = crate::interiors::plan_now(st, building)?;
    let stop = |k: usize| Some((k, plan.links.get(k)?.stop(level)?.at));
    let goes = |k: &usize| hops(post, &plan, *k, level).contains(&to);
    if let Some((k, at)) = post.round.via.filter(goes).and_then(stop) {
        return Some((k, at, to));
    }
    let map = foes::map_of(st, foe.place)?;
    let nearest = |ways: Vec<(usize, i32)>| {
        (ways.into_iter())
            .filter_map(|(k, hop)| Some((stop(k)?, hop)))
            .filter_map(|((k, at), hop)| Some((walk(&map, post, foe.pos, at)?, k, at, hop)))
            .min_by_key(|(steps, k, _, _)| (*steps, *k))
            .map(|(_, k, at, hop)| (k, at, hop))
    };
    let one_hop = (0..plan.links.len())
        .filter(goes)
        .map(|k| (k, to))
        .collect();
    if let Some(way) = nearest(one_hop) {
        return Some(way);
    }
    let dist = hops_to(post, &plan, to);
    let ways: Vec<(usize, usize, i32)> = (0..plan.links.len())
        .filter_map(|k| {
            (hops(post, &plan, k, level).into_iter())
                .filter_map(|hop| Some((*dist.get(&hop)?, k, hop)))
                .min()
        })
        .collect();
    let fewest = ways.iter().map(|(d, _, _)| *d).min()?;
    nearest(
        (ways.into_iter())
            .filter(|(d, _, _)| *d == fewest)
            .map(|(_, k, hop)| (k, hop))
            .collect(),
    )
}

/// The fewest hops from each level of `plan` to level `to` by the links a guard of
/// `post` rides (`hops`), for the levels that reach it at all: `to` itself 0.
pub(super) fn hops_to(post: &Post, plan: &FloorPlan, to: i32) -> BTreeMap<i32, usize> {
    let rides: Vec<(i32, Vec<i32>)> = (plan.levels.iter())
        .map(|l| {
            let next = (0..plan.links.len()).flat_map(|k| hops(post, plan, k, *l));
            (*l, next.collect())
        })
        .collect();
    let mut dist = BTreeMap::from([(to, 0)]);
    for d in 1..=rides.len() {
        let reached: Vec<i32> = (rides.iter())
            .filter(|(l, next)| {
                !dist.contains_key(l) && next.iter().any(|n| dist.get(n) == Some(&(d - 1)))
            })
            .map(|(l, _)| *l)
            .collect();
        if reached.is_empty() {
            break;
        }
        dist.extend(reached.into_iter().map(|l| (l, d)));
    }
    dist
}

/// The steps of a guard's walk from `from` to `to` on `map`, its card's doors open:
/// None when there's no way.
pub(super) fn walk(map: &MapRef, post: &Post, from: Pos, to: Pos) -> Option<usize> {
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
    let lift = (crate::interiors::plan_now(st, building)).is_some_and(|plan| is_lift(&plan, link));
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
/// longer goes there from the guard's level (a save from another build), or whose stop
/// it no longer stands on (it went off hunting meanwhile), is let go.
pub(crate) fn wait_for_lift(st: &mut AppState, i: usize, link: usize, to: i32, now: u64) {
    let pos = st.foes[i].pos;
    let rides = match st.foes[i].place {
        Place::Floor { building, level } => crate::interiors::plan_now(st, building).map(|plan| {
            let post = st.foes[i].post.as_ref();
            let on_stop = (plan.links.get(link))
                .and_then(|l| l.stop(level))
                .is_some_and(|s| s.at == pos);
            (
                on_stop && plan.hops(link, level).contains(&to),
                post.is_some_and(|p| hops(p, &plan, link, level).contains(&to)),
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

/// Guard `i` lets go of a lift it waits for, its way back (`round.via`) and a trail it
/// follows (`follow`) whose link no longer stops on the level it stands on: each names a
/// link by its place in a plan made again from code, which a later build may order
/// otherwise. Called on reopening and before each step.
pub(crate) fn forget_stale(st: &mut AppState, i: usize) {
    let Some(foe) = st.foes.get(i) else {
        return;
    };
    let (Some(post), Place::Floor { building, level }) = (foe.post.as_ref(), foe.place) else {
        return;
    };
    let Some(plan) = crate::interiors::plan_now(st, building) else {
        return;
    };
    let stale = |link: Option<usize>| link.is_some_and(|k| plan.hops(k, level).is_empty());
    let (lift, via, follow) = (
        stale(post.round.lift.map(|(k, _)| k)),
        stale(post.round.via),
        stale(post.follow.map(|(k, _)| k)),
    );
    drop(plan);
    let Some(post) = st.foes[i].post.as_mut() else {
        return;
    };
    if lift {
        post.round.lift = None;
    }
    if via {
        post.round.via = None;
    }
    if follow {
        post.follow = None;
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
    fn a_guard_two_floors_from_home_walks_back_into_its_round() {
        let mut st = ward3();
        stand(&mut st, 0, Pos::new(28, 3)); // in the private room, out of sight
        let i = guard(&mut st, 2, 0, &[Pos::new(6, 3), Pos::new(10, 3)]);
        // It followed you up twice and lost you: calm on 3F, its round on L.
        st.foes[i].pos = Pos::new(20, 3);
        st.foes[i].post.as_mut().unwrap().home = Some(0);
        run_until(&mut st, 3000, |st| st.foes[i].place == on(1));
        run_until(&mut st, 3000, |st| st.foes[i].place == on(0));
        run_until(&mut st, 3000, |st| st.foes[i].pos == Pos::new(10, 3));
        assert_eq!(st.foes[i].post.as_ref().unwrap().round.via, None);
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
    fn a_zone_0_card_never_takes_the_carded_lift_and_an_alarm_still_lets_a_guard_ride() {
        let mut st = ward();
        stand(&mut st, 0, Pos::new(28, 3));
        let plan = st.interiors.plan_of(WARD).unwrap().clone();
        assert!(
            hops(&post(0, &[]), &plan, 1, 0).is_empty(),
            "the lift's 2F asks zone 1"
        );
        assert_eq!(hops(&post(1, &[]), &plan, 1, 0), vec![1]);
        crate::net::lock_down(&mut st, WARD);
        assert_eq!(
            hops(&post(1, &[]), &plan, 1, 0),
            vec![1],
            "a lockdown shuts the lift to you, not to security"
        );
        let i = guard(&mut st, 0, 1, &[Pos::new(12, 3)]);
        assert_eq!(
            link_to(&st, i, 1).map(|(k, _, _)| k),
            Some(1),
            "the nearer lift"
        );
    }

    #[test]
    fn on_alert_a_waiting_guard_still_rides_the_lift() {
        let mut st = ward();
        stand(&mut st, 0, Pos::new(28, 3));
        let i = guard(&mut st, 0, 1, &[Pos::new(12, 1)]);
        st.foes[i].post.as_mut().unwrap().round.lift = Some((1, 1));
        crate::net::lock_down(&mut st, WARD);
        run_until(&mut st, 4 * GUARD_LIFT_TICKS, |st| {
            st.foes[i].place == on(1)
        });
        assert_eq!(st.foes[i].post.as_ref().unwrap().round.lift, None);
    }

    #[test]
    fn a_lift_wait_is_let_go_once_the_guard_is_off_its_stop() {
        let mut st = ward();
        stand(&mut st, 1, Pos::new(28, 3));
        let i = guard(&mut st, 0, 1, &[Pos::new(10, 3)]);
        // It waited at (12,1), then went off hunting and lost you here.
        st.foes[i].post.as_mut().unwrap().round.lift = Some((1, 1));
        run(&mut st, 2 * GUARD_LIFT_TICKS);
        assert_eq!(
            st.foes[i].place,
            on(0),
            "never carried from where it stands"
        );
        assert_eq!(st.foes[i].post.as_ref().unwrap().round.lift, None);
    }

    #[test]
    fn a_guard_never_takes_a_crawl() {
        let st = ward();
        let mut plan = st.interiors.plan_of(WARD).unwrap().clone();
        plan.links[0].kind = citygen::LinkKind::Crawl;
        assert!(hops(&post(2, &[]), &plan, 0, 0).is_empty());
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
