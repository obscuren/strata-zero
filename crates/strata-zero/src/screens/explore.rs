//! The map screen: free-camera Explore mode and the Walk mode with the `@`.

use crate::actions::{perform, report};
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::messages::Kind;
use crate::render::fx::FxOverlay;
use crate::render::items::GroundOverlay;
use crate::render::overlay::Digital;
use crate::render::people::{PeopleOverlay, card_lines};
use crate::render::viewport::{Camera, MapView, Overlay, tile_to_screen};
use crate::render::weather::Weather;
use crate::screens::configure::Configure;
use crate::screens::console;
use crate::screens::deck::Deck;
use crate::screens::files::Files;
use crate::screens::generating::Generating;
use crate::screens::help;
use crate::screens::messages::Messages;
use crate::screens::pause::Pause;
use crate::screens::popup::{Choice, Pick, Popup, WorldAct};
use crate::screens::title::Title;
use crate::ui::chrome::{CYAN, MAGENTA, TEXT, put};
use crate::ui::console::{Console, ConsoleEvent};
use crate::ui::hud;
use crate::ui::panel::inspector_lines;
use crate::ui::sidebar::{self, Section};
use crate::walker::{Player, port_name};
use citygen::rng::hash64;
use citygen::{Dir, Pos, Rect as TileRect, StratumKind};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Paragraph, Widget};
use std::cell::Cell;
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Explore,
    Walk,
}

/// Frames per second over the last second.
/// Hotkey letters for popup rows built here: h j k l q close or navigate, t is take all.
const WORLD_LETTERS: &str = "abcdefgimnoprsuvwxyz";

pub struct FpsMeter {
    /// Start of the current measuring window.
    last: Instant,
    frames: u32,
    pub fps: f32,
}

impl Default for FpsMeter {
    fn default() -> Self {
        FpsMeter::new(Instant::now())
    }
}

impl FpsMeter {
    pub fn new(now: Instant) -> Self {
        FpsMeter {
            last: now,
            frames: 0,
            fps: 0.0,
        }
    }

    pub fn frame(&mut self) {
        self.frame_at(Instant::now());
    }

    /// Restarts the window after frames stopped arriving (a menu was on top); the
    /// last reading stands until a fresh second has been measured.
    pub fn resume(&mut self, now: Instant) {
        self.last = now;
        self.frames = 0;
    }

    pub fn frame_at(&mut self, now: Instant) {
        self.frames += 1;
        let elapsed = now.saturating_duration_since(self.last).as_secs_f32();
        if elapsed >= 1.0 {
            self.fps = self.frames as f32 / elapsed;
            self.frames = 0;
            self.last = now;
        }
    }
}

pub struct MapScreen {
    pub mode: Mode,
    pub stratum: StratumKind,
    pub cam: Camera,
    /// The tile under inspection in Explore mode; the camera follows it.
    pub cursor: Pos,
    pub digital: bool,
    pub help: bool,
    fps: FpsMeter,
    /// The dev console while it is open; it takes every key.
    console: Option<Console>,
    /// Tick of the last update; a gap means a menu was on top.
    last_tick: u64,
    cam_ready: bool,
    /// The map area of the last frame, so `update` knows the view size.
    last_map: Cell<Rect>,
}

fn dir_for(code: KeyCode) -> Option<Dir> {
    Some(match code {
        KeyCode::Up | KeyCode::Char('k') => Dir::N,
        KeyCode::Down | KeyCode::Char('j') => Dir::S,
        KeyCode::Left | KeyCode::Char('h') => Dir::W,
        KeyCode::Right | KeyCode::Char('l') => Dir::E,
        KeyCode::Char('y') => Dir::NW,
        KeyCode::Char('u') => Dir::NE,
        KeyCode::Char('b') => Dir::SW,
        KeyCode::Char('n') => Dir::SE,
        _ => return None,
    })
}

/// The grid: a four-row band, a heavy rule, the map beside the right column, a heavy
/// rule, then the two footer rows. One light vertical rule separates the columns.
struct Regions {
    band_l: [Rect; 4],
    band_r: [Rect; 4],
    top_rule: Rect,
    map: Rect,
    side: Rect,
    vrule_x: u16,
    bottom_rule: Rect,
    status: Rect,
    abilities: Rect,
}

fn layout(area: Rect) -> Regions {
    let r = sidebar::width(area.width);
    let l = area.width.saturating_sub(r + 1);
    let x_rule = area.x + l;
    let row = |y: u16, x: u16, w: u16| Rect::new(x, area.y + y, w, 1);
    let main_y = area.y + 5;
    let main_h = area.height.saturating_sub(8);
    Regions {
        band_l: [
            row(0, area.x, l),
            row(1, area.x, l),
            row(2, area.x, l),
            row(3, area.x, l),
        ],
        band_r: [
            row(0, x_rule + 1, r),
            row(1, x_rule + 1, r),
            row(2, x_rule + 1, r),
            row(3, x_rule + 1, r),
        ],
        top_rule: row(4, area.x, area.width),
        map: Rect::new(area.x, main_y, l, main_h),
        side: Rect::new(x_rule + 1, main_y, r, main_h),
        vrule_x: x_rule,
        bottom_rule: row(area.height.saturating_sub(3), area.x, area.width),
        status: row(area.height.saturating_sub(2), area.x, area.width),
        abilities: row(area.height.saturating_sub(1), area.x, area.width),
    }
}

impl MapScreen {
    /// The map screen on foot from its first frame, dev mode or not: the walker spawns
    /// here, not on the first update, so no camera-mode frame slips out under the
    /// transition.
    pub fn walking(st: &mut AppState) -> Self {
        let mut s = Self::explore(st.tick);
        s.enter_walk(st);
        s.cam_ready = s.mode == Mode::Walk;
        s
    }

    pub fn explore(_tick: u64) -> Self {
        MapScreen {
            mode: Mode::Explore,
            stratum: StratumKind::Street,
            cam: Camera::default(),
            cursor: Pos::new(0, 0),
            digital: false,
            help: false,
            fps: FpsMeter::default(),
            console: None,
            last_tick: 0,
            cam_ready: false,
            last_map: Cell::new(Rect::new(0, 0, 100, 28)),
        }
    }

    /// The tile the inspector describes: the player in Walk mode, the cursor otherwise.
    fn focus_pos(&self, st: &AppState) -> Pos {
        match (self.mode, &st.player) {
            (Mode::Walk, Some(p)) => p.pos,
            _ => self.cursor,
        }
    }

    /// Active abilities in slot order, for the action bar and the number keys.
    fn actives(st: &AppState) -> Vec<game::AbilityId> {
        st.character
            .abilities(&st.content)
            .into_iter()
            .map(|(id, _)| id)
            .filter(|id| {
                st.content
                    .ability(id)
                    .is_some_and(|a| matches!(a.kind, game::AbilityKind::Active(_)))
            })
            .take(hud::SLOTS)
            .collect()
    }

    fn slots(st: &AppState) -> Vec<hud::Slot> {
        Self::actives(st)
            .into_iter()
            .filter_map(|id| {
                let a = st.content.ability(&id)?;
                let state = match st.character.cooling(&st.content, &id, st.tick) {
                    Some(left) => hud::SlotState::Cooling(left.div_ceil(30)),
                    None => hud::SlotState::Ready,
                };
                let flash = st
                    .character
                    .fired
                    .get(&id)
                    .is_some_and(|f| st.tick.saturating_sub(*f) < hud::FLASH_TICKS);
                Some(hud::Slot {
                    name: a.name.clone(),
                    state,
                    flash,
                })
            })
            .collect()
    }

    /// The VIEW popup: one row per sidebar section, its letter toggles it.
    fn view_popup(&self, st: &AppState) -> ScreenCmd {
        let choices = Section::ALL
            .iter()
            .enumerate()
            .filter(|(_, s)| st.dev || !s.dev_only())
            .map(|(i, s)| {
                let mark = if st.sidebar.is_on(*s) { "▣" } else { "▢" };
                Choice::new(
                    (b'a' + i as u8) as char,
                    format!("{mark} {}  F{}", s.label(), i + 1),
                    Pick::View(*s),
                )
            })
            .collect();
        ScreenCmd::Push {
            to: Box::new(Popup::verbs("VIEW", choices, st.tick)),
            fx: TransitionKind::None,
        }
    }

    fn change_stratum(&mut self, st: &AppState, delta: i32) {
        let max = st
            .city
            .as_ref()
            .map(|c| c.strata.len() as i32 - 1)
            .unwrap_or(3);
        let i = (self.stratum.index() as i32 + delta).clamp(0, max);
        if let Some(k) = StratumKind::from_index(i as usize) {
            self.stratum = k;
        }
    }

    fn enter_walk(&mut self, st: &mut AppState) {
        let bonus = st.character.sight_bonus(&st.content);
        let Some(city) = st.city.as_ref() else { return };
        if st.player.is_none() {
            let mut p = Player::spawn(city, hash64(city.seed, st.tick));
            p.update_fov(city, bonus);
            st.player = Some(p);
        }
        if let Some(p) = &st.player {
            self.stratum = p.stratum;
            self.cam.snap(p.pos);
        }
        self.mode = Mode::Walk;
    }

