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
use crate::ui::chrome::{CYAN, MAGENTA, put};
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

/// A walk in flight: the tiles still to step, and when the next one falls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Walk {
    pub path: Vec<Pos>,
    pub next_at: u64,
}

/// A program key pressed: the target is being picked on the map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Targeting {
    pub program: game::ItemId,
    /// The strip slot it came from, for the flash.
    pub slot: usize,
    /// What the program may run against, nearest first: target, tile, name.
    pub candidates: Vec<(game::HackTarget, Pos, String)>,
    pub picked: usize,
}

pub struct MapScreen {
    pub mode: Mode,
    /// The map on screen: a stratum, or the floor the player walks.
    pub place: Place,
    pub cam: Camera,
    /// The tile under inspection in Explore mode; the camera follows it.
    pub cursor: Pos,
    /// The tile a walk is aimed at, in a construct.
    pub aim: Option<Pos>,
    /// The aimed walk being stepped, a tile per `WALK_TICKS`; keys wait for it.
    pub walking: Option<Walk>,
    /// A program waiting for its target.
    pub targeting: Option<Targeting>,
    pub digital: bool,
    pub help: bool,
    fps: FpsMeter,
    /// The dev console while it is open; it takes every key.
    console: Option<Console>,
    /// Tick of the last update; a gap means a menu was on top.
    last_tick: u64,
    /// The program slot fired a moment ago and when, for the strip's flash.
    program_flash: Option<(usize, u64)>,
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
    footer: [Rect; 4],
}

