//! Application state, the screen trait and the fixed-rate frame loop.

use crate::messages::{Kind, MessageLog};
use crate::render::style::StylePack;
use crate::screens::deck::Tab;
use crate::term::{TerminalGuard, install_panic_hook};
use crate::ui::sidebar::Sidebar;
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
    /// The professions and lines of the city's people.
    pub people: sim::People,
    /// The living city, once there is a city.
    pub world: Option<sim::World>,
    /// The floors walked into this session.
    pub interiors: crate::interiors::Interiors,
    /// The constructs entered this session.
    pub constructs: crate::constructs::Constructs,
    /// The Net's state: traces, hacks, backdoors, cooling, the program in flight.
    pub net: crate::net::NetState,
    /// Items lying in the city.
    pub ground: game::Ground,
    /// Ability effects in flight, oldest first.
    pub fx: Vec<FxInstance>,
    /// Set when the screen should be repainted in full, as after a glitch kick.
    pub repaint: bool,
    /// Where the deck was last: its tab and the selected row per tab.
    pub deck_tab: Tab,
    pub deck_rows: [usize; Tab::ALL.len()],
    /// The character being built, between the chooser and JACK IN.
    pub build: Option<game::Build>,
    /// What happened, newest last.
    pub log: MessageLog,
    /// Which sidebar sections are on.
    pub sidebar: Sidebar,
    /// The player's flat and its rig, when the city has one.
    pub home: Option<crate::home::HomeRig>,
}

/// One ability effect playing on the map.
#[derive(Clone, Debug, PartialEq)]
pub struct FxInstance {
    pub fx: game::Fx,
    pub place: citygen::Place,
    pub origin: citygen::Pos,
    /// The ability's radius: Scan or Ping.
    pub radius: i32,
    pub started: u64,
    /// For a scan, the tiles it revealed; the wave hides them until it passes.
    pub revealed: Vec<citygen::Pos>,
    /// For a trace, the node tiles it found; bolts run to them and marks sit on them.
    pub targets: Vec<citygen::Pos>,
}

impl FxInstance {
    /// The tick from which the instance draws nothing.
    pub fn until(&self) -> u64 {
        match self.fx {
            game::Fx::Kick(ticks) => self.started + ticks,
            game::Fx::Wave { ticks, .. } => {
                self.started + ticks + crate::render::fx::wave_tail_ticks(ticks, self.radius)
            }
            game::Fx::Glow { ticks, .. }
            | game::Fx::Bolt { ticks, .. }
            | game::Fx::Mark { ticks, .. } => self.started + ticks,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        let content = game::Content::load().expect("embedded content pack is valid");
        let character = game::Character::from_template(&content, &game::TemplateId::new("vex"))
            .expect("the vex template builds");
        let people = sim::People::load().expect("the people pack is valid");
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
            people,
            world: None,
            interiors: Default::default(),
            constructs: Default::default(),
            net: Default::default(),
            ground: game::Ground::default(),
            fx: Vec::new(),
            repaint: false,
            deck_tab: Tab::Skills,
            deck_rows: [0; Tab::ALL.len()],
            build: None,
            log: MessageLog::default(),
            sidebar: Sidebar::default(),
            home: None,
        }
    }

    /// The game's tick: the world's own clock, which stops in overview and in
    /// menus; the frame tick before there is a world.
    pub fn clock_tick(&self) -> u64 {
        self.world.as_ref().map_or(self.tick, |w| w.tick())
    }

    /// The time of night as (hour, minute), from the game's tick.
    pub fn clock(&self) -> (u8, u8) {
        crate::ui::hud::clock_at(self.clock_tick())
    }

    /// How far past the base the character sees: their perks, and two more tiles
    /// with low-light between 18:00 and 05:59.
    pub fn sight_bonus(&self) -> i32 {
        let mut bonus = self.character.sight_bonus(&self.content);
        let (hour, _) = self.clock();
        let night = !(6..18).contains(&hour);
        if night
            && self
                .character
                .has_ability(&self.content, &game::AbilityId::new("lowlight"))
        {
            bonus += 2;
        }
        bonus
    }

    /// Turns dev mode on or off; off also drops the dev-only sidebar sections.
    pub fn set_dev(&mut self, on: bool) {
        self.dev = on;
        if !on {
            self.sidebar.leave_dev();
        }
    }

    /// Drops the effects that have finished by `tick`; a finished kick asks for a full
    /// repaint, since its glyphs may have left the terminal out of step.
    pub fn prune_fx(&mut self, tick: u64) {
        let before = self.fx.len();
        let kicks_before = self
            .fx
            .iter()
            .filter(|i| matches!(i.fx, game::Fx::Kick(_)))
            .count();
        self.fx.retain(|i| tick < i.until());
        let kicks_after = self
            .fx
            .iter()
            .filter(|i| matches!(i.fx, game::Fx::Kick(_)))
            .count();
        if before != self.fx.len() && kicks_after < kicks_before {
            self.repaint = true;
        }
    }

