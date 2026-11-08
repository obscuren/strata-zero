//! A world caught up after time away: a city you come back to is caught up before it's
//! seen. A short gap is replayed, a minute at a time with every trip found on its minute;
//! a long one puts everyone where their day says they'd be now. See
//! `2026-11-08-world-while-away-design.md`.

use super::{TICKS_PER_MINUTE, World};
use crate::people::{Activity, Dest, Goal, PersonId, Whereabouts};
use citygen::City;

/// The longest gap, in ticks, a world replays: a game hour. Longer, it's placed by day.
pub const SHORT_GAP: u64 = 60 * TICKS_PER_MINUTE;

/// What a catch-up did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatchUp {
    /// Nothing: the world was at the clock already.
    Current,
    /// The gap was replayed, minute by minute.
    Replayed,
    /// Everyone was put where their day says.
    Placed,
}

impl World {
    /// Catches the world up to `tick`, unseen: a gap of up to `SHORT_GAP` replayed, a
    /// longer one placed by day. A world at or past `tick` is left as it is.
    pub fn catch_up(&mut self, city: &City, tick: u64) -> CatchUp {
        if tick <= self.tick {
            return CatchUp::Current;
        }
        if tick - self.tick <= SHORT_GAP {
            self.replay_to(city, tick);
            CatchUp::Replayed
        } else {
            self.place_at(city, tick);
            CatchUp::Placed
        }
    }

    /// Puts everyone where their day says they'd be at `tick`, in one pass, the clock set
    /// there with no minute stepped: what a world long left needs, however long. The dead
    /// are left as they lie, and the people the story holds where it holds them, with what
    /// sends them on once let go. Chats and dates end. A trip in flight is dropped, its
    /// car parked at the kerb it was bound for, and nothing is left queued.
    pub fn place_at(&mut self, city: &City, tick: u64) {
        self.tick = tick;
        self.minute = tick / TICKS_PER_MINUTE;
        // the displaced back where they stood: the player is long gone
        let blocker = self.blocker.take();
        self.settle();
        self.blocker = blocker;
        self.displaced.clear();
        // chats end, everyone back to what they were doing
        for p in &mut self.people {
            if let Whereabouts::At { activity, .. } = &mut p.whereabouts
                && *activity == Activity::Chatting
            {
                *activity = self
                    .chat_resume
                    .get(&p.id)
                    .copied()
                    .unwrap_or(Activity::Working);
            }
        }
        self.chats.clear();
        self.chat_resume.clear();
        for d in std::mem::take(&mut self.dates) {
            self.date_over(city, d);
        }
        let minute = self.minute_of_day();
        for i in 0..self.people.len() {
            let id = PersonId(i as u32);
            if self.dead.contains(&id) || self.held.contains(&id) {
                continue;
            }
            self.park_cars_of(i);
            self.cut_short.remove(&id);
            let due = self.due(i, minute);
            // a goal they're done with (a client after a date) keeps them on the one they
            // have, as the minute step does
            let goal = match self.satisfied.get(&id) {
                Some(done) if *done == due => self.people[i].goal,
                _ => {
                    self.satisfied.remove(&id);
                    due
                }
            };
            self.people[i].goal = goal;
            // the story's cast off the map or on their way off it, with no road out now:
            // back in at once, never placed off the map
            let leaving = match &self.people[i].whereabouts {
                Whereabouts::Away => true,
                Whereabouts::Travelling(t) => matches!(t.dest, Dest::Away(_)),
                _ => false,
            };
            if leaving && self.way_out(i).is_none() {
                if let Some(dest) = [goal, Goal::Home, Goal::Work]
                    .into_iter()
                    .find_map(|g| self.dest_for(city, i, g))
                {
                    self.arrive(i, dest, goal);
                    continue;
                }
                if self.people[i].whereabouts == Whereabouts::Away {
                    continue;
                }
            }
            match self.dest_for(city, i, goal) {
                Some(dest) => self.arrive(i, dest, goal),
                // nowhere for the goal: where they are, or at the end of the trip they're on
                None => {
                    if let Whereabouts::Travelling(t) = &self.people[i].whereabouts {
                        let (dest, goal) = (t.dest, t.goal);
                        self.arrive(i, dest, goal);
                    }
                }
            }
        }
        // a date's end asks for the client's walk home; they're home already
        self.pending.clear();
        self.refresh_open();
    }

    /// Replays the minutes up to `tick` as the live clock runs them: each minute stepped,
    /// then every trip it asked for found, so a trip leaves on the minute it was asked for
    /// and nothing is left queued. Nothing happens when `tick` isn't ahead.
    pub fn replay_to(&mut self, city: &City, tick: u64) {
        while self.tick < tick {
            let next = (self.tick / TICKS_PER_MINUTE + 1) * TICKS_PER_MINUTE;
            self.advance(city, next.min(tick));
            self.path_some(city, usize::MAX);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::People;
    use crate::people::{Activity, Dest, Goal, Leg, Person, SimParams, Trip, Whereabouts, Work};
    use citygen::{GenParams, Generator, SizePreset};

    fn city() -> City {
        Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 8,
            ..Default::default()
        })
        .unwrap()
    }

