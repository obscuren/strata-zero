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
    /// Dev: tint the walkers who can see you.
    pub show_eyes: bool,
    /// Dev: paint each visible tile grey by its light.
    pub show_light: bool,
    /// A popup asked the map for its look cursor on this tile; the map takes it.
    pub look_at: Option<citygen::Pos>,
    /// The console's `target`: a Street tile the free camera should jump to.
    pub jump_to: Option<citygen::Pos>,
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
    /// Your backdoors, the route of a rig run, the trace home and the raid.
    pub backdoors: crate::backdoors::Backdoors,
    /// Items lying in the city.
    pub ground: game::Ground,
    /// Ability effects in flight, oldest first.
    pub fx: Vec<FxInstance>,
    /// Who stands against you on foot this session; cleared with the city.
    pub foes: Vec<crate::foes::Foe>,
    /// Blasts lying live with a fuse; cleared with the city.
    pub live: Vec<crate::combat::Live>,
    /// Rounds fired this session: the roll's seed.
    pub shots: u64,
    /// Every round lands, or none; for tests.
    #[cfg(test)]
    pub force_hit: Option<bool>,
    /// Set when the screen should be repainted in full, as after a glitch kick.
    pub repaint: bool,
    /// Where the deck was last: its tab and the selected row per tab.
    pub deck_tab: Tab,
    pub deck_rows: [usize; Tab::ALL.len()],
    /// Where GEAR & INVENTORY was left: the focus, the tile, the pack's row, the filter.
    pub deck_kit: crate::screens::kit::KitState,
    /// The part the jacked deck's blueprint points at, kept between openings.
    pub deck_part: usize,
    /// The character being built, between the chooser and JACK IN.
    pub build: Option<game::Build>,
    /// What happened, newest last.
    pub log: MessageLog,
    /// Which sidebar sections are on.
    pub sidebar: Sidebar,
    /// The player's flat and its rig, when the city has one.
    pub home: Option<crate::home::HomeRig>,
    /// What the player knows: door codes, and the lost cards already laid out.
    pub journal: crate::credentials::Journal,
    pub helix: crate::helix::Helix,
    /// The fixers' jobs and what each fixer thinks of you.
    pub jobs: crate::jobs::Jobs,
    /// The fixers' voice.
    pub flavor: crate::jobs::Flavor,
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
    /// For a shot, the impact tile.
    pub targets: Vec<citygen::Pos>,
    /// What a number reads (`-4`); empty for every other shape.
    pub label: String,
    /// For a shot's shapes: whether the round landed.
    pub hit: bool,
    /// A colour the net lends a number (its bolt's); `None` is the street's own.
    pub tint: Option<(u8, u8, u8)>,
    /// For a thrown thing: how it crosses the line.
    pub arc: Option<game::Arc>,
}

impl FxInstance {
    /// How long a blast's stain outlasts it: only where it hurt.
    fn scorch(&self) -> u64 {
        if self.hit {
            crate::combat::SCORCH_TICKS
        } else {
            0
        }
    }

    /// The tick from which the instance draws nothing.
    pub fn until(&self) -> u64 {
        use crate::render::fx::{
            BLOOM_TICKS, CASING_TICKS, FLASH_TICKS, IMPACT_TICKS, NUMBER_TICKS, SHOCK_TICKS,
            STREAK_TICKS, WHITEOUT_TICKS, tracer_ticks,
        };
        match self.fx {
            game::Fx::Kick(ticks) => self.started + ticks,
            game::Fx::Wave { ticks, .. } => {
                self.started + ticks + crate::render::fx::wave_tail_ticks(ticks, self.radius)
            }
            game::Fx::Glow { ticks, .. }
            | game::Fx::Bolt { ticks, .. }
            | game::Fx::Mark { ticks, .. } => self.started + ticks,
            game::Fx::Tracer { speed, .. } => {
                self.started
                    + self
                        .targets
                        .first()
                        .map_or(1, |t| tracer_ticks(self.origin, *t, speed))
            }
            game::Fx::Streak { .. } => self.started + STREAK_TICKS,
            game::Fx::Flash { .. } => self.started + FLASH_TICKS,
            game::Fx::Chips | game::Fx::Spark | game::Fx::Tear => self.started + IMPACT_TICKS,
            game::Fx::Number { .. } => self.started + NUMBER_TICKS,
            game::Fx::Casing => self.started + CASING_TICKS,
            game::Fx::Throw => {
                self.started
                    + self
                        .targets
                        .first()
                        .map_or(0, |t| crate::combat::throw_ticks(self.origin, *t))
            }
            game::Fx::Bloom => self.started + BLOOM_TICKS + self.scorch(),
            game::Fx::Whiteout => self.started + WHITEOUT_TICKS + self.scorch(),
            game::Fx::Shock => self.started + SHOCK_TICKS,
            // Drawn by a later task.
            game::Fx::Trail | game::Fx::Pulse { .. } => self.started,
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
            show_eyes: false,
            show_light: false,
            look_at: None,
            jump_to: None,
            content,
            character,
            people,
            world: None,
            interiors: Default::default(),
            constructs: Default::default(),
            net: Default::default(),
            backdoors: Default::default(),
            ground: game::Ground::default(),
            fx: Vec::new(),
            foes: Vec::new(),
            live: Vec::new(),
            shots: 0,
            #[cfg(test)]
            force_hit: None,
            repaint: false,
            deck_tab: Tab::Skills,
            deck_rows: [0; Tab::ALL.len()],
            deck_kit: Default::default(),
            deck_part: 0,
            build: None,
            log: MessageLog::default(),
            sidebar: Sidebar::default(),
            home: None,
            journal: Default::default(),
            helix: Default::default(),
            jobs: Default::default(),
            flavor: crate::jobs::Flavor::load().expect("the jobs pack is valid"),
        }
    }

