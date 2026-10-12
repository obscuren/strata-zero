//! Application state, the screen trait and the fixed-rate frame loop.

use crate::render::style::StylePack;
use crate::term::{TerminalGuard, install_panic_hook};
use anyhow::Result;
use citygen::{City, GenParams};
use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::Paragraph;
use std::time::{Duration, Instant};

pub const MIN_W: u16 = 100;
pub const MIN_H: u16 = 30;
pub const FRAME: Duration = Duration::from_micros(33_333);

/// State shared by every screen.
pub struct AppState {
    pub params: GenParams,
    pub city: Option<City>,
    pub player: Option<crate::walker::Player>,
    pub style: StylePack,
    pub tick: u64,
    status: Option<(String, u64)>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            params: GenParams::default(),
            city: None,
            player: None,
            style: StylePack::load().expect("embedded style pack is valid"),
            tick: 0,
            status: None,
        }
    }

    /// Shows a one-line message for `ttl` ticks.
    pub fn set_status(&mut self, msg: impl Into<String>, ttl: u64) {
        self.status = Some((msg.into(), self.tick + ttl));
    }

    pub fn status_text(&self) -> Option<&str> {
        match &self.status {
            Some((msg, until)) if *until > self.tick => Some(msg.as_str()),
            _ => None,
        }
    }
}

pub use crate::ui::transition::{Transition, TransitionKind};

/// What a screen asks the loop to do after an update.
pub enum ScreenCmd {
    Stay,
    Switch {
        to: Box<dyn Screen>,
        fx: TransitionKind,
    },
    Quit,
}

pub trait Screen {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd;
    fn draw(&self, st: &AppState, f: &mut Frame);
}

/// Centered notice for terminals under the minimum size. Never panics, even at 1x1.
pub fn draw_too_small(f: &mut Frame) {
    let area = f.area();
    let msg = format!(
        "resize to at least {MIN_W}x{MIN_H} (now {}x{})",
        area.width, area.height
    );
    let y = area.height / 2;
    let line = Rect::new(area.x, area.y + y, area.width, 1.min(area.height));
    f.render_widget(
        Paragraph::new(msg)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Rgb(255, 80, 120))),
        line,
    );
}

/// One screen plus the state the loop threads through frames.
pub struct Runner {
    screen: Box<dyn Screen>,
    pub st: AppState,
    transition: Option<Transition>,
    /// Our own copy of the last drawn frame; ratatui swaps buffers after every draw.
    last_frame: Option<Buffer>,
}

impl Runner {
    pub fn new(screen: Box<dyn Screen>, st: AppState) -> Self {
        Runner {
            screen,
            st,
            transition: None,
            last_frame: None,
        }
    }

    #[cfg(test)]
    pub fn transition(&self) -> Option<&Transition> {
        self.transition.as_ref()
    }

    /// Runs one frame: tick, update with `keys`, draw. Returns false when the app quits.
    pub fn frame<B: Backend>(&mut self, term: &mut Terminal<B>, keys: &[KeyEvent]) -> Result<bool> {
        self.st.tick += 1;
        if self
            .transition
            .as_ref()
            .is_some_and(|t| t.active(self.st.tick))
            && !keys.is_empty()
        {
            self.transition = None;
        }
        match self.screen.update(&mut self.st, keys) {
            ScreenCmd::Stay => {}
            ScreenCmd::Switch { to, fx } => {
                let snapshot = self
                    .last_frame
                    .clone()
                    .unwrap_or_else(|| Buffer::empty(term.get_frame().area()));
                self.screen = to;
                self.transition = Some(Transition::new(fx, snapshot, self.st.tick));
            }
            ScreenCmd::Quit => return Ok(false),
        }
        let st = &self.st;
        let screen = &self.screen;
        let transition = &self.transition;
        let mut captured: Option<Buffer> = None;
        term.draw(|f| {
            if f.area().width < MIN_W || f.area().height < MIN_H {
                draw_too_small(f);
            } else {
                screen.draw(st, f);
                if let Some(t) = transition {
                    t.apply(f, st.tick);
                }
            }
            captured = Some(f.buffer_mut().clone());
        })?;
        self.last_frame = captured;
        Ok(true)
    }
}

/// The fixed-rate loop: poll input, run a frame, sleep the remainder.
pub fn run(screen: Box<dyn Screen>, st: AppState) -> Result<()> {
    install_panic_hook();
    let (_guard, mut term) = TerminalGuard::enter()?;
    let mut runner = Runner::new(screen, st);
    loop {
        let start = Instant::now();
        let mut keys = Vec::new();
        while event::poll(Duration::ZERO)? {
            if let Event::Key(k) = event::read()?
                && k.kind == KeyEventKind::Press
            {
                keys.push(k);
            }
        }
        if !runner.frame(&mut term, &keys)? {
            break;
        }
        if let Some(rest) = FRAME.checked_sub(start.elapsed()) {
            std::thread::sleep(rest);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn text(t: &Terminal<TestBackend>) -> String {
        let b = t.backend().buffer();
        let mut s = String::new();
        for y in 0..b.area.height {
            for x in 0..b.area.width {
                s.push_str(b[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn too_small_message_never_panics() {
        for (w, h) in [(1u16, 1u16), (99, 29), (10, 2)] {
            let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
            t.draw(draw_too_small).unwrap();
        }
        let mut t = Terminal::new(TestBackend::new(99, 29)).unwrap();
        t.draw(draw_too_small).unwrap();
        assert!(text(&t).contains("100"));
    }

    struct Says(&'static str, bool);

    impl Screen for Says {
        fn update(&mut self, _st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
            if self.1 && !keys.is_empty() {
                return ScreenCmd::Switch {
                    to: Box::new(Says("WORLD", false)),
                    fx: TransitionKind::SlideUp,
                };
            }
            ScreenCmd::Stay
        }
        fn draw(&self, _st: &AppState, f: &mut Frame) {
            f.render_widget(ratatui::widgets::Paragraph::new(self.0), f.area());
        }
    }

    #[test]
    fn switch_snapshots_the_previous_frame() {
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut runner = Runner::new(Box::new(Says("HELLO", true)), AppState::new());
        assert!(runner.frame(&mut t, &[]).unwrap());
        assert!(text(&t).contains("HELLO"));
        let key = KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        );
        assert!(runner.frame(&mut t, &[key]).unwrap());
        let tr = runner.transition().expect("a transition is running");
        let snapshot: String = (0..5)
            .map(|x| tr.from[(x, 0)].symbol().to_string())
            .collect();
        assert_eq!(snapshot, "HELLO");
        assert!(text(&t).contains("WORLD") || text(&t).contains("HELLO"));
    }

    #[test]
    fn app_state_status_expires() {
        let mut st = AppState::new();
        st.set_status("hi", 10);
        assert_eq!(st.status_text(), Some("hi"));
        st.tick += 11;
        assert_eq!(st.status_text(), None);
    }
}
