//! The map screen: free-camera Explore mode and the Walk mode with the `@`.

use crate::actions::{perform, report};
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::messages::Kind;
use crate::render::fx::FxOverlay;
use crate::render::items::GroundOverlay;
use crate::render::map::{MapRef, TileMap};
use crate::render::overlay::Digital;
use crate::render::people::{InteriorPeople, PeopleOverlay, card_lines};
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
use citygen::{Dir, Place, Pos, Rect as TileRect, StratumKind};
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
    /// The map on screen: a stratum, or the floor the player walks.
    pub place: Place,
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
    /// Where the occupants of the floor the player walks stand, keyed by the place
    /// and the world's minute it was worked out for.
    placement: Option<(Place, u64, crate::interiors::Placement)>,
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
            place: Place::Stratum(StratumKind::Street),
            cam: Camera::default(),
            cursor: Pos::new(0, 0),
            digital: false,
            help: false,
            fps: FpsMeter::default(),
            console: None,
            last_tick: 0,
            cam_ready: false,
            last_map: Cell::new(Rect::new(0, 0, 100, 28)),
            placement: None,
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
        let Place::Stratum(current) = self.place else {
            return;
        };
        let max = st
            .city
            .as_ref()
            .map(|c| c.strata.len() as i32 - 1)
            .unwrap_or(3);
        let i = (current.index() as i32 + delta).clamp(0, max);
        if let Some(k) = StratumKind::from_index(i as usize) {
            self.place = Place::Stratum(k);
        }
    }

    /// The map at the screen's place; a floor not yet generated falls back to the street.
    fn map<'a>(&self, st: &'a AppState, city: &'a citygen::City) -> MapRef<'a> {
        MapRef::for_place(city, &st.interiors, &st.constructs, self.place)
            .unwrap_or_else(|| MapRef::stratum_of(city, StratumKind::Street))
    }

    /// Works out where the occupants stand when the player is on a floor; once per
    /// place and world minute.
    fn refresh_placement(&mut self, st: &AppState) {
        let Some(p) = st.player.as_ref() else {
            self.placement = None;
            return;
        };
        let Place::Floor { building, level } = p.place else {
            self.placement = None;
            return;
        };
        let Some(world) = st.world.as_ref() else {
            return;
        };
        let minute = world.tick() / sim::TICKS_PER_MINUTE;
        if self
            .placement
            .as_ref()
            .is_some_and(|(pl, m, _)| *pl == p.place && *m == minute)
        {
            return;
        }
        let Some((plan, floor)) = st.interiors.floor(building, level) else {
            return;
        };
        self.placement = Some((
            p.place,
            minute,
            crate::interiors::Placement::new(plan, floor, world),
        ));
    }

    /// Whoever stands on a tile of the floor the player walks.
    fn inside_person_at(&self, place: Place, pos: Pos) -> Option<sim::PersonId> {
        self.placement
            .as_ref()
            .filter(|(pl, _, _)| *pl == place)
            .and_then(|(_, _, p)| p.person_at(pos))
    }

    /// What the status line calls the map.
    fn place_label(&self, st: &AppState) -> String {
        match (self.place, st.city.as_ref()) {
            (Place::Stratum(k), _) => k.label().to_string(),
            (Place::Floor { level, .. }, _) => citygen::FloorPlan::label(level),
            (Place::Construct(_), _) => "construct".to_string(),
        }
    }

    fn enter_walk(&mut self, st: &mut AppState) {
        let bonus = st.character.sight_bonus(&st.content);
        let Some(city) = st.city.as_ref() else { return };
        if st.player.is_none() {
            let mut p = Player::spawn(city, hash64(city.seed, st.tick));
            p.update_fov(&MapRef::stratum_of(city, StratumKind::Street), bonus);
            st.player = Some(p);
        }
        if let Some(p) = &st.player {
            self.place = p.place;
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
                            .focus(self.place, self.focus_pos(st))
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
        let (place, pos) = (player.place, player.pos);
        let k = place.stratum();
        let map = self.map(st, city);
        let mut choices = Vec::new();
        // Letters for take rows; j k q h l t and the arrows are taken by the popup and verbs.
        let mut letters = WORLD_LETTERS.chars();
        let stacks = st.ground.at(place, pos);
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
        if let Some(k) = k {
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
        // Inside: the lift panel on a lift bank, the next floors on the stairs.
        if let (Place::Floor { building, level }, Some((plan, floor))) = (
            place,
            place
                .stratum()
                .is_none()
                .then(|| match place {
                    Place::Floor { building, level } => st.interiors.floor(building, level),
                    _ => None,
                })
                .flatten(),
        ) {
            match floor.tiles.get(pos).map(|t| t.terrain) {
                Some(citygen::Terrain::Lift) => {
                    if let Some(rows) = self.floor_rows(st) {
                        choices.extend(rows);
                    }
                }
                Some(citygen::Terrain::Stair) => {
                    if level < plan.top {
                        choices.push(Choice::new(
                            '>',
                            format!("up to {}", citygen::FloorPlan::label(level + 1)),
                            Pick::World(WorldAct::Climb(true)),
                        ));
                    }
                    if level > plan.lowest {
                        choices.push(Choice::new(
                            '<',
                            format!("down to {}", citygen::FloorPlan::label(level - 1)),
                            Pick::World(WorldAct::Climb(false)),
                        ));
                    }
                }
                _ => {}
            }
            let _ = building;
        }
        if place.is_street()
            && let Some(world) = st.world.as_ref()
        {
            for n in pos.neighbors4() {
                let Some(id) = world.person_at(n) else {
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
        if !place.is_street()
            && let Some(world) = st.world.as_ref()
        {
            for n in pos.neighbors4() {
                let Some(id) = self.inside_person_at(place, n) else {
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
        // Inside a construct: the run in flight first, then the programs that apply
        // to the node or the ICE at hand.
        let mut title = "HERE";
        if let (Place::Construct(_), Some(run)) = (place, st.net.run.as_ref()) {
            let label = st
                .content
                .item(&run.program)
                .and_then(|i| i.program)
                .map(|p| p.op.label())
                .unwrap_or("run");
            title = "RUN";
            choices.push(Choice::new(
                'c',
                format!("continue · {label} {}/{}", run.done, run.work),
                Pick::Continue,
            ));
        }
        if let Place::Construct(b) = place
            && st.character.deck(&st.content).is_some()
        {
            let now = st.net.now(st);
            let security = st.constructs.get(b).map(|k| k.security).unwrap_or(0);
            let targets = crate::net::targets_near(st, b, pos);
            for (target, what) in targets {
                let Some(kind) = crate::net::target_kind(st, b, target) else {
                    continue;
                };
                for item in st.character.loaded_programs(&st.content) {
                    let Some(spec) = item.program else { continue };
                    if !game::applies(spec.op, kind) {
                        continue;
                    }
                    let Some(key) = letters.next() else { break };
                    title = "RUN";
                    let label = spec.op.label();
                    let copied = spec.op == game::Op::Copy
                        && matches!(target, game::HackTarget::Node(id) if st.net.copied(id, now));
                    let on = format!(" · {what}");
                    if copied {
                        choices.push(Choice::disabled(
                            key,
                            format!("{label}{on} · copied already"),
                        ));
                        continue;
                    }
                    let pw = game::power(&st.character, &st.content, &spec, kind);
                    // A program on yourself never fails.
                    let odds = if kind == game::Target::Deck {
                        100
                    } else {
                        game::chance(pw, game::difficulty(security))
                    };
                    choices.push(Choice::new(
                        key,
                        format!(
                            "{label}{on} · {odds}% · {} turn{} · noise {}",
                            spec.work,
                            if spec.work == 1 { "" } else { "s" },
                            spec.noise
                        ),
                        Pick::Run(item.id.clone(), target),
                    ));
                }
            }
        }
        let node = match place {
            Place::Construct(b) => st
                .constructs
                .get(b)
                .and_then(|k| k.node_at(pos))
                .map(|id| city.net.node(id)),
            _ => k
                .and_then(|k| Digital::node_under(city, k, pos))
                .map(|id| city.net.node(id)),
        };
        let person = st
            .world
            .as_ref()
            .filter(|_| place.is_street())
            .and_then(|w| w.person_at(pos).map(|id| w.card(id, city)));
        choices.push(Choice::new(
            'l',
            "look",
            Pick::Card(inspector_lines(
                &map,
                pos,
                node,
                stacks,
                &st.content,
                person.as_ref(),
            )),
        ));
        Some(ScreenCmd::Push {
            to: Box::new(Popup::verbs(title, choices, st.tick)),
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

    /// A step into someone swaps places with them, and they drift back once the
    /// player moves on; a step into a car goes nowhere.
    /// `Err` blocks the step; `Ok(true)` swapped places with someone (the step goes
    /// on, but not through a door); `Ok(false)` found nobody.
    fn bump(&self, st: &mut AppState, dir: Dir) -> Result<bool, ScreenCmd> {
        let Some(player) = st.player.as_ref() else {
            return Ok(false);
        };
        let (from, target, place) = (player.pos, player.pos.step(dir), player.place);
        if !place.is_street() {
            // Indoors people stand at their stations and are not swapped around.
            return if self.inside_person_at(place, target).is_some() {
                Err(ScreenCmd::Stay)
            } else {
                Ok(false)
            };
        }
        let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) else {
            return Ok(false);
        };
        if let Some(id) = world.person_at(target) {
            if city.walkable(StratumKind::Street, target) {
                world.swap(id, from, target);
                return Ok(true);
            }
            return Ok(false);
        }
        if world.car_at(target).is_some() {
            st.warn("a car is in the way");
            return Err(ScreenCmd::Stay);
        }
        Ok(false)
    }

    /// A step onto a closed building's door goes inside; onto an exit, back out; onto
    /// a locked door, nowhere.
    fn doorway(&mut self, st: &mut AppState, dir: Dir) -> Option<ScreenCmd> {
        let player = st.player.as_ref()?;
        let (target, place) = (player.pos.step(dir), player.place);
        let city = st.city.as_ref()?;
        match place {
            Place::Stratum(k) => {
                let here = player.pos;
                let t = city.tile(k, target)?;
                // Onto the door from outside, or from the door into the building's bulk.
                let (b, door) = match t.terrain {
                    citygen::Terrain::Door => (t.building?, target),
                    citygen::Terrain::Mass => {
                        let b = t.building?;
                        let h = city.tile(k, here)?;
                        if h.terrain != citygen::Terrain::Door || h.building != Some(b) {
                            return None;
                        }
                        (b, here)
                    }
                    _ => return None,
                };
                if !city.building(b).kind.is_closed() || !city.portals(b).contains(&(k, door)) {
                    return None;
                }
                let result = crate::interiors::enter(st, b, (k, door));
                report(st, result);
                Some(ScreenCmd::Stay)
            }
            Place::Floor { building, level } => {
                let (_, floor) = st.interiors.floor(building, level)?;
                if let Some(exit) = floor.exit_at(target).copied() {
                    let result = crate::interiors::leave(st, &exit);
                    report(st, result);
                    return Some(ScreenCmd::Stay);
                }
                let owner = self
                    .placement
                    .as_ref()
                    .filter(|(pl, _, _)| *pl == place)
                    .and_then(|(_, _, p)| p.owner_of(target))
                    .and_then(|id| {
                        st.world
                            .as_ref()
                            .map(|w| w.people()[id.0 as usize].name.clone())
                    });
                let here = player.pos;
                let open = st.net.unlocked(building, st.net.now(st));
                let locked = floor
                    .door_of(target)
                    .filter(|r| r.private && !r.rect.contains(here) && !open)
                    .map(|r| {
                        let mut what = r.kind.label().to_string();
                        if r.number > 0 {
                            what = format!("{what} {}", r.number);
                        }
                        match &owner {
                            Some(name) => format!("{name}'s {what}"),
                            None => what,
                        }
                    });
                if let Some(what) = locked {
                    st.warn(format!("locked · {what}"));
                    return Some(ScreenCmd::Stay);
                }
                None
            }
            // The gate in and out of a construct comes with the construct cache.
            Place::Construct(_) => None,
        }
    }

    /// A construct's gate on the Net, stepped onto from the open grid, leads inside; the
    /// entry inside, stepped off outward, leads back onto the grid.
    fn gateway(&mut self, st: &mut AppState, dir: Dir) -> Option<ScreenCmd> {
        let player = st.player.as_ref()?;
        let (here, target) = (player.pos, player.pos.step(dir));
        let city = st.city.as_ref()?;
        match player.place {
            Place::Stratum(StratumKind::Net) => {
                // Onto the gate from the grid, or from the gate into the construct's bulk.
                let gate_of = |p: Pos| {
                    city.tile(StratumKind::Net, p)
                        .filter(|t| t.terrain == citygen::Terrain::NetGate)
                        .and_then(|t| t.building)
                };
                let fp = |b: citygen::BuildingId| city.building_opt(b).map(|x| x.footprint);
                let b = match (gate_of(target), gate_of(here)) {
                    (Some(b), _) if fp(b).is_some_and(|f| !f.contains(here)) => b,
                    (_, Some(b)) if fp(b).is_some_and(|f| f.contains(target)) => b,
                    _ => return None,
                };
                let result = crate::constructs::enter_construct(st, b);
                report(st, result);
                self.follow_player(st);
                Some(ScreenCmd::Stay)
            }
            Place::Construct(b) => {
                let k = st.constructs.get(b)?;
                if here != k.entry || k.tiles.in_bounds(target) {
                    return None;
                }
                let result = crate::constructs::leave_construct(st);
                report(st, result);
                self.follow_player(st);
                Some(ScreenCmd::Stay)
            }
            _ => None,
        }
    }

    /// The screen's place and camera catch up with the player after a jump.
    fn follow_player(&mut self, st: &AppState) {
        if let Some(p) = st.player.as_ref() {
            self.place = p.place;
            self.jump_to(st, p.pos);
        }
    }

    /// Puts the camera on a tile of the current place at once: snapped, clamped to
    /// that place's map (a small map sits centred), and settled, so a new map never
    /// slides into view.
    fn jump_to(&mut self, st: &AppState, pos: Pos) {
        self.cam.snap(pos);
        if let Some(city) = st.city.as_ref() {
            let view = self.last_map.get();
            if view.width > 0 && view.height > 0 {
                let size = TileMap::size(&self.map(st, city));
                self.cam.clamp(size, view.width, view.height);
                self.cam.settle();
            }
        }
    }

    /// `>` and `<` inside: the lift panel on a lift, a floor on the stairs, nothing else.
    fn vertical(&mut self, st: &mut AppState, up: bool) -> Option<ScreenCmd> {
        let player = st.player.as_ref()?;
        let Place::Floor { building, level } = player.place else {
            return None;
        };
        let (_, floor) = st.interiors.floor(building, level)?;
        match floor.tiles.get(player.pos)?.terrain {
            citygen::Terrain::Lift => self.floors_popup(st),
            _ => {
                let result = crate::interiors::climb(st, up);
                report(st, result);
                Some(ScreenCmd::Stay)
            }
        }
    }

    /// The FLOORS popup: every floor of the building, the current one marked.
    fn floors_popup(&self, st: &AppState) -> Option<ScreenCmd> {
        let rows = self.floor_rows(st)?;
        Some(ScreenCmd::Push {
            to: Box::new(Popup::verbs("FLOORS", rows, st.tick)),
            fx: TransitionKind::None,
        })
    }

    fn floor_rows(&self, st: &AppState) -> Option<Vec<Choice>> {
        let player = st.player.as_ref()?;
        let Place::Floor { building, level } = player.place else {
            return None;
        };
        let plan = st.interiors.plan_of(building)?;
        let mut letters = WORLD_LETTERS.chars();
        Some(
            crate::interiors::floor_rows(plan, level)
                .into_iter()
                .rev()
                .map(|(l, label)| {
                    let key = letters.next().unwrap_or(' ');
                    let mut c = Choice::new(key, label, Pick::World(WorldAct::Lift(l)));
                    c.enabled = l != level;
                    c
                })
                .collect(),
        )
    }

    fn handle_walk(&mut self, st: &mut AppState, k: &KeyEvent) -> Option<ScreenCmd> {
        let before = st.player.as_ref().map(|p| (p.place, p.pos));
        let cmd = self.handle_walk_inner(st, k);
        let after = st.player.as_ref().map(|p| (p.place, p.pos));
        // A turn in the Net: a wait steps the program in flight, a move aborts it, and
        // either lets awake ICE beside you have its say.
        if after != before && st.net.run.take().is_some() {
            st.say("run aborted");
        }
        if k.code == KeyCode::Char('.') {
            crate::net::step_run(st);
        }
        if k.code == KeyCode::Char('.') || after != before {
            crate::net::ice_turn(st);
        }
        if st.player.as_ref().map(|p| p.place) != before.map(|b| b.0) {
            self.follow_player(st);
        }
        cmd
    }

    fn handle_walk_inner(&mut self, st: &mut AppState, k: &KeyEvent) -> Option<ScreenCmd> {
        if k.code == KeyCode::Char(' ') {
            return self.interact(st);
        }
        if k.code == KeyCode::Enter
            && st.player.as_ref().is_some_and(|p| !p.jacked_in())
            && let Some(why) = crate::net::jack_in_refusal(st)
        {
            st.warn(why);
            return Some(ScreenCmd::Stay);
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
        if let Some(dir) = dir_for(k.code) {
            if let Some(cmd) = self.gateway(st, dir) {
                return Some(cmd);
            }
            match self.bump(st, dir) {
                Err(cmd) => return Some(cmd),
                Ok(true) => {}
                Ok(false) => {
                    if let Some(cmd) = self.doorway(st, dir) {
                        return Some(cmd);
                    }
                }
            }
        }
        if matches!(k.code, KeyCode::Char('<') | KeyCode::Char('>'))
            && let Some(cmd) = self.vertical(st, k.code == KeyCode::Char('>'))
        {
            return Some(cmd);
        }
        let dev = st.dev;
        let bonus = st.character.sight_bonus(&st.content);
        let city = st.city.as_ref()?;
        let interiors = &st.interiors;
        let constructs = &st.constructs;
        let player = st.player.as_mut()?;
        let map = MapRef::for_place(city, interiors, constructs, player.place)
            .unwrap_or_else(|| MapRef::stratum_of(city, StratumKind::Street));
        match k.code {
            KeyCode::Char('c') if dev => {
                self.cursor = player.pos;
                self.mode = Mode::Explore;
            }
            KeyCode::Enter => {
                if player.jacked_in() {
                    match player.jack_out(city, bonus) {
                        Ok(()) => {
                            self.place = player.place;
                            self.cam.snap(player.pos);
                            st.say("jacked out");
                            return Some(ScreenCmd::Flash(TransitionKind::Glitch));
                        }
                        Err(e) => st.warn(e),
                    }
                } else {
                    match player.jack_in(city, bonus) {
                        Ok(port) => {
                            self.place = Place::Stratum(StratumKind::Net);
                            self.cam.snap(port);
                            let where_ = port_name(city, port);
                            st.say(format!("jacked in at `{where_}`"));
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
                        self.place = Place::Stratum(dest);
                        player.update_fov(&MapRef::stratum_of(city, dest), bonus);
                        st.say(format!("now on `{}`", dest.label()));
                    }
                    None => st.warn(if up {
                        "nothing leads up from here"
                    } else {
                        "nothing leads down from here"
                    }),
                }
            }
            KeyCode::Char('.') => player.update_fov(&map, bonus),
            code => {
                let dir = dir_for(code)?;
                if player.try_move(&map, dir) {
                    player.update_fov(&map, bonus);
                }
            }
        }
        Some(ScreenCmd::Stay)
    }
}

impl Screen for MapScreen {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        // Black ICE killed: the screen reads the cause.
        if let Some(cause) = st.net.death.clone() {
            return ScreenCmd::Switch {
                to: Box::new(crate::screens::flatlined::Flatlined::new(
                    cause,
                    &st.character.handle,
                    st.character.level,
                    st.tick,
                )),
                fx: TransitionKind::Glitch,
            };
        }
        if st.tick > self.last_tick + 1 {
            self.fps.resume(Instant::now());
        }
        // The world runs only while walking, by the frames since the last update, a
        // minute at most: overview and menus freeze it.
        let elapsed = st
            .tick
            .saturating_sub(self.last_tick)
            .min(sim::TICKS_PER_MINUTE);
        self.last_tick = st.tick;
        self.fps.frame();
        st.prune_fx(st.tick);
        // The map follows the player: a popup may have jacked them or taken the stairs.
        if self.mode == Mode::Walk
            && let Some(p) = st.player.as_ref()
            && p.place != self.place
        {
            let pos = p.pos;
            self.place = p.place;
            self.jump_to(st, pos);
        }
        // The console takes every key while it is open.
        let focus = game::At {
            place: self.place,
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
        // A key may have taken the player through a door, up a lift or out again: the
        // map follows before this frame draws.
        if self.mode == Mode::Walk
            && let Some(p) = st.player.as_ref()
            && p.place != self.place
        {
            let pos = p.pos;
            self.place = p.place;
            self.jump_to(st, pos);
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
        // The camera is bound by the map it shows, not the city: a floor or a construct
        // smaller than the view sits centred.
        let bounds = st
            .city
            .as_ref()
            .map(|c| TileMap::size(&self.map(st, c)))
            .unwrap_or(size);
        self.cam.clamp(bounds, map.width, map.height);
        self.cam.update();
        // The city lives: a minute step when the clock turns, a few paths per frame,
        // and nobody walks onto the player.
        let standing = match (self.mode, &st.player) {
            (Mode::Walk, Some(p)) if p.place.is_street() => Some(p.pos),
            _ => None,
        };
        if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
            world.set_blocker(standing);
            if self.mode == Mode::Walk {
                world.advance(city, world.tick() + elapsed);
                world.path_some(city, 4);
            }
        }
        // The Net's clock: traces cool, hacks expire, alerts end.
        let now = st.net.now(st);
        let inside = match st.player.as_ref().map(|p| p.place) {
            Some(Place::Construct(b)) => Some(b),
            _ => None,
        };
        st.net.minute(now, inside, &mut st.constructs);
        self.refresh_placement(st);
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
        let map_area = r.map;
        self.last_map.set(map_area);
        let origin = self.cam.origin(map_area.width, map_area.height);
        let cursor = self.focus_pos(st);
        let map = self.map(st, city);
        let focus = match self.place.stratum() {
            Some(k) if self.digital || k == StratumKind::Net => {
                Digital::node_under(city, k, cursor)
            }
            _ => None,
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
        let walking =
            self.mode == Mode::Walk && st.player.as_ref().is_some_and(|p| p.place == self.place);
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
        let (hour, _) = st.clock();
        let people = st.world.as_ref().map(|world| PeopleOverlay {
            world,
            night: hour >= 18 || hour <= 5,
            vis,
        });
        if let Some(p) = people.as_ref() {
            overlays.push(p);
        }
        let inside = match (&self.placement, st.world.as_ref()) {
            (Some((pl, _, placement)), Some(world)) if *pl == self.place => Some(InteriorPeople {
                placement,
                world,
                vis,
            }),
            _ => None,
        };
        if let Some(o) = inside.as_ref() {
            overlays.push(o);
        }
        let fx = FxOverlay { fx: &st.fx, vis };
        overlays.push(&fx);
        MapView {
            map: &map,
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
        .render(map_area, f.buffer_mut());

        if let Some(p) = &st.player {
            if let Some((bk, bp)) = p.body
                && bk == self.place
                && let Some((sx, sy)) = tile_to_screen(map_area, origin, bp)
            {
                f.buffer_mut()[(sx, sy)].set_char('@').set_style(
                    Style::default()
                        .fg(Color::Rgb(110, 120, 130))
                        .add_modifier(Modifier::DIM),
                );
            }
            if p.place == self.place
                && let Some((sx, sy)) = tile_to_screen(map_area, origin, p.pos)
            {
                let fg = if p.place == Place::Stratum(StratumKind::Net) {
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
            hud::bar_row("FOCUS", ch.focus, ch.focus_max(&st.content), CYAN, lw),
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
        let mut location = hud::location(city, &st.interiors, self.place, cursor);
        let alert_building = match self.place {
            Place::Construct(b) | Place::Floor { building: b, .. } => Some(b),
            Place::Stratum(_) => None,
        };
        if alert_building.is_some_and(|b| st.net.alert(b, st.net.now(st))) {
            location = format!("ALERT · {location}");
        }
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
            .filter(|_| self.place.is_street())
            .and_then(|w| w.person_at(cursor).map(|id| w.card(id, city)))
            .or_else(|| {
                let id = self.inside_person_at(self.place, cursor)?;
                st.world.as_ref().map(|w| w.card(id, city))
            });
        let view = sidebar::SidebarView {
            map,
            focus: cursor,
            person: person.as_ref(),
            view: TileRect::new(
                origin.x,
                origin.y,
                map_area.width as i32,
                map_area.height as i32,
            ),
            player: st
                .player
                .as_ref()
                .filter(|p| p.place == self.place)
                .map(|p| p.pos),
            style: &st.style,
            content: &st.content,
            character: &st.character,
            ground: &st.ground,
            log: &st.log,
            tick: st.tick,
            dev: st.dev,
            net: &st.net,
            now: st.net.now(st),
            ice_lines: match (self.place, st.player.as_ref()) {
                (Place::Construct(b), Some(p)) if p.place == self.place => {
                    let vis = p.vis();
                    crate::net::ice_lines(st, b, |q| {
                        vis.get(q)
                            .is_some_and(|v| *v != crate::render::Visibility::Unseen)
                    })
                }
                _ => Vec::new(),
            },
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
                format!("on foot · {}", self.place_label(st)),
                Style::default().fg(TEXT),
            )],
            (Mode::Explore, _) => vec![Span::styled(
                format!("camera · {}", self.place_label(st)),
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
            Mode::Walk if st.net.run.is_some() => hud::RUN_VERBS,
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
                map_area.x + 1,
                map_area.bottom().saturating_sub(1),
                &e.marked(),
                style,
                map_area.right(),
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
    use crate::walker::Visibility;
    use citygen::{BuildingKind, FloorPlan, IceGrade, NodeKind};
    use game::ItemId;

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
        assert_eq!(s.place, Place::Stratum(StratumKind::Skyways));
        s.update(&mut st, &[key(KeyCode::Char('['))]);
        s.update(&mut st, &[key(KeyCode::Char('['))]);
        assert_eq!(s.place, Place::Stratum(StratumKind::Undercity));
        s.update(&mut st, &[key(KeyCode::Char('['))]);
        assert_eq!(s.place, Place::Stratum(StratumKind::Undercity));
        s.update(&mut st, &[key(KeyCode::Char('w'))]);
        assert!(st.player.is_some());
        assert_eq!(s.mode, Mode::Walk);
        assert_eq!(s.place, Place::Stratum(StratumKind::Street));
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
        s.place = Place::Stratum(StratumKind::Undercity);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('s'))]),
            ScreenCmd::Push { .. }
        ));
        assert_eq!(
            (s.mode, s.place),
            (Mode::Walk, Place::Stratum(StratumKind::Undercity))
        );
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
        assert_eq!(s.place, Place::Stratum(StratumKind::Net));
        s.update(&mut st, &[key(KeyCode::Char(']'))]);
        assert_eq!(s.place, Place::Stratum(StratumKind::Net));
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
            p.place = Place::Stratum(StratumKind::Street);
        }
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Flash(TransitionKind::Glitch)
        ));
        let p = st.player.as_ref().unwrap();
        assert!(p.jacked_in());
        assert_eq!(s.place, Place::Stratum(StratumKind::Net));
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
        s.place = Place::Stratum(StratumKind::Street);
        s.cursor = stand;
        s.cam.snap(stand);
        t.draw(|f| s.draw(&st, f)).unwrap();
        let map = s.last_map.get();
        let origin = s.cam.origin(map.width, map.height);
        let (sx, sy) = tile_to_screen(map, origin, stand).unwrap();
        assert_eq!(t.backend().buffer()[(sx, sy)].symbol(), "@");
        s.mode = Mode::Walk;
        s.place = Place::Stratum(StratumKind::Net);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Flash(TransitionKind::Glitch)
        ));
        let p = st.player.as_ref().unwrap();
        assert!(!p.jacked_in());
        assert_eq!(
            (p.place, p.pos),
            (Place::Stratum(StratumKind::Street), stand)
        );
        assert_eq!(s.place, Place::Stratum(StratumKind::Street));
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
            (p.place, p.pos)
        };
        let walkable: Vec<Pos> = {
            let city = st.city.as_ref().unwrap();
            pos.neighbors8()
                .filter(|q| city.walkable(k.stratum().unwrap(), *q))
                .collect()
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
            (p.place, p.pos)
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
        assert_eq!(
            s.place,
            Place::Stratum(StratumKind::Net),
            "the map follows the player"
        );
        let ScreenCmd::Push { to: mut p, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!()
        };
        st.tick += 40;
        assert!(draw_screen(&*p, &st, 120, 40).contains("j  jack out"));
        p.update(&mut st, &[key(KeyCode::Char('j'))]);
        s.update(&mut st, &[]);
        assert_eq!(s.place, Place::Stratum(street));
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
        assert_eq!(s.place, Place::Stratum(StratumKind::Undercity));
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
    fn walking_into_someone_swaps_places_and_space_talks() {
        let mut st = state();
        seed_world(&mut st);
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        run_an_hour(&mut st, &mut s);
        let city = st.city.as_ref().unwrap();
        let whole = citygen::Rect::new(0, 0, city.size.w as i32, city.size.h as i32);
        let world = st.world.as_ref().unwrap();
        let walker = world
            .visible(whole)
            .into_iter()
            .find(|m| matches!(m.kind, sim::MoverKind::Walker { .. }))
            .expect("someone is out");
        let beside = walker
            .pos
            .neighbors4()
            .find(|n| city.walkable(StratumKind::Street, *n) && world.person_at(*n).is_none())
            .expect("a free tile beside them");
        {
            let p = st.player.as_mut().unwrap();
            p.pos = beside;
        }
        let bonus = st.character.sight_bonus(&st.content);
        let city = st.city.as_ref().unwrap();
        st.player
            .as_mut()
            .unwrap()
            .update_fov(&MapRef::stratum_of(city, StratumKind::Street), bonus);
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
        // Walking into them swaps places: no prompt, the player takes their tile and
        // they take the player's.
        let dir = match (walker.pos.x - beside.x, walker.pos.y - beside.y) {
            (1, 0) => KeyCode::Char('l'),
            (-1, 0) => KeyCode::Char('h'),
            (0, 1) => KeyCode::Char('j'),
            _ => KeyCode::Char('k'),
        };
        let cmd = s.update(&mut st, &[key(dir)]);
        assert!(
            matches!(cmd, ScreenCmd::Stay),
            "a bump swaps, it does not prompt"
        );
        assert_eq!(
            st.player.as_ref().unwrap().pos,
            walker.pos,
            "the player took their tile"
        );
        let sim::MoverKind::Walker { id, .. } = walker.kind else {
            unreachable!()
        };
        let world = st.world.as_ref().unwrap();
        assert_eq!(
            world.person_at(beside),
            Some(id),
            "they took the player's old tile"
        );
        assert_eq!(world.person_at(walker.pos), None);
        let card = world.card(id, st.city.as_ref().unwrap());
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
            .visible(whole)
            .into_iter()
            .find(|m| matches!(m.kind, sim::MoverKind::Walker { .. }))
            .expect("someone is out");
        st.player.as_mut().unwrap().pos = walker.pos;
        s.update(&mut st, &[]);
        let pos = st.player.as_ref().unwrap().pos;
        for _ in 0..3 {
            st.tick += 9;
            s.update(&mut st, &[]);
            assert!(
                st.world.as_ref().unwrap().person_at(pos).is_none(),
                "held a tile back while the player stands there"
            );
        }
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.contains('@'));
    }

    #[test]
    fn street_people_do_not_block_or_talk_on_the_undercity() {
        let mut st = state();
        seed_world(&mut st);
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let under = StratumKind::Undercity;
        let mut found = None;
        for _ in 0..8 {
            run_an_hour(&mut st, &mut s);
            let city = st.city.as_ref().unwrap();
            let whole = citygen::Rect::new(0, 0, city.size.w as i32, city.size.h as i32);
            found = st
                .world
                .as_ref()
                .unwrap()
                .visible(whole)
                .into_iter()
                .filter(|m| matches!(m.kind, sim::MoverKind::Walker { .. }))
                .find_map(|m| {
                    if !city.walkable(under, m.pos) {
                        return None;
                    }
                    m.pos
                        .neighbors4()
                        .find(|n| city.walkable(under, *n))
                        .map(|n| (m.pos, n))
                });
            if found.is_some() {
                break;
            }
        }
        let (person, beside) = found.expect("a tunnel under someone");
        {
            let p = st.player.as_mut().unwrap();
            p.place = Place::Stratum(under);
            p.pos = beside;
        }
        s.place = Place::Stratum(under);
        s.update(&mut st, &[]);
        let dir = match (person.x - beside.x, person.y - beside.y) {
            (1, 0) => KeyCode::Char('l'),
            (-1, 0) => KeyCode::Char('h'),
            (0, 1) => KeyCode::Char('j'),
            _ => KeyCode::Char('k'),
        };
        let ScreenCmd::Push { to: mut here, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens HERE")
        };
        st.tick += 40;
        let text = draw_screen(&*here, &st, 120, 40);
        assert!(
            !text.contains("talk to"),
            "nobody to talk to down here: {text}"
        );
        here.update(&mut st, &[key(KeyCode::Esc)]);
        let cmd = s.update(&mut st, &[key(dir)]);
        assert!(matches!(cmd, ScreenCmd::Stay));
        assert_eq!(
            st.player.as_ref().unwrap().pos,
            person,
            "nobody upstairs blocks the tunnel"
        );
    }

    // ---- interiors ---------------------------------------------------------

    fn state8() -> AppState {
        let mut st = AppState::new();
        st.city = Some(
            Generator::run(GenParams {
                seed: 6,
                size: SizePreset::Custom { w: 120, h: 60 },
                districts: 8,
                ..Default::default()
            })
            .unwrap(),
        );
        st
    }

    fn dir_between(from: Pos, to: Pos) -> KeyCode {
        match (to.x - from.x, to.y - from.y) {
            (1, 0) => KeyCode::Char('l'),
            (-1, 0) => KeyCode::Char('h'),
            (0, 1) => KeyCode::Char('j'),
            (0, -1) => KeyCode::Char('k'),
            (1, -1) => KeyCode::Char('u'),
            (-1, -1) => KeyCode::Char('y'),
            (1, 1) => KeyCode::Char('n'),
            _ => KeyCode::Char('b'),
        }
    }

    fn stand(st: &mut AppState, s: &mut MapScreen, place: Place, pos: Pos) {
        {
            let p = st.player.as_mut().unwrap();
            p.place = place;
            p.pos = pos;
        }
        s.place = place;
        s.cam.snap(pos);
        // A wait refreshes sight on the new map.
        s.update(st, &[key(KeyCode::Char('.'))]);
    }

    fn player(st: &AppState) -> (Place, Pos) {
        let p = st.player.as_ref().unwrap();
        (p.place, p.pos)
    }

    fn tower(st: &AppState) -> citygen::BuildingId {
        st.city
            .as_ref()
            .unwrap()
            .buildings
            .iter()
            .find(|b| b.kind == BuildingKind::CorpTower)
            .unwrap()
            .id
    }

    #[test]
    fn a_street_door_enters_the_lobby_and_its_door_leaves_again() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let b = tower(&st);
        let (door, outside, name) = {
            let city = st.city.as_ref().unwrap();
            let fp = city.building(b).footprint;
            let (_, door) = city
                .portals(b)
                .into_iter()
                .find(|(k, _)| *k == StratumKind::Street)
                .unwrap();
            let outside = door
                .neighbors4()
                .find(|n| !fp.contains(*n) && city.walkable(StratumKind::Street, *n))
                .unwrap();
            (door, outside, city.building(b).name.clone().unwrap())
        };
        stand(
            &mut st,
            &mut s,
            Place::Stratum(StratumKind::Street),
            outside,
        );
        let cmd = s.update(&mut st, &[key(dir_between(outside, door))]);
        assert!(matches!(cmd, ScreenCmd::Stay));
        let (place, pos) = player(&st);
        assert_eq!(
            place,
            Place::Floor {
                building: b,
                level: 0
            }
        );
        let (_, lobby) = st.interiors.floor(b, 0).expect("the lobby was generated");
        let exit = lobby.exits.iter().find(|e| e.door == door).unwrap();
        assert_eq!(pos, lobby.doorstep(exit));
        assert!(last(&st).unwrap().contains(&name), "{:?}", last(&st));
        assert_eq!(s.place, place, "the map follows");
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.contains(&format!("{name} · L")), "{text}");
        assert!(text.contains("on foot · L"), "{text}");
        // Back out through the door: onto the sidewalk in front of it, not the door.
        let exit = *exit;
        let cmd = s.update(&mut st, &[key(dir_between(pos, exit.at))]);
        assert!(matches!(cmd, ScreenCmd::Stay));
        let (place, pos) = player(&st);
        assert_eq!(place, Place::Stratum(StratumKind::Street));
        assert_eq!(pos, exit.landing);
        assert_ne!(pos, door);
        assert_eq!(s.place, place);
    }

    #[test]
    fn the_lift_lists_floors_and_rides_and_stairs_climb() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let b = tower(&st);
        let city = st.city.as_ref().unwrap().clone();
        let plan = st.interiors.plan(&city, b).unwrap().clone();
        st.interiors.ensure(&city, b, 0).unwrap();
        let lift = plan.lift.expect("a tower has a lift bank");
        stand(
            &mut st,
            &mut s,
            Place::Floor {
                building: b,
                level: 0,
            },
            lift,
        );
        let ScreenCmd::Push { to: mut popup, .. } = s.update(&mut st, &[key(KeyCode::Char('>'))])
        else {
            panic!("the lift opens the FLOORS popup")
        };
        st.tick += 40;
        let text = draw_screen(&*popup, &st, 120, 40);
        assert!(text.contains("FLOORS"), "{text}");
        assert!(
            text.contains(" L ") || text.contains(" L·") || text.contains("L · here"),
            "{text}"
        );
        assert!(text.contains("2F"), "{text}");
        assert!(text.contains(&FloorPlan::label(plan.top)), "{text}");
        assert!(text.contains("roof"), "{text}");
        popup.update(&mut st, &[key(KeyCode::Esc)]);
        // A ride to a floor never generated before works (and generates it).
        assert!(st.interiors.floor(b, plan.top).is_none());
        crate::interiors::ride(&mut st, b, plan.top).unwrap();
        s.update(&mut st, &[]);
        let (place, pos) = player(&st);
        assert_eq!(
            place,
            Place::Floor {
                building: b,
                level: plan.top
            }
        );
        assert_eq!(pos, lift);
        assert!(st.interiors.floor(b, plan.top).is_some());
        assert_eq!(s.place, place);
        // Stairs: one floor at a time, same tile.
        stand(&mut st, &mut s, place, plan.stair);
        s.update(&mut st, &[key(KeyCode::Char('<'))]);
        let (place, pos) = player(&st);
        assert_eq!(
            place,
            Place::Floor {
                building: b,
                level: plan.top - 1
            }
        );
        assert_eq!(pos, plan.stair);
        s.update(&mut st, &[key(KeyCode::Char('>'))]);
        assert_eq!(
            player(&st).0,
            Place::Floor {
                building: b,
                level: plan.top
            }
        );
        s.update(&mut st, &[key(KeyCode::Char('>'))]);
        assert_eq!(
            player(&st).0,
            Place::Floor {
                building: b,
                level: plan.top
            },
            "no floor above the top"
        );
        assert!(
            last(&st).unwrap().contains("nothing leads up"),
            "{:?}",
            last(&st)
        );
        // Off the stair, > and < lead nowhere inside.
        let (_, floor) = st.interiors.floor(b, plan.top).unwrap();
        let open = floor
            .tiles
            .iter()
            .find(|(p, t)| {
                t.terrain == citygen::Terrain::Floor && t.feature.is_none() && *p != plan.stair
            })
            .unwrap()
            .0;
        stand(
            &mut st,
            &mut s,
            Place::Floor {
                building: b,
                level: plan.top,
            },
            open,
        );
        s.update(&mut st, &[key(KeyCode::Char('>'))]);
        assert_eq!(player(&st).1, open);
    }

    #[test]
    fn roof_bridge_and_basement_doors_enter_and_leave_on_their_strata() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let city = st.city.as_ref().unwrap().clone();
        let mut covered = 0;
        for b in city.buildings.iter().filter(|b| b.kind.is_closed()) {
            for (k, door) in city.portals(b.id) {
                if k == StratumKind::Street {
                    continue;
                }
                // Stand on the walkable tile the door opens onto.
                let beside = door.neighbors4().find(|n| {
                    city.walkable(k, *n) && (k == StratumKind::Heights || !b.footprint.contains(*n))
                });
                let Some(beside) = beside else { continue };
                stand(&mut st, &mut s, Place::Stratum(k), beside);
                let seen_before = st
                    .player
                    .as_ref()
                    .unwrap()
                    .vis()
                    .count(|v| *v != Visibility::Unseen);
                s.update(&mut st, &[key(dir_between(beside, door))]);
                let (place, pos) = player(&st);
                let Place::Floor { building, level } = place else {
                    panic!("{k:?} door of {:?} did not enter: {place:?}", b.id)
                };
                assert_eq!(building, b.id);
                let plan = st.interiors.plan_of(b.id).unwrap().clone();
                let expected = match k {
                    StratumKind::Heights => plan.top,
                    StratumKind::Skyways => plan.bridge.unwrap(),
                    StratumKind::Undercity => -1,
                    _ => unreachable!(),
                };
                assert_eq!(level, expected, "{k:?}");
                let (_, floor) = st.interiors.floor(b.id, level).unwrap();
                let exit = *floor.exits.iter().find(|e| e.door == door).unwrap();
                assert_eq!(pos, floor.doorstep(&exit));
                // And back out onto the same stratum.
                s.update(&mut st, &[key(dir_between(pos, exit.at))]);
                let (place, pos) = player(&st);
                assert_eq!(place, Place::Stratum(k), "{k:?}");
                assert_eq!(pos, exit.landing);
                assert!(
                    st.player
                        .as_ref()
                        .unwrap()
                        .vis()
                        .count(|v| *v != Visibility::Unseen)
                        >= seen_before,
                    "sight memory of {k:?} is kept"
                );
                covered |= match k {
                    StratumKind::Heights => 1,
                    StratumKind::Skyways => 2,
                    _ => 4,
                };
                if covered == 7 {
                    return;
                }
            }
        }
        assert_eq!(covered, 7, "roof, bridge and basement doors all tried");
    }

    #[test]
    fn jacking_in_inside_finds_no_terminal_and_the_body_place_round_trips() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let b = tower(&st);
        let city = st.city.as_ref().unwrap().clone();
        st.interiors.ensure(&city, b, 0).unwrap();
        let (_, lobby) = st.interiors.floor(b, 0).unwrap();
        let step = lobby.doorstep(&lobby.exits[0]);
        stand(
            &mut st,
            &mut s,
            Place::Floor {
                building: b,
                level: 0,
            },
            step,
        );
        s.update(&mut st, &[key(KeyCode::Enter)]);
        assert_eq!(last(&st), Some("no terminal within reach"));
        let ScreenCmd::Push { to: mut here, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens HERE")
        };
        st.tick += 40;
        let text = draw_screen(&*here, &st, 120, 40);
        assert!(!text.contains("jack in"), "{text}");
        here.update(&mut st, &[key(KeyCode::Esc)]);
        // Outside, beside a terminal, the body stays on the street while jacked in.
        let (jack, beside) = {
            let jack = city
                .connectors
                .iter()
                .find(|k| k.kind == citygen::ConnectorKind::Jack && k.public)
                .unwrap();
            let beside = jack
                .pos
                .neighbors8()
                .find(|n| city.walkable(StratumKind::Street, *n))
                .unwrap();
            (jack.pos, beside)
        };
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), beside);
        s.update(&mut st, &[key(KeyCode::Enter)]);
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.place, Place::Stratum(StratumKind::Net));
        assert_eq!(p.pos, jack);
        assert_eq!(p.body, Some((Place::Stratum(StratumKind::Street), beside)));
        s.update(&mut st, &[key(KeyCode::Enter)]);
        assert_eq!(player(&st), (Place::Stratum(StratumKind::Street), beside));
    }

    #[test]
    fn locked_doors_stay_shut_and_say_so() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let city = st.city.as_ref().unwrap().clone();
        let b = city
            .buildings
            .iter()
            .find(|b| b.kind == BuildingKind::Megablock)
            .unwrap()
            .id;
        st.interiors.plan(&city, b).unwrap();
        st.interiors.ensure(&city, b, 1).unwrap();
        let (_, floor) = st.interiors.floor(b, 1).unwrap();
        let (door, corridor) = floor
            .rooms
            .iter()
            .filter(|r| r.private)
            .find_map(|r| {
                let d = r.door?;
                let c = d
                    .neighbors4()
                    .find(|n| !r.rect.contains(*n) && floor.tiles[*n].is_walkable())?;
                Some((d, c))
            })
            .expect("a locked room with a corridor outside its door");
        stand(
            &mut st,
            &mut s,
            Place::Floor {
                building: b,
                level: 1,
            },
            corridor,
        );
        s.update(&mut st, &[key(dir_between(corridor, door))]);
        assert_eq!(player(&st).1, corridor, "the door does not open");
        assert!(last(&st).unwrap().contains("locked"), "{:?}", last(&st));
        assert!(last_is_warning(&st));
    }

    fn clock(st: &mut AppState, hhmm: &str) {
        let at = {
            let p = st.player.as_ref().unwrap();
            game::At {
                place: p.place,
                pos: p.pos,
            }
        };
        crate::screens::console::run(st, at, &format!("clock {hhmm}")).unwrap();
    }

    #[test]
    fn occupants_stand_at_their_stations_and_space_talks_to_them() {
        let mut st = state8();
        seed_world(&mut st);
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        clock(&mut st, "23:00");
        let city = st.city.as_ref().unwrap().clone();
        let (b, keeper) = city
            .buildings
            .iter()
            .filter(|b| b.kind == BuildingKind::Bar)
            .find_map(|b| {
                let world = st.world.as_ref().unwrap();
                world
                    .inside(b.id)
                    .into_iter()
                    .find(|o| world.people()[o.id.0 as usize].profession == "barkeep")
                    .map(|o| (b.id, o.id))
            })
            .expect("a bar with its keeper behind the bar at eleven");
        st.interiors.ensure(&city, b, 0).unwrap();
        let step = {
            let (_, f) = st.interiors.floor(b, 0).unwrap();
            f.doorstep(&f.exits[0])
        };
        stand(
            &mut st,
            &mut s,
            Place::Floor {
                building: b,
                level: 0,
            },
            step,
        );
        let placement = s
            .placement
            .as_ref()
            .map(|(_, _, p)| p.clone())
            .expect("a placement for the floor");
        let pos = placement
            .people()
            .find(|(_, id)| *id == keeper)
            .map(|(p, _)| p)
            .expect("the barkeep is placed");
        let (_, floor) = st.interiors.floor(b, 0).unwrap();
        assert_eq!(
            floor.station_at(pos).map(|s| s.role),
            Some(citygen::Role::Bar)
        );
        let beside = pos
            .neighbors4()
            .find(|n| floor.tiles[*n].is_walkable() && placement.person_at(*n).is_none())
            .expect("room beside the bar");
        stand(
            &mut st,
            &mut s,
            Place::Floor {
                building: b,
                level: 0,
            },
            beside,
        );
        let text = draw_screen(&s, &st, 120, 40);
        assert!(
            text.matches('@').count() >= 2,
            "the keeper draws beside the player: {text}"
        );
        s.update(&mut st, &[key(dir_between(beside, pos))]);
        assert_eq!(player(&st).1, beside, "people indoors are solid");
        assert_eq!(
            s.inside_person_at(player(&st).0, pos),
            Some(keeper),
            "and not swapped"
        );
        let ScreenCmd::Push { to: mut here, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens HERE")
        };
        st.tick += 40;
        let popup = draw_screen(&*here, &st, 120, 40);
        let world = st.world.as_ref().unwrap();
        let card = world.card(keeper, &city);
        assert!(popup.contains(&format!("talk to {}", card.name)), "{popup}");
        assert_eq!(card.doing, "behind the bar");
        here.update(&mut st, &[key(KeyCode::Esc)]);
    }

    #[test]
    fn a_locked_flat_names_its_resident() {
        let mut st = state8();
        seed_world(&mut st);
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        clock(&mut st, "03:00");
        let city = st.city.as_ref().unwrap().clone();
        let b = {
            let world = st.world.as_ref().unwrap();
            city.buildings
                .iter()
                .find(|b| {
                    b.kind == BuildingKind::Megablock
                        && world
                            .inside(b.id)
                            .iter()
                            .any(|o| o.role == sim::OccupantRole::Living)
                })
                .expect("a megablock with people home at three")
                .id
        };
        let plan = st.interiors.plan(&city, b).unwrap().clone();
        let mut found = None;
        for level in plan.corridor_floors() {
            st.interiors.ensure(&city, b, level).unwrap();
            let (plan, floor) = st.interiors.floor(b, level).unwrap();
            let placement =
                crate::interiors::Placement::new(plan, floor, st.world.as_ref().unwrap());
            let owned = floor.rooms.iter().filter(|r| r.private).find_map(|r| {
                let d = r.door?;
                let id = placement.owner_of(d)?;
                let outside = d
                    .neighbors4()
                    .find(|n| !r.rect.contains(*n) && floor.tiles[*n].is_walkable())?;
                Some((level, d, outside, id))
            });
            if owned.is_some() {
                found = owned;
                break;
            }
        }
        let (level, door, outside, id) = found.expect("a flat with its resident home");
        stand(
            &mut st,
            &mut s,
            Place::Floor { building: b, level },
            outside,
        );
        s.update(&mut st, &[key(dir_between(outside, door))]);
        assert_eq!(player(&st).1, outside);
        let name = st.world.as_ref().unwrap().people()[id.0 as usize]
            .name
            .clone();
        assert!(
            last(&st).is_some_and(|l| l.starts_with(&format!("locked · {name}'s flat"))),
            "{:?}",
            last(&st)
        );
    }

    #[test]
    fn locked_doors_open_from_the_inside() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let city = st.city.as_ref().unwrap().clone();
        let b = city
            .buildings
            .iter()
            .find(|b| b.kind == BuildingKind::Megablock)
            .unwrap()
            .id;
        st.interiors.plan(&city, b).unwrap();
        st.interiors.ensure(&city, b, 1).unwrap();
        let (_, floor) = st.interiors.floor(b, 1).unwrap();
        let (door, inside) = floor
            .rooms
            .iter()
            .filter(|r| r.private)
            .find_map(|r| {
                let d = r.door?;
                let i = d
                    .neighbors4()
                    .find(|n| r.rect.contains(*n) && floor.tiles[*n].is_walkable())?;
                Some((d, i))
            })
            .expect("a locked room with floor inside its door");
        stand(
            &mut st,
            &mut s,
            Place::Floor {
                building: b,
                level: 1,
            },
            inside,
        );
        s.update(&mut st, &[key(dir_between(inside, door))]);
        assert_eq!(player(&st).1, door, "a door opens from the inside");
    }

    #[test]
    fn space_on_a_stair_climbs_without_a_lift() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let city = st.city.as_ref().unwrap().clone();
        let b = city
            .buildings
            .iter()
            .filter(|b| b.kind.is_closed())
            .find(|b| {
                let plan = FloorPlan::new(&city, b.id).unwrap();
                plan.lift.is_none() && plan.floors() >= 2
            })
            .expect("a two-floor building without a lift")
            .id;
        let plan = st.interiors.plan(&city, b).unwrap().clone();
        st.interiors.ensure(&city, b, plan.lowest).unwrap();
        stand(
            &mut st,
            &mut s,
            Place::Floor {
                building: b,
                level: plan.lowest,
            },
            plan.stair,
        );
        let ScreenCmd::Push { to: mut here, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens HERE")
        };
        st.tick += 40;
        let text = draw_screen(&*here, &st, 120, 40);
        assert!(
            text.contains(&format!("up to {}", FloorPlan::label(plan.lowest + 1))),
            "{text}"
        );
        here.update(&mut st, &[key(KeyCode::Char('>'))]);
        s.update(&mut st, &[]);
        let (place, pos) = player(&st);
        assert_eq!(
            place,
            Place::Floor {
                building: b,
                level: plan.lowest + 1
            }
        );
        assert_eq!(pos, plan.stair, "stairs keep the tile");
    }

    #[test]
    fn leaving_lands_on_standable_ground_even_when_the_landing_is_blocked() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let b = tower(&st);
        let city = st.city.as_ref().unwrap().clone();
        st.interiors.ensure(&city, b, 0).unwrap();
        let (_, lobby) = st.interiors.floor(b, 0).unwrap();
        let exit = lobby.exits[0];
        let step = lobby.doorstep(&exit);
        // A barrier dropped on the landing.
        st.city
            .as_mut()
            .unwrap()
            .tile_mut(StratumKind::Street, exit.landing)
            .unwrap()
            .feature = Some(citygen::Feature::Barrier);
        stand(
            &mut st,
            &mut s,
            Place::Floor {
                building: b,
                level: 0,
            },
            step,
        );
        s.update(&mut st, &[key(dir_between(step, exit.at))]);
        let (place, pos) = player(&st);
        assert_eq!(place, Place::Stratum(StratumKind::Street));
        assert_ne!(pos, exit.landing);
        assert!(st.city.as_ref().unwrap().walkable(StratumKind::Street, pos));
        assert!(
            pos.manhattan(exit.door) <= 2,
            "{pos:?} beside the door {:?}",
            exit.door
        );
    }

    #[test]
    fn the_gate_leads_into_the_construct_and_back() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let city = st.city.as_ref().unwrap();
        // A building's gate on the Net, approached from the open grid beside it.
        let net = city.stratum(StratumKind::Net).unwrap();
        let (b, gate, outside, dir) = city
            .buildings
            .iter()
            .find_map(|b| {
                let gate = b
                    .footprint
                    .border()
                    .find(|p| net.tiles[*p].terrain == citygen::Terrain::NetGate)?;
                [Dir::N, Dir::S, Dir::E, Dir::W].into_iter().find_map(|d| {
                    let out = gate.step(d);
                    (!b.footprint.contains(out) && city.walkable(StratumKind::Net, out))
                        .then_some((b.id, gate, out, d.opposite()))
                })
            })
            .expect("a gate with grid beside it");
        let name = city.building(b).name.clone().unwrap();
        {
            let p = st.player.as_mut().unwrap();
            p.body = Some((p.place, p.pos));
            p.place = Place::Stratum(StratumKind::Net);
            p.pos = outside;
        }
        s.place = Place::Stratum(StratumKind::Net);
        let code = match dir {
            Dir::N => KeyCode::Char('k'),
            Dir::S => KeyCode::Char('j'),
            Dir::E => KeyCode::Char('l'),
            _ => KeyCode::Char('h'),
        };
        s.update(&mut st, &[key(code)]);
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.place, Place::Construct(b), "{:?}", last(&st));
        let k = st.constructs.get(b).expect("generated on entry");
        assert_eq!(p.pos, k.entry);
        assert_eq!(last(&st), Some(format!("Entering {name}").as_str()));
        assert_eq!(s.place, Place::Construct(b));
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains(&format!("{name} · construct")), "{text}");
        if !k.guards.is_empty() {
            assert!(text.contains('▣'), "the gate guard draws: {text}");
        }
        // Out through the entry, back onto the grid at the gate.
        s.update(&mut st, &[key(KeyCode::Char('h'))]);
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.place, Place::Stratum(StratumKind::Net), "{:?}", last(&st));
        assert_eq!(p.pos, gate);
        assert_eq!(
            last(&st),
            Some(format!("Leaving {name} · onto the grid").as_str())
        );
        // In again: the construct is generated once; jacking out from inside works.
        s.update(&mut st, &[key(code)]);
        assert_eq!(st.player.as_ref().unwrap().place, Place::Construct(b));
        assert_eq!(st.constructs.generated(), 1);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Flash(TransitionKind::Glitch)
        ));
        let p = st.player.as_ref().unwrap();
        assert!(!p.jacked_in() && p.place.is_street());
    }

    /// Puts the walking player inside the construct of a tower with a vault and
    /// a lock, jacked in from the street beside it; returns the building.
    fn in_construct(st: &mut AppState, s: &mut MapScreen) -> citygen::BuildingId {
        in_construct_at(st, s, 200)
    }

    fn in_construct_at(st: &mut AppState, s: &mut MapScreen, security: u8) -> citygen::BuildingId {
        walking(st, s);
        let city = st.city.as_ref().unwrap().clone();
        let b = city
            .buildings
            .iter()
            .find(|b| {
                b.kind == BuildingKind::CorpTower
                    && city
                        .net
                        .nodes
                        .iter()
                        .any(|n| n.building == Some(b.id) && n.kind == NodeKind::DoorLock)
                    && city
                        .net
                        .nodes
                        .iter()
                        .any(|n| n.building == Some(b.id) && n.kind == NodeKind::DataVault)
            })
            .expect("a tower with a lock and a vault")
            .id;
        let gate = crate::constructs::city_gate(&city, b).unwrap();
        // Security 200: grey ICE at the gate and the vault, and a crack that lands on a ten.
        st.city
            .as_mut()
            .unwrap()
            .net
            .subnets
            .iter_mut()
            .find(|s| s.owner == citygen::SubnetOwner::Building(b))
            .unwrap()
            .security = security;
        {
            let p = st.player.as_mut().unwrap();
            p.body = Some((Place::Stratum(StratumKind::Street), p.pos));
            p.place = Place::Stratum(StratumKind::Net);
            p.pos = gate;
        }
        s.place = Place::Stratum(StratumKind::Net);
        let r = crate::constructs::enter_construct(st, b).unwrap();
        assert!(r.starts_with("Entering "));
        s.place = Place::Construct(b);
        b
    }

    fn node_of(st: &AppState, b: citygen::BuildingId, kind: NodeKind) -> (citygen::NodeId, Pos) {
        let city = st.city.as_ref().unwrap();
        let k = st.constructs.get(b).unwrap();
        k.nodes
            .iter()
            .find(|(id, _)| city.net.node(*id).kind == kind)
            .copied()
            .unwrap()
    }

    fn beside(st: &AppState, b: citygen::BuildingId, p: Pos) -> Pos {
        let k = st.constructs.get(b).unwrap();
        p.neighbors4()
            .find(|n| k.tiles.get(*n).is_some_and(|t| t.is_walkable()))
            .unwrap()
    }

    #[test]
    fn a_run_cracks_a_lock_in_turns_and_the_building_feels_it() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let (lock, lock_pos) = node_of(&st, b, NodeKind::DoorLock);
        let here = beside(&st, b, lock_pos);
        stand(&mut st, &mut s, Place::Construct(b), here);
        st.net.force_roll = Some(10);
        // The RUN popup lists crack against the lock, with the odds, and not blind.
        let ScreenCmd::Push { to: mut run, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens RUN")
        };
        st.tick += 40;
        let text = draw_screen(&*run, &st, 120, 40);
        assert!(text.contains("RUN"), "{text}");
        assert!(text.contains("crack · door lock · 40%"), "{text}");
        assert!(text.contains("3 turns · noise 4"), "{text}");
        assert!(!text.contains("blind"), "{text}");
        run.update(&mut st, &[key(KeyCode::Char('a'))]);
        assert_eq!(last(&st), Some("crack starts · 3 turns"));
        assert!(st.net.run.is_some());
        // Three waits land it.
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(last(&st), Some("crack 1/3"));
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert!(
            last(&st).is_some_and(|l| l.starts_with("lock cracked")),
            "{:?}",
            last(&st)
        );
        assert!(st.net.run.is_none());
        let now = st.net.now(&st);
        assert!(st.net.unlocked(b, now));
        assert_eq!(st.net.traces[&b].heat, 12);
        assert!(
            st.net
                .hacks
                .iter()
                .any(|h| h.target == game::HackTarget::Node(lock))
        );
        // A second run aborts on a move.
        let ScreenCmd::Push { to: mut run, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens RUN")
        };
        run.update(&mut st, &[key(KeyCode::Char('a'))]);
        assert!(st.net.run.is_some());
        let dir = [Dir::N, Dir::S, Dir::E, Dir::W]
            .into_iter()
            .find(|d| {
                st.constructs
                    .get(b)
                    .unwrap()
                    .tiles
                    .get(here.step(*d))
                    .is_some_and(|t| t.is_walkable())
            })
            .unwrap();
        let code = match dir {
            Dir::N => KeyCode::Char('k'),
            Dir::S => KeyCode::Char('j'),
            Dir::E => KeyCode::Char('l'),
            _ => KeyCode::Char('h'),
        };
        s.update(&mut st, &[key(code)]);
        assert!(st.net.run.is_none());
        assert_eq!(last(&st), Some("run aborted"));
        // On the tower's first floor, the cracked lock lets the player into an office.
        let city = st.city.as_ref().unwrap().clone();
        st.interiors.ensure(&city, b, 1).unwrap();
        let (_, floor) = st.interiors.floor(b, 1).unwrap();
        let (door, outside) = floor
            .rooms
            .iter()
            .filter(|r| r.private)
            .find_map(|r| {
                let door = r.door?;
                let outside = door
                    .neighbors4()
                    .find(|n| !r.rect.contains(*n) && floor.tiles[*n].is_walkable())?;
                Some((door, outside))
            })
            .expect("a locked office with a corridor outside");
        let name = city.building(b).name.clone().unwrap();
        {
            let p = st.player.as_mut().unwrap();
            p.body = None;
        }
        stand(
            &mut st,
            &mut s,
            Place::Floor {
                building: b,
                level: 1,
            },
            outside,
        );
        let step = [Dir::N, Dir::S, Dir::E, Dir::W]
            .into_iter()
            .find(|d| outside.step(*d) == door)
            .unwrap();
        let code = match step {
            Dir::N => KeyCode::Char('k'),
            Dir::S => KeyCode::Char('j'),
            Dir::E => KeyCode::Char('l'),
            _ => KeyCode::Char('h'),
        };
        s.update(&mut st, &[key(code)]);
        assert_eq!(st.player.as_ref().unwrap().pos, door, "{:?}", last(&st));
        // The alarm locks it again, stops the lifts and shows on the HUD.
        stand(
            &mut st,
            &mut s,
            Place::Floor {
                building: b,
                level: 1,
            },
            outside,
        );
        let at = game::At {
            place: Place::Floor {
                building: b,
                level: 1,
            },
            pos: outside,
        };
        assert_eq!(
            crate::screens::console::run(&mut st, at, "alarm").unwrap(),
            format!("ALARM · `{name}` locks down")
        );
        s.update(&mut st, &[key(code)]);
        assert_eq!(st.player.as_ref().unwrap().pos, outside);
        assert!(
            last(&st).is_some_and(|l| l.starts_with("locked")),
            "{:?}",
            last(&st)
        );
        assert_eq!(
            crate::interiors::ride(&mut st, b, 0).unwrap_err(),
            "lifts locked down"
        );
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.contains("ALERT · "), "{text}");
    }

    #[test]
    fn the_trace_dumps_you_and_the_port_refuses_while_cooling_or_locked_down() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let name = st.city.as_ref().unwrap().building(b).name.clone().unwrap();
        let (_, server) = node_of(&st, b, NodeKind::Server);
        let here = beside(&st, b, server);
        stand(&mut st, &mut s, Place::Construct(b), here);
        let at = game::At {
            place: Place::Construct(b),
            pos: here,
        };
        let body = st.player.as_ref().unwrap().body.unwrap();
        assert_eq!(
            crate::screens::console::run(&mut st, at, "heat 100").unwrap(),
            "heat 100"
        );
        let p = st.player.as_ref().unwrap();
        assert!(!p.jacked_in(), "dumped");
        assert_eq!((p.place, p.pos), body);
        assert_eq!(st.character.focus, 0);
        assert!(last_is_warning(&st) && last(&st).is_some_and(|l| l.starts_with("DUMPED")));
        s.update(&mut st, &[]);
        assert_eq!(s.place, body.0);
        let now = st.net.now(&st);
        assert_eq!(st.net.cooling(now), Some(game::COOLING_MINUTES));
        // The port refuses while the deck cools.
        s.update(&mut st, &[key(KeyCode::Enter)]);
        assert_eq!(last(&st), Some("deck cooling · 10 min"));
        st.net.cooling_until = None;
        // And beside a building on alert.
        st.net.trace(b).add(game::ALARM_HEAT, now);
        st.net.alarm(b, now);
        let city = st.city.as_ref().unwrap().clone();
        let fp = city.building(b).footprint;
        let port = city
            .connectors
            .iter()
            .find(|k| k.kind == citygen::ConnectorKind::Jack && fp.grow(1).contains(k.pos))
            .map(|k| k.pos);
        if let Some(port) = port {
            let beside = port
                .neighbors8()
                .find(|n| city.walkable(StratumKind::Street, *n))
                .unwrap();
            stand(&mut st, &mut s, Place::Stratum(StratumKind::Street), beside);
            s.update(&mut st, &[key(KeyCode::Enter)]);
            assert_eq!(last(&st), Some("port locked down"));
        }
        // The gate refuses too.
        {
            let p = st.player.as_mut().unwrap();
            p.body = Some((Place::Stratum(StratumKind::Street), p.pos));
            p.place = Place::Stratum(StratumKind::Net);
        }
        assert_eq!(
            crate::constructs::enter_construct(&mut st, b).unwrap_err(),
            format!("gate closed · `{name}` on alert")
        );
    }

    #[test]
    fn copy_backdoor_and_awake_guards() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        st.net.force_roll = Some(10);
        let (vault, vault_pos) = node_of(&st, b, NodeKind::DataVault);
        let here = beside(&st, b, vault_pos);
        stand(&mut st, &mut s, Place::Construct(b), here);
        let at = game::At {
            place: Place::Construct(b),
            pos: here,
        };
        let copy = ItemId::new("copy");
        let before = st.character.count(&ItemId::new("vault_data"));
        crate::net::start_run(&mut st, copy.clone(), game::HackTarget::Node(vault)).unwrap();
        for _ in 0..4 {
            s.update(&mut st, &[key(KeyCode::Char('.'))]);
        }
        assert_eq!(st.character.count(&ItemId::new("vault_data")), before + 1);
        assert!(
            last(&st).is_some_and(|l| l.starts_with("vault copied")),
            "{:?}",
            last(&st)
        );
        assert_eq!(
            crate::net::start_run(&mut st, copy.clone(), game::HackTarget::Node(vault))
                .unwrap_err(),
            "copied already"
        );
        // A backdoor lifts the gate guard and cools faster.
        let (server, server_pos) = node_of(&st, b, NodeKind::Server);
        let here = beside(&st, b, server_pos);
        stand(&mut st, &mut s, Place::Construct(b), here);
        st.character.unload(&st.content, &copy).unwrap();
        st.character
            .unload(&st.content, &ItemId::new("crack"))
            .unwrap();
        assert_eq!(
            crate::screens::console::run(&mut st, at, "program backdoor").unwrap(),
            "`Backdoor` loaded · 2/2 slots"
        );
        crate::net::start_run(
            &mut st,
            ItemId::new("backdoor"),
            game::HackTarget::Node(server),
        )
        .unwrap();
        for _ in 0..5 {
            s.update(&mut st, &[key(KeyCode::Char('.'))]);
        }
        assert!(st.net.backdoors.contains(&b), "{:?}", last(&st));
        let k = st.constructs.get(b).unwrap();
        assert!(!k.guards.is_empty());
        assert_eq!(k.guard_at(k.guards[0].pos), None, "the gate guard is down");
        let heat = st.net.traces[&b].heat;
        let now = st.net.now(&st);
        st.net.minute(now + 1, None, &mut st.constructs);
        assert_eq!(st.net.traces[&b].heat, heat.saturating_sub(2));
        // On alert, a standing guard beside you hits you at the end of a turn.
        let guard = st.constructs.get(b).unwrap().guards[1].pos;
        let here = beside(&st, b, guard);
        stand(&mut st, &mut s, Place::Construct(b), here);
        crate::screens::console::run(&mut st, at, "alarm").unwrap();
        assert!(st.player.as_ref().unwrap().jacked_in());
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert!(
            last(&st).is_some_and(|l| l.starts_with("grey ICE hits")),
            "{:?}",
            last(&st)
        );
        // The alert ends: heat zero, the guards stand again but the backdoor's gate guard.
        let now = st.net.now(&st);
        st.net
            .minute(now + game::ALERT_MINUTES + 1, None, &mut st.constructs);
        assert_eq!(st.net.traces[&b].heat, 0);
        assert!(!st.net.alert(b, now + game::ALERT_MINUTES + 1));
        let k = st.constructs.get(b).unwrap();
        assert_eq!(k.guard_at(k.guards[0].pos), None);
        assert_eq!(k.guard_at(k.guards[1].pos), Some(1));
    }

    #[test]
    fn the_here_section_shows_the_trace_inside_a_construct() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let k = st.constructs.get(b).unwrap();
        let entry = k.entry;
        stand(&mut st, &mut s, Place::Construct(b), entry);
        st.net.trace(b).heat = 34;
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.contains("TRACE ▰▰▰▱▱▱▱▱▱▱ 34"), "{text}");
        let now = st.net.now(&st);
        st.net.trace(b).add(game::ALARM_HEAT, now);
        st.net.alarm(b, now);
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.contains("ALERT · 60 min"), "{text}");
    }

    #[test]
    fn nothing_runs_without_a_deck() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let (lock, lock_pos) = node_of(&st, b, NodeKind::DoorLock);
        let here = beside(&st, b, lock_pos);
        stand(&mut st, &mut s, Place::Construct(b), here);
        st.character.equipment.remove(&game::Slot::Deck);
        assert_eq!(st.character.loaded.len(), 2, "the list is kept");
        assert_eq!(
            crate::net::start_run(&mut st, ItemId::new("crack"), game::HackTarget::Node(lock))
                .unwrap_err(),
            "no deck jacked"
        );
        let ScreenCmd::Push { to: here_popup, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens a popup")
        };
        st.tick += 40;
        let text = draw_screen(&*here_popup, &st, 120, 40);
        assert!(!text.contains("crack ·") && text.contains("HERE"), "{text}");
    }

    #[test]
    fn the_here_section_reads_the_node_under_you_not_its_kind() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let city = st.city.as_ref().unwrap().clone();
        let b = city
            .buildings
            .iter()
            .find(|b| {
                city.net
                    .nodes
                    .iter()
                    .filter(|n| n.building == Some(b.id) && n.kind == NodeKind::Camera)
                    .count()
                    >= 2
            })
            .expect("a building with two cameras")
            .id;
        {
            let p = st.player.as_mut().unwrap();
            p.body = Some((Place::Stratum(StratumKind::Street), p.pos));
            p.place = Place::Stratum(StratumKind::Net);
        }
        crate::constructs::enter_construct(&mut st, b).unwrap();
        s.place = Place::Construct(b);
        let k = st.constructs.get(b).unwrap();
        let cameras: Vec<(citygen::NodeId, Pos)> = k
            .nodes
            .iter()
            .filter(|(id, _)| city.net.node(*id).kind == NodeKind::Camera)
            .copied()
            .collect();
        assert!(cameras.len() >= 2);
        let now = st.net.now(&st);
        st.net.land(
            b,
            game::HackTarget::Node(cameras[0].0),
            game::Target::Node(NodeKind::Camera),
            game::Op::Blind,
            now,
        );
        stand(&mut st, &mut s, Place::Construct(b), cameras[1].1);
        let text = draw_screen(&s, &st, 120, 40);
        assert!(
            text.contains("◆ camera") && !text.contains("camera · blind"),
            "{text}"
        );
        stand(&mut st, &mut s, Place::Construct(b), cameras[0].1);
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.contains("camera · blind"), "{text}");
    }

    #[test]
    fn an_alarm_mid_run_keeps_the_run_and_a_jack_out_drops_it() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        st.net.force_roll = Some(10);
        let (lock, _) = node_of(&st, b, NodeKind::DoorLock);
        let (vault, vault_pos) = node_of(&st, b, NodeKind::DataVault);
        let here = beside(&st, b, vault_pos);
        stand(&mut st, &mut s, Place::Construct(b), here);
        let now = st.net.now(&st);
        st.net.land(
            b,
            game::HackTarget::Node(lock),
            game::Target::Node(NodeKind::DoorLock),
            game::Op::Crack,
            now,
        );
        crate::net::start_run(&mut st, ItemId::new("copy"), game::HackTarget::Node(vault)).unwrap();
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        let at = game::At {
            place: Place::Construct(b),
            pos: here,
        };
        crate::screens::console::run(&mut st, at, "alarm").unwrap();
        assert!(st.net.run.is_some(), "the alarm does not clear the run");
        assert!(!st.net.unlocked(b, now), "the lock hack expired");
        let until = st.net.traces[&b].alert_until;
        crate::screens::console::run(&mut st, at, "alarm").unwrap();
        assert_eq!(
            st.net.traces[&b].alert_until, until,
            "no second alarm while alert"
        );
        // Jacking out drops the run; the construct keeps its heat and its map.
        let heat = st.net.traces[&b].heat;
        s.update(&mut st, &[key(KeyCode::Enter)]);
        assert!(!st.player.as_ref().unwrap().jacked_in());
        assert!(st.net.run.is_none());
        assert_eq!(st.net.traces[&b].heat, heat);
        assert_eq!(st.constructs.generated(), 1);
        // A server copy gives paydata.
        {
            let p = st.player.as_mut().unwrap();
            p.body = Some((p.place, p.pos));
            p.place = Place::Stratum(StratumKind::Net);
        }
        let (server, server_pos) = node_of(&st, b, NodeKind::Server);
        let here = beside(&st, b, server_pos);
        stand(&mut st, &mut s, Place::Construct(b), here);
        let before = st.character.count(&ItemId::new("paydata"));
        crate::net::start_run(&mut st, ItemId::new("copy"), game::HackTarget::Node(server))
            .unwrap();
        for _ in 0..4 {
            s.update(&mut st, &[key(KeyCode::Char('.'))]);
        }
        assert_eq!(
            st.character.count(&ItemId::new("paydata")),
            before + 1,
            "{:?}",
            last(&st)
        );
    }

    #[test]
    fn a_run_with_no_chance_still_runs_and_slips() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        // Security 255: difficulty 31, beyond a rating-one crack with any roll.
        st.constructs.get_mut(b).unwrap().security = 255;
        let (_, lock_pos) = node_of(&st, b, NodeKind::DoorLock);
        let here = beside(&st, b, lock_pos);
        stand(&mut st, &mut s, Place::Construct(b), here);
        let ScreenCmd::Push { to: run, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))]) else {
            panic!("space opens RUN")
        };
        st.tick += 40;
        let text = draw_screen(&*run, &st, 120, 40);
        assert!(
            text.contains("crack · door lock · 0% · 3 turns · noise 4"),
            "{text}"
        );
        let (lock, _) = node_of(&st, b, NodeKind::DoorLock);
        assert_eq!(
            crate::net::start_run(&mut st, ItemId::new("crack"), game::HackTarget::Node(lock))
                .unwrap(),
            "crack starts · 3 turns"
        );
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert!(
            last(&st).is_some_and(|l| l.starts_with("crack slips")),
            "{:?}",
            last(&st)
        );
    }

    #[test]
    fn a_run_in_flight_shows_its_turn_hint_and_a_continue_row() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        st.net.force_roll = Some(10);
        let (lock, lock_pos) = node_of(&st, b, NodeKind::DoorLock);
        let here = beside(&st, b, lock_pos);
        stand(&mut st, &mut s, Place::Construct(b), here);
        let text = draw_screen(&s, &st, 120, 40);
        assert!(
            text.contains("[space]act") && !text.contains("[.]turn"),
            "{text}"
        );
        crate::net::start_run(&mut st, ItemId::new("crack"), game::HackTarget::Node(lock)).unwrap();
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        let text = draw_screen(&s, &st, 120, 40);
        assert!(
            text.contains("[.]turn") && text.contains("[space]continue"),
            "{text}"
        );
        // The popup leads with continue, and picking it is a turn.
        let ScreenCmd::Push { to: mut run, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens RUN")
        };
        st.tick += 40;
        let text = draw_screen(&*run, &st, 120, 40);
        assert!(text.contains("c  continue · crack 1/3"), "{text}");
        run.update(&mut st, &[key(KeyCode::Char('c'))]);
        assert_eq!(last(&st), Some("crack 2/3"));
        assert_eq!(st.net.run.as_ref().map(|r| r.done), Some(2));
        // Picking another program says the first run was dropped.
        let ScreenCmd::Push { to: mut run, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))])
        else {
            panic!("space opens RUN")
        };
        run.update(&mut st, &[key(KeyCode::Char('a'))]);
        assert_eq!(last(&st), Some("run aborted · crack starts · 3 turns"));
        assert_eq!(st.net.run.as_ref().map(|r| r.done), Some(0));
    }

    /// A room tile of the construct the gate guard cannot see into.
    fn room_tile(st: &AppState, b: citygen::BuildingId) -> Pos {
        let k = st.constructs.get(b).unwrap();
        k.rooms[0]
            .iter()
            .find(|p| k.tiles[*p].terrain == citygen::Terrain::NetFloor && k.node_at(*p).is_none())
            .unwrap()
    }

    #[test]
    fn asleep_ice_walks_its_beat_and_turns_at_the_end() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let hide = room_tile(&st, b);
        stand(&mut st, &mut s, Place::Construct(b), hide);
        let beat = st.constructs.get(b).unwrap().guards[0].beat.clone();
        assert!(beat.len() >= 3, "{beat:?}");
        let mut seen = Vec::new();
        for _ in 0..(2 * beat.len()) {
            s.update(&mut st, &[key(KeyCode::Char('.'))]);
            let k = st.constructs.get(b).unwrap();
            seen.push(k.guards[0].pos);
            assert_eq!(k.guard_at(k.guards[0].pos), Some(0), "the feature travels");
            assert!(matches!(
                st.net.ice[&b][0].mode,
                crate::net::IceMode::Asleep
            ));
        }
        // Every stop is on the beat, no stride is longer than a tile (a turn beside
        // the other piece may hold it), and it reaches the far end and comes back.
        assert!(seen.iter().all(|p| beat.contains(p)), "{seen:?}");
        for w in seen.windows(2) {
            assert!(w[0].chebyshev(w[1]) <= 1, "{seen:?}");
        }
        let far = seen
            .iter()
            .position(|p| *p == beat[beat.len() - 1])
            .expect("the far end");
        assert!(
            seen[far + 1..].contains(&beat[beat.len() - 2]),
            "and back: {seen:?}"
        );
    }

    #[test]
    fn ice_sees_along_the_spine_not_into_rooms_and_less_under_a_mask() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let k = st.constructs.get(b).unwrap();
        let g = k.guards[0].clone();
        let sy = g.pos.y;
        let on_spine = |dx: i32| Pos::new(g.pos.x + dx, sy);
        assert!(crate::net::sees(k, g.pos, g.grade, on_spine(6), false));
        assert!(!crate::net::sees(k, g.pos, g.grade, on_spine(7), false));
        assert!(crate::net::sees(k, g.pos, g.grade, on_spine(3), true));
        assert!(
            !crate::net::sees(k, g.pos, g.grade, on_spine(4), true),
            "masked: half"
        );
        let hide = room_tile(&st, b);
        assert!(
            !crate::net::sees(k, g.pos, g.grade, hide, false),
            "rooms are not seen into"
        );
        assert!(!crate::net::sees(
            k,
            g.pos,
            IceGrade::White,
            on_spine(5),
            false
        ));
    }

    #[test]
    fn seen_ice_wakes_hunts_loses_you_in_a_room_and_goes_home() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let (post, beat) = {
            let g = st.constructs.get(b).unwrap().guards[0].clone();
            (g.post, g.beat.clone())
        };
        let seen_at = Pos::new(post.x + 5, post.y);
        stand(&mut st, &mut s, Place::Construct(b), seen_at);
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        let ice = &st.net.ice[&b][0];
        assert!(
            matches!(ice.mode, crate::net::IceMode::Awake { seen, .. } if seen == seen_at),
            "{:?}",
            ice.mode
        );
        let before = st.constructs.get(b).unwrap().guards[0].pos;
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        let after = st.constructs.get(b).unwrap().guards[0].pos;
        assert_eq!(
            after.chebyshev(seen_at),
            before.chebyshev(seen_at) - 1,
            "one tile a turn"
        );
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.contains("grey ICE · hunting"), "{text}");
        // Into a room: it reaches the last seen tile, loses you, and never enters.
        let room = st.constructs.get(b).unwrap().rooms[0];
        let hide = room_tile(&st, b);
        stand(&mut st, &mut s, Place::Construct(b), hide);
        for _ in 0..14 {
            s.update(&mut st, &[key(KeyCode::Char('.'))]);
            let p = st.constructs.get(b).unwrap().guards[0].pos;
            assert!(!room.contains(p), "ICE in the room at {p:?}");
        }
        let ice = &st.net.ice[&b][0];
        assert!(
            matches!(ice.mode, crate::net::IceMode::Asleep),
            "{:?}",
            ice.mode
        );
        assert!(
            beat.contains(&st.constructs.get(b).unwrap().guards[0].pos),
            "back on its beat"
        );
        assert!(st.player.as_ref().unwrap().jacked_in(), "{:?}", last(&st));
    }

    #[test]
    fn the_alarm_wakes_every_piece_and_two_hunters_never_share_a_tile() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let hide = room_tile(&st, b);
        stand(&mut st, &mut s, Place::Construct(b), hide);
        let at = game::At {
            place: Place::Construct(b),
            pos: hide,
        };
        crate::screens::console::run(&mut st, at, "alarm").unwrap();
        assert!(
            st.net.ice[&b]
                .iter()
                .all(|i| matches!(i.mode, crate::net::IceMode::Awake { .. }))
        );
        for _ in 0..10 {
            s.update(&mut st, &[key(KeyCode::Char('.'))]);
            let k = st.constructs.get(b).unwrap();
            assert_ne!(k.guards[0].pos, k.guards[1].pos);
            assert_ne!(k.guards[0].pos, hide);
        }
    }

    #[test]
    fn white_ice_adds_heat_and_grey_ice_drains_focus_to_a_dump() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct_at(&mut st, &mut s, 170);
        let (post, grade) = {
            let g = &st.constructs.get(b).unwrap().guards[0];
            (g.post, g.grade)
        };
        assert_eq!(grade, IceGrade::White);
        stand(
            &mut st,
            &mut s,
            Place::Construct(b),
            Pos::new(post.x - 1, post.y),
        );
        assert_eq!(
            last(&st),
            Some("white ICE hits · heat 5"),
            "{:?}",
            last(&st)
        );
        assert_eq!(st.net.traces[&b].heat, 5);
        // Grey: ten less the deck's rating a hit, a dump at zero.
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let post = st.constructs.get(b).unwrap().guards[0].post;
        stand(
            &mut st,
            &mut s,
            Place::Construct(b),
            Pos::new(post.x - 1, post.y),
        );
        assert_eq!(
            last(&st),
            Some("grey ICE hits · focus 21"),
            "{:?}",
            last(&st)
        );
        assert_eq!(st.character.focus, 21);
        for _ in 0..2 {
            s.update(&mut st, &[key(KeyCode::Char('.'))]);
        }
        assert_eq!(st.character.focus, 3);
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(st.character.focus, 0);
        assert_eq!(last(&st), Some("DUMPED · grey ICE drained you"));
        assert!(!st.player.as_ref().unwrap().jacked_in());
    }

    #[test]
    fn a_shield_absorbs_a_hit_and_an_attack_breaks_ice() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        st.net.force_roll = Some(10);
        let post = st.constructs.get(b).unwrap().guards[0].post;
        // Only the gate guard plays: the vault guard is down under a landed crack.
        let now = st.net.now(&st);
        st.net.land(
            b,
            game::HackTarget::Guard {
                building: b,
                index: 1,
            },
            game::Target::Guard(IceGrade::Grey),
            game::Op::Crack,
            now,
        );
        st.net.sync_guards(&mut st.constructs, b, now);
        let hide = room_tile(&st, b);
        stand(&mut st, &mut s, Place::Construct(b), hide);
        let at = game::At {
            place: Place::Construct(b),
            pos: hide,
        };
        st.character
            .unload(&st.content, &ItemId::new("copy"))
            .unwrap();
        st.character
            .unload(&st.content, &ItemId::new("crack"))
            .unwrap();
        crate::screens::console::run(&mut st, at, "program shield").unwrap();
        crate::screens::console::run(&mut st, at, "program attack").unwrap();
        // Shield on yourself never fails and holds one charge.
        crate::net::start_run(&mut st, ItemId::new("shield"), game::HackTarget::Deck).unwrap();
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(
            last(&st),
            Some("shield raised · 1 charge"),
            "{:?}",
            last(&st)
        );
        assert_eq!(st.net.shield, 1);
        // Two tiles up the spine from the patrolling guard: it sees you and comes
        // beside you. Then the shield takes its first hit and the attack breaks it.
        let gate = st.constructs.get(b).unwrap().guards[0].pos;
        let _ = post;
        stand(
            &mut st,
            &mut s,
            Place::Construct(b),
            Pos::new(gate.x + 2, gate.y),
        );
        let gate = st.constructs.get(b).unwrap().guards[0].pos;
        assert_eq!(
            gate.chebyshev(st.player.as_ref().unwrap().pos),
            1,
            "{:?}",
            last(&st)
        );
        let post = gate;
        let target = game::HackTarget::Guard {
            building: b,
            index: 0,
        };
        crate::net::start_run(&mut st, ItemId::new("attack"), target).unwrap();
        let focus = st.character.focus;
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        let log: Vec<String> = st
            .log
            .iter()
            .rev()
            .take(3)
            .map(|e| e.text.clone())
            .collect();
        assert!(log.iter().any(|l| l == "attack · grey ICE 4/6"), "{log:?}");
        assert!(log.iter().any(|l| l == "shield absorbs the hit"), "{log:?}");
        assert_eq!(st.character.focus, focus);
        assert_eq!(st.net.shield, 0);
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(st.character.focus, focus - 9);
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(last(&st), Some("grey ICE broken"), "{:?}", last(&st));
        assert!(st.net.run.is_none());
        let k = st.constructs.get(b).unwrap();
        assert_eq!(k.guard_at(k.guards[0].pos), None);
        assert!(k.tiles[k.guards[0].pos].is_walkable());
        let now = st.net.now(&st);
        st.net.sync_guards(&mut st.constructs, b, now);
        assert_eq!(
            st.constructs.get(b).unwrap().guard_at(post),
            None,
            "stays down"
        );
        // After the hour it stands where it was, whole again.
        st.net
            .minute(now + game::HACK_MINUTES, None, &mut st.constructs);
        let k = st.constructs.get(b).unwrap();
        assert_eq!(k.guard_at(k.guards[0].pos), Some(0));
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(st.net.ice[&b][0].integrity, 6, "whole again");
        // The RUN popup offers the deck programs on yourself.
        let ScreenCmd::Push { to: run, .. } = s.update(&mut st, &[key(KeyCode::Char(' '))]) else {
            panic!("space opens RUN")
        };
        st.tick += 40;
        let text = draw_screen(&*run, &st, 120, 40);
        assert!(text.contains("shield · yourself · 100% · 1 turn"), "{text}");
    }

    #[test]
    fn no_hit_lands_on_the_turn_you_jack_out() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let post = st.constructs.get(b).unwrap().guards[0].post;
        stand(
            &mut st,
            &mut s,
            Place::Construct(b),
            Pos::new(post.x - 1, post.y),
        );
        let focus = st.character.focus;
        assert!(focus < 30, "the first hit landed on the wait");
        s.update(&mut st, &[key(KeyCode::Enter)]);
        assert!(!st.player.as_ref().unwrap().jacked_in());
        assert_eq!(st.character.focus, focus, "{:?}", last(&st));
        assert_eq!(last(&st), Some("jacked out"));
    }

    #[test]
    fn black_ice_takes_hp_and_the_failsafe_pulls_you_out() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct_at(&mut st, &mut s, 230);
        let vault_guard = st.constructs.get(b).unwrap().guards[1].clone();
        assert_eq!(vault_guard.grade, IceGrade::Black);
        let beside_vault = Pos::new(vault_guard.post.x - 1, vault_guard.post.y);
        let hp_max = st.character.hp_max(&st.content);
        assert_eq!(st.character.hp, hp_max);
        stand(&mut st, &mut s, Place::Construct(b), beside_vault);
        // 6 + (230 − 230) / 5, less the deck's rating.
        assert_eq!(st.character.hp, hp_max - 5, "{:?}", last(&st));
        assert_eq!(
            last(&st),
            Some(format!("black ICE hits · hp {}", hp_max - 5).as_str())
        );
        // Already under a quarter: the next hit dumps, it does not kill.
        st.character.hp = hp_max / 4;
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(last(&st), Some("DUMPED · the failsafe pulled you out"));
        assert!(!st.player.as_ref().unwrap().jacked_in());
        assert_eq!(st.character.hp, hp_max / 4 - 5, "the hp stays lost");
        let now = st.net.now(&st);
        assert_eq!(st.net.cooling(now), Some(crate::net::FAILSAFE_COOLING));
        assert!(st.net.death.is_none());
    }

    #[test]
    fn with_the_failsafe_off_black_ice_kills_and_flatlined_leads_to_the_title() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct_at(&mut st, &mut s, 230);
        let name = st.city.as_ref().unwrap().building(b).name.clone().unwrap();
        let vault_guard = st.constructs.get(b).unwrap().guards[1].clone();
        let beside_vault = Pos::new(vault_guard.post.x - 1, vault_guard.post.y);
        st.net.failsafe = false;
        st.character.hp = 4;
        stand(&mut st, &mut s, Place::Construct(b), beside_vault);
        assert_eq!(st.character.hp, 0);
        assert_eq!(
            st.net.death.as_deref(),
            Some(format!("black ICE at {name}").as_str())
        );
        let ScreenCmd::Switch { to: dead, .. } = s.update(&mut st, &[]) else {
            panic!("death switches the screen")
        };
        st.tick += 40;
        let text = draw_screen(&*dead, &st, 120, 40);
        assert!(text.contains("FLATLINED") || text.contains('█'), "{text}");
        assert!(text.contains(&format!("black ICE at {name}")), "{text}");
        assert!(
            text.contains("VEX · level 1") && text.contains("any key"),
            "{text}"
        );
        let mut dead = dead;
        assert!(matches!(
            dead.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Switch { .. }
        ));
        assert!(st.net.death.is_none());
    }

    #[test]
    fn a_mask_halves_sight_for_six_turns_and_warms_the_trace() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        st.net.force_roll = Some(10);
        let hide = room_tile(&st, b);
        stand(&mut st, &mut s, Place::Construct(b), hide);
        let at = game::At {
            place: Place::Construct(b),
            pos: hide,
        };
        st.character
            .unload(&st.content, &ItemId::new("copy"))
            .unwrap();
        crate::screens::console::run(&mut st, at, "program mask").unwrap();
        crate::net::start_run(&mut st, ItemId::new("mask"), game::HackTarget::Deck).unwrap();
        let heat = st.net.traces.get(&b).map_or(0, |t| t.heat);
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(last(&st), Some("mask raised · 6 turns"), "{:?}", last(&st));
        assert_eq!(st.net.mask, crate::net::MASK_TURNS);
        // A guard four tiles up the spine does not see a masked runner (half of six).
        let g = st.constructs.get(b).unwrap().guards[0].clone();
        let four = Pos::new(g.pos.x + 4, g.pos.y);
        stand(&mut st, &mut s, Place::Construct(b), four);
        assert!(
            matches!(st.net.ice[&b][0].mode, crate::net::IceMode::Asleep),
            "{:?}",
            st.net.ice[&b][0].mode
        );
        stand(&mut st, &mut s, Place::Construct(b), hide);
        for _ in 0..4 {
            s.update(&mut st, &[key(KeyCode::Char('.'))]);
        }
        assert_eq!(st.net.mask, 0, "six turns and it is gone");
        assert_eq!(
            st.net.traces[&b].heat,
            heat + 1 + crate::net::MASK_TURNS,
            "one heat a held turn"
        );
        let text = draw_screen(&s, &st, 120, 40);
        assert!(!text.contains("MASK "), "{text}");
    }

    #[test]
    fn the_console_wakes_lists_and_toggles() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let hide = room_tile(&st, b);
        stand(&mut st, &mut s, Place::Construct(b), hide);
        let at = game::At {
            place: Place::Construct(b),
            pos: hide,
        };
        let listed = crate::screens::console::run(&mut st, at, "ice").unwrap();
        let g = st.constructs.get(b).unwrap().guards[0].pos;
        assert!(
            listed.starts_with(&format!("0 grey asleep 6/6 at {},{}", g.x, g.y)),
            "{listed}"
        );
        assert_eq!(
            crate::screens::console::run(&mut st, at, "wake").unwrap(),
            "2 pieces awake"
        );
        assert!(
            st.net.ice[&b]
                .iter()
                .all(|i| matches!(i.mode, crate::net::IceMode::Awake { .. }))
        );
        assert_eq!(
            crate::screens::console::run(&mut st, at, "failsafe off").unwrap(),
            "failsafe off"
        );
        assert!(!st.net.failsafe);
        assert_eq!(
            crate::screens::console::run(&mut st, at, "failsafe on").unwrap(),
            "failsafe on"
        );
        assert_eq!(
            crate::screens::console::run(&mut st, at, "failsafe").unwrap_err(),
            "failsafe on|off"
        );
        assert_eq!(
            crate::screens::console::run(&mut st, at, "hp 10").unwrap(),
            "hp 10/45"
        );
        assert_eq!(st.character.hp, 10);
    }

    #[test]
    fn an_attack_needs_the_piece_beside_you_and_wakes_it() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        st.net.force_roll = Some(10);
        let now = st.net.now(&st);
        st.net.land(
            b,
            game::HackTarget::Guard {
                building: b,
                index: 1,
            },
            game::Target::Guard(IceGrade::Grey),
            game::Op::Crack,
            now,
        );
        st.net.sync_guards(&mut st.constructs, b, now);
        // From a room tile diagonal to the spine, with the asleep gate guard parked on
        // the spine tile beside it: the attack lands and the piece wakes to you.
        let room = st.constructs.get(b).unwrap().rooms[0];
        let door_side = {
            let k = st.constructs.get(b).unwrap();
            room.iter()
                .find(|p| {
                    p.neighbors8().any(|n| {
                        k.tiles
                            .get(n)
                            .is_some_and(|t| t.terrain == citygen::Terrain::NetGate)
                    })
                })
                .unwrap()
        };
        let spine_tile = {
            let k = st.constructs.get(b).unwrap();
            door_side
                .neighbors8()
                .find(|n| {
                    k.tiles
                        .get(*n)
                        .is_some_and(|t| t.terrain == citygen::Terrain::NetGate)
                })
                .unwrap()
        };
        stand(&mut st, &mut s, Place::Construct(b), door_side);
        st.constructs.get_mut(b).unwrap().move_guard(0, spine_tile);
        st.net.ice.get_mut(&b).unwrap()[0].mode = crate::net::IceMode::Asleep;
        st.character
            .unload(&st.content, &ItemId::new("copy"))
            .unwrap();
        st.character.give(ItemId::new("attack"), 1);
        st.character
            .load(&st.content, &ItemId::new("attack"))
            .unwrap();
        let target = game::HackTarget::Guard {
            building: b,
            index: 0,
        };
        crate::net::start_run(&mut st, ItemId::new("attack"), target).unwrap();
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(st.net.ice[&b][0].integrity, 4, "{:?}", last(&st));
        assert!(
            matches!(st.net.ice[&b][0].mode, crate::net::IceMode::Awake { seen, .. } if seen == door_side),
            "a hit piece turns on you: {:?}",
            st.net.ice[&b][0].mode
        );
        // Walked away: the attack cannot reach and ends.
        let far = st.constructs.get(b).unwrap().guards[0].post;
        st.constructs.get_mut(b).unwrap().move_guard(0, far);
        st.net.ice.get_mut(&b).unwrap()[0].mode = crate::net::IceMode::Asleep;
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(last(&st), Some("attack · out of reach"), "{:?}", last(&st));
        assert!(st.net.run.is_none());
        assert_eq!(st.net.ice[&b][0].integrity, 4);
    }

    #[test]
    fn death_ends_the_runner_and_the_nets_session() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct_at(&mut st, &mut s, 230);
        let vault_guard = st.constructs.get(b).unwrap().guards[1].clone();
        let beside_vault = Pos::new(vault_guard.post.x - 1, vault_guard.post.y);
        st.net.failsafe = false;
        st.character.hp = 4;
        stand(&mut st, &mut s, Place::Construct(b), beside_vault);
        assert!(st.net.death.is_some());
        // No piece acts on a corpse: no dump, the body stays where it fell this frame.
        assert_eq!(last(&st), Some("black ICE hits · hp 0"));
        let ScreenCmd::Switch { to: mut dead, .. } = s.update(&mut st, &[]) else {
            panic!("death switches the screen")
        };
        st.tick += 40;
        dead.update(&mut st, &[key(KeyCode::Enter)]);
        // The Net's session and the constructs are gone with the runner; a fresh
        // runner stands ready for the next game.
        assert!(st.net.ice.is_empty() && st.net.traces.is_empty() && st.net.failsafe);
        assert_eq!(st.constructs.generated(), 0);
        assert!(st.character.hp > 0 && !st.character.loaded.is_empty());
        assert_eq!(st.character.handle, "VEX");
    }

    #[test]
    fn a_piece_does_not_stand_up_under_another() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let hide = room_tile(&st, b);
        stand(&mut st, &mut s, Place::Construct(b), hide);
        let now = st.net.now(&st);
        st.net.land(
            b,
            game::HackTarget::Guard {
                building: b,
                index: 0,
            },
            game::Target::Guard(IceGrade::Grey),
            game::Op::Attack,
            now,
        );
        st.net.sync_guards(&mut st.constructs, b, now);
        let spot = st.constructs.get(b).unwrap().guards[0].pos;
        st.constructs.get_mut(b).unwrap().move_guard(1, spot);
        st.net
            .minute(now + game::HACK_MINUTES, None, &mut st.constructs);
        let k = st.constructs.get(b).unwrap();
        assert_eq!(
            k.guard_at(spot),
            Some(1),
            "the standing piece keeps the tile"
        );
        assert_eq!(k.guards[0].pos, spot);
        let away = k.guards[1].post;
        st.constructs.get_mut(b).unwrap().move_guard(1, away);
        st.net
            .sync_guards(&mut st.constructs, b, now + game::HACK_MINUTES);
        let k = st.constructs.get(b).unwrap();
        assert_eq!(k.guard_at(spot), Some(0), "and stands once it is free");
    }

    #[test]
    fn a_small_map_sits_centred_and_does_not_slide() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let size = st.constructs.get(b).unwrap().size;
        let centre = Pos::new(size.w as i32 / 2, size.h as i32 / 2);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        // The helper put the player inside by hand; a frame on the grid, then the
        // screen notices the change of place the way it does in play.
        s.place = Place::Stratum(StratumKind::Net);
        t.draw(|f| s.draw(&st, f)).unwrap();
        s.update(&mut st, &[]);
        t.draw(|f| s.draw(&st, f)).unwrap();
        let area = s.last_map.get();
        let want = (area.x + area.width / 2, area.y + area.height / 2);
        let at = |s: &MapScreen| {
            let origin = s.cam.origin(area.width, area.height);
            tile_to_screen(area, origin, centre)
        };
        let first = at(&s).expect("the centre is on screen");
        assert!(
            (first.0 as i32 - want.0 as i32).abs() <= 1
                && (first.1 as i32 - want.1 as i32).abs() <= 1,
            "{first:?} vs {want:?}"
        );
        for _ in 0..12 {
            st.tick += 1;
            s.update(&mut st, &[]);
            t.draw(|f| s.draw(&st, f)).unwrap();
            assert_eq!(at(&s), Some(first), "the map must not slide");
        }
        // Walking inside keeps it put too.
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(at(&s), Some(first));
    }

    #[test]
    fn the_void_around_a_small_map_reads_as_its_world() {
        // A construct floats on the Net's lattice; a floor sits in the building's mass.
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        s.place = Place::Stratum(StratumKind::Net);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        s.update(&mut st, &[]);
        t.draw(|f| s.draw(&st, f)).unwrap();
        let area = s.last_map.get();
        let origin = s.cam.origin(area.width, area.height);
        let size = st.constructs.get(b).unwrap().size;
        let dots = (area.y..area.bottom())
            .flat_map(|sy| (area.x..area.right()).map(move |sx| (sx, sy)))
            .filter(|(sx, sy)| {
                let p = crate::render::viewport::screen_to_tile(area, origin, (*sx, *sy));
                p.x < 0 || p.y < 0 || p.x >= size.w as i32 || p.y >= size.h as i32
            })
            .filter(|(sx, sy)| t.backend().buffer()[(*sx, *sy)].symbol() == "·")
            .count();
        assert!(
            dots > 20,
            "the lattice shows around the construct: {dots} dots"
        );
        // A floor: the void is the building's mass, not black.
        let city = st.city.as_ref().unwrap().clone();
        let tower = tower(&st);
        st.interiors.ensure(&city, tower, 0).unwrap();
        let (_, lobby) = st.interiors.floor(tower, 0).unwrap();
        let step = lobby.doorstep(&lobby.exits[0]);
        {
            let p = st.player.as_mut().unwrap();
            p.body = None;
        }
        stand(
            &mut st,
            &mut s,
            Place::Floor {
                building: tower,
                level: 0,
            },
            step,
        );
        t.draw(|f| s.draw(&st, f)).unwrap();
        let corner = t.backend().buffer()[(area.right() - 1, area.bottom() - 1)].clone();
        assert_ne!(
            corner.symbol(),
            " ",
            "the corner outside a floor is not blank"
        );
    }

    #[test]
    #[ignore]
    fn fuzz_every_construct_entry_and_a_few_turns() {
        for (seed, size, districts) in [
            (8u64, SizePreset::Medium, 7u8),
            (6, SizePreset::Custom { w: 120, h: 60 }, 8),
            (3, SizePreset::Custom { w: 160, h: 80 }, 6),
        ] {
            let mut st = AppState::new();
            st.city = Some(
                Generator::run(GenParams {
                    seed,
                    size,
                    districts,
                    ..Default::default()
                })
                .unwrap(),
            );
            let mut s = MapScreen::explore(0);
            walking(&mut st, &mut s);
            let city = st.city.as_ref().unwrap().clone();
            let mut t = Terminal::new(TestBackend::new(220, 60)).unwrap();
            for b in &city.buildings {
                {
                    let p = st.player.as_mut().unwrap();
                    p.body = Some((Place::Stratum(StratumKind::Street), p.pos));
                    p.place = Place::Stratum(StratumKind::Net);
                }
                s.place = Place::Stratum(StratumKind::Net);
                t.draw(|f| s.draw(&st, f)).unwrap();
                if crate::constructs::enter_construct(&mut st, b.id).is_err() {
                    let p = st.player.as_mut().unwrap();
                    p.body = None;
                    continue;
                }
                s.update(&mut st, &[]);
                t.draw(|f| s.draw(&st, f)).unwrap();
                for k in [
                    KeyCode::Char('.'),
                    KeyCode::Char('l'),
                    KeyCode::Char('.'),
                    KeyCode::Char(' '),
                ] {
                    if let ScreenCmd::Push { to: mut popup, .. } = s.update(&mut st, &[key(k)]) {
                        t.draw(|f| popup.draw(&st, f)).unwrap();
                        popup.update(&mut st, &[key(KeyCode::Esc)]);
                    }
                    t.draw(|f| s.draw(&st, f)).unwrap();
                }
                let p = st.player.as_mut().unwrap();
                p.body = None;
            }
            eprintln!("seed {seed}: {} buildings ok", city.buildings.len());
        }
    }
}
