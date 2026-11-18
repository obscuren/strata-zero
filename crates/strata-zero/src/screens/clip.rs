//! The Keyjack clipped on a lock: the copy that runs, the run it makes of that lock and
//! the beat it is on. A keypad and a container's lock host it the same way; each keeps the
//! lock's faults and what a hit keys in, and the clip keeps the marker, the zone and what
//! the slab shows.

use crate::app::FPS;
use crate::keyjack;
use crate::screens::keypad::DENIED_TICKS;
use crate::ui::slab::{Panel, Shows};
use game::CopyId;

/// How the Keyjack's run ended, while it still hangs on the lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ended {
    /// The last fault burnt its fuse and locked the lock.
    Blown,
    /// The fourth hit opened it.
    Cracked,
}

/// What a strike came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Strike {
    /// Too soon, or the run is over.
    Nothing,
    /// The marker was in the zone on beat `beat`; the next beat is set.
    Hit { beat: usize },
    /// The fourth hit.
    Cracked,
    /// A miss: the host counts the fault and calls `Clip::faulted`.
    Miss,
}

/// The Keyjack clipped on a lock. The faults are the lock's, held by the host; `faults` is
/// their count as this run last saw it, for the lights (which stay lit on OPEN, though the
/// lock's clear).
#[derive(Clone, Debug)]
pub(crate) struct Clip {
    pub(crate) copy: CopyId,
    pub(crate) run: keyjack::Run,
    /// The lock's seed, for where its zones sit.
    pub(crate) seed: u64,
    /// The digit being cracked, left to right; 4 once all are.
    pub(crate) beat: usize,
    /// The zone's left cell.
    pub(crate) at: u16,
    /// The tick the marker sets off from the bar's left end. After a miss it lies ahead,
    /// and ERR shows until then.
    pub(crate) from: u64,
    /// The last strike missed.
    pub(crate) missed: bool,
    pub(crate) faults: u8,
    pub(crate) ended: Option<Ended>,
}

impl Clip {
    /// Clipped on at `tick`, on beat 0 with its zone placed.
    pub(crate) fn new(copy: CopyId, run: keyjack::Run, seed: u64, faults: u8, tick: u64) -> Clip {
        let mut clip = Clip {
            copy,
            run,
            seed,
            beat: 0,
            at: 0,
            from: tick,
            missed: false,
            faults,
            ended: None,
        };
        clip.place();
        clip
    }

    /// The zone's width on this beat.
    pub(crate) fn width(&self) -> u16 {
        self.run.zones[self.beat.min(3)]
    }

    /// The marker's cell at `tick`: off from the left end at the beat's speed, end to end
    /// and back, a cell's time at each end. A tick is a frame.
    pub(crate) fn marker(&self, tick: u64) -> u16 {
        let bar = self.run.bar.max(1);
        let speed = self.run.speed[self.beat.min(3)] as f64;
        let cells = tick.saturating_sub(self.from) as f64 * speed / FPS as f64;
        let p = (cells % (2 * bar) as f64) as u16;
        if p < bar { p } else { 2 * bar - 1 - p }
    }

    /// A strike with the marker on `cell` lands in the zone.
    pub(crate) fn hits(&self, cell: u16) -> bool {
        cell >= self.at && cell < self.at + self.width()
    }

    /// Puts this beat's zone where the lock and its faults so far put it; after a miss,
    /// never back where it was.
    pub(crate) fn place(&mut self) {
        let (w, bar) = (self.width(), self.run.bar);
        let seed = citygen::rng::hash64(self.seed, self.faults as u64);
        let mut at = keyjack::zone_at(seed, self.beat, w, bar);
        let room = bar.saturating_sub(w);
        if self.missed && at == self.at && room > 0 {
            at = (at + room.div_ceil(2)) % (room + 1);
        }
        self.at = at;
    }

    /// A strike at `tick`. The frame moves the tick on before the keys come in, so the
    /// space was pressed on the frame before: that is where it is judged, on the marker
    /// the player saw. A beat starts on this tick, so a second space in the same keys,
    /// pressed before its first frame showed, strikes nothing.
    pub(crate) fn strike(&mut self, tick: u64) -> Strike {
        let shown = tick.saturating_sub(1);
        if self.ended.is_some() || shown < self.from {
            return Strike::Nothing;
        }
        if !self.hits(self.marker(shown)) {
            return Strike::Miss;
        }
        let beat = self.beat;
        self.beat += 1;
        self.missed = false;
        if self.beat == 4 {
            self.ended = Some(Ended::Cracked);
            return Strike::Cracked;
        }
        self.from = tick;
        self.place();
        Strike::Hit { beat }
    }

    /// A miss counted: the lock now has `faults`. The last blows the fuse (true); else
    /// the beat starts over after ERR.
    pub(crate) fn faulted(&mut self, faults: u8, tick: u64) -> bool {
        self.faults = faults;
        if faults >= self.run.faults {
            self.ended = Some(Ended::Blown);
            return true;
        }
        self.missed = true;
        self.from = tick + DENIED_TICKS;
        self.place();
        false
    }

    /// The slab's frame at `tick`, with `digits` cracked so far.
    pub(crate) fn panel(&self, tick: u64, digits: &[u8]) -> Panel {
        let shows = match self.ended {
            Some(Ended::Blown) => Shows::Blown,
            Some(Ended::Cracked) => Shows::Open,
            None if self.missed && tick < self.from => Shows::Err,
            None => Shows::Sweep,
        };
        Panel {
            shows,
            bar: self.run.bar,
            zone: (self.at, self.width()),
            marker: self.marker(tick),
            digits: digits.to_vec(),
            faults: self.faults,
            allowed: self.run.faults,
            beat: self.beat.min(3),
            rating: self.run.rating,
        }
    }

    /// The clock moved on to the frame after one that shows the marker in the zone (or
    /// out of it), so a strike now hits (or misses), for a test or a shot to aim with.
    #[cfg(test)]
    pub(crate) fn aim_tick(&self, now: u64, hit: bool) -> u64 {
        let start = now.max(self.from);
        let shown = (start..start + 3_000)
            .find(|t| self.hits(self.marker(*t)) == hit)
            .expect("the marker gets there");
        shown + 1
    }
}
