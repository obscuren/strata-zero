//! A world caught up after time away: a city you come back to is caught up before it's
//! seen. A short gap is replayed, a minute at a time with every trip found on its minute;
//! a long one puts everyone where their day says they'd be now. See
//! `2026-11-08-world-while-away-design.md`.

use super::{TICKS_PER_MINUTE, World};
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
        self.replay_to(city, tick);
        CatchUp::Replayed
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
    use crate::people::{SimParams, Whereabouts};
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
}