    /// The city's people at seven in the morning, as the commute starts.
    fn morning(c: &City) -> World {
        World::seed(
            c,
            &People::load().unwrap(),
            &SimParams::default(),
            7 * 60,
            0,
        )
    }

    fn places(w: &World) -> Vec<Whereabouts> {
        w.people().iter().map(|p| p.whereabouts.clone()).collect()
    }

    /// How many people are somewhere other than `before` has them.
    fn moved(w: &World, before: &[Whereabouts]) -> usize {
        w.people()
            .iter()
            .zip(before)
            .filter(|(p, b)| p.whereabouts != **b)
            .count()
    }

    #[test]
    fn an_hour_replayed_leaves_nothing_queued() {
        let c = city();
        let mut w = morning(&c);
        w.replay_to(&c, SHORT_GAP);
        assert_eq!(w.tick(), SHORT_GAP);
        assert_eq!(w.pending(), 0);
    }

    #[test]
    fn an_hour_replayed_moves_people_as_a_live_hour_does() {
        let c = city();
        let mut live = morning(&c);
        let before = places(&live);
        for t in 1..=SHORT_GAP {
            live.advance(&c, t);
            live.path_some(&c, 4);
        }
        let mut bare = morning(&c);
        bare.advance(&c, SHORT_GAP);
        let mut replayed = morning(&c);
        replayed.replay_to(&c, SHORT_GAP);
        let (live, bare, replayed) = (
            moved(&live, &before),
            moved(&bare, &before),
            moved(&replayed, &before),
        );
        assert!(
            replayed * 10 >= live * 9,
            "replayed {replayed}, live {live}"
        );
        assert!(
            replayed > 2 * bare,
            "replayed {replayed}, a bare advance {bare}"
        );
    }

    #[test]
    fn catch_up_is_nothing_when_current() {
        let c = city();
        let mut w = morning(&c);
        let before = places(&w);
        assert_eq!(w.catch_up(&c, 0), CatchUp::Current);
        assert_eq!(w.tick(), 0);
        assert_eq!(places(&w), before);
    }

    #[test]
    fn catch_up_replays_up_to_an_hour() {
        let c = city();
        let mut w = morning(&c);
        assert_eq!(w.catch_up(&c, SHORT_GAP), CatchUp::Replayed);
        assert_eq!((w.tick(), w.pending()), (SHORT_GAP, 0));
    }

    #[test]
    fn catch_up_places_past_an_hour() {
        let c = city();
        let mut w = morning(&c);
        assert_eq!(w.catch_up(&c, SHORT_GAP + 1), CatchUp::Placed);
        assert_eq!((w.tick(), w.pending()), (SHORT_GAP + 1, 0));
    }

    /// Ten minutes of the morning run live, so some trips are in flight.
    fn under_way(c: &City, minutes: u64) -> World {
        let mut w = morning(c);
        for t in 1..=minutes * TICKS_PER_MINUTE {
            w.advance(c, t);
            w.path_some(c, 4);
        }
        w
    }