    /// Puts a city in place: no body, no effects in flight, the home found.
    pub fn enter_city(&mut self, city: City) {
        self.home = crate::home::HomeRig::find(&city);
        self.world = Some(sim::World::seed(
            &city,
            &self.people,
            &sim::SimParams::default(),
            crate::ui::hud::START_MINUTE_OF_DAY,
            self.tick,
        ));
        self.city = Some(city);
        self.interiors = Default::default();
        self.constructs = Default::default();
        self.net = Default::default();
        self.player = None;
        self.fx.clear();
        self.deck_tab = Tab::Skills;
        self.deck_rows = [0; Tab::ALL.len()];
    }

    /// Logs a plain result.
    pub fn say(&mut self, msg: impl Into<String>) {
        self.log.push(Kind::Info, msg, self.clock_tick());
    }

    /// Logs a refusal.
    pub fn warn(&mut self, msg: impl Into<String>) {
        self.log.push(Kind::Warn, msg, self.clock_tick());
    }

    /// Logs a quest or level line.
    pub fn story(&mut self, msg: impl Into<String>) {
        self.log.push(Kind::Story, msg, self.clock_tick());
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
    /// A screen change or transition asked for a full repaint once nothing is animating.
    repaint_pending: bool,
    full_repaints: u32,
}

impl Runner {
    pub fn new(screen: Box<dyn Screen>, st: AppState) -> Self {
        Runner {
            screens: vec![screen],
            st,
            transition: None,
            last_frame: None,
            repaint_pending: false,
            full_repaints: 0,
        }
    }

    #[cfg(test)]
    pub fn transition(&self) -> Option<&Transition> {
        self.transition.as_ref()
    }