    fn handle_common(&mut self, st: &mut AppState, k: &KeyEvent) -> Option<ScreenCmd> {
        match k.code {
            KeyCode::Char('`') => {
                st.set_dev(!st.dev);
                st.say(if st.dev {
                    "dev mode on"
                } else {
                    "dev mode off"
                });
            }
            KeyCode::Char(':') if st.dev => self.console = Some(Console::default()),
            KeyCode::Char('v') => return Some(self.view_popup(st)),
            KeyCode::F(n @ 1..=6) => {
                let section = Section::ALL[n as usize - 1];
                if st.dev || !section.dev_only() {
                    st.sidebar.toggle(section);
                }
            }
            KeyCode::Char('m') => {
                return Some(ScreenCmd::Push {
                    to: Box::new(Messages::new(st.tick)),
                    fx: TransitionKind::None,
                });
            }
            KeyCode::Char('d') => self.digital = !self.digital,
            KeyCode::Char('?') => self.help = !self.help,
            KeyCode::Tab => {
                return Some(ScreenCmd::Push {
                    to: Box::new(
                        Deck::new(st.tick)
                            .focus(self.stratum, self.focus_pos(st))
                            .restore(st),
                    ),
                    fx: TransitionKind::Glitch,
                });
            }
            KeyCode::Char('s') => {
                let name = st.city.as_ref().map(|c| c.name.clone())?;
                return Some(ScreenCmd::Push {
                    to: Box::new(Files::save(st.tick, &name)),
                    fx: TransitionKind::None,
                });
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.help {
                    self.help = false;
                } else {
                    return Some(ScreenCmd::Push {
                        to: Box::new(Pause::new(st.tick)),
                        fx: TransitionKind::None,
                    });
                }
            }
            _ => return None,
        }
        Some(ScreenCmd::Stay)
    }

    fn handle_explore(&mut self, st: &mut AppState, k: &KeyEvent) -> Option<ScreenCmd> {
        let fast = k.modifiers.contains(KeyModifiers::SHIFT)
            || matches!(k.code, KeyCode::Char('H' | 'J' | 'K' | 'L'));
        let step = if fast { 8 } else { 1 };
        let code = match k.code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            other => other,
        };
        match code {
            _ if dir_for(code).is_some()
                && !matches!(code, KeyCode::Char('y' | 'u' | 'b' | 'n')) =>
            {
                let (dx, dy) = dir_for(code).expect("checked").delta();
                let size = st.city.as_ref().map(|c| c.size)?;
                self.cursor = Pos::new(
                    (self.cursor.x + dx * step).clamp(0, size.w as i32 - 1),
                    (self.cursor.y + dy * step).clamp(0, size.h as i32 - 1),
                );
            }
            KeyCode::Char('[') => self.change_stratum(st, -1),
            KeyCode::Char(']') => self.change_stratum(st, 1),
            KeyCode::Char('w') => self.enter_walk(st),
            KeyCode::Char('r') => {
                let params = st.params.clone().with_seed(hash64(st.params.seed, st.tick));
                return Some(match Generating::new(params, st.tick) {
                    Ok(next) => ScreenCmd::Switch {
                        to: Box::new(next),
                        fx: TransitionKind::Glitch,
                    },
                    Err(e) => {
                        st.warn(e.to_string());
                        ScreenCmd::Stay
                    }
                });
            }
            KeyCode::Char('g') => {
                return Some(ScreenCmd::Switch {
                    to: Box::new(Configure::new(st.params.clone(), st.tick)),
                    fx: TransitionKind::SlideUp,
                });
            }
            _ => return None,
        }
        Some(ScreenCmd::Stay)
    }

    /// Feeds keys to the open console; a run line goes through the commands.
    fn feed_console(&mut self, st: &mut AppState, focus: game::At, keys: &[KeyEvent]) {
        let Some(con) = self.console.as_mut() else {
            return;
        };
        let mut close = false;
        for k in keys {
            match con.key(*k, &|head, word| console::complete(st, head, word)) {
                ConsoleEvent::Run(line) => {
                    let result = console::run(st, focus, &line);
                    report(st, result);
                }
                ConsoleEvent::Close => close = true,
                ConsoleEvent::None => {}
            }
        }
        if close {
            self.console = None;
        }
    }

    /// What Space offers where the player stands: take rows, jack, stairs, look.
    fn interact(&self, st: &AppState) -> Option<ScreenCmd> {
        let (city, player) = (st.city.as_ref()?, st.player.as_ref()?);
        let (k, pos) = (player.stratum, player.pos);
        let mut choices = Vec::new();
        // Letters for take rows; j k q h l t and the arrows are taken by the popup and verbs.
        let mut letters = WORLD_LETTERS.chars();
        let stacks = st.ground.at(k, pos);
        for s in stacks {
            let Some(item) = st.content.item(&s.item) else {
                continue;
            };
            let Some(key) = letters.next() else { break };
            choices.push(Choice::new(
                key,
                format!("take {}× {}", s.qty, item.name),
                Pick::Act(game::Action::Take {
                    item: s.item.clone(),
                    qty: s.qty,
                }),
            ));
        }
        if stacks.len() > 1 {
            choices.push(Choice::new(
                't',
                "take all",
                Pick::Act(game::Action::TakeAll),
            ));
        }
        if player.jacked_in() {
            choices.push(Choice::new('j', "jack out", Pick::World(WorldAct::JackOut)));
        } else if player.terminal_in_reach(city).is_some() {
            choices.push(Choice::new('j', "jack in", Pick::World(WorldAct::JackIn)));
        }
        let (mut up, mut down) = (None, None);
        for c in city
            .connectors_at(pos)
            .filter(|c| c.kind != citygen::ConnectorKind::Jack)
        {
            // Only directions that lead somewhere walkable: a row never refuses after being offered.
            if let Some((below, above)) = c.serves_from(k) {
                up = up.or(above.filter(|d| city.walkable(*d, pos)));
                down = down.or(below.filter(|d| city.walkable(*d, pos)));
            }
        }
        if let Some(dest) = up {
            choices.push(Choice::new(
                '>',
                format!("go up to {}", dest.label()),
                Pick::World(WorldAct::Up),
            ));
        }
        if let Some(dest) = down {
            choices.push(Choice::new(
                '<',
                format!("go down to {}", dest.label()),
                Pick::World(WorldAct::Down),
            ));
        }
        if let Some(world) = st.world.as_ref() {
            for n in pos.neighbors4() {
                let Some(id) = world.person_at(n, st.tick) else {
                    continue;
                };
                let Some(key) = letters.next() else { break };
                let card = world.card(id, city);
                choices.push(Choice::new(
                    key,
                    format!("talk to {}", card.name),
                    Pick::Card(card_lines(&card)),
                ));
            }
        }
        let node = Digital::node_under(city, k, pos).map(|id| city.net.node(id));
        let person = st
            .world
            .as_ref()
            .and_then(|w| w.person_at(pos, st.tick).map(|id| w.card(id, city)));
        choices.push(Choice::new(
            'l',
            "look",
            Pick::Card(inspector_lines(
                city,
                k,
                pos,
                node,
                stacks,
                &st.content,
                person.as_ref(),
            )),
        ));
        Some(ScreenCmd::Push {
            to: Box::new(Popup::verbs("HERE", choices, st.tick)),
            fx: TransitionKind::None,
        })
    }

    /// The actives the character can fire, cooling ones greyed; none says so.
    fn abilities(&self, st: &mut AppState) -> ScreenCmd {
        let mut choices = Vec::new();
        let mut letters = WORLD_LETTERS.chars();
        for (id, _) in st.character.abilities(&st.content) {
            let Some(a) = st.content.ability(&id) else {
                continue;
            };
            if !matches!(a.kind, game::AbilityKind::Active(_)) {
                continue;
            }
            let Some(key) = letters.next() else { break };
            choices.push(match st.character.cooling(&st.content, &id, st.tick) {
                Some(left) => {
                    Choice::disabled(key, format!("{} · cooling {}s", a.name, left.div_ceil(30)))
                }
                None => Choice::new(
                    key,
                    format!("{} · ready", a.name),
                    Pick::Act(game::Action::Fire(id.clone())),
                ),
            });
        }
        if choices.is_empty() {
            st.say("no active abilities");
            return ScreenCmd::Stay;
        }
        ScreenCmd::Push {
            to: Box::new(Popup::verbs("ABILITIES", choices, st.tick)),
            fx: TransitionKind::None,
        }
    }

    /// A step into someone is a word with them; a step into a car goes nowhere.
    fn bump(&self, st: &mut AppState, dir: Dir) -> Option<ScreenCmd> {
        let target = st.player.as_ref()?.pos.step(dir);
        let tick = st.tick;
        let (city, world) = (st.city.as_ref()?, st.world.as_ref()?);
        if let Some(id) = world.person_at(target, tick) {
            let card = world.card(id, city);
            return Some(Self::talk(&card, tick));
        }
        if world.car_at(target, tick).is_some() {
            st.warn("a car is in the way");
            return Some(ScreenCmd::Stay);
        }
        None
    }

    fn talk(card: &sim::Card, tick: u64) -> ScreenCmd {
        let choices = vec![Choice::new('t', "talk", Pick::Card(card_lines(card)))];
        ScreenCmd::Push {
            to: Box::new(Popup::verbs(&card.name.to_uppercase(), choices, tick)),
            fx: TransitionKind::None,
        }
    }

    fn handle_walk(&mut self, st: &mut AppState, k: &KeyEvent) -> Option<ScreenCmd> {
        if k.code == KeyCode::Char(' ') {
            return self.interact(st);
        }
        if k.code == KeyCode::Char('a') {
            return Some(self.abilities(st));
        }
        if let KeyCode::Char(c) = k.code
            && let Some(n) = c.to_digit(10)
            && (1..=hud::SLOTS as u32).contains(&n)
        {
            match Self::actives(st).get(n as usize - 1).cloned() {
                Some(id) => {
                    let result = perform(st, game::Action::Fire(id));
                    report(st, result);
                }
                None => st.warn(format!("nothing in slot {n}")),
            }
            return Some(ScreenCmd::Stay);
        }
        if let Some(dir) = dir_for(k.code)
            && let Some(cmd) = self.bump(st, dir)
        {
            return Some(cmd);
        }
        let dev = st.dev;
        let bonus = st.character.sight_bonus(&st.content);
        let city = st.city.as_ref()?;
        let player = st.player.as_mut()?;
        match k.code {
            KeyCode::Char('c') if dev => {
                self.cursor = player.pos;
                self.mode = Mode::Explore;
            }
            KeyCode::Enter => {
                if player.jacked_in() {
                    match player.jack_out(city, bonus) {
                        Ok(()) => {
                            self.stratum = player.stratum;
                            self.cam.snap(player.pos);
                            st.say("jacked out");
                            return Some(ScreenCmd::Flash(TransitionKind::Glitch));
                        }
                        Err(e) => st.warn(e),
                    }
                } else {
                    match player.jack_in(city, bonus) {
                        Ok(port) => {
                            self.stratum = StratumKind::Net;
                            self.cam.snap(port);
                            let where_ = port_name(city, port);
                            st.say(format!("jacked in at {where_}"));
                            return Some(ScreenCmd::Flash(TransitionKind::Glitch));
                        }
                        Err(e) => st.warn(e),
                    }
                }
            }
            KeyCode::Char('<') | KeyCode::Char('>') => {
                let up = k.code == KeyCode::Char('>');
                match player.use_connector(city, up) {
                    Some(dest) => {
                        self.stratum = dest;
                        player.update_fov(city, bonus);
                        st.say(format!("now on {}", dest.label()));
                    }
                    None => st.warn(if up {
                        "nothing leads up from here"
                    } else {
                        "nothing leads down from here"
                    }),
                }
            }
            KeyCode::Char('.') => player.update_fov(city, bonus),
            code => {
                let dir = dir_for(code)?;
                if player.try_move(city, dir) {
                    player.update_fov(city, bonus);
                }
            }
        }
        Some(ScreenCmd::Stay)
    }
}

