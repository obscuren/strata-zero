//! Application state, the screen trait and the fixed-rate frame loop.

use crate::render::style::StylePack;
use crate::term::{TerminalGuard, install_panic_hook};
use anyhow::Result;
use citygen::{City, GenParams};
use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use ratatui::Frame;
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

/// How a screen change is animated. Replaced by `ui::transition` later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionKind {
    None,
    Glitch,
    SlideUp,
}

/// Placeholder transition: inactive until the ui module lands.
pub struct Transition;

impl Transition {
    pub fn new(_kind: TransitionKind, _from: Buffer, _start: u64) -> Self {
        Transition
    }
    pub fn active(&self, _tick: u64) -> bool {
        false
    }
    pub fn apply(&self, _f: &mut Frame, _tick: u64) {}
}

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

/// The fixed-rate loop: poll input, tick, update, draw.
pub fn run(mut screen: Box<dyn Screen>, mut st: AppState) -> Result<()> {
    install_panic_hook();
    let (_guard, mut term) = TerminalGuard::enter()?;
    let mut transition: Option<Transition> = None;
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
        st.tick += 1;
        if transition.as_ref().is_some_and(|t| t.active(st.tick)) && !keys.is_empty() {
            transition = None;
        }
        match screen.update(&mut st, &keys) {
            ScreenCmd::Stay => {}
            ScreenCmd::Switch { to, fx } => {
                let snapshot = term.current_buffer_mut().clone();
                screen = to;
                transition = Some(Transition::new(fx, snapshot, st.tick));
            }
            ScreenCmd::Quit => break,
        }
        term.draw(|f| {
            if f.area().width < MIN_W || f.area().height < MIN_H {
                draw_too_small(f);
                return;
            }
            screen.draw(&st, f);
            if let Some(t) = &transition {
                t.apply(f, st.tick);
            }
        })?;
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

    #[test]
    fn app_state_status_expires() {
        let mut st = AppState::new();
        st.set_status("hi", 10);
        assert_eq!(st.status_text(), Some("hi"));
        st.tick += 11;
        assert_eq!(st.status_text(), None);
    }
}
