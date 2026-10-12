//! Screen transitions: a glitch wipe or a vertical slide, a few ticks long.

use crate::anim::effects::glitch_cells;
use citygen::rng::hash64;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::style::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionKind {
    None,
    Glitch,
    SlideUp,
}

/// A transition from a snapshot of the previous frame into the new screen.
pub struct Transition {
    pub kind: TransitionKind,
    pub from: Buffer,
    pub start: u64,
    pub dur: u64,
}

impl Transition {
    pub fn new(kind: TransitionKind, from: Buffer, start: u64) -> Self {
        let dur = match kind {
            TransitionKind::None => 0,
            TransitionKind::Glitch => 6,
            TransitionKind::SlideUp => 8,
        };
        Transition {
            kind,
            from,
            start,
            dur,
        }
    }

    pub fn active(&self, tick: u64) -> bool {
        self.dur > 0 && tick >= self.start && tick < self.start + self.dur
    }

    fn progress(&self, tick: u64) -> f32 {
        if self.dur == 0 {
            return 1.0;
        }
        ((tick.saturating_sub(self.start)) as f32 / self.dur as f32).clamp(0.0, 1.0)
    }

    /// Draws the effect over the already-rendered new frame.
    pub fn apply(&self, f: &mut Frame, tick: u64) {
        if !self.active(tick) {
            return;
        }
        let p = self.progress(tick);
        let area = f.area();
        let buf = f.buffer_mut();
        let same_size = self.from.area.width == area.width && self.from.area.height == area.height;
        match self.kind {
            TransitionKind::None => {}
            TransitionKind::Glitch => {
                if same_size && p < 0.5 {
                    for y in 0..area.height {
                        if hash64(tick, y as u64).is_multiple_of(3) {
                            for x in 0..area.width {
                                let src = &self.from[(x, y)];
                                buf[(area.x + x, area.y + y)] = src.clone();
                            }
                        }
                    }
                }
                for (x, y, ch) in glitch_cells(area, tick, p) {
                    let tint = if hash64(x as u64, y as u64).is_multiple_of(2) {
                        Color::Rgb(0, 255, 230)
                    } else {
                        Color::Rgb(255, 0, 200)
                    };
                    buf[(x, y)].set_char(ch).set_fg(tint);
                }
            }
            TransitionKind::SlideUp => {
                if !same_size {
                    return;
                }
                let shift = (p * area.height as f32).round() as u16;
                for y in 0..area.height.saturating_sub(shift) {
                    for x in 0..area.width {
                        let src = &self.from[(x, y + shift)];
                        buf[(area.x + x, area.y + y)] = src.clone();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    fn glitch_transition_ends_and_applies() {
        let from = Buffer::empty(Rect::new(0, 0, 20, 5));
        let tr = Transition::new(TransitionKind::Glitch, from.clone(), 10);
        assert!(tr.active(12));
        assert!(!tr.active(17));
        assert!(!Transition::new(TransitionKind::None, from.clone(), 10).active(10));
        let mut t = Terminal::new(TestBackend::new(20, 5)).unwrap();
        for kind in [TransitionKind::Glitch, TransitionKind::SlideUp] {
            let tr = Transition::new(kind, from.clone(), 10);
            for tick in 10..20 {
                t.draw(|f| tr.apply(f, tick)).unwrap();
            }
        }
        // A snapshot of a different size is ignored, not indexed out of bounds.
        let big = Buffer::empty(Rect::new(0, 0, 80, 30));
        let tr = Transition::new(TransitionKind::SlideUp, big, 10);
        t.draw(|f| tr.apply(f, 12)).unwrap();
    }
}