    #[test]
    fn placed_people_are_where_their_day_says() {
        let c = city();
        for hours in [6u64, 72] {
            let mut w = under_way(&c, 10);
            assert!(
                w.people()
                    .iter()
                    .any(|p| matches!(p.whereabouts, Whereabouts::Travelling(_))),
                "some trips to drop"
            );
            let at = w.tick() + hours * 60 * TICKS_PER_MINUTE;
            w.place_at(&c, at);
            assert_eq!((w.tick(), w.pending()), (at, 0));
            assert!(
                w.cars().iter().all(|car| car.parked && car.bound.is_none()),
                "no car on the road"
            );
            let minute = w.minute_of_day();
            for (i, p) in w.people().iter().enumerate() {
                assert!(
                    !matches!(p.whereabouts, Whereabouts::Travelling(_)),
                    "{} on the road",
                    p.name
                );
                if w.satisfied.contains_key(&p.id) {
                    continue;
                }
                assert_eq!(p.goal, w.due(i, minute), "{}", p.name);
                match (p.goal, p.home, p.work) {
                    (Goal::Home, Some(h), _) => {
                        assert_eq!(p.whereabouts, Whereabouts::Inside(h), "{}", p.name)
                    }
                    (Goal::Work, _, Work::Building(b)) => {
                        assert_eq!(p.whereabouts, Whereabouts::Inside(b), "{}", p.name)
                    }
                    (Goal::Work, _, Work::Spot(s)) => assert_eq!(
                        p.whereabouts,
                        Whereabouts::At {
                            pos: s,
                            activity: Activity::Working
                        },
                        "{}",
                        p.name
                    ),
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn placing_is_one_pass_whatever_the_gap() {
        // the same world placed at the same hour two days apart comes out the same
        let c = city();
        let (mut a, mut b) = (morning(&c), morning(&c));
        let six = 6 * 60 * TICKS_PER_MINUTE;
        let day = 24 * 60 * TICKS_PER_MINUTE;
        a.place_at(&c, six);
        b.place_at(&c, six + 2 * day);
        assert_eq!(a.minute_of_day(), b.minute_of_day());
        assert!(a.people() == b.people());
    }

    #[test]
    fn the_dead_the_held_and_the_cast_keep_what_the_story_gave_them() {
        let c = city();
        let mut w = under_way(&c, 20);
        let on_foot = |p: &Person| {
            matches!(&p.whereabouts,
                Whereabouts::Travelling(t) if t.legs.iter().all(|l| l.car.is_none()))
        };
        let walker = w.people().iter().find(|p| on_foot(p)).unwrap().id;
        w.keep_where(walker);
        let held_at = w.people()[walker.0 as usize].whereabouts.clone();
        let worker = w
            .people()
            .iter()
            .find(|p| p.id != walker && matches!(p.work, Work::Building(_)))
            .unwrap()
            .id;
        w.keep_at_work(worker);
        let worked_at = w.people()[worker.0 as usize].whereabouts.clone();
        let dead = w
            .people()
            .iter()
            .find(|p| ![walker, worker].contains(&p.id))
            .unwrap()
            .id;
        let dead_at = w.people()[dead.0 as usize].whereabouts.clone();
        w.kill(dead);
        // someone off the map when the story cast them (this city has no commuters, so
        // put there): kept in town, they have no road out
        let cast = w
            .people()
            .iter()
            .find(|p| ![walker, worker, dead].contains(&p.id))
            .unwrap()
            .id;
        w.people[cast.0 as usize].whereabouts = Whereabouts::Away;
        w.keep_in_town(cast);
        // fourteen hours on: the evening, when commuters have gone home
        w.place_at(&c, w.tick() + 14 * 60 * TICKS_PER_MINUTE);
        assert_eq!(w.people()[walker.0 as usize].whereabouts, held_at);
        assert_eq!(w.people()[worker.0 as usize].whereabouts, worked_at);
        assert_eq!(w.people()[worker.0 as usize].goal, Goal::Work);
        assert!(w.is_dead(dead));
        assert_eq!(w.people()[dead.0 as usize].whereabouts, dead_at);
        assert_ne!(w.people()[cast.0 as usize].whereabouts, Whereabouts::Away);
        // let go, the walker stopped mid-trip asks for a trip the next minute
        w.release(walker);
        let t = w.tick();
        w.advance(&c, t + TICKS_PER_MINUTE);
        assert!(
            w.pending.iter().any(|r| r.who == walker),
            "sent on once released"
        );
    }

    #[test]
    fn the_cast_walking_out_of_town_is_never_placed_off_the_map() {
        // cast while on their way off the map, with nowhere in town for the night's goal
        // (no home here): placed in town all the same
        let c = city();
        let mut w = morning(&c);
        let i = w
            .people()
            .iter()
            .position(|p| matches!(p.work, Work::Building(_)))
            .unwrap();
        let id = w.people()[i].id;
        let at = match w.people()[i].whereabouts {
            Whereabouts::At { pos, .. } => pos,
            _ => w.anchor(i).unwrap(),
        };
        w.people[i].home = None;
        w.people[i].whereabouts = Whereabouts::Travelling(Trip {
            from: at,
            depart: w.tick(),
            legs: vec![Leg {
                path: vec![at],
                car: None,
            }],
            goal: Goal::Home,
            dest: Dest::Away(at),
        });
        w.keep_in_town(id);
        // three in the morning, twenty hours on from seven
        let night = 20 * 60 * TICKS_PER_MINUTE;
        assert_eq!(w.due(i, 3 * 60), Goal::Home, "home is due");
        w.place_at(&c, night);
        assert_eq!(w.minute_of_day(), 3 * 60);
        let p = &w.people()[i];
        assert!(
            !matches!(
                p.whereabouts,
                Whereabouts::Away | Whereabouts::Travelling(_)
            ),
            "{:?}",
            p.whereabouts
        );
    }

    #[test]
    fn a_held_chatter_goes_back_to_what_they_were_doing() {
        let c = city();
        let mut w = morning(&c);
        let id = w
            .people()
            .iter()
            .find(|p| matches!(p.whereabouts, Whereabouts::At { .. }))
            .unwrap()
            .id;
        w.keep_where(id);
        let Whereabouts::At { pos, activity } = w.people()[id.0 as usize].whereabouts else {
            unreachable!()
        };
        // caught in a chat as the city was left
        w.chat_resume.insert(id, activity);
        w.people[id.0 as usize].whereabouts = Whereabouts::At {
            pos,
            activity: Activity::Chatting,
        };
        w.chats.push((pos, w.minute + 1));
        w.place_at(&c, w.tick() + 3 * 60 * TICKS_PER_MINUTE);
        assert_eq!(
            w.people()[id.0 as usize].whereabouts,
            Whereabouts::At { pos, activity }
        );
        assert!(w.chats.is_empty() && w.chat_resume.is_empty());
    }
}