    #[cfg(test)]
    /// How many full repaints the runner has forced; one follows every screen change.
    pub fn full_repaints(&self) -> u32 {
        self.full_repaints
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
                self.repaint_pending = true;
            }
            ScreenCmd::Push { to, fx } => {
                let snapshot = self.snapshot(area);
                self.screens.push(to);
                self.transition = Some(Transition::new(fx, snapshot, self.st.tick));
                self.repaint_pending = true;
            }
            ScreenCmd::Pop { fx } => {
                if self.screens.len() > 1 {
                    let snapshot = self.snapshot(area);
                    self.screens.pop();
                    self.transition = Some(Transition::new(fx, snapshot, self.st.tick));
                    self.repaint_pending = true;
                }
            }
            ScreenCmd::Flash(fx) => {
                let snapshot = self.snapshot(area);
                self.transition = Some(Transition::flash(fx, snapshot, self.st.tick));
                self.repaint_pending = true;
            }
            ScreenCmd::Quit => return Ok(false),
        }
        // Once nothing animates, repaint every cell: a glitch or a wide glyph may have
        // left the terminal out of step with what ratatui believes is on screen.
        if self.st.repaint {
            self.repaint_pending = true;
            self.st.repaint = false;
        }
        let animating = self
            .transition
            .as_ref()
            .is_some_and(|t| t.active(self.st.tick));
        if self.repaint_pending && !animating {
            // Every cell is sent again on the next draw, blanks included, without
            // clearing the screen first (which showed as a blank pause on a big
            // terminal): the buffer the next draw diffs against is filled with a glyph
            // nothing real ever draws, then swapped into the "on screen" slot.
            for cell in term.current_buffer_mut().content.iter_mut() {
                cell.set_symbol("\u{0}");
            }
            term.swap_buffers();
            self.full_repaints += 1;
            self.repaint_pending = false;
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
    fn transition_end_and_stack_changes_force_a_full_repaint() {
        let base = Probe {
            name: "BASE",
            updates: Rc::new(Cell::new(0)),
        };
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut runner = Runner::new(Box::new(base), AppState::new());
        runner.frame(&mut t, &[]).unwrap();
        assert_eq!(runner.full_repaints(), 0);
        // A flash runs a ten-tick glitch: no repaint while it runs, one when it ends.
        runner.frame(&mut t, &[key('f')]).unwrap();
        for _ in 0..9 {
            runner.frame(&mut t, &[]).unwrap();
            assert_eq!(runner.full_repaints(), 0);
        }
        runner.frame(&mut t, &[]).unwrap();
        assert_eq!(runner.full_repaints(), 1);
        runner.frame(&mut t, &[]).unwrap();
        assert_eq!(runner.full_repaints(), 1, "only once per transition");
        // A push with no effect repaints on its next frame.
        runner.frame(&mut t, &[key('p')]).unwrap();
        assert_eq!(runner.full_repaints(), 2);
        runner.frame(&mut t, &[key('o')]).unwrap();
        assert_eq!(runner.full_repaints(), 3);
        // The state can ask for one too, as when a kick ends.
        runner.st.repaint = true;
        runner.frame(&mut t, &[]).unwrap();
        assert_eq!(runner.full_repaints(), 4);
        assert!(!runner.st.repaint);
    }

    #[test]
    fn fx_instances_end_and_prune() {
        let mut st = AppState::new();
        let at = |fx: game::Fx| FxInstance {
            fx,
            place: citygen::Place::Stratum(citygen::StratumKind::Street),
            origin: citygen::Pos::new(0, 0),
            radius: 5,
            started: 100,
            revealed: Vec::new(),
            targets: Vec::new(),
        };
        st.fx.push(at(game::Fx::Kick(4)));
        st.fx.push(at(game::Fx::Wave {
            colour: (0, 0, 0),
            ticks: 10,
        }));
        st.fx.push(at(game::Fx::Glow {
            colour: (0, 0, 0),
            ticks: 30,
        }));
        // The wave's tail allowance is the ticks the front needs to carry its nine-cell
        // ripple band past the reach: ceil(9 * 10 / (5 * sqrt 5)) = 9.
        let ends: Vec<u64> = st.fx.iter().map(|i| i.until()).collect();
        assert_eq!(ends, vec![104, 119, 130]);
        st.prune_fx(103);
        assert_eq!(st.fx.len(), 3);
        assert!(!st.repaint);
        st.prune_fx(104);
        assert_eq!(st.fx.len(), 2);
        assert!(st.repaint, "a finished kick asks for a full repaint");
        st.repaint = false;
        st.prune_fx(130);
        assert!(st.fx.is_empty());
        assert!(!st.repaint, "waves and glows do not");
        let mut wide = at(game::Fx::Wave {
            colour: (0, 0, 0),
            ticks: 18,
        });
        wide.radius = 12;
        assert_eq!(wide.until(), 100 + 18 + 7);
        wide.radius = 0;
        assert_eq!(wide.until(), 100 + 18);
    }

    #[test]
    fn entering_a_city_clears_the_body_and_effects() {
        let mut st = AppState::new();
        let city = citygen::Generator::run(citygen::GenParams {
            seed: 3,
            size: citygen::SizePreset::Custom { w: 80, h: 40 },
            districts: 4,
            ..Default::default()
        })
        .unwrap();
        st.player = Some(crate::walker::Player::spawn(&city, 1));
        st.fx.push(FxInstance {
            fx: game::Fx::Kick(4),
            place: citygen::Place::Stratum(citygen::StratumKind::Street),
            origin: citygen::Pos::new(0, 0),
            radius: 0,
            started: 0,
            revealed: Vec::new(),
            targets: Vec::new(),
        });
        st.deck_tab = Tab::Gear;
        st.deck_rows[Tab::Gear.index()] = 3;
        let seed = city.seed;
        st.enter_city(city);
        assert!(st.player.is_none() && st.fx.is_empty());
        assert_eq!(st.city.as_ref().map(|c| c.seed), Some(seed));
        assert_eq!(
            (st.deck_tab, st.deck_rows),
            (Tab::Skills, [0; Tab::ALL.len()]),
            "a new city forgets where the deck was"
        );
    }

    #[test]
    fn clock_runs_from_ten_pm_in_real_time() {
        let mut st = AppState::new();
        assert_eq!(st.clock(), (22, 0));
        st.tick = 89;
        assert_eq!(st.clock(), (22, 0));
        st.tick = 90;
        assert_eq!(st.clock(), (22, 1));
        st.tick = 90 * 120;
        assert_eq!(st.clock(), (0, 0));
        st.tick = 90 * (24 * 60 + 61);
        assert_eq!(st.clock(), (23, 1));
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

    /// Draws one glyph on its first frame and nothing after, like a glitch that ends.
    struct Flicker;

    impl Screen for Flicker {
        fn update(&mut self, _st: &mut AppState, _keys: &[KeyEvent]) -> ScreenCmd {
            ScreenCmd::Stay
        }

        fn draw(&self, st: &AppState, f: &mut Frame) {
            if st.tick == 1 {
                f.buffer_mut()[(3, 3)].set_char('▓');
            }
        }
    }

    #[test]
    fn a_full_repaint_wipes_a_stale_glyph_without_clearing_the_screen() {
        let mut t = Terminal::new(TestBackend::new(MIN_W, MIN_H)).unwrap();
        let mut r = Runner::new(Box::new(Flicker), AppState::new());
        r.frame(&mut t, &[]).unwrap();
        assert_eq!(t.backend().buffer()[(3, 3)].symbol(), "▓");
        // The screen draws nothing there now, and asks for a full repaint.
        r.st.repaint = true;
        r.frame(&mut t, &[]).unwrap();
        assert_eq!(
            t.backend().buffer()[(3, 3)].symbol(),
            " ",
            "the stale glyph is gone"
        );
        assert_eq!(r.full_repaints(), 1);
    }
}