impl Screen for MapScreen {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        if st.tick > self.last_tick + 1 {
            self.fps.resume(Instant::now());
        }
        self.last_tick = st.tick;
        self.fps.frame();
        st.prune_fx(st.tick);
        // The map follows the player: a popup may have jacked them or taken the stairs.
        if self.mode == Mode::Walk
            && let Some(p) = st.player.as_ref()
            && p.stratum != self.stratum
        {
            self.stratum = p.stratum;
            self.cam.snap(p.pos);
        }
        // The console takes every key while it is open.
        let focus = game::At {
            stratum: self.stratum,
            pos: match (self.mode, &st.player) {
                (Mode::Walk, Some(p)) => p.pos,
                _ => self.cursor,
            },
        };
        if self.console.is_some() {
            self.feed_console(st, focus, keys);
            return ScreenCmd::Stay;
        }
        let Some(size) = st.city.as_ref().map(|c| c.size) else {
            if keys
                .iter()
                .any(|k| matches!(k.code, KeyCode::Char('q') | KeyCode::Esc))
            {
                return ScreenCmd::Switch {
                    to: Box::new(Title::new(st.tick)),
                    fx: TransitionKind::SlideUp,
                };
            }
            return ScreenCmd::Stay;
        };
        if !self.cam_ready {
            self.cam.snap(size.center());
            self.cursor = size.center();
            self.cam_ready = true;
        }
        for (i, k) in keys.iter().enumerate() {
            let cmd = match self.handle_common(st, k) {
                Some(cmd) => Some(cmd),
                None => match self.mode {
                    Mode::Explore => self.handle_explore(st, k),
                    Mode::Walk => self.handle_walk(st, k),
                },
            };
            if let Some(cmd) = cmd
                && !matches!(cmd, ScreenCmd::Stay)
            {
                return cmd;
            }
            // `:` just opened the console: the rest of this frame's keys are typing.
            if self.console.is_some() {
                self.feed_console(st, focus, &keys[i + 1..]);
                return ScreenCmd::Stay;
            }
        }
        if !st.dev && self.mode == Mode::Explore {
            // Without dev mode there is no free camera: the player is always on foot.
            self.enter_walk(st);
        }
        let map = self.last_map.get();
        match (self.mode, &st.player) {
            (Mode::Walk, Some(p)) => self.cam.follow(p.pos, map.width, map.height),
            _ => self.cam.follow(self.cursor, map.width, map.height),
        }
        self.cam.clamp(size, map.width, map.height);
        self.cam.update();
        // The city lives: a minute step when the clock turns, a few paths per frame,
        // and nobody walks onto the player.
        let standing = match (self.mode, &st.player) {
            (Mode::Walk, Some(p)) if p.stratum == StratumKind::Street => Some(p.pos),
            _ => None,
        };
        if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
            world.advance(city, st.tick);
            world.path_some(city, 4);
            world.set_blocker(standing);
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let Some(city) = st.city.as_ref() else {
            f.render_widget(
                Paragraph::new("no city loaded · q back").alignment(Alignment::Center),
                Rect::new(area.x, area.y + area.height / 2, area.width, 1),
            );
            return;
        };
        if area.width < 41 || area.height < 8 {
            crate::app::draw_too_small(f);
            return;
        }
        let r = layout(area);
        let map = r.map;
        self.last_map.set(map);
        let origin = self.cam.origin(map.width, map.height);
        let cursor = self.focus_pos(st);
        let focus = if self.digital || self.stratum == StratumKind::Net {
            Digital::node_under(city, self.stratum, cursor)
        } else {
            None
        };
        let digital = Digital {
            enabled: self.digital,
            focus,
        };
        let mut overlays: Vec<&dyn Overlay> = Vec::new();
        if st.rain {
            overlays.push(&Weather);
        }
        overlays.push(&digital);
        let walking = self.mode == Mode::Walk
            && st
                .player
                .as_ref()
                .is_some_and(|p| p.stratum == self.stratum);
        let vis = if walking {
            st.player.as_ref().map(|p| p.vis())
        } else {
            None
        };
        let ground = GroundOverlay {
            ground: &st.ground,
            content: &st.content,
            vis,
        };
        overlays.push(&ground);
        let (hour, _) = hud::clock_at(st.tick);
        let people = st.world.as_ref().map(|world| PeopleOverlay {
            world,
            night: hour >= 18 || hour <= 5,
            vis,
        });
        if let Some(p) = people.as_ref() {
            overlays.push(p);
        }
        let fx = FxOverlay { fx: &st.fx, vis };
        overlays.push(&fx);
        MapView {
            city,
            stratum: self.stratum,
            origin,
            tick: st.tick,
            style: &st.style,
            vis,
            overlays: &overlays,
            cursor: if self.mode == Mode::Explore {
                Some(cursor)
            } else {
                None
            },
        }
        .render(map, f.buffer_mut());

        if let Some(p) = &st.player {
            if let Some((bk, bp)) = p.body
                && bk == self.stratum
                && let Some((sx, sy)) = tile_to_screen(map, origin, bp)
            {
                f.buffer_mut()[(sx, sy)].set_char('@').set_style(
                    Style::default()
                        .fg(Color::Rgb(110, 120, 130))
                        .add_modifier(Modifier::DIM),
                );
            }
            if p.stratum == self.stratum
                && let Some((sx, sy)) = tile_to_screen(map, origin, p.pos)
            {
                let fg = if p.stratum == StratumKind::Net {
                    Color::Rgb(0, 255, 200)
                } else {
                    Color::Rgb(255, 255, 255)
                };
                f.buffer_mut()[(sx, sy)]
                    .set_char('@')
                    .set_style(Style::default().fg(fg).add_modifier(Modifier::BOLD));
            }
        }