fn layout(area: Rect) -> Regions {
    let r = sidebar::width(area.width);
    let l = area.width.saturating_sub(r + 1);
    let x_rule = area.x + l;
    let row = |y: u16, x: u16, w: u16| Rect::new(x, area.y + y, w, 1);
    let main_y = area.y + 5;
    let main_h = area.height.saturating_sub(10);
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
        bottom_rule: row(area.height.saturating_sub(5), area.x, area.width),
        footer: [
            row(area.height.saturating_sub(4), area.x, area.width),
            row(area.height.saturating_sub(3), area.x, area.width),
            row(area.height.saturating_sub(2), area.x, area.width),
            row(area.height.saturating_sub(1), area.x, area.width),
        ],
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
            aim: None,
            walking: None,
            targeting: None,
            digital: false,
            help: false,
            fps: FpsMeter::default(),
            console: None,
            last_tick: 0,
            program_flash: None,
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

    /// What the footer reads this frame.
    fn footer<'a>(
        &self,
        st: &AppState,
        city: &citygen::City,
        jacked: bool,
        slots: &'a [hud::Slot],
    ) -> hud::Footer<'a> {
        let place = match (jacked, st.player.as_ref().map(|p| p.place)) {
            (true, Some(Place::Construct(b))) => {
                let district = city
                    .building_opt(b)
                    .map(|bl| city.district(bl.district).name.clone())
                    .unwrap_or_else(|| city.name.clone());
                format!("{} · {district}", crate::net::building_name(st, b))
            }
            (true, Some(_)) => {
                let pos = st.player.as_ref().map(|q| q.pos).unwrap_or(Pos::new(0, 0));
                format!("{} · grid", crate::net::district_name(city, pos))
            }
            _ => self.place_label(st),
        };
        let now = st.net.now(st);
        let trace = match (jacked, st.player.as_ref()) {
            (true, Some(p)) => Some(match p.place {
                Place::Construct(b) => {
                    let heat = st.net.traces.get(&b).map_or(0, |t| t.heat);
                    let mut t = format!("TRACE {} {heat}", crate::net::trace_bar(heat));
                    if let Some(until) = st
                        .net
                        .traces
                        .get(&b)
                        .and_then(|t| t.alert_until)
                        .filter(|u| now < *u)
                    {
                        t.push_str(&format!(" · ALERT {} min", until - now));
                    }
                    if st.net.shield > 0 {
                        t.push_str(&format!(" · SHIELD ×{}", st.net.shield));
                    }
                    if st.net.mask > 0 {
                        t.push_str(&format!(" · MASK {}", st.net.mask));
                    }
                    t
                }
                _ => match st.net.nearest_trace(city, p.pos) {
                    Some((b, heat)) => format!(
                        "TRACE {} {heat} · {}",
                        crate::net::trace_bar(heat),
                        crate::net::building_name(st, b)
                    ),
                    None => "no trace in reach".to_string(),
                },
            }),
            _ => None,
        };
        let run = match (
            jacked,
            st.net.run.as_ref(),
            st.player.as_ref().map(|p| p.place),
        ) {
            (true, Some(run), Some(Place::Construct(b))) => crate::net::run_readout(st, b, run),
            _ => None,
        };
        let ice = match (jacked, st.player.as_ref()) {
            (true, Some(p)) => match p.place {
                Place::Construct(b) => {
                    let vis = p.vis();
                    crate::net::ice_lines(st, b, |q| {
                        vis.get(q)
                            .is_some_and(|v| *v != crate::render::Visibility::Unseen)
                    })
                }
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };
        hud::Footer {
            place,
            jacked,
            camera: self.mode == Mode::Explore,
            overburdened: st.character.overburdened(&st.content),
            trace,
            hunted: jacked && crate::net::hunted(st),
            turn: crate::net::turn_readout(st),
            run,
            ice,
            slots_label: if jacked { "PROGRAMS" } else { "ABILITIES" },
            slots,
            hollow: !crate::net::your_turn(st)
                || crate::net::phase(st) == Some(crate::net::Phase::Move),
            acting: crate::net::phase(st).map(|p| p == crate::net::Phase::Act),
            prompt: self.turn_prompt(st),
            budgets: st.net.engaged.as_ref().map(|e| {
                if e.actor() == crate::net::Actor::You {
                    (e.moves, crate::net::BASE_MOVE, e.ap, crate::net::BASE_AP)
                } else {
                    (
                        crate::net::BASE_MOVE,
                        crate::net::BASE_MOVE,
                        crate::net::BASE_AP,
                        crate::net::BASE_AP,
                    )
                }
            }),
        }
    }

    /// Row 2 of the footer on turns: whose turn, which half, and the keys for it.
    fn turn_prompt(&self, st: &AppState) -> Option<(String, String)> {
        use crate::net::{Actor, Phase};
        if let Some(t) = self.targeting.as_ref() {
            let (_, _, what) = &t.candidates[t.picked];
            let op = st
                .content
                .item(&t.program)
                .and_then(|i| i.program)
                .map(|p| p.op.label().to_uppercase())
                .unwrap_or_else(|| "RUN".to_string());
            let cost = if st.net.engaged.is_some() {
                " (1 AP)"
            } else {
                ""
            };
            return Some((
                format!("{op} › {what}"),
                format!("tab next target · space runs it{cost} · esc cancels"),
            ));
        }
        let e = st.net.engaged.as_ref()?;
        Some(match e.actor() {
            Actor::You => match e.phase {
                Phase::Move => (
                    "YOUR TURN · MOVE".to_string(),
                    "arrows aim · space or enter walks · space on yourself stays".to_string(),
                ),
                // Act: pick a program (and a target when several fit), then space
                // runs it for an action point; `.` ends the turn.
                Phase::Act => (
                    "YOUR TURN · ACT".to_string(),
                    match st
                        .net
                        .run
                        .as_ref()
                        .and_then(|r| crate::net::run_label(st, r))
                    {
                        Some(label) => {
                            format!(
                                "space runs {label} (1 AP) · 1-5 another program · . ends the turn"
                            )
                        }
                        None => "1-5 pick a program · then space runs it (1 AP) · . ends the turn"
                            .to_string(),
                    },
                ),
            },
            Actor::Ice(i) => {
                let grade = st
                    .constructs
                    .get(e.building)
                    .and_then(|k| k.guards.get(i))
                    .map(|g| g.grade.label())
                    .unwrap_or("the");
                (format!("{grade} ICE ACTS"), "wait".to_string())
            }
        })
    }

    /// The strip's slots: the deck's programs while jacked in, abilities otherwise.
    fn slots(&self, st: &AppState) -> Vec<hud::Slot> {
        if st.player.as_ref().is_some_and(|p| p.jacked_in()) {
            return st
                .character
                .loaded
                .iter()
                .enumerate()
                .filter_map(|(i, id)| {
                    let spec = st.content.item(id)?.program?;
                    let state = hud::SlotState::Ready;
                    let flash = self.program_flash.is_some_and(|(slot, at)| {
                        slot == i && st.tick.saturating_sub(at) < hud::FLASH_TICKS
                    });
                    Some(hud::Slot {
                        name: spec.op.label().to_string(),
                        state,
                        flash,
                    })
                })
                .take(hud::SLOTS)
                .collect();
        }
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
        let bonus = st.sight_bonus();
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
        // Inside a construct: the programs that apply to the node or the ICE at hand.
        let mut title = "HERE";
        if let Place::Construct(b) = place
            && st.character.deck(&st.content).is_some()
        {
            let rows = Self::run_choices(st, b, pos, None, &mut letters);
            if !rows.is_empty() {
                title = "RUN";
            }
            choices.extend(rows);
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

    /// One RUN row per program that applies to a target in reach, with the odds;
    /// `only` keeps it to one program.
    fn run_choices(
        st: &AppState,
        b: citygen::BuildingId,
        pos: Pos,
        only: Option<&game::ItemId>,
        letters: &mut std::str::Chars,
    ) -> Vec<Choice> {
        let mut choices = Vec::new();
        let now = st.net.now(st);
        let security = st.constructs.get(b).map(|k| k.security).unwrap_or(0);
        for (target, what) in crate::net::targets_near(st, b, pos) {
            let Some(kind) = crate::net::target_kind(st, b, target) else {
                continue;
            };
            for item in st.character.loaded_programs(&st.content) {
                if only.is_some_and(|id| *id != item.id) {
                    continue;
                }
                let Some(spec) = item.program else { continue };
                if !game::applies(spec.op, kind) {
                    continue;
                }
                let Some(key) = letters.next() else { break };
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
        choices
    }

    /// A number key jacked in: the deck's program in that slot, run at once against
    /// the one target it applies to, or the short list when there are several.
    fn program_key(&mut self, st: &mut AppState, n: usize) -> ScreenCmd {
        let Some((place, pos)) = st.player.as_ref().map(|p| (p.place, p.pos)) else {
            return ScreenCmd::Stay;
        };
        let Place::Construct(b) = place else {
            st.warn("programs run inside a construct");
            return ScreenCmd::Stay;
        };
        if st.character.deck(&st.content).is_none() {
            st.warn("no deck jacked");
            return ScreenCmd::Stay;
        }
        let Some(id) = st.character.loaded.get(n - 1).cloned() else {
            st.warn(format!("nothing in slot {n}"));
            return ScreenCmd::Stay;
        };
        let Some(spec) = st.content.item(&id).and_then(|i| i.program) else {
            return ScreenCmd::Stay;
        };
        // Your turn's move half: programs wait for the act half.
        if crate::net::phase(st) == Some(crate::net::Phase::Move) {
            st.warn("move first · space to stay");
            return ScreenCmd::Stay;
        }
        // What it may run against, nearest first; the pick waits for space.
        let now = st.net.now(st);
        let mut copied = false;
        let mut candidates = Vec::new();
        for (target, what) in crate::net::targets_near(st, b, pos) {
            let Some(kind) = crate::net::target_kind(st, b, target) else {
                continue;
            };
            if !game::applies(spec.op, kind) {
                continue;
            }
            if spec.op == game::Op::Copy
                && matches!(target, game::HackTarget::Node(id) if st.net.copied(id, now))
            {
                copied = true;
                continue;
            }
            let Some(at) = crate::net::target_pos(st, b, target) else {
                continue;
            };
            candidates.push((target, at, what));
        }
        if candidates.is_empty() {
            st.warn(if copied {
                format!("{} · copied already", spec.op.label())
            } else {
                format!("{} · nothing to run it against", spec.op.label())
            });
            return ScreenCmd::Stay;
        }
        candidates.sort_by_key(|(_, at, _)| at.chebyshev(pos));
        self.aim = None;
        self.targeting = Some(Targeting {
            program: id,
            slot: n - 1,
            candidates,
            picked: 0,
        });
        ScreenCmd::Stay
    }

    /// Keys while a target is being picked: next and previous, space or enter runs
    /// it (an action point on turns), esc drops it, another number picks another
    /// program, `.` drops it and goes on as the wait key.
    fn handle_targeting(&mut self, st: &mut AppState, k: &KeyEvent) -> ScreenCmd {
        let Some(t) = self.targeting.as_mut() else {
            return ScreenCmd::Stay;
        };
        let n = t.candidates.len();
        match k.code {
            KeyCode::Tab | KeyCode::Right | KeyCode::Down | KeyCode::Char('l' | 'j' | 'n') => {
                t.picked = (t.picked + 1) % n
            }
            KeyCode::BackTab | KeyCode::Left | KeyCode::Up | KeyCode::Char('h' | 'k' | 'p') => {
                t.picked = (t.picked + n - 1) % n
            }
            KeyCode::Char(' ') | KeyCode::Enter => return self.confirm_target(st),
            KeyCode::Esc => self.targeting = None,
            KeyCode::Char(c) if c.is_ascii_digit() => {
                self.targeting = None;
                if let Some(d) = c.to_digit(10)
                    && (1..=hud::SLOTS as u32).contains(&d)
                {
                    return self.program_key(st, d as usize);
                }
            }
            KeyCode::Char('.') => {
                self.targeting = None;
                return self.handle_walk(st, k).unwrap_or(ScreenCmd::Stay);
            }
            _ => {}
        }
        ScreenCmd::Stay
    }

    /// The picked target: the run is armed and rolled once, for an action point on
    /// turns.
    fn confirm_target(&mut self, st: &mut AppState) -> ScreenCmd {
        let Some(t) = self.targeting.take() else {
            return ScreenCmd::Stay;
        };
        let (target, _, _) = t.candidates[t.picked].clone();
        if st.net.engaged.is_some()
            && let Err(why) = crate::net::spend_ap(st)
        {
            st.warn(why);
            self.targeting = Some(t);
            return ScreenCmd::Stay;
        }
        match crate::net::start_run(st, t.program.clone(), target) {
            Ok(line) => {
                st.say(line);
                self.program_flash = Some((t.slot, st.tick));
                crate::net::step_run(st);
            }
            Err(why) => {
                // Nothing ran: the point comes back.
                if let Some(e) = st.net.engaged.as_mut() {
                    e.ap = (e.ap + 1).min(crate::net::BASE_AP);
                }
                st.warn(why);
            }
        }
        ScreenCmd::Stay
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

    /// Tiles from `pos` to the construct's entry, none when it cannot be reached.
    fn steps_to_entry(k: &citygen::Construct, pos: Pos) -> Option<usize> {
        crate::net::path_to(k, pos, k.entry).map(|p| p.len())
    }

    /// The path from the player to the aim and how many tiles the moves left cover;
    /// `None` when nothing reaches the aim. An aim off the map past the entry is
    /// the gate: the path runs to the entry and one tile out.
    pub fn aim_path(&self, st: &AppState) -> Option<(Vec<Pos>, usize)> {
        let aim = self.aim?;
        let player = st.player.as_ref()?;
        let Place::Construct(b) = player.place else {
            return None;
        };
        let k = st.constructs.get(b)?;
        let path = if k.tiles.in_bounds(aim) {
            crate::net::path_to(k, player.pos, aim)?
        } else {
            let mut path = crate::net::path_to(k, player.pos, k.entry)?;
            path.push(aim);
            path
        };
        let lit = match st.net.engaged.as_ref() {
            Some(e) => e.moves as usize,
            None => path.len(),
        };
        Some((path, lit))
    }

    /// Enter on the aim: the walk starts if the moves left reach it.
    fn start_walk(&mut self, st: &mut AppState, aim: Pos) -> ScreenCmd {
        let Some((path, lit)) = self.aim_path(st) else {
            st.warn("no way there");
            return ScreenCmd::Stay;
        };
        if path.len() > lit {
            if lit == 0 {
                st.warn("no moves left");
            } else {
                st.warn(format!(
                    "too far · {lit} move{}",
                    if lit == 1 { "" } else { "s" }
                ));
            }
            return ScreenCmd::Stay;
        }
        let _ = aim;
        self.aim = None;
        self.walking = Some(Walk {
            path,
            next_at: st.clock_tick() + crate::net::WALK_TICKS,
        });
        ScreenCmd::Stay
    }

    /// One tile of the walk in flight when its time has come: a move spent in a
    /// fight, sight refreshed, the run aborted as any step aborts it.
    fn walk_step(&mut self, st: &mut AppState) {
        let Some(w) = self.walking.as_ref() else {
            return;
        };
        if st.clock_tick() < w.next_at {
            return;
        }
        let Some(&next) = w.path.first() else {
            self.walking = None;
            return;
        };
        let Some((Place::Construct(b), _)) = st.player.as_ref().map(|p| (p.place, p.pos)) else {
            self.walking = None;
            return;
        };
        let here = st.player.as_ref().map(|p| p.pos);
        // The last tile off the map from the entry is the gate: out onto the grid.
        let leaving = st
            .constructs
            .get(b)
            .is_some_and(|k| !k.tiles.in_bounds(next) && here == Some(k.entry));
        if leaving {
            self.walking = None;
            if let Err(why) = crate::net::spend_move(st) {
                st.warn(why);
                return;
            }
            let result = crate::constructs::leave_construct(st);
            report(st, result);
            self.follow_player(st);
            return;
        }
        let open = here.is_some_and(|me| me.chebyshev(next) <= 1)
            && st.constructs.get(b).is_some_and(|k| {
                k.tiles.get(next).is_some_and(|t| t.is_walkable()) && k.guard_at(next).is_none()
            });
        if !open {
            self.walking = None;
            st.warn("the way is blocked");
            return;
        }
        if let Err(why) = crate::net::spend_move(st) {
            self.walking = None;
            st.warn(why);
            return;
        }
        let bonus = st.sight_bonus();
        if let Some(city) = st.city.as_ref()
            && let Some(map) =
                MapRef::for_place(city, &st.interiors, &st.constructs, Place::Construct(b))
            && let Some(player) = st.player.as_mut()
        {
            player.pos = next;
            player.update_fov(&map, bonus);
        }
        if st.net.run.take().is_some() {
            st.say("run aborted");
        }
        let now = st.clock_tick();
        if let Some(w) = self.walking.as_mut() {
            w.path.remove(0);
            if w.path.is_empty() {
                self.walking = None;
            } else {
                w.next_at = now + crate::net::WALK_TICKS;
            }
        }
        // The last move spent: the act half opens by itself.
        if st.net.engaged.as_ref().is_some_and(|e| e.moves == 0) && crate::net::stay(st) {
            self.walking = None;
        }
    }

    fn handle_walk(&mut self, st: &mut AppState, k: &KeyEvent) -> Option<ScreenCmd> {
        let before = st.player.as_ref().map(|p| (p.place, p.pos));
        let cmd = self.handle_walk_inner(st, k);
        let after = st.player.as_ref().map(|p| (p.place, p.pos));
        // In the Net a move aborts the run in flight; the wait key hurries the next
        // beat, which is when the run rolls and the ICE moves.
        if after != before && st.net.run.take().is_some() {
            st.say("run aborted");
        }
        if k.code == KeyCode::Char('.')
            && st
                .player
                .as_ref()
                .is_some_and(|p| matches!(p.place, Place::Construct(_)))
        {
            if st.net.engaged.is_some() {
                // Only your own turn ends on the key; the ICE's turns run on the pace.
                if crate::net::your_turn(st) {
                    crate::net::end_turn(st);
                }
            } else {
                st.net.beat = st.clock_tick();
                crate::net::beat(st);
            }
        }
        if st.player.as_ref().map(|p| p.place) != before.map(|b| b.0) {
            self.follow_player(st);
        }
        cmd
    }

    fn handle_walk_inner(&mut self, st: &mut AppState, k: &KeyEvent) -> Option<ScreenCmd> {
        let jacked = st.player.as_ref().is_some_and(|p| p.jacked_in());
        if k.code == KeyCode::Char(' ') {
            // An aim on another tile: space confirms the walk, as enter does.
            if let Some(aim) = self.aim
                && st.player.as_ref().is_some_and(|p| p.pos != aim)
                && matches!(
                    st.player.as_ref().map(|p| p.place),
                    Some(Place::Construct(_))
                )
            {
                return Some(self.start_walk(st, aim));
            }
            // On your turn's move half, space stays put and opens the act half.
            if crate::net::stay(st) {
                self.aim = None;
                st.say("staying put · act");
                return Some(ScreenCmd::Stay);
            }
            // A run in flight: space rolls it, an action point in a fight, the next
            // beat hurried in calm.
            if jacked && st.net.run.is_some() {
                if st.net.engaged.is_some() {
                    if let Err(why) = crate::net::spend_ap(st) {
                        st.warn(why);
                        return Some(ScreenCmd::Stay);
                    }
                    crate::net::step_run(st);
                } else {
                    st.net.beat = st.clock_tick();
                    crate::net::beat(st);
                }
                return Some(ScreenCmd::Stay);
            }
            return self.interact(st);
        }
        // `e`: turns by hand, and back to the beat when nothing hunts you.
        if k.code == KeyCode::Char('e') {
            match st.player.as_ref().map(|p| p.place) {
                Some(Place::Construct(b)) => match st.net.engaged.as_ref() {
                    None => crate::net::engage(st, b, true),
                    Some(e) if e.actor() != crate::net::Actor::You => st.warn("not your turn"),
                    Some(_) if crate::net::any_awake(st, b) => st.warn("still hunted"),
                    Some(_) => crate::net::disengage(st),
                },
                _ => st.warn("nothing to engage"),
            }
            return Some(ScreenCmd::Stay);
        }
        // Not your turn: the board belongs to the ICE.
        if st
            .net
            .engaged
            .as_ref()
            .is_some_and(|e| e.actor() != crate::net::Actor::You)
            && !matches!(
                k.code,
                KeyCode::Esc | KeyCode::Char('q' | '?' | 'm' | ':' | '`')
            )
        {
            st.warn("not your turn");
            return Some(ScreenCmd::Stay);
        }
        if k.code == KeyCode::Enter
            && st.player.as_ref().is_some_and(|p| !p.jacked_in())
            && let Some(why) = crate::net::jack_in_refusal(st)
        {
            st.warn(why);
            return Some(ScreenCmd::Stay);
        }
        let moving = crate::net::phase(st) == Some(crate::net::Phase::Move);
        if k.code == KeyCode::Char('a') {
            if moving {
                st.warn("move first · space to stay");
                return Some(ScreenCmd::Stay);
            }
            return Some(self.abilities(st));
        }
        if let KeyCode::Char(c) = k.code
            && let Some(n) = c.to_digit(10)
            && (1..=hud::SLOTS as u32).contains(&n)
        {
            if jacked {
                return Some(self.program_key(st, n as usize));
            }
            match Self::actives(st).get(n as usize - 1).cloned() {
                Some(id) => {
                    let result = perform(st, game::Action::Fire(id));
                    report(st, result);
                }
                None => st.warn(format!("nothing in slot {n}")),
            }
            return Some(ScreenCmd::Stay);
        }
        // The aim: `g` starts it anywhere in a construct, the arrows move it while it
        // is up or while the board is on turns, enter walks it, esc drops it.
        let inside = match st.player.as_ref().map(|p| (p.place, p.pos)) {
            Some((Place::Construct(b), pos)) => Some((b, pos)),
            _ => None,
        };
        if let Some((b, pos)) = inside {
            let acting = crate::net::phase(st) == Some(crate::net::Phase::Act);
            if k.code == KeyCode::Char('g') {
                if acting {
                    st.warn("moves are done");
                } else {
                    self.aim = Some(pos);
                }
                return Some(ScreenCmd::Stay);
            }
            if let Some(dir) = dir_for(k.code)
                && (self.aim.is_some() || st.net.engaged.is_some())
            {
                if acting {
                    st.warn("moves are done");
                    return Some(ScreenCmd::Stay);
                }
                // The aim reaches as far as the moves do; a piece within that reach
                // may be aimed at (no path: a red cross).
                let from = self.aim.unwrap_or(pos);
                let to = from.step(dir);
                let cap = crate::net::aim_cap(st);
                let open = st.constructs.get(b).is_some_and(|k| {
                    crate::net::path_to(k, pos, to).is_some_and(|p| p.len() <= cap)
                        || (k.guard_at(to).is_some() && pos.chebyshev(to) as usize <= cap)
                        // Off the map from the entry: the gate, one tile past it.
                        || (from == k.entry
                            && !k.tiles.in_bounds(to)
                            && Self::steps_to_entry(k, pos).is_some_and(|n| n < cap))
                });
                if open {
                    self.aim = Some(to);
                }
                return Some(ScreenCmd::Stay);
            }
            if k.code == KeyCode::Enter
                && let Some(aim) = self.aim
                && aim != pos
            {
                return Some(self.start_walk(st, aim));
            }
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
        let bonus = st.sight_bonus();
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
            // An aim up: esc drops it and nothing else. A walk in flight: the keys
            // wait for it, but esc still pauses.
            if self.mode == Mode::Walk && self.console.is_none() {
                if k.code == KeyCode::Esc && self.aim.is_some() {
                    self.aim = None;
                    continue;
                }
                if self.walking.is_some() && k.code != KeyCode::Esc {
                    continue;
                }
                // A target being picked takes every key.
                if self.targeting.is_some() {
                    let cmd = self.handle_targeting(st, k);
                    if !matches!(cmd, ScreenCmd::Stay) {
                        return cmd;
                    }
                    continue;
                }
            }
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
        // The beat of a calm construct: one per frame at most, so a long menu is
        // never a burst of rolls.
        let was_engaged = st.net.engaged.is_some();
        if self.mode == Mode::Walk && inside.is_some() {
            if st.net.engaged.is_some() {
                crate::net::ice_pace(st);
            } else if st.clock_tick() >= st.net.beat + crate::net::BEAT_TICKS {
                st.net.beat = st.clock_tick();
                crate::net::beat(st);
            }
        }
        // Out of the construct, out of the fight.
        if inside.is_none() && st.net.engaged.is_some() {
            crate::net::disengage(st);
        }
        // The walk: it stops where it is when a fight starts under it or the turn
        // is no longer yours, else it steps on its ticks.
        if self.walking.is_some()
            && ((st.net.engaged.is_some() && !was_engaged) || !crate::net::your_turn(st))
        {
            self.walking = None;
            self.aim = None;
            st.say("the walk stops");
        }
        if inside.is_none() {
            self.aim = None;
            self.walking = None;
            self.targeting = None;
        }
        // The turn left you while you were picking: nothing to confirm now.
        if self.targeting.is_some() && !crate::net::your_turn(st) {
            self.targeting = None;
        }
        if self.mode == Mode::Walk {
            self.walk_step(st);
        }
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
        let ghost = match self.place {
            Place::Construct(b) => Some(crate::render::ghost::GhostOverlay {
                pieces: crate::net::ghost_pieces(st, b),
                vis,
            }),
            _ => None,
        };
        if let Some(o) = ghost.as_ref() {
            overlays.push(o);
        }
        let aimed = self.aim_path(st);
        let path = self.aim.map(|aim| crate::render::path::PathOverlay {
            path: aimed.as_ref().map_or(&[][..], |(p, _)| p.as_slice()),
            lit: aimed.as_ref().map_or(0, |(_, lit)| *lit),
            aim,
            reachable: aimed.is_some(),
        });
        if let Some(o) = path.as_ref() {
            overlays.push(o);
        }
        let picks: Vec<Pos> = self
            .targeting
            .as_ref()
            .map(|t| t.candidates.iter().map(|c| c.1).collect())
            .unwrap_or_default();
        let targets = self
            .targeting
            .as_ref()
            .map(|t| crate::render::path::TargetOverlay {
                candidates: &picks,
                picked: t.candidates[t.picked].1,
            });
        if let Some(o) = targets.as_ref() {
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
        // Footer: four rows, the same four in every state.
        let jacked = st.player.as_ref().is_some_and(|p| p.jacked_in());
        let slots = self.slots(st);
        let footer = self.footer(st, city, jacked, &slots);
        let rows = hud::footer_rows(&footer, area.width);
        let recent = st.log.recent(st.tick);
        for (i, row) in rows.into_iter().enumerate() {
            let row = match (&self.console, i) {
                (Some(con), 3) => console::line(con, st.tick, recent),
                _ => row,
            };
            f.render_widget(Paragraph::new(row), r.footer[i]);
        }
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
        assert!(
            text.contains("· grid") && text.contains("PROGRAMS"),
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
        assert!(text.contains("[?]") && !text.contains("c camera"), "{text}");
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
        let status = text.lines().nth_back(3).unwrap();
        assert!(
            status.starts_with(" camera ·") && status.trim_end().ends_with("[?]"),
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
        assert!(text.contains("[1] netscan 4s"), "{text}");
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
            let last_main = h as usize - 6;
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
                at(h as usize - 5, l),
                Some('┷'),
                "and ends on the footer rule with a light stem: {text}"
            );
            for y in (0..4).chain(5..h as usize - 5) {
                assert_eq!(at(y, l), Some('│'), "row {y}: {text}");
            }
            assert_ne!(
                at(h as usize - 4, l),
                Some('│'),
                "footer spans the width: {text}"
            );
            assert!(
                lines[h as usize - 4].starts_with(" camera · Street"),
                "{text}"
            );
            assert!(
                !lines[h as usize - 4].contains("HERE")
                    && lines[h as usize - 4].trim_end().ends_with("[?]"),
                "{text}"
            );
            assert!(
                lines[h as usize - 1].starts_with(" ABILITIES  [1] netscan ●"),
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
            text.contains(" on foot · Street") && !text.contains("[space]"),
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
        assert!(text.contains("[?]") && text.contains("[F1]Here*"), "{text}");
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
        assert!(text.contains(" ⚡ ") && text.contains("· grid"), "{text}");
        assert!(text.contains("PROGRAMS") && text.contains("[?]"), "{text}");
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
        // The dev tabs sit on the column's last row: 30 - 6.
        let row = 24u16;
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
        let bonus = st.sight_bonus();
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
        // A wait refreshes sight on the new map; in a fight it ends your turn and the
        // ICE plays its turns out, so a stand is a round as a wait is a beat.
        s.update(st, &[key(KeyCode::Char('.'))]);
        settle(st, s);
    }

    /// Lets the ICE play its turns out on the pace until the order is back to you
    /// or the fight ends.
    fn settle(st: &mut AppState, s: &mut MapScreen) {
        use crate::net::PACE_TICKS;
        for _ in 0..80 {
            if crate::net::your_turn(st) {
                return;
            }
            st.tick += PACE_TICKS;
            s.update(st, &[]);
        }
        panic!("the ICE never gave the turn back: {:?}", st.net.engaged);
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
        advance(&mut st, &mut s);
        assert!(logged(&st, "grey ICE hits"), "{:?}", last(&st));
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
            roll(&mut st, &mut s);
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
        assert!(!text.contains("RUN  crack"), "{text}");
        crate::net::start_run(&mut st, ItemId::new("crack"), game::HackTarget::Node(lock)).unwrap();
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.contains("RUN  crack · door lock  ▰▱▱ 1/3"), "{text}");
        // Space is a turn of the run in flight, with no popup in between.
        let cmd = s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(matches!(cmd, ScreenCmd::Stay), "space continues");
        assert_eq!(last(&st), Some("crack 2/3"));
        assert_eq!(st.net.run.as_ref().map(|r| r.done), Some(2));
        // Picking the same program at the same lock again goes on where it stood:
        // the third roll lands it.
        s.update(&mut st, &[key(KeyCode::Char('1')), key(KeyCode::Char(' '))]);
        assert!(logged(&st, "crack goes on · 2/3"));
        assert!(!logged(&st, "run aborted"));
        assert!(logged(&st, "lock cracked"), "{:?}", last(&st));
        assert!(st.net.run.is_none());
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
        assert!(st.net.engaged.is_some(), "seen: the fight is on");
        pass_turn(&mut st, &mut s);
        let after = st.constructs.get(b).unwrap().guards[0].pos;
        assert_eq!(
            after.chebyshev(seen_at),
            (before.chebyshev(seen_at) - 2).max(1),
            "two tiles a turn, stopping beside you"
        );
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.contains("grey ICE · hunting"), "{text}");
        // Into a room: it reaches the last seen tile, loses you, and never enters.
        let room = st.constructs.get(b).unwrap().rooms[0];
        let hide = room_tile(&st, b);
        stand(&mut st, &mut s, Place::Construct(b), hide);
        for _ in 0..14 {
            advance(&mut st, &mut s);
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
        assert!(logged(&st, "white ICE hits · heat 5"), "{:?}", last(&st));
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
        assert!(logged(&st, "grey ICE hits · focus 21"), "{:?}", last(&st));
        assert_eq!(st.character.focus, 21);
        for _ in 0..2 {
            pass_turn(&mut st, &mut s);
        }
        assert_eq!(st.character.focus, 3);
        pass_turn(&mut st, &mut s);
        assert_eq!(st.character.focus, 0);
        assert!(logged(&st, "DUMPED · grey ICE drained you"));
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
        assert!(
            st.net.engaged.is_some(),
            "pointing an attack at a piece is the fight"
        );
        let focus = st.character.focus;
        roll(&mut st, &mut s);
        assert_eq!(last(&st), Some("attack · grey ICE 4/6"));
        pass_turn(&mut st, &mut s);
        assert!(logged(&st, "shield absorbs the hit"));
        assert_eq!(st.character.focus, focus);
        assert_eq!(st.net.shield, 0);
        roll(&mut st, &mut s);
        assert_eq!(last(&st), Some("attack · grey ICE 2/6"));
        pass_turn(&mut st, &mut s);
        assert_eq!(st.character.focus, focus - 9);
        roll(&mut st, &mut s);
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
        advance(&mut st, &mut s);
        assert_eq!(st.net.ice[&b][0].integrity, 6, "whole again");
        // The piece stood again awake, so the beat put the board on turns: stay
        // first. Then the RUN popup offers the deck programs on yourself.
        if crate::net::phase(&st) == Some(crate::net::Phase::Move) {
            s.update(&mut st, &[key(KeyCode::Char(' '))]);
        }
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
        assert!(logged(&st, &format!("black ICE hits · hp {}", hp_max - 5)));
        // Already under a quarter: the next hit dumps, it does not kill.
        st.character.hp = hp_max / 4;
        pass_turn(&mut st, &mut s);
        assert!(
            logged(&st, "DUMPED · the failsafe pulled you out"),
            "{:?}",
            last(&st)
        );
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
            advance(&mut st, &mut s);
        }
        assert_eq!(st.net.mask, 0, "six turns and it is gone");
        assert_eq!(
            st.net.traces[&b].heat,
            heat + 1 + crate::net::MASK_TURNS,
            "one heat a held turn"
        );
        let text = draw_screen(&s, &st, 120, 40);
        assert!(!text.contains("· MASK "), "{text}");
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
        roll(&mut st, &mut s);
        assert_eq!(st.net.ice[&b][0].integrity, 4, "{:?}", last(&st));
        assert!(
            matches!(st.net.ice[&b][0].mode, crate::net::IceMode::Awake { seen, .. } if seen == door_side),
            "a hit piece turns on you: {:?}",
            st.net.ice[&b][0].mode
        );
        // Out of sight: the attack cannot reach and ends.
        st.net.ice.get_mut(&b).unwrap()[0].mode = crate::net::IceMode::Asleep;
        let hide = room_tile(&st, b);
        stand(&mut st, &mut s, Place::Construct(b), hide);
        roll(&mut st, &mut s);
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
        assert!(logged(&st, "black ICE hits · hp 0"), "{:?}", last(&st));
        assert!(st.net.engaged.is_none(), "no fight over a corpse");
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

    fn on_the_grid(st: &mut AppState, s: &mut MapScreen) -> citygen::BuildingId {
        let b = in_construct(st, s);
        let city = st.city.clone().unwrap();
        let gate = crate::constructs::city_gate(&city, b).unwrap();
        st.player.as_mut().unwrap().body = Some((Place::Stratum(StratumKind::Street), gate));
        stand(st, s, Place::Stratum(StratumKind::Net), gate);
        b
    }

    fn lock_run(st: &mut AppState, s: &mut MapScreen) -> (citygen::BuildingId, citygen::NodeId) {
        let b = in_construct(st, s);
        let (lock, lock_pos) = node_of(st, b, NodeKind::DoorLock);
        let here = beside(st, b, lock_pos);
        stand(st, s, Place::Construct(b), here);
        st.net.force_roll = Some(10);
        (b, lock)
    }

    fn learn(st: &mut AppState, talent: &str) {
        st.character.talents.insert(game::TalentId::new(talent), 1);
    }

    #[test]
    fn deep_dive_copies_a_vault_without_heat_on_a_success() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let (vault, vault_pos) = node_of(&st, b, NodeKind::DataVault);
        let here = beside(&st, b, vault_pos);
        stand(&mut st, &mut s, Place::Construct(b), here);
        learn(&mut st, "deep_dive");
        st.net.force_roll = Some(10);
        let heat = st.net.traces.get(&b).map_or(0, |t| t.heat);
        crate::net::start_run(&mut st, ItemId::new("copy"), game::HackTarget::Node(vault)).unwrap();
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(last(&st), Some("copy 1/4"));
        assert_eq!(
            st.net.traces.get(&b).map_or(0, |t| t.heat),
            heat,
            "a quiet success"
        );
        // A slip is still loud: copy's noise 3 plus 2.
        st.net.force_roll = Some(1);
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert!(
            last(&st).unwrap().starts_with("copy slips"),
            "{:?}",
            last(&st)
        );
        assert_eq!(st.net.traces[&b].heat, heat + 5);
        // Without the flag a success costs copy's noise.
        st.character.talents.clear();
        st.net.force_roll = Some(10);
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(st.net.traces[&b].heat, heat + 5 + 3);
    }

    #[test]
    fn null_route_turns_the_first_dump_away_once() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let (b, lock) = lock_run(&mut st, &mut s);
        learn(&mut st, "null_route");
        let now = st.net.now(&st);
        crate::net::start_run(&mut st, ItemId::new("crack"), game::HackTarget::Node(lock)).unwrap();
        st.net.trace(b).heat = 98;
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert!(st.player.as_ref().unwrap().jacked_in(), "not dumped");
        assert_eq!(st.net.traces[&b].heat, game::ALARM_HEAT);
        assert!(st.net.null_routed.contains(&b));
        assert!(st.net.run.is_some(), "the run goes on");
        let routed: Vec<String> = st
            .log
            .iter()
            .rev()
            .take(6)
            .map(|e| e.text.clone())
            .collect();
        assert!(
            routed
                .iter()
                .any(|l| l == "null route · the trace loses you"),
            "{routed:?}"
        );
        assert!(st.net.alert(b, now), "the alarm had tripped on the way up");
        // The second time is real.
        st.net.trace(b).heat = 98;
        roll(&mut st, &mut s);
        assert!(!st.player.as_ref().unwrap().jacked_in(), "dumped");
    }

    #[test]
    fn null_route_on_the_turn_that_also_crosses_the_alarm_line() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let (b, lock) = lock_run(&mut st, &mut s);
        learn(&mut st, "null_route");
        // A slip is crack's noise 4 plus 2: from 94 that is the dump line, and the
        // alarm line was never crossed before.
        st.net.force_roll = Some(1);
        crate::net::start_run(&mut st, ItemId::new("crack"), game::HackTarget::Node(lock)).unwrap();
        st.net.trace(b).heat = 94;
        st.net.trace(b).alert_until = None;
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        let now = st.net.now(&st);
        assert!(st.player.as_ref().unwrap().jacked_in(), "not dumped");
        assert_eq!(st.net.traces[&b].heat, game::ALARM_HEAT);
        assert!(st.net.alert(b, now), "the alarm trips");
        let recent: Vec<String> = st
            .log
            .iter()
            .rev()
            .take(4)
            .map(|e| e.text.clone())
            .collect();
        assert_eq!(
            recent
                .iter()
                .filter(|l| l.as_str() == "null route · the trace loses you")
                .count(),
            1,
            "{recent:?}"
        );
        assert!(recent.iter().any(|l| l.starts_with("ALARM")), "{recent:?}");
    }

    #[test]
    fn a_white_hit_and_a_held_mask_route_away_too() {
        // A white hit at 96 (+5) routes; a held mask's heat at 99 (+1) routes.
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct_at(&mut st, &mut s, 170);
        assert_eq!(
            st.constructs.get(b).unwrap().guards[0].grade,
            IceGrade::White
        );
        learn(&mut st, "null_route");
        let post = st.constructs.get(b).unwrap().guards[0].post;
        let k = st.constructs.get(b).unwrap().clone();
        st.net.ice_of(&k);
        let beside_post = Pos::new(post.x + 1, post.y);
        st.net.trace(b).heat = 96;
        st.net.wake_all(b, beside_post);
        stand(&mut st, &mut s, Place::Construct(b), beside_post);
        assert!(st.player.as_ref().unwrap().jacked_in(), "{:?}", last(&st));
        assert_eq!(st.net.traces[&b].heat, game::ALARM_HEAT);
        // Fresh construct state for the mask: a second building would do, but the
        // route is per construct, so reset it.
        st.net.null_routed.clear();
        st.net.mask = 2;
        st.net.trace(b).heat = 99;
        let far = room_tile(&st, b);
        st.player.as_mut().unwrap().pos = far;
        // The mask wears at the start of your next turn: the round routes it away.
        advance(&mut st, &mut s);
        assert!(st.player.as_ref().unwrap().jacked_in(), "{:?}", last(&st));
        assert_eq!(st.net.traces[&b].heat, game::ALARM_HEAT);
    }

    #[test]
    fn the_deep_end_halves_black_hits_and_moves_the_failsafe_line() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct_at(&mut st, &mut s, 255);
        learn(&mut st, "deep_end");
        let vault_guard = st.constructs.get(b).unwrap().guards[1].clone();
        assert_eq!(vault_guard.grade, IceGrade::Black);
        let beside_vault = Pos::new(vault_guard.post.x - 1, vault_guard.post.y);
        let hp_max = st.character.hp_max(&st.content);
        stand(&mut st, &mut s, Place::Construct(b), beside_vault);
        // 6 + (255 − 230) / 5 = 11, less the deck's rating 1, halved: 5.
        assert_eq!(st.character.hp, hp_max - 5, "{:?}", last(&st));
        // Two hits a turn, ten in all: from a quarter plus five you land over the
        // eighth, no failsafe yet.
        st.character.hp = hp_max / 4 + 5;
        pass_turn(&mut st, &mut s);
        assert!(st.player.as_ref().unwrap().jacked_in(), "{:?}", last(&st));
        assert_eq!(st.character.hp, hp_max / 4 - 5);
        // At the eighth it pulls, and cools ten minutes.
        st.character.hp = hp_max / 8 + 5;
        pass_turn(&mut st, &mut s);
        assert!(
            logged(&st, "DUMPED · the failsafe pulled you out"),
            "{:?}",
            last(&st)
        );
        let now = st.net.now(&st);
        assert_eq!(st.net.cooling(now), Some(crate::net::DEEP_END_COOLING));
        assert_eq!(crate::net::DEEP_END_COOLING, 10);
    }

    #[test]
    fn the_deep_end_with_the_failsafe_off_still_flatlines_at_zero() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct_at(&mut st, &mut s, 255);
        learn(&mut st, "deep_end");
        st.net.failsafe = false;
        let vault_guard = st.constructs.get(b).unwrap().guards[1].clone();
        let beside_vault = Pos::new(vault_guard.post.x - 1, vault_guard.post.y);
        st.character.hp = 3;
        stand(&mut st, &mut s, Place::Construct(b), beside_vault);
        assert_eq!(st.character.hp, 0);
        assert!(st.net.death.is_some());
    }

    fn fire(st: &mut AppState, id: &str) -> Result<String, String> {
        crate::actions::perform(st, game::Action::Fire(game::AbilityId::new(id)))
    }

    #[test]
    fn ping_inside_a_construct_lights_every_node_and_warms_the_trace() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        learn(&mut st, "handshake");
        let nodes: Vec<Pos> = st
            .constructs
            .get(b)
            .unwrap()
            .nodes
            .iter()
            .map(|(_, p)| *p)
            .collect();
        let heat = st.net.traces.get(&b).map_or(0, |t| t.heat);
        let msg = fire(&mut st, "ping").unwrap();
        assert_eq!(msg, format!("Ping: {} nodes answer", nodes.len()));
        assert_eq!(st.net.traces[&b].heat, heat + crate::net::PING_NOISE);
        let glow = st
            .fx
            .iter()
            .find(|f| matches!(f.fx, game::Fx::Glow { .. }))
            .expect("the ping's glow");
        let mut lit = glow.targets.clone();
        lit.sort_by_key(|p| (p.y, p.x));
        let mut expect = nodes;
        expect.sort_by_key(|p| (p.y, p.x));
        assert_eq!(lit, expect);
        // On the street the ping is the old ping: no heat, no node targets.
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        learn(&mut st, "handshake");
        assert_eq!(fire(&mut st, "ping").unwrap(), "Ping: pulse sent");
        assert!(st.net.traces.is_empty());
    }

    #[test]
    fn arc_trace_inside_a_construct_stuns_the_ice_in_range() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let k = st.constructs.get(b).unwrap().clone();
        assert_eq!(k.guards.len(), 2);
        st.net.ice_of(&k);
        let post = k.guards[0].post;
        let beside_post = Pos::new(post.x + 1, post.y);
        st.net.wake_all(b, beside_post);
        // Stand beside the awake gate guard without a turn passing.
        {
            let p = st.player.as_mut().unwrap();
            p.place = Place::Construct(b);
            p.pos = beside_post;
        }
        s.place = Place::Construct(b);
        let heat = st.net.traces.get(&b).map_or(0, |t| t.heat);
        let msg = fire(&mut st, "arc_trace").unwrap();
        assert_eq!(msg, "Arc Trace: 2 pieces stunned");
        assert_eq!(st.net.traces[&b].heat, heat + crate::net::TRACE_NOISE);
        assert_eq!(st.net.ice[&b][0].stunned, crate::net::STUN_TURNS);
        assert_eq!(st.net.ice[&b][1].stunned, crate::net::STUN_TURNS);
        let bolt = st
            .fx
            .iter()
            .find(|f| matches!(f.fx, game::Fx::Bolt { .. }))
            .unwrap();
        assert_eq!(bolt.targets.len(), 2);
        let lines = crate::net::ice_lines(&st, b, |_| true);
        assert!(lines.iter().any(|l| l.contains("· stunned 2")), "{lines:?}");
        // Two turns beside it and it does nothing; the third it hits.
        let hp = st.character.hp;
        let focus = st.character.focus;
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(st.net.ice[&b][0].stunned, 1);
        assert!(
            last(&st).is_none_or(|l| !l.contains("ICE hits")),
            "{:?}",
            last(&st)
        );
        // The alarm wakes a stunned piece, and it still waits out the stun.
        crate::net::raise_alarm(&mut st, b);
        advance(&mut st, &mut s);
        assert_eq!(st.net.ice[&b][0].stunned, 0);
        assert!(!logged(&st, "ICE hits"), "{:?}", last(&st));
        assert_eq!((st.character.hp, st.character.focus), (hp, focus));
        advance(&mut st, &mut s);
        assert!(logged(&st, "grey ICE hits"), "{:?}", last(&st));
    }

    #[test]
    fn arc_trace_on_the_grid_is_the_old_arc_trace() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let city = st.city.clone().unwrap();
        let gate = crate::constructs::city_gate(&city, b).unwrap();
        st.player.as_mut().unwrap().body = Some((Place::Stratum(StratumKind::Street), gate));
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), gate);
        let heat = st.net.traces.get(&b).map_or(0, |t| t.heat);
        assert_eq!(fire(&mut st, "arc_trace").unwrap(), "Arc Trace: pulse sent");
        assert_eq!(st.net.traces.get(&b).map_or(0, |t| t.heat), heat);
        assert!(
            st.net
                .ice
                .get(&b)
                .is_none_or(|v| v.iter().all(|i| i.stunned == 0))
        );
    }

    #[test]
    fn signal_ghost_feels_awake_ice_through_walls() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let k = st.constructs.get(b).unwrap().clone();
        st.net.ice_of(&k);
        let hide = room_tile(&st, b);
        stand(&mut st, &mut s, Place::Construct(b), hide);
        assert!(
            crate::net::ice_lines(&st, b, |_| false).is_empty(),
            "asleep and unseen: nothing"
        );
        assert!(crate::net::ghost_pieces(&st, b).is_empty());
        st.net.wake_all(b, hide);
        let lines = crate::net::ice_lines(&st, b, |_| false);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines.iter().all(|l| l.contains("hunting")));
        let felt = crate::net::ghost_pieces(&st, b);
        assert_eq!(felt.len(), 2);
        // Without the ability, nothing through walls.
        st.character =
            game::Character::from_template(&st.content, &game::TemplateId::new("kessler")).unwrap();
        assert!(
            !st.character
                .has_ability(&st.content, &game::AbilityId::new("signal_ghost"))
        );
        assert!(crate::net::ice_lines(&st, b, |_| false).is_empty());
        assert!(crate::net::ghost_pieces(&st, b).is_empty());
    }

    #[test]
    fn low_light_sees_two_more_tiles_between_eighteen_and_six() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        assert!(
            st.character
                .has_ability(&st.content, &game::AbilityId::new("lowlight")),
            "the optics"
        );
        let base = st.character.sight_bonus(&st.content);
        // The clock starts at 22:00 and runs ninety ticks a minute.
        let at =
            |h: u64, m: u64| ((h * 60 + m + 24 * 60 - 22 * 60) % (24 * 60)) * sim::TICKS_PER_MINUTE;
        for (tick, bonus, when) in [
            (at(22, 0), base + 2, "22:00"),
            (at(5, 59), base + 2, "05:59"),
            (at(6, 0), base, "06:00"),
            (at(12, 0), base, "12:00"),
            (at(18, 0), base + 2, "18:00"),
            (at(17, 59), base, "17:59"),
        ] {
            st.tick = tick;
            assert_eq!(
                st.clock().0,
                (tick / sim::TICKS_PER_MINUTE / 60 + 22) as u8 % 24,
                "{when}"
            );
            assert_eq!(st.sight_bonus(), bonus, "{when}");
        }
        // Without the optics there is no low-light and no bonus at all.
        let eyes = st
            .character
            .equipment
            .iter()
            .find(|(_, id)| id.0 == "nightglass_optics")
            .map(|(slot, _)| *slot)
            .unwrap();
        st.character.equipment.remove(&eyes);
        st.tick = at(22, 0);
        assert_eq!(st.sight_bonus(), 0);
    }

    #[test]
    fn the_slots_row_reads_programs_while_jacked_in() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.contains(" ABILITIES  [1] netscan"), "{text}");
        lock_run(&mut st, &mut s);
        let text = draw_screen(&s, &st, 120, 40);
        assert!(
            text.contains(" PROGRAMS   [1] CRACK ●    [2] COPY ●    [3] ──"),
            "{text}"
        );
        assert!(!text.contains("ABILITIES"), "{text}");
    }

    #[test]
    fn a_number_key_fires_the_program_at_the_one_target_in_reach() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        lock_run(&mut st, &mut s);
        // Beside the lock and nothing else: the lock is picked, no popup; space runs
        // it, armed and rolled once.
        let cmd = s.update(&mut st, &[key(KeyCode::Char('1'))]);
        assert!(matches!(cmd, ScreenCmd::Stay), "no popup");
        assert_eq!(s.targeting.as_ref().map(|t| t.candidates.len()), Some(1));
        assert!(st.net.run.is_none());
        let cmd = s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(matches!(cmd, ScreenCmd::Stay), "space runs it, no popup");
        assert_eq!(last(&st), Some("crack 1/3"));
        assert!(st.net.run.is_some());
        // Space again is another turn of it.
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(last(&st), Some("crack 2/3"));
        // Copy does nothing to a lock: the strip says so and nothing changes.
        s.update(&mut st, &[key(KeyCode::Char('2'))]);
        assert_eq!(last(&st), Some("copy · nothing to run it against"));
        assert!(last_is_warning(&st));
        assert_eq!(
            st.net.run.as_ref().map(|r| r.done),
            Some(2),
            "the crack runs on"
        );
        s.update(&mut st, &[key(KeyCode::Char('3'))]);
        assert_eq!(last(&st), Some("nothing in slot 3"));
    }

    #[test]
    fn a_number_key_with_several_targets_opens_the_short_list() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        st.net.force_roll = Some(10);
        // Beside the lock, with the gate guard moved to the other side of it: two
        // things crack applies to.
        let (_, lock_pos) = node_of(&st, b, NodeKind::DoorLock);
        let here = beside(&st, b, lock_pos);
        stand(&mut st, &mut s, Place::Construct(b), here);
        let other = {
            let k = st.constructs.get(b).unwrap();
            here.neighbors8()
                .find(|p| {
                    *p != lock_pos
                        && k.tiles.get(*p).is_some_and(|t| t.is_walkable())
                        && k.node_at(*p).is_none()
                })
                .expect("a free tile beside the player")
        };
        st.constructs.get_mut(b).unwrap().move_guard(0, other);
        s.update(&mut st, &[key(KeyCode::Char('1'))]);
        let t = s.targeting.as_ref().expect("two targets: the pick is up");
        assert_eq!(t.candidates.len(), 2, "{:?}", t.candidates);
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.contains("CRACK › door lock"), "{text}");
        s.update(&mut st, &[key(KeyCode::Tab)]);
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.contains("CRACK › ICE · grey"), "{text}");
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(st.net.run.is_some());
        assert!(
            matches!(
                st.net.run.as_ref().unwrap().target,
                game::HackTarget::Guard { .. }
            ),
            "the picked piece"
        );
    }

    #[test]
    fn number_keys_on_the_grid_and_without_a_deck_say_so() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let city = st.city.as_ref().unwrap().clone();
        let gate = crate::constructs::city_gate(&city, b).unwrap();
        st.player.as_mut().unwrap().body = Some((Place::Stratum(StratumKind::Street), gate));
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), gate);
        let cmd = s.update(&mut st, &[key(KeyCode::Char('1'))]);
        assert!(matches!(cmd, ScreenCmd::Stay));
        assert_eq!(last(&st), Some("programs run inside a construct"));
        assert!(st.net.run.is_none());
        // Back inside without a deck.
        let entry = st.constructs.get(b).unwrap().entry;
        stand(&mut st, &mut s, Place::Construct(b), entry);
        let deck_slot = st
            .character
            .equipment
            .iter()
            .find(|(_, id)| {
                st.content
                    .item(id)
                    .is_some_and(|i| i.kind == game::ItemKind::Deck)
            })
            .map(|(slot, _)| *slot)
            .expect("a deck is jacked");
        st.character.equipment.remove(&deck_slot);
        let cmd = s.update(&mut st, &[key(KeyCode::Char('1'))]);
        assert!(matches!(cmd, ScreenCmd::Stay));
        assert_eq!(last(&st), Some("no deck jacked"));
    }

    #[test]
    fn the_strip_reads_the_run_the_heat_and_the_hunt() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let (b, _) = lock_run(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('1')), key(KeyCode::Char(' '))]);
        assert_eq!(last(&st), Some("crack 1/3"));
        let text = draw_screen(&s, &st, 180, 40);
        assert!(text.contains("TRACE ▱▱▱▱▱▱▱▱▱▱ 4"), "{text}");
        assert!(text.contains("RUN  crack · door lock  ▰▱▱ 1/3"), "{text}");
        // Every piece asleep: no hunt in the strip.
        for i in st.net.ice.get_mut(&b).unwrap() {
            i.mode = crate::net::IceMode::Asleep;
        }
        let text = draw_screen(&s, &st, 180, 40);
        assert!(!text.contains("HUNTED"), "{text}");
        let pos = player(&st).1;
        st.net.wake_all(b, pos);
        let text = draw_screen(&s, &st, 180, 40);
        assert!(text.contains("HUNTED"), "{text}");
        // No run: the Net verbs.
        st.net.run = None;
        let text = draw_screen(&s, &st, 180, 40);
        assert!(!text.contains("RUN  crack"), "{text}");
    }

    #[test]
    fn the_location_on_the_net_is_the_district() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let city = st.city.as_ref().unwrap().clone();
        let gate = crate::constructs::city_gate(&city, b).unwrap();
        let district = city.district(city.building(b).district).name.clone();
        st.player.as_mut().unwrap().body = Some((Place::Stratum(StratumKind::Street), gate));
        stand(&mut st, &mut s, Place::Stratum(StratumKind::Net), gate);
        assert_eq!(
            hud::location(&city, &st.interiors, Place::Stratum(StratumKind::Net), gate),
            format!("{district} · Net")
        );
    }

    #[test]
    fn a_hotkey_whose_only_row_is_disabled_says_why() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let (server, server_pos) = node_of(&st, b, NodeKind::Server);
        let here = beside(&st, b, server_pos);
        stand(&mut st, &mut s, Place::Construct(b), here);
        let now = st.net.now(&st);
        st.net.land(
            b,
            game::HackTarget::Node(server),
            game::Target::Node(NodeKind::Server),
            game::Op::Copy,
            now,
        );
        let cmd = s.update(&mut st, &[key(KeyCode::Char('2'))]);
        assert!(
            matches!(cmd, ScreenCmd::Stay),
            "no popup for one disabled row"
        );
        assert_eq!(last(&st), Some("copy · copied already"));
        assert!(last_is_warning(&st));
        assert!(st.net.run.is_none());
    }

    #[test]
    fn the_layout_gives_the_footer_four_rows() {
        let r = layout(Rect::new(0, 0, 100, 30));
        assert_eq!(r.bottom_rule.y, 25);
        assert_eq!(
            [r.footer[0].y, r.footer[1].y, r.footer[2].y, r.footer[3].y],
            [26, 27, 28, 29]
        );
        assert_eq!(r.map.height, 20);
        assert_eq!(r.map.y, 5);
    }

    #[test]
    fn the_screen_draws_the_footer_without_hints() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        let text = draw_screen(&s, &st, 120, 40);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[36].starts_with(" on foot ·"), "{:?}", lines[36]);
        assert!(lines[36].trim_end().ends_with("[?]"), "{:?}", lines[36]);
        assert_eq!(lines[37].trim(), "");
        assert_eq!(lines[38].trim(), "");
        assert!(
            lines[39].starts_with(" ABILITIES  [1] netscan ●"),
            "{:?}",
            lines[39]
        );
        assert!(
            !text.contains("[space]") && !text.contains("STATUS:") && !text.contains("HERE:"),
            "{text}"
        );
        // Jacked in with a run: the run row and the programs row, same four rows.
        let (_, _) = lock_run(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('1')), key(KeyCode::Char(' '))]);
        assert!(st.net.run.is_some());
        let text = draw_screen(&s, &st, 120, 40);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[36].starts_with(" ⚡ "), "{:?}", lines[36]);
        assert!(lines[36].contains("TRACE ▱▱▱▱▱▱▱▱▱▱ 4"), "{:?}", lines[36]);
        assert_eq!(lines[37].trim(), "");
        assert!(
            lines[38].starts_with(" RUN  crack · door lock"),
            "{:?}",
            lines[38]
        );
        assert!(
            lines[39].starts_with(" PROGRAMS   [1] CRACK ●    [2] COPY ●"),
            "{:?}",
            lines[39]
        );
        // The console takes the last row.
        st.set_dev(true);
        s.update(&mut st, &[key(KeyCode::Char(':'))]);
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.lines().nth(39).unwrap().contains(":"), "{text}");
        assert!(
            text.lines().nth(38).unwrap().starts_with(" RUN  crack"),
            "{text}"
        );
    }

    #[test]
    fn help_lists_the_keys_the_footer_dropped() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        walking(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('?'))]);
        let text = draw_screen(&s, &st, 140, 45);
        for k in ["space", "enter", "tab", "v ", "m ", ". "] {
            assert!(text.contains(k), "{k:?} missing: {text}");
        }
    }

    #[test]
    fn a_calm_construct_rolls_the_run_on_the_beat_once_per_frame() {
        use crate::net::BEAT_TICKS;
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        lock_run(&mut st, &mut s);
        // The pick runs it once at once; the beat rolls it from there.
        s.update(&mut st, &[key(KeyCode::Char('1')), key(KeyCode::Char(' '))]);
        assert_eq!(last(&st), Some("crack 1/3"));
        s.update(&mut st, &[]);
        assert_eq!(last(&st), Some("crack 1/3"), "no beat yet");
        st.tick += BEAT_TICKS;
        s.update(&mut st, &[]);
        assert_eq!(last(&st), Some("crack 2/3"), "the beat rolls once");
        // A long gap (a menu was up) is still one beat, never a burst.
        st.net.force_roll = Some(1);
        st.tick += BEAT_TICKS * 3;
        s.update(&mut st, &[]);
        assert!(
            last(&st).unwrap().starts_with("crack slips"),
            "{:?}",
            last(&st)
        );
        assert_eq!(
            st.log
                .iter()
                .filter(|e| e.text.starts_with("crack slips"))
                .count(),
            1,
            "one roll for three beats' worth of ticks"
        );
        assert_eq!(st.net.run.as_ref().map(|r| r.done), Some(2));
    }

    #[test]
    fn asleep_ice_patrols_on_the_beat_and_not_when_you_move() {
        use crate::net::BEAT_TICKS;
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let hide = room_tile(&st, b);
        stand(&mut st, &mut s, Place::Construct(b), hide);
        let k = st.constructs.get(b).unwrap().clone();
        st.net.ice_of(&k);
        for i in st.net.ice.get_mut(&b).unwrap() {
            i.mode = crate::net::IceMode::Asleep;
        }
        let before = st.constructs.get(b).unwrap().guards[0].pos;
        // A step inside the room gives the ICE no turn.
        let step = hide
            .neighbors4()
            .find(|p| k.tiles.get(*p).is_some_and(|t| t.is_walkable()) && k.node_at(*p).is_none())
            .expect("room to step in");
        s.update(&mut st, &[key(dir_between(hide, step))]);
        assert_eq!(player(&st).1, step);
        assert_eq!(
            st.constructs.get(b).unwrap().guards[0].pos,
            before,
            "a move is not a turn"
        );
        // The beat moves it along its beat.
        st.tick += BEAT_TICKS;
        s.update(&mut st, &[]);
        assert_ne!(
            st.constructs.get(b).unwrap().guards[0].pos,
            before,
            "a beat is a turn"
        );
    }

    #[test]
    fn nothing_beats_on_the_grid_and_the_wait_key_hurries_the_beat() {
        use crate::net::BEAT_TICKS;
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = on_the_grid(&mut st, &mut s);
        let k = st.constructs.get(b).unwrap().clone();
        st.net.ice_of(&k);
        let before = st.constructs.get(b).unwrap().guards[0].pos;
        let beat = st.net.beat;
        st.tick += BEAT_TICKS * 2;
        s.update(&mut st, &[]);
        assert_eq!(st.constructs.get(b).unwrap().guards[0].pos, before);
        assert_eq!(st.net.beat, beat, "no beat outside a construct");
        // Inside, the wait key is a beat now.
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        lock_run(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('1')), key(KeyCode::Char(' '))]);
        assert_eq!(last(&st), Some("crack 1/3"));
        assert_eq!(st.net.beat, st.clock_tick(), "the beat phase restarts");
    }

    #[test]
    fn the_beat_bar_fills_toward_the_next_roll() {
        use crate::net::{BEAT_TICKS, beat_bar};
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        lock_run(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(beat_bar(&st, 8), "░░░░░░░░");
        st.tick += BEAT_TICKS / 2;
        assert_eq!(beat_bar(&st, 8), "▓▓▓▓░░░░");
        s.update(&mut st, &[key(KeyCode::Char('1')), key(KeyCode::Char(' '))]);
        let text = draw_screen(&s, &st, 140, 40);
        assert!(
            text.contains("RUN  crack · door lock  ▰▱▱ 1/3 · 40%   ▓▓▓▓░░░░  next roll"),
            "{text}"
        );
    }

    fn logged(st: &AppState, needle: &str) -> bool {
        st.log.iter().any(|e| e.text.contains(needle))
    }

    /// One roll of the run in flight: a hurried beat in calm, an action point in a
    /// fight (the turn passes first when none is left).
    fn roll(st: &mut AppState, s: &mut MapScreen) {
        if st.net.engaged.is_none() {
            s.update(st, &[key(KeyCode::Char('.'))]);
            return;
        }
        if st.net.engaged.as_ref().is_some_and(|e| e.ap == 0) || !crate::net::your_turn(st) {
            pass_turn(st, s);
        }
        if st.net.engaged.is_some() {
            // The move half first: a space stays, the next rolls.
            if crate::net::phase(st) == Some(crate::net::Phase::Move) {
                s.update(st, &[key(KeyCode::Char(' '))]);
            }
            s.update(st, &[key(KeyCode::Char(' '))]);
        } else {
            s.update(st, &[key(KeyCode::Char('.'))]);
        }
    }

    /// Time passes: a beat in calm, a whole round in a fight.
    fn advance(st: &mut AppState, s: &mut MapScreen) {
        if st.net.engaged.is_some() {
            pass_turn(st, s);
        } else {
            s.update(st, &[key(KeyCode::Char('.'))]);
        }
    }

    /// Ends your turn and lets the ICE play its turns out on the pace until the
    /// order comes back to you or the fight ends.
    fn pass_turn(st: &mut AppState, s: &mut MapScreen) {
        s.update(st, &[key(KeyCode::Char('.'))]);
        settle(st, s);
    }

    fn engaged_at_the_gate(st: &mut AppState, s: &mut MapScreen) -> citygen::BuildingId {
        let b = in_construct(st, s);
        let entry = st.constructs.get(b).unwrap().entry;
        // On the spine in the gate guard's sight: the first beat wakes it.
        stand(st, s, Place::Construct(b), entry);
        assert!(st.net.engaged.is_some(), "{:?}", last(st));
        b
    }

    #[test]
    fn a_wake_puts_the_construct_on_turns_with_an_order() {
        use crate::net::{Actor, BASE_AP, BASE_MOVE};
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = engaged_at_the_gate(&mut st, &mut s);
        let e = st.net.engaged.clone().unwrap();
        assert_eq!(e.building, b);
        assert_eq!(e.round, 1);
        assert_eq!(e.order.len(), 3, "{:?}", e.order);
        assert_eq!(e.order[0], Actor::You, "ties go to you: {:?}", e.order);
        assert_eq!(&e.order[1..], &[Actor::Ice(0), Actor::Ice(1)]);
        assert_eq!((e.moves, e.ap), (BASE_MOVE, BASE_AP));
        assert!(!e.manual);
        let text = draw_screen(&s, &st, 140, 40);
        assert!(text.contains("TURN 1  YOU › grey ICE › grey ICE"), "{text}");
        assert!(text.contains("MOVE ▰▰▰▰ 4/4    AP ●● 2/2"), "{text}");
        assert!(logged(&st, "ENGAGED · the ICE is on you"));
    }

    #[test]
    fn initiative_puts_black_ice_before_grey_and_you_before_a_tie() {
        use crate::net::{Actor, initiative};
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct_at(&mut st, &mut s, 255);
        // The deck adds one Net: at a base of 0 you score 1, under grey's 2 and black's 3.
        st.character.stats.net = 0;
        let order = initiative(&st, b);
        assert_eq!(
            order,
            vec![Actor::Ice(1), Actor::Ice(0), Actor::You],
            "black 3, grey 2, you 1"
        );
        // At 3 you tie black and the tie is yours.
        st.character.stats.net = 2;
        assert_eq!(
            initiative(&st, b),
            vec![Actor::You, Actor::Ice(1), Actor::Ice(0)]
        );
    }

    #[test]
    fn your_keys_spend_the_pools_and_the_dot_ends_your_turn() {
        use crate::net::{BASE_AP, BASE_MOVE};
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let (b, _) = lock_run(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        assert!(st.net.engaged.as_ref().is_some_and(|e| e.manual));
        // Steps inside the room spend moves: the arrows aim, enter walks; the fifth
        // is refused.
        let here = player(&st).1;
        let k = st.constructs.get(b).unwrap().clone();
        let there = here
            .neighbors4()
            .find(|p| k.tiles.get(*p).is_some_and(|t| t.is_walkable()) && k.node_at(*p).is_none())
            .expect("room to step in");
        let (go, back) = (dir_between(here, there), dir_between(there, here));
        for (n, d) in [go, back, go, back].into_iter().enumerate() {
            s.update(&mut st, &[key(d), key(KeyCode::Enter)]);
            walk_tick(&mut st, &mut s);
            assert_eq!(
                st.net.engaged.as_ref().unwrap().moves,
                BASE_MOVE - 1 - n as u8
            );
        }
        s.update(&mut st, &[key(go)]);
        assert_eq!(last(&st), Some("moves are done"));
        assert_eq!(player(&st).1, here, "refused: not moved");
        // A program starts for free and rolls per action point.
        s.update(&mut st, &[key(KeyCode::Char('1'))]);
        assert!(
            s.targeting.is_some(),
            "the lock is picked, nothing spent yet"
        );
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, BASE_AP);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(last(&st), Some("crack 1/3"));
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, BASE_AP - 1);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(last(&st), Some("crack 2/3"));
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(last(&st), Some("no AP left"));
        assert_eq!(st.net.run.as_ref().map(|r| r.done), Some(2));
        // Ending the turn brings a fresh turn back, nothing awake, still engaged by hand.
        pass_turn(&mut st, &mut s);
        let e = st.net.engaged.as_ref().unwrap();
        assert_eq!((e.moves, e.ap, e.round), (BASE_MOVE, BASE_AP, 2));
    }

    #[test]
    fn an_ice_turn_steps_on_the_pace_then_hits_per_action_point() {
        use crate::net::{Actor, PACE_TICKS};
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        // At the gate, two tiles from the guard's post, with no beat yet: the alarm
        // wakes it and the fight starts with it still at its post.
        let me = st.constructs.get(b).unwrap().entry;
        {
            let p = st.player.as_mut().unwrap();
            p.place = Place::Construct(b);
            p.pos = me;
        }
        s.place = Place::Construct(b);
        crate::net::raise_alarm(&mut st, b);
        assert!(st.net.engaged.is_some());
        assert!(crate::net::your_turn(&st));
        // Your turn: do nothing, end it. The gate guard (grey: 2 moves, 1 AP) comes.
        let start = st.constructs.get(b).unwrap().guards[0].pos;
        let d0 = start.chebyshev(me);
        assert_eq!(d0, 2, "the guard starts {d0} tiles off");
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert_eq!(st.net.engaged.as_ref().unwrap().actor(), Actor::Ice(0));
        s.update(&mut st, &[]);
        assert_eq!(
            st.constructs.get(b).unwrap().guards[0].pos,
            start,
            "nothing before the pace"
        );
        st.tick += PACE_TICKS;
        s.update(&mut st, &[]);
        let after_one = st.constructs.get(b).unwrap().guards[0].pos;
        assert_eq!(after_one.chebyshev(me), d0 - 1, "one tile a pace");
        let text = draw_screen(&s, &st, 140, 40);
        assert!(
            text.contains("grey ICE moves 1") || text.contains("grey ICE hits"),
            "{text}"
        );
        assert!(
            text.contains("[1] CRACK ○"),
            "hollow keys on its turn: {text}"
        );
        s.update(&mut st, &[key(KeyCode::Char('h'))]);
        assert_eq!(last(&st), Some("not your turn"));
        let focus = st.character.focus;
        for _ in 0..3 {
            st.tick += PACE_TICKS;
            s.update(&mut st, &[]);
        }
        if d0 <= 3 {
            assert_eq!(
                st.character.focus,
                focus - 9,
                "beside you at the end of its moves: one hit"
            );
        }
        // The second grey's turn and then back to you.
        for _ in 0..8 {
            if crate::net::your_turn(&st) {
                break;
            }
            st.tick += PACE_TICKS;
            s.update(&mut st, &[]);
        }
        assert!(crate::net::your_turn(&st));
        assert_eq!(st.net.engaged.as_ref().unwrap().round, 2);
    }

    #[test]
    fn black_ice_hits_twice_a_turn() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct_at(&mut st, &mut s, 255);
        let hp_max = st.character.hp_max(&st.content);
        let vault_guard = st.constructs.get(b).unwrap().guards[1].clone();
        let beside_vault = Pos::new(vault_guard.post.x - 1, vault_guard.post.y);
        // The beat that lands you there is one hit; the engagement follows.
        stand(&mut st, &mut s, Place::Construct(b), beside_vault);
        assert!(st.net.engaged.is_some());
        let hp = st.character.hp;
        assert!(hp < hp_max);
        pass_turn(&mut st, &mut s);
        // Black (2 AP) hit twice, the other grey once if it reached you; the failsafe
        // did not fire above a quarter.
        assert!(hp - st.character.hp >= 20, "{} -> {}", hp, st.character.hp);
        assert!(st.player.as_ref().unwrap().jacked_in());
    }

    #[test]
    fn a_stunned_piece_passes_its_turn_and_a_calm_round_ends_the_fight() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = engaged_at_the_gate(&mut st, &mut s);
        let focus = st.character.focus;
        assert_eq!(
            fire(&mut st, "arc_trace").unwrap_err(),
            "move first · space to stay"
        );
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(
            fire(&mut st, "arc_trace").unwrap(),
            "Arc Trace: 2 pieces stunned"
        );
        assert_eq!(
            st.net.engaged.as_ref().unwrap().ap,
            crate::net::BASE_AP - 1,
            "a pulse is an action"
        );
        let before: Vec<Pos> = st
            .constructs
            .get(b)
            .unwrap()
            .guards
            .iter()
            .map(|g| g.pos)
            .collect();
        pass_turn(&mut st, &mut s);
        let after: Vec<Pos> = st
            .constructs
            .get(b)
            .unwrap()
            .guards
            .iter()
            .map(|g| g.pos)
            .collect();
        assert_eq!(before, after, "stunned pieces neither move nor hit");
        assert_eq!(st.character.focus, focus);
        assert_eq!(st.net.ice[&b][0].stunned, 1);
        // Hidden in a room with everything asleep, a round passes: back to the beat.
        let hide = room_tile(&st, b);
        st.player.as_mut().unwrap().pos = hide;
        for i in st.net.ice.get_mut(&b).unwrap() {
            i.mode = crate::net::IceMode::Asleep;
            i.stunned = 0;
        }
        pass_turn(&mut st, &mut s);
        assert!(st.net.engaged.is_none(), "calm again");
        assert_eq!(last(&st), Some("calm"));
    }

    #[test]
    fn e_switches_by_hand_and_is_refused_while_hunted() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let hide = room_tile(&st, b);
        stand(&mut st, &mut s, Place::Construct(b), hide);
        assert!(st.net.engaged.is_none());
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        assert!(st.net.engaged.as_ref().is_some_and(|e| e.manual));
        pass_turn(&mut st, &mut s);
        assert!(st.net.engaged.is_some(), "by hand it stays on");
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        assert!(st.net.engaged.is_none());
        // Hunted: no way back to calm by hand.
        st.net.wake_all(b, hide);
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        assert!(st.net.engaged.is_some());
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        assert_eq!(last(&st), Some("still hunted"));
        assert!(st.net.engaged.is_some());
        // During a piece's turn the key is refused too.
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert!(!crate::net::your_turn(&st));
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        assert_eq!(last(&st), Some("not your turn"));
        assert!(st.net.engaged.is_some());
        // On the grid there is nothing to engage.
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        on_the_grid(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        assert_eq!(last(&st), Some("nothing to engage"));
    }

    #[test]
    fn jacking_out_a_dump_and_leaving_end_the_engagement() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        engaged_at_the_gate(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Enter)]);
        assert!(!st.player.as_ref().unwrap().jacked_in());
        s.update(&mut st, &[]);
        assert!(st.net.engaged.is_none(), "jacked out: no fight");
        // A dump mid-turn: nobody acts on a body on the street.
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = engaged_at_the_gate(&mut st, &mut s);
        st.net.trace(b).heat = 99;
        s.update(&mut st, &[key(KeyCode::Char(' ')), key(KeyCode::Char('1'))]);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(!st.player.as_ref().unwrap().jacked_in(), "{:?}", last(&st));
        assert!(st.net.engaged.is_none());
        let hp = st.character.hp;
        for _ in 0..5 {
            st.tick += crate::net::PACE_TICKS;
            s.update(&mut st, &[]);
        }
        assert_eq!(st.character.hp, hp);
    }

    #[test]
    fn the_run_keeps_its_progress_across_calm_and_turns() {
        use crate::net::BEAT_TICKS;
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        lock_run(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('1')), key(KeyCode::Char(' '))]);
        assert_eq!(last(&st), Some("crack 1/3"));
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        s.update(&mut st, &[key(KeyCode::Char(' ')), key(KeyCode::Char(' '))]);
        assert_eq!(st.net.run.as_ref().map(|r| r.done), Some(2));
        let text = draw_screen(&s, &st, 140, 40);
        assert!(
            text.contains("RUN  crack · door lock  ▰▰▱ 2/3") && !text.contains("next roll"),
            "{text}"
        );
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        assert!(st.net.engaged.is_none());
        st.tick += BEAT_TICKS;
        s.update(&mut st, &[]);
        assert!(
            last(&st).unwrap().contains("lock cracked"),
            "{:?}",
            last(&st)
        );
    }

    #[test]
    fn the_console_engages_and_calms_by_hand() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let hide = room_tile(&st, b);
        stand(&mut st, &mut s, Place::Construct(b), hide);
        let at = game::At {
            place: Place::Construct(b),
            pos: hide,
        };
        assert_eq!(
            crate::screens::console::run(&mut st, at, "engage").unwrap(),
            "engaged by hand"
        );
        assert!(st.net.engaged.as_ref().is_some_and(|e| e.manual));
        assert_eq!(
            crate::screens::console::run(&mut st, at, "engage").unwrap(),
            "already engaged"
        );
        assert_eq!(
            crate::screens::console::run(&mut st, at, "calm").unwrap(),
            "calm"
        );
        assert!(st.net.engaged.is_none());
        assert_eq!(
            crate::screens::console::run(&mut st, at, "calm").unwrap(),
            "already calm"
        );
        st.net.wake_all(b, hide);
        crate::screens::console::run(&mut st, at, "engage").unwrap();
        assert_eq!(
            crate::screens::console::run(&mut st, at, "calm").unwrap_err(),
            "still hunted"
        );
        on_the_grid(&mut st, &mut s);
        assert_eq!(
            crate::screens::console::run(&mut st, at, "engage").unwrap_err(),
            "not in a construct"
        );
    }

    /// A start tile and a key whose direction has `len` open tiles after it: floor,
    /// no node, no piece. Rooms first, where no piece patrols or looks, then the rest.
    fn straight(st: &AppState, b: citygen::BuildingId, len: i32) -> (Pos, KeyCode, Dir) {
        let k = st.constructs.get(b).unwrap();
        let open = |p: Pos| {
            k.tiles.get(p).is_some_and(|t| t.is_walkable())
                && k.node_at(p).is_none()
                && k.guard_at(p).is_none()
        };
        let size = k.tiles.size();
        let mut tiles: Vec<Pos> = k.rooms.iter().flat_map(|r| r.iter()).collect();
        tiles.extend(
            (0..size.h as i32).flat_map(|y| (0..size.w as i32).map(move |x| Pos::new(x, y))),
        );
        for start in tiles {
            {
                if !open(start) {
                    continue;
                }
                for (key, d) in [
                    (KeyCode::Char('l'), Dir::E),
                    (KeyCode::Char('h'), Dir::W),
                    (KeyCode::Char('j'), Dir::S),
                    (KeyCode::Char('k'), Dir::N),
                ] {
                    let mut p = start;
                    let mut n = 0;
                    while n < len {
                        p = p.step(d);
                        if !open(p) {
                            break;
                        }
                        n += 1;
                    }
                    if n == len {
                        return (start, key, d);
                    }
                }
            }
        }
        panic!("no straight of {len} in the construct");
    }

    /// Stuns every piece for the rest of the test, so a scanned line stays open.
    fn freeze_ice(st: &mut AppState, b: citygen::BuildingId) {
        let k = st.constructs.get(b).unwrap().clone();
        st.net.ice_of(&k);
        for i in st.net.ice.get_mut(&b).unwrap() {
            i.stunned = 99;
        }
    }

    fn walk_tick(st: &mut AppState, s: &mut MapScreen) {
        st.tick += crate::net::WALK_TICKS;
        s.update(st, &[]);
    }

    #[test]
    fn path_to_walks_the_floor_around_pieces() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let (start, _, d) = straight(&st, b, 3);
        let k = st.constructs.get(b).unwrap();
        let end = start.step(d).step(d).step(d);
        assert_eq!(
            crate::net::path_to(k, start, end),
            Some(vec![start.step(d), start.step(d).step(d), end])
        );
        assert_eq!(crate::net::path_to(k, start, start), Some(vec![]));
        let guard = k.guards[0].pos;
        assert_eq!(
            crate::net::path_to(k, start, guard),
            None,
            "a standing piece blocks"
        );
        let wall = (0..k.tiles.size().h as i32)
            .flat_map(|y| (0..k.tiles.size().w as i32).map(move |x| Pos::new(x, y)))
            .find(|p| !k.tiles[*p].is_walkable())
            .unwrap();
        assert_eq!(crate::net::path_to(k, start, wall), None);
    }

    #[test]
    fn in_a_fight_the_arrows_aim_and_the_path_lights_to_your_moves() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        freeze_ice(&mut st, b);
        let (start, go, d) = straight(&st, b, 4);
        stand(&mut st, &mut s, Place::Construct(b), start);
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        assert!(st.net.engaged.is_some());
        // Three moves left: the aim walks three tiles out and no further.
        st.net.engaged.as_mut().unwrap().moves = 3;
        for n in 1..=3 {
            s.update(&mut st, &[key(go)]);
            let mut want = start;
            for _ in 0..n {
                want = want.step(d);
            }
            assert_eq!(s.aim, Some(want), "step {n}");
            assert_eq!(player(&st).1, start, "aiming does not move you");
        }
        let three = s.aim;
        s.update(&mut st, &[key(go)]);
        assert_eq!(s.aim, three, "the fourth tile is out of reach");
        let (path, lit) = s.aim_path(&st).expect("a path");
        assert_eq!((path.len(), lit), (3, 3), "three moves light three tiles");
        let text = draw_screen(&s, &st, 120, 40);
        assert!(text.contains('×'), "{text}");
        assert!(text.matches('•').count() >= 2, "{text}");
        assert!(!text.contains("[space]"));
        // esc drops the aim and nothing else happens.
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Stay
        ));
        assert_eq!(s.aim, None);
        // Aimed onto a piece: no path, the cross stands alone.
        let guard = st.constructs.get(b).unwrap().guards[0].pos;
        let near = beside(&st, b, guard);
        stand(&mut st, &mut s, Place::Construct(b), near);
        s.update(&mut st, &[key(dir_between(near, guard))]);
        assert_eq!(s.aim, Some(guard));
        assert_eq!(s.aim_path(&st), None);
        assert!(draw_screen(&s, &st, 120, 40).contains('×'));
    }

    #[test]
    fn enter_walks_the_path_a_tile_per_walk_ticks_and_locks_the_keys() {
        use crate::net::BASE_MOVE;
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        freeze_ice(&mut st, b);
        let (start, go, d) = straight(&st, b, 3);
        stand(&mut st, &mut s, Place::Construct(b), start);
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        s.update(&mut st, &[key(go), key(go), key(go)]);
        s.update(&mut st, &[key(KeyCode::Enter)]);
        assert!(s.walking.is_some());
        assert_eq!(s.aim, None, "the aim is spent");
        assert_eq!(player(&st).1, start, "nothing is instant");
        walk_tick(&mut st, &mut s);
        assert_eq!(player(&st).1, start.step(d));
        assert_eq!(st.net.engaged.as_ref().unwrap().moves, BASE_MOVE - 1);
        // A key during the walk is swallowed.
        let logged_before = st.log.len();
        s.update(&mut st, &[key(KeyCode::Char('.')), key(go)]);
        assert_eq!(player(&st).1, start.step(d));
        assert_eq!(st.log.len(), logged_before);
        assert!(
            crate::net::your_turn(&st),
            "the wait key did not end the turn"
        );
        walk_tick(&mut st, &mut s);
        walk_tick(&mut st, &mut s);
        assert_eq!(player(&st).1, start.step(d).step(d).step(d));
        assert_eq!(st.net.engaged.as_ref().unwrap().moves, BASE_MOVE - 3);
        assert!(s.walking.is_none());
        // One move left: the aim reaches one tile back, no more; walking it spends
        // the last move and the act half opens.
        let back = dir_between(start.step(d), start);
        s.update(&mut st, &[key(back), key(back)]);
        assert_eq!(s.aim, Some(start.step(d).step(d)));
        s.update(&mut st, &[key(KeyCode::Enter)]);
        walk_tick(&mut st, &mut s);
        assert_eq!(player(&st).1, start.step(d).step(d));
        assert_eq!(st.net.engaged.as_ref().unwrap().moves, 0);
        assert_eq!(crate::net::phase(&st), Some(crate::net::Phase::Act));
        s.update(&mut st, &[key(back)]);
        assert_eq!(last(&st), Some("moves are done"));
    }

    #[test]
    fn a_walk_aborts_the_run_and_a_wake_stops_the_walk_where_it_is() {
        use crate::net::BEAT_TICKS;
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let (b, _) = lock_run(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('1')), key(KeyCode::Char(' '))]);
        assert!(st.net.run.is_some());
        // In calm `g` aims from where you stand.
        let here = player(&st).1;
        let k = st.constructs.get(b).unwrap().clone();
        let step = here
            .neighbors4()
            .find(|p| {
                k.tiles.get(*p).is_some_and(|t| t.is_walkable())
                    && k.node_at(*p).is_none()
                    && k.guard_at(*p).is_none()
            })
            .expect("an open tile beside the lock");
        s.update(&mut st, &[key(KeyCode::Char('g'))]);
        assert_eq!(s.aim, Some(here));
        s.update(
            &mut st,
            &[key(dir_between(here, step)), key(KeyCode::Enter)],
        );
        walk_tick(&mut st, &mut s);
        assert_eq!(player(&st).1, step);
        assert!(st.net.run.is_none(), "a step aborts the run");
        assert!(logged(&st, "run aborted"));
        assert!(st.net.engaged.is_none(), "calm: the walk cost nothing");
        // A longer walk in calm, and a wake on the beat stops it at the tile reached.
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        freeze_ice(&mut st, b);
        let (start, go, d) = straight(&st, b, 3);
        stand(&mut st, &mut s, Place::Construct(b), start);
        assert!(st.net.engaged.is_none(), "stunned pieces see nothing");
        st.net.beat = st.clock_tick();
        s.update(
            &mut st,
            &[
                key(KeyCode::Char('g')),
                key(go),
                key(go),
                key(go),
                key(KeyCode::Enter),
            ],
        );
        walk_tick(&mut st, &mut s);
        assert_eq!(player(&st).1, start.step(d));
        st.net.wake_all(b, start.step(d));
        st.tick += BEAT_TICKS;
        s.update(&mut st, &[]);
        assert!(st.net.engaged.is_some(), "the beat saw the awake piece");
        assert!(s.walking.is_none(), "the walk stops");
        assert_eq!(
            player(&st).1,
            start.step(d),
            "at the tile reached, no double step"
        );
        assert_eq!(
            st.net.engaged.as_ref().unwrap().moves,
            crate::net::BASE_MOVE
        );
    }

    /// A bolt as the tests read it: colour, ticks, from, to.
    type Bolt = ((u8, u8, u8), u64, Pos, Vec<Pos>);
    /// A mark as the tests read it: colour, ticks, on.
    type Mark = ((u8, u8, u8), u64, Vec<Pos>);

    fn bolts(st: &AppState) -> Vec<Bolt> {
        st.fx
            .iter()
            .filter_map(|f| match f.fx {
                game::Fx::Bolt { colour, ticks } => {
                    Some((colour, ticks, f.origin, f.targets.clone()))
                }
                _ => None,
            })
            .collect()
    }

    fn marks(st: &AppState) -> Vec<Mark> {
        st.fx
            .iter()
            .filter_map(|f| match f.fx {
                game::Fx::Mark { colour, ticks } => Some((colour, ticks, f.targets.clone())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_program_turn_bolts_to_its_target_and_a_slip_marks_it() {
        use crate::net::{BOLT_ROLL, MARK_SLIP, MARK_TICKS, ROLL_TICKS};
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let (b, lock) = lock_run(&mut st, &mut s);
        let (_, lock_pos) = node_of(&st, b, NodeKind::DoorLock);
        let _ = lock;
        s.update(&mut st, &[key(KeyCode::Char('1'))]);
        st.fx.clear();
        // The roll bolts from you to the lock.
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(last(&st), Some("crack 1/3"));
        let me = player(&st).1;
        assert_eq!(
            bolts(&st),
            vec![(BOLT_ROLL, ROLL_TICKS, me, vec![lock_pos])]
        );
        assert!(marks(&st).is_empty());
        // A slip bolts and marks the target in magenta.
        st.fx.clear();
        st.net.force_roll = Some(1);
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert!(
            last(&st).unwrap().starts_with("crack slips"),
            "{:?}",
            last(&st)
        );
        assert_eq!(bolts(&st).len(), 1);
        assert_eq!(marks(&st), vec![(MARK_SLIP, MARK_TICKS, vec![lock_pos])]);
        // A hurried roll by space in a fight bolts the same.
        st.net.force_roll = Some(10);
        s.update(&mut st, &[key(KeyCode::Char('e')), key(KeyCode::Char(' '))]);
        st.fx.clear();
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(bolts(&st).len(), 1);
    }

    #[test]
    fn an_attack_bolts_white_a_hit_bolts_back_and_a_shield_marks_you_cyan() {
        use crate::net::{BOLT_HIT, MARK_SHIELD, MARK_TICKS, ROLL_TICKS};
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        st.net.force_roll = Some(10);
        let gate = st.constructs.get(b).unwrap().guards[0].pos;
        stand(
            &mut st,
            &mut s,
            Place::Construct(b),
            Pos::new(gate.x + 2, gate.y),
        );
        settle(&mut st, &mut s);
        let gate = st.constructs.get(b).unwrap().guards[0].pos;
        let me = player(&st).1;
        assert_eq!(gate.chebyshev(me), 1, "{:?}", last(&st));
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
        st.fx.clear();
        roll(&mut st, &mut s);
        assert!(
            last(&st).unwrap().starts_with("attack · grey ICE"),
            "{:?}",
            last(&st)
        );
        assert_eq!(
            bolts(&st),
            vec![((240, 240, 240), ROLL_TICKS, me, vec![gate])]
        );
        // Its turn: the hit comes back as a magenta bolt; with a shield up, a cyan
        // mark sits on you where it was absorbed. The pace outlives a bolt, so the
        // fx are gathered as the turn plays.
        st.net.shield = 1;
        st.fx.clear();
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        let (mut seen_bolts, mut seen_marks) = (Vec::new(), Vec::new());
        for _ in 0..80 {
            if crate::net::your_turn(&st) {
                break;
            }
            st.tick += crate::net::PACE_TICKS;
            s.update(&mut st, &[]);
            seen_bolts.extend(bolts(&st));
            seen_marks.extend(marks(&st));
        }
        assert!(logged(&st, "shield absorbs the hit"));
        let gate = st.constructs.get(b).unwrap().guards[0].pos;
        assert!(
            seen_bolts.contains(&(BOLT_HIT, ROLL_TICKS, gate, vec![me])),
            "{seen_bolts:?}"
        );
        assert!(
            seen_marks.contains(&(MARK_SHIELD, MARK_TICKS, vec![me])),
            "{seen_marks:?}"
        );
    }

    #[test]
    fn black_ice_hits_with_a_red_bolt() {
        use crate::net::ROLL_TICKS;
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct_at(&mut st, &mut s, 255);
        let vault_guard = st.constructs.get(b).unwrap().guards[1].clone();
        let beside_vault = Pos::new(vault_guard.post.x - 1, vault_guard.post.y);
        st.fx.clear();
        stand(&mut st, &mut s, Place::Construct(b), beside_vault);
        assert!(logged(&st, "black ICE hits"), "{:?}", last(&st));
        let from = st.constructs.get(b).unwrap().guards[1].pos;
        assert!(
            bolts(&st).contains(&((255, 40, 40), ROLL_TICKS, from, vec![beside_vault])),
            "{:?}",
            bolts(&st)
        );
    }

    #[test]
    fn the_alarm_beat_gives_the_ice_no_free_action() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let (b, _) = lock_run(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('1')), key(KeyCode::Char(' '))]);
        assert_eq!(last(&st), Some("crack 1/3"));
        st.net.trace(b).heat = game::ALARM_HEAT - 1;
        st.net.force_roll = Some(1);
        let before: Vec<Pos> = st
            .constructs
            .get(b)
            .unwrap()
            .guards
            .iter()
            .map(|g| g.pos)
            .collect();
        let (hp, focus) = (st.character.hp, st.character.focus);
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        assert!(logged(&st, "ALARM"), "{:?}", last(&st));
        assert!(
            st.net
                .engaged
                .as_ref()
                .is_some_and(|e| !e.manual && crate::net::your_turn(&st))
        );
        let after: Vec<Pos> = st
            .constructs
            .get(b)
            .unwrap()
            .guards
            .iter()
            .map(|g| g.pos)
            .collect();
        assert_eq!(
            after, before,
            "the fight starts on your turn: nothing moves outside the pace"
        );
        assert_eq!((st.character.hp, st.character.focus), (hp, focus));
    }

    #[test]
    fn on_turns_the_arrows_never_walk_you_out_of_the_construct() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let entry = st.constructs.get(b).unwrap().entry;
        {
            let p = st.player.as_mut().unwrap();
            p.place = Place::Construct(b);
            p.pos = entry;
        }
        s.place = Place::Construct(b);
        crate::net::raise_alarm(&mut st, b);
        assert!(st.net.engaged.is_some() && crate::net::your_turn(&st));
        for code in [
            KeyCode::Char('h'),
            KeyCode::Char('j'),
            KeyCode::Char('k'),
            KeyCode::Char('l'),
        ] {
            s.update(&mut st, &[key(code)]);
            assert_eq!(
                player(&st),
                (Place::Construct(b), entry),
                "{code:?} aims, it does not walk"
            );
            assert!(st.net.engaged.is_some(), "{code:?} left the fight");
        }
    }

    #[test]
    fn a_piece_that_stands_again_joins_the_end_of_the_order() {
        use crate::net::Actor;
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let hide = room_tile(&st, b);
        stand(&mut st, &mut s, Place::Construct(b), hide);
        // The gate guard broken: lifted until the hour or the alert ends.
        let target = game::HackTarget::Guard {
            building: b,
            index: 0,
        };
        let kind = crate::net::target_kind(&st, b, target).unwrap();
        let now = st.net.now(&st);
        st.net.land(b, target, kind, game::Op::Attack, now);
        st.net.sync_guards(&mut st.constructs, b, now);
        {
            let k = st.constructs.get(b).unwrap();
            assert_eq!(k.guard_at(k.guards[0].pos), None, "lifted");
        }
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        assert_eq!(
            st.net.engaged.as_ref().unwrap().order,
            vec![Actor::You, Actor::Ice(1)]
        );
        // The hour ends the hack: the piece stands and, at the round's wrap, takes
        // its place at the end of the order.
        st.net
            .minute(now + game::HACK_MINUTES, Some(b), &mut st.constructs);
        st.net
            .sync_guards(&mut st.constructs, b, now + game::HACK_MINUTES);
        {
            let k = st.constructs.get(b).unwrap();
            assert_eq!(k.guard_at(k.guards[0].pos), Some(0), "standing again");
        }
        st.net.wake_all(b, hide);
        pass_turn(&mut st, &mut s);
        assert_eq!(
            st.net.engaged.as_ref().unwrap().order,
            vec![Actor::You, Actor::Ice(1), Actor::Ice(0)]
        );
        // And it acts: a round passes and it has hunted, a step toward you or a
        // turn counted lost where no path leads into your room.
        let before = st.constructs.get(b).unwrap().guards[0].pos;
        pass_turn(&mut st, &mut s);
        let after = st.constructs.get(b).unwrap().guards[0].pos;
        let hunted = before != after
            || matches!(st.net.ice[&b][0].mode, crate::net::IceMode::Awake { lost, .. } if lost > 0);
        assert!(
            hunted,
            "the seated piece acts on its turn: {:?}",
            st.net.ice[&b][0]
        );
    }

    #[test]
    fn your_turn_is_move_then_act() {
        use crate::net::Phase;
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        lock_run(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        assert_eq!(crate::net::phase(&st), Some(Phase::Move));
        // Programs wait for the move: the keys draw hollow and refuse.
        s.update(&mut st, &[key(KeyCode::Char('1'))]);
        assert_eq!(last(&st), Some("move first · space to stay"));
        assert!(st.net.run.is_none());
        let text = draw_screen(&s, &st, 140, 40);
        assert!(text.contains("YOUR TURN · MOVE"), "{text}");
        assert!(text.contains("[1] CRACK ○"), "{text}");
        s.update(&mut st, &[key(KeyCode::Char('a'))]);
        assert_eq!(last(&st), Some("move first · space to stay"));
        // Space stays put: the act phase, programs live, no more moving.
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(crate::net::phase(&st), Some(Phase::Act));
        let text = draw_screen(&s, &st, 140, 40);
        assert!(text.contains("YOUR TURN · ACT"), "{text}");
        assert!(text.contains("[1] CRACK ●"), "{text}");
        s.update(&mut st, &[key(KeyCode::Char('l'))]);
        assert_eq!(last(&st), Some("moves are done"));
        s.update(&mut st, &[key(KeyCode::Char('g'))]);
        assert_eq!(last(&st), Some("moves are done"));
        assert!(text.contains("1-5 pick a program"), "{text}");
        s.update(&mut st, &[key(KeyCode::Char('1'))]);
        let text = draw_screen(&s, &st, 140, 40);
        assert!(text.contains("CRACK › door lock"), "{text}");
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(last(&st), Some("crack 1/3"));
        assert_eq!(
            st.net.engaged.as_ref().unwrap().ap,
            1,
            "one action point spent"
        );
        let text = draw_screen(&s, &st, 140, 40);
        assert!(
            text.contains("space runs crack · door lock (1 AP)"),
            "{text}"
        );
        // The next turn opens on the move again.
        pass_turn(&mut st, &mut s);
        assert_eq!(crate::net::phase(&st), Some(Phase::Move));
        assert_eq!(st.net.engaged.as_ref().unwrap().round, 2);
    }

    #[test]
    fn the_aim_stops_at_your_moves_and_the_last_move_opens_act() {
        use crate::net::Phase;
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        freeze_ice(&mut st, b);
        let (start, go, d) = straight(&st, b, 3);
        stand(&mut st, &mut s, Place::Construct(b), start);
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        st.net.engaged.as_mut().unwrap().moves = 2;
        s.update(&mut st, &[key(go), key(go), key(go)]);
        assert_eq!(
            s.aim,
            Some(start.step(d).step(d)),
            "two moves: two tiles, no further"
        );
        s.update(&mut st, &[key(KeyCode::Enter)]);
        walk_tick(&mut st, &mut s);
        assert_eq!(
            crate::net::phase(&st),
            Some(Phase::Move),
            "a move left: still moving"
        );
        walk_tick(&mut st, &mut s);
        assert_eq!(player(&st).1, start.step(d).step(d));
        assert_eq!(
            crate::net::phase(&st),
            Some(Phase::Act),
            "no move left: act"
        );
    }

    #[test]
    fn a_program_key_picks_a_target_and_space_runs_it() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let (b, _) = lock_run(&mut st, &mut s);
        let (_, lock_pos) = node_of(&st, b, NodeKind::DoorLock);
        let before = st.log.len();
        s.update(&mut st, &[key(KeyCode::Char('1'))]);
        let t = s.targeting.as_ref().expect("picking a target");
        assert_eq!(
            t.candidates[t.picked].1, lock_pos,
            "the lock beside you is picked"
        );
        assert!(st.net.run.is_none(), "nothing runs until space");
        assert_eq!(st.log.len(), before);
        let text = draw_screen(&s, &st, 140, 40);
        assert!(text.contains("CRACK › door lock"), "{text}");
        assert!(text.contains("space runs it"), "{text}");
        assert!(text.contains('×'), "{text}");
        // esc cancels; a key again picks again; space runs it: armed and rolled.
        s.update(&mut st, &[key(KeyCode::Esc)]);
        assert!(s.targeting.is_none());
        s.update(&mut st, &[key(KeyCode::Char('1')), key(KeyCode::Char(' '))]);
        assert!(s.targeting.is_none());
        assert_eq!(last(&st), Some("crack 1/3"));
        assert!(logged(&st, "crack starts · 3 turns"));
        assert!(st.net.engaged.is_none(), "calm: no action point asked");
        // On turns the run costs the action point as it is confirmed.
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        lock_run(&mut st, &mut s);
        s.update(&mut st, &[key(KeyCode::Char('e')), key(KeyCode::Char(' '))]);
        s.update(&mut st, &[key(KeyCode::Char('1'))]);
        let text = draw_screen(&s, &st, 140, 40);
        assert!(text.contains("space runs it (1 AP)"), "{text}");
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(last(&st), Some("crack 1/3"));
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, crate::net::BASE_AP - 1);
    }

    #[test]
    fn attack_reaches_a_piece_in_sight_at_range_and_tab_cycles_targets() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        st.net.force_roll = Some(10);
        let entry = st.constructs.get(b).unwrap().entry;
        {
            let p = st.player.as_mut().unwrap();
            p.place = Place::Construct(b);
            p.pos = entry;
        }
        s.place = Place::Construct(b);
        crate::net::raise_alarm(&mut st, b);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        let me = player(&st).1;
        let gate = st.constructs.get(b).unwrap().guards[0].pos;
        assert!(gate.chebyshev(me) >= 2, "not beside you");
        st.character
            .unload(&st.content, &ItemId::new("copy"))
            .unwrap();
        st.character.give(ItemId::new("attack"), 1);
        st.character
            .load(&st.content, &ItemId::new("attack"))
            .unwrap();
        s.update(&mut st, &[key(KeyCode::Char('2'))]);
        let t = s.targeting.as_ref().expect("picking a target");
        let picks: Vec<Pos> = t.candidates.iter().map(|c| c.1).collect();
        assert!(
            picks.contains(&gate),
            "the gate guard in sight at range: {picks:?}"
        );
        assert_eq!(t.candidates[t.picked].1, gate, "the nearest piece first");
        let n = t.candidates.len();
        s.update(&mut st, &[key(KeyCode::Tab)]);
        assert_eq!(s.targeting.as_ref().unwrap().picked, 1 % n);
        s.update(&mut st, &[key(KeyCode::BackTab)]);
        assert_eq!(s.targeting.as_ref().unwrap().picked, 0);
        st.fx.clear();
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(last(&st), Some("attack · grey ICE 4/6"), "{:?}", last(&st));
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, crate::net::BASE_AP - 1);
        assert_eq!(bolts(&st).len(), 1, "a white bolt to the piece");
        // The same key, the same piece: the attack goes on, nothing is aborted.
        let aborted = st
            .log
            .iter()
            .filter(|e| e.text.contains("run aborted"))
            .count();
        s.update(&mut st, &[key(KeyCode::Char('2')), key(KeyCode::Char(' '))]);
        assert_eq!(last(&st), Some("attack · grey ICE 2/6"), "{:?}", last(&st));
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, 0);
        assert_eq!(
            st.log
                .iter()
                .filter(|e| e.text.contains("run aborted"))
                .count(),
            aborted,
            "the second pick continues the run"
        );
        // Out of sight, out of reach: the next roll ends the attack.
        let hide = room_tile(&st, b);
        stand(&mut st, &mut s, Place::Construct(b), hide);
        roll(&mut st, &mut s);
        assert_eq!(last(&st), Some("attack · out of reach"), "{:?}", last(&st));
    }

    #[test]
    fn on_turns_the_aim_reaches_the_gate_and_the_walk_leaves_the_construct() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        freeze_ice(&mut st, b);
        let k = st.constructs.get(b).unwrap().clone();
        let entry = k.entry;
        let (out_key, out) = [
            (KeyCode::Char('h'), Dir::W),
            (KeyCode::Char('l'), Dir::E),
            (KeyCode::Char('k'), Dir::N),
            (KeyCode::Char('j'), Dir::S),
        ]
        .into_iter()
        .map(|(key, d)| (key, entry.step(d)))
        .find(|(_, p)| !k.tiles.in_bounds(*p))
        .expect("the gate lies off the map beside the entry");
        // One tile in from the entry, on turns by hand: the aim walks back onto the
        // entry and one more onto the gate.
        let inside = entry
            .neighbors4()
            .find(|p| k.tiles.get(*p).is_some_and(|t| t.is_walkable()) && k.guard_at(*p).is_none())
            .expect("a tile inside the entry");
        stand(&mut st, &mut s, Place::Construct(b), inside);
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        s.update(&mut st, &[key(dir_between(inside, entry)), key(out_key)]);
        assert_eq!(s.aim, Some(out), "the gate is aimed at");
        let (path, lit) = s.aim_path(&st).expect("a path out");
        assert_eq!(path, vec![entry, out]);
        assert_eq!(lit, 4);
        s.update(&mut st, &[key(KeyCode::Enter)]);
        walk_tick(&mut st, &mut s);
        assert_eq!(player(&st), (Place::Construct(b), entry));
        walk_tick(&mut st, &mut s);
        assert_eq!(
            player(&st).0,
            Place::Stratum(StratumKind::Net),
            "out on the grid"
        );
        assert!(logged(&st, "Leaving"), "{:?}", last(&st));
        s.update(&mut st, &[]);
        assert!(st.net.engaged.is_none() && s.walking.is_none() && s.aim.is_none());
    }

    #[test]
    fn broken_ice_stays_down_through_the_alarm_and_the_alert() {
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        let hide = room_tile(&st, b);
        stand(&mut st, &mut s, Place::Construct(b), hide);
        // On alert already, the gate guard is broken.
        crate::net::raise_alarm(&mut st, b);
        let target = game::HackTarget::Guard {
            building: b,
            index: 0,
        };
        let kind = crate::net::target_kind(&st, b, target).unwrap();
        let now = st.net.now(&st);
        st.net.land(b, target, kind, game::Op::Attack, now);
        st.net.sync_guards(&mut st.constructs, b, now);
        let down = |st: &AppState| {
            let k = st.constructs.get(b).unwrap();
            k.guard_at(k.guards[0].pos).is_none()
        };
        assert!(down(&st), "broken");
        // A second alarm does not stand it; an alert ending early does not either.
        crate::net::raise_alarm(&mut st, b);
        assert!(down(&st), "the alarm leaves a broken piece broken");
        st.net.trace(b).alert_until = Some(now + 10);
        st.net.minute(now + 11, Some(b), &mut st.constructs);
        assert!(down(&st), "the alert's end leaves it broken");
        // The hour brings it back.
        st.net
            .minute(now + game::HACK_MINUTES, Some(b), &mut st.constructs);
        assert!(!down(&st), "whole again after the hour");
    }

    #[test]
    fn space_on_an_aimed_tile_walks_and_on_your_own_tile_stays() {
        use crate::net::Phase;
        let mut st = state8();
        let mut s = MapScreen::explore(0);
        let b = in_construct(&mut st, &mut s);
        freeze_ice(&mut st, b);
        let (start, go, d) = straight(&st, b, 2);
        stand(&mut st, &mut s, Place::Construct(b), start);
        s.update(&mut st, &[key(KeyCode::Char('e'))]);
        // Aimed one tile out, space confirms the walk like enter does.
        s.update(&mut st, &[key(go), key(KeyCode::Char(' '))]);
        assert!(s.walking.is_some(), "space walks the aim");
        assert_eq!(crate::net::phase(&st), Some(Phase::Move));
        walk_tick(&mut st, &mut s);
        assert_eq!(player(&st).1, start.step(d));
        // Aimed back onto your own tile, space stays put: the act half opens.
        let back = dir_between(start.step(d), start);
        s.update(&mut st, &[key(go), key(back)]);
        assert_eq!(s.aim, Some(start.step(d)), "the aim is back on you");
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(s.walking.is_none());
        assert_eq!(crate::net::phase(&st), Some(Phase::Act));
        assert_eq!(player(&st).1, start.step(d));
    }
}
