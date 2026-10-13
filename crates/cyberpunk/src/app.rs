//! Application state, the screen trait and the fixed-rate frame loop.

use crate::messages::{Kind, MessageLog};
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

/// Whether the app starts in dev mode: free camera, map browsing and the inspector.
pub const DEV_DEFAULT: bool = true;

/// State shared by every screen.
pub struct AppState {
    pub params: GenParams,
    pub city: Option<City>,
    pub player: Option<crate::walker::Player>,
    pub style: StylePack,
    pub tick: u64,
    /// Dev mode: free camera, stratum browsing and the inspector. Toggled with `` ` ``.
    pub dev: bool,
    /// Rain overlay on the map (Settings).
    pub rain: bool,
    /// FPS readout in the status bar (Settings).
    pub show_fps: bool,
    pub content: game::Content,
    pub character: game::Character,
    /// Items lying in the city.
    pub ground: game::Ground,
    /// A ping in flight: lights terminals and stairs around it until `until`.
    pub ping: Option<Ping>,
    /// What happened, newest last.
    pub log: MessageLog,
}

/// A handshake burst from the `ping` ability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ping {
    pub stratum: citygen::StratumKind,
    pub pos: citygen::Pos,
    pub radius: i32,
    pub until: u64,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        let content = game::Content::load().expect("embedded content pack is valid");
        let character = game::Character::from_starter(&content);
        AppState {
            params: GenParams::default(),
            city: None,
            player: None,
            style: StylePack::load().expect("embedded style pack is valid"),
            tick: 0,
            dev: DEV_DEFAULT,
            rain: true,
            show_fps: true,
            content,
            character,
            ground: game::Ground::default(),
            ping: None,
            log: MessageLog::default(),
        }
    }

    /// Logs a plain result.
    pub fn say(&mut self, msg: impl Into<String>) {
        self.log.push(Kind::Info, msg, self.tick);
    }

    /// Logs a refusal.
    pub fn warn(&mut self, msg: impl Into<String>) {
        self.log.push(Kind::Warn, msg, self.tick);
    }

    /// Logs a quest or level line.
    pub fn story(&mut self, msg: impl Into<String>) {
        self.log.push(Kind::Story, msg, self.tick);
    }
}

pub use crate::ui::transition::{Transition, TransitionKind};

/// What a screen asks the loop to do after an update.
pub enum ScreenCmd {
    Stay,
    /// Replace the whole stack with one screen.
    Switch {
        to: Box<dyn Screen>,
        fx: TransitionKind,
    },
    /// Put a screen on top; the ones below keep drawing, washed, and stop updating.
    Push {
        to: Box<dyn Screen>,
        fx: TransitionKind,
    },
    /// Remove the top screen. Ignored on the last one.
    Pop {
        fx: TransitionKind,
    },
    /// Run a transition in place without changing screens.
    Flash(TransitionKind),
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

/// The screen stack plus the state the loop threads through frames.
pub struct Runner {
    screens: Vec<Box<dyn Screen>>,
    pub st: AppState,
    transition: Option<Transition>,
    /// Our own copy of the last drawn frame; ratatui swaps buffers after every draw.
    last_frame: Option<Buffer>,
}

impl Runner {
    pub fn new(screen: Box<dyn Screen>, st: AppState) -> Self {
        Runner {
            screens: vec![screen],
            st,
            transition: None,
            last_frame: None,
        }
    }

    #[cfg(test)]
    pub fn transition(&self) -> Option<&Transition> {
        self.transition.as_ref()
    }

    #[cfg(test)]
    pub fn depth(&self) -> usize {
        self.screens.len()
    }

    fn snapshot(&self, area: Rect) -> Buffer {
        self.last_frame
            .clone()
            .unwrap_or_else(|| Buffer::empty(area))
    }