        // The vertical rule on every row but the two rule rows.
        {
            let buf = f.buffer_mut();
            for y in area.y..r.bottom_rule.y {
                if y == r.top_rule.y {
                    continue;
                }
                buf[(r.vrule_x, y)].set_char('│').set_style(hud::RULE_STYLE);
            }
        }
        // Top band.
        let ch = &st.character;
        let lw = r.band_l[0].width;
        let rw = r.band_r[0].width;
        let status = hud::status_line(
            &hud::Status {
                dev: st.dev,
                handle: &ch.handle,
                level: ch.level,
                credits: ch.credits,
                seed: city.seed,
                fps: st.show_fps.then_some(self.fps.fps),
            },
            lw,
        );
        f.render_widget(Paragraph::new(status), r.band_l[0]);
        let hp_max = ch.hp_max(&st.content);
        let rows = [
            hud::bar_row(
                "HP",
                ch.hp,
                hp_max,
                hud::hp_colour(ch.hp, hp_max, st.tick),
                lw,
            ),
            hud::bar_row("FOCUS", ch.focus, ch.focus_max, CYAN, lw),
            hud::bar_row(
                "XP",
                ch.xp,
                game::Character::xp_to_next(ch.level),
                hud::DIM_AMBER,
                lw,
            ),
        ];
        for (i, row) in rows.into_iter().enumerate() {
            f.render_widget(Paragraph::new(row), r.band_l[i + 1]);
        }
        let location = hud::location(city, self.stratum, cursor);
        f.render_widget(
            Paragraph::new(hud::location_line(&location, st.clock(), rw)),
            r.band_r[0],
        );
        f.render_widget(
            Paragraph::new(hud::carry_row(
                ch.weight(&st.content),
                ch.carry_cap(&st.content),
                rw,
            )),
            r.band_r[1],
        );
        let cross = r.vrule_x - area.x;
        f.render_widget(
            Paragraph::new(hud::rule_row(area.width, cross, '┿')),
            r.top_rule,
        );
        f.render_widget(
            Paragraph::new(hud::rule_row(area.width, cross, '┷')),
            r.bottom_rule,
        );
        // Right column.
        let person = st
            .world
            .as_ref()
            .and_then(|w| w.person_at(cursor, st.tick).map(|id| w.card(id, city)));
        let view = sidebar::SidebarView {
            city,
            stratum: self.stratum,
            focus: cursor,
            person: person.as_ref(),
            view: TileRect::new(origin.x, origin.y, map.width as i32, map.height as i32),
            player: st
                .player
                .as_ref()
                .filter(|p| p.stratum == self.stratum)
                .map(|p| p.pos),
            style: &st.style,
            content: &st.content,
            character: &st.character,
            ground: &st.ground,
            log: &st.log,
            tick: st.tick,
            dev: st.dev,
        };
        let here = sidebar::here_summary(&view);
        // In dev mode the column's last row holds the dev tabs, with an empty row above.
        let side = if st.dev && r.side.height > 2 {
            Rect::new(r.side.x, r.side.y, r.side.width, r.side.height - 2)
        } else {
            r.side
        };
        let drawn = sidebar::draw(&view, &st.sidebar, side, f.buffer_mut());
        let log_drawn = drawn.contains(&Section::Log);
        let (dev_tabs, tabs) = hud::tab_rows(&st.sidebar, st.dev, &drawn);
        if st.dev && r.side.height > 2 {
            f.render_widget(
                Paragraph::new(dev_tabs),
                Rect::new(r.side.x, r.side.bottom() - 1, r.side.width, 1),
            );
        }
        f.render_widget(Paragraph::new(tabs), r.band_r[3]);
        // Footer.
        let jacked = st.player.as_ref().is_some_and(|p| p.jacked_in());
        let mut status_spans = match (self.mode, jacked) {
            (Mode::Walk, true) => vec![
                Span::styled("⚡", Style::default().fg(CYAN)),
                Span::styled(" jacked in · Net", Style::default().fg(TEXT)),
            ],
            (Mode::Walk, false) => vec![Span::styled(
                format!("on foot · {}", self.stratum.label()),
                Style::default().fg(TEXT),
            )],
            (Mode::Explore, _) => vec![Span::styled(
                format!("camera · {}", self.stratum.label()),
                Style::default().fg(TEXT),
            )],
        };
        if ch.overburdened(&st.content) {
            status_spans.push(Span::styled(
                " · overburdened",
                Style::default().fg(MAGENTA),
            ));
        }
        let verbs = match self.mode {
            Mode::Walk => hud::WALK_VERBS,
            Mode::Explore => hud::EXPLORE_VERBS,
        };
        f.render_widget(
            Paragraph::new(hud::footer_status(status_spans, &here, verbs, area.width)),
            r.status,
        );
        let slots = Self::slots(st);
        let recent = st.log.recent(st.tick);
        let bottom = match &self.console {
            Some(con) => console::line(con, st.tick, recent),
            None => hud::footer_abilities(&slots, st.dev, area.width),
        };
        f.render_widget(Paragraph::new(bottom), r.abilities);
        // With the LOG pane off or squeezed out, the newest entry shows as a toast.
        if !log_drawn
            && self.console.is_none()
            && let Some(e) = recent
        {
            let style = match e.kind {
                Kind::Warn => Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD),
                Kind::Story => Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
                Kind::Info => Style::default().fg(CYAN),
            };
            put(
                f.buffer_mut(),
                map.x + 1,
                map.bottom().saturating_sub(1),
                &e.marked(),
                style,
                map.right(),
            );
        }

        if self.help {
            help::draw(f, self.mode);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn last(st: &AppState) -> Option<&str> {
        st.log.latest().map(|e| e.text.as_str())
    }

    fn last_is_warning(st: &AppState) -> bool {
        st.log.latest().is_some_and(|e| e.kind == Kind::Warn)
    }
    use crate::app::{AppState, Screen, ScreenCmd};
    use crate::screens::testutil::{buffer_text, key};
    use citygen::{GenParams, Generator, SizePreset, StratumKind};
    use crossterm::event::KeyCode;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn state() -> AppState {
        let mut st = AppState::new();
        st.city = Some(
            Generator::run(GenParams {
                seed: 6,
                size: SizePreset::Custom { w: 120, h: 60 },
                districts: 5,
                ..Default::default()
            })
            .unwrap(),
        );
        st
    }

    #[test]
    fn explore_and_walk_round_trip() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("Street"), "{text}");
        s.update(&mut st, &[key(KeyCode::Char(']'))]);
        assert_eq!(s.stratum, StratumKind::Skyways);
        s.update(&mut st, &[key(KeyCode::Char('['))]);
        s.update(&mut st, &[key(KeyCode::Char('['))]);
        assert_eq!(s.stratum, StratumKind::Undercity);
        s.update(&mut st, &[key(KeyCode::Char('['))]);
        assert_eq!(s.stratum, StratumKind::Undercity);
        s.update(&mut st, &[key(KeyCode::Char('w'))]);
        assert!(st.player.is_some());
        assert_eq!(s.mode, Mode::Walk);
        assert_eq!(s.stratum, StratumKind::Street);
        for d in ['h', 'j', 'k', 'l', 'y', 'u', 'b', 'n'] {
            st.tick += 1;
            s.update(&mut st, &[key(KeyCode::Char(d))]);
        }
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        s.update(&mut st, &[key(KeyCode::Char('<'))]);
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert!(buffer_text(t.backend().buffer()).contains('@'));
        s.update(&mut st, &[key(KeyCode::Char('d'))]);
        assert!(s.digital);
        t.draw(|f| s.draw(&st, f)).unwrap();
        s.update(&mut st, &[key(KeyCode::Char('?'))]);
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert!(buffer_text(t.backend().buffer()).contains("move"));
        s.update(&mut st, &[key(KeyCode::Esc)]);
        assert!(!s.help);
        // Saving pushes the save screen over the live map; the map keeps its state.
        s.stratum = StratumKind::Undercity;
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('s'))]),
            ScreenCmd::Push { .. }
        ));
        assert_eq!((s.mode, s.stratum), (Mode::Walk, StratumKind::Undercity));
        s.update(&mut st, &[key(KeyCode::Char('c'))]);
        assert_eq!(s.mode, Mode::Explore);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('q'))]),
            ScreenCmd::Push { .. }
        ));
    }

    #[test]
    fn explore_pans_and_switches_to_other_screens() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        s.update(&mut st, &[]);
        let before = s.cursor;
        s.update(&mut st, &[key(KeyCode::Right), key(KeyCode::Down)]);
        assert_eq!(s.cursor, Pos::new(before.x + 1, before.y + 1));
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('r'))]),
            ScreenCmd::Switch { .. }
        ));
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('g'))]),
            ScreenCmd::Switch { .. }
        ));
        let mut empty = AppState::new();
        let mut t2 = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t2.draw(|f| s.draw(&empty, f)).unwrap();
        assert!(matches!(
            s.update(&mut empty, &[key(KeyCode::Char('w'))]),
            ScreenCmd::Stay
        ));
    }

    #[test]
    fn explore_cursor_reaches_the_map_corners_without_showing_void() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        s.update(&mut st, &[]);
        let map = s.last_map.get();
        for _ in 0..40 {
            s.update(&mut st, &[key(KeyCode::Char('H')), key(KeyCode::Char('K'))]);
        }
        assert_eq!(s.cursor, Pos::new(0, 0));
        for _ in 0..30 {
            s.update(&mut st, &[]);
        }
        let o = s.cam.origin(map.width, map.height);
        assert!(o.x >= 0 && o.y >= 0, "origin {o:?}");
        for _ in 0..40 {
            s.update(&mut st, &[key(KeyCode::Char('L')), key(KeyCode::Char('J'))]);
        }
        assert_eq!(s.cursor, Pos::new(119, 59));
        for _ in 0..30 {
            s.update(&mut st, &[]);
        }
        let o = s.cam.origin(map.width, map.height);
        assert!(
            o.x + map.width as i32 <= 120 && o.y + map.height as i32 <= 60,
            "origin {o:?}"
        );
        st.sidebar.set(Section::Inspect, true);
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("119,59"), "{text}");
    }

    #[test]
    fn enter_jacks_in_and_the_net_is_the_fifth_stratum() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        for _ in 0..4 {
            s.update(&mut st, &[key(KeyCode::Char(']'))]);
        }
        assert_eq!(s.stratum, StratumKind::Net);
        s.update(&mut st, &[key(KeyCode::Char(']'))]);
        assert_eq!(s.stratum, StratumKind::Net);
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("· Net"), "{text}");
        assert!(
            text.contains('═') || text.contains('┃') || text.contains('━'),
            "{text}"
        );

        // Walk up to a public port and jack in.
        s.update(&mut st, &[key(KeyCode::Char('['))]);
        s.update(&mut st, &[key(KeyCode::Char('w'))]);
        let city = st.city.as_ref().unwrap();
        let jack = city
            .connectors
            .iter()
            .find(|k| k.kind == citygen::ConnectorKind::Jack && k.public)
            .unwrap()
            .pos;
        let stand = jack
            .neighbors8()
            .find(|n| city.walkable(StratumKind::Street, *n))
            .unwrap();
        {
            let p = st.player.as_mut().unwrap();
            p.pos = stand;
            p.stratum = StratumKind::Street;
        }
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Flash(TransitionKind::Glitch)
        ));
        let p = st.player.as_ref().unwrap();
        assert!(p.jacked_in());
        assert_eq!(s.stratum, StratumKind::Net);
        assert_eq!(p.pos, jack);
        st.tick += 500;
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("jacked in"), "{text}");
        assert!(
            text.contains("· Net") && text.contains("[tab]deck"),
            "{text}"
        );
        // The body shows on the Street while the avatar is in the Net.
        s.mode = Mode::Explore;
        s.stratum = StratumKind::Street;
        s.cursor = stand;
        s.cam.snap(stand);
        t.draw(|f| s.draw(&st, f)).unwrap();
        let map = s.last_map.get();
        let origin = s.cam.origin(map.width, map.height);
        let (sx, sy) = tile_to_screen(map, origin, stand).unwrap();
        assert_eq!(t.backend().buffer()[(sx, sy)].symbol(), "@");
        s.mode = Mode::Walk;
        s.stratum = StratumKind::Net;
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Flash(TransitionKind::Glitch)
        ));
        let p = st.player.as_ref().unwrap();
        assert!(!p.jacked_in());
        assert_eq!((p.stratum, p.pos), (StratumKind::Street, stand));
        assert_eq!(s.stratum, StratumKind::Street);
        s.update(&mut st, &[key(KeyCode::Char('?'))]);
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert!(buffer_text(t.backend().buffer()).contains("jack in"));
    }

    #[test]
    fn dev_mode_off_locks_the_player_into_walk_mode() {
        let mut st = state();
        st.dev = false;
        let mut s = MapScreen::explore(0);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        s.update(&mut st, &[]);
        assert_eq!(s.mode, Mode::Walk);
        assert!(st.player.is_some());
        s.update(&mut st, &[key(KeyCode::Tab)]);
        assert_eq!(s.mode, Mode::Walk, "tab must not open the free camera");
        st.sidebar.set(Section::Inspect, true);
        st.tick += 500;
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(!text.contains("INSPECT"), "{text}");
        assert!(!text.contains("c camera"), "{text}");
        assert!(!text.contains("DEV"), "{text}");
        // The backtick flips dev mode at runtime; the camera and inspector come back.
        s.update(&mut st, &[key(KeyCode::Char('`'))]);
        assert!(st.dev);
        s.update(&mut st, &[key(KeyCode::Char('c'))]);
        assert_eq!(s.mode, Mode::Explore);
        st.sidebar.set(Section::Inspect, true);
        st.tick += 500;
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("pos ") && text.contains("DEV"), "{text}");
        // Flipping it off again drops straight back into the walk.
        s.update(&mut st, &[key(KeyCode::Char('`'))]);
        assert!(!st.dev);
        assert_eq!(s.mode, Mode::Walk);
    }

    #[test]
    fn rain_and_fps_follow_the_settings() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        st.tick = 100;
        s.update(&mut st, &[]);
        let rain: Vec<char> = st.style.glyphs.rain.clone();
        let count = |t: &Terminal<TestBackend>| {
            buffer_text(t.backend().buffer())
                .chars()
                .filter(|c| rain.contains(c))
                .count()
        };
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert!(count(&t) > 0, "no rain on the street");
        assert!(buffer_text(t.backend().buffer()).contains("fps"));
        st.rain = false;
        st.show_fps = false;
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert_eq!(count(&t), 0);
        assert!(!buffer_text(t.backend().buffer()).contains("fps"));
    }

    #[test]
    fn esc_closes_help_before_pausing() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        s.update(&mut st, &[key(KeyCode::Char('?'))]);
        assert!(s.help);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Stay
        ));
        assert!(!s.help);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Push { .. }
        ));
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('q'))]),
            ScreenCmd::Push { .. }
        ));
    }

    #[test]
    fn tab_opens_the_deck_and_c_is_the_dev_camera() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Tab)]),
            ScreenCmd::Push {
                fx: TransitionKind::Glitch,
                ..
            }
        ));
        s.update(&mut st, &[key(KeyCode::Char('w'))]);
        assert_eq!(s.mode, Mode::Walk);
        s.update(&mut st, &[key(KeyCode::Char('c'))]);
        assert_eq!(s.mode, Mode::Explore, "c returns to the camera in dev mode");
        s.update(&mut st, &[key(KeyCode::Char('w'))]);
        st.dev = false;
        s.update(&mut st, &[key(KeyCode::Char('c'))]);
        assert_eq!(s.mode, Mode::Walk, "no camera without dev mode");
        assert!(
            matches!(
                s.update(&mut st, &[key(KeyCode::Tab)]),
                ScreenCmd::Push { .. }
            ),
            "the deck opens in play too"
        );
        st.tick += 500;
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(
            text.contains("[tab]deck") && !text.contains("c camera"),
            "{text}"
        );
    }

    #[test]
    fn fps_meter_ignores_the_gap_while_a_menu_is_open() {
        use std::time::{Duration, Instant};
        let t0 = Instant::now();
        let mut m = FpsMeter::new(t0);
        for i in 1..=60u64 {
            m.frame_at(t0 + Duration::from_millis(i * 1000 / 60 + 1));
        }
        assert!((m.fps - 60.0).abs() < 2.0, "{}", m.fps);
        // Three seconds under a menu: the map resumes and the first frame back opens a
        // new window instead of measuring one frame over the whole pause.
        let back = t0 + Duration::from_millis(4001);
        m.resume(back);
        m.frame_at(back);
        assert!((m.fps - 60.0).abs() < 2.0, "after the gap: {}", m.fps);
        // A fresh second of frames measures normally again.
        for i in 1..=60u64 {
            m.frame_at(back + Duration::from_millis(i * 1000 / 60 + 1));
        }
        assert!((m.fps - 60.0).abs() < 2.0, "{}", m.fps);
    }

    #[test]
    fn explore_action_bar_and_help_mention_save() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        let status = text.lines().nth_back(1).unwrap();
        assert!(
            status.contains("[w]alk") && status.contains("[?]keys"),
            "{status:?}"
        );
        s.update(&mut st, &[key(KeyCode::Char('?'))]);
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert!(buffer_text(t.backend().buffer()).contains("save city"));
        s.update(&mut st, &[key(KeyCode::Char('?'))]);
    }

    #[test]
    fn deck_opened_from_the_free_camera_links_to_the_cursor() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        let (pos, name, other, other_name) = {
            let city = st.city.as_ref().unwrap();
            let street = StratumKind::Street;
            let mut tiles = (0..city.size.h as i32)
                .flat_map(|y| (0..city.size.w as i32).map(move |x| Pos::new(x, y)))
                .filter_map(|p| Some((p, city.tile(street, p)?.district?)));
            let (pos, d) = tiles.next().expect("a tile with a district");
            let (other, d2) = tiles.find(|(_, d2)| *d2 != d).expect("a second district");
            let name = |d| city.district(d).name.clone();
            (pos, name(d), other, name(d2))
        };
        assert_ne!(name, other_name);
        // The player stands in another district: under the free camera the cursor wins.
        let mut p = crate::walker::Player::spawn(st.city.as_ref().unwrap(), 1);
        p.pos = other;
        st.player = Some(p);
        s.cursor = pos;
        let ScreenCmd::Push { to, .. } = s.update(&mut st, &[key(KeyCode::Tab)]) else {
            panic!("tab opens the deck");
        };
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        st.tick = 100;
        t.draw(|f| to.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains(&format!("LINK · {name} · Street")), "{text}");
        assert!(!text.contains(&format!("LINK · {other_name}")), "{text}");
    }

    #[test]
    fn fps_meter_counts_slow_frames() {
        use std::time::{Duration, Instant};
        let t0 = Instant::now();
        let mut m = FpsMeter::new(t0);
        for i in 1..=60u64 {
            m.frame_at(t0 + Duration::from_millis(i * 1000 / 60 + 1));
        }
        assert!((m.fps - 60.0).abs() < 2.0, "{}", m.fps);
        // Two frames seven tenths of a second apart are a stall, not a menu: they count.
        m.frame_at(t0 + Duration::from_millis(1701));
        m.frame_at(t0 + Duration::from_millis(2401));
        assert!((m.fps - 1.43).abs() < 0.1, "{}", m.fps);
    }

    #[test]
    fn a_tick_gap_restarts_the_fps_window() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        st.tick = 1;
        s.update(&mut st, &[]);
        st.tick = 2;
        s.update(&mut st, &[]);
        assert_eq!(s.fps.frames, 2);
        st.tick = 50; // a menu was on top for 48 ticks
        s.update(&mut st, &[]);
        assert_eq!(
            s.fps.frames, 1,
            "the window restarts on the first frame back"
        );
    }

    #[test]
    fn walk_draws_piles_around_the_player() {
        let mut st = state();
        st.rain = false;
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        s.update(&mut st, &[key(KeyCode::Char('w'))]);
        let (k, pos) = {
            let p = st.player.as_ref().unwrap();
            (p.stratum, p.pos)
        };
        let walkable: Vec<Pos> = {
            let city = st.city.as_ref().unwrap();
            pos.neighbors8().filter(|q| city.walkable(k, *q)).collect()
        };
        assert!(!walkable.is_empty());
        for q in &walkable {
            st.ground.add(k, *q, game::ItemId::new("scrap_servo"), 1);
        }
        st.ground.add(k, pos, game::ItemId::new("copper_coil"), 1);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let buf = t.backend().buffer();
        let cells: Vec<(u16, u16)> = (0..40u16)
            .flat_map(|y| (0..120u16).map(move |x| (x, y)))
            .collect();
        assert!(cells.iter().any(|&(x, y)| buf[(x, y)].symbol() == "@"));
        let amber = crate::render::items::AMBER.to_color();
        assert!(
            cells
                .iter()
                .any(|&(x, y)| buf[(x, y)].symbol() == "▪" && buf[(x, y)].fg == amber),
            "a pile next to the player"
        );
    }

    fn walking(st: &mut AppState, s: &mut MapScreen) {
        s.update(st, &[]);
        s.update(st, &[key(KeyCode::Char('w'))]);
        assert_eq!(s.mode, Mode::Walk);
    }

    fn draw_screen(scr: &dyn Screen, st: &AppState, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| scr.draw(st, f)).unwrap();
        buffer_text(t.backend().buffer())
    }

    #[test]
    fn space_offers_take_rows_and_takes() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let (k, pos) = {
            let p = st.player.as_ref().unwrap();
            (p.stratum, p.pos)
        };
        st.ground.add(k, pos, game::ItemId::new("copper_coil"), 2);
        st.ground.add(k, pos, game::ItemId::new("dead_phone"), 1);
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens the interact popup")
        };
        st.tick += 40;
        let text = draw_screen(&*p, &st, 120, 40);
        assert!(
            text.contains("a  take 2× Copper coil")
                && text.contains("b  take 1× Dead phone")
                && text.contains("t  take all")
                && text.contains("l  look"),
            "{text}"
        );
        p.update(&mut st, &[key(KeyCode::Char('t'))]);
        assert_eq!(st.character.count(&game::ItemId::new("copper_coil")), 5);
        assert_eq!(st.character.count(&game::ItemId::new("dead_phone")), 2);
        assert!(st.ground.at(k, pos).is_empty());
        assert_eq!(last(&st), Some("took 3 items"));
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!()
        };
        st.tick += 40;
        let text = draw_screen(&*p, &st, 120, 40);
        assert!(text.contains("l  look") && !text.contains("take"), "{text}");
        p.update(&mut st, &[key(KeyCode::Char('l'))]);
        st.tick += 40;
        assert!(draw_screen(&*p, &st, 120, 40).contains("terrain"));
    }

    #[test]
    fn space_jacks_and_takes_stairs_and_the_map_follows() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let city = st.city.clone().unwrap();
        let street = StratumKind::Street;
        let beside_jack = city
            .connectors
            .iter()
            .filter(|k| {
                k.kind == citygen::ConnectorKind::Jack
                    && k.serves.contains(&street)
                    && city.walkable(StratumKind::Net, k.pos)
            })
            .find_map(|k| {
                std::iter::once(k.pos)
                    .chain(k.pos.neighbors8())
                    .find(|q| city.walkable(street, *q))
            })
            .unwrap();
        st.player.as_mut().unwrap().pos = beside_jack;
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens the interact popup")
        };
        st.tick += 40;
        assert!(draw_screen(&*p, &st, 120, 40).contains("j  jack in"));
        let cmd = p.update(&mut st, &[key(KeyCode::Char('j'))]);
        assert!(
            matches!(
                cmd,
                ScreenCmd::Pop {
                    fx: crate::app::TransitionKind::Glitch
                }
            ),
            "status: {:?}",
            last(&st)
        );
        assert!(st.player.as_ref().unwrap().jacked_in());
        s.update(&mut st, &[]);
        assert_eq!(s.stratum, StratumKind::Net, "the map follows the player");
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!()
        };
        st.tick += 40;
        assert!(draw_screen(&*p, &st, 120, 40).contains("j  jack out"));
        p.update(&mut st, &[key(KeyCode::Char('j'))]);
        s.update(&mut st, &[]);
        assert_eq!(s.stratum, street);
        let stair = city
            .connectors
            .iter()
            .find(|k| {
                k.public
                    && k.serves.contains(&StratumKind::Undercity)
                    && k.serves.contains(&street)
                    && city.walkable(street, k.pos)
                    && city.walkable(StratumKind::Undercity, k.pos)
            })
            .unwrap()
            .pos;
        st.player.as_mut().unwrap().pos = stair;
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!()
        };
        st.tick += 40;
        assert!(draw_screen(&*p, &st, 120, 40).contains("<  go down to Undercity"));
        p.update(&mut st, &[key(KeyCode::Char('<'))]);
        s.update(&mut st, &[]);
        assert_eq!(s.stratum, StratumKind::Undercity);
        assert_eq!(last(&st), Some("now on Undercity"));
    }

    #[test]
    fn a_lists_actives_fires_and_cools() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        st.tick = 100;
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char('a'))])
        else {
            panic!("a opens the abilities popup")
        };
        st.tick += 40;
        let text = draw_screen(&*p, &st, 120, 40);
        assert!(
            text.contains("ABILITIES") && text.contains("a  Netscan · ready"),
            "{text}"
        );
        let seen = |st: &AppState| {
            st.player
                .as_ref()
                .unwrap()
                .vis()
                .count(|v| *v != crate::render::Visibility::Unseen)
        };
        let before = seen(&st);
        p.update(&mut st, &[key(KeyCode::Char('a'))]);
        assert!(seen(&st) > before, "the scan painted the map");
        assert_eq!(last(&st), Some("Netscan: pulse sent"));
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char('a'))])
        else {
            panic!()
        };
        st.tick += 40;
        let text = draw_screen(&*p, &st, 120, 40);
        assert!(text.contains("Netscan · cooling"), "{text}");
        let msg = last(&st).map(|m| m.to_string());
        // The cooling row's own letter does nothing; Enter would fire the next ready one.
        assert!(
            matches!(
                p.update(&mut st, &[key(KeyCode::Char('a'))]),
                ScreenCmd::Stay
            ),
            "a cooling row cannot fire"
        );
        assert_eq!(last(&st).map(|m| m.to_string()), msg);
        st.character.equipment.clear();
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('a'))]),
            ScreenCmd::Stay
        ));
        assert_eq!(last(&st), Some("no active abilities"));
    }

    #[test]
    fn colon_opens_the_console_in_dev_mode() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        st.dev = false;
        s.update(&mut st, &[key(KeyCode::Char(':'))]);
        assert!(s.console.is_none());
        st.dev = true;
        s.update(&mut st, &[key(KeyCode::Char(':'))]);
        assert!(s.console.is_some());
        let text = draw_screen(&s, &st, 120, 40);
        let prompt = text.lines().last().unwrap();
        assert!(prompt.starts_with(": "), "{prompt:?}");
        st.character.hp = 1;
        for c in "heal".chars() {
            s.update(&mut st, &[key(KeyCode::Char(c))]);
        }
        assert!(draw_screen(&s, &st, 120, 40).contains(": heal"));
        s.update(&mut st, &[key(KeyCode::Enter)]);
        assert_eq!(st.character.hp, 45);
        assert_eq!(last(&st), Some("hp 45/45"));
        let screen = draw_screen(&s, &st, 120, 40);
        let last_row = screen.lines().last().unwrap();
        assert!(
            last_row.starts_with(": ") && last_row.contains("hp 45/45"),
            "the result shows beside the prompt: {last_row:?}"
        );
        for c in "fly".chars() {
            s.update(&mut st, &[key(KeyCode::Char(c))]);
        }
        s.update(&mut st, &[key(KeyCode::Enter)]);
        let screen = draw_screen(&s, &st, 120, 40);
        assert!(
            screen
                .lines()
                .last()
                .unwrap()
                .contains("unknown command fly")
        );
        s.update(&mut st, &[key(KeyCode::Esc)]);
        assert!(s.console.is_none());
    }

    #[test]
    fn stairs_are_offered_only_where_they_lead() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let city = st.city.clone().unwrap();
        let street = StratumKind::Street;
        let mut checked = 0;
        for k in city.connectors.iter().filter(|k| {
            k.kind != citygen::ConnectorKind::Jack
                && k.serves.contains(&street)
                && city.walkable(street, k.pos)
        }) {
            st.player.as_mut().unwrap().pos = k.pos;
            let ScreenCmd::Push { to, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))]) else {
                panic!()
            };
            st.tick += 40;
            let text = draw_screen(&*to, &st, 120, 40);
            let (below, above) = k.serves_from(street).unwrap_or((None, None));
            let leads_down = below.is_some_and(|d| city.walkable(d, k.pos));
            let leads_up = above.is_some_and(|d| city.walkable(d, k.pos));
            assert_eq!(text.contains("go down"), leads_down, "{:?}\n{text}", k.pos);
            assert_eq!(text.contains("go up"), leads_up, "{:?}\n{text}", k.pos);
            checked += 1;
            if checked == 40 {
                break;
            }
        }
        assert!(checked > 0);
    }

    #[test]
    fn keys_after_colon_in_the_same_frame_go_to_the_console() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        st.dev = true;
        let cmd = s.update(&mut st, &[key(KeyCode::Char(':')), key(KeyCode::Char('s'))]);
        assert!(
            matches!(cmd, ScreenCmd::Stay),
            "s must not open the save screen"
        );
        assert_eq!(s.console.as_ref().map(|c| c.input.as_str()), Some("s"));
    }

    #[test]
    fn number_keys_fire_slots() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('1'))]);
        assert_eq!(last(&st), Some("Netscan: pulse sent"));
        st.tick += 200;
        let text = draw_screen(&s, &st, 100, 30);
        assert!(text.contains("Netscan: pulse sent"), "{text}");
        assert!(text.contains("1 netscan 4s"), "{text}");
        s.update(&mut st, &[key(KeyCode::Char('1'))]);
        assert!(last_is_warning(&st));
        s.update(&mut st, &[key(KeyCode::Char('3'))]);
        assert_eq!(last(&st), Some("nothing in slot 3"));
    }

    #[test]
    fn view_popup_toggles_sections() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char('v'))])
        else {
            panic!("v opens the view popup")
        };
        st.tick += 40;
        let text = draw_screen(&*p, &st, 100, 30);
        assert!(
            text.contains("VIEW") && text.contains("a  ▣ here") && text.contains("e  ▢ inspect"),
            "{text}"
        );
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Char('a'))]),
            ScreenCmd::Pop { .. }
        ));
        assert!(!st.sidebar.is_on(Section::Here));
        let text = draw_screen(&s, &st, 100, 30);
        assert!(
            !text.contains("[F1]Here*") && text.contains("[F1]Here "),
            "{text}"
        );
        st.dev = false;
        let ScreenCmd::Push { to: p, .. } = s.update(&mut st, &[key(KeyCode::Char('v'))]) else {
            panic!()
        };
        let text = draw_screen(&*p, &st, 100, 30);
        assert!(!text.contains("inspect"), "{text}");
    }

    #[test]
    fn dev_off_clears_dev_sections() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        st.sidebar.set(Section::Inspect, true);
        s.update(&mut st, &[key(KeyCode::Char('`'))]);
        assert!(!st.dev && !st.sidebar.is_on(Section::Inspect));
    }

    #[test]
    fn m_opens_the_message_history() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        st.say("took it");
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char('m'))])
        else {
            panic!("m opens messages")
        };
        let text = draw_screen(&*p, &st, 100, 30);
        assert!(
            text.contains("MESSAGES") && text.contains("took it"),
            "{text}"
        );
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
    }

    #[test]
    fn grid_draws_bands_rules_and_the_column() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        for (w, h) in [(100u16, 30u16), (120, 40)] {
            let text = draw_screen(&s, &st, w, h);
            let lines: Vec<&str> = text.lines().collect();
            let l = (w - sidebar::width(w) - 1) as usize;
            let at = |y: usize, x: usize| lines[y].chars().nth(x);
            assert!(
                lines[0].contains(" DEV ") && lines[0].contains("VEX :: LVL 1 :: ¤ 320"),
                "{text}"
            );
            assert!(
                lines[0].contains("· Street") && lines[0].trim_end().ends_with("22:00"),
                "{text}"
            );
            assert!(
                lines[1].starts_with(" HP ") && lines[1].contains("CARRY"),
                "{text}"
            );
            assert!(
                lines[2].starts_with(" FOCUS") && !lines[2].contains("[F5]"),
                "the band row the dev tabs left stays blank: {text}"
            );
            let last_main = h as usize - 4;
            assert!(
                lines[last_main].contains("[F5]Inspect [F6]Legend"),
                "dev tabs sit on the column's last row: {text}"
            );
            let above: String = lines[last_main - 1].chars().skip(l + 1).collect();
            assert!(above.trim().is_empty(), "one empty row above them: {text}");
            assert!(
                lines[3].starts_with(" XP ")
                    && lines[3].contains("[F1]Here* [F2]Quest* [F3]Map* [F4]Log*"),
                "{text}"
            );
            assert_eq!(
                at(4, l),
                Some('┿'),
                "the light column rule crosses the heavy band rule: {text}"
            );
            assert_eq!(
                at(h as usize - 3, l),
                Some('┷'),
                "and ends on the footer rule with a light stem: {text}"
            );
            for y in (0..4).chain(5..h as usize - 3) {
                assert_eq!(at(y, l), Some('│'), "row {y}: {text}");
            }
            assert_ne!(
                at(h as usize - 2, l),
                Some('│'),
                "footer spans the width: {text}"
            );
            assert!(
                lines[h as usize - 2].starts_with(" STATUS: camera · Street"),
                "{text}"
            );
            assert!(
                lines[h as usize - 2].contains("HERE: ")
                    && lines[h as usize - 2].trim_end().ends_with("[?]keys"),
                "{text}"
            );
            assert!(
                lines[h as usize - 1].starts_with(" ABILITIES: 1 netscan ●"),
                "{text}"
            );
            assert!(
                !text.contains('╭') && !text.contains("── HERE") && !text.contains("── LOG"),
                "{text}"
            );
        }
        walking(&mut st, &mut s);
        let text = draw_screen(&s, &st, 100, 30);
        assert!(
            text.contains(" STATUS: on foot · Street") && text.contains("[space]act"),
            "{text}"
        );
        st.dev = false;
        let text = draw_screen(&s, &st, 100, 30);
        assert!(
            !text.contains("[F5]") && !text.contains(" DEV ") && !text.contains("[:]console"),
            "{text}"
        );
    }

    #[test]
    fn frame_fits_at_minimum_with_every_section() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        for sec in Section::ALL {
            st.sidebar.set(sec, true);
        }
        for _ in 0..30 {
            st.say("a long line of text that will need wrapping in the log");
        }
        st.warn("nothing leads up from here");
        st.tick += 200;
        let text = draw_screen(&s, &st, 100, 30);
        assert!(
            text.contains("[tab]deck") && text.contains("[F1]Here*"),
            "{text}"
        );
        // The LOG pane lost its rows to the dev panes: the refusal still shows.
        assert!(text.contains("! nothing leads up from here"), "{text}");
        let text = draw_screen(&s, &st, 200, 60);
        assert!(text.contains("· Street"), "{text}");
    }

    #[test]
    fn hidden_log_shows_a_toast() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        st.sidebar.set(Section::Log, false);
        st.warn("nothing leads up from here");
        let text = draw_screen(&s, &st, 100, 30);
        assert!(text.contains("! nothing leads up from here"), "{text}");
        st.tick += 100;
        let text = draw_screen(&s, &st, 100, 30);
        assert!(!text.contains("nothing leads up"), "{text}");
    }

    #[test]
    fn f_keys_toggle_sections() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        s.update(&mut st, &[key(KeyCode::F(1))]);
        assert!(!st.sidebar.is_on(Section::Here));
        assert!(draw_screen(&s, &st, 100, 30).contains("[F1]Here [F2]Quest*"));
        s.update(&mut st, &[key(KeyCode::F(1)), key(KeyCode::F(5))]);
        assert!(st.sidebar.is_on(Section::Here) && st.sidebar.is_on(Section::Inspect));
        st.set_dev(false);
        let before = st.log.len();
        s.update(&mut st, &[key(KeyCode::F(5)), key(KeyCode::F(7))]);
        assert!(!st.sidebar.is_on(Section::Inspect));
        assert_eq!(st.log.len(), before);
    }

    #[test]
    fn status_zone_follows_the_jack() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let city = st.city.clone().unwrap();
        let street = StratumKind::Street;
        let beside_jack = city
            .connectors
            .iter()
            .filter(|k| {
                k.kind == citygen::ConnectorKind::Jack
                    && k.serves.contains(&street)
                    && city.walkable(StratumKind::Net, k.pos)
            })
            .find_map(|k| {
                std::iter::once(k.pos)
                    .chain(k.pos.neighbors8())
                    .find(|q| city.walkable(street, *q))
            })
            .unwrap();
        let p = st.player.as_mut().unwrap();
        p.pos = beside_jack;
        p.jack_in(&city, 0).unwrap();
        s.update(&mut st, &[]);
        let text = draw_screen(&s, &st, 120, 40);
        // The bolt is a double-width glyph in the buffer: match around it.
        assert!(
            text.contains("STATUS: ⚡") && text.contains("jacked in · Net"),
            "{text}"
        );
        assert!(
            text.contains("· Net") && text.contains("[enter]jack"),
            "{text}"
        );
    }

    #[test]
    fn default_panes_all_draw_at_the_minimum() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        st.say("took it");
        st.tick += 200;
        let text = draw_screen(&s, &st, 100, 30);
        assert!(text.contains("THE DROP"), "{text}");
        assert!(text.contains("· Street "), "minimap coordinate row: {text}");
        assert!(text.contains("took it"), "log pane: {text}");
    }

    #[test]
    fn frame_carries_no_explicit_background() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let buf = t.backend().buffer();
        // Rule rows, the right column and the footer keep the terminal's own background.
        for (x, y) in [
            (0u16, 4u16),
            (50, 4),
            (99, 27),
            (70, 10),
            (95, 6),
            (5, 28),
            (5, 29),
        ] {
            assert_eq!(
                buf[(x, y)].bg,
                Color::Reset,
                "({x},{y}) bg {:?}",
                buf[(x, y)].bg
            );
        }
    }

    #[test]
    fn tiny_frame_falls_back_to_the_notice() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        for (w, h) in [(30u16, 5u16), (41, 8), (40, 30), (100, 7)] {
            let text = draw_screen(&s, &st, w, h);
            assert!(
                text.contains("resize") || (w >= 41 && h >= 8),
                "{w}x{h}: {text}"
            );
        }
    }

    #[test]
    fn squeezed_out_pane_tab_reads_dim() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        for sec in Section::ALL {
            st.sidebar.set(sec, true);
        }
        st.sidebar.set(Section::Log, false);
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let buf = t.backend().buffer();
        // The dev tabs sit on the column's last row: 30 - 4.
        let row = 26u16;
        let cells: Vec<String> = (0..100u16)
            .map(|x| buf[(x, row)].symbol().to_string())
            .collect();
        let x = (0..cells.len() - 7)
            .find(|&i| cells[i..i + 7].concat() == "Legend*")
            .expect("legend tab") as u16;
        assert_eq!(
            buf[(x, row)].fg,
            crate::ui::chrome::DIM,
            "{}",
            cells.concat()
        );
    }

    #[test]
    fn firing_a_slot_leaves_fx_that_prune() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('1'))]);
        assert_eq!(st.fx.len(), 1, "{:?}", st.fx);
        // Mid-wave the frame draws; the kick is live for its first ticks.
        st.tick += 2;
        draw_screen(&s, &st, 100, 30);
        st.tick += 40;
        let text = draw_screen(&s, &st, 100, 30);
        assert!(text.contains("Netscan: pulse sent"), "{text}");
        s.update(&mut st, &[]);
        assert!(st.fx.is_empty());
    }

    #[test]
    fn a_fresh_city_starts_walking_even_in_dev_mode() {
        let mut st = state();
        st.dev = true;
        let mut s = MapScreen::walking(&mut st);
        assert_eq!(
            s.mode,
            Mode::Walk,
            "on foot from construction: no camera frame before the first update"
        );
        let p = st.player.as_ref().unwrap().pos;
        assert_eq!(
            (s.cam.tx, s.cam.ty),
            (p.x, p.y),
            "the camera opens on the walker"
        );
        assert!(
            s.cam_ready,
            "so the first update does not re-snap the camera to the city centre"
        );
        s.update(&mut st, &[]);
        assert_eq!(s.mode, Mode::Walk);
        s.update(&mut st, &[key(KeyCode::Char('c'))]);
        assert_eq!(s.mode, Mode::Explore, "the dev camera is still a key away");
    }

    fn seed_world(st: &mut AppState) {
        let city = st.city.as_ref().unwrap();
        st.world = Some(sim::World::seed(
            city,
            &st.people,
            &sim::SimParams::default(),
            22 * 60,
            st.tick,
        ));
    }

    /// Runs the world an hour, pathing a few trips per frame as the app does.
    fn run_an_hour(st: &mut AppState, s: &mut MapScreen) {
        for _ in 0..240 {
            st.tick += 90 / 4;
            s.update(st, &[]);
        }
    }

    #[test]
    fn people_walk_the_streets_block_the_player_and_talk() {
        let mut st = state();
        seed_world(&mut st);
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        run_an_hour(&mut st, &mut s);
        let city = st.city.as_ref().unwrap();
        let whole = citygen::Rect::new(0, 0, city.size.w as i32, city.size.h as i32);
        let world = st.world.as_ref().unwrap();
        let walker = world
            .visible(whole, st.tick)
            .into_iter()
            .find(|m| matches!(m.kind, sim::MoverKind::Walker { .. }))
            .expect("someone is out");
        let beside = walker
            .pos
            .neighbors4()
            .find(|n| {
                city.walkable(StratumKind::Street, *n) && world.person_at(*n, st.tick).is_none()
            })
            .expect("a free tile beside them");
        {
            let p = st.player.as_mut().unwrap();
            p.pos = beside;
        }
        let bonus = st.character.sight_bonus(&st.content);
        let city = st.city.as_ref().unwrap();
        st.player.as_mut().unwrap().update_fov(city, bonus);
        s.cam.snap(beside);
        s.update(&mut st, &[]);
        let text = draw_screen(&s, &st, 120, 40);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let buf = t.backend().buffer();
        let dim_ats = buf
            .content
            .iter()
            .filter(|c| c.symbol() == "@" && c.fg == crate::ui::chrome::DIM)
            .count();
        assert!(dim_ats >= 1, "a walker draws as a dim @: {text}");
        let bold_white = buf
            .content
            .iter()
            .filter(|c| c.symbol() == "@" && c.modifier.contains(Modifier::BOLD))
            .count();
        assert_eq!(bold_white, 1, "the player stays the one bold @");
        // Walking into them talks; the player does not move.
        let dir = match (walker.pos.x - beside.x, walker.pos.y - beside.y) {
            (1, 0) => KeyCode::Char('l'),
            (-1, 0) => KeyCode::Char('h'),
            (0, 1) => KeyCode::Char('j'),
            _ => KeyCode::Char('k'),
        };
        let cmd = s.update(&mut st, &[key(dir)]);
        let ScreenCmd::Push { to: mut p, .. } = cmd else {
            panic!("a bump talks")
        };
        assert_eq!(st.player.as_ref().unwrap().pos, beside, "blocked");
        st.tick += 40;
        let popup = draw_screen(&*p, &st, 120, 40);
        let sim::MoverKind::Walker { id, .. } = walker.kind else {
            unreachable!()
        };
        let card = st
            .world
            .as_ref()
            .unwrap()
            .card(id, st.city.as_ref().unwrap());
        assert!(
            popup.contains(&card.name.to_uppercase()) && popup.contains("t  talk"),
            "{popup}"
        );
        p.update(&mut st, &[key(KeyCode::Char('t'))]);
        st.tick += 40;
        let popup = draw_screen(&*p, &st, 120, 40);
        assert!(
            popup.contains(&card.profession) && popup.contains(&card.line),
            "{popup}"
        );
        // Space lists them as well.
        let ScreenCmd::Push { to: mut here, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens HERE")
        };
        st.tick += 40;
        let text = draw_screen(&*here, &st, 120, 40);
        assert!(text.contains(&format!("talk to {}", card.name)), "{text}");
        here.update(&mut st, &[key(KeyCode::Esc)]);
    }

    #[test]
    fn nobody_stands_on_the_player() {
        let mut st = state();
        seed_world(&mut st);
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        run_an_hour(&mut st, &mut s);
        let city = st.city.as_ref().unwrap();
        let whole = citygen::Rect::new(0, 0, city.size.w as i32, city.size.h as i32);
        let walker = st
            .world
            .as_ref()
            .unwrap()
            .visible(whole, st.tick)
            .into_iter()
            .find(|m| matches!(m.kind, sim::MoverKind::Walker { .. }))
            .expect("someone is out");
        st.player.as_mut().unwrap().pos = walker.pos;
        s.update(&mut st, &[]);
        let pos = st.player.as_ref().unwrap().pos;
        let world = st.world.as_ref().unwrap();
        for _ in 0..3 {
            assert!(
                world.person_at(pos, st.tick).is_none(),
                "held a tile back while the player stands there"
            );
        }
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.contains('@'));
    }
}
