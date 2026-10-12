//! Tick-driven animation primitives: easing, tweens, timelines.

pub mod effects;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Easing {
    Linear,
    OutCubic,
    InOutCubic,
    OutBack,
}

impl Easing {
    /// Maps linear progress in 0..=1 to eased progress.
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::OutCubic => 1.0 - (1.0 - t).powi(3),
            Easing::InOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Easing::OutBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
        }
    }
}

/// One animated value running from `start` for `dur` ticks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tween {
    pub start: u64,
    pub dur: u64,
    pub easing: Easing,
}

impl Tween {
    pub fn new(start: u64, dur: u64, easing: Easing) -> Self {
        Tween { start, dur, easing }
    }

    /// Eased progress in 0..=1 at `tick`.
    pub fn progress(&self, tick: u64) -> f32 {
        if tick <= self.start {
            return 0.0;
        }
        if self.dur == 0 || tick >= self.start + self.dur {
            return 1.0;
        }
        self.easing
            .apply((tick - self.start) as f32 / self.dur as f32)
    }

    pub fn done(&self, tick: u64) -> bool {
        tick >= self.start + self.dur
    }

    pub fn active(&self, tick: u64) -> bool {
        tick >= self.start && !self.done(tick)
    }
}

/// Named tweens, looked up by label.
#[derive(Clone, Debug, Default)]
pub struct Timeline {
    tweens: Vec<(&'static str, Tween)>,
}

impl Timeline {
    pub fn new(tweens: Vec<(&'static str, Tween)>) -> Self {
        Timeline { tweens }
    }

    /// Progress of `label`; unknown labels count as finished.
    pub fn progress(&self, label: &str, tick: u64) -> f32 {
        self.tweens
            .iter()
            .find(|(l, _)| *l == label)
            .map(|(_, t)| t.progress(tick))
            .unwrap_or(1.0)
    }

    pub fn all_done(&self, tick: u64) -> bool {
        self.tweens.iter().all(|(_, t)| t.done(tick))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tween_clamps_and_eases() {
        let t = Tween {
            start: 10,
            dur: 10,
            easing: Easing::Linear,
        };
        assert_eq!(t.progress(5), 0.0);
        assert_eq!(t.progress(15), 0.5);
        assert_eq!(t.progress(30), 1.0);
        assert!(t.done(20));
        assert!(t.active(12));
        assert!(!t.active(5));
        assert!(Easing::OutCubic.apply(0.5) > 0.5);
        assert_eq!(Easing::OutBack.apply(1.0), 1.0);
        assert_eq!(Easing::InOutCubic.apply(0.0), 0.0);
    }

    #[test]
    fn timeline_lookup() {
        let tl = Timeline::new(vec![
            (
                "a",
                Tween {
                    start: 0,
                    dur: 10,
                    easing: Easing::Linear,
                },
            ),
            (
                "b",
                Tween {
                    start: 10,
                    dur: 10,
                    easing: Easing::Linear,
                },
            ),
        ]);
        assert_eq!(tl.progress("a", 5), 0.5);
        assert_eq!(tl.progress("b", 5), 0.0);
        assert_eq!(tl.progress("zzz", 5), 1.0);
        assert!(!tl.all_done(15));
        assert!(tl.all_done(20));
    }
}