    /// The game's tick: the world's own clock, which stops in overview and in
    /// menus; the frame tick before there is a world.
    pub fn clock_tick(&self) -> u64 {
        self.world.as_ref().map_or(self.tick, |w| w.tick())
    }

    /// The time of night as (hour, minute), from the game's tick.
    pub fn clock(&self) -> (u8, u8) {
        let m = self.minute_of_day();
        ((m / 60) as u8, (m % 60) as u8)
    }

    /// The minute of the day: the living city's clock, or the tick's before there is one.
    pub fn minute_of_day(&self) -> u16 {
        self.world.as_ref().map_or_else(
            || crate::ui::hud::minute_at(self.tick),
            |w| w.minute_of_day(),
        )
    }

    /// How far past the base the character sees: their perks, and two more tiles
    /// with low-light while the lamps are on, 18:00 to 05:59.
    pub fn sight_bonus(&self) -> i32 {
        let mut bonus = self.character.sight_bonus(&self.content);
        let night = citygen::lighting::lamps_on(self.minute_of_day());
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

    /// Puts a city in place: no body, no effects in flight, the home found. The
    /// clock starts at the city's usual hour.
    pub fn enter_city(&mut self, city: City) {
        // The city's clock has run since the session started, as it always has.
        let day = 24 * 60;
        let spent = (self.tick / sim::TICKS_PER_MINUTE) % day;
        let minute = (crate::ui::hud::START_MINUTE_OF_DAY as u64 + spent) % day;
        self.enter_city_at(city, minute as u16);
    }

    /// Puts a city in place with its clock reading `minute` of the day on entry,
    /// whatever ticks the session has already spent.
    pub fn enter_city_at(&mut self, city: City, minute: u16) {
        self.home = crate::home::HomeRig::find(&city);
        let day = 24 * 60;
        let spent = (self.tick / sim::TICKS_PER_MINUTE) % day;
        let start = (minute as u64 + day - spent) % day;
        self.world = Some(sim::World::seed(
            &city,
            &self.people,
            &sim::SimParams::default(),
            start as u16,
            self.tick,
        ));
        self.city = Some(city);
        self.interiors = Default::default();
        self.constructs = Default::default();
        self.net = Default::default();
        self.backdoors = Default::default();
        self.journal = Default::default();
        self.helix = Default::default();
        self.jobs = Default::default();
        crate::credentials::purge(&mut self.content, &mut self.character);
        self.ground = Default::default();
        self.player = None;
        self.fx.clear();
        self.foes.clear();
        self.live.clear();
        self.deck_tab = Tab::Skills;
        self.deck_rows = [0; Tab::ALL.len()];
        self.deck_kit = Default::default();
        self.deck_part = 0;
    }

    /// Logs a plain result.
    pub fn say(&mut self, msg: impl Into<String>) {
        self.log.push(Kind::Info, msg, self.clock_tick());
    }

    /// Logs a refusal.
    pub fn warn(&mut self, msg: impl Into<String>) {
        self.log.push(Kind::Warn, msg, self.clock_tick());
    }

    /// Logs a job or level line.
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
    /// Swap the top screen for another in one step; with one screen it only pushes.
    Replace {
        to: Box<dyn Screen>,
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
            ScreenCmd::Replace { to, fx } => {
                let snapshot = self.snapshot(area);
                if self.screens.len() > 1 {
                    self.screens.pop();
                }
                self.screens.push(to);
                self.transition = Some(Transition::new(fx, snapshot, self.st.tick));
                self.repaint_pending = true;
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
                crossterm::event::KeyCode::Char('r') => ScreenCmd::Replace {
                    to: Box::new(Probe {
                        name: "SWAP",
                        updates: Rc::new(Cell::new(0)),
                    }),
                    fx: TransitionKind::None,
                },
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
    fn replace_swaps_the_top_and_never_pops_the_last_screen() {
        let base = Probe {
            name: "BASE",
            updates: Rc::new(Cell::new(0)),
        };
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut runner = Runner::new(Box::new(base), AppState::new());
        // With one screen, a replace only pushes: the base stays underneath.
        assert!(runner.frame(&mut t, &[key('r')]).unwrap());
        assert_eq!(runner.depth(), 2);
        assert!(text(&t).contains("SWAP"));
        // With two, it swaps the top: the height stays two.
        assert!(runner.frame(&mut t, &[key('r')]).unwrap());
        assert_eq!(runner.depth(), 2);
        assert!(runner.frame(&mut t, &[key('o')]).unwrap());
        assert_eq!(runner.depth(), 1);
        assert!(text(&t).contains("BASE"));
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
    fn until_per_shot_shape() {
        use citygen::Pos;
        let at = |fx: game::Fx, targets: Vec<Pos>| FxInstance {
            fx,
            place: citygen::Place::Stratum(citygen::StratumKind::Street),
            origin: Pos::new(6, 8),
            radius: 0,
            started: 100,
            revealed: Vec::new(),
            targets,
            label: String::new(),
            hit: false,
            tint: None,
            arc: None,
        };
        let to = vec![Pos::new(18, 8)];
        assert_eq!(at(game::Fx::Flash { big: true }, to.clone()).until(), 102);
        assert_eq!(
            at(game::Fx::Streak { colour: (1, 1, 1) }, to.clone()).until(),
            103
        );
        assert_eq!(at(game::Fx::Chips, to.clone()).until(), 103);
        assert_eq!(at(game::Fx::Spark, to.clone()).until(), 103);
        assert_eq!(at(game::Fx::Tear, to.clone()).until(), 103);
        assert_eq!(at(game::Fx::Number { tag: true }, to.clone()).until(), 114);
        let tracer = game::Fx::Tracer {
            colour: (1, 1, 1),
            speed: 3,
        };
        assert_eq!(at(tracer.clone(), to).until(), 104);
        assert_eq!(at(tracer, Vec::new()).until(), 101);
    }

    #[test]
    fn until_for_the_blast_shapes_and_the_throw() {
        use citygen::Pos;
        let at = |fx: game::Fx, hit: bool| FxInstance {
            fx,
            place: citygen::Place::Stratum(citygen::StratumKind::Street),
            origin: Pos::new(6, 8),
            radius: 2,
            started: 100,
            revealed: Vec::new(),
            targets: vec![Pos::new(12, 8)],
            label: String::new(),
            hit,
            tint: None,
            arc: None,
        };
        let scorch = crate::combat::SCORCH_TICKS;
        assert_eq!(at(game::Fx::Throw, false).until(), 105, "six tiles: 3 + 2");
        assert_eq!(at(game::Fx::Bloom, false).until(), 113);
        assert_eq!(at(game::Fx::Bloom, true).until(), 113 + scorch);
        assert_eq!(at(game::Fx::Whiteout, true).until(), 108 + scorch);
        assert_eq!(at(game::Fx::Whiteout, false).until(), 108);
        assert_eq!(at(game::Fx::Shock, true).until(), 104, "no scorch");
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
            label: String::new(),
            hit: false,
            tint: None,
            arc: None,
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
            label: String::new(),
            hit: false,
            tint: None,
            arc: None,
        });
        st.deck_tab = Tab::Kit;
        st.deck_rows[Tab::Stats.index()] = 3;
        st.deck_part = 300;
        st.deck_kit = crate::screens::kit::KitState {
            pane: crate::screens::kit::Pane::Pack,
            tile: 4,
            row: 2,
            filter: Some(game::ItemKind::Program),
        };
        let seed = city.seed;
        st.enter_city(city);
        assert!(st.player.is_none() && st.fx.is_empty());
        assert_eq!(st.city.as_ref().map(|c| c.seed), Some(seed));
        assert_eq!(
            (st.deck_tab, st.deck_rows, st.deck_part),
            (Tab::Skills, [0; Tab::ALL.len()], 0),
            "a new city forgets where the deck was"
        );
        assert_eq!(st.deck_kit, crate::screens::kit::KitState::default());
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