    /// Runs one frame: tick, update the top screen with `keys`, draw the stack. Returns
    /// false when the app quits.
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
        let area = term.get_frame().area();
        let top = self.screens.last_mut().expect("the stack is never empty");
        match top.update(&mut self.st, keys) {
            ScreenCmd::Stay => {}
            ScreenCmd::Switch { to, fx } => {
                let snapshot = self.snapshot(area);
                self.screens = vec![to];
                self.transition = Some(Transition::new(fx, snapshot, self.st.tick));
            }
            ScreenCmd::Push { to, fx } => {
                let snapshot = self.snapshot(area);
                self.screens.push(to);
                self.transition = Some(Transition::new(fx, snapshot, self.st.tick));
            }
            ScreenCmd::Pop { fx } => {
                if self.screens.len() > 1 {
                    let snapshot = self.snapshot(area);
                    self.screens.pop();
                    self.transition = Some(Transition::new(fx, snapshot, self.st.tick));
                }
            }
            ScreenCmd::Flash(fx) => {
                let snapshot = self.snapshot(area);
                self.transition = Some(Transition::flash(fx, snapshot, self.st.tick));
            }
            ScreenCmd::Quit => return Ok(false),
        }
        let st = &self.st;
        let screens = &self.screens;
        let transition = &self.transition;
        let mut captured: Option<Buffer> = None;
        term.draw(|f| {
            if f.area().width < MIN_W || f.area().height < MIN_H {
                draw_too_small(f);
            } else {
                for (i, screen) in screens.iter().enumerate() {
                    if i > 0 {
                        crate::ui::dim::wash(f.buffer_mut());
                    }
                    screen.draw(st, f);
                }
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

    use std::cell::Cell;
    use std::rc::Rc;

    /// A screen that draws its name on row 0 (row 1 for TOP) and reacts to single letters.
    struct Probe {
        name: &'static str,
        updates: Rc<Cell<u32>>,
    }

    impl Screen for Probe {
        fn update(&mut self, _st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
            self.updates.set(self.updates.get() + 1);
            let Some(k) = keys.first() else {
                return ScreenCmd::Stay;
            };
            match k.code {
                crossterm::event::KeyCode::Char('p') => ScreenCmd::Push {
                    to: Box::new(Probe {
                        name: "TOP",
                        updates: Rc::new(Cell::new(0)),
                    }),
                    fx: TransitionKind::None,
                },
                crossterm::event::KeyCode::Char('o') => ScreenCmd::Pop {
                    fx: TransitionKind::None,
                },
                crossterm::event::KeyCode::Char('f') => ScreenCmd::Flash(TransitionKind::Glitch),
                crossterm::event::KeyCode::Char('s') => ScreenCmd::Switch {
                    to: Box::new(Probe {
                        name: "WORLD",
                        updates: Rc::new(Cell::new(0)),
                    }),
                    fx: TransitionKind::SlideUp,
                },
                crossterm::event::KeyCode::Char('q') => ScreenCmd::Quit,
                _ => ScreenCmd::Stay,
            }
        }
        fn draw(&self, _st: &AppState, f: &mut Frame) {
            let row = if self.name == "TOP" { 1 } else { 0 };
            f.render_widget(
                ratatui::widgets::Paragraph::new(self.name)
                    .style(Style::default().fg(Color::Rgb(200, 200, 200))),
                Rect::new(0, row, 10, 1),
            );
        }
    }

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(
            crossterm::event::KeyCode::Char(c),
            crossterm::event::KeyModifiers::NONE,
        )
    }

    fn fg_at(t: &Terminal<TestBackend>, x: u16, y: u16) -> Color {
        t.backend().buffer()[(x, y)].fg
    }

    #[test]
    fn stack_pushes_dims_updates_top_and_pops() {
        let updates = Rc::new(Cell::new(0));
        let base = Probe {
            name: "BASE",
            updates: updates.clone(),
        };
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut runner = Runner::new(Box::new(base), AppState::new());
        assert!(runner.frame(&mut t, &[]).unwrap());
        assert_eq!(runner.depth(), 1);
        assert_eq!(fg_at(&t, 0, 0), Color::Rgb(200, 200, 200));
        assert!(runner.frame(&mut t, &[key('p')]).unwrap());
        assert_eq!(runner.depth(), 2);
        assert_eq!(updates.get(), 2);
        let text = text(&t);
        assert!(text.contains("BASE") && text.contains("TOP"), "{text}");
        let Color::Rgb(r, _, _) = fg_at(&t, 0, 0) else {
            panic!("base row lost its colour");
        };
        assert!(r < 120, "base is not dimmed: {r}");
        // Only the top screen updates while it is up.
        assert!(runner.frame(&mut t, &[]).unwrap());
        assert_eq!(updates.get(), 2);
        assert!(runner.frame(&mut t, &[key('x')]).unwrap());
        assert_eq!(runner.depth(), 2, "TOP stays on an unknown key");
        assert!(runner.frame(&mut t, &[key('o')]).unwrap());
        assert_eq!(runner.depth(), 1);
        assert_eq!(fg_at(&t, 0, 0), Color::Rgb(200, 200, 200));
        assert_eq!(updates.get(), 2);
        assert!(runner.frame(&mut t, &[]).unwrap());
        assert_eq!(updates.get(), 3);
    }

    #[test]
    fn pop_on_last_screen_is_ignored_and_switch_clears() {
        let base = Probe {
            name: "BASE",
            updates: Rc::new(Cell::new(0)),
        };
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut runner = Runner::new(Box::new(base), AppState::new());
        assert!(runner.frame(&mut t, &[key('o')]).unwrap());
        assert_eq!(runner.depth(), 1);
        assert!(text(&t).contains("BASE"));
        assert!(runner.frame(&mut t, &[key('p')]).unwrap());
        assert!(runner.frame(&mut t, &[key('p')]).unwrap());
        assert_eq!(runner.depth(), 3);
        // Switch from the top replaces the whole stack and snapshots the frame.
        assert!(runner.frame(&mut t, &[key('s')]).unwrap());
        assert_eq!(runner.depth(), 1);
        let tr = runner.transition().expect("a transition is running");
        let snapshot: String = (0..4)
            .map(|x| tr.from[(x, 0)].symbol().to_string())
            .collect();
        assert_eq!(snapshot, "BASE");
        for _ in 0..12 {
            assert!(runner.frame(&mut t, &[]).unwrap());
        }
        assert!(text(&t).contains("WORLD"));
        assert!(!runner.frame(&mut t, &[key('q')]).unwrap());
    }

    #[test]
    fn flash_runs_a_transition_in_place() {
        let base = Probe {
            name: "BASE",
            updates: Rc::new(Cell::new(0)),
        };
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut runner = Runner::new(Box::new(base), AppState::new());
        assert!(runner.frame(&mut t, &[]).unwrap());
        assert!(runner.frame(&mut t, &[key('f')]).unwrap());
        assert_eq!(runner.depth(), 1);
        let tr = runner.transition().expect("flash started a transition");
        assert_eq!(tr.kind, TransitionKind::Glitch);
        assert_eq!(tr.dur, 10);
        // Drawing at another size while the flash runs must not touch the stale snapshot;
        // once the flash is over the screen is still the same one.
        let mut small = Terminal::new(TestBackend::new(100, 31)).unwrap();
        for _ in 0..12 {
            assert!(runner.frame(&mut small, &[]).unwrap());
        }
        assert!(
            runner
                .transition()
                .is_none_or(|t| !t.active(runner.st.tick))
        );
        assert!(text(&small).contains("BASE"));
    }

    #[test]
    fn app_state_logs_with_the_current_tick() {
        let mut st = AppState::new();
        st.tick = 5;
        st.say("hi");
        st.warn("no");
        st.story("level 2");
        let kinds: Vec<(String, Kind, u64)> = st
            .log
            .iter()
            .map(|e| (e.text.clone(), e.kind, e.tick))
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("hi".to_string(), Kind::Info, 5),
                ("no".to_string(), Kind::Warn, 5),
                ("level 2".to_string(), Kind::Story, 5)
            ]
        );
        assert!(st.log.recent(94).is_some());
        assert!(st.log.recent(95).is_none());
    }
}
